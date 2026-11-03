//! F067, F051, F061, F214 (tech-debt wave W5): one core export door.
//!
//! Before W5 only the GUI/MCP door (viz `io/export.rs`) ran the
//! machine-safety pass and the high-feedrate rapid rewrite. The core door
//! that the CLI uses ran neither, so one project emitted `G1` traverses
//! from the GUI and `G0` rapids from the CLI, and no CLI file was checked.
//! The load gate's advisory note was bound to `_note` and dropped, an
//! override discarded the refusal it waived, and the refusal text named
//! the core field `accept_exceeded`, which no surface accepts.
//!
//! This file pins the door from the core crate:
//!
//! - the rewrite follows the session post config;
//! - the machine-safety pass reads the program BEFORE the rewrite, so the
//!   findings are the same with the rewrite on or off;
//! - K6 (ruling Q6): no load row refuses; each unmodelled row is a warning
//!   with its id in the result and in the program header, and no warning
//!   names an override (none exists);
//! - the validator reads the canned-cycle fact from the post data (F214).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::export::gcode_validator::{FindingKind, validate};
use rs_cam_core::gcode::PostFormat;
use rs_cam_core::geo::P3;
use rs_cam_core::session::{
    AdoptResultArgs, Command, ProjectSession, SetPostConfigArgs, ToolpathComputeResult,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

mod common;
use common::make_endmill_6mm;
use common::session::{polygon_model, single_op_session_with, square_polygon, stock_under};

const MODEL_HALF: f64 = 10.0;
const STOCK_Z: f64 = 12.0;
const FEED: f64 = 600.0;

/// A rapid that moves in X at Z5. The stock top is world Z0 and the
/// retract plane is the post `safe_z` above it, so this traverse is below
/// the plane: the machine-safety pass must flag it.
fn sample_toolpath() -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), FEED);
    path.feed_to(P3::new(10.0, 10.0, -1.0), FEED);
    path.rapid_to(P3::new(10.0, 10.0, 5.0));
    path.rapid_to(P3::new(-10.0, 10.0, 5.0));
    path
}

fn session(high_feedrate_mode: bool) -> ProjectSession {
    let mut session = single_op_session_with(
        stock_under(MODEL_HALF, STOCK_Z),
        make_endmill_6mm(),
        polygon_model(vec![square_polygon(MODEL_HALF)], "square"),
        "Door Pocket",
        OperationConfig::Pocket(PocketConfig::default()),
        |_| {},
    );
    let revision = session.toolpath_revision_at(0);
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(ToolpathComputeResult {
                op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(
                    AnnotatedToolpath::new(sample_toolpath()),
                )),
                stats: Default::default(),
                debug_trace: None,
                semantic_trace: None,
                provenance: rs_cam_core::session::ResultProvenance::Dressed,
            }),
        }))
        .expect("adopt the hand-built result");
    let mut post = session.post_config().clone();
    post.high_feedrate_mode = high_feedrate_mode;
    let _ = session
        .apply(Command::SetPostConfig(SetPostConfigArgs {
            post: Box::new(post),
        }))
        .expect("set the post config");
    session
}

fn has_g0(gcode: &str) -> bool {
    gcode
        .lines()
        .any(|l| l.trim_start().starts_with("G0 ") || l.trim_start().starts_with("G0X"))
}

#[test]
fn the_core_door_rewrites_rapids_under_high_feedrate_mode() {
    let off = session(false)
        .export_gcode_checked(None)
        .expect("export, rewrite off");
    assert!(
        has_g0(&off.gcode),
        "the plain export keeps G0:\n{}",
        off.gcode
    );

    let on_session = session(true);
    let feed = on_session.post_config().high_feedrate;
    let on = on_session
        .export_gcode_checked(None)
        .expect("export, rewrite on");
    assert!(
        !has_g0(&on.gcode),
        "the rewrite leaves no G0:\n{}",
        on.gcode
    );
    let word = format!(" F{feed:.1}");
    assert!(
        on.gcode
            .lines()
            .any(|l| l.starts_with("G1") && l.ends_with(&word)),
        "a rewritten rapid is a G1 at the post config's high feedrate ({word}):\n{}",
        on.gcode
    );
}

#[test]
fn the_safety_pass_reads_the_program_before_the_rewrite() {
    let off = session(false)
        .export_gcode_checked(None)
        .expect("export, rewrite off");
    let on = session(true)
        .export_gcode_checked(None)
        .expect("export, rewrite on");
    let low_rapids = |findings: &[rs_cam_core::export::gcode_validator::Finding]| {
        findings
            .iter()
            .filter(|f| f.kind == FindingKind::RapidBelowClearance)
            .count()
    };
    assert!(
        low_rapids(&off.machine_safety) > 0,
        "non-vacuity: the X traverse at Z5 is below the plane: {:?}",
        off.machine_safety
    );
    assert_eq!(
        low_rapids(&on.machine_safety),
        low_rapids(&off.machine_safety),
        "the rewrite removes every G0, so a pass after it would find none"
    );
}

/// K6: no simulation runs here, so every trace-reading gate is
/// unmodelled. The export writes the program and warns on each row; the
/// header names each warning, and no text names an override.
#[test]
fn an_unmodelled_load_warns_and_the_header_names_it_k6() {
    let exported = session(false)
        .export_gcode_checked(None)
        .expect("no simulation: the export still writes the program");
    let unmodelled: Vec<_> = exported
        .warnings
        .iter()
        .filter(|w| w.id.ends_with(".unmodeled"))
        .collect();
    assert!(!unmodelled.is_empty(), "{:?}", exported.warnings);
    for warning in &unmodelled {
        assert!(
            exported.gcode.contains(&warning.header_line()),
            "the header names `{}`:\n{}",
            warning.id,
            exported.gcode
        );
        assert!(
            !warning.detail.contains("accept"),
            "no warning names an override: {}",
            warning.detail
        );
    }
}

/// F214: the validator reads `canned_drill_cycles` from the post data.
/// Only the grblHAL post sets it, so LinuxCNC and Mach3 now refuse a
/// canned cycle, as Grbl always did.
#[test]
fn the_canned_cycle_rule_reads_the_post_data() {
    let program = "G21\nG90\nG54\nM3 S10000\nG81 X0 Y0 Z-2 R2 F200\nG80\nM5\nM30\n";
    for format in PostFormat::ALL {
        let flagged = validate(program, *format)
            .iter()
            .any(|f| f.kind == FindingKind::UnsupportedWord && f.message.contains("canned"));
        assert_eq!(
            flagged,
            !format.definition().canned_drill_cycles,
            "{}: the rule follows `canned_drill_cycles`",
            format.label()
        );
    }
    assert!(
        PostFormat::GrblHal.definition().canned_drill_cycles,
        "non-vacuity: one post runs canned cycles"
    );
}
