//! The MCP simulation surface: run and scrub the simulation, read the
//! cut trace, check collisions, and capture the simulation viewport.
//!
//! Split out of `app/mcp.rs` (P4). Every item is a verbatim move; the
//! handlers stay reachable as `RsCamApp::mcp_*` because an inherent
//! `impl` compiles in any module of the crate.

use std::path::Path;

use rs_cam_core::compute::config::ComputeStatus;

use rs_cam_mcp::server::{json_str, text};

use crate::app::RsCamApp;
use crate::mcp_bridge::McpResponse;
use crate::ui::AppEvent;
use crate::ui_command::{NoArgs, SimJumpToMoveArgs, UiCommand};

use super::diagnostics::parse_span_kind_filter;

impl RsCamApp {
    pub(super) fn mcp_runtime_status_for_toolpath_id(
        &self,
        toolpath_id: rs_cam_core::ToolpathId,
    ) -> serde_json::Value {
        let state = self.controller.state();
        let rt = state.gui.toolpath_rt.get(&toolpath_id);
        let enabled = state
            .session
            .find_toolpath_config_by_id(toolpath_id)
            .is_none_or(|(_, tc)| tc.enabled);
        let raw = rt.map_or(&ComputeStatus::Pending, |r| &r.status);
        let status = ComputeStatus::effective(enabled, raw);
        serde_json::json!({
            "status": status.label(),
            "error": status.error_text(),
            "awaiting_prior_stock": status.blocked_on().map(|b| serde_json::json!({
                "blocking_toolpath_id": b.blocking_toolpath_id,
                "blocking_toolpath_index": b.blocking_toolpath_index,
                "message": b.message,
            })),
            // A/M6: what the three-valued `claims_reference` param actually
            // RESOLVED to, and why. It belongs here and not under `params`
            // because it is not a param: `auto` resolves against whether a
            // machined prior stock was in scope at generation time, which no
            // config field records. `null` until the operation has generated,
            // and on any operation that runs no claims pipeline.
            "claims_reference": rt
                .and_then(|r| r.result.as_ref())
                .and_then(|res| res.stats.claims_reference)
                .map(|f| serde_json::json!({
                    "setting": f.resolution.setting(),
                    "resolved": f.resolution.reference(),
                    "resolution": f.resolution.label(),
                    "derived": f.resolution.is_derived(),
                    "prior_stock_in_scope": f.resolution.prior_stock_in_scope(),
                    "needs_attention": f.resolution.needs_attention(),
                    "territory_clip_requested": f.territory_clip_requested,
                    "territory_clip_skipped": f.territory_clip_skipped(),
                    "why": f.resolution.why(),
                })),
            "stale": rt.is_some_and(|r| r.stale_since.is_some()),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn mcp_get_cut_trace(
        &self,
        toolpath_id: Option<usize>, // wire-level raw id; typed immediately below
        max_hotspots: Option<usize>,
        max_issues: Option<usize>,
        span_kind: Option<&str>,
        span_id: Option<u32>,
        pass_index: Option<u32>,
        include_drill_samples: bool,
        caps: rs_cam_mcp::response::CutTraceCaps,
    ) -> String {
        let state = self.controller.state();
        let sim_state = &state.simulation;
        let Some(results) = sim_state.results.as_ref() else {
            return json_str(
                serde_json::json!({"error": "No simulation result. Run run_simulation first."}),
            );
        };
        let Some(ct) = results.cut_trace.as_ref() else {
            return json_str(
                serde_json::json!({"error": "No cut trace data. Run simulation with a loaded project."}),
            );
        };

        let req = CutTraceRequest {
            toolpath_id,
            max_hotspots,
            max_issues,
            span_kind,
            span_id,
            pass_index,
            include_drill_samples,
            caps,
        };
        match build_cut_trace_response(state, ct, &req) {
            Ok(value) => json_str(value),
            Err(e) => json_str(serde_json::json!({"error": e})),
        }
    }

    /// Per-toolpath listing of collisions detected during simulation:
    /// holder collisions (sim.checks.collision_report) and rapid collisions
    /// (sim.checks.rapid_collision_move_indices), grouped by the toolpath
    /// each move belongs to. Localizes the project-wide
    /// `rapid_collision_count` from `run_simulation` so the user can drill
    /// to the specific lift/retract that's clipping uncleared stock.
    pub(super) fn mcp_inspect_collisions(&self) -> String {
        let state = self.controller.state();
        let sim = &state.simulation;

        if !sim.has_results() {
            return json_str(serde_json::json!({
                "error": "No simulation results. Call run_simulation first."
            }));
        }

        let mut holder_by_tp: std::collections::BTreeMap<usize, Vec<serde_json::Value>> =
            std::collections::BTreeMap::new();
        let mut rapid_by_tp: std::collections::BTreeMap<usize, Vec<serde_json::Value>> =
            std::collections::BTreeMap::new();

        // Holder/shank collisions live in collision_report. Their `move_idx`
        // is in global move space; map back to (toolpath_id, local_move).
        if let Some(report) = sim.checks.collision_report.as_ref() {
            for c in &report.collisions {
                let (tp_id, local_move) = sim
                    .move_to_local_toolpath_move(c.move_idx)
                    .map(|(_, id, local)| (id.0, local))
                    .unwrap_or((usize::MAX, c.move_idx));
                holder_by_tp
                    .entry(tp_id)
                    .or_default()
                    .push(serde_json::json!({
                        "global_move": c.move_idx,
                        "local_move": local_move,
                        "segment": format!("{:?}", c.segment),
                    }));
            }
        }

        // Rapid collisions are stored as bare global move indices.
        for &global_move in &sim.checks.rapid_collision_move_indices {
            let (tp_id, local_move) = sim
                .move_to_local_toolpath_move(global_move)
                .map(|(_, id, local)| (id.0, local))
                .unwrap_or((usize::MAX, global_move));
            rapid_by_tp
                .entry(tp_id)
                .or_default()
                .push(serde_json::json!({
                    "global_move": global_move,
                    "local_move": local_move,
                }));
        }

        // Build the per-toolpath rollup. Include every TP that has at least
        // one of either kind so the agent doesn't have to merge two maps.
        let all_tps: std::collections::BTreeSet<usize> = holder_by_tp
            .keys()
            .chain(rapid_by_tp.keys())
            .copied()
            .collect();

        let session = &state.simulation;
        let by_toolpath: Vec<serde_json::Value> = all_tps
            .iter()
            .map(|tp_id| {
                let name = session
                    .boundaries()
                    .iter()
                    .find(|b| b.id.0 == *tp_id)
                    .map(|b| b.name.clone())
                    .unwrap_or_else(|| format!("(unknown {tp_id})"));
                let holder = holder_by_tp.get(tp_id).cloned().unwrap_or_default();
                let rapid = rapid_by_tp.get(tp_id).cloned().unwrap_or_default();
                serde_json::json!({
                    "toolpath_id": tp_id,
                    "toolpath_name": name,
                    "holder_collision_count": holder.len(),
                    "rapid_collision_count": rapid.len(),
                    "holder_collisions": holder,
                    "rapid_collisions": rapid,
                })
            })
            .collect();

        let total_holder: usize = holder_by_tp.values().map(|v| v.len()).sum();
        let total_rapid: usize = rapid_by_tp.values().map(|v| v.len()).sum();

        json_str(serde_json::json!({
            "holder_collision_count": total_holder,
            "rapid_collision_count": total_rapid,
            "by_toolpath": by_toolpath,
        }))
    }

    pub(super) fn mcp_run_simulation(
        &mut self,
        resolution: Option<f64>,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    ) {
        // Set resolution if provided
        if let Some(res) = resolution {
            self.controller.state_mut().simulation.resolution = res;
            self.controller.state_mut().simulation.auto_resolution = false;
        }

        // Always enable metrics when MCP triggers simulation — the standalone
        // MCP server hardcodes this, and diagnostics/cut_trace require it.
        // `capture_arc_engagement` is also forced on so the tool-load `power`
        // criterion can evaluate against the run.
        self.controller
            .state_mut()
            .simulation
            .set_metric_capture_enabled(true);

        // Push the simulation event
        self.controller
            .events_mut()
            .push(crate::ui::AppEvent::RunSimulation);

        // Store the oneshot sender
        if let Some(ref mut pending) = self.controller.pending_mcp {
            pending.simulation = Some(response_tx);
        } else {
            let _ = response_tx.send(McpResponse {
                result: Err("MCP compute tracking not initialized".to_owned()),
            });
        }
    }

    pub(super) fn mcp_collision_check(
        &mut self,
        _index: usize,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    ) {
        // Push the collision check event
        self.controller
            .events_mut()
            .push(crate::ui::AppEvent::RunCollisionCheck);

        // Store the oneshot sender
        if let Some(ref mut pending) = self.controller.pending_mcp {
            pending.collision = Some(response_tx);
        } else {
            let _ = response_tx.send(McpResponse {
                result: Err("MCP compute tracking not initialized".to_owned()),
            });
        }
    }

    pub(super) fn mcp_screenshot_simulation(
        &self,
        path: &str,
        width: Option<u32>,
        height: Option<u32>,
        checkpoint: Option<usize>,
        include_toolpaths: Option<bool>,
    ) -> String {
        let sim_state = &self.controller.state().simulation;
        let Some(results) = sim_state.results.as_ref() else {
            return text("No simulation result. Run run_simulation first.");
        };

        if path.ends_with(".png") {
            let w = width.unwrap_or(1200);
            let h = height.unwrap_or(800);
            let cp_idx = checkpoint.unwrap_or_else(|| results.checkpoints.len().saturating_sub(1));
            // Anchor to the STOCK-RELATIVE ZERO-ROOTED frame, which is what
            // the simulated stock is actually in: `compute::simulate` builds
            // its global grid as `0..(max-min)` per axis precisely because
            // `local_to_global` yields stock-relative coordinates, not world
            // ones. Passing the WORLD bbox here (as this did briefly) unions
            // two different frames — on a stock at origin (-20,-25) the footer
            // read `X -20.0..140.1` for a 140 mm blank, i.e. the world bbox
            // and the zero-rooted grid side by side.
            let stock_bbox = self.controller.state().session.stock_bbox();
            let world_frame = rs_cam_core::geo::BoundingBox3 {
                min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                max: rs_cam_core::geo::P3::new(
                    stock_bbox.max.x - stock_bbox.min.x,
                    stock_bbox.max.y - stock_bbox.min.y,
                    stock_bbox.max.z - stock_bbox.min.z,
                ),
            };
            // A checkpoint whose stock is SETUP-LOCAL (a lateral setup —
            // G-LATERALSCRUB) cannot be rendered in the global frame this
            // footer anchors to: its grid is in a different frame, and
            // rendering it here would misregister the part exactly the way
            // G-SIM-IDENTITY-FRAME did. Its `mesh` is already mapped into the
            // global frame, so take that route instead — and it is the route
            // that actually shows the lateral cut.
            let local_framed_stock = results
                .checkpoints
                .get(cp_idx)
                .is_some_and(|cp| cp.stock_local_to_global().is_some());
            let pixels = if let Some(cp) = results
                .checkpoints
                .get(cp_idx)
                .filter(|_| !local_framed_stock)
            {
                rs_cam_core::export::fingerprint::render_stock_composite_in_frame(
                    cp.stock(),
                    &world_frame,
                    w,
                    h,
                )
            } else if let Some(cp) = results.checkpoints.get(cp_idx) {
                rs_cam_core::export::fingerprint::render_mesh_composite_in_frame(
                    cp.mesh(),
                    Some(&world_frame),
                    w,
                    h,
                )
            } else {
                rs_cam_core::export::fingerprint::render_mesh_composite_in_frame(
                    &results.mesh,
                    Some(&world_frame),
                    w,
                    h,
                )
            };
            match image::save_buffer(Path::new(path), &pixels, w, h, image::ColorType::Rgba8) {
                Ok(()) => text(format!(
                    "6-view composite exported to {path} ({w}x{h}, checkpoint {cp_idx})",
                )),
                Err(e) => text(format!("Failed to save PNG: {e}")),
            }
        } else {
            let session = &self.controller.state().session;
            let toolpaths: Vec<&rs_cam_core::toolpath::Toolpath> =
                if include_toolpaths.unwrap_or(true) {
                    self.controller
                        .state()
                        .gui
                        .toolpath_rt
                        .values()
                        .filter_map(|rt| rt.result.as_ref())
                        .map(|r| r.toolpath())
                        .collect()
                } else {
                    Vec::new()
                };

            let html = rs_cam_core::export::viz::stock_mesh_to_3d_html(
                &sim_mesh_in_world_frame(&results.mesh, session),
                &toolpaths,
                &format!("{} -- Simulation", session.name()),
            );

            match std::fs::write(path, &html) {
                Ok(()) => text(format!(
                    "Simulation view exported to {path} ({} vertices, {} triangles)",
                    results.mesh.vertex_count(),
                    results.mesh.indices.len() / 3,
                )),
                Err(e) => text(format!("Failed to write: {e}")),
            }
        }
    }

    pub(super) fn mcp_sim_jump_to_move(&mut self, move_index: usize) -> String {
        let sim = &self.controller.state().simulation;
        if !sim.has_results() {
            return json_str(
                serde_json::json!({"error": "No simulation result. Run run_simulation first."}),
            );
        }
        self.controller
            .events_mut()
            .push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                move_index,
            })));
        self.mcp_sim_playback_state(move_index)
    }

    pub(super) fn mcp_sim_jump_to_start(&mut self) -> String {
        let sim = &self.controller.state().simulation;
        if !sim.has_results() {
            return json_str(
                serde_json::json!({"error": "No simulation result. Run run_simulation first."}),
            );
        }
        self.controller
            .events_mut()
            .push(AppEvent::Ui(UiCommand::SimJumpToStart(NoArgs)));
        self.mcp_sim_playback_state(0)
    }

    pub(super) fn mcp_sim_jump_to_end(&mut self) -> String {
        let sim = &self.controller.state().simulation;
        if !sim.has_results() {
            return json_str(
                serde_json::json!({"error": "No simulation result. Run run_simulation first."}),
            );
        }
        let total = sim.total_moves();
        self.controller
            .events_mut()
            .push(AppEvent::Ui(UiCommand::SimJumpToEnd(NoArgs)));
        self.mcp_sim_playback_state(total)
    }

    /// Scrub to a percentage position within a specific toolpath.
    pub(super) fn mcp_sim_scrub_toolpath(
        &mut self,
        index: usize,
        percent: f64,
    ) -> Result<String, String> {
        let sim = &self.controller.state().simulation;
        if !sim.has_results() {
            return Err("No simulation result. Run run_simulation first.".to_owned());
        }

        let boundaries = sim.boundaries();
        let boundary = boundaries.get(index).ok_or_else(|| {
            format!(
                "Toolpath index {index} not found in simulation boundaries (have {})",
                boundaries.len()
            )
        })?;

        let clamped_percent = percent.clamp(0.0, 100.0);
        let start = boundary.start_move;
        let end = boundary.end_move;
        let range = end.saturating_sub(start);
        let move_index = start + (range as f64 * clamped_percent / 100.0) as usize;
        let tp_name = boundary.name.clone();
        let total_moves_in_toolpath = range;

        self.controller
            .events_mut()
            .push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                move_index,
            })));

        Ok(json_str(serde_json::json!({
            "move_index": move_index,
            "toolpath_name": tp_name,
            "percent": clamped_percent,
            "total_moves_in_toolpath": total_moves_in_toolpath,
            "start_move": start,
            "end_move": end,
        })))
    }

    /// Build a JSON response with the current simulation playback state.
    fn mcp_sim_playback_state(&self, move_index: usize) -> String {
        let sim = &self.controller.state().simulation;
        let total = sim.total_moves();
        let clamped = move_index.min(total);

        // Find which toolpath is active at this move index.
        let active_toolpath = sim
            .boundaries()
            .iter()
            .find(|b| clamped >= b.start_move && clamped <= b.end_move)
            .map(|b| {
                serde_json::json!({
                    "name": b.name,
                    "tool": b.tool_name,
                    "start_move": b.start_move,
                    "end_move": b.end_move,
                })
            });

        // Find the checkpoint index (boundary index of the nearest completed toolpath).
        let checkpoint = sim
            .boundaries()
            .iter()
            .enumerate()
            .filter(|(_, b)| clamped >= b.end_move)
            .map(|(i, _)| i)
            .next_back();

        json_str(serde_json::json!({
            "move_index": clamped,
            "total_moves": total,
            "active_toolpath": active_toolpath,
            "checkpoint": checkpoint,
        }))
    }
}

