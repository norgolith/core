use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

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
            json!({
                "uri": e.uri,
                "name": e.name,
                "mimeType": "text/x-norg"
            })
        })
        .collect();

    json!({"resources": resources})
}

fn handle_resources_read(req: &Value) -> Value {
    let uri = req["params"]["uri"].as_str().unwrap_or("");

    for entry in DOC_ENTRIES {
        if entry.uri == uri {
            return json!({
                "contents": [{
                    "uri": entry.uri,
                    "mimeType": "text/x-norg",
                    "text": entry.content
                }]
            });
        }
    }

    json!({"error": {"code": -32602, "message": format!("Resource not found: {}", uri)}})
}

fn handle_tools_list() -> Value {
    json!({
        "tools": [{
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
        }]
    })
}

fn handle_tools_call(req: &Value) -> Value {
    let name = req["params"]["name"].as_str().unwrap_or("");
    let args = &req["params"]["arguments"];

    if name != "search_docs" {
        return json!({"error": {"code": -32602, "message": format!("Unknown tool: {}", name)}});
    }

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
