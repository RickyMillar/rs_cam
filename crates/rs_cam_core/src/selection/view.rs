//! The view of a resolved selection that an op can cut (DESIGN 4.2–4.4,
//! lead ruling D2/D13).
//!
//! [`view`] is the one function that turns a [`ResolvedSelection`] into a
//! refusal or into the items of the op. The op's [`ItemUse`] decides which
//! kinds it cuts ([`Shape::serves`]):
//!
//! - An item that the default rule matched and that the op cannot cut is
//!   left out. [`OpItems::skipped_by_kind`] counts it. Only a skipped open
//!   path gives a [`SelectionNote`] (by layer): an outline that is not
//!   closed is a likely drawing defect. A loop in a Drill or a point in a
//!   Pocket is expected, so it gives no note (lead ruling "notes", P2.2).
//! - An item that the user named (a pick, or a rule that names its kind)
//!   and that the op cannot cut refuses with `KindNotCut`.
//! - With nothing left to cut, the op refuses with `Empty`, which names
//!   every item left out, of every kind. A Pin Drill (`Holes { may_be_empty: true }`) may be
//!   empty.
//!
//! Containment (`detect_containment`) runs here, over the selected loops,
//! circle rings and (for `Curves`) paths, in selection order. A face keeps
//! its own inner loops and does not nest with another item.
//!
//! [`Regions`], [`Curves`] and [`Holes`] have private fields: only `view`
//! builds them, so a `Regions` holds closed polygons only.

use std::collections::BTreeMap;

use crate::compute::catalog::ItemUse;
use crate::geo::P2;
use crate::geometry::enriched_mesh::HORIZONTAL_FACE_MIN_NORMAL_Z;
use crate::polygon::{Polygon2, detect_containment};

use super::item::{Item, ItemKind, Shape};
use super::refusal::{ItemRef, RefusalCause, SelectionNote, SelectionRefusal};
use super::resolve::ResolvedSelection;

impl ItemKind {
    /// True when an op of `use_` cuts an item of this kind. The match has
    /// no wildcard, so a new kind or a new use states each answer.
    pub fn serves(self, use_: ItemUse) -> bool {
        match (self, use_) {
            (ItemKind::Loop | ItemKind::Circle | ItemKind::Face, ItemUse::Regions) => true,
            (ItemKind::Path | ItemKind::Point, ItemUse::Regions) => false,
            (ItemKind::Loop | ItemKind::Circle | ItemKind::Path, ItemUse::Curves) => true,
            (ItemKind::Point | ItemKind::Face, ItemUse::Curves) => false,
            (ItemKind::Point | ItemKind::Circle, ItemUse::Holes { .. }) => true,
            (ItemKind::Loop | ItemKind::Path | ItemKind::Face, ItemUse::Holes { .. }) => false,
            (
                ItemKind::Loop
                | ItemKind::Path
                | ItemKind::Point
                | ItemKind::Circle
                | ItemKind::Face,
                ItemUse::None,
            ) => false,
        }
    }
}

impl Shape {
    /// True when an op of `use_` cuts this shape ([`ItemKind::serves`]).
    pub fn serves(&self, use_: ItemUse) -> bool {
        self.kind().serves(use_)
    }
}

/// Lines to follow, closed and open, nested by containment.
#[derive(Debug, Clone)]
pub struct Curves {
    polygons: Vec<Polygon2>,
}

impl Curves {
    pub fn as_slice(&self) -> &[Polygon2] {
        &self.polygons
    }
}

/// Closed areas: every polygon is closed. The drawing loops and circles are
/// nested by containment; each face follows with its own inner loops.
#[derive(Debug, Clone)]
pub struct Regions {
    curves: Curves,
}

impl Regions {
    pub fn as_slice(&self) -> &[Polygon2] {
        self.curves.as_slice()
    }

    /// The same polygons as lines to follow: a closed region is also a
    /// closed curve (Trace with compensation reads this).
    pub fn as_curves(&self) -> &Curves {
        &self.curves
    }
}

/// One hole position.
#[derive(Debug, Clone, PartialEq)]
pub struct Hole {
    pub xy: P2,
    /// The circle diameter; `None` for a point.
    pub diameter: Option<f64>,
    pub layer: Option<String>,
}

/// Hole positions, in selection order. Two holes on one centre stay two
/// holes here; the drill adapter drops the repeat (`unique_holes`).
#[derive(Debug, Clone, PartialEq)]
pub struct Holes {
    holes: Vec<Hole>,
}

