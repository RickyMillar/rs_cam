//! Can this simulation actually measure the metric you are about to gate on?
//!
//! # Why this module exists
//!
//! `SIMULATION_ISSUE_CHANNEL_CENSUS.md` §5 measured a hard, fixed floor in
//! the stamping kernel
//! ([`crate::dexel_stock::FRESH_MATERIAL_THRESHOLD_MM`], 0.05 mm)
//! and a second, cell-size-dependent lateral-resolution condition
//! (`PERP_COVERAGE_GATE`, census "Floor 2"). Under either, the perpendicular
//! extent of fresh cells never gets two distinct readings, so
//! `radial_engagement` is set to **exactly zero** — and since air cut is
//! *defined* as `radial_woc_fraction < 0.02`, every cutting sample of such a
//! pass is classified air cut.
//!
//! The census's measured arms, same geometry, two depths:
//!
//! ```text
//! shallow 0.02 mm: air 95.9% of total runtime, avg engagement 0.0000,
//!                  peak removed height 0.0200 mm, removed volume 63.7 mm³
//! deep    2.00 mm: air 49.6% of total runtime, avg engagement 0.2293,
//!                  peak removed height 1.9800 mm, removed volume 6515.9 mm³
//! ```
//!
//! The shallow arm removes real material, reports its removed *height*
//! correctly, and reports zero engagement with 96% air cut. Nothing in any
//! shipped surface said the number was unmeasurable: it printed as a
//! precise-looking percentage and **cleared every bar** — the GUI's 20%
//! banner, the CLI's 40% verdict, and the 30% finish band in
//! [`crate::compute::catalog::OperationType::air_cut_high_threshold_pct`],
//! which is exactly where sub-0.05 mm passes actually live.
//!
//! # What was ruled
//!
//! Checkpoint D Q2 (2026-08-04): **a gate consuming a `NotMeasurable` metric
//! abstains**, with a stated reason, instead of producing a verdict. The
//! floor is documented, not tuned (lowering it re-admits the float-noise
//! cells it exists to reject). **Collision detection always stays live** —
//! it does not touch the engagement path at all.
//!
//! This module is the detector and the report. It changes no threshold: a
//! `NotMeasurable` metric simply stops feeding its gate.
//!
//! # How the detection works
//!
//! Purely from the trace, per toolpath, with no kernel change: a sample that
//! **removed material and still read zero radial engagement** is a sample the
//! engagement channel could not see.
//!
//! ```text
//! removing = cutting samples with removed_volume_est_mm3 > 0
//! blind    = removing samples with engagement.radial_woc_fraction <= 0
//! ```
//!
//! Note what this deliberately does *not* flag: a pass over already-cleared
//! ground removes nothing and reads zero, which is a **correct** air-cut
//! reading, not a blind one. The census's `pocket-recut-air` toolpath — 100%
//! air, 20,328 of 20,328 cutting samples — is `Measurable`.
//!
//! The two floors are told apart by the deepest blind sample's removed
//! height: at or under the material floor it is Floor 1 (independent of cell
//! size); above it, the material was thick enough and the *lateral* extent is
//! what failed, which is Floor 2.

use crate::compute::catalog::OperationType;
use crate::dexel_stock::FRESH_MATERIAL_THRESHOLD_MM;
use crate::ids::ToolpathId;
use crate::simulation_cut::SimulationCutTrace;
use serde::{Deserialize, Serialize};

/// A simulation metric that a gate, a verdict, or a UI panel might read.
///
/// Split by **measurement mechanism**, not by field name, because that is
/// what determines whether a metric survives the floors (census §5.2). The
/// first three all derive from the same perpendicular-extent measurement and
/// therefore fail together; the rest are independent of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimMetric {
    /// `engagement.radial_woc_fraction`, `average_engagement`,
    /// `peak_radial_woc_fraction`.
    RadialEngagement,
    /// `air_cut_time_s` and **both** its percentages, plus
    /// `low_engagement_time_s`. Derived from `radial_woc_fraction < 0.02`.
    AirCut,
    /// `arc_engagement_radians` and everything downstream of it — chip
    /// thickness, and the chipload gate that reads it.
    ChipEngagement,
    /// `axial_engagement_mm` / `peak_axial_doc_mm`. Measured as
    /// `pre_ray_len − post_ray_len`, with no coverage gate; survives both
    /// floors.
    AxialEngagement,
    /// Gross removed volume / removed height. Z is continuous; survives.
    MaterialRemoval,
    /// Rapid-through-stock scan. Independent of the engagement path.
    RapidCollision,
    /// Holder / shank / fixture sweep. Analytic; independent.
    HolderCollision,
}

