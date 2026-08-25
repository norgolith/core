use std::path::PathBuf;
use std::process::exit;

use clap::ValueEnum;
use miette::{
    Diagnostic, IntoDiagnostic, NamedSource, Report, Result, Severity, WrapErr, bail, miette,
};
use regex::Regex;
use serde::Serialize;
use walkdir::WalkDir;

use crate::schema::ValidationError;
use crate::shared::{
    SitePaths, extract_metadata_from_content, validate_content_metadata_errors, validation_report,
};
use crate::{config, fs};

/// Output format for `lith check` diagnostics.
#[derive(ValueEnum, Clone, Copy, Default)]
pub enum CheckFormat {
    /// Human-readable miette rendering (default)
    #[default]
    Human,
    /// Structured JSON for tooling
    Json,
    /// GitHub Actions workflow commands (inline PR annotations)
    Github,
}

impl std::fmt::Display for CheckFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::Github => "github",
        };
        write!(f, "{s}")
    }
}

/// A content file that failed validation, kept structured so machine formats
/// can serialize it without parsing miette output.
struct FileFailure {
    path: PathBuf,
    content: String,
    kind: FailureKind,
}

enum FailureKind {
    Validation(Vec<ValidationError>),
    /// Metadata block could not be parsed at all.
    Extract(Report),
}

impl FileFailure {
    fn error_count(&self) -> usize {
        match &self.kind {
            FailureKind::Validation(errors) => errors
                .iter()
                .filter(|e| !matches!(e.severity(), Some(Severity::Warning)))
                .count(),
            FailureKind::Extract(_) => 1,
        }
    }
}

pub fn check(format: CheckFormat) -> Result<()> {
    let Some(root) = fs::find_config_file()? else {
        bail!(
            "{}: not in a Norgolith site directory",
            "Could not check the site"
        );
    };

    let config_content = std::fs::read_to_string(&root)
        .into_diagnostic()
        .wrap_err("Failed to read config file")?;
    let config_content_for_validation = config_content.clone();
    let site_config: config::SiteConfig = toml::from_str(&config_content).map_err(|e| {
        miette!("Failed to parse site configuration: {}", e)
            .with_source_code(NamedSource::new(root.display().to_string(), config_content))
    })?;
    let validation_errors = site_config.validate();
    if !validation_errors.is_empty() {
        return Err(miette!(
            "Site configuration has validation errors:\n{}",
            validation_errors.join("\n")
        )
        .with_source_code(NamedSource::new(
            root.display().to_string(),
            config_content_for_validation,
        )));
    }

    let Some(schema) = &site_config.content_schema else {
        println!("No content schema configured; nothing to check");
        return Ok(());
    };

    let root_dir = root
        .parent()
        .ok_or_else(|| {
            miette!(
                "Config file path {} has no parent directory",
                root.display()
            )
        })?
        .to_path_buf();
    let paths = SitePaths::new(root_dir.clone());

    let mut total = 0usize;
    let mut failures: Vec<FileFailure> = Vec::new();
    for entry in WalkDir::new(&paths.content)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "norg"))
    {
        let path = entry.path();
        let rel_path = path
            .strip_prefix(&paths.content)
            .into_diagnostic()
            .wrap_err("Failed to resolve content path")?;
        if rel_path.starts_with(&site_config.categories_dir) {
            continue;
        }
        total += 1;
        let content = std::fs::read_to_string(path).into_diagnostic()?;
        let metadata =
            match extract_metadata_from_content(&content, rel_path, &site_config.root_url) {
                Ok(metadata) => metadata,
                Err(report) => {
                    failures.push(FileFailure {
                        path: path.to_path_buf(),
                        content: content.clone(),
                        kind: FailureKind::Extract(report),
                    });
                    continue;
                }
            };
        match validate_content_metadata_errors(&paths.content, path, &metadata, schema, &content) {
            Ok(errors)
                if !errors.is_empty()
                    && errors
                        .iter()
                        .any(|e| !matches!(e.severity(), Some(Severity::Warning))) =>
            {
                failures.push(FileFailure {
                    path: path.to_path_buf(),
                    content,
                    kind: FailureKind::Validation(errors),
                });
            }
            Ok(_) => {}
            Err(report) => {
                failures.push(FileFailure {
                    path: path.to_path_buf(),
                    content,
                    kind: FailureKind::Extract(report),
                });
            }
        }
    }

    if failures.is_empty() {
        match format {
            CheckFormat::Human => {
                println!("All {total} content file(s) passed schema validation");
            }
            CheckFormat::Json => {
                let out = JsonOutput {
                    summary: Summary {
                        files: total,
                        errors: 0,
                        warnings: 0,
                    },
                    diagnostics: Vec::new(),
                };
                println!("{}", serde_json::to_string_pretty(&out).into_diagnostic()?);
            }
            CheckFormat::Github => {}
        }
        return Ok(());
    }

    let error_count: usize = failures.iter().map(FileFailure::error_count).sum();
    match format {
        CheckFormat::Human => {
            for failure in &failures {
                match &failure.kind {
                    FailureKind::Validation(errors) => {
                        println!(
                            "{:?}",
                            validation_report(&failure.path, &failure.content, errors)
                        );
                    }
                    FailureKind::Extract(report) => println!("{report:?}"),
                }
            }
            bail!(
                "Schema validation failed: {} file(s) failed ({} error(s) total)",
                failures.len(),
                error_count
            );
        }
        CheckFormat::Json => {
            let out = collect_json_output(&failures, total);
            println!("{}", serde_json::to_string_pretty(&out).into_diagnostic()?);
        }
        CheckFormat::Github => {
            for diag in collect_json_output(&failures, total).diagnostics {
                println!("{}", github_annotation(&diag));
            }
        }
    }
    // Machine-readable formats must not mix a human bail! report into stderr;
    // a plain non-zero exit is the CI signal.
    exit(1);
}

