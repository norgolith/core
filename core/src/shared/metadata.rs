use std::collections::{HashMap, HashSet};
use std::path::Path;

use miette::{NamedSource, Result, Severity, miette};
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::config::CollectionConfig;
use crate::converter;
use crate::schema::{ContentSchema, ValidationError, ValidationErrors, validate_metadata};

fn find_field_span(content: &str, field: &str) -> Option<miette::SourceSpan> {
    scan_meta_spans(content).get(field).copied()
}

/// Scans the `@document.meta` block of `content` and records the byte span of
/// every metadata field value, keyed by its dot/bracket path (matching the
/// paths produced by `FieldDefinition::validate`: `author.name`,
/// `categories[0]`, `authors[0].name`).
///
/// Mirrors the rust_norg metadata grammar: objects are `{ key: value }` and
/// arrays `[ value ]`, both newline-separated (no commas); keys and values
/// cannot contain `{}[]` or newlines; indentation is irrelevant. Fields with
/// empty (Nil) values are not recorded. Malformed input degrades gracefully:
/// the offending path is simply absent from the result.
fn scan_meta_spans(content: &str) -> HashMap<String, miette::SourceSpan> {
    let mut spans = HashMap::new();
    let Some(meta_start) = content.find("@document.meta") else {
        return spans;
    };
    let base = meta_start + "@document.meta".len();
    spans.insert(
        "@meta".to_string(),
        miette::SourceSpan::new(meta_start.into(), "@document.meta".len()),
    );
    let block = &content[base..];
    let Some(block_end_rel) = block.find("@end") else {
        return spans;
    };
    parse_object(&mut spans, &block[..block_end_rel], 0, base, "");
    spans
}

fn skip_ws(s: &str) -> usize {
    s.len() - s.trim_start_matches([' ', '\t', '\n', '\r']).len()
}

fn skip_hspaces(s: &str) -> usize {
    s.len() - s.trim_start_matches([' ', '\t']).len()
}

fn is_nil_at(text: &str, pos: usize) -> bool {
    text.as_bytes().get(pos).is_none_or(|c| {
        matches!(c, b'\n' | b'}' | b']')
    })
}

fn parse_object(
    spans: &mut HashMap<String, miette::SourceSpan>,
    text: &str,
    pos: usize,
    base: usize,
    path: &str,
) -> (miette::SourceSpan, usize) {
    let mut i = pos + 1; // skip '{'
    loop {
        i += skip_ws(&text[i..]);
        if i >= text.len() || text.as_bytes()[i] == b'}' {
            break;
        }
        let key_start = i;
        while i < text.len()
            && !matches!(
                text.as_bytes()[i],
                b':' | b'{' | b'}' | b'[' | b']' | b'\n'
            )
        {
            i += 1;
        }
        if i >= text.len() || text.as_bytes()[i] != b':' {
            break;
        }
        let key = text[key_start..i].trim();
        i += 1; // ':'
        i += skip_hspaces(&text[i..]);
        if is_nil_at(text, i) {
            continue; // empty value, nothing to point at
        }
        let child_path = if path.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", path, key)
        };
        let (_child_span, consumed) = parse_value_into(spans, text, i, base, &child_path);
        i += consumed;
    }
    let mut end = i;
    if end < text.len() && text.as_bytes()[end] == b'}' {
        end += 1;
    }
    let span = miette::SourceSpan::new((base + pos).into(), end - pos);
    spans.insert(path.to_string(), span);
    (span, end - pos)
}

fn parse_array(
    spans: &mut HashMap<String, miette::SourceSpan>,
    text: &str,
    pos: usize,
    base: usize,
    path: &str,
) -> (miette::SourceSpan, usize) {
    let mut i = pos + 1; // skip '['
    let mut index = 0usize;
    loop {
        i += skip_ws(&text[i..]);
        if i >= text.len() || text.as_bytes()[i] == b']' {
            break;
        }
        let elem_path = format!("{}[{}]", path, index);
        let (_elem_span, consumed) = parse_value_into(spans, text, i, base, &elem_path);
        i += consumed;
        index += 1;
    }
    let mut end = i;
    if end < text.len() && text.as_bytes()[end] == b']' {
        end += 1;
    }
    let span = miette::SourceSpan::new((base + pos).into(), end - pos);
    spans.insert(path.to_string(), span);
    (span, end - pos)
}