impl SimMetric {
    /// Every metric this module reports on, in report order.
    pub const ALL: [SimMetric; 7] = [
        SimMetric::RadialEngagement,
        SimMetric::AirCut,
        SimMetric::ChipEngagement,
        SimMetric::AxialEngagement,
        SimMetric::MaterialRemoval,
        SimMetric::RapidCollision,
        SimMetric::HolderCollision,
    ];

    /// The three metrics derived from the perpendicular-extent measurement,
    /// and therefore the only ones the two floors can take out.
    pub const ENGAGEMENT_DERIVED: [SimMetric; 3] = [
        SimMetric::RadialEngagement,
        SimMetric::AirCut,
        SimMetric::ChipEngagement,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::RadialEngagement => "radial_engagement",
            Self::AirCut => "air_cut",
            Self::ChipEngagement => "chip_engagement",
            Self::AxialEngagement => "axial_engagement",
            Self::MaterialRemoval => "material_removal",
            Self::RapidCollision => "rapid_collision",
            Self::HolderCollision => "holder_collision",
        }
    }

    /// Operator-facing name.
    pub fn label(self) -> &'static str {
        match self {
            Self::RadialEngagement => "radial engagement",
            Self::AirCut => "air-cut %",
            Self::ChipEngagement => "chip engagement",
            Self::AxialEngagement => "axial DOC",
            Self::MaterialRemoval => "material removal",
            Self::RapidCollision => "rapid collision",
            Self::HolderCollision => "holder collision",
        }
    }
}

/// Why a metric could not be measured. Carries the numbers that justify the
/// call so the abstention can state itself rather than assert itself.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum MeasurabilityReason {
    /// Census Floor 1. The pass removes less material per stamp than the
    /// kernel's fixed fresh-material threshold, so no cell qualifies to
    /// contribute a perpendicular extent. **Independent of cell size** —
    /// re-running at a finer resolution does not fix this.
    BelowFreshMaterialFloor {
        /// Deepest removed height among the blind samples (mm). At or below
        /// `floor_mm` by construction.
        peak_removed_mm: f64,
        /// [`FRESH_MATERIAL_THRESHOLD_MM`].
        floor_mm: f64,
        /// Fraction of material-removing cutting samples that read zero.
        blind_fraction: f64,
    },
    /// Census Floor 2. Material was thick enough to clear the fresh-material
    /// floor, so the *lateral* extent is what failed: fewer than two cell
    /// centres qualified at distinct perpendicular offsets. **This one is a
    /// function of cell size** — a finer grid can fix it.
    ///
    /// The closed form `cell ≲ √(2·R_tip·d − d²)` needs the tool's tip
    /// radius, which the trace does not carry, so the payload reports the
    /// measured inputs rather than a derived contact radius (census §9: the
    /// closed form is derived, not swept).
    CellTooCoarseForTipContact {
        /// Simulation cell size (mm), when the caller supplied it.
        cell_mm: Option<f64>,
        /// Deepest removed height among the blind samples (mm). Above
        /// `FRESH_MATERIAL_THRESHOLD_MM` by construction.
        peak_removed_mm: f64,
        blind_fraction: f64,
    },
    /// The op's kinematics fall outside the model entirely — the existing
    /// drill / alignment-pin-drill case, already flagged as
    /// `metrics_not_applicable`. Drill-native metrics are measured
    /// separately and are unaffected.
    KinematicsNotModelled,
}

impl MeasurabilityReason {
    /// One-line operator/agent-facing statement of the reason.
    pub fn describe(&self) -> String {
        match *self {
            Self::BelowFreshMaterialFloor {
                peak_removed_mm,
                floor_mm,
                blind_fraction,
            } => format!(
                "{:.0}% of material-removing samples read zero engagement: the pass removes \
                 at most {peak_removed_mm:.3} mm per stamp, under the {floor_mm:.2} mm \
                 fresh-material floor. A finer simulation cell does NOT fix this — the floor \
                 is independent of cell size.",
                blind_fraction * 100.0
            ),
            Self::CellTooCoarseForTipContact {
                cell_mm,
                peak_removed_mm,
                blind_fraction,
            } => {
                let cell = match cell_mm {
                    Some(c) => format!("{c:.3} mm cell"),
                    None => "the simulation cell".to_owned(),
                };
                format!(
                    "{:.0}% of material-removing samples read zero engagement at {cell}, \
                     while removing up to {peak_removed_mm:.3} mm: the contact patch is \
                     narrower than the grid can resolve. Re-simulate below the tool's TIP \
                     radius.",
                    blind_fraction * 100.0
                )
            }
            Self::KinematicsNotModelled => "the dexel's XY-cylinder side-engagement model does \
                                            not apply to this op's kinematics (Z-only moves); \
                                            drill-native metrics are measured separately."
                .to_owned(),
        }
    }
}

