use super::super::pills::PillSuggestions;

use crate::state::toolpath::{
    AdaptiveConfig, CompensationType, FaceConfig, FaceDirection, InlayConfig, InlayPlugConfig,
    PocketConfig, PocketPattern, ProfileConfig, ProfileSide, RegionOrdering, VCarveConfig,
    ZigzagConfig,
};

use super::super::{advanced, depth_caution_row, dv, dv_pill, p, schema_range, through_cut_row};
use super::{DepthBeyondStock, ThroughCut, draw_tab_diagram};
use crate::ui::components::UiExt as _;
use rs_cam_core::compute::catalog::OperationType;

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
            .truncate()
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
            p(OperationType::Face, "stepover", "Stepover:"),
            &mut cfg.stepover,
            " mm",
            0.5,
            stepover_sugg,
        );
        dv(
            ui,
            p(OperationType::Face, "depth", "Depth:"),
            &mut cfg.depth,
            " mm",
            0.1,
        );
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            p(OperationType::Face, "depth_per_pass", "Depth/Pass:"),
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            dpp_sugg,
        );
        dv(
            ui,
            p(OperationType::Face, "stock_offset", "Stock Offset:"),
            &mut cfg.stock_offset,
            " mm",
            0.5,
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
            .truncate()
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
            p(OperationType::Pocket, "stepover", "Stepover:"),
            &mut cfg.stepover,
            " mm",
            0.1,
            stepover_sugg,
        );
        dv(
            ui,
            p(OperationType::Pocket, "depth", "Depth:"),
            &mut cfg.depth,
            " mm",
            0.1,
        );
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            p(OperationType::Pocket, "depth_per_pass", "Depth/Pass:"),
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            dpp_sugg,
        );
        ui.label("Climb:");
        ui.checkbox(&mut cfg.climb, "");
        ui.end_row();
        region_ordering_row(ui, "pocket_ord", &mut cfg.region_ordering);
        if cfg.pattern == PocketPattern::Zigzag {
            dv(
                ui,
                p(OperationType::Pocket, "angle", "Angle:"),
                &mut cfg.angle,
                " deg",
                1.0,
            );
        }
    });
    advanced(ui, "pocket_adv", |ui| {
        finishing_passes_row(ui, OperationType::Pocket, &mut cfg.finishing_passes);
    });
}

/// The "Finishing Passes" row of Pocket and Profile.
///
/// UI-10: an integer field. `ValueRow` edits an `f64`, and a temporary f64
/// would change what a partial edit commits.
fn finishing_passes_row(ui: &mut egui::Ui, op: OperationType, passes: &mut usize) {
    ui.label("Finishing Passes:");
    let mut fp = *passes as i32;
    if ui
        .add(
            egui::DragValue::new(&mut fp)
                .range(schema_range(op, "finishing_passes"))
                .speed(0.1),
        )
        .on_hover_text("Spring passes at final depth for dimensional accuracy")
        .changed()
    {
        *passes = fp.max(0) as usize;
    }
    ui.end_row();
}

