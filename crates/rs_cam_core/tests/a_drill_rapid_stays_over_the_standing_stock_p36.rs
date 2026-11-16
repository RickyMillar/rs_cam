//! P36 A (`planning/feeds_contract_2026-11-03/phases/P36_DRILL_RAPID_CENTRE_DESCENT_2026-11-16.md`
//! section 1): no rapid of a Drill ends in wood.
//!
//! The wanaka job gave `project.rapid_collision` on its light holes: a
//! rapid `30.5 -> 21.5` at a hole where the stock stood at Z 25.5. Two
//! defects made it.
//!
//! - A1: the R-plane followed a Top Z pinned below the standing wood
//!   (`compute::execute::drilling::drill_hole_columns`), on fresh stock and
//!   on the remaining stock.
//! - A2: with the R-plane on the retract plane of the op (the job: both
//!   30.5, the project `safe_z` and the stock top + 5), the rapid reorder
//!   (`dressup::tsp`) took each peck of a hole as its own segment and put
//!   the pecks of one hole in any order: 11 holes of the job went down to
//!   the last peck first, a rapid through 3.5 mm of wood.
//!
//! The claims:
//!
//! 1. Fresh stock, the Top Z pinned 9 mm below the stock top: the R-plane
//!    is the stock top + the clearance (5 mm), and no rapid ends under the
//!    wood. Red before A1 (the R-plane was the pinned top + 5, 4 mm inside
//!    the stock).
//! 2. Remaining stock, the Top Z pinned at the floor of an earlier pocket:
//!    the hole in the pocket keeps its R-plane 5 mm above the pocket floor;
//!    the hole beside the pocket has its R-plane 5 mm above the stock top
//!    that stands there. Red before A1 (2 mm inside the stock).
//! 3. The R-plane on the retract plane of the op (pinned there, as the
//!    job has it): the fed descents of each hole are in falling order of
//!    Z and no rapid ends under the wood. Red before A2.
//!
//! The wood rule of this file (`rapids_stay_over_the_wood`): at one hole
//! the wood top starts at the top that stands there and falls to the end
//! of each feed. A rapid at that hole never ends below it.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_drill_rapid_stays_over_the_standing_stock_p36
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::{
    TestHole, drawing_items, generate, polygon_model, single_op_session_with, square_polygon,
    stock_over, toolpath_config,
};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{HeightMode, StockSource};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::{DrillConfig, DrillCycleType, PocketConfig};
use rs_cam_core::session::{
    Command, ProjectSession, ProjectSessionBuilder, SetSimulationResolutionArgs, SimulationOptions,
    SimulationResolution,
};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType};

/// The R-plane clearance of a Drill above the top of its hole
/// (`compute::config::SAFE_Z_CLEARANCE_MM`).
const CLEARANCE: f64 = rs_cam_core::compute::config::SAFE_Z_CLEARANCE_MM;
const TOOL_D: f64 = 6.0;
const CELL_MM: f64 = 0.5;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// A peck Drill to `depth` below its Top Z, peck 3 mm.
fn drill(depth: f64) -> OperationConfig {
    let mut op = OperationConfig::Drill(DrillConfig {
        floor: Floor::Top { depth },
        cycle: DrillCycleType::Peck,
        peck_depth: Some(3.0),
        ..DrillConfig::default()
    });
    rs_cam_core::compute::fixture_values::fill_unset_geometry(&mut op);
    op
}

fn holes_model(name: &str, holes: &[[f64; 2]]) -> rs_cam_core::session::LoadedModel {
    let targets: Vec<TestHole> = holes
        .iter()
        .map(|&[x, y]| TestHole {
            x,
            y,
            layer: "holes".to_owned(),
            diameter: None,
        })
        .collect();
    let mut model = polygon_model(Vec::new(), name);
    model.items = Arc::new(drawing_items(Vec::new(), &targets));
    model
}

fn at(m: &Move, xy: [f64; 2]) -> bool {
    (m.target.x - xy[0]).abs() < 1e-9 && (m.target.y - xy[1]).abs() < 1e-9
}

/// The R-plane of the hole at `xy`: the Z of the second rapid at the hole
/// (the first is the rapid at the safe Z). On the remaining stock the
/// dressups turn a peck through air into rapids, so the first feed does
/// not start on the R-plane.
fn r_plane(moves: &[Move], xy: [f64; 2]) -> f64 {
    moves
        .iter()
        .filter(|m| m.move_type == MoveType::Rapid && at(m, xy))
        .nth(1)
        .expect("a hole rapids to its R-plane")
        .target
        .z
}

