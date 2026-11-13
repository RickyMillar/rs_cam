//! **Feeds contract P17: the cut force is the deflection force, and the
//! budget line exists only under the force-budget rule.**
//!
//! # The claim
//!
//! 1. One law. At every sample of the force card's population, the peak
//!    force (`cut_force::sample_peak_force_n`) is the force behind the
//!    deflection gate: `tip_deflection_from_engagement` at the same
//!    engagement (`deflection::sample_engagement`) equals the tip
//!    compliance (`deflection::tip_compliance_mm_per_n`) times that force,
//!    to 1e-9 mm. The card's peak is the reading's peak
//!    (`ToolpathLoadVerdict::cut_force`), at the same sample.
//! 2. Lead ruling 3 (2026-11-06). `cut_force::budget_line` is `Some` only
//!    when `machine.rigidity.depth_cap_of(kind.body_depth_limit())` is
//!    `Some(DepthCapRule { .. })`: on a Pocket (a roughing Pocket
//!    family) and on an Adaptive (P29-4, ruling R1: before P29-4 the
//!    Factor rule gave `None`), `None` on a Scallop finish pass. The card's
//!    state and bound source follow the line.
//! 3. MCP parity. The verdict serialises `cut_force` with `peak_n` and
//!    `budget_n`, so `get_tool_load_report` carries the reading.
//! 4. The refusals: a material with no force line gives
//!    `NotMeasured(Unmodeled(MaterialUnvalidated))`; a drill gives no card.
//!
//! Non-vacuity: the population counts every one of the three hand-built
//! samples, and the peak force is positive.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::feeds::predict::tip_deflection_from_engagement;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::{DepthCapRule, MachineProfile};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::cut_force::{budget_line, cut_points, sample_peak_force_n};
use rs_cam_core::tool_load::deflection::{
    deflection_feed_per_tooth_mm, sample_engagement, tip_compliance_mm_per_n,
};
use rs_cam_core::tool_load::distribution::gate_population;
use rs_cam_core::tool_load::verdict::{BoundSource, LoadState, UnmodeledReason};
use rs_cam_core::tool_load::{
    DistributionMetric, DistributionOutcome, GateEnv, NotMeasuredReason, ToleranceBands,
    ToolpathLoadContext, evaluate_toolpath, metric_distribution,
};

const TP: ToolpathId = ToolpathId::from_raw(3);
const RPM: u32 = 18_000;
const FLUTES: u32 = 2;

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.0, 20.0)),
        6.0,
        40.0,
        FLUTES,
        ToolMaterial::Carbide,
    )
}

fn maple() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

/// One steady-state cutting sample: a depth, an arc and a feed.
fn sample(idx: usize, ap_mm: f64, arc_rad: f64, feed: f64) -> SimulationCutSample {
    SimulationCutSample {
        toolpath_id: TP,
        move_index: idx,
        sample_index: idx,
        position: [0.0, 0.0, -ap_mm],
        cumulative_time_s: 0.1 * idx as f64,
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: feed,
        spindle_rpm: RPM,
        flute_count: FLUTES,
        // The legacy wire depth is set APART from the engaged depth: the
        // force must read `axial_engagement_mm` (the deflection gate's
        // engagement), never `Engagement::from_sample` (`axial_doc_mm`).
        axial_doc_mm: ap_mm * 0.5,
        axial_engagement_mm: ap_mm,
        arc_engagement_radians: Some(arc_rad),
        chipload_mm_per_tooth: feed / (f64::from(RPM) * f64::from(FLUTES)),
        effective_chip_thickness_mm: Some(feed / (f64::from(RPM) * f64::from(FLUTES))),
        engagement: Engagement::with_radial_woc(0.5),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        ..SimulationCutSample::test_fixture()
    }
}

