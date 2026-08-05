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
//! The census closed the arithmetic exactly (§4.3): the gate's number
//! was the commanded feed-per-tooth times a chip-geometry factor times
//! the achieved/commanded feed ratio. Every term was computed inside
//! `tool_load::chipload` and then **discarded**. This record keeps them,
//! each under its own name and unit.
//!
//! ## What changed on 2026-08-06 — the chip-geometry stage is gone
//!
//! `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` answered
//! Checkpoint B item T4.1 from primary sources: **every vendor family in
//! the shipped LUT publishes its chipload column as a linear advance per
//! tooth**, `feed ÷ (rpm × cutting edges)` — printed as a defining
//! identity by Onsrud, Freud, Amana and Garr, and numerically
//! self-verifying on the Amana chart behind the live B3 row. No wood
//! source publishes a *radial* engagement condition for the column at
//! all; the LUT's `ae` windows are repo-authored application windows
//! (verdict §2.3), not vendor measurement conditions.
//!
//! So the gate's chip-geometry step was not a conversion between two
//! published quantities — it converted a published advance into a chip
//! that nothing published. It has been **deleted**, not inverted, and
//! the gate now observes
//!
//! ```text
//! gate_observed = effective_feed / (rpm · flutes)
//!               = commanded_fpt × predicted_feed / commanded_feed
//! ```
//!
//! which is the same unit as stage 1 and stage 2. The old "stage 3 — LUT
//! arc" is therefore no longer a stage in this pipeline; the row's `ae`
//! window survives on [`LutBandStage`] as report-only provenance.
//!
//! ## The one rule this type is written under
//!
//! **It labels stages. It does not pick a winner.** What it may now also
//! say — because the literature settled it, not because this module
//! decided it — is that stages 1, 2 and 4 are all the **same unit**:
//! [`ADVANCE_PER_TOOTH`]. [`FeedExplanation::commanded_over_band_max`]
//! and the gate's own verdict are therefore comparisons of like with
//! like, differing only by the disclosed achieved-feed ratio.

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
/// verdict is judged against. Unit: linear advance per tooth, verified
/// per source family in `CHIPLOAD_LITERATURE_VERDICT.md` §2.
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
    /// The row's calibrated radial-engagement window (mm), if it carries
    /// one. **Report-only and deliberately inert.**
    ///
    /// Until 2026-08-06 the gate derived an engagement arc from the
    /// midpoint of this window and renormalised its observation to it
    /// (D9). `CHIPLOAD_LITERATURE_VERDICT.md` §2.3 established that on
    /// every wood row these values are *repo-authored application
    /// windows* — `"scallop driven"`, `"10% to 30%D"`, `"width-at-depth"`
    /// — not transcriptions of a vendor measurement condition. No wood
    /// chart in the LUT publishes a radial condition for its chipload
    /// column; all three that state a condition state an axial one. The
    /// only vendor-published `ae` rules in the whole LUT belong to the
    /// two metal families, both at or above 0.5 D.
    ///
    /// Kept on the record because the operator should be able to see
    /// what the row claims, and because its absence still classifies the
    /// bounds source (`ChipBoundsSource::VendorLutMissingAe`). Nothing
    /// computes with it.
    pub ae_window_mm: Option<(f64, f64)>,
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

/// **Stage 3 — achieved vs commanded feed.**
///
/// The **only** multiplier between stage 1 and stage 4: the substitution
/// `tool_load::effective_feed_for_sample` performs when the trace
/// carries a kinematics-predicted feed map (F-035). On a corner-heavy
/// 3D path this routinely reads far below 1.0 — the live B3 op ran at
/// 0.128, i.e. −87 %.
///
/// Before 2026-08-06 there was a second multiplier here, a chip-geometry
/// factor. It is gone; see the module header.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AchievedFeedStage {
    /// True when the trace carried a populated predicted-feed map. When
    /// false the ratio is 1.0 *by absence of data*, not by measurement.
    pub predicted_feeds_present: bool,
    /// Median `predicted / commanded` over the sample set the gate
    /// measured. `None` when no predicted feeds were available.
    pub median_ratio: Option<f64>,
}

