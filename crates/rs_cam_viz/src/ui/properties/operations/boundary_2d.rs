use super::super::pills::PillSuggestions;

use crate::state::toolpath::{
    AdaptiveConfig, CompensationType, FaceConfig, FaceDirection, InlayConfig, InlayPlugConfig,
    PocketConfig, PocketPattern, ProfileConfig, ProfileSide, RegionOrdering, VCarveConfig,
    ZigzagConfig,
};

use super::super::{
    advanced, depth_caution_row, dv, dv_pill, help_for, p, schema_range, through_cut_row,
};
use super::{DepthBeyondStock, FloorInputs, ThroughCut, draw_tab_diagram};
use crate::ui::components::value_row::label_cell;
use crate::ui::components::{Button, OneLine, UiExt as _, ValueRow};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::floor::{Floor, FloorRefusal};
use rs_cam_core::compute::operation_configs::OpenPaths;

/// The label of the reference combo of the floor row.
const FLOOR_FROM_LABEL: &str = "Floor from:";

/// The hover of the reference combo (ruling L-10).
const FLOOR_FROM_HOVER: &str = "The reference of the floor. A change of reference keeps the \
    floor Z: the value is expressed again from the new reference.";

/// The short reason a reference is not offered, for the hover of its
/// disabled entry.
fn floor_reason(refusal: FloorRefusal) -> &'static str {
    match refusal {
        FloorRefusal::NoModelMesh => "The model has no mesh, so it has no top.",
        FloorRefusal::NoFace => "No up face is picked in the selection.",
        FloorRefusal::OutOfDomain { .. } | FloorRefusal::AboveTop { .. } => {
            "The floor would be at or above the Top Z."
        }
        FloorRefusal::NotFinite => "The floor is not a finite number.",
    }
}

/// The floor row of Pocket, Profile, Zigzag, Adaptive 2D and Bore (geometry
/// selection P3.7, design 8.1; P6.6). Two grid rows:
///
/// 1. "Floor from:" and the reference combo (`Floor::EACH_REFERENCE`). A
///    change of reference re-expresses the value, so the resolved floor Z
///    stays (ruling L-10, `FloorReferences::re_express`). An entry that
///    has no Z here, or whose value the domain refuses, is disabled with its
///    reason. With no resolved heights the combo is disabled: nothing keeps
///    the Z.
/// 2. The value ("Depth:" or "Z:"). The range is `Floor::domain`, the one
///    rule of the door, so a refused value never reaches the config. The
///    hover is `Floor::reference_help`: the schema text of the `floor`
///    parameter (a driver-set field has no registry `help`).
/// 3. The resolved floor Z; under "each picked face", the floor of each face.
pub(super) fn floor_row(
    ui: &mut egui::Ui,
    op: OperationType,
    floor: &mut Floor,
    inputs: Option<&FloorInputs>,
) {
    let refs = inputs.map(|inputs| inputs.refs);
    label_cell(ui, FLOOR_FROM_LABEL).on_hover_text(FLOOR_FROM_HOVER);
    let current = *floor;
    ui.add_enabled_ui(refs.is_some(), |ui| {
        egui::ComboBox::from_id_salt(("floor_from", op.name()))
            .truncate()
            .selected_text(current.reference_label())
            .show_ui(ui, |ui| {
                for template in Floor::EACH_REFERENCE {
                    let selected = current.same_reference(&template);
                    let next = match refs {
                        Some(refs) if !selected => Some(refs.re_express(current, template)),
                        Some(_) | None => None,
                    };
                    let enabled = selected || matches!(next, Some(Ok(_)));
                    let response = ui
                        .add_enabled(
                            enabled,
                            egui::Button::selectable(selected, template.reference_label()),
                        )
                        .on_hover_text(template.reference_help())
                        .on_disabled_hover_text(match next {
                            Some(Err(refusal)) => floor_reason(refusal),
                            Some(Ok(_)) | None => "",
                        });
                    if response.clicked()
                        && let Some(Ok(re_expressed)) = next
                    {
                        *floor = re_expressed;
                    }
                }
            });
    })
    .response
    .on_disabled_hover_text("The heights of the toolpath do not resolve, so no floor Z is kept.");
    ui.end_row();

    let (label, mut value) = match *floor {
        Floor::Z { z } => ("Z:", z),
        other => ("Depth:", other.value()),
    };
    let out = ValueRow::new(
        label,
        &mut value,
        " mm",
        0.1,
        floor.domain().inclusive_bounds(),
    )
    .tooltip(Some(floor.reference_help()))
    .show(ui);
    if out.edited && floor.domain().accepts(value) {
        *floor = floor.with_value(value);
    }

    // The resolved floor Z, on its own row so the value row fits the
    // 240-point rail. Under "each picked face": the floor of each face.
    let face_floors = match (*floor, inputs) {
        (Floor::EachFace { .. }, Some(inputs)) => inputs.face_floors.as_slice(),
        _ => &[],
    };
    let text = if face_floors.is_empty() {
        match refs.map(|refs| refs.floor_z(floor)) {
            Some(Ok(z)) => format!("= Z {z:.2}"),
            Some(Err(refusal)) => format!("= no Z: {}", floor_reason(refusal)),
            None => "= Z \u{2014}".to_owned(),
        }
    } else {
        let count = face_floors.len();
        let runs: Vec<String> = face_floors
            .iter()
            .enumerate()
            .map(|(i, run)| format!("F{} Z {:.2}", i + 1, run.z))
            .collect();
        format!(
            "{} \u{00B7} {count} floor{}",
            runs.join(" \u{00B7} "),
            if count == 1 { "" } else { "s" }
        )
    };
    ui.label("");
    ui.add(OneLine::new(text).detail(
        "The resolved floor Z in the setup frame. Under Each picked face: the floor of \
         each cut face, its plane Z less the depth; a face at the top is not cut.",
    ));
    ui.end_row();
}

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
    floor: Option<&FloorInputs>,
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
        floor_row(ui, OperationType::Pocket, &mut cfg.floor, floor);
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
        open_paths_rows(ui, &mut cfg.open_paths);
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

