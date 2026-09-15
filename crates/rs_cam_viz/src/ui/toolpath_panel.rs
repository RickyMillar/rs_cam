use std::sync::Arc;

use super::AppEvent;
use crate::render::toolpath_render::palette_color;
use crate::state::AppState;
use crate::state::freshness::{FreshnessState, freshness};
use crate::state::job::{SetupId, ToolId};
use crate::state::selection::Selection;
use crate::state::toolpath::{OperationType, ToolpathId};
use crate::state::viewport::ViewportState;
use crate::ui::theme;
use crate::ui::tokens;
use crate::ui_command::UiCommand;

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
    has_result: bool,
    /// The one state every surface should read (R0.1 §4.4). Derived here,
    /// beside the core result cache the derivation needs, because the card
    /// body no longer holds the session borrow.
    freshness: FreshnessState,
}

/// Left panel for the Toolpath workspace: the operation queue.
pub fn draw(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    // DC2 — the heading and its rule leave the panel. A panel inside a
    // workspace tab named "Toolpaths" does not need a second word for the
    // same idea, and the rule separated that word from nothing. The one
    // action this panel is FOR takes the space, at the full section width.
    ui.add_space(tokens::SPACE_2);
    if ui
        .add(crate::ui::components::Button::primary("Generate All").min_width(ui.available_width()))
        .clicked()
    {
        events.push(AppEvent::GenerateAll);
    }

    ui.add_space(tokens::SPACE_4);

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
            ui.add_space(tokens::SPACE_2);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&setup_name)
                        .strong()
                        .color(tokens::TEXT_STRONG),
                );

                // DC2 — the `{ready}/{total}` fraction leaves this header.
                // A fraction between two words is a state rendered as almost
                // nothing (Pattern B). The setup card carries the state
                // indicator instead.

                // Per-setup add menu.
                add_toolpath_menu(ui, setup_id, state, events);
            });
            ui.separator();
        }

        // Drop zone for this setup
        let drop_frame = egui::Frame::default().inner_margin(2.0);
        let tp_count = toolpath_indices.len();
        let (inner_resp, dropped_payload) = ui.dnd_drop_zone::<ToolpathId, ()>(drop_frame, |ui| {
            if toolpath_indices.is_empty() {
                crate::ui::components::EmptyState::new("No toolpaths")
                    .detail("Add an operation to begin.")
                    .show(ui);
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
                        has_result: r.result.is_some(),
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

    // Single-setup: the add menu sits below the operation list.
    if !multi_setup && let Some(setup) = state.session.list_setups().first() {
        let sid = SetupId(setup.id);
        ui.add_space(tokens::SPACE_2);
        add_toolpath_menu(ui, sid, state, events);
    }

    // R29 — the Tool Library left this panel for the Setup workspace. A tool
    // is a project RESOURCE, like the stock, the machine and the models, and
    // Setup is where the resources live. It was a collapsed disclosure at the
    // bottom of the operation queue, which drew one object kind at a weight
    // no other resource carries (Rule D).
}

/// Draw a single toolpath card, wrapped in a drag source.
///
/// R32 — the card is ONE row, at one height, for every operation:
/// `swatch · state dot · name · tool · eye · …`.
///
/// It carried thirteen elements when selected and eight at rest. The four
/// that only READ were loud and always on screen, and the six that DID
/// something were 12-point glyphs that appeared only on hover (Pattern B).
/// Rule B inverts that: the state is one quiet dot, and every action is in
/// the `…` menu, which is always visible. The right-click menu keeps the
/// same list.
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
    // F2.2 — the one state the dot reads. It replaces the
    // `ComputeStatus::effective` call that used to stand here: A/M11's "one
    // taxonomy" rule is unchanged and `enabled` still wins over everything,
    // but `FreshnessState` folds that in itself (`Disabled` is its first
    // arm) and adds the one state `ComputeStatus` cannot express —
    // generated, then edited. A card with no runtime entry at all has never
    // been generated.
    let freshness = rt.map_or(&FreshnessState::NoResult, |r| &r.freshness);
    let has_result = rt.is_some_and(|r| r.has_result);
    let dim = !tc.enabled || !visible;

    // Read every session-derived value HERE. The row body borrows
    // `state.viewport` mutably, so it cannot also hold a `&AppState`.
    let tool_summary = state
        .session
        .tools()
        .iter()
        .find(|t| t.id == ToolId(tc.tool_id))
        .map(|tool| tool.summary());
    let rest = match tc.operation {
        crate::state::toolpath::OperationConfig::Rest(ref rest_cfg) => {
            Some(rest_badge(state, rest_cfg, tp_id))
        }
        _ => None,
    };

    let pc = palette_color(global_idx);
    let swatch_color = tokens::from_linear_rgb(pc);

    let border_color = if selected {
        swatch_color
    } else {
        tokens::HAIRLINE
    };

    let inner_response = egui::Frame::default()
        .fill(if selected {
            theme::CARD_FILL_SELECTED
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::new(1.0_f32, border_color))
        .inner_margin(tokens::SPACE_1)
        .corner_radius(tokens::RADIUS_SM)
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
                events.push(AppEvent::Ui(UiCommand::Select(Selection::Toolpath(tp_id))));
            }

            // The one row. Rule D: the height does not vary with the
            // content, so every card in the queue is the same height.
            ui.horizontal(|ui| {
                ui.set_min_height(tokens::ROW_ACTION);
                draw_swatch(ui, tp_id, swatch_color);
                draw_state_dot(ui, freshness, tp_id, events);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button("\u{2026}", |ui| {
                        card_menu(
                            ui,
                            tp_id,
                            visible,
                            tc.enabled,
                            has_result,
                            &mut state.viewport,
                            events,
                        );
                    });
                    if state.viewport.show_all_toolpaths {
                        draw_eye(ui, tp_id, visible, events);
                    }

                    // The name and the tool take the width the controls
                    // leave.
                    let tool = tool_summary.as_deref();
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        draw_name_and_tool(ui, &tc.name, tool, rest, dim);
                    });
                });
            });

            // Right-click carries the same list. It costs no screen space,
            // so it stays BESIDE the `…` menu rather than instead of it.
            card_resp.context_menu(|ui| {
                card_menu(
                    ui,
                    tp_id,
                    visible,
                    tc.enabled,
                    has_result,
                    &mut state.viewport,
                    events,
                );
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
            egui::Stroke::new(2.0_f32, theme::ACCENT),
        );
    }
}

