//! F-036 / F-039 — Per-move adaptive feed modulation algorithm.
//!
//! Two strategies live behind one entry point:
//!
//! - [`ModulationStrategy::ConstrainedMax`] (F-039, default) — solves a
//!   per-move constrained-optimisation problem. Six candidate limits
//!   compete; the smallest wins, scaled by `aggressiveness`, then
//!   floored at the chipload-min band edge. Each per-move decision
//!   records the [`BindingConstraint`] that drove it so the diagnostic
//!   surface can explain *why* a given feed landed where it did.
//! - [`ModulationStrategy::BandMid`] (F-036, fallback) — the original
//!   "target band-mid × chip-thinning × clamps" heuristic. Kept as a
//!   one-release-cycle fallback so users surprised by F-039's
//!   behaviour can opt back into the legacy algorithm without
//!   reverting the build.
//!
//! ## Module surface
//!
//! - [`adaptive_feed_modulate`] mutates the toolpath's per-move
//!   `feed_rate` and returns a [`ModulationOutcome`] (move count
//!   touched, per-move map of `(feed, binding)`).
//! - [`ModulationContext`] carries the per-toolpath inputs (RPM,
//!   flutes, machine cap, LUT band, kinematics, optional
//!   deflection/power inputs).
//! - [`BindingConstraint`] is re-exported from
//!   [`crate::tool_load::BindingConstraint`] so consumers can match
//!   without an extra import.
//! - [`ChiploadBand`] and [`PerMoveEngagement`] are unchanged from
//!   F-036; the constrained-max solver consumes the same engagement
//!   summary the band-mid path always has.
//!
//! ## What this module does NOT do
//!
//! - It does not read or write G-code. The toolpath IR carries per-
//!   move `feed_rate`; G-code emission of `F<rate>` on every feed
//!   change is F-036a. F-039 layered an *optional* per-move comment
//!   onto that path; see [`PostDefinition`] / `emit_modulation_comments`.
//! - It does not call the simulator. Engagement summaries are an
//!   input.
//! - It does not modulate `MoveType::Rapid`, retract moves
//!   (`MoveIntent::Retract`), or drilling plunges (`MoveIntent::Drilling`,
//!   `MoveIntent::EntryPlunge`). The chip-thinning + force corrections
//!   are only well-defined for lateral / arc / helix engagement.
//!
//! See `planning/acceptance_loop/findings/F-039-constrained-max-feed-modulation.md`
//! for the design narrative + acceptance bars.

use std::collections::BTreeMap;

use crate::machine_kinematics::{MachineKinematics, predicted_feeds_for_toolpath};
use crate::toolpath::{MoveIntent, MoveType, Toolpath};

pub use crate::tool_load::BindingConstraint;

/// Selects which modulation algorithm runs. Stored on
/// [`crate::session::SimulationOptions`]; defaults to
/// [`Self::ConstrainedMax`] in F-039.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ModulationStrategy {
    /// F-036 "target band-mid" heuristic. Walks the chipload band's
    /// geometric mid as the target and applies the chip-thinning
    /// correction (`1 / sqrt(radial_woc_fraction)`), then clamps at
    /// the band edges and the machine cap. Kept as a fallback for
    /// one release cycle.
    BandMid,
    /// F-039 constrained-max solver (default). Computes the smallest
    /// of six candidate feed limits — chipload-max, deflection-max,
    /// power-max, machine-max, kinematic-reach, chipload-min floor —
    /// and emits the binding value scaled by `aggressiveness`.
    #[default]
    ConstrainedMax,
}

impl ModulationStrategy {
    /// Stable tag mirroring the strategy enum onto the verdict-side
    /// serialised form. Used by [`adaptive_feed_modulate`] to populate
    /// [`crate::tool_load::ModulationSummary::strategy`].
    pub fn tag(self) -> crate::tool_load::ModulationStrategyTag {
        match self {
            Self::BandMid => crate::tool_load::ModulationStrategyTag::BandMid,
            Self::ConstrainedMax => crate::tool_load::ModulationStrategyTag::ConstrainedMax,
        }
    }
}

/// Chipload band (`mm/tooth`) sourced from the vendor LUT for a given
/// tool / material pair.
///
/// `min` is the rubbing / heat-burn floor (below this the tooth scrapes
/// instead of slicing; in wood the workpiece scorches). `max` is the
/// breakage / over-load ceiling. The modulator floors emitted feeds at
/// `min × rpm × flutes` (after aggressiveness) and caps the
/// constrained-max search at `max × rpm × flutes`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChiploadBand {
    /// Minimum chipload (mm per tooth). Must be > 0 and ≤ `max`.
    pub min_mm_per_tooth: f64,
    /// Maximum chipload (mm per tooth). Must be ≥ `min`.
    pub max_mm_per_tooth: f64,
}

