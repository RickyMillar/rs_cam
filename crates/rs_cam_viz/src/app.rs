use std::sync::Arc;

use crate::controller::AppController;
use crate::render::camera::OrbitCamera;
use crate::render::mesh_render::MeshGpuData;
use crate::render::sim_render::{self, SimMeshGpuData, ToolModelGpuData};
use crate::render::stock_render::StockGpuData;
use crate::render::toolpath_render::ToolpathGpuData;
use crate::render::{LineUniforms, MeshUniforms, RenderResources, ViewportCallback};
use crate::state::Workspace;
use crate::state::selection::Selection;
use crate::state::simulation::StockVizMode;
use crate::ui::AppEvent;

pub struct RsCamApp {
    controller: AppController,
    camera: OrbitCamera,
    /// Cached viewport rect for click detection.
    viewport_rect: egui::Rect,
    /// Flag: need to load checkpoint mesh for backward scrubbing on next frame.
    pending_checkpoint_load: bool,
    /// Frame counter for auto-screenshot mode (RS_CAM_SCREENSHOT env var).
    auto_screenshot_frame: Option<u32>,
}

impl RsCamApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_theme(&cc.egui_ctx);

        if let Some(render_state) = cc.wgpu_render_state.as_ref() {
            let resources = RenderResources::new(&render_state.device, render_state.target_format);
            render_state
                .renderer
                .write()
                .callback_resources
                .insert(resources);
        }

        // Auto-screenshot mode: set RS_CAM_SCREENSHOT=1 (or workspace name) to capture and exit.
        let auto_screenshot_frame = std::env::var("RS_CAM_SCREENSHOT").ok().map(|_| 0u32);

        let mut controller = AppController::new();

        // Switch workspace if requested via env var
        if let Ok(val) = std::env::var("RS_CAM_SCREENSHOT") {
            match val.to_lowercase().as_str() {
                "setup" => controller.state_mut().workspace = Workspace::Setup,
                "simulation" | "sim" => {
                    controller.state_mut().workspace = Workspace::Simulation;
                }
                _ => {}
            }
        }

        Self {
            controller,
            camera: OrbitCamera::new(),
            viewport_rect: egui::Rect::NOTHING,
            pending_checkpoint_load: false,
            auto_screenshot_frame,
        }
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        let events = self.controller.drain_events();

        for event in events {
            match event {
                AppEvent::ImportStl(path) => match self.controller.import_stl_path(&path) {
                    Ok(Some(bbox)) => self.fit_camera_to_bbox(&bbox),
                    Ok(None) => {}
                    Err(error) => tracing::error!("STL import failed: {error}"),
                },
                AppEvent::ImportSvg(path) => {
                    if let Err(error) = self.controller.import_svg_path(&path) {
                        tracing::error!("SVG import failed: {error}");
                    }
                }
                AppEvent::ImportDxf(path) => {
                    if let Err(error) = self.controller.import_dxf_path(&path) {
                        tracing::error!("DXF import failed: {error}");
                    }
                }
                AppEvent::RescaleModel(model_id, new_units) => {
                    match self.controller.rescale_model(model_id, new_units) {
                        Ok(Some(bbox)) => self.fit_camera_to_bbox(&bbox),
                        Ok(None) => {}
                        Err(error) => tracing::error!("Rescale failed: {error}"),
                    }
                }
                AppEvent::SetViewPreset(preset) => self.camera.set_preset(preset),
                AppEvent::ResetView => self.fit_camera_to_first_mesh(),

                // Workspace transitions (need camera/viewport changes in app)
                AppEvent::SwitchWorkspace(target) => {
                    let state = self.controller.state_mut();
                    let old = state.workspace;
                    if old != target {
                        // Entering Simulation: save viewport, set sim-friendly defaults
                        if old != Workspace::Simulation && target == Workspace::Simulation {
                            state.simulation.saved_viewport.show_cutting =
                                state.viewport.show_cutting;
                            state.simulation.saved_viewport.show_rapids =
                                state.viewport.show_rapids;
                            state.simulation.saved_viewport.show_stock = state.viewport.show_stock;
                            state.viewport.show_cutting = false;
                            state.viewport.show_rapids = false;
                            state.viewport.show_stock = true;
                        }
                        // Leaving Simulation: restore viewport
                        if old == Workspace::Simulation && target != Workspace::Simulation {
                            state.viewport.show_cutting =
                                state.simulation.saved_viewport.show_cutting;
                            state.viewport.show_rapids =
                                state.simulation.saved_viewport.show_rapids;
                            state.viewport.show_stock = state.simulation.saved_viewport.show_stock;
                        }
                        state.workspace = target;
                    }
                }
                AppEvent::SimStepBackward => {
                    if self.controller.state().simulation.has_results() {
                        let pb = &mut self.controller.state_mut().simulation.playback;
                        pb.playing = false;
                        pb.current_move = pb.current_move.saturating_sub(1);
                        self.pending_checkpoint_load = true;
                    }
                }
                AppEvent::SimJumpToStart => {
                    if self.controller.state().simulation.has_results() {
                        let pb = &mut self.controller.state_mut().simulation.playback;
                        pb.playing = false;
                        pb.current_move = 0;
                        self.pending_checkpoint_load = true;
                    }
                }
                AppEvent::SimJumpToOpStart(boundary_idx) => {
                    if let Some(start) = self
                        .controller
                        .state()
                        .simulation
                        .boundaries()
                        .get(boundary_idx)
                        .map(|b| b.start_move)
                    {
                        let pb = &mut self.controller.state_mut().simulation.playback;
                        pb.playing = false;
                        pb.current_move = start;
                        self.pending_checkpoint_load = true;
                    }
                }

                AppEvent::SimVizModeChanged => {
                    // Re-upload sim mesh on next frame with new viz colors
                    self.controller.set_pending_upload();
                }

                // Export events (need file dialogs)
                AppEvent::ExportGcode => {
                    self.controller.state_mut().show_preflight = true;
                }
                AppEvent::ExportGcodeConfirmed => {
                    self.export_gcode_with_summary();
                }
                AppEvent::ExportCombinedGcode => {
                    match crate::io::export::export_combined_gcode(&self.controller.state().job) {
                        Ok(gcode) => {
                            let default_name =
                                format!("{}_combined.nc", self.controller.state().job.name)
                                    .replace(' ', "_")
                                    .to_lowercase();
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("G-code", &["nc", "gcode", "ngc"])
                                .set_file_name(&default_name)
                                .save_file()
                            {
                                if let Err(error) = std::fs::write(&path, &gcode) {
                                    tracing::error!("Failed to write G-code: {error}");
                                } else {
                                    tracing::info!(
                                        "Exported combined G-code to {}",
                                        path.display()
                                    );
                                }
                            }
                        }
                        Err(error) => tracing::error!("Export failed: {error}"),
                    }
                }
                AppEvent::ExportSetupGcode(setup_id) => {
                    let setup_name = self
                        .controller
                        .state()
                        .job
                        .setups
                        .iter()
                        .find(|setup| setup.id == setup_id)
                        .map(|setup| setup.name.clone())
                        .unwrap_or_default();
                    match crate::io::export::export_setup_gcode(
                        &self.controller.state().job,
                        setup_id,
                    ) {
                        Ok(gcode) => {
                            let default_name =
                                format!("{}_{}.nc", self.controller.state().job.name, setup_name)
                                    .replace(' ', "_")
                                    .to_lowercase();
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("G-code", &["nc", "gcode", "ngc"])
                                .set_file_name(&default_name)
                                .save_file()
                            {
                                if let Err(error) = std::fs::write(&path, &gcode) {
                                    tracing::error!("Failed to write G-code: {error}");
                                } else {
                                    tracing::info!(
                                        "Exported setup '{}' G-code to {}",
                                        setup_name,
                                        path.display()
                                    );
                                }
                            }
                        }
                        Err(error) => tracing::error!("Export failed: {error}"),
                    }
                }
                AppEvent::ExportSvgPreview => self.export_svg_preview(),

                AppEvent::ExportSetupSheet => {
                    let html = self.controller.export_setup_sheet_html();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("HTML", &["html"])
                        .set_file_name("setup_sheet.html")
                        .save_file()
                    {
                        if let Err(error) = std::fs::write(&path, &html) {
                            tracing::error!("Failed to write setup sheet: {error}");
                        } else {
                            tracing::info!("Exported setup sheet to {}", path.display());
                        }
                    }
                }
                AppEvent::SaveJob => {
                    let path = self.controller.state().job.file_path.clone().or_else(|| {
                        rfd::FileDialog::new()
                            .add_filter("TOML Job", &["toml"])
                            .set_file_name("job.toml")
                            .save_file()
                    });
                    if let Some(path) = path {
                        match self.controller.save_job_to_path(&path) {
                            Ok(()) => {
                                tracing::info!("Saved job to {}", path.display());
                            }
                            Err(error) => tracing::error!("Save failed: {error}"),
                        }
                    }
                }
                AppEvent::OpenJob => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("TOML Job", &["toml"])
                        .pick_file()
                    {
                        match self.controller.open_job_from_path(&path) {
                            Ok(()) => {
                                tracing::info!("Loaded job from {}", path.display());
                            }
                            Err(error) => tracing::error!("Load failed: {error}"),
                        }
                    }
                }

                AppEvent::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),

                // Everything else delegated to controller
                other => self.controller.handle_internal_event(other),
            }
        }
    }

    fn fit_camera_to_bbox(&mut self, bbox: &rs_cam_core::geo::BoundingBox3) {
        self.camera.fit_to_bounds(
            [bbox.min.x as f32, bbox.min.y as f32, bbox.min.z as f32],
            [bbox.max.x as f32, bbox.max.y as f32, bbox.max.z as f32],
        );
    }

    fn fit_camera_to_first_mesh(&mut self) {
        if let Some(bbox) = self
            .controller
            .state()
            .job
            .models
            .iter()
            .find_map(|model| model.mesh.as_ref().map(|mesh| mesh.bbox))
        {
            self.fit_camera_to_bbox(&bbox);
        } else {
            self.camera = OrbitCamera::new();
        }
    }

    // --- Simulation helpers ---

    /// Load the nearest checkpoint mesh for backward scrubbing.
    fn load_checkpoint_for_move(&mut self, move_idx: usize, frame: &mut eframe::Frame) {
        if let Some(cp_idx) = self
            .controller
            .state()
            .simulation
            .checkpoint_for_move(move_idx)
        {
            let mesh = match self.controller.state().simulation.checkpoints().get(cp_idx) {
                Some(c) => c.mesh.clone(),
                None => return,
            };
            let colors = self.compute_sim_colors(&mesh);
            self.controller.state_mut().simulation.playback.display_mesh = Some(mesh);
            if let Some(rs) = frame.wgpu_render_state() {
                let mesh_ref = self
                    .controller
                    .state()
                    .simulation
                    .playback
                    .display_mesh
                    .as_ref()
                    .unwrap();
                let mut renderer = rs.renderer.write();
                let resources: &mut RenderResources =
                    renderer.callback_resources.get_mut().unwrap();
                resources.sim_mesh_data = Some(SimMeshGpuData::from_heightmap_mesh_colored(
                    &rs.device, mesh_ref, &colors,
                ));
            }
        }
    }

    /// Compute per-vertex colors for the sim mesh based on current viz mode.
    fn compute_sim_colors(&self, mesh: &rs_cam_core::simulation::HeightmapMesh) -> Vec<[f32; 3]> {
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

    /// Incrementally simulate the stock heightmap to match current_move.
    ///
    /// On forward playback this simulates the new moves since last frame.
    /// On backward scrub it resets from the nearest checkpoint heightmap.
    fn update_live_sim(&mut self, frame: &mut eframe::Frame) {
        use rs_cam_core::simulation::{heightmap_to_mesh, simulate_toolpath_range};

        let target_move = self.controller.state().simulation.playback.current_move;
        let live_move = self.controller.state().simulation.playback.live_sim_move;

        if target_move == live_move {
            return; // nothing changed
        }

        // Collect the toolpath data we need (tool configs + toolpath arcs)
        let tp_data: Vec<_> = self
            .controller
            .state()
            .job
            .all_toolpaths()
            .filter(|tp| tp.enabled)
            .filter_map(|tp| {
                let result = tp.result.as_ref()?;
                let tool = self
                    .controller
                    .state()
                    .job
                    .tools
                    .iter()
                    .find(|t| t.id == tp.tool_id)?
                    .clone();
                Some((Arc::clone(&result.toolpath), tool))
            })
            .collect();

        if tp_data.is_empty() {
            return;
        }

        // If moving backward, reset from nearest checkpoint
        if target_move < live_move {
            // Find the highest checkpoint at or before target_move
            let boundaries = self.controller.state().simulation.boundaries();
            let mut best_cp: Option<usize> = None;
            for (i, b) in boundaries.iter().enumerate() {
                if b.end_move <= target_move {
                    best_cp = Some(i);
                }
            }

            if let Some(cp_idx) = best_cp {
                if let Some(cp) = self.controller.state().simulation.checkpoints().get(cp_idx)
                    && let Some(hm) = &cp.heightmap
                {
                    let hm_clone = hm.clone();
                    let cp_end = boundaries[cp_idx].end_move;
                    let pb = &mut self.controller.state_mut().simulation.playback;
                    pb.live_heightmap = Some(hm_clone);
                    pb.live_sim_move = cp_end;
                }
            } else {
                // Before any checkpoint — reset to fresh stock
                let bbox = self.controller.state().job.stock.bbox();
                let res = self.controller.state().simulation.resolution;
                let fresh =
                    rs_cam_core::simulation::Heightmap::from_bounds(&bbox, Some(bbox.max.z), res);
                let pb = &mut self.controller.state_mut().simulation.playback;
                pb.live_heightmap = Some(fresh);
                pb.live_sim_move = 0;
            }
        }

        // Now simulate forward from live_sim_move to target_move
        let current_live = self.controller.state().simulation.playback.live_sim_move;
        if current_live < target_move {
            // Take the heightmap out to avoid borrow conflicts
            let mut heightmap = self
                .controller
                .state_mut()
                .simulation
                .playback
                .live_heightmap
                .take();

            if let Some(ref mut heightmap) = heightmap {
                let mut global_offset = 0;
                for (toolpath, tool) in &tp_data {
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
                        simulate_toolpath_range(
                            toolpath,
                            cutter.as_ref(),
                            heightmap,
                            local_start,
                            local_end,
                        );
                    }
                    global_offset += tp_moves;
                }
            }

            // Put it back
            let pb = &mut self.controller.state_mut().simulation.playback;
            pb.live_heightmap = heightmap;
            pb.live_sim_move = target_move;
        }

        // Convert heightmap to mesh and upload to GPU
        if let Some(heightmap) = &self.controller.state().simulation.playback.live_heightmap {
            let mesh = heightmap_to_mesh(heightmap);
            let colors = self.compute_sim_colors(&mesh);
            self.controller.state_mut().simulation.playback.display_mesh = Some(mesh);

            if let Some(rs) = frame.wgpu_render_state() {
                let mesh_ref = self
                    .controller
                    .state()
                    .simulation
                    .playback
                    .display_mesh
                    .as_ref()
                    .unwrap();
                let mut renderer = rs.renderer.write();
                let resources: &mut RenderResources =
                    renderer.callback_resources.get_mut().unwrap();
                resources.sim_mesh_data = Some(SimMeshGpuData::from_heightmap_mesh_colored(
                    &rs.device, mesh_ref, &colors,
                ));
            }
        }
    }

    fn export_gcode_with_summary(&self) {
        match self.controller.export_gcode() {
            Ok(gcode) => {
                let line_count = gcode.lines().count();
                let mut total_moves = 0usize;
                let mut cutting_dist = 0.0f64;
                let mut est_time_min = 0.0f64;

                for tp in self.controller.state().job.all_toolpaths() {
                    if tp.enabled
                        && let Some(result) = &tp.result
                    {
                        total_moves += result.stats.move_count;
                        cutting_dist += result.stats.cutting_distance;
                        let feed = tp.operation.feed_rate().max(1.0);
                        est_time_min += result.stats.cutting_distance / feed;
                    }
                }

                let mut seen_tools = Vec::new();
                for tp in self.controller.state().job.all_toolpaths() {
                    if tp.enabled && !seen_tools.contains(&tp.tool_id) {
                        seen_tools.push(tp.tool_id);
                    }
                }
                let tool_changes = if seen_tools.len() > 1 {
                    seen_tools.len() - 1
                } else {
                    0
                };

                tracing::info!(
                    "Export summary: {} G-code lines, {} moves, {:.0} mm cutting, {} tool changes, ~{:.1} min",
                    line_count,
                    total_moves,
                    cutting_dist,
                    tool_changes,
                    est_time_min,
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
            Err(error) => tracing::error!("Export failed: {error}"),
        }
    }

    fn export_svg_preview(&self) {
        match self.controller.export_svg_preview() {
            Ok(svg) => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name("toolpath_preview.svg")
                    .save_file()
                {
                    if let Err(error) = std::fs::write(&path, &svg) {
                        tracing::error!("Failed to write SVG: {error}");
                    } else {
                        tracing::info!("Exported SVG preview to {}", path.display());
                    }
                }
            }
            Err(error) => tracing::warn!("{error}"),
        }
    }

    fn upload_gpu_data(&mut self, frame: &mut eframe::Frame) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };

        let mut renderer = render_state.renderer.write();
        let resources: &mut RenderResources = renderer.callback_resources.get_mut().unwrap();

        // Upload mesh data for the first STL model
        if let Some(model) = self
            .controller
            .state()
            .job
            .models
            .iter()
            .find(|model| model.mesh.is_some())
            && let Some(mesh) = &model.mesh
        {
            resources.mesh_data = Some(MeshGpuData::from_mesh(&render_state.device, mesh));
        }

        // Upload stock wireframe
        let stock_bbox = self.controller.state().job.stock.bbox();
        resources.stock_data = Some(StockGpuData::from_bbox(&render_state.device, &stock_bbox));

        // Upload fixture and keep-out wireframes.
        {
            use crate::render::fixture_render::FixtureGpuData;

            let job = &self.controller.state().job;
            let mut boxes = Vec::new();
            for setup in &job.setups {
                for fixture in &setup.fixtures {
                    if fixture.enabled {
                        boxes.push((fixture.clearance_bbox(), [0.9_f32, 0.7, 0.2]));
                    }
                }
                for keep_out in &setup.keep_out_zones {
                    if keep_out.enabled {
                        let bbox = rs_cam_core::geo::BoundingBox3 {
                            min: rs_cam_core::geo::P3::new(
                                keep_out.origin_x,
                                keep_out.origin_y,
                                job.stock.origin_z,
                            ),
                            max: rs_cam_core::geo::P3::new(
                                keep_out.origin_x + keep_out.size_x,
                                keep_out.origin_y + keep_out.size_y,
                                job.stock.origin_z + job.stock.z,
                            ),
                        };
                        boxes.push((bbox, [0.9_f32, 0.2, 0.2]));
                    }
                }
            }

            let stock_top = job.stock.origin_z + job.stock.z;
            let mut pin_vertices = Vec::new();
            for setup in &job.setups {
                for pin in &setup.alignment_pins {
                    let radius = (pin.diameter / 2.0) as f32;
                    let x = pin.x as f32;
                    let y = pin.y as f32;
                    let z = stock_top as f32;
                    let color = [0.2_f32, 0.9, 0.3];
                    pin_vertices.push(crate::render::LineVertex {
                        position: [x - radius, y, z],
                        color,
                    });
                    pin_vertices.push(crate::render::LineVertex {
                        position: [x + radius, y, z],
                        color,
                    });
                    pin_vertices.push(crate::render::LineVertex {
                        position: [x, y - radius, z],
                        color,
                    });
                    pin_vertices.push(crate::render::LineVertex {
                        position: [x, y + radius, z],
                        color,
                    });
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
                resources.sim_mesh_data = Some(SimMeshGpuData::from_heightmap_mesh_colored(
                    &render_state.device,
                    mesh,
                    &colors,
                ));
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
            let mut verts = Vec::new();
            for p in self.controller.collision_positions() {
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
            use egui_wgpu::wgpu::util::DeviceExt;
            resources.collision_vertex_buffer = Some(render_state.device.create_buffer_init(
                &egui_wgpu::wgpu::util::BufferInitDescriptor {
                    label: Some("collision_markers"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: egui_wgpu::wgpu::BufferUsages::VERTEX,
                },
            ));
            resources.collision_vertex_count = verts.len() as u32;
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

        for (i, tp) in self.controller.state().job.toolpaths_enumerated() {
            // Skip invisible toolpaths; also skip if not the isolated toolpath
            let visible = tp.visible
                && match isolate {
                    Some(iso_id) => tp.id == iso_id,
                    None => true,
                };
            if visible && let Some(result) = &tp.result {
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

    /// Update tool model position during simulation playback.
    fn update_sim_tool_position(&mut self, frame: &mut eframe::Frame) {
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
                        let pos = result.toolpath.moves[local_idx].target;
                        let tool_info = self
                            .controller
                            .state()
                            .job
                            .tools
                            .iter()
                            .find(|tool| tool.id == tp.tool_id)
                            .map(|tool| {
                                (
                                    tool.diameter / 2.0,
                                    tool.cutting_length as f32,
                                    tool.tool_type.label().to_string(),
                                    matches!(
                                        tool.tool_type,
                                        crate::state::job::ToolType::BallNose
                                            | crate::state::job::ToolType::TaperedBallNose
                                    ),
                                )
                            });
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
            if let Some((tool_radius, _, tool_type_label, _)) = &tool_info {
                pb.tool_radius = *tool_radius;
                pb.tool_type_label = tool_type_label.clone();
            }
        }

        if let Some((tool_radius, cutting_length, _, is_ball)) = tool_info
            && let Some(rs) = frame.wgpu_render_state()
        {
            let mut renderer = rs.renderer.write();
            let resources: &mut RenderResources = renderer.callback_resources.get_mut().unwrap();
            resources.tool_model_data = Some(ToolModelGpuData::from_tool(
                &rs.device,
                tool_radius as f32,
                cutting_length,
                is_ball,
                [pos.x as f32, pos.y as f32, pos.z as f32],
            ));
        }
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

        for tp in self.controller.state().job.all_toolpaths() {
            if !tp.visible {
                continue;
            }
            // Respect isolation
            if let Some(iso_id) = self.controller.state().viewport.isolate_toolpath
                && tp.id != iso_id
            {
                continue;
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
            self.controller.state_mut().selection = Selection::Toolpath(id);
        }
    }

    fn draw_viewport(&mut self, ui: &mut egui::Ui) {
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
        if rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) && scroll != 0.0 {
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
                && state.viewport.render_mode == crate::state::viewport::RenderMode::Shaded,
            show_grid: state.viewport.show_grid,
            show_stock: state.viewport.show_stock
                && state.job.models.iter().any(|model| model.mesh.is_some()),
            show_fixtures: state.viewport.show_fixtures,
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
            if (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
                && let Selection::Toolpath(id) = self.controller.state().selection
            {
                self.controller
                    .events_mut()
                    .push(AppEvent::RemoveToolpath(id));
            }

            // G: generate selected toolpath, Shift+G: generate all
            if i.key_pressed(egui::Key::G) {
                if modifiers.shift {
                    self.controller.events_mut().push(AppEvent::GenerateAll);
                } else if let Selection::Toolpath(id) = self.controller.state().selection {
                    self.controller
                        .events_mut()
                        .push(AppEvent::GenerateToolpath(id));
                }
            }

            // Space: switch to simulation workspace if results exist
            if i.key_pressed(egui::Key::Space) && self.controller.state().simulation.has_results() {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Simulation));
            }

            // I: toggle isolation mode
            if i.key_pressed(egui::Key::I) {
                self.controller
                    .events_mut()
                    .push(AppEvent::ToggleIsolateToolpath);
            }

            // H: toggle visibility of selected toolpath
            if i.key_pressed(egui::Key::H)
                && let Selection::Toolpath(id) = self.controller.state().selection
            {
                self.controller
                    .events_mut()
                    .push(AppEvent::ToggleToolpathVisibility(id));
            }

            // 1-4: view presets
            if i.key_pressed(egui::Key::Num1) {
                self.controller.events_mut().push(AppEvent::SetViewPreset(
                    crate::render::camera::ViewPreset::Top,
                ));
            }
            if i.key_pressed(egui::Key::Num2) {
                self.controller.events_mut().push(AppEvent::SetViewPreset(
                    crate::render::camera::ViewPreset::Front,
                ));
            }
            if i.key_pressed(egui::Key::Num3) {
                self.controller.events_mut().push(AppEvent::SetViewPreset(
                    crate::render::camera::ViewPreset::Right,
                ));
            }
            if i.key_pressed(egui::Key::Num4) {
                self.controller.events_mut().push(AppEvent::SetViewPreset(
                    crate::render::camera::ViewPreset::Isometric,
                ));
            }
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
                self.controller.events_mut().push(AppEvent::SimStepBackward);
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                self.controller.events_mut().push(AppEvent::SimStepForward);
            }

            // Home/End: jump to start/end
            if i.key_pressed(egui::Key::Home) {
                self.controller.events_mut().push(AppEvent::SimJumpToStart);
            }
            if i.key_pressed(egui::Key::End) {
                self.controller.events_mut().push(AppEvent::SimJumpToEnd);
            }

            // Space: play/pause
            if i.key_pressed(egui::Key::Space) {
                self.controller
                    .events_mut()
                    .push(AppEvent::ToggleSimPlayback);
            }

            // Escape: back to toolpaths workspace
            if i.key_pressed(egui::Key::Escape) {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Toolpaths));
            }

            // [ / ]: speed down/up
            if i.key_pressed(egui::Key::OpenBracket) {
                let pb = &mut self.controller.state_mut().simulation.playback;
                pb.speed = (pb.speed * 0.5).max(10.0);
            }
            if i.key_pressed(egui::Key::CloseBracket) {
                let pb = &mut self.controller.state_mut().simulation.playback;
                pb.speed = (pb.speed * 2.0).min(50000.0);
            }
        });
    }

    // --- Layout methods ---

    fn draw_setup_layout(&mut self, ctx: &egui::Context) {
        // Left panel: setup list with summary cards
        egui::SidePanel::left("setup_tree")
            .default_width(240.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (state, events) = self.controller.state_ref_and_events_mut();
                    crate::ui::setup_panel::draw(ui, state, events);
                });
            });

        // Right panel: setup properties
        egui::SidePanel::right("setup_properties")
            .default_width(280.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (state, events) = self.controller.state_and_events_mut();
                    crate::ui::properties::draw(ui, state, events);
                });
            });

        let col_count = self.controller.collision_positions().len();
        let lane_snapshots = self.controller.lane_snapshots();
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            crate::ui::status_bar::draw(ui, self.controller.state(), col_count, &lane_snapshots);
        });

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

    fn draw_toolpath_layout(&mut self, ctx: &egui::Context) {
        // Left panel: project tree (toolpath-focused)
        egui::SidePanel::left("toolpath_tree")
            .default_width(220.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (state, events) = self.controller.state_ref_and_events_mut();
                    crate::ui::project_tree::draw(ui, state, events);
                });
            });

        // Right panel: operation/tool parameters
        egui::SidePanel::right("toolpath_properties")
            .default_width(280.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (state, events) = self.controller.state_and_events_mut();
                    crate::ui::properties::draw(ui, state, events);
                });
            });

        let col_count = self.controller.collision_positions().len();
        let lane_snapshots = self.controller.lane_snapshots();
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            crate::ui::status_bar::draw(ui, self.controller.state(), col_count, &lane_snapshots);
        });

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
        // Sim action bar: display toggles + re-run/reset
        egui::TopBottomPanel::top("sim_top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                {
                    let viewport = &mut self.controller.state_mut().viewport;
                    ui.checkbox(&mut viewport.show_cutting, "Paths");
                    ui.checkbox(&mut viewport.show_stock, "Stock");
                    ui.checkbox(&mut viewport.show_fixtures, "Fixtures");
                    ui.checkbox(&mut viewport.show_collisions, "Collisions");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Re-run").clicked() {
                        self.controller.events_mut().push(AppEvent::RunSimulation);
                    }
                    if ui.button("Reset").clicked() {
                        self.controller.events_mut().push(AppEvent::ResetSimulation);
                    }
                });
            });
        });

        // Bottom panel: timeline
        egui::TopBottomPanel::bottom("sim_timeline")
            .min_height(60.0)
            .show(ctx, |ui| {
                let (state, events) = self.controller.state_and_events_mut();
                crate::ui::sim_timeline::draw(ui, &mut state.simulation, &state.job, events);
            });

        // Left panel: operation list
        egui::SidePanel::left("sim_op_list")
            .default_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (state, events) = self.controller.state_ref_and_events_mut();
                    crate::ui::sim_op_list::draw(ui, &state.simulation, &state.job, events);
                });
            });

        // Right panel: diagnostics
        egui::SidePanel::right("sim_diagnostics")
            .default_width(240.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (state, events) = self.controller.state_and_events_mut();
                    crate::ui::sim_diagnostics::draw(ui, &mut state.simulation, &state.job, events);
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

    /// Save an egui screenshot to a PNG file in the current directory.
    fn save_screenshot(image: &egui::ColorImage) {
        let pixels: Vec<u8> = image
            .pixels
            .iter()
            .flat_map(|c| [c.r(), c.g(), c.b(), c.a()])
            .collect();
        let img_buf: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            match image::ImageBuffer::from_raw(image.size[0] as u32, image.size[1] as u32, pixels) {
                Some(buf) => buf,
                None => {
                    tracing::error!("Failed to create image buffer from screenshot");
                    return;
                }
            };

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let path = format!("screenshot_{timestamp}.png");
        match img_buf.save(&path) {
            Ok(()) => tracing::info!("Screenshot saved to {path}"),
            Err(e) => tracing::error!("Failed to save screenshot: {e}"),
        }
    }
}

