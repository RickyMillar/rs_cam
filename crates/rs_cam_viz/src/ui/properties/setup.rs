use super::PanelEdit;
use crate::state::job::{FaceUp, ModelId, SetupId, ZRotation};
use crate::state::selection::Selection;
use crate::ui::AppEvent;
use crate::ui_command::UiCommand;
use rs_cam_core::session::{Corner, Fixture, FixtureKind, KeepOutZone, SetupData, XYDatum, ZDatum};

fn fixture_kind_label(kind: FixtureKind) -> &'static str {
    match kind {
        FixtureKind::Clamp => "Clamp",
        FixtureKind::Vise => "Vise",
        FixtureKind::VacuumPod => "Vacuum Pod",
        FixtureKind::Custom => "Custom",
    }
}

const FIXTURE_KINDS: &[FixtureKind] = &[
    FixtureKind::Clamp,
    FixtureKind::Vise,
    FixtureKind::VacuumPod,
    FixtureKind::Custom,
];

/// Draw the setup overview panel (fixtures list, keep-out list, setup name).
///
/// `pin_count` is the number of alignment pins on the stock (pins are now
/// stock-level, not per-setup). `has_flip_axis` indicates whether a flip axis
/// is configured on the stock. `all_models` lists every loaded model for the
/// model-scoping checkboxes.
///
/// WP6: `setup_data` is a SCRATCH copy the caller owns, not the session's
/// own record. The caller compares the draft against the stored setup and
/// applies one command per field group that moved. The panel therefore
/// pushes no `FixtureChanged` event of its own.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut egui::Ui,
    setup_id: SetupId,
    setup_data: &mut SetupData,
    pin_count: usize,
    has_flip_axis: bool,
    all_models: &[(ModelId, String)],
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

    ui.label(
        egui::RichText::new("Orientation")
            .strong()
            .color(crate::ui::tokens::TEXT_STRONG),
    );
    ui.horizontal(|ui| {
        ui.label("Face Up:");
        egui::ComboBox::from_id_salt("face_up")
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
    ui.horizontal(|ui| {
        ui.label("Z Rotation:");
        for &rot in ZRotation::ALL {
            if ui
                .selectable_label(setup_data.z_rotation == rot, rot.label())
                .clicked()
            {
                setup_data.z_rotation = rot;
                edit.commit();
            }
        }
    });
    if setup_data.face_up != FaceUp::Top {
        ui.label(
            egui::RichText::new(setup_data.face_up.flip_instruction())
                .italics()
                .color(crate::ui::tokens::CAUTION),
        );
        // Hint: suggest alignment pins when flipped setup has no pins configured
        if pin_count == 0 && !has_flip_axis {
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

    ui.label(
        egui::RichText::new("Datum / Alignment")
            .strong()
            .color(crate::ui::tokens::TEXT_STRONG),
    );

    let xy_label = match &setup_data.datum.xy_method {
        XYDatum::CornerProbe(corner) => format!("Corner Probe ({})", corner.label()),
        XYDatum::CenterOfStock => "Center of Stock".into(),
        XYDatum::AlignmentPins => "Alignment Pins".into(),
        XYDatum::Manual => "Manual".into(),
    };
    ui.horizontal(|ui| {
        ui.label("XY Method:");
        egui::ComboBox::from_id_salt("xy_datum")
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
        ui.label(
            egui::RichText::new("No pins defined — add them in Stock properties")
                .small()
                .color(crate::ui::tokens::CAUTION),
        );
    }

    // ── Models ──────────────────────────────────────────────────────
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Models")
            .strong()
            .color(crate::ui::tokens::TEXT_STRONG),
    );
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

    // ── Fixtures ──────────────────────────────────────────────────
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Fixtures")
            .strong()
            .color(crate::ui::tokens::TEXT_STRONG),
    );

    if setup_data.fixtures.is_empty() {
        ui.label(
            egui::RichText::new("No fixtures")
                .italics()
                .color(crate::ui::tokens::TEXT_MUTED),
        );
    }
    for fixture in &setup_data.fixtures {
        let label = format!("{} [{}]", fixture.name, fixture_kind_label(fixture.kind));
        let resp = ui.selectable_label(false, &label);
        if resp.clicked() {
            events.push(AppEvent::Ui(UiCommand::Select(Selection::Fixture(
                setup_id, fixture.id,
            ))));
        }
        resp.context_menu(|ui| {
            if ui.button("Delete").clicked() {
                events.push(AppEvent::RemoveFixture(setup_id, fixture.id));
                ui.close();
            }
        });
    }
    if ui.small_button("+ Add Fixture").clicked() {
        events.push(AppEvent::AddFixture(setup_id));
    }

    ui.add_space(8.0);

    ui.label(
        egui::RichText::new("Keep-Out Zones")
            .strong()
            .color(crate::ui::tokens::TEXT_STRONG),
    );

    if setup_data.keep_out_zones.is_empty() {
        ui.label(
            egui::RichText::new("No keep-out zones")
                .italics()
                .color(crate::ui::tokens::TEXT_MUTED),
        );
    }
    for zone in &setup_data.keep_out_zones {
        let resp = ui.selectable_label(false, &zone.name);
        if resp.clicked() {
            events.push(AppEvent::Ui(UiCommand::Select(Selection::KeepOut(
                setup_id, zone.id,
            ))));
        }
        resp.context_menu(|ui| {
            if ui.button("Delete").clicked() {
                events.push(AppEvent::RemoveKeepOut(setup_id, zone.id));
                ui.close();
            }
        });
    }
    if ui.small_button("+ Add Keep-Out Zone").clicked() {
        events.push(AppEvent::AddKeepOut(setup_id));
    }

    edit
}

/// Draw fixture property editor over a SCRATCH copy (WP6).
///
/// The caller applies `Command::ReplaceFixture` when the returned
/// [`PanelEdit`] reports a finished edit.
pub fn draw_fixture_properties(
    ui: &mut egui::Ui,
    _setup_id: SetupId,
    fixture: &mut Fixture,
) -> PanelEdit {
    ui.heading("Fixture Properties");
    ui.separator();

    let mut edit = PanelEdit::default();

    ui.horizontal(|ui| {
        ui.label("Name:");
        edit.click(&ui.text_edit_singleline(&mut fixture.name));
    });

    ui.horizontal(|ui| {
        ui.label("Type:");
        egui::ComboBox::from_id_salt("fixture_kind")
            .selected_text(fixture_kind_label(fixture.kind))
            .show_ui(ui, |ui| {
                for &kind in FIXTURE_KINDS {
                    if ui
                        .selectable_label(fixture.kind == kind, fixture_kind_label(kind))
                        .clicked()
                    {
                        fixture.kind = kind;
                        edit.commit();
                    }
                }
            });
    });

    edit.click(&ui.checkbox(&mut fixture.enabled, "Enabled"));

    ui.add_space(4.0);

    ui.label(
        egui::RichText::new("Position (mm)")
            .strong()
            .color(crate::ui::tokens::TEXT_MUTED),
    );
    egui::Grid::new("fixture_position")
        .num_columns(2)
        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
        .min_row_height(crate::ui::tokens::ROW_DENSE)
        .show(ui, |ui| {
            ui.label("X:");
            edit.drag(
                &ui.add(
                    egui::DragValue::new(&mut fixture.origin_x)
                        .speed(0.5)
                        .suffix(" mm"),
                ),
            );
            ui.end_row();
            ui.label("Y:");
            edit.drag(
                &ui.add(
                    egui::DragValue::new(&mut fixture.origin_y)
                        .speed(0.5)
                        .suffix(" mm"),
                ),
            );
            ui.end_row();
            ui.label("Z:");
            edit.drag(
                &ui.add(
                    egui::DragValue::new(&mut fixture.origin_z)
                        .speed(0.5)
                        .suffix(" mm"),
                ),
            );
            ui.end_row();
        });

    ui.add_space(4.0);

    ui.label(
        egui::RichText::new("Size (mm)")
            .strong()
            .color(crate::ui::tokens::TEXT_MUTED),
    );
    egui::Grid::new("fixture_size")
        .num_columns(2)
        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
        .min_row_height(crate::ui::tokens::ROW_DENSE)
        .show(ui, |ui| {
            ui.label("X:");
            edit.drag(
                &ui.add(
                    egui::DragValue::new(&mut fixture.size_x)
                        .speed(0.5)
                        .range(0.1..=10000.0)
                        .suffix(" mm"),
                ),
            );
            ui.end_row();
            ui.label("Y:");
            edit.drag(
                &ui.add(
                    egui::DragValue::new(&mut fixture.size_y)
                        .speed(0.5)
                        .range(0.1..=10000.0)
                        .suffix(" mm"),
                ),
            );
            ui.end_row();
            ui.label("Z:");
            edit.drag(
                &ui.add(
                    egui::DragValue::new(&mut fixture.size_z)
                        .speed(0.5)
                        .range(0.1..=10000.0)
                        .suffix(" mm"),
                ),
            );
            ui.end_row();
        });

    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label("Clearance:");
        edit.drag(
            &ui.add(
                egui::DragValue::new(&mut fixture.clearance)
                    .speed(0.1)
                    .range(0.0..=100.0)
                    .suffix(" mm"),
            ),
        );
    });

    edit
}

