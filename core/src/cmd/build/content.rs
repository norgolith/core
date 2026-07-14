use std::path::Path;

use miette::{IntoDiagnostic, Result, WrapErr, miette};
use rss::Channel;
use tera::{Context, Tera};
use tracing::{instrument, warn};

use crate::{config, shared};

fn collect_xml_templates(tera: &Tera) -> Vec<String> {
    tera.get_template_names()
        .filter(|name| name.ends_with(".xml"))
        .map(|name| name.to_string())
        .collect()
}

#[instrument(level = "debug", skip(tera, shared_context, public_dir))]
pub(super) fn generate_xml_feeds(
    tera: &Tera,
    shared_context: &Context,
    public_dir: &Path,
) -> Result<(usize, Vec<String>)> {
    let xml_templates = collect_xml_templates(tera);
    let count = xml_templates.len();
    if count == 0 {
        return Ok((0, vec![]));
    }

    let context = shared_context.clone();

    for template_name in &xml_templates {
        let rendered = tera
            .render(template_name, &context)
            .map_err(|e| miette!("Failed to render '{}': {}", template_name, e))?;

        if (template_name.contains("rss") && template_name.ends_with(".xml"))
            && let Err(e) = Channel::read_from(rendered.as_bytes())
        {
            warn!(
                template = %template_name,
                "'{}' does not validate as RSS ({}); written as-is",
                template_name,
                e
            );
        }

        let output_path = public_dir.join(template_name);
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).into_diagnostic().wrap_err(format!(
                "Failed to create output directory for '{}'",
                template_name
            ))?;
        }
        std::fs::write(&output_path, &rendered)
            .into_diagnostic().wrap_err(format!("Failed to write '{}'", output_path.display()))?;
    }

    Ok((count, xml_templates))
}

pub(super) fn build_category_pages(
    tera: &Tera,
    public_dir: &Path,
    posts: &[toml::Value],
    config: &config::SiteConfig,
    collections: &shared::PrecomputedCollections,
) -> Result<usize> {
    let categories = shared::collect_all_posts_categories(posts);
    let categories_dir = public_dir.join(&config.categories_dir);

    if posts.is_empty() {
        return Ok(0);
    }

    let content = shared::render_category_index(tera, posts, config, collections)?;

    std::fs::create_dir_all(&categories_dir).into_diagnostic().wrap_err("Failed to create categories directory")?;
    std::fs::write(categories_dir.join("index.html"), content).into_diagnostic().wrap_err("Failed to write categories index")?;
    let mut page_count = 1usize;

    for category in categories {
        let cat_posts: Vec<_> = posts
            .iter()
            .filter(|post| {
                post.get("categories")
                    .and_then(|c| c.as_array())
                    .map(|cats| {
                        cats.iter()
                            .any(|c| c.as_str().map(|s| s.trim()) == Some(category.as_str()))
                    })
                    .unwrap_or(false)
            })
            .collect();

        let content = shared::render_category_page(tera, &category, &cat_posts, config)?;

        let cat_dir = categories_dir.join(&category);
        std::fs::create_dir_all(&cat_dir).into_diagnostic().wrap_err("Failed to create category directory")?;

        std::fs::write(cat_dir.join("index.html"), content).into_diagnostic().wrap_err("Failed to write category page")?;
        page_count += 1;
    }

    Ok(page_count)
}

#[instrument(level = "debug", skip(tera, shared_context, public_dir))]
pub(super) fn build_error_pages(
    tera: &Tera,
    shared_context: &Context,
    public_dir: &Path,
) -> Result<usize> {
    let mut count = 0usize;
    for name in &["404.html", "500.html"] {
        if !tera.get_template_names().any(|n| n == *name) {
            continue;
        }
        let rendered = tera
            .render(name, shared_context)
            .map_err(|e| miette!("Failed to render {}: {}", name, e))?;
        std::fs::write(public_dir.join(name), &rendered)
            .into_diagnostic().wrap_err(format!("Failed to write {}", name))?;
        count += 1;
    }
    Ok(count)
}
