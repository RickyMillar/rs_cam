use super::helpers::*;
use super::semantic::*;
use super::*;
use rs_cam_core::geo::P3;
use rs_cam_core::semantic_trace::ToolpathSemanticKind;

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
    set_phase("Initialize stock");
    let mut stock = TriDexelStock::from_bounds(&req.stock_bbox, req.resolution);
    let sample_step_mm = req.resolution.max(0.25);

    let mut total_moves = 0;
    let mut boundary_index = 0;
    let mut boundaries = Vec::new();
    let mut checkpoints = Vec::new();
    let mut playback_data = Vec::new();
    let mut cut_samples = Vec::new();

    for group in &req.groups {
        for sim_toolpath in &group.toolpaths {
            let tp_id = sim_toolpath.id;
            let tp_name = &sim_toolpath.name;
            let toolpath = &sim_toolpath.toolpath;
            let tool_config = &sim_toolpath.tool;
            set_phase(&format!("Simulate {tp_name}"));
            let start_move = total_moves;
            let cutter = build_cutter(tool_config);
            if req.metric_options.enabled {
                let mut samples = stock
                    .simulate_toolpath_with_metrics_with_cancel(
                        toolpath,
                        cutter.as_ref(),
                        group.direction,
                        tp_id.0,
                        req.spindle_rpm,
                        tool_config.flute_count,
                        req.rapid_feed_mm_min,
                        sample_step_mm,
                        sim_toolpath.semantic_trace.as_deref(),
                        &|| cancel.load(Ordering::SeqCst),
                    )
                    .map_err(|_| ComputeError::Cancelled)?;
                cut_samples.append(&mut samples);
            } else {
                stock
                    .simulate_toolpath_with_cancel(
                        toolpath,
                        cutter.as_ref(),
                        group.direction,
                        &|| cancel.load(Ordering::SeqCst),
                    )
                    .map_err(|_| ComputeError::Cancelled)?;
            }
            total_moves += toolpath.moves.len();

            boundaries.push(SimBoundary {
                id: tp_id,
                name: tp_name.clone(),
                tool_name: tool_config.summary(),
                start_move,
                end_move: total_moves,
                direction: group.direction,
            });

            checkpoints.push(SimCheckpointMesh {
                boundary_index,
                mesh: dexel_stock_to_mesh(&stock),
                stock: stock.checkpoint(),
            });

            playback_data.push((Arc::clone(toolpath), tool_config.clone(), group.direction));

            boundary_index += 1;
        }
    }

    // Check for rapid-through-stock collisions on each toolpath
    set_phase("Scan rapid collisions");
    let mut rapid_collisions = Vec::new();
    let mut rapid_collision_move_indices = Vec::new();
    {
        use rs_cam_core::collision::check_rapid_collisions;
        let mut cumulative_offset = 0;
        for group in &req.groups {
            for sim_toolpath in &group.toolpaths {
                let rapids = check_rapid_collisions(&sim_toolpath.toolpath, &req.stock_bbox);
                for rc in &rapids {
                    rapid_collision_move_indices.push(cumulative_offset + rc.move_index);
                }
                rapid_collisions.extend(rapids);
                cumulative_offset += sim_toolpath.toolpath.moves.len();
            }
        }
    }

    let (cut_trace, cut_trace_path) = if req.metric_options.enabled {
        let semantic_traces: Vec<_> = req
            .groups
            .iter()
            .flat_map(|group| {
                group.toolpaths.iter().filter_map(|toolpath| {
                    toolpath
                        .semantic_trace
                        .as_deref()
                        .map(|trace| (toolpath.id.0, trace))
                })
            })
            .collect();
        let trace = rs_cam_core::simulation_cut::SimulationCutTrace::from_samples_with_semantics(
            sample_step_mm,
            cut_samples,
            semantic_traces,
        );
        let artifact = build_simulation_cut_artifact(req, trace.clone());
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
        (Some(Arc::new(trace)), path)
    } else {
        (None, None)
    };

    set_phase("Build simulation mesh");
    Ok(SimulationResult {
        mesh: dexel_stock_to_mesh(&stock),
        total_moves,
        deviations: None,
        boundaries,
        checkpoints,
        playback_data,
        rapid_collisions,
        rapid_collision_move_indices,
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
            let mut stock_poly = rs_cam_core::polygon::Polygon2::rectangle(
                bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y,
            );
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

#[allow(dead_code)]
fn run_pocket(req: &ComputeRequest, cfg: &PocketConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let depth = make_depth_with_finishing(cfg.depth, cfg.depth_per_pass, cfg.finishing_passes);
    let mut out = Toolpath::new();
    for p in polys {
        let tp = depth_stepped_toolpath(&depth, effective_safe_z(req), |z| match cfg.pattern {
            PocketPattern::Contour => pocket_toolpath(
                p,
                &PocketParams {
                    tool_radius: tr,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: effective_safe_z(req),
                    climb: cfg.climb,
                },
            ),
            PocketPattern::Zigzag => zigzag_toolpath(
                p,
                &ZigzagParams {
                    tool_radius: tr,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: effective_safe_z(req),
                    angle: cfg.angle.to_radians(),
                },
            ),
        });
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

fn run_profile(req: &ComputeRequest, cfg: &ProfileConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let side = match cfg.side {
        crate::state::toolpath::ProfileSide::Outside => rs_cam_core::profile::ProfileSide::Outside,
        crate::state::toolpath::ProfileSide::Inside => rs_cam_core::profile::ProfileSide::Inside,
    };
    let depth = make_depth_with_finishing(cfg.depth, cfg.depth_per_pass, cfg.finishing_passes);
    let mut out = Toolpath::new();
    for p in polys {
        let mut tp = depth_stepped_toolpath(&depth, effective_safe_z(req), |z| {
            profile_toolpath(
                p,
                &ProfileParams {
                    tool_radius: tr,
                    side,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: effective_safe_z(req),
                    climb: cfg.climb,
                },
            )
        });
        if cfg.tab_count > 0 {
            tp = apply_tabs(
                &tp,
                &even_tabs(cfg.tab_count, cfg.tab_width, cfg.tab_height),
                -cfg.depth.abs(),
            );
        }
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

fn run_adaptive_annotated(
    req: &ComputeRequest,
    cfg: &AdaptiveConfig,
    cancel: &AtomicBool,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<
    (
        Toolpath,
        Vec<AdaptiveLevelSlice>,
        Vec<rs_cam_core::adaptive::AdaptiveRuntimeAnnotation>,
    ),
    ComputeError,
> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let depth = make_depth(cfg.depth, cfg.depth_per_pass);
    let mut out = Toolpath::new();
    let mut level_slices = Vec::new();
    let mut annotations = Vec::new();
    for (polygon_idx, p) in polys.iter().enumerate() {
        for (level_idx, z) in depth.all_levels().into_iter().enumerate() {
            let (tp, level_annotations) =
                rs_cam_core::adaptive::adaptive_toolpath_structured_annotated_traced_with_cancel(
                    p,
                    &AdaptiveParams {
                        tool_radius: tr,
                        stepover: cfg.stepover,
                        cut_depth: z,
                        feed_rate: cfg.feed_rate,
                        plunge_rate: cfg.plunge_rate,
                        safe_z: effective_safe_z(req),
                        tolerance: cfg.tolerance,
                        slot_clearing: cfg.slot_clearing,
                        min_cutting_radius: cfg.min_cutting_radius,
                    },
                    &|| cancel.load(Ordering::SeqCst),
                    debug,
                )
                .map_err(|_| ComputeError::Cancelled)?;
            if !tp.moves.is_empty() {
                if level_idx > 0 && !out.moves.is_empty() {
                    out.final_retract(effective_safe_z(req));
                }
                let move_start = out.moves.len();
                annotations.extend(level_annotations.into_iter().map(|annotation| {
                    rs_cam_core::adaptive::AdaptiveRuntimeAnnotation {
                        move_index: move_start + annotation.move_index,
                        event: annotation.event,
                    }
                }));
                out.moves.extend(tp.moves);
                level_slices.push(AdaptiveLevelSlice {
                    polygon_index: polygon_idx,
                    level_index: level_idx,
                    z,
                    move_start,
                    move_end_exclusive: out.moves.len(),
                });
            }
        }
    }
    Ok((out, level_slices, annotations))
}

fn run_vcarve(req: &ComputeRequest, cfg: &VCarveConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let ha = match req.tool.tool_type {
        ToolType::VBit => (req.tool.included_angle / 2.0).to_radians(),
        _ => return Err("VCarve requires V-Bit tool".into()),
    };
    let mut out = Toolpath::new();
    for p in polys {
        let tp = vcarve_toolpath(
            p,
            &VCarveParams {
                half_angle: ha,
                max_depth: cfg.max_depth,
                stepover: cfg.stepover,
                feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate,
                safe_z: effective_safe_z(req),
                tolerance: cfg.tolerance,
            },
        );
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

fn run_rest(req: &ComputeRequest, cfg: &RestConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let ptr = req.prev_tool_radius.ok_or("Previous tool not set")?;
    let depth = make_depth(cfg.depth, cfg.depth_per_pass);
    let mut out = Toolpath::new();
    for p in polys {
        let tp = depth_stepped_toolpath(&depth, effective_safe_z(req), |z| {
            rest_machining_toolpath(
                p,
                &RestParams {
                    prev_tool_radius: ptr,
                    tool_radius: tr,
                    cut_depth: z,
                    stepover: cfg.stepover,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: effective_safe_z(req),
                    angle: cfg.angle.to_radians(),
                },
            )
        });
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

fn run_inlay(req: &ComputeRequest, cfg: &InlayConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let ha = match req.tool.tool_type {
        ToolType::VBit => (req.tool.included_angle / 2.0).to_radians(),
        _ => return Err("Inlay requires V-Bit tool".into()),
    };
    let mut out = Toolpath::new();
    for p in polys {
        let r = inlay_toolpaths(
            p,
            &InlayParams {
                half_angle: ha,
                pocket_depth: cfg.pocket_depth,
                glue_gap: cfg.glue_gap,
                flat_depth: cfg.flat_depth,
                boundary_offset: cfg.boundary_offset,
                stepover: cfg.stepover,
                flat_tool_radius: cfg.flat_tool_radius,
                feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate,
                safe_z: effective_safe_z(req),
                tolerance: cfg.tolerance,
            },
        );
        out.moves.extend(r.female.moves);
        out.moves.extend(r.male.moves);
    }
    Ok(out)
}

fn run_zigzag(req: &ComputeRequest, cfg: &ZigzagConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let depth = make_depth(cfg.depth, cfg.depth_per_pass);
    let mut out = Toolpath::new();
    for p in polys {
        let tp = depth_stepped_toolpath(&depth, effective_safe_z(req), |z| {
            zigzag_toolpath(
                p,
                &ZigzagParams {
                    tool_radius: tr,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: effective_safe_z(req),
                    angle: cfg.angle.to_radians(),
                },
            )
        });
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

fn prepare_mesh_operation<'a>(
    req: &'a ComputeRequest,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<(&'a TriangleMesh, SpatialIndex, Box<dyn MillingCutter>), String> {
    let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Prepare input"));
    let _prepare_scope = debug.map(|ctx| ctx.start_span("prepare_input", "Prepare input"));
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    Ok((mesh, index, cutter))
}

#[allow(dead_code)]
fn run_dropcutter(
    req: &ComputeRequest,
    cfg: &DropCutterConfig,
    cancel: &AtomicBool,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, ComputeError> {
    let (mesh, index, cutter) =
        prepare_mesh_operation(req, phase_tracker, debug).map_err(ComputeError::Message)?;
    let grid = {
        let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Drop-cutter grid"));
        let _grid_scope = debug.map(|ctx| ctx.start_span("dropcutter_grid", "Drop-cutter grid"));
        batch_drop_cutter_with_cancel(
            mesh,
            &index,
            cutter.as_ref(),
            cfg.stepover,
            0.0,
            cfg.min_z,
            &|| cancel.load(Ordering::SeqCst),
        )
        .map_err(|_| ComputeError::Cancelled)?
    };
    let toolpath = {
        let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Rasterize grid"));
        let _raster_scope = debug.map(|ctx| ctx.start_span("rasterize_grid", "Rasterize grid"));
        raster_toolpath_from_grid(&grid, cfg.feed_rate, cfg.plunge_rate, effective_safe_z(req))
    };
    Ok(toolpath)
}

fn run_adaptive3d_annotated(
    req: &ComputeRequest,
    cfg: &Adaptive3dConfig,
    cancel: &AtomicBool,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<
    (
        Toolpath,
        Vec<rs_cam_core::adaptive3d::Adaptive3dRuntimeAnnotation>,
    ),
    ComputeError,
> {
    let (mesh, index, cutter) =
        prepare_mesh_operation(req, phase_tracker, debug).map_err(ComputeError::Message)?;
    let entry = match cfg.entry_style {
        EntryStyle::Plunge => EntryStyle3d::Plunge,
        EntryStyle::Helix => EntryStyle3d::Helix {
            radius: req.tool.diameter * 0.3,
            pitch: 2.0,
        },
        EntryStyle::Ramp => EntryStyle3d::Ramp {
            max_angle_deg: 10.0,
        },
    };
    let params = Adaptive3dParams {
        tool_radius: req.tool.diameter / 2.0,
        stepover: cfg.stepover,
        depth_per_pass: cfg.depth_per_pass,
        stock_to_leave: cfg.stock_to_leave_axial,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        tolerance: cfg.tolerance,
        min_cutting_radius: cfg.min_cutting_radius,
        stock_top_z: cfg.stock_top_z,
        entry_style: entry,
        fine_stepdown: if cfg.fine_stepdown > 0.0 {
            Some(cfg.fine_stepdown)
        } else {
            None
        },
        detect_flat_areas: cfg.detect_flat_areas,
        max_stay_down_dist: None,
        region_ordering: match cfg.region_ordering {
            crate::state::toolpath::RegionOrdering::Global => {
                rs_cam_core::adaptive3d::RegionOrdering::Global
            }
            crate::state::toolpath::RegionOrdering::ByArea => {
                rs_cam_core::adaptive3d::RegionOrdering::ByArea
            }
        },
    };
    rs_cam_core::adaptive3d::adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
        mesh,
        &index,
        cutter.as_ref(),
        &params,
        &|| cancel.load(Ordering::SeqCst),
        debug,
    )
    .map_err(|_| ComputeError::Cancelled)
}

#[allow(dead_code)]
fn run_waterline(
    req: &ComputeRequest,
    cfg: &WaterlineConfig,
    cancel: &AtomicBool,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, ComputeError> {
    let (mesh, index, cutter) =
        prepare_mesh_operation(req, phase_tracker, debug).map_err(ComputeError::Message)?;
    let params = WaterlineParams {
        sampling: cfg.sampling,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
    };
    {
        let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Waterline slices"));
        let _waterline_scope =
            debug.map(|ctx| ctx.start_span("waterline_slices", "Waterline slices"));
        waterline_toolpath_with_cancel(
            mesh,
            &index,
            cutter.as_ref(),
            cfg.start_z,
            cfg.final_z,
            cfg.z_step,
            &params,
            &|| cancel.load(Ordering::SeqCst),
        )
        .map_err(|_| ComputeError::Cancelled)
    }
}

fn run_pencil_annotated(
    req: &ComputeRequest,
    cfg: &PencilConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<rs_cam_core::pencil::PencilRuntimeAnnotation>), String> {
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = PencilParams {
        bitangency_angle: cfg.bitangency_angle,
        min_cut_length: cfg.min_cut_length,
        hookup_distance: cfg.hookup_distance,
        num_offset_passes: cfg.num_offset_passes,
        offset_stepover: cfg.offset_stepover,
        sampling: cfg.sampling,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        stock_to_leave: cfg.stock_to_leave_axial,
    };
    Ok(rs_cam_core::pencil::pencil_toolpath_structured_annotated(
        mesh,
        &index,
        cutter.as_ref(),
        &params,
        debug,
    ))
}

fn run_scallop_annotated(
    req: &ComputeRequest,
    cfg: &ScallopConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<
    (
        Toolpath,
        Vec<rs_cam_core::scallop::ScallopRuntimeAnnotation>,
    ),
    String,
> {
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = ScallopParams {
        scallop_height: cfg.scallop_height,
        tolerance: cfg.tolerance,
        direction: match cfg.direction {
            self::ScallopDirection::OutsideIn => CoreScalDir::OutsideIn,
            self::ScallopDirection::InsideOut => CoreScalDir::InsideOut,
        },
        continuous: cfg.continuous,
        slope_from: cfg.slope_from,
        slope_to: cfg.slope_to,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        stock_to_leave: cfg.stock_to_leave_axial,
    };
    Ok(rs_cam_core::scallop::scallop_toolpath_structured_annotated(
        mesh,
        &index,
        cutter.as_ref(),
        &params,
        debug,
    ))
}

fn run_steep_shallow(
    req: &ComputeRequest,
    cfg: &SteepShallowConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, String> {
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = SteepShallowParams {
        threshold_angle: cfg.threshold_angle,
        overlap_distance: cfg.overlap_distance,
        wall_clearance: cfg.wall_clearance,
        steep_first: cfg.steep_first,
        stepover: cfg.stepover,
        z_step: cfg.z_step,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave_axial,
        tolerance: cfg.tolerance,
    };
    Ok(steep_shallow_toolpath(
        mesh,
        &index,
        cutter.as_ref(),
        &params,
    ))
}

fn run_ramp_finish_annotated(
    req: &ComputeRequest,
    cfg: &RampFinishConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<
    (
        Toolpath,
        Vec<rs_cam_core::ramp_finish::RampFinishRuntimeAnnotation>,
    ),
    String,
> {
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = RampFinishParams {
        max_stepdown: cfg.max_stepdown,
        slope_from: cfg.slope_from,
        slope_to: cfg.slope_to,
        direction: match cfg.direction {
            self::CutDirection::Climb => CoreCutDir::Climb,
            self::CutDirection::Conventional => CoreCutDir::Conventional,
            self::CutDirection::BothWays => CoreCutDir::BothWays,
        },
        order_bottom_up: cfg.order_bottom_up,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave_axial,
        tolerance: cfg.tolerance,
    };
    Ok(
        rs_cam_core::ramp_finish::ramp_finish_toolpath_structured_annotated(
            mesh,
            &index,
            cutter.as_ref(),
            &params,
            debug,
        ),
    )
}

fn run_spiral_finish_annotated(
    req: &ComputeRequest,
    cfg: &SpiralFinishConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<
    (
        Toolpath,
        Vec<rs_cam_core::spiral_finish::SpiralFinishRuntimeAnnotation>,
    ),
    String,
> {
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = SpiralFinishParams {
        stepover: cfg.stepover,
        direction: match cfg.direction {
            self::SpiralDirection::InsideOut => CoreSpiralDir::InsideOut,
            self::SpiralDirection::OutsideIn => CoreSpiralDir::OutsideIn,
        },
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        stock_to_leave: cfg.stock_to_leave_axial,
    };
    Ok(
        rs_cam_core::spiral_finish::spiral_finish_toolpath_structured_annotated(
            mesh,
            &index,
            cutter.as_ref(),
            &params,
            debug,
        ),
    )
}

fn run_radial_finish(
    req: &ComputeRequest,
    cfg: &RadialFinishConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, String> {
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = RadialFinishParams {
        angular_step: cfg.angular_step,
        point_spacing: cfg.point_spacing,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        stock_to_leave: cfg.stock_to_leave_axial,
    };
    Ok(radial_finish_toolpath(
        mesh,
        &index,
        cutter.as_ref(),
        &params,
    ))
}

fn run_horizontal_finish(
    req: &ComputeRequest,
    cfg: &HorizontalFinishConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, String> {
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = HorizontalFinishParams {
        angle_threshold: cfg.angle_threshold,
        stepover: cfg.stepover,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        stock_to_leave: cfg.stock_to_leave_axial,
    };
    Ok(horizontal_finish_toolpath(
        mesh,
        &index,
        cutter.as_ref(),
        &params,
    ))
}

fn run_project_curve_annotated(
    req: &ComputeRequest,
    cfg: &ProjectCurveConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<ProjectCurveSlice>), String> {
    let polys = require_polygons(req)?;
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = ProjectCurveParams {
        depth: cfg.depth,
        point_spacing: cfg.point_spacing,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
    };
    let mut out = Toolpath::new();
    let mut slices = Vec::new();
    for (source_curve_index, p) in polys.iter().enumerate() {
        let move_start = out.moves.len();
        let tp = project_curve_toolpath(p, mesh, &index, cutter.as_ref(), &params);
        out.moves.extend(tp.moves);
        let move_end_exclusive = out.moves.len();
        if move_end_exclusive > move_start {
            slices.push(ProjectCurveSlice {
                source_curve_index: source_curve_index + 1,
                move_start,
                move_end_exclusive,
            });
        }
    }
    Ok((out, slices))
}

#[allow(dead_code)]
fn run_face(req: &ComputeRequest, cfg: &FaceConfig) -> Result<Toolpath, String> {
    let bbox = req
        .stock_bbox
        .ok_or("No stock defined for face operation")?;
    let tr = req.tool.diameter / 2.0;
    let params = FaceParams {
        tool_radius: tr,
        stepover: cfg.stepover,
        depth: cfg.depth,
        depth_per_pass: cfg.depth_per_pass,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        stock_offset: cfg.stock_offset,
        direction: match cfg.direction {
            self::FaceDirection::OneWay => CoreFaceDir::OneWay,
            self::FaceDirection::Zigzag => CoreFaceDir::Zigzag,
        },
    };
    Ok(face_toolpath(&bbox, &params))
}

fn run_trace(req: &ComputeRequest, cfg: &TraceConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let depth = make_depth(cfg.depth, cfg.depth_per_pass);
    let mut out = Toolpath::new();
    for p in polys {
        let tp = depth_stepped_toolpath(&depth, effective_safe_z(req), |z| {
            let params = TraceParams {
                tool_radius: tr,
                depth: z.abs(),
                depth_per_pass: cfg.depth_per_pass,
                feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate,
                safe_z: effective_safe_z(req),
                compensation: match cfg.compensation {
                    self::TraceCompensation::None => CoreTraceComp::None,
                    self::TraceCompensation::Left => CoreTraceComp::Left,
                    self::TraceCompensation::Right => CoreTraceComp::Right,
                },
            };
            trace_toolpath(p, &params)
        });
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

#[allow(dead_code)]
fn run_drill(req: &ComputeRequest, cfg: &DrillConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let mut holes = Vec::new();
    for p in polys {
        if p.exterior.is_empty() {
            continue;
        }
        let (sx, sy) = p
            .exterior
            .iter()
            .fold((0.0, 0.0), |(ax, ay), pt| (ax + pt.x, ay + pt.y));
        let n = p.exterior.len() as f64;
        holes.push([sx / n, sy / n]);
    }
    if holes.is_empty() {
        return Err("No hole positions found (import SVG with circles)".to_string());
    }
    let cycle = match cfg.cycle {
        self::DrillCycleType::Simple => DrillCycle::Simple,
        self::DrillCycleType::Dwell => DrillCycle::Dwell(cfg.dwell_time),
        self::DrillCycleType::Peck => DrillCycle::Peck(cfg.peck_depth),
        self::DrillCycleType::ChipBreak => {
            DrillCycle::ChipBreak(cfg.peck_depth, cfg.retract_amount)
        }
    };
    let params = DrillParams {
        depth: cfg.depth,
        cycle,
        feed_rate: cfg.feed_rate,
        safe_z: effective_safe_z(req),
        retract_z: cfg.retract_z,
    };
    Ok(drill_toolpath(&holes, &params))
}

fn run_chamfer(req: &ComputeRequest, cfg: &ChamferConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let ha = match req.tool.tool_type {
        ToolType::VBit => (req.tool.included_angle / 2.0).to_radians(),
        _ => return Err("Chamfer requires V-Bit tool".into()),
    };
    let mut out = Toolpath::new();
    for p in polys {
        let params = ChamferParams {
            chamfer_width: cfg.chamfer_width,
            tip_offset: cfg.tip_offset,
            tool_half_angle: ha,
            tool_radius: req.tool.diameter / 2.0,
            feed_rate: cfg.feed_rate,
            plunge_rate: cfg.plunge_rate,
            safe_z: effective_safe_z(req),
        };
        let tp = chamfer_toolpath(p, &params);
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

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
            scope.bind_to_toolpath(
                toolpath,
                polygon_slices.first().expect("polygon slices").move_start,
                polygon_slices
                    .last()
                    .expect("polygon slices")
                    .move_end_exclusive,
            );
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
                    if let Some(open_pass) = current_pass.take() {
                        if open_pass.pass_index == *pass_index {
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

impl SemanticToolpathOp for FaceConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let bbox = ctx.req.stock_bbox.ok_or_else(|| {
            ComputeError::Message("No stock defined for face operation".to_string())
        })?;
        let rect = Polygon2::rectangle(
            bbox.min.x - self.stock_offset,
            bbox.min.y - self.stock_offset,
            bbox.max.x + self.stock_offset,
            bbox.max.y + self.stock_offset,
        );
        let levels = if self.depth <= 0.0 {
            vec![0.0]
        } else {
            make_depth(self.depth, self.depth_per_pass).all_levels()
        };

        let op_scope = ctx
            .semantic_root
            .map(|root| root.start_item(ToolpathSemanticKind::Operation, "Face"));
        if let Some(scope) = op_scope.as_ref() {
            if let Some(debug_span_id) = ctx.core_debug_span_id {
                scope.set_debug_span_id(debug_span_id);
            }
            scope.set_param("depth", self.depth);
            scope.set_param("stepover", self.stepover);
            scope.set_param("stock_offset", self.stock_offset);
        }
        let op_ctx = op_scope.as_ref().map(|scope| scope.context());

        let mut out = Toolpath::new();
        {
            let mut writer = rs_cam_core::semantic_trace::ToolpathSemanticWriter::new(&mut out);
            for (level_idx, z) in levels.into_iter().enumerate() {
                let level_scope = op_ctx.as_ref().map(|root| {
                    root.start_item(
                        ToolpathSemanticKind::DepthLevel,
                        format!("Level {}", level_idx + 1),
                    )
                });
                if let Some(scope) = level_scope.as_ref() {
                    scope.set_param("z", z);
                    scope.set_param("level_index", level_idx + 1);
                }
                let level_start = writer.move_count();
                let lines = rs_cam_core::zigzag::zigzag_lines(
                    &rect,
                    ctx.req.tool.diameter / 2.0,
                    self.stepover,
                    0.0,
                );
                for (row_idx, line) in lines.iter().enumerate() {
                    let row_scope = level_scope.as_ref().map(|scope| {
                        scope
                            .context()
                            .start_item(ToolpathSemanticKind::Row, format!("Row {}", row_idx + 1))
                    });
                    if let Some(scope) = row_scope.as_ref() {
                        scope.set_param("row_index", row_idx + 1);
                        scope.set_param("z", z);
                    }
                    append_toolpath(
                        &mut writer,
                        row_scope.as_ref(),
                        line_toolpath(
                            line[0],
                            line[1],
                            z,
                            effective_safe_z(ctx.req),
                            self.plunge_rate,
                            self.feed_rate,
                        ),
                    );
                }
                if let Some(scope) = level_scope.as_ref() {
                    writer.bind_scope_to_current_range(scope, level_start);
                }
            }
        }
        if let Some(scope) = op_scope.as_ref() {
            scope.bind_to_toolpath(&out, 0, out.moves.len());
        }
        Ok(out)
    }
}

impl SemanticToolpathOp for PocketConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let polys = require_polygons(ctx.req).map_err(ComputeError::Message)?;
        let op_scope = ctx
            .semantic_root
            .map(|root| root.start_item(ToolpathSemanticKind::Operation, "Pocket"));
        if let Some(scope) = op_scope.as_ref() {
            if let Some(debug_span_id) = ctx.core_debug_span_id {
                scope.set_debug_span_id(debug_span_id);
            }
            scope.set_param("pattern", format!("{:?}", self.pattern));
            scope.set_param("depth", self.depth);
            scope.set_param("stepover", self.stepover);
        }
        let op_ctx = op_scope.as_ref().map(|scope| scope.context());
        let depth =
            make_depth_with_finishing(self.depth, self.depth_per_pass, self.finishing_passes);
        let levels = depth.all_levels();
        let reverse = self.climb;
        let mut out = Toolpath::new();
        {
            let mut writer = rs_cam_core::semantic_trace::ToolpathSemanticWriter::new(&mut out);
            for (poly_idx, polygon) in polys.iter().enumerate() {
                let poly_scope = op_ctx.as_ref().map(|ctx| {
                    ctx.start_item(
                        ToolpathSemanticKind::Region,
                        format!("Polygon {}", poly_idx + 1),
                    )
                });
                if let Some(scope) = poly_scope.as_ref() {
                    scope.set_param("polygon_index", poly_idx + 1);
                }
                let poly_ctx = poly_scope.as_ref().map(|scope| scope.context());
                for (level_idx, z) in levels.iter().copied().enumerate() {
                    let level_scope = poly_ctx.as_ref().map(|ctx| {
                        ctx.start_item(
                            ToolpathSemanticKind::DepthLevel,
                            format!("Level {}", level_idx + 1),
                        )
                    });
                    if let Some(scope) = level_scope.as_ref() {
                        scope.set_param("z", z);
                        scope.set_param("level_index", level_idx + 1);
                    }
                    let level_start = writer.move_count();
                    match self.pattern {
                        PocketPattern::Contour => {
                            let contours = rs_cam_core::pocket::pocket_contours(
                                polygon,
                                ctx.req.tool.diameter / 2.0,
                                self.stepover,
                            );
                            let is_finish = self.finishing_passes > 0
                                && level_idx >= levels.len().saturating_sub(self.finishing_passes);
                            for (contour_idx, contour) in contours.iter().enumerate() {
                                let contour_scope = level_scope.as_ref().map(|scope| {
                                    scope.context().start_item(
                                        if is_finish {
                                            ToolpathSemanticKind::FinishPass
                                        } else {
                                            ToolpathSemanticKind::Contour
                                        },
                                        if is_finish {
                                            format!("Finish contour {}", contour_idx + 1)
                                        } else {
                                            format!("Contour {}", contour_idx + 1)
                                        },
                                    )
                                });
                                if let Some(scope) = contour_scope.as_ref() {
                                    scope.set_param("contour_index", contour_idx + 1);
                                    scope.set_param("z", z);
                                    scope.set_param("climb", self.climb);
                                }
                                append_toolpath(
                                    &mut writer,
                                    contour_scope.as_ref(),
                                    contour_toolpath(
                                        contour,
                                        z,
                                        effective_safe_z(ctx.req),
                                        self.plunge_rate,
                                        self.feed_rate,
                                        reverse,
                                    ),
                                );
                            }
                        }
                        PocketPattern::Zigzag => {
                            let lines = rs_cam_core::zigzag::zigzag_lines(
                                polygon,
                                ctx.req.tool.diameter / 2.0,
                                self.stepover,
                                self.angle.to_radians(),
                            );
                            let is_finish = self.finishing_passes > 0
                                && level_idx >= levels.len().saturating_sub(self.finishing_passes);
                            for (row_idx, line) in lines.iter().enumerate() {
                                let row_scope = level_scope.as_ref().map(|scope| {
                                    scope.context().start_item(
                                        if is_finish {
                                            ToolpathSemanticKind::FinishPass
                                        } else {
                                            ToolpathSemanticKind::Raster
                                        },
                                        if is_finish {
                                            format!("Finish raster {}", row_idx + 1)
                                        } else {
                                            format!("Raster {}", row_idx + 1)
                                        },
                                    )
                                });
                                if let Some(scope) = row_scope.as_ref() {
                                    scope.set_param("row_index", row_idx + 1);
                                    scope.set_param("angle_deg", self.angle);
                                    scope.set_param("z", z);
                                }
                                append_toolpath(
                                    &mut writer,
                                    row_scope.as_ref(),
                                    line_toolpath(
                                        line[0],
                                        line[1],
                                        z,
                                        effective_safe_z(ctx.req),
                                        self.plunge_rate,
                                        self.feed_rate,
                                    ),
                                );
                            }
                        }
                    }
                    if let Some(scope) = level_scope.as_ref() {
                        writer.bind_scope_to_current_range(scope, level_start);
                    }
                }
            }
        }
        if let Some(scope) = op_scope.as_ref() {
            scope.bind_to_toolpath(&out, 0, out.moves.len());
        }
        Ok(out)
    }
}

impl SemanticToolpathOp for ProfileConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let polys = require_polygons(ctx.req).map_err(ComputeError::Message)?;
        let tp = run_profile(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope =
            annotate_operation_scope(ctx.semantic_root, ctx.core_debug_span_id, "Profile", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("side", format!("{:?}", self.side));
            scope.set_param("tab_count", self.tab_count);
            scope.set_param("climb", self.climb);
            scope.set_param("compensation", format!("{:?}", self.compensation));
        }
        if let Some(op_ctx) = semantic_child_context(op_scope.as_ref()) {
            let mut run_iter = cutting_runs(&tp).into_iter();
            let levels =
                make_depth_with_finishing(self.depth, self.depth_per_pass, self.finishing_passes)
                    .all_levels();
            let total_levels = levels.len();
            for (poly_idx, _) in polys.iter().enumerate() {
                let poly_scope = if polys.len() > 1 {
                    let scope = op_ctx.start_item(
                        ToolpathSemanticKind::Chain,
                        format!("Chain {}", poly_idx + 1),
                    );
                    scope.set_param("chain_index", poly_idx + 1);
                    Some(scope)
                } else {
                    None
                };
                let poly_ctx = poly_scope
                    .as_ref()
                    .map(|scope| scope.context())
                    .unwrap_or_else(|| op_ctx.clone());
                for (level_idx, z) in levels.iter().copied().enumerate() {
                    let level_scope = poly_ctx.start_item(
                        ToolpathSemanticKind::DepthLevel,
                        format!("Level {}", level_idx + 1),
                    );
                    level_scope.set_param("level_index", level_idx + 1);
                    level_scope.set_param("z", z);
                    if let Some(run) = run_iter.next() {
                        let is_finish = self.finishing_passes > 0
                            && level_idx >= total_levels.saturating_sub(self.finishing_passes);
                        let kind = if is_finish {
                            ToolpathSemanticKind::FinishPass
                        } else {
                            ToolpathSemanticKind::Contour
                        };
                        let contour_scope = level_scope.context().start_item(
                            kind,
                            if is_finish {
                                format!("Finish contour {}", poly_idx + 1)
                            } else {
                                format!("Contour {}", poly_idx + 1)
                            },
                        );
                        contour_scope.set_param("chain_index", poly_idx + 1);
                        contour_scope.set_param("level_index", level_idx + 1);
                        contour_scope.set_param("z", z);
                        contour_scope.set_param("side", format!("{:?}", self.side));
                        contour_scope.set_param("climb", self.climb);
                        contour_scope.set_param("compensation", format!("{:?}", self.compensation));
                        contour_scope.set_param("has_tabs", self.tab_count > 0);
                        bind_scope_to_run(&contour_scope, &tp, &run);
                        level_scope.bind_to_toolpath(&tp, run.move_start, run.move_end_exclusive);
                    }
                }
            }
        }
        Ok(tp)
    }
}

impl SemanticToolpathOp for AdaptiveConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let (tp, level_slices, annotations) =
            run_adaptive_annotated(ctx.req, self, ctx.cancel, ctx.debug_root)?;
        let op_scope =
            annotate_operation_scope(ctx.semantic_root, ctx.core_debug_span_id, "Adaptive", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("stepover", self.stepover);
            scope.set_param("slot_clearing", self.slot_clearing);
            scope.set_param("tolerance", self.tolerance);
        }
        annotate_adaptive_runtime_semantics(op_scope.as_ref(), &tp, &level_slices, &annotations);
        Ok(tp)
    }
}

impl SemanticToolpathOp for VCarveConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let polys = require_polygons(ctx.req).map_err(ComputeError::Message)?;
        let tp = run_vcarve(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope =
            annotate_operation_scope(ctx.semantic_root, ctx.core_debug_span_id, "VCarve", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("max_depth", self.max_depth);
            scope.set_param("stepover", self.stepover);
            scope.set_param("tolerance", self.tolerance);
        }
        if let Some(op_ctx) = semantic_child_context(op_scope.as_ref()) {
            let runs = cutting_runs(&tp);
            let mut run_iter = runs.iter();
            for (poly_idx, _) in polys.iter().enumerate() {
                let curve_scope = op_ctx.start_item(
                    ToolpathSemanticKind::Curve,
                    format!("Source curve {}", poly_idx + 1),
                );
                curve_scope.set_param("source_curve_index", poly_idx + 1);
                let curve_ctx = curve_scope.context();
                while let Some(run) = run_iter.next() {
                    let kind = if run.constant_z && run.closed_loop {
                        ToolpathSemanticKind::Contour
                    } else if run.constant_z {
                        ToolpathSemanticKind::FinishPass
                    } else {
                        ToolpathSemanticKind::Centerline
                    };
                    let label = match kind {
                        ToolpathSemanticKind::Centerline => "Centerline".to_string(),
                        ToolpathSemanticKind::FinishPass => "Finish pass".to_string(),
                        _ => "Contour".to_string(),
                    };
                    let scope = curve_ctx.start_item(kind, label);
                    scope.set_param("source_curve_index", poly_idx + 1);
                    scope.set_param("z_min", run.z_min);
                    scope.set_param("z_max", run.z_max);
                    bind_scope_to_run(&scope, &tp, run);
                    if run.closed_loop {
                        break;
                    }
                }
            }
        }
        Ok(tp)
    }
}

impl SemanticToolpathOp for RestConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let polys = require_polygons(ctx.req).map_err(ComputeError::Message)?;
        let tp = run_rest(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(
            ctx.semantic_root,
            ctx.core_debug_span_id,
            "Rest machining",
            &tp,
        );
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("depth", self.depth);
            scope.set_param("stepover", self.stepover);
            scope.set_param("angle_deg", self.angle);
            if let Some(prev_tool_radius) = ctx.req.prev_tool_radius {
                scope.set_param("previous_tool_radius", prev_tool_radius);
            }
        }
        if let Some(op_ctx) = semantic_child_context(op_scope.as_ref()) {
            let tr = ctx.req.tool.diameter / 2.0;
            let ptr = ctx.req.prev_tool_radius.unwrap_or_default();
            let depth = make_depth(self.depth, self.depth_per_pass);
            let actual_runs = cutting_runs(&tp);
            let mut run_cursor = 0usize;
            for (poly_idx, polygon) in polys.iter().enumerate() {
                let region_scope = op_ctx.start_item(
                    ToolpathSemanticKind::Region,
                    format!("Region {}", poly_idx + 1),
                );
                region_scope.set_param("region_index", poly_idx + 1);
                let region_ctx = region_scope.context();
                for (level_idx, z) in depth.all_levels().into_iter().enumerate() {
                    let level_scope = region_ctx.start_item(
                        ToolpathSemanticKind::DepthLevel,
                        format!("Level {}", level_idx + 1),
                    );
                    level_scope.set_param("level_index", level_idx + 1);
                    level_scope.set_param("z", z);
                    let expected_tp = rest_machining_toolpath(
                        polygon,
                        &RestParams {
                            prev_tool_radius: ptr,
                            tool_radius: tr,
                            cut_depth: z,
                            stepover: self.stepover,
                            feed_rate: self.feed_rate,
                            plunge_rate: self.plunge_rate,
                            safe_z: effective_safe_z(ctx.req),
                            angle: self.angle.to_radians(),
                        },
                    );
                    let expected_runs = cutting_runs(&expected_tp);
                    for pass_idx in 0..expected_runs.len() {
                        if let Some(run) = actual_runs.get(run_cursor) {
                            let scope = level_scope.context().start_item(
                                ToolpathSemanticKind::Pass,
                                format!("Rest pass {}", pass_idx + 1),
                            );
                            scope.set_param("region_index", poly_idx + 1);
                            scope.set_param("level_index", level_idx + 1);
                            scope.set_param("pass_index", pass_idx + 1);
                            if let Some(prev_tool_radius) = ctx.req.prev_tool_radius {
                                scope.set_param("previous_tool_radius", prev_tool_radius);
                            }
                            bind_scope_to_run(&scope, &tp, run);
                            run_cursor += 1;
                        }
                    }
                }
            }
        }
        Ok(tp)
    }
}

impl SemanticToolpathOp for InlayConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let polys = require_polygons(ctx.req).map_err(ComputeError::Message)?;
        let tp = run_inlay(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope =
            annotate_operation_scope(ctx.semantic_root, ctx.core_debug_span_id, "Inlay", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("pocket_depth", self.pocket_depth);
            scope.set_param("glue_gap", self.glue_gap);
            scope.set_param("flat_depth", self.flat_depth);
        }
        if let Some(op_ctx) = semantic_child_context(op_scope.as_ref()) {
            let ha = match ctx.req.tool.tool_type {
                ToolType::VBit => (ctx.req.tool.included_angle / 2.0).to_radians(),
                _ => return Err(ComputeError::Message("Inlay requires V-Bit tool".into())),
            };
            let actual_runs = cutting_runs(&tp);
            let mut run_cursor = 0usize;
            for (poly_idx, polygon) in polys.iter().enumerate() {
                let region_scope = op_ctx.start_item(
                    ToolpathSemanticKind::Curve,
                    format!("Source curve {}", poly_idx + 1),
                );
                region_scope.set_param("source_curve_index", poly_idx + 1);
                let region_ctx = region_scope.context();
                let generated = inlay_toolpaths(
                    polygon,
                    &InlayParams {
                        half_angle: ha,
                        pocket_depth: self.pocket_depth,
                        glue_gap: self.glue_gap,
                        flat_depth: self.flat_depth,
                        boundary_offset: self.boundary_offset,
                        stepover: self.stepover,
                        flat_tool_radius: self.flat_tool_radius,
                        feed_rate: self.feed_rate,
                        plunge_rate: self.plunge_rate,
                        safe_z: effective_safe_z(ctx.req),
                        tolerance: self.tolerance,
                    },
                );
                for (label, kind_runs) in [
                    ("Pocket contour", cutting_runs(&generated.female)),
                    ("Male contour", cutting_runs(&generated.male)),
                ] {
                    for (run_idx, _) in kind_runs.iter().enumerate() {
                        if let Some(run) = actual_runs.get(run_cursor) {
                            let scope = region_ctx.start_item(
                                ToolpathSemanticKind::Contour,
                                format!("{label} {}", run_idx + 1),
                            );
                            scope.set_param("source_curve_index", poly_idx + 1);
                            bind_scope_to_run(&scope, &tp, run);
                            run_cursor += 1;
                        }
                    }
                }
            }
        }
        Ok(tp)
    }
}

impl SemanticToolpathOp for ZigzagConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let polys = require_polygons(ctx.req).map_err(ComputeError::Message)?;
        let tp = run_zigzag(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope =
            annotate_operation_scope(ctx.semantic_root, ctx.core_debug_span_id, "Zigzag", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("depth", self.depth);
            scope.set_param("stepover", self.stepover);
            scope.set_param("angle_deg", self.angle);
        }
        if let Some(op_ctx) = semantic_child_context(op_scope.as_ref()) {
            let runs = cutting_runs(&tp);
            let mut run_cursor = 0usize;
            let depth = make_depth(self.depth, self.depth_per_pass);
            for (poly_idx, polygon) in polys.iter().enumerate() {
                let region_scope = op_ctx.start_item(
                    ToolpathSemanticKind::Region,
                    format!("Region {}", poly_idx + 1),
                );
                let region_ctx = region_scope.context();
                for (level_idx, z) in depth.all_levels().into_iter().enumerate() {
                    let level_scope = region_ctx.start_item(
                        ToolpathSemanticKind::DepthLevel,
                        format!("Level {}", level_idx + 1),
                    );
                    level_scope.set_param("level_index", level_idx + 1);
                    level_scope.set_param("z", z);
                    let lines = rs_cam_core::zigzag::zigzag_lines(
                        polygon,
                        ctx.req.tool.diameter / 2.0,
                        self.stepover,
                        self.angle.to_radians(),
                    );
                    for (row_idx, _) in lines.iter().enumerate() {
                        if let Some(run) = runs.get(run_cursor) {
                            let scope = level_scope.context().start_item(
                                ToolpathSemanticKind::Raster,
                                format!("Raster {}", row_idx + 1),
                            );
                            scope.set_param("row_index", row_idx + 1);
                            scope.set_param("angle_deg", self.angle);
                            bind_scope_to_run(&scope, &tp, run);
                            run_cursor += 1;
                        }
                    }
                }
            }
        }
        Ok(tp)
    }
}

impl SemanticToolpathOp for TraceConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let polys = require_polygons(ctx.req).map_err(ComputeError::Message)?;
        let tp = run_trace(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope =
            annotate_operation_scope(ctx.semantic_root, ctx.core_debug_span_id, "Trace", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("depth", self.depth);
            scope.set_param("compensation", format!("{:?}", self.compensation));
        }
        if let Some(op_ctx) = semantic_child_context(op_scope.as_ref()) {
            let runs = cutting_runs(&tp);
            let mut run_iter = runs.iter();
            let levels = make_depth(self.depth, self.depth_per_pass).all_levels();
            for (poly_idx, _) in polys.iter().enumerate() {
                let curve_scope = op_ctx.start_item(
                    ToolpathSemanticKind::Curve,
                    format!("Source curve {}", poly_idx + 1),
                );
                curve_scope.set_param("source_curve_index", poly_idx + 1);
                let curve_ctx = curve_scope.context();
                for (level_idx, z) in levels.iter().copied().enumerate() {
                    let level_scope = curve_ctx.start_item(
                        ToolpathSemanticKind::DepthLevel,
                        format!("Level {}", level_idx + 1),
                    );
                    level_scope.set_param("level_index", level_idx + 1);
                    level_scope.set_param("z", z);
                    if let Some(run) = run_iter.next() {
                        let scope = level_scope.context().start_item(
                            ToolpathSemanticKind::Contour,
                            format!("Trace contour {}", poly_idx + 1),
                        );
                        scope.set_param("source_curve_index", poly_idx + 1);
                        scope.set_param("compensation", format!("{:?}", self.compensation));
                        bind_scope_to_run(&scope, &tp, run);
                        level_scope.bind_to_toolpath(&tp, run.move_start, run.move_end_exclusive);
                    }
                }
            }
        }
        Ok(tp)
    }
}

impl SemanticToolpathOp for DrillConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let polys = require_polygons(ctx.req).map_err(ComputeError::Message)?;
        let mut holes = Vec::new();
        for p in polys {
            if p.exterior.is_empty() {
                continue;
            }
            let (sx, sy) = p
                .exterior
                .iter()
                .fold((0.0, 0.0), |(ax, ay), pt| (ax + pt.x, ay + pt.y));
            let n = p.exterior.len() as f64;
            holes.push([sx / n, sy / n]);
        }
        if holes.is_empty() {
            return Err(ComputeError::Message(
                "No hole positions found (import SVG with circles)".to_string(),
            ));
        }
        let cycle = match self.cycle {
            self::DrillCycleType::Simple => DrillCycle::Simple,
            self::DrillCycleType::Dwell => DrillCycle::Dwell(self.dwell_time),
            self::DrillCycleType::Peck => DrillCycle::Peck(self.peck_depth),
            self::DrillCycleType::ChipBreak => {
                DrillCycle::ChipBreak(self.peck_depth, self.retract_amount)
            }
        };
        let params = DrillParams {
            depth: self.depth,
            cycle,
            feed_rate: self.feed_rate,
            safe_z: effective_safe_z(ctx.req),
            retract_z: self.retract_z,
        };
        let op_scope = ctx
            .semantic_root
            .map(|root| root.start_item(ToolpathSemanticKind::Operation, "Drill"));
        if let Some(scope) = op_scope.as_ref() {
            if let Some(debug_span_id) = ctx.core_debug_span_id {
                scope.set_debug_span_id(debug_span_id);
            }
            scope.set_param("cycle", format!("{:?}", self.cycle));
            scope.set_param("depth", self.depth);
        }
        let op_ctx = op_scope.as_ref().map(|scope| scope.context());
        let mut out = Toolpath::new();
        {
            let mut writer = rs_cam_core::semantic_trace::ToolpathSemanticWriter::new(&mut out);
            for (hole_idx, hole) in holes.iter().enumerate() {
                let hole_scope = op_ctx.as_ref().map(|ctx| {
                    ctx.start_item(ToolpathSemanticKind::Hole, format!("Hole {}", hole_idx + 1))
                });
                if let Some(scope) = hole_scope.as_ref() {
                    scope.set_param("hole_index", hole_idx + 1);
                    scope.set_param("x", hole[0]);
                    scope.set_param("y", hole[1]);
                    scope.set_param("cycle", format!("{:?}", self.cycle));
                }
                let start = writer.move_count();
                let hole_tp = drill_toolpath(&[*hole], &params);
                append_toolpath(&mut writer, hole_scope.as_ref(), hole_tp.clone());
                if let Some(scope) = hole_scope.as_ref() {
                    let hole_runs = cutting_runs(&hole_tp);
                    for (cycle_idx, run) in hole_runs.iter().enumerate() {
                        let cycle_scope = scope.context().start_item(
                            ToolpathSemanticKind::Cycle,
                            format!("Cycle {}", cycle_idx + 1),
                        );
                        cycle_scope.set_param("hole_index", hole_idx + 1);
                        cycle_scope.set_param("cycle_index", cycle_idx + 1);
                        cycle_scope.set_param("cycle", format!("{:?}", self.cycle));
                        bind_scope_to_offset_run(&cycle_scope, writer.toolpath(), start, run);
                    }
                }
            }
        }
        if let Some(scope) = op_scope.as_ref() {
            scope.bind_to_toolpath(&out, 0, out.moves.len());
        }
        Ok(out)
    }
}

impl SemanticToolpathOp for ChamferConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let polys = require_polygons(ctx.req).map_err(ComputeError::Message)?;
        let tp = run_chamfer(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope =
            annotate_operation_scope(ctx.semantic_root, ctx.core_debug_span_id, "Chamfer", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("chamfer_width", self.chamfer_width);
            scope.set_param("tip_offset", self.tip_offset);
        }
        if let Some(op_ctx) = semantic_child_context(op_scope.as_ref()) {
            let runs = cutting_runs(&tp);
            for (poly_idx, run) in polys.iter().enumerate().zip(runs.iter()) {
                let scope = op_ctx.start_item(
                    ToolpathSemanticKind::Contour,
                    format!("Source edge {}", poly_idx.0 + 1),
                );
                scope.set_param("source_curve_index", poly_idx.0 + 1);
                scope.set_param("chamfer_width", self.chamfer_width);
                scope.set_param("tip_offset", self.tip_offset);
                bind_scope_to_run(&scope, &tp, run);
            }
        }
        Ok(tp)
    }
}

impl SemanticToolpathOp for AlignmentPinDrillConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        if self.holes.is_empty() {
            return Err(ComputeError::Message(
                "No alignment pin positions defined".to_string(),
            ));
        }
        let stock_z = ctx
            .req
            .stock_bbox
            .as_ref()
            .map(|b| b.max.z - b.min.z)
            .unwrap_or(25.0);
        let depth = stock_z + self.spoilboard_penetration;
        let cycle = match self.cycle {
            DrillCycleType::Simple => DrillCycle::Simple,
            DrillCycleType::Dwell => DrillCycle::Dwell(0.5),
            DrillCycleType::Peck => DrillCycle::Peck(self.peck_depth),
            DrillCycleType::ChipBreak => DrillCycle::ChipBreak(self.peck_depth, 0.5),
        };
        let params = DrillParams {
            depth,
            cycle,
            feed_rate: self.feed_rate,
            safe_z: effective_safe_z(ctx.req),
            retract_z: self.retract_z,
        };
        Ok(drill_toolpath(&self.holes, &params))
    }
}

impl SemanticToolpathOp for DropCutterConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let (mesh, index, cutter) =
            prepare_mesh_operation(ctx.req, ctx.phase_tracker, ctx.debug_root)
                .map_err(ComputeError::Message)?;
        let grid = {
            let _phase_scope = ctx
                .phase_tracker
                .map(|tracker| tracker.start_phase("Drop-cutter grid"));
            let _grid_scope = ctx
                .debug_root
                .map(|ctx| ctx.start_span("dropcutter_grid", "Drop-cutter grid"));
            batch_drop_cutter_with_cancel(
                mesh,
                &index,
                cutter.as_ref(),
                self.stepover,
                0.0,
                self.min_z,
                &|| ctx.cancel.load(Ordering::SeqCst),
            )
            .map_err(|_| ComputeError::Cancelled)?
        };
        let op_scope = ctx
            .semantic_root
            .map(|root| root.start_item(ToolpathSemanticKind::Operation, "3D Finish"));
        if let Some(scope) = op_scope.as_ref() {
            if let Some(debug_span_id) = ctx.core_debug_span_id {
                scope.set_debug_span_id(debug_span_id);
            }
            scope.set_param("stepover", self.stepover);
            scope.set_param("min_z", self.min_z);
        }
        let op_ctx = op_scope.as_ref().map(|scope| scope.context());
        let mut out = Toolpath::new();
        {
            let _phase_scope = ctx
                .phase_tracker
                .map(|tracker| tracker.start_phase("Rasterize grid"));
            let _raster_scope = ctx
                .debug_root
                .map(|ctx| ctx.start_span("rasterize_grid", "Rasterize grid"));
            let mut writer = rs_cam_core::semantic_trace::ToolpathSemanticWriter::new(&mut out);
            for row in 0..grid.rows {
                let cols: Vec<usize> = if row % 2 == 0 {
                    (0..grid.cols).collect()
                } else {
                    (0..grid.cols).rev().collect()
                };
                if cols.is_empty() {
                    continue;
                }
                let start_pt = grid.get(row, cols[0]);
                let row_scope = op_ctx.as_ref().map(|ctx| {
                    ctx.start_item(ToolpathSemanticKind::Row, format!("Row {}", row + 1))
                });
                if let Some(scope) = row_scope.as_ref() {
                    scope.set_param("row_index", row + 1);
                }
                let mut row_tp = Toolpath::new();
                row_tp.rapid_to(P3::new(start_pt.x, start_pt.y, effective_safe_z(ctx.req)));
                row_tp.feed_to(start_pt.position(), self.plunge_rate);
                for &col in cols.iter().skip(1) {
                    row_tp.feed_to(grid.get(row, col).position(), self.feed_rate);
                }
                let last_pt = grid.get(row, *cols.last().expect("row has points"));
                row_tp.rapid_to(P3::new(last_pt.x, last_pt.y, effective_safe_z(ctx.req)));
                append_toolpath(&mut writer, row_scope.as_ref(), row_tp);
            }
        }
        if let Some(scope) = op_scope.as_ref() {
            scope.bind_to_toolpath(&out, 0, out.moves.len());
        }
        Ok(out)
    }
}

impl SemanticToolpathOp for Adaptive3dConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let (tp, annotations) =
            run_adaptive3d_annotated(ctx.req, self, ctx.cancel, ctx.phase_tracker, ctx.debug_root)?;
        let op_scope =
            annotate_operation_scope(ctx.semantic_root, ctx.core_debug_span_id, "3D Rough", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("stepover", self.stepover);
            scope.set_param("depth_per_pass", self.depth_per_pass);
            scope.set_param("stock_to_leave", self.stock_to_leave_axial);
            scope.set_param("detect_flat_areas", self.detect_flat_areas);
            scope.set_param(
                "region_ordering",
                match self.region_ordering {
                    crate::state::toolpath::RegionOrdering::Global => "global",
                    crate::state::toolpath::RegionOrdering::ByArea => "by_area",
                },
            );
        }
        annotate_adaptive3d_runtime_semantics(
            op_scope.as_ref(),
            &tp,
            &annotations,
            self.detect_flat_areas,
            self.region_ordering,
        );
        Ok(tp)
    }
}

impl SemanticToolpathOp for WaterlineConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let (mesh, index, cutter) =
            prepare_mesh_operation(ctx.req, ctx.phase_tracker, ctx.debug_root)
                .map_err(ComputeError::Message)?;
        let params = WaterlineParams {
            sampling: self.sampling,
            feed_rate: self.feed_rate,
            plunge_rate: self.plunge_rate,
            safe_z: effective_safe_z(ctx.req),
        };
        let op_scope = ctx
            .semantic_root
            .map(|root| root.start_item(ToolpathSemanticKind::Operation, "Waterline"));
        if let Some(scope) = op_scope.as_ref() {
            if let Some(debug_span_id) = ctx.core_debug_span_id {
                scope.set_debug_span_id(debug_span_id);
            }
            scope.set_param("start_z", self.start_z);
            scope.set_param("final_z", self.final_z);
            scope.set_param("z_step", self.z_step);
        }
        let op_ctx = op_scope.as_ref().map(|scope| scope.context());
        let mut out = Toolpath::new();
        {
            let _phase_scope = ctx
                .phase_tracker
                .map(|tracker| tracker.start_phase("Waterline slices"));
            let _waterline_scope = ctx
                .debug_root
                .map(|ctx| ctx.start_span("waterline_slices", "Waterline slices"));
            let mut writer = rs_cam_core::semantic_trace::ToolpathSemanticWriter::new(&mut out);
            let mut z = self.start_z;
            let mut level_idx = 0usize;
            while z >= self.final_z - 1e-10 {
                let contours = rs_cam_core::waterline::waterline_contours_with_cancel(
                    mesh,
                    &index,
                    cutter.as_ref(),
                    z,
                    self.sampling,
                    &|| ctx.cancel.load(Ordering::SeqCst),
                )
                .map_err(|_| ComputeError::Cancelled)?;
                let level_scope = op_ctx.as_ref().map(|ctx| {
                    ctx.start_item(
                        ToolpathSemanticKind::Slice,
                        format!("Slice {}", level_idx + 1),
                    )
                });
                if let Some(scope) = level_scope.as_ref() {
                    scope.set_param("z", z);
                    scope.set_param("slice_index", level_idx + 1);
                }
                let level_start = writer.move_count();
                for (contour_idx, contour) in contours.iter().enumerate() {
                    if contour.len() < 3 {
                        continue;
                    }
                    let contour_scope = level_scope.as_ref().map(|scope| {
                        scope.context().start_item(
                            ToolpathSemanticKind::Contour,
                            format!("Contour {}", contour_idx + 1),
                        )
                    });
                    if let Some(scope) = contour_scope.as_ref() {
                        scope.set_param("contour_index", contour_idx + 1);
                        scope.set_param("z", z);
                    }
                    let mut contour_tp = Toolpath::new();
                    contour_tp.rapid_to(P3::new(contour[0].x, contour[0].y, params.safe_z));
                    contour_tp.feed_to(P3::new(contour[0].x, contour[0].y, z), params.plunge_rate);
                    for pt in contour.iter().skip(1) {
                        contour_tp.feed_to(P3::new(pt.x, pt.y, z), params.feed_rate);
                    }
                    contour_tp.feed_to(P3::new(contour[0].x, contour[0].y, z), params.feed_rate);
                    contour_tp.rapid_to(P3::new(contour[0].x, contour[0].y, params.safe_z));
                    append_toolpath(&mut writer, contour_scope.as_ref(), contour_tp);
                }
                if let Some(scope) = level_scope.as_ref() {
                    writer.bind_scope_to_current_range(scope, level_start);
                }
                z -= self.z_step;
                level_idx += 1;
            }
        }
        if let Some(scope) = op_scope.as_ref() {
            scope.bind_to_toolpath(&out, 0, out.moves.len());
        }
        Ok(out)
    }
}

impl SemanticToolpathOp for PencilConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let (tp, annotations) =
            run_pencil_annotated(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
                .map_err(ComputeError::Message)?;
        let op_scope =
            annotate_operation_scope(ctx.semantic_root, ctx.core_debug_span_id, "Pencil", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("bitangency_angle", self.bitangency_angle);
            scope.set_param("offset_passes", self.num_offset_passes);
            scope.set_param("offset_stepover", self.offset_stepover);
        }
        annotate_pencil_runtime_semantics(op_scope.as_ref(), &tp, &annotations);
        Ok(tp)
    }
}

