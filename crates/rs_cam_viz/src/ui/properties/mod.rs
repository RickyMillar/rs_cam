mod feeds_speeds;
mod linking_dressup;
mod machine_panel;
mod model_sim_panels;
pub mod operations;
mod panel_apply;
mod pills;
pub mod post;
pub mod setup;
pub mod stock;
mod tab_badges;
pub mod tool;
mod toolpath_panel;

pub use operations::{
    DepthBeyondStock, ThroughCut, ToolpathValidationContext, bottom_z_pin_note,
    collect_diagnostics, depth_beyond_stock, draw_height_diagram, profile_through_cut,
    profile_through_cut_line, validate_toolpath, validate_toolpath_config,
};
pub(crate) use panel_apply::{apply_fixture_draft, apply_stock_draft, commit_tool_draft};

// The children of this module. Every name below is a private re-export: it
// keeps the item's path at `ui::properties::<name>`, which is what a
// descendant such as `operations/boundary_2d.rs` (`super::super::dv`) or
// `pills.rs` (`super::prov_from_chipload`) resolves through.
use feeds_speeds::prov_from_chipload;
use linking_dressup::{depth_caution_row, dv, dv_pill, p, through_cut_row};
use machine_panel::draw_machine_panel;
use model_sim_panels::{draw_model_properties, draw_simulation_panel};
use panel_apply::{
    apply_keep_out_draft, apply_panel_command, apply_setup_draft, flush_machine_snapshot,
    flush_post_snapshot, flush_tool_draft, flush_toolpath_snapshot,
};
use toolpath_panel::draw_toolpath_panel;

use crate::state::AppState;
use crate::state::selection::Selection;
use crate::state::toolpath::{BoundarySource, ToolpathEntry, ToolpathId};
use crate::ui::AppEvent;

/// Candidate source toolpath for a `BoundarySource::DerivedRestRegions`
/// picker: (id, display name, whether its cached result already has
/// non-empty rest regions ready to use, and — when ready — the regions
/// themselves). The regions are captured here rather than re-fetched later
/// so the Machining Boundary panel can run
/// [`rs_cam_core::surface::rest_field::classify_rest_regions`] against the SOURCE's
/// regions (sliver-storm / giant-region warning, 2026-07-06 incident)
/// without new session/runtime plumbing.
/// The trailing `Option<f64>` is the SOURCE toolpath's covered XY footprint
/// (mm²), read off its own rest grid — the honest denominator for the
/// giant-region share (`MEASUREMENT_DOMAINS.md` LH-2). `None` when that
/// toolpath carries no rest grid: **not measured**, which classifies as
/// silence (C2 — it used to be spelled `0.0`).
type BoundaryRestCandidate = (
    ToolpathId,
    String,
    bool,
    Option<std::sync::Arc<Vec<rs_cam_core::polygon::Polygon2>>>,
    Option<f64>,
);

/// The part's covered XY footprint (mm²) measured on a rest grid — the
/// denominator [`rs_cam_core::surface::rest_field::classify_rest_regions`] requires.
/// `None` when there is no grid to measure it on.
fn rest_grid_footprint_area(
    grid: Option<&rs_cam_core::surface::rest_field::RestGrid>,
) -> Option<f64> {
    grid.map(rs_cam_core::surface::rest_field::RestGrid::covered_footprint_area_mm2)
}

/// What one frame of a scratch-copy panel did to its draft (WP6).
///
/// An immediate-mode panel writes a CLONE of the project record and
/// applies one `Command` when an edit finishes. This is what the panel
/// reports back, so the caller knows when to apply and when the draft
/// may be dropped.
///
/// Plan §19 ruling 8 sets the two commit rules. A `DragValue` or a
/// `Slider` applies on release or on lost focus, never per frame: each
/// frame of a drag used to push an event that dropped every cached
/// result. A checkbox, a combo, a button and a text field apply on
/// `changed()`, which is one event per operator action already.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanelEdit {
    /// A widget wrote the draft this frame.
    pub changed: bool,
    /// An edit FINISHED this frame, so the caller applies its command.
    pub committed: bool,
    /// A widget of this panel is still dragged or focused. The draft
    /// must survive the frame; `false` lets the caller drop it and read
    /// the session again, which is how an edit from another surface
    /// reaches the panel.
    pub in_flight: bool,
}

impl PanelEdit {
    /// Read a `DragValue` or a `Slider` response.
    pub fn drag(&mut self, response: &egui::Response) {
        self.changed |= response.changed();
        self.committed |= response.drag_stopped() || response.lost_focus();
        self.in_flight |= response.dragged() || response.has_focus();
    }

    /// Read a checkbox, combo, button or text-field response.
    pub fn click(&mut self, response: &egui::Response) {
        self.changed |= response.changed();
        self.committed |= response.changed();
        self.in_flight |= response.has_focus();
    }

    /// Record a finished edit a `Response` cannot report — a helper that
    /// answers `bool`, or a deferred structural change.
    pub fn commit(&mut self) {
        self.changed = true;
        self.committed = true;
    }

    /// Fold in what a sub-panel reported.
    pub fn merge(&mut self, other: Self) {
        self.changed |= other.changed;
        self.committed |= other.committed;
        self.in_flight |= other.in_flight;
    }
}

