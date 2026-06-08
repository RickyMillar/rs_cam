use rs_cam_core::feeds::FeedsResult;

use crate::state::toolpath::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, DropCutterConfig, PencilConfig,
    RegionOrdering, ScallopConfig, ScallopDirection, SteepShallowConfig, WaterlineConfig,
};

use super::super::{dv, dv_pill};
use super::draw_feed_params;

pub(in crate::ui::properties) fn draw_dropcutter_params(
    ui: &mut egui::Ui,
    cfg: &mut DropCutterConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    egui::Grid::new("dc_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.1,
                0.05..=50.0,
                stepover_sugg,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
            dv(ui, "Min Z:", &mut cfg.min_z, " mm", 0.5, -500.0..=0.0);
            dv(
                ui,
                "Slope From:",
                &mut cfg.slope_from,
                " deg",
                1.0,
                0.0..=90.0,
            );
            dv(ui, "Slope To:", &mut cfg.slope_to, " deg", 1.0, 0.0..=90.0);
        });
}

pub(in crate::ui::properties) fn draw_adaptive3d_params(
    ui: &mut egui::Ui,
    cfg: &mut Adaptive3dConfig,
    feeds_result: Option<&FeedsResult>,
) {
    // Spec: pill stepover + depth_per_pass; leave fine_stepdown alone
    // (finishing-pass param the LUT doesn't speak to).
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    let dpp_sugg = feeds_result.map(|r| (r.axial_depth_mm, &r.chipload_source));
    egui::Grid::new("a3d_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.1,
                0.05..=50.0,
                stepover_sugg,
            );
            dv_pill(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
                dpp_sugg,
            );
            dv(
                ui,
                "Floor Stock:",
                &mut cfg.stock_to_leave_axial,
                " mm",
                0.05,
                0.0..=10.0,
            );
            dv(
                ui,
                "Wall Stock:",
                &mut cfg.stock_to_leave_radial,
                " mm",
                0.05,
                0.0..=10.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
            dv(
                ui,
                "Tolerance:",
                &mut cfg.tolerance,
                " mm",
                0.01,
                0.01..=1.0,
            );
            dv(
                ui,
                "Min Cut Radius:",
                &mut cfg.min_cutting_radius,
                " mm",
                0.1,
                0.0..=50.0,
            );
            ui.label("Entry Style:");
            egui::ComboBox::from_id_salt("a3d_entry")
                .selected_text(match cfg.entry_style {
                    Adaptive3dEntryStyle::Plunge => "Plunge",
                    Adaptive3dEntryStyle::Helix => "Helix",
                    Adaptive3dEntryStyle::Ramp => "Ramp",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut cfg.entry_style,
                        Adaptive3dEntryStyle::Plunge,
                        "Plunge",
                    );
                    ui.selectable_value(&mut cfg.entry_style, Adaptive3dEntryStyle::Helix, "Helix");
                    ui.selectable_value(&mut cfg.entry_style, Adaptive3dEntryStyle::Ramp, "Ramp");
                });
            ui.end_row();
            // Entry-style-specific parameters — only shown for the style in use.
            match cfg.entry_style {
                Adaptive3dEntryStyle::Plunge => {}
                Adaptive3dEntryStyle::Ramp => {
                    dv(
                        ui,
                        "Ramp Angle:",
                        &mut cfg.ramp_angle_deg,
                        " deg",
                        0.5,
                        0.5..=45.0,
                    );
                }
                Adaptive3dEntryStyle::Helix => {
                    dv(
                        ui,
                        "Helix Radius:",
                        &mut cfg.helix_radius_factor,
                        " × D",
                        0.05,
                        0.05..=0.5,
                    );
                    dv(
                        ui,
                        "Helix Pitch:",
                        &mut cfg.helix_pitch,
                        " mm",
                        0.1,
                        0.1..=10.0,
                    );
                }
            }
            dv(
                ui,
                "Fine Stepdown:",
                &mut cfg.fine_stepdown,
                " mm",
                0.1,
                0.0..=10.0,
            );
            ui.label("Detect Flat:");
            ui.checkbox(&mut cfg.detect_flat_areas, "");
            ui.end_row();
            ui.label("Ordering:");
            egui::ComboBox::from_id_salt("a3d_ord")
                .selected_text(match cfg.region_ordering {
                    RegionOrdering::Global => "Global",
                    RegionOrdering::ByArea => "By Area",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.region_ordering, RegionOrdering::Global, "Global");
                    ui.selectable_value(
                        &mut cfg.region_ordering,
                        RegionOrdering::ByArea,
                        "By Area",
                    );
                });
            ui.end_row();
            ui.label("Strategy:");
            egui::ComboBox::from_id_salt("a3d_strat")
                .selected_text(match cfg.clearing_strategy {
                    ClearingStrategy::ContourParallel => "Contour Parallel",
                    ClearingStrategy::Adaptive => "Adaptive",
                    ClearingStrategy::AgentSearch => "Agent Search",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut cfg.clearing_strategy,
                        ClearingStrategy::ContourParallel,
                        "Contour Parallel",
                    );
                    ui.selectable_value(
                        &mut cfg.clearing_strategy,
                        ClearingStrategy::Adaptive,
                        "Adaptive",
                    );
                    ui.selectable_value(
                        &mut cfg.clearing_strategy,
                        ClearingStrategy::AgentSearch,
                        "Agent Search",
                    )
                    .on_hover_text(
                        "Per-step direction search with preflight skip and widen-band \
                         recovery. Slow to generate — use when Contour Parallel or \
                         Adaptive leave uncut bands on difficult geometry.",
                    );
                });
            ui.end_row();
            ui.label("Z Blend:");
            ui.checkbox(&mut cfg.z_blend, "");
            ui.end_row();
            ui.label("Mill Shallow:").on_hover_text(
                "Insert fine sub-passes on low-slope cells within each \
                     DPP descent. Steep walls keep the normal DPP cadence; \
                     shallow areas come off the rough nearly smooth. \
                     Works with any clearing strategy.",
            );
            ui.checkbox(&mut cfg.mill_shallow_areas, "");
            ui.end_row();
            if cfg.mill_shallow_areas {
                let mut angle = cfg.shallow_angle_deg.unwrap_or(30.0);
                ui.label("Shallow Angle:");
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut angle)
                                .speed(1.0)
                                .range(5.0..=60.0)
                                .suffix("°"),
                        )
                        .changed()
                    {
                        cfg.shallow_angle_deg = Some(angle);
                    }
                });
                ui.end_row();
                let mut step = cfg.shallow_stepdown.unwrap_or(cfg.depth_per_pass * 0.5);
                ui.label("Shallow Step:");
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut step)
                                .speed(0.05)
                                .range(0.05..=cfg.depth_per_pass.max(0.1))
                                .suffix(" mm"),
                        )
                        .changed()
                    {
                        cfg.shallow_stepdown = Some(step);
                    }
                });
                ui.end_row();
            }
        });
}