impl eframe::App for RsCamApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Handle screenshot results from previous frame
        ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    Self::save_screenshot(image);
                }
            }
        });

        // F12: request screenshot
        if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }

        crate::ui::automation::begin_frame(ctx);

        self.controller.drain_compute_results();

        if self.controller.take_pending_upload() {
            self.upload_gpu_data(frame);
        }

        if self.controller.state().workspace == Workspace::Simulation
            && self.controller.state().simulation.has_results()
        {
            self.update_sim_tool_position(frame);
        }

        // Handle keyboard shortcuts (before UI to prevent conflicts)
        match self.controller.state().workspace {
            Workspace::Setup | Workspace::Toolpaths => self.handle_keyboard_shortcuts(ctx),
            Workspace::Simulation => self.handle_simulation_shortcuts(ctx),
        }

        // Menu bar (shown in all workspaces)
        {
            let (state, events) = self.controller.state_ref_and_events_mut();
            crate::ui::menu_bar::draw(ctx, state, events);
        }

        // Workspace switcher bar (shown in all workspaces)
        egui::TopBottomPanel::top("workspace_bar")
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(34, 34, 42))
                    .inner_margin(egui::Margin::symmetric(8.0, 2.0)),
            )
            .show(ctx, |ui| {
                let (state, events) = self.controller.state_ref_and_events_mut();
                crate::ui::workspace_bar::draw(ui, state, events);
            });

        // Draw workspace-specific layout
        match self.controller.state().workspace {
            Workspace::Setup => self.draw_setup_layout(ctx),
            Workspace::Toolpaths => self.draw_toolpath_layout(ctx),
            Workspace::Simulation => self.draw_simulation_layout(ctx),
        }

        // Pre-flight checklist modal (shown on top of either layout)
        if self.controller.state().show_preflight {
            let (state, events) = self.controller.state_ref_and_events_mut();
            if !crate::ui::preflight::draw(ctx, state, events) {
                self.controller.state_mut().show_preflight = false;
            }
        }

        self.handle_events(ctx);

        // Load checkpoint mesh for backward scrubbing
        if self.pending_checkpoint_load {
            self.pending_checkpoint_load = false;
            let move_idx = self.controller.state().simulation.playback.current_move;
            self.load_checkpoint_for_move(move_idx, frame);
        }

        // Load warnings window
        if self.controller.show_load_warnings() {
            let mut show = self.controller.show_load_warnings();
            egui::Window::new("Project Load Warnings")
                .open(&mut show)
                .resizable(true)
                .show(ctx, |ui| {
                    let response =
                        ui.label("The project loaded, but some references need attention:");
                    crate::ui::automation::record(
                        ui,
                        "project_load_warnings",
                        &response,
                        "Project Load Warnings",
                    );
                    ui.add_space(6.0);
                    for warning in self.controller.load_warnings() {
                        ui.label(format!("\u{2022} {warning}"));
                    }
                });
            self.controller.set_show_load_warnings(show);
        }

        // Advance simulation playback
        if self.controller.state().simulation.playback.playing {
            let dt = ctx.input(|i| i.stable_dt);
            self.controller.state_mut().simulation.advance(dt);
            ctx.request_repaint();
        }

        // Incremental stock simulation: update live heightmap to match current_move
        if self.controller.state().workspace == Workspace::Simulation
            && self.controller.state().simulation.has_results()
        {
            self.update_live_sim(frame);
        }

        self.controller.process_auto_regen();

        let active_lanes = self
            .controller
            .lane_snapshots()
            .into_iter()
            .any(|lane| lane.is_active() || lane.queue_depth > 0);
        if active_lanes || self.controller.state().simulation.playback.playing {
            ctx.request_repaint();
        }

        // Auto-screenshot mode: request on frame 3, save on frame 4, exit on frame 5
        if let Some(ref mut frame_count) = self.auto_screenshot_frame {
            *frame_count += 1;
            if *frame_count == 3 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                ctx.request_repaint();
            }
            if *frame_count >= 6 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
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
