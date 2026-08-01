use colored::Colorize;
use miette::{Diagnostic, Severity, miette};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod validator;

pub use validator::validate_metadata;

#[derive(Clone, Debug, Diagnostic)]
pub enum ValidationError {
    #[diagnostic(
        code("norgolith::schema::missing_field"),
        url("https://norgolith.dev/docs/content-schemas/#norgolith-schema-missing-field"),
        help("Add the missing field '{}' to your content metadata", field),
        severity(Error)
    )]
    MissingField {
        field: String,
        #[label("missing field '{}'", field)]
        span: Option<miette::SourceSpan>,
    },
    #[diagnostic(
        code("norgolith::schema::type_mismatch"),
        url("https://norgolith.dev/docs/content-schemas/#norgolith-schema-type-mismatch"),
        help("Change field '{}' to expected type {}", field, expected),
        severity(Error)
    )]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
        #[label("expected {}, got {}", expected, actual)]
        span: Option<miette::SourceSpan>,
    },
    #[diagnostic(
        code("norgolith::schema::constraint_violation"),
        url("https://norgolith.dev/docs/content-schemas/#norgolith-schema-constraint-violation"),
        help("{}", message),
        severity(Error)
    )]
    ConstraintViolation {
        field: String,
        message: String,
        #[label("{}", message)]
        span: Option<miette::SourceSpan>,
    },
    #[diagnostic(
        code("norgolith::schema::rule_condition"),
        url("https://norgolith.dev/docs/content-schemas/#norgolith-schema-rule-condition"),
        help("{}", message),
        severity(Warning)
    )]
    RuleConditionFailed { message: String },
    #[diagnostic(
        code("norgolith::schema::unknown_field"),
        url("https://norgolith.dev/docs/content-schemas/#norgolith-schema-unknown-field"),
        help("Field '{}' is not defined in the content schema{}", field, suggested.as_ref().map(|s| format!("; did you mean '{}'?", s)).unwrap_or_default()),
        severity(Warning)
    )]
    UnknownField {
        field: String,
        suggested: Option<String>,
        #[label("unknown field '{}'{}", field, suggested.as_ref().map(|s| format!(" (did you mean '{}'?)", s)).unwrap_or_default())]
        span: Option<miette::SourceSpan>,
    },
}

impl std::error::Error for ValidationError {}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField { field, .. } => {
                write!(f, "{} '{}'", "Missing field".bold(), field.bold())
            }
            Self::TypeMismatch {
                field,
                expected,
                actual,
                ..
            } => write!(
                f,
                "{} '{}': expected {}, got {}",
                "Type mismatch for field".bold(),
                field.bold(),
                expected.bold(),
                actual.bold()
            ),
            Self::ConstraintViolation { field, message, .. } => {
                write!(
                    f,
                    "{} '{}': {}",
                    "Constraint violation for field".bold(),
                    field.bold(),
                    message
                )
            }
            Self::RuleConditionFailed { message } => {
                write!(f, "{}: {}", "Rule condition failed".bold(), message)
            }
            Self::UnknownField { field, suggested, .. } => match suggested {
                Some(s) => write!(
                    f,
                    "{} '{}' (did you mean '{}'?)",
                    "Unknown field".bold(),
                    field.bold(),
                    s.bold()
                ),
                None => write!(f, "{} '{}'", "Unknown field".bold(), field.bold()),
            },
        }
    }
}

#[derive(Debug, Diagnostic)]
#[diagnostic(help("Fix the listed metadata field(s) and rebuild"))]
pub struct ValidationErrors(#[related] pub Vec<ValidationError>);

impl std::error::Error for ValidationErrors {}

impl std::fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Schema validation failed ({} errors)", self.0.len())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentSchema {
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub fields: HashMap<String, FieldDefinition>,
    #[serde(default, rename = "rules")] // Fix: handle TOML array format
    pub rules: Vec<ValidationRule>,
    #[serde(default, rename = "paths")]
    pub paths: HashMap<String, Box<ContentSchema>>,
}

// Struct to hold merged validation requirements
#[derive(Default, Debug)]
pub struct MergedSchema {
    pub required: Vec<String>,
    pub fields: HashMap<String, FieldDefinition>,
    pub rules: Vec<ValidationRule>,
}

