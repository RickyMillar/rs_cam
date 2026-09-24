//! Canonical feed/speed suggestion plumbing.
//!
//! This module owns the bridge from an [`OperationConfig`] + project context into
//! the feeds calculator, and the write-back path from a [`FeedsResult`] into an
//! operation. GUI, MCP, and diagnostics call through here so recommendations and
//! pre-sim warning baselines stay in lock-step.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{FeedsError, FeedsInput, FeedsResult, SetupContext, VendorLut};
use crate::machine::MachineProfile;
use crate::material::Material;

mod adaptive_entry;
mod aggressiveness;
mod apply;
mod axial_envelope;
mod invariants;

pub use apply::{
    ApplicableRecommendation, ApplyContext, ApplyScope, FeedsPreview, FieldApplyPreview,
    FieldApplyPreviews, apply, apply_cut_geometry_to_op, apply_drill_defaults,
    apply_feeds_result_to_op, apply_speeds_to_op, apply_stock_defaults,
    feeds_preview_for_operation, operation_feeds_hints, preview_field_applies, preview_field_apply,
    resolve_operation_invariants,
};
pub(crate) use axial_envelope::axial_envelope_for_operation;
// The deflection back-off target, re-exported so `feeds::rationale` can
// format the bound it renders from the constant the loop compares
// against. The text cannot then drift from the number.
pub(crate) use invariants::DEFLECTION_BACKOFF_TARGET_UM;

/// Stock-derived values used by stock-aware operation defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StockContext {
    /// Top of the stock in part-Z coordinates.
    pub stock_top_z: f64,
    /// Bottom of the stock in part-Z coordinates.
    pub stock_bottom_z: f64,
    /// Stock thickness in mm.
    pub stock_z: f64,
    /// Stock-top minus model-top margin in mm.
    pub stock_padding: f64,
}

