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
/// Vertices are not duplicated (last implicitly connects back to first).
#[derive(Debug, Clone)]
pub struct Polygon2 {
    pub exterior: Vec<P2>,
    pub holes: Vec<Vec<P2>>,
}

impl Polygon2 {
    /// Create a polygon from exterior vertices (CCW winding assumed).
    pub fn new(exterior: Vec<P2>) -> Self {
        Self {
            exterior,
            holes: Vec::new(),
        }
    }

    /// Create a polygon with holes.
    pub fn with_holes(exterior: Vec<P2>, holes: Vec<Vec<P2>>) -> Self {
        Self { exterior, holes }
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
        Self { exterior, holes }
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
}

/// Offset a polygon by `distance`.
///
/// Uses cavalier_contours for arc-preserving parallel offset.
///
/// Sign convention (for CCW exterior, matching cavalier_contours):
/// - `distance > 0` = **inward** (shrink) — used for pocket clearing
/// - `distance < 0` = **outward** (grow) — used for profile offset
///
/// Returns empty Vec if the polygon collapses entirely.
/// May return multiple polygons if the offset splits the shape.
pub fn offset_polygon(polygon: &Polygon2, distance: f64) -> Vec<Polygon2> {
    if polygon.exterior.len() < 3 {
        return Vec::new();
    }

    if polygon.holes.is_empty() {
        // Simple case: just offset the exterior
        let pline = polygon.exterior_to_pline();
        let results = pline.parallel_offset(distance);
        results.iter().map(Polygon2::from_pline).collect()
    } else {
        // Polygon with holes: use Shape to handle hole interaction
        use cavalier_contours::shape_algorithms::Shape;

        let mut plines = vec![polygon.exterior_to_pline()];
        for hole in &polygon.holes {
            let mut hole_pline = Polyline::with_capacity(hole.len(), true);
            for p in hole {
                hole_pline.add(p.x, p.y, 0.0);
            }
            plines.push(hole_pline);
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
            if hole.is_empty() {
                continue;
            }
            let test_pt = &hole[0];
            let mut assigned = false;
            for poly in &mut polygons {
                if poly.contains_point(test_pt) {
                    poly.holes.push(hole.clone());
                    assigned = true;
                    break;
                }
            }
            if !assigned && !polygons.is_empty() {
                // Fallback: attach to first polygon (preserves old behavior)
                polygons[0].holes.push(hole);
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
    let mut consumed = vec![false; polygons.len()];
    // Track holes to add to each polygon
    let mut holes_for: Vec<Vec<usize>> = vec![Vec::new(); polygons.len()];

    // For each polygon (smallest first), check if it's inside a larger one
    for i in (0..polygons.len()).rev() {
        if consumed[i] {
            continue;
        }
        // Check against all larger polygons
        for j in 0..i {
            if consumed[j] {
                continue;
            }
            if polygon_contains_polygon(&polygons[j], &polygons[i]) {
                holes_for[j].push(i);
                consumed[i] = true;
                break; // Only nest one level deep (innermost containing polygon)
            }
        }
    }

    // Build result: outer polygons with their holes attached
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

/// Ray-casting point-in-polygon test.
fn point_in_polygon(point: &P2, polygon: &[P2]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let pi = &polygon[i];
        let pj = &polygon[j];
        if ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
        j = i;
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

fn shoelace_area(pts: &[P2]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
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

fn ring_from_geo(ring: &geo::LineString<f64>) -> Vec<P2> {
    let mut pts: Vec<P2> = ring.coords().map(|c| P2::new(c.x, c.y)).collect();
    // Strip duplicated closing vertex
    if pts.len() >= 2 {
        let first = pts[0];
        let last = pts[pts.len() - 1];
        if (first.x - last.x).abs() < 1e-10 && (first.y - last.y).abs() < 1e-10 {
            pts.pop();
        }
    }
    pts
}

fn ring_perimeter(pts: &[P2]) -> f64 {
    let n = pts.len();
    if n < 2 {
        return 0.0;
    }
    let mut perim = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        perim += (pts[j] - pts[i]).norm();
    }
    perim
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
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
        let poly = Polygon2::with_holes(outer.exterior.clone(), vec![hole]);
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
        let result = detect_containment(vec![poly.clone()]);
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
}
