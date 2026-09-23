#![deny(clippy::indexing_slicing)]

mod export;
mod gpu_upload;
mod input;
#[cfg(feature = "mcp")]
pub(crate) mod mcp;
mod simulation;
mod viewport;

use crate::controller::AppController;
use crate::render::RenderResources;
use crate::render::camera::OrbitCamera;
use crate::state::Workspace;

/// What the unsaved-changes dialog is guarding — the action that runs once
/// the operator answers Save or Discard.
///
/// G-OPENGUARD (F1.12): the dialog used to guard exactly one action, so it
/// was a `bool` and every button ended in `ViewportCommand::Close`. File ›
/// Open / Ctrl+O discards the project just as thoroughly as quitting does
/// and asked nothing. One dialog, one set of three answers, two things it
/// can be guarding — rather than a second dialog that would drift from
/// this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnsavedGuard {
    /// Close the window.
    Quit,
    /// Open another project, replacing this one.
    OpenJob,
}

use crate::ui::components::{Notice, NoticeStack, Role};

impl UnsavedGuard {
    /// The verb the dialog's buttons end in.
    fn verb(self) -> &'static str {
        match self {
            Self::Quit => "Quit",
            Self::OpenJob => "Open\u{2026}",
        }
    }

    /// What the operator is about to lose the project to.
    fn consequence(self) -> &'static str {
        match self {
            Self::Quit => "Quitting will close this project.",
            Self::OpenJob => "Opening another project will replace this one.",
        }
    }
}

pub struct RsCamApp {
    controller: AppController,
    /// The egui context, held so [`RsCamApp::off_frame_pump`] can dispatch
    /// without an `eframe::Frame` or an `egui::Ui` — neither of which exists
    /// outside a paint. Cheap `Arc` handle; `Clone + Send + Sync`.
    ///
    /// `pump_dispatch` reads the field inside its `#[cfg(feature = "mcp")]`
    /// arm alone, so a build without the `mcp` feature never reads it.
    #[cfg(feature = "mcp")]
    egui_ctx: egui::Context,
    camera: OrbitCamera,
    /// Cached viewport rect for click detection.
    viewport_rect: egui::Rect,
    /// Flag: need to load checkpoint mesh for backward scrubbing on next frame.
    pending_checkpoint_load: bool,
    /// Frame counter for auto-screenshot mode (RS_CAM_SCREENSHOT env var).
    auto_screenshot_frame: Option<u32>,
    /// G-LV.1 repro lever (RS_CAM_MINIMIZE_AFTER_FRAMES env var): frames left
    /// before this window minimises itself. `None` = never, which is every
    /// normal session.
    minimize_after_frames: Option<u32>,
    /// Currently hovered BREP face (updated on mouse move in Toolpaths workspace).
    last_hover_face: Option<rs_cam_core::geometry::enriched_mesh::FaceGroupId>,
    /// The unsaved-changes confirmation dialog, and what it is guarding.
    /// `None` means it is not shown. See [`UnsavedGuard`].
    unsaved_guard: Option<UnsavedGuard>,
    /// Every upload-time overlay dial, as one comparable key. See the
    /// detector in [`RsCamApp::update`].
    last_overlay_upload_key: OverlayUploadKey,
    /// Track the active drill op's target selection (toolpath id + picked
    /// holes) so the viewport markers re-upload when it changes from any
    /// source (viewport pick, panel buttons, selection change).
    last_drill_marker_key: Option<(usize, Vec<[f64; 2]>)>,
    /// MCP request receiver (populated when `--mcp` is passed).
    #[cfg(feature = "mcp")]
    mcp_receiver: Option<std::sync::mpsc::Receiver<crate::mcp_bridge::McpRequest>>,
    /// A/M12 — read payloads this thread republishes each frame so the MCP
    /// server can answer status calls while the frame loop is stalled behind
    /// a generation. Shared with the server thread.
    #[cfg(feature = "mcp")]
    mcp_reads: crate::mcp_bridge::McpReadCache,
    /// Last time [`Self::mcp_reads`] was refreshed — the publish is rate
    /// limited so it stays off the per-frame hot path.
    #[cfg(feature = "mcp")]
    mcp_reads_published_at: Option<std::time::Instant>,
}

/// Ceiling for a workspace's left / right panel, in points (P6, 2026-09-08).
///
/// egui remembers a resizable panel's width across frames and across window
/// resizes, and does not shrink it when the window shrinks. A pair dragged
/// wide on a big monitor therefore survives into a 1400 x 900 capture and can
/// leave the 3D view nothing — `screenshot_gui` came back with no viewport at
/// all. This is the ceiling on each side; `panel::MIN_VIEWPORT_WIDTH` is the
/// floor under the view itself, and the two are deliberately separate: this
/// one bounds a user drag, that one bounds the arithmetic.
pub const SIDE_PANEL_MAX_WIDTH: f32 = 420.0;

/// The edge that a workspace side panel docks to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidePanelEdge {
    Left,
    Right,
}

/// The one wrapper for every workspace side panel (G-PANELFIT, 2026-09-23).
///
/// The panel width does not follow the content width. The old wrapper put a
/// bare `ScrollArea::vertical()` in the panel. Its x axis took the content
/// width, and the panel stored that width for the next frame. One wide row
/// thus grew the panel to `SIDE_PANEL_MAX_WIDTH`, and the panel clipped the
/// rest. Here `ScrollArea::both()` sizes the x axis from the panel, and the
/// content `Ui` gets the inner width as its minimum and maximum. Text that
/// wraps keeps its wrap. A row that is still too wide shows a horizontal
/// scroll bar, and the panel keeps its width.
///
/// Do not replace this with `Style::wrap_mode = Wrap` (reverted in
/// b21e3294): that setting breaks grid labels mid-word.
pub fn side_panel(
    ui: &mut egui::Ui,
    edge: SidePanelEdge,
    id: &'static str,
    default_width: f32,
    add: impl FnOnce(&mut egui::Ui),
) {
    let panel = match edge {
        SidePanelEdge::Left => egui::Panel::left(id),
        SidePanelEdge::Right => egui::Panel::right(id),
    };
    panel
        .default_size(default_width)
        .max_size(SIDE_PANEL_MAX_WIDTH)
        .resizable(true)
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let width = ui.available_width();
                    ui.set_min_width(width);
                    ui.set_max_width(width);
                    add(ui);
                });
        });
}

