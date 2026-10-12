//! R1 (2026-10-12): the pin holes are drilled by size.
//!
//! # The defect
//!
//! `AlignmentPinDrillConfig` had no diameter, and the generation pass
//! copied EVERY stock pin into the holes of the one pin-drill operation.
//! The mixed preset (two pins of two diameters) was therefore not
//! machinable: the large hole got the small drill, and the large dowel did
//! not go in.
//!
//! # What this test measures
//!
//! 1. Two pin-drill operations, one of 6 mm and one of 8 mm, on a mixed
//!    set: each generated toolpath drills ONLY the holes of its own size.
//! 2. The bindings: one row per pin size, each with the operation and the
//!    tool of that size; a tool of another size is a pin-tool finding.
//! 3. A size with no operation takes a tool of its size, never another
//!    size.
//! 4. The hole sources: a drill operation on the 8 mm pin is "drilled
//!    twice" only when an 8 mm pin-drill operation exists.
//! 5. The setup notes say "drills the holes" only when every pin size is
//!    drilled in that setup.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::alignment_pins::{
    AlignmentPin, DrilledHole, HoleOp, HoleOpKind, HoleSourceKind, PinFindingKind,
    hole_source_findings, pin_diameters, pin_holes_of_diameter,
};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{AlignmentPinDrillConfig, DrillCycleType};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

const SMALL: AlignmentPin = AlignmentPin {
    x: 10.0,
    y: 50.0,
    diameter: 6.0,
};
const LARGE: AlignmentPin = AlignmentPin {
    x: 90.0,
    y: 50.0,
    diameter: 8.0,
};

fn drill(id: usize, diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(id), ToolType::VBit);
    t.diameter = diameter;
    t
}

fn pin_drill_op(diameter: f64, tool_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: format!("Pin Drill {diameter:.2} mm"),
        enabled: true,
        operation: OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
            diameter,
            cycle: DrillCycleType::Simple,
            ..AlignmentPinDrillConfig::default()
        }),
        dressups: DressupConfig::for_op(OperationType::AlignmentPinDrill),
        heights: HeightsConfig::default(),
        tool_id,
        model_id: usize::MAX,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn stock() -> StockConfig {
    StockConfig {
        x: 100.0,
        y: 100.0,
        z: 20.0,
        origin_z: -20.0,
        auto_from_model: false,
        alignment_pins: vec![SMALL, LARGE],
        ..StockConfig::default()
    }
}

/// A session with the 6 mm and 8 mm drills and, for each size in
/// `op_sizes`, a pin-drill operation with the drill of `tool_of(size)`.
fn session(op_sizes: &[(f64, usize)]) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock())
        .tool(drill(6, 6.0))
        .tool(drill(8, 8.0));
    for &(d, tool) in op_sizes {
        let _ = builder.add_toolpath(0, pin_drill_op(d, tool)).unwrap();
    }
    builder.build()
}

/// The distinct XY positions (um) of the moves of toolpath `index`.
fn drilled_xy(session: &mut ProjectSession, index: usize) -> Vec<(i64, i64)> {
    let cancel = AtomicBool::new(false);
    let result = session.generate_toolpath(index, &cancel).expect("generate");
    let mut xys: Vec<(i64, i64)> = result
        .op_data
        .annotated()
        .toolpath
        .moves
        .iter()
        .map(|m| {
            (
                (m.target.x * 1000.0).round() as i64,
                (m.target.y * 1000.0).round() as i64,
            )
        })
        .collect();
    xys.sort_unstable();
    xys.dedup();
    xys
}

fn um(p: &AlignmentPin) -> (i64, i64) {
    ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64)
}

#[test]
fn the_size_helpers_group_by_cad_rounding() {
    let pins = vec![
        AlignmentPin::new(0.0, 0.0, 8.0),
        AlignmentPin::new(1.0, 0.0, 5.999),
        AlignmentPin::new(2.0, 0.0, 6.0),
    ];
    assert_eq!(pin_diameters(&pins), vec![5.999, 8.0]);
    assert_eq!(
        pin_holes_of_diameter(&pins, 6.0),
        vec![[1.0, 0.0], [2.0, 0.0]]
    );
}

