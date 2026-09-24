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
use crate::material::Material;

use super::invariants::clamp_plunge_to_feed;
use super::{FeedRecalibrationCap, SuggestContext, SuggestScope, SuggestWarning};

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
    // The step ladder (D7): a coarse level enters at its own step, so the
    // entry plunges the deepest step.
    let Some(dpp) = operation.deepest_axial_step() else {
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
/// fires when `deepest_axial_step()` is `Some` (the deepest step of a 3D
/// Rough step ladder, else `depth_per_pass`).
pub(super) fn check_plunge_entry_stability(
    operation: &OperationConfig,
    tool: &ToolConfig,
    pass_role: PassRole,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    // The step ladder (D7): the deepest step is the deepest plunge.
    if let Some(current) = operation.deepest_axial_step()
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
fn geometry_feed_factor(geom: ToolGeometryHint, tool: &ToolConfig, _ae_mm: f64, ap_mm: f64) -> f64 {
    // Feeds matrix R2 (2026-09-23): the ladder diameter is the one the
    // calculator's Step 5a reads. The shank is the one
    // `suggest_for_operation` puts in `FeedsInput::shank_diameter`.
    let ladder_d = crate::feeds::geometry::feed_ladder_diameter_mm(
        geom,
        ap_mm,
        tool.diameter,
        tool.shank_diameter,
    );
    crate::feeds::geometry::depth_tier_multiplier(ap_mm, ladder_d)
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
/// (target chipload, RPM, flute count, the L/D overhang derate, the power
/// derate, the safety factor) is independent of
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
/// # What re-checks the power ceiling
///
/// [`recheck_power_after_rescale`] — pass 10, T-15, 2026-09-18. Required power
/// scales with both the feed and the cross-section, and the cross-section moves
/// with `ae` / `ap`, so a rescale can invalidate a power-limited feed. Pass 10
/// re-evaluates the canonical power model at the final operating point and
/// lowers the feed onto the ceiling when the feed does not fit.
///
/// Until T-15 this pass skipped that check, and its doc gave a measurement as
/// the reason: `tests/power_ceiling_parity_f2.rs` reported the power branch
/// never firing at all, peak utilisation 23.6 %. That figure was taken before
/// R1 rebuilt the power model about 8.6× higher. It is withdrawn as stale, not
/// re-measured.
///
/// # What it deliberately does not re-check
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
    // The step ladder (D7): one feed serves every level, so the feed is
    // re-derived at the deepest step. With a ladder `entry_dpp` is the
    // calculator's depth (`enforce_invariants` sets it so; the apply funnel
    // keeps the operator's steps, ruling 1 of 2026-09-24), so a ladder
    // reads as a moved depth and the depth-tier factor of the deepest step
    // applies. With no ladder the deepest step is `depth_per_pass`.
    let final_ap = operation
        .deepest_axial_step()
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
    //
    // T-18 (2026-09-18) read the ceiling on the COMMANDED axis, after the
    // safety factor. Ruling R4 (2026-09-24) removed the factor, so the
    // commanded and the cutting ceilings are one number: the highest feed the
    // calculator can emit.
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

    // Calculator Step 9b, re-applied as a warning. A downward re-derivation
    // (a raised stepover that cancels a chip-thinning lift) can put the
    // commanded advance under the chip-formation threshold. Ruling R4 WP2a
    // (2026-09-23): the pass warns and does NOT raise the feed, the same as
    // Step 9b. The operator has the levers: raise the feed or lower the RPM.
    //
    // The band read is `context.chipload_bounds`, which pass 0 already
    // re-derated against its own DPP mutation. A DPP that the rigidity,
    // cutting-length or deflection clamps lowered further is *not* re-derated
    // there, so the floor can read a slightly lower band minimum than the
    // final DOC deserves. That can only lower the floor, so the warning can
    // only fire less often, not more.
    if let Some(rpm) = op_rpm {
        let divisor = rpm * flutes;
        if divisor > 0.0 {
            let commanded = rescaled / divisor;
            // A2: with no band, the floor reads the printed point of the
            // matched row at the deepest step that ships.
            let point = operation.deepest_axial_step().and_then(|dpp| {
                super::axial_envelope::chip_point_for_dpp(
                    context.matched_lut_row,
                    context.effective_diameter_mm,
                    operation,
                    dpp,
                )
            });
            let (floor, source) = crate::feeds::rubbing_floor_at(context.chipload_bounds, point);
            if commanded > 0.0 && commanded < floor {
                warnings.push(SuggestWarning::FeedClampedToChiploadFloor {
                    requested_mm_per_tooth: commanded,
                    floor_mm_per_tooth: floor,
                    source,
                });
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

/// Pass 10 (T-15, 2026-09-18): **re-check the spindle power ceiling at the
/// operating point the operation ships.**
///
/// Calculator Step 6 sizes the power check at the `ap` / `ae` it was handed.
/// Pass 9 above then re-multiplies the feed by `depth_tier_multiplier` at the
/// FINAL depth, and that multiplier RISES as the depth falls — 1.00 / 0.75 /
/// 0.50 / 0.45 at `ap/D` of 1 / 2 / 3. So a clamp that lowers the depth across
/// a tier boundary makes pass 9 RAISE the feed, by up to 2.22×, for a depth
/// loss that can be arbitrarily small. Until this pass nothing re-checked
/// Step 6, and the shipped recipe could draw more power than the spindle has.
///
/// # When it runs
///
/// Only when pass 9 actually moved the feed — [`enforce_invariants`] gates on
/// [`SuggestWarning::FeedRescaledToFinalGeometry`]. An operation no pass
/// touched keeps its feed **byte-identical**, which is the same contract pass
/// 9 holds and which `tests/suggest_feed_matches_final_geometry.rs` pins.
///
/// # What it evaluates
///
/// The ONE power model, [`crate::tool_load::power::PowerTerms`] — the same
/// terms `feeds::calculate` assembles in `power_model_terms` and the same ones
/// the post-simulation gate `tool_load::power::evaluate` reads. Suggest reads
/// it through the public door [`crate::feeds::power_at_operating_point`] (S2,
/// 2026-09-18), which is where the list below now lives. Suggest does not hold
/// the calculator's `FeedsInput`, so the door rebuilds the inputs from the
/// operation and the tool rather than re-modelling them:
///
/// - `ap` / `ae` — the operation's final depth and stepover, with the
///   calculator's own values as the fallback for an operation that carries no
///   such field (the same fallback pass 9 uses, and it cancels exactly).
/// - `effective_d` — [`crate::feeds::effective_diameter`] at the FINAL depth,
///   the chip-thinning contact circle Step 5 computes. It sets both the
///   immersion angle ψ and the cutting velocity `Vc = π·D·n`.
/// - ψ — [`crate::feeds::force::immersion_angle`], one immersion definition
///   for the whole engine.
/// - the shank — `tool.shank_diameter`, which is what `suggest_for_operation`
///   already puts in `FeedsInput::shank_diameter`. It reaches
///   `effective_diameter` on the tapered-ball arm only.
///
/// # The ceiling, and the axis
///
/// `machine.power_at_rpm(rpm)`, the rated curve — what Step 6 names
/// `gate_available_power` and what `tool_load::power::evaluate` compares
/// against. Ruling R4 Q2 (2026-09-24) removed the `safety_factor` fraction
/// from both sides, so the comparison is direct.
///
/// # The clamp
///
/// Feed-only and downward. The geometry is final here, and required power is
/// affine in the feed — the shear term scales with it, the edge term does not
/// — so [`crate::feeds::PowerFigure::feed_for_kw`] solves the feed at which
/// required equals the ceiling in closed form. No bisection is needed, and
/// the solved feed is exact rather than converged.
///
/// When `feed_for_kw` returns `None` the feed-free edge term alone is at or
/// over the ceiling and no feed fits. The feed then goes back to the value
/// pass 9 started from — a value Step 6 validated at the calculator's geometry
/// — and the warning carries `fits_at_any_feed: false`. Raising the feed on a
/// cut the spindle cannot turn would trade one wrong number for another. The
/// pass never ships a feed above a value some ceiling check has passed, so it
/// takes the lower of the two when pass 9 moved the feed DOWN.
///
/// # What it does not do
///
/// It does not raise a clamped feed to the Step 9b rubbing floor. Since ruling
/// R4 WP2a (2026-09-23) no step raises a feed to the floor; the floor only
/// warns. The power ceiling is a physical limit and the floor is an advisory
/// band, and both warnings stand so the operator sees both facts.
///
/// It abstains when the material publishes no `Kc`. There is no power model
/// without one, and Step 6 did not check a ceiling either, so there is no
/// ceiling here to re-check — fabricating one would be worse than the absence.
///
/// # The single guard
///
/// A further post-rescale ceiling re-check belongs HERE, beside this one, at
/// the end of `enforce_invariants` where the last lift has run.
pub(super) fn recheck_power_after_rescale(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pre_rescale_feed_mm_min: f64,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    let usable = |v: f64| v.is_finite() && v > 0.0;

    // The evaluation itself is the public door `feeds::power_at_operating_point`
    // (S2, 2026-09-18). It reads exactly what this pass read when T-15 landed
    // it — the operation's final `ap` / `ae` / RPM with the calculator's point
    // as the fallback, the chip-thinning effective diameter at the final
    // depth, and the ceiling `power_at_rpm` (no fraction since ruling R4 Q2).
    // Pass 10 keeps the
    // decision: the clamp, the fallback feed and the warning.
    let figure = match crate::feeds::power_at_operating_point(
        operation,
        tool,
        material,
        machine,
        context.calculator_operating_point,
    ) {
        Ok(figure) => figure,
        Err(crate::feeds::PowerUnmodeled::MaterialUnvalidated) => {
            tracing::debug!(
                reason = "no_kc",
                "Suggest pass 10 abstains — the material publishes no Kc, so Step 6 \
                 applied no power ceiling and there is none to re-check"
            );
            return warnings;
        }
        Err(_) => return warnings,
    };
    let rescaled = figure.feed_mm_min;
    let ceiling = figure.available_kw;

    let required = figure.required_kw;
    if required <= ceiling {
        return warnings;
    }

    let (shipped, fits_at_any_feed) = match figure.feed_for_kw(ceiling) {
        // `feed_for_kw` answers on the same COMMANDED axis the feed sits on,
        // so it is written straight back. It is below `rescaled` by
        // construction: required is increasing in the feed and it is over the
        // ceiling at `rescaled`.
        Some(cap) if usable(cap) => (rescaled.min(cap), true),
        _ => (rescaled.min(pre_rescale_feed_mm_min.max(0.0)), false),
    };

    tracing::debug!(
        rescaled_mm_per_min = rescaled,
        shipped_mm_per_min = shipped,
        required_kw = required,
        available_kw = ceiling,
        fits_at_any_feed,
        "Suggest pass 10 re-checked the power ceiling at the final geometry"
    );
    warnings.push(SuggestWarning::PowerRecheckedAfterRescale {
        rescaled_mm_per_min: rescaled,
        shipped_mm_per_min: shipped,
        required_kw_at_rescaled: required,
        available_kw: ceiling,
        fits_at_any_feed,
    });

    if usable(shipped) && shipped < rescaled {
        operation.set_feed_rate(shipped);
        // Pass 9 re-established this invariant after IT moved the feed, for
        // the same reason: a lower feed can leave the plunge rate above it.
        warnings.extend(clamp_plunge_to_feed(operation));
    }
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
