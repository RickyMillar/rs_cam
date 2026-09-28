//! G-DRILLTWICE (2026-09-29) — a drill hole is cut once.
//!
//! ## The live case
//!
//! A `Drill` op on a DXF with 92 CIRCLE entities (one `DrillTarget` per
//! circle) emitted the whole peck cycle two times at each hole, before it
//! moved to the next hole:
//!
//! ```text
//! G0 X14.570 Y495.925 Z5.000
//! G1 X14.570 Y495.925 Z-7.000 F2082
//! G0 Z5.000 / G0 Z5.000 / G0 Z-6.500 / G1 Z-12.000 / G0 Z5.000 / G0 Z5.000
//! G1 X14.570 Y495.925 Z-7.000 F2082      <- the same cycle again
//! G0 Z5.000 / Z-6.500 / G1 Z-12.000 / G0 Z5.000
//! G0 X24.141 Y479.349 Z5.000             <- next hole
//! ```
//!
//! `inspect_spans` read 4 `DrillPeck` spans per hole and 368 `LinkBridge`
//! spans for 92 holes. The op: depth 12, peck cycle, peck depth 15,
//! `retract_z` 2, feed 2082, stock top Z 0 (a 26 mm stock). Every dressup
//! was on: entry None, link moves (10 mm, 500 mm/min), arc fitting,
//! segment merge 0.3, feed optimisation, rapid-order optimisation, air
//! bridge policy `Always`.
//!
//! ## What this test measures
//!
//! The same op over three `DrillTarget`s, through `generate_toolpath`, with
//! those dressups. For each hole XY:
//!
//! 1. exactly ONE fed descent reaches the hole bottom (Z -12);
//! 2. exactly TWO fed descents exist: 5 -> -7 and -6.5 -> -12. The R-plane
//!    is `effective_safe_z(2, 0) = 5`, the peck (15) is clamped to the depth
//!    (12), and the cycle is rooted at the R-plane like Fanuc G83
//!    (`drill::fed_descents`, `clamp_peck_depth`). The 12 mm then 5 mm split
//!    is that design, not a second defect;
//! 3. the moves at that XY form one contiguous run (one visit per hole).
//!
//! Three arms: all dressups off (the generator alone), the live dressups,
//! and the live dressups after a simulation (so the air-cut filter reads a
//! prior stock, as in the GUI). A red arm names the stage that doubles the
//! cycle: off red is the generator, off green and live red is a dressup.
//!
//! A fourth arm gives each hole TWO circles on one centre. The DXF door
//! gives one target per circle, so the model then has two targets per hole.
//! The live counts (368 `LinkBridge` spans, two per cycle, and 4 pecks
//! under one "Hole N" span) are what 184 cycles in adjacent pairs give.
//! Before the fix `drill_holes_for_config` drilled every target, and this
//! arm cut each hole two times.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_drill_hole_is_cut_once_g_drilltwice
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::toolpath_config;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    ArcFitParams, DressupConfig, DressupEntryStyle, LinkDressupParams, SegmentMergeParams,
};
use rs_cam_core::compute::operation_configs::{DrillConfig, DrillCycleType};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::dressup::AirBridgePolicy;
use rs_cam_core::geo::P2;
use rs_cam_core::io::dxf_input::{DrillTarget, DrillTargetKind};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder, SimulationOptions};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

const STOCK_X: f64 = 400.0;
const STOCK_Y: f64 = 520.0;
const STOCK_Z: f64 = 26.0;
const DEPTH: f64 = 12.0;
const HOLE_DIAMETER: f64 = 8.0;
const XY_EPS: f64 = 1e-6;
const Z_EPS: f64 = 1e-6;

/// The two live holes, and one 7.2 mm from the second so a link move of
/// 10 mm could reach it.
const HOLES: [[f64; 2]; 3] = [[14.570, 495.925], [24.141, 479.349], [30.0, 472.5]];

fn stock() -> StockConfig {
    StockConfig {
        x: STOCK_X,
        y: STOCK_Y,
        z: STOCK_Z,
        origin_x: 0.0,
        origin_y: 0.0,
        // Stock top at world Z 0, as in the live setup frame.
        origin_z: -STOCK_Z,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// A 32-gon for one DXF circle, as `circle_to_points` would tessellate it.
fn circle(centre: [f64; 2]) -> Polygon2 {
    let r = HOLE_DIAMETER * 0.5;
    let pts = (0..32)
        .map(|k| {
            let a = std::f64::consts::TAU * f64::from(k) / 32.0;
            P2::new(centre[0] + r * a.cos(), centre[1] + r * a.sin())
        })
        .collect();
    let mut poly = Polygon2::new(pts);
    poly.ensure_winding();
    poly
}

/// The DXF model: `rings` closed circles on each hole centre, and one
/// `DrillTarget` per circle, in entity order, as `dxf_input` gives them.
/// `rings = 2` is a counterbore drawn as two rings, or an entity the CAD
/// export wrote twice.
fn dxf_model(rings: usize) -> LoadedModel {
    let targets = HOLES
        .iter()
        .flat_map(|&[x, y]| {
            (0..rings).map(move |_| DrillTarget {
                x,
                y,
                layer: "0".to_owned(),
                kind: DrillTargetKind::CircleCenter {
                    diameter: HOLE_DIAMETER,
                },
            })
        })
        .collect();
    LoadedModel {
        id: 0,
        name: "holes".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(HOLES.iter().map(|&c| circle(c)).collect())),
        drill_targets: Arc::new(targets),
        layers: Arc::new(vec!["0".to_owned()]),
        path: PathBuf::from("synthetic://holes.dxf"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn drill_op() -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        depth: DEPTH,
        cycle: DrillCycleType::Peck,
        peck_depth: 15.0,
        retract_z: 2.0,
        feed_rate: 2082.0,
        ..DrillConfig::default()
    })
}

/// The live dressups: every dressup on, entry None.
// By value: `session` takes these as `fn(DressupConfig) -> DressupConfig`.
#[allow(clippy::needless_pass_by_value)]
fn live_dressups(base: DressupConfig) -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::None,
        link_moves: Some(LinkDressupParams {
            max_distance: 10.0,
            feed_rate: 500.0,
        }),
        arc_fitting: Some(ArcFitParams::default()),
        segment_merge: Some(SegmentMergeParams { tolerance: 0.3 }),
        feed_optimization: true,
        optimize_rapid_order: true,
        air_bridge_policy: AirBridgePolicy::Always,
        ..base
    }
}

