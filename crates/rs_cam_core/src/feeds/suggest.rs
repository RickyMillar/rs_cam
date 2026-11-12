//! Canonical feed/speed suggestion plumbing.
//!
//! This module owns the bridge from an [`OperationConfig`] + project context into
//! the feeds calculator, and the write-back path from a [`FeedsResult`] into an
//! operation. GUI, MCP, and diagnostics call through here so recommendations and
//! pre-sim warning baselines stay in lock-step.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{FeedsError, FeedsInput, FeedsResult, VendorLut};
use crate::machine::MachineProfile;
use crate::material::Material;

mod adaptive_entry;
mod apply;
mod axial_envelope;
mod invariants;

#[cfg(any(test, feature = "test-support"))]
pub use apply::apply_speeds_to_op;
pub(crate) use apply::peck_for_hole;
pub use apply::{
    ApplyContext, Complete, Completeness, ExploredSpeeds, FeedsPreview, FieldApplyPreview,
    FieldApplyPreviews, OperationApplyInput, Partial, PickedGeometry, Recommendation, apply,
    apply_drill_defaults, apply_stock_defaults, apply_to_operation, feeds_preview_for_operation,
    operation_feeds_hints, preview_field_applies, preview_field_apply,
};
pub(crate) use axial_envelope::axial_envelope_for_operation;
// The deflection back-off target, re-exported so `feeds::rationale` can
// format the bound it renders from the constant the loop compares
// against. The text cannot then drift from the number.
pub(crate) use invariants::DEFLECTION_BACKOFF_TARGET_UM;
// P5 (K2b): the F201 rows of the envelope's runtime and deflection limits,
// for `formula_constants` and the envelope.
pub(crate) use invariants::{
    DEFLECTION_BACKOFF_TARGET_ROW, STEPOVER_BACKOFF_DIAMETER_FRACTION_ROW,
    STEPOVER_BACKOFF_TARGET_MOVES_ROW,
};

/// Stock-derived values used by stock-aware operation defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StockContext {
    /// Top of the stock in part-Z coordinates.
    pub stock_top_z: f64,
    /// Bottom of the stock in part-Z coordinates.
    pub stock_bottom_z: f64,
    /// Stock thickness in mm, or `None` when the stock has no thickness
    /// (S11: not set). No through-stock floor is written then.
    pub stock_z: Option<f64>,
    /// Stock-top minus model-top margin in mm.
    pub stock_padding: f64,
}

impl StockContext {
    /// Build a stock context from a stock bbox and padding value.
    pub fn from_stock_bbox(bbox: crate::geo::BoundingBox3, padding: f64) -> Self {
        // S11: a flat box is a stock whose thickness is not set.
        let thickness = bbox.max.z - bbox.min.z;
        let stock_z = (thickness > 0.0).then_some(thickness);
        Self {
            stock_top_z: bbox.max.z,
            stock_bottom_z: bbox.min.z,
            stock_z,
            stock_padding: padding,
        }
    }
}

/// The owned project slots of one toolpath's Suggest call: the stock and
/// the bbox of the model it machines (F114).
///
/// [`SuggestContext`] borrows its slots, so a session method cannot hand
/// one back over locals. [`crate::session::ProjectSession::suggest_slots`]
/// gives this owned carrier, and [`Self::context`] lends the context. Every
/// project door (the Apply funnel, the profile behind the card, MCP and the
/// CLI) assembles its context here, so the stock slot is set on each one.
#[derive(Debug, Clone, Copy)]
pub struct SuggestSlots {
    /// The stock of the project.
    pub stock: StockContext,
    /// The bbox of the model the toolpath machines; `None` when the id
    /// names no model or the model has no finite geometry.
    pub model_bbox: Option<crate::geo::BoundingBox3>,
}

impl SuggestSlots {
    /// The Suggest context of these slots and the toolpath's dressups (the
    /// G6 entry θ), with the default policy.
    #[must_use]
    pub fn context<'a>(
        &'a self,
        dressups: &'a crate::compute::config::DressupConfig,
    ) -> SuggestContext<'a> {
        SuggestContext {
            model_bbox: self.model_bbox.as_ref(),
            stock: Some(&self.stock),
            dressups: Some(dressups),
            ..SuggestContext::default()
        }
    }
}

/// v3.3a (2026-06-04): How aggressively the orchestrator rewrites
/// strategy fields (entry_style, clearing_strategy, stock_to_leave_*).
/// The chipload / DPP / stepover clamps and back-offs run under both
/// scopes — this enum gates the *strategy* layer specifically.
///
/// Default (v3.3c, 2026-06-04) is `StrategyAndFeeds` per the directive
/// "stratagy is important. and the architecture should be built to
/// support this". `FeedsWithGates` preserves the pre-v3.3 / v3.0d
/// recipe (clamps + back-offs only, no strategy rewrites).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SuggestScope {
    /// Clamps + back-offs + chipload recalibration. Strategy fields
    /// (entry_style, clearing_strategy, stock_to_leave_*) are not
    /// rewritten — Suggest only emits warnings (e.g.
    /// [`SuggestWarning::PlungeEntryUnstableAtDpp`]) when they look
    /// risky. Pre-v3.3 / v3.0d behaviour.
    FeedsWithGates,
    /// Above + strategy advice. The clearing-strategy pass is warn-only.
    /// Suggest never writes the entry style (ruling Q11, G10,
    /// 2026-09-25): the operator owns it. v3 default per the
    /// "strategy-aware Suggest" directive.
    #[default]
    StrategyAndFeeds,
}

