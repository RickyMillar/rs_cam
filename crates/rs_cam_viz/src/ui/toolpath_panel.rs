use std::collections::HashMap;
use std::sync::Arc;

use rs_cam_core::session::dependencies::{self, EdgeKind, EdgeState};

use super::AppEvent;
use crate::render::toolpath_render::palette_color;
use crate::state::AppState;
use crate::state::freshness::{FreshnessState, freshness};
use crate::state::job::{SetupId, ToolId};
use crate::state::selection::Selection;
use crate::state::toolpath::{OperationType, ToolpathId};
use crate::state::viewport::ViewportState;
use crate::ui::components::Role;
use crate::ui::theme;
use crate::ui::tokens;
use crate::ui_command::UiCommand;

/// How heavy a connector draws.
///
/// Two points, never a hairline. The first drawing used a 1 point line in
/// `HAIRLINE` grey for a ready edge, and the operator's verdict on screen
/// was "once it's done, you can see nothing" (2026-09-18).
const CONNECTOR_WIDTH: f32 = 2.0;

/// How heavy a BROKEN connector draws.
///
/// Stroke width is the THIRD channel of section 2.6 rule 3, after colour and
/// the dash pattern. The gutter cannot hold a glyph, so a broken edge says
/// "fault" by being heavier as well as red and dashed.
const CONNECTOR_BROKEN_WIDTH: f32 = 2.5;

/// How long one dash and one gap of an unready connector are.
const CONNECTOR_DASH: f32 = tokens::SPACE_1;

/// The size of the arrowhead that lands in the dependent row.
const CONNECTOR_ARROW: f32 = tokens::SPACE_2;

/// The column the connectors run down, as the drop zone's left margin.
///
/// Twelve points, not eight. The line leaves the source row's swatch and
/// arrives at the dependent row's swatch, so it needs room for a 2 point
/// line beside a 2.5 point one without either touching a card border.
const CONNECTOR_GUTTER: f32 = tokens::SPACE_4;

/// How heavy the simulating ring draws.
const RING_WIDTH: f32 = 1.5;

/// Minimal snapshot of a `ToolpathConfig` with just the fields the card reads.
/// Cloning this releases the `state.session` borrow so `state.viewport` can be
/// borrowed mutably inside the card body.
struct CardInfo {
    id: rs_cam_core::ToolpathId,
    name: String,
    enabled: bool,
    tool_id: usize,
}

/// Minimal snapshot of `ToolpathRuntime` fields the card needs.
///
/// Built for EVERY row, including a row with no runtime entry at all. A
/// never-generated operation that machines the remaining stock is exactly
/// the pending connector the gutter exists to draw, and an `Option` here
/// dropped it.
struct RuntimeSnapshot {
    visible: bool,
    has_result: bool,
    /// The one state every surface should read (R0.1 §4.4). Derived
    /// here, beside the core result cache the derivation needs, because the
    /// card body no longer holds the session borrow.
    freshness: FreshnessState,
    /// Every dependency this row declares, read forward from the core's
    /// [`dependencies::primary_edges`]. Never stored; re-read each frame
    /// beside `freshness`. The MCP `depends_on` row reads the same door
    /// (W5), so the card and the wire cannot disagree.
    edges_in: Vec<EdgeRow>,
    /// What a worker is doing to this row now. A SEPARATE read from
    /// `freshness`, which keeps its seven arms.
    in_flight: Option<InFlight>,
}

/// One dependency, as the connector draws it.
///
/// The three [`EdgeKind`] arms carry what the Rest card's `dep` badge used
/// to say about `PrevTool` alone: `Ready` is the old `Resolved`, `Pending`
/// the old `Stale`, `Broken` the old `Missing` (R3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeRow {
    /// The source row, or `None` when the declaration resolves to no
    /// toolpath. A `None` source is always [`EdgeState::Broken`].
    pub source: Option<ToolpathId>,
    pub kind: EdgeKind,
    pub state: EdgeState,
}

