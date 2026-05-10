//! Unified operation execution — one entry point for all 23 toolpath operations.
//!
//! Both `ProjectSession` and the GUI compute worker delegate here so the
//! operation dispatch logic exists exactly once.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::compute::catalog::{OperationConfig, OperationTransformCapabilities};
use crate::compute::config::{DressupConfig, DressupEntryStyle, ResolvedHeights};
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::{ToolConfig, ToolType};
use crate::debug_trace::ToolpathDebugContext;
use crate::geo::BoundingBox3;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::semantic_trace::{ToolpathSemanticContext, ToolpathSemanticKind, ToolpathSemanticScope};
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath::Toolpath;
use crate::toolpath_spans::AnnotatedToolpath;

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

// ── Generated toolpath helpers ───────────────────────────────────────

pub type GeneratedToolpath = AnnotatedToolpath;

fn generated_with_spans(
    toolpath: Toolpath,
    spans: Vec<crate::toolpath_spans::Span>,
) -> GeneratedToolpath {
    AnnotatedToolpath::with_spans(toolpath, spans)
}

fn generated_with_depth_run_spans(toolpath: Toolpath, levels: &[f64]) -> GeneratedToolpath {
    let spans = crate::compute::spans::spans_from_depth_runs(&toolpath, levels);
    AnnotatedToolpath::with_spans(toolpath, spans)
}

fn generated_with_cut_run_spans(toolpath: Toolpath, label_prefix: &str) -> GeneratedToolpath {
    let spans = crate::compute::spans::spans_from_cutting_runs(&toolpath, label_prefix);
    AnnotatedToolpath::with_spans(toolpath, spans)
}

fn generated_with_drill_spans(toolpath: Toolpath) -> GeneratedToolpath {
    let spans = crate::compute::spans::spans_from_drill_holes(&toolpath);
    AnnotatedToolpath::with_spans(toolpath, spans)
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
        None,
        None,
    )
    .map(|at| at.toolpath)
}

