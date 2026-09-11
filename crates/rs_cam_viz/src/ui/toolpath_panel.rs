use std::sync::Arc;

use super::AppEvent;
use super::readiness;
use super::sim_debug::draw_trace_badge;
use crate::render::toolpath_render::palette_color;
use crate::state::AppState;
use crate::state::freshness::{FreshnessState, freshness};
use crate::state::job::{SetupId, ToolId};
use crate::state::selection::Selection;
use crate::state::simulation::SimulationState;
use crate::state::toolpath::{OperationType, ToolpathId};
use crate::ui::theme;
use rs_cam_core::compute::config::ToolpathStats;

/// Minimal snapshot of a `ToolpathConfig` with just the fields the card reads.
/// Cloning this releases the `state.session` borrow so `state.viewport` can be
/// borrowed mutably inside the card body.
struct CardInfo {
    id: rs_cam_core::ToolpathId,
    name: String,
    enabled: bool,
    tool_id: usize,
    operation: crate::state::toolpath::OperationConfig,
}

/// Minimal snapshot of `ToolpathRuntime` fields the card needs.
struct RuntimeSnapshot {
    visible: bool,
    auto_regen: bool,
    has_result: bool,
    stats: Option<ToolpathStats>,
    /// The one state every surface should read (R0.1 §4.4). Derived here,
    /// beside the core result cache the derivation needs, because the card
    /// body no longer holds the session borrow.
    freshness: FreshnessState,
}

