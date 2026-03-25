use super::AppEvent;
use crate::state::AppState;
use crate::state::job::{FaceUp, Setup, XYDatum};
use crate::state::selection::Selection;
use crate::ui::theme;

/// Left panel for the Setup workspace: setup list with summary cards.
pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Setups");
    ui.separator();

    // Stock summary — show effective dims for the active setup orientation
    let stock = &state.job.stock;
    let (eff_w, eff_d, eff_h) = if let Some(setup) = active_setup(state) {
        setup.effective_stock(stock)
    } else {
        (stock.x, stock.y, stock.z)
    };
    theme::card_frame(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Stock")
                        .strong()
                        .color(theme::TEXT_HEADING),
                );
                ui.label(
                    egui::RichText::new(format!("{:.0} x {:.0} x {:.0} mm", eff_w, eff_d, eff_h))
                        .small()
                        .color(theme::TEXT_MUTED),
                );
            });
            let selected = state.selection == Selection::Stock;
            if ui
                .selectable_label(selected, "Edit stock dimensions")
                .clicked()
            {
                events.push(AppEvent::Select(Selection::Stock));
            }
        });

    ui.add_space(6.0);

    // Setup cards
    for setup in &state.job.setups {
        draw_setup_card(ui, setup, state, events);
        ui.add_space(4.0);
    }

    // Add setup button
    if state.job.setups.len() > 1 || !state.job.setups.is_empty() {
        ui.add_space(4.0);
        if ui.button("+ Add Setup").clicked() {
            events.push(AppEvent::AddSetup);
        }
    }

    ui.add_space(12.0);

    // Models section (compact)
    egui::CollapsingHeader::new("Models")
        .default_open(false)
        .show(ui, |ui| {
            if state.job.models.is_empty() {
                ui.label(
                    egui::RichText::new("No models imported")
                        .italics()
                        .color(theme::TEXT_DIM),
                );
            }
            for model in &state.job.models {
                let selected = state.selection == Selection::Model(model.id);
                let response = ui.selectable_label(selected, &model.name);
                if response.clicked() {
                    events.push(AppEvent::Select(Selection::Model(model.id)));
                }
                response.context_menu(|ui| {
                    if ui.button("Reload from disk").clicked() {
                        events.push(AppEvent::ReloadModel(model.id));
                        ui.close_menu();
                    }
                    if ui.button("Delete").clicked() {
                        events.push(AppEvent::RemoveModel(model.id));
                        ui.close_menu();
                    }
                });
            }
        });
}

fn draw_setup_card(ui: &mut egui::Ui, setup: &Setup, state: &AppState, events: &mut Vec<AppEvent>) {
    let is_selected = state.selection == Selection::Setup(setup.id);
    let base_border = if is_selected {
        theme::ACCENT
    } else {
        egui::Color32::from_rgb(55, 55, 65)
    };

    let card_response = egui::Frame::default()
        .fill(egui::Color32::from_rgb(38, 40, 50))
        .stroke(egui::Stroke::new(1.0, base_border))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            // Header: setup name + face label
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&setup.name)
                        .strong()
                        .color(theme::TEXT_STRONG),
                );

                if setup.face_up != FaceUp::Top {
                    ui.label(
                        egui::RichText::new(format!("[{}]", setup.face_up.label()))
                            .small()
                            .color(theme::WARNING),
                    );
                }
            });

            // Summary row: orientation + datum
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;

                // Orientation chip
                let orient = if setup.z_rotation == crate::state::job::ZRotation::Deg0 {
                    setup.face_up.label().to_string()
                } else {
                    format!("{} +{}", setup.face_up.label(), setup.z_rotation.label())
                };
                chip(
                    ui,
                    "Orient",
                    &orient,
                    egui::Color32::from_rgb(100, 140, 180),
                );

                // Datum chip
                let datum = match &setup.datum.xy_method {
                    XYDatum::CornerProbe(c) => format!("Corner ({})", c.label()),
                    XYDatum::CenterOfStock => "Center".into(),
                    XYDatum::AlignmentPins => "Pins".into(),
                    XYDatum::Manual => "Manual".into(),
                };
                chip(ui, "XY", &datum, egui::Color32::from_rgb(140, 160, 100));
            });

            // Counts row
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;

                let fixture_count = setup.fixtures.len();
                let keepout_count = setup.keep_out_zones.len();
                let pin_count = state.job.stock.alignment_pins.len();

                if fixture_count > 0 {
                    chip(
                        ui,
                        "Fix",
                        &fixture_count.to_string(),
                        egui::Color32::from_rgb(160, 130, 100),
                    );
                }
                if keepout_count > 0 {
                    chip(
                        ui,
                        "Keep Out",
                        &keepout_count.to_string(),
                        egui::Color32::from_rgb(180, 100, 100),
                    );
                }
                if pin_count > 0 {
                    chip(
                        ui,
                        "Pins",
                        &pin_count.to_string(),
                        egui::Color32::from_rgb(100, 160, 140),
                    );
                }
                if fixture_count == 0 && keepout_count == 0 && pin_count == 0 {
                    ui.label(
                        egui::RichText::new("No workholding")
                            .small()
                            .italics()
                            .color(theme::TEXT_DIM),
                    );
                }
            });

            // Flip instruction
            if setup.face_up != FaceUp::Top {
                ui.label(
                    egui::RichText::new(setup.face_up.flip_instruction())
                        .small()
                        .italics()
                        .color(theme::WARNING_MILD),
                );
            }

            // Fresh-stock warning for non-first setups
            if state.job.setups.first().map(|s| s.id) != Some(setup.id) {
                ui.label(
                    egui::RichText::new("Starts from uncut stock (prior setups not reflected)")
                        .small()
                        .italics()
                        .color(theme::WARNING_MILD),
                );
            }
        })
        .response
        .interact(egui::Sense::click());

    if card_response.clicked() {
        events.push(AppEvent::Select(Selection::Setup(setup.id)));
    }

    // Update border color on hover
    if card_response.hovered() && !is_selected {
        let hover_border = egui::Color32::from_rgb(80, 120, 170);
        ui.painter().rect_stroke(
            card_response.rect,
            4.0,
            egui::Stroke::new(1.0, hover_border),
        );
    }
}

/// A compact label chip: "Key: Value" in a tinted style with a tooltip.
fn chip(ui: &mut egui::Ui, key: &str, value: &str, color: egui::Color32) {
    let tooltip = match key {
        "Fix" => "Fixtures",
        "Keep Out" => "Keep-out zones",
        "Orient" => "Setup orientation",
        "XY" => "XY datum method",
        _ => key,
    };
    ui.label(
        egui::RichText::new(format!("{key}: {value}"))
            .small()
            .color(color),
    )
    .on_hover_text(tooltip);
}

/// Determine the active setup from the current selection.
fn active_setup(state: &AppState) -> Option<&Setup> {
    let setup_id = match &state.selection {
        Selection::Setup(id) => Some(*id),
        Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
        Selection::Toolpath(tp_id) => state.job.setup_of_toolpath(*tp_id),
        _ => None,
    };
    if let Some(sid) = setup_id {
        state.job.setups.iter().find(|s| s.id == sid)
    } else {
        state.job.setups.first()
    }
}
