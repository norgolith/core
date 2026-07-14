mod assets;
mod content;
mod timings;

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Instant,
};

use colored::{ColoredString, Colorize};
use miette::{IntoDiagnostic, NamedSource, Result, WrapErr, bail, miette};
use tera::Context;
use tracing::{debug, error, instrument, warn};
use walkdir::WalkDir;

use super::seo;
use crate::shared::{BuildContext, SitePaths};
use crate::{cache::BuildCache, config, fs, plugin, shortcode, shared};

use assets::copy_assets;
use content::{build_category_pages, build_error_pages, generate_xml_feeds};
use timings::BuildTimings;

pub(super) type CacheInsert = (PathBuf, String, serde_json::Value);
pub(super) type BuildResult = Result<Option<(PathBuf, String, String, Option<CacheInsert>)>>;

fn href_root_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r#"href="(/|&#x2F;)"#).expect("valid regex"))
}

#[instrument(skip(public_dir))]
fn prepare_build_directory(public_dir: &Path) -> Result<()> {
    debug!(path = %public_dir.display(), "Preparing build directory");
    if public_dir.exists() {
        for entry in std::fs::read_dir(public_dir).into_diagnostic().wrap_err(format!(
            "{}: {}",
            "Failed to read existing public directory".bold(),
            public_dir.display()
        ))? {
            let entry = entry.into_diagnostic().wrap_err(format!(
                "{}: {}",
                "Failed to iterate existing public directory".bold(),
                public_dir.display()
            ))?;
            let path = entry.path();
            let file_name = path.file_name().and_then(|name| name.to_str());

            if file_name == Some(".git") {
                debug!(path = %path.display(), "Keeping git metadata directory");
                continue;
            }

            let metadata = entry.metadata().into_diagnostic().wrap_err(format!(
                "{}: {}",
                "Failed to stat existing public entry".bold(),
                path.display()
            ))?;

            if metadata.is_dir() {
                std::fs::remove_dir_all(&path).into_diagnostic().wrap_err(format!(
                    "{}: {}",
                    "Failed to remove existing public directory entry".bold(),
                    path.display()
                ))?;
            } else {
                std::fs::remove_file(&path).into_diagnostic().wrap_err(format!(
                    "{}: {}",
                    "Failed to remove existing public file entry".bold(),
                    path.display()
                ))?;
            }
        }
    } else {
        debug!(path = %public_dir.display(), "Creating public directory");
        std::fs::create_dir_all(public_dir).into_diagnostic().wrap_err(format!(
            "{}: {}",
            "Failed to create public directory".bold(),
            public_dir.display()
        ))?;
    }

    debug!("Build directory prepared successfully");
    Ok(())
}

#[instrument]
fn determine_public_path(public_dir: &Path, rel_path: &Path) -> Result<PathBuf> {
    let stem = rel_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| miette!("Invalid file stem for path: {}", rel_path.display()))?;
    if stem == "index" {
        Ok(public_dir.join(rel_path).with_extension("html"))
    } else {
        Ok(public_dir
            .join(rel_path.parent().unwrap_or(Path::new("")))
            .join(stem)
            .join("index.html"))
    }
}

fn precreate_output_dirs(paths: &SitePaths) -> Result<()> {
    let entries = WalkDir::new(&paths.content)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "norg"));

    let mut dirs = std::collections::HashSet::new();
    for entry in entries {
        let rel_path = entry.path().strip_prefix(&paths.content).into_diagnostic().wrap_err("Failed to resolve content path")?;
        if let Ok(public_path) = determine_public_path(&paths.public, rel_path)
            && let Some(parent) = public_path.parent()
        {
            dirs.insert(parent.to_path_buf());
        }
    }
    for dir in &dirs {
        std::fs::create_dir_all(dir).into_diagnostic().wrap_err("Failed to create output directory")?;
    }
    Ok(())
}

#[instrument(skip(rendered))]
fn write_public_file(public_path: &Path, rendered: &str) -> Result<bool> {
    if let Ok(existing) = std::fs::read(public_path)
        && existing == rendered.as_bytes()
    {
        return Ok(false);
    }
    std::fs::write(public_path, rendered).into_diagnostic().wrap_err(format!(
        "{}: {}",
        "Failed to write to public path".bold(),
        public_path.display()
    ))?;
    Ok(true)
}