/// v3.0b (2026-06-04): caller-supplied policy that the combined-Suggest
/// orchestrator threads through [`SuggestContext`].
///
/// Ruling R4 (2026-09-24) deleted the `aggressiveness` field and its enum
/// `SuggestAggressiveness` (Conservative / Default / Speed). It placed the
/// chipload inside the band, and it was inert since 2026-08-13. The ruling
/// holds the chipload at the band midpoint times the depth ladder. D9
/// (2026-11-04) removed the machine dial too; machine rigidity is
/// `MachineProfile::rigidity`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SuggestPolicy {
    /// How aggressively to rewrite strategy fields. See [`SuggestScope`].
    pub scope: SuggestScope,
}

/// Project-level context that gate-aware Suggest needs to forward-predict
/// runtime + cross-toolpath constraints. Slots that aren't relevant for a
/// given Suggest invocation should be `None` — the caller provides what
/// it can, the calculator gracefully degrades.
///
/// v1.1 ignores every field on this struct; it exists so v1.2
/// (runtime-sanity stepover floor) and v2 (finish-DPP from upstream
/// leftover) can read project state without grabbing the whole
/// [`crate::session::ProjectSession`]. Callers should populate what
/// they cheaply have and pass [`SuggestContext::default()`] when they
/// don't.
#[derive(Debug, Clone, Copy, Default)]
pub struct SuggestContext<'a> {
    /// AABB of the model this toolpath is operating on. Used by v1.2 to
    /// estimate move count at a proposed stepover, and by future
    /// strategy selection to classify model complexity.
    pub model_bbox: Option<&'a crate::geo::BoundingBox3>,
    /// Stock context (top/bottom Z, stock_z, padding). Same struct
    /// Suggest already uses for stock-aware defaults — re-exposed here
    /// so v1.2+ paths can read stock dimensions without a second
    /// out-of-band parameter.
    pub stock: Option<&'a StockContext>,
    /// Amount of material left from the upstream toolpath in this
    /// setup, in mm. `None` when this is the first op in the setup or
    /// when the caller can't cheaply compute it. v1 doesn't use this;
    /// v2 finish-DPP selection will.
    pub upstream_leftover_stock_mm: Option<f64>,
    /// Strategy choice from a neighboring (rough → finish) op. `None`
    /// when not applicable. v1 doesn't use this.
    pub neighboring_strategy_hint: Option<&'a str>,
    /// LUT chipload band for the matched op × tool combination, when
    /// [`FeedsResult::chipload_bounds`] could derive one. Read by the
    /// v2 step 2 chipload-aware feed-up recalibration in
    /// [`enforce_invariants_at_fixed_geometry`](invariants::enforce_invariants_at_fixed_geometry). `apply` lifts this
    /// off `FeedsResult` automatically; direct callers of
    /// `enforce_invariants_at_fixed_geometry` (tests synthesizing post-v1.1 operating
    /// points) populate it explicitly. `None` means "no constraint
    /// signal" — the recalibration short-circuits.
    pub chipload_bounds: Option<crate::feeds::ChiploadBounds>,
    /// Matched LUT row (post-scaling) — populated by `apply`
    /// from `FeedsResult::matched_lut_row`. Consumed by `pick_axial_envelope`
    /// to derive the vendor ap bound. `None` when the calculator did not
    /// match any vendor row.
    pub matched_lut_row: Option<&'a crate::feeds::vendor_lookup::LookupResult>,
    /// Effective cutter diameter at the calculator's commanded axial DOC
    /// (mm). Used by the chipload-bounds re-derivation step in
    /// `enforce_invariants_at_fixed_geometry` when the axial-envelope pass mutates DPP —
    /// the new `doc_derating_scale(new_dpp / effective_d)` must be
    /// applied to keep `chipload_bounds` consistent with the post-mutation
    /// operating point.
    ///
    /// **Sentinel contract (C2, 2026-07-30) — DOCUMENTED AND TESTED, not
    /// converted.** `0.0` means *not populated* (no calculator result fed
    /// this context), and the single consumer,
    /// [`recompute_chipload_bounds_for_dpp`](axial_envelope::recompute_chipload_bounds_for_dpp), treats a non-positive diameter
    /// exactly as it treats a drill op: `doc_ratio = 0.0`, which
    /// `doc_derating_scale` maps to a scale of 1.0, i.e. the raw LUT band
    /// passes through underated. It is left as `f64` because a cutter
    /// diameter can never legitimately BE zero, so there is no measured-zero
    /// case for `Option` to distinguish, and the sole consumer already
    /// branches on it explicitly. Pinned by
    /// `unpopulated_effective_diameter_skips_doc_derating`.
    pub effective_diameter_mm: f64,
    /// The operating point [`crate::feeds::calculate`] derived its feed at
    /// (G-SUGGEST-NOCLAMP, 2026-08-19). Populated by `apply_feeds_subset`
    /// from the `FeedsResult` it is applying; read only by
    /// [`rescale_feed_to_final_geometry`](adaptive_entry::rescale_feed_to_final_geometry).
    ///
    /// `None` means **there is no derivation point to reconcile against**,
    /// and the rescale pass short-circuits. That is the correct answer for a
    /// drag-to-explore apply (`with_explored_speeds`): a hand-set feed was
    /// never derived from a chip-thinning term, so there is nothing to
    /// re-derive.
    pub calculator_operating_point: Option<CalculatorOperatingPoint>,
    /// v3.0b: caller-supplied policy threading through the
    /// orchestrator. Default = `SuggestPolicy::default()`.
    pub policy: SuggestPolicy,
    /// The toolpath's dressups, for the entry θ of a dressup operation (G6
    /// ramp). `None` means not known; the ramp then falls back to the plunge
    /// rate. Adaptive3d reads its own entry and does not use this slot.
    pub dressups: Option<&'a crate::compute::config::DressupConfig>,
}

