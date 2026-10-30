use super::super::pills::PillSuggestions;

use crate::state::toolpath::{
    AdaptiveConfig, CompensationType, FaceConfig, FaceDirection, InlayConfig, InlayPlugConfig,
    PocketConfig, PocketPattern, ProfileConfig, ProfileSide, RegionOrdering, VCarveConfig,
    ZigzagConfig,
};

use super::super::linking_dressup::{
    Fields, check_in_row, check_param, combo_control, combo_in_row, combo_param, count_in_row,
    count_param, plain_row,
};
use super::super::{advanced, dv, dv_opt, dv_pill, dv_pill_opt, p, through_cut_line};
use super::{DepthBeyondStock, ThroughCut, draw_tab_diagram};
use crate::ui::components::{Button, OneLine, PropRow};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::operation_configs::OpenPaths;

pub(in crate::ui::properties) fn draw_face_params(
    ui: &mut egui::Ui,
    cfg: &mut FaceConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
    fields: Fields<'_>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    // G-DEPTHSTOCK: the caution is the outline of the Depth row.
    let fields = fields.with_depth_caution("depth", depth_caution);
    combo_in_row(
        ui,
        plain_row(
            "Direction",
            Some("One way cuts every pass in one direction. Zigzag cuts in both directions."),
        ),
        "face_dir",
        match cfg.direction {
            FaceDirection::OneWay => "One way",
            FaceDirection::Zigzag => "Zigzag",
        },
        |ui| {
            ui.selectable_value(&mut cfg.direction, FaceDirection::OneWay, "One way");
            ui.selectable_value(&mut cfg.direction, FaceDirection::Zigzag, "Zigzag");
        },
    );
    dv_pill(
        ui,
        p(OperationType::Face, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.5,
        stepover_sugg,
        fields,
    );
    dv(
        ui,
        p(OperationType::Face, "depth", "Depth"),
        &mut cfg.depth,
        " mm",
        0.1,
        fields,
    );
    dv_pill(
        ui,
        p(OperationType::Face, "depth_per_pass", "Depth/pass"),
        &mut cfg.depth_per_pass,
        " mm",
        0.1,
        dpp_sugg,
        fields,
    );
    // S11: empty = the tool radius.
    dv_opt(
        ui,
        p(OperationType::Face, "stock_offset", "Stock offset"),
        &mut cfg.stock_offset,
        " mm",
        0.5,
        Some("tool R"),
        fields,
    );
}

pub(in crate::ui::properties) fn draw_pocket_params(
    ui: &mut egui::Ui,
    cfg: &mut PocketConfig,
    pills: Option<&PillSuggestions>,
    model_areas: bool,
    fields: Fields<'_>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    combo_in_row(
        ui,
        plain_row(
            "Pattern",
            Some("Contour follows the outline inward. Zigzag cuts parallel lines."),
        ),
        "pocket_pat",
        match cfg.pattern {
            PocketPattern::Contour => "Contour",
            PocketPattern::Zigzag => "Zigzag",
        },
        |ui| {
            ui.selectable_value(&mut cfg.pattern, PocketPattern::Contour, "Contour");
            ui.selectable_value(&mut cfg.pattern, PocketPattern::Zigzag, "Zigzag");
        },
    );
    dv_pill(
        ui,
        p(OperationType::Pocket, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.1,
        stepover_sugg,
        fields,
    );
    dv_pill(
        ui,
        p(OperationType::Pocket, "depth_per_pass", "Depth/pass"),
        &mut cfg.depth_per_pass,
        " mm",
        0.1,
        dpp_sugg,
        fields,
    );
    check_in_row(
        ui,
        plain_row(
            "Climb",
            Some("Climb milling: the cutter edge meets the material in the direction of the feed."),
        ),
        &mut cfg.climb,
    );
    region_ordering_row(ui, "pocket_ord", &mut cfg.region_ordering);
    open_paths_rows(ui, &mut cfg.open_paths, fields);
    // A drawing Pocket has no face, so it shows no row. The row stays
    // while the setting is on, so the operator can turn it off.
    if model_areas || cfg.cut_past_open_sides {
        open_sides_row(ui, &mut cfg.cut_past_open_sides, fields);
    }
    if cfg.pattern == PocketPattern::Zigzag {
        dv(
            ui,
            p(OperationType::Pocket, "angle", "Angle"),
            &mut cfg.angle,
            " deg",
            1.0,
            fields,
        );
    }
    advanced(ui, "pocket_adv", |ui| {
        finishing_passes_row(ui, OperationType::Pocket, &mut cfg.finishing_passes);
    });
}

/// The text of the Flip button of the Pocket open paths side (P6.7). The
/// sketch shows U+21C4; the inspector fonts do not hold that glyph, so the
/// button uses U+2194.
const FLIP_LABEL: &str = "\u{2194} Flip";

/// The hover of the Side row and its [`FLIP_LABEL`] action.
const OPEN_SIDE_HOVER: &str = "The side of the drawn direction that Extend to stock cuts. \
    Flip cuts the area on the other side of each open path.";

/// The side of an open path that Extend to stock cuts, the other side
/// (P6.7: the Flip button). `Skip` has no side and stays `Skip`.
fn flipped(open_paths: OpenPaths) -> OpenPaths {
    match open_paths {
        OpenPaths::Skip => OpenPaths::Skip,
        OpenPaths::ExtendLeft => OpenPaths::ExtendRight,
        OpenPaths::ExtendRight => OpenPaths::ExtendLeft,
    }
}

/// The "Open paths" rows of Pocket (geometry selection P6.7, ruling O-2):
/// Skip or Extend to stock; under Extend, the side of the drawn direction
/// and the Flip button. The rows write the draft; the panel sends one
/// `ReplaceToolpathConfig`. The counts and a refusal are in the Selection
/// section, which owns the report.
fn open_paths_rows(ui: &mut egui::Ui, open_paths: &mut OpenPaths, fields: Fields<'_>) {
    let extend = *open_paths != OpenPaths::Skip;
    combo_param(
        ui,
        p(OperationType::Pocket, "open_paths", "Open paths"),
        fields,
        "pocket_open_paths",
        if extend { "Extend to stock" } else { "Skip" },
        |ui| {
            if ui.selectable_label(!extend, "Skip").clicked() {
                *open_paths = OpenPaths::Skip;
            }
            // O-2: the area on the left of the drawn direction first; Flip
            // gives the right.
            if ui.selectable_label(extend, "Extend to stock").clicked() && !extend {
                *open_paths = OpenPaths::ExtendLeft;
            }
        },
    )
    .on_hover_text(open_paths.label());
    if *open_paths == OpenPaths::Skip {
        return;
    }
    // The Flip button is the quiet action of the Side row.
    let out = plain_row("Side", Some(OPEN_SIDE_HOVER))
        .action(FLIP_LABEL)
        .show(ui, |ui| {
            combo_control(ui, "pocket_open_side", open_side_label(*open_paths), |ui| {
                for side in [OpenPaths::ExtendLeft, OpenPaths::ExtendRight] {
                    ui.selectable_value(open_paths, side, open_side_label(side));
                }
            })
        });
    let _ = out.inner.on_hover_text(&out.hover);
    if out.action_clicked {
        *open_paths = flipped(*open_paths);
    }
}

/// The label of the Pocket "Cut past open sides" row (operator ruling
/// 2026-10-23). The hover is the registry help of the parameter. W3g: the
/// label is short, so it fits the label column at 340 points.
pub(in crate::ui::properties) const OPEN_SIDES_LABEL: &str = "Past open sides";

/// The "Cut past open sides" row of Pocket (operator ruling 2026-10-23):
/// one checkbox. On, the area of each model face grows out through each
/// side with no wall, to the stock edge. The row writes the draft; the
/// panel sends one `ReplaceToolpathConfig`.
fn open_sides_row(ui: &mut egui::Ui, on: &mut bool, fields: Fields<'_>) {
    check_param(
        ui,
        p(
            OperationType::Pocket,
            "cut_past_open_sides",
            OPEN_SIDES_LABEL,
        ),
        on,
        fields,
    );
}

/// The Side label of an Extend choice (P6.7).
fn open_side_label(open_paths: OpenPaths) -> &'static str {
    match open_paths {
        OpenPaths::Skip => "Skip",
        OpenPaths::ExtendLeft => "Left of the drawn direction",
        OpenPaths::ExtendRight => "Right of the drawn direction",
    }
}

