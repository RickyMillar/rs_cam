//! Extend an open path to the stock edge (geometry selection P6.4,
//! operator ruling O-2; `P6_DESIGN.md` 3.3 and 3.4).
//!
//! A Pocket with `OpenPaths::ExtendLeft` or `ExtendRight` cuts an open path
//! as a closed area: each end of the path goes on along its tangent to the
//! stock edge, and the stock edge closes the area on the stated side of the
//! drawn direction. The functions here are pure. They read no stock config
//! and no tool: the caller gives the stock rectangle in the setup frame
//! (`ResolvedSelection::stock_xy`), grown by the tool radius (O-2 (B)).
//!
//! Two steps, so that a later "ends to the stock" option of Trace and
//! Profile On (lead ruling L-3) can use the first step alone:
//!
//! 1. [`extend_ends`]: each end inside the stock goes on along its tangent
//!    to the stock edge (a ray). An end on the edge gets no ray.
//! 2. [`close_to_stock`]: the stock edge closes the extended path on the
//!    stated side. The result is one closed ring, counter-clockwise.
//!
//! Each failure is a [`CannotExtend`]; `view` turns it into a refusal that
//! names the path (`RefusalCause::CannotExtend`).

use crate::geo::P2;
use crate::polygon::Polygon2;

use super::join::CLOSE_TOL_MM;
use super::refusal::ItemRef;

/// The side of the drawn direction of an open path that an op cuts
/// (geometry selection P6.4, ruling O-2). The side is in the DRAWING frame:
/// a setup that mirrors the drawing (a `Bottom` setup) gives the other side
/// in the setup frame, as Trace does (`TraceParams::drawing_mirrored`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathSide {
    /// The area on the left of the direction of travel.
    Left,
    /// The area on the right of the direction of travel.
    Right,
}

impl PathSide {
    /// The other side.
    #[must_use]
    pub fn flipped(self) -> Self {
        match self {
            PathSide::Left => PathSide::Right,
            PathSide::Right => PathSide::Left,
        }
    }
}

/// The stock rectangle in the XY plane of the setup frame (geometry
/// selection P6.4, lead ruling L-9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StockXy {
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
}

