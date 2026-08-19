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

/// **The report-tier label on the two chipload-recalibration entries**
/// (`ChiploadTarget`, `ChiploadCapBound`).
///
/// Both entries quote a chipload that was produced by the retired
/// `predict_observed_chipload_mm` — `nominal × arc_fit_ratio`, where the
/// ratio table had been fitted against the post-sim gate's **arc-mean chip
/// thickness** observation. That observation was deleted on 2026-08-06 (the
/// gate now reports `effective_feed / (rpm · flutes)`, a linear advance per
/// tooth), so the quoted number predicted a quantity nothing measured any
/// more, and the entries presented it as gate-targeted calibration.
///
/// This label was the operator review's pre-ruling remedy, shipped
/// **report-tier by construction**: it moved no recipe number, changed no
/// verdict, and left the applied feed exactly where it was.
///
/// **Status since 2026-08-13 (Checkpoint J-1/J-5, ledger row F-T35).** The
/// lift itself is retired, so `feeds::suggest` no longer emits the two
/// warnings these entries render and **nothing in the shipped Suggest path
/// reaches this label today**. Both the warnings and this rendering were
/// kept deliberately, for the simulation-backed path to reuse. When that
/// producer lands it will be supplying a **measured** observation, at which
/// point this label becomes wrong and must be removed in the same commit —
/// a legacy-estimate disclaimer on a measured number is a new defect, not a
/// leftover.
///
/// It lives here rather than in the GUI because every renderer — the feeds
/// modal's "Why these values?" list, the MCP `get_suggest_rationale` payload
/// — prints [`RationaleEntry::headline`] and [`RationaleEntry::detail`]
/// verbatim. One string, every surface.
pub const LEGACY_ESTIMATE_LABEL: &str = "Legacy pre-simulation estimate";

/// The sentence that says *why* [`LEGACY_ESTIMATE_LABEL`] applies, so a
/// reader who has never heard of the arc-fit table still knows what to do
/// about it (simulate).
pub const LEGACY_ESTIMATE_NOTE: &str = "Estimated against the arc-mean chip observation the post-sim gate retired on 2026-08-06, \
     not the advance per tooth it reports today — simulate to get the observed value.";

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
    /// `operation.stock_to_leave_radial` / `stock_to_leave_axial`
    /// (Adaptive3d). v3.3 placeholder.
    StockToLeave,
}

/// Why a rationale entry exists. One-to-one with the nine
/// [`SuggestWarning`] variants currently emitted, plus three reserved
/// values for v3.3 strategy-aware passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RationaleReason {
    /// FeedsCalculator wrote `plunge > feed`; clamped plunge to feed.
    PlungeClampedToFeed,
    /// Stepover exceeded tool diameter; clamped to diameter.
    StepoverClampedToDiameter,
    /// Spindle-rigidity factor capped the roughing DPP.
    RigidityFactor,
    /// Tool cutting length capped DPP.
    CuttingLengthCap,
    /// Runtime move-count predictor raised stepover.
    RuntimeFloor,
    /// Closed-form deflection predictor capped DPP.
    DeflectionPredict,
    /// Adaptive plunge entry unsafe at this DPP / diameter ratio.
    /// Warning-only — Suggest does not rewrite the strategy.
    PlungeEntryUnstable,
    /// Feed recalibrated to land observed chipload at the policy
    /// target inside the LUT band.
    ///
    /// The chipload figures on this entry are a **legacy pre-simulation
    /// estimate** — see [`LEGACY_ESTIMATE_LABEL`].
    ChiploadTarget,
    /// Feed could not reach LUT target; bound by a cap (MaxFeed or
    /// DeflectionThreshold).
    ///
    /// Same caveat as [`Self::ChiploadTarget`]: the quoted chipload is an
    /// estimate, not a gate observation ([`LEGACY_ESTIMATE_LABEL`]).
    ChiploadCapBound,
    /// v3.3 placeholder: strategy classifier picked an
    /// `entry_style` / `clearing_strategy` / `stock_to_leave` value
    /// based on model geometry. Not emitted by current code.
    GeometryClassifier,
    /// v3.3 placeholder: caller explicitly pinned this field; Suggest
    /// did not rewrite it. Not emitted by current code.
    UserPin,
    /// v3 placeholder: `SuggestAggressiveness::Speed` reached LUT max
    /// but the deflection budget refused. Not yet emitted (today's
    /// closed-form predictor is feed-independent so Speed never
    /// trips the deflection refusal).
    SpeedTargetGated,
    /// G-SUGGEST-NOCLAMP (2026-08-19): the feed was re-derived after the
    /// invariant passes settled the operation's final stepover / DPP,
    /// because `feeds::calculate` sizes the chip-thinning and depth-tier
    /// terms against the geometry it was handed and later passes overwrite
    /// that geometry. Declared with the warning variants; not emitted until
    /// the rescale pass lands.
    FinalGeometryRescale,
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

