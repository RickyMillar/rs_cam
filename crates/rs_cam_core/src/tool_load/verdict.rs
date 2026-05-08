//! Verdict and confidence types for the tool-load monitor.
//!
//! Each guardrail criterion (chipload, power, deflection, ...) reports an
//! independent `Verdict`. There is no scalar "load %" — a project-wide
//! report is a vector of per-criterion verdicts per toolpath. A criterion that
//! cannot be evaluated honestly returns `Unmodeled` with a typed reason; it
//! never silently falls back to a passing or failing value.

use std::ops::Range;

use serde::{Deserialize, Serialize};

/// Why a criterion could not be evaluated. Typed (not free-form strings) so
/// callers can branch and the UI can localize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum UnmodeledReason {
    /// No simulation has been run, or the cached trace doesn't cover this
    /// toolpath. The criterion needs per-sample metrics.
    SimulationRequired,
    /// A simulation trace exists, but its provenance hashes don't match the
    /// current project state — the toolpath, tool, stock, or machine has
    /// changed since it was captured.
    StaleSimulation,
    /// The simulation was run without arc-engagement capture enabled.
    /// Re-run with `MetricOptions::capture_arc_engagement = true`.
    ArcEngagementNotCaptured,
    /// No vendor LUT row matches the (tool family, material family) tuple
    /// for this toolpath. The chipload bounds are unknown.
    NoVendorData,
    /// The simulation trace exists, but no samples for this toolpath are
    /// running at the operation's commanded feed rate (steady-state
    /// cutting). Typically a pure-plunge drill cycle, or a toolpath where
    /// every sample is a ramp/entry move at a different feed. The
    /// chipload-vs-LUT comparison is calibrated for steady-state cutting,
    /// so we refuse rather than flag transient feeds against it.
    SteadyStateSamplesNotPresent,
    /// The material is `Custom` without an explicitly-validated `kc`.
    /// We refuse to compute a force-derived envelope from a guessed Kc.
    MaterialUnvalidated,
    /// The cutter shape cannot model the engagement mode in this region
    /// (e.g. V-bit at the tip, ball nose past the hemisphere pole).
    /// The free-form `String` carries the cutter-supplied reason.
    CutterModeUnsupported(String),
    /// The criterion is intentionally not implemented yet (deferred to a
    /// later phase). The string names the phase or follow-up.
    NotImplemented(String),
}

/// What a "Within" or "Exceeds" verdict claims about its inputs.
///
/// `Validated` is rare — it means every input was independently checked.
/// Most useful results are `Approximate` with a typed reason; UI must render
/// `Approximate` differently from `Validated` so users don't anchor on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum Confidence {
    /// All inputs validated; the verdict is trustworthy.
    Validated,
    /// Verdict is best-effort given known input limitations. The string
    /// describes which input is approximate (e.g. "isotropic Kc only",
    /// "slot-engagement decomposition").
    Approximate(String),
}

/// A single criterion's outcome for a single toolpath.
///
/// `peak` is the criterion-specific scalar that drove the verdict — for
/// chipload it's mm/tooth, for L/D it's the ratio. Always carries a unit
/// in the criterion's documentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Verdict {
    /// Criterion modeled and within bounds.
    Within { peak: f64, confidence: Confidence },
    /// Criterion modeled and out of bounds. `sample_range` is the
    /// half-open per-toolpath sample index range that triggered (empty
    /// for criteria that don't have per-sample resolution, e.g. L/D).
    Exceeds {
        peak: f64,
        sample_range: Range<usize>,
        reason: ExceedsReason,
        confidence: Confidence,
    },
    /// Criterion not evaluated; reason is typed.
    Unmodeled { reason: UnmodeledReason },
}

/// Why a criterion exceeded its bound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExceedsReason {
    /// Chipload below vendor min — rubbing/burning risk, dulls the edge.
    ChiploadBurnRisk,
    /// Chipload above vendor max — breakage risk.
    ChiploadBreakageRisk,
    /// Cantilever L/D too long — tool stiffness inadequate regardless of
    /// load. Geometric only; no force inputs.
    LongToolStiffnessUnsafe,
    /// Instantaneous spindle power exceeds available power × safety factor.
    SpindlePowerExceeded,
}