impl StockXy {
    /// The rectangle `[x_min, x_max] x [y_min, y_max]`, or `None` when a
    /// bound is not finite or the rectangle has no area.
    #[must_use]
    pub fn new(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> Option<Self> {
        let finite = [x_min, y_min, x_max, y_max].iter().all(|v| v.is_finite());
        (finite && x_max > x_min && y_max > y_min).then_some(Self {
            x_min,
            y_min,
            x_max,
            y_max,
        })
    }

    /// The same rectangle, `by` larger on each side (O-2 (B): the tool
    /// radius, so the tool centre reaches the stock edge).
    #[must_use]
    pub fn grown(self, by: f64) -> Self {
        Self {
            x_min: self.x_min - by,
            y_min: self.y_min - by,
            x_max: self.x_max + by,
            y_max: self.y_max + by,
        }
    }

    fn width(&self) -> f64 {
        self.x_max - self.x_min
    }

    fn height(&self) -> f64 {
        self.y_max - self.y_min
    }

    fn perimeter(&self) -> f64 {
        2.0 * (self.width() + self.height())
    }

    /// The signed distance from `p` to the nearest edge line: positive
    /// inside, negative outside.
    fn edge_gap(&self, p: P2) -> f64 {
        (p.x - self.x_min)
            .min(self.x_max - p.x)
            .min(p.y - self.y_min)
            .min(self.y_max - p.y)
    }

    /// True when `p` is inside, or outside by `CLOSE_TOL_MM` or less.
    fn holds(&self, p: P2) -> bool {
        self.edge_gap(p) >= -CLOSE_TOL_MM
    }

    /// True when `p` is within `CLOSE_TOL_MM` of the edge (inside or
    /// outside).
    fn on_edge(&self, p: P2) -> bool {
        self.edge_gap(p).abs() <= CLOSE_TOL_MM
    }

    /// The point of the edge nearest to `p`: `p` clamped into the
    /// rectangle, then moved onto its nearest edge.
    fn snap(&self, p: P2) -> P2 {
        let x = p.x.clamp(self.x_min, self.x_max);
        let y = p.y.clamp(self.y_min, self.y_max);
        let gaps = [
            (x - self.x_min, 0),
            (self.x_max - x, 1),
            (y - self.y_min, 2),
            (self.y_max - y, 3),
        ];
        let nearest = gaps
            .iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(0, |g| g.1);
        match nearest {
            0 => P2::new(self.x_min, y),
            1 => P2::new(self.x_max, y),
            2 => P2::new(x, self.y_min),
            _ => P2::new(x, self.y_max),
        }
    }

    /// Where the ray from `p` (inside) along the unit direction `d` leaves
    /// the rectangle. The rectangle is convex, so the ray leaves it once.
    fn ray_hit(&self, p: P2, d: P2) -> P2 {
        let mut t = f64::INFINITY;
        if d.x > 0.0 {
            t = t.min((self.x_max - p.x) / d.x);
        } else if d.x < 0.0 {
            t = t.min((self.x_min - p.x) / d.x);
        }
        if d.y > 0.0 {
            t = t.min((self.y_max - p.y) / d.y);
        } else if d.y < 0.0 {
            t = t.min((self.y_min - p.y) / d.y);
        }
        self.snap(P2::new(p.x + d.x * t, p.y + d.y * t))
    }

    /// The position of the edge point `h` along the edge, counter-clockwise
    /// from the corner `(x_min, y_min)`, in `[0, perimeter)`.
    fn position(&self, h: P2) -> f64 {
        let (w, d) = (self.width(), self.height());
        let gaps = [
            (h.y - self.y_min).abs(),
            (self.x_max - h.x).abs(),
            (self.y_max - h.y).abs(),
            (h.x - self.x_min).abs(),
        ];
        let edge = gaps
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |(i, _)| i);
        let s = match edge {
            0 => h.x - self.x_min,
            1 => w + (h.y - self.y_min),
            2 => w + d + (self.x_max - h.x),
            _ => 2.0 * w + d + (self.y_max - h.y),
        };
        s.rem_euclid(self.perimeter())
    }

    /// The four corners with their edge positions, counter-clockwise.
    fn corners(&self) -> [(f64, P2); 4] {
        let (w, d) = (self.width(), self.height());
        [
            (0.0, P2::new(self.x_min, self.y_min)),
            (w, P2::new(self.x_max, self.y_min)),
            (w + d, P2::new(self.x_max, self.y_max)),
            (2.0 * w + d, P2::new(self.x_min, self.y_max)),
        ]
    }
}

/// Why an open path cannot extend to the stock (`P6_DESIGN.md` 3.4). Each
/// gives a refusal that names the path.
#[derive(Debug, Clone, PartialEq)]
pub enum CannotExtend {
    /// An end of the path is outside the stock.
    EndOutsideStock,
    /// The path crosses or touches the stock edge between its ends.
    CrossesStockEdge,
    /// The path crosses itself, or a ray from one end crosses the path.
    SelfCrossing,
    /// The two rays cross or meet inside the stock: the ends point at each
    /// other.
    RaysCross,
    /// The area has no size: the path lies on the stock edge, or is
    /// shorter than `CLOSE_TOL_MM`.
    NoArea,
    /// A selected closed ring crosses or touches the edge of the extended
    /// area. It does not nest, so it can be no island (a ring that crosses
    /// the area would be cut as its own pocket where the user drew an
    /// island).
    CrossesItem {
        /// The ring that crosses the area (boxed: a key is large, and a
        /// refusal travels in every `Result` of the session).
        other: Box<ItemRef>,
    },
    /// The selection has no stock rectangle: a hand-built selection, or a
    /// view in the model frame (an internal case).
    NoStock,
}

