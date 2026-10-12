//! The "Drills" block of the pin section (R1, R2, 2026-10-12): one row for
//! each pin size. A row says which operation and which tool drill the
//! holes of the size, and it fixes a missing drill IN PLACE: a tool
//! chooser of the tools of the size, "Add a <d> mm drill" and "Add a Top
//! setup". No row sends the operator to another screen.
//!
//! Every action goes to the one pin door (`controller/events/pins.rs`)
//! as an `AppEvent`, so the GUI and the MCP drill the same holes.

use super::{PinDrillBlock, PinDrillRow, PinDrillState, PinSectionOut};
use crate::state::job::FaceUp;
use crate::ui::AppEvent;
use crate::ui::components::{Button, ChoiceRow, Role, StatusChip, text};

/// The words and the role of the chip of one pin size.
#[must_use]
pub fn drill_chip(row: &PinDrillRow) -> (String, Role) {
    let size = format!("{:.2} mm pins ({})", row.diameter, row.pin_count);
    match (&row.op, &row.block) {
        (Some((name, _)), _) if row.tool_mismatch() => (
            format!(
                "{size}: '{name}' drills them with a {:.2} mm tool",
                row.tool_diameter.unwrap_or_default()
            ),
            Role::Danger,
        ),
        (Some((name, holes)), _) => (format!("{size}: '{name}', {holes} holes"), Role::Ok),
        (None, Some(PinDrillBlock::NeedsTool(_))) => (format!("{size}: no drill"), Role::Danger),
        (None, Some(PinDrillBlock::NeedsTopSetup(_))) => {
            (format!("{size}: no Top setup for the drill"), Role::Danger)
        }
        (None, None) => (format!("{size}: drill not created"), Role::Caution),
    }
}

/// A short reason under a row whose operation the door cannot create. The
/// door's own text names the pin section; here the operator is in it.
fn block_caption(row: &PinDrillRow) -> Option<String> {
    match row.block.as_ref()? {
        PinDrillBlock::NeedsTool(_) => Some(format!(
            "No tool of {:.2} mm is in the project. A hole is the size of its drill.",
            row.diameter
        )),
        PinDrillBlock::NeedsTopSetup(_) => Some(
            "No setup has face Top at 0 deg. The pin drill drills from the top of the stock."
                .to_owned(),
        ),
    }
}

/// Draw the "Drills" block: one row for each pin size.
pub(super) fn drill_rows(
    ui: &mut egui::Ui,
    state: &PinDrillState,
    out: &mut PinSectionOut,
    events: &mut Vec<AppEvent>,
) {
    let rows = state.rows();
    if rows.is_empty() {
        return;
    }
    ui.add_space(crate::ui::tokens::SPACE_2);
    super::wrapped(
        ui,
        text::caption("Drills: one pin-drill operation for each pin size."),
    );
    for row in rows {
        ui.push_id(("pin_drill_row", row.diameter.to_bits()), |ui| {
            drill_row(ui, row, out, events);
        });
    }
}

fn drill_row(
    ui: &mut egui::Ui,
    row: &PinDrillRow,
    out: &mut PinSectionOut,
    events: &mut Vec<AppEvent>,
) {
    let (word, role) = drill_chip(row);
    ui.add(StatusChip::new(word, role));
    if let Some(caption) = block_caption(row) {
        super::wrapped(ui, text::caption(caption));
    }

    // The tool chooser: every tool of the size. A row with one matching
    // tool names it; a row with more, or with a tool of another size,
    // offers the choice.
    let current = row.tool.as_ref().map(|(id, _)| *id);
    if row.candidates.len() > 1 || (row.tool_mismatch() && !row.candidates.is_empty()) {
        let options: Vec<(usize, &str)> = row
            .candidates
            .iter()
            .map(|(id, name)| (*id, name.as_str()))
            .collect();
        let mut chosen = current.unwrap_or(options.first().map_or(0, |(id, _)| *id));
        let before = chosen;
        ui.add(ChoiceRow::new("Drill", &mut chosen, &options).hover(
            "The tools of this pin size. The pin-drill operation of the size drills the \
             holes with the chosen tool.",
        ));
        if chosen != before {
            events.push(AppEvent::PinDrillUseTool {
                diameter: row.diameter,
                tool_id: chosen,
            });
        }
    } else if let Some((_, name)) = &row.tool {
        super::wrapped(ui, text::caption(format!("Drill: {name}")));
    }

    ui.horizontal_wrapped(|ui| {
        let no_tool_of_size = row.candidates.is_empty();
        if (no_tool_of_size || row.tool_mismatch())
            && ui
                .add(Button::primary(format!(
                    "Add a {:.2} mm drill",
                    row.diameter
                )))
                .on_hover_text(
                    "Adds a drill of this pin size to the tools (a copy of the project's drill \
                     with the new diameter) and makes the pin-drill operation of the size with \
                     it.",
                )
                .clicked()
        {
            events.push(AppEvent::PinDrillAddTool {
                diameter: row.diameter,
            });
        }
        if matches!(row.block, Some(PinDrillBlock::NeedsTopSetup(_)))
            && ui
                .add(Button::new("Add a Top setup"))
                .on_hover_text(
                    "Adds a setup with face Top at 0 deg. The pin drill goes in it, and the \
                     selection stays here.",
                )
                .clicked()
        {
            events.push(AppEvent::PinAddSetup(FaceUp::Top));
        }
        if row.op.is_none()
            && row.block.is_none()
            && ui.add(Button::new("Create the pin drill")).clicked()
        {
            out.sync_pin_drill = true;
        }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn row() -> PinDrillRow {
        PinDrillRow {
            diameter: 8.0,
            pin_count: 1,
            op: None,
            tool: None,
            tool_diameter: None,
            candidates: Vec::new(),
            block: Some(PinDrillBlock::NeedsTool("t".to_owned())),
        }
    }

    #[test]
    fn a_size_with_no_drill_says_so_in_place() {
        let (word, role) = drill_chip(&row());
        assert_eq!(word, "8.00 mm pins (1): no drill");
        assert_eq!(role, Role::Danger);
        let caption = block_caption(&row()).unwrap();
        assert!(caption.contains("8.00 mm"), "{caption}");
    }

    #[test]
    fn a_tool_of_another_size_is_danger() {
        let mut r = row();
        r.op = Some(("Pin Drill".to_owned(), 1));
        r.tool = Some((3, "6 mm drill".to_owned()));
        r.tool_diameter = Some(6.0);
        r.block = None;
        let (word, role) = drill_chip(&r);
        assert!(word.contains("6.00 mm tool"), "{word}");
        assert_eq!(role, Role::Danger);
    }
}