/// What a worker is doing to one row right now.
///
/// **Not a [`FreshnessState`] arm.** Freshness is derived from the core
/// result cache, and "a worker is chewing on this" is a lane fact the cache
/// cannot express. The seven state function keeps its seven arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InFlight {
    /// The toolpath lane owns this row.
    Generating,
    /// The analysis lane is simulating a run that covers this row.
    Simulating,
}

/// What the generation plan is doing, reduced to the two facts a row needs.
///
/// `simulating_upto` is a setup POSITION, not a setup id: a prefix
/// simulation covers every enabled operation in setups `0..=position`, so
/// the row question is a comparison of positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlanFocus {
    /// The operation the plan generates now.
    pub generating: Option<ToolpathId>,
    /// The last setup position the plan's simulation step covers.
    pub simulating_upto: Option<usize>,
}

/// The facts about one row that the in-flight question reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowFacts {
    pub id: ToolpathId,
    /// Position of this row's setup in the project's setup order.
    pub setup_position: usize,
    pub enabled: bool,
    /// The GUI holds geometry for this row, so a simulation carves it.
    pub has_result: bool,
}

/// Is a worker chewing on this row? Pure, so the sentry drives it.
///
/// **Precedence, one line:** `Generating` beats `Simulating` beats the dot.
///
/// The generating arm comes from the freshness state, which already folds
/// the toolpath lane, or from the plan naming this row as its Generate step.
/// The simulating arm cannot come from freshness: the analysis lane is
/// project wide and the result cache says nothing about it. It reads the
/// plan's own simulation step first, and otherwise a running lane plus the
/// rows that run carves.
///
/// The lane guard is deliberate. A claim that outlives its run still draws
/// no ring, because the lane is quiet. The residual failure is a MISSING
/// ring, never a stuck one.
#[must_use]
pub fn in_flight(
    freshness: &FreshnessState,
    plan: PlanFocus,
    analysis_simulating: bool,
    row: RowFacts,
) -> Option<InFlight> {
    if matches!(freshness, FreshnessState::Regenerating) || plan.generating == Some(row.id) {
        return Some(InFlight::Generating);
    }
    if !row.enabled {
        return None;
    }
    // The plan's own prefix simulation covers setups `0..=position`.
    if plan
        .simulating_upto
        .is_some_and(|upto| row.setup_position <= upto)
    {
        return Some(InFlight::Simulating);
    }
    // A simulation started outside a plan carves what the project holds.
    if analysis_simulating && row.has_result {
        return Some(InFlight::Simulating);
    }
    None
}

/// What the operation panel needs that `AppState` does not hold.
///
/// One context struct, so the signature grows once and not per feature.
/// Neither the compute lanes nor the generation plan is mirrored onto
/// `AppState`: a mirrored copy is a second store, which is the defect class
/// this sweep closes.
pub struct PanelContext {
    /// The analysis lane is running a SIMULATION, not a collision check.
    pub analysis_simulating: bool,
    /// Where the plan in flight has got to, or `None` when none runs.
    pub plan: Option<crate::controller::generate_all::GenerationPlanProgress>,
    /// The question a plan waits on. The button is disabled while it
    /// stands, and this is the hover that says why.
    pub pending_confirm: Option<String>,
}

/// The plan's position, resolved against this project's setup order.
fn plan_focus(
    plan: Option<&crate::controller::generate_all::GenerationPlanProgress>,
    session: &rs_cam_core::session::ProjectSession,
) -> PlanFocus {
    use crate::controller::generate_all::Activity;

    let Some(plan) = plan else {
        return PlanFocus::default();
    };
    match &plan.activity {
        Activity::Generating(id, _) => PlanFocus {
            generating: Some(*id),
            simulating_upto: None,
        },
        Activity::Simulating(setup, _) => PlanFocus {
            generating: None,
            simulating_upto: session.find_setup_by_id(setup.0).map(|(at, _)| at),
        },
    }
}