/// The "Finishing passes" row of Pocket and Profile.
fn finishing_passes_row(ui: &mut egui::Ui, op: OperationType, passes: &mut usize) {
    count_in_row(
        ui,
        plain_row(
            "Finishing passes",
            Some("Spring passes at final depth for dimensional accuracy."),
        ),
        passes,
        0.1,
        (op, "finishing_passes"),
    );
}

/// The "Ordering" row of a Layered 2.5D op (ruling R4): Pocket, Zigzag,
/// Trace and Profile. Region-first (By Area) is the default.
pub(in crate::ui::properties) fn region_ordering_row(
    ui: &mut egui::Ui,
    salt: &str,
    ordering: &mut RegionOrdering,
) {
    combo_in_row(
        ui,
        plain_row(
            "Ordering",
            Some("The order of the outlines and the depths."),
        ),
        salt,
        match ordering {
            RegionOrdering::Global => "Global",
            RegionOrdering::ByArea => "By Area",
        },
        |ui| {
            ui.selectable_value(ordering, RegionOrdering::ByArea, "By Area")
                .on_hover_text("Cut each outline to full depth before the next.");
            ui.selectable_value(ordering, RegionOrdering::Global, "Global")
                .on_hover_text("Cut every outline at one depth before the next depth.");
        },
    );
}