/// THE wood rule: at each hole the wood top starts at `standing(hole)` and
/// falls to the end of each feed there; no rapid at the hole ends below it.
/// Returns the count of rapids that it read at the holes.
fn rapids_stay_over_the_wood(
    moves: &[Move],
    holes: &[[f64; 2]],
    standing: impl Fn([f64; 2]) -> f64,
) -> usize {
    let mut wood: Vec<f64> = holes.iter().map(|&h| standing(h)).collect();
    let mut read = 0;
    for (index, m) in moves.iter().enumerate() {
        let Some(k) = holes.iter().position(|&h| at(m, h)) else {
            continue;
        };
        if m.move_type == MoveType::Rapid {
            read += 1;
            assert!(
                m.target.z >= wood[k] - 1e-9,
                "move {index}: a rapid at hole {:?} ends at Z {}, under wood that stands to Z {}",
                holes[k],
                m.target.z,
                wood[k]
            );
        } else {
            wood[k] = wood[k].min(m.target.z);
        }
    }
    read
}

/// Claim 1.
#[test]
fn a_pinned_top_under_the_stock_top_keeps_the_r_plane_over_the_stock() {
    // The stock top is at Z 18 (the frame of the job: Z 0 on the table), so
    // the saved absolute `retract_z` of the Drill (2.0) is under the stock.
    const STOCK_TOP: f64 = 18.0;
    const PINNED_TOP: f64 = 9.0;
    let hole = [5.0, 5.0];
    let mut s = single_op_session_with(
        stock_over(40.0, STOCK_TOP),
        endmill_tool_config(TOOL_D),
        holes_model("p36_fresh", &[hole]),
        "p36 fresh drill",
        drill(5.0),
        |tc| tc.heights.top_z = HeightMode::Manual(PINNED_TOP),
    );
    let result = s.generate_toolpath(0, &AtomicBool::new(false)).unwrap();
    let moves = &result.op_data.annotated().toolpath.moves;
    let r = r_plane(moves, hole);
    assert!(
        near(r, STOCK_TOP + CLEARANCE),
        "the R-plane is the clearance over the stock top, not over the pinned top: {r}"
    );
    let read = rapids_stay_over_the_wood(moves, &[hole], |_| STOCK_TOP);
    assert!(read >= 2, "the hole has rapids to read: {read}");
    // The cut does not move (F19): the floor is the pinned top less 5.
    let lowest = moves
        .iter()
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    assert!(near(lowest, PINNED_TOP - 5.0), "the floor: {lowest}");
    // The drill-op view names the same R-plane.
    let view = result.op_data.drill_op().expect("a drill-op view");
    assert!(near(view.holes[0].retract_z, r), "the view R-plane");
    assert!(near(view.holes[0].top_z, PINNED_TOP), "the view cut start");
}

const HALF: f64 = 15.0;
const POCKET_DEPTH: f64 = 7.0;
const STOCK_Z: f64 = 12.0;
/// The pocket floor: the stock top (Z 12) less the pocket depth.
const POCKET_FLOOR: f64 = STOCK_Z - POCKET_DEPTH;
const HOLE_IN_POCKET: [f64; 2] = [0.0, 0.0];
const HOLE_BESIDE_POCKET: [f64; 2] = [HALF + 8.0, 0.0];

fn two_op_stock() -> StockConfig {
    StockConfig {
        x: 2.0 * HALF + 20.0,
        y: 2.0 * HALF + 4.0,
        z: Some(STOCK_Z),
        origin_x: -HALF - 2.0,
        origin_y: -HALF - 2.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn simulate(session: &mut ProjectSession) {
    let _ = session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(CELL_MM),
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &AtomicBool::new(false),
        )
        .expect("simulation");
}

/// Claim 2.
#[test]
fn a_rest_drill_keeps_the_r_plane_over_the_column_that_stands() {
    let mut builder = ProjectSessionBuilder::new().stock(two_op_stock());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_D));
    let tool_id = rs_cam_core::ids::ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let pocket_model = builder.add_model(polygon_model(vec![square_polygon(HALF)], "pocket"));
    let holes = [HOLE_IN_POCKET, HOLE_BESIDE_POCKET];
    let holes_id = builder.add_model(holes_model("p36_rest", &holes));
    let pocket = OperationConfig::Pocket(PocketConfig {
        stepover: Some(2.0),
        floor: Floor::top(POCKET_DEPTH),
        depth_per_pass: Some(1.0),
        ..PocketConfig::default()
    });
    let _ = builder
        .add_toolpath(0, toolpath_config("pocket", pocket, tool_id, pocket_model))
        .expect("add the pocket");
    let mut drill_tp = toolpath_config("drill", drill(3.0), tool_id, holes_id);
    drill_tp.stock_source = StockSource::FromRemainingStock;
    drill_tp.heights.top_z = HeightMode::Manual(POCKET_FLOOR);
    let _ = builder.add_toolpath(0, drill_tp).expect("add the drill");
    let mut session = builder.build();

    generate(&mut session, 0);
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("a positive cell");
    simulate(&mut session);
    generate(&mut session, 1);

    let result = session.result_at(1).expect("the drill holds a result");
    let moves = &result.toolpath().moves;
    let in_pocket = r_plane(moves, HOLE_IN_POCKET);
    assert!(
        near(in_pocket, POCKET_FLOOR + CLEARANCE),
        "the hole in the pocket keeps its clearance over the pocket floor: {in_pocket}"
    );
    let beside = r_plane(moves, HOLE_BESIDE_POCKET);
    assert!(
        near(beside, STOCK_Z + CLEARANCE),
        "the hole beside the pocket has its R-plane over the stock top (Z 12) that stands \
         there, not over the pinned top: {beside}"
    );
    let standing = |h: [f64; 2]| {
        if h == HOLE_IN_POCKET {
            POCKET_FLOOR
        } else {
            STOCK_Z
        }
    };
    let read = rapids_stay_over_the_wood(moves, &holes, standing);
    assert!(read >= 4, "both holes have rapids to read: {read}");
    // The drill-op view names the same R-planes; the cut start keeps the
    // pinned top.
    let view = result.op_data.drill_op().expect("a drill-op view");
    for h in &view.holes {
        assert!(near(h.retract_z, r_plane(moves, h.xy)), "view R-plane");
        assert!(near(h.top_z, POCKET_FLOOR), "view cut start: {}", h.top_z);
    }
}

