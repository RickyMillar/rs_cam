//! Boundary newtypes for the three feeds quantities that share one word.
//!
//! **Why this module exists.** Until 2026-08-08 the viewport heat-map,
//! two sim-timeline tracks and the Cut-Metrics panel all printed or
//! coloured by an *arc-mean chip thickness* while comparing it to a
//! vendor band published as a *linear advance per tooth*. Nothing in the
//! type system objected, because all four quantities were `f64`. The
//! census that found it is
//! `planning/review_2026-08-08/HEATMAP_VOCAB_CENSUS.md`; the ruling that
//! fixed it is Checkpoint H in that directory's `ORCHESTRATION_LOG.md`.
//!
//! This is deliberately **not** a units framework. It wraps exactly the
//! five values that cross the gate → graph → UI boundary, with one
//! property to defend:
//!
//! > handing an [`ArcMeanChipThicknessMm`] to a band comparison or a
//! > colour scale that wants an [`AdvancePerToothMm`] must not compile.
//!
//! There is deliberately **no** conversion between the two. None exists:
//! `CHIPLOAD_LITERATURE_VERDICT.md` §2.3 established that no wood chart
//! in the shipped LUT publishes an engagement condition for its chipload
//! column, so a chip thickness cannot be converted into the band's unit
//! from any source. That is the same reasoning that made the 2026-08-06
//! gate correction a deletion rather than an inversion.
//!
//! ## The three names
//!
//! Spelled once here so every renderer quotes the same string rather
//! than each site inventing its own wording (Checkpoint H2):
//!
//! | constant | quantity | expression | arc-sensitive? |
//! |---|---|---|---|
//! | [`COMMANDED_ADVANCE_PER_TOOTH`] | what the operator asked for | `feed / (rpm · flutes)` | no |
//! | [`ACHIEVED_ADVANCE_PER_TOOTH`] | what the machine reaches | `effective_feed / (rpm · flutes)` | no |
//! | [`ARC_MEAN_CHIP_THICKNESS`] | cut geometry | `chip_geometry(..).mean_chip_thickness_mm` | **yes** |
//!
//! The vendor band is published in the first two. The post-simulation
//! chipload gate observes the second. The word *chipload* survives on
//! rendered surfaces only where a vendor band is being named, because
//! that is the vendors' own column heading — see [`VendorChiploadBand`].
//!
//! The stage-labelled record in [`crate::feeds::explanation`] is the
//! prose sibling of this module: it explains one gate observation in
//! stages, this types the values so two stages cannot be silently
//! swapped. `ADVANCE_PER_TOOTH` there and [`AdvancePerToothMm`] here name
//! the same unit.

use std::ops::Range;

use serde::{Deserialize, Serialize};

/// Canonical rendered name for `feed / (rpm · flutes)` at the commanded
/// feed — Suggest's output, the modal's recommendation, the panel row.
pub const COMMANDED_ADVANCE_PER_TOOTH: &str = "Commanded advance/tooth";

/// Canonical rendered name for `effective_feed / (rpm · flutes)` — what
/// the machine actually reaches after the F-035 kinematics substitution,
/// and what the chipload gate observes.
pub const ACHIEVED_ADVANCE_PER_TOOTH: &str = "Achieved advance/tooth";

/// Canonical rendered name for the dexel-measured arc-average chip
/// thickness. **Never** compared against a vendor band: no source
/// publishes one in this unit.
pub const ARC_MEAN_CHIP_THICKNESS: &str = "Arc-mean chip thickness";

/// Unit suffix shared by the two advance quantities and by the band.
pub const ADVANCE_PER_TOOTH_UNIT: &str = "mm/tooth";

/// A feed rate the operator (or Suggest) commanded, mm/min.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CommandedFeedMmMin(f64);

impl CommandedFeedMmMin {
    #[must_use]
    pub const fn new(mm_min: f64) -> Self {
        Self(mm_min)
    }

    #[must_use]
    pub const fn mm_min(self) -> f64 {
        self.0
    }
}

/// The feed the machine is predicted to actually reach on a move, mm/min
/// — the F-035 kinematics substitution
/// ([`crate::tool_load::effective_feed_for_sample`]). Equal to the
/// commanded feed when no prediction is available, which is why it is a
/// *different type* rather than a flag: the caller must say which one it
/// holds.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct AchievedFeedMmMin(f64);

impl AchievedFeedMmMin {
    #[must_use]
    pub const fn new(mm_min: f64) -> Self {
        Self(mm_min)
    }

