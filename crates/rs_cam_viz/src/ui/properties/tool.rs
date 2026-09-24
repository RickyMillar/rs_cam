use crate::state::job::{BitCutDirection, ToolConfig, ToolMaterial, ToolType};
use crate::ui::components::UiExt as _;
use crate::ui::components::ValueRow;
use crate::ui::theme;
use rs_cam_core::compute::tool_config::SizeUnits;

/// The "Size in" choices, in the order the selector shows them.
const SIZE_UNIT_CHOICES: &[(SizeUnits, &str)] = &[
    (SizeUnits::Metric, SizeUnits::Metric.label()),
    (SizeUnits::Imperial, SizeUnits::Imperial.label()),
];

/// TOO-003 — what the operator asked the tool editor to do this frame.
/// The properties panel edits a draft clone; the caller commits or
/// discards based on this.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToolEditAction {
    /// No commit action this frame (the draft may still have been edited).
    None,
    /// Commit the draft to the session.
    Apply,
    /// Discard the draft and restore the committed tool.
    Revert,
}

/// Draw the tool editor for `draft` (a clone of the committed tool).
/// `modified` is whether the draft differs from the committed tool; when
/// true a "● modified — Apply / Revert" affordance is shown so the
/// pending-until-committed state is legible (matching the Tool Library
/// modal's draft-then-Save model — TOO-003). Returns the operator action.
pub fn draw(
    ui: &mut egui::Ui,
    tool: &mut ToolConfig,
    modified: bool,
    panels: &mut crate::state::panels::PanelDrafts,
) -> ToolEditAction {
    ui.heading(&tool.name);
    ui.label(egui::RichText::new(tool.summary()).color(crate::ui::tokens::TEXT_FAINT));
    ui.separator();

    draw_tool_fields(ui, tool);

    // TOO-003 — commit affordance. Edits are pending until Apply (or until
    // the user navigates away, which auto-commits); Revert discards them.
    ui.add_space(8.0);
    ui.separator();
    let mut action = ToolEditAction::None;
    ui.horizontal(|ui| {
        if modified {
            ui.label(
                egui::RichText::new("\u{25CF} modified")
                    .small()
                    .color(theme::WARNING),
            );
            if ui
                .button("Apply")
                .on_hover_text("Commit these edits to the project tool.")
                .clicked()
            {
                action = ToolEditAction::Apply;
            }
            if ui
                .button("Revert")
                .on_hover_text("Discard these edits and restore the saved values.")
                .clicked()
            {
                action = ToolEditAction::Revert;
            }
        } else {
            ui.label(
                egui::RichText::new("\u{2713} saved")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
        }
    });

    ui.add_space(8.0);
    ui.separator();
    // Save this tool into a reusable library catalog. Importing it later
    // (Add Tool ▸ From library) copies a fresh snapshot into a project.
    // TOO-004: add-or-replace by geometry signature, so re-saving an edited
    // tool overwrites its catalog entry instead of silently piling up
    // duplicates that only the modal's Dedupe button could clean.
    // UI-09: the catalog name and the status line are typed state on
    // `AppState`, not egui temporary memory.
    let mut save_clicked = false;
    let mut trimmed = String::new();
    ui.horizontal(|ui| {
        ui.label("Save to library:");
        ui.add(
            egui::TextEdit::singleline(&mut panels.tool_catalog_name)
                .desired_width(120.0)
                .hint_text("catalog e.g. endmills"),
        );
        trimmed = panels.tool_catalog_name.trim().to_owned();
        save_clicked = ui
            .add_enabled(!trimmed.is_empty(), egui::Button::new("Save"))
            .on_hover_text("Add to the catalog, or overwrite the matching entry if one exists.")
            .clicked();
    });
    if save_clicked {
        match rs_cam_core::io::tool_library::add_or_replace_tool(&trimmed, tool.clone()) {
            Ok((path, replaced)) => {
                let verb = if replaced { "Updated" } else { "Saved" };
                panels.tool_catalog_status =
                    format!("{verb} '{}' in {}", tool.name, path.display());
            }
            Err(e) => {
                tracing::error!("tool library save failed: {e}");
                panels.tool_catalog_status = format!("Save failed: {e}");
            }
        }
    }
    if !panels.tool_catalog_status.is_empty() {
        ui.label(
            egui::RichText::new(&panels.tool_catalog_status)
                .small()
                .weak(),
        );
    }

    action
}

/// Draw the editable tool fields (name, type, params, preview, holder).
/// Shared by the properties panel and the Tool Library modal's edit form.
pub(crate) fn draw_tool_fields(ui: &mut egui::Ui, tool: &mut ToolConfig) {
    // Editable name
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut tool.name);
    });

    ui.add_space(4.0);

    // Tool type selector.
    //
    // Switching type IN PLACE has to fix up geometry, or it builds a tool
    // the cutter constructors reject. `ToolConfig::new_default` normalises
    // (cross-type geometry zeroed), so a tool switched EndMill -> VBit
    // arrives with `included_angle = 0.0` while `VBitEndmill::new` asserts
    // `0 < included_angle < 180` — a panic on the COMPUTE WORKER, reached
    // from a combo box. Tapered ball is worse: it asserts on both
    // `taper_half_angle` and `shaft_diameter >= diameter`.
    //
    // So on a change: drop the geometry the OLD type owned, then refill
    // whatever the NEW type requires from that type's own defaults. Both
    // halves live in core so this stays free of per-type knowledge.
    let previous_type = tool.tool_type;
    ui.horizontal(|ui| {
        ui.label("Type:");
        egui::ComboBox::from_id_salt("tool_type")
            .selected_text(tool.tool_type.label())
            .show_ui(ui, |ui| {
                for &tt in ToolType::ALL {
                    ui.selectable_value(&mut tool.tool_type, tt, tt.label());
                }
            });
    });
    if tool.tool_type != previous_type {
        tool.normalize_geometry();
        let defaults = ToolConfig::new_default(tool.id, tool.tool_type);
        for field in tool.missing_defining_geometry() {
            field.set_on(tool, field.value_of(&defaults));
        }
    }

    ui.add_space(8.0);

    // The name names a tip size the geometry does not have (for example
    // "2mm tip" on a Ø1.00 tool). Advisory: the name is free text.
    if let Some(caution) = tool.name_tip_size_mismatch().and_then(|m| m.caution_line()) {
        ui.label(
            egui::RichText::new(format!("{} {caution}", crate::ui::tokens::GLYPH_CAUTION))
                .color(crate::ui::tokens::CAUTION),
        );
        ui.add_space(4.0);
    }

    // The unit the size is shown and typed in. A display and entry
    // convention only: the draft keeps every length in mm, and the edit
    // reaches the session through the same Apply as every other field.
    // Until the operator picks one, the unit follows the name (a `1/4"`
    // name reads in inches).
    let mut units = tool.effective_size_units();
    if ui
        .add(
            crate::ui::components::ChoiceRow::new("Size in", &mut units, SIZE_UNIT_CHOICES)
                .hover("The unit the tool size is shown and typed in. Lengths stay in mm."),
        )
        .changed()
    {
        tool.size_units = Some(units);
    }

    // Parameters grid
    ui.param_grid("tool_params", |ui| {
        // One convention: `Ø` is a diameter, `R` a radius. A tapered
        // ball's `diameter` is the TIP, so its label says "Tip Ø", and a
        // ball tip shows its radius beside the field so the operator can
        // compare it with an "R1.0" vendor name. The field shows and
        // accepts the size in `units` (`1/4`, `1/4"`, `0.25in`, `6mm`).
        let diameter_label = tool.diameter_field_label();
        let tip_radius = tool.derived_tip_radius_text();
        ValueRow::new(diameter_label, &mut tool.diameter, " mm", 0.1, 0.1..=100.0)
            .length_units(units)
            .note(tip_radius.as_deref())
            .show(ui);

        ValueRow::new(
            "Cutting Length:",
            &mut tool.cutting_length,
            " mm",
            0.5,
            0.1..=200.0,
        )
        .show(ui);

        // Flute count (critical for feeds calculation).
        // UI-10: an integer field. `ValueRow` edits an `f64`, and a
        // temporary f64 would change what a partial edit commits.
        ui.label("Flutes:");
        let mut flutes_i = tool.flute_count as i32;
        if ui
            .add(egui::DragValue::new(&mut flutes_i).range(1..=8))
            .changed()
        {
            tool.flute_count = flutes_i.max(1) as u32;
        }
        ui.end_row();

        ValueRow::new("Helix:", &mut tool.helix_deg, " deg", 1.0, 0.0..=60.0).show(ui);

        if matches!(tool.tool_type, ToolType::EndMill) {
            ValueRow::new(
                "Corner Radius:",
                &mut tool.corner_radius_mm,
                " mm",
                0.01,
                0.0..=tool.diameter / 2.0,
            )
            .show(ui);
        }

        // Tool material
        ui.label("Material:");
        egui::ComboBox::from_id_salt("tool_material")
            .selected_text(tool.tool_material.label())
            .show_ui(ui, |ui| {
                for &tm in ToolMaterial::ALL {
                    ui.selectable_value(&mut tool.tool_material, tm, tm.label());
                }
            });
        ui.end_row();

        // Cut direction
        ui.label("Cut Dir:");
        egui::ComboBox::from_id_salt("cut_direction")
            .selected_text(tool.cut_direction.label())
            .show_ui(ui, |ui| {
                for &cd in BitCutDirection::ALL {
                    ui.selectable_value(&mut tool.cut_direction, cd, cd.label());
                }
            });
        ui.end_row();

        // Type-specific parameters
        match tool.tool_type {
            ToolType::BullNose => {
                ValueRow::new(
                    "Corner Radius:",
                    &mut tool.corner_radius,
                    " mm",
                    0.05,
                    0.01..=tool.diameter / 2.0,
                )
                .show(ui);
            }
            ToolType::VBit => {
                ValueRow::new(
                    "Included Angle:",
                    &mut tool.included_angle,
                    " deg",
                    1.0,
                    1.0..=179.0,
                )
                .show(ui);
            }
            ToolType::TaperedBallNose => {
                ValueRow::new(
                    "Taper Half-Angle:",
                    &mut tool.taper_half_angle,
                    " deg",
                    0.5,
                    0.5..=89.0,
                )
                .show(ui);
                // TOO-005: the tapered "shaft diameter" (taper top) used to
                // sit here next to the holder "shank diameter", two near-
                // identical names for different geometry. It now lives in its
                // own "Cutter geometry" group below, distinct from Holder/Shank.
            }
            _ => {}
        }
    });

    // TOO-005 — "Cutter geometry" group: the tapered ball-nose upper-shaft
    // diameter (taper top), separated from the holder "Shank Ø (in collet)"
    // so the two diameters live under distinct headers, not as sibling rows.
    if matches!(tool.tool_type, ToolType::TaperedBallNose) {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Cutter geometry").strong());
        ui.param_grid("tool_cutter_geometry", |ui| {
            ValueRow::new(
                "Upper shaft \u{00D8} (taper top):",
                &mut tool.shaft_diameter,
                " mm",
                0.1,
                tool.diameter..=100.0,
            )
            .show(ui);
        });
    }

    // Cross-section preview
    ui.add_space(12.0);
    ui.label("Cross-Section Preview:");
    draw_tool_preview(ui, tool);

    // Holder section (collapsible). TOO-006: the "collision check skipped"
    // safety state is promoted onto the header itself — a warning glyph +
    // phrase in the WARNING colour, visible *without* expanding the section
    // (the old free-floating italic prose below the collapsed section was
    // effectively invisible). UI wins, not prose.
    ui.add_space(12.0);
    let no_holder = tool.holder_diameter < 0.01;
    let (header_text, header_color) = if no_holder {
        (
            "\u{26A0} Holder / Shank — no holder, collision check skipped".to_owned(),
            theme::WARNING,
        )
    } else {
        ("Holder / Shank".to_owned(), theme::TEXT_HEADING)
    };
    egui::CollapsingHeader::new(egui::RichText::new(header_text).color(header_color))
        .id_salt("tool_holder_shank")
        .show(ui, |ui| {
            ui.param_grid("holder_params", |ui| {
                ValueRow::new(
                    "Holder Diameter:",
                    &mut tool.holder_diameter,
                    " mm",
                    0.5,
                    0.0..=200.0,
                )
                .show(ui);

                // TOO-005: role-bearing label, distinct from the tapered
                // "Upper shaft Ø" in the Cutter geometry group above.
                ValueRow::new(
                    "Shank \u{00D8} (in collet):",
                    &mut tool.shank_diameter,
                    " mm",
                    0.1,
                    0.0..=100.0,
                )
                .show(ui);

                ValueRow::new(
                    "Shank Length:",
                    &mut tool.shank_length,
                    " mm",
                    0.5,
                    0.0..=200.0,
                )
                .show(ui);

                ValueRow::new("Stickout:", &mut tool.stickout, " mm", 0.5, 0.0..=300.0).show(ui);
            });
        });

    // TOO-004 — Catalog metadata: vendor / product-id are now editable (they
    // previously rendered read-only in the library modal and were absent from
    // this shared editor), so a tool can carry correctable source provenance.
    ui.add_space(12.0);
    ui.collapsing("Catalog metadata", |ui| {
        ui.param_grid("tool_catalog_metadata", |ui| {
            ui.label("Vendor:");
            ui.text_edit_singleline(&mut tool.vendor);
            ui.end_row();
            ui.label("Product ID:");
            ui.text_edit_singleline(&mut tool.product_id);
            ui.end_row();
        });
    });
}

