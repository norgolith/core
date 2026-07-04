use std::path::{Path, PathBuf};

use clap::Subcommand;
use colored::Colorize;
use eyre::{bail, Result};
use flate2::read::GzDecoder;
use git2::Repository;
use serde::Deserialize;
use tar::Archive;
use tempfile::tempdir;

use crate::plugin::{self, PluginManifest, CORE_ABI_VERSION};

#[derive(Subcommand, Clone)]
pub enum PluginCommands {
    /// List installed plugins and their status
    List,
    /// Scaffold a new plugin project
    New {
        /// Plugin name (used for directory and crate name)
        name: String,
    },
    /// Build and install a plugin
    Install {
        /// Plugin source: local path, crates.io name, or git URL (with --git)
        source: String,
        /// Install from a git repository
        #[arg(long)]
        git: bool,
        /// Git tag to checkout
        #[arg(long)]
        tag: Option<String>,
        /// Git branch to checkout
        #[arg(long)]
        branch: Option<String>,
    },
    /// Remove an installed plugin
    Uninstall {
        /// Plugin name to remove
        name: String,
    },
}

pub fn handle(subcommand: &PluginCommands) -> Result<()> {
    match subcommand {
        PluginCommands::List => list_plugins(),
        PluginCommands::New { name } => new_plugin(name),
        PluginCommands::Install { source, git, tag, branch } => {
            install(source, *git, tag.as_deref(), branch.as_deref())
        }
        PluginCommands::Uninstall { name } => uninstall_plugin(name),
    }
}

fn list_plugins() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mgr = plugin::PluginManager::load(&cwd);

    if mgr.is_empty() {
        println!("{}", "No plugins installed.".dimmed());
        println!(
            "Install one with {} or scaffold a new one with {}",
            "lith plugin install <path>".cyan(),
            "lith plugin new <name>".cyan()
        );
        return Ok(());
    }

    for p in mgr.plugins() {
        let hooks = p.manifest.hooks.declared_hooks();
        let hooks_str = if hooks.is_empty() {
            "none".dimmed().to_string()
        } else {
            hooks.join(", ")
        };

        let status = if hooks.is_empty() {
            "no hooks".yellow().to_string()
        } else {
            "ok".green().to_string()
        };

        let fs = match p.manifest.capabilities.filesystem {
            plugin::FilesystemAccess::None => "none".dimmed().to_string(),
            plugin::FilesystemAccess::Read => "read".to_string(),
            plugin::FilesystemAccess::Write => "write".to_string(),
            plugin::FilesystemAccess::ReadWrite => "read-write".to_string(),
        };
        let net = if p.manifest.capabilities.network {
            "yes".to_string()
        } else {
            "no".dimmed().to_string()
        };

        println!("{}", p.name.bold());
        println!(
            "   version:  {:<10}  hooks:      {}",
            p.version, hooks_str
        );
        println!(
            "   status:   {:<19}  priority:   {}",
            status, p.manifest.priority
        );
        println!(
            "   abi:      {:<10}  norgolith:  {}",
            p.manifest.plugin.abi, p.manifest.plugin.norgolith
        );
        println!(
            "   timeout:  {:<10}  fs:         {}     net: {}",
            format!("{}s", p.manifest.timeout_ms / 1000),
            fs,
            net
        );
    }

    println!("\n{}", format!("{} plugin(s) loaded", mgr.len()).bold());
    Ok(())
}

