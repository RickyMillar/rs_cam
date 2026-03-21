use std::sync::Arc;

use crate::compute::{
    CollisionRequest, ComputeBackend, ComputeError, ComputeMessage, ComputeRequest,
    SimulationRequest,
};
use rs_cam_core::geo::BoundingBox3;

use crate::state::history::UndoAction;
use crate::state::job::{Fixture, KeepOutZone, Setup, ToolConfig};
use crate::state::selection::Selection;
use crate::state::simulation::{SimulationResults, SimulationRunMeta};
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
            AppEvent::AddSetup => {
                let id = self.state.job.next_setup_id();
                let name = format!("Setup {}", id.0 + 1);
                self.state.job.setups.push(Setup::new(id, name));
                self.state.selection = Selection::Setup(id);
                self.state.job.mark_edited();
            }
            AppEvent::RemoveSetup(setup_id) => {
                if self.state.job.setups.len() > 1 {
                    self.state.job.setups.retain(|setup| setup.id != setup_id);
                    match self.state.selection {
                        Selection::Setup(id) if id == setup_id => {
                            self.state.selection = Selection::None;
                        }
                        Selection::Fixture(id, _) if id == setup_id => {
                            self.state.selection = Selection::None;
                        }
                        Selection::KeepOut(id, _) if id == setup_id => {
                            self.state.selection = Selection::None;
                        }
                        _ => {}
                    }
                    self.pending_upload = true;
                    self.state.job.mark_edited();
                }
            }
            AppEvent::RenameSetup(setup_id, name) => {
                if let Some(setup) = self
                    .state
                    .job
                    .setups
                    .iter_mut()
                    .find(|setup| setup.id == setup_id)
                {
                    setup.name = name;
                    self.state.job.mark_edited();
                }
            }
            AppEvent::AddFixture(setup_id) => {
                let fixture_id = self.state.job.next_fixture_id();
                if let Some(setup) = self
                    .state
                    .job
                    .setups
                    .iter_mut()
                    .find(|setup| setup.id == setup_id)
                {
                    setup.fixtures.push(Fixture::new_default(fixture_id));
                    self.state.selection = Selection::Fixture(setup_id, fixture_id);
                    self.pending_upload = true;
                    self.state.job.mark_edited();
                }
            }
            AppEvent::RemoveFixture(setup_id, fixture_id) => {
                if let Some(setup) = self
                    .state
                    .job
                    .setups
                    .iter_mut()
                    .find(|setup| setup.id == setup_id)
                {
                    setup.fixtures.retain(|fixture| fixture.id != fixture_id);
                    if self.state.selection == Selection::Fixture(setup_id, fixture_id) {
                        self.state.selection = Selection::Setup(setup_id);
                    }
                    self.pending_upload = true;
                    self.state.job.mark_edited();
                }
            }
            AppEvent::AddKeepOut(setup_id) => {
                let keep_out_id = self.state.job.next_keep_out_id();
                if let Some(setup) = self
                    .state
                    .job
                    .setups
                    .iter_mut()
                    .find(|setup| setup.id == setup_id)
                {
                    setup
                        .keep_out_zones
                        .push(KeepOutZone::new_default(keep_out_id));
                    self.state.selection = Selection::KeepOut(setup_id, keep_out_id);
                    self.pending_upload = true;
                    self.state.job.mark_edited();
                }
            }
            AppEvent::RemoveKeepOut(setup_id, keep_out_id) => {
                if let Some(setup) = self
                    .state
                    .job
                    .setups
                    .iter_mut()
                    .find(|setup| setup.id == setup_id)
                {
                    setup
                        .keep_out_zones
                        .retain(|keep_out| keep_out.id != keep_out_id);
                    if self.state.selection == Selection::KeepOut(setup_id, keep_out_id) {
                        self.state.selection = Selection::Setup(setup_id);
                    }
                    self.pending_upload = true;
                    self.state.job.mark_edited();
                }
            }
            AppEvent::FixtureChanged => {
                self.pending_upload = true;
                self.state.job.mark_edited();
            }
            AppEvent::AddToolpath(op_type) => {
                let target_setup_id = match self.state.selection {
                    Selection::Toolpath(tp_id) => self.state.job.setup_of_toolpath(tp_id),
                    Selection::Setup(setup_id) => Some(setup_id),
                    Selection::Fixture(setup_id, _) => Some(setup_id),
                    Selection::KeepOut(setup_id, _) => Some(setup_id),
                    _ => None,
                }
                .or_else(|| self.state.job.setups.first().map(|setup| setup.id));

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
                if let Some(setup_id) = target_setup_id {
                    self.state.job.push_toolpath_to_setup(setup_id, entry);
                } else {
                    self.state.job.push_toolpath(entry);
                }
                self.state.job.mark_edited();
            }
            AppEvent::DuplicateToolpath(tp_id) => {
                let new_id = self.state.job.next_toolpath_id();
                let target_setup_id = self.state.job.setup_of_toolpath(tp_id);
                if let Some(src) = self.state.job.find_toolpath(tp_id) {
                    self.state.selection = Selection::Toolpath(new_id);
                    let entry = src.duplicate_as(new_id, format!("{} (copy)", src.name));
                    if let Some(setup_id) = target_setup_id {
                        self.state.job.push_toolpath_to_setup(setup_id, entry);
                    } else {
                        self.state.job.push_toolpath(entry);
                    }
                    self.state.job.mark_edited();
                }
            }
            AppEvent::MoveToolpathUp(tp_id) => {
                if self.state.job.move_toolpath_up(tp_id) {
                    self.state.job.mark_edited();
                }
            }
            AppEvent::MoveToolpathDown(tp_id) => {
                if self.state.job.move_toolpath_down(tp_id) {
                    self.state.job.mark_edited();
                }
            }
            AppEvent::ReorderToolpath(tp_id, target_idx) => {
                if self.state.job.reorder_toolpath(tp_id, target_idx) {
                    self.state.job.mark_edited();
                }
            }
            AppEvent::MoveToolpathToSetup(tp_id, setup_id, idx) => {
                if self.state.job.move_toolpath_to_setup(tp_id, setup_id, idx) {
                    self.pending_upload = true;
                    self.state.job.mark_edited();
                }
            }
            AppEvent::ToggleToolpathEnabled(tp_id) => {
                if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                    toolpath.enabled = !toolpath.enabled;
                }
            }
            AppEvent::RemoveToolpath(tp_id) => {
                self.state.job.remove_toolpath(tp_id);
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
                    .all_toolpaths()
                    .map(|toolpath| toolpath.id)
                    .collect();
                for id in ids {
                    self.submit_toolpath_compute(id);
                }
            }
            AppEvent::ToggleToolpathVisibility(tp_id) => {
                if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
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
                self.state.simulation.playback.playing = !self.state.simulation.playback.playing;
            }
            AppEvent::ResetSimulation => {
                let sim = &mut self.state.simulation;
                sim.results = None;
                sim.playback = Default::default();
                sim.checks = Default::default();
                sim.last_run = None;
                self.collision_positions.clear();
                self.pending_upload = true;
            }
            AppEvent::ToggleSimToolpath(_) => {}
            AppEvent::SimStepForward => {
                if self.state.simulation.has_results() {
                    let total = self.state.simulation.total_moves();
                    let pb = &mut self.state.simulation.playback;
                    pb.playing = false;
                    pb.current_move = (pb.current_move + 1).min(total);
                }
            }
            AppEvent::SimStepBackward => {
                if self.state.simulation.has_results() {
                    let pb = &mut self.state.simulation.playback;
                    pb.playing = false;
                    pb.current_move = pb.current_move.saturating_sub(1);
                }
            }
            AppEvent::SimJumpToStart => {
                if self.state.simulation.has_results() {
                    let pb = &mut self.state.simulation.playback;
                    pb.playing = false;
                    pb.current_move = 0;
                }
            }
            AppEvent::SimJumpToEnd => {
                if self.state.simulation.has_results() {
                    let total = self.state.simulation.total_moves();
                    let pb = &mut self.state.simulation.playback;
                    pb.playing = false;
                    pb.current_move = total;
                }
            }
            AppEvent::SimJumpToOpStart(boundary_idx) => {
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
            AppEvent::SimJumpToOpEnd(boundary_idx) => {
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
            | AppEvent::ExportCombinedGcode
            | AppEvent::ExportSetupGcode(_)
            | AppEvent::ExportGcodeConfirmed
            | AppEvent::ExportSetupSheet
            | AppEvent::ExportSvgPreview
            | AppEvent::SaveJob
            | AppEvent::OpenJob
            | AppEvent::SetViewPreset(_)
            | AppEvent::ResetView
            | AppEvent::SwitchWorkspace(_)
            | AppEvent::SimVizModeChanged
            | AppEvent::Quit => {}
        }
    }

    pub fn run_simulation_with_all(&mut self) {
        let toolpaths: Vec<_> = self
            .state
            .job
            .all_toolpaths()
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
            let model_mesh = self.state.job.models.iter().find_map(|m| m.mesh.clone());
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
            .all_toolpaths()
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
            let model_mesh = self.state.job.models.iter().find_map(|m| m.mesh.clone());
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
        }
    }

    pub fn submit_toolpath_compute(&mut self, tp_id: ToolpathId) {
        let Some((
            tool_id,
            model_id,
            operation,
            dressups,
            heights_config,
            stock_source,
            toolpath_name,
            boundary_enabled,
            boundary_containment,
        )) = self.state.job.find_toolpath(tp_id).map(|toolpath| {
            (
                toolpath.tool_id,
                toolpath.model_id,
                toolpath.operation.clone(),
                toolpath.dressups.clone(),
                toolpath.heights.clone(),
                toolpath.stock_source,
                toolpath.name.clone(),
                toolpath.boundary_enabled,
                toolpath.boundary_containment,
            )
        })
        else {
            return;
        };

        let Some(tool) = self
            .state
            .job
            .tools
            .iter()
            .find(|tool| tool.id == tool_id)
            .cloned()
        else {
            return;
        };

        let setup_ref = self
            .state
            .job
            .setups
            .iter()
            .find(|setup| setup.toolpaths.iter().any(|toolpath| toolpath.id == tp_id));
        let mut keep_out_footprints = setup_ref
            .map(|setup| {
                let mut footprints = Vec::new();
                for fixture in &setup.fixtures {
                    if fixture.enabled {
                        footprints.push(fixture.footprint());
                    }
                }
                for keep_out in &setup.keep_out_zones {
                    if keep_out.enabled {
                        footprints.push(keep_out.footprint());
                    }
                }
                footprints
            })
            .unwrap_or_default();
        let transform_setup = setup_ref.and_then(|setup| {
            if setup.needs_transform() {
                let mut transform_setup = Setup::new(setup.id, setup.name.clone());
                transform_setup.face_up = setup.face_up;
                transform_setup.z_rotation = setup.z_rotation;
                Some(transform_setup)
            } else {
                None
            }
        });
        let stock_snapshot = self.state.job.stock.clone();

        let model = self
            .state
            .job
            .models
            .iter()
            .find(|model| model.id == model_id);
        let mut polygons = model.and_then(|model| model.polygons.clone());
        let mut mesh = model.and_then(|model| model.mesh.clone());

        if let Some(transform_setup) = transform_setup.as_ref() {
            if let Some(raw_mesh) = mesh.as_ref() {
                mesh = Some(Arc::new(crate::state::job::transform_mesh(
                    raw_mesh,
                    transform_setup,
                    &stock_snapshot,
                )));
            }
            if let Some(raw_polygons) = polygons.as_ref() {
                polygons = Some(Arc::new(crate::state::job::transform_polygons(
                    raw_polygons,
                    transform_setup,
                    &stock_snapshot,
                )));
            }
            keep_out_footprints = crate::state::job::transform_polygons(
                &keep_out_footprints,
                transform_setup,
                &stock_snapshot,
            );
        }

        let is_3d = operation.is_3d();
        if is_3d && mesh.is_none() {
            if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                toolpath.status = ComputeStatus::Error("No 3D mesh (import STL first)".to_string());
            }
            return;
        }
        if !is_3d && polygons.is_none() {
            if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                toolpath.status =
                    ComputeStatus::Error("No 2D geometry (import SVG first)".to_string());
            }
            return;
        }

        let prev_tool_radius = if let OperationConfig::Rest(config) = &operation {
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

        if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
            toolpath.status = ComputeStatus::Computing;
            toolpath.result = None;
        }

        let safe_z = self.state.job.post.safe_z;
        let heights = heights_config.resolve(safe_z, operation.default_depth_for_heights());
        let stock_bbox = if let Some(transform_setup) = transform_setup.as_ref() {
            let (width, depth, height) = transform_setup.effective_stock(&stock_snapshot);
            Some(BoundingBox3 {
                min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                max: rs_cam_core::geo::P3::new(width, depth, height),
            })
        } else {
            Some(self.state.job.stock.bbox())
        };

        self.compute.submit_toolpath(ComputeRequest {
            toolpath_id: tp_id,
            toolpath_name,
            polygons,
            mesh,
            operation,
            dressups,
            stock_source,
            tool,
            safe_z,
            prev_tool_radius,
            stock_bbox,
            boundary_enabled,
            boundary_containment,
            keep_out_footprints,
            heights,
        });
    }

    pub fn drain_compute_results(&mut self) {
        for message in self.compute.drain_results() {
            match message {
                ComputeMessage::Toolpath(result) => {
                    if let Some(toolpath) = self.state.job.find_toolpath_mut(result.toolpath_id) {
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
                        // Build boundaries
                        let boundaries: Vec<_> = simulation
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

                        // Build setup boundaries
                        let setup_boundaries = {
                            let mut sbs = Vec::new();
                            let mut last_setup_id = None;
                            for boundary in &boundaries {
                                let setup_id = self.state.job.setup_of_toolpath(boundary.id);
                                if setup_id != last_setup_id {
                                    if let Some(setup_id) = setup_id {
                                        let setup_name = self
                                            .state
                                            .job
                                            .setups
                                            .iter()
                                            .find(|setup| setup.id == setup_id)
                                            .map(|setup| setup.name.clone())
                                            .unwrap_or_default();
                                        sbs.push(crate::state::simulation::SetupBoundary {
                                            setup_id,
                                            setup_name,
                                            start_move: boundary.start_move,
                                        });
                                    }
                                    last_setup_id = setup_id;
                                }
                            }
                            sbs
                        };

                        // Build checkpoints
                        let checkpoints: Vec<_> = simulation
                            .checkpoints
                            .into_iter()
                            .map(|checkpoint| crate::state::simulation::SimCheckpoint {
                                boundary_index: checkpoint.boundary_index,
                                mesh: checkpoint.mesh,
                                heightmap: Some(checkpoint.heightmap),
                            })
                            .collect();

                        // Store rapid collision data in checks
                        if !simulation.rapid_collisions.is_empty() {
                            tracing::warn!(
                                "{} rapid collisions detected",
                                simulation.rapid_collisions.len()
                            );
                        }
                        self.state.simulation.checks.rapid_collisions = simulation.rapid_collisions;
                        self.state.simulation.checks.rapid_collision_move_indices =
                            simulation.rapid_collision_move_indices;

                        // Cache display mesh and deviations for viz mode re-coloring
                        self.state.simulation.playback.display_deviations = simulation.deviations;
                        self.state.simulation.playback.display_mesh = Some(simulation.mesh.clone());

                        // Store results as cached artifact
                        self.state.simulation.results = Some(SimulationResults {
                            mesh: simulation.mesh,
                            total_moves: simulation.total_moves,
                            boundaries,
                            setup_boundaries,
                            checkpoints,
                            selected_toolpaths: None,
                        });

                        // Update playback to end position
                        self.state.simulation.playback.current_move = simulation.total_moves;

                        // Store the initial (fresh stock) heightmap for playback
                        let initial_heightmap = rs_cam_core::simulation::Heightmap::from_bounds(
                            &self.state.job.stock.bbox(),
                            Some(self.state.job.stock.bbox().max.z),
                            self.state.simulation.resolution,
                        );
                        self.state.simulation.playback.live_heightmap = Some(initial_heightmap);
                        self.state.simulation.playback.live_sim_move = 0;

                        // Update staleness metadata
                        let prev_gen = self
                            .state
                            .simulation
                            .last_run
                            .as_ref()
                            .map_or(0, |m| m.sim_generation);
                        self.state.simulation.last_run = Some(SimulationRunMeta {
                            sim_generation: prev_gen + 1,
                            last_sim_edit_counter: self.state.job.edit_counter,
                        });

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
                            tracing::info!("No holder clearance issues detected");
                        } else {
                            tracing::warn!(
                                "{} holder clearance issues, min safe stickout: {:.1} mm",
                                count,
                                collision.report.min_safe_stickout
                            );
                        }
                        // Wire results into simulation checks state
                        self.state.simulation.checks.holder_collision_count = count;
                        self.state.simulation.checks.min_safe_stickout = if count > 0 {
                            Some(collision.report.min_safe_stickout)
                        } else {
                            None
                        };
                        self.state.simulation.checks.collision_report = Some(collision.report);
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
                    if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
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
                    if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
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
    toolpaths: &[(
        ToolpathId,
        String,
        Arc<rs_cam_core::toolpath::Toolpath>,
        ToolConfig,
    )],
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
