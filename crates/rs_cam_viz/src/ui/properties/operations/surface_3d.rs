use super::super::pills::PillSuggestions;
use rs_cam_core::finish::pencil::PencilDetector;

use crate::state::toolpath::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClaimsReference, ClearingStrategy, DropCutterConfig,
    PencilConfig, RegionOrdering, ScallopConfig, ScallopDirection, SteepShallowConfig, StockSource,
    UnifiedFinishConfig, WaterlineConfig,
};

use super::super::linking_dressup::{
    Fields, check_in_row, check_param, combo_control, combo_in_row, combo_param, count_in_row,
    keyed_row, plain_row,
};
use super::super::{advanced, dv, dv_narrowed, dv_opt, dv_pill_opt, p};
use crate::ui::components::{DataRow, DataTable, ProvKind, Role, SuggestButton};
use rs_cam_core::compute::catalog::OperationType;

pub(in crate::ui::properties) fn draw_dropcutter_params(
    ui: &mut egui::Ui,
    cfg: &mut DropCutterConfig,
    pills: Option<&PillSuggestions>,
    fields: Fields<'_>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    dv_pill_opt(
        ui,
        p(OperationType::DropCutter, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.1,
        stepover_sugg,
        fields,
    );
    advanced(ui, "dc_adv", |ui| {
        dv(
            ui,
            p(OperationType::DropCutter, "min_z", "Min Z"),
            &mut cfg.min_z,
            " mm",
            0.5,
            fields,
        );
        dv(
            ui,
            p(OperationType::DropCutter, "slope_from", "Slope from"),
            &mut cfg.slope_from,
            " deg",
            1.0,
            fields,
        );
        dv(
            ui,
            p(OperationType::DropCutter, "slope_to", "Slope to"),
            &mut cfg.slope_to,
            " deg",
            1.0,
            fields,
        );
    });
}

