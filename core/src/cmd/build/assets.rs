use std::path::Path;
use std::sync::OnceLock;

use colored::Colorize;
use miette::{IntoDiagnostic, Result, WrapErr, miette};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use tracing::{instrument, warn};
use walkdir::WalkDir;

#[instrument]
pub(super) fn should_minify_asset(src: &Path) -> bool {
    let file_stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let file_ext = src.extension().and_then(|s| s.to_str()).unwrap_or_default();
    !file_stem.ends_with(".min") && (file_ext == "js" || file_ext == "css")
}

fn minify_html_cfg() -> &'static minify_html::Cfg {
    static CFG: OnceLock<minify_html::Cfg> = OnceLock::new();
    CFG.get_or_init(|| minify_html::Cfg {
        minify_js: true,
        minify_css: true,
        ..minify_html::Cfg::default()
    })
}

#[instrument]
pub(super) fn minify_html_content(rendered: String) -> Result<String> {
    String::from_utf8(minify_html::minify(rendered.as_bytes(), minify_html_cfg()))
        .map_err(|e| miette!("{}: {}", "HTML minification failed".bold(), e))
}

#[instrument(skip(src_path, dest_path))]
fn minify_js_asset(src_path: &Path, dest_path: &Path) -> Result<()> {
    let content = std::fs::read(src_path).into_diagnostic()?;
    let mut minified = Vec::new();
    let session = minify_js::Session::new();
    minify_js::minify(
        &session,
        minify_js::TopLevelMode::Global,
        &content,
        &mut minified,
    )
    .map_err(|e| {
        miette!(
            "{}: {}",
            format!("JS minification failed for {}", src_path.display()).bold(),
            e
        )
    })?;
    std::fs::write(dest_path, minified)
        .into_diagnostic().wrap_err_with(|| format!("Failed to write minified JS to {}", dest_path.display()))?;
    Ok(())
}

#[instrument(skip(src_path, dest_path))]
fn minify_css_asset(src_path: &Path, dest_path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(src_path).into_diagnostic()?;

    let mut stylesheet =
        StyleSheet::parse(&content, ParserOptions::default()).map_err(|e| miette!("{}", e))?;
    stylesheet.minify(MinifyOptions::default()).into_diagnostic()?;
    let minified = stylesheet.to_css(PrinterOptions {
        minify: true,
        ..Default::default()
    }).into_diagnostic()?;

    std::fs::write(dest_path, minified.code).into_diagnostic().wrap_err_with(|| {
        format!("Failed to write minified CSS to {}", dest_path.display()).bold()
    })?;
    Ok(())
}

#[instrument(skip(src_path, dest_path))]
fn copy_binary_asset(src_path: &Path, dest_path: &Path) -> Result<()> {
    let content = std::fs::read(src_path).into_diagnostic()?;
    std::fs::write(dest_path, content).into_diagnostic().wrap_err_with(|| {
        format!(
            "Failed to copy asset from {} to {}",
            src_path.display(),
            dest_path.display()
        )
        .bold()
    })?;
    Ok(())
}

#[instrument(skip(src_path, dest_path, minify))]
fn copy_asset_file(src_path: &Path, dest_path: &Path, minify: bool) -> Result<()> {
    if minify && should_minify_asset(src_path) {
        let file_ext = src_path.extension().unwrap().to_str().unwrap();

        match file_ext {
            "js" => minify_js_asset(src_path, dest_path)?,
            "css" => minify_css_asset(src_path, dest_path)?,
            _ => copy_binary_asset(src_path, dest_path)?,
        }
    } else {
        copy_binary_asset(src_path, dest_path)?;
    }
    Ok(())
}

#[instrument(skip(assets_dir, target_dir, minify))]
pub(super) fn copy_assets(assets_dir: &Path, target_dir: &Path, minify: bool) -> Result<usize> {
    let mut file_count = 0usize;
    for entry in WalkDir::new(assets_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| match e {
            Ok(e) => Some(e),
            Err(e) => {
                warn!("WalkDir error: {}", e);
                None
            }
        })
    {
        let Some(rel_path) = entry.path().strip_prefix(assets_dir).ok() else {
            warn!(
                "Skipping asset outside assets directory: {}",
                entry.path().display()
            );
            continue;
        };
        let target_path = target_dir.join(rel_path);
        if entry.path().is_dir() {
            if !target_path.exists() {
                std::fs::create_dir_all(target_path).into_diagnostic()?;
            }
        } else {
            copy_asset_file(entry.path(), &target_path, minify)?;
            file_count += 1;
        }
    }

    Ok(file_count)
}
