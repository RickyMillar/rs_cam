//! Phase 0 fixture capture for the g-code export overhaul.
//!
//! Each `#[test]` builds one of the 6 fixtures (see
//! `planning/fixtures/README.md`), runs it through each shipped post
//! (Grbl / LinuxCNC / Mach3), and writes the output to
//! `planning/gcode_current_outputs/<fixture>_<dialect>.nc`.
//!
//! All tests are `#[ignore]` so they don't run as part of the normal
//! test suite. Run manually with:
//!
//! ```bash
//! cargo test --test gcode_phase0_capture -- --ignored --nocapture
//! ```
//!
//! Outputs are committed under `planning/gcode_current_outputs/` so
//! they can be diffed against the Fusion reference outputs (Phase 0 step
//! 4) without re-running the capture.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use rs_cam_core::gcode::{
    CoolantMode, GcodePhase, GcodeSetupPhase, PostDefinition, ToolLoadExportPolicy, emit_gcode,
    export_gcode_multi_setup_checked, export_gcode_phases_checked, post,
};
use rs_cam_core::geo::P3;
use rs_cam_core::toolpath::Toolpath;
use std::path::PathBuf;

fn output_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is .../crates/rs_cam_core; up two levels reaches the workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("planning")
        .join("gcode_current_outputs")
}

fn write_capture(fixture: &str, dialect: &str, gcode: &str) {
    let dir = output_dir();
    std::fs::create_dir_all(&dir).expect("create gcode_current_outputs dir");
    let path = dir.join(format!("{fixture}_{dialect}.nc"));
    std::fs::write(&path, gcode).expect("write capture");
    println!("wrote {}", path.display());
}

fn dialects() -> [(&'static str, &'static PostDefinition); 4] {
    [
        ("grbl", post::grbl()),
        ("grblhal", post::grblhal()),
        ("linuxcnc", post::linuxcnc()),
        ("mach3", post::mach3()),
    ]
}

fn capture_single(fixture: &str, tp: &Toolpath, rpm: u32) {
    for (dialect, post) in dialects() {
        write_capture(fixture, dialect, &emit_gcode(tp, post, rpm));
    }
}

fn capture_phased(fixture: &str, phases: &[GcodePhase<'_>]) {
    for (dialect, post) in dialects() {
        let gcode =
            export_gcode_phases_checked(phases, post, None, ToolLoadExportPolicy::default())
                .expect("phased emit");
        write_capture(fixture, dialect, &gcode);
    }
}

fn capture_multi_setup(fixture: &str, setups: &[GcodeSetupPhase<'_>], safe_z: f64) {
    for (dialect, post) in dialects() {
        let gcode = export_gcode_multi_setup_checked(
            setups,
            post,
            safe_z,
            None,
            ToolLoadExportPolicy::default(),
        )
        .expect("multi-setup emit");
        write_capture(fixture, dialect, &gcode);
    }
}

// ── F1 ────────────────────────────────────────────────────────────────
#[test]
#[ignore = "phase 0 fixture capture; run with --ignored"]
fn capture_f1_basic_lines() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    tp.feed_to(P3::new(10.0, 10.0, -2.0), 600.0);
    tp.feed_to(P3::new(0.0, 10.0, -2.0), 600.0);
    tp.feed_to(P3::new(0.0, 0.0, 5.0), 1000.0);
    capture_single("f1_basic_lines", &tp, 18_000);
}

// ── F2 ────────────────────────────────────────────────────────────────
#[test]
#[ignore = "phase 0 fixture capture; run with --ignored"]
fn capture_f2_arcs_xy() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    tp.arc_cw_to(P3::new(0.0, 10.0, -2.0), -10.0, 0.0, 600.0);
    tp.arc_ccw_to(P3::new(-10.0, 0.0, -2.0), 0.0, -10.0, 600.0);
    tp.feed_to(P3::new(-10.0, 0.0, 5.0), 1000.0);
    capture_single("f2_arcs_xy", &tp, 18_000);
}

// ── F3 ────────────────────────────────────────────────────────────────
#[test]
#[ignore = "phase 0 fixture capture; run with --ignored"]
fn capture_f3_helical_ramp() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, 0.0), 300.0);
    tp.arc_cw_to(P3::new(0.0, 10.0, -1.0), -10.0, 0.0, 600.0);
    tp.arc_cw_to(P3::new(-10.0, 0.0, -2.0), 0.0, -10.0, 600.0);
    tp.arc_cw_to(P3::new(0.0, -10.0, -3.0), 10.0, 0.0, 600.0);
    tp.arc_cw_to(P3::new(10.0, 0.0, -4.0), 0.0, 10.0, 600.0);
    tp.feed_to(P3::new(10.0, 0.0, 5.0), 1000.0);
    capture_single("f3_helical_ramp", &tp, 18_000);
}

