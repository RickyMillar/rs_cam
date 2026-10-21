//! The hole fit: is a hole of the tool size (geometry selection P6.5,
//! operator ruling O-3, design 4.2).
//!
//! A circle states the size of its hole (S11). A Drill makes a hole of the
//! tool size only, so a circle of another size is not a hole the Drill can
//! make. [`hole_fit`](fn@hole_fit) is the one rule that compares the two diameters.
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

/// How one hole compares with the tool (design 4.2). `d` is the circle
/// diameter, `D` the tool diameter and `e` [`CAD_ROUNDING_MM`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleFit {
    /// A point: it states no size.
    NoSize,
    /// `|d - D| <= e`: the tool makes this hole.
    ToolSize,
    /// `D + e < d <= 2 D`: one helix of the tool round the centre makes the
    /// hole.
    Larger,
    /// `d > 2 D`: one helix of the tool round the centre leaves a core.
    LeavesCore,
    /// `d < D - e`: the tool is larger than the hole.
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
/// a tool of `tool_diameter` (design 4.2). Two diameters closer than
/// [`CAD_ROUNDING_MM`] are one size. The `2 D` bound is geometry: one
/// helix of radius `r` sweeps the ring `r - D/2 .. r + D/2`, and it reaches
/// the centre only when `r <= D/2`, so when `d <= 2 D`.
pub fn hole_fit(diameter: Option<f64>, tool_diameter: f64) -> HoleFit {
    let Some(d) = diameter else {
        return HoleFit::NoSize;
    };
    if (d - tool_diameter).abs() <= CAD_ROUNDING_MM {
        HoleFit::ToolSize
    } else if d < tool_diameter {
        HoleFit::Smaller
    } else if d > 2.0 * tool_diameter {
        HoleFit::LeavesCore
    } else {
        HoleFit::Larger
    }
}

/// The fit step: apply [`hole_fit`](fn@hole_fit) to the holes of `items` (the view of
/// `resolved` for `item_use`) with a tool of `tool_diameter`. It returns the
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
    tool_diameter: f64,
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
        let fit = hole_fit(hole.diameter, tool_diameter);
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
                RefusalCause::HoleDoesNotFit { fit, tool_diameter },
            ));
        }
        skipped.push(index);
        notes.push(SelectionNote {
            kind: r.item().kind(),
            layer: r.item().layer().map(str::to_owned),
            count: 1,
            first,
            item_use,
            cause: NoteCause::HoleFit { fit, tool_diameter },
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
        assert_eq!(hole_fit(None, tool), HoleFit::NoSize);
        assert_eq!(hole_fit(Some(tool), tool), HoleFit::ToolSize);
        assert_eq!(hole_fit(Some(tool + 0.999 * e), tool), HoleFit::ToolSize);
        assert_eq!(hole_fit(Some(tool - 0.999 * e), tool), HoleFit::ToolSize);
        assert_eq!(hole_fit(Some(tool + 1.001 * e), tool), HoleFit::Larger);
        assert_eq!(hole_fit(Some(tool - 1.001 * e), tool), HoleFit::Smaller);
        // 1/4 in and 6 mm are two sizes, both ways.
        assert_eq!(hole_fit(Some(6.0), 6.35), HoleFit::Smaller);
        assert_eq!(hole_fit(Some(6.35), 6.0), HoleFit::Larger);
        assert_eq!(hole_fit(Some(2.0 * tool), tool), HoleFit::Larger);
        assert_eq!(hole_fit(Some(2.0 * tool + 0.1), tool), HoleFit::LeavesCore);
    }
}
