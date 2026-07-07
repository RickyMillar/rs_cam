use std::sync::Arc;

use rs_cam_core::dexel_stock::TriDexelStock;

use crate::compute::{ComputeBackend, ComputeError, ComputeMessage, ComputeRequest};
use crate::state::simulation::{SimulationResults, SimulationRunMeta};
use crate::state::toolpath::{ComputeStatus, OperationConfig, StockSource, ToolpathId};

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    /// Fail a toolpath generation at *submit* time — i.e. before the
    /// request ever reaches the compute worker. Every precondition
    /// rejection inside `submit_toolpath_compute` (missing tool, failed
    /// validation, no 3D mesh, no prior simulated stock for rest
    /// machining, a `DerivedRestRegions` boundary whose source is
    /// missing/self-referential/ungenerated/regionless, ...) must funnel
    /// through here.
    ///
    /// Without this, an MCP `generate_toolpath` / `generate_all` caller
    /// hangs forever: `notify_mcp_toolpath_complete` was previously only
    /// invoked from `drain_compute_results`, which only ever runs for
    /// requests that actually made it to the compute worker. A submit-time
    /// early return produced no worker result, so nothing ever drained,
    /// and the MCP oneshot channel was never resolved (confirmed live: an
    /// MCP `generate_toolpath` call sat unresolved for ~9 hours while the
    /// GUI correctly showed the toolpath in `Error` state).
    ///
    /// Sets `rt.status = Error(msg)` (matching what `drain_compute_results`
    /// does for a compute `Err`) and resolves any pending MCP waiter for
    /// this toolpath with the same error. If no MCP request is pending
    /// (GUI-initiated generate), `notify_mcp_toolpath_complete` is a no-op,
    /// matching existing behavior.
    fn fail_toolpath_submit(&mut self, tp_id: ToolpathId, msg: impl Into<String>) {
        let msg = msg.into();
        let rt = self.state.gui.toolpath_rt_or_default(tp_id);
        rt.status = ComputeStatus::Error(msg);
        rt.result = None;
        #[cfg(feature = "mcp")]
        self.notify_mcp_toolpath_complete(tp_id);
    }

    pub(crate) fn submit_toolpath_compute(&mut self, tp_id: ToolpathId) {
        let Some((tp_idx, tc)) = self.state.session.find_toolpath_config_by_id(tp_id) else {
            self.fail_toolpath_submit(tp_id, "Toolpath config not found".to_owned());
            return;
        };

        let tool_id_raw = tc.tool_id;
        let model_id_raw = tc.model_id;
        let mut operation = tc.operation.clone();
        let dressups = tc.dressups.clone();
        let heights_config = tc.heights.clone();
        let stock_source = tc.stock_source;
        let toolpath_name = tc.name.clone();
        let boundary = tc.boundary.clone();
        let rest_analysis = tc.rest_analysis.clone();
        let debug_options = tc.debug_options;
        let face_selection_for_toolpath = tc.face_selection.clone();

        let Some(tool) = self
            .state
            .session
            .tools()
            .iter()
            .find(|t| t.id.0 == tool_id_raw)
            .cloned()
        else {
            self.push_notification(
                "Cannot generate: no tool assigned to this toolpath".into(),
                super::super::Severity::Warning,
            );
            self.fail_toolpath_submit(tp_id, "No tool assigned to this toolpath".to_owned());
            return;
        };

        // Run validation
        {
            let validation =
                crate::ui::properties::ToolpathValidationContext::from_session(&self.state.session);
            if let Some((_, tc)) = self.state.session.find_toolpath_config_by_id(tp_id) {
                let errs = crate::ui::properties::validate_toolpath_config(tc, &validation);
                if !errs.is_empty() {
                    self.fail_toolpath_submit(tp_id, errs.join("; "));
                    return;
                }
            }
        }

        // Find the setup that contains this toolpath.
        // F-030: every frame-derived value below (transform, stock bbox,
        // heights, safe_z) reads from one `SetupEvalContext` so this
        // controller path can never diverge from `session::compute::compute`.
        let setup_data = self
            .state
            .session
            .list_setups()
            .iter()
            .find(|s| s.toolpath_indices.contains(&tp_idx));
        let ctx = rs_cam_core::session::SetupEvalContext::build_for_setup(
            &self.state.session,
            setup_data,
        );

        let mut keep_out_footprints = setup_data
            .map(|setup| {
                let mut footprints = Vec::new();
                for fixture in &setup.fixtures {
                    if fixture.enabled {
                        footprints.push(fixture.footprint());
                    }
                }
                for keep_out in &setup.keep_out_zones {
                    if keep_out.enabled {
                        footprints.push(keep_out.footprint());
                    }
                }
                footprints
            })
            .unwrap_or_default();

        // Build a lightweight "setup" for transforms using session types
        let transform_setup = setup_data.map(|setup| {
            crate::state::job::Setup::new(crate::state::job::SetupId(setup.id), setup.name.clone())
        });
        // If there's a real setup, copy face_up and z_rotation
        let transform_setup = match (transform_setup, setup_data) {
            (Some(mut s), Some(sd)) => {
                s.face_up = sd.face_up;
                s.z_rotation = sd.z_rotation;
                Some(s)
            }
            _ => None,
        };

        // F-028 viz-path follow-up + F-030: identity setups
        // (`face_up=Top`, `z_rotation=Deg0`) skip the geometry transform so
        // the generation pipeline emits cuts in world frame, mirroring
        // `session::compute::compute`. The `SetupEvalContext::needs_transform()`
        // predicate is the single source of truth — pre-F-028 the viz path
        // took the transform branch even for identity setups, which
        // anchored heights at `local stock_top` (12 mm for AS001) and
        // emitted cuts at Z=10, 8, 6 while the downstream sim grid sat in
        // world frame (Z=0 stock top), so the cutter swept through air the
        // whole run.
        let transform_setup = transform_setup.filter(|_| ctx.needs_transform());

        if let OperationConfig::ProjectCurve(ref mut cfg) = operation {
            // F-030: `is_z_flipped` is false for identity setups, so this
            // collapses the previous `setup_is_z_flipped && needs_transform`.
            cfg.setup_z_flipped = ctx.is_z_flipped();
        }

        let stock_snapshot = self.state.session.stock_config().clone();

        let model = self
            .state
            .session
            .models()
            .iter()
            .find(|m| m.id == model_id_raw);
        let mut polygons = model.and_then(|m| m.polygons.clone());
        let mut mesh = model.and_then(|m| m.mesh.clone());
        let enriched_mesh = model.and_then(|m| m.enriched_mesh.clone());
        let face_selection = face_selection_for_toolpath;

        // ProjectCurve: use a separate model's mesh for the 3D surface when configured.
        if let OperationConfig::ProjectCurve(ref cfg) = operation
            && let Some(surface_id) = cfg.surface_model_id
        {
            mesh = self
                .state
                .session
                .models()
                .iter()
                .find(|m| m.id == surface_id.0)
                .and_then(|m| m.mesh.clone());
        }

        // Derive polygons from selected BREP faces when no explicit polygons exist.
        let mut face_top_z: Option<f64> = None;
        if polygons.is_none()
            && let (Some(face_ids), Some(enriched)) = (&face_selection, &enriched_mesh)
            && !face_ids.is_empty()
        {
            if let Some(poly) = enriched.faces_boundary_as_polygon(face_ids) {
                polygons = Some(Arc::new(vec![poly]));
                let z = face_ids
                    .iter()
                    .filter_map(|fid| enriched.face_group(*fid))
                    .map(|fg| fg.bbox.max.z)
                    .fold(f64::NEG_INFINITY, f64::max);
                if z.is_finite() {
                    face_top_z = Some(z);
                }
            } else {
                tracing::warn!(
                    "Selected faces did not produce a boundary polygon (non-horizontal or non-planar)"
                );
                self.status_message = Some((
                    "Face selection ignored: selected faces are not horizontal planes".to_owned(),
                    std::time::Instant::now(),
                ));
            }
        }

        if let Some(transform_setup) = transform_setup.as_ref() {
            if let Some(raw_mesh) = mesh.as_ref() {
                mesh = Some(Arc::new(crate::state::job::transform_mesh(
                    raw_mesh,
                    transform_setup,
                    &stock_snapshot,
                )));
            }
            if let Some(raw_polygons) = polygons.as_ref() {
                polygons = Some(Arc::new(crate::state::job::transform_polygons(
                    raw_polygons,
                    transform_setup,
                    &stock_snapshot,
                )));
            }
            keep_out_footprints = crate::state::job::transform_polygons(
                &keep_out_footprints,
                transform_setup,
                &stock_snapshot,
            );
        }

        let is_3d = operation.is_3d();
        if is_3d && mesh.is_none() {
            self.fail_toolpath_submit(tp_id, "No 3D mesh (import STL or STEP)".to_owned());
            return;
        }
        if !is_3d && !operation.is_stock_based() && polygons.is_none() {
            self.fail_toolpath_submit(
                tp_id,
                "No 2D geometry (import SVG/DXF or select STEP faces)".to_owned(),
            );
            return;
        }

        let prev_tool_radius = if let OperationConfig::Rest(config) = &operation {
            config.prev_tool_id.and_then(|prev_tool_id| {
                self.state
                    .session
                    .tools()
                    .iter()
                    .find(|t| t.id == prev_tool_id)
                    .map(|t| t.diameter / 2.0)
            })
        } else {
            None
        };

        // R1 (pencil): resolve the real reference tool config from the Pencil
        // op's `reference_tool_id`, mirroring prev_tool_radius above. `None`
        // (unset or not found) falls back to the nominal reference diameter.
        //
        // P2.5: non-Pencil ops with `rest_analysis` enabled resolve their
        // reference tool the same way, from `RestAnalysisConfig::reference_tool_id`
        // — same slot core's `resolve_generation_inputs` reuses, so both
        // paths agree on which real tool becomes the rest reference.
        let reference_tool_cfg = if let OperationConfig::Pencil(config) = &operation {
            config.reference_tool_id.and_then(|ref_id| {
                self.state
                    .session
                    .tools()
                    .iter()
                    .find(|t| t.id == ref_id)
                    .cloned()
            })
        } else if rest_analysis.enabled {
            rest_analysis.reference_tool_id.and_then(|ref_id| {
                self.state
                    .session
                    .tools()
                    .iter()
                    .find(|t| t.id == ref_id)
                    .cloned()
            })
        } else {
            None
        };

        // Refresh pin drill holes from current stock state before submitting.
        if let OperationConfig::AlignmentPinDrill(ref mut cfg) = operation {
            cfg.holes = self
                .state
                .session
                .stock_config()
                .alignment_pins
                .iter()
                .map(|p| [p.x, p.y])
                .collect();
        }

        // Update GUI runtime status
        let rt = self.state.gui.toolpath_rt_or_default(tp_id);
        rt.status = ComputeStatus::Computing;
        // "Stale" means "needs a submit" — this submit satisfies it. Leaving
        // the flag set let `process_auto_regen`'s 500ms sweep resubmit the
        // same id while it was still the lane's active job, which the
        // worker's resubmit-cancels-and-requeues rule turned into a
        // deterministic "generation cancelled" for any slow op right after
        // load_project (Back Rough, 2×, 2026-07-07).
        rt.stale_since = None;
        rt.result = None;
        rt.debug_trace = None;
        rt.semantic_trace = None;
        rt.debug_trace_path = None;

        // F-030: stock bbox + safe_z come from the shared
        // `SetupEvalContext`. `heights_stock_bbox` is world frame for
        // identity setups (F-028 invariant) and local zero-rooted for
        // non-identity setups; `safe_z` is floored at the local stock
        // top per F-024.
        let stock_bbox = ctx.heights_stock_bbox;
        let safe_z = ctx.safe_z;

        let model_bb = self
            .state
            .session
            .models()
            .iter()
            .find(|m| m.id == model_id_raw)
            .and_then(|m| m.mesh.as_ref().map(|mesh| mesh.bbox));
        let (model_top_z, model_bottom_z) = match (model_bb, transform_setup.as_ref()) {
            (Some(bb), Some(setup)) => {
                let mut min_z = f64::INFINITY;
                let mut max_z = f64::NEG_INFINITY;
                for &x in &[bb.min.x, bb.max.x] {
                    for &y in &[bb.min.y, bb.max.y] {
                        for &z in &[bb.min.z, bb.max.z] {
                            let local = setup.transform_point(
                                rs_cam_core::geo::P3::new(x, y, z),
                                &stock_snapshot,
                            );
                            if local.z < min_z {
                                min_z = local.z;
                            }
                            if local.z > max_z {
                                max_z = local.z;
                            }
                        }
                    }
                }
                (Some(max_z), Some(min_z))
            }
            (Some(bb), None) => (Some(bb.max.z), Some(bb.min.z)),
            _ => (None, None),
        };
        let height_ctx = crate::state::toolpath::HeightContext {
            safe_z,
            op_depth: operation.default_depth_for_heights(),
            stock_top_z: stock_bbox.max.z,
            stock_bottom_z: stock_bbox.min.z,
            model_top_z,
            model_bottom_z,
        };
        let mut heights = heights_config.resolve(&height_ctx);
        if let Some(fz) = face_top_z
            && heights_config.top_z.is_auto()
        {
            heights.top_z = fz;
            if heights_config.bottom_z.is_auto() {
                heights.bottom_z = fz - operation.default_depth_for_heights().abs();
            }
        }

        // Rest machining: when this toolpath cuts the stock left by previous
        // ops, seed generation with the simulated stock as it stood *before*
        // this op. Requires a prior simulation to have produced a snapshot
        // for this toolpath's id; if absent we FAIL HARD (do not fall back
        // to fresh stock — a fine rest tool would clear the whole part
        // instead of the leftover: unbounded compute and a wrong result).
        //
        // F.4: the snapshot is looked up directly by toolpath id via
        // `SimulationState::prior_stock_for`, which is populated from the
        // same `prior_stocks` map core's `generate_toolpath` gate checks
        // (`sim.prior_stocks.get(&tc.id)` in `session/compute.rs`). This
        // replaces a `boundaries()`-position / `checkpoints()`-lookup that
        // could only ever find a snapshot for a toolpath that already had
        // its OWN boundary — i.e. one that had already been generated —
        // so an ungenerated `FromRemainingStock` toolpath could never
        // regenerate after a fresh project load, even once its predecessor
        // had been simulated. `prior_stocks` now also carries a phantom
        // snapshot for the first pending toolpath in each group (see
        // `rs_cam_core::compute::simulate::SimGroupEntry::
        // phantom_prior_stock`), which is exactly the case this gate needs
        // to unblock.
        let prior_stock: Option<TriDexelStock> = if stock_source == StockSource::FromRemainingStock
        {
            let found = self
                .state
                .simulation
                .prior_stock_for(tp_id)
                .map(|stock| stock.as_ref().clone());
            let Some(found) = found else {
                self.fail_toolpath_submit(
                    tp_id,
                    format!(
                        "'{toolpath_name}' uses remaining stock (rest machining) but no prior \
                         simulated stock is available — run a simulation of the preceding \
                         operations first, then regenerate. (Not falling back to fresh stock.)"
                    ),
                );
                self.push_notification(
                    format!(
                        "Rest machining: '{toolpath_name}' has no prior simulated stock — run a \
                         simulation first, then regenerate. (Not falling back to fresh stock.)"
                    ),
                    super::super::Severity::Error,
                );
                return;
            };
            Some(found)
        } else {
            None
        };
        let cutting_levels = operation.cutting_levels(heights.top_z);
        let material = stock_snapshot.material;

        // P2.2/P2.3 (rest-region boundary): resolve `DerivedRestRegions` now,
        // while we still have full session + gui access — mirrors
        // `prev_tool_radius` / `reference_tool_cfg` above. The worker's
        // `ComputeRequest` is scoped to this one toolpath, so any
        // cross-toolpath lookup has to happen here, not in the worker.
        //
        // Fail-hard precondition, same shape and wording as core's
        // `ProjectSession::resolve_derived_rest_region_polys`
        // (session/compute.rs): a toolpath whose enabled boundary
        // references a missing / self-referential / ungenerated /
        // regionless source toolpath refuses to generate rather than
        // silently falling back to the stock rectangle. The previous
        // silent fallback let a full-part toolpath through with no error
        // before the source ever ran, and again after the source ran
        // whenever its rest regions (genuine terrain rest analysis
        // commonly yields many disjoint islands) didn't union down to
        // exactly one polygon.
        let derived_rest_regions: Option<Vec<rs_cam_core::polygon::Polygon2>> = if boundary.enabled
            && let crate::state::toolpath::BoundarySource::DerivedRestRegions { source_toolpath_id } =
                &boundary.source
        {
            let source_id = *source_toolpath_id;
            if source_id == tp_id {
                self.fail_toolpath_submit(
                    tp_id,
                    "Boundary references this toolpath's own rest regions — a toolpath \
                     cannot use itself as the source for a derived-rest-regions \
                     boundary. Pick a different source toolpath."
                        .to_owned(),
                );
                self.push_notification(
                    format!(
                        "'{toolpath_name}': boundary references its own rest regions — pick \
                         a different source toolpath."
                    ),
                    super::super::Severity::Error,
                );
                return;
            }
            let Some((_, source_tc)) = self.state.session.find_toolpath_config_by_id(source_id)
            else {
                self.fail_toolpath_submit(
                    tp_id,
                    format!(
                        "Boundary references toolpath id {} for its rest regions, but no \
                         toolpath with that id exists anymore. Pick a different source \
                         toolpath for the boundary, or disable the boundary.",
                        source_id.0
                    ),
                );
                self.push_notification(
                    format!(
                        "'{toolpath_name}': rest-regions boundary source (id {}) no longer \
                         exists — pick a different source toolpath.",
                        source_id.0
                    ),
                    super::super::Severity::Error,
                );
                return;
            };
            let source_name = source_tc.name.clone();
            let Some(source_result) = self
                .state
                .gui
                .toolpath_rt
                .get(&source_id)
                .and_then(|rt| rt.result.as_ref())
            else {
                self.fail_toolpath_submit(
                    tp_id,
                    format!(
                        "'{source_name}' has no generated result yet — generate \
                         '{source_name}' first; its rest analysis produces the regions this \
                         boundary needs.",
                    ),
                );
                self.push_notification(
                    format!(
                        "'{toolpath_name}': rest-regions source '{source_name}' has no \
                         generated result yet — generate it first."
                    ),
                    super::super::Severity::Error,
                );
                return;
            };
            match source_result.annotated.rest_regions.as_ref() {
                Some(regions) if !regions.is_empty() => Some((**regions).clone()),
                _ => {
                    self.fail_toolpath_submit(
                        tp_id,
                        format!(
                            "'{source_name}' produced no rest regions — it must be a pencil \
                             operation with the rest-depth detector enabled, and its rest \
                             analysis must have found material above the threshold. Check \
                             the pencil rest-depth settings on '{source_name}' and \
                             regenerate it.",
                        ),
                    );
                    self.push_notification(
                        format!(
                            "'{toolpath_name}': rest-regions source '{source_name}' produced \
                             no rest regions — check its pencil rest-depth settings and \
                             regenerate it."
                        ),
                        super::super::Severity::Error,
                    );
                    return;
                }
            }
        } else {
            None
        };

        // P1 quantitative linker: mirror core's `session::compute::generate_toolpath`,
        // which builds `LinkKinematics` from `self.machine` (see the comment there
        // for why each accessor is used).
        let machine = self.state.session.machine();
        let link_kinematics = Some(rs_cam_core::machine_kinematics::LinkKinematics {
            kinematics: machine.effective_kinematics(),
            max_feed_mm_min: machine.cutting_feed_ceiling_mm_min().max(1.0),
            rapid_feed_mm_min: machine.max_feed_mm_min.max(1.0),
        });

        self.compute.submit_toolpath(ComputeRequest {
            toolpath_id: tp_id,
            toolpath_name,
            debug_options,
            polygons,
            mesh,
            enriched_mesh,
            face_selection,
            operation,
            dressups,
            stock_source,
            tool,
            safe_z,
            prev_tool_radius,
            reference_tool_cfg,
            stock_bbox: Some(stock_bbox),
            boundary,
            keep_out_footprints,
            heights,
            cutting_levels,
            prior_stock,
            material,
            derived_rest_regions,
            rest_analysis,
            link_kinematics,
        });
    }

    /// Mark every toolpath whose *enabled* boundary is `DerivedRestRegions`
    /// referencing `source_id` as stale, using the same `stale_since`
    /// mechanism `mcp_apply_stale` uses for direct config edits
    /// (`app/mcp.rs::mcp_apply_stale`). A `DerivedRestRegions` boundary's
    /// clip depends entirely on the source toolpath's cached
    /// `rest_regions` — any regeneration of the source (regions changed,
    /// vanished, or newly appeared) or its removal invalidates every
    /// dependent's cached result just as surely as editing the dependent's
    /// own boundary config would, so this sweep is called from both the
    /// generation-completion handler (`drain_compute_results`, below) and
    /// `handle_remove_toolpath` (`controller/events/toolpath.rs`).
    pub(crate) fn mark_derived_rest_dependents_stale(&mut self, source_id: ToolpathId) {
        let dependent_ids: Vec<ToolpathId> = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .filter(|tc| {
                tc.boundary.enabled
                    && matches!(
                        tc.boundary.source,
                        crate::state::toolpath::BoundarySource::DerivedRestRegions {
                            source_toolpath_id,
                        } if source_toolpath_id == source_id
                    )
            })
            .map(|tc| tc.id)
            .collect();
        if dependent_ids.is_empty() {
            return;
        }
        let now = std::time::Instant::now();
        for id in dependent_ids {
            self.state.gui.toolpath_rt_or_default(id).stale_since = Some(now);
        }
    }

    // SAFETY: tp_index from position() within setup.toolpaths, slice always in bounds
    #[allow(clippy::indexing_slicing)]
    pub(crate) fn drain_compute_results(&mut self) {
        for message in self.compute.drain_results() {
            match message {
                ComputeMessage::Toolpath(result) => {
                    let tp_id = result.toolpath_id;
                    let rt = self.state.gui.toolpath_rt_or_default(tp_id);
                    rt.debug_trace = result.debug_trace.clone();
                    rt.semantic_trace = result.semantic_trace.clone();
                    rt.debug_trace_path = result.debug_trace_path.clone();
                    match result.result {
                        Ok(computed) => {
                            rt.status = ComputeStatus::Done;
                            // Sync the core-side `session.results` cache so the
                            // single point of truth for "this toolpath has a
                            // fresh result" is `ProjectSession`, not the
                            // viz-side `gui.toolpath_rt`. Without this, the
                            // gcode export / load-report paths read stale-empty
                            // `session.results[idx]` after Apply (see
                            // `planning/F1_RCA.md`).
                            if let Some((tp_index, _)) =
                                self.state.session.find_toolpath_config_by_id(tp_id)
                            {
                                // Honour the §6.E dual-representation
                                // invariant: drill ops carry both the
                                // DrillOp payload and the annotated
                                // toolpath. The worker already built the
                                // DrillOp into `computed.drill_op`;
                                // routing it through here so
                                // `session.results[idx].drill_op()`
                                // returns Some for drill TPs matches the
                                // `session.generate_toolpath` production
                                // path. Without this, drill_gates on the
                                // tool-load report never populate (the
                                // gate evaluator reads `result.drill_op()`).
                                let op_data = match &computed.drill_op {
                                    Some(drill_op_arc) => rs_cam_core::drill_op::OpData::DrillOp(
                                        Arc::clone(drill_op_arc),
                                        Arc::clone(&computed.annotated),
                                    ),
                                    None => rs_cam_core::drill_op::OpData::Toolpath(Arc::clone(
                                        &computed.annotated,
                                    )),
                                };
                                let core_result = rs_cam_core::session::ToolpathComputeResult {
                                    op_data,
                                    stats: computed.stats.clone(),
                                    // Debug + semantic traces stay viz-side
                                    // (Arc'd on `rt.debug_trace` /
                                    // `rt.semantic_trace` above). Core readers
                                    // currently only consume `annotated.spans`
                                    // from `session.results`; bloating the
                                    // cache with owned trace copies is wasted
                                    // work.
                                    debug_trace: None,
                                    semantic_trace: None,
                                };
                                let _ = self.state.session.insert_result(tp_index, core_result);
                            }
                            rt.result = Some(computed);
                            // Any toolpath whose `DerivedRestRegions` boundary
                            // depends on this one just saw its source result
                            // replaced (rest_regions may have appeared,
                            // changed, or vanished) — force a regenerate so
                            // the dependent re-resolves against the fresh
                            // regions instead of clipping against a stale set.
                            self.mark_derived_rest_dependents_stale(tp_id);
                        }
                        Err(ComputeError::Cancelled) => {
                            rt.status = ComputeStatus::Pending;
                            rt.result = None;
                        }
                        Err(ComputeError::Message(error)) => {
                            rt.status = ComputeStatus::Error(error);
                            rt.result = None;
                        }
                    }
                    self.pending_upload = true;

                    // U4 reconciliation: clear this toolpath from the
                    // pending list. When the list empties we kick the
                    // reconciliation sim — this fires once, after the
                    // last applied toolpath finishes regenerating.
                    self.state
                        .pending_reconciliation_for_ids
                        .retain(|id| *id != tp_id);
                    if self.state.pending_reconciliation_for_ids.is_empty()
                        && matches!(
                            self.state.optimize_project.as_ref().map(|v| &v.status),
                            Some(crate::state::OptimizeProjectStatus::Reconciling(_))
                        )
                    {
                        self.run_simulation_with_all();
                    }

                    // Roadmap F.2 — auto-verify after per-TP Apply. If
                    // this regen is for the just-applied candidate,
                    // clear the pending flag and kick a full project
                    // sim so the user sees the verified live verdict
                    // without having to click Run Simulation by hand.
                    if self.state.pending_apply_resim == Some(tp_id.0) {
                        self.state.pending_apply_resim = None;
                        self.run_simulation_with_all();
                    }

                    // Notify pending MCP request for this toolpath
                    #[cfg(feature = "mcp")]
                    self.notify_mcp_toolpath_complete(tp_id);
                }
                ComputeMessage::Simulation(result) => match result {
                    Ok(simulation) => {
                        if simulation.resolution_clamped {
                            self.push_notification(
                                "Sim resolution was coarsened to fit grid limits — \
                                 consider reducing stock size or increasing resolution"
                                    .to_owned(),
                                crate::controller::Severity::Warning,
                            );
                        }
                        if simulation.mesh.indices.is_empty() {
                            self.push_notification(
                                "Simulation produced an empty mesh — \
                                 try increasing resolution or check stock dimensions"
                                    .to_owned(),
                                crate::controller::Severity::Warning,
                            );
                        }
                        let boundaries: Vec<_> = simulation
                            .boundaries
                            .iter()
                            .map(|boundary| crate::state::simulation::ToolpathBoundary {
                                id: boundary.id,
                                name: boundary.name.clone(),
                                tool_name: boundary.tool_name.clone(),
                                start_move: boundary.start_move,
                                end_move: boundary.end_move,
                                direction: boundary.direction,
                            })
                            .collect();

                        let setup_boundaries = {
                            let mut sbs = Vec::new();
                            let mut last_setup_id = None;
                            for boundary in &boundaries {
                                let setup_id = self.setup_of_toolpath(boundary.id);
                                if setup_id != last_setup_id {
                                    if let Some(setup_id) = setup_id {
                                        let setup_name = self
                                            .state
                                            .session
                                            .list_setups()
                                            .iter()
                                            .find(|s| s.id == setup_id.0)
                                            .map(|s| s.name.clone())
                                            .unwrap_or_default();
                                        sbs.push(crate::state::simulation::SetupBoundary {
                                            setup_id,
                                            setup_name,
                                            start_move: boundary.start_move,
                                        });
                                    }
                                    last_setup_id = setup_id;
                                }
                            }
                            sbs
                        };

                        let checkpoints: Vec<_> = simulation
                            .checkpoints
                            .into_iter()
                            .map(|checkpoint| crate::state::simulation::SimCheckpoint {
                                boundary_index: checkpoint.boundary_index,
                                mesh: checkpoint.mesh,
                                stock: Some(checkpoint.stock),
                            })
                            .collect();

                        // F.4 — retain the per-toolpath (and phantom)
                        // prior-stock snapshots so the submit-time
                        // FromRemainingStock gate can look them up by id.
                        let prior_stocks = simulation.prior_stocks;

                        if !simulation.rapid_collisions.is_empty() {
                            tracing::warn!(
                                "{} rapid collisions detected",
                                simulation.rapid_collisions.len()
                            );
                        }
                        self.state.simulation.checks.rapid_collisions = simulation.rapid_collisions;
                        self.state.simulation.checks.rapid_collision_move_indices =
                            simulation.rapid_collision_move_indices;

                        self.state.simulation.playback.display_deviations = simulation.deviations;
                        self.state.simulation.playback.display_mesh = None;
                        self.state.simulation.playback.display_mesh_move = None;
                        self.state.simulation.playback.last_mesh_upload_at = None;
                        self.state.simulation.playback.tool_gpu_move = None;
                        self.state.simulation.playback.display_mesh_preview = false;
                        self.state.simulation.playback.scrub_drag_active = false;

                        let stock = self.state.session.stock_config();
                        let stock_bbox = rs_cam_core::geo::BoundingBox3 {
                            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                            max: rs_cam_core::geo::P3::new(stock.x, stock.y, stock.z),
                        };

                        self.state.simulation.results = Some(SimulationResults {
                            mesh: simulation.mesh,
                            total_moves: simulation.total_moves,
                            boundaries,
                            setup_boundaries,
                            checkpoints,
                            selected_toolpaths: None,
                            playback_data: simulation.playback_data,
                            stock_bbox,
                            cut_trace: simulation.cut_trace,
                            cut_trace_path: simulation.cut_trace_path,
                            prior_stocks,
                        });

                        // F-039 — apply adaptive feed modulation to the
                        // just-completed sim trace (unified load model §10.6,
                        // option A). The async worker runs the dexel sim only;
                        // modulation needs session context (material / machine
                        // / vendor LUT) so it runs here on the main thread. The
                        // post-pass stamps `modulation_summaries` onto the trace
                        // — so the Feeds-tab "operating point" card + the
                        // tool-load report populate — and swaps the modulated
                        // toolpaths into `session.results`, which G-code export
                        // reads, so exported feeds are the optimized per-move
                        // schedule. Default-on in the GUI. Take the trace out
                        // and put it back so the session (results) and the
                        // viz-side cut_trace are borrowed disjointly.
                        {
                            let opts = rs_cam_core::session::SimulationOptions {
                                adaptive_feed_modulation: true,
                                modulation_strategy:
                                    rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
                                modulation_aggressiveness: 1.0,
                                ..Default::default()
                            };
                            let mut cut_trace = self
                                .state
                                .simulation
                                .results
                                .as_mut()
                                .and_then(|r| r.cut_trace.take());
                            if cut_trace.is_some() {
                                self.state
                                    .session
                                    .modulate_simulation_trace(&mut cut_trace, &opts);
                                if let Some(results) = self.state.simulation.results.as_mut() {
                                    results.cut_trace = cut_trace;
                                }
                            }
                        }

                        let inspect_target =
                            self.state.simulation.debug.pending_inspect_toolpath.take();
                        if let Some(move_index) = inspect_target.and_then(|toolpath_id| {
                            self.state
                                .simulation
                                .boundaries()
                                .iter()
                                .find(|boundary| boundary.id == toolpath_id)
                                .map(|boundary| boundary.start_move)
                        }) {
                            self.state.simulation.playback.current_move = move_index;
                            self.state.simulation.playback.playing = false;
                        } else {
                            self.state.simulation.playback.current_move = 0;
                            self.state.simulation.playback.playing = false;
                        }

                        let initial_stock = TriDexelStock::from_bounds(
                            &stock_bbox,
                            self.state.simulation.resolution,
                        );
                        self.state.simulation.playback.live_stock = Some(initial_stock);
                        self.state.simulation.playback.live_sim_move = 0;

                        let prev_gen = self
                            .state
                            .simulation
                            .last_run
                            .as_ref()
                            .map_or(0, |m| m.sim_generation);
                        self.state.simulation.last_run = Some(SimulationRunMeta {
                            sim_generation: prev_gen + 1,
                            last_sim_edit_counter: self.state.gui.edit_counter,
                        });

                        self.pending_upload = true;

                        // U4 reconciliation: if the rollup is in
                        // Reconciling state, populate per-row
                        // reconciled values from the new trace and
                        // transition to Reconciled.
                        self.maybe_finalize_reconciliation();

                        // Notify pending MCP simulation request
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_simulation_complete();
                    }
                    Err(ComputeError::Cancelled) => {
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_simulation_error("Simulation cancelled");
                    }
                    Err(ComputeError::Message(error)) => {
                        tracing::error!("Simulation failed: {error}");
                        self.push_notification(
                            format!("Simulation failed: {error}"),
                            super::super::Severity::Error,
                        );
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_simulation_error(&error);
                    }
                },
                ComputeMessage::Collision(result) => match result {
                    Ok(collision) => {
                        let count = collision.report.collisions.len();
                        if count == 0 {
                            tracing::info!("No holder clearance issues detected");
                            self.push_notification(
                                "No holder clearance issues detected".into(),
                                super::super::Severity::Info,
                            );
                        } else {
                            let msg = format!(
                                "{} holder clearance issues, min safe stickout: {:.1} mm",
                                count, collision.report.min_safe_stickout
                            );
                            tracing::warn!("{msg}");
                            self.push_notification(msg, super::super::Severity::Warning);
                        }
                        self.state.simulation.checks.holder_collision_count = count;
                        self.state.simulation.checks.min_safe_stickout = if count > 0 {
                            Some(collision.report.min_safe_stickout)
                        } else {
                            None
                        };
                        // Extract MCP response data before moving ownership
                        #[cfg(feature = "mcp")]
                        let mcp_collision_count = collision.report.collisions.len();
                        #[cfg(feature = "mcp")]
                        let mcp_min_safe_stickout = collision.report.min_safe_stickout;
                        #[cfg(feature = "mcp")]
                        let mcp_is_clear = collision.report.is_clear();

                        self.state.simulation.checks.collision_report = Some(collision.report);
                        self.collision_positions = collision.positions;
                        self.pending_upload = true;

                        // Notify pending MCP collision request
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_collision_complete(
                            mcp_collision_count,
                            mcp_min_safe_stickout,
                            mcp_is_clear,
                        );
                    }
                    Err(ComputeError::Cancelled) => {
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_collision_error("Collision check cancelled");
                    }
                    Err(ComputeError::Message(error)) => {
                        tracing::error!("Collision check failed: {error}");
                        self.push_notification(
                            format!("Collision check failed: {error}"),
                            super::super::Severity::Error,
                        );
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_collision_error(&error);
                    }
                },
                ComputeMessage::Optimize(result) => {
                    self.handle_optimize_result(*result);
                }
            }
        }
    }

    /// Restore the session moved into the Optimize lane and stash the
    /// outcome on `AppState`. The per-toolpath modal (U2 retrofit) and
    /// the project rollup (U3) hook off the cached state from here.
    fn handle_optimize_result(&mut self, result: crate::compute::OptimizeResult) {
        use crate::compute::OptimizeResultKind;

        // Restore the session first — every other update reads it.
        self.state.session = result.session;
        self.state.is_optimizing = false;

        match result.kind {
            OptimizeResultKind::Toolpath {
                toolpath_id,
                outcome,
            } => {
                if let Some(modal) = self.state.optimize_modal.as_mut() {
                    if modal.toolpath_id == toolpath_id {
                        modal.status = crate::state::OptimizeRunStatus::Ready(outcome);
                    } else {
                        // The modal was reopened on a different
                        // toolpath while the worker was running —
                        // discard the stale result.
                        tracing::debug!(
                            "Optimize result for tp {toolpath_id} dropped — modal now open on tp {}",
                            modal.toolpath_id
                        );
                    }
                } else {
                    tracing::debug!(
                        "Optimize result for tp {toolpath_id} arrived after modal was closed — discarded"
                    );
                }
            }
            OptimizeResultKind::Project { report } => {
                // Default-select every row that has a recommended
                // candidate. The user can flip individual rows before
                // clicking Apply selected.
                let row_selected: Vec<bool> = report
                    .per_toolpath
                    .iter()
                    .map(|(_, outcome)| outcome.first_safe().is_some())
                    .collect();
                if let Some(view) = self.state.optimize_project.as_mut() {
                    view.status = crate::state::OptimizeProjectStatus::Ready(report);
                    view.row_selected = row_selected;
                } else {
                    // Worker returned a result for a view that was
                    // already closed (cancelled) — discard.
                    tracing::debug!(
                        "Project optimize result arrived after view was closed — discarded"
                    );
                }
            }
        }
    }

    /// U4: if the rollup is in `Reconciling` state and the latest sim
    /// result just landed, populate `reconciled_cycle_time_s` /
    /// `reconciled_verdict` per applied row and transition to
    /// `Reconciled`. Each row's reconciled values come from running
    /// the existing project load report against the new trace and
    /// reading per-toolpath cycle times directly off the trace.
    pub(crate) fn maybe_finalize_reconciliation(&mut self) {
        use rs_cam_core::tool_load::optimize::{OutcomeKind, ProjectOptimizeReport};

        let Some(view) = self.state.optimize_project.as_ref() else {
            return;
        };
        let crate::state::OptimizeProjectStatus::Reconciling(report) = &view.status else {
            return;
        };
        let Some(sim_results) = self.state.simulation.results.as_ref() else {
            return;
        };
        let Some(trace) = sim_results.cut_trace.as_deref() else {
            return;
        };

        // Compute per-toolpath verdicts off the new trace.
        let load_report = rs_cam_core::gcode::project_load_report(&self.state.session, Some(trace));
        // Map toolpath_id -> verdict for fast lookup. Index by id
        // (not toolpath_index) because the report uses ids.
        let mut verdict_by_id: std::collections::HashMap<
            rs_cam_core::ToolpathId,
            &rs_cam_core::tool_load::ToolpathLoadVerdict,
        > = std::collections::HashMap::new();
        for v in &load_report.per_toolpath {
            verdict_by_id.insert(v.toolpath_id, v);
        }

        // Walk the report and populate the first_safe candidate's
        // reconciled values for each row that has one.
        let mut updated_report = report.clone();
        for (tp_index, outcome) in updated_report.per_toolpath.iter_mut() {
            let toolpath_id = self
                .state
                .session
                .toolpath_configs()
                .get(*tp_index)
                .map(|tc| tc.id);
            let Some(toolpath_id) = toolpath_id else {
                continue;
            };
            let cycle = trace
                .toolpath_summaries
                .iter()
                .find(|s| s.toolpath_id == toolpath_id)
                .map(|s| s.total_runtime_s);
            let verdict = verdict_by_id.get(&toolpath_id).cloned().cloned();
            if outcome.kind == OutcomeKind::Ranked {
                // The applied candidate is `first_safe` per the rollup
                // Apply path. Find its index by re-running the same
                // selector, then update that candidate. We fall back
                // to candidate index 1 if the rollup logic disagrees
                // (e.g. the candidate set changed mid-flight, which
                // shouldn't happen since the report is immutable).
                let candidates = &mut outcome.candidates;
                let idx = ProjectOptimizeReport::first_safe_index(candidates).unwrap_or(0);
                if idx > 0
                    && let Some(c) = candidates.get_mut(idx)
                {
                    c.reconciled_cycle_time_s = cycle;
                    c.reconciled_verdict = verdict;
                }
            }
        }

        if let Some(view) = self.state.optimize_project.as_mut() {
            view.status = crate::state::OptimizeProjectStatus::Reconciled(updated_report);
        }
    }

    // ── MCP notification helpers ─────────────────────────────────────
    #[cfg(feature = "mcp")]
    fn notify_mcp_toolpath_complete(&mut self, tp_id: crate::state::toolpath::ToolpathId) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp {
            // Check individual toolpath request
            if let Some(sender) = pending.toolpath.remove(&tp_id) {
                let rt = self.state.gui.toolpath_rt.get(&tp_id);
                let resp = match rt.and_then(|rt| rt.result.as_ref()) {
                    Some(result) => json_str(serde_json::json!({
                        "id": tp_id.0,
                        "move_count": result.stats.move_count,
                        "cutting_distance_mm": result.stats.cutting_distance,
                        "rapid_distance_mm": result.stats.rapid_distance,
                    })),
                    None => {
                        let status_msg = rt
                            .map(|rt| match &rt.status {
                                ComputeStatus::Error(e) => format!("Error: {e}"),
                                // The only way a drained result reaches this
                                // arm with `Pending` is the cancel path a few
                                // lines above (`Err(ComputeError::Cancelled)`
                                // resets status to `Pending`) — call it out
                                // by name instead of the generic fallback so
                                // an MCP caller waiting on `cancel_generation`
                                // sees an unambiguous outcome.
                                ComputeStatus::Pending => "Generation was cancelled".to_owned(),
                                _ => "Toolpath generation produced no result".to_owned(),
                            })
                            .unwrap_or_else(|| "Toolpath not found".to_owned());
                        json_str(serde_json::json!({"error": status_msg}))
                    }
                };
                let _ = sender.send(McpResponse { result: Ok(resp) });
            }

            // Check generate_all tracking
            if let Some(ref mut ga) = pending.generate_all {
                if let Some(pos) = ga.remaining.iter().position(|id| *id == tp_id) {
                    ga.remaining.remove(pos);
                    let rt = self.state.gui.toolpath_rt.get(&tp_id);
                    if rt.and_then(|rt| rt.result.as_ref()).is_some() {
                        ga.completed += 1;
                    } else {
                        ga.failed += 1;
                        // Roadmap E.4 — distinguish "completed cleanly with
                        // zero moves" (likely a config issue: depth/stock/
                        // model) from a thrown error or an in-flight status.
                        let tp_name = self
                            .state
                            .session
                            .find_toolpath_config_by_id(tp_id)
                            .map(|(_, tc)| tc.name.clone())
                            .unwrap_or_else(|| format!("toolpath {}", tp_id.0));
                        let error_msg = match rt.map(|rt| &rt.status) {
                            Some(crate::state::toolpath::ComputeStatus::Error(e)) => e.clone(),
                            Some(crate::state::toolpath::ComputeStatus::Done) => format!(
                                "{tp_name}: completed with no moves — check depth, stock, or model assignment"
                            ),
                            // Cancel resets status to `Pending` (see the
                            // single-toolpath branch above) — name it
                            // explicitly instead of falling into the
                            // generic "status=Pending" label below.
                            Some(crate::state::toolpath::ComputeStatus::Pending) => {
                                format!("{tp_name}: generation cancelled")
                            }
                            Some(status) => {
                                let label = match status {
                                    crate::state::toolpath::ComputeStatus::Pending => "Pending",
                                    crate::state::toolpath::ComputeStatus::Computing => "Computing",
                                    crate::state::toolpath::ComputeStatus::Done => "Done",
                                    crate::state::toolpath::ComputeStatus::Error(_) => "Error",
                                };
                                format!("{tp_name}: no result, status={label}")
                            }
                            None => format!("{tp_name}: toolpath runtime not found"),
                        };
                        ga.errors.push((tp_id.0, error_msg));
                    }

                    // Send progress update via the progress channel (non-blocking).
                    if let Some(ref progress_tx) = ga.progress_tx {
                        let total = (ga.completed + ga.failed + ga.remaining.len()) as f64;
                        let current = (ga.completed + ga.failed) as f64;
                        let tp_name = self
                            .state
                            .session
                            .find_toolpath_config_by_id(tp_id)
                            .map(|(_, tc)| tc.name.clone())
                            .unwrap_or_else(|| format!("toolpath {}", tp_id.0));
                        let msg = format!(
                            "Completed {}/{}: {}",
                            current as usize, total as usize, tp_name
                        );
                        let _ = progress_tx.try_send(crate::mcp_bridge::ProgressUpdate {
                            message: msg,
                            progress: current,
                            total: Some(total),
                        });
                    }
                }

                if ga.remaining.is_empty()
                    && let Some(ga) = pending.generate_all.take()
                {
                    let resp = if ga.errors.is_empty() {
                        rs_cam_mcp::server::text(format!("Generated {} toolpaths", ga.completed,))
                    } else {
                        let error_details: Vec<String> = ga
                            .errors
                            .iter()
                            .map(|(id, msg)| format!("  toolpath {id}: {msg}"))
                            .collect();
                        rs_cam_mcp::server::text(format!(
                            "Generated {} toolpaths ({} failed):\n{}",
                            ga.completed,
                            ga.failed,
                            error_details.join("\n"),
                        ))
                    };
                    let _ = ga.response_tx.send(McpResponse { result: Ok(resp) });
                }
            }
        }
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_simulation_complete(&mut self) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.simulation.take()
        {
            let resp = self.build_mcp_diagnostics();
            let _ = sender.send(McpResponse {
                result: Ok(json_str(resp)),
            });
        }
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_simulation_error(&mut self, error: &str) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.simulation.take()
        {
            let _ = sender.send(McpResponse {
                result: Ok(json_str(serde_json::json!({"error": error}))),
            });
        }
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_collision_complete(
        &mut self,
        collision_count: usize,
        min_safe_stickout: f64,
        is_clear: bool,
    ) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.collision.take()
        {
            let resp = json_str(serde_json::json!({
                "collision_count": collision_count,
                "min_safe_stickout_mm": min_safe_stickout,
                "is_clear": is_clear,
            }));
            let _ = sender.send(McpResponse { result: Ok(resp) });
        }
    }

    /// Build diagnostics JSON from GUI state (toolpath_rt + simulation results).
    ///
    /// Unlike `session.diagnostics()` which reads from the session's internal
    /// result cache (only populated by the standalone MCP), this reads from
    /// `gui.toolpath_rt` and `state.simulation.results` — where the GUI's
    /// compute pipeline actually stores data.
    #[cfg(feature = "mcp")]
    pub fn build_mcp_diagnostics(&self) -> serde_json::Value {
        let session = &self.state.session;
        let gui = &self.state.gui;

        let mut per_toolpath = Vec::new();
        let mut runtime_errors = Vec::new();
        for (index, tc) in session.toolpath_configs().iter().enumerate() {
            let tool_name = session
                .tools()
                .iter()
                .find(|t| t.id.0 == tc.tool_id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            if let Some(rt) = gui.toolpath_rt.get(&tc.id) {
                let (status, error) = match &rt.status {
                    ComputeStatus::Pending => ("Pending", None),
                    ComputeStatus::Computing => ("Computing", None),
                    ComputeStatus::Done => ("Done", None),
                    ComputeStatus::Error(e) => ("Error", Some(e.clone())),
                };
                if let Some(error) = error.clone() {
                    runtime_errors.push(serde_json::json!({
                        "toolpath_index": index,
                        "toolpath_id": tc.id,
                        "name": tc.name,
                        "error": error,
                    }));
                }
                let mut row = serde_json::json!({
                    "toolpath_index": index,
                    "toolpath_id": tc.id,
                    "name": tc.name,
                    "operation_type": tc.operation.label(),
                    "tool_name": tool_name,
                    "status": status,
                    "error": error,
                    "stale": rt.stale_since.is_some(),
                });
                if let Some(ref result) = rt.result {
                    // SAFETY: row is a known object constructed above.
                    #[allow(clippy::indexing_slicing)]
                    {
                        row["move_count"] = serde_json::json!(result.stats.move_count);
                        row["cutting_distance_mm"] =
                            serde_json::json!(result.stats.cutting_distance);
                        row["rapid_distance_mm"] = serde_json::json!(result.stats.rapid_distance);
                    }
                }
                per_toolpath.push(row);
            }
        }

        let (total_runtime_s, air_cut_pct, avg_engagement) = if let Some(ref sim_results) =
            self.state.simulation.results
            && let Some(ref ct) = sim_results.cut_trace
        {
            let s = &ct.summary;
            let air = if s.total_runtime_s > 0.0 {
                s.air_cut_time_s / s.total_runtime_s * 100.0
            } else {
                0.0
            };
            (s.total_runtime_s, air, s.average_engagement)
        } else {
            (0.0, 0.0, 0.0)
        };

        let rapid_collision_count = self.state.simulation.checks.rapid_collisions.len();

        let verdict = if rapid_collision_count > 0 {
            "WARNING: rapid collisions detected"
        } else if air_cut_pct > 20.0 {
            "WARNING: high air cutting"
        } else {
            "OK"
        };

        let mut resp = serde_json::json!({
            "total_runtime_s": total_runtime_s,
            "air_cut_percentage": air_cut_pct,
            "average_engagement": avg_engagement,
            "collision_count": 0,
            "rapid_collision_count": rapid_collision_count,
            "verdict": verdict,
            "per_toolpath": per_toolpath,
            "runtime_errors": runtime_errors,
        });

        if let Some(ref sim_results) = self.state.simulation.results
            && let Some(ref ct) = sim_results.cut_trace
        {
            // SAFETY: resp is a known JSON object we just constructed
            #[allow(clippy::indexing_slicing)]
            {
                resp["semantic_summary_count"] = serde_json::json!(ct.semantic_summaries.len());
                resp["hotspot_count"] = serde_json::json!(ct.hotspots.len());
                resp["issue_count"] = serde_json::json!(ct.issues.len());
            }
        }

        resp
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_collision_error(&mut self, error: &str) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.collision.take()
        {
            let _ = sender.send(McpResponse {
                result: Ok(json_str(serde_json::json!({"error": error}))),
            });
        }
    }
}
