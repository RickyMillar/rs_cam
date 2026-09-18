//! **S4 — a criterion carries the bound it was judged against, and says
//! where that bound came from.**
//!
//! Before S4 a criterion row carried a reading and no limit. Two of the
//! three caps the badge drew against were built in the GUI
//! (`rs_cam_viz/src/ui/sim_diagnostics.rs`): the power cap as
//! `max_power_kw × safety_factor`, and the deflection cap as
//! `DEFLECTION_SAFE_LD_RATIO = 4.0`. The second is an L over D RATIO
//! drawn beside a gate that judges MILLIMETRES against
//! `deflection::EXCEEDS_BOUND_MM`. That is defect class 2 of this
//! programme (`RESUME_PLAN` §9): a quantity divided by a fraction of a
//! DIFFERENT quantity. The GUI is not an alternate data model.
//!
//! This file pins the repair. Every modelled row states `bound` in its
//! own `unit`, and `bound_source` says which setting set it. The bound
//! is the value the gate already judged against. S4 adds no number.
//!
//! The provenance is TYPED, never prose (`REVIEW_DESIGN.md` §6.3: doc
//! comments under `crates/` cite 231 `planning/…` paths and 117 of them
//! resolve to nothing). A clause is formatted from the values the
//! source carries, so it cannot drift from the constant it names. This
//! file therefore checks the DIGITS of the bound inside the clause, not
//! a literal string.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
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
use rs_cam_core::tool_load::deflection::EXCEEDS_BOUND_MM;
use rs_cam_core::tool_load::verdict::{
    BoundSource, ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, ChiploadVerdict,
    Confidence, CriterionKind, CriterionRow, CriterionStatus, DeflectionVerdict, DepthVerdict,
    LoadState, PowerVerdict, SampleEvidence, ToolpathLoadVerdict, UnmodeledReason,
};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext};

const TP: ToolpathId = ToolpathId(0);
/// The spindle speed every sample of the measured trace runs at. The
/// power gate's ceiling is `power_at_rpm(rpm) × safety_factor`, so this
/// is the number the bound's provenance must carry back.
const SAMPLE_RPM: u32 = 18_000;

/// The four words `BoundSource::setting` may return. The operator's
/// rule 3: every limit traces back to the setting that set it, and a
/// setting is a live symbol a reader can find.
const SETTINGS: [&str; 4] = ["machine", "tool", "material", "vendor row"];

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
        spindle_rpm: SAMPLE_RPM,
        flute_count: 2,
        axial_doc_mm: 1.0,
        axial_engagement_mm: 1.0,
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

fn machine() -> MachineProfile {
    MachineProfile::shapeoko_makita()
}

