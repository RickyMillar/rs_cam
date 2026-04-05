use crate::render::{LineUniforms, MeshUniforms, ViewportCallback};
use crate::state::Workspace;
use crate::state::selection::Selection;
use crate::ui::AppEvent;

use super::RsCamApp;

impl RsCamApp {
    /// Click-to-select: find nearest toolpath to click position in screen space.
    fn handle_viewport_click(&mut self, click_pos: egui::Pos2) {
        let rect = self.viewport_rect;
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }

        if self.handle_simulation_semantic_pick(click_pos) {
            return;
        }

        let ctx = crate::interaction::picking::PickContext {
            camera: &self.camera,
            screen_x: click_pos.x - rect.min.x,
            screen_y: click_pos.y - rect.min.y,
            aspect: rect.width() / rect.height(),
            vw: rect.width(),
            vh: rect.height(),
        };

        let workspace = self.controller.state().workspace;
        let isolate = self.controller.state().viewport.isolate_toolpath;

        let hit = crate::interaction::picking::pick(
            &ctx,
            &self.controller.state().job,
            self.controller.collision_positions(),
            workspace,
            isolate,
        );

        if let Some(hit) = hit {
            self.handle_pick_hit(hit);
        } else {
            self.controller.state_mut().selection = Selection::None;
        }
    }

    fn handle_simulation_semantic_pick(&mut self, click_pos: egui::Pos2) -> bool {
        if self.controller.state().workspace != Workspace::Simulation
            || !self.controller.state().simulation.debug.enabled
        {
            return false;
        }

        let rect = self.viewport_rect;
        let Some((ray_origin, ray_dir)) = self.camera.unproject_ray(
            click_pos.x - rect.min.x,
            click_pos.y - rect.min.y,
            rect.width() / rect.height(),
            rect.width(),
            rect.height(),
        ) else {
            return false;
        };

        let target = {
            let state = self.controller.state_mut();
            state.simulation.pick_semantic_item_with_ray(
                &state.job,
                &rs_cam_core::geo::P3::new(
                    ray_origin.x as f64,
                    ray_origin.y as f64,
                    ray_origin.z as f64,
                ),
                &rs_cam_core::geo::V3::new(ray_dir.x as f64, ray_dir.y as f64, ray_dir.z as f64),
            )
        };

        if let Some(target) = target {
            {
                let state = self.controller.state_mut();
                if let Some(item_id) = target.semantic_item_id {
                    state
                        .simulation
                        .pin_semantic_item(target.toolpath_id, item_id);
                }
                state.simulation.debug.focused_issue_index = None;
                state.simulation.debug.focused_hotspot = None;
            }
            self.controller
                .events_mut()
                .push(AppEvent::SimJumpToMove(target.move_index));
            return true;
        }

        false
    }

    fn handle_pick_hit(&mut self, hit: crate::interaction::PickHit) {
        use crate::interaction::PickHit;

        let workspace = self.controller.state().workspace;
        match (workspace, hit) {
            (
                Workspace::Setup,
                PickHit::Fixture {
                    setup_id,
                    fixture_id,
                },
            ) => {
                self.controller.state_mut().selection = Selection::Fixture(setup_id, fixture_id);
            }
            (
                Workspace::Setup,
                PickHit::KeepOut {
                    setup_id,
                    keep_out_id,
                },
            ) => {
                self.controller.state_mut().selection = Selection::KeepOut(setup_id, keep_out_id);
            }
            (Workspace::Setup | Workspace::Toolpaths, PickHit::AlignmentPin { .. }) => {
                // Pins are stock-level; selecting a pin selects the stock.
                self.controller.state_mut().selection = Selection::Stock;
            }
            (_, PickHit::StockFace { .. }) => {
                self.controller.state_mut().selection = Selection::Stock;
            }
            (Workspace::Simulation, PickHit::CollisionMarker { index }) => {
                // Jump playback to the collision move
                if let Some(&move_idx) = self
                    .controller
                    .state()
                    .simulation
                    .checks
                    .rapid_collision_move_indices
                    .get(index)
                {
                    let pb = &mut self.controller.state_mut().simulation.playback;
                    pb.current_move = move_idx;
                    pb.playing = false;
                    self.pending_checkpoint_load = true;
                }
            }
            (_, PickHit::Toolpath { id }) => {
                self.controller.state_mut().selection = Selection::Toolpath(id);
            }
            (Workspace::Toolpaths, PickHit::ModelFace { model_id, face_id }) => {
                // Route face toggle through controller event for undo support.
                // The controller handler also updates visual selection and pending_upload.
                if let Selection::Toolpath(tp_id) = self.controller.state().selection {
                    self.controller
                        .events_mut()
                        .push(crate::ui::AppEvent::ToggleFaceSelection {
                            toolpath_id: tp_id,
                            model_id,
                            face_id,
                        });
                }
            }
            _ => {}
        }
    }

    pub(super) fn draw_viewport(&mut self, ui: &mut egui::Ui) {
        let lane_snapshots = self.controller.lane_snapshots();
        let workspace = self.controller.state().workspace;
        let sim_active = self.controller.state().simulation.has_results();
        {
            let (state, events) = self.controller.state_and_events_mut();
            crate::ui::viewport_overlay::draw(
                ui,
                workspace,
                sim_active,
                &mut state.viewport,
                &lane_snapshots,
                events,
            );
        }

        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        self.viewport_rect = rect;

        // Click-to-select toolpath in viewport
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.handle_viewport_click(pos);
        }

        // Update hovered face for BREP hover highlighting
        self.last_hover_face = None;
        if response.hovered()
            && self.controller.state().workspace == Workspace::Toolpaths
            && self
                .controller
                .state()
                .job
                .models
                .iter()
                .any(|m| m.enriched_mesh.is_some())
            && let Some(pos) = ui.input(|i| i.pointer.hover_pos())
        {
            let pick_ctx = crate::interaction::picking::PickContext {
                camera: &self.camera,
                screen_x: pos.x - rect.min.x,
                screen_y: pos.y - rect.min.y,
                aspect: if rect.height() > 0.0 {
                    rect.width() / rect.height()
                } else {
                    1.0
                },
                vw: rect.width(),
                vh: rect.height(),
            };
            let state = self.controller.state();
            if let Some(crate::interaction::picking::PickHit::ModelFace { face_id, .. }) =
                crate::interaction::picking::pick(
                    &pick_ctx,
                    &state.job,
                    self.controller.collision_positions(),
                    state.workspace,
                    state.viewport.isolate_toolpath,
                )
            {
                self.last_hover_face = Some(face_id);
            }
        }

        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = response.drag_delta();
            self.camera.orbit(delta.x, delta.y);
        }
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            let delta = response.drag_delta();
            self.camera.pan(delta.x, delta.y);
        }

        let scroll_raw = ui.input(|i| i.smooth_scroll_delta.y);
        if rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()))
            && scroll_raw != 0.0
        {
            // Normalize scroll direction: use signum for consistent zoom
            // across platforms, then scale by the absolute magnitude clamped
            // to a reasonable range so track-pad and mouse-wheel both feel right.
            let magnitude = scroll_raw.abs().clamp(1.0, 120.0);
            let scroll = scroll_raw.signum() * magnitude;
            self.camera.zoom(scroll);
        }

        let aspect = if rect.height() > 0.0 {
            rect.width() / rect.height()
        } else {
            1.0
        };
        let view_proj = self.camera.view_proj(aspect);
        let eye = self.camera.eye();
        let ppp = ui.ctx().pixels_per_point();
        let state = self.controller.state();

        let callback = ViewportCallback {
            mesh_uniforms: MeshUniforms {
                view_proj,
                light_dir: [0.5, 0.3, 0.8],
                _pad0: 0.0,
                camera_pos: [eye.x, eye.y, eye.z],
                _pad1: 0.0,
            },
            line_uniforms: LineUniforms { view_proj },
            has_mesh: state.job.models.iter().any(|model| model.mesh.is_some())
                && state.viewport.render_mode == crate::state::viewport::RenderMode::Shaded
                && state.workspace != Workspace::Simulation,
            show_grid: state.viewport.show_grid,
            show_stock: state.viewport.show_stock
                && state.job.models.iter().any(|model| model.mesh.is_some()),
            show_fixtures: state.viewport.show_fixtures
                && (state.workspace != Workspace::Simulation
                    || !state.job.stock.alignment_pins.is_empty()),
            show_solid_stock: state.viewport.show_stock && state.workspace == Workspace::Setup,
            show_height_planes: state.workspace == Workspace::Toolpaths
                && matches!(state.selection, Selection::Toolpath(_)),
            show_sim_mesh: state.workspace == Workspace::Simulation
                && state.simulation.has_results(),
            sim_mesh_opacity: state.simulation.stock_opacity,
            show_cutting: state.viewport.show_cutting,
            show_rapids: state.viewport.show_rapids,
            show_collisions: state.viewport.show_collisions,
            show_tool_model: state.workspace == Workspace::Simulation
                && state.simulation.has_results()
                && state.simulation.playback.tool_position.is_some(),
            toolpath_move_limit: if state.workspace == Workspace::Simulation
                && state.simulation.has_results()
                && state.simulation.playback.current_move < state.simulation.total_moves()
            {
                Some(state.simulation.playback.current_move)
            } else {
                None
            },
            show_origin_axes: state.viewport.show_stock
                && state.job.models.iter().any(|model| model.mesh.is_some()),
            origin_axes_origin: [
                state.job.stock.origin_x as f32,
                state.job.stock.origin_y as f32,
                state.job.stock.origin_z as f32,
            ],
            origin_axes_length: {
                let s = &state.job.stock;
                let min_dim = s.x.min(s.y).min(s.z) as f32;
                (min_dim * 0.3).clamp(5.0, 50.0)
            },
            viewport_width: (rect.width() * ppp) as u32,
            viewport_height: (rect.height() * ppp) as u32,
        };

        let cb = egui_wgpu::Callback::new_paint_callback(rect, callback);
        ui.painter().add(cb);

        // Draw orientation gizmo overlay (2D, on top of the 3D viewport)
        self.draw_orientation_gizmo(ui, rect);

        if workspace == Workspace::Simulation {
            let active_overlay = {
                let state = self.controller.state_mut();
                if state.simulation.debug.enabled && state.simulation.debug.highlight_active_item {
                    state
                        .simulation
                        .active_semantic_item(&state.job)
                        .and_then(|active| {
                            state
                                .simulation
                                .semantic_item_bbox_in_simulation(
                                    &state.job,
                                    active.toolpath_id,
                                    &active.item,
                                )
                                .map(|bbox| (active.item.label.clone(), bbox))
                        })
                } else {
                    None
                }
            };
            if let Some((label, bbox)) = active_overlay.as_ref() {
                self.draw_semantic_item_overlay(ui, rect, label, bbox);
            }
        }
    }

    /// Draw a small XYZ orientation gizmo in the top-right corner of the viewport.
    /// Uses the camera view matrix to rotate unit vectors, then draws 2D lines.
    fn draw_orientation_gizmo(&self, ui: &mut egui::Ui, viewport_rect: egui::Rect) {
        let gizmo_size = 50.0;
        let margin = 10.0;
        let gizmo_center = egui::pos2(
            viewport_rect.max.x - margin - gizmo_size * 0.5,
            viewport_rect.min.y + margin + gizmo_size * 0.5,
        );
        let axis_len = 20.0;

        let view = self.camera.view_matrix();
        let painter = ui.painter();

        // Background circle for readability
        painter.circle_filled(
            gizmo_center,
            gizmo_size * 0.5,
            egui::Color32::from_rgba_premultiplied(20, 20, 30, 160),
        );

        let axes: [(nalgebra::Vector3<f32>, egui::Color32, &str); 3] = [
            (
                nalgebra::Vector3::new(1.0, 0.0, 0.0),
                egui::Color32::from_rgb(220, 60, 60),
                "X",
            ),
            (
                nalgebra::Vector3::new(0.0, 1.0, 0.0),
                egui::Color32::from_rgb(60, 200, 60),
                "Y",
            ),
            (
                nalgebra::Vector3::new(0.0, 0.0, 1.0),
                egui::Color32::from_rgb(70, 100, 230),
                "Z",
            ),
        ];

        // Sort axes by depth (draw back-to-front)
        let mut axis_data: Vec<(f32, egui::Pos2, egui::Color32, &str)> = axes
            .iter()
            .map(|(axis, color, label)| {
                let rotated = view.transform_vector(axis);
                let screen_end =
                    gizmo_center + egui::vec2(rotated.x * axis_len, -rotated.y * axis_len);
                // Use Z for depth sorting (more negative = further back)
                (rotated.z, screen_end, *color, *label)
            })
            .collect();
        axis_data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        for (_, end, color, label) in &axis_data {
            painter.line_segment([gizmo_center, *end], egui::Stroke::new(2.0, *color));
            painter.text(
                *end,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(10.0),
                *color,
            );
        }
    }

    // SAFETY: edge indices are compile-time constants 0..7 into an 8-element projected array
    #[allow(clippy::indexing_slicing)]
    fn draw_semantic_item_overlay(
        &self,
        ui: &mut egui::Ui,
        viewport_rect: egui::Rect,
        label: &str,
        bbox: &rs_cam_core::geo::BoundingBox3,
    ) {
        let width = viewport_rect.width().max(1.0);
        let height = viewport_rect.height().max(1.0);
        let aspect = width / height;
        let corners = [
            [bbox.min.x as f32, bbox.min.y as f32, bbox.min.z as f32],
            [bbox.max.x as f32, bbox.min.y as f32, bbox.min.z as f32],
            [bbox.max.x as f32, bbox.max.y as f32, bbox.min.z as f32],
            [bbox.min.x as f32, bbox.max.y as f32, bbox.min.z as f32],
            [bbox.min.x as f32, bbox.min.y as f32, bbox.max.z as f32],
            [bbox.max.x as f32, bbox.min.y as f32, bbox.max.z as f32],
            [bbox.max.x as f32, bbox.max.y as f32, bbox.max.z as f32],
            [bbox.min.x as f32, bbox.max.y as f32, bbox.max.z as f32],
        ];
        let projected: Vec<_> = corners
            .iter()
            .map(|corner| {
                self.camera
                    .project_to_screen(*corner, aspect, width, height)
                    .map(|point| {
                        egui::pos2(
                            viewport_rect.min.x + point[0],
                            viewport_rect.min.y + point[1],
                        )
                    })
            })
            .collect();
        let edges = [
            (0usize, 1usize),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ];
        let color = egui::Color32::from_rgb(255, 210, 120);
        let painter = ui.painter();
        for (start_idx, end_idx) in edges {
            if let (Some(start), Some(end)) = (projected[start_idx], projected[end_idx]) {
                painter.line_segment([start, end], egui::Stroke::new(1.5, color));
            }
        }

        if let Some(anchor) = projected.iter().flatten().next().copied() {
            painter.text(
                anchor + egui::vec2(6.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::FontId::proportional(12.0),
                color,
            );
        }
    }
}