impl Verdict {
    /// Convenience: a criterion that this phase doesn't implement yet.
    pub fn not_implemented(phase: &str) -> Self {
        Verdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented(phase.to_owned()),
        }
    }

    /// True if the verdict is `Exceeds`. Used by the export gate.
    pub fn is_exceeded(&self) -> bool {
        matches!(self, Verdict::Exceeds { .. })
    }

    /// True if the verdict is `Unmodeled`. Used by the export gate.
    pub fn is_unmodeled(&self) -> bool {
        matches!(self, Verdict::Unmodeled { .. })
    }

    /// Coarse outcome shared with the typed verdicts. Lets viz / export
    /// iterate over all three gates without knowing each variant.
    pub fn state(&self) -> LoadState {
        match self {
            Verdict::Within { .. } => LoadState::Within,
            Verdict::Exceeds { .. } => LoadState::Exceeds,
            Verdict::Unmodeled { .. } => LoadState::Unmodeled,
        }
    }

    /// `Some` for Within / Exceeds; `None` for Unmodeled. Mirrors the
    /// typed verdicts' helper so iteration code can stay generic.
    pub fn confidence(&self) -> Option<&Confidence> {
        match self {
            Verdict::Within { confidence, .. } | Verdict::Exceeds { confidence, .. } => {
                Some(confidence)
            }
            Verdict::Unmodeled { .. } => None,
        }
    }
}

/// Per-toolpath outcome across all criteria.
///
/// `toolpath_id` is the core `usize` index into the project's enabled
/// toolpath list (matches `SimulationCutSample::toolpath_id` semantics).
///
/// All three gates have migrated to typed verdicts (G16 Step 7b–7d).
/// The legacy flat `Verdict` and `ExceedsReason` enums remain in this
/// module only for the export-gate label tuple in `exceeded_toolpaths`;
/// Step 7e migrates that to `ExceededCriterion`, then 7f deletes both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolpathLoadVerdict {
    pub toolpath_id: usize,
    pub chipload: ChiploadVerdict,
    pub power: PowerVerdict,
    pub deflection: DeflectionVerdict,
}

impl ToolpathLoadVerdict {
    /// Count criteria with a non-`Unmodeled` verdict (i.e. actually evaluated).
    pub fn modeled_count(&self) -> usize {
        let mut n = 0;
        if !self.chipload.is_unmodeled() {
            n += 1;
        }
        if !self.power.is_unmodeled() {
            n += 1;
        }
        if !self.deflection.is_unmodeled() {
            n += 1;
        }
        n
    }

    /// True if any criterion is `Exceeds`.
    pub fn any_exceeded(&self) -> bool {
        self.chipload.is_exceeded() || self.power.is_exceeded() || self.deflection.is_exceeded()
    }

    /// True if any criterion is `Unmodeled`.
    pub fn any_unmodeled(&self) -> bool {
        self.chipload.is_unmodeled() || self.power.is_unmodeled() || self.deflection.is_unmodeled()
    }
}

/// Project-level report: one verdict per toolpath.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLoadReport {
    pub per_toolpath: Vec<ToolpathLoadVerdict>,
}

impl ToolLoadReport {
    pub fn any_exceeded(&self) -> bool {
        self.per_toolpath
            .iter()
            .any(ToolpathLoadVerdict::any_exceeded)
    }

    pub fn any_unmodeled(&self) -> bool {
        self.per_toolpath
            .iter()
            .any(ToolpathLoadVerdict::any_unmodeled)
    }

    /// All toolpath indices that have at least one `Exceeds` verdict, with
    /// the per-criterion reasons. Used by the export gate to produce the
    /// blocking error message.
    pub fn exceeded_toolpaths(&self) -> Vec<(usize, Vec<(&'static str, ExceedsReason)>)> {
        self.per_toolpath
            .iter()
            .filter_map(|v| {
                let mut reasons: Vec<(&'static str, ExceedsReason)> = Vec::new();
                if let ChiploadVerdict::Exceeds { side, .. } = &v.chipload {
                    let r = match side {
                        ChipSide::Low => ExceedsReason::ChiploadBurnRisk,
                        ChipSide::High => ExceedsReason::ChiploadBreakageRisk,
                    };
                    reasons.push(("chipload", r));
                }
                if v.power.is_exceeded() {
                    reasons.push(("power", ExceedsReason::SpindlePowerExceeded));
                }
                if v.deflection.is_exceeded() {
                    reasons.push(("deflection", ExceedsReason::LongToolStiffnessUnsafe));
                }
                if reasons.is_empty() {
                    None
                } else {
                    Some((v.toolpath_id, reasons))
                }
            })
            .collect()
    }
}

// ─────────────────────────────────────────────────────────────────────
// Typed verdict scaffolding (G16 Step 7a). Lives alongside the legacy
// flat `Verdict` until the per-gate evaluators migrate. No consumers
// read these yet.
// ─────────────────────────────────────────────────────────────────────

/// Coarse outcome shared across all gate verdicts. Lets UI / export /
/// timeline iterate without knowing each typed verdict's internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadState {
    Within,
    Exceeds,
    Unmodeled,
}

/// Identifies which gate produced a verdict. Used by the helper layer
/// to label criteria for UI, export, and the MCP wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionKind {
    Chipload,
    Power,
    Deflection,
}

