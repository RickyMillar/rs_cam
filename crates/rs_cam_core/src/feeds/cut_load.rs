//! Feeds contract P29-1: THE load of one cut at one operating point.
//!
//! [`CutLoad`] gives the four load values of one [`Engagement`] at one
//! [`OperatingPoint`]: the spindle power, the engaged-edge force (mean and
//! peak), the tip deflection and the peak chip. Each accessor calls one
//! model function, and no other code calls these functions for a forward
//! read:
//!
//! | Value | Model function |
//! |---|---|
//! | [`CutLoad::power_kw`] | `tool_load::power::PowerTerms::of(..).kw_at_feed(..)` |
//! | [`CutLoad::force`] | [`Engagement::edge_force`] |
//! | [`CutLoad::deflection_mm`] | [`crate::feeds::predict::tip_deflection_at`] |
//! | [`CutLoad::chip`] | [`RadialThinning::lift`] |
//!
//! The accessors are lazy and hold no cache. `edge_force` samples each
//! rotation of one flute pitch, and a gate reads one value for each of its
//! samples (up to 1e6 samples for one operation). An eager record would add
//! an edge sum to each power sample and each chip sample.
//!
//! A value that the inputs cannot give is a [`NoNumber`] with its reason,
//! never a zero.
//!
//! An inverse read stays outside this type: there the feed or the depth is
//! the unknown, so no operating point exists
//! (`Engagement::fz_cap_for_peak_force`, `PowerTerms::feed_for_kw`,
//! `force_budget::depth_at_budget`, `cutter_constraints`, the feed that
//! Step 5 sizes from the lift).
//!
//! P29-3: [`CutLoad::of_cut`] reads a [`Cut`]. Its engagement is a side cut
//! (as [`CutLoad::of`]), the straight plunge of a flat end mill
//! (`PowerModelInputs::axial_plunge`: power only; force, deflection and
//! chip are [`NoNumber::NotModelled`]), or no model (every value is
//! [`NoNumber::NotModelled`]). The two plunge doors (the calculator and
//! the operating-point figure) read it.
//!
//! # Open items of P29 (named here, not changed)
//!
//! - **M4, two depths for one sample.** `Engagement::from_sample` reads
//!   `axial_doc_mm`; `tool_load::deflection::sample_engagement` reads
//!   `axial_engagement_mm`. `CutLoad` takes the engagement that the caller
//!   built.
//!
//! P29-11 commit 11-1 closed the open item "the operating point differs by
//! door": every load door of a sample reads [`OperatingPoint::of_sample`],
//! with the flute count of the tool.
//!
//! The sentry is `tests/one_cut_gives_one_load_on_every_door_p29_1.rs`.

use crate::feeds::engagement::{EdgeForce, Engagement};
use crate::feeds::geometry::RadialThinning;
use crate::limits::{Cut, CutEngagement};
use crate::material::Material;
use crate::material::force_line::ForceLine;
use crate::stock::simulation_cut::SimulationCutSample;
use crate::tool::ToolDefinition;
use crate::tool_load::power::PowerTerms;

/// Why a [`CutLoad`] accessor gives no value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoNumber {
    /// The material has no force line (ruling B6), or the caller gave none.
    NoForceLine,
    /// The operating point holds no feed (a chip-only point).
    NoFeed,
    /// The operating point holds no spindle speed.
    NoSpeed,
    /// The caller gave no built tool: the cantilever has no body to bend.
    NoToolBody,
    /// The model function refuses the inputs (no helix, no engaged edge, no
    /// stickout, a value that is not usable).
    NotModelled,
}

/// The feed, the spindle speed, the chip and the flute count of one cut.
///
/// The fields are private. Each door of the load builds its point with the
/// named constructor of that door, so the chip rule and the flute count of
/// every door are stated in this file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OperatingPoint {
    fz_mm: f64,
    feed_mm_min: Option<f64>,
    rpm: Option<f64>,
    flute_count: u32,
}

