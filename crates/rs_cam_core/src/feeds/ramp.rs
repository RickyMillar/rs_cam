//! G6 ramp: the sourced feed of a helix or ramp entry
//! (`planning/extrapolation_2026-09-24/RAMP_PLAN.md`, ruling of 2026-09-25).
//!
//! The ramp feed is `min(cut feed, axial chip x RPM x Z / tan θ)`. The axial
//! chip comes from the G6 drill claim (`extrapolation::drill`): the tool's
//! printed Amana Spektra side row / Z. Every other cell writes `None`, and
//! the entry then runs at the plunge rate. The record states why.
//!
//! The value has two parts:
//!
//! 1. [`RampBasis`] is the sourced half. It does not use θ. `calculate`
//!    sets it on `FeedsResult::ramp` (step 8b).
//! 2. [`RampFeed`] is the number. It needs θ, the shipped feed and the
//!    shipped RPM, so the apply funnel (`suggest::apply`) resolves it with
//!    [`resolve_ramp_feed`] after the invariant passes.
//!
//! There are two θ systems. A ramp slope is `tan(angle)`. A helix slope is
//! `pitch / (2π r)`. The guards are those of `dressup::entry_descent`: a
//! ramp angle in (0, 90) degrees, a helix radius above zero and a pitch of
//! at least 0.01 mm.
//!
//! The rule divides by tan θ, not by sin θ. The machine feed is the path
//! speed, so its vertical part is F x sin θ. Division by tan θ gives a
//! value lower by a factor of cos θ (0.9986 at 3 degrees), which is
//! conservative.

use std::f64::consts::TAU;

use super::extrapolation::{Claim, DrillClaim};
use super::vendor_lut::ToolFamily;
use super::{FeedsInput, OperationFamily};
use crate::compute::catalog::OperationConfig;
use crate::compute::config::{DressupConfig, DressupEntryStyle};
use crate::compute::operation_configs::Adaptive3dEntryStyle;

/// The ramp rule in words, for the detail line.
pub const RAMP_RULE_TEXT: &str = "ramp feed = min(cut feed, axial chip x RPM x Z / tan θ); \
     tan θ is conservative against sin θ";

/// The sourced half of the ramp feed. It does not use θ.
#[derive(Debug, Clone, PartialEq)]
pub enum RampBasis {
    /// The G6 drill claim gives the axial chip per tooth.
    Sourced {
        /// The side row's chip / Z (mm/tooth), with the G2 hardness scale.
        axial_chip_mm: f64,
        /// The drill claim that reads the side row.
        drill: Box<DrillClaim>,
        /// The G1 size claim of the side row, when the row is off size.
        size: Option<Box<Claim>>,
    },
    /// No claim gives an axial chip. The entry uses the plunge rate.
    PlungeRate { reason: RampFallback },
}

impl RampBasis {
    /// The card text: a headline and a detail line.
    ///
    /// Example headline: "ramp chip (G6 drill claim): 0.0635 mm/tooth
    /// axial".
    #[must_use]
    pub fn card_text(&self) -> (String, String) {
        match self {
            Self::Sourced {
                axial_chip_mm,
                drill,
                size,
            } => {
                let headline = format!(
                    "ramp chip ({} {} claim): {axial_chip_mm:.4} mm/tooth axial",
                    drill.gap.group(),
                    drill.gap.label()
                );
                (headline, sourced_detail(drill, size.as_deref()))
            }
            Self::PlungeRate { reason } => {
                ("ramp chip: none, the plunge rate".to_owned(), reason.text())
            }
        }
    }
}

/// The detail line of a sourced chip: the claim, the row, the rule and the
/// size claim when there is one.
fn sourced_detail(drill: &DrillClaim, size: Option<&Claim>) -> String {
    let (claim, _) = drill.card_text();
    let size = size.map_or_else(String::new, |c| format!("; {}", c.card_text().0));
    format!(
        "{claim}; row {} ({}); {}; the ramp reads the chip at the shipped RPM, not at the \
         drill RPM cap{size}; {RAMP_RULE_TEXT}",
        drill.side_row, drill.source_id, drill.rule
    )
}

