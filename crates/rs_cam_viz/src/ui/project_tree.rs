use super::AppEvent;
use super::sim_debug::draw_trace_badge;
use crate::render::toolpath_render::palette_color;
use crate::state::AppState;
use crate::state::job::{ModelId, SetupId, ToolType};
use crate::state::runtime::ComputeStatus;
use crate::state::selection::Selection;
use crate::state::simulation::SimulationState;
use crate::state::toolpath::OperationType;
use crate::ui::theme;

pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Project");
    ui.separator();

    ui.label(
        egui::RichText::new(format!("Job: {}", state.session.name()))
            .strong()
            .color(theme::TEXT_STRONG),
    );

    ui.add_space(4.0);

    // Stock
    let sc = state.session.stock_config();
    if ui
        .selectable_label(
            state.selection == Selection::Stock,
            format!(
                "Stock ({:.0} x {:.0} x {:.0} mm)",
                sc.x, sc.y, sc.z
            ),
        )
        .clicked()
    {
        events.push(AppEvent::Select(Selection::Stock));
    }

    // Post processor
    if ui
        .selectable_label(
            state.selection == Selection::PostProcessor,
            format!("Post Processor: {}", state.gui.post.format.label()),
        )
        .clicked()
    {
        events.push(AppEvent::Select(Selection::PostProcessor));
    }

    // Machine
    if ui
        .selectable_label(
            state.selection == Selection::Machine,
            format!("Machine: {}", state.session.machine().name),
        )
        .clicked()
    {
        events.push(AppEvent::Select(Selection::Machine));
    }

    ui.add_space(4.0);

    // Models
    ui.collapsing("Models", |ui| {
        if state.session.models().is_empty() {
            ui.label(
                egui::RichText::new("No models imported")
                    .italics()
                    .color(theme::TEXT_DIM),
            );
        }
        for model in state.session.models() {
            use rs_cam_core::compute::stock_config::ModelKind;
            let mid = ModelId(model.id);
            let selected = state.selection == Selection::Model(mid);
            let icon = match model.kind {
                Some(ModelKind::Stl) => "STL",
                Some(ModelKind::Svg) => "SVG",
                Some(ModelKind::Dxf) => "DXF",
                Some(ModelKind::Step) => "STEP",
                None => "?",
            };
            let response = ui.selectable_label(selected, format!("[{}] {}", icon, model.name));
            if response.clicked() {
                events.push(AppEvent::Select(Selection::Model(mid)));
            }
            response.context_menu(|ui| {
                if ui.button("Reload from disk").clicked() {
                    events.push(AppEvent::ReloadModel(mid));
                    ui.close_menu();
                }
                if ui.button("Delete").clicked() {
                    events.push(AppEvent::RemoveModel(mid));
                    ui.close_menu();
                }
            });
        }
    });

    // Tool library
    ui.collapsing("Tool Library", |ui| {
        if state.session.tools().is_empty() {
            ui.label(
                egui::RichText::new("No tools defined")
                    .italics()
                    .color(theme::TEXT_DIM),
            );
        }
        for tool in state.session.tools() {
            let selected = state.selection == Selection::Tool(tool.id);
            let response = ui.selectable_label(selected, tool.summary());
            if response.clicked() {
                events.push(AppEvent::Select(Selection::Tool(tool.id)));
            }
            response.context_menu(|ui| {
                if ui.button("Duplicate").clicked() {
                    events.push(AppEvent::DuplicateTool(tool.id));
                    ui.close_menu();
                }
                if ui.button("Delete").clicked() {
                    events.push(AppEvent::RemoveTool(tool.id));
                    ui.close_menu();
                }
            });
        }
        ui.add_space(4.0);
        ui.menu_button("+ Add Tool", |ui| {
            for &tt in ToolType::ALL {
                if ui.button(tt.label()).clicked() {
                    events.push(AppEvent::AddTool(tt));
                    ui.close_menu();
                }
            }
        });
    });

    let setups = state.session.list_setups();
    let multi_setup = setups.len() > 1;
    let mut global_idx = 0usize;

    for setup in setups {
        let setup_id = SetupId(setup.id);
        let header = if multi_setup {
            use crate::state::job::FaceUp;
            if setup.face_up != FaceUp::Top {
                format!("Setup: {} [{}]", setup.name, setup.face_up.label())
            } else {
                format!("Setup: {}", setup.name)
            }
        } else {
            "Toolpaths".to_owned()
        };

        ui.collapsing(&header, |ui| {
            if multi_setup {
                let setup_selected = state.selection == Selection::Setup(setup_id);
                let resp = ui.selectable_label(
                    setup_selected,
                    egui::RichText::new(&setup.name)
                        .strong()
                        .color(theme::TEXT_HEADING),
                );
                if resp.clicked() {
                    events.push(AppEvent::Select(Selection::Setup(setup_id)));
                }
                resp.context_menu(|ui| {
                    if ui.button("Export G-code").clicked() {
                        events.push(AppEvent::ExportSetupGcode(setup_id));
                        ui.close_menu();
                    }
                    ui.separator();
                    let can_delete = setups.len() > 1;
                    if ui
                        .add_enabled(can_delete, egui::Button::new("Delete Setup"))
                        .clicked()
                    {
                        events.push(AppEvent::RemoveSetup(setup_id));
                        ui.close_menu();
                    }
                });
                ui.separator();
            }

            if !setup.fixtures.is_empty() || !setup.keep_out_zones.is_empty() {
                ui.label(
                    egui::RichText::new("Workholding")
                        .small()
                        .strong()
                        .color(theme::TEXT_MUTED),
                );
                for fixture in &setup.fixtures {
                    let selected = state.selection == Selection::Fixture(setup_id, fixture.id);
                    let dim = !fixture.enabled;
                    let color = if dim {
                        theme::TEXT_FAINT
                    } else {
                        theme::WARNING
                    };
                    let kind_label = fixture_kind_label(&fixture.kind);
                    let label = format!("  {} [{}]", fixture.name, kind_label);
                    let resp =
                        ui.selectable_label(selected, egui::RichText::new(&label).color(color));
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
                for keep_out in &setup.keep_out_zones {
                    let selected = state.selection == Selection::KeepOut(setup_id, keep_out.id);
                    let dim = !keep_out.enabled;
                    let color = if dim { theme::TEXT_FAINT } else { theme::ERROR };
                    let label = format!("  {} (keep-out)", keep_out.name);
                    let resp =
                        ui.selectable_label(selected, egui::RichText::new(&label).color(color));
                    if resp.clicked() {
                        events.push(AppEvent::Select(Selection::KeepOut(setup_id, keep_out.id)));
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Delete").clicked() {
                            events.push(AppEvent::RemoveKeepOut(setup_id, keep_out.id));
                            ui.close_menu();
                        }
                    });
                }
                ui.separator();
            }

            if setup.toolpath_indices.is_empty() {
                ui.label(
                    egui::RichText::new("No toolpaths")
                        .italics()
                        .color(theme::TEXT_DIM),
                );
            }
            for &tp_idx in &setup.toolpath_indices {
                let Some(tc) = state.session.get_toolpath_config(tp_idx) else {
                    continue;
                };
                let rt = state.gui.toolpath_rt.get(&tc.id);
                let tp_id = crate::state::toolpath::ToolpathId(tc.id);
                let i = global_idx;
                global_idx += 1;
                let selected = state.selection == Selection::Toolpath(tp_id);

                let status = rt.map_or(&ComputeStatus::Pending, |r| &r.status);
                let visible = rt.is_none_or(|r| r.visible);
                let has_result = rt.and_then(|r| r.result.as_ref()).is_some();

                let (status_icon, status_color) = match status {
                    ComputeStatus::Pending => ("\u{25CB}", theme::TEXT_DIM),
                    ComputeStatus::Computing => ("\u{25CF}", theme::WARNING),
                    ComputeStatus::Done => ("\u{25CF}", theme::SUCCESS_BRIGHT),
                    ComputeStatus::Error(_) => ("\u{25CF}", theme::ERROR),
                };

                let pc = palette_color(i);
                let swatch_color = egui::Color32::from_rgb(
                    (pc[0] * 255.0) as u8,
                    (pc[1] * 255.0) as u8,
                    (pc[2] * 255.0) as u8,
                );

                let dim = !tc.enabled || !visible;

                let row = ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 1.0, swatch_color);
                    ui.label(
                        egui::RichText::new(status_icon)
                            .color(status_color)
                            .size(10.0),
                    );

                    let text_color = if dim {
                        theme::TEXT_FAINT
                    } else if selected {
                        egui::Color32::from_rgb(220, 220, 230)
                    } else {
                        egui::Color32::from_rgb(180, 180, 190)
                    };
                    let label = format!("[{}] {}", i + 1, tc.name);
                    let _ = ui
                        .selectable_label(selected, egui::RichText::new(&label).color(text_color));
                    draw_trace_badge(
                        ui,
                        SimulationState::trace_availability_for_toolpath(&state.job, tp_id),
                    );
                });
                // Make the full row clickable (not just the label text)
                let response = ui.interact(
                    row.response.rect,
                    egui::Id::new(("tp_row", tc.id)),
                    egui::Sense::click(),
                );
                if response.clicked() {
                    events.push(AppEvent::Select(Selection::Toolpath(tp_id)));
                }

                response.context_menu(|ui| {
                    let vis_label = if visible { "Hide" } else { "Show" };
                    if ui.button(vis_label).clicked() {
                        events.push(AppEvent::ToggleToolpathVisibility(tp_id));
                        ui.close_menu();
                    }
                    let en_label = if tc.enabled { "Disable" } else { "Enable" };
                    if ui.button(en_label).clicked() {
                        events.push(AppEvent::ToggleToolpathEnabled(tp_id));
                        ui.close_menu();
                    }
                    if ui.button("Duplicate").clicked() {
                        events.push(AppEvent::DuplicateToolpath(tp_id));
                        ui.close_menu();
                    }
                    if has_result && ui.button("Inspect in Simulation").clicked() {
                        events.push(AppEvent::InspectToolpathInSimulation(tp_id));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Move Up").clicked() {
                        events.push(AppEvent::MoveToolpathUp(tp_id));
                        ui.close_menu();
                    }
                    if ui.button("Move Down").clicked() {
                        events.push(AppEvent::MoveToolpathDown(tp_id));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Delete").clicked() {
                        events.push(AppEvent::RemoveToolpath(tp_id));
                        ui.close_menu();
                    }
                });
            }

            ui.add_space(4.0);
            ui.menu_button("+ Add Toolpath", |ui| {
                ui.label(egui::RichText::new("2.5D (Boundary)").strong());
                for &op in OperationType::ALL_2D {
                    if ui.button(op.label()).clicked() {
                        events.push(AppEvent::AddToolpath(op));
                        ui.close_menu();
                    }
                }
                ui.separator();
                ui.label(egui::RichText::new("3D (Surface)").strong());
                for &op in OperationType::ALL_3D {
                    if ui.button(op.label()).clicked() {
                        events.push(AppEvent::AddToolpath(op));
                        ui.close_menu();
                    }
                }
            });
        });
    }

    if ui.small_button("+ Add Setup").clicked() {
        events.push(AppEvent::AddSetup);
    }

    ui.add_space(8.0);

    // Import buttons
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("+ STL").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("STL", &["stl", "STL"])
                .pick_file()
        {
            events.push(AppEvent::ImportStl(path));
        }
        if ui.small_button("+ SVG").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("SVG", &["svg", "SVG"])
                .pick_file()
        {
            events.push(AppEvent::ImportSvg(path));
        }
        if ui.small_button("+ DXF").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("DXF", &["dxf", "DXF"])
                .pick_file()
        {
            events.push(AppEvent::ImportDxf(path));
        }
        if ui.small_button("+ STEP").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("STEP", &["step", "stp", "STEP", "STP"])
                .pick_file()
        {
            events.push(AppEvent::ImportStep(path));
        }
    });
}

/// Human-readable label for a session `FixtureKind`.
fn fixture_kind_label(kind: &rs_cam_core::session::FixtureKind) -> &'static str {
    use rs_cam_core::session::FixtureKind;
    match kind {
        FixtureKind::Clamp => "Clamp",
        FixtureKind::Vise => "Vise",
        FixtureKind::VacuumPod => "Vacuum Pod",
        FixtureKind::Custom => "Custom",
    }
}
