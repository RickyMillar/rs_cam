use crate::state::wizard::OutputLayout;
use crate::ui::components::format::slugify;
use rs_cam_core::gcode_validator::{Severity, validate};
use std::path::Path;

use super::RsCamApp;

impl RsCamApp {
    /// Save the wizard's configured export to disk. Pops a file or
    /// directory picker (depending on layout), respects the validator
    /// gate, writes the file(s), notifies, and closes the wizard on
    /// success.
    pub(super) fn handle_wizard_save(&mut self) {
        let state = self.controller.state();
        let layout = state.gui.wizard.output_layout;
        let template = state.gui.wizard.filename_template.clone();
        let allow_errors = state.gui.wizard.allow_validator_errors;
        let job = if state.session.name().is_empty() {
            "untitled".to_owned()
        } else {
            slugify(state.session.name())
        };
        let last_dir = state.gui.wizard.last_save_dir.clone();
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
                    format!(
                        "Exported {} setup file(s) to {}",
                        written.len(),
                        dir.display()
                    ),
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
        let toolpaths: Vec<(rs_cam_core::ToolpathId, String)> = self
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

    fn gate(
        &mut self,
        gcode: &str,
        post: rs_cam_core::gcode::PostFormat,
        allow_errors: bool,
    ) -> bool {
        if allow_errors {
            return true;
        }
        let findings = validate(gcode, post);
        let errors = findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count();
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
            self.controller.state_mut().gui.wizard.last_save_dir = Some(d.to_path_buf());
        }
    }

    fn close_wizard(&mut self) {
        self.controller.state_mut().show_export_wizard = false;
    }
}

fn render_filename(
    template: &str,
    job: &str,
    setup: Option<&str>,
    toolpath: Option<&str>,
) -> String {
    let mut out = template
        .replace("{job}", job)
        .replace(
            "{setup}",
            &setup.map(slugify).unwrap_or_else(|| "setup".to_owned()),
        )
        .replace(
            "{toolpath}",
            &toolpath
                .map(slugify)
                .unwrap_or_else(|| "toolpath".to_owned()),
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
                // G-TIMEEST — the log line below used to carry its own
                // `cutting_distance / feed_rate()`, so an export logged a
                // cycle time that disagreed with every GUI surface. It now
                // reads the one shared decision, basis included.
                let cycle = crate::ui::readiness::estimate_total_time(self.controller.state());

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
                    "Export summary: {} G-code lines, {} moves, {:.0} mm cutting, {} tool changes, {} ({})",
                    line_count,
                    total_moves,
                    cutting_dist,
                    tool_changes,
                    crate::ui::readiness::format_cycle_time(cycle.seconds),
                    cycle.basis.map_or("no estimate", |b| b.qualifier()),
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

    /// Render the unsaved-changes confirmation dialog.
    ///
    /// G-OPENGUARD (F1.12): ONE dialog for every action that discards the
    /// open project. It used to guard only Quit; File > Open / Ctrl+O
    /// replaced the project just as completely and asked nothing, so an
    /// unsaved job could be lost by opening another one. The three answers
    /// are the same in both cases — Save, Discard, Cancel — and only the
    /// verb and the consequence line change, so the two routes cannot
    /// drift apart the way two dialogs would.
    pub(super) fn show_unsaved_changes_dialog(&mut self, ctx: &egui::Context) {
        let Some(guard) = self.unsaved_guard else {
            return;
        };
        egui::Window::new("Unsaved Changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("You have unsaved changes. {}", guard.consequence()));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(format!("Save & {}", guard.verb())).clicked()
                        && self.save_for_unsaved_guard()
                    {
                        self.unsaved_guard = None;
                        self.proceed_past_unsaved_guard(ctx, guard);
                    }
                    // If the operator cancelled the file dialog,
                    // `save_for_unsaved_guard` returns false and this
                    // dialog stays open — nothing is discarded.
                    if ui.button(format!("Discard & {}", guard.verb())).clicked() {
                        // Clear the dirty flag so the next-frame
                        // close_requested check in `update` doesn't
                        // re-open this dialog and trap the user in a
                        // CancelClose loop.
                        self.controller.state_mut().gui.dirty = false;
                        self.unsaved_guard = None;
                        self.proceed_past_unsaved_guard(ctx, guard);
                    }
                    if ui.button("Cancel").clicked() {
                        self.unsaved_guard = None;
                    }
                });
            });
    }

    /// Save the open project for the unsaved-changes dialog, asking for a
    /// path when it has never been saved. Returns whether it saved: `false`
    /// means the operator cancelled the file dialog or the save failed, and
    /// the guarded action must NOT proceed.
    fn save_for_unsaved_guard(&mut self) -> bool {
        let path = self.controller.state().gui.file_path.clone().or_else(|| {
            rfd::FileDialog::new()
                .add_filter("TOML Job", &["toml"])
                .set_file_name("job.toml")
                .save_file()
        });
        let Some(path) = path else {
            return false;
        };
        match self.controller.save_job_to_path(&path) {
            Ok(()) => {
                tracing::info!("Saved job to {}", path.display());
                true
            }
            Err(error) => {
                self.controller.push_error(&error);
                false
            }
        }
    }

    /// Run the action the dialog was guarding, now that the project is
    /// saved or the operator has chosen to discard it.
    fn proceed_past_unsaved_guard(&mut self, ctx: &egui::Context, guard: super::UnsavedGuard) {
        match guard {
            super::UnsavedGuard::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            super::UnsavedGuard::OpenJob => self.open_job_interactive(),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use rs_cam_core::session::ProjectSessionBuilder;

    /// The wizard shows a filename preview; this module writes the file.
    /// Both read the one `slugify` in `ui::components::format`. A job
    /// name with a space must not preview one name and write another.
    #[test]
    fn the_preview_matches_the_saved_filename_for_a_spaced_job_name() {
        // L8 deleted the `SetProjectName` row. The builder names a
        // session the way every other fixture does.
        let state = crate::state::AppState {
            session: ProjectSessionBuilder::new()
                .name("Job 1".to_owned())
                .build(),
            ..crate::state::AppState::default()
        };

        let preview = crate::ui::export_wizard::render_filename_preview(
            "{job}.nc",
            &state,
            OutputLayout::SingleFile,
        );
        let saved = render_filename("{job}.nc", &slugify(state.session.name()), None, None);

        assert_eq!(preview, saved);
        assert_eq!(saved, "Job_1.nc");
    }
}
