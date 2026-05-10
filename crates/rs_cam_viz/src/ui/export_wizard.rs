//! Multi-step Export Wizard — Phase 5 of `GCODE_EXPORT_OVERHAUL.md`.
//!
//! Resumable settings live on `session.wizard()`; the active step and
//! visibility flag live on `AppState`. Each step is a small `step_*`
//! draw fn; `draw()` dispatches based on `state.wizard_active_step`.
//!
//! Step inventory (filled in incrementally — only Step 1 is wired so
//! far):
//!
//! 1. **Post.** Dropdown of `PostFormat::ALL`, post metadata, PostLimits warnings.
//! 2. Output layout (radio + filename template).
//! 3. Coordinate & units (WCS picker + units override + safe-Z).
//! 4. Tool change & spindle summary.
//! 5. Preview + validator findings inline.
//! 6. Save with summary.

use rs_cam_core::gcode::{PostDefinition, PostFormat};
use rs_cam_core::session::OutputLayout;

use super::AppEvent;
use crate::state::AppState;

/// Total number of steps in the wizard.
pub const STEP_COUNT: u8 = 6;

const STEP_TITLES: [&str; STEP_COUNT as usize] = [
    "Post",
    "Output layout",
    "Coordinate & units",
    "Tool change & spindle",
    "Preview & validate",
    "Save",
];

/// Render the wizard if `state.show_export_wizard` is set.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    if !state.show_export_wizard {
        return;
    }
    let mut still_open = true;
    egui::Window::new("Export Wizard")
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(640.0)
        .open(&mut still_open)
        .show(ctx, |ui| {
            draw_stepper(ui, state.wizard_active_step, events);
            ui.separator();
            ui.add_space(6.0);
            match state.wizard_active_step {
                0 => step_post(ui, state, events),
                1 => step_output_layout(ui, state, events),
                _ => placeholder(ui, state.wizard_active_step),
            }
            ui.add_space(8.0);
            ui.separator();
            draw_nav(ui, state.wizard_active_step, events);
        });
    if !still_open {
        events.push(AppEvent::CloseExportWizard);
    }
}

fn draw_stepper(ui: &mut egui::Ui, active: u8, events: &mut Vec<AppEvent>) {
    ui.horizontal(|ui| {
        for (idx, title) in STEP_TITLES.iter().enumerate() {
            let n = idx as u8;
            let label = format!("{}. {}", idx + 1, title);
            let mut text = egui::RichText::new(label);
            if n == active {
                text = text.strong();
            }
            if ui
                .add(egui::SelectableLabel::new(n == active, text))
                .clicked()
            {
                events.push(AppEvent::WizardSetStep(n));
            }
            if idx + 1 < STEP_TITLES.len() {
                ui.label("›");
            }
        }
    });
}

fn draw_nav(ui: &mut egui::Ui, active: u8, events: &mut Vec<AppEvent>) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(active > 0, egui::Button::new("◀ Back"))
            .clicked()
        {
            events.push(AppEvent::WizardSetStep(active.saturating_sub(1)));
        }
        let next_label = if active + 1 == STEP_COUNT {
            "Save"
        } else {
            "Next ▶"
        };
        let next_enabled = active + 1 < STEP_COUNT;
        if ui
            .add_enabled(next_enabled, egui::Button::new(next_label))
            .clicked()
        {
            events.push(AppEvent::WizardSetStep(active + 1));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Cancel").clicked() {
                events.push(AppEvent::CloseExportWizard);
            }
        });
    });
}

fn placeholder(ui: &mut egui::Ui, step: u8) {
    ui.label(
        egui::RichText::new(format!(
            "Step {} — coming soon. Use Back to return to wired steps.",
            step + 1
        ))
        .italics(),
    );
}

// ── Step 1 — Post ────────────────────────────────────────────────────