fn new_plugin(name: &str) -> Result<()> {
    validate_plugin_name(name)?;

    let cwd = std::env::current_dir()?;
    let plugins_dir = cwd.join("plugins").join(name);

    if plugins_dir.exists() {
        bail!("Plugin '{}' already exists at {}", name, plugins_dir.display());
    }

    std::fs::create_dir_all(plugins_dir.join("src"))?;

    // NOTE: I still need to handle the case where the plugin requires a dev norgolith version, e.g. (>=0.4.0-COMMIT_HASH)
    // For now, just use the current version of norgolith from Cargo.toml. I'll need to figure out if semver crate can
    // handle this case, or if I need to implement a custom version comparison for dev versions.
    const NORGOLITH_VERSION: &str = env!("CARGO_PKG_VERSION");

    // plugin.toml
    let manifest = format!(
        r#"[plugin]
name = "{name}"
version = "0.1.0"
norgolith = ">={NORGOLITH_VERSION}"
abi = {CORE_ABI_VERSION}

[hooks]
pre_build = false
post_convert = false
post_render = false
post_build = false

[capabilities]
filesystem = "none"
network = false

timeout_ms = 10000
priority = 100
"#
    );
    std::fs::write(plugins_dir.join("plugin.toml"), manifest)?;

    // Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
norgolith-plugin-sdk = "0.1"
"#
    );
    std::fs::write(plugins_dir.join("Cargo.toml"), cargo_toml)?;

    // src/lib.rs
    let lib_rs = format!(
        r#"use norgolith_plugin_sdk::*;

register_plugin!("{name}", "0.1.0")
    .on_post_render(|ctx| {{
        // Access per-plugin config from norgolith.toml
        // if let Some(theme) = ctx.config.as_ref().and_then(|c| c.get("theme")) {{
        //     println!("Config theme: {{theme}}");
        // }}
        Ok(Some(ctx.html))
    }})
    .register();
"#
    );
    std::fs::write(plugins_dir.join("src").join("lib.rs"), lib_rs)?;

    println!(
        "Plugin '{}' created at {}",
        name.bold(),
        plugins_dir.display()
    );
    println!("\nNext steps:");
    println!("  1. cd plugins/{}", name);
    println!("  2. Implement your hooks in src/lib.rs");
    println!("  3. Build with `cargo build`");
    println!("  4. Test with `lith plugin install plugins/{}'", name);

    Ok(())
}

fn build_plugin(source_dir: &Path) -> Result<()> {
    println!("{}", "Building plugin...".dimmed());
    let status = std::process::Command::new("cargo")
        .arg("build")
        .arg("--release")
        .current_dir(source_dir)
        .status()?;
    if !status.success() {
        bail!("cargo build failed");
    }
    Ok(())
}

fn find_built_library(source_dir: &Path, name: &str) -> Result<PathBuf> {
    let target_dir = source_dir.join("target").join("release");
    let lib_name = plugin::library_filename(name);
    let lib_path = target_dir.join(&lib_name);
    if lib_path.is_file() {
        return Ok(lib_path);
    }
    // Fallback: scan for any matching library
    let ext = plugin::library_extension();
    std::fs::read_dir(&target_dir)
        .ok()
        .and_then(|entries| {
            entries.filter_map(|e| e.ok()).find(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == ext)
                    .unwrap_or(false)
            })
        })
        .map(|e| e.path())
        .ok_or_else(|| eyre::eyre!("Built library not found in {}", target_dir.display()))
}

fn install_to_plugins(lib_path: &Path, manifest_path: &Path, name: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let dest_dir = cwd.join("plugins").join(name);
    std::fs::create_dir_all(&dest_dir)?;
    std::fs::copy(lib_path, dest_dir.join(lib_path.file_name().unwrap()))?;
    std::fs::copy(manifest_path, dest_dir.join("plugin.toml"))?;
    Ok(())
}

fn install(source: &str, git: bool, tag: Option<&str>, branch: Option<&str>) -> Result<()> {
    if git {
        return install_from_git(source, tag, branch);
    }
    // Auto-detect: starts with . or / → local, contains @ → crates.io + version, else → crates.io latest
    if source.starts_with('.') || source.starts_with('/') {
        return install_from_local(Path::new(source));
    }
    // crates.io: parse "name@version" or "name" (latest)
    if let Some((name, version)) = source.split_once('@') {
        install_from_crates_io(name, Some(version))
    } else {
        install_from_crates_io(source, None)
    }
}

