//! The edit half of the pin section: the preset picker, the pattern and
//! set controls, Auto-place, the part-model chooser and the single-pin
//! fold.
//!
//! A drag writes the draft every frame and commits once, on release
//! (`PanelEdit::drag`). A button commits on its click. So one finished edit
//! is one `SetAlignmentPins` and one undo step.

use rs_cam_core::compute::alignment_pins::{PIN_MATCH_TOL_MM, PinSetupMap, set_diameter};
use rs_cam_core::session::{PinPreset, PinSetReport};

use super::edit::{self, PresetChoice};
use super::{KeyedPinInputs, PartModelRow, PinSectionInputs, PinSectionOut};
use crate::state::job::{AlignmentPin, StockConfig};
use crate::state::panels::KeyedPinPlan;
use crate::ui::components::{Button, ChoiceRow, UiExt as _, ValueRow, text};
use crate::ui::properties::PanelEdit;

/// The nudge steps of "Move all", in mm.
const NUDGE_STEPS_MM: [f64; 4] = [-1.0, -0.5, 0.5, 1.0];

/// The preset picker. It shows the preset the pins fit; a click on another
/// preset writes that preset's pins.
pub(super) fn preset_picker(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    inputs: &PinSectionInputs<'_>,
    plan: &KeyedPinPlan,
    out: &mut PinSectionOut,
) {
    let report = inputs.report;
    if report.map.axis_segment().is_none() {
        super::wrapped(
            ui,
            text::caption("The presets need a flip axis: add a Bottom setup at 0 or 180 deg."),
        );
        return;
    }
    let shown = edit::preset_choice(report);
    let mut choice = shown;
    ui.add(
        ChoiceRow::new("Preset", &mut choice, &PresetChoice::OPTIONS).hover(
            "3 pins: one pin on the flip axis plus a mirror pair off it; it keys with one \
             drill. 2 pins, mixed diameter: two pins of different diameters on the axis; it \
             keys with two holes and needs two drills (the Drills block below adds a missing \
             one). Custom: any other set.",
        ),
    );
    // The refusal of the last preset click stays on the panel until the
    // next finished edit, so the operator reads it.
    if let Some(reason) = inputs.preset_refusal {
        super::wrapped(
            ui,
            egui::RichText::new(reason)
                .small()
                .color(crate::ui::tokens::DANGER),
        );
    }
    if choice == shown {
        return;
    }
    let built = match choice {
        PresetChoice::AxisPair => plan
            .as_ref()
            .map(|pins| pins.to_vec())
            .map_err(|reason| format!("The 3-pin preset is not placed: {reason}")),
        PresetChoice::Mixed => to_mixed(stock, report, inputs, plan),
        // Custom is what any edit off a preset gives; there is nothing to
        // write.
        PresetChoice::Custom => return,
    };
    match built {
        Ok(pins) => {
            stock.alignment_pins = pins;
            out.edit.commit();
        }
        Err(reason) => out.preset_refusal = Some(Some(reason)),
    }
}

/// The large dowel size of a new mixed preset when no larger drill is in
/// the project: the small size plus twice [`PIN_MATCH_TOL_MM`], the hole
/// slop a dowel finds its hole within. It is a start value, not a dowel
/// standard: the operator sets the "Large diameter" to the dowel he has,
/// and the "Drills" block offers "Add a <d> mm drill" for it.
#[must_use]
pub fn mixed_start_large_diameter(small_d: f64) -> f64 {
    small_d + 2.0 * PIN_MATCH_TOL_MM
}

/// The large dowel size of a new mixed preset: the smallest drill of the
/// project that is larger than `small_d`, else
/// [`mixed_start_large_diameter`].
#[must_use]
pub fn mixed_large_diameter(small_d: f64, drill_diameters: &[f64]) -> f64 {
    drill_diameters
        .iter()
        .copied()
        .find(|d| *d > small_d + edit::MIN_DIAMETER_STEP_MM)
        .unwrap_or_else(|| mixed_start_large_diameter(small_d))
}

