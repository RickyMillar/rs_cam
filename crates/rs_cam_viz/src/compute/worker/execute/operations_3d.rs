use super::{
    Adaptive3dConfig, Adaptive3dEntryStyle, Adaptive3dParams, AtomicBool, ComputeError,
    ComputeRequest, DropCutterConfig, EntryStyle3d, HorizontalFinishConfig, HorizontalFinishParams,
    OperationExecutionContext, Ordering, P3, PencilConfig, PencilParams, ProjectCurveConfig,
    ProjectCurveParams, ProjectCurveSlice, RadialFinishConfig, RadialFinishParams,
    RampFinishConfig, RampFinishParams, ScallopConfig, ScallopParams, SemanticToolpathOp,
    SpatialIndex, SpiralFinishConfig, SpiralFinishParams, SteepShallowConfig, SteepShallowParams,
    ToolDefinition, Toolpath, ToolpathPhaseTracker, ToolpathSemanticKind, TriangleMesh,
    WaterlineConfig, WaterlineParams, annotate_adaptive3d_runtime_semantics,
    annotate_horizontal_finish_semantics, annotate_operation_scope,
    annotate_pencil_runtime_semantics, annotate_project_curve_semantics,
    annotate_radial_finish_semantics, annotate_ramp_finish_runtime_semantics,
    annotate_scallop_runtime_semantics, annotate_spiral_finish_runtime_semantics, append_toolpath,
    batch_drop_cutter_with_cancel, bind_scope_to_run, build_cutter, cutting_runs, effective_safe_z,
    horizontal_finish_toolpath, project_curve_toolpath, radial_finish_toolpath, require_mesh,
    require_polygons, steep_shallow_toolpath,
};
use crate::compute::OperationError;

/// Compute per-cell slope angle (degrees from horizontal) from a drop-cutter grid.
///
/// Uses finite differences of adjacent Z values to estimate the surface normal
/// at each cell, then converts to an angle from horizontal (0° = flat, 90° = vertical).
#[allow(clippy::indexing_slicing)] // bounded by grid dimensions
fn compute_grid_slopes(grid: &rs_cam_core::dropcutter::DropCutterGrid) -> Vec<f64> {
    let rows = grid.rows;
    let cols = grid.cols;
    let mut slopes = vec![0.0f64; rows * cols];

    for row in 0..rows {
        for col in 0..cols {
            let z = grid.get(row, col).z;

            // Finite difference: dz/dx and dz/dy from neighbors
            let dz_dx = if col > 0 && col + 1 < cols {
                (grid.get(row, col + 1).z - grid.get(row, col.saturating_sub(1)).z)
                    / (2.0 * grid.x_step)
            } else if col + 1 < cols {
                (grid.get(row, col + 1).z - z) / grid.x_step
            } else if col > 0 {
                (z - grid.get(row, col - 1).z) / grid.x_step
            } else {
                0.0
            };

            let dz_dy = if row > 0 && row + 1 < rows {
                (grid.get(row + 1, col).z - grid.get(row.saturating_sub(1), col).z)
                    / (2.0 * grid.y_step)
            } else if row + 1 < rows {
                (grid.get(row + 1, col).z - z) / grid.y_step
            } else if row > 0 {
                (z - grid.get(row - 1, col).z) / grid.y_step
            } else {
                0.0
            };

            // Slope angle from horizontal: atan(sqrt(dz_dx^2 + dz_dy^2))
            let gradient = (dz_dx * dz_dx + dz_dy * dz_dy).sqrt();
            slopes[row * cols + col] = gradient.atan().to_degrees();
        }
    }
    slopes
}

