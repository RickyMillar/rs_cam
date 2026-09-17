//! Simulation and feed modulation.
//!
//! Builds the simulation request, runs the tri-dexel simulation, and feeds the
//! resulting cut trace back into the toolpath as a feed modulation. Split out
//! of `session/compute.rs` (P4).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tracing::instrument;

use crate::compute::cutter::build_cutter;
use crate::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, run_simulation,
};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::transform::FaceUp;
use crate::dexel_stock::StockCutDirection;
use crate::geo::{BoundingBox3, P3};
use crate::ids::ToolpathId;
use crate::mesh::TriangleMesh;
use crate::session::{ProjectSession, SessionError, SimulationOptions};
use crate::stock::simulation_cut::SimulationMetricOptions;

use super::{
    FeedContext, SimRequestContext, auto_resolution_for_groups, build_sim_request,
    modulate_annotated_against_trace,
};

/// The cut direction a per-setup simulation GROUP stock is stamped with.
///
/// Two faces, not six. A group stock is built in the SETUP-LOCAL frame,
/// where local −Z is always the tool axis, so the tool always arrives
/// from local `FromTop`. `FaceUp::Bottom` is the one face whose local Z
/// runs against the global Z the dexel columns are indexed on, so it is
/// the one face that stamps `FromBottom`.
///
/// # Do not replace this with `SetupTransformInfo::cut_direction()`
///
/// That accessor answers a DIFFERENT question — which side the tool
/// arrives from in the stock-relative GLOBAL frame — and returns
/// `FromFront` / `FromBack` / `FromLeft` / `FromRight` for the four
/// lateral faces. It feeds the global playback stock. This rule feeds the
/// per-setup group stock, which every metric, gate, collision check and
/// checkpoint mesh is computed on. The two agree on `Top` and `Bottom`
/// and diverge on the four laterals, and that divergence is the design,
/// not a defect. `compute/transform.rs:493` records two earlier defects
/// in exactly this table (G-LATERALSIGN, G-FRONTNAME), both of which came
/// from reading one question as the other.
///
/// STK-06 gave the rule this one home. It used to be an untitled `_` arm
/// written out in two files, which read as an oversight rather than as a
/// decision.
///
/// The S5 simulation prefix hash reads what this returns
/// (`compute/sim_prefix.rs`), so the answer must not move for a lateral
/// setup without a deliberate cache break.
pub(crate) fn group_stock_cut_direction(face_up: FaceUp) -> StockCutDirection {
    match face_up {
        FaceUp::Top => StockCutDirection::FromTop,
        FaceUp::Bottom => StockCutDirection::FromBottom,
        // The tool axis is setup-local Z on a lateral face too.
        FaceUp::Front | FaceUp::Back | FaceUp::Left | FaceUp::Right => StockCutDirection::FromTop,
    }
}

/// Translate a triangle mesh by (dx, dy, dz) and rebuild bbox/faces.
fn translate_mesh(mesh: &TriangleMesh, dx: f64, dy: f64, dz: f64) -> TriangleMesh {
    let verts: Vec<P3> = mesh
        .vertices
        .iter()
        .map(|v| P3::new(v.x + dx, v.y + dy, v.z + dz))
        .collect();
    TriangleMesh::from_raw(verts, mesh.triangles.clone())
}

impl ProjectSession {
    /// Shared `SimulationRequest` assembly for [`run_simulation`](Self::run_simulation)
    /// and `simulate_candidate_isolated` (S.12 dedup).
    ///
    /// The session half is the two records the assembly reads. Everything
    /// else is the free `build_sim_request`, so the strategy
    /// advisor's candidate simulation builds its request holding no session.
    fn build_sim_request(
        &self,
        groups: Vec<SimGroupEntry>,
        stock_bbox: BoundingBox3,
        resolution: f64,
        metric_options: SimulationMetricOptions,
        model_mesh: Option<Arc<TriangleMesh>>,
        use_predicted_feed_in_gates: bool,
    ) -> SimulationRequest {
        build_sim_request(
            &SimRequestContext {
                machine: &self.machine,
                post: &self.post,
            },
            groups,
            stock_bbox,
            resolution,
            metric_options,
            model_mesh,
            use_predicted_feed_in_gates,
        )
    }