fn install_from_git(url: &str, tag: Option<&str>, branch: Option<&str>) -> Result<()> {
    let tmp = tempdir()?;
    let clone_path = tmp.path();

    println!("{}", "Cloning repository...".dimmed());
    Repository::clone(url, clone_path)
        .map_err(|e| eyre::eyre!("Failed to clone {}: {}", url, e))?;

    // Checkout tag or branch if specified
    if let Some(tag_name) = tag {
        let repo = Repository::open(clone_path)?;
        let obj = repo.revparse_single(tag_name)?;
        repo.checkout_tree(&obj, None)?;
    } else if let Some(branch_name) = branch {
        let repo = Repository::open(clone_path)?;
        let reference = repo.find_branch(branch_name, git2::BranchType::Local)?;
        repo.checkout_tree(reference.get().peel_to_tree()?.as_object(), None)?;
    }

    let manifest_path = clone_path.join("plugin.toml");
    if !manifest_path.is_file() {
        bail!(
            "No plugin.toml found in {} (not a norgolith plugin?)",
            url
        );
    }

    let manifest = PluginManifest::load(&manifest_path)?;
    validate_plugin_name(&manifest.plugin.name)?;
    manifest.validate_abi()?;
    manifest.validate_semver()?;

    build_plugin(clone_path)?;
    let lib_path = find_built_library(clone_path, &manifest.plugin.name)?;
    install_to_plugins(&lib_path, &manifest_path, &manifest.plugin.name)?;

    // Cleanup on success
    tmp.close().ok();

    println!(
        "Plugin '{}' v{} installed",
        manifest.plugin.name.bold(),
        manifest.plugin.version
    );
    Ok(())
}

fn install_from_local(source_dir: &Path) -> Result<()> {
    if !source_dir.is_dir() {
        bail!("Not a directory: {}", source_dir.display());
    }

    let manifest_path = source_dir.join("plugin.toml");
    if !manifest_path.is_file() {
        bail!(
            "No plugin.toml found in {}",
            source_dir.display()
        );
    }

    let manifest = PluginManifest::load(&manifest_path)?;
    validate_plugin_name(&manifest.plugin.name)?;
    manifest.validate_abi()?;
    manifest.validate_semver()?;

    build_plugin(source_dir)?;
    let lib_path = find_built_library(source_dir, &manifest.plugin.name)?;
    install_to_plugins(&lib_path, &manifest_path, &manifest.plugin.name)?;

    println!(
        "Plugin '{}' v{} installed",
        manifest.plugin.name.bold(),
        manifest.plugin.version
    );
    Ok(())
}

#[derive(Deserialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    krate: CrateInfo,
}

#[derive(Deserialize)]
struct CrateInfo {
    max_stable_version: Option<String>,
}