/// The mixed preset from the current pins.
///
/// The positions: two pins on the axis keep their places; else the axis
/// positions of the placer's plan. The diameters: the current dowel, and
/// [`mixed_large_diameter`]. The preset never refuses for a missing
/// drill (R2): a size with no drill shows in the "Drills" block with "Add
/// a <d> mm drill", in this section.
fn to_mixed(
    stock: &StockConfig,
    report: &PinSetReport,
    inputs: &PinSectionInputs<'_>,
    plan: &KeyedPinPlan,
) -> Result<Vec<AlignmentPin>, String> {
    let small_d = inputs.keyed.plan_pin_diameter(stock);
    let large_d = mixed_large_diameter(small_d, inputs.drill_diameters);
    edit::mixed_preset_pins(
        &stock.alignment_pins,
        &report.map,
        plan.as_ref().ok(),
        small_d,
        large_d,
    )
}

/// The largest distance from the flip axis to a stock corner, in mm: the
/// upper end of the "off axis" control.
fn max_across(map: &PinSetupMap, stock: &StockConfig) -> f64 {
    let Some([a, b]) = map.axis_segment() else {
        return 0.0;
    };
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    let n = (-(b.1 - a.1) / len, (b.0 - a.0) / len);
    [
        (0.0, 0.0),
        (stock.x, 0.0),
        (0.0, stock.y),
        (stock.x, stock.y),
    ]
    .into_iter()
    .map(|(x, y)| ((x - a.0) * n.0 + (y - a.1) * n.1).abs())
    .fold(0.0, f64::max)
}

/// The dimensions of the preset the pins fit.
pub(super) fn pattern_controls(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    report: &PinSetReport,
    drills: &[f64],
    edit_out: &mut PanelEdit,
) {
    let map = &report.map;
    let length = edit::axis_length(map);
    let along_hover = map.axis_segment().map(|[a, _]| {
        format!(
            "The distance along the flip axis ({}) from its end at ({:.1}, {:.1}) mm.",
            map.axis_label().unwrap_or_default(),
            a.0,
            a.1
        )
    });
    let along_hover = along_hover.as_deref();
    match report.preset {
        Some(PinPreset::AxisPlusMirrorPair(fit)) => {
            let mut p = fit;
            let across_max = max_across(map, stock).max(edit::MIN_PAIR_ACROSS_MM);
            ui.param_grid("pin_pattern_axis_pair", |ui| {
                let out = ValueRow::new(
                    "Axis pin along",
                    &mut p.axis_along,
                    " mm",
                    0.5,
                    0.0..=length,
                )
                .tooltip(along_hover)
                .show(ui);
                edit_out.drag(&out.value_response);
                let out = ValueRow::new("Pair along", &mut p.pair_along, " mm", 0.5, 0.0..=length)
                    .tooltip(along_hover)
                    .show(ui);
                edit_out.drag(&out.value_response);
                let out = ValueRow::new(
                    "Pair off axis",
                    &mut p.pair_across,
                    " mm",
                    0.5,
                    edit::MIN_PAIR_ACROSS_MM..=across_max,
                )
                .tooltip(Some("The distance of each pair pin from the flip axis."))
                .show(ui);
                edit_out.drag(&out.value_response);
                let out =
                    ValueRow::new("Diameter", &mut p.diameter, " mm", 0.1, 1.0..=25.0).show(ui);
                edit_out.drag(&out.value_response);
            });
            if p != fit
                && let Some(pins) = edit::axis_pair_pins(map, p)
            {
                stock.alignment_pins = pins;
            }
        }
        Some(PinPreset::TwoMixedDiameters(fit)) => {
            let mut p = fit;
            ui.param_grid("pin_pattern_mixed", |ui| {
                let out = ValueRow::new(
                    "Small pin along",
                    &mut p.small_along,
                    " mm",
                    0.5,
                    0.0..=length,
                )
                .tooltip(along_hover)
                .show(ui);
                edit_out.drag(&out.value_response);
                let out = ValueRow::new(
                    "Large pin along",
                    &mut p.large_along,
                    " mm",
                    0.5,
                    0.0..=length,
                )
                .tooltip(along_hover)
                .show(ui);
                edit_out.drag(&out.value_response);
                let small_max = p.large_diameter - edit::MIN_DIAMETER_STEP_MM;
                let out = ValueRow::new(
                    "Small diameter",
                    &mut p.small_diameter,
                    " mm",
                    0.1,
                    1.0..=small_max,
                )
                .show(ui);
                edit_out.drag(&out.value_response);
                let large_min = p.small_diameter + edit::MIN_DIAMETER_STEP_MM;
                let out = ValueRow::new(
                    "Large diameter",
                    &mut p.large_diameter,
                    " mm",
                    0.1,
                    large_min..=25.0,
                )
                .show(ui);
                edit_out.drag(&out.value_response);
            });
            // The drills of the project larger than the small pin: one
            // click makes the large pin that size.
            let picks: Vec<f64> = drills
                .iter()
                .copied()
                .filter(|d| *d > p.small_diameter + edit::MIN_DIAMETER_STEP_MM)
                .filter(|d| {
                    !rs_cam_core::compute::alignment_pins::same_pin_diameter(*d, p.large_diameter)
                })
                .collect();
            if !picks.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(text::caption("Large pin of a drill:"));
                    for d in &picks {
                        if ui
                            .add(Button::quiet(format!("{d:.2} mm")))
                            .on_hover_text(
                                "Makes the large pin this size: a drill of it is in the project.",
                            )
                            .clicked()
                        {
                            p.large_diameter = *d;
                            edit_out.commit();
                        }
                    }
                });
            }
            if p != fit
                && let Some(pins) = edit::mixed_pins(map, p)
            {
                stock.alignment_pins = pins;
            }
        }
        Some(PinPreset::Custom) | None => {}
    }
}

