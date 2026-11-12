//! P22f-2a (feeds contract, `planning/feeds_contract_2026-11-03/phases/P22f-2a.md`):
//! a Geodesic Adaptive cut reads every fed step on the 3D planner stock.
//!
//! The fixture is the shelf pocket of the D65 sentry, every length scaled by
//! `k = D / 6`, with an 18 mm tool (the fixture of
//! `a_geodesic_slice_cuts_no_full_slot_sweep_p22f`). Its shelf is a 7.5 mm
//! terrace above the level. Since P22f-1 the grown 2D domain covers the
//! terrace; the 2D plan holds its cells cut while the lift drapes and splits
//! the crossings, so an arc that runs down to the terrace foot can meet
//! standing stock (C22 §11.10 H1; P22f-2 design).
//!
//! Claim 1 (through `ProjectSession`, generated and simulated): no fed
//! sample of an uncapped clearing move reads past `c D` plus one planner
//! cell plus one simulation cell with real removal (removal ratio at least
//! `REAL_REMOVAL`). Each such step is capped or split by the 3D stock read.
//! Two named classes of the P22a readout are not such steps and are
//! skipped: a phantom-transit sample (the gates' own filter) and a joint
//! sample (within the read precision of the end a move shares with a capped
//! or draped step; the simulation reads that step's stock there).
//!
//! Claim 2 (through the test-support probe `debug_level_plan_probe`, the
//! planner run directly with the parameters of the P22f-1 sentry): after
//! the level, no cell that the level's 2D plans hold cut stands in the 3D
//! planner stock more than one planner cell (the vertical read tolerance of
//! the planner stock) above the level's floor. The 2D plan and the 3D stock
//! agree at the end of the level. The band-edge load is in time order: an
//! arc end meets stock that a lifted or split crossing left, and later
//! moves of the level cut it; the 3D read of every fed step (claim 1)
//! states the feed of that arc end. (Measured 2026-11-10: 12288 plan-cut
//! cells, 0 standing; the design's "the 2D plan holds standing stock cut
//! after the level" did not hold.)
//!
//! Red before P22f-2a (2026-11-10, on `129deb5c2`): claim 1 found 2 uncapped
//! clearing samples past the read precision with real removal (1952 moves,
//! 190 capped). Inject check: with the 3D read of the flat steps switched
//! off, the same 2 samples come back.

#![expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "test code: an expect is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::session::{mesh_model, single_op_session_with, stock_over};
use common::tools::endmill_tool_config;
use common::wide_samples::{
    REAL_REMOVAL, at_cap_joint, ceiling_fraction, engagement, planner_cell, removal_ratio,
};

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, KeepDownRoute, RegionOrdering, StepSplit, debug_level_plan_probe,
};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy,
};
use rs_cam_core::limits::{CapTable, CutGroup, SampleContext};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::session::{ProjectSession, SimulationOptions, SimulationResolution};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::tool_load::locality::judged_group;
use rs_cam_core::toolpath::MoveIntent;

/// The tool diameter of this sentry and of the D65 fixture (mm).
const D: f64 = 18.0;
const D65_D: f64 = 6.0;
/// The scale of every length of the D65 fixture.
const K: f64 = D / D65_D;

// The D65 fixture, as in `a_geodesic_step_over_the_ceiling_is_cut_capped_d65`.
const STEPOVER: f64 = 2.0;
const TOLERANCE: f64 = 0.1;
const STOCK_TO_LEAVE: f64 = 0.5;
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const POCKET_HALF_X: f64 = 34.0;
const POCKET_HALF_Y: f64 = 24.0;
const ISLAND_RADIUS: f64 = 5.0;
const SHELF_Z: f64 = 6.5;
const SHELF_X: (f64, f64) = (12.0, 30.0);
const SHELF_HALF_Y: f64 = 12.0;
/// The P22a fixture arm's simulation cell, scaled.
const SIM_CELL: f64 = K * 0.25;

fn shelf() -> TriangleMesh {
    height_field(K * MESH_HALF, K * MESH_STEP, |x, y| {
        if x.hypot(y) <= K * ISLAND_RADIUS {
            K * TOP_Z
        } else if (K * SHELF_X.0..=K * SHELF_X.1).contains(&x) && y.abs() <= K * SHELF_HALF_Y {
            K * SHELF_Z
        } else if x.abs() <= K * POCKET_HALF_X && y.abs() <= K * POCKET_HALF_Y {
            K * FLOOR_Z
        } else {
            K * TOP_Z
        }
    })
}

fn session() -> ProjectSession {
    let op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: Some(K * STEPOVER),
        depth_per_pass: Some(K * (TOP_Z - FLOOR_Z)),
        tolerance: K * TOLERANCE,
        clearing_strategy: ClearingStrategy::GeodesicAdaptive,
        entry_style: Adaptive3dEntryStyle::Ramp,
        ..Adaptive3dConfig::default()
    });
    single_op_session_with(
        stock_over(K * MESH_HALF, K * TOP_Z),
        endmill_tool_config(D),
        mesh_model(shelf(), "scaled shelf"),
        "scaled shelf rough",
        op,
        |_| {},
    )
}

/// The parameters of `a_geodesic_slice_cuts_no_full_slot_sweep_p22f` (the
/// D65 sentry's, scaled by `K`).
fn params() -> Adaptive3dParams {
    let r = D / 2.0;
    Adaptive3dParams {
        entry_clearance_mm: K * 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: r,
            envelope_radius: r,
            stepover: K * STEPOVER,
            tolerance: K * TOLERANCE,
            segment_merge_tolerance: None,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
            centre_boundary: Vec::new(),
            gouge_probe: None,
        },
        depth: Adaptive3dDepth {
            depth_per_pass: K * (TOP_Z - FLOOR_Z),
            stock_to_leave: K * STOCK_TO_LEAVE,
            stock_top_z: K * TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
            rest_top_z: None,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: K * 0.5,
            step_split: Some(StepSplit::StockDepth),
            keep_down_route: Some(KeepDownRoute::CutFeed),
            level_contour: None,
            rest_region_filter: None,
        },
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: K * 15.0,
        entry_style: EntryStyle3d::Plunge,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::GeodesicAdaptive,
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
    }
}

