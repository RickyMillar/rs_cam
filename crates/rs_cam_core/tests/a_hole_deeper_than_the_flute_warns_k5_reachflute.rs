//! K5 ReachFlute (feeds contract P7): a cut deeper below the top than the
//! flute is long warns, with no mesh.
//!
//! The case is op 3 of the blind review (report 04 (a)): the 6 mm 2F flat
//! end mill (flute 25 mm, estimated; stickout 30 mm, estimated) drills the
//! flip pins 30 mm deep, the stock 25.5 plus 4.5 into the spoilboard. Before
//! K5 no check fired: every flute check read a per-pass depth or a mesh, and
//! op 3 has neither. The pre-simulation row reads the emitted moves and the
//! stock box. Ruling Q6: generation and export always give output.
//!
//! The simulation stage (P7 commit B, the ninth shared decision
//! `compute::simulate::flute_reach`) measures the wood over the tip above
//! the flute on the carved stock (`SimulationResult::flute_overreach`).
//! A fresh run supersedes the pre-simulation row through the one reducer:
//! `tool.flute_overreach` where wood stands above the flute,
//! `tool.flute_reach_within` where none does.
//!
//! - op 3: the drill path (analytic removal) measures the 30 mm hole.
//! - a one-level pocket deeper than the flute on fresh stock: the walk
//!   measures it.
//! - the same pocket in steps, inside a pocket that an earlier op cleared:
//!   the pre-simulation row warns "by the stock box", the simulation is
//!   clean, and its row supersedes the warning.
//!
//! Red: make `ReachFlute` give no row (its predicate `false`): op 3 has no
//! pre-simulation warning. Make `drill_flute_overreach` find no hole: op 3
//! has no simulation row. Make `RapidClearanceCheck::flute_candidate`
//! `false`: the one-level pocket has no simulation row. Drop the
//! `supersedes` of `flute_reach_measured`: the pre-simulation row stays.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::k5_cases::{FLAT_6_STICKOUT_MM, export, export_names, op3_generated, rule_texts};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{CentreCutting, ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::diagnostics::{Diagnostic, ids};
use rs_cam_core::gcode::ExportedGcode;
use rs_cam_core::session::{
    ProjectSession, ProjectSessionBuilder, SimulationOptions, SimulationResolution,
};

/// The hole depth of op 3 (report 04 (b)): `(stock max z - stock min z) +
/// spoilboard_penetration` = 25.5 + 4.5.
const OP3_HOLE_MM: f64 = 25.5 + 4.5;

#[test]
fn a_pin_hole_past_the_flute_warns_with_no_mesh() {
    let session = op3_generated(FLAT_6_STICKOUT_MM);
    assert!(
        !session
            .result_at(0)
            .expect("a result")
            .toolpath()
            .moves
            .is_empty(),
        "op 3 generates (Q6)"
    );
    let texts = rule_texts(&session, 0, ids::TOOL_REACH_PAST_FLUTE);
    assert_eq!(texts.len(), 1, "one ReachFlute row: {texts:?}");
    let text = texts.first().expect("one row");
    assert!(
        text.contains("flute 25 mm, estimated") && text.contains("cuts 30 mm below the top"),
        "the row names the flute, how it is known, and the depth: {text}"
    );
    let row = session
        .result_at(0)
        .and_then(|r| r.stats.feasibility.as_ref())
        .and_then(|f| {
            f.rows()
                .iter()
                .find(|r| r.id() == ids::TOOL_REACH_PAST_FLUTE)
        })
        .expect("the row");
    assert_eq!(row.measured(), OP3_HOLE_MM, "the hole depth");
    assert_eq!(row.limit(), Some(25.0), "the flute length of tool 0");

    let exported = export(&session);
    assert!(
        !exported.gcode.is_empty(),
        "the export writes a program (Q6)"
    );
    assert!(
        export_names(&exported, ids::TOOL_REACH_PAST_FLUTE),
        "the export and the header name the warning: {:?}",
        exported.warnings
    );
}

/// Simulate every op of `session` at `cell` mm. Returns the diagnostics of
/// toolpath `index` and the export, both read with the run's cut trace.
fn simulated(
    session: &mut ProjectSession,
    cell: f64,
    index: usize,
) -> (Vec<Diagnostic>, ExportedGcode) {
    let opts = SimulationOptions {
        resolution: SimulationResolution::Fixed(cell),
        adaptive_feed_modulation: false,
        ..SimulationOptions::default()
    };
    session
        .run_simulation(&opts, &AtomicBool::new(false))
        .expect("the simulation runs");
    let trace = session
        .simulation_result()
        .and_then(|sim| sim.cut_trace.clone());
    let diagnostics = session
        .diagnose_toolpath_with_trace(index, trace.as_deref())
        .expect("the diagnostics");
    let exported = session
        .export_gcode_checked(trace.as_deref())
        .expect("the export writes the program (Q6)");
    (diagnostics, exported)
}

/// The flute overreach records of toolpath `index` in the stored run.
fn overreach_count(session: &ProjectSession, index: usize) -> usize {
    let id = session.toolpath_configs()[index].id;
    session
        .simulation_result()
        .expect("a simulation")
        .flute_overreach
        .iter()
        .filter(|hit| hit.toolpath_id == id)
        .count()
}

fn has(diagnostics: &[Diagnostic], id: &str) -> bool {
    diagnostics.iter().any(|d| d.id.0 == id)
}