/// The text of the Flip button of the Pocket open paths side (P6.7). The
/// sketch shows U+21C4; the inspector fonts do not hold that glyph, so the
/// button uses U+2194.
const FLIP_LABEL: &str = "\u{2194} Flip";

/// The hover of [`FLIP_LABEL`].
const FLIP_HOVER: &str = "Cut the area on the other side of each open path.";

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
fn open_paths_rows(ui: &mut egui::Ui, open_paths: &mut OpenPaths) {
    let help = help_for(OperationType::Pocket, "open_paths");
    let label = ui.label("Open paths:");
    if let Some(help) = help {
        label.on_hover_text(help);
    }
    let extend = *open_paths != OpenPaths::Skip;
    egui::ComboBox::from_id_salt("pocket_open_paths")
        .truncate()
        .selected_text(if extend { "Extend to stock" } else { "Skip" })
        .show_ui(ui, |ui| {
            if ui.selectable_label(!extend, "Skip").clicked() {
                *open_paths = OpenPaths::Skip;
            }
            // O-2: the area on the left of the drawn direction first; Flip
            // gives the right.
            if ui.selectable_label(extend, "Extend to stock").clicked() && !extend {
                *open_paths = OpenPaths::ExtendLeft;
            }
        })
        .response
        .on_hover_text(open_paths.label());
    ui.end_row();
    if *open_paths == OpenPaths::Skip {
        return;
    }
    ui.label("Side:");
    egui::ComboBox::from_id_salt("pocket_open_side")
        .truncate()
        .selected_text(open_side_label(*open_paths))
        .show_ui(ui, |ui| {
            for side in [OpenPaths::ExtendLeft, OpenPaths::ExtendRight] {
                ui.selectable_value(open_paths, side, open_side_label(side));
            }
        });
    ui.end_row();
    // The Flip button has its own row: beside the combo it does not fit
    // the 240-point rail.
    ui.label("");
    if ui
        .add(Button::quiet(FLIP_LABEL))
        .on_hover_text(FLIP_HOVER)
        .clicked()
    {
        *open_paths = flipped(*open_paths);
    }
    ui.end_row();
}

