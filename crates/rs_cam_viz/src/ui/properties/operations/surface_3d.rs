use super::super::pills::PillSuggestions;
use rs_cam_core::finish::pencil::PencilDetector;
use rs_cam_core::ops::adaptive_shared::{
    radial_woc_fraction_from_leading_arc, target_engagement_fraction,
};

use crate::state::toolpath::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClaimsReference, ClearingStrategy, DropCutterConfig,
    PencilConfig, RegionOrdering, ScallopConfig, ScallopDirection, SteepShallowConfig, StockSource,
    UnifiedFinishConfig, WaterlineConfig,
};

use super::super::{dv, dv_pill, help_for, p};
use crate::ui::components::{Button, UiExt as _, ValueRow};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::execute::adaptive3d_step_ladder_refusal;

/// Fallback tool radius (1/8" endmill) when the active tool's radius is
/// unavailable or non-physical — keeps the load↔stepover bridge finite.
const FALLBACK_TOOL_RADIUS: f64 = 3.175;

pub(in crate::ui::properties) fn draw_dropcutter_params(
    ui: &mut egui::Ui,
    cfg: &mut DropCutterConfig,
    pills: Option<&PillSuggestions>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    ui.param_grid("dc_p", |ui| {
        dv_pill(
            ui,
            p(OperationType::DropCutter, "stepover", "Stepover:"),
            &mut cfg.stepover,
            " mm",
            0.1,
            0.05..=50.0,
            stepover_sugg,
        );
        dv(
            ui,
            p(OperationType::DropCutter, "min_z", "Min Z:"),
            &mut cfg.min_z,
            " mm",
            0.5,
            -500.0..=0.0,
        );
        dv(
            ui,
            p(OperationType::DropCutter, "slope_from", "Slope From:"),
            &mut cfg.slope_from,
            " deg",
            1.0,
            0.0..=90.0,
        );
        dv(
            ui,
            p(OperationType::DropCutter, "slope_to", "Slope To:"),
            &mut cfg.slope_to,
            " deg",
            1.0,
            0.0..=90.0,
        );
    });
}

pub(in crate::ui::properties) fn draw_adaptive3d_params(
    ui: &mut egui::Ui,
    cfg: &mut Adaptive3dConfig,
    tool_radius: f64,
    pills: Option<&PillSuggestions>,
) {
    // Spec: pill stepover + depth_per_pass.
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    // The ContourSpiral strategy holds engagement flat by construction, so
    // its primary knob is the friendly "Optimal load" slider (it derives
    // the stepover). The raw stepover pill is retired for that strategy and
    // shown derived instead.
    let spiral = matches!(cfg.clearing_strategy, ClearingStrategy::ContourSpiral);
    ui.param_grid("a3d_p", |ui| {
        if spiral {
            draw_spiral_load_control(ui, cfg, tool_radius);
        } else {
            dv_pill(
                ui,
                p(OperationType::Adaptive3d, "stepover", "Stepover:"),
                &mut cfg.stepover,
                " mm",
                0.1,
                0.05..=50.0,
                stepover_sugg,
            );
        }
        dv_pill(
            ui,
            p(OperationType::Adaptive3d, "depth_per_pass", "Depth/Pass:"),
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
            0.1..=50.0,
            dpp_sugg,
        );
        draw_coarse_steps(ui, cfg);
        dv(
            ui,
            p(
                OperationType::Adaptive3d,
                "stock_to_leave_axial",
                "Stock to Leave:",
            ),
            &mut cfg.stock_to_leave_axial,
            " mm",
            0.05,
            0.0..=10.0,
        );
        dv(
            ui,
            p(OperationType::Adaptive3d, "tolerance", "Tolerance:"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            0.01..=1.0,
        );
        dv(
            ui,
            p(
                OperationType::Adaptive3d,
                "min_cutting_radius",
                "Min Cut Radius:",
            ),
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
                ui.selectable_value(&mut cfg.entry_style, Adaptive3dEntryStyle::Plunge, "Plunge");
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
                    p(OperationType::Adaptive3d, "ramp_angle_deg", "Ramp Angle:"),
                    &mut cfg.ramp_angle_deg,
                    " deg",
                    0.5,
                    0.5..=45.0,
                );
            }
            Adaptive3dEntryStyle::Helix => {
                dv(
                    ui,
                    p(
                        OperationType::Adaptive3d,
                        "helix_radius_factor",
                        "Helix Radius:",
                    ),
                    &mut cfg.helix_radius_factor,
                    " × D",
                    0.05,
                    0.05..=0.5,
                );
                dv(
                    ui,
                    p(OperationType::Adaptive3d, "helix_pitch", "Helix Pitch:"),
                    &mut cfg.helix_pitch,
                    " mm",
                    0.1,
                    0.1..=10.0,
                );
            }
        }
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
                ui.selectable_value(&mut cfg.region_ordering, RegionOrdering::ByArea, "By Area");
            });
        ui.end_row();
        ui.label("Strategy:");
        egui::ComboBox::from_id_salt("a3d_strat")
            .selected_text(match cfg.clearing_strategy {
                ClearingStrategy::ContourParallel => "Contour Parallel",
                ClearingStrategy::Adaptive => "Adaptive",
                ClearingStrategy::AgentSearch => "Agent Search",
                ClearingStrategy::ContourSpiral => "Contour Spiral",
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
                ui.selectable_value(
                    &mut cfg.clearing_strategy,
                    ClearingStrategy::ContourSpiral,
                    "Contour Spiral",
                )
                .on_hover_text(
                    "Constructive inside-out spiral per slice: one continuous \
                         stay-down pass per region with engagement bounded by the \
                         stepover. Experimental (Stage 1, algorithm review 2026-06-12).",
                );
            });
        ui.end_row();
        ui.label("Z Blend:");
        ui.checkbox(&mut cfg.z_blend, "");
        ui.end_row();
    });
}

