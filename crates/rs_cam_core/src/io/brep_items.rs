//! The model edge items of a STEP model (geometry selection UX-7, operator
//! ruling Q3, GT-Q9): face rings, edge chains, circular edges and vertices,
//! each at its own Z.
//!
//! The source is the 3D boundary loops of the BREP faces
//! (`FaceGroup::boundary_loops`, ordered by the boundary walk). No edge
//! curve type is read: the tessellation gives each edge as a polyline, and
//! two faces that share an edge share its vertices.
//!
//! - **Rings.** Each outer and inner ring of an UP face item is a `Loop` at
//!   the face Z. A down face gives no ring: a 2.5D op reaches it from above
//!   only through the material. A round ring stays a `Loop` (so the hole
//!   rim is an island of a Pocket); its circular edge is a `Circle` too.
//! - **Edge chains.** Each face loop of a face that is not horizontal is cut
//!   into runs: the maximal runs of segments at one Z
//!   ([`HORIZONTAL_TOL_MM`]) that border one other face. An open run is a
//!   `Path`; a closed run (a rim that the whole loop of a curved face holds)
//!   is a `Circle` when its points lie on one circle, else a `Loop`. A
//!   horizontal face gives no chain: each of its edges borders a wall, and
//!   the wall loop gives it. A run that two faces both give is one item.
//! - **Vertices.** The two ends of each open run are `Point` items: the
//!   corners where a horizontal edge meets another edge. A closed rim has no
//!   vertex item.
//! - **Skipped.** A run that is not at one Z (a vertical or a sloped edge)
//!   gives no item; [`BrepEdgeItems::skipped`] counts the edges.
//! - **Edge form (R-C).** Each ring, chain and circle, and each up face,
//!   holds a [`ModelEdge`]: whether each segment is a convex top edge (an
//!   up face at the item Z on one side, a face at or below that Z on the
//!   other: the material is below the edge and on one side only), a
//!   concave edge (the face beside the up face rises over the edge), or
//!   neither. The side of the up face is the material side. A segment
//!   whose faces the door cannot read is neither ([`EdgeForm::Other`]), so
//!   a Chamfer never cuts it.
//!
//! The items are in a fixed order (rings in face order, then circles,
//! paths, loops and points, each sorted by Z from the top, then X, then Y),
//! so a re-import of the same file gives the same list.

use std::collections::{BTreeMap, BTreeSet};

use crate::geo::{P2, P3};
use crate::geometry::enriched_mesh::EnrichedMesh;
use crate::polygon::Polygon2;
use crate::selection::{
    CAD_ROUNDING_MM, EdgeForm, EdgeMaterial, Item, ItemKind, ItemSet, ModelEdge, Shape, SourceRef,
};

/// The Z tolerance (mm) of a horizontal edge: two points of one run differ
/// in Z by no more than this. It is the vertex grid of the STEP door
/// (`step_input::quantize_point`, 1e-3 mm), so a run at one BREP Z is never
/// cut by the round-off of the tessellation.
pub const HORIZONTAL_TOL_MM: f64 = 1e-3;

/// The fewest points of a closed run that reads as a circle. A tessellated
/// circle at the door tolerance has more; a square rim has 4.
const CIRCLE_MIN_POINTS: usize = 8;

/// The Z tolerance (mm) of the edge form (R-C): the up face lies at the
/// edge Z within this distance. It is the CAD rounding of a drawn length
/// (`CAD_ROUNDING_MM`, L-13).
const EDGE_FORM_Z_TOL_MM: f64 = CAD_ROUNDING_MM;

/// The distance (mm) from a segment midpoint to the probe point that finds
/// the side of the up face (R-C). Ten vertex grid cells: far from the
/// round-off of the segment, and far inside any face a 2.5D part holds.
const SIDE_PROBE_MM: f64 = 10.0 * HORIZONTAL_TOL_MM;

/// The model edge items of one STEP model, and the edges that gave none.
#[derive(Debug, Clone, Default)]
pub struct BrepEdgeItems {
    /// The rings, chains, circles and points, bound to
    /// `SourceRef::ToolpathModel` (the session binds them).
    pub items: Vec<Item>,
    /// The count of edges that are not at one Z (vertical or sloped): no
    /// item stands for them.
    pub skipped: usize,
    /// The model edge data of each face item of the `faces` input, in its
    /// order (R-C): `None` for a face that points down. An up face holds
    /// the form of all its rings together.
    pub face_edges: Vec<Option<ModelEdge>>,
}

