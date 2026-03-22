//! DXF file input — extracts entities as [`Polygon2`].
//!
//! ### Supported entity types
//! - **LwPolyline** — closed lightweight polylines (with optional bulge arcs)
//! - **Polyline** — closed legacy polylines (with optional bulge arcs)
//! - **Circle** — tessellated to a closed polygon
//! - **Ellipse** — tessellated to a closed polygon (full or partial)
//! - **Arc** — full-circle arcs become circles; partial arcs are chord-closed
//! - **Line** — individual 2-point segments are chain-linked by shared endpoints;
//!   closed chains become closed polygons, open chains become open-path polygons
//!   (useful for trace/engraving toolpaths)
//! - **Spline** — B-spline curves evaluated via De Boor's algorithm; both closed
//!   and open splines are returned
//!
//! Both closed and open paths are returned. Closed paths are suitable for pocket
//! and profile operations; open paths are suitable for trace and engraving
//! toolpaths.
//!
//! ### Unit handling
//! The `$INSUNITS` header variable is read (via `drawing.header.default_drawing_units`).
//! When the DXF specifies units other than millimeters, all coordinates are scaled
//! to mm automatically.

use crate::geo::P2;
use crate::polygon::Polygon2;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DxfError {
    #[error("Failed to read DXF file: {0}")]
    Io(#[from] dxf::DxfError),
    #[error("No closed entities found in DXF")]
    NoEntities,
}

/// Return a scale factor to convert from `$INSUNITS` to millimeters.
///
/// Falls back to 1.0 (assume mm) for unrecognized or unitless drawings.
fn insunits_to_mm_scale(units: dxf::enums::Units) -> f64 {
    use dxf::enums::Units;
    match units {
        Units::Inches => 25.4,
        Units::Feet => 304.8,
        Units::Millimeters => 1.0,
        Units::Centimeters => 10.0,
        Units::Meters => 1000.0,
        Units::Microinches => 25.4e-6,
        Units::Mils => 0.0254,
        Units::Yards => 914.4,
        Units::Nanometers => 1e-6,
        Units::Microns => 0.001,
        Units::Decimeters => 100.0,
        Units::Decameters => 10_000.0,
        Units::Hectometers => 100_000.0,
        Units::Kilometers => 1_000_000.0,
        // Unitless or exotic (Angstroms, AU, LightYears, etc.) — assume mm
        _ => 1.0,
    }
}

/// Load closed polygon entities from a DXF file.
///
/// Arc segments (bulge values in polylines, circles, ellipses) are
/// tessellated to line segments with the given angular tolerance in degrees.
/// Coordinates are scaled from `$INSUNITS` to millimeters automatically.
pub fn load_dxf(path: &Path, arc_tolerance_deg: f64) -> Result<Vec<Polygon2>, DxfError> {
    let drawing = dxf::Drawing::load_file(path.to_str().unwrap_or(""))?;
    Ok(extract_polygons(&drawing, arc_tolerance_deg))
}

/// Load closed polygon entities from a DXF Drawing.
pub fn extract_polygons(drawing: &dxf::Drawing, arc_tolerance_deg: f64) -> Vec<Polygon2> {
    let scale = insunits_to_mm_scale(drawing.header.default_drawing_units);
    let mut raw = extract_polygons_flat(drawing, arc_tolerance_deg);
    if (scale - 1.0).abs() > 1e-12 {
        for poly in &mut raw {
            for pt in &mut poly.exterior {
                pt.x *= scale;
                pt.y *= scale;
            }
            for hole in &mut poly.holes {
                for pt in hole {
                    pt.x *= scale;
                    pt.y *= scale;
                }
            }
        }
    }
    // Detect containment: inner shapes become holes of outer shapes
    crate::polygon::detect_containment(raw)
}

/// Extract polygons without containment detection (flat list).
fn extract_polygons_flat(drawing: &dxf::Drawing, arc_tolerance_deg: f64) -> Vec<Polygon2> {
    let mut polygons = Vec::new();
    let arc_step_rad = arc_tolerance_deg.to_radians();

    // Collect Line segments for chain-linking after the main loop.
    let mut line_segments: Vec<(P2, P2)> = Vec::new();

    for entity in drawing.entities() {
        match &entity.specific {
            dxf::entities::EntityType::LwPolyline(lwp) => {
                if lwp.is_closed() && lwp.vertices.len() >= 3 {
                    let pts = lwpolyline_to_points(lwp, arc_step_rad);
                    if pts.len() >= 3 {
                        let mut poly = Polygon2::new(pts);
                        poly.ensure_winding();
                        polygons.push(poly);
                    }
                }
            }
            dxf::entities::EntityType::Polyline(poly_ent) => {
                if poly_ent.is_closed() {
                    let verts: Vec<_> = poly_ent.vertices().collect();
                    if verts.len() >= 3 {
                        let pts = polyline_to_points(&verts, arc_step_rad);
                        if pts.len() >= 3 {
                            let mut poly = Polygon2::new(pts);
                            poly.ensure_winding();
                            polygons.push(poly);
                        }
                    }
                }
            }
            dxf::entities::EntityType::Circle(circle) => {
                let pts = circle_to_points(
                    circle.center.x,
                    circle.center.y,
                    circle.radius,
                    arc_step_rad,
                );
                if pts.len() >= 3 {
                    let mut poly = Polygon2::new(pts);
                    poly.ensure_winding();
                    polygons.push(poly);
                }
            }
            dxf::entities::EntityType::Ellipse(ell) => {
                let pts = ellipse_to_points(ell, arc_step_rad);
                if pts.len() >= 3 {
                    let mut poly = Polygon2::new(pts);
                    poly.ensure_winding();
                    polygons.push(poly);
                }
            }
            dxf::entities::EntityType::Arc(arc) => {
                let pts = arc_entity_to_points(arc, arc_step_rad);
                if pts.len() >= 3 {
                    let mut poly = Polygon2::new(pts);
                    poly.ensure_winding();
                    polygons.push(poly);
                }
            }
            dxf::entities::EntityType::Line(line) => {
                let p1 = P2::new(line.p1.x, line.p1.y);
                let p2 = P2::new(line.p2.x, line.p2.y);
                line_segments.push((p1, p2));
            }
            dxf::entities::EntityType::Spline(spline) => {
                let pts = spline_to_points(spline, arc_step_rad);
                if pts.len() >= 2 {
                    let mut poly = Polygon2::new(pts);
                    poly.ensure_winding();
                    polygons.push(poly);
                }
            }
            _ => {}
        }
    }

    // Chain-link Line segments into polygons.
    if !line_segments.is_empty() {
        let chains = chain_line_segments(&line_segments);
        for chain in chains {
            if chain.len() >= 2 {
                let mut poly = Polygon2::new(chain);
                poly.ensure_winding();
                polygons.push(poly);
            }
        }
    }

    polygons
}

fn lwpolyline_to_points(lwp: &dxf::entities::LwPolyline, arc_step: f64) -> Vec<P2> {
    let n = lwp.vertices.len();
    let mut pts = Vec::new();

    for i in 0..n {
        let v = &lwp.vertices[i];
        let p = P2::new(v.x, v.y);

        pts.push(p);

        // If this vertex has a bulge, tessellate the arc to the next vertex
        if v.bulge.abs() > 1e-10 {
            let next = &lwp.vertices[(i + 1) % n];
            let next_p = P2::new(next.x, next.y);
            tessellate_bulge_arc(p, next_p, v.bulge, arc_step, &mut pts);
        }
    }

    pts
}

fn polyline_to_points(verts: &[&dxf::entities::Vertex], arc_step: f64) -> Vec<P2> {
    let n = verts.len();
    let mut pts = Vec::new();

    for i in 0..n {
        let v = verts[i];
        let p = P2::new(v.location.x, v.location.y);

        pts.push(p);

        if v.bulge.abs() > 1e-10 && i + 1 < n {
            let next_p = P2::new(verts[i + 1].location.x, verts[i + 1].location.y);
            tessellate_bulge_arc(p, next_p, v.bulge, arc_step, &mut pts);
        }
    }

    pts
}

/// Tessellate an arc defined by bulge value between two points.
///
/// Bulge = tan(sweep_angle / 4). Positive = CCW arc, negative = CW arc.
fn tessellate_bulge_arc(p1: P2, p2: P2, bulge: f64, arc_step: f64, out: &mut Vec<P2>) {
    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    let chord = (dx * dx + dy * dy).sqrt();
    if chord < 1e-10 {
        return;
    }

    // Arc properties from bulge
    let sweep = 4.0 * bulge.atan(); // sweep angle (signed)
    let abs_sweep = sweep.abs();
    if abs_sweep < 1e-10 {
        return;
    }

    let radius = chord / (2.0 * (abs_sweep / 2.0).sin());

    // Center of arc
    let mid_x = (p1.x + p2.x) / 2.0;
    let mid_y = (p1.y + p2.y) / 2.0;

    // Perpendicular offset from chord midpoint to center
    let sagitta = radius * (1.0 - (abs_sweep / 2.0).cos());
    let h = radius - sagitta; // distance from midpoint to center along perpendicular

    // Direction perpendicular to chord
    let ux = -dy / chord;
    let uy = dx / chord;

    // Center: offset from midpoint in perpendicular direction
    // Sign depends on bulge direction
    let sign = if bulge > 0.0 { 1.0 } else { -1.0 };
    let cx = mid_x + sign * h * ux;
    let cy = mid_y + sign * h * uy;

    // Start and end angles
    let start_angle = (p1.y - cy).atan2(p1.x - cx);

    // Number of intermediate points
    let n_steps = (abs_sweep / arc_step).ceil() as usize;
    if n_steps <= 1 {
        return;
    }

    let angle_step = sweep / n_steps as f64;

    // Generate intermediate points (skip first = p1, skip last = p2)
    for i in 1..n_steps {
        let angle = start_angle + angle_step * i as f64;
        let x = cx + radius.abs() * angle.cos();
        let y = cy + radius.abs() * angle.sin();
        out.push(P2::new(x, y));
    }
}

fn circle_to_points(cx: f64, cy: f64, radius: f64, arc_step: f64) -> Vec<P2> {
    let n = (std::f64::consts::TAU / arc_step).ceil() as usize;
    let n = n.max(8); // minimum 8 points
    (0..n)
        .map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / n as f64;
            P2::new(cx + radius * angle.cos(), cy + radius * angle.sin())
        })
        .collect()
}

