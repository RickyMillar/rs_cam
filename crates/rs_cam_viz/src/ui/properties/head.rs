//! H2 (`planning/op_architecture_2026-10-14/PLAN.md`, item H): the "Head"
//! section of the Machine panel.
//!
//! The head is the solid above the tool holder face: the fixed parts (the
//! collet nut, then the spindle shelf) and the optional dust shoe. Core owns
//! the model (`rs_cam_core::machine::head`) and its rules
//! (`MachineHead::validate`).
//!
//! The section edits a DRAFT held in `state.panels.head_editor`. A preset
//! button applies at once. Apply writes the draft through
//! `panel_apply::apply_head` (`Command::SetMachineHead`), and a refusal
//! comes back into the section in the core's words (the S4 contract).

use rs_cam_core::machine::domain::HeadDimension;
use rs_cam_core::machine::head::{
    DustShoe, HeadPart, MachineHead, PartKind, ShoeTravel, UNMODELLED_HEAD_DIAMETER_MM,
};

use crate::state::panels::HeadEditor;
use crate::ui::components::{
    Banner, Button, ChoiceRow, DataRow, DataTable, Role, UiExt as _, ValueRow,
};
use crate::ui::tokens;

/// The two part lists of a head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartList {
    /// The fixed parts, from the holder face up.
    Fixed,
    /// The dust shoe parts, from the brush end up.
    Shoe,
}

/// What the operator asked the section to do this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadAction {
    /// Nothing, or an edit of the draft.
    None,
    /// Apply the preset with this name at once.
    Preset(&'static str),
    /// Write the draft to the machine.
    Apply,
    /// Put the draft back to the stored head.
    Cancel,
}

/// The width choice of a part: a diameter, or a shelf (no width limit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Width {
    Diameter,
    Shelf,
}

const WIDTH_OPTIONS: &[(Width, &str)] = &[(Width::Diameter, "Diameter"), (Width::Shelf, "Shelf")];

const KIND_OPTIONS: &[(PartKind, &str)] = &[(PartKind::Hard, "Hard"), (PartKind::Soft, "Soft")];

/// The hover of the section header.
pub const SECTION_HOVER: &str = "The solid above the tool holder face: the collet nut, the \
     spindle and the dust shoe. Hard parts block a move in every check: the obstacle \
     step, the simulation and the collision check. Soft parts (a brush) may touch the \
     stock and the clamps, so they never block.";

const PRESET_HOVER: &str = "Replace the head with this preset. Undo puts the old head back.";

const FIXED_HOVER: &str = "The fixed parts, from the holder face up. The holder face is at \
     the tool stickout above the tip, so the parts move with the stickout. A change of \
     these parts drops every toolpath result.";

const SHOE_HOVER: &str = "The dust shoe parts, from the brush end up. The shoe moves to fit \
     the toolpath; the toolpath does not move to fit the shoe. A shoe change drops only \
     the simulation.";

const WIDTH_HOVER: &str = "Diameter: a band of this width.\nShelf: no width limit, for \
     example the spindle body. A shelf is the last part of its list.";

const KIND_HOVER: &str = "Hard: a rigid part. It blocks a move.\nSoft: a flexible part, for \
     example a brush. It may touch the stock and the clamps, so it never blocks.";

const HEIGHT_HOVER: &str = "The height of this part. It places the next part above it.";

const MOUNT_HOVER: &str = "The distance from the holder face up to the bottom of the first \
     fixed part.";

const GAP_HOVER: &str = "The gap from the bit end up to the brush end that you set for \
     normal work. The shoe solver gives no gap below it.";

const TRAVEL_HOVER: &str = "The travel range of the shoe mount: the lowest and the highest \
     brush-end position, in mm from the holder face, positive up. Off: the range is not \
     known, and the solver applies no upper limit.";