fn step_post(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Post-processor");
    ui.add_space(4.0);

    let current = state.gui.post.format;
    let mut selected = current;

    egui::ComboBox::from_label("Controller")
        .selected_text(current.label())
        .show_ui(ui, |ui| {
            for pf in PostFormat::ALL {
                ui.selectable_value(&mut selected, *pf, pf.label());
            }
        });

    if selected != current {
        events.push(AppEvent::WizardSetPost(selected));
    }

    ui.add_space(10.0);
    let post_def = selected.definition();

    egui::Grid::new("wizard_step_post_meta")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Name:");
            ui.label(&post_def.name);
            ui.end_row();

            ui.label("Units:");
            ui.label(post_def.units.as_word());
            ui.end_row();

            ui.label("Default WCS:");
            let wcs_label = post_def
                .wcs
                .map(|w| w.as_word().to_owned())
                .unwrap_or_else(|| "(none in preamble)".to_owned());
            ui.label(wcs_label);
            ui.end_row();

            ui.label("XYZ decimals:");
            ui.label(post_def.decimals.xyz.to_string());
            ui.end_row();

            ui.label("Feed decimals:");
            ui.label(post_def.decimals.feed.to_string());
            ui.end_row();

            ui.label("Cutter comp:");
            ui.label(if post_def.supports_cutter_comp {
                "supported"
            } else {
                "not supported (lines dropped)"
            });
            ui.end_row();

            ui.label("Arc linearise:");
            let arc_lbl = if post_def.arc_linearize.enabled {
                format!(
                    "on (≤ {:.3} mm radius → G1 chord)",
                    post_def.arc_linearize.threshold_mm
                )
            } else {
                "off".to_owned()
            };
            ui.label(arc_lbl);
            ui.end_row();
        });

    ui.add_space(10.0);
    draw_limit_warnings(ui, post_def, state.gui.post.spindle_speed);
}

// ── Step 2 — Output layout ───────────────────────────────────────────

fn step_output_layout(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Output layout");
    ui.add_space(4.0);

    let wiz = state.session.wizard();
    let current = wiz.output_layout;
    let mut selected = current;

    ui.label("How should the emitted g-code be split across files?");
    ui.add_space(4.0);
    for &layout in &[
        OutputLayout::SingleFile,
        OutputLayout::PerSetup,
        OutputLayout::PerToolpath,
    ] {
        ui.radio_value(&mut selected, layout, layout.label());
    }
    if selected != current {
        events.push(AppEvent::WizardSetOutputLayout(selected));
    }

    ui.add_space(4.0);
    let setup_count = state.session.list_setups().len();
    if matches!(selected, OutputLayout::PerSetup) && setup_count <= 1 {
        ui.colored_label(
            egui::Color32::from_rgb(220, 140, 0),
            "⚠ Project has only one setup — \"per setup\" will produce a single file.",
        );
    }

    ui.add_space(12.0);
    ui.heading("Filename template");
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Substitutions: {job}, {setup}, {toolpath}, {ext}")
            .small()
            .italics(),
    );
    let mut template = wiz.filename_template.clone();
    let resp = ui.add(
        egui::TextEdit::singleline(&mut template)
            .desired_width(360.0)
            .hint_text("{job}.nc"),
    );
    if resp.changed() {
        events.push(AppEvent::WizardSetFilenameTemplate(template));
    }

    ui.add_space(8.0);
    let preview = render_filename_preview(&wiz.filename_template, state, selected);
    egui::Grid::new("wizard_filename_preview")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Preview:");
            ui.label(egui::RichText::new(preview).monospace());
            ui.end_row();
        });
}

fn render_filename_preview(template: &str, state: &AppState, layout: OutputLayout) -> String {
    let job = if state.session.name().is_empty() {
        "untitled"
    } else {
        state.session.name()
    };
    let setup = state
        .session
        .list_setups()
        .first()
        .map(|s| s.name.as_str())
        .unwrap_or("setup1");
    let toolpath = state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.enabled)
        .map(|tc| tc.name.as_str())
        .unwrap_or("toolpath1");
    let mut out = template
        .replace("{job}", &slugify(job))
        .replace("{setup}", &slugify(setup))
        .replace("{toolpath}", &slugify(toolpath))
        .replace("{ext}", "nc");
    if !out.contains('.') {
        out.push_str(".nc");
    }
    match layout {
        OutputLayout::SingleFile => out,
        OutputLayout::PerSetup => format!("{out}  (one per setup)"),
        OutputLayout::PerToolpath => format!("{out}  (one per toolpath)"),
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

fn draw_limit_warnings(ui: &mut egui::Ui, post: &PostDefinition, project_rpm: u32) {
    let mut any = false;
    if let Some(max_rpm) = post.limits.max_rpm
        && project_rpm > max_rpm.get()
    {
        ui.colored_label(
            egui::Color32::from_rgb(220, 140, 0),
            format!(
                "⚠ Project spindle {} rpm exceeds post limit {} rpm — \
                 emitter will clamp at the move site.",
                project_rpm,
                max_rpm.get(),
            ),
        );
        any = true;
    }
    if let Some(max_feed) = post.limits.max_feed {
        ui.label(
            egui::RichText::new(format!(
                "Post limit: max feed {:.0} mm/min (per-move clamping)",
                max_feed.get()
            ))
            .small(),
        );
        any = true;
    }
    if !any {
        ui.label(
            egui::RichText::new("No PostLimits set — values pass through unclamped.")
                .small()
                .italics(),
        );
    }
}
