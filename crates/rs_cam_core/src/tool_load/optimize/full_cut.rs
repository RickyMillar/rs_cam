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
//! - the over-cut peak: the peak bite of the standing-material rule
//!   (`stock::sim_triage::standing_material_reading`, the population and
//!   the bar of the "crosses material" finding
//!   `diagnostics::ids::PROJECT_CROSSES_STANDING_MATERIAL`), without the
//!   sample and fraction floors of that finding.
//!
//! The baseline is the reference. It carries its readings with the
//! verdict [`FullCutVerdict::Holds`].

use serde::{Deserialize, Serialize};

use crate::ids::ToolpathId;
use crate::session::{ProjectEvidence, ProjectSession};
use crate::stock::sim_triage::{
    StandingMaterialReading, commanded_step_mm, standing_material_reading,
};
use crate::stock::simulation_cut::SimulationCutTrace;

/// The result of the full-cut check on one candidate.
///
/// Each reading is three-valued. `None` means NOT MEASURED. A measured
/// population with no sample over the bar is `Some(0.0)` for the over-cut
/// peak, and no flagged rapid is `Some(0)` for the collision count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullCutCheck {
    /// The volume this toolpath removed, in mm³. `None` when the trace
    /// holds no summary for the toolpath.
    pub removed_volume_mm3: Option<f64>,
    /// The rapid collisions the simulation attributes to this toolpath.
    /// `None` when no simulation result was available.
    pub rapid_collisions: Option<usize>,
    /// The over-cut peak, in mm: the largest bite of the standing-material
    /// population when one or more samples are over the bar (the commanded
    /// step plus one cell, or three times the own median bite), and
    /// `Some(0.0)` when no sample is over the bar. `None` (NOT MEASURED)
    /// when no simulation result was available or the population is empty.
    ///
    /// The reading has no sample floor and no fraction floor. A deep cell
    /// with fewer than 50 steady samples is therefore measured, and does
    /// not read as clean only because the triage finding abstained.
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
    /// `resolution_mm` is the REALISED dexel cell of the candidate's
    /// simulation (`SimulationResult::column_grid_cell_mm`, see
    /// [`realised_cell_mm`]). The triage finding reads the same field, so
    /// the finding and this check use one tolerance.
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
    ///
    ///    The derivation assumes that the operation removes `A × T`. An
    ///    operation that removes less (rest stock, or a top below the
    ///    stock top) has `V < A × T`, so the tolerance `V × r / T` is
    ///    smaller than one floor layer `A × r`. The rule is then stricter
    ///    than one floor layer: it fails in the safe direction.
    /// 2. **Collisions.** The candidate has more rapid collisions than
    ///    the baseline.
    /// 3. **Over-cut.** The candidate's over-cut peak
    ///    ([`FullCutCheck::overcut_peak_mm`]) is more than the baseline's
    ///    peak plus one cell (`r`). The rule compares two peaks, not two
    ///    findings. A side with no sample over its bar reads 0. The bar of
    ///    a stepped op already holds one cell (the dexel quantisation of a
    ///    bite against its own step); the cell in this comparison is a
    ///    second one, for the quantisation between two simulations.
    ///
    /// The collision count is an UPPER-BOUND reading: the dexel
    /// `conservative_top` stays high over two partial covers, so the check
    /// can flag a rapid that does not strike. A rise in the count still
    /// fails the candidate.
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
                        "the rapid-collision check flags {cand} rapids against {base} on the \
                         baseline (an upper-bound reading)"
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
            (None, _) => missing.push("the over-cut peak of the baseline".to_owned()),
            (_, None) => missing.push("the over-cut peak of the candidate".to_owned()),
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

/// The dexel cell of the session's simulation, in mm: the realised
/// `SimulationResult::column_grid_cell_mm`, which `effective_cell_size` can
/// make larger than the requested resolution.
///
/// Fallback: with no simulation result on the session, the stored project
/// resolution (`ProjectSession::simulation_resolution_mm`). The cut trace
/// carries no cell of its own. In that case the collisions and the over-cut
/// are NOT MEASURED ([`read_full_cut`]), so the fallback can only move the
/// volume tolerance.
#[must_use]
pub(crate) fn realised_cell_mm(session: &ProjectSession) -> f64 {
    session.simulation_result().map_or_else(
        || session.simulation_resolution_mm(),
        |sim| sim.column_grid_cell_mm,
    )
}