impl CannotExtend {
    /// The cause as a phrase, after "the open path cannot extend to the
    /// stock: ".
    pub(crate) fn phrase(&self) -> String {
        match self {
            CannotExtend::EndOutsideStock => "an end of the path is outside the stock".to_owned(),
            CannotExtend::CrossesStockEdge => {
                "the path crosses the stock edge between its ends".to_owned()
            }
            CannotExtend::SelfCrossing => "the path crosses itself after extension".to_owned(),
            CannotExtend::RaysCross => "the two ends point at each other".to_owned(),
            CannotExtend::NoArea => {
                "the path lies on the stock edge, so the area has no size".to_owned()
            }
            CannotExtend::CrossesItem { other } => {
                format!("the {other} crosses the extended area, so it can be no island")
            }
            CannotExtend::NoStock => "the selection has no stock rectangle".to_owned(),
        }
    }

    /// The fix, one text per cause.
    pub(crate) fn fix(&self) -> &'static str {
        match self {
            CannotExtend::EndOutsideStock => "Trim the path to the stock, or move the stock",
            CannotExtend::CrossesStockEdge => "Split the path at the stock edge",
            CannotExtend::SelfCrossing => "Edit the drawing so that the path does not cross itself",
            CannotExtend::RaysCross => {
                "Close the path with the drawing edit Close with line, and cut it as a loop"
            }
            CannotExtend::NoArea => "Move the path off the stock edge, or flip the side",
            CannotExtend::CrossesItem { .. } => {
                "Remove the loop from the selection, or edit the drawing so that the loop is \
                 fully inside or fully outside the extended area"
            }
            CannotExtend::NoStock => "Generate the op in a project that has a stock",
        }
    }
}

/// An open path with each end carried to the stock edge
/// ([`extend_ends`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtendedEnds {
    /// The path, with each end on the edge snapped onto it, and with no
    /// two equal points in sequence.
    pub path: Vec<P2>,
    /// Where the ray from the start leaves the stock; the start itself
    /// when it is on the edge.
    pub start_hit: P2,
    /// Where the ray from the end leaves the stock; the end itself when it
    /// is on the edge.
    pub end_hit: P2,
}

/// Points closer than this (mm) are one point. It only removes repeated
/// points; `CLOSE_TOL_MM` is the drawing tolerance.
const SAME_POINT_MM: f64 = 1e-9;

