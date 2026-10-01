//! Command-line parsing for the proxy.

use std::path::PathBuf;
use std::time::Duration;

pub const USAGE: &str = "usage: rs_cam_mcp_proxy [--log <file>] [--binary <path>] [--auto-restart] \
[--init-timeout-ms <ms>] [--grace-ms <ms>] [--probe-timeout-ms <ms>] -- <child command> [args...]";

/// The proxy settings. The child command follows the first `--`.
pub struct Config {
    pub command: Vec<String>,
    pub log_file: Option<PathBuf>,
    /// The GUI binary that `gui_status` reports. When it is not set, the
    /// proxy reads the running executable or the first file argument.
    pub binary: Option<PathBuf>,
    pub auto_restart: bool,
    /// The time the proxy waits for the child `initialize` answer before it
    /// answers the client itself.
    pub init_timeout: Duration,
    /// The time between the stop steps: stdin close, SIGTERM, SIGKILL.
    pub grace: Duration,
    /// The default time limit of the live probe in `gui_status`.
    pub probe_timeout: Duration,
}

pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Config, String> {
    let mut log_file = None;
    let mut binary = None;
    let mut auto_restart = false;
    let mut init_timeout = Duration::from_millis(20_000);
    let mut grace = Duration::from_millis(5_000);
    let mut probe_timeout = Duration::from_millis(3_000);
    loop {
        let Some(arg) = args.next() else {
            return Err("missing `--` and the child command".to_owned());
        };
        match arg.as_str() {
            "--" => break,
            "--log" => log_file = Some(PathBuf::from(value(&mut args, "--log")?)),
            "--binary" => binary = Some(PathBuf::from(value(&mut args, "--binary")?)),
            "--auto-restart" => auto_restart = true,
            "--init-timeout-ms" => init_timeout = millis(&mut args, "--init-timeout-ms")?,
            "--grace-ms" => grace = millis(&mut args, "--grace-ms")?,
            "--probe-timeout-ms" => probe_timeout = millis(&mut args, "--probe-timeout-ms")?,
            "-h" | "--help" => return Err("help requested".to_owned()),
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    let command: Vec<String> = args.collect();
    if command.is_empty() {
        return Err("the child command after `--` is empty".to_owned());
    }
    Ok(Config {
        command,
        log_file,
        binary,
        auto_restart,
        init_timeout,
        grace,
        probe_timeout,
    })
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("`{flag}` needs a value"))
}

fn millis(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<Duration, String> {
    let text = value(args, flag)?;
    text.parse::<u64>()
        .map(Duration::from_millis)
        .map_err(|e| format!("`{flag}` needs a whole number of milliseconds, got `{text}` ({e})"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    // SAFETY: test code; a failed unwrap is the test failure report.

    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn the_first_double_dash_starts_the_child_command() {
        let cfg = parse(args(&[
            "--log",
            "/tmp/x",
            "--",
            "systemd-run",
            "--",
            "gui",
            "--mcp",
        ]))
        .unwrap();
        assert_eq!(cfg.command, ["systemd-run", "--", "gui", "--mcp"]);
        assert_eq!(cfg.log_file, Some(PathBuf::from("/tmp/x")));
        assert!(!cfg.auto_restart);
    }

    #[test]
    fn bad_arguments_are_refused() {
        assert!(parse(args(&["gui"])).is_err());
        assert!(parse(args(&["--"])).is_err());
        assert!(parse(args(&["--grace-ms", "soon", "--", "gui"])).is_err());
    }
}
