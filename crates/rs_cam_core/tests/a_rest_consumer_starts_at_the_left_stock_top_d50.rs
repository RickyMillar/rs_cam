//! D50 (`planning/adaptive_research_2026-10-23/phase3/d48/README.md` §3,
//! proposal c; integration step 7.5b, ported from the research branch
//! 3682d1dcc onto the product arm): a 3D Rough publishes the top Z of the stock it left in its
//! rest regions, and a consumer on remaining stock plans no level above it.
//!
//! The fixture is the stepped pocket of the D44 sentries with two channels
//! below the wide pocket, so the source leaves pieces at its lowest level
//! only. The consumer's boundary is then two regions, and the planner does
//! not pre-clear the stock outside them. The source leaves more axial stock
//! than the consumer, so a skin stays on the wide floor above the top of
//! the channels' slab. Before D50 the consumer's level plan started at that
//! skin (the G-LADDERANCHOR anchor) and cut at an upper level first (a
//! waterline cleanup at Z 7, before the floor's pass). Now the published
//! top is the anchor's ceiling: no level above the source's lowest level
//! cuts, and nothing cuts before the floor's pass.
//!
//! The source is Geodesic Adaptive (`clearing_strategy = geodesic_adaptive`).

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use common::meshes::height_field;
use common::session::{mesh_model, stock_over, toolpath_config};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundarySource, StockSource};
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
use rs_cam_core::ids::ToolId;
use rs_cam_core::session::{
    AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder, SimulationOptions,
    SimulationResolution,
};
use rs_cam_core::trace::toolpath_spans::{SpanKind, SpanPayload};
use std::sync::atomic::AtomicBool;

const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const CHANNEL_HALF_LENGTH: f64 = 30.0;
const CHANNEL_HALF_WIDTH: f64 = 4.8;
/// The Y of the two channel centre lines.
const CHANNEL_Y: f64 = 12.0;
const WIDE_HALF_X: f64 = 34.0;
const WIDE_HALF_Y: f64 = 24.0;
const WIDE_FLOOR_Z: f64 = 7.0;
const TOOL_DIAMETER: f64 = 6.0;
const DEPTH_PER_PASS: f64 = 3.0;
/// The source's axial leave; the consumer leaves none.
const SOURCE_LEAVE: f64 = 1.0;
const SIM_CELL: f64 = 0.5;

fn rough(strategy: ClearingStrategy, leave: f64, depth_per_pass: f64) -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: Some(2.0),
        depth_per_pass: Some(depth_per_pass),
        stock_to_leave_axial: leave,
        clearing_strategy: strategy,
        ..Adaptive3dConfig::default()
    })
}

fn in_channel(x: f64, y: f64, inset: f64) -> bool {
    x.abs() <= CHANNEL_HALF_LENGTH - inset
        && ((y - CHANNEL_Y).abs() <= CHANNEL_HALF_WIDTH - inset
            || (y + CHANNEL_Y).abs() <= CHANNEL_HALF_WIDTH - inset)
}

/// Op 0 the source rough, op 1 its consumer on remaining stock with
/// `consumer_dpp`.
fn session(consumer_dpp: f64) -> ProjectSession {
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if in_channel(x, y, 0.0) {
            FLOOR_Z
        } else if x.abs() <= WIDE_HALF_X && y.abs() <= WIDE_HALF_Y {
            WIDE_FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let mut stock = stock_over(MESH_HALF, TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model_id = builder.add_model(mesh_model(mesh, "channels"));
    let mut op0 = toolpath_config(
        "rough",
        rough(
            ClearingStrategy::GeodesicAdaptive,
            SOURCE_LEAVE,
            DEPTH_PER_PASS,
        ),
        tool_id,
        model_id,
    );
    op0.rest_analysis.enabled = true;
    let index0 = builder.add_toolpath(0, op0).expect("add the rough");
    let mut session = builder.build();
    let source_toolpath_id = session.toolpath_configs()[index0].id;
    let mut op1 = toolpath_config(
        "edges, CP rest",
        rough(ClearingStrategy::ContourParallel, 0.0, consumer_dpp),
        tool_id,
        model_id,
    );
    op1.stock_source = StockSource::FromRemainingStock;
    op1.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::DerivedRestRegions { source_toolpath_id },
        ..BoundaryConfig::default()
    };
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(op1),
        }))
        .expect("add the consumer");
    session
}

fn generate(session: &mut ProjectSession, index: usize) {
    let cancel = AtomicBool::new(false);
    if let Err(e) = session.generate_toolpath(index, &cancel) {
        panic!("generating toolpath {index}: {e}");
    }
}

