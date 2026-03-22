use super::*;
use serde_json::json;
use std::path::PathBuf;

use rs_cam_core::simulation::Heightmap;

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

pub(super) fn apply_dressups(
    mut tp: Toolpath,
    req: &ComputeRequest,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
    semantic: Option<&rs_cam_core::semantic_trace::ToolpathSemanticContext>,
) -> Toolpath {
    let cfg = &req.dressups;
    let tool = &req.tool;
    let safe_z = effective_safe_z(req);

    match cfg.entry_style {
        DressupEntryStyle::Ramp => {
            let debug_scope = debug.map(|ctx| ctx.start_span("entry_style", "Ramp entry"));
            let debug_span_id = debug_scope.as_ref().map(|scope| scope.id());
            let semantic_scope = semantic.map(|ctx| {
                let scope = ctx.start_item(
                    rs_cam_core::semantic_trace::ToolpathSemanticKind::Entry,
                    "Ramp entry",
                );
                if let Some(span_id) = debug_span_id {
                    scope.set_debug_span_id(span_id);
                }
                scope.set_param("kind", "ramp");
                scope.set_param("max_angle_deg", cfg.ramp_angle);
                scope
            });
            tp = apply_entry(
                &tp,
                CoreEntryStyle::Ramp {
                    max_angle_deg: cfg.ramp_angle,
                },
                tool.diameter / 2.0,
            );
            if let Some(scope) = semantic_scope.as_ref() {
                scope.bind_to_toolpath(&tp, 0, tp.moves.len());
            }
            if let Some(scope) = debug_scope.as_ref()
                && !tp.moves.is_empty()
            {
                scope.set_move_range(0, tp.moves.len() - 1);
            }
        }
        DressupEntryStyle::Helix => {
            let debug_scope = debug.map(|ctx| ctx.start_span("entry_style", "Helix entry"));
            let debug_span_id = debug_scope.as_ref().map(|scope| scope.id());
            let semantic_scope = semantic.map(|ctx| {
                let scope = ctx.start_item(
                    rs_cam_core::semantic_trace::ToolpathSemanticKind::Entry,
                    "Helix entry",
                );
                if let Some(span_id) = debug_span_id {
                    scope.set_debug_span_id(span_id);
                }
                scope.set_param("kind", "helix");
                scope.set_param("radius", cfg.helix_radius);
                scope.set_param("pitch", cfg.helix_pitch);
                scope
            });
            tp = apply_entry(
                &tp,
                CoreEntryStyle::Helix {
                    radius: cfg.helix_radius,
                    pitch: cfg.helix_pitch,
                },
                tool.diameter / 2.0,
            );
            if let Some(scope) = semantic_scope.as_ref() {
                scope.bind_to_toolpath(&tp, 0, tp.moves.len());
            }
            if let Some(scope) = debug_scope.as_ref()
                && !tp.moves.is_empty()
            {
                scope.set_move_range(0, tp.moves.len() - 1);
            }
        }
        DressupEntryStyle::None => {}
    }
    if cfg.dogbone {
        let debug_scope = debug.map(|ctx| ctx.start_span("dogbones", "Apply dogbones"));
        let debug_span_id = debug_scope.as_ref().map(|scope| scope.id());
        let semantic_scope = semantic.map(|ctx| {
            let scope = ctx.start_item(
                rs_cam_core::semantic_trace::ToolpathSemanticKind::Dressup,
                "Dogbones",
            );
            if let Some(span_id) = debug_span_id {
                scope.set_debug_span_id(span_id);
            }
            scope.set_param("angle_deg", cfg.dogbone_angle);
            scope
        });
        tp = apply_dogbones(&tp, tool.diameter / 2.0, cfg.dogbone_angle);
        if let Some(scope) = semantic_scope.as_ref() {
            scope.bind_to_toolpath(&tp, 0, tp.moves.len());
        }
        if let Some(scope) = debug_scope.as_ref()
            && !tp.moves.is_empty()
        {
            scope.set_move_range(0, tp.moves.len() - 1);
        }
    }
    if cfg.lead_in_out {
        let debug_scope = debug.map(|ctx| ctx.start_span("lead_in_out", "Apply lead in/out"));
        let debug_span_id = debug_scope.as_ref().map(|scope| scope.id());
        let semantic_scope = semantic.map(|ctx| {
            let scope = ctx.start_item(
                rs_cam_core::semantic_trace::ToolpathSemanticKind::Dressup,
                "Lead in/out",
            );
            if let Some(span_id) = debug_span_id {
                scope.set_debug_span_id(span_id);
            }
            scope.set_param("radius", cfg.lead_radius);
            scope
        });
        tp = apply_lead_in_out(&tp, cfg.lead_radius);
        if let Some(scope) = semantic_scope.as_ref() {
            scope.bind_to_toolpath(&tp, 0, tp.moves.len());
        }
        if let Some(scope) = debug_scope.as_ref()
            && !tp.moves.is_empty()
        {
            scope.set_move_range(0, tp.moves.len() - 1);
        }
    }
    if cfg.link_moves {
        let debug_scope = debug.map(|ctx| ctx.start_span("link_moves", "Apply link moves"));
        let debug_span_id = debug_scope.as_ref().map(|scope| scope.id());
        let semantic_scope = semantic.map(|ctx| {
            let scope = ctx.start_item(
                rs_cam_core::semantic_trace::ToolpathSemanticKind::Dressup,
                "Link moves",
            );
            if let Some(span_id) = debug_span_id {
                scope.set_debug_span_id(span_id);
            }
            scope.set_param("max_link_distance", cfg.link_max_distance);
            scope.set_param("link_feed_rate", cfg.link_feed_rate);
            scope
        });
        tp = apply_link_moves(
            &tp,
            &LinkMoveParams {
                max_link_distance: cfg.link_max_distance,
                link_feed_rate: cfg.link_feed_rate,
                safe_z_threshold: safe_z * 0.9,
            },
        );
        if let Some(scope) = semantic_scope.as_ref() {
            scope.bind_to_toolpath(&tp, 0, tp.moves.len());
        }
        if let Some(scope) = debug_scope.as_ref()
            && !tp.moves.is_empty()
        {
            scope.set_move_range(0, tp.moves.len() - 1);
        }
    }
    if cfg.arc_fitting {
        let debug_scope = debug.map(|ctx| ctx.start_span("arc_fit", "Fit arcs"));
        let debug_span_id = debug_scope.as_ref().map(|scope| scope.id());
        let semantic_scope = semantic.map(|ctx| {
            let scope = ctx.start_item(
                rs_cam_core::semantic_trace::ToolpathSemanticKind::Optimization,
                "Arc fitting",
            );
            if let Some(span_id) = debug_span_id {
                scope.set_debug_span_id(span_id);
            }
            scope.set_param("tolerance", cfg.arc_tolerance);
            scope
        });
        tp = fit_arcs(&tp, cfg.arc_tolerance);
        if let Some(scope) = semantic_scope.as_ref() {
            scope.bind_to_toolpath(&tp, 0, tp.moves.len());
        }
        if let Some(scope) = debug_scope.as_ref()
            && !tp.moves.is_empty()
        {
            scope.set_move_range(0, tp.moves.len() - 1);
        }
    }
    if cfg.feed_optimization {
        let debug_scope = debug.map(|ctx| ctx.start_span("feed_optimization", "Optimize feeds"));
        let debug_span_id = debug_scope.as_ref().map(|scope| scope.id());
        let semantic_scope = semantic.map(|ctx| {
            let scope = ctx.start_item(
                rs_cam_core::semantic_trace::ToolpathSemanticKind::Optimization,
                "Feed optimization",
            );
            if let Some(span_id) = debug_span_id {
                scope.set_debug_span_id(span_id);
            }
            scope.set_param("max_feed_rate", cfg.feed_max_rate);
            scope.set_param("ramp_rate", cfg.feed_ramp_rate);
            scope
        });
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
        if let Some(scope) = semantic_scope.as_ref() {
            scope.bind_to_toolpath(&tp, 0, tp.moves.len());
        }
        if let Some(scope) = debug_scope.as_ref()
            && !tp.moves.is_empty()
        {
            scope.set_move_range(0, tp.moves.len() - 1);
        }
    }
    if cfg.optimize_rapid_order {
        let debug_scope = debug.map(|ctx| ctx.start_span("rapid_order", "Optimize rapid order"));
        let debug_span_id = debug_scope.as_ref().map(|scope| scope.id());
        let semantic_scope = semantic.map(|ctx| {
            let scope = ctx.start_item(
                rs_cam_core::semantic_trace::ToolpathSemanticKind::Optimization,
                "Rapid ordering",
            );
            if let Some(span_id) = debug_span_id {
                scope.set_debug_span_id(span_id);
            }
            scope.set_param("safe_z", safe_z);
            scope
        });
        tp = rs_cam_core::tsp::optimize_rapid_order(&tp, safe_z);
        if let Some(scope) = semantic_scope.as_ref() {
            scope.bind_to_toolpath(&tp, 0, tp.moves.len());
        }
        if let Some(scope) = debug_scope.as_ref()
            && !tp.moves.is_empty()
        {
            scope.set_move_range(0, tp.moves.len() - 1);
        }
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

#[allow(dead_code)]
pub(super) fn run_collision_check(
    req: &CollisionRequest,
    cancel: &AtomicBool,
) -> Result<CollisionResult, ComputeError> {
    run_collision_check_with_phase(req, cancel, |_| {})
}

pub(super) fn run_collision_check_with_phase<F>(
    req: &CollisionRequest,
    cancel: &AtomicBool,
    mut set_phase: F,
) -> Result<CollisionResult, ComputeError>
where
    F: FnMut(&str),
{
    set_phase("Build collision index");
    let index = SpatialIndex::build_auto(&req.mesh);
    let assembly = ToolAssembly {
        cutter_radius: req.tool.diameter / 2.0,
        cutter_length: req.tool.cutting_length,
        shank_diameter: req.tool.shank_diameter,
        shank_length: req.tool.shank_length,
        holder_diameter: req.tool.holder_diameter,
        holder_length: req.tool.stickout - req.tool.cutting_length - req.tool.shank_length,
    };
    set_phase("Check collisions");
    let report = check_collisions_interpolated_with_cancel(
        &req.toolpath,
        &assembly,
        &req.mesh,
        &index,
        1.0,
        &|| cancel.load(Ordering::SeqCst),
    )
    .map_err(|_| ComputeError::Cancelled)?;
    set_phase("Collect collision markers");
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

pub(super) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate dir has workspace parent")
        .parent()
        .expect("workspace root available")
        .to_path_buf()
}

pub(super) fn debug_artifact_dir() -> PathBuf {
    workspace_root().join("target").join("toolpath_debug")
}

pub(super) fn build_trace_artifact(
    req: &ComputeRequest,
    debug_trace: Option<rs_cam_core::debug_trace::ToolpathDebugTrace>,
    semantic_trace: Option<rs_cam_core::semantic_trace::ToolpathSemanticTrace>,
) -> rs_cam_core::semantic_trace::ToolpathTraceArtifact {
    let stock_bbox = req.stock_bbox.as_ref().map(|bbox| {
        json!({
            "min": { "x": bbox.min.x, "y": bbox.min.y, "z": bbox.min.z },
            "max": { "x": bbox.max.x, "y": bbox.max.y, "z": bbox.max.z },
        })
    });

    let request_snapshot = json!({
        "toolpath_id": req.toolpath_id.0,
        "toolpath_name": req.toolpath_name,
        "operation": &req.operation,
        "operation_label": req.operation.label(),
        "dressups": &req.dressups,
        "stock_source": &req.stock_source,
        "tool": &req.tool,
        "safe_z": req.safe_z,
        "heights": {
            "clearance_z": req.heights.clearance_z,
            "retract_z": req.heights.retract_z,
            "feed_z": req.heights.feed_z,
            "top_z": req.heights.top_z,
            "bottom_z": req.heights.bottom_z,
        },
        "stock_bbox": stock_bbox,
        "boundary_enabled": req.boundary_enabled,
        "boundary_containment": format!("{:?}", req.boundary_containment),
        "keep_out_count": req.keep_out_footprints.len(),
        "debug_options": &req.debug_options,
    });

    rs_cam_core::semantic_trace::ToolpathTraceArtifact::new(
        req.toolpath_id.0,
        req.toolpath_name.clone(),
        req.operation.label(),
        req.tool.summary(),
        request_snapshot,
        debug_trace,
        semantic_trace,
    )
}
