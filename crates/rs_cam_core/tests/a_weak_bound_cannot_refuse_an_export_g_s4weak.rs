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
//! - an exceedance whose source does not gate is REPORTED, not refused;
//! - an exceedance with NO source still refuses. Absence fails safe.
//!
//! The arms run through the pure decision helpers, with hand-built
//! `CriterionStatus` rows. That is deliberate: `RigidityRuleOfThumb`
//! has no producer until S3 lands the depth-of-cut gate, so a report
//! built from the shipped gates cannot reach the weak arm yet. The
//! helpers are pure, so the rule is testable the day it is written
//! rather than the day its first producer arrives.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::gcode::{
    ToolLoadExportPolicy, enforce_exceedance_policy, enforce_load_policy, refusing_exceedances,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::tool_load::verdict::{
    BoundSource, CriterionKind, CriterionStatus, ExceededCriterion, LoadState, PowerVerdict,
    SampleEvidence, ToolLoadReport, ToolpathLoadVerdict,
};
use rs_cam_core::tool_load::{ChiploadVerdict, Confidence, DeflectionVerdict, UnmodeledReason};

const TP: ToolpathId = ToolpathId(4);

/// The rule-of-thumb bound S3 will produce: the machine rigidity factor
/// times the tool diameter. Nothing publishes it.
fn weak_source() -> BoundSource {
    BoundSource::RigidityRuleOfThumb {
        factor: 0.25,
        diameter_mm: 12.0,
    }
}

/// A row that exceeded a bound built from a rule of thumb. The KIND is
/// borrowed from a shipped gate on purpose: the rule under test keys on
/// the source, never on the quantity, and this arm proves it by putting
/// a weak source on a kind that normally gates.
fn weak_exceedance() -> CriterionStatus<'static> {
    CriterionStatus {
        kind: CriterionKind::Deflection,
        state: LoadState::Exceeds,
        confidence: None,
        unmodeled_reason: None,
        sample_range: None,
        population: None,
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
        kind: CriterionKind::Power,
        state: LoadState::Exceeds,
        confidence: None,
        unmodeled_reason: None,
        sample_range: None,
        population: None,
        display_peak: Some(1.4),
        unit: "kW",
        bound: Some(0.71),
        bound_source: Some(BoundSource::MachinePowerCurve {
            rpm: 18_000.0,
            safety_factor: 0.8,
        }),
        exceeded: Some(ExceededCriterion::power()),
    }
}

/// A row that exceeded a bound whose provenance nobody stated. This is
/// the fail-safe case, not a hypothetical: every shipped gate carries a
/// source today, so a row without one is a gate that forgot.
fn sourceless_exceedance() -> CriterionStatus<'static> {
    CriterionStatus {
        kind: CriterionKind::Chipload,
        state: LoadState::Exceeds,
        confidence: None,
        unmodeled_reason: None,
        sample_range: None,
        population: None,
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
        kind: CriterionKind::Power,
        state: LoadState::Within,
        confidence: None,
        unmodeled_reason: None,
        sample_range: None,
        population: None,
        display_peak: Some(0.4),
        unit: "kW",
        bound: Some(0.71),
        bound_source: Some(BoundSource::MachinePowerCurve {
            rpm: 18_000.0,
            safety_factor: 0.8,
        }),
        exceeded: None,
    }
}

// ── (a) the filter itself ────────────────────────────────────────────

/// A rule of thumb does not refuse. A machine ceiling does. A bound
/// with no source does, because absence must fail safe.
#[test]
fn only_a_bound_with_a_gating_source_refuses() {
    let criteria = vec![
        within_row(),
        weak_exceedance(),
        power_exceedance(),
        sourceless_exceedance(),
    ];
    let refusing = refusing_exceedances(&criteria);
    let kinds: Vec<CriterionKind> = refusing.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        vec![CriterionKind::Power, CriterionKind::Chipload],
        "the power ceiling and the sourceless bound refuse; the rule of thumb does not"
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
    assert!(!weak_exceedance().refuses_export());
    assert!(power_exceedance().refuses_export());
    assert!(sourceless_exceedance().refuses_export());
    assert!(
        !within_row().refuses_export(),
        "a Within row refuses nothing"
    );
}

