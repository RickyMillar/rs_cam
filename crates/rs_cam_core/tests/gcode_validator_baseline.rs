//! Phase 1 validator baseline: runs `gcode_validator::validate` on each
//! captured fixture and asserts the finding count + kinds match what we
//! expect from `planning/gcode_gap_report.md`.
//!
//! Phase 4b: dialect set grew from 3 to 4 (added grblHAL) and corpus
//! grew from 6 to 16 fixtures. grblHAL captures all read 0 findings;
//! the new fixtures inherit the same per-dialect issues as F1–F6.
//! Current baseline: 98 findings across 64 captures.
//!
//! The goal of subsequent phases is to drive each of these counts to
//! zero. This test acts as the regression suite: when Phase 2/3 fixes
//! the emitter, update the expected counts here in the same commit.
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
use rs_cam_core::gcode_validator::{Finding, FindingKind, validate};
use std::path::PathBuf;

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

fn dialect_to_post(dialect: &str) -> PostFormat {
    match dialect {
        "grbl" => PostFormat::Grbl,
        "grblhal" => PostFormat::GrblHal,
        "linuxcnc" => PostFormat::LinuxCnc,
        "mach3" => PostFormat::Mach3,
        other => panic!("unknown dialect: {other}"),
    }
}

fn count_kind(findings: &[Finding], kind: FindingKind) -> usize {
    findings.iter().filter(|f| f.kind == kind).count()
}

/// One row of the expected baseline: counts of each finding kind for
/// (fixture, dialect). Anything not listed is expected to be 0.
struct Expected {
    fixture: &'static str,
    dialect: &'static str,
    /// (kind, count) pairs.
    findings: &'static [(FindingKind, usize)],
}