/// Where one drawn row landed this frame.
///
/// The connector needs BOTH rects. It leaves the source row's swatch and it
/// lands beside the dependent row's swatch, so a card rect alone cannot
/// place either end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowGeometry {
    /// The card's own frame.
    pub card: egui::Rect,
    /// The colour swatch inside it, which is also the drag grip.
    pub swatch: egui::Rect,
}

/// Left panel for the Toolpath workspace: the operation queue.
pub fn draw(
    ui: &mut egui::Ui,
    state: &mut AppState,
    ctx: &PanelContext,
    events: &mut Vec<AppEvent>,
) {
    // DC2 — the heading and its rule leave the panel. A panel inside a
    // workspace tab named "Toolpaths" does not need a second word for the
    // same idea, and the rule separated that word from nothing. The one
    // action this panel is FOR takes the space, at the full section width.
    ui.add_space(tokens::SPACE_2);
    draw_generate_all(ui, ctx, events);

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

    // W3 - every dependency in the project, read ONCE per frame. Per card
    // this walk is O(n^2) and buys nothing. `primary_edges` keeps the
    // nearest enabled source per (consumer, kind), which is exactly the one
    // line per row the gutter draws.
    let mut edges: HashMap<ToolpathId, Vec<EdgeRow>> = HashMap::new();
    for edge in dependencies::primary_edges(&state.session) {
        let row = EdgeRow {
            source: edge.on,
            kind: edge.kind,
            state: dependencies::state(&edge, &state.session),
        };
        edges.entry(edge.from).or_default().push(row);
    }
    let focus = plan_focus(ctx.plan.as_ref(), &state.session);
    let names: HashMap<ToolpathId, String> = state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| (tc.id, tc.name.clone()))
        .collect();
    // Where each drawn card landed THIS frame. A local, so it cannot
    // outlive the layout it describes, and a second pass in the SAME frame,
    // because the panel sits in a scroll area and a lagged map is wrong by
    // the scroll delta on every scrolling frame.
    let mut row_rects: Vec<(ToolpathId, RowGeometry, Vec<EdgeRow>)> = Vec::new();

    for (setup_position, (setup_id, setup_name, toolpath_indices)) in
        setups_data.into_iter().enumerate()
    {
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

        // Drop zone for this setup. W3 - the gutter is this zone's LEFT
        // margin, so every card rect starts CONNECTOR_GUTTER right of the
        // zone and the connectors own the space to their left. The card's
        // own inner margin is 2 points, which cannot hold a 2.5 point line
        // without touching the border, so the column lives outside the
        // frame. `compute_drop_index` reads the pointer Y only, so drag and
        // drop does not notice.
        //
        // SAFETY: CONNECTOR_GUTTER is 12.0, which is exact in i8.
        #[allow(clippy::cast_possible_truncation)]
        let gutter = CONNECTOR_GUTTER as i8;
        let drop_frame = egui::Frame::default().inner_margin(egui::Margin {
            left: gutter,
            right: 2,
            top: 2,
            bottom: 2,
        });
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
                };
                let rt = state.gui.toolpath_rt.get(&card.id);
                let row_freshness =
                    freshness(tc_src, rt, state.session.get_result(tp_idx).is_some());
                let has_result = rt.is_some_and(|r| r.result.is_some());
                let flight = in_flight(
                    &row_freshness,
                    focus,
                    ctx.analysis_simulating,
                    RowFacts {
                        id: card.id,
                        setup_position,
                        enabled: card.enabled,
                        has_result,
                    },
                );
                let rt_snap = RuntimeSnapshot {
                    visible: rt.is_none_or(|r| r.visible),
                    has_result,
                    freshness: row_freshness,
                    edges_in: edges.remove(&card.id).unwrap_or_default(),
                    in_flight: flight,
                };
                let geometry = draw_toolpath_card(ui, state, events, &card, &rt_snap, i, local_idx);
                row_rects.push((card.id, geometry, rt_snap.edges_in));
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

    // W3 - the second pass, in the same frame as the cards it joins. A
    // cross-setup edge needs a rect recorded inside an EARLIER setup's drop
    // zone, which a per-zone pass cannot see.
    draw_connectors(ui, &row_rects, &names, events);

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