/// The hover of the greyed "In Control" entry under side On (P6.7).
const IN_CONTROL_UNDER_ON: &str = "side On has no side for G41/G42";

/// The hint beside Climb under side On (P6.7, design 2.2).
const CLIMB_ON_HINT: &str = "open paths cut in drawn direction";

/// The hover of [`CLIMB_ON_HINT`].
const CLIMB_ON_DETAIL: &str = "An open path is cut from its first point to its last point. \
    Climb sets the direction of the closed loops only.";

/// The Tabs header under side On (P6.7, design 2.5, L-8).
const TABS_ON_HEADER: &str = "Tabs \u{00B7} closed loops only";

/// The hover of the "On the line" side (P6.7).
const SIDE_ON_HOVER: &str = "The tool centre follows the line: closed loops and open paths. \
    Compensation In Control changes to In Computer, because side On has no side for G41/G42.";

/// True when `cfg` with compensation In Control is refused: the core
/// predicate of the door (`domain::side_on_in_control`).
fn in_control_refused(cfg: &ProfileConfig) -> bool {
    let candidate = ProfileConfig {
        compensation: CompensationType::InControl,
        ..cfg.clone()
    };
    rs_cam_core::compute::domain::side_on_in_control(
        &rs_cam_core::compute::catalog::OperationConfig::Profile(candidate),
    )
}

/// The refusal text when `cfg` with side On is refused for its floor (the
/// core predicate `domain::side_on_each_face`), or `None`.
fn side_on_refusal(cfg: &ProfileConfig) -> Option<String> {
    let candidate = rs_cam_core::compute::catalog::OperationConfig::Profile(ProfileConfig {
        side: ProfileSide::On,
        ..cfg.clone()
    });
    rs_cam_core::compute::domain::side_on_each_face(&candidate).then(|| {
        rs_cam_core::compute::domain::OpDomainRefusal::SideOnEachFace {
            op: candidate.label(),
        }
        .to_string()
    })
}

/// The Side row of Profile (P6.7): Outside, Inside, On the line.
///
/// A pick of "On the line" under In Control sets In Computer in the same
/// draft, so the one write passes the door (`domain::side_on_in_control`).
/// "On the line" is greyed while the floor is Each picked face
/// (`domain::side_on_each_face`), with the core refusal as the hover.
fn profile_side_row(ui: &mut egui::Ui, cfg: &mut ProfileConfig) {
    let on_refusal = side_on_refusal(cfg);
    let before = cfg.side;
    combo_in_row(
        ui,
        plain_row("Side", Some("The side of the outline that the tool cuts.")),
        "prof_side",
        profile_side_label(cfg.side),
        |ui| {
            for side in [ProfileSide::Outside, ProfileSide::Inside] {
                ui.selectable_value(&mut cfg.side, side, profile_side_label(side));
            }
            ui.add_enabled_ui(on_refusal.is_none(), |ui| {
                ui.selectable_value(
                    &mut cfg.side,
                    ProfileSide::On,
                    profile_side_label(ProfileSide::On),
                )
                .on_hover_text(SIDE_ON_HOVER)
                .on_disabled_hover_text(on_refusal.as_deref().unwrap_or_default());
            });
        },
    );
    if cfg.side == ProfileSide::On
        && before != ProfileSide::On
        && cfg.compensation == CompensationType::InControl
    {
        cfg.compensation = CompensationType::InComputer;
    }
}

