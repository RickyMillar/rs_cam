use std::time::{Duration, Instant};

use crate::render::RenderResources;
use crate::render::sim_render::SimMeshGpuData;

use super::RsCamApp;

impl RsCamApp {
    /// Load the nearest checkpoint mesh for backward scrubbing.
    pub(super) fn load_checkpoint_for_move(&mut self, move_idx: usize, frame: &mut eframe::Frame) {
        if let Some(cp_idx) = self
            .controller
            .state()
            .simulation
            .checkpoint_for_move(move_idx)
        {
            let mut mesh = match self.controller.state().simulation.checkpoints().get(cp_idx) {
                Some(c) => c.mesh().clone(),
                None => return,
            };
            // Checkpoint mesh is in global stock frame — transform to active
            // setup's local frame so it matches the tool position.
            self.transform_mesh_to_local_frame(&mut mesh, move_idx);
            let colors = self.compute_sim_colors(&mesh);
            {
                let pb = &mut self.controller.state_mut().simulation.playback;
                pb.display_mesh = Some(mesh);
                pb.display_mesh_move = Some(move_idx);
                pb.display_mesh_preview = false;
                pb.last_mesh_upload_at = Some(Instant::now());
            }
            if let Some(rs) = frame.wgpu_render_state() {
                // SAFETY: display_mesh was set to Some on the line above.
                #[allow(clippy::unwrap_used)]
                let mesh_ref = self
                    .controller
                    .state()
                    .simulation
                    .playback
                    .display_mesh
                    .as_ref()
                    .unwrap();
                let mut renderer = rs.renderer.write();
                // SAFETY: RenderResources inserted in RsCamApp::new; always present.
                #[allow(clippy::unwrap_used)]
                let resources: &mut RenderResources =
                    renderer.callback_resources.get_mut().unwrap();
                let updated = resources
                    .sim_mesh_data
                    .as_mut()
                    .is_some_and(|data| data.update_mesh_if_fits(&rs.queue, mesh_ref, &colors));
                if !updated {
                    resources.sim_mesh_data = SimMeshGpuData::from_heightmap_mesh_colored(
                        &rs.device,
                        &resources.gpu_limits,
                        mesh_ref,
                        &colors,
                    );
                }
            }
        }
    }

