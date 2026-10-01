//! The tools that the proxy itself serves, next to the GUI tools.

use serde_json::{Value, json};

pub const GUI_STATUS: &str = "gui_status";
pub const GUI_RESTART: &str = "gui_restart";
pub const GUI_STOP: &str = "gui_stop";

/// The default time that `gui_restart` waits for the new GUI.
pub const DEFAULT_WAIT_READY_S: f64 = 60.0;

pub fn is_proxy_tool(name: &str) -> bool {
    matches!(name, GUI_STATUS | GUI_RESTART | GUI_STOP)
}

pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": GUI_STATUS,
            "description": concat!(
                "Live connection state of the rs_cam GUI behind the MCP proxy: state ",
                "(starting | connected | down | restarting), child and GUI pid, command, ",
                "binary path and mtime (and whether the binary is newer than the running ",
                "process), uptime, last exit (code, signal, reason normal | error_exit | ",
                "killed | oom_killed | stopped, with the OOM evidence that the proxy could ",
                "read), restart count, last handshake latency, and a live ping probe of the GUI."
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "probe_timeout_s": {
                        "type": "number",
                        "description": "Time limit of the live ping probe in seconds."
                    }
                }
            }
        }),
        json!({
            "name": GUI_RESTART,
            "description": concat!(
                "Stop the rs_cam GUI (close stdin, then SIGTERM, then SIGKILL), start it ",
                "again with the same command, replay the MCP handshake and wait until it ",
                "answers tools/list. Use it after a rebuild or when gui_status says down. ",
                "The GUI loses its unsaved project state. Sends ",
                "notifications/tools/list_changed when the GUI is ready."
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "wait_ready_s": {
                        "type": "number",
                        "description": "Time limit for the new GUI to answer, in seconds (default 60)."
                    }
                }
            }
        }),
        json!({
            "name": GUI_STOP,
            "description": "Stop the rs_cam GUI and do not start it again. gui_restart starts it again.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
    ]
}

/// The result of a proxy tool call.
pub fn call_result(payload: &Value, is_error: bool) -> Value {
    let text = serde_json::to_string_pretty(payload).unwrap_or_else(|_| payload.to_string());
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    })
}

/// Reads a positive number of seconds from the tool arguments.
pub fn seconds_arg(args: Option<&Value>, key: &str) -> Option<f64> {
    args?
        .get(key)?
        .as_f64()
        .filter(|s| s.is_finite() && *s > 0.0)
}
