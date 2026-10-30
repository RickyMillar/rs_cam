//! The vendor cutting data window (W3i).
//!
//! The Feeds tab held the vendor table as an inline expander of one-line
//! rows. W3i (operator design, 2026-10-30) moved it into a large window: a
//! real table that scrolls both ways, with a fixed header row. The rows of
//! the current tool's family show; a row at the current diameter (within
//! 0.1 mm) is in the OK tone. The Feeds tab's `Vendor data…` button and MCP
//! `set_ui_view modal: vendor_data` open it.

use super::shared::{material_family_label, tool_family_label, toolpath_name};
use crate::state::AppState;
use crate::ui::{AppEvent, theme, tokens};
use crate::ui_command::{NoArgs, UiCommand};

/// The window's own identity, independent of its title (the title carries
/// the toolpath name, and egui derives the area id from the title).
const WINDOW_ID: &str = "feeds_vendor_data_window";

/// Air left between the window frame and each screen edge.
const SCREEN_MARGIN: f32 = 48.0;

/// The share of the screen the window takes when it opens: "most of the
/// window" (operator, 2026-10-30).
const SCREEN_SHARE: f32 = 0.8;

/// The columns: the header text and the width of each.
const COLUMNS: [(&str, f32); 9] = [
    ("Material", 150.0),
    ("Ø mm", 56.0),
    ("Flutes", 52.0),
    ("RPM", 104.0),
    ("Chipload mm/tooth", 132.0),
    ("DOC mm", 72.0),
    ("Operation", 120.0),
    ("Grade", 104.0),
    ("Source", 260.0),
];

/// The height of one table row.
const ROW_HEIGHT: f32 = 20.0;

/// Map `ToolType` to the vendor LUT `ToolFamily` for filtering.
pub(crate) fn tool_type_to_lut_family(
    tt: crate::state::job::ToolType,
) -> rs_cam_core::feeds::vendor_lut::ToolFamily {
    use rs_cam_core::feeds::vendor_lut::ToolFamily;
    match tt {
        crate::state::job::ToolType::EndMill => ToolFamily::FlatEnd,
        crate::state::job::ToolType::BallNose => ToolFamily::BallNose,
        crate::state::job::ToolType::BullNose => ToolFamily::BullNose,
        crate::state::job::ToolType::VBit => ToolFamily::ChamferVbit,
        crate::state::job::ToolType::TaperedBallNose => ToolFamily::TaperedBallNose,
    }
}

/// Human-readable label for an `EvidenceGrade`.
fn evidence_grade_label(g: rs_cam_core::feeds::vendor_lut::EvidenceGrade) -> &'static str {
    use rs_cam_core::feeds::vendor_lut::EvidenceGrade;
    match g {
        EvidenceGrade::A => "A (vendor)",
        EvidenceGrade::B => "B (derived)",
        EvidenceGrade::C => "C (community)",
    }
}

/// The range text of two optional bounds at `decimals`, "-" for none.
fn range_text(lo: Option<f64>, hi: Option<f64>, decimals: usize) -> String {
    match (lo, hi) {
        (Some(lo), Some(hi)) => format!("{lo:.decimals$}-{hi:.decimals$}"),
        (Some(v), None) | (None, Some(v)) => format!("{v:.decimals$}"),
        (None, None) => "-".to_owned(),
    }
}

/// The cells of one observation, in [`COLUMNS`] order.
fn row_cells(obs: &rs_cam_core::feeds::vendor_lut::VendorObservation) -> [String; 9] {
    let diameter = obs
        .diameter_mm
        .map_or_else(|| "\u{2014}".to_owned(), |d| format!("{d:.1}"));
    let rpm = match (obs.rpm_min, obs.rpm_max, obs.rpm_nominal) {
        (None, None, Some(nom)) => format!("{nom:.0}"),
        (lo, hi, _) => range_text(lo, hi, 0),
    };
    [
        format!(
            "{} \u{00B7} {}",
            material_family_label(obs.material_family),
            obs.material_label
        ),
        diameter,
        obs.flute_count.to_string(),
        rpm,
        range_text(obs.chipload_min_mm_tooth, obs.chipload_max_mm_tooth, 4),
        range_text(obs.ap_min_mm, obs.ap_max_mm, 1),
        format!("{:?} {:?}", obs.operation_family, obs.pass_role),
        evidence_grade_label(obs.evidence_grade).to_owned(),
        format!("{} \u{00B7} {}", obs.source_vendor, obs.observation_id),
    ]
}

