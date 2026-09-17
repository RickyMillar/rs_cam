mod boundary_2d;
mod drill;
mod engrave;
mod finishing;
mod height_diagram;
mod project;
pub mod registry;
mod shape_diagrams;
mod surface_3d;
mod validate;

// Re-export all draw_*_params functions so callers don't need to change imports.
pub(super) use boundary_2d::{
    draw_adaptive_params, draw_face_params, draw_inlay_params, draw_pocket_params,
    draw_profile_params, draw_rest_params, draw_vcarve_params, draw_zigzag_params,
};
pub(super) use drill::{draw_alignment_pin_drill_params, draw_drill_params};
pub(super) use engrave::{draw_chamfer_params, draw_trace_params};
pub(super) use finishing::{
    draw_horizontal_finish_params, draw_radial_finish_params, draw_ramp_finish_params,
    draw_spiral_finish_params,
};
pub(super) use project::draw_project_curve_params;
pub(super) use surface_3d::{
    draw_adaptive3d_params, draw_dropcutter_params, draw_pencil_params, draw_scallop_params,
    draw_steep_shallow_params, draw_unified_finish_params, draw_waterline_params,
};

// The P4 split moved three topics into children beside this file. Each name
// keeps its module path through these re-exports, so `ui/properties/mod.rs`
// and the sibling operation editors import exactly what they imported before.
pub use height_diagram::draw_height_diagram;
pub(super) use shape_diagrams::{
    StepoverPattern, draw_dogbone_diagram, draw_inlay_diagram, draw_lead_in_out_diagram,
    draw_outline_diagram, draw_pencil_diagram, draw_point_set_diagram, draw_radial_diagram,
    draw_ramp_finish_diagram, draw_spiral_diagram, draw_steep_shallow_diagram,
    draw_stepover_diagram, draw_tab_diagram,
};
pub use validate::{
    DepthBeyondStock, ThroughCut, ToolpathValidationContext, collect_diagnostics,
    depth_beyond_stock, profile_through_cut, profile_through_cut_line, validate_toolpath,
    validate_toolpath_config,
};

use crate::state::toolpath::{
    HeightContext, HeightMode, HeightReference, HeightsConfig, OperationType, ReferenceOffset,
};

// Feed / plunge editing moved off the per-op Geometry panel into the
// Feeds & Speeds tab's SPEED section (W3.2) — the `draw_feed_params` helper
// (and its `FeedsResult` / `dv_pill` deps) was retired with the recharter.
//
// The per-operation spindle RPM override moved to the Feeds-card SPEED section
// (W3.1) as a `PrecedenceField`, where the project default RPM is reachable and
// the override-vs-default precedence renders honestly. The old in-place
// `draw_spindle_rpm_row` (hardcoded 18 000 default) was retired.

// ── Heights panel ────────────────────────────────────────────────────────

/// How one height row is *rendered*: every mode is shown as an offset from a
/// named reference, including `Auto` and `Manual`.
///
/// This projection is a **pure read** — the signature takes `&HeightMode` so it
/// cannot write, and that is the whole point.
///
/// G-HEIGHTSTAB (2026-08-23): the row used to promote `Auto` →
/// `FromReference(default)` and `Manual` → `FromReference(nearest)` at the top
/// of its *draw* function, so merely rendering the Heights tab rewrote the
/// stored config. The panel's write-back (`write_entry_config_to_session`) then
/// saw `heights` change, stamped `stale_since`, and auto-regeneration took the
/// op apart. On any operation whose `default_depth_for_heights()` is 0 — the
/// entire 3D family, which reports `DepthSemantics::None` — the promoted bottom
/// row became `FromReference { StockTop, -0.0 }`: bottom == stock top, and the
/// promotion also flipped `ResolvedHeights::bottom_pinned` from false to true,
/// so the operation honoured a floor at the stock top and regenerated to **zero
/// moves**. Opening a read-only tab destroyed a healthy 7 672-move toolpath.
///
/// Nothing here may write to `mode`; see [`commit_height_row`] for the one
/// place that does, and what gates it.
fn height_row_display(
    mode: &HeightMode,
    default_ref: HeightReference,
    default_z: f64,
    ctx: &HeightContext,
) -> ReferenceOffset {
    match *mode {
        HeightMode::FromReference(ref_offset) => ref_offset,
        // `default_z` is the absolute Z the CORE resolver produces for this
        // row, so the displayed value can never disagree with what generation
        // actually does.
        HeightMode::Auto => ReferenceOffset {
            reference: default_ref,
            offset: default_z - default_ref.resolve_z(ctx),
        },
        HeightMode::Manual(abs_z) => {
            let best_ref = find_nearest_reference(abs_z, ctx);
            ReferenceOffset {
                reference: best_ref,
                offset: abs_z - best_ref.resolve_z(ctx),
            }
        }
    }
}

