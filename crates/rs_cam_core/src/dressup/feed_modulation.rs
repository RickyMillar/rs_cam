//! F-036 / F-039 — Per-move adaptive feed modulation algorithm.
//!
//! Two strategies live behind one entry point:
//!
//! - [`ModulationStrategy::ConstrainedMax`] (F-039, default) — solves a
//!   per-move constrained-optimisation problem. Six candidate limits
//!   compete; the smallest wins, scaled by `feed_scale`, then
//!   floored at the chipload-min band edge. A printed point
//!   ([`ChiploadBand::point`], A2) gets no floor. Each per-move decision
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
//! ## The Phase 3 plunge guard (2026-09-07)
//!
//! The skip above is keyed on the move's INTENT tag. A generator that
//! emits a vertical descent as a plain cutting move escapes it, and every
//! strategy then lifts that descent to the lateral chipload band. So
//! [`adaptive_feed_modulate`] adds one GEOMETRIC cap after the strategy
//! decides: a move the shared classifier
//! ([`crate::machine::kinematic_utilization::classify_move`]) calls
//! [`crate::machine::kinematic_utilization::MotionClass::Plunge`] is capped at
//! [`ModulationContext::plunge_rate_mm_min`] and reports
//! [`BindingConstraint::PlungeRate`]. The intent skip is UNCHANGED — a
//! tagged plunge still keeps its operator-tuned feed exactly.
//!
//! See `planning/acceptance_loop/findings/F-039-constrained-max-feed-modulation.md`
//! for the design narrative + acceptance bars.

use std::collections::BTreeMap;

use crate::feeds::force::DeflectionCapRefusal;
use crate::machine::kinematics::{MachineKinematics, predicted_feeds_for_toolpath};
use crate::tool_load::power::PowerModelInputs;
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
    /// and emits the binding value scaled by `feed_scale`.
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
/// `min` is the matched vendor row's own lower edge — below it the row
/// says the tooth scrapes instead of slicing and in wood the workpiece
/// scorches. `max` is the breakage / over-load ceiling. The modulator
/// floors emitted feeds at `min × rpm × flutes` (after the feed scale)
/// and caps the constrained-max search at `max × rpm × flutes`.
///
/// **Not the same thing as the rubbing floor** (Checkpoint K (d2),
/// 2026-08-13): [`crate::feeds::RUBBING_FLOOR_MM_TOOTH`] is a global,
/// diameter- and material-independent chip-formation threshold, and
/// [`crate::feeds::effective_rubbing_floor`] subordinates it to
/// `band.max`. On a sub-Ø2 tool the two land at **opposite ends of this
/// band**. This field is `band.min` and nothing else.
///
/// ## A printed point (A2, point mode)
///
/// A vendor row that prints one value gives a point, not a band
/// ([`crate::feeds::vendor_lookup::PrintedChipload::Point`]).
/// [`Self::point`] holds it as `min == max == v` and marks it, so
/// [`Self::is_point`] is `true`. The modulator reads a point so:
///
/// - `v × rpm × flutes` is the chipload cap (both strategies).
/// - No floor applies, because nothing printed a minimum. The feed scale
///   can take the feed below the point.
/// - The deflection and power "pin" arms pin to `v`.
/// - A move held at the point keeps the `ChiploadMax` tag (decision Q7).
///
/// [`Self::new`] with `min == max` is NOT a point: it stays a zero-width
/// band with a floor, as before A2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChiploadBand {
    /// Minimum chipload (mm per tooth). Must be > 0 and ≤ `max`. For a
    /// point it equals `max`, and the modulator applies no floor.
    pub min_mm_per_tooth: f64,
    /// Maximum chipload (mm per tooth). Must be ≥ `min`.
    pub max_mm_per_tooth: f64,
    /// `true` when this holds one printed value (A2). Only
    /// [`Self::point`] sets it.
    point: bool,
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
            point: false,
        })
    }

    /// Construct a printed point `v` (A2, point mode). It stores
    /// `min = max = v` and [`Self::is_point`] is `true`. Returns `None`
    /// if `v` is non-finite or ≤ 0.
    pub fn point(value_mm_per_tooth: f64) -> Option<Self> {
        if !value_mm_per_tooth.is_finite() || value_mm_per_tooth <= 0.0 {
            return None;
        }
        Some(Self {
            min_mm_per_tooth: value_mm_per_tooth,
            max_mm_per_tooth: value_mm_per_tooth,
            point: true,
        })
    }

    /// `true` when this holds one printed value, not a band. The
    /// modulator then applies no chipload floor.
    #[inline]
    pub fn is_point(&self) -> bool {
        self.point
    }

    /// Geometric midpoint of the band — the [`ModulationStrategy::BandMid`]
    /// target. Geometric (not arithmetic) so the target sits
    /// proportionally between min and max regardless of band width.
    #[inline]
    pub(crate) fn mid_mm_per_tooth(&self) -> f64 {
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
    ///
    /// **This is a fraction of the tool's FLUTE LENGTH**, not of the
    /// operation's depth per pass — see `dexel_stock::simulation`, which
    /// writes it as `axial_engagement_mm / flute_length`. It is kept for
    /// the zero-engagement short-circuits, which only test it against
    /// zero. **Do not multiply it by a depth**; T-11 did exactly that and
    /// read every cut about ten times too shallow. Use
    /// [`Self::axial_doc_mm`] for the depth.
    pub axial_doc_fraction: f64,
    /// T-11 (2026-09-16) — mean axial engagement in MILLIMETRES across
    /// the move's cutting samples, straight off `CutSample::axial_doc_mm`.
    ///
    /// The dexel measures this directly, so nothing needs to re-derive it.
    /// `0.0` means the move had no measured cutting engagement, which the
    /// solver treats the same way it treats an air move.
    pub axial_doc_mm: f64,
}

