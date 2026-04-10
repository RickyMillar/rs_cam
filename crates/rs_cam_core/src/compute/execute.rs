//! Unified operation execution — one entry point for all 23 toolpath operations.
//!
//! Both `ProjectSession` and the GUI compute worker delegate here so the
//! operation dispatch logic exists exactly once.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{DressupConfig, DressupEntryStyle, ResolvedHeights};
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::{ToolConfig, ToolType};
use crate::debug_trace::ToolpathDebugContext;
use crate::geo::BoundingBox3;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath::Toolpath;

// ── Error type ────────────────────────────────────────────────────────

/// Errors that can occur during operation execution.
#[derive(Debug, Clone)]
pub enum OperationError {
    /// Required geometry (mesh, polygons) is missing or invalid.
    MissingGeometry(String),
    /// Tool type doesn't match operation requirements.
    InvalidTool(String),
    /// Operation was cancelled.
    Cancelled,
    /// Other operation failure.
    Other(String),
}

// ── Annotation types ─────────────────────────────────────────────────

/// Runtime annotations returned alongside the toolpath.
/// Only operations whose algorithms emit structured events populate this.
pub enum OperationAnnotations {
    None,
    Adaptive3d(Vec<crate::adaptive3d::Adaptive3dRuntimeAnnotation>),
    Adaptive2d(Vec<crate::adaptive::AdaptiveRuntimeAnnotation>),
    Scallop(Vec<crate::scallop::ScallopRuntimeAnnotation>),
    RampFinish(Vec<crate::ramp_finish::RampFinishRuntimeAnnotation>),
    SpiralFinish(Vec<crate::spiral_finish::SpiralFinishRuntimeAnnotation>),
    Pencil(Vec<crate::pencil::PencilRuntimeAnnotation>),
}

pub struct AnnotatedToolpath {
    pub toolpath: Toolpath,
    pub annotations: OperationAnnotations,
}