fn parse_value_into(
    spans: &mut HashMap<String, miette::SourceSpan>,
    text: &str,
    pos: usize,
    base: usize,
    path: &str,
) -> (miette::SourceSpan, usize) {
    let rest = &text[pos..];
    if rest.starts_with('{') {
        parse_object(spans, text, pos, base, path)
    } else if rest.starts_with('[') {
        parse_array(spans, text, pos, base, path)
    } else {
        let mut j = 0;
        while j < rest.len() {
            let c = rest.as_bytes()[j];
            if matches!(c, b'{' | b'}' | b'[' | b']' | b'\n') {
                break;
            }
            if c == b'\\' {
                j += 2;
                continue;
            }
            j += 1;
        }
        let raw = &text[pos..pos + j];
        let trimmed = raw.trim_end();
        let span = miette::SourceSpan::new((base + pos).into(), trimmed.len());
        spans.insert(path.to_string(), span);
        (span, j)
    }
}

fn enrich_spans(
    errors: Vec<ValidationError>,
    spans: &HashMap<String, miette::SourceSpan>,
) -> Vec<ValidationError> {
    errors
        .into_iter()
        .map(|error| match error {
            ValidationError::MissingField { field, .. } => {
                let span = spans.get("@meta").copied();
                ValidationError::MissingField { field, span }
            }
            ValidationError::TypeMismatch {
                field,
                expected,
                actual,
                ..
            } => {
                let span = spans.get(&field).copied();
                ValidationError::TypeMismatch {
                    field,
                    expected,
                    actual,
                    span,
                }
            }
            ValidationError::ConstraintViolation {
                field,
                message,
                ..
            } => {
                let span = spans.get(&field).copied();
                ValidationError::ConstraintViolation {
                    field,
                    message,
                    span,
                }
            }
            ValidationError::UnknownField {
                field,
                suggested,
                ..
            } => {
                let span = spans.get(&field).copied();
                ValidationError::UnknownField {
                    field,
                    suggested,
                    span,
                }
            }
            other => other,
        })
        .collect()
}

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
pub fn load_metadata_from_content(
    content: &str,
    rel_path: &Path,
    routes_url: &str,
) -> Result<toml::Value> {
    let (html, toc) = converter::html::convert(content, routes_url)
        .map_err(|e| miette!("Failed to convert {}: {}", rel_path.display(), e))?;
    let mut metadata = converter::meta::convert(content, Some(converter::html::toc_to_toml(&toc)))
        .map_err(|e| miette!("Failed to parse metadata for {}: {}", rel_path.display(), e))?;
    let permalink = compute_permalink(rel_path, routes_url);
    normalize_datetimes(&mut metadata);
    normalize_categories(&mut metadata);
    if let toml::Value::Table(ref mut table) = metadata {
        table.insert("raw".to_string(), toml::Value::String(html.to_string()));
        table.insert("permalink".to_string(), toml::Value::String(permalink));
        table.insert(
            "rel_path".to_string(),
            toml::Value::String(rel_path.to_string_lossy().to_string()),
        );
    }
    Ok(metadata)
}

pub fn extract_metadata_from_content(
    content: &str,
    rel_path: &Path,
    routes_url: &str,
) -> Result<toml::Value> {
    let mut metadata = converter::meta::convert(content, None)
        .map_err(|e| miette!("Failed to parse metadata for {}: {}", rel_path.display(), e))?;
    let permalink = compute_permalink(rel_path, routes_url);
    normalize_datetimes(&mut metadata);
    if let toml::Value::Table(ref mut table) = metadata {
        table.insert("permalink".to_string(), toml::Value::String(permalink));
    }
    Ok(metadata)
}

