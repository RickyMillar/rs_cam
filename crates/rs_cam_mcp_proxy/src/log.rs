//! The proxy log: every line goes to the proxy stderr and to the optional
//! log file. Stdout is the MCP transport, so nothing here writes to it.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::lock;

pub struct Logger {
    file: Option<Mutex<File>>,
}

impl Logger {
    pub fn open(path: Option<&Path>) -> Result<Self, String> {
        let file = match path {
            None => None,
            Some(path) => Some(Mutex::new({
                if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                    std::fs::create_dir_all(dir).map_err(|e| {
                        format!("cannot create log directory {}: {e}", dir.display())
                    })?;
                }
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|e| format!("cannot open log file {}: {e}", path.display()))?
            })),
        };
        Ok(Self { file })
    }

    /// Writes one proxy event.
    pub fn event(&self, message: &str) {
        let line = format!("[rs_cam_mcp_proxy {:.3}] {message}\n", unix_now());
        let _ = std::io::stderr().write_all(line.as_bytes());
        self.to_file(&line);
    }

    /// Passes one child stderr line through unchanged and keeps a copy.
    pub fn child_stderr(&self, line: &str) {
        let _ = std::io::stderr().write_all(format!("{line}\n").as_bytes());
        self.to_file(&format!("[gui {:.3}] {line}\n", unix_now()));
    }

    fn to_file(&self, line: &str) {
        if let Some(file) = &self.file {
            let mut file = lock(file);
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
}

/// Seconds since the Unix epoch, as a fraction.
pub fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}
