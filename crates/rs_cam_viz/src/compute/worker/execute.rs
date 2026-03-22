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
    debug_root: Option<&'a rs_cam_core::debug_trace::ToolpathDebugContext>,
    semantic_root: Option<&'a rs_cam_core::semantic_trace::ToolpathSemanticContext>,
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

    let mut total_moves = 0;
    let mut boundary_index = 0;
    let mut boundaries = Vec::new();
    let mut checkpoints = Vec::new();
    let mut playback_data = Vec::new();

    for group in &req.groups {
        for (tp_id, tp_name, toolpath, tool_config) in &group.toolpaths {
            set_phase(&format!("Simulate {tp_name}"));
            let start_move = total_moves;
            let cutter = build_cutter(tool_config);
            stock
                .simulate_toolpath_with_cancel(toolpath, cutter.as_ref(), group.direction, &|| {
                    cancel.load(Ordering::SeqCst)
                })
                .map_err(|_| ComputeError::Cancelled)?;
            total_moves += toolpath.moves.len();

            boundaries.push(SimBoundary {
                id: *tp_id,
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
            for (_tp_id, _tp_name, toolpath, _tool_config) in &group.toolpaths {
                let rapids = check_rapid_collisions(toolpath, &req.stock_bbox);
                for rc in &rapids {
                    rapid_collision_move_indices.push(cumulative_offset + rc.move_index);
                }
                rapid_collisions.extend(rapids);
                cumulative_offset += toolpath.moves.len();
            }
        }
    }

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
                debug_root: core_ctx.as_ref(),
                semantic_root: semantic_root.as_ref(),
            };
            req.operation.semantic_op().generate_with_tracing(&exec_ctx)
        }?;

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
            let _boundary_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("boundary_clip", "Clip to boundary"));
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
                    scope.set_param(
                        "containment",
                        match req.boundary_containment {
                            crate::state::toolpath::BoundaryContainment::Center => "center",
                            crate::state::toolpath::BoundaryContainment::Inside => "inside",
                            crate::state::toolpath::BoundaryContainment::Outside => "outside",
                        },
                    );
                    scope.set_param("keep_out_count", req.keep_out_footprints.len());
                    scope.bind_to_toolpath(&tp, 0, tp.moves.len());
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
        let debug_trace = debug_recorder.finish();
        let semantic_trace = semantic_recorder.finish();
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

