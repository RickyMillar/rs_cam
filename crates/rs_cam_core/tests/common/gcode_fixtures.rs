//! The G-code golden corpus: one builder per fixture (G9, 2026-10-08).
//!
//! Each builder emits its program for one post through the same checked
//! door production uses. `gcode_phase0_capture` compares the bytes with
//! the goldens in `tests/fixtures/gcode_golden/`. `gcode_validator_baseline`
//! and `gcode_emulator_validation` read the same goldens.
//!
//! A builder returns `Err` when the door refuses the fixture on that post
//! (for example a controller-compensation program on a post without
//! cutter compensation). A refused pair has no golden file.

use rs_cam_core::gcode::{
    CoolantMode, ExportError, GcodePhase, GcodeSetupPhase, PhaseDrill, PhaseTool, PostDefinition,
    PostFormat, ToolLoadExportPolicy, WizardOverlay, emitter,
    export_gcode_multi_setup_with_overlay_checked, export_gcode_phases_with_overlay_checked,
    program_builder,
};
use rs_cam_core::geo::P3;
use rs_cam_core::toolpath::Toolpath;
use std::path::PathBuf;

/// One fixture builder.
pub type FixtureFn = fn(&PostDefinition, &WizardOverlay) -> Result<String, ExportError>;

/// The four shipped dialects, by golden file suffix.
pub const DIALECTS: [(&str, PostFormat); 4] = [
    ("grbl", PostFormat::Grbl),
    ("grblhal", PostFormat::GrblHal),
    ("linuxcnc", PostFormat::LinuxCnc),
    ("mach3", PostFormat::Mach3),
];

/// The directory that holds the goldens.
pub fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("gcode_golden")
}

/// The golden path for one fixture and one dialect.
pub fn golden_path(fixture: &str, dialect: &str) -> PathBuf {
    golden_dir().join(format!("{fixture}_{dialect}.nc"))
}

/// The fixtures have no project: pass the explicitly empty "no load
/// evaluation performed" report (C1).
fn no_load_evaluation() -> rs_cam_core::tool_load::ToolLoadReport {
    rs_cam_core::tool_load::ToolLoadReport {
        per_toolpath: vec![],
    }
}

fn single(
    tp: &Toolpath,
    rpm: u32,
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    let program = program_builder::build_single(tp, rpm);
    Ok(emitter::emit_program_with_overlay(&program, post, overlay))
}