/// The text of one part row: the width, the height and the kind.
#[must_use]
pub fn part_summary(part: &HeadPart) -> String {
    let kind = match part.kind {
        PartKind::Hard => "hard",
        PartKind::Soft => "soft",
    };
    match part.diameter_mm {
        Some(d) => format!(
            "\u{00D8} {d:.1} \u{00B7} {:.1} tall \u{00B7} {kind}",
            part.height_mm
        ),
        None => format!("shelf \u{00B7} {kind}"),
    }
}

/// The diameter a new part, or a shelf that becomes a band, takes: the
/// nearest part with a width, else the unmodelled holder diameter.
fn seed_diameter(parts: &[HeadPart]) -> f64 {
    parts
        .iter()
        .rev()
        .find_map(|part| part.diameter_mm)
        .unwrap_or(UNMODELLED_HEAD_DIAMETER_MM)
}

/// Add a hard part with no height to `parts`, below a trailing shelf (a
/// shelf is the last part). Returns the index of the new part.
pub fn add_part(parts: &mut Vec<HeadPart>) -> usize {
    let at = if parts.last().is_some_and(HeadPart::is_shelf) {
        parts.len() - 1
    } else {
        parts.len()
    };
    let part = HeadPart {
        name: format!("Part {}", parts.len() + 1),
        diameter_mm: Some(seed_diameter(parts)),
        height_mm: 0.0,
        kind: PartKind::Hard,
    };
    parts.insert(at, part);
    at
}

/// Make part `index` a shelf, and move it to the end of `parts` (a shelf
/// is the last part). Returns the new index.
pub fn make_shelf(parts: &mut Vec<HeadPart>, index: usize) -> usize {
    if index >= parts.len() {
        return index;
    }
    let mut part = parts.remove(index);
    part.diameter_mm = None;
    parts.push(part);
    parts.len() - 1
}

