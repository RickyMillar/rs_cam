//! The pickable 2D geometry of a model: [`Item`] and [`ItemSet`].
//!
//! An item is one loop, open path, point, circle or flat face. Each import
//! door (DXF, SVG, STEP) and the stock source make items through
//! [`Item::new`]. That constructor checks the shape invariants and computes
//! the [`MatchKey`], so no item holds a key that does not match its shape,
//! and no `Loop` is open.
//!
//! An item holds `Polygon2` rings. It does not replace `Polygon2` in the
//! kernels. Containment (islands) is not part of an item: a loop has no
//! holes, and the nesting runs over the selected loops (DESIGN 4.4).

use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::geo::{P2, V3};
use crate::geometry::enriched_mesh::FaceGroupId;
use crate::ids::ModelId;
use crate::polygon::Polygon2;

use super::key::MatchKey;

/// The arc step of [`Shape::circle`]: the arc tolerance (degrees) that the
/// model door passes to the DXF door (`io::load_geometry`).
const CIRCLE_ARC_STEP_DEG: f64 = 5.0;

/// Where an item comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRef {
    /// The model of the toolpath (`ToolpathConfig::model_id`). An item set
    /// that is not in a session yet uses this value; the session binds it
    /// to [`SourceRef::Model`] when the model enters
    /// ([`ItemSet::bound_to`]).
    ToolpathModel,
    /// A named project model (`LoadedModel::id`). Exact id; no fallback.
    Model(ModelId),
    /// The stock of the setup, in the emission frame.
    Stock,
}

/// The fieldless mirror of [`Shape`], for rules, keys and refusal text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Loop,
    Path,
    Point,
    Circle,
    Face,
}

/// A flat face: a BREP face now, a flat region of a mesh in P4.
#[derive(Debug, Clone)]
pub struct FaceItem {
    /// The outer ring first, then the face's OWN inner loops (not
    /// containment).
    pub rings: Polygon2,
    /// The Z of the face plane, in the frame of the item's owner (the model
    /// frame in `LoadedModel`).
    pub plane_z: f64,
    /// The unit normal, SIGNED: an up face points to +Z, a down face to
    /// -Z. Same frame as `plane_z`.
    pub normal: V3,
    /// The BREP id, as a tie-break for the match only (DESIGN 9.3).
    pub brep: Option<FaceGroupId>,
}

/// The geometry of one item.
#[derive(Debug, Clone)]
pub enum Shape {
    /// A closed ring with no holes. Invariant: closed, 3 or more points.
    Loop(Polygon2),
    /// An open polyline. Invariant: not closed, 2 or more points, no holes.
    Path(Polygon2),
    /// A location: a DXF POINT.
    Point(P2),
    /// A circle: a DXF CIRCLE, a full-circle ARC, a circle-like SVG ring.
    /// Invariant: `ring` is closed with 3 or more points and no holes.
    Circle {
        centre: P2,
        diameter: f64,
        ring: Polygon2,
    },
    /// A flat face. Invariant: the outer ring is closed with 3 or more
    /// points, and the normal has unit length.
    Face(FaceItem),
}

impl Shape {
    /// A circle of `diameter` about `centre`, with the ring the DXF door
    /// makes for a CIRCLE at its arc step of 5 degrees (72 points, CCW).
    pub fn circle(centre: P2, diameter: f64) -> Shape {
        let radius = diameter / 2.0;
        let n = (std::f64::consts::TAU / CIRCLE_ARC_STEP_DEG.to_radians()).ceil() as usize;
        let n = n.max(8);
        let pts = (0..n)
            .map(|i| {
                let angle = std::f64::consts::TAU * i as f64 / n as f64;
                P2::new(
                    centre.x + radius * angle.cos(),
                    centre.y + radius * angle.sin(),
                )
            })
            .collect();
        let mut ring = Polygon2::new(pts);
        ring.ensure_winding();
        Shape::Circle {
            centre,
            diameter,
            ring,
        }
    }

    /// The kind of this shape.
    pub fn kind(&self) -> ItemKind {
        match self {
            Shape::Loop(_) => ItemKind::Loop,
            Shape::Path(_) => ItemKind::Path,
            Shape::Point(_) => ItemKind::Point,
            Shape::Circle { .. } => ItemKind::Circle,
            Shape::Face(_) => ItemKind::Face,
        }
    }