/// The point [`crate::feeds::calculate`] evaluated its feed expression at.
///
/// `enforce_invariants_at_fixed_geometry` may legally overwrite the operation's stepover and
/// DPP after the calculator has already folded chip-thinning and depth-tier
/// terms sized at *these* values into the feed. Reconciling the two needs
/// the original point, and it must be the calculator's **unrounded** values:
/// `apply_feeds_subset` writes rounded copies onto the operation, and
/// anchoring the re-derivation on a rounded intermediate would fold that
/// rounding into the ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalculatorOperatingPoint {
    /// `FeedsResult::radial_width_mm` — the `ae` the chip-thinning term was
    /// sized at.
    pub radial_width_mm: f64,
    /// `FeedsResult::axial_depth_mm` — the `ap` the depth-tier term and the
    /// chip-thinning effective diameter were sized at.
    pub axial_depth_mm: f64,
    /// `FeedsResult::feed_rate_mm_min`, unrounded.
    pub feed_rate_mm_min: f64,
    /// `FeedsResult::rpm`. Paired with the feed so the pass can recover the
    /// commanded advance per tooth rather than working in feed units, which
    /// keeps the re-derivation exact when the applied RPM is a rounded copy
    /// of this one.
    pub rpm: f64,
}

impl CalculatorOperatingPoint {
    /// The point `result` was sized at (feeds contract P14, review M9: the
    /// one builder from a [`FeedsResult`]; the surfaces no longer copy the
    /// four fields by hand).
    #[must_use]
    pub const fn of(result: &FeedsResult) -> Self {
        Self {
            radial_width_mm: result.radial_width_mm,
            axial_depth_mm: result.axial_depth_mm,
            feed_rate_mm_min: result.feed_rate_mm_min,
            rpm: result.rpm,
        }
    }
}