/// Step 1 of `P6_DESIGN.md` 3.3: each end of the open `path` carried to the
/// edge of `stock` along its tangent.
///
/// The checks, in the order of the refusal rows of 3.4:
///
/// 1. Each end is inside the stock or on its edge (`EndOutsideStock`).
/// 2. No point between the ends is outside the stock or on its edge
///    (`CrossesStockEdge`). The stock is convex, so a segment between two
///    inside points stays inside.
/// 3. The path does not cross or touch itself (`SelfCrossing`).
/// 4. The tangent at an end is the direction of its last segment, read
///    back over segments shorter than `CLOSE_TOL_MM` (`NoArea` when the
///    path is shorter). The ray from an end inside the stock leaves it
///    once. An end on the edge gets no ray.
/// 5. The two rays do not cross or meet (`RaysCross`), and no ray crosses
///    the path (`SelfCrossing`).
pub fn extend_ends(path: &[P2], stock: &StockXy) -> Result<ExtendedEnds, CannotExtend> {
    let mut pts: Vec<P2> = Vec::with_capacity(path.len());
    for &p in path {
        if pts
            .last()
            .is_none_or(|q: &P2| (p - *q).norm() > SAME_POINT_MM)
        {
            pts.push(p);
        }
    }
    let (Some(&first), Some(&last)) = (pts.first(), pts.last()) else {
        return Err(CannotExtend::NoArea);
    };
    if pts.len() < 2 {
        return Err(CannotExtend::NoArea);
    }
    if !stock.holds(first) || !stock.holds(last) {
        return Err(CannotExtend::EndOutsideStock);
    }
    let n = pts.len();
    if pts
        .iter()
        .take(n - 1)
        .skip(1)
        .any(|&p| !stock.holds(p) || stock.on_edge(p))
    {
        return Err(CannotExtend::CrossesStockEdge);
    }
    if path_crosses_itself(&pts) {
        return Err(CannotExtend::SelfCrossing);
    }

    // The tangent at the start points back along the first segment; the
    // tangent at the end points on along the last segment.
    let start_dir = tangent(pts.iter().copied());
    let end_dir = tangent(pts.iter().rev().copied());
    let (Some(start_dir), Some(end_dir)) = (start_dir, end_dir) else {
        return Err(CannotExtend::NoArea);
    };
    let start_on_edge = stock.on_edge(first);
    let end_on_edge = stock.on_edge(last);
    if start_on_edge && let Some(p) = pts.first_mut() {
        *p = stock.snap(first);
    }
    if end_on_edge && let Some(p) = pts.last_mut() {
        *p = stock.snap(last);
    }
    let first = pts.first().copied().unwrap_or(first);
    let last = pts.last().copied().unwrap_or(last);
    let start_hit = if start_on_edge {
        first
    } else {
        stock.ray_hit(first, start_dir)
    };
    let end_hit = if end_on_edge {
        last
    } else {
        stock.ray_hit(last, end_dir)
    };

    let start_ray = (!start_on_edge).then_some((first, start_hit));
    let end_ray = (!end_on_edge).then_some((last, end_hit));
    if let (Some((a, b)), Some((c, d))) = (start_ray, end_ray)
        && segments_touch(a, b, c, d)
    {
        return Err(CannotExtend::RaysCross);
    }
    let segments: Vec<(P2, P2)> = pts
        .windows(2)
        .filter_map(|w| Some((*w.first()?, *w.get(1)?)))
        .collect();
    let m = segments.len();
    // A ray shares its own end with the segment at that end; it may meet
    // no other segment.
    if let Some((a, b)) = start_ray
        && segments
            .iter()
            .skip(1)
            .any(|&(p, q)| segments_touch(a, b, p, q))
    {
        return Err(CannotExtend::SelfCrossing);
    }
    if let Some((a, b)) = end_ray
        && segments
            .iter()
            .take(m.saturating_sub(1))
            .any(|&(p, q)| segments_touch(a, b, p, q))
    {
        return Err(CannotExtend::SelfCrossing);
    }
    Ok(ExtendedEnds {
        path: pts,
        start_hit,
        end_hit,
    })
}