    #[must_use]
    pub const fn mm_min(self) -> f64 {
        self.0
    }
}

/// Linear advance of the cutter per tooth, mm. The unit every vendor
/// chipload column in the shipped LUT is published in
/// (`CHIPLOAD_LITERATURE_VERDICT.md` §2, verified per source family).
///
/// Construct it from a feed so the divisor is applied in one place; the
/// two constructors differ only in which feed they accept, and that is
/// the whole point.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct AdvancePerToothMm(f64);

impl AdvancePerToothMm {
    /// Wrap a value already known to be an advance per tooth — e.g. a
    /// vendor LUT bound, or a stage of [`crate::feeds::FeedExplanation`].
    #[must_use]
    pub const fn new(mm: f64) -> Self {
        Self(mm)
    }

    /// `commanded_feed / (rpm · flutes)`. `None` when the divisor is not
    /// positive, which is a broken sample rather than a modelling limit.
    #[must_use]
    pub fn from_commanded(feed: CommandedFeedMmMin, rpm: u32, flutes: u32) -> Option<Self> {
        Self::divide(feed.mm_min(), rpm, flutes)
    }

    /// `effective_feed / (rpm · flutes)` — the gate's observation.
    #[must_use]
    pub fn from_achieved(feed: AchievedFeedMmMin, rpm: u32, flutes: u32) -> Option<Self> {
        Self::divide(feed.mm_min(), rpm, flutes)
    }

    fn divide(feed_mm_min: f64, rpm: u32, flutes: u32) -> Option<Self> {
        let divisor = f64::from(rpm) * f64::from(flutes);
        (divisor > 0.0).then(|| Self(feed_mm_min / divisor))
    }

    #[must_use]
    pub const fn mm(self) -> f64 {
        self.0
    }
}

/// Arc-average chip thickness measured by the dexel simulator, mm.
///
/// **Has no conversion to [`AdvancePerToothMm`] and must not acquire
/// one.** It is a legitimate engagement/force signal; it is not a
/// quantity any shipped vendor band is expressed in. Its one surviving
/// visual (the sim-timeline track) is drawn unbanded for that reason —
/// a shaded envelope is a comparison, and there is no source for this
/// comparison.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ArcMeanChipThicknessMm(f64);

impl ArcMeanChipThicknessMm {
    #[must_use]
    pub const fn new(mm: f64) -> Self {
        Self(mm)
    }

    #[must_use]
    pub const fn mm(self) -> f64 {
        self.0
    }
}

/// Where an observed advance per tooth falls relative to a vendor band.
///
/// Five classes, transcribed unchanged from the viewport's shipped
/// colour thresholds (`rs_cam_viz::render::toolpath_render`, verified
/// 2026-08-08). Nothing here moved when the *measure* changed; only the
/// quantity fed into it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChiploadBandClass {
    /// `< min` — under-engaged, rubbing risk.
    BelowBand,
    /// `[min, 1.1·min)` — just inside the floor.
    JustAboveFloor,
    /// `[1.1·min, 0.9·max)` — comfortably in band.
    Within,
    /// `[0.9·max, max]` — approaching the ceiling.
    NearCeiling,
    /// `> max` — above the ceiling, breakage risk.
    AboveBand,
}

/// A matched vendor LUT row's chipload window, in advance per tooth.
///
/// This is the one place the word *chipload* is kept on purpose: it is
/// the vendors' own column heading, and [`Self::render_label`] spells the
/// unit out so the reader is never left to guess which of the three
/// quantities the band applies to.
///
/// Mirrors the `Range<f64>` that
/// [`crate::tool_load::chipload_envelopes_for_session`] returns — that
/// function uses `ChiploadBoundPolicy::RequireBoth`, so both bounds
/// always exist on the display path.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VendorChiploadBand {
    min: AdvancePerToothMm,
    max: AdvancePerToothMm,
}

impl VendorChiploadBand {
    #[must_use]
    pub const fn new(min: AdvancePerToothMm, max: AdvancePerToothMm) -> Self {
        Self { min, max }
    }

    /// Adopt a `Range<f64>` from
    /// [`crate::tool_load::chipload_envelopes_for_session`]. The range's
    /// endpoints are advance per tooth; this asserts that in the type
    /// system rather than in a comment at the call site.
    #[must_use]
    pub const fn from_advance_range(range: &Range<f64>) -> Self {
        Self {
            min: AdvancePerToothMm::new(range.start),
            max: AdvancePerToothMm::new(range.end),
        }
    }

    #[must_use]
    pub const fn min(self) -> AdvancePerToothMm {
        self.min
    }

