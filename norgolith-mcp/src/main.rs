use serde_json::{json, Value};
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
        _ => json!({"error": {"code": -32602, "message": format!("Unknown tool: {}", name)}}),
    }
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
        format!("{} (showing first 50 results)\n\n", results.len())
            + &results.join("\n")
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
        None => return json!({
            "content": [{"type": "text", "text": "Repository root not embedded. This tool only works in the norgolith repository."}]
        }),
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