/// Left panel for the Toolpath workspace: operation queue with status chips.
pub fn draw(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Operations");
    ui.separator();

    // Action bar: generate all
    ui.horizontal(|ui| {
        if ui.button("Generate All").clicked() {
            events.push(AppEvent::GenerateAll);
        }
    });

    ui.add_space(6.0);

    // Clone per-setup metadata up front so we can borrow `state.viewport`
    // mutably inside the draw loop without fighting the borrow checker.
    let setups_data: Vec<(SetupId, String, Vec<usize>)> = state
        .session
        .list_setups()
        .iter()
        .map(|s| (SetupId(s.id), s.name.clone(), s.toolpath_indices.clone()))
        .collect();
    let multi_setup = setups_data.len() > 1;
    let mut global_idx = 0usize;

    for (setup_id, setup_name, toolpath_indices) in setups_data {
        // Setup header (only if multi-setup)
        if multi_setup {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&setup_name)
                        .strong()
                        .color(theme::TEXT_HEADING),
                );
                // Count ready/total
                // F2.2 — `is_current()`, not `ComputeStatus::Done`. An
                // edited operation keeps `Done`, so this header read n/n
                // beside a card that said STALE: the same panel disagreeing
                // with itself about the same operation.
                let ready = toolpath_indices
                    .iter()
                    .filter(|&&idx| {
                        crate::state::freshness::freshness_at(&state.session, &state.gui, idx)
                            .is_some_and(|f| f.is_current())
                    })
                    .count();
                let total = toolpath_indices.len();
                ui.label(
                    egui::RichText::new(format!("{ready}/{total}"))
                        .small()
                        .color(theme::TEXT_DIM),
                );

                // Per-setup + Add menu
                add_toolpath_menu(ui, setup_id, state, events);
            });
            ui.separator();
        }

        // Drop zone for this setup
        let drop_frame = egui::Frame::default().inner_margin(2.0);
        let tp_count = toolpath_indices.len();
        let (inner_resp, dropped_payload) = ui.dnd_drop_zone::<ToolpathId, ()>(drop_frame, |ui| {
            if toolpath_indices.is_empty() {
                ui.label(
                    egui::RichText::new("No toolpaths")
                        .italics()
                        .color(theme::TEXT_DIM),
                );
            }

            for (local_idx, &tp_idx) in toolpath_indices.iter().enumerate() {
                let i = global_idx;
                global_idx += 1;
                // Snapshot just the fields the card reads, releasing the
                // session/gui borrow before we re-enter `state` for viewport.
                let Some(tc_src) = state.session.get_toolpath_config(tp_idx) else {
                    continue;
                };
                let card = CardInfo {
                    id: tc_src.id,
                    name: tc_src.name.clone(),
                    enabled: tc_src.enabled,
                    tool_id: tc_src.tool_id,
                    operation: tc_src.operation.clone(),
                };
                let rt_snap = state
                    .gui
                    .toolpath_rt
                    .get(&card.id)
                    .map(|r| RuntimeSnapshot {
                        visible: r.visible,
                        auto_regen: r.auto_regen,
                        has_result: r.result.is_some(),
                        stats: r.result.as_ref().map(|res| res.stats.clone()),
                        freshness: freshness(
                            tc_src,
                            Some(r),
                            state.session.get_result(tp_idx).is_some(),
                        ),
                    });
                draw_toolpath_card(ui, state, events, &card, rt_snap.as_ref(), i, local_idx);
            }
        });

        // Handle drop
        if let Some(payload) = dropped_payload {
            let dragged_tp_id: ToolpathId = Arc::unwrap_or_clone(payload);
            let source_setup = state
                .session
                .setup_of_toolpath_id(dragged_tp_id)
                .map(|idx| {
                    // SAFETY: setup_of_toolpath_id returns a valid index into list_setups
                    #[allow(clippy::indexing_slicing)]
                    SetupId(state.session.list_setups()[idx].id)
                });

            // Determine drop index from pointer position
            let drop_idx = compute_drop_index(&inner_resp.response, ui, tp_count);

            if source_setup == Some(setup_id) {
                // Same setup: reorder
                events.push(AppEvent::ReorderToolpath(dragged_tp_id, drop_idx));
            } else {
                // Different setup: move
                events.push(AppEvent::MoveToolpathToSetup(
                    dragged_tp_id,
                    setup_id,
                    Some(drop_idx),
                ));
            }
        }
    }

    // Single-setup: show "+ Add" below toolpath list
    if !multi_setup && let Some(setup) = state.session.list_setups().first() {
        let sid = SetupId(setup.id);
        ui.add_space(4.0);
        add_toolpath_menu(ui, sid, state, events);
    }

    // Tool library (compact, collapsed by default). This is the *live*
    // tool home and holds the full CRUD set: per-tool Duplicate/Delete
    // (context menu or Del key), library manager, and library import.
    // W1.1 harvested these from the dead `project_tree.rs` (deleted) — the
    // shipping GUI previously had no way to delete/duplicate a tool or
    // reach the library from the panel.
    ui.add_space(12.0);
    egui::CollapsingHeader::new("Tool Library")
        .default_open(false)
        .show(ui, |ui| {
            if ui
                .small_button("Manage library…")
                .on_hover_text("Browse, edit, and organise the reusable tool catalogs.")
                .clicked()
            {
                events.push(AppEvent::OpenToolLibrary);
            }
            ui.add_space(4.0);
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
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
                        events.push(AppEvent::RemoveTool(tool.id));
                        ui.close();
                    }
                });
            }
            ui.add_space(4.0);
            ui.menu_button("+ Add Tool", |ui| {
                for &tt in crate::state::job::ToolType::ALL {
                    if ui.button(tt.label()).clicked() {
                        events.push(AppEvent::AddTool(tt));
                        ui.close();
                    }
                }
                let libraries = rs_cam_core::tool_library::list_libraries();
                if !libraries.is_empty() {
                    ui.separator();
                    ui.menu_button("From library", |ui| {
                        for lib in &libraries {
                            ui.menu_button(
                                lib,
                                |ui| match rs_cam_core::tool_library::load_library(lib) {
                                    Ok(catalog) if catalog.tools.is_empty() => {
                                        ui.label("(empty)");
                                    }
                                    Ok(catalog) => {
                                        for tool in &catalog.tools {
                                            let label =
                                                format!("{} — ⌀{:.2}mm", tool.name, tool.diameter);
                                            if ui.button(label).clicked() {
                                                events.push(AppEvent::AddToolFromLibrary(
                                                    Box::new(tool.clone()),
                                                ));
                                                ui.close();
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        ui.label(format!("load error: {e}"));
                                    }
                                },
                            );
                        }
                    });
                }
            });
        });
}

