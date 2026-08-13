//! Per-candidate diff types and verdict comparators.
//!
//! - [`ParamDelta`] — which knobs a candidate moves vs the baseline.
//! - [`GateDelta`] / [`GateDeltas`] — per-gate relative state, used by
//!   the tier dispatcher in `build_outcome` to distinguish pure
//!   improvements from trade-offs.
//! - [`classify_candidate_vs_baseline`] + the typed `classify_one_gate_*`
//!   helpers — pure verdict comparators; no cycle-time judgment.
//! - [`delta_against_baseline`] — pure `OperationConfig` diff used by
//!   strategies and the tier dispatcher.

use serde::{Deserialize, Serialize};

use crate::compute::catalog::OperationConfig;
use crate::tool_load::verdict::{
    ChiploadVerdict, DeflectionVerdict, PowerVerdict, ToolpathLoadVerdict,
};

use super::OptimizeCandidate;
use super::search_policy;

/// Human-readable diff between a candidate and the baseline. Each field
/// carries `Some(new_value)` only if the candidate is changing it.
/// Used by the modal to render "feed 1899→2100" style summaries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParamDelta {
    /// New feed in mm/min. `None` if the candidate matches baseline feed.
    pub feed_mm_min: Option<f64>,
    /// New spindle RPM. `None` if the candidate is not changing RPM.
    pub spindle_rpm: Option<u32>,
    /// New radial stepover in mm.
    pub stepover_mm: Option<f64>,
    /// New depth-per-pass in mm.
    pub depth_per_pass_mm: Option<f64>,
    /// New scallop ridge height in mm. Distinct axis from `stepover_mm`
    /// because they live in different units — Scallop derives stepover
    /// from `(scallop_height, ball_radius)` via the chord-step formula.
    /// G2 (2026-05-08).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scallop_height_mm: Option<f64>,
}

impl ParamDelta {
    /// True if any field is `Some(_)` — i.e. the candidate is non-trivial.
    pub fn has_changes(&self) -> bool {
        self.feed_mm_min.is_some()
            || self.spindle_rpm.is_some()
            || self.stepover_mm.is_some()
            || self.depth_per_pass_mm.is_some()
            || self.scallop_height_mm.is_some()
    }
}

/// One gate's relative state for a candidate vs the baseline. Used
/// by the tier dispatcher to distinguish pure improvements (no
/// regressions) from trade-offs (improves the failing gate but
/// worsens another).
///
/// `Within → Within` is `Same` regardless of peak magnitude — the
/// directional meaning of "lower peak" is criterion-specific
/// (chipload prefers near-midpoint, power prefers lower, deflection
/// prefers lower) and the optimizer can't make a useful judgment
/// from peaks alone in the safe region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateDelta {
    /// `Exceeds → Within` (crossed back into safety) or both Exceeds
    /// with strictly smaller peak.
    Improved,
    /// Both `Within`, or both `Exceeds` with effectively equal peak.
    Same,
    /// `Within → Exceeds` (crossed out of safety) or both Exceeds
    /// with strictly larger peak.
    Worsened,
    /// At least one side is `Unmodeled` — comparison not meaningful.
    Unmodeled,
}

/// Per-criterion deltas for a candidate vs baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateDeltas {
    pub chipload: GateDelta,
    pub power: GateDelta,
    pub deflection: GateDelta,
}

impl GateDeltas {
    /// True if no gate regressed (every delta is Improved, Same, or
    /// Unmodeled). A "pure improvement" candidate satisfies this.
    pub fn no_regression(&self) -> bool {
        !matches!(self.chipload, GateDelta::Worsened)
            && !matches!(self.power, GateDelta::Worsened)
            && !matches!(self.deflection, GateDelta::Worsened)
    }

    /// True if at least one gate was Improved.
    pub fn any_improved(&self) -> bool {
        matches!(self.chipload, GateDelta::Improved)
            || matches!(self.power, GateDelta::Improved)
            || matches!(self.deflection, GateDelta::Improved)
    }

    /// True if at least one gate was Worsened.
    pub fn any_worsened(&self) -> bool {
        matches!(self.chipload, GateDelta::Worsened)
            || matches!(self.power, GateDelta::Worsened)
            || matches!(self.deflection, GateDelta::Worsened)
    }
}

