use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/docs.rs"));

/// Whether the client advertised `capabilities.elicitation` during initialize.
// NOTE: no mainstream client advertises elicitation yet; OpenCode tracks it
// for v2 (anomalyco/opencode#28567). Until then fix_diagnostic is inert in
// the wild; tests/elicitation.rs simulates a capable client as the proxy.
static CLIENT_ELICITATION: OnceLock<bool> = OnceLock::new();

/// Bidirectional connection for server-to-client requests (elicitation).
struct Conn<'a> {
    reader: &'a mut dyn BufRead,
    writer: &'a mut dyn Write,
}

impl Conn<'_> {
    /// Sends a server-to-client request and waits for its matching response.
    // XXX: assumes the client answers synchronously during tools/call and
    // does not pipeline unrelated requests; true for every stdio MCP client
    // today. Revisit with a proper demux if that changes.
    fn request(&mut self, method: &str, params: Value) -> Option<Value> {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1000);
        let id = json!(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.writer, "{}", serde_json::to_string(&req).ok()?).ok()?;
        self.writer.flush().ok()?;

        loop {
            let mut line = String::new();
            let n = self.reader.read_line(&mut line).ok()?;
            if n == 0 {
                return None; // EOF
            }
            let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            // Responses carry an id and either result or error; skip anything else.
            if v["id"] == id && !v["result"].is_null() || v["error"].is_object() && v["id"] == id {
                return Some(v);
            }
        }
    }

    fn elicit(&mut self, message: &str, schema: Value) -> Option<Value> {
        let resp = self.request(
            "elicitation/create",
            json!({
                "mode": "form",
                "message": message,
                "requestedSchema": schema,
            }),
        )?;
        Some(resp["result"].clone())
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    loop {
        let mut line = String::new();
        let n = match reader.read_line(&mut line) {
            Ok(n) => n,
            Err(_) => break,
        };
        if n == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = req["method"].as_str().unwrap_or("").to_owned();
        let id = req["id"].clone();

        let response = match method.as_str() {
            "initialize" => handle_initialize(&req),
            "ping" => wrap(&id, json!({})),
            "notifications/initialized" => continue,
            "resources/list" => wrap(&id, handle_resources_list()),
            "resources/read" => wrap(&id, handle_resources_read(&req)),
            "tools/list" => wrap(&id, handle_tools_list()),
            "tools/call" => wrap(&id, handle_tools_call(&req, &mut reader, &mut writer)),
            _ => {
                if id.is_null() {
                    continue;
                }
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "Method not found"}})
            }
        };

        if id.is_null() {
            continue;
        }

        let _ = writeln!(writer, "{}", serde_json::to_string(&response).unwrap());
        let _ = writer.flush();
    }
}