// ── F4 ────────────────────────────────────────────────────────────────
#[test]
#[ignore = "phase 0 fixture capture; run with --ignored"]
fn capture_f4_profile_multipass() {
    let mut tp = Toolpath::new();
    for z in [-2.0, -4.0, -6.0] {
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp.feed_to(P3::new(0.0, 0.0, z), 300.0);
        tp.feed_to(P3::new(20.0, 0.0, z), 600.0);
        tp.feed_to(P3::new(20.0, 10.0, z), 600.0);
        tp.feed_to(P3::new(0.0, 10.0, z), 600.0);
        tp.feed_to(P3::new(0.0, 0.0, z), 600.0);
    }
    capture_single("f4_profile_multipass", &tp, 18_000);
}

// ── F5 ────────────────────────────────────────────────────────────────
#[test]
#[ignore = "phase 0 fixture capture; run with --ignored"]
fn capture_f5_two_tool_changes() {
    let mut tp1 = Toolpath::new();
    tp1.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp1.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    tp1.feed_to(P3::new(10.0, 10.0, -2.0), 600.0);

    let mut tp2 = Toolpath::new();
    tp2.rapid_to(P3::new(20.0, 0.0, 5.0));
    tp2.feed_to(P3::new(30.0, 0.0, -1.0), 300.0);
    tp2.feed_to(P3::new(30.0, 10.0, -1.0), 300.0);

    let phases = [
        GcodePhase {
            toolpath: &tp1,
            spindle_rpm: 18_000,
            label: "Op 0 — pocket T1",
            pre_gcode: None,
            post_gcode: None,
            tool_number: Some(1),
            coolant: CoolantMode::Off,
            controller_compensation: None,
        },
        GcodePhase {
            toolpath: &tp2,
            spindle_rpm: 24_000,
            label: "Op 1 — finish T2",
            pre_gcode: None,
            post_gcode: None,
            tool_number: Some(2),
            coolant: CoolantMode::Off,
            controller_compensation: None,
        },
    ];
    capture_phased("f5_two_tool_changes", &phases);
}

// ── F7  Full circle: end == start with IJK to centre ──────────────────
#[test]
#[ignore = "phase 4b corpus broaden"]
fn capture_f7_full_circle() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, -2.0), 300.0);
    // Full circle CCW around origin: end-point identical to start, IJK
    // points to centre. Some controllers require this be split into two
    // semicircles; emulator validation will tell us which.
    tp.arc_ccw_to(P3::new(10.0, 0.0, -2.0), -10.0, 0.0, 600.0);
    tp.feed_to(P3::new(10.0, 0.0, 5.0), 1000.0);
    capture_single("f7_full_circle", &tp, 18_000);
}

// ── F8  Single-axis feed (X only) ─────────────────────────────────────
#[test]
#[ignore = "phase 4b corpus broaden"]
fn capture_f8_x_only_feed() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp.feed_to(P3::new(0.0, 0.0, -1.0), 200.0);
    // Y and Z held constant, only X advances. Tests axis-word emission
    // when most words are unchanged.
    for x in [10.0, 20.0, 30.0, 40.0, 50.0] {
        tp.feed_to(P3::new(x, 0.0, -1.0), 600.0);
    }
    tp.feed_to(P3::new(50.0, 0.0, 5.0), 1000.0);
    capture_single("f8_x_only_feed", &tp, 18_000);
}

// ── F9  Ramp into arc (linear feed transitions directly to arc) ───────
#[test]
#[ignore = "phase 4b corpus broaden"]
fn capture_f9_ramp_into_arc() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    // Diagonal ramp from Z=0 down to Z=-2 over 10mm of X — entry move.
    tp.feed_to(P3::new(10.0, 0.0, -2.0), 300.0);
    // Immediately enter an arc without lifting; tests modal-state
    // continuity from G1 → G2.
    tp.arc_cw_to(P3::new(20.0, 10.0, -2.0), 0.0, 10.0, 600.0);
    tp.arc_cw_to(P3::new(10.0, 20.0, -2.0), -10.0, 0.0, 600.0);
    tp.feed_to(P3::new(10.0, 20.0, 5.0), 1000.0);
    capture_single("f9_ramp_into_arc", &tp, 18_000);
}

