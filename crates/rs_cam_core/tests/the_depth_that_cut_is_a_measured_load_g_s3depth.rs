//! **S3 — the depth that cut is a measured load, and it says what set
//! its limit.**
//!
//! Depth of cut sets the depth on nearly every recipe this engine
//! ships, and before S3 the operator could not see it as a limit. The
//! cap lived in one place only: `feeds::suggest::invariants`, where
//! `clamp_dpp_to_rigidity` lowered the depth per pass and pushed a
//! `RoughingDepthClampedToRigidity` warning. After the clamp the number
//! disappeared.
//!
//! `PLAN.md` §11 settled the category question by STAGE. Before a
//! simulation the depth is a value the engine chose, so it stays a
//! rationale entry. After a simulation it is a MEASURED quantity with a
//! real population — the per-sample `axial_engagement_mm` varies with
//! entry ramps, curved surfaces and arcs — so it belongs beside the
//! other measured loads. That is Reading A, and this file pins it.
//!
//! **The row does not gate.** Its bound is the machine force-budget
//! depth (P13c; the Adaptive family since P29-4). The budget stands in
//! for an unmeasured frame, so the bound is
//! `BoundSource::MachineForceBudget`, whose `gates_export()` is false.
//! Arm (d) below is the end-to-end exercise of that path: an exceeded
//! depth row is REPORTED and the export still goes ahead, while an
//! exceeded power row beside it still warns. P29-4 deleted the rule of
//! thumb `factor x D` and its `BoundSource::RigidityRuleOfThumb`.
//!
//! **S3 adds no number.** Every bound here comes through the one shared
//! producer `force_budget::rigidity_depth_cap`, which the envelope calls
//! too. The two cannot disagree.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{EnvelopeInput, LimitKind, PickRequest, SafeEnvelope};
use rs_cam_core::feeds::force_budget::{RigidityCapOutcome, rigidity_depth_cap};
use rs_cam_core::feeds::suggest::{
    NoProject, OperationApplyInput, SuggestContext, apply_to_operation,
};
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::gcode::load_findings;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::{MachineProfile, RigidityDepthCap};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::ops::drill::DrillCycle;
use rs_cam_core::ops::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::{
    BoundSource, Confidence, CriterionKind, CriterionStatus, DepthVerdict, LoadState, PowerVerdict,
    SampleEvidence, ToolLoadReport, ToolpathLoadVerdict, UnmodeledReason,
};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext};

const TP: ToolpathId = ToolpathId::from_raw(0);
const SAMPLE_RPM: u32 = 18_000;

/// The cutter every measured arm uses. Ø6.35 flat, two flutes.
const DIAMETER_MM: f64 = 6.35;

/// The measured trace's depths, in mm, one per sample. They VARY, so
/// "peak" is a real maximum over a population and not a constant
/// restated. The maximum is 0.95 mm.
fn depths() -> Vec<f64> {
    (0..12).map(|i| 0.40 + 0.05 * i as f64).collect()
}

/// The deep hand-set trace of arm (d): every sample cuts past the roughing
/// cap on this tool and profile.
///
/// P13c (O1): the roughing Pocket cap is the force-budget depth at the
/// measured cut (19.3 mm on this softwood fixture), so the deep trace cuts
/// 1 mm past it, read from the one producer (it was 3.000 mm against the
/// 0.25 D cap).
fn deep_mm() -> f64 {
    cap_for(OperationType::Pocket).cap_mm() + 1.0
}

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(DIAMETER_MM, 20.0)),
        DIAMETER_MM,
        30.0,
        2,
        ToolMaterial::Carbide,
    )
}

