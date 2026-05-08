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
use crate::feeds::vendor_lookup::{LookupQuery, find_best_row};
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole, ToolFamily, VendorLut};
use crate::feeds::vendor_normalize::material_to_lut;
use crate::material::Material;
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::{MillingCutter, ToolDefinition};

/// Process-wide cache of the embedded Amana vendor LUT. The LUT is built
/// from `include_str!` data, so building it parses 5 JSON files; we do
/// that once.
static EMBEDDED_LUT: OnceLock<VendorLut> = OnceLock::new();

pub(super) fn embedded_lut() -> &'static VendorLut {
    EMBEDDED_LUT.get_or_init(VendorLut::embedded)
}

use super::verdict::{Confidence, ExceedsReason, UnmodeledReason, Verdict};

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
/// what gets passed to the LUT lookup. `OperationType::ProjectCurve`
/// is a special case: ProjectCurve isn't a vendor LUT family in its
/// own right but is geometrically a 3D contour-trace operation, so
/// for ball/tapered-ball tools we route it to (Parallel, Finish) and
/// for flat tools to (Contour, Finish). V-bit and bull-nose
/// project_curve toolpaths leave the lookup unrouted and return
/// `Unmodeled(NoVendorData)` (Item D of the tool-load fidelity plan).
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
) -> Verdict {
    // 1. Provenance gate.
    let Some(trace) = sim_trace else {
        return Verdict::Unmodeled {
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
        return Verdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
    }
    if steady_samples.is_empty() {
        return Verdict::Unmodeled {
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
        return Verdict::Unmodeled {
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
        return Verdict::Unmodeled {
            reason: UnmodeledReason::NoVendorData,
        };
    };

    // 5. Bounds: upper bound is required. A missing lower bound means
    // burn/rubbing cannot be modeled for this row, not that we invent one.
    let (min, max) = match (result.chip_load_min_mm, result.chip_load_max_mm) {
        (Some(lo), Some(hi)) if lo > 0.0 && hi >= lo => (Some(lo), hi),
        (None, Some(hi)) if hi > 0.0 => (None, hi),
        _ => {
            return Verdict::Unmodeled {
                reason: UnmodeledReason::NoVendorData,
            };
        }
    };

    let mut peak_above: Option<(f64, usize)> = None;
    let mut peak_in_range: f64 = 0.0;
    let mut valid_count: usize = 0;
    let mut missing_arc_count: usize = 0;
    let mut chip_geometry_unsupported_count: usize = 0;
    // Per-sample chip thicknesses for the per-toolpath median used by
    // the burn-risk verdict. See `Burn-risk verdict semantics` above.
    let mut burn_samples: Vec<(f64, usize)> = Vec::new();

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
        valid_count += 1;
        burn_samples.push((cl, i));
        if cl > max {
            let dev = cl - max;
            if peak_above.is_none_or(|(prev, _)| dev > prev) {
                peak_above = Some((dev, i));
            }
        } else if cl > peak_in_range {
            peak_in_range = cl;
        }
    }

    // Burn-risk: median of per-sample chip thickness vs LUT min.
    let peak_below: Option<(f64, usize)> = if let Some(min) = min
        && !burn_samples.is_empty()
    {
        burn_samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let median_idx = burn_samples.len() / 2;
        #[allow(clippy::indexing_slicing)] // SAFETY: median_idx < len() by construction
        let (median_cl, median_sample_idx) = burn_samples[median_idx];
        if median_cl < min {
            Some((min - median_cl, median_sample_idx))
        } else {
            None
        }
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
        return Verdict::Unmodeled { reason };
    }

    // 6. Build verdict. Above-max takes priority over below-min: breakage is more
    // catastrophic than burn risk and we want it surfaced.
    if let Some((dev, idx)) = peak_above {
        return Verdict::Exceeds {
            peak: max + dev,
            sample_range: idx..(idx + 1),
            reason: ExceedsReason::ChiploadBreakageRisk,
            confidence: Confidence::Validated,
        };
    }
    if let Some((dev, idx)) = peak_below {
        return Verdict::Exceeds {
            peak: min.map(|min| min - dev).unwrap_or_default().max(0.0),
            sample_range: idx..(idx + 1),
            reason: ExceedsReason::ChiploadBurnRisk,
            confidence: Confidence::Validated,
        };
    }
    Verdict::Within {
        peak: peak_in_range,
        confidence: Confidence::Validated,
    }
}

/// Map a cutter geometry hint to a vendor-LUT tool family. Mirrors
/// `feeds::vendor_normalize::to_lookup_query` so the same routing logic
/// runs for the chipload guardrail as for the F&S calculator.
pub(crate) fn routed_lookup_family(
    operation_kind: OperationType,
    tool_family: ToolFamily,
    operation_family: LutOperationFamily,
    pass_role: LutPassRole,
) -> Option<(LutOperationFamily, LutPassRole)> {
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
            arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
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
        let t = trace(vec![sample(0, 0, 0.02, 0.5)]);
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
        );
        assert!(matches!(v, Verdict::Within { .. }), "got {v:?}");
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
        );
        assert!(matches!(
            v,
            Verdict::Unmodeled {
                reason: UnmodeledReason::NoVendorData
            }
        ));
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
        );
        match v {
            Verdict::Unmodeled {
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
        );
        assert!(matches!(
            v,
            Verdict::Unmodeled {
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
        );
        assert!(
            matches!(
                v,
                Verdict::Within {
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
        );
        match v {
            Verdict::Exceeds {
                reason: ExceedsReason::ChiploadBreakageRisk,
                ..
            } => {}
            other => panic!("expected Exceeds(ChiploadBreakageRisk), got {other:?}"),
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
        );
        match v {
            Verdict::Exceeds {
                reason: ExceedsReason::ChiploadBurnRisk,
                ..
            } => {}
            other => panic!("expected Exceeds(ChiploadBurnRisk), got {other:?}"),
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
        );
        assert!(matches!(v, Verdict::Within { .. }));
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
        );
        assert!(matches!(v, Verdict::Within { .. }));
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
        );
        match v {
            Verdict::Unmodeled {
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
        );
        assert!(
            matches!(
                v,
                Verdict::Within {
                    confidence: Confidence::Validated,
                    ..
                }
            ),
            "expected Within(Validated), got {v:?}"
        );
    }

    /// Item C verify #3: pure-Helix steady-state samples on a sloped 3D
    /// cut (the canonical adaptive3d / drop_cutter pattern, where the
    /// simulator tags every XY+Z move as `Helix`) are *kept* by the
    /// filter as long as their feed matches the commanded operation
    /// feed. This is the negative-of-Item-C-original case: filtering on
    /// `cut_kinematics == Linear` would have wrongly discarded these.
    #[test]
    fn helix_steady_state_samples_are_kept() {
        // All Helix at the commanded feed; a chipload above max would
        // surface as Exceeds(Breakage). If the filter wrongly dropped
        // Helix samples we'd see SteadyStateSamplesNotPresent instead.
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
        );
        match v {
            Verdict::Exceeds {
                reason: ExceedsReason::ChiploadBreakageRisk,
                ..
            } => {}
            other => panic!(
                "expected Exceeds(ChiploadBreakageRisk) — Helix samples must be kept by the filter, got {other:?}"
            ),
        }
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
        );
        assert!(
            matches!(v, Verdict::Within { .. }),
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
        );
        assert!(
            matches!(v, Verdict::Within { .. }),
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
        );
        assert!(
            matches!(
                v,
                Verdict::Unmodeled {
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
        );
        assert!(matches!(
            v,
            Verdict::Unmodeled {
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
}
