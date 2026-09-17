//! Canonical feed/speed suggestion plumbing.
//!
//! This module owns the bridge from an [`OperationConfig`] + project context into
//! the feeds calculator, and the write-back path from a [`FeedsResult`] into an
//! operation. GUI, MCP, and diagnostics call through here so recommendations and
//! pre-sim warning baselines stay in lock-step.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{
    FeedsError, FeedsInput, FeedsResult, OperationFamily as FeedsOperationFamily, PassRole,
    SetupContext, ToolGeometryHint, VendorLut, WorkholdingRigidity,
};
use crate::machine::MachineProfile;
use crate::material::Material;

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

/// v3.0b (2026-06-04): where inside the LUT chipload band the
/// feed-up recalibration aims. Used by
/// [`recalibrate_feed_for_chipload`] via [`SuggestPolicy`] on
/// [`SuggestContext`].
///
/// Default (v3.0c, 2026-06-04) is `Default` (LUT band midpoint) per the
/// 2026-06-03 directive. Conservative preserves the v2.1 / pre-v3 recipe
/// (target = LUT band lower bound).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SuggestAggressiveness {
    /// Aim observed median chipload at LUT band minimum. v2.1
    /// behaviour. Use when material variability is high, the tool
    /// is new, or workholding is suspect.
    Conservative,
    /// Aim observed median chipload at LUT band midpoint
    /// (`(min + max) / 2`). v3 default per directive 2026-06-03.
    #[default]
    Default,
    /// Aim observed median chipload at LUT band maximum. Gated by
    /// the deflection refusal (200 µm − 10 µm headroom) and
    /// `machine.max_feed_mm_min`. Equivalent to the pre-v2
    /// "push for speed" recipe.
    Speed,
}

impl SuggestAggressiveness {
    /// Resolve the chipload target this aggressiveness level aims
    /// for inside the given LUT band.
    pub fn target_chipload(self, bounds: crate::feeds::ChiploadBounds) -> f64 {
        match self {
            Self::Conservative => bounds.min_mm_per_tooth,
            Self::Default => 0.5 * (bounds.min_mm_per_tooth + bounds.max_mm_per_tooth),
            Self::Speed => bounds.max_mm_per_tooth,
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
/// orchestrator threads through [`SuggestContext`]. v3.3a added the
/// `scope` field — v3 design doc still proposes a third
/// (`verbose_rationale`) which lands in a later phase.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SuggestPolicy {
    /// Where inside the LUT chipload band the feed-up recalibration
    /// aims. See [`SuggestAggressiveness`].
    pub aggressiveness: SuggestAggressiveness,
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
    /// orchestrator. Default = `SuggestPolicy::default()` =
    /// aggressiveness `Default` (LUT band midpoint) since the v3.0c
    /// flip per the 2026-06-03 directive.
    pub policy: SuggestPolicy,
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
        /// Policy-selected target inside the LUT band, set by
        /// [`SuggestAggressiveness`] (mm/tooth). Sourced from
        /// [`FeedsResult::chipload_bounds`] via
        /// [`SuggestAggressiveness::target_chipload`]. v2.1 targeted
        /// the band lower bound exactly; v3.0b makes this the
        /// policy-selected target. Default policy
        /// (`Conservative`) still resolves to the band lower bound
        /// for bit-identical v2.1 behaviour.
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
    /// configured (tool, material, workholding) combination — the
    /// operator's options are to drop RPM (which raises chipload at
    /// constant feed) or accept the rubbing-floor recipe with the
    /// understanding that edge heating may shorten tool life.
    ChiploadStillLowAfterRecalibration {
        /// Best predicted observed median chipload the loop achieved
        /// (mm/tooth), still below `lut_target_mm_per_tooth`.
        predicted_observed_mm_per_tooth: f64,
        /// Policy-selected target inside the LUT band the loop was
        /// aiming for (mm/tooth), set by [`SuggestAggressiveness`].
        /// Default policy (`Conservative`) resolves to the band lower
        /// bound; other policies aim higher inside the band.
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
        /// The floor actually applied (mm/tooth) — the chip-formation floor,
        /// or the band maximum where the whole derated band sits beneath it.
        floor_mm_per_tooth: f64,
        /// `Some(_)` when the floor was itself capped to the matched band
        /// maximum, mirroring `FeedsWarning::ChiploadClampedToFloor`'s
        /// `band_capped_from`.
        band_capped_from: Option<f64>,
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
    pub workholding: WorkholdingRigidity,
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
    pub workholding: WorkholdingRigidity,
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
    let mut operation = OperationConfig::new_default(input.op_type);
    apply_stock_defaults(&mut operation, input.stock_ctx);
    suggest_for_operation(SuggestForOperationInput {
        operation: &operation,
        tool: input.tool,
        machine: input.machine,
        material: input.material,
        workholding: input.workholding,
        lut: input.lut,
        spindle_strategy: input.spindle_strategy,
        context: input.context,
    })
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
        input.workholding,
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
fn feeds_input_for_operation<'a>(
    operation: &OperationConfig,
    tool: &'a ToolConfig,
    material: &'a Material,
    machine: &'a MachineProfile,
    workholding: WorkholdingRigidity,
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
            workholding_rigidity: workholding,
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
    workholding: WorkholdingRigidity,
    lut: &VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
) -> Result<FeedsResult, FeedsError> {
    let input = feeds_input_for_operation(
        operation,
        tool,
        material,
        machine,
        workholding,
        lut,
        spindle_strategy,
    );
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
    workholding: WorkholdingRigidity,
    lut: &VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
) -> crate::feeds::FeedsExplain {
    let input = feeds_input_for_operation(
        operation,
        tool,
        material,
        machine,
        workholding,
        lut,
        spindle_strategy,
    );
    crate::feeds::explain_feeds(&input)
}

/// Which dimensions of a [`FeedsResult`] an apply writes back to the operation.
///
/// W3.1 (IA cleanup) split the single apply into a SPEED path (feed / plunge /
/// RPM — "how fast") and a CUT-geometry path (stepover / DOC — "how deep/wide,
/// changes the cut"), so the Feeds UI can offer a speed-only "Apply recommended
/// speeds" that never silently rewrites the cut geometry.
///
/// **There is deliberately no `Field` arm** (Checkpoint I-1, 2026-08-12). The
/// Feeds & Speeds modal used to carry six per-field `Apply` buttons that wrote
/// `FeedsExplain::recommended` straight into the operation, skipping
/// [`enforce_invariants`] entirely; on the shipped default fixture that wrote a
/// **4.445 mm** depth of cut where this funnel writes **1.27 mm** (3.50×). The
/// ruling deleted those buttons rather than plumbing a per-field scope, so the
/// scope vocabulary stays "how fast" / "changes the cut" — the two things a
/// user can be told about — and every apply resolves the *whole* operating
/// point before copying a subset back.
///
/// The surviving per-field affordance — the inline ⚡ pill — does not get a
/// scope either. Since G-PILLCLAMP (2026-09-10) it reads its value from
/// [`preview_field_applies`], a dry run of `Both` on a scratch clone, so it
/// offers and writes the funnel's number for that one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyScope {
    /// feed / plunge / RPM — "how fast". Never touches the cut geometry.
    Speeds,
    /// stepover / DOC — "changes the cut". Never touches the speeds.
    CutGeometry,
    /// Both halves, in one transaction.
    Both,
}

/// Apply the calculator's recommendation to `operation`, writing back only the
/// requested [`ApplyScope`].
///
/// The recommended *full* operating point is run through `enforce_invariants`
/// on a scratch clone — so the feed ↔ chipload ↔ DPP coupling is resolved
/// against the complete recommended state — and then only the requested fields
/// are copied into the real operation. This keeps a speed-only or geometry-only
/// apply byte-identical to the combined apply for the fields it does write,
/// while leaving the others (and their provenance) untouched.
// SAFETY: the canonical suggest funnel needs the operation, the provenance
// out-param, the result, tool/machine/material, pass_role, context, the apply
// subset and the speeds-explored flag together.
#[allow(clippy::too_many_arguments)]
fn apply_feeds_subset(
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
    subset: ApplyScope,
    speeds_explored: bool,
) -> Vec<SuggestWarning> {
    let mut scratch = operation.clone();
    scratch.set_feed_rate(round_suggestion_value(result.feed_rate_mm_min, 1.0));
    scratch.set_plunge_rate(round_suggestion_value(result.plunge_rate_mm_min, 1.0));
    // T-12: both setters return `bool` so the caller can refuse instead of
    // discarding the value — their doc comments say exactly that. Capture
    // the refusals here and report them below, but ONLY when this call was
    // asked to write the cut geometry. Under `ApplyScope::Speeds` not
    // writing the depth is the contract, not a dropped recommendation.
    let stepover_mm = round_suggestion_value(result.radial_width_mm, 0.001);
    let depth_mm = round_suggestion_value(result.axial_depth_mm, 0.001);
    let stepover_held = scratch.set_stepover(stepover_mm);
    // Snap the proposal to a depth the machine will actually cut.
    //
    // Generation steps a 2.5D cut as `total / ceil(total / per_pass)`
    // (`DepthDistribution::Even`, which no operation makes configurable), so
    // the realised depth is a staircase: on a 12 mm pocket only 4.00, 3.00,
    // 2.40, 2.00, 1.71, 1.50 and 1.33 are reachable.
    //
    // This does NOT change the cut — generation applies the same arithmetic to
    // whatever is written. It makes the number the engine writes, reasons
    // about and shows the operator equal the number the machine will cut.
    // Without it a recommendation of 2.90 mm is reported as 2.90 while 2.40 is
    // cut, off by 17 %, and the power ladder's own account of what it did
    // names a depth that never happens. See T-12.
    //
    // No total, no snap: an operation whose depth comes from the model surface
    // has nothing to divide, and inventing a total would be worse than leaving
    // the proposal alone.
    let depth_mm = match scratch.total_depth() {
        Some(total) => crate::ops::depth::realised_step_down(total, depth_mm).unwrap_or(depth_mm),
        None => depth_mm,
    };
    let depth_held = scratch.set_depth_per_pass(depth_mm);
    // v3.0d (2026-06-04): also write the calculator's chosen RPM so the
    // rest of enforce_invariants reads a consistent operating point
    // instead of the operation's prior spindle_rpm value.
    //
    // The idempotency argument originally named the chipload
    // recalibration pass (retired 2026-08-13) whose closed-form solve
    // `target × rpm × flutes / arc_fit` propagated RPM drift linearly
    // into feed. That consumer is gone, but the write stays: A-5 measured
    // the rewritten RPM differing from the authored value on three of four
    // fixtures (14 000 → 16 000, 18 000 → 19 000, 20 000 → 19 000), so any
    // downstream pass or reconstruction that reads the operation's RPM
    // still needs this to be the calculator's choice, not a stale one.
    let rpm_written = result.rpm.is_finite() && result.rpm > 0.0;
    if rpm_written {
        scratch.set_spindle_rpm(Some(result.rpm.round() as u32));
    }
    // Enrich the caller-supplied context with the LUT chipload band
    // derived by the calculator so the invariant passes can read it
    // without a separate parameter. Also thread through the matched LUT
    // row + effective diameter so the axial-DOC envelope pass
    // (`pick_axial_envelope`) can query vendor `ap_*_factor` / `ap_*_mm`
    // and the chipload-bounds re-derivation step in `enforce_invariants`
    // can re-apply `doc_derating_scale` against a mutated DPP.
    // ...and the operating point the calculator sized the feed at, so the
    // final pass can re-derive the chip-thinning and depth-tier terms once
    // the clamps above have settled what the operation actually runs.
    // Unrounded on purpose — see `CalculatorOperatingPoint`.
    //
    // Withheld for a drag-to-explore apply: those speeds are the operator's,
    // not the calculator's, so there is no derivation to reconcile and pass 9
    // must leave them exactly as dialled. See `with_explored_speeds`.
    let enriched = SuggestContext {
        chipload_bounds: result.chipload_bounds,
        matched_lut_row: result.matched_lut_row.as_ref(),
        effective_diameter_mm: result.effective_diameter_mm,
        calculator_operating_point: (!speeds_explored).then_some(CalculatorOperatingPoint {
            radial_width_mm: result.radial_width_mm,
            axial_depth_mm: result.axial_depth_mm,
            feed_rate_mm_min: result.feed_rate_mm_min,
            rpm: result.rpm,
        }),
        ..context
    };
    let mut warnings =
        enforce_invariants(&mut scratch, tool, machine, material, pass_role, enriched);

    let write_speeds = matches!(subset, ApplyScope::Speeds | ApplyScope::Both);
    let write_geometry = matches!(subset, ApplyScope::CutGeometry | ApplyScope::Both);
    if write_speeds {
        operation.set_feed_rate(scratch.feed_rate());
        operation.set_plunge_rate(scratch.plunge_rate());
        if rpm_written {
            operation.set_spindle_rpm(scratch.spindle_rpm());
        }
    }
    if write_geometry {
        if let Some(v) = scratch.as_params().stepover() {
            operation.set_stepover(v);
        }
        if let Some(v) = scratch.as_params().depth_per_pass() {
            operation.set_depth_per_pass(v);
        }
        // T-12: this call was asked to write the cut geometry, and the
        // operation has no field to hold one of the values. Say so. The
        // pre-T-12 funnel returned success here, so a caller that needed a
        // shallower pass — the power derate does — could not tell the
        // difference between "applied" and "silently dropped".
        let op_kind = operation.op_type().name();
        if !depth_held {
            warnings.push(SuggestWarning::CutGeometryFieldNotHeld {
                param_name: "depth_per_pass",
                op_kind,
                recommended_mm: depth_mm,
            });
        }
        if !stepover_held {
            warnings.push(SuggestWarning::CutGeometryFieldNotHeld {
                param_name: "stepover",
                op_kind,
                recommended_mm: stepover_mm,
            });
        }
    }
    // Stamp per-field provenance from what actually produced these values
    // (W2.1), gated to the subset we wrote. enforce_invariants may have
    // recalibrated feed/DPP, but the values remain suggest-derived, so the
    // source labels still hold.
    provenance.apply_suggested_subset(result, operation, rpm_written, write_speeds, write_geometry);
    warnings
}

/// Apply the full recommendation — both speeds and cut geometry. The canonical
/// suggest funnel (`suggest_for_operation`, CLI, the GUI "Apply all").
///
/// Values are rounded for UI-friendly display before clamping, matching the
/// historical Suggest-button behaviour.
///
/// `context` carries project-level slots (model bbox, upstream leftover,
/// strategy hint) used by gate-aware Suggest paths — v1.2 reads
/// `context.model_bbox` to gate the runtime-sanity stepover back-off.
/// Callers that don't have the context cheaply available should pass
/// [`SuggestContext::default()`]; the back-off short-circuits to a no-op
/// when `model_bbox` is `None`.
// SAFETY: W2.1 added the `provenance` out-param for per-field stamping, so the
// funnel needs the operation, the provenance, the result, tool/machine/material,
// pass_role and context together.
#[allow(clippy::too_many_arguments)]
pub fn apply_feeds_result_to_op(
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    apply_feeds_subset(
        operation,
        provenance,
        result,
        tool,
        machine,
        material,
        pass_role,
        context,
        ApplyScope::Both,
        false,
    )
}

/// Apply only the recommended *speeds* (feed / plunge / RPM), leaving the cut
/// geometry (stepover / DOC) and its provenance untouched. The "Apply
/// recommended speeds" action — speed-only by construction (W3.1).
// SAFETY: this door forwards the whole funnel argument list — operation,
// provenance, result, tool/machine/material, pass_role and context — to
// `apply_feeds_subset`, so it cannot carry fewer arguments.
#[allow(clippy::too_many_arguments)]
pub fn apply_speeds_to_op(
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    apply_feeds_subset(
        operation,
        provenance,
        result,
        tool,
        machine,
        material,
        pass_role,
        context,
        ApplyScope::Speeds,
        false,
    )
}

/// Apply only the recommended *cut geometry* (stepover / DOC), leaving the
/// speeds and their provenance untouched. The attributed "Apply cut" action
/// (W3.1) — kept distinct because changing DOC/WOC changes the cut.
// SAFETY: this door forwards the whole funnel argument list — operation,
// provenance, result, tool/machine/material, pass_role and context — to
// `apply_feeds_subset`, so it cannot carry fewer arguments.
#[allow(clippy::too_many_arguments)]
pub fn apply_cut_geometry_to_op(
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    apply_feeds_subset(
        operation,
        provenance,
        result,
        tool,
        machine,
        material,
        pass_role,
        context,
        ApplyScope::CutGeometry,
        false,
    )
}

// ── Per-field preview of the funnel (G-PILLCLAMP, 2026-09-10) ──────────────
//
// The GUI's per-field ⚡ pill (Feeds tab feed / plunge, Geometry tab stepover /
// depth-per-pass / drill feed) used to write the RAW calculator value —
// `round_suggestion_value(result.axial_depth_mm, 0.001)` — while every Apply
// button went through `apply_feeds_subset` and wrote the clamped one. On the
// demo pocket (Ø6 flat, Generic Wood Router) the pill wrote 4.2 mm of DOC where
// Apply wrote 1.2 mm (UX-R03-014). The pill now offers and writes the value
// below, which is the funnel's own output read back per field, so the clamp
// chain is never duplicated in the GUI.

/// The value one per-field apply would write, and the stamp it would record.
///
/// Produced by [`preview_field_applies`]. `value` is bit-identical to what
/// `apply_feeds_subset` leaves on the operation for that field (it is read
/// off the funnel's scratch clone, not recomputed), and `provenance` is the
/// stamp the funnel records for it.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldApplyPreview {
    pub field: crate::feeds::FeedsField,
    pub value: f64,
    pub provenance: crate::feeds::ValueProvenance,
}

impl FieldApplyPreview {
    /// Write this one field into `operation` and stamp its provenance. Nothing
    /// else on the operation moves — that is the whole contract of a per-field
    /// pill, and `pill_writes_clamped_value_g_pillclamp.rs` pins it.
    ///
    /// Test door:
    /// `crates/rs_cam_core/tests/pill_writes_clamped_value_g_pillclamp.rs`.
    pub fn write_to(
        &self,
        operation: &mut OperationConfig,
        provenance: &mut crate::feeds::FeedsProvenance,
    ) {
        use crate::feeds::FeedsField;
        match self.field {
            FeedsField::FeedRate => operation.set_feed_rate(self.value),
            FeedsField::PlungeRate => operation.set_plunge_rate(self.value),
            // `value` came from `spindle_rpm()` on the scratch clone, a `u32`,
            // so the round-trip is exact (same cast the funnel itself makes).
            FeedsField::SpindleRpm => operation.set_spindle_rpm(Some(self.value.round() as u32)),
            // These two setters report whether the operation carries the
            // field (N5). This funnel keeps its existing behaviour and
            // discards the answer.
            FeedsField::Stepover => {
                operation.set_stepover(self.value);
            }
            FeedsField::DepthPerPass => {
                operation.set_depth_per_pass(self.value);
            }
            FeedsField::ScallopHeight => operation.set_scallop_height(self.value),
        }
        provenance.set(self.field, self.provenance.clone());
    }
}

/// Every field the funnel would write for this recommendation, as one
/// preview per field. A field the funnel does **not** write — the operation
/// carries no such dial, or no RPM was recommended — is `None`, and a GUI
/// pill for it must say so rather than offer the raw calculator value as if
/// it were clamped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FieldApplyPreviews {
    pub feed_rate: Option<FieldApplyPreview>,
    pub plunge_rate: Option<FieldApplyPreview>,
    pub spindle_rpm: Option<FieldApplyPreview>,
    pub stepover: Option<FieldApplyPreview>,
    pub depth_per_pass: Option<FieldApplyPreview>,
}