impl ChiploadBand {
    /// Construct a chipload band. Returns `None` if either value is
    /// non-finite, `min` is ≤ 0, or `max` < `min`.
    pub fn new(min_mm_per_tooth: f64, max_mm_per_tooth: f64) -> Option<Self> {
        if !min_mm_per_tooth.is_finite()
            || !max_mm_per_tooth.is_finite()
            || min_mm_per_tooth <= 0.0
            || max_mm_per_tooth < min_mm_per_tooth
        {
            return None;
        }
        Some(Self {
            min_mm_per_tooth,
            max_mm_per_tooth,
        })
    }

    /// Geometric midpoint of the band — the [`ModulationStrategy::BandMid`]
    /// target. Geometric (not arithmetic) so the target sits
    /// proportionally between min and max regardless of band width.
    #[inline]
    pub fn mid_mm_per_tooth(&self) -> f64 {
        (self.min_mm_per_tooth * self.max_mm_per_tooth).sqrt()
    }
}

/// Per-move engagement summary the modulator consumes.
///
/// One entry per cutting move in the toolpath (Rapids carry no
/// engagement and the modulator skips them). The two fractions match
/// the simulator's `Engagement` axes: `radial_woc_fraction` is the
/// cylinder-side radial WOC fraction (0.0 = air, 1.0 = full slot);
/// `axial_doc_fraction` is the cutter's flute-engaged-height fraction.
/// Both are time-weighted means across the move's samples.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PerMoveEngagement {
    /// Mean radial WOC fraction across the move's cutting samples.
    /// `0.0` is acceptable (the modulator treats the move as air and
    /// leaves its feed at the commanded value).
    pub radial_woc_fraction: f64,
    /// Mean axial DOC fraction across the move's cutting samples.
    /// Used by the constrained-max solver to derive deflection +
    /// power limits.
    pub axial_doc_fraction: f64,
}

/// Optional deflection-cap inputs for the constrained-max solver.
/// When `None` the deflection constraint is skipped (typical for
/// unit tests and band-mid runs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeflectionLimitInputs {
    /// Material specific cutting energy (`Kc`) in `N/mm²`.
    pub kc_n_per_mm2: f64,
    /// Tool stickout (mm) — distance from the collet face to the
    /// tool tip.
    pub stickout_mm: f64,
    /// Effective diameter at the engagement depth (mm). Drives the
    /// engagement-radius / radial-width calculation.
    pub engagement_diameter_mm: f64,
    /// Young's modulus of the tool material in `N/mm²`. Carbide
    /// ≈ 600 000, HSS ≈ 200 000.
    pub youngs_modulus_n_per_mm2: f64,
    /// Maximum allowed tip displacement (mm). F-039 uses 0.2 mm
    /// matching [`crate::tool_load::deflection::EXCEEDS_BOUND_MM`].
    pub max_tip_deflection_mm: f64,
}

/// Optional power-cap inputs for the constrained-max solver. `None`
/// disables the power constraint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerLimitInputs {
    /// Effective `Kc` (already multiplied by the anisotropy factor).
    /// `tool_load::power` uses `2.5 × material.kc_n_per_mm2()`.
    pub kc_eff_n_per_mm2: f64,
    /// Effective diameter at the engagement depth (mm).
    pub engagement_diameter_mm: f64,
    /// Available spindle power × safety factor at the running RPM
    /// (kW). The constrained-max solver caps the feed so predicted
    /// `P = Kc × DOC × WOC × feed / 60_000_000` stays inside this.
    pub available_kw: f64,
}

/// Static inputs the modulator needs to convert engagement into feed.
///
/// Kept tool/material/machine-agnostic at the struct level so a caller
/// can stage a synthetic context for a unit test without instantiating
/// the full `Tool` / `Material` / `Machine` graph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModulationContext<'a> {
    /// Spindle speed at which the toolpath is commanded (rev/min).
    /// Must be > 0.
    pub spindle_rpm: f64,
    /// Flute count of the active cutter. Must be ≥ 1.
    pub flute_count: u32,
    /// Machine's `max_feed_mm_min` hard cap.
    pub max_feed_mm_min: f64,
    /// Rapid-move feed (mm/min) the machine emits for `G0`. Used only
    /// to thread the kinematics integrator correctly; rapids
    /// themselves are not modulated.
    pub rapid_feed_mm_min: f64,
    /// LUT chipload band (mm/tooth).
    pub chipload_band: ChiploadBand,
    /// Machine kinematics — drives the `predicted_feeds_for_toolpath`
    /// per-move achievable-velocity cap.
    pub kinematics: &'a MachineKinematics,
    /// F-039 — which algorithm to run.
    pub strategy: ModulationStrategy,
    /// F-039 — aggressiveness scalar (default 1.0). Ignored by
    /// `BandMid`.
    pub aggressiveness: f64,
    /// F-039 — optional deflection-cap inputs (see
    /// [`DeflectionLimitInputs`]). `None` disables the constraint.
    pub deflection_inputs: Option<DeflectionLimitInputs>,
    /// F-039 — optional power-cap inputs (see [`PowerLimitInputs`]).
    /// `None` disables the constraint.
    pub power_inputs: Option<PowerLimitInputs>,
    /// F-039 — per-move axial DOC (mm) at full engagement. Used as
    /// the `axial_doc_mm` term when scaling deflection + power
    /// limits via `engagement.axial_doc_fraction`. Falls back to
    /// `engagement_diameter_mm` when zero/None.
    pub nominal_axial_doc_mm: f64,
}

