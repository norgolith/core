use std::path::Path;

use colored::Colorize;
use eyre::eyre;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use tera::{Error, Function, Kwargs, State, Tera, Value};

// ponytail: RFC 3986 unreserved characters — keeps id anchors clean
const UNRESERVED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

#[derive(Debug, Clone)]
struct TocNode {
    level: u8,
    title: String,
    id: String,
    children: Vec<usize>,
}

#[derive(Debug)]
struct TocTree {
    nodes: Vec<TocNode>,
    root_indices: Vec<usize>,
}

fn parse_toc(value: &Value) -> std::result::Result<TocTree, tera::Error> {
    let entries = value
        .as_array()
        .ok_or_else(|| Error::msg("TOC must be an array"))?;
    let mut tree = TocTree {
        nodes: Vec::new(),
        root_indices: Vec::new(),
    };
    let mut stack: Vec<usize> = Vec::new();

    for entry in entries {
        let level = entry
            .get("level")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| Error::msg("Missing or invalid level"))? as u8;

        let title = entry
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let id = entry
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        // Find the parent index
        let parent_idx = stack
            .iter()
            .rev()
            .find(|&&idx| tree.nodes[idx].level < level)
            .copied();

        // Create new node
        let node_idx = tree.nodes.len();
        tree.nodes.push(TocNode {
            level,
            title,
            id,
            children: Vec::new(),
        });

        // Add to parent or root
        if let Some(parent_idx) = parent_idx {
            tree.nodes[parent_idx].children.push(node_idx);
        } else {
            tree.root_indices.push(node_idx);
        }

        // Update stack
        while stack
            .last()
            .map(|&idx| tree.nodes[idx].level >= level)
            .unwrap_or(false)
        {
            stack.pop();
        }
        stack.push(node_idx);
    }

    Ok(tree)
}

fn generate_nested_html(tree: &TocTree, list_type: &str) -> String {
    fn render_node(tree: &TocTree, node_idx: usize, list_type: &str) -> String {
        let node = &tree.nodes[node_idx];

        let mut html = format!(
            "<li><a href=\"#{}\">{}</a>",
            utf8_percent_encode(&node.id, UNRESERVED),
            tera::escape_html(&node.title)
        );

        if !node.children.is_empty() {
            html.push_str(&format!("<{}>", list_type));
            for &child_idx in &node.children {
                html.push_str(&render_node(tree, child_idx, list_type));
            }
            html.push_str(&format!("</{}>", list_type));
        }

        html.push_str("</li>");
        html
    }

    let mut html = format!("<{}>", list_type);
    for &root_idx in &tree.root_indices {
        html.push_str(&render_node(tree, root_idx, list_type));
    }
    html.push_str(&format!("</{}>", list_type));
    html
}

pub(crate) struct GenerateToc;
impl Function for GenerateToc {
    fn call(&self, args: &HashMap<String, Value>) -> tera::Result<Value> {
        let toc = args
            .get("toc")
            .ok_or_else(|| Error::msg("Missing 'toc' argument"))?;
        let list_type = args
            .get("list_type")
            .and_then(|v| v.as_str())
            .unwrap_or("ol");

        let nodes = parse_toc(toc)?;
        let html = generate_nested_html(&nodes, list_type);
        Ok(Value::String(html))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

// Tera engine construction

/// Constructs a Tera engine with the given template directories.
///
/// Loads default error templates, then theme templates, then user templates,
/// and registers the custom function `generate_toc`.
pub(crate) fn init(templates_dir: &str, theme_templates_dir: &Path) -> eyre::Result<Tera> {
    let mut tera = Tera::default();

    // Register built-in error templates as defaults; theme/user templates override via extend().
    tera.add_raw_template("404.html", include_str!("../resources/templates/404.html"))
        .map_err(|e| eyre!("Failed to register default 404 template: {}", e))?;
    tera.add_raw_template("500.html", include_str!("../resources/templates/500.html"))
        .map_err(|e| eyre!("Failed to register default 500 template: {}", e))?;

    // Loading theme templates first allows the user to extend the theme templates using their own user-defined
    // templates aka inheriting from the theme templates.
    if theme_templates_dir.exists() {
        let theme_glob = format!("{}/**/*.html", theme_templates_dir.display());
        let theme_tera =
            Tera::parse(&theme_glob).map_err(|e| eyre!("Error parsing theme templates: {}", e))?;
        tera.extend(&theme_tera)?;

        let theme_xml_glob = format!("{}/**/*.xml", theme_templates_dir.display());
        let theme_xml_tera = Tera::parse(&theme_xml_glob)
            .map_err(|e| eyre!("Error parsing theme XML templates: {}", e))?;
        tera.extend(&theme_xml_tera)?;
    }

    // Load user's templates
    let user_glob = format!("{}/**/*.html", templates_dir);
    let user_tera =
        Tera::parse(&user_glob).map_err(|e| eyre!("Error parsing user templates: {}", e))?;
    tera.extend(&user_tera)?;

    let xml_glob = format!("{}/**/*.xml", templates_dir);
    let xml_tera =
        Tera::parse(&xml_glob).map_err(|e| eyre!("Error parsing user XML templates: {}", e))?;
    tera.extend(&xml_tera)?;

    tera.build_inheritance_chains()
        .map_err(|e| eyre!("{}: {}", "Failed to build templates inheritance".bold(), e))?;

    // Register functions
    tera.register_function("generate_toc", GenerateToc);

    Ok(tera)
}