#[test]
fn a_rest_consumer_starts_at_the_left_stock_top() {
    let mut session = session(DEPTH_PER_PASS);
    generate(&mut session, 0);
    let produced = &session
        .result_at(0)
        .expect("the rough's result")
        .annotated()
        .produced;
    let regions = produced
        .rest_regions
        .as_deref()
        .expect("the rough states the regions it left")
        .len();
    let top = produced
        .rest_regions_top_z
        .expect("the rough states the top of the stock it left");
    eprintln!("rough: {regions} regions, left stock top {top:.3}");
    // The pieces are the channels at the lowest level only: their slab
    // reaches up to the level above, the wide floor level.
    assert!(
        (top - WIDE_FLOOR_Z).abs() < 1e-9,
        "the left stock top {top} is not the level above the channels ({WIDE_FLOOR_Z}): \
         the fixture leaves pieces at an upper level"
    );
    assert!(regions >= 2, "the consumer needs two regions: {regions}");

    let cancel = AtomicBool::new(false);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(SIM_CELL),
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulate the rough");
    generate(&mut session, 1);
    let annotated = session
        .result_at(1)
        .expect("the consumer's result")
        .annotated();
    assert!(annotated.spans_valid, "the consumer's spans are not valid");
    let moves = &annotated.toolpath.moves;

    // The consumer's level plan. Each level that holds work opens a depth
    // pass with its Z. The source left stock only at its lowest level, the
    // floor, so every pass that cuts must be at the floor, and nothing may
    // cut before the floor's pass. A level above the floor can hold no
    // region work and still cut: a waterline cleanup of that level (the
    // skin of the wide floor), before the floor's pass. An entry or a link
    // is not a cut of a level. A level that cuts nothing (a Detect Flat
    // shelf at the top) is allowed.
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    let tol = match &session.toolpath_configs()[1].operation {
        OperationConfig::Adaptive3d(c) => c.tolerance,
        _ => panic!("op 1 is not a 3D Rough"),
    };
    let passes: Vec<(usize, f64, u32)> = annotated
        .spans
        .iter()
        .filter_map(|span| match (span.kind, &span.payload) {
            (
                SpanKind::DepthPass,
                Some(SpanPayload::DepthPass {
                    z_level,
                    pass_index,
                }),
            ) => Some((span.start_move, *z_level, *pass_index)),
            _ => None,
        })
        .collect();
    let transit = |i: usize| {
        annotated.spans.iter().any(|span| {
            matches!(span.kind, SpanKind::Entry | SpanKind::LinkBridge)
                && (span.start_move..span.end_move).contains(&i)
        })
    };
    let floor_pass = passes
        .iter()
        .filter(|(_, z, _)| (*z - FLOOR_Z).abs() <= tol)
        .min_by_key(|(start, _, _)| *start)
        .copied()
        .expect("the consumer has a pass at the floor level");
    let early_cuts: Vec<f64> = moves
        .iter()
        .enumerate()
        .take(floor_pass.0)
        .filter(|(i, m)| m.move_type.is_cutting() && !transit(*i))
        .map(|(_, m)| m.target.z)
        .collect();
    eprintln!(
        "consumer: {} moves, passes (start, z, index) {passes:?}, {} cuts before the floor pass",
        moves.len(),
        early_cuts.len()
    );
    let upper: Vec<f64> = passes
        .iter()
        .filter(|(start, z, _)| {
            *z > FLOOR_Z + tol
                && moves
                    .iter()
                    .enumerate()
                    .skip(*start)
                    .take_while(|(i, _)| {
                        !passes
                            .iter()
                            .any(|(other, _, _)| other > start && i >= other)
                    })
                    .any(|(i, m)| m.move_type.is_cutting() && !transit(i))
        })
        .map(|(_, z, _)| *z)
        .collect();
    assert!(
        upper.is_empty(),
        "the consumer cuts at levels above the source's lowest level (Z {FLOOR_Z}): {upper:?}"
    );
    assert!(
        early_cuts.is_empty(),
        "the consumer cuts before its floor pass, at Z {early_cuts:?}"
    );
    // Integration design §3.1: the top lowers only the start, never the
    // floor. The consumer leaves no stock, so its floor is the surface
    // bottom, the channel floor: its lowest pass is there.
    let lowest = passes
        .iter()
        .map(|(_, z, _)| *z)
        .fold(f64::INFINITY, f64::min);
    assert!(
        (lowest - FLOOR_Z).abs() <= tol,
        "the consumer's lowest level is {lowest}, not its floor {FLOOR_Z}: the top moved the floor"
    );
}