impl RsCamApp {
    /// `mcp_exit` shares the MCP server thread's lifetime with the event-loop
    /// host. It also carries the wakeup that survives a parked frame loop.
    /// `None` is the normal interactive GUI path.
    pub(crate) fn new(
        cc: &eframe::CreationContext<'_>,
        mcp_mode: bool,
        #[cfg(feature = "mcp")] mcp_exit: Option<&crate::mcp_lifecycle::McpExitSignal>,
    ) -> Self {
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

        // G-LV.1 repro lever. See `minimize_after_frames`.
        let minimize_after_frames = std::env::var("RS_CAM_MINIMIZE_AFTER_FRAMES")
            .ok()
            .and_then(|v| v.parse::<u32>().ok());

        let mut controller = AppController::new();

        // Set up MCP channel and spawn server thread if requested.
        #[cfg(feature = "mcp")]
        let mcp_reads = crate::mcp_bridge::McpReadCache::new();
        #[cfg(feature = "mcp")]
        let mcp_receiver = if mcp_mode {
            controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
            // W1/W5: the cell the MCP server thread reads plan progress
            // from. `generation_status` is answered off the frame loop, so a
            // controller field is unreachable there.
            controller.set_plan_beat(mcp_reads.plan_handle());

            let (tx, rx) = std::sync::mpsc::channel();
            let egui_ctx = cc.egui_ctx.clone();
            // A/M12: the server's second door onto the compute lane. Taken
            // here, on the GUI thread, because the backend lives on the
            // controller — but usable from any thread thereafter.
            let generation = controller.generation_control();
            let reads = mcp_reads.clone();

            if let Some(exit_signal) = mcp_exit {
                let thread_exit = exit_signal.clone();
                let spawn_result =
                    std::thread::Builder::new()
                        .name("mcp-server".into())
                        .spawn(move || {
                            // Created before any fallible setup. A return or an
                            // unwind crossing this outer server-thread closure
                            // reports failure unless `waiting()` completed
                            // successfully. Detached handler-task panics do not
                            // unwind this closure and are not covered here.
                            let mut completion =
                                crate::mcp_lifecycle::McpCompletionGuard::new(thread_exit.clone());
                            let rt = match tokio::runtime::Runtime::new() {
                                Ok(rt) => rt,
                                Err(e) => {
                                    tracing::error!("Failed to create tokio runtime for MCP: {e}");
                                    return;
                                }
                            };
                            let clean_eof = rt.block_on(async move {
                                let waker = thread_exit.waker();
                                let server = crate::mcp_server::EmbeddedCamServer::new(
                                    tx, egui_ctx, generation, reads,
                                )
                                .with_waker(waker);
                                let tool_router =
                                    crate::mcp_server::EmbeddedCamServer::into_tool_router();

                                use rmcp::ServiceExt as _;

                                let mut router = rmcp::handler::server::router::Router::new(server);
                                router.tool_router = tool_router;

                                tracing::info!("Starting embedded MCP server on stdio");
                                match router.serve(rmcp::transport::stdio()).await {
                                    Ok(service) => match service.waiting().await {
                                        Ok(rmcp::service::QuitReason::Closed) => true,
                                        Ok(reason) => {
                                            tracing::error!(
                                                "MCP service stopped without clean EOF: {reason:?}"
                                            );
                                            false
                                        }
                                        Err(e) => {
                                            tracing::error!("MCP service error: {e}");
                                            false
                                        }
                                    },
                                    Err(e) => {
                                        tracing::error!("MCP serve error: {e}");
                                        false
                                    }
                                }
                            });
                            if clean_eof {
                                completion.mark_clean();
                                tracing::info!("Embedded MCP server reached clean EOF");
                            } else {
                                tracing::error!("Embedded MCP server shut down after failure");
                            }
                        });
                if let Err(e) = spawn_result {
                    tracing::error!("Failed to spawn MCP server thread: {e}");
                    exit_signal.request_failure();
                }
            } else {
                tracing::error!("MCP mode started without an event-loop exit signal");
            }

            Some(rx)
        } else {
            None
        };
        #[cfg(not(feature = "mcp"))]
        let _ = mcp_mode;

        // Load job file if RS_CAM_JOB is set
        let mut loaded_a_job = false;
        if let Ok(job_path) = std::env::var("RS_CAM_JOB") {
            let path = std::path::Path::new(&job_path);
            match controller.open_job_from_path(path) {
                Ok(()) => {
                    loaded_a_job = true;
                    tracing::info!("Loaded job from {}", path.display());
                }
                Err(e) => tracing::error!("Failed to load job: {e}"),
            }
        }

        // Select a specific setup by index via RS_CAM_SETUP
        if let Ok(setup_str) = std::env::var("RS_CAM_SETUP")
            && let Ok(idx) = setup_str.parse::<usize>()
            && let Some(setup) = controller.state().session.list_setups().get(idx)
        {
            let setup_id = crate::state::job::SetupId(setup.id);
            controller.state_mut().selection = crate::state::selection::Selection::Setup(setup_id);
        }

        // Switch workspace if requested via env var.
        //
        // Through `switch_workspace`, not by assigning the field: a workspace
        // carries per-workspace overlay defaults (P6), and a bare assignment
        // would land the auto-screenshot in Simulation with the Toolpaths
        // overlay set — no simulated stock, no collisions.
        if let Ok(val) = std::env::var("RS_CAM_SCREENSHOT") {
            let target = match val.to_lowercase().as_str() {
                "setup" => Some(Workspace::Setup),
                "simulation" | "sim" => Some(Workspace::Simulation),
                _ => None,
            };
            if let Some(target) = target {
                crate::ui::overlays::registry::switch_workspace(controller.state_mut(), target);
            }
        }

        let last_overlay_upload_key = overlay_upload_key(controller.state());

        let mut app = Self {
            controller,
            #[cfg(feature = "mcp")]
            egui_ctx: cc.egui_ctx.clone(),
            camera: OrbitCamera::new(),
            viewport_rect: egui::Rect::NOTHING,
            pending_checkpoint_load: false,
            auto_screenshot_frame,
            minimize_after_frames,
            last_hover_face: None,
            unsaved_guard: None,
            last_overlay_upload_key,
            last_drill_marker_key: None,
            #[cfg(feature = "mcp")]
            mcp_receiver,
            #[cfg(feature = "mcp")]
            mcp_reads,
            #[cfg(feature = "mcp")]
            mcp_reads_published_at: None,
        };

        // G-WSMENU (2026-09-10): the third `open_job_from_path` route. The
        // camera does not exist while the job is loading above, so the fit
        // happens here instead — the alternative is an auto-screenshot run
        // framed on an empty default scene. Same routine as the other two
        // routes and as Reset View.
        if loaded_a_job {
            app.fit_camera_to_first_model();
        }
        app
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
            .session
            .models()
            .iter()
            .find_map(|model| {
                model.mesh.as_ref().map(|m| m.bbox).or_else(|| {
                    model
                        .polygons
                        .as_deref()
                        .and_then(|polys| rs_cam_core::session::polygons_bbox(polys))
                })
            })
        {
            self.fit_camera_to_bbox(&bbox);
        } else {
            self.camera = OrbitCamera::new();
        }
    }

