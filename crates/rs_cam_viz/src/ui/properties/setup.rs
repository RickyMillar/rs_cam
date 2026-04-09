use crate::state::job::{FaceUp, ModelId, SetupId, ZRotation};
use crate::state::runtime::{Corner, SetupRuntime, XYDatum, ZDatum};
use crate::state::selection::Selection;
use crate::ui::AppEvent;
use rs_cam_core::session::{Fixture, FixtureKind, KeepOutZone, SetupData};

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
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut egui::Ui,
    setup_id: SetupId,
    setup_data: &mut SetupData,
    setup_rt: &mut SetupRuntime,
    pin_count: usize,
    has_flip_axis: bool,
    all_models: &[(ModelId, String)],
    events: &mut Vec<AppEvent>,
) {
    ui.heading("Setup Properties");
    ui.separator();

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
            .color(egui::Color32::from_rgb(180, 180, 195)),
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
                        events.push(AppEvent::FixtureChanged);
                        events.push(AppEvent::PreviewOrientation(face));
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
                events.push(AppEvent::FixtureChanged);
            }
        }
    });
    if setup_data.face_up != FaceUp::Top {
        ui.label(
            egui::RichText::new(setup_data.face_up.flip_instruction())
                .italics()
                .color(egui::Color32::from_rgb(220, 180, 60)),
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
            .color(egui::Color32::from_rgb(180, 180, 195)),
    );

    let xy_label = match &setup_rt.datum.xy_method {
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
                            setup_rt.datum.xy_method == XYDatum::CornerProbe(corner),
                            &label,
                        )
                        .clicked()
                    {
                        setup_rt.datum.xy_method = XYDatum::CornerProbe(corner);
                        events.push(AppEvent::FixtureChanged);
                    }
                }
                if ui
                    .selectable_label(
                        setup_rt.datum.xy_method == XYDatum::CenterOfStock,
                        "Center of Stock",
                    )
                    .clicked()
                {
                    setup_rt.datum.xy_method = XYDatum::CenterOfStock;
                    events.push(AppEvent::FixtureChanged);
                }
                if ui
                    .selectable_label(
                        setup_rt.datum.xy_method == XYDatum::AlignmentPins,
                        "Alignment Pins",
                    )
                    .clicked()
                {
                    setup_rt.datum.xy_method = XYDatum::AlignmentPins;
                    events.push(AppEvent::FixtureChanged);
                }
                if ui
                    .selectable_label(setup_rt.datum.xy_method == XYDatum::Manual, "Manual")
                    .clicked()
                {
                    setup_rt.datum.xy_method = XYDatum::Manual;
                    events.push(AppEvent::FixtureChanged);
                }
            });
    });

    let z_label = setup_rt.datum.z_method.label();
    ui.horizontal(|ui| {
        ui.label("Z Method:");
        egui::ComboBox::from_id_salt("z_datum")
            .selected_text(&z_label)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(setup_rt.datum.z_method == ZDatum::StockTop, "Stock Top")
                    .clicked()
                {
                    setup_rt.datum.z_method = ZDatum::StockTop;
                    events.push(AppEvent::FixtureChanged);
                }
                if ui
                    .selectable_label(
                        setup_rt.datum.z_method == ZDatum::MachineTable,
                        "Machine Table",
                    )
                    .clicked()
                {
                    setup_rt.datum.z_method = ZDatum::MachineTable;
                    events.push(AppEvent::FixtureChanged);
                }
                if ui
                    .selectable_label(
                        matches!(setup_rt.datum.z_method, ZDatum::FixedOffset(_)),
                        "Fixed Offset",
                    )
                    .clicked()
                {
                    setup_rt.datum.z_method = ZDatum::FixedOffset(0.0);
                    events.push(AppEvent::FixtureChanged);
                }
                if ui
                    .selectable_label(setup_rt.datum.z_method == ZDatum::Manual, "Manual")
                    .clicked()
                {
                    setup_rt.datum.z_method = ZDatum::Manual;
                    events.push(AppEvent::FixtureChanged);
                }
            });
    });
    if let ZDatum::FixedOffset(ref mut z) = setup_rt.datum.z_method {
        ui.horizontal(|ui| {
            ui.label("  Z Offset:");
            if ui
                .add(egui::DragValue::new(z).speed(0.5).suffix(" mm"))
                .changed()
            {
                events.push(AppEvent::FixtureChanged);
            }
        });
    }

    ui.horizontal(|ui| {
        ui.label("Notes:");
        if ui.text_edit_singleline(&mut setup_rt.datum.notes).changed() {
            events.push(AppEvent::FixtureChanged);
        }
    });

    ui.add_space(4.0);

    // Alignment pins are now defined on the stock (shared across setups).
    if pin_count > 0 {
        ui.label(
            egui::RichText::new(format!("{pin_count} alignment pin(s) on stock"))
                .small()
                .color(egui::Color32::from_rgb(140, 180, 140)),
        );
    } else if setup_rt.datum.xy_method == XYDatum::AlignmentPins {
        ui.label(
            egui::RichText::new("No pins defined — add them in Stock properties")
                .small()
                .color(egui::Color32::from_rgb(220, 180, 60)),
        );
    }

    // ── Models ──────────────────────────────────────────────────────
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Models")
            .strong()
            .color(egui::Color32::from_rgb(180, 180, 195)),
    );
    if all_models.is_empty() {
        ui.label(
            egui::RichText::new("No models loaded")
                .italics()
                .color(egui::Color32::from_rgb(120, 120, 130)),
        );
    } else {
        if setup_rt.model_ids.is_empty() {
            ui.label(
                egui::RichText::new("All models (unconstrained)")
                    .small()
                    .color(egui::Color32::from_rgb(140, 180, 140)),
            );
        }
        for &(model_id, ref model_name) in all_models {
            let mut checked =
                setup_rt.model_ids.is_empty() || setup_rt.model_ids.contains(&model_id);
            if ui.checkbox(&mut checked, model_name.as_str()).changed() {
                if checked {
                    // When toggling on: if currently "all", start explicit list with this one.
                    if setup_rt.model_ids.is_empty() {
                        // On check: if explicit list exists, add to it.
                        setup_rt.model_ids.push(model_id);
                    } else if !setup_rt.model_ids.contains(&model_id) {
                        setup_rt.model_ids.push(model_id);
                    }
                    // If all models are now checked, revert to empty (= all).
                    if setup_rt.model_ids.len() == all_models.len() {
                        setup_rt.model_ids.clear();
                    }
                } else {
                    // When toggling off: if currently "all", materialise the full list first.
                    if setup_rt.model_ids.is_empty() {
                        setup_rt.model_ids = all_models.iter().map(|(id, _)| *id).collect();
                    }
                    setup_rt.model_ids.retain(|id| *id != model_id);
                }
            }
        }
    }

    // ── Fixtures ──────────────────────────────────────────────────
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Fixtures")
            .strong()
            .color(egui::Color32::from_rgb(180, 180, 195)),
    );

    if setup_data.fixtures.is_empty() {
        ui.label(
            egui::RichText::new("No fixtures")
                .italics()
                .color(egui::Color32::from_rgb(120, 120, 130)),
        );
    }
    for fixture in &setup_data.fixtures {
        let label = format!("{} [{}]", fixture.name, fixture_kind_label(fixture.kind));
        let resp = ui.selectable_label(false, &label);
        if resp.clicked() {
            events.push(AppEvent::Select(Selection::Fixture(setup_id, fixture.id)));
        }
        resp.context_menu(|ui| {
            if ui.button("Delete").clicked() {
                events.push(AppEvent::RemoveFixture(setup_id, fixture.id));
                ui.close_menu();
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
            .color(egui::Color32::from_rgb(180, 180, 195)),
    );

    if setup_data.keep_out_zones.is_empty() {
        ui.label(
            egui::RichText::new("No keep-out zones")
                .italics()
                .color(egui::Color32::from_rgb(120, 120, 130)),
        );
    }
    for zone in &setup_data.keep_out_zones {
        let resp = ui.selectable_label(false, &zone.name);
        if resp.clicked() {
            events.push(AppEvent::Select(Selection::KeepOut(setup_id, zone.id)));
        }
        resp.context_menu(|ui| {
            if ui.button("Delete").clicked() {
                events.push(AppEvent::RemoveKeepOut(setup_id, zone.id));
                ui.close_menu();
            }
        });
    }
    if ui.small_button("+ Add Keep-Out Zone").clicked() {
        events.push(AppEvent::AddKeepOut(setup_id));
    }
}

/// Draw fixture property editor.
pub fn draw_fixture_properties(
    ui: &mut egui::Ui,
    _setup_id: SetupId,
    fixture: &mut Fixture,
    events: &mut Vec<AppEvent>,
) {
    ui.heading("Fixture Properties");
    ui.separator();

    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label("Name:");
        changed |= ui.text_edit_singleline(&mut fixture.name).changed();
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
                        changed = true;
                    }
                }
            });
    });

    changed |= ui.checkbox(&mut fixture.enabled, "Enabled").changed();

    ui.add_space(4.0);

    ui.label(
        egui::RichText::new("Position (mm)")
            .strong()
            .color(egui::Color32::from_rgb(160, 160, 175)),
    );
    egui::Grid::new("fixture_position")
        .num_columns(2)
        .spacing([8.0, 3.0])
        .show(ui, |ui| {
            ui.label("X:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut fixture.origin_x)
                        .speed(0.5)
                        .suffix(" mm"),
                )
                .changed();
            ui.end_row();
            ui.label("Y:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut fixture.origin_y)
                        .speed(0.5)
                        .suffix(" mm"),
                )
                .changed();
            ui.end_row();
            ui.label("Z:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut fixture.origin_z)
                        .speed(0.5)
                        .suffix(" mm"),
                )
                .changed();
            ui.end_row();
        });

    ui.add_space(4.0);

    ui.label(
        egui::RichText::new("Size (mm)")
            .strong()
            .color(egui::Color32::from_rgb(160, 160, 175)),
    );
    egui::Grid::new("fixture_size")
        .num_columns(2)
        .spacing([8.0, 3.0])
        .show(ui, |ui| {
            ui.label("X:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut fixture.size_x)
                        .speed(0.5)
                        .range(0.1..=10000.0)
                        .suffix(" mm"),
                )
                .changed();
            ui.end_row();
            ui.label("Y:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut fixture.size_y)
                        .speed(0.5)
                        .range(0.1..=10000.0)
                        .suffix(" mm"),
                )
                .changed();
            ui.end_row();
            ui.label("Z:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut fixture.size_z)
                        .speed(0.5)
                        .range(0.1..=10000.0)
                        .suffix(" mm"),
                )
                .changed();
            ui.end_row();
        });

    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label("Clearance:");
        changed |= ui
            .add(
                egui::DragValue::new(&mut fixture.clearance)
                    .speed(0.1)
                    .range(0.0..=100.0)
                    .suffix(" mm"),
            )
            .changed();
    });

    if changed {
        events.push(AppEvent::FixtureChanged);
    }
}