impl ContentSchema {
    pub fn resolve_path<'a>(&'a self, content_path: &str) -> Vec<&'a ContentSchema> {
        let mut nodes = vec![self];
        let mut current = self;

        // Split path into components (e.g. "posts/2023" -> ["posts", "2023"])
        for component in content_path.split('/').filter(|s| !s.is_empty()) {
            if let Some(child) = current.paths.get(component) {
                nodes.push(child);
                current = child;
            } else if let Some(child) = current.paths.get("*") {
                nodes.push(child);
                current = child;
            } else if let Some(child) = current.paths.get("**") {
                nodes.push(child);
                break;
            } else {
                break;
            }
        }

        nodes
    }

    /// Merges schema hierarchy into final validation rules
    pub fn merge_hierarchy(nodes: &[&Self]) -> MergedSchema {
        // Only merge the hierarchy nodes in order (global -> specific)
        nodes.iter().fold(MergedSchema::default(), |mut acc, node| {
            // Merge required fields with deduplication
            let current_required = acc.required.clone();

            acc.required.extend(
                node.required
                    .iter()
                    .filter(|f| !current_required.contains(f))
                    .cloned(),
            );

            // Merge fields with later nodes overriding earlier ones
            for (k, v) in &node.fields {
                acc.fields.insert(k.clone(), v.clone());
            }

            // Merge rules while maintaining order
            acc.rules.extend(node.rules.iter().cloned());

            acc
        })
    }

    /// Validates config consistency: required fields must have matching field definitions
    pub fn validate_config(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for field in &self.required {
            if !self.fields.contains_key(field) {
                errors.push(format!(
                    "Required field '{}' has no matching [content_schema.fields.{}] section",
                    field, field
                ));
            }
        }
        errors
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FieldDefinition {
    String {
        min_length: Option<usize>,
        max_length: Option<usize>,
        pattern: Option<String>, // Regex patterns
    },
    Integer {
        min: Option<i64>,
        max: Option<i64>,
    },
    Float {
        min: Option<f64>,
        max: Option<f64>,
    },
    Array {
        items: Box<FieldDefinition>,
        min_items: Option<usize>,
        max_items: Option<usize>,
        must_contain: Option<Vec<toml::Value>>,
    },
    Boolean,
    Object {
        schema: HashMap<String, FieldDefinition>,
    },
}

impl FieldDefinition {
    pub fn validate(&self, value: &toml::Value, field_name: &str) -> Result<(), ValidationError> {
        match (self, value) {
            (
                FieldDefinition::String {
                    min_length,
                    max_length,
                    pattern,
                },
                toml::Value::String(s),
            ) => {
                if let Some(min) = min_length
                    && s.len() < *min
                {
                    return Err(ValidationError::ConstraintViolation {
                        field: field_name.to_string(),
                        message: format!("Below minimum length {}", min),
                        span: None,
                    });
                }
                if let Some(max) = max_length
                    && s.len() > *max
                {
                    return Err(ValidationError::ConstraintViolation {
                        field: field_name.to_string(),
                        message: format!("Exceeds max length {}", max),
                        span: None,
                    });
                }
                if let Some(pattern) = pattern {
                    let re = match Regex::new(pattern) {
                        Ok(r) => r,
                        Err(_) => {
                            return Err(ValidationError::ConstraintViolation {
                                field: field_name.to_string(),
                                message: format!("Invalid regex pattern: {}", pattern),
                                span: None,
                            });
                        }
                    };
                    if !re.is_match(s) {
                        return Err(ValidationError::ConstraintViolation {
                            field: field_name.to_string(),
                            message: format!("No pattern matching {}", pattern),
                            span: None,
                        });
                    }
                }
                Ok(())
            }
            (FieldDefinition::Integer { min, max }, toml::Value::Integer(n)) => {
                if let Some(min) = min && *n < *min {
                    return Err(ValidationError::ConstraintViolation {
                        field: field_name.to_string(),
                        message: format!("Below minimum value {}", min),
                        span: None,
                    });
                }
                if let Some(max) = max && *n > *max {
                    return Err(ValidationError::ConstraintViolation {
                        field: field_name.to_string(),
                        message: format!("Exceeds maximum value {}", max),
                        span: None,
                    });
                }
                Ok(())
            }
            (FieldDefinition::Float { min, max }, toml::Value::Float(n)) => {
                if let Some(min) = min && *n < *min {
                    return Err(ValidationError::ConstraintViolation {
                        field: field_name.to_string(),
                        message: format!("Below minimum value {}", min),
                        span: None,
                    });
                }
                if let Some(max) = max && *n > *max {
                    return Err(ValidationError::ConstraintViolation {
                        field: field_name.to_string(),
                        message: format!("Exceeds maximum value {}", max),
                        span: None,
                    });
                }
                Ok(())
            }
            (
                FieldDefinition::Array {
                    items,
                    min_items,
                    max_items,
                    must_contain,
                },
                toml::Value::Array(arr),
            ) => {
                if let Some(required_values) = must_contain {
                    for required in required_values {
                        if !arr.contains(required) {
                            return Err(ValidationError::ConstraintViolation {
                                field: field_name.to_string(),
                                message: format!("Missing value {}", required),
                                span: None,
                            });
                        }
                    }
                }
                if let Some(min) = min_items
                    && arr.len() < *min
                {
                    return Err(ValidationError::ConstraintViolation {
                        field: field_name.to_string(),
                        message: format!("Must contain at least {} value(s)", *min),
                        span: None,
                    });
                }
                if let Some(max) = max_items
                    && arr.len() > *max
                {
                    return Err(ValidationError::ConstraintViolation {
                        field: field_name.to_string(),
                        message: format!("Exceeds values limit (expected {} value(s))", *max),
                        span: None,
                    });
                }
                for (i, item) in arr.iter().enumerate() {
                    items.validate(item, &format!("{}[{}]", field_name, i))?;
                }
                Ok(())
            }
            (FieldDefinition::Boolean, value) => {
                if !value.is_bool() {
                    return Err(ValidationError::TypeMismatch {
                        field: field_name.to_string(),
                        expected: self.type_name(),
                        actual: value.to_string(),
                        span: None,
                    });
                }
                Ok(())
            }
            (FieldDefinition::Object { schema }, toml::Value::Table(table)) => {
                for (key, def) in schema {
                    if let Some(val) = table.get(key) {
                        def.validate(val, &format!("{}.{}", field_name, key))?;
                    }
                }
                Ok(())
            }
            _ => Err(ValidationError::TypeMismatch {
                field: field_name.to_string(),
                expected: self.type_name(),
                actual: value.to_string(),
                span: None,
            }),
        }
    }

    fn type_name(&self) -> String {
        match self {
            FieldDefinition::String { .. } => "string",
            FieldDefinition::Integer { .. } => "integer",
            FieldDefinition::Float { .. } => "float",
            FieldDefinition::Array { .. } => "array",
            FieldDefinition::Boolean => "boolean",
            FieldDefinition::Object { .. } => "object",
        }
        .to_string()
    }
}

/// Resolves a dot/bracket path like `author.team` or `authors[0].role` into the
/// nested metadata value. Plain keys resolve as before.
// ponytail: exact paths only: `key`, `key.sub`, `arr[0]`, `arr[0].sub`. No wildcards
// (match-any across array items is a separate feature), no escaping.
fn get_path<'a>(metadata: &'a HashMap<String, toml::Value>, path: &str) -> Option<&'a toml::Value> {
    let mut parts = path.split('.');
    let (key, idx) = split_index(parts.next()?);
    let mut value = metadata.get(key)?;
    if let Some(i) = idx {
        value = value.as_array()?.get(i)?;
    }
    for segment in parts {
        let (key, idx) = split_index(segment);
        value = match idx {
            Some(i) => value.as_table()?.get(key)?.as_array()?.get(i)?,
            None => value.as_table()?.get(key)?,
        };
    }
    Some(value)
}

