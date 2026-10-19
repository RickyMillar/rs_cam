//! SVG file input — extracts closed and open paths as selection items.
//!
//! Uses usvg to parse and simplify SVG into basic path segments,
//! then flattens bezier curves to polylines within a configurable tolerance.
//!
//! ### Unit handling (what the code does today)
//! The door reads the path data of each node (`path.data()`) and does NOT
//! apply `abs_transform()`. So the coordinates are the raw user units of
//! the file: the units of the viewBox, or of the root when the file has no
//! viewBox. The door takes one user unit as 1 mm, times `scale`. It ignores
//! the declared `width` and `height`, and every group transform.
//!
//! The P4.0 probe (2026-10-19, `planning/geometry_selection_2026-10-16/PLAN.md`,
//! "P4.0 SVG probe facts") measured a 100 x 50 rect at scale 1.0:
//!
//! | Input | Size (mm) |
//! |---|---|
//! | `width="100mm" viewBox="0 0 100 50"` | 100 x 50 |
//! | `width="100"` (no units), no viewBox | 100 x 50 |
//! | `width="100mm"`, no viewBox | 100 x 50 |
//! | as row 1, in `<g transform="translate(10,5) scale(2)">` | 100 x 50 at (0, 0): the transform is LOST |
//! | `width="100mm"`, viewBox in px (`0 0 377.95 188.98`) | 377.95 x 188.98 (3.78 x too large) |
//!
//! So a modern Inkscape file (a mm viewBox, no group transform) imports at
//! 1:1. The default scale is [`SVG_DEFAULT_UNITS`]: the operator has not
//! ruled on O-2 (the SVG scale and the lost transforms). [`svg_declared_size`]
//! reads the declared size, so the import dialog can show it beside the
//! size of the import.
//!
//! ### The join of open subpaths (P4.8, ruling O-5)
//! The open subpaths of one layer join end to end where two ends meet
//! within the join tolerance, with the DXF rule
//! ([`crate::selection::join`]). The join runs in user units, before
//! `scale`, as the DXF join runs before the declared units. A chain that
//! comes back to its start is a loop. Before P4.8 the door did not join, so
//! an open subpath whose ends met stayed an open path.

use crate::compute::stock_config::ModelUnits;
use crate::geo::P2;
use crate::io::{DrawingImport, ImportReport};
use crate::polygon::Polygon2;
use crate::selection::join::{OpenPiece, join_open_pieces};
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
    /// The root element could not be read for its declared size.
    #[error("Failed to read the SVG size: {0}")]
    Size(String),
}

/// THE default units of an SVG import (P4.8). Every interactive door (the
/// GUI import dialog, MCP `import_model` with no `units`) reads it through
/// [`svg_default_units`]. Today it is 1:1: one user unit is 1 mm (see the
/// module doc). Operator ruling O-2 changes this one value.
pub const SVG_DEFAULT_UNITS: ModelUnits = ModelUnits::Millimeters;

/// The default units for an SVG with this declared size. Today it is
/// [`SVG_DEFAULT_UNITS`] for every file. The function takes the declared
/// size so that an O-2 ruling that reads it changes this function only.
pub fn svg_default_units(_declared: &SvgDeclaredSize) -> ModelUnits {
    SVG_DEFAULT_UNITS
}

/// One declared length of the root `<svg>`: the text as written, and its
/// value in mm when the unit is a physical one (mm, cm, in, pt, pc).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DeclaredLength {
    /// The attribute text, for example `100mm`.
    pub text: String,
    /// The length in mm. `None` for no unit, `px`, `%` or an unknown unit:
    /// those name no physical size.
    pub mm: Option<f64>,
}

/// The size an SVG file declares on its root element: `width`, `height`
/// and `viewBox`, as written. The door does not read them (module doc);
/// the import dialog shows them beside the size of the import.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct SvgDeclaredSize {
    pub width: Option<DeclaredLength>,
    pub height: Option<DeclaredLength>,
    /// `[min_x, min_y, width, height]` in user units.
    pub view_box: Option<[f64; 4]>,
}

