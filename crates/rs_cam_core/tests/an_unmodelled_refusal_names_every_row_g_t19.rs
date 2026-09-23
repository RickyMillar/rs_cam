//! **T-19 — the export gate's unmodelled half must name every row it
//! refused on.**
//!
//! `enforce_load_policy` had two halves and they read the criterion
//! tier from two different places. The DECISION derived from
//! `ToolpathLoadVerdict::criteria()`, which holds five milling rows
//! since S1 and S3. The MESSAGE enumerated three typed verdicts by
//! hand — chipload, power, deflection.
//!
//! So a toolpath whose only unmodelled row was the depth row refused
//! the export and named no criterion. Measured on HEAD `00b2d9ca`,
//! against the fixture [`depth_only_verdict`] below:
//!
//! ```text
//! G-code export refused: tool load not fully modeled for toolpath(s):
//!   toolpath 4:
//! Pass `accept_unmodeled=true` to acknowledge unmodeled criteria.
//! ```
//!
//! The line after the colon is empty. That is defect class 3 of the
//! programme: an absence rendered as a blank line. The operator is told
//! the job is refused and is handed nothing to act on.
//!
//! What this file pins:
//!
//! - the refusal names EVERY counting row, kind and clause, in
//!   `criteria()` order;
//! - the decision did not move. The gate refuses exactly when
//!   `ToolpathLoadVerdict::any_unmodeled` says a toolpath needs
//!   operator action, and that function is untouched by T-19;
//! - the gantry-push row still refuses nothing and is named nowhere: it
//!   is a known absence (register T-10) and no operator action fills
//!   it;
//! - a drill cycle still exports. Its milling gates do not apply, and a
//!   cycle that ran the drill gates is measured, not unmeasured.
//!
//! What this file deliberately does NOT assert: any bound, any force
//! and any physics. T-19 changes a message and nothing else.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::gcode::{ToolLoadExportPolicy, enforce_load_policy, refusing_unmodeled};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::RigidityDepthCap;
use rs_cam_core::material::Material;
use rs_cam_core::ops::drill::DrillCycle;
use rs_cam_core::ops::drill_metrics::{build_drill_toolpath_summary, emit_drill_samples};
use rs_cam_core::ops::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};
use rs_cam_core::tool_load::verdict::{
    ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, ChiploadVerdict, Confidence,
    CriterionKind, DeflectionBounds, DeflectionVerdict, DepthVerdict, LoadState, PowerVerdict,
    SampleEvidence, ToolLoadReport, ToolpathLoadVerdict, UnmodeledReason,
};

const TP: ToolpathId = ToolpathId(4);

/// The depth gate's own refusal when the tool states no diameter the
/// rigidity cap can multiply. The string is the fixture's, not a
/// constant this file asserts against a model.
const DEPTH_REASON: &str = "no usable tool diameter for the rigidity cap";

// ── fixtures ─────────────────────────────────────────────────────────

fn chipload_within() -> ChiploadVerdict {
    ChiploadVerdict::Within {
        approach_to_min: None,
        approach_to_max: ChiploadMetric {
            observed_mm_per_tooth: 0.05,
            statistic: ChiploadStatistic::PeakInRange,
            evidence: SampleEvidence::empty(),
            bounds: ChipBounds {
                min_mm_per_tooth: Some(0.038),
                max_mm_per_tooth: 0.07,
                source: ChipBoundsSource::VendorLut,
            },
        },
        confidence: Confidence::Validated,
        entry_spikes: Vec::new(),
        burn_advisory: None,
    }
}

fn power_within() -> PowerVerdict {
    PowerVerdict::Within {
        peak_kw: 0.5,
        available_kw: 0.71,
        evidence: SampleEvidence::empty(),
        confidence: Confidence::Validated,
        entry_spike: None,
        bound_source: None,
    }
}

fn deflection_within() -> DeflectionVerdict {
    DeflectionVerdict::Within {
        peak_mm: 0.02,
        bounds: DeflectionBounds {
            validated_within_mm: 0.050,
            exceeds_mm: 0.200,
        },
        evidence: SampleEvidence::empty(),
        confidence: Confidence::Validated,
        entry_spike: None,
    }
}