/// Draw a single toolpath card, wrapped in a drag source.
fn draw_toolpath_card(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    tc: &CardInfo,
    rt: Option<&RuntimeSnapshot>,
    global_idx: usize,
    _local_idx: usize,
) {
    let tp_id = tc.id;
    let selected = state.selection == Selection::Toolpath(tp_id);
    let visible = rt.is_none_or(|r| r.visible);
    let auto_regen = rt.is_none_or(|r| r.auto_regen);
    // F2.2 — the one state the chip, the stats row and the ▶ button all
    // read. It replaces the `ComputeStatus::effective` call that used to
    // stand here: A/M11's "one taxonomy" rule is unchanged and `enabled`
    // still wins over everything, but `FreshnessState` folds that in itself
    // (`Disabled` is its first arm) and adds the one state `ComputeStatus`
    // cannot express — generated, then edited. A card with no runtime entry
    // at all has never been generated.
    let freshness = rt.map_or(&FreshnessState::NoResult, |r| &r.freshness);
    let has_result = rt.is_some_and(|r| r.has_result);
    let stats = rt.and_then(|r| r.stats.as_ref());
    // G-TIMEEST — the card's per-op time. Resolved HERE, before the card body
    // takes `&mut state`, and from the one shared decision rather than the
    // local `cutting_distance / feed` this row used to carry. `CycleTime` is
    // `Copy`, so nothing borrows `state` past this line.
    let cycle = stats.map_or(readiness::CycleTime::NONE, |s| {
        readiness::toolpath_cycle_time(
            &state.session,
            state
                .simulation
                .results
                .as_ref()
                .and_then(|r| r.cut_trace.as_ref()),
            tp_id,
            s.cutting_distance,
            tc.operation.feed_rate(),
        )
    });
    let dim = !tc.enabled || !visible;

    let pc = palette_color(global_idx);
    let swatch_color = egui::Color32::from_rgb(
        (pc[0] * 255.0) as u8,
        (pc[1] * 255.0) as u8,
        (pc[2] * 255.0) as u8,
    );

    let border_color = if selected {
        swatch_color
    } else {
        egui::Color32::from_rgb(48, 48, 58)
    };

    let inner_response = egui::Frame::default()
        .fill(if selected {
            theme::CARD_FILL_SELECTED
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::new(1.0, border_color))
        .inner_margin(4.0)
        .corner_radius(3)
        .show(ui, |ui| {
            // MCP parameter highlight: glow the card when an MCP action recently
            // changed a parameter on this toolpath.
            #[cfg(feature = "mcp")]
            {
                let prefix = format!("toolpath_{}_", tc.id);
                // Find the most recent highlight timestamp for this toolpath.
                let newest = state
                    .gui
                    .mcp_highlights
                    .iter()
                    .filter(|(k, _)| k.starts_with(&prefix))
                    .map(|(_, when)| *when)
                    .max();
                if let Some(when) = newest {
                    let elapsed = when.elapsed().as_secs_f32();
                    let duration = 2.0;
                    if elapsed < duration {
                        // SAFETY: arithmetic bounded: (1.0 - 0..1) * 60.0 = 0..60, fits u8.
                        #[allow(clippy::indexing_slicing)]
                        let alpha = ((1.0 - elapsed / duration) * 60.0) as u8;
                        let rect = ui.max_rect();
                        ui.painter().rect_filled(
                            rect,
                            3.0,
                            egui::Color32::from_rgba_unmultiplied(100, 180, 255, alpha),
                        );
                    }
                }
            }

            // Click anywhere on the card to select.
            let card_resp = ui.interact(
                ui.max_rect(),
                egui::Id::new("tp_card_click").with(tc.id),
                egui::Sense::click(),
            );
            if card_resp.clicked() {
                events.push(AppEvent::Select(Selection::Toolpath(tp_id)));
            }

            // Row 1: drag grip + swatch + status + name
            ui.horizontal(|ui| {
                // Drag grip handle — drag this to reorder.
                let grip_id = egui::Id::new("tp_grip").with(tc.id);
                let (grip_rect, grip_resp) =
                    ui.allocate_exact_size(egui::vec2(10.0, 14.0), egui::Sense::drag());
                // Draw grip dots (⠿)
                let grip_color = if grip_resp.dragged() {
                    theme::ACCENT
                } else if grip_resp.hovered() {
                    theme::TEXT_MUTED
                } else {
                    theme::TEXT_FAINT
                };
                ui.painter().text(
                    grip_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "\u{2807}",
                    egui::FontId::proportional(12.0),
                    grip_color,
                );
                if grip_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
                if grip_resp.dragged() {
                    egui::DragAndDrop::set_payload(ui.ctx(), tp_id);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
                let _ = grip_id; // used for identification

                // Color swatch
                let (rect, _) = ui.allocate_exact_size(egui::vec2(6.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, swatch_color);

                // Status chip. The mapping is a pure function so the
                // sentry can drive every state without a `Ui` (F2.2).
                let (status_text, status_color, hover) = status_chip(freshness);
                let chip_resp = ui.label(
                    egui::RichText::new(status_text)
                        .small()
                        .strong()
                        .color(status_color),
                );
                if let Some(msg) = hover {
                    chip_resp.on_hover_text(msg);
                }
                // Manual-gen indicator for 3D ops
                if !auto_regen {
                    ui.label(
                        egui::RichText::new("MAN")
                            .small()
                            .color(theme::TEXT_FAINT),
                    )
                    .on_hover_text(
                        "Manual generation \u{2014} press G to generate this operation. 3D operations are not auto-regenerated on parameter change.",
                    );
                }

                // Trace badges are generator-debug provenance. When every
                // toolpath carries the identical availability the badge is
                // wallpaper (8× "TRACE" said nothing in the 2026-06-11
                // capture sweep) — show it only on cards that differ from
                // the rest of the queue.
                let availability =
                    SimulationState::trace_availability_for_toolpath(&state.gui, tp_id);
                let uniform = state.session.toolpath_configs().iter().all(|other| {
                    SimulationState::trace_availability_for_toolpath(&state.gui, other.id)
                        == availability
                });
                if !uniform {
                    draw_trace_badge(ui, availability);
                }

                // Name
                let text_color = if dim {
                    theme::TEXT_FAINT
                } else {
                    egui::Color32::from_rgb(190, 190, 200)
                };
                ui.label(egui::RichText::new(&tc.name).color(text_color));
            });

            // Row 2: tool info + quick actions
            ui.horizontal(|ui| {
                // Tool name
                if let Some(tool) = state.session.tools().iter().find(|t| t.id == ToolId(tc.tool_id)) {
                    ui.label(
                        egui::RichText::new(tool.summary())
                            .small()
                            .color(theme::TEXT_MUTED),
                    );
                }

                // Rest dependency badge
                if let crate::state::toolpath::OperationConfig::Rest(ref rest_cfg) = tc.operation {
                    draw_rest_badge(ui, rest_cfg, state, tp_id);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if has_result
                        && ui
                            .small_button("Sim")
                            .on_hover_text("Inspect in Simulation")
                            .clicked()
                    {
                        events.push(AppEvent::InspectToolpathInSimulation(tp_id));
                    }

                    // Quick generate button. F2.2: driven by freshness, not
                    // `ComputeStatus::needs_generation()`, which answers
                    // `false` for `Done` — and an edited operation keeps
                    // `Done`, so the button was hidden on precisely the card
                    // whose whole message is "regenerate me".
                    let needs_generation = !matches!(
                        freshness,
                        FreshnessState::Current
                            | FreshnessState::Regenerating
                            | FreshnessState::Disabled
                    );
                    if needs_generation
                        && ui
                            .small_button("\u{25B6}")
                            .on_hover_text("Generate")
                            .clicked()
                    {
                        events.push(AppEvent::GenerateToolpath(tp_id));
                    }
                });
            });

            // Row 3: stats (only when computed)
            if let Some(stats) = stats {
                let total_dist_m = (stats.cutting_distance + stats.rapid_distance) / 1000.0;
                let time_str = match cycle.basis {
                    Some(_) => readiness::format_cycle_time(cycle.seconds),
                    // No estimate is a dash, never a plausible-looking 0 s.
                    None => "\u{2014}".to_owned(),
                };
                // F2.2 (R0.1 §4.4): on an edited operation these figures
                // count the PREVIOUS generation's moves. Left unmarked they
                // are the strongest thing on the card saying "this is a
                // finished, measured operation" — an amber chip two rows up
                // does not undo three confident numbers. The prefix says
                // whose they are and the fainter colour stops them reading
                // as the current answer.
                let is_stale = matches!(freshness, FreshnessState::EditedSince);
                let stats_text = format!(
                    "{}{} moves \u{00B7} {} \u{00B7} {:.1} m",
                    if is_stale { "old: " } else { "" },
                    stats.move_count,
                    time_str,
                    total_dist_m,
                );
                let resp = ui.label(
                    egui::RichText::new(stats_text).small().color(if is_stale {
                        theme::TEXT_FAINT
                    } else {
                        theme::TEXT_DIM
                    }),
                );
                // The card is too narrow for the basis inline, so it lives on
                // hover here — the surfaces an operator plans a cut from
                // (readiness, pre-flight, export, setup sheet) all state it
                // without hovering.
                if let Some(basis) = cycle.basis {
                    resp.on_hover_text(format!(
                        "Estimated time ({}). {}",
                        basis.qualifier(),
                        basis.caveat()
                    ));
                }
            }

            // Row 4: shared per-toolpath row controls (eye / C / R / isolate).
            // Hover/selection only (density Batch 2) — six always-on glyphs
            // per card made an 8-op rail ~100 touch targets when a scan
            // needs swatch + name + status. Every action also remains
            // reachable from the right-click context menu.
            //
            // The hover test runs against the FULL card rect from the
            // previous frame (stored in temp memory), and geometrically
            // (`rect_contains_pointer`, not `Response::hovered`): testing
            // only the content drawn so far made the row vanish the moment
            // the pointer entered it, and a hit-test-layered check would
            // flicker when the row's own buttons take hover priority.
            let card_rect_id = egui::Id::new("tp_card_full_rect").with(tc.id);
            let hovered_card = ui
                .ctx()
                .data_mut(|d| d.get_temp::<egui::Rect>(card_rect_id))
                .is_some_and(|r| ui.rect_contains_pointer(r));
            let controls_visible = selected || hovered_card;
            if controls_visible {
                ui.horizontal(|ui| {
                    crate::ui::toolpath_row_controls::draw(
                        ui,
                        tp_id,
                        visible,
                        Some(tc.enabled),
                        &mut state.viewport,
                        events,
                    );
                });
            }
            // Remember this card's full extent (rows 1–4 as drawn this
            // frame, padded by the frame margin) for next frame's test.
            ui.ctx()
                .data_mut(|d| d.insert_temp(card_rect_id, ui.min_rect().expand(4.0)));

            // Context menu
            card_resp.context_menu(|ui| {
                if ui.button("Generate").clicked() {
                    events.push(AppEvent::GenerateToolpath(tp_id));
                    ui.close();
                }
                if has_result && ui.button("Inspect in Simulation").clicked() {
                    events.push(AppEvent::InspectToolpathInSimulation(tp_id));
                    ui.close();
                }
                let is_isolated = state.viewport.isolate_toolpath == Some(tp_id);
                let iso_label = if is_isolated {
                    "Clear isolation"
                } else {
                    "Isolate this toolpath"
                };
                if ui.button(iso_label).clicked() {
                    if is_isolated {
                        events.push(AppEvent::ClearIsolation);
                    } else {
                        events.push(AppEvent::Select(Selection::Toolpath(tp_id)));
                        events.push(AppEvent::ToggleIsolateToolpath);
                    }
                    ui.close();
                }
                let vis_label = if visible { "Hide" } else { "Show" };
                if ui.button(vis_label).clicked() {
                    events.push(AppEvent::ToggleToolpathVisibility(tp_id));
                    ui.close();
                }
                let en_label = if tc.enabled { "Disable" } else { "Enable" };
                if ui.button(en_label).clicked() {
                    events.push(AppEvent::ToggleToolpathEnabled(tp_id));
                    ui.close();
                }
                if ui.button("Duplicate").clicked() {
                    events.push(AppEvent::DuplicateToolpath(tp_id));
                    ui.close();
                }
                ui.separator();
                if ui.button("Move Up").clicked() {
                    events.push(AppEvent::MoveToolpathUp(tp_id));
                    ui.close();
                }
                if ui.button("Move Down").clicked() {
                    events.push(AppEvent::MoveToolpathDown(tp_id));
                    ui.close();
                }
                ui.separator();
                if ui.button("Delete").clicked() {
                    events.push(AppEvent::RemoveToolpath(tp_id));
                    ui.close();
                }
            });
        });

    let inner_response = inner_response.response;

    // If this card is being hovered while something is dragged, show insertion indicator
    if egui::DragAndDrop::has_payload_of_type::<ToolpathId>(ui.ctx())
        && inner_response.contains_pointer()
    {
        let rect = inner_response.rect;
        let painter = ui.painter();
        // Draw a thin line at the bottom to indicate drop position
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(2.0, theme::ACCENT),
        );
    }
}

/// Compute the drop index based on pointer position relative to the drop zone.
fn compute_drop_index(response: &egui::Response, ui: &egui::Ui, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    // Simple heuristic: use the Y position within the drop zone
    let pointer_y = ui
        .input(|i| i.pointer.hover_pos())
        .unwrap_or(response.rect.center())
        .y;
    let zone_top = response.rect.top();
    let zone_height = response.rect.height().max(1.0);
    let fraction = ((pointer_y - zone_top) / zone_height).clamp(0.0, 1.0);
    let idx = (fraction * count as f32).round() as usize;
    idx.min(count)
}

/// Chip text, colour and hover for one freshness state — the card's whole
/// vocabulary, as a pure function so it can be tested without a `Ui`.
///
/// F2.2: driven by [`FreshnessState`], not `ComputeStatus`. The two agree on
/// six of seven; the seventh is the point. An operation whose inputs moved
/// after it was generated still carries `ComputeStatus::Done` — the
/// generation that produced the drawn geometry really did finish — so this
/// chip used to read a confident green `OK` over geometry the project can no
/// longer reproduce.
pub(crate) fn status_chip(
    freshness: &FreshnessState,
) -> (&'static str, egui::Color32, Option<&str>) {
    match freshness {
        FreshnessState::NoResult => ("PEND", theme::TEXT_DIM, None),
        FreshnessState::Regenerating => ("GEN", theme::WARNING, None),
        FreshnessState::Current => ("OK", theme::SUCCESS_BRIGHT, None),
        // Amber, and never green: this is not a fresh result. Amber rather
        // than red because nothing is WRONG — no gate tripped, no collision
        // — the answer on screen is simply the previous question's. Red is
        // what this panel spends on ERR and on collisions, and spending it
        // here would flatten that distinction.
        //
        // The word is "STALE" because that is the vocabulary the simulation
        // badge, `is_stale` and the MCP wire already use, and a second word
        // for one idea is how three stores came to disagree in the first
        // place. The hover carries the weight four letters cannot: what is
        // drawn AND the numbers below it are the previous generation's.
        FreshnessState::EditedSince => (
            "STALE",
            theme::WARNING,
            Some(
                "Inputs changed after this was generated. The path drawn in the viewport \
                 and the figures below are from the PREVIOUS generation, not from the \
                 settings now in the project. Regenerate.",
            ),
        ),
        // A/M11: WAIT is a sequencing state, not a failure — it must not
        // read as red. Hover names the blocking op.
        FreshnessState::WaitingOnUpstream(block) => {
            ("WAIT", theme::WARNING, Some(block.message.as_str()))
        }
        FreshnessState::Disabled => ("OFF", theme::TEXT_FAINT, None),
        FreshnessState::Error(msg) => ("ERR", theme::ERROR, Some(msg.as_str())),
    }
}

/// Menu button for adding a toolpath to a specific setup.
/// Emits Select(Setup(id)) first, then AddToolpath, so the handler targets the right setup.
fn add_toolpath_menu(
    ui: &mut egui::Ui,
    setup_id: SetupId,
    state: &AppState,
    events: &mut Vec<AppEvent>,
) {
    let has_mesh = state.session.models().iter().any(|m| m.mesh.is_some());
    let has_polygons = state.session.models().iter().any(|m| m.polygons.is_some());

    ui.menu_button("+ Add", |ui| {
        ui.label(egui::RichText::new("2.5D (from SVG)").strong());
        for &op in OperationType::ALL_2D {
            add_op_menu_item(ui, op, setup_id, has_mesh, has_polygons, events);
        }
        ui.separator();
        ui.label(egui::RichText::new("3D (from STL)").strong());
        for &op in OperationType::ALL_3D {
            add_op_menu_item(ui, op, setup_id, has_mesh, has_polygons, events);
        }
    });
}

fn add_op_menu_item(
    ui: &mut egui::Ui,
    op: OperationType,
    setup_id: SetupId,
    has_mesh: bool,
    has_polygons: bool,
    events: &mut Vec<AppEvent>,
) {
    use crate::state::toolpath::GeometryRequirement;

    let spec = op.spec();
    let available = match spec.geometry {
        GeometryRequirement::Stock => true,
        GeometryRequirement::Polygons => has_polygons,
        GeometryRequirement::Mesh => has_mesh,
        GeometryRequirement::Both => has_polygons && has_mesh,
    };

    if available {
        if ui
            .button(spec.label)
            .on_hover_text(spec.description)
            .clicked()
        {
            events.push(AppEvent::Select(Selection::Setup(setup_id)));
            events.push(AppEvent::AddToolpath(op));
            ui.close();
        }
    } else {
        let reason = match spec.geometry {
            GeometryRequirement::Polygons => "Requires 2D geometry (SVG/DXF)",
            GeometryRequirement::Mesh => "Requires 3D mesh (STL/STEP)",
            GeometryRequirement::Both => "Requires both 2D curves and 3D mesh",
            GeometryRequirement::Stock => "",
        };
        ui.add_enabled(false, egui::Button::new(spec.label))
            .on_disabled_hover_text(format!("{}\n{}", spec.description, reason));
    }
}

/// What the Rest dependency badge says about one Rest card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestBadge {
    /// A predecessor exists and its runtime is generated and fresh: green `dep`.
    Resolved,
    /// A predecessor exists but needs generation or is stale: yellow `dep`.
    Stale,
    /// No predecessor, or no previous tool configured: red `no dep`.
    Missing,
}

