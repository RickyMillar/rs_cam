//! SVG file input — extracts closed and open paths as selection items.
//!
//! Uses usvg to parse and simplify SVG into basic path segments,
//! then flattens bezier curves to polylines within a configurable tolerance.
//!
//! ### Unit handling
//! usvg normalizes all SVG length units (mm, in, cm, px, pt, etc.) into
//! user-space coordinates (pixels at 96 DPI) during parsing. The resulting
//! coordinates are in **px** (CSS pixels, 1px = 1/96 inch). If the SVG
//! specifies explicit `width`/`height` with physical units (e.g. `width="100mm"`),
//! usvg applies the viewBox transform so coordinates are still in px.
//!
//! **Implication for CAM**: callers should treat SVG coordinates as px and
//! apply a DPI-based scale factor when physical dimensions matter.
//! At 96 DPI: 1 px = 0.2646 mm (25.4 / 96).

use crate::geo::P2;
use crate::polygon::Polygon2;
use crate::selection::{Item, ItemError, ItemSet, Shape, SourceRef};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SvgError {
    #[error("Failed to read SVG file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse SVG: {0}")]
    Parse(#[from] usvg::Error),
    #[error("No closed paths found in SVG")]
    NoPaths,
    /// The file is over [`crate::io::MAX_IMPORT_FILE_SIZE`]. The door
    /// refuses before it reads the bytes (EDG-03).
    #[error("SVG file too large ({size_mb} MB, limit {limit_mb} MB)")]
    FileTooLarge { size_mb: u64, limit_mb: u64 },
    /// A shape that breaks an item invariant: a defect in this door.
    #[error("SVG import made an invalid item: {0}")]
    Item(#[from] ItemError),
}

/// Load the paths of an SVG file as selection items (P2.1).
///
/// Bezier curves are flattened to line segments with the given tolerance
/// (user units). `scale` moves user units to mm; the door applies it after
/// the flattening and before it classifies circles, so the circle floor is
/// in mm. See [`load_svg_data`] for the items.
pub fn load_svg(path: &Path, tolerance: f64, scale: f64) -> Result<ItemSet, SvgError> {
    if let Some(over) = crate::io::file_size_over_limit(path, crate::io::MAX_IMPORT_FILE_SIZE)? {
        return Err(SvgError::FileTooLarge {
            size_mb: over.size_mb,
            limit_mb: over.limit_mb,
        });
    }
    let data = std::fs::read(path)?;
    load_svg_data(&data, tolerance, scale)
}

/// Load the closed and open paths of SVG data bytes as items, in document
/// order:
///
/// - a closed subpath is a `Circle` when it is circle-like
///   ([`circle_like_ring`]), else a `Loop`;
/// - an open subpath of two or more points is a `Path` (P1): no chord joins
///   its ends;
/// - the layer of an item is the `id` of its nearest ancestor `<g>` with an
///   id. usvg keeps the `id` of a group and drops `inkscape:label`, so the
///   id is the label this door can read. A shape outside such a group has
///   no layer.
///
/// Containment does not run here (P2.1). The coordinates are user-space
/// units (CSS pixels at 96 DPI) times `scale`.
pub fn load_svg_data(data: &[u8], tolerance: f64, scale: f64) -> Result<ItemSet, SvgError> {
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(data, &opt)?;
    let mut raw = Vec::new();
    visit_group(tree.root(), None, tolerance, &mut raw);
    // In user units: a closed ring is a loop, an open subpath a path.
    let items = raw
        .into_iter()
        .map(|(layer, poly)| {
            let shape = if poly.closed {
                Shape::Loop(poly)
            } else {
                Shape::Path(poly)
            };
            Item::new(SourceRef::ToolpathModel, layer, shape)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Scale, then find the circles in mm.
    Ok(ItemSet::new(items)
        .scaled(scale)?
        .with_circles(|ring| circle_like_ring(&ring.exterior))?)
}

/// One flattened subpath with the layer of its group.
type RawRing = (Option<Arc<str>>, Polygon2);

fn visit_group(
    group: &usvg::Group,
    layer: Option<&Arc<str>>,
    tolerance: f64,
    out: &mut Vec<RawRing>,
) {
    for node in group.children() {
        match node {
            usvg::Node::Path(path) => {
                let mut polys = Vec::new();
                extract_polygons_from_path(path.data(), tolerance, &mut polys);
                out.extend(polys.into_iter().map(|p| (layer.cloned(), p)));
            }
            usvg::Node::Group(child) => {
                // The nearest group with an id names the layer.
                let own: Arc<str>;
                let child_layer = if child.id().is_empty() {
                    layer
                } else {
                    own = Arc::from(child.id());
                    Some(&own)
                };
                visit_group(child, child_layer, tolerance, out);
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
                // Start a new subpath. An unclosed previous one is an open
                // path (P1).
                emit_open_subpath(&mut current_subpath, out);
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
                        // SAFETY: len >= 2 checked above
                        #[allow(clippy::indexing_slicing)]
                        let first = pts[0];
                        // SAFETY: pts.len() >= 2 checked above
                        #[allow(clippy::expect_used)]
                        let last_pt = *pts.last().expect("len >= 2");
                        if (first.x - last_pt.x).abs() < 1e-6 && (first.y - last_pt.y).abs() < 1e-6
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
    // The last subpath, when the path data ends without a close.
    emit_open_subpath(&mut current_subpath, out);
}

/// P1: push an unclosed subpath of two or more points as an open path, then
/// clear it. Before P1 the import dropped it in silence.
fn emit_open_subpath(subpath: &mut Vec<P2>, out: &mut Vec<Polygon2>) {
    if subpath.len() >= 2 {
        out.push(Polygon2::open_path(std::mem::take(subpath)));
    } else {
        subpath.clear();
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

/// Layer name every circle-like SVG target carries, so the inspector's
/// "select all in layer" works on an SVG too.
pub const SVG_CIRCLE_LAYER: &str = "circles";

/// Minimum vertex count for a closed ring to be a circle candidate.
const CIRCLE_LIKE_MIN_VERTICES: usize = 8;
/// Relative radial tolerance: every vertex within 2 % of the mean radius.
const CIRCLE_LIKE_REL_TOL: f64 = 0.02;
/// Absolute floor on that tolerance, mm, so a tiny circle is not rejected
/// on rounding.
const CIRCLE_LIKE_ABS_TOL_MM: f64 = 0.05;

/// The centre and diameter of a closed ring when it is circle-like, else
/// `None`. GEOMETRY CLASSIFIER, not a safety gate (G-DRILLCENTROID rework).
///
/// The rule: at least [`CIRCLE_LIKE_MIN_VERTICES`] vertices, and with `c`
/// the vertex centroid and `r̄` the mean vertex distance from `c`, every
/// vertex satisfies `|r_i − r̄| ≤ max(0.02·r̄, 0.05 mm)`. A star, a
/// rectangle or a free shape fails on the radius spread; a triangle or a
/// rectangle also fails on the count. A regular polygon of 8+ sides is
/// accepted — it drills at its centre, which is what an operator who drew
/// one on a hole layer meant. A duplicated closing vertex is harmless.
pub fn circle_like_ring(ring: &[P2]) -> Option<(P2, f64)> {
    if ring.len() < CIRCLE_LIKE_MIN_VERTICES {
        return None;
    }
    let n = ring.len() as f64;
    let (sx, sy) = ring
        .iter()
        .fold((0.0, 0.0), |(ax, ay), p| (ax + p.x, ay + p.y));
    let c = P2::new(sx / n, sy / n);
    let radii: Vec<f64> = ring.iter().map(|p| (p.x - c.x).hypot(p.y - c.y)).collect();
    let mean = radii.iter().sum::<f64>() / n;
    if !mean.is_finite() || mean <= 0.0 {
        return None;
    }
    let tol = (CIRCLE_LIKE_REL_TOL * mean).max(CIRCLE_LIKE_ABS_TOL_MM);
    radii
        .iter()
        .all(|r| (r - mean).abs() <= tol)
        .then_some((c, 2.0 * mean))
}

/// Every circle-like ring in a polygon set — exteriors AND holes, because
/// `detect_containment` files a circle drawn inside an outline as that
/// outline's hole — as pickable drill targets on [`SVG_CIRCLE_LAYER`].
///
/// This is how an SVG drawing exposed drill targets before P2.1: usvg
/// flattens a `<circle>` to a path before this crate sees it, so the source
/// element is gone and only the shape remains. Its one caller is the bridge
/// `LoadedModel::drill_targets_pre_selection`, which gives it the nested
/// polygon list in mm. P2.3c deletes this with the bridge.
pub fn circle_like_drill_targets(polygons: &[Polygon2]) -> Vec<crate::io::dxf_input::DrillTarget> {
    polygons
        .iter()
        // An open arc is not a hole, however round it is (P1).
        .filter(|poly| poly.closed)
        .flat_map(|poly| {
            std::iter::once(poly.exterior.as_slice()).chain(poly.holes.iter().map(Vec::as_slice))
        })
        .filter_map(circle_like_ring)
        .map(|(c, diameter)| crate::io::dxf_input::DrillTarget {
            x: c.x,
            y: c.y,
            layer: SVG_CIRCLE_LAYER.to_owned(),
            kind: crate::io::dxf_input::DrillTargetKind::CircleCenter { diameter },
        })
        .collect()
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

    /// The polygon list of the import before P2.1 (the bridge): the drawn
    /// rings, nested.
    fn load_polys(data: &[u8], tolerance: f64) -> Vec<Polygon2> {
        load_svg_data(data, tolerance, 1.0)
            .unwrap()
            .outlines_pre_selection()
            .as_ref()
            .clone()
    }

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
        let polys = load_polys(simple_rect_svg(), 0.1);
        assert_eq!(polys.len(), 1, "Rectangle SVG should produce 1 polygon");
        let poly = &polys[0];
        assert!(
            poly.exterior.len() >= 4,
            "Rectangle should have at least 4 points"
        );

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
        let polys = load_polys(triangle_svg(), 0.1);
        assert_eq!(polys.len(), 1);
        let poly = &polys[0];
        assert!(poly.exterior.len() >= 3);
    }

    #[test]
    fn test_load_circle() {
        let polys = load_polys(circle_svg(), 0.5);
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
        let polys = load_polys(multi_path_svg(), 0.1);
        assert_eq!(polys.len(), 2, "Two rectangles should produce 2 polygons");
    }

    #[test]
    fn test_empty_svg() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"></svg>"#;
        let polys = load_polys(svg, 0.1);
        assert!(polys.is_empty());
    }

    /// P1 sentry (f): an open SVG subpath is kept as an open path, with no
    /// chord. This holds for the last subpath of the data, for a subpath
    /// that a later `M` ends, and for a 2-point line.
    #[test]
    fn an_open_svg_subpath_is_kept_open_p1() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <path d="M 10 10 L 90 10 L 90 90"/>
        </svg>"#;
        let polys = load_polys(svg, 0.1);
        assert_eq!(polys.len(), 1, "the open path is kept");
        assert!(!polys[0].closed);
        assert_eq!(polys[0].exterior.len(), 3, "no closing point is added");

        // An open subpath, then a closed square, then a 2-point line.
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <path d="M 0 0 L 5 5 L 9 0 M 20 20 L 40 20 L 40 40 L 20 40 Z M 60 60 L 80 60"/>
        </svg>"#;
        let polys = load_polys(svg, 0.1);
        let open: Vec<&Polygon2> = polys.iter().filter(|p| !p.closed).collect();
        let closed: Vec<&Polygon2> = polys.iter().filter(|p| p.closed).collect();
        assert_eq!(closed.len(), 1, "the square");
        assert_eq!(open.len(), 2, "the open subpath and the line");
        assert!(open.iter().any(|p| p.exterior.len() == 3));
        assert!(open.iter().any(|p| p.exterior.len() == 2));
        // An open path is never a drill target, round or not.
        assert!(circle_like_drill_targets(&polys).is_empty());
    }

    /// P1: a round open arc is not a circle-like drill target.
    #[test]
    fn an_open_round_arc_is_no_drill_target_p1() {
        let ring: Vec<P2> = (0..=16)
            .map(|i| {
                let t = std::f64::consts::PI * 1.9 * f64::from(i) / 16.0;
                P2::new(10.0 * t.cos(), 10.0 * t.sin())
            })
            .collect();
        assert!(circle_like_ring(&ring).is_some(), "the ring is round");
        assert!(circle_like_drill_targets(&[Polygon2::open_path(ring)]).is_empty());
    }

    #[test]
    fn test_closed_path_with_curves() {
        // Path with cubic bezier (heart-like shape)
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <path d="M 50 80 C 50 80 10 50 10 30 C 10 10 30 10 50 30 C 70 10 90 10 90 30 C 90 50 50 80 50 80 Z"/>
        </svg>"#;
        let polys = load_polys(svg, 0.5);
        assert_eq!(
            polys.len(),
            1,
            "Closed bezier path should produce 1 polygon"
        );
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
        let polys = load_polys(svg, 0.5);
        assert_eq!(
            polys.len(),
            1,
            "Circle inside rect should be detected as hole"
        );
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
        let polys = load_polys(simple_rect_svg(), 0.1);
        for poly in &polys {
            assert!(
                poly.signed_area() > 0.0,
                "All polygons should be CCW after ensure_winding"
            );
        }
    }

    #[test]
    fn circle_like_classifier_accepts_circles_and_rejects_stars_and_rects() {
        // A 32-gon of radius 10 about (3, 4).
        let circle: Vec<P2> = (0..32)
            .map(|i| {
                let a = std::f64::consts::TAU * f64::from(i) / 32.0;
                P2::new(3.0 + 10.0 * a.cos(), 4.0 + 10.0 * a.sin())
            })
            .collect();
        let (c, d) = circle_like_ring(&circle).expect("a 32-gon is circle-like");
        assert!((c.x - 3.0).abs() < 1e-9 && (c.y - 4.0).abs() < 1e-9);
        assert!((d - 20.0).abs() < 1e-6, "diameter {d}");

        // The demo star: ten vertices, radii alternate ~45 and ~18.
        let star: Vec<P2> = [
            (50.0, 5.0),
            (61.0, 35.0),
            (95.0, 35.0),
            (68.0, 57.0),
            (79.0, 91.0),
            (50.0, 70.0),
            (21.0, 91.0),
            (32.0, 57.0),
            (5.0, 35.0),
            (39.0, 35.0),
        ]
        .iter()
        .map(|&(x, y)| P2::new(x, y))
        .collect();
        assert!(circle_like_ring(&star).is_none(), "a star is not a circle");

        // A rectangle: too few vertices, and the radii differ anyway.
        let rect = Polygon2::rectangle(0.0, 0.0, 10.0, 4.0);
        assert!(circle_like_ring(&rect.exterior).is_none());

        // A 12-vertex ellipse 10 × 6 fails the 2 % spread.
        let ellipse: Vec<P2> = (0..12)
            .map(|i| {
                let a = std::f64::consts::TAU * f64::from(i) / 12.0;
                P2::new(10.0 * a.cos(), 6.0 * a.sin())
            })
            .collect();
        assert!(
            circle_like_ring(&ellipse).is_none(),
            "an ellipse is not a circle"
        );
    }

    #[test]
    fn circle_inside_a_rect_is_a_target_via_the_hole_ring() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 80">
            <rect x="5" y="5" width="70" height="50"/>
            <circle cx="40" cy="30" r="10"/>
        </svg>"#;
        let polys = load_polys(svg, 0.1);
        let targets = circle_like_drill_targets(&polys);
        assert_eq!(targets.len(), 1, "one circle, one target: {targets:?}");
        assert!((targets[0].x - 40.0).abs() < 0.1 && (targets[0].y - 30.0).abs() < 0.1);
        assert_eq!(targets[0].layer, SVG_CIRCLE_LAYER);
    }
}