/// Write an edited row back to the stored mode.
///
/// `edited` must be true only when the user moved a widget **this frame**
/// (a DragValue change or a reference pick). Rendering alone never sets it, so
/// viewing the Heights tab leaves `Auto` as `Auto` — which is what keeps
/// `bottom_pinned` false and the operation's own depth semantics in charge.
fn commit_height_row(mode: &mut HeightMode, display: ReferenceOffset, edited: bool) {
    if edited {
        *mode = HeightMode::FromReference(display);
    }
}

/// F360-style height row: [offset value] [from Reference ▾]
///
/// `default_z` is the absolute Z the core resolver produces for this row (see
/// [`height_row_display`]). An `Auto` row displays that value and stays `Auto`
/// until the user actually edits the row.
#[allow(clippy::too_many_arguments)]
fn draw_height_row(
    ui: &mut egui::Ui,
    label: &str,
    tooltip: &str,
    mode: &mut HeightMode,
    default_ref: HeightReference,
    default_z: f64,
    ctx: &HeightContext,
    id_salt: &str,
) {
    let was_auto = mode.is_auto();
    let mut display = height_row_display(mode, default_ref, default_z, ctx);

    ui.label(label).on_hover_text(tooltip);

    let mut edited = ui
        .add(
            egui::DragValue::new(&mut display.offset)
                .suffix(" mm")
                .speed(0.5)
                .range(-500.0..=500.0),
        )
        .changed();

    let combo = egui::ComboBox::from_id_salt(format!("hr_{id_salt}"))
        .width(105.0)
        .selected_text(ref_label(display.reference, display.offset))
        .show_ui(ui, |ui| {
            let mut picked = false;
            for &href in HeightReference::ALL {
                // `.clicked()` rather than `.changed()`: re-picking the
                // reference an Auto row is merely *displaying* is still an
                // explicit "pin it here" from the operator.
                picked |= ui
                    .selectable_value(&mut display.reference, href, href.label())
                    .clicked();
            }
            picked
        });
    edited |= combo.inner.unwrap_or(false);

    commit_height_row(mode, display, edited);

    // Resolved absolute Z as a dim hint. "(auto)" marks a row that is still
    // deferring to the resolver rather than carrying a pinned value.
    let resolved = display.reference.resolve_z(ctx) + display.offset;
    let hint = if was_auto && !edited {
        format!("= {resolved:.1} (auto)")
    } else {
        format!("= {resolved:.1}")
    };
    ui.label(
        egui::RichText::new(hint)
            .small()
            .color(crate::ui::tokens::DIAGRAM_DIM),
    );

    ui.end_row();
}

/// Descriptive label for the reference dropdown: "above/below Stock Top" etc.
fn ref_label(reference: HeightReference, offset: f64) -> String {
    let dir = if offset >= 0.0 { "above" } else { "below" };
    format!("{dir} {}", reference.label())
}

/// Find the nearest reference point to an absolute Z value.
fn find_nearest_reference(z: f64, ctx: &HeightContext) -> HeightReference {
    let mut best = HeightReference::StockTop;
    let mut best_dist = f64::INFINITY;
    for &href in HeightReference::ALL {
        let ref_z = href.resolve_z(ctx);
        let dist = (z - ref_z).abs();
        if dist < best_dist {
            best_dist = dist;
            best = href;
        }
    }
    best
}

