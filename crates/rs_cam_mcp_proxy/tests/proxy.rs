//! Integration tests: the proxy in front of the fake MCP child
//! (`rs_cam_mcp_fake_child`), driven over stdio as Claude Code drives it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// SAFETY: test code; a failed unwrap or panic is the test failure report.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const WAIT: Duration = Duration::from_secs(20);

struct Harness {
    proxy: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Value>,
    /// Messages read while a test waited for another message.
    seen: Vec<Value>,
    next_id: i64,
    log: PathBuf,
}

fn fake() -> &'static str {
    env!("CARGO_BIN_EXE_rs_cam_mcp_fake_child")
}

fn start(name: &str, child: &[&str], env: &[(&str, &str)]) -> Harness {
    start_with(
        name,
        &["--grace-ms", "500", "--init-timeout-ms", "5000"],
        child,
        env,
    )
}

fn start_with(name: &str, options: &[&str], child: &[&str], env: &[(&str, &str)]) -> Harness {
    let log = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("proxy_{name}.log"));
    let _ = std::fs::remove_file(&log);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rs_cam_mcp_proxy"));
    cmd.arg("--log")
        .arg(&log)
        .args(options)
        .arg("--")
        .args(child)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut proxy = cmd.spawn().unwrap();
    let stdin = proxy.stdin.take();
    let stdout = proxy.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { return };
            let value = serde_json::from_str(&line).unwrap_or_else(|_| json!({ "NOT_JSON": line }));
            if tx.send(value).is_err() {
                return;
            }
        }
    });
    Harness {
        proxy,
        stdin,
        rx,
        seen: Vec::new(),
        next_id: 1,
        log,
    }
}

impl Harness {
    fn send(&mut self, message: &Value) {
        self.send_raw(&message.to_string());
    }

    fn send_raw(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{line}").unwrap();
        stdin.flush().unwrap();
    }

    fn log_text(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// Waits for the first message that `pred` accepts.
    fn wait_for(&mut self, what: &str, pred: impl Fn(&Value) -> bool) -> Value {
        if let Some(pos) = self.seen.iter().position(&pred) {
            return self.seen.remove(pos);
        }
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(v) => {
                    assert!(
                        v.get("NOT_JSON").is_none(),
                        "proxy wrote a non-JSON line: {v}"
                    );
                    if pred(&v) {
                        return v;
                    }
                    self.seen.push(v);
                }
                Err(_) => panic!("no {what} within {WAIT:?}; log:\n{}", self.log_text()),
            }
        }
    }

    fn response(&mut self, id: &Value) -> Value {
        let id = id.clone();
        self.wait_for(&format!("response {id}"), move |v| {
            v.get("id") == Some(&id) && v.get("method").is_none()
        })
    }

    fn request(&mut self, method: &str, params: &Value) -> Value {
        let id = json!(self.next_id);
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        self.response(&id)
    }

    fn handshake(&mut self) -> Value {
        let init = self.request(
            "initialize",
            &json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "test", "version": "0" } }),
        );
        self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        init
    }

    fn call(&mut self, name: &str, args: &Value) -> Value {
        self.request("tools/call", &json!({ "name": name, "arguments": args }))
    }

    /// The JSON payload of a proxy tool result.
    fn proxy_tool(&mut self, name: &str, args: &Value) -> (Value, bool) {
        let response = self.call(name, args);
        let result = response
            .get("result")
            .unwrap_or_else(|| panic!("{response}"));
        let text = result
            .pointer("/content/0/text")
            .and_then(Value::as_str)
            .unwrap();
        let is_error = result.get("isError").and_then(Value::as_bool).unwrap();
        (serde_json::from_str(text).unwrap(), is_error)
    }

    fn whoami(&mut self) -> Value {
        let r = self.call("whoami", &json!({}));
        let text = r.pointer("/result/content/0/text").and_then(Value::as_str);
        serde_json::from_str(text.unwrap_or_else(|| panic!("{r}"))).unwrap()
    }

    fn close(&mut self) -> std::process::ExitStatus {
        self.stdin = None;
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(status) = self.proxy.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "proxy did not exit; log:\n{}",
                self.log_text()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.proxy.kill();
        let _ = self.proxy.wait();
    }
}

fn tool_names(response: &Value) -> Vec<String> {
    response
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{response}"))
        .iter()
        .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

fn error_message(response: &Value) -> String {
    response
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("expected an error: {response}"))
        .to_owned()
}

fn alive(pid: u64) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
        stat.rsplit(')')
            .next()
            .and_then(|r| r.split_whitespace().next())
            != Some("Z")
    })
}