/// Warnings emitted while applying suggestions to an operation.
#[derive(Debug, Clone, PartialEq)]
pub enum SuggestWarning {
    PlungeClampedToFeed {
        requested: f64,
        capped: f64,
    },
    /// v1.3 combined-Suggest: the LUT + rigidity-clamp combo wrote a
    /// roughing DPP that exceeds the safe envelope for a plunge entry
    /// on an Adaptive-family op. The entry planner can't engage at
    /// this depth, so the resulting toolpath will produce ~zero
    /// cutting moves. Warning-only — Suggest does not modify the
    /// strategy params (v2 will). User should switch to helix/ramp
    /// entry or reduce DPP manually.
    ///
    /// Threshold: fires when `dpp > 0.5 × tool.diameter` (was 1.0×D in
    /// v1.3 initial). Calibrated against Wanaka plunge-entry spike data
    /// (2026-06-03): a 6 mm tool at DPP=3.69 mm (ratio 0.615) showed a
    /// 362 µm transient deflection spike during plunge entry even though
    /// steady-state landed at 162 µm. A DPP/D ratio of ~0.5 is enough
    /// for the transient to breach the 200 µm gate.
    PlungeEntryUnstableAtDpp {
        dpp_mm: f64,
        diameter_mm: f64,
        entry_style: String,
    },
    /// v3.3c (2026-06-05): the geometry classifier prefers a different
    /// value for a strategy field, but the orchestrator does NOT
    /// auto-apply it. (Its write-back sibling `StrategyRewrote` went with
    /// ruling Q11, G10, 2026-09-25.)
    /// Shipped for `clearing_strategy`, where auto-selection is
    /// deferred to v4 until the classifier is calibrated (the
    /// `ContourParallel` → `GeodesicAdaptive` promotion on mixed terrain needs
    /// uncut-band evidence, not just a bbox heuristic).
    ///
    /// `param` is the snake-case field name, the values are labels, and
    /// `reason` is the short heuristic key. `current` is left untouched on
    /// the operation.
    StrategyRecommendedNotApplied {
        param: &'static str,
        current: String,
        recommended: String,
        reason: &'static str,
    },
    /// **DECLARED 2026-08-19, NOT YET PRODUCED — G-SUGGEST-NOCLAMP.**
    ///
    /// Reserved for the pass that re-derives the feed once
    /// `enforce_invariants_at_fixed_geometry` has settled the operation's final stepover and
    /// DPP. `feeds::calculate` freezes the feed against the geometry it was
    /// handed; later passes (`backoff_stepover_for_runtime`,
    /// `pick_axial_envelope`, the rigidity/deflection clamps) then overwrite
    /// that geometry, leaving the chip-thinning and depth-tier terms derived
    /// at an operating point the operation does not run.
    ///
    /// Measured motivating case (wanaka200 tp 8, `drop_cutter`, R1.5 tapered
    /// ball in White Oak): the calculator sized the feed at `ae = 0.09` — the
    /// `operation_default_profile(Parallel, Finish).ae_factor` fallback,
    /// because no op sends a radial width hint (`CutSpec::todays_hints`) — while
    /// `backoff_stepover_for_runtime` then wrote `ae = 0.30375`. The stale
    /// radial-chip-thinning lift left the commanded advance at
    /// 0.033154 mm/tooth against a band maximum of 0.020553, i.e. **1.613×**,
    /// with no warning. Re-running the calculator at the true stepover yields
    /// 781.02 mm/min, landing exactly on the band maximum.
    ///
    /// The defect is symmetric, so the pass must handle both signs: on tp 5
    /// the axial envelope clamps DPP 9.0 → 4.2 mm while the feed keeps a
    /// `depth_tier` of 0.75 computed at 9.0, where 4.2 mm on a Ø6 tool is
    /// tier 1.0 — that op is **under**-fed by 1.33×.
    ///
    /// Feeds contract P5 deleted its sentry
    /// (`suggest_feed_matches_final_geometry`) with the geometry chain.
    FeedRescaledToFinalGeometry {
        /// `feed_rate` (mm/min) as the calculator and earlier invariants left it.
        requested_mm_per_min: f64,
        /// `feed_rate` (mm/min) after rescaling to the final geometry.
        rescaled_mm_per_min: f64,
        /// Combined chip-thinning × depth-tier factor at the operating point
        /// the calculator sized the feed against.
        factor_at_calculator: f64,
        /// The same combined factor at the operation's final stepover / DPP.
        factor_at_final: f64,
        /// `Some(MaxFeed)` when the re-derived feed was above the machine's
        /// cutting-feed ceiling and got truncated there, so
        /// `rescaled_mm_per_min` is the ceiling rather than the re-derived
        /// value. Added 2026-08-19 alongside the pass that first produces
        /// this warning: an up-rescale is bounded only by the geometry terms
        /// (worst case ~8.9×), and emitting a feed the machine cannot run
        /// would trade one wrong number for another. Same cap vocabulary the
        /// retired pass 8 used for the same ceiling.
        cap_hit: Option<FeedRecalibrationCap>,
    },
    /// **DECLARED 2026-08-19, NOT YET PRODUCED — G-SUGGEST-NOCLAMP.**
    ///
    /// Reserved for the case where rescaling to the final geometry pushes the
    /// commanded advance *below* the chip-formation floor, so the existing
    /// rubbing-floor rule lifts it back. This is the licensed exception to
    /// the "implied target chipload must agree at both operating points"
    /// invariant: when it fires, the commanded advance sits on the reported
    /// floor rather than on the derated target.
    FeedClampedToChiploadFloor {
        /// Advance per tooth the rescale asked for (mm/tooth), before the floor.
        requested_mm_per_tooth: f64,
        /// The floor that the test used (mm/tooth). See
        /// [`crate::feeds::rubbing_floor`].
        floor_mm_per_tooth: f64,
        /// The bound that set the floor (ruling R4 Q9): the repo constant,
        /// the band minimum, or the maximum of a band with no minimum.
        source: crate::feeds::RubbingFloorSource,
    },
    /// T-15 (2026-09-18): Suggest pass 10 re-evaluated the spindle power
    /// ceiling at the operating point the operation ships, after pass 9 had
    /// re-derived the feed, and the feed did not fit.
    ///
    /// Calculator Step 6 checks the power ceiling at the geometry it was
    /// handed. Pass 9 then re-multiplies the feed by the depth-tier factor at
    /// the FINAL depth, and that factor RISES as the depth falls (1.00 / 0.75
    /// / 0.50 / 0.45 at `ap/D` of 1 / 2 / 3). A clamp that lowers the depth
    /// across a tier boundary therefore raises the feed by up to 2.22×, for a
    /// depth loss that can be arbitrarily small. Nothing re-checked Step 6.
    ///
    /// Pass 10 re-evaluates the canonical model
    /// ([`crate::tool_load::power::PowerTerms`]) at the final `ap`, `ae`, RPM
    /// and feed, against the gate's ceiling `power_at_rpm(rpm)` (the rated
    /// curve, ruling R4 Q2), and lowers the feed onto that ceiling. The clamp is
    /// feed-only and downward: the geometry is final by then.
    ///
    /// `fits_at_any_feed` is `false` when the feed-free EDGE term alone meets
    /// or exceeds the ceiling. No feed rescues that cut — thinning the chip
    /// leaves the ploughing power where it is — so the feed goes back to the
    /// value pass 9 started from and the conflict is reported. Same
    /// clamp-and-warn convention as calculator Step 6 rung 4 and the
    /// rubbing floor at Step 9b.
    ///
    /// Feeds contract P5 deleted its sentry (`..._g_t15`) with the geometry
    /// chain. Register: `planning/TECH_DEBT_REGISTER.md` T-15.
    PowerRecheckedAfterRescale {
        /// `feed_rate` (mm/min) pass 9 wrote, before this pass.
        rescaled_mm_per_min: f64,
        /// `feed_rate` (mm/min) the operation ships after this pass.
        shipped_mm_per_min: f64,
        /// Predicted spindle power (kW) at `rescaled_mm_per_min` and the
        /// final geometry. On the gate's COMMANDED axis, so it compares
        /// directly with `available_kw`.
        required_kw_at_rescaled: f64,
        /// The gate's ceiling (kW): the rated curve `power_at_rpm(rpm)`.
        available_kw: f64,
        /// `false` when the edge term alone is at or over the ceiling, so no
        /// feed satisfies it.
        fits_at_any_feed: bool,
    },
    /// T-12 (2026-09-16): the calculator produced a cut-geometry value that
    /// this operation has no field to hold, so the value was NOT applied.
    ///
    /// `OperationParams::set_depth_per_pass` and `set_stepover` return
    /// `bool` for exactly this purpose — their doc comments say "so the
    /// caller can refuse instead of discarding the value". Before this
    /// variant the funnel discarded it: 10 of the 24 operation configs
    /// implement `depth_per_pass`, so for the other 14 the calculator's
    /// axial depth evaporated and the apply path still reported success.
    ///
    /// That was harmless while nothing depended on the axial value. The
    /// derate work makes it load-bearing: a power-limited cut that cannot
    /// be fixed by speed needs a shallower pass, and on an operation that
    /// cannot hold one the user must be told that the fix was not applied
    /// rather than shown a warning with a silent non-fix behind it. See
    /// `planning/TECH_DEBT_REGISTER.md` T-12.
    ///
    /// Warning-only, and it mutates nothing — the sibling of
    /// [`Self::StrategyRecommendedNotApplied`], which reports the same
    /// shape for a strategy field.
    ///
    /// For several of those 14 the absence is correct rather than a gap:
    /// a V-carve's depth is `distance / tan(half_angle)` at every point, a
    /// chamfer's depth IS the chamfer width the user asked for, and the
    /// surface-finishing ops take their depth from the model surface. The
    /// warning states what was not applied; it does not claim the field
    /// ought to exist.
    CutGeometryFieldNotHeld {
        /// Snake-case field name: `"depth_per_pass"` or `"stepover"`.
        param_name: &'static str,
        /// The operation that refused it, for the rationale line.
        op_kind: &'static str,
        /// The value the calculator produced and the operation discarded.
        recommended_mm: f64,
    },
    /// Ruling R4 Q10 (2026-09-24): the calculator's Step 7 lowered the RPM
    /// to hold the chipload at the machine feed ceiling
    /// ([`crate::feeds::FeedsWarning::RpmLoweredForFeedCeiling`]). This is
    /// the rationale row on the RPM entry; the diagnostic comes from the
    /// calculator's warning, so it is filed once.
    RpmLoweredForFeedCeiling {
        /// The RPM before the descent.
        rpm_from: f64,
        /// The RPM that ships.
        rpm_to: f64,
        /// The machine cutting-feed ceiling (mm/min).
        feed_ceiling_mm_min: f64,
        /// The lowest RPM the descent may reach.
        rpm_floor: f64,
        /// What set `rpm_floor`.
        floor_source: crate::feeds::RpmFloorSource,
        /// `true` when the chipload is held in full.
        held: bool,
        /// Q1 (P8): the two arms at the cap and the one that ships.
        arms: crate::feeds::CapArms,
    },
    /// G6 ramp (2026-09-25): the entry feed Suggest wrote. One record per
    /// apply that wrote the speeds, on an operation with the field. The
    /// record carries the number, the arm that set it and θ, or the reason
    /// for the plunge-rate fallback.
    RampFeed {
        /// The operation's `ramp_feed_rate` before the write.
        from_mm_min: Option<f64>,
        /// What Suggest wrote, and why.
        record: crate::feeds::RampFeed,
        /// The entry rules of the operation that ships (G10 Q6-Q10, Q12):
        /// the angle, the helix, the pip or the core, the clearance, and the
        /// straight-plunge caution. Each note is one card line.
        notes: crate::feeds::ramp::EntryNotes,
    },
    /// CONTRACT K2 (feeds contract P5, lead decision D4): the apply wrote a
    /// field whose basis reads an Unsourced rule (F201), and the apply is
    /// the operator's acceptance of it. `rows` names each rule. The record
    /// is the acceptance; the card shows the basis before the click.
    UnsourcedRowsAccepted {
        /// The F201 names of the Unsourced rules.
        rows: Vec<String>,
    },
    /// P13c (ruling O3): the op's width is free and computed (a
    /// Pocket-family roughing op: the envelope proposal is the
    /// removal-rate pick). No limit models chip evacuation or tool life,
    /// so the card, MCP and the CLI state both assumptions
    /// (`force_budget::COMPUTED_WIDTH_ASSUMPTIONS`, one producer).
    ComputedWidthAssumptions,
    /// CONTRACT K2b (ruling Q6, K6 `project.outside_envelope`): the picked
    /// or the stated point sits outside the safe envelope. A warning; the
    /// write goes ahead.
    OutsideEnvelope {
        /// The limit that the point crosses.
        binding: crate::feeds::envelope::LimitKind,
        /// The negative share of the limit (`-0.2`: 20 % past it).
        margin: f64,
        /// The depth of the point (mm).
        ap_mm: f64,
        /// The width of the point (mm).
        ae_mm: f64,
    },
    /// G10 (2026-09-25): the funnel ran the plunge rule again at the feed
    /// that ships. Pass 9, pass 10 or an explored feed moved the feed after
    /// the calculator, so the plunge moved too. Filed only when the plunge
    /// moved by 1 mm/min or more.
    PlungeReDerived {
        /// The plunge (mm/min) before the funnel ran the rule again.
        from_mm_min: f64,
        /// The plunge (mm/min) that ships.
        to_mm_min: f64,
        /// The term that sets the plunge that ships.
        binding: crate::feeds::plunge::PlungeBinding,
    },
}

