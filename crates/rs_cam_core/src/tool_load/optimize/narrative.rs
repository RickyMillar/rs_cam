//! Structured failure / trade-off narratives for `OptimizeOutcome`.
//!
//! A1 of `planning/OPTIMIZE_EXPLAINABILITY_AND_PEAK_FINDING.md` —
//! promotes the free `explanation: String` to typed fields the UI
//! (A2) and any agent client (MCP) can render selectively.
//!
//! - [`FailureNarrative`] attaches to `NoSafeImprovement` and
//!   `MarginalSafe`. Carries: a one-line headline, per-gate limiting
//!   readings on the closest-to-safe candidate, the search envelope
//!   reached, and any operator-actionable suggestions (A4 fills these;
//!   A1 emits an empty `Vec`).
//! - [`TradeOffNarrative`] attaches to `TradeOff`. Carries headline +
//!   improved / worsened gate lists + envelope.
//!
//! Builders here are pure functions over the attempted candidate set;
//! `build_outcome` calls them. Headlines are mechanical for A1 — A2
//! can rephrase them in the UI without touching this module.
//!
//! **Vocabulary contract:** when thread B (peak-finding) lands, the
//! per-probe rationale must reuse [`LimitingGate`] / [`OperatorSuggestion`]
//! enums so engine and UI converge on the same operator-facing words.

use serde::{Deserialize, Serialize};

use crate::tool_load::verdict::{
    ChipSide, ChiploadVerdict, DeflectionVerdict, PowerVerdict, ToolpathLoadVerdict,
};

use super::OptimizeCandidate;

/// Narrative attached to `NoSafeImprovement` and `MarginalSafe`.
/// `Default` produces an empty narrative — useful for test fixtures
/// that construct outcomes by hand without going through `build_outcome`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FailureNarrative {
    /// One-line headline for the modal. Operator-facing language;
    /// mechanically generated for A1.
    pub headline: String,
    /// Per-gate limiting readings on the closest-to-safe candidate.
    /// For `NoSafeImprovement`, an Exceeds gate; for `MarginalSafe`,
    /// a Within reading admitted only by the phase-1 tolerance band.
    pub limiting_gates: Vec<LimitingGate>,
    /// Extents per knob axis across the attempted candidate set —
    /// surfaces "We tried feeds up to X mm/min" without the UI
    /// re-computing.
    pub envelope: SearchEnvelopeReached,
    /// Operator-actionable suggestions. A4 will populate from
    /// heuristics; A1 emits `Vec::new()`.
    pub suggestions: Vec<OperatorSuggestion>,
}

/// Narrative attached to `TradeOff`. Distinct shape from
/// `FailureNarrative` because trade-offs are described by which gates
/// moved up vs down, not by a single limiting reading. `Default`
/// produces an empty narrative for test fixtures.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TradeOffNarrative {
    pub headline: String,
    pub improved_gates: Vec<GateKind>,
    pub worsened_gates: Vec<GateKind>,
    pub envelope: SearchEnvelopeReached,
}

/// One per-gate limiting reading. The same shape covers
/// "Exceeds — over the bound" (NoSafeImprovement) and
/// "Within but admitted by tolerance band" (MarginalSafe), with
/// `band_admitted` distinguishing the cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitingGate {
    pub gate: GateKind,
    /// Only meaningful for chipload (Low burn / High breakage). `None`
    /// for power / deflection.
    pub side: Option<ChipSide>,
    /// Observed value at the limiting sample.
    pub observed: f64,
    /// LUT / model bound the observation was compared against.
    /// For chipload Low this is `min_mm_per_tooth`; for chipload
    /// High this is `max_mm_per_tooth`. For power, `available_kw`.
    /// For deflection, `bounds.exceeds_mm`.
    pub bound: f64,
    /// Signed fractional overshoot: `(observed - bound) / bound`.
    /// Positive for High / over-the-bound readings, negative for Low /
    /// under-the-bound readings.
    pub overshoot_fraction: f64,
    /// True if the gate verdict is `Within` and this reading exceeded
    /// the strict LUT bound but was admitted by the phase-1 tolerance
    /// band (G16 §11.4 layer 1). False otherwise — including all
    /// `Exceeds` cases.
    pub band_admitted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    Chipload,
    Power,
    Deflection,
}

