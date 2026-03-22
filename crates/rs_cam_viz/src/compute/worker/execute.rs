use super::helpers::*;
use super::*;

pub(super) fn run_simulation(
    req: &SimulationRequest,
    cancel: &AtomicBool,
) -> Result<SimulationResult, ComputeError> {
    let mut stock = TriDexelStock::from_bounds(&req.stock_bbox, req.resolution);

    let mut total_moves = 0;
    let mut boundary_index = 0;
    let mut boundaries = Vec::new();
    let mut checkpoints = Vec::new();
    let mut playback_data = Vec::new();

    for group in &req.groups {
        for (tp_id, tp_name, toolpath, tool_config) in &group.toolpaths {
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

pub(super) fn run_compute(
    req: &ComputeRequest,
    cancel: &AtomicBool,
) -> Result<ToolpathResult, ComputeError> {
    let mut tp = match &req.operation {
        OperationConfig::Face(c) => run_face(req, c).map_err(ComputeError::Message),
        OperationConfig::Pocket(c) => run_pocket(req, c).map_err(ComputeError::Message),
        OperationConfig::Profile(c) => run_profile(req, c).map_err(ComputeError::Message),
        OperationConfig::Adaptive(c) => run_adaptive(req, c, cancel),
        OperationConfig::VCarve(c) => run_vcarve(req, c).map_err(ComputeError::Message),
        OperationConfig::Rest(c) => run_rest(req, c).map_err(ComputeError::Message),
        OperationConfig::Inlay(c) => run_inlay(req, c).map_err(ComputeError::Message),
        OperationConfig::Zigzag(c) => run_zigzag(req, c).map_err(ComputeError::Message),
        OperationConfig::Trace(c) => run_trace(req, c).map_err(ComputeError::Message),
        OperationConfig::Drill(c) => run_drill(req, c).map_err(ComputeError::Message),
        OperationConfig::Chamfer(c) => run_chamfer(req, c).map_err(ComputeError::Message),
        OperationConfig::DropCutter(c) => run_dropcutter(req, c, cancel),
        OperationConfig::Adaptive3d(c) => run_adaptive3d(req, c, cancel),
        OperationConfig::Waterline(c) => run_waterline(req, c, cancel),
        OperationConfig::Pencil(c) => run_pencil(req, c).map_err(ComputeError::Message),
        OperationConfig::Scallop(c) => run_scallop(req, c).map_err(ComputeError::Message),
        OperationConfig::SteepShallow(c) => {
            run_steep_shallow(req, c).map_err(ComputeError::Message)
        }
        OperationConfig::RampFinish(c) => run_ramp_finish(req, c).map_err(ComputeError::Message),
        OperationConfig::SpiralFinish(c) => {
            run_spiral_finish(req, c).map_err(ComputeError::Message)
        }
        OperationConfig::RadialFinish(c) => {
            run_radial_finish(req, c).map_err(ComputeError::Message)
        }
        OperationConfig::HorizontalFinish(c) => {
            run_horizontal_finish(req, c).map_err(ComputeError::Message)
        }
        OperationConfig::ProjectCurve(c) => {
            run_project_curve(req, c).map_err(ComputeError::Message)
        }
    }?;

    tp = apply_dressups(tp, req);

    if req.boundary_enabled
        && let Some(bbox) = &req.stock_bbox
    {
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
        }
    }

    let stats = compute_stats(&tp);
    Ok(ToolpathResult {
        toolpath: Arc::new(tp),
        stats,
    })
}

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
) -> Result<Toolpath, ComputeError> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let depth = make_depth(cfg.depth, cfg.depth_per_pass);
    let mut out = Toolpath::new();
    for p in polys {
        for (level_idx, z) in depth.all_levels().into_iter().enumerate() {
            let tp = adaptive_toolpath_with_cancel(
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

fn run_dropcutter(
    req: &ComputeRequest,
    cfg: &DropCutterConfig,
    cancel: &AtomicBool,
) -> Result<Toolpath, ComputeError> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let grid = batch_drop_cutter_with_cancel(
        mesh,
        &index,
        cutter.as_ref(),
        cfg.stepover,
        0.0,
        cfg.min_z,
        &|| cancel.load(Ordering::SeqCst),
    )
    .map_err(|_| ComputeError::Cancelled)?;
    Ok(raster_toolpath_from_grid(
        &grid,
        cfg.feed_rate,
        cfg.plunge_rate,
        effective_safe_z(req),
    ))
}

fn run_adaptive3d(
    req: &ComputeRequest,
    cfg: &Adaptive3dConfig,
    cancel: &AtomicBool,
) -> Result<Toolpath, ComputeError> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
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
    adaptive_3d_toolpath_with_cancel(mesh, &index, cutter.as_ref(), &params, &|| {
        cancel.load(Ordering::SeqCst)
    })
    .map_err(|_| ComputeError::Cancelled)
}

fn run_waterline(
    req: &ComputeRequest,
    cfg: &WaterlineConfig,
    cancel: &AtomicBool,
) -> Result<Toolpath, ComputeError> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let params = WaterlineParams {
        sampling: cfg.sampling,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req),
    };
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

fn run_pencil(req: &ComputeRequest, cfg: &PencilConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
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

fn run_scallop(req: &ComputeRequest, cfg: &ScallopConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
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

fn run_steep_shallow(req: &ComputeRequest, cfg: &SteepShallowConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
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

fn run_ramp_finish(req: &ComputeRequest, cfg: &RampFinishConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
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

fn run_spiral_finish(req: &ComputeRequest, cfg: &SpiralFinishConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
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

fn run_radial_finish(req: &ComputeRequest, cfg: &RadialFinishConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
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
) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
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

fn run_project_curve(req: &ComputeRequest, cfg: &ProjectCurveConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
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
