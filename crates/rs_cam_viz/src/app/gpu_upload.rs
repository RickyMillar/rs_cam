use std::sync::Arc;

use crate::render::RenderResources;
use crate::render::mesh_render::MeshGpuData;
use crate::render::sim_render::{self, SimMeshGpuData};
use crate::render::stock_render::StockGpuData;
use crate::render::toolpath_render::{self, ToolpathGpuData};
use crate::state::Workspace;
use crate::state::job::transform_mesh;
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
                .job
                .find_toolpath(*tp_id)
                .and_then(|entry| entry.face_selection.clone())
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
            StockVizMode::ByHeight => sim_render::height_gradient_colors(&mesh.vertices),
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
            let stock_cfg = &self.controller.state().job.stock;
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
        let active_setup_ref = {
            let sel = &self.controller.state().selection;
            let setup_id = match sel {
                Selection::Setup(id) => Some(*id),
                Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
                Selection::Toolpath(tp_id) => self.controller.state().job.setup_of_toolpath(*tp_id),
                _ => None,
            };
            if let Some(sid) = setup_id {
                self.controller
                    .state()
                    .job
                    .setups
                    .iter()
                    .find(|s| s.id == sid)
            } else {
                self.controller.state().job.setups.first()
            }
        };
        let use_local_frame = active_setup_ref.is_some();

        // Upload mesh data for all models with geometry
        resources.enriched_mesh_data_list.clear();
        resources.mesh_data_list.clear();
        let selected_faces = self.selected_face_ids();
        let hovered_face = self.hovered_face_id();
        for model in &self.controller.state().job.models {
            // If model has enriched mesh (STEP), use face-colored rendering
            if let Some(enriched) = &model.enriched_mesh {
                let transform: crate::render::mesh_render::VertexTransform<'_> = if use_local_frame
                {
                    // SAFETY: use_local_frame is active_setup_ref.is_some()
                    #[allow(clippy::unwrap_used)]
                    let setup = active_setup_ref.unwrap();
                    let stock = &self.controller.state().job.stock;
                    Some(Box::new(move |p| setup.transform_point(p, stock)))
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
                    let setup = active_setup_ref.unwrap();
                    let transformed =
                        transform_mesh(mesh, setup, &self.controller.state().job.stock);
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
                let setup = active_setup_ref.unwrap();
                let (_, _, h) = setup.effective_stock(&self.controller.state().job.stock);
                h as f32 + 0.05
            } else {
                (self.controller.state().job.stock.origin_z + self.controller.state().job.stock.z)
                    as f32
                    + 0.05
            };
            // Convert a 2D ring to line-list vertices, closing the loop.
            let ring_to_lines = |ring: &[rs_cam_core::geo::P2], verts: &mut Vec<LineVertex>| {
                if ring.len() < 2 {
                    return;
                }
                // Consecutive edges: a→b for windows, plus closing edge last→first.
                for pair in ring.windows(2) {
                    // SAFETY: windows(2) guarantees exactly 2 elements per slice.
                    #[allow(clippy::indexing_slicing)]
                    let (a, b) = (&pair[0], &pair[1]);
                    let (ax, ay, bx, by) = if use_local_frame {
                        // SAFETY: use_local_frame iff active_setup_ref.is_some()
                        #[allow(clippy::unwrap_used)]
                        let setup = active_setup_ref.unwrap();
                        let stock = &self.controller.state().job.stock;
                        let pa =
                            setup.transform_point(rs_cam_core::geo::P3::new(a.x, a.y, 0.0), stock);
                        let pb =
                            setup.transform_point(rs_cam_core::geo::P3::new(b.x, b.y, 0.0), stock);
                        (pa.x, pa.y, pb.x, pb.y)
                    } else {
                        (a.x, a.y, b.x, b.y)
                    };
                    verts.push(LineVertex {
                        position: [ax as f32, ay as f32, poly_z],
                        color,
                    });
                    verts.push(LineVertex {
                        position: [bx as f32, by as f32, poly_z],
                        color,
                    });
                }
                // Close the ring: last → first
                if let (Some(last), Some(first)) = (ring.last(), ring.first()) {
                    let (ax, ay, bx, by) = if use_local_frame {
                        #[allow(clippy::unwrap_used)]
                        let setup = active_setup_ref.unwrap();
                        let stock = &self.controller.state().job.stock;
                        let pa = setup
                            .transform_point(rs_cam_core::geo::P3::new(last.x, last.y, 0.0), stock);
                        let pb = setup.transform_point(
                            rs_cam_core::geo::P3::new(first.x, first.y, 0.0),
                            stock,
                        );
                        (pa.x, pa.y, pb.x, pb.y)
                    } else {
                        (last.x, last.y, first.x, first.y)
                    };
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
            for model in &self.controller.state().job.models {
                if let Some(polys) = &model.polygons {
                    let mut verts = Vec::new();
                    for poly in polys.iter() {
                        ring_to_lines(&poly.exterior, &mut verts);
                        for hole in &poly.holes {
                            ring_to_lines(hole, &mut verts);
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
            let setup = active_setup_ref.unwrap();
            let (w, d, h) = setup.effective_stock(&self.controller.state().job.stock);
            rs_cam_core::geo::BoundingBox3 {
                min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                max: rs_cam_core::geo::P3::new(w, d, h),
            }
        } else {
            self.controller.state().job.stock.bbox()
        };
        resources.stock_data = Some(StockGpuData::from_bbox(&render_state.device, &stock_bbox));
        resources.solid_stock_data =
            Some(crate::render::stock_render::SolidStockGpuData::from_bbox(
                &render_state.device,
                &stock_bbox,
            ));

        // Upload origin axes at stock origin (local origin when in machine view)
        {
            let s = &self.controller.state().job.stock;
            let origin = if use_local_frame {
                [0.0_f32, 0.0, 0.0]
            } else {
                [s.origin_x as f32, s.origin_y as f32, s.origin_z as f32]
            };
            let min_dim = s.x.min(s.y).min(s.z) as f32;
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

            let job = &self.controller.state().job;
            let selection = &self.controller.state().selection;

            // Helper: forward-transform a bbox into the setup's local frame.
            // After transforming corners, min/max may swap, so rebuild via from_points.
            let transform_bbox =
                |bb: BoundingBox3, setup: &crate::state::job::Setup| -> BoundingBox3 {
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
                    BoundingBox3::from_points(
                        corners
                            .iter()
                            .map(|c| setup.transform_point(*c, &job.stock)),
                    )
                };

            // Only show fixtures/keepouts/pins from the active setup (each
            // setup has its own local frame, so mixing them is wrong).
            let active_setups: Vec<&crate::state::job::Setup> =
                active_setup_ref.into_iter().collect();

            // Fixture/keep-out boxes are only shown outside Simulation.
            let mut boxes = Vec::new();
            let in_sim = self.controller.state().workspace == Workspace::Simulation;
            if !in_sim {
                for setup in &active_setups {
                    for fixture in &setup.fixtures {
                        if fixture.enabled {
                            let selected = *selection == Selection::Fixture(setup.id, fixture.id);
                            let color = if selected {
                                [1.0_f32, 0.9, 0.4] // bright highlight
                            } else {
                                [0.9_f32, 0.7, 0.2]
                            };
                            let clearance = fixture.clearance_bbox();
                            let display_clearance = transform_bbox(clearance, setup);
                            boxes.push((display_clearance, color));
                            if selected {
                                let inner = fixture.bbox();
                                let display_inner = transform_bbox(inner, setup);
                                boxes.push((display_inner, [0.9_f32, 0.9, 0.9]));
                            }
                        }
                    }
                    for keep_out in &setup.keep_out_zones {
                        if keep_out.enabled {
                            let selected = *selection == Selection::KeepOut(setup.id, keep_out.id);
                            let color = if selected {
                                [1.0_f32, 0.4, 0.4] // bright highlight
                            } else {
                                [0.9_f32, 0.2, 0.2]
                            };
                            let ko_bb = keep_out.bbox(&job.stock);
                            let display_ko = transform_bbox(ko_bb, setup);
                            boxes.push((display_ko, color));
                        }
                    }
                }
            }

            // Render stock-level alignment pins as circles (visible in all setups).
            // Pin coords are stock-relative — add origin to get global for transform_point.
            let mut pin_vertices: Vec<crate::render::LineVertex> = Vec::new();
            let ox = job.stock.origin_x;
            let oy = job.stock.origin_y;
            let oz = job.stock.origin_z;
            for setup in &active_setups {
                for pin in &job.stock.alignment_pins {
                    let radius = (pin.diameter / 2.0) as f32;
                    // Slight Z offset above stock top to avoid Z-fighting with stock surface.
                    let global_pt = P3::new(pin.x + ox, pin.y + oy, oz + job.stock.z + 0.1);
                    let local_pt = setup.transform_point(global_pt, &job.stock);
                    let (cx, cy, cz) = (local_pt.x as f32, local_pt.y as f32, local_pt.z as f32);
                    let color = [0.2_f32, 0.9, 0.3];
                    super::push_circle_vertices(&mut pin_vertices, cx, cy, cz, radius, color, 16);
                }

                // Flip axis dashed centerline
                if let Some(axis) = job.stock.flip_axis {
                    let stock_top = oz + job.stock.z;
                    let (start_g, end_g) = match axis {
                        crate::state::job::FlipAxis::Horizontal => {
                            let y = job.stock.y / 2.0 + oy;
                            (
                                P3::new(ox, y, stock_top),
                                P3::new(ox + job.stock.x, y, stock_top),
                            )
                        }
                        crate::state::job::FlipAxis::Vertical => {
                            let x = job.stock.x / 2.0 + ox;
                            (
                                P3::new(x, oy, stock_top),
                                P3::new(x, oy + job.stock.y, stock_top),
                            )
                        }
                    };
                    let start_l = setup.transform_point(start_g, &job.stock);
                    let end_l = setup.transform_point(end_g, &job.stock);
                    let s = [start_l.x as f32, start_l.y as f32, start_l.z as f32];
                    let e = [end_l.x as f32, end_l.y as f32, end_l.z as f32];
                    let axis_color = [0.9_f32, 0.7, 0.2];
                    super::push_dashed_line_vertices(&mut pin_vertices, s, e, axis_color, 5.0, 3.0);
                }
            }

            // Add datum crosshair markers in Setup workspace.
            // Datum is always in local frame coords.
            if self.controller.state().workspace == Workspace::Setup
                && let Some(setup) = active_setup_ref
            {
                use crate::state::job::{Corner, XYDatum};

                let (eff_w, eff_d, eff_h) = setup.effective_stock(&job.stock);
                let color = [0.9_f32, 0.2, 0.9]; // magenta

                // Datum in setup-local frame: XY at corner/center, Z at top surface
                let local_datum: Option<P3> = match &setup.datum.xy_method {
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

        // Upload collision markers as red crosses
        if !self.controller.collision_positions().is_empty() {
            use crate::render::LineVertex;
            let s = 1.0f32; // marker size in mm
            let color = [0.95, 0.15, 0.15];
            let vertex_size = std::mem::size_of::<LineVertex>();
            let mut verts = Vec::new();
            for p in self.controller.collision_positions() {
                // Cap marker count to stay within GPU buffer limits
                if verts.len() * vertex_size >= resources.gpu_limits.max_buffer_size {
                    tracing::warn!(
                        markers = self.controller.collision_positions().len(),
                        "Too many collision markers — truncating to fit GPU buffer"
                    );
                    break;
                }
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
        let active_setup_id = active_setup_ref.map(|s| s.id);

        for (i, tp) in self.controller.state().job.toolpaths_enumerated() {
            // Only show toolpaths from the active setup — each setup has its
            // own local coordinate frame, so mixing them would be wrong.
            {
                let tp_setup = self.controller.state().job.setup_of_toolpath(tp.id);
                if tp_setup != active_setup_id {
                    continue;
                }
            }

            // Skip invisible toolpaths; also skip if not the isolated toolpath
            let visible = tp.visible
                && match isolate {
                    Some(iso_id) => tp.id == iso_id,
                    None => true,
                };
            if visible && let Some(result) = &tp.result {
                let selected = selected_tp_id == Some(tp.id);

                // In Setup/Toolpaths workspace (local frame), toolpaths are already
                // Toolpaths are always in local coords, viewport is always in
                // local frame — use directly, no transform needed.
                let render_tp = result.toolpath.as_ref();

                let mut gpu_data = ToolpathGpuData::from_toolpath(
                    &render_state.device,
                    &resources.gpu_limits,
                    render_tp,
                    i,
                    selected,
                );

                // Generate entry path preview for selected toolpaths with a non-None entry style
                if selected {
                    use crate::state::toolpath::DressupEntryStyle;
                    let entry_style = match tp.dressups.entry_style {
                        DressupEntryStyle::None => toolpath_render::EntryStyle::None,
                        DressupEntryStyle::Ramp => toolpath_render::EntryStyle::Ramp,
                        DressupEntryStyle::Helix => toolpath_render::EntryStyle::Helix,
                    };
                    let height_ctx = self.controller.state().job.height_context_for(tp);
                    let resolved = tp.heights.resolve(&height_ctx);
                    let config = toolpath_render::EntryPreviewConfig {
                        entry_style,
                        ramp_angle_deg: tp.dressups.ramp_angle,
                        helix_radius: tp.dressups.helix_radius,
                        helix_pitch: tp.dressups.helix_pitch,
                        lead_in_out: tp.dressups.lead_in_out,
                        lead_radius: tp.dressups.lead_radius,
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

        // Upload height plane overlays whenever a toolpath is selected (any workspace)
        if let Selection::Toolpath(tp_id) = self.controller.state().selection {
            let job = &self.controller.state().job;
            if let Some(tp) = job.all_toolpaths().find(|t| t.id == tp_id) {
                let height_ctx = job.height_context_for(tp);
                let heights = tp.heights.resolve(&height_ctx);
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
