//! Chipload guardrail — per-sample peak chip (P11b2) vs vendor-LUT bounds.
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
//! ## P11b2 (2026-11-05): the observation is the PEAK chip
//!
//! Rulings L1 and L6 (`planning/feeds_contract_2026-11-03/phases/P11.md`)
//! made Suggest size the feed so the PEAK chip is the band chip (radial
//! chip thinning, Garr: `h = fz sin psi` for `psi < 90 degrees`). The gate
//! now judges that chip:
//!
//! ```text
//! observed = effective_feed / (rpm · flutes) / lift(measured engagement)
//! ```
//!
//! through [`super::display::peak_chip_per_tooth`]. The lift is
//! `feeds::geometry::RadialThinning::lift` at the sample's measured arc
//! (the K7 `Engagement::from_sample` width and engaged diameter); a finish
//! pass (L2) takes the advance. The band is the printed band at every depth
//! (no depth de-rate). The low side reads the same peak chip: a light
//! stepover makes a thin chip, and that is what the burn reference checks.
//! The text above about the advance per tooth is the record of 2026-08-06.
//!
//! **The sim remains the operational arbiter.** This change makes
//! Suggest, the narration and this gate speak one unit; it does not
//! promote a static band over a measured cut.
//!
//! **The population (P29-6 6b).** The gate reads every sample of the
//! toolpath that removes material
//! ([`crate::stock::simulation_cut::SimulationCutSample::removes_material`]),
//! at any feed. The 95 % feed filter (Item C) is deleted: it dropped capped
//! steps and slot rings, which are body cuts, and it read the first-run feed
//! of a modulated trace. An entry and a phantom transit sample still leave
//! the trip set inside the loop until P29-6 6d. A removing sample with no
//! chip number (no measured arc, an arc of 0) counts in `missing_arc_count`.
//! With no removing sample the gate returns `AllSamplesAirCutOrRapid` (or
//! `SimulationRequired` when the toolpath has no sample).
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
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::feeds::vendor_normalize::material_to_lut;
use crate::ids::ToolpathId;
use crate::stock::simulation_cut::SimulationCutTrace;

use super::locality::SpanLookup;

/// F3.4 (defect class C5) — THE canonical chipload-envelope row
/// resolution. Before F3.4 several call sites (the gate below, the
/// viewport envelope map, and the optimizer) found this row with three
/// different lookup strategies, so advice could be computed against a
/// different envelope than the verdict.
///
/// Semantics:
/// - `Custom` material → `None` (no validated LUT category).
/// - Operation routing via [`routed_lookup_family`] (ProjectCurve /
///   Adaptive3d reroutes).
/// - `lookup_diameter_mm` is the LUT lookup key. Every caller computes
///   it with [`crate::feeds::geometry::lut_key_diameter_for_cutter`]: the
///   gate at the peak steady-state DOC and the viewport at its peak DOC.
///   A tapered ball is keyed at its tip at every depth (ruling A1,
///   2026-09-24); a V-bit at its nominal diameter (ruling B4, 2026-09-25).
/// - Since B4 the recipe resolver tries the same chipload-bearing rows
///   first for a V-bit, so Suggest's chip row is this row.
/// - Angle-aware dispatch for V-bit / chamfer geometry.
/// - Only chipload-bearing rows compete
///   ([`find_best_chip_envelope_row`]) — RPM-only rows are feeds-
///   calculator anchors, not envelopes.
/// - A row whose G1 size basis is `Refused` gives `None` (extrapolation P1
///   step 3). Suggest refuses that cell, so the gate, the optimizer and the
///   viewport do not judge it against a band either: the gate reports
///   `Unmodeled(NoVendorData)`.
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
    // Checkpoint K (a4) — ONE routing site, shared with
    // `vendor_normalize::to_lookup_query`. This used to be a private
    // `routed_lookup_family` that only this side called; A-6 measured
    // the cost of that asymmetry at 489 divergent rows and 378
    // one-sided refusals over 3 024 pairs.
    let (operation_family, pass_role) = crate::feeds::vendor_normalize::lut_query_for(
        operation_kind,
        tool_family,
        operation_family,
        pass_role,
    )?;
    let (material_family, hardness_kind, hardness_value) = material_to_lut(material);
    let diameter_mm = lookup_diameter_mm;
    let query = LookupQuery {
        tool_family,
        // Ruling 2026-10-31: the same series key as the Suggest query
        // (`vendor_normalize::to_lookup_query_unrouted`).
        tool_subfamily: crate::feeds::series::ToolSeries::for_cut_direction(tool.cut_direction)
            .map(|s| s.key().to_owned()),
        diameter_mm,
        flute_count: tool.flute_count,
        material_family,
        hardness_kind: Some(hardness_kind),
        hardness_value: Some(hardness_value),
        operation_family,
        pass_role,
    };
    find_best_chip_envelope_row(embedded_lut(), &query, &geometry_hint)
        .filter(|row| !row.size_basis.is_refused())
}

pub(super) fn embedded_lut() -> &'static crate::feeds::VendorLut {
    crate::feeds::embedded_vendor_lut()
}

use super::verdict::{
    ChipBounds, ChipBoundsSource, ChipSide, ChiploadMetric, ChiploadStatistic, ChiploadVerdict,
    Confidence, EntrySpike, SampleEvidence, UnmodeledReason,
};

/// The samples of one toolpath that remove material
/// ([`crate::stock::simulation_cut::SimulationCutSample::removes_material`],
/// ruling Q2 of P29-6), as `(global_index_into_trace, sample)`. The chip
/// gate and its histogram read this one set. No feed test: a sample at a
/// capped or a ramp feed is a cut too (P29-6 6b, row `M1` closed).
pub(crate) fn removing_samples_for_toolpath(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
) -> Vec<(usize, &crate::stock::simulation_cut::SimulationCutSample)> {
    trace
        .samples
        .iter()
        .enumerate()
        .filter(|(_, s)| s.toolpath_id == toolpath_id && s.removes_material())
        .collect()
}