/// **Stage 4 — what the gate reported.**
///
/// Unit is [`ADVANCE_PER_TOOTH`], the same as stages 1 and 2:
/// `effective_feed / (rpm · flutes)`, i.e. stage 1 evaluated at stage
/// 3's achieved feed. Census F-1 — the gate comparing a chip thickness
/// against a band of advance — is closed by deletion (module header).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GateObservationStage {
    pub statistic: ObservedStatistic,
    pub value_mm: f64,
    /// Steady-state samples that entered the gate's population.
    ///
    /// Still gated on the sample carrying a resolvable chip-thickness
    /// reading, which the observation itself no longer uses — see
    /// `tool_load::chipload`'s note on the vestigial validity predicate.
    pub sample_count: usize,
}

impl GateObservationStage {
    #[must_use]
    pub const fn unit(&self) -> &'static str {
        ADVANCE_PER_TOOTH
    }
}

/// One operation's feed story, stage by stage, with every stage's unit
/// named. See the module header for the rule this type is written under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedExplanation {
    pub commanded: CommandedStage,
    pub band: LutBandStage,
    pub achieved_feed: AchievedFeedStage,
    pub gate: GateObservationStage,
}

impl FeedExplanation {
    /// Stage 1 ÷ stage 2's maximum — the **commanded** operation against
    /// the authored band, the comparison nothing surfaced before (census
    /// P-10 / T1.5).
    ///
    /// Both sides are a linear advance per tooth, so a value of 7.8
    /// means the operation is commanded to advance 7.8× further per
    /// tooth than the matched vendor row's published maximum. `None`
    /// when the band maximum is not positive.
    ///
    /// Since 2026-08-06 this is no longer the record's *only* legitimate
    /// ratio — [`Self::gate_over_band_max`] is the same comparison at the
    /// achieved feed, and the two differ by exactly stage 3. Reporting
    /// both, labelled, is what the verdict document calls the
    /// operator-actionable fact: the kinematic throttle between them.
    #[must_use]
    pub fn commanded_over_band_max(&self) -> Option<f64> {
        if self.band.max_mm_per_tooth > 0.0 {
            Some(self.commanded.feed_per_tooth_mm / self.band.max_mm_per_tooth)
        } else {
            None
        }
    }

    /// Stage 4 ÷ stage 2's maximum — what the operation **achieved**
    /// against the same band. `None` when the band maximum is not
    /// positive or the gate produced no finite observation.
    #[must_use]
    pub fn gate_over_band_max(&self) -> Option<f64> {
        if self.band.max_mm_per_tooth > 0.0 && self.gate.value_mm.is_finite() {
            Some(self.gate.value_mm / self.band.max_mm_per_tooth)
        } else {
            None
        }
    }

    /// Stage 1 × stage 3 — what stage 4 must read if the achieved-feed
    /// ratio fully explains the delta between commanded and observed.
    ///
    /// Since the chip-geometry stage was deleted this identity has one
    /// multiplier instead of two, so it is always computable: an absent
    /// predicted-feed map means a ratio of 1.0 *by absence of data*, and
    /// [`AchievedFeedStage::predicted_feeds_present`] is how a reader
    /// tells that apart from a measured 1.0.
    ///
    /// This is a *prediction of* stage 4, never a replacement for it:
    /// the record always reports the gate's own observation as stage 4.
    #[must_use]
    pub fn predicted_gate_observation_mm(&self) -> Option<f64> {
        let feed_ratio = self.achieved_feed.median_ratio.unwrap_or(1.0);
        Some(self.commanded.feed_per_tooth_mm * feed_ratio)
    }

    /// The multiplier between the commanded number an operator set and
    /// the number the verdict quotes, as a human-readable clause. Used
    /// by the diagnostic adapter so a report never states stage 4
    /// without stating what separates it from stage 1.
    #[must_use]
    pub fn multiplier_clause(&self) -> String {
        match self.achieved_feed.median_ratio {
            Some(r) => format!("×{r:.4} achieved/commanded feed"),
            None => "×1.0 feed (no predicted-feed map)".to_owned(),
        }
    }
}