impl Holes {
    pub fn as_slice(&self) -> &[Hole] {
        &self.holes
    }
}

#[derive(Debug, Clone)]
enum View {
    None,
    Regions(Regions),
    Curves(Curves),
    Holes(Holes),
}

/// The items an op cuts, in the form its [`ItemUse`] reads.
#[derive(Debug, Clone)]
pub struct OpItems {
    view: View,
    skipped_by_kind: BTreeMap<ItemKind, usize>,
}

impl OpItems {
    /// The count of the items, by kind, that the default rule matched and
    /// that the op left out. Every kind counts here; only an open path
    /// also gives a [`SelectionNote`]. The `SelectionStatus` query (P2.4)
    /// reads it.
    pub fn skipped_by_kind(&self) -> &BTreeMap<ItemKind, usize> {
        &self.skipped_by_kind
    }

    /// The closed areas of a `Regions` op.
    pub fn regions(&self) -> Option<&Regions> {
        match &self.view {
            View::Regions(r) => Some(r),
            View::None | View::Curves(_) | View::Holes(_) => None,
        }
    }

    /// The lines of a `Curves` op, or the closed areas of a `Regions` op
    /// as lines.
    pub fn curves(&self) -> Option<&Curves> {
        match &self.view {
            View::Regions(r) => Some(r.as_curves()),
            View::Curves(c) => Some(c),
            View::None | View::Holes(_) => None,
        }
    }

    /// The hole positions of a `Holes` op.
    pub fn holes(&self) -> Option<&Holes> {
        match &self.view {
            View::Holes(h) => Some(h),
            View::None | View::Regions(_) | View::Curves(_) => None,
        }
    }
}

/// The items of `resolved` that an op of `item_use` cuts, and the notes for
/// the open paths the default rule matched and the op left out. The op
/// left out other kinds too; [`OpItems::skipped_by_kind`] counts every
/// kind.
///
/// The refusals, in this order:
///
/// 1. A missing source (`MissingSource`).
/// 2. A lost pick (`LostPick`) or an ambiguous pick (`AmbiguousPick`).
/// 3. A rule layer that names no item (`UnknownLayer`).
/// 4. An item the user named and the op cannot cut (`KindNotCut`), or a
///    face that is not flat and horizontal in the frame of the selection
///    (`FaceNotFlat`; call `view` after the frame door).
/// 5. Nothing left to cut (`Empty`), unless the op may be empty. `Empty`
///    names the skipped items of every kind.
///
/// The text has no op name: the caller adds it with
/// [`SelectionRefusal::for_op`].
pub fn view(
    resolved: &ResolvedSelection,
    item_use: ItemUse,
) -> Result<(OpItems, Vec<SelectionNote>), SelectionRefusal> {
    if let Some(source) = resolved.missing_sources().first() {
        return Err(SelectionRefusal::new(
            None,
            RefusalCause::MissingSource(*source),
        ));
    }
    if let Some(lost) = resolved.lost().first() {
        let cause = if lost.matches == 0 {
            RefusalCause::LostPick
        } else {
            RefusalCause::AmbiguousPick {
                matches: lost.matches,
            }
        };
        return Err(SelectionRefusal::new(
            Some(ItemRef::new(lost.key.clone())),
            cause,
        ));
    }
    if let Some(layer) = resolved.unmatched_layers().first() {
        return Err(SelectionRefusal::new(
            None,
            RefusalCause::UnknownLayer(layer.clone()),
        ));
    }

    // One note per kind and layer, every kind: `Empty` names them all.
    // The return keeps the open-path notes only.
    let mut notes: Vec<SelectionNote> = Vec::new();
    let mut skipped_by_kind: BTreeMap<ItemKind, usize> = BTreeMap::new();
    let mut served: Vec<&Item> = Vec::new();
    for r in resolved.items() {
        let shape = r.item().shape();
        if shape.serves(item_use) {
            // A face that the setup turns on its side (a lateral setup)
            // has no floor in the work plane: it refuses by name.
            if let Shape::Face(face) = shape
                && face.normal.z.abs() < HORIZONTAL_FACE_MIN_NORMAL_Z
            {
                return Err(SelectionRefusal::new(
                    Some(ItemRef::new(r.identity().clone())),
                    RefusalCause::FaceNotFlat { face: None },
                ));
            }
            served.push(r.item());
            continue;
        }
        if r.by().is_explicit() {
            return Err(SelectionRefusal::new(
                Some(ItemRef::new(r.identity().clone())),
                RefusalCause::KindNotCut {
                    kind: shape.kind(),
                    item_use,
                },
            ));
        }
        let kind = shape.kind();
        *skipped_by_kind.entry(kind).or_insert(0) += 1;
        let layer = r.item().layer();
        match notes
            .iter_mut()
            .find(|n| n.kind == kind && n.layer.as_deref() == layer)
        {
            Some(note) => note.count += 1,
            None => notes.push(SelectionNote {
                kind,
                layer: layer.map(str::to_owned),
                count: 1,
                first: ItemRef::new(r.identity().clone()),
                item_use,
            }),
        }
    }

    let empty = |notes: Vec<SelectionNote>| {
        SelectionRefusal::new(None, RefusalCause::Empty { skipped: notes })
    };
    let view = match item_use {
        ItemUse::None => View::None,
        ItemUse::Regions => {
            let polygons = regions_of(&served);
            if polygons.is_empty() {
                return Err(empty(notes));
            }
            View::Regions(Regions {
                curves: Curves { polygons },
            })
        }
        ItemUse::Curves => {
            let polygons = curves_of(&served);
            if polygons.is_empty() {
                return Err(empty(notes));
            }
            View::Curves(Curves { polygons })
        }
        ItemUse::Holes { may_be_empty } => {
            let holes: Vec<Hole> = served
                .iter()
                .filter_map(|item| {
                    let layer = item.layer().map(str::to_owned);
                    match item.shape() {
                        Shape::Point(p) => Some(Hole {
                            xy: *p,
                            diameter: None,
                            layer,
                        }),
                        Shape::Circle {
                            centre, diameter, ..
                        } => Some(Hole {
                            xy: *centre,
                            diameter: Some(*diameter),
                            layer,
                        }),
                        Shape::Loop(_) | Shape::Path(_) | Shape::Face(_) => None,
                    }
                })
                .collect();
            if holes.is_empty() && !may_be_empty {
                return Err(empty(notes));
            }
            View::Holes(Holes { holes })
        }
    };
    notes.retain(|n| n.kind == ItemKind::Path);
    Ok((
        OpItems {
            view,
            skipped_by_kind,
        },
        notes,
    ))
}