/// Three samples: a light cut, a half-immersion cut and a full slot. The
/// full slot at the largest depth carries the peak force.
fn trace() -> SimulationCutTrace {
    let samples = vec![
        sample(0, 1.0, 0.8, 1200.0),
        sample(1, 2.0, std::f64::consts::FRAC_PI_2, 1500.0),
        sample(2, 3.0, std::f64::consts::PI, 1500.0),
    ];
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
            average_engagement: 0.5,
            peak_chipload_mm_per_tooth: 0.05,
            peak_axial_doc_mm: 3.0,
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

fn ctx<'a>(
    tool: &'a ToolDefinition,
    material: &'a Material,
    op: OperationType,
) -> ToolpathLoadContext<'a> {
    ToolpathLoadContext::of_operation(
        TP,
        &rs_cam_core::compute::catalog::OperationConfig::new_default(op),
        tool,
        material,
        rs_cam_core::machine::SteadyFeedMmMin::op_feed_only(1500.0),
        None,
        rs_cam_core::limits::CapTable::empty(),
        None,
    )
}

/// Claim 1: one law, at every sample.
#[test]
fn the_force_card_peak_is_the_force_behind_the_deflection_gate() {
    let tool = tool();
    let material = maple();
    let machine = MachineProfile::shapeoko_makita();
    let trace = trace();
    let tolerance = ToleranceBands::default();
    let ctx = ctx(&tool, &material, OperationType::Pocket);
    let env = GateEnv {
        sim_trace: Some(&trace),
        machine: Some(&machine),
        tolerance: &tolerance,
    };

    let points = cut_points(&ctx, &env);
    assert_eq!(points.len(), 3, "every hand-built sample is a cut point");
    for p in &points {
        let s = &trace.samples[p.sample_index];
        let fz = deflection_feed_per_tooth_mm(s, &tool, &trace.predicted_feeds);
        let force = sample_peak_force_n(&tool, &material, s, fz).expect("a force");
        assert_eq!(
            p.force_n,
            Some(force),
            "the point carries the sample's force"
        );
        assert!(force > 0.0, "a cutting sample has a positive force");
        assert!((p.ap_mm - s.axial_engagement_mm).abs() < 1e-12);
        let engagement = sample_engagement(&tool, s).expect("an engagement");
        assert!((p.ae_mm - engagement.ae_mm()).abs() < 1e-12);
        // deflection = compliance x force, the one law.
        let deflection = tip_deflection_from_engagement(&tool, &material, &engagement, fz)
            .expect("a deflection");
        let compliance = tip_compliance_mm_per_n(&tool, s.axial_engagement_mm).expect("compliance");
        assert!(
            (deflection - compliance * force).abs() < 1e-9,
            "sample {}: deflection {deflection} != compliance {compliance} x force {force}",
            p.sample_index
        );
    }

    let population = gate_population(DistributionMetric::CutForce, &ctx, &env);
    assert_eq!(
        population.len(),
        3,
        "the card bins every point with a force"
    );
    let card_peak = population
        .iter()
        .max_by(|a, b| a.value.total_cmp(&b.value))
        .expect("a peak");
    assert_eq!(
        card_peak.sample_index, 2,
        "the full slot at 3 mm is the peak"
    );

    let verdict = evaluate_toolpath(&ctx, Some(&trace), Some(&machine), &tolerance);
    let reading = verdict.cut_force.as_ref().expect("a cut-force reading");
    assert_eq!(reading.population.contributing, 3);
    assert_eq!(reading.population.offered, 3);
    assert_eq!(reading.peak_sample_index, Some(card_peak.sample_index));
    assert!(
        (reading.peak_n - card_peak.value).abs() < 1e-12,
        "the reading's peak is the card's peak"
    );
    let s = &trace.samples[2];
    let fz = deflection_feed_per_tooth_mm(s, &tool, &trace.predicted_feeds);
    assert_eq!(
        Some(reading.peak_n),
        sample_peak_force_n(&tool, &material, s, fz),
        "the peak is `sample_peak_force_n` at the peak sample"
    );
}

