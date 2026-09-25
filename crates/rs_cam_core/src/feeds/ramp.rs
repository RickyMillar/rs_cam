//! G6 ramp: the sourced feed of a helix or ramp entry
//! (`planning/extrapolation_2026-09-24/RAMP_PLAN.md`, ruling of 2026-09-25),
//! and the G10 Q2 plunge slope (`G10_PLAN.md` §3 A5).
//!
//! The ramp feed has three arms:
//!
//! - G6: `min(cut feed, axial chip x RPM x Z / tan θ)`. The axial chip comes
//!   from the G6 drill claim (`extrapolation::drill`): the tool's printed
//!   Amana Spektra side row / Z.
//! - G10 Q2: with no G6 chip, the ramp holds its vertical rate at the
//!   plunge: `min(cut feed, plunge / tan θ)`. This is a repo rule with no
//!   source, and the card says so.
//! - No entry feed: a straight plunge, an unknown entry, a slope with no
//!   valid θ, no cut feed or no plunge. Suggest writes `None`, and the entry
//!   runs at the plunge rate. The record states why.
//!
//! [`entry_notes`] states the entry rules of the operation that ships (G10
//! Q6-Q10, Q12): the angle, the helix, the pip or the core, the clearance,
//! and the straight-plunge caution.
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

use super::extrapolation::{Claim, DrillClaim, tool_family_label};
use super::vendor_lut::ToolFamily;
use super::{FeedsInput, OperationFamily, PassRole};
use crate::compute::catalog::OperationConfig;
use crate::compute::config::{
    DressupConfig, DressupEntryStyle, HELIX_RADIUS_OVER_D, HelixRadius, default_entry_clearance_mm,
};
use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
use crate::compute::tool_config::ToolConfig;
use crate::tool::MillingCutter;

/// The ramp rule in words, for the detail line.
pub const RAMP_RULE_TEXT: &str = "ramp feed = min(cut feed, axial chip x RPM x Z / tan θ); \
     tan θ is conservative against sin θ";

/// The G10 Q2 rule in words, for the detail line of a plunge-slope ramp.
pub const PLUNGE_SLOPE_RULE_TEXT: &str = "no source; vertical rate = plunge (repo rule)";

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
    /// No claim gives an axial chip. The ramp holds its vertical rate at
    /// the plunge (G10 Q2), or the entry uses the plunge rate.
    NoChip { reason: RampFallback },
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
            Self::NoChip { reason } => (
                "ramp chip: none; the ramp holds its vertical rate at the plunge".to_owned(),
                reason.text(),
            ),
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

/// Why no G6 chip sets the ramp, or why the entry has no ramp feed.
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
    /// The operation ships no positive plunge, so the ramp has no vertical
    /// rate (G10 Q2).
    NoPlunge,
}

impl RampFallback {
    /// One sentence for the card.
    ///
    /// A reason that leaves no G6 chip (no vendor table, no claim, a
    /// refused size, no RPM) ends "so the ramp holds its vertical rate at
    /// the plunge" (G10 Q2). A reason that leaves no entry feed ends "the
    /// entry uses the plunge rate".
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::DrillCycle => {
                "A drill cycle has no helix or ramp entry; the entry uses the plunge rate."
                    .to_owned()
            }
            Self::NoLut => "No vendor table was given, so no G6 claim gives an axial chip, so \
                            the ramp holds its vertical rate at the plunge."
                .to_owned(),
            Self::NoDrillClaim {
                tool_family,
                diameter_mm,
                flutes,
            } => format!(
                "No G6 drill claim covers a {diameter_mm} mm {flutes}-flute {} (the claim covers \
                 the Amana Spektra flat end mill, 3.0-12.7 mm, 2 or 3 flutes, in wood, plywood \
                 and MDF), so the ramp holds its vertical rate at the plunge.",
                tool_family_label(*tool_family)
            ),
            Self::SizeRefused => "The G1 size claim refused the side row, so no axial chip is \
                                  sourced, so the ramp holds its vertical rate at the plunge."
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
                                   rate, so the ramp holds its vertical rate at the plunge."
                .to_owned(),
            Self::NoCutFeed => "The operation ships no positive cut feed; the entry uses the \
                                plunge rate."
                .to_owned(),
            Self::NoPlunge => "The operation ships no positive plunge, so the ramp has no \
                               vertical rate; the entry uses the plunge rate."
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

/// The entry of the operation that ships: its slope, where it comes from,
/// and where it starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entry {
    /// The configured slope.
    pub geometry: EntryGeometry,
    /// Where the slope comes from.
    pub source: EntrySource,
    /// The height (mm) above the material top where the helix or ramp
    /// starts (`DressupConfig::entry_clearance_mm` or
    /// `Adaptive3dConfig::entry_clearance_mm`).
    pub clearance_mm: f64,
}

