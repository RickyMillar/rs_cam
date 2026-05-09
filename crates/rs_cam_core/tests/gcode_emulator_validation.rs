//! Phase 0.5 emulator validation: pipe each captured fixture through an
//! open-source controller emulator's parser and assert it accepts the
//! program.
//!
//! Currently uses `gvalidate` (from `grbl/grbl-sim`) as the cross-dialect
//! syntax checker. It's permissive enough to accept LinuxCNC's `G91.1`,
//! `G53`, and `M2` without erroring, so it serves as a useful baseline
//! across all three of our shipped dialects (Grbl / LinuxCNC / Mach3).
//!
//! LinuxCNC-specific semantic rules (G91.1 required, M2 vs M30 hygiene,
//! `%` wrapping) are owned by the Phase 1 validator instead — those are
//! known from spec reading, no emulator needed.
//!
//! grblHAL validation is deferred to Phase 4 (no rs_cam grblHAL post yet,
//! and `grblHAL_validator` has a headless-init hang to debug).
//!
//! ## Running
//!
//! These tests are `#[ignore]`-gated so they only run when explicitly
//! invoked. Build the validator first:
//!
//! ```bash
//! cd reference/validators/grbl/grbl/sim && make gvalidate
//! cargo test --test gcode_emulator_validation -- --ignored --nocapture
//! ```
//!
//! If `gvalidate` is not built, tests skip with a clear message instead
//! of failing — so the gate degrades gracefully for contributors who
//! haven't set up the validators yet.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("canonicalize workspace root")
}

fn gvalidate_path() -> PathBuf {
    workspace_root()
        .join("reference")
        .join("validators")
        .join("grbl")
        .join("grbl")
        .join("sim")
        .join("gvalidate.exe")
}

fn fixture_output(fixture: &str, dialect: &str) -> PathBuf {
    workspace_root()
        .join("planning")
        .join("gcode_current_outputs")
        .join(format!("{fixture}_{dialect}.nc"))
}

/// Strip pause-blocking M-codes from g-code before sending to gvalidate.
/// gvalidate emulates real Grbl, which on `M0`/`M1` waits for a resume
/// signal (cycle-start button) — there's no flag to bypass. We strip
/// these for parser-validation purposes; the M0 emission is itself
/// regression-tested elsewhere by the existing in-source unit tests.
fn strip_pause_codes(input: &str) -> String {
    input
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("M0\n")
                || t == "M0"
                || t.starts_with("M0 ")
                || t.starts_with("M1\n")
                || t == "M1"
                || t.starts_with("M1 "))
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// Run gvalidate on one capture. Returns Some((exit_code, stdout)).
/// Returns None if gvalidate isn't built (test then skips).
///
/// Uses a 10s timeout — gvalidate is supposed to be near-instant on
/// our tiny fixtures; anything longer indicates a hang (e.g. waiting
/// on an M-code we forgot to strip).
fn run_gvalidate(capture: &Path) -> Option<(i32, String)> {
    let validator = gvalidate_path();
    if !validator.exists() {
        eprintln!(
            "SKIP: gvalidate not found at {}\n      build with: cd reference/validators/grbl/grbl/sim && make gvalidate",
            validator.display()
        );
        return None;
    }
    if !capture.exists() {
        panic!("capture missing: {}", capture.display());
    }

    let original = std::fs::read_to_string(capture).expect("read capture");
    let cleaned = strip_pause_codes(&original);

    // Pipe cleaned content via stdin to /dev/stdin since gvalidate takes
    // a path argument. Easier: write to a temp file.
    let tmp = std::env::temp_dir().join(format!(
        "rscam_gvalidate_{}.nc",
        capture.file_stem().and_then(|s| s.to_str()).unwrap_or("x")
    ));
    std::fs::write(&tmp, &cleaned).expect("write temp");

    let mut child = Command::new(&validator)
        .arg(&tmp)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn gvalidate");

    // Poll for up to 10s.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let exit = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(-1),
            Ok(None) if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&tmp);
                panic!(
                    "gvalidate timed out (>10s) on {} — likely hung on a pause M-code",
                    capture.display()
                );
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => panic!("wait failed: {e}"),
        }
    };

    let mut stdout = String::new();
    if let Some(mut out) = child.stdout.take() {
        use std::io::Read;
        let _ = out.read_to_string(&mut stdout);
    }
    let _ = std::fs::remove_file(&tmp);
    Some((exit, stdout))
}

// Suppress unused-import warning when only the file-path path is taken.
#[allow(dead_code)]
fn _unused_write_keep() -> impl Write {
    std::io::sink()
}

fn assert_gvalidate_accepts(fixture: &str, dialect: &str) {
    let capture = fixture_output(fixture, dialect);
    let Some((exit, stdout)) = run_gvalidate(&capture) else {
        return;
    };
    println!("{fixture}_{dialect}: exit {exit}");
    if !stdout.is_empty() {
        println!("---stdout---\n{stdout}---");
    }
    assert_eq!(
        exit, 0,
        "gvalidate rejected {fixture}_{dialect} (exit {exit}). stdout:\n{stdout}"
    );
}