fn depth_within() -> DepthVerdict {
    DepthVerdict::Within {
        peak_mm: 1.0,
        bound: RigidityDepthCap {
            factor: 0.25,
            diameter_mm: 6.0,
        },
        evidence: SampleEvidence::empty(),
        confidence: Confidence::Validated,
    }
}

fn depth_unmodeled() -> DepthVerdict {
    DepthVerdict::Unmodeled {
        reason: UnmodeledReason::NotImplemented(DEPTH_REASON.to_owned()),
    }
}

fn milling_verdict(chipload: ChiploadVerdict, depth: DepthVerdict) -> ToolpathLoadVerdict {
    ToolpathLoadVerdict {
        toolpath_id: TP,
        chipload,
        power: power_within(),
        deflection: deflection_within(),
        depth,
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
    }
}

/// **The case the old message could not name.** Four milling gates
/// measure, the depth row does not, and the gantry row is the known
/// absence every milling toolpath carries.
fn depth_only_verdict() -> ToolpathLoadVerdict {
    milling_verdict(chipload_within(), depth_unmodeled())
}

/// Every milling gate measures. Only the gantry row is unmodelled.
fn fully_modelled_verdict() -> ToolpathLoadVerdict {
    milling_verdict(chipload_within(), depth_within())
}

/// Two counting rows, in `criteria()` order: chipload first, depth
/// fourth.
fn chipload_and_depth_verdict() -> ToolpathLoadVerdict {
    milling_verdict(
        ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        depth_unmodeled(),
    )
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

/// A drill cycle through the shipped drill path: every milling gate
/// short-circuits to `NotApplicableForOp`, and the three drill gates
/// measure the cycle.
fn drill_verdict() -> ToolpathLoadVerdict {
    let d = drill_op();
    let samples = emit_drill_samples(TP, &d);
    let summary = build_drill_toolpath_summary(TP, &d, &samples);
    let gates = rs_cam_core::tool_load::drill_gates::evaluate(&d, &summary);
    let not_applicable =
        || UnmodeledReason::NotApplicableForOp("drill cycle — no continuous engagement".to_owned());
    ToolpathLoadVerdict {
        toolpath_id: TP,
        chipload: ChiploadVerdict::Unmodeled {
            reason: not_applicable(),
        },
        power: PowerVerdict::Unmodeled {
            reason: not_applicable(),
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: not_applicable(),
        },
        depth: DepthVerdict::Unmodeled {
            reason: not_applicable(),
        },
        drill_gates: Some(gates),
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
    }
}

fn refuse(verdict: ToolpathLoadVerdict) -> String {
    let report = ToolLoadReport {
        per_toolpath: vec![verdict],
    };
    enforce_load_policy(&report, &ToolLoadExportPolicy::default())
        .expect_err("an unmodelled row must refuse under the default policy")
        .to_string()
}

// ── (a) the row the old message could not name ───────────────────────

/// A toolpath whose ONLY counting row is the depth row refuses, and the
/// refusal names the row and says why it could not be modelled. On HEAD
/// this printed `toolpath 4: ` and stopped.
#[test]
fn a_depth_only_refusal_names_the_depth_row_and_its_reason() {
    let v = depth_only_verdict();
    // Non-vacuity: exactly one row counts, and it is the depth row.
    let criteria = v.criteria();
    let refusing = refusing_unmodeled(&criteria);
    let kinds: Vec<CriterionKind> = refusing.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        vec![CriterionKind::DepthOfCut],
        "the depth row is the only counting row in this fixture"
    );

    let msg = refuse(v);
    assert!(msg.contains("toolpath 4"), "names the toolpath: {msg}");
    assert!(
        msg.contains("depth of cut"),
        "names the criterion the gate refused on: {msg}"
    );
    assert!(
        msg.contains(DEPTH_REASON),
        "names why the row could not be modelled: {msg}"
    );
    assert!(
        !msg.contains("toolpath 4: \n"),
        "a refusal must not render the criterion list as a blank line: {msg}"
    );
    assert!(
        !msg.contains("NotImplemented"),
        "the operator reads a clause, never a Rust variant name: {msg}"
    );
}