impl FieldApplyPreviews {
    pub fn get(&self, field: crate::feeds::FeedsField) -> Option<&FieldApplyPreview> {
        use crate::feeds::FeedsField;
        match field {
            FeedsField::FeedRate => self.feed_rate.as_ref(),
            FeedsField::PlungeRate => self.plunge_rate.as_ref(),
            FeedsField::SpindleRpm => self.spindle_rpm.as_ref(),
            FeedsField::Stepover => self.stepover.as_ref(),
            FeedsField::DepthPerPass => self.depth_per_pass.as_ref(),
            // Never suggested, never written by the funnel.
            FeedsField::ScallopHeight => None,
        }
    }
}

/// Dry-run the apply funnel and read back every field it would write.
///
/// Runs `apply_feeds_subset(ApplyScope::Both)` on a scratch clone of
/// `operation` with a scratch provenance, then reads each field's value and
/// stamp off the scratch. There is still no `ApplyScope::Field` arm (Checkpoint
/// I-1): the whole operating point is resolved, and a single field is copied
/// out of it — which is exactly what a per-field pill must offer.
// SAFETY: the dry run resolves the same operating point as the funnel, so it
// needs the operation, the result, tool/machine/material, pass_role and context.
#[allow(clippy::too_many_arguments)]
pub fn preview_field_applies(
    operation: &OperationConfig,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> FieldApplyPreviews {
    use crate::feeds::FeedsField;
    let mut scratch = operation.clone();
    let mut prov = crate::feeds::FeedsProvenance::default();
    apply_feeds_subset(
        &mut scratch,
        &mut prov,
        result,
        tool,
        machine,
        material,
        pass_role,
        context,
        ApplyScope::Both,
        false,
    );
    let slot = |field: FeedsField, value: Option<f64>| -> Option<FieldApplyPreview> {
        let value = value?;
        let provenance = prov.get(field)?.clone();
        Some(FieldApplyPreview {
            field,
            value,
            provenance,
        })
    };
    FieldApplyPreviews {
        feed_rate: slot(FeedsField::FeedRate, Some(scratch.feed_rate())),
        plunge_rate: slot(FeedsField::PlungeRate, Some(scratch.plunge_rate())),
        spindle_rpm: slot(FeedsField::SpindleRpm, scratch.spindle_rpm().map(f64::from)),
        stepover: slot(FeedsField::Stepover, scratch.stepover()),
        depth_per_pass: slot(FeedsField::DepthPerPass, scratch.depth_per_pass()),
    }
}

/// One field of [`preview_field_applies`].
///
/// Test door:
/// `crates/rs_cam_core/tests/pill_writes_clamped_value_g_pillclamp.rs` and
/// `crates/rs_cam_viz/tests/apply_contract_a3.rs`.
// SAFETY: one field of the dry run, so it carries the whole
// `preview_field_applies` argument list plus the field selector.
#[allow(clippy::too_many_arguments)]
pub fn preview_field_apply(
    operation: &OperationConfig,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
    field: crate::feeds::FeedsField,
) -> Option<FieldApplyPreview> {
    preview_field_applies(
        operation, result, tool, machine, material, pass_role, context,
    )
    .get(field)
    .cloned()
}

// ── The one application funnel (Checkpoint I, 2026-08-12) ──────────────────
//
// A-3's census (`planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md`) found
// thirteen GUI affordances writing a recommendation into an `OperationConfig`,
// of which eleven were reachable on a tool × operation pairing the engine had
// already declared physically unrunnable, and seven wrote the raw calculator
// output with no clamp, back-off or rounding at all.
//
// The repair is structural rather than defensive: the type a *preview* surface
// holds ([`FeedsPreview`]) carries no method that can write, and the only way
// to obtain something that can write ([`ApplicableRecommendation`]) is
// [`FeedsPreview::applicable`], which returns `None` when
// [`crate::feeds::validate_tool_for_operation`] refused. There is exactly one
// write function ([`apply`]), and it always runs `enforce_invariants`.
//
// EXCLUDED FROM THIS FUNNEL, DELIBERATELY (Checkpoint I-4):
//
// - The optimizer's candidate apply (`AppEvent::ApplyOptimizeCandidate`) and
//   its project batch. Their candidates are whole `OperationConfig` snapshots
//   that have been **scored against a simulated cut trace end to end** — they
//   answer to the gate verdicts, not to the pre-simulation feeds calculator,
//   and re-clamping a sim-verified operating point against a pre-sim estimator
//   would substitute the weaker evidence for the stronger one.
// - NOT excluded: the optimizer's single-axis suggestion accept
//   (`reoptimize_with_axis_override`). That writes an **un-simulated**
//   suggested value with no clamp, which is the same defect in a second
//   neighbourhood, so it routes through [`resolve_operation_invariants`].

/// A read-only feeds preview. Infallible by construction — buildable for any
/// input, including a tool × operation pairing the engine refuses, because
/// drawing the nomogram for a pairing you would decline to *run* is exactly
/// what an explanatory surface is for.
///
/// It carries no method that writes to an [`OperationConfig`]. The only bridge
/// from here to a write is [`FeedsPreview::applicable`].
#[derive(Debug, Clone)]
pub struct FeedsPreview {
    explain: crate::feeds::FeedsExplain,
    refusal: Option<FeedsError>,
}

impl FeedsPreview {
    /// Build a preview from a calculator input. Never fails: when
    /// [`crate::feeds::validate_tool_for_operation`] refuses, the refusal is
    /// **recorded**, not returned, and the explanation is still computed.
    pub fn build(input: &FeedsInput<'_>) -> Self {
        let refusal = crate::feeds::validate_tool_for_operation(input).err();
        Self {
            explain: crate::feeds::explain_feeds(input),
            refusal,
        }
    }

    /// The full explanation payload — matched LUT row, sibling rows, machine
    /// envelope. Display only.
    pub fn explain(&self) -> &crate::feeds::FeedsExplain {
        &self.explain
    }

    /// The recommended operating point. **Display only** — this is the raw
    /// calculator output, before any clamp or back-off; writing it into an
    /// operation is the defect this module exists to prevent.
    pub fn recommended(&self) -> &FeedsResult {
        &self.explain.recommended
    }

    /// The refusal, when the tool × operation pairing is physically
    /// unrunnable. A UI holding a `Some` here must render it *in place of*
    /// its apply affordances (Checkpoint I-3) — the explanation survives, the
    /// write becomes impossible.
    pub fn refusal(&self) -> Option<&FeedsError> {
        self.refusal.as_ref()
    }

    /// The only bridge from a preview to a write. `None` exactly when the
    /// pairing was refused.
    ///
    /// The recommendation handed back is `explain().recommended`, which is
    /// `calculate(input)` — bit-identical to what
    /// [`feeds_result_for_operation`] returns for the same input, since both
    /// call the same calculator on the same [`FeedsInput`]. Pinned by
    /// `preview_recommendation_is_bit_identical_to_validated_recipe`.
    pub fn applicable(&self) -> Option<ApplicableRecommendation<'_>> {
        if self.refusal.is_some() {
            return None;
        }
        Some(ApplicableRecommendation {
            result: std::borrow::Cow::Borrowed(&self.explain.recommended),
            speeds_explored: false,
        })
    }
}