/// Verdict for one `(toolpath, metric)` pair.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Measurability {
    /// The measurement is sound; read the number.
    Measurable,
    /// Part of the pass is unmeasurable. The number is real for the rest, so
    /// it is published — but a gate reading it should say so.
    Degraded(MeasurabilityReason),
    /// The number is not a measurement. **It must not be published as a
    /// value, and any gate consuming it abstains** (Checkpoint D Q2).
    NotMeasurable(MeasurabilityReason),
}

impl Measurability {
    /// True when a gate reading this metric must decline to produce a
    /// verdict. `Degraded` does **not** abstain — the reading is real for
    /// the measurable part of the pass, and abstaining there would hide more
    /// than it protects.
    pub fn abstains(&self) -> bool {
        matches!(self, Self::NotMeasurable(_))
    }

    /// True when the value may be published as a number at all.
    pub fn publishable(&self) -> bool {
        !self.abstains()
    }

    pub fn reason(&self) -> Option<MeasurabilityReason> {
        match *self {
            Self::Measurable => None,
            Self::Degraded(r) | Self::NotMeasurable(r) => Some(r),
        }
    }
}

/// One row of the report.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MetricMeasurability {
    pub toolpath_id: ToolpathId,
    pub metric: SimMetric,
    pub measurability: Measurability,
}

/// Fraction of material-removing samples reading zero engagement at or above
/// which the metric is declared `NotMeasurable`.
///
/// At half, the *majority* of the samples that actually cut are invisible to
/// the engagement channel, so any aggregate over them (a time-weighted mean,
/// a percentage of runtime) is describing the blind part more than the
/// measured part. The census's shallow arm sits at ~1.0 and its deep arm at
/// ~0.0, so the fixture that motivated the ruling is nowhere near this
/// boundary — this is not a tuned knob, and it moves no gate threshold.
pub const NOT_MEASURABLE_BLIND_FRACTION: f64 = 0.5;

/// Fraction at or above which the metric is `Degraded` — measurable in part.
/// Below it, blind samples are edge effects (pass ends, boundary cells) that
/// every real cut produces.
pub const DEGRADED_BLIND_FRACTION: f64 = 0.1;

/// Per-`(toolpath, metric)` measurability, plus the project-level context it
/// was derived under.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MeasurabilityReport {
    pub entries: Vec<MetricMeasurability>,
    /// Simulation cell size the trace was captured at (mm), when known.
    #[serde(default)]
    pub cell_mm: Option<f64>,
}

impl MeasurabilityReport {
    /// Build the report from a finished trace.
    ///
    /// `cell_mm` is the simulation resolution, when the caller knows it —
    /// it only ever enriches a reason payload, never changes a verdict.
    pub fn from_trace(trace: &SimulationCutTrace, cell_mm: Option<f64>) -> Self {
        let mut entries = Vec::new();

        for tp in &trace.toolpath_summaries {
            let id = tp.toolpath_id;

            // The metrics that survive both floors, always. Stated
            // explicitly rather than left absent, because the ruling
            // requires an abstention to name what *is* still valid —
            // collision detection above all.
            for metric in [
                SimMetric::AxialEngagement,
                SimMetric::MaterialRemoval,
                SimMetric::RapidCollision,
                SimMetric::HolderCollision,
            ] {
                entries.push(MetricMeasurability {
                    toolpath_id: id,
                    metric,
                    measurability: Measurability::Measurable,
                });
            }

            let engagement_verdict = if tp.metrics_not_applicable {
                Measurability::NotMeasurable(MeasurabilityReason::KinematicsNotModelled)
            } else {
                classify_engagement(trace, id, cell_mm)
            };
            for metric in SimMetric::ENGAGEMENT_DERIVED {
                entries.push(MetricMeasurability {
                    toolpath_id: id,
                    metric,
                    measurability: engagement_verdict,
                });
            }
        }

        Self { entries, cell_mm }
    }