/// Vector of rationale entries — one per emitted warning, in emission
/// order. Renderers should preserve the order so a top-to-bottom read
/// follows the same sequence the orchestrator passes ran in.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SuggestRationale {
    pub entries: Vec<RationaleEntry>,
}

impl SuggestRationale {
    /// Materialize a rationale tree from the warning vector returned
    /// by `apply_feeds_result_to_op` / `suggest_for_operation`.
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
        SuggestWarning::StepoverClampedToToolDiameter { requested, capped } => RationaleEntry {
            param: RationaleParam::Stepover,
            reason: RationaleReason::StepoverClampedToDiameter,
            from_value: Some(*requested),
            to_value: Some(*capped),
            headline: format!("Stepover clamped to tool diameter ({capped:.2} mm)"),
            detail: Some(format!(
                "Requested {requested:.2} mm exceeded tool diameter"
            )),
        },
        SuggestWarning::RoughingDepthClampedToRigidity { requested, capped } => RationaleEntry {
            param: RationaleParam::Dpp,
            reason: RationaleReason::RigidityFactor,
            from_value: Some(*requested),
            to_value: Some(*capped),
            headline: format!("DPP capped to rigidity factor ({capped:.2} mm)"),
            detail: Some(format!(
                "Requested {requested:.2} mm exceeded machine rigidity ceiling"
            )),
        },
        SuggestWarning::DepthClampedToCuttingLength { requested, capped } => RationaleEntry {
            param: RationaleParam::Dpp,
            reason: RationaleReason::CuttingLengthCap,
            from_value: Some(*requested),
            to_value: Some(*capped),
            headline: format!("DPP capped to tool cutting length ({capped:.2} mm)"),
            detail: Some(format!(
                "Requested {requested:.2} mm exceeded the tool's flute length"
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
        SuggestWarning::DppCappedByDeflection {
            requested_mm,
            capped_mm,
            predicted_um_at_requested,
            predicted_um_at_capped,
            iterations,
        } => RationaleEntry {
            param: RationaleParam::Dpp,
            reason: RationaleReason::DeflectionPredict,
            from_value: Some(*requested_mm),
            to_value: Some(*capped_mm),
            headline: format!(
                "DPP backed off to {capped_mm:.2} mm (deflection {predicted_um_at_capped:.0} µm)"
            ),
            detail: Some(format!(
                "Predicted {predicted_um_at_requested:.0} µm at requested {requested_mm:.2} mm; {iterations} back-off iterations"
            )),
        },
        SuggestWarning::StepoverRaisedForRuntime {
            requested_mm,
            raised_mm,
            predicted_moves_at_requested,
            predicted_moves_at_raised,
            iterations,
        } => RationaleEntry {
            param: RationaleParam::Stepover,
            reason: RationaleReason::RuntimeFloor,
            from_value: Some(*requested_mm),
            to_value: Some(*raised_mm),
            headline: format!("Stepover raised to {raised_mm:.3} mm (runtime sanity)"),
            detail: Some(format!(
                "Predicted {predicted_moves_at_requested} moves at {requested_mm:.3} mm; {predicted_moves_at_raised} after {iterations} raises"
            )),
        },
        SuggestWarning::FeedRaisedForChipload {
            requested_mm_per_min,
            raised_mm_per_min,
            predicted_observed_chipload_before,
            predicted_observed_chipload_after,
            lut_target_mm_per_tooth,
            cap_hit,
        } => {
            let cap_note = match cap_hit {
                Some(FeedRecalibrationCap::MaxFeed) => " — capped at machine max",
                Some(FeedRecalibrationCap::DeflectionThreshold) => {
                    " — reverted by deflection refusal"
                }
                None => "",
            };
            RationaleEntry {
                param: RationaleParam::Feed,
                reason: RationaleReason::ChiploadTarget,
                from_value: Some(*requested_mm_per_min),
                to_value: Some(*raised_mm_per_min),
                headline: format!("Feed raised to {raised_mm_per_min:.0} mm/min{cap_note}"),
                detail: Some(format!(
                    "{LEGACY_ESTIMATE_LABEL}: chipload {predicted_observed_chipload_before:.4} → {predicted_observed_chipload_after:.4} mm/tooth (LUT target {lut_target_mm_per_tooth:.4}). {LEGACY_ESTIMATE_NOTE}"
                )),
            }
        }
        SuggestWarning::ChiploadStillLowAfterRecalibration {
            predicted_observed_mm_per_tooth,
            lut_target_mm_per_tooth,
            feed_at_termination_mm_per_min,
            blocking_cap,
        } => {
            let cap_label = match blocking_cap {
                FeedRecalibrationCap::MaxFeed => "machine max-feed",
                FeedRecalibrationCap::DeflectionThreshold => "deflection budget",
            };
            RationaleEntry {
                param: RationaleParam::Feed,
                reason: RationaleReason::ChiploadCapBound,
                from_value: None,
                to_value: Some(*feed_at_termination_mm_per_min),
                headline: format!("Chipload still low after recal — bound by {cap_label}"),
                detail: Some(format!(
                    "{LEGACY_ESTIMATE_LABEL}: {predicted_observed_mm_per_tooth:.4} mm/tooth vs LUT target {lut_target_mm_per_tooth:.4} at feed {feed_at_termination_mm_per_min:.0} mm/min. {LEGACY_ESTIMATE_NOTE}"
                )),
            }
        }
        SuggestWarning::StrategyRewrote {
            param,
            from,
            to,
            reason,
        } => {
            let rationale_param = match *param {
                "entry_style" => RationaleParam::EntryStyle,
                "clearing_strategy" => RationaleParam::ClearingStrategy,
                "stock_to_leave_radial" | "stock_to_leave_axial" | "stock_to_leave" => {
                    RationaleParam::StockToLeave
                }
                // Future strategy fields: fall back to EntryStyle as a
                // safe sentinel rather than crash. The headline still
                // names the field literally.
                _ => RationaleParam::EntryStyle,
            };
            RationaleEntry {
                param: rationale_param,
                reason: RationaleReason::GeometryClassifier,
                from_value: None,
                to_value: None,
                headline: format!("{param} rewritten: {from} → {to}"),
                detail: Some(format!("Heuristic: {reason}")),
            }
        }
        SuggestWarning::AxialEnvelopeSafeBandEmpty {
            op_kind,
            max_safe_doc_mm,
            chipload_floor_doc_mm,
            binding_upper,
        } => RationaleEntry {
            param: RationaleParam::Dpp,
            reason: RationaleReason::DeflectionPredict,
            from_value: None,
            to_value: None,
            headline: format!(
                "{op_kind}: tool / surface / chipload mismatch — \
                 safe band empty (floor {chipload_floor_doc_mm:.2} mm > \
                 max {max_safe_doc_mm:.2} mm, binding {binding_upper})"
            ),
            detail: Some(format!(
                "Chipload-burn floor exceeds the {binding_upper} ceiling — \
                 switch tool, raise chipload (smaller WOC), or accept the \
                 rubbing recipe; automatic stock_to_leave repair deferred \
                 until in-process stock"
            )),
        },
        SuggestWarning::AxialDocClampedByEnvelope {
            op_kind,
            param_name,
            commanded_mm,
            clamped_mm,
            binding,
        } => RationaleEntry {
            param: RationaleParam::Dpp,
            reason: RationaleReason::DeflectionPredict,
            from_value: Some(*commanded_mm),
            to_value: Some(*clamped_mm),
            headline: format!(
                "{op_kind} {param_name} clamped {commanded_mm:.2} → \
                 {clamped_mm:.2} mm ({binding})"
            ),
            detail: Some(format!("Cutter axial envelope binding: {binding}")),
        },
        SuggestWarning::AxialDocBelowBurnFloor {
            op_kind,
            param_name,
            commanded_mm,
            floor_mm,
        } => RationaleEntry {
            param: RationaleParam::Dpp,
            reason: RationaleReason::ChiploadCapBound,
            from_value: Some(*commanded_mm),
            to_value: None,
            headline: format!(
                "{op_kind} {param_name} {commanded_mm:.2} mm below \
                 chipload-burn floor {floor_mm:.2} mm"
            ),
            detail: Some(
                "Per policy C the envelope respects deliberately \
                 conservative pins — the rubbing-floor warning surfaces \
                 the trade-off but does not rewrite the value"
                    .to_owned(),
            ),
        },
        SuggestWarning::ProjectCurveDepthInfeasible {
            commanded_mm,
            max_safe_mm,
            binding,
        } => RationaleEntry {
            param: RationaleParam::Dpp,
            reason: RationaleReason::DeflectionPredict,
            from_value: Some(*commanded_mm),
            to_value: None,
            headline: format!(
                "Engraving depth {commanded_mm:.2} mm exceeds safe \
                 envelope {max_safe_mm:.2} mm ({binding})"
            ),
            detail: Some(
                "Reduce target_depth or switch to a stiffer cutter; \
                 Suggest does not rewrite engraving depth"
                    .to_owned(),
            ),
        },
        SuggestWarning::FinishEnvelopeAdvisory {
            op_kind,
            max_safe_doc_mm,
            binding,
        } => RationaleEntry {
            param: RationaleParam::StockToLeave,
            reason: RationaleReason::DeflectionPredict,
            from_value: None,
            to_value: None,
            headline: format!("{op_kind} envelope max DOC {max_safe_doc_mm:.2} mm ({binding})"),
            detail: Some(format!(
                "Coordinate rough's stock_to_leave so the finish per-pass \
                 DOC sits below {max_safe_doc_mm:.2} mm; automatic \
                 coordination deferred until in-process stock at gen time"
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
                "stock_to_leave_radial" | "stock_to_leave_axial" | "stock_to_leave" => {
                    RationaleParam::StockToLeave
                }
                // Future strategy fields: same safe sentinel as the
                // StrategyRewrote arm — headline names the field.
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
        SuggestWarning::FeedClampedToChiploadFloor {
            requested_mm_per_tooth,
            floor_mm_per_tooth,
            band_capped_from,
        } => RationaleEntry {
            param: RationaleParam::Feed,
            reason: RationaleReason::FinalGeometryRescale,
            from_value: Some(*requested_mm_per_tooth),
            to_value: Some(*floor_mm_per_tooth),
            headline: format!(
                "Chipload clamped to floor after rescale ({requested_mm_per_tooth:.4} → \
                 {floor_mm_per_tooth:.4} mm/tooth)"
            ),
            detail: Some(band_capped_from.map_or_else(
                || {
                    "Re-deriving the feed at the final geometry put the advance below the \
                     chip-formation floor; the floor governs"
                        .to_owned()
                },
                |band_max| {
                    format!(
                        "The whole derated band sits below the chip-formation floor, so the \
                         floor was itself capped to the band maximum {band_max:.4} mm/tooth — \
                         expect burnishing"
                    )
                },
            )),
        },
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
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
    fn stepover_diameter_clamp_round_trips() {
        let e = rt(SuggestWarning::StepoverClampedToToolDiameter {
            requested: 8.0,
            capped: 6.0,
        });
        assert_eq!(e.param, RationaleParam::Stepover);
        assert_eq!(e.reason, RationaleReason::StepoverClampedToDiameter);
        assert_eq!(e.to_value, Some(6.0));
    }

    #[test]
    fn rigidity_dpp_clamp_round_trips() {
        let e = rt(SuggestWarning::RoughingDepthClampedToRigidity {
            requested: 12.0,
            capped: 9.0,
        });
        assert_eq!(e.param, RationaleParam::Dpp);
        assert_eq!(e.reason, RationaleReason::RigidityFactor);
        assert_eq!(e.from_value, Some(12.0));
    }

    #[test]
    fn cutting_length_dpp_clamp_round_trips() {
        let e = rt(SuggestWarning::DepthClampedToCuttingLength {
            requested: 20.0,
            capped: 12.0,
        });
        assert_eq!(e.param, RationaleParam::Dpp);
        assert_eq!(e.reason, RationaleReason::CuttingLengthCap);
        assert_eq!(e.to_value, Some(12.0));
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
    fn deflection_dpp_backoff_round_trips() {
        let e = rt(SuggestWarning::DppCappedByDeflection {
            requested_mm: 9.0,
            capped_mm: 3.69,
            predicted_um_at_requested: 395.0,
            predicted_um_at_capped: 178.0,
            iterations: 4,
        });
        assert_eq!(e.param, RationaleParam::Dpp);
        assert_eq!(e.reason, RationaleReason::DeflectionPredict);
        assert_eq!(e.from_value, Some(9.0));
        assert!(e.detail.as_deref().unwrap_or_default().contains("395"));
    }

    #[test]
    fn stepover_runtime_backoff_round_trips() {
        let e = rt(SuggestWarning::StepoverRaisedForRuntime {
            requested_mm: 0.03,
            raised_mm: 0.228,
            predicted_moves_at_requested: 3_333_333,
            predicted_moves_at_raised: 438_957,
            iterations: 5,
        });
        assert_eq!(e.param, RationaleParam::Stepover);
        assert_eq!(e.reason, RationaleReason::RuntimeFloor);
        assert_eq!(e.from_value, Some(0.03));
        assert_eq!(e.to_value, Some(0.228));
    }

    #[test]
    fn feed_raised_uncapped_round_trips() {
        let e = rt(SuggestWarning::FeedRaisedForChipload {
            requested_mm_per_min: 911.0,
            raised_mm_per_min: 5268.0,
            predicted_observed_chipload_before: 0.0093,
            predicted_observed_chipload_after: 0.054,
            lut_target_mm_per_tooth: 0.054,
            cap_hit: None,
        });
        assert_eq!(e.param, RationaleParam::Feed);
        assert_eq!(e.reason, RationaleReason::ChiploadTarget);
        assert_eq!(e.from_value, Some(911.0));
        // No cap note when uncapped.
        assert!(!e.headline.contains("capped"));
        assert!(!e.headline.contains("reverted"));
    }

    /// A-5 report-tier label (ledger **F-T35**): both chipload-recalibration
    /// entries must mark their quoted chipload as an estimate rather than a
    /// gate observation, and must say why.
    ///
    /// Report-tier only — this asserts the *strings*. Nothing here constrains
    /// `from_value` / `to_value`, which are the applied feeds and did not
    /// move.
    #[test]
    fn chipload_recalibration_entries_are_labelled_a_legacy_estimate() {
        let raised = rt(SuggestWarning::FeedRaisedForChipload {
            requested_mm_per_min: 911.0,
            raised_mm_per_min: 5268.0,
            predicted_observed_chipload_before: 0.0093,
            predicted_observed_chipload_after: 0.054,
            lut_target_mm_per_tooth: 0.054,
            cap_hit: None,
        });
        let still_low = rt(SuggestWarning::ChiploadStillLowAfterRecalibration {
            predicted_observed_mm_per_tooth: 0.0357,
            lut_target_mm_per_tooth: 0.0708,
            feed_at_termination_mm_per_min: 10_000.0,
            blocking_cap: FeedRecalibrationCap::MaxFeed,
        });

        for (tag, entry) in [("FeedRaised", &raised), ("StillLow", &still_low)] {
            let detail = entry
                .detail
                .as_deref()
                .unwrap_or_else(|| panic!("{tag}: entry must carry a detail line"));
            assert!(
                detail.contains(LEGACY_ESTIMATE_LABEL),
                "{tag}: detail must carry the estimate label, got {detail:?}"
            );
            assert!(
                detail.contains("2026-08-06"),
                "{tag}: detail must date the retired observation so the label is checkable, \
                 got {detail:?}"
            );
        }

        // The applied numbers are untouched by the label.
        assert_eq!(raised.from_value, Some(911.0));
        assert_eq!(raised.to_value, Some(5268.0));
        assert_eq!(still_low.to_value, Some(10_000.0));
    }

    #[test]
    fn feed_raised_max_feed_cap_round_trips() {
        let e = rt(SuggestWarning::FeedRaisedForChipload {
            requested_mm_per_min: 4672.0,
            raised_mm_per_min: 10_000.0,
            predicted_observed_chipload_before: 0.0167,
            predicted_observed_chipload_after: 0.0357,
            lut_target_mm_per_tooth: 0.0708,
            cap_hit: Some(FeedRecalibrationCap::MaxFeed),
        });
        assert!(e.headline.contains("capped at machine max"));
    }

    #[test]
    fn feed_raised_deflection_revert_round_trips() {
        let e = rt(SuggestWarning::FeedRaisedForChipload {
            requested_mm_per_min: 911.0,
            raised_mm_per_min: 911.0,
            predicted_observed_chipload_before: 0.005,
            predicted_observed_chipload_after: 0.005,
            lut_target_mm_per_tooth: 0.027,
            cap_hit: Some(FeedRecalibrationCap::DeflectionThreshold),
        });
        assert!(e.headline.contains("reverted by deflection"));
    }

    #[test]
    fn chipload_still_low_max_feed_round_trips() {
        let e = rt(SuggestWarning::ChiploadStillLowAfterRecalibration {
            predicted_observed_mm_per_tooth: 0.0357,
            lut_target_mm_per_tooth: 0.0708,
            feed_at_termination_mm_per_min: 10_000.0,
            blocking_cap: FeedRecalibrationCap::MaxFeed,
        });
        assert_eq!(e.param, RationaleParam::Feed);
        assert_eq!(e.reason, RationaleReason::ChiploadCapBound);
        assert!(e.headline.contains("machine max-feed"));
    }

    #[test]
    fn chipload_still_low_deflection_round_trips() {
        let e = rt(SuggestWarning::ChiploadStillLowAfterRecalibration {
            predicted_observed_mm_per_tooth: 0.005,
            lut_target_mm_per_tooth: 0.027,
            feed_at_termination_mm_per_min: 911.0,
            blocking_cap: FeedRecalibrationCap::DeflectionThreshold,
        });
        assert!(e.headline.contains("deflection budget"));
    }

    #[test]
    fn strategy_rewrote_entry_style_round_trips() {
        let e = rt(SuggestWarning::StrategyRewrote {
            param: "entry_style",
            from: "plunge".to_owned(),
            to: "ramp".to_owned(),
            reason: "deflection_predict_at_dpp",
        });
        assert_eq!(e.param, RationaleParam::EntryStyle);
        assert_eq!(e.reason, RationaleReason::GeometryClassifier);
        assert!(e.headline.contains("plunge"));
        assert!(e.headline.contains("ramp"));
        assert!(
            e.detail
                .as_deref()
                .unwrap_or_default()
                .contains("deflection_predict_at_dpp")
        );
    }

    #[test]
    fn strategy_rewrote_clearing_strategy_round_trips() {
        let e = rt(SuggestWarning::StrategyRewrote {
            param: "clearing_strategy",
            from: "agent_search".to_owned(),
            to: "contour_parallel".to_owned(),
            reason: "default_for_mixed_terrain",
        });
        assert_eq!(e.param, RationaleParam::ClearingStrategy);
        assert_eq!(e.reason, RationaleReason::GeometryClassifier);
    }

    #[test]
    fn strategy_recommended_not_applied_round_trips() {
        let e = rt(SuggestWarning::StrategyRecommendedNotApplied {
            param: "clearing_strategy",
            current: "contour_parallel".to_owned(),
            recommended: "adaptive".to_owned(),
            reason: "mixed_terrain_classifier",
        });
        assert_eq!(e.param, RationaleParam::ClearingStrategy);
        assert_eq!(e.reason, RationaleReason::GeometryClassifier);
        // Warn-only entries carry no numeric from/to.
        assert_eq!(e.from_value, None);
        assert_eq!(e.to_value, None);
        assert!(e.headline.contains("adaptive"));
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
            SuggestWarning::StepoverClampedToToolDiameter {
                requested: 8.0,
                capped: 6.0,
            },
        ]);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].param, RationaleParam::Plunge);
        assert_eq!(r.entries[1].param, RationaleParam::Stepover);
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
}