/// Phase 1 baseline — what each capture currently produces. Numbers
/// here come from the gap report's "still-pending bugs" list; driving
/// them to zero is the Phase 2/3 work.
const BASELINE: &[Expected] = &[
    // ── Grbl: only need MissingWcs (Grbl post doesn't emit G54), plus
    //    UnsupportedM6 on the multi-tool fixture. F6 has the same
    //    no-WCS issue inside its single emitted block.
    Expected {
        fixture: "f1_basic_lines",
        dialect: "grbl",
        findings: &[(FindingKind::MissingWcs, 1)],
    },
    Expected {
        fixture: "f2_arcs_xy",
        dialect: "grbl",
        findings: &[(FindingKind::MissingWcs, 1)],
    },
    Expected {
        fixture: "f3_helical_ramp",
        dialect: "grbl",
        findings: &[(FindingKind::MissingWcs, 1)],
    },
    Expected {
        fixture: "f4_profile_multipass",
        dialect: "grbl",
        findings: &[(FindingKind::MissingWcs, 1)],
    },
    Expected {
        fixture: "f5_two_tool_changes",
        dialect: "grbl",
        findings: &[
            (FindingKind::UnsupportedM6, 1),
            (FindingKind::MissingWcs, 1),
        ],
    },
    Expected {
        fixture: "f6_two_setups",
        dialect: "grbl",
        findings: &[(FindingKind::MissingWcs, 1)],
    },

    // ── grblHAL: emits G54 + supports M6 + M30 program end →
    //    zero findings across the existing F1–F6 corpus.
    Expected { fixture: "f1_basic_lines",       dialect: "grblhal", findings: &[] },
    Expected { fixture: "f2_arcs_xy",           dialect: "grblhal", findings: &[] },
    Expected { fixture: "f3_helical_ramp",      dialect: "grblhal", findings: &[] },
    Expected { fixture: "f4_profile_multipass", dialect: "grblhal", findings: &[] },
    Expected { fixture: "f5_two_tool_changes",  dialect: "grblhal", findings: &[] },
    Expected { fixture: "f6_two_setups",        dialect: "grblhal", findings: &[] },

    // ── LinuxCNC: missing G91.1, missing % (×2 leading + trailing),
    //    M2 instead of M30. G54 IS emitted by the LinuxCNC post so
    //    MissingWcs should be 0. M6 is supported on LinuxCNC so F5
    //    doesn't add an UnsupportedM6.
    Expected {
        fixture: "f1_basic_lines",
        dialect: "linuxcnc",
        findings: &[
            (FindingKind::MissingG91_1, 1),
            (FindingKind::MissingProgramBrackets, 2),
            (FindingKind::WrongProgramEndCode, 1),
        ],
    },
    Expected {
        fixture: "f2_arcs_xy",
        dialect: "linuxcnc",
        findings: &[
            (FindingKind::MissingG91_1, 1),
            (FindingKind::MissingProgramBrackets, 2),
            (FindingKind::WrongProgramEndCode, 1),
        ],
    },
    Expected {
        fixture: "f3_helical_ramp",
        dialect: "linuxcnc",
        findings: &[
            (FindingKind::MissingG91_1, 1),
            (FindingKind::MissingProgramBrackets, 2),
            (FindingKind::WrongProgramEndCode, 1),
        ],
    },
    Expected {
        fixture: "f4_profile_multipass",
        dialect: "linuxcnc",
        findings: &[
            (FindingKind::MissingG91_1, 1),
            (FindingKind::MissingProgramBrackets, 2),
            (FindingKind::WrongProgramEndCode, 1),
        ],
    },
    Expected {
        fixture: "f5_two_tool_changes",
        dialect: "linuxcnc",
        findings: &[
            (FindingKind::MissingG91_1, 1),
            (FindingKind::MissingProgramBrackets, 2),
            (FindingKind::WrongProgramEndCode, 1),
        ],
    },
    Expected {
        fixture: "f6_two_setups",
        dialect: "linuxcnc",
        findings: &[
            (FindingKind::MissingG91_1, 1),
            (FindingKind::MissingProgramBrackets, 2),
            (FindingKind::WrongProgramEndCode, 1),
        ],
    },

    // ── Mach3: only MissingWcs (Mach3 post also doesn't emit G54).
    //    M6 supported, M30 emitted, G91.1 not required, % not required.
    Expected {
        fixture: "f1_basic_lines",
        dialect: "mach3",
        findings: &[(FindingKind::MissingWcs, 1)],
    },
    Expected {
        fixture: "f2_arcs_xy",
        dialect: "mach3",
        findings: &[(FindingKind::MissingWcs, 1)],
    },
    Expected {
        fixture: "f3_helical_ramp",
        dialect: "mach3",
        findings: &[(FindingKind::MissingWcs, 1)],
    },
    Expected {
        fixture: "f4_profile_multipass",
        dialect: "mach3",
        findings: &[(FindingKind::MissingWcs, 1)],
    },
    Expected {
        fixture: "f5_two_tool_changes",
        dialect: "mach3",
        findings: &[(FindingKind::MissingWcs, 1)],
    },
    Expected {
        fixture: "f6_two_setups",
        dialect: "mach3",
        findings: &[(FindingKind::MissingWcs, 1)],
    },

    // ── Phase 4b broadened corpus (F7-F16). Expected counts derive
    //    from the same per-dialect rules as F1-F6. Probed empirically
    //    on first run; any drift here is a real validator/emitter
    //    change and must be reviewed in the same commit.

    // F7  full-circle CCW
    Expected { fixture: "f7_full_circle", dialect: "grbl",     findings: &[(FindingKind::MissingWcs, 1)] },
    Expected { fixture: "f7_full_circle", dialect: "grblhal",  findings: &[] },
    Expected { fixture: "f7_full_circle", dialect: "linuxcnc", findings: &[(FindingKind::MissingG91_1, 1), (FindingKind::MissingProgramBrackets, 2), (FindingKind::WrongProgramEndCode, 1)] },
    Expected { fixture: "f7_full_circle", dialect: "mach3",    findings: &[(FindingKind::MissingWcs, 1)] },

    // F8  X-only feed (5 linear moves, no arcs)
    Expected { fixture: "f8_x_only_feed", dialect: "grbl",     findings: &[(FindingKind::MissingWcs, 1)] },
    Expected { fixture: "f8_x_only_feed", dialect: "grblhal",  findings: &[] },
    Expected { fixture: "f8_x_only_feed", dialect: "linuxcnc", findings: &[(FindingKind::MissingG91_1, 1), (FindingKind::MissingProgramBrackets, 2), (FindingKind::WrongProgramEndCode, 1)] },
    Expected { fixture: "f8_x_only_feed", dialect: "mach3",    findings: &[(FindingKind::MissingWcs, 1)] },

    // F9  Ramp into arc
    Expected { fixture: "f9_ramp_into_arc", dialect: "grbl",     findings: &[(FindingKind::MissingWcs, 1)] },
    Expected { fixture: "f9_ramp_into_arc", dialect: "grblhal",  findings: &[] },
    Expected { fixture: "f9_ramp_into_arc", dialect: "linuxcnc", findings: &[(FindingKind::MissingG91_1, 1), (FindingKind::MissingProgramBrackets, 2), (FindingKind::WrongProgramEndCode, 1)] },
    Expected { fixture: "f9_ramp_into_arc", dialect: "mach3",    findings: &[(FindingKind::MissingWcs, 1)] },

    // F10 Tiny arcs (sub-0.05mm)
    Expected { fixture: "f10_tiny_arcs", dialect: "grbl",     findings: &[(FindingKind::MissingWcs, 1)] },
    Expected { fixture: "f10_tiny_arcs", dialect: "grblhal",  findings: &[] },
    Expected { fixture: "f10_tiny_arcs", dialect: "linuxcnc", findings: &[(FindingKind::MissingG91_1, 1), (FindingKind::MissingProgramBrackets, 2), (FindingKind::WrongProgramEndCode, 1)] },
    Expected { fixture: "f10_tiny_arcs", dialect: "mach3",    findings: &[(FindingKind::MissingWcs, 1)] },

    // F11 Depth-step boundary
    Expected { fixture: "f11_depth_step_boundary", dialect: "grbl",     findings: &[(FindingKind::MissingWcs, 1)] },
    Expected { fixture: "f11_depth_step_boundary", dialect: "grblhal",  findings: &[] },
    Expected { fixture: "f11_depth_step_boundary", dialect: "linuxcnc", findings: &[(FindingKind::MissingG91_1, 1), (FindingKind::MissingProgramBrackets, 2), (FindingKind::WrongProgramEndCode, 1)] },
    Expected { fixture: "f11_depth_step_boundary", dialect: "mach3",    findings: &[(FindingKind::MissingWcs, 1)] },

    // F12 Tool change at Z=0 (multi-tool)
    Expected { fixture: "f12_tool_change_at_z_zero", dialect: "grbl",     findings: &[(FindingKind::UnsupportedM6, 1), (FindingKind::MissingWcs, 1)] },
    Expected { fixture: "f12_tool_change_at_z_zero", dialect: "grblhal",  findings: &[] },
    Expected { fixture: "f12_tool_change_at_z_zero", dialect: "linuxcnc", findings: &[(FindingKind::MissingG91_1, 1), (FindingKind::MissingProgramBrackets, 2), (FindingKind::WrongProgramEndCode, 1)] },
    Expected { fixture: "f12_tool_change_at_z_zero", dialect: "mach3",    findings: &[(FindingKind::MissingWcs, 1)] },

    // F13 Climb vs conventional (single tool, two phases)
    Expected { fixture: "f13_climb_vs_conventional", dialect: "grbl",     findings: &[(FindingKind::MissingWcs, 1)] },
    Expected { fixture: "f13_climb_vs_conventional", dialect: "grblhal",  findings: &[] },
    Expected { fixture: "f13_climb_vs_conventional", dialect: "linuxcnc", findings: &[(FindingKind::MissingG91_1, 1), (FindingKind::MissingProgramBrackets, 2), (FindingKind::WrongProgramEndCode, 1)] },
    Expected { fixture: "f13_climb_vs_conventional", dialect: "mach3",    findings: &[(FindingKind::MissingWcs, 1)] },

    // F14 Multi-line pause message — surfaces a real comment-syntax
    //     bug (newline inside () comment block), but the validator's
    //     5 priority rules don't cover that yet; finding count
    //     matches F6 (multi-setup with WCS issue per dialect).
    Expected { fixture: "f14_multi_line_pause_message", dialect: "grbl",     findings: &[(FindingKind::MissingWcs, 1)] },
    Expected { fixture: "f14_multi_line_pause_message", dialect: "grblhal",  findings: &[] },
    Expected { fixture: "f14_multi_line_pause_message", dialect: "linuxcnc", findings: &[(FindingKind::MissingG91_1, 1), (FindingKind::MissingProgramBrackets, 2), (FindingKind::WrongProgramEndCode, 1)] },
    Expected { fixture: "f14_multi_line_pause_message", dialect: "mach3",    findings: &[(FindingKind::MissingWcs, 1)] },

    // F15 Embedded-newline pre/post snippets
    Expected { fixture: "f15_embedded_newline_snippets", dialect: "grbl",     findings: &[(FindingKind::MissingWcs, 1)] },
    Expected { fixture: "f15_embedded_newline_snippets", dialect: "grblhal",  findings: &[] },
    Expected { fixture: "f15_embedded_newline_snippets", dialect: "linuxcnc", findings: &[(FindingKind::MissingG91_1, 1), (FindingKind::MissingProgramBrackets, 2), (FindingKind::WrongProgramEndCode, 1)] },
    Expected { fixture: "f15_embedded_newline_snippets", dialect: "mach3",    findings: &[(FindingKind::MissingWcs, 1)] },

    // F16 Cutter compensation round-trip (G41 / G40)
    Expected { fixture: "f16_comp_round_trip", dialect: "grbl",     findings: &[(FindingKind::MissingWcs, 1)] },
    Expected { fixture: "f16_comp_round_trip", dialect: "grblhal",  findings: &[] },
    Expected { fixture: "f16_comp_round_trip", dialect: "linuxcnc", findings: &[(FindingKind::MissingG91_1, 1), (FindingKind::MissingProgramBrackets, 2), (FindingKind::WrongProgramEndCode, 1)] },
    Expected { fixture: "f16_comp_round_trip", dialect: "mach3",    findings: &[(FindingKind::MissingWcs, 1)] },
];