/// Build span-level cut summaries for `get_cut_trace`.
///
/// Returns summaries for every structural span that has samples in the current
/// trace. When a span filter is active, only accepted span ids are summarized.
///
/// **C1 (2026-08-06).** This used to nest a full scan of the project's entire
/// sample vector inside a loop over every span, so a bare `get_cut_trace()`
/// cost `Σ_toolpaths (spans × total_samples)` — on the egui frame-loop
/// thread, with every other queued MCP request waiting behind it. The
/// accumulation now happens in one pass in
/// [`rs_cam_core::stock::simulation_cut::accumulate_by_span`], which owns the
/// nesting rule (a sample belongs to EVERY span in its `span_path`) and is
/// pinned against a verbatim transcription of the old walk by
/// `crates/rs_cam_core/tests/span_summary_single_pass_c1.rs`. This function
/// keeps only the JSON shaping; no wire key changed.
/// One `get_cut_trace` request, resolved from the wire.
pub(crate) struct CutTraceRequest<'a> {
    /// Project-level toolpath **id** — see [`build_cut_trace_response`].
    pub toolpath_id: Option<usize>,
    pub max_hotspots: Option<usize>,
    pub max_issues: Option<usize>,
    pub span_kind: Option<&'a str>,
    pub span_id: Option<u32>,
    pub pass_index: Option<u32>,
    pub include_drill_samples: bool,
    pub caps: rs_cam_mcp::response::CutTraceCaps,
}