#[instrument(level = "debug", skip(ctx, shared_context, cache))]
fn build_contents(
    ctx: BuildContext<'_>,
    shared_context: &Context,
    cache: &mut BuildCache,
    minify: bool,
) -> Result<(usize, Vec<String>, BuildTimings)> {
    use rayon::prelude::*;

    let entries: Vec<_> = WalkDir::new(&ctx.paths.content)
        .into_iter()
        .filter_map(|e| match e {
            Ok(e) => Some(e),
            Err(e) => {
                warn!("WalkDir error: {}", e);
                None
            }
        })
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "norg"))
        .collect();

    let results: Vec<BuildResult> = entries
        .par_iter()
        .map(|entry| {
            let path = entry.path();
            build_content_entry(path, ctx, shared_context, cache, minify)
        })
        .collect();

    let mut buffered_writes = Vec::new();
    let mut permalinks = Vec::new();
    for result in results {
        match result {
            Ok(Some((public_path, content, permalink, cache_entry))) => {
                buffered_writes.push((public_path, content));
                permalinks.push(permalink);
                if let Some((key, content_str, metadata)) = cache_entry {
                    cache.insert(&key, &content_str, metadata);
                }
            }
            Ok(None) => {}
            Err(e) => error!("{:?}", e),
        }
    }

    let write_start = Instant::now();
    let mut built_count = 0usize;
    for (public_path, content) in &buffered_writes {
        if write_public_file(public_path, content)? {
            built_count += 1;
        }
    }
    let write_ms = write_start.elapsed().as_millis();

    let mut timings = BuildTimings::new();
    timings.page_write_ms = write_ms;
    timings.page_count = built_count;

    Ok((built_count, permalinks, timings))
}

#[instrument(level = "debug", skip(path, ctx, shared_context, cache))]
fn build_content_entry(
    path: &Path,
    ctx: BuildContext<'_>,
    shared_context: &Context,
    cache: &BuildCache,
    minify: bool,
) -> BuildResult {
    let rel_path = path
        .strip_prefix(&ctx.paths.content)
        .into_diagnostic().wrap_err("Failed to strip prefix")?;

    let Ok(content) = std::fs::read_to_string(path) else {
        error!(
            "{} {}",
            "Norg file not found for".bold(),
            rel_path.display()
        );
        return Ok(None);
    };

    let metadata =
        shared::extract_metadata_from_content(&content, rel_path, &ctx.site_config.root_url);

    if let Some(schema) = &ctx.site_config.content_schema
        && !rel_path.starts_with(&ctx.site_config.categories_dir)
    {
        shared::validate_content_metadata(
            &ctx.paths.content,
            path,
            &metadata,
            schema,
            false,
        )?;
    }

    let is_draft = match metadata.get("draft") {
        Some(val) => val.as_bool().ok_or_else(|| {
            miette!("'draft' field must be a boolean for '{}'", path.display())
        })?,
        None => false,
    };
    if is_draft {
        return Ok(None);
    }

    let cache_key = rel_path.with_extension("");
    let cached = cache.get(&cache_key, &content);

    let (mut metadata, cache_insert) = if let Some(cached) = cached {
        match serde_json::from_value::<toml::Value>(cached.clone()) {
            Ok(md) => (md, None),
            Err(_) => {
                let md = shared::load_metadata_from_content(
                    &content,
                    rel_path,
                    &ctx.site_config.root_url,
                );
                let cache_val = serde_json::to_value(&md).unwrap_or_default();
                (md, Some((cache_key, content.clone(), cache_val)))
            }
        }
    } else {
        let md = shared::load_metadata_from_content(&content, rel_path, &ctx.site_config.root_url);
        let cache_val = serde_json::to_value(&md).unwrap_or_default();
        (md, Some((cache_key, content.clone(), cache_val)))
    };

    ctx.plugins
        .run_post_convert(ctx.site_config, &mut metadata, rel_path);

    // Process shortcode component calls inside @embed html islands.
    // Runs after plugin post_convert (which may modify raw) but before
    // render_norg_page wraps content in the Tera layout.
    if let Some(raw) = metadata.get("raw").and_then(|v| v.as_str())
        && raw.contains("<!--lith:embed-->")
    {
        let mut shortcode_ctx = shared_context.clone();
        shortcode_ctx.insert("metadata", &metadata);
        if let Ok(processed) = shortcode::process(raw, ctx.tera, &shortcode_ctx)
            && let toml::Value::Table(ref mut table) = metadata
        {
            table.insert("raw".to_string(), toml::Value::String(processed));
        }
    }

    let public_path = determine_public_path(&ctx.paths.public, rel_path)?;

    let mut rendered = shared::render_norg_page(ctx.tera, &metadata, shared_context)?;

    rendered = ctx
        .plugins
        .run_post_render(ctx.site_config, rendered, &metadata, rel_path);

    let href_re = href_root_re();
    rendered = href_re
        .replace_all(&rendered, format!("href=\"{}/", ctx.site_config.root_url))
        .into_owned();

    let rendered = if minify && !rendered.is_empty() {
        assets::minify_html_content(rendered)?
    } else {
        rendered
    };

    let permalink = metadata
        .get("permalink")
        .and_then(|v| v.as_str())
        .unwrap_or("/")
        .to_string();

    Ok(Some((public_path, rendered, permalink, cache_insert)))
}