/// Errors the modulator can return for malformed inputs. Kept small so
/// the workspace's `result_large_err` lint stays happy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModulationError {
    /// `engagements` length didn't match `toolpath.moves.len()`.
    EngagementLengthMismatch,
    /// `spindle_rpm`, `flute_count`, `max_feed_mm_min`, or `rapid_feed_mm_min`
    /// was non-positive or non-finite.
    InvalidContext,
}

/// F-039 — outcome of an [`adaptive_feed_modulate`] call.
///
/// Captures both the legacy "how many moves changed" scalar and the
/// per-move `(feed, binding_constraint)` map the new diagnostic
/// surface consumes. The map is keyed by toolpath-local move index;
/// the caller prefixes with `toolpath_id` when stamping onto
/// `SimulationCutTrace::modulated_feeds`.
#[derive(Debug, Clone, Default)]
pub struct ModulationOutcome {
    /// Number of moves whose `feed_rate` was actually rewritten.
    pub changed: usize,
    /// Per-move map of `(emitted_feed_mm_min, binding_constraint)`
    /// for every move the modulator visited (including no-ops where
    /// the emitted feed matched commanded). Skipped moves (rapids,
    /// drilling, etc.) are absent from the map.
    pub per_move: BTreeMap<usize, (f64, BindingConstraint)>,
}

impl ModulationOutcome {
    /// Build the F-039 per-toolpath
    /// [`crate::tool_load::ModulationSummary`] from this outcome and
    /// the toolpath's pre-modulation commanded feed (used to compute
    /// the median delta percentage).
    pub fn build_summary(
        &self,
        commanded_feed_mm_min: f64,
        aggressiveness: f64,
        strategy: ModulationStrategy,
    ) -> Option<crate::tool_load::ModulationSummary> {
        if self.per_move.is_empty() {
            return None;
        }
        let moves_total = self.per_move.len();
        let moves_touched = self.changed;
        let mut deltas: Vec<f64> = Vec::with_capacity(moves_touched);
        let mut binding_counts: BTreeMap<BindingConstraint, usize> = BTreeMap::new();
        for (feed, binding) in self.per_move.values() {
            if commanded_feed_mm_min > 0.0 && (feed - commanded_feed_mm_min).abs() > 0.5 {
                deltas.push((feed - commanded_feed_mm_min) / commanded_feed_mm_min * 100.0);
            }
            *binding_counts.entry(*binding).or_insert(0) += 1;
        }
        deltas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = if deltas.is_empty() {
            0.0
        } else {
            // SAFETY: deltas.len() > 0 verified just above.
            #[allow(clippy::indexing_slicing)]
            {
                let mid = deltas.len() / 2;
                if deltas.len() % 2 == 1 {
                    deltas[mid]
                } else {
                    (deltas[mid - 1] + deltas[mid]) / 2.0
                }
            }
        };
        let total = moves_total.max(1) as f64;
        let binding_constraint_distribution: BTreeMap<BindingConstraint, f64> = binding_counts
            .into_iter()
            .filter(|(_, c)| *c > 0)
            .map(|(k, c)| (k, c as f64 / total))
            .collect();
        Some(crate::tool_load::ModulationSummary {
            moves_touched,
            moves_total,
            median_feed_delta_pct: median,
            binding_constraint_distribution,
            aggressiveness,
            strategy: strategy.tag(),
        })
    }
}

/// Return `true` when the modulator should leave this move's feed alone.
fn should_skip_modulation(move_type: MoveType, intent: MoveIntent) -> bool {
    if matches!(move_type, MoveType::Rapid) {
        return true;
    }
    matches!(
        intent,
        MoveIntent::Retract
            | MoveIntent::Drilling
            | MoveIntent::EntryPlunge
            // F-040: lead-in / lead-out feeds are operator-tuned (cleaner
            // entry mark / faster chip-clear on exit). Treat them as
            // user-tuned and skip modulation — the dressup's
            // `lead_in_feed_rate` / `lead_out_feed_rate` already carries
            // the operator's intent for these moves.
            | MoveIntent::LeadIn
            | MoveIntent::LeadOut
    )
}