pub fn draw(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    // Show simulation panel only when in the Simulation workspace
    if state.workspace == crate::state::Workspace::Simulation && state.simulation.has_results() {
        draw_simulation_panel(ui, state, events);
        return;
    }

    // Flush pending undo snapshots when selection changes away from a tracked panel.
    flush_tool_draft(state);
    flush_post_snapshot(state);
    flush_machine_snapshot(state);
    flush_toolpath_snapshot(state);

    match state.selection.clone() {
        Selection::None => {
            if state.session.models().is_empty()
                && state
                    .session
                    .list_setups()
                    .iter()
                    .all(|s| s.toolpath_indices.is_empty())
            {
                ui.label(
                    egui::RichText::new("Getting started:")
                        .strong()
                        .color(crate::ui::tokens::TEXT_STRONG),
                );
                ui.add_space(4.0);
                ui.label("1. Import a model (File > Import)");
                ui.label("2. Configure stock dimensions");
                ui.label("3. Add a cutting tool");
                ui.label("4. Create a toolpath");
                ui.label("5. Generate toolpaths");
                // G-WSMENU (2026-09-10): step 5 used to read "Generate and
                // export G-code", which walks a first-time operator from a
                // generated path straight to the machine. Simulation is
                // where collisions, air cutting and the tool-load gates are
                // measured, so it is a step of its own and it comes first.
                ui.label("6. Simulate and review");
                ui.label("7. Export G-code");
            } else {
                ui.label(
                    // Not "the project tree": no panel of that name exists
                    // (IA/CURRENT_MAP.md §1). Name the four things that can
                    // actually be selected (G-WSMENU, 2026-09-10).
                    egui::RichText::new("Select an operation, tool, setup or model")
                        .italics()
                        .color(crate::ui::tokens::TEXT_FAINT),
                );
            }
        }
        Selection::Stock => {
            // Capture snapshot for undo before editing
            if state.history.stock_snapshot.is_none() {
                state.history.stock_snapshot = Some(state.session.stock_config().clone());
            }
            let has_flipped_setup = state
                .session
                .list_setups()
                .iter()
                .any(|s| s.face_up != crate::state::job::FaceUp::Top);
            // WP6: the widgets write a DRAFT, not the session. The draft
            // is re-read from the session on any frame no widget of this
            // panel is dragged or focused, so an edit made on another
            // surface reaches the panel instead of being overwritten.
            let mut draft = match state.history.stock_draft.take() {
                Some(draft) => draft,
                None => state.session.stock_config().clone(),
            };
            let edit = stock::draw(ui, &mut draft, has_flipped_setup, events);
            if edit.committed {
                apply_stock_draft(state, draft.clone());
                // The undo compare runs AFTER the command, so the
                // session already holds the new value and the comparison
                // is true exactly once. It used to compare a session the
                // panel had already written in place.
                if let Some(old) = state.history.stock_snapshot.take()
                    && old != *state.session.stock_config()
                {
                    state
                        .history
                        .push(crate::state::history::UndoAction::StockChange {
                            old,
                            new: state.session.stock_config().clone(),
                        });
                }
            }
            if edit.in_flight {
                state.history.stock_draft = Some(draft);
            }
        }
        Selection::PostProcessor => {
            // Capture snapshot for undo before editing
            if state.history.post_snapshot.is_none() {
                state.history.post_snapshot = Some(state.gui.post.clone());
            }
            let stock_top = state.session.stock_config().origin_z + state.session.stock_config().z;
            post::draw(ui, &mut state.gui.post, stock_top);
            // W2.2 [P1-004]: the session is canonical for post config. Push the
            // edited gui.post straight through so `session.post_config()` can't
            // lag behind the panel (the stale window the GUI-vs-MCP race read).
            // Guarded on a real change: the panel must not write the
            // session on every idle frame. WP17 moved the second half of
            // this reason into the setter — `set_post_config` clears the
            // simulation only for a field that reaches emitted motion,
            // so an unchanged write clears nothing now either.
            let session_post = state.gui.post.clone();
            if *state.session.post_config() != session_post {
                let command = rs_cam_core::session::Command::SetPostConfig(
                    rs_cam_core::session::SetPostConfigArgs {
                        post: Box::new(session_post),
                    },
                );
                // UI-08: the Post tab is a panel door, so it marks the
                // project edited exactly as the other seven doors do. Without
                // this the operator changes Safe Z, closes the window, and the
                // `app.rs` close guard reads a clean project and asks nothing.
                // The compare above already gates on a real change, so an idle
                // frame still dirties nothing.
                if apply_panel_command(state, command) {
                    state.gui.mark_edited();
                }
            }
        }
        Selection::Machine => {
            // Capture snapshot for undo before editing
            if state.history.machine_snapshot.is_none() {
                state.history.machine_snapshot = Some(state.session.machine().clone());
            }
            draw_machine_panel(ui, state, events);
        }
        Selection::Model(id) => {
            draw_model_properties(ui, id, state, events);
        }
        Selection::Tool(id) => {
            // TOO-003 — draft-commit: edit a clone, commit on Apply (or on
            // navigate-away via flush_tool_draft), discard on Revert.
            if let Some(committed) = state.session.tools().iter().find(|t| t.id == id).cloned() {
                // (Re)initialise the draft when entering a different tool.
                if state.history.tool_draft.as_ref().map(|(d, _)| *d) != Some(id) {
                    state.history.tool_draft = Some((id, committed.clone()));
                }
                let modified = state
                    .history
                    .tool_draft
                    .as_ref()
                    .is_some_and(|(_, draft)| *draft != committed);
                let mut action = tool::ToolEditAction::None;
                // UI-09: the draft and the panel's own typed state are
                // disjoint fields of `AppState`, so both borrow at once.
                let panels = &mut state.panels;
                if let Some((_, draft)) = state.history.tool_draft.as_mut() {
                    action = tool::draw(ui, draft, modified, panels);
                }
                match action {
                    tool::ToolEditAction::Apply => {
                        if let Some((_, draft)) = state.history.tool_draft.clone() {
                            commit_tool_draft(state, id, draft);
                        }
                    }
                    tool::ToolEditAction::Revert => {
                        state.history.tool_draft = Some((id, committed));
                    }
                    tool::ToolEditAction::None => {}
                }
            } else {
                // Tool vanished (e.g. deleted while selected).
                state.history.tool_draft = None;
            }
        }
        Selection::Setup(setup_id) => {
            let pin_count = state.session.stock_config().alignment_pins.len();
            let has_flip_axis = state.session.stock_config().flip_axis.is_some();
            let all_models: Vec<_> = state
                .session
                .models()
                .iter()
                .map(|m| (crate::state::job::ModelId(m.id), m.name.clone()))
                .collect();
            // WP6: the panel writes a DRAFT. The caller compares the
            // draft against the stored setup and applies one command per
            // field group that moved.
            //
            // G-FRESHSTATE: the panel used to write `face_up` /
            // `z_rotation` straight into `SetupData`, so no core setter
            // ran and every toolpath in the setup kept a result
            // generated in the OLD frame. The datum and the model scope
            // had no core door at all.
            let stored = state
                .session
                .find_setup_by_id(setup_id.0)
                .map(|(index, setup)| (index, setup.clone()));
            if let Some((setup_index, stored)) = stored {
                let mut draft = match state.history.setup_draft.take() {
                    Some((id, data)) if id == setup_id => data,
                    _ => stored.clone(),
                };
                let edit = setup::draw(
                    ui,
                    setup_id,
                    &mut draft,
                    pin_count,
                    has_flip_axis,
                    &all_models,
                    events,
                );
                if edit.committed {
                    apply_setup_draft(state, setup_index, &stored, &draft);
                }
                if edit.in_flight {
                    state.history.setup_draft = Some((setup_id, draft));
                }
            }
        }
        Selection::Fixture(setup_id, fixture_id) => {
            let stored =
                state
                    .session
                    .find_setup_by_id(setup_id.0)
                    .and_then(|(index, setup_data)| {
                        setup_data
                            .fixtures
                            .iter()
                            .find(|fixture| fixture.id == fixture_id)
                            .map(|fixture| (index, fixture.clone()))
                    });
            if let Some((setup_index, stored)) = stored {
                let mut draft = match state.history.fixture_draft.take() {
                    Some((s_id, f_id, data)) if s_id == setup_id && f_id == fixture_id => data,
                    _ => stored.clone(),
                };
                let edit = setup::draw_fixture_properties(ui, setup_id, &mut draft);
                if edit.committed && draft != stored {
                    apply_fixture_draft(state, setup_index, fixture_id, draft.clone());
                }
                if edit.in_flight {
                    state.history.fixture_draft = Some((setup_id, fixture_id, draft));
                }
            }
        }
        Selection::KeepOut(setup_id, keep_out_id) => {
            let stored =
                state
                    .session
                    .find_setup_by_id(setup_id.0)
                    .and_then(|(index, setup_data)| {
                        setup_data
                            .keep_out_zones
                            .iter()
                            .find(|zone| zone.id == keep_out_id)
                            .map(|zone| (index, zone.clone()))
                    });
            if let Some((setup_index, stored)) = stored {
                let mut draft = match state.history.keep_out_draft.take() {
                    Some((s_id, z_id, data)) if s_id == setup_id && z_id == keep_out_id => data,
                    _ => stored.clone(),
                };
                let edit = setup::draw_keep_out_properties(ui, setup_id, &mut draft);
                if edit.committed && draft != stored {
                    apply_keep_out_draft(state, setup_index, keep_out_id, draft.clone());
                }
                if edit.in_flight {
                    state.history.keep_out_draft = Some((setup_id, keep_out_id, draft));
                }
            }
        }
        Selection::Face(model_id, face_id) => {
            ui.heading("Face Selected");
            ui.separator();
            let model_name = state
                .session
                .models()
                .iter()
                .find(|m| m.id == model_id.0)
                .map(|m| m.name.as_str())
                .unwrap_or("Unknown");
            ui.label(format!("Model: {model_name}"));
            ui.label(format!("Face: {}", face_id.0));
            if let Some(model) = state.session.models().iter().find(|m| m.id == model_id.0)
                && let Some(enriched) = &model.enriched_mesh
                && let Some(group) = enriched.face_group(face_id)
            {
                ui.label(format!("Surface type: {:?}", group.surface_type));
                ui.label(format!("Triangles: {}", group.triangle_range.len()));
            }
        }
        Selection::Faces(model_id, ref face_ids) => {
            ui.heading("Faces Selected");
            ui.separator();
            let model_name = state
                .session
                .models()
                .iter()
                .find(|m| m.id == model_id.0)
                .map(|m| m.name.as_str())
                .unwrap_or("Unknown");
            ui.label(format!("Model: {model_name}"));
            ui.label(format!("{} faces selected", face_ids.len()));
        }
        Selection::Toolpath(id) => {
            // Capture snapshot for undo before editing
            if state.history.toolpath_snapshot.is_none()
                && let Some((_, tc)) = state.session.find_toolpath_config_by_id(id)
            {
                state.history.toolpath_snapshot = Some((
                    id,
                    tc.operation.clone(),
                    tc.dressups.clone(),
                    tc.face_selection.clone(),
                    tc.feeds_provenance.clone(),
                ));
            }

            // Snapshot heights and boundary for the two side effects
            // below. WP5 deleted the operation snapshot beside them: the
            // operation is inside the generation-inputs signature, so the
            // command door reports it stale, and no side effect here
            // needs to know that it was the operation that moved.
            let heights_before = state
                .session
                .find_toolpath_config_by_id(id)
                .map(|(_, tc)| format!("{:?}", tc.heights));
            // P2.2: boundary source/containment/offset changes (including
            // picking a `DerivedRestRegions` source toolpath) also affect
            // the generated toolpath. The command door stamps them stale;
            // this snapshot drives the rest-analysis hook below.
            let boundary_before = state
                .session
                .find_toolpath_config_by_id(id)
                .map(|(_, tc)| format!("{:?}", tc.boundary));

            // One-shot tab override from the MCP set_ui_view tool. Consumed
            // only when the panel renders the override's TARGET toolpath —
            // a blind take() here used to fire on whatever toolpath rendered
            // first (workspace/selection changes from the same set_ui_view
            // call land on different frames), persisting the tab onto the
            // wrong toolpath. The tab bar persists the applied value via
            // the regular temp-memory path.
            // UI-06: the pending tab is already a `ToolpathTab`. The MCP
            // boundary parses the agent's key once, so no string reaches
            // here and no second key table can drift from the parser.
            let tab_override = match state.gui.pending_toolpath_tab {
                Some((target, tab)) if target == id => {
                    state.gui.pending_toolpath_tab = None;
                    Some(tab)
                }
                _ => None,
            };

            // Copied out and written back so the panel's `&mut bool` cannot
            // collide with the session borrows in the same argument list.
            let mut show_reach_map = state.viewport.show_reach_map;

            // UI-01: the panel's whole read side, assembled by one
            // function. The 21 arguments it replaces were built here by
            // hand, so a caller could substitute a default for one and
            // still get a panel that rendered.
            let inputs = toolpath_panel_inputs(
                id,
                &state.session,
                &state.gui,
                state
                    .simulation
                    .results
                    .as_ref()
                    .and_then(|r| r.cut_trace.as_deref()),
                tab_override,
            );

            // Build the temporary entry and its canonical session diagnostic
            // contexts together. Keeping them in one snapshot makes it
            // impossible for this production call chain to draw an entry while
            // silently omitting either static context.
            if let Some(mut snapshot) = toolpath_panel_snapshot(id, &state.session, &state.gui) {
                // The panel takes the whole snapshot. The entry, both
                // static diagnostic contexts and the model bbox travel as
                // one value, so no caller can draw the entry while
                // silently substituting a default for one of the others.
                draw_toolpath_panel(ui, &mut snapshot, &inputs, &mut show_reach_map, events);

                state.viewport.show_reach_map = show_reach_map;

                // ORDER IS LOAD-BEARING. The runtime write-back runs
                // FIRST, because it copies `entry.stale_since` — the
                // value read before the draw — back onto the runtime row.
                // The config write-back stamps the fresh value, so the
                // other order would erase the stamp on every edited
                // frame and leave the card green over dropped geometry.
                write_entry_runtime_to_gui(&snapshot.entry, &mut state.gui);
                // Write config changes back to the session, through the
                // one command door. The call stamps `stale_since` on
                // every index the core dropped, and dirties the project,
                // so no caller keeps a staleness model of its own.
                let _ = write_entry_config_to_session(&snapshot.entry, state);
            }

            // Two side effects need to know WHICH field moved, and
            // `Effects::stale` cannot say. Both fields are inside the
            // generation-inputs signature, so the command reports the
            // toolpath stale either way; these comparisons say which of
            // the two caused it (plan §17 ruling 4).
            //
            // Demand-driven rest-analysis producer hook (P2 pencil-panel
            // consolidation): captured here (while `tc` is borrowed) and
            // applied below, once the session borrow above is released —
            // see the comment on the follow-up block.
            let mut auto_enable_rest_source: Option<ToolpathId> = None;
            let mut heights_moved = false;
            if let Some((_, tc)) = state.session.find_toolpath_config_by_id(id) {
                let heights_changed = heights_before
                    .as_ref()
                    .is_some_and(|b| *b != format!("{:?}", tc.heights));
                let boundary_changed = boundary_before
                    .as_ref()
                    .is_some_and(|b| *b != format!("{:?}", tc.boundary));
                if heights_changed {
                    // Re-upload only. The heights edit itself already
                    // dirtied the project and dropped that toolpath's
                    // result. WP6 replaced the `HeightPlanesChanged`
                    // event with the flag the frame loop reads.
                    heights_moved = true;
                }
                if boundary_changed
                    && tc.boundary.enabled
                    && let BoundarySource::DerivedRestRegions { source_toolpath_id } =
                        &tc.boundary.source
                {
                    auto_enable_rest_source = Some(*source_toolpath_id);
                }
            }
            if heights_moved {
                state.panel_side_effects.upload = true;
            }
            // The GUI's boundary picker (Machining Boundary section, above)
            // writes `tc.boundary` through `write_entry_config_to_session`
            // rather than going through `session::set_boundary_config` — the
            // MCP entry point (`app/mcp/commands.rs`, the `SetBoundaryConfig`
            // arm of the describe step) is the
            // one caller of that setter. Run the same demand-driven producer
            // hook here so picking "Rest Regions" in the GUI has the same
            // effect: the source toolpath's rest analysis turns on and its
            // cached result invalidates, so it actually produces regions on
            // next generation.
            // WP3: the setter reports `Option<Effects>`. `None` says the
            // call changed nothing, which is the old `false`.
            //
            // WP15a: the row FOLDS that `None` into an empty `Effects`,
            // so the answer is `Ok` either way. `Effects::revision` is
            // the discriminator — the setter names the source index on
            // the arm that changed something, and the fold names none.
            if let Some(source_id) = auto_enable_rest_source {
                let command = rs_cam_core::session::Command::AutoEnableRestAnalysis(
                    rs_cam_core::session::AutoEnableRestAnalysisArgs { source_id },
                );
                if let Ok(effects) = state.session.apply(command)
                    && effects.revision.is_some()
                {
                    crate::state::stale::stamp_stale(state, &effects.stale);
                    state.gui.mark_edited();
                    // WP19 (plan §28). The gate stays on `revision`: the
                    // fold that changed nothing reports no revision AND
                    // clears nothing, so the two agree.
                    if effects.simulation_cleared {
                        state.panel_side_effects.invalidate_simulation = true;
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
// ── Toolpath property tab system ─────────────────────────────────────────
// The per-toolpath properties panel is chartered into five concern tabs
// (IA cleanup W3.2, FINAL_DESIGN §4): Geometry (what to cut), Feeds & Speeds
// (how fast — the SPEED/CUT split from W3.1), Linking (how moves connect:
// entry/exit, move optimization, retract), Heights (Z planes), and Dressup
// (edge work / path quality). Replaces the old [Params][Feeds][Heights][Dressups].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolpathTab {
    Geometry,
    FeedsSpeeds,
    Linking,
    Heights,
    Dressup,
}

impl ToolpathTab {
    pub const ALL: &[ToolpathTab] = &[
        ToolpathTab::Geometry,
        ToolpathTab::FeedsSpeeds,
        ToolpathTab::Linking,
        ToolpathTab::Heights,
        ToolpathTab::Dressup,
    ];

    fn label(self) -> &'static str {
        match self {
            ToolpathTab::Geometry => "Geometry",
            ToolpathTab::FeedsSpeeds => "Feeds & Speeds",
            ToolpathTab::Linking => "Linking",
            ToolpathTab::Heights => "Heights",
            ToolpathTab::Dressup => "Dressup",
        }
    }

    /// The canonical agent-facing key of this tab.
    ///
    /// UI-06: this is the ONE key table. The MCP `set_ui_view` tool builds
    /// its valid-key list from `ALL.map(key)`, so the list cannot disagree
    /// with what [`Self::parse`] accepts.
    pub fn key(self) -> &'static str {
        match self {
            ToolpathTab::Geometry => "geometry",
            ToolpathTab::FeedsSpeeds => "feeds",
            ToolpathTab::Linking => "linking",
            ToolpathTab::Heights => "heights",
            ToolpathTab::Dressup => "dressup",
        }
    }

    /// Parse the agent-facing tab key used by the MCP `set_ui_view` tool.
    ///
    /// Every [`Self::key`] parses back to its own variant. `feeds_speeds` is
    /// an extra spelling the tool has always accepted for the Feeds tab.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "geometry" => Some(ToolpathTab::Geometry),
            "feeds" | "feeds_speeds" => Some(ToolpathTab::FeedsSpeeds),
            "linking" => Some(ToolpathTab::Linking),
            "heights" => Some(ToolpathTab::Heights),
            "dressup" => Some(ToolpathTab::Dressup),
            _ => None,
        }
    }
}

/// Owned inputs assembled for the production toolpath-properties panel.
///
/// The entry and both static diagnostic contexts are captured together so a
/// caller cannot render a resolved toolpath while accidentally substituting
/// `None` for one of the contexts.
pub struct ToolpathPanelSnapshot {
    pub entry: ToolpathEntry,
    pub preconditions: rs_cam_core::diagnostics::diagnose::PreconditionContext,
    pub model_refs: rs_cam_core::diagnostics::diagnose::ModelRefContext,
    /// Q1: the bounding box of the model this toolpath machines, read
    /// once here through `ProjectSession::model_bbox`.
    ///
    /// The panel's two Suggest sites — the Geometry-tab pill funnel and
    /// the Feeds-tab card — put it in `SuggestContext::model_bbox`,
    /// which gates the runtime-sanity stepover back-off. Both sites
    /// passed `SuggestContext::default()` before, so the back-off read
    /// "no constraint signal" in the GUI while the controller and the
    /// MCP surfaces read the real box. `None` means the id names no
    /// model, or the model carries no finite geometry.
    pub model_bbox: Option<rs_cam_core::geo::BoundingBox3>,
}

/// Build the production panel snapshot from session config + GUI runtime.
///
/// This is public only so integration sentries can exercise the exact assembly
/// used by [`draw`], rather than manually recreating its contexts.
pub fn toolpath_panel_snapshot(
    id: crate::state::toolpath::ToolpathId,
    session: &rs_cam_core::session::ProjectSession,
    gui: &crate::state::runtime::GuiState,
) -> Option<ToolpathPanelSnapshot> {
    let (_, tc) = session.find_toolpath_config_by_id(id)?;
    let rt = gui.toolpath_rt.get(&id);
    let default_rt = crate::state::runtime::ToolpathRuntime::new(true);
    let rt = rt.unwrap_or(&default_rt);
    let entry = ToolpathEntry {
        id: tc.id,
        pill_stamped_fields: Vec::new(),
        name: tc.name.clone(),
        enabled: tc.enabled,
        visible: rt.visible,
        locked: rt.locked,
        tool_id: crate::state::job::ToolId(tc.tool_id),
        model_id: crate::state::job::ModelId(tc.model_id),
        operation: tc.operation.clone(),
        dressups: tc.dressups.clone(),
        heights: tc.heights.clone(),
        boundary: tc.boundary.clone(),
        rest_analysis: tc.rest_analysis.clone(),
        coolant: tc.coolant,
        pre_gcode: tc.pre_gcode.clone().unwrap_or_default(),
        post_gcode: tc.post_gcode.clone().unwrap_or_default(),
        stock_source: tc.stock_source,
        status: rt.status.clone(),
        result: rt.result.clone(),
        stale_since: rt.stale_since,
        auto_regen: rt.auto_regen,
        face_selection: tc.face_selection.clone(),
        feeds_result: rt.feeds_result.clone(),
        feeds_provenance: tc.feeds_provenance.clone(),
        planner_origin: tc.planner_origin.clone(),
        debug_options: tc.debug_options,
        debug_trace: rt.debug_trace.clone(),
        semantic_trace: rt.semantic_trace.clone(),
        debug_trace_path: rt.debug_trace_path.clone(),
    };
    Some(ToolpathPanelSnapshot {
        entry,
        preconditions: session.precondition_context_for_toolpath(tc),
        model_refs: session.model_ref_context_for_toolpath(tc),
        // Q1: the one place the panel's model bbox enters. The id is the
        // STORED `tc.model_id`, so the box is the box of the model this
        // toolpath machines.
        model_bbox: session.model_bbox(tc.model_id),
    })
}

/// Every read-only value the toolpath inspector draws from, assembled once.
///
/// UI-01: `draw_toolpath_panel` took 25 parameters, 21 of which were this
/// read side. A caller that assembles that list by hand can pass a default
/// for one and still get a panel that renders, so the list is a struct and
/// one function builds it. [`ToolpathPanelSnapshot`] beside it carries the
/// part the panel EDITS; this carries the part it only READS.
pub(crate) struct ToolpathPanelInputs {
    pub tools: Vec<(crate::state::job::ToolId, String, f64)>,
    pub models: Vec<(crate::state::job::ModelId, String)>,
    pub tool_configs: Vec<(crate::state::job::ToolId, crate::state::job::ToolConfig)>,
    pub boundary_source_candidates: Vec<BoundaryRestCandidate>,
    /// Names of toolpaths currently consuming THIS one's rest-depth analysis
    /// as a `DerivedRestRegions` machining boundary (P2 pencil-panel
    /// consolidation, §2) — empty when nothing depends on it yet.
    pub rest_region_consumers: Vec<String>,
    pub validation: ToolpathValidationContext,
    pub material: rs_cam_core::material::Material,
    pub machine: rs_cam_core::machine::MachineProfile,
    pub workholding: rs_cam_core::feeds::WorkholdingRigidity,
    pub spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    pub project_default_rpm: u32,
    pub model_has_enriched: bool,
    pub model_is_step_missing_brep: bool,
    pub height_ctx: Option<crate::state::toolpath::HeightContext>,
    pub stale_default_defects: Vec<rs_cam_core::compute::validate::StaleDefault>,
    pub load_verdict: Option<rs_cam_core::tool_load::ToolpathLoadVerdict>,
    pub tab_override: Option<ToolpathTab>,
    pub drill_layers: Vec<String>,
    pub drill_targets: Vec<rs_cam_core::io::dxf_input::DrillTarget>,
    /// What the panel prints beside the reach-map checkbox (P5).
    pub reach: ReachPanelSummary,
    /// F2.2 — the header prints this in place of the raw `ComputeStatus`,
    /// which cannot say "generated, then edited".
    pub freshness: crate::state::freshness::FreshnessState,
}

/// Assemble [`ToolpathPanelInputs`] from the session, the GUI store and the
/// two values that live outside both.
///
/// `sim_cut_trace` is the accepted run's cut trace, which lives on
/// `AppState::simulation`. `tab_override` is the one-shot MCP tab, which the
/// caller CONSUMES (it clears `GuiState::pending_toolpath_tab`) before
/// calling. Both are handed in so this function needs no `AppState`, which
/// is what lets a sentry build it from a `ProjectSession` alone.
pub(crate) fn toolpath_panel_inputs(
    id: crate::state::toolpath::ToolpathId,
    session: &rs_cam_core::session::ProjectSession,
    gui: &crate::state::runtime::GuiState,
    sim_cut_trace: Option<&rs_cam_core::stock::simulation_cut::SimulationCutTrace>,
    tab_override: Option<ToolpathTab>,
) -> ToolpathPanelInputs {
    // Snapshot tool/model lists to avoid borrow conflict with toolpaths
    let tools: Vec<_> = session
        .tools()
        .iter()
        .map(|t| (t.id, t.summary(), t.diameter))
        .collect();
    // NOT filtered by the owning setup's `model_ids` (empty =
    // all). W9 / P-2 gave that field a home on `SetupData` and
    // on the wire, so the operator's choice now survives a save
    // — but this dropdown still lists every model. Stated
    // rather than silently fixed: filtering here changes which
    // models a toolpath can be reassigned to, which is a UI
    // behaviour change P-2 was not scoped to make.
    let models: Vec<_> = session
        .models()
        .iter()
        .map(|m| (crate::state::job::ModelId(m.id), m.name.clone()))
        .collect();
    // Snapshot tool configs for feeds calculation
    let tool_configs: Vec<_> = session.tools().iter().map(|t| (t.id, t.clone())).collect();
    // Candidate source toolpaths for a `DerivedRestRegions` boundary
    // (P2.2): every other toolpath in the session, plus whether its
    // last cached result already has non-empty rest regions ready to
    // use. Toolpaths without a ready result are still selectable —
    // generation fails hard with a clear message if the source turns
    // out to have no usable rest regions.
    let boundary_source_candidates: Vec<BoundaryRestCandidate> = session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.id != id)
        .map(|tc| {
            let cached = gui
                .toolpath_rt
                .get(&tc.id)
                .and_then(|rt| rt.result.as_ref());
            let cached_regions = cached.and_then(|r| r.annotated.rest_regions.clone());
            // LH-2: the source's OWN rest-grid footprint travels with
            // its regions, so the pathology share has an honest
            // denominator without new session plumbing.
            let footprint_area =
                rest_grid_footprint_area(cached.and_then(|r| r.annotated.rest_grid.as_deref()));
            let ready = cached_regions
                .as_ref()
                .is_some_and(|regions| !regions.is_empty());
            (
                tc.id,
                tc.name.clone(),
                ready,
                cached_regions,
                footprint_area,
            )
        })
        .collect();

    // Names of toolpaths that consume THIS toolpath's rest-depth
    // analysis as their machining boundary (P2 pencil-panel
    // consolidation, §2/§3): drives the Rest Analysis section's
    // demand-driven auto-enable + "Producing rest regions for: ..."
    // label instead of a plain checkbox.
    let rest_region_consumer_names: Vec<String> = session
        .rest_region_consumers(id)
        .into_iter()
        .filter_map(|consumer_id| {
            session
                .find_toolpath_config_by_id(consumer_id)
                .map(|(_, tc)| tc.name.clone())
        })
        .collect();

    let validation = ToolpathValidationContext::from_session(session);
    let material = session.stock_config().material.clone();
    let machine = session.machine().clone();
    let workholding = session.stock_config().workholding_rigidity;

    // Check if the toolpath's model has enriched mesh (for face selection UI)
    let model_for_panel = session
        .find_toolpath_config_by_id(id)
        .and_then(|(_, tc)| session.models().iter().find(|m| m.id == tc.model_id));
    let model_has_enriched = model_for_panel
        .map(|m| m.enriched_mesh.is_some())
        .unwrap_or(false);
    // Defensive: if a STEP model loaded without BREP (e.g. older
    // project file or future loader regression), surface a warning
    // so the face picker isn't silently absent.
    let model_is_step_missing_brep = model_for_panel
        .map(|m| {
            m.kind == Some(rs_cam_core::compute::stock_config::ModelKind::Step)
                && m.enriched_mesh.is_none()
        })
        .unwrap_or(false);

    // (Removed 2026-07-29, LH-2.) The rest-region pathology caption
    // used to divide a rest-region area by the model's mesh XY
    // BOUNDING RECTANGLE — a mask mismatch that under-reads every
    // non-rectangular part by roughly its bbox fill ratio, so the
    // "single giant region" warning fired late or never. The
    // denominator is now each toolpath's own covered rest-grid
    // footprint (`rest_grid_footprint_area`), measured on the same
    // grid the regions came from. See `MEASUREMENT_DOMAINS.md` X-4.

    // Snapshot the toolpath model's drill targets + layers (DXF point /
    // circle-centre picking) for the drill-op panels.
    let drill_layers: Vec<String> = model_for_panel
        .map(|m| (*m.layers).clone())
        .unwrap_or_default();
    let drill_targets: Vec<rs_cam_core::io::dxf_input::DrillTarget> = model_for_panel
        .map(|m| (*m.drill_targets).clone())
        .unwrap_or_default();

    // Snapshot height context before mutable borrow. Use the shared
    // helper so model_top/bottom_z are in the setup-local frame.
    let height_ctx = session
        .find_toolpath_config_by_id(id)
        .map(|(_, tc)| crate::state::job::height_context_from_session(session, tc));
    // Pre-compute stale-default defects for this TP so the panel
    // can render the validator banner without needing a session
    // reference. Defects are recomputed each frame, so a Fix
    // click takes effect immediately on the next render.
    let stale_default_defects = session
        .find_toolpath_config_by_id(id)
        .map(|(_, tc)| {
            let tool = session
                .tools()
                .iter()
                .find(|t| t.id == rs_cam_core::compute::tool_config::ToolId(tc.tool_id));
            let stock_bottom_z = session.stock_config().origin_z;
            rs_cam_core::compute::validate::validate_one_toolpath(
                tc,
                tool,
                &session.stock_config().material,
                stock_bottom_z,
            )
        })
        .unwrap_or_default();

    // Compute the load verdict for this TP so the params panel can
    // surface chipload / power / deflection / drill-gate
    // diagnostics in the unified ribbon rather than only in the
    // separate tool-load surface.
    let load_report = rs_cam_core::gcode::project_load_report(session, sim_cut_trace);
    let load_verdict_for_tp = load_report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == id)
        .cloned();

    // P5 — the reach map for THIS toolpath, if the overlay is holding
    // one. An overlay pointed at another toolpath reads `Idle`: the
    // sweep has not caught up yet, and showing the other toolpath's
    // percentage here would be worse than showing none.
    let reach_summary = {
        let overlay = &gui.reach_overlay;
        if overlay.toolpath == Some(id) {
            match &overlay.status {
                crate::state::runtime::ReachStatus::Idle => ReachPanelSummary::Idle,
                crate::state::runtime::ReachStatus::Computing => ReachPanelSummary::Computing,
                crate::state::runtime::ReachStatus::Failed(message) => {
                    ReachPanelSummary::Failed(message.clone())
                }
                crate::state::runtime::ReachStatus::Ready(map) => {
                    if map.is_measured() {
                        ReachPanelSummary::Measured {
                            unreachable_pct: map.unreachable_pct(),
                            max_gap_mm: map.max_gap_mm,
                            grid_note: map.grid_note(),
                            area_basis_note: map.area_basis_note(),
                            over_statement_note: map.over_statement_note(),
                            tolerance_below_floor: map.tolerance_below_floor(),
                        }
                    } else {
                        ReachPanelSummary::NotMeasured
                    }
                }
            }
        } else {
            ReachPanelSummary::Idle
        }
    };
    // F2.2 — the one freshness state, derived here where the
    // session and the GUI store are both in hand. The panel takes a
    // `ToolpathEntry`, a snapshot, and cannot derive it itself.
    let freshness_for_tp = session
        .find_toolpath_config_by_id(id)
        .and_then(|(index, _)| crate::state::freshness::freshness_at(session, gui, index))
        .unwrap_or(crate::state::freshness::FreshnessState::NoResult);
    ToolpathPanelInputs {
        tools,
        models,
        tool_configs,
        boundary_source_candidates,
        rest_region_consumers: rest_region_consumer_names,
        validation,
        material,
        machine,
        workholding,
        spindle_strategy: session.post_config().spindle_strategy,
        project_default_rpm: session.post_config().spindle_speed,
        model_has_enriched,
        model_is_step_missing_brep,
        height_ctx,
        stale_default_defects,
        load_verdict: load_verdict_for_tp,
        tab_override,
        drill_layers,
        drill_targets,
        reach: reach_summary,
        freshness: freshness_for_tp,
    }
}

