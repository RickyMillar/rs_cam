use rs_cam_core::dressup::{apply_tabs, even_tabs};
use rs_cam_core::inlay::{InlayParams, inlay_toolpaths};
use rs_cam_core::profile::{ProfileParams, profile_toolpath};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::zigzag::{ZigzagParams, zigzag_toolpath};

use super::super::helpers::{effective_safe_z, require_polygons};
use super::super::{ComputeRequest, ToolType};
use crate::compute::OperationError;
use crate::state::toolpath::{InlayConfig, ProfileConfig, ZigzagConfig};

pub(super) fn run_profile(
    req: &ComputeRequest,
    cfg: &ProfileConfig,
) -> Result<Toolpath, OperationError> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let side = cfg.side;
    let safe_z = effective_safe_z(req);
    let levels = &req.cutting_levels;
    let final_z = levels
        .last()
        .copied()
        .unwrap_or(req.heights.top_z - cfg.depth.abs());
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
                    compensate_in_controller: cfg.compensation
                        == rs_cam_core::compute::CompensationType::InControl,
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
                    pass_tp,
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

pub(super) fn run_inlay(
    req: &ComputeRequest,
    cfg: &InlayConfig,
) -> Result<Toolpath, OperationError> {
    let polys = require_polygons(req)?;
    let ha = match req.tool.tool_type {
        ToolType::VBit => (req.tool.included_angle / 2.0).to_radians(),
        _ => {
            return Err(OperationError::InvalidTool(
                "Inlay requires V-Bit tool".into(),
            ));
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

pub(super) fn run_zigzag(
    req: &ComputeRequest,
    cfg: &ZigzagConfig,
) -> Result<Toolpath, OperationError> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let safe_z = effective_safe_z(req);
    let mut out = Toolpath::new();
    for p in polys {
        let tp = rs_cam_core::depth::toolpath_at_levels(&req.cutting_levels, safe_z, |z| {
            zigzag_toolpath(
                p,
                &ZigzagParams {
                    tool_radius: tr,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z,
                    angle: cfg.angle,
                },
            )
        });
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

// SemanticToolpathOp implementations removed — dispatch now goes through
// rs_cam_core::compute::execute::execute_operation.
