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
        },
    ];
    capture_multi_setup("f6_two_setups", &setups, 25.0);
}