/// The over-cut peak of a standing-material reading
/// ([`FullCutCheck::overcut_peak_mm`]): `None` for an empty population,
/// the peak bite when one or more samples are over the bar, else 0.
#[must_use]
pub(crate) fn overcut_peak_from_reading(reading: &StandingMaterialReading) -> Option<f64> {
    if reading.population == 0 {
        return None;
    }
    if reading.over_bar == 0 {
        return Some(0.0);
    }
    reading.peak_bite_mm
}

/// Read the three full-cut readings of one toolpath.
///
/// `trace` gives the volume and the over-cut. The session's simulation
/// result gives the collisions and the dexel cell
/// ([`realised_cell_mm`]). The over-cut reads the standing-material rule
/// through its one reading door,
/// [`crate::stock::sim_triage::standing_material_reading`], with the
/// commanded step of the operation the session holds now
/// (`sim_triage::commanded_step_mm`) and the realised cell: the population
/// and the bar of the triage finding. With no simulation result on the
/// session, the collisions and the over-cut are `None` (NOT MEASURED).
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
    let commanded_step = session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)
        .and_then(|tc| commanded_step_mm(&tc.operation));
    let reading = standing_material_reading(
        trace,
        toolpath_id,
        commanded_step,
        Some(sim.column_grid_cell_mm),
    );
    FullCutReadings {
        removed_volume_mm3,
        rapid_collisions: Some(rapid_collisions),
        overcut_peak_mm: overcut_peak_from_reading(&reading),
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

    /// R2: a trace with 10 steady samples, one of them a 3x bite. The
    /// triage finding abstains (fewer than 50 samples). The reading still
    /// reports the peak, and the over-cut rule fails the candidate on it.
    #[test]
    fn a_reading_under_the_finding_floor_still_fails_the_candidate_on_its_peak() {
        use crate::stock::sim_triage::standing_material_reading;
        use crate::stock::simulation_cut::{Engagement, SimulationCutSample};

        const STEP_MM: f64 = 1.0;
        let samples: Vec<SimulationCutSample> = (0..10)
            .map(|i| SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: i,
                sample_index: i,
                segment_time_s: 0.01,
                is_cutting: true,
                axial_engagement_mm: if i == 0 { 3.0 * STEP_MM } else { STEP_MM },
                engagement: Engagement::with_radial_woc(0.3),
                removed_volume_est_mm3: 1.0,
                ..SimulationCutSample::test_fixture()
            })
            .collect();
        let trace = SimulationCutTrace::from_samples(0.5, samples);

        let reading =
            standing_material_reading(&trace, ToolpathId(1), Some(STEP_MM), Some(CELL_MM));
        assert_eq!(reading.population, 10, "{reading:?}");
        assert_eq!(reading.over_bar, 1, "{reading:?}");
        assert_eq!(reading.peak_bite_mm, Some(3.0 * STEP_MM), "{reading:?}");
        // The finding abstains under its sample floor; the sim_triage test
        // `r2_a_reading_reports_the_peak_where_the_finding_abstains` pins it.
        assert!(reading.population < crate::stock::sim_triage::STANDING_MATERIAL_MIN_SAMPLES);

        let peak = overcut_peak_from_reading(&reading);
        assert_eq!(peak, Some(3.0 * STEP_MM));
        let base = FullCutCheck::reference(readings(10_000.0, 0, 0.0));
        let check = FullCutCheck::judge(
            &base,
            FullCutReadings {
                overcut_peak_mm: peak,
                ..readings(10_000.0, 0, 0.0)
            },
            CELL_MM,
            Some(DEPTH_MM),
        );
        assert!(
            matches!(check.verdict, FullCutVerdict::Fails { .. }),
            "{check:?}"
        );
    }

    /// An empty population is NOT MEASURED; a population with no sample
    /// over the bar is a measured 0.
    #[test]
    fn an_empty_population_is_not_measured_and_a_clean_one_is_zero() {
        use crate::stock::sim_triage::StandingMaterialReading;
        assert_eq!(
            overcut_peak_from_reading(&StandingMaterialReading::default()),
            None
        );
        let clean = StandingMaterialReading {
            population: 5,
            peak_bite_mm: Some(1.0),
            over_bar: 0,
            bar_mm: Some(1.2),
        };
        assert_eq!(overcut_peak_from_reading(&clean), Some(0.0));
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
