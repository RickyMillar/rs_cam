//! The proxy core: client and child message routing, the MCP handshake and
//! its replay, and the child lifecycle.
//!
//! Threads:
//!
//! - the main thread reads client stdin;
//! - one writer thread owns the proxy stdout (the client transport);
//! - each child has a stdin writer, a stdout reader, a stderr reader, a
//!   waiter that reaps it, and a short discovery thread;
//! - a proxy tool call and the client `initialize` run on their own thread.
//!
//! All writes go through unbounded channels, so a slow or hung GUI never
//! blocks the client reader. No code holds the state lock while it waits.
//!
//! Framing: one JSON-RPC 2.0 message per line. The proxy parses each line to
//! read `id` and `method`, and forwards the original text unless it must
//! change the message (the `initialize` and the first `tools/list` result).

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ChildStdin, Command, ExitCode, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::config::Config;
use crate::log::{Logger, unix_now};
use crate::{lock, procinfo, tools};

/// The id prefix of requests that the proxy itself sends to the child.
const INTERNAL_PREFIX: &str = "rs_cam_mcp_proxy:";
/// The JSON-RPC error code for "the GUI is not connected".
const DISCONNECTED_CODE: i64 = -32001;
const STDERR_TAIL_LINES: usize = 40;
/// The time that a late child `initialize` answer can take after the proxy
/// answered the client itself.
const LATE_HANDSHAKE_LIMIT: Duration = Duration::from_secs(120);
/// The shortest time between two automatic restarts.
const AUTO_RESTART_MIN_GAP: Duration = Duration::from_secs(30);
const PROXY_INSTRUCTIONS: &str = "This MCP server is rs_cam_mcp_proxy in front of the rs_cam GUI. \
The proxy tools gui_status, gui_restart and gui_stop report and control the GUI process. \
When a GUI tool fails with 'GUI disconnected', call gui_status for the reason and gui_restart \
to start the GUI again.";

/// A client request that waits in the queue until the child is ready.
struct Queued {
    line: String,
    request: Option<InFlight>,
}

/// A client request that the child has not answered yet.
struct InFlight {
    id: Value,
    method: String,
    /// True for a `tools/list` without a cursor: the proxy appends its tools.
    first_page: bool,
}

struct ChildSlot {
    generation: u64,
    pid: u32,
    gui_pid: u32,
    process: Arc<Mutex<Child>>,
    /// `None` after the proxy closed the child stdin.
    to_child: Option<Sender<String>>,
    spawned_unix: f64,
    spawned_at: Instant,
    cgroup: Option<String>,
    unit: Option<String>,
    /// The child answered the proxy `initialize`.
    init_answered: bool,
    /// The child also received `notifications/initialized`.
    ready: bool,
    /// Send `notifications/tools/list_changed` when the child becomes ready
    /// (the client saw only the proxy tools before).
    announce_on_ready: bool,
    /// Set when the proxy stops the child on purpose.
    stop_reason: Option<String>,
    in_flight: HashMap<String, InFlight>,
}

#[derive(Default)]
struct Inner {
    child: Option<ChildSlot>,
    next_generation: u64,
    /// The `params` of the client `initialize`.
    client_init: Option<Value>,
    /// The client `notifications/initialized` line.
    client_initialized: Option<String>,
    queue: VecDeque<Queued>,
    waiters: HashMap<String, (u64, Sender<Value>)>,
    restart_in_progress: bool,
    shutting_down: bool,
    restarts: u64,
    last_handshake_ms: Option<f64>,
    last_exit: Option<Value>,
    last_spawn_error: Option<String>,
    last_auto_restart: Option<Instant>,
    stderr_tail: VecDeque<String>,
}

impl Inner {
    fn state(&self) -> &'static str {
        match &self.child {
            Some(slot) if slot.ready => "connected",
            _ if self.restart_in_progress => "restarting",
            Some(_) => "starting",
            None => "down",
        }
    }

    fn slot(&mut self, generation: u64) -> Option<&mut ChildSlot> {
        self.child
            .as_mut()
            .filter(|slot| slot.generation == generation)
    }

    fn has_generation(&self, generation: u64) -> bool {
        self.child
            .as_ref()
            .is_some_and(|slot| slot.generation == generation)
    }

    /// The reason that a request cannot reach the GUI now.
    fn down_reason(&self) -> String {
        if let Some(summary) = self
            .last_exit
            .as_ref()
            .and_then(|e| e.get("summary"))
            .and_then(Value::as_str)
        {
            return summary.to_owned();
        }
        if let Some(err) = &self.last_spawn_error {
            return format!("spawn failed: {err}");
        }
        "no GUI process".to_owned()
    }
}

pub struct Proxy {
    cfg: Config,
    log: Logger,
    to_client: Sender<String>,
    inner: Mutex<Inner>,
    changed: Condvar,
    next_id: AtomicU64,
    started_at: Instant,
}