/// Test-only entry projection used by the inspector write-back sentries.
#[cfg(test)]
pub(crate) fn build_entry_from_session_and_gui(
    id: crate::state::toolpath::ToolpathId,
    session: &rs_cam_core::session::ProjectSession,
    gui: &crate::state::runtime::GuiState,
) -> Option<ToolpathEntry> {
    toolpath_panel_snapshot(id, session, gui).map(|snapshot| snapshot.entry)
}

/// The one line the Geometry tab prints above the boundary controls
/// (UX-R03-009, G-BOUNDARYINHERIT). It names the STORED `boundary.source`,
/// which is the boundary generation clips to. `auto` appends "(auto)" when
/// the caller can prove the controller assigned the source on add; the
/// panel cannot today (no provenance is stored), so it passes `false`.
pub fn boundary_summary_line(
    boundary: &crate::state::toolpath::BoundaryConfig,
    auto: bool,
) -> String {
    let source = match &boundary.source {
        BoundarySource::Stock => "stock rectangle".to_owned(),
        BoundarySource::ModelSilhouette => "model silhouette".to_owned(),
        BoundarySource::Geometry { polygon_indices } => {
            format!("imported geometry ({} polygons)", polygon_indices.len())
        }
        BoundarySource::FaceSelection => "face selection".to_owned(),
        BoundarySource::DerivedRestRegions { source_toolpath_id } => {
            format!("rest regions of toolpath {}", source_toolpath_id.0)
        }
        BoundarySource::PlannedTierRegions { .. } => "planned tier regions".to_owned(),
    };
    let suffix = if auto { " (auto)" } else { "" };
    format!("Boundary: {source}{suffix}")
}

