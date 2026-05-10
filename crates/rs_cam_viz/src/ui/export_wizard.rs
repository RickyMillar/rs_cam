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

use rs_cam_core::gcode::{CoolantMode, PostDefinition, PostFormat, Units, WcsCode};
use rs_cam_core::gcode_validator::{Finding, Severity, validate};
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
                2 => step_coord_units(ui, state, events),
                3 => step_tool_change(ui, state, events),
                4 => step_preview(ui, state, events),
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

// ── Step 3 — Coordinate & units ──────────────────────────────────────

fn step_coord_units(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Coordinate system & units");
    ui.add_space(4.0);

    let post = state.gui.post.format.definition();
    let wiz = state.session.wizard();

    // ── WCS picker ──
    ui.label(
        egui::RichText::new(format!(
            "Post default: {}",
            post.wcs.map_or("(none in preamble)", |w| w.as_word())
        ))
        .small()
        .italics(),
    );
    let mut wcs_selected = wiz.wcs_override;
    let wcs_label = wcs_selected.map_or_else(|| "Use post default".to_owned(), |w| w.as_word().to_owned());
    egui::ComboBox::from_label("Work coordinate system (WCS)")
        .selected_text(wcs_label)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut wcs_selected, None, "Use post default");
            for w in [
                WcsCode::G54,
                WcsCode::G55,
                WcsCode::G56,
                WcsCode::G57,
                WcsCode::G58,
                WcsCode::G59,
            ] {
                ui.selectable_value(&mut wcs_selected, Some(w), w.as_word());
            }
        });
    if wcs_selected != wiz.wcs_override {
        events.push(AppEvent::WizardSetWcsOverride(wcs_selected));
    }

    ui.add_space(12.0);

    // ── Units picker ──
    ui.label(
        egui::RichText::new(format!("Post emits: {} ({})", post.units.as_word(), units_label(post.units)))
            .small()
            .italics(),
    );
    let mut units_selected = wiz.units_override;
    let units_label_text = units_selected.map_or_else(
        || "Use post default".to_owned(),
        |u| format!("{} ({})", u.as_word(), units_label(u)),
    );
    egui::ComboBox::from_label("Units")
        .selected_text(units_label_text)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut units_selected, None, "Use post default");
            ui.selectable_value(&mut units_selected, Some(Units::Mm), "G21 (mm)");
            ui.selectable_value(&mut units_selected, Some(Units::Inch), "G20 (inch)");
        });
    if units_selected != wiz.units_override {
        events.push(AppEvent::WizardSetUnitsOverride(units_selected));
    }

    if let Some(u) = units_selected
        && u != post.units
    {
        ui.colored_label(
            egui::Color32::from_rgb(220, 140, 0),
            format!(
                "⚠ Units override ({}) differs from post default ({}). \
                 Coordinate values are not auto-converted — verify your \
                 toolpath is in the right unit system.",
                u.as_word(),
                post.units.as_word(),
            ),
        );
    }

    ui.add_space(12.0);

    // ── Safe-Z override ──
    let project_safe_z = state.gui.post.safe_z;
    ui.label(
        egui::RichText::new(format!("Project default: {project_safe_z:.3} mm"))
            .small()
            .italics(),
    );
    let mut use_override = wiz.safe_z_override.is_some();
    let mut safe_z_value = wiz.safe_z_override.unwrap_or(project_safe_z);
    let toggle = ui.checkbox(&mut use_override, "Override safe-Z for this export");
    let prev_override = wiz.safe_z_override;

    if use_override {
        let resp = ui.add(
            egui::DragValue::new(&mut safe_z_value)
                .speed(0.5)
                .suffix(" mm")
                .range(-1000.0..=1000.0),
        );
        let new_override = Some(safe_z_value);
        if (toggle.changed() || resp.changed()) && new_override != prev_override {
            events.push(AppEvent::WizardSetSafeZOverride(new_override));
        }
    } else if toggle.changed() && prev_override.is_some() {
        events.push(AppEvent::WizardSetSafeZOverride(None));
    }
}

// ── Step 4 — Tool change & spindle ───────────────────────────────────