impl CriterionKind {
    pub fn label(self) -> &'static str {
        match self {
            CriterionKind::Chipload => "chipload",
            CriterionKind::Power => "power",
            CriterionKind::Deflection => "deflection",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            CriterionKind::Chipload => "mm/tooth",
            CriterionKind::Power => "kW",
            CriterionKind::Deflection => "mm",
        }
    }
}

/// Generic per-criterion summary used by UI / export / timeline. A
/// typed verdict produces one of these via `as_criterion_status`,
/// hiding its internals from consumers that just want
/// "what kind, what state, what peak, what range".
#[derive(Debug, Clone)]
pub struct CriterionStatus<'a> {
    pub kind: CriterionKind,
    pub state: LoadState,
    pub confidence: Option<&'a Confidence>,
    pub sample_range: Option<Range<usize>>,
    pub display_peak: Option<f64>,
    pub unit: &'static str,
}

/// Sample-range evidence behind a peak metric. Empty range (`0..0`)
/// means the criterion has no per-sample resolution (or no specific
/// triggering sample is recorded). `statistic` is `Some` only on the
/// chipload gate, which uses different statistics for the burn vs.
/// breakage sides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleEvidence {
    pub sample_range: Range<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statistic: Option<ChiploadStatistic>,
}

impl SampleEvidence {
    /// No specific sample recorded: range `0..0`, no statistic.
    pub fn empty() -> Self {
        Self {
            sample_range: 0..0,
            statistic: None,
        }
    }

    /// Single-sample range at `idx`, no statistic descriptor.
    pub fn at(idx: usize) -> Self {
        Self {
            sample_range: idx..(idx + 1),
            statistic: None,
        }
    }

    /// Single-sample range at `idx` annotated with a chipload statistic.
    pub fn at_with_stat(idx: usize, statistic: ChiploadStatistic) -> Self {
        Self {
            sample_range: idx..(idx + 1),
            statistic: Some(statistic),
        }
    }
}

/// Which statistic produced a reported chipload value. Burn-risk uses
/// the *median* of in-cut chip thicknesses (robust to transient low-arc
/// samples); breakage uses the per-sample peak. `PeakInRange` is the
/// within-bounds reporting case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChiploadStatistic {
    /// Median of in-cut sample chip thicknesses, vs LUT min.
    MedianLow,
    /// Per-sample peak above LUT max.
    PeakHigh,
    /// Per-sample peak inside the LUT envelope (Within reporting).
    PeakInRange,
}

/// Where the chipload bounds came from. Distinct from
/// `optimize::bounds::BoundsSource` (axis-bound source).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChipBoundsSource {
    /// Direct vendor LUT row match for the (tool, material, op) tuple.
    VendorLut,
    /// Vendor LUT row extrapolated to the query's diameter / hardness.
    /// The diagnostic detail (scale factors, calibrated row id) lives
    /// on the verdict's `Confidence::Approximate` payload.
    VendorLutExtrapolated,
}

/// Vendor LUT chip-load bounds. `min_mm_per_tooth` is `Option` because
/// some LUT rows ship only an upper bound — the chipload evaluator can
/// still flag breakage from `PeakHigh` while burn-risk falls back to
/// `Unmodeled` for that row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChipBounds {
    pub min_mm_per_tooth: Option<f64>,
    pub max_mm_per_tooth: f64,
    pub source: ChipBoundsSource,
}

/// Which side of the LUT envelope a chipload exceedance landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChipSide {
    /// Below LUT min — burn / rubbing risk. Statistic is `MedianLow`.
    Low,
    /// Above LUT max — breakage risk. Statistic is `PeakHigh`.
    High,
}