/// The first line and the override hint are the ones the gate has
/// always printed. T-19 changes what sits between them and nothing
/// else.
#[test]
fn the_refusal_keeps_its_headline_and_its_override_hint() {
    let msg = refuse(depth_only_verdict());
    assert!(
        msg.starts_with("G-code export refused: tool load not fully modeled for toolpath(s):\n"),
        "headline unchanged: {msg}"
    );
    assert!(
        msg.ends_with("Pass `accept_unmodeled=true` to acknowledge unmodeled criteria."),
        "override hint unchanged: {msg}"
    );
}

/// A stale trace is a different operator action from a missing one,
/// and the headline still says which. T-19 moved the stale test off
/// the three typed verdicts and onto the criterion rows; only the
/// rewrite in `project_load_report` writes
/// [`UnmodeledReason::StaleSimulation`], and it writes it onto rows
/// that are in this list, so the answer is the one the gate gave
/// before.
#[test]
fn a_stale_trace_keeps_its_own_headline_and_names_its_row() {
    let v = milling_verdict(
        ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::StaleSimulation,
        },
        depth_within(),
    );
    let msg = refuse(v);
    assert!(
        msg.starts_with("G-code export refused: cached simulation is stale"),
        "a stale trace reads as stale, not as never simulated: {msg}"
    );
    assert!(msg.contains("chipload"), "names the stale row: {msg}");
    assert!(
        msg.ends_with("export against the stale evidence anyway."),
        "the stale override hint is unchanged: {msg}"
    );
}

// ── (b) every counting row, in criterion order ───────────────────────

/// Two unmodelled rows, two named rows, in the order `criteria()` lists
/// them. A message that names one of two is the same defect wearing a
/// smaller hat.
#[test]
fn a_refusal_names_every_counting_row_in_criterion_order() {
    let v = chipload_and_depth_verdict();
    let criteria = v.criteria();
    let kinds: Vec<CriterionKind> = refusing_unmodeled(&criteria)
        .iter()
        .map(|s| s.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![CriterionKind::Chipload, CriterionKind::DepthOfCut],
        "both rows count, chipload first"
    );

    let msg = refuse(v);
    let chipload_at = msg.find("chipload").expect("the chipload row is named");
    let depth_at = msg.find("depth of cut").expect("the depth row is named");
    assert!(
        chipload_at < depth_at,
        "the message follows `criteria()` order: {msg}"
    );
    assert!(
        msg.contains("simulation has not been run"),
        "the chipload row states its own reason: {msg}"
    );
    assert!(
        msg.contains(DEPTH_REASON),
        "the depth row states its own reason: {msg}"
    );
}

// ── (c) a known absence refuses nothing and is named nowhere ─────────

/// The gantry-push row is `Unmodeled` on every milling toolpath and no
/// operator action fills it (register T-10). It must not refuse an
/// export, and it must not appear in a refusal another row caused.
#[test]
fn the_gantry_row_alone_refuses_nothing_and_is_never_named() {
    let v = fully_modelled_verdict();
    let criteria = v.criteria();
    // Non-vacuity: the row IS in the tier, and it IS unmodelled.
    let gantry = criteria
        .iter()
        .find(|s| s.kind == CriterionKind::GantryPush)
        .expect("a milling toolpath carries the gantry row");
    assert_eq!(gantry.state, LoadState::Unmodeled);
    assert!(gantry.is_known_absence());
    assert!(
        refusing_unmodeled(&criteria).is_empty(),
        "a known absence is not a row an operator can act on"
    );
    let report = ToolLoadReport {
        per_toolpath: vec![fully_modelled_verdict()],
    };
    let outcome = enforce_load_policy(&report, &ToolLoadExportPolicy::default());
    assert!(
        outcome.is_ok(),
        "a known absence must not refuse an export: {}",
        outcome.unwrap_err()
    );

    // And when another row refuses, the known absence stays out of the
    // message.
    let msg = refuse(depth_only_verdict());
    assert!(
        !msg.contains("gantry"),
        "a refusal must not name a row no action can fill: {msg}"
    );
}

// ── (d) a drill cycle is measured, not unmeasured ────────────────────