/// The label of a Profile side (P6.7).
fn profile_side_label(side: ProfileSide) -> &'static str {
    match side {
        ProfileSide::Outside => "Outside",
        ProfileSide::Inside => "Inside",
        ProfileSide::On => "On the line",
    }
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
    through_cut: Option<&ThroughCut>,
    tab_editor: (
        crate::state::toolpath::ToolpathId,
        &mut crate::state::pick::SelectionUi,
    ),
    fields: Fields<'_>,
) {
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    profile_side_row(ui, cfg);
    through_cut_line(ui, through_cut);
    dv_pill(
        ui,
        p(OperationType::Profile, "depth_per_pass", "Depth/pass"),
        &mut cfg.depth_per_pass,
        " mm",
        0.1,
        dpp_sugg,
        fields,
    );
    check_in_row(
        ui,
        plain_row(
            "Climb",
            Some("Climb milling: the cutter edge meets the material in the direction of the feed."),
        ),
        &mut cfg.climb,
    );
    // P6.7 (design 2.2): Climb does not act on an open path.
    if cfg.side == ProfileSide::On {
        let _ = PropRow::note(ui, OneLine::new(CLIMB_ON_HINT).detail(CLIMB_ON_DETAIL));
    }
    region_ordering_row(ui, "prof_ord", &mut cfg.region_ordering);
    ui.add_space(8.0);
    // G-THROUGHCUT: a through cut with no tabs opens the Tabs disclosure by
    // default. `default_open` only reads on the header's first frame, so the
    // open is also forced on the frame the condition BECOMES true (depth
    // dragged up to the board, tabs set back to zero). Every other frame
    // passes `None`, which leaves the stored state — and the operator's
    // collapse — alone. Same id as the old `ui.collapsing("Tabs", ..)`.
    draw_profile_tabs(ui, cfg, through_cut, tab_editor, fields);
    advanced(ui, "profile_adv", |ui| {
        // P6.7: under side On, In Control is greyed. Core refuses the pair
        // at every door (`domain::side_on_in_control`), so the combo reads
        // that predicate and does not state a second rule.
        let in_control_refused = in_control_refused(cfg);
        combo_in_row(
            ui,
            plain_row(
                "Compensation",
                Some(
                    "In Computer offsets the path in the G-code. In Control writes G41/G42, and the controller offsets the path.",
                ),
            ),
            "prof_comp",
            match cfg.compensation {
                CompensationType::InComputer => "In Computer",
                CompensationType::InControl => "In Control",
            },
            |ui| {
                ui.selectable_value(
                    &mut cfg.compensation,
                    CompensationType::InComputer,
                    "In Computer",
                );
                ui.add_enabled_ui(!in_control_refused, |ui| {
                    ui.selectable_value(
                        &mut cfg.compensation,
                        CompensationType::InControl,
                        "In Control",
                    )
                    .on_disabled_hover_text(IN_CONTROL_UNDER_ON);
                });
            },
        );
        finishing_passes_row(ui, OperationType::Profile, &mut cfg.finishing_passes);
    });
}

/// The hover of the `Edit in viewport` button of the Tabs row (P4.6).
const TAB_EDIT_HOVER: &str = "Show the tabs in the viewport. Click a loop to add a tab, drag a tab \
     to move it, right-click a tab to delete it. An edit of a loop with automatic tabs makes all \
     of its tabs hand tabs. The last tab of a loop cannot be deleted.";