/// True if every criterion is non-`Exceeds`. `Within` and `Unmodeled`
/// both pass; `Unmodeled` is the gate's honest "I don't know" and
/// shouldn't block a recommendation by itself.
///
/// Note: a `Within` reading admitted only by the layer-1 tolerance band
/// (G16 §11.4) still passes here. To distinguish strictly-safe from
/// band-admitted candidates, see [`candidate_is_strictly_safe`] and
/// [`candidate_is_marginally_safe`].
pub(crate) fn candidate_is_safe(candidate: &OptimizeCandidate) -> bool {
    // Derived from `criteria()` (Phase 6 task 5) so a future fourth
    // gate participates automatically instead of being a silent miss.
    !candidate.verdict.any_exceeded()
}

/// True if the candidate is `Within` on every gate AND every reading is
/// inside the strict (un-widened) LUT bound. These are the candidates
/// safe to auto-recommend in the `Ranked` outcome.
///
/// G16 §11.4 Layer 3: separates "Within because the gate said so" from
/// "Within because the tolerance band widened the gate".
pub(crate) fn candidate_is_strictly_safe(candidate: &OptimizeCandidate) -> bool {
    candidate_is_safe(candidate) && !candidate_is_marginally_safe(candidate)
}

/// True if the candidate is `Within` on every gate BUT at least one
/// reading is outside the strict LUT bound — i.e. it was admitted only
/// by the layer-1 tolerance band. These candidates are safe enough to
/// attempt on a scrap but should not auto-recommend without operator
/// review.
///
/// Today's defaults (`breakage_tolerance = 0.05`, `burn_tolerance = 0.05`,
/// `power_breach_tolerance = 0`, `deflection_breach_tolerance = 0`) make
/// chipload the only gate that can produce a band-admitted Within. The
/// power and deflection branches are dormant pending §11 phase 2c
/// calibration.
pub(crate) fn candidate_is_marginally_safe(candidate: &OptimizeCandidate) -> bool {
    if !candidate_is_safe(candidate) {
        return false;
    }
    chipload_within_breaches_strict(&candidate.verdict.chipload)
        || chipload_within_carries_burn_advisory(&candidate.verdict.chipload)
        || power_within_breaches_strict(&candidate.verdict.power)
        || deflection_within_breaches_strict(&candidate.verdict.deflection)
}

/// F3.3 — a `Within` chipload verdict carrying a weak-provenance burn
/// advisory (median below a point-preset / extrapolated / ae-less burn
/// floor) is safe enough to attempt on a scrap but must not
/// auto-recommend: route to MarginalSafe. Pre-F3.3 such a candidate
/// either hard-refused on the fabricated floor or recommended with
/// full authority (A4 dead-signal finding on
/// `ChipBoundsSource::VendorLutExtrapolated`).
fn chipload_within_carries_burn_advisory(v: &ChiploadVerdict) -> bool {
    matches!(
        v,
        ChiploadVerdict::Within {
            burn_advisory: Some(_),
            ..
        }
    )
}

/// # Checkpoint P (2), 2026-08-14 — these re-decisions honour the gate's
/// boundary contract
///
/// The three helpers below re-decide, from scratch, a comparison a gate has
/// already made: they take a `Within` verdict and ask whether the reading was
/// *strictly* inside its bound or only admitted by the tolerance band. Until
/// P-(2) each wrote that as a bare `>` / `<` and reached none of Checkpoint
/// K's epsilon helpers, so a candidate the epsilon-aware gate called `Within`
/// could be re-decided here as a strict breach **on float noise** — the
/// G-CHIP-ULP shape, relocated into the optimizer's tier dispatch, where the
/// consequence is auto-recommend (`Ranked`) vs "verify on a scrap"
/// (`MarginalSafe`).
///
/// Tolerance is `0.0` at every site on purpose: the *point* of these helpers
/// is to compare against the un-widened bound. The boundary epsilon is not a
/// tolerance (see [`crate::tool_load::boundary`]) and applies regardless.
fn chipload_within_breaches_strict(v: &ChiploadVerdict) -> bool {
    let ChiploadVerdict::Within {
        approach_to_min,
        approach_to_max,
        ..
    } = v
    else {
        return false;
    };
    // High-side breach: per-sample peak above the strict LUT max.
    if approach_to_max
        .bounds
        .exceeds_high(approach_to_max.observed_mm_per_tooth, 0.0)
    {
        return true;
    }
    // Low-side breach: median observed below the strict LUT min, when
    // the matched row publishes a min (some rows are upper-bound only).
    // `below_low` returns `None` for exactly that upper-bound-only case, and
    // `None` must not collapse into "breached".
    if let Some(min_metric) = approach_to_min
        && min_metric
            .bounds
            .below_low(min_metric.observed_mm_per_tooth, 0.0)
            == Some(true)
    {
        return true;
    }
    false
}

