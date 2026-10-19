//! Sentry (H1): **the machine head and the dust shoe join the safety
//! checks.**
//!
//! Source: `planning/op_architecture_2026-10-14/PLAN.md`, section H.
//!
//! The claims:
//!
//! - (a) a shelf (a head part with no width limit) is a Z floor for the tip:
//!   the highest top minus the shelf height above the tip. The check flags
//!   a tip below it, and the obstacle step passes a toolpath above it
//!   through unchanged;
//! - (b) the shoe moves no toolpath: the generated toolpath with the dust
//!   shoe head is the toolpath with the unmodelled head;
//! - (c) with the shoe on Auto the solver gives the lowest clear gap, and
//!   the simulation finds no strike;
//! - (d) a stated gap below the solver gap files a Caution that names the
//!   op and the needed gap, and the simulation reports the shoe plate in
//!   the stock.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::meshes::height_field;
use common::op_fixtures::simulate;
use common::session::{generate, mesh_model, pinned_heights, single_op_session_with, stock_under};
use common::tools::ball_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::obstacle::{JoinStart, ObstacleSet, clip_annotated_to_obstacles};
use rs_cam_core::compute::operation_configs::DropCutterConfig;
use rs_cam_core::compute::tool_config::ToolConfig;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::machine::head::{HeadFit, MachineHead, ResolvedShoe, ShoeSetting};
use rs_cam_core::session::{
    Command, ProjectSession, SetMachineHeadArgs, SetToolpathShoeArgs, VerdictKind,
};
use rs_cam_core::stock::collision::{AssemblySegment, CollisionKind};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::trace::transform_provenance::ReconcileSet;

/// The stock half-width and depth (mm): top at Z 0, bottom at Z -70.
const HALF: f64 = 30.0;
const DEPTH: f64 = 70.0;
/// The finished floor (mm).
const FLOOR_Z: f64 = -55.0;

fn tool() -> ToolConfig {
    ToolConfig {
        cutting_length: 60.0,
        shank_diameter: 6.0,
        stickout: 60.0,
        ..ball_tool_config(6.0)
    }
}

fn session() -> ProjectSession {
    single_op_session_with(
        stock_under(HALF, DEPTH),
        tool(),
        mesh_model(height_field(HALF, 2.0, |_, _| FLOOR_Z), "Floor"),
        "3D Finish",
        OperationConfig::DropCutter(DropCutterConfig {
            stepover: 4.0,
            min_z: FLOOR_Z - 5.0,
            ..DropCutterConfig::default()
        }),
        |tc| tc.heights = pinned_heights(0.0, FLOOR_Z - 5.0),
    )
}

fn set_head(session: &mut ProjectSession, head: MachineHead) {
    let _ = session
        .apply(Command::SetMachineHead(SetMachineHeadArgs {
            head: Box::new(head),
        }))
        .expect("a valid head");
}

fn set_shoe(session: &mut ProjectSession, shoe: ShoeSetting) {
    let _ = session
        .apply(Command::SetToolpathShoe(SetToolpathShoeArgs {
            index: 0,
            shoe,
        }))
        .expect("a valid shoe setting");
}