/// The one action this panel is FOR, and the plan's ONE progress surface.
///
/// While a plan runs the button says where it has got to and carries a bar
/// along its bottom edge, and a click cancels. No new panel, and the five
/// second status line no longer carries plan progress, so the plan is
/// reported in one place.
///
/// The button is never disabled while a plan runs: it IS the cancel, and a
/// disabled control with no route would leave a long plan unstoppable from
/// the panel. It IS disabled while a plan waits on the resolution question,
/// because a second plan would be refused, and the hover names the question.
fn draw_generate_all(ui: &mut egui::Ui, ctx: &PanelContext, events: &mut Vec<AppEvent>) {
    use crate::controller::generate_all::Activity;

    // The literal survives as the idle label: the DC1 sentry anchors on it,
    // and a fully dynamic label would make every arm of that scan pass for
    // the wrong reason.
    let mut label = "Generate All".to_owned();
    if let Some(plan) = ctx.plan.as_ref() {
        match &plan.activity {
            Activity::Generating(_, name) => {
                label.push_str(&format!(" \u{00B7} {}/{} {name}", plan.step, plan.of));
            }
            Activity::Simulating(_, setup) => {
                label.push_str(&format!(" \u{00B7} simulating {setup}"));
            }
        }
    }

    let mut button = crate::ui::components::Button::primary(label)
        .min_width(ui.available_width())
        .enabled(ctx.pending_confirm.is_none());
    if let Some(plan) = ctx.plan.as_ref() {
        // SAFETY: `of` is clamped to 1, so the ratio is finite and in 0..=1,
        // and both counts are plan step positions, far inside f32.
        #[allow(clippy::cast_precision_loss)]
        let fraction = plan.step as f32 / plan.of.max(1) as f32;
        button = button.progress(fraction);
    }
    let response = ui.add(button);

    if let Some(question) = ctx.pending_confirm.as_deref() {
        response.on_disabled_hover_text(question);
        return;
    }
    let Some(plan) = ctx.plan.as_ref() else {
        if response
            .on_hover_text("Generate every enabled operation, in dependency order.")
            .clicked()
        {
            events.push(AppEvent::GenerateAll);
        }
        return;
    };
    if plan.cancellable {
        if response.on_hover_text("Click to stop the plan.").clicked() {
            events.push(AppEvent::CancelGeneration);
        }
    } else {
        // A step that cannot be stopped swallows the click rather than
        // raising an event the handler would drop.
        response.on_hover_text("This step cannot be stopped. It finishes first.");
    }
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
    rt: &RuntimeSnapshot,
    global_idx: usize,
    _local_idx: usize,
) -> RowGeometry {
    let tp_id = tc.id;
    let selected = state.selection == Selection::Toolpath(tp_id);
    let visible = rt.visible;
    // F2.2 — the one state the dot reads. It replaces the
    // `ComputeStatus::effective` call that used to stand here: A/M11's "one
    // taxonomy" rule is unchanged and `enabled` still wins over everything,
    // but `FreshnessState` folds that in itself (`Disabled` is its first
    // arm) and adds the one state `ComputeStatus` cannot express —
    // generated, then edited. A card with no runtime entry at all has never
    // been generated.
    let freshness = &rt.freshness;
    let has_result = rt.has_result;
    // The ring's hover names what the run carves for THIS row, so it needs
    // to know whether the row takes its stock from the ops above it.
    let has_stock_edge = rt.edges_in.iter().any(|e| e.kind == EdgeKind::Stock);
    let flight = rt.in_flight;
    let dim = !tc.enabled || !visible;

    // Read every session-derived value HERE. The row body borrows
    // `state.viewport` mutably, so it cannot also hold a `&AppState`.
    let tool_summary = state
        .session
        .tools()
        .iter()
        .find(|t| t.id == ToolId(tc.tool_id))
        .map(|tool| tool.summary());
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
            let swatch = ui
                .horizontal(|ui| {
                    ui.set_min_height(tokens::ROW_ACTION);
                    let swatch = draw_swatch(ui, tp_id, swatch_color);
                    draw_state_dot(
                        ui,
                        freshness,
                        flight,
                        tp_id,
                        &tc.name,
                        has_stock_edge,
                        events,
                    );

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
                            draw_name_and_tool(ui, &tc.name, tool, dim);
                        });
                    });
                    swatch
                })
                .inner;

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
            swatch
        });

    let swatch = inner_response.inner;
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

    RowGeometry {
        card: inner_response.rect,
        swatch,
    }
}

