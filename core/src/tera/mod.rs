use std::path::{Path, PathBuf};

use eyre::{eyre, Result};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use tera::value::Key;
use tera::{Error, Filter, Function, Kwargs, State, Tera, TeraResult, Value};
use walkdir::WalkDir;

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
        .ok_or_else(|| Error::message("TOC must be an array"))?;
    let mut tree = TocTree {
        nodes: Vec::new(),
        root_indices: Vec::new(),
    };
    let mut stack: Vec<usize> = Vec::new();

    for entry in entries {
        let map = entry
            .as_map()
            .ok_or_else(|| Error::message("TOC entry must be a map"))?;

        let level = map
            .get(&Key::Str("level"))
            .and_then(|v| v.as_i64())
            .ok_or_else(|| Error::message("Missing or invalid level"))? as u8;

        let title = map
            .get(&Key::Str("title"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let id = map
            .get(&Key::Str("id"))
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

        let mut escaped_title = Vec::new();
        tera::escape_html(&node.title, &mut escaped_title).expect("escape_html failed");
        let escaped_title = String::from_utf8(escaped_title).expect("not UTF-8");
        let mut html = format!(
            "<li><a href=\"#{}\">{}</a>",
            utf8_percent_encode(&node.id, UNRESERVED),
            escaped_title
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
impl Function<TeraResult<Value>> for GenerateToc {
    fn call(&self, kwargs: Kwargs, _state: &State) -> TeraResult<Value> {
        let toc = kwargs.must_get::<Value>("toc")?;
        let list_type = kwargs.get::<String>("list_type")?.unwrap_or_else(|| "ol".into());

        let nodes = parse_toc(&toc)?;
        let html = generate_nested_html(&nodes, &list_type);
        Ok(Value::from(html))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

// --- Shim filters (removed in Tera v2, kept for backward compatibility) ---

struct FilterByAttribute;
impl Filter<&Value, TeraResult<Value>> for FilterByAttribute {
    fn call(&self, val: &Value, kwargs: Kwargs, _: &State) -> TeraResult<Value> {
        let arr = val
            .as_array()
            .ok_or_else(|| Error::message("filter() requires an array"))?;
        let attr = kwargs.must_get::<&str>("attribute")?;
        let target = kwargs.must_get::<Value>("value")?;
        let filtered: Vec<Value> = arr
            .iter()
            .filter(|item| {
                item.as_map()
                    .and_then(|m| m.get(&Key::Str(attr)))
                    .is_some_and(|v| *v == target)
            })
            .cloned()
            .collect();
        Ok(Value::from(filtered))
    }
}

struct SliceFilter;
impl Filter<&Value, TeraResult<Value>> for SliceFilter {
    fn call(&self, val: &Value, kwargs: Kwargs, _: &State) -> TeraResult<Value> {
        let arr = val
            .as_array()
            .ok_or_else(|| Error::message("slice() requires an array"))?;
        let start = kwargs.get::<usize>("start")?.unwrap_or(0);
        let end = kwargs.get::<usize>("end")?.unwrap_or(arr.len());
        let end = end.min(arr.len());
        let sliced: Vec<Value> = arr[start..end].to_vec();
        Ok(Value::from(sliced))
    }
}

// Tera engine construction

/// Collect template files from a directory with the given extension.
fn collect_template_files(
    files: &mut Vec<(PathBuf, Option<String>)>,
    dir: &Path,
    ext: &str,
) {
    if !dir.exists() {
        return;
    }
    let suffix = format!(".{}", ext);
    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path().to_path_buf();
        if path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(&suffix) {
            continue;
        }
        let relative = path
            .strip_prefix(dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        files.push((path, Some(relative)));
    }
}

/// Constructs a Tera engine with the given template directories.
///
/// Loads default error templates, then theme templates, then user templates,
/// and registers the custom function `generate_toc`.
pub(crate) fn init(templates_dir: &str, theme_templates_dir: &Path) -> Result<Tera> {
    let mut tera = Tera::default();

    // Register functions FIRST (v2 validates at compile-time)
    tera.register_function("generate_toc", GenerateToc);

    // Register tera-contrib date filters/functions
    tera.register_function("now", tera_contrib::dates::now);
    tera.register_filter("date", tera_contrib::dates::date);
    tera.register_test("before", tera_contrib::dates::is_before);
    tera.register_test("after", tera_contrib::dates::is_after);

    // Register tera-contrib regex filters/tests
    tera.register_filter("striptags", tera_contrib::regex::striptags);
    tera.register_filter("spaceless", tera_contrib::regex::spaceless);
    tera.register_test("matching", tera_contrib::regex::Matching::default());
    tera.register_filter("regex_replace", tera_contrib::regex::RegexReplace::default());

    // Shim filters for v1 compatibility (removed in Tera v2)
    tera.register_filter("filter", FilterByAttribute);
    tera.register_filter("slice", SliceFilter);

    // Collect all template files from theme + user dirs into one batch.
    // load_from_glob replaces previous glob results, so we use
    // add_template_files instead which accumulates correctly.
    // Theme files first, user files second - user overrides theme on name collision.
    let mut files: Vec<(PathBuf, Option<String>)> = Vec::new();
    collect_template_files(&mut files, theme_templates_dir, "html");
    collect_template_files(&mut files, theme_templates_dir, "xml");
    collect_template_files(&mut files, Path::new(templates_dir), "html");
    collect_template_files(&mut files, Path::new(templates_dir), "xml");

    tera.add_template_files(files)
        .map_err(|e| eyre!("Error loading templates: {}", e))?;

    // Register built-in error templates AFTER all templates loaded
    // (v2 resolves inheritance at load time, so base.html must exist first)
    tera.add_raw_template("404.html", include_str!("../resources/templates/404.html"))
        .map_err(|e| eyre!("Failed to register default 404 template: {}", e))?;
    tera.add_raw_template("500.html", include_str!("../resources/templates/500.html"))
        .map_err(|e| eyre!("Failed to register default 500 template: {}", e))?;

    Ok(tera)
}
