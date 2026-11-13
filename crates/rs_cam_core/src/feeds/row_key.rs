//! Feeds contract P29-11 (commit L-2, `P29-11_DESIGN.md` section 5.4, sites
//! S1 and S2): the vendor row key of an operation.
//!
//! The vendor tables print rows by the op's feeds family and pass role, and
//! the a4 routing reads the op kind (`vendor_normalize::lut_query_for`). These
//! three values select a ROW. They are not a scope of a limit: a limit reads
//! the facts of each cut (`limits::CutFacts`, `OperationConfig::cuts`).
//!
//! [`RowKey`] holds the three values in private fields, and
//! [`RowKey::of_operation`] is its one constructor. The calculator input
//! (`FeedsInput`) and the gate context (`tool_load::ToolpathLoadContext`)
//! carry the key, not the values. Four methods open it:
//!
//! - [`RowKey::feeds_style`] and [`RowKey::op_type`]. Their names are tokens
//!   of the P29-last guard (`tests/no_limit_reads_the_op_type_p29_last.rs`),
//!   so each call is a hit: the guard allows it only in a vendor module
//!   (class V) and never in a limit file.
//! - [`RowKey::lut_route`] (the row routing rule of the kind) and
//!   [`RowKey::routing_label`] (the kind in a warning text). The guard does
//!   NOT see a call of these two: their names are not tokens. The
//!   calculator's chip step (`calculator/chip.rs`) calls both, for the row
//!   routing and its warning. A new reader of the key calls one of the two
//!   token methods, so that the guard sees it.

use crate::compute::catalog::{OperationConfig, OperationType};

use super::{OperationFamily, PassRole};

/// The vendor row key of one operation: its feeds family, its pass role and
/// its kind. See the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowKey {
    family: OperationFamily,
    role: PassRole,
    kind: OperationType,
}

impl RowKey {
    /// The row key of `operation`: its registry feeds style and its kind.
    #[must_use]
    pub fn of_operation(operation: &OperationConfig) -> Self {
        let (family, role) = operation.feeds_style();
        Self {
            family,
            role,
            kind: operation.op_type(),
        }
    }

    /// The feeds family and the pass role of the vendor row. Only a vendor
    /// module opens the key (the guard holds it).
    #[must_use]
    pub const fn feeds_style(self) -> (OperationFamily, PassRole) {
        (self.family, self.role)
    }

    /// The op kind of the a4 row routing. Only a vendor module opens the
    /// key (the guard holds it).
    #[must_use]
    pub const fn op_type(self) -> OperationType {
        self.kind
    }

    /// The row routing rule of the op kind (`Capabilities::lut_route`, D2).
    #[must_use]
    pub fn lut_route(self) -> crate::compute::catalog::LutRoute {
        self.kind.capabilities().lut_route
    }

    /// The op kind as the warning `FeedsWarning::NoVendorRowsForRoutedOperation`
    /// prints it (the text of the old optional field, `Some(<kind>)`).
    #[must_use]
    pub fn routing_label(self) -> String {
        format!("{:?}", Some(self.kind))
    }
}
