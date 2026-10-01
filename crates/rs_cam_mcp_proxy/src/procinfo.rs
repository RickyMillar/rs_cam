//! Process facts from `/proc`, signals through the `kill` command, and the
//! OOM evidence for a killed child.
//!
//! The proxy sends SIGTERM with the `kill` command and SIGKILL with
//! `std::process::Child::kill`. Both are safe code: the workspace denies
//! `unsafe_code`, and a signal crate is one more dependency for one call.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Launchers that start the GUI as a child or replace themselves with it.
/// The proxy looks through them to find the GUI process.
const WRAPPERS: &[&str] = &[
    "systemd-run",
    "bash",
    "sh",
    "dash",
    "zsh",
    "env",
    "nice",
    "timeout",
    "gui_logged.sh",
];

pub fn comm(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_owned())
}

/// True when the process exists and is not a zombie.
pub fn is_alive(pid: u32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => false,
        Ok(stat) => stat_fields(&stat).and_then(|mut f| f.next()) != Some("Z"),
    }
}

/// The fields of `/proc/<pid>/stat` after the `comm` field. The `comm`
/// field can hold spaces, so the split starts after the last `)`.
fn stat_fields(stat: &str) -> Option<std::str::SplitWhitespace<'_>> {
    let close = stat.rfind(')')?;
    stat.get(close + 1..).map(str::split_whitespace)
}

fn children(pid: u32) -> Vec<u32> {
    if let Ok(text) = std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")) {
        return text
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
    }
    // Fallback: scan every process for its parent id.
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    dir.filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()))
        .filter(|child| {
            std::fs::read_to_string(format!("/proc/{child}/stat"))
                .ok()
                .and_then(|stat| {
                    stat_fields(&stat)
                        .and_then(|mut f| f.nth(1))
                        .and_then(|p| p.parse::<u32>().ok())
                })
                == Some(pid)
        })
        .collect()
}

/// The GUI process: the spawned process, or the first descendant that is
/// not a known launcher. `systemd-run --scope` replaces itself with the
/// command, so the two ids are usually the same.
pub fn gui_pid(pid: u32) -> u32 {
    let mut current = pid;
    for _ in 0..6 {
        let Some(name) = comm(current) else {
            return current;
        };
        if !WRAPPERS.contains(&name.as_str()) {
            return current;
        }
        match children(current).first() {
            Some(child) => current = *child,
            None => return current,
        }
    }
    current
}

/// The cgroup v2 path of the process, for example
/// `/user.slice/.../app.slice/run-u123.scope`.
pub fn cgroup_path(pid: u32) -> Option<String> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("0::"))
        .map(str::to_owned)
}

/// The systemd unit that owns the cgroup, when the last path part is one.
pub fn unit_of(cgroup: &str) -> Option<String> {
    let last = cgroup.rsplit('/').next()?;
    (last.ends_with(".scope") || last.ends_with(".service")).then(|| last.to_owned())
}

/// The executable of a running process and whether it was replaced on disk
/// after the start (Linux shows ` (deleted)`).
pub fn exe(pid: u32) -> Option<(PathBuf, bool)> {
    let link = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    let text = link.to_string_lossy();
    match text.strip_suffix(" (deleted)") {
        Some(stripped) => Some((PathBuf::from(stripped), true)),
        None => Some((link, false)),
    }
}

pub fn mtime_unix(path: &Path) -> Option<f64> {
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs_f64())
}

/// Sends a signal by name (`TERM`, `KILL`) with the `kill` command.
pub fn send_signal(pid: u32, signal: &str) -> Result<(), String> {
    run_with_timeout(
        "kill",
        &[&format!("-{signal}"), &pid.to_string()],
        Duration::from_secs(3),
    )
    .map(drop)
}

pub fn signal_name(signal: i32) -> &'static str {
    match signal {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        4 => "SIGILL",
        6 => "SIGABRT",
        7 => "SIGBUS",
        8 => "SIGFPE",
        9 => "SIGKILL",
        11 => "SIGSEGV",
        13 => "SIGPIPE",
        15 => "SIGTERM",
        _ => "signal",
    }
}