/// The colour swatch, which is also the card's drag grip (R26).
///
/// The card opened with a 10-point grip glyph beside a 6-point swatch: two
/// rectangles, one job each. The swatch takes the drag now, so the thing the
/// operator grabs is the thing that names the row in the viewport.
fn draw_swatch(ui: &mut egui::Ui, tp_id: ToolpathId, swatch_color: egui::Color32) -> egui::Rect {
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
    rect
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
    flight: Option<InFlight>,
    tp_id: ToolpathId,
    name: &str,
    has_stock_edge: bool,
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
    draw_state_glyph(ui, rect, flight, status_role.text());
    let mut text = match hover {
        Some(message) => format!("{status_text} \u{2014} {message}"),
        None => status_text.to_owned(),
    };
    // One hover carries the state AND the activity, so the row keeps one
    // indicator, one place and one word (R24).
    if flight == Some(InFlight::Simulating) {
        if has_stock_edge {
            text.push_str(&format!("\nSimulating \u{00B7} stock for {name}"));
        } else {
            text.push_str("\nSimulating \u{00B7} this operation carves the stock");
        }
    }
    if needs_generation {
        text.push_str("\nClick to generate this operation.");
    }
    let resp = resp.on_hover_text(text);
    if resp.clicked() {
        events.push(AppEvent::GenerateToolpath(tp_id));
    }
}

/// The one glyph the state box draws: a spinner, a ring, or the dot.
///
/// Three forms in one box of one size, so the card's height never varies
/// with its state (Rule D), and the ring REPLACES the dot rather than
/// standing beside it (R24, one indicator).
///
/// Simulating is a different SHAPE, not a second spinner colour. §2.6 rule 3
/// says colour is never the only channel, and two spinners in two colours
/// make it the only one.
pub fn draw_state_glyph(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    flight: Option<InFlight>,
    dot_colour: egui::Color32,
) {
    match flight {
        // The operator asked for a spinner on anything that generates.
        Some(InFlight::Generating) => {
            ui.put(rect, egui::Spinner::new().size(tokens::SPACE_3));
        }
        Some(InFlight::Simulating) => draw_sim_ring(ui, rect),
        // SPACE_2 is the RADIUS here, so the dot is the 8 points R24 asks
        // for.
        None => {
            ui.painter()
                .circle_filled(rect.center(), tokens::SPACE_2, dot_colour);
        }
    }
}

