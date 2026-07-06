//! 2D polygon types and offset operations.
//!
//! Internal representation uses nalgebra P2. Converts to geo-types and
//! cavalier_contours at operation boundaries per architecture rules.

use crate::geo::P2;
use cavalier_contours::polyline::{PlineCreation, PlineSource, PlineSourceMut, Polyline};

/// A closed 2D polygon with optional holes.
///
/// - `exterior`: outer boundary vertices in CCW order (positive area)
/// - `holes`: inner boundary vertices in CW order (negative area)
///
/// Vertices are not duplicated (last implicitly connects back to first for closed polygons).
#[derive(Debug, Clone)]
pub struct Polygon2 {
    pub exterior: Vec<P2>,
    pub holes: Vec<Vec<P2>>,
    /// If true (default), the last vertex connects back to the first.
    /// Open paths (e.g. rivers, contour lines) set this to false.
    pub closed: bool,
}

impl Polygon2 {
    /// Create a closed polygon from exterior vertices (CCW winding assumed).
    pub fn new(exterior: Vec<P2>) -> Self {
        Self {
            exterior,
            holes: Vec::new(),
            closed: true,
        }
    }

    /// Create an open path (not closed — no edge from last to first vertex).
    pub fn open_path(exterior: Vec<P2>) -> Self {
        Self {
            exterior,
            holes: Vec::new(),
            closed: false,
        }
    }

    /// Create a polygon with holes.
    pub fn with_holes(exterior: Vec<P2>, holes: Vec<Vec<P2>>) -> Self {
        Self {
            exterior,
            holes,
            closed: true,
        }
    }

