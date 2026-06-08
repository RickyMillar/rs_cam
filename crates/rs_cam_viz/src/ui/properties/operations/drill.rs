use rs_cam_core::feeds::FeedsResult;

use crate::state::toolpath::{AlignmentPinDrillConfig, DrillConfig, DrillCycleType};

use super::super::{dv, dv_pill};

pub(in crate::ui::properties) fn draw_drill_params(
    ui: &mut egui::Ui,
    cfg: &mut DrillConfig,
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
}

pub(in crate::ui::properties) fn draw_alignment_pin_drill_params(
    ui: &mut egui::Ui,
    cfg: &mut AlignmentPinDrillConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let feed_sugg = feeds_result.map(|r| (r.feed_rate_mm_min, &r.chipload_source));
    ui.label(format!("{} hole(s)", cfg.holes.len()));
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
}