/// A staggered grid of holes (the pitch of the job's light holes), listed
/// in an order that is not the travel order.
fn grid() -> Vec<[f64; 2]> {
    let mut holes = Vec::new();
    for row in 0..6 {
        for col in 0..7 {
            let shift = if row % 2 == 0 { 0.0 } else { 9.57 };
            holes.push([
                -60.0 + shift + 19.14 * f64::from(col),
                -45.0 + 16.575 * f64::from(row),
            ]);
        }
    }
    // Deal the holes into three hands, so the drawing order jumps.
    let mut dealt = Vec::with_capacity(holes.len());
    for hand in 0..3 {
        dealt.extend(holes.iter().skip(hand).step_by(3).copied());
    }
    dealt
}

/// Claim 3.
#[test]
fn a_peck_cycle_with_its_r_plane_on_the_safe_z_stays_in_order() {
    const STOCK_TOP: f64 = 18.0;
    let holes = grid();
    // The retract plane of the job: the stock top + 5, which is also the
    // R-plane of a Drill with an automatic Top Z.
    let mut s = single_op_session_with(
        stock_over(80.0, STOCK_TOP),
        endmill_tool_config(TOOL_D),
        holes_model("p36_grid", &holes),
        "p36 grid drill",
        drill(10.0),
        |tc| tc.heights.retract_z = HeightMode::Manual(STOCK_TOP + CLEARANCE),
    );
    let tc = s.toolpath_configs()[0].clone();
    let safe_z = s.toolpath_heights(&tc).unwrap().heights.retract_z;
    let result = s.generate_toolpath(0, &AtomicBool::new(false)).unwrap();
    let moves = &result.op_data.annotated().toolpath.moves;
    // Not vacuous: the R-plane is the safe Z, the case of the defect.
    let r = r_plane(moves, holes[0]);
    assert!(
        near(r, safe_z),
        "the fixture needs the R-plane ({r}) on the safe Z ({safe_z})"
    );
    for &hole in &holes {
        let fed: Vec<f64> = moves
            .iter()
            .filter(|m| m.intent == MoveIntent::Drilling && at(m, hole))
            .map(|m| m.target.z)
            .collect();
        assert_eq!(fed.len(), 5, "hole {hole:?}: 15 mm from R 5 at a 3 mm peck");
        assert!(
            fed.windows(2).all(|w| w[1] < w[0]),
            "hole {hole:?}: the pecks are not in falling order: {fed:?}"
        );
    }
    let read = rapids_stay_over_the_wood(moves, &holes, |_| STOCK_TOP);
    assert!(read >= holes.len() * 4, "every hole has rapids: {read}");
    // The reorder still orders the holes: the visit order is not the
    // drawing order.
    let visit: Vec<usize> = moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Drilling)
        .filter_map(|m| holes.iter().position(|&h| at(m, h)))
        .fold(Vec::new(), |mut order, k| {
            if order.last() != Some(&k) {
                order.push(k);
            }
            order
        });
    assert_eq!(visit.len(), holes.len(), "each hole is visited one time");
    assert!(
        visit.windows(2).any(|w| w[1] != w[0] + 1),
        "the reorder changed the hole order: {visit:?}"
    );
}