/// Optional deflection-cap inputs for the constrained-max solver.
/// When `None` the deflection constraint is skipped (typical for
/// unit tests and band-mid runs).
///
/// These carry the **same feed-aware affine force model + beam compliance
/// the post-sim deflection gate uses**, so the optimizer and gate agree on
/// a cut. The lateral force is affine in feed per tooth
/// `F = ap·(Ks·fz·sin θ_peak + F_edge)` and tip deflection is linear in
/// force `δ = compliance · F`, so the solver inverts `δ ≤ bound` for the
/// feed cap in closed form. `Ks`/`F_edge` come from
/// [`crate::feeds::force::affine_coefficients`]; `compliance` is evaluated
/// once at the toolpath's peak axial DOC from the integrated two-section
/// cantilever ([`crate::tool::ToolDefinition::tip_deflection_mm`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeflectionLimitInputs {
    /// Affine force slope `Ks` (N/mm²) — feeds::force literature-absolute.
    pub ks_n_per_mm2: f64,
    /// Affine edge intercept `F_edge` (N per mm of axial engagement) — the
    /// feed-independent force floor.
    pub f_edge_n_per_mm: f64,
    /// Tip deflection per newton of lateral force (mm/N), from the same
    /// integrated cantilever the gate uses, at the toolpath's peak axial
    /// DOC. Deflection is linear in force, so one scalar suffices.
    pub compliance_mm_per_n: f64,
    /// Maximum allowed tip displacement (mm). F-039 uses 0.2 mm
    /// matching [`crate::tool_load::deflection::EXCEEDS_BOUND_MM`].
    pub max_tip_deflection_mm: f64,
}

