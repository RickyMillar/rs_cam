//! **Feeds contract P17: a rough pass may deflect up to its radial stock to
//! leave; every other pass keeps the finish target.**
//!
//! # The claim
//!
//! The deflection gate's `Exceeds` limit is `deflection::exceeds_limit`
//! (the derivation is in its doc): the wall error equals the tip
//! deflection, and a rough pass leaves `s` for the finish pass to clean up.
//!
//! 1. A Bore with `finish_allowance` 0.5 mm (its doc: "Radial stock the
//!    rough helix leaves on the wall") judges at 0.5 mm, with
//!    `BoundSource::StockToLeave { mm: 0.5 }` (setting "operation", gates
//!    an export), and the detail names the leave.
//! 2. The same Bore with `finish_allowance` 0.0 judges at
//!    `EXCEEDS_BOUND_MM` and the detail names the zero leave.
//! 3. A Pocket (no radial leave field) judges at `EXCEEDS_BOUND_MM` and
//!    the detail says no field.
//! 4. A Scallop (a finish pass) judges at `EXCEEDS_BOUND_MM` and the
//!    detail names the finish pass.
//!
//! Every context comes through the F070 door
//! `ProjectSession::load_context_parts`, so the test holds the read of the
//! op field (`OperationConfig::radial_stock_to_leave_mm`) too: the context
//! carries it as the body fact `Leave` (P29-6, `CutFacts::leave_after_mm`).
//!
//! Non-vacuity: the one sample contributes, and its peak deflection is
//! above the finish target, so the limit decides the verdict.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::session::{polygon_model, single_op_session, square_polygon, stock_under};
use rs_cam_core::compute::ToolType;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::{BoreConfig, PocketConfig, ScallopConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolMaterial};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::limits::Leave;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool_load::deflection::{EXCEEDS_BOUND_MM, evaluate};
use rs_cam_core::tool_load::verdict::{
    BoundSource, Confidence, DeflectionBasis, DeflectionVerdict, LoadState,
};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands};

const RPM: u32 = 18_000;
const FLUTES: u32 = 2;
/// A long, thin tool: 3 mm at 42 mm stickout (L/D 14), the shank the tool
/// diameter (no step). The hand-built fixture of `deflection::tests`
/// Exceeds 200 um at 36 mm with a full-length flute; the session cutter
/// has a shorter flute, so it is stiffer, and 42 mm puts the full slot at
/// 5000 mm/min past 200 um and under a 0.5 mm leave.
const TOOL_D: f64 = 3.0;
const STICKOUT: f64 = 42.0;

fn tool_config() -> ToolConfig {
    let mut tool = ToolConfig::builder(ToolId::from_raw(0), ToolType::EndMill)
        .diameter(TOOL_D)
        .flutes(FLUTES)
        .tool_material(ToolMaterial::Carbide)
        .build();
    tool.set_stickout(ToolFact::Stated(STICKOUT));
    // The hand-built fixture has no shank step: the shank is the tool.
    tool.set_shank_diameter(ToolFact::Stated(TOOL_D));
    tool
}

fn session_with(op: OperationConfig) -> ProjectSession {
    let mut stock = stock_under(20.0, 18.0);
    stock.material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    single_op_session(
        stock,
        tool_config(),
        polygon_model(vec![square_polygon(20.0)], "p17_leave"),
        "p17",
        op,
    )
}

fn bore(finish_allowance: f64) -> OperationConfig {
    OperationConfig::Bore(BoreConfig {
        finish_allowance,
        feed_rate: Some(5000.0),
        ..BoreConfig::default()
    })
}

fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        feed_rate: Some(5000.0),
        ..PocketConfig::default()
    })
}

fn scallop() -> OperationConfig {
    OperationConfig::Scallop(ScallopConfig {
        feed_rate: Some(5000.0),
        ..ScallopConfig::default()
    })
}