#[test]
fn the_simulation_measures_the_pin_hole_and_supersedes_the_stock_box_row() {
    let mut session = op3_generated(FLAT_6_STICKOUT_MM);
    let (diagnostics, exported) = simulated(&mut session, 1.0, 0);
    assert!(
        overreach_count(&session, 0) > 0,
        "the drill path measures the 30 mm hole past the 25 mm flute"
    );
    let hit = &session.simulation_result().expect("a run").flute_overreach[0].overreach;
    assert_eq!(hit.material_over_tip_mm, OP3_HOLE_MM, "the hole depth");
    assert!(
        has(&diagnostics, ids::TOOL_FLUTE_OVERREACH),
        "{diagnostics:?}"
    );
    assert!(
        !has(&diagnostics, ids::TOOL_REACH_PAST_FLUTE),
        "the simulation row supersedes the stock-box row"
    );
    assert!(export_names(&exported, ids::TOOL_FLUTE_OVERREACH));
    assert!(
        !exported
            .warnings
            .iter()
            .any(|w| w.id == ids::TOOL_REACH_PAST_FLUTE),
        "the export prints the measured row only: {:?}",
        exported.warnings
    );
}

/// A test-built tool (ruling D12: it states its facts): 6 mm, flute 8 mm,
/// stickout 30 mm, centre cutting. The values are the case, not a real
/// tool.
fn short_flute_tool() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(8.0));
    tool.set_stickout(ToolFact::Stated(30.0));
    tool.set_centre_cutting(ToolFact::Stated(CentreCutting::Yes));
    tool
}

/// The stock of the pocket cases: 44 x 44, top at Z 25.5 (the wanaka
/// thickness), bottom at Z 0.
const STOCK_TOP_Z: f64 = 25.5;
/// The deep pocket: half 8 mm, floor at Z 15 (10.5 mm below the top: more
/// than the 8 mm flute).
const DEEP_FLOOR_Z: f64 = 15.0;
/// The clearing pocket: half 18 mm, floor at Z 20 (5.5 mm below the top).
const CLEAR_FLOOR_Z: f64 = 20.0;

fn pocket(floor_z: f64, depth_per_pass: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: Floor::Z { z: floor_z },
        depth_per_pass,
        ..PocketConfig::default()
    })
}

/// A session with one pocket per `(half, op)`, each on its own square, in
/// order, generated.
fn pockets(ops: Vec<(f64, OperationConfig)>) -> ProjectSession {
    let mut stock = common::session::stock_over(20.0, STOCK_TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(short_flute_tool());
    let tool_id = rs_cam_core::ids::ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    for (k, (half, op)) in ops.into_iter().enumerate() {
        let model = builder.add_model(common::session::polygon_model(
            vec![common::session::square_polygon(half)],
            &format!("square_{k}"),
        ));
        let cfg = common::session::toolpath_config(&format!("pocket {k}"), op, tool_id, model);
        let _ = builder.add_toolpath(0, cfg).expect("add the pocket");
    }
    let mut session = builder.build();
    for index in 0..session.toolpath_configs().len() {
        common::session::generate(&mut session, index);
    }
    session
}

#[test]
fn the_walk_measures_a_one_level_cut_past_the_flute() {
    // One level of 10.5 mm: the flank cuts wood 10.5 mm over the tip.
    let mut session = pockets(vec![(8.0, pocket(DEEP_FLOOR_Z, 12.0))]);
    let (diagnostics, exported) = simulated(&mut session, 0.5, 0);
    assert!(
        overreach_count(&session, 0) > 0,
        "the walk measures the 10.5 mm level past the 8 mm flute"
    );
    assert!(
        has(&diagnostics, ids::TOOL_FLUTE_OVERREACH),
        "{diagnostics:?}"
    );
    assert!(!has(&diagnostics, ids::TOOL_REACH_PAST_FLUTE));
    assert!(export_names(&exported, ids::TOOL_FLUTE_OVERREACH));
}

#[test]
fn a_cut_inside_cleared_material_is_measured_clean() {
    // Op 0 clears a wide pocket to Z 20; op 1 steps a narrow pocket to Z 15
    // inside it. By the stock box op 1 cuts 10.5 mm below the top (past
    // the flute); on the carved stock no step has more than 5 mm of wood
    // over the tip.
    let mut session = pockets(vec![
        (18.0, pocket(CLEAR_FLOOR_Z, 2.0)),
        (8.0, pocket(DEEP_FLOOR_Z, 2.0)),
    ]);
    let texts = rule_texts(&session, 1, ids::TOOL_REACH_PAST_FLUTE);
    assert_eq!(texts.len(), 1, "the pre-simulation row warns: {texts:?}");
    assert!(texts.iter().all(|t| t.contains("by the stock box")));
    let (diagnostics, exported) = simulated(&mut session, 0.5, 1);
    assert_eq!(overreach_count(&session, 1), 0, "measured clean");
    assert!(
        has(&diagnostics, ids::TOOL_FLUTE_REACH_WITHIN),
        "the clean row is in the list: {diagnostics:?}"
    );
    assert!(
        !has(&diagnostics, ids::TOOL_REACH_PAST_FLUTE),
        "the clean row supersedes the stock-box row"
    );
    assert!(
        !exported.warnings.iter().any(|w| {
            w.toolpath == "pocket 1"
                && (w.id == ids::TOOL_REACH_PAST_FLUTE || w.id == ids::TOOL_FLUTE_OVERREACH)
        }),
        "the export names no flute row for the clean op: {:?}",
        exported.warnings
    );
}
