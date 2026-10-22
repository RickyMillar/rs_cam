//! The "Drills" rows of the pin wizard (R1, R2; round 2): one row for each
//! pin size of the wizard's draft. A row names the drill of the size and
//! fixes a missing drill IN PLACE: a tool chooser of the tools of the
//! size, "Add a `<d>` mm drill" and "Add a Top setup". No row sends the
//! operator to another screen.
//!
//! A drill add and a tool choice wait in the wizard's queue ([`DrillQueue`])
//! until Apply; the pin door applies them. "Add a Top setup" applies at
//! once, through the door (`AppEvent::PinAddSetup`).

use rs_cam_core::compute::alignment_pins::same_pin_diameter;
use rs_cam_core::session::ProjectSession;

use super::{PinDrillBlock, PinDrillRow, PinDrillState};
use crate::state::job::{AlignmentPin, FaceUp};
use crate::ui::AppEvent;
use crate::ui::components::{Button, ChoiceRow, Role, StatusChip};

/// The wizard's drill queue: what Apply adds and binds.
pub struct DrillQueue<'a> {
    /// The pin sizes that Apply adds a drill for.
    pub add_drills: &'a mut Vec<f64>,
    /// The tool that Apply binds to the pin-drill operation of a size.
    pub use_tools: &'a mut Vec<(f64, rs_cam_core::ids::ToolId)>,
}

/// The words and the role of the chip of one pin size. `queued` says that
/// Apply adds a drill of the size; `chosen` names the tool the operator
/// chose in the wizard.
#[must_use]
pub fn drill_chip(row: &PinDrillRow, queued: bool, chosen: Option<&str>) -> (String, Role) {
    let size = format!("{:.2} mm", row.diameter);
    if queued {
        return (format!("{size}: Apply adds a drill"), Role::Info);
    }
    if let Some(name) = chosen {
        return (format!("{size}: {name}"), Role::Ok);
    }
    match (&row.tool, &row.block) {
        (_, Some(PinDrillBlock::NeedsTopSetup)) => (format!("{size}: no Top setup"), Role::Danger),
        (_, Some(PinDrillBlock::NeedsTool)) => (format!("{size}: no drill"), Role::Danger),
        (Some(_), None) if row.tool_mismatch() => (
            format!(
                "{size}: drilled with a {:.2} mm tool",
                row.tool_diameter.unwrap_or_default()
            ),
            Role::Danger,
        ),
        (Some((_, name)), None) => (format!("{size}: {name}"), Role::Ok),
        (None, None) => (format!("{size}: no drill"), Role::Danger),
    }
}

/// Draw one row for each pin size of `pins`.
pub fn drill_rows(
    ui: &mut egui::Ui,
    session: &ProjectSession,
    pins: &[AlignmentPin],
    queue: &mut DrillQueue<'_>,
    events: &mut Vec<AppEvent>,
) {
    let state = PinDrillState::of(session, pins);
    for row in state.rows() {
        ui.push_id(("pin_drill_row", row.diameter.to_bits()), |ui| {
            drill_row(ui, row, queue, events);
        });
    }
    // A queued add or choice of a size the draft no longer has is dropped.
    let live: Vec<f64> = state.rows().iter().map(|r| r.diameter).collect();
    let alive = |d: f64| live.iter().any(|l| same_pin_diameter(*l, d));
    queue.add_drills.retain(|d| alive(*d));
    queue.use_tools.retain(|(d, _)| alive(*d));
}

fn drill_row(
    ui: &mut egui::Ui,
    row: &PinDrillRow,
    queue: &mut DrillQueue<'_>,
    events: &mut Vec<AppEvent>,
) {
    let d = row.diameter;
    let queued = queue
        .add_drills
        .iter()
        .position(|a| same_pin_diameter(*a, d));
    let chosen = queue
        .use_tools
        .iter()
        .find(|(s, _)| same_pin_diameter(*s, d))
        .map(|(_, id)| *id);
    let chosen_name = chosen.and_then(|id| {
        row.candidates
            .iter()
            .find(|(c, _)| *c == id)
            .map(|(_, n)| n.as_str())
    });
    ui.horizontal_wrapped(|ui| {
        let (word, role) = drill_chip(row, queued.is_some(), chosen_name);
        ui.add(StatusChip::new(word, role));
        if let Some(at) = queued {
            if ui.add(Button::quiet("Do not add")).clicked() {
                queue.add_drills.remove(at);
            }
            return;
        }
        if matches!(row.block, Some(PinDrillBlock::NeedsTopSetup)) {
            if ui
                .add(Button::new("Add a Top setup"))
                .on_hover_text("Adds it now. Cancel does not remove it.")
                .clicked()
            {
                events.push(AppEvent::PinAddSetup(FaceUp::Top));
            }
            return;
        }
        if row.candidates.is_empty()
            && ui
                .add(Button::primary(format!("Add a {d:.2} mm drill")))
                .on_hover_text("Apply adds a copy of the project's drill at this size.")
                .clicked()
        {
            queue.add_drills.push(d);
        }
    });
    let mut pick: Option<rs_cam_core::ids::ToolId> = None;
    if row.tool_mismatch() && chosen.is_none() {
        // The operation drills with a tool of another size: one button
        // for each tool of the size.
        ui.horizontal_wrapped(|ui| {
            for (id, name) in &row.candidates {
                if ui.add(Button::quiet(format!("Use {name}"))).clicked() {
                    pick = Some(*id);
                }
            }
        });
    } else if row.candidates.len() > 1 {
        // The tool chooser: more than one tool of the size.
        let options: Vec<(rs_cam_core::ids::ToolId, &str)> = row
            .candidates
            .iter()
            .map(|(id, name)| (*id, name.as_str()))
            .collect();
        let current = chosen
            .or_else(|| row.tool.as_ref().map(|(id, _)| *id))
            .filter(|id| options.iter().any(|(o, _)| o == id))
            .or_else(|| options.first().map(|(id, _)| *id));
        if let Some(mut value) = current {
            let before = value;
            ui.add(
                ChoiceRow::new("Drill", &mut value, &options).hover("The tools of this pin size."),
            );
            if value != before {
                pick = Some(value);
            }
        }
    }
    if let Some(id) = pick {
        queue.use_tools.retain(|(s, _)| !same_pin_diameter(*s, d));
        queue.use_tools.push((d, id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> PinDrillRow {
        PinDrillRow {
            diameter: 8.0,
            tool: None,
            tool_diameter: None,
            candidates: Vec::new(),
            block: Some(PinDrillBlock::NeedsTool),
        }
    }

    #[test]
    fn a_size_with_no_drill_says_so_in_place() {
        assert_eq!(
            drill_chip(&row(), false, None),
            ("8.00 mm: no drill".to_owned(), Role::Danger)
        );
        assert_eq!(drill_chip(&row(), true, None).1, Role::Info);
    }

    #[test]
    fn a_tool_of_another_size_is_danger() {
        let mut r = row();
        r.tool = Some((
            rs_cam_core::ids::ToolId::from_raw(3),
            "6 mm drill".to_owned(),
        ));
        r.tool_diameter = Some(6.0);
        r.block = None;
        let (word, role) = drill_chip(&r, false, None);
        assert!(word.contains("6.00 mm tool"), "{word}");
        assert_eq!(role, Role::Danger);
    }
}