/// The closed areas of the served shapes: the loops and circle rings nested
/// by containment, then each face with its own inner loops.
fn regions_of(served: &[&Item]) -> Vec<Polygon2> {
    let mut polygons = nest_closed_rings(served.iter().copied());
    polygons.extend(served.iter().filter_map(|i| match i.shape() {
        Shape::Face(face) => Some(face.rings.clone()),
        Shape::Loop(_) | Shape::Circle { .. } | Shape::Path(_) | Shape::Point(_) => None,
    }));
    polygons
}

/// The loops and circle rings of `items`, in this order, nested by
/// containment. The one nesting of closed drawing rings: [`view`] with
/// `ItemUse::Regions` and [`super::ItemSet::closed_regions`] (the stock
/// change and obstacle outlines, P2.5) both call it. A path, a point and a
/// face give no ring here.
pub(crate) fn nest_closed_rings<'a>(items: impl IntoIterator<Item = &'a Item>) -> Vec<Polygon2> {
    let rings: Vec<Polygon2> = items
        .into_iter()
        .filter_map(|i| match i.shape() {
            Shape::Loop(p) => Some(p.clone()),
            Shape::Circle { ring, .. } => Some(ring.clone()),
            Shape::Path(_) | Shape::Point(_) | Shape::Face(_) => None,
        })
        .collect();
    detect_containment(rings)
}

/// The lines of the served shapes: loops, circle rings and paths, nested by
/// containment (a path stays top level).
fn curves_of(served: &[&Item]) -> Vec<Polygon2> {
    nest_drawing_rings(served.iter().copied())
}

/// Every drawn ring of `items` (loops, circle rings and paths), in this
/// order, nested by containment; a path stays top level. The one nesting
/// of drawn lines: [`view`] with `ItemUse::Curves` and
/// [`super::ItemSet::drawing_curves`] (the pin walls, P2.5) both call it.
pub(crate) fn nest_drawing_rings<'a>(items: impl IntoIterator<Item = &'a Item>) -> Vec<Polygon2> {
    let rings: Vec<Polygon2> = items
        .into_iter()
        .filter_map(|i| i.shape().drawing_ring().cloned())
        .collect();
    detect_containment(rings)
}