/// Assert that gvalidate REJECTS a capture with a specific exit code,
/// because the emitter is producing g-code that the controller can't
/// accept (a real bug we want tracked as a regression test).
fn assert_gvalidate_rejects(fixture: &str, dialect: &str, expected_exit: i32, reason: &str) {
    let capture = fixture_output(fixture, dialect);
    let Some((exit, stdout)) = run_gvalidate(&capture) else {
        return;
    };
    println!("{fixture}_{dialect}: exit {exit} (EXPECTED FAILURE: {reason})");
    if exit == 0 {
        panic!(
            "gvalidate ACCEPTED {fixture}_{dialect}, but we expected it to fail (exit {expected_exit}: {reason}).\n\
             Either the emitter was fixed (good — flip this test to assert_gvalidate_accepts!) \
             or gvalidate became more permissive.\nstdout:\n{stdout}"
        );
    }
    assert_eq!(
        exit, expected_exit,
        "gvalidate rejected {fixture}_{dialect} but with exit {exit}, not the expected {expected_exit}.\n\
         Reason was: {reason}\nstdout:\n{stdout}"
    );
}

// ── Grbl: primary, gvalidate IS the Grbl parser ────────────────────────
#[test]
#[ignore = "phase 0.5 emulator validation; run with --ignored after building gvalidate"]
fn validate_f1_grbl() {
    assert_gvalidate_accepts("f1_basic_lines", "grbl");
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f2_grbl() {
    assert_gvalidate_accepts("f2_arcs_xy", "grbl");
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f3_grbl() {
    assert_gvalidate_accepts("f3_helical_ramp", "grbl");
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f4_grbl() {
    assert_gvalidate_accepts("f4_profile_multipass", "grbl");
}
/// Real bug surfaced by Phase 0.5: rs_cam emits `M6 T2` for Grbl tool
/// changes, but Grbl 1.1 does NOT support M6 (gvalidate rejects with
/// error 20). Fusion's grbl.cps defaults `useM06=false` for this exact
/// reason. The Grbl post needs a `useM06`-equivalent toggle (Phase 3
/// when posts become data-driven), defaulting to false.
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f5_grbl() {
    assert_gvalidate_rejects(
        "f5_two_tool_changes",
        "grbl",
        20,
        "Grbl 1.1 does not support M6; rs_cam Grbl post should not emit it (useM06=false equivalent)",
    );
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f6_grbl() {
    assert_gvalidate_accepts("f6_two_setups", "grbl");
}

// ── LinuxCNC: gvalidate as syntax check (semantic rules → Phase 1) ─────
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f1_linuxcnc() {
    assert_gvalidate_accepts("f1_basic_lines", "linuxcnc");
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f2_linuxcnc() {
    assert_gvalidate_accepts("f2_arcs_xy", "linuxcnc");
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f3_linuxcnc() {
    assert_gvalidate_accepts("f3_helical_ramp", "linuxcnc");
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f4_linuxcnc() {
    assert_gvalidate_accepts("f4_profile_multipass", "linuxcnc");
}
/// Phase 0.5 limitation: gvalidate (Grbl) rejects M6, but M6 IS valid
/// in LinuxCNC. We need a real LinuxCNC parser to validate this output
/// — deferred to Phase 4. Until then, F5 LinuxCNC is expected to fail
/// gvalidate with exit 20 for the same M6 reason as Grbl.
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f5_linuxcnc() {
    assert_gvalidate_rejects(
        "f5_two_tool_changes",
        "linuxcnc",
        20,
        "gvalidate proxy doesn't implement M6 (valid in LinuxCNC); needs real LinuxCNC parser in Phase 4",
    );
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f6_linuxcnc() {
    assert_gvalidate_accepts("f6_two_setups", "linuxcnc");
}

// ── Mach3: gvalidate as syntax-only proxy ──────────────────────────────
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f1_mach3() {
    assert_gvalidate_accepts("f1_basic_lines", "mach3");
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f2_mach3() {
    assert_gvalidate_accepts("f2_arcs_xy", "mach3");
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f3_mach3() {
    assert_gvalidate_accepts("f3_helical_ramp", "mach3");
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f4_mach3() {
    assert_gvalidate_accepts("f4_profile_multipass", "mach3");
}
/// Same Phase 0.5 limitation as F5 LinuxCNC — M6 is valid in Mach3
/// but unsupported by gvalidate. Real validation deferred to Phase 4.
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f5_mach3() {
    assert_gvalidate_rejects(
        "f5_two_tool_changes",
        "mach3",
        20,
        "gvalidate proxy doesn't implement M6 (valid in Mach3); needs real LinuxCNC parser in Phase 4",
    );
}
#[test]
#[ignore = "phase 0.5 emulator validation"]
fn validate_f6_mach3() {
    assert_gvalidate_accepts("f6_two_setups", "mach3");
}