    /// Create a rectangle from bounds.
    pub fn rectangle(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> Self {
        // CCW winding
        Self::new(vec![
            P2::new(x_min, y_min),
            P2::new(x_max, y_min),
            P2::new(x_max, y_max),
            P2::new(x_min, y_max),
        ])
    }

    /// Signed area via shoelace formula. Positive for CCW, negative for CW.
    pub fn signed_area(&self) -> f64 {
        shoelace_area(&self.exterior)
    }

    /// Absolute area (exterior minus holes).
    pub fn area(&self) -> f64 {
        let ext = self.signed_area().abs();
        let holes: f64 = self.holes.iter().map(|h| shoelace_area(h).abs()).sum();
        ext - holes
    }

    /// Ensure exterior is CCW and holes are CW.
    pub fn ensure_winding(&mut self) {
        if shoelace_area(&self.exterior) < 0.0 {
            self.exterior.reverse();
        }
        for hole in &mut self.holes {
            if shoelace_area(hole) > 0.0 {
                hole.reverse();
            }
        }
    }

    /// True if winding is correct (exterior CCW, holes CW).
    pub fn has_correct_winding(&self) -> bool {
        if shoelace_area(&self.exterior) < 0.0 {
            return false;
        }
        self.holes.iter().all(|h| shoelace_area(h) < 0.0)
    }

    /// Convert to a `geo::Polygon`. geo-types requires the closing vertex duplicated.
    pub fn to_geo_polygon(&self) -> geo::Polygon<f64> {
        let exterior = ring_to_geo(&self.exterior);
        let holes: Vec<geo::LineString<f64>> = self.holes.iter().map(|h| ring_to_geo(h)).collect();
        geo::Polygon::new(exterior, holes)
    }

    /// Create from a `geo::Polygon`. Strips the duplicated closing vertex.
    pub fn from_geo_polygon(poly: &geo::Polygon<f64>) -> Self {
        let exterior = ring_from_geo(poly.exterior());
        let holes: Vec<Vec<P2>> = poly.interiors().iter().map(ring_from_geo).collect();
        Self {
            exterior,
            holes,
            closed: true,
        }
    }

    /// Convert exterior to a cavalier_contours closed Polyline (no arcs).
    pub fn exterior_to_pline(&self) -> Polyline<f64> {
        let mut pline = Polyline::with_capacity(self.exterior.len(), true);
        for p in &self.exterior {
            pline.add(p.x, p.y, 0.0);
        }
        pline
    }

    /// Create from a cavalier_contours Polyline (arcs flattened to line segments).
    ///
    /// Arc segments (non-zero bulge) are approximated as their chord endpoints.
    /// For arc-preserving output, use the Polyline directly.
    pub fn from_pline(pline: &Polyline<f64>) -> Self {
        let exterior: Vec<P2> = pline.iter_vertexes().map(|v| P2::new(v.x, v.y)).collect();
        Self::new(exterior)
    }

    /// Perimeter length of the exterior boundary.
    pub fn perimeter(&self) -> f64 {
        ring_perimeter(&self.exterior)
    }

    /// Test if a point is inside this polygon (inside exterior, not inside any hole).
    pub fn contains_point(&self, p: &P2) -> bool {
        if !point_in_polygon(p, &self.exterior) {
            return false;
        }
        !self.holes.iter().any(|h| point_in_polygon(p, h))
    }

    /// Like `contains_point`, but also treats points within `eps` of any
    /// exterior/hole edge as inside.
    ///
    /// Marching-squares output sits exactly on grid-aligned edges, where
    /// the strict ray-cast's inside/outside call is a coin flip. This
    /// widens the boundary by `eps` so those points read as inside
    /// consistently. Does not change `contains_point` itself — that stays
    /// the hot-path check used by adaptive/material_grid.
    pub fn contains_point_eps(&self, p: &P2, eps: f64) -> bool {
        if self.contains_point(p) {
            return true;
        }
        ring_near_point(&self.exterior, p, eps)
            || self.holes.iter().any(|h| ring_near_point(h, p, eps))
    }

    /// True if the exterior or any hole ring crosses itself.
    ///
    /// A shared endpoint between two *adjacent* segments (the normal ring
    /// connectivity) does not count. A repeated vertex or crossing between
    /// two *non-adjacent* segments (a bowtie pinch) does.
    pub fn has_self_intersection(&self) -> bool {
        ring_has_self_intersection(&self.exterior)
            || self.holes.iter().any(|h| ring_has_self_intersection(h))
    }

    /// Normalize a self-intersecting polygon into simple polygons.
    ///
    /// Uses geo's boolean-overlay engine to union the polygon with itself:
    /// `boolean_op` feeds the exterior/hole rings straight into the
    /// i_overlay engine regardless of self-intersections, and the
    /// even-odd fill rule resolves pinched/bowtied rings into distinct
    /// simple polygons the same way a `buffer(0)` trick does in
    /// JTS/shapely. Non-self-intersecting input returns unchanged.
    pub fn repaired(&self) -> Vec<Polygon2> {
        if !self.has_self_intersection() {
            return vec![self.clone()];
        }
        use geo::BooleanOps;
        let geo_poly = self.to_geo_polygon();
        multipolygon_to_polygons(&geo_poly.union(&geo_poly))
    }

    /// Boolean union with `other`. May return multiple polygons if the
    /// inputs are disjoint, or one polygon per merged region otherwise.
    pub fn union(&self, other: &Polygon2) -> Vec<Polygon2> {
        let self_valid = self.exterior.len() >= 3;
        let other_valid = other.exterior.len() >= 3;
        match (self_valid, other_valid) {
            (false, false) => Vec::new(),
            (true, false) => vec![normalized_clone(self)],
            (false, true) => vec![normalized_clone(other)],
            (true, true) => {
                use geo::BooleanOps;
                let a = self.to_geo_polygon();
                let b = other.to_geo_polygon();
                multipolygon_to_polygons(&a.union(&b))
            }
        }
    }

    /// Boolean intersection with `other`. Empty if the polygons don't overlap.
    pub fn intersection(&self, other: &Polygon2) -> Vec<Polygon2> {
        if self.exterior.len() < 3 || other.exterior.len() < 3 {
            return Vec::new();
        }
        use geo::BooleanOps;
        let a = self.to_geo_polygon();
        let b = other.to_geo_polygon();
        multipolygon_to_polygons(&a.intersection(&b))
    }

    /// Boolean difference: regions of `self` not covered by `other`.
    pub fn difference(&self, other: &Polygon2) -> Vec<Polygon2> {
        if self.exterior.len() < 3 {
            return Vec::new();
        }
        if other.exterior.len() < 3 {
            return vec![normalized_clone(self)];
        }
        use geo::BooleanOps;
        let a = self.to_geo_polygon();
        let b = other.to_geo_polygon();
        multipolygon_to_polygons(&a.difference(&b))
    }

    /// Merge a whole set of polygons via a single efficient union pass
    /// (`geo::unary_union`), rather than folding pairwise unions.
    pub fn union_all(polys: &[Polygon2]) -> Vec<Polygon2> {
        let geo_polys: Vec<geo::Polygon<f64>> = polys
            .iter()
            .filter(|p| p.exterior.len() >= 3)
            .map(Polygon2::to_geo_polygon)
            .collect();
        if geo_polys.is_empty() {
            return Vec::new();
        }
        multipolygon_to_polygons(&geo::unary_union(&geo_polys))
    }
}

/// Offset a polygon by `distance`.
///
/// Uses cavalier_contours for arc-preserving parallel offset.
///
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Sign convention (for CCW exterior, matching cavalier_contours):
/// - `distance > 0` = **inward** (shrink) — used for pocket clearing
/// - `distance < 0` = **outward** (grow) — used for profile offset
///
/// Returns empty Vec if the polygon collapses entirely.
/// May return multiple polygons if the offset splits the shape.
///
/// R1.5 (2026-07-06): self-intersecting input (bowtie/pinched rings —
/// the shape marching-squares output can produce at saddle points) is
/// repaired via `Polygon2::repaired` *before* it ever reaches
/// cavalier_contours, since that's the actual class of degenerate input
/// most likely to hit the panic path below.
pub fn offset_polygon(polygon: &Polygon2, distance: f64) -> Vec<Polygon2> {
    let pieces: Vec<Polygon2> = if polygon.has_self_intersection() {
        polygon.repaired()
    } else {
        vec![polygon.clone()]
    };

    pieces
        .iter()
        .flat_map(|piece| offset_one(piece, distance))
        .collect()
}

/// The single chokepoint where cavalier_contours' offset is called.
fn offset_one(polygon: &Polygon2, distance: f64) -> Vec<Polygon2> {
    // R1 (tech-debt review 2026-06-10): cavalier_contours 0.7.0 asserts
    // on some degenerate offset inputs ("start index should be less
    // than or equal to end index if polyline is open" in
    // `Shape::parallel_offset`'s slice stitching — reproduced live by
    // WANAKA Back Rough's terrain slices: 86-vertex exterior, 13 holes,
    // inward 5.53 mm; asset at test_data/cavalier_panic_polygon_r1.json).
    // This is the single chokepoint where the dependency is called, so
    // contain the panic here: treat an offset that panics as a collapsed
    // offset (empty result). Every caller already handles empty as
    // "polygon collapsed" — pocket rings end, the adaptive machinability
    // probe reports not-machinable — all under-cut directions, never a
    // gouge.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        offset_polygon_inner(polygon, distance)
    })) {
        Ok(v) => v,
        Err(_payload) => {
            tracing::warn!(
                distance,
                exterior_verts = polygon.exterior.len(),
                holes = polygon.holes.len(),
                "offset_polygon: cavalier_contours panicked on degenerate input \
                 despite self-intersection repair already having run; \
                 treating as collapsed offset (empty result)"
            );
            Vec::new()
        }
    }
}

/// cavalier_contours' offset contract requires inputs with no
/// repeat-position vertexes (`debug_assert!` in `pline_offset`; in
/// release the invariant is silently assumed). Repeats appear when
/// chained offsets feed cavalier output back in — `pocket_offsets` at
/// small stepovers (found by the R1 generator-extremes fuzz,
/// pocket@0.05 mm floors). Epsilon matches cavalier's
/// `OffsetOptions::default().pos_equal_eps`.
const PLINE_POS_EQUAL_EPS: f64 = 1e-5;

fn dedupe_pline(pline: Polyline<f64>) -> Polyline<f64> {
    pline
        .remove_repeat_pos(PLINE_POS_EQUAL_EPS)
        .unwrap_or(pline)
}