    /// Incrementally simulate the stock heightmap to match current_move.
    ///
    /// On forward playback this simulates the new moves since last frame.
    /// On backward scrub it resets from the nearest checkpoint heightmap.
    // SAFETY: cp_idx is from enumerate over boundaries; vertex loop uses step_by(3) within len
    #[allow(clippy::indexing_slicing)]
    pub(super) fn update_live_sim(&mut self, frame: &mut eframe::Frame) {
        use rs_cam_core::dexel_mesh::{dexel_stock_to_entry_surface_mesh, dexel_stock_to_mesh};
        use rs_cam_core::dexel_stock::TriDexelStock;

        let target_move = self.controller.state().simulation.playback.current_move;
        let live_move = self.controller.state().simulation.playback.live_sim_move;

        let playback_needs_full_mesh = {
            let pb = &self.controller.state().simulation.playback;
            pb.display_mesh_preview && !pb.playing && !pb.scrub_drag_active
        };
        if target_move == live_move
            && self
                .controller
                .state()
                .simulation
                .playback
                .display_mesh_move
                == Some(target_move)
            && !playback_needs_full_mesh
        {
            return; // nothing changed
        }

        // While the pointer is dragging a timeline/plot scrubber, do not replay
        // stock or rebuild/upload a mesh on the UI thread. The playhead and tool
        // marker stay responsive; the stock catches up on the first frame after
        // release because `live_sim_move` remains behind/ahead of `current_move`.
        if self
            .controller
            .state()
            .simulation
            .playback
            .scrub_drag_active
        {
            return;
        }

        // Quick presence check — the actual playback_data is read by reference
        // inside the forward-simulation block below to avoid a per-frame
        // clone of the (Toolpath, Tool, StockCutDirection) vec.
        let playback_empty = self
            .controller
            .state()
            .simulation
            .results
            .as_ref()
            .is_none_or(|r| r.playback_data.is_empty());
        if playback_empty {
            return;
        }

        // If moving backward, reset from nearest checkpoint.
        // Checkpoints store global-frame stocks (stamped in parallel with per-setup
        // simulation), so they're compatible with the global-frame playback toolpaths.
        if target_move < live_move {
            let boundaries = self.controller.state().simulation.boundaries();
            let mut best_cp: Option<usize> = None;
            for (i, b) in boundaries.iter().enumerate() {
                if b.end_move <= target_move {
                    best_cp = Some(i);
                }
            }

            if let Some(cp_idx) = best_cp {
                if let Some(cp) = self.controller.state().simulation.checkpoints().get(cp_idx) {
                    let stock_clone = cp.stock().clone();
                    let cp_end = boundaries[cp_idx].end_move;
                    let pb = &mut self.controller.state_mut().simulation.playback;
                    pb.live_stock = Some(stock_clone);
                    pb.live_sim_move = cp_end;
                }
            } else {
                // Before any checkpoint — reset to fresh stock (global frame)
                let bbox = self
                    .controller
                    .state()
                    .simulation
                    .results
                    .as_ref()
                    .map(|r| r.stock_bbox)
                    .unwrap_or_else(|| self.controller.state().session.stock_bbox());
                let res = self.controller.state().simulation.resolution;
                let fresh = TriDexelStock::from_bounds(&bbox, res);
                let pb = &mut self.controller.state_mut().simulation.playback;
                pb.live_stock = Some(fresh);
                pb.live_sim_move = 0;
            }
        }

        // Now simulate forward from live_sim_move to target_move
        let current_live = self.controller.state().simulation.playback.live_sim_move;
        if current_live < target_move {
            // Take the stock out first. Doing this BEFORE we borrow
            // playback_data immutably means the upcoming `state()` read
            // can hold its borrow uninterrupted across the whole inner
            // loop — and lets us read playback_data by reference instead
            // of cloning a thousands-of-moves Vec every frame.
            let mut stock = self
                .controller
                .state_mut()
                .simulation
                .playback
                .live_stock
                .take();

            if let Some(ref mut stock) = stock
                && let Some(results) = self.controller.state().simulation.results.as_ref()
            {
                let mut global_offset = 0;
                for (toolpath, tool, direction, drill_op) in &results.playback_data {
                    let tp_moves = toolpath.moves.len();
                    let tp_start = global_offset;
                    let tp_end = global_offset + tp_moves;

                    if tp_end > current_live && tp_start < target_move {
                        if let Some(drill_op_arc) = drill_op {
                            // Drill ops use analytical removal (DEXEL Step 3 PR1).
                            // `simulate_toolpath_range`'s degenerate-Z dexel
                            // stamping doesn't match the analytical kernel — and
                            // the live-sim mesh would diverge from the
                            // checkpoint mesh during forward scrub. `apply_drill_op`
                            // is idempotent, so re-calling on each forward replay
                            // is safe.
                            stock.apply_drill_op(drill_op_arc);
                        } else {
                            let local_start = current_live.saturating_sub(tp_start);
                            let local_end = if target_move < tp_end {
                                target_move - tp_start
                            } else {
                                tp_moves
                            };

                            let cutter = crate::compute::worker::helpers::build_cutter(tool);
                            stock.simulate_toolpath_range(
                                toolpath,
                                &cutter,
                                *direction,
                                local_start,
                                local_end,
                            );
                        }
                    }
                    global_offset += tp_moves;
                }
            }

            // Put it back — fresh mutable borrow, no overlap with the
            // immutable borrow above (which dropped at the end of the
            // `if let` block).
            let pb = &mut self.controller.state_mut().simulation.playback;
            pb.live_stock = stock;
            pb.live_sim_move = target_move;
        }

        // Convert stock to mesh and upload to GPU. During continuous playback,
        // throttle this expensive UI-thread work; the playhead/tool still move
        // every frame, but full stock remesh/upload is capped to keep egui responsive.
        const LIVE_MESH_UPLOAD_INTERVAL: Duration = Duration::from_millis(50);
        let total_moves = self.controller.state().simulation.total_moves();
        let skip_mesh_refresh = {
            let pb = &self.controller.state().simulation.playback;
            pb.playing
                && target_move < total_moves
                && pb
                    .last_mesh_upload_at
                    .is_some_and(|last| last.elapsed() < LIVE_MESH_UPLOAD_INTERVAL)
        };
        if skip_mesh_refresh {
            return;
        }

        if let Some(stock) = &self.controller.state().simulation.playback.live_stock {
            let total_start = Instant::now();
            let mesh_start = Instant::now();
            let use_preview_mesh = self.controller.state().simulation.playback.playing;
            let preview_direction = self
                .controller
                .state()
                .simulation
                .current_boundary()
                .map_or(rs_cam_core::dexel_stock::StockCutDirection::FromTop, |b| {
                    b.direction
                });
            let mut mesh = if use_preview_mesh {
                dexel_stock_to_entry_surface_mesh(stock, preview_direction)
            } else {
                dexel_stock_to_mesh(stock)
            };

            // Append analytical drill cylinders for drill ops whose TP the
            // playhead has entered. This matches the compute path's checkpoint
            // mesh (which appends cylinders at every boundary) — without it
            // the live-sim mesh drops the cylinder geometry between
            // checkpoints and drill holes render as the dexel-stamped
            // approximation only. See `compute/simulate.rs:504-508`.
            if let Some(results) = self.controller.state().simulation.results.as_ref() {
                let mut offset: usize = 0;
                let completed: Vec<&rs_cam_core::drill_op::DrillOp> = results
                    .playback_data
                    .iter()
                    .filter_map(|(tp, _t, _d, drill_op)| {
                        let tp_start = offset;
                        offset += tp.moves.len();
                        if tp_start < target_move {
                            drill_op.as_deref()
                        } else {
                            None
                        }
                    })
                    .collect();
                if !completed.is_empty() {
                    rs_cam_core::dexel_mesh::append_drill_cylinders(&mut mesh, &completed);
                }
            }

            // Transform mesh from global stock frame to the active setup's
            // local frame so it matches the tool position (already in local).
            self.transform_mesh_to_local_frame(&mut mesh, target_move);
            let mesh_elapsed = mesh_start.elapsed();

            let color_start = Instant::now();
            let colors = self.compute_sim_colors(&mesh);
            let color_elapsed = color_start.elapsed();
            {
                let pb = &mut self.controller.state_mut().simulation.playback;
                pb.display_mesh = Some(mesh);
                pb.display_mesh_move = Some(target_move);
                pb.display_mesh_preview = use_preview_mesh;
                pb.last_mesh_upload_at = Some(Instant::now());
            }

            let mut upload_elapsed = Duration::ZERO;
            if let Some(rs) = frame.wgpu_render_state() {
                let upload_start = Instant::now();
                // SAFETY: display_mesh was set to Some on the line above.
                #[allow(clippy::unwrap_used)]
                let mesh_ref = self
                    .controller
                    .state()
                    .simulation
                    .playback
                    .display_mesh
                    .as_ref()
                    .unwrap();
                let mut renderer = rs.renderer.write();
                // SAFETY: RenderResources inserted in RsCamApp::new; always present.
                #[allow(clippy::unwrap_used)]
                let resources: &mut RenderResources =
                    renderer.callback_resources.get_mut().unwrap();
                let updated = resources
                    .sim_mesh_data
                    .as_mut()
                    .is_some_and(|data| data.update_mesh_if_fits(&rs.queue, mesh_ref, &colors));
                if !updated {
                    resources.sim_mesh_data = SimMeshGpuData::from_heightmap_mesh_colored(
                        &rs.device,
                        &resources.gpu_limits,
                        mesh_ref,
                        &colors,
                    );
                }
                upload_elapsed = upload_start.elapsed();
            }

            let total_elapsed = total_start.elapsed();
            if total_elapsed > Duration::from_millis(16) {
                tracing::debug!(
                    target_move,
                    mesh_ms = mesh_elapsed.as_secs_f64() * 1000.0,
                    color_ms = color_elapsed.as_secs_f64() * 1000.0,
                    upload_ms = upload_elapsed.as_secs_f64() * 1000.0,
                    total_ms = total_elapsed.as_secs_f64() * 1000.0,
                    "slow live simulation mesh refresh"
                );
            }
        }
    }