/// A recommendation that has passed [`crate::feeds::validate_tool_for_operation`].
///
/// Constructible **only** via [`FeedsPreview::applicable`] — the field is
/// private and there is no public constructor — so possession of one is proof
/// that the pairing validated. It is the sole input to [`apply`].
#[derive(Debug, Clone)]
pub struct ApplicableRecommendation<'a> {
    result: std::borrow::Cow<'a, FeedsResult>,
    /// Set by [`ApplicableRecommendation::with_explored_speeds`]: the feed and
    /// RPM on `result` came off a chart the operator dragged, not out of
    /// [`crate::feeds::calculate`].
    ///
    /// Read by [`apply_feeds_subset`], which then withholds
    /// [`SuggestContext::calculator_operating_point`] so the pass-9 rescale
    /// finds no derivation to reconcile and leaves the dialled value alone.
    speeds_explored: bool,
}

impl ApplicableRecommendation<'_> {
    /// The validated operating point.
    pub fn result(&self) -> &FeedsResult {
        &self.result
    }

    /// Replace the two speed axes with an operator-chosen point, keeping the
    /// rest of the validated recommendation. Backs the modal's
    /// drag-to-explore apply, whose values come off a chart the operator
    /// dragged rather than off the calculator.
    ///
    /// The chipload band is **dropped** on the returned recommendation, and
    /// that is load-bearing: `enforce_invariants`'
    /// `recalibrate_feed_for_chipload` pass short-circuits without a band, so
    /// the funnel's clamps (plunge-to-feed, machine envelope, the DOC chain)
    /// still run while the feed the operator explicitly dialled is **not**
    /// silently re-solved back to the band target. Applying an explored point
    /// and then having the feed move on its own would be a new instance of
    /// the defect this funnel closes, not a fix for it.
    ///
    /// The dropped band is **not** a general "do not touch this feed" signal
    /// and must not be reused as one — a legitimate calculator result on an
    /// RPM-only vendor row publishes no band either, and that feed does want
    /// reconciling. So the 2026-08-19 pass-9 rescale keys off its own explicit
    /// [`ApplicableRecommendation::speeds_explored`] flag, set here, rather
    /// than inferring the operator's intent from an absent band.
    #[must_use]
    pub fn with_explored_speeds(self, feed_mm_min: f64, rpm: f64) -> Self {
        let mut owned = self.result.into_owned();
        owned.feed_rate_mm_min = feed_mm_min.max(1.0);
        owned.rpm = rpm.max(1.0);
        owned.chipload_bounds = None;
        Self {
            result: std::borrow::Cow::Owned(owned),
            speeds_explored: true,
        }
    }
}

/// Everything the funnel needs about the world the operation lives in.
/// Bundled so [`apply`] takes one context parameter rather than five.
#[derive(Debug, Clone, Copy)]
pub struct ApplyContext<'a> {
    pub tool: &'a ToolConfig,
    pub machine: &'a MachineProfile,
    pub material: &'a Material,
    pub pass_role: PassRole,
    /// Project-level slots (model bbox, stock, policy). Pass
    /// [`SuggestContext::default()`] when the caller doesn't cheaply have
    /// them; the funnel enriches it with the recommendation's own LUT band,
    /// matched row and effective diameter regardless.
    pub suggest: SuggestContext<'a>,
}

/// **The single write.** Every apply surface — properties panel, Feeds &
/// Speeds modal, the modal's project batch, the MCP `apply_feeds` tool, the
/// CLI — reaches an `OperationConfig` through here, and this always runs
/// `enforce_invariants`.
///
/// Because the only `ApplicableRecommendation` in existence came out of a
/// [`FeedsPreview`] whose validation succeeded, a refused pairing cannot reach
/// this function at all; and because there is one function, a write cannot
/// skip the clamps.
pub fn apply(
    rec: &ApplicableRecommendation<'_>,
    scope: ApplyScope,
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
    ctx: ApplyContext<'_>,
) -> Vec<SuggestWarning> {
    apply_feeds_subset(
        operation,
        provenance,
        rec.result(),
        ctx.tool,
        ctx.machine,
        ctx.material,
        ctx.pass_role,
        ctx.suggest,
        scope,
        rec.speeds_explored,
    )
}

/// Run the funnel's **clamp stage** over an operation that has just been
/// edited by hand or by the optimizer — an edit that is not a `FeedsResult`
/// and therefore has nothing for [`apply`] to write.
///
/// Checkpoint I-4 routes `reoptimize_with_axis_override` (OPT-005, "accept
/// this axis suggestion") through here. That path sets one of feed / RPM /
/// stepover / DOC / scallop height to a value the optimizer *suggested* but
/// never simulated, and before this it did so with no clamp at all.
///
/// The value itself is left alone — no rounding, no feed re-solve. Callers
/// that want the chipload recalibration must populate
/// `context.chipload_bounds`; with the default context that pass
/// short-circuits, which is what an accepted operator choice should get: the
/// safety clamps, not a substitute number.
pub fn resolve_operation_invariants(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    enforce_invariants(operation, tool, machine, material, pass_role, context)
}

/// Build a [`FeedsPreview`] for an operation in a project context — the
/// preview counterpart of [`feeds_result_for_operation`] and the entry point
/// every apply surface should use.
///
/// Prefer this over [`feeds_explain_for_operation`] anywhere an Apply button
/// might live: the explain payload alone cannot tell a UI that the pairing was
/// refused, which is precisely how the modal came to offer eleven writes on
/// operations the engine had declined to run.
pub fn feeds_preview_for_operation(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    workholding: WorkholdingRigidity,
    lut: &VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
) -> FeedsPreview {
    let input = feeds_input_for_operation(
        operation,
        tool,
        material,
        machine,
        workholding,
        lut,
        spindle_strategy,
    );
    FeedsPreview::build(&input)
}

/// Apply drill-cycle defaults that depend on tool diameter + material —
/// specifically `peck_depth`, which gets overwritten with
/// `material.drill_default_peck_depth_mm(tool.diameter)`.
///
/// Pre-2026-06-02 `DrillConfig::default()` hardcoded `peck_depth = 3.0`
/// regardless of cutter diameter or material; for a 3 mm bit that's a
/// full-diameter peck (unsafe), for a 12 mm bit that's 0.25×D
/// (trivially shallow). The Suggest path now overwrites the value
/// every time it runs, so the operating point auto-scales with tool
/// diameter and material per-peck threshold (audit finding "peck_depth
/// is a hardcoded constant with no diameter/material scaling",
/// workflow `w39ma2j1y`, fix #6).
///
/// Non-drill ops are untouched.
///
/// # Why `stock` (DR-PIN, 2026-08-14)
///
/// Both drill families get the same Suggest peck, but only `Drill`
/// carries its hole depth in its own config. `AlignmentPinDrill`'s hole
/// depth is `stock_z + spoilboard_penetration`, computed at generation
/// time in `compute::execute::generate_alignment_pin_drill` — so until
/// this parameter existed the pin family was written **unclamped**, and
/// a softwood Ø6 pin drill took an 18 mm Suggest peck against a ~13 mm
/// hole: a single-shot cycle wearing a `Peck` label, which the per-peck
/// gate then passed at 2.17 vs 6.0 because one descent is a legal
/// descent (`TECH_DEBT_2_CLOSEOUT.md` §4.4, DR-PIN).
///
/// `stock` is `Option` because not every Suggest caller has a project:
/// the canonical GUI/MCP path
/// ([`crate::session::ProjectSession::cutter_op_profile`]) populates
/// [`SuggestContext::stock`], the strategy-advisor probe in
/// `session::compute` passes `SuggestContext::default()`. **`None` means
/// the pin clamp cannot be applied**, not that it was applied and found
/// nothing to do — the value is left at the unclamped Suggest default,
/// exactly as before, and generation's own emitter guard
/// ([`crate::ops::drill::fed_descents`]) remains the last line.
pub fn apply_drill_defaults(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    stock: Option<&StockContext>,
) {
    let d = tool.diameter;
    if !d.is_finite() || d <= 0.0 {
        return;
    }
    let peck = material.drill_default_peck_depth_mm(d);
    match operation {
        OperationConfig::Drill(cfg) => {
            // Clamp peck so the cycle still pecks at least once
            // (peck must be strictly less than hole depth). With the
            // Janka-banded per-peck max introduced 2026-06-03, a
            // softwood Suggest default of 3×D can exceed shallow
            // drill_depth values (e.g. 12 mm bit + 25 mm hole gives
            // 36 mm peck before clamping). Without this guard the
            // matrix `drill_no_peck_cycle` anti-pattern fires.
            cfg.peck_depth = clamp_peck_to_depth(peck, cfg.depth);
        }
        OperationConfig::AlignmentPinDrill(cfg) => {
            // Same clamp, same ceiling factor, against the depth this
            // family will actually drill. Mirrors `generate_alignment_
            // pin_drill`'s `let depth = stock_z + cfg.spoilboard_
            // penetration;` — if that expression moves, this one has to
            // move with it.
            cfg.peck_depth = match stock {
                Some(ctx) => clamp_peck_to_depth(peck, ctx.stock_z + cfg.spoilboard_penetration),
                None => peck,
            };
        }
        _ => {}
    }
}