    // --- Layout methods ---

    fn draw_setup_layout(&mut self, ui: &mut egui::Ui) {
        // Left panel: setup list with summary cards
        side_panel(ui, SidePanelEdge::Left, "setup_tree", 240.0, |ui| {
            let (state, events) = self.controller.state_ref_and_events_mut();
            crate::ui::setup_panel::draw(ui, state, events);
        });

        // Right panel: setup properties
        side_panel(ui, SidePanelEdge::Right, "setup_properties", 280.0, |ui| {
            let (state, events) = self.controller.state_and_events_mut();
            crate::ui::properties::draw(ui, state, events);
        });

        let col_count = self
            .controller
            .state()
            .simulation
            .checks
            .total_collision_count();
        let lane_snapshots = self.controller.lane_snapshots();
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            // DC7: the bar carries the load-warnings count and reports the
            // click. The controller owns the list, so the open decision is
            // taken here.
            let open_warnings = crate::ui::status_bar::draw(
                ui,
                self.controller.state(),
                col_count,
                &lane_snapshots,
                self.controller.load_warnings(),
            );
            if open_warnings {
                self.controller.set_show_load_warnings(true);
            }
            if let Some(msg) = self.controller.status_message() {
                ui.separator();
                ui.label(egui::RichText::new(msg).color(crate::ui::tokens::CAUTION));
            }
        });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(crate::ui::tokens::SURFACE_SUNKEN)
                    .inner_margin(0.0),
            )
            .show(ui, |ui| {
                self.draw_viewport(ui);
            });
    }

    fn draw_readiness_layout(&mut self, ui: &mut egui::Ui) {
        // Job-readiness dashboard (W3.8): a focused, centred "is this safe to
        // cut?" page — no side panels or viewport, just the consolidated
        // verdict. Status bar stays for consistency with the other workspaces.
        let col_count = self
            .controller
            .state()
            .simulation
            .checks
            .total_collision_count();
        let lane_snapshots = self.controller.lane_snapshots();
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            // DC7: the bar carries the load-warnings count and reports the
            // click. The controller owns the list, so the open decision is
            // taken here.
            let open_warnings = crate::ui::status_bar::draw(
                ui,
                self.controller.state(),
                col_count,
                &lane_snapshots,
                self.controller.load_warnings(),
            );
            if open_warnings {
                self.controller.set_show_load_warnings(true);
            }
            if let Some(msg) = self.controller.status_message() {
                ui.separator();
                ui.label(egui::RichText::new(msg).color(crate::ui::tokens::CAUTION));
            }
        });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(crate::ui::tokens::SURFACE_SUNKEN)
                    .inner_margin(16.0),
            )
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_max_width(560.0);
                    let (state, events) = self.controller.state_ref_and_events_mut();
                    crate::ui::readiness_panel::draw(ui, state, events);
                });
            });
    }

    fn draw_toolpath_layout(&mut self, ui: &mut egui::Ui) {
        // W3 - the operation panel needs three facts `AppState` does not
        // hold: where the generation plan has got to, whether the analysis
        // lane is simulating, and the question a plan waits on. They are
        // read HERE, before the panel borrows the controller, and they are
        // NOT mirrored onto `AppState`: a mirrored copy is a second store.
        let lane_snapshots = self.controller.lane_snapshots();
        let panel_ctx = crate::ui::toolpath_panel::PanelContext {
            // The analysis lane also runs collision checks, and a collision
            // check carves no stock. The lane's own job label is what the
            // status bar tells them apart by.
            analysis_simulating: lane_snapshots
                .iter()
                .find(|lane| lane.lane == crate::compute::ComputeLane::Analysis)
                .is_some_and(|lane| {
                    lane.is_active()
                        && lane
                            .current_job
                            .as_deref()
                            .is_some_and(|job| job.starts_with("Simulation"))
                }),
            plan: self.controller.generation_plan_progress(),
            pending_confirm: self
                .controller
                .pending_plan_confirm()
                .map(|confirm| confirm.message.clone()),
        };

        // Left panel: operation queue
        side_panel(ui, SidePanelEdge::Left, "toolpath_tree", 240.0, |ui| {
            let (state, events) = self.controller.state_and_events_mut();
            crate::ui::toolpath_panel::draw(ui, state, &panel_ctx, events);
        });

        // Right panel: operation/tool parameters
        side_panel(
            ui,
            SidePanelEdge::Right,
            "toolpath_properties",
            280.0,
            |ui| {
                let (state, events) = self.controller.state_and_events_mut();
                crate::ui::properties::draw(ui, state, events);
            },
        );

        let col_count = self
            .controller
            .state()
            .simulation
            .checks
            .total_collision_count();
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            // DC7: the bar carries the load-warnings count and reports the
            // click. The controller owns the list, so the open decision is
            // taken here.
            let open_warnings = crate::ui::status_bar::draw(
                ui,
                self.controller.state(),
                col_count,
                &lane_snapshots,
                self.controller.load_warnings(),
            );
            if open_warnings {
                self.controller.set_show_load_warnings(true);
            }
            if let Some(msg) = self.controller.status_message() {
                ui.separator();
                ui.label(egui::RichText::new(msg).color(crate::ui::tokens::CAUTION));
            }
        });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(crate::ui::tokens::SURFACE_SUNKEN)
                    .inner_margin(0.0),
            )
            .show(ui, |ui| {
                self.draw_viewport(ui);
            });
    }

    fn draw_simulation_layout(&mut self, ui: &mut egui::Ui) {
        // (The old `sim_analysis_bar` top strip — Debug / Cut Metrics /
        // Highlight / Record trace — was consolidated into the right-panel
        // Inspector's "View" section. One place for all display settings.)

        // Bottom panel: transport, verdict pills and the boundary timeline,
        // plus the time-series drawer when the Inspector opens it (PLAN
        // §3.3). The two states use two panel ids: egui stores one size per
        // id and clamps it to the current range, so one shared id would
        // forget the open height while the drawer is closed.
        //
        // Open: `max_size` is required because the drawer's ScrollArea
        // uses `auto_shrink([false, false])` — without a panel cap the two
        // form a feedback loop where the panel sizes to the ScrollArea's
        // requested max height, the ScrollArea then sees more space and
        // requests more, and so on until the panel takes the whole window.
        //
        // Closed: the panel is not resizable and follows its content, the
        // transport bar and the timeline.
        let time_series_open = self.controller.state().simulation.time_series_open;
        let bottom_panel = if time_series_open {
            egui::Panel::bottom("sim_timeline")
                .min_size(60.0)
                .max_size(480.0)
                .resizable(true)
                .default_size(360.0)
        } else {
            egui::Panel::bottom("sim_transport")
                .min_size(60.0)
                .max_size(240.0)
                .resizable(false)
                .default_size(120.0)
        };
        bottom_panel.show(ui, |ui| {
            let (state, events) = self.controller.state_and_events_mut();
            crate::ui::sim_timeline::draw(
                ui,
                &mut state.simulation,
                &state.session,
                &state.gui,
                events,
            );
        });

        // Left panel: operation list
        side_panel(ui, SidePanelEdge::Left, "sim_op_list", 240.0, |ui| {
            let (state, events) = self.controller.state_and_events_mut();
            crate::ui::sim_op_list::draw(
                ui,
                &mut state.simulation,
                &state.session,
                &state.gui,
                &mut state.viewport,
                events,
            );
        });

        // Right panel: diagnostics
        side_panel(ui, SidePanelEdge::Right, "sim_diagnostics", 240.0, |ui| {
            let (state, events) = self.controller.state_and_events_mut();
            crate::ui::sim_diagnostics::draw(
                ui,
                &mut state.simulation,
                &state.session,
                &state.gui,
                events,
            );
        });

        // Central panel: 3D viewport
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(crate::ui::tokens::SURFACE_SUNKEN)
                    .inner_margin(0.0),
            )
            .show(ui, |ui| {
                self.draw_viewport(ui);
            });

        let playback = &self.controller.state().simulation.playback;
        if playback.playing || playback.display_mesh_preview {
            egui::Area::new(egui::Id::new("sim_preview_quality_notice"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 54.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::default()
                        .fill(egui::Color32::from_rgba_unmultiplied(
                            crate::ui::tokens::TINT_CAUTION.r(),
                            crate::ui::tokens::TINT_CAUTION.g(),
                            crate::ui::tokens::TINT_CAUTION.b(),
                            220,
                        ))
                        .corner_radius(5)
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(
                                    "Mesh quality reduced for playback — pause for full render",
                                )
                                .small()
                                .strong()
                                .color(crate::ui::tokens::CAUTION),
                            );
                        });
                });
        }
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

