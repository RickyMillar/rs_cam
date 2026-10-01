//! `rs_cam_mcp_proxy`: a stdio MCP supervisor in front of `rs_cam_gui --mcp`.
//!
//! The MCP client (Claude Code) starts the proxy, and the proxy starts the
//! GUI as its child. The proxy stays up for the whole client session. When
//! the GUI exits, the agent reads the reason with `gui_status` and starts
//! the GUI again with `gui_restart`, without a client reconnect.

mod config;
mod log;
mod procinfo;
mod proxy;
mod tools;

use std::io::Write;
use std::process::ExitCode;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks a mutex. A panic in another thread does not stop the proxy, so a
/// poisoned lock gives its data back.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn main() -> ExitCode {
    match config::parse(std::env::args().skip(1)) {
        Ok(cfg) => proxy::run(cfg),
        Err(message) => {
            let _ = writeln!(
                std::io::stderr(),
                "rs_cam_mcp_proxy: {message}\n{}",
                config::USAGE
            );
            ExitCode::from(2)
        }
    }
}