/// `gates_export` is false for exactly one variant today.
#[test]
fn only_the_rule_of_thumb_declines_to_gate() {
    assert!(!weak_source().gates_export());
    for source in [
        BoundSource::MachinePowerCurve {
            rpm: 18_000.0,
            safety_factor: 0.8,
        },
        BoundSource::DeflectionBudget,
        BoundSource::DrillEnvelope,
    ] {
        assert!(
            source.gates_export(),
            "{source:?} is a stated bound and must keep gating"
        );
    }
}

// ── (b) the decision, and what the operator reads ────────────────────

/// A job whose only exceedance is weak exports, and the note names the
/// row. Reported, not refused, is the whole ruling.
#[test]
fn a_weak_exceedance_alone_exports_and_is_still_reported() {
    let per = vec![(TP, vec![within_row(), weak_exceedance()])];
    let note = enforce_exceedance_policy(&per, false).expect("a weak bound must not refuse");
    let note = note.expect("the operator must still be told");
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
        note.contains("rule of thumb"),
        "the note says why it did not refuse: {note}"
    );
}

/// Add a power exceedance and the export is refused. The message names
/// what refused AND what exceeded without refusing, so the operator
/// sees both rather than one.
#[test]
fn a_hard_exceedance_refuses_and_the_message_names_both() {
    let per = vec![(TP, vec![weak_exceedance(), power_exceedance()])];
    let err = enforce_exceedance_policy(&per, false).expect_err("a power exceedance must refuse");
    let msg = err.to_string();
    assert!(msg.contains("refused"), "it is a refusal: {msg}");
    assert!(
        msg.contains("power"),
        "the message names what refused: {msg}"
    );
    assert!(
        msg.contains("spindle power"),
        "the message keeps the reason label: {msg}"
    );
    assert!(
        msg.contains("deflection"),
        "the message also names what exceeded without refusing: {msg}"
    );
    assert!(
        msg.contains("rule of thumb"),
        "and says why that one did not refuse: {msg}"
    );
}

/// With the override on, nothing refuses, and the weak row is still
/// reported. The override is about danger, not about silence.
#[test]
fn the_override_never_hides_the_advisory() {
    let per = vec![(TP, vec![weak_exceedance(), power_exceedance()])];
    let note = enforce_exceedance_policy(&per, true).expect("the override accepts the exceedance");
    let note = note.expect("the weak row is still reported");
    assert!(note.contains("deflection"), "{note}");
}

/// A clean list produces neither a refusal nor a note.
#[test]
fn nothing_exceeded_says_nothing() {
    let per = vec![(TP, vec![within_row()])];
    let note = enforce_exceedance_policy(&per, false).expect("a clean list exports");
    assert_eq!(note, None, "silence is the right answer here");
}

// ── (c) the shipped gate still refuses on the gates that gate ────────

/// The anchor. Every arm above runs on hand-built rows; this one runs
/// the shipped `enforce_load_policy` on a real report, so the file
/// cannot pass while the export gate has stopped refusing.
#[test]
fn the_shipped_gate_still_refuses_a_deflection_exceedance() {
    let report = ToolLoadReport {
        per_toolpath: vec![ToolpathLoadVerdict {
            toolpath_id: TP,
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
                },
                evidence: SampleEvidence::at(0),
                confidence: Confidence::Validated,
            },
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
        }],
    };
    let strict = ToolLoadExportPolicy {
        accept_unmodeled: true,
        accept_exceeded: false,
    };
    let err = enforce_load_policy(&report, &strict)
        .expect_err("the deflection budget is a stated bound and must still refuse");
    let msg = err.to_string();
    assert!(msg.contains("toolpath 4"), "{msg}");
    assert!(msg.contains("deflection"), "{msg}");
}
