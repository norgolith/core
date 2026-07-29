use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar};
use miette::{IntoDiagnostic, NamedSource, Result, Severity, WrapErr, miette};
use tera::{Context, Tera};
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, info, instrument};

/// Result of converting a single page: (html, raw, Option<(path, content, metadata)>)
type ConversionResult =
    Result<Option<(String, String, Option<(PathBuf, String, serde_json::Value)>)>>;
use walkdir::WalkDir;

use crate::cmd::build::progress::{make_bar, make_spinner};
use crate::cmd::build::search;
use crate::shared::{BuildContext, SitePaths};
use crate::{config, plugin, shared, shortcode};

pub(super) struct ServerState {
    pub reload_tx: Arc<broadcast::Sender<()>>,
    pub tera: Arc<RwLock<Tera>>,
    pub config: Arc<RwLock<config::SiteConfig>>,
    pub paths: SitePaths,
    pub build_drafts: bool,
    pub routes_url: String,
    pub posts: Arc<RwLock<Vec<toml::Value>>>,
    pub cache: Arc<RwLock<crate::cache::BuildCache>>,
    pub rendered_pages: Arc<RwLock<HashMap<String, String>>>,
    pub search_entries: Arc<RwLock<Vec<search::SearchEntry>>>,
    pub plugin_mgr: Arc<plugin::PluginManager>,
}

impl ServerState {
    #[instrument(level = "debug", skip(self))]
    pub async fn reload_templates(&self) -> Result<()> {
        debug!("Reloading templates");
        let new_tera = crate::tera::init(
            self.paths.templates.to_str().ok_or_else(|| {
                miette!(
                    "Templates path is not valid UTF-8: {}",
                    self.paths.templates.display()
                )
            })?,
            &self.paths.theme_templates,
        )?;
        let mut tera = self.tera.write().await;
        *tera = new_tera;

        info!("Templates reloaded successfully");
        let templates: Vec<&str> = tera.get_template_names().collect();
        debug!("There are {} templates loaded", templates.len());

        self.send_reload()?;
        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn reload_config(&self) -> Result<()> {
        debug!("Reloading config");
        let config_content = tokio::fs::read_to_string(&self.paths.config_file)
            .await
            .into_diagnostic()
            .wrap_err("Failed to read config file")?;
        let new_config: config::SiteConfig = toml::from_str(&config_content).map_err(|e| {
            miette!("Failed to parse site configuration: {}", e).with_source_code(NamedSource::new(
                self.paths.config_file.display().to_string(),
                config_content,
            ))
        })?;

        let new_posts = shared::collect_all_posts_metadata(
            &self.paths.content,
            &self.routes_url,
            &new_config.collections,
        )?;

        {
            let mut config = self.config.write().await;
            *config = new_config;
        }
        {
            let mut posts = self.posts.write().await;
            *posts = new_posts;
        }

        info!("Config reloaded successfully");
        self.send_reload()?;
        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn rebuild_rendered_pages(&self) {
        let tera = self.tera.read().await;
        let config = self.config.read().await.clone();
        let posts = self.posts.read().await.clone();
        let mut cache = self.cache.write().await;
        // NOTE(cache): fresh cache discards stale rendered_html on template rebuild
        if let Ok(fresh) = crate::cache::BuildCache::open(self.paths.config_file.parent().unwrap())
        {
            *cache = fresh;
        }

        match render_all_pages(
            BuildContext {
                tera: &tera,
                paths: &self.paths,
                site_config: &config,
                plugins: &self.plugin_mgr,
            },
            &posts,
            &self.routes_url,
            &mut cache,
            None,
        ) {
            Ok(new_pages) => {
                let entries = search::extract_entries(&new_pages);
                {
                    let mut pages = self.rendered_pages.write().await;
                    *pages = new_pages;
                }
                {
                    let mut search = self.search_entries.write().await;
                    *search = entries;
                }
                info!("Rendered pages cache rebuilt");
            }
            Err(e) => eprintln!(
                "{:?}",
                miette!(
                    severity = Severity::Warning,
                    help = "Check template and content files for errors",
                    "Failed to rebuild rendered pages: {}",
                    e
                )
            ),
        }
    }

    #[instrument(skip(self))]
    pub fn send_reload(&self) -> Result<()> {
        debug!("Sending reload signal to clients");
        if self.reload_tx.receiver_count() == 0 {
            debug!("No active receivers, skipping reload signal");
            return Ok(());
        }

        self.reload_tx
            .send(())
            .map(|_| {
                debug!(
                    "Reload signal sent to {} clients",
                    self.reload_tx.receiver_count()
                );
            })
            .map_err(|e| miette!("Failed to send reload signal: {}", e))
    }
}

pub fn render_all_pages(
    ctx: BuildContext<'_>,
    posts: &[toml::Value],
    routes_url: &str,
    cache: &mut crate::cache::BuildCache,
    progress: Option<&ProgressBar>,
) -> Result<HashMap<String, String>> {
    use rayon::prelude::*;

    let collections = shared::precompute_collection_subsets(posts, ctx.site_config);
    let shared_context = shared::build_shared_context(posts, ctx.site_config, &collections);

    let entries: Vec<_> = WalkDir::new(&ctx.paths.content)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "norg"))
        .map(|e| e.path().to_path_buf())
        .collect();

