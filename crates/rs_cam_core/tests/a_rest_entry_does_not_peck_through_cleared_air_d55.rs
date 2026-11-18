//! D55 (G-AIRPECK, `planning/adaptive_research_2026-10-23/DEFECTS.md`): a
//! rest op's plunge entry into a cleared pocket is one fed stroke, not a
//! peck ladder through the air above it.
//!
//! The planner's rapid floor is the sliver-safe stock top over every cell
//! that touches the tool disc. Beside a wall that is the wall top, though
//! the column the tool cuts was cleared by the op before. The plunge used
//! to peck in Depth/Pass strokes from that floor down through the air
//! (wanaka op 7: about 3 strokes per site). It now feeds straight through
//! the air to the contact start (`contact_z + entry_clearance_mm`, the
//! start the helix and the ramp take) and pecks only below it.
//!
//! The session: a pocket (floor 4 in a top at 10) that op 0 (contour
//! parallel, so it clears the whole pocket) roughs with a leave of 1 mm;
//! op 1 roughs the same pocket on the remaining stock with no leave and
//! `Plunge` entries. Every entry of op 1 goes down through the cleared
//! pocket to cut the 1 mm skin, so each site is one stroke. Its column from
//! the rapid floor is deeper than one Depth/Pass (asserted), so the old
//! ladder pecked it. Real stock still pecks: the emitter tests in
//! `adaptive3d/path.rs` (`a_plunge_into_real_stock_still_pecks_d55`,
//! `air_above_real_stock_feeds_then_pecks_d55`).

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: an expect is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use common::meshes::height_field;
use common::session::{mesh_model, stock_over, toolpath_config};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy,
};
use rs_cam_core::ids::ToolId;
use rs_cam_core::session::{
    AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder, SimulationOptions,
    SimulationResolution,
};
use rs_cam_core::toolpath::MoveIntent;
use std::sync::atomic::AtomicBool;

const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const POCKET_HALF_X: f64 = 30.0;
/// P41: the pocket is a slot 7 mm wide for the 6 mm tool, so a centre of the
/// slot stands beside the model wall and its rapid floor is the wall top:
/// the deep column of the fixture is air beside the model. With a half
/// width of 20 mm the one deep site was a corner where op 0 left real stock
/// 6 mm tall; the one-pass rule (`adaptive3d/one_pass.rs`) holds the tool
/// off that stock, so that site was not air and it is gone.
const POCKET_HALF_Y: f64 = 3.5;
const TOOL_DIAMETER: f64 = 6.0;
const DEPTH_PER_PASS: f64 = 3.0;
/// The source's axial leave: the skin op 1 cuts.
const SOURCE_LEAVE: f64 = 1.0;
const SIM_CELL: f64 = 0.5;

fn rough(strategy: ClearingStrategy, leave: f64) -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: Some(2.0),
        depth_per_pass: Some(DEPTH_PER_PASS),
        stock_to_leave_axial: leave,
        clearing_strategy: strategy,
        entry_style: Adaptive3dEntryStyle::Plunge,
        ..Adaptive3dConfig::default()
    })
}

fn session() -> ProjectSession {
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.abs() <= POCKET_HALF_X && y.abs() <= POCKET_HALF_Y {
            FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let mut stock = stock_over(MESH_HALF, TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model_id = builder.add_model(mesh_model(mesh, "pocket"));
    let op0 = toolpath_config(
        "rough",
        rough(ClearingStrategy::ContourParallel, SOURCE_LEAVE),
        tool_id,
        model_id,
    );
    let _ = builder.add_toolpath(0, op0).expect("add the rough");
    let mut session = builder.build();
    let mut op1 = toolpath_config(
        "rest",
        rough(ClearingStrategy::ContourParallel, 0.0),
        tool_id,
        model_id,
    );
    op1.stock_source = StockSource::FromRemainingStock;
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(op1),
        }))
        .expect("add the rest op");
    session
}

/// Each entry site: `(strokes, top, bottom)`. A stroke is a run of
/// `EntryPlunge` feeds at one XY with no other move between them; `top` is
/// the Z the first stroke starts from, `bottom` the entry Z.
fn entry_sites(moves: &[rs_cam_core::toolpath::Move]) -> Vec<(usize, f64, f64)> {
    let mut sites: Vec<(usize, f64, f64)> = Vec::new();
    let mut site_xy: Option<(f64, f64)> = None;
    let mut open = false;
    let mut prev_z = f64::NAN;
    for m in moves {
        let xy = (m.target.x, m.target.y);
        if m.intent == MoveIntent::EntryPlunge {
            if site_xy != Some(xy) {
                sites.push((0, prev_z, m.target.z));
                site_xy = Some(xy);
                open = false;
            }
            if let Some(last) = sites.last_mut() {
                if !open {
                    last.0 += 1;
                }
                last.2 = m.target.z;
            }
            open = true;
        } else {
            open = false;
            if site_xy.is_some_and(|s| s != xy) {
                site_xy = None;
            }
        }
        prev_z = m.target.z;
    }
    sites
}

#[test]
fn a_rest_entry_does_not_peck_through_cleared_air() {
    let mut session = session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate the rough");
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(SIM_CELL),
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulate the rough");
    session
        .generate_toolpath(1, &cancel)
        .expect("generate the rest op");
    let moves = &session
        .result_at(1)
        .expect("the rest op's result")
        .annotated()
        .toolpath
        .moves;
    let sites = entry_sites(moves);
    let most = sites.iter().map(|s| s.0).max().unwrap_or(0);
    eprintln!(
        "rest op: {} moves, entry sites (strokes, top, bottom) {sites:?}",
        moves.len()
    );
    assert!(!sites.is_empty(), "the rest op made no plunge entry");
    // Non-vacuity: a site's fed column is deeper than one Depth/Pass, so a
    // ladder from its top would peck.
    assert!(
        sites
            .iter()
            .any(|(_, top, bottom)| top - bottom > DEPTH_PER_PASS),
        "no entry column is deeper than one Depth/Pass: the fixture tests nothing"
    );
    assert_eq!(
        most, 1,
        "a rest entry pecked down through cleared air: up to {most} strokes at one site"
    );
}
