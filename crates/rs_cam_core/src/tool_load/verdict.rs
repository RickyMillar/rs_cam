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
}

/// Per-toolpath outcome across all criteria.
///
/// `toolpath_id` is the core `usize` index into the project's enabled
/// toolpath list (matches `SimulationCutSample::toolpath_id` semantics).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolpathLoadVerdict {
    pub toolpath_id: usize,
    pub chipload: Verdict,
    pub power: Verdict,
    pub deflection: Verdict,
}

impl ToolpathLoadVerdict {
    /// Count criteria with a non-`Unmodeled` verdict (i.e. actually evaluated).
    pub fn modeled_count(&self) -> usize {
        [&self.chipload, &self.power, &self.deflection]
            .iter()
            .filter(|v| !v.is_unmodeled())
            .count()
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
                if let Verdict::Exceeds { reason, .. } = &v.chipload {
                    reasons.push(("chipload", reason.clone()));
                }
                if let Verdict::Exceeds { reason, .. } = &v.power {
                    reasons.push(("power", reason.clone()));
                }
                if let Verdict::Exceeds { reason, .. } = &v.deflection {
                    reasons.push(("deflection", reason.clone()));
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
            chipload: Verdict::Within {
                peak: 0.05,
                confidence: Confidence::Validated,
            },
            power: Verdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            deflection: Verdict::Within {
                peak: 3.5,
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
                chipload: Verdict::Within {
                    peak: 0.05,
                    confidence: Confidence::Approximate("isotropic Kc only".to_owned()),
                },
                power: Verdict::Unmodeled {
                    reason: UnmodeledReason::CutterModeUnsupported("v-bit tip".to_owned()),
                },
                deflection: Verdict::Within {
                    peak: 4.5,
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
                    chipload: Verdict::Within {
                        peak: 0.05,
                        confidence: Confidence::Validated,
                    },
                    power: Verdict::not_implemented("phase 1b"),
                    deflection: Verdict::Exceeds {
                        peak: 8.5,
                        sample_range: 0..0,
                        reason: ExceedsReason::LongToolStiffnessUnsafe,
                        confidence: Confidence::Validated,
                    },
                },
                ToolpathLoadVerdict {
                    toolpath_id: 1,
                    chipload: Verdict::Within {
                        peak: 0.04,
                        confidence: Confidence::Validated,
                    },
                    power: Verdict::not_implemented("phase 1b"),
                    deflection: Verdict::Within {
                        peak: 2.5,
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
}