    /// Run tri-dexel stock simulation over all computed toolpaths.
    #[instrument(skip(self, opts))]
    pub fn run_simulation(
        &mut self,
        opts: &SimulationOptions,
        cancel: &AtomicBool,
    ) -> Result<&super::SimulationResult, SessionError> {
        let stock_bbox = self.stock_bbox();

        // Build simulation groups from setups
        let mut groups = Vec::new();
        for setup in &self.setups {
            let direction = group_stock_cut_direction(setup.face_up);

            let mut entries = Vec::new();
            // F.4: track whether this group's first pending (enabled,
            // ungenerated) `FromRemainingStock` toolpath needs a phantom
            // `prior_stocks` snapshot. Visited for every toolpath config in
            // plan order — not just the ones that make it into `entries` —
            // so the scan sees the true "generated yet?" state regardless
            // of `skip_ids` / short-toolpath filtering below.
            let mut phantom_scan = crate::compute::simulate::PhantomPriorStockScan::default();
            for &tp_idx in &setup.toolpath_indices {
                let Some(tc) = self.toolpath_configs.get(tp_idx) else {
                    continue;
                };
                let result = self.results.get(&tp_idx);
                // G-STICKYEMPTY: the scan's question is "will this op
                // contribute a carve to the simulation being built", NOT
                // "does a result object exist". This builder drops entries
                // below `MIN_SIMULATED_MOVES` a few lines down, and a
                // dropped entry never reaches
                // `prior_stocks.insert(entry.id, ..)` — so answering
                // `result.is_some()` here made an op that generated EMPTY
                // ineligible for a prior-stock snapshot by both routes at
                // once, and a `FromRemainingStock` op in that state can
                // never regenerate again (its generate-time precondition
                // refuses for want of the snapshot) until a project reload
                // clears its result cache.
                let contributes_carve = result.is_some_and(|r| {
                    crate::compute::simulate::contributes_simulated_motion(
                        r.annotated().toolpath.moves.len(),
                    )
                });
                phantom_scan.visit(
                    entries.len(),
                    tc.enabled,
                    contributes_carve,
                    tc.id,
                    tc.stock_source,
                );
                if let Some(result) = result {
                    if opts.skip_ids.contains(&tc.id) {
                        continue;
                    }
                    if !crate::compute::simulate::contributes_simulated_motion(
                        result.annotated().toolpath.moves.len(),
                    ) {
                        continue;
                    }

                    let tool_config = self.find_tool_by_raw_id(tc.tool_id);
                    let flute_count = tool_config.map(|t| t.flute_count).unwrap_or(2);
                    let tool_summary = tool_config
                        .map(|t| t.summary())
                        .unwrap_or_else(|| "Unknown".to_owned());
                    let tool_def = tool_config.map(build_cutter).unwrap_or_else(|| {
                        build_cutter(&ToolConfig::new_default(ToolId(0), ToolType::EndMill))
                    });

                    // §6.C / §6.I revision: prefer the move-intent signal
                    // (a toolpath containing any `MoveIntent::Drilling` move)
                    // over the op-kind heuristic. The op-kind matches stay
                    // as the fallback so an in-flight Drill/AlignmentPinDrill
                    // whose generator hasn't been migrated still gets flagged.
                    let op_type = tc.operation.op_type();
                    let has_drilling_intent = result
                        .annotated()
                        .toolpath
                        .moves
                        .iter()
                        .any(|m| matches!(m.intent, crate::toolpath::MoveIntent::Drilling));
                    // §6.E DrillOp variant is the new primary signal;
                    // intent + op-kind remain as fallbacks for the
                    // dual-representation invariant.
                    let metrics_not_applicable = result.is_drill_op()
                        || has_drilling_intent
                        || matches!(
                            op_type,
                            crate::compute::catalog::OperationType::Drill
                                | crate::compute::catalog::OperationType::AlignmentPinDrill
                        );
                    entries.push(SimToolpathEntry {
                        id: tc.id,
                        name: tc.name.clone(),
                        annotated: Arc::clone(result.annotated()),
                        tool: tool_def,
                        flute_count,
                        tool_summary,
                        semantic_trace: result.semantic_trace.as_ref().map(|t| Arc::new(t.clone())),
                        spindle_rpm: tc.operation.spindle_rpm(),
                        metrics_not_applicable,
                        drill_op: result.drill_op().cloned(),
                        operation_config_hash: crate::compute::simulate::hash_operation_config(
                            &tc.operation,
                        ),
                    });
                }
            }

            let phantom_prior_stock = phantom_scan.finish();
            // F.4: a setup whose every toolpath is still ungenerated builds
            // an empty `entries` vec — but if the FIRST enabled config in
            // plan order is a pending `FromRemainingStock` op, the "stock
            // before it" is simply the setup's untouched initial stock
            // (there are zero predecessors to distrust), so the phantom is
            // still valid and the group must still be emitted (with an
            // empty `toolpaths` vec) to carry it.
            if !entries.is_empty() || phantom_prior_stock.is_some() {
                // Per-setup local stock bbox and transform info derived from
                // the shared SetupTransformInfo helper (Phase E/D dedup).
                //
                // F-024 (2026-05-25): the per-setup dexel grid MUST span the
                // same Z range as the toolpath the simulator will stamp into
                // it. The toolpath generator's auto-default `top_z` is `0.0`
                // (see `compute/config.rs` `HeightsConfig::resolve`), so the
                // toolpath emits cut moves at Z=[0, -depth] regardless of
                // setup orientation. For identity setups (`face_up=Top`,
                // `z_rotation=Deg0`) no transform is applied to the toolpath
                // before stamping, so the dexel grid must also be in world
                // frame (`stock.origin_z..stock.origin_z + stock.z`). Using a
                // zero-rooted local bbox `(0..stock_z)` here placed the
                // cutter at world Z=-2 below every dexel ray, which clears
                // the full ray length and inflates `axial_engagement_mm` to
                // the full stock height. Falling through to `None` makes
                // `run_simulation` fall back to `request.stock_bbox` (world
                // frame) for the per-setup grid, matching the toolpath frame.
                //
                // Non-identity setups continue to use the zero-origin
                // effective bbox — that path's frame consistency (toolpath
                // emission, `local_to_global` transform shape) is outside
                // F-024's scope.
                //
                // F-030: drive these decisions from the shared
                // `SetupEvalContext`. `sim_local_stock_bbox()` returns
                // `None` for identity setups (F-024 invariant) and
                // `Some(local_stock_bbox)` paired with `local_to_global`
                // for non-identity setups.
                let setup_ctx = super::SetupEvalContext::build_for_setup(self, Some(setup));
                let local_stock_bbox = setup_ctx.sim_local_stock_bbox();
                let local_to_global = setup_ctx.local_to_global.clone();

                groups.push(SimGroupEntry {
                    toolpaths: entries,
                    direction,
                    local_stock_bbox,
                    local_to_global,
                    phantom_prior_stock,
                });
            }
        }

        // Compute effective resolution: auto-resolution matches the GUI's
        // heuristic (5 cells across the smallest tool radius, clamped to
        // [0.02, 0.5] mm, further capped so the grid stays under ~8M cells).
        let resolution = if opts.auto_resolution {
            auto_resolution_for_groups(&groups, &stock_bbox)
        } else {
            opts.resolution
        };

        // Deviation comparison happens in the simulation's stock-relative
        // global frame (0..stock_size); `SimulationRequest::model_mesh`'s
        // contract is that frame, so translate the world-space model by
        // -stock_origin here. NON-identity groups' `local_to_global`
        // outputs land in that frame directly (face/rotation transforms
        // cancel and origin is never re-added). IDENTITY groups' grids
        // are WORLD-framed (F-024) — the deviation passes frame-map their
        // query points by -stock_bbox.min themselves (see
        // `collect_column_deviations`; the first scaled-wanaka cascade
        // A/B mis-read a uniform ~−4 mm "overcut" when this half of the
        // contract was missing, 2026-07-13).
        let model_mesh = self.models.iter().find_map(|m| m.mesh.clone()).map(|m| {
            Arc::new(translate_mesh(
                &m,
                -self.stock.origin_x,
                -self.stock.origin_y,
                -self.stock.origin_z,
            ))
        });
        // F-034: opt-in kinematics-aware cycle time / F-035 predicted-feed
        // gates are threaded through as `opts.use_predicted_feed_in_gates`;
        // see `build_sim_request`'s doc comment for how this path's knobs
        // differ from `simulate_candidate_isolated`'s.
        let request = self.build_sim_request(
            groups,
            stock_bbox,
            resolution,
            SimulationMetricOptions {
                enabled: opts.metrics_enabled,
                capture_arc_engagement: opts.metrics_enabled,
            },
            model_mesh,
            opts.use_predicted_feed_in_gates,
        );

        let mut result = run_simulation(&request, cancel)?;

        // F-036b — adaptive feed modulation post-pass.
        //
        // After the simulator produces the per-sample engagement
        // record, walk it once per cutting toolpath, aggregate samples
        // into a `Vec<PerMoveEngagement>` keyed by `move_index`, look
        // up the vendor LUT's chipload band, and call
        // [`crate::dressup::feed_modulation::adaptive_feed_modulate`] on a
        // mutable clone of the toolpath. The modulated toolpath replaces
        // the cached `Arc<AnnotatedToolpath>` in `self.results` so the
        // downstream G-code emitter
        // (`crate::gcode::emit_gcode` via `export_gcode_checked`) picks
        // up the per-move modulated feeds and emits per-move F-words —
        // F-036a's emitter contract.
        //
        // Inert when:
        //  - `opts.adaptive_feed_modulation == false` — which is NOT the
        //    default. `impl Default for SimulationOptions`
        //    (`session/mod.rs:878`) sets it `true` (Checkpoint J-3,
        //    2026-08-13, operator-binding; it was `false` before that).
        //    So on the default sim path this post-pass RUNS: it re-solves
        //    per-move feeds from the measured engagement and swaps the
        //    modulated toolpath into `self.results`, which is what the
        //    G-code emitter reads. Consequence a reader must not miss —
        //    a change to the engagement instrument changes emitted
        //    F-words, not just reported numbers. Callers that need the
        //    unmodulated IR (measurement harnesses, byte-identity A/Bs)
        //    must pass `adaptive_feed_modulation: false` explicitly.
        //    Measured and written up in
        //    `planning/perf_review_2026-08-19/DELTA_w5b_f3_corpus.md`
        //    §5.c / §6.
        //  - The vendor LUT has no `chip_load_min_mm` /
        //    `chip_load_max_mm` row for the active
        //    `(tool family, material, op family, pass role, diameter)`
        //    tuple — modulator gets no `ChiploadBand`, the per-toolpath
        //    call is skipped, the IR is untouched.
        //
        // Modulation no longer requires an explicit machine `kinematics`
        // block — it falls back to `effective_kinematics` (the generic
        // wood-router profile), matching the strategy advisor. The GUI/MCP
        // sim path applies the same pass via [`modulate_simulation_trace`].
        // N7 (2026-09-11): the pass's cycle-time re-integration DOES require
        // that block, so with `kinematics: None` the feeds move and the
        // published runtimes do not.
        self.modulate_simulation_trace(&mut result.cut_trace, opts);

        self.simulation = Some(result);
        // SAFETY: we just assigned Some
        #[allow(clippy::unwrap_used)]
        Ok(self.simulation.as_ref().unwrap())
    }

