use std::{io, path::{Path, PathBuf}};

use clap::Subcommand;
use colored::Colorize;
use miette::{IntoDiagnostic, Result, WrapErr, bail, miette};
use flate2::read::GzDecoder;
use git2::Repository;
use serde::Deserialize;
use tar::Archive;
use tempfile::tempdir;

use crate::plugin::{self, CORE_ABI_VERSION, PluginManifest};

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
        PluginCommands::Install {
            source,
            git,
            tag,
            branch,
        } => install(source, *git, tag.as_deref(), branch.as_deref()),
        PluginCommands::Uninstall { name } => uninstall_plugin(name),
    }
}

fn list_plugins() -> Result<()> {
    let cwd = std::env::current_dir().into_diagnostic().wrap_err("Failed to determine current directory")?;
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
        println!("   version:  {:<10}  hooks:      {}", p.version, hooks_str);
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

    let cwd = std::env::current_dir().into_diagnostic().wrap_err("Failed to determine current directory")?;
    let plugins_dir = cwd.join("plugins").join(name);

    if plugins_dir.exists() {
        bail!(
            "Plugin '{}' already exists at {}",
            name,
            plugins_dir.display()
        );
    }

    std::fs::create_dir_all(plugins_dir.join("src")).into_diagnostic().wrap_err("Failed to create plugin source directory")?;

    // NOTE: I still need to handle the case where the plugin requires a dev norgolith version, e.g. (>=0.4.0-COMMIT_HASH)
    // For now, just use the current version of norgolith from Cargo.toml. I'll need to figure out if semver crate can
    // handle this case, or if I need to implement a custom version comparison for dev versions.
    const NORGOLITH_VERSION: &str = env!("CARGO_PKG_VERSION");

    // plugin.toml
    let manifest = format!(
        r#"[plugin]
name = "{name}"
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
    std::fs::write(plugins_dir.join("plugin.toml"), manifest).into_diagnostic().wrap_err("Failed to write plugin.toml")?;

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
    std::fs::write(plugins_dir.join("Cargo.toml"), cargo_toml).into_diagnostic().wrap_err("Failed to write Cargo.toml")?;

    // src/lib.rs
    let lib_rs = format!(
        r#"use norgolith_plugin_sdk::*;

register_plugin!("{name}")
    .on_post_render(|ctx| {{
        // Access per-plugin config from norgolith.toml
        // if let Some(theme) = ctx.config.as_ref().and_then(|c| c.get("theme")) {{
        //     plugin_log!("info", "config theme: {{theme}}");
        // }}
        Ok(Some(ctx.html))
    }})
    .register();
"#
    );
    std::fs::write(plugins_dir.join("src").join("lib.rs"), lib_rs).into_diagnostic().wrap_err("Failed to write plugin source file")?;

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
    let output = std::process::Command::new("cargo")
        .arg("build")
        .arg("--release")
        .current_dir(source_dir)
        .output()
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                miette::miette!("cargo not found in PATH. Install Rust from https://rustup.rs")
            } else {
                miette::miette!("Failed to run cargo: {e}")
            }
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Plugin cargo build failed:\n{}", stderr.trim());
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
        .ok_or_else(|| miette::miette!("Built library not found in {}", target_dir.display()))
}

