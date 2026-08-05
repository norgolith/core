use std::path::PathBuf;

use miette::{IntoDiagnostic, NamedSource, Report, Result, Severity, WrapErr, bail, miette};
use walkdir::WalkDir;

use crate::shared::{SitePaths, extract_metadata_from_content, validate_content_metadata};
use crate::{config, fs};

pub fn check() -> Result<()> {
    let Some(root) = fs::find_config_file()? else {
        bail!("{}: not in a Norgolith site directory", "Could not check the site");
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
        .ok_or_else(|| miette!("Config file path {} has no parent directory", root.display()))?
        .to_path_buf();
    let paths = SitePaths::new(root_dir.clone());

    let mut failures: Vec<(PathBuf, Report)> = Vec::new();
    let mut total = 0usize;
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
        let metadata = match extract_metadata_from_content(&content, rel_path, &site_config.root_url) {
            Ok(metadata) => metadata,
            Err(report) => {
                failures.push((path.to_path_buf(), report));
                continue;
            }
        };
        if let Err(report) = validate_content_metadata(&paths.content, path, &content, &metadata, schema, false)
            && count_errors(&report) > 0
        {
            failures.push((path.to_path_buf(), report));
        }
    }

    if failures.is_empty() {
        println!("All {} content file(s) passed schema validation", total);
        return Ok(());
    }

    let error_count: usize = failures.iter().map(|(_, r)| count_errors(r)).sum();
    for (_, report) in &failures {
        println!("{report:?}");
    }
    bail!(
        "Schema validation failed: {} file(s) failed ({} error(s) total)",
        failures.len(),
        error_count
    );
}

/// Counts Error-severity diagnostics; warnings (unknown_field, rule_condition) do not fail check.
// ponytail: warnings-only files validate to Err(report) but are discarded here since they never
// gate CI; surface them via the build output instead. The ValidationErrors container itself has
// no severity, so a report with related diagnostics counts only those; anything else is a real
// error (e.g. metadata parse failure) and counts as one.
fn count_errors(report: &Report) -> usize {
    match report.related() {
        Some(related) => related
            .filter(|d| matches!(d.severity(), None | Some(Severity::Error)))
            .count(),
        None => usize::from(matches!(report.severity(), None | Some(Severity::Error))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{ValidationError, ValidationErrors};
    use miette::NamedSource;

    #[test]
    fn count_errors_counts_only_error_severity() {
        let errors = vec![
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
        ];
        assert_eq!(count_errors(&Report::new(ValidationErrors(errors))), 1);
    }

    #[test]
    fn count_errors_zero_for_warning_only() {
        let report = Report::new(ValidationErrors(vec![ValidationError::UnknownField {
            field: "Version".into(),
            suggested: Some("version".into()),
            span: None,
        }]));
        assert_eq!(count_errors(&report), 0);
    }

    #[test]
    fn count_errors_handles_with_source_code_wrapper() {
        // validate_content_metadata attaches source code, which re-wraps the report;
        // related() must still surface the inner ValidationErrors.
        let report = Report::new(ValidationErrors(vec![ValidationError::TypeMismatch {
            field: "version".into(),
            expected: "number".into(),
            actual: "string".into(),
            span: None,
        }]))
        .with_source_code(NamedSource::new("test.norg", "version: one".to_string()));
        assert_eq!(count_errors(&report), 1);
    }
}
