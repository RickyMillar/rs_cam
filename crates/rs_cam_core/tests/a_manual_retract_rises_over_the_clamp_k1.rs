//! Sentry (K1 review, finding 2; K3): **an operator-set retract below a
//! clamp top never crosses the clamp, and the move between two toolpaths
//! is checked**.
//!
//! ## The defect this pins
//!
//! Only an Auto retract rose to the obstacle plane. A Manual or From
//! Reference retract kept its value. The per-op clip keeps each toolpath
//! out of the clamp, but the rapid BETWEEN two toolpaths was checked
//! nowhere: the obstacle check read the first move of a toolpath as a
//! point. Two traces on each side of a 20 mm clamp with a Manual retract of
//! 10 mm then crossed the clamp at Z 10 in the G-code, and every check read
//! clear.
//!
//! ## The fix this pins
//!
//! 1. K3: the op keeps its own retract (the plane is local). The per-op
//!    obstacle step lifts each move that crosses the clamp to the plane,
//!    and starts every toolpath at the plane, so the join into it rises
//!    before it traverses.
//! 2. `ObstacleToolView::check_toolpath` checks the join from the previous
//!    toolpath of the setup (`JoinStart`) as the G-code builder emits it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::simulate;
use common::session::{stock_under, toolpath_config};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{HeightMode, HeightsConfig, SAFE_Z_CLEARANCE_MM};
use rs_cam_core::compute::obstacle::JoinStart;
use rs_cam_core::compute::operation_configs::TraceConfig;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ObstacleId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AddObstacleArgs, Command, Obstacle, ObstacleGeometry, ProjectSession, ProjectSessionBuilder,
};
use rs_cam_core::toolpath::Toolpath;

const HALF: f64 = 25.0;
const DEPTH: f64 = 6.0;
/// The clamp top and its clearance: the grown top is 21 mm.
const CLAMP_TOP: f64 = 20.0;
const CLEARANCE: f64 = 1.0;
/// The operator's retract: below the clamp top.
const MANUAL_RETRACT: f64 = 10.0;

/// A clamp across the stock, between the two traces.
fn clamp() -> Obstacle {
    Obstacle {
        id: ObstacleId(0),
        name: "Middle clamp".to_owned(),
        enabled: true,
        clearance: CLEARANCE,
        geometry: ObstacleGeometry::Box {
            min: [-3.0, -40.0, -DEPTH],
            max: [3.0, 40.0, CLAMP_TOP],
        },
    }
}

/// Two traces, one on each side of the clamp, one tool, Manual retract 10
/// and Manual clearance 12. Generated after the clamp is added.
fn two_traces_across_the_clamp() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_under(HALF, DEPTH));
    let tool_idx = builder.add_tool(endmill_tool_config(6.0));
    let tool_id = builder.tools()[tool_idx].id.0;
    for (name, x0, x1) in [("Left", -20.0, -10.0), ("Right", 10.0, 20.0)] {
        let model_id = builder.add_model(common::session::polygon_model(
            vec![Polygon2::rectangle(x0, -5.0, x1, 5.0)],
            name,
        ));
        let op = OperationConfig::Trace(TraceConfig {
            depth: 1.0,
            depth_per_pass: 1.0,
            ..TraceConfig::default()
        });
        let mut cfg = toolpath_config(name, op, tool_id, model_id);
        cfg.heights = HeightsConfig {
            retract_z: HeightMode::Manual(MANUAL_RETRACT),
            clearance_z: HeightMode::Manual(MANUAL_RETRACT + 2.0),
            ..HeightsConfig::default()
        };
        builder.add_toolpath(0, cfg).expect("add toolpath");
    }
    let mut session = builder.build();
    let _ = session
        .apply(Command::AddObstacle(AddObstacleArgs {
            setup_index: 0,
            obstacle: Box::new(clamp()),
        }))
        .expect("add the clamp");
    let cancel = AtomicBool::new(false);
    for index in 0..session.toolpath_count() {
        session
            .generate_toolpath(index, &cancel)
            .expect("generation must succeed");
    }
    session
}

fn plane() -> f64 {
    CLAMP_TOP + CLEARANCE + SAFE_Z_CLEARANCE_MM
}

/// The XY half-width (mm) of the clamp footprint the cutter (r 3) must
/// keep out of: the clamp half-width 3, the clearance 1 and the radius 3.
const KEEP_OUT_HALF_X: f64 = 3.0 + CLEARANCE + 3.0;