/// Step 2 of `P6_DESIGN.md` 3.3: the extended path closed by the stock
/// edge on `side`, as one counter-clockwise ring.
///
/// `Left`: the ring runs along the path, then counter-clockwise along the
/// stock edge from the end hit back to the start hit, with the corners on
/// the way. The area is then on the left of the path. `Right`: clockwise
/// along the edge, then the ring turns to counter-clockwise. An area with
/// no size refuses `NoArea`.
pub fn close_to_stock(
    ends: &ExtendedEnds,
    stock: &StockXy,
    side: PathSide,
) -> Result<Polygon2, CannotExtend> {
    let (Some(&first), Some(&last)) = (ends.path.first(), ends.path.last()) else {
        return Err(CannotExtend::NoArea);
    };
    let mut ring: Vec<P2> = Vec::with_capacity(ends.path.len() + 6);
    if (ends.start_hit - first).norm() > SAME_POINT_MM {
        ring.push(ends.start_hit);
    }
    ring.extend_from_slice(&ends.path);
    if (ends.end_hit - last).norm() > SAME_POINT_MM {
        ring.push(ends.end_hit);
    }

    let perimeter = stock.perimeter();
    let from = stock.position(ends.end_hit);
    let to = stock.position(ends.start_hit);
    // The walk from the end hit to the start hit, by the edge position
    // offset of each corner from the end hit.
    let offset = |s: f64| match side {
        PathSide::Left => (s - from).rem_euclid(perimeter),
        PathSide::Right => (from - s).rem_euclid(perimeter),
    };
    let length = offset(to);
    let mut corners: Vec<(f64, P2)> = stock
        .corners()
        .into_iter()
        .map(|(s, p)| (offset(s), p))
        .filter(|(o, _)| *o > SAME_POINT_MM && *o < length - SAME_POINT_MM)
        .collect();
    corners.sort_by(|a, b| a.0.total_cmp(&b.0));
    ring.extend(corners.into_iter().map(|(_, p)| p));

    let mut polygon = Polygon2::new(ring);
    if polygon.has_self_intersection() {
        return Err(CannotExtend::SelfCrossing);
    }
    polygon.ensure_winding();
    // An area thinner than about two `CLOSE_TOL_MM` has no size.
    if polygon.area() <= CLOSE_TOL_MM * polygon.perimeter() {
        return Err(CannotExtend::NoArea);
    }
    Ok(polygon)
}

/// [`extend_ends`], then [`close_to_stock`].
pub fn extend_to_stock(
    path: &[P2],
    stock: &StockXy,
    side: PathSide,
) -> Result<Polygon2, CannotExtend> {
    close_to_stock(&extend_ends(path, stock)?, stock, side)
}

/// True when the closed rings `a` and `b` cross or touch.
pub(crate) fn rings_touch(a: &[P2], b: &[P2]) -> bool {
    let edges = |ring: &[P2]| -> Vec<(P2, P2)> {
        let n = ring.len();
        (0..n)
            .filter_map(|i| Some((*ring.get(i)?, *ring.get((i + 1) % n)?)))
            .collect()
    };
    let (ea, eb) = (edges(a), edges(b));
    ea.iter().any(|&(p, q)| {
        eb.iter()
            .any(|&(r, s)| boxes_overlap(p, q, r, s) && segments_touch(p, q, r, s))
    })
}

/// The unit direction from the first point of `pts` away from the next
/// point that is `CLOSE_TOL_MM` or more from it: the outward tangent at
/// that end. `None` when no such point exists.
fn tangent(mut pts: impl Iterator<Item = P2>) -> Option<P2> {
    let end = pts.next()?;
    let back = pts.find(|p| (end - *p).norm() >= CLOSE_TOL_MM)?;
    let d = end - back;
    let len = d.norm();
    Some(P2::new(d.x / len, d.y / len))
}

/// True when two segments of the open path that are not neighbours cross
/// or touch.
fn path_crosses_itself(pts: &[P2]) -> bool {
    let segments: Vec<(P2, P2)> = pts
        .windows(2)
        .filter_map(|w| Some((*w.first()?, *w.get(1)?)))
        .collect();
    segments.iter().enumerate().any(|(i, &(a, b))| {
        segments
            .iter()
            .skip(i + 2)
            .any(|&(c, d)| boxes_overlap(a, b, c, d) && segments_touch(a, b, c, d))
    })
}

/// The tolerance (mm squared) of the orientation test: well above the
/// rounding of a cross product of coordinates of some hundred mm.
const ORIENT_EPS: f64 = 1e-9;

