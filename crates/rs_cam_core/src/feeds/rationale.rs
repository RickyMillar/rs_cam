//! v3.1 (2026-06-04): Structured rationale tree for the combined-Suggest
//! orchestrator.
//!
//! Each [`crate::feeds::suggest::SuggestWarning`] variant materializes
//! into one [`RationaleEntry`] via [`SuggestRationale::from_warnings`].
//! The conversion is an exhaustive match — adding a new warning variant
//! is a compile error here, which forces the rationale surface to stay
//! in sync.
//!
//! The type is `Serialize` so GUI (feeds modal "Why these values?"
//! section, v3.1 ships) and MCP (`get_suggest_rationale`, v3.2) can
//! consume the same payload. The type *intentionally* duplicates some
//! information the structured `SuggestWarning` variants already carry —
//! the duplication is for human-readable strings (`headline`, `detail`)
//! that the wire-format consumers shouldn't be expected to format
//! themselves.

use serde::{Deserialize, Serialize};

use crate::feeds::suggest::{FeedRecalibrationCap, SuggestWarning};

/// Which operation field a rationale entry describes a change (or
/// warning) about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RationaleParam {
    /// `operation.plunge_rate` (mm/min).
    Plunge,
    /// `operation.stepover` (mm).
    Stepover,
    /// `operation.depth_per_pass` (mm).
    Dpp,
    /// `operation.feed_rate` (mm/min).
    Feed,
    /// `operation.entry_style` (Adaptive3d).
    EntryStyle,
    /// `operation.clearing_strategy` (Adaptive3d). v3.3 placeholder.
    ClearingStrategy,
    /// `operation.spindle_rpm` (rev/min). Ruling R4 Q10.
    Rpm,
    /// The rough pass's `stock_to_leave` allowance. The
    /// finish-envelope advisory is the one producer: it asks the
    /// operator to coordinate the rough allowance with the finish
    /// per-pass DOC.
    StockToLeave,
    /// `operation.ramp_feed_rate` (mm/min), the helix and ramp entry feed
    /// (G6 ramp).
    RampFeed,
}

/// Why a rationale entry exists. One-to-one with the nine
/// [`SuggestWarning`] variants currently emitted, plus three reserved
/// values for v3.3 strategy-aware passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RationaleReason {
    /// FeedsCalculator wrote `plunge > feed`; clamped plunge to feed.
    PlungeClampedToFeed,
    /// Adaptive plunge entry unsafe at this DPP / diameter ratio.
    /// Warning-only — Suggest does not rewrite the strategy.
    PlungeEntryUnstable,
    /// v3.3 placeholder: strategy classifier picked an
    /// `entry_style` / `clearing_strategy` / `stock_to_leave` value
    /// based on model geometry. Not emitted by current code.
    GeometryClassifier,
    /// G-SUGGEST-NOCLAMP (2026-08-19): the feed was re-derived after the
    /// invariant passes settled the operation's final stepover / DPP,
    /// because `feeds::calculate` sizes the chip-thinning and depth-tier
    /// terms against the geometry it was handed and later passes overwrite
    /// that geometry. Declared with the warning variants; not emitted until
    /// the rescale pass lands.
    FinalGeometryRescale,
    /// T-12 (2026-09-16): the operation has no field to hold a cut-geometry
    /// value the calculator produced, so the value was not applied. The
    /// entry reports a NON-change: `to_value` is `None`, because nothing
    /// was written.
    CutGeometryFieldNotHeld,
    /// T-15 (2026-09-18): Suggest pass 10 re-evaluated the spindle power
    /// ceiling at the geometry the operation ships, after pass 9 had
    /// re-derived the feed, and lowered the feed onto that ceiling.
    PowerCeilingAfterRescale,
    /// Ruling R4 Q10 (2026-09-24): the RPM came down to hold the chipload
    /// at the machine feed ceiling.
    RpmLoweredForFeedCeiling,
    /// G6 ramp (2026-09-25): Suggest wrote the entry feed, from the G6
    /// axial chip and the entry slope, or `None` with the plunge-rate
    /// fallback reason.
    RampFeed,
    /// G10 (2026-09-25): the funnel ran the plunge rule again at the feed
    /// that ships, and the plunge moved.
    PlungeReDerived,
    /// P5 (K2, D4): the apply accepted fields whose basis reads an
    /// Unsourced rule.
    UnsourcedRowsAccepted,
    /// P5 (K2b, K6): the point sits outside the safe envelope.
    OutsideEnvelope,
    /// P13c (O3): the assumptions of a computed width.
    ComputedWidthAssumptions,
}

