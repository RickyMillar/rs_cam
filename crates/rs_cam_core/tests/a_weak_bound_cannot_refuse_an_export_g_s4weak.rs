//! **S4 — a bound with no source may not refuse a job.**
//!
//! `enforce_load_policy` refused on any `Exceeds`. That is right while
//! every bound is a measured or published one. It stops being right the
//! moment a row reads against a rule of thumb: `0.20 × D` has no
//! published source in this repository, and refusing an operator's
//! export on a number nobody can cite is how a gate gets switched off
//! wholesale.
//!
//! So the decision moves onto the PROVENANCE, not onto the quantity.
//! [`BoundSource::gates_export`] answers it, and the day a rule of
//! thumb is measured its variant is replaced by a measured one and the
//! row starts gating with no change to any renderer.
//!
//! Two rules this file pins, and the second is the safety one:
//!
//! - an exceedance whose source does not gate is REPORTED as an advisory;
//! - an exceedance with NO source still warns. Absence fails safe.
//!
//! K6 (feeds contract P2, ruling Q6): no row refuses an export now. A row
//! that gated becomes an export warning with its id; the advisory rule
//! above is unchanged.
//!
//! The arms run through the pure decision helpers, with hand-built
//! `CriterionStatus` rows. The helpers are pure, so the rule is testable
//! with no simulation. P29-4 (ruling Q7): the weak source is
//! `MachineForceBudget`, the one bound that does not gate since the rule
//! of thumb `RigidityRuleOfThumb` was deleted.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::diagnostics::ids;
use rs_cam_core::gcode::{exceedance_findings, load_findings, warning_exceedances};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::tool_load::verdict::{
    BoundSource, CriterionKind, CriterionStatus, ExceededCriterion, LoadState, PowerVerdict,
    SampleEvidence, ToolLoadReport, ToolpathLoadVerdict,
};
use rs_cam_core::tool_load::{ChiploadVerdict, Confidence, DeflectionVerdict, UnmodeledReason};

const TP: ToolpathId = ToolpathId::from_raw(4);

/// The weak bound: the machine force-budget depth. The budget stands in
/// for an unmeasured frame, so it does not gate (P29-4, ruling Q7).
fn weak_source() -> BoundSource {
    BoundSource::MachineForceBudget {
        budget_n: 35.0,
        ae_mm: 3.0,
        fz_mm: 0.05,
        depth_mm: 3.0,
    }
}

/// A row that exceeded a weak bound. The KIND is
/// borrowed from a shipped gate on purpose: the rule under test keys on
/// the source, never on the quantity, and this arm proves it by putting
/// a weak source on a kind that normally gates.
fn weak_exceedance() -> CriterionStatus<'static> {
    CriterionStatus {
        group: None,
        kind: CriterionKind::Deflection,
        state: LoadState::Exceeds,
        confidence: None,
        unmodeled_reason: None,
        sample_range: None,
        population: None,
        deepest: None,
        display_peak: Some(4.2),
        unit: "mm",
        bound: Some(3.0),
        bound_source: Some(weak_source()),
        exceeded: Some(ExceededCriterion::deflection()),
    }
}

/// A row that exceeded the machine's own power ceiling.
fn power_exceedance() -> CriterionStatus<'static> {
    CriterionStatus {
        group: None,
        kind: CriterionKind::Power,
        state: LoadState::Exceeds,
        confidence: None,
        unmodeled_reason: None,
        sample_range: None,
        population: None,
        deepest: None,
        display_peak: Some(1.4),
        unit: "kW",
        bound: Some(0.71),
        bound_source: Some(BoundSource::MachinePowerCurve { rpm: 18_000.0 }),
        exceeded: Some(ExceededCriterion::power()),
    }
}

/// A row that exceeded a bound whose provenance nobody stated. This is
/// the fail-safe case, not a hypothetical: every shipped gate carries a
/// source today, so a row without one is a gate that forgot.
fn sourceless_exceedance() -> CriterionStatus<'static> {
    CriterionStatus {
        group: None,
        kind: CriterionKind::Chipload,
        state: LoadState::Exceeds,
        confidence: None,
        unmodeled_reason: None,
        sample_range: None,
        population: None,
        deepest: None,
        display_peak: Some(0.12),
        unit: "mm/tooth",
        bound: Some(0.07),
        bound_source: None,
        exceeded: Some(ExceededCriterion::chipload_breakage()),
    }
}

/// A plain `Within` row, so no arm below passes on an empty list.
fn within_row() -> CriterionStatus<'static> {
    CriterionStatus {
        group: None,
        kind: CriterionKind::Power,
        state: LoadState::Within,
        confidence: None,
        unmodeled_reason: None,
        sample_range: None,
        population: None,
        deepest: None,
        display_peak: Some(0.4),
        unit: "kW",
        bound: Some(0.71),
        bound_source: Some(BoundSource::MachinePowerCurve { rpm: 18_000.0 }),
        exceeded: None,
    }
}

// ── (a) the filter itself ────────────────────────────────────────────

/// A weak bound does not warn. A machine ceiling does. A bound
/// with no source does, because absence must fail safe.
#[test]
fn only_a_bound_with_a_gating_source_warns() {
    let criteria = vec![
        within_row(),
        weak_exceedance(),
        power_exceedance(),
        sourceless_exceedance(),
    ];
    let warning = warning_exceedances(&criteria);
    let kinds: Vec<CriterionKind> = warning.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        vec![CriterionKind::Power, CriterionKind::Chipload],
        "the power ceiling and the sourceless bound warn; the weak bound does not"
    );
    // Non-vacuity in both directions: the list held four rows, one of
    // which is not an exceedance at all.
    assert_eq!(criteria.len(), 4);
    assert!(
        criteria.iter().any(|s| s.state == LoadState::Within),
        "the fixture must hold a non-exceeding row"
    );
}