// ── F10 Tiny arcs (sub-0.05mm radius) — arc-linearize candidate ───────
#[test]
#[ignore = "phase 4b corpus broaden"]
fn capture_f10_tiny_arcs() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp.feed_to(P3::new(0.0, 0.0, -1.0), 200.0);
    // Five tiny arcs with R≈0.02mm. Some controllers reject sub-0.05mm
    // arcs; this fixture exercises the (future) arc_linearize toggle.
    let r = 0.02_f64;
    for k in 0..5 {
        let cx = (k as f64) * 0.5;
        tp.arc_cw_to(P3::new(cx + r, r, -1.0), r, 0.0, 400.0);
        tp.arc_cw_to(P3::new(cx, 2.0 * r, -1.0), -r, 0.0, 400.0);
    }
    tp.feed_to(P3::new(0.0, 2.0 * r, 5.0), 1000.0);
    capture_single("f10_tiny_arcs", &tp, 18_000);
}

// ── F11 Depth-step boundary (exact-Z boundary across passes) ──────────
#[test]
#[ignore = "phase 4b corpus broaden"]
fn capture_f11_depth_step_boundary() {
    let mut tp = Toolpath::new();
    // Three passes at exactly Z=-1.000, -2.000, -3.000. Tests that the
    // Z formatter doesn't drop trailing zeroes inconsistently across
    // passes (would surface as a diff in the capture).
    for z in [-1.0, -2.0, -3.0] {
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp.feed_to(P3::new(0.0, 0.0, z), 200.0);
        tp.feed_to(P3::new(10.0, 0.0, z), 600.0);
        tp.feed_to(P3::new(10.0, 10.0, z), 600.0);
        tp.feed_to(P3::new(0.0, 10.0, z), 600.0);
        tp.feed_to(P3::new(0.0, 0.0, z), 600.0);
    }
    capture_single("f11_depth_step_boundary", &tp, 18_000);
}

// ── F12 Tool change with the tool sitting at Z=0 ──────────────────────
#[test]
#[ignore = "phase 4b corpus broaden"]
fn capture_f12_tool_change_at_z_zero() {
    let mut tp1 = Toolpath::new();
    tp1.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp1.feed_to(P3::new(10.0, 0.0, 0.0), 300.0);
    // Leave the tool sitting at exactly Z=0 — the tool-change sequence
    // has to retract safely from this position.

    let mut tp2 = Toolpath::new();
    tp2.rapid_to(P3::new(20.0, 0.0, 5.0));
    tp2.feed_to(P3::new(30.0, 0.0, -1.0), 300.0);

    let phases = [
        GcodePhase {
            toolpath: &tp1,
            spindle_rpm: 18_000,
            label: "Op 0 — surface skim T1",
            pre_gcode: None,
            post_gcode: None,
            tool_number: Some(1),
            coolant: CoolantMode::Off,
            controller_compensation: None,
        },
        GcodePhase {
            toolpath: &tp2,
            spindle_rpm: 24_000,
            label: "Op 1 — finish T2",
            pre_gcode: None,
            post_gcode: None,
            tool_number: Some(2),
            coolant: CoolantMode::Off,
            controller_compensation: None,
        },
    ];
    capture_phased("f12_tool_change_at_z_zero", &phases);
}

// ── F13 Climb vs conventional (same geometry, different direction) ────
#[test]
#[ignore = "phase 4b corpus broaden"]
fn capture_f13_climb_vs_conventional() {
    // Conventional: counter-clockwise around perimeter (climb on the
    // outside of an outer profile when viewed from above).
    let mut tp_conv = Toolpath::new();
    tp_conv.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp_conv.feed_to(P3::new(0.0, 0.0, -2.0), 300.0);
    tp_conv.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    tp_conv.feed_to(P3::new(10.0, 10.0, -2.0), 600.0);
    tp_conv.feed_to(P3::new(0.0, 10.0, -2.0), 600.0);
    tp_conv.feed_to(P3::new(0.0, 0.0, -2.0), 600.0);

    // Climb: same path traversed clockwise (reverse the perimeter).
    let mut tp_climb = Toolpath::new();
    tp_climb.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp_climb.feed_to(P3::new(0.0, 0.0, -2.0), 300.0);
    tp_climb.feed_to(P3::new(0.0, 10.0, -2.0), 600.0);
    tp_climb.feed_to(P3::new(10.0, 10.0, -2.0), 600.0);
    tp_climb.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    tp_climb.feed_to(P3::new(0.0, 0.0, -2.0), 600.0);

    let phases = [
        GcodePhase {
            toolpath: &tp_conv,
            spindle_rpm: 18_000,
            label: "Op 0 — conventional CCW",
            pre_gcode: None,
            post_gcode: None,
            tool_number: Some(1),
            coolant: CoolantMode::Off,
            controller_compensation: None,
        },
        GcodePhase {
            toolpath: &tp_climb,
            spindle_rpm: 18_000,
            label: "Op 1 — climb CW",
            pre_gcode: None,
            post_gcode: None,
            tool_number: Some(1),
            coolant: CoolantMode::Off,
            controller_compensation: None,
        },
    ];
    capture_phased("f13_climb_vs_conventional", &phases);
}