/// Runs the proxy until the client closes stdin.
pub fn run(cfg: Config) -> ExitCode {
    let log = match Logger::open(cfg.log_file.as_deref()) {
        Ok(log) => log,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "rs_cam_mcp_proxy: {e}");
            return ExitCode::from(2);
        }
    };
    let (to_client, client_rx) = mpsc::channel::<String>();
    let proxy = Arc::new(Proxy {
        cfg,
        log,
        to_client,
        inner: Mutex::new(Inner::default()),
        changed: Condvar::new(),
        next_id: AtomicU64::new(1),
        started_at: Instant::now(),
    });
    proxy
        .log
        .event(&format!("start; child command: {:?}", proxy.cfg.command));

    let writer = {
        let proxy = Arc::clone(&proxy);
        std::thread::Builder::new()
            .name("client-writer".to_owned())
            .spawn(move || proxy.client_writer(&client_rx))
    };

    if let Err(e) = proxy.spawn_child() {
        proxy.log.event(&format!("cannot start the GUI: {e}"));
    }

    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => proxy.on_client_line(&buf),
            Err(e) => {
                proxy.log.event(&format!("client stdin read error: {e}"));
                break;
            }
        }
    }
    proxy.log.event("client closed stdin; stopping the GUI");
    proxy.shutdown("stopped because the MCP client closed the session");
    // An empty line tells the writer to finish the queued output and stop.
    let _ = proxy.to_client.send(String::new());
    if let Ok(handle) = writer {
        let _ = handle.join();
    }
    ExitCode::SUCCESS
}

/// Removes the line end.
fn trim_line(bytes: &[u8]) -> &[u8] {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    bytes.strip_suffix(b"\r").unwrap_or(bytes)
}

fn id_key(id: &Value) -> String {
    id.to_string()
}

fn response(id: &Value, result: &Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn error_response(id: &Value, code: i64, message: &str, data: Option<&Value>) -> String {
    let mut error = json!({ "code": code, "message": message });
    if let (Some(data), Some(obj)) = (data, error.as_object_mut()) {
        obj.insert("data".to_owned(), data.clone());
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": error }).to_string()
}

fn disconnected_message(reason: &str) -> String {
    format!("GUI disconnected ({reason}); call gui_restart")
}

/// The child `initialize` result with `tools.listChanged = true` and the
/// proxy instructions when the child gives none.
fn merged_initialize(mut result: Value) -> Value {
    if let Some(obj) = result.as_object_mut() {
        let caps = obj.entry("capabilities").or_insert_with(|| json!({}));
        if let Some(caps) = caps.as_object_mut() {
            let tools = caps.entry("tools").or_insert_with(|| json!({}));
            if let Some(tools) = tools.as_object_mut() {
                tools.insert("listChanged".to_owned(), Value::Bool(true));
            }
        }
        obj.entry("instructions")
            .or_insert_with(|| Value::String(PROXY_INSTRUCTIONS.to_owned()));
    }
    result
}

/// The proxy `initialize` result when no GUI answers.
fn own_initialize(params: &Value) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("2025-06-18");
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": true } },
        "serverInfo": { "name": "rs_cam_mcp_proxy", "version": env!("CARGO_PKG_VERSION") },
        "instructions": format!("{PROXY_INSTRUCTIONS} The GUI did not answer the handshake yet."),
    })
}

fn proxy_tools_only() -> Value {
    json!({ "tools": tools::definitions() })
}

/// Appends the proxy tools to a `tools/list` response of the last page.
fn append_proxy_tools(mut message: Value) -> Value {
    if let Some(result) = message.get_mut("result")
        && result.get("nextCursor").is_none_or(Value::is_null)
        && let Some(Value::Array(list)) = result.get_mut("tools")
    {
        list.retain(|t| {
            !t.get("name")
                .and_then(Value::as_str)
                .is_some_and(tools::is_proxy_tool)
        });
        list.extend(tools::definitions());
    }
    message
}

