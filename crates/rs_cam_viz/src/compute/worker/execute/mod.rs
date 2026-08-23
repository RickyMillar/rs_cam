use super::helpers::{
    apply_dressups, build_simulation_cut_artifact, build_trace_artifact, debug_artifact_dir,
    effective_safe_z, simulation_metric_artifact_dir,
};
use super::{
    Arc, AtomicBool, ComputeError, ComputeRequest, SimBoundary, SimulationRequest,
    SimulationResult, ToolpathPhaseTracker, ToolpathResult,
};
#[cfg(test)]
use super::{
    BoundingBox3, DressupConfig, OperationConfig, StockSource, ToolConfig, ToolType, ToolpathId,
};
#[cfg(test)]
use crate::state::toolpath::{
    FaceConfig, FaceDirection, HeightContext, HeightsConfig, InlayConfig, ProfileConfig,
    RestConfig, ZigzagConfig,
};
use rs_cam_core::compute::build_cutter;
use rs_cam_core::compute::execute::execute_operation_annotated_with_regions;
#[cfg(test)]
use rs_cam_core::geo::P3;
#[cfg(test)]
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::semantic_trace::{SemanticKey, ToolpathSemanticKind};
#[cfg(test)]
use rs_cam_core::toolpath::MoveType;

/// How many simulation cut artifacts to retain in `target/simulation_metrics`
/// (G-SIMDUMP: each dump can be multiple GB at fine resolutions).
const SIM_CUT_ARTIFACT_RETAIN: usize = 5;

pub(super) struct ComputeExecutionOutcome {
    pub result: Result<ToolpathResult, ComputeError>,
    pub debug_trace: Option<Arc<rs_cam_core::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<std::path::PathBuf>,
}

/// Bridge function: delegates toolpath generation to core's `execute_operation`.
///
/// Builds the spatial index on-demand (only for 3D mesh operations), adds a
/// top-level semantic scope for the operation, and maps errors to `ComputeError`.
fn generate_via_core(
    req: &ComputeRequest,
    cancel: &AtomicBool,
    debug_ctx: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
    semantic_root: Option<&rs_cam_core::semantic_trace::ToolpathSemanticContext>,
    core_debug_span_id: Option<u64>,
) -> Result<
    (
        rs_cam_core::toolpath_spans::AnnotatedToolpath,
        rs_cam_core::compute::execute::GenerationFindings,
    ),
    ComputeError,