impl OperatingPoint {
    /// A planned cut with a feed and a spindle speed (the calculator, the
    /// operating-point figure, a printed slot point). The chip is
    /// `feed / (rpm x Z)`, and `Z` is the flute count the caller plans
    /// with.
    #[must_use]
    pub fn planned(feed_mm_min: f64, rpm: f64, flute_count: u32) -> Self {
        Self {
            fz_mm: feed_mm_min / (rpm * f64::from(flute_count)),
            feed_mm_min: Some(feed_mm_min),
            rpm: Some(rpm),
            flute_count,
        }
    }

    /// A cut that states its chip and no feed: a forward read inside a
    /// solver (`cutter_constraints`, `force_budget::peak_force_n`), the
    /// wall step of a Waterline, and the deflection door of a sample, which
    /// gets its chip from [`Self::of_sample`] as an argument. `Z` is the flute count of the edge sum. The chip keeps its
    /// bits: no feed is derived from it.
    #[must_use]
    pub const fn of_chip(fz_mm: f64, flute_count: u32) -> Self {
        Self {
            fz_mm,
            feed_mm_min: None,
            rpm: None,
            flute_count,
        }
    }

    /// The point of the pre-simulation deflection predictor
    /// (`predict::predict_peak_deflection_um`).
    ///
    /// The chip is `feed / (rpm x max(Z plan, 1))` with `Z plan` the
    /// planning flute count of the tool config, and 0 when the feed is not
    /// set, the feed is not finite or the speed is not positive. The edge
    /// sum takes `Z built`, the flute count of the built tool: the
    /// predictor builds the tool after it has the chip, and then calls
    /// [`Self::with_edge_flute_count`]. The two counts are equal:
    /// `build_cutter` builds the tool with the planning flute count.
    #[must_use]
    pub fn of_predictor(feed_mm_min: Option<f64>, rpm: f64, planning_flute_count: u32) -> Self {
        let flute_count = f64::from(planning_flute_count.max(1));
        let fz_mm = match feed_mm_min {
            Some(feed) if rpm > 0.0 && flute_count > 0.0 && feed.is_finite() => {
                feed / (rpm * flute_count)
            }
            Some(_) | None => 0.0,
        };
        Self {
            fz_mm,
            feed_mm_min,
            rpm: Some(rpm),
            flute_count: planning_flute_count.max(1),
        }
    }

    /// The same point with `flute_count` as the flute count of the edge
    /// sum. The chip does not change. Only the predictor door calls it
    /// ([`Self::of_predictor`]).
    #[must_use]
    pub const fn with_edge_flute_count(self, flute_count: u32) -> Self {
        Self {
            fz_mm: self.fz_mm,
            feed_mm_min: self.feed_mm_min,
            rpm: self.rpm,
            flute_count,
        }
    }

    /// **THE operating point of one simulation sample** (P29-11 commit
    /// 11-1, `P29-11_DESIGN.md` section 1 item 3). Every load door of a
    /// sample builds its point here: the power gate
    /// (`tool_load::power::sample_power_kw`), the deflection gate and the
    /// force reading (`tool_load::deflection::deflection_feed_per_tooth_mm`)
    /// and the chip gate (`tool_load::display::peak_chip_per_tooth`).
    ///
    /// One flute-count rule: `Z` is the flute count of the TOOL, for the
    /// chip, the edge sum and the power edge term. A product sample states
    /// the same count: `compute::simulate::entry_tool_fields` writes the
    /// planning count of the tool that `build_cutter` builds, and asserts
    /// in a debug build that the two are equal.
    ///
    /// The feed is `feed_mm_min` (each door passes
    /// `tool_load::effective_feed_for_sample`), the speed is the sample's.
    /// The chip is `feed / (rpm x Z)`. With no positive divisor (no spindle
    /// speed) the chip is the sample's commanded `chipload_mm_per_tooth`.
    /// The power reads the feed and the speed, not the chip; the chip gate
    /// refuses a sample with no divisor before it builds the point
    /// (`tool_load::display::achieved_advance_per_tooth`).
    #[must_use]
    pub fn of_sample(
        sample: &SimulationCutSample,
        tool: &ToolDefinition,
        feed_mm_min: f64,
    ) -> Self {
        let flute_count = tool.flute_count;
        let divisor = f64::from(sample.spindle_rpm) * f64::from(flute_count);
        Self {
            fz_mm: if divisor > 0.0 {
                feed_mm_min / divisor
            } else {
                sample.chipload_mm_per_tooth
            },
            feed_mm_min: Some(feed_mm_min),
            rpm: Some(f64::from(sample.spindle_rpm)),
            flute_count,
        }
    }