/// Why the entry uses the plunge rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RampFallback {
    /// A drill cycle has no helix or ramp entry.
    DrillCycle,
    /// No vendor table was given.
    NoLut,
    /// No G6 drill claim covers the tool.
    NoDrillClaim {
        tool_family: ToolFamily,
        diameter_mm: f64,
        flutes: u32,
    },
    /// The G1 size claim refused the side row.
    SizeRefused,
    /// The entry is a straight plunge.
    EntryOff,
    /// The toolpath's entry dressup is not known at the call.
    EntryUnknown,
    /// The helix or ramp slope is not valid.
    DegenerateEntry,
    /// The operation ships no spindle RPM.
    NoShippedRpm,
    /// The operation ships no positive cut feed.
    NoCutFeed,
}

/// The name of a tool family in card text.
const fn family_label(family: ToolFamily) -> &'static str {
    match family {
        ToolFamily::FlatEnd => "flat end mill",
        ToolFamily::BallNose => "ball nose",
        ToolFamily::TaperedBallNose => "tapered ball nose",
        ToolFamily::BullNose => "bull nose",
        ToolFamily::ChamferVbit => "V-bit",
        ToolFamily::FacingBit => "facing bit",
    }
}

impl RampFallback {
    /// One sentence for the card.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::DrillCycle => {
                "A drill cycle has no helix or ramp entry; the entry uses the plunge rate."
                    .to_owned()
            }
            Self::NoLut => "No vendor table was given, so no G6 claim gives an axial chip; the \
                            entry uses the plunge rate."
                .to_owned(),
            Self::NoDrillClaim {
                tool_family,
                diameter_mm,
                flutes,
            } => format!(
                "No G6 drill claim covers a {diameter_mm} mm {flutes}-flute {} (the claim covers \
                 the Amana Spektra flat end mill, 3.175-6.0 mm, 2 or 3 flutes, in wood, plywood \
                 and MDF); the entry uses the plunge rate.",
                family_label(*tool_family)
            ),
            Self::SizeRefused => "The G1 size claim refused the side row, so no axial chip is \
                                  sourced; the entry uses the plunge rate."
                .to_owned(),
            Self::EntryOff => {
                "The entry is a straight plunge, with no helix or ramp; the entry uses the \
                 plunge rate."
                    .to_owned()
            }
            Self::EntryUnknown => "The toolpath's entry dressup is not known here; the entry \
                                   uses the plunge rate."
                .to_owned(),
            Self::DegenerateEntry => "The helix or ramp slope is zero or not valid; the entry \
                                      uses the plunge rate."
                .to_owned(),
            Self::NoShippedRpm => "The operation ships no spindle RPM, so the chip term has no \
                                   rate; the entry uses the plunge rate."
                .to_owned(),
            Self::NoCutFeed => "The operation ships no positive cut feed; the entry uses the \
                                plunge rate."
                .to_owned(),
        }
    }
}

/// The configured slope of one helix or ramp entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EntryGeometry {
    /// A ramp at `angle_deg` from the horizontal.
    Ramp { angle_deg: f64 },
    /// A helix of radius `radius_mm` that descends `pitch_mm` per turn.
    Helix { radius_mm: f64, pitch_mm: f64 },
}

impl EntryGeometry {
    /// tan θ of the slope, or `None` when the entry code would not run the
    /// slope (the guards of `dressup::entry_descent`).
    ///
    /// - A ramp: `tan(angle)` for 0 < angle < 90 degrees.
    /// - A helix: `pitch / (2π r)` for r > 0 and pitch >= 0.01 mm.
    #[must_use]
    pub fn tan_theta(&self) -> Option<f64> {
        let tan = match *self {
            Self::Ramp { angle_deg } if angle_deg > 0.0 && angle_deg < 90.0 => {
                angle_deg.to_radians().tan()
            }
            Self::Helix {
                radius_mm,
                pitch_mm,
            } if radius_mm > 0.0 && pitch_mm >= 0.01 => pitch_mm / (TAU * radius_mm),
            Self::Ramp { .. } | Self::Helix { .. } => return None,
        };
        (tan.is_finite() && tan > 0.0).then_some(tan)
    }

    /// θ in degrees, or `None` when [`Self::tan_theta`] is `None`.
    #[must_use]
    pub fn theta_deg(&self) -> Option<f64> {
        self.tan_theta().map(|t| t.atan().to_degrees())
    }