/// Draw the body of the "Head" section over the draft in `editor`. `open`
/// is the part whose rows show. The caller draws the section header and
/// applies on the returned [`HeadAction`].
pub fn draw_head(
    ui: &mut egui::Ui,
    stored: &MachineHead,
    editor: &mut HeadEditor,
    open: &mut Option<(PartList, usize)>,
) -> HeadAction {
    let mut action = HeadAction::None;
    let before = editor.draft.clone();
    ui.horizontal_wrapped(|ui| {
        ui.label("Preset:");
        for name in MachineHead::PRESETS {
            if ui
                .add(Button::new(name))
                .on_hover_text(PRESET_HOVER)
                .clicked()
            {
                action = HeadAction::Preset(name);
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.add(egui::TextEdit::singleline(&mut editor.draft.name).desired_width(f32::INFINITY));
    });
    ui.param_grid("machine_head_mount", |ui| {
        // F210: each drag range in this editor is `HeadDimension::ui_range`,
        // a GUI convenience. `MachineHead::validate` reads the domain.
        let _ = ValueRow::new(
            "Mount",
            &mut editor.draft.mount_mm,
            " mm",
            0.5,
            HeadDimension::Mount.ui_range(),
        )
        .keep_existing()
        .tooltip(Some(MOUNT_HOVER))
        .show(ui);
    });

    ui.label(
        egui::RichText::new("Fixed parts, from the holder face up")
            .small()
            .color(tokens::TEXT_MUTED),
    )
    .on_hover_text(FIXED_HOVER);
    draw_part_list(ui, PartList::Fixed, &mut editor.draft.parts, open);

    ui.add_space(4.0);
    match editor.draft.shoe.as_mut() {
        None => {
            if ui
                .small_button("+ Add dust shoe")
                .on_hover_text(SHOE_HOVER)
                .clicked()
            {
                editor.draft.shoe = MachineHead::dust_shoe().shoe;
            }
        }
        Some(shoe) => {
            let remove = draw_shoe(ui, shoe, open);
            if remove {
                editor.draft.shoe = None;
                if matches!(open, Some((PartList::Shoe, _))) {
                    *open = None;
                }
            }
        }
    }

    let dirty = editor.draft != *stored;
    if editor.draft != before {
        editor.refusal = None;
    }
    if let Some(refusal) = &editor.refusal {
        let _ = Banner::new(Role::Danger, refusal.clone()).show(ui);
    }
    if dirty {
        ui.horizontal(|ui| {
            if ui
                .add(Button::primary("Apply head"))
                .on_hover_text("Write this head to the machine. Undo puts the old head back.")
                .clicked()
            {
                action = HeadAction::Apply;
            }
            if ui
                .add(Button::quiet("Cancel"))
                .on_hover_text("Put the section back to the head that the machine holds.")
                .clicked()
            {
                action = HeadAction::Cancel;
            }
        });
    }
    action
}

/// Draw the dust shoe block. Returns true when the operator removed it.
fn draw_shoe(ui: &mut egui::Ui, shoe: &mut DustShoe, open: &mut Option<(PartList, usize)>) -> bool {
    let mut remove = false;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Dust shoe, from the brush end up")
                .small()
                .color(tokens::TEXT_MUTED),
        )
        .on_hover_text(SHOE_HOVER);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(Button::quiet("Remove"))
                .on_hover_text("Remove the dust shoe from the machine head.")
                .clicked()
            {
                remove = true;
            }
        });
    });
    draw_part_list(ui, PartList::Shoe, &mut shoe.parts, open);
    ui.param_grid("machine_head_shoe", |ui| {
        let _ = ValueRow::new(
            "Normal gap",
            &mut shoe.normal_gap_mm,
            " mm",
            0.5,
            HeadDimension::NormalGap.ui_range(),
        )
        .keep_existing()
        .tooltip(Some(GAP_HOVER))
        .show(ui);
    });
    let mut limited = shoe.travel_mm.is_some();
    if ui
        .checkbox(&mut limited, "Travel limit")
        .on_hover_text(TRAVEL_HOVER)
        .changed()
    {
        shoe.travel_mm = limited.then_some(ShoeTravel {
            min_mm: 0.0,
            max_mm: 0.0,
        });
    }
    if let Some(travel) = shoe.travel_mm.as_mut() {
        ui.param_grid("machine_head_travel", |ui| {
            let _ = ValueRow::new(
                "Travel min",
                &mut travel.min_mm,
                " mm",
                0.5,
                HeadDimension::TravelEnd.ui_range(),
            )
            .keep_existing()
            .tooltip(Some(TRAVEL_HOVER))
            .show(ui);
            let _ = ValueRow::new(
                "Travel max",
                &mut travel.max_mm,
                " mm",
                0.5,
                HeadDimension::TravelEnd.ui_range(),
            )
            .keep_existing()
            .tooltip(Some(TRAVEL_HOVER))
            .show(ui);
        });
    }
    remove
}

/// Draw one part list as a table, the open part's rows, and the add
/// button.
fn draw_part_list(
    ui: &mut egui::Ui,
    list: PartList,
    parts: &mut Vec<HeadPart>,
    open: &mut Option<(PartList, usize)>,
) {
    let rows = parts.iter().map(|part| {
        let name = if part.name.is_empty() {
            "(no name)".to_owned()
        } else {
            part.name.clone()
        };
        DataRow::value(name, part_summary(part))
            .hover("Click to edit this part.")
            .clickable()
    });
    let response = DataTable::new(("machine_head_parts", list == PartList::Shoe))
        .rows(rows)
        .show(ui);
    if let Some(index) = response.clicked {
        *open = if *open == Some((list, index)) {
            None
        } else {
            Some((list, index))
        };
    }
    if let Some((open_list, index)) = *open
        && open_list == list
        && index < parts.len()
    {
        let next = draw_part_rows(ui, list, parts, index);
        *open = next.map(|index| (list, index));
    }
    if ui
        .small_button("+ Add part")
        .on_hover_text("Add a hard part below the shelf of this list.")
        .clicked()
    {
        let index = add_part(parts);
        *open = Some((list, index));
    }
}

