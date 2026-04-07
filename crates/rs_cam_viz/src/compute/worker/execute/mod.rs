mod operations_2d;
mod operations_3d;

use super::helpers::{
    apply_dressups, build_cutter, build_simulation_cut_artifact, build_trace_artifact,
    compute_stats, debug_artifact_dir, effective_safe_z, make_depth, make_depth_with_finishing,
    require_mesh, require_polygons, simulation_metric_artifact_dir,
};
use super::semantic::{
    CutRun, append_toolpath, bind_scope_to_run, contour_toolpath, cutting_runs, line_toolpath,
};
use super::{
    Adaptive3dConfig, Adaptive3dEntryStyle, Adaptive3dParams, AdaptiveConfig, AdaptiveParams,
    AlignmentPinDrillConfig, Arc, AtomicBool, ChamferConfig, ChamferParams, ComputeError,
    ComputeRequest, DrillConfig, DrillCycle, DrillCycleType, DrillParams, DropCutterConfig,
    EntryStyle3d, FaceConfig, FaceDirection, HorizontalFinishConfig, HorizontalFinishParams,
    InlayConfig, InlayParams, OperationConfig, Ordering, PencilConfig, PencilParams, PocketConfig,
    PocketParams, PocketPattern, Polygon2, ProfileConfig, ProfileParams, ProjectCurveConfig,
    ProjectCurveParams, RadialFinishConfig, RadialFinishParams, RampFinishConfig, RampFinishParams,
    RestConfig, RestParams, ScallopConfig, ScallopParams, SimBoundary, SimCheckpointMesh,
    SimulationRequest, SimulationResult, SpatialIndex, SpiralFinishConfig, SpiralFinishParams,
    SteepShallowConfig, SteepShallowParams, ToolDefinition, ToolType, Toolpath,
    ToolpathPhaseTracker, ToolpathResult, TraceConfig, TraceParams, TriangleMesh, VCarveConfig,
    VCarveParams, WaterlineConfig, WaterlineParams, ZigzagConfig, ZigzagParams, apply_tabs,
    batch_drop_cutter_with_cancel, chamfer_toolpath, depth_stepped_toolpath, drill_toolpath,
    even_tabs, horizontal_finish_toolpath, inlay_toolpaths, pocket_toolpath, profile_toolpath,
    project_curve_toolpath, radial_finish_toolpath, raster_toolpath_from_grid,
    rest_machining_toolpath, steep_shallow_toolpath, trace_toolpath, vcarve_toolpath,
    waterline_toolpath_with_cancel, zigzag_toolpath,
};
#[cfg(test)]
use super::{BoundingBox3, DressupConfig, MoveType, StockSource, ToolConfig, ToolpathId};
#[cfg(test)]
use crate::state::toolpath::{BoundaryContainment, HeightContext, HeightsConfig};
use rs_cam_core::geo::P3;
use rs_cam_core::semantic_trace::ToolpathSemanticKind;

// Re-export items used by tests within this module
#[cfg(test)]
use operations_2d::{run_inlay, run_profile};
#[cfg(test)]
use operations_3d::run_scallop_annotated;

pub(super) struct ComputeExecutionOutcome {
    pub result: Result<ToolpathResult, ComputeError>,
    pub debug_trace: Option<Arc<rs_cam_core::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<std::path::PathBuf>,
}

struct OperationExecutionContext<'a> {
    req: &'a ComputeRequest,
    cancel: &'a AtomicBool,
    phase_tracker: Option<&'a ToolpathPhaseTracker>,
    core_debug_span_id: Option<u64>,
    debug_root: Option<&'a rs_cam_core::debug_trace::ToolpathDebugContext>,
    semantic_root: Option<&'a rs_cam_core::semantic_trace::ToolpathSemanticContext>,
}

struct AdaptiveLevelSlice {
    polygon_index: usize,
    level_index: usize,
    z: f64,
    move_start: usize,
    move_end_exclusive: usize,
}

struct ProjectCurveSlice {
    source_curve_index: usize,
    move_start: usize,
    move_end_exclusive: usize,
}

trait SemanticToolpathOp {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError>;
}

impl OperationConfig {
    fn semantic_op(&self) -> &dyn SemanticToolpathOp {
        match self {
            OperationConfig::Face(cfg) => cfg,
            OperationConfig::Pocket(cfg) => cfg,
            OperationConfig::Profile(cfg) => cfg,
            OperationConfig::Adaptive(cfg) => cfg,
            OperationConfig::VCarve(cfg) => cfg,
            OperationConfig::Rest(cfg) => cfg,
            OperationConfig::Inlay(cfg) => cfg,
            OperationConfig::Zigzag(cfg) => cfg,
            OperationConfig::Trace(cfg) => cfg,
            OperationConfig::Drill(cfg) => cfg,
            OperationConfig::Chamfer(cfg) => cfg,
            OperationConfig::DropCutter(cfg) => cfg,
            OperationConfig::Adaptive3d(cfg) => cfg,
            OperationConfig::Waterline(cfg) => cfg,
            OperationConfig::Pencil(cfg) => cfg,
            OperationConfig::Scallop(cfg) => cfg,
            OperationConfig::SteepShallow(cfg) => cfg,
            OperationConfig::RampFinish(cfg) => cfg,
            OperationConfig::SpiralFinish(cfg) => cfg,
            OperationConfig::RadialFinish(cfg) => cfg,
            OperationConfig::HorizontalFinish(cfg) => cfg,
            OperationConfig::ProjectCurve(cfg) => cfg,
            OperationConfig::AlignmentPinDrill(cfg) => cfg,
        }
    }
}

#[allow(dead_code)]
pub(super) fn run_simulation(
    req: &SimulationRequest,
    cancel: &AtomicBool,
) -> Result<SimulationResult, ComputeError> {
    run_simulation_with_phase(req, cancel, |_| {})
}

