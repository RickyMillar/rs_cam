use std::sync::Arc;

use crate::compute::ComputeManager;
use crate::compute::worker::{CollisionRequest, ComputeRequest, SimulationRequest};
use crate::state::history::UndoAction;
use crate::io::import;
use crate::render::camera::OrbitCamera;
use crate::render::mesh_render::MeshGpuData;
use crate::render::sim_render::{SimMeshGpuData, ToolModelGpuData};
use crate::render::stock_render::StockGpuData;
use crate::render::toolpath_render::ToolpathGpuData;
use crate::render::{LineUniforms, MeshUniforms, RenderResources, ViewportCallback};
use crate::state::{AppMode, AppState};
use crate::state::job::ToolConfig;
use crate::state::selection::Selection;
use crate::state::toolpath::{ComputeStatus, OperationConfig, StockSource, ToolpathEntry, ToolpathId};
use crate::ui::AppEvent;

pub struct RsCamApp {
    state: AppState,
    camera: OrbitCamera,
    events: Vec<AppEvent>,
    compute: ComputeManager,
    /// Flag: need to upload mesh/stock/toolpath to GPU on next frame.
    pending_upload: bool,
    /// Collision marker positions (from last collision check).
    collision_positions: Vec<[f32; 3]>,
    /// When the current compute started (for elapsed time display).
    compute_start: Option<std::time::Instant>,
    /// Cached viewport rect for click detection.
    viewport_rect: egui::Rect,
    /// Flag: need to load checkpoint mesh for backward scrubbing on next frame.
    pending_checkpoint_load: bool,
}

