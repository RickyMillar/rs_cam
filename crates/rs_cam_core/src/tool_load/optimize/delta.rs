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
/// Used by the modal to render "feed 1899→2100" style summaries and by
/// the Apply path to know which `feeds_auto.*` flags need clearing.
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
    !candidate.verdict.chipload.is_exceeded()
        && !candidate.verdict.power.is_exceeded()
        && !candidate.verdict.deflection.is_exceeded()
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
        || power_within_breaches_strict(&candidate.verdict.power)
        || deflection_within_breaches_strict(&candidate.verdict.deflection)
}

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
    if approach_to_max.observed_mm_per_tooth > approach_to_max.bounds.max_mm_per_tooth {
        return true;
    }
    // Low-side breach: median observed below the strict LUT min, when
    // the matched row publishes a min (some rows are upper-bound only).
    if let Some(min_metric) = approach_to_min
        && let Some(strict_min) = min_metric.bounds.min_mm_per_tooth
        && min_metric.observed_mm_per_tooth < strict_min
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
    *peak_kw > *available_kw
}

fn deflection_within_breaches_strict(v: &DeflectionVerdict) -> bool {
    let DeflectionVerdict::Within {
        peak_mm, bounds, ..
    } = v
    else {
        return false;
    };
    *peak_mm > bounds.exceeds_mm
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
