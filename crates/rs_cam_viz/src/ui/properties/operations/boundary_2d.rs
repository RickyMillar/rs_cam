use super::super::pills::PillSuggestions;

use crate::state::toolpath::{
    AdaptiveConfig, CompensationType, FaceConfig, FaceDirection, InlayConfig, PocketConfig,
    PocketPattern, ProfileConfig, ProfileSide, RestConfig, VCarveConfig, ZigzagConfig,
};

use super::super::{depth_caution_row, dv, dv_pill, through_cut_row};
use super::{DepthBeyondStock, ThroughCut, draw_tab_diagram};
use crate::ui::components::UiExt as _;

pub(in crate::ui::properties) fn draw_face_params(
    ui: &mut egui::Ui,
    cfg: &mut FaceConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    ui.param_grid("face_p", |ui| {
        ui.label("Direction:");
        egui::ComboBox::from_id_salt("face_dir")
            .selected_text(match cfg.direction {
                FaceDirection::OneWay => "One Way",
                FaceDirection::Zigzag => "Zigzag",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut cfg.direction, FaceDirection::OneWay, "One Way");
                ui.selectable_value(&mut cfg.direction, FaceDirection::Zigzag, "Zigzag");
            });
        ui.end_row();
        dv_pill(
            ui,
            "Stepover:",
            &mut cfg.stepover,
            " mm",
            0.5,
            0.5..=100.0,
            stepover_sugg,
        );
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.0..=50.0);
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            "Depth/Pass:",
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            0.1..=20.0,
            dpp_sugg,
        );
        dv(
            ui,
            "Stock Offset:",
            &mut cfg.stock_offset,
            " mm",
            0.5,
            0.0..=50.0,
        );
    });
}

pub(in crate::ui::properties) fn draw_pocket_params(
    ui: &mut egui::Ui,
    cfg: &mut PocketConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    ui.param_grid("pocket_p", |ui| {
        ui.label("Pattern:");
        egui::ComboBox::from_id_salt("pocket_pat")
            .selected_text(match cfg.pattern {
                PocketPattern::Contour => "Contour",
                PocketPattern::Zigzag => "Zigzag",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut cfg.pattern, PocketPattern::Contour, "Contour");
                ui.selectable_value(&mut cfg.pattern, PocketPattern::Zigzag, "Zigzag");
            });
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
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            "Depth/Pass:",
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            0.1..=50.0,
            dpp_sugg,
        );
        ui.label("Climb:");
        ui.checkbox(&mut cfg.climb, "");
        ui.end_row();
        if cfg.pattern == PocketPattern::Zigzag {
            dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
        }
        ui.label("Finishing Passes:");
        let mut fp = cfg.finishing_passes as i32;
        if ui
            .add(egui::DragValue::new(&mut fp).range(0..=10).speed(0.1))
            .on_hover_text("Spring passes at final depth for dimensional accuracy")
            .changed()
        {
            cfg.finishing_passes = fp.max(0) as usize;
        }
        ui.end_row();
    });
}

