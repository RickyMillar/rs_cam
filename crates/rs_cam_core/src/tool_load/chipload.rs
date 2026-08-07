//! Chipload guardrail — per-sample advance-per-tooth vs vendor-LUT bounds.
//!
//! Vendor LUT entries report a chipload range
//! [`chipload_min_mm_tooth`, `chipload_max_mm_tooth`] for a given
//! (tool family, material family, operation, pass role) tuple. Below the
//! min, the cutter is rubbing the wood instead of slicing — burns the
//! cutting edge and the workpiece. Above the max, the chip is too thick
//! for the flute geometry to clear and the tooth breaks.
//!
//! ## The unit — settled 2026-08-06, and what it deleted
//!
//! `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` answered
//! Checkpoint B item T4.1 from primary sources: **every source family in
//! the shipped LUT publishes its chipload column as a linear ADVANCE PER
//! TOOTH** — `feed_rate ÷ (rpm × cutting edges)`. Onsrud, Freud, Amana
//! and Garr print that identity on the same page as the column, and the
//! Amana chart behind the live B3 row publishes IPM and chip load side
//! by side at a fixed 18,000 RPM, so the identity is self-verifying to
//! the chart's own printed precision (verdict §2.2).
//!
//! Until that date this gate observed something else: each sample's
//! `effective_chip_thickness_mm` — an arc-AVERAGE chip thickness —
//! renormalised (D9) to an engagement arc derived from the matched row's
//! `ae_min_mm`/`ae_max_mm` window. Two things were wrong with it, and
//! only the second is obvious:
//!
//! 1. It compared a chip thickness against a band of advance. That is
//!    census F-1, and on the live B3 op it read the operation as sitting
//!    at **16 % of the band minimum** when it was in fact at **99.9 % of
//!    the band maximum**.
//! 2. The `ae` window it renormalised *to* is not a vendor measurement
//!    condition. Verdict §2.3: no wood chart in the LUT publishes a
//!    radial-engagement condition for its chipload column at all — all
//!    three that state a condition state an axial one — and every
//!    `ae`-bearing wood row's `ae_rule` is a repo-authored application
//!    window (`"scallop driven"`, `"10% to 30%D"`, `"width-at-depth"`).
//!    On the B3 row the midpoint was 8.3 % of the tool because the rule
//!    is a *finish-pass stepover*.
//!
//! The census had already proved (§4.3, pinned by
//! `tests/feed_explanation_snapshot_b3.rs`) that the sample's own arc
//! cancels out of the normalisation — `cl_norm = fz · f(arc_sample) ·
//! f_lut / f(arc_sample) = fz · f_lut` — so the whole chip-geometry
//! chain was a constant factor on the commanded advance. The correction
//! is therefore a **deletion, not an inversion**, and this gate now
//! observes exactly what the band publishes:
//!
//! ```text
//! observed = effective_feed_for_sample(s) / (rpm · flutes)
//! ```
//!
//! `effective_feed_for_sample` is the F-035 kinematics substitution, so
//! the number is what the machine will *achieve*, not what the operator
//! typed. The commanded value is the same expression at commanded feed
//! and is reported beside it on `feeds::FeedExplanation`; their ratio is
//! the kinematic throttle, which the verdict document names as the
//! operator-actionable fact.
//!
//! **The sim remains the operational arbiter.** This change makes
//! Suggest, the narration and this gate speak one unit; it does not
//! promote a static band over a measured cut.
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

use crate::compute::catalog::OperationType;
use crate::feeds::vendor_lookup::{LookupQuery, LookupResult, find_best_chip_envelope_row};
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole, ToolFamily};
use crate::feeds::vendor_normalize::material_to_lut;
use crate::ids::ToolpathId;
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::MillingCutter;

use super::locality::SpanLookup;

/// F3.4 (defect class C5) — THE canonical chipload-envelope row
/// resolution. Pre-F3.4 four call sites re-derived this independently
/// (the gate below, the optimizer context's `find_matched_lut_row`,
/// the viewport envelope map, the preflight/retargeter via the
/// optimizer row) — with three different lookup strategies (angle-aware
/// vs first-of-enumerate vs first-match), so advice could be computed
/// against a different envelope than the verdict.
///
/// Semantics:
/// - `Custom` material → `None` (no validated LUT category).
/// - Operation routing via [`routed_lookup_family`] (ProjectCurve /
///   Adaptive3d reroutes).
/// - `lookup_diameter_mm` is the caller's engaged-diameter choice —
///   the gate uses `lookup_diameter_at(peak steady-state DOC)`, the
///   optimizer `diameter_for_lut_lookup(tool, commanded DOC)` (nominal
///   fallback when no DOC is commanded).
/// - Angle-aware dispatch for V-bit / chamfer geometry.
/// - Only chipload-bearing rows compete
///   ([`find_best_chip_envelope_row`]) — RPM-only rows are feeds-
///   calculator anchors, not envelopes.
pub(crate) fn matched_chip_envelope(
    tool: &crate::tool::ToolDefinition,
    material: &crate::material::Material,
    operation_kind: OperationType,
    operation_family: LutOperationFamily,
    pass_role: LutPassRole,
    lookup_diameter_mm: f64,
) -> Option<LookupResult> {
    if matches!(material, crate::material::Material::Custom { .. }) {
        return None;
    }
    let geometry_hint = tool.to_geometry_hint();
    let tool_family = geometry_hint.cutter_kind().lut_family();
    let (operation_family, pass_role) =
        routed_lookup_family(operation_kind, tool_family, operation_family, pass_role)?;
    let (material_family, hardness_kind, hardness_value) = material_to_lut(material);
    let diameter_mm = lookup_diameter_mm;
    let query = LookupQuery {
        tool_family,
        tool_subfamily: None,
        diameter_mm,
        flute_count: tool.flute_count,
        material_family,
        hardness_kind: Some(hardness_kind),
        hardness_value: Some(hardness_value),
        operation_family,
        pass_role,
    };
    find_best_chip_envelope_row(embedded_lut(), &query, &geometry_hint)
}

