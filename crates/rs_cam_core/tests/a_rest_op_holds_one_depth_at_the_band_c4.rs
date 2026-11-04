//! C4 (G-RESTDOC; operator ruling D69, 2026-10-28): with "Contour each
//! level" on, a 3D Rough on remaining stock holds one Depth/Pass where its
//! tool meets stock beyond its raw boundary.
//!
//! The fixture is D63's (`phase3/d63/`, research `805368f98`). The stock
//! stands well past the model. The source (op 0, contour parallel,
//! `Center` on the silhouette, 1 mm leave) cuts out to the silhouette plus
//! one tool radius, so a band beyond that stays at the stock top. The rest
//! op (op 1, contour parallel on remaining stock, no leave) has the
//! silhouette grown by half a tool radius with `Center` containment: its
//! centre reaches that line, so its tool cuts the band out to one and a
//! half radii, one radius past its raw boundary. The boundary clear keeps
//! only the cells inside the raw line, where only the source's 1 mm skin
//! stands.
//!
//! - Option off: today's output. The level region holds no centre cell over
//!   the band, so the band is cut at the lowest level by the disc edge,
//!   deeper than one Depth/Pass (G-RESTDOC, pinned as an inverted
//!   assertion).
//! - Option on: the anchor reads the stock the tool meets before the
//!   boundary clear (the D63 port), so the first level is at most one
//!   Depth/Pass under the band; and a contour along the boundary on each
//!   level takes the band down one Depth/Pass at a time, so the simulated
//!   axial engagement stays within one Depth/Pass (plus one simulation
//!   cell, the dexel quantisation of the reading).

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
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, StockSource,
};
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
use rs_cam_core::ids::ToolId;
use rs_cam_core::session::{
    AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder, SimulationOptions,
    SimulationResolution,
};
use rs_cam_core::trace::toolpath_spans::{SpanKind, SpanPayload};
use std::sync::atomic::AtomicBool;

const TOP_Z: f64 = 10.0;
const PLATE_Z: f64 = 1.0;
const MESH_HALF: f64 = 30.0;
const MESH_STEP: f64 = 0.5;
const TOOL_DIAMETER: f64 = 6.0;
const DEPTH_PER_PASS: f64 = 3.0;
const STEPOVER: f64 = 2.0;
const SIM_CELL: f64 = 0.5;
/// The source's axial leave: the skin inside the rest op's raw line.
const SOURCE_LEAVE: f64 = 1.0;
/// How far the stock stands past the model: wider than two tool radii, so a
/// band stays beyond the source's reach and inside the rest op's.
const STOCK_MARGIN: f64 = 8.0;

fn rough(
    leave: f64,
    offset: f64,
    stock_source: StockSource,
    contour_each_level: bool,
) -> (OperationConfig, BoundaryConfig, StockSource) {
    (
        OperationConfig::Adaptive3d(Adaptive3dConfig {
            stepover: Some(STEPOVER),
            depth_per_pass: Some(DEPTH_PER_PASS),
            stock_to_leave_axial: leave,
            clearing_strategy: ClearingStrategy::ContourParallel,
            contour_each_level,
            ..Adaptive3dConfig::default()
        }),
        BoundaryConfig {
            enabled: true,
            source: BoundarySource::ModelSilhouette,
            containment: BoundaryContainment::Center,
            offset,
        },
        stock_source,
    )
}

/// The pair; the rest op's boundary is the silhouette grown by
/// `rest_offset`, and the rest op has "Contour each level" set to `contour`.
fn session(rest_offset: f64, contour: bool) -> ProjectSession {
    let mesh = height_field(MESH_HALF, MESH_STEP, |_, _| PLATE_Z);
    let mut stock = stock_over(MESH_HALF + STOCK_MARGIN, TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model_id = builder.add_model(mesh_model(mesh, "plate"));
    let (op, boundary, stock_source) = rough(SOURCE_LEAVE, 0.0, StockSource::Fresh, false);
    let mut op0 = toolpath_config("rough", op, tool_id, model_id);
    op0.boundary = boundary;
    op0.stock_source = stock_source;
    let _ = builder.add_toolpath(0, op0).expect("add the rough");
    let mut session = builder.build();
    let (op, boundary, stock_source) =
        rough(0.0, rest_offset, StockSource::FromRemainingStock, contour);
    let mut op1 = toolpath_config("rest", op, tool_id, model_id);
    op1.boundary = boundary;
    op1.stock_source = stock_source;
    // The debug trace states the planned level list ("Compute Z levels").
    op1.debug_options.enabled = true;
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(op1),
        }))
        .expect("add the rest op");
    session
}