/// `through_cut` is the G-THROUGHCUT (UX-R03-006) reading for this Profile:
/// `Some` when the cut bottom reaches the stock bottom. It draws the
/// informational line under Depth (before the G-DEPTHSTOCK caution, which
/// may show as well when the depth is beyond the board) and, when no tabs
/// are configured, opens the Tabs disclosure. The open is a DEFAULT: it is
/// forced only on the frame the condition first appears, so the operator can
/// still collapse the section afterwards.
pub(in crate::ui::properties) fn draw_profile_params(
    ui: &mut egui::Ui,
    cfg: &mut ProfileConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
    through_cut: Option<&ThroughCut>,
) {
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    ui.param_grid("profile_p", |ui| {
        ui.label("Side:");
        egui::ComboBox::from_id_salt("prof_side")
            .selected_text(match cfg.side {
                ProfileSide::Outside => "Outside",
                ProfileSide::Inside => "Inside",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut cfg.side, ProfileSide::Outside, "Outside");
                ui.selectable_value(&mut cfg.side, ProfileSide::Inside, "Inside");
            });
        ui.end_row();
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        through_cut_row(ui, through_cut);
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            "Depth/Pass:",
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            0.1..=50.0,
            dpp_sugg,
        );
        ui.label("Climb:");
        ui.checkbox(&mut cfg.climb, "");
        ui.end_row();
        ui.label("Compensation:");
        egui::ComboBox::from_id_salt("prof_comp")
            .selected_text(match cfg.compensation {
                CompensationType::InComputer => "In Computer",
                CompensationType::InControl => "In Control",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut cfg.compensation,
                    CompensationType::InComputer,
                    "In Computer",
                );
                ui.selectable_value(
                    &mut cfg.compensation,
                    CompensationType::InControl,
                    "In Control",
                );
            });
        ui.end_row();
    });
    ui.add_space(8.0);
    // G-THROUGHCUT: a through cut with no tabs opens the Tabs disclosure by
    // default. `default_open` only reads on the header's first frame, so the
    // open is also forced on the frame the condition BECOMES true (depth
    // dragged up to the board, tabs set back to zero). Every other frame
    // passes `None`, which leaves the stored state — and the operator's
    // collapse — alone. Same id as the old `ui.collapsing("Tabs", ..)`.
    let tabs_default_open = through_cut.is_some() && cfg.tab_count == 0;
    let tabs_seen_id = ui.id().with("prof_tabs_through_cut_seen");
    let tabs_seen_open: Option<bool> = ui.data(|d| d.get_temp(tabs_seen_id));
    ui.data_mut(|d| d.insert_temp(tabs_seen_id, tabs_default_open));
    let tabs_force_open = (tabs_default_open && tabs_seen_open != Some(true)).then_some(true);
    egui::CollapsingHeader::new("Tabs")
        .default_open(tabs_default_open)
        .open(tabs_force_open)
        .show(ui, |ui| {
            ui.param_grid("tab_p", |ui| {
                ui.label("Count:");
                let mut count = cfg.tab_count as i32;
                if ui
                    .add(egui::DragValue::new(&mut count).range(0..=20))
                    .changed()
                {
                    cfg.tab_count = count.max(0) as usize;
                }
                ui.end_row();
                if cfg.tab_count > 0 {
                    dv(ui, "Width:", &mut cfg.tab_width, " mm", 0.5, 1.0..=50.0);
                    dv(ui, "Height:", &mut cfg.tab_height, " mm", 0.5, 0.5..=20.0);
                }
            });
            if cfg.tab_count > 0 {
                ui.add_space(4.0);
                draw_tab_diagram(ui, cfg.tab_count, cfg.tab_width, cfg.tab_height);
            }
        });
    ui.param_grid("prof_finish", |ui| {
        ui.label("Finishing Passes:");
        let mut fp = cfg.finishing_passes as i32;
        if ui
            .add(egui::DragValue::new(&mut fp).range(0..=10).speed(0.1))
            .on_hover_text("Spring passes at final depth for dimensional accuracy")
            .changed()
        {
            cfg.finishing_passes = fp.max(0) as usize;
        }
        ui.end_row();
    });
}

pub(in crate::ui::properties) fn draw_adaptive_params(
    ui: &mut egui::Ui,
    cfg: &mut AdaptiveConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    ui.param_grid("adapt_p", |ui| {
        dv_pill(
            ui,
            "Stepover:",
            &mut cfg.stepover,
            " mm",
            0.1,
            0.05..=50.0,
            stepover_sugg,
        );
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        depth_caution_row(ui, depth_caution);
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
            "Tolerance:",
            &mut cfg.tolerance,
            " mm",
            0.01,
            0.01..=1.0,
        );
        ui.label("Slot Clearing:");
        ui.checkbox(&mut cfg.slot_clearing, "");
        ui.end_row();
        dv(
            ui,
            "Min Cut Radius:",
            &mut cfg.min_cutting_radius,
            " mm",
            0.1,
            0.0..=50.0,
        );
        ui.label("Cleanup:");
        egui::ComboBox::from_id_salt("adaptive_cleanup_strategy")
            .selected_text(format!("{:?}", cfg.cleanup_strategy))
            .show_ui(ui, |ui| {
                use rs_cam_core::adaptive::CleanupStrategy;
                ui.selectable_value(
                    &mut cfg.cleanup_strategy,
                    CleanupStrategy::ContourParallelHybrid,
                    "ContourParallelHybrid (default)",
                );
                ui.selectable_value(
                    &mut cfg.cleanup_strategy,
                    CleanupStrategy::ContourParallelNarrow,
                    "ContourParallelNarrow",
                );
                ui.selectable_value(
                    &mut cfg.cleanup_strategy,
                    CleanupStrategy::ResidueMop,
                    "ResidueMop",
                );
                ui.selectable_value(&mut cfg.cleanup_strategy, CleanupStrategy::Legacy, "Legacy");
            });
        ui.end_row();
    });
}