    /// The drawn ring of a drawing item: the loop, the path or the circle
    /// ring. A point and a face have none.
    pub fn drawing_ring(&self) -> Option<&Polygon2> {
        match self {
            Shape::Loop(p) | Shape::Path(p) => Some(p),
            Shape::Circle { ring, .. } => Some(ring),
            Shape::Point(_) | Shape::Face(_) => None,
        }
    }

    /// The same shape with every coordinate multiplied by `scale`. A face
    /// normal does not change.
    fn scaled(&self, scale: f64) -> Shape {
        match self {
            Shape::Loop(p) => Shape::Loop(scaled_polygon(p, scale)),
            Shape::Path(p) => Shape::Path(scaled_polygon(p, scale)),
            Shape::Point(p) => Shape::Point(P2::new(p.x * scale, p.y * scale)),
            Shape::Circle {
                centre,
                diameter,
                ring,
            } => Shape::Circle {
                centre: P2::new(centre.x * scale, centre.y * scale),
                diameter: diameter * scale,
                ring: scaled_polygon(ring, scale),
            },
            Shape::Face(face) => Shape::Face(FaceItem {
                rings: scaled_polygon(&face.rings, scale),
                plane_z: face.plane_z * scale,
                normal: face.normal,
                brep: face.brep,
            }),
        }
    }
}

/// A copy of `poly` with each point multiplied by `scale`, point by point.
/// The import doors scale with this same product, so a scaled item holds
/// the bytes the doors held before P2.1.
fn scaled_polygon(poly: &Polygon2, scale: f64) -> Polygon2 {
    let mut out = poly.clone();
    for pt in &mut out.exterior {
        pt.x *= scale;
        pt.y *= scale;
    }
    for hole in &mut out.holes {
        for pt in hole {
            pt.x *= scale;
            pt.y *= scale;
        }
    }
    // `exterior` moved — see the mutation contract on `Polygon2`.
    out.invalidate_bbox();
    out
}

/// A shape that breaks its invariant. A source checks its geometry before
/// it calls [`Item::new`], so this error names a defect in the source.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ItemError {
    #[error("a {kind:?} item needs a closed ring")]
    RingOpen { kind: ItemKind },
    #[error("a path item must be open")]
    PathClosed,
    #[error("a {kind:?} item needs {min} or more points, not {points}")]
    TooFewPoints {
        kind: ItemKind,
        min: usize,
        points: usize,
    },
    #[error("a {kind:?} item holds no holes (containment runs at resolution)")]
    HasHoles { kind: ItemKind },
    #[error("a face item needs a unit normal")]
    FaceNormal,
}

/// One pickable piece of 2D geometry.
///
/// The fields are private. [`Item::new`] is the only constructor.
#[derive(Debug, Clone)]
pub struct Item {
    source: SourceRef,
    layer: Option<Arc<str>>,
    key: MatchKey,
    shape: Shape,
}

/// A ring check shared by the closed shapes.
fn check_ring(kind: ItemKind, ring: &Polygon2, holes_allowed: bool) -> Result<(), ItemError> {
    if !ring.closed {
        return Err(ItemError::RingOpen { kind });
    }
    if ring.exterior.len() < 3 {
        return Err(ItemError::TooFewPoints {
            kind,
            min: 3,
            points: ring.exterior.len(),
        });
    }
    if !holes_allowed && !ring.holes.is_empty() {
        return Err(ItemError::HasHoles { kind });
    }
    Ok(())
}