impl Proxy {
    fn state(&self) -> MutexGuard<'_, Inner> {
        lock(&self.inner)
    }

    fn send_client(&self, line: String) {
        let _ = self.to_client.send(line);
    }

    /// Waits until `done` is true or the deadline passes.
    fn wait_until(&self, deadline: Instant, done: impl Fn(&Inner) -> bool) -> bool {
        let mut inner = self.state();
        loop {
            if done(&inner) {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            inner = self
                .changed
                .wait_timeout(inner, deadline - now)
                .map_or_else(|e| e.into_inner().0, |(guard, _)| guard);
        }
    }

    fn client_writer(self: &Arc<Self>, rx: &Receiver<String>) {
        let stdout = std::io::stdout();
        for line in rx {
            if line.is_empty() {
                return;
            }
            let mut out = stdout.lock();
            let written = out
                .write_all(line.as_bytes())
                .and_then(|()| out.write_all(b"\n"))
                .and_then(|()| out.flush());
            if let Err(e) = written {
                drop(out);
                self.log
                    .event(&format!("client stdout write failed ({e}); stopping"));
                self.shutdown("stopped because the MCP client went away");
                std::process::exit(0);
            }
        }
    }

    // ----- client messages -------------------------------------------------

    fn on_client_line(self: &Arc<Self>, bytes: &[u8]) {
        let bytes = trim_line(bytes);
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return;
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            self.log
                .event("client sent a line that is not UTF-8; skipped");
            return;
        };
        let message: Value = match serde_json::from_str(text) {
            Ok(Value::Object(map)) => Value::Object(map),
            Ok(_) => {
                self.log
                    .event("client sent a JSON value that is not an object; skipped");
                return;
            }
            Err(e) => {
                self.log
                    .event(&format!("client sent a malformed line ({e}); skipped"));
                return;
            }
        };
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").filter(|id| !id.is_null());
        match (method, id) {
            (Some(method), Some(id)) => self.on_client_request(method, id, &message, text),
            (Some(method), None) => self.on_client_notification(method, &message, text),
            (None, Some(_)) => self.forward_client_response(text),
            (None, None) => self
                .log
                .event("client sent a message without method or id; skipped"),
        }
    }

    fn on_client_request(self: &Arc<Self>, method: &str, id: &Value, message: &Value, text: &str) {
        let params = message.get("params");
        match method {
            "initialize" => {
                let params = params.cloned().unwrap_or_else(|| json!({}));
                self.state().client_init = Some(params.clone());
                let proxy = Arc::clone(self);
                let id = id.clone();
                self.spawn_thread("client-initialize", move || {
                    proxy.client_initialize(&id, &params);
                });
            }
            "ping" => self.send_client(response(id, &json!({}))),
            "tools/call"
                if params
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str)
                    .is_some_and(tools::is_proxy_tool) =>
            {
                let proxy = Arc::clone(self);
                let id = id.clone();
                let params = params.cloned().unwrap_or(Value::Null);
                self.spawn_thread("proxy-tool", move || proxy.run_proxy_tool(&id, &params));
            }
            _ => {
                let first_page =
                    method == "tools/list" && params.and_then(|p| p.get("cursor")).is_none();
                self.route_request(text, id, method, first_page);
            }
        }
    }

    /// Sends a client request to the child, queues it, or answers it.
    fn route_request(&self, text: &str, id: &Value, method: &str, first_page: bool) {
        let request = InFlight {
            id: id.clone(),
            method: method.to_owned(),
            first_page,
        };
        let mut inner = self.state();
        let state = inner.state();
        match state {
            "connected" => {
                if let Some(slot) = inner.child.as_mut() {
                    Self::send_to_slot(slot, text.to_owned(), Some(request));
                }
            }
            _ if method == "tools/list" => {
                // The client must see the proxy tools at all times. The
                // proxy sends `tools/list_changed` when the GUI is ready.
                if let Some(slot) = inner.child.as_mut() {
                    slot.announce_on_ready = true;
                }
                self.send_client(response(id, &proxy_tools_only()));
            }
            "starting" | "restarting" => inner.queue.push_back(Queued {
                line: text.to_owned(),
                request: Some(request),
            }),
            _ => {
                let reason = inner.down_reason();
                drop(inner);
                self.send_client(error_response(
                    id,
                    DISCONNECTED_CODE,
                    &disconnected_message(&reason),
                    None,
                ));
            }
        }
    }

    fn send_to_slot(slot: &mut ChildSlot, line: String, request: Option<InFlight>) {
        if let Some(request) = request {
            slot.in_flight.insert(id_key(&request.id), request);
        }
        if let Some(tx) = &slot.to_child {
            let _ = tx.send(line);
        }
    }

    fn on_client_notification(&self, method: &str, message: &Value, text: &str) {
        let mut inner = self.state();
        match method {
            "notifications/initialized" => {
                inner.client_initialized = Some(text.to_owned());
                let complete = inner
                    .child
                    .as_ref()
                    .is_some_and(|s| s.init_answered && !s.ready);
                if complete {
                    self.mark_ready(&mut inner, text);
                }
            }
            _ => {
                if method == "notifications/cancelled"
                    && let Some(request_id) = message.get("params").and_then(|p| p.get("requestId"))
                    && let Some(slot) = inner.child.as_mut()
                {
                    slot.in_flight.remove(&id_key(request_id));
                }
                match inner.state() {
                    "connected" => {
                        if let Some(slot) = inner.child.as_mut() {
                            Self::send_to_slot(slot, text.to_owned(), None);
                        }
                    }
                    "starting" | "restarting" => inner.queue.push_back(Queued {
                        line: text.to_owned(),
                        request: None,
                    }),
                    _ => {}
                }
            }
        }
    }

    /// A client response to a child-initiated request.
    fn forward_client_response(&self, text: &str) {
        let mut inner = self.state();
        if let Some(slot) = inner.child.as_mut() {
            Self::send_to_slot(slot, text.to_owned(), None);
        }
    }

    /// Sends `notifications/initialized` to the child, marks it ready and
    /// sends the queued requests. The caller holds the lock.
    fn mark_ready(&self, inner: &mut Inner, initialized_line: &str) {
        let queued: Vec<Queued> = inner.queue.drain(..).collect();
        let Some(slot) = inner.child.as_mut() else {
            return;
        };
        if let Some(tx) = &slot.to_child {
            let _ = tx.send(initialized_line.to_owned());
        }
        slot.ready = true;
        let announce = std::mem::take(&mut slot.announce_on_ready);
        for item in queued {
            Self::send_to_slot(slot, item.line, item.request);
        }
        if announce {
            self.send_client(
                json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" })
                    .to_string(),
            );
        }
        self.log
            .event(&format!("GUI ready (generation {})", slot.generation));
        self.changed.notify_all();
    }

    /// Records the child `initialize` answer. The child becomes ready now if
    /// the client already sent `notifications/initialized`.
    fn on_child_initialized(&self, generation: u64, latency: Duration) {
        let mut inner = self.state();
        inner.last_handshake_ms = Some(latency.as_secs_f64() * 1000.0);
        let Some(slot) = inner.slot(generation) else {
            return;
        };
        slot.init_answered = true;
        if let Some(line) = inner.client_initialized.clone() {
            self.mark_ready(&mut inner, &line);
        }
    }

    /// Answers the client `initialize`. The proxy forwards it to the child
    /// and merges the answer. When the child does not answer in time, the
    /// proxy answers itself and completes the child handshake later.
    fn client_initialize(&self, id: &Value, params: &Value) {
        let generation = self.state().child.as_ref().map(|s| s.generation);
        let Some(generation) = generation else {
            self.send_client(response(id, &own_initialize(params)));
            return;
        };
        let started = Instant::now();
        let rx = match self.begin_internal(generation, "initialize", Some(params)) {
            Ok(rx) => rx,
            Err(e) => {
                self.log
                    .event(&format!("initialize not sent to the GUI: {e}"));
                self.send_client(response(id, &own_initialize(params)));
                return;
            }
        };
        match rx.recv_timeout(self.cfg.init_timeout) {
            Ok(answer) => {
                if let Some(result) = answer
                    .get("result")
                    .filter(|_| answer.get("error").is_none())
                {
                    self.on_child_initialized(generation, started.elapsed());
                    self.send_client(response(id, &merged_initialize(result.clone())));
                } else {
                    self.log
                        .event(&format!("the GUI refused initialize: {answer}"));
                    self.send_client(response(id, &own_initialize(params)));
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.send_client(response(id, &own_initialize(params)));
            }
            Err(RecvTimeoutError::Timeout) => {
                self.log.event(&format!(
                    "the GUI did not answer initialize within {:?}; the proxy answers itself",
                    self.cfg.init_timeout
                ));
                if let Some(slot) = self.state().slot(generation) {
                    slot.announce_on_ready = true;
                }
                self.send_client(response(id, &own_initialize(params)));
                match rx.recv_timeout(LATE_HANDSHAKE_LIMIT) {
                    Ok(answer) if answer.get("error").is_none() => {
                        self.on_child_initialized(generation, started.elapsed());
                    }
                    // When the child is gone, `on_child_exit` or a restart
                    // owns the queue; it can hold requests for a new child.
                    _ if self.state().has_generation(generation) => {
                        self.log.event("the GUI did not complete the handshake");
                        self.fail_queue("the GUI did not complete the MCP handshake");
                    }
                    _ => {}
                }
            }
        }
    }

    fn fail_queue(&self, reason: &str) {
        let queued: Vec<Queued> = self.state().queue.drain(..).collect();
        for item in queued {
            if let Some(request) = item.request {
                self.send_client(error_response(
                    &request.id,
                    DISCONNECTED_CODE,
                    &disconnected_message(reason),
                    None,
                ));
            }
        }
    }

    // ----- proxy-originated requests --------------------------------------

    fn begin_internal(
        &self,
        generation: u64,
        method: &str,
        params: Option<&Value>,
    ) -> Result<Receiver<Value>, String> {
        let id = format!(
            "{INTERNAL_PREFIX}{}",
            self.next_id.fetch_add(1, Ordering::Relaxed)
        );
        let mut message = json!({ "jsonrpc": "2.0", "id": id, "method": method });
        if let (Some(params), Some(obj)) = (params, message.as_object_mut()) {
            obj.insert("params".to_owned(), params.clone());
        }
        let (tx, rx) = mpsc::channel();
        let mut inner = self.state();
        let sender = inner
            .slot(generation)
            .ok_or_else(|| "the GUI process is gone".to_owned())?
            .to_child
            .clone()
            .ok_or_else(|| "the GUI stdin is closed".to_owned())?;
        inner.waiters.insert(id, (generation, tx));
        sender
            .send(message.to_string())
            .map_err(|e| format!("the GUI stdin writer stopped: {e}"))?;
        Ok(rx)
    }

    /// Sends a proxy request to the child and waits for the full answer.
    fn internal_request(
        &self,
        generation: u64,
        method: &str,
        params: Option<&Value>,
        timeout: Duration,
    ) -> Result<Value, String> {
        let rx = self.begin_internal(generation, method, params)?;
        rx.recv_timeout(timeout).map_err(|e| match e {
            RecvTimeoutError::Timeout => format!(
                "the GUI gave no answer to {method} within {:.1} s",
                timeout.as_secs_f64()
            ),
            RecvTimeoutError::Disconnected => format!("the GUI exited before it answered {method}"),
        })
    }

    // ----- child messages --------------------------------------------------

    fn on_child_line(&self, generation: u64, bytes: &[u8]) {
        let bytes = trim_line(bytes);
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return;
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            self.log
                .event("the GUI sent a line that is not UTF-8; skipped");
            return;
        };
        let message: Value = match serde_json::from_str(text) {
            Ok(v @ Value::Object(_)) => v,
            Ok(_) | Err(_) => {
                let head: String = text.chars().take(200).collect();
                self.log.event(&format!(
                    "the GUI sent a line that is not a JSON-RPC message; skipped: {head}"
                ));
                return;
            }
        };
        let id = message.get("id").filter(|id| !id.is_null());
        let Some(id) = id.filter(|_| message.get("method").is_none()) else {
            // A request or a notification from the GUI goes to the client.
            self.send_client(text.to_owned());
            return;
        };
        if let Some(key) = id.as_str().filter(|s| s.starts_with(INTERNAL_PREFIX)) {
            let waiter = self.state().waiters.remove(key);
            if let Some((_, tx)) = waiter {
                let _ = tx.send(message);
            }
            return;
        }
        let request = self
            .state()
            .slot(generation)
            .and_then(|slot| slot.in_flight.remove(&id_key(id)));
        match request {
            Some(request) if request.method == "tools/list" && request.first_page => {
                self.send_client(append_proxy_tools(message).to_string());
            }
            Some(_) => self.send_client(text.to_owned()),
            None => self.log.event(&format!(
                "the GUI answered an unknown request id {id}; dropped"
            )),
        }
    }

    // ----- child lifecycle -------------------------------------------------

    fn spawn_thread(&self, name: &str, body: impl FnOnce() + Send + 'static) {
        if let Err(e) = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(body)
        {
            self.log.event(&format!("cannot start thread {name}: {e}"));
        }
    }

    /// Starts the child and its threads. Returns the generation.
    fn spawn_child(self: &Arc<Self>) -> Result<u64, String> {
        let (program, args) = self
            .cfg
            .command
            .split_first()
            .ok_or_else(|| "the child command is empty".to_owned())?;
        let spawned = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(e) => {
                let message = format!("cannot run `{program}`: {e}");
                self.state().last_spawn_error = Some(message.clone());
                self.changed.notify_all();
                return Err(message);
            }
        };
        let pid = child.id();
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("the child pipes are missing".to_owned());
        };
        let process = Arc::new(Mutex::new(child));
        let (tx, rx) = mpsc::channel::<String>();
        let generation = {
            let mut inner = self.state();
            inner.next_generation += 1;
            let generation = inner.next_generation;
            inner.last_spawn_error = None;
            inner.stderr_tail.clear();
            inner.child = Some(ChildSlot {
                generation,
                pid,
                gui_pid: pid,
                process: Arc::clone(&process),
                to_child: Some(tx),
                spawned_unix: unix_now(),
                spawned_at: Instant::now(),
                cgroup: None,
                unit: None,
                init_answered: false,
                ready: false,
                announce_on_ready: false,
                stop_reason: None,
                in_flight: HashMap::new(),
            });
            generation
        };
        self.changed.notify_all();
        self.log
            .event(&format!("GUI started: pid {pid}, generation {generation}"));

        self.spawn_thread("child-writer", move || child_writer(stdin, &rx));
        let proxy = Arc::clone(self);
        self.spawn_thread("child-reader", move || {
            let mut reader = BufReader::new(stdout);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => proxy.on_child_line(generation, &buf),
                    Err(e) => {
                        proxy.log.event(&format!("GUI stdout read error: {e}"));
                        break;
                    }
                }
            }
            proxy.on_child_stdout_closed(generation);
        });
        let proxy = Arc::clone(self);
        self.spawn_thread("child-stderr", move || proxy.child_stderr(stderr));
        let proxy = Arc::clone(self);
        self.spawn_thread("child-waiter", move || {
            proxy.child_waiter(generation, &process);
        });
        let proxy = Arc::clone(self);
        self.spawn_thread("child-discovery", move || proxy.discover(generation, pid));
        Ok(generation)
    }

    fn child_stderr(&self, stderr: impl Read) {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => return,
                Ok(_) => {
                    let line = String::from_utf8_lossy(trim_line(&buf)).into_owned();
                    self.log.child_stderr(&line);
                    let mut inner = self.state();
                    if inner.stderr_tail.len() >= STDERR_TAIL_LINES {
                        inner.stderr_tail.pop_front();
                    }
                    inner.stderr_tail.push_back(line);
                }
            }
        }
    }

    /// Finds the GUI pid and its cgroup while the launcher settles.
    fn discover(&self, generation: u64, pid: u32) {
        for attempt in 0..50 {
            let gui = procinfo::gui_pid(pid);
            let cgroup = procinfo::cgroup_path(gui);
            let unit = cgroup.as_deref().and_then(procinfo::unit_of);
            let settled = unit.as_deref().is_some_and(|u| u.ends_with(".scope"))
                || (attempt >= 10 && cgroup.is_some());
            {
                let mut inner = self.state();
                let Some(slot) = inner.slot(generation) else {
                    return;
                };
                slot.gui_pid = gui;
                if cgroup.is_some() {
                    slot.cgroup = cgroup;
                    slot.unit = unit;
                }
            }
            if settled {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn child_waiter(self: &Arc<Self>, generation: u64, process: &Mutex<Child>) {
        loop {
            let polled = lock(process).try_wait();
            match polled {
                Ok(Some(status)) => return self.on_child_exit(generation, Some(status)),
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => {
                    self.log.event(&format!("cannot wait for the GUI: {e}"));
                    return self.on_child_exit(generation, None);
                }
            }
        }
    }

    /// The child closed stdout. It can no longer answer, so the proxy waits
    /// for its exit and kills it if it does not exit.
    fn on_child_stdout_closed(&self, generation: u64) {
        let deadline = Instant::now() + self.cfg.grace;
        if !self.wait_until(deadline, |inner| !inner.has_generation(generation)) {
            self.log
                .event("the GUI closed stdout but did not exit; killing it");
            let process = self
                .state()
                .slot(generation)
                .map(|slot| Arc::clone(&slot.process));
            if let Some(process) = process {
                let _ = lock(&process).kill();
            }
        }
    }

    fn on_child_exit(self: &Arc<Self>, generation: u64, status: Option<ExitStatus>) {
        let Some((pid, gui_pid, cgroup, unit, spawned_unix, uptime, stop_reason)) =
            self.state().slot(generation).map(|slot| {
                (
                    slot.pid,
                    slot.gui_pid,
                    slot.cgroup.clone(),
                    slot.unit.clone(),
                    slot.spawned_unix,
                    slot.spawned_at.elapsed().as_secs_f64(),
                    slot.stop_reason.clone(),
                )
            })
        else {
            return;
        };
        let record = exit_record(
            status,
            &ExitFacts {
                pid,
                gui_pid,
                cgroup,
                unit,
                spawned_unix,
                uptime,
                stop_reason: stop_reason.clone(),
            },
        );
        let summary = record
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("the GUI exited")
            .to_owned();
        self.log.event(&format!("GUI exited: {summary}"));
        for check in record
            .get("oom_checks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            self.log
                .event(&format!("  OOM check: {}", check.as_str().unwrap_or("")));
        }

        let mut inner = self.state();
        let Some(slot) = inner.child.take_if(|slot| slot.generation == generation) else {
            return;
        };
        inner.last_exit = Some(record);
        inner.waiters.retain(|_, (g, _)| *g != generation);
        let mut failed: Vec<Value> = slot.in_flight.into_values().map(|r| r.id).collect();
        if !inner.restart_in_progress {
            failed.extend(
                inner
                    .queue
                    .drain(..)
                    .filter_map(|q| q.request.map(|r| r.id)),
            );
        }
        let auto = self.cfg.auto_restart
            && stop_reason.is_none()
            && !inner.shutting_down
            && !inner.restart_in_progress
            && inner
                .last_auto_restart
                .is_none_or(|t| t.elapsed() >= AUTO_RESTART_MIN_GAP);
        if auto {
            inner.last_auto_restart = Some(Instant::now());
        }
        drop(inner);
        self.changed.notify_all();
        let message = disconnected_message(&summary);
        for id in failed {
            self.send_client(error_response(&id, DISCONNECTED_CODE, &message, None));
        }
        if auto {
            self.log.event("--auto-restart: starting the GUI again");
            // The restart runs on the waiter thread of the dead child. The
            // thread holds no lock here, and the next child has its own waiter.
            let wait = Duration::from_secs_f64(tools::DEFAULT_WAIT_READY_S);
            match self.restart(wait) {
                Ok(_) => self.log.event("--auto-restart: the GUI is ready"),
                Err(e) => self.log.event(&format!("--auto-restart failed: {e}")),
            }
        }
    }

    /// Stops the current child: close stdin, wait, SIGTERM, wait, SIGKILL.
    fn stop_child(&self, reason: &str) {
        let target = self.state().child.as_mut().map(|slot| {
            slot.stop_reason = Some(reason.to_owned());
            slot.to_child = None;
            (
                slot.generation,
                slot.pid,
                slot.gui_pid,
                Arc::clone(&slot.process),
            )
        });
        let Some((generation, pid, gui_pid, process)) = target else {
            return;
        };
        self.log
            .event(&format!("stopping the GUI (pid {pid}): {reason}"));
        let gone = |inner: &Inner| !inner.has_generation(generation);
        if self.wait_until(Instant::now() + self.cfg.grace, gone) {
            return;
        }
        self.log
            .event("the GUI did not exit after stdin closed; sending SIGTERM");
        for target in pids(pid, gui_pid) {
            if let Err(e) = procinfo::send_signal(target, "TERM") {
                self.log.event(&format!("SIGTERM to {target}: {e}"));
            }
        }
        if self.wait_until(Instant::now() + self.cfg.grace, gone) {
            return;
        }
        self.log
            .event("the GUI did not exit after SIGTERM; sending SIGKILL");
        let _ = lock(&process).kill();
        if gui_pid != pid && procinfo::is_alive(gui_pid) {
            let _ = procinfo::send_signal(gui_pid, "KILL");
        }
        if !self.wait_until(Instant::now() + Duration::from_secs(5), gone) {
            self.log.event("the GUI is still not reaped after SIGKILL");
        }
    }

    fn shutdown(&self, reason: &str) {
        {
            let mut inner = self.state();
            if inner.shutting_down {
                return;
            }
            inner.shutting_down = true;
        }
        self.stop_child(reason);
    }

    // ----- proxy tools -----------------------------------------------------

    fn run_proxy_tool(self: &Arc<Self>, id: &Value, params: &Value) {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        let args = params.get("arguments");
        let (payload, is_error) = match name {
            tools::GUI_STATUS => {
                let timeout = tools::seconds_arg(args, "probe_timeout_s")
                    .map_or(self.cfg.probe_timeout, Duration::from_secs_f64);
                (self.status(Some(timeout)), false)
            }
            tools::GUI_RESTART => {
                let wait =
                    tools::seconds_arg(args, "wait_ready_s").unwrap_or(tools::DEFAULT_WAIT_READY_S);
                match self.restart(Duration::from_secs_f64(wait)) {
                    Ok(payload) => (payload, false),
                    Err(payload) => (payload, true),
                }
            }
            tools::GUI_STOP => {
                self.stop_child("stopped by gui_stop");
                (self.status(None), false)
            }
            _ => (
                json!({ "error": format!("unknown proxy tool {name}") }),
                true,
            ),
        };
        self.send_client(response(id, &tools::call_result(&payload, is_error)));
    }

    /// Stops the child, starts a new one, replays the handshake and waits
    /// until the new child answers `tools/list`.
    fn restart(self: &Arc<Self>, wait: Duration) -> Result<Value, Value> {
        {
            let mut inner = self.state();
            if inner.restart_in_progress {
                return Err(json!({ "error": "a restart is already running" }));
            }
            if inner.shutting_down {
                return Err(json!({ "error": "the proxy is shutting down" }));
            }
            inner.restart_in_progress = true;
        }
        self.changed.notify_all();
        let deadline = Instant::now() + wait;
        self.stop_child("stopped by gui_restart");
        let outcome = self.start_and_handshake(deadline);
        {
            let mut inner = self.state();
            inner.restart_in_progress = false;
            inner.restarts += 1;
        }
        self.changed.notify_all();
        match outcome {
            Ok(tool_count) => {
                self.send_client(
                    json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" })
                        .to_string(),
                );
                let mut status = self.status(None);
                if let Some(obj) = status.as_object_mut() {
                    obj.insert("gui_tool_count".to_owned(), json!(tool_count));
                }
                Ok(status)
            }
            Err(error) => {
                self.fail_queue(&error);
                let tail: Vec<String> = self.state().stderr_tail.iter().cloned().collect();
                Err(json!({
                    "error": error,
                    "stderr_tail": tail,
                    "status": self.status(None),
                }))
            }
        }
    }

    fn start_and_handshake(self: &Arc<Self>, deadline: Instant) -> Result<usize, String> {
        let generation = self.spawn_child()?;
        let remaining = || deadline.saturating_duration_since(Instant::now());
        let params = self.state().client_init.clone();
        let Some(params) = params else {
            // No client handshake to replay: the client initialize starts it.
            return Ok(0);
        };
        let started = Instant::now();
        let answer = self.internal_request(generation, "initialize", Some(&params), remaining())?;
        if let Some(error) = answer.get("error") {
            return Err(format!("the GUI refused initialize: {error}"));
        }
        self.on_child_initialized(generation, started.elapsed());
        let answer = self.internal_request(generation, "tools/list", None, remaining())?;
        if let Some(error) = answer.get("error") {
            return Err(format!("the GUI refused tools/list: {error}"));
        }
        Ok(answer
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(Value::as_array)
            .map_or(0, Vec::len))
    }

    /// The `gui_status` payload. With a timeout, it also probes the child.
    fn status(&self, probe_timeout: Option<Duration>) -> Value {
        let (mut status, probe_target) = {
            let mut inner = self.state();
            let state = inner.state();
            let child = inner.child.as_mut().map(|slot| {
                if procinfo::is_alive(slot.pid) {
                    slot.gui_pid = procinfo::gui_pid(slot.pid);
                    if let Some(cgroup) = procinfo::cgroup_path(slot.gui_pid) {
                        slot.unit = procinfo::unit_of(&cgroup);
                        slot.cgroup = Some(cgroup);
                    }
                }
                json!({
                    "generation": slot.generation,
                    "pid": slot.pid,
                    "gui_pid": slot.gui_pid,
                    "gui_comm": procinfo::comm(slot.gui_pid),
                    "started_unix": slot.spawned_unix,
                    "uptime_s": slot.spawned_at.elapsed().as_secs_f64(),
                    "cgroup": slot.cgroup,
                    "systemd_unit": slot.unit,
                    "handshake_complete": slot.ready,
                    "in_flight": slot.in_flight.len(),
                })
            });
            let probe_target = inner
                .child
                .as_ref()
                .map(|slot| (slot.generation, slot.ready, slot.gui_pid, slot.spawned_unix));
            let status = json!({
                "state": state,
                "child": child,
                "command": self.cfg.command,
                "binary": Value::Null,
                "probe": Value::Null,
                "last_exit": inner.last_exit,
                "last_spawn_error": inner.last_spawn_error,
                "restarts": inner.restarts,
                "last_handshake_ms": inner.last_handshake_ms,
                "queued": inner.queue.len(),
                "auto_restart": self.cfg.auto_restart,
                "stderr_tail": inner.stderr_tail.iter().rev().take(10).rev().collect::<Vec<_>>(),
                "proxy": {
                    "pid": std::process::id(),
                    "version": env!("CARGO_PKG_VERSION"),
                    "uptime_s": self.started_at.elapsed().as_secs_f64(),
                },
            });
            (status, probe_target)
        };
        let binary = self.binary_info(probe_target.map(|(_, _, gui, start)| (gui, start)));
        let probe = match (probe_target, probe_timeout) {
            (_, None) => Value::Null,
            (None, Some(_)) => json!({ "ok": false, "detail": "no GUI process" }),
            (Some((_, false, _, _)), Some(_)) => {
                json!({ "ok": false, "detail": "skipped: the MCP handshake is not complete" })
            }
            (Some((generation, true, _, _)), Some(timeout)) => self.probe(generation, timeout),
        };
        if let Some(obj) = status.as_object_mut() {
            obj.insert("binary".to_owned(), binary);
            obj.insert("probe".to_owned(), probe);
        }
        status
    }

    /// A live `ping` to the child. Any answer, also an error, proves that
    /// the GUI MCP server reads and answers.
    fn probe(&self, generation: u64, timeout: Duration) -> Value {
        let started = Instant::now();
        match self.internal_request(generation, "ping", None, timeout) {
            Ok(answer) => json!({
                "ok": true,
                "method": "ping",
                "latency_ms": started.elapsed().as_secs_f64() * 1000.0,
                "answered_with_error": answer.get("error"),
            }),
            Err(detail) => json!({ "ok": false, "method": "ping", "detail": detail }),
        }
    }

    fn binary_info(&self, running: Option<(u32, f64)>) -> Value {
        let exe = running.and_then(|(gui_pid, _)| procinfo::exe(gui_pid));
        let path = self
            .cfg
            .binary
            .clone()
            .or_else(|| exe.as_ref().map(|(p, _)| p.clone()))
            .or_else(|| {
                self.cfg
                    .command
                    .iter()
                    .map(std::path::PathBuf::from)
                    .find(|p| p.is_file())
            });
        let Some(path) = path else {
            return Value::Null;
        };
        let mtime = procinfo::mtime_unix(&path);
        let newer = match (mtime, running) {
            (Some(mtime), Some((_, started))) => Some(mtime > started),
            _ => None,
        };
        json!({
            "path": path.display().to_string(),
            "mtime_unix": mtime,
            "newer_than_process": newer,
            "running_exe_replaced": exe.map(|(_, deleted)| deleted),
        })
    }
}

fn pids(pid: u32, gui_pid: u32) -> Vec<u32> {
    if gui_pid == pid || !procinfo::is_alive(gui_pid) {
        vec![pid]
    } else {
        vec![pid, gui_pid]
    }
}

fn child_writer(mut stdin: ChildStdin, rx: &Receiver<String>) {
    for line in rx {
        let written = stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.write_all(b"\n"))
            .and_then(|()| stdin.flush());
        if written.is_err() {
            return;
        }
    }
    // The channel closed: dropping `stdin` here closes the child stdin.
}

struct ExitFacts {
    pid: u32,
    gui_pid: u32,
    cgroup: Option<String>,
    unit: Option<String>,
    spawned_unix: f64,
    uptime: f64,
    stop_reason: Option<String>,
}

/// Classifies an exit: `stopped`, `normal`, `error_exit`, `killed` or
/// `oom_killed`. A launcher that reports exit code 128+n reports signal n.
fn exit_record(status: Option<ExitStatus>, facts: &ExitFacts) -> Value {
    let code = status.and_then(|s| s.code());
    let signal = status.and_then(|s| s.signal());
    let wrapped_signal = code.filter(|c| (129..=192).contains(c)).map(|c| c - 128);
    let effective_signal = signal.or(wrapped_signal);
    let mut oom_checks = Vec::new();
    let (reason, summary) = if let Some(stop) = &facts.stop_reason {
        ("stopped", stop.clone())
    } else if let Some(sig) = effective_signal {
        let name = procinfo::signal_name(sig);
        let via = if signal.is_none() {
            " (exit code 128+n from a launcher)"
        } else {
            ""
        };
        if sig == 9 {
            // `since` rounds down, so the journal window includes the spawn.
            let since = facts.spawned_unix.max(0.0).floor();
            let since = since as u64;
            let (oom, notes) = procinfo::oom_evidence(
                facts.cgroup.as_deref(),
                facts.unit.as_deref(),
                since,
                facts.gui_pid,
            );
            oom_checks = notes;
            if oom {
                ("oom_killed", format!("OOM-killed: {name}{via}"))
            } else {
                (
                    "killed",
                    format!("killed by {name}{via}; OOM not confirmed"),
                )
            }
        } else {
            ("killed", format!("killed by {name} ({sig}){via}"))
        }
    } else if code == Some(0) {
        ("normal", "the GUI exited normally (code 0)".to_owned())
    } else if let Some(code) = code {
        ("error_exit", format!("the GUI exited with code {code}"))
    } else {
        ("unknown", "the GUI exit status is unknown".to_owned())
    };
    json!({
        "reason": reason,
        "summary": summary,
        "code": code,
        "signal": signal,
        "signal_name": effective_signal.map(procinfo::signal_name),
        "oom_checks": oom_checks,
        "pid": facts.pid,
        "gui_pid": facts.gui_pid,
        "systemd_unit": facts.unit,
        "exited_unix": unix_now(),
        "uptime_s": facts.uptime,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    // SAFETY: test code; a failed unwrap is the test failure report.

    use super::*;

    fn facts(stop: Option<&str>) -> ExitFacts {
        ExitFacts {
            pid: 1,
            gui_pid: 1,
            cgroup: None,
            unit: None,
            spawned_unix: unix_now(),
            uptime: 1.0,
            stop_reason: stop.map(str::to_owned),
        }
    }

    #[test]
    fn initialize_merge_sets_list_changed_and_keeps_child_fields() {
        let merged = merged_initialize(json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": { "listChanged": false }, "logging": {} },
            "serverInfo": { "name": "rs-cam" },
        }));
        assert_eq!(merged["capabilities"]["tools"]["listChanged"], json!(true));
        assert_eq!(merged["capabilities"]["logging"], json!({}));
        assert_eq!(merged["serverInfo"]["name"], json!("rs-cam"));
        assert!(
            merged["instructions"]
                .as_str()
                .unwrap()
                .contains("gui_restart")
        );
        let bare = merged_initialize(json!({ "protocolVersion": "x" }));
        assert_eq!(bare["capabilities"]["tools"]["listChanged"], json!(true));
    }

    #[test]
    fn proxy_tools_join_only_the_last_tools_list_page() {
        let last = append_proxy_tools(json!({ "id": 1, "result": { "tools": [{ "name": "a" }] } }));
        assert_eq!(last["result"]["tools"].as_array().unwrap().len(), 4);
        let page = append_proxy_tools(
            json!({ "id": 1, "result": { "tools": [{ "name": "a" }], "nextCursor": "c" } }),
        );
        assert_eq!(page["result"]["tools"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn exit_classification() {
        let normal = exit_record(Some(ExitStatus::from_raw(0)), &facts(None));
        assert_eq!(normal["reason"], json!("normal"));
        let error = exit_record(Some(ExitStatus::from_raw(3 << 8)), &facts(None));
        assert_eq!(error["reason"], json!("error_exit"));
        assert_eq!(error["code"], json!(3));
        let segv = exit_record(Some(ExitStatus::from_raw(11)), &facts(None));
        assert_eq!(segv["reason"], json!("killed"));
        assert_eq!(segv["signal_name"], json!("SIGSEGV"));
        // A launcher that does not exec reports 128 + 15.
        let wrapped = exit_record(Some(ExitStatus::from_raw(143 << 8)), &facts(None));
        assert_eq!(wrapped["signal_name"], json!("SIGTERM"));
        let stopped = exit_record(
            Some(ExitStatus::from_raw(15)),
            &facts(Some("stopped by gui_stop")),
        );
        assert_eq!(stopped["reason"], json!("stopped"));
        assert_eq!(stopped["summary"], json!("stopped by gui_stop"));
    }
}