/// One measured cutting sample at `axial_engagement_mm = depth`, not in
/// a transit span: a body-group sample (`locality::judged_group`).
fn cutting_sample(idx: usize, depth: f64) -> SimulationCutSample {
    let feed = 2000.0;
    SimulationCutSample {
        toolpath_id: TP,
        move_index: idx,
        sample_index: idx,
        position: [0.0, 0.0, -1.0],
        cumulative_time_s: 0.1 * idx as f64,
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: feed,
        spindle_rpm: SAMPLE_RPM,
        flute_count: 2,
        // `axial_doc_mm` is the LEGACY wire name and reads 0.0 on a
        // pure-vertical plunge; `axial_engagement_mm` is the axis the
        // engagement-reading gates consume. They differ here on
        // purpose, so an arm that reads the wrong one fails.
        axial_doc_mm: 0.0,
        axial_engagement_mm: depth,
        arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
        chipload_mm_per_tooth: feed / (f64::from(SAMPLE_RPM) * 2.0),
        effective_chip_thickness_mm: Some(feed / (f64::from(SAMPLE_RPM) * 2.0)),
        engagement: Engagement::with_radial_woc(0.5),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        in_transit_span: false,
        ..SimulationCutSample::test_fixture()
    }
}

fn trace_from(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
    let peak = samples
        .iter()
        .map(|s| s.axial_engagement_mm)
        .fold(0.0_f64, f64::max);
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
            peak_axial_doc_mm: peak,
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

fn measured_trace() -> SimulationCutTrace {
    trace_from(
        depths()
            .into_iter()
            .enumerate()
            .map(|(i, d)| cutting_sample(i, d))
            .collect(),
    )
}

fn deep_trace() -> SimulationCutTrace {
    let deep = deep_mm();
    trace_from((0..6).map(|i| cutting_sample(i, deep)).collect())
}

/// A trace whose only sample for this toolpath is NOT cutting. The gate
/// is offered one sample and contributes none: X-VAC.
fn air_trace() -> SimulationCutTrace {
    trace_from(vec![SimulationCutSample {
        is_cutting: false,
        axial_engagement_mm: 0.0,
        ..cutting_sample(0, 0.0)
    }])
}

fn machine() -> MachineProfile {
    MachineProfile::shapeoko_makita()
}

/// The peak the gate must report, computed here from the trace and
/// nothing else. This is the independent reading arm (a) compares
/// against.
fn peak_axial_engagement(trace: &SimulationCutTrace) -> f64 {
    trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == TP && s.is_cutting)
        .map(|s| s.axial_engagement_mm)
        .fold(0.0_f64, f64::max)
}

fn depth_verdict_for(op: OperationType, trace: &SimulationCutTrace) -> DepthVerdict {
    let tool = tool();
    let material = Material::default();
    let machine = machine();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext::of_operation(
        TP,
        &rs_cam_core::compute::catalog::OperationConfig::new_default(op),
        &tool,
        &material,
        rs_cam_core::machine::SteadyFeedMmMin::op_feed_only(2000.0),
        None,
        rs_cam_core::limits::CapTable::empty(),
        None,
    );
    let env = GateEnv {
        sim_trace: Some(trace),
        machine: Some(&machine),
        tolerance: &tolerance,
    };
    rs_cam_core::tool_load::depth::evaluate(&ctx, &env)
}

/// A full milling verdict through the shipped assembly site, so the
/// depth row this file reads is the one every consumer sees.
fn milling_verdict(op: OperationType, trace: &SimulationCutTrace) -> ToolpathLoadVerdict {
    let tool = tool();
    let material = Material::default();
    let machine = machine();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext::of_operation(
        TP,
        &rs_cam_core::compute::catalog::OperationConfig::new_default(op),
        &tool,
        &material,
        rs_cam_core::machine::SteadyFeedMmMin::op_feed_only(2000.0),
        None,
        rs_cam_core::limits::CapTable::empty(),
        None,
    );
    rs_cam_core::tool_load::evaluate_toolpath(&ctx, Some(trace), Some(&machine), &tolerance)
}

fn depth_row<'a>(criteria: &'a [CriterionStatus<'a>]) -> &'a CriterionStatus<'a> {
    let rows: Vec<&CriterionStatus<'_>> = criteria
        .iter()
        .filter(|s| s.kind == CriterionKind::DepthOfCut)
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "the criterion tier must carry exactly one depth-of-cut row, found {}",
        rows.len()
    );
    rows[0]
}