/// Tessellate a DXF Arc entity into polygon points.
///
/// DXF Arc angles are in degrees, measured CCW from the +X axis.
/// The resulting polygon is closed by connecting the arc endpoints with a chord,
/// producing a "pie-slice" or "segment" shape that can be used as a closed region.
/// A full-circle arc (start == end or sweep ~360) is treated like a Circle entity.
fn arc_entity_to_points(arc: &dxf::entities::Arc, arc_step: f64) -> Vec<P2> {
    let cx = arc.center.x;
    let cy = arc.center.y;
    let r = arc.radius;

    // DXF arc angles are in degrees
    let start_deg = arc.start_angle;
    let end_deg = arc.end_angle;

    // Compute sweep (always positive, CCW direction)
    let mut sweep_deg = end_deg - start_deg;
    if sweep_deg <= 0.0 {
        sweep_deg += 360.0;
    }

    // Full circle check (within ~0.01 degree tolerance)
    if (sweep_deg - 360.0).abs() < 0.01 {
        return circle_to_points(cx, cy, r, arc_step);
    }

    let start_rad = start_deg.to_radians();
    let sweep_rad = sweep_deg.to_radians();

    let n = (sweep_rad / arc_step).ceil() as usize;
    let n = n.max(3);

    // Generate arc points (including start and end)
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = start_rad + sweep_rad * i as f64 / n as f64;
        pts.push(P2::new(cx + r * t.cos(), cy + r * t.sin()));
    }

    pts
}

