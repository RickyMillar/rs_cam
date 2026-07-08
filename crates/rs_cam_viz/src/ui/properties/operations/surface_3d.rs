use rs_cam_core::adaptive_shared::{
    radial_woc_fraction_from_leading_arc, target_engagement_fraction,
};
use rs_cam_core::feeds::FeedsResult;

use crate::state::toolpath::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, DropCutterConfig, PencilConfig,
    RegionOrdering, ScallopConfig, ScallopDirection, SteepShallowConfig, StockSource,
    UnifiedFinishConfig, WaterlineConfig,
};

use super::super::{dv, dv_pill};

/// Fallback tool radius (1/8" endmill) when the active tool's radius is
/// unavailable or non-physical — keeps the load↔stepover bridge finite.
const FALLBACK_TOOL_RADIUS: f64 = 3.175;

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
    tool_radius: f64,
    feeds_result: Option<&FeedsResult>,
) {
    // Spec: pill stepover + depth_per_pass; leave fine_stepdown alone
    // (finishing-pass param the LUT doesn't speak to).
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    let dpp_sugg = feeds_result.map(|r| (r.axial_depth_mm, &r.chipload_source));
    // The ContourSpiral strategy holds engagement flat by construction, so
    // its primary knob is the friendly "Optimal load" slider (it derives
    // the stepover). The raw stepover pill is retired for that strategy and
    // shown derived instead.
    let spiral = matches!(cfg.clearing_strategy, ClearingStrategy::ContourSpiral);
    egui::Grid::new("a3d_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            if spiral {
                draw_spiral_load_control(ui, cfg, tool_radius);
            } else {
                dv_pill(
                    ui,
                    "Stepover:",
                    &mut cfg.stepover,
                    " mm",
                    0.1,
                    0.05..=50.0,
                    stepover_sugg,
                );
            }
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
                "Stock to Leave:",
                &mut cfg.stock_to_leave_axial,
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
            .color(egui::Color32::from_rgb(140, 140, 150)),
    );
    ui.end_row();
}

pub(in crate::ui::properties) fn draw_waterline_params(
    ui: &mut egui::Ui,
    cfg: &mut WaterlineConfig,
    _feeds_result: Option<&FeedsResult>,
) {
    // Waterline: z_step is the axial pass spacing, but Step 3 LUT mapping
    // (axial_depth_mm) is calibrated for clearing DOC, not contour Z-step.
    // Leave Z Step alone; feed/plunge live on the Feeds tab (W3.2).
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
        });
}

