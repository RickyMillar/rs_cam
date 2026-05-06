use super::AppEvent;
use super::sim_debug::{semantic_kind_color, semantic_kind_label};
use crate::render::toolpath_render::palette_color;
use crate::state::job::SetupId;
use crate::state::runtime::GuiState;
use crate::state::simulation::SimulationState;
use crate::state::toolpath::ToolpathId;
use crate::state::viewport::ViewportState;
use crate::ui::theme;
use rs_cam_core::session::ProjectSession;

/// Left panel in simulation workspace: slim operation list with visibility and jump controls.
pub fn draw(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    viewport: &mut ViewportState,
    events: &mut Vec<AppEvent>,
) {
    let max_feed = session.machine().max_feed_mm_min;
    ui.heading("Verification");
    ui.separator();

    // --- Setup & run ---
    // Capture toggles + Run button live together: these are the "what
    // should the next sim record?" controls. Display-only toggles (stock
    // coloring, generator overlay) live in the right-panel View section.
    let any_compute = session.toolpath_configs().iter().any(|tc| tc.enabled);
    if any_compute {
        let mut capture_trace_all = session
            .toolpath_configs()
            .iter()
            .any(|tc| tc.debug_options.enabled);
        ui.label(
            egui::RichText::new("Setup & run")
                .small()
                .strong()
                .color(theme::TEXT_HEADING),
        );
        ui.checkbox(
            &mut sim.metric_options.enabled,
            "Capture cutting metrics",
        )
        .on_hover_text(
            "Records per-sample chipload, engagement, MRR during simulation. Required for the bottom-panel signal graphs to show data. Re-run simulation to apply.",
        );
        if ui
            .checkbox(&mut capture_trace_all, "Record generator trace")
            .on_hover_text(
                "Captures the toolpath generator's step-by-step output, used to inspect how a toolpath was built. Re-generate the toolpaths to apply.",
            )
            .changed()
        {
            events.push(AppEvent::SetGeneratorTraceCaptureAll(capture_trace_all));
        }
        if sim.metric_options.enabled {
            sim.metric_options.capture_arc_engagement = true;
        }

        // Resolution: defines how detailed the dexel grid records material
        // removal. Belongs with the capture toggles since it's a recording
        // setting, not a display setting.
        ui.horizontal(|ui| {
            ui.label("Resolution:");
            if sim.auto_resolution {
                ui.label(format!("{:.3} mm (auto)", sim.resolution));
            } else {
                ui.add(
                    egui::Slider::new(&mut sim.resolution, 0.02..=1.0)
                        .suffix(" mm")
                        .logarithmic(true)
                        .show_value(true),
                );
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut sim.auto_resolution, "Auto from tool size");
            if !sim.auto_resolution {
                ui.label(
                    egui::RichText::new("(re-run to apply)")
                        .small()
                        .color(theme::WARNING),
                );
            }
        });
        {
            let sx = session.stock_config().x;
            let sy = session.stock_config().y;
            let res = sim.resolution;
            if !sim.auto_resolution
                && rs_cam_core::dexel::DexelGrid::would_exceed_grid(res, sx, sy).is_some()
            {
                ui.label(
                    egui::RichText::new("Grid too large — resolution will be coarsened")
                        .small()
                        .color(theme::WARNING),
                );
            }
        }

        ui.add_space(4.0);
        let run_label = if sim.boundaries().is_empty() {
            "Run Simulation"
        } else {
            "Re-run Simulation"
        };
        let btn = egui::Button::new(egui::RichText::new(run_label).strong())
            .min_size(egui::vec2(ui.available_width(), 28.0));
        if ui.add(btn).clicked() {
            events.push(AppEvent::RunSimulation);
        }
        ui.add_space(4.0);
        ui.separator();
    }

    // Empty state: no results yet
    if sim.boundaries().is_empty() {
        let has_computed = session.toolpath_configs().iter().any(|tc| {
            tc.enabled
                && gui
                    .toolpath_rt
                    .get(&tc.id)
                    .and_then(|rt| rt.result.as_ref())
                    .is_some()
        });

        egui::Frame::default()
            .fill(theme::CARD_FILL)
            .inner_margin(12.0)
            .rounding(4.0)
            .show(ui, |ui| {
                if has_computed {
                    ui.label(
                        egui::RichText::new("Ready to simulate")
                            .strong()
                            .color(theme::TEXT_HEADING),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "Use Run Simulation above to verify toolpaths, check collisions, and review stock removal.",
                        )
                        .small()
                        .color(theme::TEXT_MUTED),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("No toolpaths computed")
                            .color(theme::WARNING),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "Switch to Toolpaths workspace to add and generate operations first.",
                        )
                        .small()
                        .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(8.0);
                    if ui.button("Go to Toolpaths").clicked() {
                        events.push(AppEvent::SwitchWorkspace(
                            crate::state::Workspace::Toolpaths,
                        ));
                    }
                }
            });
        return;
    }

    // Staleness warning
    if sim.is_stale(gui.edit_counter) {
        egui::Frame::default()
            .fill(egui::Color32::from_rgb(50, 42, 20))
            .stroke(egui::Stroke::new(1.5, theme::WARNING))
            .inner_margin(8.0)
            .rounding(4.0)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("\u{26A0} Results may be stale")
                        .strong()
                        .color(theme::WARNING),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Parameters changed since the last simulation run.")
                        .small()
                        .color(egui::Color32::from_rgb(180, 160, 100)),
                );
                ui.add_space(6.0);
                let btn = egui::Button::new(egui::RichText::new("Re-run Simulation").strong())
                    .min_size(egui::vec2(ui.available_width(), 28.0));
                if ui.add(btn).clicked() {
                    events.push(AppEvent::RunSimulation);
                }
            });
        ui.add_space(4.0);
        ui.separator();
    }

    // Collect selected toolpath IDs for checkbox state
    let all_selected = sim.selected_toolpaths().is_none();
    let selected_set: Vec<ToolpathId> = sim.selected_toolpaths().cloned().unwrap_or_default();
    let boundaries = sim.boundaries().to_vec();
    let setup_boundaries = sim.setup_boundaries().to_vec();
    sim.sync_debug_state(gui, max_feed);
    let active_item = sim.active_semantic_item(gui, max_feed);
    let active_item_id = active_item
        .as_ref()
        .map(|item| (item.toolpath_id, item.item.id));

    // Track if user toggled any checkbox
    let mut toggled_id: Option<ToolpathId> = None;
    let mut current_setup_id: Option<SetupId> = None;

    for (i, boundary) in boundaries.iter().enumerate() {
        // Insert setup transition divider when the setup changes
        let this_setup = setup_boundaries
            .iter()
            .rev()
            .find(|sb| sb.start_move <= boundary.start_move);
        if let Some(sb) = this_setup
            && current_setup_id != Some(sb.setup_id)
        {
            current_setup_id = Some(sb.setup_id);
            if i > 0 {
                ui.add_space(4.0);
                ui.separator();
            }
            ui.label(
                egui::RichText::new(&sb.setup_name)
                    .strong()
                    .color(theme::TEXT_HEADING),
            );
            ui.add_space(2.0);
        }
        let is_focused = sim.focused_toolpath() == Some(boundary.id);
        let pc = palette_color(i);
        let color = egui::Color32::from_rgb(
            (pc[0] * 255.0) as u8,
            (pc[1] * 255.0) as u8,
            (pc[2] * 255.0) as u8,
        );

        // Frame the focused (currently-playing) operation with an accent
        // border so the row visibly tracks playback.
        let frame = if is_focused {
            egui::Frame::default()
                .fill(theme::CARD_FILL_SELECTED)
                .stroke(egui::Stroke::new(1.0, color))
                .inner_margin(4.0)
                .rounding(3.0)
        } else {
            egui::Frame::default().inner_margin(4.0)
        };

        let inner = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                // Checkbox for including in simulation
                let mut checked = all_selected || selected_set.contains(&boundary.id);
                if ui.checkbox(&mut checked, "").changed() {
                    toggled_id = Some(boundary.id);
                }

                // Palette color swatch
                let (swatch_rect, _) =
                    ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                ui.painter().rect_filled(swatch_rect, 2.0, color);

                // Operation name — visual focus indicator only; the click
                // target is the whole card (handled below the frame).
                let name_text = egui::RichText::new(&boundary.name).small();
                let name_text = if is_focused {
                    name_text.strong()
                } else {
                    name_text
                };
                ui.label(name_text);
            });

            ui.label(
                egui::RichText::new(&boundary.tool_name)
                    .small()
                    .color(theme::TEXT_MUTED),
            );

            // Per-toolpath visibility controls: eye / cut / rapid / isolate.
            // Shared with the Toolpaths-workspace panel for a consistent row.
            let overall_visible = gui
                .toolpath_rt
                .get(&boundary.id.0)
                .is_none_or(|rt| rt.visible);
            ui.horizontal(|ui| {
                crate::ui::toolpath_row_controls::draw(
                    ui,
                    boundary.id,
                    overall_visible,
                    viewport,
                    events,
                );
            });

            if sim.debug.enabled {
                let has_semantic = gui
                    .toolpath_rt
                    .get(&boundary.id.0)
                    .and_then(|rt| rt.semantic_trace.as_ref())
                    .is_some();
                if is_focused && has_semantic {
                    sim.debug.set_toolpath_expanded(boundary.id, true);
                }

                if has_semantic {
                    ui.add_space(4.0);
                    let expanded = sim.debug.is_toolpath_expanded(boundary.id);
                    let toggle_label = if expanded {
                        "Hide semantics"
                    } else {
                        "Show semantics"
                    };
                    if ui
                        .small_button(toggle_label)
                        .on_hover_text("Expand semantic trace for this toolpath")
                        .clicked()
                    {
                        sim.debug.toggle_toolpath_expanded(boundary.id);
                    }

                    if sim.debug.is_toolpath_expanded(boundary.id) {
                        draw_semantic_outline(ui, sim, gui, boundary, active_item_id, events);
                    }
                }
            }
        });

        // Whole-card click → jump playback to this TP's start. Inner widgets
        // (checkbox, visibility eyes, "Show semantics") consume their own
        // clicks first; only clicks on empty card real-estate fall through
        // here. We give the card a discrete `Id` so the response doesn't
        // collide with neighbours.
        let card_resp = ui
            .interact(
                inner.response.rect,
                ui.id().with(("sim_card", boundary.id.0)),
                egui::Sense::click(),
            )
            .on_hover_text("Click anywhere on this card to jump playback to the toolpath's start.");
        if card_resp.clicked() {
            events.push(AppEvent::SimJumpToOpStart(i));
        }

        if i + 1 < boundaries.len() {
            ui.add_space(2.0);
        }
    }

    // If a checkbox was toggled, re-run sim with new selection
    if let Some(id) = toggled_id {
        let mut new_selection: Vec<ToolpathId> = if all_selected {
            // Was "all" — now exclude the toggled one
            sim.boundaries()
                .iter()
                .map(|b| b.id)
                .filter(|bid| *bid != id)
                .collect()
        } else {
            let mut s = selected_set;
            if s.contains(&id) {
                s.retain(|x| *x != id);
            } else {
                s.push(id);
            }
            s
        };

        // If all are selected again, use None (meaning "all")
        if new_selection.len() == boundaries.len() {
            new_selection.clear();
        }

        if new_selection.is_empty() {
            events.push(AppEvent::RunSimulation);
        } else {
            events.push(AppEvent::RunSimulationWith(new_selection));
        }
    }

    fn draw_semantic_outline(
        ui: &mut egui::Ui,
        sim: &mut SimulationState,
        gui: &GuiState,
        boundary: &crate::state::simulation::ToolpathBoundary,
        active_item_id: Option<(ToolpathId, u64)>,
        events: &mut Vec<AppEvent>,
    ) {
        let Some(rt) = gui.toolpath_rt.get(&boundary.id.0) else {
            return;
        };
        let Some(trace) = rt.semantic_trace.as_ref() else {
            return;
        };
        let Some(index) = sim.debug.semantic_indexes.get(&boundary.id).cloned() else {
            return;
        };
        let root_items = index
            .child_indices_by_parent
            .get(&None)
            .cloned()
            .unwrap_or_default();
        if root_items.is_empty() {
            ui.label(
                egui::RichText::new("No move-linked semantics")
                    .small()
                    .italics()
                    .color(theme::TEXT_DIM),
            );
            return;
        }

        ui.add_space(2.0);
        for item_index in root_items {
            draw_semantic_item_row(
                ui,
                trace,
                &index,
                sim,
                boundary,
                item_index,
                0,
                active_item_id,
                events,
            );
        }
    }

    // SAFETY: item_index from recursive traversal of trace.items children
    #[allow(clippy::too_many_arguments, clippy::indexing_slicing)]
    fn draw_semantic_item_row(
        ui: &mut egui::Ui,
        trace: &rs_cam_core::semantic_trace::ToolpathSemanticTrace,
        index: &crate::state::simulation::SimulationSemanticIndex,
        sim: &mut SimulationState,
        boundary: &crate::state::simulation::ToolpathBoundary,
        item_index: usize,
        depth: usize,
        active_item_id: Option<(ToolpathId, u64)>,
        events: &mut Vec<AppEvent>,
    ) {
        let item = &trace.items[item_index];
        let color = semantic_kind_color(&item.kind);
        let is_active = active_item_id == Some((boundary.id, item.id));

        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 12.0);
            ui.label(
                egui::RichText::new(semantic_kind_label(&item.kind))
                    .small()
                    .color(color),
            );

            let text = if is_active {
                egui::RichText::new(&item.label).small().strong()
            } else {
                egui::RichText::new(&item.label).small()
            };
            let response = ui.selectable_label(is_active, text);
            if response.clicked() {
                sim.pin_semantic_item(boundary.id, item.id);
                if let Some(move_start) = item.move_start {
                    events.push(AppEvent::SimJumpToMove(boundary.start_move + move_start));
                }
            }

            if let (Some(move_start), Some(move_end)) = (item.move_start, item.move_end) {
                if ui
                    .small_button("►|")
                    .on_hover_text("Jump to semantic item end")
                    .clicked()
                {
                    events.push(AppEvent::SimJumpToMove(boundary.start_move + move_end));
                }
                ui.label(
                    egui::RichText::new(format!("{move_start}-{move_end}"))
                        .small()
                        .color(theme::TEXT_DIM),
                );
            }
        });

        if let Some(children) = index.child_indices_by_parent.get(&Some(item.id)) {
            for child_index in children {
                draw_semantic_item_row(
                    ui,
                    trace,
                    index,
                    sim,
                    boundary,
                    *child_index,
                    depth + 1,
                    active_item_id,
                    events,
                );
            }
        }
    }
}
