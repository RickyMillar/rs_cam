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
//! **The row does not gate.** Its bound is
//! `RigidityProfile::doc_*_factor` times the tool diameter: a rule of
//! thumb with no published source. S4 typed that as
//! `BoundSource::RigidityRuleOfThumb`, whose `gates_export()` is false,
//! and wrote the export policy against hand-built rows because nothing
//! produced the variant yet. S3 is its first producer, so arm (d) below
//! is the first end-to-end exercise of that path: an exceeded depth row
//! is REPORTED and the export still goes ahead, while an exceeded power
//! row beside it still refuses.
//!
//! **S3 adds no number.** Every bound here is the factor the profile
//! already carries times the tool diameter, through the one shared
//! helper `RigidityProfile::depth_cap_mm`, which the Suggest clamp now
//! calls too. The two cannot disagree.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    ApplyContext, ApplyScope, FeedsPreview, SuggestContext, SuggestWarning,
};
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::feeds::{
    FeedsInput, OperationFamily, PassRole, SetupContext, SpindleStrategy, ToolGeometryHint,
    embedded_vendor_lut,
};
use rs_cam_core::gcode::{ToolLoadExportPolicy, enforce_load_policy};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
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

const TP: ToolpathId = ToolpathId(0);
const SAMPLE_RPM: u32 = 18_000;

/// The cutter every measured arm uses. Ø6.35 flat, two flutes.
const DIAMETER_MM: f64 = 6.35;

/// The measured trace's depths, in mm, one per sample. They VARY, so
/// "peak" is a real maximum over a population and not a constant
/// restated. The maximum is 0.95 mm.
fn depths() -> Vec<f64> {
    (0..12).map(|i| 0.40 + 0.05 * i as f64).collect()
}

/// The deep hand-set trace of arm (d): every sample cuts 3.000 mm, over
/// the 1.5875 mm roughing cap on this tool and profile.
const DEEP_MM: f64 = 3.0;

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(DIAMETER_MM, 20.0)),
        DIAMETER_MM,
        30.0,
        20.0,
        30.0,
        2,
        ToolMaterial::Carbide,
    )
}

/// One measured cutting sample at `axial_engagement_mm = depth`, not in
/// a transit span, so `locality::is_steady_state_for_gate` keeps it.
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
    trace_from((0..6).map(|i| cutting_sample(i, DEEP_MM)).collect())
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