/// The entry geometry of the operation that ships.
///
/// There are two θ systems: a helix slope is `pitch / (2π r)`, and a ramp
/// slope is `tan(angle)`.
///
/// - Adaptive3d reads its own `entry_style`. Plunge gives `EntryOff`. Ramp
///   gives `ramp_angle_deg`. Helix gives the radius
///   `Adaptive3dConfig::helix_radius_for` emits (D x `helix_radius_factor`,
///   capped at the flat bottom) and `helix_pitch`. This is the mapping of
///   `compute::execute::finish_3d::generate_adaptive3d`.
/// - Every other operation reads `dressups`. `None` gives `EntryUnknown`.
///   `DressupEntryStyle::None` gives `EntryOff`. Ramp gives `ramp_angle`,
///   and Helix gives the radius `DressupConfig::helix_radius_for` emits
///   (the operator value or 0.3 x D, capped at the flat bottom) and
///   `helix_pitch`. This is the mapping of `compute::execute::dressup_apply`.
///
/// θ is the slope of the helix the engine emits, so the ramp feed and the
/// card read the capped radius (G10 Q6).
///
/// # Errors
///
/// The [`RampFallback`] that makes the entry use the plunge rate.
pub fn entry_geometry(
    op: &OperationConfig,
    dressups: Option<&DressupConfig>,
    tool: &ToolConfig,
) -> Result<Entry, RampFallback> {
    let cutter = crate::compute::cutter::build_cutter(tool);
    if let OperationConfig::Adaptive3d(cfg) = op {
        let geometry = match cfg.entry_style {
            Adaptive3dEntryStyle::Plunge => return Err(RampFallback::EntryOff),
            Adaptive3dEntryStyle::Ramp => EntryGeometry::Ramp {
                angle_deg: cfg.ramp_angle_deg,
            },
            Adaptive3dEntryStyle::Helix => EntryGeometry::Helix {
                radius_mm: cfg.helix_radius_for(&cutter).emitted_mm,
                pitch_mm: cfg.helix_pitch,
            },
        };
        return Ok(Entry {
            geometry,
            source: EntrySource::Adaptive3d,
            clearance_mm: cfg.entry_clearance_mm,
        });
    }
    let Some(cfg) = dressups else {
        return Err(RampFallback::EntryUnknown);
    };
    let geometry = match cfg.entry_style {
        DressupEntryStyle::None => return Err(RampFallback::EntryOff),
        DressupEntryStyle::Ramp => EntryGeometry::Ramp {
            angle_deg: cfg.ramp_angle,
        },
        DressupEntryStyle::Helix => EntryGeometry::Helix {
            radius_mm: cfg.helix_radius_for(&cutter).emitted_mm,
            pitch_mm: cfg.helix_pitch,
        },
    };
    Ok(Entry {
        geometry,
        source: EntrySource::Dressup,
        clearance_mm: cfg.entry_clearance_mm,
    })
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
    let fallback = |reason| RampBasis::NoChip { reason };
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
    /// The plunge term `plunge / tan θ` is the smaller term (G10 Q2).
    PlungeTerm,
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
    /// The shipped plunge (mm/min), G10 feed / Z. It equals the G6
    /// vertical rate only at depth tier 1 (G10_PLAN F2); the card shows
    /// both.
    pub plunge_mm_min: f64,
    pub drill: Box<DrillClaim>,
    pub size: Option<Box<Claim>>,
}

/// A ramp feed with no G6 chip: the ramp holds its vertical rate at the
/// plunge (G10 Q2, a repo rule with no source).
#[derive(Debug, Clone, PartialEq)]
pub struct PlungeSlopeRamp {
    /// The feed that ships (mm/min), rounded down to 1 mm/min.
    pub value_mm_min: f64,
    /// The term that set it: `CutFeed` or `PlungeTerm`.
    pub arm: RampArm,
    /// The configured entry slope.
    pub entry: EntryGeometry,
    /// Where the entry comes from.
    pub source: EntrySource,
    pub tan_theta: f64,
    pub theta_deg: f64,
    /// The shipped plunge (mm/min): the vertical rate.
    pub plunge_mm_min: f64,
    /// `plunge / tan θ` (mm/min).
    pub plunge_term_mm_min: f64,
    /// The shipped cut feed (mm/min).
    pub cut_feed_mm_min: f64,
    /// Why no G6 chip sets the ramp.
    pub no_chip: RampFallback,
}

