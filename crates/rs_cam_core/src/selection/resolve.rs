//! The resolver: a [`Selection`] against the items of the models (DESIGN
//! 4.1).
//!
//! [`resolve`] is pure. It reports; it never refuses. [`super::view`] is
//! the one function that turns the report into a refusal, so generation
//! and a status query give the same answer.
//!
//! [`ResolvedSelection::to_setup_frame`] is the one frame door for the
//! items: rings, points, circle centres, and the plane Z and normal of a
//! face.

use std::collections::{BTreeMap, BTreeSet};

use crate::compute::transform::SetupTransformInfo;
use crate::geo::{BoundingBox3, P3, V3};
use crate::polygon::Polygon2;
use crate::session::LoadedModel;

use crate::ids::ModelId;

use super::item::{FaceItem, Item, ItemError, ItemSet, Shape, SourceRef};
use super::key::MatchKey;
use super::rule::Selection;
use super::stock::stock_items;

/// What a selection resolves against.
pub struct ResolveScope<'a> {
    /// The model of the toolpath (exact id), or the id it names when no
    /// model has that id. A rule or a pick on [`SourceRef::ToolpathModel`]
    /// then reports the missing source as `SourceRef::Model(id)`, so the
    /// refusal names the id (P2.4: no fallback to another model).
    pub toolpath_model: Result<&'a LoadedModel, ModelId>,
    /// Every model of the project, for [`SourceRef::Model`].
    pub models: &'a [LoadedModel],
    /// The stock in the emission frame, for [`SourceRef::Stock`].
    pub stock_bbox: &'a BoundingBox3,
    /// The setup transform; `None` for an identity setup. Only the face
    /// filter reads it: a face filter is in the SETUP frame
    /// ([`face_in_setup_frame`]). The items stay in the model frame until
    /// [`ResolvedSelection::to_setup_frame`].
    pub setup: Option<&'a SetupTransformInfo>,
}

/// How an item came into the selection. It decides what `view` does with
/// an item that the op cannot cut (lead ruling D2/D13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchedBy {
    /// The rule at this index, with `KindFilter::Served`: the default
    /// rule. An item the op cannot cut is left out with a note.
    Default { rule: usize },
    /// The rule at this index names its kinds (`KindFilter::Only`). An
    /// item the op cannot cut refuses.
    Named { rule: usize },
    /// A stored pick. An item the op cannot cut refuses.
    Pick,
}

impl MatchedBy {
    /// True for an act of the user: a pick, or a rule that names its kinds.
    pub fn is_explicit(self) -> bool {
        match self {
            MatchedBy::Default { .. } => false,
            MatchedBy::Named { .. } | MatchedBy::Pick => true,
        }
    }

    /// The rank when two acts match one item: the explicit act wins, so a
    /// picked item that the op cannot cut refuses even when the default
    /// rule also matched it.
    fn rank(self) -> u8 {
        match self {
            MatchedBy::Default { .. } => 0,
            MatchedBy::Named { .. } => 1,
            MatchedBy::Pick => 2,
        }
    }
}

/// One item of a resolved selection.
#[derive(Debug, Clone)]
pub struct ResolvedItem {
    item: Item,
    by: MatchedBy,
    /// The key of the item at resolution, in the frame of its source. It
    /// stays when the frame door moves the item, so a refusal names the
    /// item as the drawing holds it.
    identity: MatchKey,
    /// The index of the item in the item set of its source, in import
    /// order (geometry selection P4.4). For an item of a model it is the
    /// index in `LoadedModel::items`; for a stock item, the index in
    /// [`super::stock_items`].
    index: usize,
}

impl ResolvedItem {
    pub fn item(&self) -> &Item {
        &self.item
    }

    /// The index of the item in the item set of its source (P4.4).
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn by(&self) -> MatchedBy {
        self.by
    }

    pub fn identity(&self) -> &MatchKey {
        &self.identity
    }
}

/// A pick that names no item, or more than one.
#[derive(Debug, Clone, PartialEq)]
pub struct LostPick {
    pub key: MatchKey,
    /// The count of the items that match the key: 0 (lost) or 2 or more
    /// (ambiguous).
    pub matches: usize,
}

/// An item that a rule matched and an exclude removed (geometry selection
/// P4.4). A pick of the same item puts it back in the selection, so an
/// item here is in no `ResolvedItem`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExcludedItem {
    /// The source of the item (`Item::source`).
    pub source: SourceRef,
    /// The index of the item in the item set of its source.
    pub index: usize,
    /// The key of the item, in the frame of its source.
    pub key: MatchKey,
}

