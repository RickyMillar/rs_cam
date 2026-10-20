//! The validity domain of each numeric machine, head and shoe field
//! (F210, W6b).
//!
//! [`MachineDimension::domain`] and [`HeadDimension::domain`] hold the hard
//! bounds. Each bound comes from the math that reads the field:
//!
//! - every value is finite;
//! - a rate (`max_feed_mm_min`, `max_cutting_feed_mm_min`, a per-axis
//!   `max_rate_xyz_mm_min`) is `> 0`: a move time is its length over the
//!   rate;
//! - an acceleration (`acceleration_mm_s2`, a per-axis
//!   `acceleration_xyz_mm_s2`) is `> 0`: a ramp time is a speed over it;
//! - `jerk_mm_s3` is `> 0`: the ramp rounding is `accel / jerk`;
//! - `junction_deviation_mm` is `>= 0`: the corner speed is
//!   `sqrt(accel * delta * s / (1 - s))`, so `0` stops at every corner and
//!   a negative value has no root;
//! - `max_junction_velocity_mm_min` is `>= 0`: it caps the corner speed,
//!   and `0` stops at every corner;
//! - `max_shank_mm` is a diameter, so `> 0`;
//! - `aggressiveness` scales the Suggest load target, so `> 0`: at `0`
//!   no load is allowed. A value above 1 warns and does not refuse
//!   (ruling R4 Q3);
//! - the head `mount_mm`, a part `height_mm`, the shoe `normal_gap_mm` and
//!   a stated shoe `gap_mm` are lengths, so `>= 0`; a part `diameter_mm`
//!   is `> 0`; a shoe travel end is a signed position from the holder
//!   face, so finite only.
//!
//! [`crate::machine::MachineProfile::validate`],
//! [`crate::machine::head::MachineHead::validate`] and
//! [`crate::machine::head::ShoeSetting::validate`] read it.
//!
//! `ui_range` is the drag range of the GUI machine, head and shoe rows.
//! No door refuses a value by it.

use std::ops::RangeInclusive;

use crate::tool::domain::{Bound, Requirement, ToolDomain};

/// A numeric machine-profile field that has a domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MachineDimension {
    /// The travel rate, mm/min.
    MaxFeed,
    /// The cutting-feed ceiling, mm/min.
    MaxCuttingFeed,
    /// The largest shank the spindle holds, mm.
    MaxShank,
    /// The Suggest load target.
    Aggressiveness,
    /// The scalar acceleration, mm/s².
    Acceleration,
    /// A per-axis acceleration, mm/s².
    AxisAcceleration,
    /// A per-axis maximum rate, mm/min.
    AxisMaxRate,
    /// The GRBL junction deviation, mm.
    JunctionDeviation,
    /// The jerk limit, mm/s³.
    Jerk,
    /// The corner-speed cap, mm/min.
    MaxJunctionVelocity,
}

/// The domain with the given ends.
const fn domain_of(lower: Bound, upper: Bound) -> ToolDomain {
    ToolDomain { lower, upper }
}

/// The widest drag range inside a domain with no upper end: a row with no
/// GUI literal of its own drags over the domain.
const POSITIVE: RangeInclusive<f64> = f64::MIN_POSITIVE..=f64::MAX;

impl MachineDimension {
    /// The field name as spelled in project TOML and on the MCP wire.
    #[must_use]
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::MaxFeed => "max_feed_mm_min",
            Self::MaxCuttingFeed => "max_cutting_feed_mm_min",
            Self::MaxShank => "max_shank_mm",
            Self::Aggressiveness => "aggressiveness",
            Self::Acceleration => "kinematics.acceleration_mm_s2",
            Self::AxisAcceleration => "kinematics.acceleration_xyz_mm_s2",
            Self::AxisMaxRate => "kinematics.max_rate_xyz_mm_min",
            Self::JunctionDeviation => "kinematics.junction_deviation_mm",
            Self::Jerk => "kinematics.jerk_mm_s3",
            Self::MaxJunctionVelocity => "kinematics.max_junction_velocity_mm_min",
        }
    }

    /// The hard validity domain (module doc).
    #[must_use]
    pub const fn domain(self) -> ToolDomain {
        use Bound::{Closed, Open, Unbounded};
        match self {
            Self::MaxFeed
            | Self::MaxCuttingFeed
            | Self::MaxShank
            | Self::Aggressiveness
            | Self::Acceleration
            | Self::AxisAcceleration
            | Self::AxisMaxRate
            | Self::Jerk => domain_of(Open(0.0), Unbounded),
            Self::JunctionDeviation | Self::MaxJunctionVelocity => {
                domain_of(Closed(0.0), Unbounded)
            }
        }
    }

    /// The drag range of the GUI machine panel.
    ///
    /// A GUI convenience, NOT a domain. No printed source gives these
    /// numbers; they are the panel literals from before F210. A field with
    /// no panel row drags over its domain.
    #[must_use]
    pub const fn ui_range(self) -> RangeInclusive<f64> {
        match self {
            Self::MaxFeed => 100.0..=30000.0,
            Self::MaxShank => 1.0..=25.0,
            Self::Aggressiveness => super::AGGRESSIVENESS_MIN..=super::AGGRESSIVENESS_MAX,
            Self::Acceleration | Self::AxisAcceleration => 10.0..=20000.0,
            Self::JunctionDeviation => 0.001..=1.0,
            Self::Jerk => 1.0..=100_000.0,
            Self::MaxCuttingFeed | Self::AxisMaxRate => POSITIVE,
            Self::MaxJunctionVelocity => 0.0..=f64::MAX,
        }
    }

    /// Check `value` against [`Self::domain`]. `axis` names the axis of a
    /// per-axis field (0 = X).
    ///
    /// # Errors
    ///
    /// [`MachineDomainError`] with the rule the value breaks.
    pub fn check(self, value: f64, axis: Option<usize>) -> Result<(), MachineDomainError> {
        match self.domain().violation(value) {
            None => Ok(()),
            Some(requirement) => Err(MachineDomainError {
                dimension: self,
                axis,
                value,
                requirement,
            }),
        }
    }
}

