//! P35 part B: the pass ladder of a Project Curve on the remaining stock.
//!
//! Operator (2026-11-15): "Can we not do something clever with rest? stock
//! top but cull the passes that are air cuts?".
//!
//! The fixture: a straight curve along Y 0 from X -40 to X 40, its full
//! depth at Z 8, in a board with its top at Z 20. A pocket before it cuts a
//! floor at Z 12 over X -45 to X 20. So the curve runs under the floor at
//! Z 12 over most of its length and under the full height Z 20 over the
//! short part X 20 to X 40. `depth_per_pass` is 2.0 mm.
//!
//! The door is the one of every rest op: `stock_source =
//! FromRemainingStock`, the prior op generated and simulated, then the op.
//!
//! # The claims
//!
//! - (a) Whatever top Z the heights state (the stock top 20, or the floor
//!   12, the operator case "top 16.5"), no simulated sample of the op cuts
//!   more than one step, and no fed move of a pass above the floor runs
//!   over the floor part. The ladder top is the stock top under the curve.
//! - (b) `Fresh` does not read the stock: the stated top is the ladder top,
//!   and the passes above the floor run over the floor part.
//! - (c) A curve wholly under the floor takes its ladder from the floor:
//!   2 passes, not the 3 that stay of a ladder from the stock top (the
//!   pass on the floor itself is not air for the filter).
//!
//! Measured with the simulation (`axial_engagement_mm`, `plunge_descent_mm`
//! and the air-cut time of the op's summary), not from the plan.
//!
//! Red proof: on the code before P35 (the ladder top is `heights.top_z`)
//! the arm "top 12" of (a) cuts 8.75 mm deep at the full height part (2
//! levels), and (c) has 3 levels (12, 10, 8). The arm "top 20" of (a) is
//! green before P35: the air-cut filter of the dressup pipeline drops the
//! passes above the floor for every op from remaining stock.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test project_curve_passes_read_the_remaining_stock_p35
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the measured figures are the record of the step"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::op_fixtures::{SIM_CELL_MM, run_prior, simulate};
use common::session::{mesh_model, pinned_heights, polygon_model, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{PocketConfig, ProjectCurveConfig};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder};
use rs_cam_core::toolpath::MoveType;

const TOOL_D: f64 = 6.0;
/// The board top, the pocket floor and the full depth of the curve (mm).
const STOCK_TOP: f64 = 20.0;
const FLOOR_Z: f64 = 12.0;
const FULL_Z: f64 = 8.0;
const STEP: f64 = 2.0;
/// The pocket covers X -45 to X 20; the curve runs X -40 to X 40.
const POCKET_X1: f64 = 20.0;
const CURVE_X0: f64 = -40.0;
const CURVE_X1: f64 = 40.0;

fn board() -> StockConfig {
    StockConfig {
        x: 100.0,
        y: 40.0,
        z: Some(STOCK_TOP),
        origin_x: -50.0,
        origin_y: -20.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The session: the pocket (toolpath 0), then the Project Curve (toolpath
/// 1) from `curve_x0` to `CURVE_X1` with the stated top Z.
fn session(source: StockSource, top_z: f64, curve_x0: f64, curve_x1: f64) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(board());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_D));
    let tool_id = rs_cam_core::ids::ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let pocket_model = builder.add_model(polygon_model(
        vec![Polygon2::new(vec![
            P2::new(-45.0, -10.0),
            P2::new(POCKET_X1, -10.0),
            P2::new(POCKET_X1, 10.0),
            P2::new(-45.0, 10.0),
        ])],
        "p35_pocket",
    ));
    // The surface: a flat plate 1 mm above the full depth of the curve.
    let surface = builder.add_model(mesh_model(
        height_field(60.0, 5.0, |_, _| FULL_Z + 1.0),
        "p35_plate",
    ));
    let curve = builder.add_model(polygon_model(
        vec![Polygon2::open_path(vec![
            P2::new(curve_x0, 0.0),
            P2::new(curve_x1, 0.0),
        ])],
        "p35_curve",
    ));
    let _ = builder
        .add_toolpath(
            0,
            toolpath_config(
                "pocket",
                OperationConfig::Pocket(PocketConfig {
                    stepover: Some(2.0),
                    floor: rs_cam_core::compute::floor::Floor::top(STOCK_TOP - FLOOR_Z),
                    depth_per_pass: Some(4.0),
                    ..PocketConfig::default()
                }),
                tool_id,
                pocket_model,
            ),
        )
        .expect("add the pocket");
    let mut river = toolpath_config(
        "river",
        OperationConfig::ProjectCurve(ProjectCurveConfig {
            depth: Some(1.0),
            depth_per_pass: Some(STEP),
            surface_model_id: Some(surface),
            ..ProjectCurveConfig::default()
        }),
        tool_id,
        curve,
    );
    river.heights = pinned_heights(top_z, 0.0);
    river.stock_source = source;
    let _ = builder.add_toolpath(0, river).expect("add the river");
    builder.build()
}

/// What the simulation and the stored path say about the river op.
#[derive(Debug)]
struct Measured {
    /// The deepest cut of a sample (mm): the axial engagement of a lateral
    /// sample or the descent of a plunge sample.
    deepest_mm: f64,
    /// The X span of that sample.
    deepest_at_x: f64,
    /// The share of the fed time that cuts air.
    air_share: f64,
    /// The distinct Z of the fed moves, the highest first.
    levels: Vec<f64>,
    /// The fed moves above the floor Z (plus the tolerance) whose target is
    /// over the floor part (X below the pocket end less the tool radius).
    fed_above_the_floor_over_the_floor_part: usize,
    fed_moves: usize,
}

