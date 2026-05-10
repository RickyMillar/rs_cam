//! Phase 3 byte-parity check: the data-driven `Emitter` (Program + PostDefinition)
//! must produce the same bytes as the legacy trait-based `PostProcessor`
//! for every shipped dialect on every Phase 0 fixture.
//!
//! Once this test is green for an extended window, the legacy
//! `PostProcessor` trait + three impls can be deleted (Phase 3 final
//! step). Until then both paths coexist so any regression in either
//! direction is caught here.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::gcode::emitter as new_emitter;
use rs_cam_core::gcode::post::{self, PostDefinition};
use rs_cam_core::gcode::program_builder;
use rs_cam_core::gcode::{
    CoolantMode, GcodePhase, GcodeSetupPhase, GrblPost, LinuxCncPost, Mach3Post, PostProcessor,
    emit_program as legacy_emit_program,
};
use rs_cam_core::geo::P3;
use rs_cam_core::toolpath::Toolpath;

/// All three shipped dialects, paired (legacy trait impl, new TOML def).
fn dialect_pairs() -> Vec<(&'static str, Box<dyn PostProcessor>, &'static PostDefinition)> {
    vec![
        ("grbl", Box::new(GrblPost), post::grbl()),
        ("linuxcnc", Box::new(LinuxCncPost), post::linuxcnc()),
        ("mach3", Box::new(Mach3Post), post::mach3()),
    ]
}

fn assert_parity(fixture: &str, build: impl Fn() -> rs_cam_core::gcode::Program) {
    for (name, legacy, new_def) in dialect_pairs() {
        let program = build();
        let legacy_out = legacy_emit_program(&program, legacy.as_ref());
        let new_out = new_emitter::emit_program(&program, new_def);
        assert_eq!(
            legacy_out, new_out,
            "byte-parity broken for fixture={fixture} dialect={name}\n\
             ===== legacy =====\n{legacy_out}\n\
             ===== new =====\n{new_out}",
        );
    }
}

// ---- F1 — basic lines ----
#[test]
fn parity_f1_basic_lines() {
    assert_parity("f1_basic_lines", || {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
        tp.feed_to(P3::new(10.0, 10.0, -2.0), 600.0);
        tp.feed_to(P3::new(0.0, 10.0, -2.0), 600.0);
        tp.feed_to(P3::new(0.0, 0.0, 5.0), 1000.0);
        program_builder::build_single(&tp, 18_000)
    });
}

// ---- F2 — XY arcs ----
#[test]
fn parity_f2_arcs_xy() {
    assert_parity("f2_arcs_xy", || {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 0.0, 5.0));
        tp.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
        tp.arc_cw_to(P3::new(0.0, 10.0, -2.0), -10.0, 0.0, 600.0);
        tp.arc_ccw_to(P3::new(-10.0, 0.0, -2.0), 0.0, -10.0, 600.0);
        tp.feed_to(P3::new(-10.0, 0.0, 5.0), 1000.0);
        program_builder::build_single(&tp, 18_000)
    });
}

// ---- F3 — helical ramp ----
#[test]
fn parity_f3_helical_ramp() {
    assert_parity("f3_helical_ramp", || {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 0.0, 5.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 300.0);
        tp.arc_cw_to(P3::new(0.0, 10.0, -1.0), -10.0, 0.0, 600.0);
        tp.arc_cw_to(P3::new(-10.0, 0.0, -2.0), 0.0, -10.0, 600.0);
        tp.arc_cw_to(P3::new(0.0, -10.0, -3.0), 10.0, 0.0, 600.0);
        tp.arc_cw_to(P3::new(10.0, 0.0, -4.0), 0.0, 10.0, 600.0);
        tp.feed_to(P3::new(10.0, 0.0, 5.0), 1000.0);
        program_builder::build_single(&tp, 18_000)
    });
}

// ---- F4 — multi-pass profile ----
#[test]
fn parity_f4_profile_multipass() {
    assert_parity("f4_profile_multipass", || {
        let mut tp = Toolpath::new();
        for z in [-2.0, -4.0, -6.0] {
            tp.rapid_to(P3::new(0.0, 0.0, 5.0));
            tp.feed_to(P3::new(0.0, 0.0, z), 300.0);
            tp.feed_to(P3::new(20.0, 0.0, z), 600.0);
            tp.feed_to(P3::new(20.0, 10.0, z), 600.0);
            tp.feed_to(P3::new(0.0, 10.0, z), 600.0);
            tp.feed_to(P3::new(0.0, 0.0, z), 600.0);
        }
        program_builder::build_single(&tp, 18_000)
    });
}

// ---- F5 — phased emission with two tool changes ----
#[test]
fn parity_f5_two_tool_changes() {
    let mut tp1 = Toolpath::new();
    tp1.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp1.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    tp1.feed_to(P3::new(10.0, 10.0, -2.0), 600.0);

    let mut tp2 = Toolpath::new();
    tp2.rapid_to(P3::new(20.0, 0.0, 5.0));
    tp2.feed_to(P3::new(30.0, 0.0, -1.0), 300.0);
    tp2.feed_to(P3::new(30.0, 10.0, -1.0), 300.0);

    assert_parity("f5_two_tool_changes", || {
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
        program_builder::build_phased(&phases)
    });
}

// ---- F6 — multi-setup with M0 between setups ----
#[test]
fn parity_f6_two_setups() {
    let mut tp1 = Toolpath::new();
    tp1.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp1.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    tp1.feed_to(P3::new(10.0, 10.0, -2.0), 600.0);

    let mut tp2 = Toolpath::new();
    tp2.rapid_to(P3::new(20.0, 0.0, 5.0));
    tp2.feed_to(P3::new(30.0, 0.0, -1.0), 300.0);
    tp2.feed_to(P3::new(30.0, 10.0, -1.0), 300.0);

    assert_parity("f6_two_setups", || {
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
        program_builder::build_multi_setup(&setups, 25.0)
    });
}

// ---- additional: phased with coolant + controller comp + pre/post snippets ----
//
// The Phase 0 captures don't exercise these code paths, but the legacy
// trait does — so prove parity here too. Catches edge cases the simple
// fixtures miss (M7/M8/M9 transitions, G41 D{n}, raw pre/post gcode).
#[test]
fn parity_phased_coolant_and_comp() {
    use rs_cam_core::gcode::ControllerCompensation;

    let mut tp1 = Toolpath::new();
    tp1.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp1.feed_to(P3::new(10.0, 0.0, -2.0), 800.0);

    let mut tp2 = Toolpath::new();
    tp2.rapid_to(P3::new(20.0, 0.0, 5.0));
    tp2.feed_to(P3::new(30.0, 0.0, -1.0), 400.0);

    assert_parity("phased_coolant_and_comp", || {
        let phases = [
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18_000,
                label: "Op 0 — flood roughing",
                pre_gcode: Some("G55"),
                post_gcode: Some("(end op 0)"),
                tool_number: Some(1),
                coolant: CoolantMode::Flood,
                controller_compensation: Some(ControllerCompensation::Left),
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 24_000,
                label: "Op 1 — mist finish",
                pre_gcode: None,
                post_gcode: None,
                tool_number: Some(2),
                coolant: CoolantMode::Mist,
                controller_compensation: Some(ControllerCompensation::Right),
            },
        ];
        program_builder::build_phased(&phases)
    });
}