/// The report of [`resolve`].
#[derive(Debug, Clone)]
pub struct ResolvedSelection {
    /// The matched items: by source (in the order the selection first
    /// names it), then in import order. An item that two rules or a rule
    /// and a pick match is here once. Two items with equal keys stay two
    /// items (a duplicate entity).
    items: Vec<ResolvedItem>,
    lost: Vec<LostPick>,
    /// The items that a rule matched and an exclude removed (P4.4).
    excluded: Vec<ExcludedItem>,
    missing_sources: Vec<SourceRef>,
    /// The rule layers that name no item of their source (S21).
    unmatched_layers: Vec<String>,
}

impl ResolvedSelection {
    /// The default rule over a hand-built polygon list: the items of
    /// [`ItemSet::from_polygons`], each matched by the default rule, with
    /// nothing lost or missing. No model is needed. [`super::view`] nests
    /// the rings as it nests a drawing.
    ///
    /// The caller: `SelectionInputs` (the empty default, and
    /// `from_polygons`, which gives a hand-built list to an
    /// `ExecutionContext` with no session). The session resolves the
    /// selection of the op, face picks included (P2.3b).
    pub(crate) fn from_polygons(polygons: Vec<Polygon2>) -> Self {
        Self::from_items(ItemSet::from_polygons(polygons).iter().cloned())
    }

    /// The default rule over hand-built items (points and circles too),
    /// each matched by the default rule, with nothing lost or missing. The
    /// caller: `SelectionInputs::from_items` (tests with no session).
    pub(crate) fn from_items(items: impl IntoIterator<Item = Item>) -> Self {
        let items = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| ResolvedItem {
                identity: item.key().clone(),
                item,
                by: MatchedBy::Default { rule: 0 },
                index,
            })
            .collect();
        Self {
            items,
            lost: Vec::new(),
            excluded: Vec::new(),
            missing_sources: Vec::new(),
            unmatched_layers: Vec::new(),
        }
    }

    pub fn items(&self) -> &[ResolvedItem] {
        &self.items
    }

    pub fn lost(&self) -> &[LostPick] {
        &self.lost
    }

    /// The items that a rule matched and an exclude removed (P4.4).
    pub fn excluded(&self) -> &[ExcludedItem] {
        &self.excluded
    }

    pub fn missing_sources(&self) -> &[SourceRef] {
        &self.missing_sources
    }

    pub fn unmatched_layers(&self) -> &[String] {
        &self.unmatched_layers
    }

    /// The highest plane Z of the faces in the selection, or `None` when
    /// the selection holds no face (DESIGN 7.2). The session reads it as
    /// the auto top of an op with picked faces.
    ///
    /// Call it after [`Self::to_setup_frame`]: the plane Z is then in the
    /// SETUP frame, so a face pick on a flipped setup gives the top of the
    /// face as that setup sees it.
    pub fn face_top_z(&self) -> Option<f64> {
        self.items
            .iter()
            .filter_map(|r| match r.item.shape() {
                Shape::Face(face) => Some(face.plane_z),
                Shape::Loop(_) | Shape::Path(_) | Shape::Point(_) | Shape::Circle { .. } => None,
            })
            .filter(|z| z.is_finite())
            .reduce(f64::max)
    }

    /// THE frame door: the items of a model move into the work plane of
    /// the setup. Rings, points and circle centres take
    /// [`SetupTransformInfo::apply_to_drawing_polygons`] and
    /// [`SetupTransformInfo::drawing_to_local`] (every op selection and
    /// every boundary selection takes this door, P2.5); the plane Z and the
    /// normal of a face take [`face_in_setup_frame`].
    ///
    /// An identity setup (`needs_transform` false) changes nothing: the
    /// world IS the emission frame. A stock item does not move: the stock
    /// source is in the emission frame already.
    pub fn to_setup_frame(self, info: &SetupTransformInfo) -> Result<Self, ItemError> {
        if !info.needs_transform() {
            return Ok(self);
        }
        let items = self
            .items
            .into_iter()
            .map(|r| {
                if r.item.source() == SourceRef::Stock {
                    return Ok(r);
                }
                let shape = shape_in_setup_frame(r.item.shape(), info);
                Ok(ResolvedItem {
                    item: r.item.with_shape(shape)?,
                    by: r.by,
                    identity: r.identity,
                    index: r.index,
                })
            })
            .collect::<Result<Vec<_>, ItemError>>()?;
        Ok(Self { items, ..self })
    }
}