impl RestBadge {
    /// The badge label the card draws.
    pub fn text(self) -> &'static str {
        match self {
            Self::Resolved | Self::Stale => "dep",
            Self::Missing => "no dep",
        }
    }
}

/// Decide the Rest dependency badge for the Rest card `tp_id`.
///
/// G-RESTBADGE (2026-09-10): the predecessor rule is
/// [`crate::state::rest_dependency::rest_predecessors`], the same rule the
/// static validator refuses on. Before this the badge accepted ANY other
/// toolpath in the setup with the previous tool, whatever its order, enabled
/// state or model, so a Rest card dragged above its roughing pass kept a
/// green `dep` while Generate was refused (R05 §2, defect 2).
///
/// Pure with respect to egui so the sentry can read it. `draw_rest_badge`
/// maps the result to a colour.
pub fn rest_badge(
    state: &AppState,
    rest_cfg: &crate::state::toolpath::RestConfig,
    tp_id: ToolpathId,
) -> RestBadge {
    let Some(prev_tool_id) = rest_cfg.prev_tool_id else {
        return RestBadge::Missing;
    };
    let Some(model_id) = state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == tp_id)
        .map(|tc| crate::state::job::ModelId(tc.model_id))
    else {
        return RestBadge::Missing;
    };
    let predecessors = crate::state::rest_dependency::rest_predecessors_in_session(
        &state.session,
        tp_id,
        model_id,
        prev_tool_id,
    );
    if predecessors.is_empty() {
        return RestBadge::Missing;
    }
    // A predecessor that is not CURRENT is not ready. A/M11: a dep that is
    // blocked on upstream stock is just as un-ready as a pending one.
    //
    // F2.2: `is_current()`, not `needs_generation() || stale_since.is_some()`.
    // `ComputeStatus::Done` is what an edited predecessor still carries, so
    // the first half said "ready"; the second half leaned on `stale_since`,
    // which F2.1 demoted to the auto-regeneration debounce clock and is NOT
    // a claim about correctness. The rows that drop a core result without
    // setting a regeneration request — toggle enabled, reorder,
    // move-to-setup, setup orientation (F2.1 §5) — therefore left this badge
    // green over a predecessor whose result no longer exists.
    let dep_stale = predecessors.iter().any(|dep_id| {
        state
            .session
            .find_toolpath_config_by_id(*dep_id)
            .and_then(|(index, _)| {
                crate::state::freshness::freshness_at(&state.session, &state.gui, index)
            })
            .is_none_or(|f| !f.is_current())
    });
    if dep_stale {
        RestBadge::Stale
    } else {
        RestBadge::Resolved
    }
}

/// Show a rest dependency badge for Rest operations.
/// Green "dep" if the dependency is resolved, yellow if stale, red "no dep" if missing.
fn draw_rest_badge(
    ui: &mut egui::Ui,
    rest_cfg: &crate::state::toolpath::RestConfig,
    state: &AppState,
    tp_id: ToolpathId,
) {
    let badge = rest_badge(state, rest_cfg, tp_id);
    let badge_color = match badge {
        RestBadge::Resolved => theme::SUCCESS_BRIGHT,
        RestBadge::Stale => theme::WARNING,
        RestBadge::Missing => theme::ERROR_MILD,
    };
    let badge_text = badge.text();

    ui.label(
        egui::RichText::new(badge_text)
            .small()
            .strong()
            .color(badge_color),
    );
}