/// What the door knows of each face group for the edge form (R-C).
struct FaceFacts {
    /// For each (face group, boundary segment on the vertex grid): the Z
    /// of the third corner of the triangle of that group on that segment.
    /// It says whether the face goes down or up from the segment.
    third_z: BTreeMap<(usize, SegmentKey), f64>,
    /// For each face group that is an up face item: its plane Z and its
    /// rings (outer first, then its own holes), by group index.
    up: BTreeMap<usize, (f64, Polygon2)>,
}

impl FaceFacts {
    fn of(enriched: &EnrichedMesh, faces: &ItemSet) -> Self {
        let index_of: BTreeMap<u16, usize> = enriched
            .face_groups
            .iter()
            .enumerate()
            .map(|(i, g)| (g.id.0, i))
            .collect();
        let mut up = BTreeMap::new();
        for item in faces {
            let Shape::Face(face) = item.shape() else {
                continue;
            };
            let Some(group) = face.brep.and_then(|id| index_of.get(&id.0)) else {
                continue;
            };
            if face.normal.z > 0.0 {
                up.insert(*group, (face.plane_z, face.rings.clone()));
            }
        }
        let mesh = &enriched.mesh;
        let mut third_z = BTreeMap::new();
        for (group, g) in enriched.face_groups.iter().enumerate() {
            for tri in mesh.triangles.get(g.triangle_range.clone()).unwrap_or(&[]) {
                let corner = |k: usize| {
                    tri.get(k)
                        .and_then(|&v| mesh.vertices.get(v as usize))
                        .copied()
                };
                let (Some(a), Some(b), Some(c)) = (corner(0), corner(1), corner(2)) else {
                    continue;
                };
                for (p, q, r) in [(a, b, c), (b, c, a), (c, a, b)] {
                    third_z.insert((group, segment_key(&p, &q)), r.z);
                }
            }
        }
        Self { third_z, up }
    }

    /// The up face at `z` of group `group`, or `None`.
    fn up_at(&self, group: usize, z: f64) -> Option<&Polygon2> {
        self.up
            .get(&group)
            .filter(|(plane_z, _)| (plane_z - z).abs() <= EDGE_FORM_Z_TOL_MM)
            .map(|(_, rings)| rings)
    }

    /// How the face `group` leaves the horizontal segment `a`-`b` at `z`,
    /// beside an up face: down (a convex top edge), up (a concave edge),
    /// or unknown. The triangle of the face on the segment says it: its
    /// third corner lies below or above the segment.
    fn side_of(&self, group: usize, a: P3, b: P3, z: f64) -> EdgeForm {
        let Some(&c) = self.third_z.get(&(group, segment_key(&a, &b))) else {
            return EdgeForm::Other;
        };
        if c < z - HORIZONTAL_TOL_MM {
            // The material side is set by the caller.
            EdgeForm::ConvexTop {
                material: EdgeMaterial::Left,
            }
        } else if c > z + HORIZONTAL_TOL_MM {
            EdgeForm::Concave
        } else {
            EdgeForm::Other
        }
    }

    /// The form of the segments of `points` (each pair of neighbours, and
    /// the closing pair when `closed`) on the face `group` at `z`: concave
    /// when one segment is, a convex top edge when every segment is one.
    fn segments_form(&self, group: usize, points: &[P3], closed: bool, z: f64) -> EdgeForm {
        let n = points.len();
        let count = if closed { n } else { n.saturating_sub(1) };
        if count == 0 {
            return EdgeForm::Other;
        }
        let mut form = EdgeForm::ConvexTop {
            material: EdgeMaterial::Left,
        };
        for i in 0..count {
            let (Some(&a), Some(&b)) = (points.get(i), points.get((i + 1) % n)) else {
                return EdgeForm::Other;
            };
            match self.side_of(group, a, b, z) {
                EdgeForm::Concave => return EdgeForm::Concave,
                EdgeForm::Other => form = EdgeForm::Other,
                EdgeForm::ConvexTop { .. } => {}
            }
        }
        form
    }
}

