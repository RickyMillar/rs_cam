//! Validator baseline: runs `gcode_validator::validate` on each captured
//! fixture and asserts the finding counts match expectations.
//!
//! History of driving the baseline to zero:
//! - Phase 1 captured 98 findings across the corpus.
//! - Grbl post gained explicit G54 → cleared 16 MissingWcs.
//! - Grbl tool_change became an M0 pause → cleared 2 UnsupportedM6.
//! - Post-layer audit fixes (2026-06-11, A5+A6): Mach3 gained a G54
//!   preamble line (cleared 16 MissingWcs), LinuxCNC gained G91.1
//!   (cleared 16 MissingG91_1), the `%`-bracket requirement was dropped
//!   (modern LinuxCNC streamers don't need it; cleared 32
//!   MissingProgramBrackets), and the M2-vs-M30 rule was retired (M2 and
//!   M30 are both valid RS274 program ends; cleared 16
//!   WrongProgramEndCode).
//!
//! **Current baseline: ZERO findings across all 64 captures.** This test
//! is the regression net: any reappearing finding is a real emitter /
//! post / validator change and must be reviewed in the same commit.
//!
//! Run with:
//!
//! ```bash
//! cargo test --test gcode_validator_baseline
//! ```
//!
//! No `#[ignore]` — this is a normal test. It only depends on files
//! committed at `planning/gcode_current_outputs/`, no external tooling.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use rs_cam_core::gcode::PostFormat;
use rs_cam_core::gcode_validator::validate;
use std::path::PathBuf;

const FIXTURES: &[&str] = &[
    "f1_basic_lines",
    "f2_arcs_xy",
    "f3_helical_ramp",
    "f4_profile_multipass",
    "f5_two_tool_changes",
    "f6_two_setups",
    "f7_full_circle",
    "f8_x_only_feed",
    "f9_ramp_into_arc",
    "f10_tiny_arcs",
    "f11_depth_step_boundary",
    "f12_tool_change_at_z_zero",
    "f13_climb_vs_conventional",
    "f14_multi_line_pause_message",
    "f15_embedded_newline_snippets",
    "f16_comp_round_trip",
];

const DIALECTS: &[(&str, PostFormat)] = &[
    ("grbl", PostFormat::Grbl),
    ("grblhal", PostFormat::GrblHal),
    ("linuxcnc", PostFormat::LinuxCnc),
    ("mach3", PostFormat::Mach3),
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("canonicalize workspace root")
}

fn read_capture(fixture: &str, dialect: &str) -> String {
    let path = workspace_root()
        .join("planning")
        .join("gcode_current_outputs")
        .join(format!("{fixture}_{dialect}.nc"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn baseline_findings_match_expected() {
    let mut failures: Vec<String> = Vec::new();
    let mut captures = 0usize;

    for fixture in FIXTURES {
        for (dialect, post) in DIALECTS {
            captures += 1;
            let gcode = read_capture(fixture, dialect);
            let findings = validate(&gcode, *post);
            println!("{fixture:32} {dialect:9} {} finding(s)", findings.len());
            if !findings.is_empty() {
                let summary: Vec<String> = findings
                    .iter()
                    .map(|f| format!("line {} {:?}", f.line, f.kind))
                    .collect();
                failures.push(format!(
                    "{fixture}_{dialect}: expected 0 findings, got {} — {}",
                    findings.len(),
                    summary.join("; ")
                ));
            }
        }
    }

    println!("\nTotal captures checked: {captures}");

    assert!(
        failures.is_empty(),
        "Baseline regression ({} captures with findings; baseline is ZERO):\n  {}\n\n\
         If you intentionally changed the emitter/posts/validator, review and update this test in the same commit.",
        failures.len(),
        failures.join("\n  ")
    );
}