pub(in crate::ui::properties) fn draw_adaptive3d_params(
    ui: &mut egui::Ui,
    cfg: &mut Adaptive3dConfig,
    pills: Option<&PillSuggestions>,
    strategy_suggestion: Option<ClearingStrategy>,
    contour_each_level_offered: bool,
    remaining_stock: bool,
    fields: Fields<'_>,
) {
    // Spec: pill stepover + depth_per_pass. Removal step R2: the
    // ContourSpiral "Optimal load" slider left with its strategy.
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    let dpp_sugg = pills.map(PillSuggestions::depth_per_pass);
    dv_pill_opt(
        ui,
        p(OperationType::Adaptive3d, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.1,
        stepover_sugg,
        fields,
    );
    dv_pill_opt(
        ui,
        p(OperationType::Adaptive3d, "depth_per_pass", "Depth/pass"),
        &mut cfg.depth_per_pass,
        " mm",
        0.1,
        dpp_sugg,
        fields,
    );
    dv(
        ui,
        p(
            OperationType::Adaptive3d,
            "stock_to_leave_axial",
            "Stock to leave",
        ),
        &mut cfg.stock_to_leave_axial,
        " mm",
        0.05,
        fields,
    );
    // F5 (W3b): the ONE Entry style control of a 3D Rough. The planner
    // plans its own entries from these rows (`rough_3d.rs`), and the
    // registry row takes no dressup entry (K4 `EntryContract::Planner`), so the
    // Linking tab draws no Entry style row for this op.
    combo_in_row(
        ui,
        keyed_row(
            "Entry style",
            "How the planner enters each level: Plunge, Helix or Ramp. The planner plans these entries itself.",
            &fields.state("entry_style"),
        ),
        "a3d_entry",
        match cfg.entry_style {
            Adaptive3dEntryStyle::Plunge => "Plunge",
            Adaptive3dEntryStyle::Helix => "Helix",
            Adaptive3dEntryStyle::Ramp => "Ramp",
        },
        |ui| {
            ui.selectable_value(&mut cfg.entry_style, Adaptive3dEntryStyle::Plunge, "Plunge");
            ui.selectable_value(&mut cfg.entry_style, Adaptive3dEntryStyle::Helix, "Helix");
            ui.selectable_value(&mut cfg.entry_style, Adaptive3dEntryStyle::Ramp, "Ramp");
        },
    );
    // Entry-style-specific parameters — only shown for the style in use.
    match cfg.entry_style {
        Adaptive3dEntryStyle::Plunge => {}
        Adaptive3dEntryStyle::Ramp => {
            dv(
                ui,
                p(OperationType::Adaptive3d, "ramp_angle_deg", "Ramp angle"),
                &mut cfg.ramp_angle_deg,
                " deg",
                0.5,
                fields,
            );
            dv(
                ui,
                p(
                    OperationType::Adaptive3d,
                    "entry_clearance_mm",
                    "Entry clearance",
                ),
                &mut cfg.entry_clearance_mm,
                " mm",
                0.1,
                fields,
            );
        }
        Adaptive3dEntryStyle::Helix => {
            dv(
                ui,
                p(
                    OperationType::Adaptive3d,
                    "helix_radius_factor",
                    "Helix radius",
                ),
                &mut cfg.helix_radius_factor,
                " × D",
                0.05,
                fields,
            );
            dv(
                ui,
                p(OperationType::Adaptive3d, "helix_pitch", "Helix pitch"),
                &mut cfg.helix_pitch,
                " mm",
                0.1,
                fields,
            );
            dv(
                ui,
                p(
                    OperationType::Adaptive3d,
                    "entry_clearance_mm",
                    "Entry clearance",
                ),
                &mut cfg.entry_clearance_mm,
                " mm",
                0.1,
                fields,
            );
        }
    }
    // G-D (§3): the Suggest advice is a ⚡ inside the Strategy well, not
    // a row. It goes when the draft already holds the recommendation.
    let suggestion = strategy_suggestion.filter(|s| *s != cfg.clearing_strategy);
    let state = fields.state("clearing_strategy");
    let out =
        keyed_row("Strategy", "The clearing strategy of each slice.", &state).show(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(recommended) = suggestion {
                    let chip = SuggestButton::field(ProvKind::RepoRule).hover(format!(
                        "Suggest: {} for this terrain (mixed-terrain classifier). Click to switch.",
                        strategy_label(recommended)
                    ));
                    if ui.add(chip).clicked() {
                        cfg.clearing_strategy = recommended;
                    }
                }
                combo_control(
                    ui,
                    "a3d_strat",
                    strategy_label(cfg.clearing_strategy),
                    |ui| {
                        ui.selectable_value(
                            &mut cfg.clearing_strategy,
                            ClearingStrategy::ContourParallel,
                            "Contour Parallel",
                        );
                        ui.selectable_value(
                            &mut cfg.clearing_strategy,
                            ClearingStrategy::GeodesicAdaptive,
                            "Geodesic Adaptive",
                        )
                        .on_hover_text(
                            "Geodesic-wavefront roughing per slice: spiral wraps that hold \
                     the pass load, with each tight turn fed down to that load. It \
                     clears the body only; follow it with a Contour Parallel rest \
                     op for the edges. New, not yet machine-tested.",
                        );
                    },
                )
            })
            .inner
        });
    let _ = out.inner.on_hover_text(&out.hover);
    // C4 (operator ruling D69): one visible row, shown where it acts: a
    // rest rough (remaining stock) with a machining boundary, any rough
    // in By Area order (C4b: the walls of each region at each level),
    // or while it is on (the Pocket "Cut past open sides" rule). The
    // hover is the registry help (UI-04: keyed on the parameter name).
    if contour_each_level_offered
        || cfg.contour_each_level
        || cfg.region_ordering == RegionOrdering::ByArea
    {
        check_param(
            ui,
            p(
                OperationType::Adaptive3d,
                "contour_each_level",
                "Contour each level",
            ),
            &mut cfg.contour_each_level,
            fields,
        );
    }
    // The rest region filter: two rows, shown on a rest rough (remaining
    // stock) or while a bound is set. Empty is off. The hover is the
    // registry help.
    if remaining_stock
        || cfg.rest_max_region_area_mm2.is_some()
        || cfg.rest_max_region_width_mm.is_some()
    {
        dv_opt(
            ui,
            p(
                OperationType::Adaptive3d,
                "rest_max_region_width_mm",
                "Max region width",
            ),
            &mut cfg.rest_max_region_width_mm,
            " mm",
            0.5,
            Some("off"),
            fields,
        );
        dv_opt(
            ui,
            p(
                OperationType::Adaptive3d,
                "rest_max_region_area_mm2",
                "Max region area",
            ),
            &mut cfg.rest_max_region_area_mm2,
            " mm²",
            10.0,
            Some("off"),
            fields,
        );
    }
    advanced(ui, "a3d_adv", |ui| {
        dv(
            ui,
            p(OperationType::Adaptive3d, "tolerance", "Tolerance"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            fields,
        );
        dv(
            ui,
            p(
                OperationType::Adaptive3d,
                "min_cutting_radius",
                "Min cut radius",
            ),
            &mut cfg.min_cutting_radius,
            " mm",
            0.1,
            fields,
        );
        check_in_row(
            ui,
            plain_row("Detect flat", Some("Detect the flat areas of the model.")),
            &mut cfg.detect_flat_areas,
        );
        combo_in_row(
            ui,
            plain_row(
                "Ordering",
                Some(
                    "Global cuts every region at one level before the next level. By Area cuts each region before the next.",
                ),
            ),
            "a3d_ord",
            match cfg.region_ordering {
                RegionOrdering::Global => "Global",
                RegionOrdering::ByArea => "By Area",
            },
            |ui| {
                ui.selectable_value(&mut cfg.region_ordering, RegionOrdering::Global, "Global");
                ui.selectable_value(&mut cfg.region_ordering, RegionOrdering::ByArea, "By Area");
            },
        );
    });
}