/// True when the point `SIDE_PROBE_MM` to the left of the segment `a`-`b`
/// lies on `face`.
fn face_on_left(face: &Polygon2, a: P2, b: P2) -> bool {
    let d = b - a;
    let len = d.norm();
    if len <= 0.0 {
        return false;
    }
    let mid = P2::new(0.5 * (a.x + b.x), 0.5 * (a.y + b.y));
    let probe = P2::new(
        mid.x - d.y / len * SIDE_PROBE_MM,
        mid.y + d.x / len * SIDE_PROBE_MM,
    );
    face.contains_point(&probe)
}

/// The form of a chain at `z` that the wall `wall` gives and that borders
/// the face `other` (R-C). `points` are the run points (the closing point
/// last when it closes). `xy` is the chain in the item point order;
/// `closed` says whether it closes (its closing point is not in `xy`).
fn chain_form(
    facts: &FaceFacts,
    wall: usize,
    other: Option<usize>,
    z: f64,
    points: &[P3],
    xy: &[P2],
    closed: bool,
) -> EdgeForm {
    let Some(up) = other.and_then(|o| facts.up_at(o, z)) else {
        return EdgeForm::Other;
    };
    match facts.segments_form(wall, points, false, z) {
        EdgeForm::ConvexTop { .. } => {}
        form @ (EdgeForm::Concave | EdgeForm::Other) => return form,
    }
    let (Some(&a), Some(&b)) = (xy.first(), xy.get(1)) else {
        return EdgeForm::Other;
    };
    let left = face_on_left(up, a, b);
    let material = if closed {
        let ccw = Polygon2::new(xy.to_vec()).signed_area() > 0.0;
        if left == ccw {
            EdgeMaterial::Inside
        } else {
            EdgeMaterial::Outside
        }
    } else if left {
        EdgeMaterial::Left
    } else {
        EdgeMaterial::Right
    };
    EdgeForm::ConvexTop { material }
}

/// The form of one ring of the up face `group` at `z` (R-C): each segment
/// reads the face on its other side ([`FaceFacts::side_of`]). Any concave
/// segment makes the ring concave; a ring is a convex top edge only when
/// every segment is one. `material` is the side of the face (inside its
/// outer ring, outside a hole ring).
fn ring_form(
    facts: &FaceFacts,
    neighbours: &BTreeMap<([i64; 3], [i64; 3]), BTreeSet<usize>>,
    group: usize,
    z: f64,
    ring: &[P2],
    material: EdgeMaterial,
) -> EdgeForm {
    let n = ring.len();
    let mut form = EdgeForm::ConvexTop { material };
    for i in 0..n {
        let (Some(a), Some(b)) = (ring.get(i), ring.get((i + 1) % n)) else {
            return EdgeForm::Other;
        };
        let (a, b) = (P3::new(a.x, a.y, z), P3::new(b.x, b.y, z));
        let other = neighbours
            .get(&segment_key(&a, &b))
            .and_then(|faces| faces.iter().copied().find(|f| *f != group));
        let segment = match other {
            Some(o) => facts.side_of(o, a, b, z),
            None => EdgeForm::Other,
        };
        match segment {
            EdgeForm::Concave => return EdgeForm::Concave,
            EdgeForm::Other => form = EdgeForm::Other,
            EdgeForm::ConvexTop { .. } => {}
        }
    }
    form
}

/// The form of an up face from the forms of its rings (R-C): concave when
/// one ring is, a convex top edge when every ring is one.
fn face_form(rings: &[EdgeForm]) -> EdgeForm {
    if rings.contains(&EdgeForm::Concave) {
        return EdgeForm::Concave;
    }
    if rings
        .iter()
        .all(|f| matches!(f, EdgeForm::ConvexTop { .. }))
    {
        return EdgeForm::ConvexTop {
            material: EdgeMaterial::Inside,
        };
    }
    EdgeForm::Other
}

/// A point on the vertex grid of the STEP door, for the joins of this
/// module (two faces give one shared vertex at one grid cell).
fn grid(p: &P3) -> [i64; 3] {
    [
        (p.x * 1000.0).round() as i64,
        (p.y * 1000.0).round() as i64,
        (p.z * 1000.0).round() as i64,
    ]
}