/// Every dressup off, entry None: the generator alone.
// By value: `session` takes these as `fn(DressupConfig) -> DressupConfig`.
#[allow(clippy::needless_pass_by_value)]
fn no_dressups(base: DressupConfig) -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::None,
        dogbone: None,
        lead_in_out: None,
        link_moves: None,
        arc_fitting: None,
        segment_merge: None,
        feed_optimization: false,
        optimize_rapid_order: false,
        ..base
    }
}

fn session(dressups: fn(DressupConfig) -> DressupConfig) -> ProjectSession {
    session_with(dressups, 1)
}

fn session_with(dressups: fn(DressupConfig) -> DressupConfig, rings: usize) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(dxf_model(rings));
    let mut tc = toolpath_config("Drill", drill_op(), tool_id, model_id);
    tc.dressups = dressups(tc.dressups);
    let _ = builder.add_toolpath(0, tc).expect("add drill toolpath");
    builder.build()
}

fn generate(session: &mut ProjectSession) -> Toolpath {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("drill generation must succeed")
        .op_data
        .annotated()
        .toolpath
        .clone()
}

fn at_xy(m: &rs_cam_core::toolpath::Move, xy: [f64; 2]) -> bool {
    (m.target.x - xy[0]).abs() < XY_EPS && (m.target.y - xy[1]).abs() < XY_EPS
}

/// Every fed descent at `xy`, as `(from_z, to_z)`.
fn fed_descents_at(tp: &Toolpath, xy: [f64; 2]) -> Vec<(f64, f64)> {
    (1..tp.moves.len())
        .filter_map(|i| {
            let (prev, m) = (&tp.moves[i - 1], &tp.moves[i]);
            let descends = m.move_type.is_cutting()
                && at_xy(m, xy)
                && at_xy(prev, xy)
                && m.target.z < prev.target.z - Z_EPS;
            descends.then_some((prev.target.z, m.target.z))
        })
        .collect()
}

/// The number of separate runs of consecutive moves that sit at `xy`.
fn visits(tp: &Toolpath, xy: [f64; 2]) -> usize {
    let mut runs = 0;
    let mut inside = false;
    for m in &tp.moves {
        let here = at_xy(m, xy);
        if here && !inside {
            runs += 1;
        }
        inside = here;
    }
    runs
}

fn assert_each_hole_cut_once(tp: &Toolpath, arm: &str) {
    let drilling = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Drilling)
        .count();
    assert!(drilling > 0, "{arm}: the fixture drilled nothing");

    let bottom = -DEPTH;
    for xy in HOLES {
        let descents = fed_descents_at(tp, xy);
        let to_bottom = descents
            .iter()
            .filter(|(_, to)| (to - bottom).abs() < Z_EPS)
            .count();
        assert_eq!(
            to_bottom, 1,
            "{arm}: the hole at {xy:?} has {to_bottom} fed descents to the bottom \
             Z {bottom}; a hole is cut once. All fed descents there: {descents:?}"
        );
        assert_eq!(
            descents.len(),
            2,
            "{arm}: the hole at {xy:?} has {} fed descents; a 12 mm peck cycle \
             from the R-plane at 5 has two (5 -> -7, -6.5 -> -12). Got {descents:?}",
            descents.len()
        );
        let first = descents[0];
        let second = descents[1];
        assert!(
            (first.1 - (5.0 - DEPTH)).abs() < Z_EPS && (second.1 - bottom).abs() < Z_EPS,
            "{arm}: the hole at {xy:?} pecks {descents:?}; expected ends -7 then -12"
        );
        assert_eq!(
            visits(tp, xy),
            1,
            "{arm}: the tool visits the hole at {xy:?} more than once"
        );
    }
}

#[test]
fn the_generator_alone_cuts_each_hole_once() {
    let tp = generate(&mut session(no_dressups));
    assert_each_hole_cut_once(&tp, "no dressups");
}

#[test]
fn a_drill_hole_is_cut_once_g_drilltwice() {
    let tp = generate(&mut session(live_dressups));
    assert_each_hole_cut_once(&tp, "live dressups");
}

#[test]
fn a_drill_hole_is_cut_once_after_a_simulation() {
    let mut s = session(live_dressups);
    let _ = generate(&mut s);
    let cancel = AtomicBool::new(false);
    s.run_simulation(&SimulationOptions::default(), &cancel)
        .expect("simulation");
    let tp = generate(&mut s);
    assert_each_hole_cut_once(&tp, "live dressups after a simulation");
}

/// Two circles on one centre are two targets and ONE hole.
#[test]
fn two_circles_on_one_centre_drill_one_hole() {
    let tp = generate(&mut session_with(live_dressups, 2));
    assert_each_hole_cut_once(&tp, "two circles per hole, live dressups");
    let tp = generate(&mut session_with(no_dressups, 2));
    assert_each_hole_cut_once(&tp, "two circles per hole, no dressups");
}