impl SvgDeclaredSize {
    /// The declared size as one line, for example
    /// `width="100mm" height="50mm" viewBox="0 0 100 50"`.
    pub fn summary_line(&self) -> String {
        let mut parts = Vec::new();
        if let Some(w) = &self.width {
            parts.push(format!("width=\"{}\"", w.text));
        }
        if let Some(h) = &self.height {
            parts.push(format!("height=\"{}\"", h.text));
        }
        if let Some([x, y, w, h]) = self.view_box {
            parts.push(format!("viewBox=\"{x} {y} {w} {h}\""));
        }
        if parts.is_empty() {
            "no declared size".to_owned()
        } else {
            parts.join(" ")
        }
    }
}

/// Parse one SVG length attribute.
fn declared_length(text: &str) -> DeclaredLength {
    let trimmed = text.trim();
    let split = trimmed
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')))
        .unwrap_or(trimmed.len());
    let (number, unit) = trimmed.split_at(split);
    let per_mm = match unit.trim() {
        "mm" => Some(1.0),
        "cm" => Some(10.0),
        "in" => Some(25.4),
        "pt" => Some(25.4 / 72.0),
        "pc" => Some(25.4 / 6.0),
        _ => None,
    };
    DeclaredLength {
        text: trimmed.to_owned(),
        mm: per_mm.and_then(|f| number.parse::<f64>().ok().map(|n| n * f)),
    }
}

/// Read the declared size of SVG data (P4.8). It reads the root element
/// only; it does not parse the drawing.
pub fn svg_declared_size(data: &[u8]) -> Result<SvgDeclaredSize, SvgError> {
    let text = std::str::from_utf8(data).map_err(|e| SvgError::Size(e.to_string()))?;
    let doc = usvg::roxmltree::Document::parse(text).map_err(|e| SvgError::Size(e.to_string()))?;
    let root = doc.root_element();
    let view_box = root.attribute("viewBox").and_then(|v| {
        let n: Vec<f64> = v
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse().ok())
            .collect();
        match n.as_slice() {
            [x, y, w, h] => Some([*x, *y, *w, *h]),
            _ => None,
        }
    });
    Ok(SvgDeclaredSize {
        width: root.attribute("width").map(declared_length),
        height: root.attribute("height").map(declared_length),
        view_box,
    })
}

/// [`svg_declared_size`] of a file, with the import size guard.
pub fn svg_declared_size_of_file(path: &Path) -> Result<SvgDeclaredSize, SvgError> {
    if let Some(over) = crate::io::file_size_over_limit(path, crate::io::MAX_IMPORT_FILE_SIZE)? {
        return Err(SvgError::FileTooLarge {
            size_mb: over.size_mb,
            limit_mb: over.limit_mb,
        });
    }
    svg_declared_size(&std::fs::read(path)?)
}

/// Load the paths of an SVG file as selection items (P2.1).
///
/// Bezier curves are flattened to line segments with the given tolerance
/// (user units). `scale` moves user units to mm; the door applies it after
/// the flattening and the join, and before it classifies circles, so the
/// circle floor is in mm. See [`load_svg_data`] for the items.
pub fn load_svg(
    path: &Path,
    tolerance: f64,
    scale: f64,
    join_tolerance_mm: f64,
) -> Result<DrawingImport, SvgError> {
    if let Some(over) = crate::io::file_size_over_limit(path, crate::io::MAX_IMPORT_FILE_SIZE)? {
        return Err(SvgError::FileTooLarge {
            size_mb: over.size_mb,
            limit_mb: over.limit_mb,
        });
    }
    let data = std::fs::read(path)?;
    load_svg_data(&data, tolerance, scale, join_tolerance_mm)
}