/// "Move all", "Snap to flip" and, for a custom set, one diameter for all.
pub(super) fn set_controls(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    report: &PinSetReport,
    edit_out: &mut PanelEdit,
) {
    let Some((cx, cy)) = edit::set_centre(&stock.alignment_pins) else {
        return;
    };
    ui.add_space(crate::ui::tokens::SPACE_2);
    let (mut x, mut y) = (cx, cy);
    ui.param_grid("pin_set_move", |ui| {
        let out = ValueRow::new(
            "Move all: centre X",
            &mut x,
            " mm",
            0.5,
            f64::MIN..=f64::MAX,
        )
        .tooltip(Some(
            "The mean X of the pins. A change moves every pin by the same step.",
        ))
        .show(ui);
        edit_out.drag(&out.value_response);
        let out = ValueRow::new(
            "Move all: centre Y",
            &mut y,
            " mm",
            0.5,
            f64::MIN..=f64::MAX,
        )
        .tooltip(Some(
            "The mean Y of the pins. A change moves every pin by the same step.",
        ))
        .show(ui);
        edit_out.drag(&out.value_response);
    });
    if x != cx || y != cy {
        stock.alignment_pins = edit::move_all(&stock.alignment_pins, x - cx, y - cy);
    }
    for (axis, dx_of) in [("X", 1.0), ("Y", 0.0)] {
        ui.horizontal_wrapped(|ui| {
            ui.label(text::caption(format!("Nudge {axis}")));
            for step in NUDGE_STEPS_MM {
                let label = format!("{step:+}");
                if ui.add(Button::quiet(label)).clicked() {
                    let (dx, dy) = (step * dx_of, step * (1.0 - dx_of));
                    stock.alignment_pins = edit::move_all(&stock.alignment_pins, dx, dy);
                    edit_out.commit();
                }
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        let can_snap = report.map.axis_segment().is_some();
        if ui
            .add(Button::new("Snap to flip").enabled(can_snap))
            .on_hover_text(
                "Moves the pins the least distance that makes the set exactly symmetric about \
                 the flip axis. A pin with no near mirror partner does not move.",
            )
            .clicked()
        {
            stock.alignment_pins = edit::snap(&stock.alignment_pins, &report.map);
            edit_out.commit();
        }
    });
    if matches!(report.preset, Some(PinPreset::Custom) | None)
        && let Some(first) = stock.alignment_pins.first().map(|p| p.diameter)
    {
        let mut d = first;
        ui.param_grid("pin_set_diameter", |ui| {
            let out = ValueRow::new("Diameter (all)", &mut d, " mm", 0.1, 1.0..=25.0)
                .tooltip(Some("One diameter for every pin."))
                .show(ui);
            edit_out.drag(&out.value_response);
        });
        if d != first {
            stock.alignment_pins = set_diameter(&stock.alignment_pins, d);
        }
    }
}

/// "Auto-place": the 2D placer, with its refusal in plain words.
pub(super) fn auto_place(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    has_flip: bool,
    plan: &KeyedPinPlan,
    edit_out: &mut PanelEdit,
) {
    ui.add_space(crate::ui::tokens::SPACE_2);
    let clicked = ui
        .add(Button::new("Auto-place keyed pins").enabled(plan.is_ok()))
        .on_hover_text(
            "Replaces the pins with three: one on the flip axis and a mirror pair off the \
             axis. The pins go in the clear material of the stock, with a wall to the edge, \
             the part, the keep-outs and the fixtures. The pin diameter is the diameter of \
             the drill that makes the pin holes.",
        )
        .clicked();
    if clicked && let Ok(pins) = plan {
        stock.alignment_pins = pins.to_vec();
        edit_out.commit();
    }
    if has_flip && let Err(reason) = plan {
        super::wrapped(
            ui,
            egui::RichText::new(format!("Auto-place refuses: {reason}"))
                .small()
                .color(crate::ui::tokens::DANGER),
        );
    }
}

/// The part-model chooser: which models the placer and the wall check
/// call the part. The default rule is in force until the operator ticks a
/// box; "Use the default" returns to it.
pub(super) fn part_models(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    models: &[PartModelRow],
    edit_out: &mut PanelEdit,
) {
    if models.is_empty() {
        return;
    }
    let count = models.iter().filter(|m| m.is_part).count();
    let chosen = stock.pin_part_models.is_some();
    let title = format!(
        "Part models: {count} of {} ({})",
        models.len(),
        if chosen { "chosen" } else { "default" }
    );
    ui.disclosure("alignment_pin_part_models", &title, false, |ui| {
        super::wrapped(
            ui,
            text::caption(
                "The pins keep a wall to these models. The default leaves out a model that \
             covers the whole stock and a drawing of pin holes.",
            ),
        );
        let mut picked: Vec<usize> = models.iter().filter(|m| m.is_part).map(|m| m.id).collect();
        let mut changed = false;
        for m in models {
            let mut on = m.is_part;
            let label = if m.default_part {
                m.name.clone()
            } else {
                format!("{} (not part by default)", m.name)
            };
            if ui.checkbox(&mut on, label).changed() {
                changed = true;
                if on {
                    picked.push(m.id);
                } else {
                    picked.retain(|id| *id != m.id);
                }
            }
        }
        if changed {
            picked.sort_unstable();
            stock.pin_part_models = Some(picked);
            edit_out.commit();
        }
        if chosen && ui.add(Button::quiet("Use the default")).clicked() {
            stock.pin_part_models = None;
            edit_out.commit();
        }
    });
}

/// The closed fold with the single-pin drags, "Remove" and "+ Add pin".
pub(super) fn single_pins(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    keyed: &KeyedPinInputs,
    edit_out: &mut PanelEdit,
) {
    ui.disclosure("alignment_pin_singles", "Edit single pins", false, |ui| {
        let (w, d) = (stock.x, stock.y);
        let mut remove: Option<usize> = None;
        for (i, pin) in stock.alignment_pins.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                ui.param_grid(("alignment_pin_single", i), |ui| {
                    let x_label = format!("Pin {} X", i + 1);
                    let out = ValueRow::new(&x_label, &mut pin.x, " mm", 0.5, 0.0..=w).show(ui);
                    edit_out.drag(&out.value_response);
                    let y_label = format!("Pin {} Y", i + 1);
                    let out = ValueRow::new(&y_label, &mut pin.y, " mm", 0.5, 0.0..=d).show(ui);
                    edit_out.drag(&out.value_response);
                });
                if ui
                    .add(Button::danger(format!("Remove pin {}", i + 1)))
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            stock.alignment_pins.remove(i);
            edit_out.commit();
        }
        let centre = (stock.x * 0.5, stock.y * 0.5);
        // Repeated clicks used to stack pins at one point.
        let centre_free = !stock.alignment_pins.iter().any(|p| {
            (p.x - centre.0).abs() < PIN_MATCH_TOL_MM && (p.y - centre.1).abs() < PIN_MATCH_TOL_MM
        });
        if ui
            .add(Button::new("+ Add pin").enabled(centre_free))
            .on_hover_text("Adds one pin at the stock centre. Drag it into place.")
            .clicked()
        {
            let diameter = stock
                .alignment_pins
                .first()
                .map(|p| p.diameter)
                .unwrap_or_else(|| keyed.plan_pin_diameter(stock));
            stock
                .alignment_pins
                .push(AlignmentPin::new(centre.0, centre.1, diameter));
            edit_out.commit();
        }
    });
}
