//! Chipload guardrail — per-sample mm-per-tooth vs vendor-LUT bounds.
//!
//! Vendor LUT entries (Amana data, currently) report a chipload range
//! [`chipload_min_mm_tooth`, `chipload_max_mm_tooth`] for a given
//! (tool family, material family, operation, pass role) tuple. Below the
//! min, the cutter is rubbing the wood instead of slicing — burns the
//! cutting edge and the workpiece. Above the max, the chip is too thick
//! for the flute geometry to clear and the tooth breaks.
//!
//! This criterion samples the live simulation: each sample's
//! `effective_chip_thickness_mm` (the per-sample chip thickness exposed
//! by `dexel_stock::effective_chip_thickness_mm`, calibrated as the
//! arc-AVERAGE chip thickness across the engagement arc) is checked
//! against the LUT bounds. The peak deviation drives the verdict.
//! Both sides of the comparison must use the arc-average convention;
//! exposing peak instantaneous chip thickness overstates by ~2.6× at
//! half immersion and trips the breakage-risk gate on healthy cuts.
//!
//! **Steady-state filter (Item C).** Vendor LUT bounds are calibrated
//! against steady-state cutting at the operation's commanded feed.
//! Transient samples — plunge, ramp, helical entry, lead-in — run at
//! lower feeds (`plunge_rate`, ramp feed, etc.) and produce
//! correspondingly lower chipload-per-tooth. Comparing those low-feed
//! samples to the steady-state LUT range produces false `BurnRisk`
//! flags. We filter to samples whose feed is within 5% of the
//! commanded operation feed. If the steady-state set is empty (e.g. an
//! all-plunge drill cycle, where every sample runs at `plunge_rate`),
//! we return `Unmodeled(SteadyStateSamplesNotPresent)` rather than
//! flag against the wrong calibration.
//!
//! No vendor LUT row matches → `Unmodeled(NoVendorData)`. We refuse to
//! invent a chipload envelope from first principles.
//!
//! Provenance: this criterion needs per-sample data, so a missing
//! simulation trace yields `Unmodeled(SimulationRequired)`. Stale-trace
//! detection lands in Phase 2 with the `SimulationProvenance` hash; for
//! now any present trace is considered live.

use std::sync::OnceLock;

use crate::compute::catalog::OperationType;
use crate::feeds::ToolGeometryHint;
use crate::feeds::vendor_lookup::{LookupQuery, LookupResult, find_best_row};
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole, ToolFamily, VendorLut};
use crate::feeds::vendor_normalize::material_to_lut;
use crate::material::Material;
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::{MillingCutter, ToolDefinition};

/// D9 — `mean_chip / feed_per_tooth` for a flat endmill at the given
/// engagement arc. Mirrors `flat_chip_geometry_for_radius`'s mean-chip
/// formula in `crate::tool::flat_chip_geometry_for_radius`. Used to
/// renormalize the per-sample chip-thickness reading to the LUT row's
/// calibrated engagement before comparison.
///
/// At slot (`arc = π`): factor ≈ 0.637.
/// At half engagement (`arc = π/2`): factor ≈ 0.373.
/// At ~27 % radial (`arc ≈ 1.085`, the wanaka LUT row's nominal): ≈ 0.233.
fn mean_chip_factor(arc_rad: f64) -> f64 {
    let arc = arc_rad.clamp(1e-9, std::f64::consts::PI);
    let h_max = if arc >= std::f64::consts::PI {
        1.0
    } else {
        arc.sin().abs()
    };
    (2.0 * h_max / arc) * (1.0 - (arc * 0.5).cos())
}

/// D9 — derive the engagement arc (radians) the LUT row was authored
/// against, from its calibrated radial-engagement window
/// (`ae_min_mm`/`ae_max_mm`) and calibrated diameter. Returns `None`
/// when either field is missing — caller falls back to direct
/// (un-normalized) comparison.
///
/// For a flat endmill of diameter `D` with radial engagement `ae`,
/// `engagement_arc = acos(1 − 2·ae/D)`, bounded by `[0, π]`.
fn lut_nominal_arc_rad(row: &LookupResult) -> Option<f64> {
    let ae_min = row.ae_min_mm?;
    let ae_max = row.ae_max_mm?;
    if row.row_diameter_mm <= 0.0 {
        return None;
    }
    let ae_mid = (ae_min + ae_max) * 0.5;
    let ratio = (1.0 - 2.0 * ae_mid / row.row_diameter_mm).clamp(-1.0, 1.0);
    Some(ratio.acos())
}

/// Process-wide cache of the embedded Amana vendor LUT. The LUT is built
/// from `include_str!` data, so building it parses 5 JSON files; we do
/// that once.
static EMBEDDED_LUT: OnceLock<VendorLut> = OnceLock::new();

pub(super) fn embedded_lut() -> &'static VendorLut {
    EMBEDDED_LUT.get_or_init(VendorLut::embedded)
}

use super::verdict::{
    ChipBounds, ChipBoundsSource, ChipSide, ChiploadMetric, ChiploadStatistic, ChiploadVerdict,
    Confidence, SampleEvidence, UnmodeledReason,
};

/// Fraction of the commanded operation feed below which a sample is
/// considered to be running on a transient feed (plunge, ramp, lead-in)
/// rather than steady-state cutting.
///
/// 5% is loose enough to absorb sub-sample feed-integration noise but
/// tight enough to exclude common ramp feeds (typically 50% of cutting
/// feed) and plunge feeds (typically 10–30%).
pub(crate) const STEADY_STATE_FEED_FRACTION: f64 = 0.95;

/// Result of filtering a sim trace to in-cut, non-air, steady-state
/// samples for a single toolpath. Borrows the underlying trace.
pub(crate) struct SteadyStateSamples<'a> {
    /// `(global_index_into_trace, sample)` tuples for samples whose
    /// feed is within `STEADY_STATE_FEED_FRACTION` of the commanded
    /// operation feed. Empty for an all-transient toolpath (e.g. an
    /// all-plunge drill cycle).
    pub samples: Vec<(usize, &'a crate::simulation_cut::SimulationCutSample)>,
    /// `true` if at least one sample for this toolpath was in-cut and
    /// out of air, regardless of feed. Distinguishes "no usable cut
    /// samples at all" (SimulationRequired) from "samples exist but
    /// none meet the steady-state threshold" (SteadyStateSamplesNotPresent).
    pub any_in_cut: bool,
}

