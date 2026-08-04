//! **The stage-labelled feed explanation record.**
//!
//! Census `planning/review_2026-08-04/FEEDS_CENSUS.md` tier-1 item T1.1,
//! ruled at Checkpoint B Q2. Reference semantics:
//! `crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs` (the
//! test-only assembler that reconciled the live B3 evidence, commit
//! `5f7bb25`).
//!
//! ## Why this type exists
//!
//! A live session on 2026-07-30 produced four chipload numbers for one
//! operation, no two agreeing, with nothing on screen saying they were
//! four *different quantities*:
//!
//! | stage | mm/tooth |
//! |---|---|
//! | narration nominal | 0.0714 |
//! | Suggest's would-be chipload → rubbing floor | 0.0044 → 0.0250 |
//! | gate observed | 0.000737 |
//! | gate band | 0.00458 – 0.00916 |
//!
//! Three of those are the same physical quantity at three stages; one is
//! a different quantity wearing the same label. The census closed the
//! arithmetic exactly (§4.3):
//!
//! ```text
//! gate_observed = commanded_fpt × mean_chip_factor(LUT nominal arc)
//!                              × predicted_feed / commanded_feed
//! ```
//!
//! Every term on the right is computed inside `tool_load::chipload` and
//! then **discarded**. This record keeps them, each under its own name
//! and unit.
//!
//! ## The one rule this type is written under
//!
//! **It labels stages. It does not pick a winner.** No method here says
//! which number is "the" chipload, and none converts between stages —
//! whether the gate's observation and the vendor band should be
//! compared in one unit at all is Checkpoint B item T4.1, still open.
//! [`FeedExplanation::commanded_over_band_max`] is the single ratio
//! offered, and it is offered precisely because it is the **only
//! same-unit, same-stage comparison** available (census P-10): both
//! sides are a linear advance per tooth.

use serde::{Deserialize, Serialize};

use crate::feeds::vendor_lut::LutPassRole;
use crate::tool_load::verdict::ChipBoundsSource;

/// Which order statistic over the steady-state sample set the gate
/// reported. Named because "chipload" alone does not say whether a
/// median or a peak is on screen, and the two arms of
/// [`crate::tool_load::verdict::ChiploadVerdict`] use different ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedStatistic {
    /// Median over steady-state, non-entry, non-phantom samples — the
    /// burn-side statistic.
    Median,
    /// Largest single steady-state sample — the breakage-side statistic.
    Peak,
}

impl ObservedStatistic {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ObservedStatistic::Median => "median of steady-state samples",
            ObservedStatistic::Peak => "peak steady-state sample",
        }
    }
}

/// **Stage 1 — what the operation was commanded to do.**
///
/// `feed / (rpm · flutes)`. The quantity `narrate.rs` prints as "nominal
/// chipload" and the one vendor tables are published in.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CommandedStage {
    pub feed_rate_mm_min: f64,
    pub spindle_rpm: u32,
    pub flute_count: u32,
    /// `feed_rate_mm_min / (spindle_rpm · flute_count)`.
    pub feed_per_tooth_mm: f64,
}

/// The unit stages 1 and 2 share. Named once so a consumer can assert
/// that a comparison between them is legitimate, rather than each site
/// spelling out a string and hoping they match.
pub const ADVANCE_PER_TOOTH: &str = "mm of linear advance per tooth";

impl CommandedStage {
    /// Unit of [`Self::feed_per_tooth_mm`], spelled out because the
    /// whole defect class here is two quantities sharing one label.
    #[must_use]
    pub const fn unit(&self) -> &'static str {
        ADVANCE_PER_TOOTH
    }
}

/// **Stage 2 — the vendor band, and where it came from.**
///
/// Bounds are post-scaling and post-DOC-derate: exactly the numbers the
/// verdict is judged against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LutBandStage {
    /// The matched observation's id, e.g.
    /// `amana-tapered-hardwood-scallop-3175-2f`.
    pub observation_id: String,
    /// Provenance class the gate assigned these bounds.
    pub bounds_source: ChipBoundsSource,
    /// Diameter the row is calibrated at (mm); `0.0` = no anchor.
    pub row_diameter_mm: f64,
    /// Engaged diameter the gate queried with (mm).
    pub queried_diameter_mm: f64,
    /// `queried / row` diameter scaling applied to the chipload bounds.
    pub diameter_scale: f64,
    /// `row_hardness / queried_hardness` scaling applied to the bounds.
    pub hardness_scale: f64,
    /// True when the combined scaling exceeded ±40 %.
    pub is_extrapolated: bool,
    /// Pass role the gate asked for.
    pub queried_pass_role: LutPassRole,
    /// Pass role the winning row actually publishes. **May differ** —
    /// pass role is a scoring term, not a filter (census §5, T4.4).
    pub row_pass_role: LutPassRole,
    /// Lower bound (mm/tooth), `None` when the row publishes no minimum.
    pub min_mm_per_tooth: Option<f64>,
    /// Upper bound (mm/tooth). Always present — the gate rejects rows
    /// without one.
    pub max_mm_per_tooth: f64,
}

