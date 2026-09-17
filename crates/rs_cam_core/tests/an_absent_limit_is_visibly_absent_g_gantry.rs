//! **S1 — an absence must render as an absence, never as nothing at all.**
//!
//! The gantry push force is the one load this crate computes and never
//! judges. `feeds::force::lateral_cutting_force` returns a number; no
//! `MachineProfile` field states the axis thrust the gantry can deliver,
//! so no bound exists to compare it against. Register item **T-10** holds
//! the blocker: no maker publishes a thrust rating, and the ballpark
//! figures span 20:1 across machine classes.
//!
//! Before this file the absence was invisible. The criterion tier listed
//! chipload, power and deflection, the operator read three rows, and the
//! fourth load — the one that makes a stepper gantry lose position —
//! was not on the surface at all. That is defect class 3 of the
//! programme (`RESUME_PLAN` §9) turned inside out: not an absence
//! rendering as a reading, but an absence rendering as nothing.
//!
//! The row this file pins is `CriterionKind::GantryPush`. It is always
//! `Unmodeled`, it never carries a number, and it must never prompt the
//! operator to act — no simulation run and no tool datum can fill it.
//!
//! What this file deliberately does NOT assert: any force, any thrust,
//! any bound. S1 adds no physics and no constant.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::gcode::{ToolLoadExportPolicy, enforce_load_policy};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::Material;
use rs_cam_core::ops::drill::DrillCycle;
use rs_cam_core::ops::drill_metrics::{build_drill_toolpath_summary, emit_drill_samples};
use rs_cam_core::ops::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::{
    ChiploadVerdict, CriterionKind, DeflectionVerdict, LoadState, PowerVerdict, ToolLoadReport,
    ToolpathLoadVerdict, UnmodeledReason,
};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext};

const TP: ToolpathId = ToolpathId(0);

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.35, 20.0)),
        6.35,
        30.0,
        20.0,
        30.0,
        2,
        ToolMaterial::Carbide,
    )
}

/// One measured cutting sample: real engagement, real arc, real feed,
/// and NOT in a transit span, so every gate's own filter keeps it.
fn cutting_sample(idx: usize) -> SimulationCutSample {
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
        spindle_rpm: 18_000,
        flute_count: 2,
        axial_doc_mm: 1.0,
        axial_engagement_mm: 1.0,
        arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
        chipload_mm_per_tooth: feed / (18_000.0 * 2.0),
        effective_chip_thickness_mm: Some(feed / (18_000.0 * 2.0)),
        engagement: Engagement::with_radial_woc(0.5),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        in_transit_span: false,
        ..SimulationCutSample::test_fixture()
    }
}

fn measured_trace() -> SimulationCutTrace {
    let samples: Vec<SimulationCutSample> = (0..12).map(cutting_sample).collect();
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
            peak_axial_doc_mm: 1.0,
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

/// A simulated MILLING toolpath: the three milling gates run against a
/// measured trace, so the report this sentry reads is the shipped shape,
/// not a hand-written one.
fn milling_verdict() -> ToolpathLoadVerdict {
    let trace = measured_trace();
    let tool = tool();
    let material = Material::default();
    let machine = MachineProfile::shapeoko_makita();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext {
        toolpath_id: TP,
        tool: &tool,
        material: &material,
        operation_family: rs_cam_core::feeds::vendor_lut::LutOperationFamily::Pocket,
        pass_role: rs_cam_core::feeds::vendor_lut::LutPassRole::Roughing,
        operation_feed_rate_mm_min: 2000.0,
        operation_kind: OperationType::Pocket,
        spans: None,
        drill_op: None,
    };
    let env = GateEnv {
        sim_trace: Some(&trace),
        machine: Some(&machine),
        tolerance: &tolerance,
    };
    ToolpathLoadVerdict {
        toolpath_id: TP,
        chipload: rs_cam_core::tool_load::chipload::evaluate(&ctx, &env),
        power: rs_cam_core::tool_load::power::evaluate(&ctx, &env),
        deflection: rs_cam_core::tool_load::deflection::evaluate(&ctx, &env),
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
    }
}

fn drill_op() -> DrillOp {
    DrillOp {
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
    }
}

/// A drill cycle, evaluated through the shipped drill path. The three
/// milling gates short-circuit to `NotApplicableForOp` on drill
/// kinematics; the gantry row must take the same arm.
fn drill_verdict() -> ToolpathLoadVerdict {
    let d = drill_op();
    let samples = emit_drill_samples(TP, &d);
    let summary = build_drill_toolpath_summary(TP, &d, &samples);
    let gates = rs_cam_core::tool_load::drill_gates::evaluate(&d, &summary);
    ToolpathLoadVerdict {
        toolpath_id: TP,
        chipload: ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(
                "drill cycle — no continuous engagement".to_owned(),
            ),
        },
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(
                "drill cycle — no continuous engagement".to_owned(),
            ),
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(
                "drill cycle — no continuous engagement".to_owned(),
            ),
        },
        drill_gates: Some(gates),
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
    }
}