/// Ensure the Suggest-default peck depth still produces a real peck
/// cycle. If `peck >= depth` the operator gets a single-shot drill
/// with no chip evacuation; clamp to a fixed fraction of the hole
/// depth so the cycle always pecks at least twice. Chosen factor:
/// 0.75 — keeps the peck count low (2 pecks for typical holes) while
/// staying strictly below `drill_depth`.
fn clamp_peck_to_depth(peck: f64, drill_depth: f64) -> f64 {
    if !drill_depth.is_finite() || drill_depth <= 0.0 {
        return peck;
    }
    let ceiling = drill_depth * 0.75;
    peck.min(ceiling)
}

/// Apply stock-aware overrides to defaults that require stock context.
pub fn apply_stock_defaults(operation: &mut OperationConfig, ctx: &StockContext) {
    match operation {
        OperationConfig::DropCutter(cfg) => {
            cfg.min_z = ctx.stock_bottom_z;
        }
        OperationConfig::Face(cfg) => {
            cfg.depth = ctx.stock_padding.max(1.0);
        }
        OperationConfig::Profile(cfg) => {
            cfg.depth = ctx.stock_z;
        }
        OperationConfig::Drill(cfg) => {
            cfg.depth = ctx.stock_z;
        }
        OperationConfig::Pocket(cfg) => {
            cfg.depth = (ctx.stock_z * 0.5).min(5.0);
        }
        OperationConfig::Adaptive(cfg) => {
            cfg.depth = ctx.stock_z * 0.5;
        }
        _ => {}
    }
}

/// Extract operation-specific hints for the feeds calculator.
/// Returns `(axial_depth_hint, radial_width_hint, scallop_hint)`.
///
/// Thin tuple adapter over the registry-layer
/// [`OperationConfig::feeds_hints`] accessor (Phase 1, architectural
/// refactor T3) — the per-op decision table lives there as an exhaustive
/// match; the old `_ => (None, None, None)` wildcard is gone.
pub fn operation_feeds_hints(
    operation: &OperationConfig,
) -> (Option<f64>, Option<f64>, Option<f64>) {
    let hints = operation.feeds_hints();
    (
        hints.axial_depth_mm,
        hints.radial_width_mm,
        hints.target_scallop_mm,
    )
}

/// Deflection ceiling (µm) for the v1.1 DPP back-off. When the closed-form
/// predictor in [`crate::feeds::predict::predict_peak_deflection_um`]
/// projects a tip deflection above this value, [`enforce_invariants`]
/// iteratively reduces DPP by 20% per step (max 5 steps, floor
/// [`DEFLECTION_BACKOFF_DPP_FLOOR_MM`]) until the prediction clears.
///
/// Until 2026-08-13 it had a second consumer: the retired pass 8 feed-up
/// recalibration refused a solve whose post-write deflection landed within
/// a 10 µm headroom of this gate. That verify was a documented no-op on the
/// current feed-independent predictor and went with the pass (Checkpoint
/// J-1); the headroom constant was deleted alongside it.
///
/// 200 µm matches the post-sim `tool_load::deflection` critical
/// threshold directly. The predictor over-shoots post-sim by ~36% on
/// the Wanaka Back Rough motivating case (487 µm predicted vs 358 µm
/// observed), so this is already conservative — backing off against
/// 80% of 200 µm would trigger too aggressively.
///
/// Drift caveat: the +36% safe-side bias is what keeps the back-off
/// conservative. If
/// [`crate::feeds::predict::predict_peak_deflection_um`]'s underlying
/// constants (especially
/// [`crate::feeds::predict::ENDMILL_CORE_FRACTION`] = 0.7) get tuned
/// closer to the post-sim integrator in
/// [`crate::tool_load::deflection`] / `ToolDefinition::tip_deflection_mm`,
/// this threshold needs re-evaluating — a predictor with smaller
/// systematic over-shoot would push the operating point closer to the
/// real 200 µm boundary.
const DEFLECTION_BACKOFF_TARGET_UM: f64 = 200.0;

/// v1.1 combined-Suggest step 2: minimum DPP the deflection back-off
/// loop is allowed to write (mm). Prevents pathological cases where
/// the predictor over-shoots so badly that iteration would zero out
/// DPP. If the loop bails on this floor, the warning still fires
/// showing the user the predicted µm at the floor — that's the "unmet
/// gate" signal until v2 adds a structured unmet-condition surface.
const DEFLECTION_BACKOFF_DPP_FLOOR_MM: f64 = 0.5;

/// v1.1 combined-Suggest step 2: per-iteration DPP reduction factor.
/// 0.8 → ~20% drop per step; with the 5-iteration cap a starting DPP
/// of 9 mm hits 9 × 0.8⁵ ≈ 2.95 mm before bailing, which is sufficient
/// headroom for every motivating case (Wanaka Back Rough settles
/// inside 3 iterations).
const DEFLECTION_BACKOFF_FACTOR: f64 = 0.8;

/// v1.1 combined-Suggest step 2: maximum back-off iterations.
const DEFLECTION_BACKOFF_MAX_ITERATIONS: u8 = 5;

/// v1.2 combined-Suggest: runtime-sanity target for the stepover
/// back-off loop. When the closed-form move-count predictor in
/// [`crate::feeds::predict::predict_move_count`] projects more samples
/// than this, [`enforce_invariants`] iteratively raises stepover by 50%
/// per step (max [`STEPOVER_BACKOFF_MAX_ITERATIONS`] iterations, ceiling
/// `tool.diameter × `[`STEPOVER_BACKOFF_DIAMETER_FRACTION`]).
///
/// 500 000 is the practical "this will hang generation" threshold on
/// the Wanaka motivating case. On a 140×150 mm stock the Wanaka 3D
/// Finish 0.03 mm stepover predicts ~4.6 M moves; clearing 500 k
/// requires raising stepover to roughly 0.3 mm — still tight for a
/// fine-finish path, but workable.
const STEPOVER_BACKOFF_TARGET_MOVES: u64 = 500_000;

/// v1.2 combined-Suggest: ceiling fraction of tool diameter the
/// stepover back-off is allowed to write. 0.5 (50% of D) is the
/// generally-accepted "you are no longer doing the operation the user
/// asked for" threshold — beyond 50% stepover a finish pass becomes a
/// roughing pass. The diameter floor prevents the back-off from raising
/// stepover into roughing territory in pursuit of runtime sanity.
const STEPOVER_BACKOFF_DIAMETER_FRACTION: f64 = 0.5;

/// v1.2 combined-Suggest: per-iteration stepover multiplier. 1.5 ×
/// (vs deflection's 0.8 ×) — stepover is being *raised*, not lowered,
/// so the factor lives on the high side of 1.0. With 5 iterations the
/// loop can lift stepover by up to 1.5⁵ ≈ 7.6× before bailing on the
/// diameter-fraction ceiling.
const STEPOVER_BACKOFF_FACTOR: f64 = 1.5;

/// v1.2 combined-Suggest: maximum back-off iterations.
const STEPOVER_BACKOFF_MAX_ITERATIONS: u8 = 5;

/// v1.3 combined-Suggest: DPP / tool-diameter ratio above which the
/// plunge-entry transient breaches the deflection gate on an
/// Adaptive-family op with `entry_style = Plunge`. Calibrated against
/// Wanaka post-sim plunge-entry spike data (2026-06-03): a 6 mm tool
/// at DPP = 3.69 mm (ratio 0.615) showed a 362 µm transient deflection
/// during plunge entry even though steady-state landed at 162 µm. A
/// ratio of ~0.5 is enough for the transient to breach the 200 µm
/// gate, so we flag at-or-above 0.5×D.
const PLUNGE_ENTRY_UNSTABLE_DPP_OVER_D: f64 = 0.5;

// RETIRED 2026-08-13 (Checkpoint J-1): `FEED_RAISE_DEFLECTION_RECAL_HEADROOM_UM
// = 10.0` — the refusal headroom under `DEFLECTION_BACKOFF_TARGET_UM` that the
// retired pass 8 feed-up verify measured against. Its only consumer went with
// the pass; the v1.1 DPP back-off uses the bare 200 µm target.

/// Apply policy C of `planning/cutter_axial_constraints_2026-06-06.md`
/// §5.4 to a single axial value against the cutter-axial-constraints
/// envelope.
///
/// - Value > `safe_max` → clamp down + emit `AxialDocClampedByEnvelope`.
/// - Value < chipload-burn floor → leave value + emit `AxialDocBelowBurnFloor`
///   (deliberate conservative pin respected).
/// - SafeBandEmpty → emit `AxialEnvelopeSafeBandEmpty` and clamp to
///   `safe_max` (the upper bound; lower bound is infeasible so further
///   conservatism is pointless).
/// - In-band → leave the value silently.
///
/// Returns the post-policy value plus the list of warnings to surface.
fn apply_axial_envelope(
    commanded_mm: f64,
    envelope: &crate::feeds::cutter_constraints::CutterAxialConstraints,
    op_kind: &'static str,
    param_name: &'static str,
) -> (f64, Vec<SuggestWarning>) {
    use crate::feeds::cutter_constraints::AxialBindingConstraint;
    let safe_max = envelope.safe_max_doc_mm();
    let mut warnings = Vec::new();
    let mut new_val = commanded_mm;

    let binding_str = axial_binding_str(envelope.binding_constraint);

    if matches!(
        envelope.binding_constraint,
        AxialBindingConstraint::SafeBandEmpty
    ) {
        let chipload_floor = envelope.min_doc_chipload_floor_mm.unwrap_or(0.0);
        // SafeBandEmpty implies one of the upper bounds (Deflection / VendorAp /
        // Scallop) is tighter than the floor; surface whichever is.
        let safe_max_now = envelope.safe_max_doc_mm();
        let inner_binding = if envelope
            .max_doc_vendor_mm
            .is_some_and(|v| (v - safe_max_now).abs() < 1.0e-9)
        {
            "vendor_ap"
        } else if envelope
            .max_doc_scallop_mm
            .is_some_and(|v| (v - safe_max_now).abs() < 1.0e-9)
        {
            "scallop"
        } else {
            "deflection"
        };
        warnings.push(SuggestWarning::AxialEnvelopeSafeBandEmpty {
            op_kind,
            max_safe_doc_mm: safe_max,
            chipload_floor_doc_mm: chipload_floor,
            binding_upper: inner_binding,
        });
        if commanded_mm > safe_max && safe_max > 0.0 {
            new_val = safe_max;
            // No additional clamp warning — `AxialEnvelopeSafeBandEmpty`
            // already names the situation.
        }
        return (new_val, warnings);
    }

    if commanded_mm > safe_max && safe_max > 0.0 {
        new_val = safe_max;
        warnings.push(SuggestWarning::AxialDocClampedByEnvelope {
            op_kind,
            param_name,
            commanded_mm,
            clamped_mm: safe_max,
            binding: binding_str,
        });
        return (new_val, warnings);
    }
    if let Some(floor) = envelope.min_doc_chipload_floor_mm
        && commanded_mm < floor
        && floor > 0.0
    {
        warnings.push(SuggestWarning::AxialDocBelowBurnFloor {
            op_kind,
            param_name,
            commanded_mm,
            floor_mm: floor,
        });
    }
    (new_val, warnings)
}

