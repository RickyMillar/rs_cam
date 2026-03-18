//! DXF file input — extracts closed entities as Polygon2.
//!
//! Supports LwPolyline, Polyline, Circle, and Ellipse entities.
//! Arc segments (bulge values) are tessellated to line segments.

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

/// Load closed polygon entities from a DXF file.
///
/// Arc segments (bulge values in polylines, circles, ellipses) are
/// tessellated to line segments with the given angular tolerance in degrees.
pub fn load_dxf(path: &Path, arc_tolerance_deg: f64) -> Result<Vec<Polygon2>, DxfError> {
    let drawing = dxf::Drawing::load_file(path.to_str().unwrap_or(""))?;
    Ok(extract_polygons(&drawing, arc_tolerance_deg))
}

/// Load closed polygon entities from a DXF Drawing.
pub fn extract_polygons(drawing: &dxf::Drawing, arc_tolerance_deg: f64) -> Vec<Polygon2> {
    let mut polygons = Vec::new();
    let arc_step_rad = arc_tolerance_deg.to_radians();

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
            _ => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lwpolyline_drawing(
        vertices: Vec<(f64, f64, f64)>,
        closed: bool,
    ) -> dxf::Drawing {
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
        let mut circle = dxf::entities::Circle::default();
        circle.center = dxf::Point::new(cx, cy, 0.0);
        circle.radius = radius;
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
            vec![
                (0.0, 0.0, 0.0),
                (100.0, 0.0, 0.0),
                (100.0, 50.0, 0.0),
            ],
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
        let mut circle = dxf::entities::Circle::default();
        circle.center = dxf::Point::new(50.0, 50.0, 0.0);
        circle.radius = 20.0;
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
        let drawing = make_lwpolyline_drawing(
            vec![(0.0, 0.0, 0.0), (10.0, 0.0, 0.0)],
            true,
        );
        let polys = extract_polygons(&drawing, 5.0);
        assert!(polys.is_empty(), "2-vertex polyline should be ignored");
    }
}