// ── (a) the row exists, once, and says why it is empty ───────────────

/// The row is in the criterion tier for a milling toolpath, exactly
/// once, with no number and a reason that names the register item.
#[test]
fn a_milling_toolpath_carries_exactly_one_gantry_push_row() {
    let v = milling_verdict();
    let rows: Vec<_> = v
        .criteria()
        .into_iter()
        .filter(|c| c.kind == CriterionKind::GantryPush)
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "a milling toolpath must carry the gantry row exactly once"
    );
    let row = &rows[0];
    assert_eq!(
        row.state,
        LoadState::Unmodeled,
        "the gantry push is computed but never judged; it cannot be Within"
    );
    assert_eq!(row.unit, "N", "the gantry push is a force");
    assert_eq!(row.kind.label(), "gantry push");
    assert!(
        row.display_peak.is_none(),
        "an unjudged row must not print a number: {:?}",
        row.display_peak
    );
    assert!(
        row.exceeded.is_none(),
        "an unjudged row cannot carry an exceedance"
    );
    assert!(row.sample_range.is_none(), "no population, no range");
    match row.unmodeled_reason {
        Some(UnmodeledReason::NotImplemented(clause)) => {
            assert!(
                clause.contains("T-10"),
                "the clause must cite the register item, got: {clause}"
            );
            assert!(
                !clause.contains("planning/"),
                "no `planning/…` path in an operator-facing string, got: {clause}"
            );
        }
        other => panic!("expected NotImplemented(<clause naming T-10>), got {other:?}"),
    }
}

/// **The X-VAC line.** Vacuity is an EMPTY population. This row has no
/// population at all, which is "not stated", never "zero", so
/// `is_vacuous` stays false and the vacuity clause stays empty.
#[test]
fn the_gantry_row_is_absent_not_vacuous() {
    let v = milling_verdict();
    let row = v
        .criteria()
        .into_iter()
        .find(|c| c.kind == CriterionKind::GantryPush)
        .expect("the gantry row");
    assert!(
        row.population.is_none(),
        "the row states no population; it is not a gate that measured nothing"
    );
    assert!(
        !row.is_vacuous(),
        "X-VAC is an empty population, not an absent one"
    );
    assert!(
        row.vacuity_clause().is_empty(),
        "a row with no population has nothing to call vacuous"
    );
}

// ── (b) a drill has no lateral engagement ────────────────────────────

