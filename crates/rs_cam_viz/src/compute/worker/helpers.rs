use super::{AtomicBool, CollisionRequest, CollisionResult, ComputeError, ComputeRequest};
use serde_json::json;
use std::path::PathBuf;

// Re-export from core so existing callers (`simulation.rs`, `properties/`) keep working.
pub use rs_cam_core::compute::build_cutter;

// WP11b: `effective_safe_z`, `apply_dressups` and `feed_optimization_stock`
// are gone. The first read a request field that no longer exists; the other
// two were the viz half of one generation pipeline (tracker row N12), and
// `rs_cam_core::session::execute_job` runs both for the GUI now. The
// feed-optimisation stock the GUI door built — and the core door did not —
// is built inside `execute_job`, which is what closes N12 item 3.

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
        obstacles: req.obstacles.clone(),
        // One toolpath, one index. The per-model hoist is the batch
        // sweep's concern (CMP-24).
        index: None,
    };
    set_phase("Check collisions");
    let core_result =
        core_cc::run_collision_check(&core_req, cancel).map_err(|_e| ComputeError::Cancelled)?;
    set_phase("Collect collision markers");
    // CMP-24: the render positions are built HERE now. The core result
    // used to carry `Vec<[f32; 3]>`, a render type in the core model whose
    // only readers were the viewport, the GPU upload and the picker.
    let positions: Vec<[f32; 3]> = core_result
        .collision_report
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
    Ok(CollisionResult {
        report: core_result.collision_report,
        positions,
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

/// The debug artifact for one generation.
///
/// WP11b: the request no longer carries the operation, the dressups, the
/// tool or the heights — the handle does, and its fields are private. The
/// snapshot therefore comes from the handle's own reader, which names the
/// same things this function used to read field by field.
pub(super) fn build_trace_artifact(
    req: &ComputeRequest,
    debug_trace: Option<rs_cam_core::trace::debug_trace::ToolpathDebugTrace>,
    semantic_trace: Option<rs_cam_core::trace::semantic_trace::ToolpathSemanticTrace>,
) -> rs_cam_core::trace::semantic_trace::ToolpathTraceArtifact {
    rs_cam_core::trace::semantic_trace::ToolpathTraceArtifact::new(
        req.viz.toolpath_id,
        req.handle.toolpath_name().to_owned(),
        req.handle.op_label(),
        req.handle.tool_summary(),
        req.handle.request_snapshot(),
        debug_trace,
        semantic_trace,
    )
}

pub(super) fn build_simulation_cut_artifact(
    req: &rs_cam_core::compute::simulate::SimulationRequest,
    trace: rs_cam_core::stock::simulation_cut::SimulationCutTrace,
) -> rs_cam_core::stock::simulation_cut::SimulationCutArtifact {
    let included_toolpath_ids: Vec<_> = req
        .groups
        .iter()
        .flat_map(|group| group.toolpaths.iter().map(|toolpath| toolpath.id))
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
                        "tool": toolpath.tool_summary,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    });

    rs_cam_core::stock::simulation_cut::SimulationCutArtifact::new(
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
