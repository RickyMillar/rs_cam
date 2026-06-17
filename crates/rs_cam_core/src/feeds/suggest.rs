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
    /// operating point. 0.0 when not populated (re-derivation skipped).
    pub effective_diameter_mm: f64,
    /// v3.0b: caller-supplied policy threading through the
    /// orchestrator. Default = `SuggestPolicy::default()` =
    /// aggressiveness `Default` (LUT band midpoint) since the v3.0c
    /// flip per the 2026-06-03 directive.
    pub policy: SuggestPolicy,
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
    /// v2 combined-Suggest step 2: After the v1.1 deflection back-off
    /// settled DPP, the chipload-aware feed recalibration loop raised
    /// `feed_rate` to bring the predicted observed median chipload up
    /// to the LUT band lower bound (see
    /// [`crate::feeds::predict::predict_observed_chipload_mm`]). The
    /// loop respects the deflection hard limit (200 µm) and the
    /// `machine.max_feed_mm_min` ceiling; if either binds before the
    /// chipload reaches the band, [`SuggestWarning::ChiploadStillLowAfterRecalibration`]
    /// fires alongside this variant with the limiting cap.
    ///
    /// Wanaka motivating case (2026-06-03 post-sim): three roughing /
    /// finishing toolpaths shipped at observed median chipload
    /// 0.0067-0.011 mm/tooth vs LUT minima 0.027-0.076 — classic
    /// rubbing/burning recipe. With deflection back-off settling DPP
    /// at 3.69 mm and steady-state δ ≈ 178 µm, this loop raises feed
    /// until either the predicted observed lands in band or the
    /// 200 µm gate (less 10 µm headroom) clamps the iteration.
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
    /// v2 combined-Suggest step 2: paired with
    /// [`SuggestWarning::FeedRaisedForChipload`] when the loop hit a
    /// binding cap before the predicted observed chipload reached the
    /// LUT minimum. The deflection budget is too tight for the
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
    apply_drill_defaults(&mut operation, input.tool, input.material);
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

/// Write a [`FeedsResult`] into an [`OperationConfig`] and enforce the canonical
/// suggestion invariants. Values are rounded for UI-friendly display before
/// clamping, matching the historical Suggest-button behaviour.
///
/// `context` carries project-level slots (model bbox, upstream leftover,
/// strategy hint) used by gate-aware Suggest paths — v1.2 reads
/// `context.model_bbox` to gate the runtime-sanity stepover back-off.
/// Callers that don't have the context cheaply available should pass
/// [`SuggestContext::default()`]; the back-off short-circuits to a no-op
/// when `model_bbox` is `None`.
/// Which dimensions of a [`FeedsResult`] an apply writes back to the operation.
///
/// W3.1 (IA cleanup) split the single apply into a SPEED path (feed / plunge /
/// RPM — "how fast") and a CUT-geometry path (stepover / DOC — "how deep/wide,
/// changes the cut"), so the Feeds UI can offer a speed-only "Apply recommended
/// speeds" that never silently rewrites the cut geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplySubset {
    Speeds,
    CutGeometry,
    Both,
}

