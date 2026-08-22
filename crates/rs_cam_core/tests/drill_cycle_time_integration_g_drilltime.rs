//! **G-DRILLTIME** — the kinematics integrator must reach drill toolpaths.
//!
//! # The observation
//!
//! Found 2026-08-22 in the live validation of the G-TIMEEST consolidation, by
//! reading the GUI rather than the code. On a project that *carries* machine
//! kinematics, after a clean generate and simulate, every operator-facing
//! surface read `2:02:34 (cutting only, no accel)` — the **weakest** basis,
//! where wall clock was expected.
//!
//! The number was right and the label was honest. The gap reconciled exactly:
//!
//! | op | cutting mm | feed | naive time |
//! |---|---|---|---|
//! | Pin Drill | 74.0 | 300 | 14.8 s |
//! | Holes | 234.0 | 300 | 46.8 s |
//! | | | | **61.6 s** |
//!
//! **61.66 s of drill motion — 0.8 % of the runtime — relabelled 100 % of the
//! estimate**, and every layer behaved exactly as designed while it happened.
//!
//! # The mechanism, and the one-slot-two-facts root cause
//!
//! Drill toolpaths set `metrics_not_applicable` and publish
//! `drill_summaries` rather than a `toolpath_summaries` row.
//! `apply_kinematics_cycle_time` computed a breakdown for *every* toolpath in
//! the request — drills included — and then folded the project total over
//! `toolpath_summaries`, so the drill's answer was computed and discarded.
//! Downstream, `readiness::toolpath_cycle_time` found no summary, correctly
//! fell back to `cutting_distance / feed` with basis `CuttingOnly`, and
//! `worse()` correctly degraded the project.
//!
//! One slot was carrying two different facts — "has no engagement metrics"
//! and "was not integrated". `SimulationCutTrace::toolpath_runtimes` is the
//! second fact given its own slot.
//!
//! # What was explicitly not done
//!
//! **Drills were not exempted from the fold.** That was the tempting
//! shortcut and it would have silently restored the overclaim the basis
//! exists to prevent. They are *integrated*, which is a different thing:
//! their time is still counted, it is simply counted by the integrator.
//!
//! **`drill_summaries` was not used as a substitute.** `DrillToolpathSummary`
//! carries `feed_time_s` and `dwell_time_s` and its own doc says it "excludes
//! rapid … runtime accounting" — a cutting-only quantity. Sourcing a
//! `MachineModel` basis from it would claim modelling it does not have, which
//! is the vacuous-substitute trap this repo has hit before.
//!
//! # What this file pins, and what the viz side pins
//!
//! Here: the **simulator** populates `toolpath_runtimes` for a drill toolpath
//! that has no engagement summary, and the project total includes it. The
//! basis arithmetic — that one drill no longer drags a modelled project to
//! `CuttingOnly` — is pinned in
//! `rs_cam_viz/tests/cycle_time_basis_g_timeest.rs`, next to the G-TIMEEST
//! sentries it belongs with.
//!
//! # Known limit, stated rather than discovered later
//!
//! The integral is over **stored motion**, so a peck cycle's real R-plane and
//! re-entry moves are in it. G82 dwell is not motion and is not in it; it is
//! reported separately as `DrillToolpathSummary::dwell_time_s`, and no surface
//! currently adds the two.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::simulate::{
    KinematicsContext, SimGroupEntry, SimToolpathEntry, SimulationRequest, SimulationResult,
    run_simulation,
};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dexel_stock::StockCutDirection;
use rs_cam_core::drill::DrillCycle;
use rs_cam_core::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine_kinematics::MachineKinematics;
use rs_cam_core::material::Material;
use rs_cam_core::simulation_cut::{SimulationCutTrace, SimulationMetricOptions};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

const STOCK_X: f64 = 100.0;
const STOCK_Y: f64 = 80.0;
const STOCK_Z: f64 = 20.0;
const MILL: ToolpathId = ToolpathId(4);
const DRILL: ToolpathId = ToolpathId(7);