fn phased(
    phases: &[GcodePhase<'_>],
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    export_gcode_phases_with_overlay_checked(
        phases,
        post,
        &no_load_evaluation(),
        ToolLoadExportPolicy::default(),
        overlay,
    )
}

fn multi(
    setups: &[GcodeSetupPhase<'_>],
    safe_z: f64,
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    export_gcode_multi_setup_with_overlay_checked(
        setups,
        post,
        safe_z,
        &no_load_evaluation(),
        ToolLoadExportPolicy::default(),
        overlay,
    )
}

/// Every fixture, by golden file stem.
pub const FIXTURES: &[(&str, FixtureFn)] = &[
    ("f1_basic_lines", f1_basic_lines),
    ("f2_arcs_xy", f2_arcs_xy),
    ("f3_helical_ramp", f3_helical_ramp),
    ("f4_profile_multipass", f4_profile_multipass),
    ("f5_two_tool_changes", f5_two_tool_changes),
    ("f7_full_circle", f7_full_circle),
    ("f8_x_only_feed", f8_x_only_feed),
    ("f9_ramp_into_arc", f9_ramp_into_arc),
    ("f10_tiny_arcs", f10_tiny_arcs),
    ("f11_depth_step_boundary", f11_depth_step_boundary),
    ("f12_tool_change_at_z_zero", f12_tool_change_at_z_zero),
    ("f13_climb_vs_conventional", f13_climb_vs_conventional),
    ("f14_multi_line_pause_message", f14_multi_line_pause_message),
    (
        "f15_embedded_newline_snippets",
        f15_embedded_newline_snippets,
    ),
    ("f16_comp_round_trip", f16_comp_round_trip),
    ("f6_two_setups", f6_two_setups),
    ("f17_drill_dwell_and_peck", f17_drill_dwell_and_peck),
    ("f18_grblhal_options", f18_grblhal_options),
];

// ── F1 ────────────────────────────────────────────────────────────────
pub fn f1_basic_lines(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    tp.feed_to(P3::new(10.0, 10.0, -2.0), 600.0);
    tp.feed_to(P3::new(0.0, 10.0, -2.0), 600.0);
    tp.feed_to(P3::new(0.0, 0.0, 5.0), 1000.0);
    single(&tp, 18_000, post, overlay)
}

// ── F2 ────────────────────────────────────────────────────────────────
pub fn f2_arcs_xy(post: &PostDefinition, overlay: &WizardOverlay) -> Result<String, ExportError> {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    tp.arc_cw_to(P3::new(0.0, 10.0, -2.0), -10.0, 0.0, 600.0);
    tp.arc_ccw_to(P3::new(-10.0, 0.0, -2.0), 0.0, -10.0, 600.0);
    tp.feed_to(P3::new(-10.0, 0.0, 5.0), 1000.0);
    single(&tp, 18_000, post, overlay)
}

// ── F3 ────────────────────────────────────────────────────────────────
pub fn f3_helical_ramp(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, 0.0), 300.0);
    tp.arc_cw_to(P3::new(0.0, 10.0, -1.0), -10.0, 0.0, 600.0);
    tp.arc_cw_to(P3::new(-10.0, 0.0, -2.0), 0.0, -10.0, 600.0);
    tp.arc_cw_to(P3::new(0.0, -10.0, -3.0), 10.0, 0.0, 600.0);
    tp.arc_cw_to(P3::new(10.0, 0.0, -4.0), 0.0, 10.0, 600.0);
    tp.feed_to(P3::new(10.0, 0.0, 5.0), 1000.0);
    single(&tp, 18_000, post, overlay)
}

// ── F4 ────────────────────────────────────────────────────────────────
pub fn f4_profile_multipass(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    let mut tp = Toolpath::new();
    for z in [-2.0, -4.0, -6.0] {
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp.feed_to(P3::new(0.0, 0.0, z), 300.0);
        tp.feed_to(P3::new(20.0, 0.0, z), 600.0);
        tp.feed_to(P3::new(20.0, 10.0, z), 600.0);
        tp.feed_to(P3::new(0.0, 10.0, z), 600.0);
        tp.feed_to(P3::new(0.0, 0.0, z), 600.0);
    }
    single(&tp, 18_000, post, overlay)
}

// ── F5 ────────────────────────────────────────────────────────────────
pub fn f5_two_tool_changes(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
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
            tool: Some(PhaseTool {
                id: 1,
                number: 1,
                label: "Tool One",
            }),
            coolant: CoolantMode::Off,
            controller_compensation: None,
            drill: None,
        },
        GcodePhase {
            toolpath: &tp2,
            spindle_rpm: 24_000,
            label: "Op 1 — finish T2",
            pre_gcode: None,
            post_gcode: None,
            tool: Some(PhaseTool {
                id: 2,
                number: 2,
                label: "Tool Two",
            }),
            coolant: CoolantMode::Off,
            controller_compensation: None,
            drill: None,
        },
    ];
    phased(&phases, post, overlay)
}

// ── F7  Full circle: end == start with IJK to centre ──────────────────
pub fn f7_full_circle(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, -2.0), 300.0);
    // Full circle CCW around origin: end-point identical to start, IJK
    // points to centre. Some controllers require this be split into two
    // semicircles; emulator validation will tell us which.
    tp.arc_ccw_to(P3::new(10.0, 0.0, -2.0), -10.0, 0.0, 600.0);
    tp.feed_to(P3::new(10.0, 0.0, 5.0), 1000.0);
    single(&tp, 18_000, post, overlay)
}

// ── F8  Single-axis feed (X only) ─────────────────────────────────────
pub fn f8_x_only_feed(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp.feed_to(P3::new(0.0, 0.0, -1.0), 200.0);
    // Y and Z held constant, only X advances. Tests axis-word emission
    // when most words are unchanged.
    for x in [10.0, 20.0, 30.0, 40.0, 50.0] {
        tp.feed_to(P3::new(x, 0.0, -1.0), 600.0);
    }
    tp.feed_to(P3::new(50.0, 0.0, 5.0), 1000.0);
    single(&tp, 18_000, post, overlay)
}

