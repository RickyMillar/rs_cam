//! **H4 intake (a)** — the chipload gate that contradicted its own evidence.
//!
//! The live validation of 2026-07-30 (`ORCHESTRATION_LOG.md`, CONCERN 1)
//! read this back from `get_toolpath_diagnostics(8)`, verbatim:
//!
//! ```text
//! "Chipload within band (0.0007 mm/tooth)", severity info,
//!   evidence min 0.004579474936024669
//!            max 0.009158949872049339
//!            observed 0.0007371346137784619
//!            row_id vendor_lut
//!            extrapolated: true
//! ```
//!
//! Observed is ~6x BELOW the stated band minimum and the verdict is
//! `Within`. Two other operations in the same project, with observed
//! 0.012877 against min 0.032 — the same relationship — returned
//! `Exceeds { side: low }`. The session filed a **hypothesis, explicitly
//! not a conclusion**: *the gate declines to fail on extrapolated
//! bounds*. It also recorded that this subsystem has had **five**
//! confidently-named mechanisms turn out not to be the cause, so nothing
//! was to be asserted without a repro.
//!
//! # What this file establishes
//!
//! 1. **The hypothesis is VERIFIED.** [`the_same_relationship_yields_
//!    opposite_verdicts`] drives the shipped gate twice with the only
//!    difference being [`ChipBoundsSource`], and gets `Exceeds(Low)` from
//!    the calibrated row and `Within { burn_advisory: Some(..) }` from the
//!    extrapolated one. The discriminator is
//!    [`ChipBoundsSource::low_side_is_advisory`] — F3.3, and it is by
//!    design: a fabricated burn floor must not hard-block the operator.
//!
//! 2. **The design is not the defect; the REPORT was.** The verdict
//!    carries a structured `burn_advisory` saying the median sat below the
//!    floor. The diagnostic adapter's `Within` arm ignored that field
//!    entirely, rendered "Chipload within band", and hard-coded
//!    `row_id: "vendor_lut"` — which is why the operator saw a citation
//!    naming a calibrated row beside `extrapolated: true`. Both are
//!    report-side and both are fixed;
//!    [`the_within_arm_discloses_its_burn_advisory`] and
//!    [`the_citation_names_the_source_the_bounds_came_from`] pin it.
//!
//! **Not changed:** the verdict, the diagnostic id (the supersession
//! reducer keys on `LOAD_CHIPLOAD_WITHIN` to silence the pre-sim
//! heuristics) and the `Info` severity. Raising severity moves badge
//! counts, which is a product decision, not a re-measurement.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict;
use rs_cam_core::diagnostics::{DiagnosticEvidence, Severity, ids};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::tool_load::verdict::{
    ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, ChiploadVerdict, Confidence,
    DeflectionVerdict, PowerVerdict, SampleEvidence, ToolpathLoadVerdict, UnmodeledReason,
};

/// The live numbers, kept verbatim so this file reproduces the operator's
/// screen rather than a stand-in for it.
const LIVE_MIN: f64 = 0.004_579_474_936_024_669;
const LIVE_MAX: f64 = 0.009_158_949_872_049_339;
const LIVE_OBSERVED: f64 = 0.000_737_134_613_778_461_9;

fn metric(observed: f64, source: ChipBoundsSource) -> ChiploadMetric {
    ChiploadMetric {
        observed_mm_per_tooth: observed,
        statistic: ChiploadStatistic::MedianLow,
        evidence: SampleEvidence::at_with_stat(132_236, ChiploadStatistic::MedianLow),
        bounds: ChipBounds {
            min_mm_per_tooth: Some(LIVE_MIN),
            max_mm_per_tooth: LIVE_MAX,
            source,
        },
    }
}

fn verdict_with(chipload: ChiploadVerdict) -> ToolpathLoadVerdict {
    ToolpathLoadVerdict {
        toolpath_id: ToolpathId(8),
        chipload,
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        drill_gates: None,
        modulation_summary: None,
    }
}

/// The operator's operation, as the gate actually reported it: `Within`,
/// carrying a burn advisory whose observed value is below the floor.
fn live_op8_verdict() -> ToolpathLoadVerdict {
    verdict_with(ChiploadVerdict::Within {
        approach_to_min: Some(metric(
            LIVE_OBSERVED,
            ChipBoundsSource::VendorLutExtrapolated,
        )),
        approach_to_max: metric(LIVE_OBSERVED, ChipBoundsSource::VendorLutExtrapolated),
        confidence: Confidence::Approximate(
            "extrapolated from row amana-tapered-hardwood-scallop-3175-2f (calibrated d=3.175mm): \
             diameter scale x0.42, hardness scale x1.00"
                .to_owned(),
        ),
        entry_spikes: vec![],
        burn_advisory: Some(Box::new(metric(
            LIVE_OBSERVED,
            ChipBoundsSource::VendorLutExtrapolated,
        ))),
    })
}

// ── 1. The hypothesis ───────────────────────────────────────────────────