/// One drawing ring in the setup frame. The door maps one polygon to one
/// polygon, so the list holds exactly one.
fn ring_in_setup_frame(ring: &Polygon2, info: &SetupTransformInfo) -> Polygon2 {
    info.apply_to_drawing_polygons(std::slice::from_ref(ring))
        .into_iter()
        .next()
        .unwrap_or_else(|| ring.clone())
}

fn shape_in_setup_frame(shape: &Shape, info: &SetupTransformInfo) -> Shape {
    match shape {
        Shape::Loop(p) => Shape::Loop(ring_in_setup_frame(p, info)),
        Shape::Path(p) => Shape::Path(ring_in_setup_frame(p, info)),
        Shape::Point(p) => Shape::Point(info.drawing_to_local(*p)),
        Shape::Circle {
            centre,
            diameter,
            ring,
        } => Shape::Circle {
            centre: info.drawing_to_local(*centre),
            diameter: *diameter,
            ring: ring_in_setup_frame(ring, info),
        },
        Shape::Face(face) => {
            let (plane_z, normal) = face_in_setup_frame(face, Some(info));
            Shape::Face(FaceItem {
                rings: ring_in_setup_frame(&face.rings, info),
                plane_z,
                normal,
                brep: face.brep,
            })
        }
    }
}

/// The plane Z and the unit normal of a face in the setup frame. `None` or
/// an identity setup gives the face's own values.
///
/// The map is the 3D setup map (`world_to_local`) of a point on the plane
/// and of that point plus the normal, so the sign of the normal comes from
/// the map itself: a `Bottom` flip turns an up face down.
///
/// A lateral setup turns a horizontal model face vertical: its normal then
/// has no Z part, so a face filter admits it neither up nor down, and its
/// plane Z is a depth along the side, not a floor. [`super::view`]
/// refuses such a face (`FaceNotFlat`, P2.4).
pub fn face_in_setup_frame(face: &FaceItem, info: Option<&SetupTransformInfo>) -> (f64, V3) {
    let Some(info) = info.filter(|i| i.needs_transform()) else {
        return (face.plane_z, face.normal);
    };
    let anchor = face
        .rings
        .exterior
        .first()
        .map_or(P3::new(0.0, 0.0, face.plane_z), |p| {
            P3::new(p.x, p.y, face.plane_z)
        });
    let base = info.world_to_local(anchor);
    let tip = info.world_to_local(anchor + face.normal);
    let normal = (tip - base).normalize();
    (base.z, normal)
}

/// The item set of one source: the slot that names it for the dedup, and
/// the items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Slot {
    Model(usize),
    Stock,
}

/// The item set of `source`, and the slot that names it.
fn locate<'a>(
    scope: &ResolveScope<'a>,
    stock: Option<&'a ItemSet>,
    source: SourceRef,
) -> Option<(Slot, &'a ItemSet)> {
    match source {
        SourceRef::ToolpathModel => {
            let m = scope.toolpath_model.ok()?;
            Some((Slot::Model(m.id), &m.items))
        }
        SourceRef::Model(id) => {
            let m = scope.models.iter().find(|m| m.id == id.0)?;
            Some((Slot::Model(m.id), &m.items))
        }
        SourceRef::Stock => Some((Slot::Stock, stock?)),
    }
}

/// The source that a missing `source` names in a refusal: the toolpath
/// model by its id, so the text names the model that does not exist.
fn missing_ref(scope: &ResolveScope<'_>, source: SourceRef) -> SourceRef {
    match (source, scope.toolpath_model) {
        (SourceRef::ToolpathModel, Err(id)) => SourceRef::Model(id),
        (SourceRef::ToolpathModel, Ok(_)) | (SourceRef::Model(_) | SourceRef::Stock, _) => source,
    }
}