// ── F9  Ramp into arc (linear feed transitions directly to arc) ───────
pub fn f9_ramp_into_arc(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    // Diagonal ramp from Z=0 down to Z=-2 over 10mm of X — entry move.
    tp.feed_to(P3::new(10.0, 0.0, -2.0), 300.0);
    // Immediately enter an arc without lifting; tests modal-state
    // continuity from G1 → G2.
    tp.arc_cw_to(P3::new(20.0, 10.0, -2.0), 0.0, 10.0, 600.0);
    tp.arc_cw_to(P3::new(10.0, 20.0, -2.0), -10.0, 0.0, 600.0);
    tp.feed_to(P3::new(10.0, 20.0, 5.0), 1000.0);
    single(&tp, 18_000, post, overlay)
}

// ── F10 Tiny arcs (sub-0.05mm radius) — arc-linearize candidate ───────
pub fn f10_tiny_arcs(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
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
    single(&tp, 18_000, post, overlay)
}

// ── F11 Depth-step boundary (exact-Z boundary across passes) ──────────
pub fn f11_depth_step_boundary(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
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
    single(&tp, 18_000, post, overlay)
}

// ── F12 Tool change with the tool sitting at Z=0 ──────────────────────
pub fn f12_tool_change_at_z_zero(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
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
            tool: Some(PhaseTool {
                id: 1,
                number: 1,
                label: "Tool One",
            }),
            coolant: CoolantMode::Off,
            controller_compensation: None,
            drill: None,
        },
        GcodePhase {
            toolpath: &tp2,
            spindle_rpm: 24_000,
            label: "Op 1 — finish T2",
            pre_gcode: None,
            post_gcode: None,
            tool: Some(PhaseTool {
                id: 2,
                number: 2,
                label: "Tool Two",
            }),
            coolant: CoolantMode::Off,
            controller_compensation: None,
            drill: None,
        },
    ];
    phased(&phases, post, overlay)
}

// ── F13 Climb vs conventional (same geometry, different direction) ────
pub fn f13_climb_vs_conventional(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
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
            tool: Some(PhaseTool {
                id: 1,
                number: 1,
                label: "Tool One",
            }),
            coolant: CoolantMode::Off,
            controller_compensation: None,
            drill: None,
        },
        GcodePhase {
            toolpath: &tp_climb,
            spindle_rpm: 18_000,
            label: "Op 1 — climb CW",
            pre_gcode: None,
            post_gcode: None,
            tool: Some(PhaseTool {
                id: 1,
                number: 1,
                label: "Tool One",
            }),
            coolant: CoolantMode::Off,
            controller_compensation: None,
            drill: None,
        },
    ];
    phased(&phases, post, overlay)
}

// ── F14 Multi-line pause message ──────────────────────────────────────
pub fn f14_multi_line_pause_message(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
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
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
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
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
            }],
            pause_message: None,
        },
    ];
    multi(&setups, 25.0, post, overlay)
}

// ── F15 Embedded-newline pre/post gcode snippets ──────────────────────
pub fn f15_embedded_newline_snippets(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
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
        tool: Some(PhaseTool {
            id: 1,
            number: 1,
            label: "Tool One",
        }),
        coolant: CoolantMode::Off,
        controller_compensation: None,
        drill: None,
    }];
    phased(&phases, post, overlay)
}

// ── F16 Cutter compensation round-trip (G41 → G40) ────────────────────
pub fn f16_comp_round_trip(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
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
        tool: Some(PhaseTool {
            id: 3,
            number: 3,
            label: "Tool Three",
        }),
        coolant: CoolantMode::Off,
        // G41 D3 emitted before first cutting move; G40 emitted at
        // end of phase. Validates the comp_started bookkeeping in
        // program_builder.
        controller_compensation: Some(rs_cam_core::gcode::ControllerCompensation::Left),
        drill: None,
    }];
    phased(&phases, post, overlay)
}

