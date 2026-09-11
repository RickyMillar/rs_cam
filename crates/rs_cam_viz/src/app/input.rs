use crate::state::Workspace;
use crate::state::selection::Selection;
use crate::ui::AppEvent;
use crate::ui_command::{
    NoArgs, SetToolLoadOverrideArgs, SimJumpToMoveArgs, SimJumpToOpStartArgs, UiCommand,
};

use super::RsCamApp;

impl RsCamApp {
    pub(super) fn handle_events(&mut self, ctx: &egui::Context) {
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
                AppEvent::ExportGcodeConfirmed => {
                    self.export_gcode_with_summary();
                }
                AppEvent::WizardSetStep(step) => {
                    let clamped = step.min(crate::ui::export_wizard::STEP_COUNT - 1);
                    let s = self.controller.state_mut();
                    s.wizard_active_step = clamped;
                    if clamped > s.gui.wizard.last_step_visited {
                        s.gui.wizard.last_step_visited = clamped;
                    }
                }
                AppEvent::WizardSetWcsOverride(wcs) => {
                    let s = self.controller.state_mut();
                    s.gui.wizard.wcs_override = wcs;
                    s.gui.mark_edited();
                }
                AppEvent::WizardSetUnitsOverride(units) => {
                    let s = self.controller.state_mut();
                    s.gui.wizard.units_override = units;
                    s.gui.mark_edited();
                }
                AppEvent::WizardSetSafeZOverride(safe_z) => {
                    let s = self.controller.state_mut();
                    s.gui.wizard.safe_z_override = safe_z;
                    s.gui.mark_edited();
                }
                AppEvent::WizardSetDryRun(dry_run) => {
                    let s = self.controller.state_mut();
                    s.gui.wizard.dry_run = dry_run;
                    s.gui.mark_edited();
                }
                AppEvent::WizardSetSpindleWarmup(secs) => {
                    let s = self.controller.state_mut();
                    s.gui.wizard.spindle_warmup_secs = secs;
                    s.gui.mark_edited();
                }
                AppEvent::WizardSetToolChangeMode(mode) => {
                    let s = self.controller.state_mut();
                    s.gui.wizard.tool_change_override = mode;
                    s.gui.mark_edited();
                }
                AppEvent::WizardSetSetupPauseMessage { setup_id, message } => {
                    self.controller.set_setup_pause_message(setup_id, message);
                }
                AppEvent::WizardSetAllowValidatorErrors(allow) => {
                    let s = self.controller.state_mut();
                    s.gui.wizard.allow_validator_errors = allow;
                    s.gui.mark_edited();
                }
                AppEvent::WizardSave => {
                    self.handle_wizard_save();
                }
                AppEvent::WizardSetOutputLayout(layout) => {
                    let s = self.controller.state_mut();
                    s.gui.wizard.output_layout = layout;
                    s.gui.mark_edited();
                }
                AppEvent::WizardSetFilenameTemplate(template) => {
                    let s = self.controller.state_mut();
                    s.gui.wizard.filename_template = template;
                    s.gui.mark_edited();
                }
                AppEvent::WizardSetPost(format) => {
                    let s = self.controller.state_mut();
                    s.gui.post.format = format;
                    s.gui.mark_edited();
                    let mut post = s.session.post_config().clone();
                    post.format = format.to_token().to_owned();
                    let command = rs_cam_core::session::Command::SetPostConfig(
                        rs_cam_core::session::SetPostConfigArgs {
                            post: Box::new(post),
                        },
                    );
                    if let Err(error) = s.session.apply(command) {
                        tracing::warn!("the post format write was refused: {error}");
                    }
                }
                AppEvent::ExportCombinedGcode => {
                    match crate::io::export::export_combined_gcode_from_session(
                        &self.controller.state().session,
                        &self.controller.state().gui,
                        &self.controller.state().simulation,
                    ) {
                        Ok(gcode) => {
                            let default_name =
                                format!("{}_combined.nc", self.controller.state().session.name())
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
                                        format!("Exported combined G-code to {}", path.display()),
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
                        .session
                        .list_setups()
                        .iter()
                        .find(|s| crate::state::job::SetupId(s.id) == setup_id)
                        .map(|s| s.name.clone())
                        .unwrap_or_default();
                    match crate::io::export::export_setup_gcode_from_session(
                        &self.controller.state().session,
                        &self.controller.state().gui,
                        &self.controller.state().simulation,
                        setup_id,
                    ) {
                        Ok(gcode) => {
                            let default_name = format!(
                                "{}_{}.nc",
                                self.controller.state().session.name(),
                                setup_name
                            )
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
                    let path = self.controller.state().gui.file_path.clone().or_else(|| {
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
                    // G-OPENGUARD (F1.12): opening another project replaces
                    // this one, so an unsaved project asks the SAME question
                    // quitting asks, through the same dialog. The check sits
                    // ahead of the file picker and the load, so a cancelled
                    // answer moves nothing — not the project, not the camera.
                    if self.controller.state().gui.dirty {
                        self.unsaved_guard = Some(super::UnsavedGuard::OpenJob);
                    } else {
                        self.open_job_interactive();
                    }
                }

                // --- View commands (WP13) ---
                //
                // These arms run in the application and not in the
                // controller, because each one writes the camera, the
                // window or a file dialog. Every other view command
                // falls through to the controller's own `Ui` arm.
                AppEvent::Ui(cmd) => match cmd {
                    UiCommand::SetViewPreset(preset) => self.camera.set_preset(preset),
                    UiCommand::ToggleProjection(NoArgs) => self.camera.toggle_projection(),
                    UiCommand::PreviewOrientation(face_up) => {
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
                    UiCommand::ResetView(NoArgs) => self.fit_camera_to_first_model(),
                    // Workspace transitions (need camera/viewport changes in app)
                    UiCommand::SwitchWorkspace(target) => {
                        crate::ui::overlays::registry::switch_workspace(
                            self.controller.state_mut(),
                            target,
                        );
                    }
                    UiCommand::SimStepBackward(NoArgs) => {
                        if self.controller.state().simulation.has_results() {
                            let pb = &mut self.controller.state_mut().simulation.playback;
                            pb.playing = false;
                            pb.current_move = pb.current_move.saturating_sub(1);
                            self.pending_checkpoint_load = true;
                        }
                    }
                    UiCommand::SimJumpToStart(NoArgs) => {
                        if self.controller.state().simulation.has_results() {
                            let pb = &mut self.controller.state_mut().simulation.playback;
                            pb.playing = false;
                            pb.current_move = 0;
                            self.pending_checkpoint_load = true;
                        }
                    }
                    UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                        move_index: move_idx,
                    }) => {
                        if self.controller.state().simulation.has_results() {
                            let total = self.controller.state().simulation.total_moves();
                            let previous = self.controller.state().simulation.playback.current_move;
                            let pb = &mut self.controller.state_mut().simulation.playback;
                            pb.playing = false;
                            pb.current_move = move_idx.min(total);
                            self.pending_checkpoint_load =
                                !pb.scrub_drag_active && pb.current_move < previous;
                        }
                    }
                    UiCommand::SimJumpToOpStart(SimJumpToOpStartArgs {
                        boundary_index: boundary_idx,
                    }) => {
                        if let Some(start) = self
                            .controller
                            .state()
                            .simulation
                            .boundaries()
                            .get(boundary_idx)
                            .map(|b| b.start_move)
                        {
                            let previous = self.controller.state().simulation.playback.current_move;
                            let pb = &mut self.controller.state_mut().simulation.playback;
                            pb.playing = false;
                            pb.current_move = start;
                            // Only force a checkpoint reload when jumping backward;
                            // forward jumps stream from current state (matches the
                            // SimJumpToMove logic). This is what makes "click TP 0
                            // (rough) from TP 6 (finish)" the slow path that spawned
                            // the loading-overlay UX, while clicks down the list
                            // stay fast. During an active drag we defer checkpoint
                            // reload until release so scrubbing doesn't block on a
                            // full checkpoint mesh upload per pointer move.
                            self.pending_checkpoint_load =
                                !pb.scrub_drag_active && pb.current_move < previous;
                        }
                    }
                    // Export events (need file dialogs)
                    UiCommand::ExportGcode(NoArgs) => {
                        let s = self.controller.state_mut();
                        s.close_modals_for_exclusivity();
                        s.show_preflight = true;
                    }
                    UiCommand::OpenExportWizard(NoArgs) => {
                        let resume = self
                            .controller
                            .state()
                            .gui
                            .wizard
                            .last_step_visited
                            .min(crate::ui::export_wizard::STEP_COUNT - 1);
                        let s = self.controller.state_mut();
                        s.close_modals_for_exclusivity();
                        s.show_export_wizard = true;
                        s.wizard_active_step = resume;
                    }
                    UiCommand::CloseExportWizard(NoArgs) => {
                        self.controller.state_mut().show_export_wizard = false;
                    }
                    UiCommand::SetToolLoadOverride(SetToolLoadOverrideArgs {
                        accept_unmodeled,
                        accept_exceeded,
                    }) => {
                        let gui = &mut self.controller.state_mut().gui;
                        gui.tool_load_overrides.accept_unmodeled = accept_unmodeled;
                        gui.tool_load_overrides.accept_exceeded = accept_exceeded;
                    }
                    UiCommand::SetStaleExportPolicy(policy) => {
                        self.controller.state_mut().gui.stale_export = policy;
                    }
                    UiCommand::ShowShortcuts(NoArgs) => {
                        self.controller.state_mut().show_shortcuts = true;
                    }
                    UiCommand::Quit(NoArgs) => {
                        if self.controller.state().gui.dirty {
                            self.unsaved_guard = Some(super::UnsavedGuard::Quit);
                        } else {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    other => self.controller.handle_internal_event(AppEvent::Ui(other)),
                },

                // Everything else delegated to controller
                other => self.controller.handle_internal_event(other),
            }
        }
    }

    /// Handle keyboard shortcuts for the viewport and application.
    pub(super) fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        // Only process shortcuts when no text edit is focused
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }

        ctx.input(|i| {
            let modifiers = i.modifiers;

            // Delete: remove the selected toolpath or tool (W1.1 — the tool
            // arm was missing, so Del never fired on a selected tool).
            if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                match self.controller.state().selection {
                    Selection::Toolpath(id) => {
                        self.controller
                            .events_mut()
                            .push(AppEvent::RemoveToolpath(id));
                    }
                    Selection::Tool(id) => {
                        self.controller.events_mut().push(AppEvent::RemoveTool(id));
                    }
                    _ => {}
                }
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
                    .push(AppEvent::Ui(UiCommand::SwitchWorkspace(
                        Workspace::Simulation,
                    )));
            }

            // I: toggle isolation mode
            if i.key_pressed(egui::Key::I) {
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::ToggleIsolateToolpath(NoArgs)));
            }

            // H: toggle visibility of selected toolpath
            if i.key_pressed(egui::Key::H)
                && let Selection::Toolpath(id) = self.controller.state().selection
            {
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::ToggleToolpathVisibility(id)));
            }

            // 1-4: view presets
            if i.key_pressed(egui::Key::Num1) {
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::SetViewPreset(
                        crate::render::camera::ViewPreset::Top,
                    )));
            }
            if i.key_pressed(egui::Key::Num2) {
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::SetViewPreset(
                        crate::render::camera::ViewPreset::Front,
                    )));
            }
            if i.key_pressed(egui::Key::Num3) {
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::SetViewPreset(
                        crate::render::camera::ViewPreset::Right,
                    )));
            }
            if i.key_pressed(egui::Key::Num4) {
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::SetViewPreset(
                        crate::render::camera::ViewPreset::Isometric,
                    )));
            }
            self.handle_overlay_shortcuts(i);
        });
    }

    /// The overlay shortcuts (UX §6.8). Bound in every workspace that renders
    /// a viewport, because the overlays they reach are wanted in all of them.
    ///
    /// Before P6 no overlay had a shortcut at all. `I`, `H`, `G` and `1`-`4`
    /// keep their meanings; none of `O`, `S`, `P`, `R`, `X`, `,` or `.` was
    /// bound in either handler.
    ///
    /// Each toggle goes through the registry, so a key cannot switch on
    /// something that cannot draw, and cannot break per-surface exclusivity.
    ///
    /// Every binding refuses a command / control / alt modifier. `Ctrl+O`
    /// opens a project and `Ctrl+S` saves one (`ui/menu_bar.rs`), so a bare
    /// `key_pressed` here would fire the overlay action alongside the file
    /// action.
    fn handle_overlay_shortcuts(&mut self, i: &egui::InputState) {
        use crate::ui::overlays::registry;

        // Readiness renders no viewport, so a key there would flip a flag
        // the operator cannot see.
        if self.controller.state().workspace == Workspace::Readiness {
            return;
        }
        let modifiers = i.modifiers;
        if modifiers.command || modifiers.ctrl || modifiers.alt {
            return;
        }

        // O: open or close the panel. Shift+O: pin or unpin it.
        if i.key_pressed(egui::Key::O) {
            let overlays = &mut self.controller.state_mut().overlays;
            if modifiers.shift {
                overlays.pinned = !overlays.pinned;
                overlays.open = true;
            } else {
                overlays.open = !overlays.open;
            }
        }

        for (key, id) in [
            (egui::Key::S, "stock_box"),
            (egui::Key::P, "cutting_moves"),
            (egui::Key::R, "rapids"),
            (egui::Key::X, "collisions"),
        ] {
            if !i.key_pressed(key) || modifiers.any() {
                continue;
            }
            let Some(row) = registry::row(id) else {
                continue;
            };
            let state = self.controller.state_mut();
            let on = (row.get)(state);
            // Switching OFF needs no precondition; switching ON does.
            if on || (row.precondition)(state).is_ready() {
                registry::set_overlay(state, row, !on);
            }
        }

        // `,` / `.`: step the active model / stock colour source.
        if modifiers.any() {
            return;
        }
        if i.key_pressed(egui::Key::Comma) {
            registry::cycle_surface(
                self.controller.state_mut(),
                crate::ui::overlays::panel::COMMA_SURFACE,
            );
        }
        if i.key_pressed(egui::Key::Period) {
            registry::cycle_surface(
                self.controller.state_mut(),
                crate::ui::overlays::panel::PERIOD_SURFACE,
            );
        }
    }

    /// Handle keyboard shortcuts for the simulation workspace.
    pub(super) fn handle_simulation_shortcuts(&mut self, ctx: &egui::Context) {
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
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::SimStepBackward(NoArgs)));
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::SimStepForward(NoArgs)));
            }

            // Home/End: jump to start/end
            if i.key_pressed(egui::Key::Home) {
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::SimJumpToStart(NoArgs)));
            }
            if i.key_pressed(egui::Key::End) {
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::SimJumpToEnd(NoArgs)));
            }

            // Space: play/pause
            if i.key_pressed(egui::Key::Space) {
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::ToggleSimPlayback(NoArgs)));
            }

            // Escape: back to toolpaths workspace
            if i.key_pressed(egui::Key::Escape) {
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::SwitchWorkspace(
                        Workspace::Toolpaths,
                    )));
            }

            self.handle_overlay_shortcuts(i);

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

    /// Pick a project file and open it, fitting the camera on success.
    ///
    /// G-OPENGUARD (F1.12): extracted from the `AppEvent::OpenJob` arm so
    /// the unsaved-changes dialog can run the SAME flow after Save or
    /// Discard. Nothing here checks `dirty` — the guard is the caller's
    /// job, and this is deliberately the only place the picker, the load
    /// and the camera fit live.
    pub(super) fn open_job_interactive(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("TOML Job", &["toml"])
            .pick_file()
        else {
            return;
        };
        match self.controller.open_job_from_path(&path) {
            Ok(()) => {
                // G-WSMENU (2026-09-10): a load replaces every model in the
                // project, so the camera has to move with it — the import
                // dispatch has always fitted and this route never did,
                // leaving a loaded job off screen at whatever the previous
                // project's scale was. Same routine as Reset View, not a
                // second fit. It runs only on `Ok`, and only after the
                // unsaved guard has been answered.
                self.fit_camera_to_first_model();
                tracing::info!("Loaded job from {}", path.display());
            }
            Err(error) => self.controller.push_error(&error),
        }
    }
}
