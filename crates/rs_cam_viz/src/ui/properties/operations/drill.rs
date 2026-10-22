use super::super::pills::PillSuggestions;
use rs_cam_core::session::HoleSelection;

use crate::state::toolpath::{AlignmentPinDrillConfig, DrillConfig, DrillCycleType};

use super::super::{depth_caution_row, dv, dv_pill, p};
use super::DepthBeyondStock;
use crate::ui::components::{OneLine, UiExt as _};
use rs_cam_core::compute::catalog::OperationType;

/// What the hole sources are, printed on every drill panel so the operator
/// knows what to import (G-DRILLCENTROID, UX-R03-004).
const HOLE_SOURCES_NOTE: &str =
    "Holes are DXF points and circle/arc centres, and circles in SVG drawings.";

/// The drill legend: what a hole source is, and how many of the items are
/// points, so circle and point markers read clearly.
///
/// Geometry selection P4.3 (lead ruling L-1): the count, Select all, Clear,
/// the layer rule and the refusal moved to the Selection section
/// (`selection_section.rs`), which sends edits only. This legend writes
/// nothing.
///
/// `needs_holes` is true for `Drill`, whose only hole source is the
/// selection, so the legend is always visible; false for the pin drill,
/// whose holes are the stock's alignment pins and for which a drawing with
/// no hole is unremarkable.
fn draw_hole_legend(ui: &mut egui::Ui, holes: Option<&HoleSelection>, needs_holes: bool) {
    let items = holes.map_or(&[][..], |h| h.items.as_slice());
    if items.is_empty() && !needs_holes {
        return;
    }
    let points = items.iter().filter(|i| i.diameter.is_none()).count();
    let legend = if points > 0 {
        format!("{HOLE_SOURCES_NOTE} {points} of these are points.")
    } else {
        HOLE_SOURCES_NOTE.to_owned()
    };
    // P6.7 (IC1): one row; the hover gives the whole text. The hole size
    // rows of the Selection section give each size and its fit.
    ui.add(OneLine::new(legend));
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
        .truncate()
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
    op: OperationType,
    feed_rate: &mut f64,
    retract_z: &mut f64,
    feed_sugg: Option<super::super::pills::PillSuggestion<'_>>,
) {
    dv_pill(
        ui,
        p(op, "feed_rate", "Feed Rate:"),
        feed_rate,
        " mm/min",
        10.0,
        feed_sugg,
    );
    dv(
        ui,
        p(op, "retract_z", "Retract (R):"),
        retract_z,
        " mm",
        0.5,
    );
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
    op: OperationType,
    cycle: DrillCycleType,
    peck_depth: &mut f64,
    dwell_time: Option<&mut f64>,
    retract_amount: Option<&mut f64>,
) {
    if matches!(cycle, DrillCycleType::Peck | DrillCycleType::ChipBreak) {
        dv(
            ui,
            p(op, "peck_depth", "Peck Depth:"),
            peck_depth,
            " mm",
            0.5,
        );
    }
    if let Some(dwell_time) = dwell_time
        && cycle == DrillCycleType::Dwell
    {
        dv(
            ui,
            p(op, "dwell_time", "Dwell Time:"),
            dwell_time,
            " s",
            0.1,
        );
    }
    if let Some(retract_amount) = retract_amount
        && cycle == DrillCycleType::ChipBreak
    {
        dv(
            ui,
            p(op, "retract_amount", "Retract Amt:"),
            retract_amount,
            " mm",
            0.1,
        );
    }
}

pub(in crate::ui::properties) fn draw_drill_params(
    ui: &mut egui::Ui,
    cfg: &mut DrillConfig,
    holes: Option<&HoleSelection>,
    pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    // Spec: drill ops get a feed pill (peck-cycle plunge feed) but no
    // stepover/DOC pills (Z-only kinematics — LUT radial/axial don't
    // apply). DrillConfig has no plunge_rate or spindle_rpm slot of its
    // own, so this is the only LUT-driven field on the panel.
    let feed_sugg = pills.map(PillSuggestions::feed_rate);
    ui.param_grid("drill_p", |ui| {
        draw_drill_cycle_combo(ui, "drill_cycle", &mut cfg.cycle);
        dv(
            ui,
            p(OperationType::Drill, "depth", "Depth:"),
            &mut cfg.depth,
            " mm",
            0.5,
        );
        depth_caution_row(ui, depth_caution);
        draw_drill_feed_rows(
            ui,
            OperationType::Drill,
            &mut cfg.feed_rate,
            &mut cfg.retract_z,
            feed_sugg,
        );
        draw_drill_cycle_rows(
            ui,
            OperationType::Drill,
            cfg.cycle,
            &mut cfg.peck_depth,
            Some(&mut cfg.dwell_time),
            Some(&mut cfg.retract_amount),
        );
        // The hole fit (operator ruling 2026-10-22): a circle within this
        // value of the tool diameter is drilled. Pin Drill has no row.
        dv(
            ui,
            p(OperationType::Drill, "size_tolerance", "Size Tolerance:"),
            &mut cfg.size_tolerance,
            " mm",
            0.01,
        );
    });
    draw_hole_legend(ui, holes, true);
}

pub(in crate::ui::properties) fn draw_alignment_pin_drill_params(
    ui: &mut egui::Ui,
    cfg: &mut AlignmentPinDrillConfig,
    holes: Option<&HoleSelection>,
    pills: Option<&PillSuggestions>,
) {
    let feed_sugg = pills.map(PillSuggestions::feed_rate);
    let extra = holes.map_or(0, |h| h.holes.len());
    // The op drills the live stock pins of its diameter (W6.5); the config
    // holds no copy of them.
    ui.label(format!(
        "The stock pins of {:.2} mm + {extra} picked hole(s)",
        cfg.diameter
    ));
    ui.param_grid("pin_drill_p", |ui| {
        dv(
            ui,
            p(
                OperationType::AlignmentPinDrill,
                "spoilboard_penetration",
                "Spoilboard:",
            ),
            &mut cfg.spoilboard_penetration,
            " mm",
            0.5,
        );
        draw_drill_cycle_combo(ui, "pin_drill_cycle", &mut cfg.cycle);
        draw_drill_feed_rows(
            ui,
            OperationType::AlignmentPinDrill,
            &mut cfg.feed_rate,
            &mut cfg.retract_z,
            feed_sugg,
        );
        draw_drill_cycle_rows(
            ui,
            OperationType::AlignmentPinDrill,
            cfg.cycle,
            &mut cfg.peck_depth,
            None,
            None,
        );
    });
    draw_hole_legend(ui, holes, false);
}