/// Validates content metadata against a schema.
///
/// This function validates the metadata of a content file against a provided schema.
/// If validation errors are found, they are logged in a user-friendly format.
///
/// # Arguments
/// * `content_dir` - The content directory.
/// * `path` - The path to the content file.
/// * `content` - The raw file content (for miette source snippets).
/// * `metadata` - The parsed metadata to validate.
/// * `schema` - The schema to validate the metadata against.
/// * `as_warnings` - Whether to format errors as warnings or errors.
pub fn validate_content_metadata(
    content_dir: &Path,
    path: &Path,
    content: &str,
    metadata: &toml::Value,
    schema: &ContentSchema,
    as_warnings: bool,
) -> Result<()> {
    let relative_path = path
        .strip_prefix(content_dir)
        .map_err(|e| miette!("Path {} is not under content_dir: {}", path.display(), e))?;

    let metadata_map = metadata
        .as_table()
        .ok_or_else(|| miette!("Metadata for {} is not a table", path.display()))?
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let content_path = relative_path
        .to_str()
        .ok_or_else(|| miette!("Non-UTF-8 path: {}", path.display()))?
        .replace('\\', "/")
        .trim_end_matches(".norg")
        .to_string();

    let schema_nodes = schema.resolve_path(&content_path);
    let merged_schema = ContentSchema::merge_hierarchy(&schema_nodes);

    let matched_path = content_path
        .split('/')
        .take(schema_nodes.len() - 1)
        .collect::<Vec<_>>()
        .join("/");

    let errors = validate_metadata(&metadata_map, &merged_schema);

    if !errors.is_empty() {
        let spans = scan_meta_spans(content);
        let errors = enrich_spans(errors, &spans);
        let source_name = if matched_path.is_empty() {
            path.display().to_string()
        } else {
            format!("{} (schema: '{}')", path.display(), matched_path)
        };
        let report = miette::Report::new(ValidationErrors(errors))
            .with_source_code(NamedSource::new(source_name, content.to_string()));
        if as_warnings {
            eprintln!("{:?}", report);
            return Ok(());
        }
        return Err(report);
    }
    Ok(())
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
                eprintln!("{:?}", miette!(
                    severity = Severity::Warning,
                    help = "Check directory permissions and ensure all content directories are readable",
                    "WalkDir error: {}", e
                ));
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
        .filter_map(|e| {
            let path = e.path().to_path_buf();
            let rel_path = path.strip_prefix(content_dir).ok()?.to_path_buf();
            Some((path, rel_path))
        })
        .collect();

    // Process metadata extraction
    let mut posts: Vec<toml::Value> = entries
        .into_par_iter()
        .map(|(path, rel_path)| -> Result<toml::Value> {
            let content = std::fs::read_to_string(&path).map_err(|_| {
                miette!("Failed to read {}: {}", rel_path.display(), path.display())
            })?;
            load_metadata_from_content(&content, &rel_path, routes_url)
        })
        .collect::<Result<Vec<_>>>()?;

    posts.sort_by(|a, b| {
        let a_date = a
            .get("created")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let b_date = b
            .get("created")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let parse_date =
            |s: &str| {
                chrono::DateTime::parse_from_rfc3339(s)
                .unwrap_or_else(|_| {
                    eprintln!("{:?}", miette!(
                        severity = Severity::Warning,
                        help = "Use RFC 3339 format for dates, e.g. 2024-01-15T12:00:00Z",
                        "Post has invalid 'created' date '{}', defaulting to epoch for sort",
                        s
                    ));
                    chrono::DateTime::from_timestamp(0, 0).unwrap().into()
                })
                .with_timezone(&chrono::Utc)
            };

        parse_date(b_date).cmp(&parse_date(a_date))
    });

    Ok(posts)
}

#[cfg(test)]
mod tests {
    use crate::schema::ValidationError;

    use super::{enrich_spans, find_field_span, scan_meta_spans};

    fn meta_doc(fields: &str) -> String {
        format!("some content\n@document.meta\n{}\n@end\nrest", fields)
    }