/// Reason the v2 step 2 feed-up recalibration terminated early
/// (i.e. before predicted observed chipload reached the LUT lower
/// bound). [`SuggestWarning::FeedRescaledToFinalGeometry`] carries it.
///
/// v2.1 (2026-06-04) collapsed the iterative feed-up into a single
/// closed-form solve, so the only caps that can fire are the
/// physical-feed ceiling and the deflection-budget refusal. A former
/// `MaxIterations` variant was removed at the same revision — no
/// out-of-crate consumers depended on it (grep verified).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedRecalibrationCap {
    /// Solving for the LUT-min target feed produced a value above
    /// `machine.max_feed_mm_min`. The op was clamped to the machine
    /// cap; operator's lever is to drop RPM (which raises chipload
    /// at constant feed) or accept the under-fed recipe.
    MaxFeed,
    /// The target feed would push the closed-form deflection
    /// predictor above `200 µm - 10 µm headroom`. The recalibration
    /// refuses to re-exceed the same gate v1.1 just settled and
    /// reverts to the pre-recal feed. Currently unreachable on the
    /// chipload-independent deflection model, but kept defensive for
    /// a future chipload-in-force predictor.
    DeflectionThreshold,
}

/// Canonical suggestion result: a fully-populated operation plus the calculator
/// output that produced it.
#[derive(Debug, Clone)]
pub struct SuggestedParams {
    pub operation: OperationConfig,
    pub feeds_result: FeedsResult,
    pub warnings: Vec<SuggestWarning>,
    /// Per-field provenance of the suggested values (W2.1). Install alongside
    /// `operation` into the target `ToolpathConfig::feeds_provenance`.
    pub provenance: crate::feeds::FeedsProvenance,
}