/// A hollow ring with a rotating gap: the simulating form (R7).
///
/// egui 0.34 has no arc primitive, so the ring is a sampled polyline, which
/// is the pattern the crate already uses for its diagrams. The clock is the
/// frame time rather than an `animate_*` helper: those interpolate towards a
/// target, and a rotation has none. §5 exempts the spinner row from the 200
/// ms cap by naming it "continuous".
fn draw_sim_ring(ui: &mut egui::Ui, rect: egui::Rect) {
    /// 270 degrees, so the gap is a quarter turn.
    const SWEEP: f32 = std::f32::consts::FRAC_PI_2 * 3.0;
    const SEGMENTS: usize = 24;

    let centre = rect.center();
    let radius = tokens::SPACE_2;
    // SAFETY: the clock is reduced to one turn first, so the cast is exact
    // to well inside f32 precision however long the session has run.
    #[allow(clippy::cast_possible_truncation)]
    let start = ui.input(|i| i.time.rem_euclid(1.0)) as f32 * std::f32::consts::TAU;
    let points: Vec<egui::Pos2> = (0..=SEGMENTS)
        .map(|k| {
            // SAFETY: k and SEGMENTS are both at most 24, so both casts are
            // exact.
            #[allow(clippy::cast_precision_loss)]
            let along = k as f32 / SEGMENTS as f32;
            let angle = start + along * SWEEP;
            egui::pos2(
                centre.x + radius * angle.cos(),
                centre.y + radius * angle.sin(),
            )
        })
        .collect();
    // INFO is the accent. A ring is not a verdict, and §2.6 gives INFO to
    // "informational".
    ui.painter().add(egui::Shape::line(
        points,
        egui::Stroke::new(RING_WIDTH, tokens::INFO),
    ));
    // The Spinner does this, and the frame loop repaints while any lane is
    // active. Both, so the ring cannot stall if either route changes.
    ui.ctx().request_repaint();
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
/// forbids. R3 took the rest-dependency badge out from between them: the
/// gutter connector says the same three things for every dependency kind,
/// and the card loses two words at rest.
fn draw_name_and_tool(ui: &mut egui::Ui, name: &str, tool_summary: Option<&str>, dim: bool) {
    let text_color = if dim {
        tokens::TEXT_FAINT
    } else {
        tokens::TEXT_STRONG
    };
    let name_text = egui::RichText::new(name).color(text_color);
    ui.add(egui::Label::new(name_text).truncate());
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

// ── the gutter connector (W3, R3, R10) ──────────────────────────────────

/// The role one edge state carries.
///
/// Ready is `Ok`, and it DRAWS as `Ok`: a green line. The first drawing read
/// a ready edge as structure rather than a verdict and gave it a grey
/// hairline, which the operator could not see at all on screen. A dependency
/// that is satisfied is a thing the operator wants confirmed, so it takes
/// the pass colour like any other verdict (ruling, 2026-09-18).
#[must_use]
pub fn edge_role(state: EdgeState) -> Role {
    match state {
        EdgeState::Ready => Role::Ok,
        EdgeState::Pending => Role::Caution,
        EdgeState::Broken => Role::Danger,
    }
}

/// How one connector draws: a colour, a width, and a dash.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectorStroke {
    pub colour: egui::Color32,
    pub width: f32,
    /// The dash and gap length, or `None` for a solid line.
    pub dash: Option<f32>,
}

/// The stroke for one edge state.
///
/// The three states are the three verdict colours. **Colour is not the only
/// channel**: §2.6 rule 3 asks for a glyph beside every colour, and a twelve
/// point gutter cannot hold one legibly. A ready edge is the only SOLID one,
/// which is the second channel, and the stroke width is the third. The WORD
/// still reaches the operator, through the connector's hover and through the
/// row's own state dot.
///
/// No arm is thinner than [`CONNECTOR_WIDTH`]. A line the operator cannot
/// see reports nothing.
#[must_use]
pub fn edge_stroke(state: EdgeState) -> ConnectorStroke {
    match state {
        EdgeState::Ready => ConnectorStroke {
            colour: Role::Ok.text(),
            width: CONNECTOR_WIDTH,
            dash: None,
        },
        EdgeState::Pending => ConnectorStroke {
            colour: Role::Caution.text(),
            width: CONNECTOR_WIDTH,
            dash: Some(CONNECTOR_DASH),
        },
        EdgeState::Broken => ConnectorStroke {
            colour: Role::Danger.text(),
            width: CONNECTOR_BROKEN_WIDTH,
            dash: Some(CONNECTOR_DASH),
        },
    }
}

/// What one connector says on hover.
///
/// Pure, so the sentry drives all nine `(kind, state)` pairs. `selectable`
/// adds the click line, and it is false when the source is not drawn: there
/// is no row to select.
#[must_use]
pub fn edge_hover(
    source_name: Option<&str>,
    kind: EdgeKind,
    state: EdgeState,
    selectable: bool,
) -> String {
    let source = source_name.unwrap_or("an operation that is gone");
    let mut text = match (kind, state) {
        (EdgeKind::Stock, EdgeState::Ready) => format!("After {source}"),
        (EdgeKind::Stock, EdgeState::Pending) => {
            format!("After {source} \u{00B7} waiting for simulation")
        }
        (EdgeKind::Stock, EdgeState::Broken) => {
            format!("After {source} \u{00B7} no upstream stock")
        }
        (EdgeKind::Regions, EdgeState::Ready) => format!("Rest regions of {source}"),
        (EdgeKind::Regions, EdgeState::Pending) => {
            format!("Rest regions of {source} \u{00B7} {source} is not current")
        }
        (EdgeKind::Regions, EdgeState::Broken) => {
            format!("Rest regions of {source} \u{00B7} source missing")
        }
        (EdgeKind::PrevTool, EdgeState::Ready) => format!("Rest after {source}"),
        (EdgeKind::PrevTool, EdgeState::Pending) => {
            format!("Rest after {source} \u{00B7} {source} is not current")
        }
        // The one arm that names no source: there is no earlier enabled
        // operation with that cutter, which is what the static validator
        // refuses on (G-RESTBADGE).
        (EdgeKind::PrevTool, EdgeState::Broken) => {
            "No previous operation with that tool".to_owned()
        }
    };
    if selectable {
        text.push_str("\nClick to select it.");
    }
    text
}

/// The one edge a row draws, when it declares several.
///
/// A rest operation can declare both a Stock edge and a Regions edge. One
/// line per row keeps the gutter one column wide and R32's "one row"
/// honest, so the WORSE state wins, ranked by `Role::severity_rank`.
#[must_use]
pub fn worst_edge(rows: &[EdgeRow]) -> Option<EdgeRow> {
    rows.iter()
        .min_by_key(|row| edge_role(row.state).severity_rank())
        .copied()
}

/// Where one connector runs, source first.
///
/// One polyline, so every corner joins:
///
/// 1. the bottom centre of the SOURCE row's swatch, which is the colour the
///    operator already reads that row by;
/// 2. left, into the gutter column;
/// 3. down that column to the dependent row;
/// 4. right, stopping one arrowhead short of the dependent row's swatch.
///
/// The caller paints the arrowhead at [`arrow_tip`], so the line points at
/// the row it feeds and the direction needs no word.
///
/// A `None` source is a source that is not DRAWN: a collapsed setup, a
/// filtered list, or a row the session no longer holds. It starts one
/// gutter above the row instead, which is the stub, and the caller adds the
/// up glyph.
///
/// A chain A to B to C draws two paths that share the column, and the second
/// starts at B's own swatch, so the two meet end to end.
#[must_use]
pub fn connector_path(source: Option<RowGeometry>, target: RowGeometry) -> ConnectorPath {
    let x = target.card.left() - CONNECTOR_GUTTER / 2.0;
    let y = target.swatch.center().y;
    let end = egui::pos2(arrow_tip(target).x - CONNECTOR_ARROW, y);
    let foot = egui::pos2(x, y);
    match source {
        Some(source) => {
            let head = egui::pos2(x, source.swatch.bottom());
            ConnectorPath {
                points: vec![
                    egui::pos2(source.swatch.center().x, source.swatch.bottom()),
                    head,
                    foot,
                    end,
                ],
                column: (head, foot),
                stub: None,
            }
        }
        None => {
            let head = egui::pos2(x, y - CONNECTOR_GUTTER);
            ConnectorPath {
                points: vec![head, foot, end],
                column: (head, foot),
                stub: Some(head),
            }
        }
    }
}

/// One connector's geometry, with the parts the painter needs by NAME.
///
/// Named rather than indexed: the path is three points for a stub and four
/// for a line whose source is on screen, and a painter that counts from
/// either end is a painter that will be wrong about one of them.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorPath {
    /// The polyline, source end first.
    pub points: Vec<egui::Pos2>,
    /// The top and the bottom of the VERTICAL run, which is the only part
    /// of the path the operator has to aim at: the two horizontal runs sit
    /// inside the rows they join.
    pub column: (egui::Pos2, egui::Pos2),
    /// Where the up glyph goes, when the source is not drawn.
    pub stub: Option<egui::Pos2>,
}