    /// The entry in words, for example "ramp 3.00°" or "helix 4.55° (r 2.00
    /// mm, pitch 1.00 mm)".
    #[must_use]
    pub fn label(&self) -> String {
        let theta = self
            .theta_deg()
            .map_or_else(|| "no valid slope".to_owned(), |t| format!("{t:.2}°"));
        match *self {
            Self::Ramp { .. } => format!("ramp {theta}"),
            Self::Helix {
                radius_mm,
                pitch_mm,
            } => format!("helix {theta} (r {radius_mm:.2} mm, pitch {pitch_mm:.2} mm)"),
        }
    }
}

/// Where the entry geometry comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntrySource {
    /// The toolpath's entry dressup (`DressupConfig`).
    Dressup,
    /// The Adaptive3d operation's own entry (`Adaptive3dConfig`). The
    /// registry forces the dressup entry to none on Adaptive3d.
    Adaptive3d,
}

impl EntrySource {
    /// The source in words.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dressup => "the toolpath's entry dressup",
            Self::Adaptive3d => "the Adaptive3d entry",
        }
    }
}

/// The entry geometry of the operation that ships.
///
/// There are two θ systems: a helix slope is `pitch / (2π r)`, and a ramp
/// slope is `tan(angle)`.
///
/// - Adaptive3d reads its own `entry_style`. Plunge gives `EntryOff`. Ramp
///   gives `ramp_angle_deg`. Helix gives the radius
///   `tool_diameter_mm x helix_radius_factor` and `helix_pitch`. This is the
///   mapping of `compute::execute::finish_3d::generate_adaptive3d`.
/// - Every other operation reads `dressups`. `None` gives `EntryUnknown`.
///   `DressupEntryStyle::None` gives `EntryOff`. Ramp gives `ramp_angle`,
///   and Helix gives `helix_radius` and `helix_pitch`. This is the mapping
///   of `compute::execute::dressup_apply`.
///
/// # Errors
///
/// The [`RampFallback`] that makes the entry use the plunge rate.
pub fn entry_geometry(
    op: &OperationConfig,
    dressups: Option<&DressupConfig>,
    tool_diameter_mm: f64,
) -> Result<(EntryGeometry, EntrySource), RampFallback> {
    if let OperationConfig::Adaptive3d(cfg) = op {
        let entry = match cfg.entry_style {
            Adaptive3dEntryStyle::Plunge => return Err(RampFallback::EntryOff),
            Adaptive3dEntryStyle::Ramp => EntryGeometry::Ramp {
                angle_deg: cfg.ramp_angle_deg,
            },
            Adaptive3dEntryStyle::Helix => EntryGeometry::Helix {
                radius_mm: tool_diameter_mm * cfg.helix_radius_factor,
                pitch_mm: cfg.helix_pitch,
            },
        };
        return Ok((entry, EntrySource::Adaptive3d));
    }
    let Some(cfg) = dressups else {
        return Err(RampFallback::EntryUnknown);
    };
    let entry = match cfg.entry_style {
        DressupEntryStyle::None => return Err(RampFallback::EntryOff),
        DressupEntryStyle::Ramp => EntryGeometry::Ramp {
            angle_deg: cfg.ramp_angle,
        },
        DressupEntryStyle::Helix => EntryGeometry::Helix {
            radius_mm: cfg.helix_radius,
            pitch_mm: cfg.helix_pitch,
        },
    };
    Ok((entry, EntrySource::Dressup))
}

/// The sourced half of the ramp feed for one calculator input
/// (`calculate` step 8b).
///
/// 1. A drill cycle gives `DrillCycle`. No vendor table gives `NoLut`.
/// 2. The query is [`super::vendor_normalize::ramp_drill_query`]: the
///    tool's plunge in the drill family.
/// 3. Only a row that the drill rule reads, with a size basis that is not
///    refused and a printed point, gives `Sourced`. Its chip is the side
///    chip / Z, with the hardness scale.
/// 4. Any other row, or no row, gives `NoDrillClaim`.
#[must_use]
pub fn ramp_basis(input: &FeedsInput<'_>) -> RampBasis {
    let fallback = |reason| RampBasis::PlungeRate { reason };
    if input.operation == OperationFamily::Drill {
        return fallback(RampFallback::DrillCycle);
    }
    let Some(lut) = input.vendor_lut else {
        return fallback(RampFallback::NoLut);
    };
    let query = super::vendor_normalize::ramp_drill_query(input);
    let no_claim = RampFallback::NoDrillClaim {
        tool_family: query.tool_family,
        diameter_mm: query.diameter_mm,
        flutes: query.flute_count,
    };
    let Some(row) =
        super::vendor_lookup::find_best_row_for_geometry(lut, &query, &input.tool_geometry)
    else {
        return fallback(no_claim);
    };
    let Some(drill) = row.drill_basis.claim() else {
        return fallback(no_claim);
    };
    if row.size_basis.is_refused() {
        return fallback(RampFallback::SizeRefused);
    }
    let Some(chip) = row
        .printed_chipload()
        .point_mm()
        .filter(|c| c.is_finite() && *c > 0.0)
    else {
        return fallback(no_claim);
    };
    RampBasis::Sourced {
        axial_chip_mm: chip,
        drill: Box::new(drill.clone()),
        size: row.size_basis.claim().map(|c| Box::new(c.clone())),
    }
}