/// The "Ordering" row of a Layered 2.5D op (ruling R4): Pocket, Zigzag,
/// Trace and Profile. Region-first (By Area) is the default.
pub(in crate::ui::properties) fn region_ordering_row(
    ui: &mut egui::Ui,
    salt: &str,
    ordering: &mut RegionOrdering,
) {
    ui.label("Ordering:");
    egui::ComboBox::from_id_salt(salt)
        .truncate()
        .selected_text(match ordering {
            RegionOrdering::Global => "Global",
            RegionOrdering::ByArea => "By Area",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(ordering, RegionOrdering::ByArea, "By Area")
                .on_hover_text("Cut each outline to full depth before the next.");
            ui.selectable_value(ordering, RegionOrdering::Global, "Global")
                .on_hover_text("Cut every outline at one depth before the next depth.");
        });
    ui.end_row();
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
            .truncate()
            .selected_text(match cfg.side {
                ProfileSide::Outside => "Outside",
                ProfileSide::Inside => "Inside",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut cfg.side, ProfileSide::Outside, "Outside");
                ui.selectable_value(&mut cfg.side, ProfileSide::Inside, "Inside");
            });
        ui.end_row();
        dv(
            ui,
            p(OperationType::Profile, "depth", "Depth:"),
            &mut cfg.depth,
            " mm",
            0.1,
        );
        through_cut_row(ui, through_cut);
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            p(OperationType::Profile, "depth_per_pass", "Depth/Pass:"),
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            dpp_sugg,
        );
        ui.label("Climb:");
        ui.checkbox(&mut cfg.climb, "");
        ui.end_row();
        region_ordering_row(ui, "prof_ord", &mut cfg.region_ordering);
    });
    ui.add_space(8.0);
    // G-THROUGHCUT: a through cut with no tabs opens the Tabs disclosure by
    // default. `default_open` only reads on the header's first frame, so the
    // open is also forced on the frame the condition BECOMES true (depth
    // dragged up to the board, tabs set back to zero). Every other frame
    // passes `None`, which leaves the stored state — and the operator's
    // collapse — alone. Same id as the old `ui.collapsing("Tabs", ..)`.
    draw_profile_tabs(ui, cfg, through_cut);
    advanced(ui, "profile_adv", |ui| {
        ui.label("Compensation:");
        egui::ComboBox::from_id_salt("prof_comp")
            .truncate()
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
        finishing_passes_row(ui, OperationType::Profile, &mut cfg.finishing_passes);
    });
}

/// The Tabs disclosure of Profile, with its diagram.
fn draw_profile_tabs(ui: &mut egui::Ui, cfg: &mut ProfileConfig, through_cut: Option<&ThroughCut>) {
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
                // UI-10: an integer field. `ValueRow` edits an `f64`, and a
                // temporary f64 would change what a partial edit commits.
                ui.label("Count:");
                let mut count = cfg.tab_count as i32;
                if ui
                    .add(
                        egui::DragValue::new(&mut count)
                            .range(schema_range(OperationType::Profile, "tab_count")),
                    )
                    .changed()
                {
                    cfg.tab_count = count.max(0) as usize;
                }
                ui.end_row();
                if cfg.tab_count > 0 {
                    dv(
                        ui,
                        p(OperationType::Profile, "tab_width", "Width:"),
                        &mut cfg.tab_width,
                        " mm",
                        0.5,
                    );
                    dv(
                        ui,
                        p(OperationType::Profile, "tab_height", "Height:"),
                        &mut cfg.tab_height,
                        " mm",
                        0.5,
                    );
                }
            });
            if cfg.tab_count > 0 {
                ui.add_space(4.0);
                draw_tab_diagram(ui, cfg.tab_count, cfg.tab_width, cfg.tab_height);
            }
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
            p(OperationType::Adaptive, "stepover", "Stepover:"),
            &mut cfg.stepover,
            " mm",
            0.1,
            stepover_sugg,
        );
        dv(
            ui,
            p(OperationType::Adaptive, "depth", "Depth:"),
            &mut cfg.depth,
            " mm",
            0.1,
        );
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            p(OperationType::Adaptive, "depth_per_pass", "Depth/Pass:"),
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            dpp_sugg,
        );
    });
    advanced(ui, "adapt_adv", |ui| {
        dv(
            ui,
            p(OperationType::Adaptive, "tolerance", "Tolerance:"),
            &mut cfg.tolerance,
            " mm",
            0.01,
        );
        ui.label("Slot Clearing:");
        ui.checkbox(&mut cfg.slot_clearing, "").on_hover_text(
            "Cut full-width seeding slots before the adaptive passes. A slot \
             line runs at full radial immersion, the one 2D Adaptive cut \
             outside the pass load (every other cut holds the stepover as \
             radial width). Off by default; a project saved with it on keeps \
             it.",
        );
        ui.end_row();
        dv(
            ui,
            p(
                OperationType::Adaptive,
                "min_cutting_radius",
                "Min Cut Radius:",
            ),
            &mut cfg.min_cutting_radius,
            " mm",
            0.1,
        );
        ui.label("Cleanup:");
        egui::ComboBox::from_id_salt("adaptive_cleanup_strategy")
            .truncate()
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
            p(OperationType::VCarve, "max_depth", "Max Depth:"),
            &mut cfg.max_depth,
            " mm",
            0.1,
            max_depth_sugg,
        );
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            p(OperationType::VCarve, "stepover", "Stepover:"),
            &mut cfg.stepover,
            " mm",
            0.05,
            stepover_sugg,
        );
    });
    advanced(ui, "vcarve_adv", |ui| {
        dv(
            ui,
            p(OperationType::VCarve, "tolerance", "Tolerance:"),
            &mut cfg.tolerance,
            " mm",
            0.01,
        );
    });
}

