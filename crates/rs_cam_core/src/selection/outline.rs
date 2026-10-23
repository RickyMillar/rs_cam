//! R-A (operator ruling 2026-10-23): the part outline of a mesh model.
//!
//! A STEP or an STL model gives ONE `Loop` item at the model bottom Z: the
//! outer ring of the shadow of the model on the XY plane. The default
//! Profile on a mesh model cuts it, Outside, to the model bottom (the
//! Kiri:Moto "Outline" and the FreeCAD Profile default).
//!
//! The ring is EXACT to the triangles, not a raster (design note
//! `planning/geometry_selection_2026-10-16/RA_OUTLINE_DESIGN.md` section 1):
//!
//! 1. Each triangle with an XY area goes to the group of its facing (up or
//!    down); a vertical triangle gives nothing. A down triangle is turned,
//!    so each group is counter-clockwise in XY.
//! 2. The boundary edges of each group are the directed edges that no
//!    triangle of the group crosses back (the net count of `(a, b)` less
//!    `(b, a)`). They chain into rings; a ring that winds clockwise is a
//!    hole of its patch and is dropped (the outline holds no hole).
//! 3. [`Polygon2::union_all`] of the rings, as in `flat_regions`: it also
//!    closes the T-junction seams between two STEP faces.
//! 4. [`silhouette_machining_outline`]: the outer ring of the largest
//!    area, with no hole (ruling R1, one rule with the boundary).
//! 5. The ring is made canonical: counter-clockwise, no repeated or
//!    collinear point, and the first point is the lowest Y, then the
//!    lowest X. So two imports of one file give one point order, and a
//!    Profile starts at one point.
//!
//! The source is pure: it reads no cache and no session. The model door
//! (`io::model_from_geometry`) calls [`part_outline_item`] once per mesh.

use std::collections::HashMap;

use crate::geo::P2;
use crate::geometry::boundary::silhouette_machining_outline;
use crate::mesh::TriangleMesh;
use crate::polygon::Polygon2;

use super::item::{Item, PartOutline, Shape, SourceRef};
use super::key::KEY_MATCH_EPS_MM;

/// The name of the part outline on every surface (R-A): the GUI row of
/// the Geometry tab and the MCP `role` text.
pub const PART_OUTLINE_LABEL: &str = "Part outline";

/// A point of the ring that is within this distance (mm) of the line of
/// its two neighbours, or of the point before it, goes. It is the key
/// match distance ([`KEY_MATCH_EPS_MM`]): a smaller step does not change
/// a key, so it is noise of the union, not a corner of the part.
const OUTLINE_POINT_EPS_MM: f64 = KEY_MATCH_EPS_MM;

/// The part outline of `mesh` as a counter-clockwise ring (see the module
/// documentation), or `None` when no triangle has an XY area.
#[must_use]
pub fn part_outline_ring(mesh: &TriangleMesh) -> Option<Polygon2> {
    // The net count of each directed edge, one map per facing.
    let mut up: HashMap<(u32, u32), i32> = HashMap::new();
    let mut down: HashMap<(u32, u32), i32> = HashMap::new();
    for tri in &mesh.triangles {
        let corner = |k: usize| {
            tri.get(k)
                .and_then(|&v| mesh.vertices.get(v as usize))
                .copied()
        };
        let (Some(a), Some(b), Some(c)) = (corner(0), corner(1), corner(2)) else {
            continue;
        };
        let area2 = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        if !area2.is_finite() || area2 == 0.0 {
            continue;
        }
        let [i, j, k] = *tri;
        // Counter-clockwise in XY: a down triangle is turned.
        let (map, ring) = if area2 > 0.0 {
            (&mut up, [i, j, k])
        } else {
            (&mut down, [i, k, j])
        };
        for (from, to) in [(ring[0], ring[1]), (ring[1], ring[2]), (ring[2], ring[0])] {
            *map.entry((from, to)).or_insert(0) += 1;
            *map.entry((to, from)).or_insert(0) -= 1;
        }
    }
    let mut rings: Vec<Polygon2> = Vec::new();
    for map in [up, down] {
        rings.extend(boundary_rings(mesh, &map));
    }
    let merged = Polygon2::union_all(&rings);
    let outer = silhouette_machining_outline(&merged)?;
    canonical_ring(outer.exterior)
}

/// The counter-clockwise boundary rings of one facing group: the directed
/// edges with a positive net count, chained from the lowest vertex index.
/// A chain that does not close (a defect of the mesh) gives no ring.
fn boundary_rings(mesh: &TriangleMesh, net: &HashMap<(u32, u32), i32>) -> Vec<Polygon2> {
    let mut next: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut edges: Vec<(u32, u32)> = net
        .iter()
        .filter(|(_, n)| **n > 0)
        .flat_map(|(&e, &n)| std::iter::repeat_n(e, usize::try_from(n).unwrap_or(0)))
        .collect();
    // A fixed order, so the chains do not depend on the hash order.
    edges.sort_unstable();
    for (from, to) in edges.iter().rev() {
        next.entry(*from).or_default().push(*to);
    }
    let mut rings = Vec::new();
    for &(start, _) in &edges {
        while next.get(&start).is_some_and(|v| !v.is_empty()) {
            let mut chain = vec![start];
            let mut at = start;
            let closed = loop {
                let Some(to) = next.get_mut(&at).and_then(Vec::pop) else {
                    break false;
                };
                if to == start {
                    break true;
                }
                chain.push(to);
                at = to;
            };
            if !closed {
                continue;
            }
            let pts: Vec<P2> = chain
                .iter()
                .filter_map(|&v| mesh.vertices.get(v as usize))
                .map(|p| P2::new(p.x, p.y))
                .collect();
            let ring = Polygon2::new(pts);
            if ring.exterior.len() >= 3 && ring.signed_area() > 0.0 {
                rings.push(ring);
            }
        }
    }
    rings
}