fn wrap(id: &Value, result: Value) -> Value {
    if let Some(err) = result.get("error") {
        return json!({"jsonrpc": "2.0", "id": id, "error": err});
    }
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// Newest spec revision this server implements; elicitation needs >= 2025-06-18.
const SPEC_VERSION: &str = "2025-06-18";

fn handle_initialize(req: &Value) -> Value {
    let client_version = req["params"]["protocolVersion"]
        .as_str()
        .unwrap_or("2024-11-05");
    // Echo the client's version unless we both speak something newer.
    let negotiated =
        if client_version.as_bytes() > SPEC_VERSION.as_bytes() || client_version == SPEC_VERSION {
            SPEC_VERSION
        } else {
            client_version
        };
    let _ = CLIENT_ELICITATION.set(req["params"]["capabilities"]["elicitation"].is_object());
    json!({
        "jsonrpc": "2.0",
        "id": req["id"],
        "result": {
            "protocolVersion": negotiated,
            "capabilities": {
                "resources": {},
                "tools": {}
            },
            "serverInfo": {
                "name": "norgolith-mcp",
                "version": "1.1.0"
            }
        }
    })
}

fn handle_resources_list() -> Value {
    let resources: Vec<Value> = DOC_ENTRIES
        .iter()
        .map(|e| {
            let mime = if e.uri.starts_with("norgolith://src/") {
                "text/x-rust"
            } else {
                "text/x-norg"
            };
            json!({
                "uri": e.uri,
                "name": e.name,
                "mimeType": mime,
            })
        })
        .collect();

    json!({"resources": resources})
}

fn handle_resources_read(req: &Value) -> Value {
    let uri = req["params"]["uri"].as_str().unwrap_or("");

    for entry in DOC_ENTRIES {
        if entry.uri == uri {
            let mime = if uri.starts_with("norgolith://src/") {
                "text/x-rust"
            } else {
                "text/x-norg"
            };
            return json!({
                "contents": [{
                    "uri": entry.uri,
                    "mimeType": mime,
                    "text": entry.content
                }]
            });
        }
    }

    json!({"error": {"code": -32602, "message": format!("Resource not found: {}", uri)}})
}

fn handle_tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "search_docs",
                "description": "Search Norgolith documentation content",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query to find in documentation"
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "read_source",
                "description": "Read a source file from the norgolith repository",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative path from repo root (e.g., core/src/plugin/mod.rs)"
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "run_check",
                "description": "Validate content metadata of a Norgolith site against its content schema. Returns a structured JSON report with a summary and per-file diagnostics (code, severity, message, help, line/column).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_dir": {
                            "type": "string",
                            "description": "Path to the Norgolith project directory (containing norgolith.toml). Defaults to the server working directory."
                        }
                    }
                }
            },
            {
                "name": "run_build",
                "description": "Run a production build of a Norgolith site and return the build output with exit status.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_dir": {
                            "type": "string",
                            "description": "Path to the Norgolith project directory (containing norgolith.toml). Defaults to the server working directory."
                        },
                        "no_minify": {
                            "type": "boolean",
                            "description": "Skip asset minification during the build"
                        }
                    }
                }
            },
            {
                "name": "fix_diagnostic",
                "description": "Interactively fix schema validation errors in a Norgolith site content files. For each fixable diagnostic, prompts the user (via elicitation) with suggested values and a free-text field, applies the chosen fix to the .norg metadata, and re-validates; patches that do not reduce the error count are reverted automatically.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_dir": {
                            "type": "string",
                            "description": "Path to the Norgolith project directory (containing norgolith.toml). Defaults to the server working directory."
                        }
                    }
                }
            },
            {
                "name": "explain_error",
                "description": "Explain a norgolith error code: what it means, common causes, how to fix it, and the documentation URL. Accepts full codes (norgolith::schema::missing_field) or short names (missing_field).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "code": {
                            "type": "string",
                            "description": "The error code to explain"
                        }
                    },
                    "required": ["code"]
                }
            }
        ]
    })
}

fn handle_tools_call(req: &Value, reader: &mut dyn BufRead, writer: &mut dyn Write) -> Value {
    let name = req["params"]["name"].as_str().unwrap_or("");
    let args = &req["params"]["arguments"];

    match name {
        "search_docs" => call_search_docs(args),
        "read_source" => call_read_source(args),
        "run_check" => call_run_check(args),
        "run_build" => call_run_build(args),
        "explain_error" => call_explain_error(args),
        "fix_diagnostic" => {
            let mut conn = Conn { reader, writer };
            call_fix_diagnostic(args, &mut conn)
        }
        _ => json!({"error": {"code": -32602, "message": format!("Unknown tool: {}", name)}}),
    }
}

/// Builds a `lith` command, honoring `NORGOLITH_BIN` so callers can pin a
/// binary newer than the installed one.
// HACK: no timeout on child processes; lith commands are user-triggered
// and bounded by site size. Wrap with wait_timeout if runaway builds matter.
fn lith_command() -> std::process::Command {
    std::process::Command::new(std::env::var("NORGOLITH_BIN").unwrap_or_else(|_| "lith".into()))
}

fn text_result(text: impl Into<String>) -> Value {
    json!({"content": [{"type": "text", "text": text.into()}]})
}

/// Runs `lith check --format json` and returns the parsed report.
fn run_check_report(project_dir: Option<&str>) -> Result<Value, String> {
    let mut cmd = lith_command();
    if let Some(dir) = project_dir {
        cmd.arg("-d").arg(dir);
    }
    cmd.args(["check", "--format", "json"]);

    let out = cmd.output().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            "lith binary not found. Install it (`cargo install norgolith`) or point the NORGOLITH_BIN environment variable at a lith executable.".to_string()
        } else {
            format!("Failed to spawn lith: {e}")
        }
    })?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Value>(&stdout).map_err(|_| {
        format!(
            "lith check exited with {} (no JSON report)\n\nstdout:\n{}\nstderr:\n{}",
            out.status,
            stdout.trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        )
    })
}