/// A drill cycle that ran the drill gates exports. Its milling gates do
/// not apply, and "does not apply" is not "could not measure".
#[test]
fn a_drill_cycle_with_its_gates_still_exports() {
    let v = drill_verdict();
    // Fixture guard: every milling row IS unmodelled, so the arm below
    // tests the partition and not an empty list.
    assert_eq!(
        v.criteria()
            .iter()
            .filter(|s| s.state == LoadState::Unmodeled)
            .count(),
        5,
        "all five milling rows read `NotApplicableForOp` on a drill cycle"
    );
    assert!(
        refusing_unmodeled(&v.criteria()).is_empty(),
        "a drill cycle with measured drill gates needs no operator action"
    );
    let report = ToolLoadReport {
        per_toolpath: vec![drill_verdict()],
    };
    let outcome = enforce_load_policy(&report, &ToolLoadExportPolicy::default());
    assert!(
        outcome.is_ok(),
        "a drill cycle must still export: {}",
        outcome.unwrap_err()
    );
}

/// The optimizer path leaves `drill_gates` `None` on a drill toolpath.
/// Nothing measured anything, so the gate refuses — exactly as it did
/// before T-19.
#[test]
fn a_drill_cycle_without_its_gates_still_refuses() {
    let mut v = drill_verdict();
    v.drill_gates = None;
    assert_eq!(v.modeled_count(), 0, "fixture guard: nothing is modelled");
    assert!(
        !refusing_unmodeled(&v.criteria()).is_empty(),
        "a toolpath that measured nothing must refuse"
    );
    let msg = refuse(v);
    assert!(
        msg.contains("not applicable to this operation"),
        "the refusal states what each row said: {msg}"
    );
}

// ── (e) the override still overrides ─────────────────────────────────

/// `accept_unmodeled` accepts the same rows it always did. The
/// override is about acknowledgement, not about a different decision.
#[test]
fn the_override_accepts_the_rows_the_gate_would_name() {
    let report = ToolLoadReport {
        per_toolpath: vec![chipload_and_depth_verdict()],
    };
    let accepted = ToolLoadExportPolicy {
        accept_unmodeled: true,
        accept_exceeded: false,
    };
    assert!(
        enforce_load_policy(&report, &accepted).is_ok(),
        "the acknowledged override must still export"
    );
    // The rows did not go away; only the refusal did.
    assert_eq!(
        refusing_unmodeled(&report.per_toolpath[0].criteria()).len(),
        2,
        "the override hides no row from the helper"
    );
}

// ── (f) the decision did not move ────────────────────────────────────

/// **The equivalence T-19 had to preserve.** `any_unmodeled` is the
/// decision source the old gate read, and T-19 did not touch it. The
/// new helper must refuse on exactly the toolpaths it names — for the
/// milling shapes, for the known absence, and for both drill shapes.
#[test]
fn the_helper_refuses_exactly_where_any_unmodeled_says_it_must() {
    let mut drill_without_gates = drill_verdict();
    drill_without_gates.drill_gates = None;
    let fixtures = [
        ("depth only", depth_only_verdict()),
        ("fully modelled", fully_modelled_verdict()),
        ("chipload and depth", chipload_and_depth_verdict()),
        ("drill with gates", drill_verdict()),
        ("drill without gates", drill_without_gates),
    ];
    // Non-vacuity: the table must hold both answers, or the assertion
    // below passes on a constant.
    assert!(fixtures.iter().any(|(_, v)| v.any_unmodeled()));
    assert!(fixtures.iter().any(|(_, v)| !v.any_unmodeled()));
    for (name, v) in &fixtures {
        let criteria = v.criteria();
        assert_eq!(
            !refusing_unmodeled(&criteria).is_empty(),
            v.any_unmodeled(),
            "{name}: the message half and the decision half must agree"
        );
    }
}

/// A fully modelled report refuses nothing, and the helper returns an
/// empty list rather than a list this file never looks at.
#[test]
fn a_fully_modelled_report_passes() {
    let report = ToolLoadReport {
        per_toolpath: vec![fully_modelled_verdict()],
    };
    assert!(
        enforce_load_policy(&report, &ToolLoadExportPolicy::default()).is_ok(),
        "nothing in this fixture refuses"
    );
    assert!(refusing_unmodeled(&report.per_toolpath[0].criteria()).is_empty());
    assert_eq!(
        report.per_toolpath[0].modeled_count(),
        4,
        "fixture guard: four milling gates measured"
    );
}
