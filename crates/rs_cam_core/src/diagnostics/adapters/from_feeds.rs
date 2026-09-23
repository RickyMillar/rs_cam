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
use crate::ids::ToolpathId;

/// Calculator-emitted [`FeedsWarning`]s for a given toolpath.
pub fn diagnostics_from_feeds_result(tp_id: ToolpathId, result: &FeedsResult) -> Vec<Diagnostic> {
    result
        .warnings
        .iter()
        .map(|w| feeds_warning_to_diagnostic(tp_id, w))
        .collect()
}

fn feeds_warning_to_diagnostic(tp_id: ToolpathId, w: &FeedsWarning) -> Diagnostic {
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
        FeedsWarning::PowerLadderReducedCut {
            rpm_from,
            rpm_to,
            axial_from,
            axial_to,
            radial_from,
            radial_to,
            feed_factor,
            required_kw_before,
            required_kw_after,
            available_kw,
        } => {
            // Name every dial that moved, and only those. A message that
            // listed a dial the ladder left alone would misreport the cut.
            let mut moved: Vec<String> = Vec::new();
            if let (Some(a), Some(b)) = (rpm_from, rpm_to) {
                moved.push(format!("RPM {a:.0} → {b:.0}"));
            }
            if let (Some(a), Some(b)) = (axial_from, axial_to) {
                moved.push(format!("depth {a:.2} → {b:.2} mm"));
            }
            if let (Some(a), Some(b)) = (radial_from, radial_to) {
                moved.push(format!("width {a:.2} → {b:.2} mm"));
            }
            if let Some(f) = feed_factor {
                moved.push(format!("feed x{f:.3} (last resort)"));
            }
            let cleared = required_kw_after <= available_kw;
            Diagnostic {
                id: DiagnosticId::from(ids::FEEDS_POWER_LADDER_REDUCED_CUT),
                scope: Scope::Toolpath { id: tp_id },
                category: Category::ToolLoad,
                // Caution, not Info: the operator asked for one cut and is
                // being handed a different one. When the ladder ran out of
                // rungs, `PowerLimited` fires alongside at the same level.
                severity: Severity::Caution,
                confidence: Confidence::Approximate,
                state: DiagnosticState::Current,
                source: Source::FeedsCalculator,
                message: format!(
                    "Cut reduced to fit spindle power ({}): {required_kw_before:.2} → \
                     {required_kw_after:.2} kW against {available_kw:.2} kW available{}. {}",
                    moved.join(", "),
                    if cleared { "" } else { " — still over" },
                    // Only the last-resort feed rung changes the chip. Saying
                    // "unchanged" when it fired would misreport the recipe.
                    if feed_factor.is_some() {
                        "Advance per tooth was thinned as a last resort."
                    } else {
                        "Advance per tooth unchanged."
                    }
                ),
                evidence: None,
                fix: None,
                supersedes: vec![],
                suppressed_diagnostics: vec![],
            }
        }
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
        // Ruling R4 WP2a (2026-09-23): the engine warns and does not raise
        // the feed. The message names the consequence and the two levers.
        FeedsWarning::ChiploadBelowRubbingFloor {
            commanded,
            floor,
            source,
            ..
        } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_CHIPLOAD_BELOW_FLOOR),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!(
                "Advance per tooth below the rubbing floor: {commanded:.4} mm/tooth \
                 (floor {floor:.4}, {}). The feed is not raised. The tool can rub and \
                 burn the work: raise the feed or lower the RPM.",
                source.describe()
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        // Ruling R4 WP1 (2026-09-23). Info: the de-rate changes no number
        // that the recipe did not already carry; the finding makes it visible.
        FeedsWarning::LongToolDerate {
            stickout_mm,
            diameter_mm,
            ratio,
            factor,
        } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_LONG_TOOL_DERATE),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Info,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!(
                "Long tool: stickout {stickout_mm:.1} mm is {ratio:.1} x D \
                 (D {diameter_mm:.3} mm); feed x{factor:.2} (repo rule, unsourced)"
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
        // Checkpoint K (a4). A Caution, unlike (a3)'s Info: the operator
        // is being told the recommendation has NO vendor backing at all,
        // and that the post-simulation gate will refuse to judge it.
        FeedsWarning::NoVendorRowsForRoutedOperation {
            operation_kind,
            tool_family,
            missing_rows,
        } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_NO_VENDOR_ROWS_FOR_ROUTED_OPERATION),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!(
                "No vendor data for {operation_kind} on a {tool_family} cutter — this \
                 recommendation is entirely formula-derived and carries no band, and the \
                 post-simulation chipload gate will report Unmodeled(NoVendorData) for the \
                 same reason. Missing: {missing_rows}."
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        // Checkpoint K (a3). Informational, not a Caution: nothing is
        // clamped, derated or unsafe — the recommendation simply has a
        // provenance the operator could not see. Saying "caution" about
        // a healthy RPM anchor would be the louder lie.
        FeedsWarning::VendorRowPublishesNoChipload {
            observation_id,
            formula_chipload_mm,
            floor_band_from,
        } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_VENDOR_ROW_PUBLISHES_NO_CHIPLOAD),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Info,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            message: format!(
                "Vendor row {observation_id} is an RPM anchor and publishes no chipload \
                 column — the recommended {formula_chipload_mm:.4} mm/tooth is the \
                 empirical formula's, not this vendor's, and this recommendation carries \
                 no band. The post-simulation gate resolves a different, chipload-bearing \
                 row, so its verdict is judged against bounds this recipe never saw.{}",
                match floor_band_from {
                    // P1: the floor is the one clamp that now DOES see that
                    // row. Say which, or the operator reads the sentence
                    // above and assumes nothing did.
                    Some(row) => format!(
                        " The rubbing floor was subordinated to that row ({row}) rather \
                         than to the global constant."
                    ),
                    None => String::new(),
                }
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
    tp_id: ToolpathId,
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
    tp_id: ToolpathId,
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
