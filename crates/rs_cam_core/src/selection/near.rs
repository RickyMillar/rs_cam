//! The distance from a point to an item: the pick rule (geometry selection
//! P4.5).
//!
//! A viewport click and the MCP `list_model_items { near }` read pick by
//! one rule: the item whose drawn lines pass nearest the point. The lines
//! of an item are [`Shape::pick_lines`]. The viewport measures the distance
//! in screen pixels after it projects each vertex; `near` measures it in
//! the model frame in mm. Both call [`polyline_distance`].

use crate::geo::P2;

use super::item::{Item, Shape};

/// One drawn line of an item: its vertices and whether the last vertex
/// joins the first.
pub type PickLine<'a> = (&'a [P2], bool);

impl Shape {
    /// The lines that the viewport draws for this shape, and that a pick
    /// measures:
    ///
    /// - a loop or a path: its ring;
    /// - a circle: its ring, and its centre as a one-vertex line (the drill
    ///   marker draws there);
    /// - a point: the point, as a one-vertex line;
    /// - a face: its outer ring and its own inner loops.
    pub fn pick_lines(&self) -> Vec<PickLine<'_>> {
        match self {
            Shape::Loop(ring) | Shape::Path(ring) => vec![(ring.exterior.as_slice(), ring.closed)],
            Shape::Circle { centre, ring, .. } => vec![
                (ring.exterior.as_slice(), ring.closed),
                (std::slice::from_ref(centre), false),
            ],
            Shape::Point(p) => vec![(std::slice::from_ref(p), false)],
            Shape::Face(face) => std::iter::once((face.rings.exterior.as_slice(), true))
                .chain(face.rings.holes.iter().map(|h| (h.as_slice(), true)))
                .collect(),
        }
    }

    /// The distance (mm, in the frame of the shape) from `p` to the nearest
    /// line of [`Self::pick_lines`].
    pub fn distance_to(&self, p: P2) -> f64 {
        self.pick_lines()
            .into_iter()
            .map(|(pts, closed)| {
                polyline_distance([p.x, p.y], pts.iter().map(|q| [q.x, q.y]), closed)
            })
            .fold(f64::INFINITY, f64::min)
    }
}

/// The distance from `p` to the polyline through `pts`. With `closed`, the
/// last vertex joins the first. One vertex gives the distance to that
/// vertex; no vertex gives infinity. The viewport pick calls it with
/// screen coordinates, `near` with model coordinates.
pub fn polyline_distance(
    p: [f64; 2],
    pts: impl IntoIterator<Item = [f64; 2]>,
    closed: bool,
) -> f64 {
    let mut iter = pts.into_iter();
    let Some(first) = iter.next() else {
        return f64::INFINITY;
    };
    let mut best = point_distance(p, first);
    let mut prev = first;
    for q in iter {
        best = best.min(segment_distance(p, prev, q));
        prev = q;
    }
    if closed {
        best = best.min(segment_distance(p, prev, first));
    }
    best
}

fn point_distance(p: [f64; 2], a: [f64; 2]) -> f64 {
    (p[0] - a[0]).hypot(p[1] - a[1])
}

/// The distance from `p` to the segment `a`–`b`.
fn segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    if len2 <= f64::EPSILON {
        return point_distance(p, a);
    }
    let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0);
    point_distance(p, [a[0] + d[0] * t, a[1] + d[1] * t])
}

/// The items within `radius_mm` of `p` (model frame), nearest first: the
/// index in `items` and the distance. Equal distances keep import order.
pub fn items_near<'a>(
    items: impl IntoIterator<Item = &'a Item>,
    p: P2,
    radius_mm: f64,
) -> Vec<(usize, f64)> {
    let mut near: Vec<(usize, f64)> = items
        .into_iter()
        .enumerate()
        .map(|(i, item)| (i, item.shape().distance_to(p)))
        .filter(|(_, d)| *d <= radius_mm)
        .collect();
    near.sort_by(|a, b| a.1.total_cmp(&b.1));
    near
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::polygon::Polygon2;
    use crate::selection::{ItemKind, SourceRef};

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Item {
        Item::new(
            SourceRef::ToolpathModel,
            Some("CUT".into()),
            Shape::Loop(Polygon2::new(vec![
                P2::new(x0, y0),
                P2::new(x1, y0),
                P2::new(x1, y1),
                P2::new(x0, y1),
            ])),
        )
        .unwrap()
    }

    /// The rule measures to a SEGMENT, not to a vertex: a point at the
    /// middle of a long edge is on the loop (distance 0), though every
    /// vertex is 50 mm away.
    #[test]
    fn the_distance_is_to_the_segment_not_the_vertex_p4_5() {
        let loop_ = square(0.0, 0.0, 100.0, 100.0);
        assert!(loop_.shape().distance_to(P2::new(50.0, 0.0)) < 1e-9);
        assert!((loop_.shape().distance_to(P2::new(50.0, 3.0)) - 3.0).abs() < 1e-9);
        // The closing edge counts.
        assert!(loop_.shape().distance_to(P2::new(0.0, 50.0)) < 1e-9);
    }

    /// `items_near` (the `list_model_items { near }` read) gives the item
    /// nearest the point first, and leaves out the items beyond the radius.
    #[test]
    fn items_near_gives_the_nearest_item_first_p4_5() {
        let circle = Item::new(
            SourceRef::ToolpathModel,
            Some("HOLES".into()),
            Shape::circle(P2::new(5.0, 5.0), 4.0),
        )
        .unwrap();
        let items = [
            square(0.0, 0.0, 60.0, 60.0),
            circle,
            square(70.0, 30.0, 90.0, 50.0),
        ];
        // On the circle ring, 3 mm from the loop edge x = 0.
        let near = items_near(&items, P2::new(3.0, 5.0), 5.0);
        assert_eq!(near.iter().map(|n| n.0).collect::<Vec<_>>(), vec![1, 0]);
        assert_eq!(items[near[0].0].kind(), ItemKind::Circle);
        // A small radius keeps the circle only.
        assert_eq!(items_near(&items, P2::new(3.0, 5.0), 1.0).len(), 1);
    }
}