pub(in crate::ui::properties) fn draw_inlay_params(
    ui: &mut egui::Ui,
    cfg: &mut InlayConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    // Spec: only the stepover field maps to LUT radial_width. The other
    // inlay fields (pocket_depth, floor_overlap) are geometry-driven, not
    // feeds-driven.
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    ui.param_grid("inlay_p", |ui| {
        dv(
            ui,
            p(OperationType::Inlay, "pocket_depth", "Pocket Depth:"),
            &mut cfg.pocket_depth,
            " mm",
            0.1,
        );
        depth_caution_row(ui, depth_caution);
        dv(
            ui,
            p(OperationType::Inlay, "floor_overlap", "Floor Overlap:"),
            &mut cfg.floor_overlap,
            " mm",
            0.1,
        );
        dv(
            ui,
            p(OperationType::Inlay, "floor_ridge", "Floor Ridge:"),
            &mut cfg.floor_ridge,
            " mm",
            0.01,
        );
        dv_pill(
            ui,
            p(OperationType::Inlay, "stepover", "Stepover:"),
            &mut cfg.stepover,
            " mm",
            0.1,
            stepover_sugg,
        );
    });
    advanced(ui, "inlay_adv", |ui| {
        dv(
            ui,
            p(OperationType::Inlay, "tolerance", "Tolerance:"),
            &mut cfg.tolerance,
            " mm",
            0.01,
        );
    });
}

pub(in crate::ui::properties) fn draw_inlay_plug_params(
    ui: &mut egui::Ui,
    cfg: &mut InlayPlugConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    // Only stepover and depth_per_pass map to the LUT; the other plug
    // fields are geometry-driven.
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    ui.param_grid("inlay_plug_p", |ui| {
        dv(
            ui,
            p(OperationType::InlayPlug, "pocket_depth", "Pocket Depth:"),
            &mut cfg.pocket_depth,
            " mm",
            0.1,
        );
        depth_caution_row(ui, depth_caution);
        dv(
            ui,
            p(OperationType::InlayPlug, "glue_gap", "Glue Gap:"),
            &mut cfg.glue_gap,
            " mm",
            0.01,
        );
        dv(
            ui,
            p(OperationType::InlayPlug, "proud_height", "Proud Height:"),
            &mut cfg.proud_height,
            " mm",
            0.1,
        );
        dv(
            ui,
            p(OperationType::InlayPlug, "outer_margin", "Outer Margin:"),
            &mut cfg.outer_margin,
            " mm",
            0.1,
        );
        dv_pill(
            ui,
            p(OperationType::InlayPlug, "depth_per_pass", "Depth/Pass:"),
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            dpp_sugg,
        );
        dv_pill(
            ui,
            p(OperationType::InlayPlug, "stepover", "Stepover:"),
            &mut cfg.stepover,
            " mm",
            0.05,
            stepover_sugg,
        );
    });
    advanced(ui, "inlay_plug_adv", |ui| {
        dv(
            ui,
            p(OperationType::InlayPlug, "tolerance", "Tolerance:"),
            &mut cfg.tolerance,
            " mm",
            0.01,
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
            p(OperationType::Zigzag, "stepover", "Stepover:"),
            &mut cfg.stepover,
            " mm",
            0.1,
            stepover_sugg,
        );
        dv(
            ui,
            p(OperationType::Zigzag, "depth", "Depth:"),
            &mut cfg.depth,
            " mm",
            0.1,
        );
        depth_caution_row(ui, depth_caution);
        dv_pill(
            ui,
            p(OperationType::Zigzag, "depth_per_pass", "Depth/Pass:"),
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            dpp_sugg,
        );
        dv(
            ui,
            p(OperationType::Zigzag, "angle", "Angle:"),
            &mut cfg.angle,
            " deg",
            1.0,
        );
        region_ordering_row(ui, "zigzag_ord", &mut cfg.region_ordering);
    });
}
