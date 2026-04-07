use std::sync::Arc;

use rs_cam_core::geo::BoundingBox3;

use crate::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, SetupSimGroup, SetupSimToolpath,
    SimulationRequest,
};
use crate::state::toolpath::ToolpathEntry;
use crate::state::toolpath::ToolpathId;

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    // ── Simulation helpers ───────────────────────────────────────────────

    pub(crate) fn handle_reset_simulation(&mut self) {
        self.invalidate_simulation();
    }

    /// Clear all cached simulation state — used after undo/redo of stock,
    /// tool, or machine changes that invalidate the sim mesh.
    pub(crate) fn invalidate_simulation(&mut self) {
        self.compute.cancel_lane(ComputeLane::Analysis);
        let sim = &mut self.state.simulation;
        sim.results = None;
        sim.playback = Default::default();
        sim.checks = Default::default();
        sim.last_run = None;
        self.collision_positions.clear();
        self.pending_upload = true;
    }

    pub(crate) fn handle_sim_jump_to_move(&mut self, move_idx: usize) {
        if self.state.simulation.has_results() {
            let total = self.state.simulation.total_moves();
            self.state.simulation.playback.playing = false;
            self.state.simulation.playback.current_move = move_idx.min(total);
        }
    }

    pub(crate) fn handle_sim_step_forward(&mut self) {
        if self.state.simulation.has_results() {
            let total = self.state.simulation.total_moves();
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = (pb.current_move + 1).min(total);
        }
    }

    pub(crate) fn handle_sim_step_backward(&mut self) {
        if self.state.simulation.has_results() {
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = pb.current_move.saturating_sub(1);
        }
    }

    pub(crate) fn handle_sim_jump_to_start(&mut self) {
        if self.state.simulation.has_results() {
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = 0;
        }
    }

    pub(crate) fn handle_sim_jump_to_end(&mut self) {
        if self.state.simulation.has_results() {
            let total = self.state.simulation.total_moves();
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = total;
        }
    }

    pub(crate) fn handle_sim_jump_to_op_start(&mut self, boundary_idx: usize) {
        if let Some(start) = self
            .state
            .simulation
            .boundaries()
            .get(boundary_idx)
            .map(|b| b.start_move)
        {
            self.state.simulation.playback.playing = false;
            self.state.simulation.playback.current_move = start;
        }
    }

    pub(crate) fn handle_sim_jump_to_op_end(&mut self, boundary_idx: usize) {
        if let Some(end) = self
            .state
            .simulation
            .boundaries()
            .get(boundary_idx)
            .map(|b| b.end_move)
        {
            self.state.simulation.playback.playing = false;
            self.state.simulation.playback.current_move = end;
        }
    }

    /// Build per-setup simulation groups by applying a per-setup toolpath filter.
    /// Returns `(groups, all_toolpaths_flat, stock_bbox)` or `None` if no
    /// toolpaths matched.
    pub(crate) fn build_simulation_groups(
        &self,
        mut include_toolpath: impl FnMut(usize, &ToolpathEntry) -> bool,
        mut stop_after_setup: impl FnMut(usize) -> bool,
    ) -> Option<(Vec<SetupSimGroup>, Vec<SetupSimToolpath>, BoundingBox3)> {
        let stock = &self.state.job.stock;
        // Toolpaths are generated in setup-local frame (0,0,0 origin) —
        // the setup transform translates meshes by -stock.origin before
        // generating. Use the same zero-origin bbox for simulation.
        let stock_bbox = BoundingBox3 {
            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
            max: rs_cam_core::geo::P3::new(stock.x, stock.y, stock.z),
        };

        let mut groups: Vec<SetupSimGroup> = Vec::new();
        let mut all_toolpaths_flat = Vec::new();

        for (i, setup) in self.state.job.setups.iter().enumerate() {
            // Toolpaths stay in setup-local frame — no transform needed.
            let toolpaths: Vec<_> = setup
                .toolpaths
                .iter()
                .filter(|tp| include_toolpath(i, tp))
                .filter_map(|tp| {
                    let result = tp.result.as_ref()?;
                    let tool = self
                        .state
                        .job
                        .tools
                        .iter()
                        .find(|t| t.id == tp.tool_id)?
                        .clone();
                    Some(SetupSimToolpath {
                        id: tp.id,
                        name: tp.name.clone(),
                        toolpath: Arc::clone(&result.toolpath),
                        tool,
                        semantic_trace: tp.semantic_trace.clone(),
                    })
                })
                .collect();

            if !toolpaths.is_empty() {
                all_toolpaths_flat.extend(toolpaths.clone());

                let direction = match setup.face_up {
                    crate::state::job::FaceUp::Bottom => {
                        rs_cam_core::dexel_stock::StockCutDirection::FromBottom
                    }
                    _ => rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                };

                groups.push(SetupSimGroup {
                    toolpaths,
                    direction,
                });
            }

            if stop_after_setup(i) {
                break;
            }
        }

        if groups.is_empty() {
            return None;
        }
        Some((groups, all_toolpaths_flat, stock_bbox))
    }

    /// Submit a simulation request, handling auto-resolution and model mesh.
    pub(crate) fn submit_simulation_for_groups(
        &mut self,
        groups: Vec<SetupSimGroup>,
        all_toolpaths_flat: &[SetupSimToolpath],
        stock_bbox: BoundingBox3,
        _model_setup_idx: Option<usize>,
    ) {
        if self.state.simulation.auto_resolution {
            self.state.simulation.resolution =
                auto_resolution_for_tools(all_toolpaths_flat, &stock_bbox);
        }

        // Pass model mesh in world coordinates — same frame as the dexel stock mesh.
        let model_mesh = self.state.job.models.iter().find_map(|m| m.mesh.clone());

        self.compute.submit_simulation(SimulationRequest {
            groups,
            stock_bbox,
            stock_top_z: stock_bbox.max.z,
            resolution: self.state.simulation.resolution,
            metric_options: self.state.simulation.metric_options,
            spindle_rpm: self.state.job.post.spindle_speed,
            rapid_feed_mm_min: if self.state.job.post.high_feedrate_mode {
                self.state.job.post.high_feedrate.max(1.0)
            } else {
                self.state.job.machine.max_feed_mm_min.max(1.0)
            },
            model_mesh,
        });
    }

    pub(crate) fn run_simulation_with_all(&mut self) {
        let Some((groups, all_toolpaths_flat, stock_bbox)) = self.build_simulation_groups(
            |_setup_idx, tp| tp.enabled,
            |_setup_idx| false, // never stop early
        ) else {
            tracing::warn!("No computed toolpaths to simulate");
            self.push_notification(
                "No computed toolpaths to simulate".into(),
                super::super::Severity::Warning,
            );
            return;
        };
        self.submit_simulation_for_groups(groups, &all_toolpaths_flat, stock_bbox, Some(0));
    }

    pub(crate) fn run_simulation_with_ids(&mut self, ids: &[ToolpathId]) {
        let target_setup_idx = self
            .state
            .job
            .setups
            .iter()
            .position(|s| s.toolpaths.iter().any(|tp| ids.contains(&tp.id)));
        let Some(target_setup_idx) = target_setup_idx else {
            tracing::warn!("No computed toolpaths to simulate");
            self.push_notification(
                "No computed toolpaths to simulate".into(),
                super::super::Severity::Warning,
            );
            return;
        };

        let Some((groups, all_toolpaths_flat, stock_bbox)) = self.build_simulation_groups(
            |setup_idx, tp| {
                if setup_idx == target_setup_idx {
                    ids.contains(&tp.id)
                } else if setup_idx < target_setup_idx {
                    tp.enabled
                } else {
                    false
                }
            },
            |setup_idx| setup_idx == target_setup_idx,
        ) else {
            return;
        };
        self.submit_simulation_for_groups(
            groups,
            &all_toolpaths_flat,
            stock_bbox,
            Some(target_setup_idx),
        );
    }

    pub(crate) fn request_collision_check(&mut self) {
        let toolpath_data = self.state.job.all_toolpaths().find_map(|toolpath| {
            let result = toolpath.result.as_ref()?;
            let tool = self
                .state
                .job
                .tools
                .iter()
                .find(|tool| tool.id == toolpath.tool_id)?
                .clone();
            let raw_mesh = self
                .state
                .job
                .models
                .iter()
                .find(|model| model.id == toolpath.model_id)
                .and_then(|model| model.mesh.clone())?;

            // Transform mesh to setup-local frame to match toolpath coordinates.
            let setup = self
                .state
                .job
                .setups
                .iter()
                .find(|s| s.toolpaths.iter().any(|tp| tp.id == toolpath.id));
            let mesh = if let Some(setup) = setup {
                Arc::new(crate::state::job::transform_mesh(
                    &raw_mesh,
                    setup,
                    &self.state.job.stock,
                ))
            } else {
                raw_mesh
            };

            Some((Arc::clone(&result.toolpath), tool, mesh))
        });

        if let Some((toolpath, tool, mesh)) = toolpath_data {
            self.compute.submit_collision(CollisionRequest {
                toolpath,
                tool,
                mesh,
            });
        } else {
            tracing::warn!("No toolpath with STL mesh available for collision check");
            self.push_notification(
                "No toolpath with STL mesh available for collision check".into(),
                super::super::Severity::Warning,
            );
        }
    }
}