/// A simulated MILLING toolpath. The three milling gates run against a
/// measured trace, so every bound this file reads is the one the
/// shipped gate judged against, not a hand-written one.
fn milling_verdict() -> ToolpathLoadVerdict {
    let trace = measured_trace();
    let tool = tool();
    let material = Material::default();
    let machine = machine();
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
        // S3: the depth gate runs against the same measured trace.
        depth: rs_cam_core::tool_load::depth::evaluate(&ctx, &env),
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

/// A drill cycle, through the shipped drill path.
fn drill_verdict() -> ToolpathLoadVerdict {
    let d = drill_op();
    let samples = emit_drill_samples(TP, &d);
    let summary = build_drill_toolpath_summary(TP, &d, &samples);
    let gates = rs_cam_core::tool_load::drill_gates::evaluate(&d, &summary);
    let na =
        || UnmodeledReason::NotApplicableForOp("drill cycle — no continuous engagement".to_owned());
    ToolpathLoadVerdict {
        toolpath_id: TP,
        chipload: ChiploadVerdict::Unmodeled { reason: na() },
        power: PowerVerdict::Unmodeled { reason: na() },
        deflection: DeflectionVerdict::Unmodeled { reason: na() },
        depth: DepthVerdict::Unmodeled { reason: na() },
        drill_gates: Some(gates),
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
    }
}

/// A hand-built chipload verdict with a KNOWN band, so the band arms
/// below cannot go vacuous on a fixture whose vendor row happens not to
/// match. The bound and the floor are the ones this verdict carries.
const BAND_FLOOR: f64 = 0.038;
const BAND_CEILING: f64 = 0.070;

fn chipload_within_known_band() -> ChiploadVerdict {
    ChiploadVerdict::Within {
        approach_to_min: None,
        approach_to_max: ChiploadMetric {
            observed_mm_per_tooth: 0.055,
            statistic: ChiploadStatistic::PeakInRange,
            evidence: SampleEvidence::at(3),
            bounds: ChipBounds {
                min_mm_per_tooth: Some(BAND_FLOOR),
                max_mm_per_tooth: BAND_CEILING,
                source: ChipBoundsSource::VendorLut,
            },
        },
        confidence: Confidence::Validated,
        entry_spikes: Vec::new(),
        burn_advisory: None,
        ceiling_advisory: None,
    }
}

/// Render the bound the way a surface does, then look for its digits.
/// The point of the clause is that the number is FORMATTED from the
/// value at render time and never typed into the string.
fn digits(value: f64, places: usize) -> String {
    format!("{value:.places$}")
}

// ── (a) every modelled row states its bound and its provenance ───────

/// The backbone. A modelled row without a bound is a reading with no
/// limit, which is what sent the GUI off to invent two of its own.
#[test]
fn every_modelled_milling_row_carries_a_bound_and_a_source() {
    let v = milling_verdict();
    let criteria = v.criteria();
    let mut modelled = 0_usize;
    for row in &criteria {
        if !matches!(
            row.kind,
            CriterionKind::Chipload | CriterionKind::Power | CriterionKind::Deflection
        ) {
            continue;
        }
        if row.state == LoadState::Unmodeled {
            continue;
        }
        modelled += 1;
        assert!(
            row.bound.is_some(),
            "{:?} is modelled and states no bound",
            row.kind
        );
        assert!(
            row.bound_source.is_some(),
            "{:?} states a bound with no source",
            row.kind
        );
    }
    assert!(
        modelled >= 2,
        "the fixture must model at least the power and deflection gates, \
         or every assertion above passes on an empty loop (modelled: {modelled})"
    );
}

/// **Non-vacuity.** At least one row is a measured `Within` with a
/// positive peak, so the arms above did not pass on an all-unmodelled
/// report.
#[test]
fn at_least_one_row_is_a_measured_within_with_a_positive_peak() {
    let v = milling_verdict();
    let criteria = v.criteria();
    let measured: Vec<&CriterionStatus<'_>> = criteria
        .iter()
        .filter(|r| {
            r.state == LoadState::Within
                && r.display_peak.is_some_and(|p| p > 0.0)
                && !r.is_vacuous()
        })
        .collect();
    assert!(
        !measured.is_empty(),
        "no row is a measured Within with a positive peak; every bound arm \
         in this file would then pass on an all-unmodelled report"
    );
    for row in measured {
        assert!(
            row.bound.is_some(),
            "{:?} measured a peak and states no bound",
            row.kind
        );
    }
}

// ── (b) the deflection bound is millimetres, not an L over D ratio ───

/// The class-2 repair, stated as a number. The gate judges the peak tip
/// displacement against `EXCEEDS_BOUND_MM`; the row must now say so.
#[test]
fn the_deflection_bound_is_the_gates_own_millimetre_budget() {
    let v = milling_verdict();
    let row = v
        .criteria()
        .into_iter()
        .find(|r| r.kind == CriterionKind::Deflection)
        .expect("the deflection row");
    assert_ne!(
        row.state,
        LoadState::Unmodeled,
        "the fixture must model deflection, or this arm proves nothing"
    );
    assert_eq!(
        row.bound,
        Some(EXCEEDS_BOUND_MM),
        "the deflection row must read against the gate's own bound"
    );
    assert_eq!(row.unit, "mm", "the bound is a displacement, not a ratio");
    assert_eq!(
        row.bound_source,
        Some(BoundSource::DeflectionBudget),
        "the deflection bound is the tip displacement budget"
    );
}

// ── (c) the power bound is the ceiling the gate used ─────────────────

/// The power bound is `available_kw`, and its provenance carries the
/// two inputs that built it: the spindle speed and the machine's
/// safety factor.
#[test]
fn the_power_bound_is_available_kw_and_names_the_rpm_and_the_safety_factor() {
    let v = milling_verdict();
    let available = match &v.power {
        PowerVerdict::Within { available_kw, .. } | PowerVerdict::Exceeds { available_kw, .. } => {
            *available_kw
        }
        PowerVerdict::Unmodeled { reason } => {
            panic!("the fixture must model power, got Unmodeled({reason:?})")
        }
    };
    assert!(available > 0.0, "a modelled power gate has a ceiling");
    let row = v
        .criteria()
        .into_iter()
        .find(|r| r.kind == CriterionKind::Power)
        .expect("the power row");
    assert_eq!(
        row.bound,
        Some(available),
        "the power row's bound is the ceiling the gate judged against"
    );
    let profile = machine();
    match row.bound_source {
        Some(BoundSource::MachinePowerCurve { rpm, safety_factor }) => {
            assert!(
                (rpm - f64::from(SAMPLE_RPM)).abs() < 1e-9,
                "the source must carry the toolpath's own rpm, got {rpm}"
            );
            assert!(
                (safety_factor - profile.safety_factor).abs() < 1e-12,
                "the source must carry the profile's safety factor, got {safety_factor}"
            );
            // The two inputs reproduce the bound. No third number hides
            // in the gate.
            let reproduced = profile.power_at_rpm(rpm) * safety_factor;
            assert!(
                (reproduced - available).abs() < 1e-9,
                "power_at_rpm({rpm}) × {safety_factor} = {reproduced}, \
                 but the gate judged against {available}"
            );
        }
        other => panic!("expected MachinePowerCurve, got {other:?}"),
    }
}

// ── (d) the chipload bound is the ceiling; the floor rides along ─────

/// The chipload row reads against the band CEILING, because that is the
/// breakage side the badge draws a percentage of. The FLOOR travels in
/// the source, because the burn branch needs it and a burn reading sits
/// BELOW its bound, not above it.
#[test]
fn the_chipload_bound_is_the_band_ceiling_and_the_source_carries_the_floor() {
    let verdict = chipload_within_known_band();
    let row = verdict.as_criterion_status();
    assert_eq!(
        row.bound,
        Some(BAND_CEILING),
        "the chipload row reads against the band ceiling"
    );
    assert_eq!(row.unit, "mm/tooth");
    match row.bound_source {
        Some(BoundSource::VendorChipBand {
            floor_mm_per_tooth,
            ceiling_mm_per_tooth,
            source,
        }) => {
            assert_eq!(
                floor_mm_per_tooth,
                Some(BAND_FLOOR),
                "the burn floor travels with the band"
            );
            assert!((ceiling_mm_per_tooth - BAND_CEILING).abs() < 1e-12);
            assert_eq!(
                source,
                ChipBoundsSource::VendorLut,
                "the row's own provenance travels with it"
            );
        }
        other => panic!("expected VendorChipBand, got {other:?}"),
    }
}

// ── (e) every clause is formatted, and names a live setting ──────────

/// A clause is non-empty, and the row's bound clause carries the
/// bound's DIGITS. Checking the digits and not a literal string is the
/// whole point: the day the constant moves, the clause moves with it.
#[test]
fn every_bound_clause_is_formatted_from_the_value_it_describes() {
    let v = milling_verdict();
    let mut checked = 0_usize;
    for row in v.criteria() {
        let Some(source) = row.bound_source.as_ref() else {
            continue;
        };
        let clause = source.clause();
        assert!(
            !clause.trim().is_empty(),
            "{:?} carries a source with no clause",
            row.kind
        );
        assert!(
            !clause.contains("planning/"),
            "no `planning/…` path in an operator-facing string: {clause}"
        );
        assert!(
            SETTINGS.contains(&source.setting()),
            "{:?} names a setting outside {SETTINGS:?}: {}",
            row.kind,
            source.setting()
        );
        let bound = row
            .bound
            .expect("a source with no bound is a contradiction");
        let full = row.bound_clause();
        assert!(
            full.contains(&digits(bound, 4)),
            "{:?}: the bound clause must carry the bound's digits. \
             bound {bound}, clause: {full}",
            row.kind
        );
        assert!(
            full.contains(row.unit),
            "{:?}: the bound clause must name the unit: {full}",
            row.kind
        );
        checked += 1;
    }
    assert!(
        checked >= 2,
        "at least two rows must carry a source, or this arm is vacuous (checked: {checked})"
    );
}

/// The deflection clause names the budget it was built from, formatted
/// from the constant. This is the one clause a reader is most likely to
/// re-type, so it gets its own arm.
#[test]
fn the_deflection_clause_formats_the_constant_it_names() {
    let clause = BoundSource::DeflectionBudget.clause();
    assert!(
        clause.contains(&digits(EXCEEDS_BOUND_MM, 3)),
        "the clause must format EXCEEDS_BOUND_MM, got: {clause}"
    );
    assert_eq!(BoundSource::DeflectionBudget.setting(), "tool");
}

// ── (f) the gantry row has no bound, because none exists ─────────────

/// S1's row states no reading. It must state no bound either: a bound
/// on a row nothing judged would be an absence rendering as a reading.
#[test]
fn the_gantry_row_carries_no_bound_and_no_source() {
    let v = milling_verdict();
    let row = v
        .criteria()
        .into_iter()
        .find(|r| r.kind == CriterionKind::GantryPush)
        .expect("the gantry row");
    assert_eq!(row.bound, None, "no thrust rating exists to bound it");
    assert_eq!(row.bound_source, None, "and so no source exists either");
}

// ── (g) a drill cycle's rows carry the drill envelope ────────────────

/// The three drill gates judge against their own material-derived
/// envelopes. Each row states the bound that decided it.
#[test]
fn a_drill_cycles_rows_carry_the_drill_envelope() {
    let v = drill_verdict();
    let rows: Vec<_> = v
        .criteria()
        .into_iter()
        .filter(|r| {
            matches!(
                r.kind,
                CriterionKind::DrillChipWelding
                    | CriterionKind::DrillPeckAdequacy
                    | CriterionKind::DrillPlungeFeed
            )
        })
        .collect();
    assert_eq!(rows.len(), 3, "a drill cycle carries three drill rows");
    for row in &rows {
        assert!(
            row.bound.is_some(),
            "{:?} judged a hole and states no bound",
            row.kind
        );
        assert_eq!(
            row.bound_source,
            Some(BoundSource::DrillEnvelope),
            "{:?} judges against the drill envelope",
            row.kind
        );
        assert_eq!(
            row.bound_source.as_ref().map(BoundSource::setting),
            Some("material"),
            "the drill envelopes are material-derived"
        );
    }
}

// ── (h) the owned row is the borrowed row, over the wire ─────────────

/// `criterion_rows()` is the owned shape the MCP wire needs.
/// `CriterionStatus` borrows its verdict, so it cannot serialize; the
/// row copies it out. The two must not drift, so this arm compares them
/// field for field, after a round trip through JSON.
#[test]
fn criterion_rows_round_trip_and_equal_the_borrowed_criteria() {
    let v = milling_verdict();
    let criteria = v.criteria();
    let rows = v.criterion_rows();
    assert_eq!(
        rows.len(),
        criteria.len(),
        "one owned row per borrowed criterion"
    );

    let json = serde_json::to_string(&rows).expect("the owned row serializes");
    // `ExceededCriterion` holds `&'static str` fields, so its
    // `Deserialize` impl needs a `'static` input. A test-lifetime leak
    // of one small JSON string is the cheapest way to exercise the real
    // wire path.
    let leaked: &'static str = Box::leak(json.into_boxed_str());
    let back: Vec<CriterionRow> = serde_json::from_str(leaked).expect("the owned row deserializes");
    assert_eq!(back, rows, "the row must survive its own wire format");

    for (status, row) in criteria.iter().zip(back.iter()) {
        assert_eq!(row.kind, status.kind);
        assert_eq!(row.state, status.state);
        assert_eq!(row.bound, status.bound);
        assert_eq!(row.bound_source, status.bound_source);
        assert_eq!(row.unit, status.unit);
        assert_eq!(row.display_peak, status.display_peak);
        assert_eq!(row.population, status.population);
        assert_eq!(row.unmodeled_reason.as_ref(), status.unmodeled_reason);
        assert_eq!(row.exceeded, status.exceeded);
    }

    // Non-vacuity: the loop above is worthless if every field is `None`.
    assert!(
        back.iter().any(|r| r.bound.is_some()),
        "at least one owned row must carry a bound"
    );
    assert!(
        back.iter().any(|r| r.display_peak.is_some()),
        "at least one owned row must carry a reading"
    );
}