/// Fraction of valid steady-state samples that must fall below
/// `cl_min` (rubbing/burn) AND above `cl_max` (breakage) for the
/// bipolar predicate to fire. Looser than 1 sample (which would
/// trigger on a single transient noise sample) but tight enough to
/// catch real bipolar toolpaths where engagement varies wildly.
pub(crate) const BIPOLAR_SIDE_FRACTION: f64 = 0.05;

/// Detect bipolar engagement: steady-state samples for one toolpath
/// straddle both the LUT row's chipload-min and chipload-max bounds.
/// When true, no single feed/RPM scaling fixes both extremes —
/// raising feed clears burn but pushes more samples above breakage;
/// lowering feed clears breakage but pushes more samples below burn.
/// The user's lever is engagement variance (stepover / DOC / op
/// strategy), not feed/RPM.
///
/// Uses sample counts rather than the median so a 30%-below /
/// 40%-above toolpath isn't classified as Within by median collapse.
/// `BIPOLAR_SIDE_FRACTION` (5% of valid samples) on each side is the
/// threshold; both sides must clear it for the predicate to fire.
pub(crate) fn is_bipolar_engagement(
    steady_samples: &[(usize, &crate::simulation_cut::SimulationCutSample)],
    cl_min: f64,
    cl_max: f64,
) -> bool {
    let mut total: usize = 0;
    let mut below: usize = 0;
    let mut above: usize = 0;
    for (_, s) in steady_samples {
        let Some(cl) = s.effective_chip_thickness_mm else {
            continue;
        };
        total += 1;
        if cl < cl_min {
            below += 1;
        } else if cl > cl_max {
            above += 1;
        }
    }
    if total == 0 {
        return false;
    }
    let threshold = ((total as f64 * BIPOLAR_SIDE_FRACTION).ceil() as usize).max(1);
    below >= threshold && above >= threshold
}

/// Filter a sim trace down to the steady-state samples for one
/// toolpath, applying the same cutting/air/feed filters used by the
/// chipload gate. Extracted so the optimizer's pre-flight classifier
/// can read the same sample set the gate verdict was computed from
/// without duplicating the threshold constants.
pub(crate) fn steady_state_samples_for_toolpath<'a>(
    trace: &'a SimulationCutTrace,
    toolpath_id: usize,
    operation_feed_rate_mm_min: f64,
) -> SteadyStateSamples<'a> {
    let feed_threshold = STEADY_STATE_FEED_FRACTION * operation_feed_rate_mm_min;
    let mut any_in_cut = false;
    let samples = trace
        .samples
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            if s.toolpath_id != toolpath_id || !s.is_cutting || s.radial_engagement < 0.02 {
                return None;
            }
            any_in_cut = true;
            (s.feed_rate_mm_min >= feed_threshold).then_some((i, s))
        })
        .collect();
    SteadyStateSamples {
        samples,
        any_in_cut,
    }
}