/// Which term sets the ramp feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RampArm {
    /// The cut feed is the smaller term.
    CutFeed,
    /// The chip term `axial chip x RPM x Z / tan θ` is the smaller term.
    ChipTerm,
}

/// A ramp feed from the G6 chip: every term of the rule.
#[derive(Debug, Clone, PartialEq)]
pub struct SourcedRamp {
    /// The feed that ships (mm/min), rounded down to 1 mm/min.
    pub value_mm_min: f64,
    /// The term that set it.
    pub arm: RampArm,
    /// The configured entry slope.
    pub entry: EntryGeometry,
    /// Where the entry comes from.
    pub source: EntrySource,
    pub tan_theta: f64,
    pub theta_deg: f64,
    /// The axial chip per tooth (mm).
    pub axial_chip_mm: f64,
    /// The shipped spindle RPM.
    pub rpm: f64,
    pub flutes: u32,
    /// `axial chip x RPM x Z` (mm/min).
    pub vertical_mm_min: f64,
    /// `vertical / tan θ` (mm/min).
    pub chip_term_mm_min: f64,
    /// The shipped cut feed (mm/min).
    pub cut_feed_mm_min: f64,
    pub drill: Box<DrillClaim>,
    pub size: Option<Box<Claim>>,
}

/// The ramp feed that Suggest writes, and why.
#[derive(Debug, Clone, PartialEq)]
pub enum RampFeed {
    /// The G6 chip sets the feed.
    Sourced(Box<SourcedRamp>),
    /// The entry uses the plunge rate. Suggest writes `None`.
    PlungeRate {
        reason: RampFallback,
        plunge_mm_min: f64,
    },
}

impl RampFeed {
    /// The value Suggest writes to `ramp_feed_rate`.
    #[must_use]
    pub fn value(&self) -> Option<f64> {
        match self {
            Self::Sourced(s) => Some(s.value_mm_min),
            Self::PlungeRate { .. } => None,
        }
    }

    /// The card text: the face line and a detail line.
    ///
    /// Example face line: "Ramp feed 4000 mm/min: the cut feed (chip limit
    /// 38162 mm/min at ramp 3.00°)". The plunge arm: "Ramp feed: the plunge
    /// rate, 703 mm/min: <reason>".
    #[must_use]
    pub fn card_text(&self) -> (String, String) {
        match self {
            Self::Sourced(s) => {
                let entry = s.entry.label();
                let headline = match s.arm {
                    RampArm::CutFeed => format!(
                        "Ramp feed {:.0} mm/min: the cut feed (chip limit {:.0} mm/min at {entry})",
                        s.value_mm_min, s.chip_term_mm_min
                    ),
                    RampArm::ChipTerm => format!(
                        "Ramp feed {:.0} mm/min: the chip limit at {entry} (cut feed {:.0} mm/min)",
                        s.value_mm_min, s.cut_feed_mm_min
                    ),
                };
                let detail = format!(
                    "axial chip {:.4} mm/tooth x {:.0} RPM x {} flutes = {:.0} mm/min vertical; \
                     / tan {:.2}° ({:.5}) = chip limit {:.0} mm/min; cut feed {:.0} mm/min; the \
                     entry is from {}; θ is the configured slope; {}",
                    s.axial_chip_mm,
                    s.rpm,
                    s.flutes,
                    s.vertical_mm_min,
                    s.theta_deg,
                    s.tan_theta,
                    s.chip_term_mm_min,
                    s.cut_feed_mm_min,
                    s.source.label(),
                    sourced_detail(&s.drill, s.size.as_deref()),
                );
                (headline, detail)
            }
            Self::PlungeRate {
                reason,
                plunge_mm_min,
            } => (
                format!(
                    "Ramp feed: the plunge rate, {plunge_mm_min:.0} mm/min: {}",
                    reason.text()
                ),
                RAMP_RULE_TEXT.to_owned(),
            ),
        }
    }
}