/// F1.19 / G-BOTTOMPIN: the sentence the Heights tab prints beside the Bottom
/// row when the operation does not read a pinned Bottom Z.
///
/// `None` means the pin DOES drive the cut, so the row is offered plain.
/// `Some(note)` names the dial that really sets the floor.
///
/// The core half measured the fact and declared it as
/// [`rs_cam_core::compute::catalog::OperationType::honors_pinned_bottom_z`]:
/// three of the twenty-four operations read `heights.bottom_z`, and on the
/// other twenty-one a pinned bottom reaches no emitted motion. This match
/// carries its own arm per operation because each family names a DIFFERENT
/// dial, and the sentry
/// (`crates/rs_cam_viz/tests/bottom_z_pin_note_g_bottompin.rs`) ties the two
/// enumerations together so they cannot drift.
///
/// The panel ANNOTATES rather than disables. A disabled row cannot be set
/// back to Auto, and a legacy project can carry a pin that puts the resolved
/// bottom above the top — which the Heights badge and the core
/// `geom.bottom_above_top_z` check both report. The operator must keep the
/// one control that clears it.
pub fn bottom_z_pin_note(op_type: OperationType) -> Option<&'static str> {
    use crate::state::toolpath::OperationType as Op;

    const DEPTH: &str = "Not used by this operation. Its Depth field sets the floor.";
    const MAX_DEPTH: &str = "Not used by this operation. Its Max Depth sets the floor.";
    const POCKET_DEPTH: &str = "Not used by this operation. Its Pocket Depth sets the floor.";
    const WIDTH: &str = "Not used by this operation. Its Chamfer Width sets the floor.";
    const PIN_DRILL: &str = "Not used by this operation. Its Depth and Spoilboard set the floor.";
    const CURVE: &str = "Not used by this operation. The projected surface sets the floor.";
    const SURFACE: &str = "Not used by this operation. The model surface sets the floor.";

    // One arm per operation. The `match` is exhaustive, so a new operation
    // cannot be added to the catalog without an answer here, and the sentry
    // ties every answer to `honors_pinned_bottom_z()`.
    match op_type {
        // The three that read `heights.bottom_z`. No note.
        Op::Adaptive3d | Op::UnifiedFinish | Op::Waterline => None,
        Op::Face => Some(DEPTH),
        Op::Pocket => Some(DEPTH),
        Op::Profile => Some(DEPTH),
        Op::Adaptive => Some(DEPTH),
        Op::Rest => Some(DEPTH),
        Op::Zigzag => Some(DEPTH),
        Op::Trace => Some(DEPTH),
        Op::Drill => Some(DEPTH),
        Op::VCarve => Some(MAX_DEPTH),
        Op::Inlay => Some(POCKET_DEPTH),
        Op::Chamfer => Some(WIDTH),
        Op::AlignmentPinDrill => Some(PIN_DRILL),
        Op::ProjectCurve => Some(CURVE),
        Op::DropCutter => Some(SURFACE),
        Op::Scallop => Some(SURFACE),
        Op::Pencil => Some(SURFACE),
        Op::HorizontalFinish => Some(SURFACE),
        Op::SteepShallow => Some(SURFACE),
        Op::RampFinish => Some(SURFACE),
        Op::SpiralFinish => Some(SURFACE),
        Op::RadialFinish => Some(SURFACE),
    }
}

/// The Bottom row tooltip for an operation that DOES read the pin.
const BOTTOM_Z_TOOLTIP: &str = "Deepest cut depth. The tool stops at this Z.";