/// Stable string token for an [`AxialBindingConstraint`] — used in
/// `SuggestWarning` payloads (and from there the rationale tree), so
/// the mapping is pinned in one place.
fn axial_binding_str(
    binding: crate::feeds::cutter_constraints::AxialBindingConstraint,
) -> &'static str {
    use crate::feeds::cutter_constraints::AxialBindingConstraint;
    match binding {
        AxialBindingConstraint::Deflection => "deflection",
        AxialBindingConstraint::VendorAp => "vendor_ap",
        AxialBindingConstraint::Scallop => "scallop",
        AxialBindingConstraint::SafeBandEmpty => "safe_band_empty",
    }
}

/// Construct the axial-DOC constraint envelope for an operation at its
/// current operating point (feed / RPM / stepover as configured), or
/// `None` for ops outside the envelope routing.
///
/// This is the env-construction half of [`pick_axial_envelope`] —
/// extracted so [`crate::feeds::profile::CutterOpProfile`] surfaces the
/// SAME envelope Suggest's pass 0 enforces, with per-op radial-WOC /
/// finish-target / deflection-limit derivation in exactly one place.
///
/// Routing (mirrors planning §5):
/// - `Adaptive3d` — rough limit, radial WOC = stepover (default 0.4·D).
/// - `VCarve` — rough limit, radial WOC = ½ engaged width at
///   `max_depth` ([`crate::feeds::geometry::vbit_width_at_depth`]).
/// - `ProjectCurve` — rough limit, radial WOC = 0.2·D.
/// - Finish-3D (Scallop / UnifiedFinish / DropCutter / Waterline /
///   SteepShallow / SpiralFinish / RadialFinish / HorizontalFinish) —
///   finish limit + default scallop target, radial WOC = stepover
///   (default 0.15·D).
/// - Everything else — `None` (2D pocket/contour/drill etc.; the
///   envelope adds nothing the other invariant passes don't cover).
pub(crate) fn axial_envelope_for_operation(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    matched_lut_row: Option<&crate::feeds::vendor_lookup::LookupResult>,
) -> Option<crate::feeds::cutter_constraints::CutterAxialConstraints> {
    use crate::compute::cutter::build_cutter;
    use crate::feeds::cutter_constraints::{
        DEFAULT_FINISH_DEFLECTION_LIMIT_UM, DEFAULT_ROUGH_DEFLECTION_LIMIT_UM,
        DEFAULT_SCALLOP_TARGET_UM, cutter_axial_constraints,
    };

    let tool_def = build_cutter(tool);
    let radial = operation.stepover();
    let feed = operation.feed_rate();
    let rpm = operation.spindle_rpm().unwrap_or(0) as f64;
    let flute_count = tool.flute_count.max(1) as f64;
    let feed_per_tooth_mm = if rpm > 0.0 {
        feed / (rpm * flute_count)
    } else {
        0.0
    };

    match operation {
        OperationConfig::Adaptive3d(_) => {
            let radial_woc = radial.unwrap_or(tool.diameter * 0.4).max(1.0e-3);
            Some(cutter_axial_constraints(
                &tool_def,
                material,
                radial_woc,
                feed_per_tooth_mm,
                matched_lut_row,
                None,
                Some(DEFAULT_ROUGH_DEFLECTION_LIMIT_UM),
            ))
        }
        OperationConfig::VCarve(cfg) => {
            // V-bit radial WOC at the commanded depth is the engaged
            // width along the surface — `vbit_width_at_depth`. We use
            // half the engaged width as the lateral WOC for the force
            // model (triangular groove averages ½ width).
            let geometry = tool_def.to_geometry_hint();
            let radial_woc = match geometry {
                ToolGeometryHint::VBit {
                    included_angle,
                    tip_diameter,
                } => {
                    let w = crate::feeds::geometry::vbit_width_at_depth(
                        included_angle,
                        tip_diameter,
                        cfg.max_depth,
                    )
                    .unwrap_or(tool.diameter * 0.2);
                    (w * 0.5).max(1.0e-3)
                }
                _ => (tool.diameter * 0.2).max(1.0e-3),
            };
            Some(cutter_axial_constraints(
                &tool_def,
                material,
                radial_woc,
                feed_per_tooth_mm,
                matched_lut_row,
                None,
                Some(DEFAULT_ROUGH_DEFLECTION_LIMIT_UM),
            ))
        }
        OperationConfig::ProjectCurve(_) => {
            let radial_woc = (tool.diameter * 0.2).max(1.0e-3);
            Some(cutter_axial_constraints(
                &tool_def,
                material,
                radial_woc,
                feed_per_tooth_mm,
                matched_lut_row,
                None,
                Some(DEFAULT_ROUGH_DEFLECTION_LIMIT_UM),
            ))
        }
        // Finish-3D family — finish deflection limit + scallop target.
        OperationConfig::Scallop(_)
        | OperationConfig::UnifiedFinish(_)
        | OperationConfig::DropCutter(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_) => {
            let radial_woc = radial.unwrap_or(tool.diameter * 0.15).max(1.0e-3);
            Some(cutter_axial_constraints(
                &tool_def,
                material,
                radial_woc,
                feed_per_tooth_mm,
                matched_lut_row,
                Some(DEFAULT_SCALLOP_TARGET_UM),
                Some(DEFAULT_FINISH_DEFLECTION_LIMIT_UM),
            ))
        }
        // Non-axial-envelope ops (2D pocket, contour, drill, etc.) —
        // the envelope adds nothing the existing passes don't already
        // cover (rigidity / cutting-length / deflection backoff).
        _ => None,
    }
}

/// Pass 0 of [`enforce_invariants`] — unified axial-DOC envelope per
/// `planning/cutter_axial_constraints_2026-06-06.md` §5.
///
/// Envelope construction (per-op radial WOC / target / limit routing)
/// lives in [`axial_envelope_for_operation`]; this pass applies the
/// policy to the operation:
/// - `Adaptive3d` — picks `depth_per_pass` via policy C against the
///   envelope. Mutates DPP when the commanded value is above
///   `safe_max_doc_mm`.
/// - `VCarve` — clamps `cfg.max_depth` via policy C. The V-bit
///   engaged width at the candidate depth is the radial WOC.
/// - `ProjectCurve` — warning-only feasibility check on `cfg.depth`.
/// - Finish-3D (Scallop / UnifiedFinish / DropCutter / Waterline /
///   SteepShallow / SpiralFinish / RadialFinish / HorizontalFinish) —
///   emits `FinishEnvelopeAdvisory`; automatic `stock_to_leave` mutation is
///   deferred until in-process stock at gen time lands (planning
///   §5.1.1).
///
/// Returns `(warnings, dpp_mutated)` so `enforce_invariants` can run
/// the chipload-bounds re-derivation step in §5.3 only when needed.
fn pick_axial_envelope(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    context: SuggestContext<'_>,
) -> (Vec<SuggestWarning>, bool) {
    let mut warnings = Vec::new();
    let mut dpp_mutated = false;

    let Some(env) =
        axial_envelope_for_operation(operation, tool, material, context.matched_lut_row)
    else {
        return (warnings, dpp_mutated);
    };

    match operation {
        OperationConfig::Adaptive3d(cfg) => {
            let commanded = cfg.depth_per_pass;
            let (new_dpp, ws) =
                apply_axial_envelope(commanded, &env, "adaptive3d", "depth_per_pass");
            warnings.extend(ws);
            if (new_dpp - commanded).abs() > 1.0e-6 {
                cfg.depth_per_pass = new_dpp;
                dpp_mutated = true;
            }
        }
        OperationConfig::VCarve(cfg) => {
            let commanded = cfg.max_depth;
            let (new_depth, ws) = apply_axial_envelope(commanded, &env, "vcarve", "max_depth");
            warnings.extend(ws);
            if (new_depth - commanded).abs() > 1.0e-6 {
                cfg.max_depth = new_depth;
                // VCarve `max_depth` is not the chipload-DPP — re-derivation
                // only matters for ops where DPP feeds back into the chip
                // geometry calculator. Leave `dpp_mutated` false here.
            }
        }
        OperationConfig::ProjectCurve(cfg) => {
            let safe_max = env.safe_max_doc_mm();
            if cfg.depth > safe_max && safe_max > 0.0 {
                warnings.push(SuggestWarning::ProjectCurveDepthInfeasible {
                    commanded_mm: cfg.depth,
                    max_safe_mm: safe_max,
                    binding: axial_binding_str(env.binding_constraint),
                });
            }
        }
        // Finish-3D family — warning-only per planning §5.1.1.
        OperationConfig::Scallop(_)
        | OperationConfig::UnifiedFinish(_)
        | OperationConfig::DropCutter(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_) => {
            let op_kind = match operation {
                OperationConfig::Scallop(_) => "scallop",
                OperationConfig::UnifiedFinish(_) => "unified_finish",
                OperationConfig::DropCutter(_) => "drop_cutter",
                OperationConfig::Waterline(_) => "waterline",
                OperationConfig::SteepShallow(_) => "steep_shallow",
                OperationConfig::SpiralFinish(_) => "spiral_finish",
                OperationConfig::RadialFinish(_) => "radial_finish",
                OperationConfig::HorizontalFinish(_) => "horizontal_finish",
                _ => "finish_3d",
            };
            let binding_str = axial_binding_str(env.binding_constraint);
            // Only emit the advisory when the envelope actually carries a
            // meaningful bound (safe max under 5×D is the sniff test —
            // anything looser is "no binding" noise).
            if env.safe_max_doc_mm() < tool.diameter * 5.0 {
                warnings.push(SuggestWarning::FinishEnvelopeAdvisory {
                    op_kind,
                    max_safe_doc_mm: env.safe_max_doc_mm(),
                    binding: binding_str,
                });
            }
            if matches!(env.safe_band_is_empty(), Some(true)) {
                warnings.push(SuggestWarning::AxialEnvelopeSafeBandEmpty {
                    op_kind,
                    max_safe_doc_mm: env.safe_max_doc_mm(),
                    chipload_floor_doc_mm: env.min_doc_chipload_floor_mm.unwrap_or(0.0),
                    binding_upper: binding_str,
                });
            }
        }
        // Unreachable in practice: `axial_envelope_for_operation`
        // returns `None` for every other variant, so we never get here
        // with an envelope. Kept as an explicit no-op rather than
        // `unreachable!` so routing changes fail soft.
        _ => {}
    }

    (warnings, dpp_mutated)
}

/// Re-derive `chipload_bounds` after the axial-envelope pass mutated
/// DPP. Uses the same shared helper as `feeds::calculate`'s in-place
/// derivation (`geometry::derate_chipload_bounds`, S.8 — see
/// `planning/finishing_stack_review_2026-07.md`) so the post-mutation
/// chipload-recalibration pass sees a bounds value consistent with the
/// new DPP / effective-D ratio. No-op when the matched LUT row is
/// absent or carries no chipload band (the original derivation would
/// also be `None`).
fn recompute_chipload_bounds_for_dpp(
    matched_row: Option<&crate::feeds::vendor_lookup::LookupResult>,
    effective_diameter_mm: f64,
    operation: &OperationConfig,
    new_dpp_mm: f64,
) -> Option<crate::feeds::ChiploadBounds> {
    let row = matched_row?;
    // Drill ops are excluded from doc-derating per `feeds::calculate`
    // (same path), so leave bounds at the raw LUT values for them —
    // forcing `doc_ratio` to `0.0` bypasses derating since
    // `doc_derating_scale` maps any ratio `<= 1.0` to a scale of
    // `1.0`.
    let op_family = operation.feeds_style().0;
    let is_drill = matches!(op_family, FeedsOperationFamily::Drill);
    let doc_ratio = if is_drill || effective_diameter_mm <= 0.0 {
        0.0
    } else {
        new_dpp_mm / effective_diameter_mm
    };
    let (min, max) = crate::feeds::geometry::derate_chipload_bounds(
        row.chip_load_min_mm,
        row.chip_load_max_mm,
        doc_ratio,
        crate::feeds::geometry::ChiploadBoundPolicy::RequireBoth,
    )?
    .into_pair()?;
    Some(crate::feeds::ChiploadBounds {
        min_mm_per_tooth: min,
        max_mm_per_tooth: max,
    })
}

