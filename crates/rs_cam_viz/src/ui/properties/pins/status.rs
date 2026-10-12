//! The status half of the pin section: the chips, the keying warning with
//! its action buttons, the pin table and the findings. Every number comes
//! from the one core report (`PinSetReport`).

use rs_cam_core::compute::alignment_pins::PIN_WALL_MM;
use rs_cam_core::session::{PinSetReport, PinStatus};

use super::{PinSectionOut, edit};
use crate::state::job::StockConfig;
use crate::state::panels::KeyedPinPlan;
use crate::ui::components::{Banner, Button, DataTable, Role, StatusChip, UiExt as _, text};
use crate::ui::properties::PanelEdit;

/// One chip: its words and its role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    pub word: String,
    pub role: Role,
}

/// The chips of a report, in order: seating, keying, walls.
///
/// A pure function of the report, so a test reads the words the panel
/// paints.
#[must_use]
pub fn chip_words(report: &PinSetReport) -> Vec<Chip> {
    let flip = &report.flip;
    let mut chips = Vec::new();
    chips.push(if !flip.modelled_flip() {
        Chip {
            word: "Seats after flip: no flip setup".to_owned(),
            role: Role::Unknown,
        }
    } else if flip.seats() {
        Chip {
            word: "Seats after flip: yes".to_owned(),
            role: Role::Ok,
        }
    } else {
        Chip {
            word: format!(
                "Seats after flip: misses by {:.1} mm",
                flip.seat_mismatch_mm.unwrap_or(f64::INFINITY)
            ),
            role: Role::Danger,
        }
    });
    chips.push(if flip.keyed() {
        Chip {
            word: match flip.keying_margin_mm() {
                Some(margin) => format!("Keyed: yes, margin {margin:.1} mm"),
                None => "Keyed: yes, no turn seats".to_owned(),
            },
            role: Role::Ok,
        }
    } else {
        let turn = flip.wrong_seating.map_or(180.0, |w| w.turn_deg.abs());
        Chip {
            word: format!("Keyed: NO: seats turned {turn:.0} deg"),
            role: Role::Danger,
        }
    });
    if let Some(min) = report.min_wall_mm() {
        let role = if min < 0.0 {
            Role::Danger
        } else if min < PIN_WALL_MM {
            Role::Caution
        } else {
            Role::Ok
        };
        chips.push(Chip {
            word: format!("Walls: min {min:.1} mm (need {PIN_WALL_MM:.1})"),
            role,
        });
    }
    chips
}

/// Draw the chips of the report. The pin-drill state of each pin size is
/// the "Drills" block (`drills.rs`).
pub(super) fn chips(ui: &mut egui::Ui, report: &PinSetReport) {
    // One chip per line: a column of chips aligns (DESIGN_SPEC §4.3), and
    // a long chip never runs off the rail.
    for chip in chip_words(report) {
        ui.add(StatusChip::new(chip.word, chip.role));
    }
}

/// The keying warning, with the actions that fix it.
pub(super) fn keying_warning(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    report: &PinSetReport,
    plan: &KeyedPinPlan,
    edit: &mut PanelEdit,
) {
    if report.flip.keyed() {
        return;
    }
    ui.add_space(crate::ui::tokens::SPACE_2);
    Banner::new(
        Role::Danger,
        "The part can go on the dowels the wrong way round. The red dashed \
         rings in the viewport show where the holes then land.",
    )
    .show(ui);
    // R5b: the two buttons stack, one per line. In a wrapped row at the
    // inspector width the second button ran past the panel edge.
    ui.vertical(|ui| {
        let use_preset = ui
            .add(Button::primary("Use 3-pin preset").enabled(plan.is_ok()))
            .on_hover_text(
                "Replaces the pins with the placer's three: one on the flip axis and a mirror \
                 pair off it, in the clear material.",
            );
        if use_preset.clicked()
            && let Ok(pins) = plan
        {
            stock.alignment_pins = pins.to_vec();
            edit.commit();
        }
        // "Move the pair off the axis" applies to two equal pins on the
        // axis. The distance off the axis is the placer's, not a guess.
        let off_axis = plan.as_ref().ok().and_then(|plan_pins| {
            let fit = rs_cam_core::compute::alignment_pins::fit_axis_plus_mirror_pair(
                plan_pins,
                &report.map,
            )?;
            edit::pair_off_axis(&stock.alignment_pins, &report.map, fit.pair_across)
        });
        if let Some(pins) = off_axis
            && ui
                .add(Button::new("Move the pair off the axis"))
                .on_hover_text(
                    "Keeps pin 1 on the flip axis and makes pin 2 a mirror pair off the axis, \
                     at the distance the placer finds clear.",
                )
                .clicked()
        {
            stock.alignment_pins = pins;
            edit.commit();
        }
    });
    if let Err(reason) = plan {
        super::wrapped(ui, text::caption(format!("The placer refuses: {reason}")));
    }
}

