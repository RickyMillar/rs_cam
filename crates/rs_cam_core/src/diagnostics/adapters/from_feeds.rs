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
use crate::feeds::suggest::SuggestWarning;
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

/// The diagnostic of one Suggest record, for the records that carry a rule
/// id. `None` for every other [`SuggestWarning`]: those are rationale rows.
///
/// D9 (2026-11-04) removed the machine aggressiveness dial and its record;
/// `project.outside_envelope` is the one record with a rule id.
#[must_use]
pub fn diagnostic_from_suggest_warning(
    tp_id: ToolpathId,
    w: &SuggestWarning,
) -> Option<Diagnostic> {
    if let SuggestWarning::OutsideEnvelope { .. } = w {
        return diagnostic_outside_envelope(tp_id, w);
    }
    None
}

/// CONTRACT K2b, K6 (P5): `project.outside_envelope`. The text names the
/// binding limit and the margin (the rationale row's sentences).
fn diagnostic_outside_envelope(tp_id: ToolpathId, w: &SuggestWarning) -> Option<Diagnostic> {
    let row = crate::feeds::rationale::SuggestRationale::from_warnings(std::slice::from_ref(w))
        .entries
        .into_iter()
        .next()?;
    Some(Diagnostic {
        id: DiagnosticId::from(ids::PROJECT_OUTSIDE_ENVELOPE),
        scope: Scope::Toolpath { id: tp_id },
        category: Category::ToolLoad,
        severity: Severity::Caution,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::FeedsCalculator,
        face: crate::diagnostics::presentation::face(ids::PROJECT_OUTSIDE_ENVELOPE),
        anchor: crate::diagnostics::presentation::anchor(ids::PROJECT_OUTSIDE_ENVELOPE),
        message: row.message(),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    })
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
            face: crate::diagnostics::presentation::face(ids::FEEDS_FEED_CLAMPED),
            anchor: crate::diagnostics::presentation::anchor(ids::FEEDS_FEED_CLAMPED),
            message: format!(
                "The machine allows a feed rate of at most {actual:.0} mm/min. The \
                 calculator asked for {requested:.0} mm/min, so the feed rate is held at \
                 {actual:.0} mm/min."
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
            face: crate::diagnostics::presentation::face(ids::FEEDS_POWER_LIMITED),
            anchor: crate::diagnostics::presentation::anchor(ids::FEEDS_POWER_LIMITED),
            message: format!(
                "The cut needs {required_kw:.2} kW. The spindle gives {available_kw:.2} kW."
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
                moved.push(format!("feed rate to {:.1} % (last resort)", f * 100.0));
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
                face: crate::diagnostics::presentation::face(ids::FEEDS_POWER_LADDER_REDUCED_CUT),
                anchor: crate::diagnostics::presentation::anchor(
                    ids::FEEDS_POWER_LADDER_REDUCED_CUT,
                ),
                message: format!(
                    "The calculator made the cut smaller to fit the spindle power ({}). The \
                     cut needs {required_kw_after:.2} kW, not {required_kw_before:.2} kW; the \
                     spindle gives {available_kw:.2} kW.{} {}",
                    moved.join(", "),
                    if cleared {
                        ""
                    } else {
                        " The cut still needs more power than the spindle gives."
                    },
                    // Only the last-resort feed rung changes the chip. Saying
                    // "unchanged" when it fired would misreport the recipe.
                    if feed_factor.is_some() {
                        "The feed per tooth was made smaller as a last resort."
                    } else {
                        "The feed per tooth does not change."
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
            face: crate::diagnostics::presentation::face(ids::FEEDS_SHANK_TOO_LARGE),
            anchor: crate::diagnostics::presentation::anchor(ids::FEEDS_SHANK_TOO_LARGE),
            message: format!(
                "The tool shank is {shank_mm:.1} mm. The machine takes shanks up to \
                 {max_mm:.1} mm."
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        FeedsWarning::DocExceedsFlute {
            requested_mm,
            guard_mm,
            shipped_mm,
        } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_DOC_EXCEEDS_FLUTE),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::Safety,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            face: crate::diagnostics::presentation::face(ids::FEEDS_DOC_EXCEEDS_FLUTE),
            anchor: crate::diagnostics::presentation::anchor(ids::FEEDS_DOC_EXCEEDS_FLUTE),
            message: if (shipped_mm - requested_mm).abs() < 1e-9 {
                format!(
                    "The depth per pass {requested_mm:.1} mm is past the flute guard \
                     {guard_mm:.1} mm. The speeds are for {shipped_mm:.1} mm, the depth that \
                     cuts."
                )
            } else {
                format!(
                    "The default depth per pass {requested_mm:.1} mm is past the flute guard. \
                     The calculator lowers it to {shipped_mm:.1} mm."
                )
            },
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        FeedsWarning::SlottingDetected {
            requested_mm,
            cap_mm,
            shipped_mm,
        } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_SLOTTING_DETECTED),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Caution,
            confidence: Confidence::Approximate,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            face: crate::diagnostics::presentation::face(ids::FEEDS_SLOTTING_DETECTED),
            anchor: crate::diagnostics::presentation::anchor(ids::FEEDS_SLOTTING_DETECTED),
            message: if (shipped_mm - requested_mm).abs() < 1e-9 {
                format!(
                    "The tool cuts a full-width slot {requested_mm:.1} mm deep, past the slot \
                     cap {cap_mm:.1} mm. The speeds are for {shipped_mm:.1} mm, the depth that \
                     cuts."
                )
            } else {
                format!(
                    "The tool cuts a full-width slot. The calculator lowers the default depth \
                     per pass to {shipped_mm:.1} mm."
                )
            },
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
            face: crate::diagnostics::presentation::face(ids::FEEDS_SCALLOP_INVALID),
            anchor: crate::diagnostics::presentation::anchor(ids::FEEDS_SCALLOP_INVALID),
            message: format!(
                "This tool cannot leave a scallop of {target:.3} mm. The largest scallop it \
                 can give is {max_possible:.1} mm."
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
            face: crate::diagnostics::presentation::face(ids::FEEDS_CHIPLOAD_BELOW_FLOOR),
            anchor: crate::diagnostics::presentation::anchor(ids::FEEDS_CHIPLOAD_BELOW_FLOOR),
            message: format!(
                "The feed per tooth is {commanded:.4} mm, below the rubbing limit of \
                 {floor:.4} mm ({}). The calculator does not raise the feed rate. The tool \
                 can rub and burn the wood: raise the feed rate or lower the spindle speed.",
                source.describe()
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        // Ruling R4 Q10 (2026-09-24). Info: the RPM came down so the
        // chipload holds at the feed ceiling. The Suggest record of the same
        // step (`SuggestWarning::RpmLoweredForFeedCeiling`) is a rationale
        // row only, so the finding is filed once.
        FeedsWarning::RpmLoweredForFeedCeiling {
            rpm_from,
            rpm_to,
            feed_ceiling_mm_min,
            rpm_floor,
            floor_source,
            held,
            arms,
        } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_RPM_LOWERED_FOR_CEILING),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Info,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            face: crate::diagnostics::presentation::face(ids::FEEDS_RPM_LOWERED_FOR_CEILING),
            anchor: crate::diagnostics::presentation::anchor(ids::FEEDS_RPM_LOWERED_FOR_CEILING),
            message: format!(
                "{}.{}",
                crate::feeds::rpm_lowered_text(
                    *rpm_from,
                    *rpm_to,
                    *feed_ceiling_mm_min,
                    *rpm_floor,
                    *floor_source,
                    *held,
                    arms,
                ),
                match arms.shipped {
                    crate::feeds::CapArm::LowerRpmSameChip =>
                        " The feed rate moves down with the RPM.",
                    crate::feeds::CapArm::SameRpmSmallerChip => "",
                }
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        // Ruling R4 WP1 (2026-09-23). Info. Since ruling R4 Q7 (2026-09-24)
        // the share lowers the dial's load target; the feed does not take it.
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
            face: format!("Long tool: load target {:.0} %", factor * 100.0),
            anchor: crate::diagnostics::presentation::anchor(ids::FEEDS_LONG_TOOL_DERATE),
            message: format!(
                "The tool sticks out {stickout_mm:.1} mm, which is {ratio:.1} \u{00D7} its \
                 diameter ({diameter_mm:.3} mm). The calculator lowers the load target to \
                 {pct:.0} % to limit deflection. This limit has no published source. The \
                 feed rate does not change.",
                pct = factor * 100.0
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
            face: crate::diagnostics::presentation::face(ids::FEEDS_DRILL_FEED_CLAMPED_TO_ENVELOPE),
            anchor: crate::diagnostics::presentation::anchor(
                ids::FEEDS_DRILL_FEED_CLAMPED_TO_ENVELOPE,
            ),
            message: format!(
                "The drill feed {requested:.0} mm/min is outside the safe range for this \
                 diameter and material ({envelope_lo:.0}–{envelope_hi:.0} mm/min). The \
                 calculator sets {actual:.0} mm/min."
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
            face: crate::diagnostics::presentation::face(
                ids::FEEDS_NO_VENDOR_ROWS_FOR_ROUTED_OPERATION,
            ),
            anchor: crate::diagnostics::presentation::anchor(
                ids::FEEDS_NO_VENDOR_ROWS_FOR_ROUTED_OPERATION,
            ),
            message: format!(
                "No vendor data for {operation_kind} with a {tool_family} cutter. The feeds \
                 come from a formula only and have no safe range. For the same reason, the \
                 chipload check after simulation cannot judge this cut. Missing: \
                 {missing_rows}."
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        // P13 item 4 (law B): a Caution, like (a4). The chip has no printed
        // row behind it; the operator sees that before the job runs.
        FeedsWarning::FormulaChipNoPrintedRow {
            formula_chipload_mm,
        } => Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_FORMULA_CHIP_NO_PRINTED_ROW),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::FeedsCalculator,
            face: crate::diagnostics::presentation::face(ids::FEEDS_FORMULA_CHIP_NO_PRINTED_ROW),
            anchor: crate::diagnostics::presentation::anchor(
                ids::FEEDS_FORMULA_CHIP_NO_PRINTED_ROW,
            ),
            message: format!(
                "No printed row; formula chip. This flat end mill has no vendor roughing row \
                 for this material, so the {formula_chipload_mm:.4} mm/tooth comes from the \
                 machine chip formula (no source in the repository) and has no safe range."
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
            face: crate::diagnostics::presentation::face(
                ids::FEEDS_VENDOR_ROW_PUBLISHES_NO_CHIPLOAD,
            ),
            anchor: crate::diagnostics::presentation::anchor(
                ids::FEEDS_VENDOR_ROW_PUBLISHES_NO_CHIPLOAD,
            ),
            message: format!(
                "Vendor row {observation_id} gives an RPM but no chipload. The recommended \
                 {formula_chipload_mm:.4} mm/tooth comes from a formula, not from this \
                 vendor, so it has no safe range.{}",
                match floor_band_from {
                    // P1: the floor is the one clamp that now DOES see that
                    // row. Say which, or the operator reads the sentence
                    // above and assumes nothing did.
                    Some(row) => format!(
                        " After simulation, the chipload check uses a different vendor row \
                         that gives a chipload, so it judges the cut against limits this \
                         recipe did not use. The rubbing limit comes from that row ({row}), \
                         not from the fixed limit."
                    ),
                    // Ruling B4: a V-bit recipe rests on an RPM anchor only
                    // when no chipload-bearing row matches, so the gate has
                    // no row either.
                    None => " No vendor row with a chipload matches this cut, so the \
                             chipload check after simulation has no range to judge it."
                        .to_owned(),
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
                    "The feed rate {feed_rate_mm_min:.0} mm/min is {ratio:.1} \u{00D7} the \
                     recommended {rec_feed:.0} mm/min. The tool can break. Run the simulation \
                     to check the load."
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
                    "The feed rate {feed_rate_mm_min:.0} mm/min is {pct:.0} % of the \
                     recommended {rec_feed:.0} mm/min. The tool can rub and burn the wood. Run \
                     the simulation to check the load.",
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
                "The stepover {stepover:.2} mm is {ratio:.1} \u{00D7} the recommended \
                 {rec_radial:.2} mm.",
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
                "The depth per pass {dpp:.2} mm is {ratio:.1} \u{00D7} the recommended \
                 {rec_axial:.2} mm.",
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

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
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
        face: crate::diagnostics::presentation::face(id),
        anchor: crate::diagnostics::presentation::anchor(id),
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