fn offset_polygon_inner(polygon: &Polygon2, distance: f64) -> Vec<Polygon2> {
    if polygon.exterior.len() < 3 {
        return Vec::new();
    }

    if polygon.holes.is_empty() {
        // Simple case: just offset the exterior
        let pline = dedupe_pline(polygon.exterior_to_pline());
        if pline.vertex_count() < 3 {
            return Vec::new();
        }
        let results = pline.parallel_offset(distance);
        results.iter().map(Polygon2::from_pline).collect()
    } else {
        // Polygon with holes: use Shape to handle hole interaction
        use cavalier_contours::shape_algorithms::Shape;

        let exterior = dedupe_pline(polygon.exterior_to_pline());
        if exterior.vertex_count() < 3 {
            return Vec::new();
        }
        let mut plines = vec![exterior];
        for hole in &polygon.holes {
            let mut hole_pline = Polyline::with_capacity(hole.len(), true);
            for p in hole {
                hole_pline.add(p.x, p.y, 0.0);
            }
            let hole_pline = dedupe_pline(hole_pline);
            // A hole that dedupes below a triangle is pure noise.
            if hole_pline.vertex_count() >= 3 {
                plines.push(hole_pline);
            }
        }

        let shape = Shape::from_plines(plines);
        let result = shape.parallel_offset(distance, Default::default());

        // CCW plines are boundaries, CW are holes.
        // Pair each hole with its containing boundary via containment test.
        let mut polygons: Vec<Polygon2> = result
            .ccw_plines
            .iter()
            .map(|ip| Polygon2::from_pline(&ip.polyline))
            .collect();

        let holes: Vec<Vec<P2>> = result
            .cw_plines
            .iter()
            .map(|ip| {
                ip.polyline
                    .iter_vertexes()
                    .map(|v| P2::new(v.x, v.y))
                    .collect()
            })
            .collect();

        for hole in holes {
            let Some(test_pt) = hole.first() else {
                continue;
            };
            let mut assigned = false;
            for poly in &mut polygons {
                if poly.contains_point(test_pt) {
                    poly.holes.push(hole.clone());
                    assigned = true;
                    break;
                }
            }
            if !assigned && let Some(first_poly) = polygons.first_mut() {
                // Fallback: attach to first polygon (preserves old behavior)
                first_poly.holes.push(hole);
            }
        }

        polygons
    }
}

/// Generate concentric inward offsets for pocket clearing.
///
/// Starting from the boundary, offsets inward by `stepover` repeatedly
/// until the polygon collapses. Returns all offset contours from
/// outermost to innermost.
pub fn pocket_offsets(polygon: &Polygon2, stepover: f64) -> Vec<Vec<Polygon2>> {
    let mut layers = Vec::new();
    let mut current = vec![polygon.clone()];

    loop {
        let mut next_layer = Vec::new();
        for poly in &current {
            next_layer.extend(offset_polygon(poly, stepover));
        }
        if next_layer.is_empty() {
            break;
        }
        layers.push(next_layer.clone());
        current = next_layer;
    }

    layers
}

