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
//! its own inner loops and does not nest with another item. The stock
//! outline (an item of `SourceRef::Stock`) nests as a drawing ring does;
//! `Regions` marks the region whose exterior it is (geometry selection
//! P4.2).
//!
//! [`Regions`], [`Curves`] and [`Holes`] have private fields: only `view`
//! builds them, so a `Regions` holds closed polygons only.

use std::collections::{BTreeMap, BTreeSet};

use crate::compute::catalog::ItemUse;
use crate::geo::P2;
use crate::geometry::enriched_mesh::HORIZONTAL_FACE_MIN_NORMAL_Z;
use crate::polygon::{Polygon2, detect_containment, detect_containment_tagged};

use super::item::{Item, ItemKind, Shape, SourceRef};
use super::refusal::{ItemRef, NoteCause, RefusalCause, SelectionNote, SelectionRefusal};
use super::resolve::ResolvedSelection;
use super::rule::Facing;

impl ItemKind {
    /// True when an op of `use_` cuts an item of this kind. The match has
    /// no wildcard, so a new kind or a new use states each answer.
    pub fn serves(self, use_: ItemUse) -> bool {
        match (self, use_) {
            (ItemKind::Loop | ItemKind::Circle | ItemKind::Face, ItemUse::Regions) => true,
            (ItemKind::Path | ItemKind::Point, ItemUse::Regions) => false,
            // P4.D: a face's rings are closed lines, so a Trace or a
            // Project Curve can follow the outline of a picked face.
            (
                ItemKind::Loop | ItemKind::Circle | ItemKind::Path | ItemKind::Face,
                ItemUse::Curves,
            ) => true,
            (ItemKind::Point, ItemUse::Curves) => false,
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

/// Closed areas: every polygon is closed. The loops, circles and the stock
/// outline are nested by containment; each face follows with its own inner
/// loops.
#[derive(Debug, Clone)]
pub struct Regions {
    curves: Curves,
    /// For each polygon: true when its exterior is the stock ring.
    stock: Vec<bool>,
    /// For each polygon: the index in `ResolvedSelection::items` of the
    /// item whose ring is its exterior (geometry selection P4.6).
    exterior_items: Vec<usize>,
    /// For each polygon: the plane Z of its exterior item when that item is
    /// a face, else `None` (geometry selection P3.5).
    plane_z: Vec<Option<f64>>,
}

impl Regions {
    pub fn as_slice(&self) -> &[Polygon2] {
        self.curves.as_slice()
    }

    /// True when the exterior of the polygon at `index` of
    /// [`Self::as_slice`] is the ring of a [`SourceRef::Stock`] item. Its
    /// holes are the drawing rings inside the stock. Face grows only this
    /// region by its stock offset (geometry selection P4.2, ruling O-3).
    pub fn is_stock(&self, index: usize) -> bool {
        self.stock.get(index).copied().unwrap_or(false)
    }

    /// The index in `ResolvedSelection::items` of the item whose ring is
    /// the exterior of the polygon at `index` of [`Self::as_slice`]
    /// (geometry selection P4.6: the loop that a tab mark names). The
    /// nesting follows each ring by its input position, so the answer is
    /// the item, not a ring with equal coordinates.
    pub fn exterior_item(&self, index: usize) -> Option<usize> {
        self.exterior_items.get(index).copied()
    }

    /// The plane Z of the face whose ring is the exterior of the polygon at
    /// `index` of [`Self::as_slice`], or `None` when the exterior is a
    /// loop, a circle or the stock ring (geometry selection P3.5). [`view`]
    /// runs after the frame door, so the Z is in the SETUP frame. The floor
    /// "each picked face" reads it (`compute::floor::region_floors`).
    pub fn plane_z(&self, index: usize) -> Option<f64> {
        self.plane_z.get(index).copied().flatten()
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

/// What [`view`] does with one item of the resolved selection (geometry
/// selection P4.4). The `selection_status` report reads it as the state of
/// the item, so the viewport and the kernel cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemRole {
    /// The op cuts the item: an outer region, a curve, a hole, a face.
    Cut,
    /// A closed ring that the nesting made a hole of a region. The op cuts
    /// round it (a `Regions` op only).
    Island,
    /// The default rule matched the item and the op cannot cut its kind.
    Skipped,
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
    /// One role per item of the resolved selection, index-aligned with
    /// `ResolvedSelection::items`.
    roles: Vec<ItemRole>,
}

impl OpItems {
    /// The role of each item of the resolved selection, index-aligned with
    /// `ResolvedSelection::items` (P4.4). An island is a ring that the
    /// nesting of [`Regions`] consumed as a hole, by its input index, so the
    /// answer is the nesting the kernel cuts.
    pub fn roles(&self) -> &[ItemRole] {
        &self.roles
    }

    /// The count of the items, by kind, that the default rule matched and
    /// that the op left out. Every kind counts here; only an open path
    /// also gives a [`SelectionNote`]. The `SelectionStatus` query (P2.4)
    /// reads it.
    pub fn skipped_by_kind(&self) -> &BTreeMap<ItemKind, usize> {
        &self.skipped_by_kind
    }

    /// Remove the closed areas at `indices` of [`Regions::as_slice`] (in
    /// increasing order) and mark the item of each exterior
    /// [`ItemRole::Skipped`] (geometry selection P3.7). THE per-area floor
    /// rule (`compute::floor::region_floors`) calls it for a face whose
    /// floor is at or above the top. Every other view does not change.
    pub fn skip_regions(&mut self, indices: &[usize]) {
        let View::Regions(regions) = &mut self.view else {
            return;
        };
        for &index in indices.iter().rev() {
            if index >= regions.curves.polygons.len() {
                continue;
            }
            regions.curves.polygons.remove(index);
            if index < regions.stock.len() {
                regions.stock.remove(index);
            }
            if index < regions.plane_z.len() {
                regions.plane_z.remove(index);
            }
            if index < regions.exterior_items.len() {
                let at = regions.exterior_items.remove(index);
                if let Some(role) = self.roles.get_mut(at) {
                    *role = ItemRole::Skipped;
                }
            }
        }
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
/// 1. A missing source (`MissingSource`), then a mesh model whose flat
///    faces are not computed (`FacesNotReady`, P4.7).
/// 2. A lost pick (`LostPick`) or an ambiguous pick (`AmbiguousPick`).
/// 3. A rule layer that names no item (`UnknownLayer`).
/// 4. An item the user named and the op cannot cut (`KindNotCut`), or a
///    face that is not flat and horizontal in the frame of the selection
///    (`FaceNotFlat`; call `view` after the frame door), or a face in the
///    closed areas of the op that points down in the setup frame
///    (`FaceFacesDown`, P3.4).
/// 5. Nothing left to cut (`Empty`), unless the op may be empty. `Empty`
///    names the skipped items of every kind.
///
/// The text has no op name: the caller adds it with
/// [`SelectionRefusal::for_op`].
///
/// This is the view of an OP selection. A machining boundary reads
/// [`view_boundary`].
pub fn view(
    resolved: &ResolvedSelection,
    item_use: ItemUse,
) -> Result<(OpItems, Vec<SelectionNote>), SelectionRefusal> {
    view_with(resolved, item_use, DownFaces::Refuse)
}

/// The view of a machining boundary selection: [`view`] with
/// `crate::session::selection_status::BOUNDARY_ITEM_USE`, except that a
/// face that points down in the setup frame gives its outline (geometry
/// selection P3.4, DESIGN 6 rule 4). A boundary reads the outline only, not
/// a Z, so the facing does not matter. The boundary generation and the
/// boundary report both read it, so the two give one answer.
pub fn view_boundary(
    resolved: &ResolvedSelection,
) -> Result<(OpItems, Vec<SelectionNote>), SelectionRefusal> {
    view_with(
        resolved,
        crate::session::selection_status::BOUNDARY_ITEM_USE,
        DownFaces::Keep,
    )
}

/// What [`view_with`] does with a face of a `Regions` reader that points
/// down in the setup frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownFaces {
    /// An op selection: the face refuses `FaceFacesDown`.
    Refuse,
    /// A boundary selection: the face gives its outline.
    Keep,
}

fn view_with(
    resolved: &ResolvedSelection,
    item_use: ItemUse,
    down_faces: DownFaces,
) -> Result<(OpItems, Vec<SelectionNote>), SelectionRefusal> {
    if let Some(source) = resolved.missing_sources().first() {
        return Err(SelectionRefusal::new(
            None,
            RefusalCause::MissingSource(*source),
        ));
    }
    if let Some(model) = resolved.faces_not_ready().first() {
        return Err(SelectionRefusal::new(
            None,
            RefusalCause::FacesNotReady { model: *model },
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
    // For each served item: its index in `resolved.items()`.
    let mut served_at: Vec<usize> = Vec::new();
    let mut roles: Vec<ItemRole> = vec![ItemRole::Skipped; resolved.items().len()];
    for (at, r) in resolved.items().iter().enumerate() {
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
            // P3.4 (L-9): a down face has its material above its plane. It
            // gives no closed area to an op; a boundary keeps its outline.
            if let Shape::Face(face) = shape
                && matches!(item_use, ItemUse::Regions)
                && down_faces == DownFaces::Refuse
                && Facing::Down.admits(face.normal.z)
            {
                return Err(SelectionRefusal::new(
                    Some(ItemRef::new(r.identity().clone())),
                    RefusalCause::FaceFacesDown,
                ));
            }
            served.push(r.item());
            served_at.push(at);
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
                cause: NoteCause::KindNotServed,
            }),
        }
    }

    let empty = |notes: Vec<SelectionNote>| {
        SelectionRefusal::new(None, RefusalCause::Empty { skipped: notes })
    };
    let view = match item_use {
        ItemUse::None => View::None,
        ItemUse::Regions => {
            let RegionParts {
                polygons,
                stock,
                exteriors,
                plane_z,
                islands,
            } = regions_of(&served);
            if polygons.is_empty() {
                return Err(empty(notes));
            }
            for (pos, at) in served_at.iter().enumerate() {
                if let Some(role) = roles.get_mut(*at) {
                    *role = if islands.contains(&pos) {
                        ItemRole::Island
                    } else {
                        ItemRole::Cut
                    };
                }
            }
            View::Regions(Regions {
                curves: Curves { polygons },
                stock,
                exterior_items: exteriors
                    .into_iter()
                    .filter_map(|pos| served_at.get(pos).copied())
                    .collect(),
                plane_z,
            })
        }
        ItemUse::Curves => {
            let polygons = curves_of(&served);
            if polygons.is_empty() {
                return Err(empty(notes));
            }
            mark_cut(&mut roles, &served_at);
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
            mark_cut(&mut roles, &served_at);
            View::Holes(Holes { holes })
        }
    };
    notes.retain(|n| n.kind == ItemKind::Path);
    Ok((
        OpItems {
            view,
            skipped_by_kind,
            roles,
        },
        notes,
    ))
}

/// Mark every served item `Cut` (an op that does not nest: `Curves`,
/// `Holes`).
fn mark_cut(roles: &mut [ItemRole], served_at: &[usize]) {
    for at in served_at {
        if let Some(role) = roles.get_mut(*at) {
            *role = ItemRole::Cut;
        }
    }
}

/// The closed areas of the served shapes, and what [`Regions`] keeps of
/// each ([`regions_of`]).
struct RegionParts {
    polygons: Vec<Polygon2>,
    /// True for each area whose exterior is the stock ring (P4.2).
    stock: Vec<bool>,
    /// For each area, the position in `served` of the item whose ring is
    /// its exterior (P4.6).
    exteriors: Vec<usize>,
    /// For each area, the plane Z of its exterior face, or `None` (P3.5).
    plane_z: Vec<Option<f64>>,
    /// The positions in `served` of the rings that became holes (P4.4).
    islands: BTreeSet<usize>,
}

/// The closed areas of the served shapes: the loops and circle rings
/// nested by containment, then each face with its own inner loops. The
/// stock ring nests as any drawing ring does, so a drawing loop inside the
/// stock is a hole of the stock region (a Pocket clears round it).
fn regions_of(served: &[&Item]) -> RegionParts {
    let nested = nest_closed_rings_at(served);
    let mut parts = RegionParts {
        polygons: Vec::new(),
        stock: Vec::new(),
        exteriors: Vec::new(),
        plane_z: Vec::new(),
        islands: nested.holes,
    };
    for (polygon, at) in nested.regions {
        let from_stock = served
            .get(at)
            .is_some_and(|i| i.source() == SourceRef::Stock);
        parts.polygons.push(polygon);
        parts.stock.push(from_stock);
        parts.exteriors.push(at);
        parts.plane_z.push(None);
    }
    for (at, face) in served
        .iter()
        .enumerate()
        .filter_map(|(at, i)| match i.shape() {
            Shape::Face(face) => Some((at, face)),
            Shape::Loop(_) | Shape::Circle { .. } | Shape::Path(_) | Shape::Point(_) => None,
        })
    {
        parts.polygons.push(face.rings.clone());
        parts.stock.push(false);
        parts.exteriors.push(at);
        parts.plane_z.push(Some(face.plane_z));
    }
    parts
}

/// The loops and circle rings of `items`, in this order, nested by
/// containment. The one nesting of closed drawing rings: [`view`] with
/// `ItemUse::Regions` and [`super::ItemSet::closed_regions`] (the stock
/// change and obstacle outlines, P2.5) both call it. A path, a point and a
/// face give no ring here.
pub(crate) fn nest_closed_rings<'a>(items: impl IntoIterator<Item = &'a Item>) -> Vec<Polygon2> {
    let items: Vec<&Item> = items.into_iter().collect();
    nest_closed_rings_at(&items)
        .regions
        .into_iter()
        .map(|(polygon, _)| polygon)
        .collect()
}

/// The one nesting of closed drawing rings, by input position.
struct NestedRings {
    /// Each region, with the position in the input of the item whose ring
    /// is its exterior. The position follows the ring through the nesting
    /// by its input index, not by its coordinates (P4.2).
    regions: Vec<(Polygon2, usize)>,
    /// The positions in the input of the items whose ring became a hole of
    /// a region: the rings that are not the exterior of a region (P4.4).
    holes: BTreeSet<usize>,
}

/// [`nest_closed_rings`] by input position: the loops and circle rings of
/// `items`, nested by `detect_containment_tagged`.
fn nest_closed_rings_at(items: &[&Item]) -> NestedRings {
    let (rings, ring_at): (Vec<Polygon2>, Vec<usize>) = items
        .iter()
        .enumerate()
        .filter_map(|(at, i)| {
            let ring = match i.shape() {
                Shape::Loop(p) => p.clone(),
                Shape::Circle { ring, .. } => ring.clone(),
                Shape::Path(_) | Shape::Point(_) | Shape::Face(_) => return None,
            };
            Some((ring, at))
        })
        .unzip();
    let nested = detect_containment_tagged(rings);
    let exteriors: BTreeSet<usize> = nested.iter().map(|(_, index)| *index).collect();
    let holes = ring_at
        .iter()
        .enumerate()
        .filter(|(index, _)| !exteriors.contains(index))
        .map(|(_, at)| *at)
        .collect();
    let regions = nested
        .into_iter()
        .filter_map(|(polygon, index)| Some((polygon, *ring_at.get(index)?)))
        .collect();
    NestedRings { regions, holes }
}

/// The lines of the served shapes: loops, circle rings and paths, nested by
/// containment (a path stays top level), then each face with its own inner
/// loops (geometry selection P4.D), as [`regions_of`] adds them.
fn curves_of(served: &[&Item]) -> Vec<Polygon2> {
    let mut polygons = nest_drawing_rings(served.iter().copied());
    polygons.extend(served.iter().filter_map(|i| match i.shape() {
        Shape::Face(face) => Some(face.rings.clone()),
        Shape::Loop(_) | Shape::Circle { .. } | Shape::Path(_) | Shape::Point(_) => None,
    }));
    polygons
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
