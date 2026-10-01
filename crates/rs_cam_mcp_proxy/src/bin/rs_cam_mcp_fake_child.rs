//! Test fixture: a minimal stdio MCP server for the proxy integration tests.
//! It is not part of the product.
//!
//! The fake refuses every request except `initialize` and `ping` until it
//! receives `notifications/initialized`, as a strict server does. So a tool
//! call that succeeds after a restart proves that the proxy replayed the
//! full handshake.
//!
//! Tools: `echo {text}`, `whoami` (pid and handshake counts), `slow {ms}`,
//! `exit {code}` (exits with no answer), `crash` (SIGKILL to itself with
//! no answer) and `alloc {mb}` (touches that much memory, for an OOM test).
//! With `FAKE_IGNORE_EOF=1` the fake does not exit when its stdin closes, so
//! the proxy must send SIGTERM. `FAKE_INIT_DELAY_MS` delays the
//! `initialize` answer.

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};

type Out = Arc<Mutex<std::io::Stdout>>;

fn send(out: &Out, message: &Value) {
    let mut out = out.lock().unwrap_or_else(PoisonError::into_inner);
    let _ = writeln!(out, "{message}");
    let _ = out.flush();
}

fn reply(out: &Out, id: &Value, result: &Value) {
    send(
        out,
        &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
    );
}

fn text_result(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": false })
}

fn main() {
    let out: Out = Arc::new(Mutex::new(std::io::stdout()));
    let initialized = AtomicBool::new(false);
    let init_count = AtomicU64::new(0);
    let initialized_count = AtomicU64::new(0);
    let pid = std::process::id();
    let _ = writeln!(std::io::stderr(), "fake child started, pid {pid}");

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        // A deliberate non-JSON line on stdout: the proxy must skip it.
        if line.contains("\"say_garbage\"") {
            let mut o = out.lock().unwrap_or_else(PoisonError::into_inner);
            let _ = writeln!(o, "this is not json");
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let Some(id) = message.get("id").cloned() else {
            if method == "notifications/initialized" {
                initialized.store(true, Ordering::SeqCst);
                initialized_count.fetch_add(1, Ordering::SeqCst);
            }
            continue;
        };
        if !matches!(method, "initialize" | "ping") && !initialized.load(Ordering::SeqCst) {
            send(
                &out,
                &json!({ "jsonrpc": "2.0", "id": id,
                         "error": { "code": -32002, "message": "fake child: not initialized" } }),
            );
            continue;
        }
        match method {
            "initialize" => {
                init_count.fetch_add(1, Ordering::SeqCst);
                if let Some(ms) = std::env::var("FAKE_INIT_DELAY_MS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                {
                    std::thread::sleep(Duration::from_millis(ms));
                }
                let version = message
                    .pointer("/params/protocolVersion")
                    .cloned()
                    .unwrap_or_else(|| json!("2025-06-18"));
                reply(
                    &out,
                    &id,
                    &json!({
                        "protocolVersion": version,
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "fake-gui", "version": "0" },
                    }),
                );
            }
            "ping" => reply(&out, &id, &json!({})),
            "tools/list" => {
                let names = [
                    "echo",
                    "whoami",
                    "slow",
                    "exit",
                    "crash",
                    "say_garbage",
                    "alloc",
                ];
                let list: Vec<Value> = names
                    .iter()
                    .map(|n| json!({ "name": n, "inputSchema": { "type": "object" } }))
                    .collect();
                reply(&out, &id, &json!({ "tools": list }));
            }
            "tools/call" => {
                let name = message
                    .pointer("/params/name")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let args = message
                    .pointer("/params/arguments")
                    .cloned()
                    .unwrap_or(Value::Null);
                match name {
                    "echo" | "say_garbage" => {
                        let text = args.get("text").and_then(Value::as_str).unwrap_or("");
                        reply(&out, &id, &text_result(text));
                    }
                    "whoami" => {
                        let info = json!({
                            "pid": pid,
                            "initialize_count": init_count.load(Ordering::SeqCst),
                            "initialized_count": initialized_count.load(Ordering::SeqCst),
                        });
                        reply(&out, &id, &text_result(&info.to_string()));
                    }
                    "slow" => {
                        let ms = args.get("ms").and_then(Value::as_u64).unwrap_or(1000);
                        let out = Arc::clone(&out);
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_millis(ms));
                            reply(&out, &id, &text_result("slow done"));
                        });
                    }
                    "exit" => {
                        let code = args.get("code").and_then(Value::as_i64).unwrap_or(0);
                        std::process::exit(i32::try_from(code).unwrap_or(1));
                    }
                    "alloc" => {
                        let mb = args.get("mb").and_then(Value::as_u64).unwrap_or(64);
                        let mut held: Vec<Vec<u8>> = Vec::new();
                        for _ in 0..mb {
                            held.push(vec![1_u8; 1 << 20]);
                        }
                        let total: usize = held.iter().map(Vec::len).sum();
                        reply(&out, &id, &text_result(&total.to_string()));
                    }
                    "crash" => {
                        let _ = std::process::Command::new("kill")
                            .args(["-KILL", &pid.to_string()])
                            .status();
                        std::thread::sleep(Duration::from_secs(10));
                    }
                    _ => send(
                        &out,
                        &json!({ "jsonrpc": "2.0", "id": id,
                                 "error": { "code": -32602, "message": "unknown tool" } }),
                    ),
                }
            }
            _ => send(
                &out,
                &json!({ "jsonrpc": "2.0", "id": id,
                         "error": { "code": -32601, "message": "method not found" } }),
            ),
        }
    }
    if std::env::var_os("FAKE_IGNORE_EOF").is_some() {
        let _ = writeln!(std::io::stderr(), "fake child: stdin closed, ignoring EOF");
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    }
}
