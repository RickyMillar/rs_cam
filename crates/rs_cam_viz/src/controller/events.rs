use std::sync::Arc;

use crate::compute::{
    CollisionRequest, ComputeBackend, ComputeError, ComputeMessage, ComputeRequest,
    SimulationRequest,
};
use rs_cam_core::geo::BoundingBox3;

use crate::state::history::UndoAction;
use crate::state::job::ToolConfig;
use crate::state::selection::Selection;
use crate::state::simulation::SimulationState;
use crate::state::toolpath::{ComputeStatus, OperationConfig, ToolpathEntry, ToolpathId};
use crate::ui::AppEvent;

use super::AppController;

impl<B: ComputeBackend> AppController<B> {
    pub fn handle_internal_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::ImportStl(path) => {
                if let Err(error) = self.import_stl_path(&path) {
                    tracing::error!("STL import failed: {error}");
                }
            }
            AppEvent::ImportSvg(path) => {
                if let Err(error) = self.import_svg_path(&path) {
                    tracing::error!("SVG import failed: {error}");
                }
            }
            AppEvent::ImportDxf(path) => {
                if let Err(error) = self.import_dxf_path(&path) {
                    tracing::error!("DXF import failed: {error}");
                }
            }
            AppEvent::Select(selection) => {
                self.state.selection = selection;
            }
            AppEvent::AddTool(tool_type) => {
                let id = self.state.job.next_tool_id();
                let tool = ToolConfig::new_default(id, tool_type);
                self.state.selection = Selection::Tool(id);
                self.state.job.tools.push(tool);
                self.state.job.mark_edited();
            }
            AppEvent::DuplicateTool(tool_id) => {
                if let Some(src) = self.state.job.tools.iter().find(|tool| tool.id == tool_id) {
                    let mut duplicate = src.clone();
                    let new_id = self.state.job.next_tool_id();
                    duplicate.id = new_id;
                    duplicate.name = format!("{} (copy)", duplicate.name);
                    self.state.selection = Selection::Tool(new_id);
                    self.state.job.tools.push(duplicate);
                    self.state.job.mark_edited();
                }
            }
            AppEvent::RemoveTool(tool_id) => {
                self.state.job.tools.retain(|tool| tool.id != tool_id);
                if self.state.selection == Selection::Tool(tool_id) {
                    self.state.selection = Selection::None;
                }
                self.state.job.mark_edited();
            }
            AppEvent::AddToolpath(op_type) => {
                let id = self.state.job.next_toolpath_id();
                let tool_id = self
                    .state
                    .job
                    .tools
                    .first()
                    .map(|tool| tool.id)
                    .unwrap_or(crate::state::job::ToolId(0));
                let model_id = self
                    .state
                    .job
                    .models
                    .first()
                    .map(|model| model.id)
                    .unwrap_or(crate::state::job::ModelId(0));
                let entry = ToolpathEntry::for_operation(
                    id,
                    format!("{} {}", op_type.label(), id.0 + 1),
                    tool_id,
                    model_id,
                    op_type,
                );
                self.state.selection = Selection::Toolpath(id);
                self.state.job.toolpaths.push(entry);
                self.state.job.mark_edited();
            }
            AppEvent::DuplicateToolpath(tp_id) => {
                let new_id = self.state.job.next_toolpath_id();
                if let Some(src) = self
                    .state
                    .job
                    .toolpaths
                    .iter()
                    .find(|toolpath| toolpath.id == tp_id)
                {
                    self.state.selection = Selection::Toolpath(new_id);
                    let entry = src.duplicate_as(new_id, format!("{} (copy)", src.name));
                    self.state.job.toolpaths.push(entry);
                    self.state.job.mark_edited();
                }
            }
            AppEvent::MoveToolpathUp(tp_id) => {
                if let Some(index) = self
                    .state
                    .job
                    .toolpaths
                    .iter()
                    .position(|toolpath| toolpath.id == tp_id)
                    && index > 0
                {
                    self.state.job.toolpaths.swap(index, index - 1);
                }
            }
            AppEvent::MoveToolpathDown(tp_id) => {
                if let Some(index) = self
                    .state
                    .job
                    .toolpaths
                    .iter()
                    .position(|toolpath| toolpath.id == tp_id)
                    && index + 1 < self.state.job.toolpaths.len()
                {
                    self.state.job.toolpaths.swap(index, index + 1);
                }
            }
            AppEvent::ToggleToolpathEnabled(tp_id) => {
                if let Some(toolpath) = self
                    .state
                    .job
                    .toolpaths
                    .iter_mut()
                    .find(|toolpath| toolpath.id == tp_id)
                {
                    toolpath.enabled = !toolpath.enabled;
                }
            }
            AppEvent::RemoveToolpath(tp_id) => {
                self.state
                    .job
                    .toolpaths
                    .retain(|toolpath| toolpath.id != tp_id);
                if self.state.selection == Selection::Toolpath(tp_id) {
                    self.state.selection = Selection::None;
                }
                if self.state.viewport.isolate_toolpath == Some(tp_id) {
                    self.state.viewport.isolate_toolpath = None;
                }
                self.pending_upload = true;
                self.state.job.mark_edited();
            }
            AppEvent::GenerateToolpath(tp_id) => self.submit_toolpath_compute(tp_id),
            AppEvent::GenerateAll => {
                let ids: Vec<_> = self
                    .state
                    .job
                    .toolpaths
                    .iter()
                    .map(|toolpath| toolpath.id)
                    .collect();
                for id in ids {
                    self.submit_toolpath_compute(id);
                }
            }
            AppEvent::ToggleToolpathVisibility(tp_id) => {
                if let Some(toolpath) = self
                    .state
                    .job
                    .toolpaths
                    .iter_mut()
                    .find(|toolpath| toolpath.id == tp_id)
                {
                    toolpath.visible = !toolpath.visible;
                    self.pending_upload = true;
                }
            }
            AppEvent::ToggleIsolateToolpath => {
                if let Selection::Toolpath(id) = self.state.selection {
                    if self.state.viewport.isolate_toolpath == Some(id) {
                        self.state.viewport.isolate_toolpath = None;
                    } else {
                        self.state.viewport.isolate_toolpath = Some(id);
                    }
                    self.pending_upload = true;
                }
            }
            AppEvent::RunSimulation => self.run_simulation_with_all(),
            AppEvent::RunSimulationWith(ids) => self.run_simulation_with_ids(&ids),
            AppEvent::ToggleSimPlayback => {
                self.state.simulation.playing = !self.state.simulation.playing;
            }
            AppEvent::ResetSimulation => {
                self.state.simulation = SimulationState::new();
                self.pending_upload = true;
            }
            AppEvent::ToggleSimToolpath(_) => {}
            AppEvent::SimStepForward => {
                if self.state.simulation.active {
                    self.state.simulation.playing = false;
                    self.state.simulation.current_move =
                        (self.state.simulation.current_move + 1).min(self.state.simulation.total_moves);
                }
            }
            AppEvent::SimStepBackward => {
                if self.state.simulation.active {
                    self.state.simulation.playing = false;
                    self.state.simulation.current_move =
                        self.state.simulation.current_move.saturating_sub(1);
                }
            }
            AppEvent::SimJumpToStart => {
                if self.state.simulation.active {
                    self.state.simulation.playing = false;
                    self.state.simulation.current_move = 0;
                }
            }
            AppEvent::SimJumpToEnd => {
                if self.state.simulation.active {
                    self.state.simulation.playing = false;
                    self.state.simulation.current_move = self.state.simulation.total_moves;
                }
            }
            AppEvent::SimJumpToOpStart(boundary_idx) => {
                if let Some(boundary) = self.state.simulation.boundaries.get(boundary_idx) {
                    self.state.simulation.playing = false;
                    self.state.simulation.current_move = boundary.start_move;
                }
            }
            AppEvent::SimJumpToOpEnd(boundary_idx) => {
                if let Some(boundary) = self.state.simulation.boundaries.get(boundary_idx) {
                    self.state.simulation.playing = false;
                    self.state.simulation.current_move = boundary.end_move;
                }
            }
            AppEvent::RescaleModel(model_id, units) => {
                if let Err(error) = self.rescale_model(model_id, units) {
                    tracing::error!("Rescale failed: {error}");
                }
            }
            AppEvent::RunCollisionCheck => self.request_collision_check(),
            AppEvent::CancelCompute => self.compute.cancel_all(),
            AppEvent::Undo => self.undo(),
            AppEvent::Redo => self.redo(),
            AppEvent::StockChanged => {
                self.pending_upload = true;
                self.state.job.mark_edited();
            }
            AppEvent::StockMaterialChanged => {
                self.state.job.mark_edited();
            }
            AppEvent::MachineChanged => {
                self.state.job.mark_edited();
            }
            AppEvent::RecalculateFeeds(_) => {}
            AppEvent::ExportGcode
            | AppEvent::ExportGcodeConfirmed
            | AppEvent::ExportSetupSheet
            | AppEvent::ExportSvgPreview
            | AppEvent::SaveJob
            | AppEvent::OpenJob
            | AppEvent::SetViewPreset(_)
            | AppEvent::ResetView
            | AppEvent::EnterSimulation
            | AppEvent::ExitSimulation
            | AppEvent::SimVizModeChanged
            | AppEvent::Quit => {}
        }
    }

    pub fn run_simulation_with_all(&mut self) {
        let toolpaths: Vec<_> = self
            .state
            .job
            .toolpaths
            .iter()
            .filter(|toolpath| toolpath.enabled)
            .filter_map(|toolpath| {
                let result = toolpath.result.as_ref()?;
                let tool = self
                    .state
                    .job
                    .tools
                    .iter()
                    .find(|tool| tool.id == toolpath.tool_id)?
                    .clone();
                Some((
                    toolpath.id,
                    toolpath.name.clone(),
                    Arc::clone(&result.toolpath),
                    tool,
                ))
            })
            .collect();

        if toolpaths.is_empty() {
            tracing::warn!("No computed toolpaths to simulate");
        } else {
            // Auto-calculate resolution from smallest tool in the simulation
            if self.state.simulation.auto_resolution {
                self.state.simulation.resolution =
                    auto_resolution_for_tools(&toolpaths, &self.state.job.stock.bbox());
            }

            let stock_bbox = self.state.job.stock.bbox();
            let model_mesh = self
                .state
                .job
                .models
                .iter()
                .find_map(|m| m.mesh.clone());
            self.compute.submit_simulation(SimulationRequest {
                toolpaths,
                stock_bbox,
                stock_top_z: stock_bbox.max.z,
                resolution: self.state.simulation.resolution,
                model_mesh,
            });
        }
    }

    pub fn run_simulation_with_ids(&mut self, ids: &[ToolpathId]) {
        let toolpaths: Vec<_> = self
            .state
            .job
            .toolpaths
            .iter()
            .filter(|toolpath| ids.contains(&toolpath.id))
            .filter_map(|toolpath| {
                let result = toolpath.result.as_ref()?;
                let tool = self
                    .state
                    .job
                    .tools
                    .iter()
                    .find(|tool| tool.id == toolpath.tool_id)?
                    .clone();
                Some((
                    toolpath.id,
                    toolpath.name.clone(),
                    Arc::clone(&result.toolpath),
                    tool,
                ))
            })
            .collect();

        if toolpaths.is_empty() {
            tracing::warn!("No computed toolpaths to simulate");
        } else {
            if self.state.simulation.auto_resolution {
                self.state.simulation.resolution =
                    auto_resolution_for_tools(&toolpaths, &self.state.job.stock.bbox());
            }

            let stock_bbox = self.state.job.stock.bbox();
            let model_mesh = self
                .state
                .job
                .models
                .iter()
                .find_map(|m| m.mesh.clone());
            self.compute.submit_simulation(SimulationRequest {
                toolpaths,
                stock_bbox,
                stock_top_z: stock_bbox.max.z,
                resolution: self.state.simulation.resolution,
                model_mesh,
            });
        }
    }

    pub fn request_collision_check(&mut self) {
        let toolpath_data = self.state.job.toolpaths.iter().find_map(|toolpath| {
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
        }
    }

    pub fn submit_toolpath_compute(&mut self, tp_id: ToolpathId) {
        let Some(toolpath) = self
            .state
            .job
            .toolpaths
            .iter_mut()
            .find(|toolpath| toolpath.id == tp_id)
        else {
            return;
        };

        let Some(tool) = self
            .state
            .job
            .tools
            .iter()
            .find(|tool| tool.id == toolpath.tool_id)
            .cloned()
        else {
            return;
        };

        let model = self
            .state
            .job
            .models
            .iter()
            .find(|model| model.id == toolpath.model_id);
        let polygons = model.and_then(|model| model.polygons.clone());
        let mesh = model.and_then(|model| model.mesh.clone());

        let is_3d = toolpath.operation.is_3d();
        if is_3d && mesh.is_none() {
            toolpath.status = ComputeStatus::Error("No 3D mesh (import STL first)".to_string());
            return;
        }
        if !is_3d && polygons.is_none() {
            toolpath.status = ComputeStatus::Error("No 2D geometry (import SVG first)".to_string());
            return;
        }

        let prev_tool_radius = if let OperationConfig::Rest(config) = &toolpath.operation {
            config.prev_tool_id.and_then(|prev_tool_id| {
                self.state
                    .job
                    .tools
                    .iter()
                    .find(|tool| tool.id == prev_tool_id)
                    .map(|tool| tool.diameter / 2.0)
            })
        } else {
            None
        };

        toolpath.status = ComputeStatus::Computing;
        toolpath.result = None;

        let safe_z = self.state.job.post.safe_z;
        let heights = toolpath
            .heights
            .resolve(safe_z, toolpath.operation.default_depth_for_heights());

        self.compute.submit_toolpath(ComputeRequest {
            toolpath_id: tp_id,
            toolpath_name: toolpath.name.clone(),
            polygons,
            mesh,
            operation: toolpath.operation.clone(),
            dressups: toolpath.dressups.clone(),
            stock_source: toolpath.stock_source,
            tool,
            safe_z,
            prev_tool_radius,
            stock_bbox: Some(self.state.job.stock.bbox()),
            boundary_enabled: toolpath.boundary_enabled,
            boundary_containment: toolpath.boundary_containment,
            heights,
        });
    }

    pub fn drain_compute_results(&mut self) {
        for message in self.compute.drain_results() {
            match message {
                ComputeMessage::Toolpath(result) => {
                    if let Some(toolpath) = self
                        .state
                        .job
                        .toolpaths
                        .iter_mut()
                        .find(|toolpath| toolpath.id == result.toolpath_id)
                    {
                        match result.result {
                            Ok(computed) => {
                                toolpath.status = ComputeStatus::Done;
                                toolpath.result = Some(computed);
                            }
                            Err(ComputeError::Cancelled) => {
                                toolpath.status = ComputeStatus::Pending;
                            }
                            Err(ComputeError::Message(error)) => {
                                toolpath.status = ComputeStatus::Error(error);
                            }
                        }
                    }
                    self.pending_upload = true;
                }
                ComputeMessage::Simulation(result) => match result {
                    Ok(simulation) => {
                        self.state.simulation.active = true;
                        self.state.simulation.total_moves = simulation.total_moves;
                        self.state.simulation.current_move = simulation.total_moves;
                        self.state.simulation.sim_generation += 1;
                        self.state.simulation.last_sim_edit_counter =
                            self.state.job.edit_counter;

                        self.state.simulation.boundaries = simulation
                            .boundaries
                            .iter()
                            .map(|boundary| crate::state::simulation::ToolpathBoundary {
                                id: boundary.id,
                                name: boundary.name.clone(),
                                tool_name: boundary.tool_name.clone(),
                                start_move: boundary.start_move,
                                end_move: boundary.end_move,
                            })
                            .collect();
                        // Store the initial (fresh stock) heightmap for playback
                        let initial_heightmap =
                            rs_cam_core::simulation::Heightmap::from_bounds(
                                &self.state.job.stock.bbox(),
                                Some(self.state.job.stock.bbox().max.z),
                                self.state.simulation.resolution,
                            );
                        self.state.simulation.live_heightmap =
                            Some(initial_heightmap);
                        self.state.simulation.live_sim_move = 0;

                        self.state.simulation.checkpoints = simulation
                            .checkpoints
                            .into_iter()
                            .map(|checkpoint| crate::state::simulation::SimCheckpoint {
                                boundary_index: checkpoint.boundary_index,
                                mesh: checkpoint.mesh,
                                heightmap: Some(checkpoint.heightmap),
                            })
                            .collect();

                        // Store rapid collision data
                        if !simulation.rapid_collisions.is_empty() {
                            tracing::warn!(
                                "{} rapid collisions detected",
                                simulation.rapid_collisions.len()
                            );
                        }
                        self.state.simulation.rapid_collisions = simulation.rapid_collisions;
                        self.state.simulation.rapid_collision_move_indices =
                            simulation.rapid_collision_move_indices;

                        // Cache mesh and deviations for viz mode re-coloring
                        self.state.simulation.current_deviations = simulation.deviations;
                        self.state.simulation.current_mesh =
                            Some(simulation.mesh.clone());
                        self.state.simulation.mesh = Some(simulation.mesh);
                        self.pending_upload = true;
                    }
                    Err(ComputeError::Cancelled) => {}
                    Err(ComputeError::Message(error)) => {
                        tracing::error!("Simulation failed: {error}");
                    }
                },
                ComputeMessage::Collision(result) => match result {
                    Ok(collision) => {
                        let count = collision.report.collisions.len();
                        if count == 0 {
                            tracing::info!("No collisions detected");
                        } else {
                            tracing::warn!(
                                "{} collisions detected, min safe stickout: {:.1} mm",
                                count,
                                collision.report.min_safe_stickout
                            );
                        }
                        self.collision_positions = collision.positions;
                        self.pending_upload = true;
                    }
                    Err(ComputeError::Cancelled) => {}
                    Err(ComputeError::Message(error)) => {
                        tracing::error!("Collision check failed: {error}");
                    }
                },
            }
        }
    }

    fn undo(&mut self) {
        if let Some(action) = self.state.history.undo() {
            match action {
                UndoAction::StockChange { old, .. } => {
                    self.state.job.stock = old;
                    self.pending_upload = true;
                }
                UndoAction::PostChange { old, .. } => {
                    self.state.job.post = old;
                }
                UndoAction::ToolChange { tool_id, old, .. } => {
                    if let Some(tool) = self
                        .state
                        .job
                        .tools
                        .iter_mut()
                        .find(|tool| tool.id == tool_id)
                    {
                        *tool = old;
                    }
                }
                UndoAction::ToolpathParamChange {
                    tp_id,
                    old_op,
                    old_dressups,
                    ..
                } => {
                    if let Some(toolpath) = self
                        .state
                        .job
                        .toolpaths
                        .iter_mut()
                        .find(|toolpath| toolpath.id == tp_id)
                    {
                        toolpath.operation = old_op;
                        toolpath.dressups = old_dressups;
                    }
                }
                UndoAction::MachineChange { old, .. } => {
                    self.state.job.machine = old;
                }
            }
        }
    }

    fn redo(&mut self) {
        if let Some(action) = self.state.history.redo() {
            match action {
                UndoAction::StockChange { new, .. } => {
                    self.state.job.stock = new;
                    self.pending_upload = true;
                }
                UndoAction::PostChange { new, .. } => {
                    self.state.job.post = new;
                }
                UndoAction::ToolChange { tool_id, new, .. } => {
                    if let Some(tool) = self
                        .state
                        .job
                        .tools
                        .iter_mut()
                        .find(|tool| tool.id == tool_id)
                    {
                        *tool = new;
                    }
                }
                UndoAction::ToolpathParamChange {
                    tp_id,
                    new_op,
                    new_dressups,
                    ..
                } => {
                    if let Some(toolpath) = self
                        .state
                        .job
                        .toolpaths
                        .iter_mut()
                        .find(|toolpath| toolpath.id == tp_id)
                    {
                        toolpath.operation = new_op;
                        toolpath.dressups = new_dressups;
                    }
                }
                UndoAction::MachineChange { new, .. } => {
                    self.state.job.machine = new;
                }
            }
        }
    }
}

/// Calculate heightmap resolution from the smallest tool in the simulation.
///
/// Targets ~5 cells across the smallest tool radius so curved profiles
/// (especially ball nose) are visually resolved.  Clamped to [0.02, 0.5] mm
/// and further limited so the grid stays under ~8 M cells.
fn auto_resolution_for_tools(
    toolpaths: &[(ToolpathId, String, Arc<rs_cam_core::toolpath::Toolpath>, ToolConfig)],
    stock_bbox: &BoundingBox3,
) -> f64 {
    let min_radius = toolpaths
        .iter()
        .map(|(_, _, _, tool)| tool.diameter / 2.0)
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