impl RsCamApp {
    /// The frame-independent half of a frame: everything in [`Self::draw_frame`]
    /// that needs neither an `eframe::Frame` nor an `egui::Ui`.
    ///
    /// A field census of the whole 6,146-line `app/mcp.rs` is the reason this
    /// is a short function rather than a rewrite: MCP handlers touch
    /// `self.controller` (169 sites), `mcp_reads`, `mcp_receiver` and
    /// `mcp_reads_published_at`, and **nothing else** — not `camera`, not
    /// `viewport_rect`, not `pending_checkpoint_load`. `egui::Context` reaches
    /// exactly one handler (`ScreenshotGui`) and is `Clone + Send + Sync`. So
    /// no handler had to move and no `McpRequestKind` arm had to change.
    ///
    /// Called from **two** places, in the same order both times: here, from
    /// the host's `about_to_wait`, and from `draw_frame` at the position the
    /// three calls it replaced already occupied. It is idempotent and near
    /// free on an empty channel, so the non-MCP path does not regress.
    fn pump_dispatch(&mut self) {
        self.controller.drain_compute_results();

        // `egui::Context` is a cheap `Arc` handle; the clone is to release the
        // borrow of `self` before the `&mut self` call, not a copy of anything.
        #[cfg(feature = "mcp")]
        {
            let ctx = self.egui_ctx.clone();
            self.drain_mcp_requests(&ctx);
            // Arms an in-flight `screenshot_gui`; issuing the capture still
            // needs a real frame, which is the one honest boundary (§6 of
            // `DISPATCH_DECOUPLING_DESIGN.md`).
            self.pump_mcp_gui_screenshot(&ctx);
        }
    }