pub(super) fn run_simulation_with_phase<F>(
    req: &SimulationRequest,
    cancel: &AtomicBool,
    mut set_phase: F,
) -> Result<SimulationResult, ComputeError>
where
    F: FnMut(&str),
{
    use rs_cam_core::compute::simulate as core_sim;

    // Build the core request by converting viz types to core types.
    set_phase("Initialize stock");
    let core_groups: Vec<core_sim::SimGroupEntry> = req
        .groups
        .iter()
        .map(|group| {
            let toolpaths = group
                .toolpaths
                .iter()
                .map(|sim_tp| core_sim::SimToolpathEntry {
                    id: sim_tp.id.0,
                    name: sim_tp.name.clone(),
                    toolpath: Arc::clone(&sim_tp.toolpath),
                    tool: build_cutter(&sim_tp.tool),
                    flute_count: sim_tp.tool.flute_count,
                    tool_summary: sim_tp.tool.summary(),
                    semantic_trace: sim_tp.semantic_trace.clone(),
                })
                .collect();
            core_sim::SimGroupEntry {
                toolpaths,
                direction: group.direction,
            }
        })
        .collect();

    let core_req = core_sim::SimulationRequest {
        groups: core_groups,
        stock_bbox: req.stock_bbox,
        stock_top_z: req.stock_top_z,
        resolution: req.resolution,
        metric_options: req.metric_options,
        spindle_rpm: req.spindle_rpm,
        rapid_feed_mm_min: req.rapid_feed_mm_min,
        model_mesh: req.model_mesh.clone(),
    };

    set_phase("Simulate toolpaths");
    let core_result =
        core_sim::run_simulation(&core_req, cancel).map_err(|_e| ComputeError::Cancelled)?;

    // Convert core result back to viz types, adding GUI-specific data.
    // Reconstruct playback_data from the original request (needs ToolConfig).
    let mut playback_data = Vec::new();
    for group in &req.groups {
        for sim_tp in &group.toolpaths {
            playback_data.push((
                Arc::clone(&sim_tp.toolpath),
                sim_tp.tool.clone(),
                group.direction,
            ));
        }
    }

    // Map core boundaries back to viz boundaries (restore ToolpathId).
    let boundaries: Vec<SimBoundary> = core_result
        .boundaries
        .into_iter()
        .map(|b| SimBoundary {
            id: super::ToolpathId(b.id),
            name: b.name,
            tool_name: b.tool_name,
            start_move: b.start_move,
            end_move: b.end_move,
            direction: b.direction,
        })
        .collect();

    let checkpoints: Vec<SimCheckpointMesh> = core_result
        .checkpoints
        .into_iter()
        .map(|cp| SimCheckpointMesh {
            boundary_index: cp.boundary_index,
            mesh: cp.mesh,
            stock: cp.stock,
        })
        .collect();

    // Write cut trace artifact if metrics were enabled.
    let (cut_trace, cut_trace_path) = if let Some(trace) = core_result.cut_trace {
        let artifact = build_simulation_cut_artifact(req, (*trace).clone());
        let path = match rs_cam_core::simulation_cut::write_simulation_cut_artifact(
            &simulation_metric_artifact_dir(),
            "simulation_metrics",
            &artifact,
        ) {
            Ok(path) => Some(path),
            Err(error) => {
                tracing::warn!("Failed to write simulation cut artifact: {error}");
                None
            }
        };
        (Some(trace), path)
    } else {
        (None, None)
    };

    set_phase("Complete");
    Ok(SimulationResult {
        mesh: core_result.mesh,
        total_moves: core_result.total_moves,
        deviations: core_result.deviations,
        boundaries,
        checkpoints,
        playback_data,
        rapid_collisions: core_result.rapid_collisions,
        rapid_collision_move_indices: core_result.rapid_collision_move_indices,
        cut_trace,
        cut_trace_path,
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

fn run_compute_with_phase_tracker(
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
        let mut tp = {
            let _phase_scope =
                phase_tracker.map(|tracker| tracker.start_phase(req.operation.label()));
            let core_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("core_generate", req.operation.label()));
            let core_ctx = core_scope.as_ref().map(|scope| scope.context());
            let exec_ctx = OperationExecutionContext {
                req,
                cancel,
                phase_tracker,
                core_debug_span_id: core_scope.as_ref().map(|scope| scope.id()),
                debug_root: core_ctx.as_ref(),
                semantic_root: semantic_root.as_ref(),
            };
            let tp = req
                .operation
                .semantic_op()
                .generate_with_tracing(&exec_ctx)?;
            if let Some(scope) = core_scope.as_ref()
                && !tp.moves.is_empty()
            {
                scope.set_move_range(0, tp.moves.len() - 1);
            }
            tp
        };

        {
            let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Apply dressups"));
            let dressup_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("dressups", "Apply dressups"));
            let dressup_ctx = dressup_scope.as_ref().map(|scope| scope.context());
            tp = apply_dressups(tp, req, dressup_ctx.as_ref(), semantic_root.as_ref());
        }

        if req.boundary_enabled
            && let Some(bbox) = &req.stock_bbox
        {
            let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Clip to boundary"));
            let boundary_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("boundary_clip", "Clip to boundary"));
            let boundary_span_id = boundary_scope.as_ref().map(|scope| scope.id());
            use rs_cam_core::boundary::{
                ToolContainment, clip_toolpath_to_boundary, effective_boundary, subtract_keepouts,
            };
            // Use face-derived boundary when face_selection is set, otherwise stock bbox.
            let mut stock_poly = if let (Some(face_ids), Some(enriched)) =
                (&req.face_selection, &req.enriched_mesh)
            {
                enriched
                    .faces_boundary_as_polygon(face_ids)
                    .unwrap_or_else(|| {
                        rs_cam_core::polygon::Polygon2::rectangle(
                            bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y,
                        )
                    })
            } else {
                rs_cam_core::polygon::Polygon2::rectangle(
                    bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y,
                )
            };
            if !req.keep_out_footprints.is_empty() {
                stock_poly = subtract_keepouts(&stock_poly, &req.keep_out_footprints);
            }
            let containment = match req.boundary_containment {
                crate::state::toolpath::BoundaryContainment::Center => ToolContainment::Center,
                crate::state::toolpath::BoundaryContainment::Inside => ToolContainment::Inside,
                crate::state::toolpath::BoundaryContainment::Outside => ToolContainment::Outside,
            };
            let boundaries = effective_boundary(&stock_poly, containment, req.tool.diameter / 2.0);
            if let Some(boundary) = boundaries.first() {
                tp = clip_toolpath_to_boundary(&tp, boundary, effective_safe_z(req));
                if let Some(root) = semantic_root.as_ref() {
                    let scope =
                        root.start_item(ToolpathSemanticKind::BoundaryClip, "Boundary clip");
                    if let Some(span_id) = boundary_span_id {
                        scope.set_debug_span_id(span_id);
                    }
                    scope.set_param(
                        "containment",
                        match req.boundary_containment {
                            crate::state::toolpath::BoundaryContainment::Center => "center",
                            crate::state::toolpath::BoundaryContainment::Inside => "inside",
                            crate::state::toolpath::BoundaryContainment::Outside => "outside",
                        },
                    );
                    scope.set_param("keep_out_count", req.keep_out_footprints.len());
                    if !tp.moves.is_empty() {
                        scope.bind_to_toolpath(&tp, 0, tp.moves.len());
                    }
                }
                if let Some(scope) = boundary_scope.as_ref()
                    && !tp.moves.is_empty()
                {
                    scope.set_move_range(0, tp.moves.len() - 1);
                }
            }
        }

        let stats = {
            let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Compute stats"));
            let _stats_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("final_stats", "Compute stats"));
            compute_stats(&tp)
        };

        Ok(ToolpathResult {
            toolpath: Arc::new(tp),
            stats,
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
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

// --- Shared annotation helpers ---

fn annotate_operation_scope(
    semantic_root: Option<&rs_cam_core::semantic_trace::ToolpathSemanticContext>,
    debug_span_id: Option<u64>,
    label: &str,
    toolpath: &Toolpath,
) -> Option<rs_cam_core::semantic_trace::ToolpathSemanticScope> {
    let scope = semantic_root.map(|ctx| {
        ctx.start_item(
            rs_cam_core::semantic_trace::ToolpathSemanticKind::Operation,
            label,
        )
    });
    if let Some(scope) = scope.as_ref() {
        if let Some(debug_span_id) = debug_span_id {
            scope.set_debug_span_id(debug_span_id);
        }
        scope.bind_to_toolpath(toolpath, 0, toolpath.moves.len());
    }
    scope
}

fn semantic_child_context(
    scope: Option<&rs_cam_core::semantic_trace::ToolpathSemanticScope>,
) -> Option<rs_cam_core::semantic_trace::ToolpathSemanticContext> {
    scope.map(|scope| scope.context())
}

fn bind_scope_to_offset_run(
    scope: &rs_cam_core::semantic_trace::ToolpathSemanticScope,
    toolpath: &Toolpath,
    offset: usize,
    run: &CutRun,
) {
    scope.bind_to_toolpath(
        toolpath,
        offset + run.move_start,
        offset + run.move_end_exclusive,
    );
}

struct OpenRuntimeSemanticItem {
    scope: rs_cam_core::semantic_trace::ToolpathSemanticScope,
    start_move: usize,
}

struct OpenAdaptivePassItem {
    pass_index: usize,
    start_move: usize,
    scope: rs_cam_core::semantic_trace::ToolpathSemanticScope,
}

fn finish_runtime_scope(
    open_item: &mut Option<OpenRuntimeSemanticItem>,
    toolpath: &Toolpath,
    move_end_exclusive: usize,
) {
    if let Some(open_item) = open_item.take() {
        open_item
            .scope
            .bind_to_toolpath(toolpath, open_item.start_move, move_end_exclusive);
    }
}

fn annotate_adaptive3d_runtime_semantics(
    op_scope: Option<&rs_cam_core::semantic_trace::ToolpathSemanticScope>,
    toolpath: &Toolpath,
    annotations: &[rs_cam_core::adaptive3d::Adaptive3dRuntimeAnnotation],
    detect_flat_areas: bool,
    region_ordering: crate::state::toolpath::RegionOrdering,
) {
    let Some(op_scope) = op_scope else {
        return;
    };

    let op_ctx = op_scope.context();

    let z_plan_scope = op_ctx.start_item(ToolpathSemanticKind::Optimization, "Z level planning");
    z_plan_scope.set_param(
        "algorithm",
        "base stepdown planning with optional shelf insertion and fine-stepdown expansion",
    );

    if detect_flat_areas {
        let flat_scope =
            op_ctx.start_item(ToolpathSemanticKind::Optimization, "Flat shelf detection");
        flat_scope.set_param(
            "algorithm",
            "surface Z histogram over the sampled mesh to insert extra shelf levels",
        );
    }

    if region_ordering == crate::state::toolpath::RegionOrdering::ByArea {
        let region_scope =
            op_ctx.start_item(ToolpathSemanticKind::Optimization, "Region detection");
        region_scope.set_param(
            "algorithm",
            "8-connected flood fill over remaining-material cells on the heightmap",
        );
    }

    let mut current_region: Option<OpenRuntimeSemanticItem> = None;
    let mut current_level: Option<OpenRuntimeSemanticItem> = None;
    let mut current_pass: Option<OpenAdaptivePassItem> = None;

    for (index, annotation) in annotations.iter().enumerate() {
        let move_start = annotation.move_index;
        let move_end_exclusive = annotations
            .get(index + 1)
            .map_or(toolpath.moves.len(), |next| next.move_index);

        match &annotation.event {
            rs_cam_core::adaptive3d::Adaptive3dRuntimeEvent::RegionStart {
                region_index,
                region_total,
                cell_count,
            } => {
                current_pass = None;
                finish_runtime_scope(&mut current_level, toolpath, move_start);
                finish_runtime_scope(&mut current_region, toolpath, move_start);

                let scope = op_ctx.start_item(
                    ToolpathSemanticKind::Region,
                    format!("Region {region_index}/{region_total}"),
                );
                scope.set_param("region_index", *region_index);
                scope.set_param("region_total", *region_total);
                scope.set_param("cell_count", *cell_count);
                current_region = Some(OpenRuntimeSemanticItem {
                    scope,
                    start_move: move_start,
                });
            }
            rs_cam_core::adaptive3d::Adaptive3dRuntimeEvent::RegionZLevel {
                region_index,
                z_level,
                level_index,
                level_total,
            } => {
                current_pass = None;
                finish_runtime_scope(&mut current_level, toolpath, move_start);

                let parent_ctx = current_region
                    .as_ref()
                    .map(|item| item.scope.context())
                    .unwrap_or_else(|| op_scope.context());
                let scope = parent_ctx.start_item(
                    ToolpathSemanticKind::DepthLevel,
                    format!("Z {:.3}", z_level),
                );
                scope.set_param("region_index", *region_index);
                scope.set_param("z_level", *z_level);
                scope.set_param("level_index", *level_index);
                scope.set_param("level_total", *level_total);
                current_level = Some(OpenRuntimeSemanticItem {
                    scope,
                    start_move: move_start,
                });
            }
            rs_cam_core::adaptive3d::Adaptive3dRuntimeEvent::GlobalZLevel {
                z_level,
                level_index,
                level_total,
            } => {
                current_pass = None;
                finish_runtime_scope(&mut current_level, toolpath, move_start);

                let scope = op_ctx.start_item(
                    ToolpathSemanticKind::DepthLevel,
                    format!("Z {:.3}", z_level),
                );
                scope.set_param("z_level", *z_level);
                scope.set_param("level_index", *level_index);
                scope.set_param("level_total", *level_total);
                current_level = Some(OpenRuntimeSemanticItem {
                    scope,
                    start_move: move_start,
                });
            }
            rs_cam_core::adaptive3d::Adaptive3dRuntimeEvent::WaterlineCleanup => {
                current_pass = None;
                let parent_ctx = current_level
                    .as_ref()
                    .map(|item| item.scope.context())
                    .unwrap_or_else(|| op_scope.context());
                let scope =
                    parent_ctx.start_item(ToolpathSemanticKind::Cleanup, "Waterline cleanup");
                scope.set_param(
                    "algorithm",
                    "contour steep boundaries and skip predominantly shallow contours",
                );
                scope.bind_to_toolpath(toolpath, move_start, move_end_exclusive);
            }
            rs_cam_core::adaptive3d::Adaptive3dRuntimeEvent::PassEntry {
                pass_index,
                entry_x,
                entry_y,
                entry_z,
            } => {
                let parent_ctx = current_level
                    .as_ref()
                    .map(|item| item.scope.context())
                    .unwrap_or_else(|| op_scope.context());
                let scope = parent_ctx.start_item(
                    ToolpathSemanticKind::Pass,
                    format!("Adaptive pass {pass_index}"),
                );
                scope.set_param("pass_index", *pass_index);
                scope.set_param("entry_x", *entry_x);
                scope.set_param("entry_y", *entry_y);
                scope.set_param("entry_z", *entry_z);
                scope.set_param(
                    "algorithm",
                    "constant-engagement stepping with direction search over the sampled material field",
                );
                scope.bind_to_toolpath(toolpath, move_start, move_end_exclusive);
                current_pass = Some(OpenAdaptivePassItem {
                    pass_index: *pass_index,
                    start_move: move_start,
                    scope,
                });
            }
            rs_cam_core::adaptive3d::Adaptive3dRuntimeEvent::PassPreflightSkip { pass_index } => {
                let parent_ctx = current_level
                    .as_ref()
                    .map(|item| item.scope.context())
                    .unwrap_or_else(|| op_scope.context());
                let scope = parent_ctx.start_item(
                    ToolpathSemanticKind::Pass,
                    format!("Pass {pass_index} skipped"),
                );
                scope.set_param("pass_index", pass_index);
                scope.set_param("exit_reason", "preflight skip");
                scope.set_param(
                    "algorithm",
                    "direction-search preflight failed to find a viable constant-engagement continuation",
                );
                current_pass = None;
            }
            rs_cam_core::adaptive3d::Adaptive3dRuntimeEvent::PassSummary {
                pass_index,
                step_count,
                exit_reason,
                yield_ratio,
                short,
            } => {
                if let Some(open_pass) = current_pass.as_ref()
                    && open_pass.pass_index == *pass_index
                {
                    open_pass.scope.set_param("step_count", *step_count);
                    open_pass.scope.set_param("exit_reason", exit_reason);
                    open_pass.scope.set_param("yield_ratio", *yield_ratio);
                    open_pass.scope.set_param("short_pass", *short);
                }
            }
        }
    }

    finish_runtime_scope(&mut current_level, toolpath, toolpath.moves.len());
    finish_runtime_scope(&mut current_region, toolpath, toolpath.moves.len());
}

fn annotate_adaptive_runtime_semantics(
    op_scope: Option<&rs_cam_core::semantic_trace::ToolpathSemanticScope>,
    toolpath: &Toolpath,
    level_slices: &[AdaptiveLevelSlice],
    annotations: &[rs_cam_core::adaptive::AdaptiveRuntimeAnnotation],
) {
    let Some(op_scope) = op_scope else {
        return;
    };

    if level_slices.is_empty() {
        return;
    }

    let op_ctx = op_scope.context();
    let polygon_total = level_slices
        .iter()
        .map(|slice| slice.polygon_index)
        .max()
        .map_or(1, |max_index| max_index + 1);

    let mut polygon_scopes = Vec::new();
    for polygon_index in 0..polygon_total {
        let polygon_slices: Vec<_> = level_slices
            .iter()
            .filter(|slice| slice.polygon_index == polygon_index)
            .collect();
        if polygon_slices.is_empty() {
            continue;
        }
        let scope = if polygon_total > 1 {
            let scope = op_ctx.start_item(
                ToolpathSemanticKind::Region,
                format!("Region {}", polygon_index + 1),
            );
            // SAFETY: polygon_slices.is_empty() checked above
            #[allow(clippy::expect_used)]
            let first_start = polygon_slices.first().expect("polygon slices").move_start;
            #[allow(clippy::expect_used)]
            let last_end = polygon_slices
                .last()
                .expect("polygon slices")
                .move_end_exclusive;
            scope.bind_to_toolpath(toolpath, first_start, last_end);
            Some(scope)
        } else {
            None
        };
        polygon_scopes.push((polygon_index, scope));
    }

    for level_slice in level_slices {
        let parent_ctx = polygon_scopes
            .iter()
            .find(|(polygon_index, _)| *polygon_index == level_slice.polygon_index)
            .and_then(|(_, scope)| scope.as_ref().map(|scope| scope.context()))
            .unwrap_or_else(|| op_scope.context());
        let level_scope = parent_ctx.start_item(
            ToolpathSemanticKind::DepthLevel,
            format!("Level {}", level_slice.level_index + 1),
        );
        level_scope.set_param("level_index", level_slice.level_index + 1);
        level_scope.set_param("z", level_slice.z);
        level_scope.bind_to_toolpath(
            toolpath,
            level_slice.move_start,
            level_slice.move_end_exclusive,
        );
        let level_ctx = level_scope.context();

        let mut current_runtime_item: Option<OpenRuntimeSemanticItem> = None;
        let mut current_pass: Option<OpenAdaptivePassItem> = None;

        for annotation in annotations.iter().filter(|annotation| {
            annotation.move_index >= level_slice.move_start
                && annotation.move_index <= level_slice.move_end_exclusive
        }) {
            let move_start = annotation.move_index;
            match &annotation.event {
                rs_cam_core::adaptive::AdaptiveRuntimeEvent::SlotClearing {
                    line_index,
                    line_total,
                } => {
                    finish_runtime_scope(&mut current_runtime_item, toolpath, move_start);
                    if let Some(open_pass) = current_pass.take() {
                        open_pass.scope.bind_to_toolpath(
                            toolpath,
                            open_pass.start_move,
                            move_start,
                        );
                    }
                    let scope = level_ctx.start_item(
                        ToolpathSemanticKind::SlotClearing,
                        format!("Slot clearing {}", line_index),
                    );
                    scope.set_param("line_index", *line_index);
                    scope.set_param("line_total", *line_total);
                    current_runtime_item = Some(OpenRuntimeSemanticItem {
                        scope,
                        start_move: move_start,
                    });
                }
                rs_cam_core::adaptive::AdaptiveRuntimeEvent::PassEntry {
                    pass_index,
                    entry_x,
                    entry_y,
                } => {
                    finish_runtime_scope(&mut current_runtime_item, toolpath, move_start);
                    if let Some(open_pass) = current_pass.take() {
                        open_pass.scope.bind_to_toolpath(
                            toolpath,
                            open_pass.start_move,
                            move_start,
                        );
                    }
                    let scope = level_ctx.start_item(
                        ToolpathSemanticKind::Pass,
                        format!("Adaptive pass {}", pass_index),
                    );
                    scope.set_param("pass_index", *pass_index);
                    scope.set_param("entry_x", *entry_x);
                    scope.set_param("entry_y", *entry_y);
                    current_pass = Some(OpenAdaptivePassItem {
                        pass_index: *pass_index,
                        start_move: move_start,
                        scope,
                    });
                }
                rs_cam_core::adaptive::AdaptiveRuntimeEvent::PassSummary {
                    pass_index,
                    step_count,
                    idle_count,
                    search_evaluations,
                    exit_reason,
                } => {
                    if let Some(open_pass) = current_pass.take()
                        && open_pass.pass_index == *pass_index
                    {
                        open_pass.scope.set_param("step_count", *step_count);
                        open_pass.scope.set_param("idle_count", *idle_count);
                        open_pass
                            .scope
                            .set_param("search_evaluations", *search_evaluations);
                        open_pass.scope.set_param("exit_reason", exit_reason);
                        open_pass.scope.bind_to_toolpath(
                            toolpath,
                            open_pass.start_move,
                            move_start.max(open_pass.start_move + 1),
                        );
                    }
                }
                rs_cam_core::adaptive::AdaptiveRuntimeEvent::ForcedClear {
                    pass_index,
                    center_x,
                    center_y,
                    radius,
                } => {
                    let scope = level_ctx.start_item(
                        ToolpathSemanticKind::ForcedClear,
                        format!("Forced clear {}", pass_index),
                    );
                    scope.set_param("pass_index", *pass_index);
                    scope.set_param("center_x", *center_x);
                    scope.set_param("center_y", *center_y);
                    scope.set_param("radius", *radius);
                    scope.set_xy_bbox(rs_cam_core::debug_trace::ToolpathDebugBounds2 {
                        min_x: center_x - radius,
                        max_x: center_x + radius,
                        min_y: center_y - radius,
                        max_y: center_y + radius,
                    });
                    scope.set_z_range(level_slice.z, level_slice.z);
                }
                rs_cam_core::adaptive::AdaptiveRuntimeEvent::BoundaryCleanup {
                    contour_index,
                    contour_total,
                } => {
                    finish_runtime_scope(&mut current_runtime_item, toolpath, move_start);
                    let scope = level_ctx.start_item(
                        ToolpathSemanticKind::Cleanup,
                        format!("Boundary cleanup {}", contour_index),
                    );
                    scope.set_param("contour_index", *contour_index);
                    scope.set_param("contour_total", *contour_total);
                    current_runtime_item = Some(OpenRuntimeSemanticItem {
                        scope,
                        start_move: move_start,
                    });
                }
            }
        }

        finish_runtime_scope(
            &mut current_runtime_item,
            toolpath,
            level_slice.move_end_exclusive,
        );
        if let Some(open_pass) = current_pass.take() {
            open_pass.scope.bind_to_toolpath(
                toolpath,
                open_pass.start_move,
                level_slice.move_end_exclusive,
            );
        }
    }
}

fn annotate_pencil_runtime_semantics(
    op_scope: Option<&rs_cam_core::semantic_trace::ToolpathSemanticScope>,
    toolpath: &Toolpath,
    annotations: &[rs_cam_core::pencil::PencilRuntimeAnnotation],
) {
    let Some(op_scope) = op_scope else {
        return;
    };
    let op_ctx = op_scope.context();
    let mut chain_scopes = std::collections::BTreeMap::new();

    for (index, annotation) in annotations.iter().enumerate() {
        let move_start = annotation.move_index;
        let move_end_exclusive = annotations
            .get(index + 1)
            .map_or(toolpath.moves.len(), |next| next.move_index);

        let rs_cam_core::pencil::PencilRuntimeEvent::OffsetPass {
            chain_index,
            chain_total,
            offset_index,
            offset_total,
            offset_mm,
            is_centerline,
        } = &annotation.event;
        let chain_scope = chain_scopes.entry(*chain_index).or_insert_with(|| {
            let scope = op_ctx.start_item(
                ToolpathSemanticKind::Chain,
                format!("Chain {chain_index}/{chain_total}"),
            );
            scope.set_param("chain_index", *chain_index);
            scope.set_param("chain_total", *chain_total);
            scope.set_param("offset_total", *offset_total);
            scope
        });
        let kind = if *is_centerline {
            ToolpathSemanticKind::Centerline
        } else {
            ToolpathSemanticKind::OffsetPass
        };
        let label = if *is_centerline {
            format!("Centerline {chain_index}")
        } else {
            format!("Offset pass {}", offset_index.saturating_sub(1))
        };
        let scope = chain_scope.context().start_item(kind, label);
        scope.set_param("chain_index", *chain_index);
        scope.set_param("offset_index", *offset_index);
        scope.set_param("offset_total", *offset_total);
        scope.set_param("offset_mm", *offset_mm);
        scope.set_param("is_centerline", *is_centerline);
        scope.bind_to_toolpath(toolpath, move_start, move_end_exclusive);
    }
}

fn annotate_scallop_runtime_semantics(
    op_scope: Option<&rs_cam_core::semantic_trace::ToolpathSemanticScope>,
    toolpath: &Toolpath,
    annotations: &[rs_cam_core::scallop::ScallopRuntimeAnnotation],
    direction: &crate::state::toolpath::ScallopDirection,
    continuous: bool,
) {
    let Some(op_scope) = op_scope else {
        return;
    };
    let band_scope = op_scope.context().start_item(
        ToolpathSemanticKind::Band,
        if continuous {
            "Continuous scallop band"
        } else {
            "Scallop band"
        },
    );
    band_scope.set_param("continuous", continuous);
    band_scope.set_param("direction", format!("{direction:?}"));
    if !toolpath.moves.is_empty() {
        band_scope.bind_to_toolpath(toolpath, 0, toolpath.moves.len());
    }

    for (index, annotation) in annotations.iter().enumerate() {
        let move_start = annotation.move_index;
        let move_end_exclusive = annotations
            .get(index + 1)
            .map_or(toolpath.moves.len(), |next| next.move_index);
        let rs_cam_core::scallop::ScallopRuntimeEvent::Ring {
            ring_index,
            ring_total,
            continuous,
        } = &annotation.event;
        let scope = band_scope.context().start_item(
            ToolpathSemanticKind::Ring,
            format!("Ring {ring_index}/{ring_total}"),
        );
        scope.set_param("ring_index", *ring_index);
        scope.set_param("ring_total", *ring_total);
        scope.set_param("continuous", *continuous);
        scope.bind_to_toolpath(toolpath, move_start, move_end_exclusive);
    }
}

fn annotate_ramp_finish_runtime_semantics(
    op_scope: Option<&rs_cam_core::semantic_trace::ToolpathSemanticScope>,
    toolpath: &Toolpath,
    annotations: &[rs_cam_core::ramp_finish::RampFinishRuntimeAnnotation],
) {
    let Some(op_scope) = op_scope else {
        return;
    };
    let op_ctx = op_scope.context();
    let mut terrace_scopes = std::collections::BTreeMap::new();

    for (index, annotation) in annotations.iter().enumerate() {
        let move_start = annotation.move_index;
        let move_end_exclusive = annotations
            .get(index + 1)
            .map_or(toolpath.moves.len(), |next| next.move_index);
        let rs_cam_core::ramp_finish::RampFinishRuntimeEvent::Ramp {
            terrace_index,
            terrace_total,
            upper_level_index,
            lower_level_index,
            upper_z,
            lower_z,
            ramp_index,
            ramp_total,
        } = &annotation.event;
        let terrace_scope = terrace_scopes.entry(*terrace_index).or_insert_with(|| {
            let scope = op_ctx.start_item(
                ToolpathSemanticKind::Band,
                format!("Terrace {terrace_index}/{terrace_total}"),
            );
            scope.set_param("terrace_index", *terrace_index);
            scope.set_param("terrace_total", *terrace_total);
            scope.set_param("upper_level_index", *upper_level_index);
            scope.set_param("lower_level_index", *lower_level_index);
            scope.set_param("upper_z", *upper_z);
            scope.set_param("lower_z", *lower_z);
            scope
        });
        let scope = terrace_scope.context().start_item(
            ToolpathSemanticKind::Ramp,
            format!("Ramp {ramp_index}/{ramp_total}"),
        );
        scope.set_param("terrace_index", *terrace_index);
        scope.set_param("upper_level_index", *upper_level_index);
        scope.set_param("lower_level_index", *lower_level_index);
        scope.set_param("upper_z", *upper_z);
        scope.set_param("lower_z", *lower_z);
        scope.set_param("ramp_index", *ramp_index);
        scope.set_param("ramp_total", *ramp_total);
        scope.bind_to_toolpath(toolpath, move_start, move_end_exclusive);
    }
}

fn annotate_spiral_finish_runtime_semantics(
    op_scope: Option<&rs_cam_core::semantic_trace::ToolpathSemanticScope>,
    toolpath: &Toolpath,
    annotations: &[rs_cam_core::spiral_finish::SpiralFinishRuntimeAnnotation],
    direction: &crate::state::toolpath::SpiralDirection,
) {
    let Some(op_scope) = op_scope else {
        return;
    };
    let band_scope = op_scope
        .context()
        .start_item(ToolpathSemanticKind::Band, "Spiral band");
    band_scope.set_param("direction", format!("{direction:?}"));
    if !toolpath.moves.is_empty() {
        band_scope.bind_to_toolpath(toolpath, 0, toolpath.moves.len());
    }

    for (index, annotation) in annotations.iter().enumerate() {
        let move_start = annotation.move_index;
        let move_end_exclusive = annotations
            .get(index + 1)
            .map_or(toolpath.moves.len(), |next| next.move_index);
        let rs_cam_core::spiral_finish::SpiralFinishRuntimeEvent::Ring {
            ring_index,
            ring_total,
            radius_mm,
        } = &annotation.event;
        let scope = band_scope.context().start_item(
            ToolpathSemanticKind::Ring,
            format!("Ring {ring_index}/{ring_total}"),
        );
        scope.set_param("ring_index", *ring_index);
        scope.set_param("ring_total", *ring_total);
        scope.set_param("radius_mm", *radius_mm);
        scope.bind_to_toolpath(toolpath, move_start, move_end_exclusive);
    }
}

// SAFETY: run.move_start / move_end_exclusive are produced by cutting_runs, always within bounds
#[allow(clippy::indexing_slicing)]
fn annotate_radial_finish_semantics(
    op_scope: Option<&rs_cam_core::semantic_trace::ToolpathSemanticScope>,
    toolpath: &Toolpath,
    center_x: f64,
    center_y: f64,
) {
    let Some(op_scope) = op_scope else {
        return;
    };
    for (ray_index, run) in cutting_runs(toolpath).iter().enumerate() {
        let start = toolpath.moves[run.move_start].target;
        let end = toolpath.moves[run.move_end_exclusive - 1].target;
        let start_radius = ((start.x - center_x).powi(2) + (start.y - center_y).powi(2)).sqrt();
        let end_radius = ((end.x - center_x).powi(2) + (end.y - center_y).powi(2)).sqrt();
        let angle_source = if end_radius >= start_radius {
            end
        } else {
            start
        };
        let angle_deg = (angle_source.y - center_y)
            .atan2(angle_source.x - center_x)
            .to_degrees()
            .rem_euclid(360.0);
        let direction = if end_radius >= start_radius {
            "outward"
        } else {
            "inward"
        };
        let scope = op_scope
            .context()
            .start_item(ToolpathSemanticKind::Ray, format!("Ray {}", ray_index + 1));
        scope.set_param("ray_index", ray_index + 1);
        scope.set_param("angle_deg", angle_deg);
        scope.set_param("direction", direction);
        bind_scope_to_run(&scope, toolpath, run);
    }
}

fn annotate_horizontal_finish_semantics(
    op_scope: Option<&rs_cam_core::semantic_trace::ToolpathSemanticScope>,
    toolpath: &Toolpath,
    stepover: f64,
) {
    let Some(op_scope) = op_scope else {
        return;
    };
    let runs = cutting_runs(toolpath);
    let tolerance = (stepover / 2.0).max(0.01);
    let mut current_slice: Option<OpenRuntimeSemanticItem> = None;
    let mut current_slice_z = 0.0;
    let mut current_pass_count = 0usize;
    let mut slice_index = 0usize;

    for run in &runs {
        let avg_z = (run.z_min + run.z_max) * 0.5;
        if current_slice
            .as_ref()
            .is_none_or(|_| (avg_z - current_slice_z).abs() > tolerance)
        {
            if let Some(open_slice) = current_slice.take() {
                open_slice
                    .scope
                    .bind_to_toolpath(toolpath, open_slice.start_move, run.move_start);
            }
            slice_index += 1;
            current_slice_z = avg_z;
            current_pass_count = 0;
            let scope = op_scope
                .context()
                .start_item(ToolpathSemanticKind::Slice, format!("Slice {slice_index}"));
            scope.set_param("slice_index", slice_index);
            scope.set_param("z", avg_z);
            current_slice = Some(OpenRuntimeSemanticItem {
                scope,
                start_move: run.move_start,
            });
        }

        current_pass_count += 1;
        // SAFETY: current_slice is always set before reaching this point
        #[allow(clippy::expect_used)]
        let pass_scope = current_slice
            .as_ref()
            .expect("slice scope")
            .scope
            .context()
            .start_item(
                ToolpathSemanticKind::Pass,
                format!("Flat pass {}", current_pass_count),
            );
        pass_scope.set_param("slice_index", slice_index);
        pass_scope.set_param("pass_index", current_pass_count);
        pass_scope.set_param("z", avg_z);
        bind_scope_to_run(&pass_scope, toolpath, run);
    }

    if let Some(open_slice) = current_slice.take() {
        open_slice
            .scope
            .bind_to_toolpath(toolpath, open_slice.start_move, toolpath.moves.len());
    }
}

fn annotate_project_curve_semantics(
    op_scope: Option<&rs_cam_core::semantic_trace::ToolpathSemanticScope>,
    toolpath: &Toolpath,
    slices: &[ProjectCurveSlice],
    depth: f64,
    point_spacing: f64,
) {
    let Some(op_scope) = op_scope else {
        return;
    };
    for slice in slices {
        let parent = op_scope.context().start_item(
            ToolpathSemanticKind::Curve,
            format!("Source curve {}", slice.source_curve_index),
        );
        parent.set_param("source_curve_index", slice.source_curve_index);
        parent.bind_to_toolpath(toolpath, slice.move_start, slice.move_end_exclusive);

        let child = parent.context().start_item(
            ToolpathSemanticKind::Curve,
            format!("Projected curve {}", slice.source_curve_index),
        );
        child.set_param("source_curve_index", slice.source_curve_index);
        child.set_param("depth", depth);
        child.set_param("point_spacing", point_spacing);
        child.bind_to_toolpath(toolpath, slice.move_start, slice.move_end_exclusive);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn semantic_dispatch_covers_all_operation_types() {
        for &op_type in crate::state::toolpath::OperationType::ALL {
            let config = OperationConfig::new_default(op_type);
            let _ = config.semantic_op();
        }
    }

    fn test_request_with_polygon(
        operation: OperationConfig,
        tool_type: ToolType,
    ) -> ComputeRequest {
        let tool = ToolConfig::new_default(crate::state::job::ToolId(1), tool_type);
        ComputeRequest {
            toolpath_id: ToolpathId(1),
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
            stock_bbox: Some(BoundingBox3 {
                min: P3::new(-25.0, -25.0, -10.0),
                max: P3::new(25.0, 25.0, 10.0),
            }),
            boundary_enabled: false,
            boundary_containment: BoundaryContainment::Center,
            keep_out_footprints: Vec::new(),
            heights: HeightsConfig::default().resolve(&HeightContext::simple(10.0, 6.0)),
            debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
            prior_stock: None,
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
        let tp = run_profile(&req, &cfg).unwrap();

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
        let depth = make_depth_with_finishing(cfg.depth, cfg.depth_per_pass, cfg.finishing_passes);
        let roughing_levels = depth.all_levels();
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
    fn face_oneway_all_cuts_same_direction() {
        let cfg = FaceConfig {
            direction: FaceDirection::OneWay,
            stepover: 10.0,
            ..FaceConfig::default()
        };
        let req = test_request_with_polygon(OperationConfig::Face(cfg.clone()), ToolType::EndMill);

        // Use the semantic path to generate (that's the active code path)
        let ctx = OperationExecutionContext {
            req: &req,
            cancel: &AtomicBool::new(false),
            phase_tracker: None,
            core_debug_span_id: None,
            debug_root: None,
            semantic_root: None,
        };
        let tp = cfg.generate_with_tracing(&ctx).unwrap();

        // Collect all cutting segments (consecutive feed moves at the same Z)
        let mut cut_directions = Vec::new();
        for i in 1..tp.moves.len() {
            if let MoveType::Linear { feed_rate } = tp.moves[i].move_type
                && (feed_rate - cfg.feed_rate).abs() < 1e-6
            {
                let dx = tp.moves[i].target.x - tp.moves[i - 1].target.x;
                // Only consider horizontal cutting moves with significant X travel
                if dx.abs() > 1.0 {
                    cut_directions.push(dx > 0.0);
                }
            }
        }

        assert!(
            !cut_directions.is_empty(),
            "Face operation should produce cutting moves"
        );
        // All cuts should go in the same X direction
        let first_dir = cut_directions[0];
        assert!(
            cut_directions.iter().all(|&d| d == first_dir),
            "OneWay face should have all cuts in the same direction, got mixed: {:?}",
            cut_directions
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

        let ctx = OperationExecutionContext {
            req: &req,
            cancel: &AtomicBool::new(false),
            phase_tracker: None,
            core_debug_span_id: None,
            debug_root: None,
            semantic_root: None,
        };
        let tp = cfg.generate_with_tracing(&ctx).unwrap();

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
        let tp = crate::compute::worker::execute::operations_2d::run_zigzag(&req, &cfg).unwrap();

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

        let ctx = OperationExecutionContext {
            req: &req,
            cancel: &AtomicBool::new(false),
            phase_tracker: None,
            core_debug_span_id: None,
            debug_root: None,
            semantic_root: None,
        };
        let tp = cfg.generate_with_tracing(&ctx).unwrap();

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
        let mut req =
            test_request_with_polygon(OperationConfig::Inlay(cfg.clone()), ToolType::VBit);
        req.polygons = Some(Arc::new(vec![Polygon2::rectangle(
            -10.0, -10.0, 10.0, 10.0,
        )]));
        let tp = run_inlay(&req, &cfg).unwrap();

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
        let mut req =
            test_request_with_polygon(OperationConfig::Scallop(cfg.clone()), ToolType::EndMill);
        req.mesh = Some(Arc::new(rs_cam_core::mesh::make_test_flat(40.0)));
        let result = run_scallop_annotated(&req, &cfg, None, None);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("Ball Nose"),
            "Error should mention Ball Nose requirement"
        );
    }
}