pub(in crate::ui::properties) fn draw_pencil_params(
    ui: &mut egui::Ui,
    cfg: &mut PencilConfig,
    tools: &[(crate::state::job::ToolId, String, f64)],
    _feeds_result: Option<&FeedsResult>,
    // P2 pencil-panel consolidation (2026-07): the "Rest reference" group
    // below owns `stock_source` directly (Fresh ⇔ reference tool, Remaining
    // Stock ⇔ machined stock) instead of leaving it to the separate generic
    // "Use remaining stock" checkbox the properties panel used to show for
    // every op — that checkbox silently overrode this panel's reference-tool
    // pick at generation time (`rest_depth_arm`'s R2 stock preference). The
    // panel is special-cased for this one call site rather than widening
    // every `draw_*_params` signature.
    stock_source: &mut StockSource,
    // Set true when the group above changes `stock_source`; the caller
    // translates this into `entry.stale_since = Some(Instant::now())`
    // (mirrors how the old checkbox marked itself stale in
    // `properties/mod.rs`). Changes to `cfg` itself are already covered by
    // the generic op-before/op-after snapshot in the caller.
    stale: &mut bool,
) {
    // Pencil's offset_stepover is a parallel-pass spacing, not the same
    // shape as a clearing radial WOC; leave it alone. Feed/plunge live on
    // the Feeds tab (W3.2).
    let curvature = cfg.detector.trim().eq_ignore_ascii_case("curvature");
    let rest_depth = {
        let d = cfg.detector.trim();
        d.eq_ignore_ascii_case("rest_depth")
            || d.eq_ignore_ascii_case("restdepth")
            || d.eq_ignore_ascii_case("rest")
    };
    egui::Grid::new("pen_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
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
                        cfg.detector = "rest_depth".to_owned();
                    }
                    if ui
                        .selectable_label(!curvature && !rest_depth, "Dihedral (crease)")
                        .clicked()
                    {
                        cfg.detector = "dihedral".to_owned();
                    }
                    if ui
                        .selectable_label(curvature, "Curvature (crest)")
                        .clicked()
                    {
                        cfg.detector = "curvature".to_owned();
                    }
                });
            ui.end_row();
            if rest_depth {
                // Rest-depth dials. Rest Cell is the XY grid resolution; Route
                // Width × sets how wide a rest region may be before it routes to
                // clearing instead of a single pencil centreline (× pencil radius).
                dv(
                    ui,
                    "Rest Cell:",
                    &mut cfg.rest_cell_mm,
                    " mm",
                    0.05,
                    0.1..=2.0,
                );
                dv(
                    ui,
                    "Route Width ×:",
                    &mut cfg.route_width_factor,
                    "",
                    0.1,
                    0.5..=10.0,
                );
                ui.end_row();
            } else if curvature {
                // Curvature-detector dials. Valley Saliency is the significance
                // knob: min concave curvature |κ₂| (1/mm) a valley must reach —
                // low traces every seam, high keeps only deep sharp valleys.
                dv(
                    ui,
                    "Valley Saliency:",
                    &mut cfg.valley_saliency,
                    " 1/mm",
                    0.01,
                    0.0..=2.0,
                );
                ui.label("Curv. Smoothing:");
                let mut s = cfg.curvature_smoothing as i32;
                if ui.add(egui::DragValue::new(&mut s).range(0..=20)).changed() {
                    cfg.curvature_smoothing = s.max(0) as usize;
                }
                ui.end_row();
            } else {
                dv(
                    ui,
                    "Bitangency Angle:",
                    &mut cfg.bitangency_angle,
                    " deg",
                    1.0,
                    90.0..=180.0,
                );
            }
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
            // Reference-tool rest gate: keep only valleys the pencil tool (the
            // op's own tool) reaches more than this deeper than the bigger
            // reference finish tool. 0 = off (trace every detected valley); raise
            // to skip shallow/already-reachable seams.
            dv(
                ui,
                "Min Valley Depth:",
                &mut cfg.min_valley_depth,
                " mm",
                0.05,
                0.0..=5.0,
            );
            // Rest reference: what "rest" is measured against — feeds every
            // detector's rest gate (RestDepth's own field IS this reference;
            // Dihedral/Curvature's reach-gap, `min_valley_depth`, also gates
            // off it), so this is shown for all three detectors, not just
            // RestDepth. Two ways to supply it, unified into one choice
            // instead of two overlapping controls:
            // - Machined stock (`stock_source = FromRemainingStock`): R2, the
            //   actual simulated stock a prior toolpath left.
            // - Reference tool (`stock_source = Fresh`): R1, a real library
            //   tool's true geometry or a synthetic nominal-Ø ball.
            ui.label("Rest reference:").on_hover_text(
                "What 'rest' is measured against — the material a previous \
                 step left. Machined stock uses the simulated result of \
                 prior ops (most accurate, needs a simulation first); a \
                 reference tool approximates it analytically.",
            );
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(
                        *stock_source == StockSource::FromRemainingStock,
                        "Machined stock (requires simulation)",
                    )
                    .clicked()
                    && *stock_source != StockSource::FromRemainingStock
                {
                    *stock_source = StockSource::FromRemainingStock;
                    *stale = true;
                }
                if ui
                    .selectable_label(*stock_source == StockSource::Fresh, "Reference tool")
                    .clicked()
                    && *stock_source != StockSource::Fresh
                {
                    *stock_source = StockSource::Fresh;
                    *stale = true;
                }
            });
            ui.end_row();
            if *stock_source == StockSource::FromRemainingStock {
                // `rest_depth_arm` (and the generic
                // `attach_generic_rest_analysis`) require the simulated
                // stock's XY bbox to overlap this model's — a silent frame
                // mismatch would read garbage rest everywhere, so instead
                // they fall back to the reference-tool resolution below.
                ui.label("");
                ui.label(
                    egui::RichText::new(
                        "Falls back to the reference tool if the simulated \
                         stock doesn't overlap this model's frame.",
                    )
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
                );
                ui.end_row();
            } else {
                // R1: "Nominal Ø" = a synthetic ball at Reference Tool Ø; or
                // pick a real library tool whose TRUE geometry (flat/vbit/
                // tapered) defines the rest — a flat leaves a different rest
                // shape than a ball of the same diameter.
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
                        "Reference Tool Ø:",
                        &mut cfg.reference_tool_diameter,
                        " mm",
                        0.5,
                        0.5..=25.0,
                    );
                }
            }
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
    _feeds_result: Option<&FeedsResult>,
) {
    // Scallop's stepover is computed from scallop_height + tool radius, not
    // an editable field, so no stepover pill. Feed/plunge live on the Feeds
    // tab (W3.2).
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

/// P2.c orchestrator params (`planning/unified_finish_planner_design.md`).
/// Mirrors `draw_scallop_params`' shape closely: no stepover pill (raster
/// stepover here is a plain editable field, not chipload-derived), no
/// feed/plunge/spindle widgets (those live on the Feeds tab, same as every
/// other 3D finish op's params panel — see the comment on
/// `draw_scallop_params`). No stepover-pattern diagram either (same
/// silent-gap acceptance as `StepoverPattern::from_operation`'s `_ => None`
/// fallback covers Scallop).
pub(in crate::ui::properties) fn draw_unified_finish_params(
    ui: &mut egui::Ui,
    cfg: &mut UnifiedFinishConfig,
    _feeds_result: Option<&FeedsResult>,
) {
    egui::Grid::new("uf_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Steep Threshold:",
                &mut cfg.steep_threshold_deg,
                " deg",
                1.0,
                5.0..=85.0,
            );
            dv(
                ui,
                "Waterline Threshold:",
                &mut cfg.waterline_threshold_deg,
                " deg",
                1.0,
                // `.min(89.0)` keeps the range well-formed even when
                // Steep Threshold sits near its own 85° ceiling (85 + 5 =
                // 90 would otherwise invert against the 89° upper bound).
                (cfg.steep_threshold_deg + 5.0).min(89.0)..=89.0,
            );
            dv(ui, "Overlap:", &mut cfg.overlap_mm, " mm", 0.1, 0.0..=10.0);
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
            dv(
                ui,
                "Raster Stepover:",
                &mut cfg.raster_stepover,
                " mm",
                0.1,
                0.05..=50.0,
            );
            dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
            dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
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