    /// Modulate ONE toolpath's per-move feeds against a simulation cut
    /// trace, reading the material, the machine and the default spindle
    /// speed off the session.
    ///
    /// The session door onto the free `modulate_annotated_against_trace`,
    /// which holds the body. The strategy advisor takes that
    /// function directly with the records its handle captured.
    #[allow(clippy::too_many_arguments)]
    fn modulate_annotated_against_trace(
        &self,
        annotated: &crate::trace::toolpath_spans::AnnotatedToolpath,
        operation: &crate::compute::OperationConfig,
        tool_cfg: &ToolConfig,
        toolpath_id: ToolpathId,
        cut_trace: &crate::stock::simulation_cut::SimulationCutTrace,
        band: crate::dressup::feed_modulation::ChiploadBand,
        kinematics: crate::machine::kinematics::MachineKinematics,
        max_feed: f64,
        rapid_feed: f64,
        strategy: crate::dressup::feed_modulation::ModulationStrategy,
        aggressiveness: f64,
    ) -> Option<(
        crate::toolpath::Toolpath,
        crate::dressup::feed_modulation::ModulationOutcome,
    )> {
        modulate_annotated_against_trace(
            &FeedContext {
                material: &self.stock.material,
                machine: &self.machine,
                default_spindle_rpm: self.post.spindle_speed,
            },
            annotated,
            operation,
            tool_cfg,
            toolpath_id,
            cut_trace,
            band,
            kinematics,
            max_feed,
            rapid_feed,
            strategy,
            aggressiveness,
        )
    }