fn simulate(session: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(SIM_CELL),
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulate");
}

/// What one run of the pair measures on the rest op.
struct RestRun {
    /// The plan's first level (`z_top` of the "Compute Z levels" span).
    planned_first: f64,
    /// The highest level that emitted a depth pass.
    emitted_first: f64,
    /// The simulated axial engagement of the rest op's cutting samples.
    max_axial: f64,
    cutting: usize,
    /// The contour on each level (the level spans' counters): its cuts,
    /// the highest chip-area load of a cut move before the cap, and the
    /// lowest cap factor (1: no cap).
    contour_runs: f64,
    contour_peak_load: f64,
    contour_min_factor: f64,
    /// The rest op's moves that carry `Move::feed_cap`.
    capped_moves: usize,
}

/// The rest op's offset that leaves the band beyond its raw line: half a
/// tool radius (its raw corners stay inside the source's rounded reach).
const BAND_BEYOND_RAW: f64 = TOOL_DIAMETER / 4.0;

fn run(rest_offset: f64, contour: bool) -> RestRun {
    let mut session = session(rest_offset, contour);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate the rough");
    simulate(&mut session);
    session
        .generate_toolpath(1, &cancel)
        .expect("generate the rest op");
    let result = session.result_at(1).expect("the rest op's result");
    let spans = &result
        .debug_trace
        .as_ref()
        .expect("the rest op's debug trace")
        .spans;
    let planned_first = spans
        .iter()
        .find(|span| span.kind == "z_level_plan")
        .and_then(|span| span.counters.get("z_top").copied())
        .expect("the level plan span states z_top");
    let counter = |name: &str| -> Vec<f64> {
        spans
            .iter()
            .filter_map(|s| s.counters.get(name).copied())
            .collect()
    };
    let contour_runs: f64 = counter("contour_runs").into_iter().sum();
    let contour_peak_load = counter("contour_peak_load").into_iter().fold(0.0, f64::max);
    let contour_min_factor = counter("contour_min_cap_factor")
        .into_iter()
        .fold(1.0, f64::min);
    let capped_moves = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| m.feed_cap.is_some())
        .count();
    let emitted_first = result
        .annotated()
        .spans
        .iter()
        .filter_map(|span| match (span.kind, &span.payload) {
            (SpanKind::DepthPass, Some(SpanPayload::DepthPass { z_level, .. })) => Some(*z_level),
            _ => None,
        })
        .fold(f64::NEG_INFINITY, f64::max);

    simulate(&mut session);
    let rest_id = session.toolpath_configs()[1].id;
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("the cut trace");
    let (mut max_axial, mut cutting) = (0.0_f64, 0usize);
    for sample in trace.samples.iter().filter(|s| s.toolpath_id == rest_id) {
        if sample.removed_volume_est_mm3 > 0.0 {
            cutting += 1;
            max_axial = max_axial.max(sample.axial_engagement_mm);
        }
    }
    eprintln!(
        "rest op (contour each level {contour}): planned first level {planned_first:.3}, first emitted pass {emitted_first:.3} \
         (band top {TOP_Z}), {cutting} cutting samples, max axial {max_axial:.3} (DPP {DEPTH_PER_PASS}); \
         contour: {contour_runs} cuts, peak load {contour_peak_load:.4} before the cap, lowest cap \
         factor {contour_min_factor:.4}, {capped_moves} capped moves"
    );
    assert!(
        cutting > 0,
        "the rest op cut nothing: the fixture tests nothing"
    );
    RestRun {
        planned_first,
        emitted_first,
        max_axial,
        cutting,
        contour_runs,
        contour_peak_load,
        contour_min_factor,
        capped_moves,
    }
}

/// Option on, the anchor (the D63 port): the plan's first level is at
/// most one Depth/Pass under the band the tool meets beyond its raw
/// boundary.
#[test]
fn the_option_plans_the_first_level_one_depth_under_the_band_c4() {
    let r = run(BAND_BEYOND_RAW, true);
    assert!(
        r.planned_first >= TOP_Z - DEPTH_PER_PASS - 1e-6,
        "the rest op plans its first level at {:.3}, more than one Depth/Pass under the band at {TOP_Z}",
        r.planned_first
    );
}