pub(super) fn draw_heights_params(
    ui: &mut egui::Ui,
    heights: &mut HeightsConfig,
    ctx: &HeightContext,
    op_type: OperationType,
) {
    // What an `Auto` row shows is the value the CORE resolver produces for
    // THIS config — not a second set of defaults maintained here. The panel
    // used to carry its own (feed_z defaulted to `stock_top + 2` while the
    // resolver's is `retract - 2`), and because the panel then wrote its
    // defaults into the config on render, the divergence was a silent edit
    // rather than a visible disagreement. Rows other than `Auto` ignore this
    // value and display their own stored offset / absolute Z.
    let auto = heights.resolve(ctx);

    // F1.19 / G-BOTTOMPIN: the Bottom row's own tooltip used to say that
    // pinning the row overrides the operation's floor. That is true on three
    // operations and false on twenty-one. The note replaces the claim on the
    // twenty-one and names the dial that does set the floor.
    let pin_note = bottom_z_pin_note(op_type);
    let bottom_tooltip = match pin_note {
        Some(note) => format!("{note} The row still shows the stored value."),
        None => BOTTOM_Z_TOOLTIP.to_owned(),
    };

    // UI-02: 4 columns, so `param_grid` does not fit.
    egui::Grid::new("heights_p")
        .num_columns(4)
        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
        .show(ui, |ui| {
            draw_height_row(
                ui,
                "Clearance:",
                "Highest safe height. Rapid moves between separate operations travel at this Z.",
                &mut heights.clearance_z,
                HeightReference::StockTop,
                auto.clearance_z,
                ctx,
                "h_clear",
            );
            draw_height_row(
                ui,
                "Retract:",
                "Rapid travel height within an operation. Tool retracts here between cutting passes.",
                &mut heights.retract_z,
                HeightReference::StockTop,
                auto.retract_z,
                ctx,
                "h_retract",
            );
            draw_height_row(
                ui,
                "Feed:",
                "Approach height. Tool switches from rapid to feed rate here before plunging into material.",
                &mut heights.feed_z,
                HeightReference::StockTop,
                auto.feed_z,
                ctx,
                "h_feed",
            );
            draw_height_row(
                ui,
                "Top:",
                "Top of material. Cutting starts at this Z. Usually the stock top surface.",
                &mut heights.top_z,
                HeightReference::StockTop,
                auto.top_z,
                ctx,
                "h_top",
            );
            draw_height_row(
                ui,
                "Bottom:",
                &bottom_tooltip,
                &mut heights.bottom_z,
                HeightReference::StockTop,
                auto.bottom_z,
                ctx,
                "h_bottom",
            );
        });

    // F1.19 / G-BOTTOMPIN: the note is drawn, not hidden behind a hover. The
    // row above stays editable, so an operator can still read a stored pin
    // and set it back to auto.
    if let Some(note) = pin_note {
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(format!("Bottom: {note}"))
                .small()
                .italics()
                .color(crate::ui::tokens::TEXT_MUTED),
        );
    }
}

// ── Stepover Pattern Diagram ─────────────────────────────────────────────