    let cache_ref: &crate::cache::BuildCache = &*cache;

    let results: Vec<ConversionResult> = entries
        .par_iter()
        .map(|path| {
            let rel_path = match path.strip_prefix(&ctx.paths.content) {
                Ok(p) => p,
                Err(_) => return Ok(None),
            };

            if let Some(b) = progress {
                b.inc(1);
            }

            let Ok(content) = std::fs::read_to_string(path) else {
                return Ok(None);
            };

            let metadata = shared::extract_metadata_from_content(&content, rel_path, routes_url)?;
            let is_draft = metadata
                .get("draft")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if is_draft {
                return Ok(None);
            }

            let cache_key = rel_path.with_extension("");

            // PERF(cache): skip re-render when rendered_html cache hit
            if let Some(cached_html) = cache_ref.get_rendered(&cache_key) {
                let body = super::handlers::rewrite_urls(
                    cached_html,
                    &ctx.site_config.root_url,
                    routes_url,
                );
                let url_path = format!("/{}", rel_path.with_extension("").display());
                return Ok(Some((url_path, body, None)));
            }

            let mut metadata =
                if let Some(cached) = cache_ref.get(&cache_key, &content) {
                    serde_json::from_value(cached).unwrap_or_else(|_| {
                        shared::load_metadata_from_content(&content, rel_path, routes_url)
                        .unwrap_or_else(|e| {
                            eprintln!("{:?}", miette!(
                                severity = Severity::Warning,
                                help = "Check the file's @document-meta section for valid metadata",
                                "Failed to load metadata for {}: {}", rel_path.display(), e
                            ));
                            toml::Value::Table(toml::map::Map::new())
                        })
                    })
                } else {
                    shared::load_metadata_from_content(&content, rel_path, routes_url)
                    .unwrap_or_else(|e| {
                        eprintln!("{:?}", miette!(
                            severity = Severity::Warning,
                            help = "Check the file's @document-meta section for valid metadata",
                            "Failed to load metadata for {}: {}", rel_path.display(), e
                        ));
                        toml::Value::Table(toml::map::Map::new())
                    })
                };

            ctx.plugins
                .run_post_convert(ctx.site_config, &mut metadata, rel_path);

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

            let body = shared::render_norg_page(ctx.tera, &metadata, &shared_context)?;

            let body = ctx
                .plugins
                .run_post_render(ctx.site_config, body, &metadata, rel_path);

            let body = super::handlers::rewrite_urls(body, &ctx.site_config.root_url, routes_url);

            let url_path = format!("/{}", rel_path.with_extension("").display());

            let cache_data = (
                cache_key,
                content,
                serde_json::to_value(&metadata).unwrap_or_default(),
            );

            Ok(Some((url_path, body, Some(cache_data))))
        })
        .collect();

    let mut pages = HashMap::new();
    for result in results {
        if let Some((url, body, cache_back)) = result? {
            if let Some((key, content, md)) = cache_back {
                cache.insert_rendered(&key, &content, md, &body);
            }
            pages.insert(url, body);
        }
    }

