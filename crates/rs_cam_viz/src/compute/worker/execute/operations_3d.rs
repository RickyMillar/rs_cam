use rs_cam_core::compute::build_cutter;
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::scallop::ScallopParams;
use rs_cam_core::tool::ToolDefinition;
use rs_cam_core::toolpath::Toolpath;

use super::super::helpers::{effective_safe_z, require_mesh};
use super::super::{ComputeRequest, ToolpathPhaseTracker};
use crate::compute::OperationError;
use crate::state::toolpath::ScallopConfig;

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

// SemanticToolpathOp implementations removed — dispatch now goes through
// rs_cam_core::compute::execute::execute_operation.
