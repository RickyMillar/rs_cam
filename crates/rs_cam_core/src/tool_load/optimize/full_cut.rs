//! The full-cut check (PLAN.md §3 of
//! `planning/optimize_review_2026-10-12/PLAN.md`).
//!
//! A faster candidate is of no use when it cuts less material, drives a
//! rapid through the stock, or takes a deeper bite through material an
//! upstream operation left standing. Each candidate therefore carries
//! three readings from its own simulation, and the check compares them
//! with the same readings of the baseline:
//!
//! - the removed volume of this toolpath
//!   (`SimulationToolpathCutSummary::total_removed_volume_est_mm3`);
//! - the rapid collisions that the simulation attributes to this
//!   toolpath (`ProjectEvidence::attributed_rapid_collisions`);
//! - the peak of the "crosses material" finding
//!   (`diagnostics::ids::PROJECT_CROSSES_STANDING_MATERIAL`, scope
//!   `Toolpath`, evidence `observed`).
//!
//! The baseline is the reference. It carries its readings with the
//! verdict [`FullCutVerdict::Holds`].

use serde::{Deserialize, Serialize};

use crate::ids::ToolpathId;
use crate::session::{ProjectEvidence, ProjectSession};
use crate::stock::simulation_cut::SimulationCutTrace;

/// The result of the full-cut check on one candidate.
///
/// Each reading is three-valued. `None` means NOT MEASURED. A measured
/// "no finding" is `Some(0.0)` for the over-cut peak and `Some(0)` for
/// the collision count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullCutCheck {
    /// The volume this toolpath removed, in mm³. `None` when the trace
    /// holds no summary for the toolpath.
    pub removed_volume_mm3: Option<f64>,
    /// The rapid collisions the simulation attributes to this toolpath.
    /// `None` when no simulation result was available.
    pub rapid_collisions: Option<usize>,
    /// The peak bite of the "crosses material" finding, in mm. `Some(0.0)`
    /// when the triage holds no such finding for this toolpath. `None`
    /// when no simulation result was available.
    pub overcut_peak_mm: Option<f64>,
    /// The verdict of the check.
    pub verdict: FullCutVerdict,
}

impl Default for FullCutCheck {
    /// A record with no readings. A record that was written before this
    /// field existed reads as this value.
    fn default() -> Self {
        Self {
            removed_volume_mm3: None,
            rapid_collisions: None,
            overcut_peak_mm: None,
            verdict: FullCutVerdict::NotMeasured {
                reason: "the record holds no full-cut readings".to_owned(),
            },
        }
    }
}

/// The verdict of the full-cut check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum FullCutVerdict {
    /// The candidate cuts the same part as the baseline. The baseline
    /// always holds: it is the reference.
    Holds,
    /// A reading of the candidate is worse than the baseline. `reason`
    /// names the reading and the two values.
    Fails { reason: String },
    /// A reading that the check needs is not available, on the candidate
    /// or on the baseline. The candidate cannot pass.
    NotMeasured { reason: String },
}

impl FullCutVerdict {
    /// True when the verdict is [`FullCutVerdict::Holds`].
    #[must_use]
    pub fn holds(&self) -> bool {
        matches!(self, FullCutVerdict::Holds)
    }

    /// The reason text of a `Fails` or a `NotMeasured` verdict. `None`
    /// for `Holds`.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            FullCutVerdict::Holds => None,
            FullCutVerdict::Fails { reason } | FullCutVerdict::NotMeasured { reason } => {
                Some(reason)
            }
        }
    }
}

/// The three readings of one simulation, for one toolpath.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FullCutReadings {
    /// See [`FullCutCheck::removed_volume_mm3`].
    pub removed_volume_mm3: Option<f64>,
    /// See [`FullCutCheck::rapid_collisions`].
    pub rapid_collisions: Option<usize>,
    /// See [`FullCutCheck::overcut_peak_mm`].
    pub overcut_peak_mm: Option<f64>,
}