fn enforce_invariants(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    // NOTE: order matters and must mirror the historical monolithic
    // implementation exactly. In particular the stepover runtime
    // back-off runs *before* the DPP-block, and the plunge-entry
    // stability warning fires *after* the deflection back-off has
    // mutated DPP — both intentional.
    //
    // Pass 0 (Phase 3 — `planning/cutter_axial_constraints_2026-06-06.md`
    // §5.3): unified axial-DOC envelope runs FIRST so downstream passes
    // (rigidity clamp / deflection back-off / chipload recalibration)
    // operate on a value consistent with the cutter geometry envelope.
    // When the pass mutates DPP, `chipload_bounds` is stale — the
    // doc-derating ratio just changed — so we re-derive it from the
    // matched LUT row before the chipload-recalibration pass downstream
    // consumes it.
    //
    // The geometry as it stands BEFORE any pass runs — i.e. what
    // `apply_feeds_subset` wrote off the calculator's result. Pass 9 compares
    // the final values against these to decide whether any pass actually
    // moved the cut, and only re-derives the feed when one did. Captured here
    // rather than reconstructed from the calculator's result because that
    // result is unrounded and these are not; comparing like with like is what
    // keeps an untouched operation's feed byte-identical.
    let entry_stepover = operation.stepover();
    let entry_dpp = operation.depth_per_pass();
    let mut working_context = context;
    let (axial_envelope_warnings, dpp_mutated) =
        pick_axial_envelope(operation, tool, material, working_context);
    let mut warnings = Vec::new();
    warnings.extend(axial_envelope_warnings);
    if dpp_mutated && let Some(new_dpp) = operation.depth_per_pass() {
        let fresh_bounds = recompute_chipload_bounds_for_dpp(
            working_context.matched_lut_row,
            working_context.effective_diameter_mm,
            operation,
            new_dpp,
        );
        working_context.chipload_bounds = fresh_bounds;
    }
    let context = working_context;
    warnings.extend(clamp_plunge_to_feed(operation));
    warnings.extend(clamp_stepover_to_diameter(operation, tool));
    warnings.extend(backoff_stepover_for_runtime(operation, tool, context));
    warnings.extend(clamp_dpp_to_rigidity(operation, tool, machine, pass_role));
    warnings.extend(clamp_dpp_to_cutting_length(operation, tool));
    warnings.extend(backoff_dpp_for_deflection(
        operation, tool, material, machine, pass_role,
    ));
    // v3.3b: strategy-aware entry-style rewrite must run BEFORE the
    // plunge-entry-stability warning — when scope = StrategyAndFeeds the
    // rewrite changes Plunge → Ramp and the downstream warning then
    // finds nothing to flag. Under scope = FeedsWithGates the rewrite
    // short-circuits and the warning still fires for the operator.
    warnings.extend(pick_adaptive3d_entry_style(
        operation, tool, pass_role, context,
    ));
    // v3.3c: clearing-strategy recommendation is warn-only (auto-rewrite
    // deferred to v4 pending classifier calibration) — reads, never writes.
    warnings.extend(pick_adaptive3d_clearing_strategy(
        operation, pass_role, context,
    ));
    warnings.extend(check_plunge_entry_stability(operation, tool, pass_role));
    // Pass 8 — the arc-fit chipload feed recalibration — was RETIRED here on
    // 2026-08-13 (Checkpoint J-1, BINDING). Suggest now emits the un-lifted
    // feed the calculator produced. See the retirement note on
    // `feeds::predict` and `planning/review_2026-08-08/ARC_FIT_RATIO_EVIDENCE.md`.
    //
    // Pass 9 runs LAST, and must: every pass above may still move the stepover
    // or the DPP, and this one exists to reconcile the feed with wherever they
    // finally land.
    warnings.extend(rescale_feed_to_final_geometry(
        operation,
        tool,
        machine,
        entry_stepover,
        entry_dpp,
        context,
    ));
    warnings
}

/// Pass 1: clamp plunge_rate to feed_rate when plunge exceeds feed.
fn clamp_plunge_to_feed(operation: &mut OperationConfig) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    let feed_rate = operation.feed_rate();
    let plunge_rate = operation.plunge_rate();
    if feed_rate.is_finite() && plunge_rate.is_finite() && plunge_rate > feed_rate {
        operation.set_plunge_rate(feed_rate);
        warnings.push(SuggestWarning::PlungeClampedToFeed {
            requested: plunge_rate,
            capped: feed_rate,
        });
    }
    warnings
}

/// Pass 2: clamp stepover to tool diameter when stepover exceeds diameter.
fn clamp_stepover_to_diameter(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(stepover) = operation.stepover()
        && stepover.is_finite()
        && tool.diameter.is_finite()
        && tool.diameter > 0.0
        && stepover > tool.diameter
    {
        operation.set_stepover(tool.diameter);
        warnings.push(SuggestWarning::StepoverClampedToToolDiameter {
            requested: stepover,
            capped: tool.diameter,
        });
    }
    warnings
}

/// Pass 3: v1.2 combined-Suggest runtime-sanity stepover floor.
///
/// The LUT + scallop-height path can write a stepover so small
/// (≤0.1 mm on a small-tip ball, e.g. 0.03 mm on the 2 mm-tip
/// tapered ball in Wanaka 3D Finish 6) that the resulting raster
/// toolpath would emit millions of moves on a normal stock envelope
/// and effectively block `generate_all`. The closed-form move-count
/// predictor consumes the operation's stepover + model bbox + tool
/// diameter and projects an upper-bound move count; when that
/// exceeds [`STEPOVER_BACKOFF_TARGET_MOVES`] we iteratively raise
/// stepover by 50 % per step (max 5 iterations, ceiling
/// tool.diameter × 0.5) until the predicted move count clears the
/// target or the loop bottoms out on the diameter-fraction ceiling.
///
/// Model-bbox is read off `context.model_bbox`. When it's None the
/// predictor returns 0 (no constraint signal) and the loop never
/// executes. Feature-driven ops (V-carve, drill, project_curve,
/// trace, profile) also return 0 — same short-circuit.
fn backoff_stepover_for_runtime(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(stepover_pre) = operation.stepover()
        && stepover_pre.is_finite()
        && stepover_pre > 0.0
        && tool.diameter.is_finite()
        && tool.diameter > 0.0
    {
        let initial_moves =
            crate::feeds::predict::predict_move_count(operation, context.model_bbox, tool);
        if initial_moves > STEPOVER_BACKOFF_TARGET_MOVES {
            let ceiling = tool.diameter * STEPOVER_BACKOFF_DIAMETER_FRACTION;
            let mut iterations: u8 = 0;
            let mut current_so = stepover_pre;
            let mut current_moves = initial_moves;

            while current_moves > STEPOVER_BACKOFF_TARGET_MOVES
                && iterations < STEPOVER_BACKOFF_MAX_ITERATIONS
            {
                let raised = (current_so * STEPOVER_BACKOFF_FACTOR).min(ceiling);
                // Already at (or above) the ceiling — bail. Without
                // this we'd spin the iteration counter without writing
                // any new value.
                if raised <= current_so {
                    break;
                }
                current_so = raised;
                operation.set_stepover(current_so);
                current_moves =
                    crate::feeds::predict::predict_move_count(operation, context.model_bbox, tool);
                iterations = iterations.saturating_add(1);
            }

            if iterations > 0 {
                tracing::debug!(
                    requested_mm = stepover_pre,
                    raised_mm = current_so,
                    predicted_moves_at_requested = initial_moves,
                    predicted_moves_at_raised = current_moves,
                    iterations,
                    "Suggest runtime-sanity back-off raised stepover"
                );
                warnings.push(SuggestWarning::StepoverRaisedForRuntime {
                    requested_mm: stepover_pre,
                    raised_mm: current_so,
                    predicted_moves_at_requested: initial_moves,
                    predicted_moves_at_raised: current_moves,
                    iterations,
                });
            }
        }
    }
    warnings
}

/// Pass 4: clamp DPP to the spindle-rigidity ceiling on roughing passes.
///
/// Adaptive ops are a "deep, narrow" engagement strategy — low
/// radial WOC lets the cutter take a high axial DOC the conventional
/// roughing factor doesn't allow. Pre-2026-06-02 the clamp used
/// `doc_roughing_factor` uniformly, stripping the upstream adaptive
/// ap (computed via `adaptive_doc_factor` at feeds/mod.rs:913) back
/// down to the conventional ceiling — defeating the adaptive
/// paradigm. The clamp now branches on feeds family.
///
/// Audit finding: BUG 1 — "Adaptive DOC clamped by conventional-
/// roughing factor" (workflow `w39ma2j1y`).
fn clamp_dpp_to_rigidity(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    pass_role: PassRole,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(current) = operation.depth_per_pass()
        && matches!(pass_role, PassRole::Roughing)
    {
        let is_adaptive_family =
            operation.op_type().spec().feeds_family == FeedsOperationFamily::Adaptive;
        let factor = if is_adaptive_family {
            machine.rigidity.adaptive_doc_factor
        } else {
            machine.rigidity.doc_roughing_factor
        };
        let cap = factor * tool.diameter;
        if current.is_finite() && cap.is_finite() && cap > 0.0 && current > cap {
            operation.set_depth_per_pass(cap);
            warnings.push(SuggestWarning::RoughingDepthClampedToRigidity {
                requested: current,
                capped: cap,
            });
        }
    }
    warnings
}

/// Pass 5: clamp DPP to the tool's cutting length.
///
/// Reads DPP fresh after the rigidity-clamp pass; runs for all pass
/// roles (not roughing-only).
fn clamp_dpp_to_cutting_length(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(current) = operation.depth_per_pass() {
        let cap = tool.cutting_length;
        if current.is_finite() && cap.is_finite() && cap > 0.0 && current > cap {
            operation.set_depth_per_pass(cap);
            warnings.push(SuggestWarning::DepthClampedToCuttingLength {
                requested: current,
                capped: cap,
            });
        }
    }
    warnings
}

/// Pass 6: v1.1 combined-Suggest deflection-aware DPP back-off.
///
/// The rigidity clamp and cutting-length clamp guarantee structural /
/// geometric safety; this clamp adds a *physical* safety check by
/// consulting the closed-form deflection predictor at the current
/// operating point, then iteratively backing DPP off by 20% per step
/// until the predicted tip deflection clears the 200 µm critical
/// threshold (or the loop bottoms out on the 0.5 mm DPP floor).
///
/// Roughing-only (same gate as the rigidity clamp): finishing passes
/// already run shallow DPPs by design and the deflection envelope
/// rarely threatens them.
///
/// The predictor returns 0 µm for refusal cases (V-bit, drill, un-
/// validated material, zero feed) — the loop's `> target` condition
/// short-circuits trivially and no back-off occurs.
fn backoff_dpp_for_deflection(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    pass_role: PassRole,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(current) = operation.depth_per_pass()
        && matches!(pass_role, PassRole::Roughing)
        && current.is_finite()
        && current > DEFLECTION_BACKOFF_DPP_FLOOR_MM
    {
        let pre_backoff_dpp = current;
        let initial_prediction =
            crate::feeds::predict::predict_peak_deflection_um(operation, tool, material, machine);
        let predicted_initial_um = initial_prediction.predicted_um;
        let mut predicted_um = predicted_initial_um;
        let mut iterations: u8 = 0;
        let mut dpp = current;

        while predicted_um > DEFLECTION_BACKOFF_TARGET_UM
            && iterations < DEFLECTION_BACKOFF_MAX_ITERATIONS
            && dpp > DEFLECTION_BACKOFF_DPP_FLOOR_MM
        {
            dpp = (dpp * DEFLECTION_BACKOFF_FACTOR).max(DEFLECTION_BACKOFF_DPP_FLOOR_MM);
            operation.set_depth_per_pass(dpp);
            let next = crate::feeds::predict::predict_peak_deflection_um(
                operation, tool, material, machine,
            );
            predicted_um = next.predicted_um;
            iterations = iterations.saturating_add(1);
        }

        if iterations > 0 {
            tracing::debug!(
                requested_mm = pre_backoff_dpp,
                capped_mm = dpp,
                predicted_um_at_requested = predicted_initial_um,
                predicted_um_at_capped = predicted_um,
                iterations,
                "Suggest deflection back-off reduced DPP"
            );
            warnings.push(SuggestWarning::DppCappedByDeflection {
                requested_mm: pre_backoff_dpp,
                capped_mm: dpp,
                predicted_um_at_requested: predicted_initial_um,
                predicted_um_at_capped: predicted_um,
                iterations,
            });
        }
    }
    warnings
}