/// One full-slot steady-state sample at 1.5 mm depth and 5000 mm/min.
fn trace(toolpath_id: ToolpathId) -> SimulationCutTrace {
    let feed = 5000.0;
    let samples = vec![SimulationCutSample {
        toolpath_id,
        move_index: 0,
        sample_index: 0,
        position: [0.0, 0.0, -1.5],
        cumulative_time_s: 0.0,
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: feed,
        spindle_rpm: RPM,
        flute_count: FLUTES,
        axial_doc_mm: 1.5,
        axial_engagement_mm: 1.5,
        arc_engagement_radians: Some(std::f64::consts::PI),
        chipload_mm_per_tooth: feed / (f64::from(RPM) * f64::from(FLUTES)),
        effective_chip_thickness_mm: Some(feed / (f64::from(RPM) * f64::from(FLUTES))),
        engagement: Engagement::with_radial_woc(1.0),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        ..SimulationCutSample::test_fixture()
    }];
    SimulationCutTrace {
        sample_step_mm: 1.0,
        summary: SimulationCutSummary {
            sample_count: samples.len(),
            toolpath_count: 1,
            issue_count: 0,
            hotspot_count: 0,
            total_runtime_s: 1.0,
            cutting_runtime_s: 1.0,
            rapid_runtime_s: 0.0,
            air_cut_time_s: 0.0,
            low_engagement_time_s: 0.0,
            average_engagement: 1.0,
            peak_chipload_mm_per_tooth: 0.05,
            peak_axial_doc_mm: 1.5,
            peak_plunge_descent_mm: 0.0,
            total_removed_volume_est_mm3: 1.0,
            average_mrr_mm3_s: 1.0,
            per_kinematics: std::collections::BTreeMap::new(),
            runtime_by_intent: None,
        },
        samples,
        ..SimulationCutTrace::test_fixture()
    }
}

/// The deflection verdict of `op`, through the F070 door.
fn judge(op: OperationConfig) -> (DeflectionVerdict, Leave) {
    let session = session_with(op);
    let parts = session
        .load_context_parts(0)
        .expect("the fixture op has a tool and a feed");
    let ctx = parts.ctx();
    let leave = ctx.body_facts().leave_after_mm;
    let trace = trace(ctx.toolpath_id());
    let tolerance = ToleranceBands::default();
    let env = GateEnv {
        sim_trace: Some(&trace),
        machine: None,
        tolerance: &tolerance,
    };
    (evaluate(&ctx, &env), leave)
}

/// The peak, the bounds, the detail and the row of a judged verdict.
fn parts_of(
    v: &DeflectionVerdict,
) -> (
    f64,
    f64,
    DeflectionBasis,
    String,
    LoadState,
    Option<f64>,
    Option<BoundSource>,
) {
    let status = v.as_criterion_status();
    let (peak, bounds, confidence) = match v {
        DeflectionVerdict::Within {
            peak_mm,
            bounds,
            confidence,
            ..
        }
        | DeflectionVerdict::Exceeds {
            peak_mm,
            bounds,
            confidence,
            ..
        } => (*peak_mm, bounds, confidence),
        DeflectionVerdict::Unmodeled { reason } => panic!("a judged verdict, got {reason:?}"),
    };
    assert_eq!(
        status.population.map(|p| p.contributing),
        Some(1),
        "non-vacuity: the one sample contributes"
    );
    let detail = match confidence {
        Confidence::Approximate(detail) => detail.clone(),
        Confidence::Validated => panic!("a peak above 50 um carries a detail"),
    };
    (
        peak,
        bounds.exceeds_mm,
        bounds.basis.clone(),
        detail,
        status.state,
        status.bound,
        status.bound_source,
    )
}