fn run_adaptive(
    req: &ComputeRequest,
    cfg: &AdaptiveConfig,
    cancel: &AtomicBool,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, ComputeError> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let depth = make_depth(cfg.depth, cfg.depth_per_pass);
    let mut out = Toolpath::new();
    for p in polys {
        for (level_idx, z) in depth.all_levels().into_iter().enumerate() {
            let tp = adaptive_toolpath_traced_with_cancel(
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
                out.moves.extend(tp.moves);
            }
        }
    }
    Ok(out)
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

fn run_adaptive3d(
    req: &ComputeRequest,
    cfg: &Adaptive3dConfig,
    cancel: &AtomicBool,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, ComputeError> {
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
    adaptive_3d_toolpath_traced_with_cancel(
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

fn run_pencil(
    req: &ComputeRequest,
    cfg: &PencilConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, String> {
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
    Ok(pencil_toolpath(mesh, &index, cutter.as_ref(), &params))
}

fn run_scallop(
    req: &ComputeRequest,
    cfg: &ScallopConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, String> {
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
    Ok(scallop_toolpath(mesh, &index, cutter.as_ref(), &params))
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

fn run_ramp_finish(
    req: &ComputeRequest,
    cfg: &RampFinishConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, String> {
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
    Ok(ramp_finish_toolpath(mesh, &index, cutter.as_ref(), &params))
}

fn run_spiral_finish(
    req: &ComputeRequest,
    cfg: &SpiralFinishConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, String> {
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
    Ok(spiral_finish_toolpath(
        mesh,
        &index,
        cutter.as_ref(),
        &params,
    ))
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

fn run_project_curve(
    req: &ComputeRequest,
    cfg: &ProjectCurveConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, String> {
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
    for p in polys {
        let tp = project_curve_toolpath(p, mesh, &index, cutter.as_ref(), &params);
        out.moves.extend(tp.moves);
    }
    Ok(out)
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
        scope.bind_to_toolpath(toolpath, 0, toolpath.moves.len());
    }
    scope
}

fn semantic_child_context(
    scope: Option<&rs_cam_core::semantic_trace::ToolpathSemanticScope>,
) -> Option<rs_cam_core::semantic_trace::ToolpathSemanticContext> {
    scope.map(|scope| scope.context())
}

fn annotate_cut_runs(
    semantic_ctx: Option<&rs_cam_core::semantic_trace::ToolpathSemanticContext>,
    toolpath: &Toolpath,
    default_kind: rs_cam_core::semantic_trace::ToolpathSemanticKind,
    label_prefix: &str,
) {
    if let Some(ctx) = semantic_ctx {
        for (run_idx, run) in cutting_runs(toolpath).iter().enumerate() {
            let kind = label_run_kind(run, default_kind.clone());
            let scope = ctx.start_item(kind, format!("{label_prefix} {}", run_idx + 1));
            scope.set_param("run_index", run_idx + 1);
            scope.set_param("closed_loop", run.closed_loop);
            scope.set_param("constant_z", run.constant_z);
            scope.set_param("z_min", run.z_min);
            scope.set_param("z_max", run.z_max);
            if let Some(bbox) = run.xy_bbox {
                scope.set_xy_bbox(bbox);
            }
            bind_scope_to_run(&scope, toolpath, run);
        }
    }
}

fn annotate_full_toolpath_item(
    semantic_ctx: Option<&rs_cam_core::semantic_trace::ToolpathSemanticContext>,
    kind: rs_cam_core::semantic_trace::ToolpathSemanticKind,
    label: impl Into<String>,
    toolpath: &Toolpath,
) {
    if let Some(ctx) = semantic_ctx {
        let scope = ctx.start_item(kind, label);
        scope.bind_to_toolpath(toolpath, 0, toolpath.moves.len());
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
            scope.set_param("pattern", format!("{:?}", self.pattern));
            scope.set_param("depth", self.depth);
            scope.set_param("stepover", self.stepover);
        }
        let op_ctx = op_scope.as_ref().map(|scope| scope.context());
        let depth =
            make_depth_with_finishing(self.depth, self.depth_per_pass, self.finishing_passes);
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
                for (level_idx, z) in depth.all_levels().into_iter().enumerate() {
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
                            for (contour_idx, contour) in contours.iter().enumerate() {
                                let contour_scope = level_scope.as_ref().map(|scope| {
                                    scope.context().start_item(
                                        ToolpathSemanticKind::Contour,
                                        format!("Contour {}", contour_idx + 1),
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
                            for (row_idx, line) in lines.iter().enumerate() {
                                let row_scope = level_scope.as_ref().map(|scope| {
                                    scope.context().start_item(
                                        ToolpathSemanticKind::Raster,
                                        format!("Raster {}", row_idx + 1),
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
        let tp = run_profile(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Profile", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("side", format!("{:?}", self.side));
            scope.set_param("tab_count", self.tab_count);
            scope.set_param("climb", self.climb);
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(
            op_ctx.as_ref(),
            &tp,
            ToolpathSemanticKind::Contour,
            "Contour",
        );
        if self.tab_count > 0 {
            annotate_full_toolpath_item(
                op_ctx.as_ref(),
                ToolpathSemanticKind::FinishPass,
                "Tabs",
                &tp,
            );
        }
        Ok(tp)
    }
}

impl SemanticToolpathOp for AdaptiveConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_adaptive(ctx.req, self, ctx.cancel, ctx.debug_root)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Adaptive", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("stepover", self.stepover);
            scope.set_param("slot_clearing", self.slot_clearing);
            scope.set_param("tolerance", self.tolerance);
        }
        let op_ctx = op_scope.as_ref().map(|scope| scope.context());
        let depth = make_depth(self.depth, self.depth_per_pass);
        for (level_idx, z) in depth.all_levels().into_iter().enumerate() {
            if let Some(op_ctx) = op_ctx.as_ref() {
                let level_scope = op_ctx.start_item(
                    ToolpathSemanticKind::DepthLevel,
                    format!("Level {}", level_idx + 1),
                );
                level_scope.set_param("z", z);
                level_scope.bind_to_toolpath(&tp, 0, tp.moves.len());
            }
        }
        annotate_cut_runs(op_ctx.as_ref(), &tp, ToolpathSemanticKind::Pass, "Pass");
        Ok(tp)
    }
}

impl SemanticToolpathOp for VCarveConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_vcarve(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "VCarve", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("max_depth", self.max_depth);
            scope.set_param("stepover", self.stepover);
            scope.set_param("tolerance", self.tolerance);
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(
            op_ctx.as_ref(),
            &tp,
            ToolpathSemanticKind::Contour,
            "Contour",
        );
        Ok(tp)
    }
}

impl SemanticToolpathOp for RestConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_rest(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Rest machining", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("depth", self.depth);
            scope.set_param("stepover", self.stepover);
            scope.set_param("angle_deg", self.angle);
            if let Some(prev_tool_radius) = ctx.req.prev_tool_radius {
                scope.set_param("previous_tool_radius", prev_tool_radius);
            }
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(
            op_ctx.as_ref(),
            &tp,
            ToolpathSemanticKind::Pass,
            "Rest pass",
        );
        Ok(tp)
    }
}

impl SemanticToolpathOp for InlayConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_inlay(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Inlay", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("pocket_depth", self.pocket_depth);
            scope.set_param("glue_gap", self.glue_gap);
            scope.set_param("flat_depth", self.flat_depth);
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(
            op_ctx.as_ref(),
            &tp,
            ToolpathSemanticKind::Contour,
            "Inlay pass",
        );
        Ok(tp)
    }
}

impl SemanticToolpathOp for ZigzagConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_zigzag(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Zigzag", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("depth", self.depth);
            scope.set_param("stepover", self.stepover);
            scope.set_param("angle_deg", self.angle);
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(op_ctx.as_ref(), &tp, ToolpathSemanticKind::Raster, "Raster");
        Ok(tp)
    }
}

impl SemanticToolpathOp for TraceConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_trace(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Trace", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("depth", self.depth);
            scope.set_param("compensation", format!("{:?}", self.compensation));
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(
            op_ctx.as_ref(),
            &tp,
            ToolpathSemanticKind::Contour,
            "Trace contour",
        );
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
                append_toolpath(
                    &mut writer,
                    hole_scope.as_ref(),
                    drill_toolpath(&[*hole], &params),
                );
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
        let tp = run_chamfer(ctx.req, self).map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Chamfer", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("chamfer_width", self.chamfer_width);
            scope.set_param("tip_offset", self.tip_offset);
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(
            op_ctx.as_ref(),
            &tp,
            ToolpathSemanticKind::Contour,
            "Chamfer contour",
        );
        Ok(tp)
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
        let tp = run_adaptive3d(ctx.req, self, ctx.cancel, ctx.phase_tracker, ctx.debug_root)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "3D Rough", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("stepover", self.stepover);
            scope.set_param("depth_per_pass", self.depth_per_pass);
            scope.set_param("stock_to_leave", self.stock_to_leave_axial);
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(op_ctx.as_ref(), &tp, ToolpathSemanticKind::Pass, "Pass");
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
        let tp = run_pencil(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
            .map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Pencil", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("bitangency_angle", self.bitangency_angle);
            scope.set_param("offset_passes", self.num_offset_passes);
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(op_ctx.as_ref(), &tp, ToolpathSemanticKind::Chain, "Chain");
        Ok(tp)
    }
}

impl SemanticToolpathOp for ScallopConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_scallop(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
            .map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Scallop", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("scallop_height", self.scallop_height);
            scope.set_param("continuous", self.continuous);
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(op_ctx.as_ref(), &tp, ToolpathSemanticKind::Ring, "Ring");
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
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Steep/Shallow", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("threshold_angle", self.threshold_angle);
            scope.set_param("overlap_distance", self.overlap_distance);
            scope.set_param("steep_first", self.steep_first);
        }
        if let Some(ctx) = op_scope.as_ref().map(|scope| scope.context()) {
            for (run_idx, run) in cutting_runs(&tp).iter().enumerate() {
                let kind = if run.closed_loop && run.constant_z {
                    ToolpathSemanticKind::Contour
                } else {
                    ToolpathSemanticKind::Raster
                };
                let label = if kind == ToolpathSemanticKind::Contour {
                    format!("Steep contour {}", run_idx + 1)
                } else {
                    format!("Shallow pass {}", run_idx + 1)
                };
                let scope = ctx.start_item(kind, label);
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
        let tp = run_ramp_finish(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
            .map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Ramp finish", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("max_stepdown", self.max_stepdown);
            scope.set_param("direction", format!("{:?}", self.direction));
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(op_ctx.as_ref(), &tp, ToolpathSemanticKind::Ramp, "Ramp");
        Ok(tp)
    }
}

impl SemanticToolpathOp for SpiralFinishConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_spiral_finish(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
            .map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Spiral finish", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("stepover", self.stepover);
            scope.set_param("direction", format!("{:?}", self.direction));
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(op_ctx.as_ref(), &tp, ToolpathSemanticKind::Ring, "Ring");
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
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Radial finish", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("angular_step", self.angular_step);
            scope.set_param("point_spacing", self.point_spacing);
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(op_ctx.as_ref(), &tp, ToolpathSemanticKind::Ray, "Ray");
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
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Horizontal finish", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("angle_threshold", self.angle_threshold);
            scope.set_param("stepover", self.stepover);
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(op_ctx.as_ref(), &tp, ToolpathSemanticKind::Slice, "Slice");
        Ok(tp)
    }
}

impl SemanticToolpathOp for ProjectCurveConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let tp = run_project_curve(ctx.req, self, ctx.phase_tracker, ctx.debug_root)
            .map_err(ComputeError::Message)?;
        let op_scope = annotate_operation_scope(ctx.semantic_root, "Project curve", &tp);
        if let Some(scope) = op_scope.as_ref() {
            scope.set_param("depth", self.depth);
            scope.set_param("point_spacing", self.point_spacing);
        }
        let op_ctx = semantic_child_context(op_scope.as_ref());
        annotate_cut_runs(op_ctx.as_ref(), &tp, ToolpathSemanticKind::Curve, "Curve");
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