    #[must_use]
    pub const fn max(self) -> AdvancePerToothMm {
        self.max
    }

    /// Classify an observation. Thresholds transcribed verbatim from the
    /// shipped viewport colour function; see [`ChiploadBandClass`].
    #[must_use]
    pub fn classify(self, observed: AdvancePerToothMm) -> ChiploadBandClass {
        let (lo, hi) = (self.min.mm(), self.max.mm());
        let v = observed.mm();
        if v < lo {
            ChiploadBandClass::BelowBand
        } else if v < lo * 1.1 {
            ChiploadBandClass::JustAboveFloor
        } else if v < hi * 0.9 {
            ChiploadBandClass::Within
        } else if v <= hi {
            ChiploadBandClass::NearCeiling
        } else {
            ChiploadBandClass::AboveBand
        }
    }

    /// Position within the [`ChiploadBandClass::JustAboveFloor`] window,
    /// `0.0` at the floor and `1.0` at `1.1 × floor`. Only meaningful for
    /// that class; clamped elsewhere.
    #[must_use]
    pub fn floor_blend_fraction(self, observed: AdvancePerToothMm) -> f64 {
        let lo = self.min.mm();
        ((observed.mm() - lo) / (lo * 0.1).max(1e-9)).clamp(0.0, 1.0)
    }

    /// The observation as a fraction of the band ceiling — `1.0` reads
    /// "at the limit". `None` when the ceiling is not positive.
    #[must_use]
    pub fn fraction_of_ceiling(self, observed: AdvancePerToothMm) -> Option<f64> {
        let hi = self.max.mm();
        (hi > 0.0).then(|| observed.mm() / hi)
    }

    /// The band as an operator-facing string, with the unit spelled out.
    /// e.g. `"0.0320–0.0550 mm/tooth"`.
    #[must_use]
    pub fn render_label(self) -> String {
        format!(
            "{:.4}\u{2013}{:.4} {ADVANCE_PER_TOOTH_UNIT}",
            self.min.mm(),
            self.max.mm()
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn band() -> VendorChiploadBand {
        VendorChiploadBand::from_advance_range(&(0.032..0.055))
    }

    /// The five classes reproduce the shipped viewport thresholds at the
    /// exact boundary values the renderer's own unit tests use.
    #[test]
    fn classes_match_the_shipped_viewport_thresholds() {
        let b = band();
        let at = AdvancePerToothMm::new;
        assert_eq!(b.classify(at(0.020)), ChiploadBandClass::BelowBand);
        assert_eq!(b.classify(at(0.033)), ChiploadBandClass::JustAboveFloor);
        assert_eq!(b.classify(at(0.040)), ChiploadBandClass::Within);
        assert_eq!(b.classify(at(0.052)), ChiploadBandClass::NearCeiling);
        assert_eq!(b.classify(at(0.060)), ChiploadBandClass::AboveBand);
        // Boundaries: `< min` is below, `== min` is not.
        assert_eq!(b.classify(at(0.032)), ChiploadBandClass::JustAboveFloor);
        assert_eq!(b.classify(at(0.055)), ChiploadBandClass::NearCeiling);
    }

    #[test]
    fn advance_per_tooth_divides_by_rpm_times_flutes() {
        let a = AdvancePerToothMm::from_commanded(CommandedFeedMmMin::new(2520.0), 18_000, 2)
            .expect("positive divisor");
        assert!((a.mm() - 0.070).abs() < 1e-12);
        let b = AdvancePerToothMm::from_achieved(AchievedFeedMmMin::new(2520.0 * 0.62), 18_000, 2)
            .expect("positive divisor");
        assert!((b.mm() - 0.0434).abs() < 1e-12);
    }

    #[test]
    fn a_sample_that_cannot_state_its_spindle_yields_none() {
        assert!(AdvancePerToothMm::from_achieved(AchievedFeedMmMin::new(1000.0), 0, 2).is_none());
        assert!(
            AdvancePerToothMm::from_commanded(CommandedFeedMmMin::new(1000.0), 18_000, 0).is_none()
        );
    }

    #[test]
    fn fraction_of_ceiling_reads_one_at_the_limit() {
        let b = band();
        let f = b
            .fraction_of_ceiling(AdvancePerToothMm::new(0.055))
            .expect("positive ceiling");
        assert!((f - 1.0).abs() < 1e-12);
    }

    #[test]
    fn render_label_spells_the_unit() {
        assert_eq!(band().render_label(), "0.0320\u{2013}0.0550 mm/tooth");
    }
}