const ALL_KINDS: &[FindingKind] = &[
    FindingKind::UnsupportedM6,
    FindingKind::MissingG91_1,
    FindingKind::WrongProgramEndCode,
    FindingKind::MissingProgramBrackets,
    FindingKind::MissingWcs,
];

#[test]
fn baseline_findings_match_expected() {
    let mut total_findings: usize = 0;
    let mut failures: Vec<String> = Vec::new();

    for expected in BASELINE {
        let gcode = read_capture(expected.fixture, expected.dialect);
        let findings = validate(&gcode, dialect_to_post(expected.dialect));
        total_findings += findings.len();

        // For each kind, check actual count matches expected (default 0).
        for &kind in ALL_KINDS {
            let actual = count_kind(&findings, kind);
            let expected_count = expected
                .findings
                .iter()
                .find(|(k, _)| *k == kind)
                .map(|(_, n)| *n)
                .unwrap_or(0);
            if actual != expected_count {
                failures.push(format!(
                    "{}_{}: expected {} {:?} finding(s), got {}",
                    expected.fixture, expected.dialect, expected_count, kind, actual
                ));
            }
        }

        // Print so the test log captures progress on driving findings down.
        let summary: Vec<String> = ALL_KINDS
            .iter()
            .filter_map(|&k| {
                let n = count_kind(&findings, k);
                if n > 0 { Some(format!("{n}×{k:?}")) } else { None }
            })
            .collect();
        println!(
            "{:24} {:9} {} finding(s){}",
            expected.fixture,
            expected.dialect,
            findings.len(),
            if summary.is_empty() {
                String::new()
            } else {
                format!(" — {}", summary.join(", "))
            }
        );
    }

    println!(
        "\nTotal findings across {} captures: {total_findings}",
        BASELINE.len()
    );

    assert!(
        failures.is_empty(),
        "Baseline mismatch ({} discrepancies):\n  {}\n\n\
         If you intentionally changed the emitter, update BASELINE in this file in the same commit.",
        failures.len(),
        failures.join("\n  ")
    );
}
