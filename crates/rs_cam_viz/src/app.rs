use crate::io::import;
use crate::render::camera::OrbitCamera;
use crate::render::mesh_render::MeshGpuData;
use crate::render::stock_render::StockGpuData;
use crate::render::{LineUniforms, MeshUniforms, RenderResources, ViewportCallback};
use crate::state::AppState;
use crate::state::job::ToolConfig;
use crate::state::selection::Selection;
use crate::ui::AppEvent;

pub struct RsCamApp {
    state: AppState,
    camera: OrbitCamera,
    events: Vec<AppEvent>,
    /// Flag: need to upload mesh/stock to GPU on next frame.
    pending_upload: bool,
}

impl RsCamApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_theme(&cc.egui_ctx);

        // Initialize wgpu render resources
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
            pending_upload: false,
        }
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        let events: Vec<AppEvent> = self.events.drain(..).collect();

        for event in events {
            match event {
                AppEvent::ImportStl(path) => {
                    let id = self.state.job.next_model_id();
                    match import::import_stl(&path, id) {
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
                            self.state.job.dirty = true;
                            self.pending_upload = true;
                        }
                        Err(e) => {
                            tracing::error!("Import failed: {}", e);
                        }
                    }
                }
                AppEvent::Select(sel) => {
                    self.state.selection = sel;
                }
                AppEvent::SetViewPreset(preset) => {
                    self.camera.set_preset(preset);
                }
                AppEvent::ResetView => {
                    if let Some(model) = self.state.job.models.first() {
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
                    self.state.job.dirty = true;
                }
                AppEvent::DuplicateTool(tool_id) => {
                    if let Some(src) = self.state.job.tools.iter().find(|t| t.id == tool_id) {
                        let mut dup = src.clone();
                        let new_id = self.state.job.next_tool_id();
                        dup.id = new_id;
                        dup.name = format!("{} (copy)", dup.name);
                        self.state.selection = Selection::Tool(new_id);
                        self.state.job.tools.push(dup);
                        self.state.job.dirty = true;
                    }
                }
                AppEvent::RemoveTool(tool_id) => {
                    self.state.job.tools.retain(|t| t.id != tool_id);
                    if self.state.selection == Selection::Tool(tool_id) {
                        self.state.selection = Selection::None;
                    }
                    self.state.job.dirty = true;
                }
                AppEvent::StockChanged => {
                    self.pending_upload = true;
                    self.state.job.dirty = true;
                }
                AppEvent::Quit => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
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
    }

    fn draw_viewport(&mut self, ui: &mut egui::Ui) {
        // Viewport overlay buttons
        crate::ui::viewport_overlay::draw(ui, &mut self.events);

        // Allocate remaining space for the 3D viewport
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        // Handle camera input
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

        // Handle scroll zoom
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) && scroll != 0.0
        {
            self.camera.zoom(scroll);
        }

        // Build camera matrices
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
            has_mesh: self
                .state
                .job
                .models
                .iter()
                .any(|m| m.mesh.is_some()),
            show_grid: self.state.viewport.show_grid,
            show_stock: self.state.viewport.show_stock
                && self.state.job.models.iter().any(|m| m.mesh.is_some()),
            viewport_width: (rect.width() * ppp) as u32,
            viewport_height: (rect.height() * ppp) as u32,
        };

        let cb = egui_wgpu::Callback::new_paint_callback(rect, callback);
        ui.painter().add(cb);
    }
}

impl eframe::App for RsCamApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Upload pending GPU data
        if self.pending_upload {
            self.pending_upload = false;
            self.upload_gpu_data(frame);
        }

        self.events.clear();

        // Menu bar
        crate::ui::menu_bar::draw(ctx, &self.state, &mut self.events);

        // Left panel: project tree (reads state immutably)
        egui::SidePanel::left("project_tree")
            .default_width(220.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::ui::project_tree::draw(ui, &self.state, &mut self.events);
                });
            });

        // Right panel: properties (needs mutable state for inline editing)
        egui::SidePanel::right("properties")
            .default_width(280.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::ui::properties::draw(ui, &mut self.state, &mut self.events);
                });
            });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            crate::ui::status_bar::draw(ui, &self.state);
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

        // Process events after UI pass
        self.handle_events(ctx);
    }
}

fn configure_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();

    // Zed-inspired dark theme
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
