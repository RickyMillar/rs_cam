mod feeds_speeds;
mod linking_dressup;
mod machine_panel;
mod model_sim_panels;
mod operations;
mod panel_apply;
mod pills;
pub mod post;
pub mod setup;
pub mod stock;
mod tab_badges;
pub mod tool;

pub use operations::{
    DepthBeyondStock, ThroughCut, ToolpathValidationContext, bottom_z_pin_note,
    collect_diagnostics, depth_beyond_stock, draw_height_diagram, profile_through_cut,
    profile_through_cut_line, validate_toolpath, validate_toolpath_config,
};
use operations::{
    StepoverPattern, draw_adaptive_params, draw_adaptive3d_params, draw_alignment_pin_drill_params,
    draw_chamfer_params, draw_drill_params, draw_dropcutter_params, draw_face_params,
    draw_heights_params, draw_horizontal_finish_params, draw_inlay_diagram, draw_inlay_params,
    draw_outline_diagram, draw_pencil_diagram, draw_pencil_params, draw_pocket_params,
    draw_point_set_diagram, draw_profile_params, draw_project_curve_params, draw_radial_diagram,
    draw_radial_finish_params, draw_ramp_finish_diagram, draw_ramp_finish_params, draw_rest_params,
    draw_scallop_params, draw_spiral_diagram, draw_spiral_finish_params,
    draw_steep_shallow_diagram, draw_steep_shallow_params, draw_stepover_diagram,
    draw_trace_params, draw_unified_finish_params, draw_vcarve_params, draw_waterline_params,
    draw_zigzag_params,
};
use pills::PillSuggestions;

use crate::state::AppState;
use crate::state::selection::Selection;
use crate::state::toolpath::{
    BoundaryContainment, BoundarySource, ComputeStatus, DressupConfig, HeightContext,
    OperationConfig, ProfileSide, SpiralDirection, ToolpathEntry, ToolpathId, TraceCompensation,
    UiProcessRole,
};
use crate::ui::AppEvent;

use feeds_speeds::{
    calculate_and_apply_feeds, draw_speed_controls, draw_vendor_lut_viewer, prov_from_chipload,
};

use tab_badges::{
    RowTier, compute_tab_badges, draw_geometry_wiring, draw_toolpath_tabs,
    merge_stateful_gate_rows, render_diagnostic_row, rest_region_pathology_caption,
    wrapped_small_label,
};

use linking_dressup::{
    depth_caution_row, draw_dressup_params, draw_linking_params, dressup_active_count, dv, dv_pill,
    through_cut_row,
};

use machine_panel::draw_machine_panel;

use model_sim_panels::{draw_model_properties, draw_simulation_panel};