/// Claim 2: the line exists exactly under the force-budget rule.
#[test]
fn the_budget_line_follows_the_depth_cap_of_the_body_cut() {
    let tool = tool();
    let material = maple();
    let machine = MachineProfile::shapeoko_makita();
    let trace = trace();
    let tolerance = ToleranceBands::default();
    let env = GateEnv {
        sim_trace: Some(&trace),
        machine: Some(&machine),
        tolerance: &tolerance,
    };

    // The rule, read from the machine, so the test types no number.
    let Some(DepthCapRule { budget_n }) = machine
        .rigidity
        .depth_cap_of(OperationType::Pocket.body_depth_limit())
    else {
        panic!("a roughing Pocket is under the force budget");
    };
    assert_eq!(
        machine
            .rigidity
            .depth_cap_of(OperationType::Adaptive.body_depth_limit()),
        Some(DepthCapRule { budget_n }),
        "P29-4: the Adaptive family is under the force budget"
    );

    let pocket_ctx = ctx(&tool, &material, OperationType::Pocket);
    assert_eq!(budget_line(&pocket_ctx, &env), Some(budget_n));
    let adaptive_ctx = ctx(&tool, &material, OperationType::Adaptive);
    assert_eq!(
        budget_line(&adaptive_ctx, &env),
        Some(budget_n),
        "P29-4: the force-budget rule gives the line on an Adaptive"
    );
    let finish_ctx = ctx(&tool, &material, OperationType::Scallop);
    assert_eq!(budget_line(&finish_ctx, &env), None, "finish pass: no line");
    let no_machine = GateEnv {
        sim_trace: Some(&trace),
        machine: None,
        tolerance: &tolerance,
    };
    assert_eq!(
        budget_line(&pocket_ctx, &no_machine),
        None,
        "no machine: no line"
    );

    // The card: a state and a bound source under the line, a reading without.
    let pocket_verdict = evaluate_toolpath(&pocket_ctx, Some(&trace), Some(&machine), &tolerance);
    assert_eq!(
        pocket_verdict.cut_force.as_ref().unwrap().budget_n,
        Some(budget_n)
    );
    let Some(DistributionOutcome::Measured(card)) = metric_distribution(
        DistributionMetric::CutForce,
        &pocket_verdict,
        &pocket_ctx,
        &env,
    ) else {
        panic!("a Pocket has a measured force card");
    };
    assert!(
        matches!(card.state, Some(LoadState::Within | LoadState::Exceeds)),
        "under the line the card has a state: {:?}",
        card.state
    );
    assert_eq!(card.histogram.ceiling, Some(budget_n));
    let Some(BoundSource::MachineForceBudget {
        budget_n: source_budget,
        depth_mm,
        ..
    }) = card.bound_source
    else {
        panic!(
            "the bound source is the machine force budget: {:?}",
            card.bound_source
        );
    };
    assert_eq!(source_budget, budget_n);
    assert!(
        (depth_mm - 3.0).abs() < 1e-12,
        "the source names the peak sample's depth"
    );
    assert!(
        !card.bound_source.as_ref().unwrap().gates_export(),
        "the force card is advisory"
    );
    assert_eq!(card.population.contributing, 3);

    // P29-4: the Adaptive card reads the line as the Pocket card does.
    let adaptive_verdict =
        evaluate_toolpath(&adaptive_ctx, Some(&trace), Some(&machine), &tolerance);
    assert_eq!(
        adaptive_verdict.cut_force.as_ref().unwrap().budget_n,
        Some(budget_n)
    );
    let Some(DistributionOutcome::Measured(card)) = metric_distribution(
        DistributionMetric::CutForce,
        &adaptive_verdict,
        &adaptive_ctx,
        &env,
    ) else {
        panic!("an Adaptive has a measured force card");
    };
    assert!(
        matches!(card.state, Some(LoadState::Within | LoadState::Exceeds)),
        "under the line the Adaptive card has a state: {:?}",
        card.state
    );
    assert_eq!(card.histogram.ceiling, Some(budget_n));
    assert!(
        matches!(
            card.bound_source,
            Some(BoundSource::MachineForceBudget { budget_n: b, .. }) if b == budget_n
        ),
        "the Adaptive bound source is the machine force budget: {:?}",
        card.bound_source
    );

    // No line (a finish pass): a reading, not a verdict. Before P29-4 the
    // Adaptive arm held this claim.
    let finish_verdict = evaluate_toolpath(&finish_ctx, Some(&trace), Some(&machine), &tolerance);
    let reading = finish_verdict.cut_force.as_ref().unwrap();
    assert_eq!(reading.budget_n, None);
    assert!(
        reading.peak_n > 0.0,
        "the force is still read without a line"
    );
    assert!(reading.rule.contains("no force limit"), "{}", reading.rule);
    let Some(DistributionOutcome::Measured(card)) = metric_distribution(
        DistributionMetric::CutForce,
        &finish_verdict,
        &finish_ctx,
        &env,
    ) else {
        panic!("a finish pass has a measured force card");
    };
    assert_eq!(card.state, None, "no line: a reading, not a verdict");
    assert_eq!(card.bound_source, None);
    assert_eq!(card.histogram.ceiling, None);
}