/// The label of a clearing strategy in the Strategy combo and its ⚡ hover.
fn strategy_label(strategy: ClearingStrategy) -> &'static str {
    match strategy {
        ClearingStrategy::ContourParallel => "Contour Parallel",
        ClearingStrategy::GeodesicAdaptive => "Geodesic Adaptive",
    }
}

pub(in crate::ui::properties) fn draw_waterline_params(
    ui: &mut egui::Ui,
    cfg: &mut WaterlineConfig,
    _pills: Option<&PillSuggestions>,
    fields: Fields<'_>,
) {
    // Waterline: z_step is the axial pass spacing, but Step 3 LUT mapping
    // (axial_depth_mm) is calibrated for clearing DOC, not contour Z-step.
    // Leave Z Step alone; feed/plunge live on the Feeds tab (W3.2).
    dv_opt(
        ui,
        p(OperationType::Waterline, "z_step", "Z step"),
        &mut cfg.z_step,
        " mm",
        0.1,
        None,
        fields,
    );
    // W1b: the field is `helical`; the label names it, and the hover is
    // its registry help (UI-04: keyed on the parameter name).
    check_param(
        ui,
        p(OperationType::Waterline, "helical", "Helical"),
        &mut cfg.helical,
        fields,
    );
    // Z range now comes from the Heights tab (top_z / bottom_z)
    advanced(ui, "wl_adv", |ui| {
        dv(
            ui,
            p(OperationType::Waterline, "sampling", "Sampling"),
            &mut cfg.sampling,
            " mm",
            0.1,
            fields,
        );
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
    fields: Fields<'_>,
) {
    // Pencil's offset_stepover is a parallel-pass spacing, not the same
    // shape as a clearing radial WOC; leave it alone. Feed/plunge live on
    // the Feeds tab (W3.2).
    // FIN-09: the config field is `PencilDetector`, so the panel compares
    // variants. The old string compares accepted four aliases the generator
    // never honoured.
    let curvature = cfg.detector == PencilDetector::Curvature;
    let rest_depth = cfg.detector == PencilDetector::RestDepth;
    // Valley-detection front-end. Rest depth = dual-tool rest field (the
    // aligned detector — the reference tool decides where pencil runs);
    // Dihedral = mesh-crease detection (clean CAD-style corners);
    // Curvature = curvature crest lines (dialled by Valley Saliency).
    let selected = if rest_depth {
        "Rest depth (recommended)"
    } else if curvature {
        "Curvature (crest)"
    } else {
        "Dihedral (crease)"
    };
    combo_in_row(
        ui,
        plain_row(
            "Detector",
            Some(
                "How pencil finds its valleys: the rest depth, the mesh creases or the curvature crests.",
            ),
        ),
        "pen_detector",
        selected,
        |ui| {
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
        },
    );
    if rest_depth {
        // Rest-depth dial. Rest Cell is the XY grid resolution of the
        // rest field. FIN-04 deleted the retired "Route Width x"
        // widget with the field it wrote.
        dv(
            ui,
            p(OperationType::Pencil, "rest_cell_mm", "Rest cell"),
            &mut cfg.rest_cell_mm,
            " mm",
            0.05,
            fields,
        );
    } else if curvature {
        // Curvature-detector dials. Valley Saliency is the significance
        // knob: min concave curvature |κ₂| (1/mm) a valley must reach —
        // low traces every seam, high keeps only deep sharp valleys.
        dv(
            ui,
            p(OperationType::Pencil, "valley_saliency", "Valley saliency"),
            &mut cfg.valley_saliency,
            " 1/mm",
            0.01,
            fields,
        );
        count_in_row(
            ui,
            plain_row(
                "Curvature smoothing",
                Some("The smoothing of the curvature field before the crest lines are found."),
            ),
            &mut cfg.curvature_smoothing,
            0.1,
            (OperationType::Pencil, "curvature_smoothing"),
        );
    } else {
        dv(
            ui,
            p(
                OperationType::Pencil,
                "bitangency_angle",
                "Bitangency angle",
            ),
            &mut cfg.bitangency_angle,
            " deg",
            1.0,
            fields,
        );
    }
    count_in_row(
        ui,
        plain_row(
            "Offset passes",
            Some("The number of offset passes beside each valley line."),
        ),
        &mut cfg.num_offset_passes,
        0.1,
        (OperationType::Pencil, "num_offset_passes"),
    );
    dv_opt(
        ui,
        p(OperationType::Pencil, "offset_stepover", "Offset stepover"),
        &mut cfg.offset_stepover,
        " mm",
        0.1,
        None,
        fields,
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
            "Min valley depth",
        ),
        &mut cfg.min_valley_depth,
        " mm",
        0.05,
        fields,
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
        let ref_label = cfg
            .reference_tool_id
            .and_then(|rid| tools.iter().find(|(id, _, _)| *id == rid))
            .map(|(_, s, _)| s.as_str())
            .unwrap_or("Nominal Ø");
        combo_param(
            ui,
            p(OperationType::Pencil, "reference_tool_id", "Reference"),
            fields,
            "pen_reference_tool",
            ref_label,
            |ui| {
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
            },
        );
        // The nominal diameter only applies when no real tool is chosen.
        if cfg.reference_tool_id.is_none() {
            dv(
                ui,
                p(
                    OperationType::Pencil,
                    "reference_tool_diameter",
                    "Reference tool Ø",
                ),
                &mut cfg.reference_tool_diameter,
                " mm",
                0.5,
                fields,
            );
        }
    }
    dv(
        ui,
        p(OperationType::Pencil, "stock_to_leave", "Stock to leave"),
        &mut cfg.stock_to_leave,
        " mm",
        0.05,
        fields,
    );
    advanced(ui, "pen_adv", |ui| {
        dv(
            ui,
            p(OperationType::Pencil, "min_cut_length", "Min cut length"),
            &mut cfg.min_cut_length,
            " mm",
            0.5,
            fields,
        );
        dv(
            ui,
            p(OperationType::Pencil, "hookup_distance", "Hookup distance"),
            &mut cfg.hookup_distance,
            " mm",
            0.5,
            fields,
        );
        dv(
            ui,
            p(OperationType::Pencil, "sampling", "Sampling"),
            &mut cfg.sampling,
            " mm",
            0.1,
            fields,
        );
    });
}

pub(in crate::ui::properties) fn draw_scallop_params(
    ui: &mut egui::Ui,
    cfg: &mut ScallopConfig,
    _pills: Option<&PillSuggestions>,
    fields: Fields<'_>,
) {
    // Scallop's stepover is computed from scallop_height + tool radius, not
    // an editable field, so no stepover pill. Feed/plunge live on the Feeds
    // tab (W3.2).
    dv(
        ui,
        p(OperationType::Scallop, "scallop_height", "Scallop height"),
        &mut cfg.scallop_height,
        " mm",
        0.01,
        fields,
    );
    combo_in_row(
        ui,
        plain_row(
            "Direction",
            Some("Outside in starts at the boundary. Inside out starts at the centre."),
        ),
        "sc_dir",
        match cfg.direction {
            ScallopDirection::OutsideIn => "Outside in",
            ScallopDirection::InsideOut => "Inside out",
        },
        |ui| {
            ui.selectable_value(
                &mut cfg.direction,
                ScallopDirection::OutsideIn,
                "Outside in",
            );
            ui.selectable_value(
                &mut cfg.direction,
                ScallopDirection::InsideOut,
                "Inside out",
            );
        },
    );
    dv(
        ui,
        p(OperationType::Scallop, "stock_to_leave", "Stock to leave"),
        &mut cfg.stock_to_leave,
        " mm",
        0.05,
        fields,
    );
    advanced(ui, "sc_adv", |ui| {
        dv(
            ui,
            p(OperationType::Scallop, "tolerance", "Tolerance"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            fields,
        );
        check_in_row(
            ui,
            plain_row(
                "Continuous",
                Some("Join the rings into one continuous path."),
            ),
            &mut cfg.continuous,
        );
        check_in_row(
            ui,
            plain_row(
                "Iso-field rings",
                Some(
                    "Rings from the iso-scallop field instead of the offset \
                     cascade: spacing varies point-by-point along each ring \
                     (organic, terrain-following), uses the spec-correct cosine \
                     slope law, and always runs to completion. Measured faster \
                     than the band mix at better coverage on terrain work.",
                ),
            ),
            &mut cfg.iso_field,
        );
        dv(
            ui,
            p(OperationType::Scallop, "slope_from", "Slope from"),
            &mut cfg.slope_from,
            " deg",
            1.0,
            fields,
        );
        dv(
            ui,
            p(OperationType::Scallop, "slope_to", "Slope to"),
            &mut cfg.slope_to,
            " deg",
            1.0,
            fields,
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
    fields: Fields<'_>,
) {
    ui.add_space(6.0);
    crate::ui::components::SectionHeader::new("Rest claims")
        .rule(false)
        .show(ui);
    check_in_row(
        ui,
        plain_row(
            "Pencil claims",
            Some(
                "Run the crease/rest detector inside this operation and cut its \
                 claimed valleys as an extra pass. Off by default: on a \
                 single-tool op the analytic detector marks exactly what this \
                 cutter cannot reach. Every dial below is INERT while this is off.",
            ),
        ),
        &mut cfg.pencil_claims,
    );

    combo_in_row(
        ui,
        plain_row(
            "Rest reference",
            Some("What the claims pipeline measures rest against."),
        ),
        "uf_claims_ref",
        match cfg.claims_reference {
            ClaimsReference::Auto => "Auto (derive)",
            ClaimsReference::SelfProbe => "Analytic self-probe",
            ClaimsReference::MachinedStock => "Machined prior stock",
        },
        |ui| {
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
        },
    );

    check_in_row(
        ui,
        plain_row(
            "Territory clip",
            Some(
                "Confine generation to rest ISLANDS instead of the full surface. \
                 Runs only under the machined-stock reference — under a self-probe \
                 reference the 'rest islands' are geometric, not material, so the \
                 clip is skipped and this becomes an all-over pass.",
            ),
        ),
        &mut cfg.territory_clip,
    );

    dv(
        ui,
        p(
            OperationType::UnifiedFinish,
            "min_rest_depth_mm",
            "Min rest depth",
        ),
        &mut cfg.min_rest_depth_mm,
        " mm",
        0.005,
        fields,
    );

    // The resolved reference: what the LAST generation used, not what the
    // dial says. `None` before the operation has generated, or when claims
    // are off and nothing was resolved. Round 2: the readouts are rows of one
    // table; the reason is on each row's hover.
    let mut table = DataTable::new("unified_finish_rest_reference");
    match resolved {
        Some(f) => {
            let r = f.resolution;
            let loud = r.needs_attention() || f.territory_clip_skipped();
            let used = match r.reference() {
                rs_cam_core::finish::unified_finish::CreaseReference::MachinedStock => {
                    "machined prior stock"
                }
                rs_cam_core::finish::unified_finish::CreaseReference::SelfProbe => {
                    "analytic self-probe"
                }
            };
            let verb = if r.is_derived() { "derived" } else { "pinned" };
            let (role, word) = if loud {
                (Role::Caution, "check")
            } else {
                (Role::Ok, "ok")
            };
            table = table.row(
                DataRow::value("Rest reference", format!("{used} ({verb})"))
                    .status(role, word)
                    .hover(r.why()),
            );
            if f.territory_clip_skipped() {
                table = table.row(
                    DataRow::note(Role::Caution, "\u{26A0} Territory Clip skipped").hover(
                        "Territory Clip was requested and skipped: this pass covered \
                         its full territory, not rest islands.",
                    ),
                );
            }
        }
        None if cfg.pencil_claims => {
            // UNKNOWN, not a grey. Nothing has been measured yet, and §2.6
            // separates "not run" from "passed" so the two cannot be read
            // alike at a glance.
            table =
                table
                    .row(DataRow::not_measured("Rest reference").hover(
                        "The resolved rest reference shows here after this operation runs.",
                    ));
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
        table = table.row(
            DataRow::note(Role::Caution, "\u{26A0} Fresh stock: no machined prior").hover(
                "This operation cuts fresh stock, so the machined-stock reference \
                 cannot be used. Turn on Rest machining: from previous ops, then generate \
                 and simulate the upstream operation.",
            ),
        );
    }
    table.show(ui);
}

pub(in crate::ui::properties) fn draw_unified_finish_params(
    ui: &mut egui::Ui,
    cfg: &mut UnifiedFinishConfig,
    _pills: Option<&PillSuggestions>,
    resolved_claims_reference: Option<rs_cam_core::compute::toolpath_stats::ClaimsReferenceFinding>,
    stock_source: StockSource,
    fields: Fields<'_>,
) {
    dv(
        ui,
        p(
            OperationType::UnifiedFinish,
            "steep_threshold_deg",
            "Steep threshold",
        ),
        &mut cfg.steep_threshold_deg,
        " deg",
        1.0,
        fields,
    );
    dv_narrowed(
        ui,
        p(
            OperationType::UnifiedFinish,
            "waterline_threshold_deg",
            "Waterline threshold",
        ),
        &mut cfg.waterline_threshold_deg,
        " deg",
        1.0,
        // `.min(89.0)` keeps the range well-formed even when
        // Steep Threshold sits near its own 85° ceiling (85 + 5 =
        // 90 would otherwise invert against the 89° upper bound).
        (cfg.steep_threshold_deg + 5.0).min(89.0)..=89.0,
        fields,
    );
    dv(
        ui,
        p(
            OperationType::UnifiedFinish,
            "scallop_height",
            "Scallop height",
        ),
        &mut cfg.scallop_height,
        " mm",
        0.01,
        fields,
    );
    dv_opt(
        ui,
        p(
            OperationType::UnifiedFinish,
            "raster_stepover",
            "Raster stepover",
        ),
        &mut cfg.raster_stepover,
        " mm",
        0.1,
        None,
        fields,
    );
    dv_opt(
        ui,
        p(OperationType::UnifiedFinish, "z_step", "Z step"),
        &mut cfg.z_step,
        " mm",
        0.1,
        None,
        fields,
    );
    dv(
        ui,
        p(
            OperationType::UnifiedFinish,
            "stock_to_leave",
            "Stock to leave",
        ),
        &mut cfg.stock_to_leave,
        " mm",
        0.05,
        fields,
    );
    advanced(ui, "uf_adv", |ui| {
        dv(
            ui,
            p(OperationType::UnifiedFinish, "overlap_mm", "Overlap"),
            &mut cfg.overlap_mm,
            " mm",
            0.1,
            fields,
        );
        dv(
            ui,
            p(OperationType::UnifiedFinish, "tolerance", "Tolerance"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            fields,
        );
        dv(
            ui,
            p(OperationType::UnifiedFinish, "sampling", "Sampling"),
            &mut cfg.sampling,
            " mm",
            0.1,
            fields,
        );

        // C2 (`planning/thin_organic_2026-08-27/`). A path-STRUCTURE
        // dial, so it sits with the other shallow-band geometry dials
        // rather than in the Rest Claims block below.
        check_in_row(
            ui,
            plain_row("Monotone cells", Some(MONOTONE_CELLS_HOVER)),
            &mut cfg.monotone_cell_decomposition,
        );
    });
    draw_unified_finish_claims(ui, cfg, resolved_claims_reference, stock_source, fields);
}

/// The hover of the Unified Finish "Monotone cells" row (C2).
const MONOTONE_CELLS_HOVER: &str = "Split each SHALLOW region into monotone cells on that region's own \
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
                     rendered surface before trusting it on a finish pass.";

pub(in crate::ui::properties) fn draw_steep_shallow_params(
    ui: &mut egui::Ui,
    cfg: &mut SteepShallowConfig,
    pills: Option<&PillSuggestions>,
    fields: Fields<'_>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    dv(
        ui,
        p(
            OperationType::SteepShallow,
            "threshold_angle",
            "Threshold angle",
        ),
        &mut cfg.threshold_angle,
        " deg",
        1.0,
        fields,
    );
    dv_pill_opt(
        ui,
        p(OperationType::SteepShallow, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.1,
        stepover_sugg,
        fields,
    );
    dv_opt(
        ui,
        p(OperationType::SteepShallow, "z_step", "Z step"),
        &mut cfg.z_step,
        " mm",
        0.1,
        None,
        fields,
    );
    dv(
        ui,
        p(
            OperationType::SteepShallow,
            "stock_to_leave",
            "Stock to leave",
        ),
        &mut cfg.stock_to_leave,
        " mm",
        0.05,
        fields,
    );
    advanced(ui, "ss_adv", |ui| {
        dv(
            ui,
            p(OperationType::SteepShallow, "overlap_distance", "Overlap"),
            &mut cfg.overlap_distance,
            " mm",
            0.1,
            fields,
        );
        dv(
            ui,
            p(
                OperationType::SteepShallow,
                "wall_clearance",
                "Wall clearance",
            ),
            &mut cfg.wall_clearance,
            " mm",
            0.1,
            fields,
        );
        check_in_row(
            ui,
            plain_row(
                "Steep first",
                Some("Cut the steep areas before the shallow areas."),
            ),
            &mut cfg.steep_first,
        );
        dv(
            ui,
            p(OperationType::SteepShallow, "sampling", "Sampling"),
            &mut cfg.sampling,
            " mm",
            0.1,
            fields,
        );
        dv(
            ui,
            p(OperationType::SteepShallow, "tolerance", "Tolerance"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            fields,
        );
    });
}