/// The gate's observed quantity: the **advance per tooth this sample
/// actually achieves**, `effective_feed / (rpm · flutes)`.
///
/// This is the whole of the conversion the B-lit verdict authorises.
/// What it replaced — `mean_chip_factor(arc)` and `lut_nominal_arc_rad`,
/// the D9 chip-geometry pair — is deleted rather than inverted, for the
/// reasons in this module's header. `git show` at the parent of this
/// commit is the record of what they were; nothing in the crate needs
/// them any more.
///
/// Returns `None` when the sample carries no usable `rpm × flutes`
/// divisor, which is a broken sample rather than a modelling limit.
fn achieved_feed_per_tooth_mm(
    sample: &crate::simulation_cut::SimulationCutSample,
    predicted_feeds: &crate::machine_kinematics::PredictedFeedMap,
) -> Option<f64> {
    let divisor = f64::from(sample.spindle_rpm) * f64::from(sample.flute_count);
    if divisor <= 0.0 {
        return None;
    }
    Some(super::effective_feed_for_sample(sample, predicted_feeds) / divisor)
}

pub(super) fn embedded_lut() -> &'static crate::feeds::VendorLut {
    crate::feeds::embedded_vendor_lut()
}

// Cross-vendor DOC-derating rule lives in `feeds::geometry` so the
// feeds calculator can apply it to the LUT chipload bounds at the same
// scale this gate uses. The single canonical home prevents the two
// paths from drifting (see `feeds::geometry::doc_derating_scale`).
// Only exercised directly by this module's own tests today (production
// callers reach it via `feeds::geometry::doc_derating_scale` directly),
// so the re-export is `#[cfg(test)]`-gated to avoid an unused-import
// warning in the non-test build.
#[cfg(test)]
pub(super) use crate::feeds::geometry::doc_derating_scale;