/// One loop with its closing point (a copy of the first) removed.
fn open_loop(pts: &[P3]) -> &[P3] {
    match (pts.first(), pts.last()) {
        (Some(a), Some(b)) if pts.len() > 1 && grid(a) == grid(b) => {
            pts.get(..pts.len() - 1).unwrap_or(pts)
        }
        _ => pts,
    }
}

/// An unordered segment on the vertex grid: its two grid points, the
/// lower first.
type SegmentKey = ([i64; 3], [i64; 3]);

/// An unordered segment on the vertex grid.
fn segment_key(a: &P3, b: &P3) -> ([i64; 3], [i64; 3]) {
    let (ga, gb) = (grid(a), grid(b));
    if ga <= gb { (ga, gb) } else { (gb, ga) }
}

/// One run of a face loop: its points and whether it closes on itself.
struct Run {
    points: Vec<P3>,
    closed: bool,
    horizontal: bool,
    /// The face on the other side of the run (one face for the whole run).
    other: Option<usize>,
}

/// The runs of one loop (closing point removed) of face `face`. A run
/// ends where the segment changes from horizontal to not, where the Z
/// level changes, or where the face on the other side changes.
fn runs_of(
    face: usize,
    pts: &[P3],
    neighbours: &BTreeMap<([i64; 3], [i64; 3]), BTreeSet<usize>>,
) -> Vec<Run> {
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    let at = |i: usize| pts.get(i % n).copied().unwrap_or(P3::origin());
    // The segment i runs from point i to point i + 1.
    let other = |i: usize| -> Option<usize> {
        neighbours
            .get(&segment_key(&at(i), &at(i + 1)))
            .and_then(|faces| faces.iter().copied().find(|f| *f != face))
    };
    let flat = |i: usize| (at(i).z - at(i + 1).z).abs() <= HORIZONTAL_TOL_MM;
    // Two segments belong to one run when both are flat at one Z with one
    // neighbour, or both are not flat with one neighbour.
    let joins = |i: usize, j: usize| {
        flat(i) == flat(j)
            && other(i) == other(j)
            && (!flat(i) || (at(i).z - at(j).z).abs() <= HORIZONTAL_TOL_MM)
    };
    // The first segment that starts a run; with none, the loop is one run.
    let Some(start) = (0..n).find(|&i| !joins((i + n - 1) % n, i)) else {
        return vec![Run {
            points: (0..n).map(at).collect(),
            closed: true,
            horizontal: flat(0),
            other: other(0),
        }];
    };
    let mut runs = Vec::new();
    let mut k = 0;
    while k < n {
        let first = start + k;
        let mut last = first;
        // A flat run holds one Z over its whole extent, not only from
        // segment to segment: a gentle slope cut fine is not flat.
        let level = at(first).z;
        while last + 1 < start + n
            && joins(last, last + 1)
            && (!flat(first) || (at(last + 2).z - level).abs() <= HORIZONTAL_TOL_MM)
        {
            last += 1;
        }
        let points: Vec<P3> = (first..=last + 1).map(at).collect();
        let closed = points.len() > 2
            && points
                .first()
                .zip(points.last())
                .is_some_and(|(a, b)| grid(a) == grid(b));
        runs.push(Run {
            points,
            closed,
            horizontal: flat(first),
            other: other(first),
        });
        k = last + 1 - start;
    }
    runs
}

/// The centre and diameter of a closed run whose points lie on one circle,
/// else `None`.
fn circle_of(points: &[P2]) -> Option<(P2, f64)> {
    if points.len() < CIRCLE_MIN_POINTS {
        return None;
    }
    let k = points.len() as f64;
    let (sx, sy) = points
        .iter()
        .fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
    let centre = P2::new(sx / k, sy / k);
    let radii: Vec<f64> = points.iter().map(|p| (p - centre).norm()).collect();
    let mean = radii.iter().sum::<f64>() / k;
    let spread = radii
        .iter()
        .map(|r| (r - mean).abs())
        .fold(0.0_f64, f64::max);
    (mean > 0.0 && spread <= HORIZONTAL_TOL_MM.max(1e-4 * mean)).then_some((centre, 2.0 * mean))
}