/// Draw the rows of part `index`. Returns the part's index after the edit,
/// or `None` when the operator removed it.
fn draw_part_rows(
    ui: &mut egui::Ui,
    list: PartList,
    parts: &mut Vec<HeadPart>,
    index: usize,
) -> Option<usize> {
    let mut at = index;
    let mut removed = false;
    let seed = seed_diameter(parts);
    let part = parts.get_mut(index)?;
    ui.indent(("machine_head_part", list == PartList::Shoe, index), |ui| {
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.add(egui::TextEdit::singleline(&mut part.name).desired_width(f32::INFINITY));
        });
        let mut width = if part.is_shelf() {
            Width::Shelf
        } else {
            Width::Diameter
        };
        let width_changed = ui
            .add(ChoiceRow::new("Width", &mut width, WIDTH_OPTIONS).hover(WIDTH_HOVER))
            .changed();
        if width_changed && width == Width::Diameter {
            part.diameter_mm = Some(seed);
        }
        if let Some(d) = part.diameter_mm.as_mut() {
            ui.param_grid(
                ("machine_head_part_d", list == PartList::Shoe, index),
                |ui| {
                    let _ = ValueRow::new(
                        "Diameter",
                        d,
                        " mm",
                        0.5,
                        HeadDimension::PartDiameter.ui_range(),
                    )
                    .keep_existing()
                    .show(ui);
                    let _ = ValueRow::new(
                        "Height",
                        &mut part.height_mm,
                        " mm",
                        0.5,
                        HeadDimension::PartHeight.ui_range(),
                    )
                    .keep_existing()
                    .tooltip(Some(HEIGHT_HOVER))
                    .show(ui);
                },
            );
        }
        let _ = ui.add(ChoiceRow::new("Kind", &mut part.kind, KIND_OPTIONS).hover(KIND_HOVER));
        if ui
            .add(Button::danger("Remove part"))
            .on_hover_text("Remove this part from the list.")
            .clicked()
        {
            removed = true;
        }
        if width_changed && width == Width::Shelf {
            at = usize::MAX;
        }
    });
    if removed {
        parts.remove(index);
        return None;
    }
    if at == usize::MAX {
        at = make_shelf(parts, index);
    }
    Some(at)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code; a failed lookup is a failed test."
)]
mod tests {
    use super::*;

    #[test]
    fn a_new_part_goes_below_the_shelf_and_a_shelf_goes_last_h2() {
        let mut head = MachineHead::bare_collet();
        let at = add_part(&mut head.parts);
        assert_eq!(at, 1, "the new part sits below the spindle shelf");
        assert_eq!(
            head.parts[1].diameter_mm,
            Some(20.0),
            "it copies the collet width"
        );
        assert!(head.parts.last().unwrap().is_shelf());
        head.validate().unwrap();

        // The collet nut becomes a shelf: it moves to the end, and the old
        // shelf is now above a shelf, which core refuses in its own words.
        let moved = make_shelf(&mut head.parts, 0);
        assert_eq!(moved, 2);
        assert!(head.parts[2].is_shelf());
        let refusal = head.validate().unwrap_err().to_string();
        assert!(refusal.contains("stands above a shelf"), "{refusal}");
    }

    #[test]
    fn the_part_summary_names_the_width_height_and_kind_h2() {
        let head = MachineHead::dust_shoe();
        assert_eq!(
            part_summary(&head.parts[0]),
            "\u{00D8} 20.0 \u{00B7} 30.0 tall \u{00B7} hard"
        );
        assert_eq!(part_summary(&head.parts[1]), "shelf \u{00B7} hard");
        let shoe = head.shoe.unwrap();
        assert_eq!(
            part_summary(&shoe.parts[0]),
            "\u{00D8} 80.0 \u{00B7} 25.0 tall \u{00B7} soft"
        );
    }
}