    #[test]
    fn finds_simple_field_value_span() {
        let content = meta_doc("title: Hello World\nauthor: Alice");
        let span = find_field_span(&content, "title").unwrap();
        let expected_start = content.find("Hello World").unwrap();
        assert_eq!(span.offset(), expected_start);
        assert_eq!(span.len(), "Hello World".len());
    }

    #[test]
    fn span_points_to_value_not_key() {
        let content = meta_doc("title: My Post");
        let span = find_field_span(&content, "title").unwrap();
        let expected = content.find("My Post").unwrap();
        assert_eq!(span.offset(), expected);
        assert_eq!(span.len(), "My Post".len());
    }

    #[test]
    fn finds_field_at_end_of_meta_block() {
        let content = meta_doc("title: My Post\nversion: 1.0");
        let span = find_field_span(&content, "version").unwrap();
        let expected = content.find("1.0").unwrap();
        assert_eq!(span.offset(), expected);
        assert_eq!(span.len(), 3);
    }

    #[test]
    fn missing_field_returns_none() {
        let content = meta_doc("title: My Post");
        assert!(find_field_span(&content, "author").is_none());
    }

    #[test]
    fn no_meta_block_returns_none() {
        let content = "just a plain norg file without metadata";
        assert!(find_field_span(&content, "title").is_none());
    }

    #[test]
    fn open_meta_block_without_end_returns_none() {
        let content = "@document.meta\ntitle: My Post\n";
        assert!(find_field_span(&content, "title").is_none());
    }

    #[test]
    fn does_not_match_field_name_inside_string_value() {
        let content = meta_doc("title: \"talks about author: X\"\nauthor: Alice");
        let span = find_field_span(&content, "author").unwrap();
        let expected = content.rfind("Alice").unwrap();
        assert_eq!(span.offset(), expected);
    }

    #[test]
    fn nested_field_dot_path_returns_none() {
        let content = meta_doc("author:\n  name: Alice");
        assert!(find_field_span(&content, "author.name").is_none());
    }

    #[test]
    fn empty_value_returns_none() {
        let content = meta_doc("title:\nauthor: Alice");
        assert!(find_field_span(&content, "title").is_none());
    }

    #[test]
    fn enrich_spans_sets_span_for_constraint_violation() {
        let content = meta_doc("title: This title is way too long");
        let errors = vec![ValidationError::ConstraintViolation {
            field: "title".into(),
            message: "Exceeds max length 5".into(),
            span: None,
        }];
        let enriched = enrich_spans(errors, &scan_meta_spans(&content));
        match &enriched[0] {
            ValidationError::ConstraintViolation { span, .. } => {
                let span = span.as_ref().unwrap();
                assert_eq!(span.offset(), content.find("This title").unwrap());
            }
            _ => panic!("expected constraint violation"),
        }
    }

    #[test]
    fn enrich_spans_sets_span_for_type_mismatch() {
        let content = meta_doc("version: 1.0");
        let errors = vec![ValidationError::TypeMismatch {
            field: "version".into(),
            expected: "string".into(),
            actual: "1.0".into(),
            span: None,
        }];
        let enriched = enrich_spans(errors, &scan_meta_spans(&content));
        match &enriched[0] {
            ValidationError::TypeMismatch { span, .. } => {
                let span = span.as_ref().unwrap();
                assert_eq!(span.offset(), content.find("1.0").unwrap());
            }
            _ => panic!("expected type mismatch"),
        }
    }

    #[test]
    fn enrich_spans_points_missing_field_at_meta_directive() {
        let errors = vec![ValidationError::MissingField {
            field: "author".into(),
            span: None,
        }];
        let content = meta_doc("title: My Post");
        let enriched = enrich_spans(errors, &scan_meta_spans(&content));
        match &enriched[0] {
            ValidationError::MissingField { field, span } => {
                assert_eq!(field, "author");
                let span = span.as_ref().unwrap();
                assert_eq!(span.offset(), content.find("@document.meta").unwrap());
            }
            _ => panic!("expected missing field"),
        }
    }