/// Build the `get_cut_trace` response under Checkpoint L's bounds.
///
/// # `toolpath_id` is an ID, and an unmatched one is refused
///
/// The trace is keyed by the project-level [`rs_cam_core::ToolpathId`]
/// throughout — samples, hotspots, issues, drill summaries and drill samples
/// all carry it — so filtering by id is the only filter that can be applied
/// without a lookup, and the parameter keeps id semantics (Checkpoint L-5).
/// Its doc string used to say *index*, which was wrong in a way that could
/// not be noticed: measured on a real project 2026-08-08, the values 4, 5
/// and 6 were **simultaneously valid indices and valid ids of different
/// toolpaths**, so an agent following the documentation received another
/// toolpath's data with no error and no warning. An id matching nothing used
/// to return a full skeleton with every array empty and
/// `issue_count_project_wide: 73326` sitting beside `issue_count: 0` — a
/// caller error dressed as a measurement. It is now an `Err`.
///
/// # Bounds
///
/// Sections are offered to the budget smallest-and-most-load-bearing first
/// and `span_summaries` **last**, because section order is priority order:
/// on overflow it is the sections offered last that are dropped, and
/// `span_summaries` was 94 % of the payload that motivated this work.
pub(crate) fn build_cut_trace_response(
    state: &crate::state::AppState,
    ct: &rs_cam_core::stock::simulation_cut::SimulationCutTrace,
    req: &CutTraceRequest<'_>,
) -> Result<serde_json::Value, String> {
    use rs_cam_core::trace::toolpath_spans::{SpanId, SpanPayload};
    use rs_cam_mcp::response::{BoundedResponse, cap_json_values};

    // ── L-5: refuse an id that matches no toolpath ──────────────────────
    let valid_ids: Vec<usize> = (0..state.session.toolpath_count())
        .filter_map(|idx| state.session.get_toolpath_config(idx).map(|tc| tc.id.0))
        .collect();
    if let Some(raw) = req.toolpath_id
        && !valid_ids.contains(&raw)
    {
        let list = valid_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "toolpath_id {raw} matches no toolpath. This parameter takes the project-level \
             ID (the `id` field from list_toolpaths), NOT the index — the two coincide only \
             by accident. Valid ids: [{list}]. Returning an empty result here would be \
             indistinguishable from a toolpath that genuinely produced no samples."
        ));
    }

    let max_h = req.max_hotspots.unwrap_or(20);
    let max_i = req.max_issues.unwrap_or(50);

    // Translate the span filter args into a per-toolpath set of accepted
    // SpanIds. A span_path matches when it contains any accepted SpanId
    // (or the filter is unset).
    //
    // Filter resolution requires the AnnotatedToolpath to look up
    // SpanKind / SpanPayload for each span index. When toolpath_id is
    // unset and any span filter is set, we resolve per toolpath.
    let want_kind = req
        .span_kind
        .map(parse_span_kind_filter)
        .transpose()
        .unwrap_or(None);
    let span_filter_active =
        req.span_kind.is_some() || req.span_id.is_some() || req.pass_index.is_some();
    let resolve_accepted = |tp_index_for_id: usize| -> Option<std::collections::HashSet<u32>> {
        if !span_filter_active {
            return None;
        }
        let tc = state.session.get_toolpath_config(tp_index_for_id)?;
        let rt = state.gui.toolpath_rt.get(&tc.id)?;
        let result = rt.result.as_ref()?;
        let spans = result.spans();
        let mut set = std::collections::HashSet::<u32>::new();
        for (i, span) in spans.iter().enumerate() {
            let id = i as u32;
            let mut accept = true;
            if let Some(kind) = want_kind {
                accept &= span.kind == kind;
            }
            if let Some(want_id) = req.span_id {
                accept &= id == want_id;
            }
            if let Some(want_pi) = req.pass_index {
                accept &= matches!(
                    &span.payload,
                    Some(SpanPayload::DepthPass { pass_index, .. }) if *pass_index == want_pi
                );
            }
            if accept {
                set.insert(id);
            }
        }
        Some(set)
    };
    // Build a {toolpath_id_raw → accepted SpanId set}. Toolpath_id arg is
    // the project-level raw id (matching SimulationCutSample.toolpath_id),
    // while accepted-set lookup needs the index — translate via session.
    let toolpath_id = req.toolpath_id.map(rs_cam_core::ToolpathId);
    let mut accepted_by_toolpath: std::collections::HashMap<
        rs_cam_core::ToolpathId,
        Option<std::collections::HashSet<u32>>,
    > = std::collections::HashMap::new();
    if span_filter_active {
        let n = state.session.toolpath_count();
        for idx in 0..n {
            if let Some(tc) = state.session.get_toolpath_config(idx)
                && toolpath_id.is_none_or(|raw_id| tc.id == raw_id)
            {
                accepted_by_toolpath.insert(tc.id, resolve_accepted(idx));
            }
        }
    }
    let span_path_matches = |tp_id: rs_cam_core::ToolpathId, path: &[SpanId]| -> bool {
        if !span_filter_active {
            return true;
        }
        match accepted_by_toolpath.get(&tp_id) {
            Some(Some(accepted)) => path.iter().any(|sid| accepted.contains(&sid.0)),
            _ => false,
        }
    };

    let summaries: Vec<&_> = ct
        .semantic_summaries
        .iter()
        .filter(|s| toolpath_id.is_none_or(|id| s.toolpath_id == id))
        .collect();
    let hotspots: Vec<&_> = ct
        .hotspots
        .iter()
        .filter(|h| toolpath_id.is_none_or(|id| h.toolpath_id == id))
        .filter(|h| span_path_matches(h.toolpath_id, &h.span_path))
        .collect();
    let issues: Vec<&_> = ct
        .issues
        .iter()
        .filter(|i| toolpath_id.is_none_or(|id| i.toolpath_id == id))
        .filter(|i| span_path_matches(i.toolpath_id, &i.span_path))
        .collect();

    let hotspot_count = hotspots.len();
    let issue_count = issues.len();

    let mut resp = BoundedResponse::new(req.caps.max_response_bytes);

    // ── Always-kept scalars and vocabulary ───────────────────────────────
    //
    // R-2 (census §3.5 D4): this response carried TWO fields named
    // `issue_count` measuring different populations — the top-level one
    // (this request's filters applied) and `summary.issue_count` nested
    // inside `summary` (the whole trace, filters ignored). An agent
    // reading a filtered response could pick either and both looked
    // authoritative. The legacy keys keep their exact values for wire
    // compatibility; the disambiguating names sit beside them and say
    // which population each counts.
    resp.insert_always("hotspot_count", serde_json::json!(hotspot_count));
    resp.insert_always("issue_count", serde_json::json!(issue_count));
    // Explicit aliases for the two same-named counts, so neither has
    // to be inferred from where it sits in the object.
    resp.insert_always(
        "issue_count_matching_filter",
        serde_json::json!(issue_count),
    );
    resp.insert_always(
        "issue_count_project_wide",
        serde_json::json!(ct.summary.issue_count),
    );
    // And what the number actually IS: coalesced contiguous
    // air/low-engagement RUNS, not per-sample tallies. The per-sample
    // tallies are `air_cut_issue_count` / `low_engagement_issue_count`
    // on the semantic summaries, and on the census fixture they were
    // 43x larger under a near-identical name.
    resp.insert_always(
        "issue_count_population",
        serde_json::json!("coalesced_segments"),
    );
    resp.insert_always(
        "span_summaries_order",
        serde_json::json!(rs_cam_mcp::response::ORDERING_SPAN_SUMMARIES),
    );
    resp.insert_always(
        "semantic_summaries_order",
        serde_json::json!(rs_cam_mcp::response::ORDERING_SEMANTIC_SUMMARIES),
    );

    // `summary` is the headline block and is never a candidate for
    // dropping — it is 3.3 kB on the census fixture and every other number
    // in the response is read against it.
    resp.insert_always(
        "summary",
        serde_json::to_value(&ct.summary).unwrap_or_else(|_| serde_json::json!({})),
    );

    // ── Capped arrays, cheapest first ────────────────────────────────────
    let hotspots_arr = cap_json_values(
        hotspot_count,
        hotspots.iter().filter_map(|h| serde_json::to_value(h).ok()),
        max_h,
        resp.budget_mut(),
    );
    resp.insert_capped("hotspots", hotspots_arr);

    let issues_arr = cap_json_values(
        issue_count,
        issues.iter().filter_map(|i| serde_json::to_value(i).ok()),
        max_i,
        resp.budget_mut(),
    );
    resp.insert_capped("issues", issues_arr);

    // §6.E / Step 3 PR2 — drill-native outputs. `drill_summaries` always
    // surfaces (per-toolpath summary block, compact). `drill_samples` is
    // gated by `include_drill_samples` because the per-peck stream can be
    // verbose on cycles with many holes.
    let drill_summaries: Vec<&_> = ct
        .drill_summaries
        .iter()
        .filter(|s| toolpath_id.is_none_or(|id| s.toolpath_id == id))
        .collect();
    let drill_summaries_arr = cap_json_values(
        drill_summaries.len(),
        drill_summaries
            .iter()
            .filter_map(|s| serde_json::to_value(s).ok()),
        req.caps.drill_summaries,
        resp.budget_mut(),
    );
    resp.insert_capped("drill_summaries", drill_summaries_arr);

    // P0 unified-finishing probe — compact per-toolpath runtime block.
    // `runtime_by_intent` is the F-034 integrator time bucketed by
    // MoveIntent class (None when the sim ran without kinematics).
    use rs_cam_core::stock::simulation_cut::AirCutRatios;
    let toolpath_summaries: Vec<&_> = ct
        .toolpath_summaries
        .iter()
        .filter(|s| toolpath_id.is_none_or(|id| s.toolpath_id == id))
        .collect();
    let toolpath_summaries_arr = cap_json_values(
        toolpath_summaries.len(),
        toolpath_summaries.iter().map(|s| {
            serde_json::json!({
                "toolpath_id": s.toolpath_id,
                "total_runtime_s": s.total_runtime_s,
                "cutting_runtime_s": s.cutting_runtime_s,
                "rapid_runtime_s": s.rapid_runtime_s,
                "air_cut_time_s": s.air_cut_time_s,
                // LH-1: both denominators, both named. Thresholds follow
                // the total-runtime reading; the MCP narration line
                // reports the cutting-time one.
                "air_cut_pct_of_total_runtime": s.air_cut_pct_of_total_runtime(),
                "air_cut_pct_of_cutting_time": s.air_cut_pct_of_cutting_time(),
                "low_engagement_time_s": s.low_engagement_time_s,
                "metrics_not_applicable": s.metrics_not_applicable,
                "runtime_by_intent": s.runtime_by_intent,
            })
        }),
        req.caps.toolpath_summaries,
        resp.budget_mut(),
    );
    resp.insert_capped("toolpath_summaries", toolpath_summaries_arr);

    let semantic_arr = cap_json_values(
        summaries.len(),
        summaries
            .iter()
            .filter_map(|s| serde_json::to_value(s).ok()),
        req.caps.semantic_summaries,
        resp.budget_mut(),
    );
    resp.insert_capped("semantic_summaries", semantic_arr);

    if req.include_drill_samples {
        let drill_samples: Vec<&_> = ct
            .drill_samples
            .iter()
            .filter(|s| toolpath_id.is_none_or(|id| s.toolpath_id == id))
            .collect();
        let drill_samples_arr = cap_json_values(
            drill_samples.len(),
            drill_samples
                .iter()
                .filter_map(|s| serde_json::to_value(s).ok()),
            req.caps.drill_samples,
            resp.budget_mut(),
        );
        resp.insert_capped("drill_samples", drill_samples_arr);
    } else {
        // Not requested is not "did not fit": `null` says the caller
        // declined the array, and no truncation keys are emitted for it.
        resp.insert_always("drill_samples", serde_json::Value::Null);
    }

    // ── The big one, last ────────────────────────────────────────────────
    let span_arr = build_span_cut_summaries(
        state,
        ct,
        toolpath_id,
        span_filter_active,
        &accepted_by_toolpath,
        req.caps.span_summaries,
        resp.budget_mut(),
    );
    resp.insert_capped("span_summaries", span_arr);

    Ok(resp.finish())
}

