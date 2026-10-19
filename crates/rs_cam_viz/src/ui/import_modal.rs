//! The import dialog of a drawing, SVG or DXF (geometry selection P4.8).
//!
//! The dialog shows the size the file declares (SVG), the scale, the size
//! of the import at that scale, the join tolerance and the join report. A
//! committed change reads the file again (`AppEvent::ImportDialogEdit`);
//! Import adds the model of that read. The default scale is core's
//! `svg_default_units`: the operator has not ruled on O-2, so it is 1:1.

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::io::ImportOptions;
use rs_cam_core::io::svg_input::SvgDeclaredSize;

use crate::state::AppState;
use crate::state::panels::ImportDialog;
use crate::ui::AppEvent;
use crate::ui::components::{DataRow, DataTable, Role};

/// The "File size" row text: the declared size in mm when both lengths
/// carry a physical unit, then the attributes as written.
pub fn declared_text(declared: &SvgDeclaredSize) -> String {
    let physical = declared
        .width
        .as_ref()
        .and_then(|w| w.mm)
        .zip(declared.height.as_ref().and_then(|h| h.mm));
    match physical {
        Some((w, h)) => format!("{w:.2} x {h:.2} mm  ({})", declared.summary_line()),
        None => format!("{}  (no physical unit)", declared.summary_line()),
    }
}

/// The "Result" row text: the XY size of the model the import gives.
pub fn result_text(model: &rs_cam_core::session::LoadedModel) -> String {
    match model.bbox() {
        Some(b) => format!("{:.2} x {:.2} mm", b.max.x - b.min.x, b.max.y - b.min.y),
        None => "no geometry".to_owned(),
    }
}

/// The fixed parts of one [`committed_number`] row.
pub struct NumberField<'a> {
    pub label: &'a str,
    pub suffix: &'a str,
    pub speed: f64,
    pub range: std::ops::RangeInclusive<f64>,
    pub hover: &'a str,
}

/// A number field that reports a value only when the operator commits it:
/// at the end of a drag, or when the text field loses focus. The draft
/// lives in egui memory under `id`, so a value is not read again on each
/// keystroke (a read of a large file takes time).
pub fn committed_number(
    ui: &mut egui::Ui,
    id: egui::Id,
    field: NumberField<'_>,
    committed: f64,
) -> Option<f64> {
    let mut draft: f64 = ui.data(|d| d.get_temp(id)).unwrap_or(committed);
    let outcome = crate::ui::components::ValueRow::new(
        field.label,
        &mut draft,
        field.suffix,
        field.speed,
        field.range,
    )
    .tooltip(Some(field.hover))
    .show(ui);
    let response = &outcome.value_response;
    let done = response.drag_stopped() || response.lost_focus();
    let active = response.dragged() || response.has_focus();
    if active {
        ui.data_mut(|d| d.insert_temp(id, draft));
    } else {
        ui.data_mut(|d| d.remove::<f64>(id));
    }
    (done && (draft - committed).abs() > f64::EPSILON).then_some(draft)
}

/// The scale row: a units preset or a custom factor. Returns the units the
/// operator chose this frame.
pub fn units_row(ui: &mut egui::Ui, id: egui::Id, current: ModelUnits) -> Option<ModelUnits> {
    let mut chosen = None;
    ui.horizontal(|ui| {
        crate::ui::components::value_row::label_cell(ui, "Scale");
        egui::ComboBox::from_id_salt(id.with("preset"))
            .truncate()
            .selected_text(current.label())
            .show_ui(ui, |ui| {
                for &(units, label) in ModelUnits::PRESETS {
                    let selected = units == current;
                    if ui.selectable_label(selected, label).clicked() && !selected {
                        chosen = Some(units);
                    }
                }
            });
    });
    if let Some(scale) = committed_number(
        ui,
        id.with("custom"),
        NumberField {
            label: "Custom scale",
            suffix: " x",
            speed: 0.01,
            range: 0.001..=100_000.0,
            hover: "One unit of the file times this factor is 1 mm.",
        },
        current.scale_factor(),
    ) {
        chosen = Some(ModelUnits::Custom(scale));
    }
    chosen
}