/// A drill cycle pushes down, not sideways. The gantry row takes the
/// same arm the three milling gates take on drill kinematics, so the
/// "drill cycle, milling gates don't apply" partition is unchanged.
#[test]
fn a_drill_cycle_reads_not_applicable_not_not_implemented() {
    let v = drill_verdict();
    let row = v
        .criteria()
        .into_iter()
        .find(|c| c.kind == CriterionKind::GantryPush)
        .expect("the gantry row");
    assert_eq!(row.state, LoadState::Unmodeled);
    match row.unmodeled_reason {
        Some(UnmodeledReason::NotApplicableForOp(detail)) => {
            assert!(
                detail.contains("drill"),
                "the detail must name the op family, got: {detail}"
            );
        }
        other => panic!("a drill's gantry row must read NotApplicableForOp, got {other:?}"),
    }
    // A drill WITH drill gates carries three modelled rows, so it
    // buckets as `within`. The gantry row must not move it.
    let report = ToolLoadReport {
        per_toolpath: vec![v],
    };
    let summary = report.summary(|_| None);
    assert_eq!(summary.within, 1);
    assert_eq!(summary.fully_unmodeled, 0);
    assert_eq!(summary.not_applicable, 0);
}

/// **The partition the gantry row could have broken.** A drill whose
/// `drill_gates` are `None` — the optimizer path — has nothing but
/// `NotApplicableForOp` rows, and `all_not_applicable` buckets it as
/// "doesn't apply", not "couldn't measure". Had the gantry row read
/// `NotImplemented` here, the toolpath would have flipped to
/// `fully_unmodeled` and the Readiness panel would have told the
/// operator to re-run a simulation for a hole.
#[test]
fn a_drill_without_drill_gates_still_buckets_as_not_applicable() {
    let mut v = drill_verdict();
    v.drill_gates = None;
    assert_eq!(v.modeled_count(), 0, "fixture guard: nothing is modelled");
    let report = ToolLoadReport {
        per_toolpath: vec![v],
    };
    let summary = report.summary(|_| None);
    assert_eq!(
        summary.not_applicable, 1,
        "a plunge-only op needs no operator action"
    );
    assert_eq!(summary.fully_unmodeled, 0);
}

// ── (c) the row never gates an export ────────────────────────────────

/// `exceeded_criteria` derives from `criteria()`, so a row added to the
/// tier gates export by construction. This row must not: it has no
/// bound, so it can never exceed one.
#[test]
fn the_gantry_row_never_refuses_an_export() {
    let v = milling_verdict();
    assert!(
        !v.exceeded_criteria()
            .iter()
            .any(|ec| ec.label.contains("gantry")),
        "an unjudged row cannot appear in the exceedance list"
    );
    let report = ToolLoadReport {
        per_toolpath: vec![v],
    };
    assert!(
        !report.any_exceeded(),
        "the fixture's modelled gates are within bounds"
    );
    // Both overrides off — the strictest policy the export gate has.
    let strict = ToolLoadExportPolicy {
        accept_unmodeled: false,
        accept_exceeded: false,
    };
    let outcome = enforce_load_policy(&report, &strict);
    assert!(
        outcome.is_ok(),
        "a known absence must not refuse an export: {}",
        outcome.unwrap_err()
    );
}

// ── (d) a permanent absence must not prompt an action ────────────────

/// The row must not make the toolpath count as unmodelled for the
/// purpose of "run the simulation or supply tool data". No operator
/// action can fill this row, so a prompt is a dead end.
#[test]
fn the_gantry_row_does_not_make_a_toolpath_unmodeled() {
    let v = milling_verdict();
    assert!(
        v.modeled_count() > 0,
        "fixture guard: the milling gates must reach a modelled verdict"
    );
    assert!(
        !v.any_unmodeled(),
        "a known absence is not an unmodelled criterion"
    );
    let report = ToolLoadReport {
        per_toolpath: vec![v],
    };
    assert!(!report.any_unmodeled());
    let summary = report.summary(|_| None);
    assert_eq!(summary.total_toolpaths, 1);
    assert_eq!(
        summary.fully_unmodeled, 0,
        "the gantry row alone cannot make a measured toolpath read as unmodelled"
    );
    assert_eq!(summary.not_applicable, 0);
    assert_eq!(summary.within, 1);
    assert_eq!(summary.exceeds, 0);
}