/// Knob-axis extents observed across attempted candidates. Each field
/// is `Some` when at least one candidate moved that axis off baseline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchEnvelopeReached {
    pub feed_mm_min: Option<AxisExtent>,
    pub spindle_rpm: Option<AxisExtent>,
    pub stepover_mm: Option<AxisExtent>,
    pub depth_per_pass_mm: Option<AxisExtent>,
    pub scallop_height_mm: Option<AxisExtent>,
}

/// Min / max range observed for one axis across the attempted set.
/// `min == max` when only one value was tried (typical for axes the
/// search held fixed).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AxisExtent {
    pub min: f64,
    pub max: f64,
}

/// Operator-actionable suggestion. A1 always emits `Vec::new()`; A4
/// populates from per-gate heuristics. Units are implied by `KnobAxis`
/// (Feed = mm/min, SpindleRpm = rpm, Stepover / DepthPerPass /
/// ScallopHeight = mm).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperatorSuggestion {
    /// "Cap feed below ~3700 mm/min and re-optimize."
    CapAxisAt { axis: KnobAxis, ceiling: f64 },
    /// "Reduce stepover below ~1.6 mm to avoid full-slot engagement."
    NarrowAxisBelow { axis: KnobAxis, ceiling: f64 },
    /// "No vendor LUT data above ~0.055 chipload — calibration would help."
    DataGapHere { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnobAxis {
    Feed,
    SpindleRpm,
    Stepover,
    DepthPerPass,
    ScallopHeight,
}

/// Build a `FailureNarrative` for a `NoSafeImprovement` outcome. The
/// closest-to-safe candidate is the first non-baseline entry in the
/// attempted set (already sorted by composite score in `build_outcome`).
/// `limiting_gates` are its `Exceeds` readings.
pub(crate) fn build_failure_narrative_no_safe(
    baseline: &OptimizeCandidate,
    attempted: &[OptimizeCandidate],
) -> FailureNarrative {
    let envelope = envelope_across(baseline, attempted);
    let limiting_gates = attempted
        .get(1)
        .map(|c| limiting_gates_from_exceeds(&c.verdict))
        .unwrap_or_default();
    let headline = headline_no_safe(attempted.len().saturating_sub(1), &limiting_gates);
    FailureNarrative {
        headline,
        limiting_gates,
        envelope,
        suggestions: Vec::new(),
    }
}

/// Build a `FailureNarrative` for a `MarginalSafe` outcome. The
/// recommended candidate is the first non-baseline entry that is
/// marginally safe (every gate Within, at least one reading
/// band-admitted). `limiting_gates` are its band-admitted readings.
pub(crate) fn build_failure_narrative_marginal(
    baseline: &OptimizeCandidate,
    candidates: &[OptimizeCandidate],
) -> FailureNarrative {
    let envelope = envelope_across(baseline, candidates);
    let limiting_gates = candidates
        .iter()
        .skip(1)
        .find(|c| !limiting_gates_from_band_admit(&c.verdict).is_empty())
        .map(|c| limiting_gates_from_band_admit(&c.verdict))
        .unwrap_or_default();
    let headline = headline_marginal(&limiting_gates);
    FailureNarrative {
        headline,
        limiting_gates,
        envelope,
        suggestions: Vec::new(),
    }
}

/// Build a `TradeOffNarrative`. Reads the recommended candidate's
/// `gate_deltas` (populated by `build_outcome`) to list improved /
/// worsened gates.
pub(crate) fn build_tradeoff_narrative(
    baseline: &OptimizeCandidate,
    candidates: &[OptimizeCandidate],
) -> TradeOffNarrative {
    let envelope = envelope_across(baseline, candidates);
    let (improved_gates, worsened_gates) = candidates
        .iter()
        .skip(1)
        .find_map(|c| c.gate_deltas.map(|d| split_improved_worsened(&d)))
        .unwrap_or_default();
    let headline = headline_tradeoff(&improved_gates, &worsened_gates);
    TradeOffNarrative {
        headline,
        improved_gates,
        worsened_gates,
        envelope,
    }
}

/// All non-clean readings for one candidate's verdict — both `Exceeds`
/// gates and `Within`-but-band-admitted readings. UI uses this to
/// render per-row "what stopped this candidate" badges in A2's modal
/// rework. Returns an empty Vec for a strictly-safe verdict.
pub fn limiting_gates_for_verdict(verdict: &ToolpathLoadVerdict) -> Vec<LimitingGate> {
    let mut out = limiting_gates_from_exceeds(verdict);
    out.extend(limiting_gates_from_band_admit(verdict));
    out
}

fn limiting_gates_from_exceeds(verdict: &ToolpathLoadVerdict) -> Vec<LimitingGate> {
    let mut out = Vec::new();
    if let ChiploadVerdict::Exceeds {
        side, triggering, ..
    } = &verdict.chipload
    {
        let bound = match side {
            ChipSide::High => triggering.bounds.max_mm_per_tooth,
            ChipSide::Low => triggering.bounds.min_mm_per_tooth.unwrap_or(f64::NAN),
        };
        out.push(LimitingGate {
            gate: GateKind::Chipload,
            side: Some(*side),
            observed: triggering.observed_mm_per_tooth,
            bound,
            overshoot_fraction: signed_overshoot(triggering.observed_mm_per_tooth, bound),
            band_admitted: false,
        });
    }
    if let PowerVerdict::Exceeds {
        peak_kw,
        available_kw,
        ..
    } = &verdict.power
    {
        out.push(LimitingGate {
            gate: GateKind::Power,
            side: None,
            observed: *peak_kw,
            bound: *available_kw,
            overshoot_fraction: signed_overshoot(*peak_kw, *available_kw),
            band_admitted: false,
        });
    }
    if let DeflectionVerdict::Exceeds {
        peak_mm, bounds, ..
    } = &verdict.deflection
    {
        out.push(LimitingGate {
            gate: GateKind::Deflection,
            side: None,
            observed: *peak_mm,
            bound: bounds.exceeds_mm,
            overshoot_fraction: signed_overshoot(*peak_mm, bounds.exceeds_mm),
            band_admitted: false,
        });
    }
    out
}

fn limiting_gates_from_band_admit(verdict: &ToolpathLoadVerdict) -> Vec<LimitingGate> {
    // A band-admitted Within reading is one whose observed value is
    // outside the strict LUT bound but inside the phase-1 tolerance
    // band (G16 §11.4). For A1 we surface the chipload high-side band
    // admit, which is the only producer today (power_breach_tolerance
    // and deflection_breach_tolerance default to 0).
    let mut out = Vec::new();
    if let ChiploadVerdict::Within {
        approach_to_max, ..
    } = &verdict.chipload
        && approach_to_max.observed_mm_per_tooth > approach_to_max.bounds.max_mm_per_tooth
    {
        out.push(LimitingGate {
            gate: GateKind::Chipload,
            side: Some(ChipSide::High),
            observed: approach_to_max.observed_mm_per_tooth,
            bound: approach_to_max.bounds.max_mm_per_tooth,
            overshoot_fraction: signed_overshoot(
                approach_to_max.observed_mm_per_tooth,
                approach_to_max.bounds.max_mm_per_tooth,
            ),
            band_admitted: true,
        });
    }
    if let ChiploadVerdict::Within {
        approach_to_min: Some(metric),
        ..
    } = &verdict.chipload
        && let Some(strict_min) = metric.bounds.min_mm_per_tooth
        && metric.observed_mm_per_tooth < strict_min
    {
        out.push(LimitingGate {
            gate: GateKind::Chipload,
            side: Some(ChipSide::Low),
            observed: metric.observed_mm_per_tooth,
            bound: strict_min,
            overshoot_fraction: signed_overshoot(metric.observed_mm_per_tooth, strict_min),
            band_admitted: true,
        });
    }
    out
}

fn split_improved_worsened(deltas: &super::delta::GateDeltas) -> (Vec<GateKind>, Vec<GateKind>) {
    use super::delta::GateDelta;
    let mut improved = Vec::new();
    let mut worsened = Vec::new();
    if matches!(deltas.chipload, GateDelta::Improved) {
        improved.push(GateKind::Chipload);
    }
    if matches!(deltas.chipload, GateDelta::Worsened) {
        worsened.push(GateKind::Chipload);
    }
    if matches!(deltas.power, GateDelta::Improved) {
        improved.push(GateKind::Power);
    }
    if matches!(deltas.power, GateDelta::Worsened) {
        worsened.push(GateKind::Power);
    }
    if matches!(deltas.deflection, GateDelta::Improved) {
        improved.push(GateKind::Deflection);
    }
    if matches!(deltas.deflection, GateDelta::Worsened) {
        worsened.push(GateKind::Deflection);
    }
    (improved, worsened)
}

fn envelope_across(
    baseline: &OptimizeCandidate,
    candidates: &[OptimizeCandidate],
) -> SearchEnvelopeReached {
    let mut env = SearchEnvelopeReached::default();
    let configs = std::iter::once(&baseline.params).chain(candidates.iter().map(|c| &c.params));
    for cfg in configs {
        let (feed, rpm, stepover, doc, scallop) = config_axes(cfg);
        if let Some(v) = feed {
            extend_extent(&mut env.feed_mm_min, v);
        }
        if let Some(v) = rpm {
            extend_extent(&mut env.spindle_rpm, v);
        }
        if let Some(v) = stepover {
            extend_extent(&mut env.stepover_mm, v);
        }
        if let Some(v) = doc {
            extend_extent(&mut env.depth_per_pass_mm, v);
        }
        if let Some(v) = scallop {
            extend_extent(&mut env.scallop_height_mm, v);
        }
    }
    env
}

/// `(feed, rpm, stepover, depth, scallop_height)` tuple.
type ConfigAxes = (
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
);

/// Read the five optimizer-known axes off an `OperationConfig`. The
/// envelope only needs the axes the optimizer actually moves; we
/// enumerate the four families the optimizer targets today
/// (Pocket / Adaptive / Adaptive3d / Scallop) and fall through to
/// `(None, …)` for everything else. Adding more is an op-by-op call
/// later.
fn config_axes(cfg: &crate::compute::catalog::OperationConfig) -> ConfigAxes {
    use crate::compute::catalog::OperationConfig as Op;
    match cfg {
        Op::Pocket(p) => (
            Some(p.feed_rate),
            None,
            Some(p.stepover),
            Some(p.depth_per_pass),
            None,
        ),
        Op::Adaptive(p) => (
            Some(p.feed_rate),
            None,
            Some(p.stepover),
            Some(p.depth_per_pass),
            None,
        ),
        Op::Adaptive3d(p) => (
            Some(p.feed_rate),
            p.spindle_rpm.map(|r| r as f64),
            Some(p.stepover),
            Some(p.depth_per_pass),
            None,
        ),
        Op::Scallop(p) => (
            Some(p.feed_rate),
            None,
            None,
            None,
            Some(p.scallop_height),
        ),
        _ => (None, None, None, None, None),
    }
}

fn extend_extent(extent: &mut Option<AxisExtent>, value: f64) {
    if !value.is_finite() {
        return;
    }
    match extent {
        Some(e) => {
            if value < e.min {
                e.min = value;
            }
            if value > e.max {
                e.max = value;
            }
        }
        None => {
            *extent = Some(AxisExtent {
                min: value,
                max: value,
            });
        }
    }
}

fn signed_overshoot(observed: f64, bound: f64) -> f64 {
    if bound.abs() < 1e-12 {
        return 0.0;
    }
    (observed - bound) / bound
}

fn headline_no_safe(non_baseline_count: usize, limiting: &[LimitingGate]) -> String {
    if non_baseline_count == 0 {
        return "No candidates were produced — the search space is empty for this op.".to_owned();
    }
    if limiting.is_empty() {
        return format!(
            "Tried {non_baseline_count} candidates; none were faster than baseline by enough to recommend."
        );
    }
    let pieces: Vec<String> = limiting
        .iter()
        .map(|g| match g.gate {
            GateKind::Chipload => match g.side {
                Some(ChipSide::High) => format!(
                    "chipload {:.4} mm/tooth ({:+.0}% over LUT max {:.4})",
                    g.observed,
                    g.overshoot_fraction * 100.0,
                    g.bound,
                ),
                Some(ChipSide::Low) => format!(
                    "chipload {:.4} mm/tooth ({:+.0}% below LUT min {:.4})",
                    g.observed,
                    g.overshoot_fraction * 100.0,
                    g.bound,
                ),
                None => format!("chipload {:.4} mm/tooth", g.observed),
            },
            GateKind::Power => format!(
                "power {:.2} kW ({:+.0}% over available {:.2})",
                g.observed,
                g.overshoot_fraction * 100.0,
                g.bound,
            ),
            GateKind::Deflection => format!(
                "deflection {:.0} µm ({:+.0}% over the {:.0} µm threshold)",
                g.observed * 1000.0,
                g.overshoot_fraction * 100.0,
                g.bound * 1000.0,
            ),
        })
        .collect();
    format!(
        "Tried {non_baseline_count} candidates; closest-to-safe still hit: {}.",
        pieces.join("; "),
    )
}

fn headline_marginal(limiting: &[LimitingGate]) -> String {
    let admitted = limiting.iter().find(|g| g.band_admitted);
    match admitted {
        Some(g) => format!(
            "Best candidate is {:+.0}% past the strict {} bound — admitted by the tolerance band; verify on a scrap before applying.",
            g.overshoot_fraction * 100.0,
            match g.gate {
                GateKind::Chipload => "chipload",
                GateKind::Power => "power",
                GateKind::Deflection => "deflection",
            },
        ),
        None => "Best candidate is admitted only by the layer-1 tolerance band — verify on a scrap before applying.".to_owned(),
    }
}

fn headline_tradeoff(improved: &[GateKind], worsened: &[GateKind]) -> String {
    let label = |g: &GateKind| match g {
        GateKind::Chipload => "chipload",
        GateKind::Power => "power",
        GateKind::Deflection => "deflection",
    };
    let imp = improved.iter().map(label).collect::<Vec<_>>().join(" + ");
    let wor = worsened.iter().map(label).collect::<Vec<_>>().join(" + ");
    if imp.is_empty() || wor.is_empty() {
        return "Faster candidate found, but the gate trade-off is mixed.".to_owned();
    }
    format!("Faster candidate improves {imp} but worsens {wor}.")
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
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::PocketConfig;
    use crate::tool_load::optimize::delta::{GateDelta, GateDeltas, ParamDelta};
    use crate::tool_load::optimize::{OptimizeCandidate, SearchStage};
    use crate::tool_load::verdict::{
        ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, Confidence,
        DeflectionBounds, SampleEvidence, ToolpathLoadVerdict,
    };

    fn candidate(
        feed: f64,
        cycle: f64,
        verdict: ToolpathLoadVerdict,
        gate_deltas: Option<GateDeltas>,
    ) -> OptimizeCandidate {
        OptimizeCandidate {
            params: OperationConfig::Pocket(PocketConfig {
                feed_rate: feed,
                ..PocketConfig::default()
            }),
            delta: ParamDelta {
                feed_mm_min: Some(feed),
                ..Default::default()
            },
            cycle_time_s: cycle,
            verdict,
            stage: SearchStage::Refined,
            reconciled_cycle_time_s: None,
            reconciled_verdict: None,
            gate_deltas,
        }
    }

    fn within_chipload(observed: f64) -> ChiploadVerdict {
        ChiploadVerdict::Within {
            approach_to_min: Some(ChiploadMetric {
                observed_mm_per_tooth: observed,
                statistic: ChiploadStatistic::MedianLow,
                evidence: SampleEvidence::empty(),
                bounds: ChipBounds {
                    min_mm_per_tooth: Some(0.038),
                    max_mm_per_tooth: 0.07,
                    source: ChipBoundsSource::VendorLut,
                },
            }),
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: observed,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::empty(),
                bounds: ChipBounds {
                    min_mm_per_tooth: Some(0.038),
                    max_mm_per_tooth: 0.07,
                    source: ChipBoundsSource::VendorLut,
                },
            },
            confidence: Confidence::Validated,
        }
    }

    fn exceeds_chipload_high(peak: f64) -> ChiploadVerdict {
        ChiploadVerdict::Exceeds {
            side: ChipSide::High,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: peak,
                statistic: ChiploadStatistic::PeakHigh,
                evidence: SampleEvidence::at(0),
                bounds: ChipBounds {
                    min_mm_per_tooth: Some(0.038),
                    max_mm_per_tooth: 0.07,
                    source: ChipBoundsSource::VendorLut,
                },
            },
            confidence: Confidence::Validated,
        }
    }

    fn exceeds_deflection(peak_mm: f64) -> DeflectionVerdict {
        DeflectionVerdict::Exceeds {
            peak_mm,
            bounds: DeflectionBounds {
                validated_within_mm: 0.050,
                exceeds_mm: 0.200,
            },
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        }
    }

    fn within_power() -> PowerVerdict {
        PowerVerdict::Within {
            peak_kw: 0.05,
            available_kw: 0.6,
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
        }
    }

    fn within_deflection(peak_mm: f64) -> DeflectionVerdict {
        DeflectionVerdict::Within {
            peak_mm,
            bounds: DeflectionBounds {
                validated_within_mm: 0.050,
                exceeds_mm: 0.200,
            },
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
        }
    }

    fn vd(chipload: ChiploadVerdict, defl: DeflectionVerdict) -> ToolpathLoadVerdict {
        ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload,
            power: within_power(),
            deflection: defl,
        }
    }

    #[test]
    fn no_safe_narrative_surfaces_chipload_high_overshoot() {
        // Wanaka TP 1 case: every refined candidate Exceeds chipload
        // High at +28%; closest-to-safe is the first non-baseline.
        let baseline = candidate(
            3150.0,
            773.0,
            vd(within_chipload(0.025), within_deflection(0.158)),
            None,
        );
        let attempted = vec![
            baseline.clone(),
            candidate(
                4000.0,
                431.0,
                vd(exceeds_chipload_high(0.0707), exceeds_deflection(0.237)),
                Some(GateDeltas {
                    chipload: GateDelta::Worsened,
                    power: GateDelta::Same,
                    deflection: GateDelta::Worsened,
                }),
            ),
            candidate(
                4000.0,
                474.0,
                vd(exceeds_chipload_high(0.0707), within_deflection(0.158)),
                Some(GateDeltas {
                    chipload: GateDelta::Worsened,
                    power: GateDelta::Same,
                    deflection: GateDelta::Same,
                }),
            ),
        ];
        let n = build_failure_narrative_no_safe(&baseline, &attempted);
        assert!(
            n.headline.contains("chipload"),
            "headline should mention chipload, got: {}",
            n.headline,
        );
        assert!(
            n.headline.contains("over LUT max") || n.headline.contains("over"),
            "headline should mention overshoot direction, got: {}",
            n.headline,
        );
        assert_eq!(n.limiting_gates.len(), 2, "chipload + deflection both Exceeds");
        let chipload_gate = n
            .limiting_gates
            .iter()
            .find(|g| matches!(g.gate, GateKind::Chipload))
            .expect("chipload gate present");
        assert_eq!(chipload_gate.side, Some(ChipSide::High));
        assert!(!chipload_gate.band_admitted);
        assert!(chipload_gate.overshoot_fraction > 0.0);
        // Envelope should span feed 3150 → 4000.
        let feed = n.envelope.feed_mm_min.expect("feed extent present");
        assert!((feed.min - 3150.0).abs() < 1e-9);
        assert!((feed.max - 4000.0).abs() < 1e-9);
        assert!(n.suggestions.is_empty(), "A1 emits no suggestions");
    }

    #[test]
    fn marginal_narrative_surfaces_band_admitted_chipload() {
        // Wanaka TP 6 case: best candidate has chipload peak 0.072
        // (1.3% over strict max 0.07) admitted by the 5% breakage_tolerance
        // band. limiting_gates.band_admitted should be true.
        let baseline = candidate(
            3150.0,
            200.0,
            vd(within_chipload(0.072), within_deflection(0.157)),
            None,
        );
        let candidates = vec![
            baseline.clone(),
            candidate(
                3500.0,
                188.0,
                vd(within_chipload(0.072), within_deflection(0.157)),
                Some(GateDeltas {
                    chipload: GateDelta::Same,
                    power: GateDelta::Same,
                    deflection: GateDelta::Same,
                }),
            ),
        ];
        let n = build_failure_narrative_marginal(&baseline, &candidates);
        let admitted = n
            .limiting_gates
            .iter()
            .find(|g| g.band_admitted)
            .expect("at least one band-admitted reading");
        assert!(matches!(admitted.gate, GateKind::Chipload));
        assert_eq!(admitted.side, Some(ChipSide::High));
        assert!(admitted.overshoot_fraction > 0.0);
        assert!(
            n.headline.contains("tolerance band") || n.headline.contains("scrap"),
            "headline should mention scrap / tolerance band, got: {}",
            n.headline,
        );
    }

    #[test]
    fn tradeoff_narrative_lists_improved_and_worsened() {
        // Synthetic: baseline Exceeds chipload, candidate brings
        // chipload Within but pushes deflection over.
        let baseline = candidate(
            3000.0,
            500.0,
            vd(exceeds_chipload_high(0.08), within_deflection(0.030)),
            None,
        );
        let candidates = vec![
            baseline.clone(),
            candidate(
                2500.0,
                450.0,
                vd(within_chipload(0.040), exceeds_deflection(0.250)),
                Some(GateDeltas {
                    chipload: GateDelta::Improved,
                    power: GateDelta::Same,
                    deflection: GateDelta::Worsened,
                }),
            ),
        ];
        let n = build_tradeoff_narrative(&baseline, &candidates);
        assert_eq!(n.improved_gates, vec![GateKind::Chipload]);
        assert_eq!(n.worsened_gates, vec![GateKind::Deflection]);
        assert!(
            n.headline.contains("chipload") && n.headline.contains("deflection"),
            "headline should reference both gates, got: {}",
            n.headline,
        );
    }
}