fn depth_verdict_for(
    op: OperationType,
    family: LutOperationFamily,
    pass_role: LutPassRole,
    trace: &SimulationCutTrace,
) -> DepthVerdict {
    let tool = tool();
    let material = Material::default();
    let machine = machine();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext {
        toolpath_id: TP,
        tool: &tool,
        material: &material,
        operation_family: family,
        pass_role,
        operation_feed_rate_mm_min: 2000.0,
        operation_kind: op,
        spans: None,
        drill_op: None,
    };
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
    let ctx = ToolpathLoadContext {
        toolpath_id: TP,
        tool: &tool,
        material: &material,
        operation_family: LutOperationFamily::Pocket,
        pass_role: LutPassRole::Roughing,
        operation_feed_rate_mm_min: 2000.0,
        operation_kind: op,
        spans: None,
        drill_op: None,
    };
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

/// The cap the profile implies for one family, read back through the
/// one shared helper. Nothing in this file multiplies a factor itself.
fn cap_for(family: OperationFamily, pass_role: PassRole) -> (f64, f64) {
    let m = machine();
    let cap = m
        .rigidity
        .depth_cap_mm(family, pass_role, DIAMETER_MM)
        .expect("a milling family carries an axial cap");
    (cap.factor, cap.cap_mm())
}

// ── (a) the row states the measured peak and the bound that judged it ─

#[test]
fn a_roughing_depth_row_states_the_measured_peak_and_its_bound() {
    let trace = measured_trace();
    let v = milling_verdict(OperationType::Pocket, &trace);
    let criteria = v.criteria();
    let row = depth_row(&criteria);

    let (factor, cap_mm) = cap_for(OperationFamily::Pocket, PassRole::Roughing);
    assert_eq!(
        row.bound,
        Some(cap_mm),
        "the depth row must read against the profile's own cap"
    );
    assert_eq!(
        row.bound_source,
        Some(BoundSource::RigidityRuleOfThumb {
            factor,
            diameter_mm: DIAMETER_MM,
        }),
        "the provenance must carry the two numbers the cap is built from"
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

/// The bound a rule of thumb sets may never refuse an export, and the
/// kind is NOT a known absence: a depth row is a real gate with a real
/// producer, unlike the gantry-push row beside it.
#[test]
fn the_depth_bound_does_not_gate_and_the_kind_is_not_a_known_absence() {
    assert!(
        !CriterionKind::DepthOfCut.is_unmodeled_by_design(),
        "the depth row is produced by a real gate"
    );
    let source = BoundSource::RigidityRuleOfThumb {
        factor: 0.25,
        diameter_mm: DIAMETER_MM,
    };
    assert!(!source.gates_export());
    assert_eq!(source.setting(), "machine");
    assert!(
        source.clause().contains("rule of thumb"),
        "the clause must say the bound has no published source: {}",
        source.clause()
    );
}

// ── (b) two capped families, and a finishing pass with no cap ────────

/// Re-blessed under feeds matrix R2 (2026-09-23). Before R2 a finishing
/// Trace read `doc_finishing_factor` (0.10 × Ø6.35 = 0.635 mm on this
/// profile). The operator ruled that a finishing or semi-finishing pass
/// gets no axial ceiling: no vendor publishes one for wood (EVIDENCE
/// 5.1-3, 5.1-12). The finishing arm now reports the measured peak with
/// no bound and no bound source, and the state is `Within`. The two
/// roughing arms keep their two distinct factors.
#[test]
fn each_family_reads_the_factor_its_own_pass_role_implies() {
    let trace = measured_trace();

    // Conventional roughing → `doc_roughing_factor`.
    let (rough_factor, rough_cap) = cap_for(OperationFamily::Pocket, PassRole::Roughing);
    let rough = depth_verdict_for(
        OperationType::Pocket,
        LutOperationFamily::Pocket,
        LutPassRole::Roughing,
        &trace,
    );
    let rough_status = rough.as_criterion_status();
    assert_eq!(rough_status.bound, Some(rough_cap));

    // Finishing → no cap (R2). The peak is reported with no bound.
    assert!(
        machine()
            .rigidity
            .depth_cap_mm(OperationFamily::Trace, PassRole::Finish, DIAMETER_MM)
            .is_none(),
        "a finishing pass must carry no axial cap"
    );
    let finish = depth_verdict_for(
        OperationType::Trace,
        LutOperationFamily::Trace,
        LutPassRole::Finish,
        &trace,
    );
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

    // Adaptive → `adaptive_doc_factor`.
    let (adaptive_factor, adaptive_cap) = cap_for(OperationFamily::Adaptive, PassRole::Roughing);
    let adaptive = depth_verdict_for(
        OperationType::Adaptive,
        LutOperationFamily::Adaptive,
        LutPassRole::Roughing,
        &trace,
    );
    let adaptive_status = adaptive.as_criterion_status();
    assert_eq!(adaptive_status.bound, Some(adaptive_cap));

    // Two DISTINCT factors, read back off the profile. Without this
    // the two capped arms above could both be reading one field.
    let m = machine();
    assert_eq!(rough_factor, m.rigidity.doc_roughing_factor);
    assert_eq!(adaptive_factor, m.rigidity.adaptive_doc_factor);
    assert!(
        rough_factor != adaptive_factor,
        "the fixture profile must publish two different factors, else \
         this arm proves nothing: {rough_factor} {adaptive_factor}"
    );
}

// ── (c) a drill cycle has no axial cap ───────────────────────────────

#[test]
fn a_drill_cycle_carries_the_same_not_applicable_clause_as_the_milling_gates() {
    let trace = measured_trace();
    let verdict = depth_verdict_for(
        OperationType::Drill,
        LutOperationFamily::Drill,
        LutPassRole::Roughing,
        &trace,
    );
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
        }],
        hole_source: HoleSource::ModelDerived,
        tool_profile: ToolProfile::StandardTwist,
        tool_diameter_mm: 6.0,
        cycle: DrillCycle::Peck(3.0),
        feed_rate_mm_min: 300.0,
        spindle_rpm: 6_000,
        flute_count: 2,
        material: Material::default(),
        retract_z_mm: 5.0,
    };
    let tool = tool();
    let material = Material::default();
    let machine = machine();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext {
        toolpath_id: TP,
        tool: &tool,
        material: &material,
        operation_family: LutOperationFamily::Drill,
        pass_role: LutPassRole::Roughing,
        operation_feed_rate_mm_min: 300.0,
        operation_kind: OperationType::Drill,
        spans: None,
        drill_op: Some(&d),
    };
    rs_cam_core::tool_load::evaluate_toolpath(&ctx, None, Some(&machine), &tolerance)
}

// ── (d) the non-gating path, end to end ──────────────────────────────

/// The first real producer of `BoundSource::RigidityRuleOfThumb`. An
/// operator typed a depth above the cap; no Suggest run lowered it. The
/// row exceeds, `exceeded_criteria()` lists it, and the export goes
/// ahead with a note.
#[test]
fn a_hand_set_depth_above_the_cap_exceeds_but_does_not_refuse_the_export() {
    let trace = deep_trace();
    let v = milling_verdict(OperationType::Pocket, &trace);
    let criteria = v.criteria();
    let row = depth_row(&criteria);
    let (_, cap_mm) = cap_for(OperationFamily::Pocket, PassRole::Roughing);

    assert_eq!(
        row.state,
        LoadState::Exceeds,
        "a {DEEP_MM:.3} mm cut against a {cap_mm:.4} mm cap exceeds"
    );
    assert!(
        row.display_peak.unwrap() > row.bound.unwrap(),
        "the exceedance must be a real one: peak {:?} vs bound {:?}",
        row.display_peak,
        row.bound
    );
    assert!(
        !row.refuses_export(),
        "a rule-of-thumb bound may not refuse an export"
    );
    assert!(
        v.exceeded_criteria()
            .iter()
            .any(|e| e.kind == CriterionKind::DepthOfCut),
        "exceeded_criteria() must still LIST the depth row"
    );

    let policy = ToolLoadExportPolicy {
        accept_unmodeled: true,
        accept_exceeded: false,
    };
    let report = ToolLoadReport {
        per_toolpath: vec![v],
    };
    let note = enforce_load_policy(&report, &policy)
        .expect("a depth exceedance alone must not refuse the export");
    let note = note.expect("the export gate must still REPORT the exceedance");
    assert!(
        note.contains("depth of cut"),
        "the note must name the depth row: {note}"
    );
    assert!(
        note.contains("rule of thumb"),
        "the note must say why the bound does not gate: {note}"
    );
}