fn wait_dead(pid: u64) {
    let deadline = Instant::now() + WAIT;
    while alive(pid) {
        assert!(Instant::now() < deadline, "process {pid} still runs");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn tools_list_merges_proxy_tools_and_calls_route_by_id() {
    let mut h = start("merge", &[fake()], &[]);
    let init = h.handshake();
    assert_eq!(
        init.pointer("/result/serverInfo/name"),
        Some(&json!("fake-gui"))
    );
    assert_eq!(
        init.pointer("/result/capabilities/tools/listChanged"),
        Some(&json!(true))
    );

    let names = tool_names(&h.request("tools/list", &json!({})));
    for expected in ["echo", "whoami", "gui_status", "gui_restart", "gui_stop"] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} missing in {names:?}"
        );
    }

    // A string id and two calls in flight at once: each answer keeps its id.
    h.send(
        &json!({ "jsonrpc": "2.0", "id": "slow-1", "method": "tools/call",
                    "params": { "name": "slow", "arguments": { "ms": 300 } } }),
    );
    h.send(
        &json!({ "jsonrpc": "2.0", "id": "echo-1", "method": "tools/call",
                    "params": { "name": "echo", "arguments": { "text": "hello" } } }),
    );
    let echo = h.response(&json!("echo-1"));
    assert_eq!(
        echo.pointer("/result/content/0/text"),
        Some(&json!("hello"))
    );
    let slow = h.response(&json!("slow-1"));
    assert_eq!(
        slow.pointer("/result/content/0/text"),
        Some(&json!("slow done"))
    );

    // A malformed client line and a non-JSON child line are skipped.
    h.send_raw("this is { not json");
    let garbage = h.call("say_garbage", &json!({ "text": "after garbage" }));
    assert_eq!(
        garbage.pointer("/result/content/0/text"),
        Some(&json!("after garbage"))
    );
    let ping = h.request("ping", &json!({}));
    assert_eq!(ping.get("result"), Some(&json!({})));

    let (status, is_error) = h.proxy_tool("gui_status", &json!({}));
    assert!(!is_error);
    assert_eq!(status["state"], json!("connected"), "{status}");
    assert_eq!(status["probe"]["ok"], json!(true), "{status}");
    assert!(status["last_handshake_ms"].is_number(), "{status}");
    assert!(
        status["binary"]["path"]
            .as_str()
            .unwrap()
            .ends_with("rs_cam_mcp_fake_child")
    );
}

#[test]
fn child_crash_fails_in_flight_requests_and_status_reports_the_kill() {
    let mut h = start("crash", &[fake()], &[]);
    h.handshake();
    h.send(
        &json!({ "jsonrpc": "2.0", "id": 900, "method": "tools/call",
                    "params": { "name": "slow", "arguments": { "ms": 30000 } } }),
    );
    let crash = h.call("crash", &json!({}));
    let slow = h.response(&json!(900));
    for r in [&crash, &slow] {
        let message = error_message(r);
        assert!(message.contains("GUI disconnected"), "{message}");
        assert!(message.contains("SIGKILL"), "{message}");
        assert!(message.contains("call gui_restart"), "{message}");
    }

    let (status, _) = h.proxy_tool("gui_status", &json!({}));
    assert_eq!(status["state"], json!("down"), "{status}");
    let exit = &status["last_exit"];
    assert_eq!(exit["signal"], json!(9), "{status}");
    assert_eq!(exit["reason"], json!("killed"), "{status}");
    assert!(
        !exit["oom_checks"].as_array().unwrap().is_empty(),
        "{status}"
    );
    assert_eq!(status["probe"]["ok"], json!(false), "{status}");

    // While no child runs, a GUI call fails at once and tools/list shows
    // only the proxy tools.
    let echo = h.call("echo", &json!({ "text": "x" }));
    assert!(error_message(&echo).contains("GUI disconnected"));
    let names = tool_names(&h.request("tools/list", &json!({})));
    assert_eq!(names, ["gui_status", "gui_restart", "gui_stop"]);
}

#[test]
fn gui_restart_replays_the_handshake_and_announces_the_tools() {
    let mut h = start("restart", &[fake()], &[]);
    h.handshake();
    let first = h.whoami();
    let exit = h.call("exit", &json!({ "code": 3 }));
    assert!(error_message(&exit).contains("code 3"));
    let (status, _) = h.proxy_tool("gui_status", &json!({}));
    assert_eq!(
        status["last_exit"]["reason"],
        json!("error_exit"),
        "{status}"
    );

    let (restarted, is_error) = h.proxy_tool("gui_restart", &json!({ "wait_ready_s": 15 }));
    assert!(!is_error, "{restarted}");
    assert_eq!(restarted["state"], json!("connected"), "{restarted}");
    assert_eq!(restarted["gui_tool_count"], json!(7), "{restarted}");
    assert_eq!(restarted["restarts"], json!(1), "{restarted}");
    h.wait_for("tools/list_changed", |v| {
        v.get("method") == Some(&json!("notifications/tools/list_changed"))
    });

    // The fake refuses tool calls before `notifications/initialized`, so
    // this answer proves the replay of both handshake messages.
    let second = h.whoami();
    assert_ne!(first["pid"], second["pid"]);
    assert_eq!(second["initialize_count"], json!(1));
    assert_eq!(second["initialized_count"], json!(1));

    // A restart of a live child stops the old process first.
    let (again, is_error) = h.proxy_tool("gui_restart", &json!({}));
    assert!(!is_error, "{again}");
    assert_eq!(again["last_exit"]["reason"], json!("stopped"), "{again}");
    wait_dead(second["pid"].as_u64().unwrap());
    assert_ne!(h.whoami()["pid"], second["pid"]);
}

