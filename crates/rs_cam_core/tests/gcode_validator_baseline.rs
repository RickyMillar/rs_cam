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
//! **Current baseline: ZERO findings across all 70 captures** (G9:
//! 72 fixture pairs less the 2 refused f16 pairs). This test
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
//! committed at `tests/fixtures/gcode_golden/` (the G9 goldens), no
//! external tooling.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

mod common;

use common::gcode_fixtures::{DIALECTS, FIXTURES, golden_path};
use rs_cam_core::export::gcode_validator::{ValidatorOptions, validate_with};

/// The golden for one pair, or `None` when the export door refuses that
/// pair (G9: a refused pair has no golden; `gcode_phase0_capture` pins the
/// refusal).
fn read_capture(fixture: &str, dialect: &str) -> Option<String> {
    std::fs::read_to_string(golden_path(fixture, dialect)).ok()
}

#[test]
fn baseline_findings_match_expected() {
    let mut failures: Vec<String> = Vec::new();
    let mut captures = 0usize;

    for (fixture, _) in FIXTURES {
        for (dialect, post) in DIALECTS {
            let Some(gcode) = read_capture(fixture, dialect) else {
                continue;
            };
            captures += 1;
            // G8: f18 emits with the grblHAL mist option on, so M7 is
            // legal there; validate it with the options it emitted with.
            let options = ValidatorOptions {
                mist_output: *fixture == "f18_grblhal_options",
                ..ValidatorOptions::default()
            };
            let findings = validate_with(&gcode, post, &options);
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
