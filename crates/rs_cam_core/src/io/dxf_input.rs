//! DXF file input — extracts entities as selection items (P2.1).
//!
//! Each entity gives one item with its layer: a closed entity a `Loop`, a
//! CIRCLE or full ARC one `Circle`, a POINT a `Point`. A chain of open
//! pieces gives a `Loop` or a `Path` with the layer of its pieces.
//!
//! ### Supported entity types
//! - **LwPolyline** — closed lightweight polylines (with optional bulge arcs)
//! - **Polyline** — closed legacy polylines (with optional bulge arcs)
//! - **Circle** — tessellated to a closed polygon
//! - **Ellipse** — a full ellipse is a closed polygon; a partial ellipse is an
//!   open path (P1: no chord joins its ends)
//! - **Arc** — full-circle arcs become circles; a partial arc is an open path
//!   (P1: no chord joins its ends)
//! - **Line** — a 2-point segment, an open piece
//! - **Spline** — B-spline curves evaluated via De Boor's algorithm; both closed
//!   and open splines are returned
//!
//! ### Chaining of open pieces (P1b)
//! A CAD export often draws one outline as separate entities: LINE and ARC
//! for a rounded rectangle, or open polylines, splines and partial ellipses.
//! The importer joins every OPEN piece (LINE, partial ARC, partial ELLIPSE,
//! SPLINE, open LWPOLYLINE and POLYLINE) end to end where two ends meet
//! within the join tolerance (default [`CLOSE_TOL_MM`]). A piece may join
//! reversed. A chain that comes back to its start is a closed polygon;
//! other chains are open paths. A chain holds the pieces of one layer only
//! (DESIGN D14). P4.8 moved the join to [`crate::selection::join`], so the
//! SVG door uses the same rule; see
//! [`chain_open_pieces`](crate::selection::join::chain_open_pieces) for the
//! order and the branch rule. The door returns the counts of the join in
//! its [`ImportReport`].
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
use crate::io::{DrawingImport, ImportReport};
use crate::polygon::Polygon2;
#[cfg(test)]
use crate::selection::join::{CLOSE_TOL_MM, chain_open_pieces};
use crate::selection::join::{OpenPiece, join_open_pieces};
use crate::selection::{Item, ItemError, ItemSet, Shape, SourceRef};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DxfError {
    #[error("Failed to read DXF file: {0}")]
    Io(#[from] dxf::DxfError),
    #[error("No closed entities found in DXF")]
    NoEntities,
    /// The file is over [`crate::io::MAX_IMPORT_FILE_SIZE`]. The door
    /// refuses before the parser reads a byte (EDG-03).
    #[error("DXF file too large ({size_mb} MB, limit {limit_mb} MB)")]
    FileTooLarge { size_mb: u64, limit_mb: u64 },
    /// A shape that breaks an item invariant: a defect in this door.
    #[error("DXF import made an invalid item: {0}")]
    Item(#[from] ItemError),
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

/// Load a DXF file as a [`DrawingImport`]: one item per entity and per
/// chain, in entity order (a chain at the lowest index of its pieces), and
/// the report of the join.
///
/// Arc segments (bulge values in polylines, circles, ellipses) are
/// tessellated to line segments with the given angular tolerance in degrees.
/// Coordinates are scaled from `$INSUNITS` to millimeters automatically.
/// `join_tolerance_mm` is the join tolerance in mm of the `$INSUNITS`
/// frame (P4.8; the default is [`CLOSE_TOL_MM`](crate::selection::join::CLOSE_TOL_MM)).
pub fn load_dxf(
    path: &Path,
    arc_tolerance_deg: f64,
    join_tolerance_mm: f64,
) -> Result<DrawingImport, DxfError> {
    if let Some(over) = crate::io::file_size_over_limit(path, crate::io::MAX_IMPORT_FILE_SIZE)
        .map_err(|e| DxfError::Io(e.into()))?
    {
        return Err(DxfError::FileTooLarge {
            size_mb: over.size_mb,
            limit_mb: over.limit_mb,
        });
    }
    let drawing = dxf::Drawing::load_file(path.to_str().unwrap_or(""))?;
    extract_dxf(&drawing, arc_tolerance_deg, join_tolerance_mm)
}

/// The geometry of one entity or chain, in drawing units, before it becomes
/// an item.
enum RawShape {
    /// A closed ring (wound CCW) or an open path.
    Ring(Polygon2),
    /// A CIRCLE or a full-circle ARC: its centre, diameter and ring.
    Circle {
        centre: P2,
        diameter: f64,
        ring: Polygon2,
    },
    /// A POINT.
    Point(P2),
}

/// Extract the items of a DXF Drawing, scaled to mm.
///
/// Containment does not run here (P2.1): the nesting runs over the selected
/// loops at resolution.
pub fn extract_dxf(
    drawing: &dxf::Drawing,
    arc_tolerance_deg: f64,
    join_tolerance_mm: f64,
) -> Result<DrawingImport, DxfError> {
    let scale = insunits_to_mm_scale(drawing.header.default_drawing_units);
    let (mut raw, layers) = extract_flat(drawing, arc_tolerance_deg, scale, join_tolerance_mm);
    if (scale - 1.0).abs() > 1e-12 {
        let scale_ring = |poly: &mut Polygon2| {
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
            // `exterior` moved — see the mutation contract on `Polygon2`.
            poly.invalidate_bbox();
        };
        for (_, _, shape) in &mut raw {
            match shape {
                RawShape::Ring(poly) => scale_ring(poly),
                RawShape::Circle {
                    centre,
                    diameter,
                    ring,
                } => {
                    centre.x *= scale;
                    centre.y *= scale;
                    *diameter *= scale;
                    scale_ring(ring);
                }
                RawShape::Point(p) => {
                    p.x *= scale;
                    p.y *= scale;
                }
            }
        }
    }
    let items = raw
        .into_iter()
        .map(|(_, layer, shape)| {
            let shape = match shape {
                RawShape::Ring(poly) if poly.closed => Shape::Loop(poly),
                RawShape::Ring(poly) => Shape::Path(poly),
                RawShape::Circle {
                    centre,
                    diameter,
                    ring,
                } => Shape::Circle {
                    centre,
                    diameter,
                    ring,
                },
                RawShape::Point(p) => Shape::Point(p),
            };
            Item::new(SourceRef::ToolpathModel, Some(Arc::from(layer)), shape)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DrawingImport {
        items: ItemSet::new(items),
        report: ImportReport {
            join_tolerance_mm,
            layers,
        },
    })
}

/// Extract the raw shapes as one flat list, each with its entity index and
/// layer.
///
/// `scale` is the `$INSUNITS` factor to mm. The coordinates here are still
/// in drawing units, so the chaining tolerance is `join_tolerance_mm / scale`.
/// The second value is the join report, one row per layer.
///
/// The shapes come out in entity order: an entity at its own index, a chain
/// at the lowest index of its pieces. A closed polyline of fewer than 3
/// points is an open path of its points (DESIGN 2.2; before P2.1 the import
/// dropped it in silence).
fn extract_flat(
    drawing: &dxf::Drawing,
    arc_tolerance_deg: f64,
    scale: f64,
    join_tolerance_mm: f64,
) -> (
    Vec<(usize, String, RawShape)>,
    Vec<crate::selection::join::LayerJoin>,
) {
    let mut shapes: Vec<(usize, String, RawShape)> = Vec::new();
    let arc_step_rad = arc_tolerance_deg.to_radians();

    // The open pieces, for the chaining step after the main loop.
    let mut pieces: Vec<OpenPiece> = Vec::new();

    for (index, entity) in drawing.entities().enumerate() {
        let layer = entity.common.layer.clone();
        let mut open_piece = |pts: Vec<P2>| {
            pieces.push(OpenPiece {
                index,
                layer: Some(Arc::from(layer.as_str())),
                pts,
            });
        };
        let closed_ring = |pts: Vec<P2>, shapes: &mut Vec<(usize, String, RawShape)>| {
            if pts.len() >= 3 {
                let mut poly = Polygon2::new(pts);
                poly.ensure_winding();
                shapes.push((index, layer.clone(), RawShape::Ring(poly)));
            } else {
                // A closed entity of 2 points holds no area: it is the
                // line between them.
                shapes.push((
                    index,
                    layer.clone(),
                    RawShape::Ring(Polygon2::open_path(pts)),
                ));
            }
        };
        match &entity.specific {
            dxf::entities::EntityType::LwPolyline(lwp) => {
                if lwp.vertices.len() >= 2 {
                    let pts = lwpolyline_to_points(lwp, arc_step_rad);
                    if lwp.is_closed() {
                        closed_ring(pts, &mut shapes);
                    } else if pts.len() >= 2 {
                        // An open polyline is an open piece: it can join
                        // other pieces of its layer.
                        open_piece(pts);
                    }
                }
            }
            dxf::entities::EntityType::Polyline(poly_ent) => {
                let verts: Vec<_> = poly_ent.vertices().collect();
                if verts.len() >= 2 {
                    let pts = polyline_to_points(&verts, arc_step_rad);
                    if poly_ent.is_closed() {
                        closed_ring(pts, &mut shapes);
                    } else if pts.len() >= 2 {
                        open_piece(pts);
                    }
                }
            }
            dxf::entities::EntityType::Circle(circle) => {
                // A circle is ONE item: a closed region (for pocket and
                // profile) and a drillable hole.
                let pts = circle_to_points(
                    circle.center.x,
                    circle.center.y,
                    circle.radius,
                    arc_step_rad,
                );
                let mut ring = Polygon2::new(pts);
                ring.ensure_winding();
                shapes.push((
                    index,
                    layer.clone(),
                    RawShape::Circle {
                        centre: P2::new(circle.center.x, circle.center.y),
                        diameter: 2.0 * circle.radius,
                        ring,
                    },
                ));
            }
            dxf::entities::EntityType::ModelPoint(mp) => {
                // A bare DXF POINT entity marks a location — a drill target.
                shapes.push((
                    index,
                    layer.clone(),
                    RawShape::Point(P2::new(mp.location.x, mp.location.y)),
                ));
            }
            dxf::entities::EntityType::Ellipse(ell) => {
                // P1: a partial ellipse is an open piece. A chord from end
                // to start would cut a line that the drawing does not hold.
                let (pts, closed) = ellipse_to_points(ell, arc_step_rad);
                if closed && pts.len() >= 3 {
                    let mut poly = Polygon2::new(pts);
                    poly.ensure_winding();
                    shapes.push((index, layer.clone(), RawShape::Ring(poly)));
                } else if !closed && pts.len() >= 2 {
                    open_piece(pts);
                }
            }
            dxf::entities::EntityType::Arc(arc) => {
                let pts = arc_entity_to_points(arc, arc_step_rad);
                if arc_is_full_circle(arc) {
                    // A full-circle arc is a circle: one item.
                    let mut ring = Polygon2::new(pts);
                    ring.ensure_winding();
                    shapes.push((
                        index,
                        layer.clone(),
                        RawShape::Circle {
                            centre: P2::new(arc.center.x, arc.center.y),
                            diameter: 2.0 * arc.radius,
                            ring,
                        },
                    ));
                } else if pts.len() >= 2 {
                    // P1: a partial arc is an open piece. A chord from end
                    // to start would cut a line that the drawing does not
                    // hold.
                    open_piece(pts);
                }
            }
            dxf::entities::EntityType::Line(line) => {
                let p1 = P2::new(line.p1.x, line.p1.y);
                let p2 = P2::new(line.p2.x, line.p2.y);
                open_piece(vec![p1, p2]);
            }
            dxf::entities::EntityType::Spline(spline) => {
                let pts = spline_to_points(spline, arc_step_rad);
                if pts.len() >= 2 {
                    // A spline carries no closed flag that the importer
                    // reads. The chaining step decides from the geometry:
                    // an open river stays open, so project_curve traces no
                    // phantom segment from end to start across the stock.
                    open_piece(pts);
                }
            }
            _ => {}
        }
    }

    // Join the open pieces. A chain that does not come back to its start
    // is an open path; a chord there would make project_curve trace a
    // phantom line across the whole chain.
    let tol = join_tolerance_mm / scale;
    let (joined, layers) = join_open_pieces(&pieces, tol);
    for shape in joined {
        // Every DXF piece has a layer (the entity's).
        let layer = shape.layer.as_deref().unwrap_or_default().to_owned();
        shapes.push((shape.index, layer, RawShape::Ring(shape.polygon)));
    }
    // Entity order. The sort is stable and the indices are distinct.
    shapes.sort_by_key(|(index, _, _)| *index);
    (shapes, layers)
}

/// True if a DXF Arc spans a full circle (sweep ~360°), in which case its
/// centre is a drillable hole just like a Circle entity.
fn arc_is_full_circle(arc: &dxf::entities::Arc) -> bool {
    let mut sweep_deg = arc.end_angle - arc.start_angle;
    if sweep_deg <= 0.0 {
        sweep_deg += 360.0;
    }
    (sweep_deg - 360.0).abs() < 0.01
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
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

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
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

/// Tessellate a DXF Arc entity into points.
///
/// DXF Arc angles are in degrees, measured CCW from the +X axis.
/// A partial arc gives its points from the start angle to the end angle,
/// both included. The caller imports them as an open path (P1): no chord
/// joins the two ends. A full-circle arc (start == end or sweep ~360) gives
/// the points of a Circle entity, which the caller imports as a closed ring.
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

/// Tessellate a DXF Ellipse entity. The flag is `true` for a full ellipse
/// (a closed ring) and `false` for a partial one (an open path from the
/// start parameter to the end parameter, both included, P1).
///
/// DXF ellipse parameters run CCW, so a partial ellipse whose end parameter
/// is below its start parameter wraps through 0.
fn ellipse_to_points(ell: &dxf::entities::Ellipse, arc_step: f64) -> (Vec<P2>, bool) {
    let cx = ell.center.x;
    let cy = ell.center.y;
    let major_x = ell.major_axis.x;
    let major_y = ell.major_axis.y;
    let ratio = ell.minor_axis_ratio;

    // Full ellipse: start=0, end=2*pi (or start == end).
    let start = ell.start_parameter;
    let end = ell.end_parameter;
    let raw = end - start;
    let full = raw.abs() < 1e-10 || (raw.abs() - std::f64::consts::TAU).abs() < 1e-9;
    let sweep = if full {
        std::f64::consts::TAU
    } else if raw < 0.0 {
        raw + std::f64::consts::TAU
    } else {
        raw
    };

    let n = (sweep.abs() / arc_step).ceil() as usize;
    let n = n.max(8);

    // Major axis angle
    let major_len = (major_x * major_x + major_y * major_y).sqrt();
    let major_angle = major_y.atan2(major_x);

    let a = major_len; // semi-major
    let b = major_len * ratio; // semi-minor

    // A full ring leaves out the end point (it is the start point); an open
    // path keeps it.
    let last = if full { n } else { n + 1 };
    let pts = (0..last)
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
        .collect();
    (pts, full)
}

// ---------------------------------------------------------------------------
// Spline evaluation (De Boor's algorithm)
// ---------------------------------------------------------------------------

/// Evaluate a B-spline at parameter `t` using De Boor's algorithm.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
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

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
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
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// The items of a drawing.
    fn items_of(drawing: &dxf::Drawing) -> ItemSet {
        extract_dxf(drawing, 5.0, CLOSE_TOL_MM).unwrap().items
    }

    /// The polygon list of the import before P2.1 (the bridge): the drawn
    /// rings, nested.
    fn extract_polygons(drawing: &dxf::Drawing, arc_tolerance_deg: f64) -> Vec<Polygon2> {
        extract_dxf(drawing, arc_tolerance_deg, CLOSE_TOL_MM)
            .unwrap()
            .items
            .outlines_pre_selection()
            .as_ref()
            .clone()
    }

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
    fn test_lwpolyline_open_imported() {
        let drawing = make_lwpolyline_drawing(
            vec![(0.0, 0.0, 0.0), (100.0, 0.0, 0.0), (100.0, 50.0, 0.0)],
            false,
        );
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(
            polys.len(),
            1,
            "Open polyline should be imported as unclosed path"
        );
        assert_eq!(polys[0].exterior.len(), 3);
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

    // ----- Circles and points (P2.1: one item each) -----

    #[test]
    fn a_circle_is_one_item() {
        let drawing = make_circle_drawing(50.0, 60.0, 4.0);
        let items = items_of(&drawing);
        assert_eq!(
            items.len(),
            1,
            "a circle is one item, not a loop and a target"
        );
        match items.iter().next().unwrap().shape() {
            Shape::Circle {
                centre, diameter, ..
            } => {
                assert!((centre.x - 50.0).abs() < 1e-9 && (centre.y - 60.0).abs() < 1e-9);
                assert!((diameter - 8.0).abs() < 1e-9, "diameter should be 2*radius");
            }
            other => panic!("expected a circle, got {other:?}"),
        }
    }

    #[test]
    fn a_point_entity_is_a_point_item() {
        let mut drawing = dxf::Drawing::new();
        let mut ent = dxf::entities::Entity::new(dxf::entities::EntityType::ModelPoint(
            dxf::entities::ModelPoint::new(dxf::Point::new(12.0, 34.0, 0.0)),
        ));
        ent.common.layer = "holes".to_owned();
        drawing.add_entity(ent);
        let items = items_of(&drawing);
        assert_eq!(items.len(), 1);
        let item = items.iter().next().unwrap();
        assert_eq!(item.layer(), Some("holes"));
        match item.shape() {
            Shape::Point(p) => assert!((p.x - 12.0).abs() < 1e-9 && (p.y - 34.0).abs() < 1e-9),
            other => panic!("expected a point, got {other:?}"),
        }
        assert_eq!(items.layers(), vec!["holes".to_owned()]);
        // A bare POINT is not a ring, so the polygon list is empty.
        assert!(extract_polygons(&drawing, 5.0).is_empty());
    }

    #[test]
    fn item_layers_are_collected_and_items_scaled() {
        let mut drawing = dxf::Drawing::new();
        drawing.header.default_drawing_units = dxf::enums::Units::Inches;
        // Two circles on layer "a", one POINT on layer "b".
        for (cx, cy, layer) in [(1.0, 0.0, "a"), (2.0, 0.0, "a")] {
            let mut ent = dxf::entities::Entity::new(dxf::entities::EntityType::Circle(
                dxf::entities::Circle {
                    center: dxf::Point::new(cx, cy, 0.0),
                    radius: 0.5,
                    ..Default::default()
                },
            ));
            ent.common.layer = layer.to_owned();
            drawing.add_entity(ent);
        }
        let mut pt = dxf::entities::Entity::new(dxf::entities::EntityType::ModelPoint(
            dxf::entities::ModelPoint::new(dxf::Point::new(3.0, 0.0, 0.0)),
        ));
        pt.common.layer = "b".to_owned();
        drawing.add_entity(pt);

        let items = items_of(&drawing);
        assert_eq!(items.len(), 3);
        assert_eq!(items.layers(), vec!["a".to_owned(), "b".to_owned()]);
        // Inches → mm: x of first circle 1.0in = 25.4mm; diameter 1.0in = 25.4mm.
        match items.iter().next().unwrap().shape() {
            Shape::Circle {
                centre, diameter, ..
            } => {
                assert!((centre.x - 25.4).abs() < 1e-6, "x should scale to mm");
                assert!(
                    (diameter - 25.4).abs() < 1e-6,
                    "diameter should scale to mm"
                );
            }
            other => panic!("expected a circle, got {other:?}"),
        }
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

    /// DESIGN 2.2: a closed polyline of 2 points is the line between them,
    /// kept as a 2-point path. Before P2.1 the import dropped it in silence.
    #[test]
    fn a_closed_two_point_polyline_is_a_path_p2_1() {
        let drawing = make_lwpolyline_drawing(vec![(0.0, 0.0, 0.0), (10.0, 0.0, 0.0)], true);
        let items = items_of(&drawing);
        assert_eq!(items.len(), 1, "the 2-vertex polyline is kept");
        let item = items.iter().next().unwrap();
        assert_eq!(item.kind(), crate::selection::ItemKind::Path);
        match item.shape() {
            Shape::Path(p) => assert_eq!(p.exterior.len(), 2),
            other => panic!("expected a path, got {other:?}"),
        }
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

    /// Every point of `poly` lies on the circle (`cx`, `cy`, `r`).
    fn on_circle(poly: &Polygon2, cx: f64, cy: f64, r: f64) -> bool {
        poly.exterior
            .iter()
            .all(|p| ((p.x - cx).hypot(p.y - cy) - r).abs() < 1e-9)
    }

    /// P1 sentry (e): a partial DXF ARC imports as an OPEN path from its
    /// start angle to its end angle. No chord joins the two ends.
    #[test]
    fn a_partial_dxf_arc_imports_open_p1() {
        for (start, end, sweep_deg) in
            [(0.0, 180.0, 180.0), (0.0, 90.0, 90.0), (270.0, 90.0, 180.0)]
        {
            let drawing = make_arc_drawing(0.0, 0.0, 10.0, start, end);
            let polys = extract_polygons(&drawing, 5.0);
            assert_eq!(polys.len(), 1, "arc {start}..{end}: one path");
            let path = &polys[0];
            assert!(!path.closed, "arc {start}..{end}: a partial arc is open");
            assert!(path.holes.is_empty());
            assert!(path.exterior.len() >= 3, "arc {start}..{end}: tessellated");
            assert!(on_circle(path, 0.0, 0.0, 10.0), "arc {start}..{end}");
            let first = path.exterior[0];
            let last = *path.exterior.last().unwrap();
            let at =
                |deg: f64| P2::new(10.0 * deg.to_radians().cos(), 10.0 * deg.to_radians().sin());
            assert!((first - at(start)).norm() < 1e-9, "starts at {start} deg");
            assert!((last - at(end)).norm() < 1e-9, "ends at {end} deg");
            // The open length is the tessellated arc length. A closing
            // chord would add the distance from the last point to the first.
            let open_len: f64 = path.exterior.windows(2).map(|w| (w[1] - w[0]).norm()).sum();
            let arc_len = 10.0 * f64::to_radians(sweep_deg);
            assert!(
                open_len <= arc_len + 1e-9 && open_len > 0.98 * arc_len,
                "arc {start}..{end}: open length {open_len} vs arc {arc_len}"
            );
        }
    }

    /// P1: a partial DXF ELLIPSE imports as an open path too.
    #[test]
    fn a_partial_dxf_ellipse_imports_open_p1() {
        let mut drawing = dxf::Drawing::new();
        let mut ell = dxf::entities::Ellipse {
            center: dxf::Point::new(0.0, 0.0, 0.0),
            major_axis: dxf::Vector::new(20.0, 0.0, 0.0),
            minor_axis_ratio: 0.5,
            start_parameter: 0.0,
            end_parameter: std::f64::consts::PI,
            ..Default::default()
        };
        drawing.add_entity(dxf::entities::Entity::new(
            dxf::entities::EntityType::Ellipse(ell.clone()),
        ));
        let polys = extract_polygons(&drawing, 5.0);
        assert_eq!(polys.len(), 1);
        assert!(!polys[0].closed, "a half ellipse is open");
        let last = *polys[0].exterior.last().unwrap();
        assert!((last - P2::new(-20.0, 0.0)).norm() < 1e-9, "ends at t = pi");

        // A full ellipse stays a closed ring.
        ell.end_parameter = std::f64::consts::TAU;
        let mut full = dxf::Drawing::new();
        full.add_entity(dxf::entities::Entity::new(
            dxf::entities::EntityType::Ellipse(ell),
        ));
        let polys = extract_polygons(&full, 5.0);
        assert_eq!(polys.len(), 1);
        assert!(polys[0].closed, "a full ellipse is closed");
        let expected = std::f64::consts::PI * 20.0 * 10.0;
        assert!((polys[0].area() - expected).abs() < expected * 0.05);
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

    /// An open piece of layer `layer` with entity index `index`.
    fn piece(index: usize, layer: &str, pts: &[(f64, f64)]) -> OpenPiece {
        OpenPiece {
            index,
            layer: Some(Arc::from(layer)),
            pts: pts.iter().map(|&(x, y)| P2::new(x, y)).collect(),
        }
    }

    #[test]
    fn test_chain_open_pieces_direct() {
        // Unit test on the chaining function itself.
        let pieces = vec![
            piece(0, "0", &[(0.0, 0.0), (1.0, 0.0)]),
            piece(1, "0", &[(2.0, 0.0), (1.0, 0.0)]), // reversed segment
            piece(2, "0", &[(2.0, 0.0), (3.0, 0.0)]),
        ];
        let chains = chain_open_pieces(&pieces, CLOSE_TOL_MM);
        assert_eq!(chains.len(), 1, "All segments connect into one chain");
        assert_eq!(chains[0].index, 0, "the chain carries its lowest index");
        assert_eq!(
            chains[0].layer.as_deref(),
            Some("0"),
            "the chain carries its layer"
        );
        assert_eq!(chains[0].pts.len(), 4, "Chain should have 4 points");
    }

    // ----- P1b: chaining of open pieces -----

    /// One piece of a P1b drawing.
    enum Draw {
        /// A LINE from (x1, y1) to (x2, y2).
        Line(f64, f64, f64, f64),
        /// An ARC: centre (cx, cy), radius, start and end angle in degrees.
        Arc(f64, f64, f64, f64, f64),
    }

    /// A drawing of `items` in this order, each on its layer.
    fn drawing_of(items: &[(&str, Draw)]) -> dxf::Drawing {
        let mut drawing = dxf::Drawing::new();
        for (layer, item) in items {
            let specific = match *item {
                Draw::Line(x1, y1, x2, y2) => {
                    dxf::entities::EntityType::Line(dxf::entities::Line {
                        p1: dxf::Point::new(x1, y1, 0.0),
                        p2: dxf::Point::new(x2, y2, 0.0),
                        ..Default::default()
                    })
                }
                Draw::Arc(cx, cy, r, a0, a1) => dxf::entities::EntityType::Arc(
                    dxf::entities::Arc::new(dxf::Point::new(cx, cy, 0.0), r, a0, a1),
                ),
            };
            let mut entity = dxf::entities::Entity::new(specific);
            entity.common.layer = (*layer).to_owned();
            drawing.add_entity(entity);
        }
        drawing
    }

    /// The rounded rectangle of the P1b sentry: 40 x 20 mm, corner radius
    /// 5 mm, its lower left corner at the origin. The entities are in a
    /// mixed order. The bottom and the top LINE run clockwise and every ARC
    /// runs counter-clockwise (DXF), so the chain takes some pieces
    /// reversed: with the seed (the top-left ARC) the bottom and the top
    /// LINE join reversed.
    fn rounded_rectangle() -> Vec<(&'static str, Draw)> {
        vec![
            ("0", Draw::Arc(5.0, 15.0, 5.0, 90.0, 180.0)),
            ("0", Draw::Line(35.0, 0.0, 5.0, 0.0)),
            ("0", Draw::Arc(35.0, 5.0, 5.0, 270.0, 360.0)),
            ("0", Draw::Line(0.0, 15.0, 0.0, 5.0)),
            ("0", Draw::Line(5.0, 20.0, 35.0, 20.0)),
            ("0", Draw::Arc(35.0, 15.0, 5.0, 0.0, 90.0)),
            ("0", Draw::Line(40.0, 5.0, 40.0, 15.0)),
            ("0", Draw::Arc(5.0, 5.0, 5.0, 180.0, 270.0)),
        ]
    }

    /// P1b sentry: a rounded rectangle drawn as 4 LINE + 4 ARC imports as
    /// ONE closed polygon with the area of the tessellated outline.
    ///
    /// Red proof (2026-10-19): with the ARC pieces kept out of the chainer
    /// (imported as open paths of their own, as after P1), this test fails:
    /// the import holds 8 open paths, not one closed polygon.
    #[test]
    fn a_rounded_rectangle_of_lines_and_arcs_imports_closed_p1b() {
        let arc_tol_deg = 5.0;
        let polys = extract_polygons(&drawing_of(&rounded_rectangle()), arc_tol_deg);
        assert_eq!(polys.len(), 1, "one outline: {} polygons", polys.len());
        let outline = &polys[0];
        assert!(outline.closed, "the outline is closed");
        assert!(outline.holes.is_empty());

        // `arc_entity_to_points` cuts a 90 degree arc into
        // n = ceil(90 / 5) chords. The polygon area is the rectangle less,
        // at each corner, the r x r square less the fan of n triangles
        // from the arc centre: n * r^2 / 2 * sin(90 deg / n).
        let (w, h, r) = (40.0_f64, 20.0_f64, 5.0_f64);
        let n = (90.0_f64 / arc_tol_deg).ceil();
        let fan = n * r * r / 2.0 * (std::f64::consts::FRAC_PI_2 / n).sin();
        let expected = w * h - 4.0 * (r * r - fan);
        let area = outline.area().abs();
        assert!(
            (area - expected).abs() < 1e-9 * expected,
            "area {area} vs the tessellated outline {expected}"
        );
    }

    /// P1b sentry: an open L of a LINE and an ARC stays ONE open path.
    #[test]
    fn an_open_l_of_a_line_and_an_arc_stays_one_open_path_p1b() {
        let polys = extract_polygons(
            &drawing_of(&[
                ("0", Draw::Line(0.0, 0.0, 10.0, 0.0)),
                ("0", Draw::Arc(10.0, 5.0, 5.0, 270.0, 360.0)),
            ]),
            5.0,
        );
        assert_eq!(polys.len(), 1, "one path");
        let path = &polys[0];
        assert!(!path.closed, "the L is open");
        assert!((path.exterior[0] - P2::new(0.0, 0.0)).norm() < 1e-9);
        assert!((*path.exterior.last().unwrap() - P2::new(15.0, 5.0)).norm() < 1e-9);
    }

    /// P1b sentry (DESIGN D14): two pieces that touch at a point but lie
    /// on two layers do not chain. On one layer they do.
    #[test]
    fn pieces_on_two_layers_do_not_chain_p1b() {
        let two = extract_polygons(
            &drawing_of(&[
                ("A", Draw::Line(0.0, 0.0, 10.0, 0.0)),
                ("B", Draw::Line(10.0, 0.0, 10.0, 10.0)),
            ]),
            5.0,
        );
        assert_eq!(two.len(), 2, "two layers: two paths");
        assert!(two.iter().all(|p| !p.closed && p.exterior.len() == 2));

        let one = extract_polygons(
            &drawing_of(&[
                ("A", Draw::Line(0.0, 0.0, 10.0, 0.0)),
                ("A", Draw::Line(10.0, 0.0, 10.0, 10.0)),
            ]),
            5.0,
        );
        assert_eq!(one.len(), 1, "one layer: one path");
    }

    /// P1b sentry: a piece joins reversed (its end meets the chain end),
    /// and an end within `CLOSE_TOL_MM` meets while one outside it does not.
    #[test]
    fn a_reversed_piece_joins_within_the_closing_tolerance_p1b() {
        let joined = extract_polygons(
            &drawing_of(&[
                ("0", Draw::Line(0.0, 0.0, 10.0, 0.0)),
                ("0", Draw::Line(20.0, 0.0, 10.0 + 0.5 * CLOSE_TOL_MM, 0.0)),
            ]),
            5.0,
        );
        assert_eq!(joined.len(), 1, "the reversed line joins");
        let path = &joined[0];
        assert_eq!(path.exterior.len(), 3, "the joint point is not repeated");
        assert!((*path.exterior.last().unwrap() - P2::new(20.0, 0.0)).norm() < 1e-9);

        let apart = extract_polygons(
            &drawing_of(&[
                ("0", Draw::Line(0.0, 0.0, 10.0, 0.0)),
                ("0", Draw::Line(20.0, 0.0, 10.0 + 2.0 * CLOSE_TOL_MM, 0.0)),
            ]),
            5.0,
        );
        assert_eq!(apart.len(), 2, "a gap over the tolerance does not join");
    }

    /// P1b: at a branch point (three ends at one point) the piece of the
    /// lowest entity index joins, and the others start chains of their own.
    /// The output is in entity order.
    #[test]
    fn a_branch_point_takes_the_lowest_entity_index_p1b() {
        let polys = extract_polygons(
            &drawing_of(&[
                ("0", Draw::Line(0.0, 0.0, 10.0, 0.0)),
                ("0", Draw::Line(10.0, 0.0, 10.0, 10.0)),
                ("0", Draw::Line(10.0, 0.0, 20.0, 0.0)),
            ]),
            5.0,
        );
        assert_eq!(polys.len(), 2);
        let first: Vec<P2> = polys[0].exterior.clone();
        assert_eq!(
            first,
            vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0), P2::new(10.0, 10.0)],
            "the seed takes entity 1 at the branch"
        );
        assert_eq!(
            polys[1].exterior,
            vec![P2::new(10.0, 0.0), P2::new(20.0, 0.0)]
        );
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