/// One per-side chipload reading: the observed value, the statistic
/// that produced it, the supporting sample-range evidence, and the
/// LUT bounds it was compared against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChiploadMetric {
    pub observed_mm_per_tooth: f64,
    pub statistic: ChiploadStatistic,
    pub evidence: SampleEvidence,
    pub bounds: ChipBounds,
}

/// Typed chipload verdict. Replaces the flat `Verdict` once the
/// chipload evaluator migrates (Step 7d). Carries both bounds-approach
/// metrics on the within case and the triggering metric on exceed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ChiploadVerdict {
    Within {
        /// Distance-to-min metric. `None` when the matched row has no
        /// `chip_load_min_mm` — burn-risk arm is `Unmodeled` for that
        /// row.
        approach_to_min: Option<ChiploadMetric>,
        /// Distance-to-max metric. Always present (the evaluator
        /// rejects rows with no max).
        approach_to_max: ChiploadMetric,
        confidence: Confidence,
    },
    Exceeds {
        side: ChipSide,
        triggering: ChiploadMetric,
        confidence: Confidence,
    },
    Unmodeled {
        reason: UnmodeledReason,
    },
}

impl ChiploadVerdict {
    pub fn state(&self) -> LoadState {
        match self {
            ChiploadVerdict::Within { .. } => LoadState::Within,
            ChiploadVerdict::Exceeds { .. } => LoadState::Exceeds,
            ChiploadVerdict::Unmodeled { .. } => LoadState::Unmodeled,
        }
    }

    pub fn is_exceeded(&self) -> bool {
        matches!(self, ChiploadVerdict::Exceeds { .. })
    }

    pub fn is_unmodeled(&self) -> bool {
        matches!(self, ChiploadVerdict::Unmodeled { .. })
    }

    pub fn confidence(&self) -> Option<&Confidence> {
        match self {
            ChiploadVerdict::Within { confidence, .. }
            | ChiploadVerdict::Exceeds { confidence, .. } => Some(confidence),
            ChiploadVerdict::Unmodeled { .. } => None,
        }
    }

    pub fn as_criterion_status(&self) -> CriterionStatus<'_> {
        let (state, peak, range) = match self {
            ChiploadVerdict::Within {
                approach_to_max, ..
            } => (
                LoadState::Within,
                Some(approach_to_max.observed_mm_per_tooth),
                option_range(&approach_to_max.evidence.sample_range),
            ),
            ChiploadVerdict::Exceeds { triggering, .. } => (
                LoadState::Exceeds,
                Some(triggering.observed_mm_per_tooth),
                option_range(&triggering.evidence.sample_range),
            ),
            ChiploadVerdict::Unmodeled { .. } => (LoadState::Unmodeled, None, None),
        };
        CriterionStatus {
            kind: CriterionKind::Chipload,
            state,
            confidence: self.confidence(),
            sample_range: range,
            display_peak: peak,
            unit: CriterionKind::Chipload.unit(),
        }
    }
}

/// Typed power verdict. Both `Within` and `Exceeds` carry
/// `available_kw` so UI / MCP can render the headroom band.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PowerVerdict {
    Within {
        peak_kw: f64,
        available_kw: f64,
        evidence: SampleEvidence,
        confidence: Confidence,
    },
    Exceeds {
        peak_kw: f64,
        available_kw: f64,
        evidence: SampleEvidence,
        confidence: Confidence,
    },
    Unmodeled {
        reason: UnmodeledReason,
    },
}

impl PowerVerdict {
    pub fn state(&self) -> LoadState {
        match self {
            PowerVerdict::Within { .. } => LoadState::Within,
            PowerVerdict::Exceeds { .. } => LoadState::Exceeds,
            PowerVerdict::Unmodeled { .. } => LoadState::Unmodeled,
        }
    }

    pub fn is_exceeded(&self) -> bool {
        matches!(self, PowerVerdict::Exceeds { .. })
    }

    pub fn is_unmodeled(&self) -> bool {
        matches!(self, PowerVerdict::Unmodeled { .. })
    }

    pub fn confidence(&self) -> Option<&Confidence> {
        match self {
            PowerVerdict::Within { confidence, .. }
            | PowerVerdict::Exceeds { confidence, .. } => Some(confidence),
            PowerVerdict::Unmodeled { .. } => None,
        }
    }