/// Burn-risk verdict semantics.
///
/// Per-sample arc-average chip thickness collapses by definition at low
/// arc engagement (`mean = (2·feed/arc)·(1 - cos(arc/2)) → 0` as
/// `arc → 0`). On a real toolpath there are always *some* low-arc
/// transient samples (corner brushes, offset-ring entries, the first
/// cell of an arc-fit segment), and their effective_chip drops well
/// below the LUT min — but they aren't *rubbing* in the burn-risk
/// sense, they're just briefly at the edge of a cut.
///
/// Vendor LUT chip-load minima describe SUSTAINED cutting condition:
/// "the average chip a flute sees over its engaged time." The right
/// per-toolpath aggregate to compare to that is the **median** of
/// in-cut sample chip thicknesses. Using the median (rather than the
/// minimum) makes the verdict robust to ~50% transient samples without
/// missing a genuine "running too slow" condition where the cut is
/// SUSTAINED below min. BreakageRisk (above-max) stays per-sample peak
/// since a single overload bite is enough to break a tooth.
///
/// Evaluate the chipload criterion for a single toolpath.
///
/// `toolpath_id` matches `SimulationCutSample::toolpath_id` (the stable
/// id from `SimToolpathEntry::id`, not a project-relative index).
///
/// `operation_feed_rate_mm_min` is the toolpath's commanded operation
/// feed (`OperationConfig::feed_rate()`); samples below 95% of this
/// feed are filtered out as non-steady-state moves. See module doc.
///
/// `operation_kind` is the toolpath's `OperationType`. For most kinds
/// the (operation_family, pass_role) tuple from the operation spec is
/// what gets passed to the LUT lookup. Two kinds get rerouted by
/// `routed_lookup_family`: `ProjectCurve` (ball/tapered-ball → Parallel,
/// flat → Contour; v-bit / bull-nose return `Unmodeled(NoVendorData)`
/// — Item D of the tool-load fidelity plan) and `Adaptive3d`
/// (Adaptive → Pocket so the LUT envelope reflects pocket-style
/// clearing instead of 2D adaptive HSM — design doc §1.3, §10).
#[allow(clippy::too_many_arguments)]
pub fn evaluate(
    toolpath_id: usize,
    tool: &ToolDefinition,
    material: &Material,
    sim_trace: Option<&SimulationCutTrace>,
    operation_family: LutOperationFamily,
    pass_role: LutPassRole,
    operation_feed_rate_mm_min: f64,
    operation_kind: OperationType,
    tolerance: &super::ToleranceBands,
) -> ChiploadVerdict {
    // 1. Provenance gate.
    let Some(trace) = sim_trace else {
        return ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
    };

    // 2. Build the steady-state sample set before LUT lookup. The same
    // set gives us the operation's engaged lookup diameter.
    //
    // Skip rapids (`!is_cutting`) and air-cut samples (`radial_engagement
    // < 0.02` — same threshold as `SimulationCutIssueKind::AirCut` per
    // `simulation_cut.rs`). Air cuts have no real chip and produce
    // misleading chipload readings. Steady-state filter (Item C): only
    // count samples whose feed rate matches the commanded operation
    // feed; transient samples (plunge, ramp, lead-in) at lower feeds
    // get a separate non-decision rather than being measured against
    // the steady-state LUT envelope. See
    // `steady_state_samples_for_toolpath` for the canonical filter.
    let SteadyStateSamples {
        samples: steady_samples,
        any_in_cut: any_in_cut_for_toolpath,
    } = steady_state_samples_for_toolpath(trace, toolpath_id, operation_feed_rate_mm_min);

    // 3. Build verdicts for no usable sample data before attempting a LUT
    // lookup. This preserves the distinction between missing samples and
    // missing vendor data.
    if !any_in_cut_for_toolpath {
        return ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
    }
    if steady_samples.is_empty() {
        return ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::SteadyStateSamplesNotPresent,
        };
    }

    // 4. Look up the vendor envelope. If no row matches, refuse.
    let lookup_axial_doc_mm = steady_samples
        .iter()
        .map(|(_, s)| s.axial_doc_mm.max(0.0))
        .fold(0.0_f64, f64::max);
    let lut = embedded_lut();
    let geometry_hint = tool.to_geometry_hint();
    let tool_family = tool_family_for(geometry_hint);
    let Some((operation_family, pass_role)) =
        routed_lookup_family(operation_kind, tool_family, operation_family, pass_role)
    else {
        return ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::NoVendorData,
        };
    };
    let (material_family, hardness_kind, hardness_value) = material_to_lut(material);
    let query = LookupQuery {
        tool_family,
        tool_subfamily: None,
        diameter_mm: tool.lookup_diameter_at(lookup_axial_doc_mm),
        flute_count: tool.flute_count,
        material_family,
        hardness_kind: Some(hardness_kind),
        hardness_value: Some(hardness_value),
        operation_family,
        pass_role,
    };
    let Some(result) = find_best_row(lut, &query) else {
        return ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::NoVendorData,
        };
    };

    // 5. Bounds: upper bound is required. A missing lower bound means
    // burn/rubbing cannot be modeled for this row, not that we invent one.
    let (min, max) = match (result.chip_load_min_mm, result.chip_load_max_mm) {
        (Some(lo), Some(hi)) if lo > 0.0 && hi >= lo => (Some(lo), hi),
        (None, Some(hi)) if hi > 0.0 => (None, hi),
        _ => {
            return ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::NoVendorData,
            };
        }
    };
    let bounds = ChipBounds {
        min_mm_per_tooth: min,
        max_mm_per_tooth: max,
        source: if result.is_extrapolated {
            ChipBoundsSource::VendorLutExtrapolated
        } else {
            ChipBoundsSource::VendorLut
        },
    };
    // Confidence is `Approximate` whenever the matched row's chipload
    // bounds were extrapolated to the query's diameter / hardness past
    // ±40 %. The detail string carries both factors so the operator can
    // see how far the row was stretched.
    let chipload_confidence = if result.is_extrapolated {
        Confidence::Approximate(format!(
            "extrapolated from row {} (calibrated d={:.3}mm): diameter scale ×{:.2}, \
             hardness scale ×{:.2}",
            result.observation_id,
            result.row_diameter_mm,
            result.chipload_diameter_scale,
            result.chipload_hardness_scale,
        ))
    } else {
        Confidence::Validated
    };

    let mut peak_above: Option<(f64, usize)> = None;
    let mut peak_in_range: (f64, usize) = (0.0, 0);
    let mut valid_count: usize = 0;
    let mut missing_arc_count: usize = 0;
    let mut chip_geometry_unsupported_count: usize = 0;
    // Per-sample chip thicknesses for the per-toolpath median used by
    // the burn-risk verdict. See `Burn-risk verdict semantics` above.
    let mut burn_samples: Vec<(f64, usize)> = Vec::new();

    // D9 — engagement-aware normalization. Vendor LUT chip-load bounds
    // are authored at a specific engagement arc (Amana flat-endmill
    // roughing rows: ~17–37 % radial, arc ≈ 1.0–1.5 rad). Each sample's
    // `effective_chip_thickness_mm` is the arc-average mean chip the
    // dexel saw at *its* engagement arc, which can differ wildly (slot
    // engagement on terrain produces mean ≈ 0.637 × feed; LUT-nominal
    // ≈ 0.233 × feed). To compare on a common basis, scale each sample
    // to the LUT-nominal-arc-equivalent mean before comparing the cap.
    // When the LUT row lacks `ae_min_mm`/`ae_max_mm` (or a sample lacks
    // arc data), the normalization is skipped — the comparison falls
    // back to the raw mean. See planning/STRUCTURAL_ENTRY_SPANS_AND_LOCALITY.md
    // D2 for the calibration audit that motivated this.
    let arc_lut_nominal = lut_nominal_arc_rad(&result);
    let lut_factor = arc_lut_nominal.map(mean_chip_factor);

    for (i, s) in steady_samples {
        // Samples whose chip-thickness model didn't produce a value (e.g.
        // axial_doc = 0 transients on a 3D toolpath, or arc not captured)
        // are skipped, not fatal. Refuse only if zero steady samples
        // produced a usable chip thickness — that's a real "we can't
        // model this op" rather than a single noisy sample.
        let Some(cl) = s.effective_chip_thickness_mm else {
            if s.arc_engagement_radians.is_none() {
                missing_arc_count += 1;
            } else {
                chip_geometry_unsupported_count += 1;
            }
            continue;
        };
        // Normalize this sample to the LUT row's nominal engagement
        // arc. If either side lacks the data, fall back to the raw
        // mean (preserves prior behaviour for rows missing ae bounds).
        let cl_normalized = match (s.arc_engagement_radians, lut_factor) {
            (Some(arc_sample), Some(lut_f)) if lut_f > 0.0 => {
                let sample_factor = mean_chip_factor(arc_sample);
                if sample_factor > 0.0 {
                    cl * (lut_f / sample_factor)
                } else {
                    cl
                }
            }
            _ => cl,
        };
        valid_count += 1;
        burn_samples.push((cl_normalized, i));
        // Layer 1 tolerance band: a single sample 1-2% over `max` (e.g.
        // wanaka TP4 5/2026 transient at 1.05% over) shouldn't flip the
        // verdict to `Exceeds(High)`. The widened trigger is purely a
        // gate-trip decision; the underlying `peak_above` deviation is
        // still recorded so downstream displays surface the value.
        let max_trigger = max * (1.0 + tolerance.breakage);
        if cl_normalized > max_trigger {
            let dev = cl_normalized - max;
            if peak_above.is_none_or(|(prev, _)| dev > prev) {
                peak_above = Some((dev, i));
            }
        } else if cl_normalized > peak_in_range.0 {
            peak_in_range = (cl_normalized, i);
        }
    }

    // Burn-risk: median of per-sample chip thickness vs LUT min.
    // Sort once and re-use both for the median chip thickness and for
    // the within-case `approach_to_min` (same statistic, both arms).
    if !burn_samples.is_empty() {
        burn_samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }
    let median_sample: Option<(f64, usize)> = if burn_samples.is_empty() {
        None
    } else {
        let median_idx = burn_samples.len() / 2;
        #[allow(clippy::indexing_slicing)] // SAFETY: median_idx < len() by construction
        Some(burn_samples[median_idx])
    };
    // Layer 1 tolerance band: narrow the burn-risk trigger by
    // `burn_tolerance` so noisy LUT-min rows don't flip on a 1-2 %
    // shortfall. The deviation `min_value - median_cl` reported on the
    // verdict still uses the strict LUT min so downstream displays read
    // the real distance to the nominal envelope.
    let peak_below: Option<(f64, usize)> = if let Some(min_value) = min
        && let Some((median_cl, median_sample_idx)) = median_sample
        && median_cl < min_value * (1.0 - tolerance.burn)
    {
        Some((min_value - median_cl, median_sample_idx))
    } else {
        None
    };

    if valid_count == 0 {
        // All steady-state samples failed the chip-thickness model. Pick
        // the dominant failure reason for the verdict.
        let reason = if missing_arc_count >= chip_geometry_unsupported_count {
            UnmodeledReason::ArcEngagementNotCaptured
        } else {
            UnmodeledReason::CutterModeUnsupported(
                "chip geometry unsupported for sampled cutter engagement".to_owned(),
            )
        };
        return ChiploadVerdict::Unmodeled { reason };
    }

    // 6. Build verdict. Above-max takes priority over below-min: breakage is more
    // catastrophic than burn risk and we want it surfaced.
    let locality_for = |idx: usize| -> Option<String> {
        trace
            .samples
            .get(idx)
            .and_then(super::locality::classify_sample_locality)
    };
    if let Some((dev, idx)) = peak_above {
        return ChiploadVerdict::Exceeds {
            side: ChipSide::High,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: max + dev,
                statistic: ChiploadStatistic::PeakHigh,
                evidence: SampleEvidence::at_with_stat(idx, ChiploadStatistic::PeakHigh)
                    .with_locality(locality_for(idx)),
                bounds,
            },
            confidence: chipload_confidence,
        };
    }
    if let Some((dev, idx)) = peak_below {
        let observed = min.map(|m| m - dev).unwrap_or_default().max(0.0);
        return ChiploadVerdict::Exceeds {
            side: ChipSide::Low,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: observed,
                statistic: ChiploadStatistic::MedianLow,
                evidence: SampleEvidence::at_with_stat(idx, ChiploadStatistic::MedianLow)
                    .with_locality(locality_for(idx)),
                bounds,
            },
            confidence: chipload_confidence,
        };
    }

    // Within: report both bounds-approach metrics. `approach_to_min` is
    // None when the LUT row has no min; otherwise it carries the median
    // sample (same statistic the burn-risk arm uses).
    let approach_to_min = match (min, median_sample) {
        (Some(_), Some((median_cl, median_idx))) => Some(ChiploadMetric {
            observed_mm_per_tooth: median_cl,
            statistic: ChiploadStatistic::MedianLow,
            evidence: SampleEvidence::at_with_stat(median_idx, ChiploadStatistic::MedianLow)
                .with_locality(locality_for(median_idx)),
            bounds: bounds.clone(),
        }),
        _ => None,
    };
    let (peak_value, peak_idx) = peak_in_range;
    let approach_to_max = ChiploadMetric {
        observed_mm_per_tooth: peak_value,
        statistic: ChiploadStatistic::PeakInRange,
        evidence: if peak_value > 0.0 {
            SampleEvidence::at_with_stat(peak_idx, ChiploadStatistic::PeakInRange)
                .with_locality(locality_for(peak_idx))
        } else {
            SampleEvidence::empty()
        },
        bounds,
    };
    ChiploadVerdict::Within {
        approach_to_min,
        approach_to_max,
        confidence: chipload_confidence,
        // C1+C2 reverted 2026-05-10 (D3 Path A); D7 will repopulate
        // from span-aware entry detection.
        entry_spikes: Vec::new(),
    }
}