/// F-039 — Constrained-max per-move solver.
///
/// Computes six candidate feed caps; the smallest binds. The binding
/// constraint is returned so the diagnostic surface can name *why*
/// the move's feed landed where it did. `aggressiveness` scales the
/// minimum *before* the chipload-min floor; values above 1.0 push past
/// the constraint and the chipload-min floor still applies.
fn max_safe_feed_for_move(
    engagement: PerMoveEngagement,
    ctx: &ModulationContext<'_>,
    predicted_cap_mm_min: f64,
) -> (f64, BindingConstraint) {
    let flutes = ctx.flute_count.max(1) as f64;
    let band = ctx.chipload_band;

    // Effective WOC fraction: clamp to a small floor so chip-thinning
    // doesn't blow up at near-zero engagement. The simulator's air-
    // cut samples are gated out upstream; this protects against
    // single-sample noise.
    let woc_eff = engagement.radial_woc_fraction.clamp(1e-3, 1.0);
    let chip_thinning_inv = woc_eff.sqrt().max(1e-6);
    let target_chipload = band.max_mm_per_tooth / chip_thinning_inv;

    let mut limits: Vec<(f64, BindingConstraint)> = Vec::with_capacity(6);

    // 1. Chipload-max constraint (with chip-thinning correction).
    limits.push((
        target_chipload * ctx.spindle_rpm * flutes,
        BindingConstraint::ChiploadMax,
    ));

    // 2. Deflection cap. Force scales linearly with feed (force =
    // Kc × axial × WOC), and tip deflection scales linearly with
    // force, so the inverse relationship lets us solve for the feed
    // that hits the max deflection bound.
    if let Some(defl) = ctx.deflection_inputs {
        let axial_mm = effective_axial_mm(engagement, ctx);
        if axial_mm > 0.0 && woc_eff > 0.0 {
            let radial_width =
                (woc_eff * std::f64::consts::PI).min(std::f64::consts::PI)
                    / std::f64::consts::PI
                    * defl.engagement_diameter_mm.max(0.0);
            // Reference force at this engagement geometry.
            let ref_force_n = defl.kc_n_per_mm2 * axial_mm * radial_width.max(1e-6);
            if ref_force_n > 0.0 {
                // Use the cutter's stepped-cantilever closed form:
                // δ_ref = tip_deflection(ref_force, axial_mm, E).
                // Since δ ∝ F ∝ feed (Kc and geometry held fixed),
                // the safe feed is the chipload feed at deflection
                // bound scaled by the linear relationship. The
                // reference force here is *not* feed-scaled — it's the
                // force at full chip thickness × full geometry. The
                // deflection limit corresponds to that reference
                // force directly; once force exceeds the cap, no
                // feed will rescue it. So we use δ_ref vs the bound
                // as a multiplier: if δ_ref <= bound, no deflection
                // cap (feed = chipload_max). If δ_ref > bound, feed
                // must shrink proportionally to δ_ref / bound (since
                // tip displacement is linear in force and force is
                // linear in chipload, which is linear in feed).
                let delta_ref_mm = simple_tip_deflection(
                    ref_force_n,
                    axial_mm,
                    defl.stickout_mm,
                    defl.engagement_diameter_mm,
                    defl.youngs_modulus_n_per_mm2,
                );
                if delta_ref_mm > defl.max_tip_deflection_mm && delta_ref_mm.is_finite() {
                    let scale = defl.max_tip_deflection_mm / delta_ref_mm;
                    let defl_cap = target_chipload * ctx.spindle_rpm * flutes * scale.max(0.0);
                    limits.push((defl_cap, BindingConstraint::DeflectionMax));
                }
            }
        }
    }

    // 3. Power cap. `P_kW = Kc_eff × DOC × WOC × feed / 60_000_000`.
    // Solve for the feed that hits `available_kw`.
    if let Some(pow) = ctx.power_inputs
        && pow.available_kw > 0.0
    {
        let axial_mm = effective_axial_mm(engagement, ctx);
        if axial_mm > 0.0 && woc_eff > 0.0 {
            let radial_width = woc_eff * pow.engagement_diameter_mm.max(0.0);
            if radial_width > 0.0 {
                let pow_cap =
                    pow.available_kw * 60_000_000.0 / (pow.kc_eff_n_per_mm2 * axial_mm * radial_width);
                if pow_cap.is_finite() {
                    limits.push((pow_cap, BindingConstraint::PowerMax));
                }
            }
        }
    }

    // 4. Machine-feed hard cap.
    limits.push((ctx.max_feed_mm_min, BindingConstraint::MachineMaxFeed));

    // 5. Kinematic-reach cap.
    if predicted_cap_mm_min.is_finite() && predicted_cap_mm_min > 0.0 {
        limits.push((predicted_cap_mm_min, BindingConstraint::KinematicReach));
    }

    // Pick the smallest cap.
    let (limit, binding) = limits
        .into_iter()
        .filter(|(v, _)| v.is_finite() && *v > 0.0)
        .min_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((ctx.max_feed_mm_min, BindingConstraint::MachineMaxFeed));

    let aggr = ctx.aggressiveness.max(0.0);
    let after_aggr = limit * aggr;

    // 6. Chipload-min floor (rubbing protection). Applied last so
    // aggressiveness can't drop feeds below the safe floor. When
    // the floor itself sits above the machine's hard cap (rare:
    // machine `max_feed` configured below `band.min × rpm × flutes`)
    // the cap wins — emitting above the cap would crash the
    // controller, which is strictly worse than burning the
    // workpiece. The machine constraint stays load-bearing.
    //
    // Binding-tag rule (per F-039 spec): re-label as `ChiploadMin`
    // only when aggressiveness scaled the limit BELOW the floor
    // (`emitted == floor && limit > floor`). When the *underlying*
    // limit itself sits below the floor (deflection or power forced
    // a low feed even at aggressiveness 1.0), keep the original
    // binding so the diagnostic surface names the load-bearing
    // physical constraint, not the floor we backed off to.
    let floor = band.min_mm_per_tooth * ctx.spindle_rpm * flutes;
    let effective_floor = floor.min(ctx.max_feed_mm_min);
    if after_aggr < effective_floor {
        let new_binding = if limit > effective_floor {
            BindingConstraint::ChiploadMin
        } else {
            binding
        };
        (effective_floor, new_binding)
    } else {
        (after_aggr, binding)
    }
}

