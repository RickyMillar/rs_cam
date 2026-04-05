use crate::state::Workspace;
use crate::state::selection::Selection;
use crate::ui::AppEvent;

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

    /// Handle keyboard shortcuts for the viewport and application.
    pub(super) fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
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
}