    pub fn as_criterion_status(&self) -> CriterionStatus<'_> {
        let (state, peak, range) = match self {
            PowerVerdict::Within {
                peak_kw, evidence, ..
            } => (
                LoadState::Within,
                Some(*peak_kw),
                option_range(&evidence.sample_range),
            ),
            PowerVerdict::Exceeds {
                peak_kw, evidence, ..
            } => (
                LoadState::Exceeds,
                Some(*peak_kw),
                option_range(&evidence.sample_range),
            ),
            PowerVerdict::Unmodeled { .. } => (LoadState::Unmodeled, None, None),
        };
        CriterionStatus {
            kind: CriterionKind::Power,
            state,
            confidence: self.confidence(),
            sample_range: range,
            display_peak: peak,
            unit: CriterionKind::Power.unit(),
        }
    }
}

/// Both deflection thresholds visible on the verdict so UI can render
/// the validated-within / approximate-within / exceeds bands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeflectionBounds {
    /// Below this peak tip deflection (mm), `Within(Validated)`.
    /// 0.050 mm in `tool_load::deflection`.
    pub validated_within_mm: f64,
    /// Above this peak tip deflection (mm), `Exceeds`. 0.200 mm.
    /// Between the two thresholds, `Within(Approximate)` with
    /// finish-degradation warning.
    pub exceeds_mm: f64,
}

/// Typed deflection verdict. Both `Within` and `Exceeds` carry the
/// bounds so consumers can render the threshold band without hardcoding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DeflectionVerdict {
    Within {
        peak_mm: f64,
        bounds: DeflectionBounds,
        evidence: SampleEvidence,
        confidence: Confidence,
    },
    Exceeds {
        peak_mm: f64,
        bounds: DeflectionBounds,
        evidence: SampleEvidence,
        confidence: Confidence,
    },
    Unmodeled {
        reason: UnmodeledReason,
    },
}

impl DeflectionVerdict {
    pub fn state(&self) -> LoadState {
        match self {
            DeflectionVerdict::Within { .. } => LoadState::Within,
            DeflectionVerdict::Exceeds { .. } => LoadState::Exceeds,
            DeflectionVerdict::Unmodeled { .. } => LoadState::Unmodeled,
        }
    }

    pub fn is_exceeded(&self) -> bool {
        matches!(self, DeflectionVerdict::Exceeds { .. })
    }

    pub fn is_unmodeled(&self) -> bool {
        matches!(self, DeflectionVerdict::Unmodeled { .. })
    }

    pub fn confidence(&self) -> Option<&Confidence> {
        match self {
            DeflectionVerdict::Within { confidence, .. }
            | DeflectionVerdict::Exceeds { confidence, .. } => Some(confidence),
            DeflectionVerdict::Unmodeled { .. } => None,
        }
    }

    pub fn as_criterion_status(&self) -> CriterionStatus<'_> {
        let (state, peak, range) = match self {
            DeflectionVerdict::Within {
                peak_mm, evidence, ..
            } => (
                LoadState::Within,
                Some(*peak_mm),
                option_range(&evidence.sample_range),
            ),
            DeflectionVerdict::Exceeds {
                peak_mm, evidence, ..
            } => (
                LoadState::Exceeds,
                Some(*peak_mm),
                option_range(&evidence.sample_range),
            ),
            DeflectionVerdict::Unmodeled { .. } => (LoadState::Unmodeled, None, None),
        };
        CriterionStatus {
            kind: CriterionKind::Deflection,
            state,
            confidence: self.confidence(),
            sample_range: range,
            display_peak: peak,
            unit: CriterionKind::Deflection.unit(),
        }
    }
}

/// Per-criterion exceedance label suitable for the export-gate error
/// message and MCP wire output. Replaces the
/// `(&'static str, ExceedsReason)` pair returned by the legacy
/// `ToolLoadReport::exceeded_toolpaths`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExceededCriterion {
    pub kind: CriterionKind,
    pub label: &'static str,
    pub reason_label: &'static str,
}

impl ExceededCriterion {
    pub fn chipload_burn() -> Self {
        Self {
            kind: CriterionKind::Chipload,
            label: "chipload",
            reason_label: "burn risk",
        }
    }

    pub fn chipload_breakage() -> Self {
        Self {
            kind: CriterionKind::Chipload,
            label: "chipload",
            reason_label: "breakage",
        }
    }

    pub fn power() -> Self {
        Self {
            kind: CriterionKind::Power,
            label: "power",
            reason_label: "spindle power",
        }
    }