/// Input for [`suggest_params`].
#[derive(Clone, Copy)]
pub struct SuggestParamsInput<'a> {
    pub op_type: OperationType,
    pub tool: &'a ToolConfig,
    pub machine: &'a MachineProfile,
    pub material: &'a Material,
    pub lut: &'a VendorLut,
    pub stock_ctx: &'a StockContext,
    /// Spindle-RPM policy. See [`crate::feeds::SpindleStrategy`].
    /// Defaults to `MatchChart` if the caller doesn't care about the
    /// distinction. `ConstantChip` holds the chip and raises the RPM to the
    /// first named limit ([`crate::feeds::RpmLimit`]).
    pub spindle_strategy: crate::feeds::SpindleStrategy,
    /// Project-level context (model bbox, upstream leftover, etc.) used
    /// by gate-aware Suggest paths. See [`SuggestContext`]. Pass
    /// `SuggestContext::default()` when the caller has nothing to
    /// contribute — v1.1 ignores every field.
    pub context: SuggestContext<'a>,
}

/// Input for [`suggest_for_operation`].
#[derive(Clone, Copy)]
pub struct SuggestForOperationInput<'a> {
    pub operation: &'a OperationConfig,
    pub tool: &'a ToolConfig,
    pub machine: &'a MachineProfile,
    pub material: &'a Material,
    pub lut: &'a VendorLut,
    /// Spindle-RPM policy. See [`crate::feeds::SpindleStrategy`].
    pub spindle_strategy: crate::feeds::SpindleStrategy,
    /// Project-level context (model bbox, upstream leftover, etc.) used
    /// by gate-aware Suggest paths. See [`SuggestContext`]. Pass
    /// `SuggestContext::default()` when the caller has nothing to
    /// contribute — v1.1 ignores every field.
    pub context: SuggestContext<'a>,
}