fn effective_axial_mm(engagement: PerMoveEngagement, ctx: &ModulationContext<'_>) -> f64 {
    let axial = ctx.nominal_axial_doc_mm.max(0.0);
    if axial > 0.0 {
        axial * engagement.axial_doc_fraction.clamp(0.0, 1.0)
    } else {
        engagement.axial_doc_fraction.clamp(0.0, 1.0)
            * ctx
                .deflection_inputs
                .map(|d| d.engagement_diameter_mm)
                .or_else(|| ctx.power_inputs.map(|p| p.engagement_diameter_mm))
                .unwrap_or(0.0)
    }
}

/// Simple stepped-cantilever tip-deflection closed form for the
/// constrained-max solver. Treats the tool as a uniform cantilever of
/// length `stickout_mm` and section diameter `diameter_mm` with the
/// load applied at the midpoint of the engaged axial length.
/// Returns the tip displacement in mm. Used as a *relative* scaling
/// factor — its absolute accuracy matters less than its proportionality
/// to force.
fn simple_tip_deflection(
    force_n: f64,
    axial_mm: f64,
    stickout_mm: f64,
    diameter_mm: f64,
    youngs_modulus_n_per_mm2: f64,
) -> f64 {
    if stickout_mm <= 0.0 || diameter_mm <= 0.0 || youngs_modulus_n_per_mm2 <= 0.0 {
        return 0.0;
    }
    let radius = diameter_mm / 2.0;
    // Second moment of area for solid cylinder.
    let i = std::f64::consts::PI * radius.powi(4) / 4.0;
    // Load applied at midpoint of engagement region from the tip.
    let a = stickout_mm - axial_mm / 2.0;
    if a <= 0.0 {
        return 0.0;
    }
    // δ = F · a² · (3·L − a) / (6·E·I) for a point load at distance a
    // from the fixed end (collet face), measured at the free end (tip)
    // — standard cantilever deflection formula.
    force_n * a * a * (3.0 * stickout_mm - a) / (6.0 * youngs_modulus_n_per_mm2 * i)
}

/// F-036 — "target band-mid" per-move feed (legacy heuristic).
fn band_mid_feed_for_move(
    commanded_feed_mm_min: f64,
    engagement: PerMoveEngagement,
    predicted_cap_mm_min: f64,
    ctx: &ModulationContext<'_>,
) -> (f64, BindingConstraint) {
    if engagement.radial_woc_fraction <= 1e-6 && engagement.axial_doc_fraction <= 1e-6 {
        return (commanded_feed_mm_min, BindingConstraint::ChiploadMax);
    }
    let woc = engagement.radial_woc_fraction.clamp(1e-3, 1.0);
    let thinning = woc.sqrt().max(1e-6);
    let target_chipload = ctx.chipload_band.mid_mm_per_tooth() / thinning;
    let flutes = ctx.flute_count.max(1) as f64;
    let base_feed = target_chipload * ctx.spindle_rpm * flutes;

    let band_floor = ctx.chipload_band.min_mm_per_tooth * ctx.spindle_rpm * flutes;
    let band_ceiling = ctx.chipload_band.max_mm_per_tooth * ctx.spindle_rpm * flutes;

    let cap = ctx
        .max_feed_mm_min
        .min(band_ceiling)
        .min(predicted_cap_mm_min.max(band_floor));
    if cap < band_floor {
        return (cap.max(0.0), BindingConstraint::MachineMaxFeed);
    }
    let clamped = base_feed.clamp(band_floor, cap);
    let binding = if (clamped - band_ceiling).abs() < 1e-6 {
        BindingConstraint::ChiploadMax
    } else if (clamped - band_floor).abs() < 1e-6 {
        BindingConstraint::ChiploadMin
    } else if (clamped - ctx.max_feed_mm_min).abs() < 1e-6 {
        BindingConstraint::MachineMaxFeed
    } else if (clamped - predicted_cap_mm_min).abs() < 1e-6 {
        BindingConstraint::KinematicReach
    } else {
        BindingConstraint::ChiploadMax
    };
    (clamped, binding)
}

