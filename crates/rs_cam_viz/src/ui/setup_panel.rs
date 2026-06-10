use super::AppEvent;
use crate::state::AppState;
use crate::state::job::{FaceUp, ModelId, SetupId};
use crate::state::runtime::XYDatum;
use crate::state::selection::Selection;
use crate::ui::theme;
use rs_cam_core::session::SetupData;

/// Left panel for the Setup workspace: setup list with summary cards.
pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Setups");
    ui.separator();

    // Stock summary — show effective dims for the active setup orientation
    let stock = state.session.stock_config();
    let (eff_w, eff_d, eff_h) = if let Some(setup) = active_setup(state) {
        let (w, d, h) = setup.face_up.effective_stock(stock.x, stock.y, stock.z);
        setup.z_rotation.effective_stock(w, d, h)
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
    ui.separator();

    // Setup cards — the rail's primary navigation job (SHE-007), promoted
    // directly under the Stock card instead of below the project rollups.
    for setup in state.session.list_setups() {
        draw_setup_card(ui, setup, state, events);
        ui.add_space(4.0);
    }

    // Add setup button
    let setups = state.session.list_setups();
    if setups.len() > 1 || !setups.is_empty() {
        ui.add_space(4.0);
        if ui.button("+ Add Setup").clicked() {
            events.push(AppEvent::AddSetup);
        }
    }

    ui.add_space(8.0);

    // Models section (compact)
    egui::CollapsingHeader::new("Models")
        .default_open(false)
        .show(ui, |ui| {
            if state.session.models().is_empty() {
                ui.label(
                    egui::RichText::new("No models imported")
                        .italics()
                        .color(theme::TEXT_DIM),
                );
            }
            for model in state.session.models() {
                let mid = ModelId(model.id);
                let selected = state.selection == Selection::Model(mid);
                let response = ui.selectable_label(selected, &model.name);
                if response.clicked() {
                    events.push(AppEvent::Select(Selection::Model(mid)));
                }
                response.context_menu(|ui| {
                    if ui.button("Reload from disk").clicked() {
                        events.push(AppEvent::ReloadModel(mid));
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
                        events.push(AppEvent::RemoveModel(mid));
                        ui.close();
                    }
                });
            }
        });

    ui.add_space(12.0);
    ui.separator();

    // Project rollups demoted to the bottom (SHE-007): whole-project readouts
    // belong with project chrome, not interleaved into per-setup navigation.
    // Summary sits behind a disclosure; diagnostics self-hide when empty.
    draw_project_summary(ui, state);
    draw_project_diagnostics_card(ui, state);
}

fn draw_setup_card(
    ui: &mut egui::Ui,
    setup: &SetupData,
    state: &AppState,
    events: &mut Vec<AppEvent>,
) {
    let setup_id = SetupId(setup.id);
    let is_selected = state.selection == Selection::Setup(setup_id);
    let base_border = if is_selected {
        theme::ACCENT
    } else {
        egui::Color32::from_rgb(55, 55, 65)
    };

    let card_response = egui::Frame::default()
        .fill(egui::Color32::from_rgb(38, 40, 50))
        .stroke(egui::Stroke::new(1.0, base_border))
        .inner_margin(8.0)
        .corner_radius(4)
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
                let orient = if setup.z_rotation == rs_cam_core::compute::transform::ZRotation::Deg0
                {
                    setup.face_up.label().to_owned()
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
                let datum_config = state.gui.setup_rt.get(&setup.id);
                let datum = datum_config
                    .map(|srt| match &srt.datum.xy_method {
                        XYDatum::CornerProbe(c) => format!("Corner ({})", c.label()),
                        XYDatum::CenterOfStock => "Center".into(),
                        XYDatum::AlignmentPins => "Pins".into(),
                        XYDatum::Manual => "Manual".into(),
                    })
                    .unwrap_or_else(|| "Corner (Front-Left)".into());
                chip(ui, "XY", &datum, egui::Color32::from_rgb(140, 160, 100));
            });

            // Counts row
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;

                let fixture_count = setup.fixtures.len();
                let keepout_count = setup.keep_out_zones.len();
                let pin_count = state.session.stock_config().alignment_pins.len();

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
            if state.session.list_setups().first().map(|s| s.id) != Some(setup.id) {
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
        events.push(AppEvent::Select(Selection::Setup(setup_id)));
    }

    // Update border color on hover
    if card_response.hovered() && !is_selected {
        let hover_border = egui::Color32::from_rgb(80, 120, 170);
        ui.painter().rect_stroke(
            card_response.rect,
            4.0,
            egui::Stroke::new(1.0, hover_border),
            egui::StrokeKind::Middle,
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

/// Compact project summary: ops, tools, estimated time, readiness.
fn draw_project_diagnostics_card(ui: &mut egui::Ui, state: &AppState) {
    use rs_cam_core::diagnostics::{Category, Severity};

    // Build the same viz-side evidence the MCP handler uses so the
    // GUI and MCP report identical project diagnostics.
    let boundaries: Vec<(rs_cam_core::ToolpathId, usize, usize)> = state
        .simulation
        .results
        .as_ref()
        .map(|r| {
            r.boundaries
                .iter()
                .map(|b| (b.id, b.start_move, b.end_move))
                .collect()
        })
        .unwrap_or_default();
    let cut_trace = state
        .simulation
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_deref());
    let evidence = rs_cam_core::session::ProjectEvidence {
        boundaries,
        rapid_collisions: &state.simulation.checks.rapid_collisions,
        rapid_collision_move_indices: &state.simulation.checks.rapid_collision_move_indices,
        cut_trace,
        // Last dedicated check only — this card draws every frame, and
        // the diagnostics layer no longer recomputes collision sweeps
        // (the setup-tab lag, 2026-06-11).
        holder_collisions: state.simulation.holder_collision_counts_by_tp(),
    };
    let diagnostics = state.session.diagnose_project_with_evidence(&evidence);
    if diagnostics.is_empty() {
        return;
    }

    ui.add_space(4.0);
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(38, 32, 32))
        .inner_margin(6.0)
        .corner_radius(4)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Project findings")
                    .small()
                    .strong()
                    .color(theme::TEXT_DIM),
            );
            for d in &diagnostics {
                let color = match d.severity {
                    Severity::Blocking | Severity::Critical => {
                        egui::Color32::from_rgb(220, 100, 80)
                    }
                    Severity::Caution => egui::Color32::from_rgb(220, 180, 60),
                    Severity::Hint | Severity::Info => egui::Color32::from_rgb(140, 180, 220),
                };
                let prefix = match d.category {
                    Category::Safety => "Safety",
                    Category::Geometry => "Geometry",
                    Category::ToolLoad => "Tool load",
                    Category::Quality => "Quality",
                    Category::Efficiency => "Efficiency",
                    Category::State => "State",
                };
                ui.label(
                    egui::RichText::new(format!("• {prefix}: {}", d.message))
                        .small()
                        .color(color),
                );
            }
        });
}