fn prepare_mesh_operation<'a>(
    req: &'a ComputeRequest,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<(&'a TriangleMesh, SpatialIndex, ToolDefinition), OperationError> {
    let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Prepare input"));
    let _prepare_scope = debug.map(|ctx| ctx.start_span("prepare_input", "Prepare input"));
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    Ok((mesh, index, cutter))
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
        prepare_mesh_operation(req, phase_tracker, debug).map_err(ComputeError::from)?;
    const ADAPTIVE3D_HELIX_RADIUS_FACTOR: f64 = 0.3;
    const ADAPTIVE3D_HELIX_PITCH_MM: f64 = 2.0;
    const ADAPTIVE3D_RAMP_ANGLE_DEG: f64 = 10.0;

    let entry = match cfg.entry_style {
        Adaptive3dEntryStyle::Plunge => EntryStyle3d::Plunge,
        Adaptive3dEntryStyle::Helix => EntryStyle3d::Helix {
            radius: req.tool.diameter * ADAPTIVE3D_HELIX_RADIUS_FACTOR,
            pitch: ADAPTIVE3D_HELIX_PITCH_MM,
        },
        Adaptive3dEntryStyle::Ramp => EntryStyle3d::Ramp {
            max_angle_deg: ADAPTIVE3D_RAMP_ANGLE_DEG,
        },
    };
    let params = Adaptive3dParams {
        tool_radius: req.tool.diameter / 2.0,
        stepover: cfg.stepover,
        depth_per_pass: cfg.depth_per_pass,
        stock_to_leave: cfg.stock_to_leave_axial.max(cfg.stock_to_leave_radial),
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        tolerance: cfg.tolerance,
        min_cutting_radius: cfg.min_cutting_radius,
        // Use actual stock top Z when available; fall back to config value.
        stock_top_z: if let Some(ref bbox) = req.stock_bbox {
            bbox.max.z
        } else {
            cfg.stock_top_z
        },
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
        initial_stock: req.prior_stock.clone(),
        clearing_strategy: match cfg.clearing_strategy {
            crate::state::toolpath::ClearingStrategy::ContourParallel => {
                rs_cam_core::adaptive3d::ClearingStrategy3d::ContourParallel
            }
            crate::state::toolpath::ClearingStrategy::Adaptive => {
                rs_cam_core::adaptive3d::ClearingStrategy3d::Adaptive
            }
        },
        z_blend: cfg.z_blend,
    };
    tracing::info!(
        stock_top_z = params.stock_top_z,
        depth_per_pass = params.depth_per_pass,
        stock_to_leave = params.stock_to_leave,
        tool_radius = params.tool_radius,
        stepover = params.stepover,
        strategy = ?params.clearing_strategy,
        "GUI adaptive3d params"
    );
    rs_cam_core::adaptive3d::adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
        mesh,
        &index,
        &cutter,
        &params,
        &|| cancel.load(Ordering::SeqCst),
        debug,
    )
    .map_err(|_e| ComputeError::Cancelled)
}

fn run_pencil_annotated(
    req: &ComputeRequest,
    cfg: &PencilConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<rs_cam_core::pencil::PencilRuntimeAnnotation>), OperationError> {
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
        stock_to_leave: cfg.stock_to_leave,
    };
    Ok(rs_cam_core::pencil::pencil_toolpath_structured_annotated(
        mesh, &index, &cutter, &params, debug,
    ))
}