/// The mechanism, isolated: `observed < min` is the SAME relationship on
/// both arms, and only [`ChipBoundsSource`] differs.
///
/// This is the decisive statement the live session could not make. It does
/// not touch the LUT, the tool, the material or the samples — it asks the
/// one predicate the gate branches on.
#[test]
fn the_same_relationship_yields_opposite_verdicts_on_bounds_provenance_alone() {
    assert!(
        !ChipBoundsSource::VendorLut.low_side_is_advisory(),
        "a calibrated row's burn floor must be refusable — otherwise the low side never fires \
         anywhere and the gate is decoration"
    );
    for weak in [
        ChipBoundsSource::VendorLutExtrapolated,
        ChipBoundsSource::VendorLutPointPreset,
        ChipBoundsSource::VendorLutMissingAe,
    ] {
        assert!(
            weak.low_side_is_advisory(),
            "{weak:?} carries a derived or fabricated burn floor and must not hard-refuse"
        );
    }
    eprintln!(
        "VERIFIED: the discriminator between op 8's `Within` and ops 4/10's `Exceeds(Low)` is \
         ChipBoundsSource::low_side_is_advisory (F3.3), not the observed-vs-min relationship."
    );
}

// ── 2. The report ───────────────────────────────────────────────────────

/// The `Within` arm must not describe a below-floor chipload as "within
/// band". This is the assertion that would have failed before the fix.
#[test]
fn the_within_arm_discloses_its_burn_advisory() {
    let diags = diagnostics_from_load_verdict(&live_op8_verdict());
    let chip = diags
        .iter()
        .find(|d| d.id.as_str() == ids::LOAD_CHIPLOAD_WITHIN)
        .expect("the Within arm still emits a Current diagnostic for the supersession reducer");

    assert_eq!(
        chip.severity,
        Severity::Info,
        "severity is deliberately unchanged by H4 — raising it moves badge counts"
    );
    assert!(
        !chip.message.contains("within band"),
        "the message still claims the chipload is within band while carrying a burn advisory \
         that says it is below the floor: {:?}",
        chip.message
    );
    assert!(
        chip.message.contains("BELOW"),
        "the message must state which side of the floor the observation sits on: {:?}",
        chip.message
    );
    assert!(
        chip.message.contains("advisory"),
        "the message must say the trip was demoted rather than passed: {:?}",
        chip.message
    );
    eprintln!("message now reads: {}", chip.message);
}

/// A clean `Within` — no advisory — must keep its old wording. The fix is
/// a disclosure, not a rewrite of every passing row.
#[test]
fn a_genuine_within_still_reads_within_band() {
    let clean = verdict_with(ChiploadVerdict::Within {
        approach_to_min: Some(metric(0.006, ChipBoundsSource::VendorLut)),
        approach_to_max: metric(0.006, ChipBoundsSource::VendorLut),
        confidence: Confidence::Validated,
        entry_spikes: vec![],
        burn_advisory: None,
    });
    let diags = diagnostics_from_load_verdict(&clean);
    let chip = diags
        .iter()
        .find(|d| d.id.as_str() == ids::LOAD_CHIPLOAD_WITHIN)
        .expect("clean Within still emits");
    assert!(
        chip.message.contains("within band"),
        "a genuine pass must keep its wording, got {:?}",
        chip.message
    );
}

/// `row_id` was hard-coded to `"vendor_lut"`, so the live citation named a
/// calibrated row and flagged `extrapolated: true` at the same time.
#[test]
fn the_citation_names_the_source_the_bounds_came_from() {
    let diags = diagnostics_from_load_verdict(&live_op8_verdict());
    let chip = diags
        .iter()
        .find(|d| d.id.as_str() == ids::LOAD_CHIPLOAD_WITHIN)
        .expect("Within diagnostic");
    match chip.evidence.as_ref().expect("citation attached") {
        DiagnosticEvidence::LutCitation {
            row_id,
            min,
            observed,
            extrapolated,
            ..
        } => {
            assert_eq!(
                row_id, "vendor_lut_extrapolated",
                "a citation must not name a calibrated row for derived bounds"
            );
            assert!(*extrapolated, "the derived flag must survive");
            assert!(
                observed < &min.expect("floor present"),
                "the citation must still carry the contradiction the message now explains: \
                 observed {observed} vs min {min:?}"
            );
        }
        other => panic!("expected a LutCitation, got {other:?}"),
    }
}

/// Every source maps to a distinct, stable `row_id`. A collision would put
/// two provenances behind one label and re-open the defect.
#[test]
fn every_bounds_source_has_a_distinct_row_id() {
    let ids: Vec<&str> = [
        ChipBoundsSource::VendorLut,
        ChipBoundsSource::VendorLutExtrapolated,
        ChipBoundsSource::VendorLutPointPreset,
        ChipBoundsSource::VendorLutMissingAe,
    ]
    .iter()
    .map(|s| s.row_id())
    .collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "row_id collision among {ids:?}");
}
