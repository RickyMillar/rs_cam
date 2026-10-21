//! The hole fit: is a hole of the tool size (geometry selection P6.5,
//! operator ruling O-3, design 4.2).
//!
//! A circle states the size of its hole (S11). A Drill makes a hole of the
//! tool size only, so a circle of another size is not a hole the Drill can
//! make. [`hole_fit`](fn@hole_fit) is the one rule that compares the two diameters.
//! The op states how far apart two diameters can be and still be one size
//! ([`DrillSize::tolerance`], the Drill parameter `size_tolerance`, operator
//! ruling 2026-10-22).
//! [`fit_holes`] applies it to the holes of an op after [`view`]: the fit
//! needs the tool, and `view` does not have it.
//!
//! - Under the default rule a circle that does not fit is left out, with
//!   one [`SelectionNote`] for each circle (lead ruling L-18). Its role
//!   becomes [`ItemRole::Skipped`].
//! - A circle that the user named (a pick, or a rule that names its kind)
//!   and that does not fit refuses with [`RefusalCause::HoleDoesNotFit`].
//! - A point states no size: it fits ([`HoleFit::NoSize`]).
//! - With no hole left, an op that may not be empty refuses `Empty`, which
//!   names the circles it left out.
//!
//! The prologue (`compute/execute/prologue.rs`), the drill-op view
//! (`compute/execute/drilling.rs`) and the `selection_status` report call
//! [`fit_holes`], so the cut, the drill op and the item states agree.
//!
//! [`view`]: fn@super::view
//! [`ItemRole::Skipped`]: super::ItemRole::Skipped

use crate::compute::catalog::ItemUse;

use super::key::CAD_ROUNDING_MM;
use super::refusal::{ItemRef, NoteCause, RefusalCause, SelectionNote, SelectionRefusal};
use super::resolve::ResolvedSelection;
use super::view::OpItems;

/// Which holes an op of [`ItemUse::Holes`] cuts, by their size (design
/// 4.5, lead ruling L-19). The fit step matches it with no wildcard, so a
/// new rule states what it keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleFitRule {
    /// Every hole, of any size. Pin Drill: its stock pins have their own
    /// diameter, and its extra holes are drilled at the tool size.
    None,
    /// The points, and the circles of the tool size ([`HoleFit::NoSize`],
    /// [`HoleFit::ToolSize`]). Drill.
    ToolSize,
}

/// The size a tool drills: its diameter `D` and the tolerance `t` of the
/// op. A circle whose diameter is within `t` of `D` (either side) is a hole
/// of the tool size. The default `t` is [`CAD_ROUNDING_MM`]: the CAD
/// rounding only (P6.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrillSize {
    /// The tool diameter `D` (mm).
    pub diameter: f64,
    /// The size tolerance `t` (mm), at least [`CAD_ROUNDING_MM`].
    pub tolerance: f64,
}

impl DrillSize {
    /// The tool size of a tool of `diameter` with the default tolerance
    /// ([`CAD_ROUNDING_MM`]).
    pub fn strict(diameter: f64) -> Self {
        Self {
            diameter,
            tolerance: CAD_ROUNDING_MM,
        }
    }

    /// True when the tolerance is larger than the default. The texts then
    /// name it. The op domain holds `t >= CAD_ROUNDING_MM`.
    pub fn has_wide_tolerance(&self) -> bool {
        self.tolerance > CAD_ROUNDING_MM
    }
}

/// How one hole compares with the tool (design 4.2). `d` is the circle
/// diameter, `D` the tool diameter and `t` the tolerance of the
/// [`DrillSize`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleFit {
    /// A point: it states no size.
    NoSize,
    /// `|d - D| <= t` and `d <= 2 D`: the tool makes this hole.
    ToolSize,
    /// `D + t < d <= 2 D`: one helix of the tool round the centre makes the
    /// hole.
    Larger,
    /// `d > 2 D`: one helix of the tool round the centre leaves a core. No
    /// tolerance makes such a circle a fit.
    LeavesCore,
    /// `d < D - t`: the tool is larger than the hole.
    Smaller,
}

impl HoleFit {
    /// True when the tool makes the hole: a point, or a circle of the tool
    /// size.
    pub fn is_tool_size(self) -> bool {
        match self {
            HoleFit::NoSize | HoleFit::ToolSize => true,
            HoleFit::Larger | HoleFit::LeavesCore | HoleFit::Smaller => false,
        }
    }
}

/// THE hole fit: how a hole of `diameter` (`None` for a point) compares with
/// a tool of `size` (design 4.2). Two diameters closer than
/// `size.tolerance` are one size. The `2 D` bound is geometry: one helix of
/// radius `r` sweeps the ring `r - D/2 .. r + D/2`, and it reaches the
/// centre only when `r <= D/2`, so when `d <= 2 D`.
///
/// The `2 D` test comes first. Thus the tolerance has an upper bound that
/// the tool gives: no tolerance turns a circle that leaves a core into a
/// fit. On the small side a tolerance of `D` or more admits every circle
/// smaller than the tool; the op domain does not refuse it, because the
/// domain does not know the tool.
pub fn hole_fit(diameter: Option<f64>, size: &DrillSize) -> HoleFit {
    let Some(d) = diameter else {
        return HoleFit::NoSize;
    };
    let tool = size.diameter;
    if d > 2.0 * tool {
        HoleFit::LeavesCore
    } else if (d - tool).abs() <= size.tolerance {
        HoleFit::ToolSize
    } else if d < tool {
        HoleFit::Smaller
    } else {
        HoleFit::Larger
    }
}

