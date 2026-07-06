use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use colored::Colorize;
use eyre::{Result, bail, eyre};
use tera::{Context, Tera};
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, error, info, instrument};
use walkdir::WalkDir;

use crate::shared::{BuildContext, SitePaths};
use crate::{config, plugin, shortcode, shared};

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
    pub plugin_mgr: Arc<plugin::PluginManager>,
}

impl ServerState {
    #[instrument(level = "debug", skip(self))]
    pub async fn reload_templates(&self) -> Result<()> {
        debug!("Reloading templates");
        let new_tera = crate::tera::init(
            self.paths.templates.to_str().unwrap(),
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
        let config_content = tokio::fs::read_to_string(&self.paths.config_file).await?;
        let new_config: config::SiteConfig = toml::from_str(&config_content)?;

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
        let cache = self.cache.read().await;

        match render_all_pages(
            BuildContext {
                tera: &tera,
                paths: &self.paths,
                site_config: &config,
                plugins: &self.plugin_mgr,
            },
            &posts,
            &self.routes_url,
            &cache,
        ) {
            Ok(new_pages) => {
                let mut pages = self.rendered_pages.write().await;
                *pages = new_pages;
                info!("Rendered pages cache rebuilt");
            }
            Err(e) => error!("Failed to rebuild rendered pages: {}", e),
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
            .map_err(|e| eyre!("Failed to send reload signal: {}", e))
    }
}

pub fn render_all_pages(
    ctx: BuildContext<'_>,
    posts: &[toml::Value],
    routes_url: &str,
    cache: &crate::cache::BuildCache,
) -> Result<HashMap<String, String>> {
    let mut pages = HashMap::new();

    let collections = shared::precompute_collection_subsets(posts, ctx.site_config);
    let shared_context = shared::build_shared_context(posts, ctx.site_config, &collections);

    // Render content pages
    for entry in WalkDir::new(&ctx.paths.content)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "norg"))
    {
        let path = entry.path();
        let rel_path = match path.strip_prefix(&ctx.paths.content) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };

        // Draft check
        let metadata = shared::extract_metadata_from_content(&content, rel_path, routes_url);
        let is_draft = metadata
            .get("draft")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_draft {
            continue;
        }

        // Full load with HTML conversion (reuse build_cache if available)
        let cache_key = rel_path.with_extension("");
        let mut metadata = if let Some(cached) = cache.get(&cache_key, &content) {
            serde_json::from_value(cached).unwrap_or_else(|_| {
                shared::load_metadata_from_content(&content, rel_path, routes_url)
            })
        } else {
            shared::load_metadata_from_content(&content, rel_path, routes_url)
        };

        ctx.plugins
            .run_post_convert(ctx.site_config, &mut metadata, rel_path);

        // Process shortcode component calls inside @embed html islands.
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

        let mut body = shared::render_norg_page(ctx.tera, &metadata, &shared_context)?;

        body = ctx
            .plugins
            .run_post_render(ctx.site_config, body, &metadata, rel_path);

        body = body.replace(
            &ctx.site_config.root_url.replace("://", ":&#x2F;&#x2F;"),
            routes_url,
        );

        let url_path = format!("/{}", rel_path.with_extension("").display());
        pages.insert(url_path, body);
    }

    // Pre-render category index
    if !posts.is_empty() {
        if let Ok(body) =
            shared::render_category_index(ctx.tera, posts, ctx.site_config, &collections)
        {
            let body = body.replace(
                &ctx.site_config.root_url.replace("://", ":&#x2F;&#x2F;"),
                routes_url,
            );
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
                let body = body.replace(
                    &ctx.site_config.root_url.replace("://", ":&#x2F;&#x2F;"),
                    routes_url,
                );
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

    let config_content = tokio::fs::read_to_string(&root).await?;
    let site_config: config::SiteConfig = toml::from_str(&config_content)?;

    let validation_errors = site_config.validate();
    if !validation_errors.is_empty() {
        for error in &validation_errors {
            eprintln!("{}", error);
        }
        bail!("Site configuration has validation errors");
    }

    let root_dir = root.parent().unwrap().to_path_buf();
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

    let tera = crate::tera::init(paths.templates.to_str().unwrap(), &paths.theme_templates)?;

    let (reload_tx, _) = broadcast::channel(16);

    let posts =
        shared::collect_all_posts_metadata(&paths.content, &routes_url, &site_config.collections)?;

    let cache = crate::cache::BuildCache::open(&root_dir)?;

    let plugin_mgr = plugin::PluginManager::load(&root_dir);
    let _ = plugin::sandbox::apply_landlock(&root_dir);
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
                error!(
                    "{} plugin '{}': {}",
                    "Plugin error:".red().bold(),
                    p.name.bold(),
                    e
                );
            }
        }
    }

    let rendered_pages = render_all_pages(
        BuildContext {
            tera: &tera,
            paths: &paths,
            site_config: &site_config,
            plugins: &plugin_mgr,
        },
        &posts,
        &routes_url,
        &cache,
    )?;

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
        plugin_mgr: Arc::new(plugin_mgr),
    }))
}