/// The rows of the step ladder (`coarse_steps`) in the 3D Rough panel.
///
/// Step-ladder Phase 4. The rows edit the clone of the operation; the
/// panel write-back sends it through `Command::ReplaceToolpathConfig`, as
/// for every other row. The rows are:
///
/// - "Coarse Steps:" with the Add and Remove buttons;
/// - one "Coarse Step N:" value row for each step, coarsest first;
/// - one line that shows the whole ladder, Depth/Pass last.
///
/// A bad ladder is not clamped here. The line turns to the caution colour
/// and its hover gives the adapter's own refusal
/// (`adaptive3d_step_ladder_refusal`). The same text disables Generate
/// through the validation arm on the registry row.
fn draw_coarse_steps(ui: &mut egui::Ui, cfg: &mut Adaptive3dConfig) {
    let help = help_for(OperationType::Adaptive3d, "coarse_steps");
    let label = ui.label("Coarse Steps:");
    if let Some(help) = help {
        let _ = label.on_hover_text(help);
    }
    ui.horizontal(|ui| {
        if ui
            .add(Button::quiet("+ Add"))
            .on_hover_text("Add a step between the last coarse step and Depth/Pass.")
            .clicked()
        {
            let next = next_coarse_step(cfg);
            cfg.coarse_steps.push(next);
        }
        if ui
            .add(Button::quiet("Remove").enabled(!cfg.coarse_steps.is_empty()))
            .on_hover_text("Remove the last coarse step.")
            .clicked()
        {
            let _ = cfg.coarse_steps.pop();
        }
    });
    ui.end_row();

    for (i, step) in cfg.coarse_steps.iter_mut().enumerate() {
        let label = format!("Coarse Step {}:", i + 1);
        let _ = ValueRow::new(&label, step, " mm", 0.1, 0.1..=100.0)
            .tooltip(help)
            .show(ui);
    }

    let refusal = adaptive3d_step_ladder_refusal(cfg);
    let color = if refusal.is_some() {
        crate::ui::tokens::CAUTION
    } else {
        crate::ui::tokens::TEXT_MUTED
    };
    ui.label("");
    let line = ui.label(egui::RichText::new(ladder_line(cfg)).small().color(color));
    if let Some(refusal) = refusal {
        let _ = line.on_hover_text(refusal);
    } else if let Some(help) = help {
        let _ = line.on_hover_text(help);
    }
    ui.end_row();
}