// ── F14 Multi-line pause message ──────────────────────────────────────
#[test]
#[ignore = "phase 4b corpus broaden"]
fn capture_f14_multi_line_pause_message() {
    let mut tp1 = Toolpath::new();
    tp1.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp1.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);

    let mut tp2 = Toolpath::new();
    tp2.rapid_to(P3::new(20.0, 0.0, 5.0));
    tp2.feed_to(P3::new(30.0, 0.0, -2.0), 600.0);

    let setups = [
        GcodeSetupPhase {
            setup_label: "Top",
            phases: vec![GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18_000,
                label: "Top pocket",
                pre_gcode: None,
                post_gcode: None,
                tool_number: Some(1),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            }],
            pause_message: None,
        },
        // Setup label deliberately contains a newline — the program-
        // pause renderer must handle multi-line comment messages
        // without breaking comment syntax (each line wrapped, or the
        // newline escaped, depending on dialect).
        GcodeSetupPhase {
            setup_label: "Bottom\nFlip stock 180 then resume",
            phases: vec![GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 18_000,
                label: "Bottom profile",
                pre_gcode: None,
                post_gcode: None,
                tool_number: Some(1),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            }],
            pause_message: None,
        },
    ];
    capture_multi_setup("f14_multi_line_pause_message", &setups, 25.0);
}

// ── F15 Embedded-newline pre/post gcode snippets ──────────────────────
#[test]
#[ignore = "phase 4b corpus broaden"]
fn capture_f15_embedded_newline_snippets() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    tp.feed_to(P3::new(10.0, 10.0, -2.0), 600.0);

    let phases = [GcodePhase {
        toolpath: &tp,
        spindle_rpm: 18_000,
        label: "Op 0 — pocket with custom prep",
        // Multi-line pre/post snippets — emitter must preserve
        // intermediate newlines and not collapse blank lines.
        pre_gcode: Some("(custom prep)\nM7\nG4 P0.5"),
        post_gcode: Some("M9\n(custom retract)\nG0 Z20.000"),
        tool_number: Some(1),
        coolant: CoolantMode::Off,
        controller_compensation: None,
    }];
    capture_phased("f15_embedded_newline_snippets", &phases);
}

// ── F16 Cutter compensation round-trip (G41 → G40) ────────────────────
#[test]
#[ignore = "phase 4b corpus broaden"]
fn capture_f16_comp_round_trip() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp.feed_to(P3::new(0.0, 0.0, -2.0), 300.0);
    tp.feed_to(P3::new(20.0, 0.0, -2.0), 600.0);
    tp.feed_to(P3::new(20.0, 20.0, -2.0), 600.0);
    tp.feed_to(P3::new(0.0, 20.0, -2.0), 600.0);
    tp.feed_to(P3::new(0.0, 0.0, -2.0), 600.0);
    tp.feed_to(P3::new(0.0, 0.0, 5.0), 1000.0);

    let phases = [GcodePhase {
        toolpath: &tp,
        spindle_rpm: 18_000,
        label: "Op 0 — left-comp profile",
        pre_gcode: None,
        post_gcode: None,
        tool_number: Some(3),
        coolant: CoolantMode::Off,
        // G41 D3 emitted before first cutting move; G40 emitted at
        // end of phase. Validates the comp_started bookkeeping in
        // program_builder.
        controller_compensation: Some(rs_cam_core::gcode::ControllerCompensation::Left),
    }];
    capture_phased("f16_comp_round_trip", &phases);
}

// ── F6 ────────────────────────────────────────────────────────────────
#[test]
#[ignore = "phase 0 fixture capture; run with --ignored"]
fn capture_f6_two_setups() {
    let mut tp1 = Toolpath::new();
    tp1.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp1.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    tp1.feed_to(P3::new(10.0, 10.0, -2.0), 600.0);

    let mut tp2 = Toolpath::new();
    tp2.rapid_to(P3::new(20.0, 0.0, 5.0));
    tp2.feed_to(P3::new(30.0, 0.0, -1.0), 300.0);
    tp2.feed_to(P3::new(30.0, 10.0, -1.0), 300.0);

    let setups = [
        GcodeSetupPhase {
            setup_label: "Top",
            phases: vec![GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18_000,
                label: "Pocket",
                pre_gcode: None,
                post_gcode: None,
                tool_number: Some(1),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            }],
            pause_message: None,
        },
        GcodeSetupPhase {
            setup_label: "Bottom",
            phases: vec![GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 18_000,
                label: "Profile",
                pre_gcode: None,
                post_gcode: None,
                tool_number: Some(1),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            }],
            pause_message: None,
        },
    ];
    capture_multi_setup("f6_two_setups", &setups, 25.0);
}