pub(in crate::ui::properties) fn draw_waterline_params(
    ui: &mut egui::Ui,
    cfg: &mut WaterlineConfig,
    feeds_result: Option<&FeedsResult>,
) {
    // Waterline: z_step is the axial pass spacing, but Step 3 LUT mapping
    // (axial_depth_mm) is calibrated for clearing DOC, not contour Z-step.
    // Leave Z Step alone; only feed/plunge/RPM get pills.
    egui::Grid::new("wl_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
            dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
            ui.label("Continuous:");
            ui.checkbox(&mut cfg.continuous, "");
            ui.end_row();
            // Z range now comes from the Heights tab (top_z / bottom_z)
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
        });
}

pub(in crate::ui::properties) fn draw_pencil_params(
    ui: &mut egui::Ui,
    cfg: &mut PencilConfig,
    feeds_result: Option<&FeedsResult>,
) {
    // Pencil's offset_stepover is a parallel-pass spacing, not the same
    // shape as a clearing radial WOC; leave it alone. Only feed/plunge/RPM
    // get pills.
    egui::Grid::new("pen_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Bitangency Angle:",
                &mut cfg.bitangency_angle,
                " deg",
                1.0,
                90.0..=180.0,
            );
            dv(
                ui,
                "Min Cut Length:",
                &mut cfg.min_cut_length,
                " mm",
                0.5,
                0.5..=50.0,
            );
            dv(
                ui,
                "Hookup Distance:",
                &mut cfg.hookup_distance,
                " mm",
                0.5,
                0.5..=50.0,
            );
            ui.label("Offset Passes:");
            let mut n = cfg.num_offset_passes as i32;
            if ui.add(egui::DragValue::new(&mut n).range(0..=10)).changed() {
                cfg.num_offset_passes = n.max(0) as usize;
            }
            ui.end_row();
            dv(
                ui,
                "Offset Stepover:",
                &mut cfg.offset_stepover,
                " mm",
                0.1,
                0.05..=10.0,
            );
            dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
            dv(
                ui,
                "Stock to Leave:",
                &mut cfg.stock_to_leave,
                " mm",
                0.05,
                0.0..=10.0,
            );
        });
}