/// The cap the profile implies for one family at the measured trace's
/// peak cut, read back through the one shared producer
/// (`feeds::force_budget::rigidity_depth_cap`). P13c (O1): a roughing
/// Pocket reads the force budget at the judged sample's width (arc pi/2:
/// ae = r) and chip (feed / (rpm x Z)); P29-4: the Adaptive family reads
/// the same budget; P29-5 (ruling R3): a Trace slot too. The depth limit
/// is the op kind's (`OperationType::body_depth_limit`). Nothing in this
/// file multiplies a factor or solves a depth itself.
fn cap_for(op: OperationType) -> RigidityDepthCap {
    let m = machine();
    let ae = DIAMETER_MM / 2.0 * (1.0 - std::f64::consts::FRAC_PI_2.cos());
    let fz = 2000.0 / (f64::from(SAMPLE_RPM) * 2.0);
    match rigidity_depth_cap(
        &m,
        op.body_depth_limit(),
        &tool(),
        &Material::default(),
        ae,
        fz,
    ) {
        Some(RigidityCapOutcome::Cap(c)) => c,
        other => panic!("a cut with the force budget carries an axial cap: {other:?}"),
    }
}

/// The bound source the depth row carries for `cap`.
fn source_of(cap: RigidityDepthCap) -> BoundSource {
    match cap {
        RigidityDepthCap::ForceBudget {
            budget_n,
            ae_mm,
            fz_mm,
            depth_mm,
            ..
        } => BoundSource::MachineForceBudget {
            budget_n,
            ae_mm,
            fz_mm,
            depth_mm,
        },
    }
}

// ── (a) the row states the measured peak and the bound that judged it ─

#[test]
fn a_roughing_depth_row_states_the_measured_peak_and_its_bound() {
    let trace = measured_trace();
    let v = milling_verdict(OperationType::Pocket, &trace);
    let criteria = v.criteria();
    let row = depth_row(&criteria);

    let cap = cap_for(OperationType::Pocket);
    let cap_mm = cap.cap_mm();
    assert_eq!(
        row.bound,
        Some(cap_mm),
        "the depth row must read against the profile's own cap"
    );
    assert_eq!(
        row.bound_source,
        Some(source_of(cap)),
        "the provenance must carry the numbers the cap is built from"
    );
    assert_eq!(row.unit, "mm");
    assert_eq!(CriterionKind::DepthOfCut.label(), "depth of cut");

    let expected_peak = peak_axial_engagement(&trace);
    assert_eq!(
        row.display_peak,
        Some(expected_peak),
        "the row must report the maximum axial_engagement_mm over the \
         trace's cutting samples"
    );
    assert_eq!(
        row.state,
        LoadState::Within,
        "{expected_peak:.4} mm is under the {cap_mm:.4} mm cap, so the row is Within"
    );
}

/// The force-budget bound may never refuse an export, and the kind is NOT
/// a known absence: a depth row is a real gate with a real producer,
/// unlike the gantry-push row beside it.
#[test]
fn the_depth_bound_does_not_gate_and_the_kind_is_not_a_known_absence() {
    assert!(
        !CriterionKind::DepthOfCut.is_unmodeled_by_design(),
        "the depth row is produced by a real gate"
    );
    let source = source_of(cap_for(OperationType::Pocket));
    assert!(!source.gates_export());
    assert_eq!(source.setting(), "machine");
    assert!(
        source.clause().contains("unmeasured frame"),
        "the clause must say the budget stands in for an unmeasured frame: {}",
        source.clause()
    );
}

// ── (b) two capped families, and a finishing pass with no cap ────────