impl RsCamApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_theme(&cc.egui_ctx);

        if let Some(render_state) = cc.wgpu_render_state.as_ref() {
            let resources =
                RenderResources::new(&render_state.device, render_state.target_format);
            render_state
                .renderer
                .write()
                .callback_resources
                .insert(resources);
        }

        Self {
            state: AppState::new(),
            camera: OrbitCamera::new(),
            events: Vec::new(),
            compute: ComputeManager::new(),
            pending_upload: false,
            collision_positions: Vec::new(),
            compute_start: None,
            viewport_rect: egui::Rect::NOTHING,
            pending_checkpoint_load: false,
        }
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        let events: Vec<AppEvent> = self.events.drain(..).collect();

        for event in events {
            match event {
                AppEvent::ImportStl(path) => {
                    let id = self.state.job.next_model_id();
                    match import::import_stl(&path, id, 1.0) {
                        Ok(model) => {
                            if let Some(mesh) = &model.mesh {
                                if self.state.job.stock.auto_from_model {
                                    self.state.job.stock.update_from_bbox(&mesh.bbox);
                                }
                                let bb = &mesh.bbox;
                                self.camera.fit_to_bounds(
                                    [bb.min.x as f32, bb.min.y as f32, bb.min.z as f32],
                                    [bb.max.x as f32, bb.max.y as f32, bb.max.z as f32],
                                );
                            }
                            self.state.selection = Selection::Model(model.id);
                            self.state.job.models.push(model);
                            self.state.job.mark_edited();
                            self.pending_upload = true;
                        }
                        Err(e) => tracing::error!("STL import failed: {}", e),
                    }
                }
                AppEvent::ImportSvg(path) => {
                    let id = self.state.job.next_model_id();
                    match import::import_svg(&path, id) {
                        Ok(model) => {
                            self.state.selection = Selection::Model(model.id);
                            self.state.job.models.push(model);
                            self.state.job.mark_edited();
                        }
                        Err(e) => tracing::error!("SVG import failed: {}", e),
                    }
                }
                AppEvent::ImportDxf(path) => {
                    let id = self.state.job.next_model_id();
                    match import::import_dxf(&path, id) {
                        Ok(model) => {
                            self.state.selection = Selection::Model(model.id);
                            self.state.job.models.push(model);
                            self.state.job.mark_edited();
                        }
                        Err(e) => tracing::error!("DXF import failed: {}", e),
                    }
                }
                AppEvent::Select(sel) => {
                    self.state.selection = sel;
                }
                AppEvent::SetViewPreset(preset) => {
                    self.camera.set_preset(preset);
                }
                AppEvent::ResetView => {
                    if let Some(model) = self.state.job.models.iter().find(|m| m.mesh.is_some()) {
                        if let Some(mesh) = &model.mesh {
                            let bb = &mesh.bbox;
                            self.camera.fit_to_bounds(
                                [bb.min.x as f32, bb.min.y as f32, bb.min.z as f32],
                                [bb.max.x as f32, bb.max.y as f32, bb.max.z as f32],
                            );
                        }
                    } else {
                        self.camera = OrbitCamera::new();
                    }
                }
                AppEvent::AddTool(tool_type) => {
                    let id = self.state.job.next_tool_id();
                    let tool = ToolConfig::new_default(id, tool_type);
                    self.state.selection = Selection::Tool(id);
                    self.state.job.tools.push(tool);
                    self.state.job.mark_edited();
                }
                AppEvent::DuplicateTool(tool_id) => {
                    if let Some(src) = self.state.job.tools.iter().find(|t| t.id == tool_id) {
                        let mut dup = src.clone();
                        let new_id = self.state.job.next_tool_id();
                        dup.id = new_id;
                        dup.name = format!("{} (copy)", dup.name);
                        self.state.selection = Selection::Tool(new_id);
                        self.state.job.tools.push(dup);
                        self.state.job.mark_edited();
                    }
                }
                AppEvent::RemoveTool(tool_id) => {
                    self.state.job.tools.retain(|t| t.id != tool_id);
                    if self.state.selection == Selection::Tool(tool_id) {
                        self.state.selection = Selection::None;
                    }
                    self.state.job.mark_edited();
                }
                AppEvent::AddToolpath(op_type) => {
                    let id = self.state.job.next_toolpath_id();
                    let tool_id = self.state.job.tools.first()
                        .map(|t| t.id).unwrap_or(crate::state::job::ToolId(0));
                    let model_id = self.state.job.models.first()
                        .map(|m| m.id).unwrap_or(crate::state::job::ModelId(0));
                    let operation = OperationConfig::new_default(op_type);
                    let is_3d = operation.is_3d();
                    let entry = ToolpathEntry {
                        id,
                        name: format!("{} {}", op_type.label(), id.0 + 1),
                        enabled: true,
                        visible: true,
                        locked: false,
                        tool_id,
                        model_id,
                        operation,
                        dressups: crate::state::toolpath::DressupConfig::default(),
                        heights: crate::state::toolpath::HeightsConfig::default(),
                        boundary_enabled: false,
                        boundary_containment: crate::state::toolpath::BoundaryContainment::Center,
                        pre_gcode: String::new(),
                        post_gcode: String::new(),
                        stock_source: StockSource::Fresh,
                        status: ComputeStatus::Pending,
                        result: None,
                        stale_since: None,
                        auto_regen: !is_3d,
                        feeds_auto: crate::state::toolpath::FeedsAutoMode::default(),
                        feeds_result: None,
                    };
                    self.state.selection = Selection::Toolpath(id);
                    self.state.job.toolpaths.push(entry);
                    self.state.job.mark_edited();
                }
                AppEvent::DuplicateToolpath(tp_id) => {
                    let src_data = self.state.job.toolpaths.iter().find(|t| t.id == tp_id).map(|src| {
                        (src.name.clone(), src.enabled, src.visible, src.tool_id,
                         src.model_id, src.operation.clone(), src.dressups.clone(), src.stock_source)
                    });
                    if let Some((name, enabled, visible, tool_id, model_id, operation, dressups, stock_source)) = src_data {
                        let new_id = self.state.job.next_toolpath_id();
                        self.state.selection = Selection::Toolpath(new_id);
                        let is_3d = operation.is_3d();
                        self.state.job.toolpaths.push(ToolpathEntry {
                            id: new_id, name: format!("{} (copy)", name),
                            enabled, visible, locked: false, tool_id, model_id, operation, dressups,
                            heights: crate::state::toolpath::HeightsConfig::default(),
                            boundary_enabled: false,
                            boundary_containment: crate::state::toolpath::BoundaryContainment::Center,
                            pre_gcode: String::new(),
                            post_gcode: String::new(),
                            stock_source,
                            status: ComputeStatus::Pending, result: None,
                            stale_since: None, auto_regen: !is_3d,
                            feeds_auto: crate::state::toolpath::FeedsAutoMode::default(),
                            feeds_result: None,
                        });
                        self.state.job.mark_edited();
                    }
                }
                AppEvent::MoveToolpathUp(tp_id) => {
                    if let Some(idx) = self.state.job.toolpaths.iter().position(|t| t.id == tp_id) {
                        if idx > 0 {
                            self.state.job.toolpaths.swap(idx, idx - 1);
                        }
                    }
                }
                AppEvent::MoveToolpathDown(tp_id) => {
                    if let Some(idx) = self.state.job.toolpaths.iter().position(|t| t.id == tp_id) {
                        if idx + 1 < self.state.job.toolpaths.len() {
                            self.state.job.toolpaths.swap(idx, idx + 1);
                        }
                    }
                }
                AppEvent::ToggleToolpathEnabled(tp_id) => {
                    if let Some(tp) = self.state.job.toolpaths.iter_mut().find(|t| t.id == tp_id) {
                        tp.enabled = !tp.enabled;
                    }
                }
                AppEvent::RemoveToolpath(tp_id) => {
                    self.state.job.toolpaths.retain(|tp| tp.id != tp_id);
                    if self.state.selection == Selection::Toolpath(tp_id) {
                        self.state.selection = Selection::None;
                    }
                    // Clear isolation if isolated toolpath was removed
                    if self.state.viewport.isolate_toolpath == Some(tp_id) {
                        self.state.viewport.isolate_toolpath = None;
                    }
                    self.pending_upload = true;
                    self.state.job.mark_edited();
                }
                AppEvent::GenerateToolpath(tp_id) => {
                    self.submit_toolpath_compute(tp_id);
                }
                AppEvent::ToggleToolpathVisibility(tp_id) => {
                    if let Some(tp) = self.state.job.toolpaths.iter_mut().find(|t| t.id == tp_id) {
                        tp.visible = !tp.visible;
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
                AppEvent::RunSimulation => {
                    self.run_simulation_with_all();
                }
                AppEvent::RunSimulationWith(ids) => {
                    self.run_simulation_with_ids(&ids);
                }
                AppEvent::ToggleSimPlayback => {
                    self.state.simulation.playing = !self.state.simulation.playing;
                }
                AppEvent::ResetSimulation => {
                    self.state.simulation = crate::state::simulation::SimulationState::new();
                    self.pending_upload = true;
                }
                AppEvent::ToggleSimToolpath(_tp_id) => {
                    // Subset toggling handled by simulation panel UI
                }
                AppEvent::EnterSimulation => {
                    // If no sim results yet, trigger a run
                    if !self.state.simulation.active {
                        self.run_simulation_with_all();
                    }
                    // Save editor viewport state and set sim defaults
                    self.state.simulation.saved_show_cutting = self.state.viewport.show_cutting;
                    self.state.simulation.saved_show_rapids = self.state.viewport.show_rapids;
                    self.state.simulation.saved_show_stock = self.state.viewport.show_stock;
                    // In sim mode: hide toolpath lines, show stock
                    self.state.viewport.show_cutting = false;
                    self.state.viewport.show_rapids = false;
                    self.state.viewport.show_stock = true;
                    self.state.mode = AppMode::Simulation;
                }
                AppEvent::ExitSimulation => {
                    // Restore editor viewport state
                    self.state.viewport.show_cutting = self.state.simulation.saved_show_cutting;
                    self.state.viewport.show_rapids = self.state.simulation.saved_show_rapids;
                    self.state.viewport.show_stock = self.state.simulation.saved_show_stock;
                    self.state.mode = AppMode::Editor;
                    // Simulation state is preserved across transitions
                }
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
                        self.pending_checkpoint_load = true;
                    }
                }
                AppEvent::SimJumpToStart => {
                    if self.state.simulation.active {
                        self.state.simulation.playing = false;
                        self.state.simulation.current_move = 0;
                        self.pending_checkpoint_load = true;
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
                        self.pending_checkpoint_load = true;
                    }
                }
                AppEvent::SimJumpToOpEnd(boundary_idx) => {
                    if let Some(boundary) = self.state.simulation.boundaries.get(boundary_idx) {
                        self.state.simulation.playing = false;
                        self.state.simulation.current_move = boundary.end_move;
                    }
                }
                AppEvent::RescaleModel(model_id, new_units) => {
                    if let Some(model) = self.state.job.models.iter().find(|m| m.id == model_id) {
                        if model.kind == crate::state::job::ModelKind::Stl {
                            let path = model.path.clone();
                            let scale = new_units.scale_factor();
                            match import::import_stl(&path, model_id, scale) {
                                Ok(mut new_model) => {
                                    new_model.units = new_units;
                                    if let Some(m) = self.state.job.models.iter_mut().find(|m| m.id == model_id) {
                                        m.mesh = new_model.mesh;
                                        m.units = new_model.units;
                                        m.winding_report = new_model.winding_report;
                                        if self.state.job.stock.auto_from_model {
                                            if let Some(mesh) = &m.mesh {
                                                self.state.job.stock.update_from_bbox(&mesh.bbox);
                                            }
                                        }
                                        if let Some(mesh) = &m.mesh {
                                            let bb = &mesh.bbox;
                                            self.camera.fit_to_bounds(
                                                [bb.min.x as f32, bb.min.y as f32, bb.min.z as f32],
                                                [bb.max.x as f32, bb.max.y as f32, bb.max.z as f32],
                                            );
                                        }
                                    }
                                    self.pending_upload = true;
                                    self.state.job.mark_edited();
                                }
                                Err(e) => tracing::error!("Rescale failed: {}", e),
                            }
                        }
                    }
                }
                AppEvent::ExportGcode => {
                    // Show pre-flight checklist instead of exporting directly
                    self.state.show_preflight = true;
                }
                AppEvent::ExportGcodeConfirmed => {
                    self.export_gcode_with_summary();
                }
                AppEvent::SimVizModeChanged => {
                    // Re-upload sim mesh will happen on next pending_upload cycle
                    self.pending_upload = true;
                }
                AppEvent::ExportSvgPreview => {
                    self.export_svg_preview();
                }
                AppEvent::ExportSetupSheet => {
                    let html = crate::io::setup_sheet::generate_setup_sheet(&self.state.job);
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("HTML", &["html"])
                        .set_file_name("setup_sheet.html")
                        .save_file()
                    {
                        if let Err(e) = std::fs::write(&path, &html) {
                            tracing::error!("Failed to write setup sheet: {}", e);
                        } else {
                            tracing::info!("Exported setup sheet to {}", path.display());
                        }
                    }
                }
                AppEvent::GenerateAll => {
                    let ids: Vec<_> = self.state.job.toolpaths.iter().map(|tp| tp.id).collect();
                    for id in ids {
                        self.submit_toolpath_compute(id);
                    }
                }
                AppEvent::RunCollisionCheck => {
                    let tp_data = self.state.job.toolpaths.iter().find_map(|tp| {
                        let result = tp.result.as_ref()?;
                        let tool = self.state.job.tools.iter().find(|t| t.id == tp.tool_id)?.clone();
                        let mesh = self.state.job.models.iter()
                            .find(|m| m.id == tp.model_id)
                            .and_then(|m| m.mesh.clone())?;
                        Some((Arc::clone(&result.toolpath), tool, mesh))
                    });
                    if let Some((toolpath, tool, mesh)) = tp_data {
                        self.compute.submit_collision(CollisionRequest { toolpath, tool, mesh });
                    } else {
                        tracing::warn!("No toolpath with STL mesh available for collision check");
                    }
                }
                AppEvent::CancelCompute => {
                    self.compute.cancel();
                }
                AppEvent::SaveJob => {
                    let path = self.state.job.file_path.clone().or_else(|| {
                        rfd::FileDialog::new()
                            .add_filter("TOML Job", &["toml"])
                            .set_file_name("job.toml")
                            .save_file()
                    });
                    if let Some(path) = path {
                        match crate::io::project::save_project(&self.state.job, &path) {
                            Ok(()) => {
                                self.state.job.file_path = Some(path.clone());
                                self.state.job.dirty = false;
                                tracing::info!("Saved job to {}", path.display());
                            }
                            Err(e) => tracing::error!("Save failed: {}", e),
                        }
                    }
                }
                AppEvent::OpenJob => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("TOML Job", &["toml"])
                        .pick_file()
                    {
                        match crate::io::project::load_project(&path) {
                            Ok((job, _inputs)) => {
                                self.state.job = job;
                                self.state.selection = Selection::None;
                                self.pending_upload = true;
                                tracing::info!("Loaded job from {}", path.display());
                            }
                            Err(e) => tracing::error!("Load failed: {}", e),
                        }
                    }
                }
                AppEvent::Undo => {
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
                                if let Some(t) = self.state.job.tools.iter_mut().find(|t| t.id == tool_id) {
                                    *t = old;
                                }
                            }
                            UndoAction::ToolpathParamChange { tp_id, old_op, old_dressups, .. } => {
                                if let Some(tp) = self.state.job.toolpaths.iter_mut().find(|t| t.id == tp_id) {
                                    tp.operation = old_op;
                                    tp.dressups = old_dressups;
                                }
                            }
                            UndoAction::MachineChange { old, .. } => {
                                self.state.job.machine = old;
                            }
                        }
                    }
                }
                AppEvent::Redo => {
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
                                if let Some(t) = self.state.job.tools.iter_mut().find(|t| t.id == tool_id) {
                                    *t = new;
                                }
                            }
                            UndoAction::ToolpathParamChange { tp_id, new_op, new_dressups, .. } => {
                                if let Some(tp) = self.state.job.toolpaths.iter_mut().find(|t| t.id == tp_id) {
                                    tp.operation = new_op;
                                    tp.dressups = new_dressups;
                                }
                            }
                            UndoAction::MachineChange { new, .. } => {
                                self.state.job.machine = new;
                            }
                        }
                    }
                }
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
                AppEvent::RecalculateFeeds(_tp_id) => {
                    // Feeds recalculation happens in the UI draw pass, this is just a marker
                }
                AppEvent::Quit => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    // --- Simulation helpers ---

    /// Load the nearest checkpoint mesh for backward scrubbing.
    /// Uses `checkpoint_for_move()` to find the right snapshot and uploads it to GPU.
    fn load_checkpoint_for_move(&mut self, move_idx: usize, frame: &mut eframe::Frame) {
        if let Some(cp_idx) = self.state.simulation.checkpoint_for_move(move_idx) {
            if let Some(checkpoint) = self.state.simulation.checkpoints.get(cp_idx) {
                if let Some(rs) = frame.wgpu_render_state() {
                    let mut renderer = rs.renderer.write();
                    let resources: &mut RenderResources =
                        renderer.callback_resources.get_mut().unwrap();
                    // Upload the checkpoint mesh as the current sim mesh
                    let hm_mesh = &checkpoint.mesh;
                    resources.sim_mesh_data =
                        Some(SimMeshGpuData::from_heightmap_mesh(&rs.device, hm_mesh));
                }
            }
        }
    }

    /// Get the first STL model mesh (if any) for simulation deviation computation.
    fn first_model_mesh(&self) -> Option<Arc<rs_cam_core::mesh::TriangleMesh>> {
        self.state.job.models.iter()
            .find_map(|m| m.mesh.clone())
    }

    fn run_simulation_with_all(&mut self) {
        let toolpaths: Vec<_> = self.state.job.toolpaths.iter()
            .filter(|tp| tp.enabled)
            .filter_map(|tp| {
                let result = tp.result.as_ref()?;
                let tool = self.state.job.tools.iter().find(|t| t.id == tp.tool_id)?.clone();
                Some((tp.id, tp.name.clone(), Arc::clone(&result.toolpath), tool))
            })
            .collect();

        if toolpaths.is_empty() {
            tracing::warn!("No computed toolpaths to simulate");
        } else {
            let stock_bbox = self.state.job.stock.bbox();
            let model_mesh = self.first_model_mesh();
            self.compute.submit_simulation(SimulationRequest {
                toolpaths,
                stock_bbox,
                stock_top_z: stock_bbox.max.z,
                resolution: 0.25,
                model_mesh,
            });
            self.state.simulation.active = false;
        }
    }

    fn run_simulation_with_ids(&mut self, ids: &[ToolpathId]) {
        let toolpaths: Vec<_> = self.state.job.toolpaths.iter()
            .filter(|tp| ids.contains(&tp.id))
            .filter_map(|tp| {
                let result = tp.result.as_ref()?;
                let tool = self.state.job.tools.iter().find(|t| t.id == tp.tool_id)?.clone();
                Some((tp.id, tp.name.clone(), Arc::clone(&result.toolpath), tool))
            })
            .collect();

        if toolpaths.is_empty() {
            tracing::warn!("No computed toolpaths to simulate");
        } else {
            let stock_bbox = self.state.job.stock.bbox();
            let model_mesh = self.first_model_mesh();
            self.compute.submit_simulation(SimulationRequest {
                toolpaths,
                stock_bbox,
                stock_top_z: stock_bbox.max.z,
                resolution: 0.25,
                model_mesh,
            });
            self.state.simulation.active = false;
        }
    }

    // --- Export helpers ---

    fn export_gcode_with_summary(&self) {
        match crate::io::export::export_gcode(&self.state.job) {
            Ok(gcode) => {
                // Compute summary stats
                let line_count = gcode.lines().count();
                let mut total_moves = 0usize;
                let mut cutting_dist = 0.0f64;
                let tool_changes;
                let mut est_time_min = 0.0f64;

                for tp in &self.state.job.toolpaths {
                    if tp.enabled {
                        if let Some(result) = &tp.result {
                            total_moves += result.stats.move_count;
                            cutting_dist += result.stats.cutting_distance;
                            // Rough time estimate: cutting at first feed rate found
                            let feed = match &tp.operation {
                                OperationConfig::Pocket(c) => c.feed_rate,
                                OperationConfig::Profile(c) => c.feed_rate,
                                OperationConfig::Adaptive(c) => c.feed_rate,
                                OperationConfig::DropCutter(c) => c.feed_rate,
                                _ => 1000.0,
                            };
                            est_time_min += result.stats.cutting_distance / feed;
                        }
                    }
                }

                // Count distinct tool IDs across enabled toolpaths
                let mut seen_tools = Vec::new();
                for tp in &self.state.job.toolpaths {
                    if tp.enabled && !seen_tools.contains(&tp.tool_id) {
                        seen_tools.push(tp.tool_id);
                    }
                }
                tool_changes = if seen_tools.len() > 1 { seen_tools.len() - 1 } else { 0 };

                // Log the summary (shown in status bar / tracing output)
                tracing::info!(
                    "Export summary: {} G-code lines, {} moves, {:.0} mm cutting, {} tool changes, ~{:.1} min",
                    line_count, total_moves, cutting_dist, tool_changes, est_time_min,
                );

                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("G-code", &["nc", "gcode", "ngc"])
                    .set_file_name("output.nc")
                    .save_file()
                {
                    if let Err(e) = std::fs::write(&path, &gcode) {
                        tracing::error!("Failed to write G-code: {}", e);
                    } else {
                        tracing::info!("Exported G-code to {}", path.display());
                    }
                }
            }
            Err(e) => tracing::error!("Export failed: {}", e),
        }
    }

    fn export_svg_preview(&self) {
        use rs_cam_core::viz::toolpath_to_svg;

        // Collect all enabled toolpaths with results
        let toolpaths: Vec<_> = self.state.job.toolpaths.iter()
            .filter(|tp| tp.enabled && tp.result.is_some())
            .filter_map(|tp| tp.result.as_ref().map(|r| &*r.toolpath))
            .collect();

        if toolpaths.is_empty() {
            tracing::warn!("No computed toolpaths for SVG export");
            return;
        }

        // Generate SVG from first toolpath
        let svg = toolpath_to_svg(toolpaths[0], 800.0, 600.0);

        if let Some(path) = rfd::FileDialog::new()
            .add_filter("SVG", &["svg"])
            .set_file_name("toolpath_preview.svg")
            .save_file()
        {
            if let Err(e) = std::fs::write(&path, &svg) {
                tracing::error!("Failed to write SVG: {}", e);
            } else {
                tracing::info!("Exported SVG preview to {}", path.display());
            }
        }
    }

    fn submit_toolpath_compute(&mut self, tp_id: ToolpathId) {
        let tp = match self.state.job.toolpaths.iter_mut().find(|t| t.id == tp_id) {
            Some(tp) => tp,
            None => return,
        };

        let tool = match self.state.job.tools.iter().find(|t| t.id == tp.tool_id) {
            Some(t) => t.clone(),
            None => return,
        };

        // Find model data
        let model = self.state.job.models.iter().find(|m| m.id == tp.model_id);
        let polygons = model.and_then(|m| m.polygons.clone());
        let mesh = model.and_then(|m| m.mesh.clone());

        let is_3d = tp.operation.is_3d();
        if is_3d && mesh.is_none() {
            tp.status = ComputeStatus::Error("No 3D mesh (import STL first)".to_string());
            return;
        }
        if !is_3d && polygons.is_none() {
            tp.status = ComputeStatus::Error("No 2D geometry (import SVG first)".to_string());
            return;
        }

        // For rest machining, resolve the previous tool radius
        let prev_tool_radius = if let OperationConfig::Rest(ref cfg) = tp.operation {
            cfg.prev_tool_id.and_then(|pid| {
                self.state.job.tools.iter().find(|t| t.id == pid).map(|t| t.diameter / 2.0)
            })
        } else {
            None
        };

        tp.status = ComputeStatus::Computing(0.0);
        tp.result = None;
        self.compute_start = Some(std::time::Instant::now());

        let stock_bbox = Some(self.state.job.stock.bbox());
        let safe_z = self.state.job.post.safe_z;

        // Resolve heights from the 5-level system
        let op_depth = operation_depth(&tp.operation);
        let heights = tp.heights.resolve(safe_z, op_depth);

        self.compute.submit(ComputeRequest {
            toolpath_id: tp_id,
            polygons,
            mesh,
            operation: tp.operation.clone(),
            dressups: tp.dressups.clone(),
            tool,
            safe_z,
            prev_tool_radius,
            stock_bbox,
            boundary_enabled: tp.boundary_enabled,
            boundary_containment: tp.boundary_containment,
            heights,
        });
    }

    fn drain_compute_results(&mut self, frame: &mut eframe::Frame) {
        let (tp_results, sim_results, col_results) = self.compute.drain_results();

        if !tp_results.is_empty() {
            self.compute_start = None;
        }
        for result in tp_results {
            if let Some(tp) = self
                .state
                .job
                .toolpaths
                .iter_mut()
                .find(|t| t.id == result.toolpath_id)
            {
                match result.result {
                    Ok(r) => {
                        tp.status = ComputeStatus::Done;
                        tp.result = Some(r);
                    }
                    Err(e) => {
                        tp.status = ComputeStatus::Error(e);
                    }
                }
            }
            self.pending_upload = true;
        }

        for result in sim_results {
            match result {
                Ok(sim) => {
                    self.state.simulation.active = true;
                    self.state.simulation.total_moves = sim.total_moves;
                    self.state.simulation.current_move = sim.total_moves;
                    self.state.simulation.sim_generation += 1;
                    self.state.simulation.last_sim_edit_counter = self.state.job.edit_counter;

                    // Auto-enter simulation workspace when results arrive
                    if self.state.mode == AppMode::Editor {
                        self.state.mode = AppMode::Simulation;
                    }

                    // Convert boundaries
                    self.state.simulation.boundaries = sim.boundaries.iter().map(|b| {
                        crate::state::simulation::ToolpathBoundary {
                            id: b.id,
                            name: b.name.clone(),
                            tool_name: b.tool_name.clone(),
                            start_move: b.start_move,
                            end_move: b.end_move,
                        }
                    }).collect();

                    // Store checkpoints
                    self.state.simulation.checkpoints = sim.checkpoints.into_iter().map(|c| {
                        crate::state::simulation::SimCheckpoint {
                            boundary_index: c.boundary_index,
                            mesh: c.mesh,
                        }
                    }).collect();

                    // Store rapid collision data
                    if !sim.rapid_collisions.is_empty() {
                        tracing::warn!("{} rapid collisions detected", sim.rapid_collisions.len());
                    }
                    self.state.simulation.rapid_collisions = sim.rapid_collisions;
                    self.state.simulation.rapid_collision_move_indices = sim.rapid_collision_move_indices;

                    // Upload final sim mesh to GPU
                    if let Some(rs) = frame.wgpu_render_state() {
                        let mut renderer = rs.renderer.write();
                        let resources: &mut RenderResources =
                            renderer.callback_resources.get_mut().unwrap();
                        resources.sim_mesh_data =
                            Some(SimMeshGpuData::from_heightmap_mesh(&rs.device, &sim.mesh));
                    }
                }
                Err(e) => {
                    tracing::error!("Simulation failed: {}", e);
                }
            }
        }

        for result in col_results {
            match result {
                Ok(col) => {
                    let n = col.report.collisions.len();
                    if n == 0 {
                        tracing::info!("No collisions detected");
                    } else {
                        tracing::warn!(
                            "{} collisions detected, min safe stickout: {:.1} mm",
                            n,
                            col.report.min_safe_stickout
                        );
                    }
                    // Store into simulation state for diagnostics panel
                    self.state.simulation.holder_collision_count = n;
                    self.state.simulation.min_safe_stickout = if n > 0 {
                        Some(col.report.min_safe_stickout)
                    } else {
                        None
                    };
                    self.state.simulation.collision_report = Some(col.report);
                    self.collision_positions = col.positions;
                }
                Err(e) => tracing::error!("Collision check failed: {}", e),
            }
        }
    }

    fn upload_gpu_data(&mut self, frame: &mut eframe::Frame) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };

        let mut renderer = render_state.renderer.write();
        let resources: &mut RenderResources = renderer.callback_resources.get_mut().unwrap();

        // Upload mesh data for the first STL model
        if let Some(model) = self.state.job.models.iter().find(|m| m.mesh.is_some()) {
            if let Some(mesh) = &model.mesh {
                resources.mesh_data =
                    Some(MeshGpuData::from_mesh(&render_state.device, mesh));
            }
        }

        // Upload stock wireframe
        let stock_bbox = self.state.job.stock.bbox();
        resources.stock_data = Some(StockGpuData::from_bbox(&render_state.device, &stock_bbox));

        // Clear sim mesh if simulation was reset
        if !self.state.simulation.active {
            resources.sim_mesh_data = None;
            resources.tool_model_data = None;
        }

        // Upload collision markers as red crosses
        if !self.collision_positions.is_empty() {
            use crate::render::LineVertex;
            let s = 1.0f32; // marker size in mm
            let color = [0.95, 0.15, 0.15];
            let mut verts = Vec::new();
            for p in &self.collision_positions {
                verts.push(LineVertex { position: [p[0] - s, p[1], p[2]], color });
                verts.push(LineVertex { position: [p[0] + s, p[1], p[2]], color });
                verts.push(LineVertex { position: [p[0], p[1] - s, p[2]], color });
                verts.push(LineVertex { position: [p[0], p[1] + s, p[2]], color });
                verts.push(LineVertex { position: [p[0], p[1], p[2] - s], color });
                verts.push(LineVertex { position: [p[0], p[1], p[2] + s], color });
            }
            use egui_wgpu::wgpu::util::DeviceExt;
            resources.collision_vertex_buffer = Some(
                render_state.device.create_buffer_init(&egui_wgpu::wgpu::util::BufferInitDescriptor {
                    label: Some("collision_markers"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: egui_wgpu::wgpu::BufferUsages::VERTEX,
                }),
            );
            resources.collision_vertex_count = verts.len() as u32;
        } else {
            resources.collision_vertex_buffer = None;
            resources.collision_vertex_count = 0;
        }

        // Upload toolpath line data (with per-toolpath colors and isolation filtering)
        resources.toolpath_data.clear();
        let selected_tp_id = match self.state.selection {
            Selection::Toolpath(id) => Some(id),
            _ => None,
        };
        let isolate = self.state.viewport.isolate_toolpath;

        for (i, tp) in self.state.job.toolpaths.iter().enumerate() {
            // Skip invisible toolpaths; also skip if not the isolated toolpath
            let visible = tp.visible && match isolate {
                Some(iso_id) => tp.id == iso_id,
                None => true,
            };
            if visible {
                if let Some(result) = &tp.result {
                    let selected = selected_tp_id == Some(tp.id);
                    resources.toolpath_data.push(ToolpathGpuData::from_toolpath(
                        &render_state.device,
                        &result.toolpath,
                        i,
                        selected,
                    ));
                }
            }
        }
    }

    /// Update tool model position during simulation playback.
    fn update_sim_tool_position(&mut self, frame: &mut eframe::Frame) {
        if !self.state.simulation.active || self.state.simulation.total_moves == 0 {
            self.state.simulation.tool_position = None;
            return;
        }

        // Find which toolpath and move index we're at
        let current = self.state.simulation.current_move;
        let mut cumulative = 0;
        for tp in &self.state.job.toolpaths {
            if !tp.enabled {
                continue;
            }
            if let Some(result) = &tp.result {
                let tp_moves = result.toolpath.moves.len();
                if current <= cumulative + tp_moves {
                    let local_idx = current.saturating_sub(cumulative);
                    if local_idx < result.toolpath.moves.len() {
                        let pos = result.toolpath.moves[local_idx].target;
                        self.state.simulation.tool_position = Some([pos.x, pos.y, pos.z]);

                        // Update tool info
                        if let Some(tool) = self.state.job.tools.iter().find(|t| t.id == tp.tool_id) {
                            self.state.simulation.tool_radius = tool.diameter / 2.0;
                            self.state.simulation.tool_type_label = tool.tool_type.label().to_string();

                            // Upload tool model to GPU
                            if let Some(rs) = frame.wgpu_render_state() {
                                let is_ball = matches!(tool.tool_type,
                                    crate::state::job::ToolType::BallNose | crate::state::job::ToolType::TaperedBallNose);
                                let mut renderer = rs.renderer.write();
                                let resources: &mut RenderResources =
                                    renderer.callback_resources.get_mut().unwrap();
                                resources.tool_model_data = Some(ToolModelGpuData::from_tool(
                                    &rs.device,
                                    (tool.diameter / 2.0) as f32,
                                    tool.cutting_length as f32,
                                    is_ball,
                                    [pos.x as f32, pos.y as f32, pos.z as f32],
                                ));
                            }
                        }
                    }
                    return;
                }
                cumulative += tp_moves;
            }
        }
        self.state.simulation.tool_position = None;
    }

    /// Click-to-select: find nearest toolpath to click position in screen space.
    fn handle_viewport_click(&mut self, click_pos: egui::Pos2) {
        let rect = self.viewport_rect;
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }

        let aspect = rect.width() / rect.height();
        let vw = rect.width();
        let vh = rect.height();
        // Convert click to viewport-local coordinates
        let local_x = click_pos.x - rect.min.x;
        let local_y = click_pos.y - rect.min.y;

        let mut best_dist = 15.0f32; // max pick distance in pixels
        let mut best_id = None;

        for tp in &self.state.job.toolpaths {
            if !tp.visible {
                continue;
            }
            // Respect isolation
            if let Some(iso_id) = self.state.viewport.isolate_toolpath {
                if tp.id != iso_id {
                    continue;
                }
            }
            let result = match &tp.result {
                Some(r) => r,
                None => continue,
            };

            // Sample every 20th move for performance
            let moves = &result.toolpath.moves;
            let step = (moves.len() / 200).max(1);
            for j in (0..moves.len()).step_by(step) {
                let m = &moves[j];
                let world = [m.target.x as f32, m.target.y as f32, m.target.z as f32];
                if let Some(screen) = self.camera.project_to_screen(world, aspect, vw, vh) {
                    let dx = screen[0] - local_x;
                    let dy = screen[1] - local_y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < best_dist {
                        best_dist = dist;
                        best_id = Some(tp.id);
                    }
                }
            }
        }

        if let Some(id) = best_id {
            self.state.selection = Selection::Toolpath(id);
            self.pending_upload = true;
        }
    }

    fn draw_viewport(&mut self, ui: &mut egui::Ui) {
        // Compute elapsed time for progress display
        let compute_elapsed = if self.state.job.toolpaths.iter().any(|tp| matches!(tp.status, ComputeStatus::Computing(_))) {
            Some(self.compute_start.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0))
        } else {
            None
        };

        crate::ui::viewport_overlay::draw(
            ui,
            self.state.mode,
            self.state.simulation.active,
            &mut self.state.viewport,
            compute_elapsed,
            &mut self.events,
        );

        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        self.viewport_rect = rect;

        // Click-to-select toolpath in viewport
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.handle_viewport_click(pos);
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

        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) && scroll != 0.0
        {
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

        let callback = ViewportCallback {
            mesh_uniforms: MeshUniforms {
                view_proj,
                light_dir: [0.5, 0.3, 0.8],
                _pad0: 0.0,
                camera_pos: [eye.x, eye.y, eye.z],
                _pad1: 0.0,
            },
            line_uniforms: LineUniforms { view_proj },
            has_mesh: self.state.job.models.iter().any(|m| m.mesh.is_some())
                && self.state.viewport.render_mode == crate::state::viewport::RenderMode::Shaded,
            show_grid: self.state.viewport.show_grid,
            show_stock: self.state.viewport.show_stock
                && self.state.job.models.iter().any(|m| m.mesh.is_some()),
            show_sim_mesh: self.state.simulation.active,
            show_cutting: self.state.viewport.show_cutting,
            show_rapids: self.state.viewport.show_rapids,
            show_collisions: self.state.viewport.show_collisions,
            show_tool_model: self.state.simulation.active && self.state.simulation.tool_position.is_some(),
            toolpath_move_limit: if self.state.simulation.active && self.state.simulation.current_move < self.state.simulation.total_moves {
                Some(self.state.simulation.current_move)
            } else {
                None
            },
            viewport_width: (rect.width() * ppp) as u32,
            viewport_height: (rect.height() * ppp) as u32,
        };

        let cb = egui_wgpu::Callback::new_paint_callback(rect, callback);
        ui.painter().add(cb);
    }

    /// Handle keyboard shortcuts for the viewport and application.
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        // Only process shortcuts when no text edit is focused
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }

        ctx.input(|i| {
            let modifiers = i.modifiers;

            // Delete: remove selected toolpath
            if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                if let Selection::Toolpath(id) = self.state.selection {
                    self.events.push(AppEvent::RemoveToolpath(id));
                }
            }

            // G: generate selected toolpath, Shift+G: generate all
            if i.key_pressed(egui::Key::G) {
                if modifiers.shift {
                    self.events.push(AppEvent::GenerateAll);
                } else if let Selection::Toolpath(id) = self.state.selection {
                    self.events.push(AppEvent::GenerateToolpath(id));
                }
            }

            // Space: play/pause simulation (editor mode — sim mode has its own handler)
            if i.key_pressed(egui::Key::Space) {
                if self.state.simulation.active {
                    self.events.push(AppEvent::EnterSimulation);
                }
            }

            // I: toggle isolation mode
            if i.key_pressed(egui::Key::I) {
                self.events.push(AppEvent::ToggleIsolateToolpath);
            }

            // H: toggle visibility of selected toolpath
            if i.key_pressed(egui::Key::H) {
                if let Selection::Toolpath(id) = self.state.selection {
                    self.events.push(AppEvent::ToggleToolpathVisibility(id));
                }
            }

            // 1-4: view presets
            if i.key_pressed(egui::Key::Num1) {
                self.events.push(AppEvent::SetViewPreset(crate::render::camera::ViewPreset::Top));
            }
            if i.key_pressed(egui::Key::Num2) {
                self.events.push(AppEvent::SetViewPreset(crate::render::camera::ViewPreset::Front));
            }
            if i.key_pressed(egui::Key::Num3) {
                self.events.push(AppEvent::SetViewPreset(crate::render::camera::ViewPreset::Right));
            }
            if i.key_pressed(egui::Key::Num4) {
                self.events.push(AppEvent::SetViewPreset(crate::render::camera::ViewPreset::Isometric));
            }
        });
    }

    // --- Layout methods ---

    fn draw_editor_layout(&mut self, ctx: &egui::Context) {
        // Left panel: project tree
        egui::SidePanel::left("project_tree")
            .default_width(220.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::ui::project_tree::draw(ui, &self.state, &mut self.events);
                });
            });

        // Right panel: properties (mutable state for inline editing)
        egui::SidePanel::right("properties")
            .default_width(280.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::ui::properties::draw(ui, &mut self.state, &mut self.events);
                });
            });

        // Status bar
        let col_count = self.collision_positions.len();
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            crate::ui::status_bar::draw(ui, &self.state, col_count);
        });

        // Central panel: 3D viewport
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(26, 26, 38))
                    .inner_margin(0.0),
            )
            .show(ctx, |ui| {
                self.draw_viewport(ui);
            });
    }

    fn draw_simulation_layout(&mut self, ctx: &egui::Context) {
        // Top bar: Back button + SIMULATION label + display toggles
        egui::TopBottomPanel::top("sim_top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("\u{2190} Back to Editor").clicked() {
                    self.events.push(AppEvent::ExitSimulation);
                }
                ui.separator();
                ui.label(
                    egui::RichText::new("SIMULATION")
                        .strong()
                        .color(egui::Color32::from_rgb(100, 180, 220)),
                );
                ui.separator();

                // Display toggles relevant to sim mode
                ui.checkbox(&mut self.state.viewport.show_cutting, "Paths");
                ui.checkbox(&mut self.state.viewport.show_stock, "Stock");
                ui.checkbox(&mut self.state.viewport.show_collisions, "Collisions");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Re-run Simulation").clicked() {
                        self.events.push(AppEvent::RunSimulation);
                    }
                    if ui.button("Reset").clicked() {
                        self.events.push(AppEvent::ResetSimulation);
                    }
                });
            });
        });

        // Bottom panel: timeline
        egui::TopBottomPanel::bottom("sim_timeline")
            .min_height(60.0)
            .show(ctx, |ui| {
                crate::ui::sim_timeline::draw(
                    ui,
                    &mut self.state.simulation,
                    &self.state.job,
                    &mut self.events,
                );
            });

        // Left panel: operation list
        egui::SidePanel::left("sim_op_list")
            .default_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::ui::sim_op_list::draw(
                        ui,
                        &self.state.simulation,
                        &self.state.job,
                        &mut self.events,
                    );
                });
            });

        // Right panel: diagnostics
        egui::SidePanel::right("sim_diagnostics")
            .default_width(240.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::ui::sim_diagnostics::draw(
                        ui,
                        &mut self.state.simulation,
                        &self.state.job,
                        &mut self.events,
                    );
                });
            });

        // Central panel: 3D viewport
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(26, 26, 38))
                    .inner_margin(0.0),
            )
            .show(ctx, |ui| {
                self.draw_viewport(ui);
            });
    }

    /// Handle keyboard shortcuts for the simulation workspace.
    fn handle_simulation_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }

        ctx.input(|i| {
            // Left/Right: step back/forward
            if i.key_pressed(egui::Key::ArrowLeft) {
                self.events.push(AppEvent::SimStepBackward);
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                self.events.push(AppEvent::SimStepForward);
            }

            // Home/End: jump to start/end
            if i.key_pressed(egui::Key::Home) {
                self.events.push(AppEvent::SimJumpToStart);
            }
            if i.key_pressed(egui::Key::End) {
                self.events.push(AppEvent::SimJumpToEnd);
            }

            // Space: play/pause
            if i.key_pressed(egui::Key::Space) {
                self.events.push(AppEvent::ToggleSimPlayback);
            }

            // Escape: back to editor
            if i.key_pressed(egui::Key::Escape) {
                self.events.push(AppEvent::ExitSimulation);
            }

            // [ / ]: speed down/up
            if i.key_pressed(egui::Key::OpenBracket) {
                self.state.simulation.speed = (self.state.simulation.speed * 0.5).max(10.0);
            }
            if i.key_pressed(egui::Key::CloseBracket) {
                self.state.simulation.speed = (self.state.simulation.speed * 2.0).min(50000.0);
            }
        });
    }
}