/// Translate a `SimulationResult::mesh` from the simulator's ZERO-ROOTED
/// stock-relative frame back into world coordinates.
///
/// The screenshot exporters composite the sim mesh with toolpaths taken raw
/// out of `gui.toolpath_rt`, i.e. in their **emission** frame, and the
/// combined scene is centroid-normalised — so only the RELATIVE offset
/// between mesh and path matters. Identity setups emit in world frame, so
/// this shift is what puts the two in register. (Non-identity setups emit
/// in their own local frame, which differs from the mesh by a flip rather
/// than a translation; that overlay was never in register and this does not
/// change it. The GUI viewport does not go through here — it maps
/// everything into the zero-rooted display frame instead, via
/// `Setup::emission_to_display_shift`.)
pub(super) fn sim_mesh_in_world_frame(
    mesh: &rs_cam_core::stock::stock_mesh::StockMesh,
    session: &rs_cam_core::session::ProjectSession,
) -> rs_cam_core::stock::stock_mesh::StockMesh {
    let min = session.stock_bbox().min;
    let (ox, oy, oz) = (min.x as f32, min.y as f32, min.z as f32);
    if ox == 0.0 && oy == 0.0 && oz == 0.0 {
        return mesh.clone();
    }
    let mut out = rs_cam_core::stock::stock_mesh::StockMesh::empty();
    out.append_transformed(mesh, |x, y, z| (x + ox, y + oy, z + oz));
    out
}

