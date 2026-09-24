use std::sync::Arc;

use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::session::ToolpathConfig;

use rs_cam_core::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, entry_metrics_not_applicable, entry_tool_fields,
    group_stock_cut_direction,
};
use rs_cam_core::compute::tool_config::ToolConfig;

use crate::compute::{CollisionRequest, ComputeBackend, ComputeLane, SimulationRequest};
use crate::state::simulation::HolderCheckScope;
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
        sim.submitted_metric_options_revision = None;
        sim.submitted_edit_counter = None;
        sim.submitted_simulation_epoch = None;
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

    /// Write the ONE stored simulation resolution (G-RESTRES), through
    /// `Command::SetSimulationResolution`.
    ///
    /// The Simulation panel, MCP `run_simulation` and MCP `generate_all`
    /// all take this door. A new value is a project edit: the file carries
    /// it, the rest rows it stales read WAIT, and a simulation at the old
    /// cell is gone from the viewport too.
    ///
    /// # Errors
    /// The session's refusal of a value that is not a positive cell size.
    pub(crate) fn set_simulation_resolution(
        &mut self,
        resolution: rs_cam_core::session::SimulationResolution,
    ) -> Result<rs_cam_core::session::Effects, rs_cam_core::session::SessionError> {
        let changed = self.state.session.simulation_resolution() != resolution;
        let effects =
            self.state
                .session
                .apply(rs_cam_core::session::Command::SetSimulationResolution(
                    rs_cam_core::session::SetSimulationResolutionArgs { resolution },
                ))?;
        crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
        if effects.simulation_cleared {
            self.invalidate_simulation();
        }
        if changed {
            self.state.gui.mark_edited();
        }
        Ok(effects)
    }

    /// Build per-setup simulation groups.
    pub(crate) fn build_simulation_groups(
        &self,
        mut include_toolpath: impl FnMut(usize, &ToolpathConfig) -> bool,
        mut stop_after_setup: impl FnMut(usize) -> bool,
    ) -> Option<(Vec<SimGroupEntry>, Vec<ToolConfig>, BoundingBox3)> {
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

        let mut groups: Vec<SimGroupEntry> = Vec::new();
        let mut all_tools_flat: Vec<ToolConfig> = Vec::new();

        for (i, setup) in self.state.session.list_setups().iter().enumerate() {
            let mut toolpaths: Vec<SimToolpathEntry> = Vec::new();
            let mut tools: Vec<ToolConfig> = Vec::new();
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
                // G-RESTSTALE (M-C): the carve reads the CORE result, the
                // one `start` and every rest record compare against. The GUI
                // copy `rt.result` outlives a core drop on purpose (the
                // viewport keeps drawing it), so carving it put a superseded
                // toolpath into the rest snapshot while core held another.
                let rt = self.state.gui.toolpath_rt.get(&tc.id);
                let result = self.state.session.get_result(tp_idx);
                // G-STICKYEMPTY: `result.is_some()` is the RIGHT answer here
                // and the wrong one in core's `ProjectSession::run_simulation`
                // — the difference is that this builder pushes every
                // result-bearing toolpath, filtering none, so even a
                // zero-move entry reaches `prior_stocks.insert(entry.id, ..)`
                // in the simulator. Core's builder drops entries below
                // `MIN_SIMULATED_MOVES` and must therefore answer with
                // `contributes_simulated_motion`. If a short-toolpath filter
                // is ever added HERE, this call has to change with it; see
                // that function's doc for the failure it prevents.
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
                let Some(result) = result else {
                    continue;
                };
                // The semantic trace stays viz-side (the drain adopts
                // `semantic_trace: None`), so it comes from `rt`, and only
                // when `rt` holds the very toolpath core holds.
                let semantic_trace = rt
                    .filter(|rt| {
                        rt.result
                            .as_ref()
                            .is_some_and(|held| Arc::ptr_eq(&held.annotated, result.annotated()))
                    })
                    .and_then(|rt| rt.semantic_trace.clone());
                let Some(tool) = self
                    .state
                    .session
                    .tools()
                    .iter()
                    .find(|t| t.id.0 == tc.tool_id)
                else {
                    continue;
                };
                let metrics_not_applicable = entry_metrics_not_applicable(
                    result.is_drill_op(),
                    &result.annotated().toolpath,
                    tc.operation.op_type(),
                );
                let (tool_def, flute_count, tool_summary) = entry_tool_fields(tool);
                tools.push(tool.clone());
                toolpaths.push(SimToolpathEntry {
                    id: tc.id,
                    name: tc.name.clone(),
                    annotated: Arc::clone(result.annotated()),
                    tool: tool_def,
                    flute_count,
                    tool_summary,
                    semantic_trace,
                    spindle_rpm: tc.operation.spindle_rpm(),
                    metrics_not_applicable,
                    drill_op: result.drill_op().cloned(),
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
                all_tools_flat.extend(tools);

                // F-030: drive per-setup frame decisions through the shared
                // `SetupEvalContext` so the viz controller, viz worker, and
                // core simulate paths can never diverge again.
                let setup_ctx = rs_cam_core::session::SetupEvalContext::build(
                    &self.state.session,
                    setup.face_up,
                    setup.z_rotation,
                );

                groups.push(SimGroupEntry {
                    toolpaths,
                    direction: group_stock_cut_direction(setup.face_up),
                    // F-024's one door. An identity setup answers `None`, so
                    // the simulator grids the request's world `stock_bbox` —
                    // the frame the toolpath emits in. The viz worker
                    // re-derived this rule by hand from a non-optional mirror
                    // field until 2026-09-18 (CMP-19); this is the third and
                    // last site to take the shared accessor.
                    local_stock_bbox: setup_ctx.sim_local_stock_bbox(),
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
        Some((groups, all_tools_flat, stock_bbox))
    }

    /// Submit a simulation request.
    pub(crate) fn submit_simulation_for_groups(
        &mut self,
        groups: Vec<SimGroupEntry>,
        all_tools_flat: &[ToolConfig],
        stock_bbox: BoundingBox3,
        _model_setup_idx: Option<usize>,
        memoize_prefix: bool,
        for_plan: bool,
    ) {
        let _ = all_tools_flat;
        // G-RESTRES: ONE stored project value sets the cell of every
        // simulation — a plain Run Simulation, a plan prefix, the closing
        // run, MCP and the CLI (operator ruling 2026-09-24). `Auto` is
        // worked out over the whole project in core, never from the tools
        // of this request, so a prefix and a full run agree.
        let request_resolution = self.state.session.simulation_resolution_mm();

        let model_mesh = self
            .state
            .session
            .models()
            .iter()
            .find_map(|m| m.mesh.clone());

        if self.state.simulation.metric_options.enabled {
            self.state.simulation.metric_options.capture_arc_engagement = true;
        }
        // G-RESTRES: a plan's simulation feeds rest operations, so it carves
        // with the cutting-metrics kernel whatever the panel's capture toggle
        // says. The two kernels leave different stock, and the CLI and MCP
        // always carve with metrics; a rest snapshot records which kernel
        // made it, and only the metrics carve is current. The toggle stays a
        // view choice for a plain Run Simulation.
        let mut metric_options = self.state.simulation.metric_options;
        if for_plan {
            metric_options.enabled = true;
            metric_options.capture_arc_engagement = true;
        }

        // G-LATESIM (F2.10): record the project's edit state as it is NOW,
        // because that is the configuration this run answers. The drain used
        // to read the live counter when the result landed, which quietly
        // absorbed every edit made while the simulation ran.
        self.state.simulation.submitted_edit_counter = Some(self.state.gui.edit_counter);
        self.state.simulation.submitted_metric_options_revision =
            Some(self.state.simulation.metric_options_revision);
        // D7 (W0c): the core's own half of the same rule. Every edit that
        // clears the simulation bumps the epoch, so a run that lands after
        // one carries a stamp the session refuses.
        self.state.simulation.submitted_simulation_epoch =
            Some(self.state.session.simulation_epoch());

        let machine = self.state.session.machine();
        let max_feed_mm_min = machine.max_feed_mm_min.max(1.0);
        // F-034/F-035 — build core's own context here, so the worker
        // forwards one value instead of re-assembling three loose fields.
        let kinematics = machine.kinematics.map(|kinematics| {
            rs_cam_core::compute::simulate::KinematicsContext {
                kinematics,
                max_feed_mm_min,
                // The GUI does not yet expose a toggle for this; `false`
                // preserves pre-F-035 gate verdicts. When the GUI grows an
                // opt-in (F-036 territory), it reads off the simulation
                // panel state and threads here.
                use_predicted_feed_in_gates: false,
            }
        });
        self.compute.submit_simulation(SimulationRequest {
            core: rs_cam_core::compute::simulate::SimulationRequest {
                groups,
                stock_bbox,
                stock_top_z: stock_bbox.max.z,
                resolution: request_resolution,
                metric_options,
                spindle_rpm: self.state.gui.post.spindle_speed,
                rapid_feed_mm_min: if self.state.gui.post.high_feedrate_mode {
                    self.state.gui.post.high_feedrate.max(1.0)
                } else {
                    max_feed_mm_min
                },
                model_mesh,
                kinematics,
            },
            memoize_prefix,
        });
    }

    /// Simulate every enabled toolpath at the panel's own resolution.
    ///
    /// Returns `false` when there was nothing to simulate and no request was
    /// submitted. A plan step must know that, or it would wait forever for a
    /// completion that will never drain.
    pub(crate) fn run_simulation_with_all(&mut self) -> bool {
        self.run_simulation_all(false, false)
    }

    /// [`Self::run_simulation_with_all`] with the S5 prefix memo and the
    /// plan's cell size.
    ///
    /// `memoize_prefix` is `false` for the plan's CLOSING simulation: it is
    /// the last one and leaves nothing behind. Only
    /// [`Self::run_simulation_prefix`] memoises.
    ///
    /// `for_plan` is `true` for a plan's closing simulation: it carves with
    /// the metrics kernel, so the rest snapshots it leaves stay current.
    pub(crate) fn run_simulation_all(&mut self, memoize_prefix: bool, for_plan: bool) -> bool {
        let Some((groups, all_tools_flat, stock_bbox)) =
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
            &all_tools_flat,
            stock_bbox,
            Some(0),
            memoize_prefix,
            for_plan,
        );
        true
    }

    /// Simulate setups `0..=setup_idx`, every enabled operation, for a plan.
    ///
    /// The filter is "every ENABLED operation in setups `0..=setup_idx`",
    /// never a narrowed id list. `build_simulation_groups` feeds the phantom
    /// scan the count of the operations its include filter ADMITTED, so
    /// dropping a generated operation that sits before the pending one
    /// shifts that count down and takes the snapshot BEFORE that operation's
    /// cuts. For a rest operation that is an over-cut, not a stale preview.
    ///
    /// Answers `false` when the builder produced no group, so the plan
    /// records `Skipped` instead of waiting for a result that never drains.
    pub(crate) fn run_simulation_prefix(&mut self, setup_idx: usize, memoize_prefix: bool) -> bool {
        let Some((groups, all_tools_flat, stock_bbox)) =
            self.build_simulation_groups(|i, tc| i <= setup_idx && tc.enabled, |i| i == setup_idx)
        else {
            return false;
        };
        self.submit_simulation_for_groups(
            groups,
            &all_tools_flat,
            stock_bbox,
            Some(setup_idx),
            memoize_prefix,
            true,
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

        let Some((groups, all_tools_flat, stock_bbox)) = self.build_simulation_groups(
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
            &all_tools_flat,
            stock_bbox,
            Some(target_setup_idx),
            false,
            false,
        );
    }

    /// Submit the holder/shank clearance check.
    ///
    /// **It examines ONE toolpath** — the first that has a result, a cutter
    /// and a mesh — and the row it feeds is titled "Holder clearance" for the
    /// whole job. F2.13 (G-HOLDERSCOPE) does not widen the check: pricing a
    /// per-toolpath sweep needs a measurement this lane could not take
    /// (`run_collision_check` builds a spatial index and runs a drop-cutter
    /// query per 1 mm sample per assembly segment). It makes the verdict carry
    /// its own population instead, so the row states what it measured rather
    /// than reading Clear for operations it never asked about.
    ///
    /// The selection is unchanged, including the two things it does not do:
    /// it does not skip a DISABLED toolpath, and it does not look for the
    /// toolpath most likely to strike. The scope's `examined` counts the
    /// ENABLED toolpath it reached, so a verdict about a switched-off
    /// operation examines none of the job and cannot read as the job's.
    pub(crate) fn request_collision_check(&mut self) {
        // F2.13: the denominator is the ENABLED operation list, not the list
        // of toolpaths a check could examine. An operation with no mesh is one
        // this checker cannot reach, and counting it out of the denominator
        // would let the row read Clear for a job most of which was never
        // asked about.
        let population = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .filter(|tc| tc.enabled)
            .count();
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
                Some((
                    index,
                    tc.enabled,
                    Arc::clone(&result.annotated),
                    tool,
                    mesh,
                    obstacles,
                ))
            });

        if let Some((index, enabled, annotated, tool, mesh, obstacles)) = toolpath_data {
            // G-HOLDERSTALE (F2.12): the core's simulation epoch as it
            // stands at SUBMIT, and only on the branch that really submits.
            // The `else` arm below reaches the lane with nothing, and a stamp
            // written before the search would sit there for an unrelated
            // later arrival.
            //
            // W4: the epoch, not the GUI edit counter. Every input this check
            // reads reaches `ProjectSession::drop_simulation`, which is the
            // one site that moves it.
            self.state.simulation.submitted_collision_epoch =
                Some(self.state.session.simulation_epoch());
            // G-HOLDERSCOPE (F2.13): and the population it covers, on the same
            // branch and for the same reason.
            //
            // `examined` counts the ENABLED toolpath the check reached, and
            // the selection above can reach a DISABLED one — the Add/Remove
            // toggle flips the config and leaves the GUI result in place. A
            // verdict about motion the job does not contain examines NONE of
            // the job, so it stamps zero and the row refuses to call the job
            // clear. The arithmetic closes what the selection leaves open;
            // the selection itself is unchanged.
            self.state.simulation.submitted_collision_scope = Some(HolderCheckScope {
                examined: usize::from(enabled),
                population,
                position: Some(index + 1),
            });
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
/// `(0,0,0)..(100, 100, 12)`. The simulator falls back to
/// `request.stock_bbox` when `local_stock_bbox` is `None` (the F-024
/// follow-up landed in commit `1dd1aa7`; CMP-19 moved the viz's copy of
/// the rule onto `SetupEvalContext::sim_local_stock_bbox`, the accessor
/// core's own builder calls), so the broken bbox was the dexel grid's
/// Z range — toolpath
/// cuts at world Z=-2 sat below every ray and `axial_engagement_mm` read
/// the full stock height instead of the commanded DOC.
pub(crate) fn build_world_stock_bbox(
    session: &rs_cam_core::session::ProjectSession,
) -> BoundingBox3 {
    session.stock_bbox()
}