/// One row in the rationale tree the GUI / MCP renders alongside a
/// Suggest invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RationaleEntry {
    /// Which field the entry concerns.
    pub param: RationaleParam,
    /// Why the change (or warning) happened.
    pub reason: RationaleReason,
    /// Pre-pass value when the entry represents a change. `None` for
    /// warning-only entries (e.g. plunge-entry instability — the
    /// warning doesn't itself rewrite any field).
    pub from_value: Option<f64>,
    /// Post-pass value when the entry represents a change. `None` for
    /// warning-only entries.
    pub to_value: Option<f64>,
    /// One-line human-readable summary. Safe to render verbatim in a
    /// monospace label.
    pub headline: String,
    /// Supplementary detail (predicted µm, iteration count, etc.) for
    /// tooltips or expanded views. `None` when the headline already
    /// carries everything useful.
    pub detail: Option<String>,
}

impl RationaleEntry {
    /// The headline and the detail as one paragraph, for a surface that
    /// prints one string (the diagnostic ribbon, the MCP and the CLI). This
    /// is the one place that joins the two into one string. The Feeds row
    /// hover prints them on two lines. The builders keep each clause in
    /// one of the two parts, so the joined text states each clause once.
    #[must_use]
    pub fn message(&self) -> String {
        match &self.detail {
            Some(detail) => format!("{} {detail}", self.headline),
            None => self.headline.clone(),
        }
    }
}

/// Vector of rationale entries — one per emitted warning, in emission
/// order. Renderers should preserve the order so a top-to-bottom read
/// follows the same sequence the orchestrator passes ran in.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SuggestRationale {
    pub entries: Vec<RationaleEntry>,
}

impl SuggestRationale {
    /// Materialize a rationale tree from the warning vector returned
    /// by `apply` / `suggest_for_operation`.
    ///
    /// One warning gives one entry.
    pub fn from_warnings(warnings: &[SuggestWarning]) -> Self {
        Self {
            entries: warnings.iter().map(entry_for_warning).collect(),
        }
    }