/// Re-blessed under feeds matrix R2 (2026-09-23). Before R2 a finishing
/// Trace read `doc_finishing_factor` (0.10 × Ø6.35 = 0.635 mm on this
/// profile). The operator ruled that a finishing or semi-finishing pass
/// gets no axial ceiling: no vendor publishes one for wood (EVIDENCE
/// 5.1-3, 5.1-12). The finishing arm now reports the measured peak with
/// no bound and no bound source, and the state is `Within`. P29-4
/// (ruling R1): the two roughing arms read one rule, the force budget.
/// P29-5 (ruling R3): the finishing arm is a one-sided Scallop; a
/// finishing Trace is a slot and reads the Pocket's cap at the same cut.
#[test]
fn each_family_reads_the_factor_its_own_pass_role_implies() {
    let trace = measured_trace();

    // Conventional roughing → the force budget (P13c, O1).
    let rough = cap_for(OperationType::Pocket);
    let rough_cap = rough.cap_mm();
    let rough_verdict = depth_verdict_for(OperationType::Pocket, &trace);
    let rough_status = rough_verdict.as_criterion_status();
    assert_eq!(rough_status.bound, Some(rough_cap));

    // A one-sided finish → no cap (R2). The peak is reported with no
    // bound. P29-5 (ruling R3): a Trace is a slot, so the one-sided finish
    // here is a Scallop.
    assert!(
        machine()
            .rigidity
            .depth_cap_of(OperationType::Scallop.body_depth_limit())
            .is_none(),
        "a one-sided finishing pass must carry no axial cap"
    );
    let finish = depth_verdict_for(OperationType::Scallop, &trace);
    assert!(
        matches!(finish, DepthVerdict::Reported { .. }),
        "a finishing pass must report its depth, got {finish:?}"
    );
    let finish_status = finish.as_criterion_status();
    assert_eq!(finish_status.bound, None);
    assert_eq!(finish_status.bound_source, None);
    assert_eq!(finish_status.state, LoadState::Within);
    assert_eq!(
        finish_status.display_peak,
        Some(peak_axial_engagement(&trace))
    );

    // A finishing Trace slot → the force budget (P29-5, ruling R3: "slot
    // is a slot!"). At the same measured cut it reads the Pocket's cap.
    let trace_slot = depth_verdict_for(OperationType::Trace, &trace);
    let trace_status = trace_slot.as_criterion_status();
    assert_eq!(
        trace_status.bound,
        Some(cap_for(OperationType::Trace).cap_mm())
    );
    assert_eq!(trace_status.bound, Some(rough_cap));

    // Adaptive → the force budget (P29-4, ruling R1). Before P29-4 it
    // read `adaptive_doc_factor`.
    let adaptive_bound = cap_for(OperationType::Adaptive);
    let adaptive_cap = adaptive_bound.cap_mm();
    let adaptive = depth_verdict_for(OperationType::Adaptive, &trace);
    let adaptive_status = adaptive.as_criterion_status();
    assert_eq!(adaptive_status.bound, Some(adaptive_cap));

    // One rule, read back off the profile (P29-4, ruling R1: the roughing
    // Pocket and the Adaptive family both read the force budget). At the
    // same cut the two caps are equal.
    let m = machine();
    let RigidityDepthCap::ForceBudget { budget_n, .. } = rough;
    let RigidityDepthCap::ForceBudget {
        budget_n: adaptive_budget_n,
        ..
    } = adaptive_bound;
    assert_eq!(budget_n, m.rigidity.force_budget_n);
    assert_eq!(adaptive_budget_n, m.rigidity.force_budget_n);
    assert_eq!(adaptive_bound, rough, "one rule at one cut gives one cap");
}

// ── (c) a drill cycle has no axial cap ───────────────────────────────