#[test]
fn client_close_stops_the_child_and_the_proxy() {
    let mut h = start("close", &[fake()], &[]);
    h.handshake();
    let pid = h.whoami()["pid"].as_u64().unwrap();
    let status = h.close();
    assert!(status.success(), "{status}");
    wait_dead(pid);
}

#[test]
fn a_child_that_ignores_eof_gets_sigterm() {
    let mut h = start("sigterm", &[fake()], &[("FAKE_IGNORE_EOF", "1")]);
    h.handshake();
    let pid = h.whoami()["pid"].as_u64().unwrap();
    let (status, _) = h.proxy_tool("gui_stop", &json!({}));
    assert_eq!(status["state"], json!("down"), "{status}");
    assert_eq!(status["last_exit"]["reason"], json!("stopped"), "{status}");
    assert_eq!(status["last_exit"]["signal"], json!(15), "{status}");
    wait_dead(pid);
    assert!(h.log_text().contains("sending SIGTERM"));
}

#[test]
fn a_missing_child_still_gives_a_working_session() {
    let mut h = start("missing", &["/nonexistent/rs_cam_gui", "--mcp"], &[]);
    let init = h.handshake();
    assert_eq!(
        init.pointer("/result/serverInfo/name"),
        Some(&json!("rs_cam_mcp_proxy"))
    );
    assert_eq!(
        init.pointer("/result/capabilities/tools/listChanged"),
        Some(&json!(true))
    );
    let names = tool_names(&h.request("tools/list", &json!({})));
    assert_eq!(names, ["gui_status", "gui_restart", "gui_stop"]);
    let (status, _) = h.proxy_tool("gui_status", &json!({}));
    assert_eq!(status["state"], json!("down"), "{status}");
    assert!(
        status["last_spawn_error"]
            .as_str()
            .unwrap()
            .contains("nonexistent")
    );
    let echo = h.call("echo", &json!({}));
    assert!(error_message(&echo).contains("spawn failed"));
    let (restart, is_error) = h.proxy_tool("gui_restart", &json!({ "wait_ready_s": 2 }));
    assert!(is_error, "{restart}");
    assert!(restart["error"].as_str().unwrap().contains("cannot run"));
}

#[test]
fn a_slow_gui_handshake_completes_after_the_proxy_answered() {
    let mut h = start_with(
        "late",
        &["--grace-ms", "500", "--init-timeout-ms", "300"],
        &[fake()],
        &[("FAKE_INIT_DELAY_MS", "1500")],
    );
    let init = h.handshake();
    // The proxy answered itself because the GUI was slow.
    assert_eq!(
        init.pointer("/result/serverInfo/name"),
        Some(&json!("rs_cam_mcp_proxy"))
    );
    // A GUI call in this time waits in the queue and is then answered.
    h.send(
        &json!({ "jsonrpc": "2.0", "id": "queued", "method": "tools/call",
                    "params": { "name": "echo", "arguments": { "text": "late" } } }),
    );
    let names = tool_names(&h.request("tools/list", &json!({})));
    assert_eq!(names, ["gui_status", "gui_restart", "gui_stop"]);
    h.wait_for("tools/list_changed", |v| {
        v.get("method") == Some(&json!("notifications/tools/list_changed"))
    });
    let queued = h.response(&json!("queued"));
    assert_eq!(
        queued.pointer("/result/content/0/text"),
        Some(&json!("late"))
    );
    assert!(
        tool_names(&h.request("tools/list", &json!({})))
            .iter()
            .any(|n| n == "echo")
    );
}

/// A real cgroup OOM kill under `systemd-run --user --scope`, as the
/// operator config runs the GUI. The test returns early when the machine has
/// no user systemd (for example a container).
#[test]
fn an_oom_kill_in_a_systemd_scope_is_reported_as_oom() {
    let probe = Command::new("systemd-run")
        .args(["--user", "--scope", "-q", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !probe.is_ok_and(|s| s.success()) {
        return;
    }
    let mut h = start(
        "oom",
        &[
            "systemd-run",
            "--user",
            "--scope",
            "-q",
            "-p",
            "MemoryMax=64M",
            "-p",
            "MemorySwapMax=0",
            fake(),
        ],
        &[],
    );
    h.handshake();
    let (status, _) = h.proxy_tool("gui_status", &json!({}));
    let unit = status["child"]["systemd_unit"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(unit.ends_with(".scope"), "{status}");
    assert!(
        status["child"]["gui_comm"]
            .as_str()
            .unwrap()
            .starts_with("rs_cam_mcp_fake")
    );
    let alloc = h.call("alloc", &json!({ "mb": 512 }));
    assert!(error_message(&alloc).contains("OOM-killed"), "{alloc}");
    let (status, _) = h.proxy_tool("gui_status", &json!({}));
    assert_eq!(
        status["last_exit"]["reason"],
        json!("oom_killed"),
        "{status}"
    );
    // systemd keeps the failed scope as OOM evidence; the test removes it.
    let _ = Command::new("systemctl")
        .args(["--user", "reset-failed", &unit])
        .status();
}