/// Load the closed and open paths of SVG data bytes as items, in document
/// order:
///
/// - a closed subpath is a `Circle` when it is circle-like
///   ([`circle_like_ring`]), else a `Loop`;
/// - the open subpaths of two or more points join end to end within
///   `join_tolerance_mm` (user units, P4.8). A chain that comes back to its
///   start is a `Loop` (or a `Circle`), else a `Path` (P1): no chord joins
///   two ends that do not meet. A chain takes the place of its first
///   subpath;
/// - the layer of an item is the `id` of its nearest ancestor `<g>` with an
///   id. usvg keeps the `id` of a group and drops `inkscape:label`, so the
///   id is the label this door can read. A shape outside such a group has
///   no layer.
///
/// Containment does not run here (P2.1). The coordinates are the raw user
/// units of the file times `scale` (see the module doc).
pub fn load_svg_data(
    data: &[u8],
    tolerance: f64,
    scale: f64,
    join_tolerance_mm: f64,
) -> Result<DrawingImport, SvgError> {
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(data, &opt)?;
    let mut raw = Vec::new();
    visit_group(tree.root(), None, tolerance, &mut raw);
    // In user units: a closed ring is a loop; the open subpaths go to the
    // join, each at its document index.
    let mut shapes: Vec<(usize, Option<Arc<str>>, Polygon2)> = Vec::new();
    let mut pieces: Vec<OpenPiece> = Vec::new();
    for (index, (layer, poly)) in raw.into_iter().enumerate() {
        if poly.closed {
            shapes.push((index, layer, poly));
        } else {
            pieces.push(OpenPiece {
                index,
                layer,
                pts: poly.exterior,
            });
        }
    }
    let (joined, layers) = join_open_pieces(&pieces, join_tolerance_mm);
    shapes.extend(
        joined
            .into_iter()
            .map(|shape| (shape.index, shape.layer, shape.polygon)),
    );
    // Document order. The sort is stable and the indices are distinct.
    shapes.sort_by_key(|(index, _, _)| *index);
    let items = shapes
        .into_iter()
        .map(|(_, layer, poly)| {
            let shape = if poly.closed {
                Shape::Loop(poly)
            } else {
                Shape::Path(poly)
            };
            Item::new(SourceRef::ToolpathModel, layer, shape)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Scale, then find the circles in mm.
    let items = ItemSet::new(items)
        .scaled(scale)?
        .with_circles(|ring| circle_like_ring(&ring.exterior))?;
    Ok(DrawingImport {
        items,
        report: ImportReport {
            join_tolerance_mm,
            layers,
        },
    })
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::selection::ItemKind;
    use crate::selection::join::CLOSE_TOL_MM;

    /// The polygon list of the import before P2.1 (the bridge): the drawn
    /// rings, nested.
    fn load_polys(data: &[u8], tolerance: f64) -> Vec<Polygon2> {
        load_svg_data(data, tolerance, 1.0, CLOSE_TOL_MM)
            .unwrap()
            .items
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
        // An open path is never a circle item, round or not.
        let items = load_svg_data(svg, 0.1, 1.0, CLOSE_TOL_MM).unwrap().items;
        assert!(!items.iter().any(|i| i.kind() == ItemKind::Circle));
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
        let path = Item::new(
            SourceRef::ToolpathModel,
            None,
            Shape::Path(Polygon2::open_path(ring)),
        )
        .unwrap();
        let items = ItemSet::new(vec![path])
            .with_circles(|r| circle_like_ring(&r.exterior))
            .unwrap();
        assert!(!items.iter().any(|i| i.kind() == ItemKind::Circle));
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
    fn circle_inside_a_rect_is_one_circle_item() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 80">
            <rect x="5" y="5" width="70" height="50"/>
            <circle cx="40" cy="30" r="10"/>
        </svg>"#;
        let items = load_svg_data(svg, 0.1, 1.0, CLOSE_TOL_MM).unwrap().items;
        let centres: Vec<P2> = items
            .iter()
            .filter_map(|i| match i.shape() {
                Shape::Circle { centre, .. } => Some(*centre),
                _ => None,
            })
            .collect();
        assert_eq!(centres.len(), 1, "one circle, one item: {centres:?}");
        assert!((centres[0].x - 40.0).abs() < 0.1 && (centres[0].y - 30.0).abs() < 0.1);
    }
}
