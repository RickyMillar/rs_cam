use std::sync::Arc;

use rs_cam_core::dexel_stock::StockCutDirection;
use rs_cam_core::geo::BoundingBox3;

use crate::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, SetupSimGroup, SetupSimToolpath,
    SimulationRequest,
};
use crate::state::job::{FaceUp, Setup};
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
        let stock_bbox = BoundingBox3 {
            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
            max: rs_cam_core::geo::P3::new(stock.x, stock.y, stock.z),
        };

        let mut groups: Vec<SetupSimGroup> = Vec::new();
        let mut all_toolpaths_flat = Vec::new();

        for (i, setup) in self.state.job.setups.iter().enumerate() {
            let direction = face_up_to_direction(setup.face_up);
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
                    let transformed = if setup.needs_transform() {
                        Arc::new(transform_toolpath_to_stock_frame(
                            &result.toolpath,
                            setup,
                            stock,
                        ))
                    } else {
                        Arc::clone(&result.toolpath)
                    };
                    Some(SetupSimToolpath {
                        id: tp.id,
                        name: tp.name.clone(),
                        toolpath: transformed,
                        tool,
                        semantic_trace: tp.semantic_trace.clone(),
                    })
                })
                .collect();

            if !toolpaths.is_empty() {
                all_toolpaths_flat.extend(toolpaths.clone());
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
            let mesh = self
                .state
                .job
                .models
                .iter()
                .find(|model| model.id == toolpath.model_id)
                .and_then(|model| model.mesh.clone())?;
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

/// Derive the stock cut direction from a setup's face-up orientation.
fn face_up_to_direction(face_up: FaceUp) -> StockCutDirection {
    match face_up {
        FaceUp::Top => StockCutDirection::FromTop,
        FaceUp::Bottom => StockCutDirection::FromBottom,
        FaceUp::Front => StockCutDirection::FromFront,
        FaceUp::Back => StockCutDirection::FromBack,
        FaceUp::Left => StockCutDirection::FromLeft,
        FaceUp::Right => StockCutDirection::FromRight,
    }
}

/// Transform a toolpath from a setup's local coordinate frame to the global
/// stock-relative frame (origin at 0,0,0, axes aligned with physical stock).
///
/// For arc moves (CW/CCW), the offset vector (i,j) is transformed by the
/// linear part of the affine transform, and arc direction is flipped when the
/// XY component of the transform is a reflection.
fn transform_toolpath_to_stock_frame(
    toolpath: &rs_cam_core::toolpath::Toolpath,
    setup: &Setup,
    stock: &crate::state::job::StockConfig,
) -> rs_cam_core::toolpath::Toolpath {
    use rs_cam_core::geo::P3;
    use rs_cam_core::toolpath::{Move, MoveType, Toolpath};

    let (eff_w, eff_d, _) = setup.face_up.effective_stock(stock.x, stock.y, stock.z);

    // Point transform: undo ZRotation, then undo FaceUp (local → global stock-relative)
    let xform = |p: P3| -> P3 {
        let unrotated = setup.z_rotation.inverse_transform_point(p, eff_w, eff_d);
        setup
            .face_up
            .inverse_transform_point(unrotated, stock.x, stock.y, stock.z)
    };

    // Direction transform for arc offsets (i,j,0): linear part only (no translation).
    let o_g = xform(P3::new(0.0, 0.0, 0.0));
    let dir_xform = |di: f64, dj: f64| -> (f64, f64) {
        let p_g = xform(P3::new(di, dj, 0.0));
        (p_g.x - o_g.x, p_g.y - o_g.y)
    };

    // Determine if XY transform is a reflection (negative determinant → flip arc direction).
    let ex_g = xform(P3::new(1.0, 0.0, 0.0));
    let ey_g = xform(P3::new(0.0, 1.0, 0.0));
    let det = (ex_g.x - o_g.x) * (ey_g.y - o_g.y) - (ex_g.y - o_g.y) * (ey_g.x - o_g.x);
    let flip_arcs = det < 0.0;

    let new_moves: Vec<Move> = toolpath
        .moves
        .iter()
        .map(|m| {
            let target = xform(m.target);
            let move_type = match m.move_type {
                MoveType::Rapid => MoveType::Rapid,
                MoveType::Linear { feed_rate } => MoveType::Linear { feed_rate },
                MoveType::ArcCW { i, j, feed_rate } => {
                    let (ni, nj) = dir_xform(i, j);
                    if flip_arcs {
                        MoveType::ArcCCW {
                            i: ni,
                            j: nj,
                            feed_rate,
                        }
                    } else {
                        MoveType::ArcCW {
                            i: ni,
                            j: nj,
                            feed_rate,
                        }
                    }
                }
                MoveType::ArcCCW { i, j, feed_rate } => {
                    let (ni, nj) = dir_xform(i, j);
                    if flip_arcs {
                        MoveType::ArcCW {
                            i: ni,
                            j: nj,
                            feed_rate,
                        }
                    } else {
                        MoveType::ArcCCW {
                            i: ni,
                            j: nj,
                            feed_rate,
                        }
                    }
                }
            };
            Move { target, move_type }
        })
        .collect();

    Toolpath { moves: new_moves }
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