/// True when two items of one kind name one edge: equal kind, and anchor,
/// measure and Z within [`CAD_ROUNDING_MM`]. Two faces give the same edge
/// with tessellation round-off, so the item key test (1e-6) is too tight.
fn same_edge(a: &Item, b: &Item) -> bool {
    let (ka, kb) = (a.key(), b.key());
    let near = |x: f64, y: f64| (x - y).abs() <= CAD_ROUNDING_MM;
    ka.kind == kb.kind
        && near(ka.anchor[0], kb.anchor[0])
        && near(ka.anchor[1], kb.anchor[1])
        && near(ka.measure, kb.measure)
        && match (ka.z, kb.z) {
            (Some(x), Some(y)) => near(x, y),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        }
}

/// The mean Z of a run.
fn mean_z(points: &[P3]) -> f64 {
    points.iter().map(|p| p.z).sum::<f64>() / points.len().max(1) as f64
}

/// The order of the non-ring items: Z from the top, then X, then Y, then
/// the measure.
fn order(a: &Item, b: &Item) -> std::cmp::Ordering {
    let (ka, kb) = (a.key(), b.key());
    kb.z.unwrap_or(0.0)
        .total_cmp(&ka.z.unwrap_or(0.0))
        .then(ka.anchor[0].total_cmp(&kb.anchor[0]))
        .then(ka.anchor[1].total_cmp(&kb.anchor[1]))
        .then(ka.measure.total_cmp(&kb.measure))
}