#[test]
fn the_plan_and_the_planner_stock_agree_after_the_level() {
    let mesh = shelf();
    let index = SpatialIndex::build(&mesh, K * 10.0);
    let cutter = FlatEndmill::new(D, K * 25.0);
    let levels = debug_level_plan_probe(&mesh, &index, &cutter, &params(), &|| false)
        .expect("the probe is not cancelled");
    let cell = planner_cell(D / 2.0, K * TOLERANCE);
    let mut standing = Vec::new();
    let mut points = 0usize;
    for level in &levels {
        points += level.points.len();
        for p in &level.points {
            if p.top_after.is_some_and(|top| top > p.floor_z + cell) {
                standing.push((level.z_level, p.x, p.y, p.floor_z, p.top_after));
            }
        }
    }
    eprintln!(
        "probe: {} levels, {points} plan-cut cells, {} stand more than one planner cell ({cell}) \
         above the floor; first: {:?}",
        levels.len(),
        standing.len(),
        standing.iter().take(6).collect::<Vec<_>>()
    );
    assert!(points > 0, "non-vacuity: the probe read no plan-cut cell");
    assert!(
        standing.is_empty(),
        "plan-cut cells that stand in the planner stock after the level (z, x, y, floor, top): {standing:?}"
    );
}

/// One uncapped fed clearing sample past the read precision with real
/// removal: (move, ae mm, ap mm, removal ratio, position).
type Wide = (usize, f64, f64, f64, [f64; 3]);

#[test]
fn a_geodesic_arc_end_reads_the_stock() {
    let mut s = session();
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel)
        .expect("the rough generates");
    s.run_simulation(
        &SimulationOptions {
            resolution: SimulationResolution::Fixed(SIM_CELL),
            skip_ids: Vec::new(),
            use_predicted_feed_in_gates: false,
            adaptive_feed_modulation: false,
            modulation_strategy:
                rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
            modulation_feed_scale: 1.0,
        },
        &cancel,
    )
    .expect("the simulation runs");
    let tc = s.toolpath_config_at(0).expect("the op");
    let tool = rs_cam_core::compute::cutter::build_cutter(
        s.tool_by_id(tc.tool_id.expect("a tool")).expect("the tool"),
    )
    .expect("the cutter builds");
    let result = s.result_at(0).expect("a result");
    let moves = &result.toolpath().moves;
    let annotated = result.annotated();
    let spans = annotated.spans_valid.then_some(annotated.spans.as_slice());
    let caps = CapTable::of_toolpath(&annotated.toolpath, result.semantic_trace.as_deref());
    let sample_ctx = SampleContext::new(&tool, spans, &caps, tc.operation.cuts().body_facts());
    let mut phantom = 0usize;
    let mut joint = 0usize;
    let trace = s
        .simulation_result()
        .and_then(|r| r.cut_trace.as_deref())
        .expect("a cut trace");
    let r = D / 2.0;
    let ceiling = ceiling_fraction(K * STEPOVER, r) * D;
    let precision = planner_cell(r, K * TOLERANCE) + SIM_CELL;
    let mut wide: Vec<Wide> = Vec::new();
    let mut graded = 0usize;
    for sample in &trace.samples {
        if sample.toolpath_id != tc.id || !sample.is_cutting {
            continue;
        }
        let Some(e) = engagement(sample, &tool) else {
            continue;
        };
        graded += 1;
        let Some(m) = moves.get(sample.move_index) else {
            continue;
        };
        if m.intent != MoveIntent::ClearingCut || m.feed_cap.is_some() {
            continue;
        }
        // P29-6 6d: a sample of the Cleanup or the Link group (a waterline
        // cleanup, a link) is judged in its own group, not in the body
        // (P22a's former phantom class).
        if matches!(
            judged_group(sample, &sample_ctx),
            Some(CutGroup::Cleanup | CutGroup::Link)
        ) {
            phantom += 1;
            continue;
        }
        if e.ae_mm() <= ceiling + precision {
            continue;
        }
        // The joint with a capped or draped step: the sample reads that
        // step's stock at the shared point (P22a's named "joint" class).
        if at_cap_joint(moves, sample.move_index, sample.position, precision) {
            joint += 1;
            continue;
        }
        let ratio = removal_ratio(sample, &e).unwrap_or(0.0);
        if ratio >= REAL_REMOVAL {
            wide.push((
                sample.move_index,
                e.ae_mm(),
                e.ap_mm(),
                ratio,
                sample.position,
            ));
        }
    }
    let capped = moves.iter().filter(|m| m.feed_cap.is_some()).count();
    eprintln!(
        "scaled shelf (D {D}): {} moves, {capped} capped, {graded} graded samples; c D {ceiling:.3} \
         + precision {precision:.3}; {phantom} phantom-transit and {joint} joint samples skipped; uncapped real wide \
         samples: {}",
        moves.len(),
        wide.len()
    );
    for w in wide.iter().take(12) {
        eprintln!("  wide: {w:?}");
    }
    assert!(graded > 0, "non-vacuity: no graded sample");
    assert!(
        wide.is_empty(),
        "uncapped clearing samples past c D plus both cells with real removal (move, ae, ap, ratio, at): {wide:?}"
    );
}
