//! The validity domain of each numeric stock field (F210, W6b).
//!
//! [`StockDimension::domain`] holds the hard bounds. Each bound comes from
//! the geometry:
//!
//! - every value is finite;
//! - `x`, `y` and `z` are the edge lengths of the stock box, so `> 0`: a
//!   box with an edge of 0 holds no material;
//! - `padding` is the margin `update_from_bbox` adds on each side of the
//!   model box, a length, so `>= 0`;
//! - `origin_x`, `origin_y` and `origin_z` are a position in the setup
//!   frame. A frame can put the stock anywhere, so they are finite only.
//!
//! [`crate::compute::stock_config::StockConfig::validate`] reads it. The
//! project loader calls it.
//!
//! [`StockDimension::ui_range`] is the drag range of the GUI stock panel.
//! No door refuses a stock by it.

use std::ops::RangeInclusive;

use crate::tool::domain::{Bound, Requirement, ToolDomain};

/// A numeric stock field that has a domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StockDimension {
    /// The X edge length, mm.
    X,
    /// The Y edge length, mm.
    Y,
    /// The Z edge length (the thickness), mm.
    Z,
    /// The X of the min corner, mm.
    OriginX,
    /// The Y of the min corner, mm.
    OriginY,
    /// The Z of the min corner, mm.
    OriginZ,
    /// The auto-from-model margin, mm.
    Padding,
}

impl StockDimension {
    /// The field name as spelled in project TOML and on the MCP wire.
    #[must_use]
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
            Self::OriginX => "origin_x",
            Self::OriginY => "origin_y",
            Self::OriginZ => "origin_z",
            Self::Padding => "padding",
        }
    }

    /// The hard validity domain (module doc). The type is the tool
    /// domain type of F029; it holds no tool-only rule.
    #[must_use]
    pub const fn domain(self) -> ToolDomain {
        use Bound::{Closed, Open, Unbounded};
        match self {
            Self::X | Self::Y | Self::Z => ToolDomain {
                lower: Open(0.0),
                upper: Unbounded,
            },
            Self::OriginX | Self::OriginY | Self::OriginZ => ToolDomain {
                lower: Unbounded,
                upper: Unbounded,
            },
            Self::Padding => ToolDomain {
                lower: Closed(0.0),
                upper: Unbounded,
            },
        }
    }

    /// The drag range of the GUI stock panel.
    ///
    /// A GUI convenience, NOT a domain: no door refuses a stock by it. No
    /// printed source gives these numbers; they are the panel literals
    /// from before F210, kept so the drag widgets behave as they did.
    #[must_use]
    pub const fn ui_range(self) -> RangeInclusive<f64> {
        match self {
            Self::X | Self::Y | Self::Z => 0.1..=10000.0,
            Self::OriginX | Self::OriginY | Self::OriginZ => f64::MIN..=f64::MAX,
            Self::Padding => 0.0..=100.0,
        }
    }

    /// Check `value` against [`Self::domain`].
    ///
    /// # Errors
    ///
    /// [`StockDomainError`] with the rule the value breaks.
    pub fn check(self, value: f64) -> Result<(), StockDomainError> {
        match self.domain().violation(value) {
            None => Ok(()),
            Some(requirement) => Err(StockDomainError {
                dimension: self,
                value,
                requirement,
            }),
        }
    }
}

/// A stock field outside its domain.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[error("stock {} = {value}: must be {requirement}", dimension.field_name())]
pub struct StockDomainError {
    /// The field.
    pub dimension: StockDimension,
    /// The value.
    pub value: f64,
    /// The rule it breaks.
    pub requirement: Requirement,
}