    /// Return the (FaceUp, ZRotation, needs_transform) for the setup whose
    /// moves contain `move_idx` in the simulation timeline.
    pub(super) fn active_setup_orientation(
        &self,
        move_idx: usize,
    ) -> Option<(
        crate::state::job::FaceUp,
        crate::state::job::ZRotation,
        bool,
    )> {
        let sim = &self.controller.state().simulation;
        let sb = sim
            .setup_boundaries()
            .iter()
            .rev()
            .find(|sb| sb.start_move <= move_idx)?;
        let setup = self
            .controller
            .state()
            .session
            .list_setups()
            .iter()
            .find(|s| crate::state::job::SetupId(s.id) == sb.setup_id)?;
        let needs_transform = setup.face_up != crate::state::job::FaceUp::Top
            || setup.z_rotation != crate::state::job::ZRotation::Deg0;
        Some((setup.face_up, setup.z_rotation, needs_transform))
    }

    /// Update tool model position during simulation playback.
    // SAFETY: local_idx bounds-checked against moves.len() before indexing
    #[allow(clippy::indexing_slicing)]
    pub(super) fn update_sim_tool_position(&mut self, frame: &mut eframe::Frame) {
        use crate::render::sim_render::ToolGeometry;

        if !self.controller.state().simulation.has_results()
            || self.controller.state().simulation.total_moves() == 0
        {
            clear_playback_tool(&mut self.controller.state_mut().simulation.playback);
            return;
        }

        // Find the active simulated boundary first. This matches the exact
        // simulation timeline ordering (including multi-setup flips), instead
        // of assuming session toolpath-config order equals playback order.
        let current = self.controller.state().simulation.playback.current_move;
        let active = self
            .controller
            .state()
            .simulation
            .move_to_local_toolpath_move(current);
        let Some((_boundary_index, toolpath_id, local_idx)) = active else {
            clear_playback_tool(&mut self.controller.state_mut().simulation.playback);
            return;
        };

        let found = (|| {
            let state = self.controller.state();
            let session = &state.session;
            let gui = &state.gui;
            let rt = gui.toolpath_rt.get(&toolpath_id);
            let result = rt.and_then(|r| r.result.as_ref())?;
            let tp = result.toolpath();
            let motion = tp.moves.get(local_idx)?;
            let tool_info = session
                .find_toolpath_config_by_id(toolpath_id)
                .and_then(|(_, tc)| session.tools().iter().find(|tool| tool.id.0 == tc.tool_id))
                .cloned()
                .map(|tool| {
                    let tool_def = crate::compute::worker::helpers::build_cutter(&tool);
                    (tool, tool_def)
                });
            // `peak_deflection_for_move` is a linear scan of the whole cut
            // trace (up to ~600k samples), and this runs every frame —
            // continuously, given the 100 ms MCP heartbeat. It is a pure
            // function of (trace, toolpath, move, tool, material), and
            // `tool_gpu_move == Some(current)` is exactly the condition under
            // which a previous frame already evaluated it for THIS move
            // against THIS trace: a completed simulation resets
            // `tool_gpu_move` to `None` (controller/events/compute.rs), as
            // does `clear_playback_tool`. So reuse the value the playback
            // state is already holding instead of rescanning.
            let deflection_mm = if state.simulation.playback.tool_gpu_move == Some(current) {
                state.simulation.playback.tool_deflection_mm
            } else {
                tool_info.as_ref().and_then(|(_, tool_def)| {
                    state
                        .simulation
                        .results
                        .as_ref()
                        .and_then(|results| results.cut_trace.as_deref())
                        .and_then(|trace| {
                            peak_deflection_for_move(
                                trace,
                                toolpath_id,
                                local_idx,
                                tool_def,
                                &session.stock_config().material,
                            )
                        })
                })
            };
            // Toolpath moves are in the emission frame (world for identity
            // setups); the display frame is zero-rooted local. Shift
            // identity-setup positions by -stock.origin so the tool marker
            // rides the displayed stock (audit 2026-06-12, finding 1).
            let display_shift = session
                .setup_of_toolpath_id(toolpath_id)
                .and_then(|idx| session.list_setups().get(idx))
                .map_or(rs_cam_core::geo::P3::new(0.0, 0.0, 0.0), |sd| {
                    crate::state::job::Setup::for_transforms(
                        crate::state::job::SetupId(sd.id),
                        sd.face_up,
                        sd.z_rotation,
                    )
                    .emission_to_display_shift(session.stock_config())
                });
            let pos = rs_cam_core::geo::P3::new(
                motion.target.x + display_shift.x,
                motion.target.y + display_shift.y,
                motion.target.z + display_shift.z,
            );
            Some((pos, tool_info, deflection_mm))
        })();

        let Some((pos, tool_info, deflection_mm)) = found else {
            clear_playback_tool(&mut self.controller.state_mut().simulation.playback);
            return;
        };

        {
            let pb = &mut self.controller.state_mut().simulation.playback;
            pb.tool_position = Some([pos.x, pos.y, pos.z]);
            pb.tool_deflection_mm = deflection_mm;
            if let Some((tool, tool_def)) = &tool_info {
                pb.tool_radius = tool.diameter / 2.0;
                pb.tool_type_label = tool.tool_type.label().to_owned();
                pb.tool_stickout = tool_def.stickout;
                pb.tool_cutting_length = rs_cam_core::tool::MillingCutter::length(tool_def);
            }
        }

        if self.controller.state().simulation.playback.tool_gpu_move == Some(current) {
            return;
        }

        if let Some((tool, tool_def)) = tool_info
            && let Some(rs) = frame.wgpu_render_state()
        {
            let geom = ToolGeometry::from_tool_config(&tool);
            let assembly_info =
                crate::render::sim_render::ToolAssemblyInfo::from_tool_definition(&tool_def);
            let mut renderer = rs.renderer.write();
            // SAFETY: RenderResources inserted in RsCamApp::new; always present.
            #[allow(clippy::unwrap_used)]
            let resources: &mut RenderResources = renderer.callback_resources.get_mut().unwrap();
            resources.tool_model_data = Some(
                crate::render::sim_render::ToolModelGpuData::from_tool_assembly_colored(
                    &rs.device,
                    &geom,
                    &assembly_info,
                    [pos.x as f32, pos.y as f32, pos.z as f32],
                    deflection_render_color(deflection_mm),
                ),
            );
            self.controller
                .state_mut()
                .simulation
                .playback
                .tool_gpu_move = Some(current);
        }
    }
}