// ── F6 ────────────────────────────────────────────────────────────────
pub fn f6_two_setups(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
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
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
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
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
            }],
            pause_message: None,
        },
    ];
    multi(&setups, 25.0, post, overlay)
}

// ── F17 Drill: G82 dwell and G83 peck, expanded (G7) ─────────────────
fn drill_phase<'a>(
    tp: &'a Toolpath,
    label: &'a str,
    tool: PhaseTool<'a>,
    cycle: rs_cam_core::ops::drill::DrillCycle,
    coolant: CoolantMode,
) -> GcodePhase<'a> {
    GcodePhase {
        toolpath: tp,
        spindle_rpm: 12_000,
        label,
        pre_gcode: None,
        post_gcode: None,
        tool: Some(tool),
        coolant,
        controller_compensation: None,
        drill: Some(PhaseDrill { cycle }),
    }
}

fn drill_tp(cycle: rs_cam_core::ops::drill::DrillCycle) -> Toolpath {
    use rs_cam_core::ops::drill::{DrillParams, drill_toolpath};
    drill_toolpath(
        &[[10.0, 10.0], [30.0, 10.0]],
        &DrillParams {
            depth: 6.0,
            top_z: 0.0,
            cycle,
            feed_rate: 250.0,
            safe_z: 5.0,
            retract_z: 2.0,
        },
    )
}

pub fn f17_drill_dwell_and_peck(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    use rs_cam_core::ops::drill::DrillCycle;
    let dwell_tp = drill_tp(DrillCycle::Dwell(0.5));
    let peck_tp = drill_tp(DrillCycle::Peck(2.5));
    let tool = PhaseTool {
        id: 4,
        number: 4,
        label: "Drill 3mm",
    };
    let phases = [
        drill_phase(
            &dwell_tp,
            "Spot dwell",
            tool,
            DrillCycle::Dwell(0.5),
            CoolantMode::Off,
        ),
        drill_phase(
            &peck_tp,
            "Peck holes",
            tool,
            DrillCycle::Peck(2.5),
            CoolantMode::Off,
        ),
    ];
    phased(&phases, post, overlay)
}

// ── F18 grblHAL export options (G5, G6, G7) ──────────────────────────
/// Every grblHAL option on: the controller waits for the spindle (the
/// 3 s warm-up dwell is dropped), a mist output (M7 is written), native
/// drill cycles, and the S range 6000..18000 (S24000 is clamped). On the
/// other posts the native cycles do not apply.
pub fn f18_grblhal_options(
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    use rs_cam_core::ops::drill::DrillCycle;
    let overlay = WizardOverlay {
        spindle_warmup_secs: 3,
        controller_waits_for_spindle: true,
        mist_output: Some(true),
        native_drill_cycles: true,
        rpm_range: Some((6_000.0, 18_000.0)),
        ..*overlay
    };
    let dwell_tp = drill_tp(DrillCycle::Dwell(0.5));
    let peck_tp = drill_tp(DrillCycle::Peck(2.5));
    let mut pocket = Toolpath::new();
    pocket.rapid_to(P3::new(0.0, 0.0, 5.0));
    pocket.feed_to(P3::new(10.0, 0.0, -2.0), 600.0);
    let drill = PhaseTool {
        id: 4,
        number: 4,
        label: "Drill 3mm",
    };
    let mut phases = vec![
        drill_phase(
            &dwell_tp,
            "Spot dwell",
            drill,
            DrillCycle::Dwell(0.5),
            CoolantMode::Mist,
        ),
        drill_phase(
            &peck_tp,
            "Peck holes",
            drill,
            DrillCycle::Peck(2.5),
            CoolantMode::Mist,
        ),
    ];
    phases.push(GcodePhase {
        toolpath: &pocket,
        spindle_rpm: 24_000,
        label: "Pocket",
        pre_gcode: None,
        post_gcode: None,
        tool: Some(PhaseTool {
            id: 1,
            number: 1,
            label: "End Mill 6mm",
        }),
        coolant: CoolantMode::Off,
        controller_compensation: None,
        drill: None,
    });
    phased(&phases, post, &overlay)
}