/// Project the panel's owned entry onto the STORED toolpath config, and
/// report the result as the configuration the session should carry.
///
/// The projection CLONES the stored config and applies the entry's
/// sixteen fields onto the clone. It never builds a fresh literal.
/// `ToolpathConfig` carries nineteen fields, and `ToolpathEntry` cannot
/// supply three of them: `id` (the entry's own id names the row to write,
/// not a value to write), the retained boundary-inherit flag (no widget
/// reads or writes it, and the core field survives for project-file
/// compatibility — G-BOUNDARYINHERIT) and `planner_origin` (the
/// multi-tool planner owns it, and a duplicate must start hand-owned).
/// A fresh literal clobbers all three.
///
/// The boundary-inherit flag is named in prose here, not as the
/// identifier. The G-BOUNDARYINHERIT source sentry scans every line under
/// `src/ui` for that identifier and allows only a constant struct write.
///
/// `feeds_provenance` is derived here, not in core, because both passes
/// need the entry. The order is load-bearing and is the reason this stays
/// viz-side.
pub(crate) fn project_entry_onto(
    stored: &rs_cam_core::session::ToolpathConfig,
    entry: &ToolpathEntry,
) -> Box<rs_cam_core::session::ToolpathConfig> {
    let mut config = stored.clone();
    // W2.1: stamp Manual on any feeds dimension the user hand-edited in the
    // param widgets this frame (value moved but provenance didn't). It reads
    // the STORED operation and the STORED provenance, which the clone above
    // keeps whole until the field writes below run.
    let mut new_provenance = entry.feeds_provenance.clone();
    new_provenance.detect_manual_edits(
        &stored.operation,
        &entry.operation,
        &stored.feeds_provenance,
    );
    // G-PILLCLAMP: a per-field ⚡ pill wrote these fields this frame and
    // stamped the recommendation's provenance on the entry. When that
    // stamp equals the stored one (same row, value moved) the pass above
    // cannot tell it from a hand edit and relabels it Manual — restore
    // the pill's stamp, because the funnel produced the value.
    for field in &entry.pill_stamped_fields {
        if let Some(stamp) = entry.feeds_provenance.get(*field) {
            new_provenance.set(*field, stamp.clone());
        }
    }
    config.name = entry.name.clone();
    config.enabled = entry.enabled;
    config.tool_id = entry.tool_id.0;
    config.model_id = entry.model_id.0;
    config.operation = entry.operation.clone();
    config.dressups = entry.dressups.clone();
    config.heights = entry.heights.clone();
    config.boundary = entry.boundary.clone();
    config.rest_analysis = entry.rest_analysis.clone();
    config.coolant = entry.coolant;
    config.pre_gcode = if entry.pre_gcode.is_empty() {
        None
    } else {
        Some(entry.pre_gcode.clone())
    };
    config.post_gcode = if entry.post_gcode.is_empty() {
        None
    } else {
        Some(entry.post_gcode.clone())
    };
    config.stock_source = entry.stock_source;
    config.face_selection = entry.face_selection.clone();
    config.debug_options = entry.debug_options;
    config.feeds_provenance = new_provenance;
    Box::new(config)
}

