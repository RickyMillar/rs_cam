use super::{
    AtomicBool, CollisionRequest, CollisionResult, ComputeError, ComputeRequest, SimulationRequest,
};
use serde_json::json;
use std::path::PathBuf;

use rs_cam_core::dexel_stock::TriDexelStock;

// Re-export from core so existing callers (`simulation.rs`, `properties/`) keep working.
pub use rs_cam_core::compute::build_cutter;

pub(super) fn effective_safe_z(req: &ComputeRequest) -> f64 {
    req.heights.retract_z
}

/// Apply all enabled dressup transforms to a computed toolpath.
///
/// Phase 4 / #44: this is now a thin wrapper around the core
/// [`rs_cam_core::compute::execute::apply_dressups`] pipeline. The viz layer
/// pre-builds the feed-optimization stock (which depends on GUI-only
/// `state::toolpath` operation-availability checks) and forwards the per-step
/// debug + semantic tracing contexts; everything else — capability gates,
/// dressup ordering, span remapping — lives in core.
pub(super) fn apply_dressups(
    annotated: rs_cam_core::toolpath_spans::AnnotatedToolpath,
    req: &ComputeRequest,
    debug: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
    semantic: Option<&rs_cam_core::semantic_trace::ToolpathSemanticContext>,
) -> rs_cam_core::toolpath_spans::AnnotatedToolpath {
    let cfg = &req.dressups;
    let tool = &req.tool;
    let safe_z = effective_safe_z(req);
    let transform_capabilities = req.operation.transform_capabilities();

    // Pre-build feed-optimization stock if requested. The GUI-only
    // availability check (`feed_optimization_stock`) must run here because
    // core has no knowledge of `state::toolpath`. If the check fails or the
    // stock can't be built we just don't pass one to core, which mirrors
    // the previous "skip with warning" behavior.
    let mut feed_opt_stock = None;
    if cfg.feed_optimization {
        match feed_optimization_stock(req) {
            Ok(stock) => feed_opt_stock = Some(stock),
            Err(reason) => {
                tracing::warn!(
                    "Skipping feed optimization for toolpath {}: {reason}",
                    req.toolpath_id.0
                );
            }
        }
    }
    let cutter = feed_opt_stock.as_ref().map(|_| build_cutter(tool));

    rs_cam_core::compute::execute::apply_dressups(
        annotated,
        cfg,
        tool.envelope_diameter(),
        safe_z,
        req.prior_stock.as_ref(),
        feed_opt_stock.as_mut(),
        cutter
            .as_ref()
            .map(|c| c as &dyn rs_cam_core::tool::MillingCutter),
        transform_capabilities,
        debug,
        semantic,
    )
}

pub(super) fn feed_optimization_stock(req: &ComputeRequest) -> Result<TriDexelStock, &'static str> {
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
    Ok(TriDexelStock::from_bounds(bbox, cell_size))
}

pub(super) fn run_collision_check_with_phase<F>(
    req: &CollisionRequest,
    cancel: &AtomicBool,
    mut set_phase: F,
) -> Result<CollisionResult, ComputeError>
where
    F: FnMut(&str),
{
    use rs_cam_core::compute::collision_check as core_cc;

    set_phase("Build collision index");
    let core_req = core_cc::CollisionCheckRequest {
        toolpath: &req.annotated.toolpath,
        tool: build_cutter(&req.tool),
        mesh: &req.mesh,
    };
    set_phase("Check collisions");
    let core_result =
        core_cc::run_collision_check(&core_req, cancel).map_err(|_e| ComputeError::Cancelled)?;
    set_phase("Collect collision markers");
    Ok(CollisionResult {
        report: core_result.collision_report,
        positions: core_result.collision_positions,
    })
}

// SAFETY: CARGO_MANIFEST_DIR always has two parent directories in a workspace layout
#[allow(clippy::expect_used)]
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

pub(super) fn simulation_metric_artifact_dir() -> PathBuf {
    workspace_root().join("target").join("simulation_metrics")
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
        "boundary_enabled": req.boundary.enabled,
        "boundary_containment": format!("{:?}", req.boundary.containment),
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

pub(super) fn build_simulation_cut_artifact(
    req: &SimulationRequest,
    trace: rs_cam_core::simulation_cut::SimulationCutTrace,
) -> rs_cam_core::simulation_cut::SimulationCutArtifact {
    let included_toolpath_ids: Vec<_> = req
        .groups
        .iter()
        .flat_map(|group| group.toolpaths.iter().map(|toolpath| toolpath.id.0))
        .collect();
    let request_snapshot = json!({
        "resolution_mm": req.resolution,
        "sample_step_mm": req.resolution.max(0.25),
        "metric_options": &req.metric_options,
        "spindle_rpm": req.spindle_rpm,
        "rapid_feed_mm_min": req.rapid_feed_mm_min,
        "stock_bbox": {
            "min": {
                "x": req.stock_bbox.min.x,
                "y": req.stock_bbox.min.y,
                "z": req.stock_bbox.min.z
            },
            "max": {
                "x": req.stock_bbox.max.x,
                "y": req.stock_bbox.max.y,
                "z": req.stock_bbox.max.z
            },
        },
        "toolpaths": req.groups.iter().map(|group| {
            json!({
                "direction": group.local_to_global.as_ref().map_or(
                    "FromTop".to_owned(),
                    |info| format!("{:?}", info.cut_direction()),
                ),
                "toolpaths": group.toolpaths.iter().map(|toolpath| {
                    json!({
                        "toolpath_id": toolpath.id.0,
                        "name": toolpath.name,
                        "tool": toolpath.tool.summary(),
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    });

    rs_cam_core::simulation_cut::SimulationCutArtifact::new(
        req.resolution,
        trace.sample_step_mm,
        [
            req.stock_bbox.min.x,
            req.stock_bbox.min.y,
            req.stock_bbox.min.z,
        ],
        [
            req.stock_bbox.max.x,
            req.stock_bbox.max.y,
            req.stock_bbox.max.z,
        ],
        included_toolpath_ids,
        request_snapshot,
        trace,
    )
}