/// Build the `span_summaries` array under an item cap and a byte budget.
///
/// The accumulation pass runs over **every** matching span regardless of the
/// cap, because `total_matching` has to be the true pre-cap population — a
/// count inferred from what was emitted would report a truncated array as a
/// complete one. Only the JSON construction is skipped past the cap, and
/// that is where the bytes were: B-1 measured this array at 52,852,388 of a
/// 56,225,225-byte response, 35,838 entries averaging 1,475 bytes.
///
/// Order is the documented [`rs_cam_mcp::response::ORDERING_SPAN_SUMMARIES`]:
/// toolpath index ascending, then span id ascending.
#[allow(clippy::too_many_arguments)]
fn build_span_cut_summaries(
    state: &crate::state::AppState,
    trace: &rs_cam_core::stock::simulation_cut::SimulationCutTrace,
    toolpath_id: Option<rs_cam_core::ToolpathId>,
    span_filter_active: bool,
    accepted_by_toolpath: &std::collections::HashMap<
        rs_cam_core::ToolpathId,
        Option<std::collections::HashSet<u32>>,
    >,
    cap: usize,
    budget: &mut rs_cam_mcp::response::ResponseBudget,
) -> rs_cam_mcp::response::CappedArray {
    use rs_cam_mcp::response::{CappedArray, ResponseBudget};

    let mut out = Vec::new();
    let mut total_matching = 0usize;
    // The enclosing `[]`, charged once — mirrors `cap_json_values`.
    budget.charge(2);
    let n = state.session.toolpath_count();
    for idx in 0..n {
        let Some(tc) = state.session.get_toolpath_config(idx) else {
            continue;
        };
        if toolpath_id.is_some_and(|id| tc.id != id) {
            continue;
        }
        let Some(rt) = state.gui.toolpath_rt.get(&tc.id) else {
            continue;
        };
        let Some(result) = rt.result.as_ref() else {
            continue;
        };
        if !result.spans_valid() {
            continue;
        }
        let spans = result.spans();
        // C1: ONE pass over the trace for this toolpath, scattering each
        // sample into every span of its path. An always-reject filter (a
        // span filter that matched nothing for this toolpath) short-circuits
        // to an empty accept set rather than scanning at all.
        let accepted: Option<std::collections::HashSet<u32>> = if span_filter_active {
            Some(
                accepted_by_toolpath
                    .get(&tc.id)
                    .and_then(|set| set.clone())
                    .unwrap_or_default(),
            )
        } else {
            None
        };
        let accs = rs_cam_core::stock::simulation_cut::accumulate_by_span(
            &trace.samples,
            tc.id,
            spans.len(),
            accepted.as_ref(),
        );
        for (span_index, span) in spans.iter().enumerate() {
            let span_id = span_index as u32;

            use rs_cam_core::stock::simulation_cut::AirCutRatios;
            let Some(acc) = accs.get(span_index) else {
                continue;
            };
            if acc.sample_count == 0 {
                continue;
            }
            // Counted before the cap is consulted: `total_matching` must be
            // the population, not the emission.
            total_matching += 1;
            if out.len() >= cap {
                continue;
            }
            // Roadmap F.9 — the chipload key carries the per-sample
            // raw peak (typically several × the chipload gate's
            // `median_low` statistic on a 2D pocket / 3D rough). The
            // explicit `per_sample_peak_*` prefix makes the statistic
            // inline so agents/operators don't read it as a gate
            // exceedance. The gate's own statistic appears separately
            // on the load report's `chipload` verdict (see
            // `ChiploadMetric.statistic`).
            let row = serde_json::json!({
                "toolpath_id": tc.id,
                "span_id": span_id,
                "kind": span.kind.label(),
                "label": &*span.label,
                "payload": span.payload.as_ref().map(|p| format!("{p:?}")),
                "start_move": span.start_move,
                "end_move": span.end_move,
                "is_boundary": span.is_boundary(),
                "sample_count": acc.sample_count,
                "total_runtime_s": acc.total_runtime_s,
                "cutting_runtime_s": acc.cutting_runtime_s,
                "rapid_runtime_s": acc.rapid_runtime_s,
                "air_cut_time_s": acc.air_cut_time_s,
                // LH-1: span-scope air cut, both denominators named.
                "air_cut_pct_of_total_runtime": acc.air_cut_pct_of_total_runtime(),
                "air_cut_pct_of_cutting_time": acc.air_cut_pct_of_cutting_time(),
                "low_engagement_time_s": acc.low_engagement_time_s,
                "wasted_runtime_s": acc.air_cut_time_s + acc.low_engagement_time_s,
                "average_engagement": acc.average_engagement(),
                "per_sample_peak_chipload_mm_per_tooth": acc.peak_chipload_mm_per_tooth,
                "peak_axial_doc_mm": acc.peak_axial_doc_mm,
                "peak_plunge_descent_mm": acc.peak_plunge_descent_mm,
                "total_removed_volume_est_mm3": acc.total_removed_volume_est_mm3,
                "average_mrr_mm3_s": acc.average_mrr(),
                // Step 2 D — per-kinematics axes inline so agents can read
                // axial-DOC for plunge-heavy passes and arc-WOC for lateral
                // ones without parsing a separate summary. See
                // planning/DEXEL_Z_ONLY_INVESTIGATION.md §6.D.
                "per_kinematics": render_per_kinematics_json(acc),
            });
            let cost = ResponseBudget::cost_of(&row).saturating_add(1);
            if budget.would_fit(cost) {
                budget.charge(cost);
                out.push(row);
            }
            // A row that does not fit is simply not emitted; the shortfall
            // shows up as `span_summaries_returned < _total_matching` with
            // `_truncated: true`, which is the honest report either way.
        }
    }
    CappedArray::from_parts(out, total_matching, cap)
}