/// Write config changes from a `ToolpathEntry` back to the session,
/// through the one command door.
///
/// The panel holds no commit event: it rebuilds the entry, draws it, and
/// calls this on EVERY frame it is open. So this runs every frame, and
/// `Command::ReplaceToolpathConfig` decides what survives. The command
/// always writes the configuration — name, coolant, the pre and post
/// G-code and the debug options land whatever else moved — and drops the
/// cached result plus everything downstream of it only when
/// `ToolpathConfig::generation_inputs_signature` moved.
///
/// G-FRESHSTATE: before that gate existed the panel wrote every field
/// through `find_toolpath_config_by_id_mut` and no core setter ran, so
/// the core went on holding — and export went on emitting — geometry
/// from the previous parameter set, and the downstream
/// `FromRemainingStock` chain was never invalidated (R0.1 §2.2 item 1).
///
/// WP5 widened what the GUI stamps. It stamped the edited toolpath alone,
/// so a downstream `FromRemainingStock` row the core had dropped stayed
/// green on screen and the auto-regeneration sweep never queued it. The
/// stamp now covers every index `Effects::stale` names.
///
/// `None` says the session carries no toolpath with the entry's id, which
/// is the only way the command can refuse here: the index comes from a
/// lookup in this function and nothing mutates the session between the
/// two.
pub(crate) fn write_entry_config_to_session(
    entry: &ToolpathEntry,
    state: &mut AppState,
) -> Option<rs_cam_core::session::Effects> {
    let (index, config) = {
        let (index, stored) = state.session.find_toolpath_config_by_id(entry.id)?;
        (index, project_entry_onto(stored, entry))
    };
    let command = rs_cam_core::session::Command::ReplaceToolpathConfig(
        rs_cam_core::session::ReplaceToolpathConfigArgs { index, config },
    );
    let effects = state.session.apply(command).ok()?;
    // An empty set is the frame the operator only LOOKED at the panel.
    // Stamping or dirtying there would mark a healthy project edited on
    // every frame (G-HEIGHTSTAB).
    if !effects.stale.is_empty() {
        crate::state::stale::stamp_stale(state, &effects.stale);
        state.gui.mark_edited();
        // WP19 (plan §28): a parameter edit clears the viewport's
        // simulation, as every other edit does. The raise stays INSIDE
        // this guard, so an idle frame raises nothing.
        if effects.simulation_cleared {
            state.panel_side_effects.invalidate_simulation = true;
        }
    }
    Some(effects)
}