/// A machine field outside its domain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MachineDomainError {
    /// The field.
    pub dimension: MachineDimension,
    /// The axis of a per-axis field (0 = X).
    pub axis: Option<usize>,
    /// The value.
    pub value: f64,
    /// The rule it breaks.
    pub requirement: Requirement,
}

impl std::fmt::Display for MachineDomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = self.dimension.field_name();
        match self.axis {
            Some(axis) => write!(f, "machine {name}[{axis}]")?,
            None => write!(f, "machine {name}")?,
        }
        write!(f, " = {}: must be {}", self.value, self.requirement)
    }
}

impl std::error::Error for MachineDomainError {}

/// A numeric head or shoe field that has a domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeadDimension {
    /// The fixed mount above the holder face, mm.
    Mount,
    /// The diameter of a part, mm.
    PartDiameter,
    /// The height of a part, mm.
    PartHeight,
    /// The shoe gap for normal work, mm.
    NormalGap,
    /// An end of the shoe travel, mm from the holder face, signed.
    TravelEnd,
    /// The gap a toolpath states for the shoe, mm.
    ShoeGap,
}

impl HeadDimension {
    /// The hard validity domain (module doc).
    #[must_use]
    pub const fn domain(self) -> ToolDomain {
        use Bound::{Closed, Open, Unbounded};
        match self {
            Self::Mount | Self::PartHeight | Self::NormalGap | Self::ShoeGap => {
                domain_of(Closed(0.0), Unbounded)
            }
            Self::PartDiameter => domain_of(Open(0.0), Unbounded),
            Self::TravelEnd => domain_of(Unbounded, Unbounded),
        }
    }

    /// The drag range of the GUI head editor and shoe row.
    ///
    /// A GUI convenience, NOT a domain. No printed source gives these
    /// numbers; they are the editor literals from before F210.
    #[must_use]
    pub const fn ui_range(self) -> RangeInclusive<f64> {
        match self {
            Self::Mount | Self::NormalGap | Self::ShoeGap => 0.0..=500.0,
            Self::PartDiameter => 0.1..=1000.0,
            Self::PartHeight => 0.0..=1000.0,
            Self::TravelEnd => -500.0..=500.0,
        }
    }

    /// The rule `value` breaks, or `None` inside [`Self::domain`].
    #[must_use]
    pub fn violation(self, value: f64) -> Option<Requirement> {
        self.domain().violation(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_names_the_field_the_axis_and_the_bound() {
        let err = MachineDimension::AxisAcceleration.check(0.0, Some(2));
        assert_eq!(
            err.map_err(|e| e.to_string()),
            Err("machine kinematics.acceleration_xyz_mm_s2[2] = 0: must be > 0".to_owned())
        );
        assert!(MachineDimension::JunctionDeviation.check(0.0, None).is_ok());
        assert!(
            MachineDimension::JunctionDeviation
                .check(-0.01, None)
                .is_err()
        );
        assert!(MachineDimension::MaxFeed.check(f64::NAN, None).is_err());
    }

    /// Every built-in machine preset and its head are inside the domain.
    #[test]
    fn every_preset_is_inside_its_domain() {
        for (name, profile) in crate::machine::MachineProfile::presets() {
            assert_eq!(profile.validate(), Ok(()), "{name}");
            assert_eq!(profile.head.validate(), Ok(()), "{name} head");
        }
        assert_eq!(
            crate::machine::kinematics::MachineKinematics::shapeoko_xxl_ricky_tuned().validate(),
            Ok(())
        );
    }

    /// The GUI drag limits refuse nothing.
    #[test]
    fn the_ui_range_is_not_a_domain() {
        assert!(!MachineDimension::MaxFeed.ui_range().contains(&50_000.0));
        assert!(MachineDimension::MaxFeed.check(50_000.0, None).is_ok());
        assert!(MachineDimension::Aggressiveness.check(2.0, None).is_ok());
        assert!(HeadDimension::PartDiameter.violation(2000.0).is_none());
        assert!(HeadDimension::TravelEnd.violation(-900.0).is_none());
    }
}