/// Draw keep-out zone property editor.
pub fn draw_keep_out_properties(
    ui: &mut egui::Ui,
    _setup_id: SetupId,
    zone: &mut KeepOutZone,
    events: &mut Vec<AppEvent>,
) {
    ui.heading("Keep-Out Zone Properties");
    ui.separator();

    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label("Name:");
        changed |= ui.text_edit_singleline(&mut zone.name).changed();
    });

    changed |= ui.checkbox(&mut zone.enabled, "Enabled").changed();

    ui.add_space(4.0);

    ui.label(
        egui::RichText::new("Position (mm)")
            .strong()
            .color(egui::Color32::from_rgb(160, 160, 175)),
    );
    egui::Grid::new("keepout_position")
        .num_columns(2)
        .spacing([8.0, 3.0])
        .show(ui, |ui| {
            ui.label("X:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut zone.origin_x)
                        .speed(0.5)
                        .suffix(" mm"),
                )
                .changed();
            ui.end_row();
            ui.label("Y:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut zone.origin_y)
                        .speed(0.5)
                        .suffix(" mm"),
                )
                .changed();
            ui.end_row();
        });

    ui.add_space(4.0);

    ui.label(
        egui::RichText::new("Size (mm)")
            .strong()
            .color(egui::Color32::from_rgb(160, 160, 175)),
    );
    egui::Grid::new("keepout_size")
        .num_columns(2)
        .spacing([8.0, 3.0])
        .show(ui, |ui| {
            ui.label("X:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut zone.size_x)
                        .speed(0.5)
                        .range(0.1..=10000.0)
                        .suffix(" mm"),
                )
                .changed();
            ui.end_row();
            ui.label("Y:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut zone.size_y)
                        .speed(0.5)
                        .range(0.1..=10000.0)
                        .suffix(" mm"),
                )
                .changed();
            ui.end_row();
        });

    if changed {
        events.push(AppEvent::FixtureChanged);
    }
}