    /// F-039 — apply the adaptive feed-modulation post-pass to an
    /// already-computed simulation cut trace. The public entry point for the
    /// GUI/MCP sim path (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §10.6):
    /// the in-process worker produces the trace, this applies modulation on
    /// the main thread where the session lives. Mutates the modulated
    /// toolpaths into `self.results` (so G-code export carries the optimized
    /// per-move feeds) and stamps `modulation_summaries` + re-timed runtimes
    /// onto the trace in place. Gated on `opts.adaptive_feed_modulation`;
    /// otherwise a no-op (the byte-identical baseline).
    pub fn modulate_simulation_trace(
        &mut self,
        cut_trace: &mut Option<Arc<crate::stock::simulation_cut::SimulationCutTrace>>,
        opts: &super::SimulationOptions,
    ) {
        if !opts.adaptive_feed_modulation {
            return;
        }
        self.apply_adaptive_feed_modulation(cut_trace, opts);
    }

    /// N2 — integrate ONE toolpath's stored motion on the caller's clock.
    ///
    /// The modulation re-time uses this to rebuild every runtime the
    /// integrator published, drills included. It reads `self.results`, so it
    /// sees the modulated IR the re-time already swapped in.
    ///
    /// Returns `None` when the toolpath has no config or no cached result.
    /// `None` means NOT INTEGRATED here; the caller decides what to keep.
    fn reintegrate_toolpath(
        &self,
        toolpath_id: ToolpathId,
        kinematics: &crate::machine::kinematics::MachineKinematics,
        max_feed: f64,
        rapid_feed: f64,
    ) -> Option<crate::machine::kinematics::CycleTimeBreakdown> {
        let (idx, _) = self
            .toolpath_configs
            .iter()
            .enumerate()
            .find(|(_, tc)| tc.id == toolpath_id)?;
        let result_slot = self.results.get(&idx)?;
        Some(crate::machine::kinematics::compute_cycle_time_breakdown(
            &result_slot.annotated().toolpath,
            kinematics,
            max_feed,
            rapid_feed,
        ))
    }