#[test]
fn each_pin_drill_drills_only_its_own_size() {
    let mut s = session(&[(6.0, 6), (8.0, 8)]);
    let small = drilled_xy(&mut s, 0);
    let large = drilled_xy(&mut s, 1);
    assert!(small.contains(&um(&SMALL)), "{small:?}");
    assert!(
        !small.contains(&um(&LARGE)),
        "the 6 mm drill must not drill the 8 mm hole: {small:?}"
    );
    assert!(large.contains(&um(&LARGE)), "{large:?}");
    assert!(
        !large.contains(&um(&SMALL)),
        "the 8 mm drill must not drill the 6 mm hole: {large:?}"
    );
}

#[test]
fn the_bindings_name_one_drill_per_size() {
    let s = session(&[(6.0, 6), (8.0, 8)]);
    let bindings = s.pin_tool_bindings();
    assert_eq!(bindings.len(), 2);
    assert_eq!(
        (
            bindings[0].pin_diameter,
            bindings[0].op_index,
            bindings[0].tool_id
        ),
        (6.0, Some(0), Some(6))
    );
    assert_eq!(
        (
            bindings[1].pin_diameter,
            bindings[1].op_index,
            bindings[1].tool_id
        ),
        (8.0, Some(1), Some(8))
    );
    assert!(
        s.pin_placement_findings()
            .iter()
            .all(|f| f.kind != PinFindingKind::PinToolMismatch),
        "every size has a drill of its size"
    );

    // The 8 mm operation bound to the 6 mm drill: a finding names it.
    let wrong = session(&[(6.0, 6), (8.0, 6)]);
    let mismatch: Vec<_> = wrong
        .pin_placement_findings()
        .into_iter()
        .filter(|f| f.kind == PinFindingKind::PinToolMismatch)
        .collect();
    assert_eq!(mismatch.len(), 1, "{mismatch:?}");
    assert!(
        mismatch[0].text.contains("8.00 mm") && mismatch[0].text.contains("6.00 mm"),
        "{mismatch:?}"
    );
}

#[test]
fn a_size_without_an_operation_takes_a_tool_of_its_size() {
    let s = session(&[(6.0, 6)]);
    assert_eq!(s.pin_drill_tool_for(6.0), Some(6));
    assert_eq!(
        s.pin_drill_tool_for(8.0),
        Some(8),
        "the 8 mm drill, never the 6 mm one"
    );
    assert_eq!(s.pin_drill_tool_for(10.0), None, "no 10 mm tool");
    let bindings = s.pin_tool_bindings();
    assert_eq!(bindings[1].op_index, None);
    assert_eq!(bindings[1].tool_diameter, Some(8.0));
}

#[test]
fn drilled_twice_is_judged_per_size() {
    let op = HoleOp {
        toolpath_id: 7,
        name: "Dowel drawing".to_owned(),
        kind: HoleOpKind::Drill,
        holes: vec![DrilledHole {
            x: LARGE.x,
            y: LARGE.y,
            diameter: Some(LARGE.diameter),
        }],
    };
    let pins = [SMALL, LARGE];
    // Only the 6 mm size has a pin-drill operation: the 8 mm hole of the
    // drawing is its one source.
    assert!(
        hole_source_findings(&pins, &[6.0], std::slice::from_ref(&op))
            .iter()
            .all(|f| f.kind != HoleSourceKind::DrilledTwice)
    );
    // With an 8 mm pin-drill operation, it is drilled twice.
    assert!(
        hole_source_findings(&pins, &[6.0, 8.0], &[op])
            .iter()
            .any(|f| f.kind == HoleSourceKind::DrilledTwice)
    );
}

#[test]
fn the_setup_notes_need_every_size_drilled() {
    use rs_cam_core::compute::transform::FaceUp;
    use rs_cam_core::session::{AddSetupArgs, Command};

    for (ops, drills) in [(&[(6.0, 6)][..], false), (&[(6.0, 6), (8.0, 8)][..], true)] {
        let mut s = session(ops);
        let _ = s
            .apply(Command::AddSetup(AddSetupArgs {
                name: Some("Flip".to_owned()),
                face_up: FaceUp::Bottom,
            }))
            .unwrap();
        let setup_id = s.list_setups()[0].id;
        let notes = s.pin_setup_notes(setup_id).expect("a flip exists");
        assert_eq!(notes.drills_holes, drills, "ops {ops:?}");
    }
}