/// The ramp feed of the operation that ships.
///
/// The order of the checks:
///
/// 1. A `PlungeRate` basis gives its reason.
/// 2. An entry error gives that reason.
/// 3. No RPM, or an RPM of 0, gives `NoShippedRpm`.
/// 4. A slope with no valid tan θ gives `DegenerateEntry`.
/// 5. A cut feed that is not positive gives `NoCutFeed`.
///
/// Then `vertical = chip x RPM x Z`, `chip_term = vertical / tan θ`, and
/// the value is `min(cut, chip_term)` rounded DOWN to 1 mm/min (T-9: the
/// value is bound from above).
#[must_use]
pub fn resolve_ramp_feed(
    basis: &RampBasis,
    entry: Result<(EntryGeometry, EntrySource), RampFallback>,
    cut_feed_mm_min: f64,
    rpm: Option<u32>,
    flutes: u32,
    plunge_mm_min: f64,
) -> RampFeed {
    let fallback = |reason| RampFeed::PlungeRate {
        reason,
        plunge_mm_min,
    };
    let (axial_chip_mm, drill, size) = match basis {
        RampBasis::Sourced {
            axial_chip_mm,
            drill,
            size,
        } => (*axial_chip_mm, drill, size),
        RampBasis::PlungeRate { reason } => return fallback(*reason),
    };
    let (entry, source) = match entry {
        Ok(pair) => pair,
        Err(reason) => return fallback(reason),
    };
    let Some(rpm) = rpm.filter(|r| *r > 0).map(f64::from) else {
        return fallback(RampFallback::NoShippedRpm);
    };
    let (Some(tan_theta), Some(theta_deg)) = (entry.tan_theta(), entry.theta_deg()) else {
        return fallback(RampFallback::DegenerateEntry);
    };
    if !(cut_feed_mm_min.is_finite() && cut_feed_mm_min > 0.0) {
        return fallback(RampFallback::NoCutFeed);
    }
    let vertical_mm_min = axial_chip_mm * rpm * f64::from(flutes);
    let chip_term_mm_min = vertical_mm_min / tan_theta;
    let raw = cut_feed_mm_min.min(chip_term_mm_min);
    let value_mm_min = super::suggest::round_suggestion_value_down(raw, 1.0);
    if !(value_mm_min.is_finite() && value_mm_min > 0.0) {
        return fallback(RampFallback::NoCutFeed);
    }
    let arm = if chip_term_mm_min < cut_feed_mm_min {
        RampArm::ChipTerm
    } else {
        RampArm::CutFeed
    };
    RampFeed::Sourced(Box::new(SourcedRamp {
        value_mm_min,
        arm,
        entry,
        source,
        tan_theta,
        theta_deg,
        axial_chip_mm,
        rpm,
        flutes,
        vertical_mm_min,
        chip_term_mm_min,
        cut_feed_mm_min,
        drill: drill.clone(),
        size: size.clone(),
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationType;
    use crate::feeds::extrapolation::{ClaimConfidence, DRILL_RULE_TEXT, Gap};
    use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};

    fn claim() -> DrillClaim {
        DrillClaim {
            gap: Gap::Drill,
            rule: DRILL_RULE_TEXT,
            scale: 0.5,
            flutes: 2,
            side_row: "amana-flat-softwood-pocket-6000-2f-spektra".to_owned(),
            source_id: "amana_spektra_spiral_plunge_v24".to_owned(),
            home: (LutOperationFamily::Pocket, LutPassRole::Roughing),
            material_label: "Wood/Plywood".to_owned(),
            range_mm: 3.175..=6.0,
            printed_rpm: Some(18_000.0),
            witness: "test witness",
            confidence: ClaimConfidence::OneWitness,
        }
    }

    fn basis() -> RampBasis {
        RampBasis::Sourced {
            axial_chip_mm: 0.0635,
            drill: Box::new(claim()),
            size: None,
        }
    }

    const HELIX: EntryGeometry = EntryGeometry::Helix {
        radius_mm: 2.0,
        pitch_mm: 1.0,
    };

    fn sourced(r: &RampFeed) -> &SourcedRamp {
        match r {
            RampFeed::Sourced(s) => s,
            RampFeed::PlungeRate { reason, .. } => panic!("expected Sourced, got {reason:?}"),
        }
    }

    fn resolve(entry: EntryGeometry, cut: f64, rpm: u32) -> RampFeed {
        resolve_ramp_feed(
            &basis(),
            Ok((entry, EntrySource::Dressup)),
            cut,
            Some(rpm),
            2,
            1000.0,
        )
    }

    /// The helix slope is pitch / (2π r): r 2, pitch 1 gives 4.55 degrees.
    #[test]
    fn the_helix_slope_is_pitch_over_the_circumference() {
        let tan = HELIX.tan_theta().unwrap();
        assert!((tan - 1.0 / (TAU * 2.0)).abs() < 1e-15);
        assert!((HELIX.theta_deg().unwrap() - 4.55).abs() < 0.01);
        let ramp = EntryGeometry::Ramp { angle_deg: 3.0 };
        assert!((ramp.tan_theta().unwrap() - 3.0_f64.to_radians().tan()).abs() < 1e-15);
        assert!((ramp.theta_deg().unwrap() - 3.0).abs() < 1e-9);
    }

    /// The worked example: 6 mm 2F, softwood, 18 000 RPM, cut feed 4572.
    #[test]
    fn the_worked_example_ships_the_cut_feed() {
        let helix = resolve(HELIX, 4572.0, 18_000);
        let s = sourced(&helix);
        assert!((s.vertical_mm_min - 2286.0).abs() < 1e-9);
        assert!(
            (s.chip_term_mm_min - 28_726.0).abs() < 1.0,
            "{}",
            s.chip_term_mm_min
        );
        assert_eq!(s.arm, RampArm::CutFeed);
        assert_eq!(helix.value(), Some(4572.0));

        let ramp = resolve(EntryGeometry::Ramp { angle_deg: 3.0 }, 4572.0, 18_000);
        let s = sourced(&ramp);
        assert!(
            (s.chip_term_mm_min - 43_619.0).abs() < 1.0,
            "{}",
            s.chip_term_mm_min
        );
        assert_eq!(s.arm, RampArm::CutFeed);
        assert_eq!(ramp.value(), Some(4572.0));
        let (face, detail) = ramp.card_text();
        assert!(face.contains("Ramp feed 4572"), "{face}");
        assert!(face.contains("cut feed"), "{face}");
        assert!(face.contains("3.00°"), "{face}");
        assert!(detail.contains(RAMP_RULE_TEXT), "{detail}");
    }

    /// A 30 degree ramp at the shipped 15 748 RPM: the chip term wins.
    #[test]
    fn a_steep_ramp_ships_the_chip_term() {
        let r = resolve(EntryGeometry::Ramp { angle_deg: 30.0 }, 4000.0, 15_748);
        let s = sourced(&r);
        assert_eq!(s.arm, RampArm::ChipTerm);
        assert!(
            (s.chip_term_mm_min - 3464.1).abs() < 0.1,
            "{}",
            s.chip_term_mm_min
        );
        assert_eq!(r.value(), Some(3464.0));
    }

    /// The value rounds down: never above min(cut, chip term), and less
    /// than 1 mm/min under it.
    #[test]
    fn the_value_rounds_down() {
        for (entry, cut, rpm) in [
            (HELIX, 4572.4, 18_000),
            (EntryGeometry::Ramp { angle_deg: 30.0 }, 4000.0, 15_748),
            (EntryGeometry::Ramp { angle_deg: 45.0 }, 9999.9, 12_345),
        ] {
            let r = resolve(entry, cut, rpm);
            let s = sourced(&r);
            let bound = s.cut_feed_mm_min.min(s.chip_term_mm_min);
            assert!(s.value_mm_min <= bound, "{} > {bound}", s.value_mm_min);
            assert!(
                s.value_mm_min > bound - 1.0,
                "{} <= {bound} - 1",
                s.value_mm_min
            );
        }
    }

    /// Every fallback writes `None` and gives its reason.
    #[test]
    fn every_fallback_writes_none() {
        let no_lut = RampBasis::PlungeRate {
            reason: RampFallback::NoLut,
        };
        let cases = [
            (
                resolve_ramp_feed(
                    &no_lut,
                    Ok((HELIX, EntrySource::Dressup)),
                    4000.0,
                    Some(18_000),
                    2,
                    700.0,
                ),
                RampFallback::NoLut,
            ),
            (
                resolve_ramp_feed(
                    &basis(),
                    Err(RampFallback::EntryOff),
                    4000.0,
                    Some(18_000),
                    2,
                    700.0,
                ),
                RampFallback::EntryOff,
            ),
            (
                resolve_ramp_feed(
                    &basis(),
                    Ok((HELIX, EntrySource::Dressup)),
                    4000.0,
                    None,
                    2,
                    700.0,
                ),
                RampFallback::NoShippedRpm,
            ),
            (
                resolve_ramp_feed(
                    &basis(),
                    Ok((HELIX, EntrySource::Dressup)),
                    4000.0,
                    Some(0),
                    2,
                    700.0,
                ),
                RampFallback::NoShippedRpm,
            ),
            (
                resolve_ramp_feed(
                    &basis(),
                    Ok((EntryGeometry::Ramp { angle_deg: 0.0 }, EntrySource::Dressup)),
                    4000.0,
                    Some(18_000),
                    2,
                    700.0,
                ),
                RampFallback::DegenerateEntry,
            ),
            (
                resolve_ramp_feed(
                    &basis(),
                    Ok((
                        EntryGeometry::Helix {
                            radius_mm: 2.0,
                            pitch_mm: 0.0,
                        },
                        EntrySource::Dressup,
                    )),
                    4000.0,
                    Some(18_000),
                    2,
                    700.0,
                ),
                RampFallback::DegenerateEntry,
            ),
            (
                resolve_ramp_feed(
                    &basis(),
                    Ok((HELIX, EntrySource::Dressup)),
                    0.0,
                    Some(18_000),
                    2,
                    700.0,
                ),
                RampFallback::NoCutFeed,
            ),
        ];
        for (record, want) in cases {
            assert_eq!(record.value(), None, "{want:?}");
            match &record {
                RampFeed::PlungeRate {
                    reason,
                    plunge_mm_min,
                } => {
                    assert_eq!(*reason, want);
                    assert!((plunge_mm_min - 700.0).abs() < 1e-12);
                }
                RampFeed::Sourced(_) => panic!("{want:?}: expected the plunge rate"),
            }
            let (face, _) = record.card_text();
            assert!(face.contains("the plunge rate, 700 mm/min"), "{face}");
            assert!(face.ends_with("the entry uses the plunge rate."), "{face}");
        }
    }

    /// The two θ systems read the right fields.
    #[test]
    fn the_entry_reads_the_dressup_or_the_adaptive3d_config() {
        let pocket = OperationConfig::new_default(OperationType::Pocket);
        assert_eq!(
            entry_geometry(&pocket, None, 6.0),
            Err(RampFallback::EntryUnknown)
        );
        let off = DressupConfig {
            entry_style: DressupEntryStyle::None,
            ..DressupConfig::default()
        };
        assert_eq!(
            entry_geometry(&pocket, Some(&off), 6.0),
            Err(RampFallback::EntryOff)
        );
        let helix = DressupConfig {
            entry_style: DressupEntryStyle::Helix,
            helix_radius: 2.0,
            helix_pitch: 1.0,
            ..DressupConfig::default()
        };
        assert_eq!(
            entry_geometry(&pocket, Some(&helix), 6.0),
            Ok((HELIX, EntrySource::Dressup))
        );

        let mut a3d = OperationConfig::new_default(OperationType::Adaptive3d);
        if let OperationConfig::Adaptive3d(cfg) = &mut a3d {
            cfg.entry_style = Adaptive3dEntryStyle::Helix;
            cfg.helix_radius_factor = 0.3;
            cfg.helix_pitch = 2.0;
        }
        let (entry, source) = entry_geometry(&a3d, None, 6.0).unwrap();
        assert_eq!(source, EntrySource::Adaptive3d);
        assert!((entry.theta_deg().unwrap() - 10.03).abs() < 0.01);
        if let OperationConfig::Adaptive3d(cfg) = &mut a3d {
            cfg.entry_style = Adaptive3dEntryStyle::Plunge;
        }
        assert_eq!(
            entry_geometry(&a3d, Some(&helix), 6.0),
            Err(RampFallback::EntryOff)
        );
    }
}