    #[test]
    fn enrich_spans_keeps_span_none_when_field_not_found() {
        let content = meta_doc("title: My Post");
        let errors = vec![ValidationError::ConstraintViolation {
            field: "author".into(),
            message: "Exceeds max length 5".into(),
            span: None,
        }];
        let enriched = enrich_spans(errors, &scan_meta_spans(&content));
        match &enriched[0] {
            ValidationError::ConstraintViolation { span, .. } => assert!(span.is_none()),
            _ => panic!("expected constraint violation"),
        }
    }

    #[test]
    fn miette_report_renders_source_label() {
        let content = "@document.meta\ntitle: A very long title here\n@end";
        let errors = enrich_spans(
            vec![ValidationError::ConstraintViolation {
                field: "title".into(),
                message: "Exceeds max length 12".into(),
                span: None,
            }],
            &scan_meta_spans(content),
        );
        let report = miette::Report::new(crate::schema::ValidationErrors(errors))
            .with_source_code(miette::NamedSource::new("test.norg", content.to_string()));
        let mut out = String::new();
        miette::GraphicalReportHandler::new()
            .render_report(&mut out, report.as_ref())
            .unwrap();
        assert!(
            out.contains("Exceeds max length 12"),
            "label text missing in rendered output:\n{}",
            out
        );
        assert!(
            out.contains("A very long title"),
            "source line missing in rendered output:\n{}",
            out
        );
    }

    // Nested object fields

    #[test]
    fn finds_nested_object_field_span() {
        let content = meta_doc("author: {\n  name: Alice\n  email: alice@example.com\n}");
        let span = find_field_span(&content, "author.name").unwrap();
        let expected = content.find("Alice").unwrap();
        assert_eq!(span.offset(), expected);
        assert_eq!(span.len(), "Alice".len());
    }

    #[test]
    fn finds_deeply_nested_object_field_span() {
        let content = meta_doc("seo: {\n  og: {\n    image: /img/hero.png\n  }\n}");
        let span = find_field_span(&content, "seo.og.image").unwrap();
        let expected = content.find("/img/hero.png").unwrap();
        assert_eq!(span.offset(), expected);
        assert_eq!(span.len(), "/img/hero.png".len());
    }

    #[test]
    fn finds_nested_field_in_inline_object() {
        let content = meta_doc("pricing: { usd: 29.99 }");
        let span = find_field_span(&content, "pricing.usd").unwrap();
        let expected = content.find("29.99").unwrap();
        assert_eq!(span.offset(), expected);
        assert_eq!(span.len(), 5);
    }

    #[test]
    fn comma_is_not_a_separator_in_objects() {
        // rust_norg grammar separates on newlines only: commas are string chars
        let content = meta_doc("pricing: { usd: 29.99, tier: premium }");
        let span = find_field_span(&content, "pricing.usd").unwrap();
        assert_eq!(span.len(), "29.99, tier: premium".len());
        assert!(find_field_span(&content, "pricing.tier").is_none());
    }

    #[test]
    fn sibling_key_not_swallowed_by_nested_object() {
        let content = meta_doc("author: {\n  name: Alice\n}\ntitle: A Post");
        let span = find_field_span(&content, "title").unwrap();
        let expected = content.find("A Post").unwrap();
        assert_eq!(span.offset(), expected);
        assert!(find_field_span(&content, "author.name").is_some());
    }

    #[test]
    fn indented_object_without_braces_is_not_nested() {
        // rust_norg parses `author:` as Nil here; `name` is a top-level sibling
        let content = meta_doc("author:\n  name: Alice");
        assert!(find_field_span(&content, "author.name").is_none());
        assert!(find_field_span(&content, "name").is_some());
    }

    // Array items

    #[test]
    fn finds_array_item_span() {
        let content = meta_doc("categories: [\n  docs\n  blog\n]");
        let span = find_field_span(&content, "categories[0]").unwrap();
        let expected = content.find("docs").unwrap();
        assert_eq!(span.offset(), expected);
        assert_eq!(span.len(), 4);

        let second = find_field_span(&content, "categories[1]").unwrap();
        let expected = content.find("blog").unwrap();
        assert_eq!(second.offset(), expected);
    }