/// Apply the calculator's recommendation to `operation`, writing back only the
/// requested [`ApplySubset`].
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
    subset: ApplySubset,
) -> Vec<SuggestWarning> {
    let mut scratch = operation.clone();
    scratch.set_feed_rate(round_suggestion_value(result.feed_rate_mm_min, 1.0));
    scratch.set_plunge_rate(round_suggestion_value(result.plunge_rate_mm_min, 1.0));
    scratch.set_stepover(round_suggestion_value(result.radial_width_mm, 0.001));
    scratch.set_depth_per_pass(round_suggestion_value(result.axial_depth_mm, 0.001));
    // v3.0d (2026-06-04): also write the calculator's chosen RPM so
    // enforce_invariants's chipload recalibration pass reads a
    // consistent operating point instead of the operation's prior
    // spindle_rpm value. Without this, Suggest is non-idempotent — a
    // second run on already-Suggested values reads a different RPM
    // (the post-first-Suggest value) and lands a different target_feed.
    // The closed-form solve is `target_feed = target × rpm × flutes /
    // arc_fit`, so any RPM drift propagates linearly into feed.
    let rpm_written = result.rpm.is_finite() && result.rpm > 0.0;
    if rpm_written {
        scratch.set_spindle_rpm(Some(result.rpm.round() as u32));
    }
    // Enrich the caller-supplied context with the LUT chipload band
    // derived by the calculator so v2 step 2 recalibration can read it
    // without a separate parameter. Also thread through the matched LUT
    // row + effective diameter so the axial-DOC envelope pass
    // (`pick_axial_envelope`) can query vendor `ap_*_factor` / `ap_*_mm`
    // and the chipload-bounds re-derivation step in `enforce_invariants`
    // can re-apply `doc_derating_scale` against a mutated DPP.
    let enriched = SuggestContext {
        chipload_bounds: result.chipload_bounds,
        matched_lut_row: result.matched_lut_row.as_ref(),
        effective_diameter_mm: result.effective_diameter_mm,
        ..context
    };
    let warnings = enforce_invariants(&mut scratch, tool, machine, material, pass_role, enriched);

    let write_speeds = matches!(subset, ApplySubset::Speeds | ApplySubset::Both);
    let write_geometry = matches!(subset, ApplySubset::CutGeometry | ApplySubset::Both);
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
        ApplySubset::Both,
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
        ApplySubset::Speeds,
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
        ApplySubset::CutGeometry,
    )
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
pub fn apply_drill_defaults(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    material: &Material,
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
        OperationConfig::AlignmentPinDrill(cfg) => cfg.peck_depth = peck,
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

/// Combined deflection ceiling (µm) used by both the v1.1 DPP back-off
/// and the v2 step 2 feed-up recalibration verify. When the closed-form
/// predictor in [`crate::feeds::predict::predict_peak_deflection_um`]
/// projects a tip deflection above this value, [`enforce_invariants`]
/// either:
///
/// * (v1.1) iteratively reduces DPP by 20% per step (max 5 steps, floor
///   [`DEFLECTION_BACKOFF_DPP_FLOOR_MM`]) until the prediction clears,
///   or
/// * (v2 step 2) refuses a feed-up solve whose post-write deflection
///   would land within [`FEED_RAISE_DEFLECTION_RECAL_HEADROOM_UM`] of
///   this gate, reverting to the pre-recal feed.
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

/// v2 step 2 refusal headroom under the deflection hard limit (µm).
/// The recalibration refuses a target feed whose predicted deflection
/// lands above `DEFLECTION_BACKOFF_TARGET_UM − 10 = 190 µm`. Headroom
/// keeps the new feed from sitting exactly at the gate so any small
/// downstream modeling error (chip thinning, hardness scaling, formula
/// vs LUT) doesn't kick the operation just over the line.
///
/// Future-proofing note: today's `predict_peak_deflection_um` is
/// feed-independent (force = Kc × axial_doc × radial_woc, no chipload
/// term), so the post-recalibration verify in `enforce_invariants` is
/// a defensive no-op on the current model. The headroom is retained
/// for a future chipload-in-force deflection predictor that would
/// couple feed to load and thus to deflection.
const FEED_RAISE_DEFLECTION_RECAL_HEADROOM_UM: f64 = 10.0;

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
/// - Finish-3D (Scallop / DropCutter / Waterline / SteepShallow /
///   SpiralFinish / RadialFinish / HorizontalFinish) — finish limit +
///   default scallop target, radial WOC = stepover (default 0.15·D).
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
/// - Finish-3D (Scallop / DropCutter / Waterline / SteepShallow /
///   SpiralFinish / RadialFinish / HorizontalFinish) — emits
///   `FinishEnvelopeAdvisory`; automatic `stock_to_leave` mutation is
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
        | OperationConfig::DropCutter(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_) => {
            let op_kind = match operation {
                OperationConfig::Scallop(_) => "scallop",
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
/// DPP. Mirrors `feeds::calculate`'s in-place derivation (around
/// `feeds/mod.rs:701`) so the post-mutation chipload-recalibration
/// pass sees a bounds value consistent with the new DPP / effective-D
/// ratio. No-op when the matched LUT row is absent or carries no
/// chipload band (the original derivation would also be `None`).
fn recompute_chipload_bounds_for_dpp(
    matched_row: Option<&crate::feeds::vendor_lookup::LookupResult>,
    effective_diameter_mm: f64,
    operation: &OperationConfig,
    new_dpp_mm: f64,
) -> Option<crate::feeds::ChiploadBounds> {
    let row = matched_row?;
    let (min, max) = match (row.chip_load_min_mm, row.chip_load_max_mm) {
        (Some(min), Some(max)) if min.is_finite() && max.is_finite() && min > 0.0 && max >= min => {
            (min, max)
        }
        _ => return None,
    };
    // Drill ops are excluded from doc-derating per
    // `feeds::calculate` (same path), so leave bounds at the raw
    // LUT values for them.
    let op_family = operation.feeds_style().0;
    let scale = if matches!(op_family, FeedsOperationFamily::Drill) || effective_diameter_mm <= 0.0
    {
        1.0
    } else {
        let ratio = new_dpp_mm / effective_diameter_mm;
        crate::feeds::geometry::doc_derating_scale(ratio)
    };
    Some(crate::feeds::ChiploadBounds {
        min_mm_per_tooth: min * scale,
        max_mm_per_tooth: max * scale,
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
    warnings.extend(recalibrate_feed_for_chipload(
        operation, tool, material, machine, context,
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

/// Pass 8: v2 step 2 chipload-aware feed recalibration — single-shot
/// closed-form solve (v2.1, 2026-06-04).
///
/// After the v1.1 deflection back-off settles DPP, the operating
/// point may still sit below the LUT chipload band lower bound.
/// Wanaka post-sim (2026-06-03) showed three roughing/finishing
/// toolpaths shipping at observed median 0.0067-0.011 mm/tooth vs
/// LUT minima 0.027-0.076 — classic rubbing/burning.
///
/// The math is closed-form:
///
/// ```text
/// predict_observed_chipload_mm = nominal × arc_fit_ratio,
/// where nominal = feed / (rpm × flutes)
/// ```
///
/// Solving for the feed that lands `observed = lut_min`:
///
/// ```text
/// feed = lut_min × rpm × flutes / arc_fit_ratio
/// ```
///
/// The pre-v2.1 implementation iterated `feed × 1.25` up to four
/// times before bailing; on the Wanaka Back Rough case the required
/// lift was 3.79× (911 → 3456 mm/min) and 1.25⁴ ≈ 2.44× fell short —
/// the loop terminated on a synthetic `MaxIterations` cap that didn't
/// reflect a real physical limit. Since `predict_peak_deflection_um`
/// is feed-independent today, iterating wasn't probing any feedback
/// term that exists in the closed-form model — it was just papering
/// over the missing chipload-in-force coupling. We solve directly
/// instead.
///
/// Caps that can still bind (post-solve):
///   * `machine.max_feed_mm_min` (hard physical limit) — clamp and
///     emit `MaxFeed`.
///   * Deflection refusal (defensive no-op on the current predictor;
///     future-proofs a chipload-in-force model) — revert to pre-recal
///     feed and emit `DeflectionThreshold`.
///
/// Skip conditions:
///   * No LUT min available (no_vendor_data, partial-band rows,
///     formula-fallback): `chipload_bounds` is None.
///   * Op family's arc-fit ratio is conservative-Default or
///     NotApplicable (drill/v-carve/project_curve produce
///     NotApplicable; Pocket/Profile/Adaptive2D/Waterline produce
///     Default). Default ratios are calibrated against the *nominal*
///     chipload scale used by the matrix anti-patterns; solving
///     feed = lut_min/Default would systematically push the operating
///     point above the matrix ipe/oak anti-patterns (e.g.
///     `ipe_micro_matches_oak_micro_chipload` at 0.030 mm/tooth on
///     flat_3mm_pocket_ipe_extreme). Until the broader op families
///     gain Wanaka cells, restrict feed-up to the Calibrated rows.
///   * Predicted observed already in band: no work to do.
///   * Spindle RPM unknown or flute count zero: the closed-form
///     denominator is undefined.
///
/// PlungeClampedToFeed is not re-checked: raising feed only widens
/// the plunge/feed margin, never narrows it.
fn recalibrate_feed_for_chipload(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(bounds) = context.chipload_bounds
        && let Some(rpm) = operation.spindle_rpm()
        && tool.flute_count > 0
        && rpm > 0
    {
        let pred_before = crate::feeds::predict::predict_observed_chipload_mm(operation, tool);
        let arc_fit_calibrated = matches!(
            pred_before.source,
            crate::feeds::predict::ArcFitRatioSource::Calibrated
        );
        let target = context.policy.aggressiveness.target_chipload(bounds);
        let initial_feed = operation.feed_rate();
        let initial_observed = pred_before.observed_median_mm_per_tooth;

        if arc_fit_calibrated
            && target.is_finite()
            && target > 0.0
            && initial_feed.is_finite()
            && initial_feed > 0.0
            && initial_observed < target
            && pred_before.arc_fit_ratio > 0.0
        {
            let target_nominal = target / pred_before.arc_fit_ratio;
            let target_feed = target_nominal * f64::from(rpm) * f64::from(tool.flute_count);
            // F4: recalibration raises CUTTING feed — cap at the
            // cutting ceiling, not the travel rate.
            let machine_cut_ceiling = machine.cutting_feed_ceiling_mm_min();
            let mut new_feed = target_feed.min(machine_cut_ceiling);
            let mut cap_hit: Option<FeedRecalibrationCap> = None;

            if target_feed > machine_cut_ceiling {
                cap_hit = Some(FeedRecalibrationCap::MaxFeed);
            }

            // Apply the solved feed and verify the deflection budget
            // still passes at the new operating point. Today's
            // `predict_peak_deflection_um` is feed-independent (force
            // = Kc × axial_doc × radial_woc), so this verify is a
            // defensive no-op on the current closed-form model. Kept
            // in place for a future chipload-in-force deflection
            // predictor that would couple feed to load and thus to
            // deflection.
            operation.set_feed_rate(new_feed);
            let deflection_refusal_um =
                DEFLECTION_BACKOFF_TARGET_UM - FEED_RAISE_DEFLECTION_RECAL_HEADROOM_UM;
            let defl = crate::feeds::predict::predict_peak_deflection_um(
                operation, tool, material, machine,
            );
            if defl.predicted_um > deflection_refusal_um {
                operation.set_feed_rate(initial_feed);
                new_feed = initial_feed;
                cap_hit = Some(FeedRecalibrationCap::DeflectionThreshold);
            }

            let pred_after = crate::feeds::predict::predict_observed_chipload_mm(operation, tool);

            let feed_moved = (new_feed - initial_feed).abs() > 0.5;
            let reverted_on_deflection = cap_hit == Some(FeedRecalibrationCap::DeflectionThreshold);

            if feed_moved || reverted_on_deflection {
                tracing::debug!(
                    requested_mm_per_min = initial_feed,
                    raised_mm_per_min = new_feed,
                    predicted_observed_chipload_before = initial_observed,
                    predicted_observed_chipload_after = pred_after.observed_median_mm_per_tooth,
                    lut_target_mm_per_tooth = target,
                    ?cap_hit,
                    "Suggest chipload-aware feed recalibration (single-shot)"
                );
                warnings.push(SuggestWarning::FeedRaisedForChipload {
                    requested_mm_per_min: initial_feed,
                    raised_mm_per_min: new_feed,
                    predicted_observed_chipload_before: initial_observed,
                    predicted_observed_chipload_after: pred_after.observed_median_mm_per_tooth,
                    lut_target_mm_per_tooth: target,
                    cap_hit,
                });
            }

            if pred_after.observed_median_mm_per_tooth < target
                && let Some(blocking_cap) = cap_hit
            {
                warnings.push(SuggestWarning::ChiploadStillLowAfterRecalibration {
                    predicted_observed_mm_per_tooth: pred_after.observed_median_mm_per_tooth,
                    lut_target_mm_per_tooth: target,
                    feed_at_termination_mm_per_min: new_feed,
                    blocking_cap,
                });
            }
        }
    }
    warnings
}

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

        apply_drill_defaults(&mut op_3mm, &tool_3mm, &material);

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

        apply_drill_defaults(&mut op_12mm, &tool_12mm, &material);

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
        apply_drill_defaults(&mut op_pocket, &tool_3mm, &material);
        assert_eq!(
            format!("{op_pocket:?}"),
            pocket_before,
            "apply_drill_defaults must be a no-op for non-drill ops"
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
            apply_drill_defaults(&mut op, &tool, material);
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
    fn deflection_machinery_caps_dpp_for_wanaka_back_rough_case() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::material::WoodSpecies;
        // Synthetic Wanaka Back Rough: 6 mm carbide endmill, 45 mm
        // stickout, DPP 9 mm (LUT × adaptive_doc_factor), WOC 1.2 mm,
        // feed 911 mm/min @ 16 kRPM, HardMaple.
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
        tool.stickout = 45.0;
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
        // DPP is clamped below the 9 mm command, but to the deflection
        // bound (~6.57 mm) — NOT thrashed down to ~2 mm as the old
        // phantom-hot back-off did.
        assert!(
            (5.5..9.0).contains(&dpp_after),
            "DPP must be clamped to the ~6.57 mm deflection bound (below 9 mm, not over-cut), got {dpp_after} mm"
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
        tool.stickout = 150.0;
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

    /// v2 combined-Suggest step 2 (2026-06-04 design doc § v2 step 2):
    /// Wanaka Back Rough motivating case. After the v1.1 deflection
    /// back-off settled DPP at ~3.69 mm (post-sim ~178 µm steady),
    /// observed median chipload still sat at 0.0067–0.011 mm/tooth vs
    /// LUT minimum ~0.027 mm/tooth. The feed-up recalibration must
    /// raise `feed_rate` and emit `FeedRaisedForChipload`.
    ///
    /// We synthesize the post-v1.1 operating point directly (DPP=3.69,
    /// feed=911 mm/min @ 16 kRPM, HardMaple, 6 mm 2-flute endmill) and
    /// drive `enforce_invariants` with an explicit `ChiploadBounds`
    /// matching the LUT min so the test is hermetic w.r.t. the
    /// embedded LUT version.
    #[test]
    fn feed_recalibration_raises_feed_for_wanaka_back_rough_case() {
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
        // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7): at
        // the original 45 mm stickout the deflection ceiling already binds
        // at the baseline 911 mm/min feed, so the feed-up loop has zero
        // headroom and can't raise feed (its intended behaviour). This
        // test verifies the chipload feed-UP recalibration, not the
        // deflection cap — stiffen the tool (stickout 45 → 18 mm; δ ∝
        // stickout³ → ~0.064×) so deflection leaves headroom and the
        // feed-up loop can do its job.
        tool.stickout = 18.0;
        tool.flute_count = 2;
        let mut machine = MachineProfile::default();
        // Disable the rigidity clamp at DPP=3.69 (3.69 < 1.6 × 6 = 9.6 is OK).
        machine.rigidity.doc_roughing_factor = 0.20;
        machine.rigidity.adaptive_doc_factor = 1.60;
        // Generous feed cap so we don't immediately hit MaxFeed.
        machine.max_feed_mm_min = 10_000.0;
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let bounds = Some(ChiploadBounds {
            min_mm_per_tooth: 0.027,
            max_mm_per_tooth: 0.060,
        });

        let initial_feed = op.feed_rate();
        let initial_observed = crate::feeds::predict::predict_observed_chipload_mm(&op, &tool)
            .observed_median_mm_per_tooth;

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext {
                chipload_bounds: bounds,
                // v3.0c (median default) flipped the implicit Suggest
                // target from LUT min to LUT median. This test asserts
                // the v2.1 / Conservative closed-form solve (target =
                // LUT min = 0.027 → feed = 3456 mm/min); pin the
                // policy so the assertions remain valid.
                policy: SuggestPolicy {
                    aggressiveness: SuggestAggressiveness::Conservative,
                    ..SuggestPolicy::default()
                },
                ..SuggestContext::default()
            },
        );

        let feed_after = op.feed_rate();
        assert!(
            feed_after > initial_feed,
            "feed-up loop must raise feed above {initial_feed}, got {feed_after}"
        );

        let warning = warnings.iter().find_map(|w| match w {
            SuggestWarning::FeedRaisedForChipload {
                requested_mm_per_min,
                raised_mm_per_min,
                predicted_observed_chipload_before,
                predicted_observed_chipload_after,
                lut_target_mm_per_tooth,
                cap_hit,
            } => Some((
                *requested_mm_per_min,
                *raised_mm_per_min,
                *predicted_observed_chipload_before,
                *predicted_observed_chipload_after,
                *lut_target_mm_per_tooth,
                *cap_hit,
            )),
            _ => None,
        });
        let (
            requested_mm_per_min,
            raised_mm_per_min,
            predicted_before,
            predicted_after,
            lut_target,
            cap_hit,
        ) = warning.expect("FeedRaisedForChipload must fire on Wanaka Back Rough case");
        assert!(
            (requested_mm_per_min - initial_feed).abs() < 1e-6,
            "warning.requested_mm_per_min must capture pre-recal feed, got {requested_mm_per_min}"
        );
        assert!(
            (raised_mm_per_min - feed_after).abs() < 1e-6,
            "warning.raised_mm_per_min must match post-recal feed, got {raised_mm_per_min}"
        );
        assert!(
            (predicted_before - initial_observed).abs() < 1e-9,
            "warning.predicted_observed_chipload_before must equal initial prediction, got {predicted_before}"
        );
        assert!(
            (lut_target - 0.027).abs() < 1e-9,
            "warning.lut_target_mm_per_tooth must round-trip the bound (Conservative policy targets band min), got {lut_target}"
        );
        // Closed-form target on Wanaka: target / arc_fit_ratio × rpm × flutes
        //   = 0.027 / 0.25 × 16000 × 2 = 3456 mm/min.
        // machine.max_feed_mm_min = 10_000 here, so no MaxFeed cap, and
        // deflection at 3.69 mm DPP / 45 mm stickout / 6 mm 2-flute /
        // HardMaple sits below the 190 µm refusal, so no
        // DeflectionThreshold cap either.
        assert!(
            (feed_after - 3456.0).abs() < 1.0,
            "Wanaka closed-form target must land at 3456 ± 1 mm/min, got {feed_after}"
        );
        assert_eq!(
            cap_hit, None,
            "no cap should bind on Wanaka case at this machine.max_feed, got {cap_hit:?}"
        );
        // Closed-form invariant: post-recal observed must equal target
        // (within float epsilon) when no cap binds.
        assert!(
            (predicted_after - lut_target).abs() < 1e-9,
            "closed-form solve must land observed at target when uncapped, got observed={predicted_after} vs target={lut_target}"
        );
    }

    /// v2 step 2 cap test: when the post-v1.1 operating point already
    /// sits at or above the 10 µm deflection refusal headroom
    /// (predicted ≥ 190 µm), the single-shot recalibration writes the
    /// solved feed, the deflection verify trips, the feed is reverted
    /// to its pre-recal value, and both `FeedRaisedForChipload` (with
    /// `cap_hit = DeflectionThreshold`, `raised == requested`) and
    /// `ChiploadStillLowAfterRecalibration` fire.
    ///
    /// **Predictor caveat (2026-06-04):** the closed-form deflection
    /// predictor computes force as `Kc × axial_doc × radial_woc` — it
    /// is *not* a function of feed/chipload. So raising feed within
    /// this step does not raise the predictor's output (the inputs
    /// `axial_doc_mm` / `radial_woc_mm` come from the operation
    /// fields, and we don't touch those). The guard is still
    /// defensive: it refuses to *write* a feed when the operation's
    /// current deflection profile is already at the boundary, even
    /// though the feed change itself doesn't perturb the prediction.
    /// To trigger `DeflectionThreshold` deterministically we set up a
    /// stickout / DPP combination whose pre-loop predicted deflection
    /// already exceeds 190 µm; the verify check then trips and the
    /// recalibration reverts before any feed change lands.
    #[test]
    fn feed_recalibration_caps_on_deflection() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::feeds::ChiploadBounds;
        use crate::material::WoodSpecies;

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
        // Stickout tuned so the pre-loop predicted deflection lands
        // in the (190, 200) µm window — above the 190 µm guard but
        // below the 200 µm v1.1 back-off target (so v1.1 doesn't fire
        // first and lower DPP underneath us).
        // Deflection-model reconciliation (2026-06-17): the predictor now
        // delegates to the integrated two-section cantilever (the gate's
        // model) instead of its old single-section `0.7·D` formula, which
        // dropped the magnitude ~4× (it had relieved the whole stickout,
        // not just the flutes). δ ∝ stickout³, so the stickout that lands
        // the pre-loop prediction in the [190, 200) µm window grew to
        // ~53.2 mm (the in-test setup guard below asserts this and tells
        // the next editor to retune if the physics shifts again).
        tool.stickout = 53.2;
        tool.flute_count = 2;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.20;
        machine.rigidity.adaptive_doc_factor = 1.60;
        machine.max_feed_mm_min = 10_000.0;
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let bounds = Some(ChiploadBounds {
            min_mm_per_tooth: 0.05,
            max_mm_per_tooth: 0.10,
        });

        // Sanity: confirm the pre-loop deflection sits in the
        // (190, 200) µm window. If this changes (e.g. Kc/HardMaple
        // calibration shifts), retune `tool.stickout`.
        let pre_predicted =
            crate::feeds::predict::predict_peak_deflection_um(&op, &tool, &material, &machine)
                .predicted_um;
        let refusal_um = DEFLECTION_BACKOFF_TARGET_UM - FEED_RAISE_DEFLECTION_RECAL_HEADROOM_UM;
        assert!(
            (refusal_um..DEFLECTION_BACKOFF_TARGET_UM).contains(&pre_predicted),
            "test setup: pre-loop predicted deflection ({pre_predicted:.1} µm) must sit in \
             [{refusal_um:.0}, {DEFLECTION_BACKOFF_TARGET_UM:.0}) µm — retune tool.stickout"
        );

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

        // v2.1 single-shot: on a deflection-cap revert, both the
        // FeedRaisedForChipload (with raised == requested) AND the
        // still-low warning fire. The cap_hit carries the diagnostic
        // signal.
        let initial_feed = 911.0_f64;
        let raised = warnings.iter().find_map(|w| match w {
            SuggestWarning::FeedRaisedForChipload {
                requested_mm_per_min,
                raised_mm_per_min,
                cap_hit,
                ..
            } => Some((*requested_mm_per_min, *raised_mm_per_min, *cap_hit)),
            _ => None,
        });
        let (requested_mm_per_min, raised_mm_per_min, raised_cap) = raised.expect(
            "FeedRaisedForChipload must fire on deflection revert (v2.1 emits on cap-hit even when reverted)",
        );
        assert!(
            (requested_mm_per_min - initial_feed).abs() < 1e-6,
            "requested feed must capture pre-recal value, got {requested_mm_per_min}"
        );
        assert!(
            (raised_mm_per_min - initial_feed).abs() < 1e-6,
            "on deflection revert, raised_mm_per_min must equal requested_mm_per_min, got {raised_mm_per_min}"
        );
        assert_eq!(
            raised_cap,
            Some(FeedRecalibrationCap::DeflectionThreshold),
            "cap_hit on FeedRaisedForChipload must be DeflectionThreshold, got {raised_cap:?}"
        );

        let still_low = warnings.iter().find_map(|w| match w {
            SuggestWarning::ChiploadStillLowAfterRecalibration {
                predicted_observed_mm_per_tooth,
                lut_target_mm_per_tooth,
                feed_at_termination_mm_per_min,
                blocking_cap,
            } => Some((
                *predicted_observed_mm_per_tooth,
                *lut_target_mm_per_tooth,
                *feed_at_termination_mm_per_min,
                *blocking_cap,
            )),
            _ => None,
        });
        let (still_low_observed, still_low_target, still_low_feed, blocking_cap) = still_low
            .expect("ChiploadStillLowAfterRecalibration must fire when deflection cap binds");
        assert_eq!(
            blocking_cap,
            FeedRecalibrationCap::DeflectionThreshold,
            "blocking cap must be DeflectionThreshold when feed-up trial breaches the gate, got {blocking_cap:?}"
        );
        assert!(
            still_low_observed < still_low_target,
            "observed ({still_low_observed}) must still be below target ({still_low_target}) when deflection cap binds"
        );
        assert!(
            (still_low_feed - initial_feed).abs() < 1e-6,
            "feed_at_termination must equal initial_feed after revert, got {still_low_feed}"
        );
        // Op feed must be reverted to the pre-recal value.
        assert!(
            (op.feed_rate() - initial_feed).abs() < 1e-6,
            "op feed must be reverted to initial after deflection revert, got {}",
            op.feed_rate()
        );
    }

    /// v3 design-doc sentry (`speed_gated_by_deflection_fires`): under
    /// `SuggestAggressiveness::Speed` the recalibration aims at the LUT
    /// band *max*, and when the deflection guard refuses the write the
    /// gating surfaces as `cap_hit = DeflectionThreshold` — the design
    /// doc's planned `SpeedGatedByDeflection` variant collapsed into
    /// this field (same structure as still-low, distinct meaning: "you
    /// asked for max chipload, deflection said no").
    ///
    /// Same long-stickout window as
    /// `feed_recalibration_caps_on_deflection` (pre-loop predicted
    /// deflection in the (190, 200) µm refusal band), but with
    /// `policy.aggressiveness = Speed`. Asserts the still-low warning's
    /// `lut_target_mm_per_tooth` is the band MAX — proving Speed's
    /// target (not Conservative's min / Default's midpoint) is what got
    /// gated.
    #[test]
    fn speed_gated_by_deflection_fires() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::feeds::ChiploadBounds;
        use crate::material::WoodSpecies;

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
        // Same stickout tuning as feed_recalibration_caps_on_deflection:
        // pre-loop deflection inside the (190, 200) µm refusal window.
        // Deflection-model reconciliation (2026-06-17): the predictor now
        // shares the gate's integrated two-section cantilever, ~4× cooler
        // than the old single-section `0.7·D` formula. δ ∝ stickout³ →
        // ~53.2 mm lands the pre-loop prediction back in the [190, 200) µm
        // window (the setup guard below asserts it).
        tool.stickout = 53.2;
        tool.flute_count = 2;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.20;
        machine.rigidity.adaptive_doc_factor = 1.60;
        // Generous cap: Speed's closed-form target (0.10/0.25 × 16000 × 2
        // = 12_800 mm/min) must stay below it so DeflectionThreshold is
        // the *only* candidate cap.
        machine.max_feed_mm_min = 20_000.0;
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let band_max = 0.10;
        let bounds = Some(ChiploadBounds {
            min_mm_per_tooth: 0.05,
            max_mm_per_tooth: band_max,
        });

        let pre_predicted =
            crate::feeds::predict::predict_peak_deflection_um(&op, &tool, &material, &machine)
                .predicted_um;
        let refusal_um = DEFLECTION_BACKOFF_TARGET_UM - FEED_RAISE_DEFLECTION_RECAL_HEADROOM_UM;
        assert!(
            (refusal_um..DEFLECTION_BACKOFF_TARGET_UM).contains(&pre_predicted),
            "test setup: pre-loop predicted deflection ({pre_predicted:.1} µm) must sit in \
             [{refusal_um:.0}, {DEFLECTION_BACKOFF_TARGET_UM:.0}) µm — retune tool.stickout"
        );

        let initial_feed = 911.0_f64;
        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext {
                chipload_bounds: bounds,
                policy: SuggestPolicy {
                    aggressiveness: SuggestAggressiveness::Speed,
                    ..SuggestPolicy::default()
                },
                ..SuggestContext::default()
            },
        );

        let raised_cap = warnings.iter().find_map(|w| match w {
            SuggestWarning::FeedRaisedForChipload { cap_hit, .. } => Some(*cap_hit),
            _ => None,
        });
        assert_eq!(
            raised_cap.expect("FeedRaisedForChipload must fire when Speed is deflection-gated"),
            Some(FeedRecalibrationCap::DeflectionThreshold),
            "Speed gating must surface as cap_hit = DeflectionThreshold"
        );

        let still_low = warnings.iter().find_map(|w| match w {
            SuggestWarning::ChiploadStillLowAfterRecalibration {
                lut_target_mm_per_tooth,
                blocking_cap,
                ..
            } => Some((*lut_target_mm_per_tooth, *blocking_cap)),
            _ => None,
        });
        let (lut_target, blocking_cap) =
            still_low.expect("still-low must fire when Speed's target is deflection-gated");
        assert_eq!(
            blocking_cap,
            FeedRecalibrationCap::DeflectionThreshold,
            "blocking cap must be DeflectionThreshold, got {blocking_cap:?}"
        );
        assert!(
            (lut_target - band_max).abs() < 1e-9,
            "Speed must target the LUT band max ({band_max}), got {lut_target}"
        );

        // Refused write ⇒ feed reverted untouched.
        assert!(
            (op.feed_rate() - initial_feed).abs() < 1e-6,
            "op feed must be reverted after Speed deflection gate, got {}",
            op.feed_rate()
        );
    }

    /// v2 step 2 cap test (v2.1 single-shot): when the closed-form
    /// target feed exceeds `machine.max_feed_mm_min`, the recalibration
    /// clamps to the machine cap and emits `MaxFeed`. If the clamped
    /// feed still leaves observed below the LUT min,
    /// `ChiploadStillLowAfterRecalibration` fires alongside.
    ///
    /// Setup: feed already at the cap (4000 mm/min) so no feed change
    /// lands — `FeedRaisedForChipload` does NOT fire (feed_moved=false
    /// and cap_hit≠DeflectionThreshold). Only the still-low warning
    /// surfaces.
    #[test]
    fn feed_recalibration_caps_on_max_feed() {
        use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
        use crate::feeds::ChiploadBounds;
        use crate::material::WoodSpecies;

        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            // Already at the machine cap — closed-form target lands
            // at 0.10/0.25 × 16000 × 2 = 12_800 mm/min, clamped to 4000.
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
        // Feed cap pinned to the initial feed so the first raise hits
        // MaxFeed immediately.
        machine.max_feed_mm_min = 4000.0;
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        // Bound chosen so the initial predicted observed (~0.0083 for
        // 4000/16000/2 × 0.25 arc-fit ≈ 0.031 mm/tooth) still sits
        // below the band — guarantees the loop enters.
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

        let still_low = warnings.iter().find_map(|w| match w {
            SuggestWarning::ChiploadStillLowAfterRecalibration { blocking_cap, .. } => {
                Some(*blocking_cap)
            }
            _ => None,
        });
        assert_eq!(
            still_low,
            Some(FeedRecalibrationCap::MaxFeed),
            "blocking cap must be MaxFeed when initial feed is at the machine cap, got {still_low:?} (warnings: {warnings:?})"
        );
        // Op feed must equal the machine cap — solved feed clamped down.
        assert!(
            (op.feed_rate() - 4000.0).abs() < 1e-6,
            "feed must equal the machine cap after clamp, got {}",
            op.feed_rate()
        );
        // FeedRaisedForChipload must NOT fire: feed didn't move
        // (target was clamped to the current value) and cap_hit is
        // MaxFeed not DeflectionThreshold.
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::FeedRaisedForChipload { .. })),
            "FeedRaisedForChipload must not fire when initial feed already equals the machine cap, got {warnings:?}"
        );
    }

    /// v2 step 2 counter-test: when the predicted observed chipload
    /// already sits at or above the LUT minimum, the recalibration
    /// loop must skip entirely — no feed change, no warning. Guards
    /// against accidentally firing the loop on every Suggest path.
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

    /// v3.0b: same op × tool × material × machine with three
    /// SuggestAggressiveness levels — feed must be monotonically
    /// non-decreasing Conservative → Default → Speed. Validates the
    /// `target_chipload` math wires through `recalibrate_feed_for_chipload`.
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
        // Sanity: Conservative should match the v2.1 baseline closed-form
        // solve (0.027 / 0.25 × 16000 × 2 = 3456 mm/min, rounded to 1).
        assert!(
            (feed_conservative - 3456.0).abs() < 1.0,
            "Conservative policy must preserve the v2.1 closed-form target (3456 mm/min ± 1), got {feed_conservative}"
        );
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