#[instrument(skip(minify))]
pub fn build(minify: bool) -> Result<()> {
    let Some(root) = fs::find_config_file()? else {
        bail!(
            "{}: not in a Norgolith site directory",
            "Could not build the site".bold()
        );
    };

    println!(
        "{} Building site{}...",
        "→".cyan().bold(),
        if minify {
            " (minified)".dimmed()
        } else {
            ColoredString::from("")
        }
    );
    let build_start = Instant::now();
    let mut timings = BuildTimings::new();

    // Load site configuration
    let t = Instant::now();
    let config_content = std::fs::read_to_string(&root).into_diagnostic().wrap_err("Failed to read config file")?;
    let site_config: config::SiteConfig = toml::from_str(&config_content).map_err(|e| {
        miette!("Failed to parse site configuration: {}", e)
            .with_source_code(NamedSource::new(root.display().to_string(), config_content))
    })?;
    let validation_errors = site_config.validate();
    if !validation_errors.is_empty() {
        bail!(
            "Site configuration has validation errors:\n{}",
            validation_errors.join("\n")
        );
    }
    debug!(?site_config, "Loaded site configuration");
    timings.config_ms = t.elapsed().as_millis();

    let root_dir = root.parent().unwrap().to_path_buf();
    let paths = SitePaths::new(root_dir.clone());

    // Initialize Tera
    let t = Instant::now();
    debug!("Initializing template engine");
    let tera = crate::tera::init(paths.templates.to_str().unwrap(), &paths.theme_templates)?;
    timings.tera_ms = t.elapsed().as_millis();

    // Load plugins and apply sandbox
    let t = Instant::now();
    let plugin_mgr = plugin::PluginManager::load(&root_dir);
    if let Err(e) = plugin::sandbox::apply_landlock(&root_dir) {
        warn!("{}", e);
    }
    timings.plugins_ms = t.elapsed().as_millis();

    println!();
    if !plugin_mgr.is_empty() {
        println!(
            "  {} {}  {} plugins",
            "•".green(),
            format!("{:<12}", "Plugins").bold(),
            plugin_mgr.len()
        );
    }

    // pre_build hook
    if plugin_mgr.has_hook(plugin::HOOK_PRE_BUILD) {
        let config_json = serde_json::to_string(&site_config).unwrap_or_default();
        for p in plugin_mgr.plugins() {
            if let Some(f) = p.hooks.pre_build
                && let Err(e) = plugin_mgr.call_hook(p, f, &config_json)
            {
                error!(
                    "{} plugin '{}': {}",
                    "Plugin error:".red().bold(),
                    p.name.bold(),
                    e
                );
            }
        }
    }

    // Prepare build directory
    let t = Instant::now();
    prepare_build_directory(&paths.public)?;
    timings.prepare_dir_ms = t.elapsed().as_millis();

    // Pre-create output directories for all content entries
    let t = Instant::now();
    precreate_output_dirs(&paths)?;
    timings.prepare_dir_ms += t.elapsed().as_millis();

    // Collect post metadata
    let t = Instant::now();
    let posts: Vec<_> = shared::collect_all_posts_metadata(
        &paths.content,
        &site_config.root_url,
        &site_config.collections,
    )?
    .into_iter()
    .filter(|post| {
        !post.get("draft").and_then(|v| v.as_bool()).unwrap_or(false)
    })
    .collect();
    timings.collect_posts_ms = t.elapsed().as_millis();

    // Pre-compute collection subsets
    let t = Instant::now();
    let collections = shared::precompute_collection_subsets(&posts, &site_config);
    timings.collections_ms = t.elapsed().as_millis();

    // Build shared context
    let t = Instant::now();
    let shared_context = shared::build_shared_context(&posts, &site_config, &collections);
    timings.shared_ctx_ms = t.elapsed().as_millis();

    // Open cache
    let t = Instant::now();
    let mut cache = BuildCache::open(&root_dir)?;
    timings.cache_open_ms = t.elapsed().as_millis();

    // Build content
    let t = Instant::now();
    let ctx = BuildContext {
        tera: &tera,
        paths: &paths,
        site_config: &site_config,
        plugins: &plugin_mgr,
    };
    let (page_count, permalinks, content_timings) =
        build_contents(ctx, &shared_context, &mut cache, minify)?;
    timings.content_ms = t.elapsed().as_millis();
    timings.page_count = page_count;
    timings.page_write_ms = content_timings.page_write_ms;
    println!(
        "  {} {}  {:<12}  {}",
        "•".green(),
        format!("{:<12}", "Content").bold(),
        format!("{} pages", page_count),
        shared::get_elapsed_time(t).dimmed()
    );

    // Category pages
    let t = Instant::now();
    let cat_count = build_category_pages(&tera, &paths.public, &posts, &site_config, &collections)?;
    timings.categories_ms = t.elapsed().as_millis();
    if cat_count > 0 {
        println!(
            "  {} {}  {:<12}  {}",
            "•".green(),
            format!("{:<12}", "Categories").bold(),
            format!("{} pages", cat_count),
            shared::get_elapsed_time(t).dimmed()
        );
    }

    // XML feeds
    let t = Instant::now();
    let (feed_count, feed_names) = generate_xml_feeds(&tera, &shared_context, &paths.public)?;
    timings.feeds_ms = t.elapsed().as_millis();
    if feed_count > 0 {
        println!(
            "  {} {}  {:<12}  {}",
            "•".green(),
            format!("{:<12}", "Feeds").bold(),
            format!("{} files", feed_count),
            shared::get_elapsed_time(t).dimmed()
        );
    }

    // SEO generation
    let t = Instant::now();
    let mut seo_count = 0usize;
    let seo_enabled = site_config.seo.is_some() || site_config.robots.is_some();
    if seo_enabled {
        let sitemap_enabled = site_config.seo.as_ref().is_none_or(|s| s.sitemap);
        if sitemap_enabled {
            use std::collections::HashMap;
            let date_map: HashMap<&str, &str> = posts
                .iter()
                .filter_map(|p| {
                    let permalink = p.get("permalink")?.as_str()?;
                    let date = p.get("updated").or_else(|| p.get("created"))?.as_str()?;
                    Some((permalink, date))
                })
                .collect();

            let mut urls = Vec::with_capacity(permalinks.len() + 16);

            urls.push(seo::SitemapUrl {
                loc: "/".into(),
                lastmod: None,
            });

            for p in &permalinks {
                let lastmod = date_map.get(p.as_str()).map(|s| s.to_string());
                urls.push(seo::SitemapUrl {
                    loc: p.clone(),
                    lastmod,
                });
            }

            if !posts.is_empty() {
                let categories = shared::collect_all_posts_categories(&posts);
                let categories_dir = &site_config.categories_dir;
                urls.push(seo::SitemapUrl {
                    loc: format!("/{}/", categories_dir),
                    lastmod: None,
                });
                for cat in &categories {
                    urls.push(seo::SitemapUrl {
                        loc: format!("/{}/{}/", categories_dir, cat),
                        lastmod: None,
                    });
                }
            }

            for feed_name in &feed_names {
                urls.push(seo::SitemapUrl {
                    loc: format!("/{}", feed_name),
                    lastmod: None,
                });
            }

            let xml = seo::generate_sitemap_xml(&urls, &site_config.root_url);
            let output_path = paths.public.join("sitemap.xml");
            std::fs::write(&output_path, &xml).into_diagnostic().wrap_err("Failed to write sitemap.xml")?;
            seo_count += 1;
        }

        if let Some(ref robots_config) = site_config.robots
            && robots_config.enable
        {
            let content =
                seo::generate_robots_txt(&site_config, robots_config, sitemap_enabled);
            let output_path = paths.public.join("robots.txt");
            std::fs::write(&output_path, &content).into_diagnostic().wrap_err("Failed to write robots.txt")?;
            seo_count += 1;
        }
    }
    timings.seo_ms = t.elapsed().as_millis();
    if seo_count > 0 {
        println!(
            "  {} {}  {:<12}  {}",
            "•".green(),
            format!("{:<12}", "SEO").bold(),
            format!("{} files", seo_count),
            shared::get_elapsed_time(t).dimmed()
        );
    }

    // Assets
    let t = Instant::now();
    let public_assets_dir = paths.public.join("assets");
    let mut asset_count = 0usize;
    if paths.theme_assets.exists() {
        asset_count += copy_assets(&paths.theme_assets, &public_assets_dir, minify)?;
    }
    asset_count += copy_assets(&paths.assets, &public_assets_dir, minify)?;
    timings.assets_ms = t.elapsed().as_millis();
    println!(
        "  {} {}  {:<12}  {}",
        "•".green(),
        format!("{:<12}", "Assets").bold(),
        format!("{} files", asset_count),
        shared::get_elapsed_time(t).dimmed()
    );

    // Error pages
    let t = Instant::now();
    let error_page_count = build_error_pages(&tera, &shared_context, &paths.public)?;
    timings.error_pages_ms = t.elapsed().as_millis();
    if error_page_count > 0 {
        println!(
            "  {} {}  {:<12}  {}",
            "•".green(),
            format!("{:<12}", "Error pages").bold(),
            format!("{} files", error_page_count),
            shared::get_elapsed_time(t).dimmed()
        );
    }

    // post_build hook
    if plugin_mgr.has_hook(plugin::HOOK_POST_BUILD) {
        let config_json = serde_json::to_string(&site_config).unwrap_or_default();
        for p in plugin_mgr.plugins() {
            if let Some(f) = p.hooks.post_build
                && let Err(e) = plugin_mgr.call_hook(p, f, &config_json)
            {
                error!(
                    "{} plugin '{}': {}",
                    "Plugin error:".red().bold(),
                    p.name.bold(),
                    e
                );
            }
        }
    }

    println!();
    let total_ms = build_start.elapsed().as_millis();
    println!(
        "{} Built in {}",
        "✓".green().bold(),
        shared::get_elapsed_time(build_start)
    );

    let plugin_timings = plugin_mgr.hook_timings();
    if !plugin_timings.is_empty() {
        println!();
        println!("{}", "  Plugin hook timings:".dimmed());
        let mut sorted: Vec<_> = plugin_timings.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (name, duration) in sorted {
            println!("    {:<20} {:>6}ms", name.dimmed(), duration.as_millis());
        }
    }

    // Save cache
    let t = Instant::now();
    if let Err(e) = cache.save() {
        warn!("Failed to save build cache: {}", e);
    }
    timings.cache_save_ms = t.elapsed().as_millis();

    if tracing::enabled!(tracing::Level::DEBUG) {
        timings.print_summary(total_ms);
    }

    Ok(())
}