/// The position of the item set in `slots`; a new set goes at the end.
fn slot_pos<'a>(
    located: Option<(Slot, &'a ItemSet)>,
    slots: &mut Vec<(Slot, Vec<&'a Item>)>,
) -> Option<usize> {
    let (slot, set) = located?;
    if let Some(pos) = slots.iter().position(|(s, _)| *s == slot) {
        return Some(pos);
    }
    slots.push((slot, set.iter().collect()));
    Some(slots.len() - 1)
}

/// Resolve `sel` against `scope`. Pure: no frame change, no refusal.
///
/// - A rule matches each item of its source that its filter admits. An
///   exclude removes a rule match (not a pick).
/// - A pick matches the items of its source whose key names the same
///   geometry ([`MatchKey::names_same_geometry`]). One match: the item is
///   in. None, or two or more: the pick is lost.
/// - A source that does not exist goes into `missing_sources`.
/// - A rule layer that names no item of its source goes into
///   `unmatched_layers`.
pub fn resolve(sel: &Selection, scope: &ResolveScope<'_>) -> ResolvedSelection {
    let names_stock = sel
        .rules()
        .iter()
        .map(|r| r.source)
        .chain(sel.picks().iter().map(|k| k.source))
        .any(|s| s == SourceRef::Stock);
    let stock_set: Option<ItemSet> = if names_stock {
        stock_items(scope.stock_bbox).ok()
    } else {
        None
    };
    let stock = stock_set.as_ref();
    let face_frame = |shape: &Shape| match shape {
        Shape::Face(face) => {
            let (z, n) = face_in_setup_frame(face, scope.setup);
            (z, n.z)
        }
        Shape::Loop(_) | Shape::Path(_) | Shape::Point(_) | Shape::Circle { .. } => (0.0, 0.0),
    };

    let mut slots: Vec<(Slot, Vec<&Item>)> = Vec::new();
    let exclude_slots: Vec<Option<Slot>> = sel
        .excludes()
        .iter()
        .map(|ex| locate(scope, stock, ex.source).map(|(s, _)| s))
        .collect();

    let mut missing_sources: Vec<SourceRef> = Vec::new();
    let mut missing = |source: SourceRef| {
        let source = missing_ref(scope, source);
        if !missing_sources.contains(&source) {
            missing_sources.push(source);
        }
    };
    let mut unmatched_layers: Vec<String> = Vec::new();
    let mut found: BTreeMap<(usize, usize), MatchedBy> = BTreeMap::new();
    // The rule matches that an exclude removed, by slot position and index.
    let mut removed: BTreeSet<(usize, usize)> = BTreeSet::new();
    let add = |found: &mut BTreeMap<(usize, usize), MatchedBy>, at, by: MatchedBy| {
        found
            .entry(at)
            .and_modify(|old: &mut MatchedBy| {
                if by.rank() > old.rank() {
                    *old = by;
                }
            })
            .or_insert(by);
    };

    // Rules, less the excludes.
    for (index, rule) in sel.rules().iter().enumerate() {
        let Some(pos) = slot_pos(locate(scope, stock, rule.source), &mut slots) else {
            missing(rule.source);
            continue;
        };
        let Some((slot, items)) = slots.get(pos) else {
            continue;
        };
        for layer in &rule.filter.layers {
            let named = items.iter().any(|i| i.layer() == Some(layer.as_str()));
            if !named && !unmatched_layers.contains(layer) {
                unmatched_layers.push(layer.clone());
            }
        }
        let by = if rule.names_kinds() {
            MatchedBy::Named { rule: index }
        } else {
            MatchedBy::Default { rule: index }
        };
        for (idx, item) in items.iter().enumerate() {
            if !rule.filter.admits(item, face_frame) {
                continue;
            }
            let excluded = sel
                .excludes()
                .iter()
                .zip(&exclude_slots)
                .any(|(ex, ex_slot)| *ex_slot == Some(*slot) && ex.names_same_geometry(item.key()));
            if excluded {
                removed.insert((pos, idx));
            } else {
                add(&mut found, (pos, idx), by);
            }
        }
    }

    // Picks.
    let mut lost: Vec<LostPick> = Vec::new();
    for pick in sel.picks() {
        let Some(pos) = slot_pos(locate(scope, stock, pick.source), &mut slots) else {
            missing(pick.source);
            continue;
        };
        let Some((_, items)) = slots.get(pos) else {
            continue;
        };
        let hits: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, i)| pick.names_same_geometry(i.key()))
            .map(|(idx, _)| idx)
            .collect();
        match hits.as_slice() {
            [idx] => add(&mut found, (pos, *idx), MatchedBy::Pick),
            _ => lost.push(LostPick {
                key: pick.clone(),
                matches: hits.len(),
            }),
        }
    }

    // A pick puts an excluded item back: it is then in `found`.
    let excluded = removed
        .into_iter()
        .filter(|at| !found.contains_key(at))
        .filter_map(|(pos, idx)| {
            let item = slots.get(pos)?.1.get(idx)?;
            Some(ExcludedItem {
                source: item.source(),
                index: idx,
                key: item.key().clone(),
            })
        })
        .collect();
    let items = found
        .into_iter()
        .filter_map(|((pos, idx), by)| {
            let item = slots.get(pos)?.1.get(idx)?;
            Some(ResolvedItem {
                item: (*item).clone(),
                by,
                identity: item.key().clone(),
                index: idx,
            })
        })
        .collect();
    ResolvedSelection {
        items,
        lost,
        excluded,
        missing_sources,
        unmatched_layers,
    }
}