/// The one line that shows the ladder: the coarse steps, then Depth/Pass,
/// for example "10 → 5 mm". With no coarse step it says "5 mm, one step".
pub(in crate::ui::properties) fn ladder_line(cfg: &Adaptive3dConfig) -> String {
    if cfg.coarse_steps.is_empty() {
        return format!("{} mm, one step", mm_text(cfg.depth_per_pass));
    }
    let steps: Vec<String> = cfg
        .coarse_steps
        .iter()
        .chain(std::iter::once(&cfg.depth_per_pass))
        .map(|&v| mm_text(v))
        .collect();
    format!("{} mm", steps.join(" \u{2192} "))
}

/// A step in mm with no trailing zeros: 10, 5, 2.5, 0.25.
fn mm_text(v: f64) -> String {
    let text = format!("{v:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// The step that "+ Add" writes. With no coarse step it is two times
/// Depth/Pass (5 gives 10). Else it is halfway between the last coarse
/// step and Depth/Pass, to 0.1 mm (10 above 1 gives 5.5). Both values are
/// larger than Depth/Pass, so the new ladder is good when the old one was.
pub(in crate::ui::properties) fn next_coarse_step(cfg: &Adaptive3dConfig) -> f64 {
    let base = cfg.depth_per_pass;
    match cfg.coarse_steps.last() {
        None => 2.0 * base,
        Some(&last) => (((last + base) / 2.0) * 10.0).round() / 10.0,
    }
}

/// Clamp the active tool's radius to a finite, positive value for the
/// load↔stepover bridge.
fn sane_tool_radius(tool_radius: f64) -> f64 {
    if tool_radius.is_finite() && tool_radius > 0.0 {
        tool_radius
    } else {
        FALLBACK_TOOL_RADIUS
    }
}

/// "Optimal load" row for the ContourSpiral strategy: the slider derives
/// `cfg.stepover` from a leading-arc engagement fraction via the
/// [`target_engagement_fraction`] bridge, so the operator dials the load
/// the spiral holds rather than a raw stepover. (The trochoidal relief cap
/// `trochoid_cap_mult` stays at its tuned engine default — it's a
/// corner-relief mechanism that rarely fires on open roughing, so it's not
/// surfaced as a knob.)
fn draw_spiral_load_control(ui: &mut egui::Ui, cfg: &mut Adaptive3dConfig, tool_radius: f64) {
    let r = sane_tool_radius(tool_radius);

    // Derived live from the raw stepover so existing projects (and the
    // feeds suggestion that wrote `stepover`) round-trip through the knob.
    let mut load_pct = (target_engagement_fraction(cfg.stepover, r) * 100.0).clamp(5.0, 45.0);
    ui.label("Optimal load:").on_hover_text(
        "Cutter engagement the spiral holds on every steady wrap \
         (leading-arc fraction of the tool). Sets the stepover for you — \
         more load = wider step = fewer passes but a heavier cut.",
    );
    if ui
        .add(egui::Slider::new(&mut load_pct, 5.0..=45.0).suffix("%"))
        .changed()
    {
        cfg.stepover = 2.0 * r * radial_woc_fraction_from_leading_arc(load_pct / 100.0);
    }
    ui.end_row();
    ui.label("");
    ui.label(
        egui::RichText::new(format!("→ stepover ≈ {:.2} mm", cfg.stepover))
            .small()
            .color(crate::ui::tokens::TEXT_MUTED),
    );
    ui.end_row();
}

pub(in crate::ui::properties) fn draw_waterline_params(
    ui: &mut egui::Ui,
    cfg: &mut WaterlineConfig,
    _pills: Option<&PillSuggestions>,
) {
    // Waterline: z_step is the axial pass spacing, but Step 3 LUT mapping
    // (axial_depth_mm) is calibrated for clearing DOC, not contour Z-step.
    // Leave Z Step alone; feed/plunge live on the Feeds tab (W3.2).
    ui.param_grid("wl_p", |ui| {
        dv(
            ui,
            p(OperationType::Waterline, "z_step", "Z Step:"),
            &mut cfg.z_step,
            " mm",
            0.1,
            0.05..=20.0,
        );
        dv(
            ui,
            p(OperationType::Waterline, "sampling", "Sampling:"),
            &mut cfg.sampling,
            " mm",
            0.1,
            0.1..=5.0,
        );
        ui.label("Continuous:");
        ui.checkbox(&mut cfg.continuous, "");
        ui.end_row();
        // Z range now comes from the Heights tab (top_z / bottom_z)
    });
}

pub(in crate::ui::properties) fn draw_pencil_params(
    ui: &mut egui::Ui,
    cfg: &mut PencilConfig,
    tools: &[(crate::state::job::ToolId, String, f64)],
    _pills: Option<&PillSuggestions>,
    // READ ONLY. W2 (G-STARTFROM): the Geometry tab's one "Start from" row
    // writes `stock_source` for every operation. Pencil reads it to decide
    // whether its analytic reference is still a choice.
    stock_source: StockSource,
) {
    // Pencil's offset_stepover is a parallel-pass spacing, not the same
    // shape as a clearing radial WOC; leave it alone. Feed/plunge live on
    // the Feeds tab (W3.2).
    // FIN-09: the config field is `PencilDetector`, so the panel compares
    // variants. The old string compares accepted four aliases the generator
    // never honoured.
    let curvature = cfg.detector == PencilDetector::Curvature;
    let rest_depth = cfg.detector == PencilDetector::RestDepth;
    ui.param_grid("pen_p", |ui| {
        // Valley-detection front-end. Rest depth = dual-tool rest field (the
        // aligned detector — the reference tool decides where pencil runs);
        // Dihedral = mesh-crease detection (clean CAD-style corners);
        // Curvature = curvature crest lines (dialled by Valley Saliency).
        ui.label("Detector:");
        let selected = if rest_depth {
            "Rest depth (recommended)"
        } else if curvature {
            "Curvature (crest)"
        } else {
            "Dihedral (crease)"
        };
        egui::ComboBox::from_id_salt("pen_detector")
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(rest_depth, "Rest depth (recommended)")
                    .clicked()
                {
                    cfg.detector = PencilDetector::RestDepth;
                }
                if ui
                    .selectable_label(!curvature && !rest_depth, "Dihedral (crease)")
                    .clicked()
                {
                    cfg.detector = PencilDetector::Dihedral;
                }
                if ui
                    .selectable_label(curvature, "Curvature (crest)")
                    .clicked()
                {
                    cfg.detector = PencilDetector::Curvature;
                }
            });
        ui.end_row();
        if rest_depth {
            // Rest-depth dial. Rest Cell is the XY grid resolution of the
            // rest field. FIN-04 deleted the retired "Route Width x"
            // widget with the field it wrote.
            dv(
                ui,
                p(OperationType::Pencil, "rest_cell_mm", "Rest Cell:"),
                &mut cfg.rest_cell_mm,
                " mm",
                0.05,
                0.1..=2.0,
            );
            ui.end_row();
        } else if curvature {
            // Curvature-detector dials. Valley Saliency is the significance
            // knob: min concave curvature |κ₂| (1/mm) a valley must reach —
            // low traces every seam, high keeps only deep sharp valleys.
            dv(
                ui,
                p(OperationType::Pencil, "valley_saliency", "Valley Saliency:"),
                &mut cfg.valley_saliency,
                " 1/mm",
                0.01,
                0.0..=2.0,
            );
            // UI-10: an integer field. `ValueRow` edits an `f64`, and a
            // temporary f64 would change what a partial edit commits.
            ui.label("Curv. Smoothing:");
            let mut s = cfg.curvature_smoothing as i32;
            if ui.add(egui::DragValue::new(&mut s).range(0..=20)).changed() {
                cfg.curvature_smoothing = s.max(0) as usize;
            }
            ui.end_row();
        } else {
            dv(
                ui,
                p(
                    OperationType::Pencil,
                    "bitangency_angle",
                    "Bitangency Angle:",
                ),
                &mut cfg.bitangency_angle,
                " deg",
                1.0,
                90.0..=180.0,
            );
        }
        dv(
            ui,
            p(OperationType::Pencil, "min_cut_length", "Min Cut Length:"),
            &mut cfg.min_cut_length,
            " mm",
            0.5,
            0.5..=50.0,
        );
        dv(
            ui,
            p(OperationType::Pencil, "hookup_distance", "Hookup Distance:"),
            &mut cfg.hookup_distance,
            " mm",
            0.5,
            0.5..=50.0,
        );
        // UI-10: an integer field. `ValueRow` edits an `f64`, and a
        // temporary f64 would change what a partial edit commits.
        ui.label("Offset Passes:");
        let mut n = cfg.num_offset_passes as i32;
        if ui.add(egui::DragValue::new(&mut n).range(0..=10)).changed() {
            cfg.num_offset_passes = n.max(0) as usize;
        }
        ui.end_row();
        dv(
            ui,
            p(OperationType::Pencil, "offset_stepover", "Offset Stepover:"),
            &mut cfg.offset_stepover,
            " mm",
            0.1,
            0.05..=10.0,
        );
        dv(
            ui,
            p(OperationType::Pencil, "sampling", "Sampling:"),
            &mut cfg.sampling,
            " mm",
            0.1,
            0.1..=5.0,
        );
        // Reference-tool rest gate: keep only valleys the pencil tool (the
        // op's own tool) reaches more than this deeper than the bigger
        // reference finish tool. 0 = off (trace every detected valley); raise
        // to skip shallow/already-reachable seams.
        dv(
            ui,
            p(
                OperationType::Pencil,
                "min_valley_depth",
                "Min Valley Depth:",
            ),
            &mut cfg.min_valley_depth,
            " mm",
            0.05,
            0.0..=5.0,
        );
        // The analytic reference: what "rest" is measured against when the
        // generator does NOT read a machined stock. Only `rest_depth_arm`
        // reads `initial_stock`; `dihedral_arm` and `curvature_arm` always
        // call `resolve_reference_cutter`, so their picker stays live under
        // either "Start from" option. Hiding it on those two would hide a
        // control that decides where pencil runs.
        let reference_is_live =
            cfg.detector != PencilDetector::RestDepth || stock_source == StockSource::Fresh;
        if reference_is_live {
            // "Nominal Ø" = a synthetic ball at Reference Tool Ø; or pick a
            // real library tool whose TRUE geometry (flat/vbit/tapered)
            // defines the rest — a flat leaves a different rest shape than a
            // ball of the same diameter.
            ui.label("Reference:");
            let ref_label = cfg
                .reference_tool_id
                .and_then(|rid| tools.iter().find(|(id, _, _)| *id == rid))
                .map(|(_, s, _)| s.as_str())
                .unwrap_or("Nominal Ø");
            egui::ComboBox::from_id_salt("pen_reference_tool")
                .selected_text(ref_label)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(cfg.reference_tool_id.is_none(), "Nominal Ø")
                        .clicked()
                    {
                        cfg.reference_tool_id = None;
                    }
                    for (id, name, _) in tools {
                        let selected = cfg.reference_tool_id == Some(*id);
                        if ui.selectable_label(selected, name.as_str()).clicked() {
                            cfg.reference_tool_id = Some(*id);
                        }
                    }
                });
            ui.end_row();
            // The nominal diameter only applies when no real tool is chosen.
            if cfg.reference_tool_id.is_none() {
                dv(
                    ui,
                    p(
                        OperationType::Pencil,
                        "reference_tool_diameter",
                        "Reference Tool Ø:",
                    ),
                    &mut cfg.reference_tool_diameter,
                    " mm",
                    0.5,
                    0.5..=25.0,
                );
            }
        }
        dv(
            ui,
            p(OperationType::Pencil, "stock_to_leave", "Stock to Leave:"),
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
    _pills: Option<&PillSuggestions>,
) {
    // Scallop's stepover is computed from scallop_height + tool radius, not
    // an editable field, so no stepover pill. Feed/plunge live on the Feeds
    // tab (W3.2).
    ui.param_grid("sc_p", |ui| {
        dv(
            ui,
            p(OperationType::Scallop, "scallop_height", "Scallop Height:"),
            &mut cfg.scallop_height,
            " mm",
            0.01,
            0.01..=2.0,
        );
        dv(
            ui,
            p(OperationType::Scallop, "tolerance", "Tolerance:"),
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
        ui.label("Iso-Field Rings:").on_hover_text(
            "Rings from the iso-scallop field instead of the offset \
                 cascade: spacing varies point-by-point along each ring \
                 (organic, terrain-following), uses the spec-correct cosine \
                 slope law, and always runs to completion. Measured faster \
                 than the band mix at better coverage on terrain work.",
        );
        ui.checkbox(&mut cfg.iso_field, "");
        ui.end_row();
        dv(
            ui,
            p(OperationType::Scallop, "slope_from", "Slope From:"),
            &mut cfg.slope_from,
            " deg",
            1.0,
            0.0..=90.0,
        );
        dv(
            ui,
            p(OperationType::Scallop, "slope_to", "Slope To:"),
            &mut cfg.slope_to,
            " deg",
            1.0,
            0.0..=90.0,
        );
        dv(
            ui,
            p(OperationType::Scallop, "stock_to_leave", "Stock to Leave:"),
            &mut cfg.stock_to_leave,
            " mm",
            0.05,
            0.0..=10.0,
        );
    });
}

/// P2.c orchestrator params (`planning/unified_finish_planner_design.md`).
/// Mirrors `draw_scallop_params`' shape closely: no stepover pill (raster
/// stepover here is a plain editable field, not chipload-derived), no
/// feed/plunge/spindle widgets (those live on the Feeds tab, same as every
/// other 3D finish op's params panel — see the comment on
/// `draw_scallop_params`). No stepover-pattern diagram either (same
/// silent-gap acceptance as `StepoverPattern::from_operation`'s `_ => None`
/// fallback covers Scallop).
/// A/M6: the "Rest Claims" block of the Unified Finish panel — the claims
/// pipeline's four dials, plus a readout of which reference the last
/// generation actually RESOLVED to and why.
///
/// The readout exists because `claims_reference: Auto` resolves against
/// something no config field records: whether a simulated prior stock was in
/// scope. Before A/M6 the dials were not in the panel at all and the
/// resolution appeared only in a `tracing::warn!` that no GUI run
/// subscribes to.
fn draw_unified_finish_claims(
    ui: &mut egui::Ui,
    cfg: &mut UnifiedFinishConfig,
    resolved: Option<rs_cam_core::compute::toolpath_stats::ClaimsReferenceFinding>,
    stock_source: StockSource,
) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Rest Claims").strong());
    ui.param_grid("uf_claims", |ui| {
        ui.label("Pencil Claims:");
        ui.checkbox(&mut cfg.pencil_claims, "").on_hover_text(
            "Run the crease/rest detector inside this operation and cut its \
                     claimed valleys as an extra pass. Off by default: on a \
                     single-tool op the analytic detector marks exactly what this \
                     cutter cannot reach. Every dial below is INERT while this is off.",
        );
        ui.end_row();

        ui.label("Rest Reference:");
        egui::ComboBox::from_id_salt("uf_claims_ref")
            .selected_text(match cfg.claims_reference {
                ClaimsReference::Auto => "Auto (derive)",
                ClaimsReference::SelfProbe => "Analytic self-probe",
                ClaimsReference::MachinedStock => "Machined prior stock",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut cfg.claims_reference,
                    ClaimsReference::Auto,
                    "Auto (derive)",
                )
                .on_hover_text(
                    "Use the machined prior stock when one is in scope, and the \
                         analytic self-probe when none is. Pin the self-probe instead \
                         if the previous pass was a ROUGHING pass — a stock-referenced \
                         detector reads roughing terraces as phantom creases.",
                );
                ui.selectable_value(
                    &mut cfg.claims_reference,
                    ClaimsReference::SelfProbe,
                    "Analytic self-probe",
                )
                .on_hover_text(
                    "Derive rest from the design surface: where can this cutter \
                         not reach the model. Right for a FIRST finish pass and for a \
                         rough-referenced chain; wrong for a same-tool rest pass, \
                         where it names precisely what the pass cannot fix (measured \
                         −88.7% cutting once corrected).",
                );
                ui.selectable_value(
                    &mut cfg.claims_reference,
                    ClaimsReference::MachinedStock,
                    "Machined prior stock",
                )
                .on_hover_text(
                    "Measure rest against the material the previous pass actually \
                         left. Needs this operation's stock source set to remaining \
                         stock, and an upstream pass that has been generated AND \
                         simulated.",
                );
            });
        ui.end_row();

        ui.label("Territory Clip:");
        ui.checkbox(&mut cfg.territory_clip, "").on_hover_text(
            "Confine generation to rest ISLANDS instead of the full surface. \
                     Runs only under the machined-stock reference — under a self-probe \
                     reference the 'rest islands' are geometric, not material, so the \
                     clip is skipped and this becomes an all-over pass.",
        );
        ui.end_row();

        dv(
            ui,
            p(
                OperationType::UnifiedFinish,
                "min_rest_depth_mm",
                "Min Rest Depth:",
            ),
            &mut cfg.min_rest_depth_mm,
            " mm",
            0.005,
            0.0..=1.0,
        );
    });

    // The resolved reference: what the LAST generation used, not what the
    // dial says. `None` before the operation has generated, or when claims
    // are off and nothing was resolved.
    match resolved {
        Some(f) => {
            let r = f.resolution;
            let loud = r.needs_attention() || f.territory_clip_skipped();
            let colour = if loud {
                crate::ui::tokens::CAUTION
            } else {
                crate::ui::tokens::OK
            };
            let used = match r.reference() {
                rs_cam_core::finish::unified_finish::CreaseReference::MachinedStock => {
                    "machined prior stock"
                }
                rs_cam_core::finish::unified_finish::CreaseReference::SelfProbe => {
                    "analytic self-probe"
                }
            };
            let verb = if r.is_derived() { "derived" } else { "pinned" };
            ui.label(
                egui::RichText::new(format!(
                    "{} Last generated against the {used} ({verb}).",
                    if loud { "\u{26A0}" } else { "\u{2713}" }
                ))
                .small()
                .color(colour),
            )
            .on_hover_text(r.why());
            if f.territory_clip_skipped() {
                ui.label(
                    egui::RichText::new(
                        "\u{26A0} Territory Clip was requested and SKIPPED — this pass \
                         covered its full territory, not rest islands.",
                    )
                    .small()
                    .color(crate::ui::tokens::CAUTION),
                );
            }
        }
        None if cfg.pencil_claims => {
            // UNKNOWN, not a grey. Nothing has been measured yet, and §2.6
            // separates "not run" from "passed" so the two cannot be read
            // alike at a glance.
            ui.label(
                egui::RichText::new(
                    "Not generated yet — the resolved rest reference appears here after \
                     this operation runs.",
                )
                .small()
                .color(crate::ui::tokens::UNKNOWN),
            );
        }
        None => {}
    }

    // Auto's input is the stock source, and that lives in a different
    // section of the same panel — say so where the choice is made rather
    // than leaving the operator to discover it from a generated finding.
    if cfg.pencil_claims
        && cfg.claims_reference != ClaimsReference::SelfProbe
        && stock_source == StockSource::Fresh
    {
        ui.label(
            egui::RichText::new(
                "\u{26A0} This operation cuts FRESH stock, so no machined prior is in \
                 scope and the machined-stock reference cannot be used. Set Stock \
                 Source to remaining stock, then generate and simulate the upstream \
                 operation.",
            )
            .small()
            .color(crate::ui::tokens::CAUTION),
        );
    }
}