fn endmill_6mm() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.0, 25.0)),
        6.0,
        20.0,
        25.0,
        45.0,
        2,
        ToolMaterial::Carbide,
    )
}

fn stock_bbox() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(STOCK_X, STOCK_Y, STOCK_Z),
    }
}

/// An ordinary milling pass, which DOES produce an engagement summary.
fn mill_entry() -> SimToolpathEntry {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(20.0, 40.0, STOCK_Z + 5.0));
    tp.feed_to(P3::new(20.0, 40.0, STOCK_Z - 2.0), 800.0);
    tp.feed_to(P3::new(80.0, 40.0, STOCK_Z - 2.0), 800.0);
    tp.rapid_to(P3::new(80.0, 40.0, STOCK_Z + 5.0));

    SimToolpathEntry {
        id: MILL,
        name: "Groove".to_owned(),
        annotated: Arc::new(AnnotatedToolpath::new(tp)),
        tool: endmill_6mm(),
        flute_count: 2,
        tool_summary: "6mm Flat".to_owned(),
        semantic_trace: None,
        spindle_rpm: None,
        metrics_not_applicable: false,
        drill_op: None,
        operation_config_hash: 0,
    }
}

/// A drill toolpath: `metrics_not_applicable`, carries a `DrillOp`, and
/// therefore produces NO `toolpath_summaries` row. That absence is the
/// fixture's whole point, and `the_fixture_really_is_the_defects_shape`
/// asserts it rather than assuming it.
fn drill_entry() -> SimToolpathEntry {
    let mut tp = Toolpath::new();
    // Two holes, fed down and rapided out — real motion the integrator can
    // walk. 2 x 12 mm of fed descent at 300 mm/min is the quantity at issue.
    for (x, y) in [(30.0_f64, 20.0_f64), (70.0, 60.0)] {
        tp.rapid_to(P3::new(x, y, STOCK_Z + 5.0));
        tp.feed_to(P3::new(x, y, STOCK_Z - 12.0), 300.0);
        tp.rapid_to(P3::new(x, y, STOCK_Z + 5.0));
    }

    let drill_op = DrillOp {
        holes: vec![
            DrillHole {
                xy: [30.0, 20.0],
                top_z: STOCK_Z,
                bottom_z: STOCK_Z - 12.0,
            },
            DrillHole {
                xy: [70.0, 60.0],
                top_z: STOCK_Z,
                bottom_z: STOCK_Z - 12.0,
            },
        ],
        hole_source: HoleSource::ModelDerived,
        tool_profile: ToolProfile::Flat,
        tool_diameter_mm: 6.0,
        cycle: DrillCycle::Simple,
        feed_rate_mm_min: 300.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        material: Material::default(),
        retract_z_mm: STOCK_Z + 5.0,
    };

    SimToolpathEntry {
        id: DRILL,
        name: "Holes".to_owned(),
        annotated: Arc::new(AnnotatedToolpath::new(tp)),
        tool: endmill_6mm(),
        flute_count: 2,
        tool_summary: "6mm Flat".to_owned(),
        semantic_trace: None,
        spindle_rpm: None,
        metrics_not_applicable: true,
        drill_op: Some(Arc::new(drill_op)),
        operation_config_hash: 0,
    }
}

fn run(with_kinematics: bool) -> SimulationResult {
    let request = SimulationRequest {
        groups: vec![SimGroupEntry {
            toolpaths: vec![mill_entry(), drill_entry()],
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: None,
        }],
        stock_bbox: stock_bbox(),
        stock_top_z: STOCK_Z,
        resolution: 1.0,
        metric_options: SimulationMetricOptions {
            enabled: true,
            capture_arc_engagement: false,
        },
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: with_kinematics.then(|| KinematicsContext {
            kinematics: MachineKinematics::generic_wood_router(),
            max_feed_mm_min: 5000.0,
            use_predicted_feed_in_gates: false,
        }),
    };
    let cancel = AtomicBool::new(false);
    run_simulation(&request, &cancel).expect("simulation completes")
}