fn split_index(segment: &str) -> (&str, Option<usize>) {
    match segment.find('[') {
        Some(i) => (
            &segment[..i],
            segment[i + 1..]
                .strip_suffix(']')
                .and_then(|s| s.parse().ok()),
        ),
        None => (segment, None),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationRule {
    #[serde(rename = "if")]
    pub condition: HashMap<String, toml::Value>,
    pub then: RuleAction,
}

impl ValidationRule {
    pub fn applies(
        &self,
        metadata: &HashMap<String, toml::Value>,
    ) -> Result<bool, ValidationError> {
        self.condition
            .iter()
            .try_fold(true, |acc, (field, expected)| match get_path(metadata, field) {
                Some(actual) => {
                    if actual.type_str() != expected.type_str() {
                        Err(ValidationError::RuleConditionFailed {
                            message: format!(
                                "Type mismatch in condition field '{}': expected {}, got {}",
                                field,
                                expected.type_str(),
                                actual.type_str()
                            ),
                        })
                    } else {
                        Ok(acc && actual == expected)
                    }
                }
                None => {
                    eprintln!(
                        "{:?}",
                        miette!(
                            severity = Severity::Warning,
                            help = "Check the 'conditional_on' field in content schema definition",
                            "Missing condition field '{}'",
                            field
                        )
                    );
                    Ok(false)
                }
            })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleAction {
    pub required: Option<Vec<String>>,
    pub fields: Option<HashMap<String, FieldDefinition>>,
}



#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn str_array(items: &[&str]) -> toml::Value {
        toml::Value::Array(
            items
                .iter()
                .map(|s| toml::Value::String(s.to_string()))
                .collect(),
        )
    }

    fn bare_schema(required: &[&str]) -> ContentSchema {
        ContentSchema {
            required: required.iter().map(|s| s.to_string()).collect(),
            fields: HashMap::new(),
            rules: Vec::new(),
            paths: HashMap::new(),
        }
    }

    // FieldDefinition::String

    #[test]
    fn string_valid() {
        let def = FieldDefinition::String {
            min_length: None,
            max_length: None,
            pattern: None,
        };
        assert!(
            def.validate(&toml::Value::String("hello".into()), "title")
                .is_ok()
        );
    }

    #[test]
    fn string_within_max_length_ok() {
        let def = FieldDefinition::String {
            min_length: None,
            max_length: Some(10),
            pattern: None,
        };
        assert!(
            def.validate(&toml::Value::String("hello".into()), "title")
                .is_ok()
        );
    }

    #[test]
    fn string_at_exact_max_length_ok() {
        let def = FieldDefinition::String {
            min_length: None,
            max_length: Some(5),
            pattern: None,
        };
        assert!(
            def.validate(&toml::Value::String("hello".into()), "title")
                .is_ok()
        );
    }

    #[test]
    fn string_exceeds_max_length() {
        let def = FieldDefinition::String {
            min_length: None,
            max_length: Some(3),
            pattern: None,
        };
        let err = def
            .validate(&toml::Value::String("hello".into()), "title")
            .unwrap_err();
        assert!(matches!(err, ValidationError::ConstraintViolation { .. }));
    }

    #[test]
    fn string_matching_pattern_ok() {
        let def = FieldDefinition::String {
            min_length: None,
            max_length: None,
            pattern: Some(r"^\d+$".into()),
        };
        assert!(
            def.validate(&toml::Value::String("1234".into()), "title")
                .is_ok()
        );
    }

    #[test]
    fn string_non_matching_pattern_errors() {
        let def = FieldDefinition::String {
            min_length: None,
            max_length: None,
            pattern: Some(r"^\d+$".into()),
        };
        let err = def
            .validate(&toml::Value::String("abc".into()), "title")
            .unwrap_err();
        assert!(matches!(err, ValidationError::ConstraintViolation { .. }));
    }

    #[test]
    fn string_wrong_type_errors() {
        let def = FieldDefinition::String {
            min_length: None,
            max_length: None,
            pattern: None,
        };
        let err = def
            .validate(&toml::Value::Boolean(true), "title")
            .unwrap_err();
        assert!(matches!(err, ValidationError::TypeMismatch { .. }));
    }

    // FieldDefinition::Boolean

    #[test]
    fn boolean_true_valid() {
        assert!(
            FieldDefinition::Boolean
                .validate(&toml::Value::Boolean(true), "draft")
                .is_ok()
        );
    }

    #[test]
    fn boolean_false_valid() {
        assert!(
            FieldDefinition::Boolean
                .validate(&toml::Value::Boolean(false), "draft")
                .is_ok()
        );
    }

    #[test]
    fn boolean_string_errors() {
        let err = FieldDefinition::Boolean
            .validate(&toml::Value::String("true".into()), "draft")
            .unwrap_err();
        assert!(matches!(err, ValidationError::TypeMismatch { .. }));
    }

    // FieldDefinition::Array

    #[test]
    fn array_valid_no_constraints() {
        let def = FieldDefinition::Array {
            items: Box::new(FieldDefinition::String {
            min_length: None,
            max_length: None,
                pattern: None,
            }),
            min_items: None,
            max_items: None,
            must_contain: None,
        };
        assert!(def.validate(&str_array(&["a", "b"]), "tags").is_ok());
    }

    #[test]
    fn array_min_items_exactly_satisfied() {
        let def = FieldDefinition::Array {
            items: Box::new(FieldDefinition::String {
            min_length: None,
            max_length: None,
                pattern: None,
            }),
            min_items: Some(2),
            max_items: None,
            must_contain: None,
        };
        assert!(def.validate(&str_array(&["a", "b"]), "tags").is_ok());
    }

    #[test]
    fn array_min_items_violated() {
        let def = FieldDefinition::Array {
            items: Box::new(FieldDefinition::String {
            min_length: None,
            max_length: None,
                pattern: None,
            }),
            min_items: Some(3),
            max_items: None,
            must_contain: None,
        };
        let err = def.validate(&str_array(&["a", "b"]), "tags").unwrap_err();
        assert!(matches!(err, ValidationError::ConstraintViolation { .. }));
    }

    #[test]
    fn array_max_items_exactly_satisfied() {
        let def = FieldDefinition::Array {
            items: Box::new(FieldDefinition::String {
            min_length: None,
            max_length: None,
                pattern: None,
            }),
            min_items: None,
            max_items: Some(3),
            must_contain: None,
        };
        assert!(def.validate(&str_array(&["a", "b"]), "tags").is_ok());
    }

    #[test]
    fn array_max_items_violated() {
        let def = FieldDefinition::Array {
            items: Box::new(FieldDefinition::String {
            min_length: None,
            max_length: None,
                pattern: None,
            }),
            min_items: None,
            max_items: Some(2),
            must_contain: None,
        };
        let err = def
            .validate(&str_array(&["a", "b", "c"]), "tags")
            .unwrap_err();
        assert!(matches!(err, ValidationError::ConstraintViolation { .. }));
    }

    #[test]
    fn array_must_contain_present_ok() {
        let def = FieldDefinition::Array {
            items: Box::new(FieldDefinition::String {
            min_length: None,
            max_length: None,
                pattern: None,
            }),
            min_items: None,
            max_items: None,
            must_contain: Some(vec![toml::Value::String("norgolith".into())]),
        };
        assert!(
            def.validate(&str_array(&["foo", "norgolith"]), "tags")
                .is_ok()
        );
    }

    #[test]
    fn array_must_contain_absent_errors() {
        let def = FieldDefinition::Array {
            items: Box::new(FieldDefinition::String {
            min_length: None,
            max_length: None,
                pattern: None,
            }),
            min_items: None,
            max_items: None,
            must_contain: Some(vec![toml::Value::String("norgolith".into())]),
        };
        let err = def
            .validate(&str_array(&["foo", "bar"]), "tags")
            .unwrap_err();
        assert!(matches!(err, ValidationError::ConstraintViolation { .. }));
    }

    #[test]
    fn array_wrong_type_errors() {
        let def = FieldDefinition::Array {
            items: Box::new(FieldDefinition::Boolean),
            min_items: None,
            max_items: None,
            must_contain: None,
        };
        let err = def
            .validate(&toml::Value::String("not an array".into()), "tags")
            .unwrap_err();
        assert!(matches!(err, ValidationError::TypeMismatch { .. }));
    }

    #[test]
    fn array_item_type_mismatch_errors() {
        let def = FieldDefinition::Array {
            items: Box::new(FieldDefinition::String {
            min_length: None,
            max_length: None,
                pattern: None,
            }),
            min_items: None,
            max_items: None,
            must_contain: None,
        };
        let arr = toml::Value::Array(vec![
            toml::Value::String("ok".into()),
            toml::Value::Integer(42),
        ]);
        let err = def.validate(&arr, "tags").unwrap_err();
        assert!(matches!(err, ValidationError::TypeMismatch { .. }));
    }

    #[test]
    fn array_item_string_pattern_validates() {
        let def = FieldDefinition::Array {
            items: Box::new(FieldDefinition::String {
            min_length: None,
            max_length: None,
                pattern: Some(r"^\d+$".into()),
            }),
            min_items: None,
            max_items: None,
            must_contain: None,
        };
        let ok = toml::Value::Array(vec![
            toml::Value::String("123".into()),
            toml::Value::String("456".into()),
        ]);
        assert!(def.validate(&ok, "tags").is_ok());

        let bad = toml::Value::Array(vec![
            toml::Value::String("123".into()),
            toml::Value::String("abc".into()),
        ]);
        let err = def.validate(&bad, "tags").unwrap_err();
        assert!(matches!(err, ValidationError::ConstraintViolation { .. }));
    }

    // ContentSchema::resolve_path

    #[test]
    fn resolve_path_root_only() {
        let schema = bare_schema(&["title"]);
        assert_eq!(schema.resolve_path("about").len(), 1);
    }

    #[test]
    fn resolve_path_single_child() {
        let mut schema = bare_schema(&["title"]);
        schema
            .paths
            .insert("posts".into(), Box::new(bare_schema(&["category"])));
        let nodes = schema.resolve_path("posts/my-post");
        assert_eq!(nodes.len(), 2);
        assert!(nodes[1].required.contains(&"category".to_string()));
    }

    #[test]
    fn resolve_path_unknown_component_stays_at_root() {
        let schema = bare_schema(&["title"]);
        assert_eq!(schema.resolve_path("nonexistent/deep").len(), 1);
    }

    #[test]
    fn resolve_path_partial_match_stops_at_last_known() {
        let mut schema = bare_schema(&["title"]);
        schema
            .paths
            .insert("posts".into(), Box::new(bare_schema(&["category"])));
        // "posts" matches, "2025" has no child entry under posts
        let nodes = schema.resolve_path("posts/2025/my-post");
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn resolve_path_star_matches_single_component() {
        let mut schema = bare_schema(&["title"]);
        schema
            .paths
            .insert("posts".into(), Box::new(bare_schema(&["category"])));
        schema
            .paths
            .get_mut("posts")
            .unwrap()
            .paths
            .insert("*".into(), Box::new(bare_schema(&["year"])));
        let nodes = schema.resolve_path("posts/2025/my-post");
        assert_eq!(nodes.len(), 3);
        assert!(nodes[2].required.contains(&"year".to_string()));
    }

    #[test]
    fn resolve_path_double_star_matches_any_depth() {
        let mut schema = bare_schema(&["title"]);
        schema
            .paths
            .insert("posts".into(), Box::new(bare_schema(&["category"])));
        schema
            .paths
            .get_mut("posts")
            .unwrap()
            .paths
            .insert("**".into(), Box::new(bare_schema(&["year"])));
        let nodes = schema.resolve_path("posts/2025/01/my-post");
        assert_eq!(nodes.len(), 3);
        assert!(nodes[2].required.contains(&"year".to_string()));
    }

    #[test]
    fn resolve_path_exact_takes_precedence_over_star() {
        let mut schema = bare_schema(&["title"]);
        schema
            .paths
            .insert("posts".into(), Box::new(bare_schema(&["category"])));
        schema
            .paths
            .get_mut("posts")
            .unwrap()
            .paths
            .insert("*".into(), Box::new(bare_schema(&["year"])));
        schema
            .paths
            .get_mut("posts")
            .unwrap()
            .paths
            .insert("2025".into(), Box::new(bare_schema(&["exact"])));
        let nodes = schema.resolve_path("posts/2025/my-post");
        assert_eq!(nodes.len(), 3);
        assert!(nodes[2].required.contains(&"exact".to_string()));
        assert!(!nodes[2].required.contains(&"year".to_string()));
    }

    #[test]
    fn resolve_path_star_skips_unknown_component() {
        let mut schema = bare_schema(&["title"]);
        schema
            .paths
            .insert("posts".into(), Box::new(bare_schema(&["category"])));
        schema
            .paths
            .get_mut("posts")
            .unwrap()
            .paths
            .insert("2025".into(), Box::new(bare_schema(&["year"])));
        // "2025" doesn't exist, so "*" would not match "2024/01"
        let nodes = schema.resolve_path("posts/nope/my-post");
        assert_eq!(nodes.len(), 2);
    }

    // ContentSchema::merge_hierarchy

    #[test]
    fn merge_single_node_identity() {
        let schema = bare_schema(&["title", "author"]);
        let merged = ContentSchema::merge_hierarchy(&[&schema]);
        assert_eq!(merged.required.len(), 2);
    }

    #[test]
    fn merge_deduplicates_required_fields() {
        let a = bare_schema(&["title", "author"]);
        let b = bare_schema(&["author", "created"]);
        let merged = ContentSchema::merge_hierarchy(&[&a, &b]);
        assert_eq!(merged.required.iter().filter(|f| *f == "author").count(), 1);
        assert_eq!(merged.required.len(), 3);
    }

    #[test]
    fn merge_later_field_definition_overrides_earlier() {
        let mut a = bare_schema(&[]);
        a.fields.insert(
            "title".into(),
            FieldDefinition::String {
            min_length: None,
            max_length: Some(50),
                pattern: None,
            },
        );
        let mut b = bare_schema(&[]);
        b.fields.insert(
            "title".into(),
            FieldDefinition::String {
            min_length: None,
            max_length: Some(120),
                pattern: None,
            },
        );
        let merged = ContentSchema::merge_hierarchy(&[&a, &b]);
        match merged.fields.get("title").unwrap() {
            FieldDefinition::String { max_length, .. } => assert_eq!(*max_length, Some(120)),
            _ => panic!("unexpected field definition type"),
        }
    }

    #[test]
    fn merge_rules_accumulate_in_order() {
        let mut a = bare_schema(&[]);
        a.rules.push(ValidationRule {
            condition: HashMap::from([("draft".into(), toml::Value::Boolean(false))]),
            then: RuleAction {
                required: Some(vec!["publish_date".into()]),
                fields: None,
            },
        });
        let mut b = bare_schema(&[]);
        b.rules.push(ValidationRule {
            condition: HashMap::from([("featured".into(), toml::Value::Boolean(true))]),
            then: RuleAction {
                required: Some(vec!["hero_image".into()]),
                fields: None,
            },
        });
        let merged = ContentSchema::merge_hierarchy(&[&a, &b]);
        assert_eq!(merged.rules.len(), 2);
    }

    // ValidationRule::applies

    fn draft_rule() -> ValidationRule {
        ValidationRule {
            condition: HashMap::from([("draft".into(), toml::Value::Boolean(false))]),
            then: RuleAction {
                required: Some(vec!["publish_date".into()]),
                fields: None,
            },
        }
    }

    #[test]
    fn rule_applies_when_condition_matches() {
        let meta = HashMap::from([("draft".into(), toml::Value::Boolean(false))]);
        assert!(matches!(draft_rule().applies(&meta), Ok(true)));
    }

    #[test]
    fn rule_does_not_apply_when_value_differs() {
        let meta = HashMap::from([("draft".into(), toml::Value::Boolean(true))]);
        assert!(matches!(draft_rule().applies(&meta), Ok(false)));
    }

    #[test]
    fn rule_skips_when_condition_field_missing() {
        assert!(matches!(draft_rule().applies(&HashMap::new()), Ok(false)));
    }

    #[test]
    fn rule_errors_on_type_mismatch_in_condition() {
        let meta = HashMap::from([("draft".into(), toml::Value::String("false".into()))]);
        assert!(matches!(
            draft_rule().applies(&meta),
            Err(ValidationError::RuleConditionFailed { .. })
        ));
    }

    #[test]
    fn nested_condition_paths() {
        let rule = ValidationRule {
            condition: HashMap::from([("author.team".into(), toml::Value::String("core".into()))]),
            then: RuleAction {
                required: None,
                fields: None,
            },
        };
        let mut team = toml::map::Map::new();
        team.insert("team".into(), toml::Value::String("core".into()));
        let nested = HashMap::from([("author".into(), toml::Value::Table(team))]);
        assert!(matches!(rule.applies(&nested), Ok(true)));
        assert!(matches!(rule.applies(&HashMap::new()), Ok(false)));

        let mut author = toml::map::Map::new();
        author.insert("role".into(), toml::Value::String("maintainer".into()));
        let array =
            HashMap::from([("authors".into(), toml::Value::Array(vec![toml::Value::Table(author)]))]);
        let array_rule = ValidationRule {
            condition: HashMap::from([(
                "authors[0].role".into(),
                toml::Value::String("maintainer".into()),
            )]),
            then: RuleAction {
                required: None,
                fields: None,
            },
        };
        assert!(matches!(array_rule.applies(&array), Ok(true)));
    }

    // FieldDefinition::Object

    fn flat_object_schema() -> FieldDefinition {
        FieldDefinition::Object {
            schema: HashMap::from([
                (
                    "name".into(),
                    FieldDefinition::String {
                        min_length: None,
                        max_length: None,
                        pattern: None,
                    },
                ),
                (
                    "email".into(),
                    FieldDefinition::String {
                        min_length: None,
                        max_length: None,
                        pattern: None,
                    },
                ),
            ]),
        }
    }

    #[test]
    fn object_valid_flat() {
        let table = toml::Value::Table(
            [("name".into(), toml::Value::String("Alice".into())),
                ("email".into(), toml::Value::String("alice@example.com".into())),
            ].into_iter().collect(),
        );
        assert!(flat_object_schema().validate(&table, "author").is_ok());
    }

    #[test]
    fn object_partial_fields_ok() {
        let table = toml::Value::Table(
            [("name".into(), toml::Value::String("Alice".into()))]
                .into_iter()
                .collect(),
        );
        assert!(flat_object_schema().validate(&table, "author").is_ok());
    }

    #[test]
    fn object_empty_ok() {
        let table = toml::Value::Table(toml::map::Map::new());
        assert!(flat_object_schema().validate(&table, "author").is_ok());
    }

    #[test]
    fn object_child_type_mismatch() {
        let table = toml::Value::Table(
            [("name".into(), toml::Value::Integer(42))]
                .into_iter()
                .collect(),
        );
        let err = flat_object_schema()
            .validate(&table, "author")
            .unwrap_err();
        assert!(matches!(err, ValidationError::TypeMismatch { .. }));
    }

    #[test]
    fn object_wrong_type_errors() {
        let err = flat_object_schema()
            .validate(&toml::Value::String("not an object".into()), "author")
            .unwrap_err();
        assert!(matches!(err, ValidationError::TypeMismatch { .. }));
    }

    // FieldDefinition::Integer

    #[test]
    fn integer_valid() {
        let def = FieldDefinition::Integer {
            min: None,
            max: None,
        };
        assert!(def.validate(&toml::Value::Integer(42), "count").is_ok());
    }

    #[test]
    fn integer_within_range_ok() {
        let def = FieldDefinition::Integer {
            min: Some(1),
            max: Some(100),
        };
        assert!(def.validate(&toml::Value::Integer(50), "count").is_ok());
    }

    #[test]
    fn integer_below_min() {
        let def = FieldDefinition::Integer {
            min: Some(10),
            max: None,
        };
        let err = def
            .validate(&toml::Value::Integer(5), "count")
            .unwrap_err();
        assert!(matches!(err, ValidationError::ConstraintViolation { .. }));
    }

    #[test]
    fn integer_above_max() {
        let def = FieldDefinition::Integer {
            min: None,
            max: Some(10),
        };
        let err = def
            .validate(&toml::Value::Integer(42), "count")
            .unwrap_err();
        assert!(matches!(err, ValidationError::ConstraintViolation { .. }));
    }

    #[test]
    fn integer_wrong_type_errors() {
        let def = FieldDefinition::Integer {
            min: None,
            max: None,
        };
        let err = def
            .validate(&toml::Value::String("42".into()), "count")
            .unwrap_err();
        assert!(matches!(err, ValidationError::TypeMismatch { .. }));
    }

    // FieldDefinition::Float

    #[test]
    fn float_valid() {
        let def = FieldDefinition::Float {
            min: None,
            max: None,
        };
        assert!(def.validate(&toml::Value::Float(std::f64::consts::PI), "pi").is_ok());
    }

    #[test]
    fn float_within_range_ok() {
        let def = FieldDefinition::Float {
            min: Some(0.0),
            max: Some(1.0),
        };
        assert!(def.validate(&toml::Value::Float(0.5), "pct").is_ok());
    }

    #[test]
    fn float_below_min() {
        let def = FieldDefinition::Float {
            min: Some(0.0),
            max: None,
        };
        let err = def
            .validate(&toml::Value::Float(-1.0), "pct")
            .unwrap_err();
        assert!(matches!(err, ValidationError::ConstraintViolation { .. }));
    }

    #[test]
    fn float_above_max() {
        let def = FieldDefinition::Float {
            min: None,
            max: Some(1.0),
        };
        let err = def
            .validate(&toml::Value::Float(42.0), "pct")
            .unwrap_err();
        assert!(matches!(err, ValidationError::ConstraintViolation { .. }));
    }

    #[test]
    fn float_wrong_type_errors() {
        let def = FieldDefinition::Float {
            min: None,
            max: None,
        };
        let err = def
            .validate(&toml::Value::String("3.14".into()), "pi")
            .unwrap_err();
        assert!(matches!(err, ValidationError::TypeMismatch { .. }));
    }

    // ContentSchema serde round-trip

    #[test]
    fn content_schema_toml_roundtrip() {
        let toml_str = r#"
required = ["title", "author"]

[fields.title]
type = "string"
max_length = 120

[fields.count]
type = "integer"
min = 0

[fields.ratio]
type = "float"
min = 0.0
max = 1.0

[fields.draft]
type = "boolean"

[fields.author]
type = "object"
schema = { name = { type = "string" }, email = { type = "string" } }
"#;
        let schema: ContentSchema = toml::from_str(toml_str).unwrap();
        let restored: ContentSchema = toml::from_str(&toml::to_string(&schema).unwrap()).unwrap();

        assert_eq!(schema.required, restored.required);
        assert_eq!(schema.fields.len(), restored.fields.len());
    }
}