/// One table row of `cells`. The full text of a cell is its hover.
fn draw_row(ui: &mut egui::Ui, cells: &[String], color: egui::Color32, strong: bool) {
    ui.horizontal(|ui| {
        for ((_, width), text) in COLUMNS.iter().zip(cells) {
            let mut rich = egui::RichText::new(text).color(color);
            if strong {
                rich = rich.strong();
            }
            let _ = ui
                .add_sized(
                    [*width, ROW_HEIGHT],
                    egui::Label::new(rich).truncate().halign(egui::Align::Min),
                )
                .on_hover_text(text);
        }
    });
}

/// The vendor table of the tool of `toolpath_id`: the rows of its family.
fn draw_table(ui: &mut egui::Ui, state: &AppState, toolpath_id: rs_cam_core::ToolpathId) {
    let session = state.session();
    let tool = session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)
        .and_then(|tc| session.tools().iter().find(|t| Some(t.id) == tc.tool_id));
    let Some(tool) = tool else {
        ui.add(
            egui::Label::new(
                egui::RichText::new("This toolpath has no tool, so it has no vendor data.")
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
        return;
    };
    let family = tool_type_to_lut_family(tool.tool_type);
    let lut = rs_cam_core::feeds::embedded_vendor_lut();
    let rows: Vec<&rs_cam_core::feeds::vendor_lut::VendorObservation> = lut
        .observations
        .iter()
        .filter(|obs| obs.tool_family == family)
        .collect();
    let matches = rows
        .iter()
        .filter(|obs| {
            obs.diameter_mm
                .is_some_and(|d| (d - tool.diameter).abs() < 0.1)
        })
        .count();
    ui.add(
        egui::Label::new(
            egui::RichText::new(format!(
                "{} tools \u{00B7} {} rows \u{00B7} {matches} at the current \u{00D8}{:.1} mm \
                 (in green)",
                tool_family_label(family),
                rows.len(),
                tool.diameter,
            ))
            .color(theme::TEXT_DIM),
        )
        .wrap(),
    );
    if rows.is_empty() {
        ui.add(
            egui::Label::new(
                egui::RichText::new("No vendor data for this tool type.").color(tokens::CAUTION),
            )
            .wrap(),
        );
        return;
    }
    ui.add_space(tokens::SPACE_2);
    // The header row stays in place while the body scrolls down; both
    // scroll sideways together.
    egui::ScrollArea::horizontal()
        .id_salt("vendor_data_h")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let header: Vec<String> = COLUMNS.iter().map(|(h, _)| (*h).to_owned()).collect();
            draw_row(ui, &header, theme::TEXT_STRONG, true);
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("vendor_data_v")
                .auto_shrink([false, false])
                .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
                    for obs in rows.iter().skip(range.start).take(range.len()) {
                        let at_diameter = obs
                            .diameter_mm
                            .is_some_and(|d| (d - tool.diameter).abs() < 0.1);
                        let color = if at_diameter {
                            tokens::OK
                        } else {
                            theme::TEXT_DIM
                        };
                        draw_row(ui, &row_cells(obs), color, at_diameter);
                    }
                });
        });
}

/// Draw the window. Nothing draws while it is closed.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    let Some(open) = state.vendor_data else {
        return;
    };
    let Some(name) = toolpath_name(state, open.toolpath_id) else {
        // The toolpath is gone (the project changed under an open window).
        events.push(AppEvent::Ui(UiCommand::CloseVendorData(NoArgs)));
        return;
    };
    let screen = ctx.content_rect();
    let max = egui::vec2(
        (screen.width() - SCREEN_MARGIN).max(320.0),
        (screen.height() - SCREEN_MARGIN).max(240.0),
    );
    let mut still_open = true;
    egui::Window::new(format!("Vendor cutting data \u{2014} {name}"))
        .id(egui::Id::new(WINDOW_ID))
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_size(egui::vec2(
            screen.width() * SCREEN_SHARE,
            screen.height() * SCREEN_SHARE,
        ))
        .max_size(max)
        .open(&mut still_open)
        .show(ctx, |ui| draw_table(ui, state, open.toolpath_id));
    if !still_open {
        events.push(AppEvent::Ui(UiCommand::CloseVendorData(NoArgs)));
    }
}
