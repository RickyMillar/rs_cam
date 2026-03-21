use super::*;

pub fn build_cutter(tool: &ToolConfig) -> Box<dyn MillingCutter> {
    match tool.tool_type {
        ToolType::EndMill => Box::new(FlatEndmill::new(tool.diameter, tool.cutting_length)),
        ToolType::BallNose => Box::new(BallEndmill::new(tool.diameter, tool.cutting_length)),
        ToolType::BullNose => Box::new(BullNoseEndmill::new(
            tool.diameter,
            tool.corner_radius,
            tool.cutting_length,
        )),
        ToolType::VBit => Box::new(VBitEndmill::new(
            tool.diameter,
            tool.included_angle,
            tool.cutting_length,
        )),
        ToolType::TaperedBallNose => Box::new(TaperedBallEndmill::new(
            tool.diameter,
            tool.taper_half_angle,
            tool.shaft_diameter,
            tool.cutting_length,
        )),
    }
}

pub(super) fn effective_safe_z(req: &ComputeRequest) -> f64 {
    req.heights.retract_z
}

pub(super) fn require_polygons(req: &ComputeRequest) -> Result<&[Polygon2], String> {
    req.polygons
        .as_ref()
        .map(|p| p.as_slice())
        .ok_or_else(|| "No 2D geometry (import SVG)".to_string())
}

pub(super) fn require_mesh(req: &ComputeRequest) -> Result<(&TriangleMesh, SpatialIndex), String> {
    let mesh = req.mesh.as_ref().ok_or("No mesh (import STL)")?;
    let index = SpatialIndex::build_auto(mesh);
    Ok((mesh, index))
}

pub(super) fn apply_dressups(mut tp: Toolpath, req: &ComputeRequest) -> Toolpath {
    let cfg = &req.dressups;
    let tool = &req.tool;
    let safe_z = effective_safe_z(req);

    match cfg.entry_style {
        DressupEntryStyle::Ramp => {
            tp = apply_entry(
                &tp,
                CoreEntryStyle::Ramp {
                    max_angle_deg: cfg.ramp_angle,
                },
                tool.diameter / 2.0,
            );
        }
        DressupEntryStyle::Helix => {
            tp = apply_entry(
                &tp,
                CoreEntryStyle::Helix {
                    radius: cfg.helix_radius,
                    pitch: cfg.helix_pitch,
                },
                tool.diameter / 2.0,
            );
        }
        DressupEntryStyle::None => {}
    }
    if cfg.dogbone {
        tp = apply_dogbones(&tp, tool.diameter / 2.0, cfg.dogbone_angle);
    }
    if cfg.lead_in_out {
        tp = apply_lead_in_out(&tp, cfg.lead_radius);
    }
    if cfg.link_moves {
        tp = apply_link_moves(
            &tp,
            &LinkMoveParams {
                max_link_distance: cfg.link_max_distance,
                link_feed_rate: cfg.link_feed_rate,
                safe_z_threshold: safe_z * 0.9,
            },
        );
    }
    if cfg.arc_fitting {
        tp = fit_arcs(&tp, cfg.arc_tolerance);
    }
    if cfg.feed_optimization {
        match feed_optimization_heightmap(req) {
            Ok(mut hm) => {
                let nominal = tp
                    .moves
                    .iter()
                    .find_map(|m| match m.move_type {
                        MoveType::Linear { feed_rate } => Some(feed_rate),
                        _ => None,
                    })
                    .unwrap_or(1000.0);
                let cutter = build_cutter(tool);
                let params = FeedOptParams {
                    nominal_feed_rate: nominal,
                    max_feed_rate: cfg.feed_max_rate,
                    min_feed_rate: nominal * 0.5,
                    ramp_rate: cfg.feed_ramp_rate,
                    air_cut_threshold: 0.05,
                };
                tp = optimize_feed_rates(&tp, cutter.as_ref(), &mut hm, &params);
            }
            Err(reason) => {
                tracing::warn!(
                    "Skipping feed optimization for toolpath {}: {reason}",
                    req.toolpath_id.0
                );
            }
        }
    }
    if cfg.optimize_rapid_order {
        tp = rs_cam_core::tsp::optimize_rapid_order(&tp, safe_z);
    }
    tp
}