    /// The chip per tooth (mm).
    #[must_use]
    pub const fn fz_mm(&self) -> f64 {
        self.fz_mm
    }

    /// The flute count of the edge sum and of the power edge term.
    #[must_use]
    pub const fn flute_count(&self) -> u32 {
        self.flute_count
    }
}

/// The peak chip of one cut ([`CutLoad::chip`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeakChip {
    /// The advance per tooth (mm): the chip of the operating point.
    pub advance_mm: f64,
    /// The radial chip-thinning lift at the engaged width.
    pub lift: f64,
    /// The peak chip (mm): `advance / lift`.
    pub peak_mm: f64,
}

/// The load of one cut. See the module doc.
#[derive(Clone, Copy)]
pub struct CutLoad<'a> {
    engagement: CutEngagement,
    at: OperatingPoint,
    tool: Option<&'a ToolDefinition>,
    line: Option<ForceLine>,
}

impl<'a> CutLoad<'a> {
    /// The load of `engagement` at `at`, for a built tool in `material`.
    /// The force line is `Material::force_line`; a material with none gives
    /// [`NoNumber::NoForceLine`] on every value but the chip.
    #[must_use]
    pub fn of(
        engagement: &Engagement,
        at: OperatingPoint,
        tool: &'a ToolDefinition,
        material: &Material,
    ) -> Self {
        Self::of_line(engagement, at, Some(tool), material.force_line().ok())
    }

    /// The load of `engagement` at `at` for a caller that holds the force
    /// line: a planned cut of a tool hint (no built tool, so no
    /// deflection), or a gate loop that read the line once.
    #[must_use]
    pub const fn of_line(
        engagement: &Engagement,
        at: OperatingPoint,
        tool: Option<&'a ToolDefinition>,
        line: Option<ForceLine>,
    ) -> Self {
        Self {
            engagement: CutEngagement::Side(*engagement),
            at,
            tool,
            line,
        }
    }

    /// The load of `cut` (P29-3), for a caller that holds the force line.
    ///
    /// # Errors
    ///
    /// [`NoNumber::NoFeed`] when the cut has no operating point.
    pub const fn of_cut(
        cut: &Cut,
        tool: Option<&'a ToolDefinition>,
        line: Option<ForceLine>,
    ) -> Result<Self, NoNumber> {
        match cut.point {
            Some(at) => Ok(Self {
                engagement: cut.engagement,
                at,
                tool,
                line,
            }),
            None => Err(NoNumber::NoFeed),
        }
    }

    /// The side-cut engagement, or [`NoNumber::NotModelled`] for a cut
    /// whose force, deflection and chip have no model.
    const fn side(&self) -> Result<&Engagement, NoNumber> {
        match &self.engagement {
            CutEngagement::Side(engagement) => Ok(engagement),
            CutEngagement::AxialPlunge { .. } | CutEngagement::NoModel(_) => {
                Err(NoNumber::NotModelled)
            }
        }
    }

    /// The two terms of the power model at the speed of the point: the
    /// shear slope and the edge floor. A caller that reads the power at
    /// more than one feed takes the terms.
    ///
    /// # Errors
    ///
    /// [`NoNumber::NoForceLine`], [`NoNumber::NoSpeed`].
    pub(crate) fn power_terms(&self) -> Result<PowerTerms, NoNumber> {
        let line = self.line.ok_or(NoNumber::NoForceLine)?;
        let rpm = self.at.rpm.ok_or(NoNumber::NoSpeed)?;
        match self.engagement {
            CutEngagement::Side(engagement) => Ok(Self::power_terms_at(
                &engagement,
                line,
                rpm,
                self.at.flute_count,
            )),
            // The one forward read of a straight plunge (P20d): the axial
            // circle, or the ring outside the core. The variant holds a
            // flat end only (`CutEngagement::axial_plunge`).
            CutEngagement::AxialPlunge {
                diameter_mm,
                core_diameter_mm,
            } => crate::feeds::engagement::PowerModelInputs::axial_plunge(
                crate::feeds::ToolGeometryHint::Flat,
                diameter_mm,
                core_diameter_mm,
                line,
                rpm,
                f64::from(self.at.flute_count),
            )
            .map(PowerTerms::of)
            .map_err(|refusal| match refusal {
                crate::feeds::section::AxialSectionRefusal::TipShapeNotModelled => {
                    NoNumber::NotModelled
                }
            }),
            CutEngagement::NoModel(_) => Err(NoNumber::NotModelled),
        }
    }