#[test]
fn a_bore_with_a_leave_judges_at_the_leave() {
    let (v, leave) = judge(bore(0.5));
    assert_eq!(
        leave,
        Leave::Mm(0.5),
        "the door reads the Bore's finish allowance"
    );
    let (peak, limit, basis, detail, state, bound, source) = parts_of(&v);
    assert!(
        peak > EXCEEDS_BOUND_MM,
        "non-vacuity: the fixture deflects past the finish target ({peak} mm)"
    );
    assert_eq!(limit, 0.5);
    assert_eq!(basis, DeflectionBasis::StockToLeave { mm: 0.5 });
    assert_eq!(bound, Some(0.5));
    assert_eq!(source, Some(BoundSource::StockToLeave { mm: 0.5 }));
    let source = source.unwrap();
    assert_eq!(source.setting(), "operation");
    assert!(
        source.gates_export(),
        "a stated bound gates as the finish target does"
    );
    assert!(source.clause().contains("0.50 mm"), "{}", source.clause());
    assert!(
        detail.contains("the radial stock to leave 0.50 mm"),
        "the detail names the leave: {detail}"
    );
    // The verdict flips: the same sample Exceeds the 200 um target (the
    // next test) and is Within the 0.5 mm leave.
    assert!(peak < 0.5, "the fixture stays under the leave ({peak} mm)");
    assert_eq!(state, LoadState::Within, "peak {peak} mm against 0.5 mm");
}

#[test]
fn a_bore_with_a_zero_leave_keeps_the_finish_target_and_says_so() {
    let (v, leave) = judge(bore(0.0));
    assert_eq!(leave, Leave::Zero);
    let (peak, limit, basis, detail, state, bound, source) = parts_of(&v);
    assert!(peak > EXCEEDS_BOUND_MM);
    assert_eq!(limit, EXCEEDS_BOUND_MM);
    assert_eq!(basis, DeflectionBasis::ZeroLeave);
    assert_eq!(bound, Some(EXCEEDS_BOUND_MM));
    assert_eq!(source, Some(BoundSource::DeflectionBudget));
    assert!(
        detail.contains("stock to leave 0: the finish target 200 um applies"),
        "{detail}"
    );
    assert_eq!(state, LoadState::Exceeds);
}

#[test]
fn a_pocket_has_no_leave_field_and_says_so() {
    let (v, leave) = judge(pocket());
    assert_eq!(leave, Leave::NoField, "a Pocket has no radial leave field");
    let (peak, limit, basis, detail, state, bound, source) = parts_of(&v);
    assert!(peak > EXCEEDS_BOUND_MM);
    assert_eq!(limit, EXCEEDS_BOUND_MM);
    assert_eq!(basis, DeflectionBasis::NoLeaveField);
    assert_eq!(bound, Some(EXCEEDS_BOUND_MM));
    assert_eq!(source, Some(BoundSource::DeflectionBudget));
    assert!(
        detail.contains("no stock-to-leave field on this op: the finish target 200 um applies"),
        "{detail}"
    );
    assert_eq!(state, LoadState::Exceeds);
}

#[test]
fn a_finish_pass_keeps_the_finish_target() {
    let (v, leave) = judge(scallop());
    assert_eq!(
        leave,
        Leave::FinishPass,
        "a Scallop's stock_to_leave is a finish leave, not a radial rough leave"
    );
    let (peak, limit, basis, detail, state, bound, source) = parts_of(&v);
    assert!(peak > EXCEEDS_BOUND_MM);
    assert_eq!(limit, EXCEEDS_BOUND_MM);
    assert_eq!(basis, DeflectionBasis::FinishTarget);
    assert_eq!(bound, Some(EXCEEDS_BOUND_MM));
    assert_eq!(source, Some(BoundSource::DeflectionBudget));
    assert!(
        detail.contains("finish pass: the finish target 200 um applies"),
        "{detail}"
    );
    assert_eq!(state, LoadState::Exceeds);
}

/// The accessor itself, for the record: Bore only; the finish ops' leave
/// is not a radial rough leave; the Adaptive3d leave is axial.
#[test]
fn only_a_bore_states_a_radial_leave_today() {
    assert_eq!(bore(0.7).radial_stock_to_leave_mm(), Some(0.7));
    for op_type in [
        OperationType::Pocket,
        OperationType::Adaptive,
        OperationType::Adaptive3d,
        OperationType::Scallop,
        OperationType::Pencil,
        OperationType::Profile,
    ] {
        let op = OperationConfig::new_default(op_type);
        assert_eq!(
            op.radial_stock_to_leave_mm(),
            None,
            "{op_type:?} states no radial leave"
        );
    }
}