/// The fit step: apply [`hole_fit`](fn@hole_fit) to the holes of `items` (the view of
/// `resolved` for `item_use`) with a tool of `size`. It returns the
/// notes for the circles it left out. Every other view, and the rule
/// [`HoleFitRule::None`], does not change.
///
/// The refusals, in selection order: a circle the user named that does not
/// fit (`HoleDoesNotFit`); then, with no hole left, `Empty` (unless the op
/// may be empty).
pub fn fit_holes(
    resolved: &ResolvedSelection,
    items: &mut OpItems,
    item_use: ItemUse,
    size: DrillSize,
) -> Result<Vec<SelectionNote>, SelectionRefusal> {
    let ItemUse::Holes { may_be_empty, fit } = item_use else {
        return Ok(Vec::new());
    };
    match fit {
        HoleFitRule::None => return Ok(Vec::new()),
        HoleFitRule::ToolSize => {}
    }
    let Some(holes) = items.holes() else {
        return Ok(Vec::new());
    };
    let mut skipped = Vec::new();
    let mut notes = Vec::new();
    for (index, hole) in holes.as_slice().iter().enumerate() {
        let fit = hole_fit(hole.diameter, &size);
        if fit.is_tool_size() {
            continue;
        }
        let Some(r) = holes.item(index).and_then(|at| resolved.items().get(at)) else {
            continue;
        };
        let first = ItemRef::new(r.identity().clone());
        if r.by().is_explicit() {
            return Err(SelectionRefusal::new(
                Some(first),
                RefusalCause::HoleDoesNotFit { fit, size },
            ));
        }
        skipped.push(index);
        notes.push(SelectionNote {
            kind: r.item().kind(),
            layer: r.item().layer().map(str::to_owned),
            count: 1,
            first,
            item_use,
            cause: NoteCause::HoleFit { fit, size },
        });
    }
    items.skip_holes(&skipped);
    let none_left = items.holes().is_some_and(|h| h.as_slice().is_empty());
    if none_left && !may_be_empty {
        return Err(SelectionRefusal::new(
            None,
            RefusalCause::Empty { skipped: notes },
        ));
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fit of each band, and the tolerance `e` on both sides of `D`.
    /// `D + e` itself is not tested: in floating point `6.35 + 0.01 - 6.35`
    /// is not exactly `0.01`.
    #[test]
    fn each_band_has_its_fit_p6_5() {
        let tool = 6.35;
        let e = CAD_ROUNDING_MM;
        let s = DrillSize::strict(tool);
        assert_eq!(hole_fit(None, &s), HoleFit::NoSize);
        assert_eq!(hole_fit(Some(tool), &s), HoleFit::ToolSize);
        assert_eq!(hole_fit(Some(tool + 0.999 * e), &s), HoleFit::ToolSize);
        assert_eq!(hole_fit(Some(tool - 0.999 * e), &s), HoleFit::ToolSize);
        assert_eq!(hole_fit(Some(tool + 1.001 * e), &s), HoleFit::Larger);
        assert_eq!(hole_fit(Some(tool - 1.001 * e), &s), HoleFit::Smaller);
        // 1/4 in and 6 mm are two sizes, both ways.
        assert_eq!(hole_fit(Some(6.0), &s), HoleFit::Smaller);
        assert_eq!(
            hole_fit(Some(6.35), &DrillSize::strict(6.0)),
            HoleFit::Larger
        );
        assert_eq!(hole_fit(Some(2.0 * tool), &s), HoleFit::Larger);
        assert_eq!(hole_fit(Some(2.0 * tool + 0.1), &s), HoleFit::LeavesCore);
    }

    /// A wide tolerance widens the tool size on both sides, and it never
    /// makes a circle that leaves a core a fit (the `2 D` test is first).
    #[test]
    fn a_wide_tolerance_stops_at_the_core_bound() {
        let wide = DrillSize {
            diameter: 6.0,
            tolerance: 0.2,
        };
        assert_eq!(hole_fit(Some(5.859), &wide), HoleFit::ToolSize);
        assert_eq!(hole_fit(Some(6.25), &wide), HoleFit::Larger);
        assert_eq!(hole_fit(Some(5.75), &wide), HoleFit::Smaller);
        let huge = DrillSize {
            diameter: 6.0,
            tolerance: 100.0,
        };
        assert_eq!(hole_fit(Some(12.0), &huge), HoleFit::ToolSize);
        assert_eq!(hole_fit(Some(12.1), &huge), HoleFit::LeavesCore);
    }
}
