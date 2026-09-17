use super::super::pills::PillSuggestions;
use rs_cam_core::compute::execute::stale_drill_picks_refusal;
use rs_cam_core::io::dxf_input::{DrillTarget, DrillTargetKind};

use crate::state::toolpath::{AlignmentPinDrillConfig, DrillConfig, DrillCycleType};

use super::super::{depth_caution_row, dv, dv_pill};
use super::DepthBeyondStock;

/// Match tolerance for comparing a picked hole to a target position (mm).
///
/// The core constant, not a local copy: the generator resolves a pick
/// against the model's targets at this same distance (G-DRILLPICKSTALE), so
/// a hole this panel calls "already picked" is a hole the generator will
/// resolve.
const TARGET_EPS: f64 = rs_cam_core::compute::execute::DRILL_PICK_MATCH_EPS_MM;

fn xy_in(holes: &[[f64; 2]], xy: [f64; 2]) -> bool {
    holes
        .iter()
        .any(|h| (h[0] - xy[0]).abs() < TARGET_EPS && (h[1] - xy[1]).abs() < TARGET_EPS)
}

/// What the target sources are, printed on every drill panel so the operator
/// knows what to import (G-DRILLCENTROID, UX-R03-004).
const TARGET_SOURCES_NOTE: &str =
    "Targets are DXF points and circle/arc centres, and circles in SVG drawings.";

/// Shared "drill targets" selector: a count, viewport hint, Select all / Clear,
/// and a per-layer "select all in layer" dropdown. Mutates `selected_holes`
/// (the resolved positions) and `selected_layers` (display/round-trip).
///
/// `no_targets_refusal` is the sentence the generator refuses with when
/// `targets` is empty and nothing is picked — `Some` for `Drill`, whose only
/// hole sources these are, so the count (0 allowed) and the refusal are
/// always visible; `None` for the pin drill, whose holes are the stock's
/// alignment pins and for which an empty target list is unremarkable.
fn draw_drill_target_selector(
    ui: &mut egui::Ui,
    selected_holes: &mut Option<Vec<[f64; 2]>>,
    selected_layers: &mut Vec<String>,
    layers: &[String],
    targets: &[DrillTarget],
    no_targets_refusal: Option<&str>,
) {
    if targets.is_empty() {
        let Some(refusal) = no_targets_refusal else {
            return;
        };
        ui.separator();
        ui.label("Drill targets: 0");
        ui.colored_label(crate::ui::tokens::DANGER, refusal);
        ui.label(egui::RichText::new(TARGET_SOURCES_NOTE).small().weak());
        return;
    }
    ui.separator();
    let total = targets.len();
    let sel_count = selected_holes.as_ref().map_or(0, Vec::len);
    match selected_holes.as_ref() {
        None => ui.label(format!(
            "Drill targets: {total} available (none selected → drilling all targets)"
        )),
        Some(picked) if picked.is_empty() => ui.colored_label(
            crate::ui::tokens::DANGER,
            format!(
                "Drill targets: 0 of {total} selected — {}",
                rs_cam_core::compute::execute::NO_DRILL_TARGETS_SELECTED_MSG
            ),
        ),
        Some(_) => ui.label(format!("Drill targets: {sel_count} of {total} selected")),
    };
    // G-DRILLPICKSTALE (F4.8): the operator must be able to see a stale
    // pick BEFORE pressing Generate, not only in the refusal. Same
    // predicate the generator refuses with, so the panel and the generator
    // cannot say different things.
    let stale = selected_holes
        .as_ref()
        .and_then(|picked| stale_drill_picks_refusal(picked, targets));
    if let Some(msg) = stale {
        ui.colored_label(crate::ui::tokens::DANGER, msg);
    }
    ui.label(
        egui::RichText::new("Click points/holes in the viewport to toggle.")
            .small()
            .weak(),
    );
    ui.horizontal(|ui| {
        if ui.button("Select all").clicked() {
            *selected_holes = Some(targets.iter().map(|t| [t.x, t.y]).collect());
            *selected_layers = layers.to_vec();
        }
        if ui.button("Clear").clicked() {
            *selected_holes = None;
            selected_layers.clear();
        }
        if !layers.is_empty() {
            egui::ComboBox::from_id_salt("drill_layer_select")
                .selected_text("Select layer…")
                .show_ui(ui, |ui| {
                    for layer in layers {
                        if ui.selectable_label(false, layer).clicked() {
                            let mut holes = selected_holes.take().unwrap_or_default();
                            for t in targets.iter().filter(|t| &t.layer == layer) {
                                let xy = [t.x, t.y];
                                if !xy_in(&holes, xy) {
                                    holes.push(xy);
                                }
                            }
                            *selected_holes = Some(holes);
                            if !selected_layers.iter().any(|l| l == layer) {
                                selected_layers.push(layer.clone());
                            }
                        }
                    }
                });
        }
    });
    // Brief legend so circle vs point markers read clearly; the point count
    // is named only when there are any.
    let points = targets
        .iter()
        .filter(|t| matches!(t.kind, DrillTargetKind::Point))
        .count();
    let legend = if points > 0 {
        format!("{TARGET_SOURCES_NOTE} {points} of these are points.")
    } else {
        TARGET_SOURCES_NOTE.to_owned()
    };
    ui.label(egui::RichText::new(legend).small().weak());
}