#[test]
fn a_manual_retract_below_the_clamp_never_crosses_it_k1() {
    let session = two_traces_across_the_clamp();
    for index in 0..2 {
        let tc = &session.toolpath_configs()[index];
        let ctx = session.height_context_for_toolpath(tc);
        let resolved = tc.heights.resolve(&ctx);
        // K3: the op keeps the operator's retract; only a crossing rises.
        assert!(
            (resolved.retract_z - MANUAL_RETRACT).abs() < 1e-9,
            "TP{index}: the Manual retract must stay {MANUAL_RETRACT}, got {}",
            resolved.retract_z
        );
        let toolpath = session.get_result(index).expect("a result").toolpath();
        // The toolpath starts at the plane, so the join into it rises first.
        assert!(
            toolpath.moves[0].target.z >= plane() - 1e-9,
            "TP{index}: the first move is at Z {}, below the plane {}",
            toolpath.moves[0].target.z,
            plane()
        );
        // Non-vacuity of the local plane: the op's own retract is used.
        assert!(
            toolpath
                .moves
                .iter()
                .any(|m| (m.target.z - MANUAL_RETRACT).abs() < 1e-9),
            "TP{index}: no move at the Manual retract"
        );
        // Every move whose XY span meets the clamp keep-out is at the plane
        // or higher. The clamp spans the whole stock in Y.
        for w in toolpath.moves.windows(2) {
            let (a, b) = (w[0].target, w[1].target);
            let meets = a.x.min(b.x) < KEEP_OUT_HALF_X && a.x.max(b.x) > -KEEP_OUT_HALF_X;
            if meets {
                assert!(
                    a.z.min(b.z) >= plane() - 1e-9,
                    "TP{index}: a move from {a:?} to {b:?} crosses the clamp below the plane"
                );
            }
        }
    }
}

#[test]
fn the_join_between_the_traces_is_clear_k1() {
    let mut session = two_traces_across_the_clamp();
    // The join into the second trace starts at the end of the first.
    let join = session.obstacle_join_for_toolpath(1);
    let last = session
        .get_result(0)
        .expect("a result")
        .toolpath()
        .moves
        .last()
        .expect("a move")
        .target;
    assert_eq!(join, JoinStart::After(last));
    let no_cancel = AtomicBool::new(false);
    for index in 0..2 {
        let hits: Vec<_> = session
            .collision_check(index, &no_cancel)
            .expect("the check runs")
            .collision_report
            .collisions;
        assert!(hits.is_empty(), "TP{index}: {hits:?}");
    }
    simulate(&mut session);
    let sim = session.simulation_result().expect("a simulation");
    assert!(
        sim.obstacle_collisions.is_empty(),
        "{:?}",
        sim.obstacle_collisions
    );
}

/// Non-vacuity: the join check DOES see a join below the clamp top. The
/// motion is the one the old resolver emitted: the first trace ends at Z
/// 10 on the left, the second starts with a rapid at Z 10 on the right.
#[test]
fn a_join_below_the_clamp_top_is_a_hit_k1() {
    let session = two_traces_across_the_clamp();
    let set = session.obstacle_set_for_toolpath(1).expect("the clamp");
    let tool = rs_cam_core::compute::cutter::build_cutter(&session.tools()[0]);
    let view = set.for_tool(&tool.to_assembly()).expect("a tool view");

    let mut second = Toolpath::new();
    second.rapid_to(P3::new(15.0, 0.0, MANUAL_RETRACT));
    second.feed_to(P3::new(15.0, 0.0, -1.0), 300.0);

    // Read as a point (the old check), the first move is clear.
    let clear = view.check_toolpath(&second, JoinStart::After(P3::new(15.0, 0.0, 10.0)));
    assert!(clear.is_empty(), "{clear:?}");
    // Read as the join the G-code builder emits, it crosses the clamp.
    let hits = view.check_toolpath(&second, JoinStart::After(P3::new(-15.0, 0.0, 10.0)));
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].move_index, 0);
    // After a tool change the XY is not known: a Z-first rapid at 10 is a
    // hit, and one at the plane is clear.
    assert_eq!(view.check_toolpath(&second, JoinStart::Unknown).len(), 1);
    let mut high = Toolpath::new();
    high.rapid_to(P3::new(15.0, 0.0, plane()));
    assert!(view.check_toolpath(&high, JoinStart::Unknown).is_empty());
}