/// Map a cutter geometry hint to a vendor-LUT tool family. Mirrors
/// `feeds::vendor_normalize::to_lookup_query` so the same routing logic
/// runs for the chipload guardrail as for the F&S calculator.
///
/// Two operation kinds get rerouted away from their declared
/// `feeds_family`:
///
/// - `ProjectCurve` isn't a vendor LUT family in its own right; it's
///   geometrically a 3D contour-trace, so ball/tapered-ball tools route
///   to `(Parallel, Finish)` and flat tools to `(Contour, Finish)`.
///   V-bit / bull-nose / facing-bit project_curve toolpaths leave the
///   lookup unrouted (returns `None`).
/// - `Adaptive3d` declares `feeds_family: Adaptive` to share F&S inputs
///   with 2D adaptive HSM, but its path geometry is closer to pocket-
///   style clearing in wood. The vendor's 2D adaptive rows narrow
///   stepover by design (e.g. 0.95mm `ae_max` for a 6mm flat in
///   hardwood), while operators want 2.5–3mm stepover on Adaptive3d.
///   Route Adaptive3d to `Pocket` so the LUT envelope reflects the
///   actual mechanical regime. (G16 §10 sign-off, design doc §1.3.)
pub(crate) fn routed_lookup_family(
    operation_kind: OperationType,
    tool_family: ToolFamily,
    operation_family: LutOperationFamily,
    pass_role: LutPassRole,
) -> Option<(LutOperationFamily, LutPassRole)> {
    if operation_kind == OperationType::Adaptive3d
        && operation_family == LutOperationFamily::Adaptive
    {
        return Some((LutOperationFamily::Pocket, pass_role));
    }
    if operation_kind != OperationType::ProjectCurve {
        return Some((operation_family, pass_role));
    }
    match tool_family {
        ToolFamily::BallNose | ToolFamily::TaperedBallNose => {
            Some((LutOperationFamily::Parallel, LutPassRole::Finish))
        }
        ToolFamily::FlatEnd => Some((LutOperationFamily::Contour, LutPassRole::Finish)),
        ToolFamily::BullNose | ToolFamily::ChamferVbit | ToolFamily::FacingBit => None,
    }
}