pub(in crate::ui::properties) fn draw_unified_finish_params(
    ui: &mut egui::Ui,
    cfg: &mut UnifiedFinishConfig,
    _pills: Option<&PillSuggestions>,
    resolved_claims_reference: Option<rs_cam_core::compute::toolpath_stats::ClaimsReferenceFinding>,
    stock_source: StockSource,
) {
    ui.param_grid("uf_p", |ui| {
        dv(
            ui,
            p(
                OperationType::UnifiedFinish,
                "steep_threshold_deg",
                "Steep Threshold:",
            ),
            &mut cfg.steep_threshold_deg,
            " deg",
            1.0,
            5.0..=85.0,
        );
        dv(
            ui,
            p(
                OperationType::UnifiedFinish,
                "waterline_threshold_deg",
                "Waterline Threshold:",
            ),
            &mut cfg.waterline_threshold_deg,
            " deg",
            1.0,
            // `.min(89.0)` keeps the range well-formed even when
            // Steep Threshold sits near its own 85° ceiling (85 + 5 =
            // 90 would otherwise invert against the 89° upper bound).
            (cfg.steep_threshold_deg + 5.0).min(89.0)..=89.0,
        );
        dv(
            ui,
            p(OperationType::UnifiedFinish, "overlap_mm", "Overlap:"),
            &mut cfg.overlap_mm,
            " mm",
            0.1,
            0.0..=10.0,
        );
        dv(
            ui,
            p(
                OperationType::UnifiedFinish,
                "scallop_height",
                "Scallop Height:",
            ),
            &mut cfg.scallop_height,
            " mm",
            0.01,
            0.01..=2.0,
        );
        dv(
            ui,
            p(OperationType::UnifiedFinish, "tolerance", "Tolerance:"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            0.01..=1.0,
        );
        dv(
            ui,
            p(
                OperationType::UnifiedFinish,
                "raster_stepover",
                "Raster Stepover:",
            ),
            &mut cfg.raster_stepover,
            " mm",
            0.1,
            0.05..=50.0,
        );
        dv(
            ui,
            p(OperationType::UnifiedFinish, "z_step", "Z Step:"),
            &mut cfg.z_step,
            " mm",
            0.1,
            0.05..=20.0,
        );
        dv(
            ui,
            p(OperationType::UnifiedFinish, "sampling", "Sampling:"),
            &mut cfg.sampling,
            " mm",
            0.1,
            0.1..=5.0,
        );
        dv(
            ui,
            p(
                OperationType::UnifiedFinish,
                "stock_to_leave",
                "Stock to Leave:",
            ),
            &mut cfg.stock_to_leave,
            " mm",
            0.05,
            0.0..=10.0,
        );

        // `dv` ends its own grid row, so this one opens straight after.
        // C2 (`planning/thin_organic_2026-08-27/`). A path-STRUCTURE
        // dial, so it sits with the other shallow-band geometry dials
        // rather than in the Rest Claims block below.
        ui.label("Monotone Cells:");
        ui.checkbox(&mut cfg.monotone_cell_decomposition, "")
            .on_hover_text(
                "Split each SHALLOW region into monotone cells on that region's own \
                     raster lattice, rotating the lattice to the region's PCA-minor axis \
                     where its elongation clears 3.0. Every cell rasters on that one \
                     shared lattice, so the relinker sees cell-shaped fragments instead \
                     of one dendritic region's worth. On by default since 2026-09-01 \
                     (C4 operator surface review passed); byte-identical to the old \
                     band when off. It is NOT a per-cell strategy: per-cell \
                     sweep directions, a cell visit order and contour-per-cell were each \
                     measured and are each slower. Measured under a realistic link \
                     ceiling: 1.155x across the reference relief's top three shallow \
                     regions, 1.215x on the elongated one — rig figures to approach, not \
                     promises. Cell seams change the cusp pattern, so look at the \
                     rendered surface before trusting it on a finish pass.",
            );
        ui.end_row();
    });
    draw_unified_finish_claims(ui, cfg, resolved_claims_reference, stock_source);
}

pub(in crate::ui::properties) fn draw_steep_shallow_params(
    ui: &mut egui::Ui,
    cfg: &mut SteepShallowConfig,
    pills: Option<&PillSuggestions>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    ui.param_grid("ss_p", |ui| {
        dv(
            ui,
            p(
                OperationType::SteepShallow,
                "threshold_angle",
                "Threshold Angle:",
            ),
            &mut cfg.threshold_angle,
            " deg",
            1.0,
            10.0..=80.0,
        );
        dv(
            ui,
            p(OperationType::SteepShallow, "overlap_distance", "Overlap:"),
            &mut cfg.overlap_distance,
            " mm",
            0.1,
            0.0..=10.0,
        );
        dv(
            ui,
            p(
                OperationType::SteepShallow,
                "wall_clearance",
                "Wall Clearance:",
            ),
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
            p(OperationType::SteepShallow, "stepover", "Stepover:"),
            &mut cfg.stepover,
            " mm",
            0.1,
            0.05..=50.0,
            stepover_sugg,
        );
        dv(
            ui,
            p(OperationType::SteepShallow, "z_step", "Z Step:"),
            &mut cfg.z_step,
            " mm",
            0.1,
            0.05..=20.0,
        );
        dv(
            ui,
            p(OperationType::SteepShallow, "sampling", "Sampling:"),
            &mut cfg.sampling,
            " mm",
            0.1,
            0.1..=5.0,
        );
        dv(
            ui,
            p(
                OperationType::SteepShallow,
                "stock_to_leave",
                "Stock to Leave:",
            ),
            &mut cfg.stock_to_leave,
            " mm",
            0.05,
            0.0..=10.0,
        );
        dv(
            ui,
            p(OperationType::SteepShallow, "tolerance", "Tolerance:"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            0.01..=1.0,
        );
    });
}