/// Claim 3: the MCP wire carries the reading.
#[test]
fn the_verdict_serialises_the_peak_and_the_budget() {
    let tool = tool();
    let material = maple();
    let machine = MachineProfile::shapeoko_makita();
    let trace = trace();
    let tolerance = ToleranceBands::default();
    let ctx = ctx(&tool, &material, OperationType::Pocket);
    let verdict = evaluate_toolpath(&ctx, Some(&trace), Some(&machine), &tolerance);
    let json = serde_json::to_value(&verdict).expect("serialise");
    let reading = json.get("cut_force").expect("the `cut_force` key");
    assert!(
        reading
            .get("peak_n")
            .and_then(serde_json::Value::as_f64)
            .unwrap()
            > 0.0
    );
    assert!(
        reading
            .get("budget_n")
            .and_then(serde_json::Value::as_f64)
            .unwrap()
            > 0.0
    );
    assert!(
        reading
            .get("rule")
            .and_then(serde_json::Value::as_str)
            .is_some()
    );

    // The reading joins no criterion: the row set is unchanged.
    assert!(
        verdict
            .criteria()
            .iter()
            .all(|row| row.unit != "N" || row.bound.is_none()),
        "no criterion row judges newtons"
    );

    // No trace: no reading, and no wire key.
    let dry = evaluate_toolpath(&ctx, None, Some(&machine), &tolerance);
    assert!(dry.cut_force.is_none());
    let json = serde_json::to_value(&dry).expect("serialise");
    assert!(
        json.get("cut_force").is_none(),
        "an absent reading has no key"
    );
}

/// Claim 4: the refusals.
#[test]
fn no_force_line_and_a_drill_refuse_as_the_deflection_gate_does() {
    let tool = tool();
    let machine = MachineProfile::shapeoko_makita();
    let trace = trace();
    let tolerance = ToleranceBands::default();
    let env = GateEnv {
        sim_trace: Some(&trace),
        machine: Some(&machine),
        tolerance: &tolerance,
    };

    let custom = Material::Custom {
        name: "Mystery".to_owned(),
        feed_scale_factor: 1.0,
    };
    let ctx_custom = ctx(&tool, &custom, OperationType::Pocket);
    let verdict = evaluate_toolpath(&ctx_custom, Some(&trace), Some(&machine), &tolerance);
    assert!(verdict.cut_force.is_none(), "no force line: no reading");
    assert_eq!(
        metric_distribution(DistributionMetric::CutForce, &verdict, &ctx_custom, &env),
        Some(DistributionOutcome::NotMeasured(
            NotMeasuredReason::Unmodeled(UnmodeledReason::MaterialUnvalidated)
        ))
    );

    let maple = maple();
    let ctx_drill = ctx(&tool, &maple, OperationType::Drill);
    let verdict = evaluate_toolpath(&ctx_drill, Some(&trace), Some(&machine), &tolerance);
    assert!(
        verdict.cut_force.is_none(),
        "a drill has no continuous engagement"
    );
    assert_eq!(
        metric_distribution(DistributionMetric::CutForce, &verdict, &ctx_drill, &env),
        None,
        "a drill has no force card"
    );
}