/// Runs a command, returns its stdout, and kills it after `timeout`.
pub fn run_with_timeout(program: &str, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run `{program}`: {e}"))?;
    let mut stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(out) = stdout.as_mut() {
            let _ = out.read_to_string(&mut text);
        }
        text
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("`{program}` gave no result within {timeout:?}"));
            }
            Err(e) => return Err(format!("`{program}` wait failed: {e}")),
        }
    };
    let text = reader.join().unwrap_or_default();
    if status.success() {
        Ok(text)
    } else {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut err);
        }
        Err(format!("`{program}` failed ({status}): {}", err.trim()))
    }
}

/// The OOM evidence for a child that a SIGKILL stopped. The function reads
/// each source that it can and says what each source gave:
///
/// 1. the `oom_kill` count in the scope `memory.events` (it is often gone,
///    because systemd removes a transient scope when it is empty);
/// 2. the `Result` property of the systemd unit (`oom-kill`);
/// 3. the user journal of the unit;
/// 4. the kernel journal line `Killed process <pid>`.
pub fn oom_evidence(
    cgroup: Option<&str>,
    unit: Option<&str>,
    since_unix: u64,
    pid: u32,
) -> (bool, Vec<String>) {
    let mut found = false;
    let mut notes = Vec::new();
    let timeout = Duration::from_secs(3);
    let since = format!("@{since_unix}");

    match cgroup {
        None => notes.push("memory.events: no cgroup was recorded for the GUI".to_owned()),
        Some(cg) => {
            let path = format!("/sys/fs/cgroup{cg}/memory.events");
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    let count = text
                        .lines()
                        .find_map(|l| l.strip_prefix("oom_kill "))
                        .and_then(|v| v.trim().parse::<u64>().ok())
                        .unwrap_or(0);
                    found |= count > 0;
                    notes.push(format!("{path}: oom_kill={count}"));
                }
                Err(e) => notes.push(format!("{path}: unreadable ({e}); the scope is gone")),
            }
        }
    }

    if let Some(unit) = unit {
        match run_with_timeout(
            "systemctl",
            &["--user", "show", unit, "-p", "Result", "--value"],
            timeout,
        ) {
            Ok(out) => {
                let result = out.trim();
                found |= result == "oom-kill";
                notes.push(format!("systemctl --user show {unit}: Result={result:?}"));
            }
            Err(e) => notes.push(format!("systemctl: {e}")),
        }
        match run_with_timeout(
            "journalctl",
            &[
                "--user",
                "-u",
                unit,
                "--since",
                &since,
                "-o",
                "cat",
                "--no-pager",
                "-n",
                "200",
            ],
            timeout,
        ) {
            Ok(out) => match out.lines().find(|l| l.to_ascii_lowercase().contains("oom")) {
                Some(line) => {
                    found = true;
                    notes.push(format!("journalctl --user -u {unit}: {line}"));
                }
                None => notes.push(format!("journalctl --user -u {unit}: no OOM line")),
            },
            Err(e) => notes.push(format!("journalctl --user: {e}")),
        }
    } else {
        notes.push("systemd unit: none (the GUI is not in a scope or service)".to_owned());
    }

    let needle = format!("Killed process {pid} ");
    match run_with_timeout(
        "journalctl",
        &[
            "-k",
            "--since",
            &since,
            "-o",
            "cat",
            "--no-pager",
            "-n",
            "500",
        ],
        timeout,
    ) {
        Ok(out) => match out.lines().find(|l| l.contains(&needle)) {
            Some(line) => {
                found = true;
                notes.push(format!("journalctl -k: {line}"));
            }
            None => notes.push(format!("journalctl -k: no line `{}`", needle.trim())),
        },
        Err(e) => notes.push(format!("journalctl -k: {e}")),
    }
    (found, notes)
}
