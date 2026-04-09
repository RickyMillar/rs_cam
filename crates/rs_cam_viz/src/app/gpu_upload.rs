use std::sync::Arc;

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
                .find_toolpath_config_by_id(tp_id.0)
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
            StockVizMode::ByOperation => sim_render::operation_placeholder_colors(num_verts),
        }
    }

    /// Transform a global-frame `StockMesh` to the active setup's local
    /// frame for the given simulation `move_idx`.  No-op for identity setups.
    // SAFETY: step_by(3) loop with i+1, i+2 bounded by vertices.len() (always multiple of 3)
    #[allow(clippy::indexing_slicing)]
    pub(super) fn transform_mesh_to_local_frame(
        &self,
        mesh: &mut rs_cam_core::simulation::StockMesh,
        move_idx: usize,
    ) {
        if let Some((face_up, z_rot, true)) = self.active_setup_orientation(move_idx) {
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
                        .setup_of_toolpath_id(tp_id.0)
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

        // Upload polygon/DXF/SVG line data
        {
            use crate::render::{LineVertex, PolygonGpuData};
            use egui_wgpu::wgpu::util::DeviceExt;
            resources.polygon_data.clear();
            let color = crate::render::colors::POLYGON_OUTLINE;
            // Slightly above stock top to avoid z-fighting with mesh.
            let poly_z = if use_local_frame {
                // SAFETY: use_local_frame iff active_setup_ref.is_some()
                #[allow(clippy::unwrap_used)]
                let setup = active_setup_ref.as_ref().unwrap();
                let (_, _, h) = setup.effective_stock(&stock);
                h as f32 + 0.05
            } else {
                (stock.origin_z + stock.z) as f32 + 0.05
            };
            // Convert a 2D ring to line-list vertices.
            // `close`: if true, adds closing edge from last→first vertex.
            let ring_to_lines =
                |ring: &[rs_cam_core::geo::P2], close: bool, verts: &mut Vec<LineVertex>| {
                    if ring.len() < 2 {
                        return;
                    }
                    let transform_pt = |p: &rs_cam_core::geo::P2| -> (f64, f64) {
                        if use_local_frame {
                            // SAFETY: use_local_frame iff active_setup_ref.is_some()
                            #[allow(clippy::unwrap_used)]
                            let setup = active_setup_ref.as_ref().unwrap();
                            let tp = setup
                                .transform_point(rs_cam_core::geo::P3::new(p.x, p.y, 0.0), &stock);
                            (tp.x, tp.y)
                        } else {
                            (p.x, p.y)
                        }
                    };
                    // Consecutive edges
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
                    // Close the ring: last → first (only for closed polygons)
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
                if let Some(polys) = &model.polygons {
                    let mut verts = Vec::new();
                    for poly in polys.iter() {
                        ring_to_lines(&poly.exterior, poly.closed, &mut verts);
                        for hole in &poly.holes {
                            ring_to_lines(hole, true, &mut verts);
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
                use crate::state::runtime::{Corner, XYDatum};

                let (eff_w, eff_d, eff_h) = setup.effective_stock(&stock);
                let color = [0.9_f32, 0.2, 0.9]; // magenta

                // Read datum from SetupRuntime
                let datum = state.gui.setup_rt.get(&sd.id).map(|sr| &sr.datum);

                // Datum in setup-local frame: XY at corner/center, Z at top surface
                let local_datum: Option<P3> = datum.and_then(|d| match &d.xy_method {
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
                });

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

        // Re-upload sim mesh with current viz mode colors, or clear if no results
        if self.controller.state().simulation.has_results() {
            if let Some(mesh) = &self.controller.state().simulation.playback.display_mesh {
                let colors = self.compute_sim_colors(mesh);
                let new_data = SimMeshGpuData::from_heightmap_mesh_colored(
                    &render_state.device,
                    &resources.gpu_limits,
                    mesh,
                    &colors,
                );
                resources.sim_mesh_data = new_data;
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
                let tp_id = crate::state::toolpath::ToolpathId(tc.id);
                let visible = rt.is_none_or(|r| r.visible)
                    && match isolate {
                        Some(iso_id) => tp_id == iso_id,
                        None => true,
                    };
                let result = rt.and_then(|r| r.result.as_ref());
                if visible && let Some(result) = result {
                    let selected = selected_tp_id == Some(tp_id);

                    // In Setup/Toolpaths workspace (local frame), toolpaths are already
                    // Toolpaths are always in local coords, viewport is always in
                    // local frame — use directly, no transform needed.
                    let render_tp = result.toolpath.as_ref();

                    let color_mode = state.viewport.toolpath_color_mode;
                    let mut gpu_data = if matches!(
                        color_mode,
                        crate::state::viewport::ToolpathColorMode::Engagement
                    ) {
                        ToolpathGpuData::from_toolpath_engagement(
                            &render_state.device,
                            &resources.gpu_limits,
                            render_tp,
                            tc.operation.feed_rate(),
                        )
                    } else {
                        ToolpathGpuData::from_toolpath(
                            &render_state.device,
                            &resources.gpu_limits,
                            render_tp,
                            i,
                            selected,
                        )
                    };

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
                            feed_z: resolved.feed_z,
                            top_z: resolved.top_z,
                        };
                        let preview_verts =
                            toolpath_render::entry_preview_vertices(&result.toolpath, &config);
                        gpu_data.attach_entry_preview(
                            &render_state.device,
                            &resources.gpu_limits,
                            &preview_verts,
                        );
                    }

                    resources.toolpath_data.push(gpu_data);
                }
            }
        }

        // Upload height plane overlays whenever a toolpath is selected (any workspace)
        if let Selection::Toolpath(tp_id) = self.controller.state().selection {
            let state = self.controller.state();
            let session = &state.session;
            if let Some((_, tc)) = session.find_toolpath_config_by_id(tp_id.0) {
                let height_ctx = height_context_from_session(session, tc);
                let heights = tc.heights.resolve(&height_ctx);
                // Use the same stock bbox as the rest of the viewport (local or global)
                let hp_stock_bbox = stock_bbox;
                resources.height_planes_data = Some(
                    crate::render::height_planes::HeightPlanesGpuData::from_heights(
                        &render_state.device,
                        &hp_stock_bbox,
                        heights.clearance_z,
                        heights.retract_z,
                        heights.feed_z,
                        heights.top_z,
                        heights.bottom_z,
                    ),
                );
            } else {
                resources.height_planes_data = None;
            }
        } else {
            resources.height_planes_data = None;
        }
    }
}