impl Item {
    /// Build an item. The call checks the shape invariants (see [`Shape`])
    /// and computes the match key.
    pub fn new(
        source: SourceRef,
        layer: Option<Arc<str>>,
        shape: Shape,
    ) -> Result<Self, ItemError> {
        match &shape {
            Shape::Loop(p) => check_ring(ItemKind::Loop, p, false)?,
            Shape::Path(p) => {
                if p.closed {
                    return Err(ItemError::PathClosed);
                }
                if p.exterior.len() < 2 {
                    return Err(ItemError::TooFewPoints {
                        kind: ItemKind::Path,
                        min: 2,
                        points: p.exterior.len(),
                    });
                }
                if !p.holes.is_empty() {
                    return Err(ItemError::HasHoles {
                        kind: ItemKind::Path,
                    });
                }
            }
            Shape::Point(_) => {}
            Shape::Circle { ring, .. } => check_ring(ItemKind::Circle, ring, false)?,
            Shape::Face(face) => {
                check_ring(ItemKind::Face, &face.rings, true)?;
                if (face.normal.norm() - 1.0).abs() > 1e-6 {
                    return Err(ItemError::FaceNormal);
                }
            }
        }
        let key = MatchKey::of(source, layer.as_deref(), &shape);
        Ok(Self {
            source,
            layer,
            key,
            shape,
        })
    }

    pub fn source(&self) -> SourceRef {
        self.source
    }

    /// The DXF layer or the SVG group label. `None` for a STEP face, the
    /// stock, and an SVG shape outside a labelled group.
    pub fn layer(&self) -> Option<&str> {
        self.layer.as_deref()
    }

    pub fn key(&self) -> &MatchKey {
        &self.key
    }

    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    pub fn kind(&self) -> ItemKind {
        self.shape.kind()
    }

    /// The same item (source and layer) with `shape` in place of its shape.
    /// The key is computed again, so it matches the new shape. The frame
    /// door (`ResolvedSelection::to_setup_frame`) calls this.
    pub(crate) fn with_shape(&self, shape: Shape) -> Result<Self, ItemError> {
        Self::new(self.source, self.layer.clone(), shape)
    }

    /// The same item with a different source. The key is computed again.
    fn rebuilt(&self, source: SourceRef, shape: Shape) -> Result<Self, ItemError> {
        Self::new(source, self.layer.clone(), shape)
    }
}

/// The items of one source, in import order. The order matters: the
/// nesting of the default selection depends on it (DESIGN 4.4).
#[derive(Debug, Clone, Default)]
pub struct ItemSet {
    items: Vec<Item>,
    /// The memo of [`ItemSet::outlines_pre_selection`]. The items cannot
    /// change after construction, so the memo cannot go stale. A scaled or
    /// rebound set carries the memo of its source set (see
    /// [`ItemSet::scaled`]).
    outlines: OnceLock<Arc<Vec<Polygon2>>>,
}

impl ItemSet {
    /// An item set over `items`, in this order.
    pub fn new(items: Vec<Item>) -> Self {
        Self {
            items,
            outlines: OnceLock::new(),
        }
    }