    /// Verdict for one `(toolpath, metric)` pair.
    ///
    /// A toolpath with no row — a trace that never covered it — is reported
    /// `Measurable` rather than abstaining: absence of a measurement is not
    /// evidence that measurement failed, and abstaining by default would
    /// silently disarm every gate on the first caller that forgot to build a
    /// report.
    pub fn for_metric(&self, toolpath_id: ToolpathId, metric: SimMetric) -> Measurability {
        self.entries
            .iter()
            .find(|e| e.toolpath_id == toolpath_id && e.metric == metric)
            .map(|e| e.measurability)
            .unwrap_or(Measurability::Measurable)
    }

    /// True when a gate on `metric` for this toolpath must abstain.
    pub fn abstains(&self, toolpath_id: ToolpathId, metric: SimMetric) -> bool {
        self.for_metric(toolpath_id, metric).abstains()
    }

    /// Toolpaths whose `metric` is `NotMeasurable`, with the reason.
    pub fn abstentions(
        &self,
        metric: SimMetric,
    ) -> impl Iterator<Item = (ToolpathId, MeasurabilityReason)> + '_ {
        self.entries.iter().filter_map(move |e| {
            if e.metric != metric {
                return None;
            }
            match e.measurability {
                Measurability::NotMeasurable(r) => Some((e.toolpath_id, r)),
                _ => None,
            }
        })
    }

    /// True when every row is `Measurable` — the "nothing to say" case the
    /// operator strip must render without a warning colour.
    pub fn all_measurable(&self) -> bool {
        self.entries
            .iter()
            .all(|e| e.measurability == Measurability::Measurable)
    }
}

/// Classify the engagement-derived metrics for one milling toolpath.
fn classify_engagement(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
    cell_mm: Option<f64>,
) -> Measurability {
    let mut removing = 0usize;
    let mut blind = 0usize;
    let mut blind_peak_removed_mm = 0.0f64;

    for s in trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == toolpath_id)
    {
        if !s.is_cutting || s.removed_volume_est_mm3 <= 0.0 {
            continue;
        }
        removing += 1;
        if s.engagement.radial_woc_fraction > 0.0 {
            continue;
        }
        blind += 1;
        // A plunge's descent lands in `plunge_descent_mm`, a lateral cut's
        // in `axial_engagement_mm`; the removed height is whichever the
        // emitter used.
        blind_peak_removed_mm =
            blind_peak_removed_mm.max(s.axial_engagement_mm.max(s.plunge_descent_mm));
    }

    // Removed nothing anywhere: a genuine all-air pass. Zero engagement is
    // the CORRECT reading, not a blind one.
    if removing == 0 {
        return Measurability::Measurable;
    }

    let blind_fraction = blind as f64 / removing as f64;
    if blind_fraction < DEGRADED_BLIND_FRACTION {
        return Measurability::Measurable;
    }

    let reason = if blind_peak_removed_mm <= FRESH_MATERIAL_THRESHOLD_MM {
        MeasurabilityReason::BelowFreshMaterialFloor {
            peak_removed_mm: blind_peak_removed_mm,
            floor_mm: FRESH_MATERIAL_THRESHOLD_MM,
            blind_fraction,
        }
    } else {
        MeasurabilityReason::CellTooCoarseForTipContact {
            cell_mm,
            peak_removed_mm: blind_peak_removed_mm,
            blind_fraction,
        }
    };

    if blind_fraction >= NOT_MEASURABLE_BLIND_FRACTION {
        Measurability::NotMeasurable(reason)
    } else {
        Measurability::Degraded(reason)
    }
}