impl FullCutCheck {
    /// The baseline record: its readings, with the verdict `Holds`.
    #[must_use]
    pub fn reference(readings: FullCutReadings) -> Self {
        Self {
            removed_volume_mm3: readings.removed_volume_mm3,
            rapid_collisions: readings.rapid_collisions,
            overcut_peak_mm: readings.overcut_peak_mm,
            verdict: FullCutVerdict::Holds,
        }
    }

    /// Judge the readings of a candidate against the baseline.
    ///
    /// The candidate fails when any of these is true:
    ///
    /// 1. **Volume.** `|V_candidate - V_baseline| > V_baseline × r / T`,
    ///    with `r` the dexel cell (`resolution_mm`) and `T` the total
    ///    depth of the operation (`total_depth_mm`). Derivation: a 2.5D
    ///    operation removes its footprint `A` over the depth `T`, so
    ///    `V = A × T`. One dexel layer of the floor is `A × r =
    ///    V × r / T`. Every realised step `T / n` puts the last level on
    ///    `T`, so only the cell quantisation of the floor can move the
    ///    volume, and one floor layer is that quantisation. An operation
    ///    with no total depth (`total_depth_mm = None`) skips this rule.
    /// 2. **Collisions.** The candidate has more rapid collisions than
    ///    the baseline.
    /// 3. **Over-cut.** The candidate's "crosses material" peak is more
    ///    than the baseline's peak plus one cell (`r`). A baseline with
    ///    no finding compares as 0.
    ///
    /// A measured failure gives `Fails`. Else, a reading that a rule needs
    /// and that is `None` on either side gives `NotMeasured`. Else the
    /// verdict is `Holds`.
    #[must_use]
    pub fn judge(
        baseline: &FullCutCheck,
        readings: FullCutReadings,
        resolution_mm: f64,
        total_depth_mm: Option<f64>,
    ) -> Self {
        let mut fails: Vec<String> = Vec::new();
        let mut missing: Vec<String> = Vec::new();

        if let Some(total) = total_depth_mm.filter(|t| t.is_finite() && *t > 0.0) {
            match (baseline.removed_volume_mm3, readings.removed_volume_mm3) {
                (Some(base), Some(cand)) => {
                    let tolerance = base.abs() * resolution_mm / total;
                    let delta = cand - base;
                    if delta.abs() > tolerance {
                        fails.push(format!(
                            "the removed volume is {cand:.0} mm³ against {base:.0} mm³ on the \
                             baseline ({delta:+.0} mm³; one floor layer is {tolerance:.0} mm³)"
                        ));
                    }
                }
                (None, _) => missing.push("the removed volume of the baseline".to_owned()),
                (_, None) => missing.push("the removed volume of the candidate".to_owned()),
            }
        }

        match (baseline.rapid_collisions, readings.rapid_collisions) {
            (Some(base), Some(cand)) => {
                if cand > base {
                    fails.push(format!(
                        "the rapids hit the stock {cand} times against {base} on the baseline"
                    ));
                }
            }
            (None, _) => missing.push("the rapid collisions of the baseline".to_owned()),
            (_, None) => missing.push("the rapid collisions of the candidate".to_owned()),
        }

        match (baseline.overcut_peak_mm, readings.overcut_peak_mm) {
            (Some(base), Some(cand)) => {
                if cand > base + resolution_mm {
                    fails.push(format!(
                        "the cut crosses standing material with a peak bite of {cand:.2} mm \
                         against {base:.2} mm on the baseline (one cell is {resolution_mm:.2} mm)"
                    ));
                }
            }
            (None, _) => missing.push("the over-cut finding of the baseline".to_owned()),
            (_, None) => missing.push("the over-cut finding of the candidate".to_owned()),
        }

        let verdict = if !fails.is_empty() {
            FullCutVerdict::Fails {
                reason: fails.join("; "),
            }
        } else if !missing.is_empty() {
            FullCutVerdict::NotMeasured {
                reason: format!("not measured: {}", missing.join(", ")),
            }
        } else {
            FullCutVerdict::Holds
        };
        Self {
            removed_volume_mm3: readings.removed_volume_mm3,
            rapid_collisions: readings.rapid_collisions,
            overcut_peak_mm: readings.overcut_peak_mm,
            verdict,
        }
    }
}