impl std::fmt::Display for OperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingGeometry(s) => write!(f, "Missing geometry: {s}"),
            Self::InvalidTool(s) => write!(f, "Invalid tool: {s}"),
            Self::Cancelled => write!(f, "Operation cancelled"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for OperationError {}

impl From<String> for OperationError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

// ── Public API ────────────────────────────────────────────────────────

/// Execute a single operation, producing a raw toolpath.
///
/// Dispatches to the correct core algorithm based on the [`OperationConfig`]
/// variant. When `cutting_levels` is non-empty, depth-stepped operations use
/// those levels directly; otherwise they build levels from the heights config.
#[allow(clippy::too_many_arguments)]
pub fn execute_operation(
    op: &OperationConfig,
    mesh: Option<&TriangleMesh>,
    index: Option<&SpatialIndex>,
    polygons: Option<&[Polygon2]>,
    tool_def: &ToolDefinition,
    tool_cfg: &ToolConfig,
    heights: &ResolvedHeights,
    cutting_levels: &[f64],
    stock_bbox: &BoundingBox3,
    prev_tool_radius: Option<f64>,
    debug_ctx: Option<&ToolpathDebugContext>,
    cancel: &AtomicBool,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
) -> Result<Toolpath, OperationError> {
    execute_operation_annotated(
        op,
        mesh,
        index,
        polygons,
        tool_def,
        tool_cfg,
        heights,
        cutting_levels,
        stock_bbox,
        prev_tool_radius,
        debug_ctx,
        cancel,
        initial_stock,
    )
    .map(|at| at.toolpath)
}

/// Execute a single operation, producing a toolpath together with runtime
/// annotations from the underlying algorithm (when available).
///
/// This is the single source-of-truth dispatch; [`execute_operation`] delegates
/// here and discards the annotations for backwards compatibility.
#[allow(clippy::too_many_arguments)]
pub fn execute_operation_annotated(
    op: &OperationConfig,
    mesh: Option<&TriangleMesh>,
    index: Option<&SpatialIndex>,
    polygons: Option<&[Polygon2]>,
    tool_def: &ToolDefinition,
    tool_cfg: &ToolConfig,
    heights: &ResolvedHeights,
    cutting_levels: &[f64],
    stock_bbox: &BoundingBox3,
    prev_tool_radius: Option<f64>,
    debug_ctx: Option<&ToolpathDebugContext>,
    cancel: &AtomicBool,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
) -> Result<AnnotatedToolpath, OperationError> {
    let tool_radius = tool_def.radius();
    let safe_z = heights.retract_z;
    let feed_rate = op.feed_rate();
    let plunge_rate = op.plunge_rate();

    match op {
        OperationConfig::Face(cfg) => {
            let params = crate::face::FaceParams {
                tool_radius,
                stepover: cfg.stepover,
                depth: cfg.depth,
                depth_per_pass: cfg.depth_per_pass,
                feed_rate,
                plunge_rate,
                safe_z,
                stock_offset: cfg.stock_offset,
                direction: cfg.direction,
            };
            Ok(AnnotatedToolpath {
                toolpath: crate::face::face_toolpath(stock_bbox, &params),
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::Pocket(cfg) => {
            let polys = require_polygons(polygons)?;
            let levels = effective_levels(cutting_levels, heights, cfg.depth_per_pass);
            let mut combined = Toolpath::new();
            for poly in polys {
                let tp = crate::depth::toolpath_at_levels(&levels, safe_z, |z| match cfg.pattern {
                    crate::compute::operation_configs::PocketPattern::Contour => {
                        crate::pocket::pocket_toolpath(
                            poly,
                            &crate::pocket::PocketParams {
                                tool_radius,
                                stepover: cfg.stepover,
                                cut_depth: z,
                                feed_rate,
                                plunge_rate,
                                safe_z,
                                climb: cfg.climb,
                            },
                        )
                    }
                    crate::compute::operation_configs::PocketPattern::Zigzag => {
                        crate::zigzag::zigzag_toolpath(
                            poly,
                            &crate::zigzag::ZigzagParams {
                                tool_radius,
                                stepover: cfg.stepover,
                                cut_depth: z,
                                feed_rate,
                                plunge_rate,
                                safe_z,
                                angle: cfg.angle,
                            },
                        )
                    }
                });
                combined.moves.extend(tp.moves);
            }
            Ok(AnnotatedToolpath {
                toolpath: combined,
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::Profile(cfg) => {
            let polys = require_polygons(polygons)?;
            let levels = effective_levels(cutting_levels, heights, cfg.depth_per_pass);
            let final_z = levels.last().copied().unwrap_or(heights.top_z - cfg.depth);
            let mut combined = Toolpath::new();
            for poly in polys {
                for (level_idx, &z) in levels.iter().enumerate() {
                    let pass_tp = crate::profile::profile_toolpath(
                        poly,
                        &crate::profile::ProfileParams {
                            tool_radius,
                            side: cfg.side,
                            cut_depth: z,
                            feed_rate,
                            plunge_rate,
                            safe_z,
                            climb: cfg.climb,
                            compensate_in_controller: cfg.compensation
                                == crate::compute::CompensationType::InControl,
                        },
                    );
                    if pass_tp.moves.is_empty() {
                        continue;
                    }
                    // Retract between levels (not before first)
                    if level_idx > 0 && !combined.moves.is_empty() {
                        combined.final_retract(safe_z);
                    }
                    let is_final = (z - final_z).abs() < 1e-9;
                    if cfg.tab_count > 0 && is_final {
                        let tabbed = crate::dressup::apply_tabs(
                            pass_tp,
                            &crate::dressup::even_tabs(
                                cfg.tab_count,
                                cfg.tab_width,
                                cfg.tab_height,
                            ),
                            z,
                        );
                        combined.moves.extend(tabbed.moves);
                    } else {
                        combined.moves.extend(pass_tp.moves);
                    }
                }
            }
            Ok(AnnotatedToolpath {
                toolpath: combined,
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::Adaptive(cfg) => {
            let polys = require_polygons(polygons)?;
            let levels = effective_levels(cutting_levels, heights, cfg.depth_per_pass);
            let cancel_fn = || cancel.load(Ordering::SeqCst);
            let mut combined = Toolpath::new();
            let mut all_annotations = Vec::new();
            for poly in polys {
                for (level_i, &z) in levels.iter().enumerate() {
                    let params = crate::adaptive::AdaptiveParams {
                        tool_radius,
                        stepover: cfg.stepover,
                        cut_depth: z,
                        feed_rate,
                        plunge_rate,
                        safe_z,
                        tolerance: cfg.tolerance,
                        slot_clearing: cfg.slot_clearing,
                        min_cutting_radius: cfg.min_cutting_radius,
                        initial_stock: initial_stock.cloned(),
                    };
                    let (level_tp, mut annotations) =
                        crate::adaptive::adaptive_toolpath_structured_annotated_traced_with_cancel(
                            poly, &params, &cancel_fn, debug_ctx,
                        )
                        .map_err(|_cancelled| OperationError::Cancelled)?;
                    if level_tp.moves.is_empty() {
                        continue;
                    }
                    // Inter-level retract (matching toolpath_at_levels behaviour)
                    if level_i > 0 {
                        let offset_before = combined.moves.len();
                        combined.final_retract(safe_z);
                        // Account for any retract moves added
                        let retract_added = combined.moves.len() - offset_before;
                        // Offset annotation move_index values
                        let offset = offset_before + retract_added;
                        for ann in &mut annotations {
                            ann.move_index += offset;
                        }
                    } else {
                        let offset = combined.moves.len();
                        for ann in &mut annotations {
                            ann.move_index += offset;
                        }
                    }
                    all_annotations.extend(annotations);
                    combined.moves.extend(level_tp.moves);
                }
            }
            Ok(AnnotatedToolpath {
                toolpath: combined,
                annotations: OperationAnnotations::Adaptive2d(all_annotations),
            })
        }
        OperationConfig::Zigzag(cfg) => {
            let polys = require_polygons(polygons)?;
            let levels = effective_levels(cutting_levels, heights, cfg.depth_per_pass);
            let mut combined = Toolpath::new();
            for poly in polys {
                let tp = crate::depth::toolpath_at_levels(&levels, safe_z, |z| {
                    crate::zigzag::zigzag_toolpath(
                        poly,
                        &crate::zigzag::ZigzagParams {
                            tool_radius,
                            stepover: cfg.stepover,
                            cut_depth: z,
                            feed_rate,
                            plunge_rate,
                            safe_z,
                            angle: cfg.angle,
                        },
                    )
                });
                combined.moves.extend(tp.moves);
            }
            Ok(AnnotatedToolpath {
                toolpath: combined,
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::Trace(cfg) => {
            let polys = require_polygons(polygons)?;
            let levels = effective_levels(cutting_levels, heights, cfg.depth_per_pass);
            let mut combined = Toolpath::new();
            for poly in polys {
                let params = crate::trace::TraceParams {
                    tool_radius,
                    depth: cfg.depth,
                    depth_per_pass: cfg.depth_per_pass,
                    feed_rate,
                    plunge_rate,
                    safe_z,
                    compensation: cfg.compensation,
                    top_z: heights.top_z,
                };
                let tp = crate::depth::toolpath_at_levels(&levels, safe_z, |z| {
                    crate::trace::trace_polygon_at_z(poly, z, &params)
                });
                combined.moves.extend(tp.moves);
            }
            Ok(AnnotatedToolpath {
                toolpath: combined,
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::VCarve(cfg) => {
            let polys = require_polygons(polygons)?;
            let ha = match tool_cfg.tool_type {
                ToolType::VBit => (tool_cfg.included_angle / 2.0).to_radians(),
                _ => {
                    return Err(OperationError::InvalidTool(
                        "VCarve requires V-Bit tool".into(),
                    ));
                }
            };
            let mut combined = Toolpath::new();
            for poly in polys {
                let tp = crate::vcarve::vcarve_toolpath(
                    poly,
                    &crate::vcarve::VCarveParams {
                        half_angle: ha,
                        max_depth: cfg.max_depth,
                        stepover: cfg.stepover,
                        feed_rate,
                        plunge_rate,
                        safe_z,
                        tolerance: cfg.tolerance,
                    },
                );
                combined.moves.extend(tp.moves);
            }
            Ok(AnnotatedToolpath {
                toolpath: combined,
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::Rest(cfg) => {
            let polys = require_polygons(polygons)?;
            let ptr = prev_tool_radius.ok_or_else(|| {
                OperationError::Other("Previous tool not set for rest machining".into())
            })?;
            let levels = effective_levels(cutting_levels, heights, cfg.depth_per_pass);
            let mut combined = Toolpath::new();
            for poly in polys {
                let tp = crate::depth::toolpath_at_levels(&levels, safe_z, |z| {
                    crate::rest::rest_machining_toolpath(
                        poly,
                        &crate::rest::RestParams {
                            prev_tool_radius: ptr,
                            tool_radius,
                            cut_depth: z,
                            stepover: cfg.stepover,
                            feed_rate,
                            plunge_rate,
                            safe_z,
                            angle: cfg.angle,
                        },
                    )
                });
                combined.moves.extend(tp.moves);
            }
            Ok(AnnotatedToolpath {
                toolpath: combined,
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::Inlay(cfg) => {
            let polys = require_polygons(polygons)?;
            let ha = match tool_cfg.tool_type {
                ToolType::VBit => (tool_cfg.included_angle / 2.0).to_radians(),
                _ => {
                    return Err(OperationError::InvalidTool(
                        "Inlay requires V-Bit tool".into(),
                    ));
                }
            };
            let mut female_out = Toolpath::new();
            let mut male_out = Toolpath::new();
            for poly in polys {
                let r = crate::inlay::inlay_toolpaths(
                    poly,
                    &crate::inlay::InlayParams {
                        half_angle: ha,
                        pocket_depth: cfg.pocket_depth,
                        glue_gap: cfg.glue_gap,
                        flat_depth: cfg.flat_depth,
                        boundary_offset: cfg.boundary_offset,
                        stepover: cfg.stepover,
                        flat_tool_radius: cfg.flat_tool_radius,
                        feed_rate,
                        plunge_rate,
                        safe_z,
                        tolerance: cfg.tolerance,
                    },
                );
                female_out.moves.extend(r.female.moves);
                male_out.moves.extend(r.male.moves);
            }
            let mut out = female_out;
            if !male_out.moves.is_empty() {
                out.final_retract(safe_z);
                out.moves.extend(male_out.moves);
            }
            Ok(AnnotatedToolpath {
                toolpath: out,
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::Drill(cfg) => {
            let polys = require_polygons(polygons)?;
            let mut holes = Vec::new();
            for poly in polys {
                if poly.exterior.is_empty() {
                    continue;
                }
                let (sx, sy) = poly
                    .exterior
                    .iter()
                    .fold((0.0, 0.0), |(ax, ay), pt| (ax + pt.x, ay + pt.y));
                let n = poly.exterior.len() as f64;
                holes.push([sx / n, sy / n]);
            }
            if holes.is_empty() {
                return Err(OperationError::MissingGeometry(
                    "No hole positions found (import SVG with circles)".to_owned(),
                ));
            }
            let cycle = cfg.cycle.to_core(cfg);
            let params = crate::drill::DrillParams {
                depth: cfg.depth,
                cycle,
                feed_rate,
                safe_z,
                retract_z: cfg.retract_z,
            };
            Ok(AnnotatedToolpath {
                toolpath: crate::drill::drill_toolpath(&holes, &params),
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::Chamfer(cfg) => {
            let polys = require_polygons(polygons)?;
            let ha = match tool_cfg.tool_type {
                ToolType::VBit => (tool_cfg.included_angle / 2.0).to_radians(),
                _ => {
                    return Err(OperationError::InvalidTool(
                        "Chamfer requires V-Bit tool".into(),
                    ));
                }
            };
            let mut combined = Toolpath::new();
            for poly in polys {
                let params = crate::chamfer::ChamferParams {
                    chamfer_width: cfg.chamfer_width,
                    tip_offset: cfg.tip_offset,
                    tool_half_angle: ha,
                    tool_radius,
                    feed_rate,
                    plunge_rate,
                    safe_z,
                };
                let tp = crate::chamfer::chamfer_toolpath(poly, &params);
                combined.moves.extend(tp.moves);
            }
            Ok(AnnotatedToolpath {
                toolpath: combined,
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::AlignmentPinDrill(cfg) => {
            if cfg.holes.is_empty() {
                return Err(OperationError::MissingGeometry(
                    "No alignment pin positions defined".to_owned(),
                ));
            }
            let stock_z = stock_bbox.max.z - stock_bbox.min.z;
            let depth = stock_z + cfg.spoilboard_penetration;
            let cycle = match cfg.cycle {
                crate::compute::operation_configs::DrillCycleType::Simple => {
                    crate::drill::DrillCycle::Simple
                }
                crate::compute::operation_configs::DrillCycleType::Dwell => {
                    crate::drill::DrillCycle::Dwell(0.5)
                }
                crate::compute::operation_configs::DrillCycleType::Peck => {
                    crate::drill::DrillCycle::Peck(cfg.peck_depth)
                }
                crate::compute::operation_configs::DrillCycleType::ChipBreak => {
                    crate::drill::DrillCycle::ChipBreak(cfg.peck_depth, 0.5)
                }
            };
            let params = crate::drill::DrillParams {
                depth,
                cycle,
                feed_rate: cfg.feed_rate,
                safe_z,
                retract_z: cfg.retract_z,
            };
            Ok(AnnotatedToolpath {
                toolpath: crate::drill::drill_toolpath(&cfg.holes, &params),
                annotations: OperationAnnotations::None,
            })
        }

        // ── 3D operations ────────────────────────────────────────────
        OperationConfig::DropCutter(cfg) => {
            let m = require_mesh(mesh)?;
            let idx = index.ok_or_else(|| {
                OperationError::Other("DropCutter requires a spatial index".into())
            })?;
            // Clamp min_z so the drop-cutter grid never emits moves below
            // the stock bottom.  The default (-50) would destroy the stock
            // during simulation via ray_subtract_above.
            let effective_min_z = cfg.min_z.max(stock_bbox.min.z - 1.0);
            let grid = crate::dropcutter::batch_drop_cutter_with_cancel(
                m,
                idx,
                tool_def,
                cfg.stepover,
                0.0,
                effective_min_z,
                &(|| cancel.load(Ordering::SeqCst)),
            )
            .map_err(|_e| OperationError::Cancelled)?;
            // Pass min_z to toolpath generation so zero-engagement passes
            // (where the entire segment is clamped at min_z) are skipped.
            let min_z_filter = if cfg.min_z > stock_bbox.min.z - 0.5 {
                Some(effective_min_z)
            } else {
                None
            };
            let slope_filter_active = cfg.slope_from > 0.01 || cfg.slope_to < 89.99;
            let tp = if slope_filter_active {
                let slope_angles = crate::dropcutter::compute_grid_slopes(&grid);
                crate::toolpath::raster_toolpath_from_grid_with_slope_filter(
                    &grid,
                    &slope_angles,
                    cfg.slope_from,
                    cfg.slope_to,
                    feed_rate,
                    plunge_rate,
                    safe_z,
                    min_z_filter,
                )
            } else {
                crate::toolpath::raster_toolpath_from_grid(
                    &grid,
                    feed_rate,
                    plunge_rate,
                    safe_z,
                    min_z_filter,
                )
            };
            Ok(AnnotatedToolpath {
                toolpath: tp,
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::Adaptive3d(cfg) => {
            let m = require_mesh(mesh)?;
            let idx = index.ok_or_else(|| {
                OperationError::Other("Adaptive3D requires a spatial index".into())
            })?;

            // Constants matching the GUI defaults for entry style params
            const HELIX_RADIUS_FACTOR: f64 = 0.3;
            const HELIX_PITCH_MM: f64 = 2.0;
            const RAMP_ANGLE_DEG: f64 = 10.0;

            let entry_style = match cfg.entry_style {
                crate::compute::operation_configs::Adaptive3dEntryStyle::Plunge => {
                    crate::adaptive3d::EntryStyle3d::Plunge
                }
                crate::compute::operation_configs::Adaptive3dEntryStyle::Ramp => {
                    crate::adaptive3d::EntryStyle3d::Ramp {
                        max_angle_deg: RAMP_ANGLE_DEG,
                    }
                }
                crate::compute::operation_configs::Adaptive3dEntryStyle::Helix => {
                    crate::adaptive3d::EntryStyle3d::Helix {
                        radius: tool_cfg.diameter * HELIX_RADIUS_FACTOR,
                        pitch: HELIX_PITCH_MM,
                    }
                }
            };
            let region_ordering = match cfg.region_ordering {
                crate::compute::operation_configs::RegionOrdering::Global => {
                    crate::adaptive3d::RegionOrdering::Global
                }
                crate::compute::operation_configs::RegionOrdering::ByArea => {
                    crate::adaptive3d::RegionOrdering::ByArea
                }
            };
            let clearing_strategy = match cfg.clearing_strategy {
                crate::compute::operation_configs::ClearingStrategy::ContourParallel => {
                    crate::adaptive3d::ClearingStrategy3d::ContourParallel
                }
                crate::compute::operation_configs::ClearingStrategy::Adaptive => {
                    crate::adaptive3d::ClearingStrategy3d::Adaptive
                }
            };
            let params = crate::adaptive3d::Adaptive3dParams {
                tool_radius,
                stepover: cfg.stepover,
                depth_per_pass: cfg.depth_per_pass,
                stock_to_leave: cfg.stock_to_leave_axial.max(cfg.stock_to_leave_radial),
                feed_rate,
                plunge_rate,
                tolerance: cfg.tolerance,
                min_cutting_radius: cfg.min_cutting_radius,
                stock_top_z: stock_bbox.max.z,
                entry_style,
                fine_stepdown: if cfg.fine_stepdown > 0.0 {
                    Some(cfg.fine_stepdown)
                } else {
                    None
                },
                detect_flat_areas: cfg.detect_flat_areas,
                max_stay_down_dist: None,
                region_ordering,
                initial_stock: initial_stock.cloned(),
                safe_z,
                clearing_strategy,
                z_blend: cfg.z_blend,
            };
            let (tp, annotations) =
                crate::adaptive3d::adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
                    m,
                    idx,
                    tool_def,
                    &params,
                    &(|| cancel.load(Ordering::SeqCst)),
                    debug_ctx,
                )
                .map_err(|_e| OperationError::Cancelled)?;
            Ok(AnnotatedToolpath {
                toolpath: tp,
                annotations: OperationAnnotations::Adaptive3d(annotations),
            })
        }
        OperationConfig::Waterline(cfg) => {
            let m = require_mesh(mesh)?;
            let idx = index.ok_or_else(|| {
                OperationError::Other("Waterline requires a spatial index".into())
            })?;
            let params = crate::waterline::WaterlineParams {
                sampling: cfg.sampling,
                feed_rate,
                plunge_rate,
                safe_z,
            };
            let tp = crate::waterline::waterline_toolpath_with_cancel(
                m,
                idx,
                tool_def,
                heights.top_z,
                heights.bottom_z,
                cfg.z_step,
                &params,
                &(|| cancel.load(Ordering::SeqCst)),
            )
            .map_err(|_e| OperationError::Cancelled)?;
            Ok(AnnotatedToolpath {
                toolpath: tp,
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::Pencil(cfg) => {
            let m = require_mesh(mesh)?;
            let idx = index
                .ok_or_else(|| OperationError::Other("Pencil requires a spatial index".into()))?;
            let params = crate::pencil::PencilParams {
                bitangency_angle: cfg.bitangency_angle,
                min_cut_length: cfg.min_cut_length,
                hookup_distance: cfg.hookup_distance,
                num_offset_passes: cfg.num_offset_passes,
                offset_stepover: cfg.offset_stepover,
                sampling: cfg.sampling,
                feed_rate,
                plunge_rate,
                safe_z,
                stock_to_leave: cfg.stock_to_leave,
            };
            let (tp, annotations) = crate::pencil::pencil_toolpath_structured_annotated(
                m, idx, tool_def, &params, debug_ctx,
            );
            Ok(AnnotatedToolpath {
                toolpath: tp,
                annotations: OperationAnnotations::Pencil(annotations),
            })
        }
        OperationConfig::Scallop(cfg) => {
            if !tool_cfg.tool_type.has_ball_tip() {
                return Err(OperationError::InvalidTool(
                    "Scallop requires a ball-tip tool (Ball Nose or Tapered Ball Nose)".into(),
                ));
            }
            let m = require_mesh(mesh)?;
            let idx = index
                .ok_or_else(|| OperationError::Other("Scallop requires a spatial index".into()))?;
            let params = crate::scallop::ScallopParams {
                scallop_height: cfg.scallop_height,
                tolerance: cfg.tolerance,
                direction: cfg.direction,
                continuous: cfg.continuous,
                slope_from: cfg.slope_from,
                slope_to: cfg.slope_to,
                feed_rate,
                plunge_rate,
                safe_z,
                stock_to_leave: cfg.stock_to_leave,
            };
            let (tp, annotations) = crate::scallop::scallop_toolpath_structured_annotated(
                m, idx, tool_def, &params, debug_ctx,
            );
            Ok(AnnotatedToolpath {
                toolpath: tp,
                annotations: OperationAnnotations::Scallop(annotations),
            })
        }
        OperationConfig::SteepShallow(cfg) => {
            let m = require_mesh(mesh)?;
            let idx = index.ok_or_else(|| {
                OperationError::Other("SteepShallow requires a spatial index".into())
            })?;
            let params = crate::steep_shallow::SteepShallowParams {
                threshold_angle: cfg.threshold_angle,
                overlap_distance: cfg.overlap_distance,
                wall_clearance: cfg.wall_clearance,
                steep_first: cfg.steep_first,
                stepover: cfg.stepover,
                z_step: cfg.z_step,
                feed_rate,
                plunge_rate,
                safe_z,
                sampling: cfg.sampling,
                stock_to_leave: cfg.stock_to_leave,
                tolerance: cfg.tolerance,
            };
            Ok(AnnotatedToolpath {
                toolpath: crate::steep_shallow::steep_shallow_toolpath(m, idx, tool_def, &params),
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::RampFinish(cfg) => {
            let m = require_mesh(mesh)?;
            let idx = index.ok_or_else(|| {
                OperationError::Other("RampFinish requires a spatial index".into())
            })?;
            let params = crate::ramp_finish::RampFinishParams {
                max_stepdown: cfg.max_stepdown,
                slope_from: cfg.slope_from,
                slope_to: cfg.slope_to,
                direction: cfg.direction,
                order_bottom_up: cfg.order_bottom_up,
                feed_rate,
                plunge_rate,
                safe_z,
                sampling: cfg.sampling,
                stock_to_leave: cfg.stock_to_leave,
                tolerance: cfg.tolerance,
            };
            let (tp, annotations) = crate::ramp_finish::ramp_finish_toolpath_structured_annotated(
                m, idx, tool_def, &params, debug_ctx,
            );
            Ok(AnnotatedToolpath {
                toolpath: tp,
                annotations: OperationAnnotations::RampFinish(annotations),
            })
        }
        OperationConfig::SpiralFinish(cfg) => {
            let m = require_mesh(mesh)?;
            let idx = index.ok_or_else(|| {
                OperationError::Other("SpiralFinish requires a spatial index".into())
            })?;
            let params = crate::spiral_finish::SpiralFinishParams {
                stepover: cfg.stepover,
                direction: cfg.direction,
                feed_rate,
                plunge_rate,
                safe_z,
                stock_to_leave: cfg.stock_to_leave,
            };
            let (tp, annotations) =
                crate::spiral_finish::spiral_finish_toolpath_structured_annotated(
                    m, idx, tool_def, &params, debug_ctx,
                );
            Ok(AnnotatedToolpath {
                toolpath: tp,
                annotations: OperationAnnotations::SpiralFinish(annotations),
            })
        }
        OperationConfig::RadialFinish(cfg) => {
            let m = require_mesh(mesh)?;
            let idx = index.ok_or_else(|| {
                OperationError::Other("RadialFinish requires a spatial index".into())
            })?;
            let params = crate::radial_finish::RadialFinishParams {
                angular_step: cfg.angular_step,
                point_spacing: cfg.point_spacing,
                feed_rate,
                plunge_rate,
                safe_z,
                stock_to_leave: cfg.stock_to_leave,
            };
            Ok(AnnotatedToolpath {
                toolpath: crate::radial_finish::radial_finish_toolpath(m, idx, tool_def, &params),
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::HorizontalFinish(cfg) => {
            let m = require_mesh(mesh)?;
            let idx = index.ok_or_else(|| {
                OperationError::Other("HorizontalFinish requires a spatial index".into())
            })?;
            let params = crate::horizontal_finish::HorizontalFinishParams {
                angle_threshold: cfg.angle_threshold,
                stepover: cfg.stepover,
                feed_rate,
                plunge_rate,
                safe_z,
                stock_to_leave: cfg.stock_to_leave,
            };
            Ok(AnnotatedToolpath {
                toolpath: crate::horizontal_finish::horizontal_finish_toolpath(
                    m, idx, tool_def, &params,
                ),
                annotations: OperationAnnotations::None,
            })
        }
        OperationConfig::ProjectCurve(cfg) => {
            let polys = require_polygons(polygons)?;
            let m = require_mesh(mesh)?;
            let idx = index.ok_or_else(|| {
                OperationError::Other("ProjectCurve requires a spatial index".into())
            })?;
            let cutter = build_cutter(tool_cfg);
            let direction = match cfg.direction {
                crate::compute::operation_configs::ProjectCurveDirection::FromAbove => {
                    crate::project_curve::ProjectDirection::FromAbove
                }
                crate::compute::operation_configs::ProjectCurveDirection::FromBelow => {
                    crate::project_curve::ProjectDirection::FromBelow
                }
            };
            let params = crate::project_curve::ProjectCurveParams {
                depth: cfg.depth,
                point_spacing: cfg.point_spacing,
                feed_rate,
                plunge_rate,
                safe_z,
                direction,
                setup_z_flipped: cfg.setup_z_flipped,
            };
            let mut combined = Toolpath::new();
            for poly in polys {
                let tp =
                    crate::project_curve::project_curve_toolpath(poly, m, idx, &cutter, &params);
                combined.moves.extend(tp.moves);
            }
            Ok(AnnotatedToolpath {
                toolpath: combined,
                annotations: OperationAnnotations::None,
            })
        }
    }
}

/// Apply the standard dressup pipeline to a toolpath.
///
/// Steps: entry style → dogbones → lead in/out → link moves → arc fitting
/// → rapid order optimization → air-cut filter → feed rate optimization.
///
/// The last two steps are optional and require additional context:
/// - Air-cut filter needs `prior_stock` (the stock state before this toolpath).
/// - Feed optimization needs a mutable stock copy, a cutter, and `cfg.feed_optimization == true`.
pub fn apply_dressups(
    mut tp: Toolpath,
    cfg: &DressupConfig,
    tool_diameter: f64,
    safe_z: f64,
    prior_stock: Option<&crate::dexel_stock::TriDexelStock>,
    feed_opt_stock: Option<&mut crate::dexel_stock::TriDexelStock>,
    cutter: Option<&dyn MillingCutter>,
) -> Toolpath {
    use crate::dressup::{
        EntryStyle, LinkMoveParams, apply_dogbones, apply_entry, apply_lead_in_out,
        apply_link_moves,
    };

    let tool_radius = tool_diameter / 2.0;

    // Determine plunge rate from toolpath
    let plunge_rate = tp
        .moves
        .iter()
        .find_map(|m| match m.move_type {
            crate::toolpath::MoveType::Linear { feed_rate } => Some(feed_rate * 0.5),
            _ => None,
        })
        .unwrap_or(500.0);

    // 1. Entry style
    match cfg.entry_style {
        DressupEntryStyle::Ramp => {
            tp = apply_entry(
                tp,
                EntryStyle::Ramp {
                    max_angle_deg: cfg.ramp_angle,
                },
                plunge_rate,
            );
        }
        DressupEntryStyle::Helix => {
            tp = apply_entry(
                tp,
                EntryStyle::Helix {
                    radius: cfg.helix_radius,
                    pitch: cfg.helix_pitch,
                },
                plunge_rate,
            );
        }
        DressupEntryStyle::None => {}
    }

    // 2. Dogbones
    if cfg.dogbone {
        tp = apply_dogbones(tp, tool_radius, cfg.dogbone_angle);
    }

    // 3. Lead in/out
    if cfg.lead_in_out {
        tp = apply_lead_in_out(tp, cfg.lead_radius);
    }

    // 4. Link moves
    if cfg.link_moves {
        tp = apply_link_moves(
            tp,
            &LinkMoveParams {
                max_link_distance: cfg.link_max_distance,
                link_feed_rate: cfg.link_feed_rate,
                safe_z_threshold: safe_z * 0.9,
            },
        );
    }

    // 5. Arc fitting
    if cfg.arc_fitting {
        tp = crate::arcfit::fit_arcs(&tp, cfg.arc_tolerance);
    }

    // 6. Rapid order optimization
    if cfg.optimize_rapid_order {
        tp = crate::tsp::optimize_rapid_order(&tp, safe_z);
    }

    // 7. Air-cut filter (when prior stock is available)
    if let Some(stock) = prior_stock {
        tp = crate::dressup::filter_air_cuts(tp, stock, tool_radius, safe_z, 0.1);
    }

    // 8. Feed rate optimization (when enabled and stock + cutter are available)
    if cfg.feed_optimization
        && let (Some(stock), Some(cut)) = (feed_opt_stock, cutter)
    {
        let nominal = tp
            .moves
            .iter()
            .find_map(|m| match m.move_type {
                crate::toolpath::MoveType::Linear { feed_rate } => Some(feed_rate),
                _ => None,
            })
            .unwrap_or(1000.0);
        let params = crate::feedopt::FeedOptParams {
            nominal_feed_rate: nominal,
            max_feed_rate: cfg.feed_max_rate,
            min_feed_rate: nominal * 0.5,
            ramp_rate: cfg.feed_ramp_rate,
            air_cut_threshold: 0.05,
        };
        tp = crate::feedopt::optimize_feed_rates(&tp, cut, stock, &params);
    }

    tp
}

// ── Internal helpers ──────────────────────────────────────────────────

fn require_polygons(polygons: Option<&[Polygon2]>) -> Result<&[Polygon2], OperationError> {
    polygons
        .filter(|p| !p.is_empty())
        .ok_or_else(|| OperationError::MissingGeometry("Operation requires 2D geometry".into()))
}

fn require_mesh(mesh: Option<&TriangleMesh>) -> Result<&TriangleMesh, OperationError> {
    mesh.ok_or_else(|| OperationError::MissingGeometry("Operation requires a 3D mesh".into()))
}

/// Compute effective depth levels from pre-computed cutting_levels or DepthStepping.
fn effective_levels(
    cutting_levels: &[f64],
    heights: &ResolvedHeights,
    depth_per_pass: f64,
) -> Vec<f64> {
    if !cutting_levels.is_empty() {
        cutting_levels.to_vec()
    } else {
        let stepping = crate::depth::DepthStepping::new(
            heights.top_z,
            heights.top_z - heights.depth(),
            depth_per_pass,
        );
        stepping.all_levels()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    use crate::compute::catalog::{OperationConfig, OperationType};
    use crate::compute::config::ResolvedHeights;
    use crate::compute::cutter::build_cutter;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::geo::{BoundingBox3, P3};
    use crate::polygon::Polygon2;

    /// Build a default tool definition and config for a given tool type.
    fn make_tool(tool_type: ToolType) -> (crate::tool::ToolDefinition, ToolConfig) {
        let cfg = ToolConfig::new_default(ToolId(0), tool_type);
        let def = build_cutter(&cfg);
        (def, cfg)
    }

    /// Sensible resolved heights for testing.
    fn test_heights() -> ResolvedHeights {
        ResolvedHeights {
            clearance_z: 40.0,
            retract_z: 30.0,
            feed_z: 28.0,
            top_z: 25.0,
            bottom_z: 0.0,
        }
    }

    /// A stock bbox suitable for most tests.
    fn test_stock_bbox() -> BoundingBox3 {
        BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 25.0),
        }
    }

    #[test]
    fn missing_polygons_error_for_2d_operation() {
        let op = OperationConfig::new_default(OperationType::Profile);
        let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
        let heights = test_heights();
        let bbox = test_stock_bbox();
        let cancel = AtomicBool::new(false);

        let result = execute_operation(
            &op,
            None,
            None,
            None, // no polygons
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            None,
            None,
            &cancel,
            None,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, OperationError::MissingGeometry(_)),
            "Expected MissingGeometry, got: {err:?}"
        );
    }

    #[test]
    fn missing_mesh_error_for_3d_operation() {
        let op = OperationConfig::new_default(OperationType::DropCutter);
        let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
        let heights = test_heights();
        let bbox = test_stock_bbox();
        let cancel = AtomicBool::new(false);

        let result = execute_operation(
            &op,
            None, // no mesh
            None,
            None,
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            None,
            None,
            &cancel,
            None,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, OperationError::MissingGeometry(_)),
            "Expected MissingGeometry, got: {err:?}"
        );
    }

    #[test]
    fn invalid_tool_for_vcarve() {
        let op = OperationConfig::new_default(OperationType::VCarve);
        let (tool_def, tool_cfg) = make_tool(ToolType::EndMill); // not a V-Bit
        let heights = test_heights();
        let bbox = test_stock_bbox();
        let cancel = AtomicBool::new(false);
        let polys = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];

        let result = execute_operation(
            &op,
            None,
            None,
            Some(&polys),
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            None,
            None,
            &cancel,
            None,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, OperationError::InvalidTool(_)),
            "Expected InvalidTool, got: {err:?}"
        );
    }

    #[test]
    fn invalid_tool_for_scallop() {
        let op = OperationConfig::new_default(OperationType::Scallop);
        let (tool_def, tool_cfg) = make_tool(ToolType::EndMill); // not ball nose
        let heights = test_heights();
        let bbox = test_stock_bbox();
        let cancel = AtomicBool::new(false);

        // Scallop requires a mesh, but the tool check happens before mesh access
        let result = execute_operation(
            &op,
            None,
            None,
            None,
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            None,
            None,
            &cancel,
            None,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, OperationError::InvalidTool(_)),
            "Expected InvalidTool, got: {err:?}"
        );
    }

    #[test]
    fn face_produces_output() {
        let op = OperationConfig::new_default(OperationType::Face);
        let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
        let heights = test_heights();
        let bbox = test_stock_bbox();
        let cancel = AtomicBool::new(false);

        let result = execute_operation(
            &op,
            None,
            None,
            None,
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            None,
            None,
            &cancel,
            None,
        );

        assert!(result.is_ok(), "Face should succeed, got: {result:?}");
        let tp = result.unwrap();
        assert!(
            !tp.moves.is_empty(),
            "Face toolpath should contain at least one move"
        );
    }

    #[test]
    fn drill_produces_output() {
        let op = OperationConfig::new_default(OperationType::Drill);
        let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
        let heights = test_heights();
        let bbox = test_stock_bbox();
        let cancel = AtomicBool::new(false);

        // Create polygons representing circle centroids (small polygons
        // whose centroid becomes the drill position).
        let circle_poly = Polygon2::rectangle(24.0, 24.0, 26.0, 26.0);
        let polys = vec![circle_poly];

        let result = execute_operation(
            &op,
            None,
            None,
            Some(&polys),
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            None,
            None,
            &cancel,
            None,
        );

        assert!(result.is_ok(), "Drill should succeed, got: {result:?}");
        let tp = result.unwrap();
        assert!(
            !tp.moves.is_empty(),
            "Drill toolpath should contain at least one move"
        );
    }

    #[test]
    fn apply_dressups_preserves_moves() {
        // Build a simple toolpath with a few moves
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 30.0));
        tp.rapid_to(P3::new(10.0, 10.0, 30.0));
        tp.feed_to(P3::new(10.0, 10.0, 0.0), 1000.0);
        tp.feed_to(P3::new(50.0, 10.0, 0.0), 1000.0);
        tp.feed_to(P3::new(50.0, 50.0, 0.0), 1000.0);
        tp.rapid_to(P3::new(50.0, 50.0, 30.0));

        let cfg = DressupConfig::default();
        let result = apply_dressups(tp, &cfg, 6.35, 30.0, None, None, None);

        assert!(
            !result.moves.is_empty(),
            "apply_dressups with default config should preserve moves"
        );
    }
}