/// Pass 7: v3.3b combined-Suggest strategy-aware entry-style auto-pick.
///
/// When `policy.scope == StrategyAndFeeds`, rewrites
/// `Adaptive3dConfig::entry_style` from `Plunge` to either `Helix` or
/// `Ramp` when the post-back-off DPP / D ratio exceeds
/// [`PLUNGE_ENTRY_UNSTABLE_DPP_OVER_D`] (today's v1.3 warning threshold)
/// and the field is at its `Default::default()` value (treated as
/// unpinned per the v3 design doc's "B" pinning heuristic).
///
/// **v3.4 Helix promotion**: When the v3.4 geometry classifier in
/// [`crate::feeds::geometry_class`] reports `MixedTerrain` /
/// `ShallowTerrain` and the model bbox has headroom for the
/// configured `helix_radius_factor` (see
/// [`crate::feeds::geometry_class::helix_feasible_in_bbox`]), the
/// rewrite picks **Helix** — the cleanest adaptive3d engagement.
/// Falls back to Ramp when the classifier can't confirm headroom or
/// returns `Unknown`. Ramp works on any geometry that fits the
/// cutter at all, so it's the safe baseline.
///
/// Pinning: only rewrites when the live value equals the default
/// (`Plunge`). A user who set Helix or Ramp explicitly keeps their
/// choice. The false-positive case ("user explicitly chose the
/// default") is accepted — the chooser would have picked the same
/// thing anyway in the common case, and the
/// [`SuggestWarning::StrategyRewrote`] entry surfaces what changed so
/// the operator can revert with one click.
///
/// Wanaka motivating case: Back Rough / 3D Rough 6 at DPP=3.69 mm on a
/// 6 mm tool with `entry_style=Plunge` (== default) ships a 362 µm
/// plunge-entry transient — above the 200 µm deflection gate. With
/// the v3.4 classifier the rewrite picks Helix (Wanaka's 140 × 150 mm
/// bbox easily clears the 15.6 mm helix-headroom requirement), giving
/// the gold-standard adaptive3d entry.
fn pick_adaptive3d_entry_style(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    use crate::compute::operation_configs::Adaptive3dEntryStyle;
    use crate::feeds::geometry_class::{self, GeometryClass};
    let mut warnings = Vec::new();
    if !matches!(context.policy.scope, SuggestScope::StrategyAndFeeds) {
        return warnings;
    }
    if !matches!(pass_role, PassRole::Roughing) {
        return warnings;
    }
    if !tool.diameter.is_finite() || tool.diameter <= 0.0 {
        return warnings;
    }
    let Some(dpp) = operation.depth_per_pass() else {
        return warnings;
    };
    if !dpp.is_finite() || dpp <= PLUNGE_ENTRY_UNSTABLE_DPP_OVER_D * tool.diameter {
        return warnings;
    }

    // Only Adaptive3d carries an Adaptive3dEntryStyle field today.
    let op_type = operation.op_type();
    let OperationConfig::Adaptive3d(cfg) = operation else {
        return warnings;
    };

    // v3 design "B" pinning heuristic: only rewrite when the field
    // equals its default. The default for Adaptive3dEntryStyle is
    // `Plunge` (see `Adaptive3dConfig::default()` in
    // `operation_configs.rs`), so the rewrite gate IS "currently set
    // to Plunge". A user who set Helix or Ramp explicitly keeps their
    // choice.
    let from = cfg.entry_style;
    if !matches!(from, Adaptive3dEntryStyle::Plunge) {
        return warnings;
    }

    // v3.4: Helix is the gold-standard adaptive3d entry. Pick it when
    // the classifier reports terrain that supports it AND the bbox
    // has headroom for the configured `helix_radius_factor`. Otherwise
    // fall back to Ramp (safe on any cutter-fitting geometry).
    let class = geometry_class::classify(op_type, context.model_bbox);
    let helix_terrain_ok = matches!(
        class,
        GeometryClass::MixedTerrain | GeometryClass::ShallowTerrain
    );
    let helix_room_ok = geometry_class::helix_feasible_in_bbox(
        context.model_bbox,
        tool.diameter,
        cfg.helix_radius_factor,
    );
    let (to, to_label, reason): (Adaptive3dEntryStyle, &'static str, &'static str) =
        if helix_terrain_ok && helix_room_ok {
            (
                Adaptive3dEntryStyle::Helix,
                "helix",
                "deflection_predict_at_dpp_with_helix_headroom",
            )
        } else {
            (
                Adaptive3dEntryStyle::Ramp,
                "ramp",
                "deflection_predict_at_dpp",
            )
        };
    cfg.entry_style = to;
    tracing::debug!(
        dpp_mm = dpp,
        diameter_mm = tool.diameter,
        from = ?from,
        to = ?to,
        ?class,
        helix_room_ok,
        "Suggest strategy-rewrote entry_style"
    );
    warnings.push(SuggestWarning::StrategyRewrote {
        param: "entry_style",
        from: "plunge".to_owned(),
        to: to_label.to_owned(),
        reason,
    });
    warnings
}

/// Pass 7b: v3.3c strategy-aware clearing-strategy recommendation —
/// **warn-only**, never rewrites.
///
/// The v3 design table maps `MixedTerrain` → `ClearingStrategy::Adaptive`
/// (curvature-adjusted clearing handles the flat/steep mix better than
/// plain contour-parallel offsets), but auto-rewriting across clearing
/// strategies is deferred to v4: today's classifier is bbox-only and
/// the `Adaptive` / `AgentSearch` promotions need calibration against
/// real uncut-band evidence before Suggest writes them. Until then this
/// pass surfaces a [`SuggestWarning::StrategyRecommendedNotApplied`]
/// so the operator (or an MCP agent reading the rationale) can opt in
/// manually.
///
/// Skips when:
/// - `scope != StrategyAndFeeds` (FeedsOnly / FeedsWithGates callers
///   asked Suggest to keep its hands off strategy),
/// - the pass role isn't Roughing (clearing strategy is a roughing knob),
/// - the op isn't Adaptive3d (only carrier of the field today),
/// - the field is pinned (heuristic B: any value ≠
///   `ClearingStrategy::ContourParallel` = `Default::default()` is a
///   deliberate user choice — don't nag),
/// - the classifier returns anything but `MixedTerrain` (no signal, no
///   recommendation).
fn pick_adaptive3d_clearing_strategy(
    operation: &OperationConfig,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    use crate::compute::operation_configs::ClearingStrategy;
    use crate::feeds::geometry_class::{self, GeometryClass};
    let mut warnings = Vec::new();
    if !matches!(context.policy.scope, SuggestScope::StrategyAndFeeds) {
        return warnings;
    }
    if !matches!(pass_role, PassRole::Roughing) {
        return warnings;
    }
    let op_type = operation.op_type();
    let OperationConfig::Adaptive3d(cfg) = operation else {
        return warnings;
    };
    // Heuristic B pinning: only the default value counts as unpinned.
    if !matches!(cfg.clearing_strategy, ClearingStrategy::ContourParallel) {
        return warnings;
    }
    let class = geometry_class::classify(op_type, context.model_bbox);
    if !matches!(class, GeometryClass::MixedTerrain) {
        return warnings;
    }
    tracing::debug!(
        ?class,
        current = "contour_parallel",
        recommended = "adaptive",
        "Suggest recommends (warn-only) adaptive clearing on mixed terrain"
    );
    warnings.push(SuggestWarning::StrategyRecommendedNotApplied {
        param: "clearing_strategy",
        current: "contour_parallel".to_owned(),
        recommended: "adaptive".to_owned(),
        reason: "mixed_terrain_classifier",
    });
    warnings
}

/// Pass 8: v1.3 combined-Suggest plunge-entry / DPP compatibility warning.
///
/// The LUT + adaptive_doc_factor path can write a DPP whose plunge
/// entry transient breaches the deflection gate on an Adaptive3d op
/// whose entry_style is Plunge. The entry planner then refuses to
/// engage and the generated toolpath contains no cutting moves
/// (Wanaka 3D Rough 6, 2026-06-03). Threshold: dpp > 0.5×D, calibrated
/// against Wanaka plunge-entry spike data — DPP=3.69 mm on a 6 mm
/// tool produced a 362 µm entry transient (steady-state 162 µm).
/// Warning-only: strategy auto-rewrite is v2.
///
/// Reads the post-deflection-back-off DPP — this is the intentional
/// ordering inherited from the monolithic enforce_invariants and only
/// fires when `depth_per_pass()` is `Some`.
fn check_plunge_entry_stability(
    operation: &OperationConfig,
    tool: &ToolConfig,
    pass_role: PassRole,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(current) = operation.depth_per_pass()
        && matches!(pass_role, PassRole::Roughing)
        && operation.op_type().spec().feeds_family == FeedsOperationFamily::Adaptive
        && tool.diameter.is_finite()
        && tool.diameter > 0.0
        && current.is_finite()
        && current > PLUNGE_ENTRY_UNSTABLE_DPP_OVER_D * tool.diameter
        && let Some(style) = adaptive_plunge_entry_label(operation)
    {
        warnings.push(SuggestWarning::PlungeEntryUnstableAtDpp {
            dpp_mm: current,
            diameter_mm: tool.diameter,
            entry_style: style.to_owned(),
        });
    }
    warnings
}

/// The geometry-dependent term [`crate::feeds::calculate`] folds into its feed:
/// the depth tier, and **only** the depth tier.
///
/// Deliberately calls the same shipped helper the calculator calls, so this is
/// a re-evaluation of the calculator's own expression at a different operating
/// point rather than a model of it — a second implementation is exactly what
/// Checkpoint C3 retired.
///
/// # Why `ae` is still a parameter
///
/// Until 2026-08-19 this also composed `clamp(radial × axial chip thinning, 1,
/// 4)`, which is why the pass fired on stepover changes at all. G-CHIPTHIN-HALFFIX
/// deleted that multiplication from the calculator, so re-deriving it here
/// would rescale the feed by a factor the feed no longer contains — the exact
/// failure this pass exists to prevent, inverted.
///
/// `ae_mm` is kept in the signature because the calculator's remaining
/// `ae` dependence — the Step 6 power check, whose cross-section moves with
/// both `ae` and `ap` — is the open **G-SUGGEST-POWERSTALE** row. When that is
/// instrumented, this is where the term goes. Keeping the parameter also keeps
/// the pass's mutation test honest: it still notices a stepover the invariants
/// moved, and still declines to act on it, rather than becoming blind to one.
fn geometry_feed_factor(
    _geom: ToolGeometryHint,
    tool: &ToolConfig,
    _ae_mm: f64,
    ap_mm: f64,
) -> f64 {
    crate::feeds::geometry::depth_tier_multiplier(ap_mm, tool.diameter)
}