impl eframe::App for RsCamApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Drain compute results
        self.drain_compute_results(frame);

        // Upload pending GPU data
        if self.pending_upload {
            self.pending_upload = false;
            self.upload_gpu_data(frame);
        }

        // Update tool model position during sim playback
        if self.state.simulation.active {
            self.update_sim_tool_position(frame);
        }

        self.events.clear();

        // Handle keyboard shortcuts (before UI to prevent conflicts)
        match self.state.mode {
            AppMode::Editor => self.handle_keyboard_shortcuts(ctx),
            AppMode::Simulation => self.handle_simulation_shortcuts(ctx),
        }

        // Menu bar (shown in both modes)
        crate::ui::menu_bar::draw(ctx, &self.state, &mut self.events);

        // Draw mode-specific layout
        match self.state.mode {
            AppMode::Editor => self.draw_editor_layout(ctx),
            AppMode::Simulation => self.draw_simulation_layout(ctx),
        }

        // Pre-flight checklist modal (shown on top of either layout)
        if self.state.show_preflight {
            if !crate::ui::preflight::draw(ctx, &self.state, &mut self.events) {
                self.state.show_preflight = false;
            }
        }

        // Process events after UI pass
        self.handle_events(ctx);

        // Load checkpoint mesh for backward scrubbing
        if self.pending_checkpoint_load {
            self.pending_checkpoint_load = false;
            let move_idx = self.state.simulation.current_move;
            self.load_checkpoint_for_move(move_idx, frame);
        }

        // Advance simulation playback
        if self.state.simulation.playing {
            let dt = ctx.input(|i| i.stable_dt);
            self.state.simulation.advance(dt);
            ctx.request_repaint();
        }

        // Debounced auto-regeneration: if a 2.5D toolpath has been stale for >500ms, regenerate
        let now = std::time::Instant::now();
        let stale_ids: Vec<_> = self.state.job.toolpaths.iter()
            .filter(|tp| tp.auto_regen && !tp.locked)
            .filter_map(|tp| {
                tp.stale_since.filter(|t| now.duration_since(*t).as_millis() > 500).map(|_| tp.id)
            })
            .collect();
        for id in stale_ids {
            if let Some(tp) = self.state.job.toolpaths.iter_mut().find(|t| t.id == id) {
                tp.stale_since = None;
            }
            self.submit_toolpath_compute(id);
        }

        // Keep repainting while computing or playing simulation
        let computing = self.state.job.toolpaths.iter()
            .any(|tp| matches!(tp.status, ComputeStatus::Computing(_)));
        if computing || self.state.simulation.playing {
            ctx.request_repaint();
        }
    }
}