/// Execute a single operation, producing a toolpath together with structural
/// spans from the underlying algorithm (when available).
///
/// This is the single source-of-truth dispatch; [`execute_operation`] delegates
/// here and discards the spans for backwards compatibility.
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
    semantic_ctx: Option<&ToolpathSemanticContext>,
    // `boundary`: effective machining boundary polygon (model silhouette inset
    // by tool_radius for containment=Inside). When provided, AgentSearch /
    // ContourParallel / Adaptive pre-clear cells outside this boundary in
    // their internal stock so the bool-grid polygon at every z-level reflects
    // the boundary. Without this, generation produces cuts across the full
    // stock that the post-generation toolpath clip then converts to rapids —
    // leaving stock unstamped and deeper z-levels biting through fresh
    // material with full-depth axial DOC.
    boundary: Option<&Polygon2>,
) -> Result<GeneratedToolpath, OperationError> {
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
            let generated = generated_with_depth_run_spans(
                crate::face::face_toolpath(stock_bbox, &params),
                &[],
            );
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            let generated = generated_with_depth_run_spans(combined, &levels);
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            let generated = generated_with_depth_run_spans(combined, &levels);
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_adaptive2d(&all_annotations, &combined, ctx);
            }
            let generated = generated_with_depth_run_spans(combined, &levels);
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            let generated = generated_with_depth_run_spans(combined, &levels);
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            let generated = generated_with_depth_run_spans(combined, &levels);
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_trace_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            let generated = generated_with_cut_run_spans(combined, "V-carve run");
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            let generated = generated_with_depth_run_spans(combined, &levels);
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            let generated = generated_with_cut_run_spans(out, "Inlay run");
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
                top_z: stock_bbox.max.z,
                cycle,
                feed_rate,
                safe_z,
                retract_z: crate::compute::config::effective_safe_z(
                    cfg.retract_z,
                    stock_bbox.max.z,
                ),
            };
            let generated =
                generated_with_drill_spans(crate::drill::drill_toolpath(&holes, &params));
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_drill_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            let generated = generated_with_cut_run_spans(combined, "Chamfer run");
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
                top_z: stock_bbox.max.z,
                cycle,
                feed_rate: cfg.feed_rate,
                safe_z,
                retract_z: crate::compute::config::effective_safe_z(
                    cfg.retract_z,
                    stock_bbox.max.z,
                ),
            };
            let generated =
                generated_with_drill_spans(crate::drill::drill_toolpath(&cfg.holes, &params));
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_drill_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
        }

        // ── 3D operations ────────────────────────────────────────────
        OperationConfig::DropCutter(cfg) => {
            let m = require_mesh(mesh)?;
            let idx = index.ok_or_else(|| {
                OperationError::Other("DropCutter requires a spatial index".into())
            })?;
            // Floor the drop-cutter min_z to the mesh bottom. A 3D finish
            // should only tip-track the mesh surface — anything lower is either
            // a non-contact clamp or the tool's taper forcing the tip below
            // the real surface (which would gouge). We also enforce the
            // stock-bottom floor as a safety check.
            let effective_min_z = cfg
                .min_z
                .max(m.bbox.min.z - 0.1)
                .max(stock_bbox.min.z - 1.0);
            let mut grid = crate::dropcutter::batch_drop_cutter_with_cancel(
                m,
                idx,
                tool_def,
                cfg.stepover,
                0.0,
                effective_min_z,
                &(|| cancel.load(Ordering::SeqCst)),
            )
            .map_err(|_e| OperationError::Cancelled)?;
            // Drop grid points whose vertical ray misses every triangle
            // in the mesh. `point_drop_cutter` marks `contacted = true`
            // whenever the cutter (which has radius) touches ANY nearby
            // triangle — including the rim of a mesh that doesn't cover
            // that XY. Without this check the tool rides the edge and
            // carves a trench around the part.
            for pt in &mut grid.points {
                let mut over = false;
                for &tri_idx in &idx.query(pt.x, pt.y, 0.0) {
                    #[allow(clippy::indexing_slicing)]
                    let tri = &m.faces[tri_idx];
                    if tri.contains_point_xy(pt.x, pt.y) {
                        over = true;
                        break;
                    }
                }
                if !over {
                    pt.z = effective_min_z;
                    pt.contacted = false;
                }
            }
            // Non-contacted grid points get clamped to effective_min_z (floored
            // at the mesh bottom). Filter them so the finish never cuts past
            // the mesh boundary.
            let min_z_filter = Some(effective_min_z);
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
            let generated = generated_with_cut_run_spans(tp, "Raster row");
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
        }
        OperationConfig::Adaptive3d(cfg) => {
            let m = require_mesh(mesh)?;
            let idx = index.ok_or_else(|| {
                OperationError::Other("Adaptive3D requires a spatial index".into())
            })?;

            let entry_style = match cfg.entry_style {
                crate::compute::operation_configs::Adaptive3dEntryStyle::Plunge => {
                    crate::adaptive3d::EntryStyle3d::Plunge
                }
                crate::compute::operation_configs::Adaptive3dEntryStyle::Ramp => {
                    crate::adaptive3d::EntryStyle3d::Ramp {
                        max_angle_deg: cfg.ramp_angle_deg,
                    }
                }
                crate::compute::operation_configs::Adaptive3dEntryStyle::Helix => {
                    crate::adaptive3d::EntryStyle3d::Helix {
                        radius: tool_def.diameter() * cfg.helix_radius_factor,
                        pitch: cfg.helix_pitch,
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
                crate::compute::operation_configs::ClearingStrategy::AgentSearch => {
                    crate::adaptive3d::ClearingStrategy3d::AgentSearch
                }
            };
            // Adaptive3d spaces passes by the tool's *engagement* radius at
            // the depth-of-cut, not the envelope radius — for tapered tools
            // these differ a lot. Floor at 0.01mm to keep stepover math safe
            // for degenerate (zero-tip) geometry.
            let engagement_radius = tool_def.engagement_radius(cfg.depth_per_pass).max(0.01);
            let params = crate::adaptive3d::Adaptive3dParams {
                tool_radius: engagement_radius,
                envelope_radius: tool_radius,
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
                boundary: boundary.cloned(),
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
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_adaptive3d(&annotations, &tp, ctx);
            }
            let spans = crate::compute::spans::spans_from_adaptive3d_annotations(
                &annotations,
                tp.moves.len(),
            );
            Ok(generated_with_spans(tp, spans))
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
            let generated = generated_with_depth_run_spans(tp, &[]);
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_pencil(&annotations, &tp, ctx);
            }
            let spans = crate::compute::spans::spans_from_labeled_events(
                tp.moves.len(),
                annotations
                    .iter()
                    .map(|ann| (ann.move_index, ann.event.label())),
            );
            Ok(generated_with_spans(tp, spans))
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
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_scallop(&annotations, &tp, ctx);
            }
            let spans = crate::compute::spans::spans_from_labeled_events(
                tp.moves.len(),
                annotations
                    .iter()
                    .map(|ann| (ann.move_index, ann.event.label())),
            );
            Ok(generated_with_spans(tp, spans))
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
            let generated = generated_with_cut_run_spans(
                crate::steep_shallow::steep_shallow_toolpath(m, idx, tool_def, &params),
                "Steep/shallow run",
            );
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_ramp_finish(&annotations, &tp, ctx);
            }
            let spans = crate::compute::spans::spans_from_labeled_events(
                tp.moves.len(),
                annotations
                    .iter()
                    .map(|ann| (ann.move_index, ann.event.label())),
            );
            Ok(generated_with_spans(tp, spans))
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
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_spiral_finish(&annotations, &tp, ctx);
            }
            let spans = crate::compute::spans::spans_from_labeled_events(
                tp.moves.len(),
                annotations
                    .iter()
                    .map(|ann| (ann.move_index, ann.event.label())),
            );
            Ok(generated_with_spans(tp, spans))
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
            let generated = generated_with_cut_run_spans(
                crate::radial_finish::radial_finish_toolpath(m, idx, tool_def, &params),
                "Radial ray",
            );
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            let generated = generated_with_cut_run_spans(
                crate::horizontal_finish::horizontal_finish_toolpath(m, idx, tool_def, &params),
                "Horizontal slice",
            );
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
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
            let side = match cfg.side {
                crate::compute::operation_configs::ProjectCurveSide::Center => {
                    crate::project_curve::ProjectSide::Center
                }
                crate::compute::operation_configs::ProjectCurveSide::Inside => {
                    crate::project_curve::ProjectSide::Inside
                }
                crate::compute::operation_configs::ProjectCurveSide::Outside => {
                    crate::project_curve::ProjectSide::Outside
                }
            };
            let params = crate::project_curve::ProjectCurveParams {
                depth: cfg.depth,
                point_spacing: cfg.point_spacing,
                feed_rate,
                plunge_rate,
                safe_z,
                direction,
                tool_radius,
                side,
                setup_z_flipped: cfg.setup_z_flipped,
            };
            let mut combined = Toolpath::new();
            for poly in polys {
                let tp =
                    crate::project_curve::project_curve_toolpath(poly, m, idx, &cutter, &params);
                combined.moves.extend(tp.moves);
            }
            let generated = generated_with_cut_run_spans(combined, "Projected curve");
            if let Some(ctx) = semantic_ctx {
                crate::compute::annotate::annotate_depth_run_spans(
                    &generated.spans,
                    &generated.toolpath,
                    ctx,
                );
            }
            Ok(generated)
        }
    }
}