    /// F-036b — apply the per-move adaptive feed modulator to every
    /// computed toolpath after `run_simulation` produces its trace.
    ///
    /// Walks each toolpath's `SimulationCutSample`s, aggregates them
    /// time-weighted into a `Vec<PerMoveEngagement>` keyed by
    /// `move_index`, looks up the vendor LUT chipload band, builds the
    /// `ModulationContext`, and calls
    /// [`crate::dressup::feed_modulation::adaptive_feed_modulate`] on a `clone`
    /// of the cached `Arc<AnnotatedToolpath>::toolpath`. When the
    /// modulator reports any feed change, the `Arc<AnnotatedToolpath>`
    /// in `self.results` is swapped for a new one wrapping the modulated
    /// toolpath — `Arc::make_mut` is not used because the trace
    /// `samples` still hold the legacy `Arc` and we want them to remain
    /// pinned to the pre-modulation IR for diagnostic continuity.
    ///
    /// No-op (silently) when:
    ///  - The simulation produced no `cut_trace` (`metrics_enabled =
    ///    false`).
    ///  - A toolpath has no cut samples (drill-only / all-rapid / etc.).
    ///  - The vendor LUT has no chipload band for the toolpath.
    ///  - The modulator returns
    ///    `ModulationError::EngagementLengthMismatch` (defensive — only
    ///    fires when the toolpath has been re-generated between sim and
    ///    modulation; impossible inside `run_simulation`'s single
    ///    transaction).
    ///
    /// The chipload band source is
    /// [`crate::tool_load::chipload_envelopes_for_session`] — the same
    /// helper the chipload viewport coloring + timeline envelope readout
    /// already use, so band semantics match the rest of the load-gates
    /// surface.
    fn apply_adaptive_feed_modulation(
        &mut self,
        cut_trace: &mut Option<Arc<crate::stock::simulation_cut::SimulationCutTrace>>,
        opts: &super::SimulationOptions,
    ) {
        // Engagement aggregation + `ModulationContext` build now live in the
        // shared `modulate_annotated_against_trace`; this pass only needs the
        // chipload band to gate which toolpaths are eligible.
        use crate::dressup::feed_modulation::ChiploadBand;

        let Some(cut_trace_ref) = cut_trace.as_deref() else {
            return;
        };
        // Modulation runs against the machine's effective kinematics — the
        // generic-wood-router fallback when no explicit block is set — so it
        // applies on every machine, matching the strategy advisor (step 5).
        let kinematics = self.machine.effective_kinematics();
        let envelopes = crate::tool_load::chipload_envelopes_for_session(self, Some(cut_trace_ref));
        if envelopes.is_empty() {
            return;
        }

        // F-039 — accumulator for the per-(toolpath_id, move_index)
        // `(feed, binding)` map and the per-toolpath
        // `ModulationSummary`. Stamped onto the cut trace below.
        let mut modulated_feeds: std::collections::BTreeMap<
            (ToolpathId, usize),
            (f64, crate::tool_load::BindingConstraint),
        > = std::collections::BTreeMap::new();
        let mut modulation_summaries: std::collections::BTreeMap<
            ToolpathId,
            crate::tool_load::ModulationSummary,
        > = std::collections::BTreeMap::new();

        // F4: modulation raises/lowers CUTTING feed — its ceiling is
        // the cutting ceiling. Rapids keep the travel rate.
        let max_feed = self.machine.cutting_feed_ceiling_mm_min().max(1.0);
        let rapid_feed = if self.post.high_feedrate_mode {
            self.post.high_feedrate.max(1.0)
        } else {
            self.machine.max_feed_mm_min.max(1.0)
        };

        // Toolpath indices to walk: enabled, with a result, with at least
        // one cut sample in the trace.
        let candidate_indices: Vec<(usize, ToolpathId)> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .filter(|(idx, tc)| tc.enabled && self.results.contains_key(idx))
            .map(|(idx, tc)| (idx, tc.id))
            .collect();

        for (idx, toolpath_id) in candidate_indices {
            let Some(band_range) = envelopes.get(&toolpath_id) else {
                continue;
            };
            let Some(band) = ChiploadBand::new(band_range.start, band_range.end) else {
                continue;
            };
            let Some(tc) = self.toolpath_configs.get(idx) else {
                continue;
            };
            let Some(tool_cfg) = self.find_tool_by_raw_id(tc.tool_id) else {
                continue;
            };
            let Some(result) = self.results.get(&idx) else {
                continue;
            };
            let annotated_arc = result.annotated();
            let commanded_feed_for_summary = tc.operation.feed_rate();
            // F-039 core (shared with the strategy advisor): aggregate
            // engagement + build the deflection / power / chipload context
            // and modulate this toolpath's per-move feeds in one place.
            let Some((modulated_toolpath, outcome)) = self.modulate_annotated_against_trace(
                annotated_arc.as_ref(),
                &tc.operation,
                tool_cfg,
                toolpath_id,
                cut_trace_ref,
                band,
                kinematics,
                max_feed,
                rapid_feed,
                opts.modulation_strategy,
                opts.modulation_aggressiveness,
            ) else {
                continue;
            };
            // Stamp per-move map onto the trace-wide accumulator
            // (always — even if no feed actually changed, the
            // diagnostic surface uses this for the "all moves
            // visited" coverage signal).
            for (move_idx, value) in &outcome.per_move {
                modulated_feeds.insert((toolpath_id, *move_idx), *value);
            }
            if let Some(summary) = outcome.build_summary(
                commanded_feed_for_summary,
                opts.modulation_aggressiveness,
                opts.modulation_strategy,
            ) {
                modulation_summaries.insert(toolpath_id, summary);
            }
            if outcome.changed == 0 {
                continue;
            }
            let new_annotated = crate::trace::toolpath_spans::AnnotatedToolpath {
                toolpath: modulated_toolpath,
                spans: annotated_arc.spans.clone(),
                spans_valid: annotated_arc.spans_valid,
                // Modulation rewrites feeds, not geometry — the planner
                // engagement samples stay valid by position.
                planner_engagement: annotated_arc.planner_engagement.clone(),
                // Rest-field overlay grid is toolpath-wide metadata, unaffected
                // by feed modulation — carry it through unchanged.
                rest_grid: annotated_arc.rest_grid.clone(),
                // Same for the derived machining-region polygons.
                rest_regions: annotated_arc.rest_regions.clone(),
            };
            let new_arc = Arc::new(new_annotated);
            // Rebuild the op_data variant with the swapped Arc.
            let new_op_data = match &result.op_data {
                crate::ops::drill_op::OpData::Toolpath(_) => {
                    crate::ops::drill_op::OpData::Toolpath(new_arc)
                }
                crate::ops::drill_op::OpData::DrillOp(drill, _) => {
                    crate::ops::drill_op::OpData::DrillOp(Arc::clone(drill), new_arc)
                }
            };
            if let Some(slot) = self.results.get_mut(&idx) {
                slot.op_data = new_op_data;
            }
        }

        // F-036b1 sequencing fix: F-034's `apply_kinematics_cycle_time`
        // runs INSIDE `run_simulation` (before modulation), so the
        // trace's `total_runtime_s` reflects the pre-modulation
        // commanded feeds. After modulation rewrites per-move feeds,
        // re-walk every toolpath through `compute_cycle_time` and update
        // the trace's per-toolpath + project-total runtime so callers
        // (F-036c regression test, GUI panel, diagnostics summary) see
        // the modulated cycle time. N2 (2026-09-10) extends "every
        // toolpath" to mean it: read the ONE CLOCK note below.
        let Some(trace_arc) = cut_trace.as_mut() else {
            return;
        };
        let trace = Arc::make_mut(trace_arc);
        // N7 (2026-09-11, operator ruling) — the re-time integrates only when
        // the machine carries a kinematics block. `MachineProfile::kinematics`
        // is the F-034 feature flag: its `None` keeps the live-sim
        // `total_runtime_s` byte-identical, and every preset ships `None`.
        // `effective_kinematics()` below falls back to the generic wood
        // router, so without this guard an unconfigured machine got a full
        // re-integration whenever modulation ran — and `readiness.rs` then
        // labelled that guess `CycleTimeBasis::MachineModel`.
        //
        // The modulation of feeds above is NOT guarded. It still applies on
        // every machine. Only the cycle-time re-integration is skipped.
        //
        // The G-AIRDENOM rebase inside the loop is skipped with the block, on
        // purpose. `rebase_cutting_times` takes the integrator's own
        // `CycleTimeBreakdown` as the clock it moves the cutting seconds onto.
        // With no integration there is no such clock, so the naive dexel
        // seconds at the commanded feed stay the one time base. A rebase
        // against a fallback integration, with `total_runtime_s` left naive,
        // rebuilds the exact mixed-base defect G-AIRDENOM closed.
        //
        // Sentry: `tests/retime_respects_no_kinematics_n7.rs`.
        if self.machine.kinematics.is_some() {
            // N2 (2026-09-10) — ONE CLOCK for every runtime this pass publishes.
            // The re-time integrates on `effective_kinematics()` and the cutting
            // feed ceiling — the clock the EMITTED feeds run at. The integrator
            // inside `run_simulation` used the machine's own `kinematics` block
            // and `max_feed_mm_min`, so the two clocks can differ. After this pass
            // no published runtime carries the integrator's clock:
            // `toolpath_runtimes`, the engagement summaries and the project total
            // are re-integrated together and published together.
            //
            // The map covers every toolpath the integrator reached, DRILLS
            // INCLUDED. A drill has no engagement summary, so a fold over
            // `toolpath_summaries` alone drops its seconds. That is G-DRILLTIME,
            // and N2 is the same defect re-opened on this path.
            let mut per_toolpath_runtime: std::collections::BTreeMap<
                ToolpathId,
                crate::machine::kinematics::CycleTimeBreakdown,
            > = std::collections::BTreeMap::new();
            for entry in &trace.toolpath_runtimes {
                // A toolpath with no config or no cached result cannot be
                // re-integrated. Keep the integrator's own value rather than drop
                // the entry: dropping it is the exact shape of the defect above.
                let b = self
                    .reintegrate_toolpath(entry.toolpath_id, &kinematics, max_feed, rapid_feed)
                    .unwrap_or(entry.breakdown);
                per_toolpath_runtime.insert(entry.toolpath_id, b);
            }
            // G-AIRDENOM (2026-09-08) — the loop below rewrites
            // `total_runtime_s` onto the integrator's clock. The cutting
            // slices must move with it or the two air-cut percentages divide
            // one numerator by two different time models. Deltas are folded
            // into the project summary after the loop, the same way the project
            // total is, so a toolpath the loop skips keeps whatever the
            // accumulator gave it.
            let mut air_delta = 0.0;
            let mut cutting_delta = 0.0;
            let mut low_engagement_delta = 0.0;
            let mut rapid_delta = 0.0;
            // Disjoint field borrows: the loop takes `toolpath_summaries`
            // mutably while the rebase reads `samples`.
            let samples = &trace.samples;
            for tp_summary in &mut trace.toolpath_summaries {
                // Recompute the MoveIntent breakdown alongside the total —
                // leaving F-034's pre-modulation breakdown in place would
                // desynchronize `runtime_by_intent.total_s` from the
                // modulated `total_runtime_s` written below.
                let b = match per_toolpath_runtime.get(&tp_summary.toolpath_id) {
                    Some(&b) => b,
                    // N2 — the integrator published no runtime for this
                    // toolpath. Integrate here, exactly as this loop did
                    // before N2, and leave the empty slot empty.
                    //
                    // N7 — this arm no longer covers the no-kinematics case.
                    // That machine exits at the guard above, because a
                    // fallback integration is not a machine model. The arm
                    // stays defensive: it covers a summary the integrator
                    // itself did not reach on a machine that DOES carry a
                    // `kinematics` block.
                    None => {
                        let Some(b) = self.reintegrate_toolpath(
                            tp_summary.toolpath_id,
                            &kinematics,
                            max_feed,
                            rapid_feed,
                        ) else {
                            continue;
                        };
                        b
                    }
                };
                tp_summary.total_runtime_s = b.total_s;
                tp_summary.runtime_by_intent = Some(b);
                // G-AIRDENOM — rebase this toolpath's cutting seconds onto the
                // clock just written above. `None` means the toolpath carried
                // no cutting samples (drill-only, all-rapid); leave it alone
                // rather than writing a zero over a measured value.
                if let Some(rebased) = crate::stock::simulation_cut::rebase_cutting_times(
                    samples,
                    tp_summary.toolpath_id,
                    &modulated_feeds,
                    &b,
                ) {
                    cutting_delta += rebased.cutting_runtime_s - tp_summary.cutting_runtime_s;
                    air_delta += rebased.air_cut_time_s - tp_summary.air_cut_time_s;
                    low_engagement_delta +=
                        rebased.low_engagement_time_s - tp_summary.low_engagement_time_s;
                    rapid_delta += rebased.rapid_runtime_s - tp_summary.rapid_runtime_s;
                    tp_summary.cutting_runtime_s = rebased.cutting_runtime_s;
                    tp_summary.air_cut_time_s = rebased.air_cut_time_s;
                    tp_summary.low_engagement_time_s = rebased.low_engagement_time_s;
                    tp_summary.rapid_runtime_s = rebased.rapid_runtime_s;
                    // `average_mrr_mm3_s` is `removed_volume / cutting_runtime_s`,
                    // baked at build time. Leaving it would break an identity a
                    // reader can check from two published fields, so it moves to
                    // the wall clock with its own denominator.
                    if rebased.cutting_runtime_s > 1e-9 {
                        tp_summary.average_mrr_mm3_s =
                            tp_summary.total_removed_volume_est_mm3 / rebased.cutting_runtime_s;
                    }
                }
            }
            // N2 — publish `toolpath_runtimes`, the matching summaries and the
            // project total through the one tail the integrator also uses, so both
            // paths write the same three slots the same way.
            crate::stock::simulation_cut::publish_cycle_times(trace, &per_toolpath_runtime);
            trace.summary.cutting_runtime_s += cutting_delta;
            trace.summary.air_cut_time_s += air_delta;
            trace.summary.low_engagement_time_s += low_engagement_delta;
            trace.summary.rapid_runtime_s += rapid_delta;
            if trace.summary.cutting_runtime_s > 1e-9 {
                trace.summary.average_mrr_mm3_s =
                    trace.summary.total_removed_volume_est_mm3 / trace.summary.cutting_runtime_s;
            }
        }
        // F-039 — stamp the per-move binding map + per-toolpath
        // modulation summaries onto the trace. Both fields are
        // `#[serde(skip)]` so artifact round-tripping is unaffected;
        // the maps are re-derivable when modulation re-runs.
        trace.modulated_feeds = modulated_feeds;
        trace.modulation_summaries = modulation_summaries;
        // Make the load gates grade the MODULATED feed, not the pre-modulation
        // sample feed. The cut-trace samples carry the feed the sim ran at
        // (modulation is a post-pass), so without this the chipload / power /
        // deflection gates — which read `effective_feed_for_sample`, backed by
        // `predicted_feeds` — would report the *un-modulated* load: an under-fed
        // path reads chipload-low even though modulation raised it into band.
        // Stamping the per-move modulated feed into `predicted_feeds` closes
        // that gap, the gate↔modulation agreement the unified load model targets
        // (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §7).
        for (&key, &(feed, _binding)) in &trace.modulated_feeds {
            trace.predicted_feeds.insert(key, feed);
        }
        // Modulation rewrote per-move feeds, so each modulated toolpath now
        // hashes differently than the pre-modulation value captured in the
        // trace's provenance — which would make `sim_trace_is_fresh` (and the
        // load report) read `StaleSimulation`, dropping the very
        // `modulation_summary` this pass just stamped. Refresh the provenance
        // toolpath hashes against the modulated IR so the trace stays FRESH
        // relative to the toolpaths it now describes. Only toolpaths the
        // trace already covers are touched (others aren't part of this sim).
        if let Some(provenance) = trace.provenance.as_mut() {
            for (idx, tc) in self.toolpath_configs.iter().enumerate() {
                if !provenance.toolpath_hashes.contains_key(&tc.id) {
                    continue;
                }
                if let Some(slot) = self.results.get(&idx) {
                    provenance.toolpath_hashes.insert(
                        tc.id,
                        crate::compute::simulate::hash_toolpath(&slot.annotated().toolpath),
                    );
                }
            }
        }
    }
}
