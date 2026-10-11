//! The status half of the pin section: the chips, the keying warning with
//! its action buttons, the pin table and the findings. Every number comes
//! from the one core report (`PinSetReport`).

use rs_cam_core::compute::alignment_pins::PIN_WALL_MM;
use rs_cam_core::session::{PinSetReport, PinStatus};

use super::{PinDrillState, PinSectionOut, edit};
use crate::state::job::StockConfig;
use crate::state::panels::KeyedPinPlan;
use crate::state::toolpath::OperationType;
use crate::ui::AppEvent;
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

/// The pin-drill chip.
#[must_use]
pub fn pin_drill_chip(state: &PinDrillState) -> Option<Chip> {
    match state {
        PinDrillState::NoPins => None,
        PinDrillState::Exists { name, holes } => Some(Chip {
            word: format!("Pin drill: '{name}', {holes} holes"),
            role: Role::Ok,
        }),
        PinDrillState::Missing => Some(Chip {
            word: "Pin drill: not created".to_owned(),
            role: Role::Caution,
        }),
        PinDrillState::Refused(_) => Some(Chip {
            word: "Pin drill: refused".to_owned(),
            role: Role::Danger,
        }),
    }
}

/// Draw the chips and, under a refused or missing pin drill, its next step.
pub(super) fn chips(
    ui: &mut egui::Ui,
    report: &PinSetReport,
    pin_drill: &PinDrillState,
    out: &mut PinSectionOut,
    events: &mut Vec<AppEvent>,
) {
    // One chip per line: a column of chips aligns (DESIGN_SPEC §4.3), and
    // a long chip never runs off the rail.
    for chip in chip_words(report)
        .into_iter()
        .chain(pin_drill_chip(pin_drill))
    {
        ui.add(StatusChip::new(chip.word, chip.role));
    }
    match pin_drill {
        PinDrillState::Refused(reason) => {
            ui.label(text::caption(*reason));
            ui.horizontal_wrapped(|ui| {
                if *reason == crate::controller::PIN_DRILL_NEEDS_TOOL {
                    if ui
                        .add(Button::new("Add a drill operation"))
                        .on_hover_text(
                            "Adds a Drill operation. Set its tool to the drill of the dowel \
                             size, then press 'Create the pin drill'.",
                        )
                        .clicked()
                    {
                        events.push(AppEvent::AddToolpath(OperationType::Drill));
                    }
                } else if ui
                    .add(Button::new("Add a setup"))
                    .on_hover_text(
                        "Adds a setup. Set its face to Top at 0 deg, then press 'Create the \
                         pin drill'.",
                    )
                    .clicked()
                {
                    events.push(AppEvent::AddSetup);
                }
                if ui.add(Button::quiet("Create the pin drill")).clicked() {
                    out.sync_pin_drill = true;
                }
            });
        }
        PinDrillState::Missing => {
            if ui.add(Button::new("Create the pin drill")).clicked() {
                out.sync_pin_drill = true;
            }
        }
        PinDrillState::NoPins | PinDrillState::Exists { .. } => {}
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
    ui.horizontal_wrapped(|ui| {
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
        ui.label(text::caption(format!("The placer refuses: {reason}")));
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
    let headers = ["Pin", "X", "Y", "\u{00D8}", "Edge", "Part"]
        .map(str::to_owned)
        .to_vec();
    DataTable::new("alignment_pin_table", headers).show(ui, |ui| {
        for (i, pin) in stock.alignment_pins.iter().enumerate() {
            let colour = crate::state::pin_view::pin_status_colour(report.pin_status(i));
            let marked = selected == Some(i);
            let number = egui::RichText::new(format!("{}", i + 1))
                .color(colour)
                .strong();
            if ui
                .selectable_label(marked, number)
                .on_hover_text(status_hover(report.pin_status(i)))
                .clicked()
            {
                out.selected_pin = Some(if marked { None } else { Some(i) });
            }
            let walls = report.walls.get(i);
            ui.label(text::numeric(format!("{:.2}", pin.x)));
            ui.label(text::numeric(format!("{:.2}", pin.y)));
            ui.label(text::numeric(format!("{:.2}", pin.diameter)));
            ui.label(text::numeric(walls.map_or("\u{2014}".to_owned(), |w| {
                format!("{:.1}", w.stock_edge_mm)
            })));
            ui.label(text::numeric(
                walls
                    .and_then(|w| w.part.as_ref())
                    .map_or("\u{2014}".to_owned(), |n| format!("{:.1}", n.wall_mm)),
            ));
            ui.end_row();
        }
    });
    ui.label(text::caption(
        "mm, stock-local (X0 Y0 at the stock's min corner). Edge and Part are the \
         walls from the pin edge.",
    ));
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
            ui.label(
                egui::RichText::new(finding)
                    .small()
                    .color(crate::ui::tokens::CAUTION),
            );
        }
    });
}