impl StockContext {
    /// Build a stock context from a stock bbox and padding value.
    pub fn from_stock_bbox(bbox: crate::geo::BoundingBox3, padding: f64) -> Self {
        let stock_z = (bbox.max.z - bbox.min.z).max(0.0);
        Self {
            stock_top_z: bbox.max.z,
            stock_bottom_z: bbox.min.z,
            stock_z,
            stock_padding: padding,
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
    /// Above + strategy-field auto-rewrite when the field equals its
    /// `Default::default()` value (treated as unpinned). User-set
    /// non-default values are left alone. v3 default per the
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
/// holds the chipload at the band midpoint times the depth ladder; the one
/// meaning of "aggressiveness" is now `MachineProfile::aggressiveness`.
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
    /// [`enforce_invariants`]. `apply_feeds_result_to_op` lifts this
    /// off `FeedsResult` automatically; direct callers of
    /// `enforce_invariants` (tests synthesizing post-v1.1 operating
    /// points) populate it explicitly. `None` means "no constraint
    /// signal" — the recalibration short-circuits.
    pub chipload_bounds: Option<crate::feeds::ChiploadBounds>,
    /// Matched LUT row (post-scaling) — populated by `apply_feeds_result_to_op`
    /// from `FeedsResult::matched_lut_row`. Consumed by `pick_axial_envelope`
    /// to derive the vendor ap bound. `None` when the calculator did not
    /// match any vendor row.
    pub matched_lut_row: Option<&'a crate::feeds::vendor_lookup::LookupResult>,
    /// Effective cutter diameter at the calculator's commanded axial DOC
    /// (mm). Used by the chipload-bounds re-derivation step in
    /// `enforce_invariants` when the axial-envelope pass mutates DPP —
    /// the new `doc_derating_scale(new_dpp / effective_d)` must be
    /// applied to keep `chipload_bounds` consistent with the post-mutation
    /// operating point.
    ///
    /// **Sentinel contract (C2, 2026-07-30) — DOCUMENTED AND TESTED, not
    /// converted.** `0.0` means *not populated* (no calculator result fed
    /// this context), and the single consumer,
    /// [`recompute_chipload_bounds_for_dpp`], treats a non-positive diameter
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
    /// [`rescale_feed_to_final_geometry`].
    ///
    /// `None` means **there is no derivation point to reconcile against**,
    /// and the rescale pass short-circuits. That is the correct answer for
    /// [`resolve_operation_invariants`], whose whole contract is "run the
    /// safety clamps over a value a human or the optimizer chose, and do
    /// not substitute a number of our own" — a hand-typed feed was never
    /// derived from a chip-thinning term, so there is nothing to re-derive.
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
/// `enforce_invariants` may legally overwrite the operation's stepover and
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

/// Warnings emitted while applying suggestions to an operation.
#[derive(Debug, Clone, PartialEq)]
pub enum SuggestWarning {
    PlungeClampedToFeed {
        requested: f64,
        capped: f64,
    },
    StepoverClampedToToolDiameter {
        requested: f64,
        capped: f64,
    },
    RoughingDepthClampedToRigidity {
        requested: f64,
        capped: f64,
    },
    DepthClampedToCuttingLength {
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
    /// v1.1 combined-Suggest step 2: the closed-form deflection
    /// predictor (see [`crate::feeds::predict::predict_peak_deflection_um`])
    /// projected that the post-rigidity-clamp DPP would drive the
    /// cantilever tip deflection past the 200 µm critical threshold
    /// (the same gate `tool_load::deflection` evaluates post-sim).
    /// `enforce_invariants` iteratively backed DPP off by 20% per
    /// iteration (max 5 iterations, floor 0.5 mm) until the predicted
    /// deflection cleared the threshold or the loop hit the floor.
    ///
    /// `predicted_um_at_capped` may still exceed 200 µm if the loop
    /// bailed on the floor — that's the "unmet gate" signal until
    /// the v2 surface adds a structured unmet-condition channel.
    /// Wanaka Back Rough motivating case: 9 mm DPP on a 6 mm carbide
    /// endmill in HardMaple shipped 358 µm post-sim; back-off settles
    /// to a DPP that clears the threshold.
    DppCappedByDeflection {
        requested_mm: f64,
        capped_mm: f64,
        predicted_um_at_requested: f64,
        predicted_um_at_capped: f64,
        iterations: u8,
    },
    /// T-4 (2026-09-18): the closed-form deflection predictor abstained,
    /// so the DPP back-off did not run. Warning-only — Suggest keeps the
    /// DPP the rigidity clamp and the axial envelope produced.
    ///
    /// The operator needs the distinction. Until T-4 the predictor
    /// reported every refusal as `0.0 µm`, the loop's `> target` test
    /// short-circuited, and nothing was said. A pass-through and a
    /// clearance were the same event. The sharp case is an unvalidated
    /// plastic on a roughing operation: nine of the ten shipped plastics
    /// carry no measured `Kc`, the stock picker names them all, and the
    /// operator had no way to know which ones the model covers.
    DeflectionBackoffUnmodeled {
        /// The DPP the operation carried when the predictor abstained.
        dpp_mm: f64,
        /// Which abstention. `reason.clause()` is the operator-facing
        /// wording and `reason.as_unmodeled_reason()` is the same
        /// vocabulary the post-simulation gate prints.
        reason: crate::feeds::predict::DeflectionUnmodeled,
    },
    /// T-4 (2026-09-18): the deflection figure the back-off ran on is a
    /// modelled FLOOR, not a bound.
    ///
    /// The back-off still runs, because backing a DPP off a floor is
    /// conservative. It does not show the cut safe. A V-bit is the one
    /// producer today: the integrator models its cone but not its flute
    /// relief, and that gap runs in the UNSAFE direction.
    DeflectionBackoffFigureIsAFloor {
        /// The DPP the operation carried at the figure below.
        dpp_mm: f64,
        /// The modelled floor (µm) at `dpp_mm`. The real deflection is
        /// larger.
        predicted_um: f64,
        /// Which part of the model is missing. `caveat.clause()` is the
        /// operator-facing wording.
        caveat: crate::feeds::predict::DeflectionCaveat,
    },
    /// v1.2 combined-Suggest: the closed-form move-count predictor (see
    /// [`crate::feeds::predict::predict_move_count`]) projected that the
    /// just-written stepover would emit more than
    /// [`STEPOVER_BACKOFF_TARGET_MOVES`] motion samples on the operation's
    /// model envelope. `enforce_invariants` iteratively raised the
    /// stepover by 50% per iteration (max 5 iterations, ceiling
    /// `tool.diameter × `[`STEPOVER_BACKOFF_DIAMETER_FRACTION`]) until the
    /// predicted move count cleared the threshold or the loop hit the
    /// ceiling. The Wanaka 3D Finish 6 motivating case (2 mm tip
    /// tapered-ball on a 140 × 150 mm stock with a `scallop_height`
    /// target that resolves to 0.03 mm stepover, 4.6 M predicted moves)
    /// settles to a usable stepover via this back-off.
    ///
    /// `predicted_moves_at_raised` may still exceed
    /// [`STEPOVER_BACKOFF_TARGET_MOVES`] if the loop bailed on the
    /// diameter-fraction ceiling — that's the "unmet gate" signal until
    /// the v2 surface adds a structured unmet-condition channel.
    /// Scallop-height-targeted ops that get raised past the scallop
    /// floor are not explicitly flagged on this variant; the warning
    /// itself signals the trade-off ("we raised stepover for runtime
    /// reasons, which probably degrades scallop").
    StepoverRaisedForRuntime {
        requested_mm: f64,
        raised_mm: f64,
        predicted_moves_at_requested: u64,
        predicted_moves_at_raised: u64,
        iterations: u8,
    },
    /// **RESERVED, not constructed by Suggest (since 2026-08-13).**
    ///
    /// This variant used to report Suggest pass 8's arc-fit chipload feed
    /// recalibration. Checkpoint J-1/J-5 retired that pass — Suggest emits
    /// the un-lifted feed the calculator produced — and **kept this variant
    /// deliberately** so the *simulation-backed* path (`feed_modulation`,
    /// `tool_load::optimize::retarget::chipload`) can report its own
    /// **measured** feed changes through a vocabulary the rationale
    /// renderer, the feeds modal and the MCP rationale endpoint already
    /// speak. Nothing in `feeds::suggest` pushes it today; the renderer
    /// arms and their round-trip tests remain live.
    ///
    /// The field semantics below are written for that reuse: they describe
    /// a feed that WAS raised and the observation it was raised against.
    /// A producer must supply a **measured** observation — the pre-sim
    /// prediction those fields were originally fed from was itself the
    /// defect (it estimated a quantity the gate stopped reporting on
    /// 2026-08-06).
    ///
    /// Historical note, kept because it is the case the retired pass was
    /// built for: Wanaka post-sim 2026-06-03 showed three toolpaths at
    /// observed median chipload 0.0067–0.011 mm/tooth vs LUT minima
    /// 0.027–0.076 — a rubbing/burning recipe. That statement is in the
    /// **deleted arc-mean quantity**; in the unit the gate now reports,
    /// A-5 measured the same operating points already inside the band.
    FeedRaisedForChipload {
        /// Pre-recalibration `feed_rate` (mm/min) — the value written
        /// by the calculator and any earlier invariants.
        requested_mm_per_min: f64,
        /// Post-recalibration `feed_rate` (mm/min) — what the loop
        /// settled on after iterating against the chipload + deflection
        /// + feed-cap constraints.
        raised_mm_per_min: f64,
        /// Forward-predicted observed median chipload before the loop
        /// (mm/tooth). Should be below `lut_target_mm_per_tooth` for
        /// the loop to fire at all.
        predicted_observed_chipload_before: f64,
        /// Forward-predicted observed median chipload at the
        /// terminating feed (mm/tooth). When `cap_hit` is `None` this
        /// is `>= lut_target_mm_per_tooth`; when a cap binds, this is
        /// the best the loop could achieve under the constraint.
        predicted_observed_chipload_after: f64,
        /// The target inside the LUT band (mm/tooth), from
        /// [`FeedsResult::chipload_bounds`].
        lut_target_mm_per_tooth: f64,
        /// `Some(_)` when the loop terminated on a constraint rather
        /// than reaching the LUT minimum. See [`FeedRecalibrationCap`]
        /// for the three modes.
        cap_hit: Option<FeedRecalibrationCap>,
    },
    /// **RESERVED, not constructed by Suggest (since 2026-08-13).**
    /// Retired and kept alongside [`SuggestWarning::FeedRaisedForChipload`]
    /// under Checkpoint J-5 — same reasoning, same intended reuse by the
    /// simulation-backed path.
    ///
    /// Semantics for a future producer: paired with `FeedRaisedForChipload`
    /// when a binding cap stopped the correction before the observation
    /// reached the band. The deflection budget is too tight for the
    /// configured (tool, material) combination — the
    /// operator's options are to drop RPM (which raises chipload at
    /// constant feed) or accept the rubbing-floor recipe with the
    /// understanding that edge heating may shorten tool life.
    ChiploadStillLowAfterRecalibration {
        /// Best predicted observed median chipload the loop achieved
        /// (mm/tooth), still below `lut_target_mm_per_tooth`.
        predicted_observed_mm_per_tooth: f64,
        /// The target inside the LUT band the loop was aiming for
        /// (mm/tooth).
        lut_target_mm_per_tooth: f64,
        /// `feed_rate` at loop termination (mm/min) — what got written
        /// into the operation.
        feed_at_termination_mm_per_min: f64,
        /// The binding constraint that stopped the loop.
        blocking_cap: FeedRecalibrationCap,
    },
    /// v3.3b (2026-06-04): the strategy-aware orchestrator rewrote a
    /// strategy field (entry_style, clearing_strategy, or one of the
    /// stock-to-leave dimensions) because the field was at its
    /// `Default::default()` value (treated as unpinned) and an
    /// orchestrator-side heuristic prefers a different choice.
    ///
    /// Wanaka motivating case: Back Rough's `entry_style = Plunge` at
    /// the post-back-off DPP of 3.69 mm on a 6 mm tool (ratio 0.62)
    /// produces a 362 µm entry transient — above the 200 µm deflection
    /// gate. The `pick_adaptive3d_entry_style` pass rewrites Plunge →
    /// Ramp when DPP / D > 0.5 and the field equals `Default`. v3.4
    /// will additionally promote to Helix when the geometry classifier
    /// reports interior pocket headroom.
    ///
    /// `param` is the snake-case field name — "entry_style" and
    /// "clearing_strategy" are the two values shipped today. `from` and
    /// `to` are the value labels (e.g. "plunge", "ramp"). `reason` is
    /// the short heuristic key that fired (e.g. "deflection_predict_at_dpp").
    StrategyRewrote {
        param: &'static str,
        from: String,
        to: String,
        reason: &'static str,
    },
    /// v3.3c (2026-06-05): warn-only sibling of [`Self::StrategyRewrote`]
    /// — the geometry classifier prefers a different value for a
    /// strategy field, but the orchestrator does NOT auto-apply it.
    /// Shipped for `clearing_strategy`, where auto-selection is
    /// deferred to v4 until the classifier is calibrated (the
    /// `ContourParallel` → `Adaptive` promotion on mixed terrain needs
    /// uncut-band evidence, not just a bbox heuristic).
    ///
    /// `param` / value-label / `reason` conventions match
    /// `StrategyRewrote`. `current` is left untouched on the operation.
    StrategyRecommendedNotApplied {
        param: &'static str,
        current: String,
        recommended: String,
        reason: &'static str,
    },
    /// Phase 3 / cutter-axial-constraints envelope: the cutter / material /
    /// engagement combination has an empty safe band (chipload-burn floor
    /// exceeds the deflection / vendor / scallop max). Warning-only in
    /// this phase — automatic 3D-finish `stock_to_leave` mutation is
    /// deferred until in-process stock at gen time lands. Mirrors the
    /// planning doc §5.2 `FinishToolMismatched` variant: surface the
    /// mismatch so the operator can switch tool / surface / chipload
    /// before committing.
    AxialEnvelopeSafeBandEmpty {
        /// Cutter / material / op kind so the rationale entry can render
        /// "scallop on 6 mm ball" rather than a bare warning string.
        op_kind: &'static str,
        max_safe_doc_mm: f64,
        chipload_floor_doc_mm: f64,
        /// Which upper bound was tighter — Deflection / VendorAp / Scallop.
        binding_upper: &'static str,
    },
    /// Phase 3 envelope policy-C clamp: the user-set / calculator-derived
    /// axial value sat above the cutter-axial-constraints `safe_max_doc_mm`,
    /// so the envelope clamped it down. Variant carries `param_name` so
    /// downstream renderers know whether `depth_per_pass` (Adaptive3d),
    /// `max_depth` (VCarve), or another axial knob was rewritten.
    AxialDocClampedByEnvelope {
        op_kind: &'static str,
        param_name: &'static str,
        commanded_mm: f64,
        clamped_mm: f64,
        /// `"deflection" | "vendor_ap" | "scallop"`. SafeBandEmpty goes
        /// through `AxialEnvelopeSafeBandEmpty` instead.
        binding: &'static str,
    },
    /// Phase 3 envelope policy-C lower-band warning: the user-set axial
    /// value sat *below* the LUT's chipload-burn floor. Per planning
    /// §5.4 policy C the envelope does NOT clamp upward — operators
    /// running deliberately conservative shallow passes should not have
    /// Suggest drown out their pin — but the warning still surfaces so
    /// the rationale tree can explain the rubbing-risk trade-off.
    AxialDocBelowBurnFloor {
        op_kind: &'static str,
        param_name: &'static str,
        commanded_mm: f64,
        floor_mm: f64,
    },
    /// Phase 3 ProjectCurve / engraving feasibility check: the user's
    /// commanded depth exceeds the cutter-axial-constraints safe envelope.
    /// Warning-only — Suggest does not rewrite engraving depth (the user
    /// chose it intentionally for the artwork).
    ProjectCurveDepthInfeasible {
        commanded_mm: f64,
        max_safe_mm: f64,
        binding: &'static str,
    },
    /// Phase 3 finish-op envelope warning: scallop / drop_cutter /
    /// horizontal_finish / waterline / spiral_finish / radial_finish
    /// landed with a deflection or scallop bound below the operator's
    /// commanded `stock_to_leave` semantics. Warning-only in this phase
    /// (automatic `stock_to_leave` mutation blocked on in-process
    /// stock); rationale tree carries the binding constraint name so
    /// the user can decide whether to drop stepover, soften the
    /// chipload, or switch tool.
    FinishEnvelopeAdvisory {
        op_kind: &'static str,
        max_safe_doc_mm: f64,
        binding: &'static str,
    },
    /// **DECLARED 2026-08-19, NOT YET PRODUCED — G-SUGGEST-NOCLAMP.**
    ///
    /// Reserved for the pass that re-derives the feed once
    /// `enforce_invariants` has settled the operation's final stepover and
    /// DPP. `feeds::calculate` freezes the feed against the geometry it was
    /// handed; later passes (`backoff_stepover_for_runtime`,
    /// `pick_axial_envelope`, the rigidity/deflection clamps) then overwrite
    /// that geometry, leaving the chip-thinning and depth-tier terms derived
    /// at an operating point the operation does not run.
    ///
    /// Measured motivating case (wanaka200 tp 8, `drop_cutter`, R1.5 tapered
    /// ball in White Oak): the calculator sized the feed at `ae = 0.09` — the
    /// `operation_default_profile(Parallel, Finish).ae_factor` fallback,
    /// because no op populates `FeedsHints::radial_width_mm` — while
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
    /// Sentry: `tests/suggest_feed_matches_final_geometry.rs`.
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
    /// Sentry: `tests/a_rescaled_feed_stays_inside_the_power_ceiling_g_t15.rs`.
    /// Register: `planning/TECH_DEBT_REGISTER.md` T-15.
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
    /// Ruling R4 (2026-09-24): Suggest pass 6b, the machine aggressiveness
    /// dial, changed the engagement to hold the load at a fraction of the
    /// load at the base engagement. The chipload did not change.
    ///
    /// The target is `k_eff = aggressiveness × ld_factor` (ruling Q7: the
    /// long-tool share lowers the target, it does not cut the feed). Below
    /// 1.0 the pass makes the depth per pass and the stepover smaller by one
    /// common scale (Q11). Above 1.0 (Q3) it makes them larger, inside the
    /// rigidity depth cap, the flute length and a stepover of one diameter.
    ///
    /// A `*_from` / `*_to` pair is `Some` exactly when that lever exists and
    /// the pass evaluated it. The two load models are the lateral force
    /// (`feeds::force`) and the spindle power (`tool_load::power`). When both
    /// refuse (no primary-source `Kc`), the chip cross-section is the proxy.
    ///
    /// Sentry: `tests/the_dial_holds_the_load_and_never_cuts_the_feed_fm7.rs`.
    EngagementReducedForAggressiveness {
        /// `MachineProfile::aggressiveness`.
        aggressiveness: f64,
        /// The long-tool share of the target, `feeds::long_tool_load_share`.
        ld_factor: f64,
        /// `aggressiveness × ld_factor`, the load fraction the pass aimed at.
        target_share: f64,
        /// The common scale the pass applied to every lever.
        scale: f64,
        /// Depth per pass before and after (mm).
        dpp_from: Option<f64>,
        dpp_to: Option<f64>,
        /// Stepover before and after (mm).
        stepover_from: Option<f64>,
        stepover_to: Option<f64>,
        /// Peak lateral cutting force before and after (N).
        force_n_before: Option<f64>,
        force_n_after: Option<f64>,
        /// Spindle power before and after (kW).
        power_kw_before: Option<f64>,
        power_kw_after: Option<f64>,
        /// The chip cross-section proxy before and after (mm²). `Some` only
        /// when both load models refuse.
        section_mm2_before: Option<f64>,
        section_mm2_after: Option<f64>,
        /// `true` when every modelled load is at or below `target_share` x
        /// its base value (below 1.0), or reached it (above 1.0).
        target_met: bool,
        /// Why the target was not met, when it was not.
        shortfall: Option<AggressivenessShortfall>,
        /// `false` when the apply wrote the speeds only
        /// (`ApplyScope::Speeds`): the pass computed this engagement, and
        /// the operation does not carry it. The card then says "Apply the cut
        /// geometry to hold the load at N %" (spec §2.5).
        applied: bool,
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
    },
    /// Ruling R4 (2026-09-24): the aggressiveness dial did not act on this
    /// operation, and why. The engagement is unchanged. This record exists
    /// so that the card states it (the "no invisible calculations" rule).
    AggressivenessNotApplied {
        /// `MachineProfile::aggressiveness`.
        aggressiveness: f64,
        /// Why the dial did not act.
        reason: AggressivenessSkip,
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
    },
}

/// Why the aggressiveness dial did not act (ruling R4, 2026-09-24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggressivenessSkip {
    /// A Finish pass (ruling Q4). The finish depth is the stock allowance and
    /// the finish width is the scallop target. The deflection check decides.
    FinishRole,
    /// A drill cycle (ruling Q5). A plunge is full-width axial; there is no
    /// engagement lever. The plunge ships the material base with no factor.
    Drill,
}

impl AggressivenessSkip {
    /// The card text, one sentence. Every surface prints this.
    #[must_use]
    pub const fn card_text(self) -> &'static str {
        match self {
            Self::FinishRole => "Finish: no dial action; deflection decides.",
            Self::Drill => "Plunge: material base, no factor; the dial does not act.",
        }
    }
}

/// Why the aggressiveness dial did not meet its load target (ruling R4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggressivenessShortfall {
    /// Below 1.0: every lever reached its floor (0.05 mm depth, 0.02 mm
    /// stepover) and the load is still above the target. The feed is not
    /// cut to close the gap.
    LeverFloor,
    /// Above 1.0: every lever reached its cap (the rigidity depth cap, the
    /// flute length, a stepover of one diameter, a deflection back-off
    /// depth, or the rated spindle power) before the load reached the
    /// target.
    EngagementCap,
    /// The operation has no depth-per-pass field and no free stepover (a
    /// scallop target set it), so the dial has no lever.
    NoLever,
}

/// Reason the v2 step 2 feed-up recalibration terminated early
/// (i.e. before predicted observed chipload reached the LUT lower
/// bound). Paired with [`SuggestWarning::FeedRaisedForChipload`] /
/// [`SuggestWarning::ChiploadStillLowAfterRecalibration`].
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
    /// distinction. `MaxSpeed` walks the constant-chipload line up to
    /// the spindle ceiling, capped by vendor.rpm_max when published.
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
    let operation = default_operation(input.op_type, input.stock_ctx);
    // G6 ramp: an add door builds `DressupConfig::for_op(op_type)` after
    // this call, so the entry θ it will ship is known here.
    let default_dressups = crate::compute::config::DressupConfig::for_op(input.op_type);
    suggest_for_operation(SuggestForOperationInput {
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
    })
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
    let warnings = apply_feeds_result_to_op(
        &mut operation,
        &mut provenance,
        &feeds_result,
        input.tool,
        input.machine,
        input.material,
        input.operation.feeds_style().1,
        input.context,
    );
    apply_drill_defaults(
        &mut operation,
        input.tool,
        input.material,
        input.context.stock,
    );
    Ok(SuggestedParams {
        operation,
        feeds_result,
        warnings,
        provenance,
    })
}

