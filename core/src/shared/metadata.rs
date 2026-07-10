use std::collections::HashSet;
use std::path::Path;

use colored::Colorize;
use eyre::{Result, eyre};
use tracing::{error, warn};
use walkdir::WalkDir;

use crate::config::CollectionConfig;
use crate::converter;
use crate::schema::{ContentSchema, format_errors, validate_metadata};

/// Computes the permalink for a content file based on its relative path.
fn compute_permalink(rel_path: &Path, routes_url: &str) -> String {
    let mut permalink_path = rel_path.with_extension("");
    if permalink_path
        .file_name()
        .is_some_and(|name| name == "index")
    {
        permalink_path = permalink_path
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
    }
    let permalink = permalink_path.to_string_lossy();
    if permalink.is_empty() {
        format!("{}/", routes_url)
    } else {
        format!("{}/{}/", routes_url, permalink)
    }
}

/// Converts TOML datetime values to RFC3339 strings in a metadata table.
fn normalize_datetimes(metadata: &mut toml::Value) {
    if let toml::Value::Table(table) = metadata {
        for (_k, v) in table.iter_mut() {
            if let toml::Value::Datetime(dt) = v {
                *v = toml::Value::String(dt.to_string());
            }
        }
    }
}

fn normalize_categories(metadata: &mut toml::Value) {
    if let toml::Value::Table(table) = metadata
        && let Some(toml::Value::Array(cats)) = table.get_mut("categories")
    {
        for cat in cats.iter_mut() {
            if let toml::Value::String(s) = cat {
                *cat = toml::Value::String(s.trim().to_string());
            }
        }
    }
}

/// Full metadata + HTML conversion from pre-read content.
///
/// This is the inner function that does the actual work. It does NOT read from disk.
pub fn load_metadata_from_content(content: &str, rel_path: &Path, routes_url: &str) -> toml::Value {
    let (html, toc) = match converter::html::convert(content, routes_url) {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to convert {}: {}", rel_path.display(), e);
            return toml::Value::Table(toml::map::Map::new());
        }
    };
    let mut metadata =
        match converter::meta::convert(content, Some(converter::html::toc_to_toml(&toc))) {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to parse metadata for {}: {}", rel_path.display(), e);
                toml::Value::Table(toml::map::Map::new())
            }
        };
    let permalink = compute_permalink(rel_path, routes_url);
    normalize_datetimes(&mut metadata);
    normalize_categories(&mut metadata);
    if let toml::Value::Table(ref mut table) = metadata {
        table.insert("raw".to_string(), toml::Value::String(html.to_string()));
        table.insert("permalink".to_string(), toml::Value::String(permalink));
    }
    metadata
}

/// Lightweight metadata extraction from pre-read content (no parse_tree).
///
/// This is the inner function that does the actual work. It does NOT read from disk.
pub fn extract_metadata_from_content(
    content: &str,
    rel_path: &Path,
    routes_url: &str,
) -> toml::Value {
    let mut metadata = match converter::meta::convert(content, None) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to parse metadata for {}: {}", rel_path.display(), e);
            toml::Value::Table(toml::map::Map::new())
        }
    };
    let permalink = compute_permalink(rel_path, routes_url);
    normalize_datetimes(&mut metadata);
    if let toml::Value::Table(ref mut table) = metadata {
        table.insert("permalink".to_string(), toml::Value::String(permalink));
    }
    metadata
}

/// Validates content metadata against a schema.
///
/// This function validates the metadata of a content file against a provided schema.
/// If validation errors are found, they are logged in a user-friendly format.
///
/// # Arguments
/// * `content_dir` - The content directory.
/// * `path` - The path to the content file.
/// * `schema` - The schema to validate the metadata against.
/// * `as_warnings` - Whether to format errors as warnings or errors.
///
/// # Returns
/// * `Result<String>` - Empty String if the validation did not find any error, an String containing all the errors otherwise.
pub fn validate_content_metadata(
    content_dir: &Path,
    path: &Path,
    metadata: &toml::Value,
    schema: &ContentSchema,
    as_warnings: bool,
) -> Result<String> {
    let relative_path = path
        .strip_prefix(content_dir)
        .map_err(|e| eyre!("Path {} is not under content_dir: {}", path.display(), e))?;
    // We do not need to do anything with the metadata permalink here so we pass an empty string to it
    let metadata_map = metadata
        .as_table()
        .ok_or_else(|| eyre!("Metadata for {} is not a table", path.display()))?
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let content_path = relative_path
        .to_str()
        .ok_or_else(|| eyre!("Non-UTF-8 path: {}", path.display()))?
        .replace('\\', "/")
        .trim_end_matches(".norg")
        .to_string();

    let schema_nodes = schema.resolve_path(&content_path);
    let merged_schema = ContentSchema::merge_hierarchy(&schema_nodes);
    let errors = validate_metadata(&metadata_map, &merged_schema);

    if !errors.is_empty() {
        return Ok(format_errors(path, &content_path, &errors, as_warnings));
    }
    Ok(String::new())
}

/// Collects all unique categories from post metadata
pub fn collect_all_posts_categories(posts: &[toml::Value]) -> HashSet<String> {
    let mut categories = HashSet::new();

    for post in posts {
        if let Some(cats) = post.get("categories").and_then(|v| v.as_array()) {
            for cat in cats {
                if let Some(cat_str) = cat.as_str() {
                    categories.insert(cat_str.trim().to_lowercase());
                }
            }
        }
    }

    categories
}

pub fn collect_all_posts_metadata(
    content_dir: &Path,
    routes_url: &str,
    collections: &[CollectionConfig],
) -> Result<Vec<toml::Value>> {
    // Collect paths first (WalkDir is sync)
    let entries: Vec<_> = WalkDir::new(content_dir)
        .into_iter()
        .filter_map(|e| match e {
            Ok(e) => Some(e),
            Err(e) => {
                warn!("WalkDir error: {}", e);
                None
            }
        })
        .filter(|e| {
            let path = e.path();
            let is_norg_file = path.extension().is_some_and(|ext| ext == "norg");
            let is_post = path.strip_prefix(content_dir).is_ok_and(|p| {
                collections.iter().any(|c| {
                    p.starts_with(&c.dir) && p != Path::new(&format!("{}/index.norg", c.dir))
                })
            });
            is_norg_file && is_post
        })
        .map(|e| {
            let path = e.path().to_path_buf();
            let rel_path = path.strip_prefix(content_dir).unwrap().to_path_buf();
            (path, rel_path)
        })
        .collect();

    // Process metadata extraction
    let mut posts: Vec<toml::Value> = entries
        .into_iter()
        .map(|(path, rel_path)| match std::fs::read_to_string(&path) {
            Ok(content) => load_metadata_from_content(&content, &rel_path, routes_url),
            Err(_) => {
                error!(
                    "{} {}",
                    "Norg file not found for".bold(),
                    rel_path.display()
                );
                toml::Value::Table(toml::map::Map::new())
            }
        })
        .collect();

    posts.sort_by(|a, b| {
        let a_date = a
            .get("created")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let b_date = b
            .get("created")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let parse_date = |s: &str| {
            chrono::DateTime::parse_from_rfc3339(s)
                .unwrap_or_else(|_| {
                    warn!(
                        "Post has invalid 'created' date '{}', defaulting to epoch for sort",
                        s
                    );
                    chrono::DateTime::from_timestamp(0, 0).unwrap().into()
                })
                .with_timezone(&chrono::Utc)
        };

        parse_date(b_date).cmp(&parse_date(a_date))
    });

    Ok(posts)
}
