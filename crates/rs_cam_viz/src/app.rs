#![deny(clippy::indexing_slicing)]

use std::sync::Arc;

use crate::controller::AppController;
use crate::render::camera::OrbitCamera;
use crate::render::mesh_render::MeshGpuData;
use crate::render::sim_render::{self, SimMeshGpuData, ToolModelGpuData};
use crate::render::stock_render::StockGpuData;
use crate::render::toolpath_render::{self, ToolpathGpuData};
use crate::render::{LineUniforms, MeshUniforms, RenderResources, ViewportCallback};
use crate::state::Workspace;
use crate::state::job::transform_mesh;
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
    /// Currently hovered BREP face (updated on mouse move in Toolpaths workspace).
    last_hover_face: Option<rs_cam_core::enriched_mesh::FaceGroupId>,
    /// Flag: show the unsaved-changes confirmation dialog before quitting.
    show_quit_dialog: bool,
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

        // Load job file if RS_CAM_JOB is set
        if let Ok(job_path) = std::env::var("RS_CAM_JOB") {
            let path = std::path::Path::new(&job_path);
            match controller.open_job_from_path(path) {
                Ok(()) => tracing::info!("Loaded job from {}", path.display()),
                Err(e) => tracing::error!("Failed to load job: {e}"),
            }
        }

        // Select a specific setup by index via RS_CAM_SETUP
        if let Ok(setup_str) = std::env::var("RS_CAM_SETUP")
            && let Ok(idx) = setup_str.parse::<usize>()
            && let Some(setup) = controller.state().job.setups.get(idx)
        {
            let setup_id = setup.id;
            controller.state_mut().selection = crate::state::selection::Selection::Setup(setup_id);
        }

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
            last_hover_face: None,
            show_quit_dialog: false,
        }
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        let events = self.controller.drain_events();

        for event in events {
            match event {
                AppEvent::ImportStl(path) => match self.controller.import_stl_path(&path) {
                    Ok(Some(bbox)) => self.fit_camera_to_bbox(&bbox),
                    Ok(None) => {}
                    Err(error) => self.controller.push_error(&error),
                },
                AppEvent::ImportSvg(path) => match self.controller.import_svg_path(&path) {
                    Ok(Some(bbox)) => self.fit_camera_to_bbox(&bbox),
                    Ok(None) => self.fit_camera_to_first_model(),
                    Err(error) => self.controller.push_error(&error),
                },
                AppEvent::ImportDxf(path) => match self.controller.import_dxf_path(&path) {
                    Ok(Some(bbox)) => self.fit_camera_to_bbox(&bbox),
                    Ok(None) => self.fit_camera_to_first_model(),
                    Err(error) => self.controller.push_error(&error),
                },
                AppEvent::ImportStep(path) => match self.controller.import_step_path(&path) {
                    Ok(Some(bbox)) => self.fit_camera_to_bbox(&bbox),
                    Ok(None) => {}
                    Err(error) => self.controller.push_error(&error),
                },
                AppEvent::RescaleModel(model_id, new_units) => {
                    match self.controller.rescale_model(model_id, new_units) {
                        Ok(Some(bbox)) => self.fit_camera_to_bbox(&bbox),
                        Ok(None) => {}
                        Err(error) => self.controller.push_error(&error),
                    }
                }
                AppEvent::SetViewPreset(preset) => self.camera.set_preset(preset),
                AppEvent::PreviewOrientation(face_up) => {
                    use crate::state::job::FaceUp;
                    match face_up {
                        FaceUp::Top => {
                            self.camera.pitch = std::f32::consts::FRAC_PI_2 - 0.01;
                        }
                        FaceUp::Bottom => {
                            self.camera.pitch = -(std::f32::consts::FRAC_PI_2 - 0.01);
                        }
                        FaceUp::Front => {
                            self.camera.yaw = 0.0;
                            self.camera.pitch = 0.0;
                        }
                        FaceUp::Back => {
                            self.camera.yaw = std::f32::consts::PI;
                            self.camera.pitch = 0.0;
                        }
                        FaceUp::Left => {
                            self.camera.yaw = std::f32::consts::FRAC_PI_2;
                            self.camera.pitch = 0.0;
                        }
                        FaceUp::Right => {
                            self.camera.yaw = -std::f32::consts::FRAC_PI_2;
                            self.camera.pitch = 0.0;
                        }
                    }
                }
                AppEvent::ResetView => self.fit_camera_to_first_model(),

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
                AppEvent::SimJumpToMove(move_idx) => {
                    if self.controller.state().simulation.has_results() {
                        let total = self.controller.state().simulation.total_moves();
                        let previous = self.controller.state().simulation.playback.current_move;
                        let pb = &mut self.controller.state_mut().simulation.playback;
                        pb.playing = false;
                        pb.current_move = move_idx.min(total);
                        self.pending_checkpoint_load = pb.current_move < previous;
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
                                    self.controller.push_notification(
                                        format!("Failed to write G-code: {error}"),
                                        crate::controller::Severity::Error,
                                    );
                                } else {
                                    tracing::info!(
                                        "Exported combined G-code to {}",
                                        path.display()
                                    );
                                    self.controller.push_notification(
                                        format!(
                                            "Exported combined G-code to {}",
                                            path.display()
                                        ),
                                        crate::controller::Severity::Info,
                                    );
                                }
                            }
                        }
                        Err(error) => self.controller.push_error(&error),
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
                                    self.controller.push_notification(
                                        format!("Failed to write G-code: {error}"),
                                        crate::controller::Severity::Error,
                                    );
                                } else {
                                    tracing::info!(
                                        "Exported setup '{}' G-code to {}",
                                        setup_name,
                                        path.display()
                                    );
                                    self.controller.push_notification(
                                        format!(
                                            "Exported setup '{}' G-code to {}",
                                            setup_name,
                                            path.display()
                                        ),
                                        crate::controller::Severity::Info,
                                    );
                                }
                            }
                        }
                        Err(error) => self.controller.push_error(&error),
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
                            self.controller.push_notification(
                                format!("Failed to write setup sheet: {error}"),
                                crate::controller::Severity::Error,
                            );
                        } else {
                            tracing::info!("Exported setup sheet to {}", path.display());
                            self.controller.push_notification(
                                format!("Exported setup sheet to {}", path.display()),
                                crate::controller::Severity::Info,
                            );
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
                                self.controller.push_notification(
                                    format!("Saved job to {}", path.display()),
                                    crate::controller::Severity::Info,
                                );
                            }
                            Err(error) => self.controller.push_error(&error),
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
                            Err(error) => self.controller.push_error(&error),
                        }
                    }
                }

                AppEvent::ShowShortcuts => {
                    self.controller.state_mut().show_shortcuts = true;
                }

                AppEvent::Quit => {
                    if self.controller.state().job.dirty {
                        self.show_quit_dialog = true;
                    } else {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }

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

    fn fit_camera_to_first_model(&mut self) {
        if let Some(bbox) = self
            .controller
            .state()
            .job
            .models
            .iter()
            .find_map(|model| model.bbox())
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

    /// Compute per-vertex colors for the sim mesh based on current viz mode.
    // SAFETY: color indices bounded by `num_verts * 3` guard above
    #[allow(clippy::indexing_slicing)]
    fn compute_sim_colors(&self, mesh: &rs_cam_core::simulation::StockMesh) -> Vec<[f32; 3]> {
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
    // SAFETY: cp_idx is from enumerate over boundaries; vertex loop uses step_by(3) within len
    #[allow(clippy::indexing_slicing)]
    fn update_live_sim(&mut self, frame: &mut eframe::Frame) {
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
                    && let Some(stock) = &cp.stock
                {
                    let stock_clone = stock.clone();
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
                    .unwrap_or_else(|| self.controller.state().job.stock.bbox());
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
                            cutter.as_ref(),
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
    fn active_setup_orientation(
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

    /// Transform a global-frame `StockMesh` to the active setup's local
    /// frame for the given simulation `move_idx`.  No-op for identity setups.
    // SAFETY: step_by(3) loop with i+1, i+2 bounded by vertices.len() (always multiple of 3)
    #[allow(clippy::indexing_slicing)]
    fn transform_mesh_to_local_frame(
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

    fn export_gcode_with_summary(&mut self) {
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
                        self.controller.push_notification(
                            format!("Failed to write G-code: {e}"),
                            crate::controller::Severity::Error,
                        );
                    } else {
                        tracing::info!("Exported G-code to {}", path.display());
                        self.controller.push_notification(
                            format!("Exported G-code to {}", path.display()),
                            crate::controller::Severity::Info,
                        );
                    }
                }
            }
            Err(error) => {
                tracing::error!("Export failed: {error}");
                self.controller.push_notification(
                    format!("Export failed: {error}"),
                    crate::controller::Severity::Error,
                );
            }
        }
    }

    fn export_svg_preview(&mut self) {
        match self.controller.export_svg_preview() {
            Ok(svg) => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name("toolpath_preview.svg")
                    .save_file()
                {
                    if let Err(error) = std::fs::write(&path, &svg) {
                        tracing::error!("Failed to write SVG: {error}");
                        self.controller.push_notification(
                            format!("Failed to write SVG: {error}"),
                            crate::controller::Severity::Error,
                        );
                    } else {
                        tracing::info!("Exported SVG preview to {}", path.display());
                        self.controller.push_notification(
                            format!("Exported SVG preview to {}", path.display()),
                            crate::controller::Severity::Info,
                        );
                    }
                }
            }
            Err(error) => {
                tracing::warn!("{error}");
                self.controller.push_notification(
                    format!("{error}"),
                    crate::controller::Severity::Warning,
                );
            }
        }
    }

    /// Render the unsaved-changes confirmation dialog when the user tries to quit.
    fn show_unsaved_changes_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_quit_dialog {
            return;
        }
        egui::Window::new("Unsaved Changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("You have unsaved changes. What would you like to do?");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save & Quit").clicked() {
                        let path =
                            self.controller.state().job.file_path.clone().or_else(|| {
                                rfd::FileDialog::new()
                                    .add_filter("TOML Job", &["toml"])
                                    .set_file_name("job.toml")
                                    .save_file()
                            });
                        if let Some(path) = path {
                            match self.controller.save_job_to_path(&path) {
                                Ok(()) => {
                                    tracing::info!("Saved job to {}", path.display());
                                    self.show_quit_dialog = false;
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                Err(error) => self.controller.push_error(&error),
                            }
                        }
                        // If user cancelled the file dialog, keep the dialog open
                    }
                    if ui.button("Discard & Quit").clicked() {
                        self.show_quit_dialog = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_quit_dialog = false;
                    }
                });
            });
    }

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

    fn upload_gpu_data(&mut self, frame: &mut eframe::Frame) {
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
                    push_circle_vertices(&mut pin_vertices, cx, cy, cz, radius, color, 16);
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
                    push_dashed_line_vertices(&mut pin_vertices, s, e, axis_color, 5.0, 3.0);
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
                resources.sim_mesh_data = SimMeshGpuData::from_heightmap_mesh_colored(
                    &render_state.device,
                    &resources.gpu_limits,
                    mesh,
                    &colors,
                );
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

    /// Update tool model position during simulation playback.
    // SAFETY: local_idx bounds-checked against moves.len() before indexing
    #[allow(clippy::indexing_slicing)]
    fn update_sim_tool_position(&mut self, frame: &mut eframe::Frame) {
        use crate::render::sim_render::{ToolGeometry, ToolShape};

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
                pb.tool_type_label = tool.tool_type.label().to_string();
            }
        }

        if let Some(tool) = tool_info
            && let Some(rs) = frame.wgpu_render_state()
        {
            let shape = match tool.tool_type {
                crate::state::job::ToolType::EndMill => ToolShape::FlatEnd,
                crate::state::job::ToolType::BallNose => ToolShape::BallNose,
                crate::state::job::ToolType::BullNose => ToolShape::BullNose,
                crate::state::job::ToolType::VBit => ToolShape::VBit,
                crate::state::job::ToolType::TaperedBallNose => ToolShape::TaperedBallNose,
            };
            let geom = ToolGeometry {
                radius: (tool.diameter / 2.0) as f32,
                cutting_length: tool.cutting_length as f32,
                shape,
                corner_radius: tool.corner_radius as f32,
                included_angle: tool.included_angle as f32,
                taper_half_angle: tool.taper_half_angle as f32,
            };
            let mut renderer = rs.renderer.write();
            // SAFETY: RenderResources inserted in RsCamApp::new; always present.
            #[allow(clippy::unwrap_used)]
            let resources: &mut RenderResources = renderer.callback_resources.get_mut().unwrap();
            let assembly_info = crate::render::sim_render::ToolAssemblyInfo {
                shank_radius: (tool.shank_diameter / 2.0) as f32,
                shank_length: tool.shank_length as f32,
                holder_radius: (tool.holder_diameter / 2.0) as f32,
                stickout: tool.stickout as f32,
            };
            resources.tool_model_data = Some(ToolModelGpuData::from_tool_assembly(
                &rs.device,
                &geom,
                &assembly_info,
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
        // Record whether a text field (or any widget) has focus *before*
        // processing key events.  Escape causes egui to clear focus, so
        // checking inside the input closure would miss the just-cleared
        // widget and fire the workspace-switch shortcut unexpectedly.
        let has_focus = ctx.memory(|m| m.focused().is_some());

        if has_focus {
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
            if let Some(msg) = self.controller.status_message() {
                ui.separator();
                ui.label(egui::RichText::new(msg).color(egui::Color32::from_rgb(255, 200, 80)));
            }
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
        // Left panel: operation queue
        egui::SidePanel::left("toolpath_tree")
            .default_width(230.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (state, events) = self.controller.state_ref_and_events_mut();
                    crate::ui::toolpath_panel::draw(ui, state, events);
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
            if let Some(msg) = self.controller.status_message() {
                ui.separator();
                ui.label(egui::RichText::new(msg).color(egui::Color32::from_rgb(255, 200, 80)));
            }
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
                    let (simulation, viewport, _) =
                        self.controller.simulation_viewport_and_events_mut();
                    ui.label(
                        egui::RichText::new("View:")
                            .small()
                            .color(egui::Color32::from_rgb(130, 130, 145)),
                    );
                    ui.checkbox(&mut viewport.show_cutting, "Paths");
                    ui.checkbox(&mut viewport.show_stock, "Stock");
                    ui.checkbox(&mut viewport.show_fixtures, "Fixtures");
                    ui.checkbox(&mut viewport.show_collisions, "Collisions");
                    ui.separator();
                    ui.label(
                        egui::RichText::new("Analysis:")
                            .small()
                            .color(egui::Color32::from_rgb(130, 130, 145)),
                    );
                    let debug_changed = ui
                        .checkbox(&mut simulation.debug.enabled, "Debug")
                        .changed();
                    if debug_changed && simulation.debug.enabled {
                        simulation.debug.drawer_open = true;
                    }
                    ui.checkbox(&mut simulation.metric_options.enabled, "Metrics")
                        .on_hover_text("Capture simulation-time cutting metrics on the next run.");
                    if simulation.debug.enabled {
                        ui.checkbox(&mut simulation.debug.highlight_active_item, "Highlight");
                    }
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
                    let (state, events) = self.controller.state_and_events_mut();
                    crate::ui::sim_op_list::draw(ui, &mut state.simulation, &state.job, events);
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
        // Intercept OS close button when there are unsaved changes
        let os_close_requested = ctx.input(|i| i.viewport().close_requested());
        if os_close_requested && self.controller.state().job.dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.show_quit_dialog = true;
        }

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

        // Keyboard shortcuts reference window
        if self.controller.state().show_shortcuts {
            let mut show = true;
            crate::ui::shortcuts_window::draw(ctx, &mut show);
            if !show {
                self.controller.state_mut().show_shortcuts = false;
            }
        }

        self.handle_events(ctx);

        // Unsaved-changes confirmation dialog (shown on top of everything)
        self.show_unsaved_changes_dialog(ctx);

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

        // Toast notifications (bottom-right corner)
        {
            self.controller.gc_notifications();
            let notifications: Vec<_> = self
                .controller
                .active_notifications()
                .map(|n| (n.message.clone(), n.severity))
                .collect();
            if !notifications.is_empty() {
                egui::Area::new(egui::Id::new("toast_notifications"))
                    .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -12.0))
                    .show(ctx, |ui| {
                        for (message, severity) in &notifications {
                            let (bg, text_color) = match severity {
                                crate::controller::Severity::Info => {
                                    (egui::Color32::from_rgb(40, 40, 50), egui::Color32::WHITE)
                                }
                                crate::controller::Severity::Warning => (
                                    egui::Color32::from_rgb(80, 60, 10),
                                    egui::Color32::from_rgb(255, 220, 100),
                                ),
                                crate::controller::Severity::Error => (
                                    egui::Color32::from_rgb(80, 20, 20),
                                    egui::Color32::from_rgb(255, 120, 120),
                                ),
                            };
                            egui::Frame::default()
                                .fill(bg)
                                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                                .rounding(6.0)
                                .show(ui, |ui: &mut egui::Ui| {
                                    ui.colored_label(text_color, message);
                                });
                            ui.add_space(4.0);
                        }
                    });
                ctx.request_repaint_after(std::time::Duration::from_secs(1));
            }
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

/// Push line-segment vertices approximating a circle in the XY plane at height `cz`.
fn push_circle_vertices(
    verts: &mut Vec<crate::render::LineVertex>,
    cx: f32,
    cy: f32,
    cz: f32,
    radius: f32,
    color: [f32; 3],
    segments: usize,
) {
    let step = std::f32::consts::TAU / segments as f32;
    for i in 0..segments {
        let a0 = i as f32 * step;
        let a1 = (i + 1) as f32 * step;
        verts.push(crate::render::LineVertex {
            position: [cx + radius * a0.cos(), cy + radius * a0.sin(), cz],
            color,
        });
        verts.push(crate::render::LineVertex {
            position: [cx + radius * a1.cos(), cy + radius * a1.sin(), cz],
            color,
        });
    }
}

/// Push line-segment vertices for a dashed line from `start` to `end`.
fn push_dashed_line_vertices(
    verts: &mut Vec<crate::render::LineVertex>,
    start: [f32; 3],
    end: [f32; 3],
    color: [f32; 3],
    dash_len: f32,
    gap_len: f32,
) {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let dz = end[2] - start[2];
    let total = (dx * dx + dy * dy + dz * dz).sqrt();
    if total < 1e-6 {
        return;
    }
    let ux = dx / total;
    let uy = dy / total;
    let uz = dz / total;

    let cycle = dash_len + gap_len;
    let mut t = 0.0_f32;
    while t < total {
        let t_end = (t + dash_len).min(total);
        verts.push(crate::render::LineVertex {
            position: [start[0] + ux * t, start[1] + uy * t, start[2] + uz * t],
            color,
        });
        verts.push(crate::render::LineVertex {
            position: [
                start[0] + ux * t_end,
                start[1] + uy * t_end,
                start[2] + uz * t_end,
            ],
            color,
        });
        t += cycle;
    }
}