/// The read-only pin table. A click on a pin number marks the pin.
pub(super) fn pin_table(
    ui: &mut egui::Ui,
    stock: &StockConfig,
    report: &PinSetReport,
    selected: Option<usize>,
    out: &mut PinSectionOut,
) {
    ui.add_space(crate::ui::tokens::SPACE_2);
    // R5b: three columns. Six columns (Pin, X, Y, diameter, Edge, Part)
    // were wider than the 240 point rail, and the table pushed the section
    // past the panel edge. The pin hover gives the diameter (the plan view
    // and the Drills block show it too). The wall column is the thinnest
    // wall; its hover names the edge and the part walls.
    let headers = ["Pin", "X, Y", "Wall"].map(str::to_owned).to_vec();
    DataTable::new("alignment_pin_table", headers).show(ui, |ui| {
        for (i, pin) in stock.alignment_pins.iter().enumerate() {
            let colour = crate::state::pin_view::pin_status_colour(report.pin_status(i));
            let marked = selected == Some(i);
            let number = egui::RichText::new(format!("{}", i + 1))
                .color(colour)
                .strong();
            if ui
                .selectable_label(marked, number)
                .on_hover_text(format!(
                    "\u{00D8} {:.2} mm. {}",
                    pin.diameter,
                    status_hover(report.pin_status(i))
                ))
                .clicked()
            {
                out.selected_pin = Some(if marked { None } else { Some(i) });
            }
            let walls = report.walls.get(i);
            ui.label(text::numeric(format!("{:.2}, {:.2}", pin.x, pin.y)));
            let dash = || "\u{2014}".to_owned();
            let wall = ui.label(text::numeric(
                walls.map_or_else(dash, |w| format!("{:.1}", w.min_mm())),
            ));
            if let Some(w) = walls {
                let _ = wall.on_hover_text(format!(
                    "Edge {:.1} mm, part {} mm.",
                    w.stock_edge_mm,
                    w.part
                        .as_ref()
                        .map_or_else(dash, |n| format!("{:.1}", n.wall_mm)),
                ));
            }
            ui.end_row();
        }
    });
    super::wrapped(
        ui,
        text::caption(
            "mm, stock-local (X0 Y0 at the stock's min corner). Wall is the thinnest wall \
             from the pin edge.",
        ),
    );
}

fn status_hover(status: PinStatus) -> String {
    match status {
        PinStatus::Good => "Every check passes.".to_owned(),
        PinStatus::ThinWall => format!("A wall is thinner than {PIN_WALL_MM:.1} mm."),
        PinStatus::Fails => "The pin does not seat after the flip, the set does not key, or \
             the pin is not wholly inside the stock."
            .to_owned(),
    }
}

/// Every finding of the report, in a closed fold under the chips.
pub(super) fn findings(ui: &mut egui::Ui, report: &PinSetReport) {
    if report.findings.is_empty() {
        return;
    }
    let title = format!("Pin findings ({})", report.findings.len());
    ui.disclosure("alignment_pin_findings", &title, false, |ui| {
        for finding in &report.findings {
            super::wrapped(
                ui,
                egui::RichText::new(finding)
                    .small()
                    .color(crate::ui::tokens::CAUTION),
            );
        }
    });
}