fn install_from_crates_io(name: &str, version: Option<&str>) -> Result<()> {
    let version = match version {
        Some(v) => v.to_string(),
        None => {
            println!("{}", "Fetching crate info...".dimmed());
            let url = format!("https://crates.io/api/v1/crates/{}", name);
            let resp: CrateResponse = ureq::get(&url)
                .header("User-Agent", "norgolith (https://github.com/norgolith)")
                .call()
                .map_err(|e| eyre::eyre!("Failed to fetch crate info: {}", e))?
                .body_mut()
                .read_json()
                .map_err(|e| eyre::eyre!("Failed to parse crate info: {}", e))?;
            resp.krate
                .max_stable_version
                .ok_or_else(|| eyre::eyre!("No versions found for crate '{}'", name))?
        }
    };

    let tmp = tempdir()?;
    let dl_path = tmp.path().join("plugin.crate");

    println!("{}", "Downloading crate...".dimmed());
    let url = format!(
        "https://crates.io/api/v1/crates/{}/{}/download",
        name, version
    );
    let mut resp = ureq::get(&url)
        .header("User-Agent", "norgolith (https://github.com/norgolith)")
        .call()
        .map_err(|e| eyre::eyre!("Failed to download crate: {}", e))?;

    let body = resp
        .body_mut()
        .read_to_vec()
        .map_err(|e| eyre::eyre!("Failed to read response: {}", e))?;
    std::fs::write(&dl_path, &body)?;

    println!("{}", "Extracting crate...".dimmed());
    let tar_gz = std::fs::File::open(&dl_path)?;
    let decoder = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(decoder);
    archive.unpack(tmp.path()).map_err(|e| eyre::eyre!("Failed to extract crate: {}", e))?;

    // Crate tarball extracts to <name>-<version>/ directory
    let crate_dir = tmp.path().join(format!("{}-{}", name, version));
    if !crate_dir.is_dir() {
        // Try without version suffix
        let alt_dir = tmp.path().join(name);
        if alt_dir.is_dir() {
            // Found it
        } else {
            bail!(
                "Could not find extracted crate directory in {}",
                tmp.path().display()
            );
        }
    }

    let source_dir = if crate_dir.is_dir() {
        crate_dir
    } else {
        tmp.path().join(name)
    };

    // Validate: must have plugin.toml
    let manifest_path = source_dir.join("plugin.toml");
    if !manifest_path.is_file() {
        bail!(
            "Crate '{}' is not a norgolith plugin (no plugin.toml found)",
            name
        );
    }

    let manifest = PluginManifest::load(&manifest_path)?;
    validate_plugin_name(&manifest.plugin.name)?;
    manifest.validate_abi()?;
    manifest.validate_semver()?;

    build_plugin(&source_dir)?;
    let lib_path = find_built_library(&source_dir, &manifest.plugin.name)?;
    install_to_plugins(&lib_path, &manifest_path, &manifest.plugin.name)?;

    // Cleanup on success
    tmp.close().ok();

    println!(
        "Plugin '{}' v{} installed",
        manifest.plugin.name.bold(),
        manifest.plugin.version
    );
    Ok(())
}

fn validate_plugin_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Plugin name cannot be empty");
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.contains(':') {
        bail!("Invalid plugin name: '{}' (no path separators or '..' allowed)", name);
    }
    Ok(())
}

