use rs_cam_core::gcode_validator::{Severity, validate};
use rs_cam_core::session::OutputLayout;
use std::path::Path;

use super::RsCamApp;

impl RsCamApp {
    /// Save the wizard's configured export to disk. Pops a file or
    /// directory picker (depending on layout), respects the validator
    /// gate, writes the file(s), notifies, and closes the wizard on
    /// success.
    pub(super) fn handle_wizard_save(&mut self) {
        let state = self.controller.state();
        let layout = state.session.wizard().output_layout;
        let template = state.session.wizard().filename_template.clone();
        let allow_errors = state.session.wizard().allow_validator_errors;
        let job = if state.session.name().is_empty() {
            "untitled".to_owned()
        } else {
            slugify(state.session.name())
        };
        let last_dir = state.session.wizard().last_save_dir.clone();
        let post_format = state.gui.post.format;

        match layout {
            OutputLayout::SingleFile => {
                let gcode = match crate::io::export::export_gcode_from_session(
                    &state.session,
                    &state.gui,
                    &state.simulation,
                ) {
                    Ok(s) => s,
                    Err(err) => {
                        self.controller.push_error(&err);
                        return;
                    }
                };
                if !self.gate(&gcode, post_format, allow_errors) {
                    return;
                }
                let suggested = render_filename(&template, &job, None, None);
                let mut dialog = rfd::FileDialog::new()
                    .add_filter("G-code", &["nc", "gcode", "ngc"])
                    .set_file_name(&suggested);
                if let Some(d) = last_dir.as_deref() {
                    dialog = dialog.set_directory(d);
                }
                let Some(path) = dialog.save_file() else {
                    return;
                };
                self.write_one(&path, &gcode);
                self.remember_dir(path.parent());
                self.close_wizard();
            }
            OutputLayout::PerSetup => {
                let Ok(setup_outputs) = self.collect_setup_outputs() else {
                    return;
                };
                let combined: String = setup_outputs.iter().map(|(_, g)| g.as_str()).collect();
                if !self.gate(&combined, post_format, allow_errors) {
                    return;
                }
                let mut dialog = rfd::FileDialog::new();
                if let Some(d) = last_dir.as_deref() {
                    dialog = dialog.set_directory(d);
                }
                let Some(dir) = dialog.pick_folder() else {
                    return;
                };
                let mut written = Vec::with_capacity(setup_outputs.len());
                for (setup_name, gcode) in &setup_outputs {
                    let name = render_filename(&template, &job, Some(setup_name), None);
                    let path = dir.join(&name);
                    if let Err(e) = std::fs::write(&path, gcode) {
                        self.controller.push_notification(
                            format!("Failed to write {}: {e}", path.display()),
                            crate::controller::Severity::Error,
                        );
                        return;
                    }
                    written.push(path);
                }
                self.remember_dir(Some(&dir));
                self.controller.push_notification(
                    format!("Exported {} setup file(s) to {}", written.len(), dir.display()),
                    crate::controller::Severity::Info,
                );
                self.close_wizard();
            }
            OutputLayout::PerToolpath => {
                let Ok(toolpath_outputs) = self.collect_toolpath_outputs() else {
                    return;
                };
                let combined: String = toolpath_outputs.iter().map(|(_, g)| g.as_str()).collect();
                if !self.gate(&combined, post_format, allow_errors) {
                    return;
                }
                let mut dialog = rfd::FileDialog::new();
                if let Some(d) = last_dir.as_deref() {
                    dialog = dialog.set_directory(d);
                }
                let Some(dir) = dialog.pick_folder() else {
                    return;
                };
                let mut written = 0usize;
                for (tp_name, gcode) in &toolpath_outputs {
                    let name = render_filename(&template, &job, None, Some(tp_name));
                    let path = dir.join(&name);
                    if let Err(e) = std::fs::write(&path, gcode) {
                        self.controller.push_notification(
                            format!("Failed to write {}: {e}", path.display()),
                            crate::controller::Severity::Error,
                        );
                        return;
                    }
                    written += 1;
                }
                self.remember_dir(Some(&dir));
                self.controller.push_notification(
                    format!("Exported {written} toolpath file(s) to {}", dir.display()),
                    crate::controller::Severity::Info,
                );
                self.close_wizard();
            }
        }
    }