fn count_warnings(failures: &[FileFailure]) -> usize {
    failures
        .iter()
        .flat_map(|f| match &f.kind {
            FailureKind::Validation(errors) => errors.iter().collect::<Vec<_>>(),
            FailureKind::Extract(_) => Vec::new(),
        })
        .filter(|e| matches!(e.severity(), Some(Severity::Warning)))
        .count()
}

fn to_json_diagnostic(failure: &FileFailure, error: &ValidationError) -> JsonDiagnostic {
    let (line, column) = error.span().map_or((None, None), |span| {
        let (line, col) = line_col(&failure.content, span.offset());
        (Some(line), Some(col))
    });
    JsonDiagnostic {
        file: failure.path.display().to_string(),
        code: error.code().map(|c| c.to_string()),
        severity: match error.severity() {
            Some(Severity::Warning) => "warning",
            _ => "error",
        },
        message: strip_ansi(&error.to_string()),
        help: error.help().map(|h| strip_ansi(&h.to_string())),
        line,
        column,
    }
}

fn collect_json_output(failures: &[FileFailure], total: usize) -> JsonOutput {
    let mut diagnostics = Vec::new();
    for failure in failures {
        match &failure.kind {
            FailureKind::Validation(errors) => {
                for error in errors {
                    diagnostics.push(to_json_diagnostic(failure, error));
                }
            }
            FailureKind::Extract(report) => {
                diagnostics.push(JsonDiagnostic {
                    file: failure.path.display().to_string(),
                    code: None,
                    severity: "error",
                    message: strip_ansi(&report.to_string()),
                    help: report.help().map(|h| strip_ansi(&h.to_string())),
                    line: None,
                    column: None,
                });
            }
        }
    }
    JsonOutput {
        summary: Summary {
            files: total,
            errors: failures.iter().map(FileFailure::error_count).sum(),
            warnings: count_warnings(failures),
        },
        diagnostics,
    }
}