#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: an unexpected variant panics"
)]
fn a_drill_cycle_carries_the_same_not_applicable_clause_as_the_milling_gates() {
    let trace = measured_trace();
    let verdict = depth_verdict_for(OperationType::Drill, &trace);
    match &verdict {
        DepthVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(clause),
        } => assert_eq!(clause, "drill cycle — no continuous engagement"),
        other => panic!("a drill cycle must be NotApplicableForOp, got {other:?}"),
    }
    let status = verdict.as_criterion_status();
    assert_eq!(status.state, LoadState::Unmodeled);
    assert_eq!(status.bound, None, "an unmodelled row states no bound");
    assert_eq!(status.bound_source, None);

    // The full drill verdict keeps the "doesn't apply" partition: the
    // depth row must not push a drill toolpath into `fully_unmodeled`.
    let v = drill_verdict();
    assert!(
        !v.any_unmodeled(),
        "a drill cycle with drill gates is not missing evidence"
    );
}

fn drill_verdict() -> ToolpathLoadVerdict {
    let d = DrillOp {
        holes: vec![DrillHole {
            xy: [0.0, 0.0],
            top_z: 0.0,
            bottom_z: -12.0,
            retract_z: 5.0,
        }],
        hole_source: HoleSource::ModelDerived,
        tool_profile: ToolProfile::StandardTwist,
        tool_diameter_mm: 6.0,
        cycle: DrillCycle::Peck(3.0),
        feed_rate_mm_min: 300.0,
        spindle_rpm: 6_000,
        flute_count: 2,
        material: Material::default(),
    };
    let tool = tool();
    let material = Material::default();
    let machine = machine();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext::of_operation(
        TP,
        &rs_cam_core::compute::catalog::OperationConfig::new_default(OperationType::Drill),
        &tool,
        &material,
        rs_cam_core::machine::SteadyFeedMmMin::op_feed_only(300.0),
        None,
        rs_cam_core::limits::CapTable::empty(),
        Some(&d),
    );
    rs_cam_core::tool_load::evaluate_toolpath(&ctx, None, Some(&machine), &tolerance)
}

// ── (d) the non-gating path, end to end ──────────────────────────────

/// The end-to-end producer of `BoundSource::MachineForceBudget`. An
/// operator typed a depth above the cap; no Suggest run lowered it. The
/// row exceeds, `exceeded_criteria()` lists it, and the export goes
/// ahead with a note.
#[test]
fn a_hand_set_depth_above_the_cap_exceeds_but_does_not_refuse_the_export() {
    let trace = deep_trace();
    let v = milling_verdict(OperationType::Pocket, &trace);
    let criteria = v.criteria();
    let row = depth_row(&criteria);
    let cap_mm = cap_for(OperationType::Pocket).cap_mm();

    assert_eq!(
        row.state,
        LoadState::Exceeds,
        "a {:.3} mm cut against a {cap_mm:.4} mm cap exceeds",
        deep_mm()
    );
    assert!(
        row.display_peak.unwrap() > row.bound.unwrap(),
        "the exceedance must be a real one: peak {:?} vs bound {:?}",
        row.display_peak,
        row.bound
    );
    assert!(
        !row.warns_on_export(),
        "a force-budget bound may not warn on an export"
    );
    assert!(
        v.exceeded_criteria()
            .iter()
            .any(|e| e.kind == CriterionKind::DepthOfCut),
        "exceeded_criteria() must still LIST the depth row"
    );

    let report = ToolLoadReport {
        per_toolpath: vec![v],
    };
    let found = load_findings(&report);
    assert!(
        !found
            .rows
            .iter()
            .any(|row| row.id == rs_cam_core::diagnostics::ids::LOAD_DEPTH_EXCEEDS),
        "a depth exceedance alone must not warn: {found:?}"
    );
    let note = found
        .outcome
        .advisory_note()
        .expect("the export gate must still REPORT the exceedance");
    assert!(
        note.contains("depth of cut"),
        "the note must name the depth row: {note}"
    );
    // P13c (O1): the roughing Pocket bound is the force budget, which
    // stands in for an unmeasured frame.
    assert!(
        note.contains("unmeasured frame"),
        "the note must say why the bound does not gate: {note}"
    );
}

