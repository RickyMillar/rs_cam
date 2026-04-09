use super::RsCamApp;

impl RsCamApp {
    pub(super) fn export_gcode_with_summary(&mut self) {
        match self.controller.export_gcode() {
            Ok(gcode) => {
                let line_count = gcode.lines().count();
                let mut total_moves = 0usize;
                let mut cutting_dist = 0.0f64;
                let mut est_time_min = 0.0f64;

                let tool_changes = {
                    let state = self.controller.state();
                    let session = &state.session;
                    let gui = &state.gui;
                    for tc in session.toolpath_configs() {
                        if tc.enabled
                            && let Some(result) =
                                gui.toolpath_rt.get(&tc.id).and_then(|r| r.result.as_ref())
                        {
                            total_moves += result.stats.move_count;
                            cutting_dist += result.stats.cutting_distance;
                            let feed = tc.operation.feed_rate().max(1.0);
                            est_time_min += result.stats.cutting_distance / feed;
                        }
                    }

                    let mut seen_tools = Vec::new();
                    for tc in session.toolpath_configs() {
                        if tc.enabled && !seen_tools.contains(&tc.tool_id) {
                            seen_tools.push(tc.tool_id);
                        }
                    }
                    if seen_tools.len() > 1 {
                        seen_tools.len() - 1
                    } else {
                        0
                    }
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

    pub(super) fn export_svg_preview(&mut self) {
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
                self.controller
                    .push_notification(format!("{error}"), crate::controller::Severity::Warning);
            }
        }
    }

    /// Render the unsaved-changes confirmation dialog when the user tries to quit.
    pub(super) fn show_unsaved_changes_dialog(&mut self, ctx: &egui::Context) {
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
}