use super::verdict::{
    ChipBounds, ChipBoundsSource, ChipSide, ChiploadMetric, ChiploadStatistic, ChiploadVerdict,
    Confidence, EntrySpike, SampleEvidence, UnmodeledReason,
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
///
/// **Unit caveat, recorded 2026-08-06 and deliberately NOT fixed here.**
/// This predicate compares each sample's raw `effective_chip_thickness_mm`
/// — an arc-mean chip, uncorrected for achieved feed — against a band the
/// B-lit verdict established is a linear **advance** per tooth. That is
/// the same mismatch the gate above just deleted, in a sibling consumer.
///
/// It is left standing on purpose, because the deletion does not
/// transfer. The gate's observation could become an advance per tooth
/// because the census proved the sample's own arc cancels out of it. This
/// predicate is *about* that arc: it asks whether engagement varies enough
/// across one toolpath that no single feed/RPM scaling fixes both ends.
/// Re-expressing it in advance per tooth would make it read only the
/// kinematic feed map and go near-vacuous — a worse answer, not a
/// better-unit one.
///
/// So the honest position is: the samples are the right quantity and the
/// **bounds** are the wrong yardstick for them. Fixing it needs a
/// chip-thickness envelope, which no vendor in the shipped LUT publishes
/// (verdict §2.3), or a different predicate. Owner: the optimizer
/// pre-flight lane. Re-open condition: a source for a chip-thickness
/// band, or a ruling that an engagement-variance refusal may be scaled
/// from an advance band by an explicit, disclosed factor.
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
    toolpath_id: ToolpathId,
    operation_feed_rate_mm_min: f64,
) -> SteadyStateSamples<'a> {
    let feed_threshold = STEADY_STATE_FEED_FRACTION * operation_feed_rate_mm_min;
    let mut any_in_cut = false;
    let samples = trace
        .samples
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            if s.toolpath_id != toolpath_id
                || !s.is_cutting
                || s.engagement.radial_woc_fraction < 0.02
            {
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
/// Every stage of [`crate::feeds::FeedExplanation`] except the gate
/// observation, which cannot be filled until the verdict has chosen
/// between its median and peak statistics.
struct PartialFeedStages {
    commanded: crate::feeds::CommandedStage,
    band: crate::feeds::LutBandStage,
    achieved_feed: crate::feeds::AchievedFeedStage,
    sample_count: usize,
}

/// The chipload verdict, plus the stage-labelled record explaining how
/// its number relates to the commanded one (census T1.1).
///
/// `None` for the explanation whenever the gate refused before matching
/// a row — an `Unmodeled` verdict has no stages to label.
pub(crate) fn evaluate_with_explanation(
    ctx: &super::ToolpathLoadContext<'_>,
    env: &super::GateEnv<'_>,
) -> (ChiploadVerdict, Option<Box<crate::feeds::FeedExplanation>>) {
    let mut partial = None;
    let verdict = evaluate_inner(ctx, env, &mut partial);
    let explanation = partial.map(|p| {
        // Stage 5 must name the statistic the VERDICT quotes, not a
        // statistic of this function's choosing — the two arms report
        // different ones and conflating them is the labelling defect
        // this record exists to end.
        let (statistic, value_mm) = match &verdict {
            ChiploadVerdict::Within {
                approach_to_min,
                approach_to_max,
                burn_advisory,
                ..
            } => burn_advisory
                .as_deref()
                .or(approach_to_min.as_ref())
                .map(|m| {
                    (
                        crate::feeds::ObservedStatistic::Median,
                        m.observed_mm_per_tooth,
                    )
                })
                .unwrap_or((
                    crate::feeds::ObservedStatistic::Peak,
                    approach_to_max.observed_mm_per_tooth,
                )),
            ChiploadVerdict::Exceeds {
                side, triggering, ..
            } => (
                match side {
                    ChipSide::Low => crate::feeds::ObservedStatistic::Median,
                    ChipSide::High => crate::feeds::ObservedStatistic::Peak,
                },
                triggering.observed_mm_per_tooth,
            ),
            ChiploadVerdict::Unmodeled { .. } => {
                (crate::feeds::ObservedStatistic::Median, f64::NAN)
            }
        };
        Box::new(crate::feeds::FeedExplanation {
            commanded: p.commanded,
            band: p.band,
            achieved_feed: p.achieved_feed,
            gate: crate::feeds::GateObservationStage {
                statistic,
                value_mm,
                sample_count: p.sample_count,
            },
        })
    });
    (verdict, explanation)
}

#[tracing::instrument(level = "debug", skip_all, fields(toolpath_id = ctx.toolpath_id.0, op = ?ctx.operation_kind))]
pub fn evaluate(ctx: &super::ToolpathLoadContext<'_>, env: &super::GateEnv<'_>) -> ChiploadVerdict {
    evaluate_inner(ctx, env, &mut None)
}

fn evaluate_inner(
    ctx: &super::ToolpathLoadContext<'_>,
    env: &super::GateEnv<'_>,
    partial_stages: &mut Option<PartialFeedStages>,
) -> ChiploadVerdict {
    let &super::ToolpathLoadContext {
        toolpath_id,
        tool,
        material,
        operation_family,
        pass_role,
        operation_feed_rate_mm_min,
        operation_kind,
        spans,
        ..
    } = ctx;
    let &super::GateEnv {
        sim_trace,
        tolerance,
        ..
    } = env;
    // Roadmap F.8 — short-circuit when the op is geometrically
    // plunge-only. Drill cycles have no continuous chipload to
    // compare against an LUT envelope; the right answer is "doesn't
    // apply" not "samples not in steady state" or "no vendor data".
    if operation_kind.is_drill_kinematics() {
        return ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(
                "drill cycle — no continuous engagement".to_owned(),
            ),
        };
    }
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
    //
    // UX dial-in A10: when sim ran end-to-end but no sample was in-cut
    // and out of air, surface the actual finding ("toolpath made no
    // contact with material") rather than `SimulationRequired`, which
    // misleadingly implies the user needs to re-sim.
    if !any_in_cut_for_toolpath {
        let reason = if trace.samples.iter().any(|s| s.toolpath_id == toolpath_id) {
            UnmodeledReason::AllSamplesAirCutOrRapid
        } else {
            UnmodeledReason::SimulationRequired
        };
        return ChiploadVerdict::Unmodeled { reason };
    }
    if steady_samples.is_empty() {
        return ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::SteadyStateSamplesNotPresent,
        };
    }

    // 4. Look up the vendor envelope. If no row matches, refuse.
    //
    // The LUT diameter query and the DOC-derating ratio below both
    // depend on `lookup_axial_doc_mm`. Folding over every steady-state
    // sample (including transit-style samples whose dexel reading
    // captures `stock_top − cutter_z` over neighbouring stock — see
    // P3_TRANSIT_PEAK_DOC_RCA.md) silently inflates this metric and
    // pushes the derating ratio past the 3×D cliff on toolpaths whose
    // only "deep" reading is a phantom waterline-cleanup or link-bridge
    // sample. Filter through the canonical
    // [`super::locality::is_steady_state_for_gate`] predicate (which
    // consults `in_transit_span` plus the Entry / WaterlineCleanup
    // ancestor check), so the LUT lookup and the trip decision below
    // measure the same population of samples.
    let span_lookup = spans.map(SpanLookup::new);
    let lookup_axial_doc_mm = steady_samples
        .iter()
        .filter(|(_, s)| super::locality::is_steady_state_for_gate(s, span_lookup.as_ref()))
        .map(|(_, s)| s.axial_doc_mm.max(0.0))
        .fold(0.0_f64, f64::max);
    // F3.4 — resolve through the canonical chipload-envelope helper
    // (angle-aware dispatch, ProjectCurve/Adaptive3d routing, engaged
    // diameter at peak DOC, RPM-only rows excluded) so the gate, the
    // optimizer, and the viewport read the SAME row.
    let result = matched_chip_envelope(
        tool,
        material,
        operation_kind,
        operation_family,
        pass_role,
        tool.lookup_diameter_at(lookup_axial_doc_mm),
    );
    let Some(result) = result else {
        tracing::debug!(
            reason = "NoVendorData",
            operation_family = ?operation_family,
            pass_role = ?pass_role,
            lookup_axial_doc_mm,
            "chipload gate refuses: no chipload-bearing LUT row matched the \
             (tool, material, op) query"
        );
        return ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::NoVendorData,
        };
    };

    // 5. Bounds: upper bound is required. A missing lower bound means
    // burn/rubbing cannot be modeled for this row, not that we invent
    // one — `AllowHalfBand` lets a present-max/absent-min row through.
    //
    // DOC-derating (cross-vendor-confirmed rule) folds in here too:
    // vendor LUT chipload bounds are authored at 1×D axial DOC; deeper
    // passes must reduce chipload to keep chip-evacuation viable.
    // Apply the piecewise scale once per toolpath using the peak axial
    // DOC, so the reported bounds match what the trip actually used.
    // Validation + scale now live in the shared
    // `geometry::derate_chipload_bounds` (S.8 — see
    // `planning/finishing_stack_review_2026-07.md`), the single home
    // for this wrapper across Suggest and both `tool_load` gate sites.
    let lookup_diameter_at_peak = tool.lookup_diameter_at(lookup_axial_doc_mm).max(1e-9);
    let doc_ratio = lookup_axial_doc_mm / lookup_diameter_at_peak;
    let Some(band) = crate::feeds::geometry::derate_chipload_bounds(
        result.chip_load_min_mm,
        result.chip_load_max_mm,
        doc_ratio,
        crate::feeds::geometry::ChiploadBoundPolicy::AllowHalfBand,
    ) else {
        tracing::debug!(
            reason = "NoVendorData",
            observation_id = %result.observation_id,
            "chipload gate refuses: matched row has unusable chipload bounds (max missing or invalid)"
        );
        return ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::NoVendorData,
        };
    };
    let (min, max) = (band.min_mm_per_tooth, band.max_mm_per_tooth);
    // F3.3 — provenance classification, worst-first. A single-point
    // "range" (raw min == max — scaling preserves equality) is a
    // nominal preset, not a calibrated envelope; extrapolation past
    // ±40 % stretches whatever the row published; a row with no `ae`
    // window is the crate's weakest-annotated class. All three make the
    // LOW side advisory-only (`low_side_is_advisory`); the HIGH side
    // stays hard everywhere.
    //
    // `VendorLutMissingAe`'s ORIGINAL justification — "this row skipped
    // the engagement-arc normalisation" — no longer exists, because no
    // row is normalised any more. Its behaviour is kept unchanged
    // deliberately: demoting a burn trip to an advisory is the
    // conservative direction, and promoting 176 of the LUT's 252 rows
    // back to hard `Exceeds(Low)` trips is a behaviour change with no
    // ruling behind it. What the label now means is "the weakest-
    // annotated row class in the LUT", which is a defensible reason to
    // soften a burn floor and an indefensible one to keep calling it an
    // arc problem. Re-open: Checkpoint item, owner unassigned.
    let source = if result
        .chip_load_min_mm
        .zip(result.chip_load_max_mm)
        .is_some_and(|(lo, hi)| lo >= hi)
    {
        ChipBoundsSource::VendorLutPointPreset
    } else if result.is_extrapolated {
        ChipBoundsSource::VendorLutExtrapolated
    } else if result.ae_min_mm.is_none() && result.ae_max_mm.is_none() {
        ChipBoundsSource::VendorLutMissingAe
    } else {
        ChipBoundsSource::VendorLut
    };
    let bounds = ChipBounds {
        min_mm_per_tooth: min,
        max_mm_per_tooth: max,
        source,
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

    // T1.1 — capture the commanded stage and the achieved-feed stage
    // BEFORE the sample loop consumes `steady_samples`. Both are already
    // computed by this gate; pre-T1.1 they were used and dropped, which
    // is why an operator could see the gate's number and the commanded
    // number and have nothing relating them.
    let commanded_stage = steady_samples.first().map(|(_, s)| {
        let divisor = f64::from(s.spindle_rpm) * f64::from(s.flute_count);
        crate::feeds::CommandedStage {
            feed_rate_mm_min: operation_feed_rate_mm_min,
            spindle_rpm: s.spindle_rpm,
            flute_count: s.flute_count,
            feed_per_tooth_mm: if divisor > 0.0 {
                operation_feed_rate_mm_min / divisor
            } else {
                0.0
            },
        }
    });
    let achieved_feed_stage = {
        let predicted_feeds_present = !trace.predicted_feeds.is_empty();
        let median_ratio = if predicted_feeds_present {
            let mut ratios: Vec<f64> = steady_samples
                .iter()
                .map(|(_, s)| {
                    super::effective_feed_for_sample(s, &trace.predicted_feeds)
                        / s.feed_rate_mm_min.max(1e-9)
                })
                .collect();
            if ratios.is_empty() {
                None
            } else {
                ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                #[allow(clippy::indexing_slicing)] // SAFETY: non-empty checked above
                Some(ratios[ratios.len() / 2])
            }
        } else {
            None
        };
        crate::feeds::AchievedFeedStage {
            predicted_feeds_present,
            median_ratio,
        }
    };

    let mut peak_above: Option<(f64, usize)> = None;
    let mut peak_in_range: (f64, usize) = (0.0, 0);
    let mut valid_count: usize = 0;
    let mut missing_arc_count: usize = 0;
    let mut chip_geometry_unsupported_count: usize = 0;
    // Samples whose `rpm × flutes` divisor is not positive, so no
    // advance per tooth can be formed. Distinct from the two counters
    // above: those describe the chip model declining, this describes a
    // sample that cannot state its own spindle. Tracked separately so
    // the refusal reason names the right cause.
    let mut no_divisor_count: usize = 0;
    // Per-sample chip thicknesses for the per-toolpath median used by
    // the burn-risk verdict. See `Burn-risk verdict semantics` above.
    let mut burn_samples: Vec<(f64, usize)> = Vec::new();
    // D7 — span-aware steady-state filter. Samples whose span_path
    // contains a `SpanKind::Entry` ancestor (configured plunge / helix /
    // ramp entries laid down by D4 / D5) are kept out of the
    // steady-state trip set and surfaced separately as `entry_spikes`
    // on the `Within` arm. Replaces the C1 kinematics filter reverted
    // in D3 (commit `8e2a7fc`). `span_lookup` was constructed alongside
    // `lookup_axial_doc_mm` above.
    // Worst Entry-ancestry over-max sample (largest observed_fpt > max).
    let mut entry_high: Option<(f64, usize)> = None;
    // Worst Entry-ancestry under-min sample (smallest observed_fpt < min).
    let mut entry_low: Option<(f64, usize)> = None;

    for (i, s) in steady_samples {
        // **The sample-validity predicate is deliberately unchanged, and
        // is now vestigial.** Before 2026-08-06 the observation WAS
        // `effective_chip_thickness_mm`, so a sample without one had
        // nothing to report. The observation is now purely kinematic and
        // needs no chip model — but keeping the filter keeps the gate's
        // POPULATION byte-identical across the unit conversion, so the
        // verdict-flip table this change ships with is attributable to
        // exactly one cause. Widening it would move verdicts for a
        // second reason at the same time.
        //
        // Recorded as NOT FIXED in the wave's log entry with its own
        // owner: the two `Unmodeled` reasons below (`ArcEngagementNotCaptured`,
        // `CutterModeUnsupported`) now refuse to report an advance per
        // tooth because a chip model did not resolve, which is no longer
        // a reason. Changing it is a population change and needs its own
        // before/after.
        if s.effective_chip_thickness_mm.is_none() {
            if s.arc_engagement_radians.is_none() {
                missing_arc_count += 1;
            } else {
                chip_geometry_unsupported_count += 1;
            }
            continue;
        }
        // The observation. `effective_feed_for_sample` is the F-035
        // kinematics substitution: when the trace carries a populated
        // predicted-feed map and this `(toolpath_id, move_index)` lookup
        // hits, it returns the feed the machine will actually reach;
        // otherwise it returns the commanded feed and this is the
        // commanded advance per tooth.
        let Some(observed_fpt) = achieved_feed_per_tooth_mm(s, &trace.predicted_feeds) else {
            no_divisor_count += 1;
            continue;
        };
        // R-10 (census §7.5, report-only). NOTE THE ORDER: this increments
        // BEFORE the `is_phantom_transit` skip immediately below, so
        // `valid_count` counts samples the loop then discards. It is
        // published as `sample_count` on the verdict, where it reads as "the
        // population that drove this gate" — and it OVERSTATES that
        // population by however many transit samples the trip set rejected.
        //
        // The gap used to be enormous on curve-heavy finishing paths: arc-fit
        // tagged every fitted arc `SpanKind::DressupArtifact`, which is on
        // the transit list, so MORE arc-fitting meant FEWER samples actually
        // gating — one committed fixture carries 630 such spans on a single
        // op. Checkpoint D Q3 (2026-08-04) moved fitted arcs to
        // `SpanKind::GeometryRefit`, which is not a transit kind, so that
        // term is gone. What remains is the genuine transit population
        // (entries, link bridges, lead-outs, dogbones, waterline cleanup).
        //
        // Left as-is deliberately: `sample_count` is a shipped wire field
        // and moving it is a separate, gated change. Documented here so the
        // next reader does not take it for the gate's true denominator.
        valid_count += 1;
        // Finding 3 split (2026-06-04): phantom-transit samples
        // (WaterlineCleanup / LinkBridge / LeadOut / DressupArtifact)
        // carry phantom-deep dexel readings and must not surface as
        // entry_spikes (the inflated chip-thickness misleads the
        // operator). Configured Entry samples (real plunge / ramp /
        // helix transients) still route to the entry_spikes advisory.
        // Both bypass the steady-state trip set.
        if super::locality::is_phantom_transit(s, span_lookup.as_ref()) {
            continue;
        }
        if super::locality::is_configured_entry(s, span_lookup.as_ref()) {
            if observed_fpt > max && entry_high.is_none_or(|(prev, _)| observed_fpt > prev) {
                entry_high = Some((observed_fpt, i));
            }
            if let Some(min_value) = min
                && observed_fpt < min_value
                && entry_low.is_none_or(|(prev, _)| observed_fpt < prev)
            {
                entry_low = Some((observed_fpt, i));
            }
            continue;
        }
        burn_samples.push((observed_fpt, i));
        // Layer 1 tolerance band: a single sample 1-2% over `max` (e.g.
        // wanaka TP4 5/2026 transient at 1.05% over) shouldn't flip the
        // verdict to `Exceeds(High)`. The widened trigger is purely a
        // gate-trip decision; the underlying `peak_above` deviation is
        // still recorded so downstream displays surface the value.
        let max_trigger = max * (1.0 + tolerance.breakage);
        if observed_fpt > max_trigger {
            let dev = observed_fpt - max;
            if peak_above.is_none_or(|(prev, _)| dev > prev) {
                peak_above = Some((dev, i));
            }
        } else if observed_fpt > peak_in_range.0 {
            peak_in_range = (observed_fpt, i);
        }
    }

    // T1.1 — assemble everything except stage 5, which needs the verdict
    // arm (median for the burn side, peak for the breakage side) and is
    // filled in by `evaluate_with_explanation`. Nothing here is
    // recomputed: every field is a value this gate already derived.
    *partial_stages = commanded_stage.map(|commanded| PartialFeedStages {
        commanded,
        band: crate::feeds::LutBandStage {
            observation_id: result.observation_id.clone(),
            bounds_source: source,
            row_diameter_mm: result.row_diameter_mm,
            queried_diameter_mm: lookup_diameter_at_peak,
            diameter_scale: result.chipload_diameter_scale,
            hardness_scale: result.chipload_hardness_scale,
            is_extrapolated: result.is_extrapolated,
            queried_pass_role: pass_role,
            row_pass_role: result.row_pass_role,
            min_mm_per_tooth: min,
            max_mm_per_tooth: max,
            // Report-only since 2026-08-06: nothing computes with this.
            ae_window_mm: result.ae_min_mm.zip(result.ae_max_mm),
        },
        achieved_feed: achieved_feed_stage,
        sample_count: valid_count,
    });

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
        // Every steady-state sample was discarded. Pick the dominant
        // cause. The first two are the vestigial chip-model predicate
        // (see the loop's note); the third is a sample that could not
        // state its own `rpm × flutes`, which the advance-per-tooth
        // observation genuinely cannot work around.
        let reason = if no_divisor_count > missing_arc_count
            && no_divisor_count > chip_geometry_unsupported_count
        {
            UnmodeledReason::CutterModeUnsupported(
                "samples carry no positive spindle-rpm x flute-count divisor, so no \
                 advance per tooth can be formed"
                    .to_owned(),
            )
        } else if missing_arc_count >= chip_geometry_unsupported_count {
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
            .and_then(|s| super::locality::classify_sample_locality(s, span_lookup.as_ref()))
    };
    if let Some((dev, idx)) = peak_above {
        let observed = max + dev;
        tracing::warn!(
            verdict = "Exceeds",
            side = "High",
            observed_mm_per_tooth = observed,
            bound_max_mm_per_tooth = max,
            "chipload gate Exceeds: peak chip thickness above the vendor-derived max → breakage risk"
        );
        return ChiploadVerdict::Exceeds {
            side: ChipSide::High,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: observed,
                statistic: ChiploadStatistic::PeakHigh,
                evidence: SampleEvidence::at_with_stat(idx, ChiploadStatistic::PeakHigh)
                    .with_locality(locality_for(idx)),
                bounds,
            },
            confidence: chipload_confidence,
        };
    }
    // F3.3 — a low-side trip on weakly-provenanced bounds (point
    // preset / extrapolated / no ae calibration) downgrades to a
    // structured advisory on the `Within` arm instead of `Exceeds(Low)`.
    // The fabricated/stretched burn floor must not hard-block the
    // operator or the optimizer; the breakage side above stays hard.
    let mut burn_advisory: Option<Box<ChiploadMetric>> = None;
    if let Some((dev, idx)) = peak_below {
        let observed = min.map(|m| m - dev).unwrap_or_default().max(0.0);
        let metric = ChiploadMetric {
            observed_mm_per_tooth: observed,
            statistic: ChiploadStatistic::MedianLow,
            evidence: SampleEvidence::at_with_stat(idx, ChiploadStatistic::MedianLow)
                .with_locality(locality_for(idx)),
            bounds: bounds.clone(),
        };
        if bounds.source.low_side_is_advisory() {
            tracing::debug!(
                verdict = "Within",
                advisory = "burn",
                source = ?bounds.source,
                observed_mm_per_tooth = observed,
                bound_min_mm_per_tooth = min.unwrap_or(f64::NAN),
                "chipload gate: median below vendor min but the burn floor's \
                 provenance is weak — surfacing a burn advisory instead of Exceeds(Low)"
            );
            burn_advisory = Some(Box::new(metric));
        } else {
            tracing::warn!(
                verdict = "Exceeds",
                side = "Low",
                observed_mm_per_tooth = observed,
                bound_min_mm_per_tooth = min.unwrap_or(f64::NAN),
                "chipload gate Exceeds: median chip thickness below vendor-derived min → burn / rubbing risk"
            );
            return ChiploadVerdict::Exceeds {
                side: ChipSide::Low,
                triggering: metric,
                confidence: chipload_confidence,
            };
        }
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
    // D7 — entry-spike advisories. Each side reports the worst Entry-
    // ancestry sample whose normalized chip-thickness landed outside
    // the LUT envelope. The trip itself fired on steady-state samples
    // only (or didn't fire — this is the `Within` arm); these spikes
    // exist purely so the operator sees that an entry transient
    // touched the bound without flipping the verdict.
    let mut entry_spikes: Vec<EntrySpike> = Vec::new();
    if let Some((observed, idx)) = entry_high {
        entry_spikes.push(EntrySpike {
            observed,
            bound: max,
            locality: locality_for(idx).unwrap_or_else(|| "entry".to_owned()),
            side: Some(ChipSide::High),
        });
    }
    if let (Some((observed, idx)), Some(min_value)) = (entry_low, min) {
        entry_spikes.push(EntrySpike {
            observed,
            bound: min_value,
            locality: locality_for(idx).unwrap_or_else(|| "entry".to_owned()),
            side: Some(ChipSide::Low),
        });
    }
    ChiploadVerdict::Within {
        approach_to_min,
        approach_to_max,
        confidence: chipload_confidence,
        entry_spikes,
        burn_advisory,
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationType;
    use crate::material::Material;
    use crate::material::WoodSpecies;
    use crate::simulation_cut::{SimulationCutSample, SimulationCutSummary, SimulationCutTrace};
    use crate::tool::ToolDefinition;
    use crate::tool::{FlatEndmill, VBitEndmill};

    /// Adapts this module's legacy positional-arg test calls to the
    /// Phase 6 `(ctx, env)` gate signature.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_args(
        toolpath_id: usize,
        tool: &crate::tool::ToolDefinition,
        material: &crate::material::Material,
        sim_trace: Option<&crate::simulation_cut::SimulationCutTrace>,
        spans: Option<&[crate::toolpath_spans::Span]>,
        operation_family: LutOperationFamily,
        pass_role: LutPassRole,
        operation_feed_rate_mm_min: f64,
        operation_kind: crate::compute::catalog::OperationType,
        tolerance: &crate::tool_load::ToleranceBands,
    ) -> ChiploadVerdict {
        evaluate(
            &crate::tool_load::ToolpathLoadContext {
                toolpath_id: ToolpathId(toolpath_id),
                tool,
                material,
                operation_family,
                pass_role,
                operation_feed_rate_mm_min,
                operation_kind,
                spans,
                drill_op: None,
            },
            &crate::tool_load::GateEnv {
                sim_trace,
                machine: None,
                tolerance,
            },
        )
    }

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
    /// flat) publishes, derived as `acos(1 − 2·ae_mid/D)` with
    /// `ae_min=1.0`, `ae_max=2.2`, `D=6.0` from
    /// `data/vendor_lut/observations/amana_flat_end.json` → ≈ 1.0844 rad.
    ///
    /// Until 2026-08-06 this value was load-bearing: it made D9's
    /// per-sample renormalisation an exact no-op, so `observed ==
    /// effective_chip_thickness_mm`. D9 is gone (module header) and this
    /// constant is now only a realistic arc for the sample to carry. It
    /// is kept because `arc_engagement_radians.is_none()` still selects
    /// which `Unmodeled` reason the all-invalid path reports.
    const TEST_LUT_NOMINAL_ARC_RAD: f64 = 1.0843860798928202;

    /// The `chipload` argument is the advance per tooth the sample is to
    /// be **observed** at. [`trace`] realises it through the F-035
    /// predicted-feed map — see its doc for why, and for the property
    /// that makes every verdict below unchanged across the conversion.
    fn sample(tp_id: usize, idx: usize, chipload: f64, engagement: f64) -> SimulationCutSample {
        SimulationCutSample {
            toolpath_id: ToolpathId(tp_id),
            move_index: idx,
            sample_index: idx,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 1000.0,
            spindle_rpm: 18000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            arc_engagement_radians: Some(TEST_LUT_NOMINAL_ARC_RAD),
            chipload_mm_per_tooth: chipload,
            effective_chip_thickness_mm: Some(chipload),
            engagement: crate::simulation_cut::Engagement::with_radial_woc(engagement),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            ..SimulationCutSample::test_fixture()
        }
    }

    /// **How the fixtures steer the observation after the 2026-08-06 unit
    /// conversion, and why no verdict in this module moved.**
    ///
    /// Before: the gate observed `effective_chip_thickness_mm`
    /// renormalised from the sample's arc to the row's. Every sample here
    /// carries [`TEST_LUT_NOMINAL_ARC_RAD`], so the renormalisation was an
    /// exact no-op and `observed == sample()`'s `chipload` argument.
    ///
    /// After: the gate observes `effective_feed / (rpm · flutes)`. Giving
    /// each sample an F-035 predicted-feed entry of
    /// `chipload × rpm × flutes` makes `observed == chipload` again —
    /// **the same number, reached through the pipeline the conversion
    /// installed.** So every trip / no-trip expectation below is
    /// unchanged, and none of them went vacuous: the `chipload` argument
    /// still steers the verdict, exactly as loudly as it did.
    ///
    /// The commanded `feed_rate_mm_min` is deliberately left where each
    /// test set it, so `steady_state_samples_for_toolpath`'s population —
    /// the 95 %-of-commanded-feed filter and the tests that exercise it —
    /// is byte-identical.
    fn trace(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
        let mut predicted_feeds = crate::machine_kinematics::PredictedFeedMap::new();
        for s in &samples {
            let divisor = f64::from(s.spindle_rpm) * f64::from(s.flute_count);
            predicted_feeds.insert(
                (s.toolpath_id, s.move_index),
                s.chipload_mm_per_tooth * divisor,
            );
        }
        SimulationCutTrace {
            predicted_feeds,
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
                peak_plunge_descent_mm: 0.0,
                total_removed_volume_est_mm3: 1.0,
                average_mrr_mm3_s: 1.0,
                per_kinematics: std::collections::BTreeMap::new(),
                runtime_by_intent: None,
            },
            samples,
            ..SimulationCutTrace::test_fixture()
        }
    }

    /// Phase 1E unit tests for the cross-vendor DOC-derating scale
    /// (Amana + Onsrud verbatim rule: 1×D=1.0, 2×D=0.75, 3×D=0.5,
    /// clamped at 3×D).
    #[test]
    fn doc_derating_unscaled_below_1x_d() {
        assert!((doc_derating_scale(0.5) - 1.0).abs() < 1e-9);
        assert!((doc_derating_scale(0.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn doc_derating_25pct_at_2x_d() {
        assert!((doc_derating_scale(2.0) - 0.75).abs() < 1e-9);
    }

    #[test]
    fn doc_derating_50pct_at_3x_d() {
        assert!((doc_derating_scale(3.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn doc_derating_clamps_at_3x_d() {
        assert!((doc_derating_scale(5.0) - 0.5).abs() < 1e-9);
        assert!((doc_derating_scale(100.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn project_curve_flat_routes_to_contour_finish() {
        // Routing assertion: `OperationType::ProjectCurve` with
        // `LutOperationFamily::Trace` falls back to a contour/finish
        // row in the LUT and produces a `Within` verdict when the
        // sample chipload sits in the matched row's band.
        //
        // 2026-05-31 Phase 4 promotion: the Onsrud OCR PDFs added direct
        // 6.35 mm hardwood contour/finish rows (0.014–0.016 in/tooth =
        // 0.3556–0.4064 mm/tooth), displacing the older 2× scaled match
        // from the Amana 3.175 mm row. Sample chipload + confidence
        // assertion updated to fit the new direct-match band. The test's
        // *purpose* — ProjectCurve→Trace routing finds a contour/finish
        // row and gates the sample — is preserved.
        //
        // 2026-06-03 family-default Janka anchor (151c7b5): the Onsrud
        // 6.35 mm hardwood row carries no per-row `hardness_value`, so
        // `hardness_ratio_raw` (then named `hardness_scale_factor`) previously degraded to identity for
        // any hardwood query. After 151c7b5 it scales against the
        // hardwood family anchor (Janka 1290, red oak). HardMaple's
        // Janka is 1450, so the matched band derates by 1290/1450 ≈
        // 0.8896 → effective band 0.3164–0.3616 mm/tooth. Sample
        // re-centred to 0.34 mm/tooth so it lands inside the derated
        // band. Routing assertion (ProjectCurve→Trace → contour/finish
        // row) is unchanged.
        //
        // Arc override prevents D9's per-sample normalization from
        // perturbing row selection (contour-finish rows have narrower
        // nominal arcs than the pocket-roughing default in `sample()`).
        let mut s = sample(0, 0, 0.34, 0.5);
        s.arc_engagement_radians = Some(0.459);
        let t = trace(vec![s]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            LutOperationFamily::Trace,
            LutPassRole::Finish,
            1000.0,
            OperationType::ProjectCurve,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(v, ChiploadVerdict::Within { .. }),
            "expected Within (routing landed in a contour/finish row with sample in band), got {v:?}"
        );
    }

    #[test]
    fn project_curve_vbit_stays_unmodeled() {
        let t = trace(vec![sample(0, 0, 0.02, 0.5)]);
        let v = evaluate_args(
            0,
            &vbit_tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            None,
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
    fn samples_exist_but_none_in_cut_reports_all_samples_air() {
        // UX dial-in A10: when sim ran and produced samples for this
        // toolpath but none are in-cut (every sample is air-cut or
        // rapid), the verdict surfaces the actionable finding
        // "toolpath made no material contact" instead of the
        // misleading "simulation required".
        let mut s = sample(0, 0, 0.05, 0.5);
        s.is_cutting = false;
        let t = trace(vec![s]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::AllSamplesAirCutOrRapid
            }
        ));
    }

    #[test]
    fn chipload_within_bounds_is_within_validated() {
        // 6.35mm flat in hard maple, pocket roughing: Amana LUT puts the
        // chipload range somewhere around 0.025–0.060 mm/tooth. Use 0.04.
        let t = trace(vec![sample(0, 0, 0.04, 0.5)]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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

    /// F3.3 — a low-side trip against weakly-provenanced bounds
    /// (extrapolated here: 0.5 mm tapered ball against a ≥1 mm
    /// calibrated row, the documented ±40 % extrapolation case) must
    /// NOT hard-refuse with `Exceeds(Low)`: it lands `Within` with a
    /// structured `burn_advisory` carrying the same MedianLow metric
    /// the trip would have reported. The breakage side stays hard for
    /// every provenance.
    #[test]
    fn weak_provenance_low_trip_demotes_to_burn_advisory() {
        use crate::tool::TaperedBallEndmill;
        let tapered = ToolDefinition::new(
            Box::new(TaperedBallEndmill::new(0.5, 7.0, 6.0, 30.0)),
            0.5,
            10.0,
            25.0,
            35.0,
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        );
        // Far below any scaled min — would trip Exceeds(Low) on a
        // calibrated row.
        let t = trace(vec![sample(0, 0, 0.0001, 0.5)]);
        let v = evaluate_args(
            0,
            &tapered,
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            LutOperationFamily::Parallel,
            LutPassRole::Finish,
            1000.0,
            OperationType::DropCutter,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            ChiploadVerdict::Within {
                burn_advisory: Some(advisory),
                confidence,
                ..
            } => {
                assert_eq!(advisory.statistic, ChiploadStatistic::MedianLow);
                assert!(
                    advisory.bounds.source.low_side_is_advisory(),
                    "advisory must carry the weak source, got {:?}",
                    advisory.bounds.source
                );
                assert!(
                    matches!(confidence, Confidence::Approximate(_)),
                    "extrapolated bounds must demote confidence, got {confidence:?}"
                );
            }
            other => panic!("expected Within + burn_advisory, got {other:?}"),
        }
    }

    #[test]
    fn chipload_far_below_min_is_exceeds_burn() {
        // 0.001 mm/tooth — rubbing.
        let t = trace(vec![sample(0, 0, 0.001, 0.5)]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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

    /// Finding 3 regression sentry (2026-06-04): a phantom-deep
    /// transit sample (e.g. a WaterlineCleanup sample whose dexel reads
    /// `stock_top − cutter_z` over neighbouring uncleared stock, not
    /// the cutter's real engagement) must NOT inflate
    /// `lookup_axial_doc_mm`. Without the in_transit_span filter, the
    /// LUT row's chipload envelope derates by `doc_derating_scale(ratio)`
    /// down to 0.5× at 3×D, and well-behaved steady-state samples land
    /// above the phantom-derated max → false `Exceeds(High)`.
    ///
    /// Setup mirrors the wanaka Back Rough symptom: two healthy 0.04
    /// chipload samples at commanded DPP ≈ 0.6×D, plus one phantom
    /// transit sample carrying axial_doc_mm = 30 mm (≫ 3×D). Verdict
    /// must remain Within because the phantom sample is filtered out
    /// of the DOC-derating ratio computation.
    #[test]
    fn phantom_transit_sample_does_not_inflate_doc_ratio() {
        // Two healthy steady-state samples: 4 mm axial DOC on a 6.35 mm
        // tool (ratio ≈ 0.63 → scale 1.0, no derating). Chipload 0.04
        // lands inside the LUT band.
        let mut healthy_a = sample(0, 0, 0.04, 0.5);
        healthy_a.axial_doc_mm = 4.0;
        let mut healthy_b = sample(0, 1, 0.04, 0.5);
        healthy_b.axial_doc_mm = 4.0;
        // Phantom transit sample: lift-bridge over uncleared stock, the
        // dexel reads 30 mm "axial DOC" though the cutter isn't actually
        // engaged that deeply. Pre-fix this drove lookup_axial_doc_mm to
        // 30 → ratio ~4.7 → scale 0.5 → bounds halve → healthy_a/b's
        // 0.04 chipload now sits above the 0.5× max ≈ 0.0275 → Exceeds.
        let mut phantom = sample(0, 2, 0.04, 0.5);
        phantom.axial_doc_mm = 30.0;
        phantom.in_transit_span = true;
        let t = trace(vec![healthy_a, healthy_b, phantom]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(v, ChiploadVerdict::Within { .. }),
            "phantom-transit sample must not inflate DOC-derating ratio, got {v:?}"
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
    /// preserved — a toolpath with samples but none in-cut now reports
    /// `AllSamplesAirCutOrRapid` (UX dial-in A10) rather than the
    /// misleading `SimulationRequired`. The "any sample exists for this
    /// TP" branch takes priority over the steady-state reason.
    #[test]
    fn no_in_cut_samples_takes_priority_over_steady_state_reason() {
        let mut s = sample(0, 0, 0.05, 0.5);
        s.is_cutting = false;
        // even at the commanded feed, the sample isn't in cut.
        s.feed_rate_mm_min = 1000.0;
        let t = trace(vec![s]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::AllSamplesAirCutOrRapid
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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

    /// Roadmap F.8 — drill / pin-drill operations: the chipload gate
    /// is geometrically not applicable (plunge-only kinematics, no
    /// continuous engagement). The gate short-circuits at the top of
    /// `evaluate` with `Unmodeled(NotApplicableForOp("drill cycle …"))`
    /// regardless of the sample stream — that's the "doesn't apply"
    /// bucket, distinct from "couldn't measure" (`SimulationRequired`
    /// / `ArcEngagementNotCaptured`) which would imply re-running the
    /// sim with better inputs.
    ///
    /// Pre-F.8 this case routed to `ArcEngagementNotCaptured` because
    /// every plunge sample dropped its arc; the old reason was correct
    /// mechanically but misleading semantically (the sim wasn't
    /// failing — the metric just has no meaning here).
    #[test]
    fn drill_like_no_arc_samples_route_to_unmodeled() {
        // engagement above the air-cut threshold (`>= 0.02`) so the
        // sample passes the steady-state filter and reaches the
        // chip-geometry path. arc=None mirrors a real plunge move
        // where the simulator can't compute an engagement arc.
        let mut s0 = sample(0, 0, 0.5, 0.5);
        s0.feed_rate_mm_min = 1500.0;
        s0.cut_kinematics = crate::simulation_cut::CutKinematics::Plunge;
        s0.arc_engagement_radians = None;
        s0.effective_chip_thickness_mm = None;
        let mut s1 = sample(0, 1, 0.5, 0.5);
        s1.feed_rate_mm_min = 1500.0;
        s1.cut_kinematics = crate::simulation_cut::CutKinematics::Plunge;
        s1.arc_engagement_radians = None;
        s1.effective_chip_thickness_mm = None;
        let t = trace(vec![s0, s1]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
            1500.0,
            OperationType::Drill,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::NotApplicableForOp(detail),
            } => {
                assert!(
                    detail.contains("drill"),
                    "NotApplicableForOp detail should name the op family, got: {detail}"
                );
            }
            other => panic!(
                "drill-like samples must route to Unmodeled(NotApplicableForOp(\"drill …\")), got {other:?}"
            ),
        }
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
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