/// The same report with a power exceedance beside it. Power's bound is
/// the machine power curve, which DOES gate, so the export warns on it
/// (K6: no refusal), and the advisory names the depth row, so the
/// operator sees the whole picture.
#[test]
fn a_power_exceedance_beside_the_depth_row_warns_and_names_both() {
    let trace = deep_trace();
    let mut v = milling_verdict(OperationType::Pocket, &trace);
    v.power = PowerVerdict::Exceeds {
        peak_kw: 2.0,
        available_kw: 1.0,
        evidence: SampleEvidence::at(0),
        confidence: Confidence::Validated,
        bound_source: Some(BoundSource::MachinePowerCurve {
            rpm: f64::from(SAMPLE_RPM),
        }),
    };
    let report = ToolLoadReport {
        per_toolpath: vec![v],
    };
    let found = load_findings(&report);
    let power = found
        .rows
        .iter()
        .find(|row| row.id == rs_cam_core::diagnostics::ids::LOAD_POWER_EXCEEDS)
        .expect("a power exceedance must warn");
    assert!(
        power.detail.contains("spindle power"),
        "the warning must name what gated: {}",
        power.detail
    );
    let note = found
        .outcome
        .advisory_note()
        .expect("the depth row is an advisory");
    assert!(
        note.contains("depth of cut"),
        "the advisory must name what exceeded without gating: {note}"
    );
}

// ── (e) a trace with no cutting samples is vacuous ───────────────────

#[test]
fn a_trace_with_no_cutting_samples_is_vacuous_not_a_green_zero() {
    let trace = air_trace();
    let verdict = depth_verdict_for(OperationType::Pocket, &trace);
    let status = verdict.as_criterion_status();
    assert_eq!(
        status.state,
        LoadState::Within,
        "X-VAC is report-tier: a vacuous verdict stays Within"
    );
    assert!(
        status.is_vacuous(),
        "and it must SAY it rests on nothing: population {:?}",
        status.population
    );
    let population = status.population.expect("the gate states its population");
    assert_eq!(population.contributing, 0);
    assert!(
        population.offered > 0,
        "the gate was offered a sample and filtered it out"
    );
    assert!(
        status.vacuity_clause().contains("VACUOUS"),
        "the shared clause must word it: {}",
        status.vacuity_clause()
    );
}

// ── (f) non-vacuity ──────────────────────────────────────────────────

/// Arm (a)'s toolpath must be a MODELLED `Within` with a positive peak.
/// Without this the file passes on a gate that refuses everything.
#[test]
fn the_measured_arm_is_a_modelled_within_with_a_positive_peak() {
    let trace = measured_trace();
    let v = milling_verdict(OperationType::Pocket, &trace);
    let criteria = v.criteria();
    let row = depth_row(&criteria);
    assert_eq!(row.state, LoadState::Within);
    assert!(
        row.display_peak.unwrap() > 0.0,
        "the measured peak must be positive, got {:?}",
        row.display_peak
    );
    assert!(
        !row.is_vacuous(),
        "arm (a)'s population must not be empty: {:?}",
        row.population
    );
    let population = row.population.expect("the gate states its population");
    assert_eq!(
        population.contributing,
        depths().len(),
        "every cutting sample of the measured trace must reach the comparison"
    );
    assert!(
        !row.bound_clause().is_empty() && row.bound_clause().contains("limit"),
        "the row must be able to word its own bound: {}",
        row.bound_clause()
    );
}

/// The peak is a MAXIMUM over a varying population, not a constant. If
/// every sample cut the same depth, arm (a) could not tell a maximum
/// from a first reading.
#[test]
fn the_measured_trace_varies_so_a_peak_means_something() {
    let d = depths();
    let min = d.iter().copied().fold(f64::INFINITY, f64::min);
    let max = d.iter().copied().fold(0.0_f64, f64::max);
    assert!(
        max > min,
        "the fixture trace must vary in depth: {min} .. {max}"
    );
    let trace = measured_trace();
    assert_eq!(peak_axial_engagement(&trace), max);
}