impl LutBandStage {
    #[must_use]
    pub const fn unit(&self) -> &'static str {
        "mm of linear advance per tooth (vendor convention)"
    }

    /// The unit FAMILY, without the vendor-convention qualifier — equal
    /// to [`CommandedStage::unit`], which is what makes
    /// [`FeedExplanation::commanded_over_band_max`] a legitimate ratio.
    #[must_use]
    pub const fn unit_family(&self) -> &'static str {
        ADVANCE_PER_TOOTH
    }

    /// True when the winning row answers a different pass role than the
    /// one queried. Report-only: it does not change any verdict.
    #[must_use]
    pub fn pass_role_substituted(&self) -> bool {
        self.queried_pass_role != self.row_pass_role
    }

    /// True when either chipload scale moved the published band at all.
    /// Distinct from [`Self::is_extrapolated`], which only fires past
    /// ±40 % — census T1.6: a 0.99× row and a 0.38× row must be
    /// distinguishable in a report, not merged into "vendor_lut".
    #[must_use]
    pub fn is_scaled(&self) -> bool {
        (self.diameter_scale - 1.0).abs() > 1e-9 || (self.hardness_scale - 1.0).abs() > 1e-9
    }
}

/// **Stage 3 — the engagement arc the row was authored at.**
///
/// `mean_chip / feed_per_tooth` at that arc: the first of the two
/// multipliers between stage 1 and stage 5. Dimensionless.
///
/// `None` when the row carries no `ae` calibration, in which case the
/// gate skipped its arc normalisation entirely (`VendorLutMissingAe`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LutArcStage {
    pub nominal_arc_rad: Option<f64>,
    pub mean_chip_factor: Option<f64>,
}

/// **Stage 4 — achieved vs commanded feed.**
///
/// The second multiplier: the substitution
/// `tool_load::effective_feed_for_sample` performs when the trace
/// carries a kinematics-predicted feed map (F-035). On a corner-heavy
/// 3D path this routinely reads far below 1.0 — the live B3 op ran at
/// 0.128, i.e. −87 %.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AchievedFeedStage {
    /// True when the trace carried a populated predicted-feed map. When
    /// false the ratio is 1.0 *by absence of data*, not by measurement.
    pub predicted_feeds_present: bool,
    /// Median `predicted / commanded` over the sample set the gate
    /// measured. `None` when no predicted feeds were available.
    pub median_ratio: Option<f64>,
}

/// **Stage 5 — what the gate reported.**
///
/// Unit is *mm of chip*, not mm of advance: it is an arc-mean chip
/// thickness renormalised to stage 3's arc and evaluated at stage 4's
/// feed. That it is compared against a stage-2 band published in mm of
/// advance is census F-1, and **this record takes no position on it**.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GateObservationStage {
    pub statistic: ObservedStatistic,
    pub value_mm: f64,
    /// Steady-state samples that produced a usable chip thickness.
    pub sample_count: usize,
}

impl GateObservationStage {
    #[must_use]
    pub const fn unit(&self) -> &'static str {
        "mm of arc-mean chip thickness, at the LUT row's nominal arc"
    }
}

/// One operation's feed story, stage by stage, with every stage's unit
/// named. See the module header for the rule this type is written under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedExplanation {
    pub commanded: CommandedStage,
    pub band: LutBandStage,
    pub lut_arc: LutArcStage,
    pub achieved_feed: AchievedFeedStage,
    pub gate: GateObservationStage,
}

impl FeedExplanation {
    /// Stage 1 ÷ stage 2's maximum — **the only same-unit, same-stage
    /// comparison in this record**, and the one nothing surfaced before
    /// (census P-10 / T1.5).
    ///
    /// Both sides are a linear advance per tooth, so a value of 7.8
    /// means the operation is commanded to advance 7.8× further per
    /// tooth than the matched vendor row's published maximum. `None`
    /// when the band maximum is not positive.
    #[must_use]
    pub fn commanded_over_band_max(&self) -> Option<f64> {
        if self.band.max_mm_per_tooth > 0.0 {
            Some(self.commanded.feed_per_tooth_mm / self.band.max_mm_per_tooth)
        } else {
            None
        }
    }

    /// Stage 1 × stage 3 × stage 4 — the census §4.3 identity, i.e. what
    /// stage 5 should read if the two multipliers fully explain the
    /// delta. Returns `None` when either multiplier is unmeasured.
    ///
    /// This is a *prediction of* stage 5, never a replacement for it:
    /// the record always reports the gate's own observation as stage 5.
    #[must_use]
    pub fn predicted_gate_observation_mm(&self) -> Option<f64> {
        let arc_factor = self.lut_arc.mean_chip_factor?;
        let feed_ratio = self.achieved_feed.median_ratio.unwrap_or(1.0);
        Some(self.commanded.feed_per_tooth_mm * arc_factor * feed_ratio)
    }

    /// The multipliers between the commanded number an operator set and
    /// the number the verdict quotes, as a human-readable clause. Used
    /// by the diagnostic adapter so a report never states stage 5
    /// without stating what separates it from stage 1.
    #[must_use]
    pub fn multiplier_clause(&self) -> String {
        let arc = match self.lut_arc.mean_chip_factor {
            Some(f) => format!("×{f:.4} arc factor"),
            None => "no arc normalisation (row has no ae calibration)".to_owned(),
        };
        let feed = match self.achieved_feed.median_ratio {
            Some(r) => format!("×{r:.4} achieved/commanded feed"),
            None => "×1.0 feed (no predicted-feed map)".to_owned(),
        };
        format!("{arc}, {feed}")
    }
}