    fn collect_setup_outputs(&mut self) -> Result<Vec<(String, String)>, ()> {
        let setups: Vec<(crate::state::job::SetupId, String)> = self
            .controller
            .state()
            .session
            .list_setups()
            .iter()
            .map(|s| (crate::state::job::SetupId(s.id), s.name.clone()))
            .collect();
        let mut out = Vec::new();
        for (sid, name) in setups {
            let state = self.controller.state();
            match crate::io::export::export_setup_gcode_from_session(
                &state.session,
                &state.gui,
                &state.simulation,
                sid,
            ) {
                Ok(g) => out.push((name, g)),
                Err(crate::error::VizError::Export(msg)) if msg.starts_with("No computed") => {
                    // Skip empty setups silently.
                }
                Err(e) => {
                    self.controller.push_error(&e);
                    return Err(());
                }
            }
        }
        if out.is_empty() {
            self.controller.push_notification(
                "No setups have computed toolpaths to export".to_owned(),
                crate::controller::Severity::Warning,
            );
            return Err(());
        }
        Ok(out)
    }

    fn collect_toolpath_outputs(&mut self) -> Result<Vec<(String, String)>, ()> {
        let toolpaths: Vec<(usize, String)> = self
            .controller
            .state()
            .session
            .toolpath_configs()
            .iter()
            .filter(|tc| tc.enabled)
            .map(|tc| (tc.id, tc.name.clone()))
            .collect();
        let mut out = Vec::new();
        for (id, name) in toolpaths {
            let state = self.controller.state();
            match crate::io::export::export_single_toolpath_from_session(
                &state.session,
                &state.gui,
                &state.simulation,
                id,
            ) {
                Ok(g) => out.push((name, g)),
                Err(e) => {
                    self.controller.push_error(&e);
                    return Err(());
                }
            }
        }
        if out.is_empty() {
            self.controller.push_notification(
                "No enabled toolpaths to export".to_owned(),
                crate::controller::Severity::Warning,
            );
            return Err(());
        }
        Ok(out)
    }

    fn write_one(&mut self, path: &Path, gcode: &str) {
        if let Err(e) = std::fs::write(path, gcode) {
            tracing::error!("Failed to write G-code: {e}");
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

    fn gate(&mut self, gcode: &str, post: rs_cam_core::gcode::PostFormat, allow_errors: bool) -> bool {
        if allow_errors {
            return true;
        }
        let findings = validate(gcode, post);
        let errors = findings.iter().filter(|f| f.severity == Severity::Error).count();
        if errors > 0 {
            self.controller.push_notification(
                format!(
                    "Save blocked: {errors} validator error(s). Tick the override on the \
                     preview step to proceed anyway."
                ),
                crate::controller::Severity::Error,
            );
            return false;
        }
        true
    }

    fn remember_dir(&mut self, dir: Option<&Path>) {
        if let Some(d) = dir {
            self.controller.state_mut().session.wizard_mut().last_save_dir =
                Some(d.to_path_buf());
        }
    }

    fn close_wizard(&mut self) {
        self.controller.state_mut().show_export_wizard = false;
    }
}

fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn render_filename(template: &str, job: &str, setup: Option<&str>, toolpath: Option<&str>) -> String {
    let mut out = template
        .replace("{job}", job)
        .replace("{setup}", &setup.map(slugify).unwrap_or_else(|| "setup".to_owned()))
        .replace(
            "{toolpath}",
            &toolpath.map(slugify).unwrap_or_else(|| "toolpath".to_owned()),
        )
        .replace("{ext}", "nc");
    if !out.contains('.') {
        out.push_str(".nc");
    }
    out
}

// Wrap the existing impl block separately so the helpers above don't
// conflict with the original methods below.
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