/// Draw a 2D cross-section preview of the full tool assembly.
///
/// Uses `profile_points()` from the `MillingCutter` trait so the preview
/// automatically matches the actual cutting geometry for any tool type.
pub(crate) fn draw_tool_preview(ui: &mut egui::Ui, tool: &ToolConfig) {
    use rs_cam_core::tool::MillingCutter;

    let desired_size = egui::vec2(ui.available_width().min(240.0), 180.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    let cx = rect.center().x;
    let bottom = rect.bottom() - 10.0;
    let draw_h = desired_size.y - 20.0; // vertical pixels available

    let tool_def = crate::compute::worker::helpers::build_cutter(tool);
    let cutter_r = tool_def.radius() as f32;
    let cutting_len = tool_def.length() as f32;
    let shank_r = (tool_def.shank_diameter / 2.0) as f32;
    let shank_len = tool_def.shank_length as f32;
    let holder_r = (tool_def.holder_diameter / 2.0) as f32;
    let holder_len = tool_def.holder_length() as f32;

    // Total height to display; clamp so preview isn't squished
    let total_h = (cutting_len + shank_len + holder_len).max(cutting_len * 1.2);
    let max_r = cutter_r.max(shank_r).max(holder_r).max(0.1);

    // Scale: fit both width and height with padding
    let scale_x = (desired_size.x * 0.4) / max_r;
    let scale_y = draw_h / total_h.max(0.1);
    let scale = scale_x.min(scale_y);

    // The cutter is the subject of the drawing; the shank and the holder are
    // the geometry around it. Three greys became two tokens: the palette
    // carries one tool tone and one construction tone, so the shank joins the
    // holder rather than inventing a rung between them.
    let cutter_stroke = egui::Stroke::new(1.5_f32, crate::ui::tokens::DIAGRAM_TOOL);
    let shank_stroke = egui::Stroke::new(1.0_f32, crate::ui::tokens::DIAGRAM_DIM);
    let holder_stroke = egui::Stroke::new(1.0_f32, crate::ui::tokens::DIAGRAM_DIM);

    // --- Cutter cross-section from profile_points ---
    let profile = tool_def.profile_points(32);
    let n = profile.len();

    // Right side: trace profile from tip (bottom) upward, then vertical to cutting_length
    let mut right_pts: Vec<egui::Pos2> = Vec::with_capacity(n + 2);
    for &(r, h) in &profile {
        right_pts.push(egui::pos2(
            cx + (r as f32) * scale,
            bottom - (h as f32) * scale,
        ));
    }
    // Extend vertically from the last profile point to cutting_length
    if let Some(&last) = right_pts.last() {
        let top_y = bottom - cutting_len * scale;
        if last.y > top_y + 0.5 {
            right_pts.push(egui::pos2(cx + cutter_r * scale, top_y));
        }
    }

    // Build closed outline: right side down (reversed) → left side up
    let mut outline: Vec<egui::Pos2> = Vec::with_capacity(n * 2 + 4);
    // Right side (top to tip)
    for pt in right_pts.iter().rev() {
        outline.push(*pt);
    }
    // Left side (tip to top) — mirror X
    for pt in &right_pts {
        outline.push(egui::pos2(2.0 * cx - pt.x, pt.y));
    }
    // Close
    if let Some(&first) = outline.first() {
        outline.push(first);
    }
    painter.add(egui::Shape::line(outline, cutter_stroke));

    // --- Shank rectangle ---
    let cutter_top_y = bottom - cutting_len * scale;
    if shank_len > 0.01 && shank_r > 0.01 {
        let shank_top_y = cutter_top_y - shank_len * scale;
        let sr = shank_r * scale;
        painter.add(egui::Shape::line(
            vec![
                egui::pos2(cx - sr, cutter_top_y),
                egui::pos2(cx + sr, cutter_top_y),
                egui::pos2(cx + sr, shank_top_y),
                egui::pos2(cx - sr, shank_top_y),
                egui::pos2(cx - sr, cutter_top_y),
            ],
            shank_stroke,
        ));
    }

    // --- Holder rectangle ---
    let shank_top_y = cutter_top_y - shank_len * scale;
    if holder_len > 0.01 && holder_r > 0.01 {
        let holder_top_y = shank_top_y - holder_len * scale;
        let hr = holder_r * scale;
        painter.add(egui::Shape::line(
            vec![
                egui::pos2(cx - hr, shank_top_y),
                egui::pos2(cx + hr, shank_top_y),
                egui::pos2(cx + hr, holder_top_y),
                egui::pos2(cx - hr, holder_top_y),
                egui::pos2(cx - hr, shank_top_y),
            ],
            holder_stroke,
        ));
    }

    // Center line
    painter.line_segment(
        [egui::pos2(cx, rect.top() + 5.0), egui::pos2(cx, bottom)],
        egui::Stroke::new(
            0.5_f32,
            // A construction line, so DIAGRAM_DIM at its original alpha.
            egui::Color32::from_rgba_unmultiplied(
                crate::ui::tokens::DIAGRAM_DIM.r(),
                crate::ui::tokens::DIAGRAM_DIM.g(),
                crate::ui::tokens::DIAGRAM_DIM.b(),
                100,
            ),
        ),
    );
}