fn step_tool_change(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Tool change & spindle");
    ui.add_space(4.0);

    let session = &state.session;
    let enabled_tcs: Vec<&rs_cam_core::session::ToolpathConfig> = session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .collect();

    if enabled_tcs.is_empty() {
        ui.colored_label(
            egui::Color32::from_rgb(220, 140, 0),
            "⚠ No enabled toolpaths — export will fail at the next step.",
        );
        return;
    }

    // ── Tool summary (read-only) ──
    ui.label(
        egui::RichText::new("Tools used (edit pre/post snippets in the toolpath inspector):")
            .small(),
    );
    ui.add_space(4.0);
    let mut by_tool: std::collections::BTreeMap<usize, Vec<&rs_cam_core::session::ToolpathConfig>> =
        std::collections::BTreeMap::new();
    for tc in &enabled_tcs {
        by_tool.entry(tc.tool_id).or_default().push(*tc);
    }
    egui::Grid::new("wizard_step_tool_summary")
        .num_columns(4)
        .spacing([12.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Tool").strong());
            ui.label(egui::RichText::new("Toolpaths").strong());
            ui.label(egui::RichText::new("Pre").strong());
            ui.label(egui::RichText::new("Post").strong());
            ui.end_row();
            for (tool_id, tcs) in &by_tool {
                let tool_name = session
                    .tools()
                    .iter()
                    .find(|t| t.id.0 == *tool_id)
                    .map_or_else(|| format!("tool {tool_id}"), |t| t.name.clone());
                ui.label(tool_name);
                ui.label(format!("{}", tcs.len()));
                let any_pre = tcs.iter().any(|tc| tc.pre_gcode.is_some());
                let any_post = tcs.iter().any(|tc| tc.post_gcode.is_some());
                ui.label(if any_pre { "✓" } else { "—" });
                ui.label(if any_post { "✓" } else { "—" });
                ui.end_row();
            }
        });
    if by_tool.len() > 1 {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "{} tool change{} required during export.",
                by_tool.len() - 1,
                if by_tool.len() == 2 { "" } else { "s" },
            ))
            .small()
            .italics(),
        );
    }

    ui.add_space(12.0);

    // ── Spindle warmup ──
    ui.heading("Spindle warmup");
    ui.add_space(4.0);
    let wiz = state.session.wizard();
    let mut warmup = wiz.spindle_warmup_secs;
    let resp = ui.add(
        egui::DragValue::new(&mut warmup)
            .speed(1.0)
            .suffix(" s")
            .range(0..=120),
    );
    ui.label(
        egui::RichText::new(
            "Dwell after spindle-on before the first cutting move. Zero = no extra dwell \
             beyond the post's preamble.",
        )
        .small()
        .italics(),
    );
    if resp.changed() && warmup != wiz.spindle_warmup_secs {
        events.push(AppEvent::WizardSetSpindleWarmup(warmup));
    }

    ui.add_space(12.0);

    // ── Coolant summary ──
    ui.heading("Coolant");
    ui.add_space(4.0);
    let mut counts = [0usize; 4];
    for tc in &enabled_tcs {
        let idx = coolant_idx(tc.coolant);
        // SAFETY: coolant_idx returns 0..=3, counts is len 4.
        #[allow(clippy::indexing_slicing)]
        {
            counts[idx] += 1;
        }
    }
    egui::Grid::new("wizard_step_coolant_summary")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for (idx, &count) in counts.iter().enumerate() {
                if count == 0 {
                    continue;
                }
                ui.label(coolant_label(coolant_from_idx(idx)));
                ui.label(format!("{count} toolpath(s)"));
                ui.end_row();
            }
        });
    ui.label(
        egui::RichText::new("Coolant is per-toolpath; edit in the toolpath inspector.")
            .small()
            .italics(),
    );
}

// ── Step 5 — Preview & validate ──────────────────────────────────────

const PREVIEW_LINE_LIMIT: usize = 200;