// ── (g) the rigidity clamp is unchanged ──────────────────────────────

/// The Suggest clamp and the criterion now read ONE helper, so the cap
/// the operator's recipe was lowered to and the cap the row draws
/// against cannot disagree. This arm runs the production apply ("Apply
/// all", feeds contract P5). P29-7 7c: for each declared cut with a number
/// (the body and the slot ring of this contour Pocket), the planned ceiling
/// and the helper's cap of a sample of that width and chip are one number,
/// bit for bit, and the envelope's rigidity ceiling is the lowest of them. Byte-identity of every other Suggest figure is carried by the
/// apply-door sentries this change re-runs.
#[test]
fn the_suggest_clamp_still_fires_at_the_helper_s_own_cap() {
    const CLAMP_DIAMETER_MM: f64 = 12.0;
    const REQUESTED_DPP_MM: f64 = 36.0;
    const STEPOVER_MM: f64 = 4.0;

    let machine = MachineProfile::generic_wood_router();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };

    let mut tool = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = CLAMP_DIAMETER_MM;
    tool.set_flute_count(ToolFact::Stated(2));
    tool.set_cutting_length(ToolFact::Stated(50.0));
    tool.set_stickout(ToolFact::Stated(50.0));

    let mut operation = OperationConfig::Pocket(PocketConfig {
        stepover: Some(STEPOVER_MM),
        floor: rs_cam_core::compute::floor::Floor::top(2.0 * REQUESTED_DPP_MM),
        depth_per_pass: Some(REQUESTED_DPP_MM),
        feed_rate: Some(1000.0),
        plunge_rate: Some(400.0),
        ..PocketConfig::default()
    });

    // Feeds contract P5 (K2b): the production clamp is the envelope's
    // rigidity ceiling, and "Apply all" ships the proposal at or under it.
    // Before P5 this arm read the retired chain door (pass 4, then the dial).
    let envelope = SafeEnvelope::for_cut(EnvelopeInput {
        operation: &operation,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::no_project(NoProject::NONE),
    })
    .expect("a roughing pocket with a flat end mill has an envelope");
    let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
    apply_to_operation(
        OperationApplyInput {
            tool: &tool,
            machine: &machine,
            material: &material,
            spindle_strategy: SpindleStrategy::MatchChart,
            lut: embedded_vendor_lut(),
            suggest: SuggestContext::no_project(NoProject::NONE),
        },
        Some(PickRequest::Proposal),
        None,
        &mut operation,
        &mut provenance,
    )
    .expect("a roughing pocket with a flat end mill is a runnable pairing");

    // P13c (O1): the cap is the force-budget depth at the stated width and
    // the chip the envelope reads there (the grid chip at the floor depth,
    // no stated RPM), through the one producer.
    let floor = envelope
        .boundary
        .iter()
        .find_map(|l| match (l.kind, &l.edge) {
            (LimitKind::EngagementFloor, rs_cam_core::feeds::envelope::Edge::ApFloor { mm }) => {
                Some(*mm)
            }
            _ => None,
        })
        .expect("the depth floor");
    let at_floor = envelope
        .pick(PickRequest::Point {
            ap_mm: floor,
            ae_mm: STEPOVER_MM,
        })
        .result()
        .clone();
    let fz = at_floor.feed_rate_mm_min / (at_floor.rpm * 2.0);
    let cutter = rs_cam_core::compute::cutter::build_cutter(&tool).unwrap();
    // P29-7 7c: for each declared cut with a number, the ceiling before the
    // simulation (the one driver, `feeds::planned`) and the gate cap of a
    // sample of that width and chip after it (`rigidity_depth_cap`) are ONE
    // number. The envelope edge is the lowest of them.
    use rs_cam_core::compute::catalog::Presence;
    use rs_cam_core::feeds::planned::{
        BodyColumn, DeclaredCuts, ForceBudgetPlanned, ForceEnv, PlannedCell, judge_declared_cuts,
    };
    let cuts = DeclaredCuts::of_operation(
        &operation,
        &tool,
        &rs_cam_core::feeds::entry_rule::ConfiguredEntry::registry_default(&operation),
    );
    let column = BodyColumn {
        ae_mm: STEPOVER_MM,
        chip_mm: Some(fz),
        stored: None,
    };
    let budget_n = machine
        .rigidity
        .depth_cap_of(OperationType::Pocket.body_depth_limit())
        .expect("a pocket roughing pass reads the force budget")
        .budget_n;
    let env = ForceEnv {
        tool: &cutter,
        material: &material,
        bound: budget_n,
    };
    let cells = judge_declared_cuts::<ForceBudgetPlanned>(&cuts, column, &env);
    let mut expected = f64::INFINITY;
    let mut kinds = Vec::new();
    for (cut, cell) in cuts.list().iter().zip(cells.iter()) {
        let planned = cuts.planned(cut, column);
        let (Ok(w), Ok(cut_fz), PlannedCell::Judged(Some(depth))) =
            (planned.width_mm, planned.chip_mm, cell.value)
        else {
            continue;
        };
        let gate = match rigidity_depth_cap(
            &machine,
            OperationType::Pocket.body_depth_limit(),
            &cutter,
            &material,
            w,
            cut_fz,
        ) {
            Some(RigidityCapOutcome::Cap(c)) => c.cap_mm(),
            other => panic!(
                "{:?}: a roughing cut carries an axial cap: {other:?}",
                cut.kind
            ),
        };
        assert_eq!(
            depth.ceiling_mm(),
            Some(gate),
            "{:?}: the planned ceiling and the gate cap must be ONE number",
            cut.kind
        );
        if cut.presence == Presence::Always {
            expected = expected.min(gate);
        }
        kinds.push(cut.kind);
    }
    assert!(
        kinds.contains(&rs_cam_core::limits::CutKind::Body)
            && kinds.contains(&rs_cam_core::limits::CutKind::SlotRing),
        "non-vacuity: the body and the slot ring have a number: {kinds:?}"
    );

    // P19: the drawn curve holds no sample at the stated width; the
    // judged ceiling there is the exact one.
    let capped = envelope
        .judged_ap_ceiling(LimitKind::Rigidity, STEPOVER_MM)
        .expect("the roughing envelope must carry the rigidity ceiling");

    assert_eq!(
        capped, expected,
        "the clamp is the lowest cap of the declared cuts"
    );
    // P13c (O4): the proposal is the removal-rate pick inside the stated
    // box, so it may trade width for depth; it ships inside the rigidity
    // ceiling at the width it ships.
    let shipped = operation.depth_per_pass().unwrap();
    let shipped_ae = operation.stepover().unwrap();
    let ceiling = envelope
        .limit(LimitKind::Rigidity)
        .and_then(|l| l.ap_ceiling_at(shipped_ae))
        .unwrap();
    assert!(
        shipped <= ceiling + 1e-9 && shipped_ae <= STEPOVER_MM + 1e-12,
        "the shipped point ({shipped_ae}, {shipped}) is above the cap {ceiling}"
    );
    assert!(
        REQUESTED_DPP_MM > expected,
        "the fixture is vacuous unless the clamp actually lowers the depth"
    );
}

/// A drill operation carries no `depth_per_pass`, so the clamp's
/// `None` arm for the drill family is unreachable from Suggest. This is
/// why returning `None` there changes no Suggest number.
#[test]
fn a_drill_operation_has_no_depth_per_pass_for_the_clamp_to_read() {
    let m = machine();
    assert_eq!(
        m.rigidity
            .depth_cap_of(OperationType::Drill.body_depth_limit()),
        None,
        "a drill cycle has no axial cap"
    );
    let drill =
        OperationConfig::Drill(rs_cam_core::compute::operation_configs::DrillConfig::default());
    assert_eq!(
        drill.depth_per_pass(),
        None,
        "the clamp's `if let Some(current)` never binds on a drill op"
    );
}