pub(super) fn feed_optimization_heightmap(req: &ComputeRequest) -> Result<Heightmap, &'static str> {
    if let Some(reason) = crate::state::toolpath::feed_optimization_unavailable_reason(
        &req.operation,
        req.stock_source,
    ) {
        return Err(reason);
    }

    let bbox = req
        .stock_bbox
        .as_ref()
        .ok_or("Feed optimization requires known stock bounds.")?;
    let cell_size = (req.tool.diameter / 4.0).clamp(0.25, 2.0);
    Ok(Heightmap::from_bounds(bbox, Some(bbox.max.z), cell_size))
}

pub(super) fn make_depth(depth: f64, per_pass: f64) -> DepthStepping {
    make_depth_ext(depth, per_pass, 0, 0.0)
}

pub(super) fn make_depth_with_finishing(
    depth: f64,
    per_pass: f64,
    finishing_passes: usize,
) -> DepthStepping {
    make_depth_ext(depth, per_pass, finishing_passes, 0.0)
}

fn make_depth_ext(depth: f64, per_pass: f64, finishing_passes: usize, top_z: f64) -> DepthStepping {
    DepthStepping {
        start_z: top_z,
        final_z: top_z - depth.abs(),
        max_step_down: per_pass,
        distribution: DepthDistribution::Even,
        finish_allowance: 0.0,
        finishing_passes,
    }
}

#[allow(dead_code)]
fn make_depth_from_heights(
    heights: &crate::state::toolpath::ResolvedHeights,
    per_pass: f64,
    finishing_passes: usize,
) -> DepthStepping {
    DepthStepping {
        start_z: heights.top_z,
        final_z: heights.bottom_z,
        max_step_down: per_pass,
        distribution: DepthDistribution::Even,
        finish_allowance: 0.0,
        finishing_passes,
    }
}

pub(super) fn run_collision_check(
    req: &CollisionRequest,
    cancel: &AtomicBool,
) -> Result<CollisionResult, ComputeError> {
    let index = SpatialIndex::build_auto(&req.mesh);
    let assembly = ToolAssembly {
        cutter_radius: req.tool.diameter / 2.0,
        cutter_length: req.tool.cutting_length,
        shank_diameter: req.tool.shank_diameter,
        shank_length: req.tool.shank_length,
        holder_diameter: req.tool.holder_diameter,
        holder_length: req.tool.stickout - req.tool.cutting_length - req.tool.shank_length,
    };
    let report = check_collisions_interpolated_with_cancel(
        &req.toolpath,
        &assembly,
        &req.mesh,
        &index,
        1.0,
        &|| cancel.load(Ordering::SeqCst),
    )
    .map_err(|_| ComputeError::Cancelled)?;
    let positions: Vec<[f32; 3]> = report
        .collisions
        .iter()
        .map(|collision| {
            [
                collision.position.x as f32,
                collision.position.y as f32,
                collision.position.z as f32,
            ]
        })
        .collect();
    Ok(CollisionResult { report, positions })
}

pub(super) fn compute_stats(tp: &Toolpath) -> ToolpathStats {
    let mut cutting = 0.0;
    let mut rapid = 0.0;
    for i in 1..tp.moves.len() {
        let from = tp.moves[i - 1].target;
        let to = tp.moves[i].target;
        let distance =
            ((to.x - from.x).powi(2) + (to.y - from.y).powi(2) + (to.z - from.z).powi(2)).sqrt();
        match tp.moves[i].move_type {
            MoveType::Rapid => rapid += distance,
            _ => cutting += distance,
        }
    }
    ToolpathStats {
        move_count: tp.moves.len(),
        cutting_distance: cutting,
        rapid_distance: rapid,
    }
}
