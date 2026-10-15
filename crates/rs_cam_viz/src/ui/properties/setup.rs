use super::PanelEdit;
use crate::state::job::{FaceUp, ModelId, SetupId, ZRotation};
use crate::ui::AppEvent;
use crate::ui::components::{OneLine, SectionHeader};
use crate::ui_command::UiCommand;
use rs_cam_core::session::{Corner, SetupData, XYDatum, ZDatum};

/// The four stock rotations, in degrees, for the Z Rotation row.
const Z_ROTATIONS: &[(ZRotation, &str)] = &[
    (ZRotation::Deg0, "0\u{00B0}"),
    (ZRotation::Deg90, "90\u{00B0}"),
    (ZRotation::Deg180, "180\u{00B0}"),
    (ZRotation::Deg270, "270\u{00B0}"),
];

/// Draw the setup overview panel: the name, the orientation, the datum and
/// the model scope. The caller draws the obstacle list (K2) and the stock
/// changes (S4) after it.
///
/// `pin_count` is the number of alignment pins on the stock (pins are now
/// stock-level, not per-setup). `all_models` lists every loaded model for
/// the model-scoping checkboxes.
///
/// WP6: `setup_data` is a SCRATCH copy the caller owns, not the session's
/// own record. The caller compares the draft against the stored setup and
/// applies one command per field group that moved. The panel therefore
/// pushes no `FixtureChanged` event of its own.
pub fn draw(
    ui: &mut egui::Ui,
    setup_id: SetupId,
    setup_data: &mut SetupData,
    pin_count: usize,
    all_models: &[(ModelId, String)],
    setup_count: usize,
    events: &mut Vec<AppEvent>,
) -> PanelEdit {
    ui.heading("Setup Properties");
    ui.separator();

    let mut edit = PanelEdit::default();

    ui.horizontal(|ui| {
        ui.label("Name:");
        let mut name = setup_data.name.clone();
        if ui.text_edit_singleline(&mut name).changed() {
            events.push(AppEvent::RenameSetup(setup_id, name));
        }
    });

    ui.add_space(4.0);
    if setup_count == 1 {
        ui.add_enabled(false, egui::Button::new("Remove Setup"))
            .on_disabled_hover_text("The last remaining setup cannot be removed.");
    } else if ui.button("Remove Setup").clicked() {
        events.push(AppEvent::RequestRemoveSetup(setup_id));
    }

    SectionHeader::new("Orientation").show(ui);
    ui.horizontal(|ui| {
        ui.label("Face Up:");
        egui::ComboBox::from_id_salt("face_up")
            .truncate()
            .selected_text(setup_data.face_up.label())
            .show_ui(ui, |ui| {
                for &face in FaceUp::ALL {
                    if ui
                        .selectable_label(setup_data.face_up == face, face.label())
                        .clicked()
                    {
                        setup_data.face_up = face;
                        edit.commit();
                        events.push(AppEvent::Ui(UiCommand::PreviewOrientation(face)));
                    }
                }
            });
    });
    // IC1: the shared closed-choice row. The four buttons with "deg" words
    // painted past the 280-point panel.
    if ui
        .add(
            crate::ui::components::ChoiceRow::new(
                "Z Rotation",
                &mut setup_data.z_rotation,
                Z_ROTATIONS,
            )
            .hover("The rotation of the stock about the vertical axis for this setup."),
        )
        .changed()
    {
        edit.commit();
    }
    if setup_data.face_up != FaceUp::Top {
        // IC1: one line; the full instruction is on the hover.
        ui.add(
            OneLine::new(setup_data.face_up.flip_instruction()).color(crate::ui::tokens::CAUTION),
        );
        // Hint: suggest alignment pins when flipped setup has no pins configured
        if pin_count == 0 {
            ui.add_space(4.0);
            if ui
                .small_button("Add alignment pins for this flip")
                .clicked()
            {
                events.push(AppEvent::SetupTwoSided);
            }
        }
    }

    ui.add_space(8.0);

    SectionHeader::new("Datum / Alignment").show(ui);

    let xy_label = match &setup_data.datum.xy_method {
        XYDatum::CornerProbe(corner) => format!("Corner Probe ({})", corner.label()),
        XYDatum::CenterOfStock => "Center of Stock".into(),
        XYDatum::AlignmentPins => "Alignment Pins".into(),
        XYDatum::Manual => "Manual".into(),
    };
    ui.horizontal(|ui| {
        ui.label("XY Method:");
        egui::ComboBox::from_id_salt("xy_datum")
            .truncate()
            .selected_text(&xy_label)
            .show_ui(ui, |ui| {
                for &corner in Corner::ALL {
                    let label = format!("Corner Probe ({})", corner.label());
                    if ui
                        .selectable_label(
                            setup_data.datum.xy_method == XYDatum::CornerProbe(corner),
                            &label,
                        )
                        .clicked()
                    {
                        setup_data.datum.xy_method = XYDatum::CornerProbe(corner);
                        edit.commit();
                    }
                }
                if ui
                    .selectable_label(
                        setup_data.datum.xy_method == XYDatum::CenterOfStock,
                        "Center of Stock",
                    )
                    .clicked()
                {
                    setup_data.datum.xy_method = XYDatum::CenterOfStock;
                    edit.commit();
                }
                if ui
                    .selectable_label(
                        setup_data.datum.xy_method == XYDatum::AlignmentPins,
                        "Alignment Pins",
                    )
                    .clicked()
                {
                    setup_data.datum.xy_method = XYDatum::AlignmentPins;
                    edit.commit();
                }
                if ui
                    .selectable_label(setup_data.datum.xy_method == XYDatum::Manual, "Manual")
                    .clicked()
                {
                    setup_data.datum.xy_method = XYDatum::Manual;
                    edit.commit();
                }
            });
    });

    let z_label = setup_data.datum.z_method.label();
    ui.horizontal(|ui| {
        ui.label("Z Method:");
        egui::ComboBox::from_id_salt("z_datum")
            .truncate()
            .selected_text(&z_label)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(setup_data.datum.z_method == ZDatum::StockTop, "Stock Top")
                    .clicked()
                {
                    setup_data.datum.z_method = ZDatum::StockTop;
                    edit.commit();
                }
                if ui
                    .selectable_label(
                        setup_data.datum.z_method == ZDatum::MachineTable,
                        "Machine Table",
                    )
                    .clicked()
                {
                    setup_data.datum.z_method = ZDatum::MachineTable;
                    edit.commit();
                }
                if ui
                    .selectable_label(
                        matches!(setup_data.datum.z_method, ZDatum::FixedOffset(_)),
                        "Fixed Offset",
                    )
                    .clicked()
                {
                    setup_data.datum.z_method = ZDatum::FixedOffset(0.0);
                    edit.commit();
                }
                if ui
                    .selectable_label(setup_data.datum.z_method == ZDatum::Manual, "Manual")
                    .clicked()
                {
                    setup_data.datum.z_method = ZDatum::Manual;
                    edit.commit();
                }
            });
    });
    if let ZDatum::FixedOffset(ref mut z) = setup_data.datum.z_method {
        ui.horizontal(|ui| {
            ui.label("  Z Offset:");
            edit.drag(&ui.add(egui::DragValue::new(z).speed(0.5).suffix(" mm")));
        });
    }

    ui.horizontal(|ui| {
        ui.label("Notes:");
        edit.click(&ui.text_edit_singleline(&mut setup_data.datum.notes));
    });

    ui.add_space(4.0);

    // Alignment pins are now defined on the stock (shared across setups).
    if pin_count > 0 {
        // OK, because this line is the pass arm of a verdict: its sibling
        // below is the CAUTION that says no pins are defined. Both wore the
        // same green before.
        ui.label(
            egui::RichText::new(format!("{pin_count} alignment pin(s) on stock"))
                .small()
                .color(crate::ui::tokens::OK),
        );
    } else if setup_data.datum.xy_method == XYDatum::AlignmentPins {
        ui.add(
            OneLine::new("\u{26A0} No alignment pins")
                .color(crate::ui::tokens::CAUTION)
                .detail("The XY method reads alignment pins. Add them on the Stock panel."),
        );
    }

    // ── Models ──────────────────────────────────────────────────────
    ui.add_space(8.0);
    SectionHeader::new("Models").show(ui);
    if all_models.is_empty() {
        ui.label(
            egui::RichText::new("No models loaded")
                .italics()
                .color(crate::ui::tokens::TEXT_MUTED),
        );
    } else {
        if setup_data.model_ids.is_empty() {
            // INFO, not OK. This states what the selection IS; it is not a
            // verdict on it. It shared one green with the pin-count pass
            // above, which is the collision §2.6 rules out.
            ui.label(
                egui::RichText::new("All models (unconstrained)")
                    .small()
                    .color(crate::ui::tokens::INFO),
            );
        }
        for &(model_id, ref model_name) in all_models {
            let mut checked =
                setup_data.model_ids.is_empty() || setup_data.model_ids.contains(&model_id);
            if ui.checkbox(&mut checked, model_name.as_str()).changed() {
                if checked {
                    // When toggling on: if currently "all", start explicit list with this one.
                    if setup_data.model_ids.is_empty() {
                        // On check: if explicit list exists, add to it.
                        setup_data.model_ids.push(model_id);
                    } else if !setup_data.model_ids.contains(&model_id) {
                        setup_data.model_ids.push(model_id);
                    }
                    // If all models are now checked, revert to empty (= all).
                    if setup_data.model_ids.len() == all_models.len() {
                        setup_data.model_ids.clear();
                    }
                } else {
                    // When toggling off: if currently "all", materialise the full list first.
                    if setup_data.model_ids.is_empty() {
                        setup_data.model_ids = all_models.iter().map(|(id, _)| *id).collect();
                    }
                    setup_data.model_ids.retain(|id| *id != model_id);
                }
                // W9 / P-2: this toggle used to write a GUI overlay that
                // was never saved, so nothing marked the project dirty.
                // It now writes persisted project state, and an edit
                // that does not set the dirty flag is an edit the user
                // can lose by closing the window.
                edit.commit();
            }
        }
    }

    edit
}