/// (a) The shelf floor of the bare collet: stickout 45, collet 30, so the
/// spindle shelf is 75 mm above the tip and the floor is 75 mm under the
/// stock top.
#[test]
fn a_shelf_is_a_z_floor_under_the_highest_top_h1() {
    let stock = BoundingBox3 {
        min: P3::new(0.0, 0.0, -100.0),
        max: P3::new(100.0, 100.0, 0.0),
    };
    let cutter = rs_cam_core::compute::cutter::build_cutter(&ToolConfig {
        cutting_length: 20.0,
        stickout: 45.0,
        ..ball_tool_config(6.0)
    })
    .unwrap();
    let assembly = cutter.to_assembly(&HeadFit::fixed(&MachineHead::bare_collet()));
    let view = ObstacleSet::default()
        .for_tool(&assembly, Some(&stock))
        .unwrap();
    assert_eq!(view.floor_z(), Some(-75.0));
    // No stock read: no top, so no floor.
    assert_eq!(
        ObstacleSet::default()
            .for_tool(&assembly, None)
            .unwrap()
            .floor_z(),
        None
    );

    let pass = |z: f64| {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(50.0, 50.0, 10.0));
        tp.feed_to(P3::new(50.0, 50.0, z), 300.0);
        tp.feed_to(P3::new(60.0, 50.0, z), 1000.0);
        tp.rapid_to(P3::new(60.0, 50.0, 10.0));
        tp
    };
    let deep = view.check_toolpath(&pass(-80.0), JoinStart::Unknown);
    assert!(
        !deep.is_empty(),
        "a tip 80 mm down puts the spindle in the stock"
    );
    assert_eq!(deep[0].kind, CollisionKind::Stock);
    assert_eq!(deep[0].segment, AssemblySegment::Holder);
    assert!(
        view.check_toolpath(&pass(-70.0), JoinStart::Unknown)
            .is_empty()
    );

    // The step passes a toolpath above the floor through unchanged: no K3
    // start rapid (that is the obstacle join rule), no lift.
    let mut channels = ReconcileSet::new(None, None);
    let above = pass(-70.0);
    let out = clip_annotated_to_obstacles(AnnotatedToolpath::new(above.clone()), &view, 10.0, None)
        .reconcile(&mut channels)
        .into_inner();
    assert_eq!(
        format!("{:?}", out.toolpath.moves),
        format!("{:?}", above.moves)
    );
    // A toolpath below the floor loses the cut under it.
    let out = clip_annotated_to_obstacles(AnnotatedToolpath::new(pass(-80.0)), &view, 10.0, None)
        .reconcile(&mut channels)
        .into_inner();
    assert!(
        view.check_toolpath(&out.toolpath, JoinStart::Unknown)
            .is_empty()
    );
}

/// (b), (c), (d) on one deep finish: the plate (25 mm above the brush end)
/// must clear the stock top with the tip at -55, so the solver gap is 30.
#[test]
fn the_shoe_plate_strikes_only_below_the_solver_gap_h1() {
    let mut plain = session();
    generate(&mut plain, 0);
    let plain_tp = plain.get_result(0).unwrap().toolpath().clone();

    let mut s = session();
    set_head(&mut s, MachineHead::dust_shoe());
    generate(&mut s, 0);
    // (b) The shoe moves no toolpath.
    assert_eq!(
        format!("{:?}", s.get_result(0).unwrap().toolpath().moves),
        format!("{:?}", plain_tp.moves)
    );

    // (c) Auto: the lowest clear gap, and a clean simulation.
    let plan = s.shoe_plan().unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].needed_gap_mm, Some(30.0));
    assert_eq!(plan[0].resolved, ResolvedShoe::Gap { gap_mm: 30.0 });
    simulate(&mut s);
    let sim = s.simulation_result().expect("a simulation");
    assert!(
        sim.obstacle_collisions.is_empty(),
        "{:?}",
        sim.obstacle_collisions.first().map(|c| &c.event)
    );
    assert!(
        !s.diagnostics()
            .verdicts
            .iter()
            .any(|v| v.kind == VerdictKind::ShoeGapTooLow)
    );

    // (d) A stated gap of 25: a Caution names the op and the gap, and the
    // simulation reports the plate in the stock.
    set_shoe(&mut s, ShoeSetting::Gap { gap_mm: 25.0 });
    assert!(
        s.get_result(0).is_some(),
        "the shoe setting moves no toolpath"
    );
    let verdict = s
        .diagnostics()
        .verdicts
        .into_iter()
        .find(|v| v.kind == VerdictKind::ShoeGapTooLow)
        .expect("a stated gap below the solver gap is named");
    assert!(
        verdict.headline.contains("3D Finish"),
        "{}",
        verdict.headline
    );
    assert!(verdict.headline.contains("30 mm"), "{}", verdict.headline);
    simulate(&mut s);
    let sim = s.simulation_result().expect("a simulation");
    let hit = sim
        .obstacle_collisions
        .first()
        .expect("the plate strikes the stock at gap 25");
    assert_eq!(hit.event.segment, AssemblySegment::Shoe);
    assert_eq!(hit.event.kind, CollisionKind::Stock);
}