impl SemanticToolpathOp for ScallopConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let (tp, annotations) =
            run_scallop_annotated(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
                .map_err(ComputeError::Message)?;
        let op_scope =
            annotate_operation_scope(ctx.semantic_root, ctx.core_debug_span_id, "Scallop", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("scallop_height", self.scallop_height);
            scope.set_param("continuous", self.continuous);
            scope.set_param("direction", format!("{:?}", self.direction));
        }
        annotate_scallop_runtime_semantics(
            op_scope.as_ref(),
            &tp,
            &annotations,
            &self.direction,
            self.continuous,
        );
        Ok(tp)
    }
}

impl SemanticToolpathOp for SteepShallowConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_steep_shallow(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
            .map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(
            ctx.semantic_root,
            ctx.core_debug_span_id,
            "Steep/Shallow",
            &tp,
        );
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("threshold_angle", self.threshold_angle);
            scope.set_param("overlap_distance", self.overlap_distance);
            scope.set_param("steep_first", self.steep_first);
        }
        if let Some(ctx) = op_scope.as_ref().map(|scope| scope.context()) {
            let steep_scope = ctx.start_item(ToolpathSemanticKind::Region, "Steep contours");
            steep_scope.set_param("kind", "steep");
            let shallow_scope = ctx.start_item(ToolpathSemanticKind::Region, "Shallow raster");
            shallow_scope.set_param("kind", "shallow");
            for (run_idx, run) in cutting_runs(&tp).iter().enumerate() {
                let kind = if run.closed_loop && run.constant_z {
                    ToolpathSemanticKind::Contour
                } else {
                    ToolpathSemanticKind::Raster
                };
                let (parent_ctx, label) = if kind == ToolpathSemanticKind::Contour {
                    (
                        steep_scope.context(),
                        format!("Steep contour {}", run_idx + 1),
                    )
                } else {
                    (
                        shallow_scope.context(),
                        format!("Shallow pass {}", run_idx + 1),
                    )
                };
                let scope = parent_ctx.start_item(kind, label);
                scope.set_param("run_index", run_idx + 1);
                bind_scope_to_run(&scope, &tp, run);
            }
        }
        Ok(tp)
    }
}