/// Build the per-DepthPass histogram for [`mcp_get_tool_load_report`].
/// Returns `{ "<toolpath_raw_id>": [ { ... }, … ] }` keyed by stringified
/// toolpath id. Toolpaths without DepthPass spans or without samples in the
/// trace are omitted.
pub(super) fn build_per_depth_pass_summary(
    state: &crate::state::AppState,
    sim_trace: Option<&rs_cam_core::stock::simulation_cut::SimulationCutTrace>,
) -> serde_json::Value {
    use rs_cam_core::trace::toolpath_spans::{SpanId, SpanKind, SpanPayload};

    let Some(trace) = sim_trace else {
        return serde_json::Value::Null;
    };

    let mut out = serde_json::Map::<String, serde_json::Value>::new();
    let n = state.session.toolpath_count();
    for idx in 0..n {
        let Some(tc) = state.session.get_toolpath_config(idx) else {
            continue;
        };
        let Some(rt) = state.gui.toolpath_rt.get(&tc.id) else {
            continue;
        };
        let Some(result) = rt.result.as_ref() else {
            continue;
        };
        let spans = result.spans();
        if !result.spans_valid() || spans.is_empty() {
            continue;
        }
        // Map each DepthPass span vec-index to its (z_level, pass_index)
        // payload, filling defaults when payload is missing.
        let mut depth_pass_meta: Vec<(usize, Option<f64>, Option<u32>)> = Vec::new();
        for (i, span) in spans.iter().enumerate() {
            if span.kind == SpanKind::DepthPass {
                let (z, p) = match &span.payload {
                    Some(SpanPayload::DepthPass {
                        z_level,
                        pass_index,
                    }) => (Some(*z_level), Some(*pass_index)),
                    _ => (None, None),
                };
                depth_pass_meta.push((i, z, p));
            }
        }
        if depth_pass_meta.is_empty() {
            continue;
        }
        let depth_pass_ids: std::collections::HashSet<u32> =
            depth_pass_meta.iter().map(|(i, _, _)| *i as u32).collect();
        // Accumulate per-pass stats in lock-step with depth_pass_meta.
        let mut accs: Vec<rs_cam_core::stock::simulation_cut::SummaryAccumulator> = (0
            ..depth_pass_meta.len())
            .map(|_| rs_cam_core::stock::simulation_cut::SummaryAccumulator::default())
            .collect();
        let pass_index_of: std::collections::HashMap<u32, usize> = depth_pass_meta
            .iter()
            .enumerate()
            .map(|(i, (sid, _, _))| (*sid as u32, i))
            .collect();

        for sample in trace.samples.iter().filter(|s| s.toolpath_id == tc.id) {
            // Find the DepthPass id in this sample's span_path.
            let pass_pos = sample.span_path.iter().find_map(|SpanId(id)| {
                if depth_pass_ids.contains(id) {
                    pass_index_of.get(id).copied()
                } else {
                    None
                }
            });
            if let Some(pos) = pass_pos {
                #[allow(clippy::indexing_slicing)] // pos came from pass_index_of
                accs[pos].observe(sample);
            }
        }

        let entries: Vec<serde_json::Value> = depth_pass_meta
            .iter()
            .zip(accs)
            .map(|((span_id, z, pass_idx), acc)| {
                serde_json::json!({
                    "span_id": *span_id,
                    "z_level": z,
                    "pass_index": pass_idx,
                    "sample_count": acc.sample_count,
                    "cutting_runtime_s": acc.cutting_runtime_s,
                    "total_removed_volume_est_mm3": acc.total_removed_volume_est_mm3,
                    "average_mrr_mm3_s": acc.average_mrr(),
                    "average_engagement": acc.average_engagement(),
                    // Roadmap F.9 — name the field by its statistic so
                    // operators don't read this as the gate-trip value.
                    // The chipload gate uses `median_low(samples)` on
                    // the burn side; this RAW per-sample peak can be
                    // ~4× larger and looks like an exceedance when read
                    // in isolation. See planning/UX_PAIN_POINTS_2026-05-11.md F.9.
                    "per_sample_peak_chipload_mm_per_tooth": acc.peak_chipload_mm_per_tooth,
                    "peak_axial_doc_mm": acc.peak_axial_doc_mm,
                    "peak_plunge_descent_mm": acc.peak_plunge_descent_mm,
                    "per_kinematics": render_per_kinematics_json(&acc),
                })
            })
            .collect();
        out.insert(tc.id.to_string(), serde_json::Value::Array(entries));
    }

    if out.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Object(out)
    }
}

