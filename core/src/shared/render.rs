

use colored::Colorize;
use eyre::{Result, eyre};
use tera::{Context, Tera};

use crate::config::SiteConfig;

use super::PrecomputedCollections;
use super::metadata::collect_all_posts_categories;

/// Pre-computes collection subsets once, avoiding O(posts × collections) per page.
pub fn precompute_collection_subsets(
    all_posts: &[toml::Value],
    config: &SiteConfig,
) -> PrecomputedCollections {
    config
        .collections
        .iter()
        .map(|collection| {
            let dir_prefix = format!("{}/", collection.dir);
            let subset: Vec<_> = all_posts
                .iter()
                .filter(|p| {
                    p.get("rel_path")
                        .and_then(|v| v.as_str())
                        .map(|rel_path| {
                            rel_path == collection.dir || rel_path.starts_with(&dir_prefix)
                        })
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            (collection.name.clone(), subset)
        })
        .collect()
}

/// Builds a Tera context with shared site data (config, posts, version, collection subsets).
///
/// This context is identical for every page render. Only `metadata` and `content` differ.
/// Build once and clone per page to avoid redundant serialization.
pub fn build_shared_context(
    posts: &[toml::Value],
    config: &SiteConfig,
    collections: &PrecomputedCollections,
) -> Context {
    let mut context = Context::new();
    context.insert("config", config);
    context.insert("posts", posts);
    context.insert(
        "lith_version",
        option_env!("LITH_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
    );
    for (name, subset) in collections {
        let key = format!("collection_{name}");
        context.insert(key, subset);
    }
    context
}

/// Render full norg page by converting it to HTML and applying tera template.
///
/// Uses a pre-built shared context; only `metadata` and `content` are inserted per page.
pub fn render_norg_page(
    tera: &Tera,
    metadata: &toml::Value,
    shared_context: &Context,
) -> Result<String> {
    let content = metadata.get("raw").and_then(|v| v.as_str()).unwrap_or("");
    let layout = metadata
        .get("layout")
        .and_then(|v| v.as_str())
        .unwrap_or("default");

    let mut context = shared_context.clone();
    context.insert("content", content);
    context.insert("metadata", metadata);

    tera.render(&format!("{}.html", layout), &context)
        .map_err(|e| {
            let msg = format!("Failed to render template for '{}': {}", layout, e).bold();
            eyre!(msg)
        })
}

pub fn render_category_index(
    tera: &Tera,
    posts: &[toml::Value],
    config: &SiteConfig,
    collections: &PrecomputedCollections,
) -> Result<String> {
    let categories = collect_all_posts_categories(posts);
    let context = {
        let mut ctx = Context::new();
        ctx.insert("config", config);
        ctx.insert("posts", posts);
        ctx.insert(
            "lith_version",
            option_env!("LITH_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
        );
        for (name, subset) in collections {
            ctx.insert(name.clone(), subset);
        }
        ctx.insert("categories", &categories.iter().collect::<Vec<_>>());
        ctx
    };

    tera.render("categories.html", &context).map_err(|e| {
        eyre!("Failed to render categories index: {e}")
    })
}

pub fn render_category_page(
    tera: &Tera,
    name: &str,
    cat_posts: &[&toml::Value],
    config: &SiteConfig,
) -> Result<String> {
    let context = {
        let mut ctx = Context::new();
        ctx.insert("config", config);
        ctx.insert("category", name);
        ctx.insert("posts", cat_posts);
        ctx.insert(
            "lith_version",
            option_env!("LITH_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
        );
        ctx
    };
    tera.render("category.html", &context).map_err(|e| {
        eyre!("Failed to render category page: {e}")
    })
}