/// Write runtime changes from a `ToolpathEntry` back to the GUI's `ToolpathRuntime`.
///
/// It copies `entry.stale_since`, which several widgets stamp while they
/// draw, so the panel MUST call it BEFORE `write_entry_config_to_session`.
/// The config write-back stamps the fresh value for every index the core
/// dropped; the other order would overwrite that with the value read
/// before the draw.
pub(crate) fn write_entry_runtime_to_gui(
    entry: &ToolpathEntry,
    gui: &mut crate::state::runtime::GuiState,
) {
    if let Some(rt) = gui.toolpath_rt.get_mut(&entry.id) {
        rt.visible = entry.visible;
        rt.locked = entry.locked;
        rt.auto_regen = entry.auto_regen;
        rt.status = entry.status.clone();
        rt.stale_since = entry.stale_since;
        rt.feeds_result = entry.feeds_result.clone();
        // result, debug_trace, semantic_trace, debug_trace_path are not
        // mutated by the panel — skip to avoid unnecessary Arc clones.
    }
}

/// What the toolpath panel prints beside the reach-map checkbox (P5).
///
/// Built by the caller from `GuiState::reach_overlay`, and owned rather than
/// borrowed so the panel can hold a `&mut` on the viewport flag at the same
/// time.
///
/// [`Self::NotMeasured`] is a separate arm from [`Self::Measured`] on
/// purpose. A reach map over a mesh with no upward-facing topmost triangle
/// reports `unreachable_fraction 0.0` — indistinguishable from a fully
/// reachable part unless the population is read first
/// (`ReachMap::is_measured`).
pub(crate) enum ReachPanelSummary {
    /// No answer is held and none is in flight.
    Idle,
    Computing,
    /// The walk found no surface to judge.
    NotMeasured,
    Measured {
        unreachable_pct: f64,
        max_gap_mm: f64,
        /// [`rs_cam_core::maps::reach_map::ReachMap::grid_note`] — the cell, the
        /// floor and the bar, plus the "the bar is under the floor" sentence
        /// where that applies. Printed under the percentage, because a
        /// percentage without its grid is not comparable with the next one
        /// (F1 / F5, 2026-09-08).
        grid_note: String,
        /// [`rs_cam_core::maps::reach_map::ReachMap::area_basis_note`] - the base
        /// every percentage on every surface owes beside it. Carried rather
        /// than rebuilt: this line printed "of MEASURED area" of its own
        /// while the panel legend printed the shared note, which is the
        /// fourth-surface drift the shared notes exist to stop.
        area_basis_note: String,
        /// [`rs_cam_core::maps::reach_map::ReachMap::over_statement_note`], shown
        /// when the bar is under the floor.
        over_statement_note: String,
        /// True when the tolerance is under
        /// [`rs_cam_core::maps::reach_map::ReachMap::discretisation_floor_mm`]. The
        /// grid's gap bias is NON-NEGATIVE (a minimum over a sampled set sits
        /// at or above the continuum minimum), so the percentage OVER-states
        /// and the truth is at or below it — see
        /// [`rs_cam_core::maps::reach_map::ReachMap::tolerance_below_floor`].
        tolerance_below_floor: bool,
    },
    Failed(String),
}

#[cfg(test)]
mod tests;
