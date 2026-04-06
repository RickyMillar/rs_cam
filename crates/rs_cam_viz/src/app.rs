#![deny(clippy::indexing_slicing)]

mod export;
mod gpu_upload;
mod input;
mod simulation;
mod viewport;

use crate::controller::AppController;
use crate::render::RenderResources;
use crate::render::camera::OrbitCamera;
use crate::state::Workspace;

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
    /// Track toolpath color mode changes to trigger re-upload.
    last_tp_color_mode: crate::state::viewport::ToolpathColorMode,
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
            last_tp_color_mode: crate::state::viewport::ToolpathColorMode::Normal,
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
            .default_width(240.0)
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
                    ui.checkbox(&mut viewport.show_polygons, "Curves");
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
                        self.controller
                            .events_mut()
                            .push(crate::ui::AppEvent::RunSimulation);
                    }
                    if ui.button("Reset").clicked() {
                        self.controller
                            .events_mut()
                            .push(crate::ui::AppEvent::ResetSimulation);
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
            .default_width(240.0)
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

        // Re-upload toolpath GPU data when color mode changes
        let current_tp_mode = self.controller.state().viewport.toolpath_color_mode;
        if current_tp_mode != self.last_tp_color_mode {
            self.last_tp_color_mode = current_tp_mode;
            self.controller.set_pending_upload();
        }

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
                        ui.set_max_width(400.0);
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
                                .rounding(4.0)
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

    configure_fonts(ctx);
}

/// Add symbol fallback fonts so Unicode glyphs (▶, ⠿, ✓, ⚠, etc.) render correctly.
fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // NotoSansSymbols covers geometric shapes (▶●○), arrows (→), math (≤), checkmarks (✓)
    fonts.font_data.insert(
        "noto_symbols".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSansSymbols-Regular.ttf"
        ))),
    );

    // NotoSansSymbols2 covers braille (⠿), dingbats (✗✅❌), and extended symbols
    fonts.font_data.insert(
        "noto_symbols2".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSansSymbols2-Regular.ttf"
        ))),
    );

    // Append as fallbacks (after egui's default fonts) for proportional text
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.push("noto_symbols".to_owned());
        family.push("noto_symbols2".to_owned());
    }

    ctx.set_fonts(fonts);
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