/// Build a [`FeedsInput`] for the given operation. Shared by both
/// [`feeds_result_for_operation`] and [`feeds_explain_for_operation`]
/// so the recommendation and the explanation are always derived from
/// identical inputs.
///
/// Public so that a test or an instrument can resolve
/// [`crate::feeds::feeds_support`] on the input a production door builds,
/// not on a second builder.
#[must_use]
pub fn feeds_input_for_operation<'a>(
    operation: &OperationConfig,
    tool: &'a ToolConfig,
    material: &'a Material,
    machine: &'a MachineProfile,
    lut: &'a VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
) -> FeedsInput<'a> {
    let (family, role) = operation.feeds_style();
    let (axial_hint, radial_hint, scallop_hint) = operation_feeds_hints(operation);
    let tool_def = build_cutter(tool);
    FeedsInput {
        tool_diameter: tool.diameter,
        flute_count: tool.flute_count,
        flute_length: tool.cutting_length,
        shank_diameter: Some(tool.shank_diameter),
        tool_geometry: tool_def.to_geometry_hint(),
        material,
        machine,
        operation: family,
        // Checkpoint K (a4) — the operation's own kind, so Suggest and
        // the gate route the LUT query through the same
        // `vendor_normalize::lut_query_for`. This is the single
        // production Suggest/Explain input builder, so supplying it here
        // is what closes the two-sided routing.
        operation_kind: Some(operation.op_type()),
        pass_role: role,
        axial_depth_mm: axial_hint,
        radial_width_mm: radial_hint,
        target_scallop_mm: scallop_hint,
        vendor_lut: Some(lut),
        setup: SetupContext {
            tool_overhang_mm: Some(tool.stickout),
        },
        spindle_strategy,
    }
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
    let input =
        feeds_input_for_operation(operation, tool, material, machine, lut, spindle_strategy);
    crate::feeds::validate_tool_for_operation(&input)?;
    Ok(crate::feeds::calculate(&input))
}

/// Same inputs as [`feeds_result_for_operation`] but returns the full
/// [`FeedsExplain`] payload — recommended values plus matched LUT row,
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
    let input =
        feeds_input_for_operation(operation, tool, material, machine, lut, spindle_strategy);
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
