use std::collections::HashMap;
use std::sync::Arc;

use rs_cam_core::feeds::{AdvancePerToothMm, VendorChiploadBand};

use crate::render::RenderResources;
use crate::render::mesh_render::MeshGpuData;
use crate::render::sim_render::{self, SimMeshGpuData};
use crate::render::stock_render::StockGpuData;
use crate::render::toolpath_render::{self, ToolpathGpuData};
use crate::state::Workspace;
use crate::state::job::{
    self, Setup, SetupId, height_context_from_session, session_fixture_bbox,
    session_fixture_clearance_bbox, session_keep_out_bbox, transform_mesh,
};
use crate::state::selection::Selection;
use crate::state::simulation::StockVizMode;

use super::RsCamApp;

impl RsCamApp {
    /// Get selected BREP face IDs for rendering highlights.
    /// Reads from the active toolpath's face_selection when a toolpath is selected,
    /// or from the visual Selection::Face/Faces state otherwise.
    fn selected_face_ids(&self) -> Vec<rs_cam_core::enriched_mesh::FaceGroupId> {
        let state = self.controller.state();
        match &state.selection {
            Selection::Toolpath(tp_id) => state
                .session
                .find_toolpath_config_by_id(*tp_id)
                .and_then(|(_, tc)| tc.face_selection.clone())
                .unwrap_or_default(),
            Selection::Face(_, face_id) => vec![*face_id],
            Selection::Faces(_, face_ids) => face_ids.clone(),
            _ => Vec::new(),
        }
    }

    /// Get the currently hovered face ID (for hover highlighting).
    fn hovered_face_id(&self) -> Option<rs_cam_core::enriched_mesh::FaceGroupId> {
        self.last_hover_face
    }

    /// Compute per-vertex colors for the sim mesh based on current viz mode.
    // SAFETY: color indices bounded by `num_verts * 3` guard above
    #[allow(clippy::indexing_slicing)]
    pub(super) fn compute_sim_colors(
        &self,
        mesh: &rs_cam_core::simulation::StockMesh,
    ) -> Vec<[f32; 3]> {
        let num_verts = mesh.vertices.len() / 3;
        match self.controller.state().simulation.stock_viz_mode {
            StockVizMode::Solid => {
                if mesh.colors.len() >= num_verts * 3 {
                    (0..num_verts)
                        .map(|i| {
                            [
                                mesh.colors[i * 3],
                                mesh.colors[i * 3 + 1],
                                mesh.colors[i * 3 + 2],
                            ]
                        })
                        .collect()
                } else {
                    vec![[0.65, 0.45, 0.25]; num_verts]
                }
            }
            StockVizMode::Deviation => {
                if let Some(devs) = &self
                    .controller
                    .state()
                    .simulation
                    .playback
                    .display_deviations
                {
                    sim_render::deviation_colors(devs)
                } else {
                    vec![[0.65, 0.45, 0.25]; num_verts]
                }
            }
            StockVizMode::ByHeight => {
                rs_cam_core::stock_mesh::height_gradient_colors(&mesh.vertices)
            }
        }
    }

    /// Transform a `StockMesh` from its simulation frame to the setup-local
    /// display frame for the given simulation `move_idx`.
    ///
    /// Every sim mesh arrives in the simulator's ZERO-ROOTED stock-relative
    /// global frame — checkpoint meshes via `transform_stock_mesh_to_global`
    /// and live-scrub meshes off the equally zero-rooted global stock. For
    /// an identity setup that frame IS the display frame, so nothing to do.
    /// Non-identity setups only need the face/rotation part of
    /// `world_to_local`; the origin translation was already applied
    /// upstream.
    ///
    /// This used to subtract `stock.origin` on the identity arm (heights /
    /// setup-frame audit 2026-06-12, finding 1). That shift was correct
    /// against a WORLD-framed mesh, which is what the simulator used to
    /// hand back for identity groups — and it cancelled the mis-registered
    /// carve those groups produced in the zero-rooted global stock, which
    /// is why the viewport looked right while
    /// `screenshot_simulation` (no such compensation) showed the part cut
    /// through. Both halves are fixed at the source now
    /// (G-SIM-IDENTITY-FRAME, 2026-08-19); keeping the shift here would
    /// double-count it.
    // SAFETY: step_by(3) loop with i+1, i+2 bounded by vertices.len() (always multiple of 3)
    #[allow(clippy::indexing_slicing)]
    pub(super) fn transform_mesh_to_local_frame(
        &self,
        mesh: &mut rs_cam_core::simulation::StockMesh,
        move_idx: usize,
    ) {
        let Some((face_up, z_rot, true)) = self.active_setup_orientation(move_idx) else {
            return;
        };
        let stock_cfg = self.controller.state().session.stock_config();
        let (eff_w, eff_d, _) = face_up.effective_stock(stock_cfg.x, stock_cfg.y, stock_cfg.z);
        for i in (0..mesh.vertices.len()).step_by(3) {
            let p = rs_cam_core::geo::P3::new(
                mesh.vertices[i] as f64,
                mesh.vertices[i + 1] as f64,
                mesh.vertices[i + 2] as f64,
            );
            let flipped = face_up.transform_point(p, stock_cfg.x, stock_cfg.y, stock_cfg.z);
            let local = z_rot.transform_point(flipped, eff_w, eff_d);
            mesh.vertices[i] = local.x as f32;
            mesh.vertices[i + 1] = local.y as f32;
            mesh.vertices[i + 2] = local.z as f32;
        }
    }