/// Apply per-move adaptive feed modulation to `toolpath`.
///
/// Strategy selection: [`ModulationContext::strategy`].
///
/// Returns a [`ModulationOutcome`] with the per-move binding-
/// constraint map. The map is suitable for stamping onto
/// [`crate::simulation_cut::SimulationCutTrace::modulated_feeds`]
/// after prefixing with the toolpath id.
pub fn adaptive_feed_modulate(
    toolpath: &mut Toolpath,
    engagements: &[PerMoveEngagement],
    ctx: &ModulationContext<'_>,
) -> Result<ModulationOutcome, ModulationError> {
    if engagements.len() != toolpath.moves.len() {
        return Err(ModulationError::EngagementLengthMismatch);
    }
    if !ctx.spindle_rpm.is_finite()
        || ctx.spindle_rpm <= 0.0
        || ctx.flute_count == 0
        || !ctx.max_feed_mm_min.is_finite()
        || ctx.max_feed_mm_min <= 0.0
        || !ctx.rapid_feed_mm_min.is_finite()
        || ctx.rapid_feed_mm_min <= 0.0
    {
        return Err(ModulationError::InvalidContext);
    }

    // Predicted achievable feed per move under the machine's accel /
    // junction limits. As in F-036b, rebuild a synthetic toolpath
    // commanded at the maximum candidate feed so the integrator
    // returns the geometric reach, not commanded-clipped reach.
    let band_ceiling_feed =
        ctx.chipload_band.max_mm_per_tooth * ctx.spindle_rpm * ctx.flute_count.max(1) as f64;
    let probe_feed = ctx.max_feed_mm_min.min(band_ceiling_feed).max(1e-3);
    let mut probe = toolpath.clone();
    for m in probe.moves.iter_mut() {
        m.move_type = match m.move_type {
            MoveType::Linear { .. } => MoveType::Linear { feed_rate: probe_feed },
            MoveType::ArcCW { i, j, .. } => MoveType::ArcCW { i, j, feed_rate: probe_feed },
            MoveType::ArcCCW { i, j, .. } => MoveType::ArcCCW { i, j, feed_rate: probe_feed },
            MoveType::Rapid => MoveType::Rapid,
        };
    }
    let predicted = predicted_feeds_for_toolpath(
        &probe,
        ctx.kinematics,
        ctx.max_feed_mm_min,
        ctx.rapid_feed_mm_min,
    );

    let mut outcome = ModulationOutcome::default();
    #[allow(clippy::indexing_slicing)]
    // SAFETY: `i` bounded by `engagements.len()`, which we just
    // verified equals `toolpath.moves.len()`.
    for (i, &engagement) in engagements.iter().enumerate() {
        let move_intent = toolpath.moves[i].intent;
        let move_type = toolpath.moves[i].move_type;
        if should_skip_modulation(move_type, move_intent) {
            continue;
        }
        let Some(commanded) = move_type.feed_rate() else {
            continue;
        };
        let predicted_cap = predicted
            .get(&i)
            .copied()
            .unwrap_or(ctx.max_feed_mm_min)
            .max(1e-3);

        let (new_feed, binding) = match ctx.strategy {
            ModulationStrategy::BandMid => {
                band_mid_feed_for_move(commanded, engagement, predicted_cap, ctx)
            }
            ModulationStrategy::ConstrainedMax => {
                if engagement.radial_woc_fraction <= 1e-6
                    && engagement.axial_doc_fraction <= 1e-6
                {
                    // No engagement — leave the commanded feed
                    // alone. Record the per-move entry so the
                    // diagnostic surface can still see it.
                    (commanded, BindingConstraint::MachineMaxFeed)
                } else {
                    max_safe_feed_for_move(engagement, ctx, predicted_cap)
                }
            }
        };

        outcome.per_move.insert(i, (new_feed, binding));
        if (new_feed - commanded).abs() > 1e-6 {
            let new_move_type = match move_type {
                MoveType::Linear { .. } => MoveType::Linear { feed_rate: new_feed },
                MoveType::ArcCW { i: ai, j: aj, .. } => {
                    MoveType::ArcCW { i: ai, j: aj, feed_rate: new_feed }
                }
                MoveType::ArcCCW { i: ai, j: aj, .. } => {
                    MoveType::ArcCCW { i: ai, j: aj, feed_rate: new_feed }
                }
                MoveType::Rapid => MoveType::Rapid,
            };
            toolpath.moves[i].move_type = new_move_type;
            outcome.changed += 1;
        }
    }
    Ok(outcome)
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
    use crate::geo::P3;
    use crate::toolpath::Toolpath;

    fn shapeoko() -> MachineKinematics {
        MachineKinematics::shapeoko_xxl_stock()
    }

    fn make_ctx<'a>(k: &'a MachineKinematics, band: ChiploadBand) -> ModulationContext<'a> {
        ModulationContext {
            spindle_rpm: 18_000.0,
            flute_count: 2,
            max_feed_mm_min: 4000.0,
            rapid_feed_mm_min: 5000.0,
            chipload_band: band,
            kinematics: k,
            strategy: ModulationStrategy::BandMid,
            aggressiveness: 1.0,
            deflection_inputs: None,
            power_inputs: None,
            nominal_axial_doc_mm: 0.0,
        }
    }

    fn band() -> ChiploadBand {
        ChiploadBand::new(0.02, 0.08).unwrap()
    }

    fn straight_toolpath(n_cuts: usize, feed_mm_min: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        for i in 0..n_cuts {
            let x = (i + 1) as f64 * 50.0;
            tp.feed_to_with_intent(
                P3::new(x, 0.0, -2.0),
                feed_mm_min,
                MoveIntent::ClearingCut,
            );
        }
        tp
    }

    fn corner_heavy_toolpath(feed_mm_min: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        let mut x = 0.0;
        let mut y = 0.0;
        for i in 0..8 {
            if i % 2 == 0 {
                x += 2.0;
            } else {
                y += 2.0;
            }
            tp.feed_to_with_intent(
                P3::new(x, y, -2.0),
                feed_mm_min,
                MoveIntent::ClearingCut,
            );
        }
        tp
    }

    #[test]
    fn chipload_band_rejects_invalid() {
        assert!(ChiploadBand::new(0.0, 0.05).is_none());
        assert!(ChiploadBand::new(0.05, 0.02).is_none());
        assert!(ChiploadBand::new(f64::NAN, 0.05).is_none());
        assert!(ChiploadBand::new(-0.01, 0.05).is_none());
    }

    #[test]
    fn chipload_band_mid_is_geometric_mean() {
        let b = ChiploadBand::new(0.02, 0.08).unwrap();
        assert!((b.mid_mm_per_tooth() - 0.04).abs() < 1e-9);
    }

    #[test]
    fn engagement_length_mismatch_errors() {
        let mut tp = straight_toolpath(3, 1500.0);
        let engagements = vec![PerMoveEngagement::default(); tp.moves.len() - 1];
        let k = shapeoko();
        let ctx = make_ctx(&k, band());
        let err = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err();
        assert_eq!(err, ModulationError::EngagementLengthMismatch);
    }

    #[test]
    fn invalid_context_errors() {
        let mut tp = straight_toolpath(2, 1500.0);
        let engagements = vec![PerMoveEngagement::default(); tp.moves.len()];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.spindle_rpm = 0.0;
        assert_eq!(
            adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err(),
            ModulationError::InvalidContext
        );
        let mut ctx = make_ctx(&k, band());
        ctx.flute_count = 0;
        assert_eq!(
            adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err(),
            ModulationError::InvalidContext
        );
        let mut ctx = make_ctx(&k, band());
        ctx.max_feed_mm_min = f64::NAN;
        assert_eq!(
            adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err(),
            ModulationError::InvalidContext
        );
    }

    /// All-air engagement: feed must stay byte-identical (no modulation
    /// triggered). Invariant for both strategies.
    #[test]
    fn zero_engagement_leaves_feed_unchanged() {
        for strategy in [ModulationStrategy::BandMid, ModulationStrategy::ConstrainedMax] {
            let mut tp = straight_toolpath(4, 1500.0);
            let engagements = vec![PerMoveEngagement::default(); tp.moves.len()];
            let k = shapeoko();
            let mut ctx = make_ctx(&k, band());
            ctx.strategy = strategy;
            let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
            assert_eq!(
                outcome.changed, 0,
                "no engagement should produce no modulation under {strategy:?}"
            );
            for m in &tp.moves {
                if let Some(f) = m.move_type.feed_rate() {
                    assert!(
                        (f - 1500.0).abs() < 1e-9,
                        "feed unchanged under {strategy:?}: got {f}"
                    );
                }
            }
        }
    }

    /// Skipped intents (Retract, Drilling, EntryPlunge) keep their
    /// commanded feed regardless of engagement summary or strategy.
    #[test]
    fn skipped_intents_keep_commanded_feed() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to_with_intent(P3::new(0.0, 0.0, -3.0), 300.0, MoveIntent::EntryPlunge);
        tp.feed_to_with_intent(P3::new(0.0, 0.0, -6.0), 300.0, MoveIntent::Drilling);
        tp.feed_to_with_intent(P3::new(0.0, 0.0, 5.0), 1500.0, MoveIntent::Retract);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement { radial_woc_fraction: 1.0, axial_doc_fraction: 1.0 },
            PerMoveEngagement { radial_woc_fraction: 1.0, axial_doc_fraction: 1.0 },
            PerMoveEngagement { radial_woc_fraction: 1.0, axial_doc_fraction: 1.0 },
        ];
        let k = shapeoko();
        let ctx = make_ctx(&k, band());
        let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        assert_eq!(outcome.changed, 0);
        assert!((tp.moves[1].move_type.feed_rate().unwrap() - 300.0).abs() < 1e-9);
        assert!((tp.moves[2].move_type.feed_rate().unwrap() - 300.0).abs() < 1e-9);
        assert!((tp.moves[3].move_type.feed_rate().unwrap() - 1500.0).abs() < 1e-9);
    }

    /// BandMid: full-slot engagement → mid_band × RPM × flutes.
    #[test]
    fn band_mid_full_slot_targets_mid_band_feed() {
        let mut tp = straight_toolpath(3, 1500.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement { radial_woc_fraction: 1.0, axial_doc_fraction: 1.0 },
            PerMoveEngagement { radial_woc_fraction: 1.0, axial_doc_fraction: 1.0 },
            PerMoveEngagement { radial_woc_fraction: 1.0, axial_doc_fraction: 1.0 },
        ];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::BandMid;
        let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        assert!(outcome.changed >= 1);
        for m in &tp.moves {
            if !matches!(m.move_type, MoveType::Rapid) {
                let f = m.move_type.feed_rate().unwrap();
                // 0.04 × 18000 × 2 = 1440.
                assert!((f - 1440.0).abs() < 5.0, "expected ~1440, got {f}");
            }
        }
    }

    /// BandMid: light engagement clamps at band-ceiling feed.
    #[test]
    fn band_mid_light_engagement_clamps_at_band_ceiling() {
        let mut tp = straight_toolpath(2, 1500.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement { radial_woc_fraction: 0.1, axial_doc_fraction: 1.0 },
            PerMoveEngagement { radial_woc_fraction: 0.1, axial_doc_fraction: 1.0 },
        ];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::BandMid;
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        let f = tp.moves[1].move_type.feed_rate().unwrap();
        // 0.08 × 18000 × 2 = 2880.
        assert!((f - 2880.0).abs() < 5.0, "expected band ceiling 2880, got {f}");
    }

    /// machine-max-feed cap wins.
    #[test]
    fn machine_max_feed_cap_wins() {
        let mut tp = straight_toolpath(2, 1500.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement { radial_woc_fraction: 0.1, axial_doc_fraction: 1.0 },
            PerMoveEngagement { radial_woc_fraction: 0.1, axial_doc_fraction: 1.0 },
        ];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.max_feed_mm_min = 500.0;
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        for m in &tp.moves {
            if let Some(f) = m.move_type.feed_rate() {
                assert!(f <= 500.0 + 1e-6, "max_feed cap violated: {f}");
            }
        }
    }

    /// Modulated feed never falls below band.min × RPM × flutes.
    #[test]
    fn modulation_never_emits_below_min_chipload() {
        let mut tp = straight_toolpath(3, 4000.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement { radial_woc_fraction: 1.0, axial_doc_fraction: 1.0 },
            PerMoveEngagement { radial_woc_fraction: 1.0, axial_doc_fraction: 1.0 },
            PerMoveEngagement { radial_woc_fraction: 1.0, axial_doc_fraction: 1.0 },
        ];
        let k = shapeoko();
        let degenerate = ChiploadBand::new(0.05, 0.05).unwrap();
        let mut ctx = make_ctx(&k, degenerate);
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        // 0.05 × 18000 × 2 = 1800 mm/min.
        for m in &tp.moves {
            if let Some(f) = m.move_type.feed_rate() {
                assert!(f >= 1800.0 - 1e-6, "floor violated: {f}");
                assert!(f <= 1800.0 + 1e-6, "ceiling violated: {f}");
            }
        }
    }

    /// F-036b retained invariant: corner-heavy toolpath respects the
    /// predicted-feed cap.
    #[test]
    fn modulation_respects_predicted_feed_cap() {
        let mut tp = corner_heavy_toolpath(4000.0);
        let engagements: Vec<_> = (0..tp.moves.len())
            .map(|_| PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
            })
            .collect();
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        let predicted_pre = predicted_feeds_for_toolpath(&tp, &k, ctx.max_feed_mm_min, 5000.0);
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        for (idx, m) in tp.moves.iter().enumerate() {
            if matches!(m.move_type, MoveType::Rapid) {
                continue;
            }
            let f = m.move_type.feed_rate().unwrap();
            if let Some(&cap) = predicted_pre.get(&idx) {
                let band_floor = ctx.chipload_band.min_mm_per_tooth
                    * ctx.spindle_rpm
                    * ctx.flute_count as f64;
                let effective_cap = cap.max(band_floor);
                assert!(
                    f <= effective_cap + 1.0,
                    "move {idx} feed {f} exceeds predicted cap {cap} (floor={band_floor})"
                );
            }
        }
    }

    /// Per-segment variation invariant (F-036b retained): modulated
    /// path has ≥ 2 distinct feeds under heterogeneous engagement.
    #[test]
    fn modulation_produces_per_segment_feed_variation() {
        for strategy in [ModulationStrategy::BandMid, ModulationStrategy::ConstrainedMax] {
            let mut tp = corner_heavy_toolpath(2000.0);
            let engagements: Vec<_> = (0..tp.moves.len())
                .map(|i| {
                    if matches!(tp.moves[i].move_type, MoveType::Rapid) {
                        PerMoveEngagement::default()
                    } else if i % 2 == 0 {
                        PerMoveEngagement { radial_woc_fraction: 1.0, axial_doc_fraction: 1.0 }
                    } else {
                        PerMoveEngagement { radial_woc_fraction: 0.2, axial_doc_fraction: 1.0 }
                    }
                })
                .collect();
            let k = shapeoko();
            let mut ctx = make_ctx(&k, band());
            ctx.strategy = strategy;
            adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
            let mut feeds: Vec<f64> = tp
                .moves
                .iter()
                .filter_map(|m| m.move_type.feed_rate())
                .collect();
            feeds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            feeds.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            assert!(
                feeds.len() >= 2,
                "expected >= 2 distinct feeds under {strategy:?}, got {feeds:?}"
            );
        }
    }
}