fn step_preview(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Preview & validate");
    ui.add_space(4.0);

    // Re-emit on every frame. The emitter is fast (sub-ms for typical
    // projects); cache only matters if profiling shows a hotspot.
    let gcode_result = crate::io::export::export_gcode_from_session(
        &state.session,
        &state.gui,
        &state.simulation,
    );

    let gcode = match gcode_result {
        Ok(s) => s,
        Err(err) => {
            ui.colored_label(
                egui::Color32::from_rgb(220, 60, 60),
                format!("Cannot generate preview: {err}"),
            );
            return;
        }
    };

    let format = state.gui.post.format;
    let findings = validate(&gcode, format);

    let line_count = gcode.lines().count();
    let preview: String = gcode
        .lines()
        .take(PREVIEW_LINE_LIMIT)
        .collect::<Vec<_>>()
        .join("\n");

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} lines emitted; preview shows first {} line(s).",
                line_count,
                line_count.min(PREVIEW_LINE_LIMIT),
            ))
            .small(),
        );
        if line_count > PREVIEW_LINE_LIMIT {
            ui.label(
                egui::RichText::new(format!("(+{} hidden)", line_count - PREVIEW_LINE_LIMIT))
                    .small()
                    .italics(),
            );
        }
    });
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .id_salt("wizard_preview_scroll")
        .max_height(220.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut preview.as_str())
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(12),
            );
        });

    ui.add_space(10.0);
    ui.heading("Validator findings");
    ui.add_space(4.0);

    if findings.is_empty() {
        ui.colored_label(
            egui::Color32::from_rgb(60, 180, 90),
            "✓ No findings. Safe to save.",
        );
    } else {
        let (errors, warnings, infos) = count_by_severity(&findings);
        ui.label(
            egui::RichText::new(format!(
                "{} error(s), {} warning(s), {} info note(s).",
                errors, warnings, infos,
            ))
            .small(),
        );
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt("wizard_findings_scroll")
            .max_height(160.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for f in &findings {
                    draw_finding(ui, f);
                }
            });

        if errors > 0 {
            ui.add_space(8.0);
            let wiz = state.session.wizard();
            let mut allow = wiz.allow_validator_errors;
            let resp = ui.checkbox(
                &mut allow,
                "I understand the risks — allow save with validator errors present",
            );
            if resp.changed() && allow != wiz.allow_validator_errors {
                events.push(AppEvent::WizardSetAllowValidatorErrors(allow));
            }
            if !allow {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 60, 60),
                    "Save is blocked: errors must be resolved or the override above must be checked.",
                );
            }
        }
    }
}

fn draw_finding(ui: &mut egui::Ui, f: &Finding) {
    let (icon, color) = match f.severity {
        Severity::Error => ("✕", egui::Color32::from_rgb(220, 60, 60)),
        Severity::Warning => ("⚠", egui::Color32::from_rgb(220, 140, 0)),
        Severity::Info => ("ℹ", egui::Color32::from_rgb(120, 160, 220)),
    };
    ui.horizontal(|ui| {
        ui.colored_label(color, icon);
        ui.label(
            egui::RichText::new(format!("L{}: {:?} — {}", f.line, f.kind, f.message)).small(),
        );
    });
}

fn count_by_severity(findings: &[Finding]) -> (usize, usize, usize) {
    let mut e = 0;
    let mut w = 0;
    let mut i = 0;
    for f in findings {
        match f.severity {
            Severity::Error => e += 1,
            Severity::Warning => w += 1,
            Severity::Info => i += 1,
        }
    }
    (e, w, i)
}

fn coolant_idx(c: CoolantMode) -> usize {
    match c {
        CoolantMode::Off => 0,
        CoolantMode::Mist => 1,
        CoolantMode::Flood => 2,
        CoolantMode::Both => 3,
    }
}
fn coolant_from_idx(i: usize) -> CoolantMode {
    match i {
        1 => CoolantMode::Mist,
        2 => CoolantMode::Flood,
        3 => CoolantMode::Both,
        _ => CoolantMode::Off,
    }
}
fn coolant_label(c: CoolantMode) -> &'static str {
    match c {
        CoolantMode::Off => "Off",
        CoolantMode::Mist => "Mist (M7)",
        CoolantMode::Flood => "Flood (M8)",
        CoolantMode::Both => "Mist + Flood (M7+M8)",
    }
}

fn units_label(u: Units) -> &'static str {
    match u {
        Units::Mm => "mm",
        Units::Inch => "inch",
    }
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