fn clear_playback_tool(playback: &mut crate::state::simulation::SimulationPlayback) {
    playback.tool_position = None;
    playback.tool_deflection_mm = None;
    playback.tool_gpu_move = None;
}

fn peak_deflection_for_move(
    trace: &rs_cam_core::simulation_cut::SimulationCutTrace,
    toolpath_id: rs_cam_core::ToolpathId,
    local_move: usize,
    tool: &rs_cam_core::tool::ToolDefinition,
    material: &rs_cam_core::material::Material,
) -> Option<f64> {
    trace
        .samples
        .iter()
        .filter(|sample| sample.toolpath_id == toolpath_id && sample.move_index == local_move)
        .filter_map(|sample| {
            // Effective feed per tooth (after kinematic prediction / F-039
            // modulation), mirroring the deflection gate so the live colour
            // matches the post-sim verdict.
            let eff_feed =
                rs_cam_core::tool_load::effective_feed_for_sample(sample, &trace.predicted_feeds);
            let flutes = sample.flute_count.max(1) as f64;
            let eff_fz = if sample.spindle_rpm > 0 {
                eff_feed / (sample.spindle_rpm as f64 * flutes)
            } else {
                sample.chipload_mm_per_tooth
            };
            rs_cam_core::tool_load::deflection::sample_tip_deflection_mm(
                tool, material, sample, eff_fz,
            )
        })
        .max_by(f64::total_cmp)
}

fn deflection_render_color(deflection_mm: Option<f64>) -> [f32; 3] {
    let Some(delta) = deflection_mm else {
        return crate::render::colors::TOOL_CUTTER;
    };
    if delta <= rs_cam_core::tool_load::deflection::WITHIN_BOUND_MM {
        [0.25, 0.95, 0.35]
    } else if delta <= rs_cam_core::tool_load::deflection::EXCEEDS_BOUND_MM {
        [1.0, 0.72, 0.20]
    } else {
        [1.0, 0.18, 0.12]
    }
}