/// The colour swatch, which is also the card's drag grip (R26).
///
/// The card opened with a 10-point grip glyph beside a 6-point swatch: two
/// rectangles, one job each. The swatch takes the drag now, so the thing the
/// operator grabs is the thing that names the row in the viewport.
fn draw_swatch(ui: &mut egui::Ui, tp_id: ToolpathId, swatch_color: egui::Color32) {
    let size = egui::vec2(tokens::SPACE_3, tokens::SPACE_5);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::drag());
    let radius = egui::CornerRadius::from(tokens::RADIUS_SM);
    ui.painter().rect_filled(rect, radius, swatch_color);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if resp.dragged() {
        egui::DragAndDrop::set_payload(ui.ctx(), tp_id);
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    resp.on_hover_text("Drag to reorder this operation, or to move it to another setup.");
}

/// The card's one state indicator (R24).
///
/// The card carried a `StatusChip`, a `Sim` button and a generate button,
/// and one `FreshnessState` drove all three. The dot carries the state, the
/// word arrives on hover, and a click generates when the state asks for
/// generation.
fn draw_state_dot(
    ui: &mut egui::Ui,
    freshness: &FreshnessState,
    tp_id: ToolpathId,
    events: &mut Vec<AppEvent>,
) {
    // The mapping is a pure function, so the sentry can drive every state
    // without a `Ui` (F2.2).
    let (status_text, status_role, hover) = status_chip(freshness);
    // F2.2: driven by freshness, not `ComputeStatus::needs_generation()`,
    // which answers `false` for `Done` — and an edited operation keeps
    // `Done`, so the route was closed on precisely the card whose whole
    // message is "regenerate me".
    let needs_generation = !matches!(
        freshness,
        FreshnessState::Current | FreshnessState::Regenerating | FreshnessState::Disabled
    );
    let sense = if needs_generation {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let size = egui::vec2(tokens::SPACE_4, tokens::SPACE_4);
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if matches!(freshness, FreshnessState::Regenerating) {
        // The operator asked for a spinner on anything that generates.
        ui.put(rect, egui::Spinner::new().size(tokens::SPACE_3));
    } else {
        // SPACE_2 is the RADIUS here, so the dot is the 8 points R24 asks
        // for.
        ui.painter()
            .circle_filled(rect.center(), tokens::SPACE_2, status_role.text());
    }
    let mut text = match hover {
        Some(message) => format!("{status_text} \u{2014} {message}"),
        None => status_text.to_owned(),
    };
    if needs_generation {
        text.push_str("\nClick to generate this operation.");
    }
    let resp = resp.on_hover_text(text);
    if resp.clicked() {
        events.push(AppEvent::GenerateToolpath(tp_id));
    }
}

/// The eye is available only while the viewport draws all toolpaths.
fn draw_eye(ui: &mut egui::Ui, tp_id: ToolpathId, visible: bool, events: &mut Vec<AppEvent>) {
    let glyph = if visible { "\u{1F441}" } else { "\u{2298}" };
    let colour = if visible {
        tokens::TEXT_BODY
    } else {
        tokens::TEXT_FAINT
    };
    let size = egui::vec2(tokens::ROW_DENSE, tokens::ROW_DENSE);
    let button = egui::Button::new(egui::RichText::new(glyph).color(colour))
        .frame(false)
        .min_size(size);
    let hover = if visible {
        "Hide this toolpath in the 3D viewport."
    } else {
        "Show this toolpath again in the 3D viewport."
    };
    if ui.add(button).on_hover_text(hover).clicked() {
        events.push(AppEvent::Ui(UiCommand::ToggleToolpathVisibility(tp_id)));
    }
}

/// The operation name and its tool, truncated and never wrapped.
///
/// A wrapped name is what made two cards different heights, which Rule D
/// forbids. The rest-dependency badge sits between them, because it
/// qualifies the operation rather than the tool.
fn draw_name_and_tool(
    ui: &mut egui::Ui,
    name: &str,
    tool_summary: Option<&str>,
    rest: Option<RestBadge>,
    dim: bool,
) {
    let text_color = if dim {
        tokens::TEXT_FAINT
    } else {
        tokens::TEXT_STRONG
    };
    let name_text = egui::RichText::new(name).color(text_color);
    ui.add(egui::Label::new(name_text).truncate());
    if let Some(badge) = rest {
        draw_rest_badge(ui, badge);
    }
    if let Some(summary) = tool_summary {
        let tool_text = egui::RichText::new(summary)
            .small()
            .color(tokens::TEXT_MUTED);
        ui.add(egui::Label::new(tool_text).truncate());
    }
}

/// Everything one operation card can do, as one list.
///
/// The `…` button and the right-click menu both build from this function, so
/// the two lists cannot drift apart. Rule B: no action on this card is
/// hover-only, and no action lost its route when the hover row went away.
fn card_menu(
    ui: &mut egui::Ui,
    tp_id: ToolpathId,
    visible: bool,
    enabled: bool,
    has_result: bool,
    viewport: &mut ViewportState,
    events: &mut Vec<AppEvent>,
) {
    if ui.button("Generate").clicked() {
        events.push(AppEvent::GenerateToolpath(tp_id));
        ui.close();
    }
    if has_result && ui.button("Inspect in Simulation").clicked() {
        events.push(AppEvent::Ui(UiCommand::InspectToolpathInSimulation(tp_id)));
        ui.close();
    }

    ui.separator();

    if viewport.show_all_toolpaths {
        let vis_label = if visible { "Hide" } else { "Show" };
        if ui.button(vis_label).clicked() {
            events.push(AppEvent::Ui(UiCommand::ToggleToolpathVisibility(tp_id)));
            ui.close();
        }
    }
    draw_move_visibility_items(ui, tp_id, viewport);

    ui.separator();

    // The operator ruled the enable toggle into this menu. The CONTROL is
    // buried; the STATE is not — a disabled operation dims its name on the
    // card and its dot takes the Unknown role. Nobody walks to the machine
    // unaware that an operation is off.
    let en_label = if enabled { "Disable" } else { "Enable" };
    let en_hover = if enabled {
        "Disable this operation. Generation, simulation and output all skip it."
    } else {
        "Enable this operation."
    };
    if ui.button(en_label).on_hover_text(en_hover).clicked() {
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
}

/// The per-toolpath cutting and rapid move filters, as two check items (R23).
///
/// DC1 asked for these to move to the Overlays panel. They cannot:
/// `ViewportState::toolpath_move_visibility` is keyed per TOOLPATH and the
/// Overlays rows are global, so that move would delete the capability rather
/// than relocate it.
///
/// The global toggles gate these, so a per-toolpath filter does nothing
/// while its global toggle is off. The item greys out, and its disabled
/// hover names the control that blocks it (P4-004).
pub(crate) fn draw_move_visibility_items(
    ui: &mut egui::Ui,
    tp_id: ToolpathId,
    viewport: &mut ViewportState,
) {
    // Read the global flags before the mutable entry borrow.
    let global_cutting = viewport.show_cutting;
    let global_rapids = viewport.show_rapids;
    let entry = viewport.toolpath_move_visibility.entry(tp_id).or_default();

    let cut = ui.add_enabled(
        global_cutting,
        egui::Checkbox::new(&mut entry.show_cutting, "Cutting moves"),
    );
    if !global_cutting {
        cut.on_disabled_hover_text(
            "Cutting moves are hidden globally (Overlays \u{25B8} Toolpath \u{25B8} \
             Cutting moves). Enable them there to use this per-toolpath filter.",
        );
    }

    let rapid = ui.add_enabled(
        global_rapids,
        egui::Checkbox::new(&mut entry.show_rapids, "Rapid moves"),
    );
    if !global_rapids {
        rapid.on_disabled_hover_text(
            "Rapid moves are hidden globally (Overlays \u{25B8} Toolpath \u{25B8} \
             Rapids). Enable them there to use this per-toolpath filter.",
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
/// The card draws the colour as one dot and the word on hover since R24, so
/// the word no longer appears on screen at rest. The mapping itself is
/// unchanged, and it stays public so the DC1 sentry can drive all seven
/// states from outside the crate.
///
/// F2.2: driven by [`FreshnessState`], not `ComputeStatus`. The two agree on
/// six of seven; the seventh is the point. An operation whose inputs moved
/// after it was generated still carries `ComputeStatus::Done` — the
/// generation that produced the drawn geometry really did finish — so this
/// chip used to read a confident green `OK` over geometry the project can no
/// longer reproduce.
pub fn status_chip(
    freshness: &FreshnessState,
) -> (&'static str, crate::ui::components::Role, Option<&str>) {
    use crate::ui::components::Role;
    match freshness {
        // UP4: PEND moves to UNKNOWN, which is what the plan called for.
        // OFF joins it, and that is a ruling: both mean "there is no verdict
        // here", the em dash is right for both, and they were only ever
        // separated by 20 units of grey. The WORD separates them now, which
        // is what §2.6 rule 3 asks for.
        FreshnessState::NoResult => ("PEND", Role::Unknown, None),
        FreshnessState::Regenerating => ("GEN", Role::Caution, None),
        FreshnessState::Current => ("OK", Role::Ok, None),
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
            Role::Caution,
            Some(
                "Inputs changed after this was generated. The path drawn in the viewport \
                 and the figures below are from the PREVIOUS generation, not from the \
                 settings now in the project. Regenerate.",
            ),
        ),
        // A/M11: WAIT is a sequencing state, not a failure — it must not
        // read as red. Hover names the blocking op.
        FreshnessState::WaitingOnUpstream(block) => {
            ("WAIT", Role::Caution, Some(block.message.as_str()))
        }
        FreshnessState::Disabled => ("OFF", Role::Unknown, None),
        FreshnessState::Error(msg) => ("ERR", Role::Danger, Some(msg.as_str())),
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

    // DC2 — the label is one character. The menu's own items say what
    // each one adds, so the word "Add" repeated it.
    ui.menu_button("+", |ui| {
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
            events.push(AppEvent::Ui(UiCommand::Select(Selection::Setup(setup_id))));
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
///
/// R25 deleted the `MAN` and the `TRACE` badge from the card. This one
/// stays: a rest dependency is a real, per-operation relationship, and it
/// changes what the operation cuts. `MAN` is a property of the operation
/// TYPE, so every 3D card carried it and it separated nothing.
///
/// The caller resolves the badge with [`rest_badge`] before it enters the
/// row, because the row body holds a mutable borrow of the viewport and
/// cannot also hold the `&AppState` that predicate needs.
fn draw_rest_badge(ui: &mut egui::Ui, badge: RestBadge) {
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