/// The ring in its canonical form: counter-clockwise, with no repeated
/// point, no collinear point ([`OUTLINE_POINT_EPS_MM`]), and the lowest Y
/// (then the lowest X) first. `None` when fewer than 3 points remain.
fn canonical_ring(mut pts: Vec<P2>) -> Option<Polygon2> {
    if shoelace(&pts) < 0.0 {
        pts.reverse();
    }
    // Repeat until no point goes: a removal can make its neighbour
    // collinear.
    loop {
        let n = pts.len();
        if n < 3 {
            return None;
        }
        let keep: Vec<bool> = (0..n)
            .map(|i| {
                let (Some(prev), Some(p), Some(nxt)) =
                    (pts.get((i + n - 1) % n), pts.get(i), pts.get((i + 1) % n))
                else {
                    return true;
                };
                (p - prev).norm() > OUTLINE_POINT_EPS_MM
                    && line_distance(*prev, *nxt, *p) > OUTLINE_POINT_EPS_MM
            })
            .collect();
        if keep.iter().all(|k| *k) {
            break;
        }
        // Drop one point per pass at most between two kept points, so two
        // neighbours never go together (a short edge keeps one end).
        let mut out = Vec::with_capacity(n);
        let mut dropped_last = false;
        for (p, k) in pts.iter().zip(&keep) {
            if *k || dropped_last {
                out.push(*p);
                dropped_last = false;
            } else {
                dropped_last = true;
            }
        }
        pts = out;
    }
    let start = pts
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)))
        .map_or(0, |(i, _)| i);
    pts.rotate_left(start);
    Some(Polygon2::new(pts))
}

/// Twice the signed area of a ring (positive: counter-clockwise).
fn shoelace(pts: &[P2]) -> f64 {
    let n = pts.len();
    (0..n)
        .filter_map(|i| Some((pts.get(i)?, pts.get((i + 1) % n)?)))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum()
}

/// The distance of `p` from the line through `a` and `b` (the distance to
/// `a` when the two are one point).
fn line_distance(a: P2, b: P2, p: P2) -> f64 {
    let d = b - a;
    let len = d.norm();
    if len <= f64::EPSILON {
        return (p - a).norm();
    }
    (d.x * (p.y - a.y) - d.y * (p.x - a.x)).abs() / len
}

/// The part-outline item of `mesh` (R-A): a `Loop` of
/// [`part_outline_ring`] at the model bottom Z (`bbox.min.z`), bound to
/// [`SourceRef::ToolpathModel`] (the session binds it to the model), with
/// the [`PartOutline`] marker that holds the model top Z. `None` when the
/// mesh gives no ring.
#[must_use]
pub fn part_outline_item(mesh: &TriangleMesh) -> Option<Item> {
    let ring = part_outline_ring(mesh)?;
    Item::at_z(
        SourceRef::ToolpathModel,
        None,
        Shape::Loop(ring),
        mesh.bbox.min.z,
    )
    .ok()
    .map(|item| {
        item.with_part_outline(PartOutline {
            top_z: mesh.bbox.max.z,
        })
    })
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::geo::P3;

    /// A closed box from (0,0,0) to (w,h,d), 12 triangles, outward.
    fn block(w: f64, h: f64, d: f64) -> TriangleMesh {
        let v = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(w, 0.0, 0.0),
            P3::new(w, h, 0.0),
            P3::new(0.0, h, 0.0),
            P3::new(0.0, 0.0, d),
            P3::new(w, 0.0, d),
            P3::new(w, h, d),
            P3::new(0.0, h, d),
        ];
        let t = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        TriangleMesh::from_raw(v, t)
    }

    #[test]
    fn a_block_gives_its_rectangle_from_the_lowest_corner() {
        let ring = part_outline_ring(&block(30.0, 20.0, 10.0)).unwrap();
        let got: Vec<(f64, f64)> = ring.exterior.iter().map(|p| (p.x, p.y)).collect();
        assert_eq!(
            got,
            vec![(0.0, 0.0), (30.0, 0.0), (30.0, 20.0), (0.0, 20.0)]
        );
    }

    #[test]
    fn the_item_is_a_loop_at_the_bottom_with_the_top_marker() {
        let item = part_outline_item(&block(30.0, 20.0, 10.0)).unwrap();
        assert_eq!(item.z(), Some(0.0));
        assert_eq!(item.part_outline(), Some(PartOutline { top_z: 10.0 }));
        assert!(matches!(item.shape(), Shape::Loop(_)));
    }

    #[test]
    fn a_vertical_sheet_gives_no_outline() {
        let v = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 1.0),
        ];
        assert!(part_outline_ring(&TriangleMesh::from_raw(v, vec![[0, 1, 2]])).is_none());
    }
}