/// Where the connector's arrowhead lands: just left of the row's swatch.
#[must_use]
pub fn arrow_tip(target: RowGeometry) -> egui::Pos2 {
    egui::pos2(
        target.swatch.left() - tokens::SPACE_1 / 2.0,
        target.swatch.center().y,
    )
}

/// The filled triangle that lands in the dependent row.
fn arrow_head(tip: egui::Pos2) -> Vec<egui::Pos2> {
    let half = CONNECTOR_ARROW / 2.0;
    vec![
        tip,
        egui::pos2(tip.x - CONNECTOR_ARROW, tip.y - half),
        egui::pos2(tip.x - CONNECTOR_ARROW, tip.y + half),
    ]
}

/// Draw every row's dependency as one line in the gutter.
///
/// The second pass of the frame, after the cards, because a line needs both
/// rects. Painted in ROW order, sources first, so a ready green goes UNDER a
/// pending amber on a shared span: the overlapping span then reads amber,
/// which is the honest answer.
fn draw_connectors(
    ui: &mut egui::Ui,
    row_rects: &[(ToolpathId, RowGeometry, Vec<EdgeRow>)],
    names: &HashMap<ToolpathId, String>,
    events: &mut Vec<AppEvent>,
) {
    // The card rects move while one is dragged, so a line drawn now is
    // wrong as well as ugly. The insertion indicator owns the picture.
    if egui::DragAndDrop::has_payload_of_type::<ToolpathId>(ui.ctx()) {
        return;
    }
    for (row_id, target, row_edges) in row_rects {
        let Some(edge) = worst_edge(row_edges) else {
            continue;
        };
        let source = edge.source.and_then(|source| {
            row_rects
                .iter()
                .find(|(id, _, _)| *id == source)
                .map(|(_, geometry, _)| *geometry)
        });
        let path = connector_path(source, *target);
        let spec = edge_stroke(edge.state);
        let stroke = egui::Stroke::new(spec.width, spec.colour);
        match spec.dash {
            None => {
                ui.painter()
                    .add(egui::Shape::line(path.points.clone(), stroke));
            }
            Some(dash) => {
                ui.painter()
                    .extend(egui::Shape::dashed_line(&path.points, stroke, dash, dash));
            }
        }
        // The arrowhead is always solid, whatever the line does: it says
        // which row the dependency feeds, and a dashed arrow says that less
        // well without saying anything more.
        ui.painter().add(egui::Shape::convex_polygon(
            arrow_head(arrow_tip(*target)),
            spec.colour,
            egui::Stroke::NONE,
        ));
        if let Some(anchor) = path.stub {
            // The stub ends in an up glyph: the source is above, and it is
            // not in this list.
            ui.painter().text(
                anchor,
                egui::Align2::CENTER_BOTTOM,
                "\u{2191}",
                egui::FontId::proportional(tokens::SPACE_4),
                spec.colour,
            );
        }

        let mut hover = edge_hover(
            edge.source
                .and_then(|id| names.get(&id))
                .map(String::as_str),
            edge.kind,
            edge.state,
            source.is_some(),
        );
        if edge.source.is_some() && source.is_none() {
            hover.push_str("\nIt is not in this list.");
        }
        // The operator aims at the vertical run.
        let (head, foot) = path.column;
        let hit = egui::Rect::from_x_y_ranges(
            (foot.x - tokens::SPACE_2)..=(foot.x + tokens::SPACE_2),
            head.y.min(foot.y)..=head.y.max(foot.y),
        );
        let response = ui
            .interact(
                hit,
                egui::Id::new("tp_edge").with(row_id.0),
                egui::Sense::click(),
            )
            .on_hover_text(hover);
        if response.clicked()
            && let Some(source_id) = edge.source
            && source.is_some()
        {
            events.push(AppEvent::Ui(UiCommand::Select(Selection::Toolpath(
                source_id,
            ))));
        }
    }
}
