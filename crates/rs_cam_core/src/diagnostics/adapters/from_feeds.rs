//! Adapter: [`crate::feeds::FeedsResult`] → [`Diagnostic`] list.
//!
//! Two flavours:
//!
//! 1. Calculator warnings ([`FeedsWarning`]) — clamping, slotting,
//!    shank-too-large. Already-emitted findings from the feeds
//!    pipeline; mapped to `ToolLoad`/`Safety` diagnostics directly.
//! 2. **Pre-sim heuristic hints** — ratio of operation params to the
//!    calculator's recommendation. These live entirely in this
//!    adapter; they didn't exist as a typed source before. They have
//!    [`crate::diagnostics::Confidence::Heuristic`] and are tagged
//!    for supersession by the
//!    `load.chipload*` / `load.deflection.*` rigorous diagnostics.

use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticEvidence, DiagnosticId, DiagnosticState, Scope,
    Severity, Source, ids,
};
use crate::feeds::{FeedsResult, FeedsWarning};

/// Calculator-emitted [`FeedsWarning`]s for a given toolpath.
pub fn diagnostics_from_feeds_result(tp_id: usize, result: &FeedsResult) -> Vec<Diagnostic> {
    result
        .warnings
        .iter()
        .map(|w| feeds_warning_to_diagnostic(tp_id, w))
        .collect()
}

fn feeds_warning_to_diagnostic(tp_id: usize, w: &FeedsWarning) -> Diagnostic {
    match w {
        FeedsWarning::FeedRateClamped { requested, actual } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_FEED_CLAMPED),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Info,
            confidence: Confidence::Verified,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!(
                "Feed clamped to machine limit: {actual:.0} mm/min (asked {requested:.0})"
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        FeedsWarning::PowerLimited {
            required_kw,
            available_kw,
        } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_POWER_LIMITED),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Caution,
            confidence: Confidence::Approximate,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!(
                "Power limited: {required_kw:.2} kW needed, {available_kw:.2} kW available"
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        FeedsWarning::ShankTooLarge { shank_mm, max_mm } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_SHANK_TOO_LARGE),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::Safety,
            severity: Severity::Critical,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!("Shank {shank_mm:.1} mm exceeds max {max_mm:.1} mm"),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        FeedsWarning::DocExceedsFlute { requested, capped } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_DOC_EXCEEDS_FLUTE),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::Safety,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!(
                "Depth-per-pass {requested:.1} mm capped to flute length {capped:.1} mm"
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        FeedsWarning::SlottingDetected { doc_reduced_to } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_SLOTTING_DETECTED),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Caution,
            confidence: Confidence::Approximate,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!(
                "Slotting detected — depth-per-pass reduced to {doc_reduced_to:.1} mm"
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        FeedsWarning::ScallopInvalid {
            target,
            max_possible,
        } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_SCALLOP_INVALID),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::Quality,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!(
                "Scallop target {target:.3} mm unachievable (max {max_possible:.1} mm)"
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        FeedsWarning::ChiploadClampedToFloor { requested, floor } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_CHIPLOAD_CLAMPED_TO_FLOOR),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!(
                "Chipload clamped to rubbing floor: {requested:.4} → {floor:.4} mm/tooth \
                 (post-derate chipload below chip-formation threshold; \
                 expect honest output above floor instead of ploughing recipe)"
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        FeedsWarning::DrillFeedClampedToEnvelope {
            requested,
            actual,
            envelope_lo,
            envelope_hi,
        } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_DRILL_FEED_CLAMPED_TO_ENVELOPE),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!(
                "Drill feed clamped into material envelope: {requested:.0} → {actual:.0} mm/min \
                 (safe band {envelope_lo:.0}–{envelope_hi:.0} mm/min for this diameter)"
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
    }
}

/// Pre-sim heuristic ratios of the toolpath's commanded values
/// against the feeds-calculator recommendation. Each diagnostic
/// declares itself superseded by the corresponding rigorous load
/// gate — the reducer drops them when sim evidence is current.
///
/// Inputs:
/// - `feed_rate_mm_min` / `stepover` / `dpp` — the toolpath's
///   commanded values (zero / `None` skips the check)
/// - `recommendation` — the calculator output for the same op
pub fn heuristic_hints_from_recommendation(
    tp_id: usize,
    feed_rate_mm_min: f64,
    stepover_mm: Option<f64>,
    dpp_mm: Option<f64>,
    recommendation: &FeedsResult,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let rec_feed = recommendation.feed_rate_mm_min;
    let rec_radial = recommendation.radial_width_mm;
    let rec_axial = recommendation.axial_depth_mm;

    if rec_feed > 0.0 && feed_rate_mm_min > 0.0 {
        let ratio = feed_rate_mm_min / rec_feed;
        if ratio > 2.0 {
            out.push(ratio_hint(
                tp_id,
                ids::FEEDS_FEED_VS_LUT_HIGH,
                format!(
                    "Feed {feed_rate_mm_min:.0} mm/min is {ratio:.1}× recommendation \
                     ({rec_feed:.0}) — tool breakage risk pre-sim"
                ),
                "feed",
                feed_rate_mm_min,
                "recommended",
                rec_feed,
                "mm/min",
            ));
        } else if ratio < 0.2 {
            out.push(ratio_hint(
                tp_id,
                ids::FEEDS_FEED_VS_LUT_LOW,
                format!(
                    "Feed {feed_rate_mm_min:.0} mm/min is {pct:.0}% of recommendation \
                     ({rec_feed:.0}) — rubbing/heat risk pre-sim",
                    pct = ratio * 100.0
                ),
                "feed",
                feed_rate_mm_min,
                "recommended",
                rec_feed,
                "mm/min",
            ));
        }
    }

    if let Some(stepover) = stepover_mm
        && rec_radial > 0.0
        && stepover / rec_radial > 2.0
    {
        out.push(ratio_hint(
            tp_id,
            ids::FEEDS_STEPOVER_VS_LUT,
            format!(
                "Stepover {stepover:.2} mm is {ratio:.1}× recommendation ({rec_radial:.2})",
                ratio = stepover / rec_radial
            ),
            "stepover",
            stepover,
            "recommended",
            rec_radial,
            "mm",
        ));
    }

    if let Some(dpp) = dpp_mm
        && rec_axial > 0.0
        && dpp / rec_axial > 2.0
    {
        out.push(ratio_hint(
            tp_id,
            ids::FEEDS_DPP_VS_LUT,
            format!(
                "Depth/pass {dpp:.2} mm is {ratio:.1}× recommendation ({rec_axial:.2})",
                ratio = dpp / rec_axial
            ),
            "depth_per_pass",
            dpp,
            "recommended",
            rec_axial,
            "mm",
        ));
    }

    out
}

#[allow(clippy::too_many_arguments)]
fn ratio_hint(
    tp_id: usize,
    id: &str,
    message: String,
    lhs_label: &str,
    lhs_value: f64,
    rhs_label: &str,
    rhs_value: f64,
    unit: &str,
) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId::from(id),
        scope: Scope::Toolpath { id: tp_id },
        category: Category::ToolLoad,
        severity: Severity::Hint,
        confidence: Confidence::Heuristic,
        state: DiagnosticState::Current,
        source: Source::FeedsCalculator,
        message,
        evidence: Some(DiagnosticEvidence::GeometryCompare {
            lhs_label: lhs_label.to_owned(),
            lhs_value,
            rhs_label: rhs_label.to_owned(),
            rhs_value,
            unit: unit.to_owned(),
        }),
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }
}
