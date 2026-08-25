//! Integration test: drives the compiled norgolith-mcp binary over stdio,
//! simulating an elicitation-capable MCP client, against a throwaway site.
//!
//! Requires NORGOLITH_BIN pointing at a lith executable (>= --format json);
//! skipped when unset:
//!   NORGOLITH_BIN=./target/debug/lith cargo test -p norgolith-mcp --test elicitation

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};

struct Server {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Server {
    fn start() -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_norgolith-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn norgolith-mcp");
        let stdin = child.stdin.take().unwrap();
        let reader = BufReader::new(child.stdout.take().unwrap());
        Server {
            child,
            stdin,
            reader,
        }
    }

    fn send(&mut self, v: &Value) {
        writeln!(self.stdin, "{}", v).expect("write request");
        self.stdin.flush().expect("flush");
    }

    /// Reads lines until a message with the given id arrives; answers any
    /// elicitation/create along the way using `answers` (popped in order).
    fn wait_for(&mut self, id: i64, answers: &mut std::collections::VecDeque<Value>) -> Value {
        loop {
            let mut line = String::new();
            let n = self.reader.read_line(&mut line).expect("read response");
            assert!(n > 0, "server closed stdout before responding to id {id}");
            let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            if v["method"] == "elicitation/create" {
                let req_id = v["id"].as_i64().expect("elicitation request id");
                let action = answers.pop_front().expect("scripted elicitation answer");
                self.send(&json!({"jsonrpc": "2.0", "id": req_id, "result": action}));
                continue;
            }
            if v["id"].as_i64() == Some(id) {
                return v;
            }
        }
    }
}

fn write_site(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("content")).unwrap();
    std::fs::write(
        dir.join("norgolith.toml"),
        r#"
rootUrl = 'http://localhost:3030'
language = 'en-US'
title = 'fixsite'
author = 'test'

[content_schema]
required = ["title"]

[content_schema.fields.title]
type = "string"
max_length = 12
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("content/index.norg"),
        "@document.meta\ntitle: A Very Long Title\n@end\n\n# Body\n",
    )
    .unwrap();
}

#[test]
fn fix_diagnostic_elicitates_applies_and_validates() {
    let Ok(lith) = std::env::var("NORGOLITH_BIN") else {
        eprintln!("skipping: NORGOLITH_BIN not set");
        return;
    };
    let dir = std::env::temp_dir().join(format!("lith-fixtest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    write_site(&dir);

    let mut srv = Server::start();
    srv.send(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {"elicitation": {}}}
    }));
    let init = srv.wait_for(1, &mut Default::default());
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");

    // Accept the prompt but override with a custom value.
    srv.send(&json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "fix_diagnostic",
                   "arguments": {"project_dir": dir.to_string_lossy()}}
    }));
    let result = srv.wait_for(
        2,
        &mut [json!({"action": "accept", "content": {"pick": "", "value": "Tiny Site"}})]
            .into_iter()
            .collect(),
    );
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("tool text result");
    assert!(text.contains("Applied:"), "{text}");
    assert!(text.contains("title fixed"), "{text}");

    let content = std::fs::read_to_string(dir.join("content/index.norg")).unwrap();
    assert!(content.contains("title: Tiny Site"), "{content}");
    assert!(content.contains("# Body"));

    // A second run finds nothing left to fix.
    srv.send(&json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "fix_diagnostic",
                   "arguments": {"project_dir": dir.to_string_lossy()}}
    }));
    let result = srv.wait_for(3, &mut Default::default());
    let text = result["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Nothing to fix"), "{text}");

    let _ = std::fs::remove_dir_all(&dir);
    drop(lith);
}

#[test]
fn fix_diagnostic_decline_leaves_file_untouched() {
    let Ok(lith) = std::env::var("NORGOLITH_BIN") else {
        eprintln!("skipping: NORGOLITH_BIN not set");
        return;
    };
    let dir = std::env::temp_dir().join(format!("lith-fixdecline-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    write_site(&dir);
    let before = std::fs::read_to_string(dir.join("content/index.norg")).unwrap();

    let mut srv = Server::start();
    srv.send(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {"elicitation": {}}}
    }));
    let _ = srv.wait_for(1, &mut Default::default());

    srv.send(&json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "fix_diagnostic",
                   "arguments": {"project_dir": dir.to_string_lossy()}}
    }));
    let result = srv.wait_for(2, &mut [json!({"action": "decline"})].into_iter().collect());
    let text = result["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("Skipped") && text.contains("decline"),
        "{text}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("content/index.norg")).unwrap(),
        before
    );

    let _ = std::fs::remove_dir_all(&dir);
    drop(lith);
}