/// The gate's sample-validity predicate: the sample carries a chip model
/// and is not air class. P29-6 (ruling Q2): an air-class sample that
/// removes material (a skin above the tip) is in the population, but the
/// air test is a reason for no number, so it never reaches a comparison.
/// The histogram reads this one predicate.
pub(super) fn has_chip_number(s: &crate::stock::simulation_cut::SimulationCutSample) -> bool {
    s.effective_chip_thickness_mm.is_some() && !s.is_air()
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
/// `ctx.steady_feed()` is the feed the op commands in steady cutting (P20e:
/// `min(op feed, feed_max_rate, cutting ceiling)`). The commanded stage
/// states it; no filter reads it (P29-6 6b). See module doc.
///
/// `operation_kind` is the toolpath's `OperationType`. For most kinds
/// the (operation_family, pass_role) tuple from the operation spec is
/// what gets passed to the LUT lookup. Two kinds get rerouted by
/// `routed_lookup_family`: `ProjectCurve` (ball/tapered-ball/bull-nose →
/// Parallel, flat → Contour, v-bit → Trace since operator ruling
/// 2026-09-25; facing bit returns `Unmodeled(NoVendorData)` — Item D of
/// the tool-load fidelity plan) and `Adaptive3d` (Adaptive → Pocket so
/// the LUT envelope reflects pocket-style clearing instead of 2D
/// adaptive HSM — design doc §1.3, §10).
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

#[tracing::instrument(level = "debug", skip_all, fields(toolpath_id = ctx.toolpath_id().raw(), op = ?ctx.operation_kind()))]
pub fn evaluate(ctx: &super::ToolpathLoadContext<'_>, env: &super::GateEnv<'_>) -> ChiploadVerdict {
    evaluate_inner(ctx, env, &mut None)
}

fn evaluate_inner(
    ctx: &super::ToolpathLoadContext<'_>,
    env: &super::GateEnv<'_>,
    partial_stages: &mut Option<PartialFeedStages>,
) -> ChiploadVerdict {
    let (toolpath_id, tool, material, steady_feed, operation_kind, spans) = (
        ctx.toolpath_id(),
        ctx.tool(),
        ctx.material(),
        ctx.steady_feed(),
        ctx.operation_kind(),
        ctx.spans(),
    );
    // The vendor row key: a row lookup, not a limit scope.
    let (operation_family, pass_role) = ctx.row_key();
    let &super::GateEnv {
        sim_trace,
        tolerance,
        ..
    } = env;
    // Roadmap F.8 — short-circuit when the op is geometrically
    // plunge-only. Drill cycles have no continuous chipload to
    // compare against an LUT envelope; the right answer is "doesn't
    // apply" not "samples not in steady state" or "no vendor data".
    // EXEMPT(P28-20)
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

    // 2. The population: every sample of the toolpath that removes
    // material, at any feed (P29-6 6b). The same set gives the peak DOC for
    // the lookup key.
    let removing_samples = removing_samples_for_toolpath(trace, toolpath_id);

    // 3. No removing sample: no chip to judge. Name the finding before
    // the LUT lookup, so missing samples and missing vendor data stay
    // distinct. UX dial-in A10: when the simulation ran but the toolpath
    // removed nothing, say so rather than `SimulationRequired`, which
    // implies a re-simulation.
    if removing_samples.is_empty() {
        let reason = if trace.samples.iter().any(|s| s.toolpath_id == toolpath_id) {
            UnmodeledReason::AllSamplesAirCutOrRapid
        } else {
            UnmodeledReason::SimulationRequired
        };
        return ChiploadVerdict::Unmodeled { reason };
    }

    // 4. Look up the vendor envelope. If no row matches, refuse.
    //
    // The LUT diameter query depends on `lookup_axial_doc_mm` (P11b2: the
    // band no longer moves with it). Folding over every removing
    // sample (including transit-style samples whose dexel reading
    // captures `stock_top − cutter_z` over neighbouring stock — see
    // P3_TRANSIT_PEAK_DOC_RCA.md) silently inflates this metric on
    // toolpaths whose only "deep" reading is a phantom waterline-cleanup
    // or link-bridge sample. Filter through the canonical
    // [`super::locality::is_steady_state_for_gate`] predicate (which
    // consults `in_transit_span` plus the Entry / WaterlineCleanup
    // ancestor check), so the LUT lookup and the trip decision below
    // measure the same population of samples.
    let span_lookup = spans.map(SpanLookup::new);
    let lookup_axial_doc_mm = removing_samples
        .iter()
        // EXEMPT(P28-21, P28-22, P28-23)
        .filter(|(_, s)| super::locality::is_steady_state_for_gate(s, span_lookup.as_ref()))
        .map(|(_, s)| s.axial_doc_mm.max(0.0))
        .fold(0.0_f64, f64::max);
    // F3.4 — resolve through the canonical chipload-envelope helper
    // (angle-aware dispatch, ProjectCurve/Adaptive3d routing, RPM-only
    // rows excluded) so the gate, the optimizer, and the viewport read
    // the SAME row. Ruling A1 (2026-09-24): the lookup key is
    // `lut_key_diameter_for_cutter`, the tip of a tapered ball; ruling B4
    // (2026-09-25): the nominal diameter of a V-bit, at every depth. The
    // key that Suggest uses (`vendor_normalize::lookup_diameter_for_input`)
    // is the same, so Suggest and the gate read one row.
    let lookup_key_mm =
        crate::feeds::geometry::lut_key_diameter_for_cutter(tool, lookup_axial_doc_mm);
    let result = matched_chip_envelope(
        tool,
        material,
        operation_kind,
        operation_family,
        pass_role,
        lookup_key_mm,
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
    // P11b2 (break; rulings L1 and L6): the gate reads the PRINTED band at
    // every depth, as Suggest does. The vendor depth table (1 / 0.75 / 0.50
    // at 1 / 2 / 3 x D) no longer lowers it; the envelope draws the table as
    // the limit `LimitKind::VendorDepthTable`. The validation is the shared
    // `geometry::validate_chipload_bounds` (S.8).
    let Some(band) = crate::feeds::geometry::validate_chipload_bounds(
        result.chip_load_min_mm,
        result.chip_load_max_mm,
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
    // A2 (point mode): a row that prints one value is a point
    // (`LookupResult::printed_chipload`). A point is not a band: the
    // bounds keep no minimum. The high side stays a hard trip at the
    // printed point v (P11b2: no depth de-rate). A median peak chip below v
    // gives a burn advisory only (decision Q1 (a); `ChipBounds::burn_reference_mm`). The gate
    // never gives `Exceeds(Low)` for a point.
    let is_point = matches!(
        result.printed_chipload(),
        crate::feeds::vendor_lookup::PrintedChipload::Point { .. }
    );
    let (min, max) = (band.min_mm_per_tooth, band.max_mm_per_tooth);
    // F3.3 — provenance classification, worst-first. A point (one
    // printed value, A2) is not a calibrated envelope; extrapolation past
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
    let source = if is_point {
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
    // BEFORE the sample loop consumes `removing_samples`. Both are already
    // computed by this gate; pre-T1.1 they were used and dropped, which
    // is why an operator could see the gate's number and the commanded
    // number and have nothing relating them.
    let commanded_stage = removing_samples.first().map(|(_, s)| {
        let divisor = f64::from(s.spindle_rpm) * f64::from(s.flute_count);
        // P20e: the stage states what the G-code commands: the steady
        // feed under the op cap and the machine ceiling, with its limit.
        let feed_per_tooth_mm = if divisor > 0.0 {
            steady_feed.mm_min() / divisor
        } else {
            0.0
        };
        crate::feeds::CommandedStage {
            feed_rate_mm_min: steady_feed.mm_min(),
            op_feed_mm_min: steady_feed.op_feed_mm_min(),
            feed_limit: steady_feed.limit(),
            spindle_rpm: s.spindle_rpm,
            flute_count: s.flute_count,
            feed_per_tooth_mm,
            // P11b2: set after the loop, at the median measured lift of
            // the trip set.
            thinning_lift: 1.0,
            peak_chip_mm: feed_per_tooth_mm,
        }
    });
    // P11b2 (L1, L2), P29-6: the thinning rule of the op's body cut
    // (`OperationConfig::cuts`), not the routed row role. A drill cycle
    // never reaches this line (the short-circuit at the top).
    let thinning = ctx.body_facts().thins_chip;
    let achieved_feed_stage = {
        let predicted_feeds_present = !trace.predicted_feeds.is_empty();
        let median_ratio = if predicted_feeds_present {
            let mut ratios: Vec<f64> = removing_samples
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
                #[expect(clippy::indexing_slicing, reason = "non-empty checked above")]
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
    // P11b2: the lift of each trip-set sample, for the commanded peak chip
    // (the median lift of the measured cut).
    let mut trip_lifts: Vec<f64> = Vec::new();
    // D7 — span-aware steady-state filter. Samples whose span_path
    // contains a `SpanKind::Entry` ancestor (configured plunge / helix /
    // ramp entries laid down by D4 / D5) are kept out of the
    // steady-state trip set and surfaced separately as `entry_spikes`
    // on the `Within` arm. Replaces the C1 kinematics filter reverted
    // in D3 (commit `a07cfa2`). `span_lookup` was constructed alongside
    // `lookup_axial_doc_mm` above.
    // Worst Entry-ancestry over-max sample (largest observed_fpt > max).
    let mut entry_high: Option<(f64, usize)> = None;
    // Worst Entry-ancestry under-min sample (smallest observed_fpt < min).
    let mut entry_low: Option<(f64, usize)> = None;
    // X-VAC (`planning/review_2026-08-08/XVAC_CENSUS.md`). `offered` is
    // the removing set handed to this loop; `contributing` is
    // `burn_samples.len()` below — the samples that actually reached a
    // bound comparison. The two differ by the sample-validity predicate,
    // the no-divisor skip, and the phantom / configured-entry split, and
    // the second can be zero while the first is large.
    //
    // Deliberately NOT `valid_count`: that counter increments before the
    // transit skip and is documented at its own definition as
    // overstating the gate's denominator. Publishing an overstated count
    // as the vacuity marker would reproduce the defect in the fix.
    let offered = removing_samples.len();

    for (i, s) in removing_samples {
        // **The sample-validity predicate** (`has_chip_number`). P29-6
        // changed it twice: an air-class removing sample has no number
        // (Q2), and an arc of 0 counts as no arc (Q7). The rest of this note
        // is the record of 2026-08-06. Before 2026-08-06 the observation WAS
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
        if !has_chip_number(s) {
            // P29-6 Q7: an arc of 0 is no arc. A removing skin above the
            // tip has no measured arc, not an unsupported chip geometry.
            // Q2: an air-class removing sample has no measured arc either.
            if s.is_air() || s.arc_engagement_radians.is_none_or(|arc| arc <= 0.0) {
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
        //
        // P11b2 (rulings L1, L2): the observation is the PEAK chip, the
        // achieved advance over the radial chip-thinning lift at the
        // sample's measured engagement (`display::peak_chip_per_tooth`, the
        // one law). A thinning cut whose sample gives no engagement cannot
        // state its peak chip; it counts with the samples that carry no arc.
        let (observed_fpt, sample_lift) =
            match super::display::peak_chip_per_tooth(s, &trace.predicted_feeds, tool, thinning) {
                Ok(reading) => (reading.peak_chip_mm, reading.lift),
                Err(super::display::PeakChipRefusal::NoDivisor) => {
                    no_divisor_count += 1;
                    continue;
                }
                Err(super::display::PeakChipRefusal::NoEngagement) => {
                    missing_arc_count += 1;
                    continue;
                }
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
        // EXEMPT(P28-22, P28-23)
        if super::locality::is_phantom_transit(s, span_lookup.as_ref()) {
            continue;
        }
        // EXEMPT(P28-21, M3)
        if super::locality::is_configured_entry(s, span_lookup.as_ref()) {
            // Checkpoint K (b1) — the advisory sides read the same
            // boundary contract as the trip sides, at zero tolerance:
            // an entry spike reported at 1 ulp over a bound is the same
            // unexplainable reading as a verdict at 1 ulp over it.
            if bounds.exceeds_high(observed_fpt, 0.0)
                && entry_high.is_none_or(|(prev, _)| observed_fpt > prev)
            {
                entry_high = Some((observed_fpt, i));
            }
            if bounds.below_low(observed_fpt, 0.0).unwrap_or(false)
                && entry_low.is_none_or(|(prev, _)| observed_fpt < prev)
            {
                entry_low = Some((observed_fpt, i));
            }
            continue;
        }
        burn_samples.push((observed_fpt, i));
        trip_lifts.push(sample_lift);
        // Layer 1 tolerance band: a single sample 1-2% over `max` (e.g.
        // wanaka TP4 5/2026 transient at 1.05% over) shouldn't flip the
        // verdict to `Exceeds(High)`. The widened trigger is purely a
        // gate-trip decision; the underlying `peak_above` deviation is
        // still recorded so downstream displays surface the value.
        //
        // Checkpoint K (b1): the comparison itself lives on `ChipBounds`
        // now, so the tolerance dial and the boundary epsilon compose in
        // one place. They are different things — the dial is the
        // optimizer's, defaults to zero, and moves every verdict; the
        // epsilon absorbs the multiply→divide reconstruction and moves
        // only verdicts float noise decided (G-CHIP-ULP).
        if bounds.exceeds_high(observed_fpt, tolerance.breakage) {
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
    // P11b2: the commanded peak chip is the commanded advance over the lift
    // of the STATED cut (`ctx.commanded_thinning_lift()`, Suggest's own rule),
    // so `commanded_above_band` compares the peak chip Suggest sized. With no
    // stated width or depth it is the median measured lift of the trip set
    // (1 on a finish pass or with no trip sample).
    let commanded_lift = match ctx.commanded_thinning_lift() {
        Some(lift) if lift.is_finite() && lift >= 1.0 => lift,
        Some(_) | None => {
            if trip_lifts.is_empty() {
                1.0
            } else {
                trip_lifts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                trip_lifts.get(trip_lifts.len() / 2).copied().unwrap_or(1.0)
            }
        }
    };
    let commanded_stage = commanded_stage.map(|mut c| {
        c.thinning_lift = commanded_lift;
        c.peak_chip_mm = c.feed_per_tooth_mm / commanded_lift;
        c
    });
    *partial_stages = commanded_stage.map(|commanded| PartialFeedStages {
        commanded,
        band: crate::feeds::LutBandStage {
            observation_id: result.observation_id.clone(),
            bounds_source: source,
            row_diameter_mm: result.row_diameter_mm,
            queried_diameter_mm: lookup_key_mm,
            diameter_scale: result.chipload_diameter_scale,
            hardness_scale: result.chipload_hardness_scale,
            is_extrapolated: result.is_extrapolated,
            queried_pass_role: pass_role,
            row_pass_role: result.row_pass_role,
            family_transferred_from: result.family_basis.claim().map(|c| c.home.0),
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
        #[expect(
            clippy::indexing_slicing,
            reason = "median_idx < len() by construction"
        )]
        Some(burn_samples[median_idx])
    };
    // Layer 1 tolerance band: narrow the burn-risk trigger by
    // `burn_tolerance` so noisy LUT-min rows don't flip on a 1-2 %
    // shortfall. The deviation `min_value - median_cl` reported on the
    // verdict still uses the strict LUT min so downstream displays read
    // the real distance to the nominal envelope.
    //
    // A2: the burn reference is the band minimum, or the printed point for
    // a point row (`burn_reference_mm`). For a point the source is
    // `VendorLutPointPreset`, so the trip below is an advisory only.
    let burn_reference = bounds.burn_reference_mm();
    let peak_below: Option<(f64, usize)> = if let Some(reference) = burn_reference
        && let Some((median_cl, median_sample_idx)) = median_sample
        && super::boundary::below_low(median_cl, reference, tolerance.burn)
    {
        Some((reference - median_cl, median_sample_idx))
    } else {
        None
    };

    if valid_count == 0 {
        // Every removing sample was discarded. Pick the dominant
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

    // X-VAC: the population every metric below reports itself against.
    // `burn_samples` IS the trip set — the same vector the median and the
    // in-range peak are drawn from — so its length is the honest count of
    // what decided this verdict.
    let population = super::verdict::GatePopulation::new(
        burn_samples.len(),
        offered,
        super::verdict::PopulationUnit::Samples,
    );

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
                    .with_locality(locality_for(idx))
                    .with_population(population),
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
        let observed = burn_reference.map(|m| m - dev).unwrap_or_default().max(0.0);
        let metric = ChiploadMetric {
            observed_mm_per_tooth: observed,
            statistic: ChiploadStatistic::MedianLow,
            evidence: SampleEvidence::at_with_stat(idx, ChiploadStatistic::MedianLow)
                .with_locality(locality_for(idx))
                .with_population(population),
            bounds: bounds.clone(),
        };
        if bounds.source.low_side_is_advisory() {
            tracing::debug!(
                verdict = "Within",
                advisory = "burn",
                source = ?bounds.source,
                observed_mm_per_tooth = observed,
                burn_reference_mm_per_tooth = burn_reference.unwrap_or(f64::NAN),
                "chipload gate: median below the burn reference but its \
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
                .with_locality(locality_for(median_idx))
                .with_population(population),
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
                .with_population(population)
        } else {
            SampleEvidence::empty().with_population(population)
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

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationType;
    use crate::feeds::vendor_lut::ToolFamily;
    use crate::feeds::vendor_normalize::lut_query_for;
    use crate::material::Material;
    use crate::material::WoodSpecies;
    use crate::stock::simulation_cut::{
        SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
    };
    use crate::tool::ToolDefinition;
    use crate::tool::{FlatEndmill, VBitEndmill};

    /// Adapts this module's legacy positional-arg test calls to the
    /// Phase 6 `(ctx, env)` gate signature.
    #[expect(
        clippy::too_many_arguments,
        reason = "test code accepts the style lint too_many_arguments"
    )]
    fn evaluate_args(
        toolpath_id: usize,
        tool: &crate::tool::ToolDefinition,
        material: &crate::material::Material,
        sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
        spans: Option<&[crate::trace::toolpath_spans::Span]>,
        operation_feed_rate_mm_min: f64,
        operation_kind: crate::compute::catalog::OperationType,
        tolerance: &crate::tool_load::ToleranceBands,
    ) -> ChiploadVerdict {
        // P29-6: the vendor row key and the body facts come from the op.
        evaluate(
            &crate::tool_load::ToolpathLoadContext::of_operation(
                ToolpathId::from_raw(toolpath_id),
                &crate::compute::catalog::OperationConfig::new_default(operation_kind),
                tool,
                material,
                crate::machine::SteadyFeedMmMin::op_feed_only(operation_feed_rate_mm_min),
                spans,
                crate::limits::CapTable::empty(),
                None,
            ),
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
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        )
    }

    fn vbit_tool() -> ToolDefinition {
        ToolDefinition::new(
            Box::new(VBitEndmill::new(6.35, 90.0, 20.0).unwrap()),
            6.35,
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
    ///
    /// P11b2: the gate observes the PEAK chip, the advance over the radial
    /// chip-thinning lift at the sample's arc. The old arc (1.0844 rad, below
    /// 90 degrees) lifted by 1 / sin(1.0844) = 1.13, which moved the
    /// `observed == chipload` premise of every fixture below. A full slot
    /// (pi) has lift 1, so the premise holds and no expectation moves.
    /// `the_gate_observes_the_peak_chip_at_the_sample_arc_p11b2` covers a
    /// light arc.
    const TEST_LUT_NOMINAL_ARC_RAD: f64 = std::f64::consts::PI;

    /// The `chipload` argument is the advance per tooth the sample is to
    /// be **observed** at. [`trace`] realises it through the F-035
    /// predicted-feed map — see its doc for why, and for the property
    /// that makes every verdict below unchanged across the conversion.
    fn sample(tp_id: usize, idx: usize, chipload: f64, engagement: f64) -> SimulationCutSample {
        SimulationCutSample {
            toolpath_id: ToolpathId::from_raw(tp_id),
            move_index: idx,
            sample_index: idx,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 1000.0,
            spindle_rpm: 18000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            arc_engagement_radians: Some(TEST_LUT_NOMINAL_ARC_RAD),
            chipload_mm_per_tooth: chipload,
            effective_chip_thickness_mm: Some(chipload),
            engagement: crate::stock::simulation_cut::Engagement::with_radial_woc(engagement),
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
    /// The commanded `feed_rate_mm_min` stays where each test set it. Since
    /// P29-6 6b no filter reads it: the population is every removing sample.
    fn trace(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
        let mut predicted_feeds = crate::machine::kinematics::PredictedFeedMap::new();
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
        // 2026-06-03 family-default Janka anchor (279fcee): the Onsrud
        // 6.35 mm hardwood row carries no per-row `hardness_value`, so
        // `hardness_ratio_raw` (then named `hardness_scale_factor`) previously degraded to identity for
        // any hardwood query. After 279fcee it scaled against a
        // hardwood family anchor of Janka 1290 (red oak).
        //
        // RE-CENTRED 2026-09-24 (extrapolation P2 step 3, one Janka
        // table): the row default is now `WoodSpecies::GenericHardwood`
        // (1450). HardMaple is 1450, so the hardness scale is 1.0 and the
        // band is the printed 0.3556–0.4064 mm/tooth. The old sample 0.34
        // sat in the ^0.5 band of the 1290 anchor, (1290 / 1450)^0.5 =
        // 0.9432 → 0.3354–0.3833, but it is below the new band. The sample
        // 0.38 is in both bands. The routing assertion (ProjectCurve→Trace
        // → contour/finish row) is unchanged.
        //
        //
        // RE-PINNED 2026-10-31 (series ruling, `feeds::series`): the sample
        // is the midpoint of the band that the gate's own resolver gives, so
        // the test pins the routing (a contour row), not a transcribed
        // number. The Onsrud 60-100MW row is a compression row; no standard
        // row ties with it on material and size, so it still serves here.
        let maple = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let row = matched_chip_envelope(
            &tool(),
            &maple,
            OperationType::ProjectCurve,
            LutOperationFamily::Trace,
            LutPassRole::Finish,
            crate::feeds::geometry::lut_key_diameter_for_cutter(&tool(), 0.5),
        )
        .expect("the routed query matches a chipload row");
        assert_eq!(
            row.family_basis.claim().map_or_else(
                || embedded_lut()
                    .observations
                    .iter()
                    .find(|o| o.observation_id == row.observation_id)
                    .map(|o| o.operation_family),
                |c| Some(c.home.0)
            ),
            Some(LutOperationFamily::Contour),
            "ProjectCurve routes to a contour row: {}",
            row.observation_id
        );
        let (lo, hi) = (
            row.chip_load_min_mm.expect("a band"),
            row.chip_load_max_mm.expect("a band"),
        );
        // Arc override prevents D9's per-sample normalization from
        // perturbing row selection (contour-finish rows have narrower
        // nominal arcs than the pocket-roughing default in `sample()`).
        let mut s = sample(0, 0, 0.5 * (lo + hi), 0.5);
        s.arc_engagement_radians = Some(0.459);
        let t = trace(vec![s]);
        let v = evaluate_args(
            0,
            &tool(),
            &maple,
            Some(&t),
            None,
            1000.0,
            OperationType::ProjectCurve,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(v, ChiploadVerdict::Within { .. }),
            "expected Within (routing landed in a contour/finish row with sample in band), got {v:?}"
        );
    }

    /// Operator ruling 2026-09-25 ("a ProjectCurve on a V-bit routes ...
    /// as a trace"): `lut_query_for` now routes `ChamferVbit` to
    /// `(Trace, Finish)` instead of refusing, so this cell is no longer
    /// `Unmodeled`. The gate's LUT query lands on the printed trace row of
    /// this 6.35 mm 90 degree tool: since 2026-10-01 the Onsrud 37-50 1/4 in
    /// Hard Wood row `onsrud-hardwood-37-50-1_4-trace` (.003-.006 in =
    /// 0.0762-0.1524 mm/tooth, printed at 6.35 mm, so no size claim). The
    /// sample sits at the band middle, (0.0762 + 0.1524) / 2 = 0.1143.
    /// Before, the row was `amana-vbit-hardwood-trace-6000-2f` with the
    /// sample at 0.045; that row is retired (finding G5 D1: not on its
    /// chart), and 0.045 is under the Onsrud band.
    #[test]
    fn project_curve_vbit_routes_to_the_printed_trace_row() {
        let t = trace(vec![sample(0, 0, 0.1143, 0.5)]);
        let v = evaluate_args(
            0,
            &vbit_tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            1000.0,
            OperationType::ProjectCurve,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(
                v,
                ChiploadVerdict::Within {
                    burn_advisory: None,
                    ..
                }
            ),
            "expected Within with no burn advisory (routing landed in the printed V-bit trace \
             row with the sample in band), got {v:?}"
        );
    }

    #[test]
    fn adaptive3d_reroutes_from_adaptive_to_pocket_family() {
        // Adaptive3d's declared feeds_family is Adaptive, but its path
        // geometry is closer to pocket-style clearing in wood. The router
        // overrides Adaptive → Pocket so the LUT envelope reflects the
        // correct mechanical regime (design doc §1.3, §10 sign-off).
        let routed = lut_query_for(
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
        let routed = lut_query_for(
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
        let routed = lut_query_for(
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
        let routed = lut_query_for(
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
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn no_trace_returns_simulation_required() {
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            None,
            None,
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
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
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
    /// (extrapolated here: a 3.175 mm ball against a 6.0 mm row, past
    /// the documented ±40 % extrapolation threshold) must
    /// NOT hard-refuse with `Exceeds(Low)`: it lands `Within` with a
    /// structured `burn_advisory` carrying the same MedianLow metric
    /// the trip would have reported. The breakage side stays hard for
    /// every provenance.
    ///
    /// RE-PREMISED 2026-09-24 (extrapolation P1, ruling A1). This test
    /// ran a Parallel finish, then a Scallop at a 0.5 mm tapered tip. P1
    /// step 3 refuses that tip on the Onsrud 1/8 in Scallop row (6.35x, no
    /// printed series), so the gate no longer judges it. The test now uses a
    /// G1 form C claim that stays extrapolated: a 3.175 mm ball nose on a
    /// hardwood Scallop matches `amana-ball-hardwood-scallop-6000-2f`
    /// (0.025-0.04 mm/tooth, Janka 1450) at a raw ratio of
    /// 3.175 / 6.0 = 0.529, inside the 0.5x-2x window of form C and past the
    /// ln 1.4 extrapolation flag (|ln 0.529| = 0.64). The claimed band is
    /// 0.0170-0.0271 mm/tooth, so 0.0001 is far below the minimum.
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn weak_provenance_low_trip_demotes_to_burn_advisory() {
        use crate::tool::BallEndmill;
        let ball = ToolDefinition::new(
            Box::new(BallEndmill::new(3.175, 12.0)),
            3.175,
            35.0,
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        );
        // Far below any scaled min — would trip Exceeds(Low) on a
        // calibrated row.
        let t = trace(vec![sample(0, 0, 0.0001, 0.5)]);
        let v = evaluate_args(
            0,
            &ball,
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            1000.0,
            OperationType::Scallop,
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
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn chipload_far_below_min_is_exceeds_burn() {
        // 0.001 mm/tooth — rubbing, on the burn fixture (see
        // `burn_fixture_lut_min`).
        let min = burn_fixture_lut_min();
        assert!(0.001 < min);
        let v = burn_fixture_verdict(0.001, &crate::tool_load::ToleranceBands::default());
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
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(v, ChiploadVerdict::Within { .. }));
    }

    // -------- P29-6 6b: no feed filter ---------

    /// The contributing count of a judged verdict (the trip set).
    fn contributing(v: &ChiploadVerdict) -> usize {
        match v {
            ChiploadVerdict::Within {
                approach_to_max, ..
            } => approach_to_max
                .evidence
                .population
                .map(|p| p.contributing)
                .unwrap_or_default(),
            ChiploadVerdict::Exceeds { triggering, .. } => triggering
                .evidence
                .population
                .map(|p| p.contributing)
                .unwrap_or_default(),
            ChiploadVerdict::Unmodeled { reason } => {
                panic!("expected a judged verdict, got Unmodeled({reason:?})")
            }
        }
    }

    /// P29-6 (ruling Q2): on a finish op (no thinning, so the peak chip
    /// reads no engagement) an air-class sample that removes material and
    /// carries a chip model is in the population with no number. A chip
    /// far above the band on it does not trip the gate.
    #[test]
    fn an_air_class_removing_sample_has_no_chip_number_on_a_finish_op() {
        let lut_max = match burn_fixture_verdict(0.03, &crate::tool_load::ToleranceBands::default())
        {
            ChiploadVerdict::Within {
                approach_to_max, ..
            }
            | ChiploadVerdict::Exceeds {
                triggering: approach_to_max,
                ..
            } => approach_to_max.bounds.max_mm_per_tooth,
            ChiploadVerdict::Unmodeled { reason } => {
                panic!("the burn fixture must be judged, got Unmodeled({reason:?})")
            }
        };
        let cut = burn_fixture_sample(0.03);
        let mut skin = burn_fixture_sample(10.0 * lut_max);
        skin.move_index = 1;
        skin.sample_index = 1;
        skin.engagement = crate::stock::simulation_cut::Engagement::with_radial_woc(0.01);
        assert!(skin.is_air() && skin.removes_material(), "non-vacuity");
        let t = trace(vec![cut, skin]);
        let v = evaluate_args(
            0,
            &burn_fixture_tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            1000.0,
            OperationType::Scallop,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(v, ChiploadVerdict::Within { .. }),
            "an air-class sample must not trip the gate, got {v:?}"
        );
        assert_eq!(contributing(&v), 1, "the air-class sample has no number");
    }

    /// P29-6 6b (was Item C verify #1, which pinned
    /// `SteadyStateSamplesNotPresent`): a toolpath whose every removing
    /// sample runs at 30 % of the commanded feed is judged. The gate grades
    /// the chip of every removing sample at any feed.
    #[test]
    fn all_low_feed_samples_are_judged() {
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
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert_eq!(contributing(&v), 2, "both low-feed samples judged: {v:?}");
    }

    /// P29-6 6b (was Item C verify #2, which pinned that the ramp-feed
    /// sample is dropped): steady-feed Linear, steady-feed Helix and a
    /// ramp-feed Linear sample all enter the trip set. The median stays on
    /// the two steady samples, so the verdict stays `Within`.
    #[test]
    fn ramp_feed_samples_are_judged() {
        let mut linear = sample(0, 0, 0.04, 0.5);
        linear.feed_rate_mm_min = 1500.0;
        linear.cut_kinematics = crate::stock::simulation_cut::CutKinematics::Linear;
        let mut helix = sample(0, 1, 0.04, 0.5);
        helix.feed_rate_mm_min = 1500.0;
        helix.cut_kinematics = crate::stock::simulation_cut::CutKinematics::Helix;
        let mut ramp = sample(0, 2, 0.001, 0.5);
        ramp.feed_rate_mm_min = 500.0;
        ramp.cut_kinematics = crate::stock::simulation_cut::CutKinematics::Linear;
        let t = trace(vec![linear, helix, ramp]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
        assert_eq!(contributing(&v), 3, "the ramp sample is judged: {v:?}");
    }

    /// P29-6 6b (was the 95 % boundary test): a sample at 94.9 % of the
    /// commanded feed, which the deleted filter dropped, is judged.
    #[test]
    fn a_sample_below_the_old_feed_threshold_is_judged() {
        let mut s = sample(0, 0, 0.04, 0.5);
        s.feed_rate_mm_min = 949.0;
        let t = trace(vec![s]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(v, ChiploadVerdict::Within { .. }),
            "a 94.9 % sample must be judged, got {v:?}"
        );
        assert_eq!(contributing(&v), 1);
    }

    /// P29-6 Q7: a removing sample with an arc of 0 and no chip model has
    /// no measured arc. The refusal names `ArcEngagementNotCaptured`, not
    /// an unsupported chip geometry.
    #[test]
    fn an_arc_of_zero_is_no_arc() {
        let mut s0 = sample(0, 0, 0.04, 0.5);
        s0.arc_engagement_radians = Some(0.0);
        s0.effective_chip_thickness_mm = None;
        let t = trace(vec![s0]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(
                v,
                ChiploadVerdict::Unmodeled {
                    reason: UnmodeledReason::ArcEngagementNotCaptured
                }
            ),
            "an arc of 0 is no arc, got {v:?}"
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
        s0.cut_kinematics = crate::stock::simulation_cut::CutKinematics::Helix;
        let mut s1 = sample(0, 1, 0.5, 0.5);
        s1.feed_rate_mm_min = 1500.0;
        s1.cut_kinematics = crate::stock::simulation_cut::CutKinematics::Helix;
        let t = trace(vec![s0, s1]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
    /// LUT row's chipload envelope derated by `doc_derating_scale(ratio)`
    /// down to 0.5× at 3×D, and well-behaved steady-state samples landed
    /// above the phantom-derated max → false `Exceeds(High)`. P11b2 removed
    /// the de-rate; the depth still keys the row, so the sentry stays.
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
            1000.0,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(v, ChiploadVerdict::Within { .. }),
            "phantom-transit sample must not inflate DOC-derating ratio, got {v:?}"
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

    /// Resolve the LUT chipload max for the 6.35 mm flat / HardMaple /
    /// Pocket / Roughing fixture by running a known-Within evaluation and
    /// reading the bounds the verdict carries. Lets the tolerance-band
    /// tests below scale their probe samples relative to the actual row,
    /// so future LUT changes don't break the test setup.
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
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

    /// The fixture whose matched row lets the gate trip a HARD `Exceeds(Low)`:
    /// a Ø6.0 ball nose ([`burn_fixture_tool`]) on a hard-maple Scallop
    /// finish, sampled 3 mm deep (3 / 6 = 0.5 x D, so the depth de-rate is
    /// 1.0). The LUT query lands on `amana-ball-hardwood-scallop-6000-2f`
    /// (0.025-0.04 mm/tooth, ae window, Janka 1450) at its own 6.0 mm key:
    /// no size claim and no extrapolation flag, so the low side stays hard.
    ///
    /// The low side is hard only on a row that publishes both chipload
    /// bounds AND an `ae` window and is not extrapolated
    /// (`ChipBoundsSource::VendorLut`). Feeds matrix R5 (2026-09-23): the
    /// Ø6.35 flat hard-maple pocket cell this module used to read now
    /// resolves to `amana-compression-wood-pocket-6350-2f`, a single-point
    /// band whose low side is advisory by policy, and no printed flat
    /// 6.35 mm row is two-sided and windowed. Until 2026-10-01 the fixture
    /// was the Ø6.35 90 degree V-bit on a trace finish, on the insert
    /// V-groove row `amana-vbit-hardwood-trace-6000-2f`. That row is
    /// retired (finding G5 D1: not on its chart). The V-bit now reads the
    /// printed Onsrud 37-50 1/4 in Hard Wood row, which prints no `ae`
    /// window, so its low side is advisory and it cannot carry this
    /// fixture.
    const BURN_FIXTURE_DOC_MM: f64 = 3.0;

    fn burn_fixture_tool() -> ToolDefinition {
        use crate::tool::BallEndmill;
        ToolDefinition::new(
            Box::new(BallEndmill::new(6.0, 20.0)),
            6.0,
            30.0,
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        )
    }

    fn burn_fixture_sample(chipload: f64) -> SimulationCutSample {
        let mut s = sample(0, 0, chipload, 0.5);
        s.axial_doc_mm = BURN_FIXTURE_DOC_MM;
        s.axial_engagement_mm = BURN_FIXTURE_DOC_MM;
        s
    }

    /// The gate's verdict for the burn fixture at `chipload` mm/tooth.
    fn burn_fixture_verdict(
        chipload: f64,
        bands: &crate::tool_load::ToleranceBands,
    ) -> ChiploadVerdict {
        let t = trace(vec![burn_fixture_sample(chipload)]);
        evaluate_args(
            0,
            &burn_fixture_tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
            1000.0,
            OperationType::Scallop,
            bands,
        )
    }

    /// The band minimum of the row the gate matches for the burn fixture.
    /// The bounds must be the hard class, or the low-side tests below
    /// measure nothing.
    fn burn_fixture_lut_min() -> f64 {
        let v = burn_fixture_verdict(0.03, &crate::tool_load::ToleranceBands::default());
        match v {
            ChiploadVerdict::Within {
                approach_to_max, ..
            }
            | ChiploadVerdict::Exceeds {
                triggering: approach_to_max,
                ..
            } => {
                assert_eq!(
                    approach_to_max.bounds.source,
                    ChipBoundsSource::VendorLut,
                    "the burn fixture must match a row whose low side is hard"
                );
                approach_to_max
                    .bounds
                    .min_mm_per_tooth
                    .expect("LUT row carries a min for the chosen fixture")
            }
            other @ ChiploadVerdict::Unmodeled { .. } => {
                panic!("expected a judged verdict, got {other:?}")
            }
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

    /// P11b2 (rulings L1, L2): at a light arc the gate observes the peak
    /// chip, the advance over `1 / sin(psi)`; on a finish pass it observes
    /// the advance.
    #[test]
    fn the_gate_observes_the_peak_chip_at_the_sample_arc_p11b2() {
        let advance = 0.05;
        let psi = 0.8_f64;
        let light = SimulationCutSample {
            arc_engagement_radians: Some(psi),
            ..sample(0, 0, advance, 0.15)
        };
        // P29-6: the op states the role. A Pocket roughs; a Drop Cutter
        // finish is a finish pass (no thinning).
        let observe = |op: OperationType| {
            let t = trace(vec![light.clone()]);
            let v = evaluate_args(
                0,
                &tool(),
                &Material::SolidWood {
                    species: WoodSpecies::HardMaple,
                },
                Some(&t),
                None,
                1000.0,
                op,
                &crate::tool_load::ToleranceBands::default(),
            );
            match v {
                ChiploadVerdict::Within {
                    approach_to_max, ..
                } => approach_to_max.observed_mm_per_tooth,
                ChiploadVerdict::Exceeds { triggering, .. } => triggering.observed_mm_per_tooth,
                ChiploadVerdict::Unmodeled { reason } => panic!("Unmodeled({reason:?})"),
            }
        };
        let rough = observe(OperationType::Pocket);
        assert!(
            (rough - advance * psi.sin()).abs() < 1e-9,
            "the peak chip is fz sin(psi): {rough}"
        );
        let finish = observe(OperationType::DropCutter);
        assert!(
            (finish - advance).abs() < 1e-12,
            "a finish pass observes the advance: {finish}"
        );
    }

    #[test]
    fn chipload_low_just_below_min_within_tolerance_is_within() {
        let min = burn_fixture_lut_min();
        let probe = min * 0.96;
        let bands = crate::tool_load::ToleranceBands {
            burn: 0.05,
            ..crate::tool_load::ToleranceBands::default()
        };
        let v = burn_fixture_verdict(probe, &bands);
        assert!(
            matches!(v, ChiploadVerdict::Within { .. }),
            "expected Within with burn_tolerance=0.05 at 0.96×min, got {v:?}"
        );
    }

    #[test]
    fn chipload_low_below_tolerance_is_exceeds() {
        let min = burn_fixture_lut_min();
        let probe = min * 0.94;
        let bands = crate::tool_load::ToleranceBands {
            burn: 0.05,
            ..crate::tool_load::ToleranceBands::default()
        };
        let v = burn_fixture_verdict(probe, &bands);
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
        s.cut_kinematics = crate::stock::simulation_cut::CutKinematics::Linear;
        let t = trace(vec![s]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn drill_like_no_arc_samples_route_to_unmodeled() {
        // the sample removes material, so it is in the population and
        // reaches the
        // chip-geometry path. arc=None mirrors a real plunge move
        // where the simulator can't compute an engagement arc.
        let mut s0 = sample(0, 0, 0.5, 0.5);
        s0.feed_rate_mm_min = 1500.0;
        s0.cut_kinematics = crate::stock::simulation_cut::CutKinematics::Plunge;
        s0.arc_engagement_radians = None;
        s0.effective_chip_thickness_mm = None;
        let mut s1 = sample(0, 1, 0.5, 0.5);
        s1.feed_rate_mm_min = 1500.0;
        s1.cut_kinematics = crate::stock::simulation_cut::CutKinematics::Plunge;
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
        s.cut_kinematics = crate::stock::simulation_cut::CutKinematics::Plunge;
        let t = trace(vec![s]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&t),
            None,
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