/// The Side label of an Extend choice (P6.7).
fn open_side_label(open_paths: OpenPaths) -> &'static str {
    match open_paths {
        OpenPaths::Skip => "Skip",
        OpenPaths::ExtendLeft => "Left of the drawn direction",
        OpenPaths::ExtendRight => "Right of the drawn direction",
    }
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
    ui.label("Side:");
    let on_refusal = side_on_refusal(cfg);
    let before = cfg.side;
    egui::ComboBox::from_id_salt("prof_side")
        .truncate()
        .selected_text(profile_side_label(cfg.side))
        .show_ui(ui, |ui| {
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
        });
    if cfg.side == ProfileSide::On
        && before != ProfileSide::On
        && cfg.compensation == CompensationType::InControl
    {
        cfg.compensation = CompensationType::InComputer;
    }
    ui.end_row();
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
    depth_caution: Option<&DepthBeyondStock>,
    through_cut: Option<&ThroughCut>,
    floor: Option<&FloorInputs>,
    tab_editor: (
        crate::state::toolpath::ToolpathId,
        &mut crate::state::pick::SelectionUi,
    ),
) {
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    ui.param_grid("profile_p", |ui| {
        profile_side_row(ui, cfg);
        floor_row(ui, OperationType::Profile, &mut cfg.floor, floor);
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
        ui.horizontal(|ui| {
            ui.checkbox(&mut cfg.climb, "");
            // P6.7 (design 2.2): Climb does not act on an open path.
            if cfg.side == ProfileSide::On {
                ui.add(OneLine::new(CLIMB_ON_HINT).detail(CLIMB_ON_DETAIL));
            }
        });
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
    draw_profile_tabs(ui, cfg, through_cut, tab_editor);
    advanced(ui, "profile_adv", |ui| {
        ui.label("Compensation:");
        // P6.7: under side On, In Control is greyed. Core refuses the pair
        // at every door (`domain::side_on_in_control`), so the combo reads
        // that predicate and does not state a second rule.
        let in_control_refused = in_control_refused(cfg);
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
                ui.add_enabled_ui(!in_control_refused, |ui| {
                    ui.selectable_value(
                        &mut cfg.compensation,
                        CompensationType::InControl,
                        "In Control",
                    )
                    .on_disabled_hover_text(IN_CONTROL_UNDER_ON);
                });
            });
        ui.end_row();
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
                // P5: hand tabs come from `tab_points`. P4.6: the viewport
                // tab editor writes them (`Edit in viewport`); MCP and the
                // project file write them too.
                if cfg.tab_count > 0 || !cfg.tab_points.is_empty() {
                    ui.label("Hand tabs:");
                    ui.horizontal(|ui| {
                        ui.label(cfg.tab_points.len().to_string());
                        if !cfg.tab_points.is_empty() && ui.small_button("Clear").clicked() {
                            cfg.tab_points.clear();
                        }
                        let on = selection_ui.tab_edit(toolpath_id).is_some();
                        let button = if on {
                            Button::primary("Edit in viewport")
                        } else {
                            Button::new("Edit in viewport")
                        };
                        if ui.add(button).on_hover_text(TAB_EDIT_HOVER).clicked() {
                            selection_ui.set_tab_edit(toolpath_id, !on);
                        }
                    });
                    ui.end_row();
                }
                if cfg.tab_count > 0 || !cfg.tab_points.is_empty() {
                    dv(
                        ui,
                        p(OperationType::Profile, "tab_width", "Bridge width:"),
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
    floor: Option<&FloorInputs>,
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
        floor_row(ui, OperationType::Adaptive, &mut cfg.floor, floor);
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
    floor: Option<&FloorInputs>,
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
        floor_row(ui, OperationType::Zigzag, &mut cfg.floor, floor);
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