/// The join tolerance row. Returns the new options when the operator
/// commits a value.
pub fn join_row(ui: &mut egui::Ui, id: egui::Id, current: ImportOptions) -> Option<ImportOptions> {
    committed_number(
        ui,
        id,
        NumberField {
            label: "Join gaps up to",
            suffix: " mm",
            speed: 0.01,
            range: 0.0001..=100.0,
            hover: "Two open piece ends closer than this join into one shape. A shape whose ends \
                    meet is closed. The value is in mm of the file, before the scale.",
        },
        current.join_tolerance_mm,
    )
    .map(|join_tolerance_mm| ImportOptions { join_tolerance_mm })
}

/// The join report as a table row: CAUTION while open shapes are left.
pub fn report_row(report: &rs_cam_core::io::ImportReport) -> DataRow {
    let line = report.summary_line();
    let row = if report.total().open_left > 0 {
        DataRow::note(Role::Caution, format!("Report: {line}"))
    } else {
        DataRow::value("Report", line)
    };
    let per_layer: Vec<String> = report
        .layers
        .iter()
        .map(|l| {
            format!(
                "{}: {} pieces \u{2192} {} shapes, {} open",
                l.layer.as_deref().unwrap_or("(no layer)"),
                l.counts.pieces,
                l.counts.shapes,
                l.counts.open_left
            )
        })
        .collect();
    if per_layer.is_empty() {
        row
    } else {
        row.hover(per_layer.join("\n"))
    }
}

/// Draw the import dialog when it is open.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    let Some(dialog) = state.panels.import_dialog.as_ref() else {
        return;
    };
    let response = egui::Modal::new(egui::Id::new("import_drawing_dialog")).show(ctx, |ui| {
        ui.set_max_width(460.0);
        draw_body(ui, dialog, events);
    });
    if response.should_close() {
        events.push(AppEvent::ImportDialogCancel);
    }
}

fn draw_body(ui: &mut egui::Ui, dialog: &ImportDialog, events: &mut Vec<AppEvent>) {
    let name = dialog
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    ui.heading(format!("Import drawing \u{2014} {name}"));
    ui.add_space(8.0);

    let mut table = DataTable::new("import_dialog_size");
    match (&dialog.kind, &dialog.declared) {
        (ModelKind::Svg, Some(Ok(declared))) => {
            table = table.row(DataRow::value("File size", declared_text(declared)).hover(
                "The size the file declares. The import does not read it today: one unit of \
                 the file is 1 mm times the scale.",
            ));
        }
        (ModelKind::Svg, Some(Err(reason))) => {
            table = table.row(DataRow::note(
                Role::Caution,
                format!("File size: not read ({reason})"),
            ));
        }
        _ => {}
    }
    table.show(ui);

    let id = egui::Id::new("import_dialog");
    let mut units = dialog.units;
    let mut options = dialog.options;
    let mut edited = false;
    if let Some(chosen) = units_row(ui, id.with("units"), dialog.units) {
        units = chosen;
        edited = true;
    }

    let mut table = DataTable::new("import_dialog_result");
    match &dialog.preview {
        Ok(model) => {
            table = table.row(DataRow::value("Result", result_text(model)));
        }
        Err(reason) => {
            table = table.row(DataRow::note(
                Role::Danger,
                format!("The import failed: {reason}"),
            ));
        }
    }
    table.show(ui);

    if let Some(chosen) = join_row(ui, id.with("join"), dialog.options) {
        options = chosen;
        edited = true;
    }
    if let Ok(model) = &dialog.preview
        && let Some(report) = &model.import_report
    {
        DataTable::new("import_dialog_report")
            .row(report_row(report))
            .show(ui);
    }
    if edited {
        events.push(AppEvent::ImportDialogEdit(units, options));
    }

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button("Cancel").clicked() {
            events.push(AppEvent::ImportDialogCancel);
        }
        if ui
            .add_enabled(dialog.preview.is_ok(), egui::Button::new("Import"))
            .clicked()
        {
            events.push(AppEvent::ImportDialogConfirm);
        }
    });
}
