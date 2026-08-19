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
    /// `param` is the snake-case field name ("entry_style",
    /// "clearing_strategy", "stock_to_leave_radial", etc.). `from` and
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
// The canonical suggest funnel legitimately needs op + provenance + result +
// tool/machine/material + pass_role + context + subset together.
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
    scratch.set_stepover(round_suggestion_value(result.radial_width_mm, 0.001));
    scratch.set_depth_per_pass(round_suggestion_value(result.axial_depth_mm, 0.001));
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
    let warnings = enforce_invariants(&mut scratch, tool, machine, material, pass_role, enriched);

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
// W2.1 added the `provenance` out-param (per-field stamping); the funnel
// legitimately needs op + provenance + result + tool/machine/material +
// pass_role + context together.
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
/// ([`crate::drill::fed_descents`]) remains the last line.
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

/// The two geometry-dependent terms [`crate::feeds::calculate`] folds into
/// its feed at Step 5 / Step 5a: `clamp(radial × axial thinning, 1, 4)` times
/// the depth tier.
///
/// Deliberately calls the same shipped helpers the calculator calls, in the
/// same order, including the calculator's SHADOW POINT — chip thinning reads
/// the effective diameter *at the commanded axial DOC*, while the depth tier
/// reads the nominal one. A second implementation of the chip-thinning
/// diameter is exactly what Checkpoint C3 retired; this is a re-evaluation of
/// the calculator's expression at a different point, not a model of it.
fn geometry_feed_factor(geom: ToolGeometryHint, tool: &ToolConfig, ae_mm: f64, ap_mm: f64) -> f64 {
    use crate::feeds::geometry;
    let effective_d =
        crate::feeds::effective_diameter(geom, tool.diameter, tool.shank_diameter, ap_mm);
    let rctf = geometry::radial_chip_thinning_factor(ae_mm, effective_d);
    let axial = match geom {
        ToolGeometryHint::Ball | ToolGeometryHint::TaperedBall { .. } => {
            geometry::axial_chip_thinning_factor_for_ball(tool.diameter, effective_d)
        }
        _ => 1.0,
    };
    (rctf * axial).clamp(1.0, 4.0) * geometry::depth_tier_multiplier(ap_mm, tool.diameter)
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
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::compute::operation_configs::{DropCutterConfig, PocketConfig};
    use crate::compute::tool_config::{ToolId, ToolType};
    use crate::feeds::{ChiploadSource, EMBEDDED_LUT};

    /// C2 sentinel contract: `SuggestContext::effective_diameter_mm == 0.0`
    /// is "not populated", and the post-mutation chipload re-derivation must
    /// then leave the vendor band UNDERATED — the same outcome the drill path
    /// gets by forcing `doc_ratio = 0.0`. A populated diameter with a DPP
    /// above it must, by contrast, actually derate.
    #[test]
    fn unpopulated_effective_diameter_skips_doc_derating() {
        let row = crate::feeds::vendor_lookup::LookupResult {
            chip_load_mm: 0.1,
            chip_load_min_mm: Some(0.05),
            chip_load_max_mm: Some(0.20),
            rpm_nominal: None,
            rpm_min: None,
            rpm_max: None,
            ap_min_mm: None,
            ap_max_mm: None,
            ap_min_factor: None,
            ap_max_factor: None,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "c2-sentinel".to_owned(),
            source_vendor: crate::feeds::vendor_lut::Vendor::Amana,
            score: 100,
            diameter_match_score: 200,
            row_diameter_mm: 6.0,
            chipload_diameter_scale: 1.0,
            chipload_hardness_scale: 1.0,
            chipload_diameter_ratio_raw: 1.0,
            chipload_hardness_ratio_raw: 1.0,
            is_extrapolated: false,
            row_pass_role: crate::feeds::vendor_lut::LutPassRole::Roughing,
        };
        let op = OperationConfig::new_default(OperationType::Pocket);

        // Not populated → raw LUT band, no derating.
        let unpopulated = recompute_chipload_bounds_for_dpp(Some(&row), 0.0, &op, 12.0)
            .expect("a row with a full band must yield bounds");
        assert!(
            (unpopulated.max_mm_per_tooth - 0.20).abs() < 1e-12
                && (unpopulated.min_mm_per_tooth - 0.05).abs() < 1e-12,
            "unpopulated effective diameter must pass the raw band through, got {unpopulated:?}"
        );

        // Populated, DPP twice the diameter → derated below the raw band.
        let derated = recompute_chipload_bounds_for_dpp(Some(&row), 6.0, &op, 12.0)
            .expect("a row with a full band must yield bounds");
        assert!(
            derated.max_mm_per_tooth < 0.20,
            "a 2.0 DOC ratio must derate the band; got {derated:?}"
        );
    }

    /// Ball-nose tool of the given diameter (tip radius = diameter / 2).
    fn ball_tool(diameter: f64) -> ToolConfig {
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::BallNose);
        tool.diameter = diameter;
        tool.cutting_length = 25.0;
        tool
    }

    /// DropCutter (3D Finish) operation with an optional scallop target.
    fn dropcutter_op(scallop_height: Option<f64>) -> OperationConfig {
        OperationConfig::DropCutter(DropCutterConfig {
            scallop_height,
            ..DropCutterConfig::default()
        })
    }

    /// Run the canonical feeds path for a DropCutter op + tool and return
    /// the suggested radial stepover (mm).
    fn dropcutter_stepover(op: &OperationConfig, tool: &ToolConfig) -> f64 {
        feeds_result_for_operation(
            op,
            tool,
            &Material::default(),
            &MachineProfile::default(),
            WorkholdingRigidity::Medium,
            &EMBEDDED_LUT,
            crate::feeds::SpindleStrategy::default(),
        )
        .expect("dropcutter stepover should not be refused for ball tool")
        .radial_width_mm
    }

    /// S1: a scallop target on DropCutter overrides the `ae_factor`
    /// formula stepover with the chord-height geometry stepover. A 10 μm
    /// scallop on a 1 mm ball (tip r = 0.5 mm) gives
    /// `2·√(2·0.5·0.010 − 0.010²) ≈ 0.199 mm` — practical wood finish —
    /// instead of the sub-micron formula value.
    #[test]
    fn scallop_height_some_overrides_default_ae_factor() {
        let stepover = dropcutter_stepover(&dropcutter_op(Some(0.010)), &ball_tool(1.0));
        let expected = 2.0 * (2.0 * 0.5 * 0.010 - 0.010_f64.powi(2)).sqrt();
        assert!(
            (stepover - expected).abs() < 1e-6,
            "scallop stepover {stepover} should match chord-height {expected}"
        );
        assert!(
            (0.198..0.200).contains(&stepover),
            "10 μm scallop on a 1 mm ball should give ~0.199 mm, got {stepover}"
        );
    }

    /// S1: with no scallop target the legacy formula-based stepover is
    /// preserved — the scallop override must not engage, so the result
    /// stays the tiny `ae_factor`-derived value (well under the scallop
    /// path's 0.199 mm). Guards the F-037 smoke baseline.
    #[test]
    fn scallop_height_none_preserves_legacy_ae() {
        let stepover = dropcutter_stepover(&dropcutter_op(None), &ball_tool(1.0));
        assert!(
            stepover > 0.0 && stepover < 0.05,
            "legacy DropCutter stepover on a 1 mm ball should stay small \
             (formula-based), got {stepover}"
        );
    }

    /// S1: a tapered ball cuts the same cusp curve as a true ball of the
    /// same tip radius — the scallop stepover is determined by the
    /// spherical tip only. A tapered ball with tip radius 0.5 mm
    /// (diameter 1.0 mm) must produce the same stepover as a 1 mm true
    /// ball for the same scallop target.
    #[test]
    fn scallop_height_uses_tip_radius_for_tapered_ball() {
        let mut tapered = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
        tapered.diameter = 1.0; // tip diameter → tip radius 0.5 mm
        tapered.cutting_length = 25.0;
        let tapered_step = dropcutter_stepover(&dropcutter_op(Some(0.010)), &tapered);
        let ball_step = dropcutter_stepover(&dropcutter_op(Some(0.010)), &ball_tool(1.0));
        assert!(
            (tapered_step - ball_step).abs() < 1e-9,
            "tapered ball (tip r=0.5) stepover {tapered_step} should equal \
             1 mm true ball stepover {ball_step}"
        );
    }

    fn stock_ctx() -> StockContext {
        StockContext {
            stock_top_z: 10.0,
            stock_bottom_z: 0.0,
            stock_z: 10.0,
            stock_padding: 1.0,
        }
    }

    fn tool(diameter: f64) -> ToolConfig {
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = diameter;
        tool.cutting_length = 25.0;
        tool
    }

    #[test]
    fn vendor_lut_row_present_path_is_used() {
        let result = suggest_params(SuggestParamsInput {
            op_type: OperationType::Pocket,
            tool: &tool(6.35),
            machine: &MachineProfile::default(),
            material: &Material::default(),
            workholding: WorkholdingRigidity::Medium,
            lut: &EMBEDDED_LUT,
            stock_ctx: &stock_ctx(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            context: SuggestContext::default(),
        })
        .expect("pocket + flat is not a refused combination");
        assert!(matches!(
            result.feeds_result.chipload_source,
            ChiploadSource::VendorLut { .. }
        ));
    }

    /// F1 (2026-06-10 defect-class cleanup): the drill-family suggested
    /// feed survives the apply funnel. Pre-fix, `apply_feeds_subset`
    /// wrote feed then plunge, and the drill configs' aliased setters
    /// ("feed IS plunge") let the milling plunge baseline win —
    /// suggested ~2400 → stored ~595 on every funnel (GUI/MCP add,
    /// Apply-all, CLI --apply-suggest). Post-fix `calculate()` aliases
    /// plunge to the envelope-clamped drill feed, so write order is
    /// irrelevant and the stored value equals the suggestion.
    #[test]
    fn drill_apply_round_trips_suggested_feed() {
        let tool = tool(6.0);
        let machine = MachineProfile::default();
        let material = Material::default();
        for op_type in [OperationType::Drill, OperationType::AlignmentPinDrill] {
            let s = suggest_params(SuggestParamsInput {
                op_type,
                tool: &tool,
                machine: &machine,
                material: &material,
                workholding: WorkholdingRigidity::Medium,
                lut: &EMBEDDED_LUT,
                stock_ctx: &stock_ctx(),
                spindle_strategy: crate::feeds::SpindleStrategy::default(),
                context: SuggestContext::default(),
            })
            .expect("drill + flat is not a refused combination");
            let stored = s.operation.feed_rate();
            let suggested = s.feeds_result.feed_rate_mm_min;
            assert_eq!(
                s.feeds_result.plunge_rate_mm_min, suggested,
                "{op_type:?}: drill FeedsResult must be self-consistent (plunge IS feed)"
            );
            assert!(
                (stored - suggested).abs() <= 1.0,
                "{op_type:?}: stored feed {stored} must match suggested {suggested} \
                 (pre-F1 the plunge baseline clobbered it)"
            );
            // And the stored feed must sit inside the drill plunge-feed
            // gate band — the calculator and the gate share one envelope.
            let (lo, hi) = material.drill_plunge_feed_envelope_per_mm();
            let ratio = stored / 6.0;
            assert!(
                ratio >= lo - 1e-9 && ratio <= hi + 1e-9,
                "{op_type:?}: stored feed/Ø {ratio:.1} outside envelope {lo}-{hi}"
            );
        }
    }

    // ── W3.1 SPEED / CUT split ──────────────────────────────────────────
    //
    // A speed-only apply must change feed/plunge/RPM exactly as the combined
    // apply does while leaving stepover/DOC (and their provenance) untouched;
    // a cut-only apply is the mirror image.

    /// A Pocket op with distinctive sentinel speeds + geometry so "unchanged"
    /// assertions are meaningful, plus the calculator's recommendation for it.
    fn split_fixture() -> (
        OperationConfig,
        FeedsResult,
        ToolConfig,
        MachineProfile,
        Material,
        PassRole,
    ) {
        let tool = tool(6.35);
        let machine = MachineProfile::default();
        let material = Material::default();
        let suggested = suggest_params(SuggestParamsInput {
            op_type: OperationType::Pocket,
            tool: &tool,
            machine: &machine,
            material: &material,
            workholding: WorkholdingRigidity::Medium,
            lut: &EMBEDDED_LUT,
            stock_ctx: &stock_ctx(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            context: SuggestContext::default(),
        })
        .expect("pocket + flat is not a refused combination");

        let mut base = OperationConfig::Pocket(PocketConfig::default());
        base.set_feed_rate(111.0);
        base.set_plunge_rate(22.0);
        base.set_spindle_rpm(Some(9000));
        base.set_stepover(1.234);
        base.set_depth_per_pass(2.345);
        let role = base.feeds_style().1;
        (base, suggested.feeds_result, tool, machine, material, role)
    }

    #[test]
    fn apply_speeds_changes_speeds_leaves_cut_geometry() {
        let (base, result, tool, machine, material, role) = split_fixture();

        let mut both = base.clone();
        let mut both_prov = crate::feeds::FeedsProvenance::default();
        apply_feeds_result_to_op(
            &mut both,
            &mut both_prov,
            &result,
            &tool,
            &machine,
            &material,
            role,
            SuggestContext::default(),
        );

        let mut speeds = base.clone();
        let mut speeds_prov = crate::feeds::FeedsProvenance::default();
        apply_speeds_to_op(
            &mut speeds,
            &mut speeds_prov,
            &result,
            &tool,
            &machine,
            &material,
            role,
            SuggestContext::default(),
        );

        // speeds identical to the combined apply
        assert_eq!(speeds.feed_rate(), both.feed_rate());
        assert_eq!(speeds.plunge_rate(), both.plunge_rate());
        assert_eq!(speeds.spindle_rpm(), both.spindle_rpm());
        assert_eq!(speeds_prov.feed_rate, both_prov.feed_rate);
        assert!(speeds_prov.feed_rate.is_some());
        // cut geometry + its provenance untouched
        assert_eq!(speeds.as_params().stepover(), base.as_params().stepover());
        assert_eq!(
            speeds.as_params().depth_per_pass(),
            base.as_params().depth_per_pass()
        );
        assert!(speeds_prov.stepover.is_none());
        assert!(speeds_prov.depth_per_pass.is_none());
    }

    #[test]
    fn apply_cut_geometry_changes_geometry_leaves_speeds() {
        let (base, result, tool, machine, material, role) = split_fixture();

        let mut both = base.clone();
        let mut both_prov = crate::feeds::FeedsProvenance::default();
        apply_feeds_result_to_op(
            &mut both,
            &mut both_prov,
            &result,
            &tool,
            &machine,
            &material,
            role,
            SuggestContext::default(),
        );

        let mut geom = base.clone();
        let mut geom_prov = crate::feeds::FeedsProvenance::default();
        apply_cut_geometry_to_op(
            &mut geom,
            &mut geom_prov,
            &result,
            &tool,
            &machine,
            &material,
            role,
            SuggestContext::default(),
        );

        // geometry identical to the combined apply
        assert_eq!(geom.as_params().stepover(), both.as_params().stepover());
        assert_eq!(
            geom.as_params().depth_per_pass(),
            both.as_params().depth_per_pass()
        );
        assert_eq!(geom_prov.stepover, both_prov.stepover);
        assert!(geom_prov.stepover.is_some());
        // speeds + their provenance untouched
        assert_eq!(geom.feed_rate(), base.feed_rate());
        assert_eq!(geom.plunge_rate(), base.plunge_rate());
        assert_eq!(geom.spindle_rpm(), base.spindle_rpm());
        assert!(geom_prov.feed_rate.is_none());
        assert!(geom_prov.spindle_rpm.is_none());
    }

    // ── Checkpoint I funnel (A-4, 2026-08-12) ──────────────────────────

    /// Fixture pair for the funnel tests: the shipped default Ø6.35 2-flute
    /// flat end mill on a Pocket (valid pairing) and on a Scallop (refused —
    /// a zero tip radius makes `2·√(2·R·h − h²)` undefined).
    fn preview_of(op: &OperationConfig, tool: &ToolConfig) -> FeedsPreview {
        feeds_preview_for_operation(
            op,
            tool,
            &Material::default(),
            &MachineProfile::default(),
            WorkholdingRigidity::Medium,
            &EMBEDDED_LUT,
            crate::feeds::SpindleStrategy::default(),
        )
    }

    fn preview_for(op_type: OperationType) -> FeedsPreview {
        preview_of(
            &OperationConfig::new_default(op_type),
            &ToolConfig::new_default(ToolId(1), ToolType::EndMill),
        )
    }

    /// The funnel's central claim: an `ApplicableRecommendation` exists iff
    /// the pairing validated, and it is the only way to reach [`apply`].
    #[test]
    fn preview_refuses_to_yield_an_applicable_recommendation_when_validation_refused() {
        let refused = preview_for(OperationType::Scallop);
        assert!(
            matches!(
                refused.refusal(),
                Some(FeedsError::WrongToolForOperation { .. })
            ),
            "fixture no longer refuses: {:?}",
            refused.refusal()
        );
        assert!(
            refused.applicable().is_none(),
            "a refused preview handed out a writable recommendation — the funnel's \
             only structural guarantee has been lost"
        );
        // The explanation survives the refusal — that is the point of I-3.
        assert!(refused.recommended().feed_rate_mm_min > 0.0);

        let ok = preview_for(OperationType::Pocket);
        assert!(ok.refusal().is_none());
        assert!(ok.applicable().is_some());
    }

    /// [`FeedsPreview::applicable`] hands back `explain().recommended`, and
    /// the panel's [`feeds_result_for_operation`] hands back `calculate()` of
    /// the same input. Both surfaces must therefore be applying the identical
    /// recipe — this pins that, so "the modal and the panel now agree" rests
    /// on a measured equality rather than on reading two call sites.
    #[test]
    fn preview_recommendation_is_bit_identical_to_validated_recipe() {
        let op = OperationConfig::new_default(OperationType::Pocket);
        let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
        let recipe = feeds_result_for_operation(
            &op,
            &tool,
            &Material::default(),
            &MachineProfile::default(),
            WorkholdingRigidity::Medium,
            &EMBEDDED_LUT,
            crate::feeds::SpindleStrategy::default(),
        )
        .expect("pocket + flat end mill is a valid pairing");
        let preview = preview_for(OperationType::Pocket);
        let previewed = preview.recommended();
        for (name, a, b) in [
            ("feed", recipe.feed_rate_mm_min, previewed.feed_rate_mm_min),
            (
                "plunge",
                recipe.plunge_rate_mm_min,
                previewed.plunge_rate_mm_min,
            ),
            ("rpm", recipe.rpm, previewed.rpm),
            ("doc", recipe.axial_depth_mm, previewed.axial_depth_mm),
            ("woc", recipe.radial_width_mm, previewed.radial_width_mm),
        ] {
            assert_eq!(a.to_bits(), b.to_bits(), "{name}: {a} vs {b}");
        }
    }

    /// [`apply`] with each scope must be indistinguishable from the three
    /// legacy entry points it now backs — the funnel was extended, not
    /// duplicated, and no recipe number may move (A-4 bar 2).
    #[test]
    fn apply_scope_matches_the_legacy_entry_points_exactly() {
        let (base, _unused, tool, machine, material, role) = split_fixture();
        let preview = preview_of(&base, &tool);
        let result = preview.recommended().clone();
        let rec = preview.applicable().expect("valid pairing");
        let ctx = ApplyContext {
            tool: &tool,
            machine: &machine,
            material: &material,
            pass_role: role,
            suggest: SuggestContext::default(),
        };

        for scope in [
            ApplyScope::Both,
            ApplyScope::Speeds,
            ApplyScope::CutGeometry,
        ] {
            let mut legacy_op = base.clone();
            let mut legacy_prov = crate::feeds::FeedsProvenance::default();
            let legacy = match scope {
                ApplyScope::Both => apply_feeds_result_to_op,
                ApplyScope::Speeds => apply_speeds_to_op,
                ApplyScope::CutGeometry => apply_cut_geometry_to_op,
            };
            legacy(
                &mut legacy_op,
                &mut legacy_prov,
                &result,
                &tool,
                &machine,
                &material,
                role,
                SuggestContext::default(),
            );

            let mut funnel_op = base.clone();
            let mut funnel_prov = crate::feeds::FeedsProvenance::default();
            apply(&rec, scope, &mut funnel_op, &mut funnel_prov, ctx);

            assert_eq!(
                funnel_op.feed_rate(),
                legacy_op.feed_rate(),
                "{scope:?} feed"
            );
            assert_eq!(
                funnel_op.plunge_rate(),
                legacy_op.plunge_rate(),
                "{scope:?} plunge"
            );
            assert_eq!(
                funnel_op.spindle_rpm(),
                legacy_op.spindle_rpm(),
                "{scope:?} rpm"
            );
            assert_eq!(
                funnel_op.as_params().stepover(),
                legacy_op.as_params().stepover(),
                "{scope:?} woc"
            );
            assert_eq!(
                funnel_op.as_params().depth_per_pass(),
                legacy_op.as_params().depth_per_pass(),
                "{scope:?} doc"
            );
        }
    }

    /// The explored-point apply keeps the operator's dragged feed/RPM (the
    /// chipload recalibration must not re-solve them) while still taking the
    /// clamps — here the plunge-to-feed clamp, exercised by dragging the feed
    /// below the operation's plunge rate.
    #[test]
    fn explored_speeds_survive_the_funnel_but_still_get_clamped() {
        let (base, _result, tool, machine, material, role) = split_fixture();
        let preview = preview_of(&base, &tool);
        let rec = preview
            .applicable()
            .expect("valid pairing")
            .with_explored_speeds(120.0, 14_000.0);
        let mut op = base.clone();
        op.set_plunge_rate(900.0);
        let mut prov = crate::feeds::FeedsProvenance::default();
        let warnings = apply(
            &rec,
            ApplyScope::Speeds,
            &mut op,
            &mut prov,
            ApplyContext {
                tool: &tool,
                machine: &machine,
                material: &material,
                pass_role: role,
                suggest: SuggestContext::default(),
            },
        );
        assert_eq!(op.feed_rate(), 120.0, "the explored feed was re-solved");
        assert_eq!(op.spindle_rpm(), Some(14_000), "the explored RPM moved");
        assert_eq!(
            op.plunge_rate(),
            120.0,
            "plunge was not clamped to the explored feed — the funnel's clamps did not run"
        );
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::PlungeClampedToFeed { .. })),
            "the clamp ran silently: {warnings:?}"
        );
        // Cut geometry untouched by a Speeds-scoped apply.
        assert_eq!(op.as_params().stepover(), base.as_params().stepover());
        assert_eq!(
            op.as_params().depth_per_pass(),
            base.as_params().depth_per_pass()
        );
    }

    /// [`resolve_operation_invariants`] is the funnel entry for a single-axis
    /// operator/optimizer edit (Checkpoint I-4, OPT-005). It must clamp, and
    /// it must NOT rewrite the accepted value.
    #[test]
    fn resolve_operation_invariants_clamps_an_axis_edit_without_re_solving_it() {
        let (mut op, _result, tool, machine, material, role) = split_fixture();
        // Optimizer suggests a stepover wider than the cutter.
        op.set_stepover(tool.diameter * 3.0);
        op.set_feed_rate(1234.0);
        let warnings = resolve_operation_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            role,
            SuggestContext::default(),
        );
        assert_eq!(
            op.as_params().stepover(),
            Some(tool.diameter),
            "stepover was not clamped to the cutter diameter"
        );
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::StepoverClampedToToolDiameter { .. })),
            "clamp ran without a warning: {warnings:?}"
        );
        assert_eq!(
            op.feed_rate(),
            1234.0,
            "the accepted feed was re-solved; the default context must leave it alone"
        );
    }

    #[test]
    fn fallback_formula_path_is_used_without_matching_lut_row() {
        // 200 mm exceeds 10x the largest embedded flat-end pocket row
        // (12.7 mm compression spiral), so no row passes the diameter
        // sanity floor and the empirical fallback must take over.
        let result = suggest_params(SuggestParamsInput {
            op_type: OperationType::Pocket,
            tool: &tool(200.0),
            machine: &MachineProfile::default(),
            material: &Material::default(),
            workholding: WorkholdingRigidity::Medium,
            lut: &EMBEDDED_LUT,
            stock_ctx: &stock_ctx(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            context: SuggestContext::default(),
        })
        .expect("pocket + flat is not a refused combination");
        assert_eq!(
            result.feeds_result.chipload_source,
            ChiploadSource::FormulaFallback
        );
    }

    #[test]
    fn invariants_clamp_and_warn() {
        let mut op = OperationConfig::Pocket(PocketConfig {
            feed_rate: 100.0,
            plunge_rate: 250.0,
            stepover: 12.0,
            depth_per_pass: 20.0,
            ..PocketConfig::default()
        });
        let mut tool = tool(6.0);
        tool.cutting_length = 1.0;
        // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7):
        // at the new_default 45 mm stickout the deflection back-off now
        // clamps DOC to 0.512, below the cutting-length clamp (1.0) this
        // invariant test asserts. Stiffen the tool (stickout 45 → 10 mm;
        // δ ∝ stickout³ → ~0.011×) so the cutting-length clamp is the
        // binding one and all four invariant warnings remain the thing
        // under test.
        tool.stickout = 10.0;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.25;

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &Material::default(),
            PassRole::Roughing,
            SuggestContext::default(),
        );

        assert_eq!(op.plunge_rate(), 100.0);
        assert_eq!(op.stepover(), Some(6.0));
        assert_eq!(op.depth_per_pass(), Some(1.0));
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::PlungeClampedToFeed { .. }))
        );
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::StepoverClampedToToolDiameter { .. }))
        );
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::RoughingDepthClampedToRigidity { .. }))
        );
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::DepthClampedToCuttingLength { .. }))
        );
    }

    /// Fix #6 (2026-06-02 audit): `peck_depth` is overwritten by
    /// `material.drill_default_peck_depth_mm(D)` when the Suggest
    /// path runs.
    ///
    /// **Updated 2026-06-03 (P5 lit-matrix fix):** softwood per-peck
    /// max is now Janka-banded at 6.0×D (was a flat 2.0 for every
    /// wood). Softwood Suggest default is therefore 0.5 × 6.0 × D =
    /// 3.0×D — inside the matrix band 3–8×D — instead of the old
    /// 1.0×D that matched dense hardwood. `apply_drill_defaults`
    /// additionally clamps the result to `0.75 × drill_depth` so
    /// even shallow holes still peck at least twice. For the test's
    /// default `DrillConfig` (`depth = 10 mm`) the clamp ceiling is
    /// 7.5 mm.
    #[test]
    fn drill_peck_depth_scales_with_diameter_and_material() {
        use crate::compute::operation_configs::{DrillConfig, DrillCycleType};
        use crate::material::Material;

        let material = Material::SolidWood {
            species: crate::material::WoodSpecies::GenericSoftwood,
        };

        // 3 mm bit — pre-2026-06-03 result was 3.0 mm (1.0×D, matched
        // hardwood). Janka-banded softwood now defaults to 0.5×6.0×D
        // = 9.0 mm, then clamps to 0.75 × default depth (10 mm) = 7.5 mm.
        let mut op_3mm = OperationConfig::Drill(DrillConfig {
            cycle: DrillCycleType::Peck,
            peck_depth: 3.0, // pre-fix hardcode value
            ..DrillConfig::default()
        });
        let mut tool_3mm = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool_3mm.diameter = 3.0;

        apply_drill_defaults(&mut op_3mm, &tool_3mm, &material, None);

        let peck_3mm = match &op_3mm {
            OperationConfig::Drill(cfg) => cfg.peck_depth,
            _ => panic!("expected Drill"),
        };
        assert!(
            (peck_3mm - 7.5).abs() < 1e-6,
            "3 mm bit softwood peck depth should be min(0.5×6.0×3.0, 0.75×10.0) = 7.5 mm \
             (Janka-banded softwood clamped to 0.75×depth), got {peck_3mm}"
        );

        // 12 mm bit — softwood Janka band gives 0.5×6.0×12 = 36.0 mm,
        // clamped to 0.75 × 10 mm = 7.5 mm by the default depth.
        let mut op_12mm = OperationConfig::Drill(DrillConfig {
            cycle: DrillCycleType::Peck,
            peck_depth: 3.0, // pre-fix hardcode value
            ..DrillConfig::default()
        });
        let mut tool_12mm = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool_12mm.diameter = 12.0;

        apply_drill_defaults(&mut op_12mm, &tool_12mm, &material, None);

        let peck_12mm = match &op_12mm {
            OperationConfig::Drill(cfg) => cfg.peck_depth,
            _ => panic!("expected Drill"),
        };
        assert!(
            peck_12mm > 6.0,
            "12 mm bit softwood peck depth should scale up beyond the old 3.0 mm hardcode, got {peck_12mm}"
        );

        // Non-drill op: untouched.
        let mut op_pocket = OperationConfig::Pocket(PocketConfig::default());
        let pocket_before = format!("{op_pocket:?}");
        apply_drill_defaults(&mut op_pocket, &tool_3mm, &material, None);
        assert_eq!(
            format!("{op_pocket:?}"),
            pocket_before,
            "apply_drill_defaults must be a no-op for non-drill ops"
        );
    }

    /// **DR-PIN sentry.** An out-of-range `AlignmentPinDrill` peck must
    /// be clamped by the same rule, with the same ceiling factor, as
    /// `Drill`'s — against the depth the pin family actually drills
    /// (`stock_z + spoilboard_penetration`, the expression
    /// `compute::execute::generate_alignment_pin_drill` uses).
    ///
    /// The pre-fix reproduction is the first assertion and it stays:
    /// with **no stock context** the pin arm is still written
    /// unclamped, because the clamp has nothing to clamp against. That
    /// was the ONLY behaviour before 2026-08-14 — the arm read
    /// `AlignmentPinDrill(cfg) => cfg.peck_depth = peck` on every path
    /// — and it is what `TECH_DEBT_2_CLOSEOUT.md` §4.4 DR-PIN measured:
    /// a softwood Ø6 pin drill takes an 18 mm peck at a ~13 mm hole, a
    /// single-shot cycle wearing a `Peck` label that the per-peck gate
    /// then passes at 2.17 vs 6.0 because one descent is a legal
    /// descent.
    ///
    /// "Identically to Drill's" is asserted as an EQUALITY between the
    /// two families at the same hole depth, not as two numbers that
    /// happen to match a literal — so a future change to
    /// `clamp_peck_to_depth`'s 0.75 factor moves both or fails here.
    #[test]
    fn alignment_pin_drill_peck_is_clamped_like_drill() {
        use crate::compute::operation_configs::{
            AlignmentPinDrillConfig, DrillConfig, DrillCycleType,
        };
        use crate::material::{Material, WoodSpecies};

        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;

        // Softwood Ø6 Suggest default: 0.5 × 6.0 × 6.0 = 18.0 mm.
        let suggest_default = material.drill_default_peck_depth_mm(6.0);
        assert!(
            (suggest_default - 18.0).abs() < 1e-9,
            "fixture assumes the softwood Ø6 Suggest peck is 18 mm, got {suggest_default}"
        );

        let pin_cfg = || AlignmentPinDrillConfig {
            spoilboard_penetration: 2.0,
            cycle: DrillCycleType::Peck,
            peck_depth: 3.0,
            ..AlignmentPinDrillConfig::default()
        };
        let peck_of = |op: &OperationConfig| -> f64 {
            match op {
                OperationConfig::AlignmentPinDrill(cfg) => cfg.peck_depth,
                OperationConfig::Drill(cfg) => cfg.peck_depth,
                other => panic!("expected a drill family op, got {other:?}"),
            }
        };

        // Pre-fix reproduction, preserved: no stock context, no clamp.
        let mut unclamped = OperationConfig::AlignmentPinDrill(pin_cfg());
        apply_drill_defaults(&mut unclamped, &tool, &material, None);
        assert!(
            (peck_of(&unclamped) - 18.0).abs() < 1e-9,
            "with no stock context the pin peck stays at the raw Suggest default \
             (the clamp has no depth to clamp against); got {}",
            peck_of(&unclamped)
        );

        // 11 mm stock + 2 mm spoilboard penetration = a 13 mm hole.
        let stock = StockContext {
            stock_top_z: 0.0,
            stock_bottom_z: -11.0,
            stock_z: 11.0,
            stock_padding: 0.0,
        };
        let hole_depth = stock.stock_z + 2.0;

        let mut pin = OperationConfig::AlignmentPinDrill(pin_cfg());
        apply_drill_defaults(&mut pin, &tool, &material, Some(&stock));

        let mut drill = OperationConfig::Drill(DrillConfig {
            cycle: DrillCycleType::Peck,
            peck_depth: 3.0,
            depth: hole_depth,
            ..DrillConfig::default()
        });
        apply_drill_defaults(&mut drill, &tool, &material, Some(&stock));

        assert!(
            (peck_of(&pin) - peck_of(&drill)).abs() < 1e-9,
            "the pin family must clamp identically to Drill at the same hole \
             depth ({hole_depth} mm): pin {} vs drill {}",
            peck_of(&pin),
            peck_of(&drill)
        );
        assert!(
            (peck_of(&pin) - 9.75).abs() < 1e-9,
            "0.75 × 13 mm = 9.75 mm is the shared ceiling; got {}",
            peck_of(&pin)
        );
        assert!(
            peck_of(&pin) < hole_depth,
            "a clamped peck must be strictly below the hole depth or the cycle \
             is single-shot: {} vs {hole_depth}",
            peck_of(&pin)
        );
    }

    /// P5 (2026-06-03 literature-matrix triage): softwood Suggest
    /// per-peck must exceed dense-hardwood Suggest per-peck. Before
    /// the Janka-banded `drill_per_peck_max_dtd` patch, every wood
    /// species returned a flat 2.0 max → 1.0×D default, so softwood
    /// pecks collapsed to the same value as ipe (Janka 3510). That
    /// triggered the matrix anti-pattern
    /// `softwood_drill_matches_hardwood_peck` (`peck_over_d < 3.0`)
    /// on `flat_3mm_drill_softwood`.
    ///
    /// Sources for the 3–8×D softwood band: Onsrud Drill Chart, FPL
    /// Wood Handbook §3.7, Vectric drill defaults, Amana Spektra.
    /// Hardwood ceiling of 5×D is the matrix invariant for white oak
    /// in `flat_3mm_drill_oak`.
    #[test]
    fn drill_peck_depth_softwood_exceeds_hardwood() {
        use crate::compute::operation_configs::{DrillConfig, DrillCycleType};
        use crate::material::{Material, WoodSpecies};

        let pine = Material::SolidWood {
            species: WoodSpecies::RadiataPine, // Janka 710 → just above 700 cutoff;
                                               // use GenericSoftwood for true softwood
        };
        let generic_softwood = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood, // Janka 600
        };
        let white_oak = Material::SolidWood {
            species: WoodSpecies::WhiteOak, // Janka 1360 (medium hardwood)
        };
        let ipe = Material::SolidWood {
            species: WoodSpecies::Ipe, // Janka 3510 (dense hardwood / unknown bucket)
        };

        let diameter = 6.0_f64;
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = diameter;

        // Use a deep hole so the per-material Suggest defaults stay
        // below the 0.75 × drill_depth clamp; otherwise all materials
        // saturate at the same ceiling and the ordering can't be
        // tested. 50 mm hole → 37.5 mm clamp ceiling, well above any
        // 6 mm-diameter Suggest result.
        let drill_depth = 50.0_f64;

        let peck_for = |material: &Material| -> f64 {
            let mut op = OperationConfig::Drill(DrillConfig {
                cycle: DrillCycleType::Peck,
                peck_depth: 3.0,
                depth: drill_depth,
                ..DrillConfig::default()
            });
            apply_drill_defaults(&mut op, &tool, material, None);
            match &op {
                OperationConfig::Drill(cfg) => cfg.peck_depth,
                _ => panic!("expected Drill"),
            }
        };

        let pine_peck = peck_for(&pine);
        let generic_softwood_peck = peck_for(&generic_softwood);
        let oak_peck = peck_for(&white_oak);
        let ipe_peck = peck_for(&ipe);

        let pine_over_d = pine_peck / diameter;
        let softwood_over_d = generic_softwood_peck / diameter;
        let oak_over_d = oak_peck / diameter;
        let ipe_over_d = ipe_peck / diameter;

        // Softwood must clear the matrix floor (3×D) for
        // `softwood_drill_matches_hardwood_peck` (peck_over_d < 3.0).
        assert!(
            softwood_over_d >= 3.0,
            "generic softwood peck must be ≥ 3×D (matrix band floor for `flat_3mm_drill_softwood`), \
             got {softwood_over_d:.3}×D"
        );
        // RadiataPine is right at the 700 Janka softwood cutoff (710);
        // it falls into the medium-hardwood band by design — assert it
        // at least beats the dense bucket.
        assert!(
            pine_over_d >= 2.5,
            "radiata pine peck should be ≥ 2.5×D (sits at softwood/medium boundary), \
             got {pine_over_d:.3}×D"
        );
        // Softwood < 8×D ceiling.
        assert!(
            softwood_over_d <= 8.0,
            "softwood peck must stay ≤ 8×D (Onsrud/FPL upper band), got {softwood_over_d:.3}×D"
        );
        // Hardwood ceiling (matrix `peck_over_d_hardwood` invariant: 5×D).
        assert!(
            oak_over_d <= 5.0,
            "white oak peck must stay ≤ 5×D (matrix hardwood ceiling), got {oak_over_d:.3}×D"
        );
        assert!(
            ipe_over_d <= 5.0,
            "ipe peck must stay ≤ 5×D (matrix hardwood ceiling), got {ipe_over_d:.3}×D"
        );

        // Ordering: softwood > oak > ipe (matches density / Janka).
        assert!(
            softwood_over_d > oak_over_d,
            "softwood ({softwood_over_d:.3}×D) must exceed white oak ({oak_over_d:.3}×D)"
        );
        assert!(
            oak_over_d > ipe_over_d,
            "white oak ({oak_over_d:.3}×D) must exceed ipe ({ipe_over_d:.3}×D)"
        );
    }

    /// Bug 1 (2026-06-02 audit, workflow `w39ma2j1y`): adaptive ops
    /// use `adaptive_doc_factor` (deep+narrow), not the
    /// conventional `doc_roughing_factor`. For a 6 mm bit on a wood
    /// router with `doc_roughing_factor=0.20` and
    /// `adaptive_doc_factor=1.50`, an Adaptive3d op with DOC=6 mm
    /// must not be clamped. Pre-fix the clamp stripped it to 1.2 mm,
    /// erasing the upstream `adaptive_doc_factor` floor and turning
    /// adaptive into "shallow conventional".
    #[test]
    fn adaptive_op_uses_adaptive_doc_factor_not_doc_roughing_factor() {
        use crate::compute::operation_configs::Adaptive3dConfig;
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            stepover: 0.88,
            depth_per_pass: 6.0,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7):
        // at the new_default 45 mm stickout the deflection back-off now
        // binds and clamps DOC=6 → 3.84, masking the DOC-factor logic this
        // test isolates. Stiffen the tool (stickout 45 → 12 mm; δ ∝
        // stickout³ → ~0.019×) so deflection doesn't interfere and the
        // adaptive_doc_factor selection is the only thing under test.
        tool.stickout = 12.0;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.20; // conventional
        machine.rigidity.adaptive_doc_factor = 1.50; // adaptive can go deep

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &Material::default(),
            PassRole::Roughing,
            SuggestContext::default(),
        );

        // DOC=6 mm is under adaptive_doc_factor*D=9 mm and under
        // cutting_length=25 mm — should NOT clamp.
        assert_eq!(
            op.depth_per_pass(),
            Some(6.0),
            "Adaptive3d DOC=6 mm on 6 mm bit must not clamp (adaptive_doc_factor=1.5)"
        );
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::RoughingDepthClampedToRigidity { .. })),
            "Adaptive3d DOC=6 mm must not trigger RoughingDepthClampedToRigidity \
             (was Bug 1 — adaptive ops clamped by conventional factor)"
        );
    }

    /// v1.3 combined-Suggest (2026-06-03 design doc § v1.3): when the
    /// LUT + rigidity-factor path writes a DPP > 0.5×D on an Adaptive3d
    /// op with `entry_style = Plunge`, the plunge-entry transient
    /// breaches the deflection gate and the toolpath generates no
    /// cutting moves (Wanaka 3D Rough 6 failure mode). Suggest must
    /// emit `PlungeEntryUnstableAtDpp` *without* rewriting strategy
    /// params — auto-rewrite is v2.
    ///
    /// Two cases covered:
    /// - DPP=9 mm, D=6 mm (pre-v1.1 motivating failure, ratio 1.5)
    /// - DPP=3.69 mm, D=6 mm (post-v1.1 transient-spike calibration
    ///   point, ratio 0.615 — steady-state 162 µm but entry transient
    ///   peaks at 362 µm)
    #[test]
    fn plunge_entry_unstable_warning_fires_when_dpp_exceeds_half_diameter() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};

        let cases: &[(f64, &str)] = &[
            (9.0, "pre-v1.1 case: DPP=9 mm on 6 mm tool (ratio 1.5)"),
            (
                3.69,
                "post-v1.1 transient-spike case: DPP=3.69 mm on 6 mm tool (ratio 0.615)",
            ),
        ];

        for (dpp, label) in cases {
            let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
                feed_rate: 1500.0,
                plunge_rate: 500.0,
                stepover: 0.88,
                depth_per_pass: *dpp,
                entry_style: Adaptive3dEntryStyle::Plunge,
                ..Adaptive3dConfig::default()
            });
            let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
            tool.diameter = 6.0;
            tool.cutting_length = 25.0;
            // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7):
            // at the new_default 45 mm stickout the deflection back-off
            // now rewrites DPP, but this test asserts the v1.3 warning-only
            // contract (DPP must NOT be auto-rewritten — the warning fires
            // without a rewrite). Stiffen the tool (stickout 45 → 12 mm; δ
            // ∝ stickout³ → ~0.019×) so deflection doesn't force a DPP
            // rewrite and the warning-only behaviour is what's under test.
            tool.stickout = 12.0;
            let mut machine = MachineProfile::default();
            machine.rigidity.doc_roughing_factor = 0.20;
            machine.rigidity.adaptive_doc_factor = 1.50;

            let warnings = enforce_invariants(
                &mut op,
                &tool,
                &machine,
                &Material::default(),
                PassRole::Roughing,
                // v3.3c (StrategyAndFeeds default) auto-rewrites
                // entry_style and suppresses this warning. This test
                // asserts the v1.3 warning-only contract, so pin to
                // FeedsWithGates explicitly.
                SuggestContext {
                    policy: SuggestPolicy {
                        scope: SuggestScope::FeedsWithGates,
                        ..SuggestPolicy::default()
                    },
                    ..SuggestContext::default()
                },
            );

            // The DPP must not have been rewritten — v1.3 is warning-only.
            assert_eq!(
                op.depth_per_pass(),
                Some(*dpp),
                "v1.3 must not auto-rewrite DPP (warning-only) — {label}"
            );
            let hit = warnings.iter().any(|w| {
                matches!(
                    w,
                    SuggestWarning::PlungeEntryUnstableAtDpp { dpp_mm, diameter_mm, entry_style }
                        if (*dpp_mm - *dpp).abs() < 1e-9
                            && (*diameter_mm - 6.0).abs() < 1e-9
                            && entry_style == "plunge"
                )
            });
            assert!(
                hit,
                "Adaptive3d + plunge entry + DPP > 0.5×D must emit \
                 PlungeEntryUnstableAtDpp — {label}, got {warnings:?}"
            );
        }
    }

    /// v1.3 counter-test: DPP well below the 0.5×D threshold must NOT
    /// fire the warning. Guards the lower bound of the calibrated
    /// 2026-06-03 threshold (was 1.0×D, widened to 0.5×D after the
    /// Wanaka transient-spike data showed entry deflection breaching
    /// at ratio ≈ 0.615).
    #[test]
    fn plunge_entry_unstable_does_not_fire_below_half_diameter() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};

        // DPP=2.0 on a 6 mm tool → ratio 0.333, comfortably below the
        // 0.5×D trigger. Plunge entry, so the *only* reason the warning
        // wouldn't fire is the threshold — not the entry-style guard.
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: 1500.0,
            plunge_rate: 500.0,
            stepover: 0.88,
            depth_per_pass: 2.0,
            entry_style: Adaptive3dEntryStyle::Plunge,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.20;
        machine.rigidity.adaptive_doc_factor = 1.50;

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &Material::default(),
            PassRole::Roughing,
            SuggestContext::default(),
        );

        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::PlungeEntryUnstableAtDpp { .. })),
            "DPP=2.0 on 6 mm tool (ratio 0.333) is below the 0.5×D \
             trigger and must not fire PlungeEntryUnstableAtDpp, got {warnings:?}"
        );
    }

    /// v1.3 counter-test: a helix or ramp entry on the same DPP/tool
    /// combo must NOT fire the warning — only plunge entries are
    /// unstable at DPP > diameter. Guards against a too-broad firing
    /// rule that would scare users off the safe entry styles.
    #[test]
    fn plunge_entry_unstable_does_not_fire_for_helix_or_ramp() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        let mut machine = MachineProfile::default();
        machine.rigidity.adaptive_doc_factor = 1.50;

        for style in [Adaptive3dEntryStyle::Helix, Adaptive3dEntryStyle::Ramp] {
            let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
                feed_rate: 1500.0,
                plunge_rate: 500.0,
                stepover: 0.88,
                depth_per_pass: 9.0,
                entry_style: style,
                ..Adaptive3dConfig::default()
            });
            let warnings = enforce_invariants(
                &mut op,
                &tool,
                &machine,
                &Material::default(),
                PassRole::Roughing,
                SuggestContext::default(),
            );
            assert!(
                !warnings
                    .iter()
                    .any(|w| matches!(w, SuggestWarning::PlungeEntryUnstableAtDpp { .. })),
                "helix/ramp entry must not trigger PlungeEntryUnstableAtDpp, got {warnings:?} \
                 for style {style:?}"
            );
        }
    }

    /// Bug 1 counter-test: conventional Pocket op DOES still clamp to
    /// `doc_roughing_factor` — the fix is selective on feeds family,
    /// not a blanket relaxation.
    #[test]
    fn conventional_op_still_clamps_by_doc_roughing_factor() {
        let mut op = OperationConfig::Pocket(PocketConfig {
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            stepover: 3.0,
            depth_per_pass: 6.0,
            ..PocketConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7):
        // at the new_default 45 mm stickout the deflection back-off now
        // clamps DOC to 0.96, masking the doc_roughing_factor clamp (1.2)
        // this counter-test isolates. Stiffen the tool (stickout 45 →
        // 12 mm; δ ∝ stickout³ → ~0.019×) so the rigidity factor is the
        // binding clamp again.
        tool.stickout = 12.0;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.20;
        machine.rigidity.adaptive_doc_factor = 1.50;

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &Material::default(),
            PassRole::Roughing,
            SuggestContext::default(),
        );

        // Pocket on 6 mm bit: doc_roughing_factor*D = 1.2 mm cap.
        let dpp = op.depth_per_pass().expect("dpp set after clamp");
        assert!(
            (dpp - 1.2).abs() < 1e-6,
            "Pocket DOC must still clamp to doc_roughing_factor*D (1.2), got {dpp}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::RoughingDepthClampedToRigidity { .. })),
            "Pocket DOC over conventional ceiling must still warn"
        );
    }

    /// v1.1 combined-Suggest: deflection-aware DPP selection for the
    /// Wanaka Back Rough motivating case — 6 mm carbide endmill at 45 mm
    /// stickout in HardMaple, 9 mm commanded DPP. The Suggest pass must
    /// produce a *deflection-safe* DPP (predicted peak ≤ the 200 µm bound).
    ///
    /// Deflection-model reconciliation (2026-06-17): the closed-form
    /// `predict_peak_deflection_um` now delegates its cantilever to the
    /// same integrated two-section model the axial envelope
    /// (`pick_axial_envelope` → `invert_deflection`) uses. The two no
    /// longer disagree, which changes *which mechanism* does the clamping
    /// and dissolves the old back-off convergence problem:
    ///
    /// - The axial envelope finds the DPP where integrated δ = 200 µm
    ///   (~6.57 mm at WOC 1.2 mm) and clamps the 9 mm command to it in one
    ///   shot, emitting `AxialDocClampedByEnvelope { binding: "deflection" }`.
    /// - The back-off loop then evaluates the *same* integrated physics at
    ///   that DPP, sees it is already at/under the 200 µm bound, and does
    ///   nothing (0 iterations, no `DppCappedByDeflection`).
    ///
    /// Pre-reconciliation the closed-form read ~3× hotter than the
    /// envelope's bound, so the back-off chased a phantom target down to
    /// ~2.15 mm and still bottomed out at ~350 µm against its 5-iteration
    /// cap. The fix is the envelope and predictor agreeing — the DPP is
    /// chosen correctly once, not thrashed. This is the sentry for "the
    /// Suggest pass lands the wanaka rough deflection-safe in one pass."
    #[test]
    fn deflection_machinery_caps_dpp_for_long_reach_tool() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::material::WoodSpecies;
        // Deflection binds only on long/thin tools under the feed-aware
        // literature-absolute force model. Long-reach 6 mm carbide endmill,
        // 75 mm stickout, DPP 9 mm command, WOC 1.2 mm, feed 911 mm/min @
        // 16 kRPM, HardMaple — the 9 mm command predicts past the 200 µm
        // bound, so the axial-DOC envelope clamps it to the deflection-safe
        // DPP in one shot (no phantom back-off thrash).
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: 911.0,
            plunge_rate: 300.0,
            stepover: 1.2,
            depth_per_pass: 9.0,
            spindle_rpm: Some(16_000),
            // Helix entry so we don't also fire PlungeEntryUnstableAtDpp.
            entry_style: Adaptive3dEntryStyle::Helix,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        tool.stickout = 85.0;
        tool.flute_count = 2;
        let mut machine = MachineProfile::default();
        // Pin the rigidity factors so the rigidity clamp does NOT
        // pre-clamp DPP — we want to see the deflection machinery
        // applied to the 9 mm starting point directly.
        machine.rigidity.doc_roughing_factor = 0.20;
        machine.rigidity.adaptive_doc_factor = 1.60; // 1.6 × 6 = 9.6 > 9, no rigidity clamp
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext::default(),
        );

        let dpp_after = op.depth_per_pass().expect("dpp set");
        // DPP is clamped below the 9 mm command, to the deflection-safe
        // bound — in one shot, NOT thrashed down by a phantom-hot back-off.
        assert!(
            (4.0..9.0).contains(&dpp_after),
            "DPP must be clamped to the deflection-safe bound (below the 9 mm command, not over-cut), got {dpp_after} mm"
        );
        // The whole point: the resulting DPP is deflection-safe. Predicted
        // peak at the chosen DPP sits at/under the 200 µm bound (allow a
        // hair of binary-search tolerance).
        let predicted_after =
            crate::feeds::predict::predict_peak_deflection_um(&op, &tool, &material, &machine)
                .predicted_um;
        assert!(
            predicted_after <= 205.0,
            "Suggest must land the wanaka rough deflection-safe (≤ 200 µm bound), \
             got {predicted_after:.1} µm at DPP={dpp_after:.2} mm"
        );
        // And it genuinely backed off from the command: the 9 mm command
        // predicts well over the bound.
        let predicted_at_command = {
            let mut probe = op.clone();
            probe.set_depth_per_pass(9.0);
            crate::feeds::predict::predict_peak_deflection_um(&probe, &tool, &material, &machine)
                .predicted_um
        };
        assert!(
            predicted_at_command > DEFLECTION_BACKOFF_TARGET_UM,
            "the 9 mm command must exceed the 200 µm bound (otherwise nothing to clamp), got {predicted_at_command:.1} µm"
        );
        // The axial envelope is the mechanism that clamps it, bound by
        // deflection. (The back-off loop is now a confirming no-op since
        // it shares the envelope's physics — so we assert the envelope
        // warning, not `DppCappedByDeflection`.)
        let clamp = warnings.iter().find_map(|w| match w {
            SuggestWarning::AxialDocClampedByEnvelope {
                clamped_mm,
                binding,
                ..
            } => Some((*clamped_mm, *binding)),
            _ => None,
        });
        let (clamped_mm, binding) =
            clamp.expect("AxialDocClampedByEnvelope must fire on the wanaka case");
        assert_eq!(
            binding, "deflection",
            "the binding constraint must be deflection, got {binding}"
        );
        assert!(
            (clamped_mm - dpp_after).abs() < 1e-9,
            "envelope clamp value must match the post-Suggest DPP, got {clamped_mm} vs {dpp_after}"
        );
    }

    /// v1.1 step 2 counter-test: a Finishing pass on the same
    /// tool/material does NOT trigger the back-off — deflection mostly
    /// threatens roughing engagement, and finishing passes already
    /// run shallow DPPs by design. Locking back-off to roughing keeps
    /// the suggest path symmetric with the existing
    /// `RoughingDepthClampedToRigidity` gate.
    #[test]
    fn deflection_back_off_skipped_for_finish_pass() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::material::WoodSpecies;
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: 911.0,
            plunge_rate: 300.0,
            stepover: 1.2,
            depth_per_pass: 9.0,
            spindle_rpm: Some(16_000),
            entry_style: Adaptive3dEntryStyle::Helix,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7):
        // the deflection force is now ~2.7× higher. At the original 45 mm
        // stickout a 9 mm DPP exceeds the *role-agnostic* axial-envelope
        // deflection ceiling (pick_axial_envelope, runs for all roles)
        // and trims DPP to ~6.57 — NOT via the roughing-only back-off
        // (backoff_dpp_for_deflection / DppCappedByDeflection), which
        // this test verifies is skipped for finish. To keep that the only
        // thing under test, stiffen the tool (stickout 45 → 18 mm; δ ∝
        // stickout³ → ~0.064×) so 9 mm sits inside the deflection
        // envelope and the DPP stays 9.0 untouched. Confirmed: the
        // back-off path remains correctly roughing-only — this is the
        // separate cutter-geometry envelope, not a finish-path
        // regression.
        tool.stickout = 18.0;
        tool.flute_count = 2;
        let machine = MachineProfile::default();
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Finish,
            SuggestContext::default(),
        );

        assert_eq!(
            op.depth_per_pass(),
            Some(9.0),
            "Finishing pass must not trigger deflection back-off (back-off is roughing-only)"
        );
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::DppCappedByDeflection { .. })),
            "Finishing pass must not emit DppCappedByDeflection, got {warnings:?}"
        );
    }

    /// SuggestContext plumbing invariant (2026-06-04 refactor): the
    /// gate-aware context plumbed onto `SuggestParamsInput` /
    /// `SuggestForOperationInput` is *carried* by v1.1 but not yet
    /// *consumed*. A populated context must therefore produce
    /// bit-identical Suggest output to `SuggestContext::default()` —
    /// guards against an accidental v1.1 read of the v1.2 slots.
    #[test]
    fn suggest_context_is_no_op_in_v1_1() {
        let stock = stock_ctx();
        // Synthetic bbox: 200 × 150 × 12 mm, origin at zero. Realistic
        // model envelope but no path-dependence so the test stays
        // hermetic.
        let bbox = crate::geo::BoundingBox3 {
            min: crate::geo::P3::new(0.0, 0.0, -12.0),
            max: crate::geo::P3::new(200.0, 150.0, 0.0),
        };
        let populated = SuggestContext {
            model_bbox: Some(&bbox),
            stock: Some(&stock),
            upstream_leftover_stock_mm: Some(1.2),
            neighboring_strategy_hint: Some("adaptive3d"),
            chipload_bounds: None,
            matched_lut_row: None,
            effective_diameter_mm: 0.0,
            calculator_operating_point: None,
            policy: SuggestPolicy::default(),
        };

        let baseline = suggest_params(SuggestParamsInput {
            op_type: OperationType::Pocket,
            tool: &tool(6.35),
            machine: &MachineProfile::default(),
            material: &Material::default(),
            workholding: WorkholdingRigidity::Medium,
            lut: &EMBEDDED_LUT,
            stock_ctx: &stock,
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            context: SuggestContext::default(),
        })
        .expect("pocket + flat is not a refused combination");
        let with_ctx = suggest_params(SuggestParamsInput {
            op_type: OperationType::Pocket,
            tool: &tool(6.35),
            machine: &MachineProfile::default(),
            material: &Material::default(),
            workholding: WorkholdingRigidity::Medium,
            lut: &EMBEDDED_LUT,
            stock_ctx: &stock,
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            context: populated,
        })
        .expect("pocket + flat is not a refused combination");

        assert_eq!(
            baseline.feeds_result.feed_rate_mm_min, with_ctx.feeds_result.feed_rate_mm_min,
            "populated SuggestContext must not perturb feed_rate in v1.1"
        );
        assert_eq!(
            baseline.feeds_result.plunge_rate_mm_min, with_ctx.feeds_result.plunge_rate_mm_min,
            "populated SuggestContext must not perturb plunge_rate in v1.1"
        );
        assert_eq!(
            baseline.feeds_result.radial_width_mm, with_ctx.feeds_result.radial_width_mm,
            "populated SuggestContext must not perturb stepover in v1.1"
        );
        assert_eq!(
            baseline.feeds_result.axial_depth_mm, with_ctx.feeds_result.axial_depth_mm,
            "populated SuggestContext must not perturb DPP in v1.1"
        );
        assert_eq!(
            baseline.feeds_result.rpm, with_ctx.feeds_result.rpm,
            "populated SuggestContext must not perturb RPM in v1.1"
        );
        assert_eq!(
            baseline.operation.feed_rate(),
            with_ctx.operation.feed_rate(),
            "operation feed_rate must match between context variants in v1.1"
        );
        assert_eq!(
            baseline.operation.depth_per_pass(),
            with_ctx.operation.depth_per_pass(),
            "operation DPP must match between context variants in v1.1"
        );
    }

    /// v1.1 step 2 floor: the loop must terminate at
    /// `DEFLECTION_BACKOFF_DPP_FLOOR_MM` even if the predicted
    /// deflection at that depth still exceeds the 200 µm target.
    /// Synthetic case: unrealistically long stickout (150 mm) on a
    /// 2 mm endmill in HardMaple — δ ∝ L³/d⁴ keeps prediction huge
    /// even at 0.5 mm DPP (the loop's floor). Pre-back-off DPP set to
    /// 1.0 mm so the 0.8-factor reduction reaches the floor in 3-4
    /// iterations rather than exhausting the 5-iteration max first.
    #[test]
    fn deflection_back_off_respects_05mm_floor() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::material::WoodSpecies;
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: 1500.0,
            plunge_rate: 500.0,
            stepover: 1.0,
            // Start at 1.0 mm so 0.8^N drops to 0.5 in <5 iterations
            // (log(0.5/1.0)/log(0.8) ≈ 3.1 steps). Otherwise the
            // 5-iteration cap would dominate over the floor and we'd
            // exit on iterations, not on the floor we're trying to
            // exercise.
            depth_per_pass: 1.0,
            spindle_rpm: Some(16_000),
            entry_style: Adaptive3dEntryStyle::Helix,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        // Very small core diameter (2 mm → core 1.4 mm, I ≈ 0.19 mm⁴)
        // combined with extreme stickout drives δ ∝ L³/d⁴ above the
        // 200 µm threshold even at the 0.5 mm DPP floor.
        tool.diameter = 2.0;
        tool.cutting_length = 25.0;
        tool.stickout = 220.0;
        tool.flute_count = 2;
        let mut machine = MachineProfile::default();
        // Disable the rigidity clamp so 1.0 mm starting DPP survives
        // to the back-off loop (1 < 2 × 0.20 = 0.4 would otherwise
        // already clamp). The loop is what we're testing.
        machine.rigidity.doc_roughing_factor = 5.0;
        machine.rigidity.adaptive_doc_factor = 5.0;
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext::default(),
        );

        // Phase 3 (Pass 0, axial-DOC envelope): the cutter-axial-constraints
        // calculator runs the canonical deflection model via binary search,
        // so for the same pathological inputs it now clamps DPP **below**
        // the v1.1 back-off floor — the envelope is the more accurate
        // constraint and it binds first. The v1.1 back-off then short-
        // circuits because `current > FLOOR` is already false. Test asserts
        // the new behaviour: envelope-clamped DPP < FLOOR + AxialDocClampedByEnvelope
        // warning carries the deflection-binding signal.
        let dpp_after = op.depth_per_pass().expect("dpp set");
        assert!(
            dpp_after < DEFLECTION_BACKOFF_DPP_FLOOR_MM,
            "Phase 3 envelope must clamp DPP below the v1.1 back-off floor on \
             pathological tools (2 mm × 150 mm stickout); got {dpp_after}"
        );
        let envelope_warning_present = warnings.iter().any(|w| {
            matches!(
                w,
                SuggestWarning::AxialDocClampedByEnvelope {
                    binding: "deflection",
                    ..
                } | SuggestWarning::AxialEnvelopeSafeBandEmpty { .. }
            )
        });
        assert!(
            envelope_warning_present,
            "Phase 3 envelope must surface an Axial* warning when it clamps DPP, got {warnings:?}"
        );
    }

    /// v1.2 combined-Suggest (2026-06-04 design doc § v1.2): Wanaka 3D
    /// Finish motivating case. A 2 mm-tip tapered-ball on a 140 × 150 mm
    /// stock with a 0.03 mm stepover (scallop-height math taken
    /// literally for a small tip radius) predicts millions of moves on
    /// the model envelope. The back-off must raise stepover until the
    /// predicted move count clears 500 k, and emit
    /// `StepoverRaisedForRuntime`.
    ///
    /// The test calls `enforce_invariants` directly with a pre-written
    /// 0.03 mm stepover (the LUT scallop path that produced it isn't
    /// re-exercised here — `predict::tests::predict_move_count_zero_for_v_carve`
    /// guards the predictor itself and the LUT/scallop path is already
    /// covered by `scallop_height_some_overrides_default_ae_factor`).
    #[test]
    fn stepover_raised_for_wanaka_3d_finish_case() {
        use crate::compute::operation_configs::DropCutterConfig;
        let mut op = OperationConfig::DropCutter(DropCutterConfig {
            stepover: 0.03,
            feed_rate: 2000.0,
            plunge_rate: 500.0,
            scallop_height: Some(0.010),
            ..DropCutterConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
        tool.diameter = 4.0; // tip diameter 4 → tip radius 2 mm
        tool.cutting_length = 25.0;
        // Wanaka envelope: 140 × 150 × 12 mm. Even at the diameter ×
        // 0.5 = 2 mm ceiling the predicted moves = (150/2) × (140/0.4)
        // ≈ 26 250 — well below 500 k, so the loop should converge
        // before bailing on the ceiling.
        let bbox = crate::geo::BoundingBox3 {
            min: crate::geo::P3::new(0.0, 0.0, -12.0),
            max: crate::geo::P3::new(140.0, 150.0, 0.0),
        };
        let machine = MachineProfile::default();
        let material = Material::default();
        let context = SuggestContext {
            model_bbox: Some(&bbox),
            ..SuggestContext::default()
        };

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Finish,
            context,
        );

        let stepover_after = op.stepover().expect("stepover must remain set");
        assert!(
            stepover_after > 0.03,
            "back-off must raise stepover above the 0.03 mm starting point, got {stepover_after}"
        );
        assert!(
            stepover_after <= tool.diameter * STEPOVER_BACKOFF_DIAMETER_FRACTION + 1e-9,
            "raised stepover must stay at or below tool.diameter × 0.5 = {} mm, got {stepover_after}",
            tool.diameter * STEPOVER_BACKOFF_DIAMETER_FRACTION
        );
        // The final predicted move count must come down below the
        // 500 k target (the back-off converged before the ceiling).
        let moves_after = crate::feeds::predict::predict_move_count(&op, Some(&bbox), &tool);
        assert!(
            moves_after <= STEPOVER_BACKOFF_TARGET_MOVES,
            "post-back-off move count must clear {STEPOVER_BACKOFF_TARGET_MOVES}, got {moves_after}"
        );

        let warning = warnings.iter().find_map(|w| match w {
            SuggestWarning::StepoverRaisedForRuntime {
                requested_mm,
                raised_mm,
                predicted_moves_at_requested,
                predicted_moves_at_raised,
                iterations,
            } => Some((
                *requested_mm,
                *raised_mm,
                *predicted_moves_at_requested,
                *predicted_moves_at_raised,
                *iterations,
            )),
            _ => None,
        });
        let (
            requested_mm,
            raised_mm,
            predicted_moves_at_requested,
            predicted_moves_at_raised,
            iterations,
        ) = warning.expect("StepoverRaisedForRuntime must fire on Wanaka 3D Finish case");
        assert!(
            (requested_mm - 0.03).abs() < 1e-6,
            "warning.requested_mm must capture the pre-back-off stepover, got {requested_mm}"
        );
        assert!(
            (raised_mm - stepover_after).abs() < 1e-9,
            "warning.raised_mm must match the post-back-off stepover, got {raised_mm} vs {stepover_after}"
        );
        assert!(
            predicted_moves_at_requested > STEPOVER_BACKOFF_TARGET_MOVES,
            "warning.predicted_moves_at_requested must exceed target (loop wouldn't have started otherwise), got {predicted_moves_at_requested}"
        );
        assert!(
            predicted_moves_at_raised <= STEPOVER_BACKOFF_TARGET_MOVES,
            "warning.predicted_moves_at_raised must clear target (no ceiling bail expected), got {predicted_moves_at_raised}"
        );
        assert!(
            (1..=STEPOVER_BACKOFF_MAX_ITERATIONS).contains(&iterations),
            "iterations must fall in [1, {}], got {iterations}",
            STEPOVER_BACKOFF_MAX_ITERATIONS
        );
    }

    /// v1.2 counter-test: when `context.model_bbox` is `None` the
    /// predictor returns 0 ("no constraint signal") and the back-off
    /// must short-circuit. Guards against the predictor being read
    /// pessimistically (treating None as "infinite envelope") and
    /// against `enforce_invariants` raising stepover when it has no
    /// envelope information to gate on.
    #[test]
    fn stepover_unchanged_when_model_bbox_unknown() {
        use crate::compute::operation_configs::DropCutterConfig;
        let mut op = OperationConfig::DropCutter(DropCutterConfig {
            stepover: 0.03,
            feed_rate: 2000.0,
            plunge_rate: 500.0,
            scallop_height: Some(0.010),
            ..DropCutterConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
        tool.diameter = 4.0;
        tool.cutting_length = 25.0;
        let machine = MachineProfile::default();
        let material = Material::default();
        // No model_bbox — the back-off must short-circuit on the
        // predictor's 0 return.
        let context = SuggestContext::default();

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Finish,
            context,
        );

        assert_eq!(
            op.stepover(),
            Some(0.03),
            "stepover must be left untouched when model_bbox is None (no constraint signal)"
        );
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::StepoverRaisedForRuntime { .. })),
            "StepoverRaisedForRuntime must not fire without a model_bbox, got {warnings:?}"
        );
    }

    /// v1.2 floor counter-test: when even raising stepover to
    /// `tool.diameter × 0.5` would not bring the predicted move count
    /// below the target, the loop must land on the diameter-fraction
    /// ceiling and emit the warning (the `predicted_moves_at_raised`
    /// field still exceeds the target — that's the "unmet gate" signal
    /// until the v2 surface adds a structured unmet-condition channel).
    ///
    /// Synthetic case: enormous stock envelope (5 × 5 m DropCutter scan)
    /// on a small-tip tapered-ball. At the 0.5 × D ceiling the predicted
    /// move count is still in the millions.
    #[test]
    fn stepover_floor_caps_back_off() {
        use crate::compute::operation_configs::DropCutterConfig;
        // Start at a stepover where 1.5⁵ growth reaches the
        // diameter-fraction ceiling before the iteration cap fires:
        // tool.diameter=2.0 → ceiling=1.0. Starting at 0.3 mm,
        // 0.3 × 1.5² = 0.675 (iter 2), × 1.5³ = 1.0125 (clamped to
        // ceiling) on iter 3 — bails on ceiling, not on max iterations.
        let mut op = OperationConfig::DropCutter(DropCutterConfig {
            stepover: 0.3,
            feed_rate: 2000.0,
            plunge_rate: 500.0,
            scallop_height: Some(0.010),
            ..DropCutterConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
        tool.diameter = 2.0; // tip diameter 2 → ceiling = 1.0 mm
        tool.cutting_length = 25.0;
        // 5 000 × 5 000 mm envelope. At ceiling 1.0 mm:
        //   passes = 5000 / 1.0 = 5000
        //   moves_per_pass = 5000 / (2.0 × 0.1) = 25 000
        //   total ≈ 1.25 × 10⁸ — well above 500 k.
        let bbox = crate::geo::BoundingBox3 {
            min: crate::geo::P3::new(0.0, 0.0, -10.0),
            max: crate::geo::P3::new(5000.0, 5000.0, 0.0),
        };
        let machine = MachineProfile::default();
        let material = Material::default();
        let context = SuggestContext {
            model_bbox: Some(&bbox),
            ..SuggestContext::default()
        };

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Finish,
            context,
        );

        let stepover_after = op.stepover().expect("stepover must be set");
        let ceiling = tool.diameter * STEPOVER_BACKOFF_DIAMETER_FRACTION;
        assert!(
            (stepover_after - ceiling).abs() < 1e-6,
            "back-off must land on the ceiling ({ceiling} mm) when target is unreachable, got {stepover_after}"
        );
        let warning = warnings
            .iter()
            .find_map(|w| match w {
                SuggestWarning::StepoverRaisedForRuntime {
                    raised_mm,
                    predicted_moves_at_raised,
                    iterations,
                    ..
                } => Some((*raised_mm, *predicted_moves_at_raised, *iterations)),
                _ => None,
            })
            .expect("warning must still fire when bailing on the diameter-fraction ceiling");
        let (raised_mm, predicted_moves_at_raised, iterations) = warning;
        assert!(
            (raised_mm - ceiling).abs() < 1e-6,
            "warning.raised_mm must equal the ceiling when loop bails, got {raised_mm}"
        );
        assert!(
            predicted_moves_at_raised > STEPOVER_BACKOFF_TARGET_MOVES,
            "warning.predicted_moves_at_raised must still exceed target to signal an unmet gate, got {predicted_moves_at_raised}"
        );
        assert!(
            iterations < STEPOVER_BACKOFF_MAX_ITERATIONS,
            "loop must bail on ceiling, not on max-iterations cap (iterations={iterations}, max={STEPOVER_BACKOFF_MAX_ITERATIONS})"
        );
    }

    /// **RE-BASELINED 2026-08-13 — Checkpoint J-1 (was
    /// `feed_recalibration_raises_feed_for_wanaka_back_rough_case`).**
    ///
    /// This was the pass-8 sentry: it required the arc-fit feed-up to fire
    /// on the Wanaka Back Rough motivating case and pinned the closed-form
    /// solve `target / arc_fit_ratio × rpm × flutes = 0.027 / 0.25 × 16000
    /// × 2` at **3456 mm/min**, up from the calculator's 911 mm/min — a
    /// 3.79× lift, with `FeedRaisedForChipload { cap_hit: None }`.
    ///
    /// Pass 8 is retired. The assertion inverts: the same input must now
    /// leave the feed where the calculator put it and emit no lift warning.
    /// Both pre-fix numbers are kept above so the inversion is readable as
    /// a movement, not a rewrite.
    #[test]
    fn retired_lift_leaves_wanaka_back_rough_feed_untouched() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::feeds::ChiploadBounds;
        use crate::material::WoodSpecies;

        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: 911.0,
            plunge_rate: 300.0,
            stepover: 1.2,
            // Post-v1.1-back-off DPP. Helix entry so we don't also
            // fire PlungeEntryUnstableAtDpp.
            depth_per_pass: 3.69,
            spindle_rpm: Some(16_000),
            entry_style: Adaptive3dEntryStyle::Helix,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        // Stiffened (45 → 18 mm) by the pre-fix version so deflection left
        // the feed-up loop headroom. Kept so the fixture is bit-identical
        // to the one that produced 3456 mm/min.
        tool.stickout = 18.0;
        tool.flute_count = 2;
        let mut machine = MachineProfile::default();
        // Disable the rigidity clamp at DPP=3.69 (3.69 < 1.6 × 6 = 9.6 is OK).
        machine.rigidity.doc_roughing_factor = 0.20;
        machine.rigidity.adaptive_doc_factor = 1.60;
        machine.max_feed_mm_min = 10_000.0;
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let bounds = Some(ChiploadBounds {
            min_mm_per_tooth: 0.027,
            max_mm_per_tooth: 0.060,
        });

        let initial_feed = op.feed_rate();
        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext {
                chipload_bounds: bounds,
                policy: SuggestPolicy {
                    aggressiveness: SuggestAggressiveness::Conservative,
                    ..SuggestPolicy::default()
                },
                ..SuggestContext::default()
            },
        );

        assert!(
            (op.feed_rate() - initial_feed).abs() < 1e-6,
            "retired pass 8 must leave feed at the calculator value {initial_feed} \
             (pre-fix it was lifted to 3456), got {}",
            op.feed_rate()
        );
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::FeedRaisedForChipload { .. })),
            "no Suggest pass constructs FeedRaisedForChipload since 2026-08-13, got {warnings:?}"
        );
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::ChiploadStillLowAfterRecalibration { .. })),
            "no Suggest pass constructs ChiploadStillLowAfterRecalibration since 2026-08-13, \
             got {warnings:?}"
        );
    }
    /// **RE-BASELINED 2026-08-13 — Checkpoint J-1 (was
    /// `feed_recalibration_caps_on_deflection` and
    /// `speed_gated_by_deflection_fires`, merged).**
    ///
    /// Both tests exercised a *cap* of the retired pass 8. Pre-fix, a
    /// 100 mm-stickout Ø6 2F endmill put the closed-form predictor's
    /// deflection in the (190, 200) µm refusal window, so the pass wrote
    /// the solved feed, the verify tripped, and it reverted 911 → 911 while
    /// still emitting `FeedRaisedForChipload { cap_hit:
    /// Some(DeflectionThreshold) }` **plus**
    /// `ChiploadStillLowAfterRecalibration { blocking_cap:
    /// DeflectionThreshold }`. Under `SuggestAggressiveness::Speed` the
    /// same fixture pinned `lut_target_mm_per_tooth` at the band max
    /// (0.10), proving Speed's target was what got gated; its uncapped
    /// solve would have been `0.10 / 0.25 × 16000 × 2 = 12 800 mm/min`.
    ///
    /// With the pass retired there is no solve, no verify and no cap. Both
    /// aggressiveness levels must now leave the feed untouched and emit
    /// neither warning. The two fixtures are kept bit-identical and run as
    /// one test because the only remaining difference between them is the
    /// policy field.
    ///
    /// **Note what this test does NOT claim.** A 100 mm-stickout Ø6
    /// endmill at 190+ µm predicted deflection is still a bad operating
    /// point; what changed is that Suggest no longer *raises feed into* it
    /// and then congratulates itself for refusing. The v1.1 DPP back-off
    /// against `DEFLECTION_BACKOFF_TARGET_UM` is untouched by this commit.
    #[test]
    fn retired_lift_fires_no_cap_on_a_deflection_bound_fixture() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::feeds::ChiploadBounds;
        use crate::material::WoodSpecies;

        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        // Band max 0.10 is what the pre-fix Speed arm pinned.
        let bounds = Some(ChiploadBounds {
            min_mm_per_tooth: 0.05,
            max_mm_per_tooth: 0.10,
        });
        let initial_feed = 911.0_f64;

        for aggressiveness in [SuggestAggressiveness::Default, SuggestAggressiveness::Speed] {
            let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
                feed_rate: initial_feed,
                plunge_rate: 300.0,
                stepover: 1.2,
                depth_per_pass: 3.69,
                spindle_rpm: Some(16_000),
                entry_style: Adaptive3dEntryStyle::Helix,
                ..Adaptive3dConfig::default()
            });
            let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
            tool.diameter = 6.0;
            tool.cutting_length = 25.0;
            // δ ∝ stickout³ — this is the value that landed the pre-fix
            // prediction in the (190, 200) µm window. Retune if the force
            // physics shifts; the setup guard below asserts the window.
            tool.stickout = 100.0;
            tool.flute_count = 2;
            let mut machine = MachineProfile::default();
            machine.rigidity.doc_roughing_factor = 0.20;
            machine.rigidity.adaptive_doc_factor = 1.60;
            machine.max_feed_mm_min = 20_000.0;

            // The fixture's premise, asserted rather than assumed: the
            // operating point really is deflection-bound. Without this the
            // "no cap fires" assertion below would be vacuous — a fixture
            // that was never near the cap proves nothing about its removal.
            let pre_predicted =
                crate::feeds::predict::predict_peak_deflection_um(&op, &tool, &material, &machine)
                    .predicted_um;
            assert!(
                (190.0..DEFLECTION_BACKOFF_TARGET_UM).contains(&pre_predicted),
                "test setup: predicted deflection ({pre_predicted:.1} µm) must sit in \
                 [190, {DEFLECTION_BACKOFF_TARGET_UM:.0}) µm — retune tool.stickout"
            );

            let warnings = enforce_invariants(
                &mut op,
                &tool,
                &machine,
                &material,
                PassRole::Roughing,
                SuggestContext {
                    chipload_bounds: bounds,
                    policy: SuggestPolicy {
                        aggressiveness,
                        ..SuggestPolicy::default()
                    },
                    ..SuggestContext::default()
                },
            );

            assert!(
                (op.feed_rate() - initial_feed).abs() < 1e-6,
                "{aggressiveness:?}: feed must stay at the calculator value {initial_feed}, got {}",
                op.feed_rate()
            );
            assert!(
                !warnings.iter().any(|w| matches!(
                    w,
                    SuggestWarning::FeedRaisedForChipload { .. }
                        | SuggestWarning::ChiploadStillLowAfterRecalibration { .. }
                )),
                "{aggressiveness:?}: no Suggest pass constructs the retired chipload-lift \
                 warnings since 2026-08-13, got {warnings:?}"
            );
        }
    }

    /// **RE-BASELINED 2026-08-13 — Checkpoint J-1 (was
    /// `feed_recalibration_caps_on_max_feed`).**
    ///
    /// Pre-fix: feed already at the 4000 mm/min machine cap, band
    /// 0.10–0.20, so pass 8's closed-form target `0.10 / 0.25 × 16000 × 2
    /// = 12 800 mm/min` clamped straight back down to 4000. Feed did not
    /// move, so `FeedRaisedForChipload` stayed silent, but
    /// `ChiploadStillLowAfterRecalibration { blocking_cap: MaxFeed }`
    /// fired — the pass reporting that it could not reach a target it had
    /// no business aiming at.
    ///
    /// Post-retirement the still-low warning must go silent too. The
    /// `MaxFeed` clamp itself is not what was retired: the calculator and
    /// `machine.cutting_feed_ceiling_mm_min()` still bound feed, and this
    /// test asserts the feed stays at 4000 for that reason.
    #[test]
    fn retired_lift_reports_no_max_feed_shortfall() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::feeds::ChiploadBounds;
        use crate::material::WoodSpecies;

        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: 4000.0,
            plunge_rate: 300.0,
            stepover: 1.2,
            depth_per_pass: 2.0,
            spindle_rpm: Some(16_000),
            entry_style: Adaptive3dEntryStyle::Helix,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        tool.stickout = 25.0;
        tool.flute_count = 2;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.20;
        machine.rigidity.adaptive_doc_factor = 1.60;
        machine.max_feed_mm_min = 4000.0;
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        // Band deliberately far above the operating point — pre-fix this is
        // what guaranteed the retired pass entered and then reported a
        // shortfall. It is kept so the inversion is measured on the fixture
        // that produced the shortfall, not on a comfortable one.
        let bounds = Some(ChiploadBounds {
            min_mm_per_tooth: 0.10,
            max_mm_per_tooth: 0.20,
        });

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext {
                chipload_bounds: bounds,
                ..SuggestContext::default()
            },
        );

        assert!(
            (op.feed_rate() - 4000.0).abs() < 1e-6,
            "feed must stay at the machine cap, got {}",
            op.feed_rate()
        );
        assert!(
            !warnings.iter().any(|w| matches!(
                w,
                SuggestWarning::ChiploadStillLowAfterRecalibration { .. }
                    | SuggestWarning::FeedRaisedForChipload { .. }
            )),
            "the retired pass's shortfall report must be silent, got {warnings:?}"
        );
    }

    /// v2 step 2 counter-test: when the predicted observed chipload
    /// already sits at or above the LUT minimum, the recalibration
    /// loop must skip entirely — no feed change, no warning. Guards
    /// against accidentally firing the loop on every Suggest path.
    ///
    /// **2026-08-13, Checkpoint J-1: this is the only one of the five
    /// pass-8 tests that SURVIVES UNCHANGED — it is now the general
    /// case.** Its assertions ("feed untouched, neither warning fires")
    /// used to describe the narrow in-band corner; with the pass retired
    /// they describe every Suggest path. Not one character of the fixture
    /// or the assertions moved, which makes it the cleanest single piece of
    /// evidence that the retirement generalised rather than inverted the
    /// contract. The comment below about arc-fit ratio 0.25 is left
    /// standing as a record of why these particular constants were chosen.
    #[test]
    fn feed_recalibration_skipped_when_already_in_band() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::feeds::ChiploadBounds;
        use crate::material::WoodSpecies;

        // Adaptive3d arc-fit ratio is 0.25 (Calibrated). To put the
        // observed median at or above 0.027 mm/tooth we need nominal
        // ≥ 0.108 mm/tooth → at 16 kRPM × 2 flutes that's feed ≥ 3456
        // mm/min. Use 4000 to be safely above.
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: 4000.0,
            plunge_rate: 500.0,
            stepover: 1.2,
            depth_per_pass: 2.0,
            spindle_rpm: Some(16_000),
            entry_style: Adaptive3dEntryStyle::Helix,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        tool.stickout = 25.0;
        tool.flute_count = 2;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.20;
        machine.rigidity.adaptive_doc_factor = 1.60;
        machine.max_feed_mm_min = 10_000.0;
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let bounds = Some(ChiploadBounds {
            min_mm_per_tooth: 0.025,
            max_mm_per_tooth: 0.050,
        });

        let initial_feed = op.feed_rate();
        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext {
                chipload_bounds: bounds,
                // v3.0c: pin to Conservative so "already in band" means
                // "above LUT min" — the v2.1 semantics the test was
                // written against. Under the new Default (median)
                // target this initial feed would still fall below band
                // midpoint and the loop would fire.
                policy: SuggestPolicy {
                    aggressiveness: SuggestAggressiveness::Conservative,
                    ..SuggestPolicy::default()
                },
                ..SuggestContext::default()
            },
        );

        assert!(
            (op.feed_rate() - initial_feed).abs() < 1e-6,
            "feed must be unchanged when observed chipload already in band, got {} vs {initial_feed}",
            op.feed_rate()
        );
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::FeedRaisedForChipload { .. })),
            "FeedRaisedForChipload must not fire when initial observed ≥ LUT min, got {warnings:?}"
        );
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::ChiploadStillLowAfterRecalibration { .. })),
            "ChiploadStillLowAfterRecalibration must not fire when initial observed ≥ LUT min, got {warnings:?}"
        );
    }

    /// **RE-BASELINED 2026-08-13 — Checkpoint J-1.**
    ///
    /// **This site was NOT in the A-5 evidence package's §6.1 re-baseline
    /// list** (which named four in-crate tests); it is a fifth, found by
    /// compiling. Recorded here rather than quietly fixed.
    ///
    /// v3.0b: same op × tool × material × machine with three
    /// SuggestAggressiveness levels. Pre-fix it asserted feed was
    /// *monotonically non-decreasing* Conservative → Default → Speed and
    /// pinned Conservative at the v2.1 closed-form solve **3456 mm/min**
    /// (`0.027 / 0.25 × 16000 × 2`), with Default and Speed above it.
    ///
    /// `SuggestAggressiveness` reached the feed **only** through pass 8's
    /// `target_chipload` solve. With that pass retired, aggressiveness no
    /// longer moves feed at all, so the progression collapses from strictly
    /// spread to flat. Monotonicity is technically preserved (equality
    /// satisfies `<=`), which is exactly why asserting it alone would be a
    /// vacuous pass — the test now asserts the stronger, true statement:
    /// **all three levels return the calculator's own feed.**
    ///
    /// This is a real loss of operator control and is named as such: the
    /// aggressiveness dial is inert on the pre-simulation path until a
    /// simulation-backed target lands (Checkpoint J-1's destination (c)).
    /// It is not inert on the rest of `feeds` — the dial still selects the
    /// target the rationale and the chipload envelopes report against.
    ///
    /// Scaffolded against the Wanaka Back Rough operating point used by
    /// `feed_recalibration_raises_feed_for_wanaka_back_rough_case`
    /// (6 mm carbide endmill, HardMaple, Adaptive3d, DPP 9 mm /
    /// stepover 1.2 mm / feed 911 mm/min @ 16 kRPM, ChiploadBounds
    /// 0.027..0.060). At Conservative the closed-form solve targets
    /// the band min; at Default it targets the midpoint; at Speed it
    /// targets the band max. Speed may trip the deflection refusal /
    /// MaxFeed cap (we allow either as long as the feed is
    /// non-decreasing relative to Default).
    #[test]
    fn aggressiveness_monotone_feed_progression() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::feeds::ChiploadBounds;
        use crate::material::WoodSpecies;

        let bounds = Some(ChiploadBounds {
            min_mm_per_tooth: 0.027,
            max_mm_per_tooth: 0.060,
        });
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };

        let feed_for = |aggressiveness: SuggestAggressiveness| -> f64 {
            let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
                feed_rate: 911.0,
                plunge_rate: 300.0,
                stepover: 1.2,
                depth_per_pass: 3.69,
                spindle_rpm: Some(16_000),
                entry_style: Adaptive3dEntryStyle::Helix,
                ..Adaptive3dConfig::default()
            });
            let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
            tool.diameter = 6.0;
            tool.cutting_length = 25.0;
            // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7):
            // at the original 45 mm stickout the deflection ceiling now
            // binds at the baseline feed, so the closed-form feed-up can't
            // reach the Conservative 3456 mm/min target this test asserts
            // and the monotone progression collapses. This test verifies
            // the aggressiveness→target→feed math, not the deflection cap;
            // stiffen the tool (stickout 45 → 18 mm; δ ∝ stickout³ →
            // ~0.064×) so deflection leaves headroom and the three
            // aggressiveness levels can spread out monotonically.
            tool.stickout = 18.0;
            tool.flute_count = 2;
            let mut machine = MachineProfile::default();
            machine.rigidity.doc_roughing_factor = 0.20;
            machine.rigidity.adaptive_doc_factor = 1.60;
            // Generous feed cap so Speed has room to climb without
            // immediately tripping MaxFeed.
            machine.max_feed_mm_min = 20_000.0;

            let _warnings = enforce_invariants(
                &mut op,
                &tool,
                &machine,
                &material,
                PassRole::Roughing,
                SuggestContext {
                    chipload_bounds: bounds,
                    policy: SuggestPolicy {
                        aggressiveness,
                        ..SuggestPolicy::default()
                    },
                    ..SuggestContext::default()
                },
            );
            op.feed_rate()
        };

        let feed_conservative = feed_for(SuggestAggressiveness::Conservative);
        let feed_default = feed_for(SuggestAggressiveness::Default);
        let feed_speed = feed_for(SuggestAggressiveness::Speed);

        assert!(
            feed_conservative <= feed_default,
            "Conservative feed ({feed_conservative}) must be ≤ Default feed ({feed_default})"
        );
        assert!(
            feed_default <= feed_speed,
            "Default feed ({feed_default}) must be ≤ Speed feed ({feed_speed})"
        );
        // The assertion that actually carries weight post-retirement: all
        // three levels land on the calculator's own 911 mm/min. Pre-fix
        // these were 3456 / 6144 / 7680 mm/min (band min / mid / max
        // divided by the 0.25 arc-fit ratio, times 16000 × 2).
        for (label, feed) in [
            ("Conservative", feed_conservative),
            ("Default", feed_default),
            ("Speed", feed_speed),
        ] {
            assert!(
                (feed - 911.0).abs() < 1e-6,
                "{label}: aggressiveness must not move feed off the calculator value 911 \
                 now that pass 8 is retired, got {feed}"
            );
        }
    }

    /// v3.3b: strategy-aware orchestrator rewrites Adaptive3d
    /// `entry_style` from Plunge to Ramp on Roughing ops when:
    ///   * `policy.scope == StrategyAndFeeds`
    ///   * `DPP / D > 0.5` (i.e. the same threshold v1.3's plunge-entry
    ///     warning fires on)
    ///   * `entry_style == Default::default() == Plunge` (the v3 design
    ///     doc's "B" pinning heuristic — only rewrite unpinned values)
    ///
    /// and DOES NOT fire under the v3.3a default `FeedsWithGates`
    /// scope. Wanaka Back Rough scaffold: 6 mm carbide endmill, DPP
    /// 3.69 mm (ratio 0.62), entry_style=Plunge.
    #[test]
    fn strategy_aware_rewrites_plunge_to_ramp_under_default_scope() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            depth_per_pass: 3.69,
            stepover: 1.2,
            feed_rate: 911.0,
            spindle_rpm: Some(16_000),
            entry_style: Adaptive3dEntryStyle::Plunge,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        // Short stickout keeps the closed-form deflection predictor
        // below the 200 µm back-off threshold at DPP 3.69 mm, so
        // pass 6 doesn't lower DPP and pass 7's DPP/D = 0.615 stays
        // above the 0.5 rewrite threshold.
        tool.stickout = 30.0;
        tool.flute_count = 2;
        let machine = MachineProfile::default();
        let material = Material::SolidWood {
            species: crate::material::WoodSpecies::HardMaple,
        };

        // Scope = StrategyAndFeeds → rewrite fires.
        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext {
                policy: SuggestPolicy {
                    scope: SuggestScope::StrategyAndFeeds,
                    ..SuggestPolicy::default()
                },
                ..SuggestContext::default()
            },
        );
        let OperationConfig::Adaptive3d(cfg) = &op else {
            panic!("op must remain Adaptive3d after enforce_invariants")
        };
        assert_eq!(
            cfg.entry_style,
            Adaptive3dEntryStyle::Ramp,
            "StrategyAndFeeds must rewrite Plunge → Ramp at DPP/D > 0.5"
        );
        assert!(
            warnings.iter().any(|w| matches!(
                w,
                SuggestWarning::StrategyRewrote {
                    param: "entry_style",
                    ..
                }
            )),
            "StrategyRewrote warning must fire, got {warnings:?}"
        );
        // v1.3 PlungeEntryUnstableAtDpp must NOT fire — the rewrite
        // already handled it.
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::PlungeEntryUnstableAtDpp { .. })),
            "PlungeEntryUnstableAtDpp must not fire after auto-rewrite, got {warnings:?}"
        );

        // Scope = FeedsWithGates → no rewrite, v1.3 warning fires.
        let mut op2 = OperationConfig::Adaptive3d(Adaptive3dConfig {
            depth_per_pass: 3.69,
            stepover: 1.2,
            feed_rate: 911.0,
            spindle_rpm: Some(16_000),
            entry_style: Adaptive3dEntryStyle::Plunge,
            ..Adaptive3dConfig::default()
        });
        let warnings2 = enforce_invariants(
            &mut op2,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext {
                policy: SuggestPolicy {
                    scope: SuggestScope::FeedsWithGates,
                    ..SuggestPolicy::default()
                },
                ..SuggestContext::default()
            },
        );
        let OperationConfig::Adaptive3d(cfg2) = &op2 else {
            panic!("op2 must remain Adaptive3d")
        };
        assert_eq!(
            cfg2.entry_style,
            Adaptive3dEntryStyle::Plunge,
            "FeedsWithGates must leave Plunge untouched"
        );
        assert!(
            warnings2
                .iter()
                .any(|w| matches!(w, SuggestWarning::PlungeEntryUnstableAtDpp { .. })),
            "PlungeEntryUnstableAtDpp must fire under FeedsWithGates, got {warnings2:?}"
        );
    }

    /// v3.3b pinning sentry (design-doc `strategy_pinning_respected`):
    /// when the user has explicitly set `entry_style` to a non-default
    /// value (Helix or Ramp), the strategy auto-pick must NOT stomp it
    /// even under StrategyAndFeeds — including the plan's exact
    /// scenario of Ramp pinned on an op where the chooser would
    /// otherwise pick Helix/Ramp itself (heuristic B: any value ≠
    /// `Default::default()` is treated as pinned).
    #[test]
    fn strategy_aware_leaves_non_default_entry_style_alone() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};

        for pinned in [Adaptive3dEntryStyle::Helix, Adaptive3dEntryStyle::Ramp] {
            let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
                depth_per_pass: 3.69,
                stepover: 1.2,
                feed_rate: 911.0,
                spindle_rpm: Some(16_000),
                entry_style: pinned,
                ..Adaptive3dConfig::default()
            });
            let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
            tool.diameter = 6.0;
            tool.cutting_length = 25.0;
            // Short stickout keeps the closed-form deflection predictor
            // below the 200 µm back-off threshold at DPP 3.69 mm, so
            // pass 6 doesn't lower DPP and pass 7's DPP/D = 0.615 stays
            // above the 0.5 rewrite threshold.
            tool.stickout = 30.0;
            tool.flute_count = 2;
            let machine = MachineProfile::default();
            let material = Material::SolidWood {
                species: crate::material::WoodSpecies::HardMaple,
            };

            let warnings = enforce_invariants(
                &mut op,
                &tool,
                &machine,
                &material,
                PassRole::Roughing,
                SuggestContext {
                    policy: SuggestPolicy {
                        scope: SuggestScope::StrategyAndFeeds,
                        ..SuggestPolicy::default()
                    },
                    ..SuggestContext::default()
                },
            );
            let OperationConfig::Adaptive3d(cfg) = &op else {
                panic!("op must remain Adaptive3d")
            };
            assert_eq!(
                cfg.entry_style, pinned,
                "user-set {pinned:?} must not be stomped"
            );
            assert!(
                !warnings.iter().any(|w| matches!(
                    w,
                    SuggestWarning::StrategyRewrote {
                        param: "entry_style",
                        ..
                    }
                )),
                "no StrategyRewrote for entry_style when user-pinned {pinned:?}, got {warnings:?}"
            );
        }
    }

    /// v3.3c: the clearing-strategy pass is WARN-ONLY. On an unpinned
    /// (ContourParallel-default) Adaptive3d roughing op over a
    /// well-formed model bbox (classifier → MixedTerrain), the
    /// recommendation warning fires AND the field is left untouched.
    #[test]
    fn clearing_strategy_recommendation_is_warn_only() {
        use crate::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
        let bbox = crate::geo::BoundingBox3 {
            min: crate::geo::P3::new(0.0, 0.0, -12.0),
            max: crate::geo::P3::new(200.0, 150.0, 0.0),
        };
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            depth_per_pass: 2.0,
            stepover: 1.2,
            feed_rate: 911.0,
            spindle_rpm: Some(16_000),
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        tool.stickout = 30.0;
        tool.flute_count = 2;
        let machine = MachineProfile::default();
        let material = Material::SolidWood {
            species: crate::material::WoodSpecies::HardMaple,
        };

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext {
                model_bbox: Some(&bbox),
                policy: SuggestPolicy {
                    scope: SuggestScope::StrategyAndFeeds,
                    ..SuggestPolicy::default()
                },
                ..SuggestContext::default()
            },
        );
        let recommended = warnings.iter().find_map(|w| match w {
            SuggestWarning::StrategyRecommendedNotApplied {
                param: "clearing_strategy",
                current,
                recommended,
                reason,
            } => Some((current.clone(), recommended.clone(), *reason)),
            _ => None,
        });
        let (current, recommended, reason) = recommended
            .expect("clearing-strategy recommendation must fire on unpinned MixedTerrain op");
        assert_eq!(current, "contour_parallel");
        assert_eq!(recommended, "adaptive");
        assert_eq!(reason, "mixed_terrain_classifier");
        // Warn-only: the field must be untouched.
        let OperationConfig::Adaptive3d(cfg) = &op else {
            panic!("op must remain Adaptive3d")
        };
        assert_eq!(
            cfg.clearing_strategy,
            ClearingStrategy::ContourParallel,
            "warn-only pass must NOT rewrite clearing_strategy"
        );
    }

    /// v3.3c skip conditions: pinned non-default value (heuristic B),
    /// FeedsWithGates scope, and missing model bbox (classifier →
    /// Unknown) must each suppress the recommendation.
    #[test]
    fn clearing_strategy_recommendation_skips() {
        use crate::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
        let bbox = crate::geo::BoundingBox3 {
            min: crate::geo::P3::new(0.0, 0.0, -12.0),
            max: crate::geo::P3::new(200.0, 150.0, 0.0),
        };
        let run = |strategy: ClearingStrategy,
                   scope: SuggestScope,
                   model_bbox: Option<&crate::geo::BoundingBox3>|
         -> Vec<SuggestWarning> {
            let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
                depth_per_pass: 2.0,
                stepover: 1.2,
                feed_rate: 911.0,
                spindle_rpm: Some(16_000),
                clearing_strategy: strategy,
                ..Adaptive3dConfig::default()
            });
            let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
            tool.diameter = 6.0;
            tool.cutting_length = 25.0;
            tool.stickout = 30.0;
            tool.flute_count = 2;
            let machine = MachineProfile::default();
            let material = Material::SolidWood {
                species: crate::material::WoodSpecies::HardMaple,
            };
            enforce_invariants(
                &mut op,
                &tool,
                &machine,
                &material,
                PassRole::Roughing,
                SuggestContext {
                    model_bbox,
                    policy: SuggestPolicy {
                        scope,
                        ..SuggestPolicy::default()
                    },
                    ..SuggestContext::default()
                },
            )
        };

        let no_recommendation = |warnings: &[SuggestWarning], ctx: &str| {
            assert!(
                !warnings
                    .iter()
                    .any(|w| matches!(w, SuggestWarning::StrategyRecommendedNotApplied { .. })),
                "{ctx}: recommendation must not fire, got {warnings:?}"
            );
        };

        // Pinned non-default values (heuristic B).
        for pinned in [ClearingStrategy::Adaptive, ClearingStrategy::AgentSearch] {
            no_recommendation(
                &run(pinned, SuggestScope::StrategyAndFeeds, Some(&bbox)),
                &format!("pinned {pinned:?}"),
            );
        }
        // Strategy passes off.
        no_recommendation(
            &run(
                ClearingStrategy::ContourParallel,
                SuggestScope::FeedsWithGates,
                Some(&bbox),
            ),
            "FeedsWithGates scope",
        );
        // No bbox → classifier Unknown → no signal.
        no_recommendation(
            &run(
                ClearingStrategy::ContourParallel,
                SuggestScope::StrategyAndFeeds,
                None,
            ),
            "missing model bbox",
        );
    }

    /// PRE-Phase-1 coverage net (architectural refactor T1): every
    /// operation's feeds hints must be an *explicit decision*. The
    /// implementation now lives in the registry-layer
    /// [`OperationConfig::feeds_hints`] exhaustive match (T3 replaced
    /// the old `_ => (None, None, None)` wildcard here); this net stays
    /// as the independent baseline guarding both the table and the tuple
    /// adapter.
    ///
    /// The expected table below is an exhaustive match over
    /// `OperationConfig` with **no wildcard arm** — adding a new
    /// operation fails to compile here, forcing the author to decide
    /// (and record) what hints the new op feeds the calculator. Changing
    /// an existing op's hint plumbing fails the assert instead of
    /// silently rerouting the feeds suggestion.
    #[test]
    fn operation_feeds_hints_is_an_explicit_per_op_decision() {
        use crate::compute::catalog::OperationType;

        for &op_type in OperationType::ALL {
            let config = OperationConfig::new_default(op_type);
            let actual = operation_feeds_hints(&config);

            #[allow(clippy::match_same_arms)] // one arm per op = the point
            let expected: (Option<f64>, Option<f64>, Option<f64>) = match &config {
                // Ops that feed operation-specific hints into the
                // calculator (axial, radial, scallop):
                OperationConfig::Scallop(cfg) => (None, None, Some(cfg.scallop_height)),
                OperationConfig::UnifiedFinish(cfg) => (None, None, Some(cfg.scallop_height)),
                OperationConfig::DropCutter(cfg) => (None, None, cfg.scallop_height),
                OperationConfig::Waterline(cfg) => (Some(cfg.z_step), None, None),
                OperationConfig::SteepShallow(cfg) => (Some(cfg.z_step), None, None),
                OperationConfig::VCarve(cfg) => (Some(cfg.max_depth), None, None),
                OperationConfig::RampFinish(cfg) => (Some(cfg.max_stepdown), None, None),
                // Ops that have explicitly DECIDED to provide no hints —
                // the calculator falls back to LUT/diameter-factor
                // defaults. Listed individually (not `_`) so each is a
                // recorded decision:
                OperationConfig::Face(_) => (None, None, None),
                OperationConfig::Pocket(_) => (None, None, None),
                OperationConfig::Profile(_) => (None, None, None),
                OperationConfig::Adaptive(_) => (None, None, None),
                OperationConfig::Rest(_) => (None, None, None),
                OperationConfig::Inlay(_) => (None, None, None),
                OperationConfig::Zigzag(_) => (None, None, None),
                OperationConfig::Trace(_) => (None, None, None),
                OperationConfig::Drill(_) => (None, None, None),
                OperationConfig::Chamfer(_) => (None, None, None),
                OperationConfig::Adaptive3d(_) => (None, None, None),
                OperationConfig::Pencil(_) => (None, None, None),
                OperationConfig::SpiralFinish(_) => (None, None, None),
                OperationConfig::RadialFinish(_) => (None, None, None),
                OperationConfig::HorizontalFinish(_) => (None, None, None),
                OperationConfig::ProjectCurve(_) => (None, None, None),
                OperationConfig::AlignmentPinDrill(_) => (None, None, None),
            };

            assert_eq!(
                actual, expected,
                "{op_type:?}: operation_feeds_hints changed — update \
                 operation_feeds_hints AND this expected table deliberately \
                 (this net guards the suggest.rs (None,None,None) wildcard)"
            );
        }
    }
}
