use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::Path;

include!(concat!(env!("OUT_DIR"), "/docs.rs"));

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = req["method"].as_str().unwrap_or("");
        let id = &req["id"];

        let response = match method {
            "initialize" => handle_initialize(&req),
            "ping" => wrap(id, json!({})),
            "notifications/initialized" => continue,
            "resources/list" => wrap(id, handle_resources_list()),
            "resources/read" => wrap(id, handle_resources_read(&req)),
            "tools/list" => wrap(id, handle_tools_list()),
            "tools/call" => wrap(id, handle_tools_call(&req)),
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

        let mut out = stdout.lock();
        let _ = writeln!(out, "{}", serde_json::to_string(&response).unwrap());
        let _ = out.flush();
    }
}

fn wrap(id: &Value, result: Value) -> Value {
    if let Some(err) = result.get("error") {
        return json!({"jsonrpc": "2.0", "id": id, "error": err});
    }
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn handle_initialize(req: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": req["id"],
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "resources": {},
                "tools": {}
            },
            "serverInfo": {
                "name": "norgolith-mcp",
                "version": "1.0.0"
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

fn handle_tools_call(req: &Value) -> Value {
    let name = req["params"]["name"].as_str().unwrap_or("");
    let args = &req["params"]["arguments"];

    match name {
        "search_docs" => call_search_docs(args),
        "read_source" => call_read_source(args),
        "run_check" => call_run_check(args),
        "run_build" => call_run_build(args),
        "explain_error" => call_explain_error(args),
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

fn call_run_check(args: &Value) -> Value {
    let mut cmd = lith_command();
    if let Some(dir) = args["project_dir"].as_str() {
        cmd.arg("-d").arg(dir);
    }
    cmd.args(["check", "--format", "json"]);

    match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<Value>(&stdout) {
                // Structured diagnostics path (lith >= 1.4 with --format json)
                Ok(mut report) => {
                    if !out.status.success() {
                        report["exit_code"] = json!(out.status.code().unwrap_or(-1));
                    }
                    match serde_json::to_string_pretty(&report) {
                        Ok(pretty) => text_result(pretty),
                        Err(e) => text_result(format!("Failed to serialize report: {e}")),
                    }
                }
                // Older lith or fatal setup error: surface raw output
                Err(_) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    text_result(format!(
                        "lith check exited with {} (no JSON report; is the installed lith too old for --format json?)\n\nstdout:\n{}\nstderr:\n{}",
                        out.status,
                        stdout.trim(),
                        stderr.trim()
                    ))
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => text_result(
            "lith binary not found. Install it (`cargo install norgolith`) or point the NORGOLITH_BIN environment variable at a lith executable.",
        ),
        Err(e) => text_result(format!("Failed to spawn lith: {e}")),
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

#[cfg(test)]
mod tests {
    use super::*;

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