/// Renders a diagnostic as a GitHub Actions workflow command, e.g.
/// `::error file=content/x.norg,line=2,col=8::norgolith::schema::missing_field: ...`
/// so GitHub shows inline PR annotations without custom actions.
fn github_annotation(diag: &JsonDiagnostic) -> String {
    let props = [
        ("file", escape_property(&diag.file)),
        ("line", diag.line.map(|l| l.to_string()).unwrap_or_default()),
        (
            "col",
            diag.column.map(|c| c.to_string()).unwrap_or_default(),
        ),
    ]
    .into_iter()
    .filter(|(_, v)| !v.is_empty())
    .map(|(k, v)| format!("{k}={v}"))
    .collect::<Vec<_>>()
    .join(",");
    let code = diag.code.as_deref().unwrap_or("schema");
    format!(
        "::{} {}::{}: {}",
        diag.severity,
        props,
        code,
        escape_data(&diag.message)
    )
}

/// Escapes a GitHub workflow-command property value.
fn escape_property(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(':', "%3A")
        .replace(',', "%2C")
}

/// Escapes a GitHub workflow-command data segment.
fn escape_data(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Strips ANSI SGR sequences so machine-readable output stays clean even when
/// colored paints the Display impls of validation errors.
fn strip_ansi(s: &str) -> String {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").unwrap())
        .replace_all(s, "")
        .into_owned()
}

/// Converts a byte offset into a 1-based (line, column) pair, counting columns
/// in chars so multi-byte Norg text reports sane positions.
fn line_col(content: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(content.len());
    let line = content[..offset].bytes().filter(|b| *b == b'\n').count() + 1;
    let line_start = content[..offset].rfind('\n').map_or(0, |i| i + 1);
    let col = content[line_start..offset].chars().count() + 1;
    (line, col)
}

#[derive(Serialize)]
struct Summary {
    files: usize,
    errors: usize,
    warnings: usize,
}

#[derive(Serialize)]
struct JsonDiagnostic {
    file: String,
    code: Option<String>,
    severity: &'static str,
    message: String,
    help: Option<String>,
    line: Option<usize>,
    column: Option<usize>,
}

#[derive(Serialize)]
struct JsonOutput {
    summary: Summary,
    diagnostics: Vec<JsonDiagnostic>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::ValidationError;

    #[test]
    fn strip_ansi_removes_sgr_sequences() {
        assert_eq!(strip_ansi("\x1b[1mfoo\x1b[0m bar"), "foo bar");
        assert_eq!(strip_ansi("plain"), "plain");
    }

    #[test]
    fn line_col_counts_bytes_for_lines_and_chars_for_columns() {
        let content = "título\nlínea dos";
        // Offset of "dos": "título" is 7 bytes + '\n' + "línea" is 6 bytes + ' ' = 15
        assert_eq!(line_col(content, 15), (2, 7));
        assert_eq!(line_col(content, 0), (1, 1));
        assert_eq!(line_col("abc", 99), (1, 4));
    }

    #[test]
    fn github_annotation_escapes_specials() {
        let diag = JsonDiagnostic {
            file: "a,b.norg".into(),
            code: Some("norgolith::schema::missing_field".into()),
            severity: "error",
            message: "missing 'x'\nsecond line".into(),
            help: None,
            line: Some(3),
            column: Some(1),
        };
        let ann = github_annotation(&diag);
        assert!(ann.starts_with("::error file=a%2Cb.norg,line=3,col=1::"));
        assert!(ann.contains("%0A"));
        assert!(!ann.contains('\n'));
    }

    #[test]
    fn github_annotation_skips_missing_position() {
        let diag = JsonDiagnostic {
            file: "x.norg".into(),
            code: None,
            severity: "warning",
            message: "no span".into(),
            help: None,
            line: None,
            column: None,
        };
        let ann = github_annotation(&diag);
        assert!(ann.starts_with("::warning file=x.norg::"));
        assert!(!ann.contains("line="));
    }

    #[test]
    fn error_count_ignores_warnings() {
        let f = FileFailure {
            path: PathBuf::from("t.norg"),
            content: String::new(),
            kind: FailureKind::Validation(vec![
                ValidationError::UnknownField {
                    field: "Version".into(),
                    suggested: Some("version".into()),
                    span: None,
                },
                ValidationError::TypeMismatch {
                    field: "version".into(),
                    expected: "number".into(),
                    actual: "string".into(),
                    span: None,
                },
            ]),
        };
        assert_eq!(f.error_count(), 1);
    }
}