fn report_error_count(report: &Value) -> usize {
    report["summary"]["errors"].as_u64().unwrap_or(0) as usize
}

fn call_run_check(args: &Value) -> Value {
    match run_check_report(args["project_dir"].as_str()) {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(pretty) => text_result(pretty),
            Err(e) => text_result(format!("Failed to serialize report: {e}")),
        },
        Err(text) => text_result(text),
    }
}

fn call_run_build(args: &Value) -> Value {
    let mut cmd = lith_command();
    if let Some(dir) = args["project_dir"].as_str() {
        cmd.arg("-d").arg(dir);
    }
    cmd.arg("build");
    if args["no_minify"].as_bool().unwrap_or(false) {
        cmd.arg("--no-minify");
    }

    match cmd.output() {
        Ok(out) => {
            let mut text = format!(
                "lith build exited with {}\n\nstdout:\n{}",
                out.status,
                String::from_utf8_lossy(&out.stdout).trim()
            );
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.trim().is_empty() {
                text.push_str("\n\nstderr:\n");
                text.push_str(stderr.trim());
            }
            text_result(text)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => text_result(
            "lith binary not found. Install it (`cargo install norgolith`) or point the NORGOLITH_BIN environment variable at a lith executable.",
        ),
        Err(e) => text_result(format!("Failed to spawn lith: {e}")),
    }
}

/// Explains a norgolith error code. Accepts both the full form
/// (`norgolith::schema::missing_field`) and the short name (`missing_field`).
fn call_explain_error(args: &Value) -> Value {
    let code = args["code"].as_str().unwrap_or("").trim();
    if code.is_empty() {
        return text_result("No error code provided.");
    }
    let key = code.rsplit("::").next().unwrap_or(code);

    // XXX: keep in sync with core/src/schema/mod.rs ValidationError variants
    let (title, cause, fix, url): (&str, &str, &str, &str) = match key {
        "missing_field" => (
            "Missing field",
            "A field marked as required by the content schema is absent from the @document.meta block.",
            "Add the missing field to the metadata block. Check which schema applies to the file's path (schemas are hierarchical: content/blog/** matches before **).",
            "https://norgolith.dev/docs/content-schemas/#norgolith-schema-missing-field",
        ),
        "type_mismatch" => (
            "Type mismatch",
            "A metadata field has a value whose type differs from the schema definition (e.g. version: 1.0 parsed as number when the schema expects string).",
            "Change the value to the expected type. Quote values that look numeric but must be strings, e.g. version = '1.0'.",
            "https://norgolith.dev/docs/content-schemas/#norgolith-schema-type-mismatch",
        ),
        "constraint_violation" => (
            "Constraint violation",
            "A field violates a constraint defined in the schema: max_length, min_length, or a custom rule.",
            "Read the violation message for the exact limit exceeded and adjust the value.",
            "https://norgolith.dev/docs/content-schemas/#norgolith-schema-constraint-violation",
        ),
        "rule_condition" => (
            "Rule condition failed",
            "A conditional schema rule did not apply as configured (warning severity, does not fail builds).",
            "Review the condition fields in your [content_schema.rules] configuration; this warning usually means the rule referenced fields that do not exist.",
            "https://norgolith.dev/docs/content-schemas/#norgolith-schema-rule-condition",
        ),
        "unknown_field" => (
            "Unknown field",
            "The metadata contains a field not defined in the content schema. A close-match suggestion may accompany the warning (warning severity, does not fail builds).",
            "Fix the typo if there was a suggestion, or add the field definition under [content_schema.fields.<name>] if it is intentional.",
            "https://norgolith.dev/docs/content-schemas/#norgolith-schema-unknown-field",
        ),
        _ => {
            return text_result(format!(
                "Unknown error code: {code}. Known codes: missing_field, type_mismatch, constraint_violation, rule_condition, unknown_field"
            ));
        }
    };

    text_result(format!(
        "{title}\n\nCode: norgolith::schema::{key}\n\nWhat it means:\n{cause}\n\nHow to fix:\n{fix}\n\nDocs: {url}"
    ))
}

fn call_search_docs(args: &Value) -> Value {
    let query = args["query"].as_str().unwrap_or("").to_lowercase();
    if query.is_empty() {
        return json!({
            "content": [{"type": "text", "text": "No query provided."}]
        });
    }

    let mut results = Vec::new();

    for entry in DOC_ENTRIES {
        for (i, line) in entry.content.lines().enumerate() {
            if line.to_lowercase().contains(&query) {
                let line_num = i + 1;
                let snippet = line.trim();
                if snippet.len() > 120 {
                    let truncated: String = snippet.chars().take(120).collect();
                    results.push(format!("{}:{}: {}...", entry.uri, line_num, truncated));
                } else {
                    results.push(format!("{}:{}: {}", entry.uri, line_num, snippet));
                }
            }
        }
    }

    if results.is_empty() {
        return json!({
            "content": [{"type": "text", "text": format!("No results found for: {}", query)}]
        });
    }

    // XXX: cap at 50 results to avoid huge responses
    let text = if results.len() > 50 {
        results.truncate(50);
        format!("{} (showing first 50 results)\n\n", results.len()) + &results.join("\n")
    } else {
        results.join("\n")
    };

    json!({
        "content": [{"type": "text", "text": text}]
    })
}

fn call_read_source(args: &Value) -> Value {
    let path = args["path"].as_str().unwrap_or("");
    if path.is_empty() {
        return json!({
            "content": [{"type": "text", "text": "No path provided."}]
        });
    }

    let root = match option_env!("NORGOLITH_ROOT") {
        Some(r) => r,
        None => {
            return json!({
                "content": [{"type": "text", "text": "Repository root not embedded. This tool only works in the norgolith repository."}]
            });
        }
    };

    let full_path = Path::new(root).join(path);

    // XXX: no path traversal protection. Only runs in trusted monorepo context.
    match std::fs::read_to_string(&full_path) {
        Ok(content) => json!({
            "content": [{"type": "text", "text": content}]
        }),
        Err(e) => json!({
            "content": [{"type": "text", "text": format!("Could not read {}: {}", path, e)}]
        }),
    }
}

// ---- fix_diagnostic: interactive schema fixes with user elicitation -------

/// A planned, single-diagnostic fix. Only shapes we know how to perform as
/// line surgery on the @document.meta block are planned; everything else is
/// reported back untouched.
#[derive(Clone)]
struct Fix {
    file: String,
    path: Vec<String>,
    description: String,
    message: String,
    candidates: Vec<String>,
    /// Sub-fields to prompt for when scaffolding a missing object field.
    scaffold_fields: &'static [&'static str],
    /// Insert a new block (missing field) vs replace an existing line.
    insert: bool,
}