/// The model edge items of `enriched` (module doc). `faces` are its face
/// items ([`super::brep_face_items`]): the rings come from them.
pub fn brep_edge_items(enriched: &EnrichedMesh, faces: &ItemSet) -> BrepEdgeItems {
    let source = SourceRef::ToolpathModel;
    let mut out = BrepEdgeItems::default();
    let facts = FaceFacts::of(enriched, faces);
    let index_of: BTreeMap<u16, usize> = enriched
        .face_groups
        .iter()
        .enumerate()
        .map(|(i, g)| (g.id.0, i))
        .collect();

    // The faces on each side of every loop segment.
    let mut neighbours: BTreeMap<([i64; 3], [i64; 3]), BTreeSet<usize>> = BTreeMap::new();
    for (face, group) in enriched.face_groups.iter().enumerate() {
        for lp in &group.boundary_loops {
            let pts = open_loop(lp);
            let n = pts.len();
            for i in 0..n {
                if let (Some(a), Some(b)) = (pts.get(i), pts.get((i + 1) % n)) {
                    neighbours
                        .entry(segment_key(a, b))
                        .or_default()
                        .insert(face);
                }
            }
        }
    }

    // Rings: each ring of each UP face, at the face Z, with its form.
    let mut rings: Vec<Item> = Vec::new();
    for item in faces {
        let Shape::Face(face) = item.shape() else {
            out.face_edges.push(None);
            continue;
        };
        if face.normal.z <= 0.0 {
            out.face_edges.push(None);
            continue;
        }
        let group = face.brep.and_then(|id| index_of.get(&id.0)).copied();
        let all = std::iter::once((&face.rings.exterior, EdgeMaterial::Inside))
            .chain(face.rings.holes.iter().map(|h| (h, EdgeMaterial::Outside)));
        let mut forms = Vec::new();
        for (ring, material) in all {
            let mut pts = ring.clone();
            if pts.len() > 1
                && pts
                    .first()
                    .zip(pts.last())
                    .is_some_and(|(a, b)| (a - b).norm() <= 1e-9)
            {
                pts.pop();
            }
            if pts.len() < 3 {
                continue;
            }
            let form = group.map_or(EdgeForm::Other, |g| {
                ring_form(&facts, &neighbours, g, face.plane_z, &pts, material)
            });
            forms.push(form);
            if let Ok(ring_item) =
                Item::at_z(source, None, Shape::Loop(Polygon2::new(pts)), face.plane_z)
            {
                rings.push(ring_item.with_model_edge(ModelEdge { form, ring: true }));
            }
        }
        out.face_edges.push(Some(ModelEdge {
            form: face_form(&forms),
            ring: true,
        }));
    }

    let mut chains: Vec<Item> = Vec::new();
    let mut corners: BTreeMap<[i64; 3], P3> = BTreeMap::new();
    let mut skipped: BTreeSet<Vec<[i64; 3]>> = BTreeSet::new();
    for (face, group) in enriched.face_groups.iter().enumerate() {
        // A horizontal face: its loop is one run at one Z, a ring (above).
        if group.boundary_loops_2d.is_some() {
            continue;
        }
        for lp in &group.boundary_loops {
            for run in runs_of(face, open_loop(lp), &neighbours) {
                if !run.horizontal {
                    let mut ends: Vec<[i64; 3]> = run.points.iter().map(grid).collect();
                    ends.sort_unstable();
                    skipped.insert(ends);
                    continue;
                }
                let z = mean_z(&run.points);
                let xy: Vec<P2> = run.points.iter().map(|p| P2::new(p.x, p.y)).collect();
                let item = if run.closed {
                    let ring: Vec<P2> = xy.get(..xy.len() - 1).unwrap_or(&xy).to_vec();
                    match circle_of(&ring) {
                        Some((centre, diameter)) => {
                            let mut ring = Polygon2::new(ring);
                            ring.ensure_winding();
                            let form = chain_form(
                                &facts,
                                face,
                                run.other,
                                z,
                                &run.points,
                                &ring.exterior,
                                true,
                            );
                            Item::at_z(
                                source,
                                None,
                                Shape::Circle {
                                    centre,
                                    diameter,
                                    ring,
                                },
                                z,
                            )
                            .map(|i| i.with_model_edge(ModelEdge { form, ring: false }))
                        }
                        None => {
                            let form =
                                chain_form(&facts, face, run.other, z, &run.points, &ring, true);
                            Item::at_z(source, None, Shape::Loop(Polygon2::new(ring)), z)
                                .map(|i| i.with_model_edge(ModelEdge { form, ring: false }))
                        }
                    }
                } else {
                    // One direction for the two faces that give the edge:
                    // from the lower end (X, then Y).
                    let mut xy = xy;
                    if let (Some(a), Some(b)) = (xy.first(), xy.last())
                        && (b.x, b.y) < (a.x, a.y)
                    {
                        xy.reverse();
                    }
                    // A run of no length (the pole of a sphere) is no edge.
                    let length: f64 = xy
                        .windows(2)
                        .map(|w| match w {
                            [a, b] => (b - a).norm(),
                            _ => 0.0,
                        })
                        .sum();
                    if length <= HORIZONTAL_TOL_MM {
                        continue;
                    }
                    for end in [run.points.first(), run.points.last()]
                        .into_iter()
                        .flatten()
                    {
                        corners.entry(grid(end)).or_insert(*end);
                    }
                    let form = chain_form(&facts, face, run.other, z, &run.points, &xy, false);
                    Item::at_z(source, None, Shape::Path(Polygon2::open_path(xy)), z)
                        .map(|i| i.with_model_edge(ModelEdge { form, ring: false }))
                };
                let Ok(item) = item else {
                    continue;
                };
                // A closed chain that is a face ring is that ring.
                let is_ring =
                    item.kind() == ItemKind::Loop && rings.iter().any(|r| same_edge(r, &item));
                if !is_ring && !chains.iter().any(|c| same_edge(c, &item)) {
                    chains.push(item);
                }
            }
        }
    }
    out.skipped = skipped.len();

    let mut points: Vec<Item> = corners
        .values()
        .filter_map(|p| Item::at_z(source, None, Shape::Point(P2::new(p.x, p.y)), p.z).ok())
        .collect();
    let by_kind = |kind: ItemKind| {
        let mut v: Vec<Item> = chains
            .iter()
            .filter(|i| i.kind() == kind)
            .cloned()
            .collect();
        v.sort_by(order);
        v
    };
    points.sort_by(order);
    out.items = rings;
    out.items.extend(by_kind(ItemKind::Circle));
    out.items.extend(by_kind(ItemKind::Path));
    out.items.extend(by_kind(ItemKind::Loop));
    out.items.extend(points);
    out
}