impl SemanticToolpathOp for RampFinishConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let (tp, annotations) =
            run_ramp_finish_annotated(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
                .map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(
            ctx.semantic_root,
            ctx.core_debug_span_id,
            "Ramp finish",
            &tp,
        );
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("max_stepdown", self.max_stepdown);
            scope.set_param("direction", format!("{:?}", self.direction));
        }
        annotate_ramp_finish_runtime_semantics(op_scope.as_ref(), &tp, &annotations);
        Ok(tp)
    }
}

impl SemanticToolpathOp for SpiralFinishConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let (tp, annotations) =
            run_spiral_finish_annotated(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
                .map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(
            ctx.semantic_root,
            ctx.core_debug_span_id,
            "Spiral finish",
            &tp,
        );
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("stepover", self.stepover);
            scope.set_param("direction", format!("{:?}", self.direction));
        }
        annotate_spiral_finish_runtime_semantics(
            op_scope.as_ref(),
            &tp,
            &annotations,
            &self.direction,
        );
        Ok(tp)
    }
}

impl SemanticToolpathOp for RadialFinishConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_radial_finish(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
            .map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(
            ctx.semantic_root,
            ctx.core_debug_span_id,
            "Radial finish",
            &tp,
        );
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("angular_step", self.angular_step);
            scope.set_param("point_spacing", self.point_spacing);
        }
        if let Some(mesh) = ctx.req.mesh.as_ref() {
            let center_x = (mesh.bbox.min.x + mesh.bbox.max.x) * 0.5;
            let center_y = (mesh.bbox.min.y + mesh.bbox.max.y) * 0.5;
            annotate_radial_finish_semantics(op_scope.as_ref(), &tp, center_x, center_y);
        }
        Ok(tp)
    }
}

impl SemanticToolpathOp for HorizontalFinishConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_horizontal_finish(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
            .map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(
            ctx.semantic_root,
            ctx.core_debug_span_id,
            "Horizontal finish",
            &tp,
        );
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("angle_threshold", self.angle_threshold);
            scope.set_param("stepover", self.stepover);
        }
        annotate_horizontal_finish_semantics(op_scope.as_ref(), &tp, self.stepover);
        Ok(tp)
    }
}

impl SemanticToolpathOp for ProjectCurveConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let (tp, slices) =
            run_project_curve_annotated(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
                .map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(
            ctx.semantic_root,
            ctx.core_debug_span_id,
            "Project curve",
            &tp,
        );
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("depth", self.depth);
            scope.set_param("point_spacing", self.point_spacing);
        }
        annotate_project_curve_semantics(
            op_scope.as_ref(),
            &tp,
            &slices,
            self.depth,
            self.point_spacing,
        );
        Ok(tp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_dispatch_covers_all_operation_types() {
        for &op_type in crate::state::toolpath::OperationType::ALL {
            let config = OperationConfig::new_default(op_type);
            let _ = config.semantic_op();
        }
    }
}
