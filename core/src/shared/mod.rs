use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use tera::Tera;

use crate::config::SiteConfig;
use crate::plugin::PluginManager;

pub(crate) mod metadata;
pub(crate) mod render;

pub use metadata::*;
pub use render::*;

/// Pre-computed collection subsets: collection name → filtered posts.
pub type PrecomputedCollections = HashMap<String, Vec<toml::Value>>;

/// Directory structure of a Norgolith site.
///
/// This struct defines paths to key directories used during the build process, and dev server.
/// Including build artifacts, public output, content sources, and theme resources.
#[derive(Debug)]
pub(crate) struct SitePaths {
    pub public: PathBuf,
    #[allow(dead_code)] // used by dev.rs only
    pub config_file: PathBuf,
    pub content: PathBuf,
    pub assets: PathBuf,
    pub templates: PathBuf,
    pub theme_assets: PathBuf,
    pub theme_templates: PathBuf,
}

impl SitePaths {
    /// Creates a new `SitePaths` instance based on the provided root directory.
    ///
    /// Initializes paths for build artifacts, public output, content sources,
    /// assets, themes, and templates by combining with root subdirectories.
    ///
    /// # Arguments
    /// * `root` - Root directory containing norgolith.toml config file
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            public: root.join("public"),
            config_file: root.join("norgolith.toml"),
            content: root.join("content"),
            assets: root.join("assets"),
            templates: root.join("templates"),
            theme_assets: root.join("theme/assets"),
            theme_templates: root.join("theme/templates"),
        }
    }
}

/// Shared state threaded through the build pipeline: tera engine, paths,
/// config, plugins. Per-function extras (posts, shared_context, cache, minify)
/// stay as separate args since they aren't common to all call sites.
#[derive(Clone, Copy)]
pub(crate) struct BuildContext<'a> {
    pub tera: &'a Tera,
    pub paths: &'a SitePaths,
    pub site_config: &'a SiteConfig,
    pub plugins: &'a PluginManager,
}

pub fn get_elapsed_time(instant: Instant) -> String {
    let duration = instant.elapsed();
    let secs = duration.as_secs_f64();

    if secs < 1.0 {
        format!("{:.0}ms", secs * 1000.0)
    } else {
        format!("{:.1}s", secs)
    }
}
