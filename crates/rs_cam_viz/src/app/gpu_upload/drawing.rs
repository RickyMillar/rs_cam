//! The drawing lines: the polygon, DXF and SVG items of the drawn models.
//!
//! Unkeyed: a cheap rebuild on every pass. The drawing of the vector-source
//! toolpath's model takes its item state colours; every other visible
//! drawing draws neutral (P4.D).
//!
//! Each model's polygons are drawn using the transform of whichever setup
//! actually references the model via a toolpath. This matters for
//! multi-setup projects: a DXF used by a face=Bottom setup must flip onto
//! the bottom in the sim view even when the top setup is selected (or
//! nothing is selected in Simulation mode). Falls back to the
//! selection-based active setup frame for unowned models.

use crate::render::RenderResources;
use crate::state::job::{SetupFrame, transform_mesh};
use crate::state::{AppState, Workspace};

use super::{Gpu, PassFrame};

/// Rebuild `resources.polygon_data`.
pub(super) fn upload(
    state: &AppState,
    frame: &PassFrame,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    use crate::render::{LineVertex, PolygonGpuData};
    use egui_wgpu::wgpu::util::DeviceExt;
    let stock = frame.stock();
    resources.polygon_data.clear();

    // When a drill op is selected, draw its model's drill targets
    // (DXF points / circle centres) as pickable markers — bright when
    // selected, dim otherwise — so the user can see and click them.
    let active_drill: Option<(
        rs_cam_core::ids::ModelId,
        rs_cam_core::session::HoleSelection,
        HoleMisfits,
    )> = {
        use crate::state::selection::Selection;
        let st = state;
        if st.workspace == Workspace::Toolpaths
            && let Selection::Toolpath(id) = st.selection
        {
            st.session()
                .toolpath_configs()
                .iter()
                .find(|tc| tc.id == id)
                .and_then(|tc| {
                    // Geometry selection P2.3c: the items the
                    // generator resolves, each marked selected
                    // or not (`ProjectSession::hole_selection`).
                    let holes = st.session().hole_selection(tc)?;
                    Some((tc.model_id, holes, hole_misfits(st, tc)))
                })
        } else {
            None
        }
    };

    // P4.D: every visible model drawing draws, with or without a
    // toolpath (`drawing_draws`). The vector-source toolpath's model
    // draws in this block, with its states; the others draw in the
    // neutral block after it.
    let drawing_draws = crate::state::viewport::drawing_draws(state);
    let vector_source = {
        drawing_draws
            .iter()
            .find_map(|draw| match draw {
                crate::state::viewport::DrawingDraw::Toolpath { toolpath, .. } => Some(*toolpath),
                crate::state::viewport::DrawingDraw::Neutral { .. } => None,
            })
            .and_then(|id| {
                let tc = state.session().find_toolpath_config_by_id(id)?.1;
                let setup = state
                    .session()
                    .setup_of_toolpath_id(id)
                    .and_then(|index| state.session().list_setups().get(index))
                    .map(SetupFrame::of);
                Some((tc.clone(), setup))
            })
    };

    let ring_to_lines = |ring: &[rs_cam_core::geo::P2],
                         close: bool,
                         setup_opt: Option<&SetupFrame>,
                         point_z: &dyn Fn(f64, f64) -> Option<f32>,
                         segment_spacing: Option<f64>,
                         color: [f32; 3],
                         verts: &mut Vec<LineVertex>| {
        if ring.len() < 2 {
            return;
        }
        // P4.4: the one point function of the draw and the pick.
        let transform_pt = |p: &rs_cam_core::geo::P2| -> (f64, f64) {
            let q = crate::interaction::picking::item_display_xy(setup_opt, stock, *p);
            (q.x, q.y)
        };
        let mut draw_segment = |a: (f64, f64), b: (f64, f64)| {
            let distance = (b.0 - a.0).hypot(b.1 - a.1);
            let pieces = segment_spacing.map_or(1, |spacing| {
                (distance / spacing.max(f64::EPSILON)).ceil().max(1.0) as usize
            });
            for piece in 0..pieces {
                let t0 = piece as f64 / pieces as f64;
                let t1 = (piece + 1) as f64 / pieces as f64;
                let p0 = (a.0 + (b.0 - a.0) * t0, a.1 + (b.1 - a.1) * t0);
                let p1 = (a.0 + (b.0 - a.0) * t1, a.1 + (b.1 - a.1) * t1);
                if let (Some(z0), Some(z1)) = (point_z(p0.0, p0.1), point_z(p1.0, p1.1)) {
                    verts.push(LineVertex {
                        position: [p0.0 as f32, p0.1 as f32, z0],
                        color,
                    });
                    verts.push(LineVertex {
                        position: [p1.0 as f32, p1.1 as f32, z1],
                        color,
                    });
                }
            }
        };
        for pair in ring.windows(2) {
            #[expect(
                clippy::indexing_slicing,
                reason = "windows(2) guarantees exactly 2 elements per slice."
            )]
            draw_segment(transform_pt(&pair[0]), transform_pt(&pair[1]));
        }
        if close && let (Some(last), Some(first)) = (ring.last(), ring.first()) {
            draw_segment(transform_pt(last), transform_pt(first));
        }
    };

    'polygon_upload: {
        let Some((toolpath, model_setup)) = vector_source else {
            break 'polygon_upload;
        };
        let Some(model) = state
            .session()
            .models()
            .iter()
            .find(|model| model.id == toolpath.model_id)
        else {
            break 'polygon_upload;
        };
        if !model.items.has_drawing() {
            break 'polygon_upload;
        }
        let setup_ref = model_setup.as_ref();
        // P4.5: the one plane of the draw and the pick.
        let planar_z =
            crate::interaction::picking::drawing_plane_z(state.session(), &toolpath, setup_ref)
                as f32;
        let surface_mesh = match &toolpath.operation {
            rs_cam_core::compute::catalog::OperationConfig::ProjectCurve(cfg) => state
                .session()
                .models()
                .iter()
                .find(|model| model.id == cfg.surface_model_id.unwrap_or(toolpath.model_id))
                .and_then(|model| model.mesh.as_ref())
                .map(|mesh| {
                    let mesh = match model_setup {
                        Some(setup) => transform_mesh(mesh, &setup, stock),
                        None => (**mesh).clone(),
                    };
                    // Projection runs on the UI upload path, so never
                    // scan every face per vector endpoint. This is the
                    // same uniform index family the generator uses.
                    let index = rs_cam_core::mesh::SpatialIndex::build_auto(&mesh);
                    (mesh, index, cfg.direction)
                }),
            _ => None,
        };
        let segment_spacing = match &toolpath.operation {
            rs_cam_core::compute::catalog::OperationConfig::ProjectCurve(cfg) => {
                Some(cfg.point_spacing)
            }
            _ => None,
        };
        let point_z = |x: f64, y: f64| -> Option<f32> {
            let Some((mesh, index, direction)) = surface_mesh.as_ref() else {
                return Some(planar_z);
            };
            let z = index
                .query(x, y, 0.0)
                .into_iter()
                .filter_map(|face_index| mesh.faces.get(face_index))
                .filter(|face| face.contains_point_xy(x, y))
                .filter_map(|face| face.z_at_xy(x, y));
            let z = match direction {
                rs_cam_core::compute::operation_configs::ProjectCurveDirection::FromAbove => {
                    z.max_by(f64::total_cmp)
                }
                rs_cam_core::compute::operation_configs::ProjectCurveDirection::FromBelow => {
                    z.min_by(f64::total_cmp)
                }
            }?;
            Some(z as f32 + 0.05)
        };
        // Each drawn ring of the drawing (P2.1): a loop, a path or a
        // circle. An island is its own loop, so the lines are the
        // lines of the nested list.
        //
        // P4.4: each item takes the colour of its state in the
        // selection report of this toolpath (the memo generation
        // reads), index-aligned with `model.items`. The loop
        // enumerates EVERY item and skips a shape with no ring
        // inside, so the index stays the item index.
        use crate::render::colors;
        use rs_cam_core::selection::{ItemKind, MatchKey, Shape, SourceRef};
        let highlight: Option<std::sync::Arc<rs_cam_core::session::SelectionReport>> =
            if state.viewport.show_selection_states {
                crate::state::selection_highlight::highlight_report(state)
                    .filter(|(id, _, _)| *id == toolpath.id)
                    .map(|(_, _, report)| report)
            } else {
                None
            };
        let flash: &[MatchKey] = state
            .viewport
            .selection_flash
            .as_ref()
            .filter(|f| f.toolpath == toolpath.id)
            .map_or(&[], |f| f.keys.as_slice());
        let flashed = |key: &MatchKey| flash.iter().any(|k| k.names_same_geometry(key));
        let mark = crate::interaction::picking::POINT_MARK_RADIUS_MM;
        let line = |a: (f64, f64), b: (f64, f64), color: [f32; 3], verts: &mut Vec<LineVertex>| {
            if let (Some(z0), Some(z1)) = (point_z(a.0, a.1), point_z(b.0, b.1)) {
                verts.push(LineVertex {
                    position: [a.0 as f32, a.1 as f32, z0],
                    color,
                });
                verts.push(LineVertex {
                    position: [b.0 as f32, b.1 as f32, z1],
                    color,
                });
            }
        };
        let display = |p: rs_cam_core::geo::P2| -> (f64, f64) {
            let q = crate::interaction::picking::item_display_xy(setup_ref, stock, p);
            (q.x, q.y)
        };
        // A cross of half size `mark` at a display point.
        let cross = |c: (f64, f64), color: [f32; 3], verts: &mut Vec<LineVertex>| {
            line(
                (c.0 - mark, c.1 - mark),
                (c.0 + mark, c.1 + mark),
                color,
                verts,
            );
            line(
                (c.0 - mark, c.1 + mark),
                (c.0 + mark, c.1 - mark),
                color,
                verts,
            );
        };
        let ring_mark =
            |c: (f64, f64), radius: f64, color: [f32; 3], verts: &mut Vec<LineVertex>| {
                if let Some(z) = point_z(c.0, c.1) {
                    crate::app::push_circle_vertices(
                        verts,
                        c.0 as f32,
                        c.1 as f32,
                        z,
                        radius.max(mark) as f32,
                        color,
                        24,
                    );
                }
            };
        // The start and end marks of an open path (P4.4): a
        // triangle at the start that points along the path, a
        // square at the end. The line pipeline cannot fill, so
        // both are outlines.
        let path_ends = |ring: &[rs_cam_core::geo::P2], verts: &mut Vec<LineVertex>| {
            let (Some(first), Some(second), Some(last)) = (ring.first(), ring.get(1), ring.last())
            else {
                return;
            };
            let a = display(*first);
            let b = display(*second);
            let len = (b.0 - a.0).hypot(b.1 - a.1);
            if len > f64::EPSILON {
                let d = ((b.0 - a.0) / len, (b.1 - a.1) / len);
                let n = (-d.1, d.0);
                let tip = (a.0 + d.0 * 2.0 * mark, a.1 + d.1 * 2.0 * mark);
                let l = (a.0 + n.0 * mark, a.1 + n.1 * mark);
                let r = (a.0 - n.0 * mark, a.1 - n.1 * mark);
                line(l, tip, colors::PATH_START, verts);
                line(tip, r, colors::PATH_START, verts);
                line(r, l, colors::PATH_START, verts);
            }
            let e = display(*last);
            let corners = [
                (e.0 - mark, e.1 - mark),
                (e.0 + mark, e.1 - mark),
                (e.0 + mark, e.1 + mark),
                (e.0 - mark, e.1 + mark),
            ];
            for (i, c) in corners.iter().enumerate() {
                let next = corners.get((i + 1) % corners.len()).copied().unwrap_or(*c);
                line(*c, next, colors::PATH_END, verts);
            }
        };

        // P6.7: the direction arrow at the middle of an open path (the
        // start rule of P2.2 on screen), a chevron that points along the
        // drawn direction.
        let path_arrow = |ring: &[rs_cam_core::geo::P2], verts: &mut Vec<LineVertex>| {
            let Some((middle, [dx, dy])) = crate::state::selection_highlight::path_middle(ring)
            else {
                return;
            };
            // The direction in the display frame: a setup may mirror or
            // turn the drawing.
            let a = display(middle);
            let b = display(rs_cam_core::geo::P2::new(middle.x + dx, middle.y + dy));
            let len = (b.0 - a.0).hypot(b.1 - a.1);
            if len <= f64::EPSILON {
                return;
            }
            let d = ((b.0 - a.0) / len, (b.1 - a.1) / len);
            let n = (-d.1, d.0);
            let tip = (a.0 + d.0 * mark, a.1 + d.1 * mark);
            let back = (a.0 - d.0 * mark, a.1 - d.1 * mark);
            line(
                (back.0 + n.0 * mark, back.1 + n.1 * mark),
                tip,
                colors::PATH_START,
                verts,
            );
            line(
                (back.0 - n.0 * mark, back.1 - n.1 * mark),
                tip,
                colors::PATH_START,
                verts,
            );
        };
        // P6.7: the side that a Trace Left/Right or a Pocket Extend cuts,
        // as the tool on that side at the middle of a cut path. The point
        // is found in the drawing frame (core reads the side there) and
        // then moved to the display frame, so a Bottom setup mirrors it.
        let path_side = crate::state::selection_highlight::open_path_side(&toolpath.operation);
        let tool_radius = state
            .session()
            .tools()
            .iter()
            .find(|tool| tool.id == toolpath.tool_id)
            .map_or(mark, |tool| tool.diameter / 2.0);
        let side_mark = |ring: &[rs_cam_core::geo::P2], verts: &mut Vec<LineVertex>| {
            let (Some(side), Some((middle, direction))) = (
                path_side,
                crate::state::selection_highlight::path_middle(ring),
            ) else {
                return;
            };
            let centre =
                crate::state::selection_highlight::side_point(middle, direction, side, tool_radius);
            line(display(middle), display(centre), colors::PATH_SIDE, verts);
            ring_mark(display(centre), tool_radius, colors::PATH_SIDE, verts);
        };

        let mut verts = Vec::new();
        // The flashed items draw last, over the rest.
        let mut flash_verts = Vec::new();
        for (index, item) in model.items.iter().enumerate() {
            let item_state = highlight
                .as_ref()
                .and_then(|report| report.item_states.get(index).copied());
            let is_flashed = flashed(item.key());
            let color = if is_flashed {
                colors::SELECTION_HOVER
            } else if let Some(item_state) = item_state {
                colors::selection_state_color(item_state)
            } else if item.kind() == ItemKind::Path {
                colors::OPEN_PATH
            } else {
                colors::POLYGON_OUTLINE
            };
            let out = if is_flashed {
                &mut flash_verts
            } else {
                &mut verts
            };
            match item.shape() {
                Shape::Loop(ring) | Shape::Path(ring) | Shape::Circle { ring, .. } => {
                    ring_to_lines(
                        &ring.exterior,
                        ring.closed,
                        setup_ref,
                        &point_z,
                        segment_spacing,
                        color,
                        out,
                    );
                    if item.kind() == ItemKind::Path && state.viewport.show_open_path_ends {
                        path_ends(&ring.exterior, out);
                        path_arrow(&ring.exterior, out);
                    }
                    if item.kind() == ItemKind::Path
                        && item_state == Some(rs_cam_core::session::ItemState::Cut)
                    {
                        side_mark(&ring.exterior, out);
                    }
                }
                // A point has no line. With a state it gets a ring
                // mark (the drill markers below then do not draw).
                Shape::Point(p) if item_state.is_some() => {
                    ring_mark(display(*p), mark, color, out);
                }
                Shape::Point(_) | Shape::Face(_) => {}
            }
            if item_state == Some(rs_cam_core::session::ItemState::Excluded) {
                let [x, y] = item.key().anchor;
                cross(
                    display(rs_cam_core::geo::P2::new(x, y)),
                    colors::SELECTION_EXCLUDED,
                    out,
                );
            }
        }

        // The rings of an item of another source (L-9): a stock
        // item draws as the stock outline at the drawing plane, an
        // item of another model as its ring, both in this
        // toolpath's frame (the frame door moves them so).
        let stock_outline = |color: [f32; 3], verts: &mut Vec<LineVertex>| {
            let (x0, y0, w, d) = match setup_ref {
                Some(setup) => {
                    let (w, d, _) = setup.effective_stock(stock);
                    (0.0, 0.0, w, d)
                }
                None => (stock.origin_x, stock.origin_y, stock.x, stock.y),
            };
            let c = [(x0, y0), (x0 + w, y0), (x0 + w, y0 + d), (x0, y0 + d)];
            for (i, a) in c.iter().enumerate() {
                let b = c.get((i + 1) % c.len()).copied().unwrap_or(*a);
                line(*a, b, color, verts);
            }
        };
        let foreign_ring = |key: &MatchKey| -> Option<rs_cam_core::polygon::Polygon2> {
            let SourceRef::Model(other) = key.source else {
                return None;
            };
            state
                .session()
                .models()
                .iter()
                .find(|m| m.id == other)?
                .items
                .iter()
                .find(|i| key.names_same_geometry(i.key()))?
                .shape()
                .drawing_ring()
                .cloned()
        };
        if let Some(report) = highlight.as_ref() {
            for (key, item_state) in &report.foreign {
                let color = if flashed(key) {
                    colors::SELECTION_HOVER
                } else {
                    colors::selection_state_color(*item_state)
                };
                if key.source == SourceRef::Stock {
                    stock_outline(color, &mut verts);
                } else if let Some(ring) = foreign_ring(key) {
                    ring_to_lines(
                        &ring.exterior,
                        ring.closed,
                        setup_ref,
                        &point_z,
                        segment_spacing,
                        color,
                        &mut verts,
                    );
                }
            }
            // P6.7: the area that a Pocket Extend cuts beside each open
            // path, in the cut colour. The report holds the outer ring
            // in the drawing frame; it does not hold the extension rays,
            // so the outline is the whole mark.
            for ring in &report.extended {
                ring_to_lines(
                    ring,
                    true,
                    setup_ref,
                    &point_z,
                    segment_spacing,
                    colors::SELECTION_CUT,
                    &mut verts,
                );
            }
            // A lost pick has no item: a ghost at its stored key
            // position, a ring of diameter `measure` for a loop, a
            // face or a circle, a cross for a point or a path.
            for lost in &report.lost {
                let [x, y] = lost.key.anchor;
                let at = display(rs_cam_core::geo::P2::new(x, y));
                let color = if flashed(&lost.key) {
                    colors::SELECTION_HOVER
                } else {
                    colors::SELECTION_LOST
                };
                match lost.key.kind {
                    ItemKind::Loop | ItemKind::Face | ItemKind::Circle => {
                        ring_mark(at, lost.key.measure / 2.0, color, &mut verts);
                    }
                    ItemKind::Point | ItemKind::Path => cross(at, color, &mut verts),
                }
            }
        }

        // P4.4: the boundary regions of the toolpath, when its
        // boundary source is a selection (`boundary_outline`).
        if state.viewport.show_boundary_outline
            && let Some(boundary) = crate::state::selection_highlight::boundary_report(state)
        {
            let in_boundary = |s: &rs_cam_core::session::ItemState| {
                matches!(
                    s,
                    rs_cam_core::session::ItemState::Cut | rs_cam_core::session::ItemState::Island
                )
            };
            for (item, item_state) in model.items.iter().zip(&boundary.item_states) {
                if !in_boundary(item_state) {
                    continue;
                }
                let ring = match item.shape() {
                    Shape::Loop(ring) | Shape::Circle { ring, .. } => ring.exterior.clone(),
                    Shape::Face(face) => face.rings.exterior.clone(),
                    Shape::Path(_) | Shape::Point(_) => continue,
                };
                ring_to_lines(
                    &ring,
                    true,
                    setup_ref,
                    &point_z,
                    segment_spacing,
                    colors::BOUNDARY_OUTLINE,
                    &mut verts,
                );
            }
            for (key, item_state) in &boundary.foreign {
                if !in_boundary(item_state) {
                    continue;
                }
                if key.source == SourceRef::Stock {
                    stock_outline(colors::BOUNDARY_OUTLINE, &mut verts);
                } else if let Some(ring) = foreign_ring(key) {
                    ring_to_lines(
                        &ring.exterior,
                        true,
                        setup_ref,
                        &point_z,
                        segment_spacing,
                        colors::BOUNDARY_OUTLINE,
                        &mut verts,
                    );
                }
            }
        }
        verts.append(&mut flash_verts);

        // Drill hole markers for the active drill op's model. With
        // the selection states on, the item states colour the holes
        // (one colour source at a time, P4.4).
        if highlight.is_none()
            && let Some((mid, holes, misfits)) = active_drill.as_ref()
            && model.id == *mid
        {
            for item in &holes.items {
                // P2.6: the one point the pick tests too.
                let at = crate::interaction::picking::hole_display_point(setup_ref, stock, item.xy);
                // G-DRILLPICKSTALE: the loop is over the ITEMS of
                // the model. A pick that names no item is drawn
                // nowhere; the panel prints its refusal instead.
                // P6.7: a selected hole that the op does not cut for its
                // size (the fit of the report) draws in the skipped colour.
                let marker_color = if item.selected && misfits.contains(&item.key) {
                    crate::render::colors::HOLE_MISFIT
                } else if item.selected {
                    crate::render::colors::HOLE_SELECTED
                } else {
                    crate::render::colors::HOLE_FREE
                };
                let radius = item
                    .diameter
                    .map_or(crate::interaction::picking::POINT_MARK_RADIUS_MM, |d| {
                        d / 2.0
                    })
                    .max(crate::interaction::picking::HOLE_MARK_MIN_RADIUS_MM)
                    as f32;
                crate::app::push_circle_vertices(
                    &mut verts,
                    at.x as f32,
                    at.y as f32,
                    at.z as f32,
                    radius,
                    marker_color,
                    16,
                );
            }
        }

        if !verts.is_empty() {
            let buffer =
                gpu.device
                    .create_buffer_init(&egui_wgpu::wgpu::util::BufferInitDescriptor {
                        label: Some("polygon_lines"),
                        contents: bytemuck::cast_slice(&verts),
                        usage: egui_wgpu::wgpu::BufferUsages::VERTEX,
                    });
            resources.polygon_data.push(PolygonGpuData {
                vertex_buffer: buffer,
                vertex_count: verts.len() as u32,
            });
        }
    }

    // P4.D: the drawings that no drawn toolpath reads, in the frame
    // of the active setup and the neutral colour. So a DXF shows on
    // import, before an op reads it.
    let neutral_verts: Vec<LineVertex> = {
        drawing_draws
            .iter()
            .filter_map(|draw| match draw {
                crate::state::viewport::DrawingDraw::Neutral { model_id } => {
                    state.session().models().iter().find(|m| m.id == *model_id)
                }
                crate::state::viewport::DrawingDraw::Toolpath { .. } => None,
            })
            .flat_map(|model| super::neutral_drawing_vertices(model, frame.frame(), stock))
            .collect()
    };
    if !neutral_verts.is_empty() {
        let buffer = gpu
            .device
            .create_buffer_init(&egui_wgpu::wgpu::util::BufferInitDescriptor {
                label: Some("neutral_drawing_lines"),
                contents: bytemuck::cast_slice(&neutral_verts),
                usage: egui_wgpu::wgpu::BufferUsages::VERTEX,
            });
        resources.polygon_data.push(PolygonGpuData {
            vertex_buffer: buffer,
            vertex_count: neutral_verts.len() as u32,
        });
    }
}

/// The keys of the holes that the op does not cut for their size
/// (geometry selection P6.7).
type HoleMisfits = Vec<rs_cam_core::selection::MatchKey>;

/// The hole items of `tc` whose fit its fit rule does not keep: the fit
/// of the `selection_status` report (`hole_fits`, the fit step that
/// generation runs) against the rule of the op row (`ItemUse::Holes`).
/// Empty for an op with no fit, and when the report does not resolve.
fn hole_misfits(state: &AppState, tc: &rs_cam_core::session::ToolpathConfig) -> HoleMisfits {
    use rs_cam_core::compute::catalog::ItemUse;
    let ItemUse::Holes { fit: rule, .. } = tc.operation.capabilities().item_use else {
        return Vec::new();
    };
    state
        .session()
        .selection_status(tc, rs_cam_core::session::SelectionTarget::Op)
        .map(|report| {
            report
                .hole_fits
                .iter()
                .filter(|(_, fit)| !rule.keeps(*fit))
                .map(|(key, _)| key.clone())
                .collect()
        })
        .unwrap_or_default()
}
