use rs_cam_core::dxf_input::{DrillTarget, DrillTargetKind};
use rs_cam_core::feeds::FeedsResult;

use crate::state::toolpath::{AlignmentPinDrillConfig, DrillConfig, DrillCycleType};

use super::super::{dv, dv_pill};

/// Match tolerance for comparing a picked hole to a target position (mm).
const TARGET_EPS: f64 = 1e-6;

fn xy_in(holes: &[[f64; 2]], xy: [f64; 2]) -> bool {
    holes
        .iter()
        .any(|h| (h[0] - xy[0]).abs() < TARGET_EPS && (h[1] - xy[1]).abs() < TARGET_EPS)
}

/// Shared "drill targets" selector: a count, viewport hint, Select all / Clear,
/// and a per-layer "select all in layer" dropdown. Mutates `selected_holes`
/// (the resolved positions) and `selected_layers` (display/round-trip).
fn draw_drill_target_selector(
    ui: &mut egui::Ui,
    selected_holes: &mut Option<Vec<[f64; 2]>>,
    selected_layers: &mut Vec<String>,
    layers: &[String],
    targets: &[DrillTarget],
) {
    if targets.is_empty() {
        return;
    }
    ui.separator();
    let total = targets.len();
    let sel_count = selected_holes.as_ref().map_or(0, Vec::len);
    if sel_count == 0 {
        ui.label(format!(
            "Drill targets: {total} available (none selected → drilling all shapes)"
        ));
    } else {
        ui.label(format!("Drill targets: {sel_count} of {total} selected"));
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
    // Brief legend so circle vs point markers read clearly.
    if targets
        .iter()
        .any(|t| matches!(t.kind, DrillTargetKind::Point))
    {
        ui.label(
            egui::RichText::new("Targets include DXF points and circle/arc centres.")
                .small()
                .weak(),
        );
    }
}

pub(in crate::ui::properties) fn draw_drill_params(
    ui: &mut egui::Ui,
    cfg: &mut DrillConfig,
    drill_layers: &[String],
    drill_targets: &[DrillTarget],
    feeds_result: Option<&FeedsResult>,
) {
    // Spec: drill ops get a feed pill (peck-cycle plunge feed) but no
    // stepover/DOC pills (Z-only kinematics — LUT radial/axial don't
    // apply). DrillConfig has no plunge_rate or spindle_rpm slot of its
    // own, so this is the only LUT-driven field on the panel.
    let feed_sugg = feeds_result.map(|r| (r.feed_rate_mm_min, &r.chipload_source));
    egui::Grid::new("drill_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Cycle:");
            egui::ComboBox::from_id_salt("drill_cycle")
                .selected_text(match cfg.cycle {
                    DrillCycleType::Simple => "Simple (G81)",
                    DrillCycleType::Dwell => "Dwell (G82)",
                    DrillCycleType::Peck => "Peck (G83)",
                    DrillCycleType::ChipBreak => "Chip Break (G73)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Simple, "Simple (G81)");
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Dwell, "Dwell (G82)");
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Peck, "Peck (G83)");
                    ui.selectable_value(
                        &mut cfg.cycle,
                        DrillCycleType::ChipBreak,
                        "Chip Break (G73)",
                    );
                });
            ui.end_row();
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.5, 0.5..=100.0);
            dv_pill(
                ui,
                "Feed Rate:",
                &mut cfg.feed_rate,
                " mm/min",
                10.0,
                1.0..=5000.0,
                feed_sugg,
            );
            dv(
                ui,
                "Retract (R):",
                &mut cfg.retract_z,
                " mm",
                0.5,
                0.5..=50.0,
            );
            if matches!(cfg.cycle, DrillCycleType::Peck | DrillCycleType::ChipBreak) {
                dv(
                    ui,
                    "Peck Depth:",
                    &mut cfg.peck_depth,
                    " mm",
                    0.5,
                    0.5..=50.0,
                );
            }
            if cfg.cycle == DrillCycleType::Dwell {
                dv(
                    ui,
                    "Dwell Time:",
                    &mut cfg.dwell_time,
                    " s",
                    0.1,
                    0.1..=10.0,
                );
            }
            if cfg.cycle == DrillCycleType::ChipBreak {
                dv(
                    ui,
                    "Retract Amt:",
                    &mut cfg.retract_amount,
                    " mm",
                    0.1,
                    0.1..=5.0,
                );
            }
        });
    draw_drill_target_selector(
        ui,
        &mut cfg.selected_holes,
        &mut cfg.selected_layers,
        drill_layers,
        drill_targets,
    );
}

pub(in crate::ui::properties) fn draw_alignment_pin_drill_params(
    ui: &mut egui::Ui,
    cfg: &mut AlignmentPinDrillConfig,
    drill_layers: &[String],
    drill_targets: &[DrillTarget],
    feeds_result: Option<&FeedsResult>,
) {
    let feed_sugg = feeds_result.map(|r| (r.feed_rate_mm_min, &r.chipload_source));
    let extra = cfg.selected_holes.as_ref().map_or(0, Vec::len);
    ui.label(format!(
        "{} pin(s) + {extra} picked hole(s)",
        cfg.holes.len()
    ));
    egui::Grid::new("pin_drill_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Spoilboard:",
                &mut cfg.spoilboard_penetration,
                " mm",
                0.5,
                0.5..=20.0,
            );
            ui.label("Cycle:");
            egui::ComboBox::from_id_salt("pin_drill_cycle")
                .selected_text(match cfg.cycle {
                    DrillCycleType::Simple => "Simple (G81)",
                    DrillCycleType::Dwell => "Dwell (G82)",
                    DrillCycleType::Peck => "Peck (G83)",
                    DrillCycleType::ChipBreak => "Chip Break (G73)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Simple, "Simple (G81)");
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Dwell, "Dwell (G82)");
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Peck, "Peck (G83)");
                    ui.selectable_value(
                        &mut cfg.cycle,
                        DrillCycleType::ChipBreak,
                        "Chip Break (G73)",
                    );
                });
            ui.end_row();
            dv_pill(
                ui,
                "Feed Rate:",
                &mut cfg.feed_rate,
                " mm/min",
                10.0,
                1.0..=5000.0,
                feed_sugg,
            );
            dv(
                ui,
                "Retract (R):",
                &mut cfg.retract_z,
                " mm",
                0.5,
                0.5..=50.0,
            );
            if matches!(cfg.cycle, DrillCycleType::Peck | DrillCycleType::ChipBreak) {
                dv(
                    ui,
                    "Peck Depth:",
                    &mut cfg.peck_depth,
                    " mm",
                    0.5,
                    0.5..=50.0,
                );
            }
        });
    draw_drill_target_selector(
        ui,
        &mut cfg.selected_holes,
        &mut cfg.selected_layers,
        drill_layers,
        drill_targets,
    );
}