> {
    let tool_def = build_cutter(&req.tool);
    let mesh_ref = req.mesh.as_deref();
    // G8: memoised per mesh identity in `rs_cam_core::geom_cache`, matching
    // the core session path. This built the whole grid inside per-toolpath
    // resolution, so an 8-operation `generate_all` rebuilt the reference
    // 661 k-triangle index eight times, multiplied again by every fixpoint
    // round. `ComputeRequest.mesh` is the model's own `Arc` for an identity
    // setup, so consecutive toolpaths over one model share one build.
    let index = req
        .mesh
        .as_ref()
        .map(rs_cam_core::geom_cache::cached_auto_index);
    let index_ref = index.as_deref();
    let polys = req.polygons.as_deref().map(|v| v.as_slice());
    let default_bbox = rs_cam_core::geo::BoundingBox3::empty();
    let stock_bbox = req.stock_bbox.as_ref().unwrap_or(&default_bbox);
    // `state::toolpath::ResolvedHeights` is a re-export of the core type —
    // copy it whole so new fields (e.g. the pinned flags from the heights
    // audit) flow through without this bridge silently dropping them.
    let heights = req.heights;

    // Add top-level semantic scope for the operation
    let op_scope = semantic_root
        .map(|root| root.start_item(ToolpathSemanticKind::Operation, req.operation.label()));
    if let Some(scope) = op_scope.as_ref()
        && let Some(span_id) = core_debug_span_id
    {
        scope.set_debug_span_id(span_id);
    }
    let op_child_ctx = op_scope.as_ref().map(|scope| scope.context());

    // P2.4/RegionSet: pre-resolve the *set* of DerivedRestRegions (each
    // region individually keep-out-subtracted + offset via
    // `RegionSet::processed` — same per-region processing as
    // `session/compute.rs::resolve_generation_inputs`'s `pre_boundary_regions`,
    // which core's session path threads into `ExecutionContext.boundary_regions`)
    // so the GUI worker can do the same. Before P2.4, `generate_via_core` only
    // ever called the plain `execute_operation_annotated` wrapper
    // (`boundary_regions = None`), so the 7 finish ops that pre-clip
    // generation on `boundary_regions` (scallop's per-island concentric
    // rings, drop-cutter's sampling skip) generated across the WHOLE part on
    // the GUI path — only the post-generation enforcement clip below
    // (`apply_boundary_clip_multi`) trimmed the result. Same final
    // containment, but full-part generation cost, and a scallop toolpath
    // that visibly "does the whole area" before being cut down instead of
    // concentric per-island rings. This now matches the core session path
    // (`ProjectSession::generate_toolpath`). Computed before `pre_boundary`
    // below so the single-polygon collapse for adaptive3d's pre-clip can
    // reuse this exact processed set instead of re-deriving it.
    let pre_boundary_regions: Option<Vec<rs_cam_core::polygon::Polygon2>> = if req.boundary.enabled
        && matches!(
            req.boundary.source,
            rs_cam_core::compute::config::BoundarySource::DerivedRestRegions { .. }
        ) {
        req.derived_rest_regions.as_deref().and_then(|regions| {
            (!regions.is_empty()).then(|| {
                rs_cam_core::region_set::RegionSet::from_slice(regions)
                    .processed(&req.keep_out_footprints, req.boundary.offset)
                    .as_slice()
                    .to_vec()
            })
        })
    } else {
        None
    };

    // Pre-resolve containment polygon (silhouette/stock + keep-outs + offset)
    // for adaptive3d's internal stock pre-clip. The post-generation boundary
    // clip (later in this function) does its own tool-radius inset for cutter
    // CENTER gating — but for pre-clip we want the silhouette itself, so the
    // cutter footprint can validly stamp cells in the band [silhouette -
    // tool_radius, silhouette]. Mirrors session/compute.rs::resolve_containment_polygon.
    let pre_boundary: Option<rs_cam_core::polygon::Polygon2> = if req.boundary.enabled {
        use rs_cam_core::boundary::subtract_keepouts;
        use rs_cam_core::compute::config::BoundarySource;
        let stock_rect = || {
            Some(rs_cam_core::polygon::Polygon2::rectangle(
                stock_bbox.min.x,
                stock_bbox.min.y,
                stock_bbox.max.x,
                stock_bbox.max.y,
            ))
        };
        if matches!(
            req.boundary.source,
            BoundarySource::DerivedRestRegions { .. }
        ) {
            // P2.2/P2.3/RegionSet: adaptive3d's internal-stock pre-clip
            // wants a single containment polygon — reuse the
            // `pre_boundary_regions` set computed above (already
            // keep-out-subtracted + offset, per region) and collapse it
            // via `RegionSet::single_union` only when it resolves to
            // exactly one polygon. `None` just skips this pre-clip
            // optimization (costs adaptive3d some discarded pre-clearing,
            // not correctness); the real enforcement clip further down
            // uses the full region set via `apply_boundary_clip_multi`
            // regardless of whether this union collapsed.
            pre_boundary_regions.as_deref().and_then(|regions| {
                rs_cam_core::region_set::RegionSet::from_slice(regions).single_union()
            })
        } else {
            let mut poly = if let (Some(face_ids), Some(enriched)) =
                (&req.face_selection, &req.enriched_mesh)
            {
                enriched
                    .faces_boundary_as_polygon(face_ids)
                    .or_else(stock_rect)
            } else if matches!(req.boundary.source, BoundarySource::ModelSilhouette)
                && let Some(mesh) = req.mesh.as_ref()
            {
                // G8: memoised per mesh identity, like the core session
                // path. Same polygons, same order, same max_by tie-break —
                // only the rasterisation is shared.
                rs_cam_core::geom_cache::cached_silhouette(mesh)
                    .iter()
                    .max_by(|a, b| {
                        a.area()
                            .partial_cmp(&b.area())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .cloned()
            } else {
                stock_rect()
            };
            if let Some(p) = poly.as_mut()
                && !req.keep_out_footprints.is_empty()
            {
                *p = subtract_keepouts(p, &req.keep_out_footprints);
            }
            // Checkpoint C, D-3b (F-8). This was
            // `if let Some(largest) = ... { *p = largest }` with no `else`:
            // on an empty result the requested offset silently did not
            // happen and `p` kept its UN-OFFSET value, which for a negative
            // offset clips to a LARGER region than was asked for. Dropped
            // now, matching the multi-region path (D-3c) and the session's
            // `resolve_containment_polygon`.
            if let Some(p) = poly.as_ref()
                && req.boundary.offset.abs() > 1e-9
            {
                use rs_cam_core::boundary::{UserOffsetOutcome, apply_user_boundary_offset};
                poly = match apply_user_boundary_offset(p, req.boundary.offset) {
                    UserOffsetOutcome::Resolved(p) => Some(p),
                    UserOffsetOutcome::Collapsed => None,
                    UserOffsetOutcome::Failed(failure) => {
                        return Err(rs_cam_core::compute::OperationError::MissingGeometry(
                            format!(
                                "the machining boundary's {:+.3} mm offset could not \
                                 be computed: {}. Refusing rather than continuing \
                                 with the UN-OFFSET boundary.",
                                req.boundary.offset,
                                failure.describe(),
                            ),
                        )
                        .into());
                    }
                };
            }
            poly
        }
    } else {
        None
    };

    // P2.5: `rest_analysis` needs to flow through regardless of whether
    // `pre_boundary_regions` resolved to anything, so this always goes
    // through the `_with_regions` variant now (unconditional `None` for
    // `boundary_regions` is a byte-identical no-op, matching what the
    // plain `execute_operation_annotated` wrapper did before P2.5).
    //
    let result = execute_operation_annotated_with_regions(
        &req.operation,
        mesh_ref,
        index_ref,
        polys,
        &tool_def,
        &req.tool,
        &heights,
        &req.cutting_levels,
        stock_bbox,
        req.prev_tool_radius,
        req.reference_tool_cfg.clone(),
        debug_ctx,
        cancel,
        req.prior_stock.as_ref(),
        op_child_ctx.as_ref(),
        pre_boundary.as_ref(),
        pre_boundary_regions.as_deref(),
        Some(&req.rest_analysis),
        req.link_kinematics.clone(),
        // G-DRILLPICK-FRAME: the controller transforms mesh and polygons into
        // the emission frame before submitting, but config-carried coordinates
        // (drill `selected_holes`) ride along untransformed, so the generator
        // needs the matrix itself. `ComputeRequest` now forwards it — this is
        // the GUI/MCP path, i.e. the one a real job actually runs, so leaving
        // it `None` would have meant the fix applied only to the CLI.
        req.setup_transform.as_ref(),
    )
    .map_err(ComputeError::from)?;
    let (result, findings) = result;

    if let Some(scope) = op_scope.as_ref()
        && !result.toolpath.moves.is_empty()
    {
        scope.bind_to_toolpath(&result.toolpath, 0, result.toolpath.moves.len());
    }

    // Return the FULL annotated toolpath: narrowing to (toolpath, spans) here
    // used to drop rest_grid / rest_regions / planner_engagement on the GUI
    // worker path, so the heatmap overlay and DerivedRestRegions boundaries
    // never saw the rest data core attached. `findings` rides alongside for
    // the same reason — a generation-time finding dropped here would never
    // reach the GUI's diagnostics list, which is the whole point of it.
    Ok((result, findings))
}

/// Convert viz `SimulationRequest` into a core `SimulationRequest` so the
/// actual simulation can be delegated to `rs_cam_core::compute::simulate`.
fn build_core_simulation_request(
    req: &SimulationRequest,
) -> rs_cam_core::compute::simulate::SimulationRequest {
    use rs_cam_core::compute::simulate::{SimGroupEntry, SimToolpathEntry};

    let groups = req
        .groups
        .iter()
        .map(|group| {
            // F-024 follow-up (2026-05-25): mirror the
            // `session::compute::compute_simulation_groups` decision shape so
            // the viz worker's production simulation path matches the core
            // `ProjectSession::run_simulation` path. For identity setups
            // (`face_up=Top`, `z_rotation=Deg0`) the viz controller leaves
            // `local_to_global = None` and the toolpath emits cut moves in
            // world frame (Z=[0, -depth], because `HeightsConfig::resolve`
            // auto-defaults `top_z = 0.0`). The per-setup dexel grid must
            // therefore also be in world frame. Forwarding the local
            // zero-rooted `local_stock_bbox` (Z=[0, stock_z]) here placed the
            // cutter at world Z=-2 below every dexel ray and inflated
            // `axial_engagement_mm` to the full stock height — which fed the
            // deflection gate 374-573 µm tip-deflection readings on AS001-
            // shape pockets that should land well under 50 µm.
            //
            // Fix: when `local_to_global` is `None` (identity setup), pass
            // `local_stock_bbox = None` too. `run_simulation` then falls back
            // to `request.stock_bbox` (world frame) for the per-setup grid,
            // matching the toolpath frame.
            //
            // Non-identity setups continue to forward the viz-side
            // zero-rooted `local_stock_bbox` paired with `local_to_global` —
            // that path is outside F-024's scope (see core finding).
            let (local_stock_bbox, local_to_global) =
                if let Some(info) = group.local_to_global.as_ref() {
                    (Some(group.local_stock_bbox), Some(info.clone()))
                } else {
                    (None, None)
                };

            SimGroupEntry {
                toolpaths: group
                    .toolpaths
                    .iter()
                    .map(|tp| {
                        let cutter = build_cutter(&tp.tool);
                        SimToolpathEntry {
                            id: tp.id,
                            name: tp.name.clone(),
                            annotated: Arc::clone(&tp.annotated),
                            tool: cutter,
                            flute_count: tp.tool.flute_count,
                            tool_summary: tp.tool.summary(),
                            semantic_trace: tp.semantic_trace.clone(),
                            spindle_rpm: tp.spindle_rpm,
                            metrics_not_applicable: tp.metrics_not_applicable,
                            drill_op: tp.drill_op.clone(),
                            operation_config_hash: tp.operation_config_hash,
                        }
                    })
                    .collect(),
                direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                local_stock_bbox,
                local_to_global,
                // F.4 — forward the phantom-prior-stock candidate computed
                // by the controller (`build_simulation_groups`) verbatim;
                // the core simulator inserts the phantom snapshot at the
                // recorded group position.
                phantom_prior_stock: group.phantom_prior_stock,
            }
        })
        .collect();

    rs_cam_core::compute::simulate::SimulationRequest {
        groups,
        stock_bbox: req.stock_bbox,
        stock_top_z: req.stock_top_z,
        resolution: req.resolution,
        metric_options: req.metric_options,
        spindle_rpm: req.spindle_rpm,
        rapid_feed_mm_min: req.rapid_feed_mm_min,
        model_mesh: req.model_mesh.clone(),
        // F-035 — the viz `SimulationRequest` now carries the active
        // `MachineProfile.kinematics` + `max_feed_mm_min` + the
        // `use_predicted_feed_in_gates` flag. When kinematics is
        // `Some`, build the core `KinematicsContext` so the
        // simulator's F-034 cycle-time override and F-035 predicted-
        // feed plumbing fire in the GUI sim path. When kinematics is
        // `None` (the default for every shipped preset), this stays
        // byte-identical to pre-F-034 / pre-F-035.
        kinematics: req
            .kinematics
            .map(|kin| rs_cam_core::compute::simulate::KinematicsContext {
                kinematics: kin,
                max_feed_mm_min: req.max_feed_mm_min.max(1.0),
                use_predicted_feed_in_gates: req.use_predicted_feed_in_gates,
            }),
    }
}

/// Build viz playback data from the viz request (global-frame toolpaths + tool
/// config + cut direction + optional global-frame drill_op). Core does not
/// produce this because it is a viz-only concern (incremental playback in the
/// 3D viewport). The `drill_op`, when present, lets `update_live_sim` apply
/// analytical removal during forward scrub rather than relying on
/// `simulate_toolpath_range`'s degenerate-Z dexel stamping for plunge moves.
fn build_playback_data(req: &SimulationRequest) -> Vec<super::PlaybackToolpath> {
    use super::PlaybackToolpath;
    use rs_cam_core::compute::simulate::{group_drill_op_to_global, group_toolpath_to_global};

    use rs_cam_core::dexel_stock::StockCutDirection;

    // The zero-rooted stock-relative frame the global playback stock lives in.
    let global_bbox = rs_cam_core::geo::BoundingBox3 {
        min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
        max: rs_cam_core::geo::P3::new(
            req.stock_bbox.max.x - req.stock_bbox.min.x,
            req.stock_bbox.max.y - req.stock_bbox.min.y,
            req.stock_bbox.max.z - req.stock_bbox.min.z,
        ),
    };

    let mut playback = Vec::new();
    for (group_ordinal, group) in req.groups.iter().enumerate() {
        let playback_direction = group
            .local_to_global
            .as_ref()
            .map_or(StockCutDirection::FromTop, |info| info.cut_direction());
        // G-LATERALSCRUB. A group whose tool axis is a global X or Y dexel
        // axis is replayed in its OWN frame instead: the global stock turns
        // only its Z grid into a closed solid, so a lateral stamp there
        // removes nothing an operator can see. The core simulator makes the
        // matching call — same predicate, same consequence — and publishes
        // that group's checkpoints with the local stock.
        let lateral = !matches!(
            playback_direction,
            StockCutDirection::FromTop | StockCutDirection::FromBottom
        );

        for tp in &group.toolpaths {
            let entry = if lateral {
                PlaybackToolpath {
                    // Setup-local coordinates verbatim — which is what the
                    // group's toolpaths already are — stamped down local Z,
                    // exactly as `compute/simulate.rs` stamps `group_stock`.
                    toolpath: Arc::new(tp.annotated.toolpath.clone()),
                    tool: tp.tool.clone(),
                    direction: StockCutDirection::FromTop,
                    drill_op: tp.drill_op.clone(),
                    group: group_ordinal,
                    frame: group.local_to_global.clone(),
                    stock_bbox: group.local_stock_bbox,
                }
            } else {
                // Frame-map through the SAME core helpers the global stock is
                // stamped with, so live playback and the checkpoint stocks
                // can't drift apart. Identity groups emit in world frame and
                // are shifted by `-stock_bbox.min`; carrying a private copy
                // of this mapping is how G-SIM-IDENTITY-FRAME got a second
                // home.
                PlaybackToolpath {
                    toolpath: Arc::new(group_toolpath_to_global(
                        &tp.annotated.toolpath,
                        &group.local_to_global,
                        req.stock_bbox.min,
                    )),
                    tool: tp.tool.clone(),
                    direction: playback_direction,
                    drill_op: tp.drill_op.as_ref().map(|drill_op_arc| {
                        Arc::new(group_drill_op_to_global(
                            drill_op_arc,
                            &group.local_to_global,
                            req.stock_bbox.min,
                        ))
                    }),
                    group: group_ordinal,
                    frame: None,
                    stock_bbox: global_bbox,
                }
            };
            playback.push(entry);
        }
    }
    playback
}

pub(crate) fn run_simulation_with_phase<F>(
    req: &SimulationRequest,
    cancel: &AtomicBool,
    set_phase: F,
    memo: Option<rs_cam_core::compute::sim_prefix::SimMemo<'_>>,
) -> Result<SimulationResult, ComputeError>
where
    F: FnMut(&str),
{
    use rs_cam_core::compute::simulate;

    // Convert viz request to core request and delegate. `memo` carries the
    // analysis lane's S5 prefix cache; `None` is the pre-S5 behaviour.
    let core_req = build_core_simulation_request(req);
    let core_result = simulate::run_simulation_memoized(&core_req, cancel, set_phase, memo)
        .map_err(|_cancelled| ComputeError::Cancelled)?;

    // Build viz-only playback data (global-frame toolpaths for viewport replay).
    let playback_data = build_playback_data(req);

    // Convert core boundaries (usize ids) to viz boundaries (ToolpathId).
    let boundaries: Vec<SimBoundary> = core_result
        .boundaries
        .into_iter()
        .map(|b| SimBoundary {
            id: b.id,
            name: b.name,
            tool_name: b.tool_name,
            start_move: b.start_move,
            end_move: b.end_move,
            direction: b.direction,
        })
        .collect();

    // Core and viz now share the same SimCheckpointMesh type (re-exported).
    let checkpoints = core_result.checkpoints;

    // Write cut-trace artifact to disk (viz-only filesystem concern).
    let (cut_trace, cut_trace_path) = if let Some(trace) = core_result.cut_trace {
        let artifact = build_simulation_cut_artifact(req, (*trace).clone());
        let path = match rs_cam_core::simulation_cut::write_simulation_cut_artifact(
            &simulation_metric_artifact_dir(),
            "simulation_metrics",
            &artifact,
        ) {
            Ok(p) => {
                // G-SIMDUMP: unbounded dumps filled the disk (96 GB observed);
                // keep only the newest few — each can be multiple GB.
                let pruned = rs_cam_core::simulation_cut::prune_simulation_cut_artifacts(
                    &simulation_metric_artifact_dir(),
                    SIM_CUT_ARTIFACT_RETAIN,
                );
                if pruned > 0 {
                    tracing::info!("Pruned {pruned} old simulation cut artifact(s)");
                }
                Some(p)
            }
            Err(error) => {
                tracing::warn!("Failed to write simulation cut artifact: {error}");
                None
            }
        };
        (Some(trace), path)
    } else {
        (None, None)
    };

    Ok(SimulationResult {
        mesh: core_result.mesh,
        total_moves: core_result.total_moves,
        deviations: core_result.deviations,
        column_deviations: core_result.column_deviations,
        boundaries,
        checkpoints,
        playback_data,
        rapid_collisions: core_result.rapid_collisions,
        rapid_collision_move_indices: core_result.rapid_collision_move_indices,
        cut_trace,
        cut_trace_path,
        resolution_clamped: core_result.resolution_clamped,
        column_grid_cell_mm: core_result.column_grid_cell_mm,
        prior_stocks: core_result.prior_stocks,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn run_compute(req: &ComputeRequest, cancel: &AtomicBool) -> ComputeExecutionOutcome {
    let debug_recorder = req.debug_options.enabled.then(|| {
        rs_cam_core::debug_trace::ToolpathDebugRecorder::new(
            req.toolpath_name.clone(),
            req.operation.label(),
        )
    });
    let semantic_recorder = req.debug_options.enabled.then(|| {
        rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new(
            req.toolpath_name.clone(),
            req.operation.label(),
        )
    });
    run_compute_with_phase_tracker(req, cancel, None, debug_recorder, semantic_recorder)
}

pub(super) fn run_compute_with_phase(
    req: &ComputeRequest,
    cancel: &AtomicBool,
    phase_tracker: &ToolpathPhaseTracker,
) -> ComputeExecutionOutcome {
    let debug_recorder = req.debug_options.enabled.then(|| {
        rs_cam_core::debug_trace::ToolpathDebugRecorder::new(
            req.toolpath_name.clone(),
            req.operation.label(),
        )
        .with_phase_sink(Arc::new(phase_tracker.clone()))
    });
    let semantic_recorder = req.debug_options.enabled.then(|| {
        rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new(
            req.toolpath_name.clone(),
            req.operation.label(),
        )
    });
    run_compute_with_phase_tracker(
        req,
        cancel,
        Some(phase_tracker),
        debug_recorder,
        semantic_recorder,
    )
}

/// `pub(super)` so the worker's tests can drive it with a semantic recorder
/// while `debug_options.enabled` is FALSE — the configuration the product
/// actually ships in, and the one C1 item 4b makes exercisable.
pub(super) fn run_compute_with_phase_tracker(
    req: &ComputeRequest,
    cancel: &AtomicBool,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug_recorder: Option<rs_cam_core::debug_trace::ToolpathDebugRecorder>,
    semantic_recorder: Option<rs_cam_core::semantic_trace::ToolpathSemanticRecorder>,
) -> ComputeExecutionOutcome {
    let debug_root = debug_recorder
        .as_ref()
        .map(|recorder| recorder.root_context());
    let semantic_root = semantic_recorder
        .as_ref()
        .map(|recorder| recorder.root_context());
    let result = (|| -> Result<ToolpathResult, ComputeError> {
        // The findings ride out of the generate block with the toolpath they
        // describe. They used to be a `let mut` default declared outside this
        // closure and assigned into — a shape that only made sense while the
        // stats mapping READ them field by field. The join MOVES them, so
        // there is no default to fabricate and no window in which a caller
        // could observe an empty one.
        // Checkpoint C (Q2): the boundary clip below can add a finding of
        // its own, so these stay mutable until the join rather than being
        // moved straight into it.
        let (tp, mut generation_findings) = {
            let _phase_scope =
                phase_tracker.map(|tracker| tracker.start_phase(req.operation.label()));
            let core_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("core_generate", req.operation.label()));
            let core_ctx = core_scope.as_ref().map(|scope| scope.context());
            let core_debug_span_id = core_scope.as_ref().map(|scope| scope.id());
            let (generated, core_findings) = generate_via_core(
                req,
                cancel,
                core_ctx.as_ref(),
                semantic_root.as_ref(),
                core_debug_span_id,
            )?;
            if let Some(scope) = core_scope.as_ref()
                && !generated.toolpath.moves.is_empty()
            {
                scope.set_move_range(0, generated.toolpath.moves.len() - 1);
            }
            (generated, core_findings)
        };

        let mut current = tp;

        // C1 item 4b: built UNCONDITIONALLY, not under `debug_options.enabled`.
        // The recorder is still debug-gated (recording every item on every
        // generation is not free), but the reconcile path is not: every
        // transform below hands back a `Transformed` and every one of them is
        // reconciled through this set, in the default configuration as much as
        // in the debug one. Pre-C1 the remap calls were themselves inside
        // `if let Some(recorder)`, so the shipping product never ran them.
        let mut channels =
            rs_cam_core::transform_provenance::ReconcileSet::new(semantic_recorder.as_ref(), None);

        {
            let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Apply dressups"));
            let dressup_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("dressups", "Apply dressups"));
            let dressup_ctx = dressup_scope.as_ref().map(|scope| scope.context());
            current = apply_dressups(
                current,
                req,
                dressup_ctx.as_ref(),
                semantic_root.as_ref(),
                &mut channels,
            );
        }

        if req.boundary.enabled
            && let Some(bbox) = &req.stock_bbox
        {
            let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Clip to boundary"));
            let boundary_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("boundary_clip", "Clip to boundary"));
            let boundary_span_id = boundary_scope.as_ref().map(|scope| scope.id());
            use rs_cam_core::boundary::{
                ToolContainment, clip_annotated_to_boundary_set, effective_boundary_reported,
                subtract_keepouts,
            };
            use rs_cam_core::compute::config::BoundarySource;
            // Resolve the source polygon. Order:
            // 1. FaceSelection (when configured + enriched mesh available)
            // 2. ModelSilhouette (when configured + mesh available) — was missing,
            //    causing model-silhouette boundaries to silently fall through to
            //    the stock-bbox rectangle (= no effective clipping). Mirrors
            //    session/compute.rs::apply_boundary_clip.
            // 3. Stock-bbox rectangle as fallback for stock-source or when the
            //    requested source's geometry isn't available.
            let stock_rect = || {
                rs_cam_core::polygon::Polygon2::rectangle(
                    bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y,
                )
            };
            if matches!(
                req.boundary.source,
                BoundarySource::DerivedRestRegions { .. }
            ) {
                // P2.2/P2.3: this is the real enforcement clip for the GUI
                // worker path — mirrors `ProjectSession::generate_toolpath`'s
                // post-dressup clip byte-for-byte by calling the same core
                // function (rather than re-deriving a single-polygon
                // approximation, which silently degraded multi-region rest
                // analysis down to "clip against nothing" whenever the
                // regions didn't union to exactly one polygon). The
                // controller fail-hard-validates this boundary before
                // submitting (see `submit_toolpath_compute`), so
                // `req.derived_rest_regions` is `Some` with a non-empty
                // `Vec` in production; the stock-rectangle fallback below
                // only matters for hand-built test requests that bypass
                // the controller.
                let regions: Vec<rs_cam_core::polygon::Polygon2> = req
                    .derived_rest_regions
                    .clone()
                    .filter(|regions| !regions.is_empty())
                    .unwrap_or_else(|| vec![stock_rect()]);
                let semantic_ctx = semantic_root.clone().unwrap_or_else(|| {
                    rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new(
                        req.toolpath_name.clone(),
                        req.operation.label(),
                    )
                    .root_context()
                });
                current = rs_cam_core::session::ProjectSession::apply_boundary_clip_multi(
                    current,
                    &req.boundary,
                    &regions,
                    &req.keep_out_footprints,
                    req.tool.envelope_diameter(),
                    effective_safe_z(req),
                    &semantic_ctx,
                    &mut channels,
                    &mut generation_findings,
                )?;
                if let Some(scope) = boundary_scope.as_ref()
                    && !current.toolpath.moves.is_empty()
                {
                    scope.set_move_range(0, current.toolpath.moves.len() - 1);
                }
            } else {
                // Resolve the source polygon. Order:
                // 1. FaceSelection (when configured + enriched mesh available)
                // 2. ModelSilhouette (when configured + mesh available) — was missing,
                //    causing model-silhouette boundaries to silently fall through to
                //    the stock-bbox rectangle (= no effective clipping). Mirrors
                //    session/compute.rs::apply_boundary_clip.
                // 3. Stock-bbox rectangle as fallback for stock-source or when the
                //    requested source's geometry isn't available.
                let mut stock_poly = if let (Some(face_ids), Some(enriched)) =
                    (&req.face_selection, &req.enriched_mesh)
                {
                    enriched
                        .faces_boundary_as_polygon(face_ids)
                        .unwrap_or_else(stock_rect)
                } else if matches!(req.boundary.source, BoundarySource::ModelSilhouette)
                    && let Some(mesh) = req.mesh.as_ref()
                {
                    // G8: memoised per mesh identity. This is the second of
                    // the two silhouette rasterisations a single toolpath ran
                    // (pre-clip above, enforcement clip here) — both now share
                    // one build with every other toolpath over the same mesh.
                    rs_cam_core::geom_cache::cached_silhouette(mesh)
                        .iter()
                        .max_by(|a, b| {
                            a.area()
                                .partial_cmp(&b.area())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .cloned()
                        .unwrap_or_else(stock_rect)
                } else {
                    stock_rect()
                };
                if !req.keep_out_footprints.is_empty() {
                    stock_poly = subtract_keepouts(&stock_poly, &req.keep_out_footprints);
                }
                // Apply user-configured boundary offset (positive = expand outward,
                // negative = shrink). Mirrors session/compute.rs apply_boundary_clip.
                // cavalier_contours convention: positive distance is INWARD shrink,
                // so flip the sign to match the user-facing convention.
                // Checkpoint C, D-3b (F-8): same three-way decision as the
                // pre-boundary site above and the session's
                // `resolve_containment_polygon`. A collapsed user offset
                // drops the containment (which then takes the ruled
                // collapsed-containment path below); a FAILED one refuses,
                // because continuing on the un-offset boundary clips to a
                // larger region than was asked for.
                let mut containment_collapsed = false;
                if req.boundary.offset.abs() > 1e-9 {
                    use rs_cam_core::boundary::{UserOffsetOutcome, apply_user_boundary_offset};
                    match apply_user_boundary_offset(&stock_poly, req.boundary.offset) {
                        UserOffsetOutcome::Resolved(p) => stock_poly = p,
                        UserOffsetOutcome::Collapsed => containment_collapsed = true,
                        UserOffsetOutcome::Failed(failure) => {
                            return Err(rs_cam_core::compute::OperationError::MissingGeometry(
                                format!(
                                    "the machining boundary's {:+.3} mm offset could \
                                     not be computed: {}. Refusing rather than \
                                     continuing with the UN-OFFSET boundary.",
                                    req.boundary.offset,
                                    failure.describe(),
                                ),
                            )
                            .into());
                        }
                    }
                }
                let containment = match req.boundary.containment {
                    crate::state::toolpath::BoundaryContainment::Center => ToolContainment::Center,
                    crate::state::toolpath::BoundaryContainment::Inside => ToolContainment::Inside,
                    crate::state::toolpath::BoundaryContainment::Outside => {
                        ToolContainment::Outside
                    }
                };
                let tool_diameter = req.tool.envelope_diameter();
                let (boundaries, offset_failure) = if containment_collapsed {
                    (Vec::new(), None)
                } else {
                    effective_boundary_reported(&stock_poly, containment, tool_diameter / 2.0)
                };
                // Checkpoint C, Q2 (F-1): this is the LIVE GUI worker's copy
                // of the boundary-clip escape. An empty `boundaries` used to
                // fall out of `if let Some(boundary) = boundaries.first()`
                // with an implicit `else { do not clip }` — the containment
                // silently not applied. Same ruling as the session path:
                // pass through with a typed finding on a genuine collapse,
                // refuse when the offset failed.
                if boundaries.is_empty() {
                    rs_cam_core::session::ProjectSession::resolve_collapsed_containment(
                        offset_failure,
                        req.boundary.containment,
                        tool_diameter,
                        1,
                        &mut generation_findings,
                    )?;
                } else {
                    // C1: one implementation of "clip, then bring every
                    // index-carrying channel along" — spans included. This
                    // used to be three hand-rolled lines here, and the same
                    // three in each session clip.
                    //
                    // Checkpoint C, D-3c: the WHOLE set, not
                    // `boundaries.first()`. A containment that splits under
                    // the offset used to keep piece 1 and clip everything
                    // outside it away; the multi-region path already kept
                    // them all, and that is the semantics that won.
                    current =
                        clip_annotated_to_boundary_set(current, &boundaries, effective_safe_z(req))
                            .reconcile(&mut channels)
                            .into_inner();
                    if let Some(root) = semantic_root.as_ref() {
                        let scope =
                            root.start_item(ToolpathSemanticKind::BoundaryClip, "Boundary clip");
                        if let Some(span_id) = boundary_span_id {
                            scope.set_debug_span_id(span_id);
                        }
                        scope.set_param(
                            SemanticKey::Containment,
                            match req.boundary.containment {
                                crate::state::toolpath::BoundaryContainment::Center => "center",
                                crate::state::toolpath::BoundaryContainment::Inside => "inside",
                                crate::state::toolpath::BoundaryContainment::Outside => "outside",
                            },
                        );
                        scope.set_param(SemanticKey::KeepOutCount, req.keep_out_footprints.len());
                        if !current.toolpath.moves.is_empty() {
                            scope.bind_to_toolpath(
                                &current.toolpath,
                                0,
                                current.toolpath.moves.len(),
                            );
                        }
                    }
                    if let Some(scope) = boundary_scope.as_ref()
                        && !current.toolpath.moves.is_empty()
                    {
                        scope.set_move_range(0, current.toolpath.moves.len() - 1);
                    }
                }
            }
        }

        // ── Entry-descent optimization ────────────────────────────────
        // P1 W2 (reworked): split long safe_z-to-cut-depth plunges by
        // rapiding down to just above the INPUT STOCK's material ceiling
        // first — using the actual stock (`req.prior_stock`, the same
        // snapshot generation was seeded with for FromRemainingStock ops)
        // or the fresh-stock top otherwise, never a mesh height. Mirrors
        // session/compute.rs::generate_toolpath's post-clip wiring. Runs
        // on every generation (not just finish passes): the stock-derived
        // ceiling is safe by construction, unlike the mesh-derived height
        // it replaces (see `optimize_entry_descents`'s doc for the
        // 151-collision Rivers lesson that motivated the rework).
        //
        // Inserts moves after span construction, so spans are remapped
        // through the same provenance-map contract the boundary clip uses
        // above, rather than invalidated.
        {
            let (transformed, _split_count) =
                rs_cam_core::dressup::optimize_entry_descents_annotated(
                    current,
                    req.prior_stock.as_ref(),
                    req.heights.top_z,
                    req.tool.envelope_diameter() / 2.0,
                );
            current = transformed.reconcile(&mut channels).into_inner();
        }

        // ── G-ENTRYEMPTY: the empty-generation gate ───────────────────
        // The live GUI worker's copy of the same gate
        // `ProjectSession::generate_toolpath` applies, calling the same
        // single-owner classifier in core so the two paths cannot drift.
        // `current` is the finished toolpath — everything downstream of
        // here only measures it.
        //
        // An `Err` from this closure reaches `drain_compute_results`'s
        // `ComputeError::Message` arm, which sets the toolpath to `Error`
        // and clears BOTH result caches — so, unlike the pre-fix
        // `Ok`-with-0-moves, an empty generation leaves nothing behind for
        // a later generation, simulation or export to consume.
        match rs_cam_core::compute::generated_empty::classify(
            &current.toolpath,
            &rs_cam_core::compute::generated_empty::EmptyGenerationInputs {
                toolpath_name: &req.toolpath_name,
                operation: &req.operation,
                stock_source: req.stock_source,
                seeded_machined_stock: req.prior_stock.is_some(),
                has_mesh: req.mesh.is_some(),
                polygon_count: req.polygons.as_deref().map_or(0, Vec::len),
                boundary_is_derived_rest_regions: matches!(
                    req.boundary.source,
                    rs_cam_core::compute::config::BoundarySource::DerivedRestRegions { .. }
                ),
            },
        ) {
            rs_cam_core::compute::generated_empty::EmptyGenerationVerdict::NotEmpty => {}
            rs_cam_core::compute::generated_empty::EmptyGenerationVerdict::Legitimate(reason) => {
                tracing::info!(
                    toolpath = %req.toolpath_name,
                    operation = %req.operation.label(),
                    reason = reason.describe(),
                    "Generated an empty toolpath, and that is expected here"
                );
            }
            rs_cam_core::compute::generated_empty::EmptyGenerationVerdict::Refuse(refusal) => {
                return Err(ComputeError::Message(refusal.to_string()));
            }
        }

        let stats = {
            let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Compute stats"));
            let _stats_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("final_stats", "Compute stats"));
            // H2.1: the worker no longer has a per-field surface here.
            //
            // This block used to be `compute_stats_with_spans(..)` followed
            // by eleven `stats.<field> = generation_findings.<field>;`
            // assignments — a mapping with no compile-time guard in either
            // direction, which is why it had to be patched once per finding
            // channel added (the comment on the last one said so: "this is
            // the fifth field to need these lines"), and why
            // `generate_via_core` was once able to drop every annotated
            // side-channel at once without a single error.
            //
            // `stats_with_findings` is the ONE join, shared with
            // `ProjectSession::generate_toolpath`. It destructures
            // `GenerationFindings` exhaustively in core, so a new finding is
            // a compile error there until someone routes it — and the GUI
            // path inherits the routing rather than re-stating it.
            // S-4 (G-BYTE): stamp the machined-stock snapshot this
            // generation consumed, mirroring
            // `session/compute.rs::generate_toolpath`. `req.prior_stock` is
            // this path's equivalent of the session's `prior_stock_arc` —
            // the same snapshot the entry-descent split above was handed.
            rs_cam_core::compute::stats_with_findings(
                &current.toolpath,
                current.spans_valid.then_some(current.spans.as_slice()),
                generation_findings,
                req.prior_stock
                    .as_ref()
                    .map(rs_cam_core::compute::config::StockSnapshotStamp::of),
            )
        };

        // §6.E build the drill-op view atomically with the annotated
        // toolpath. Mirrors session/compute.rs production path so the
        // GUI worker carries the same dual-representation invariant.
        let local_tool_def = build_cutter(&req.tool);
        let default_bbox = rs_cam_core::geo::BoundingBox3::empty();
        let local_stock_bbox = req.stock_bbox.as_ref().unwrap_or(&default_bbox);
        let drill_op = rs_cam_core::compute::execute::build_drill_op_for_config(
            &req.operation,
            req.polygons.as_deref().map(|v| v.as_slice()),
            &local_tool_def,
            &req.tool,
            local_stock_bbox,
            req.material.clone(),
            // Same transform the generate call above uses, and it has to be:
            // the drill-op view must name the holes THIS path's toolpath
            // actually drills (§6.E dual-rep).
            req.setup_transform.as_ref(),
        )
        .map(Arc::new);
        Ok(ToolpathResult {
            annotated: Arc::new(current),
            stats,
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
            drill_op,
        })
    })();

    let (debug_trace, semantic_trace, debug_trace_path) = if let (
        Some(debug_recorder),
        Some(semantic_recorder),
    ) = (debug_recorder, semantic_recorder)
    {
        let mut debug_trace = debug_recorder.finish();
        let mut semantic_trace = semantic_recorder.finish();
        rs_cam_core::semantic_trace::enrich_traces(&mut debug_trace, &mut semantic_trace);
        let artifact =
            build_trace_artifact(req, Some(debug_trace.clone()), Some(semantic_trace.clone()));
        let file_stem = format!("{}-{}", req.toolpath_id.0, req.toolpath_name);
        let path = match rs_cam_core::semantic_trace::write_toolpath_trace_artifact(
            &debug_artifact_dir(),
            &file_stem,
            &artifact,
        ) {
            Ok(path) => Some(path),
            Err(error) => {
                tracing::warn!(
                    "Failed to write toolpath debug artifact for {}: {error}",
                    req.toolpath_id.0
                );
                None
            }
        };
        (
            Some(Arc::new(debug_trace)),
            Some(Arc::new(semantic_trace)),
            path,
        )
    } else {
        (None, None, None)
    };

    let result = match result {
        Ok(mut computed) => {
            computed.debug_trace = debug_trace.clone();
            computed.semantic_trace = semantic_trace.clone();
            computed.debug_trace_path = debug_trace_path.clone();
            Ok(computed)
        }
        Err(error) => Err(error),
    };

    ComputeExecutionOutcome {
        result,
        debug_trace,
        semantic_trace,
        debug_trace_path,
    }
}

// Annotation helpers (annotate_operation_scope, annotate_adaptive3d_runtime_semantics,
// etc.) were removed along with their only callers — the SemanticToolpathOp impls.
// Dispatch now goes through rs_cam_core::compute::execute::execute_operation.

// Tests dispatch directly through `generate_via_core` instead of bespoke run_* helpers.

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::compute::worker::Toolpath;

    fn test_request_with_polygon(
        operation: OperationConfig,
        tool_type: ToolType,
    ) -> ComputeRequest {
        let tool = ToolConfig::new_default(crate::state::job::ToolId(1), tool_type);
        let heights = HeightsConfig::default().resolve(&HeightContext::simple(10.0, 6.0));
        let cutting_levels = operation.cutting_levels(heights.top_z);
        ComputeRequest {
            // Identity-setup fixture: no local<->global transform to apply.
            setup_transform: None,
            toolpath_id: ToolpathId(1),
            toolpath_index: 0,
            toolpath_name: "Test".to_owned(),
            polygons: Some(Arc::new(vec![Polygon2::rectangle(
                -20.0, -20.0, 20.0, 20.0,
            )])),
            mesh: None,
            enriched_mesh: None,
            face_selection: None,
            operation,
            dressups: DressupConfig::default(),
            stock_source: StockSource::Fresh,
            tool,
            safe_z: 10.0,
            prev_tool_radius: None,
            reference_tool_cfg: None,
            stock_bbox: Some(BoundingBox3 {
                min: P3::new(-25.0, -25.0, -10.0),
                max: P3::new(25.0, 25.0, 10.0),
            }),
            boundary: crate::state::toolpath::BoundaryConfig::default(),
            keep_out_footprints: Vec::new(),
            heights,
            cutting_levels,
            debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
            prior_stock: None,
            material: rs_cam_core::material::Material::default(),
            derived_rest_regions: None,
            rest_analysis: Default::default(),
            link_kinematics: None,
        }
    }

    fn dominant_cut_axis(tp: &Toolpath, feed_rate: f64) -> Option<char> {
        let mut x_travel = 0.0;
        let mut y_travel = 0.0;

        for idx in 1..tp.moves.len() {
            if let MoveType::Linear {
                feed_rate: move_feed,
            } = tp.moves[idx].move_type
                && (move_feed - feed_rate).abs() < 1e-6
            {
                let dx = tp.moves[idx].target.x - tp.moves[idx - 1].target.x;
                let dy = tp.moves[idx].target.y - tp.moves[idx - 1].target.y;
                if dx.abs().max(dy.abs()) > 1e-3 {
                    x_travel += dx.abs();
                    y_travel += dy.abs();
                }
            }
        }

        if x_travel <= 1e-6 && y_travel <= 1e-6 {
            None
        } else if y_travel > x_travel {
            Some('y')
        } else {
            Some('x')
        }
    }

    // --- Task A3: Tabs only on final depth pass ---

    #[test]
    fn profile_multi_pass_tabs_only_on_final_depth() {
        let cfg = ProfileConfig {
            depth: 6.0,
            depth_per_pass: 2.0,
            tab_count: 4,
            tab_width: 6.0,
            tab_height: 2.0,
            finishing_passes: 0,
            ..ProfileConfig::default()
        };
        let req =
            test_request_with_polygon(OperationConfig::Profile(cfg.clone()), ToolType::EndMill);
        let cancel = AtomicBool::new(false);
        let tp = generate_via_core(&req, &cancel, None, None, None)
            .unwrap()
            .0
            .toolpath;

        let final_z = -cfg.depth;
        let tab_z = final_z + cfg.tab_height;

        // Tab height moves should exist (tabs applied to final pass)
        let tab_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| (m.target.z - tab_z).abs() < 0.01)
            .collect();
        assert!(
            !tab_moves.is_empty(),
            "Final pass should have tab height moves at z={tab_z}"
        );

        // Intermediate passes (at Z != final_z) should have NO tab-height lifts.
        // Roughing levels are at -2, -4; final is at -6. Tab height is -4.
        // Check that the Z=-4 moves are actual cutting moves, not tab lifts:
        // Tab lifts have z = final_z + tab_height = -6 + 2 = -4 which
        // coincides with a roughing level. Instead, verify that there are
        // no tab-height moves between roughing passes (moves with z > final_z
        // that aren't at a legitimate roughing level or safe_z).
        let depth_stepping = rs_cam_core::depth::DepthStepping {
            start_z: 0.0,
            final_z: -cfg.depth.abs(),
            max_step_down: cfg.depth_per_pass,
            distribution: rs_cam_core::depth::DepthDistribution::Even,
            finish_allowance: 0.0,
            finishing_passes: cfg.finishing_passes,
        };
        let roughing_levels = depth_stepping.all_levels();
        for m in &tp.moves {
            if let MoveType::Linear { .. } = m.move_type {
                let z = m.target.z;
                // Any linear move should be at a legitimate level, tab_z, or
                // a plunge between levels.
                let at_known_level = roughing_levels.iter().any(|&lv| (z - lv).abs() < 0.01);
                let at_tab_z = (z - tab_z).abs() < 0.01;
                let is_plunge_or_retract = z > final_z + 0.01 && z < effective_safe_z(&req) - 0.01;
                assert!(
                    at_known_level || at_tab_z || is_plunge_or_retract,
                    "unexpected linear move z={z}, expected one of levels {roughing_levels:?}, tab_z={tab_z}, or plunge"
                );
            }
        }
    }

    // --- Task A4: Face OneWay produces unidirectional cuts ---

    #[test]
    fn face_oneway_produces_nonempty_toolpath() {
        // OneWay face direction correctness is validated by core's own tests
        // (face::tests::face_oneway_all_passes_same_x_direction).
        // This test verifies that the viz -> core dispatch produces a non-empty result.
        let cfg = FaceConfig {
            direction: FaceDirection::OneWay,
            stepover: 10.0,
            ..FaceConfig::default()
        };
        let req = test_request_with_polygon(OperationConfig::Face(cfg), ToolType::EndMill);

        let cancel = AtomicBool::new(false);
        let tp = generate_via_core(&req, &cancel, None, None, None)
            .unwrap()
            .0
            .toolpath;

        // Verify the operation produces cutting moves
        let cutting_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .collect();
        assert!(
            !cutting_moves.is_empty(),
            "Face OneWay operation should produce cutting moves"
        );
    }

    #[test]
    fn face_zigzag_alternates_direction() {
        let cfg = FaceConfig {
            direction: FaceDirection::Zigzag,
            stepover: 10.0,
            ..FaceConfig::default()
        };
        let req = test_request_with_polygon(OperationConfig::Face(cfg.clone()), ToolType::EndMill);

        let cancel = AtomicBool::new(false);
        let tp = generate_via_core(&req, &cancel, None, None, None)
            .unwrap()
            .0
            .toolpath;

        let mut cut_directions = Vec::new();
        for i in 1..tp.moves.len() {
            if let MoveType::Linear { feed_rate } = tp.moves[i].move_type
                && (feed_rate - cfg.feed_rate).abs() < 1e-6
            {
                let dx = tp.moves[i].target.x - tp.moves[i - 1].target.x;
                if dx.abs() > 1.0 {
                    cut_directions.push(dx > 0.0);
                }
            }
        }

        assert!(
            cut_directions.len() >= 2,
            "Zigzag face should have multiple cutting rows"
        );
        // Should have at least one direction change
        let has_alternation = cut_directions.windows(2).any(|w| w[0] != w[1]);
        assert!(
            has_alternation,
            "Zigzag face should alternate cut directions"
        );
    }

    #[test]
    fn zigzag_angle_uses_degrees_for_runtime_path() {
        let cfg = ZigzagConfig {
            angle: 90.0,
            stepover: 6.0,
            depth: 2.0,
            depth_per_pass: 2.0,
            ..ZigzagConfig::default()
        };
        let req =
            test_request_with_polygon(OperationConfig::Zigzag(cfg.clone()), ToolType::EndMill);
        let cancel = AtomicBool::new(false);
        let tp = generate_via_core(&req, &cancel, None, None, None)
            .unwrap()
            .0
            .toolpath;

        assert_eq!(
            dominant_cut_axis(&tp, cfg.feed_rate),
            Some('y'),
            "90 degree zigzag should cut along Y, not a radian-converted skew"
        );
    }

    #[test]
    fn rest_semantic_angle_uses_degrees() {
        let cfg = RestConfig {
            angle: 90.0,
            stepover: 4.0,
            depth: 2.0,
            depth_per_pass: 2.0,
            ..RestConfig::default()
        };
        let mut req =
            test_request_with_polygon(OperationConfig::Rest(cfg.clone()), ToolType::EndMill);
        req.prev_tool_radius = Some(8.0);

        let cancel = AtomicBool::new(false);
        let tp = generate_via_core(&req, &cancel, None, None, None)
            .unwrap()
            .0
            .toolpath;

        assert_eq!(
            dominant_cut_axis(&tp, cfg.feed_rate),
            Some('y'),
            "semantic rest reconstruction should preserve the degree-based raster angle"
        );
    }

    // --- Task A10: Inlay female and male are separated ---

    #[test]
    fn inlay_output_contains_female_and_male_sections() {
        let cfg = InlayConfig::default();
        let mut req = test_request_with_polygon(OperationConfig::Inlay(cfg), ToolType::VBit);
        req.polygons = Some(Arc::new(vec![Polygon2::rectangle(
            -10.0, -10.0, 10.0, 10.0,
        )]));
        let cancel = AtomicBool::new(false);
        let tp = generate_via_core(&req, &cancel, None, None, None)
            .unwrap()
            .0
            .toolpath;

        assert!(!tp.moves.is_empty(), "Inlay should produce moves");

        // Find retract-to-safe-z moves that separate sections. The female
        // and male toolpaths should be separated by a retract to safe_z.
        let safe_z = effective_safe_z(&req);
        let retract_indices: Vec<usize> = tp
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| m.move_type == MoveType::Rapid && (m.target.z - safe_z).abs() < 0.01)
            .map(|(i, _)| i)
            .collect();

        // There should be multiple retract moves (at least the separator
        // between female and male plus retracts within each section)
        assert!(
            retract_indices.len() >= 2,
            "Should have retract separators between female and male sections"
        );

        // Find cutting moves below Z=0 (both female and male cut below the surface)
        let cutting_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }) && m.target.z < -0.01)
            .collect();
        assert!(
            !cutting_moves.is_empty(),
            "Should have cutting moves below stock surface"
        );
    }

    // --- Task C14: Scallop requires BallNose tool ---

    #[test]
    fn scallop_rejects_non_ballnose_tool() {
        let OperationConfig::Scallop(cfg) =
            OperationConfig::new_default(crate::state::toolpath::OperationType::Scallop)
        else {
            unreachable!();
        };
        let mut req = test_request_with_polygon(OperationConfig::Scallop(cfg), ToolType::EndMill);
        req.mesh = Some(Arc::new(rs_cam_core::mesh::make_test_flat(40.0)));
        let cancel = AtomicBool::new(false);
        let result = generate_via_core(&req, &cancel, None, None, None);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("Ball Nose"),
            "Error should mention Ball Nose requirement"
        );
    }
}
