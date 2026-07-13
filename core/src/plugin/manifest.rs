use std::path::Path;

use miette::{Result, bail, miette};
use norgolith_plugin_sdk::{
    CORE_ABI_VERSION, HOOK_POST_BUILD, HOOK_POST_CONVERT, HOOK_POST_RENDER, HOOK_PRE_BUILD,
};
use serde::Deserialize;

/// Default hook timeout in milliseconds
const DEFAULT_TIMEOUT_MS: u64 = 10_000;

/// Parsed representation of a `plugin.toml` manifest
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMetadata,
    pub hooks: HookConfig,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Execution priority (lower runs first, default 100)
    #[serde(default = "default_priority")]
    pub priority: u32,
}

fn default_priority() -> u32 {
    100
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginMetadata {
    /// Name of the plugin (e.g. "my-plugin")
    pub name: String,
    /// Semver requirement for norgolith compatibility (e.g. ">=0.4.0")
    pub norgolith: String,
    /// ABI version this plugin was compiled against
    pub abi: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HookConfig {
    #[serde(default)]
    pub pre_build: bool,
    #[serde(default)]
    pub post_convert: bool,
    #[serde(default)]
    pub post_render: bool,
    #[serde(default)]
    pub post_build: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Capabilities {
    #[serde(default)]
    pub filesystem: FilesystemAccess,
    #[serde(default)]
    pub network: bool,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FilesystemAccess {
    #[default]
    None,
    Read,
    Write,
    #[serde(rename = "read-write")]
    ReadWrite,
}

fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

impl HookConfig {
    /// Returns a bitmask of declared hooks
    /// Bits: PRE_BUILD=1, POST_CONVERT=2, POST_RENDER=4, POST_BUILD=8
    pub fn to_mask(&self) -> u32 {
        let mut mask = 0u32;
        if self.pre_build {
            mask |= HOOK_PRE_BUILD;
        }
        if self.post_convert {
            mask |= HOOK_POST_CONVERT;
        }
        if self.post_render {
            mask |= HOOK_POST_RENDER;
        }
        if self.post_build {
            mask |= HOOK_POST_BUILD;
        }
        mask
    }

    pub fn declared_hooks(&self) -> Vec<&'static str> {
        let mut hooks = Vec::new();
        if self.pre_build {
            hooks.push("pre_build");
        }
        if self.post_convert {
            hooks.push("post_convert");
        }
        if self.post_render {
            hooks.push("post_render");
        }
        if self.post_build {
            hooks.push("post_build");
        }
        hooks
    }
}

impl PluginManifest {
    /// Parse a `plugin.toml` file at the given path
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| miette!("Failed to read {}: {}", path.display(), e))?;
        let manifest: PluginManifest = toml::from_str(&content)
            .map_err(|e| miette!("Failed to parse {}: {}", path.display(), e))?;
        Ok(manifest)
    }

    /// Validate ABI compatibility. Returns Ok(()) if compatible
    pub fn validate_abi(&self) -> Result<()> {
        if self.plugin.abi != CORE_ABI_VERSION {
            bail!(
                "ABI mismatch: plugin '{}' requires abi={}, core provides abi={}",
                self.plugin.name,
                self.plugin.abi,
                CORE_ABI_VERSION
            );
        }
        Ok(())
    }

    /// Validate semver compatibility with the running norgolith version
    pub fn validate_semver(&self) -> Result<()> {
        let req = semver::VersionReq::parse(&self.plugin.norgolith).map_err(|e| {
            miette!(
                "Invalid semver requirement '{}': {}",
                self.plugin.norgolith,
                e
            )
        })?;
        let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
            .map_err(|e| miette!("Invalid core version: {}", e))?;
        if !req.matches(&current) {
            bail!(
                "Version mismatch: plugin '{}' requires norgolith {}, installed is {}",
                self.plugin.name,
                self.plugin.norgolith,
                current
            );
        }
        Ok(())
    }
}