    /// The item set of a drawing that a test or a tool builds from
    /// polygons, bound to [`SourceRef::ToolpathModel`]. Each polygon gives
    /// items in this order:
    ///
    /// - a closed polygon of 3 or more points: a `Loop` of its exterior,
    ///   then one `Loop` for each hole ring of 3 or more points;
    /// - a closed polygon of 2 points or an open polygon of 2 or more
    ///   points: a `Path` (DESIGN 2.2: a closed ring of 2 points is a path);
    /// - a polygon of 1 point: a `Point`.
    ///
    /// A polygon with no point gives no item: it holds nothing to keep.
    ///
    /// The caller states the nested list: until P2.3 deletes the bridge,
    /// `outlines_pre_selection` gives `polygons` unchanged, in this order.
    /// A hand-built list need not be in the form `detect_containment`
    /// gives (an open path wound clockwise, two loops not sorted by area),
    /// and the model before P2.1 held it as given.
    pub fn from_polygons(polygons: Vec<Polygon2>) -> Self {
        let mut items = Vec::new();
        let source = SourceRef::ToolpathModel;
        let outlines = Arc::new(polygons.clone());
        for poly in polygons {
            let shapes: Vec<Shape> = match poly.exterior.len() {
                0 => Vec::new(),
                1 => poly
                    .exterior
                    .first()
                    .map(|p| Shape::Point(*p))
                    .into_iter()
                    .collect(),
                2 => vec![Shape::Path(Polygon2::open_path(poly.exterior))],
                _ if !poly.closed => vec![Shape::Path(Polygon2::open_path(poly.exterior))],
                _ => {
                    let mut shapes = vec![Shape::Loop(Polygon2::new(poly.exterior))];
                    shapes.extend(
                        poly.holes
                            .into_iter()
                            .filter(|h| h.len() >= 3)
                            .map(|h| Shape::Loop(Polygon2::new(h))),
                    );
                    shapes
                }
            };
            // Each shape above meets its invariant, so `Item::new` accepts it.
            items.extend(
                shapes
                    .into_iter()
                    .filter_map(|shape| Item::new(source, None, shape).ok()),
            );
        }
        Self::with_outlines(items, outlines)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Item> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// True when the set holds a drawing item (loop, path, point or
    /// circle). A STEP model holds faces only, and a mesh holds no item.
    /// This is the "is a 2D drawing" test of every reader.
    pub fn has_drawing(&self) -> bool {
        self.items
            .iter()
            .any(|i| !matches!(i.shape, Shape::Face(_)))
    }

    /// The distinct layers of every item, sorted.
    pub fn layers(&self) -> Vec<String> {
        let mut layers: Vec<String> = self
            .items
            .iter()
            .filter_map(|i| i.layer().map(str::to_owned))
            .collect();
        layers.sort();
        layers.dedup();
        layers
    }

    /// `[x_min, y_min, x_max, y_max]` over the drawn rings (loops, paths and
    /// circle rings). Points and faces do not count. `None` when no ring
    /// exists.
    pub fn bbox(&self) -> Option<[f64; 4]> {
        let mut b = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for ring in self.items.iter().filter_map(|i| i.shape.drawing_ring()) {
            for p in &ring.exterior {
                b[0] = b[0].min(p.x);
                b[1] = b[1].min(p.y);
                b[2] = b[2].max(p.x);
                b[3] = b[3].max(p.y);
            }
        }
        b[0].is_finite().then_some(b)
    }

    /// The same items with each coordinate multiplied by `scale`. A scale
    /// within 1e-9 of 1 returns the set unchanged, as the import doors did.
    ///
    /// The nested outline list of the new set is the list of this set with
    /// each point scaled. Before P2.1 the import nested the rings first and
    /// scaled them after; a nesting after the scale can order two rings of
    /// equal area the other way (the areas differ in the last bit).
    pub fn scaled(&self, scale: f64) -> Result<Self, ItemError> {
        if (scale - 1.0).abs() < 1e-9 {
            return Ok(self.clone());
        }
        let outlines: Vec<Polygon2> = self
            .outlines_pre_selection()
            .iter()
            .map(|p| scaled_polygon(p, scale))
            .collect();
        let items = self
            .items
            .iter()
            .map(|i| i.rebuilt(i.source, i.shape.scaled(scale)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::with_outlines(items, Arc::new(outlines)))
    }

    /// The same items with `source` as their source.
    pub fn bound_to(&self, source: SourceRef) -> Result<Self, ItemError> {
        if self.items.iter().all(|i| i.source == source) {
            return Ok(self.clone());
        }
        let items = self
            .items
            .iter()
            .map(|i| i.rebuilt(source, i.shape.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        // The geometry does not change, so the memo stays true.
        Ok(Self {
            items,
            outlines: self.outlines.clone(),
        })
    }

    /// A loop of this set that is circle-like becomes a `Circle` (the SVG
    /// door: usvg has flattened every `<circle>`). `circle_of` gives the
    /// centre and diameter of a circle-like ring. The rings do not change,
    /// so the nested outline list stays the list of this set.
    pub(crate) fn with_circles(
        &self,
        circle_of: impl Fn(&Polygon2) -> Option<(P2, f64)>,
    ) -> Result<Self, ItemError> {
        let outlines = self.outlines_pre_selection();
        let items = self
            .items
            .iter()
            .map(|i| match &i.shape {
                Shape::Loop(ring) => match circle_of(ring) {
                    Some((centre, diameter)) => i.rebuilt(
                        i.source,
                        Shape::Circle {
                            centre,
                            diameter,
                            ring: ring.clone(),
                        },
                    ),
                    None => Ok(i.clone()),
                },
                _ => Ok(i.clone()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::with_outlines(items, outlines))
    }

    /// A set whose nested outline list is `outlines` (already computed).
    fn with_outlines(items: Vec<Item>, outlines: Arc<Vec<Polygon2>>) -> Self {
        let memo = OnceLock::new();
        let _ = memo.set(outlines);
        Self {
            items,
            outlines: memo,
        }
    }

    /// The polygon list of the import before P2.1: every drawn ring in
    /// import order, nested by `detect_containment` as the import did.
    ///
    /// P2.3 deletes this. Read it through
    /// `LoadedModel::outlines_pre_selection`.
    pub(crate) fn outlines_pre_selection(&self) -> Arc<Vec<Polygon2>> {
        Arc::clone(self.outlines.get_or_init(|| {
            let rings: Vec<Polygon2> = self
                .items
                .iter()
                .filter_map(|i| i.shape.drawing_ring().cloned())
                .collect();
            Arc::new(crate::polygon::detect_containment(rings))
        }))
    }
}

impl<'a> IntoIterator for &'a ItemSet {
    type Item = &'a Item;
    type IntoIter = std::slice::Iter<'a, Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, s: f64) -> Polygon2 {
        Polygon2::new(vec![
            P2::new(x0, y0),
            P2::new(x0 + s, y0),
            P2::new(x0 + s, y0 + s),
            P2::new(x0, y0 + s),
        ])
    }

    #[test]
    fn a_loop_must_be_closed_and_a_path_open() {
        let open = Polygon2::open_path(vec![P2::new(0.0, 0.0), P2::new(1.0, 0.0)]);
        assert_eq!(
            Item::new(SourceRef::ToolpathModel, None, Shape::Loop(open.clone())).unwrap_err(),
            ItemError::RingOpen {
                kind: ItemKind::Loop
            }
        );
        assert_eq!(
            Item::new(
                SourceRef::ToolpathModel,
                None,
                Shape::Path(square(0.0, 0.0, 1.0))
            )
            .unwrap_err(),
            ItemError::PathClosed
        );
        let item = Item::new(
            SourceRef::ToolpathModel,
            Some("A".into()),
            Shape::Path(open),
        )
        .expect("an open path of 2 points is a path");
        assert_eq!(item.kind(), ItemKind::Path);
        assert_eq!(item.layer(), Some("A"));
    }

    #[test]
    fn a_loop_holds_no_hole() {
        let mut holed = square(0.0, 0.0, 10.0);
        holed.holes.push(square(2.0, 2.0, 1.0).exterior);
        assert_eq!(
            Item::new(SourceRef::ToolpathModel, None, Shape::Loop(holed)).unwrap_err(),
            ItemError::HasHoles {
                kind: ItemKind::Loop
            }
        );
    }

    #[test]
    fn from_polygons_splits_holes_and_keeps_the_given_list() {
        let mut holed = square(0.0, 0.0, 10.0);
        let mut hole = square(2.0, 2.0, 1.0).exterior;
        hole.reverse();
        holed.holes.push(hole.clone());
        let set = ItemSet::from_polygons(vec![holed]);
        assert_eq!(set.len(), 2, "the exterior and the hole are two loops");
        let outlines = set.outlines_pre_selection();
        assert_eq!(outlines.len(), 1);
        assert_eq!(outlines[0].holes, vec![hole]);
        // A scaled set nests first and scales after, as the import did.
        let scaled = set.scaled(2.0).unwrap();
        assert_eq!(scaled.outlines_pre_selection()[0].holes.len(), 1);
    }

    #[test]
    fn scaling_and_binding_compute_the_key_again() {
        let set = ItemSet::from_polygons(vec![square(1.0, 1.0, 2.0)]);
        let scaled = set.scaled(10.0).unwrap();
        let key = scaled.iter().next().unwrap().key();
        assert!((key.anchor[0] - 20.0).abs() < 1e-9);
        assert!((key.measure - 20.0).abs() < 1e-9);
        let bound = scaled.bound_to(SourceRef::Model(ModelId(7))).unwrap();
        assert_eq!(
            bound.iter().next().unwrap().key().source,
            SourceRef::Model(ModelId(7))
        );
    }
}