/// The Tabs disclosure of Profile, with its diagram.
fn draw_profile_tabs(
    ui: &mut egui::Ui,
    cfg: &mut ProfileConfig,
    through_cut: Option<&ThroughCut>,
    (toolpath_id, selection_ui): (
        crate::state::toolpath::ToolpathId,
        &mut crate::state::pick::SelectionUi,
    ),
    fields: Fields<'_>,
) {
    let tabs_default_open =
        through_cut.is_some() && cfg.tab_count == 0 && cfg.tab_points.is_empty();
    let tabs_seen_id = ui.id().with("prof_tabs_through_cut_seen");
    let tabs_seen_open: Option<bool> = ui.data(|d| d.get_temp(tabs_seen_id));
    ui.data_mut(|d| d.insert_temp(tabs_seen_id, tabs_default_open));
    let tabs_force_open = (tabs_default_open && tabs_seen_open != Some(true)).then_some(true);
    // P6.7 (L-8): under side On the tabs act on the closed loops only.
    // The id salt keeps the header id of the plain "Tabs" header.
    let header = if cfg.side == ProfileSide::On {
        TABS_ON_HEADER
    } else {
        "Tabs"
    };
    egui::CollapsingHeader::new(header)
        .id_salt("Tabs")
        .default_open(tabs_default_open)
        .open(tabs_force_open)
        .show(ui, |ui| {
            count_param(
                ui,
                p(OperationType::Profile, "tab_count", "Count"),
                &mut cfg.tab_count,
                0.1,
                fields,
            );
            // P5: hand tabs come from `tab_points`. P4.6: the viewport
            // tab editor writes them (`Edit in viewport`); MCP and the
            // project file write them too.
            if cfg.tab_count > 0 || !cfg.tab_points.is_empty() {
                // The count and its Clear share the well; the quiet
                // action turns the viewport tab editor on and off.
                let on = selection_ui.tab_edit(toolpath_id).is_some();
                let out = plain_row("Hand tabs", Some(TAB_EDIT_HOVER))
                    .action(if on {
                        "Stop editing"
                    } else {
                        "Edit in viewport"
                    })
                    .show(ui, |ui| {
                        let _ = crate::ui::components::prop_row::well_value(
                            ui,
                            &cfg.tab_points.len().to_string(),
                        );
                        if !cfg.tab_points.is_empty() {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.add(Button::quiet("Clear")).clicked() {
                                        cfg.tab_points.clear();
                                    }
                                },
                            );
                        }
                    });
                if out.action_clicked {
                    selection_ui.set_tab_edit(toolpath_id, !on);
                }
            }
            if cfg.tab_count > 0 || !cfg.tab_points.is_empty() {
                dv(
                    ui,
                    p(OperationType::Profile, "tab_width", "Bridge width"),
                    &mut cfg.tab_width,
                    " mm",
                    0.5,
                    fields,
                );
                dv(
                    ui,
                    p(OperationType::Profile, "tab_height", "Height"),
                    &mut cfg.tab_height,
                    " mm",
                    0.5,
                    fields,
                );
            }
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
    fields: Fields<'_>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    dv_pill(
        ui,
        p(OperationType::Adaptive, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.1,
        stepover_sugg,
        fields,
    );
    dv_pill(
        ui,
        p(OperationType::Adaptive, "depth_per_pass", "Depth/pass"),
        &mut cfg.depth_per_pass,
        " mm",
        0.1,
        dpp_sugg,
        fields,
    );
    advanced(ui, "adapt_adv", |ui| {
        dv(
            ui,
            p(OperationType::Adaptive, "tolerance", "Tolerance"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            fields,
        );
        check_in_row(
            ui,
            plain_row(
                "Slot clearing",
                Some(
                    "Cut full-width seeding slots before the adaptive passes. A slot \
                     line runs at full radial immersion, the one 2D Adaptive cut \
                     outside the pass load (every other cut holds the stepover as \
                     radial width). Off by default; a project saved with it on keeps \
                     it.",
                ),
            ),
            &mut cfg.slot_clearing,
        );
        dv(
            ui,
            p(
                OperationType::Adaptive,
                "min_cutting_radius",
                "Min cut radius",
            ),
            &mut cfg.min_cutting_radius,
            " mm",
            0.1,
            fields,
        );
        combo_in_row(
            ui,
            plain_row(
                "Cleanup",
                Some("How the adaptive clears the material that its passes leave."),
            ),
            "adaptive_cleanup_strategy",
            format!("{:?}", cfg.cleanup_strategy),
            |ui| {
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
            },
        );
    });
}

pub(in crate::ui::properties) fn draw_vcarve_params(
    ui: &mut egui::Ui,
    cfg: &mut VCarveConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
    fields: Fields<'_>,
) {
    // G-DEPTHSTOCK: the caution is the outline of the depth row.
    let fields = fields.with_depth_caution("max_depth", depth_caution);
    // V-carve uses `max_depth` as the axial limit and `stepover` as the
    // lateral step. Map LUT axial → max_depth, radial → stepover.
    let max_depth_sugg = pills.map(PillSuggestions::depth_per_pass);
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    dv_pill_opt(
        ui,
        p(OperationType::VCarve, "max_depth", "Max depth"),
        &mut cfg.max_depth,
        " mm",
        0.1,
        max_depth_sugg,
        fields,
    );
    dv_pill(
        ui,
        p(OperationType::VCarve, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.05,
        stepover_sugg,
        fields,
    );
    advanced(ui, "vcarve_adv", |ui| {
        dv(
            ui,
            p(OperationType::VCarve, "tolerance", "Tolerance"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            fields,
        );
    });
}

pub(in crate::ui::properties) fn draw_inlay_params(
    ui: &mut egui::Ui,
    cfg: &mut InlayConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
    fields: Fields<'_>,
) {
    // G-DEPTHSTOCK: the caution is the outline of the depth row.
    let fields = fields.with_depth_caution("pocket_depth", depth_caution);
    // Spec: only the stepover field maps to LUT radial_width. The other
    // inlay fields (pocket_depth, floor_overlap) are geometry-driven, not
    // feeds-driven.
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    dv_opt(
        ui,
        p(OperationType::Inlay, "pocket_depth", "Pocket depth"),
        &mut cfg.pocket_depth,
        " mm",
        0.1,
        None,
        fields,
    );
    // S11: empty = 0.9 x the tool diameter.
    dv_opt(
        ui,
        p(OperationType::Inlay, "floor_overlap", "Floor overlap"),
        &mut cfg.floor_overlap,
        " mm",
        0.1,
        Some("0.9 x D"),
        fields,
    );
    dv(
        ui,
        p(OperationType::Inlay, "floor_ridge", "Floor ridge"),
        &mut cfg.floor_ridge,
        " mm",
        0.01,
        fields,
    );
    dv_pill(
        ui,
        p(OperationType::Inlay, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.1,
        stepover_sugg,
        fields,
    );
    advanced(ui, "inlay_adv", |ui| {
        dv(
            ui,
            p(OperationType::Inlay, "tolerance", "Tolerance"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            fields,
        );
    });
}

pub(in crate::ui::properties) fn draw_inlay_plug_params(
    ui: &mut egui::Ui,
    cfg: &mut InlayPlugConfig,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
    fields: Fields<'_>,
) {
    // G-DEPTHSTOCK: the caution is the outline of the depth row.
    let fields = fields.with_depth_caution("pocket_depth", depth_caution);
    // Only stepover and depth_per_pass map to the LUT; the other plug
    // fields are geometry-driven.
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    dv_opt(
        ui,
        p(OperationType::InlayPlug, "pocket_depth", "Pocket depth"),
        &mut cfg.pocket_depth,
        " mm",
        0.1,
        None,
        fields,
    );
    dv(
        ui,
        p(OperationType::InlayPlug, "glue_gap", "Glue gap"),
        &mut cfg.glue_gap,
        " mm",
        0.01,
        fields,
    );
    dv(
        ui,
        p(OperationType::InlayPlug, "proud_height", "Proud height"),
        &mut cfg.proud_height,
        " mm",
        0.1,
        fields,
    );
    dv(
        ui,
        p(OperationType::InlayPlug, "outer_margin", "Outer margin"),
        &mut cfg.outer_margin,
        " mm",
        0.1,
        fields,
    );
    dv_pill(
        ui,
        p(OperationType::InlayPlug, "depth_per_pass", "Depth/pass"),
        &mut cfg.depth_per_pass,
        " mm",
        0.1,
        dpp_sugg,
        fields,
    );
    dv_pill(
        ui,
        p(OperationType::InlayPlug, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.05,
        stepover_sugg,
        fields,
    );
    advanced(ui, "inlay_plug_adv", |ui| {
        dv(
            ui,
            p(OperationType::InlayPlug, "tolerance", "Tolerance"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            fields,
        );
    });
}

pub(in crate::ui::properties) fn draw_zigzag_params(
    ui: &mut egui::Ui,
    cfg: &mut ZigzagConfig,
    pills: Option<&PillSuggestions>,
    fields: Fields<'_>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    dv_pill(
        ui,
        p(OperationType::Zigzag, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.1,
        stepover_sugg,
        fields,
    );
    dv_pill(
        ui,
        p(OperationType::Zigzag, "depth_per_pass", "Depth/pass"),
        &mut cfg.depth_per_pass,
        " mm",
        0.1,
        dpp_sugg,
        fields,
    );
    dv(
        ui,
        p(OperationType::Zigzag, "angle", "Angle"),
        &mut cfg.angle,
        " deg",
        1.0,
        fields,
    );
    region_ordering_row(ui, "zigzag_ord", &mut cfg.region_ordering);
}
