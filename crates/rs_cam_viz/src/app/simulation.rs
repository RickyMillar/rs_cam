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
                Some(c) => c.mesh.clone(),
                None => return,
            };
            // Checkpoint mesh is in global stock frame — transform to active
            // setup's local frame so it matches the tool position.
            self.transform_mesh_to_local_frame(&mut mesh, move_idx);
            let colors = self.compute_sim_colors(&mesh);
            self.controller.state_mut().simulation.playback.display_mesh = Some(mesh);
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
                resources.sim_mesh_data = SimMeshGpuData::from_heightmap_mesh_colored(
                    &rs.device,
                    &resources.gpu_limits,
                    mesh_ref,
                    &colors,
                );
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
        use rs_cam_core::dexel_mesh::dexel_stock_to_mesh;
        use rs_cam_core::dexel_stock::TriDexelStock;

        let target_move = self.controller.state().simulation.playback.current_move;
        let live_move = self.controller.state().simulation.playback.live_sim_move;

        if target_move == live_move {
            return; // nothing changed
        }

        // Use pre-transformed playback data from simulation results.
        // Each entry has the toolpath already in global stock frame + its direction.
        let playback_data: Vec<_> = self
            .controller
            .state()
            .simulation
            .results
            .as_ref()
            .map(|r| r.playback_data.clone())
            .unwrap_or_default();

        if playback_data.is_empty() {
            return;
        }

        // If moving backward, reset to fresh stock and re-simulate from start.
        // Per-setup simulation stores local-frame stocks in checkpoints, but
        // playback_data uses global-frame toolpaths. Always reset to a fresh
        // global stock to avoid frame mismatches.
        if target_move < live_move {
            let bbox = self
                .controller
                .state()
                .simulation
                .results
                .as_ref()
                .map(|r| r.stock_bbox)
                .unwrap_or_else(|| self.controller.state().job.stock.bbox());
            let res = self.controller.state().simulation.resolution;
            let fresh = TriDexelStock::from_bounds(&bbox, res);
            let pb = &mut self.controller.state_mut().simulation.playback;
            pb.live_stock = Some(fresh);
            pb.live_sim_move = 0;
        }

        // Now simulate forward from live_sim_move to target_move
        let current_live = self.controller.state().simulation.playback.live_sim_move;
        if current_live < target_move {
            // Take the stock out to avoid borrow conflicts
            let mut stock = self
                .controller
                .state_mut()
                .simulation
                .playback
                .live_stock
                .take();

            if let Some(ref mut stock) = stock {
                let mut global_offset = 0;
                for (toolpath, tool, direction) in &playback_data {
                    let tp_moves = toolpath.moves.len();
                    let tp_start = global_offset;
                    let tp_end = global_offset + tp_moves;

                    if tp_end > current_live && tp_start < target_move {
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
                    global_offset += tp_moves;
                }
            }

            // Put it back
            let pb = &mut self.controller.state_mut().simulation.playback;
            pb.live_stock = stock;
            pb.live_sim_move = target_move;
        }

        // Convert stock to mesh and upload to GPU.
        if let Some(stock) = &self.controller.state().simulation.playback.live_stock {
            let mut mesh = dexel_stock_to_mesh(stock);

            // Transform mesh from global stock frame to the active setup's
            // local frame so it matches the tool position (already in local).
            self.transform_mesh_to_local_frame(&mut mesh, target_move);

            let colors = self.compute_sim_colors(&mesh);
            self.controller.state_mut().simulation.playback.display_mesh = Some(mesh);

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
                resources.sim_mesh_data = SimMeshGpuData::from_heightmap_mesh_colored(
                    &rs.device,
                    &resources.gpu_limits,
                    mesh_ref,
                    &colors,
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
            .job
            .setups
            .iter()
            .find(|s| s.id == sb.setup_id)?;
        Some((setup.face_up, setup.z_rotation, setup.needs_transform()))
    }

    /// Update tool model position during simulation playback.
    // SAFETY: local_idx bounds-checked against moves.len() before indexing
    #[allow(clippy::indexing_slicing)]
    pub(super) fn update_sim_tool_position(&mut self, frame: &mut eframe::Frame) {
        use crate::render::sim_render::ToolGeometry;

        if !self.controller.state().simulation.has_results()
            || self.controller.state().simulation.total_moves() == 0
        {
            self.controller
                .state_mut()
                .simulation
                .playback
                .tool_position = None;
            return;
        }

        // Find which toolpath and move index we're at
        let current = self.controller.state().simulation.playback.current_move;
        let mut cumulative = 0;
        let mut found = None;
        for tp in self.controller.state().job.all_toolpaths() {
            if !tp.enabled {
                continue;
            }
            if let Some(result) = &tp.result {
                let tp_moves = result.toolpath.moves.len();
                if current <= cumulative + tp_moves {
                    let local_idx = current.saturating_sub(cumulative);
                    if local_idx < result.toolpath.moves.len() {
                        // Toolpath is in local coords, viewport is in local frame — use directly
                        let pos = result.toolpath.moves[local_idx].target;
                        let tool_info = self
                            .controller
                            .state()
                            .job
                            .tools
                            .iter()
                            .find(|tool| tool.id == tp.tool_id)
                            .cloned();
                        found = Some((pos, tool_info));
                    }
                    break;
                }
                cumulative += tp_moves;
            }
        }

        let Some((pos, tool_info)) = found else {
            self.controller
                .state_mut()
                .simulation
                .playback
                .tool_position = None;
            return;
        };

        {
            let pb = &mut self.controller.state_mut().simulation.playback;
            pb.tool_position = Some([pos.x, pos.y, pos.z]);
            if let Some(tool) = &tool_info {
                pb.tool_radius = tool.diameter / 2.0;
                pb.tool_type_label = tool.tool_type.label().to_owned();
            }
        }

        if let Some(tool) = tool_info
            && let Some(rs) = frame.wgpu_render_state()
        {
            let geom = ToolGeometry::from_tool_config(&tool);
            let tool_def = crate::compute::worker::helpers::build_cutter(&tool);
            let assembly_info =
                crate::render::sim_render::ToolAssemblyInfo::from_tool_definition(&tool_def);
            let mut renderer = rs.renderer.write();
            // SAFETY: RenderResources inserted in RsCamApp::new; always present.
            #[allow(clippy::unwrap_used)]
            let resources: &mut RenderResources = renderer.callback_resources.get_mut().unwrap();
            resources.tool_model_data = Some(
                crate::render::sim_render::ToolModelGpuData::from_tool_assembly(
                    &rs.device,
                    &geom,
                    &assembly_info,
                    [pos.x as f32, pos.y as f32, pos.z as f32],
                ),
            );
        }
    }
}