/// Draw keep-out zone property editor over a SCRATCH copy (WP6).
///
/// The caller applies `Command::ReplaceKeepOut` when the returned
/// [`PanelEdit`] reports a finished edit.
pub fn draw_keep_out_properties(
    ui: &mut egui::Ui,
    _setup_id: SetupId,
    zone: &mut KeepOutZone,
) -> PanelEdit {
    ui.heading("Keep-Out Zone Properties");
    ui.separator();

    let mut edit = PanelEdit::default();

    ui.horizontal(|ui| {
        ui.label("Name:");
        edit.click(&ui.text_edit_singleline(&mut zone.name));
    });

    edit.click(&ui.checkbox(&mut zone.enabled, "Enabled"));

    ui.add_space(4.0);

    ui.label(
        egui::RichText::new("Position (mm)")
            .strong()
            .color(crate::ui::tokens::TEXT_MUTED),
    );
    egui::Grid::new("keepout_position")
        .num_columns(2)
        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
        .min_row_height(crate::ui::tokens::ROW_DENSE)
        .show(ui, |ui| {
            ui.label("X:");
            edit.drag(
                &ui.add(
                    egui::DragValue::new(&mut zone.origin_x)
                        .speed(0.5)
                        .suffix(" mm"),
                ),
            );
            ui.end_row();
            ui.label("Y:");
            edit.drag(
                &ui.add(
                    egui::DragValue::new(&mut zone.origin_y)
                        .speed(0.5)
                        .suffix(" mm"),
                ),
            );
            ui.end_row();
        });

    ui.add_space(4.0);

    ui.label(
        egui::RichText::new("Size (mm)")
            .strong()
            .color(crate::ui::tokens::TEXT_MUTED),
    );
    egui::Grid::new("keepout_size")
        .num_columns(2)
        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
        .min_row_height(crate::ui::tokens::ROW_DENSE)
        .show(ui, |ui| {
            ui.label("X:");
            edit.drag(
                &ui.add(
                    egui::DragValue::new(&mut zone.size_x)
                        .speed(0.5)
                        .range(0.1..=10000.0)
                        .suffix(" mm"),
                ),
            );
            ui.end_row();
            ui.label("Y:");
            edit.drag(
                &ui.add(
                    egui::DragValue::new(&mut zone.size_y)
                        .speed(0.5)
                        .range(0.1..=10000.0)
                        .suffix(" mm"),
                ),
            );
            ui.end_row();
        });

    edit
}