fn orient(a: P2, b: P2, c: P2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// True when `p` is on the segment `a`-`b`, given that the three points
/// are collinear.
fn within(a: P2, b: P2, p: P2) -> bool {
    let eps = SAME_POINT_MM;
    p.x >= a.x.min(b.x) - eps
        && p.x <= a.x.max(b.x) + eps
        && p.y >= a.y.min(b.y) - eps
        && p.y <= a.y.max(b.y) + eps
}

fn boxes_overlap(a: P2, b: P2, c: P2, d: P2) -> bool {
    let eps = SAME_POINT_MM;
    a.x.min(b.x) <= c.x.max(d.x) + eps
        && c.x.min(d.x) <= a.x.max(b.x) + eps
        && a.y.min(b.y) <= c.y.max(d.y) + eps
        && c.y.min(d.y) <= a.y.max(b.y) + eps
}

/// True when the segments `a`-`b` and `c`-`d` cross or touch (a shared
/// point counts).
fn segments_touch(a: P2, b: P2, c: P2, d: P2) -> bool {
    let d1 = orient(c, d, a);
    let d2 = orient(c, d, b);
    let d3 = orient(a, b, c);
    let d4 = orient(a, b, d);
    let opposite =
        |u: f64, v: f64| (u > ORIENT_EPS && v < -ORIENT_EPS) || (u < -ORIENT_EPS && v > ORIENT_EPS);
    if opposite(d1, d2) && opposite(d3, d4) {
        return true;
    }
    (d1.abs() <= ORIENT_EPS && within(c, d, a))
        || (d2.abs() <= ORIENT_EPS && within(c, d, b))
        || (d3.abs() <= ORIENT_EPS && within(a, b, c))
        || (d4.abs() <= ORIENT_EPS && within(a, b, d))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    //! P6.4 lib sentries: each refusal row of `P6_DESIGN.md` 3.4; Left and
    //! Right of one L are the two complements of the stock; an end on the
    //! edge needs no ray; a loop inside the Left area becomes its island.

    use super::*;
    use crate::compute::catalog::ItemUse;
    use crate::selection::item::{Item, Shape, SourceRef};
    use crate::selection::refusal::RefusalCause;
    use crate::selection::resolve::ResolvedSelection;
    use crate::selection::view::{ItemRole, view};

    /// The test stock: X 0 to 100, Y 0 to 60.
    fn stock() -> StockXy {
        StockXy::new(0.0, 0.0, 100.0, 60.0).unwrap()
    }

    fn pts(xy: &[[f64; 2]]) -> Vec<P2> {
        xy.iter().map(|&[x, y]| P2::new(x, y)).collect()
    }

    /// The L: north from (20, 10) to (20, 40), then east to (70, 40). The
    /// start ray goes south to (20, 0); the end ray goes east to (100, 40).
    const L: [[f64; 2]; 3] = [[20.0, 10.0], [20.0, 40.0], [70.0, 40.0]];

    fn path_item(xy: &[[f64; 2]]) -> Item {
        Item::new(
            SourceRef::ToolpathModel,
            Some("OPEN".into()),
            Shape::Path(Polygon2::open_path(pts(xy))),
        )
        .unwrap()
    }

    fn loop_item(x0: f64, y0: f64, x1: f64, y1: f64) -> Item {
        Item::new(
            SourceRef::ToolpathModel,
            Some("LOOP".into()),
            Shape::Loop(Polygon2::rectangle(x0, y0, x1, y1)),
        )
        .unwrap()
    }

    fn extend_view(
        items: Vec<Item>,
        side: PathSide,
    ) -> Result<crate::selection::OpItems, crate::selection::SelectionRefusal> {
        let resolved = ResolvedSelection::from_items(items).with_stock_xy(stock());
        view(&resolved, ItemUse::RegionsExtendOpen { side }).map(|(items, _)| items)
    }

    #[test]
    fn left_and_right_of_one_l_are_the_two_complements_of_the_stock() {
        let left = extend_to_stock(&pts(&L), &stock(), PathSide::Left).unwrap();
        let right = extend_to_stock(&pts(&L), &stock(), PathSide::Right).unwrap();
        assert!(
            left.signed_area() > 0.0 && right.signed_area() > 0.0,
            "both rings are CCW"
        );
        let total = 100.0 * 60.0;
        assert!(
            (left.area() + right.area() - total).abs() < 1e-9,
            "Left {} + Right {} is not the stock {total}",
            left.area(),
            right.area()
        );
        // The Left area of the L (north, then east) is west and north of
        // it: the corner (0, 60). The Right area holds (100, 0).
        let west_north = P2::new(5.0, 55.0);
        let east_south = P2::new(95.0, 5.0);
        assert!(left.contains_point(&west_north) && !left.contains_point(&east_south));
        assert!(right.contains_point(&east_south) && !right.contains_point(&west_north));
        // Left = (20 x 60 west of the start ray) + (80 x 20 north of the
        // end ray): 1200 + 1600.
        assert!(
            (left.area() - 2800.0).abs() < 1e-9,
            "Left area {}",
            left.area()
        );
    }

    #[test]
    fn an_end_on_the_edge_needs_no_ray() {
        // The start is on the west edge; the end ray goes east.
        let path = pts(&[[0.0, 30.0], [50.0, 30.0]]);
        let ends = extend_ends(&path, &stock()).unwrap();
        assert_eq!(ends.start_hit, P2::new(0.0, 30.0), "no ray at the start");
        assert_eq!(
            ends.end_hit,
            P2::new(100.0, 30.0),
            "the end ray reaches the east edge"
        );
        let left = close_to_stock(&ends, &stock(), PathSide::Left).unwrap();
        // The start, the end, the end hit and the two north corners.
        assert_eq!(left.exterior.len(), 5, "{:?}", left.exterior);
        assert!((left.area() - 100.0 * 30.0).abs() < 1e-9);
        // An end within `CLOSE_TOL_MM` outside the edge snaps onto it.
        let near = pts(&[[-0.005, 30.0], [50.0, 30.0]]);
        let ends = extend_ends(&near, &stock()).unwrap();
        assert_eq!(ends.start_hit, P2::new(0.0, 30.0));
        assert_eq!(ends.path.first().copied(), Some(P2::new(0.0, 30.0)));
    }

    #[test]
    fn each_failure_row_refuses_by_its_cause() {
        let s = stock();
        let side = PathSide::Left;
        let cases: [(&str, Vec<P2>, CannotExtend); 5] = [
            (
                "end outside",
                pts(&[[20.0, 30.0], [120.0, 30.0]]),
                CannotExtend::EndOutsideStock,
            ),
            (
                "crosses the edge",
                pts(&[[20.0, 30.0], [50.0, 80.0], [80.0, 30.0]]),
                CannotExtend::CrossesStockEdge,
            ),
            (
                "touches the edge",
                pts(&[[20.0, 30.0], [50.0, 60.0], [80.0, 30.0]]),
                CannotExtend::CrossesStockEdge,
            ),
            (
                "crosses itself",
                pts(&[[20.0, 20.0], [60.0, 40.0], [60.0, 20.0], [20.0, 40.0]]),
                CannotExtend::SelfCrossing,
            ),
            (
                "ends point at each other",
                pts(&[
                    [45.0, 30.0],
                    [35.0, 30.0],
                    [35.0, 40.0],
                    [65.0, 40.0],
                    [65.0, 30.0],
                    [55.0, 30.0],
                ]),
                CannotExtend::RaysCross,
            ),
        ];
        for (name, path, why) in cases {
            assert_eq!(extend_to_stock(&path, &s, side).err(), Some(why), "{name}");
        }
        // A ray that crosses the path: a hook whose start ray (west) runs
        // through its own last leg.
        let hook = pts(&[
            [50.0, 30.0],
            [70.0, 30.0],
            [70.0, 50.0],
            [30.0, 50.0],
            [30.0, 20.0],
        ]);
        assert_eq!(
            extend_to_stock(&hook, &s, side).err(),
            Some(CannotExtend::SelfCrossing),
            "a ray crosses the path"
        );
        // The path on the stock edge: its Right side has no area; its Left
        // side is the whole stock.
        let on_edge = pts(&[[20.0, 0.0], [80.0, 0.0]]);
        assert_eq!(
            extend_to_stock(&on_edge, &s, PathSide::Right).err(),
            Some(CannotExtend::NoArea)
        );
        let whole = extend_to_stock(&on_edge, &s, PathSide::Left).unwrap();
        assert!((whole.area() - 6000.0).abs() < 1e-9);
        // A path shorter than the drawing tolerance has no tangent.
        assert_eq!(
            extend_to_stock(&pts(&[[20.0, 20.0], [20.004, 20.0]]), &s, side).err(),
            Some(CannotExtend::NoArea)
        );
    }

    #[test]
    fn a_loop_inside_the_left_area_becomes_its_island() {
        // The loop at (5..15, 45..55) is in the Left area of the L; the
        // loop at (80..90, 5..15) is in its Right area.
        let items = vec![
            path_item(&L),
            loop_item(5.0, 45.0, 15.0, 55.0),
            loop_item(80.0, 5.0, 90.0, 15.0),
        ];
        let op_items = extend_view(items, PathSide::Left).unwrap();
        assert_eq!(
            op_items.roles(),
            &[ItemRole::Cut, ItemRole::Island, ItemRole::Cut],
            "the path is cut; the loop inside its area is its island; the loop outside is \
             its own area"
        );
        let regions = op_items.regions().unwrap();
        assert_eq!(regions.as_slice().len(), 2);
        let area = &regions.as_slice()[0];
        assert_eq!(
            regions.exterior_item(0),
            Some(0),
            "the path is the exterior"
        );
        assert_eq!(area.holes.len(), 1, "one island");
        assert!(!regions.is_stock(0));
        assert_eq!(regions.exterior_item(1), Some(2));
    }

    #[test]
    fn a_loop_across_the_area_edge_refuses_and_no_stock_refuses() {
        // The loop straddles the end ray at Y 40.
        let items = vec![path_item(&L), loop_item(80.0, 35.0, 90.0, 45.0)];
        let refusal = extend_view(items, PathSide::Left).unwrap_err();
        let RefusalCause::CannotExtend {
            why: CannotExtend::CrossesItem { other },
        } = &refusal.cause
        else {
            panic!("not CrossesItem: {refusal}");
        };
        assert_eq!(other.key.layer.as_deref(), Some("LOOP"));
        assert_eq!(
            refusal.item.as_ref().map(|i| i.key.layer.as_deref()),
            Some(Some("OPEN")),
            "the refusal names the path"
        );
        // A selection with no stock rectangle refuses `NoStock`.
        let resolved = ResolvedSelection::from_items(vec![path_item(&L)]);
        let refusal = view(
            &resolved,
            ItemUse::RegionsExtendOpen {
                side: PathSide::Left,
            },
        )
        .unwrap_err();
        assert_eq!(
            refusal.cause,
            RefusalCause::CannotExtend {
                why: CannotExtend::NoStock
            }
        );
        // With no open path, no stock is needed: the loops are cut as
        // closed areas.
        let resolved = ResolvedSelection::from_items(vec![loop_item(5.0, 5.0, 15.0, 15.0)]);
        assert!(
            view(
                &resolved,
                ItemUse::RegionsExtendOpen {
                    side: PathSide::Left,
                },
            )
            .is_ok()
        );
    }

    #[test]
    fn the_stock_grows_by_the_tool_radius() {
        let resolved = ResolvedSelection::from_items(vec![path_item(&L)])
            .with_stock_xy(stock())
            .with_stock_grown(3.0);
        assert_eq!(
            resolved.stock_xy(),
            StockXy::new(-3.0, -3.0, 103.0, 63.0),
            "O-2 (B): one tool radius past each edge"
        );
    }
}