/// Byte range of the first `@document.meta` block interior (between the
/// header line's newline and the `@end` marker).
fn meta_span(content: &str) -> Option<std::ops::Range<usize>> {
    let start = content.find("@document.meta")?;
    let after = start + "@document.meta".len();
    let nl = content[after..].find('\n')?;
    let body_start = after + nl + 1;
    let end_rel = content[body_start..].find("@end")?;
    Some(body_start..body_start + end_rel)
}

/// Line indexes of the meta block: first interior line and the `@end` line.
fn block_line_range(content: &str) -> Option<(usize, usize)> {
    let span = meta_span(content)?;
    let start = content[..span.start].matches('\n').count();
    let interior_lines = content[span.start..span.end].lines().count();
    Some((start, start + interior_lines))
}

/// Locates a scalar key line inside the meta block. Path is `["title"]` or
/// `["author", "name"]`. Returns (line index, current raw value).
fn find_scalar(content: &str, path: &[&str]) -> Option<(usize, String)> {
    let (bs, be) = block_line_range(content)?;
    let mut sub_open = false;
    for i in bs..be {
        let line = content.lines().nth(i)?;
        let t = line.trim_start();
        if path.len() == 1 && !sub_open && t.starts_with(&format!("{}:", path[0])) {
            return Some((i, t[path[0].len() + 1..].trim().to_string()));
        }
        if path.len() == 2 {
            if !sub_open && t.starts_with(&format!("{}:", path[0])) && t.contains('{') {
                sub_open = true;
            } else if sub_open && t == "}" {
                sub_open = false;
            } else if sub_open && t.starts_with(&format!("{}:", path[1])) {
                return Some((i, t[path[1].len() + 1..].trim().to_string()));
            }
        }
    }
    None
}

/// Locates the line index of the first item of a top-level array key.
fn find_array_first_item(content: &str, key: &str) -> Option<usize> {
    let (bs, be) = block_line_range(content)?;
    let mut in_array = false;
    for i in bs..be {
        let t = content.lines().nth(i)?.trim_start();
        if !in_array {
            if t.starts_with(&format!("{key}:")) && t.contains('[') {
                in_array = true;
            }
        } else if t.starts_with(']') {
            return None;
        } else if !t.is_empty() {
            return Some(i);
        }
    }
    None
}