/// The same report with a power exceedance beside it. Power's bound is
/// the machine power curve, which DOES gate, so the export refuses —
/// and the message names both rows, so the operator sees the whole
/// picture and not just the half that stopped them.
#[test]
fn a_power_exceedance_beside_the_depth_row_refuses_and_names_both() {
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
    let policy = ToolLoadExportPolicy {
        accept_unmodeled: true,
        accept_exceeded: false,
    };
    let err = enforce_load_policy(&report, &policy)
        .expect_err("a power exceedance must refuse the export");
    let msg = err.to_string();
    assert!(
        msg.contains("spindle power"),
        "the refusal must name what refused: {msg}"
    );
    assert!(
        msg.contains("depth of cut"),
        "the refusal must also name what exceeded without refusing: {msg}"
    );
}

// ── (e) a trace with no cutting samples is vacuous ───────────────────

#[test]
fn a_trace_with_no_cutting_samples_is_vacuous_not_a_green_zero() {
    let trace = air_trace();
    let verdict = depth_verdict_for(
        OperationType::Pocket,
        LutOperationFamily::Pocket,
        LutPassRole::Roughing,
        &trace,
    );
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
/// against cannot disagree. This arm runs the whole Suggest door and
/// asserts the clamp still fires at the helper's own number, bit for
/// bit. Byte-identity of every other Suggest figure is carried by the
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

    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = CLAMP_DIAMETER_MM;
    tool.flute_count = 2;
    tool.cutting_length = 50.0;
    tool.stickout = 50.0;

    let mut operation = OperationConfig::Pocket(PocketConfig {
        stepover: STEPOVER_MM,
        depth: 2.0 * REQUESTED_DPP_MM,
        depth_per_pass: REQUESTED_DPP_MM,
        feed_rate: 1000.0,
        plunge_rate: 400.0,
        ..PocketConfig::default()
    });

    let input = FeedsInput {
        tool_diameter: CLAMP_DIAMETER_MM,
        flute_count: 2,
        flute_length: 50.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(REQUESTED_DPP_MM),
        radial_width_mm: Some(STEPOVER_MM),
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    };
    let preview = FeedsPreview::build(&input);
    let rec = preview
        .applicable()
        .expect("a roughing pocket with a flat end mill is a runnable pairing");
    let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
    let warnings = rs_cam_core::feeds::suggest::apply(
        &rec,
        ApplyScope::Both,
        &mut operation,
        &mut provenance,
        ApplyContext {
            tool: &tool,
            machine: &machine,
            material: &material,
            pass_role: PassRole::Roughing,
            suggest: SuggestContext::default(),
        },
    );

    let expected = machine
        .rigidity
        .depth_cap_mm(OperationFamily::Pocket, PassRole::Roughing, tool.diameter)
        .expect("a pocket roughing pass carries an axial cap")
        .cap_mm();

    let capped = warnings
        .iter()
        .find_map(|w| match w {
            SuggestWarning::RoughingDepthClampedToRigidity { capped, .. } => Some(*capped),
            _ => None,
        })
        .expect("the rigidity clamp must still fire on this fixture");

    assert_eq!(
        capped, expected,
        "the clamp and the criterion must read ONE cap"
    );
    // Ruling R4 WP3 (2026-09-24): after the clamp, the aggressiveness dial
    // (default 0.85) scales the depth down from the cap. Measured: 2.4 ->
    // 1.8 mm. The dial record starts from the cap the helper returns.
    let shipped = operation.depth_per_pass().unwrap();
    assert!(
        shipped <= expected + 1e-12,
        "the shipped depth {shipped} is above the cap {expected}"
    );
    let dial = warnings.iter().find_map(|w| match w {
        SuggestWarning::EngagementReducedForAggressiveness {
            dpp_from, dpp_to, ..
        } => Some((*dpp_from, *dpp_to)),
        _ => None,
    });
    assert_eq!(
        dial,
        Some((Some(expected), Some(shipped))),
        "the dial record must start from the helper's cap and end at the shipped depth"
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
            .depth_cap_mm(OperationFamily::Drill, PassRole::Roughing, DIAMETER_MM),
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