    pub fn deflection() -> Self {
        Self {
            kind: CriterionKind::Deflection,
            label: "deflection",
            reason_label: "stiffness",
        }
    }
}

fn option_range(range: &Range<usize>) -> Option<Range<usize>> {
    if range.is_empty() {
        None
    } else {
        Some(range.clone())
    }
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

    #[test]
    fn modeled_count_ignores_unmodeled() {
        let v = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: ChiploadVerdict::Within {
                approach_to_min: None,
                approach_to_max: ChiploadMetric {
                    observed_mm_per_tooth: 0.05,
                    statistic: ChiploadStatistic::PeakInRange,
                    evidence: SampleEvidence::empty(),
                    bounds: ChipBounds {
                        min_mm_per_tooth: Some(0.038),
                        max_mm_per_tooth: 0.07,
                        source: ChipBoundsSource::VendorLut,
                    },
                },
                confidence: Confidence::Validated,
            },
            power: PowerVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            deflection: DeflectionVerdict::Within {
                peak_mm: 3.5,
                bounds: DeflectionBounds {
                    validated_within_mm: 0.050,
                    exceeds_mm: 0.200,
                },
                evidence: SampleEvidence::empty(),
                confidence: Confidence::Validated,
            },
        };
        assert_eq!(v.modeled_count(), 2);
        assert!(!v.any_exceeded());
        assert!(v.any_unmodeled());
    }

    /// Regression: `Confidence::Approximate(String)` and
    /// `UnmodeledReason::CutterModeUnsupported(String)` are newtype variants
    /// inside internally-tagged enums; without `content = "detail"` serde
    /// fails at runtime and the MCP layer silently returned `null`.
    #[test]
    fn report_serializes_with_string_carrying_variants() {
        let r = ToolLoadReport {
            per_toolpath: vec![ToolpathLoadVerdict {
                toolpath_id: 0,
                chipload: ChiploadVerdict::Within {
                    approach_to_min: None,
                    approach_to_max: ChiploadMetric {
                        observed_mm_per_tooth: 0.05,
                        statistic: ChiploadStatistic::PeakInRange,
                        evidence: SampleEvidence::empty(),
                        bounds: ChipBounds {
                            min_mm_per_tooth: Some(0.038),
                            max_mm_per_tooth: 0.07,
                            source: ChipBoundsSource::VendorLut,
                        },
                    },
                    confidence: Confidence::Approximate("isotropic Kc only".to_owned()),
                },
                power: PowerVerdict::Unmodeled {
                    reason: UnmodeledReason::CutterModeUnsupported("v-bit tip".to_owned()),
                },
                deflection: DeflectionVerdict::Within {
                    peak_mm: 4.5,
                    bounds: DeflectionBounds {
                        validated_within_mm: 0.050,
                        exceeds_mm: 0.200,
                    },
                    evidence: SampleEvidence::empty(),
                    confidence: Confidence::Approximate("L/D in 4-6 range".to_owned()),
                },
            }],
        };
        let v = serde_json::to_value(&r).expect("must round-trip");
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("isotropic Kc only"), "lost detail string: {s}");
        assert!(s.contains("v-bit tip"), "lost detail string: {s}");
        assert!(s.contains("L/D in 4-6 range"), "lost detail string: {s}");
    }

    #[test]
    fn report_collects_exceeded_reasons() {
        let r = ToolLoadReport {
            per_toolpath: vec![
                ToolpathLoadVerdict {
                    toolpath_id: 0,
                    chipload: ChiploadVerdict::Within {
                        approach_to_min: None,
                        approach_to_max: ChiploadMetric {
                            observed_mm_per_tooth: 0.05,
                            statistic: ChiploadStatistic::PeakInRange,
                            evidence: SampleEvidence::empty(),
                            bounds: ChipBounds {
                                min_mm_per_tooth: Some(0.038),
                                max_mm_per_tooth: 0.07,
                                source: ChipBoundsSource::VendorLut,
                            },
                        },
                        confidence: Confidence::Validated,
                    },
                    power: PowerVerdict::Unmodeled {
                        reason: UnmodeledReason::NotImplemented("phase 1b".to_owned()),
                    },
                    deflection: DeflectionVerdict::Exceeds {
                        peak_mm: 8.5,
                        bounds: DeflectionBounds {
                            validated_within_mm: 0.050,
                            exceeds_mm: 0.200,
                        },
                        evidence: SampleEvidence::empty(),
                        confidence: Confidence::Validated,
                    },
                },
                ToolpathLoadVerdict {
                    toolpath_id: 1,
                    chipload: ChiploadVerdict::Within {
                        approach_to_min: None,
                        approach_to_max: ChiploadMetric {
                            observed_mm_per_tooth: 0.04,
                            statistic: ChiploadStatistic::PeakInRange,
                            evidence: SampleEvidence::empty(),
                            bounds: ChipBounds {
                                min_mm_per_tooth: Some(0.038),
                                max_mm_per_tooth: 0.07,
                                source: ChipBoundsSource::VendorLut,
                            },
                        },
                        confidence: Confidence::Validated,
                    },
                    power: PowerVerdict::Unmodeled {
                        reason: UnmodeledReason::NotImplemented("phase 1b".to_owned()),
                    },
                    deflection: DeflectionVerdict::Within {
                        peak_mm: 2.5,
                        bounds: DeflectionBounds {
                            validated_within_mm: 0.050,
                            exceeds_mm: 0.200,
                        },
                        evidence: SampleEvidence::empty(),
                        confidence: Confidence::Validated,
                    },
                },
            ],
        };
        assert!(r.any_exceeded());
        let exceeded = r.exceeded_toolpaths();
        assert_eq!(exceeded.len(), 1);
        assert_eq!(exceeded[0].0, 0);
        assert_eq!(exceeded[0].1.len(), 1);
        assert_eq!(exceeded[0].1[0].0, "deflection");
    }

    // ── Typed verdict scaffolding (Step 7a) ─────────────────────────

    fn chip_bounds_with_min() -> ChipBounds {
        ChipBounds {
            min_mm_per_tooth: Some(0.038),
            max_mm_per_tooth: 0.07,
            source: ChipBoundsSource::VendorLut,
        }
    }

    fn chip_bounds_no_min() -> ChipBounds {
        ChipBounds {
            min_mm_per_tooth: None,
            max_mm_per_tooth: 0.07,
            source: ChipBoundsSource::VendorLut,
        }
    }

    fn deflection_bounds() -> DeflectionBounds {
        DeflectionBounds {
            validated_within_mm: 0.050,
            exceeds_mm: 0.200,
        }
    }

    #[test]
    fn chipload_verdict_state_maps_to_load_state() {
        let within = ChiploadVerdict::Within {
            approach_to_min: None,
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: 0.06,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::at(7),
                bounds: chip_bounds_with_min(),
            },
            confidence: Confidence::Validated,
        };
        assert_eq!(within.state(), LoadState::Within);
        assert!(!within.is_exceeded());
        assert!(!within.is_unmodeled());

        let exceeds = ChiploadVerdict::Exceeds {
            side: ChipSide::Low,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: 0.012,
                statistic: ChiploadStatistic::MedianLow,
                evidence: SampleEvidence::at_with_stat(3, ChiploadStatistic::MedianLow),
                bounds: chip_bounds_with_min(),
            },
            confidence: Confidence::Validated,
        };
        assert_eq!(exceeds.state(), LoadState::Exceeds);
        assert!(exceeds.is_exceeded());

        let unmodeled = ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
        assert_eq!(unmodeled.state(), LoadState::Unmodeled);
        assert!(unmodeled.is_unmodeled());
    }

    #[test]
    fn chipload_within_with_no_min_round_trips() {
        // Anchors the "LUT row has only `chip_load_max_mm`" case the
        // chipload evaluator carries through to Step 7d.
        let v = ChiploadVerdict::Within {
            approach_to_min: None,
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: 0.06,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::empty(),
                bounds: chip_bounds_no_min(),
            },
            confidence: Confidence::Validated,
        };
        let json = serde_json::to_string(&v).expect("ser");
        let back: ChiploadVerdict = serde_json::from_str(&json).expect("de");
        assert_eq!(back, v);
    }

    #[test]
    fn chipload_exceeds_low_carries_median_statistic_in_status() {
        // Pins the design corollary that BurnRisk (Low side) is driven
        // by MedianLow today, not PeakLow. Step 7d's evaluator must
        // populate `triggering.statistic` accordingly.
        let v = ChiploadVerdict::Exceeds {
            side: ChipSide::Low,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: 0.012,
                statistic: ChiploadStatistic::MedianLow,
                evidence: SampleEvidence::at_with_stat(11, ChiploadStatistic::MedianLow),
                bounds: chip_bounds_with_min(),
            },
            confidence: Confidence::Validated,
        };
        let status = v.as_criterion_status();
        assert_eq!(status.kind, CriterionKind::Chipload);
        assert_eq!(status.state, LoadState::Exceeds);
        assert_eq!(status.unit, "mm/tooth");
        assert_eq!(status.display_peak, Some(0.012));
        assert_eq!(status.sample_range, Some(11..12));
    }

    #[test]
    fn power_verdict_within_carries_available_kw() {
        let v = PowerVerdict::Within {
            peak_kw: 0.4,
            available_kw: 0.71,
            evidence: SampleEvidence::at(2),
            confidence: Confidence::Approximate("isotropic Kc".to_owned()),
        };
        match &v {
            PowerVerdict::Within { available_kw, .. } => {
                assert!((*available_kw - 0.71).abs() < 1e-9);
            }
            other => panic!("expected Within, got {other:?}"),
        }
        let status = v.as_criterion_status();
        assert_eq!(status.kind, CriterionKind::Power);
        assert_eq!(status.state, LoadState::Within);
        assert_eq!(status.unit, "kW");
        assert_eq!(status.display_peak, Some(0.4));
        assert_eq!(status.sample_range, Some(2..3));
    }

    #[test]
    fn power_exceeds_carries_available_kw_and_round_trips() {
        let v = PowerVerdict::Exceeds {
            peak_kw: 1.5,
            available_kw: 0.5,
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        };
        let json = serde_json::to_string(&v).expect("ser");
        let back: PowerVerdict = serde_json::from_str(&json).expect("de");
        assert_eq!(back, v);
        match back {
            PowerVerdict::Exceeds { available_kw, .. } => {
                assert!((available_kw - 0.5).abs() < 1e-9);
            }
            other => panic!("expected Exceeds, got {other:?}"),
        }
    }

    #[test]
    fn deflection_bounds_round_trip_keeps_both_thresholds() {
        let v = DeflectionVerdict::Within {
            peak_mm: 0.080,
            bounds: deflection_bounds(),
            evidence: SampleEvidence::at(5),
            confidence: Confidence::Approximate("approximate band".to_owned()),
        };
        let json = serde_json::to_string(&v).expect("ser");
        let back: DeflectionVerdict = serde_json::from_str(&json).expect("de");
        assert_eq!(back, v);
        match back {
            DeflectionVerdict::Within { bounds, .. } => {
                assert!((bounds.validated_within_mm - 0.050).abs() < 1e-9);
                assert!((bounds.exceeds_mm - 0.200).abs() < 1e-9);
            }
            other => panic!("expected Within, got {other:?}"),
        }
    }

    #[test]
    fn empty_sample_range_yields_none_in_status() {
        let v = PowerVerdict::Within {
            peak_kw: 0.2,
            available_kw: 0.71,
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
        };
        let s = v.as_criterion_status();
        assert!(s.sample_range.is_none());
    }

    #[test]
    fn unmodeled_status_has_no_peak_no_range_no_confidence() {
        let v = DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
        let s = v.as_criterion_status();
        assert_eq!(s.kind, CriterionKind::Deflection);
        assert_eq!(s.state, LoadState::Unmodeled);
        assert!(s.display_peak.is_none());
        assert!(s.sample_range.is_none());
        assert!(s.confidence.is_none());
    }

    #[test]
    fn criterion_kind_label_and_unit() {
        assert_eq!(CriterionKind::Chipload.label(), "chipload");
        assert_eq!(CriterionKind::Power.label(), "power");
        assert_eq!(CriterionKind::Deflection.label(), "deflection");
        assert_eq!(CriterionKind::Chipload.unit(), "mm/tooth");
        assert_eq!(CriterionKind::Power.unit(), "kW");
        assert_eq!(CriterionKind::Deflection.unit(), "mm");
    }

    #[test]
    fn exceeded_criterion_constructors_label_and_reason() {
        assert_eq!(
            ExceededCriterion::chipload_burn().kind,
            CriterionKind::Chipload
        );
        assert_eq!(ExceededCriterion::chipload_burn().reason_label, "burn risk");
        assert_eq!(
            ExceededCriterion::chipload_breakage().reason_label,
            "breakage"
        );
        assert_eq!(ExceededCriterion::power().reason_label, "spindle power");
        assert_eq!(ExceededCriterion::deflection().reason_label, "stiffness");
    }
}