    #[test]
    fn finds_array_item_in_inline_array() {
        let content = meta_doc("categories: [docs]");
        let span = find_field_span(&content, "categories[0]").unwrap();
        let expected = content.find("docs").unwrap();
        assert_eq!(span.offset(), expected);
        assert_eq!(span.len(), 4);
    }

    #[test]
    fn comma_is_not_a_separator_in_metadata() {
        // rust_norg grammar separates on newlines only: commas are string chars
        let content = meta_doc("categories: [a, b, c]");
        let span = find_field_span(&content, "categories[0]").unwrap();
        assert_eq!(span.len(), "a, b, c".len());
        assert!(find_field_span(&content, "categories[1]").is_none());
    }

    #[test]
    fn finds_nested_field_in_array_of_objects() {
        let content = meta_doc("authors: [\n  { name: Alice }\n  { name: Bob }\n]");
        let span = find_field_span(&content, "authors[0].name").unwrap();
        let expected = content.find("Alice").unwrap();
        assert_eq!(span.offset(), expected);

        let second = find_field_span(&content, "authors[1].name").unwrap();
        let expected = content.find("Bob").unwrap();
        assert_eq!(second.offset(), expected);
    }

    #[test]
    fn empty_array_and_object_have_no_children() {
        let content = meta_doc("categories: []\nmeta: {}");
        assert!(find_field_span(&content, "categories[0]").is_none());
        assert!(find_field_span(&content, "meta.foo").is_none());
    }

    #[test]
    fn array_out_of_bounds_returns_none() {
        let content = meta_doc("categories: [\n  docs\n]");
        assert!(find_field_span(&content, "categories[3]").is_none());
    }

    #[test]
    fn nested_array_inside_object() {
        let content = meta_doc("post: {\n  tags: [\n    rust\n    norg\n  ]\n}");
        let span = find_field_span(&content, "post.tags[1]").unwrap();
        let expected = content.find("norg").unwrap();
        assert_eq!(span.offset(), expected);
    }

    // Enrichment with nested paths

    #[test]
    fn enrich_spans_resolves_nested_path() {
        let content = meta_doc("author: {\n  name: X\n}");
        let errors = vec![ValidationError::TypeMismatch {
            field: "author.name".into(),
            expected: "string".into(),
            actual: "integer".into(),
            span: None,
        }];
        let enriched = enrich_spans(errors, &scan_meta_spans(&content));
        match &enriched[0] {
            ValidationError::TypeMismatch { span, .. } => {
                let span = span.as_ref().unwrap();
                assert_eq!(span.offset(), content.find("X").unwrap());
            }
            _ => panic!("expected type mismatch"),
        }
    }

    #[test]
    fn enrich_spans_resolves_array_item_path() {
        let content = meta_doc("categories: [\n  docs\n]");
        let errors = vec![ValidationError::ConstraintViolation {
            field: "categories[0]".into(),
            message: "Exceeds max length 2".into(),
            span: None,
        }];
        let enriched = enrich_spans(errors, &scan_meta_spans(&content));
        match &enriched[0] {
            ValidationError::ConstraintViolation { span, .. } => {
                let span = span.as_ref().unwrap();
                assert_eq!(span.offset(), content.find("docs").unwrap());
            }
            _ => panic!("expected constraint violation"),
        }
    }

    #[test]
    fn miette_report_renders_nested_source_label() {
        let content = "@document.meta\nauthor: {\n  name: Very Long Name Here\n}\n@end";
        let errors = enrich_spans(
            vec![ValidationError::ConstraintViolation {
                field: "author.name".into(),
                message: "Exceeds max length 12".into(),
                span: None,
            }],
            &scan_meta_spans(content),
        );
        let report = miette::Report::new(crate::schema::ValidationErrors(errors))
            .with_source_code(miette::NamedSource::new("test.norg", content.to_string()));
        let mut out = String::new();
        miette::GraphicalReportHandler::new()
            .render_report(&mut out, report.as_ref())
            .unwrap();
        assert!(
            out.contains("Very Long Name Here"),
            "nested source line missing in rendered output:\n{}",
            out
        );
    }
}
