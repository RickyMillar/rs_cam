//! Adaptive3d strategy picking and final-geometry feed rescaling: the entry
//! style and clearing strategy auto-picks, the plunge-entry stability check,
//! and the geometry factor that rescales a feed after the invariant clamps
//! rewrite stepover or DPP.
//!
//! Split out of `feeds/suggest.rs` by P4; every item is unchanged apart from
//! its visibility and the `use` lines.

use crate::compute::catalog::OperationConfig;
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{OperationFamily as FeedsOperationFamily, PassRole, ToolGeometryHint};
use crate::machine::MachineProfile;

use super::{
    FeedRecalibrationCap, SuggestContext, SuggestScope, SuggestWarning, clamp_plunge_to_feed,
};

/// v1.3 combined-Suggest: DPP / tool-diameter ratio above which the
/// plunge-entry transient breaches the deflection gate on an
/// Adaptive-family op with `entry_style = Plunge`. Calibrated against
/// Wanaka post-sim plunge-entry spike data (2026-06-03): a 6 mm tool
/// at DPP = 3.69 mm (ratio 0.615) showed a 362 µm transient deflection
/// during plunge entry even though steady-state landed at 162 µm. A
/// ratio of ~0.5 is enough for the transient to breach the 200 µm
/// gate, so we flag at-or-above 0.5×D.
const PLUNGE_ENTRY_UNSTABLE_DPP_OVER_D: f64 = 0.5;

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
pub(super) fn pick_adaptive3d_entry_style(
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
pub(super) fn pick_adaptive3d_clearing_strategy(
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
pub(super) fn check_plunge_entry_stability(
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
pub(super) fn rescale_feed_to_final_geometry(
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