pub(super) fn run_scallop_annotated(
    req: &ComputeRequest,
    cfg: &ScallopConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<
    (
        Toolpath,
        Vec<rs_cam_core::scallop::ScallopRuntimeAnnotation>,
    ),
    OperationError,
> {
    if !req.tool.tool_type.has_ball_tip() {
        return Err(OperationError::InvalidTool(
            "Scallop operation requires a Ball Nose or Tapered Ball Nose tool".into(),
        ));
    }
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = ScallopParams {
        scallop_height: cfg.scallop_height,
        tolerance: cfg.tolerance,
        direction: cfg.direction,
        continuous: cfg.continuous,
        slope_from: cfg.slope_from,
        slope_to: cfg.slope_to,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        stock_to_leave: cfg.stock_to_leave,
    };
    Ok(rs_cam_core::scallop::scallop_toolpath_structured_annotated(
        mesh, &index, &cutter, &params, debug,
    ))
}

fn run_steep_shallow(
    req: &ComputeRequest,
    cfg: &SteepShallowConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, OperationError> {
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
        stock_to_leave: cfg.stock_to_leave,
        tolerance: cfg.tolerance,
    };
    Ok(steep_shallow_toolpath(mesh, &index, &cutter, &params))
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
    OperationError,
> {
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = RampFinishParams {
        max_stepdown: cfg.max_stepdown,
        slope_from: cfg.slope_from,
        slope_to: cfg.slope_to,
        direction: cfg.direction,
        order_bottom_up: cfg.order_bottom_up,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave,
        tolerance: cfg.tolerance,
    };
    Ok(
        rs_cam_core::ramp_finish::ramp_finish_toolpath_structured_annotated(
            mesh, &index, &cutter, &params, debug,
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
    OperationError,
> {
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = SpiralFinishParams {
        stepover: cfg.stepover,
        direction: cfg.direction,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        stock_to_leave: cfg.stock_to_leave,
    };
    Ok(
        rs_cam_core::spiral_finish::spiral_finish_toolpath_structured_annotated(
            mesh, &index, &cutter, &params, debug,
        ),
    )
}

fn run_radial_finish(
    req: &ComputeRequest,
    cfg: &RadialFinishConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, OperationError> {
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = RadialFinishParams {
        angular_step: cfg.angular_step,
        point_spacing: cfg.point_spacing,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        stock_to_leave: cfg.stock_to_leave,
    };
    Ok(radial_finish_toolpath(mesh, &index, &cutter, &params))
}

fn run_horizontal_finish(
    req: &ComputeRequest,
    cfg: &HorizontalFinishConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath, OperationError> {
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let params = HorizontalFinishParams {
        angle_threshold: cfg.angle_threshold,
        stepover: cfg.stepover,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        stock_to_leave: cfg.stock_to_leave,
    };
    Ok(horizontal_finish_toolpath(mesh, &index, &cutter, &params))
}

fn run_project_curve_annotated(
    req: &ComputeRequest,
    cfg: &ProjectCurveConfig,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<ProjectCurveSlice>), OperationError> {
    let polys = require_polygons(req)?;
    let (mesh, index, cutter) = prepare_mesh_operation(req, phase_tracker, debug)?;
    let direction = match cfg.direction {
        crate::state::toolpath::ProjectCurveDirection::FromAbove => {
            rs_cam_core::project_curve::ProjectDirection::FromAbove
        }
        crate::state::toolpath::ProjectCurveDirection::FromBelow => {
            rs_cam_core::project_curve::ProjectDirection::FromBelow
        }
    };
    let params = ProjectCurveParams {
        depth: cfg.depth,
        point_spacing: cfg.point_spacing,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
        direction,
    };
    let mut out = Toolpath::new();
    let mut slices = Vec::new();
    for (source_curve_index, p) in polys.iter().enumerate() {
        let move_start = out.moves.len();
        let tp = project_curve_toolpath(p, mesh, &index, &cutter, &params);
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

// --- SemanticToolpathOp implementations for 3D operations ---

impl SemanticToolpathOp for DropCutterConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let (mesh, index, cutter) =
            prepare_mesh_operation(ctx.req, ctx.phase_tracker, ctx.debug_root)
                .map_err(ComputeError::from)?;
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
                &cutter,
                self.stepover,
                0.0,
                self.min_z,
                &|| ctx.cancel.load(Ordering::SeqCst),
            )
            .map_err(|_e| ComputeError::Cancelled)?
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
            scope.set_param("slope_from", self.slope_from);
            scope.set_param("slope_to", self.slope_to);
        }
        let op_ctx = op_scope.as_ref().map(|scope| scope.context());

        // Pre-compute per-cell slope angle (degrees from horizontal) for slope filtering.
        let slope_filter_active = self.slope_from > 0.01 || self.slope_to < 89.99;
        let slope_angles: Vec<f64> = if slope_filter_active {
            compute_grid_slopes(&grid)
        } else {
            Vec::new()
        };

        let mut out = Toolpath::new();
        {
            let _phase_scope = ctx
                .phase_tracker
                .map(|tracker| tracker.start_phase("Rasterize grid"));
            let _raster_scope = ctx
                .debug_root
                .map(|ctx| ctx.start_span("rasterize_grid", "Rasterize grid"));
            let mut writer = rs_cam_core::semantic_trace::ToolpathSemanticWriter::new(&mut out);
            let safe_z = effective_safe_z(ctx.req);
            for row in 0..grid.rows {
                let cols: Vec<usize> = if row % 2 == 0 {
                    (0..grid.cols).collect()
                } else {
                    (0..grid.cols).rev().collect()
                };
                if cols.is_empty() {
                    continue;
                }
                let row_scope = op_ctx.as_ref().map(|ctx| {
                    ctx.start_item(ToolpathSemanticKind::Row, format!("Row {}", row + 1))
                });
                if let Some(scope) = row_scope.as_ref() {
                    scope.set_param("row_index", row + 1);
                }

                // Build row toolpath, splitting at slope-filtered gaps
                let mut row_tp = Toolpath::new();
                let mut in_cut = false;
                for &col in &cols {
                    // Slope filter: skip points outside [slope_from, slope_to]
                    #[allow(clippy::collapsible_if)]
                    if slope_filter_active {
                        let idx = row * grid.cols + col;
                        if let Some(&angle) = slope_angles.get(idx) {
                            if angle < self.slope_from || angle > self.slope_to {
                                if in_cut {
                                    // Retract at end of cutting segment
                                    let prev = grid.get(row, col);
                                    row_tp.rapid_to(P3::new(prev.x, prev.y, safe_z));
                                    in_cut = false;
                                }
                                continue;
                            }
                        }
                    }

                    let pt = grid.get(row, col);
                    if !in_cut {
                        // Start new cutting segment
                        row_tp.rapid_to(P3::new(pt.x, pt.y, safe_z));
                        row_tp.feed_to(pt.position(), self.plunge_rate);
                        in_cut = true;
                    } else {
                        row_tp.feed_to(pt.position(), self.feed_rate);
                    }
                }
                if in_cut {
                    // SAFETY: if in_cut, we have at least one point
                    #[allow(clippy::expect_used)]
                    let last_col = cols
                        .iter()
                        .rev()
                        .find(|&&c| {
                            if !slope_filter_active {
                                return true;
                            }
                            let idx = row * grid.cols + c;
                            slope_angles
                                .get(idx)
                                .is_none_or(|&a| a >= self.slope_from && a <= self.slope_to)
                        })
                        .expect("in_cut implies at least one valid col");
                    let last_pt = grid.get(row, *last_col);
                    row_tp.rapid_to(P3::new(last_pt.x, last_pt.y, safe_z));
                }
                if !row_tp.moves.is_empty() {
                    append_toolpath(&mut writer, row_scope.as_ref(), row_tp);
                }
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
                .map_err(ComputeError::from)?;
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
            scope.set_param("start_z", ctx.req.heights.top_z);
            scope.set_param("final_z", ctx.req.heights.bottom_z);
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
            let wl_start_z = ctx.req.heights.top_z;
            let wl_final_z = ctx.req.heights.bottom_z;
            let mut z = wl_start_z;
            let mut level_idx = 0usize;
            while z >= wl_final_z - 1e-10 {
                let contours = rs_cam_core::waterline::waterline_contours_with_cancel(
                    mesh,
                    &index,
                    &cutter,
                    z,
                    self.sampling,
                    &|| ctx.cancel.load(Ordering::SeqCst),
                )
                .map_err(|_e| ComputeError::Cancelled)?;
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
                    // SAFETY: contour.len() >= 3 guaranteed by guard above
                    #[allow(clippy::indexing_slicing)]
                    let first = contour[0];
                    let mut contour_tp = Toolpath::new();
                    contour_tp.rapid_to(P3::new(first.x, first.y, params.safe_z));
                    contour_tp.feed_to(P3::new(first.x, first.y, z), params.plunge_rate);
                    for pt in contour.iter().skip(1) {
                        contour_tp.feed_to(P3::new(pt.x, pt.y, z), params.feed_rate);
                    }
                    contour_tp.feed_to(P3::new(first.x, first.y, z), params.feed_rate);
                    contour_tp.rapid_to(P3::new(first.x, first.y, params.safe_z));
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
                .map_err(ComputeError::from)?;
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
                .map_err(ComputeError::from)?;
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
            .map_err(ComputeError::from)?;
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
                .map_err(ComputeError::from)?;
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
                .map_err(ComputeError::from)?;
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
            .map_err(ComputeError::from)?;
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
            .map_err(ComputeError::from)?;
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
                .map_err(ComputeError::from)?;
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