/// Render a `SummaryAccumulator`'s per-`CutKinematics` sub-accumulators as
/// inline JSON for MCP per-span / per-depth-pass output. Step 2 D — exposes
/// axial-DOC for plunge-heavy passes, arc-WOC for lateral ones, etc. without
/// reshuffling the surrounding payload. Empty when no cutting samples were
/// observed in this scope.
fn render_per_kinematics_json(
    acc: &rs_cam_core::stock::simulation_cut::SummaryAccumulator,
) -> serde_json::Value {
    use rs_cam_core::stock::simulation_cut::CutKinematics;
    let mut map = serde_json::Map::new();
    for kind in CutKinematics::ALL {
        #[allow(clippy::indexing_slicing)] // SAFETY: `kind.index()` is bounded by COUNT.
        let sub = &acc.per_kinematics[kind.index()];
        if sub.sample_count == 0 {
            continue;
        }
        let summary = sub.clone().finish();
        let label = match kind {
            CutKinematics::Linear => "linear",
            CutKinematics::Plunge => "plunge",
            CutKinematics::Helix => "helix",
            CutKinematics::Arc => "arc",
            CutKinematics::Rapid => "rapid",
        };
        map.insert(
            label.to_owned(),
            serde_json::json!({
                "cutting_runtime_s": summary.cutting_runtime_s,
                "sample_count": summary.sample_count,
                "average_radial_woc_fraction": summary.average_radial_woc_fraction,
                "peak_radial_woc_fraction": summary.peak_radial_woc_fraction,
                "average_axial_doc_fraction": summary.average_axial_doc_fraction,
                "peak_axial_doc_fraction": summary.peak_axial_doc_fraction,
                "peak_axial_doc_mm": summary.peak_axial_doc_mm,
                "peak_plunge_descent_mm": summary.peak_plunge_descent_mm,
                "average_arc_radians": summary.average_arc_radians,
                "average_mean_chip_thickness_mm": summary.average_mean_chip_thickness_mm,
                "peak_chip_thickness_mm": summary.peak_chip_thickness_mm,
                "average_leading_edge_speed_mm_min": summary.average_leading_edge_speed_mm_min,
            }),
        );
    }
    serde_json::Value::Object(map)
}