/// Targets ~5 cells across the smallest tool radius so curved profiles
/// (especially ball nose) are visually resolved.  Clamped to [0.02, 0.5] mm
/// and further limited so the grid stays under ~8 M cells.
fn auto_resolution_for_tools(toolpaths: &[SetupSimToolpath], stock_bbox: &BoundingBox3) -> f64 {
    let min_radius = toolpaths
        .iter()
        .map(|toolpath| toolpath.tool.diameter / 2.0)
        .fold(f64::INFINITY, f64::min);

    // 5 cells across the radius gives decent curve resolution
    let from_tool = (min_radius / 5.0).clamp(0.02, 0.5);

    // Cap so grid stays under ~8M cells (reasonable memory / mesh size)
    let max_cells: f64 = 8_000_000.0;
    let sx = stock_bbox.max.x - stock_bbox.min.x;
    let sy = stock_bbox.max.y - stock_bbox.min.y;
    let from_grid = ((sx * sy) / max_cells).sqrt().max(0.02);

    let resolution = from_tool.max(from_grid);

    tracing::info!(
        "Auto sim resolution: {:.3} mm (smallest tool \u{00D8}{:.2} mm, grid ~{}x{})",
        resolution,
        min_radius * 2.0,
        (sx / resolution).ceil() as usize,
        (sy / resolution).ceil() as usize,
    );

    resolution
}
