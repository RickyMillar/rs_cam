//! The validity domain of each numeric tool dimension (F029 step 1, F210).
//!
//! [`ToolDimension::domain`] holds the hard bounds. Each bound comes from
//! the geometry, not from a table of preferences:
//!
//! - every value is finite;
//! - a diameter and a flute length are lengths, so `> 0`;
//! - a helix angle is in `[0, 90)`: at 90° the flute does not advance;
//! - a tool has at least one flute;
//! - a corner radius is at most the tool radius; a bull nose needs a
//!   corner (`> 0`), else it is a flat end mill;
//! - a V-bit included angle is in `(0, 180)`, a taper half-angle in
//!   `(0, 90)`: at the ends the cone is a line or a plane;
//! - a taper shaft is at least the tip; a shank and a stickout are `>= 0`.
//!
//! The cutter constructors and `ToolConfig::validate` (the session
//! commands, the project loader, the tool-library loader and writers, MCP
//! `add_tool`) refuse a value outside it.
//!
//! [`ToolDimension::ui_range`] is a different thing: the drag range of the
//! GUI tool editor. No door refuses a tool by it.

use std::ops::RangeInclusive;

/// A numeric tool field that has a domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolDimension {
    /// Cutting diameter, mm (the tip diameter on a tapered ball).
    Diameter,
    /// Flute length, mm.
    CuttingLength,
    /// Helix angle, degrees.
    HelixDeg,
    /// Number of flutes.
    FluteCount,
    /// Corner radius of a flat end mill, mm.
    CornerRadiusMm,
    /// Corner radius of a bull nose, mm.
    CornerRadius,
    /// V-bit full included angle, degrees.
    IncludedAngle,
    /// Tapered-ball cone half-angle, degrees.
    TaperHalfAngle,
    /// Tapered-ball cone base ("shaft") diameter, mm.
    ShaftDiameter,
    /// Shank diameter in the collet, mm. 0 means "not set".
    ShankDiameter,
    /// Stickout from the holder face, mm.
    Stickout,
}

/// One end of a [`ToolDomain`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bound {
    /// No bound at this end (the value must still be finite).
    Unbounded,
    /// The end value is in the domain.
    Closed(f64),
    /// The end value is not in the domain.
    Open(f64),
}

/// The validity domain of one dimension. Each end is open or closed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolDomain {
    pub lower: Bound,
    pub upper: Bound,
}

/// The rule a refused value breaks, as the error states it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Requirement {
    /// The value is NaN or infinite.
    Finite,
    /// `> bound`.
    GreaterThan(f64),
    /// `>= bound`.
    AtLeast(f64),
    /// `< bound`.
    LessThan(f64),
    /// `<= bound`.
    AtMost(f64),
}

impl std::fmt::Display for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Requirement::Finite => write!(f, "a finite number"),
            Requirement::GreaterThan(b) => write!(f, "> {b}"),
            Requirement::AtLeast(b) => write!(f, ">= {b}"),
            Requirement::LessThan(b) => write!(f, "< {b}"),
            Requirement::AtMost(b) => write!(f, "<= {b}"),
        }
    }
}

impl ToolDomain {
    const fn new(lower: Bound, upper: Bound) -> Self {
        Self { lower, upper }
    }

    /// `None` when `value` is in the domain, else the rule it breaks.
    pub fn violation(&self, value: f64) -> Option<Requirement> {
        if !value.is_finite() {
            return Some(Requirement::Finite);
        }
        match self.lower {
            Bound::Closed(b) if value < b => return Some(Requirement::AtLeast(b)),
            Bound::Open(b) if value <= b => return Some(Requirement::GreaterThan(b)),
            _ => {}
        }
        match self.upper {
            Bound::Closed(b) if value > b => Some(Requirement::AtMost(b)),
            Bound::Open(b) if value >= b => Some(Requirement::LessThan(b)),
            _ => None,
        }
    }
}