// ── Dressup tracing helper (Phase 4 / #44) ────────────────────────────

/// Identifiers for one dressup step, used by the tracing wrapper.
struct DressupTraceInfo<'a> {
    debug_key: &'a str,
    debug_label: &'a str,
    kind: ToolpathSemanticKind,
    semantic_label: &'a str,
}

/// Run one dressup step with optional debug + semantic tracing scopes.
///
/// Both contexts are optional — when both are `None` (CLI / session paths)
/// the helper is just `transform(annotated)` with no overhead. When the
/// GUI passes them through, each step appears as its own item in the
/// semantic trace tree (consumed by `sim_op_list` etc.) and as a span in
/// the debug trace.
fn apply_dressup_traced(
    annotated: AnnotatedToolpath,
    debug_ctx: Option<&ToolpathDebugContext>,
    semantic_ctx: Option<&ToolpathSemanticContext>,
    info: DressupTraceInfo<'_>,
    set_params: impl FnOnce(&ToolpathSemanticScope),
    transform: impl FnOnce(AnnotatedToolpath) -> AnnotatedToolpath,
) -> AnnotatedToolpath {
    let debug_scope = debug_ctx.map(|ctx| ctx.start_span(info.debug_key, info.debug_label));
    let debug_span_id = debug_scope.as_ref().map(|s| s.id());
    let semantic_scope = semantic_ctx.map(|ctx| {
        let scope = ctx.start_item(info.kind, info.semantic_label);
        if let Some(span_id) = debug_span_id {
            scope.set_debug_span_id(span_id);
        }
        set_params(&scope);
        scope
    });
    let result = transform(annotated);
    if let Some(scope) = semantic_scope.as_ref() {
        scope.bind_to_toolpath(&result.toolpath, 0, result.toolpath.moves.len());
    }
    if let Some(scope) = debug_scope.as_ref()
        && !result.toolpath.moves.is_empty()
    {
        scope.set_move_range(0, result.toolpath.moves.len() - 1);
    }
    result
}