pub(in crate::ui::properties) fn draw_scallop_params(
    ui: &mut egui::Ui,
    cfg: &mut ScallopConfig,
    feeds_result: Option<&FeedsResult>,
) {
    // Scallop's stepover is computed from scallop_height + tool radius, not
    // an editable field, so no stepover pill. Only feed/plunge/RPM.
    egui::Grid::new("sc_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Scallop Height:",
                &mut cfg.scallop_height,
                " mm",
                0.01,
                0.01..=2.0,
            );
            dv(
                ui,
                "Tolerance:",
                &mut cfg.tolerance,
                " mm",
                0.01,
                0.01..=1.0,
            );
            ui.label("Direction:");
            egui::ComboBox::from_id_salt("sc_dir")
                .selected_text(match cfg.direction {
                    ScallopDirection::OutsideIn => "Outside In",
                    ScallopDirection::InsideOut => "Inside Out",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut cfg.direction,
                        ScallopDirection::OutsideIn,
                        "Outside In",
                    );
                    ui.selectable_value(
                        &mut cfg.direction,
                        ScallopDirection::InsideOut,
                        "Inside Out",
                    );
                });
            ui.end_row();
            ui.label("Continuous:");
            ui.checkbox(&mut cfg.continuous, "");
            ui.end_row();
            dv(
                ui,
                "Slope From:",
                &mut cfg.slope_from,
                " deg",
                1.0,
                0.0..=90.0,
            );
            dv(ui, "Slope To:", &mut cfg.slope_to, " deg", 1.0, 0.0..=90.0);
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
            dv(
                ui,
                "Stock to Leave:",
                &mut cfg.stock_to_leave,
                " mm",
                0.05,
                0.0..=10.0,
            );
        });
}

pub(in crate::ui::properties) fn draw_steep_shallow_params(
    ui: &mut egui::Ui,
    cfg: &mut SteepShallowConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    egui::Grid::new("ss_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Threshold Angle:",
                &mut cfg.threshold_angle,
                " deg",
                1.0,
                10.0..=80.0,
            );
            dv(
                ui,
                "Overlap:",
                &mut cfg.overlap_distance,
                " mm",
                0.1,
                0.0..=10.0,
            );
            dv(
                ui,
                "Wall Clearance:",
                &mut cfg.wall_clearance,
                " mm",
                0.1,
                0.0..=10.0,
            );
            ui.label("Steep First:");
            ui.checkbox(&mut cfg.steep_first, "");
            ui.end_row();
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.1,
                0.05..=50.0,
                stepover_sugg,
            );
            dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
            dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
            dv(
                ui,
                "Stock to Leave:",
                &mut cfg.stock_to_leave,
                " mm",
                0.05,
                0.0..=10.0,
            );
            dv(
                ui,
                "Tolerance:",
                &mut cfg.tolerance,
                " mm",
                0.01,
                0.01..=1.0,
            );
        });
}
