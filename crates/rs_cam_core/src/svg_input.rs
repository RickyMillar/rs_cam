//! SVG file input — extracts closed paths as Polygon2.
//!
//! Uses usvg to parse and simplify SVG into basic path segments,
//! then flattens bezier curves to polylines within a configurable tolerance.

use crate::geo::P2;
use crate::polygon::Polygon2;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SvgError {
    #[error("Failed to read SVG file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse SVG: {0}")]
    Parse(#[from] usvg::Error),
    #[error("No closed paths found in SVG")]
    NoPaths,
}

/// Load closed polygon paths from an SVG file.
///
/// Bezier curves are flattened to line segments with the given tolerance (mm).
/// Only closed subpaths are returned. Open paths are ignored.
pub fn load_svg(path: &Path, tolerance: f64) -> Result<Vec<Polygon2>, SvgError> {
    let data = std::fs::read(path)?;
    load_svg_data(&data, tolerance)
}

/// Load closed polygon paths from SVG data bytes.
pub fn load_svg_data(data: &[u8], tolerance: f64) -> Result<Vec<Polygon2>, SvgError> {
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(data, &opt)?;
    let mut polygons = Vec::new();
    visit_group(tree.root(), tolerance, &mut polygons);
    // Detect containment: inner shapes become holes of outer shapes
    let polygons = crate::polygon::detect_containment(polygons);
    Ok(polygons)
}

fn visit_group(group: &usvg::Group, tolerance: f64, out: &mut Vec<Polygon2>) {
    for node in group.children() {
        match node {
            usvg::Node::Path(path) => {
                extract_polygons_from_path(path.data(), tolerance, out);
            }
            usvg::Node::Group(group) => {
                visit_group(group, tolerance, out);
            }
            _ => {}
        }
    }
}

fn extract_polygons_from_path(
    path_data: &usvg::tiny_skia_path::Path,
    tolerance: f64,
    out: &mut Vec<Polygon2>,
) {
    use usvg::tiny_skia_path::PathSegment;

    let mut current_subpath: Vec<P2> = Vec::new();
    let mut subpath_start = P2::new(0.0, 0.0);
    let mut last = P2::new(0.0, 0.0);

    for seg in path_data.segments() {
        match seg {
            PathSegment::MoveTo(pt) => {
                // Start new subpath (discard unclosed previous)
                current_subpath.clear();
                let p = P2::new(pt.x as f64, pt.y as f64);
                current_subpath.push(p);
                subpath_start = p;
                last = p;
            }
            PathSegment::LineTo(pt) => {
                let p = P2::new(pt.x as f64, pt.y as f64);
                current_subpath.push(p);
                last = p;
            }
            PathSegment::QuadTo(ctrl, end) => {
                let c = P2::new(ctrl.x as f64, ctrl.y as f64);
                let e = P2::new(end.x as f64, end.y as f64);
                flatten_quad(last, c, e, tolerance, &mut current_subpath);
                last = e;
            }
            PathSegment::CubicTo(c1, c2, end) => {
                let cp1 = P2::new(c1.x as f64, c1.y as f64);
                let cp2 = P2::new(c2.x as f64, c2.y as f64);
                let e = P2::new(end.x as f64, end.y as f64);
                flatten_cubic(last, cp1, cp2, e, tolerance, &mut current_subpath);
                last = e;
            }
            PathSegment::Close => {
                // Emit closed subpath as polygon if it has enough points
                if current_subpath.len() >= 3 {
                    // Remove closing vertex if it duplicates start
                    let mut pts = current_subpath.clone();
                    if pts.len() >= 2 {
                        let first = pts[0];
                        let last_pt = pts[pts.len() - 1];
                        if (first.x - last_pt.x).abs() < 1e-6
                            && (first.y - last_pt.y).abs() < 1e-6
                        {
                            pts.pop();
                        }
                    }
                    if pts.len() >= 3 {
                        let mut poly = Polygon2::new(pts);
                        poly.ensure_winding();
                        out.push(poly);
                    }
                }
                current_subpath.clear();
                last = subpath_start;
            }
        }
    }
}

/// Flatten a quadratic bezier curve to line segments.
fn flatten_quad(p0: P2, p1: P2, p2: P2, tolerance: f64, out: &mut Vec<P2>) {
    // Flatness test: max distance of control point from the chord
    let dx = p2.x - p0.x;
    let dy = p2.y - p0.y;
    let d = ((p1.x - p2.x) * dy - (p1.y - p2.y) * dx).abs();
    let chord_sq = dx * dx + dy * dy;

    if d * d <= tolerance * tolerance * chord_sq || chord_sq < 1e-20 {
        out.push(p2);
        return;
    }

    // Subdivide at t=0.5
    let m01 = P2::new((p0.x + p1.x) * 0.5, (p0.y + p1.y) * 0.5);
    let m12 = P2::new((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5);
    let mid = P2::new((m01.x + m12.x) * 0.5, (m01.y + m12.y) * 0.5);

    flatten_quad(p0, m01, mid, tolerance, out);
    flatten_quad(mid, m12, p2, tolerance, out);
}

/// Flatten a cubic bezier curve to line segments.
fn flatten_cubic(p0: P2, p1: P2, p2: P2, p3: P2, tolerance: f64, out: &mut Vec<P2>) {
    // Flatness test: max distance of control points from the chord
    let dx = p3.x - p0.x;
    let dy = p3.y - p0.y;
    let d2 = ((p1.x - p3.x) * dy - (p1.y - p3.y) * dx).abs();
    let d3 = ((p2.x - p3.x) * dy - (p2.y - p3.y) * dx).abs();
    let chord_sq = dx * dx + dy * dy;

    if (d2 + d3) * (d2 + d3) <= tolerance * tolerance * chord_sq || chord_sq < 1e-20 {
        out.push(p3);
        return;
    }

    // De Casteljau subdivision at t=0.5
    let m01 = P2::new((p0.x + p1.x) * 0.5, (p0.y + p1.y) * 0.5);
    let m12 = P2::new((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5);
    let m23 = P2::new((p2.x + p3.x) * 0.5, (p2.y + p3.y) * 0.5);
    let m012 = P2::new((m01.x + m12.x) * 0.5, (m01.y + m12.y) * 0.5);
    let m123 = P2::new((m12.x + m23.x) * 0.5, (m12.y + m23.y) * 0.5);
    let mid = P2::new((m012.x + m123.x) * 0.5, (m012.y + m123.y) * 0.5);

    flatten_cubic(p0, m01, m012, mid, tolerance, out);
    flatten_cubic(mid, m123, m23, p3, tolerance, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_rect_svg() -> &'static [u8] {
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect x="10" y="10" width="80" height="60"/>
        </svg>"#
    }

    fn triangle_svg() -> &'static [u8] {
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <polygon points="50,10 90,90 10,90"/>
        </svg>"#
    }

    fn circle_svg() -> &'static [u8] {
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <circle cx="50" cy="50" r="40"/>
        </svg>"#
    }

    fn multi_path_svg() -> &'static [u8] {
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
            <rect x="10" y="10" width="80" height="60"/>
            <rect x="100" y="100" width="50" height="50"/>
        </svg>"#
    }

    #[test]
    fn test_load_rect() {
        let polys = load_svg_data(simple_rect_svg(), 0.1).unwrap();
        assert_eq!(polys.len(), 1, "Rectangle SVG should produce 1 polygon");
        let poly = &polys[0];
        assert!(poly.exterior.len() >= 4, "Rectangle should have at least 4 points");

        // Area should be approximately 80 * 60 = 4800
        let area = poly.area();
        assert!(
            (area - 4800.0).abs() < 10.0,
            "Rectangle area {} should be ~4800",
            area
        );
    }

    #[test]
    fn test_load_triangle() {
        let polys = load_svg_data(triangle_svg(), 0.1).unwrap();
        assert_eq!(polys.len(), 1);
        let poly = &polys[0];
        assert!(poly.exterior.len() >= 3);
    }

    #[test]
    fn test_load_circle() {
        let polys = load_svg_data(circle_svg(), 0.5).unwrap();
        assert_eq!(polys.len(), 1);
        let poly = &polys[0];

        // Circle with r=40: area = pi * 40^2 ≈ 5027
        let area = poly.area();
        assert!(
            (area - 5026.5).abs() < 100.0,
            "Circle area {} should be ~5027",
            area
        );
        // Should have many vertices from bezier flattening
        assert!(
            poly.exterior.len() > 8,
            "Flattened circle should have >8 points, got {}",
            poly.exterior.len()
        );
    }

    #[test]
    fn test_load_multiple_paths() {
        let polys = load_svg_data(multi_path_svg(), 0.1).unwrap();
        assert_eq!(
            polys.len(),
            2,
            "Two rectangles should produce 2 polygons"
        );
    }

    #[test]
    fn test_empty_svg() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"></svg>"#;
        let polys = load_svg_data(svg, 0.1).unwrap();
        assert!(polys.is_empty());
    }

    #[test]
    fn test_open_path_ignored() {
        // An open path (no Z/close) should be ignored
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <path d="M 10 10 L 90 10 L 90 90"/>
        </svg>"#;
        let polys = load_svg_data(svg, 0.1).unwrap();
        assert!(polys.is_empty(), "Open path should not produce a polygon");
    }

    #[test]
    fn test_closed_path_with_curves() {
        // Path with cubic bezier (heart-like shape)
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <path d="M 50 80 C 50 80 10 50 10 30 C 10 10 30 10 50 30 C 70 10 90 10 90 30 C 90 50 50 80 50 80 Z"/>
        </svg>"#;
        let polys = load_svg_data(svg, 0.5).unwrap();
        assert_eq!(polys.len(), 1, "Closed bezier path should produce 1 polygon");
        assert!(
            polys[0].exterior.len() > 10,
            "Flattened bezier should have many points"
        );
    }

    #[test]
    fn test_containment_rect_with_circle_hole() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 80 80">
            <rect x="5" y="5" width="70" height="70" rx="8" ry="8"/>
            <circle cx="40" cy="40" r="12"/>
        </svg>"#;
        let polys = load_svg_data(svg, 0.5).unwrap();
        assert_eq!(polys.len(), 1, "Circle inside rect should be detected as hole");
        assert_eq!(polys[0].holes.len(), 1, "Should have 1 hole");

        // Area with hole should be less than the exterior alone
        let exterior_area = polys[0].signed_area().abs();
        let net_area = polys[0].area();
        assert!(
            net_area < exterior_area,
            "Net area {} should be less than exterior area {}",
            net_area,
            exterior_area
        );

        // Net area should be approximately rect - circle
        // rect ≈ 70*70 = 4900 (rounded corners slightly less)
        // circle = pi * 12^2 ≈ 452
        let expected_approx = 4900.0 - 452.0;
        assert!(
            (net_area - expected_approx).abs() < 200.0,
            "Net area {} should be near {} (rect minus circle)",
            net_area,
            expected_approx
        );
    }

    #[test]
    fn test_winding_is_ccw() {
        let polys = load_svg_data(simple_rect_svg(), 0.1).unwrap();
        for poly in &polys {
            assert!(
                poly.signed_area() > 0.0,
                "All polygons should be CCW after ensure_winding"
            );
        }
    }
}
