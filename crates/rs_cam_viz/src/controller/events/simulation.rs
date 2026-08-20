use std::sync::Arc;

use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::session::ToolpathConfig;

use crate::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, SetupSimGroup, SetupSimToolpath,
    SimulationRequest,
};
use crate::state::toolpath::ToolpathId;

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    // ── Simulation helpers ───────────────────────────────────────────────

    pub(crate) fn handle_reset_simulation(&mut self) {
        self.invalidate_simulation();
    }

    /// Clear all cached simulation state.
    pub(crate) fn invalidate_simulation(&mut self) {
        self.compute.cancel_lane(ComputeLane::Analysis);
        let sim = &mut self.state.simulation;
        sim.results = None;
        sim.playback = Default::default();
        sim.checks = Default::default();
        sim.last_run = None;
        self.collision_positions.clear();
        self.pending_upload = true;
    }

    pub(crate) fn handle_sim_jump_to_move(&mut self, move_idx: usize) {
        if self.state.simulation.has_results() {
            let total = self.state.simulation.total_moves();
            self.state.simulation.playback.playing = false;
            self.state.simulation.playback.current_move = move_idx.min(total);
        }
    }

    pub(crate) fn handle_sim_step_forward(&mut self) {
        if self.state.simulation.has_results() {
            let total = self.state.simulation.total_moves();
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = (pb.current_move + 1).min(total);
        }
    }

    pub(crate) fn handle_sim_step_backward(&mut self) {
        if self.state.simulation.has_results() {
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = pb.current_move.saturating_sub(1);
        }
    }

    pub(crate) fn handle_sim_jump_to_start(&mut self) {
        if self.state.simulation.has_results() {
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = 0;
        }
    }

    pub(crate) fn handle_sim_jump_to_end(&mut self) {
        if self.state.simulation.has_results() {
            let total = self.state.simulation.total_moves();
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = total;
        }
    }

    pub(crate) fn handle_sim_jump_to_op_start(&mut self, boundary_idx: usize) {
        if let Some(start) = self
            .state
            .simulation
            .boundaries()
            .get(boundary_idx)
            .map(|b| b.start_move)
        {
            self.state.simulation.playback.playing = false;
            self.state.simulation.playback.current_move = start;
        }
    }

    pub(crate) fn handle_sim_jump_to_op_end(&mut self, boundary_idx: usize) {
        if let Some(end) = self
            .state
            .simulation
            .boundaries()
            .get(boundary_idx)
            .map(|b| b.end_move)
        {
            self.state.simulation.playback.playing = false;
            self.state.simulation.playback.current_move = end;
        }
    }

    /// Build per-setup simulation groups.
    pub(crate) fn build_simulation_groups(
        &self,
        mut include_toolpath: impl FnMut(usize, &ToolpathConfig) -> bool,
        mut stop_after_setup: impl FnMut(usize) -> bool,
    ) -> Option<(Vec<SetupSimGroup>, Vec<SetupSimToolpath>, BoundingBox3)> {
        // F-024 third-site fix: the world `stock_bbox` forwarded to the
        // worker must respect the stock's origin offset. Previously this
        // constructed a zero-rooted bbox (`(0,0,0)..(stock.x, stock.y,
        // stock.z)`), which dropped `stock.origin_{x,y,z}`. For AS001
        // (`origin_z = -12`) the toolpath cuts at world Z=-2 while the
        // bbox said the stock spanned Z=[0, 12] — the dexel grid never
        // matched the toolpath frame and `axial_engagement_mm` read the
        // full stock height instead of the commanded DOC. `ProjectSession::
        // stock_bbox()` delegates to `StockConfig::bbox()` which applies
        // the origin correctly.
        let stock_bbox = build_world_stock_bbox(&self.state.session);

        let mut groups: Vec<SetupSimGroup> = Vec::new();
        let mut all_toolpaths_flat = Vec::new();

        for (i, setup) in self.state.session.list_setups().iter().enumerate() {
            let mut toolpaths: Vec<SetupSimToolpath> = Vec::new();
            // F.4: mirrors `ProjectSession::run_simulation`'s phantom-
            // prior-stock scan (`rs_cam_core::compute::simulate::
            // PhantomPriorStockScan`) so the core and GUI builders can't
            // drift — see that type's doc comment for the validity rule.
            // Walked over every toolpath config in plan order, not just
            // the ones `include_toolpath` admits: "has this been
            // generated yet?" is session/GUI-runtime state, independent
            // of this particular request's scope.
            let mut phantom_scan = rs_cam_core::compute::simulate::PhantomPriorStockScan::default();
            for &tp_idx in &setup.toolpath_indices {
                let Some(tc) = self.state.session.toolpath_configs().get(tp_idx) else {
                    continue;
                };
                let rt = self.state.gui.toolpath_rt.get(&tc.id);
                let result = rt.and_then(|rt| rt.result.as_ref());
                phantom_scan.visit(
                    toolpaths.len(),
                    tc.enabled,
                    result.is_some(),
                    tc.id,
                    tc.stock_source,
                );

                if !include_toolpath(i, tc) {
                    continue;
                }
                // Re-derive `rt`/`result` together (rather than reusing the
                // scan's `rt`/`result` locals above) so neither needs an
                // `.unwrap()` to recover the other.
                let Some(rt) = rt else { continue };
                let Some(result) = rt.result.as_ref() else {
                    continue;
                };
                let Some(tool) = self
                    .state
                    .session
                    .tools()
                    .iter()
                    .find(|t| t.id.0 == tc.tool_id)
                else {
                    continue;
                };
                let op_type = tc.operation.op_type();
                let metrics_not_applicable = matches!(
                    op_type,
                    rs_cam_core::compute::catalog::OperationType::Drill
                        | rs_cam_core::compute::catalog::OperationType::AlignmentPinDrill
                );
                toolpaths.push(SetupSimToolpath {
                    id: tc.id,
                    name: tc.name.clone(),
                    annotated: Arc::clone(&result.annotated),
                    tool: tool.clone(),
                    semantic_trace: rt.semantic_trace.clone(),
                    spindle_rpm: tc.operation.spindle_rpm(),
                    metrics_not_applicable,
                    drill_op: result.drill_op.clone(),
                    operation_config_hash: rs_cam_core::compute::simulate::hash_operation_config(
                        &tc.operation,
                    ),
                });
            }

            let phantom_prior_stock = phantom_scan.finish();
            // F.4: a setup whose every toolpath is still ungenerated
            // builds an empty `toolpaths` vec — but if the FIRST enabled
            // config in plan order is a pending `FromRemainingStock` op,
            // the "stock before it" is simply the setup's untouched
            // initial stock (zero predecessors to distrust), so the
            // phantom is still valid and the group must still be
            // emitted (with an empty `toolpaths` vec) to carry it.
            if !toolpaths.is_empty() || phantom_prior_stock.is_some() {
                all_toolpaths_flat.extend(toolpaths.clone());

                // F-030: drive per-setup frame decisions through the shared
                // `SetupEvalContext` so the viz controller, viz worker, and
                // core simulate paths can never diverge again.
                let setup_ctx = rs_cam_core::session::SetupEvalContext::build(
                    &self.state.session,
                    setup.face_up,
                    setup.z_rotation,
                );

                groups.push(SetupSimGroup {
                    toolpaths,
                    local_stock_bbox: setup_ctx.local_stock_bbox,
                    local_to_global: setup_ctx.local_to_global,
                    phantom_prior_stock,
                });
            }

            if stop_after_setup(i) {
                break;
            }
        }

        if groups.is_empty() {
            return None;
        }
        Some((groups, all_toolpaths_flat, stock_bbox))
    }

    /// Submit a simulation request.
    pub(crate) fn submit_simulation_for_groups(
        &mut self,
        groups: Vec<SetupSimGroup>,
        all_toolpaths_flat: &[SetupSimToolpath],
        stock_bbox: BoundingBox3,
        _model_setup_idx: Option<usize>,
        memoize_prefix: bool,
    ) {
        if self.state.simulation.auto_resolution {
            self.state.simulation.resolution =
                auto_resolution_for_tools(all_toolpaths_flat, &stock_bbox);
        }

        let model_mesh = self
            .state
            .session
            .models()
            .iter()
            .find_map(|m| m.mesh.clone());

        if self.state.simulation.metric_options.enabled {
            self.state.simulation.metric_options.capture_arc_engagement = true;
        }

        let machine = self.state.session.machine();
        let max_feed_mm_min = machine.max_feed_mm_min.max(1.0);
        let kinematics = machine.kinematics;
        self.compute.submit_simulation(SimulationRequest {
            groups,
            stock_bbox,
            stock_top_z: stock_bbox.max.z,
            resolution: self.state.simulation.resolution,
            metric_options: self.state.simulation.metric_options,
            spindle_rpm: self.state.gui.post.spindle_speed,
            rapid_feed_mm_min: if self.state.gui.post.high_feedrate_mode {
                self.state.gui.post.high_feedrate.max(1.0)
            } else {
                max_feed_mm_min
            },
            model_mesh,
            // F-035 — forward the active machine kinematics + max feed
            // so the worker can populate the core
            // `KinematicsContext`. The GUI doesn't yet expose a
            // toggle for `use_predicted_feed_in_gates`; the flag
            // defaults to `false`, preserving pre-F-035 gate
            // verdicts. When the GUI grows an opt-in (F-036
            // territory), the toggle reads off the simulation panel
            // state and threads here.
            kinematics,
            use_predicted_feed_in_gates: false,
            max_feed_mm_min,
            memoize_prefix,
        });
    }

    /// Simulate every enabled toolpath. Returns `false` when there was
    /// nothing to simulate and no request was submitted — A/M11's fixpoint
    /// loop must know that, or it would wait forever for a completion that
    /// will never drain.
    pub(crate) fn run_simulation_with_all(&mut self) -> bool {
        self.run_simulation_with_all_memoized(false)
    }

    /// [`Self::run_simulation_with_all`] with control over the S5 prefix memo.
    ///
    /// `memoize_prefix` is set **only** by the `generate_all` fixpoint ladder
    /// (`controller::events::compute::settle_generate_all_round`), the one
    /// caller that runs several simulations over a growing project back to
    /// back. Every other simulation still consumes a held snapshot when it
    /// matches, but leaves none behind.
    pub(crate) fn run_simulation_with_all_memoized(&mut self, memoize_prefix: bool) -> bool {
        let Some((groups, all_toolpaths_flat, stock_bbox)) =
            self.build_simulation_groups(|_setup_idx, tc| tc.enabled, |_setup_idx| false)
        else {
            tracing::warn!("No computed toolpaths to simulate");
            self.push_notification(
                "No computed toolpaths to simulate".into(),
                super::super::Severity::Warning,
            );
            return false;
        };
        self.submit_simulation_for_groups(
            groups,
            &all_toolpaths_flat,
            stock_bbox,
            Some(0),
            memoize_prefix,
        );
        true
    }

    pub(crate) fn run_simulation_with_ids(&mut self, ids: &[ToolpathId]) {
        let target_setup_idx = self.state.session.list_setups().iter().position(|s| {
            s.toolpath_indices.iter().any(|&tp_idx| {
                self.state
                    .session
                    .toolpath_configs()
                    .get(tp_idx)
                    .is_some_and(|tc| ids.contains(&tc.id))
            })
        });
        let Some(target_setup_idx) = target_setup_idx else {
            tracing::warn!("No computed toolpaths to simulate");
            self.push_notification(
                "No computed toolpaths to simulate".into(),
                super::super::Severity::Warning,
            );
            return;
        };

        let Some((groups, all_toolpaths_flat, stock_bbox)) = self.build_simulation_groups(
            |setup_idx, tc| {
                if setup_idx == target_setup_idx {
                    ids.contains(&tc.id)
                } else if setup_idx < target_setup_idx {
                    tc.enabled
                } else {
                    false
                }
            },
            |setup_idx| setup_idx == target_setup_idx,
        ) else {
            return;
        };
        self.submit_simulation_for_groups(
            groups,
            &all_toolpaths_flat,
            stock_bbox,
            Some(target_setup_idx),
            false,
        );
    }

    pub(crate) fn request_collision_check(&mut self) {
        // Find first toolpath with a result and matching tool/model
        let toolpath_data = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .enumerate()
            .find_map(|(index, tc)| {
                let rt = self.state.gui.toolpath_rt.get(&tc.id)?;
                let result = rt.result.as_ref()?;
                let tool = self
                    .state
                    .session
                    .tools()
                    .iter()
                    .find(|t| t.id.0 == tc.tool_id)?
                    .clone();
                let mesh = self
                    .state
                    .session
                    .models()
                    .iter()
                    .find(|m| m.id == tc.model_id)
                    .and_then(|m| m.mesh.clone())?;
                // W0.1 — carry the setup's fixtures so the GUI check
                // flags holder-vs-clamp crashes, not just mesh hits.
                let obstacles = self.state.session.collision_obstacles_for_toolpath(index);
                Some((Arc::clone(&result.annotated), tool, mesh, obstacles))
            });

        if let Some((annotated, tool, mesh, obstacles)) = toolpath_data {
            self.compute.submit_collision(CollisionRequest {
                annotated,
                tool,
                mesh,
                obstacles,
            });
        } else {
            tracing::warn!("No toolpath with STL mesh available for collision check");
            self.push_notification(
                "No toolpath with STL mesh available for collision check".into(),
                super::super::Severity::Warning,
            );
        }
    }
}