/// Construct a default operation for `op_type`, apply stock-aware defaults, run
/// the feeds calculator, write recommendations into the operation, and enforce
/// cross-field invariants before returning.
///
/// Returns `Err(FeedsError)` when the tool × operation combination is
/// physically unrunnable (e.g. flat endmill assigned to a Scallop
/// op). Callers that want the legacy "always produce something"
/// behaviour should `.unwrap_or_else(|_| _)` or fall back to a default,
/// but the GUI / CLI / MCP Suggest buttons should surface the refusal
/// to the user instead of writing a meaningless recipe into the op.
pub fn suggest_params(input: SuggestParamsInput<'_>) -> Result<SuggestedParams, FeedsError> {
    // P10 (ruling D8): the add door writes the tool's geometry rule values
    // first, so Suggest writes the speeds for that geometry.
    let (operation, rule_stamps) = default_operation_for_tool(
        input.op_type,
        input.stock_ctx,
        crate::compute::geometry_default::RuleContext {
            tool: input.tool,
            material: input.material,
            machine: input.machine,
        },
    );
    // G6 ramp: an add door builds `DressupConfig::for_op(op_type)` after
    // this call, so the entry θ it will ship is known here.
    let default_dressups = crate::compute::config::DressupConfig::for_op(input.op_type);
    let mut suggested = suggest_for_operation(SuggestForOperationInput {
        operation: &operation,
        tool: input.tool,
        machine: input.machine,
        material: input.material,
        lut: input.lut,
        spindle_strategy: input.spindle_strategy,
        context: SuggestContext {
            dressups: input.context.dressups.or(Some(&default_dressups)),
            ..input.context
        },
    })?;
    merge_rule_stamps(&mut suggested.provenance, &rule_stamps);
    Ok(suggested)
}

/// P10 (ruling D8): [`default_operation`] with the geometry rule values of
/// the tool of `ctx` (`compute::geometry_default`), and the stamp of each.
#[must_use]
pub fn default_operation_for_tool(
    op_type: OperationType,
    stock_ctx: &StockContext,
    ctx: crate::compute::geometry_default::RuleContext<'_>,
) -> (OperationConfig, crate::feeds::FeedsProvenance) {
    let mut operation = default_operation(op_type, stock_ctx);
    let mut stamps = crate::feeds::FeedsProvenance::default();
    let _ = crate::compute::geometry_default::write_rule_values(
        &mut operation,
        &mut stamps,
        ctx,
        crate::compute::geometry_default::Keep::Nothing,
    );
    (operation, stamps)
}

/// P10: copy each geometry stamp of `rule` into `into` where `into` has no
/// stamp (the speeds-only funnel stamps no geometry).
pub fn merge_rule_stamps(
    into: &mut crate::feeds::FeedsProvenance,
    rule: &crate::feeds::FeedsProvenance,
) {
    use crate::feeds::FeedsField;
    for slot in [
        FeedsField::Stepover,
        FeedsField::DepthPerPass,
        FeedsField::PeckDepth,
        FeedsField::ZStep,
    ] {
        if into.get(slot).is_none()
            && let Some(stamp) = rule.get(slot)
        {
            into.set(slot, stamp.clone());
        }
    }
}

/// The operation `suggest_params` starts from: the registry default with
/// the stock defaults applied, and no feeds recipe. An add door falls back
/// to it when Suggest refuses with [`FeedsError::Unbacked`] (ruling R1): the
/// operation is still created, the recipe is not.
#[must_use]
pub fn default_operation(op_type: OperationType, stock_ctx: &StockContext) -> OperationConfig {
    let mut operation = OperationConfig::new_default(op_type);
    apply_stock_defaults(&mut operation, stock_ctx);
    operation
}