/// Detect containment among a flat list of polygons and nest inner polygons
/// as holes of their containing polygon.
///
/// Given polygons from an SVG or DXF where separate shapes may represent
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// an outer boundary with interior islands, this function:
/// 1. Sorts polygons by area (largest first)
/// 2. For each smaller polygon, checks if it's fully inside a larger one
/// 3. If so, converts it to a hole of that polygon
///
/// Returns the nested polygons (outer boundaries with holes attached).
pub fn detect_containment(mut polygons: Vec<Polygon2>) -> Vec<Polygon2> {
    if polygons.len() <= 1 {
        return polygons;
    }

    // Ensure all have correct winding before containment test
    for poly in &mut polygons {
        poly.ensure_winding();
    }

    // Sort by area descending (largest = outer boundaries)
    polygons.sort_by(|a, b| {
        b.area()
            .partial_cmp(&a.area())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Track which polygons have been consumed as holes
    let n = polygons.len();
    let mut consumed = vec![false; n];
    // Track holes to add to each polygon
    let mut holes_for: Vec<Vec<usize>> = vec![Vec::new(); n];

    // For each polygon (smallest first), check if it's inside a larger one.
    // Only closed polygons participate — open paths (rivers, traces, engrave
    // curves) must stay top-level or project_curve will close their "hole"
    // rings and carve phantom lines between fragments.
    // Safety: i and j are bounded by n (polygon count). consumed and holes_for
    // are sized to n. All indices are valid by loop construction.
    #[allow(clippy::indexing_slicing)]
    {
        for i in (0..n).rev() {
            if consumed[i] {
                continue;
            }
            if !polygons[i].closed {
                continue;
            }
            // Check against all larger polygons
            for j in 0..i {
                if consumed[j] {
                    continue;
                }
                if !polygons[j].closed {
                    continue;
                }
                if polygon_contains_polygon(&polygons[j], &polygons[i]) {
                    holes_for[j].push(i);
                    consumed[i] = true;
                    break; // Only nest one level deep (innermost containing polygon)
                }
            }
        }
    }

    // Build result: outer polygons with their holes attached
    // Safety: i iterates over 0..n, hole_idx values stored in holes_for[i]
    // are valid polygon indices by construction above.
    #[allow(clippy::indexing_slicing)]
    let result = {
        let mut result = Vec::new();
        for (i, poly) in polygons.iter().enumerate() {
            if consumed[i] {
                continue;
            }
            let mut outer = poly.clone();
            for &hole_idx in &holes_for[i] {
                let mut hole_pts = polygons[hole_idx].exterior.clone();
                // Holes must be CW (reverse if CCW)
                if shoelace_area(&hole_pts) > 0.0 {
                    hole_pts.reverse();
                }
                outer.holes.push(hole_pts);
            }
            result.push(outer);
        }
        result
    };

    result
}

/// Test if all vertices of `inner` are inside `outer`'s exterior boundary.
fn polygon_contains_polygon(outer: &Polygon2, inner: &Polygon2) -> bool {
    // Quick bbox check
    let outer_bb = polygon_bbox(&outer.exterior);
    let inner_bb = polygon_bbox(&inner.exterior);
    if inner_bb.0 < outer_bb.0
        || inner_bb.1 < outer_bb.1
        || inner_bb.2 > outer_bb.2
        || inner_bb.3 > outer_bb.3
    {
        return false;
    }

    // Check that all inner vertices are inside the outer polygon (ray casting)
    inner
        .exterior
        .iter()
        .all(|p| point_in_polygon(p, &outer.exterior))
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Ray-casting point-in-polygon test. THE canonical ray cast for this
/// crate — `pub(crate)` so other modules reuse it instead of keeping their
/// own copy (R1.7, 2026-07-06: `boundary.rs`'s test-only duplicate was
/// deleted in favor of `Polygon2::contains_point`, which wraps this).
pub(crate) fn point_in_polygon(point: &P2, polygon: &[P2]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    // Safety: i is bounded by 0..n, j tracks previous index (also < n).
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        let pi = &polygon[i];
        let pj = &polygon[(i + n - 1) % n];
        if ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
    }
    inside
}

fn polygon_bbox(pts: &[P2]) -> (f64, f64, f64, f64) {
    let mut x_min = f64::INFINITY;
    let mut y_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for p in pts {
        x_min = x_min.min(p.x);
        y_min = y_min.min(p.y);
        x_max = x_max.max(p.x);
        y_max = y_max.max(p.y);
    }
    (x_min, y_min, x_max, y_max)
}

// --- helpers ---

/// Pick the polygon with the largest `area()` out of a slice, NaN-safe.
///
/// Ties (equal area) resolve to the *last* maximal element — the same
/// behavior `Iterator::max_by` gives for equal keys. Returns `None` for
/// an empty slice.
pub fn largest_by_area(polys: &[Polygon2]) -> Option<&Polygon2> {
    polys.iter().max_by(|a, b| {
        a.area()
            .partial_cmp(&b.area())
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn shoelace_area(pts: &[P2]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    // Safety: i is bounded by 0..n, j = (i+1)%n is also < n.
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i].x * pts[j].y;
        area -= pts[j].x * pts[i].y;
    }
    area / 2.0
}

fn ring_to_geo(pts: &[P2]) -> geo::LineString<f64> {
    let mut coords: Vec<geo::Coord<f64>> =
        pts.iter().map(|p| geo::Coord { x: p.x, y: p.y }).collect();
    // geo requires closing vertex
    if let Some(&first) = pts.first() {
        coords.push(geo::Coord {
            x: first.x,
            y: first.y,
        });
    }
    geo::LineString::new(coords)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn ring_from_geo(ring: &geo::LineString<f64>) -> Vec<P2> {
    let mut pts: Vec<P2> = ring.coords().map(|c| P2::new(c.x, c.y)).collect();
    // Strip duplicated closing vertex
    if let (Some(&first), Some(&last)) = (pts.first(), pts.last())
        && pts.len() >= 2
        && (first.x - last.x).abs() < 1e-10
        && (first.y - last.y).abs() < 1e-10
    {
        pts.pop();
    }
    pts
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn ring_perimeter(pts: &[P2]) -> f64 {
    let n = pts.len();
    if n < 2 {
        return 0.0;
    }
    let mut perim = 0.0;
    // Safety: i is bounded by 0..n, j = (i+1)%n is also < n.
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        let j = (i + 1) % n;
        perim += (pts[j] - pts[i]).norm();
    }
    perim
}

// --- boolean-op helpers ---

/// Convert a `geo::MultiPolygon` (boolean-op output) back to `Polygon2`s,
/// with holes populated and winding normalized to this crate's convention.
fn multipolygon_to_polygons(mp: &geo::MultiPolygon<f64>) -> Vec<Polygon2> {
    mp.iter()
        .map(|p| {
            let mut poly = Polygon2::from_geo_polygon(p);
            poly.ensure_winding();
            poly
        })
        .collect()
}

/// Owned, winding-normalized copy — used when a boolean op degenerates to
/// "return one of the operands unchanged" (e.g. union/difference against
/// an empty/degenerate polygon).
fn normalized_clone(p: &Polygon2) -> Polygon2 {
    let mut c = p.clone();
    c.ensure_winding();
    c.closed = true;
    c
}

// --- self-intersection detection & repair helpers ---

/// Epsilon for the orientation-sign / collinearity tests in
/// `segments_intersect`. Loose relative to `PLINE_POS_EQUAL_EPS` since
/// this operates on raw doubles (areas of triangles), not offset positions.
const ORIENT_EPS: f64 = 1e-9;

/// Signed area of the triangle (a, b, c); sign gives orientation.
fn orient(a: &P2, b: &P2, c: &P2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// True if `p` (already known collinear with segment `a`-`b`) lies within
/// the segment's bounding box.
fn on_segment(a: &P2, b: &P2, p: &P2) -> bool {
    p.x <= a.x.max(b.x) + ORIENT_EPS
        && p.x >= a.x.min(b.x) - ORIENT_EPS
        && p.y <= a.y.max(b.y) + ORIENT_EPS
        && p.y >= a.y.min(b.y) - ORIENT_EPS
}

/// True if segment bounding boxes overlap — cheap prefilter before the
/// full intersection test.
fn segment_bboxes_overlap(a1: &P2, a2: &P2, b1: &P2, b2: &P2) -> bool {
    let (a_min_x, a_max_x) = (a1.x.min(a2.x), a1.x.max(a2.x));
    let (a_min_y, a_max_y) = (a1.y.min(a2.y), a1.y.max(a2.y));
    let (b_min_x, b_max_x) = (b1.x.min(b2.x), b1.x.max(b2.x));
    let (b_min_y, b_max_y) = (b1.y.min(b2.y), b1.y.max(b2.y));
    a_min_x <= b_max_x && a_max_x >= b_min_x && a_min_y <= b_max_y && a_max_y >= b_min_y
}

/// General segment-segment intersection test — proper crossing or mere
/// touching (shared point, collinear overlap). Adjacency filtering (two
/// segments sharing a ring vertex) is the caller's job; this only answers
/// "do these two segments meet anywhere".
fn segments_intersect(p1: &P2, p2: &P2, p3: &P2, p4: &P2) -> bool {
    let d1 = orient(p3, p4, p1);
    let d2 = orient(p3, p4, p2);
    let d3 = orient(p1, p2, p3);
    let d4 = orient(p1, p2, p4);

    if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) {
        return true;
    }
    // Collinear/touching cases (covers a repeated non-adjacent vertex,
    // i.e. a bowtie pinch, and partial collinear overlap).
    (d1.abs() < ORIENT_EPS && on_segment(p3, p4, p1))
        || (d2.abs() < ORIENT_EPS && on_segment(p3, p4, p2))
        || (d3.abs() < ORIENT_EPS && on_segment(p1, p2, p3))
        || (d4.abs() < ORIENT_EPS && on_segment(p1, p2, p4))
}

/// True if the ring (implicitly closed) crosses itself. Rings here are
/// marching-squares/import output, typically well under 500 vertices, so
/// the O(n^2) segment-pair scan (with a bbox prefilter) is cheap enough.
fn ring_has_self_intersection(ring: &[P2]) -> bool {
    let n = ring.len();
    if n < 4 {
        // A triangle (or fewer vertices) cannot self-intersect: every
        // segment pair is adjacent.
        return false;
    }
    // Safety: i, j, and their (+1)%n successors are all bounded by 0..n.
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        let a1 = &ring[i];
        let a2 = &ring[(i + 1) % n];
        for j in (i + 1)..n {
            let adjacent = j == i + 1 || (i == 0 && j == n - 1);
            if adjacent {
                continue;
            }
            let b1 = &ring[j];
            let b2 = &ring[(j + 1) % n];
            if segment_bboxes_overlap(a1, a2, b1, b2) && segments_intersect(a1, a2, b1, b2) {
                return true;
            }
        }
    }
    false
}

/// Squared point-to-segment distance.
fn point_segment_distance_sq(p: &P2, a: &P2, b: &P2) -> f64 {
    let ab = *b - *a;
    let len_sq = ab.norm_squared();
    if len_sq < f64::EPSILON {
        return (*p - *a).norm_squared();
    }
    let t = ((*p - *a).dot(&ab) / len_sq).clamp(0.0, 1.0);
    let closest = *a + ab * t;
    (*p - closest).norm_squared()
}

/// True if `p` lies within `eps` of any edge of the ring (implicitly closed).
fn ring_near_point(ring: &[P2], p: &P2, eps: f64) -> bool {
    let n = ring.len();
    if n < 2 {
        return false;
    }
    let eps_sq = eps * eps;
    // Safety: i and its (+1)%n successor are bounded by 0..n.
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        let a = &ring[i];
        let b = &ring[(i + 1) % n];
        if point_segment_distance_sq(p, a, b) <= eps_sq {
            return true;
        }
    }
    false
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn square(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    #[test]
    fn test_rectangle_area() {
        let rect = Polygon2::rectangle(0.0, 0.0, 10.0, 5.0);
        assert_relative_eq!(rect.area(), 50.0, epsilon = 1e-10);
    }

    #[test]
    fn test_signed_area_ccw() {
        let sq = square(10.0);
        assert!(
            sq.signed_area() > 0.0,
            "CCW square should have positive area"
        );
        assert_relative_eq!(sq.area(), 100.0, epsilon = 1e-10);
    }

    #[test]
    fn test_signed_area_cw() {
        let pts = vec![
            P2::new(-5.0, -5.0),
            P2::new(-5.0, 5.0),
            P2::new(5.0, 5.0),
            P2::new(5.0, -5.0),
        ];
        // CW winding
        let area = shoelace_area(&pts);
        assert!(area < 0.0, "CW should have negative area");

        let mut poly = Polygon2::new(pts);
        poly.ensure_winding();
        assert!(
            poly.signed_area() > 0.0,
            "After ensure_winding, should be CCW"
        );
    }

    #[test]
    fn test_polygon_with_hole() {
        let outer = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        // CW hole (5x5 centered at 10,10)
        let hole = vec![
            P2::new(7.5, 7.5),
            P2::new(7.5, 12.5),
            P2::new(12.5, 12.5),
            P2::new(12.5, 7.5),
        ];
        let poly = Polygon2::with_holes(outer.exterior, vec![hole]);
        assert_relative_eq!(poly.area(), 400.0 - 25.0, epsilon = 1e-10);
    }

    #[test]
    fn test_perimeter() {
        let sq = square(10.0);
        assert_relative_eq!(sq.perimeter(), 40.0, epsilon = 1e-10);
    }

    #[test]
    fn test_ensure_winding() {
        // Create CW polygon
        let mut poly = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(0.0, 10.0),
            P2::new(10.0, 10.0),
            P2::new(10.0, 0.0),
        ]);
        assert!(poly.signed_area() < 0.0);
        assert!(!poly.has_correct_winding());

        poly.ensure_winding();
        assert!(poly.signed_area() > 0.0);
        assert!(poly.has_correct_winding());
    }

    // --- geo conversion tests ---

    #[test]
    fn test_geo_roundtrip() {
        let original = Polygon2::rectangle(1.0, 2.0, 11.0, 7.0);
        let geo_poly = original.to_geo_polygon();
        let recovered = Polygon2::from_geo_polygon(&geo_poly);

        assert_eq!(recovered.exterior.len(), original.exterior.len());
        assert_relative_eq!(recovered.area(), original.area(), epsilon = 1e-10);
    }

    #[test]
    fn test_geo_roundtrip_with_holes() {
        let hole = vec![
            P2::new(3.0, 3.0),
            P2::new(3.0, 5.0),
            P2::new(5.0, 5.0),
            P2::new(5.0, 3.0),
        ];
        let original = Polygon2::with_holes(
            Polygon2::rectangle(0.0, 0.0, 10.0, 10.0).exterior,
            vec![hole],
        );
        let geo_poly = original.to_geo_polygon();
        let recovered = Polygon2::from_geo_polygon(&geo_poly);

        assert_eq!(recovered.holes.len(), 1);
        assert_relative_eq!(recovered.area(), original.area(), epsilon = 1e-10);
    }

    // --- pline conversion tests ---

    #[test]
    fn test_pline_roundtrip() {
        let original = square(20.0);
        let pline = original.exterior_to_pline();
        assert_eq!(pline.vertex_count(), 4);
        assert!(pline.is_closed());

        let recovered = Polygon2::from_pline(&pline);
        assert_eq!(recovered.exterior.len(), 4);
        assert_relative_eq!(recovered.area(), original.area(), epsilon = 1e-10);
    }

    // --- offset tests ---

    #[test]
    fn test_offset_inward_square() {
        let sq = square(20.0); // 20x20 centered at origin
        let results = offset_polygon(&sq, 2.0); // inward by 2

        assert_eq!(
            results.len(),
            1,
            "Single inward offset should produce one polygon"
        );
        let inner = &results[0];

        // Area should be approximately (20 - 2*2)^2 = 256
        // cavalier_contours uses round joins, so corners are rounded.
        // Area will be slightly larger than a pure rectangle but close.
        let expected_rect_area = 16.0 * 16.0; // 256
        assert!(
            inner.area() > expected_rect_area * 0.95,
            "Inner area {} should be close to {} (rounded corners make it slightly larger)",
            inner.area(),
            expected_rect_area
        );
        assert!(inner.area() < sq.area());
    }

    #[test]
    fn test_offset_collapse() {
        let sq = square(10.0); // 10x10
        let results = offset_polygon(&sq, 6.0); // inward by 6, exceeds half-width

        assert!(
            results.is_empty(),
            "Offset exceeding half-width should collapse: got {} polygons",
            results.len()
        );
    }

    #[test]
    fn test_offset_outward_square() {
        let sq = square(10.0);
        let results = offset_polygon(&sq, -2.0); // outward by 2

        assert_eq!(results.len(), 1);
        assert!(
            results[0].area() > sq.area(),
            "Outward offset should increase area"
        );
    }

    #[test]
    fn test_offset_non_convex_l_shape() {
        // L-shaped polygon (concave)
        let l_shape = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(20.0, 0.0),
            P2::new(20.0, 10.0),
            P2::new(10.0, 10.0),
            P2::new(10.0, 20.0),
            P2::new(0.0, 20.0),
        ]);
        assert_relative_eq!(l_shape.area(), 300.0, epsilon = 1e-10);

        // Small inward offset should produce one polygon
        let results = offset_polygon(&l_shape, 1.0);
        assert!(
            !results.is_empty(),
            "Small offset of L-shape should not collapse"
        );
        let total_area: f64 = results.iter().map(|p| p.area()).sum();
        assert!(total_area < l_shape.area());
        assert!(
            total_area > 200.0,
            "Area {} too small for 1mm inward offset",
            total_area
        );
    }

    #[test]
    fn test_offset_polygon_with_hole() {
        // 30x30 square with 10x10 hole in center
        let hole = vec![
            P2::new(10.0, 10.0),
            P2::new(10.0, 20.0),
            P2::new(20.0, 20.0),
            P2::new(20.0, 10.0),
        ]; // CW
        let poly = Polygon2::with_holes(
            Polygon2::rectangle(0.0, 0.0, 30.0, 30.0).exterior,
            vec![hole],
        );
        assert_relative_eq!(poly.area(), 900.0 - 100.0, epsilon = 1e-10);

        // Inward offset by 2mm should shrink exterior and grow hole
        let results = offset_polygon(&poly, 2.0);
        assert!(
            !results.is_empty(),
            "Offset of polygon-with-hole should not collapse"
        );
        let total_area: f64 = results.iter().map(|p| p.area()).sum();
        assert!(
            total_area < poly.area(),
            "Offset area {} should be less than original {}",
            total_area,
            poly.area()
        );
    }

    #[test]
    fn test_offset_preserves_center() {
        // Symmetric square centered at origin - offset should stay centered
        let sq = square(20.0);
        let results = offset_polygon(&sq, 3.0);
        assert_eq!(results.len(), 1);

        let inner = &results[0];
        // Centroid of offset result should be near origin
        let cx: f64 = inner.exterior.iter().map(|p| p.x).sum::<f64>() / inner.exterior.len() as f64;
        let cy: f64 = inner.exterior.iter().map(|p| p.y).sum::<f64>() / inner.exterior.len() as f64;
        assert!(cx.abs() < 0.5, "Centroid x={} should be near 0", cx);
        assert!(cy.abs() < 0.5, "Centroid y={} should be near 0", cy);
    }

    #[test]
    fn test_offset_zero_distance() {
        let sq = square(10.0);
        let results = offset_polygon(&sq, 0.0);
        assert_eq!(results.len(), 1);
        assert_relative_eq!(results[0].area(), sq.area(), epsilon = 0.1);
    }

    #[test]
    fn test_pocket_offsets() {
        let sq = square(20.0); // 20x20
        let layers = pocket_offsets(&sq, 3.0); // stepover = 3mm

        // With 20x20 square and 3mm stepover, we should get ~3 layers
        // (half-width = 10, so 10/3 ≈ 3.33 layers)
        assert!(
            layers.len() >= 2 && layers.len() <= 4,
            "Expected 2-4 layers for 20x20 square with 3mm stepover, got {}",
            layers.len()
        );

        // Each layer should have smaller area than the previous
        let mut prev_area: f64 = sq.area();
        for (i, layer) in layers.iter().enumerate() {
            let layer_area: f64 = layer.iter().map(|p| p.area()).sum();
            assert!(
                layer_area < prev_area,
                "Layer {} area ({}) should be less than previous ({})",
                i,
                layer_area,
                prev_area
            );
            prev_area = layer_area;
        }
    }

    // --- containment detection tests ---

    #[test]
    fn test_containment_rect_with_hole() {
        let outer = Polygon2::rectangle(0.0, 0.0, 50.0, 50.0);
        let inner = Polygon2::rectangle(15.0, 15.0, 35.0, 35.0);

        let result = detect_containment(vec![outer, inner]);
        assert_eq!(
            result.len(),
            1,
            "Inner should become a hole, not a separate polygon"
        );
        assert_eq!(result[0].holes.len(), 1, "Outer should have 1 hole");
        assert_relative_eq!(result[0].area(), 50.0 * 50.0 - 20.0 * 20.0, epsilon = 1.0);
    }

    #[test]
    fn test_containment_no_nesting() {
        let a = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let b = Polygon2::rectangle(30.0, 0.0, 50.0, 20.0);

        let result = detect_containment(vec![a, b]);
        assert_eq!(result.len(), 2, "Separate polygons should stay separate");
        assert!(result[0].holes.is_empty());
        assert!(result[1].holes.is_empty());
    }

    #[test]
    fn test_containment_multiple_holes() {
        let outer = Polygon2::rectangle(0.0, 0.0, 100.0, 100.0);
        let hole1 = Polygon2::rectangle(10.0, 10.0, 30.0, 30.0);
        let hole2 = Polygon2::rectangle(50.0, 50.0, 70.0, 70.0);

        let result = detect_containment(vec![hole1, outer, hole2]);
        assert_eq!(result.len(), 1, "Both inner rects should become holes");
        assert_eq!(result[0].holes.len(), 2);
    }

    #[test]
    fn test_containment_preserves_winding() {
        let outer = Polygon2::rectangle(0.0, 0.0, 50.0, 50.0);
        let inner = Polygon2::rectangle(10.0, 10.0, 40.0, 40.0);

        let result = detect_containment(vec![outer, inner]);
        assert!(result[0].signed_area() > 0.0, "Outer should be CCW");
        // Holes should be CW (negative area)
        let hole_area = shoelace_area(&result[0].holes[0]);
        assert!(hole_area < 0.0, "Hole should be CW, got area {}", hole_area);
    }

    #[test]
    fn test_containment_single_polygon() {
        let poly = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let result = detect_containment(vec![poly]);
        assert_eq!(result.len(), 1);
        assert!(result[0].holes.is_empty());
    }

    #[test]
    fn test_offset_two_separate_regions() {
        // Two separate rectangles, each with a hole. Offset inward.
        // Verify each polygon keeps its own hole after re-pairing.
        let rect1 = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let hole1 = vec![
            P2::new(10.0, 10.0),
            P2::new(10.0, 20.0),
            P2::new(20.0, 20.0),
            P2::new(20.0, 10.0),
        ]; // CW
        let rect2 = Polygon2::rectangle(50.0, 0.0, 80.0, 30.0);
        let hole2 = vec![
            P2::new(60.0, 10.0),
            P2::new(60.0, 20.0),
            P2::new(70.0, 20.0),
            P2::new(70.0, 10.0),
        ]; // CW

        let poly1 = Polygon2::with_holes(rect1.exterior, vec![hole1]);
        let poly2 = Polygon2::with_holes(rect2.exterior, vec![hole2]);

        // Offset each separately — both should retain their holes
        let r1 = offset_polygon(&poly1, 1.0);
        let r2 = offset_polygon(&poly2, 1.0);
        assert!(!r1.is_empty(), "First offset should succeed");
        assert!(!r2.is_empty(), "Second offset should succeed");

        // Each result should have holes
        let r1_holes: usize = r1.iter().map(|p| p.holes.len()).sum();
        let r2_holes: usize = r2.iter().map(|p| p.holes.len()).sum();
        assert!(
            r1_holes >= 1,
            "First polygon should keep its hole, got {} holes",
            r1_holes
        );
        assert!(
            r2_holes >= 1,
            "Second polygon should keep its hole, got {} holes",
            r2_holes
        );
    }

    #[test]
    fn test_offset_single_region_two_holes() {
        // One rect with two holes — both should stay attached after offset.
        let hole1 = vec![
            P2::new(5.0, 5.0),
            P2::new(5.0, 10.0),
            P2::new(10.0, 10.0),
            P2::new(10.0, 5.0),
        ]; // CW
        let hole2 = vec![
            P2::new(20.0, 5.0),
            P2::new(20.0, 10.0),
            P2::new(25.0, 10.0),
            P2::new(25.0, 5.0),
        ]; // CW
        let poly = Polygon2::with_holes(
            Polygon2::rectangle(0.0, 0.0, 30.0, 15.0).exterior,
            vec![hole1, hole2],
        );

        let results = offset_polygon(&poly, 1.0);
        assert!(!results.is_empty(), "Offset should succeed");

        let total_holes: usize = results.iter().map(|p| p.holes.len()).sum();
        assert!(
            total_holes >= 2,
            "Both holes should survive offset, got {} holes",
            total_holes
        );
    }

    #[test]
    fn test_point_in_polygon_basic() {
        let square = vec![
            P2::new(0.0, 0.0),
            P2::new(10.0, 0.0),
            P2::new(10.0, 10.0),
            P2::new(0.0, 10.0),
        ];
        assert!(point_in_polygon(&P2::new(5.0, 5.0), &square));
        assert!(!point_in_polygon(&P2::new(15.0, 5.0), &square));
        assert!(!point_in_polygon(&P2::new(-1.0, 5.0), &square));
    }

    #[test]
    fn test_largest_by_area_empty() {
        let polys: Vec<Polygon2> = Vec::new();
        assert!(largest_by_area(&polys).is_none());
    }

    #[test]
    fn test_largest_by_area_single() {
        let polys = vec![Polygon2::rectangle(0.0, 0.0, 10.0, 10.0)];
        let picked = largest_by_area(&polys).expect("single element is Some");
        assert_relative_eq!(picked.area(), 100.0);
    }

    #[test]
    fn test_largest_by_area_picks_largest() {
        let small = Polygon2::rectangle(0.0, 0.0, 5.0, 5.0);
        let large = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let mid = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let polys = vec![small, large, mid];
        let picked = largest_by_area(&polys).expect("non-empty slice is Some");
        assert_relative_eq!(picked.area(), 400.0);
    }

    #[test]
    fn test_largest_by_area_tie_picks_last() {
        // Documents tie behavior: equal-area candidates resolve to the
        // *last* one in the slice (matches `Iterator::max_by` semantics).
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(100.0, 100.0, 110.0, 110.0); // same area, different position
        let polys = vec![a, b];
        let picked = largest_by_area(&polys).expect("non-empty slice is Some");
        assert_relative_eq!(picked.exterior[0].x, 100.0);
    }

    // --- self-intersection detection & repair (R1.5) ---

    #[test]
    fn test_bowtie_self_intersection() {
        // Figure-eight / bowtie: edges (0,0)-(10,10) and (10,0)-(0,10) cross
        // at the center; the other two edges don't.
        let bowtie = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(10.0, 10.0),
            P2::new(10.0, 0.0),
            P2::new(0.0, 10.0),
        ]);
        assert!(bowtie.has_self_intersection());

        let repaired = bowtie.repaired();
        assert!(
            !repaired.is_empty(),
            "repair should produce at least one simple polygon"
        );
        for piece in &repaired {
            assert!(
                !piece.has_self_intersection(),
                "repaired piece should be simple"
            );
        }
    }

    #[test]
    fn test_simple_square_no_self_intersection() {
        let sq = square(10.0);
        assert!(!sq.has_self_intersection());

        let repaired = sq.repaired();
        assert_eq!(
            repaired.len(),
            1,
            "non-intersecting input passes through unchanged"
        );
        assert_relative_eq!(repaired[0].area(), sq.area(), epsilon = 1e-9);
    }

    #[test]
    fn test_offset_polygon_r1_panic_repro_does_not_panic() {
        // The captured WANAKA terrain-slice input that used to panic
        // cavalier_contours (see offset_polygon's R1 doc comment) is
        // already exercised end-to-end by the integration test
        // `tests/offset_polygon_degenerate_inputs_r1.rs` (loads the same
        // asset via `common::repo_root`). This unit-level copy just
        // confirms the asset still parses from inside the crate's own
        // test harness and the offset call — now routed through the
        // R1.5 self-intersection repair first — still returns without
        // panicking.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../test_data/cavalier_panic_polygon_r1.json");
        let Ok(raw) = std::fs::read_to_string(&path) else {
            // Asset not present in this checkout — nothing to assert.
            return;
        };
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse capture");
        let distance = v["distance"].as_f64().expect("distance");
        let ring = |val: &serde_json::Value| -> Vec<P2> {
            val.as_array()
                .expect("ring array")
                .iter()
                .map(|p| P2::new(p[0].as_f64().expect("x"), p[1].as_f64().expect("y")))
                .collect()
        };
        let mut poly = Polygon2::new(ring(&v["exterior"]));
        poly.closed = v["closed"].as_bool().unwrap_or(true);
        poly.holes = v["holes"]
            .as_array()
            .expect("holes array")
            .iter()
            .map(ring)
            .collect();

        let result = offset_polygon(&poly, distance);
        // The bar is "no panic" — result may legitimately be empty.
        tracing::info!(count = result.len(), "R1 panic-repro offset result");
    }

    // --- boolean ops (R1.6) ---

    #[test]
    fn test_union_overlapping_squares() {
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(5.0, 5.0, 15.0, 15.0);
        let result = a.union(&b);
        assert_eq!(
            result.len(),
            1,
            "overlapping squares should merge into one polygon"
        );
        assert_relative_eq!(result[0].area(), 175.0, epsilon = 1e-9);
        assert!(result[0].has_correct_winding());
    }

    #[test]
    fn test_intersection_overlapping_squares() {
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(5.0, 5.0, 15.0, 15.0);
        let result = a.intersection(&b);
        assert_eq!(result.len(), 1);
        assert_relative_eq!(result[0].area(), 25.0, epsilon = 1e-9);
        assert!(result[0].has_correct_winding());
    }

    #[test]
    fn test_difference_overlapping_squares_l_shape() {
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(5.0, 5.0, 15.0, 15.0);
        let result = a.difference(&b);
        assert_eq!(result.len(), 1);
        assert_relative_eq!(result[0].area(), 75.0, epsilon = 1e-9);
        assert!(result[0].has_correct_winding());
    }

    #[test]
    fn test_union_disjoint_squares() {
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(20.0, 20.0, 30.0, 30.0);
        let result = a.union(&b);
        assert_eq!(result.len(), 2, "disjoint squares stay separate polygons");
        for poly in &result {
            assert!(poly.has_correct_winding());
        }
    }

    #[test]
    fn test_difference_produces_hole() {
        let outer = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let inner = Polygon2::rectangle(5.0, 5.0, 15.0, 15.0);
        let result = outer.difference(&inner);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].holes.len(), 1);
        assert_relative_eq!(result[0].area(), 400.0 - 100.0, epsilon = 1e-9);
        assert!(result[0].has_correct_winding());
    }

    #[test]
    fn test_union_all_merges_set() {
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(5.0, 5.0, 15.0, 15.0); // overlaps a
        let c = Polygon2::rectangle(30.0, 30.0, 40.0, 40.0); // disjoint
        let result = Polygon2::union_all(&[a, b, c]);
        assert_eq!(result.len(), 2, "a+b merge, c stays separate");
        let total_area: f64 = result.iter().map(Polygon2::area).sum();
        assert_relative_eq!(total_area, 175.0 + 100.0, epsilon = 1e-9);
        for poly in &result {
            assert!(poly.has_correct_winding());
        }
    }

    #[test]
    fn test_union_all_empty() {
        assert!(Polygon2::union_all(&[]).is_empty());
    }

    // --- contains_point_eps (R1.7) ---

    #[test]
    fn test_contains_point_eps_on_edge() {
        let sq = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let eps = 1e-6;
        assert!(sq.contains_point_eps(&P2::new(10.0, 5.0), eps));
    }

    #[test]
    fn test_contains_point_eps_just_outside() {
        let sq = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let eps = 1e-6;
        assert!(sq.contains_point_eps(&P2::new(10.0 + eps / 2.0, 5.0), eps));
    }

    #[test]
    fn test_contains_point_eps_far_outside() {
        let sq = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let eps = 1e-6;
        assert!(!sq.contains_point_eps(&P2::new(10.0 + eps * 10.0, 5.0), eps));
    }

    #[test]
    fn test_contains_point_eps_inside_hole_near_edge() {
        let hole = vec![
            P2::new(-3.0, -3.0),
            P2::new(-3.0, 3.0),
            P2::new(3.0, 3.0),
            P2::new(3.0, -3.0),
        ]; // CW
        let poly = Polygon2::with_holes(
            Polygon2::rectangle(-10.0, -10.0, 10.0, 10.0).exterior,
            vec![hole],
        );
        let eps = 1e-6;
        // Strictly inside the hole (excluded by strict containment) but
        // within eps of the hole's edge.
        let p = P2::new(3.0 - eps / 2.0, 0.0);
        assert!(
            !poly.contains_point(&p),
            "point should be excluded by strict containment"
        );
        assert!(poly.contains_point_eps(&p, eps));
    }
}