/// Operation families whose per-stamp axial engagement routinely sits at or
/// under the fresh-material floor, from the census §5.3 matrix.
///
/// **Advisory only — this is not the detector.** The verdict always comes
/// from the measured trace ([`MeasurabilityReport::from_trace`]); this is for
/// pre-simulation UI copy that wants to warn before there is anything to
/// measure. A finish op at a coarse stepover can be perfectly measurable and
/// a pocket at a 0.02 mm spring pass can be blind, so never gate on this.
pub fn engagement_is_floor_prone(op: OperationType) -> bool {
    matches!(
        op,
        OperationType::DropCutter
            | OperationType::Scallop
            | OperationType::Waterline
            | OperationType::HorizontalFinish
            | OperationType::SteepShallow
            | OperationType::RampFinish
            | OperationType::SpiralFinish
            | OperationType::RadialFinish
            | OperationType::UnifiedFinish
            | OperationType::Pencil
    )
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
    use crate::simulation_cut::{Engagement, SimulationCutSample};

    fn sample(id: usize, removed_mm3: f64, radial: f64, axial_mm: f64) -> SimulationCutSample {
        SimulationCutSample {
            toolpath_id: ToolpathId(id),
            is_cutting: true,
            segment_time_s: 0.01,
            removed_volume_est_mm3: removed_mm3,
            axial_engagement_mm: axial_mm,
            axial_doc_mm: axial_mm,
            engagement: Engagement::with_radial_woc(radial),
            ..SimulationCutSample::test_fixture()
        }
    }

    #[test]
    fn a_pass_under_the_material_floor_is_not_measurable() {
        // Removes real material at 0.02 mm per stamp and reads exactly zero
        // engagement — the census §5.1 shallow arm.
        let samples: Vec<_> = (0..20).map(|_| sample(1, 0.4, 0.0, 0.02)).collect();
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let report = MeasurabilityReport::from_trace(&trace, Some(0.5));

        for metric in SimMetric::ENGAGEMENT_DERIVED {
            let m = report.for_metric(ToolpathId(1), metric);
            assert!(m.abstains(), "{metric:?} should abstain, got {m:?}");
            assert!(matches!(
                m.reason(),
                Some(MeasurabilityReason::BelowFreshMaterialFloor { .. })
            ));
        }
    }

    #[test]
    fn collisions_and_removal_stay_live_when_engagement_abstains() {
        let samples: Vec<_> = (0..20).map(|_| sample(1, 0.4, 0.0, 0.02)).collect();
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let report = MeasurabilityReport::from_trace(&trace, Some(0.5));

        for metric in [
            SimMetric::RapidCollision,
            SimMetric::HolderCollision,
            SimMetric::MaterialRemoval,
            SimMetric::AxialEngagement,
        ] {
            assert_eq!(
                report.for_metric(ToolpathId(1), metric),
                Measurability::Measurable,
                "{metric:?} must stay live — the ruling says collision detection always does"
            );
        }
    }

    #[test]
    fn a_normal_cut_is_measurable() {
        let samples: Vec<_> = (0..20).map(|_| sample(1, 8.0, 0.35, 2.0)).collect();
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let report = MeasurabilityReport::from_trace(&trace, Some(0.5));
        assert!(report.all_measurable());
    }

    #[test]
    fn an_all_air_pass_reads_zero_correctly_and_is_measurable() {
        // Removes nothing, reads zero. That is a CORRECT air-cut reading,
        // not a blind one — the census's `pocket-recut-air` toolpath.
        let samples: Vec<_> = (0..20).map(|_| sample(1, 0.0, 0.0, 0.0)).collect();
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let report = MeasurabilityReport::from_trace(&trace, Some(0.5));
        assert_eq!(
            report.for_metric(ToolpathId(1), SimMetric::AirCut),
            Measurability::Measurable
        );
    }

    #[test]
    fn thick_material_reading_zero_blames_the_cell_not_the_floor() {
        // Removes 0.4 mm per stamp — well clear of the material floor — and
        // still reads zero: the lateral extent is what failed.
        let samples: Vec<_> = (0..20).map(|_| sample(1, 2.0, 0.0, 0.4)).collect();
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let report = MeasurabilityReport::from_trace(&trace, Some(0.5));
        let m = report.for_metric(ToolpathId(1), SimMetric::RadialEngagement);
        assert!(matches!(
            m.reason(),
            Some(MeasurabilityReason::CellTooCoarseForTipContact {
                cell_mm: Some(0.5),
                ..
            })
        ));
    }

    #[test]
    fn a_mostly_measured_pass_is_degraded_not_abstained() {
        // 20% blind — real, worth saying, but the reading still describes
        // the majority of the cut.
        let mut samples: Vec<_> = (0..16).map(|_| sample(1, 8.0, 0.35, 2.0)).collect();
        samples.extend((0..4).map(|_| sample(1, 0.4, 0.0, 0.02)));
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let report = MeasurabilityReport::from_trace(&trace, Some(0.5));
        let m = report.for_metric(ToolpathId(1), SimMetric::AirCut);
        assert!(matches!(m, Measurability::Degraded(_)), "got {m:?}");
        assert!(!m.abstains(), "Degraded must NOT abstain");
    }

    #[test]
    fn an_unknown_toolpath_does_not_abstain_by_default() {
        let report = MeasurabilityReport::default();
        assert!(!report.abstains(ToolpathId(99), SimMetric::AirCut));
    }
}