/// Pass 9 (G-SUGGEST-NOCLAMP, 2026-08-19): **re-derive the feed at the
/// geometry the operation actually ships.**
///
/// `feeds::calculate` sizes two terms — radial × axial chip thinning (Step 5)
/// and the depth tier (Step 5a) — against the `ae` / `ap` it was handed, and
/// bakes both into the feed it returns. Every pass above is then free to
/// rewrite exactly those two values: `backoff_stepover_for_runtime` raises
/// stepover for move-count sanity, `pick_axial_envelope` /
/// `clamp_dpp_to_rigidity` / `clamp_dpp_to_cutting_length` /
/// `backoff_dpp_for_deflection` lower DPP. Nothing reconciled the feed with
/// the result once retired pass 8 left in Checkpoint J-1, so Suggest shipped a
/// feed quoting a cut the operation does not make — over-fed when a stepover
/// was raised, under-fed when a DPP was clamped down a tier.
///
/// # What is held fixed
///
/// The **implied target chipload**: commanded advance per tooth divided by the
/// geometry terms. Every other factor in the calculator's feed expression
/// (target chipload, RPM, flute count, the L/D overhang derate, the
/// workholding factor, the power derate, the safety factor) is independent of
/// `ae` / `ap`, so holding the implied target fixed and re-multiplying by the
/// factor at the final geometry reproduces exactly the feed the calculator
/// would have produced had it been handed the final values — without
/// re-running it, and so without disturbing anything else it decides.
///
/// The re-derivation works in advance-per-tooth rather than feed units so it
/// stays exact when the operation carries a rounded copy of the calculator's
/// RPM (`apply_feeds_subset` writes `result.rpm.round()`).
///
/// # When it does nothing
///
/// - No calculator operating point in the context — [`resolve_operation_invariants`]
///   and direct callers. There is no derivation to reconcile against; see the
///   field docs on [`SuggestContext::calculator_operating_point`].
/// - No pass moved the stepover or the DPP. This is the common case and the
///   pass must leave it **byte-identical**: the entry values are the rounded
///   ones `apply_feeds_subset` wrote, so re-deriving unconditionally would
///   silently un-round every feed in the product for no physical reason.
///
/// # What it deliberately does not re-check
///
/// The **power ceiling** (calculator Step 6). Required power scales with both
/// the feed and the cross-section, and the cross-section moves with `ae`/`ap`,
/// so a rescale can in principle invalidate a power-limited feed. It is not
/// re-checked here because `tests/power_ceiling_parity_f2.rs` measured the
/// power branch never firing at all across three shipped presets × ten species
/// × Ø3/Ø6/Ø12 — rigidity and the machine cutting ceiling bind first, peak
/// utilisation 23.6 % — and the machine ceiling *is* enforced below. On a
/// profile where power does bind this pass can over-feed; that wants its own
/// instrument rather than an unmeasured clamp bolted on here.
///
/// The **deflection budget** (retired pass 8 verified it after its lift). The
/// closed-form predictor is feed-independent — force is `Kc × axial_doc ×
/// radial_woc` — so the verify was a documented no-op there and would be one
/// here too. `backoff_dpp_for_deflection` has already settled DPP above.
fn rescale_feed_to_final_geometry(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    entry_stepover: Option<f64>,
    entry_dpp: Option<f64>,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    let Some(calc) = context.calculator_operating_point else {
        return warnings;
    };
    let usable = |v: f64| v.is_finite() && v > 0.0;
    let requested = operation.feed_rate();
    if !(usable(calc.feed_rate_mm_min) && usable(requested)) {
        return warnings;
    }

    // The operation's final cut geometry. An operation that exposes no
    // stepover / DPP field at all (the surface-following finishes command no
    // axial step, and `narrate_toolpath` says so in those words) cannot have
    // gone stale on that axis, so it falls back to the calculator's own value
    // and that term cancels exactly.
    let final_ae = operation
        .stepover()
        .filter(|v| usable(*v))
        .unwrap_or(calc.radial_width_mm);
    let final_ap = operation
        .depth_per_pass()
        .filter(|v| usable(*v))
        .unwrap_or(calc.axial_depth_mm);

    // Did any pass above actually move the cut? Compared against the entry
    // values, not the calculator's, so an untouched operation is left exactly
    // as it was rather than being re-derived to an unrounded near-identical
    // number.
    let moved = |entry: Option<f64>, final_v: f64| {
        entry.is_some_and(|e| (e - final_v).abs() > f64::EPSILON * e.abs().max(1.0))
    };
    if !(moved(entry_stepover, final_ae) || moved(entry_dpp, final_ap)) {
        return warnings;
    }

    let geom = build_cutter(tool).to_geometry_hint();
    let factor_at_calculator =
        geometry_feed_factor(geom, tool, calc.radial_width_mm, calc.axial_depth_mm);
    let factor_at_final = geometry_feed_factor(geom, tool, final_ae, final_ap);
    if !(usable(factor_at_calculator) && usable(factor_at_final)) {
        return warnings;
    }
    // The cut moved but the feed's geometry terms did not — a stepover raised
    // while both points sit at or past the half-diameter chip-thinning
    // shoulder, a DPP clamped inside one depth tier. There is nothing to
    // re-derive, and re-deriving anyway would replace the calculator's rounded
    // feed with an unrounded near-identical one and file a warning saying
    // nothing changed.
    if (factor_at_final - factor_at_calculator).abs() <= factor_at_calculator * 1e-12 {
        return warnings;
    }

    let flutes = f64::from(tool.flute_count.max(1));
    // Work in advance per tooth where we can — see the RPM note above. When
    // the operation carries no RPM the terms cancel identically in feed units,
    // which is the same statement one algebraic step earlier.
    let op_rpm = operation
        .spindle_rpm()
        .map(f64::from)
        .filter(|v| usable(*v));
    let mut rescaled = match (op_rpm, calc.rpm.is_finite() && calc.rpm > 0.0) {
        (Some(rpm), true) => {
            let implied_target = calc.feed_rate_mm_min / (calc.rpm * flutes) / factor_at_calculator;
            implied_target * factor_at_final * rpm * flutes
        }
        _ => calc.feed_rate_mm_min * factor_at_final / factor_at_calculator,
    };
    if !usable(rescaled) {
        return warnings;
    }

    // Calculator Step 7, re-applied. The cutting-feed ceiling is a physical
    // limit, not a derate, so it survives a re-derivation the same way it
    // survived retired pass 8's lift.
    let mut cap_hit = None;
    let ceiling = machine.cutting_feed_ceiling_mm_min();
    if usable(ceiling) && rescaled > ceiling {
        rescaled = ceiling;
        cap_hit = Some(FeedRecalibrationCap::MaxFeed);
    }

    warnings.push(SuggestWarning::FeedRescaledToFinalGeometry {
        requested_mm_per_min: requested,
        rescaled_mm_per_min: rescaled,
        factor_at_calculator,
        factor_at_final,
        cap_hit,
    });

    // Calculator Step 9b, re-applied. A downward re-derivation (a raised
    // stepover cancelling a chip-thinning lift) can put the commanded advance
    // under the chip-formation threshold, and the floor governs there for the
    // same reason it governs inside the calculator: the derates exist to
    // protect the tool, but rubbing burns the work.
    //
    // The band read is `context.chipload_bounds`, which pass 0 already
    // re-derated against its own DPP mutation. A DPP the rigidity /
    // cutting-length / deflection clamps lowered further is *not* re-derated
    // there, which leaves this floor judged against a slightly harsher band
    // than the final DOC deserves — conservative in the safe direction (a
    // harsher band can only lower the ceiling the floor is capped to), and
    // ledgered rather than fixed inside a feed pass.
    if let Some(rpm) = op_rpm {
        let divisor = rpm * flutes;
        if divisor > 0.0 {
            let commanded = rescaled / divisor;
            let floor = crate::feeds::effective_rubbing_floor(context.chipload_bounds);
            if commanded > 0.0 && commanded < floor {
                let band_capped_from =
                    match crate::feeds::rubbing_floor_clamp_reason(context.chipload_bounds) {
                        crate::feeds::ClampReason::RubbingFloorCappedToBandCeiling {
                            global_floor_mm_per_tooth,
                            ..
                        } => Some(global_floor_mm_per_tooth),
                        crate::feeds::ClampReason::RubbingFloor { .. } => None,
                    };
                warnings.push(SuggestWarning::FeedClampedToChiploadFloor {
                    requested_mm_per_tooth: commanded,
                    floor_mm_per_tooth: floor,
                    band_capped_from,
                });
                // Same conflict resolution as Step 9b: the machine cap wins
                // over the floor, and the warning still fires so the operator
                // sees that neither guarantee was met.
                let machine_max_after_safety = machine.max_feed_mm_min * machine.safety_factor;
                rescaled = (floor * divisor).min(machine_max_after_safety);
            }
        }
    }

    operation.set_feed_rate(rescaled);
    // Pass 1 ran before the feed moved. A downward re-derivation can leave the
    // plunge rate above the feed it was clamped to, so the invariant it exists
    // to hold has to be re-established here rather than left broken.
    warnings.extend(clamp_plunge_to_feed(operation));
    warnings
}

// ── Pass 8 RETIRED 2026-08-13 — Checkpoint J-1/J-5 (operator, BINDING) ──
//
// `recalibrate_feed_for_chipload` stood here. It solved
//
//     feed = target / arc_fit_ratio × rpm × flutes
//
// gated on `ArcFitRatioSource::Calibrated`, so exactly two op families
// (Adaptive3d 0.25, DropCutter 0.15) received an automatic feed lift sized
// by `1 / ratio` — 4.00× and 6.67× on the solved feed, 1.88×–6.67× on the
// shipped feed once the machine cutting-feed ceiling truncates it.
//
// The ratios were fitted against the post-sim chipload gate's arc-mean chip
// thickness, an observation deleted on 2026-08-06. Against the gate's
// current observation (`effective_feed / (rpm · flutes)`, a linear advance
// per tooth) the lift is not merely unjustified but directionally WRONG:
// A-5 measured the shipped recommendation, applied unmodified and
// simulated, at **1.85×–5.00× the gate's own band maximum on 4 of 4
// fixtures** — Suggest's recommendation failed the gate Suggest claimed to
// target, on the default path.
//
// What replaces it: nothing, on the pre-simulation side. Removing the lift
// leaves the commanded advance per tooth at the derated band **minimum**,
// which the calculator already reaches unaided (measured 0.999× on both
// Adaptive3d fixtures) — the rubbing case this pass was built for in
// 2026-06 was stated in the deleted arc-mean quantity. Any lift toward the
// band median now belongs to the **simulation-backed** path, which corrects
// from a measured observation rather than a family constant, and which A-5
// measured clean 4/4 on the same fixtures (`feed_modulation`, whose default
// was flipped ON under Checkpoint J-3).
//
// `SuggestWarning::FeedRaisedForChipload` and
// `SuggestWarning::ChiploadStillLowAfterRecalibration` were deliberately
// KEPT (J-5) so the simulation-backed path can report its own measured feed
// changes through the vocabulary the rationale renderer already speaks.
// Nothing in Suggest constructs them today.

/// Returns `Some(label)` when `operation` is an Adaptive-family op whose
/// entry_style is `Plunge`. Used by [`enforce_invariants`] for the v1.3
/// compatibility warning. Returns `None` for non-Adaptive ops, for
/// Adaptive ops that don't expose an entry_style field (2D Adaptive),
/// and for Adaptive ops whose entry_style is Helix/Ramp.
fn adaptive_plunge_entry_label(operation: &OperationConfig) -> Option<&'static str> {
    use crate::compute::operation_configs::Adaptive3dEntryStyle;
    match operation {
        OperationConfig::Adaptive3d(cfg) => match cfg.entry_style {
            Adaptive3dEntryStyle::Plunge => Some("plunge"),
            Adaptive3dEntryStyle::Helix | Adaptive3dEntryStyle::Ramp => None,
        },
        _ => None,
    }
}

/// Round a suggestion value to the nearest multiple of `step`.
///
/// Used by the Suggest path to produce UI-friendly numbers before
/// clamping (e.g. round chipload to 0.005 mm, RPM to 100). When
/// `step <= 0.0` the value is returned unchanged.
pub fn round_suggestion_value(value: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value;
    }
    (value / step).round() * step
}

#[cfg(test)]
mod tests;