/// Apply the standard dressup pipeline to a toolpath.
///
/// Steps: entry style → dogbones → lead in/out → link moves → arc fitting
/// → rapid order optimization → air-cut filter → feed rate optimization.
/// Move-order/link transforms are gated by operation capabilities.
///
/// TSP barriers are derived from the input [`AnnotatedToolpath`]'s spans
/// (`RapidOrderBarrier` + `DepthPass` span starts) — see
/// [`AnnotatedToolpath::rapid_order_barriers`].
///
/// `prior_stock` enables the air-cut filter step. `feed_opt_stock` + `cutter`
/// enable feed optimization. Both `debug_ctx` and `semantic_ctx` are optional
/// per-step tracing scopes — the GUI passes them through to populate the
/// sim-tree view; CLI / session callers pass `None` and pay zero overhead.
///
/// All dressups in this pipeline are span-aware (Phase 3 sub-tasks
/// #50–#58); spans on the input are remapped through each step.
#[allow(clippy::too_many_arguments)]
pub fn apply_dressups(
    annotated: AnnotatedToolpath,
    cfg: &DressupConfig,
    tool_diameter: f64,
    safe_z: f64,
    prior_stock: Option<&crate::dexel_stock::TriDexelStock>,
    feed_opt_stock: Option<&mut crate::dexel_stock::TriDexelStock>,
    cutter: Option<&dyn MillingCutter>,
    transform_capabilities: OperationTransformCapabilities,
    debug_ctx: Option<&ToolpathDebugContext>,
    semantic_ctx: Option<&ToolpathSemanticContext>,
) -> AnnotatedToolpath {
    use crate::dressup::{
        EntryStyle, LinkMoveParams, apply_dogbones, apply_entry, apply_lead_in_out,
        apply_link_moves,
    };

    // Capability gate: barriered TSP only fires when the input has barriers.
    let rapid_order_barriers = annotated.rapid_order_barriers();
    let input_valid = annotated.spans_valid;
    let mut current = annotated;

    let tool_radius = tool_diameter / 2.0;

    if cfg.optimize_rapid_order
        && !rapid_order_barriers.is_empty()
        && transform_capabilities.allows_barriered_rapid_reorder()
    {
        let barrier_count = rapid_order_barriers.len();
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            DressupTraceInfo {
                debug_key: "rapid_order",
                debug_label: "Optimize rapid order",
                kind: ToolpathSemanticKind::Optimization,
                semantic_label: "Rapid ordering",
            },
            |scope| {
                scope.set_param("safe_z", safe_z);
                scope.set_param("barrier_count", barrier_count);
            },
            |at| crate::tsp::optimize_rapid_order(at, safe_z),
        );
    }

    let plunge_rate = current
        .toolpath
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
            let ramp_angle = cfg.ramp_angle;
            current = apply_dressup_traced(
                current,
                debug_ctx,
                semantic_ctx,
                DressupTraceInfo {
                    debug_key: "entry_style",
                    debug_label: "Ramp entry",
                    kind: ToolpathSemanticKind::Entry,
                    semantic_label: "Ramp entry",
                },
                |scope| {
                    scope.set_param("kind", "ramp");
                    scope.set_param("max_angle_deg", ramp_angle);
                },
                |at| {
                    apply_entry(
                        at,
                        EntryStyle::Ramp {
                            max_angle_deg: ramp_angle,
                        },
                        plunge_rate,
                    )
                },
            );
        }
        DressupEntryStyle::Helix => {
            let helix_radius = cfg.helix_radius;
            let helix_pitch = cfg.helix_pitch;
            current = apply_dressup_traced(
                current,
                debug_ctx,
                semantic_ctx,
                DressupTraceInfo {
                    debug_key: "entry_style",
                    debug_label: "Helix entry",
                    kind: ToolpathSemanticKind::Entry,
                    semantic_label: "Helix entry",
                },
                |scope| {
                    scope.set_param("kind", "helix");
                    scope.set_param("radius", helix_radius);
                    scope.set_param("pitch", helix_pitch);
                },
                |at| {
                    apply_entry(
                        at,
                        EntryStyle::Helix {
                            radius: helix_radius,
                            pitch: helix_pitch,
                        },
                        plunge_rate,
                    )
                },
            );
        }
        DressupEntryStyle::None => {}
    }

    // 2. Dogbones
    if cfg.dogbone {
        let angle = cfg.dogbone_angle;
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            DressupTraceInfo {
                debug_key: "dogbones",
                debug_label: "Apply dogbones",
                kind: ToolpathSemanticKind::Dressup,
                semantic_label: "Dogbones",
            },
            |scope| {
                scope.set_param("angle_deg", angle);
            },
            |at| apply_dogbones(at, tool_radius, angle),
        );
    }

    // 3. Lead in/out
    if cfg.lead_in_out {
        let radius = cfg.lead_radius;
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            DressupTraceInfo {
                debug_key: "lead_in_out",
                debug_label: "Apply lead in/out",
                kind: ToolpathSemanticKind::Dressup,
                semantic_label: "Lead in/out",
            },
            |scope| {
                scope.set_param("radius", radius);
            },
            |at| apply_lead_in_out(at, radius),
        );
    }

    // 4. Link moves
    if cfg.link_moves && transform_capabilities.allows_link_moves() {
        let max_dist = cfg.link_max_distance;
        let link_feed = cfg.link_feed_rate;
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            DressupTraceInfo {
                debug_key: "link_moves",
                debug_label: "Apply link moves",
                kind: ToolpathSemanticKind::Dressup,
                semantic_label: "Link moves",
            },
            |scope| {
                scope.set_param("max_link_distance", max_dist);
                scope.set_param("link_feed_rate", link_feed);
            },
            |at| {
                apply_link_moves(
                    at,
                    &LinkMoveParams {
                        max_link_distance: max_dist,
                        link_feed_rate: link_feed,
                        safe_z_threshold: safe_z * 0.9,
                    },
                )
            },
        );
    }

    // 5. Arc fitting
    if cfg.arc_fitting {
        let tolerance = cfg.arc_tolerance;
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            DressupTraceInfo {
                debug_key: "arc_fit",
                debug_label: "Fit arcs",
                kind: ToolpathSemanticKind::Optimization,
                semantic_label: "Arc fitting",
            },
            |scope| {
                scope.set_param("tolerance", tolerance);
            },
            |at| crate::arcfit::fit_arcs(at, tolerance),
        );
    }

    // 6. Rapid order optimization (unbarriered fallback)
    if cfg.optimize_rapid_order
        && rapid_order_barriers.is_empty()
        && transform_capabilities.allows_unbarriered_rapid_reorder()
    {
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            DressupTraceInfo {
                debug_key: "rapid_order",
                debug_label: "Optimize rapid order",
                kind: ToolpathSemanticKind::Optimization,
                semantic_label: "Rapid ordering",
            },
            |scope| {
                scope.set_param("safe_z", safe_z);
            },
            |at| crate::tsp::optimize_rapid_order(at, safe_z),
        );
    }

    // 7. Air-cut filter
    if let Some(stock) = prior_stock {
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            DressupTraceInfo {
                debug_key: "air_cut_filter",
                debug_label: "Filter air cuts",
                kind: ToolpathSemanticKind::Optimization,
                semantic_label: "Air-cut filter",
            },
            |scope| {
                scope.set_param("tool_radius", tool_radius);
                scope.set_param("safe_z", safe_z);
            },
            |at| crate::dressup::filter_air_cuts(at, stock, tool_radius, safe_z, 0.1),
        );
    }

    // 8. Feed rate optimization. Rewrites feed rates only — move count/order
    // and spans pass through unchanged.
    if cfg.feed_optimization
        && let (Some(stock), Some(cut)) = (feed_opt_stock, cutter)
    {
        let nominal = current
            .toolpath
            .moves
            .iter()
            .find_map(|m| match m.move_type {
                crate::toolpath::MoveType::Linear { feed_rate } => Some(feed_rate),
                _ => None,
            })
            .unwrap_or(1000.0);
        let max_rate = cfg.feed_max_rate;
        let ramp_rate = cfg.feed_ramp_rate;
        let params = crate::feedopt::FeedOptParams {
            nominal_feed_rate: nominal,
            max_feed_rate: max_rate,
            min_feed_rate: nominal * 0.5,
            ramp_rate,
            air_cut_threshold: 0.05,
        };
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            DressupTraceInfo {
                debug_key: "feed_optimization",
                debug_label: "Optimize feeds",
                kind: ToolpathSemanticKind::Optimization,
                semantic_label: "Feed optimization",
            },
            |scope| {
                scope.set_param("max_feed_rate", max_rate);
                scope.set_param("ramp_rate", ramp_rate);
            },
            |at| crate::feedopt::optimize_feed_rates(at, cut, stock, &params),
        );
    }

    AnnotatedToolpath {
        toolpath: current.toolpath,
        spans: current.spans,
        spans_valid: input_valid && current.spans_valid,
    }
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
    use crate::mesh::{SpatialIndex, TriangleMesh, make_test_flat, make_test_hemisphere};
    use crate::polygon::Polygon2;
    use crate::toolpath_spans::SpanKind;

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

    fn make_v_groove_mesh(length: f64, depth: f64, width: f64) -> TriangleMesh {
        TriangleMesh::from_raw(
            vec![
                P3::new(0.0, -width, 0.0),
                P3::new(length, -width, 0.0),
                P3::new(0.0, 0.0, -depth),
                P3::new(length, 0.0, -depth),
                P3::new(0.0, width, 0.0),
                P3::new(length, width, 0.0),
            ],
            vec![[0, 2, 1], [1, 2, 3], [2, 4, 3], [3, 4, 5]],
        )
    }

    #[derive(Clone, Copy)]
    enum MeshFixture {
        Flat,
        Hemisphere,
        Groove,
    }

    #[derive(Clone, Copy)]
    enum PolygonFixture {
        Standard,
        Drill,
        ProjectCurve,
    }

    struct SpanCoverageCase {
        name: &'static str,
        op: OperationConfig,
        tool_type: ToolType,
        mesh: Option<MeshFixture>,
        polygons: Option<PolygonFixture>,
        prev_tool_radius: Option<f64>,
        expected_kinds: Vec<SpanKind>,
        forbidden_kinds: Vec<SpanKind>,
        expected_label_fragments: Vec<&'static str>,
    }

    fn op_with_updates(
        op_type: OperationType,
        update: impl FnOnce(&mut OperationConfig),
    ) -> OperationConfig {
        let mut op = OperationConfig::new_default(op_type);
        update(&mut op);
        op
    }

    fn span_coverage_cases() -> Vec<SpanCoverageCase> {
        let depth_expected = vec![
            SpanKind::RapidOrderBarrier,
            SpanKind::DepthPass,
            SpanKind::Region,
        ];
        let region_expected = vec![SpanKind::Region];
        let drill_expected = vec![SpanKind::Region];

        let adaptive3d = op_with_updates(OperationType::Adaptive3d, |op| {
            let OperationConfig::Adaptive3d(cfg) = op else {
                unreachable!("default op kind mismatch");
            };
            cfg.depth_per_pass = 4.0;
            cfg.detect_flat_areas = true;
            cfg.region_ordering = crate::compute::operation_configs::RegionOrdering::ByArea;
        });
        let alignment_pin = op_with_updates(OperationType::AlignmentPinDrill, |op| {
            let OperationConfig::AlignmentPinDrill(cfg) = op else {
                unreachable!("default op kind mismatch");
            };
            cfg.holes = vec![[25.0, 25.0], [55.0, 55.0]];
        });
        let drop_cutter = op_with_updates(OperationType::DropCutter, |op| {
            let OperationConfig::DropCutter(cfg) = op else {
                unreachable!("default op kind mismatch");
            };
            cfg.stepover = 2.0;
            cfg.min_z = -5.0;
        });
        let waterline = op_with_updates(OperationType::Waterline, |op| {
            let OperationConfig::Waterline(cfg) = op else {
                unreachable!("default op kind mismatch");
            };
            cfg.z_step = 2.0;
            cfg.sampling = 1.0;
        });
        let scallop = op_with_updates(OperationType::Scallop, |op| {
            let OperationConfig::Scallop(cfg) = op else {
                unreachable!("default op kind mismatch");
            };
            cfg.scallop_height = 0.2;
            cfg.tolerance = 0.2;
            cfg.continuous = true;
        });
        let ramp_finish = op_with_updates(OperationType::RampFinish, |op| {
            let OperationConfig::RampFinish(cfg) = op else {
                unreachable!("default op kind mismatch");
            };
            cfg.max_stepdown = 2.0;
            cfg.sampling = 2.0;
            cfg.tolerance = 0.2;
        });
        let spiral_finish = op_with_updates(OperationType::SpiralFinish, |op| {
            let OperationConfig::SpiralFinish(cfg) = op else {
                unreachable!("default op kind mismatch");
            };
            cfg.stepover = 2.0;
        });
        let radial_finish = op_with_updates(OperationType::RadialFinish, |op| {
            let OperationConfig::RadialFinish(cfg) = op else {
                unreachable!("default op kind mismatch");
            };
            cfg.angular_step = 30.0;
            cfg.point_spacing = 2.0;
        });
        let horizontal_finish = op_with_updates(OperationType::HorizontalFinish, |op| {
            let OperationConfig::HorizontalFinish(cfg) = op else {
                unreachable!("default op kind mismatch");
            };
            cfg.stepover = 3.0;
        });
        let project_curve = op_with_updates(OperationType::ProjectCurve, |op| {
            let OperationConfig::ProjectCurve(cfg) = op else {
                unreachable!("default op kind mismatch");
            };
            cfg.depth = 0.75;
            cfg.point_spacing = 1.0;
        });

        vec![
            SpanCoverageCase {
                name: "Face",
                op: OperationConfig::new_default(OperationType::Face),
                tool_type: ToolType::EndMill,
                mesh: None,
                polygons: None,
                prev_tool_radius: None,
                expected_kinds: depth_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Depth pass", "Run"],
            },
            SpanCoverageCase {
                name: "Pocket",
                op: OperationConfig::new_default(OperationType::Pocket),
                tool_type: ToolType::EndMill,
                mesh: None,
                polygons: Some(PolygonFixture::Standard),
                prev_tool_radius: None,
                expected_kinds: depth_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Depth pass", "Run"],
            },
            SpanCoverageCase {
                name: "Profile",
                op: OperationConfig::new_default(OperationType::Profile),
                tool_type: ToolType::EndMill,
                mesh: None,
                polygons: Some(PolygonFixture::Standard),
                prev_tool_radius: None,
                expected_kinds: depth_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Depth pass", "Run"],
            },
            SpanCoverageCase {
                name: "Adaptive",
                op: OperationConfig::new_default(OperationType::Adaptive),
                tool_type: ToolType::EndMill,
                mesh: None,
                polygons: Some(PolygonFixture::Standard),
                prev_tool_radius: None,
                expected_kinds: depth_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Depth pass", "Run"],
            },
            SpanCoverageCase {
                name: "VCarve",
                op: OperationConfig::new_default(OperationType::VCarve),
                tool_type: ToolType::VBit,
                mesh: None,
                polygons: Some(PolygonFixture::Standard),
                prev_tool_radius: None,
                expected_kinds: region_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["V-carve run"],
            },
            SpanCoverageCase {
                name: "Rest",
                op: OperationConfig::new_default(OperationType::Rest),
                tool_type: ToolType::EndMill,
                mesh: None,
                polygons: Some(PolygonFixture::Standard),
                prev_tool_radius: Some(6.0),
                expected_kinds: depth_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Depth pass", "Run"],
            },
            SpanCoverageCase {
                name: "Inlay",
                op: OperationConfig::new_default(OperationType::Inlay),
                tool_type: ToolType::VBit,
                mesh: None,
                polygons: Some(PolygonFixture::Standard),
                prev_tool_radius: None,
                expected_kinds: region_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Inlay run"],
            },
            SpanCoverageCase {
                name: "Zigzag",
                op: OperationConfig::new_default(OperationType::Zigzag),
                tool_type: ToolType::EndMill,
                mesh: None,
                polygons: Some(PolygonFixture::Standard),
                prev_tool_radius: None,
                expected_kinds: depth_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Depth pass", "Run"],
            },
            SpanCoverageCase {
                name: "Trace",
                op: OperationConfig::new_default(OperationType::Trace),
                tool_type: ToolType::EndMill,
                mesh: None,
                polygons: Some(PolygonFixture::Standard),
                prev_tool_radius: None,
                expected_kinds: depth_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Depth pass", "Run"],
            },
            SpanCoverageCase {
                name: "Drill",
                op: OperationConfig::new_default(OperationType::Drill),
                tool_type: ToolType::EndMill,
                mesh: None,
                polygons: Some(PolygonFixture::Drill),
                prev_tool_radius: None,
                expected_kinds: drill_expected.clone(),
                forbidden_kinds: vec![SpanKind::DepthPass],
                expected_label_fragments: vec!["Hole", "plunge"],
            },
            SpanCoverageCase {
                name: "Chamfer",
                op: OperationConfig::new_default(OperationType::Chamfer),
                tool_type: ToolType::VBit,
                mesh: None,
                polygons: Some(PolygonFixture::Standard),
                prev_tool_radius: None,
                expected_kinds: region_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Chamfer run"],
            },
            SpanCoverageCase {
                name: "DropCutter",
                op: drop_cutter,
                tool_type: ToolType::BallNose,
                mesh: Some(MeshFixture::Flat),
                polygons: None,
                prev_tool_radius: None,
                expected_kinds: region_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Raster row"],
            },
            SpanCoverageCase {
                name: "Adaptive3d",
                op: adaptive3d,
                tool_type: ToolType::EndMill,
                mesh: Some(MeshFixture::Hemisphere),
                polygons: None,
                prev_tool_radius: None,
                // D4 — Adaptive3D now emits SpanKind::Entry per pass.
                expected_kinds: {
                    let mut k = depth_expected.clone();
                    k.push(SpanKind::Entry);
                    k
                },
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Z level", "Adaptive region", "plunge entry"],
            },
            SpanCoverageCase {
                name: "Waterline",
                op: waterline,
                tool_type: ToolType::EndMill,
                mesh: Some(MeshFixture::Hemisphere),
                polygons: None,
                prev_tool_radius: None,
                expected_kinds: depth_expected,
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Depth pass", "Run"],
            },
            SpanCoverageCase {
                name: "Pencil",
                op: OperationConfig::new_default(OperationType::Pencil),
                tool_type: ToolType::BallNose,
                mesh: Some(MeshFixture::Groove),
                polygons: None,
                prev_tool_radius: None,
                expected_kinds: region_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Chain"],
            },
            SpanCoverageCase {
                name: "Scallop",
                op: scallop,
                tool_type: ToolType::BallNose,
                mesh: Some(MeshFixture::Hemisphere),
                polygons: None,
                prev_tool_radius: None,
                expected_kinds: region_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Ring"],
            },
            SpanCoverageCase {
                name: "SteepShallow",
                op: OperationConfig::new_default(OperationType::SteepShallow),
                tool_type: ToolType::BallNose,
                mesh: Some(MeshFixture::Hemisphere),
                polygons: None,
                prev_tool_radius: None,
                expected_kinds: region_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Steep/shallow run"],
            },
            SpanCoverageCase {
                name: "RampFinish",
                op: ramp_finish,
                tool_type: ToolType::BallNose,
                mesh: Some(MeshFixture::Hemisphere),
                polygons: None,
                prev_tool_radius: None,
                expected_kinds: region_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Terrace"],
            },
            SpanCoverageCase {
                name: "SpiralFinish",
                op: spiral_finish,
                tool_type: ToolType::BallNose,
                mesh: Some(MeshFixture::Hemisphere),
                polygons: None,
                prev_tool_radius: None,
                expected_kinds: region_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Ring"],
            },
            SpanCoverageCase {
                name: "RadialFinish",
                op: radial_finish,
                tool_type: ToolType::BallNose,
                mesh: Some(MeshFixture::Flat),
                polygons: None,
                prev_tool_radius: None,
                expected_kinds: region_expected.clone(),
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Radial ray"],
            },
            SpanCoverageCase {
                name: "HorizontalFinish",
                op: horizontal_finish,
                tool_type: ToolType::BallNose,
                mesh: Some(MeshFixture::Flat),
                polygons: None,
                prev_tool_radius: None,
                expected_kinds: region_expected,
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Horizontal slice"],
            },
            SpanCoverageCase {
                name: "ProjectCurve",
                op: project_curve,
                tool_type: ToolType::BallNose,
                mesh: Some(MeshFixture::Hemisphere),
                polygons: Some(PolygonFixture::ProjectCurve),
                prev_tool_radius: None,
                expected_kinds: vec![SpanKind::Region],
                forbidden_kinds: Vec::new(),
                expected_label_fragments: vec!["Projected curve"],
            },
            SpanCoverageCase {
                name: "AlignmentPinDrill",
                op: alignment_pin,
                tool_type: ToolType::EndMill,
                mesh: None,
                polygons: None,
                prev_tool_radius: None,
                expected_kinds: drill_expected,
                forbidden_kinds: vec![SpanKind::DepthPass],
                expected_label_fragments: vec!["Hole", "plunge"],
            },
        ]
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
    fn all_operation_families_emit_expected_structural_span_kinds() {
        let heights = test_heights();
        let bbox = test_stock_bbox();
        let cancel = AtomicBool::new(false);
        let standard_polygons = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];
        let drill_polygons = vec![
            Polygon2::rectangle(24.0, 24.0, 26.0, 26.0),
            Polygon2::rectangle(54.0, 54.0, 56.0, 56.0),
        ];
        let project_curve_polygons = vec![
            Polygon2::rectangle(12.0, 12.0, 40.0, 40.0),
            Polygon2::rectangle(18.0, 18.0, 32.0, 32.0),
        ];
        let flat_mesh = make_test_flat(80.0);
        let flat_index = SpatialIndex::build_auto(&flat_mesh);
        let hemisphere_mesh = make_test_hemisphere(25.0, 16);
        let hemisphere_index = SpatialIndex::build_auto(&hemisphere_mesh);
        let groove_mesh = make_v_groove_mesh(60.0, 8.0, 14.0);
        let groove_index = SpatialIndex::build_auto(&groove_mesh);

        for case in span_coverage_cases() {
            let (tool_def, tool_cfg) = make_tool(case.tool_type);
            let cutting_levels = case.op.cutting_levels(heights.top_z);
            let mesh_and_index = match case.mesh {
                Some(MeshFixture::Flat) => Some((&flat_mesh, &flat_index)),
                Some(MeshFixture::Hemisphere) => Some((&hemisphere_mesh, &hemisphere_index)),
                Some(MeshFixture::Groove) => Some((&groove_mesh, &groove_index)),
                None => None,
            };
            let polygons = match case.polygons {
                Some(PolygonFixture::Standard) => Some(standard_polygons.as_slice()),
                Some(PolygonFixture::Drill) => Some(drill_polygons.as_slice()),
                Some(PolygonFixture::ProjectCurve) => Some(project_curve_polygons.as_slice()),
                None => None,
            };

            let result = execute_operation_annotated(
                &case.op,
                mesh_and_index.map(|(mesh, _)| mesh),
                mesh_and_index.map(|(_, index)| index),
                polygons,
                &tool_def,
                &tool_cfg,
                &heights,
                &cutting_levels,
                &bbox,
                case.prev_tool_radius,
                None,
                &cancel,
                None,
                None,
                None,
            )
            .unwrap_or_else(|err| panic!("{} should generate: {err}", case.name));

            assert!(result.spans_valid, "{} spans should be valid", case.name);
            assert!(
                !result.toolpath.moves.is_empty(),
                "{} should generate moves for span coverage",
                case.name
            );
            result
                .check_invariants()
                .unwrap_or_else(|err| panic!("{} span invariants failed: {err}", case.name));
            assert!(
                result
                    .spans
                    .iter()
                    .any(|span| span.kind == SpanKind::Operation),
                "{} should retain an Operation span",
                case.name
            );
            assert!(
                result
                    .spans
                    .iter()
                    .any(|span| span.kind != SpanKind::Operation),
                "{} should expose structure below the Operation span",
                case.name
            );
            for expected in &case.expected_kinds {
                assert!(
                    result.spans.iter().any(|span| span.kind == *expected),
                    "{} should emit {expected:?} spans; got {:?}",
                    case.name,
                    result
                        .spans
                        .iter()
                        .map(|span| span.kind)
                        .collect::<Vec<_>>()
                );
            }
            for forbidden in &case.forbidden_kinds {
                assert!(
                    result.spans.iter().all(|span| span.kind != *forbidden),
                    "{} should not emit {forbidden:?} spans",
                    case.name
                );
            }
            for fragment in &case.expected_label_fragments {
                assert!(
                    result
                        .spans
                        .iter()
                        .any(|span| span.label.contains(fragment)),
                    "{} should emit a span label containing {fragment:?}; labels: {:?}",
                    case.name,
                    result
                        .spans
                        .iter()
                        .map(|span| span.label.as_ref())
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn trace_annotated_output_has_depth_and_region_spans() {
        let op = OperationConfig::new_default(OperationType::Trace);
        let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
        let heights = test_heights();
        let bbox = test_stock_bbox();
        let cancel = AtomicBool::new(false);
        let polys = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];
        let levels = op.cutting_levels(heights.top_z);

        let result = execute_operation_annotated(
            &op,
            None,
            None,
            Some(&polys),
            &tool_def,
            &tool_cfg,
            &heights,
            &levels,
            &bbox,
            None,
            None,
            &cancel,
            None,
            None,
            None,
        )
        .expect("trace should succeed");

        assert!(result.spans_valid);
        assert!(
            result
                .spans
                .iter()
                .any(|span| span.kind == crate::toolpath_spans::SpanKind::DepthPass),
            "trace should emit DepthPass spans"
        );
        assert!(
            result
                .spans
                .iter()
                .any(|span| span.kind == crate::toolpath_spans::SpanKind::Region),
            "trace should emit per-chain Region spans"
        );
        result
            .check_invariants()
            .expect("trace spans should satisfy invariants");
    }

    #[test]
    fn drill_annotated_output_has_hole_and_plunge_spans_without_depth_barriers() {
        let op = OperationConfig::new_default(OperationType::Drill);
        let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
        let heights = test_heights();
        let bbox = test_stock_bbox();
        let cancel = AtomicBool::new(false);
        let polys = vec![
            Polygon2::rectangle(24.0, 24.0, 26.0, 26.0),
            Polygon2::rectangle(54.0, 54.0, 56.0, 56.0),
        ];

        let result = execute_operation_annotated(
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
            None,
            None,
        )
        .expect("drill should succeed");

        assert!(result.spans_valid);
        let hole_count = result
            .spans
            .iter()
            .filter(|span| span.label.starts_with("Hole ") && !span.label.contains("plunge"))
            .count();
        let plunge_count = result
            .spans
            .iter()
            .filter(|span| span.label.contains("plunge"))
            .count();
        assert_eq!(hole_count, 2, "expected one hole span per input hole");
        assert!(plunge_count >= 2, "expected drill plunge child spans");
        assert!(
            result
                .spans
                .iter()
                .all(|span| span.kind != crate::toolpath_spans::SpanKind::DepthPass),
            "drill must not emit DepthPass spans because they act as TSP barriers"
        );
        result
            .check_invariants()
            .expect("drill spans should satisfy invariants");
    }

    #[test]
    fn trace_semantic_trace_has_depth_and_chain_children() {
        let op = OperationConfig::new_default(OperationType::Trace);
        let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
        let heights = test_heights();
        let bbox = test_stock_bbox();
        let cancel = AtomicBool::new(false);
        let polys = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];
        let levels = op.cutting_levels(heights.top_z);
        let recorder = crate::semantic_trace::ToolpathSemanticRecorder::new("Trace", "Trace");
        let ctx = recorder.root_context();

        let _ = execute_operation_annotated(
            &op,
            None,
            None,
            Some(&polys),
            &tool_def,
            &tool_cfg,
            &heights,
            &levels,
            &bbox,
            None,
            None,
            &cancel,
            None,
            Some(&ctx),
            None,
        )
        .expect("trace should succeed");
        let semantic = recorder.finish();

        assert!(
            semantic
                .items
                .iter()
                .any(|item| item.kind == crate::semantic_trace::ToolpathSemanticKind::DepthLevel),
            "trace should emit DepthLevel semantic items"
        );
        assert!(
            semantic
                .items
                .iter()
                .any(|item| item.kind == crate::semantic_trace::ToolpathSemanticKind::Chain),
            "trace should emit Chain semantic items"
        );
    }

    #[test]
    fn drill_semantic_trace_has_hole_and_cycle_children() {
        let op = OperationConfig::new_default(OperationType::Drill);
        let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
        let heights = test_heights();
        let bbox = test_stock_bbox();
        let cancel = AtomicBool::new(false);
        let polys = vec![Polygon2::rectangle(24.0, 24.0, 26.0, 26.0)];
        let recorder = crate::semantic_trace::ToolpathSemanticRecorder::new("Drill", "Drill");
        let ctx = recorder.root_context();

        let _ = execute_operation_annotated(
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
            Some(&ctx),
            None,
        )
        .expect("drill should succeed");
        let semantic = recorder.finish();

        assert!(
            semantic
                .items
                .iter()
                .any(|item| item.kind == crate::semantic_trace::ToolpathSemanticKind::Hole),
            "drill should emit Hole semantic items"
        );
        assert!(
            semantic
                .items
                .iter()
                .any(|item| item.kind == crate::semantic_trace::ToolpathSemanticKind::Cycle),
            "drill should emit Cycle semantic items"
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
        let result = apply_dressups(
            AnnotatedToolpath::new(tp),
            &cfg,
            6.35,
            30.0,
            None,
            None,
            None,
            OperationType::DropCutter.transform_capabilities(),
            None,
            None,
        );

        assert!(
            !result.toolpath.moves.is_empty(),
            "apply_dressups with default config should preserve moves"
        );
    }
}