fn power_within_breaches_strict(v: &PowerVerdict) -> bool {
    let PowerVerdict::Within {
        peak_kw,
        available_kw,
        ..
    } = v
    else {
        return false;
    };
    crate::tool_load::boundary::exceeds_high(*peak_kw, *available_kw, 0.0)
}

fn deflection_within_breaches_strict(v: &DeflectionVerdict) -> bool {
    let DeflectionVerdict::Within {
        peak_mm, bounds, ..
    } = v
    else {
        return false;
    };
    crate::tool_load::boundary::exceeds_high(*peak_mm, bounds.exceeds_mm, 0.0)
}

/// Compute the per-criterion delta for one candidate vs the baseline
/// verdict. Pure function — no tie-breaking with cycle time, no peak
/// magnitude judgment for `Within → Within`.
pub(crate) fn classify_candidate_vs_baseline(
    baseline: &ToolpathLoadVerdict,
    candidate: &ToolpathLoadVerdict,
) -> GateDeltas {
    GateDeltas {
        chipload: classify_one_gate_chipload(&baseline.chipload, &candidate.chipload),
        power: classify_one_gate_power(&baseline.power, &candidate.power),
        deflection: classify_one_gate_deflection(&baseline.deflection, &candidate.deflection),
    }
}

pub(crate) fn classify_one_gate_chipload(b: &ChiploadVerdict, c: &ChiploadVerdict) -> GateDelta {
    use ChiploadVerdict::*;
    match (b, c) {
        (Unmodeled { .. }, _) | (_, Unmodeled { .. }) => GateDelta::Unmodeled,
        (Exceeds { .. }, Within { .. }) => GateDelta::Improved,
        (Within { .. }, Exceeds { .. }) => GateDelta::Worsened,
        (Exceeds { triggering: bm, .. }, Exceeds { triggering: cm, .. }) => {
            classify_exceeds_pair(bm.observed_mm_per_tooth, cm.observed_mm_per_tooth)
        }
        (Within { .. }, Within { .. }) => GateDelta::Same,
    }
}

/// Power-typed counterpart to `classify_one_gate`. Same logic;
/// pulls peak from the typed `peak_kw` field. Will fold back into
/// a generic state+peak helper once chipload + deflection migrate.
pub(crate) fn classify_one_gate_power(b: &PowerVerdict, c: &PowerVerdict) -> GateDelta {
    use PowerVerdict::*;
    match (b, c) {
        (Unmodeled { .. }, _) | (_, Unmodeled { .. }) => GateDelta::Unmodeled,
        (Exceeds { .. }, Within { .. }) => GateDelta::Improved,
        (Within { .. }, Exceeds { .. }) => GateDelta::Worsened,
        (Exceeds { peak_kw: bp, .. }, Exceeds { peak_kw: cp, .. }) => {
            classify_exceeds_pair(*bp, *cp)
        }
        (Within { .. }, Within { .. }) => GateDelta::Same,
    }
}

/// Deflection-typed counterpart to `classify_one_gate`. Same shape as
/// the power version; reads `peak_mm` for the both-failing branch.
pub(crate) fn classify_one_gate_deflection(
    b: &DeflectionVerdict,
    c: &DeflectionVerdict,
) -> GateDelta {
    use DeflectionVerdict::*;
    match (b, c) {
        (Unmodeled { .. }, _) | (_, Unmodeled { .. }) => GateDelta::Unmodeled,
        (Exceeds { .. }, Within { .. }) => GateDelta::Improved,
        (Within { .. }, Exceeds { .. }) => GateDelta::Worsened,
        (Exceeds { peak_mm: bp, .. }, Exceeds { peak_mm: cp, .. }) => {
            classify_exceeds_pair(*bp, *cp)
        }
        (Within { .. }, Within { .. }) => GateDelta::Same,
    }
}

/// Both-Exceeds branch: peak comparison with a policy-derived
/// noise threshold so noise-level changes don't flip the classification.
pub(crate) fn classify_exceeds_pair(b_peak: f64, c_peak: f64) -> GateDelta {
    let policy = search_policy();
    let threshold = (b_peak.abs() * policy.ranking.failing_gate_relative_threshold.value)
        .max(policy.ranking.failing_gate_absolute_epsilon.value);
    if c_peak + threshold < b_peak {
        GateDelta::Improved
    } else if c_peak > b_peak + threshold {
        GateDelta::Worsened
    } else {
        GateDelta::Same
    }
}