impl ToolDimension {
    /// The field name as spelled in project TOML and on the MCP wire.
    pub const fn field_name(self) -> &'static str {
        match self {
            ToolDimension::Diameter => "diameter",
            ToolDimension::CuttingLength => "cutting_length",
            ToolDimension::HelixDeg => "helix_deg",
            ToolDimension::FluteCount => "flute_count",
            ToolDimension::CornerRadiusMm => "corner_radius_mm",
            ToolDimension::CornerRadius => "corner_radius",
            ToolDimension::IncludedAngle => "included_angle",
            ToolDimension::TaperHalfAngle => "taper_half_angle",
            ToolDimension::ShaftDiameter => "shaft_diameter",
            ToolDimension::ShankDiameter => "shank_diameter",
            ToolDimension::Stickout => "stickout",
        }
    }

    /// The hard validity domain (module doc). `diameter_mm` is the cutting
    /// diameter of the tool, for the bounds that depend on it.
    pub fn domain(self, diameter_mm: f64) -> ToolDomain {
        use Bound::{Closed, Open, Unbounded};
        let half = diameter_mm / 2.0;
        match self {
            ToolDimension::Diameter | ToolDimension::CuttingLength => {
                ToolDomain::new(Open(0.0), Unbounded)
            }
            ToolDimension::HelixDeg => ToolDomain::new(Closed(0.0), Open(90.0)),
            ToolDimension::FluteCount => ToolDomain::new(Closed(1.0), Unbounded),
            ToolDimension::CornerRadiusMm => ToolDomain::new(Closed(0.0), Closed(half)),
            ToolDimension::CornerRadius => ToolDomain::new(Open(0.0), Closed(half)),
            ToolDimension::IncludedAngle => ToolDomain::new(Open(0.0), Open(180.0)),
            ToolDimension::TaperHalfAngle => ToolDomain::new(Open(0.0), Open(90.0)),
            ToolDimension::ShaftDiameter => ToolDomain::new(Closed(diameter_mm), Unbounded),
            ToolDimension::ShankDiameter | ToolDimension::Stickout => {
                ToolDomain::new(Closed(0.0), Unbounded)
            }
        }
    }

    /// The drag range of the GUI tool editor.
    ///
    /// A GUI convenience, NOT a domain: no door refuses a tool by it. No
    /// printed source gives these numbers; they are the editor's literals
    /// from before F210, kept so the drag widgets behave as they did. The
    /// editor keeps a stored value outside this range (`ValueRow::keep_existing`);
    /// egui still clamps a value the operator drags or types to it.
    pub fn ui_range(self, diameter_mm: f64) -> RangeInclusive<f64> {
        match self {
            ToolDimension::Diameter => 0.1..=100.0,
            ToolDimension::CuttingLength => 0.1..=200.0,
            ToolDimension::HelixDeg => 0.0..=60.0,
            ToolDimension::FluteCount => 1.0..=8.0,
            ToolDimension::CornerRadiusMm => 0.0..=diameter_mm / 2.0,
            ToolDimension::CornerRadius => 0.01..=diameter_mm / 2.0,
            ToolDimension::IncludedAngle => 1.0..=179.0,
            ToolDimension::TaperHalfAngle => 0.5..=89.0,
            ToolDimension::ShaftDiameter => diameter_mm..=100.0,
            ToolDimension::ShankDiameter => 0.0..=100.0,
            ToolDimension::Stickout => 0.0..=300.0,
        }
    }

    /// Check `value` against [`Self::domain`].
    pub fn check(self, value: f64, diameter_mm: f64) -> Result<(), ToolGeometryError> {
        match self.domain(diameter_mm).violation(value) {
            None => Ok(()),
            Some(requirement) => Err(ToolGeometryError {
                dimension: self,
                value,
                requirement,
            }),
        }
    }
}

/// A tool dimension outside its domain. The cutter constructors and
/// `ToolConfig::validate` return it; no door may build a cutter from it.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[error("tool {} = {value}: must be {requirement}", dimension.field_name())]
pub struct ToolGeometryError {
    pub dimension: ToolDimension,
    pub value: f64,
    pub requirement: Requirement,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_names_the_field_the_value_and_the_bound() {
        let err = ToolDimension::IncludedAngle.check(0.0, 6.0);
        assert_eq!(
            err.map_err(|e| e.to_string()),
            Err("tool included_angle = 0: must be > 0".to_owned())
        );
        let err = ToolDimension::IncludedAngle.check(180.0, 6.0);
        assert_eq!(
            err.map_err(|e| e.to_string()),
            Err("tool included_angle = 180: must be < 180".to_owned())
        );
        assert!(ToolDimension::IncludedAngle.check(0.5, 6.0).is_ok());
        assert!(ToolDimension::Diameter.check(f64::NAN, 6.0).is_err());
        assert!(ToolDimension::Stickout.check(f64::INFINITY, 6.0).is_err());
    }

    #[test]
    fn the_open_and_closed_ends_are_exact() {
        assert!(ToolDimension::CornerRadius.check(3.0, 6.0).is_ok());
        assert!(ToolDimension::CornerRadius.check(3.01, 6.0).is_err());
        assert!(ToolDimension::CornerRadius.check(0.0, 6.0).is_err());
        assert!(ToolDimension::CornerRadiusMm.check(0.0, 6.0).is_ok());
        assert!(ToolDimension::ShaftDiameter.check(5.9, 6.0).is_err());
        assert!(ToolDimension::ShaftDiameter.check(6.0, 6.0).is_ok());
        assert!(ToolDimension::HelixDeg.check(0.0, 6.0).is_ok());
        assert!(ToolDimension::HelixDeg.check(90.0, 6.0).is_err());
        assert!(ToolDimension::Diameter.check(0.0, 6.0).is_err());
    }

    /// The GUI drag limits refuse nothing: a 200 mm, 12-flute tool is valid.
    #[test]
    fn the_ui_range_is_not_a_domain() {
        assert!(!ToolDimension::Diameter.ui_range(200.0).contains(&200.0));
        assert!(ToolDimension::Diameter.check(200.0, 200.0).is_ok());
        assert!(ToolDimension::FluteCount.check(12.0, 6.0).is_ok());
        assert!(ToolDimension::Stickout.check(500.0, 6.0).is_ok());
    }
}