/// The four-way drill-cycle combo, one renderer for both editors.
///
/// UI-11: `draw_drill_params` and `draw_alignment_pin_drill_params` wrote
/// this combo out twice, label strings included, so a new cycle or a
/// re-worded label reached one editor and not the other. `id_salt` keeps the
/// two combos' egui ids distinct.
fn draw_drill_cycle_combo(ui: &mut egui::Ui, id_salt: &str, cycle: &mut DrillCycleType) {
    ui.label("Cycle:");
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(match cycle {
            DrillCycleType::Simple => "Simple (G81)",
            DrillCycleType::Dwell => "Dwell (G82)",
            DrillCycleType::Peck => "Peck (G83)",
            DrillCycleType::ChipBreak => "Chip Break (G73)",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(cycle, DrillCycleType::Simple, "Simple (G81)");
            ui.selectable_value(cycle, DrillCycleType::Dwell, "Dwell (G82)");
            ui.selectable_value(cycle, DrillCycleType::Peck, "Peck (G83)");
            ui.selectable_value(cycle, DrillCycleType::ChipBreak, "Chip Break (G73)");
        });
    ui.end_row();
}

/// The feed-rate and retract rows both drill editors carry.
fn draw_drill_feed_rows(
    ui: &mut egui::Ui,
    feed_rate: &mut f64,
    retract_z: &mut f64,
    feed_sugg: Option<super::super::pills::PillSuggestion<'_>>,
) {
    dv_pill(
        ui,
        "Feed Rate:",
        feed_rate,
        " mm/min",
        10.0,
        1.0..=5000.0,
        feed_sugg,
    );
    dv(ui, "Retract (R):", retract_z, " mm", 0.5, 0.5..=50.0);
}

/// The rows a cycle adds: peck depth, dwell time and chip-break retract.
///
/// `dwell_time` and `retract_amount` are `None` for the alignment-pin
/// editor, whose config carries no knobs for them — pin drilling fixes dwell
/// at 0.5 s and chip-break retract at 0.5 mm
/// (`AlignmentPinDrillConfig::drill_cycle`). That is the ONLY difference
/// between the two editors' conditional rows.
fn draw_drill_cycle_rows(
    ui: &mut egui::Ui,
    cycle: DrillCycleType,
    peck_depth: &mut f64,
    dwell_time: Option<&mut f64>,
    retract_amount: Option<&mut f64>,
) {
    if matches!(cycle, DrillCycleType::Peck | DrillCycleType::ChipBreak) {
        dv(ui, "Peck Depth:", peck_depth, " mm", 0.5, 0.5..=50.0);
    }
    if let Some(dwell_time) = dwell_time
        && cycle == DrillCycleType::Dwell
    {
        dv(ui, "Dwell Time:", dwell_time, " s", 0.1, 0.1..=10.0);
    }
    if let Some(retract_amount) = retract_amount
        && cycle == DrillCycleType::ChipBreak
    {
        dv(ui, "Retract Amt:", retract_amount, " mm", 0.1, 0.1..=5.0);
    }
}

pub(in crate::ui::properties) fn draw_drill_params(
    ui: &mut egui::Ui,
    cfg: &mut DrillConfig,
    drill_layers: &[String],
    drill_targets: &[DrillTarget],
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    // Spec: drill ops get a feed pill (peck-cycle plunge feed) but no
    // stepover/DOC pills (Z-only kinematics — LUT radial/axial don't
    // apply). DrillConfig has no plunge_rate or spindle_rpm slot of its
    // own, so this is the only LUT-driven field on the panel.
    let feed_sugg = pills.map(PillSuggestions::feed_rate);
    egui::Grid::new("drill_p")
        .num_columns(2)
        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
        .min_row_height(crate::ui::tokens::ROW_DENSE)
        .show(ui, |ui| {
            draw_drill_cycle_combo(ui, "drill_cycle", &mut cfg.cycle);
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.5, 0.5..=100.0);
            depth_caution_row(ui, depth_caution);
            draw_drill_feed_rows(ui, &mut cfg.feed_rate, &mut cfg.retract_z, feed_sugg);
            draw_drill_cycle_rows(
                ui,
                cfg.cycle,
                &mut cfg.peck_depth,
                Some(&mut cfg.dwell_time),
                Some(&mut cfg.retract_amount),
            );
        });
    draw_drill_target_selector(
        ui,
        &mut cfg.selected_holes,
        &mut cfg.selected_layers,
        drill_layers,
        drill_targets,
        Some(rs_cam_core::compute::execute::NO_DRILL_TARGETS_MSG),
    );
}

pub(in crate::ui::properties) fn draw_alignment_pin_drill_params(
    ui: &mut egui::Ui,
    cfg: &mut AlignmentPinDrillConfig,
    drill_layers: &[String],
    drill_targets: &[DrillTarget],
    pills: Option<&PillSuggestions>,
) {
    let feed_sugg = pills.map(PillSuggestions::feed_rate);
    let extra = cfg.selected_holes.as_ref().map_or(0, Vec::len);
    ui.label(format!(
        "{} pin(s) + {extra} picked hole(s)",
        cfg.holes.len()
    ));
    egui::Grid::new("pin_drill_p")
        .num_columns(2)
        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
        .min_row_height(crate::ui::tokens::ROW_DENSE)
        .show(ui, |ui| {
            dv(
                ui,
                "Spoilboard:",
                &mut cfg.spoilboard_penetration,
                " mm",
                0.5,
                0.5..=20.0,
            );
            draw_drill_cycle_combo(ui, "pin_drill_cycle", &mut cfg.cycle);
            draw_drill_feed_rows(ui, &mut cfg.feed_rate, &mut cfg.retract_z, feed_sugg);
            draw_drill_cycle_rows(ui, cfg.cycle, &mut cfg.peck_depth, None, None);
        });
    draw_drill_target_selector(
        ui,
        &mut cfg.selected_holes,
        &mut cfg.selected_layers,
        drill_layers,
        drill_targets,
        None,
    );
}