/// The predicate on the row says the same thing as the filter, so a
/// renderer and the gate cannot disagree about which rows are hard.
#[test]
fn the_row_predicate_agrees_with_the_filter() {
    assert!(!weak_exceedance().warns_on_export());
    assert!(power_exceedance().warns_on_export());
    assert!(sourceless_exceedance().warns_on_export());
    assert!(
        !within_row().warns_on_export(),
        "a Within row warns on nothing"
    );
}

/// `gates_export` is false for exactly one variant today: the machine
/// force budget.
#[test]
fn only_the_machine_bounds_decline_to_gate() {
    assert!(!weak_source().gates_export());
    for source in [
        BoundSource::MachinePowerCurve { rpm: 18_000.0 },
        BoundSource::DeflectionBudget,
        // P17: the radial stock to leave of a rough op is a stated bound.
        BoundSource::StockToLeave { mm: 0.5 },
        BoundSource::DrillEnvelope,
    ] {
        assert!(
            source.gates_export(),
            "{source:?} is a stated bound and must keep gating"
        );
    }
}

// ── (b) the decision, and what the operator reads ────────────────────

/// A job whose only exceedance is weak warns on nothing, and the note
/// names the row. Reported as an advisory is the whole ruling.
#[test]
fn a_weak_exceedance_alone_exports_and_is_still_reported() {
    let per = vec![(TP, vec![within_row(), weak_exceedance()])];
    let found = exceedance_findings(&per);
    assert!(
        found.rows.is_empty(),
        "a weak bound must not warn: {found:?}"
    );
    let note = found
        .outcome
        .advisory_note()
        .expect("the operator must still be told");
    assert!(
        note.contains("toolpath 4"),
        "the note names the toolpath: {note}"
    );
    assert!(
        note.contains("deflection"),
        "the note names the row: {note}"
    );
    assert!(
        note.contains("4.2000"),
        "the note carries the reading: {note}"
    );
    assert!(
        note.contains("3.0000"),
        "the note carries the bound it read against: {note}"
    );
    assert!(
        note.contains("unmeasured frame"),
        "the note says why it did not warn: {note}"
    );
}

/// Add a power exceedance and the export warns on it (K6: no refusal).
/// The warning names what gated, and the advisory names what exceeded
/// without gating, so the operator sees both rather than one.
#[test]
fn a_hard_exceedance_warns_and_names_both() {
    let per = vec![(TP, vec![weak_exceedance(), power_exceedance()])];
    let found = exceedance_findings(&per);
    let row = found
        .rows
        .iter()
        .find(|row| row.id == ids::LOAD_POWER_EXCEEDS)
        .expect("a power exceedance must warn");
    assert_eq!(found.rows.len(), 1, "only the power row gates: {found:?}");
    assert!(
        row.detail.contains("power"),
        "the warning names what gated: {}",
        row.detail
    );
    assert!(
        row.detail.contains("spindle power"),
        "the warning keeps the reason label: {}",
        row.detail
    );
    let note = found
        .outcome
        .advisory_note()
        .expect("the weak row is still reported");
    assert!(
        note.contains("deflection"),
        "the advisory names what exceeded without gating: {note}"
    );
    assert!(
        note.contains("unmeasured frame"),
        "and says why that one did not warn: {note}"
    );
}

/// A clean list produces neither a warning nor a note.
#[test]
fn nothing_exceeded_says_nothing() {
    let per = vec![(TP, vec![within_row()])];
    let found = exceedance_findings(&per);
    assert!(found.rows.is_empty(), "{found:?}");
    assert!(found.outcome.is_empty(), "silence is the right answer here");
}

// ── (c) the shipped gate still warns on the gates that gate ──────────

/// The anchor. Every arm above runs on hand-built rows; this one runs
/// the shipped `load_findings` on a real report, so the file cannot pass
/// while the export has stopped warning.
#[test]
fn the_shipped_gate_still_warns_on_a_deflection_exceedance() {
    let report = ToolLoadReport {
        per_toolpath: vec![ToolpathLoadVerdict {
            toolpath_id: TP,
            groups: rs_cam_core::tool_load::verdict::GroupVerdicts::headline_only_for_tests(
                rs_cam_core::limits::CutGroup::Body,
            ),
            chipload: ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::NoVendorData,
            },
            power: PowerVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            deflection: DeflectionVerdict::Exceeds {
                peak_mm: 0.4,
                bounds: rs_cam_core::tool_load::verdict::DeflectionBounds {
                    validated_within_mm: 0.050,
                    exceeds_mm: 0.200,
                    basis: rs_cam_core::tool_load::verdict::DeflectionBasis::Target,
                },
                evidence: SampleEvidence::at(0),
                confidence: Confidence::Validated,
            },
            // S3: this fixture supplies no trace, so the depth row is
            // unmodelled beside the gates above it.
            depth: rs_cam_core::tool_load::verdict::DepthVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
            material_split: None,
            cut_force: None,
        }],
    };
    let found = load_findings(&report);
    let row = found
        .rows
        .iter()
        .find(|row| row.id == ids::LOAD_DEFLECTION_EXCEEDS)
        .expect("the deflection budget is a stated bound and must still warn");
    let msg = &row.detail;
    assert!(msg.contains("toolpath 4"), "{msg}");
    assert!(msg.contains("deflection"), "{msg}");
}
