use super::*;
use crate::compute::OperationError;

#[allow(dead_code)]
fn run_pocket(req: &ComputeRequest, cfg: &PocketConfig) -> Result<Toolpath, OperationError> {
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
                    angle: cfg.angle,
                },
            ),
        });
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

pub(super) fn run_profile(req: &ComputeRequest, cfg: &ProfileConfig) -> Result<Toolpath, OperationError> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let side = cfg.side;
    let depth = make_depth_with_finishing(cfg.depth, cfg.depth_per_pass, cfg.finishing_passes);
    let safe_z = effective_safe_z(req);
    let levels = depth.all_levels();
    let final_z = levels.last().copied().unwrap_or(-cfg.depth.abs());
    let mut out = Toolpath::new();
    for p in polys {
        let make_pass = |z: f64| {
            profile_toolpath(
                p,
                &ProfileParams {
                    tool_radius: tr,
                    side,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z,
                    climb: cfg.climb,
                },
            )
        };
        for (level_idx, &z) in levels.iter().enumerate() {
            let pass_tp = make_pass(z);
            if pass_tp.moves.is_empty() {
                continue;
            }
            // Retract between levels (not before first)
            if level_idx > 0 && !out.moves.is_empty() {
                out.final_retract(safe_z);
            }
            let is_final = (z - final_z).abs() < 1e-9;
            if cfg.tab_count > 0 && is_final {
                let tabbed = apply_tabs(
                    &pass_tp,
                    &even_tabs(cfg.tab_count, cfg.tab_width, cfg.tab_height),
                    z,
                );
                out.moves.extend(tabbed.moves);
            } else {
                out.moves.extend(pass_tp.moves);
            }
        }
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
                        initial_stock: req.prior_stock.clone(),
                    },
                    &|| cancel.load(Ordering::SeqCst),
                    debug,
                )
                .map_err(|_e| ComputeError::Cancelled)?;
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

fn run_vcarve(req: &ComputeRequest, cfg: &VCarveConfig) -> Result<Toolpath, OperationError> {
    let polys = require_polygons(req)?;
    let ha = match req.tool.tool_type {
        ToolType::VBit => (req.tool.included_angle / 2.0).to_radians(),
        _ => {
            return Err(OperationError::InvalidTool(
                "VCarve requires V-Bit tool".into(),
            ))
        }
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

fn run_rest(req: &ComputeRequest, cfg: &RestConfig) -> Result<Toolpath, OperationError> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let ptr = req
        .prev_tool_radius
        .ok_or_else(|| OperationError::Other("Previous tool not set".into()))?;
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
                    angle: cfg.angle,
                },
            )
        });
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

pub(super) fn run_inlay(req: &ComputeRequest, cfg: &InlayConfig) -> Result<Toolpath, OperationError> {
    let polys = require_polygons(req)?;
    let ha = match req.tool.tool_type {
        ToolType::VBit => (req.tool.included_angle / 2.0).to_radians(),
        _ => {
            return Err(OperationError::InvalidTool(
                "Inlay requires V-Bit tool".into(),
            ))
        }
    };
    let safe_z = effective_safe_z(req);
    let mut female_out = Toolpath::new();
    let mut male_out = Toolpath::new();
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
                safe_z,
                tolerance: cfg.tolerance,
            },
        );
        female_out.moves.extend(r.female.moves);
        male_out.moves.extend(r.male.moves);
    }
    // Combine female and male with a clear separator so they can be
    // distinguished in the output (retract to safe_z between sections).
    let mut out = female_out;
    if !male_out.moves.is_empty() {
        out.final_retract(safe_z);
        out.moves.extend(male_out.moves);
    }
    Ok(out)
}

pub(super) fn run_zigzag(req: &ComputeRequest, cfg: &ZigzagConfig) -> Result<Toolpath, OperationError> {
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
                    angle: cfg.angle,
                },
            )
        });
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

fn run_trace(req: &ComputeRequest, cfg: &TraceConfig) -> Result<Toolpath, OperationError> {
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
                compensation: cfg.compensation,
            };
            trace_toolpath(p, &params)
        });
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

#[allow(dead_code)]
fn run_drill(req: &ComputeRequest, cfg: &DrillConfig) -> Result<Toolpath, OperationError> {
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
        return Err(OperationError::MissingGeometry(
            "No hole positions found (import SVG with circles)".to_string(),
        ));
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

fn run_chamfer(req: &ComputeRequest, cfg: &ChamferConfig) -> Result<Toolpath, OperationError> {
    let polys = require_polygons(req)?;
    let ha = match req.tool.tool_type {
        ToolType::VBit => (req.tool.included_angle / 2.0).to_radians(),
        _ => {
            return Err(OperationError::InvalidTool(
                "Chamfer requires V-Bit tool".into(),
            ))
        }
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

// --- SemanticToolpathOp implementations for 2D operations ---

impl SemanticToolpathOp for FaceConfig {
    fn generate_with_tracing(
        &self,
        ctx: &OperationExecutionContext<'_>,
    ) -> Result<Toolpath, ComputeError> {
        let bbox = ctx.req.stock_bbox.ok_or_else(|| {
            ComputeError::from(OperationError::MissingGeometry(
                "No stock defined for face operation".to_string(),
            ))
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
                let mut lines = rs_cam_core::zigzag::zigzag_lines(
                    &rect,
                    ctx.req.tool.diameter / 2.0,
                    self.stepover,
                    0.0,
                );
                if self.direction == FaceDirection::OneWay {
                    // For one-way cuts, ensure all lines go in the same
                    // direction by un-reversing the odd rows that
                    // zigzag_lines alternated.
                    for (i, line) in lines.iter_mut().enumerate() {
                        if i % 2 != 0 {
                            line.swap(0, 1);
                        }
                    }
                }
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
        let polys = require_polygons(ctx.req).map_err(ComputeError::from)?;
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
                                self.angle,
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
        let polys = require_polygons(ctx.req).map_err(ComputeError::from)?;
        let tp = run_profile(ctx.req, self).map_err(ComputeError::from)?;
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
        let polys = require_polygons(ctx.req).map_err(ComputeError::from)?;
        let tp = run_vcarve(ctx.req, self).map_err(ComputeError::from)?;
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
                for run in run_iter.by_ref() {
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
        let polys = require_polygons(ctx.req).map_err(ComputeError::from)?;
        let tp = run_rest(ctx.req, self).map_err(ComputeError::from)?;
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
                            angle: self.angle,
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
        let polys = require_polygons(ctx.req).map_err(ComputeError::from)?;
        let tp = run_inlay(ctx.req, self).map_err(ComputeError::from)?;
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
                _ => {
                    return Err(OperationError::InvalidTool(
                        "Inlay requires V-Bit tool".into(),
                    )
                    .into())
                }
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
        let polys = require_polygons(ctx.req).map_err(ComputeError::from)?;
        let tp = run_zigzag(ctx.req, self).map_err(ComputeError::from)?;
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
                        self.angle,
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
        let polys = require_polygons(ctx.req).map_err(ComputeError::from)?;
        let tp = run_trace(ctx.req, self).map_err(ComputeError::from)?;
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
        let polys = require_polygons(ctx.req).map_err(ComputeError::from)?;
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
            return Err(OperationError::MissingGeometry(
                "No hole positions found (import SVG with circles)".to_string(),
            )
            .into());
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
        let polys = require_polygons(ctx.req).map_err(ComputeError::from)?;
        let tp = run_chamfer(ctx.req, self).map_err(ComputeError::from)?;
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
            return Err(OperationError::MissingGeometry(
                "No alignment pin positions defined".to_string(),
            )
            .into());
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