    /// The two terms of the power model of `engagement` at `rpm` for
    /// `flute_count` flutes: the one call of `PowerTerms::of` for a side
    /// cut. [`Self::power_terms`] calls it; the Step 6 ladder calls it too,
    /// because it reads the terms at more than one feed and holds no
    /// operating point.
    #[must_use]
    pub(crate) fn power_terms_at(
        engagement: &Engagement,
        line: ForceLine,
        rpm: f64,
        flute_count: u32,
    ) -> PowerTerms {
        PowerTerms::of(engagement.power_inputs(line, rpm, f64::from(flute_count)))
    }

    /// The spindle power (kW) at the feed of the point:
    ///
    /// ```text
    /// P = A (Ks x section x feed + F_edge x Z x n x J) / 6e7
    /// ```
    ///
    /// An edge input that is not usable gives no edge term, and the shear
    /// term stands (`PowerTerms::of`).
    ///
    /// # Errors
    ///
    /// [`NoNumber::NoFeed`], and the errors of [`Self::power_terms`].
    pub fn power_kw(&self) -> Result<f64, NoNumber> {
        let feed = self.at.feed_mm_min.ok_or(NoNumber::NoFeed)?;
        Ok(self.power_terms()?.kw_at_feed(feed))
    }

    /// The engaged-edge force (N) at the chip of the point: the mean
    /// tangential force and the peak resultant.
    ///
    /// # Errors
    ///
    /// [`NoNumber::NoForceLine`], [`NoNumber::NotModelled`] when the edge
    /// sum refuses.
    pub fn force(&self) -> Result<EdgeForce, NoNumber> {
        let line = self.line.ok_or(NoNumber::NoForceLine)?;
        self.side()?
            .edge_force(line, self.at.fz_mm, self.at.flute_count)
            .ok_or(NoNumber::NotModelled)
    }

    /// The tip deflection (mm) under the peak engaged-edge force.
    ///
    /// # Errors
    ///
    /// [`NoNumber::NoToolBody`], [`NoNumber::NoForceLine`],
    /// [`NoNumber::NotModelled`] when the beam or the edge sum refuses.
    pub fn deflection_mm(&self) -> Result<f64, NoNumber> {
        let tool = self.tool.ok_or(NoNumber::NoToolBody)?;
        let line = self.line.ok_or(NoNumber::NoForceLine)?;
        crate::feeds::predict::tip_deflection_at(
            tool,
            line,
            self.side()?,
            self.at.fz_mm,
            self.at.flute_count,
        )
        .ok_or(NoNumber::NotModelled)
    }

    /// The peak chip (mm) under the thinning rule `thinning`: the advance
    /// of the point over the lift at the engaged width.
    ///
    /// # Errors
    ///
    /// [`NoNumber::NotModelled`] for a cut that is not a side cut.
    pub fn chip(&self, thinning: RadialThinning) -> Result<PeakChip, NoNumber> {
        let advance_mm = self.at.fz_mm;
        let lift = Self::thinning_lift(self.side()?, thinning);
        Ok(PeakChip {
            advance_mm,
            lift,
            peak_mm: advance_mm / lift,
        })
    }

    /// The radial chip-thinning lift of `engagement` under `thinning`
    /// ([`RadialThinning::lift`] at the engaged width and diameter). A
    /// caller with an engagement and no operating point reads the lift
    /// here.
    #[must_use]
    pub fn thinning_lift(engagement: &Engagement, thinning: RadialThinning) -> f64 {
        thinning.lift(engagement.ae_mm(), engagement.d_eng_mm())
    }
}