/// Run the canonical suggestion path for an existing operation. Operation fields
/// that act as feed-calculator hints (for example scallop height) are read from
/// `operation`; suggested feed/plunge/stepover/depth are written into a clone.
///
/// Returns `Err(FeedsError)` for physically-unrunnable
/// tool × operation combinations — see [`suggest_params`] for the
/// rationale.
pub fn suggest_for_operation(
    input: SuggestForOperationInput<'_>,
) -> Result<SuggestedParams, FeedsError> {
    let feeds_result = feeds_result_for_operation(
        input.operation,
        input.tool,
        input.material,
        input.machine,
        input.lut,
        input.spindle_strategy,
    )?;
    let mut operation = input.operation.clone();
    let mut provenance = crate::feeds::FeedsProvenance::default();
    // F042: the one write, `apply`. K2b (ruling Q4, lead decision D1): a
    // registry default at the add door is a stated value, so the door
    // writes the speeds for the geometry the op keeps and no geometry.
    // `feeds_result_for_operation` validated the pairing above, which is
    // what `validated` requires. D4: the door accepts the Unsourced rows
    // and records the warning that names them.
    let (rec, accepted) = Recommendation::validated(&feeds_result).accept_unsourced();
    let mut warnings = apply(
        &rec,
        &mut operation,
        &mut provenance,
        ApplyContext {
            tool: input.tool,
            machine: input.machine,
            material: input.material,
            pass_role: input.operation.feeds_style().1,
            suggest: input.context,
        },
    );
    warnings.extend(accepted);
    if has_computed_width(input.operation) {
        warnings.push(SuggestWarning::ComputedWidthAssumptions);
    }
    Ok(SuggestedParams {
        operation,
        feeds_result,
        warnings,
        provenance,
    })
}

/// P13c (O3): true for an op whose width is free and computed: a
/// Pocket-family roughing op that cuts a stepover (the envelope's
/// corner-coverage rule and removal-rate pick). P29-3: the offset-ring
/// fact of the op's body cut, the fact of the envelope's `computed_width`.
#[must_use]
pub fn has_computed_width(operation: &OperationConfig) -> bool {
    // EXEMPT(P28-05, P28-06, P28-07)
    operation.cuts().body_facts().offset_ring
}

/// Run the feeds calculator for an operation and project context without
/// mutating the operation.
///
/// Returns `Err(FeedsError)` when the tool × operation pairing is
/// physically unrunnable — see [`crate::feeds::validate_tool_for_operation`]
/// for the predicate. Existing callers that just want the numbers can
/// `.unwrap_or_else(|_| FeedsResult::default())`; the GUI Suggest path
/// (properties/feeds modal) should propagate the refusal so the user
/// sees why the recipe was withheld.
pub fn feeds_result_for_operation(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    lut: &VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
) -> Result<FeedsResult, FeedsError> {
    // F029: the one Suggest door refuses a tool outside its domain, so no
    // pass below meets a tool that builds no cutter.
    tool.validate().map_err(FeedsError::ToolOutOfDomain)?;
    let input = FeedsInput::of_operation(operation, tool, material, machine, lut, spindle_strategy);
    crate::feeds::validate_tool_for_operation(&input)?;
    Ok(crate::feeds::calculate(&input))
}

/// Same inputs as [`feeds_result_for_operation`] but returns the full
/// [`FeedsExplain`](crate::feeds::explain_payload::FeedsExplain) payload — recommended values plus matched LUT row,
/// sibling rows, and machine envelope. Used by the redesigned Feeds &
/// Speeds modal.
pub fn feeds_explain_for_operation(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    lut: &VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
) -> crate::feeds::FeedsExplain {
    let input = FeedsInput::of_operation(operation, tool, material, machine, lut, spindle_strategy);
    crate::feeds::explain_feeds(&input)
}

/// Round a suggestion value to the nearest multiple of `step`.
///
/// Used by the Suggest path to produce UI-friendly numbers before
/// clamping (e.g. round chipload to 0.005 mm, RPM to 100). When
/// `step <= 0.0` the value is returned unchanged.
///
/// **Which of the two to call.** A value with no upper bound at the point
/// of the call rounds to the NEAREST multiple, and takes this function. A
/// value a clamp already bound from ABOVE takes
/// [`round_suggestion_value_down`], because the nearest multiple is above
/// the limit about half the time.
pub fn round_suggestion_value(value: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value;
    }
    (value / step).round() * step
}

/// Round a suggestion value DOWN to a multiple of `step`.
///
/// [`round_suggestion_value`] snaps to the NEAREST multiple, which goes UP
/// about half the time. That is right for a value the calculator left free.
/// It is wrong for a value a ceiling already fixed: the shipped number then
/// sits one part-step ABOVE the limit the clamp exists to enforce. The
/// commanded feed is such a value — the Step 6 power gate and the Step 7
/// machine ceiling both land the recommendation exactly on a limit, and
/// nothing re-checks a limit after the quantisation. See T-9 in
/// `planning/TECH_DEBT_REGISTER.md`.
///
/// `MachineProfile::next_rpm_at_or_below` is the same fix on the RPM axis.
///
/// Returns `value` unchanged when `step <= 0.0`.
#[must_use]
pub fn round_suggestion_value_down(value: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value;
    }
    (value / step).floor() * step
}

#[cfg(test)]
mod tests;