/// Option on, G-RESTDOC fixed: the simulated axial engagement of the rest
/// op is at most one Depth/Pass plus one simulation cell, and its first
/// emitted pass is at most one Depth/Pass under the band.
#[test]
fn the_option_holds_one_depth_at_the_band_c4() {
    let r = run(BAND_BEYOND_RAW, true);
    assert!(
        r.max_axial <= DEPTH_PER_PASS + SIM_CELL + 1e-6,
        "with the option on the rest op still cuts {:.3} mm axially (DPP {DEPTH_PER_PASS}, \
         one sim cell {SIM_CELL})",
        r.max_axial
    );
    assert!(
        r.emitted_first >= TOP_Z - DEPTH_PER_PASS - 1e-6,
        "with the option on the first emitted pass is at {:.3}, more than one Depth/Pass under \
         the band at {TOP_Z}",
        r.emitted_first
    );
}

/// Option off, G-RESTDOC, known defect, as an inverted assertion that runs
/// (`tests/CLAUDE.md`): the option is off by default, and off the output is
/// today's. The level region holds no centre cell over the reach band, so
/// the band is cut at the lowest level by the edge of the disc, more than
/// one Depth/Pass deep. If this FAILS with the option off, the default
/// output changed: that is outside the ruling.
#[test]
fn g_restdoc_stays_with_the_option_off_c4() {
    let r = run(BAND_BEYOND_RAW, false);
    assert!(
        r.max_axial > DEPTH_PER_PASS + SIM_CELL && r.emitted_first < TOP_Z - DEPTH_PER_PASS - 1e-6,
        "G-RESTDOC no longer reproduces with the option off (max axial {:.3}, first emitted pass \
         {:.3}, {} cutting samples): the default output changed",
        r.max_axial,
        r.emitted_first,
        r.cutting
    );
}

/// Option on, the load (the lead's addition to C4): a contour at the disc
/// edge is close to a half slot. Its moves are read by the one load
/// (`chip_load_fraction`); where a cut passes the pass ceiling (the radial
/// fraction of `adaptive::pass_engagement_limit`, the Geodesic step cap's
/// ceiling) it carries `Move::feed_cap` at a factor under 1.
#[test]
fn the_option_caps_the_contour_at_the_pass_load_c4() {
    let r = run(BAND_BEYOND_RAW, true);
    let ceiling = rs_cam_core::ops::adaptive_shared::radial_woc_fraction_from_leading_arc(
        rs_cam_core::adaptive::pass_engagement_limit(STEPOVER, TOOL_DIAMETER / 2.0),
    );
    eprintln!(
        "contour load: peak {:.4} against the ceiling {ceiling:.4}; with the cap the feed \
         factor is at least {:.4}, so the peak load per unit of feed is {:.4}",
        r.contour_peak_load,
        r.contour_min_factor,
        r.contour_peak_load * r.contour_min_factor
    );
    assert!(
        r.contour_runs > 0.0,
        "the contour cut nothing: the fixture tests nothing"
    );
    if r.contour_peak_load > ceiling {
        assert!(
            r.capped_moves > 0 && r.contour_min_factor < 1.0,
            "a contour cut reads {:.4} over the ceiling {ceiling:.4} but carries no cap",
            r.contour_peak_load
        );
    }
}

/// Option on, determinism: two runs in one process give the same moves.
/// The contour's closed ring starts at its point nearest the tool, not at
/// the containment polygon's first vertex, which the polygon library does
/// not fix from run to run (found while writing C4: before the rule, three
/// runs gave 117, 118 and 126 moves).
#[test]
fn the_option_is_deterministic_c4() {
    let moves = || {
        let mut session = session(BAND_BEYOND_RAW, true);
        let cancel = AtomicBool::new(false);
        session
            .generate_toolpath(0, &cancel)
            .expect("generate the rough");
        simulate(&mut session);
        session
            .generate_toolpath(1, &cancel)
            .expect("generate the rest op");
        format!(
            "{:?}",
            session
                .result_at(1)
                .expect("the rest op's result")
                .toolpath()
                .moves
        )
    };
    assert_eq!(moves(), moves(), "two runs of the same rest op differ");
}