    /// `true` when there are no entries — the Suggest pass made no
    /// changes worth surfacing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Exhaustive match — adding a new [`SuggestWarning`] variant is a
/// compile error here, which is by design. The renderer surface
/// stays in lock-step with the warning enum.
fn entry_for_warning(w: &SuggestWarning) -> RationaleEntry {
    match w {
        SuggestWarning::CutGeometryFieldNotHeld {
            param_name,
            op_kind,
            recommended_mm,
        } => RationaleEntry {
            param: if *param_name == "stepover" {
                RationaleParam::Stepover
            } else {
                RationaleParam::Dpp
            },
            reason: RationaleReason::CutGeometryFieldNotHeld,
            // `from_value` is the value that was NOT applied, and
            // `to_value` is `None` because nothing was written. A
            // `to_value` here would claim a change that did not happen.
            from_value: Some(*recommended_mm),
            to_value: None,
            headline: format!("{op_kind} has no {param_name} to set"),
            detail: Some(format!(
                "The calculator recommended {recommended_mm:.3} mm. This \
                 operation carries no {param_name} field, so the value was \
                 not applied. Change the cut another way."
            )),
        },
        SuggestWarning::PlungeClampedToFeed { requested, capped } => RationaleEntry {
            param: RationaleParam::Plunge,
            reason: RationaleReason::PlungeClampedToFeed,
            from_value: Some(*requested),
            to_value: Some(*capped),
            headline: format!("Plunge clamped to feed ({capped:.0} mm/min)"),
            detail: Some(format!(
                "Requested plunge {requested:.0} mm/min exceeded feed rate"
            )),
        },
        SuggestWarning::PlungeEntryUnstableAtDpp {
            dpp_mm,
            diameter_mm,
            entry_style,
        } => RationaleEntry {
            param: RationaleParam::EntryStyle,
            reason: RationaleReason::PlungeEntryUnstable,
            from_value: None,
            to_value: None,
            headline: format!("Plunge entry unstable at DPP {dpp_mm:.2} mm / Ø{diameter_mm:.2} mm"),
            detail: Some(format!(
                "Entry style '{entry_style}' likely to spike deflection on plunge; switch to helix/ramp or reduce DPP"
            )),
        },
        SuggestWarning::StrategyRecommendedNotApplied {
            param,
            current,
            recommended,
            reason,
        } => {
            let rationale_param = match *param {
                "entry_style" => RationaleParam::EntryStyle,
                "clearing_strategy" => RationaleParam::ClearingStrategy,
                // Future strategy fields: a safe sentinel rather than a
                // crash. The headline names the field.
                _ => RationaleParam::EntryStyle,
            };
            RationaleEntry {
                param: rationale_param,
                reason: RationaleReason::GeometryClassifier,
                from_value: None,
                to_value: None,
                headline: format!(
                    "{param}: classifier recommends {recommended} (not auto-applied, current {current})"
                ),
                detail: Some(format!(
                    "Heuristic: {reason}; auto-selection across clearing strategies ships in v4"
                )),
            }
        }
        // G-SUGGEST-NOCLAMP: produced by Suggest pass 9 since 2026-08-19.
        SuggestWarning::FeedRescaledToFinalGeometry {
            requested_mm_per_min,
            rescaled_mm_per_min,
            factor_at_calculator,
            factor_at_final,
            cap_hit,
        } => RationaleEntry {
            param: RationaleParam::Feed,
            reason: RationaleReason::FinalGeometryRescale,
            from_value: Some(*requested_mm_per_min),
            to_value: Some(*rescaled_mm_per_min),
            headline: format!(
                "Feed re-derived at the final geometry ({requested_mm_per_min:.0} → \
                 {rescaled_mm_per_min:.0} mm/min)"
            ),
            detail: Some(format!(
                "Chip-thinning × depth-tier was {factor_at_calculator:.4} at the operating \
                 point the calculator sized the feed against, and is {factor_at_final:.4} at \
                 the stepover / DPP the operation actually runs{}",
                match cap_hit {
                    Some(FeedRecalibrationCap::MaxFeed) =>
                        "; truncated at the machine's cutting-feed ceiling, so the re-derived \
                         feed was not reached",
                    Some(FeedRecalibrationCap::DeflectionThreshold) =>
                        "; refused by the deflection budget",
                    None => "",
                }
            )),
        },
        // Ruling R4 WP2a (2026-09-23): pass 9 warns and does not raise the
        // feed. The entry reports a NON-change: `to_value` is `None`,
        // because nothing was written.
        SuggestWarning::FeedClampedToChiploadFloor {
            requested_mm_per_tooth,
            floor_mm_per_tooth,
            source,
        } => RationaleEntry {
            param: RationaleParam::Feed,
            reason: RationaleReason::FinalGeometryRescale,
            from_value: Some(*requested_mm_per_tooth),
            to_value: None,
            headline: format!(
                "Advance per tooth below the rubbing floor after rescale: \
                 {requested_mm_per_tooth:.4} mm/tooth (floor {floor_mm_per_tooth:.4}, {}). \
                 The feed is not raised.",
                source.describe()
            ),
            detail: Some(
                "Below the floor the tool rubs instead of cutting and can burn the work. \
                 Raise the feed or lower the RPM."
                    .to_owned(),
            ),
        },
        // T-15: produced by Suggest pass 10 since 2026-09-18.
        SuggestWarning::PowerRecheckedAfterRescale {
            rescaled_mm_per_min,
            shipped_mm_per_min,
            required_kw_at_rescaled,
            available_kw,
            fits_at_any_feed,
        } => RationaleEntry {
            param: RationaleParam::Feed,
            reason: RationaleReason::PowerCeilingAfterRescale,
            from_value: Some(*rescaled_mm_per_min),
            to_value: Some(*shipped_mm_per_min),
            headline: if *fits_at_any_feed {
                format!(
                    "Feed lowered to the spindle power ceiling ({rescaled_mm_per_min:.0} → \
                     {shipped_mm_per_min:.0} mm/min)"
                )
            } else {
                format!(
                    "No feed fits the spindle at this cut ({rescaled_mm_per_min:.0} → \
                     {shipped_mm_per_min:.0} mm/min)"
                )
            },
            detail: Some(if *fits_at_any_feed {
                format!(
                    "Re-deriving the feed at the final depth and stepover asked for \
                     {required_kw_at_rescaled:.3} kW against the {available_kw:.3} kW this \
                     spindle has at the commanded speed. The feed now lands on that ceiling."
                )
            } else {
                format!(
                    "The cut draws {required_kw_at_rescaled:.3} kW against the \
                     {available_kw:.3} kW this spindle has at the commanded speed, and the \
                     part that does not scale with the feed is already over it. A thinner \
                     chip cannot rescue the cut, so the feed went back to the value before \
                     the re-derivation. Take less depth, less stepover or a lower speed."
                )
            }),
        },
        SuggestWarning::RpmLoweredForFeedCeiling {
            rpm_from,
            rpm_to,
            feed_ceiling_mm_min,
            rpm_floor,
            floor_source,
            held,
            arms,
        } => RationaleEntry {
            param: RationaleParam::Rpm,
            reason: RationaleReason::RpmLoweredForFeedCeiling,
            from_value: Some(*rpm_from),
            to_value: Some(*rpm_to),
            headline: crate::feeds::rpm_lowered_text(
                *rpm_from,
                *rpm_to,
                *feed_ceiling_mm_min,
                *rpm_floor,
                *floor_source,
                *held,
                arms,
            ),
            detail: Some(match arms.shipped {
                crate::feeds::CapArm::LowerRpmSameChip => {
                    "The feed moves down with the RPM, so the feed per tooth stays at the \
                     vendor value."
                        .to_owned()
                }
                crate::feeds::CapArm::SameRpmSmallerChip => {
                    "The lower RPM is outside the power curve of the spindle, so the RPM \
                     holds and the feed per tooth is thinner at the feed limit."
                        .to_owned()
                }
            }),
        },
        SuggestWarning::RampFeed {
            from_mm_min,
            record,
            notes,
        } => {
            let (headline, mut detail) = record.card_text();
            // G10: the entry rules ride on the ramp record. Each note is
            // one sentence of the detail here; the card paints each as a
            // face line.
            for note in notes.as_slice() {
                let mark = if note.caution { "Caution: " } else { "" };
                detail.push_str(&format!("; {mark}{}", note.headline));
            }
            RationaleEntry {
                param: RationaleParam::RampFeed,
                reason: RationaleReason::RampFeed,
                from_value: *from_mm_min,
                to_value: record.value(),
                headline,
                detail: Some(detail),
            }
        }
        SuggestWarning::UnsourcedRowsAccepted { rows } => RationaleEntry {
            param: RationaleParam::Feed,
            reason: RationaleReason::UnsourcedRowsAccepted,
            from_value: None,
            to_value: None,
            headline: format!("Apply accepted {} rule(s) with no source", rows.len()),
            detail: Some(format!(
                "The written values read rules that no file in the repo prints (F201): {}. \
                 The apply is the acceptance (D4).",
                rows.join(", ")
            )),
        },
        SuggestWarning::ComputedWidthAssumptions => RationaleEntry {
            param: RationaleParam::Stepover,
            reason: RationaleReason::ComputedWidthAssumptions,
            from_value: None,
            to_value: None,
            headline: crate::feeds::force_budget::computed_width_assumptions_line(),
            detail: Some(format!(
                "No limit models these: {}. The width is the cut with the most removal per \
                 minute inside the limits, bounded by the corner-coverage rule.",
                crate::feeds::force_budget::COMPUTED_WIDTH_ASSUMPTIONS.join(", ")
            )),
        },
        SuggestWarning::OutsideEnvelope {
            binding,
            ap_mm,
            ae_mm,
        } => RationaleEntry {
            param: RationaleParam::Dpp,
            reason: RationaleReason::OutsideEnvelope,
            from_value: None,
            to_value: None,
            headline: format!(
                "Cut {ap_mm:.2} x {ae_mm:.2} mm is outside the safe envelope: {}",
                binding.label()
            ),
            // P29-10 10i (lead ruling 4): no margin from the placement; the
            // stated row of the limit holds the number.
            detail: Some(format!(
                "The point is past the {} limit; its stated row gives the number. The value is \
                 written; it is a warning (ruling Q6).",
                binding.label()
            )),
        },
        SuggestWarning::PlungeReDerived {
            from_mm_min,
            to_mm_min,
            binding,
        } => RationaleEntry {
            param: RationaleParam::Plunge,
            reason: RationaleReason::PlungeReDerived,
            from_value: Some(*from_mm_min),
            to_value: Some(*to_mm_min),
            headline: format!(
                "Plunge {from_mm_min:.0} -> {to_mm_min:.0} mm/min: the plunge rule at the feed \
                 that ships"
            ),
            detail: Some(format!(
                "The feed moved after the calculator (pass 9, pass 10 or an explored feed), so \
                 the funnel ran the plunge rule again at the feed that ships (G10). The plunge \
                 is set by {}.",
                binding.label()
            )),
        },
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    fn rt(w: SuggestWarning) -> RationaleEntry {
        SuggestRationale::from_warnings(&[w]).entries.remove(0)
    }

    #[test]
    fn plunge_clamped_round_trips() {
        let e = rt(SuggestWarning::PlungeClampedToFeed {
            requested: 800.0,
            capped: 600.0,
        });
        assert_eq!(e.param, RationaleParam::Plunge);
        assert_eq!(e.reason, RationaleReason::PlungeClampedToFeed);
        assert_eq!(e.from_value, Some(800.0));
        assert_eq!(e.to_value, Some(600.0));
        assert!(e.headline.contains("Plunge clamped"));
    }

    #[test]
    fn plunge_entry_unstable_round_trips() {
        let e = rt(SuggestWarning::PlungeEntryUnstableAtDpp {
            dpp_mm: 3.69,
            diameter_mm: 6.0,
            entry_style: "plunge".to_owned(),
        });
        assert_eq!(e.param, RationaleParam::EntryStyle);
        assert_eq!(e.reason, RationaleReason::PlungeEntryUnstable);
        // Warning-only: from/to should be None.
        assert_eq!(e.from_value, None);
        assert_eq!(e.to_value, None);
        assert!(e.detail.as_deref().unwrap_or_default().contains("plunge"));
    }

    #[test]
    fn strategy_recommended_not_applied_round_trips() {
        let e = rt(SuggestWarning::StrategyRecommendedNotApplied {
            param: "clearing_strategy",
            current: "contour_parallel".to_owned(),
            recommended: "geodesic_adaptive".to_owned(),
            reason: "mixed_terrain_classifier",
        });
        assert_eq!(e.param, RationaleParam::ClearingStrategy);
        assert_eq!(e.reason, RationaleReason::GeometryClassifier);
        // Warn-only entries carry no numeric from/to.
        assert_eq!(e.from_value, None);
        assert_eq!(e.to_value, None);
        assert!(e.headline.contains("geodesic_adaptive"));
        assert!(
            e.headline.contains("not auto-applied"),
            "headline must say the recommendation was not applied, got {:?}",
            e.headline
        );
        assert!(
            e.detail
                .as_deref()
                .unwrap_or_default()
                .contains("mixed_terrain_classifier")
        );
    }

    #[test]
    fn empty_warnings_yields_empty_rationale() {
        let r = SuggestRationale::from_warnings(&[]);
        assert!(r.is_empty());
        assert_eq!(r.entries.len(), 0);
    }

    #[test]
    fn preserves_emission_order() {
        let r = SuggestRationale::from_warnings(&[
            SuggestWarning::PlungeClampedToFeed {
                requested: 800.0,
                capped: 600.0,
            },
            SuggestWarning::StrategyRecommendedNotApplied {
                param: "clearing_strategy",
                current: "contour_parallel".to_owned(),
                recommended: "geodesic_adaptive".to_owned(),
                reason: "mixed_terrain_classifier",
            },
        ]);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].param, RationaleParam::Plunge);
        assert_eq!(r.entries[1].param, RationaleParam::ClearingStrategy);
    }