pub(in crate::ui::properties) fn draw_vcarve_params(
    ui: &mut egui::Ui,
    cfg: &mut VCarveConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    // V-carve uses `max_depth` as the axial limit and `stepover` as the
    // lateral step. Map LUT axial → max_depth, radial → stepover.
    let max_depth_sugg = pills.map(PillSuggestions::depth_per_pass);
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    ui.param_grid("vcarve_p", |ui| {
        dv_pill(
            ui,
            "Max Depth:",
            &mut cfg.max_depth,
            " mm",
            0.1,
            0.1..=50.0,
            max_depth_sugg,
        );
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            "Stepover:",
            &mut cfg.stepover,
            " mm",
            0.05,
            0.01..=10.0,
            stepover_sugg,
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

pub(in crate::ui::properties) fn draw_rest_params(
    ui: &mut egui::Ui,
    cfg: &mut RestConfig,
    tools: &[(crate::state::job::ToolId, String, f64)],
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    ui.param_grid("rest_p", |ui| {
        ui.label("Previous Tool:");
        let prev_label = cfg
            .prev_tool_id
            .and_then(|pid| tools.iter().find(|(id, _, _)| *id == pid))
            .map(|(_, s, _)| s.as_str())
            .unwrap_or("(select)");
        egui::ComboBox::from_id_salt("rest_prev_tool")
            .selected_text(prev_label)
            .show_ui(ui, |ui| {
                for (id, name, _) in tools {
                    let selected = cfg.prev_tool_id == Some(*id);
                    if ui.selectable_label(selected, name.as_str()).clicked() {
                        cfg.prev_tool_id = Some(*id);
                    }
                }
            });
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
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            "Depth/Pass:",
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            0.1..=50.0,
            dpp_sugg,
        );
        dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
    });
}

pub(in crate::ui::properties) fn draw_inlay_params(
    ui: &mut egui::Ui,
    cfg: &mut InlayConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    // Spec: only the stepover field maps to LUT radial_width. The other
    // inlay fields (pocket_depth, glue_gap, flat_depth, boundary_offset,
    // flat_tool_radius) are geometry-driven, not feeds-driven.
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    ui.param_grid("inlay_p", |ui| {
        dv(
            ui,
            "Pocket Depth:",
            &mut cfg.pocket_depth,
            " mm",
            0.1,
            0.1..=50.0,
        );
        depth_caution_row(ui, depth_caution);
        dv(ui, "Glue Gap:", &mut cfg.glue_gap, " mm", 0.01, 0.0..=2.0);
        dv(
            ui,
            "Flat Depth:",
            &mut cfg.flat_depth,
            " mm",
            0.1,
            0.0..=20.0,
        );
        dv(
            ui,
            "Boundary Offset:",
            &mut cfg.boundary_offset,
            " mm",
            0.05,
            0.0..=10.0,
        );
        dv_pill(
            ui,
            "Stepover:",
            &mut cfg.stepover,
            " mm",
            0.1,
            0.05..=50.0,
            stepover_sugg,
        );
        dv(
            ui,
            "Flat Tool Radius:",
            &mut cfg.flat_tool_radius,
            " mm",
            0.1,
            0.1..=50.0,
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

pub(in crate::ui::properties) fn draw_zigzag_params(
    ui: &mut egui::Ui,
    cfg: &mut ZigzagConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    ui.param_grid("zigzag_p", |ui| {
        dv_pill(
            ui,
            "Stepover:",
            &mut cfg.stepover,
            " mm",
            0.1,
            0.05..=50.0,
            stepover_sugg,
        );
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            "Depth/Pass:",
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            0.1..=50.0,
            dpp_sugg,
        );
        dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
    });
}