fn uninstall_plugin(name: &str) -> Result<()> {
    validate_plugin_name(name)?;

    let cwd = std::env::current_dir()?;
    let plugin_dir = cwd.join("plugins").join(name);

    if !plugin_dir.is_dir() {
        bail!("Plugin '{}' is not installed", name);
    }

    std::fs::remove_dir_all(&plugin_dir)?;
    println!("Plugin '{}' uninstalled", name.bold());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    enum SourceType {
        Local,
        CratesIo,
    }

    impl SourceType {
        fn is_local(&self) -> bool {
            matches!(self, SourceType::Local)
        }
        fn is_crates_io(&self) -> bool {
            matches!(self, SourceType::CratesIo)
        }
    }

    fn classify_source(source: &str) -> SourceType {
        if source.starts_with('.') || source.starts_with('/') {
            SourceType::Local
        } else {
            SourceType::CratesIo
        }
    }

    #[test]
    fn test_validate_plugin_name_ok() {
        assert!(validate_plugin_name("my-plugin").is_ok());
        assert!(validate_plugin_name("foo_bar").is_ok());
        assert!(validate_plugin_name("norgolith-tree-sitter-highlight").is_ok());
    }

    #[test]
    fn test_validate_plugin_name_empty() {
        assert!(validate_plugin_name("").is_err());
    }

    #[test]
    fn test_validate_plugin_name_path_separator() {
        assert!(validate_plugin_name("foo/bar").is_err());
        assert!(validate_plugin_name("foo\\bar").is_err());
    }

    #[test]
    fn test_validate_plugin_name_dotdot() {
        assert!(validate_plugin_name("..").is_err());
        assert!(validate_plugin_name("foo/..").is_err());
    }

    #[test]
    fn test_validate_plugin_name_colon() {
        assert!(validate_plugin_name("foo:bar").is_err());
    }

    #[test]
    fn test_parse_crates_io_spec_latest() {
        let source = "norgolith-tree-sitter-highlight";
        let (name, version) = match source.split_once('@') {
            Some((n, v)) => (n, Some(v)),
            None => (source, None),
        };
        assert_eq!(name, "norgolith-tree-sitter-highlight");
        assert!(version.is_none());
    }

    #[test]
    fn test_parse_crates_io_spec_versioned() {
        let source = "norgolith-tree-sitter-highlight@0.1.0";
        let (name, version) = match source.split_once('@') {
            Some((n, v)) => (n, Some(v)),
            None => (source, None),
        };
        assert_eq!(name, "norgolith-tree-sitter-highlight");
        assert_eq!(version, Some("0.1.0"));
    }

    #[test]
    fn test_classify_source_local_relative() {
        assert!(classify_source("./my-plugin").is_local());
        assert!(classify_source("../plugins/foo").is_local());
    }

    #[test]
    fn test_classify_source_local_absolute() {
        assert!(classify_source("/home/user/plugins/foo").is_local());
    }

    #[test]
    fn test_classify_source_crate_name() {
        assert!(classify_source("norgolith-tree-sitter-highlight").is_crates_io());
    }

    #[test]
    fn test_classify_source_crate_with_version() {
        assert!(classify_source("norgolith-tree-sitter-highlight@0.1.0").is_crates_io());
    }

    #[test]
    fn test_git_install_from_local_fixture() {
        // Create a local bare repo with a minimal plugin
        let tmp = tempdir().unwrap();
        let bare_repo_dir = tmp.path().join("test-plugin.git");
        Repository::init_bare(&bare_repo_dir).unwrap();

        // Clone the bare repo, add plugin files, push
        let work_dir = tmp.path().join("work");
        let work_repo = Repository::clone(bare_repo_dir.to_str().unwrap(), &work_dir).unwrap();

        // Write plugin.toml
        let manifest = format!(
            r#"[plugin]
name = "test-local-git-plugin"
version = "0.1.0"
norgolith = ">={}"
abi = {}

[hooks]
pre_build = false
post_convert = false
post_render = true
post_build = false

[capabilities]
filesystem = "none"
network = false

timeout_ms = 5000
priority = 100
"#,
            env!("CARGO_PKG_VERSION"),
            CORE_ABI_VERSION
        );
        std::fs::write(work_dir.join("plugin.toml"), manifest).unwrap();

        // Stage and commit
        let mut index = work_repo.index().unwrap();
        index.add_path(std::path::Path::new("plugin.toml")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = work_repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("test", "test@test.com").unwrap();
        work_repo.commit(Some("HEAD"), &sig, &sig, "init plugin", &tree, &[]).unwrap();

        // Push to bare (origin remote was already added by clone)
        work_repo
            .find_remote("origin")
            .unwrap()
            .push(&["refs/heads/master"], None)
            .unwrap();

        // Now test install_from_git from the bare repo URL
        // We can't call install_from_git directly because it builds with cargo,
        // but we can test the clone + validation logic
        let clone_dir = tmp.path().join("cloned");
        let _cloned = Repository::clone(bare_repo_dir.to_str().unwrap(), &clone_dir).unwrap();
        let manifest_path = clone_dir.join("plugin.toml");
        assert!(manifest_path.is_file(), "plugin.toml should be cloned");

        let manifest = PluginManifest::load(&manifest_path).unwrap();
        assert_eq!(manifest.plugin.name, "test-local-git-plugin");
        assert!(manifest.validate_abi().is_ok());
        assert!(manifest.validate_semver().is_ok());
    }
}