pub(crate) use panel_apply::{apply_fixture_draft, apply_stock_draft, commit_tool_draft};
use panel_apply::{
    apply_keep_out_draft, apply_panel_command, apply_setup_draft, flush_machine_snapshot,
    flush_post_snapshot, flush_tool_draft, flush_toolpath_snapshot,
};

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
            let session_post = crate::state::runtime::GuiState::post_to_session(&state.gui.post);
            if *state.session.post_config() != session_post {
                let command = rs_cam_core::session::Command::SetPostConfig(
                    rs_cam_core::session::SetPostConfigArgs {
                        post: Box::new(session_post),
                    },
                );
                let _ = apply_panel_command(state, command);
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
                if let Some((_, draft)) = state.history.tool_draft.as_mut() {
                    action = tool::draw(ui, draft, modified);
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
            // Snapshot tool/model lists to avoid borrow conflict with toolpaths
            let tools: Vec<_> = state
                .session
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
            let models: Vec<_> = state
                .session
                .models()
                .iter()
                .map(|m| (crate::state::job::ModelId(m.id), m.name.clone()))
                .collect();
            // Snapshot tool configs for feeds calculation
            let tool_configs: Vec<_> = state
                .session
                .tools()
                .iter()
                .map(|t| (t.id, t.clone()))
                .collect();
            // Candidate source toolpaths for a `DerivedRestRegions` boundary
            // (P2.2): every other toolpath in the session, plus whether its
            // last cached result already has non-empty rest regions ready to
            // use. Toolpaths without a ready result are still selectable —
            // generation fails hard with a clear message if the source turns
            // out to have no usable rest regions.
            let boundary_source_candidates: Vec<BoundaryRestCandidate> = state
                .session
                .toolpath_configs()
                .iter()
                .filter(|tc| tc.id != id)
                .map(|tc| {
                    let cached = state
                        .gui
                        .toolpath_rt
                        .get(&tc.id)
                        .and_then(|rt| rt.result.as_ref());
                    let cached_regions = cached.and_then(|r| r.annotated.rest_regions.clone());
                    // LH-2: the source's OWN rest-grid footprint travels with
                    // its regions, so the pathology share has an honest
                    // denominator without new session plumbing.
                    let footprint_area = rest_grid_footprint_area(
                        cached.and_then(|r| r.annotated.rest_grid.as_deref()),
                    );
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
            let rest_region_consumer_names: Vec<String> = state
                .session
                .rest_region_consumers(id)
                .into_iter()
                .filter_map(|consumer_id| {
                    state
                        .session
                        .find_toolpath_config_by_id(consumer_id)
                        .map(|(_, tc)| tc.name.clone())
                })
                .collect();

            let validation = ToolpathValidationContext::from_session(&state.session);
            let material = state.session.stock_config().material.clone();
            let machine = state.session.machine().clone();
            let workholding = state.session.stock_config().workholding_rigidity;

            // Check if the toolpath's model has enriched mesh (for face selection UI)
            let model_for_panel = state
                .session
                .find_toolpath_config_by_id(id)
                .and_then(|(_, tc)| state.session.models().iter().find(|m| m.id == tc.model_id));
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
            let height_ctx = state
                .session
                .find_toolpath_config_by_id(id)
                .map(|(_, tc)| crate::state::job::height_context_from_session(&state.session, tc));

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

            // Pre-compute stale-default defects for this TP so the panel
            // can render the validator banner without needing a session
            // reference. Defects are recomputed each frame, so a Fix
            // click takes effect immediately on the next render.
            let stale_default_defects = state
                .session
                .find_toolpath_config_by_id(id)
                .map(|(_, tc)| {
                    let tool =
                        state.session.tools().iter().find(|t| {
                            t.id == rs_cam_core::compute::tool_config::ToolId(tc.tool_id)
                        });
                    let stock_bottom_z = state.session.stock_config().origin_z;
                    rs_cam_core::compute::validate::validate_one_toolpath(
                        tc,
                        tool,
                        &state.session.stock_config().material,
                        stock_bottom_z,
                    )
                })
                .unwrap_or_default();

            // Compute the load verdict for this TP so the params panel can
            // surface chipload / power / deflection / drill-gate
            // diagnostics in the unified ribbon rather than only in the
            // separate tool-load surface.
            let load_report = {
                let sim_trace = state
                    .simulation
                    .results
                    .as_ref()
                    .and_then(|r| r.cut_trace.as_deref());
                rs_cam_core::gcode::project_load_report(&state.session, sim_trace)
            };
            let load_verdict_for_tp = load_report
                .per_toolpath
                .iter()
                .find(|v| v.toolpath_id == id)
                .cloned();

            // One-shot tab override from the MCP set_ui_view tool. Consumed
            // only when the panel renders the override's TARGET toolpath —
            // a blind take() here used to fire on whatever toolpath rendered
            // first (workspace/selection changes from the same set_ui_view
            // call land on different frames), persisting the tab onto the
            // wrong toolpath. The tab bar persists the applied value via
            // the regular temp-memory path.
            let tab_override = match state.gui.pending_toolpath_tab.as_ref() {
                Some((target, tab)) if *target == id => {
                    let parsed = ToolpathTab::parse(tab);
                    state.gui.pending_toolpath_tab = None;
                    parsed
                }
                _ => None,
            };

            // P5 — the reach map for THIS toolpath, if the overlay is holding
            // one. An overlay pointed at another toolpath reads `Idle`: the
            // sweep has not caught up yet, and showing the other toolpath's
            // percentage here would be worse than showing none.
            let reach_summary = {
                let overlay = &state.gui.reach_overlay;
                if overlay.toolpath == Some(id) {
                    match &overlay.status {
                        crate::state::runtime::ReachStatus::Idle => ReachPanelSummary::Idle,
                        crate::state::runtime::ReachStatus::Computing => {
                            ReachPanelSummary::Computing
                        }
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
            let freshness_for_tp = state
                .session
                .find_toolpath_config_by_id(id)
                .and_then(|(index, _)| {
                    crate::state::freshness::freshness_at(&state.session, &state.gui, index)
                })
                .unwrap_or(crate::state::freshness::FreshnessState::NoResult);

            // Copied out and written back so the panel's `&mut bool` cannot
            // collide with the session borrows in the same argument list.
            let mut show_reach_map = state.viewport.show_reach_map;

            // Build the temporary entry and its canonical session diagnostic
            // contexts together. Keeping them in one snapshot makes it
            // impossible for this production call chain to draw an entry while
            // silently omitting either static context.
            if let Some(snapshot) = toolpath_panel_snapshot(id, &state.session, &state.gui) {
                let ToolpathPanelSnapshot {
                    mut entry,
                    preconditions,
                    model_refs,
                } = snapshot;
                draw_toolpath_panel(
                    ui,
                    &mut entry,
                    &tools,
                    &models,
                    &tool_configs,
                    &boundary_source_candidates,
                    &rest_region_consumer_names,
                    &validation,
                    &material,
                    &machine,
                    workholding,
                    state.session.post_config().spindle_strategy,
                    state.session.post_config().spindle_speed,
                    model_has_enriched,
                    model_is_step_missing_brep,
                    height_ctx.as_ref(),
                    &preconditions,
                    &model_refs,
                    &stale_default_defects,
                    load_verdict_for_tp.as_ref(),
                    tab_override,
                    &drill_layers,
                    &drill_targets,
                    &mut show_reach_map,
                    &reach_summary,
                    &freshness_for_tp,
                    events,
                );

                state.viewport.show_reach_map = show_reach_map;

                // ORDER IS LOAD-BEARING. The runtime write-back runs
                // FIRST, because it copies `entry.stale_since` — the
                // value read before the draw — back onto the runtime row.
                // The config write-back stamps the fresh value, so the
                // other order would erase the stamp on every edited
                // frame and leave the card green over dropped geometry.
                write_entry_runtime_to_gui(&entry, &mut state.gui);
                // Write config changes back to the session, through the
                // one command door. The call stamps `stale_since` on
                // every index the core dropped, and dirties the project,
                // so no caller keeps a staleness model of its own.
                let _ = write_entry_config_to_session(&entry, state);
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
#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolpathTab {
    Geometry,
    FeedsSpeeds,
    Linking,
    Heights,
    Dressup,
}

impl ToolpathTab {
    const ALL: &[ToolpathTab] = &[
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

    /// Parse the agent-facing tab key used by the MCP `set_ui_view` tool
    /// (stored in `GuiState::pending_toolpath_tab`).
    fn parse(s: &str) -> Option<Self> {
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
    })
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

#[allow(clippy::too_many_arguments)]
fn draw_toolpath_panel(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tools: &[(crate::state::job::ToolId, String, f64)],
    models: &[(crate::state::job::ModelId, String)],
    tool_configs: &[(crate::state::job::ToolId, crate::state::job::ToolConfig)],
    boundary_source_candidates: &[BoundaryRestCandidate],
    // Names of toolpaths currently consuming THIS one's rest-depth analysis
    // as a `DerivedRestRegions` machining boundary (P2 pencil-panel
    // consolidation, §2) — empty when nothing depends on it yet.
    rest_region_consumers: &[String],
    validation: &ToolpathValidationContext,
    material: &rs_cam_core::material::Material,
    machine: &rs_cam_core::machine::MachineProfile,
    workholding: rs_cam_core::feeds::WorkholdingRigidity,
    spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    project_default_rpm: u32,
    model_has_enriched: bool,
    model_is_step_missing_brep: bool,
    height_ctx: Option<&HeightContext>,
    preconditions: &rs_cam_core::diagnostics::diagnose::PreconditionContext,
    model_refs: &rs_cam_core::diagnostics::diagnose::ModelRefContext,
    stale_default_defects: &[rs_cam_core::compute::validate::StaleDefault],
    load_verdict: Option<&rs_cam_core::tool_load::ToolpathLoadVerdict>,
    tab_override: Option<ToolpathTab>,
    drill_layers: &[String],
    drill_targets: &[rs_cam_core::io::dxf_input::DrillTarget],
    // P5 — `show_reach_map` is the viewport's reach-map checkbox, threaded in
    // as a `&mut bool` rather than reached through `AppState`, because this
    // panel takes no state reference; the caller copies the flag out and
    // writes it back. `reach` is what to print beside that checkbox, borrowed
    // from a value the caller built before the panel runs, so holding it
    // costs no borrow of `AppState`.
    show_reach_map: &mut bool,
    reach: &ReachPanelSummary,
    // F2.2 — derived by the caller from the core result cache. The header
    // prints it in place of the raw `ComputeStatus`, which cannot say
    // "generated, then edited".
    freshness: &crate::state::freshness::FreshnessState,
    events: &mut Vec<AppEvent>,
) {
    // The inspector is a fixed-width side panel. Keep children — especially
    // long Feeds annotations — from enlarging its requested width.
    ui.set_max_width(ui.available_width());

    // ── Shared header (always visible above tabs) ───────────────────

    // Name — single editable home for the toolpath name; the duplicate
    // read-only heading was dropped (density pass 2026-06-11).
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut entry.name);
    });

    ui.separator();

    // Generate button + status (always visible) — promoted to the top
    // (SHE-004) so "is it generated? any warnings?" reads before the geometry
    // combos the user rarely revisits after setup.
    ui.add_space(4.0);
    let validation_errors = validate_toolpath(entry, validation);
    let can_generate = !tools.is_empty() && validation_errors.is_empty();
    // DC5 (Pattern C): the state the "requires manual generation" sentence
    // used to carry. Read before the row so the closure holds no extra
    // borrow of `entry`.
    let manual_generation = !entry.auto_regen && matches!(entry.status, ComputeStatus::Pending);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(can_generate, egui::Button::new("Generate"))
            .clicked()
        {
            events.push(AppEvent::GenerateToolpath(entry.id));
        }
        // F2.2 — the same one state the card chip and the workspace counts
        // read (R0.1 §4.4), in place of the raw `ComputeStatus`. The two
        // agree on six of seven; the seventh is why this changed.
        use crate::state::freshness::FreshnessState;
        match freshness {
            FreshnessState::NoResult => {
                // One word, then a hover. A printed sentence that names the
                // button next to it tells the reader nothing the button does
                // not already say.
                if manual_generation {
                    ui.label(egui::RichText::new("Manual").color(crate::ui::tokens::TEXT_MUTED))
                        .on_hover_text(
                            "This operation does not regenerate on its own. Press G, or click \
                         Generate, to compute it.",
                        );
                } else {
                    ui.label("Ready");
                }
            }
            FreshnessState::Regenerating => {
                ui.label("Computing...");
            }
            FreshnessState::Current => {
                ui.label(egui::RichText::new("Done").color(crate::ui::tokens::OK));
            }
            // NOT green, and it says both halves: the generation finished,
            // AND what it produced no longer answers the configuration on
            // this screen. "Done" alone was the whole defect — the header
            // sat directly above the fields the operator had just changed
            // and reported success at them.
            FreshnessState::EditedSince => {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("Done \u{00B7} edited since \u{2014} regenerate")
                            .color(crate::ui::tokens::CAUTION),
                    )
                    .wrap(),
                )
                .on_hover_text(
                    "This operation generated successfully, then one of its inputs changed. \
                     The path in the viewport and the figures on this panel are from that \
                     earlier generation, not from the settings shown here.",
                );
            }
            FreshnessState::WaitingOnUpstream(block) => {
                // Amber, not red: this operation is fine, it is waiting its
                // turn. The hover names the operation it is waiting for.
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("Waiting on upstream stock")
                            .color(crate::ui::tokens::CAUTION),
                    )
                    .wrap(),
                )
                .on_hover_text(&block.message);
            }
            FreshnessState::Disabled => {
                ui.label(egui::RichText::new("Disabled").color(crate::ui::tokens::TEXT_MUTED));
            }
            FreshnessState::Error(e) => {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("Error: {e}")).color(crate::ui::tokens::DANGER),
                    )
                    .wrap(),
                )
                .on_hover_text(e);
            }
        }
        // DC1 ruling R27 moved the card's figures off the toolpath card, so
        // this is the product's per-operation move count. It must say whose
        // generation it counts.
        //
        // F2.2 made the card prefix an edited operation's figures with
        // `old: `, for a reason that applies here unchanged: a confident
        // number outweighs a quiet state word beside it. R27 deleted the
        // surface that carried the mark, so the mark moves with the figure.
        // The word is the card's word, so the two surfaces cannot fork.
        if let Some(result) = &entry.result {
            let is_stale = matches!(freshness, FreshnessState::EditedSince);
            let moves = result.stats.move_count;
            let text = if is_stale {
                format!("old: {moves} moves")
            } else {
                format!("{moves} moves")
            };
            ui.label(
                egui::RichText::new(text)
                    .small()
                    .color(crate::ui::tokens::TEXT_FAINT),
            )
            .on_hover_text(if is_stale {
                "This count is from the previous generation. Regenerate the \
                 operation to measure the settings shown here."
            } else {
                "The move count of the current generation."
            });
        }
    });
    if !validation_errors.is_empty() {
        for err in &validation_errors {
            // A sentence, so it wraps explicitly rather than inheriting a
            // wrap mode from whatever layout encloses this panel
            // (G-REACHWRAP).
            wrapped_small_label(ui, err.clone(), crate::ui::tokens::CAUTION);
        }
    }

    // Contextual diagnostics (non-blocking). Native `Diagnostic`
    // rendering — splits findings into three tiers:
    //  * Actionable (state Current, severity ≥ Caution) — coloured row
    //    with evidence + confidence + optional Fix button.
    //  * Stateful (NeedsSimulation / StaleEvidence) — neutral grey
    //    "needs current simulation" / "re-run sim" rows.
    //  * Hints (state Current, severity ≤ Hint) — collapsed by default.
    //
    // Load-gate diagnostics (chipload / power / deflection / drill)
    // surface in the same ribbon now that we have `load_verdict`.
    //
    // DC5: the tiers are BUILT here, because the tab strip's badges read
    // them. The rows DRAW in the panel footer, below the tab content.
    let tool_for_diags = tool_configs
        .iter()
        .find(|(id, _)| *id == entry.tool_id)
        .map(|(_, tc)| tc);
    let diagnostics = operations::collect_diagnostics(
        entry,
        tool_for_diags,
        stale_default_defects,
        height_ctx,
        preconditions,
        model_refs,
        load_verdict,
    );
    // DC5 (Pattern C): the auto-regen workflow notice used to be synthesised
    // here as a `Category::State` diagnostic and printed as the sentence
    // "This operation requires manual generation. Press G or click Generate."
    // The sentence only named the button beside it. The state now reads as
    // one word on the Generate row, with the same fact on hover.
    let mut actionable = Vec::new();
    let mut stateful = Vec::new();
    let mut hints = Vec::new();
    for d in &diagnostics {
        use rs_cam_core::diagnostics::DiagnosticState;
        match d.state {
            DiagnosticState::Current if d.severity.is_actionable() => actionable.push(d),
            DiagnosticState::Current => {
                // Workflow / State category notices render as stateful
                // (neutral) — they don't deserve the yellow tier even
                // though they live in `Current` state.
                if matches!(d.category, rs_cam_core::diagnostics::Category::State) {
                    stateful.push(d);
                } else {
                    hints.push(d);
                }
            }
            DiagnosticState::NeedsSimulation | DiagnosticState::StaleEvidence => stateful.push(d),
            DiagnosticState::NotApplicable => {}
        }
    }

    // ── Tab bar ─────────────────────────────────────────────────────

    ui.add_space(8.0);
    let tab_id = ui.id().with("tp_tab").with(entry.id.0);
    let mut active_tab: ToolpathTab = tab_override.unwrap_or_else(|| {
        ui.memory(|mem| mem.data.get_temp(tab_id))
            .unwrap_or(ToolpathTab::Geometry)
    });
    let tab_badges = compute_tab_badges(entry, &diagnostics, height_ctx);
    draw_toolpath_tabs(ui, &mut active_tab, &tab_badges);
    ui.memory_mut(|mem| mem.data.insert_temp(tab_id, active_tab));
    ui.separator();

    // ── Tab content ─────────────────────────────────────────────────

    match active_tab {
        ToolpathTab::Geometry => {
            ui.add_space(4.0);

            // DC5: the geometry WIRING opens this tab — Tool, Input model,
            // Faces and the stock source. It used to sit behind a `Geometry`
            // disclosure above the tab strip, which gave one name two homes
            // at one level. One name, one place (Rule A).
            draw_geometry_wiring(
                ui,
                entry,
                tools,
                models,
                model_has_enriched,
                model_is_step_missing_brep,
            );
            ui.separator();
            ui.add_space(4.0);

            // Validator-driven Fix banner (PR-2C Phase 1). One row per
            // detected stale-default rule for this TP, with a one-click
            // Fix that mutates the operation directly. Defects are
            // recomputed by the caller each frame, so the banner
            // disappears as soon as the fix takes effect.
            for defect in stale_default_defects {
                egui::Frame::group(ui.style())
                    .fill(crate::ui::tokens::TINT_CAUTION)
                    .stroke(egui::Stroke::new(1.0_f32, crate::ui::tokens::CAUTION))
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("\u{26A0}")
                                    .color(crate::ui::tokens::CAUTION)
                                    .strong(),
                            );
                            ui.label(egui::RichText::new(&defect.title).strong());
                        });
                        ui.label(
                            egui::RichText::new(&defect.detail)
                                .small()
                                .color(crate::ui::tokens::TEXT_STRONG),
                        );
                        if ui
                            .small_button(format!("\u{2713} Fix (set to {:.3})", defect.new_value))
                            .on_hover_text(
                                "Apply the validator's auto-fix to this toolpath. \
                                 Mark stale and regenerate to apply.",
                            )
                            .clicked()
                        {
                            rs_cam_core::compute::validate::apply_stale_default_to_op(
                                &mut entry.operation,
                                &mut entry.feeds_provenance,
                                defect,
                            );
                            entry.stale_since = Some(std::time::Instant::now());
                        }
                    });
                ui.add_space(2.0);
            }

            // Compute + cache the LUT feeds result so the Geometry-row ⚡
            // pills (stepover / depth-per-pass) can render. Feed advice and
            // the single validated Apply all route live on the Feeds tab.
            let pill_tool_cfg = tool_configs
                .iter()
                .find(|(id, _)| *id == entry.tool_id)
                .map(|(_, t)| t);
            if let Some(tool_cfg) = pill_tool_cfg {
                entry.feeds_result = rs_cam_core::feeds::suggest::feeds_result_for_operation(
                    &entry.operation,
                    tool_cfg,
                    material,
                    machine,
                    workholding,
                    rs_cam_core::feeds::embedded_vendor_lut(),
                    spindle_strategy,
                )
                .ok();
            }
            // G-PILLCLAMP (UX-R03-014): one dry run of the apply funnel per
            // frame, so every ⚡ pill below offers and writes the value
            // the canonical Apply all route would write for its field — not
            // the raw calculator number (4.2 mm vs 1.2 mm of DOC on the demo
            // pocket).
            let pills = match (entry.feeds_result.as_ref(), pill_tool_cfg) {
                (Some(result), Some(tool_cfg)) => Some(PillSuggestions::new(
                    &entry.operation,
                    result,
                    tool_cfg,
                    machine,
                    material,
                )),
                _ => None,
            };

            // Operation description from spec (consistent across all operations)
            let spec = entry.operation.op_type().spec();
            ui.label(
                egui::RichText::new(spec.description)
                    .italics()
                    .color(crate::ui::tokens::TEXT_MUTED),
            );
            ui.add_space(2.0);
            // PR-2D Phase 2 — pass the per-field pill suggestions to every
            // per-op draw so each numeric field can render an inline ⚡
            // Suggest pill. Built above from the cached `entry.feeds_result`
            // and the funnel dry run, so this is just a borrow.
            let feeds_for_pills = pills.as_ref();
            // A/M6: read before the mutable borrow of `entry.operation`
            // below. Both are `Copy`, so nothing is held across it.
            let resolved_claims_reference =
                entry.result.as_ref().and_then(|r| r.stats.claims_reference);
            let stock_source_for_claims = entry.stock_source;
            // G-DEPTHSTOCK (UX-R03-007): the same rule the header ribbon
            // prints, read once here and handed to the depth field's row.
            // `Copy`, so nothing is held across the mutable borrow below.
            let depth_caution = height_ctx.and_then(|hctx| {
                operations::depth_beyond_stock(&entry.operation, &entry.heights, hctx)
            });
            let depth_caution = depth_caution.as_ref();
            // G-THROUGHCUT (UX-R03-006): Profile only. Read here, before the
            // mutable borrow, for the same reason as the caution above.
            let through_cut = match (&entry.operation, height_ctx) {
                (OperationConfig::Profile(cfg), Some(hctx)) => {
                    operations::profile_through_cut(cfg, &entry.heights, hctx)
                }
                _ => None,
            };
            let through_cut = through_cut.as_ref();
            match &mut entry.operation {
                OperationConfig::Face(cfg) => {
                    draw_face_params(ui, cfg, feeds_for_pills, depth_caution);
                }
                OperationConfig::Pocket(cfg) => {
                    draw_pocket_params(ui, cfg, feeds_for_pills, depth_caution);
                }
                OperationConfig::Profile(cfg) => {
                    draw_profile_params(ui, cfg, feeds_for_pills, depth_caution, through_cut);
                }
                OperationConfig::Adaptive(cfg) => {
                    draw_adaptive_params(ui, cfg, feeds_for_pills, depth_caution);
                }
                OperationConfig::VCarve(cfg) => {
                    draw_vcarve_params(ui, cfg, feeds_for_pills, depth_caution);
                }
                OperationConfig::Rest(cfg) => {
                    draw_rest_params(ui, cfg, tools, feeds_for_pills, depth_caution);
                }
                OperationConfig::Inlay(cfg) => {
                    draw_inlay_params(ui, cfg, feeds_for_pills, depth_caution);
                }
                OperationConfig::Zigzag(cfg) => {
                    draw_zigzag_params(ui, cfg, feeds_for_pills, depth_caution);
                }
                OperationConfig::Trace(cfg) => {
                    draw_trace_params(ui, cfg, feeds_for_pills, depth_caution);
                }
                OperationConfig::Drill(cfg) => {
                    draw_drill_params(
                        ui,
                        cfg,
                        drill_layers,
                        drill_targets,
                        feeds_for_pills,
                        depth_caution,
                    );
                }
                OperationConfig::Chamfer(cfg) => {
                    draw_chamfer_params(ui, cfg, feeds_for_pills, depth_caution);
                }
                OperationConfig::DropCutter(cfg) => {
                    draw_dropcutter_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::Adaptive3d(cfg) => {
                    // The "Optimal load" knob needs the active tool's
                    // radius to map engagement ↔ stepover.
                    let tool_radius = tool_configs
                        .iter()
                        .find(|(id, _)| *id == entry.tool_id)
                        .map(|(_, t)| t.diameter / 2.0)
                        .unwrap_or(0.0);
                    draw_adaptive3d_params(ui, cfg, tool_radius, feeds_for_pills);
                }
                OperationConfig::Waterline(cfg) => {
                    draw_waterline_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::Pencil(cfg) => {
                    // Pencil is special-cased (not part of the uniform
                    // `draw_*_params(ui, cfg, ...)` shape above): its
                    // "Rest reference" group needs `stock_source` and a
                    // stale flag alongside `cfg` — see the P2 consolidation
                    // comment on `draw_pencil_params`. `entry.stock_source`
                    // is a field disjoint from `entry.operation` (borrowed
                    // above as `cfg`), so both are borrowable here.
                    let mut pencil_ref_changed = false;
                    draw_pencil_params(
                        ui,
                        cfg,
                        tools,
                        feeds_for_pills,
                        &mut entry.stock_source,
                        &mut pencil_ref_changed,
                    );
                    if pencil_ref_changed {
                        entry.stale_since = Some(std::time::Instant::now());
                    }
                }
                OperationConfig::Scallop(cfg) => draw_scallop_params(ui, cfg, feeds_for_pills),
                OperationConfig::UnifiedFinish(cfg) => {
                    // A/M6: the claims block needs two things the config
                    // does not carry — what the LAST generation resolved
                    // `claims_reference` to (a `ToolpathStats` finding), and
                    // this op's stock source, which is what `Auto` derives
                    // against. Both are read-only here; `entry.result` and
                    // `entry.stock_source` are disjoint from
                    // `entry.operation`, which is borrowed mutably by the
                    // enclosing `match`.
                    draw_unified_finish_params(
                        ui,
                        cfg,
                        feeds_for_pills,
                        resolved_claims_reference,
                        stock_source_for_claims,
                    );
                }
                OperationConfig::SteepShallow(cfg) => {
                    draw_steep_shallow_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::RampFinish(cfg) => {
                    draw_ramp_finish_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::SpiralFinish(cfg) => {
                    draw_spiral_finish_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::RadialFinish(cfg) => {
                    draw_radial_finish_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::HorizontalFinish(cfg) => {
                    draw_horizontal_finish_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::ProjectCurve(cfg) => {
                    draw_project_curve_params(ui, cfg, models, feeds_for_pills);
                }
                OperationConfig::AlignmentPinDrill(cfg) => {
                    draw_alignment_pin_drill_params(
                        ui,
                        cfg,
                        drill_layers,
                        drill_targets,
                        feeds_for_pills,
                    );
                }
            }
            // G-PILLCLAMP: a ⚡ pill wrote its field this frame — stamp the
            // recommendation's provenance (the value is the funnel's, not a
            // hand edit) and remember it for the flush.
            if let Some((field, preview)) = pills.as_ref().and_then(|p| p.take_clicked()) {
                entry
                    .feeds_provenance
                    .set(field, preview.provenance.clone());
                entry.pill_stamped_fields.push(field);
            }

            // Pattern diagrams for all operation types
            ui.add_space(6.0);
            if let Some(pattern) = StepoverPattern::from_operation(&entry.operation) {
                draw_stepover_diagram(ui, &pattern);
            } else {
                match &entry.operation {
                    OperationConfig::Profile(cfg) => {
                        let side = match cfg.side {
                            ProfileSide::Outside => "Outside",
                            ProfileSide::Inside => "Inside",
                        };
                        draw_outline_diagram(ui, &format!("Profile ({side})"), Some(side));
                    }
                    OperationConfig::Chamfer(_) => {
                        draw_outline_diagram(ui, "Chamfer (edge contour)", None);
                    }
                    OperationConfig::Trace(cfg) => {
                        let comp = match cfg.compensation {
                            TraceCompensation::None => None,
                            TraceCompensation::Left => Some("Inside"),
                            TraceCompensation::Right => Some("Outside"),
                        };
                        draw_outline_diagram(ui, "Trace", comp);
                    }
                    OperationConfig::ProjectCurve(_) => {
                        draw_outline_diagram(ui, "Project Curve", None);
                    }
                    OperationConfig::Adaptive(cfg) => {
                        draw_spiral_diagram(ui, cfg.stepover, true);
                    }
                    OperationConfig::Adaptive3d(cfg) => {
                        draw_spiral_diagram(ui, cfg.stepover, true);
                    }
                    OperationConfig::SpiralFinish(cfg) => {
                        let outward = cfg.direction == SpiralDirection::InsideOut;
                        draw_spiral_diagram(ui, cfg.stepover, outward);
                    }
                    OperationConfig::RadialFinish(cfg) => {
                        draw_radial_diagram(ui, cfg.angular_step);
                    }
                    OperationConfig::Drill(_) => {
                        draw_point_set_diagram(ui, "Drill Points");
                    }
                    OperationConfig::AlignmentPinDrill(_) => {
                        draw_point_set_diagram(ui, "Pin Drill Holes");
                    }
                    OperationConfig::Pencil(cfg) => {
                        draw_pencil_diagram(ui, cfg.num_offset_passes, cfg.offset_stepover);
                    }
                    OperationConfig::SteepShallow(cfg) => {
                        draw_steep_shallow_diagram(ui, cfg.threshold_angle);
                    }
                    OperationConfig::RampFinish(cfg) => {
                        draw_ramp_finish_diagram(ui, cfg.max_stepdown);
                    }
                    OperationConfig::Inlay(cfg) => {
                        draw_inlay_diagram(ui, cfg.pocket_depth, cfg.glue_gap, cfg.flat_depth);
                    }
                    _ => {}
                }
            }

            // ── Machining Boundary ─────────────────────────────────────
            // W3.2: the boundary defines *what region to cut*, so it lives
            // under Geometry (was on the old Dressups tab).
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Machining Boundary")
                    .small()
                    .strong()
                    .color(crate::ui::tokens::TEXT_MUTED),
            );
            ui.checkbox(&mut entry.boundary.enabled, "Enable boundary")
                .on_hover_text(
                    "Restrict toolpath to a boundary polygon. \
                     Moves outside the boundary are converted to rapids at safe Z.",
                );
            if entry.boundary.enabled {
                // UX-R03-009 (G-BOUNDARYINHERIT): the stored source is the
                // one generation uses (`session/compute.rs` clones
                // `tc.boundary` unconditionally). Name it, then show the
                // controls that drive it. The controller assigns the model
                // silhouette on add for 3D ops on a mesh; that provenance is
                // not stored, so the line never claims "(auto)".
                ui.label(
                    egui::RichText::new(boundary_summary_line(&entry.boundary, false))
                        .small()
                        .color(crate::ui::tokens::TEXT_MUTED),
                );
                // Source selector
                ui.horizontal(|ui| {
                    ui.label("Source:");
                    egui::ComboBox::from_id_salt("boundary_source")
                        .selected_text(entry.boundary.source.label())
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    matches!(entry.boundary.source, BoundarySource::Stock),
                                    "Stock",
                                )
                                .on_hover_text("Use the stock bounding rectangle")
                                .clicked()
                            {
                                entry.boundary.source = BoundarySource::Stock;
                            }
                            if ui
                                .selectable_label(
                                    matches!(
                                        entry.boundary.source,
                                        BoundarySource::ModelSilhouette
                                    ),
                                    "Model Silhouette",
                                )
                                .on_hover_text(
                                    "XY projection of the 3D model. Only machines \
                                     where the model actually is.",
                                )
                                .clicked()
                            {
                                entry.boundary.source = BoundarySource::ModelSilhouette;
                            }
                            if ui
                                .selectable_label(
                                    matches!(entry.boundary.source, BoundarySource::FaceSelection),
                                    "Face Selection",
                                )
                                .on_hover_text("Boundary derived from selected STEP faces")
                                .clicked()
                            {
                                entry.boundary.source = BoundarySource::FaceSelection;
                            }
                            let has_rest_candidates = !boundary_source_candidates.is_empty();
                            if ui
                                .add_enabled(
                                    has_rest_candidates,
                                    egui::Button::selectable(
                                        matches!(
                                            entry.boundary.source,
                                            BoundarySource::DerivedRestRegions { .. }
                                        ),
                                        "Rest Regions",
                                    ),
                                )
                                .on_hover_text(if has_rest_candidates {
                                    "Boundary = rest regions computed by another \
                                     toolpath's rest analysis. Pick the source \
                                     toolpath below."
                                } else {
                                    "No other toolpaths in this project yet — add \
                                     one, switch on its Rest Analysis and \
                                     generate it to use as the source."
                                })
                                .clicked()
                            {
                                let default_source = boundary_source_candidates
                                    .first()
                                    .map(|(candidate_id, _, _, _, _)| *candidate_id)
                                    .unwrap_or(entry.id);
                                entry.boundary.source = BoundarySource::DerivedRestRegions {
                                    source_toolpath_id: default_source,
                                };
                            }
                        });
                });

                // Rest-regions source-toolpath picker (P2.2) — only shown
                // when `Source` above is set to `DerivedRestRegions`.
                // Candidates are every other toolpath in the session;
                // ones with a cached result whose rest regions are
                // already non-empty are labelled "(regions ready)" and
                // sorted first, but a not-yet-generated toolpath is
                // still selectable — generation fails hard with a clear
                // message if the source turns out unusable.
                if let BoundarySource::DerivedRestRegions { source_toolpath_id } =
                    &mut entry.boundary.source
                {
                    ui.horizontal(|ui| {
                        ui.label("Rest source:");
                        let current_label = boundary_source_candidates
                            .iter()
                            .find(|(candidate_id, _, _, _, _)| candidate_id == source_toolpath_id)
                            .map(|(_, name, ready, _, _)| {
                                if *ready {
                                    format!("{name} (regions ready)")
                                } else {
                                    name.clone()
                                }
                            })
                            .unwrap_or_else(|| "(toolpath not found)".to_owned());
                        let mut sorted = boundary_source_candidates.to_vec();
                        sorted.sort_by_key(|(_, _, ready, _, _)| !*ready);
                        egui::ComboBox::from_id_salt("boundary_rest_source")
                            .selected_text(current_label)
                            .show_ui(ui, |ui| {
                                for (candidate_id, name, ready, _, _) in &sorted {
                                    let label = if *ready {
                                        format!("{name} (regions ready)")
                                    } else {
                                        name.clone()
                                    };
                                    let selected = *source_toolpath_id == *candidate_id;
                                    if ui.selectable_label(selected, label).clicked() {
                                        *source_toolpath_id = *candidate_id;
                                    }
                                }
                            })
                            .response
                            .on_hover_text(
                                "The toolpath whose rest analysis supplies the \
                                 rest regions. ANY operation produces them when \
                                 its Rest Analysis is on and the project carries \
                                 a mesh; the pencil rest-depth detector and the \
                                 Unified Finish claims pipeline attach their own.",
                            );
                    });

                    // Sliver-storm / giant-region caption (2026-07-06
                    // incident): the SOURCE toolpath is who suffers the
                    // per-island generation explosion or the "barely
                    // restricts anything" giant-region case, so classify
                    // ITS cached regions (captured in
                    // `boundary_source_candidates` alongside `ready`),
                    // not this consumer's own (this toolpath has none —
                    // it's the one consuming the boundary).
                    //
                    // LH-2: the denominator is the SOURCE toolpath's own
                    // rest-grid footprint (captured alongside its
                    // regions), not this model's bounding rectangle.
                    let selected = boundary_source_candidates
                        .iter()
                        .find(|(candidate_id, _, _, _, _)| candidate_id == source_toolpath_id);
                    let selected_regions =
                        selected.and_then(|(_, _, _, regions, _)| regions.as_ref());
                    // `None` twice over: no candidate selected, or the
                    // selected one has no rest grid. Both are "not
                    // measured", and `classify_rest_regions` stays silent.
                    let source_footprint_area = selected.and_then(|(_, _, _, _, area)| *area);
                    if let Some(regions) = selected_regions
                        && let Some(pathology) =
                            rs_cam_core::surface::rest_field::classify_rest_regions(
                                regions,
                                source_footprint_area,
                            )
                    {
                        ui.label(
                            egui::RichText::new(rest_region_pathology_caption(pathology))
                                .small()
                                .color(crate::ui::tokens::CAUTION),
                        );
                    }
                }

                // Containment mode
                ui.horizontal(|ui| {
                    ui.label("Containment:");
                    egui::ComboBox::from_id_salt("boundary_contain")
                        .selected_text(match entry.boundary.containment {
                            BoundaryContainment::Center => "Center",
                            BoundaryContainment::Inside => "Inside",
                            BoundaryContainment::Outside => "Outside",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut entry.boundary.containment,
                                BoundaryContainment::Center,
                                "Center",
                            )
                            .on_hover_text("Tool center stays inside boundary");
                            ui.selectable_value(
                                &mut entry.boundary.containment,
                                BoundaryContainment::Inside,
                                "Inside",
                            )
                            .on_hover_text(
                                "Entire tool stays inside boundary (shrinks by tool radius)",
                            );
                            ui.selectable_value(
                                &mut entry.boundary.containment,
                                BoundaryContainment::Outside,
                                "Outside",
                            )
                            .on_hover_text("Tool edge can extend outside boundary");
                        });
                });

                // Offset
                ui.horizontal(|ui| {
                    ui.label("Offset:");
                    ui.add(
                        egui::DragValue::new(&mut entry.boundary.offset)
                            .speed(0.1)
                            .suffix(" mm"),
                    )
                    .on_hover_text(
                        "Expand (positive) or shrink (negative) the boundary. \
                         Applied before tool-radius containment.",
                    );
                });
            }

            // ── Rest Analysis (P2.5 → P2 pencil-panel consolidation) ────
            // Sibling of Machining Boundary: any toolpath can turn on the
            // rest-depth detector against ITS OWN tool, attaching the
            // heatmap grid + derived regions this op leaves behind — the
            // same analysis that used to be pencil-only. Now demand-driven
            // rather than a manual checkbox on every op: a downstream
            // `Rest Regions` boundary auto-enables it (see the
            // `auto_enable_rest_source` handling below this panel's draw
            // call, and `session::auto_enable_rest_analysis_for_source`
            // for the MCP-path twin), and it's
            // hidden entirely on a `rest_depth` pencil, whose own detector
            // already attaches the same artifacts (invisibly, per
            // `compute::execute::attach_generic_rest_analysis`'s precedence
            // check) — showing a second, redundant control there was the
            // third overlapping rest control this consolidation removes.
            let is_rest_depth_pencil = matches!(
                &entry.operation,
                OperationConfig::Pencil(cfg)
                    if rs_cam_core::finish::pencil::PencilDetector::parse(&cfg.detector)
                        == rs_cam_core::finish::pencil::PencilDetector::RestDepth
            );
            ui.add_space(8.0);
            if is_rest_depth_pencil {
                ui.label(
                    egui::RichText::new(
                        "Rest heatmap & regions: produced by the Rest depth detector.",
                    )
                    .small()
                    .color(crate::ui::tokens::TEXT_MUTED),
                );
            } else {
                ui.label(
                    egui::RichText::new("Rest Analysis")
                        .small()
                        .strong()
                        .color(crate::ui::tokens::TEXT_MUTED),
                );
                if rest_region_consumers.is_empty() {
                    ui.checkbox(
                        &mut entry.rest_analysis.enabled,
                        "Compute rest heatmap (material left after this op)",
                    )
                    .on_hover_text(
                        "Run the rest-depth detector against this toolpath's own tool \
                         after generation, attaching a heatmap grid. Also makes this op \
                         selectable as a `Rest Regions` boundary source on other \
                         toolpaths.",
                    );
                } else {
                    // Demand-driven: a consumer's boundary picker (or the
                    // MCP `set_boundary_config` path) already flipped this
                    // on — see `auto_enable_rest_analysis_for_source`. Force
                    // it here too so a stale project file (or a consumer
                    // whose boundary was set before this session started)
                    // still reflects reality.
                    if !entry.rest_analysis.enabled {
                        entry.rest_analysis.enabled = true;
                        entry.stale_since = Some(std::time::Instant::now());
                    }
                    ui.label(format!(
                        "Producing rest regions for: {}",
                        rest_region_consumers.join(", ")
                    ))
                    .on_hover_text(
                        "Enabled automatically — those toolpaths use this op's rest \
                         regions as their machining boundary.",
                    );
                }
                if entry.rest_analysis.enabled {
                    ui.horizontal(|ui| {
                        ui.label("Reference:");
                        let ref_label = entry
                            .rest_analysis
                            .reference_tool_id
                            .and_then(|rid| tools.iter().find(|(id, _, _)| *id == rid))
                            .map(|(_, name, _)| name.as_str())
                            .unwrap_or("Self / machined stock");
                        egui::ComboBox::from_id_salt("rest_analysis_reference_tool")
                            .selected_text(ref_label)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(
                                        entry.rest_analysis.reference_tool_id.is_none(),
                                        "Self / machined stock",
                                    )
                                    .clicked()
                                {
                                    entry.rest_analysis.reference_tool_id = None;
                                }
                                for (id, name, _) in tools {
                                    let selected =
                                        entry.rest_analysis.reference_tool_id == Some(*id);
                                    if ui.selectable_label(selected, name.as_str()).clicked() {
                                        entry.rest_analysis.reference_tool_id = Some(*id);
                                    }
                                }
                            })
                            .response
                            .on_hover_text(
                                "The reference the rest gate measures 'deeper than'. \
                                 Unset = prefer the actual machined stock from a prior \
                                 simulation, else a self-referenced bare-surface probe.",
                            );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Cell Size:");
                        ui.add(
                            egui::DragValue::new(&mut entry.rest_analysis.cell_mm)
                                .speed(0.05)
                                .range(0.05..=10.0)
                                .suffix(" mm"),
                        )
                        .on_hover_text(
                            "XY grid cell size for the rest field. Smaller = finer regions.",
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Min Valley Depth:");
                        ui.add(
                            egui::DragValue::new(&mut entry.rest_analysis.min_valley_depth)
                                .speed(0.01)
                                .range(0.0..=5.0)
                                .suffix(" mm"),
                        )
                        .on_hover_text(
                            "A cell counts as REST material once the reference floats \
                             more than this above the true surface.",
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Region Margin:");
                        ui.add(
                            egui::DragValue::new(&mut entry.rest_analysis.region_margin_mm)
                                .speed(0.05)
                                .range(0.0..=10.0)
                                .suffix(" mm"),
                        )
                        .on_hover_text(
                            "Extra clearance added around detected rest regions beyond \
                             this toolpath's own tool radius.",
                        );
                    });
                }
            }

            // Sliver-storm / giant-region caption (2026-07-06 incident):
            // classify THIS toolpath's own generated rest regions, whether
            // they came from the checkbox-driven detector above or (for a
            // rest_depth pencil) the detector it always runs. Deliberately
            // outside the `is_rest_depth_pencil` branch so both cases show
            // it.
            //
            // LH-2: the giant-region denominator is THIS result's own rest
            // grid — covered cells × cell², the same grid the regions were
            // extracted from. It used to be the model's XY bounding
            // rectangle, which overstates a non-rectangular part's footprint
            // and made the warning fire late (`MEASUREMENT_DOMAINS.md` X-4).
            // No grid ⇒ 0.0 ⇒ silence, never a guess.
            if let Some(result) = &entry.result
                && let Some(regions) = result.annotated.rest_regions.as_ref()
                && let Some(pathology) = rs_cam_core::surface::rest_field::classify_rest_regions(
                    regions,
                    rest_grid_footprint_area(result.annotated.rest_grid.as_deref()),
                )
            {
                ui.label(
                    egui::RichText::new(rest_region_pathology_caption(pathology))
                        .small()
                        .color(crate::ui::tokens::CAUTION),
                );
            }
        }

        ToolpathTab::FeedsSpeeds => {
            let tool_info = tool_configs
                .iter()
                .find(|(id, _)| *id == entry.tool_id)
                .map(|(_, t)| (t.diameter, t.tool_type));
            let (tool_diameter, tool_type) =
                tool_info.unwrap_or((6.0, crate::state::job::ToolType::EndMill));
            ui.horizontal(|ui| {
                if ui
                    .button("Explore…")
                    .on_hover_text("Open the feed-versus-RPM nomogram for this operation.")
                    .clicked()
                {
                    events.push(AppEvent::OpenFeedsModal(entry.id));
                }
            });
            ui.add_space(4.0);
            draw_speed_controls(ui, entry, project_default_rpm);
            ui.add_space(4.0);
            if let Some(tool_cfg) = tool_configs
                .iter()
                .find(|(id, _)| *id == entry.tool_id)
                .map(|(_, t)| t)
            {
                calculate_and_apply_feeds(
                    ui,
                    entry,
                    tool_cfg,
                    material,
                    machine,
                    workholding,
                    spindle_strategy,
                    project_default_rpm,
                    load_verdict,
                    events,
                );
            }
            // Vendor cutting data viewer (always available, filtered by tool)
            draw_vendor_lut_viewer(ui, tool_type, tool_diameter);
        }

        ToolpathTab::Linking => {
            // How moves connect: entry/exit, move optimization, retract
            // strategy (W3.2 — lifted out of the old Dressups tab).
            draw_linking_params(ui, entry, height_ctx);
        }

        ToolpathTab::Heights => {
            let fallback_ctx = HeightContext::simple(10.0, 5.0);
            let ctx = height_ctx.unwrap_or(&fallback_ctx);
            // F1.19 / G-BOTTOMPIN: the Bottom row is annotated per operation,
            // so the panel needs the operation. Read before the mutable
            // borrow of `entry.heights`; `OperationType` is `Copy`.
            // F-6 / R33: the diagram takes the same operation, so its Bottom
            // line agrees with the sentence the panel prints beside the row.
            let op_type = entry.operation.op_type();
            draw_heights_params(ui, &mut entry.heights, ctx, op_type);
            ui.add_space(6.0);
            draw_height_diagram(ui, &mut entry.heights, ctx, op_type);
        }

        ToolpathTab::Dressup => {
            // Active dressup summary + reset button
            let (active, total) = dressup_active_count(&entry.dressups);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{active}/{total} dressups active"))
                        .small()
                        .color(if active > 0 {
                            crate::ui::tokens::OK
                        } else {
                            crate::ui::tokens::TEXT_MUTED
                        }),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let role = entry.operation.op_type().spec().ui_process_role;
                    if ui
                        .small_button("Reset to recommended")
                        .on_hover_text(format!(
                            "Apply recommended dressups for {} operations",
                            match role {
                                UiProcessRole::Roughing => "roughing",
                                UiProcessRole::SemiFinish => "semi-finish",
                                UiProcessRole::Finish => "finishing",
                            }
                        ))
                        .clicked()
                    {
                        entry.dressups = DressupConfig::for_role(role);
                    }
                });
            });

            // Edge work / path quality (W3.2 — Entry/Exit, Optimization
            // and Retract moved to the Linking tab; Machining Boundary
            // moved to Geometry).
            draw_dressup_params(ui, &mut entry.dressups);

            // Manual G-code fields (pre_gcode, post_gcode) kept in state
            // for future export wiring — UI removed until export is implemented.
        }
    }

    // ── Panel footer ────────────────────────────────────────────────
    //
    // DC5 (Pattern A / Pattern C): everything below belongs to the panel, not
    // to a tab, so it renders BELOW the tab strip's content. The reach
    // readings and the diagnostic ribbon used to draw ABOVE the strip, which
    // put a paragraph of text between the Generate button and the tabs and
    // left the reader unable to tell what contained what.
    ui.separator();

    // Per-tool reach map (P5). Reach-capable operations only — every other
    // operation gets no checkbox at all rather than a disabled one, because
    // the question does not apply to it (`OperationType::supports_reach_map`).
    if entry.operation.op_type().supports_reach_map() {
        ui.horizontal(|ui| {
            ui.checkbox(show_reach_map, "Show reach map").on_hover_text(
                "Colour the model by what THIS toolpath's cutter can form: green where \
                     the cutter reaches the surface within the operation's tolerance, red \
                     where a gap is left. The measure is top-down, so undersides, overhangs \
                     and vertical walls are NOT MEASURED and keep the plain model colour.",
            );
            // Only the two SHORT status words share the checkbox's row.
            // A horizontal layout's wrap mode is `Extend` (egui's
            // `Ui::wrap_mode`: a layout that is neither vertical nor
            // main-wrapped falls to the `Extend` arm), so a bare `ui.label`
            // here runs past the panel and clips. A `Label::wrap()` WOULD
            // still wrap — the label's own mode overrides the Ui's
            // (`label.rs:191`, `self.wrap_mode.unwrap_or_else(|| ui.wrap_mode())`) —
            // but at `ui.available_width()`, which on this row is only what
            // the checkbox leaves: a narrow column, not the panel width.
            // These two readings are under twenty characters and carry no
            // caveat, so either fate is fine for them; the ones that do
            // carry a caveat are below, where they get the full width
            // (G-REACHWRAP).
            match reach {
                ReachPanelSummary::Computing => {
                    ui.label(
                        egui::RichText::new("reach: computing…")
                            .small()
                            .color(crate::ui::tokens::TEXT_MUTED),
                    );
                }
                // A zero percentage over an empty population is
                // indistinguishable from a clean part, so it is never shown
                // as one.
                ReachPanelSummary::NotMeasured => {
                    ui.label(
                        egui::RichText::new("reach: not measured")
                            .small()
                            .color(crate::ui::tokens::CAUTION),
                    );
                }
                ReachPanelSummary::Measured { .. }
                | ReachPanelSummary::Failed(_)
                | ReachPanelSummary::Idle => {}
            }
        });

        // G-REACHWRAP (UX-R09-001, 2026-09-10): the readings used to sit on
        // the checkbox's row as bare `ui.label`s, which under that row's
        // `Extend` mode ran past the panel and clipped — the review's
        // 1400×900 capture shows the summary cut off mid-number with
        // `grid_note` never drawn at all
        // (`results/W02/evidence/01_finish_defaults.png`). `grid_note` and
        // `over_statement_note` are the MISSING-GUARANTEE sentences — the
        // cell, the floor and the bar, and the statement that the grid
        // over-states — so a clipped one reads as an unqualified result.
        // They now get the panel's full width, one wrapped line each.
        match reach {
            ReachPanelSummary::Measured {
                unreachable_pct,
                max_gap_mm,
                grid_note,
                area_basis_note,
                over_statement_note,
                tolerance_below_floor,
            } => {
                // The base comes from `ReachMap::area_basis_note`, not
                // from a sentence written here. This line said "of
                // MEASURED area" while the panel legend said "of 3D
                // surface area, rim-eroded 3.0 mm" - two surfaces, one
                // quantity, two descriptions, and that is the drift the
                // shared notes exist to prevent.
                wrapped_small_label(
                    ui,
                    format!(
                        "unreachable {unreachable_pct:.1} % {area_basis_note} · \
                         max gap {max_gap_mm:.2} mm"
                    ),
                    crate::ui::tokens::TEXT_STRONG,
                );
                wrapped_small_label(
                    ui,
                    grid_note.clone(),
                    if *tolerance_below_floor {
                        crate::ui::tokens::CAUTION
                    } else {
                        crate::ui::tokens::TEXT_MUTED
                    },
                );
                if *tolerance_below_floor {
                    // The shared sentence, quoted - not a fourth
                    // hand-written paraphrase of the same bias.
                    wrapped_small_label(
                        ui,
                        over_statement_note.clone(),
                        crate::ui::tokens::CAUTION,
                    );
                }
            }
            ReachPanelSummary::Failed(message) => {
                wrapped_small_label(ui, format!("reach: {message}"), crate::ui::tokens::CAUTION);
            }
            ReachPanelSummary::Computing
            | ReachPanelSummary::NotMeasured
            | ReachPanelSummary::Idle => {}
        }
    }

    for d in &actionable {
        render_diagnostic_row(ui, d, RowTier::Actionable, entry, stale_default_defects);
    }
    let (merged_gate_rows, stateful_rest) = merge_stateful_gate_rows(&stateful);
    for d in &merged_gate_rows {
        render_diagnostic_row(ui, d, RowTier::Stateful, entry, stale_default_defects);
    }
    for d in &stateful_rest {
        render_diagnostic_row(ui, d, RowTier::Stateful, entry, stale_default_defects);
    }

    // DC5 (Pattern C): the hint list is a COUNT at the bottom of the panel,
    // not a block of text at the top. The operator's words were "hints is a
    // lot of text and its at the top, why?". Every hint and its severity
    // order survive; the row's placement and its resting weight are what
    // changed. `id_salt` pins the collapse state, which the header text used
    // to derive and would otherwise reset on every count change.
    if !hints.is_empty() {
        egui::CollapsingHeader::new(
            egui::RichText::new(format!("{} hints", hints.len()))
                .small()
                .color(crate::ui::tokens::TEXT_MUTED),
        )
        .id_salt("tp_hints")
        .default_open(false)
        .show(ui, |ui| {
            for d in &hints {
                render_diagnostic_row(ui, d, RowTier::Hint, entry, stale_default_defects);
            }
        });
    }
}

#[cfg(test)]
mod tests;