    /// Dispatch MCP work and advance deferred compute **without a paint**.
    ///
    /// This is the answer to G-LV.1. On Wayland a hidden, occluded or
    /// screen-locked surface never gets a compositor frame callback, so winit
    /// never emits `RedrawRequested` and no frame ever runs — but
    /// `Event::AboutToWait` is dispatched unconditionally on every event-loop
    /// iteration (`winit-0.30.13 .../wayland/event_loop/mod.rs:515`, with no
    /// reference to frame-callback state). Everything below was frame-coupled
    /// by where the drain was written, not by the platform.
    ///
    /// Does NOT open the frame bracket and does NOT count as a frame — see
    /// [`crate::mcp_bridge::FrameLoopBeat::pump_beat`].
    pub(crate) fn off_frame_pump(&mut self) {
        self.pump_dispatch();
        self.controller.process_auto_regen();
        // P5 — keep the reach-map overlay pointed at the current selection.
        // Beside `process_auto_regen` on both pump paths, because a selection
        // made while the surface is parked must still resolve a request.
        self.controller.process_reach_overlay();
        #[cfg(feature = "mcp")]
        self.mcp_pump_beat();
    }

    /// Whether the event loop must keep waking on a timer rather than
    /// sleeping until the next external event.
    ///
    /// **This is the safety net, not the mechanism.** MCP *enqueues* wake the
    /// loop through the `GuiWaker`, so no read waits on this. What needs it is
    /// work that completes with nobody to announce it: a compute result
    /// arrives on an `mpsc` channel from a worker thread that requests no
    /// repaint and holds no proxy, and the only thing that ever noticed was
    /// the next frame's poll (`app.rs`'s `active_lanes` re-request) — which is
    /// precisely the polling loop a park stops. `generate_all`'s fixpoint
    /// round handoff lives on that path, and it is what the 2026-08-07
    /// incident actually died on.
    ///
    /// Armed only while something is genuinely outstanding, so an idle session
    /// still sleeps.
    pub(crate) fn needs_pump_tick(&self) -> bool {
        if self.controller.awaiting_deferred_completions() > 0 {
            return true;
        }
        self.controller
            .lane_snapshots()
            .into_iter()
            .any(|lane| lane.is_active() || lane.queue_depth > 0)
    }
}

impl eframe::App for RsCamApp {
    // eframe 0.34 made `ui` the required entry point (the old `update(ctx)`
    // is deprecated). We draw everything via panels nested in this root
    // `ui` with `show`, so the whole frame lives in `draw_frame`.
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.draw_frame(&ctx, ui, frame);
    }
}

impl RsCamApp {
    fn draw_frame(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // Intercept OS close button when there are unsaved changes
        let os_close_requested = ctx.input(|i| i.viewport().close_requested());
        if os_close_requested && self.controller.state().gui.dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.unsaved_guard = Some(UnsavedGuard::Quit);
        }