/// The ramp feed that Suggest writes, and why.
#[derive(Debug, Clone, PartialEq)]
pub enum RampFeed {
    /// The G6 chip sets the feed.
    Sourced(Box<SourcedRamp>),
    /// No G6 chip: the ramp holds its vertical rate at the plunge (G10 Q2).
    PlungeSlope(Box<PlungeSlopeRamp>),
    /// The entry has no ramp feed and uses the plunge rate. Suggest writes
    /// `None`.
    NoEntryFeed {
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
            Self::PlungeSlope(s) => Some(s.value_mm_min),
            Self::NoEntryFeed { .. } => None,
        }
    }

    /// The short name of the arm, for the FM1 columns.
    #[must_use]
    pub fn arm_name(&self) -> &'static str {
        match self {
            Self::Sourced(s) => match s.arm {
                RampArm::CutFeed => "Sourced/CutFeed",
                RampArm::ChipTerm => "Sourced/ChipTerm",
                RampArm::PlungeTerm => "Sourced/PlungeTerm",
            },
            Self::PlungeSlope(s) => match s.arm {
                RampArm::CutFeed => "PlungeSlope/CutFeed",
                RampArm::ChipTerm => "PlungeSlope/ChipTerm",
                RampArm::PlungeTerm => "PlungeSlope/PlungeTerm",
            },
            Self::NoEntryFeed { .. } => "NoEntryFeed",
        }
    }

    /// The card text: the face line and a detail line.
    ///
    /// Example face lines:
    /// - G6: "Ramp feed 4000 mm/min: the cut feed (chip limit 38162 mm/min
    ///   at ramp 3.00°)";
    /// - G10 Q2: "Ramp feed 2400 mm/min: the cut feed (plunge limit 22897
    ///   mm/min = plunge 1200 / tan 3.00°)";
    /// - no entry feed: "Ramp feed: the plunge rate, 703 mm/min: <reason>".
    #[must_use]
    pub fn card_text(&self) -> (String, String) {
        match self {
            Self::Sourced(s) => {
                let entry = s.entry.label();
                let headline = match s.arm {
                    RampArm::CutFeed | RampArm::PlungeTerm => format!(
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
                     G6 vertical rate is {:.0} mm/min and the plunge (feed / Z, ruling Q4) is {:.0} \
                     mm/min; the two agree only at depth tier 1; the entry is from {}; θ is the \
                     configured slope; {}",
                    s.axial_chip_mm,
                    s.rpm,
                    s.flutes,
                    s.vertical_mm_min,
                    s.theta_deg,
                    s.tan_theta,
                    s.chip_term_mm_min,
                    s.cut_feed_mm_min,
                    s.vertical_mm_min,
                    s.plunge_mm_min,
                    s.source.label(),
                    sourced_detail(&s.drill, s.size.as_deref()),
                );
                (headline, detail)
            }
            Self::PlungeSlope(s) => {
                let headline = match s.arm {
                    RampArm::CutFeed | RampArm::ChipTerm => format!(
                        "Ramp feed {:.0} mm/min: the cut feed (plunge limit {:.0} mm/min = plunge \
                         {:.0} / tan {:.2}°)",
                        s.value_mm_min, s.plunge_term_mm_min, s.plunge_mm_min, s.theta_deg
                    ),
                    RampArm::PlungeTerm => format!(
                        "Ramp feed {:.0} mm/min: the plunge limit at {} (cut feed {:.0} mm/min)",
                        s.value_mm_min,
                        s.entry.label(),
                        s.cut_feed_mm_min
                    ),
                };
                let detail = format!(
                    "{PLUNGE_SLOPE_RULE_TEXT}; plunge {:.0} mm/min / tan {:.2}° ({:.5}) = plunge \
                     limit {:.0} mm/min; cut feed {:.0} mm/min; the entry is from {}; no G6 chip: \
                     {}",
                    s.plunge_mm_min,
                    s.theta_deg,
                    s.tan_theta,
                    s.plunge_term_mm_min,
                    s.cut_feed_mm_min,
                    s.source.label(),
                    s.no_chip.text()
                );
                (headline, detail)
            }
            Self::NoEntryFeed {
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
/// 1. A drill cycle basis gives `NoEntryFeed { DrillCycle }`.
/// 2. An entry error (`EntryOff`, `EntryUnknown`) gives `NoEntryFeed`.
/// 3. A cut feed that is not positive gives `NoCutFeed`.
/// 4. A slope with no valid tan θ gives `DegenerateEntry`.
/// 5. A `Sourced` basis with a positive RPM gives the G6 arm:
///    `vertical = chip x RPM x Z`, `chip_term = vertical / tan θ`, and the
///    value is `min(cut, chip_term)`.
/// 6. Otherwise a plunge that is not positive gives `NoEntryFeed {
///    NoPlunge }`.
/// 7. Otherwise the G10 Q2 arm: the value is `min(cut, plunge / tan θ)`.
///    `no_chip` is the basis reason, or `NoShippedRpm`.
///
/// Every value rounds DOWN to 1 mm/min (T-9: the value is bound from
/// above).
#[must_use]
pub fn resolve_ramp_feed(
    basis: &RampBasis,
    entry: Result<Entry, RampFallback>,
    cut_feed_mm_min: f64,
    rpm: Option<u32>,
    flutes: u32,
    plunge_mm_min: f64,
) -> RampFeed {
    let fallback = |reason| RampFeed::NoEntryFeed {
        reason,
        plunge_mm_min,
    };
    if let RampBasis::NoChip {
        reason: RampFallback::DrillCycle,
    } = basis
    {
        return fallback(RampFallback::DrillCycle);
    }
    let entry = match entry {
        Ok(entry) => entry,
        Err(reason) => return fallback(reason),
    };
    if !(cut_feed_mm_min.is_finite() && cut_feed_mm_min > 0.0) {
        return fallback(RampFallback::NoCutFeed);
    }
    let (Some(tan_theta), Some(theta_deg)) =
        (entry.geometry.tan_theta(), entry.geometry.theta_deg())
    else {
        return fallback(RampFallback::DegenerateEntry);
    };
    let rpm = rpm.filter(|r| *r > 0).map(f64::from);
    let no_chip = match (basis, rpm) {
        (
            RampBasis::Sourced {
                axial_chip_mm,
                drill,
                size,
            },
            Some(rpm),
        ) => {
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
            return RampFeed::Sourced(Box::new(SourcedRamp {
                value_mm_min,
                arm,
                entry: entry.geometry,
                source: entry.source,
                tan_theta,
                theta_deg,
                axial_chip_mm: *axial_chip_mm,
                rpm,
                flutes,
                vertical_mm_min,
                chip_term_mm_min,
                cut_feed_mm_min,
                plunge_mm_min,
                drill: drill.clone(),
                size: size.clone(),
            }));
        }
        (RampBasis::Sourced { .. }, None) => RampFallback::NoShippedRpm,
        (RampBasis::NoChip { reason }, _) => *reason,
    };
    if !(plunge_mm_min.is_finite() && plunge_mm_min > 0.0) {
        return fallback(RampFallback::NoPlunge);
    }
    let plunge_term_mm_min = plunge_mm_min / tan_theta;
    let raw = cut_feed_mm_min.min(plunge_term_mm_min);
    let value_mm_min = super::suggest::round_suggestion_value_down(raw, 1.0);
    if !(value_mm_min.is_finite() && value_mm_min > 0.0) {
        return fallback(RampFallback::NoPlunge);
    }
    let arm = if plunge_term_mm_min < cut_feed_mm_min {
        RampArm::PlungeTerm
    } else {
        RampArm::CutFeed
    };
    RampFeed::PlungeSlope(Box::new(PlungeSlopeRamp {
        value_mm_min,
        arm,
        entry: entry.geometry,
        source: entry.source,
        tan_theta,
        theta_deg,
        plunge_mm_min,
        plunge_term_mm_min,
        cut_feed_mm_min,
        no_chip,
    }))
}

/// One entry rule on the card: a face line and its hover.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryNote {
    /// The face line.
    pub headline: String,
    /// The hover: the rule, its source or the lack of one.
    pub detail: String,
    /// True when the line is a caution (the card marks it).
    pub caution: bool,
}

/// The entry rules of one operation, in card order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntryNotes(pub Vec<EntryNote>);

impl EntryNotes {
    /// The notes, in card order.
    #[must_use]
    pub fn as_slice(&self) -> &[EntryNote] {
        &self.0
    }

    /// True when the operation has no entry rule to state.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// True when two configured values are the same number.
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9
}

/// "a repo rule (default 3.00°)" or "an operator value (repo rule default
/// 3.00°)".
fn rule_or_operator(value: f64, default: f64, unit: &str) -> String {
    if same(value, default) {
        format!("a repo rule (default {default:.2}{unit})")
    } else {
        format!("an operator value (repo rule default {default:.2}{unit})")
    }
}

/// "a repo rule (0.3 x D = 0.95 mm)" or "an operator value 2.00 mm (repo
/// rule 0.3 x D = 0.95 mm)". `diameter_mm` is the cutter's `diameter()`.
fn radius_rule_or_operator(helix: &HelixRadius, diameter_mm: f64) -> String {
    let rule_mm = HELIX_RADIUS_OVER_D * diameter_mm;
    let rule = format!("{HELIX_RADIUS_OVER_D} x D = {rule_mm:.2} mm");
    if helix.from_rule {
        format!("a repo rule ({rule})")
    } else {
        format!(
            "an operator value {:.2} mm (repo rule {rule})",
            helix.requested_mm
        )
    }
}

/// The ruling Q12 caution on a straight plunge entry.
fn straight_plunge_note() -> EntryNote {
    EntryNote {
        headline: "Straight plunge entry: a down-cut or compression tool must ramp (Vortex, AXYZ, \
                   grade c)"
            .to_owned(),
        detail: "The engine does not read the tool's cut direction (ruling Q12, future work). \
                 Choose a helix or a ramp entry for a down-cut or compression tool."
            .to_owned(),
        caution: true,
    }
}

/// The geometry note of a helix (rulings Q6 and Q7): no core, the cap, or a
/// centre pip. The numbers come from the tool profile. The engine caps r at
/// the flat bottom, so a flat or bull helix never leaves a core.
fn helix_geometry_note(cutter: &dyn MillingCutter, helix: &HelixRadius) -> EntryNote {
    let radius_mm = helix.emitted_mm;
    if let Some(flat_mm) = helix.flat_bottom_mm {
        if helix.capped() {
            let requested_mm = helix.requested_mm;
            return EntryNote {
                headline: format!(
                    "Helix leaves no core: capped r {requested_mm:.2} → {flat_mm:.2} mm (no-core \
                     rule, geometry)"
                ),
                detail: format!(
                    "Geometry: r {requested_mm:.2} mm is larger than the flat bottom \
                     {flat_mm:.2} mm and would leave a core Ø {:.2} mm, so the engine emits the \
                     helix at the flat bottom (IMCO, Sandvik and Fusion print the limit r <= \
                     the flat bottom; ruling Q6).",
                    2.0 * (requested_mm - flat_mm)
                ),
                caution: false,
            };
        }
        return EntryNote {
            headline: format!(
                "Helix leaves no core: r {radius_mm:.2} mm <= {flat_mm:.2} mm, the flat bottom"
            ),
            detail: "Geometry: a helix radius at or inside the flat bottom cuts the centre \
                     (IMCO, Sandvik and Fusion print the same limit; ruling Q6)."
                .to_owned(),
            caution: false,
        };
    }
    match cutter.height_at_radius(radius_mm) {
        Some(pip_mm) => EntryNote {
            headline: format!("The helix leaves a centre pip {pip_mm:.2} mm high"),
            detail: format!(
                "Derived from the tool profile: the profile height at r {radius_mm:.2} mm is the \
                 height of the stock that stays at the centre (ruling Q7)."
            ),
            caution: false,
        },
        None => {
            let tool_radius_mm = cutter.radius();
            let core_mm = 2.0 * (radius_mm - tool_radius_mm);
            EntryNote {
                headline: format!(
                    "The helix leaves a core Ø {core_mm:.2} mm: r {radius_mm:.2} mm is larger than \
                     the tool radius {tool_radius_mm:.2} mm"
                ),
                detail: "Geometry: the helix radius is larger than the tool radius, so a post of \
                         stock stays at the centre (ruling Q7)."
                    .to_owned(),
                caution: true,
            }
        }
    }
}

/// The entry rules of the operation that ships (G10 Q6-Q10, Q12). Each
/// note is one face line on the card.
///
/// - A straight plunge on a Roughing pass: the Q12 caution.
/// - A ramp: the angle, a repo rule or an operator value (Q9).
/// - A helix: r, r / D, the pitch and θ (Q8), then the core or the pip
///   from the tool profile (Q6, Q7).
/// - A helix or a ramp: the clearance above the material (Q10).
///
/// An unknown entry gives no note.
#[must_use]
pub fn entry_notes(
    op: &OperationConfig,
    dressups: Option<&DressupConfig>,
    tool: &ToolConfig,
    pass_role: PassRole,
) -> EntryNotes {
    let mut notes = Vec::new();
    let entry = match entry_geometry(op, dressups, tool) {
        Ok(entry) => entry,
        Err(RampFallback::EntryOff) => {
            if matches!(pass_role, PassRole::Roughing) {
                notes.push(straight_plunge_note());
            }
            return EntryNotes(notes);
        }
        Err(_) => return EntryNotes(notes),
    };
    let dressup_default = DressupConfig::default();
    let adaptive3d_default = Adaptive3dConfig::default();
    match entry.geometry {
        EntryGeometry::Ramp { angle_deg } => {
            let default = match entry.source {
                EntrySource::Dressup => dressup_default.ramp_angle,
                EntrySource::Adaptive3d => adaptive3d_default.ramp_angle_deg,
            };
            let who = if same(angle_deg, default) {
                format!("repo rule (default {default:.2}°)")
            } else {
                format!("operator value (repo rule default {default:.2}°)")
            };
            notes.push(EntryNote {
                headline: format!("Ramp angle {angle_deg:.2}°: {who}, no wood or router source"),
                detail: "The ramp angle is a repo rule or an operator value (ruling Q9). No wood \
                         or router source prints a ramp angle. Metal context only: Harvey 3-10° \
                         in soft material (grade c); SGS compression router bits 5° at most."
                    .to_owned(),
                caution: false,
            });
        }
        EntryGeometry::Helix {
            radius_mm,
            pitch_mm,
        } => {
            let cutter = crate::compute::cutter::build_cutter(tool);
            let (helix, pitch_default) = match (op, dressups) {
                (OperationConfig::Adaptive3d(cfg), _) => (
                    cfg.helix_radius_for(&cutter),
                    adaptive3d_default.helix_pitch,
                ),
                (_, Some(cfg)) => (cfg.helix_radius_for(&cutter), dressup_default.helix_pitch),
                // `entry_geometry` gave a helix, so one of the two holds it.
                (_, None) => (
                    HelixRadius::resolve(radius_mm, false, &cutter),
                    dressup_default.helix_pitch,
                ),
            };
            let theta = entry
                .geometry
                .theta_deg()
                .map_or_else(|| "no valid slope".to_owned(), |t| format!("{t:.2}°"));
            // D is the one the engine scales the rule by: the nominal
            // cutting diameter (the tip ball on a tapered ball).
            let diameter_mm = crate::compute::config::helix_entry_diameter_mm(&cutter);
            let over_d = if diameter_mm > 0.0 {
                format!("{:.2} x D", radius_mm / diameter_mm)
            } else {
                "no D".to_owned()
            };
            notes.push(EntryNote {
                headline: format!(
                    "Helix r {radius_mm:.2} mm ({over_d}), pitch {pitch_mm:.2} mm = {theta}"
                ),
                detail: format!(
                    "The helix radius is {}; the pitch is {} (rulings Q6 and Q8). No wood source \
                     prints a helix radius or pitch (metal context: IMCO 0.5-5°, grade c).",
                    radius_rule_or_operator(&helix, diameter_mm),
                    rule_or_operator(pitch_mm, pitch_default, " mm")
                ),
                caution: false,
            });
            notes.push(helix_geometry_note(&cutter, &helix));
        }
    }
    let clearance_default = default_entry_clearance_mm();
    let clearance_mm = entry.clearance_mm;
    let who = if same(clearance_mm, clearance_default) {
        format!("no source; operator rule {clearance_default} mm")
    } else {
        format!("operator value (no source; operator rule {clearance_default} mm)")
    };
    notes.push(EntryNote {
        headline: format!("Entry starts {clearance_mm:.2} mm above the material: {who}"),
        detail: "Above this height the entry is a straight move through air (operator ruling \
                 2026-09-25: never helix air). No source prints an entry clearance (ruling Q10)."
            .to_owned(),
        caution: false,
    });
    EntryNotes(notes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationType;
    use crate::compute::tool_config::{ToolId, ToolType};
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
            diameter_mm: 6.0,
            range_mm: 3.0..=12.7,
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

    fn entry(geometry: EntryGeometry) -> Entry {
        Entry {
            geometry,
            source: EntrySource::Dressup,
            clearance_mm: 0.5,
        }
    }

    fn sourced(r: &RampFeed) -> &SourcedRamp {
        match r {
            RampFeed::Sourced(s) => s,
            RampFeed::PlungeSlope(s) => panic!("expected Sourced, got {s:?}"),
            RampFeed::NoEntryFeed { reason, .. } => panic!("expected Sourced, got {reason:?}"),
        }
    }

    fn plunge_slope(r: &RampFeed) -> &PlungeSlopeRamp {
        match r {
            RampFeed::PlungeSlope(s) => s,
            RampFeed::Sourced(s) => panic!("expected PlungeSlope, got {s:?}"),
            RampFeed::NoEntryFeed { reason, .. } => panic!("expected PlungeSlope, got {reason:?}"),
        }
    }

    fn resolve(geometry: EntryGeometry, cut: f64, rpm: u32) -> RampFeed {
        resolve_ramp_feed(&basis(), Ok(entry(geometry)), cut, Some(rpm), 2, 1000.0)
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
        assert!(
            detail.contains("the plunge (feed / Z, ruling Q4)"),
            "{detail}"
        );
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
        for (geometry, cut, rpm) in [
            (HELIX, 4572.4, 18_000),
            (EntryGeometry::Ramp { angle_deg: 30.0 }, 4000.0, 15_748),
            (EntryGeometry::Ramp { angle_deg: 45.0 }, 9999.9, 12_345),
        ] {
            let r = resolve(geometry, cut, rpm);
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

    /// Every entry fallback writes `None` and gives its reason: a straight
    /// plunge, an unknown entry, a slope with no valid θ, no cut feed, no
    /// plunge and a drill cycle.
    #[test]
    fn every_entry_fallback_writes_none() {
        let no_lut = RampBasis::NoChip {
            reason: RampFallback::NoLut,
        };
        let drill = RampBasis::NoChip {
            reason: RampFallback::DrillCycle,
        };
        let cases = [
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
                    &no_lut,
                    Err(RampFallback::EntryUnknown),
                    4000.0,
                    Some(18_000),
                    2,
                    700.0,
                ),
                RampFallback::EntryUnknown,
            ),
            (
                resolve_ramp_feed(
                    &basis(),
                    Ok(entry(EntryGeometry::Ramp { angle_deg: 0.0 })),
                    4000.0,
                    Some(18_000),
                    2,
                    700.0,
                ),
                RampFallback::DegenerateEntry,
            ),
            (
                resolve_ramp_feed(
                    &no_lut,
                    Ok(entry(EntryGeometry::Helix {
                        radius_mm: 2.0,
                        pitch_mm: 0.0,
                    })),
                    4000.0,
                    Some(18_000),
                    2,
                    700.0,
                ),
                RampFallback::DegenerateEntry,
            ),
            (
                resolve_ramp_feed(&basis(), Ok(entry(HELIX)), 0.0, Some(18_000), 2, 700.0),
                RampFallback::NoCutFeed,
            ),
            (
                resolve_ramp_feed(&drill, Ok(entry(HELIX)), 4000.0, Some(18_000), 2, 700.0),
                RampFallback::DrillCycle,
            ),
        ];
        for (record, want) in cases {
            assert_eq!(record.value(), None, "{want:?}");
            match &record {
                RampFeed::NoEntryFeed {
                    reason,
                    plunge_mm_min,
                } => {
                    assert_eq!(*reason, want);
                    assert!((plunge_mm_min - 700.0).abs() < 1e-12);
                }
                RampFeed::Sourced(_) | RampFeed::PlungeSlope(_) => {
                    panic!("{want:?}: expected no entry feed")
                }
            }
            let (face, _) = record.card_text();
            assert!(face.contains("the plunge rate, 700 mm/min"), "{face}");
            assert!(face.ends_with("the entry uses the plunge rate."), "{face}");
        }
        // No plunge: the ramp has no vertical rate.
        let record = resolve_ramp_feed(&no_lut, Ok(entry(HELIX)), 4000.0, Some(18_000), 2, 0.0);
        assert_eq!(record.value(), None);
        assert!(matches!(
            record,
            RampFeed::NoEntryFeed {
                reason: RampFallback::NoPlunge,
                ..
            }
        ));
    }

    /// With no G6 chip (no vendor table, or no shipped RPM), the ramp holds
    /// its vertical rate at the plunge: `min(F, plunge / tan θ)` (G10 Q2).
    #[test]
    fn a_no_chip_ramp_holds_the_plunge_slope() {
        let no_lut = RampBasis::NoChip {
            reason: RampFallback::NoLut,
        };
        let ramp3 = EntryGeometry::Ramp { angle_deg: 3.0 };
        // 700 / tan 3° = 13 357 > 4000: the cut feed binds.
        let r = resolve_ramp_feed(&no_lut, Ok(entry(ramp3)), 4000.0, Some(18_000), 2, 700.0);
        let s = plunge_slope(&r);
        assert_eq!(s.arm, RampArm::CutFeed);
        assert_eq!(s.no_chip, RampFallback::NoLut);
        assert!((s.plunge_term_mm_min - 700.0 / 3.0_f64.to_radians().tan()).abs() < 1e-9);
        assert_eq!(r.value(), Some(4000.0));
        let (face, detail) = r.card_text();
        assert!(face.contains("plunge limit"), "{face}");
        assert!(detail.starts_with(PLUNGE_SLOPE_RULE_TEXT), "{detail}");
        assert!(detail.contains("no G6 chip"), "{detail}");

        // A 45° ramp: 700 / tan 45° = 700 < 4000, so the plunge term binds.
        let steep = EntryGeometry::Ramp { angle_deg: 45.0 };
        let r = resolve_ramp_feed(&no_lut, Ok(entry(steep)), 4000.0, Some(18_000), 2, 700.0);
        let s = plunge_slope(&r);
        assert_eq!(s.arm, RampArm::PlungeTerm);
        let want = (700.0 / 45.0_f64.to_radians().tan()).floor();
        assert_eq!(r.value(), Some(want));
        assert!(r.card_text().0.contains("the plunge limit at ramp 45.00°"));

        // A G6 basis with no shipped RPM: the plunge slope, NoShippedRpm.
        let r = resolve_ramp_feed(&basis(), Ok(entry(ramp3)), 4000.0, None, 2, 700.0);
        let s = plunge_slope(&r);
        assert_eq!(s.no_chip, RampFallback::NoShippedRpm);
        assert_eq!(r.value(), Some(4000.0));
    }

    /// The two θ systems read the right fields, and the clearance comes with
    /// the entry.
    #[test]
    fn the_entry_reads_the_dressup_or_the_adaptive3d_config() {
        let pocket = OperationConfig::new_default(OperationType::Pocket);
        let flat = tool(ToolType::EndMill, 6.0);
        assert_eq!(
            entry_geometry(&pocket, None, &flat),
            Err(RampFallback::EntryUnknown)
        );
        let off = DressupConfig {
            entry_style: DressupEntryStyle::None,
            ..DressupConfig::default()
        };
        assert_eq!(
            entry_geometry(&pocket, Some(&off), &flat),
            Err(RampFallback::EntryOff)
        );
        let helix = DressupConfig {
            entry_style: DressupEntryStyle::Helix,
            helix_radius: Some(2.0),
            helix_pitch: 1.0,
            entry_clearance_mm: 0.8,
            ..DressupConfig::default()
        };
        assert_eq!(
            entry_geometry(&pocket, Some(&helix), &flat),
            Ok(Entry {
                geometry: HELIX,
                source: EntrySource::Dressup,
                clearance_mm: 0.8,
            })
        );

        let mut a3d = OperationConfig::new_default(OperationType::Adaptive3d);
        if let OperationConfig::Adaptive3d(cfg) = &mut a3d {
            cfg.entry_style = Adaptive3dEntryStyle::Helix;
            cfg.helix_radius_factor = 0.3;
            cfg.helix_pitch = 2.0;
        }
        let e = entry_geometry(&a3d, None, &flat).unwrap();
        assert_eq!(e.source, EntrySource::Adaptive3d);
        assert!((e.geometry.theta_deg().unwrap() - 10.03).abs() < 0.01);
        assert!((e.clearance_mm - default_entry_clearance_mm()).abs() < 1e-12);
        if let OperationConfig::Adaptive3d(cfg) = &mut a3d {
            cfg.entry_style = Adaptive3dEntryStyle::Plunge;
        }
        assert_eq!(
            entry_geometry(&a3d, Some(&helix), &flat),
            Err(RampFallback::EntryOff)
        );
    }

    fn tool(kind: ToolType, diameter: f64) -> ToolConfig {
        let mut t = ToolConfig::new_default(ToolId(1), kind);
        t.diameter = diameter;
        t.cutting_length = (diameter * 4.0).max(12.0);
        if matches!(kind, ToolType::VBit) {
            t.included_angle = 60.0;
        }
        t
    }

    fn helix_notes(tool: &ToolConfig, radius_mm: f64) -> EntryNotes {
        let pocket = OperationConfig::new_default(OperationType::Pocket);
        let dressups = DressupConfig {
            entry_style: DressupEntryStyle::Helix,
            helix_radius: Some(radius_mm),
            helix_pitch: 1.0,
            ..DressupConfig::default()
        };
        entry_notes(&pocket, Some(&dressups), tool, PassRole::Roughing)
    }

    fn geometry_note(notes: &EntryNotes) -> &EntryNote {
        notes
            .as_slice()
            .get(1)
            .unwrap_or_else(|| panic!("a helix gives its geometry note second: {notes:?}"))
    }

    /// The pip and the cap come from the tool profile (rulings Q6, Q7):
    /// a 6 mm ball at r 1.8 leaves a pip of 3 - sqrt(9 - 1.8²) = 0.60 mm; a
    /// 3.175 mm flat at r 2.0 would leave a core of 2 x (2.0 - 1.5875) =
    /// 0.83 mm, so the engine caps r at the flat bottom 1.5875 mm (G10 Part
    /// B); a 60° V-bit at r 1.8 leaves a pip of 1.8 / tan 30° = 3.12 mm.
    #[test]
    fn the_pip_and_the_core_come_from_the_tool_profile() {
        let ball = helix_notes(&tool(ToolType::BallNose, 6.0), 1.8);
        let note = geometry_note(&ball);
        assert!(!note.caution, "{note:?}");
        assert!(note.headline.contains("centre pip 0.60 mm"), "{note:?}");

        let flat = helix_notes(&tool(ToolType::EndMill, 3.175), 2.0);
        let note = geometry_note(&flat);
        assert!(!note.caution, "{note:?}");
        assert!(
            note.headline
                .contains("capped r 2.00 → 1.59 mm (no-core rule, geometry)"),
            "{note:?}"
        );
        assert!(note.detail.contains("core Ø 0.83 mm"), "{note:?}");
        let helix_line = flat.as_slice().first().unwrap();
        assert!(
            helix_line
                .headline
                .starts_with("Helix r 1.59 mm (0.50 x D)"),
            "the helix line states the emitted radius: {flat:?}"
        );
        assert!(
            helix_line.detail.contains("an operator value 2.00 mm"),
            "{flat:?}"
        );

        let inside = helix_notes(&tool(ToolType::EndMill, 6.0), 1.8);
        let note = geometry_note(&inside);
        assert!(!note.caution, "{note:?}");
        assert!(note.headline.contains("no core"), "{note:?}");

        let vbit = helix_notes(&tool(ToolType::VBit, 12.7), 1.8);
        let note = geometry_note(&vbit);
        assert!(!note.caution, "{note:?}");
        assert!(note.headline.contains("centre pip 3.12 mm"), "{note:?}");
    }

    /// The ramp angle, the clearance and the straight-plunge caution.
    #[test]
    fn the_notes_name_the_angle_the_clearance_and_the_plunge_caution() {
        let pocket = OperationConfig::new_default(OperationType::Pocket);
        let flat = tool(ToolType::EndMill, 6.0);
        let ramp = DressupConfig {
            entry_style: DressupEntryStyle::Ramp,
            ..DressupConfig::default()
        };
        let notes = entry_notes(&pocket, Some(&ramp), &flat, PassRole::Roughing);
        let lines: Vec<&str> = notes
            .as_slice()
            .iter()
            .map(|n| n.headline.as_str())
            .collect();
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("Ramp angle 3.00°: repo rule")),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("no source; operator rule 0.5 mm")),
            "{lines:?}"
        );

        let off = DressupConfig {
            entry_style: DressupEntryStyle::None,
            ..DressupConfig::default()
        };
        let notes = entry_notes(&pocket, Some(&off), &flat, PassRole::Roughing);
        assert_eq!(notes.as_slice().len(), 1, "{notes:?}");
        assert!(notes.as_slice().iter().all(|n| n.caution), "{notes:?}");
        let notes = entry_notes(&pocket, Some(&off), &flat, PassRole::Finish);
        assert!(notes.is_empty(), "{notes:?}");
        assert!(entry_notes(&pocket, None, &flat, PassRole::Roughing).is_empty());
    }
}