fn draw_project_summary(ui: &mut egui::Ui, state: &AppState) {
    let enabled_ops = state
        .session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .count();
    if enabled_ops == 0 {
        return;
    }

    let computed = state
        .session
        .toolpath_configs()
        .iter()
        .filter(|tc| {
            tc.enabled
                && state
                    .gui
                    .toolpath_rt
                    .get(&tc.id)
                    .and_then(|rt| rt.result.as_ref())
                    .is_some()
        })
        .count();
    let tool_count = state.session.tools().len();

    // Estimated cycle time from computed toolpaths
    let mut total_time_min = 0.0_f64;
    for tc in state.session.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        if let Some(rt) = state.gui.toolpath_rt.get(&tc.id)
            && let Some(ref result) = rt.result
        {
            let feed = tc.operation.feed_rate();
            if feed > 0.0 {
                total_time_min += result.stats.cutting_distance / feed;
            }
        }
    }

    let time_str = if total_time_min >= 60.0 {
        let h = (total_time_min / 60.0).floor();
        let m = (total_time_min - h * 60.0).round();
        format!("{h:.0}:{m:02.0}")
    } else if total_time_min >= 1.0 {
        format!("{:.0} min", total_time_min)
    } else {
        format!("{:.0} s", total_time_min * 60.0)
    };

    // SHE-007 — the project rollup is a whole-project readout, demoted behind a
    // disclosure with the headline numbers in the header text. Default
    // collapsed; the full table is one click away. A stable `id_salt` keeps the
    // collapse state from resetting as the header numbers change.
    let header = if computed > 0 {
        format!(
            "Project summary  \u{00B7}  {enabled_ops} ops \u{00B7} {tool_count} tools \u{00B7} ~{time_str}"
        )
    } else {
        format!("Project summary  \u{00B7}  {enabled_ops} ops \u{00B7} {tool_count} tools")
    };
    egui::CollapsingHeader::new(egui::RichText::new(header).small().color(theme::TEXT_MUTED))
        .id_salt("project_summary")
        .default_open(false)
        .show(ui, |ui| {
            egui::Frame::default()
                .fill(egui::Color32::from_rgb(34, 36, 44))
                .inner_margin(6.0)
                .corner_radius(4)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{computed}/{enabled_ops} ops"))
                                .small()
                                .strong()
                                .color(if computed == enabled_ops {
                                    theme::SUCCESS
                                } else {
                                    theme::WARNING
                                }),
                        );
                        ui.label(
                            egui::RichText::new(format!("\u{00B7} {tool_count} tool(s)"))
                                .small()
                                .color(theme::TEXT_DIM),
                        );
                        if computed > 0 {
                            ui.label(
                                egui::RichText::new(format!("\u{00B7} ~{time_str}"))
                                    .small()
                                    .color(theme::TEXT_DIM),
                            );
                        }
                    });
                });
        });
}

/// Determine the active setup from the current selection.
fn active_setup(state: &AppState) -> Option<&SetupData> {
    let setups = state.session.list_setups();
    let setup_id = match &state.selection {
        Selection::Setup(id) => Some(*id),
        Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
        Selection::Toolpath(tp_id) => state
            .session
            .setup_of_toolpath_id(*tp_id)
            .and_then(|idx| setups.get(idx))
            .map(|s| SetupId(s.id)),
        _ => None,
    };
    if let Some(sid) = setup_id {
        setups.iter().find(|s| s.id == sid.0)
    } else {
        setups.first()
    }
}