    // Pre-render category index
    if !posts.is_empty() {
        if let Ok(body) =
            shared::render_category_index(ctx.tera, posts, ctx.site_config, &collections)
        {
            let body = super::handlers::rewrite_urls(body, &ctx.site_config.root_url, routes_url);
            pages.insert(format!("/{}", ctx.site_config.categories_dir), body);
        }

        // Pre-render individual category pages
        let categories = shared::collect_all_posts_categories(posts);
        for category in &categories {
            let category_posts: Vec<_> = posts
                .iter()
                .filter(|post| {
                    post.get("categories")
                        .and_then(|c| c.as_array())
                        .map(|cats| cats.iter().any(|c| c.as_str() == Some(category.as_str())))
                        .unwrap_or(false)
                })
                .collect();

            let mut context = Context::new();
            context.insert("config", ctx.site_config);
            context.insert("category", category);
            context.insert("posts", &category_posts);
            context.insert(
                "lith_version",
                option_env!("LITH_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
            );

            if let Ok(body) = ctx.tera.render("category.html", &context) {
                let body =
                    super::handlers::rewrite_urls(body, &ctx.site_config.root_url, routes_url);
                let url_path = format!("/{}/{}", ctx.site_config.categories_dir, category);
                pages.insert(url_path, body);
            }
        }
    }

    // Pre-render XML feed templates
    for template_name in ctx.tera.get_template_names() {
        if !template_name.ends_with(".xml") {
            continue;
        }
        let context = shared_context.clone();
        if let Ok(body) = ctx.tera.render(template_name, &context) {
            let url_path = format!("/{}", template_name);
            pages.insert(url_path, body);
        }
    }

    debug!(count = pages.len(), "Pre-rendered pages into memory");
    Ok(pages)
}

#[instrument(skip(root, drafts, routes_url))]
pub(super) async fn setup_server_state(
    root: PathBuf,
    drafts: bool,
    routes_url: String,
) -> Result<Arc<ServerState>> {
    debug!("Setting up server state");

    let config_content = tokio::fs::read_to_string(&root)
        .await
        .into_diagnostic()
        .wrap_err("Failed to read config file")?;
    debug!("Config file path: {:?}", root);
    debug!("Config content:\n{}", config_content);
    let config_content_for_validation = config_content.clone();
    let site_config: config::SiteConfig = toml::from_str(&config_content).map_err(|e| {
        miette!("Failed to parse site configuration: {}", e)
            .with_source_code(NamedSource::new(root.display().to_string(), config_content))
    })?;
    debug!("Parsed categories_dir: {}", site_config.categories_dir);

    let validation_errors = site_config.validate();
    if !validation_errors.is_empty() {
        return Err(miette!(
            "Site configuration has validation errors:\n{}",
            validation_errors.join("\n")
        )
        .with_source_code(NamedSource::new(
            root.display().to_string(),
            config_content_for_validation,
        )));
    }

    let root_dir = root
        .parent()
        .ok_or_else(|| {
            miette!(
                "Config file path {} has no parent directory",
                root.display()
            )
        })?
        .to_path_buf();
    let mut paths = SitePaths::new(root_dir.clone());

    if let Ok(real) = tokio::fs::canonicalize(&paths.content).await {
        paths.content = real;
    }
    if let Ok(real) = tokio::fs::canonicalize(&paths.assets).await {
        paths.assets = real;
    }
    if let Ok(real) = tokio::fs::canonicalize(&paths.templates).await {
        paths.templates = real;
    }
    if let Ok(real) = tokio::fs::canonicalize(&paths.theme_assets).await {
        paths.theme_assets = real;
    }
    if let Ok(real) = tokio::fs::canonicalize(&paths.theme_templates).await {
        paths.theme_templates = real;
    }

    let templates_dir = paths.templates.to_str().ok_or_else(|| {
        miette!(
            "Templates path is not valid UTF-8: {}",
            paths.templates.display()
        )
    })?;
    let tera = crate::tera::init(templates_dir, &paths.theme_templates)?;

    let (reload_tx, _) = broadcast::channel(16);

    let mp = MultiProgress::new();
    let meta_spinner = make_spinner(&mp, "Collecting posts metadata");

    let posts =
        shared::collect_all_posts_metadata(&paths.content, &routes_url, &site_config.collections)?;

    meta_spinner.finish_and_clear();

    let mut cache = crate::cache::BuildCache::open(&root_dir)?;

    let plugin_mgr = plugin::PluginManager::load(&root_dir);
    if !plugin_mgr.is_empty() {
        mp.println(format!(
            "  {} {}  {} plugins",
            "•".green(),
            format!("{:<12}", "Plugins").bold(),
            plugin_mgr.len()
        ))
        .ok();
    }
    if let Err(e) = plugin::sandbox::apply_landlock(&root_dir) {
        eprintln!(
            "{:?}",
            miette!(
                severity = Severity::Warning,
                help = "Landlock may not be supported on your system/kernel version",
                "{}",
                e
            )
        );
    }
    if plugin_mgr.has_hook(plugin::HOOK_PRE_BUILD) {
        let input = serde_json::json!({
            "site_config": site_config,
            "pages_dir": paths.content,
            "output_dir": root_dir.join("public"),
        })
        .to_string();
        for p in plugin_mgr.plugins() {
            if let Some(f) = p.hooks.pre_build
                && let Err(e) = plugin_mgr.call_hook(p, f, &input)
            {
                eprintln!(
                    "{:?}",
                    miette!(
                        severity = Severity::Warning,
                        help = "Check the plugin output or contact plugin maintainer",
                        "{} plugin '{}': {}",
                        "Plugin error:".red().bold(),
                        p.name.bold(),
                        e
                    )
                );
            }
        }
    }

    let entry_count: usize = WalkDir::new(&paths.content)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "norg"))
        .count();

    let render_bar = make_bar(&mp, entry_count as u64, "Loading pages in-memory");

    let rendered_pages = render_all_pages(
        BuildContext {
            tera: &tera,
            paths: &paths,
            site_config: &site_config,
            plugins: &plugin_mgr,
        },
        &posts,
        &routes_url,
        &mut cache,
        Some(&render_bar),
    )?;

    render_bar.finish_and_clear();
    drop(mp);

    let search_entries = search::extract_entries(&rendered_pages);

    let tera = Arc::new(RwLock::new(tera));

    Ok(Arc::new(ServerState {
        reload_tx: Arc::new(reload_tx),
        tera,
        config: Arc::new(RwLock::new(site_config)),
        paths,
        build_drafts: drafts,
        routes_url,
        posts: Arc::new(RwLock::new(posts)),
        cache: Arc::new(RwLock::new(cache)),
        rendered_pages: Arc::new(RwLock::new(rendered_pages)),
        search_entries: Arc::new(RwLock::new(search_entries)),
        plugin_mgr: Arc::new(plugin_mgr),
    }))
}