    pub(super) fn upload_gpu_data(&mut self, frame: &mut eframe::Frame) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };

        let mut renderer = render_state.renderer.write();
        // SAFETY: RenderResources inserted in RsCamApp::new; always present.
        #[allow(clippy::unwrap_used)]
        let resources: &mut RenderResources = renderer.callback_resources.get_mut().unwrap();

        // Everything is always displayed in the active setup's local coordinate
        // frame ("machine view").  Toolpaths, simulation, mesh, stock — all at
        // (0,0,0)-relative local coords.
        let active_setup_ref: Option<Setup> = {
            let state = self.controller.state();
            let sel = &state.selection;
            let setup_id = match sel {
                Selection::Setup(id) => Some(*id),
                Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
                Selection::Toolpath(tp_id) => {
                    // Map toolpath ID → setup via session
                    state
                        .session
                        .setup_of_toolpath_id(*tp_id)
                        .and_then(|idx| state.session.list_setups().get(idx))
                        .map(|sd| SetupId(sd.id))
                }
                _ => None,
            };
            let session_setup = if let Some(sid) = setup_id {
                state
                    .session
                    .list_setups()
                    .iter()
                    .find(|s| SetupId(s.id) == sid)
            } else {
                state.session.list_setups().first()
            };
            session_setup.map(|sd| Setup::for_transforms(SetupId(sd.id), sd.face_up, sd.z_rotation))
        };
        let use_local_frame = active_setup_ref.is_some();

        // Upload mesh data for all models with geometry
        resources.enriched_mesh_data_list.clear();
        resources.mesh_data_list.clear();
        let selected_faces = self.selected_face_ids();
        let hovered_face = self.hovered_face_id();
        let stock = self.controller.state().session.stock_config().clone();

        // Emission-frame → display-frame shift for the active setup.
        // Identity setups emit toolpaths (and sim positions) in world
        // coordinates while the viewport draws everything zero-rooted;
        // the difference is exactly -stock.origin. Zero for non-identity
        // setups and for stocks with origin at (0,0,0).
        let display_shift = active_setup_ref
            .as_ref()
            .map_or(rs_cam_core::geo::P3::new(0.0, 0.0, 0.0), |s| {
                s.emission_to_display_shift(&stock)
            });
        let has_display_shift =
            display_shift.x != 0.0 || display_shift.y != 0.0 || display_shift.z != 0.0;
        for model in self.controller.state().session.models() {
            // If model has enriched mesh (STEP), use face-colored rendering
            if let Some(enriched) = &model.enriched_mesh {
                let transform: crate::render::mesh_render::VertexTransform<'_> = if use_local_frame
                {
                    // SAFETY: use_local_frame is active_setup_ref.is_some()
                    #[allow(clippy::unwrap_used)]
                    let setup = active_setup_ref.as_ref().unwrap();
                    let stock_ref = &stock;
                    Some(Box::new(move |p| setup.transform_point(p, stock_ref)))
                } else {
                    None
                };
                if let Some(gpu) = crate::render::mesh_render::enriched_mesh_gpu_data(
                    &render_state.device,
                    &resources.gpu_limits,
                    enriched,
                    &selected_faces,
                    hovered_face,
                    &transform,
                ) {
                    resources.enriched_mesh_data_list.push(gpu);
                }
            } else if let Some(mesh) = &model.mesh {
                let gpu = if use_local_frame {
                    // SAFETY: use_local_frame is true iff active_setup_ref.is_some().
                    #[allow(clippy::unwrap_used)]
                    let setup = active_setup_ref.as_ref().unwrap();
                    let transformed = transform_mesh(mesh, setup, &stock);
                    MeshGpuData::from_mesh(
                        &render_state.device,
                        &resources.gpu_limits,
                        &Arc::new(transformed),
                    )
                } else {
                    MeshGpuData::from_mesh(&render_state.device, &resources.gpu_limits, mesh)
                };
                if let Some(gpu) = gpu {
                    resources.mesh_data_list.push(gpu);
                }
            }
        }

        // Upload polygon/DXF/SVG line data.
        //
        // Each model's polygons are drawn using the transform of whichever
        // setup actually references the model via a toolpath. This matters
        // for multi-setup projects: a DXF used by a face=Bottom setup must
        // flip onto the bottom in the sim view even when the top setup is
        // selected (or nothing is selected in Simulation mode). Falls back
        // to the selection-based `active_setup_ref` for unowned models.
        {
            use crate::render::{LineVertex, PolygonGpuData};
            use egui_wgpu::wgpu::util::DeviceExt;
            resources.polygon_data.clear();
            let color = crate::render::colors::POLYGON_OUTLINE;

            // When a drill op is selected, draw its model's drill targets
            // (DXF points / circle centres) as pickable markers — bright when
            // selected, dim otherwise — so the user can see and click them.
            let active_drill: Option<(usize, Vec<[f64; 2]>)> = {
                use crate::state::selection::Selection;
                use rs_cam_core::compute::catalog::OperationConfig;
                let st = self.controller.state();
                if st.workspace == Workspace::Toolpaths
                    && let Selection::Toolpath(id) = st.selection
                {
                    st.session
                        .toolpath_configs()
                        .iter()
                        .find(|tc| tc.id == id)
                        .and_then(|tc| match &tc.operation {
                            OperationConfig::Drill(c) => {
                                Some((tc.model_id, c.selected_holes.clone().unwrap_or_default()))
                            }
                            OperationConfig::AlignmentPinDrill(c) => {
                                Some((tc.model_id, c.selected_holes.clone().unwrap_or_default()))
                            }
                            _ => None,
                        })
                } else {
                    None
                }
            };

            let setup_for_model = |model_id: usize| -> Option<Setup> {
                let state = self.controller.state();
                let tc = state
                    .session
                    .toolpath_configs()
                    .iter()
                    .find(|tc| tc.model_id == model_id)?;
                let setup_idx = state.session.setup_of_toolpath_id(tc.id)?;
                let sd = state.session.list_setups().get(setup_idx)?;
                Some(Setup::for_transforms(
                    SetupId(sd.id),
                    sd.face_up,
                    sd.z_rotation,
                ))
            };

            let ring_to_lines = |ring: &[rs_cam_core::geo::P2],
                                 close: bool,
                                 setup_opt: Option<&Setup>,
                                 poly_z: f32,
                                 verts: &mut Vec<LineVertex>| {
                if ring.len() < 2 {
                    return;
                }
                let transform_pt = |p: &rs_cam_core::geo::P2| -> (f64, f64) {
                    if let Some(setup) = setup_opt {
                        let tp =
                            setup.transform_point(rs_cam_core::geo::P3::new(p.x, p.y, 0.0), &stock);
                        (tp.x, tp.y)
                    } else {
                        (p.x, p.y)
                    }
                };
                for pair in ring.windows(2) {
                    // SAFETY: windows(2) guarantees exactly 2 elements per slice.
                    #[allow(clippy::indexing_slicing)]
                    let (a, b) = (&pair[0], &pair[1]);
                    let (ax, ay) = transform_pt(a);
                    let (bx, by) = transform_pt(b);
                    verts.push(LineVertex {
                        position: [ax as f32, ay as f32, poly_z],
                        color,
                    });
                    verts.push(LineVertex {
                        position: [bx as f32, by as f32, poly_z],
                        color,
                    });
                }
                if close && let (Some(last), Some(first)) = (ring.last(), ring.first()) {
                    let (ax, ay) = transform_pt(last);
                    let (bx, by) = transform_pt(first);
                    verts.push(LineVertex {
                        position: [ax as f32, ay as f32, poly_z],
                        color,
                    });
                    verts.push(LineVertex {
                        position: [bx as f32, by as f32, poly_z],
                        color,
                    });
                }
            };

            for model in self.controller.state().session.models() {
                let Some(polys) = &model.polygons else {
                    continue;
                };
                // Prefer the setup that actually uses this model; fall back to
                // the current active/selected setup.
                let model_setup = setup_for_model(model.id).or_else(|| {
                    active_setup_ref
                        .as_ref()
                        .map(|s| Setup::for_transforms(s.id, s.face_up, s.z_rotation))
                });
                // Draw slightly above the setup's local stock top to avoid
                // z-fighting. When no setup is known, fall back to world-frame
                // stock top.
                let poly_z = if let Some(ref setup) = model_setup {
                    let (_, _, h) = setup.effective_stock(&stock);
                    h as f32 + 0.05
                } else {
                    (stock.origin_z + stock.z) as f32 + 0.05
                };
                let setup_ref = model_setup.as_ref();
                let mut verts = Vec::new();
                for poly in polys.iter() {
                    ring_to_lines(&poly.exterior, poly.closed, setup_ref, poly_z, &mut verts);
                    for hole in &poly.holes {
                        ring_to_lines(hole, true, setup_ref, poly_z, &mut verts);
                    }
                }

                // Drill target markers for the active drill op's model.
                if let Some((mid, selected)) = active_drill.as_ref()
                    && model.id == *mid
                {
                    use rs_cam_core::dxf_input::DrillTargetKind;
                    for t in model.drill_targets.iter() {
                        let (tx, ty) = if let Some(setup) = setup_ref {
                            let tp = setup
                                .transform_point(rs_cam_core::geo::P3::new(t.x, t.y, 0.0), &stock);
                            (tp.x, tp.y)
                        } else {
                            (t.x, t.y)
                        };
                        let is_sel = selected
                            .iter()
                            .any(|h| (h[0] - t.x).abs() < 1e-6 && (h[1] - t.y).abs() < 1e-6);
                        let marker_color = if is_sel {
                            [0.2_f32, 0.95, 0.4] // bright green = selected
                        } else {
                            [0.95_f32, 0.55, 0.15] // orange = available
                        };
                        let radius = match t.kind {
                            DrillTargetKind::CircleCenter { diameter } => (diameter / 2.0) as f32,
                            DrillTargetKind::Point => 1.5,
                        }
                        .max(1.0);
                        super::push_circle_vertices(
                            &mut verts,
                            tx as f32,
                            ty as f32,
                            poly_z,
                            radius,
                            marker_color,
                            16,
                        );
                    }
                }

                if !verts.is_empty() {
                    let buffer = render_state.device.create_buffer_init(
                        &egui_wgpu::wgpu::util::BufferInitDescriptor {
                            label: Some("polygon_lines"),
                            contents: bytemuck::cast_slice(&verts),
                            usage: egui_wgpu::wgpu::BufferUsages::VERTEX,
                        },
                    );
                    resources.polygon_data.push(PolygonGpuData {
                        vertex_buffer: buffer,
                        vertex_count: verts.len() as u32,
                    });
                }
            }
        }

        // Upload stock wireframe + solid stock
        let stock_bbox = if use_local_frame {
            // SAFETY: use_local_frame is true iff active_setup_ref.is_some().
            #[allow(clippy::unwrap_used)]
            let setup = active_setup_ref.as_ref().unwrap();
            let (w, d, h) = setup.effective_stock(&stock);
            rs_cam_core::geo::BoundingBox3 {
                min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                max: rs_cam_core::geo::P3::new(w, d, h),
            }
        } else {
            stock.bbox()
        };
        resources.stock_data = Some(StockGpuData::from_bbox(&render_state.device, &stock_bbox));
        resources.solid_stock_data =
            Some(crate::render::stock_render::SolidStockGpuData::from_bbox(
                &render_state.device,
                &stock_bbox,
            ));

        // Upload origin axes at stock origin (local origin when in machine view)
        {
            let origin = if use_local_frame {
                [0.0_f32, 0.0, 0.0]
            } else {
                [
                    stock.origin_x as f32,
                    stock.origin_y as f32,
                    stock.origin_z as f32,
                ]
            };
            let min_dim = stock.x.min(stock.y).min(stock.z) as f32;
            let length = (min_dim * 0.3).clamp(5.0, 50.0);
            resources.origin_axes_data = Some(crate::render::grid_render::OriginAxesGpuData::new(
                &render_state.device,
                origin,
                length,
            ));
        }

        // Upload fixture and keep-out wireframes.
        {
            use crate::render::fixture_render::FixtureGpuData;
            use rs_cam_core::geo::{BoundingBox3, P3};

            let state = self.controller.state();
            let selection = &state.selection;

            // Helper: forward-transform a bbox into the setup's local frame.
            // After transforming corners, min/max may swap, so rebuild via from_points.
            let transform_bbox = |bb: BoundingBox3, setup: &Setup| -> BoundingBox3 {
                let corners = [
                    P3::new(bb.min.x, bb.min.y, bb.min.z),
                    P3::new(bb.max.x, bb.min.y, bb.min.z),
                    P3::new(bb.min.x, bb.max.y, bb.min.z),
                    P3::new(bb.max.x, bb.max.y, bb.min.z),
                    P3::new(bb.min.x, bb.min.y, bb.max.z),
                    P3::new(bb.max.x, bb.min.y, bb.max.z),
                    P3::new(bb.min.x, bb.max.y, bb.max.z),
                    P3::new(bb.max.x, bb.max.y, bb.max.z),
                ];
                BoundingBox3::from_points(corners.iter().map(|c| setup.transform_point(*c, &stock)))
            };

            // Only show fixtures/keepouts/pins from the active setup (each
            // setup has its own local frame, so mixing them is wrong).
            // Get the active setup's session data for fixtures/keepouts/pins.
            let active_session_setup = active_setup_ref.as_ref().and_then(|s| {
                state
                    .session
                    .list_setups()
                    .iter()
                    .find(|sd| sd.id == s.id.0)
            });

            // Fixture/keep-out boxes are only shown outside Simulation.
            let mut boxes = Vec::new();
            let in_sim = state.workspace == Workspace::Simulation;
            if !in_sim && let Some(sd) = active_session_setup {
                // SAFETY: active_setup_ref.is_some() when active_session_setup.is_some()
                #[allow(clippy::unwrap_used)]
                let setup = active_setup_ref.as_ref().unwrap();
                for fixture in &sd.fixtures {
                    if fixture.enabled {
                        let selected = *selection == Selection::Fixture(SetupId(sd.id), fixture.id);
                        let color = if selected {
                            [1.0_f32, 0.9, 0.4] // bright highlight
                        } else {
                            [0.9_f32, 0.7, 0.2]
                        };
                        let clearance = session_fixture_clearance_bbox(fixture);
                        let display_clearance = transform_bbox(clearance, setup);
                        boxes.push((display_clearance, color));
                        if selected {
                            let inner = session_fixture_bbox(fixture);
                            let display_inner = transform_bbox(inner, setup);
                            boxes.push((display_inner, [0.9_f32, 0.9, 0.9]));
                        }
                    }
                }
                for keep_out in &sd.keep_out_zones {
                    if keep_out.enabled {
                        let selected =
                            *selection == Selection::KeepOut(SetupId(sd.id), keep_out.id);
                        let color = if selected {
                            [1.0_f32, 0.4, 0.4] // bright highlight
                        } else {
                            [0.9_f32, 0.2, 0.2]
                        };
                        let ko_bb = session_keep_out_bbox(keep_out, &stock);
                        let display_ko = transform_bbox(ko_bb, setup);
                        boxes.push((display_ko, color));
                    }
                }
            }

            // Render stock-level alignment pins as circles (visible in all setups).
            // Pin coords are stock-relative — add origin to get global for transform_point.
            let mut pin_vertices: Vec<crate::render::LineVertex> = Vec::new();
            let ox = stock.origin_x;
            let oy = stock.origin_y;
            let oz = stock.origin_z;
            if let Some(setup) = active_setup_ref.as_ref() {
                for pin in &stock.alignment_pins {
                    let radius = (pin.diameter / 2.0) as f32;
                    // Slight Z offset above stock top to avoid Z-fighting with stock surface.
                    let global_pt = P3::new(pin.x + ox, pin.y + oy, oz + stock.z + 0.1);
                    let local_pt = setup.transform_point(global_pt, &stock);
                    let (cx, cy, cz) = (local_pt.x as f32, local_pt.y as f32, local_pt.z as f32);
                    let color = [0.2_f32, 0.9, 0.3];
                    super::push_circle_vertices(&mut pin_vertices, cx, cy, cz, radius, color, 16);
                }

                // Flip axis dashed centerline
                if let Some(axis) = stock.flip_axis {
                    let stock_top = oz + stock.z;
                    let (start_g, end_g) = match axis {
                        job::FlipAxis::Horizontal => {
                            let y = stock.y / 2.0 + oy;
                            (
                                P3::new(ox, y, stock_top),
                                P3::new(ox + stock.x, y, stock_top),
                            )
                        }
                        job::FlipAxis::Vertical => {
                            let x = stock.x / 2.0 + ox;
                            (
                                P3::new(x, oy, stock_top),
                                P3::new(x, oy + stock.y, stock_top),
                            )
                        }
                    };
                    let start_l = setup.transform_point(start_g, &stock);
                    let end_l = setup.transform_point(end_g, &stock);
                    let s = [start_l.x as f32, start_l.y as f32, start_l.z as f32];
                    let e = [end_l.x as f32, end_l.y as f32, end_l.z as f32];
                    let axis_color = [0.9_f32, 0.7, 0.2];
                    super::push_dashed_line_vertices(&mut pin_vertices, s, e, axis_color, 5.0, 3.0);
                }
            }

            // Add datum crosshair markers in Setup workspace.
            // Datum is always in local frame coords.
            if state.workspace == Workspace::Setup
                && let Some(setup) = active_setup_ref.as_ref()
                && let Some(sd) = active_session_setup
            {
                use rs_cam_core::session::{Corner, XYDatum};

                let (eff_w, eff_d, eff_h) = setup.effective_stock(&stock);
                let color = [0.9_f32, 0.2, 0.9]; // magenta

                // The datum is persisted project state (W9 / P-2), read
                // straight off the setup instead of a GUI-side overlay.
                let datum = &sd.datum;

                // Datum in setup-local frame: XY at corner/center, Z at top surface
                let local_datum: Option<P3> = match &datum.xy_method {
                    XYDatum::CornerProbe(corner) => {
                        let x = match corner {
                            Corner::FrontLeft | Corner::BackLeft => 0.0,
                            Corner::FrontRight | Corner::BackRight => eff_w,
                        };
                        let y = match corner {
                            Corner::FrontLeft | Corner::FrontRight => 0.0,
                            Corner::BackLeft | Corner::BackRight => eff_d,
                        };
                        Some(P3::new(x, y, eff_h))
                    }
                    XYDatum::CenterOfStock => Some(P3::new(eff_w / 2.0, eff_d / 2.0, eff_h)),
                    _ => None,
                };

                if let Some(local) = local_datum {
                    // Always in local frame — use local coords directly.
                    let dx = local.x as f32;
                    let dy = local.y as f32;
                    let dz = local.z as f32;

                    let arm = 15.0_f32;
                    let diamond = 3.0_f32;

                    // Crosshair lines in all 3 directions (works for any face)
                    for &(ax, ay, az) in &[(arm, 0.0, 0.0), (0.0, arm, 0.0), (0.0, 0.0, arm)] {
                        pin_vertices.push(crate::render::LineVertex {
                            position: [dx - ax, dy - ay, dz - az],
                            color,
                        });
                        pin_vertices.push(crate::render::LineVertex {
                            position: [dx + ax, dy + ay, dz + az],
                            color,
                        });
                    }

                    // Diamond in XY plane at datum Z
                    for &[(x1, y1), (x2, y2)] in &[
                        [(diamond, 0.0), (0.0, diamond)],
                        [(0.0, diamond), (-diamond, 0.0)],
                        [(-diamond, 0.0), (0.0, -diamond)],
                        [(0.0, -diamond), (diamond, 0.0)],
                    ] {
                        pin_vertices.push(crate::render::LineVertex {
                            position: [dx + x1, dy + y1, dz],
                            color,
                        });
                        pin_vertices.push(crate::render::LineVertex {
                            position: [dx + x2, dy + y2, dz],
                            color,
                        });
                    }
                }
            }

            if boxes.is_empty() && pin_vertices.is_empty() {
                resources.fixture_data = None;
            } else {
                resources.fixture_data = Some(FixtureGpuData::from_boxes_and_lines(
                    &render_state.device,
                    &boxes,
                    &pin_vertices,
                ));
            }
        }

        // Re-upload sim mesh with current viz mode colors, or clear if no results.
        // If the geometry is already uploaded (typical SimVizModeChanged path),
        // update the single vertex buffer in-place instead of recreating GPU buffers.
        if self.controller.state().simulation.has_results() {
            if let Some(mesh) = &self.controller.state().simulation.playback.display_mesh {
                let colors = self.compute_sim_colors(mesh);
                let can_update_existing = resources
                    .sim_mesh_data
                    .as_ref()
                    .is_some_and(|data| data.chunks.len() == 1);
                if can_update_existing && let Some(data) = resources.sim_mesh_data.as_mut() {
                    data.update_colors_if_changed(&render_state.queue, mesh, &colors);
                } else {
                    let new_data = SimMeshGpuData::from_heightmap_mesh_colored(
                        &render_state.device,
                        &resources.gpu_limits,
                        mesh,
                        &colors,
                    );
                    resources.sim_mesh_data = new_data;
                }
            }
        } else {
            resources.sim_mesh_data = None;
            resources.tool_model_data = None;
        }

        // Upload collision markers with density-based heatmap coloring.
        // Nearby collisions cluster to brighter red; isolated ones are dimmer yellow.
        if !self.controller.collision_positions().is_empty() {
            use crate::render::LineVertex;
            let positions = self.controller.collision_positions();
            let s = 1.0f32; // marker size in mm
            let cluster_radius = 5.0_f32; // mm radius for density estimation

            // Precompute density for each collision point
            let densities: Vec<usize> = positions
                .iter()
                .map(|p| {
                    positions
                        .iter()
                        .filter(|q| {
                            let dx = p[0] - q[0];
                            let dy = p[1] - q[1];
                            let dz = p[2] - q[2];
                            dx * dx + dy * dy + dz * dz < cluster_radius * cluster_radius
                        })
                        .count()
                })
                .collect();
            let max_density = densities.iter().copied().max().unwrap_or(1).max(1);

            let vertex_size = std::mem::size_of::<LineVertex>();
            let mut verts = Vec::new();
            for (i, p) in positions.iter().enumerate() {
                if verts.len() * vertex_size >= resources.gpu_limits.max_buffer_size {
                    tracing::warn!(
                        markers = positions.len(),
                        "Too many collision markers — truncating to fit GPU buffer"
                    );
                    break;
                }
                // Collision positions come from the sim trace in the
                // emission frame — shift identity-setup markers into the
                // display frame alongside the toolpath lines below.
                let p = [
                    p[0] + display_shift.x as f32,
                    p[1] + display_shift.y as f32,
                    p[2] + display_shift.z as f32,
                ];
                // SAFETY: densities has same length as positions
                #[allow(clippy::indexing_slicing)]
                let t = densities[i] as f32 / max_density as f32;
                // Yellow (isolated) → Red (clustered)
                let color = [0.95, 0.8 * (1.0 - t) + 0.1, 0.1 * (1.0 - t)];
                verts.push(LineVertex {
                    position: [p[0] - s, p[1], p[2]],
                    color,
                });
                verts.push(LineVertex {
                    position: [p[0] + s, p[1], p[2]],
                    color,
                });
                verts.push(LineVertex {
                    position: [p[0], p[1] - s, p[2]],
                    color,
                });
                verts.push(LineVertex {
                    position: [p[0], p[1] + s, p[2]],
                    color,
                });
                verts.push(LineVertex {
                    position: [p[0], p[1], p[2] - s],
                    color,
                });
                verts.push(LineVertex {
                    position: [p[0], p[1], p[2] + s],
                    color,
                });
            }
            resources.collision_vertex_buffer = crate::render::gpu_safety::try_create_buffer(
                &render_state.device,
                &resources.gpu_limits,
                "collision_markers",
                bytemuck::cast_slice(&verts),
                egui_wgpu::wgpu::BufferUsages::VERTEX,
            );
            resources.collision_vertex_count = if resources.collision_vertex_buffer.is_some() {
                verts.len() as u32
            } else {
                0
            };
        } else {
            resources.collision_vertex_buffer = None;
            resources.collision_vertex_count = 0;
        }

        // Upload toolpath line data (with per-toolpath colors and isolation filtering)
        resources.toolpath_data.clear();
        let selected_tp_id = match self.controller.state().selection {
            Selection::Toolpath(id) => Some(id),
            _ => None,
        };
        let isolate = self.controller.state().viewport.isolate_toolpath;

        // Determine which setup is active for filtering toolpath display
        let active_setup_id = active_setup_ref.as_ref().map(|s| s.id);

        // Iterate session toolpath configs + GUI runtime
        {
            let state = self.controller.state();
            let session = &state.session;
            let gui = &state.gui;

            // For the advance/tooth colour mode: build per-toolpath vendor
            // band + per-move achieved advance/tooth maps once before the
            // per-toolpath loop. Both maps are keyed by toolpath_id (the
            // simulator-side `usize`).
            //
            // Both halves are typed (`VendorChiploadBand` /
            // `AdvancePerToothMm`) so the pairing that produced F-HEATMAP —
            // an arc-mean chip thickness handed to a band comparison —
            // cannot be reassembled here without a compile error.
            // SAFETY: complex tuple type used only as a local binding in
            // this function; aliasing it project-wide would obscure the
            // (band, per-move) pairing.
            #[allow(clippy::type_complexity)]
            let advance_inputs: Option<(
                HashMap<rs_cam_core::ToolpathId, VendorChiploadBand>,
                HashMap<rs_cam_core::ToolpathId, HashMap<usize, AdvancePerToothMm>>,
            )> = if matches!(
                state.viewport.toolpath_color_mode,
                crate::state::viewport::ToolpathColorMode::AdvancePerTooth
            ) {
                let sim_trace = state
                    .simulation
                    .results
                    .as_ref()
                    .and_then(|r| r.cut_trace.as_deref());
                let bands = build_advance_bands(session, sim_trace);
                let per_move =
                    rs_cam_core::tool_load::display::advance_per_tooth_per_move(sim_trace);
                Some((bands, per_move))
            } else {
                None
            };

            for (i, tc) in session.toolpath_configs().iter().enumerate() {
                // Find which setup owns this toolpath
                let tp_setup_id = session
                    .setup_of_toolpath_id(tc.id)
                    .and_then(|idx| session.list_setups().get(idx))
                    .map(|sd| SetupId(sd.id));
                if tp_setup_id != active_setup_id {
                    continue;
                }

                // Get runtime state for this toolpath
                let rt = gui.toolpath_rt.get(&tc.id);

                // Skip invisible toolpaths; also skip if not the isolated toolpath
                let tp_id = tc.id;
                let visible = rt.is_none_or(|r| r.visible)
                    && match isolate {
                        Some(iso_id) => tp_id == iso_id,
                        None => true,
                    };
                let result = rt.and_then(|r| r.result.as_ref());
                if visible && let Some(result) = result {
                    let selected = selected_tp_id == Some(tp_id);

                    // Toolpaths arrive in their emission frame: setup-local
                    // for non-identity setups, *world* for identity setups
                    // (F-028). The viewport draws zero-rooted local, so
                    // identity toolpaths must shift by -stock.origin to land
                    // on the mesh (heights/setup-frame audit 2026-06-12,
                    // finding 1 — pre-fix they rendered "through the stock
                    // floor" on any origin != 0 project).
                    let shifted_annotated = has_display_shift
                        .then(|| translate_annotated(&result.annotated, display_shift));
                    let render_annotated = shifted_annotated.as_ref().unwrap_or(&result.annotated);
                    let render_tp = &render_annotated.toolpath;

                    let color_mode = state.viewport.toolpath_color_mode;
                    let mut gpu_data = match color_mode {
                        crate::state::viewport::ToolpathColorMode::Engagement => {
                            ToolpathGpuData::from_toolpath_engagement(
                                &render_state.device,
                                &resources.gpu_limits,
                                render_tp,
                                tc.operation.feed_rate(),
                            )
                        }
                        crate::state::viewport::ToolpathColorMode::AdvancePerTooth => {
                            let band = advance_inputs
                                .as_ref()
                                .and_then(|(bands, _)| bands.get(&tc.id).copied());
                            let empty: HashMap<usize, AdvancePerToothMm> = HashMap::new();
                            let per_move = advance_inputs
                                .as_ref()
                                .and_then(|(_, m)| m.get(&tc.id))
                                .unwrap_or(&empty);
                            ToolpathGpuData::from_toolpath_advance_per_tooth(
                                &render_state.device,
                                &resources.gpu_limits,
                                render_tp,
                                band.as_ref(),
                                per_move,
                            )
                        }
                        crate::state::viewport::ToolpathColorMode::Normal => {
                            ToolpathGpuData::from_toolpath(
                                &render_state.device,
                                &resources.gpu_limits,
                                render_annotated,
                                i,
                                selected,
                                &state.viewport.span_kind_filter,
                            )
                        }
                    };
                    gpu_data.toolpath_id = Some(tc.id);

                    // Generate entry path preview for selected toolpaths with a non-None entry style
                    if selected {
                        use crate::state::toolpath::DressupEntryStyle;
                        let entry_style = match tc.dressups.entry_style {
                            DressupEntryStyle::None => toolpath_render::EntryStyle::None,
                            DressupEntryStyle::Ramp => toolpath_render::EntryStyle::Ramp,
                            DressupEntryStyle::Helix => toolpath_render::EntryStyle::Helix,
                        };
                        let height_ctx = height_context_from_session(session, tc);
                        let resolved = tc.heights.resolve(&height_ctx);
                        let config = toolpath_render::EntryPreviewConfig {
                            entry_style,
                            ramp_angle_deg: tc.dressups.ramp_angle,
                            helix_radius: tc.dressups.helix_radius,
                            helix_pitch: tc.dressups.helix_pitch,
                            lead_in_out: tc.dressups.lead_in_out,
                            lead_radius: tc.dressups.lead_radius,
                            // Heights resolve in the emission frame; shift
                            // alongside the toolpath the preview rides on.
                            feed_z: resolved.feed_z + display_shift.z,
                            top_z: resolved.top_z + display_shift.z,
                        };
                        let preview_verts =
                            toolpath_render::entry_preview_vertices(render_tp, &config);
                        gpu_data.attach_entry_preview(
                            &render_state.device,
                            &resources.gpu_limits,
                            &preview_verts,
                        );

                        // Tool-profile ghost overlay (optional).
                        if self.controller.state().viewport.show_tool_profile_preview
                            && let Some(tool) =
                                session.tools().iter().find(|t| t.id.0 == tc.tool_id)
                        {
                            let cutter = rs_cam_core::compute::build_cutter(tool);
                            let profile_verts =
                                toolpath_render::tool_profile_preview_vertices(render_tp, &cutter);
                            gpu_data.attach_tool_profile_preview(
                                &render_state.device,
                                &resources.gpu_limits,
                                &profile_verts,
                            );
                        }
                    }

                    resources.toolpath_data.push(gpu_data);
                }
            }
        }

        // Upload height plane overlays whenever a toolpath is selected (any workspace)
        if let Selection::Toolpath(tp_id) = self.controller.state().selection {
            let state = self.controller.state();
            let session = &state.session;
            if let Some((_, tc)) = session.find_toolpath_config_by_id(tp_id) {
                let height_ctx = height_context_from_session(session, tc);
                let heights = tc.heights.resolve(&height_ctx);
                // Use the same stock bbox as the rest of the viewport (local
                // or global). Heights resolve in the emission frame, so
                // identity setups shift by -origin_z to match it.
                let hp_stock_bbox = stock_bbox;
                resources.height_planes_data = Some(
                    crate::render::height_planes::HeightPlanesGpuData::from_heights(
                        &render_state.device,
                        &hp_stock_bbox,
                        heights.clearance_z + display_shift.z,
                        heights.retract_z + display_shift.z,
                        heights.feed_z + display_shift.z,
                        heights.top_z + display_shift.z,
                        heights.bottom_z + display_shift.z,
                    ),
                );
            } else {
                resources.height_planes_data = None;
            }
        } else {
            resources.height_planes_data = None;
        }

        // Upload rest-depth heatmap overlay (pencil detector #4) whenever the
        // selected toolpath carries a populated `rest_grid`. Mirrors the
        // height-plane upload gate directly above — rebuilt on the same
        // pending-upload cycle, not every frame (`upload_gpu_data` only runs
        // when `take_pending_upload()` fires, so this isn't a per-frame cost).
        {
            let rest_grid = if let Selection::Toolpath(tp_id) = self.controller.state().selection {
                let state = self.controller.state();
                state
                    .gui
                    .toolpath_rt
                    .get(&tp_id)
                    .and_then(|rt| rt.result.as_ref())
                    .and_then(|result| {
                        // Rest grids arrive in the emission frame alongside
                        // the toolpath they came from; re-frame in lockstep
                        // with the same display shift applied to toolpath
                        // lines above (identity-setup world→local shift).
                        if has_display_shift {
                            translate_annotated(&result.annotated, display_shift).rest_grid
                        } else {
                            result.annotated.rest_grid.clone()
                        }
                    })
            } else {
                None
            };
            resources.rest_heatmap_data = rest_grid.and_then(|grid| {
                rs_cam_core::rest_heatmap_mesh::rest_grid_to_heatmap_mesh(&grid).and_then(|hm| {
                    SimMeshGpuData::from_heightmap_mesh(
                        &render_state.device,
                        &resources.gpu_limits,
                        &hm,
                    )
                })
            });
        }
    }
}