/// The removed volume of one toolpath from a trace. `None` when the
/// trace holds no summary for the toolpath.
#[must_use]
pub(crate) fn removed_volume_from_trace(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
) -> Option<f64> {
    trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == toolpath_id)
        .map(|s| s.total_removed_volume_est_mm3)
}

/// Read the three full-cut readings of one toolpath.
///
/// `trace` gives the volume. The session's simulation result gives the
/// collisions and, through `ProjectSession::simulation_triage`, the
/// over-cut finding; the triage reads `trace`. With no simulation result
/// on the session, the collisions and the over-cut are `None` (NOT
/// MEASURED).
#[must_use]
pub(crate) fn read_full_cut(
    session: &ProjectSession,
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
) -> FullCutReadings {
    let removed_volume_mm3 = removed_volume_from_trace(trace, toolpath_id);
    let Some(sim) = session.simulation_result() else {
        return FullCutReadings {
            removed_volume_mm3,
            rapid_collisions: None,
            overcut_peak_mm: None,
        };
    };
    let mut evidence = ProjectEvidence::from_simulation(sim);
    evidence.cut_trace = Some(trace);
    let rapid_collisions = evidence
        .attributed_rapid_collisions()
        .iter()
        .filter(|c| matches!(c.toolpath, Some((id, _)) if id == toolpath_id))
        .count();
    let triage = session.simulation_triage(&evidence);
    let overcut_peak_mm = triage
        .actions
        .iter()
        .chain(triage.safety.iter())
        .filter(|f| {
            f.diagnostic.id.as_str() == crate::diagnostics::ids::PROJECT_CROSSES_STANDING_MATERIAL
                && matches!(
                    f.diagnostic.scope,
                    crate::diagnostics::Scope::Toolpath { id } if id == toolpath_id
                )
        })
        .filter_map(|f| match &f.diagnostic.evidence {
            Some(crate::diagnostics::DiagnosticEvidence::SampleRange { observed, .. }) => {
                Some(*observed)
            }
            _ => None,
        })
        .fold(0.0_f64, f64::max);
    FullCutReadings {
        removed_volume_mm3,
        rapid_collisions: Some(rapid_collisions),
        overcut_peak_mm: Some(overcut_peak_mm),
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

    /// The dexel cell of these hand-built readings, in mm.
    const CELL_MM: f64 = 0.2;
    /// The total depth of these hand-built readings, in mm.
    const DEPTH_MM: f64 = 20.0;

    fn readings(volume: f64, collisions: usize, overcut: f64) -> FullCutReadings {
        FullCutReadings {
            removed_volume_mm3: Some(volume),
            rapid_collisions: Some(collisions),
            overcut_peak_mm: Some(overcut),
        }
    }

    fn baseline() -> FullCutCheck {
        FullCutCheck::reference(readings(10_000.0, 1, 0.5))
    }

    #[test]
    fn the_baseline_holds() {
        assert!(baseline().verdict.holds());
    }

    #[test]
    fn identical_readings_hold() {
        let check = FullCutCheck::judge(
            &baseline(),
            readings(10_000.0, 1, 0.5),
            CELL_MM,
            Some(DEPTH_MM),
        );
        assert!(check.verdict.holds(), "{check:?}");
    }

    /// One floor layer is 10 000 × 0.2 / 20 = 100 mm³.
    #[test]
    fn a_volume_within_one_floor_layer_holds_and_beyond_it_fails() {
        let inside = FullCutCheck::judge(
            &baseline(),
            readings(9_901.0, 1, 0.5),
            CELL_MM,
            Some(DEPTH_MM),
        );
        assert!(inside.verdict.holds(), "{inside:?}");
        let short = FullCutCheck::judge(
            &baseline(),
            readings(9_899.0, 1, 0.5),
            CELL_MM,
            Some(DEPTH_MM),
        );
        assert!(
            matches!(short.verdict, FullCutVerdict::Fails { .. }),
            "{short:?}"
        );
        let over = FullCutCheck::judge(
            &baseline(),
            readings(10_101.0, 1, 0.5),
            CELL_MM,
            Some(DEPTH_MM),
        );
        assert!(
            matches!(over.verdict, FullCutVerdict::Fails { .. }),
            "{over:?}"
        );
    }

    #[test]
    fn no_total_depth_skips_the_volume_rule() {
        let check = FullCutCheck::judge(&baseline(), readings(1.0, 1, 0.5), CELL_MM, None);
        assert!(check.verdict.holds(), "{check:?}");
    }

    #[test]
    fn one_more_rapid_collision_fails() {
        let check = FullCutCheck::judge(
            &baseline(),
            readings(10_000.0, 2, 0.5),
            CELL_MM,
            Some(DEPTH_MM),
        );
        assert!(
            matches!(check.verdict, FullCutVerdict::Fails { .. }),
            "{check:?}"
        );
        let fewer = FullCutCheck::judge(
            &baseline(),
            readings(10_000.0, 0, 0.5),
            CELL_MM,
            Some(DEPTH_MM),
        );
        assert!(fewer.verdict.holds(), "{fewer:?}");
    }

    #[test]
    fn an_overcut_beyond_one_cell_fails() {
        let inside = FullCutCheck::judge(
            &baseline(),
            readings(10_000.0, 1, 0.7),
            CELL_MM,
            Some(DEPTH_MM),
        );
        assert!(inside.verdict.holds(), "{inside:?}");
        let deeper = FullCutCheck::judge(
            &baseline(),
            readings(10_000.0, 1, 0.71),
            CELL_MM,
            Some(DEPTH_MM),
        );
        assert!(
            matches!(deeper.verdict, FullCutVerdict::Fails { .. }),
            "{deeper:?}"
        );
    }

    /// A baseline with no finding compares as 0.
    #[test]
    fn a_new_overcut_finding_compares_against_zero() {
        let base = FullCutCheck::reference(readings(10_000.0, 0, 0.0));
        let new = FullCutCheck::judge(&base, readings(10_000.0, 0, 0.25), CELL_MM, Some(DEPTH_MM));
        assert!(
            matches!(new.verdict, FullCutVerdict::Fails { .. }),
            "{new:?}"
        );
    }

    #[test]
    fn a_missing_reading_is_not_measured() {
        let base = FullCutCheck::reference(FullCutReadings {
            removed_volume_mm3: Some(10_000.0),
            rapid_collisions: None,
            overcut_peak_mm: None,
        });
        let check = FullCutCheck::judge(&base, readings(10_000.0, 0, 0.0), CELL_MM, Some(DEPTH_MM));
        assert!(
            matches!(check.verdict, FullCutVerdict::NotMeasured { .. }),
            "{check:?}"
        );
    }

    /// A measured failure outranks a missing reading.
    #[test]
    fn a_measured_failure_outranks_a_missing_reading() {
        let base = FullCutCheck::reference(FullCutReadings {
            removed_volume_mm3: Some(10_000.0),
            rapid_collisions: None,
            overcut_peak_mm: None,
        });
        let check = FullCutCheck::judge(&base, readings(5_000.0, 0, 0.0), CELL_MM, Some(DEPTH_MM));
        assert!(
            matches!(check.verdict, FullCutVerdict::Fails { .. }),
            "{check:?}"
        );
    }
}
