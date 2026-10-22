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
//!
//! The items are in a fixed order (rings in face order, then circles,
//! paths, loops and points, each sorted by Z from the top, then X, then Y),
//! so a re-import of the same file gives the same list.

use std::collections::{BTreeMap, BTreeSet};

use crate::geo::{P2, P3};
use crate::geometry::enriched_mesh::EnrichedMesh;
use crate::polygon::Polygon2;
use crate::selection::{CAD_ROUNDING_MM, Item, ItemKind, ItemSet, Shape, SourceRef};

/// The Z tolerance (mm) of a horizontal edge: two points of one run differ
/// in Z by no more than this. It is the vertex grid of the STEP door
/// (`step_input::quantize_point`, 1e-3 mm), so a run at one BREP Z is never
/// cut by the round-off of the tessellation.
pub const HORIZONTAL_TOL_MM: f64 = 1e-3;

/// The fewest points of a closed run that reads as a circle. A tessellated
/// circle at the door tolerance has more; a square rim has 4.
const CIRCLE_MIN_POINTS: usize = 8;

/// The model edge items of one STEP model, and the edges that gave none.
#[derive(Debug, Clone, Default)]
pub struct BrepEdgeItems {
    /// The rings, chains, circles and points, bound to
    /// `SourceRef::ToolpathModel` (the session binds them).
    pub items: Vec<Item>,
    /// The count of edges that are not at one Z (vertical or sloped): no
    /// item stands for them.
    pub skipped: usize,
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

    // Rings: each ring of each UP face, at the face Z.
    let mut rings: Vec<Item> = Vec::new();
    for item in faces {
        let Shape::Face(face) = item.shape() else {
            continue;
        };
        if face.normal.z <= 0.0 {
            continue;
        }
        let all = std::iter::once(&face.rings.exterior).chain(face.rings.holes.iter());
        for ring in all {
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
            if let Ok(ring_item) =
                Item::at_z(source, None, Shape::Loop(Polygon2::new(pts)), face.plane_z)
            {
                rings.push(ring_item);
            }
        }
    }

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
                        }
                        None => Item::at_z(source, None, Shape::Loop(Polygon2::new(ring)), z),
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
                    Item::at_z(source, None, Shape::Path(Polygon2::open_path(xy)), z)
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