/// Rewrites one line, preserving indentation and the original key text.
fn replace_line_value(content: &str, idx: usize, new_value: &str) -> String {
    content
        .lines()
        .enumerate()
        .map(|(i, line)| {
            if i != idx {
                return line.to_string();
            }
            let indent_len = line.len() - line.trim_start().len();
            let indent = &line[..indent_len];
            let key = line.trim_start().split(':').next().unwrap_or("");
            format!("{indent}{key}: {new_value}")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if content.ends_with('\n') { "\n" } else { "" }
}

fn join_lines(mut lines: Vec<String>, had_trailing_newline: bool) -> String {
    if had_trailing_newline {
        lines.push(String::new());
    }
    lines.join("\n")
}

/// Replaces the whole line at idx with an indented multi-line block.
fn replace_line_with_block(
    content: &str,
    idx: usize,
    block: &[String],
    base_indent: &str,
) -> String {
    let indented: Vec<String> = block
        .iter()
        .map(|l| {
            if l.is_empty() {
                l.clone()
            } else {
                format!("{base_indent}{l}")
            }
        })
        .collect();
    join_lines(
        content
            .lines()
            .enumerate()
            .flat_map(|(i, line)| {
                if i == idx {
                    indented.clone()
                } else {
                    vec![line.to_string()]
                }
            })
            .collect(),
        content.ends_with('\n'),
    )
}

/// Inserts new lines immediately before the `@end` of the meta block.
fn insert_before_end(content: &str, block: &[String]) -> String {
    let (_, end_line) = match block_line_range(content) {
        Some(r) => r,
        None => return content.to_string(),
    };
    join_lines(
        content
            .lines()
            .enumerate()
            .flat_map(|(i, line)| {
                if i == end_line {
                    let mut with_block = block.to_vec();
                    with_block.push(line.to_string());
                    with_block
                } else {
                    vec![line.to_string()]
                }
            })
            .collect(),
        content.ends_with('\n'),
    )
}

// INFO: values are written verbatim, norg meta has no quoting mechanism, so
// any added quotes become literal data (verified against rust-norg parsing).
// The re-validation gate catches structural breakage instead of trying to
// pre-sanitize.
fn fmt_value(v: &str) -> String {
    v.trim().to_string()
}

/// Builds the list of fixes we know how to perform from a check report.
fn plan_fixes(report: &Value) -> Vec<Fix> {
    let mut fixes = Vec::new();
    let empty = Vec::new();
    for diag in report["diagnostics"].as_array().unwrap_or(&empty) {
        let code = diag["code"].as_str().unwrap_or("");
        let message = diag["message"].as_str().unwrap_or("");
        let file = diag["file"].as_str().unwrap_or("").to_string();
        if diag["severity"].as_str().unwrap_or("error") != "error" {
            continue;
        }

        // Constraint violation ... Exceeds max length N
        if code.ends_with("constraint_violation") {
            if let Some(field) = message.split('\'').nth(1) {
                let limit = message
                    .split("Exceeds max length ")
                    .nth(1)
                    .and_then(|s| s.split_whitespace().next())
                    .and_then(|s| s.parse::<usize>().ok());
                if let Some(limit) = limit {
                    fixes.push(Fix {
                        file,
                        path: field.split('.').map(str::to_string).collect(),
                        description: format!("Shorten value (max_length {limit})"),
                        message: message.to_string(),
                        candidates: Vec::new(),
                        scaffold_fields: &[],
                        insert: false,
                    });
                }
            }
            continue;
        }

        // INFO: type_mismatch "expected string, got <number>" is deliberately not
        // fixed: norg meta has no quoting mechanism (quotes are literal
        // characters), so a numeric-looking string cannot be expressed. The
        // user must change the schema type or the value instead.

        // Missing object fields we have scaffolds for
        if code.ends_with("missing_field") {
            let field = message.split('\'').nth(1).unwrap_or("");
            let fields: &[&str] = match field {
                "author" => &["name", "email"],
                _ => &[],
            };
            if !fields.is_empty() {
                fixes.push(Fix {
                    file,
                    path: vec![field.to_string()],
                    description: format!("Scaffold missing `{field}` object"),
                    message: message.to_string(),
                    candidates: Vec::new(),
                    scaffold_fields: fields,
                    insert: true,
                });
            }
        }
    }
    fixes
}

fn elicit_schema_for(fix: &Fix, current: Option<&str>) -> Value {
    let mut props = serde_json::Map::new();
    if fix.scaffold_fields.is_empty() {
        if !fix.candidates.is_empty() {
            props.insert(
                "pick".into(),
                json!({"type": "string", "enum": fix.candidates,
                       "description": "Choose a suggestion (takes precedence over the free-text value)"}),
            );
        }
        props.insert(
            "value".into(),
            json!({
                "type": "string",
                "description": match current {
                    Some(c) => format!("Replacement value (current: {c})"),
                    None => "New value".to_string(),
                }
            }),
        );
        json!({"type": "object", "properties": props, "required": ["value"]})
    } else {
        for f in fix.scaffold_fields {
            props.insert(
                (*f).to_string(),
                json!({"type": "string",
                       "description": format!("{f} for {}", fix.path.join("."))}),
            );
        }
        json!({"type": "object", "properties": props,
               "required": [fix.scaffold_fields[0]]})
    }
}

/// Performs the file edit for a fix. Pure string transform; the caller owns
/// write + validation + revert.
fn apply_transform(
    content: &str,
    fix: &Fix,
    values: &serde_json::Map<String, Value>,
) -> Result<String, String> {
    if !fix.scaffold_fields.is_empty() {
        let picked: Vec<&str> = fix
            .scaffold_fields
            .iter()
            .map(|f| values.get(*f).and_then(|v| v.as_str()).unwrap_or(""))
            .collect();
        if picked.iter().all(|v| v.is_empty()) {
            return Err("All scaffold fields were left empty".to_string());
        }
        let body: Vec<String> = fix
            .scaffold_fields
            .iter()
            .zip(picked.iter())
            .map(|(f, v)| format!("  {f}: {}", fmt_value(v)))
            .chain(std::iter::once("}".to_string()))
            .collect();

        if fix.insert {
            Ok(insert_before_end(content, &body))
        } else {
            let idx = find_array_first_item(content, &fix.path[0])
                .ok_or_else(|| format!("Could not locate array items for '{}'", fix.path[0]))?;
            let item_line = content.lines().nth(idx).unwrap_or_default();
            let indent = &item_line[..item_line.len() - item_line.trim_start().len()];
            // The replaced line keeps its array position; open the object there.
            let mut block = vec!["{".to_string()];
            block.extend(body.iter().cloned());
            Ok(replace_line_with_block(content, idx, &block, indent))
        }
    } else {
        let custom = values.get("value").and_then(|v| v.as_str()).unwrap_or("");
        let pick = values.get("pick").and_then(|v| v.as_str()).unwrap_or("");
        let chosen = if !pick.is_empty() { pick } else { custom };
        if chosen.is_empty() {
            return Err("No replacement value provided".to_string());
        }
        let path: Vec<&str> = fix.path.iter().map(String::as_str).collect();
        let (idx, _) = find_scalar(content, &path).ok_or_else(|| {
            format!(
                "Could not locate field '{}' in the metadata block",
                fix.path.join(".")
            )
        })?;
        Ok(replace_line_value(content, idx, &fmt_value(chosen)))
    }
}

fn call_fix_diagnostic(args: &Value, conn: &mut Conn) -> Value {
    if !CLIENT_ELICITATION.get().copied().unwrap_or(false) {
        return text_result(
            "fix_diagnostic requires an MCP client that supports elicitation \
             (capabilities.elicitation). Use run_check instead and ask the user \
             for replacement values before editing files.",
        );
    }

    let project_dir = args["project_dir"].as_str().unwrap_or(".");
    let report = match run_check_report(args["project_dir"].as_str()) {
        Ok(r) => r,
        Err(text) => return text_result(text),
    };
    let error_count = report_error_count(&report);
    if error_count == 0 {
        return text_result("Nothing to fix: all content passed schema validation.");
    }

    let fixes = plan_fixes(&report);
    let unhandled = error_count.saturating_sub(fixes.len());
    if fixes.is_empty() {
        return text_result(format!(
            "{error_count} error(s) found but none are auto-fixable. Run run_check for details."
        ));
    }

    // XXX: file paths come straight from our own lith invocation; only paths
    // under the checked project's content/ tree are touched. Revisit if lith
    // ever reports untrusted locations.
    let mut applied = Vec::new();
    let mut skipped = Vec::new();
    let mut rejected = Vec::new();
    let mut current_errors = error_count;

    for fix in fixes {
        let Ok(original) = std::fs::read_to_string(&fix.file) else {
            skipped.push(format!("{}: could not read file", fix.file));
            continue;
        };

        let rel_file = Path::new(&fix.file)
            .strip_prefix(project_dir)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| fix.file.clone());

        let mut prompt_fix = fix.clone();
        if let Some(limit) = fix
            .message
            .split("Exceeds max length ")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse::<usize>().ok())
        {
            let path: Vec<&str> = fix.path.iter().map(String::as_str).collect();
            if let Some((_, current)) = find_scalar(&original, &path) {
                prompt_fix.candidates = vec![current.chars().take(limit).collect()];
            }
        }
        let current = find_scalar(
            &original,
            &fix.path.iter().map(String::as_str).collect::<Vec<_>>(),
        )
        .map(|(_, v)| v);

        let msg = format!(
            "{}\n\n{}\n\n{}",
            rel_file,
            fix.message,
            if fix.description.is_empty() {
                "Provide a replacement:".to_string()
            } else {
                format!(
                    "{}: provide a replacement or pick a suggestion",
                    fix.description
                )
            }
        );

        let Some(result) = conn.elicit(&msg, elicit_schema_for(&prompt_fix, current.as_deref()))
        else {
            skipped.push(format!("{rel_file}: elicitation failed"));
            continue;
        };
        match result["action"].as_str().unwrap_or("cancel") {
            "accept" => {
                let Some(content) = result["content"].as_object() else {
                    skipped.push(format!("{rel_file}: no form data returned"));
                    continue;
                };
                let patched = match apply_transform(&original, &prompt_fix, content) {
                    Ok(p) => p,
                    Err(e) => {
                        skipped.push(format!("{rel_file}: {e}"));
                        continue;
                    }
                };
                if std::fs::write(&fix.file, &patched).is_err() {
                    skipped.push(format!("{rel_file}: write failed"));
                    continue;
                }
                // Validation gate: keep the patch only if errors went down.
                let after = match run_check_report(args["project_dir"].as_str()) {
                    Ok(r) => r,
                    Err(text) => {
                        let _ = std::fs::write(&fix.file, &original);
                        rejected.push(format!(
                            "{rel_file}: reverted (re-validation failed: {text})"
                        ));
                        continue;
                    }
                };
                let new_count = report_error_count(&after);
                if new_count < current_errors {
                    applied.push(format!(
                        "{rel_file}: {} fixed ({} -> {} error(s))",
                        fix.path.join("."),
                        current_errors,
                        new_count
                    ));
                    current_errors = new_count;
                } else {
                    let _ = std::fs::write(&fix.file, &original);
                    rejected.push(format!(
                        "{rel_file}: reverted (errors unchanged at {current_errors})"
                    ));
                }
            }
            action => skipped.push(format!("{rel_file}: user chose {action}")),
        }
    }

    let mut summary = String::new();
    if !applied.is_empty() {
        summary.push_str(&format!("Applied:\n{}\n", applied.join("\n")));
    }
    if !rejected.is_empty() {
        summary.push_str(&format!(
            "\nRejected (auto-reverted):\n{}\n",
            rejected.join("\n")
        ));
    }
    if !skipped.is_empty() {
        summary.push_str(&format!("\nSkipped:\n{}\n", skipped.join("\n")));
    }
    if unhandled > 0 {
        summary.push_str(&format!(
            "\n{unhandled} diagnostic(s) had no automatic fix; use run_check to review them.\n"
        ));
    }
    text_result(if summary.is_empty() {
        "No fixes performed.".to_string()
    } else {
        summary.trim_end().to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const META: &str = "@document.meta\ntitle: Greeting\nversion: 1.0\nauthor: {\n  name: Bob\n  email: b@x.io\n}\nauthors: [\n  amartin\n]\n@end\n\n# Body\n";

    #[test]
    fn meta_span_finds_block_interior() {
        let (bs, be) = block_line_range(META).unwrap();
        let lines: Vec<&str> = META.lines().collect();
        assert_eq!(lines[bs], "title: Greeting");
        assert!(lines[be].starts_with("@end"));
    }

    #[test]
    fn find_scalar_handles_top_level_and_nested() {
        let (i, v) = find_scalar(META, &["title"]).unwrap();
        assert_eq!(v, "Greeting");
        let lines: Vec<&str> = META.lines().collect();
        assert_eq!(lines[i], "title: Greeting");

        let (_, name) = find_scalar(META, &["author", "name"]).unwrap();
        assert_eq!(name, "Bob");
        assert!(find_scalar(META, &["author", "nope"]).is_none());
    }

    #[test]
    fn replace_line_value_preserves_indent_and_key() {
        let (i, _) = find_scalar(META, &["author", "name"]).unwrap();
        let out = replace_line_value(META, i, "\"Alice\"");
        assert!(out.contains("  name: \"Alice\"\n"));
        assert_eq!(out.lines().count(), META.lines().count());
        // Other lines untouched
        assert!(out.contains("email: b@x.io"));
    }

    #[test]
    fn insert_before_end_appends_inside_block() {
        let block = vec!["draft: false".to_string()];
        let out = insert_before_end(META, &block);
        let idx_draft = out.find("draft: false").unwrap();
        let idx_end = out.find("@end").unwrap();
        assert!(idx_draft < idx_end);
        assert!(out.contains("# Body")); // body untouched
    }

    #[test]
    fn array_item_replacement_builds_object_in_place() {
        let idx = find_array_first_item(META, "authors").unwrap();
        let item = META.lines().nth(idx).unwrap();
        let indent = &item[..item.len() - item.trim_start().len()];
        // Mirrors apply_transform's scaffold shape: open brace + field lines + close.
        let block = vec![
            "{".to_string(),
            "  name: Alice".to_string(),
            "}".to_string(),
        ];
        let out = replace_line_with_block(META, idx, &block, indent);
        let expected = "authors: [\n  {\n    name: Alice\n  }\n]";
        assert!(out.contains(expected), "{out}");
    }

    #[test]
    fn fmt_value_passes_through_verbatim() {
        assert_eq!(fmt_value("plain-value"), "plain-value");
        assert_eq!(fmt_value("  spaced  "), "spaced");
        assert_eq!(fmt_value(""), "");
    }

    #[test]
    fn plan_fixes_maps_known_diagnostics() {
        let report = json!({
            "diagnostics": [
                {"code": "norgolith::schema::constraint_violation",
                 "severity": "error",
                 "file": "/x/a.norg",
                 "message": "Constraint violation for field 'title': Exceeds max length 12"},
                // type_mismatch numeric->string is deliberately unfixable
                {"code": "norgolith::schema::type_mismatch",
                 "severity": "error",
                 "file": "/x/a.norg",
                 "message": "Type mismatch for field 'version': expected string, got 1"},
                {"code": "norgolith::schema::missing_field",
                 "severity": "error",
                 "file": "/x/b.norg",
                 "message": "Missing field 'author'"},
                {"code": "norgolith::schema::unknown_field",
                 "severity": "warning",
                 "file": "/x/c.norg",
                 "message": "Unknown field 'Version'"}
            ]
        });
        let fixes = plan_fixes(&report);
        assert_eq!(fixes.len(), 2);
        assert_eq!(fixes[0].path, vec!["title"]);
        assert_eq!(fixes[1].scaffold_fields, &["name", "email"]);
        assert!(fixes[1].insert);
    }

    #[test]
    fn apply_transform_scalar_uses_pick_over_custom() {
        let fix = Fix {
            file: String::new(),
            path: vec!["title".into()],
            description: String::new(),
            message: String::new(),
            candidates: vec![],
            scaffold_fields: &[],
            insert: false,
        };
        let values = serde_json::Map::from_iter([
            ("value".to_string(), json!("ignored")),
            ("pick".to_string(), json!("Chosen")),
        ]);
        let out = apply_transform(META, &fix, &values).unwrap();
        assert!(out.starts_with("@document.meta\ntitle: Chosen\n"));
    }

    #[test]
    fn explain_error_normalizes_full_and_short_codes() {
        let full = call_explain_error(&json!({"code": "norgolith::schema::missing_field"}));
        let short = call_explain_error(&json!({"code": "missing_field"}));
        assert_eq!(full, short);
        assert!(
            full["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Docs: https://")
        );
    }

    #[test]
    fn explain_error_rejects_unknown_codes() {
        let out = call_explain_error(&json!({"code": "nope"}));
        assert!(
            out["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Unknown error code")
        );
    }
}