/// Optional power-cap inputs for the constrained-max solver. `None`
/// disables the power constraint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerLimitInputs {
    /// Raw material `Kc` in `N/mm²` — the value
    /// [`crate::material::Material::kc_n_per_mm2`] returns directly.
    /// The constrained-max solver applies
    /// [`crate::tool_load::power::GRAIN_ANISOTROPY_FACTOR`] internally
    /// so callers don't pre-multiply — see S2-9 in
    /// `planning/tool_kinematics_chipload_audit_2026-05-31.md` for
    /// the rationale (pre-S2-9 callers passed `2.0 × kc` and any
    /// future change to the anisotropy factor required N diffs).
    pub kc_n_per_mm2: f64,
    /// Effective diameter at the engagement depth (mm).
    pub engagement_diameter_mm: f64,
    /// Available spindle power × safety factor at the running RPM
    /// (kW). The constrained-max solver caps the feed so the predicted
    /// two-term power (`tool_load::power::PowerTerms`) stays inside
    /// this. Because the edge term carries no feed, a budget below that
    /// term's own floor has no feed answer at all; the solver pins to
    /// the chipload-min floor and still tags the move `PowerMax`.
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
    /// LUT chipload band (mm/tooth). **`None` = bandless**
    /// (wanaka200 IMPLEMENTATION_PLAN work item A, 2026-09-19): a
    /// toolpath whose `(tool, material, op)` tuple resolves no vendor
    /// chipload row (e.g. a V-bit ProjectCurve — the LUT publishes no
    /// V-bit contour rows). Bandless modulation does NO chipload
    /// targeting: the only caps applied are the machine cutting-feed
    /// ceiling and the Phase 3 geometric plunge guard, so a bandless
    /// descent can never escape its operation's plunge rate.
    ///
    /// A printed point ([`ChiploadBand::point`], A2) caps at the point
    /// with no floor.
    pub chipload_band: Option<ChiploadBand>,
    /// Machine kinematics — drives the `predicted_feeds_for_toolpath`
    /// per-move achievable-velocity cap.
    pub kinematics: &'a MachineKinematics,
    /// F-039 — which algorithm to run.
    pub strategy: ModulationStrategy,
    /// F-039 — the feed scale (default 1.0). The constrained-max solver
    /// multiplies the smallest candidate feed limit by this value, then
    /// applies the chipload-min floor. `1.0` emits at the binding limit;
    /// `0.7` emits at 70 % of it. `BandMid` ignores it.
    ///
    /// Not the machine dial: `MachineProfile::aggressiveness` is a load
    /// target for Suggest. This value is a plain multiplier on the
    /// modulator's output feed (ruling R4 Q6, 2026-09-24).
    pub feed_scale: f64,
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
    /// Phase 3 (2026-09-07) — the OPERATION's own plunge rate
    /// (mm/min). [`adaptive_feed_modulate`] caps a vertical-dominant
    /// move at this rate whatever its intent tag says.
    ///
    /// A value that is not finite and positive DISABLES the guard: an
    /// operation legitimately carries no plunge rate, and `min(feed,
    /// 0.0)` would stop the machine. Pass `f64::INFINITY` to disable
    /// the guard deliberately (the Phase 3 A/B's control arm).
    pub plunge_rate_mm_min: f64,
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
        feed_scale: f64,
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
            feed_scale,
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
/// the move's feed landed where it did. `ctx.feed_scale` multiplies the
/// minimum *before* the chipload-min floor; values above 1.0 push past
/// the constraint and the chipload-min floor still applies.
///
/// A2 (point mode): for a point band ([`ChiploadBand::is_point`]) the
/// chipload cap is `v × rpm × flutes`, the pin arms pin to `v`, and the
/// step-6 floor is skipped. The feed scale still applies.
fn max_safe_feed_for_move(
    engagement: PerMoveEngagement,
    band: ChiploadBand,
    ctx: &ModulationContext<'_>,
    predicted_cap_mm_min: f64,
) -> (f64, BindingConstraint) {
    let flutes = ctx.flute_count.max(1) as f64;

    // Effective WOC fraction: clamped to a small floor because the deflection
    // and power limits below divide by it. The simulator's air-cut samples are
    // gated out upstream; this protects against single-sample noise.
    let woc_eff = engagement.radial_woc_fraction.clamp(1e-3, 1.0);

    // Chip-thinning correction DELETED 2026-08-19 (G-CHIPTHIN-HALFFIX,
    // operator-ruled). This site used a different formula from Suggest's —
    // `band.max ÷ sqrt(woc_eff)` rather than `radial_chip_thinning_factor` —
    // but it made the same claim, and it fails for the same reason: the vendor
    // chipload column it re-keys against publishes no radial reference
    // condition to correct from (`CHIPLOAD_LITERATURE_VERDICT.md` N-8, a
    // deliberate negative result over six wood charts read in full).
    //
    // The magnitude here was larger than Suggest's, not smaller: `1/sqrt(woc)`
    // is unbounded below and had no equivalent of Suggest's `[1, 4]` clamp, so
    // at the 1e-3 floor it reached 31.6×. It was applied to the band MAXIMUM,
    // so a lightly-engaged move could be handed a target tens of times the
    // largest chipload the vendor publishes — and the post-simulation gate
    // that judges the result deleted its own half of this correction on
    // 2026-08-06. The two halves now agree.
    //
    // The target is simply the band maximum: this is a constrained-MAX solver,
    // so the chipload ceiling is the constraint, and the five other caps below
    // are what actually bind on an engaged move.
    let target_chipload = band.max_mm_per_tooth;

    let mut limits: Vec<(f64, BindingConstraint)> = Vec::with_capacity(6);
    // The first pin arm that fired (deflection or power), if any. For a
    // point the pin value equals the chipload cap, so the tie-break below
    // needs it.
    let mut pinned: Option<(f64, BindingConstraint)> = None;

    // 1. Chipload-max constraint (with chip-thinning correction).
    limits.push((
        target_chipload * ctx.spindle_rpm * flutes,
        BindingConstraint::ChiploadMax,
    ));

    // 2. Deflection cap (feed-aware, gate-consistent). The lateral force
    // is AFFINE in feed per tooth:
    //   F = ap · (Ks · fz·sin θ_peak + F_edge),   cos ψ = 1 − 2·woc_frac
    // and tip deflection is linear in force: δ = compliance · F. So solve
    // δ ≤ bound for the feed cap in closed form — the same affine force
    // model + beam compliance the post-sim deflection gate uses, so the
    // optimizer and gate cannot disagree on the cut (unlike the old
    // feed-blind `Kc·ap·ae` reference force this replaced).
    if let Some(defl) = ctx.deflection_inputs {
        let axial_mm = effective_axial_mm(engagement, ctx);
        // One solver, shared with the pre-simulation feeds chart: the
        // closed-form inverse lives in `feeds::force`, not here. Two
        // copies of one model drift apart.
        match crate::feeds::force::chipload_cap_for_deflection_with_reason(
            defl.ks_n_per_mm2,
            defl.f_edge_n_per_mm,
            defl.compliance_mm_per_n,
            defl.max_tip_deflection_mm,
            axial_mm,
            woc_eff,
        ) {
            Ok(fz_cap) => {
                let defl_cap = fz_cap * ctx.spindle_rpm * flutes;
                limits.push((defl_cap, BindingConstraint::DeflectionMax));
            }
            Err(
                DeflectionCapRefusal::NoLateralEngagement
                | DeflectionCapRefusal::EdgeForceOverBudget,
            ) => {
                // Even zero feed exceeds the bound (the edge floor alone is
                // over budget), or no lateral engagement: feed cannot rescue
                // deflection — pin to the chipload-min floor and let the
                // binding tag name deflection. Dropping DOC/stepover is the
                // real fix (out of scope for per-move feed). For a point
                // (A2) `min == v`, so this pins to the point.
                let floor = band.min_mm_per_tooth * ctx.spindle_rpm * flutes;
                let pin = (floor.max(1e-9), BindingConstraint::DeflectionMax);
                pinned = pinned.or(Some(pin));
                limits.push(pin);
            }
            // No DOC, no compliance, no Ks: the deflection constraint has
            // no signal here. It contributes no cap, and it must not claim
            // the binding tag either.
            Err(DeflectionCapRefusal::Unmodelled) => {}
        }
    }

    // 3. Power cap. R1 (2026-09-16): power is the two-term affine model
    // `P = A·(Ks·MRR + F_edge·ap·Vc·z·ψ/2π)`, built from the SAME
    // `(Ks, F_edge)` pair the deflection cap above consumes. One solver
    // again: the model lives in `tool_load::power`, not here. This site
    // previously carried its own copy of the linear formula, so a change
    // to the gate left the optimizer predicting different power for the
    // same cut.
    //
    // Only the shear term moves with feed, so the cap is an affine
    // inversion, not a ratio — `PowerTerms::feed_for_kw` does it.
    if let Some(pow) = ctx.power_inputs
        && pow.available_kw > 0.0
    {
        let axial_mm = effective_axial_mm(engagement, ctx);
        if axial_mm > 0.0 && woc_eff > 0.0 {
            let radial_width = woc_eff * pow.engagement_diameter_mm.max(0.0);
            if radial_width > 0.0 {
                // Callers pass raw Kc; `PowerTerms::of` applies the
                // canonical anisotropy factor (S2-9) so any future
                // change to GRAIN_ANISOTROPY_FACTOR is one diff, not N.
                //
                // ψ from the same `cos ψ = 1 − 2·woc_fraction` the
                // deflection cap uses — the modulator has one
                // engagement definition, not two.
                let terms = crate::tool_load::power::PowerTerms::of(PowerModelInputs {
                    kc_n_per_mm2: pow.kc_n_per_mm2,
                    cross_section_mm2: axial_mm * radial_width,
                    axial_doc_mm: axial_mm,
                    immersion_rad: crate::feeds::force::immersion_angle(
                        radial_width,
                        pow.engagement_diameter_mm / 2.0,
                    ),
                    engagement_diameter_mm: pow.engagement_diameter_mm,
                    spindle_rpm: ctx.spindle_rpm,
                    flute_count: flutes,
                });
                match terms.feed_for_kw(pow.available_kw) {
                    Some(pow_cap) => limits.push((pow_cap, BindingConstraint::PowerMax)),
                    None => {
                        // The feed-free edge term alone already meets the
                        // power budget, so no feed rescues it — the same
                        // shape as `EdgeForceOverBudget` on the
                        // deflection side, and handled the same way: pin
                        // to the chipload-min floor and let the binding
                        // tag name power. The real fix is less DOC,
                        // less stepover or less RPM, none of which a
                        // per-move feed solver can reach for. For a point
                        // (A2) `min == v`, so this pins to the point.
                        let floor = band.min_mm_per_tooth * ctx.spindle_rpm * flutes;
                        let pin = (floor.max(1e-9), BindingConstraint::PowerMax);
                        pinned = pinned.or(Some(pin));
                        limits.push(pin);
                    }
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

    // A2 (point mode): a pin arm pins to `v`, which is also the chipload
    // cap, so the two tie. `min_by` keeps the first entry of a tie, which
    // is `ChiploadMax`. The pin names the load-bearing constraint (no feed
    // satisfies it), so for a point the pin tag wins a tie. A band pins at
    // `min < max`, so this changes nothing for a band.
    let binding = match pinned {
        Some((pin_feed, pin_tag)) if band.is_point() && pin_feed <= limit => pin_tag,
        _ => binding,
    };

    let scale = ctx.feed_scale.max(0.0);
    let after_scale = limit * scale;

    // A2 (point mode): no floor for a point. Nothing printed a minimum,
    // so the feed scale can take the feed below the point. The binding
    // tag stays the tag of the smallest cap (Q7: a move held at the
    // point keeps `ChiploadMax`).
    if band.is_point() {
        return (after_scale, binding);
    }

    // 6. Chipload-min floor — the matched row's own `band.min`, NOT
    // `feeds::effective_rubbing_floor` (see `ChiploadBand`). Applied last so
    // the feed scale can't drop feeds below the safe floor. When
    // the floor itself sits above the machine's hard cap (rare:
    // machine `max_feed` configured below `band.min × rpm × flutes`)
    // the cap wins — emitting above the cap would crash the
    // controller, which is strictly worse than burning the
    // workpiece. The machine constraint stays load-bearing.
    //
    // Binding-tag rule (per F-039 spec): re-label as `ChiploadMin`
    // only when the feed scale took the limit BELOW the floor
    // (`emitted == floor && limit > floor`). When the *underlying*
    // limit itself sits below the floor (deflection or power forced
    // a low feed even at feed scale 1.0), keep the original
    // binding so the diagnostic surface names the load-bearing
    // physical constraint, not the floor we backed off to.
    let floor = band.min_mm_per_tooth * ctx.spindle_rpm * flutes;
    let effective_floor = floor.min(ctx.max_feed_mm_min);
    if after_scale < effective_floor {
        let new_binding = if limit > effective_floor {
            BindingConstraint::ChiploadMin
        } else {
            binding
        };
        (effective_floor, new_binding)
    } else {
        (after_scale, binding)
    }
}

/// The axial depth this move actually cuts, in mm.
///
/// ## T-11 (2026-09-16) — what this used to compute, and why it was wrong
///
/// It used to return `ctx.nominal_axial_doc_mm * engagement.axial_doc_fraction`.
/// That multiplied a depth in mm by a fraction of a DIFFERENT quantity: the
/// dexel writes `axial_doc_fraction` as `axial_engagement_mm / flute_length`
/// (`dexel_stock::simulation`), not as a fraction of the nominal depth. The
/// product was `mm x (mm / flute_length)`, which is not a depth.
///
/// The error factor was `flute_length / nominal_axial_doc_mm`, and it always
/// read too SHALLOW, because a pass is always shallower than the flute. A
/// 2 mm pass on a tool with a 25 mm flute came out at 0.16 mm, 12.5x low.
/// This value scales both the deflection cap and the power cap in the
/// constrained-max solver, so both were far too permissive and the modulator
/// handed out feeds the machine should not have been given. Those feeds reach
/// the post-sim power verdict through `trace.predicted_feeds`
/// (`session::compute` stamps them), so the defect also broke the very
/// gate-versus-modulation agreement that stamping exists to create.
///
/// Every fixture pinned `axial_doc_fraction` at `1.0` — the single value at
/// which the wrong expression returns the right answer — so no test could
/// fail on it. See `planning/TECH_DEBT_REGISTER.md` T-11.
///
/// It now reads the millimetre measurement the dexel already took, which is
/// what `Engagement::axial_doc_fraction`'s own doc comment directs a caller
/// to do.
fn effective_axial_mm(engagement: PerMoveEngagement, ctx: &ModulationContext<'_>) -> f64 {
    let measured = engagement.axial_doc_mm;
    if measured > 0.0 {
        return measured;
    }
    // No per-move measurement. Fall back to the toolpath's nominal depth,
    // which `session::compute` derives as the maximum measured axial
    // engagement over the toolpath's cutting samples — a real reading, not a
    // fabricated one.
    let nominal = ctx.nominal_axial_doc_mm.max(0.0);
    if nominal > 0.0 {
        return nominal;
    }
    // Neither a per-move nor a per-toolpath axial signal exists, which means
    // the trace carried no cutting samples for this path. Returning the
    // engagement diameter here is a proxy, not a measurement: it keeps the
    // pre-T-11 behaviour for legacy and analytical traces rather than
    // silently disabling both caps, and the caller treats a zero as an air
    // move. The deflection cap carries no diameter (it works off compliance
    // plus the affine coefficients), so only `power_inputs` contributes.
    ctx.power_inputs
        .map(|p| p.engagement_diameter_mm)
        .unwrap_or(0.0)
}

/// F-036 — "target band-mid" per-move feed (legacy heuristic).
///
/// A2 (point mode): for a point band the target and the ceiling are
/// `v × rpm × flutes`, and no floor applies. The kinematic cap is then
/// not raised to a floor.
fn band_mid_feed_for_move(
    commanded_feed_mm_min: f64,
    engagement: PerMoveEngagement,
    predicted_cap_mm_min: f64,
    ctx: &ModulationContext<'_>,
    band: ChiploadBand,
) -> (f64, BindingConstraint) {
    if engagement.radial_woc_fraction <= 1e-6 && engagement.axial_doc_fraction <= 1e-6 {
        return (commanded_feed_mm_min, BindingConstraint::ChiploadMax);
    }
    let woc = engagement.radial_woc_fraction.clamp(1e-3, 1.0);
    let thinning = woc.sqrt().max(1e-6);
    let target_chipload = band.mid_mm_per_tooth() / thinning;
    let flutes = ctx.flute_count.max(1) as f64;
    let base_feed = target_chipload * ctx.spindle_rpm * flutes;

    if band.is_point() {
        // A2: the target and the ceiling are the point. No floor, and no
        // chip-thinning lift (the ceiling is the point, so a lift above it
        // is always clamped back).
        let ceiling = band.max_mm_per_tooth * ctx.spindle_rpm * flutes;
        let feed = ceiling
            .min(ctx.max_feed_mm_min)
            .min(predicted_cap_mm_min)
            .max(0.0);
        let binding = if (feed - ctx.max_feed_mm_min).abs() < 1e-6 {
            BindingConstraint::MachineMaxFeed
        } else if (feed - ceiling).abs() < 1e-6 {
            BindingConstraint::ChiploadMax
        } else if (feed - predicted_cap_mm_min).abs() < 1e-6 {
            BindingConstraint::KinematicReach
        } else {
            BindingConstraint::ChiploadMax
        };
        return (feed, binding);
    }

    let band_floor = band.min_mm_per_tooth * ctx.spindle_rpm * flutes;
    let band_ceiling = band.max_mm_per_tooth * ctx.spindle_rpm * flutes;

    let cap = ctx
        .max_feed_mm_min
        .min(band_ceiling)
        .min(predicted_cap_mm_min.max(band_floor));
    if cap < band_floor {
        return (cap.max(0.0), BindingConstraint::MachineMaxFeed);
    }
    let clamped = base_feed.clamp(band_floor, cap);
    // Machine-cap check comes first: when the machine cap ties the band
    // ceiling (or another candidate) at the same clamped value, the feed
    // is genuinely machine-limited and the diagnostic should say so
    // rather than falling through to the first tied arm (ChiploadMax).
    let binding = if (clamped - ctx.max_feed_mm_min).abs() < 1e-6 {
        BindingConstraint::MachineMaxFeed
    } else if (clamped - band_ceiling).abs() < 1e-6 {
        BindingConstraint::ChiploadMax
    } else if (clamped - band_floor).abs() < 1e-6 {
        BindingConstraint::ChiploadMin
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
/// [`crate::stock::simulation_cut::SimulationCutTrace::modulated_feeds`]
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
    // Bandless: probe at the machine ceiling — there is no band
    // ceiling to fold in.
    let band_ceiling_feed = ctx.chipload_band.map_or(ctx.max_feed_mm_min, |band| {
        band.max_mm_per_tooth * ctx.spindle_rpm * ctx.flute_count.max(1) as f64
    });
    let probe_feed = ctx.max_feed_mm_min.min(band_ceiling_feed).max(1e-3);
    let mut probe = toolpath.clone();
    for m in probe.moves.iter_mut() {
        m.move_type = m.move_type.with_feed_rate(probe_feed);
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
    // verified equals `toolpath.moves.len()`. The Phase 3 guard also
    // reads `moves[i - 1]`, under its own `i > 0` test.
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

        let (mut new_feed, mut binding) = match (ctx.strategy, ctx.chipload_band) {
            // BANDLESS (wanaka200 IMPLEMENTATION_PLAN work item A,
            // 2026-09-19): no vendor chipload row, so no chipload
            // targeting — clamp the commanded feed to the machine
            // cutting ceiling only, then fall through to the geometric
            // plunge guard below. This is the arm a V-bit ProjectCurve
            // reaches: its 20° V point previously descended at the
            // full lateral commanded feed (1165 mm/min against a 400
            // mm/min plunge rate) because the guard lived downstream of
            // a band gate it could never pass. Same binding tag as the
            // banded no-engagement arm, for consistency.
            (_, None) => (
                commanded.min(ctx.max_feed_mm_min),
                BindingConstraint::MachineMaxFeed,
            ),
            (ModulationStrategy::BandMid, Some(band)) => {
                band_mid_feed_for_move(commanded, engagement, predicted_cap, ctx, band)
            }
            (ModulationStrategy::ConstrainedMax, Some(band)) => {
                if engagement.radial_woc_fraction <= 1e-6 && engagement.axial_doc_fraction <= 1e-6 {
                    // No engagement — leave the commanded feed
                    // alone. Record the per-move entry so the
                    // diagnostic surface can still see it.
                    (commanded, BindingConstraint::MachineMaxFeed)
                } else {
                    max_safe_feed_for_move(engagement, band, ctx, predicted_cap)
                }
            }
        };

        // Phase 3 (2026-09-07) — the GEOMETRIC plunge guard.
        //
        // `should_skip_modulation` above protects a plunge that carries a
        // plunge INTENT. The adaptive3d rough emits its step-down and
        // re-entry descents as plain cutting moves, so they fall through
        // that filter and every branch above lifts them to the lateral
        // chipload band. On the wanaka front rough that produced 1807
        // mm/min of pure-vertical descent against a 512 mm/min plunge
        // rate. Intent-keyed protection; untagged geometry escaped it.
        //
        // The guard reads the move's own vector through
        // `kinematic_utilization::classify_move` — the ONE construction
        // site, shared with the Phase 2 instrument, so the guard and the
        // instrument can never disagree about what a plunge is.
        //
        // It is a FLOOR UNDER THE LIFT, not a skip: a move already at or
        // below the plunge rate keeps the strategy's binding, and a tagged
        // `EntryPlunge` never reaches here at all.
        if i > 0 && ctx.plunge_rate_mm_min.is_finite() && ctx.plunge_rate_mm_min > 1e-9 {
            let prev = toolpath.moves[i - 1].target;
            let curr = toolpath.moves[i].target;
            let delta = [curr.x - prev.x, curr.y - prev.y, curr.z - prev.z];
            if crate::machine::kinematic_utilization::classify_move(move_type, delta)
                == crate::machine::kinematic_utilization::MotionClass::Plunge
                && new_feed > ctx.plunge_rate_mm_min
            {
                new_feed = ctx.plunge_rate_mm_min;
                binding = BindingConstraint::PlungeRate;
            }
        }

        outcome.per_move.insert(i, (new_feed, binding));
        if (new_feed - commanded).abs() > 1e-6 {
            toolpath.moves[i].move_type = move_type.with_feed_rate(new_feed);
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
            chipload_band: Some(band),
            kinematics: k,
            strategy: ModulationStrategy::BandMid,
            feed_scale: 1.0,
            deflection_inputs: None,
            power_inputs: None,
            nominal_axial_doc_mm: 0.0,
            // A realistic operation plunge rate. Every fixture in this
            // module cuts laterally or ramps at 45 degrees, so the Phase 3
            // guard classifies nothing here as a plunge and no expectation
            // below moves. `tests/plunge_guard_p3.rs` exercises the guard.
            plunge_rate_mm_min: 300.0,
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
            tp.feed_to_with_intent(P3::new(x, 0.0, -2.0), feed_mm_min, MoveIntent::ClearingCut);
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
            tp.feed_to_with_intent(P3::new(x, y, -2.0), feed_mm_min, MoveIntent::ClearingCut);
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

    /// Step-3 convergence sentry: the feed-aware deflection cap is the
    /// EXACT inverse of the affine force model — plugging the capped feed
    /// back through `δ = compliance · ap · (Ks·fz·sinθ + F_edge)` lands on
    /// the deflection bound. This is what makes the optimizer and the
    /// post-sim gate agree on a cut (they share the model now); the old
    /// `δ ∝ feed` scaling against a feed-blind reference force could not.
    #[test]
    fn deflection_cap_inverts_affine_model_onto_the_bound() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        ctx.nominal_axial_doc_mm = 2.0;
        // Long-reach tool where the edge floor fits under budget but the
        // chipload-max feed would overshoot, so the affine SOLVE fires
        // (not the can't-satisfy floor branch).
        let bound = 0.2_f64;
        let ks = 42.7_f64;
        let f_edge = 4.53_f64;
        let compliance = 0.015_f64; // mm/N
        ctx.deflection_inputs = Some(DeflectionLimitInputs {
            ks_n_per_mm2: ks,
            f_edge_n_per_mm: f_edge,
            compliance_mm_per_n: compliance,
            max_tip_deflection_mm: bound,
        });
        // Full slot, full axial: woc = 1 ⇒ θ_peak = π/2 (sin = 1), ap = 2.
        let engagement = PerMoveEngagement {
            radial_woc_fraction: 1.0,
            axial_doc_fraction: 1.0,
            // T-11: the depth is now read in mm, not derived from the
            // fraction. 2.0 is what `nominal x fraction` meant here.
            axial_doc_mm: 2.0,
        };
        // Large kinematic cap so deflection is the binding constraint.
        let (feed_cap, binding) =
            max_safe_feed_for_move(engagement, ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert_eq!(
            binding,
            BindingConstraint::DeflectionMax,
            "deflection should bind on this long-reach tool; got {binding:?} at feed {feed_cap}"
        );
        // Plug the capped feed back through the affine model + compliance.
        let flutes = ctx.flute_count as f64;
        let fz = feed_cap / (ctx.spindle_rpm * flutes);
        let ap = 2.0; // nominal × axial_doc_fraction
        let force = ap * (ks * fz * 1.0 + f_edge); // sinθ_peak = 1 at full slot
        let delta_mm = compliance * force;
        assert!(
            (delta_mm - bound).abs() < 1.0e-3,
            "capped feed must invert onto the deflection bound: δ={delta_mm:.4} mm vs bound {bound} \
             (feed {feed_cap:.1}, fz {fz:.4})"
        );
    }

    /// All-air engagement: feed must stay byte-identical (no modulation
    /// triggered). Invariant for both strategies.
    #[test]
    fn zero_engagement_leaves_feed_unchanged() {
        for strategy in [
            ModulationStrategy::BandMid,
            ModulationStrategy::ConstrainedMax,
        ] {
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
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
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
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
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
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
        ];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::BandMid;
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        let f = tp.moves[1].move_type.feed_rate().unwrap();
        // 0.08 × 18000 × 2 = 2880.
        assert!(
            (f - 2880.0).abs() < 5.0,
            "expected band ceiling 2880, got {f}"
        );
    }

    /// S.4 regression: when the machine cap ties the band ceiling at the
    /// same clamped feed, `band_mid_feed_for_move` must report
    /// `MachineMaxFeed`, not `ChiploadMax` — the machine cap is the
    /// hard physical constraint and should win diagnostic priority over
    /// a coincidentally-equal chipload ceiling. This pins the arm-order
    /// fix (`MachineMaxFeed` checked before `ChiploadMax`).
    #[test]
    fn band_mid_tie_between_machine_cap_and_ceiling_reports_machine_cap() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::BandMid;
        // band_ceiling = 0.08 * 18000 * 2 = 2880 — tie the machine cap
        // to it exactly.
        ctx.max_feed_mm_min = 2880.0;
        let engagement = PerMoveEngagement {
            radial_woc_fraction: 0.01,
            axial_doc_fraction: 1.0,
            // T-11: the depth is now read in mm, not derived from the
            // fraction. 2.0 is what `nominal x fraction` meant here.
            axial_doc_mm: 2.0,
        };
        // Light engagement inflates the chip-thinning target well past
        // the cap, so the clamp — not the natural target — decides the
        // feed and lands exactly on the tie.
        let predicted_cap_mm_min = 10_000.0; // not the tightest constraint
        let (feed, binding) = band_mid_feed_for_move(
            1500.0,
            engagement,
            predicted_cap_mm_min,
            &ctx,
            ctx.chipload_band.unwrap(),
        );
        assert!(
            (feed - 2880.0).abs() < 1e-6,
            "expected tied cap 2880, got {feed}"
        );
        assert_eq!(
            binding,
            BindingConstraint::MachineMaxFeed,
            "machine cap should win the tie, got {binding:?}"
        );
    }

    /// machine-max-feed cap wins.
    #[test]
    fn machine_max_feed_cap_wins() {
        let mut tp = straight_toolpath(2, 1500.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
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
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
            },
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
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
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
                let band_floor = ctx.chipload_band.unwrap().min_mm_per_tooth
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
        for strategy in [
            ModulationStrategy::BandMid,
            ModulationStrategy::ConstrainedMax,
        ] {
            let mut tp = corner_heavy_toolpath(2000.0);
            let engagements: Vec<_> = (0..tp.moves.len())
                .map(|i| {
                    if matches!(tp.moves[i].move_type, MoveType::Rapid) {
                        PerMoveEngagement::default()
                    } else if i % 2 == 0 {
                        PerMoveEngagement {
                            radial_woc_fraction: 1.0,
                            axial_doc_fraction: 1.0,
                            // T-11: the depth is now read in mm, not derived from the
                            // fraction. 2.0 is what `nominal x fraction` meant here.
                            axial_doc_mm: 2.0,
                        }
                    } else {
                        PerMoveEngagement {
                            radial_woc_fraction: 0.2,
                            axial_doc_fraction: 1.0,
                            // T-11: the depth is now read in mm, not derived from the
                            // fraction. 2.0 is what `nominal x fraction` meant here.
                            axial_doc_mm: 2.0,
                        }
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

    // ── A2 (point mode) ────────────────────────────────────────────────

    /// A full-slot engagement at 2 mm axial depth.
    fn full_slot() -> PerMoveEngagement {
        PerMoveEngagement {
            radial_woc_fraction: 1.0,
            axial_doc_fraction: 1.0,
            axial_doc_mm: 2.0,
        }
    }

    /// A2: `point(v)` stores `min = max = v` and is a point. `new(v, v)`
    /// is not a point. A non-finite or non-positive value is refused.
    #[test]
    fn point_band_holds_one_value() {
        let p = ChiploadBand::point(0.05).unwrap();
        assert!(p.is_point());
        assert!((p.min_mm_per_tooth - 0.05).abs() < 1e-12);
        assert!((p.max_mm_per_tooth - 0.05).abs() < 1e-12);
        assert!(!ChiploadBand::new(0.05, 0.05).unwrap().is_point());
        assert!(!band().is_point());
        assert!(ChiploadBand::point(0.0).is_none());
        assert!(ChiploadBand::point(-0.01).is_none());
        assert!(ChiploadBand::point(f64::NAN).is_none());
        assert!(ChiploadBand::point(f64::INFINITY).is_none());
    }

    /// A2 ConstrainedMax: the point is the chipload cap.
    /// 0.05 × 18000 × 2 = 1800 mm/min, tag `ChiploadMax` (Q7).
    #[test]
    fn point_constrained_max_caps_at_the_point() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, ChiploadBand::point(0.05).unwrap());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        let (feed, binding) =
            max_safe_feed_for_move(full_slot(), ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert!((feed - 1800.0).abs() < 1e-6, "expected 1800, got {feed}");
        assert_eq!(binding, BindingConstraint::ChiploadMax);
    }

    /// A2 ConstrainedMax: the feed scale applies and no floor restores the
    /// feed. At scale 0.7 a point emits 0.7 × 1800 = 1260 mm/min with the
    /// `ChiploadMax` tag. The zero-width band `new(v, v)` restores its
    /// floor at 1800 and keeps the cap's tag (the pre-A2 behaviour: the
    /// cap equals the floor, so the floor does not re-tag the move).
    #[test]
    fn point_feed_scale_has_no_floor() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, ChiploadBand::point(0.05).unwrap());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        ctx.feed_scale = 0.7;
        let (feed, binding) =
            max_safe_feed_for_move(full_slot(), ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert!((feed - 1260.0).abs() < 1e-6, "expected 1260, got {feed}");
        assert_eq!(binding, BindingConstraint::ChiploadMax);

        let zero_width = ChiploadBand::new(0.05, 0.05).unwrap();
        let (feed, binding) = max_safe_feed_for_move(full_slot(), zero_width, &ctx, 1.0e9);
        assert!(
            (feed - 1800.0).abs() < 1e-6,
            "expected floor 1800, got {feed}"
        );
        assert_eq!(binding, BindingConstraint::ChiploadMax);

        // The same through the public entry point: every engaged move
        // emits about 1260, and no floor lifts it back to the point. The
        // kinematic cap on these moves can tie the chipload cap, so this
        // part does not assert the tag. The tolerance matches the sibling
        // tests on this fixture.
        let mut tp = straight_toolpath(3, 1500.0);
        let engagements: Vec<_> = (0..tp.moves.len())
            .map(|i| {
                if i == 0 {
                    PerMoveEngagement::default()
                } else {
                    full_slot()
                }
            })
            .collect();
        let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        assert_eq!(outcome.changed, 3);
        for (feed, _) in outcome.per_move.values() {
            assert!(*feed < 1800.0 - 1.0, "a floor lifted the feed: {feed}");
            assert!((feed - 1260.0).abs() < 5.0, "expected ~1260, got {feed}");
        }
    }

    /// A2 ConstrainedMax: the deflection pin arm pins to the point. The
    /// edge force alone is over budget (0.1 mm/N × 2 mm × 4.53 N/mm =
    /// 0.906 mm > 0.2 mm), so the move pins to 1800 mm/min. The pin
    /// value ties the chipload cap; the tag names deflection. At scale
    /// 0.7 the pin gets no floor: 1260 mm/min.
    #[test]
    fn point_deflection_pin_is_the_point() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, ChiploadBand::point(0.05).unwrap());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        ctx.deflection_inputs = Some(DeflectionLimitInputs {
            ks_n_per_mm2: 42.7,
            f_edge_n_per_mm: 4.53,
            compliance_mm_per_n: 0.1,
            max_tip_deflection_mm: 0.2,
        });
        let (feed, binding) =
            max_safe_feed_for_move(full_slot(), ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert!(
            (feed - 1800.0).abs() < 1e-6,
            "expected pin 1800, got {feed}"
        );
        assert_eq!(binding, BindingConstraint::DeflectionMax);

        ctx.feed_scale = 0.7;
        let (feed, binding) =
            max_safe_feed_for_move(full_slot(), ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert!((feed - 1260.0).abs() < 1e-6, "expected 1260, got {feed}");
        assert_eq!(binding, BindingConstraint::DeflectionMax);
    }

    /// A2 ConstrainedMax: the power pin arm pins to the point. A budget
    /// of 1 W is below the edge-force power, so no feed satisfies it and
    /// the move pins to 1800 mm/min with the `PowerMax` tag.
    #[test]
    fn point_power_pin_is_the_point() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, ChiploadBand::point(0.05).unwrap());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        ctx.power_inputs = Some(PowerLimitInputs {
            kc_n_per_mm2: 20.0,
            engagement_diameter_mm: 6.0,
            available_kw: 0.001,
        });
        let (feed, binding) =
            max_safe_feed_for_move(full_slot(), ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert!(
            (feed - 1800.0).abs() < 1e-6,
            "expected pin 1800, got {feed}"
        );
        assert_eq!(binding, BindingConstraint::PowerMax);
    }

    /// A2 BandMid: the target and the ceiling are the point, and no floor
    /// lifts a lower kinematic cap. Full slot: 1800 mm/min, `ChiploadMax`.
    /// A kinematic cap of 500 mm/min binds at 500 (`KinematicReach`); the
    /// zero-width band `new(v, v)` lifts that cap to its floor, 1800.
    #[test]
    fn point_band_mid_targets_the_point_with_no_floor() {
        let k = shapeoko();
        let point = ChiploadBand::point(0.05).unwrap();
        let ctx = make_ctx(&k, point);
        let (feed, binding) = band_mid_feed_for_move(1500.0, full_slot(), 1.0e9, &ctx, point);
        assert!((feed - 1800.0).abs() < 1e-6, "expected 1800, got {feed}");
        assert_eq!(binding, BindingConstraint::ChiploadMax);

        // Light engagement: the chip-thinning lift does not pass the point.
        let light = PerMoveEngagement {
            radial_woc_fraction: 0.1,
            ..full_slot()
        };
        let (feed, _) = band_mid_feed_for_move(1500.0, light, 1.0e9, &ctx, point);
        assert!((feed - 1800.0).abs() < 1e-6, "expected 1800, got {feed}");

        let (feed, binding) = band_mid_feed_for_move(1500.0, full_slot(), 500.0, &ctx, point);
        assert!((feed - 500.0).abs() < 1e-6, "expected 500, got {feed}");
        assert_eq!(binding, BindingConstraint::KinematicReach);

        let zero_width = ChiploadBand::new(0.05, 0.05).unwrap();
        let (feed, _) = band_mid_feed_for_move(1500.0, full_slot(), 500.0, &ctx, zero_width);
        assert!(
            (feed - 1800.0).abs() < 1e-6,
            "expected floor 1800, got {feed}"
        );
    }
}