fn install_to_plugins(lib_path: &Path, manifest_path: &Path, name: &str) -> Result<()> {
    let cwd = std::env::current_dir().into_diagnostic().wrap_err("Failed to determine current directory")?;
    let dest_dir = cwd.join("plugins").join(name);
    std::fs::create_dir_all(&dest_dir).into_diagnostic().wrap_err("Failed to create plugin install directory")?;
    let filename = lib_path.file_name().ok_or_else(|| miette!("Plugin library path has no filename: {}", lib_path.display()))?;
    std::fs::copy(lib_path, dest_dir.join(filename)).into_diagnostic().wrap_err("Failed to copy plugin library")?;
    std::fs::copy(manifest_path, dest_dir.join("plugin.toml")).into_diagnostic().wrap_err("Failed to copy plugin manifest")?;
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
    let tmp = tempdir().into_diagnostic().wrap_err("Failed to create temporary directory for cloning")?;
    let clone_path = tmp.path();

    println!("{}", "Cloning repository...".dimmed());
    Repository::clone(url, clone_path)
        .map_err(|e| miette::miette!("Failed to clone {}: {}", url, e))?;

    // Checkout tag or branch if specified
    if let Some(tag_name) = tag {
        let repo = Repository::open(clone_path).into_diagnostic().wrap_err("Failed to open cloned repository")?;
        let obj = repo.revparse_single(tag_name).into_diagnostic().wrap_err("Failed to resolve tag reference")?;
        repo.checkout_tree(&obj, None).into_diagnostic().wrap_err("Failed to checkout tag")?;
    } else if let Some(branch_name) = branch {
        let repo = Repository::open(clone_path).into_diagnostic().wrap_err("Failed to open cloned repository")?;
        let reference = repo.find_branch(branch_name, git2::BranchType::Local).into_diagnostic().wrap_err("Failed to find branch")?;
        repo.checkout_tree(reference.get().peel_to_tree().into_diagnostic().wrap_err("Failed to resolve branch tree")?.as_object(), None).into_diagnostic().wrap_err("Failed to checkout branch")?;
    }

    let manifest_path = clone_path.join("plugin.toml");
    if !manifest_path.is_file() {
        bail!("No plugin.toml found in {} (not a norgolith plugin?)", url);
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
        "Plugin '{}' installed from source",
        manifest.plugin.name.bold()
    );
    Ok(())
}

fn install_from_local(source_dir: &Path) -> Result<()> {
    if !source_dir.is_dir() {
        bail!("Not a directory: {}", source_dir.display());
    }

    let manifest_path = source_dir.join("plugin.toml");
    if !manifest_path.is_file() {
        bail!("No plugin.toml found in {}", source_dir.display());
    }

    let manifest = PluginManifest::load(&manifest_path)?;
    validate_plugin_name(&manifest.plugin.name)?;
    manifest.validate_abi()?;
    manifest.validate_semver()?;

    build_plugin(source_dir)?;
    let lib_path = find_built_library(source_dir, &manifest.plugin.name)?;
    install_to_plugins(&lib_path, &manifest_path, &manifest.plugin.name)?;

    println!(
        "Plugin '{}' installed from source",
        manifest.plugin.name.bold()
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
                .map_err(|e| miette::miette!("Failed to fetch crate info: {}", e))?
                .body_mut()
                .read_json()
                .map_err(|e| miette::miette!("Failed to parse crate info: {}", e))?;
            resp.krate
                .max_stable_version
                .ok_or_else(|| miette::miette!("No versions found for crate '{}'", name))?
        }
    };

    let tmp = tempdir().into_diagnostic().wrap_err("Failed to create temporary directory")?;
    let dl_path = tmp.path().join("plugin.crate");

    println!("{}", "Downloading crate...".dimmed());
    let url = format!(
        "https://crates.io/api/v1/crates/{}/{}/download",
        name, version
    );
    let mut resp = ureq::get(&url)
        .header("User-Agent", "norgolith (https://github.com/norgolith)")
        .call()
        .map_err(|e| miette::miette!("Failed to download crate: {}", e))?;

    let body = resp
        .body_mut()
        .read_to_vec()
        .map_err(|e| miette::miette!("Failed to read response: {}", e))?;
    std::fs::write(&dl_path, &body).into_diagnostic().wrap_err("Failed to write downloaded crate to disk")?;

    println!("{}", "Extracting crate...".dimmed());
    let tar_gz = std::fs::File::open(&dl_path).into_diagnostic().wrap_err("Failed to open downloaded crate file")?;
    let decoder = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(tmp.path())
        .map_err(|e| miette::miette!("Failed to extract crate: {}", e))?;

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
        version
    );
    Ok(())
}


fn uninstall_plugin(name: &str) -> Result<()> {
    validate_plugin_name(name)?;

    let cwd = std::env::current_dir().into_diagnostic().wrap_err("Failed to determine current directory")?;
    let plugin_dir = cwd.join("plugins").join(name);

    if !plugin_dir.is_dir() {
        bail!("Plugin '{}' is not installed", name);
    }

    std::fs::remove_dir_all(&plugin_dir).into_diagnostic().wrap_err("Failed to remove plugin directory")?;
    println!("Plugin '{}' uninstalled", name.bold());
    Ok(())
}

fn validate_plugin_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Plugin name cannot be empty");
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.contains(':') {
        bail!(
            "Invalid plugin name: '{}' (no path separators or '..' allowed)",
            name
        );
    }
    Ok(())
}