fn trace(result: &SimulationResult) -> &SimulationCutTrace {
    result
        .cut_trace
        .as_deref()
        .expect("metrics are on, so a cut trace exists")
}

/// **Non-vacuity, first.** Everything below is a claim about a toolpath with
/// no engagement summary. If the simulator ever starts giving drills one, the
/// rest of this file stops testing the defect and starts testing nothing — so
/// the precondition is asserted, not assumed.
#[test]
fn the_fixture_really_is_the_defects_shape() {
    let result = run(true);
    let t = trace(&result);

    assert!(
        t.toolpath_summaries.iter().any(|s| s.toolpath_id == MILL),
        "the milling pass must produce an engagement summary, or the \
         comparison below has nothing to compare against"
    );
    assert!(
        !t.toolpath_summaries.iter().any(|s| s.toolpath_id == DRILL),
        "the drill must have NO engagement summary — that absence is the \
         defect's mechanism. If this fires, drills now produce summaries and \
         this whole file needs rewriting rather than relaxing."
    );
    assert!(
        t.drill_summaries.iter().any(|d| d.toolpath_id == DRILL),
        "the drill must publish a drill_summaries row instead"
    );
}

/// The fix: the integrator's answer is published for the drill too.
#[test]
fn the_integrator_publishes_a_runtime_for_the_drill_toolpath() {
    let result = run(true);
    let t = trace(&result);

    let drill_rt = t
        .toolpath_runtimes
        .iter()
        .find(|r| r.toolpath_id == DRILL)
        .expect(
            "the integrator walks every toolpath in the request; before \
             G-DRILLTIME it computed this and threw it away",
        );
    let mill_rt = t
        .toolpath_runtimes
        .iter()
        .find(|r| r.toolpath_id == MILL)
        .expect("and the milling pass, unchanged");

    // Non-vacuity: a zero-second drill would satisfy "is present" while
    // proving nothing. 24 mm of fed descent at 300 mm/min is ~4.8 s of
    // cutting alone, before rapids and before accel ramps.
    assert!(
        drill_rt.breakdown.total_s > 1.0,
        "the drill's integrated runtime must be a real quantity, got {}",
        drill_rt.breakdown.total_s
    );
    assert!(mill_rt.breakdown.total_s > 0.0);
}

/// And the project total counts it. This is the number the operator reads.
#[test]
fn the_project_total_includes_the_drill() {
    let result = run(true);
    let t = trace(&result);

    let sum: f64 = t
        .toolpath_runtimes
        .iter()
        .map(|r| r.breakdown.total_s)
        .sum();
    assert!(
        (t.summary.total_runtime_s - sum).abs() < 1e-6,
        "project total {} must equal the sum of every integrated toolpath {}",
        t.summary.total_runtime_s,
        sum
    );

    // The control that gives the assertion above its teeth: dropping the
    // drill's contribution must change the answer. If it did not, the total
    // could be summing the milling pass alone and still "match".
    let drill_s = t
        .toolpath_runtimes
        .iter()
        .find(|r| r.toolpath_id == DRILL)
        .expect("drill runtime")
        .breakdown
        .total_s;
    assert!(
        drill_s > 0.0 && (t.summary.total_runtime_s - (sum - drill_s)).abs() > 1e-6,
        "the drill must materially contribute to the total"
    );
}

/// With no kinematics on the machine, nothing is integrated and the slot stays
/// empty — the pre-F-034 path is untouched, and the basis logic downstream is
/// still free to say `SimulatedNoAccel`.
#[test]
fn no_kinematics_means_no_integrated_runtimes() {
    let result = run(false);
    let t = trace(&result);
    assert!(
        t.toolpath_runtimes.is_empty(),
        "the integrator did not run, so it must not claim it did"
    );
    assert!(
        t.summary.runtime_by_intent.is_none(),
        "and the intent breakdown is likewise absent"
    );
}