    #[test]
    fn serializes_to_json() {
        // Smoke test: the type round-trips through JSON. MCP / GUI
        // surfaces depend on this — if the derive accidentally lands
        // on a non-Serialize variant we want to know.
        let r = SuggestRationale::from_warnings(&[SuggestWarning::PlungeClampedToFeed {
            requested: 800.0,
            capped: 600.0,
        }]);
        let json = serde_json::to_string(&r).expect("serialize");
        let back: SuggestRationale = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, back);
    }

    /// The clauses of a text, split at `;`, `:` and `.`, trimmed. A clause
    /// under 16 characters ("mm", "Caution") is too short to count as a
    /// repeat.
    fn long_clauses(text: &str) -> Vec<String> {
        text.split([';', ':', '.'])
            .map(|c| c.trim().to_owned())
            .filter(|c| c.len() >= 16)
            .collect()
    }

    /// Panic when one clause occurs twice in `text`.
    fn assert_no_repeated_clause(text: &str) {
        let clauses = long_clauses(text);
        for (i, clause) in clauses.iter().enumerate() {
            assert!(
                !clauses[i + 1..].contains(clause),
                "the clause {clause:?} occurs twice in: {text}"
            );
        }
    }

    /// The joined text of every round-trip fixture in this module states
    /// each clause once.
    #[test]
    fn no_rationale_message_repeats_a_clause() {
        let warnings = [
            SuggestWarning::PlungeClampedToFeed {
                requested: 800.0,
                capped: 600.0,
            },
            SuggestWarning::StrategyRecommendedNotApplied {
                param: "clearing_strategy",
                current: "contour_parallel".to_owned(),
                recommended: "geodesic_adaptive".to_owned(),
                reason: "mixed_terrain_classifier",
            },
        ];
        for entry in SuggestRationale::from_warnings(&warnings).entries {
            assert_no_repeated_clause(&entry.message());
        }
    }
}