pub(crate) fn tool_family_for(hint: ToolGeometryHint) -> ToolFamily {
    match hint {
        ToolGeometryHint::Flat => ToolFamily::FlatEnd,
        ToolGeometryHint::Ball => ToolFamily::BallNose,
        ToolGeometryHint::Bull { .. } => ToolFamily::BullNose,
        ToolGeometryHint::VBit { .. } => ToolFamily::ChamferVbit,
        ToolGeometryHint::TaperedBall { .. } => ToolFamily::TaperedBallNose,
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
    use crate::material::WoodSpecies;
    use crate::simulation_cut::{SimulationCutSample, SimulationCutSummary, SimulationCutTrace};
    use crate::tool::{FlatEndmill, VBitEndmill};

    fn tool() -> ToolDefinition {
        ToolDefinition::new(
            Box::new(FlatEndmill::new(6.35, 20.0)),
            6.35,
            30.0,
            20.0,
            30.0,
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        )
    }

    fn vbit_tool() -> ToolDefinition {
        ToolDefinition::new(
            Box::new(VBitEndmill::new(6.35, 90.0, 20.0)),
            6.35,
            30.0,
            20.0,
            30.0,
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        )
    }

    /// Engagement arc the test LUT row (HardMaple Pocket Roughing 6 mm
    /// flat) is calibrated against, so D9's per-sample normalization is
    /// a no-op for these fixtures and tests preserve their pre-D9
    /// semantics. Derived as `acos(1 − 2·ae_mid/D)` with `ae_min=1.0`,
    /// `ae_max=2.2`, `D=6.0` from
    /// `data/vendor_lut/observations/amana_flat_end.json` → ≈ 1.0844 rad.
    /// Tests that want to exercise normalization itself live in the
    /// dedicated `engagement_aware_normalization` module below.
    const TEST_LUT_NOMINAL_ARC_RAD: f64 = 1.0843860798928202;

    fn sample(tp_id: usize, idx: usize, chipload: f64, engagement: f64) -> SimulationCutSample {
        SimulationCutSample {
            toolpath_id: tp_id,
            move_index: idx,
            sample_index: idx,
            position: [0.0, 0.0, 0.0],
            cumulative_time_s: 0.0,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 1000.0,
            spindle_rpm: 18000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            radial_engagement: engagement,
            arc_engagement_radians: Some(TEST_LUT_NOMINAL_ARC_RAD),
            chipload_mm_per_tooth: chipload,
            effective_chip_thickness_mm: Some(chipload),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            semantic_item_id: None,
            span_path: Vec::new(),
        }
    }

    fn trace(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
        SimulationCutTrace {
            schema_version: 1,
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
                total_removed_volume_est_mm3: 1.0,
                average_mrr_mm3_s: 1.0,
            },
            toolpath_summaries: Vec::new(),
            semantic_summaries: Vec::new(),
            hotspots: Vec::new(),
            issues: Vec::new(),
            samples,
            provenance: None,
        }
    }

    #[test]
    fn project_curve_flat_routes_to_contour_finish() {
        // 6.35 mm flat tool against the 3.175 mm hardwood `contour/finish`
        // row diameter-scales chipload bounds by ~2.0×, so the sample
        // chipload must be in the scaled band (raw 0.018–0.030 → scaled
        // ≈ 0.032–0.054). The verdict is `Approximate` because the
        // diameter scale crosses the ±40 % threshold.
        // The contour-finish row's nominal arc is much narrower than the
        // pocket-roughing row used by the default `sample()` fixture
        // (`ae_rule: "2% to 8%D"` → arc ≈ 0.459 rad). Override here so
        // D9's per-sample normalization is a no-op for this test — the
        // assertion is about row selection, not engagement geometry.
        let mut s = sample(0, 0, 0.04, 0.5);
        s.arc_engagement_radians = Some(0.459);
        let t = trace(vec![s]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Trace,
            LutPassRole::Finish,
            1000.0,
            OperationType::ProjectCurve,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(
                v,
                ChiploadVerdict::Within {
                    confidence: Confidence::Approximate(_),
                    ..
                }
            ),
            "got {v:?}"
        );
    }

    #[test]
    fn project_curve_vbit_stays_unmodeled() {
        let t = trace(vec![sample(0, 0, 0.02, 0.5)]);
        let v = evaluate(
            0,
            &vbit_tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Trace,
            LutPassRole::Finish,
            1000.0,
            OperationType::ProjectCurve,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::NoVendorData
            }
        ));
    }

    #[test]
    fn adaptive3d_reroutes_from_adaptive_to_pocket_family() {
        // Adaptive3d's declared feeds_family is Adaptive, but its path
        // geometry is closer to pocket-style clearing in wood. The router
        // overrides Adaptive → Pocket so the LUT envelope reflects the
        // correct mechanical regime (design doc §1.3, §10 sign-off).
        let routed = routed_lookup_family(
            OperationType::Adaptive3d,
            ToolFamily::FlatEnd,
            LutOperationFamily::Adaptive,
            LutPassRole::Roughing,
        );
        assert_eq!(
            routed,
            Some((LutOperationFamily::Pocket, LutPassRole::Roughing))
        );
    }

    #[test]
    fn adaptive3d_reroute_preserves_pass_role() {
        // SemiFinish pass-role passes through unchanged (we only swap
        // the family axis, not the role axis).
        let routed = routed_lookup_family(
            OperationType::Adaptive3d,
            ToolFamily::FlatEnd,
            LutOperationFamily::Adaptive,
            LutPassRole::SemiFinish,
        );
        assert_eq!(
            routed,
            Some((LutOperationFamily::Pocket, LutPassRole::SemiFinish))
        );
    }

    #[test]
    fn adaptive3d_with_non_adaptive_family_passes_through() {
        // Defensive: the rule only fires when the incoming family is
        // Adaptive (the catalog default). Any other incoming family is
        // a custom override and must be respected.
        let routed = routed_lookup_family(
            OperationType::Adaptive3d,
            ToolFamily::FlatEnd,
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
        );
        assert_eq!(
            routed,
            Some((LutOperationFamily::Pocket, LutPassRole::Roughing))
        );
    }

    #[test]
    fn pocket_op_with_adaptive_family_passes_through() {
        // The Adaptive3d reroute is gated on operation_kind too — a
        // Pocket op asking for the Adaptive family stays Adaptive.
        let routed = routed_lookup_family(
            OperationType::Pocket,
            ToolFamily::FlatEnd,
            LutOperationFamily::Adaptive,
            LutPassRole::Roughing,
        );
        assert_eq!(
            routed,
            Some((LutOperationFamily::Adaptive, LutPassRole::Roughing))
        );
    }

    #[test]
    fn no_trace_returns_simulation_required() {
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            None,
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            } => {}
            other => panic!("expected Unmodeled(SimulationRequired), got {other:?}"),
        }
    }

    #[test]
    fn no_cutting_samples_returns_simulation_required() {
        // Sample exists but is_cutting=false → no in-cut data
        let mut s = sample(0, 0, 0.05, 0.5);
        s.is_cutting = false;
        let t = trace(vec![s]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired
            }
        ));
    }

    #[test]
    fn chipload_within_bounds_is_within_validated() {
        // 6.35mm flat in hard maple, pocket roughing: Amana LUT puts the
        // chipload range somewhere around 0.025–0.060 mm/tooth. Use 0.04.
        let t = trace(vec![sample(0, 0, 0.04, 0.5)]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(
                v,
                ChiploadVerdict::Within {
                    confidence: Confidence::Validated,
                    ..
                }
            ),
            "expected Within(Validated), got {v:?}"
        );
    }

    #[test]
    fn chipload_far_above_max_is_exceeds_breakage() {
        // 0.5 mm/tooth on a 6.35mm 2-flute end mill is absurdly high.
        let t = trace(vec![sample(0, 0, 0.5, 0.5)]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            ChiploadVerdict::Exceeds {
                side: ChipSide::High,
                triggering,
                ..
            } => {
                assert_eq!(triggering.statistic, ChiploadStatistic::PeakHigh);
            }
            other => panic!("expected Exceeds(High/PeakHigh), got {other:?}"),
        }
    }

    #[test]
    fn chipload_far_below_min_is_exceeds_burn() {
        // 0.001 mm/tooth — rubbing.
        let t = trace(vec![sample(0, 0, 0.001, 0.5)]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            ChiploadVerdict::Exceeds {
                side: ChipSide::Low,
                triggering,
                ..
            } => {
                assert_eq!(triggering.statistic, ChiploadStatistic::MedianLow);
            }
            other => panic!("expected Exceeds(Low/MedianLow), got {other:?}"),
        }
    }

    #[test]
    fn samples_for_other_toolpath_are_ignored() {
        // Toolpath 1 has a wildly out-of-bounds sample, but we're
        // evaluating toolpath 0 which has a normal sample.
        let t = trace(vec![sample(0, 0, 0.04, 0.5), sample(1, 1, 0.5, 0.5)]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(v, ChiploadVerdict::Within { .. }));
    }

    #[test]
    fn air_cut_samples_are_skipped() {
        // Engagement 0.01 is below the 0.02 air-cut threshold; the
        // exorbitant chipload should be ignored as a phantom reading.
        let t = trace(vec![sample(0, 0, 0.5, 0.01), sample(0, 1, 0.04, 0.5)]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(v, ChiploadVerdict::Within { .. }));
    }

    // -------- Item C: steady-state feed-rate filter ---------

    /// Item C verify #1: a toolpath whose every in-cut sample runs at a
    /// plunge feed below 95% of the commanded cutting feed (e.g. a pure
    /// drill cycle whose plunge_rate is 30% of feed_rate) returns
    /// `Unmodeled(SteadyStateSamplesNotPresent)` rather than measuring
    /// the plunge-feed chipload against the steady-state LUT range.
    /// Mirrors the wanaka TP 7 false-BurnRisk symptom.
    #[test]
    fn all_plunge_samples_returns_steady_state_not_present() {
        // commanded operation feed = 1000 mm/min; every sample at the
        // 300 mm/min plunge rate (= 0.30 × 1000, well below the 0.95
        // threshold) and at a chipload that would otherwise read as
        // BurnRisk against the LUT.
        let mut s0 = sample(0, 0, 0.0083, 0.5);
        s0.feed_rate_mm_min = 300.0;
        let mut s1 = sample(0, 1, 0.0083, 0.5);
        s1.feed_rate_mm_min = 300.0;
        let t = trace(vec![s0, s1]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::SteadyStateSamplesNotPresent,
            } => {}
            other => panic!("expected Unmodeled(SteadyStateSamplesNotPresent), got {other:?}"),
        }
    }

    /// Item C verify #2: a mix of steady-feed Linear, steady-feed Helix,
    /// and ramp-feed Linear samples — the peak must be computed only over
    /// the steady-feed samples. The ramp-feed sample carries an
    /// otherwise-flagworthy chipload that must be ignored.
    /// Mirrors the wanaka TP 10 false-BurnRisk symptom.
    #[test]
    fn ramp_feed_samples_excluded_from_peak() {
        // commanded feed 1500. Linear + Helix at 1500 mm/min carry an
        // in-range chipload (0.04). Ramp at 500 mm/min carries 0.001
        // (would trigger BurnRisk). Filter must drop the ramp sample.
        let mut linear = sample(0, 0, 0.04, 0.5);
        linear.feed_rate_mm_min = 1500.0;
        linear.cut_kinematics = crate::simulation_cut::CutKinematics::Linear;
        let mut helix = sample(0, 1, 0.04, 0.5);
        helix.feed_rate_mm_min = 1500.0;
        helix.cut_kinematics = crate::simulation_cut::CutKinematics::Helix;
        let mut ramp = sample(0, 2, 0.001, 0.5);
        ramp.feed_rate_mm_min = 500.0;
        ramp.cut_kinematics = crate::simulation_cut::CutKinematics::Linear;
        let t = trace(vec![linear, helix, ramp]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1500.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(
                v,
                ChiploadVerdict::Within {
                    confidence: Confidence::Validated,
                    ..
                }
            ),
            "expected Within(Validated), got {v:?}"
        );
    }

    /// G17 C1+C2 reverted 2026-05-10 (D3 Path A): Helix samples are no
    /// longer routed to an entry-spike advisory — D0 confirmed Helix
    /// kinematics is almost entirely terrain-following on adaptive3d,
    /// not configured entry. So a Helix sample above LUT max trips
    /// Exceeds again, the same as Linear.
    #[test]
    fn helix_high_sample_trips_exceeds() {
        let mut s0 = sample(0, 0, 0.5, 0.5);
        s0.feed_rate_mm_min = 1500.0;
        s0.cut_kinematics = crate::simulation_cut::CutKinematics::Helix;
        let mut s1 = sample(0, 1, 0.5, 0.5);
        s1.feed_rate_mm_min = 1500.0;
        s1.cut_kinematics = crate::simulation_cut::CutKinematics::Helix;
        let t = trace(vec![s0, s1]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1500.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(
                v,
                ChiploadVerdict::Exceeds {
                    side: ChipSide::High,
                    ..
                }
            ),
            "Helix high sample must trip Exceeds post-revert, got {v:?}"
        );
    }

    /// Item C edge case: a sample at exactly 95% of the commanded feed
    /// is included (boundary inclusive). A sample at 94.9% is excluded.
    #[test]
    fn feed_threshold_is_inclusive_at_95pct() {
        // Sample at exactly 950 mm/min (= 0.95 × 1000) must be kept;
        // value below would otherwise be filtered.
        let mut s = sample(0, 0, 0.04, 0.5);
        s.feed_rate_mm_min = 950.0;
        let t = trace(vec![s]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(v, ChiploadVerdict::Within { .. }),
            "exactly-95% sample must be kept, got {v:?}"
        );
    }

    /// Sub-segments where the simulator measures `axial_doc_mm = 0` (e.g.
    /// the cutter just kissing the surface on a 3D toolpath, or transient
    /// dexel-grid edge cases on project_curve) cause the chip-thickness
    /// model to return `OutOfRange`, which the simulator surfaces as
    /// `effective_chip_thickness_mm = None`. A single such sample mixed
    /// in with otherwise-valid steady-state samples must NOT abort the
    /// verdict — only refuse when zero valid samples remain.
    #[test]
    fn missing_chip_thickness_samples_are_skipped_not_fatal() {
        // 2 valid samples + 1 sample with effective_chip_thickness_mm = None.
        let valid_a = sample(0, 0, 0.04, 0.5);
        let valid_b = sample(0, 1, 0.04, 0.5);
        let mut noise = sample(0, 2, 0.04, 0.5);
        noise.effective_chip_thickness_mm = None;
        // arc_engagement is still Some — simulating a chip_geometry Err
        // case rather than a missing-arc case.
        let t = trace(vec![valid_a, valid_b, noise]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(v, ChiploadVerdict::Within { .. }),
            "one None sample must be skipped, not abort the verdict, got {v:?}"
        );
    }

    /// Counterpart to the above: if EVERY steady-state sample has a
    /// missing chip thickness, the verdict refuses with the dominant
    /// failure reason (preserving refusal-first when the model genuinely
    /// can't see the cut).
    #[test]
    fn all_missing_chip_thickness_refuses() {
        let mut s0 = sample(0, 0, 0.04, 0.5);
        s0.effective_chip_thickness_mm = None;
        let mut s1 = sample(0, 1, 0.04, 0.5);
        s1.effective_chip_thickness_mm = None;
        let t = trace(vec![s0, s1]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(
                v,
                ChiploadVerdict::Unmodeled {
                    reason: UnmodeledReason::CutterModeUnsupported(_)
                }
            ),
            "all-None samples must refuse with CutterModeUnsupported, got {v:?}"
        );
    }

    /// Item C edge case: existing `no_cutting_samples` semantics
    /// preserved — a toolpath with zero in-cut samples (every sample is
    /// `is_cutting=false`) still reports `SimulationRequired`, not
    /// `SteadyStateSamplesNotPresent`. The two reasons describe
    /// different failure modes; the gate may render them differently.
    #[test]
    fn no_in_cut_samples_takes_priority_over_steady_state_reason() {
        let mut s = sample(0, 0, 0.05, 0.5);
        s.is_cutting = false;
        // even at the commanded feed, the sample isn't in cut.
        s.feed_rate_mm_min = 1000.0;
        let t = trace(vec![s]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired
            }
        ));
    }

    // ── Bipolar engagement predicate ─────────────────────────────────

    #[test]
    fn bipolar_fires_when_both_extremes_populated() {
        // 30 samples: 5 below cl_min=0.02, 5 above cl_max=0.10, rest in
        // range. 5 / 30 = 16.7%, well above the 5% per-side threshold.
        let mut samples = Vec::new();
        for i in 0..5 {
            samples.push(sample(0, i, 0.005, 0.5));
        }
        for i in 0..5 {
            samples.push(sample(0, 100 + i, 0.15, 0.5));
        }
        for i in 0..20 {
            samples.push(sample(0, 200 + i, 0.05, 0.5));
        }
        let refs: Vec<_> = samples.iter().enumerate().collect();
        assert!(is_bipolar_engagement(&refs, 0.02, 0.10));
    }

    #[test]
    fn bipolar_does_not_fire_when_only_below_min() {
        // 10 samples below cl_min, 0 above cl_max → not bipolar (just
        // burn risk, fixable by raising feed).
        let mut samples = Vec::new();
        for i in 0..10 {
            samples.push(sample(0, i, 0.005, 0.5));
        }
        for i in 0..20 {
            samples.push(sample(0, 100 + i, 0.05, 0.5));
        }
        let refs: Vec<_> = samples.iter().enumerate().collect();
        assert!(!is_bipolar_engagement(&refs, 0.02, 0.10));
    }

    #[test]
    fn bipolar_does_not_fire_when_only_above_max() {
        // 10 samples above cl_max, 0 below cl_min → not bipolar (just
        // breakage risk, fixable by lowering feed).
        let mut samples = Vec::new();
        for i in 0..10 {
            samples.push(sample(0, i, 0.20, 0.5));
        }
        for i in 0..20 {
            samples.push(sample(0, 100 + i, 0.05, 0.5));
        }
        let refs: Vec<_> = samples.iter().enumerate().collect();
        assert!(!is_bipolar_engagement(&refs, 0.02, 0.10));
    }

    #[test]
    fn bipolar_ignores_single_transient_samples_below_threshold() {
        // 1 sample below + 1 sample above out of 100 → 1% on each side,
        // below the 5% threshold. Single transient samples (corner
        // brush, lead-in) shouldn't trip bipolar.
        let mut samples = Vec::new();
        samples.push(sample(0, 0, 0.005, 0.5));
        samples.push(sample(0, 1, 0.20, 0.5));
        for i in 0..98 {
            samples.push(sample(0, 100 + i, 0.05, 0.5));
        }
        let refs: Vec<_> = samples.iter().enumerate().collect();
        assert!(!is_bipolar_engagement(&refs, 0.02, 0.10));
    }

    #[test]
    fn bipolar_returns_false_for_empty_samples() {
        let samples: Vec<(usize, &SimulationCutSample)> = Vec::new();
        assert!(!is_bipolar_engagement(&samples, 0.02, 0.10));
    }

    #[test]
    fn bipolar_skips_samples_without_chip_thickness() {
        // Samples whose effective_chip_thickness is None don't count
        // toward the total — they aren't classified.
        let mut s_below = sample(0, 0, 0.005, 0.5);
        s_below.effective_chip_thickness_mm = None;
        let mut s_above = sample(0, 1, 0.20, 0.5);
        s_above.effective_chip_thickness_mm = None;
        let in_range = sample(0, 2, 0.05, 0.5);
        let samples = [s_below, s_above, in_range];
        let refs: Vec<_> = samples.iter().enumerate().collect();
        assert!(!is_bipolar_engagement(&refs, 0.02, 0.10));
    }

    /// Resolve the LUT chipload max for the 6.35 mm flat / HardMaple /
    /// Pocket / Roughing fixture by running a known-Within evaluation and
    /// reading the bounds the verdict carries. Lets the tolerance-band
    /// tests below scale their probe samples relative to the actual row,
    /// so future LUT changes don't break the test setup.
    fn pocket_rough_lut_max() -> f64 {
        let t = trace(vec![sample(0, 0, 0.04, 0.5)]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            ChiploadVerdict::Within {
                approach_to_max, ..
            } => approach_to_max.bounds.max_mm_per_tooth,
            other => panic!("expected Within, got {other:?}"),
        }
    }

    fn pocket_rough_lut_min() -> f64 {
        let t = trace(vec![sample(0, 0, 0.04, 0.5)]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            ChiploadVerdict::Within {
                approach_to_max, ..
            } => approach_to_max
                .bounds
                .min_mm_per_tooth
                .expect("LUT row carries a min for the chosen fixture"),
            other => panic!("expected Within, got {other:?}"),
        }
    }

    /// Wanaka TP4 5/2026: a single transient sample 1.05 % above LUT max
    /// flipped the verdict to `Exceeds(High)` and killed every roughing
    /// optimization candidate. Layer 1's `breakage_tolerance` widens the
    /// trigger by 5 %, so 4 % over now stays Within.
    #[test]
    fn chipload_high_just_above_max_within_tolerance_is_within() {
        let max = pocket_rough_lut_max();
        let probe = max * 1.04;
        let t = trace(vec![sample(0, 0, probe, 0.5)]);
        let bands = crate::tool_load::ToleranceBands {
            breakage: 0.05,
            ..crate::tool_load::ToleranceBands::default()
        };
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &bands,
        );
        assert!(
            matches!(v, ChiploadVerdict::Within { .. }),
            "expected Within with breakage_tolerance=0.05 at 1.04×max, got {v:?}"
        );
    }

    /// Mirror of the above: a sample 6 % over LUT max still exceeds the
    /// 5 % tolerance band and must trip `Exceeds(High)`.
    #[test]
    fn chipload_high_above_tolerance_is_exceeds() {
        let max = pocket_rough_lut_max();
        let probe = max * 1.06;
        let t = trace(vec![sample(0, 0, probe, 0.5)]);
        let bands = crate::tool_load::ToleranceBands {
            breakage: 0.05,
            ..crate::tool_load::ToleranceBands::default()
        };
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &bands,
        );
        assert!(
            matches!(
                v,
                ChiploadVerdict::Exceeds {
                    side: ChipSide::High,
                    ..
                }
            ),
            "expected Exceeds(High) at 1.06×max with breakage_tolerance=0.05, got {v:?}"
        );
    }

    #[test]
    fn chipload_low_just_below_min_within_tolerance_is_within() {
        let min = pocket_rough_lut_min();
        let probe = min * 0.96;
        let t = trace(vec![sample(0, 0, probe, 0.5)]);
        let bands = crate::tool_load::ToleranceBands {
            burn: 0.05,
            ..crate::tool_load::ToleranceBands::default()
        };
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &bands,
        );
        assert!(
            matches!(v, ChiploadVerdict::Within { .. }),
            "expected Within with burn_tolerance=0.05 at 0.96×min, got {v:?}"
        );
    }

    #[test]
    fn chipload_low_below_tolerance_is_exceeds() {
        let min = pocket_rough_lut_min();
        let probe = min * 0.94;
        let t = trace(vec![sample(0, 0, probe, 0.5)]);
        let bands = crate::tool_load::ToleranceBands {
            burn: 0.05,
            ..crate::tool_load::ToleranceBands::default()
        };
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &bands,
        );
        assert!(
            matches!(
                v,
                ChiploadVerdict::Exceeds {
                    side: ChipSide::Low,
                    ..
                }
            ),
            "expected Exceeds(Low) at 0.94×min with burn_tolerance=0.05, got {v:?}"
        );
    }

    // ── Gate-trip behavior across kinematics (G17 C1+C2 reverted) ────────

    /// A Linear (steady-state) sample over LUT max trips Exceeds.
    #[test]
    fn linear_steady_state_high_sample_trips_exceeds() {
        // Uses pocket-rough LUT max (0.058208…). 0.5 mm/tooth >> max.
        let mut s = sample(0, 0, 0.5, 0.5);
        s.feed_rate_mm_min = 1500.0;
        s.cut_kinematics = crate::simulation_cut::CutKinematics::Linear;
        let t = trace(vec![s]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1500.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(
                v,
                ChiploadVerdict::Exceeds {
                    side: ChipSide::High,
                    ..
                }
            ),
            "Linear steady-state high sample must trip Exceeds, got {v:?}"
        );
    }

    /// G17 C1+C2 reverted 2026-05-10 (D3 Path A): Plunge samples no
    /// longer skip the trip. A Plunge sample over LUT max trips
    /// Exceeds — the same as Linear/Helix. (Real adaptive3d Plunge
    /// samples emit no chip-thickness, so this case mostly matters for
    /// other op families that do model plunge chip thickness.)
    #[test]
    fn plunge_high_sample_trips_exceeds() {
        let mut s = sample(0, 0, 0.5, 0.5);
        s.feed_rate_mm_min = 1500.0;
        s.cut_kinematics = crate::simulation_cut::CutKinematics::Plunge;
        let t = trace(vec![s]);
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1500.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(
                v,
                ChiploadVerdict::Exceeds {
                    side: ChipSide::High,
                    ..
                }
            ),
            "Plunge high sample must trip Exceeds post-revert, got {v:?}"
        );
    }
}