/// Build the world-frame stock bbox for a simulation request.
///
/// Delegates to `ProjectSession::stock_bbox()` (which applies
/// `StockConfig::origin_{x,y,z}`). Factored out as a free function so
/// the controller's bbox construction can be exercised from unit tests
/// without needing a full `AppController<B>` instance.
///
/// **F-024 third-site fix (2026-05-25)**: the prior inline construction
/// in `build_simulation_groups` was `(0,0,0)..(stock.x, stock.y, stock.z)`
/// — it ignored the stock origin. For AS001 (`origin_z = -12`) the world
/// bbox should be `(-10,-10,-12)..(90, 90, 0)` but was being sent as
/// `(0,0,0)..(100, 100, 12)`. The viz worker's
/// `build_core_simulation_request` falls back to `request.stock_bbox`
/// when `local_to_global` is `None` (the F-024 follow-up landed in commit
/// `0c907a6`), so the broken bbox was the dexel grid's Z range — toolpath
/// cuts at world Z=-2 sat below every ray and `axial_engagement_mm` read
/// the full stock height instead of the commanded DOC.
pub(crate) fn build_world_stock_bbox(
    session: &rs_cam_core::session::ProjectSession,
) -> BoundingBox3 {
    session.stock_bbox()
}

/// Auto-resolution calculation.
fn auto_resolution_for_tools(toolpaths: &[SetupSimToolpath], stock_bbox: &BoundingBox3) -> f64 {
    let min_radius = toolpaths
        .iter()
        .map(|toolpath| toolpath.tool.diameter / 2.0)
        .fold(f64::INFINITY, f64::min);

    let from_tool = (min_radius / 5.0).clamp(0.02, 0.5);

    let max_cells: f64 = 8_000_000.0;
    let sx = stock_bbox.max.x - stock_bbox.min.x;
    let sy = stock_bbox.max.y - stock_bbox.min.y;
    let from_grid = ((sx * sy) / max_cells).sqrt().max(0.02);

    let resolution = from_tool.max(from_grid);

    tracing::info!(
        "Auto sim resolution: {:.3} mm (smallest tool \u{00D8}{:.2} mm, grid ~{}x{})",
        resolution,
        min_radius * 2.0,
        (sx / resolution).ceil() as usize,
        (sy / resolution).ceil() as usize,
    );

    resolution
}