/// Translate an annotated toolpath by `shift` — the display-frame adapter
/// for identity setups, whose toolpaths emit in world coordinates while the
/// viewport draws zero-rooted local. Delegates to
/// [`rs_cam_core::toolpath_spans::AnnotatedToolpath::translated`], which
/// re-frames every coordinate-bearing field (move targets, planner
/// engagement samples, rest-grid heatmap) in lockstep — arc center offsets
/// (`i`/`j` on the `MoveType`) are relative and survive translation
/// unchanged.
fn translate_annotated(
    annotated: &rs_cam_core::toolpath_spans::AnnotatedToolpath,
    shift: rs_cam_core::geo::P3,
) -> rs_cam_core::toolpath_spans::AnnotatedToolpath {
    annotated.translated(shift)
}

/// Build a `toolpath_id -> vendor chipload band` map from the matched LUT
/// row per toolpath. Toolpaths with no LUT match (custom material, no
/// vendor data for the tool/op family, etc.) are absent from the map; the
/// renderer falls back to grey.
///
/// The core helper returns a bare `Range<f64>`; this is the display
/// boundary where the range acquires its unit
/// (`VendorChiploadBand` — linear advance per tooth).
fn build_advance_bands(
    session: &rs_cam_core::session::ProjectSession,
    sim_trace: Option<&rs_cam_core::simulation_cut::SimulationCutTrace>,
) -> HashMap<rs_cam_core::ToolpathId, VendorChiploadBand> {
    rs_cam_core::tool_load::chipload_envelopes_for_session(session, sim_trace)
        .iter()
        .map(|(id, range)| (*id, VendorChiploadBand::from_advance_range(range)))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::translate_annotated;
    use rs_cam_core::geo::P3;
    use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};
    use rs_cam_core::toolpath_spans::AnnotatedToolpath;

    #[test]
    fn translate_annotated_shifts_targets_and_preserves_arc_offsets() {
        let mut tp = Toolpath::new();
        tp.moves.push(Move {
            target: P3::new(1.0, 2.0, 3.0),
            move_type: MoveType::Rapid,
            intent: MoveIntent::Unknown,
        });
        tp.moves.push(Move {
            target: P3::new(4.0, 5.0, -2.0),
            move_type: MoveType::ArcCW {
                i: 0.5,
                j: -0.5,
                feed_rate: 1000.0,
            },
            intent: MoveIntent::Unknown,
        });
        let annotated = AnnotatedToolpath {
            toolpath: tp,
            spans: Vec::new(),
            spans_valid: true,
            planner_engagement: Vec::new(),
            rest_grid: None,
            rest_regions: None,
        };

        let shifted = translate_annotated(&annotated, P3::new(0.0, 0.0, 19.0));
        assert_eq!(shifted.toolpath.moves[0].target.z, 22.0);
        assert_eq!(shifted.toolpath.moves[1].target.z, 17.0);
        // Arc center offsets are relative — translation must not touch them.
        match shifted.toolpath.moves[1].move_type {
            MoveType::ArcCW { i, j, .. } => {
                assert_eq!((i, j), (0.5, -0.5));
            }
            _ => panic!("arc move type changed"),
        }
        // Original untouched.
        assert_eq!(annotated.toolpath.moves[0].target.z, 3.0);
    }
}