/// Extract the nominal depth from an operation config (for auto-computing bottom_z).
fn operation_depth(op: &OperationConfig) -> f64 {
    match op {
        OperationConfig::Face(c) => c.depth,
        OperationConfig::Pocket(c) => c.depth,
        OperationConfig::Profile(c) => c.depth,
        OperationConfig::Adaptive(c) => c.depth,
        OperationConfig::VCarve(c) => c.max_depth,
        OperationConfig::Rest(c) => c.depth,
        OperationConfig::Inlay(c) => c.pocket_depth,
        OperationConfig::Zigzag(c) => c.depth,
        OperationConfig::Trace(c) => c.depth,
        OperationConfig::Drill(c) => c.depth,
        OperationConfig::Chamfer(c) => c.chamfer_width, // approximate
        OperationConfig::DropCutter(c) => c.min_z.abs(),
        OperationConfig::Adaptive3d(c) => c.stock_top_z,
        OperationConfig::Waterline(c) => (c.start_z - c.final_z).abs(),
        OperationConfig::Pencil(_) => 25.0, // no explicit depth, use default
        OperationConfig::Scallop(_) => 25.0,
        OperationConfig::SteepShallow(_) => 25.0,
        OperationConfig::RampFinish(_) => 25.0,
        OperationConfig::SpiralFinish(_) => 25.0,
        OperationConfig::RadialFinish(_) => 25.0,
        OperationConfig::HorizontalFinish(_) => 25.0,
        OperationConfig::ProjectCurve(c) => c.depth,
    }
}

fn configure_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = egui::Color32::from_rgb(30, 30, 36);
    visuals.window_fill = egui::Color32::from_rgb(30, 30, 36);
    visuals.extreme_bg_color = egui::Color32::from_rgb(22, 22, 28);
    visuals.faint_bg_color = egui::Color32::from_rgb(38, 38, 46);

    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(38, 38, 46);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 45, 56);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 55, 68);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(65, 75, 95);

    visuals.selection.bg_fill = egui::Color32::from_rgb(50, 60, 90);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 140, 210));

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    ctx.set_style(style);
}