// The thirteen diagram renderers moved to `shape_diagrams.rs` (P4 split).
// The banner above marks the end of the Heights panel section.

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use rs_cam_core::mesh::make_test_flat;
    use rs_cam_core::polygon::Polygon2;
    use rs_cam_core::session::ProjectSessionBuilder;

    use super::*;
    use crate::state::job::{ModelId, ModelKind, ModelUnits, ToolConfig, ToolId, ToolType};
    use crate::state::toolpath::{OperationConfig, OperationType, ToolpathEntry, ToolpathId};

    // ── G-HEIGHTSTAB: the Heights tab is a pure read ──────────────────
    //
    // These pin the two halves of the row contract: the display projection
    // cannot write (it takes `&HeightMode`), and the commit is gated on a
    // real user edit. See `height_row_display`'s doc comment for the defect.

    /// Stock 0..25, and an operation with NO depth semantics — the whole 3D
    /// family (`DepthSemantics::None` → `default_depth_for_heights() == 0`).
    /// This is the exact shape that took the adaptive3d rough to zero moves.
    fn zero_depth_3d_ctx() -> HeightContext {
        HeightContext {
            safe_z: 30.0,
            op_depth: 0.0,
            stock_top_z: 25.0,
            stock_bottom_z: 0.0,
            model_top_z: Some(24.0),
            model_bottom_z: Some(2.0),
        }
    }

    #[test]
    fn heights_display_leaves_auto_rows_unpinned_g_heightstab() {
        let heights = HeightsConfig::default();
        let ctx = zero_depth_3d_ctx();
        let auto = heights.resolve(&ctx);

        // Pre-condition: an all-Auto config leaves the floor to the operation.
        assert!(!auto.bottom_pinned);
        assert!(!auto.top_pinned);
        // …and on a zero-depth op the resolver's Auto bottom sits AT the stock
        // top, which is exactly why committing it clipped the op to nothing.
        assert!((auto.bottom_z - ctx.stock_top_z).abs() < 1e-9);

        // Rendering projects each Auto row for display only.
        for (mode, default_z) in [
            (&heights.clearance_z, auto.clearance_z),
            (&heights.retract_z, auto.retract_z),
            (&heights.feed_z, auto.feed_z),
            (&heights.top_z, auto.top_z),
            (&heights.bottom_z, auto.bottom_z),
        ] {
            let display = height_row_display(mode, HeightReference::StockTop, default_z, &ctx);
            assert_eq!(display.reference, HeightReference::StockTop);
            // The displayed "= z" hint must equal what the core resolver
            // produces — the panel must not carry its own defaults.
            let shown = display.reference.resolve_z(&ctx) + display.offset;
            assert!(
                (shown - default_z).abs() < 1e-9,
                "displayed {shown} != resolved {default_z}"
            );
        }

        // The stored config is untouched, so nothing goes stale and the
        // operation keeps deciding its own floor.
        assert!(heights.clearance_z.is_auto());
        assert!(heights.retract_z.is_auto());
        assert!(heights.feed_z.is_auto());
        assert!(heights.top_z.is_auto());
        assert!(heights.bottom_z.is_auto());
        assert!(!heights.resolve(&ctx).bottom_pinned);
    }

    #[test]
    fn height_row_commits_only_on_a_user_edit_g_heightstab() {
        let ctx = zero_depth_3d_ctx();
        let mut heights = HeightsConfig::default();
        let auto = heights.resolve(&ctx);
        let display = height_row_display(
            &heights.bottom_z,
            HeightReference::StockTop,
            auto.bottom_z,
            &ctx,
        );

        // Viewing the tab: no widget moved.
        commit_height_row(&mut heights.bottom_z, display, false);
        assert!(heights.bottom_z.is_auto());
        assert!(!heights.resolve(&ctx).bottom_pinned);

        // The user drags the row: now it pins, and only now.
        let edited = ReferenceOffset {
            reference: HeightReference::StockTop,
            offset: -6.0,
        };
        commit_height_row(&mut heights.bottom_z, edited, true);
        assert!(matches!(
            heights.bottom_z,
            HeightMode::FromReference(r) if (r.offset + 6.0).abs() < 1e-9
        ));
        let resolved = heights.resolve(&ctx);
        assert!(resolved.bottom_pinned);
        assert!((resolved.bottom_z - 19.0).abs() < 1e-9);
    }

    #[test]
    fn manual_height_rows_display_against_the_nearest_reference() {
        let ctx = zero_depth_3d_ctx();
        // 2.4 is nearest ModelBottom (2.0), not StockBottom (0.0).
        let mode = HeightMode::Manual(2.4);
        let display = height_row_display(&mode, HeightReference::StockTop, 0.0, &ctx);
        assert_eq!(display.reference, HeightReference::ModelBottom);
        assert!((display.offset - 0.4).abs() < 1e-9);
        // Still Manual — projecting it for display does not rewrite it.
        assert!(matches!(mode, HeightMode::Manual(v) if (v - 2.4).abs() < 1e-9));
    }

    fn session_polygon_model(id: usize) -> rs_cam_core::session::LoadedModel {
        rs_cam_core::session::LoadedModel {
            id,
            path: PathBuf::from("demo.svg"),
            name: "2D".to_owned(),
            kind: Some(ModelKind::Svg),
            mesh: None,
            polygons: Some(Arc::new(vec![Polygon2::rectangle(
                -10.0, -10.0, 10.0, 10.0,
            )])),
            drill_targets: std::sync::Arc::new(Vec::new()),
            layers: std::sync::Arc::new(Vec::new()),
            enriched_mesh: None,
            units: Some(ModelUnits::Millimeters),
            winding_report: None,
            load_error: None,
        }
    }

    fn session_mesh_model(id: usize) -> rs_cam_core::session::LoadedModel {
        rs_cam_core::session::LoadedModel {
            id,
            path: PathBuf::from("demo.stl"),
            name: "3D".to_owned(),
            kind: Some(ModelKind::Stl),
            mesh: Some(Arc::new(make_test_flat(20.0))),
            polygons: None,
            drill_targets: std::sync::Arc::new(Vec::new()),
            layers: std::sync::Arc::new(Vec::new()),
            enriched_mesh: None,
            units: Some(ModelUnits::Millimeters),
            winding_report: None,
            load_error: None,
        }
    }

    fn sample_tool(id: ToolId, tool_type: ToolType, diameter: f64) -> ToolConfig {
        let mut tool = ToolConfig::new_default(id, tool_type);
        tool.diameter = diameter;
        tool
    }

    fn make_session_toolpath_config(
        name: &str,
        tool_id: usize,
        model_id: usize,
        op: OperationConfig,
    ) -> rs_cam_core::session::ToolpathConfig {
        rs_cam_core::session::ToolpathConfig {
            id: rs_cam_core::ToolpathId(0), // assigned by session.add_toolpath
            name: name.to_owned(),
            enabled: true,
            operation: op,
            dressups: Default::default(),
            heights: Default::default(),
            tool_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: Default::default(),
            boundary_inherit: true,
            stock_source: Default::default(),
            coolant: Default::default(),
            face_selection: None,
            debug_options: Default::default(),
            feeds_provenance: Default::default(),
            rest_analysis: Default::default(),
            planner_origin: None,
        }
    }

    #[test]
    fn validate_toolpath_rejects_wrong_geometry_type() {
        // The builder keeps the ids this fixture chose. `add_tool` and
        // `add_model` overwrite them.
        let session = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 6.0))
            .model(session_mesh_model(2))
            .build();

        let entry = ToolpathEntry::for_operation(
            ToolpathId(3),
            "Pocket".to_owned(),
            ToolId(1),
            ModelId(2),
            OperationType::Pocket,
        );

        let errs = validate_toolpath(&entry, &ToolpathValidationContext::from_session(&session));
        assert!(
            errs.iter().any(|err| err.contains("2D geometry")),
            "expected 2D geometry validation error, got {errs:?}"
        );
    }

    /// A drawing with one circle-centre target — what a DXF with a `CIRCLE`
    /// entity imports to.
    fn session_target_model(id: usize) -> rs_cam_core::session::LoadedModel {
        let mut model = session_polygon_model(id);
        model.drill_targets = Arc::new(vec![rs_cam_core::io::dxf_input::DrillTarget {
            x: 0.0,
            y: 0.0,
            layer: "holes".to_owned(),
            kind: rs_cam_core::io::dxf_input::DrillTargetKind::CircleCenter { diameter: 6.0 },
        }]);
        model
    }

    fn drill_entry(model: usize) -> ToolpathEntry {
        ToolpathEntry::for_operation(
            ToolpathId(3),
            "Drill".to_owned(),
            ToolId(1),
            ModelId(model),
            OperationType::Drill,
        )
    }

    /// G-DRILLCENTROID (UX-R03-004): a Drill op on a drawing with closed
    /// shapes but no circles or points is blocked with the generator's own
    /// sentence. Pre-fix the validator was silent and Generate drilled the
    /// polygon centroid.
    #[test]
    fn validate_drill_blocks_when_model_exposes_no_targets() {
        let session = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 3.0))
            .model(session_polygon_model(2))
            .build();

        let errs = validate_toolpath(
            &drill_entry(2),
            &ToolpathValidationContext::from_session(&session),
        );
        assert!(
            errs.iter()
                .any(|e| e == rs_cam_core::compute::execute::NO_DRILL_TARGETS_MSG),
            "expected {:?}, got {errs:?}",
            rs_cam_core::compute::execute::NO_DRILL_TARGETS_MSG
        );

        // The session-config door prints the same sentence.
        let tc =
            make_session_toolpath_config("Drill", 1, 2, OperationConfig::Drill(Default::default()));
        let errs =
            validate_toolpath_config(&tc, &ToolpathValidationContext::from_session(&session));
        assert!(
            errs.iter()
                .any(|e| e == rs_cam_core::compute::execute::NO_DRILL_TARGETS_MSG),
            "config door: got {errs:?}"
        );
    }

    #[test]
    fn validate_drill_passes_when_model_exposes_a_target() {
        let session = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 3.0))
            .model(session_target_model(2))
            .build();

        let errs = validate_toolpath(
            &drill_entry(2),
            &ToolpathValidationContext::from_session(&session),
        );
        assert!(
            !errs.iter().any(|e| e.contains("drill targets")),
            "a target-bearing model must not be blocked: {errs:?}"
        );
    }

    #[test]
    fn validate_drill_passes_on_an_explicit_pick_without_model_targets() {
        let session = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 3.0))
            .model(session_polygon_model(2))
            .build();

        let mut entry = drill_entry(2);
        if let OperationConfig::Drill(cfg) = &mut entry.operation {
            cfg.selected_holes = Some(vec![[1.0, 2.0]]);
        }
        let errs = validate_toolpath(&entry, &ToolpathValidationContext::from_session(&session));
        assert!(
            !errs.iter().any(|e| e.contains("drill targets")),
            "a pick is a hole source: {errs:?}"
        );
    }

    #[test]
    fn validate_rest_requires_earlier_matching_operation() {
        let mut builder = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 10.0))
            .tool(sample_tool(ToolId(2), ToolType::EndMill, 6.0))
            .model(session_polygon_model(4));

        // Add the rest toolpath config to the session (no prior roughing)
        let mut rest_op = OperationConfig::Rest(Default::default());
        if let OperationConfig::Rest(cfg) = &mut rest_op {
            cfg.prev_tool_id = Some(ToolId(1));
        }
        let rest_config = make_session_toolpath_config("Rest", 2, 4, rest_op);
        let rest_idx = builder
            .add_toolpath(0, rest_config)
            .expect("add_toolpath reports the new toolpath index");
        let session = builder.build();

        // Build entry with the session-assigned ID
        // SAFETY: rest_idx bounded by add_toolpath return
        #[allow(clippy::indexing_slicing)]
        let rest_id = session.toolpath_configs()[rest_idx].id;
        let mut rest = ToolpathEntry::for_operation(
            rest_id,
            "Rest".to_owned(),
            ToolId(2),
            ModelId(4),
            OperationType::Rest,
        );
        if let OperationConfig::Rest(cfg) = &mut rest.operation {
            cfg.prev_tool_id = Some(ToolId(1));
        }

        let errs = validate_toolpath(&rest, &ToolpathValidationContext::from_session(&session));
        assert!(
            errs.iter()
                .any(|err| err.contains("earlier enabled operation")),
            "expected earlier-operation validation error, got {errs:?}"
        );
    }

    #[test]
    fn validate_rest_accepts_earlier_matching_operation() {
        let mut builder = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 10.0))
            .tool(sample_tool(ToolId(2), ToolType::EndMill, 6.0))
            .model(session_polygon_model(4));

        // Add roughing toolpath first
        let roughing_config = make_session_toolpath_config(
            "Pocket",
            1,
            4,
            OperationConfig::Pocket(Default::default()),
        );
        let _ = builder.add_toolpath(0, roughing_config).unwrap();

        // Add rest toolpath — session assigns the ID
        let mut rest_op = OperationConfig::Rest(Default::default());
        if let OperationConfig::Rest(cfg) = &mut rest_op {
            cfg.prev_tool_id = Some(ToolId(1));
        }
        let rest_config = make_session_toolpath_config("Rest", 2, 4, rest_op);
        let rest_idx = builder
            .add_toolpath(0, rest_config)
            .expect("add_toolpath reports the new toolpath index");
        let session = builder.build();

        // Build entry with the session-assigned ID so validation can locate it
        // SAFETY: rest_idx bounded by add_toolpath return
        #[allow(clippy::indexing_slicing)]
        let rest_id = session.toolpath_configs()[rest_idx].id;
        let mut rest = ToolpathEntry::for_operation(
            rest_id,
            "Rest".to_owned(),
            ToolId(2),
            ModelId(4),
            OperationType::Rest,
        );
        if let OperationConfig::Rest(cfg) = &mut rest.operation {
            cfg.prev_tool_id = Some(ToolId(1));
        }

        let errs = validate_toolpath(&rest, &ToolpathValidationContext::from_session(&session));
        assert!(
            !errs
                .iter()
                .any(|err| err.contains("earlier enabled operation")),
            "did not expect rest-ordering error, got {errs:?}"
        );
    }
}
