//! **The planned cuts on the width x depth plane** (feeds contract P29-10
//! 10g, `P29-10_DESIGN.md` 3.8).
//!
//! [`PlaneMarks::at`] places each declared cut of an op at one point of the
//! plane: the body at the point; a cut that declares the slot width (a slot
//! ring, an entry) at the tool diameter; a planner cap at its own width
//! (`min(m x ae, D)`); an entry at its own depth. The positions read the
//! declared width and depth rules (`planned::DeclaredCuts::planned`), so no
//! judgement runs and no number of a cut moves. A cut with no width or no
//! depth on the plane (a plunge, a cut with no width rule) has no mark.
//!
//! The envelope places the marks twice: at the proposal (the point of its
//! planned rows, so each mark is tied to the rows of its cut) and at the
//! stated point (the cut that the op holds, which the simulation samples
//! show). The Passes plot draws the first set with the tone of the rows;
//! the simulation chart draws the second set with no tone, unless the two
//! points are one point (`describes_rows`).

use serde::Serialize;

use super::envelope::CutPoint;
use super::planned::{BodyColumn, DeclaredCuts, PlaneDepth};
use crate::limits::{CutKind, Occurs, PlanePoint};

/// One declared cut on the plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PlaneMark {
    /// The declared cut.
    #[serde(serialize_with = "cut_label")]
    pub cut: CutKind,
    /// Whether the plan always cuts it.
    pub occurs: Occurs,
    /// Its point on the plane.
    pub at: PlanePoint,
}

fn cut_label<S: serde::Serializer>(cut: &CutKind, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(cut.label())
}

/// The declared cuts of one op at one point of the plane.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlaneMarks {
    point: CutPoint,
    describes_rows: bool,
    marks: Vec<PlaneMark>,
}

impl PlaneMarks {
    /// The marks of `cuts` at `point`. `describes_rows`: the planned rows
    /// of the envelope describe this point (it is the proposal point).
    #[must_use]
    pub fn at(cuts: &DeclaredCuts, point: CutPoint, describes_rows: bool) -> Self {
        let column = BodyColumn {
            ae_mm: point.ae_mm,
            chip_mm: None,
            stored: None,
        };
        let marks = cuts
            .list()
            .iter()
            .filter_map(|cut| {
                let planned = cuts.planned(cut, column);
                let (ap_mm, own_depth) = match planned.depth {
                    PlaneDepth::Body => (point.ap_mm, false),
                    PlaneDepth::Own(own) => (own.mm, true),
                    PlaneDepth::Axial => return None,
                };
                let ae_mm = planned.width_mm.ok()?;
                let usable = |v: f64| v.is_finite() && v >= 0.0;
                (usable(ae_mm) && usable(ap_mm)).then_some(PlaneMark {
                    cut: cut.kind,
                    occurs: match cut.presence {
                        crate::compute::catalog::Presence::Always => Occurs::Always,
                        crate::compute::catalog::Presence::MayOccur(_) => Occurs::MayOccur,
                    },
                    at: PlanePoint {
                        ae_mm,
                        ap_mm,
                        own_depth,
                    },
                })
            })
            .collect();
        Self {
            point,
            describes_rows,
            marks,
        }
    }

    /// The point the marks are placed at.
    #[must_use]
    pub const fn point(&self) -> CutPoint {
        self.point
    }

    /// `true` when the planned rows of the envelope describe this point
    /// (the proposal point), so a mark may take the tone of its rows.
    #[must_use]
    pub const fn describes_rows(&self) -> bool {
        self.describes_rows
    }

    /// The marks, in the declaration order of the cuts.
    #[must_use]
    pub fn marks(&self) -> &[PlaneMark] {
        &self.marks
    }

    /// The mark of `cut`, if the cut has one.
    #[must_use]
    pub fn of_cut(&self, cut: CutKind) -> Option<&PlaneMark> {
        self.marks.iter().find(|m| m.cut == cut)
    }
}