        // Handle screenshot results from previous frame. An in-flight MCP
        // screenshot_gui request consumes the event (writes to its own
        // path + completes the deferred response); otherwise fall back to
        // the F12 save-to-cwd path.
        ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    #[cfg(feature = "mcp")]
                    if self.complete_mcp_gui_screenshot(image) {
                        continue;
                    }
                    Self::save_screenshot(image);
                }
            }
        });

        // F12: request screenshot
        if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }

        crate::ui::automation::begin_frame(ctx);

        // G-LV.1: open the frame bracket, then run the same dispatch the
        // off-frame pump runs — `drain_compute_results`, `drain_mcp_requests`,
        // and the `screenshot_gui` pump, in that order, at the position those
        // three calls already occupied. In-frame ordering is unchanged;
        // `process_auto_regen` and `end_mcp_frame` stay where they are, below,
        // because moving them would reorder work relative to the UI pass.
        #[cfg(feature = "mcp")]
        self.begin_mcp_frame();
        self.pump_dispatch();

        // Request repaint while MCP highlights are fading or notifications are active.
        #[cfg(feature = "mcp")]
        if !self.controller.state().gui.mcp_highlights.is_empty()
            || self.controller.active_notifications().next().is_some()
        {
            ctx.request_repaint();
        }

        // MCP heartbeat: while the MCP server is wired, keep a low-frequency
        // repaint scheduled so the request channel is drained within ~100 ms.
        // `drain_mcp_requests` only runs inside `update()`, and `update()` only
        // runs on a repaint — but the cross-thread `request_repaint()` from the
        // server thread (`McpServer::send_request`) does not reliably wake a
        // sleeping winit loop. Without this heartbeat, a request issued while
        // the GUI is idle (notably the one right after a long `run_simulation`,
        // once highlights/notifications have faded) stalls in the channel until
        // an OS event or an operator `/mcp` reconnect wakes the loop.
        #[cfg(feature = "mcp")]
        if self.mcp_receiver.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        // Re-upload when any UPLOAD-TIME overlay dial changes.
        //
        // P6 replaced two detectors with one. An upload-time flag is
        // consumed in `app/gpu_upload.rs`, which runs only when
        // `take_pending_upload()` fires, so such a flag with no trigger is a
        // control that does nothing until an unrelated event happens to fire
        // an upload. Two of the three dials had a detector each; the third —
        // the tool-profile ghost — never got one, and was a dead control for
        // its whole life (audit §3.6, fix §6.1). P6 also made five more
        // dials upload-time when it split the fixture buffer, so one
        // composite key is the shape that cannot leave a new dial behind.
        //
        // The key carries the WORKSPACE as well, because
        // `UiCommand::SwitchWorkspace` sets no pending upload of its own and
        // the fixture buffer's contents depend on the workspace's overlay
        // defaults (audit §3.3).
        let current_overlay_key = overlay_upload_key(self.controller.state());
        if current_overlay_key != self.last_overlay_upload_key {
            self.last_overlay_upload_key = current_overlay_key;
            self.controller.set_pending_upload();
        }
        // Re-upload drill target markers when the active drill op's selection
        // changes (panel buttons, layer select, viewport pick, or selecting a
        // different drill toolpath).
        let current_drill_key = self.current_drill_marker_key();
        if current_drill_key != self.last_drill_marker_key {
            self.last_drill_marker_key = current_drill_key;
            self.controller.set_pending_upload();
        }

        // WP6: a properties panel applies its edit through one command
        // and cannot reach the controller. It raises a flag instead; the
        // work runs here, beside the two detectors above and before the
        // upload the first flag asks for.
        self.controller.discharge_panel_side_effects();

        if self.controller.take_pending_upload() {
            self.upload_gpu_data(frame);
        }

        // Handle keyboard shortcuts (before UI to prevent conflicts)
        match self.controller.state().workspace {
            Workspace::Setup | Workspace::Toolpaths | Workspace::Readiness => {
                self.handle_keyboard_shortcuts(ctx);
            }
            Workspace::Simulation => self.handle_simulation_shortcuts(ctx),
        }

        // Menu bar (shown in all workspaces)
        {
            let (state, events) = self.controller.state_ref_and_events_mut();
            crate::ui::menu_bar::draw(ui, state, events);
        }

        // Workspace switcher bar (shown in all workspaces)
        egui::Panel::top("workspace_bar")
            .frame(
                egui::Frame::default()
                    .fill(crate::ui::tokens::SURFACE_RAISED)
                    .inner_margin(egui::Margin::symmetric(8, 2)),
            )
            .show(ui, |ui| {
                let (state, events) = self.controller.state_ref_and_events_mut();
                crate::ui::workspace_bar::draw(ui, state, events);
            });

        // Draw workspace-specific layout.
        //
        // WP24, operator ruling 2026-09-13 (§30 item 3): the full-screen
        // Optimize placeholder is DELETED. It was a POLICY and never a
        // necessity. Before WP14b the lane OWNED the session — a
        // `mem::replace` pulled it into the worker request — so the panels
        // would have drawn against an empty one. Every route now runs over
        // a clone, and the operator keeps the whole GUI during a run.
        //
        // What tells the operator a run is in flight is the progress row in
        // `ui::workspace_bar`, which also cancels it. One run at a time
        // stays the policy (§28 ruling 8), and the three submit sites now
        // refuse with a toast rather than a log line.
        match self.controller.state().workspace {
            Workspace::Setup => self.draw_setup_layout(ui),
            Workspace::Toolpaths => self.draw_toolpath_layout(ui),
            Workspace::Simulation => self.draw_simulation_layout(ui),
            Workspace::Readiness => self.draw_readiness_layout(ui),
        }

        // Pre-flight checklist modal (shown on top of either layout)
        if self.controller.state().show_preflight {
            let (state, events) = self.controller.state_ref_and_events_mut();
            if !crate::ui::preflight::draw(ctx, state, events) {
                self.controller.state_mut().show_preflight = false;
            }
        }

        // Export wizard (Phase 5)
        if self.controller.state().show_export_wizard {
            let (state, events) = self.controller.state_ref_and_events_mut();
            crate::ui::export_wizard::draw(ctx, state, events);
        }

        // Optimize modal (per-toolpath)
        if self.controller.state().optimize_modal.is_some() {
            let (state, events) = self.controller.state_ref_and_events_mut();
            crate::ui::optimize_modal::draw(ctx, state, events);
        }

        // Optimize-project rollup
        if self.controller.state().optimize_project.is_some() {
            let (state, events) = self.controller.state_ref_and_events_mut();
            crate::ui::optimize_project::draw(ctx, state, events);
        }

        // Multi-tool finishing planner (Phase U). Takes `&mut AppState`
        // because it edits its own dials in place — the same shape
        // `viewport_overlay` uses for the viewport toggles. Only the four
        // ACTIONS (open / preview / apply / close) go through events, and
        // nothing it does touches the project.
        {
            let (state, events) = self.controller.state_and_events_mut();
            crate::ui::multitool_planner::draw(ctx, state, events);
        }

        // Feeds & Speeds modal (redesigned Feeds tab)
        if self.controller.state().feeds_modal.is_some() {
            let (state, events) = self.controller.state_ref_and_events_mut();
            crate::ui::feeds::draw(ctx, state, events);
        }

        // Tool Library management modal
        if self.controller.state().tool_library_modal.is_some() {
            let (state, events) = self.controller.state_ref_and_events_mut();
            crate::ui::tool_library_modal::draw(ctx, state, events);
        }

        // Both the setup inspector and setup rail request this one persistent
        // confirmation; only its confirmed event can remove a setup.
        {
            let (state, events) = self.controller.state_ref_and_events_mut();
            crate::ui::setup_deletion_modal::draw(ctx, state, events);
        }

        // The generation plan's one question (R1). Its state lives on the
        // controller, not on `AppState`: the plan is the controller's, and
        // the answer starts it.
        if let Some(confirm) = self.controller.pending_plan_confirm().cloned() {
            use crate::ui::generation_resolution_modal::PlanResolutionChoice;
            match crate::ui::generation_resolution_modal::draw(ctx, &confirm) {
                Some(PlanResolutionChoice::UseRequired) => {
                    self.controller.accept_plan_resolution();
                }
                Some(PlanResolutionChoice::Cancel) => {
                    self.controller.cancel_plan_resolution();
                }
                None => {}
            }
        }

        // Machine Library management modal
        if self.controller.state().machine_library_open {
            let (state, events) = self.controller.state_ref_and_events_mut();
            crate::ui::machine_library_modal::draw(ctx, state, events);
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

        // Load checkpoint mesh for backward scrubbing. During active pointer
        // scrubbing, defer this potentially large clone/remesh/upload until
        // release; `update_live_sim()` will catch the live stock up then.
        if self.pending_checkpoint_load
            && !self
                .controller
                .state()
                .simulation
                .playback
                .scrub_drag_active
        {
            self.pending_checkpoint_load = false;
            let move_idx = self.controller.state().simulation.playback.current_move;
            self.load_checkpoint_for_move(move_idx, frame);
        }

        // The load-warnings modal (DC7, defect F-4, ruling R31).
        //
        // This was a NON-MODAL window titled "Project Load Warnings". The
        // operator could leave it open over the workspace tab bar, where it
        // covered Setup, Toolpaths and Simulation, so navigation stopped. A
        // modal cannot be left anywhere: it is a thing you open, read and
        // close.
        //
        // The status bar carries the count and opens this, so the warnings
        // stay reachable after a close. The window could be dismissed once
        // and never reopened.
        if self.controller.show_load_warnings() {
            // The content box. It is centred and it never reaches the tab
            // bar at the top of the window.
            const MODAL_WIDTH: f32 = 420.0;
            const MODAL_MAX_LIST_HEIGHT: f32 = 280.0;

            // `NoticeStack` is the bounded renderer §4.11 was written for,
            // and this is one of its six consumers. A raw loop over a project
            // with 300 warnings drew 300 rows.
            let notices: Vec<Notice> = self
                .controller
                .load_warnings()
                .iter()
                .map(|w| Notice::new(Role::Caution, w.clone()))
                .collect();
            let expand_id = egui::Id::new("load_warnings_expanded");
            let expanded = ctx.data(|d| d.get_temp::<bool>(expand_id).unwrap_or(false));
            // The title is the same count the status bar states, so the two
            // surfaces cannot disagree about how many there are.
            let title = crate::ui::status_bar::warnings_label(notices.len());

            let modal = egui::Modal::new(egui::Id::new("load_warnings_modal"))
                .backdrop_color(crate::ui::tokens::SCRIM)
                .show(ctx, |ui| {
                    ui.set_max_width(MODAL_WIDTH);
                    let title = title.as_str();
                    let response = ui.label(crate::ui::components::text::display(title));
                    crate::ui::automation::record(ui, "project_load_warnings", &response, title);
                    ui.add_space(crate::ui::tokens::SPACE_3);
                    // Rule F: the container scrolls rather than clips.
                    let expand_clicked = egui::ScrollArea::vertical()
                        .max_height(MODAL_MAX_LIST_HEIGHT)
                        .show(ui, |ui| {
                            NoticeStack::new(notices).expanded(expanded).show(ui)
                        })
                        .inner;
                    ui.add_space(crate::ui::tokens::SPACE_3);
                    let close_clicked = ui
                        .add(crate::ui::components::Button::primary("Close"))
                        .clicked();
                    (expand_clicked, close_clicked)
                });

            let (expand_clicked, close_clicked) = modal.inner;
            if expand_clicked {
                ctx.data_mut(|d| d.insert_temp(expand_id, true));
            }
            if close_clicked || modal.should_close() {
                self.controller.set_show_load_warnings(false);
                ctx.data_mut(|d| d.insert_temp(expand_id, false));
            }
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
                    .order(egui::Order::Foreground)
                    .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -12.0))
                    .show(ctx, |ui| {
                        ui.set_max_width(400.0);
                        // Q1, operator ruling 2026-09-14: the visible toast
                        // stack caps at four. This loop used to draw EVERY
                        // active notification, so a burst covered the corner
                        // of the product it floats over.
                        //
                        // Nothing about TTLs, severities or the
                        // `get_notifications` wire changes — this is a
                        // RENDERING bound, and `NoticeStack` applies §4.11's
                        // rules, so an ERROR toast still renders however many
                        // notices are queued ahead of it.
                        let notices: Vec<Notice> = notifications
                            .iter()
                            .map(|(message, severity)| {
                                let role = match severity {
                                    crate::controller::Severity::Info => Role::Info,
                                    crate::controller::Severity::Warning => Role::Caution,
                                    crate::controller::Severity::Error => Role::Danger,
                                };
                                Notice::new(role, message.clone())
                            })
                            .collect();
                        let _ = NoticeStack::new(notices).show(ui);
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

        if self.controller.state().workspace == Workspace::Simulation
            && self.controller.state().simulation.has_results()
        {
            // Update after UI/event handling and playback advancement so the
            // rendered tool matches the same move as the stock preview/full mesh.
            self.update_sim_tool_position(frame);
            // Incremental stock simulation: update live heightmap to match current_move
            self.update_live_sim(frame);
        }

        self.controller.process_auto_regen();
        self.controller.process_reach_overlay();

        // G-LV.1: anything an MCP caller is still awaiting needs a FUTURE
        // frame to reach it — the compute drain, `generate_all`'s fixpoint
        // round handoff (`settle_generate_all_round` ->
        // `resume_generate_all_after_simulation`, both reached from
        // `drain_compute_results`), and the screenshot pump all live in this
        // function. The lane-activity check below covers the window where a
        // job is running; this covers the gap it cannot see — the round
        // boundary, where the lane has gone idle, the result is still in the
        // channel, and nothing has yet asked for the frame that would pick it
        // up. Stated once here rather than at each handoff site: a per-site
        // request is one refactor away from being forgotten, and this
        // condition is exactly "someone is still owed something". Phase O made
        // it ungated: a GUI-started `generate_all` ladder needs those same
        // frames and has no MCP slot at all.
        if self.controller.awaiting_deferred_completions() > 0 {
            ctx.request_repaint();
        }

        let active_lanes = self
            .controller
            .lane_snapshots()
            .into_iter()
            .any(|lane| lane.is_active() || lane.queue_depth > 0);
        if active_lanes || self.controller.state().simulation.playback.playing {
            ctx.request_repaint();
        }

        // G-LV.1 repro lever: minimise this window after N frames.
        //
        // This is the only way to park a Wayland surface from inside the
        // client, and therefore the only way to reproduce the 2026-08-07
        // incident without hiding or locking the operator's real desktop.
        // `Window::set_visible(false)` is a documented no-op on Wayland
        // (`winit-0.30.13 .../wayland/window/mod.rs:253-255`, and
        // `is_visible()` returns `None` at `:258-260` — which is also why
        // eframe's own invisible-window rescue never fires there).
        // `set_minimized(true)` does work (`:436-444`) and is ONE-WAY: winit
        // refuses to un-minimise on Wayland. That asymmetry is the incident,
        // not a limitation of the lever — it is why the ~9.5 h park that
        // blocked waves A-1 and A-2 had no observed unpark trigger.
        //
        // Unset in every normal session. Never wire this to a UI affordance.
        if let Some(remaining) = self.minimize_after_frames.as_mut() {
            if *remaining == 0 {
                self.minimize_after_frames = None;
                tracing::warn!(
                    "RS_CAM_MINIMIZE_AFTER_FRAMES: minimising the window now (G-LV.1 repro). On \
                     Wayland this is irreversible from inside the process."
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            } else {
                *remaining = remaining.saturating_sub(1);
                ctx.request_repaint();
            }
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

        // G-LV.1: last statement in the frame. Closes the bracket
        // `drain_mcp_requests` opened, so `frame_loop.in_frame` covers the
        // whole body — a frame that spends 30 s inside a narration or a mesh
        // upload reports as BUSY, not as the parked window G-LV.1 is about.
        // Keep this last: anything after it runs outside the bracket and
        // would be misattributed.
        #[cfg(feature = "mcp")]
        self.end_mcp_frame();
    }
}

fn configure_theme(ctx: &egui::Context) {
    // The whole token set, both themes. `ui::tokens::apply` uses
    // `all_styles_mut`; the `set_visuals` / `set_global_style` pair this
    // replaced wrote one theme only (`DESIGN_SPEC.md` §10.3).
    crate::ui::tokens::apply(ctx);
    crate::ui::tokens::apply_fonts(ctx);
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

/// Every upload-time overlay dial, in one comparable value — plus the two
/// inputs of the WP27 draw rule, which is consumed in the same pass.
///
/// The `PartialEq` derive is the whole mechanism: the frame loop compares
/// this frame's key with the last one and fires exactly one
/// `set_pending_upload()` when anything in it moved. Adding a dial to
/// `app/gpu_upload.rs` means adding a field here — a registry row that names
/// `OverlayMechanism::UploadTime` and is missing from this key is the defect
/// class the tool-profile ghost shipped with (audit §3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OverlayUploadKey {
    tool_profile: bool,
    fixtures: bool,
    keep_outs: bool,
    alignment_pins: bool,
    flip_axis: bool,
    datum: bool,
    span_filter: crate::state::viewport::SpanKindFilter,
    toolpath_color_mode: crate::state::viewport::ToolpathColorMode,
    stock_viz_mode: crate::state::simulation::StockVizMode,
    /// The fixture buffer's contents depend on the active setup, which the
    /// workspace can change, and `SwitchWorkspace` fires no upload itself.
    workspace: Workspace,
    /// WP27 — the viewport draws the selected toolpath only unless this is
    /// set. The Overlays row and the viewport-bar button both write it.
    show_all_toolpaths: bool,
    /// WP27 — the draw SET now depends on the selection, and 39 of the 40
    /// production selection writers fire no upload of their own. Same reason
    /// as the `workspace` field above: one field covers every writer, and no
    /// call site changes.
    ///
    /// `Selection` is not `Copy`, so the DERIVED toolpath id is stored here
    /// and never the enum.
    selected_toolpath: Option<crate::state::toolpath::ToolpathId>,
    /// Source vectors fall back to Simulation's current boundary when no
    /// operation is selected, so scrubbing must invalidate their upload.
    vector_source_toolpath: Option<crate::state::toolpath::ToolpathId>,
}

pub(crate) fn overlay_upload_key(state: &crate::state::AppState) -> OverlayUploadKey {
    OverlayUploadKey {
        tool_profile: state.viewport.show_tool_profile_preview,
        fixtures: state.viewport.show_fixtures,
        keep_outs: state.viewport.show_keep_outs,
        alignment_pins: state.viewport.show_alignment_pins,
        flip_axis: state.viewport.show_flip_axis,
        datum: state.viewport.show_datum,
        span_filter: state.viewport.span_kind_filter,
        toolpath_color_mode: state.viewport.toolpath_color_mode,
        stock_viz_mode: state.simulation.stock_viz_mode,
        workspace: state.workspace,
        show_all_toolpaths: state.viewport.show_all_toolpaths,
        selected_toolpath: match state.selection {
            crate::state::selection::Selection::Toolpath(id) => Some(id),
            _ => None,
        },
        vector_source_toolpath: crate::state::viewport::vector_source_toolpath(state),
    }
}