/// Compute a [`ParamDelta`] from two `OperationConfig`s. Honors the
/// policy-defined feed display tolerance so sub-mm/min noise doesn't
/// surface as a "change" in the modal.
pub(crate) fn delta_against_baseline(
    baseline: &OperationConfig,
    candidate: &OperationConfig,
) -> ParamDelta {
    let mut delta = ParamDelta::default();
    let feed_delta_tolerance = search_policy().feed.delta_display_tolerance_mm_min.value;
    if (baseline.feed_rate() - candidate.feed_rate()).abs() > feed_delta_tolerance {
        delta.feed_mm_min = Some(candidate.feed_rate());
    }
    if baseline.spindle_rpm() != candidate.spindle_rpm() {
        delta.spindle_rpm = candidate.spindle_rpm();
    }
    if baseline.stepover() != candidate.stepover()
        && let Some(s) = candidate.stepover()
    {
        delta.stepover_mm = Some(s);
    }
    if baseline.depth_per_pass() != candidate.depth_per_pass()
        && let Some(d) = candidate.depth_per_pass()
    {
        delta.depth_per_pass_mm = Some(d);
    }
    if baseline.scallop_height() != candidate.scallop_height()
        && let Some(s) = candidate.scallop_height()
    {
        delta.scallop_height_mm = Some(s);
    }
    delta
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
    use crate::compute::operation_configs::PocketConfig;
    use crate::ids::ToolpathId;
    use crate::tool_load::verdict::{
        ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, Confidence,
        DeflectionBounds, SampleEvidence,
    };

    /// The G-CHIP-ULP reference bound: the value A-6's census found a
    /// multiply→divide round trip landing 1 ulp above in 6–8 % of the
    /// (rpm, flutes) grid. Reused here so the optimizer's tier decision is
    /// probed at the same number the gate's own contract is documented at.
    const REF_BOUND: f64 = 0.011_525_378_354_629_83;

    fn one_ulp_above(x: f64) -> f64 {
        f64::from_bits(x.to_bits() + 1)
    }

    fn one_ulp_below(x: f64) -> f64 {
        f64::from_bits(x.to_bits() - 1)
    }

    fn bounds(min: Option<f64>, max: f64) -> ChipBounds {
        ChipBounds {
            min_mm_per_tooth: min,
            max_mm_per_tooth: max,
            source: ChipBoundsSource::VendorLut,
        }
    }

    fn metric(observed: f64, b: ChipBounds) -> ChiploadMetric {
        ChiploadMetric {
            observed_mm_per_tooth: observed,
            statistic: ChiploadStatistic::PeakInRange,
            evidence: SampleEvidence::empty(),
            bounds: b,
        }
    }

    fn within_chipload(high_observed: f64, low: Option<(f64, f64)>) -> ChiploadVerdict {
        ChiploadVerdict::Within {
            approach_to_min: low.map(|(observed, min)| metric(observed, bounds(Some(min), 1.0))),
            approach_to_max: metric(high_observed, bounds(None, REF_BOUND)),
            confidence: Confidence::Validated,
            entry_spikes: Vec::new(),
            burn_advisory: None,
            ceiling_advisory: None,
        }
    }

    fn within_power(peak_kw: f64, available_kw: f64) -> PowerVerdict {
        PowerVerdict::Within {
            peak_kw,
            available_kw,
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
            entry_spike: None,
        }
    }

    fn within_deflection(peak_mm: f64) -> DeflectionVerdict {
        DeflectionVerdict::Within {
            peak_mm,
            bounds: DeflectionBounds {
                validated_within_mm: 0.050,
                exceeds_mm: 0.200,
            },
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
            entry_spike: None,
        }
    }

    fn candidate(verdict: ToolpathLoadVerdict) -> OptimizeCandidate {
        OptimizeCandidate {
            params: OperationConfig::Pocket(PocketConfig::default()),
            delta: ParamDelta::default(),
            cycle_time_s: 100.0,
            verdict,
            stage: super::super::SearchStage::Refined,
            reconciled_cycle_time_s: None,
            reconciled_verdict: None,
            gate_deltas: None,
            air_cut_fraction_of_total_runtime: None,
        }
    }

    fn verdict_with(chipload: ChiploadVerdict) -> ToolpathLoadVerdict {
        ToolpathLoadVerdict {
            toolpath_id: ToolpathId(0),
            chipload,
            power: within_power(0.4, 1.0),
            deflection: within_deflection(0.020),
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
        }
    }

    /// **Checkpoint P (2) — the tier flip, at one ulp.**
    ///
    /// A candidate the epsilon-aware chipload gate has already called `Within`
    /// arrives here with its observation sitting **1 ulp above** the band
    /// ceiling — the reconstruction noise `tool_load::boundary` exists to
    /// absorb, routine on the recipes the rubbing-floor clamp parks exactly on
    /// the ceiling.
    ///
    /// RED at the parent (`e94be53a`): `chipload_within_breaches_strict` wrote
    /// a bare `observed > bounds.max_mm_per_tooth`, so this candidate was
    /// re-decided as a strict breach and routed to `MarginalSafe` — the modal
    /// says **"verify on a scrap"** instead of auto-recommending, on the last
    /// bit of a multiply/divide round trip.
    ///
    /// GREEN after: the comparison goes through `ChipBounds::exceeds_high`,
    /// which carries `BOUNDARY_EPSILON_REL`, and the tier is `strictly safe`.
    #[test]
    fn one_ulp_above_the_ceiling_does_not_demote_the_tier() {
        let c = candidate(verdict_with(within_chipload(
            one_ulp_above(REF_BOUND),
            None,
        )));
        assert!(
            !candidate_is_marginally_safe(&c),
            "a 1-ulp reconstruction must not route an auto-recommendable \
             candidate to verify-on-scrap (observed {:.17e} vs bound \
             {REF_BOUND:.17e})",
            one_ulp_above(REF_BOUND)
        );
        assert!(
            candidate_is_strictly_safe(&c),
            "and it must land in the auto-recommend tier"
        );
    }

    /// The other half of the same contract: the epsilon is not a tolerance in
    /// disguise. A breach twelve orders of magnitude smaller than 5 % still
    /// demotes the tier, so P-(2) cannot be read as widening the band.
    #[test]
    fn a_genuine_breach_still_demotes_the_tier() {
        let c = candidate(verdict_with(within_chipload(
            REF_BOUND * (1.0 + 1e-9),
            None,
        )));
        assert!(
            candidate_is_marginally_safe(&c),
            "a real overshoot must still route to verify-on-scrap"
        );
        assert!(!candidate_is_strictly_safe(&c));
    }

    /// The low side, same shape. `below_low` returns `Option<bool>`; the
    /// upper-bound-only case must stay `None`-safe rather than collapsing to
    /// "breached".
    #[test]
    fn the_low_side_absorbs_one_ulp_and_still_catches_a_real_burn() {
        let noise = candidate(verdict_with(within_chipload(
            REF_BOUND * 0.5,
            Some((one_ulp_below(0.032), 0.032)),
        )));
        assert!(
            !candidate_is_marginally_safe(&noise),
            "1 ulp below the floor is reconstruction noise, not a burn"
        );

        let real = candidate(verdict_with(within_chipload(
            REF_BOUND * 0.5,
            Some((0.032 * (1.0 - 1e-9), 0.032)),
        )));
        assert!(
            candidate_is_marginally_safe(&real),
            "a genuine sub-floor median must still demote"
        );

        let no_floor = candidate(verdict_with(within_chipload(REF_BOUND * 0.5, None)));
        assert!(
            !candidate_is_marginally_safe(&no_floor),
            "an upper-bound-only row has no floor to breach"
        );
    }

    /// Sites 3 and 4 of the sweep: power and deflection re-decide their own
    /// `Within` the same way and reach `boundary::exceeds_high` now.
    #[test]
    fn power_and_deflection_re_decisions_absorb_one_ulp_too() {
        let mut v = verdict_with(within_chipload(REF_BOUND * 0.5, None));
        v.power = within_power(one_ulp_above(1.0), 1.0);
        assert!(
            !candidate_is_marginally_safe(&candidate(v.clone())),
            "1 ulp over available_kw is not a power breach"
        );
        v.power = within_power(1.0 * (1.0 + 1e-9), 1.0);
        assert!(
            candidate_is_marginally_safe(&candidate(v)),
            "a genuine power overshoot still demotes"
        );

        let mut d = verdict_with(within_chipload(REF_BOUND * 0.5, None));
        d.deflection = within_deflection(one_ulp_above(0.200));
        assert!(
            !candidate_is_marginally_safe(&candidate(d.clone())),
            "1 ulp over the deflection bound is not a breach"
        );
        d.deflection = within_deflection(0.200 * (1.0 + 1e-9));
        assert!(
            candidate_is_marginally_safe(&candidate(d)),
            "a genuine deflection overshoot still demotes"
        );
    }
}