fn ellipse_to_points(ell: &dxf::entities::Ellipse, arc_step: f64) -> Vec<P2> {
    let cx = ell.center.x;
    let cy = ell.center.y;
    let major_x = ell.major_axis.x;
    let major_y = ell.major_axis.y;
    let ratio = ell.minor_axis_ratio;

    // Full ellipse: start=0, end=2*pi
    let start = ell.start_parameter;
    let end = ell.end_parameter;
    let sweep = if (end - start).abs() < 1e-10 {
        std::f64::consts::TAU
    } else {
        end - start
    };

    let n = (sweep.abs() / arc_step).ceil() as usize;
    let n = n.max(8);

    // Major axis angle
    let major_len = (major_x * major_x + major_y * major_y).sqrt();
    let major_angle = major_y.atan2(major_x);

    let a = major_len; // semi-major
    let b = major_len * ratio; // semi-minor

    (0..n)
        .map(|i| {
            let t = start + sweep * i as f64 / n as f64;
            // Point on axis-aligned ellipse
            let ex = a * t.cos();
            let ey = b * t.sin();
            // Rotate by major axis angle
            let x = cx + ex * major_angle.cos() - ey * major_angle.sin();
            let y = cy + ex * major_angle.sin() + ey * major_angle.cos();
            P2::new(x, y)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Line chain-linking
// ---------------------------------------------------------------------------

/// Epsilon for endpoint matching when chain-linking line segments.
const CHAIN_EPS: f64 = 1e-6;

fn pts_near(a: P2, b: P2) -> bool {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy < CHAIN_EPS * CHAIN_EPS
}

/// Chain-link a set of individual line segments into connected polyline chains.
///
/// Greedy algorithm: pick an unused segment, then extend the chain in both
/// directions by finding unused segments whose endpoint matches the chain's
/// current head or tail.  Returns one `Vec<P2>` per chain.
fn chain_line_segments(segments: &[(P2, P2)]) -> Vec<Vec<P2>> {
    let n = segments.len();
    let mut used = vec![false; n];
    let mut chains: Vec<Vec<P2>> = Vec::new();

    for seed in 0..n {
        if used[seed] {
            continue;
        }
        used[seed] = true;
        let mut chain = vec![segments[seed].0, segments[seed].1];

        // Extend the chain until no more segments attach.
        let mut changed = true;
        while changed {
            changed = false;
            let tail = *chain.last().unwrap();
            let head = chain[0];

            for i in 0..n {
                if used[i] {
                    continue;
                }
                let (a, b) = segments[i];
                if pts_near(tail, a) {
                    // a matches tail — append b
                    chain.push(b);
                    used[i] = true;
                    changed = true;
                } else if pts_near(tail, b) {
                    // b matches tail — append a
                    chain.push(a);
                    used[i] = true;
                    changed = true;
                } else if pts_near(head, b) {
                    // b matches head — prepend a
                    chain.insert(0, a);
                    used[i] = true;
                    changed = true;
                } else if pts_near(head, a) {
                    // a matches head — prepend b
                    chain.insert(0, b);
                    used[i] = true;
                    changed = true;
                }
            }
        }

        chains.push(chain);
    }

    chains
}

// ---------------------------------------------------------------------------
// Spline evaluation (De Boor's algorithm)
// ---------------------------------------------------------------------------

/// Evaluate a B-spline at parameter `t` using De Boor's algorithm.
///
/// - `degree`: spline degree (p)
/// - `knots`: knot vector of length `control_points.len() + degree + 1`
/// - `control_points`: control-point (x, y) list
///
/// Returns `None` if `t` is out of range or inputs are invalid.
fn de_boor_eval(degree: usize, knots: &[f64], control_points: &[P2], t: f64) -> Option<P2> {
    let n = control_points.len();
    let p = degree;
    if n == 0 || knots.len() < n + p + 1 || p == 0 {
        return None;
    }

    // Find knot span index k such that knots[k] <= t < knots[k+1],
    // restricted to [p, n-1].  For t == knots[n] (the end), clamp to n-1.
    let mut k = p;
    for i in p..n {
        if knots[i + 1] > t {
            k = i;
            break;
        }
        k = i;
    }

    // Copy the p+1 relevant control points: d[j] = control_points[j + k - p]
    let mut d: Vec<P2> = Vec::with_capacity(p + 1);
    for j in 0..=p {
        let idx = j + k - p;
        if idx >= n {
            return None;
        }
        d.push(control_points[idx]);
    }

    // Triangular computation
    for r in 1..=p {
        for j in (r..=p).rev() {
            // Original index in the control-point array
            let i = j + k - p;
            let left = knots[i];
            let right = knots[i + p + 1 - r];
            let denom = right - left;
            if denom.abs() < 1e-14 {
                continue;
            }
            let alpha = (t - left) / denom;
            d[j] = P2::new(
                (1.0 - alpha) * d[j - 1].x + alpha * d[j].x,
                (1.0 - alpha) * d[j - 1].y + alpha * d[j].y,
            );
        }
    }

    Some(d[p])
}

/// Tessellate a DXF Spline entity into polyline points using De Boor's algorithm.
fn spline_to_points(spline: &dxf::entities::Spline, arc_step: f64) -> Vec<P2> {
    let degree = spline.degree_of_curve as usize;
    let knots = &spline.knot_values;
    let cp_dxf = &spline.control_points;

    if cp_dxf.len() < 2 || knots.len() < cp_dxf.len() + degree + 1 || degree == 0 {
        return Vec::new();
    }

    let control_points: Vec<P2> = cp_dxf.iter().map(|p| P2::new(p.x, p.y)).collect();

    // Valid parameter range: [knots[degree], knots[n]] where n = control_points.len().
    let t_start = knots[degree];
    let t_end = knots[control_points.len()];
    let span = t_end - t_start;
    if span.abs() < 1e-14 {
        return Vec::new();
    }

    // Choose the number of evaluation steps.  Use the arc_step angular
    // tolerance as a rough proxy: more segments for smaller tolerances.
    // A reasonable heuristic: number of spans * ceil(pi / arc_step).
    let n_spans = control_points.len().saturating_sub(degree);
    let steps_per_span = (std::f64::consts::PI / arc_step).ceil() as usize;
    let n_steps = (n_spans * steps_per_span).max(2);

    let mut pts = Vec::with_capacity(n_steps + 1);
    for i in 0..=n_steps {
        let t = t_start + span * (i as f64 / n_steps as f64);
        if let Some(pt) = de_boor_eval(degree, knots, &control_points, t) {
            pts.push(pt);
        }
    }

    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lwpolyline_drawing(vertices: Vec<(f64, f64, f64)>, closed: bool) -> dxf::Drawing {
        let mut drawing = dxf::Drawing::new();
        let mut lwp = dxf::entities::LwPolyline::default();
        for (x, y, bulge) in vertices {
            lwp.vertices.push(dxf::LwPolylineVertex {
                x,
                y,
                bulge,
                ..Default::default()
            });
        }
        lwp.set_is_closed(closed);
        drawing.add_entity(dxf::entities::Entity::new(
            dxf::entities::EntityType::LwPolyline(lwp),
        ));
        drawing
    }

    fn make_circle_drawing(cx: f64, cy: f64, radius: f64) -> dxf::Drawing {
        let mut drawing = dxf::Drawing::new();
        let circle = dxf::entities::Circle {
            center: dxf::Point::new(cx, cy, 0.0),
            radius,
            ..Default::default()
        };
        drawing.add_entity(dxf::entities::Entity::new(
            dxf::entities::EntityType::Circle(circle),
        ));
        drawing
    }

    #[test]
    fn test_lwpolyline_rectangle() {
        let drawing = make_lwpolyline_drawing(
            vec![
                (0.0, 0.0, 0.0),
                (100.0, 0.0, 0.0),
                (100.0, 50.0, 0.0),
                (0.0, 50.0, 0.0),
            ],
            true,
        );
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1);
        let area = polys[0].area();
        assert!(
            (area - 5000.0).abs() < 1.0,
            "Rectangle area {} should be 5000",
            area
        );
    }

    #[test]
    fn test_lwpolyline_open_ignored() {
        let drawing = make_lwpolyline_drawing(
            vec![(0.0, 0.0, 0.0), (100.0, 0.0, 0.0), (100.0, 50.0, 0.0)],
            false,
        );
        let polys = extract_polygons(&drawing, 5.0);
        assert!(polys.is_empty(), "Open polyline should be ignored");
    }

    #[test]
    fn test_circle() {
        let drawing = make_circle_drawing(50.0, 50.0, 25.0);
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1);

        let expected = std::f64::consts::PI * 25.0 * 25.0;
        let area = polys[0].area();
        assert!(
            (area - expected).abs() < expected * 0.05,
            "Circle area {} should be ~{} (within 5%)",
            area,
            expected
        );
    }

    #[test]
    fn test_lwpolyline_with_bulge_arcs() {
        // Rectangle with rounded corners (bulge on each vertex)
        let bulge = 0.4142; // ~tan(45/4) for ~90 degree arcs
        let drawing = make_lwpolyline_drawing(
            vec![
                (10.0, 0.0, bulge),
                (90.0, 0.0, bulge),
                (100.0, 10.0, bulge),
                (100.0, 40.0, bulge),
                (90.0, 50.0, bulge),
                (10.0, 50.0, bulge),
                (0.0, 40.0, bulge),
                (0.0, 10.0, bulge),
            ],
            true,
        );
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1);

        // Should have more points than the 8 vertices (arcs tessellated)
        assert!(
            polys[0].exterior.len() > 8,
            "Bulge arcs should add intermediate points, got {}",
            polys[0].exterior.len()
        );
    }

    #[test]
    fn test_multiple_entities() {
        let mut drawing = dxf::Drawing::new();

        // Add a rectangle
        let mut lwp = dxf::entities::LwPolyline::default();
        for (x, y) in [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)] {
            lwp.vertices.push(dxf::LwPolylineVertex {
                x,
                y,
                ..Default::default()
            });
        }
        lwp.set_is_closed(true);
        drawing.add_entity(dxf::entities::Entity::new(
            dxf::entities::EntityType::LwPolyline(lwp),
        ));

        // Add a circle
        let circle = dxf::entities::Circle {
            center: dxf::Point::new(50.0, 50.0, 0.0),
            radius: 20.0,
            ..Default::default()
        };
        drawing.add_entity(dxf::entities::Entity::new(
            dxf::entities::EntityType::Circle(circle),
        ));

        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 2, "Should extract both rectangle and circle");
    }

    #[test]
    fn test_winding_is_ccw() {
        let drawing = make_lwpolyline_drawing(
            vec![
                (0.0, 0.0, 0.0),
                (0.0, 100.0, 0.0), // CW winding
                (100.0, 100.0, 0.0),
                (100.0, 0.0, 0.0),
            ],
            true,
        );
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1);
        assert!(
            polys[0].signed_area() > 0.0,
            "Should be CCW after ensure_winding"
        );
    }

    #[test]
    fn test_too_few_vertices_ignored() {
        let drawing = make_lwpolyline_drawing(vec![(0.0, 0.0, 0.0), (10.0, 0.0, 0.0)], true);
        let polys = extract_polygons(&drawing, 5.0);
        assert!(polys.is_empty(), "2-vertex polyline should be ignored");
    }

    fn make_arc_drawing(
        cx: f64,
        cy: f64,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    ) -> dxf::Drawing {
        let mut drawing = dxf::Drawing::new();
        let arc = dxf::entities::Arc::new(dxf::Point::new(cx, cy, 0.0), radius, start_deg, end_deg);
        drawing.add_entity(dxf::entities::Entity::new(dxf::entities::EntityType::Arc(
            arc,
        )));
        drawing
    }

    #[test]
    fn test_arc_full_circle() {
        // A full-circle arc (0 to 360) should produce the same result as a Circle entity
        let drawing = make_arc_drawing(50.0, 50.0, 25.0, 0.0, 360.0);
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1, "Full-circle arc should produce 1 polygon");

        let expected = std::f64::consts::PI * 25.0 * 25.0;
        let area = polys[0].area();
        assert!(
            (area - expected).abs() < expected * 0.05,
            "Full-circle arc area {} should be ~{} (within 5%)",
            area,
            expected
        );
    }

    #[test]
    fn test_arc_semicircle() {
        // A 180-degree arc (semicircle) from 0 to 180 degrees
        let drawing = make_arc_drawing(0.0, 0.0, 10.0, 0.0, 180.0);
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1, "Semicircle arc should produce 1 polygon");

        // The polygon is the semicircle closed by a chord (diameter).
        // Area of a semicircular segment = pi*r^2/2
        let expected = std::f64::consts::PI * 10.0 * 10.0 / 2.0;
        let area = polys[0].area();
        assert!(
            (area - expected).abs() < expected * 0.10,
            "Semicircle area {} should be ~{} (within 10%)",
            area,
            expected
        );
    }

    #[test]
    fn test_arc_quarter_circle() {
        // A 90-degree arc
        let drawing = make_arc_drawing(0.0, 0.0, 10.0, 0.0, 90.0);
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1, "Quarter arc should produce 1 polygon");
        assert!(
            polys[0].exterior.len() >= 3,
            "Quarter arc should have at least 3 points"
        );
    }

    #[test]
    fn test_arc_wrap_around() {
        // Arc from 270 to 90 degrees (wraps through 0)
        let drawing = make_arc_drawing(0.0, 0.0, 10.0, 270.0, 90.0);
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1, "Wrap-around arc should produce 1 polygon");
        // This is a 180-degree arc (270 -> 360 -> 90)
        let expected = std::f64::consts::PI * 10.0 * 10.0 / 2.0;
        let area = polys[0].area();
        assert!(
            (area - expected).abs() < expected * 0.10,
            "Wrap-around arc area {} should be ~{} (within 10%)",
            area,
            expected
        );
    }

    #[test]
    fn test_insunits_inches_scale() {
        let mut drawing = dxf::Drawing::new();
        drawing.header.default_drawing_units = dxf::enums::Units::Inches;

        // Add a 1x1 inch rectangle
        let mut lwp = dxf::entities::LwPolyline::default();
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            lwp.vertices.push(dxf::LwPolylineVertex {
                x,
                y,
                ..Default::default()
            });
        }
        lwp.set_is_closed(true);
        drawing.add_entity(dxf::entities::Entity::new(
            dxf::entities::EntityType::LwPolyline(lwp),
        ));

        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1);

        // 1x1 inch = 25.4x25.4 mm = 645.16 mm^2
        let expected = 25.4 * 25.4;
        let area = polys[0].area();
        assert!(
            (area - expected).abs() < 1.0,
            "1x1 inch square area {} should be ~{} mm^2",
            area,
            expected
        );
    }

    #[test]
    fn test_insunits_mm_no_scale() {
        let mut drawing = dxf::Drawing::new();
        drawing.header.default_drawing_units = dxf::enums::Units::Millimeters;

        let mut lwp = dxf::entities::LwPolyline::default();
        for (x, y) in [(0.0, 0.0), (100.0, 0.0), (100.0, 50.0), (0.0, 50.0)] {
            lwp.vertices.push(dxf::LwPolylineVertex {
                x,
                y,
                ..Default::default()
            });
        }
        lwp.set_is_closed(true);
        drawing.add_entity(dxf::entities::Entity::new(
            dxf::entities::EntityType::LwPolyline(lwp),
        ));

        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1);

        let area = polys[0].area();
        assert!(
            (area - 5000.0).abs() < 1.0,
            "mm drawing should not scale: area {} should be 5000",
            area
        );
    }

    #[test]
    fn test_insunits_centimeters_scale() {
        let mut drawing = dxf::Drawing::new();
        drawing.header.default_drawing_units = dxf::enums::Units::Centimeters;

        // 10x10 cm square
        let mut lwp = dxf::entities::LwPolyline::default();
        for (x, y) in [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)] {
            lwp.vertices.push(dxf::LwPolylineVertex {
                x,
                y,
                ..Default::default()
            });
        }
        lwp.set_is_closed(true);
        drawing.add_entity(dxf::entities::Entity::new(
            dxf::entities::EntityType::LwPolyline(lwp),
        ));

        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1);

        // 10x10 cm = 100x100 mm = 10000 mm^2
        let expected = 10_000.0;
        let area = polys[0].area();
        assert!(
            (area - expected).abs() < 1.0,
            "10x10 cm square area {} should be ~{} mm^2",
            area,
            expected
        );
    }

    // ----- Line chain-linking tests -----

    fn make_line_drawing(segments: &[(f64, f64, f64, f64)]) -> dxf::Drawing {
        let mut drawing = dxf::Drawing::new();
        for &(x1, y1, x2, y2) in segments {
            let line = dxf::entities::Line {
                p1: dxf::Point::new(x1, y1, 0.0),
                p2: dxf::Point::new(x2, y2, 0.0),
                ..Default::default()
            };
            drawing.add_entity(dxf::entities::Entity::new(dxf::entities::EntityType::Line(
                line,
            )));
        }
        drawing
    }

    #[test]
    fn test_line_chain_triangle_closed() {
        // Three line segments forming a triangle (0,0)-(10,0)-(5,8)-(0,0)
        let drawing = make_line_drawing(&[
            (0.0, 0.0, 10.0, 0.0),
            (10.0, 0.0, 5.0, 8.0),
            (5.0, 8.0, 0.0, 0.0),
        ]);
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1, "Triangle should produce 1 polygon");
        // The chain has 4 points (3 segments, first == last for closed).
        // After Polygon2 construction, winding is ensured.
        assert!(
            polys[0].exterior.len() >= 3,
            "Triangle polygon should have at least 3 points, got {}",
            polys[0].exterior.len()
        );
    }

    #[test]
    fn test_line_chain_open_path() {
        // Two connected segments forming an open path: (0,0)-(5,0)-(5,5)
        let drawing = make_line_drawing(&[(0.0, 0.0, 5.0, 0.0), (5.0, 0.0, 5.0, 5.0)]);
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(
            polys.len(),
            1,
            "Connected open path should produce 1 polygon"
        );
        assert_eq!(
            polys[0].exterior.len(),
            3,
            "Open 2-segment chain should have 3 points"
        );
    }

    #[test]
    fn test_line_chain_disconnected_segments() {
        // Two disconnected segments far apart
        let drawing = make_line_drawing(&[(0.0, 0.0, 1.0, 0.0), (100.0, 100.0, 101.0, 100.0)]);
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(
            polys.len(),
            2,
            "Disconnected segments should produce 2 separate polygons"
        );
    }

    #[test]
    fn test_chain_line_segments_direct() {
        // Unit test on the chaining function itself.
        let segs = vec![
            (P2::new(0.0, 0.0), P2::new(1.0, 0.0)),
            (P2::new(2.0, 0.0), P2::new(1.0, 0.0)), // reversed segment
            (P2::new(2.0, 0.0), P2::new(3.0, 0.0)),
        ];
        let chains = chain_line_segments(&segs);
        assert_eq!(chains.len(), 1, "All segments connect into one chain");
        assert_eq!(chains[0].len(), 4, "Chain should have 4 points");
    }

    // ----- Spline tests -----

    /// Helper: build a cubic B-spline entity in a Drawing from control points and knots.
    fn make_spline_drawing(
        control_points: &[(f64, f64)],
        knots: &[f64],
        degree: i32,
        closed: bool,
    ) -> dxf::Drawing {
        let mut drawing = dxf::Drawing::new();
        let mut spline = dxf::entities::Spline {
            degree_of_curve: degree,
            knot_values: knots.to_vec(),
            ..Default::default()
        };
        for &(x, y) in control_points {
            spline.control_points.push(dxf::Point::new(x, y, 0.0));
        }
        spline.set_is_closed(closed);
        drawing.add_entity(dxf::entities::Entity::new(
            dxf::entities::EntityType::Spline(spline),
        ));
        drawing
    }

    #[test]
    fn test_spline_cubic_line() {
        // A cubic B-spline with 4 collinear control points should produce
        // a roughly straight set of points from (0,0) to (3,0).
        let cp = [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0)];
        let knots = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let drawing = make_spline_drawing(&cp, &knots, 3, false);
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1, "Spline should produce 1 polygon");
        let ext = &polys[0].exterior;
        assert!(ext.len() >= 2, "Spline should produce at least 2 points");

        // All points should have y ≈ 0 (collinear control points).
        for pt in ext {
            assert!(
                pt.y.abs() < 0.01,
                "Collinear cubic spline point should have y~0, got {}",
                pt.y
            );
        }
        // First point near (0,0), last near (3,0).
        assert!(
            (ext[0].x).abs() < 0.01,
            "Start x should be ~0, got {}",
            ext[0].x
        );
        assert!(
            (ext[ext.len() - 1].x - 3.0).abs() < 0.01,
            "End x should be ~3, got {}",
            ext[ext.len() - 1].x
        );
    }

    #[test]
    fn test_spline_closed_square_like() {
        // Build a closed cubic B-spline that approximates a loop.
        // Use 7 control points (4 unique + 3 overlapping for closure)
        // with a uniform knot vector.
        let cp = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),   // overlap to close
            (10.0, 0.0),  // overlap
            (10.0, 10.0), // overlap
        ];
        // Uniform knots for 7 control points, degree 3: length = 7+3+1 = 11
        let knots = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let drawing = make_spline_drawing(&cp, &knots, 3, true);
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1, "Closed spline should produce 1 polygon");
        let ext = &polys[0].exterior;
        // The first and last evaluated points should be near each other
        // (spline is periodic / closed).
        let first = ext[0];
        let last = ext[ext.len() - 1];
        let dist = ((first.x - last.x).powi(2) + (first.y - last.y).powi(2)).sqrt();
        assert!(
            dist < 1.0,
            "Closed spline endpoints should be near each other, distance = {}",
            dist
        );
    }

    #[test]
    fn test_de_boor_eval_basic() {
        // Degree-1 (linear) B-spline = polyline interpolation.
        // 3 control points, knots = [0,0,0.5,1,1]
        let cp = [P2::new(0.0, 0.0), P2::new(5.0, 10.0), P2::new(10.0, 0.0)];
        let knots = [0.0, 0.0, 0.5, 1.0, 1.0];
        // At t=0 -> first control point
        let p0 = de_boor_eval(1, &knots, &cp, 0.0).unwrap();
        assert!((p0.x).abs() < 0.01 && (p0.y).abs() < 0.01, "t=0 -> (0,0)");
        // At t=0.5 -> second control point
        let p1 = de_boor_eval(1, &knots, &cp, 0.5).unwrap();
        assert!(
            (p1.x - 5.0).abs() < 0.01 && (p1.y - 10.0).abs() < 0.01,
            "t=0.5 -> (5,10), got ({}, {})",
            p1.x,
            p1.y
        );
        // At t=1.0 -> last control point
        let p2 = de_boor_eval(1, &knots, &cp, 1.0).unwrap();
        assert!(
            (p2.x - 10.0).abs() < 0.01 && (p2.y).abs() < 0.01,
            "t=1 -> (10,0), got ({}, {})",
            p2.x,
            p2.y
        );
    }
}