fn measure(mut session: ProjectSession) -> Measured {
    run_prior(&mut session);
    let cancel = AtomicBool::new(false);
    let _ = session
        .generate_toolpath(1, &cancel)
        .expect("the river generates");
    let tp = session
        .result_at(1)
        .expect("a result")
        .annotated()
        .toolpath
        .clone();
    simulate(&mut session);
    let id = session.toolpath_configs()[1].id;
    let sim = session.simulation_result().expect("a simulation");
    let trace = sim.cut_trace.as_ref().expect("a cut trace");
    let (mut deepest_mm, mut deepest_at_x) = (0.0_f64, f64::NAN);
    for s in trace.samples.iter().filter(|s| s.toolpath_id == id) {
        let depth = s.axial_engagement_mm.max(s.plunge_descent_mm);
        if s.is_cutting && depth > deepest_mm {
            deepest_mm = depth;
            deepest_at_x = s.position[0];
        }
    }
    let summary = trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == id)
        .expect("the summary of the river");
    let fed: Vec<_> = tp
        .moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .collect();
    let mut levels: Vec<f64> = Vec::new();
    for m in &fed {
        // The cut levels only: a plunge passes through other Z.
        if (m.target.z * 2.0).fract().abs() < 1e-9
            && !levels.iter().any(|z| (z - m.target.z).abs() < 1e-6)
        {
            levels.push(m.target.z);
        }
    }
    levels.sort_by(|a, b| b.total_cmp(a));
    Measured {
        deepest_mm,
        deepest_at_x,
        air_share: summary.air_cut_time_s / summary.cutting_runtime_s.max(1e-9),
        levels,
        fed_above_the_floor_over_the_floor_part: fed
            .iter()
            .filter(|m| {
                m.target.z > FLOOR_Z + 0.1 + 1e-6 && m.target.x < POCKET_X1 - TOOL_D - SIM_CELL_MM
            })
            .count(),
        fed_moves: fed.len(),
    }
}

/// One step, plus one simulation cell: a sample reads the stock on the
/// cell grid.
const ONE_STEP_MM: f64 = STEP + SIM_CELL_MM;

/// (a)
#[test]
fn no_sample_cuts_more_than_one_step_whatever_the_stated_top_p35() {
    let fresh = measure(session(StockSource::Fresh, STOCK_TOP, CURVE_X0, CURVE_X1));
    for top_z in [STOCK_TOP, FLOOR_Z] {
        let m = measure(session(
            StockSource::FromRemainingStock,
            top_z,
            CURVE_X0,
            CURVE_X1,
        ));
        eprintln!("remaining stock, stated top {top_z}: {m:?}");
        assert!(
            m.deepest_mm <= ONE_STEP_MM,
            "stated top {top_z}: a sample at X {:.1} cuts {:.2} mm; one step is {STEP} mm \
             (bound {ONE_STEP_MM})",
            m.deepest_at_x,
            m.deepest_mm
        );
        assert_eq!(
            m.fed_above_the_floor_over_the_floor_part, 0,
            "stated top {top_z}: no pass above the floor runs over the part that the pocket cut"
        );
        assert_eq!(
            m.levels,
            vec![18.0, 16.0, 14.0, 12.0, 10.0, 8.0],
            "stated top {top_z}: the ladder starts at the stock top under the curve (20)"
        );
        assert!(m.fed_moves > 100, "non-vacuity: {} fed moves", m.fed_moves);
        // The air-cut share of the fed time, against the Fresh op from the
        // stock top (measured 2026-11-16: 0.223 against 0.558). What stays
        // is the pass at Z 12 on the floor at Z 12: the air-cut filter
        // calls a move air only with 0.1 mm of clearance.
        assert!(
            m.air_share < 0.5 * fresh.air_share,
            "stated top {top_z}: air share {:.3}, Fresh {:.3}",
            m.air_share,
            fresh.air_share
        );
    }
}

/// (b)
#[test]
fn a_fresh_op_reads_the_stated_top_and_not_the_stock_p35() {
    let m = measure(session(StockSource::Fresh, STOCK_TOP, CURVE_X0, CURVE_X1));
    eprintln!("fresh, stated top {STOCK_TOP}: {m:?}");
    assert_eq!(m.levels, vec![18.0, 16.0, 14.0, 12.0, 10.0, 8.0]);
    assert!(
        m.fed_above_the_floor_over_the_floor_part > 100,
        "a Fresh op feeds its upper passes over the floor part: {} moves",
        m.fed_above_the_floor_over_the_floor_part
    );
    let low = measure(session(StockSource::Fresh, FLOOR_Z, CURVE_X0, CURVE_X1));
    eprintln!("fresh, stated top {FLOOR_Z}: {low:?}");
    assert_eq!(
        low.levels,
        vec![10.0, 8.0],
        "the stated top is the ladder top"
    );
    assert!(
        low.deepest_mm > 2.0 * STEP,
        "the operator case: from a top at the floor a Fresh op cuts {:.2} mm deep at the full \
         height part",
        low.deepest_mm
    );
}

/// (c)
#[test]
fn a_curve_under_the_floor_takes_its_ladder_from_the_floor_p35() {
    // The curve ends 8 mm before the pocket wall: the tool (radius 3) and
    // one cell stay over the floor.
    let m = measure(session(
        StockSource::FromRemainingStock,
        STOCK_TOP,
        CURVE_X0,
        POCKET_X1 - 8.0,
    ));
    eprintln!("remaining stock, a curve under the floor: {m:?}");
    assert_eq!(m.levels, vec![10.0, 8.0], "2 passes from the floor at 12");
    assert!(m.deepest_mm <= ONE_STEP_MM, "{m:?}");
}
