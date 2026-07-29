use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use colored::Colorize;
use miette::{IntoDiagnostic, Result, Severity, WrapErr, miette};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_minifier::{Minifier, MinifierOptions};
use oxc_parser::Parser;
use oxc_span::SourceType;
use tracing::instrument;
use walkdir::WalkDir;



#[instrument]
pub(super) fn should_fingerprint_asset(src: &Path) -> bool {
    let file_ext = src.extension().and_then(|s| s.to_str()).unwrap_or_default();
    file_ext == "js" || file_ext == "css"
}

fn minify_html_cfg() -> &'static minify_html_onepass::Cfg {
    static CFG: OnceLock<minify_html_onepass::Cfg> = OnceLock::new();
    CFG.get_or_init(|| minify_html_onepass::Cfg {
        minify_js: true,
        minify_css: true,
    })
}

#[instrument]
pub(super) fn minify_html_content(mut rendered: String) -> Result<String> {
    let new_len = minify_html_onepass::in_place(
        unsafe { rendered.as_bytes_mut() },
        minify_html_cfg(),
    )
    .map_err(|e| {
        miette!(
            "{} at position {}: {:?}",
            "HTML minification failed".bold(),
            e.position,
            e.error_type
        )
    })?;
    rendered.truncate(new_len);
    Ok(rendered)
}

#[instrument(skip(src_path))]
fn minify_js_asset(src_path: &Path) -> Result<String> {
    let source = std::fs::read_to_string(src_path)
        .into_diagnostic().wrap_err("Failed to read JS asset")?;

    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, &source, SourceType::mjs()).parse();

    if !ret.diagnostics.is_empty() {
        for diag in &ret.diagnostics {
            eprintln!("{:?}", miette!(
                severity = Severity::Warning,
                help = "This may cause minification issues or runtime errors",
                "JS parse warning for {}: {:?}", src_path.display(), diag
            ));
        }
    }

    let mut program = ret.program;
    Minifier::new(MinifierOptions::default()).minify(&allocator, &mut program);

    let output = Codegen::new()
        .with_options(CodegenOptions::minify())
        .build(&program);
    Ok(output.code)
}

#[instrument(skip(src_path))]
fn minify_css_asset(src_path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(src_path).into_diagnostic().wrap_err("Failed to read CSS asset")?;

    let mut stylesheet =
        StyleSheet::parse(&content, ParserOptions::default())
            .map_err(|e| miette!("Failed to parse CSS: {}", e))?;
    stylesheet.minify(MinifyOptions::default()).into_diagnostic().wrap_err("Failed to minify CSS")?;
    let minified = stylesheet.to_css(PrinterOptions {
        minify: true,
        ..Default::default()
    }).into_diagnostic().wrap_err("Failed to serialize minified CSS")?;

    Ok(minified.code)
}

#[instrument(skip(src_path, dest_path))]
fn copy_binary_asset(src_path: &Path, dest_path: &Path) -> Result<()> {
    let content = std::fs::read(src_path).into_diagnostic().wrap_err("Failed to read asset file")?;
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



#[instrument(skip(assets_dir, target_dir, minify))]
pub(super) fn copy_assets(
    assets_dir: &Path,
    target_dir: &Path,
    minify: bool,
    fingerprint: bool,
) -> Result<(usize, HashMap<String, String>)> {
    let entries: Vec<_> = WalkDir::new(assets_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| match e {
            Ok(e) => Some(e),
            Err(e) => {
                eprintln!("{:?}", miette!(
                    severity = Severity::Warning,
                    help = "Check directory permissions",
                    "WalkDir error: {}", e
                ));
                None
            }
        })
        .collect();

    let mut dirs = Vec::new();
    let mut file_ops = Vec::new();
    for entry in &entries {
        let Some(rel_path) = entry.path().strip_prefix(assets_dir).ok() else {
            eprintln!("{:?}", miette!(
                severity = Severity::Warning,
                help = "Assets should be placed in the site's assets directory (default: 'assets/')",
                "Skipping asset outside assets directory: {}",
                entry.path().display()
            ));
            continue;
        };
        if entry.path().is_dir() {
            let target_path = target_dir.join(rel_path);
            if !target_path.exists() {
                dirs.push(target_path);
            }
        } else {
            file_ops.push((entry.path().to_path_buf(), rel_path.to_path_buf()));
        }
    }

    for dir in &dirs {
        std::fs::create_dir_all(dir)
            .into_diagnostic()
            .wrap_err("Failed to create asset directory")?;
    }

    let mut fingerprint_map = HashMap::new();
    let mut count = 0;

    for (src, rel_path) in &file_ops {
        if fingerprint && should_fingerprint_asset(src) {
            let content = if minify {
                let ext = src
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                match ext {
                    "js" => minify_js_asset(src)?,
                    "css" => minify_css_asset(src)?,
                    _ => std::fs::read_to_string(src)
                        .into_diagnostic()
                        .wrap_err("Failed to read asset")?,
                }
            } else {
                std::fs::read_to_string(src)
                    .into_diagnostic()
                    .wrap_err("Failed to read asset")?
            };

            let hash = blake3::hash(content.as_bytes()).to_hex()[..8].to_string();

            let orig_name = rel_path.to_string_lossy().to_string();
            let stem = rel_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let ext = rel_path
                .extension()
                .and_then(|s| s.to_str())
                .map(|e| format!(".{}", e))
                .unwrap_or_default();
            let fp_name = format!("{}.{}{}", stem, hash, ext);
            let fp_rel_path: PathBuf = rel_path.with_file_name(&fp_name);

            let target_fp = target_dir.join(&fp_rel_path);
            std::fs::write(&target_fp, content.as_bytes())
                .into_diagnostic()
                .wrap_err_with(|| format!("Failed to write asset to {}", target_fp.display()))?;

            fingerprint_map.insert(orig_name, fp_rel_path.to_string_lossy().to_string());
            count += 1;
        } else {
            copy_binary_asset(src, &target_dir.join(rel_path))?;
            count += 1;
        }
    }

    Ok((count, fingerprint_map))
}