/// **The predicate must not read the bare `NotImplemented` variant.**
///
/// Found while building S1: two shipped gates already refuse with
/// `NotImplemented` for reasons an operator CAN fix — the power gate
/// when no `MachineProfile` reached the evaluator, and the deflection
/// gate when the tool reports zero stickout. A predicate keyed on the
/// variant alone would stop the export gate refusing on either, which
/// silently weakens two gates that work.
///
/// The rule is the KIND: a criterion this crate models not at all.
#[test]
fn a_fillable_not_implemented_row_is_not_a_known_absence() {
    // The row S1 adds: no model exists, nothing fills it.
    assert!(CriterionKind::GantryPush.is_unmodeled_by_design());
    // The three that are modelled, and refuse only when an input is
    // missing.
    for kind in [
        CriterionKind::Chipload,
        CriterionKind::Power,
        CriterionKind::Deflection,
    ] {
        assert!(
            !kind.is_unmodeled_by_design(),
            "{kind:?} has a gate; a refusal from it names a missing input"
        );
    }

    // The power gate's own `NotImplemented` refusal, verbatim: no
    // machine profile. It must still block an export.
    let v = ToolpathLoadVerdict {
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented(
                "machine profile not provided to evaluator".to_owned(),
            ),
        },
        ..milling_verdict()
    };
    let power_row = v
        .criteria()
        .into_iter()
        .find(|c| c.kind == CriterionKind::Power)
        .expect("the power row");
    assert!(
        !power_row.is_known_absence(),
        "supply a machine profile and this row fills; it is not a known absence"
    );
    assert!(
        v.any_unmodeled(),
        "a missing machine profile must still read as unmodelled"
    );
    let report = ToolLoadReport {
        per_toolpath: vec![v],
    };
    let strict = ToolLoadExportPolicy {
        accept_unmodeled: false,
        accept_exceeded: false,
    };
    assert!(
        enforce_load_policy(&report, &strict).is_err(),
        "the export gate must still refuse a report with no machine profile"
    );
}

/// The drill arm is not a known absence either — it keeps the
/// `NotApplicableForOp` partition the three milling gates use, and the
/// drill short-circuit in `any_unmodeled` already handles it.
#[test]
fn a_drill_gantry_row_is_not_a_known_absence() {
    let v = drill_verdict();
    let row = v
        .criteria()
        .into_iter()
        .find(|c| c.kind == CriterionKind::GantryPush)
        .expect("the gantry row");
    assert!(
        !row.is_known_absence(),
        "the drill arm is `NotApplicableForOp`, which the partition already owns"
    );
}

// ── (e) non-vacuity: the row did not displace the three ──────────────

/// The anchor. A file that only asserts the new row would still pass if
/// the three modelled criteria vanished.
#[test]
fn the_three_milling_criteria_are_still_there_and_at_least_one_is_modelled() {
    let v = milling_verdict();
    let kinds: Vec<CriterionKind> = v.criteria().iter().map(|c| c.kind).collect();
    for wanted in [
        CriterionKind::Chipload,
        CriterionKind::Power,
        CriterionKind::Deflection,
    ] {
        assert!(
            kinds.contains(&wanted),
            "{wanted:?} left the criterion tier: {kinds:?}"
        );
    }
    assert_eq!(
        kinds,
        vec![
            CriterionKind::Chipload,
            CriterionKind::Power,
            CriterionKind::Deflection,
            CriterionKind::GantryPush,
        ],
        "the gantry row goes after deflection, and no drill row joins a milling toolpath"
    );
    let modelled = v
        .criteria()
        .into_iter()
        .filter(|c| {
            c.state != LoadState::Unmodeled
                && matches!(
                    c.kind,
                    CriterionKind::Chipload | CriterionKind::Power | CriterionKind::Deflection
                )
        })
        .count();
    assert!(
        modelled >= 1,
        "at least one milling criterion must be modelled, or every assertion above is vacuous"
    );
}
