use std::sync::mpsc;
use std::sync::Arc;

use rs_cam_core::adaptive::{AdaptiveParams, adaptive_toolpath};
use rs_cam_core::collision::{CollisionReport, ToolAssembly, check_collisions_interpolated};
use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::simulation::{Heightmap, HeightmapMesh, heightmap_to_mesh, simulate_toolpath};
use rs_cam_core::adaptive3d::{Adaptive3dParams, EntryStyle3d, adaptive_3d_toolpath};
use rs_cam_core::depth::{DepthDistribution, DepthStepping, depth_stepped_toolpath};
use rs_cam_core::dropcutter::batch_drop_cutter;
use rs_cam_core::arcfit::fit_arcs;
use rs_cam_core::dressup::{EntryStyle as CoreEntryStyle, LinkMoveParams, apply_dogbones, apply_entry, apply_lead_in_out, apply_link_moves, apply_tabs, even_tabs};
use rs_cam_core::feedopt::{FeedOptParams, optimize_feed_rates};
use rs_cam_core::inlay::{InlayParams, inlay_toolpaths};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::pencil::{PencilParams, pencil_toolpath};
use rs_cam_core::pocket::{PocketParams, pocket_toolpath};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::profile::{ProfileParams, profile_toolpath};
use rs_cam_core::ramp_finish::{CutDirection as CoreCutDir, RampFinishParams, ramp_finish_toolpath};
use rs_cam_core::rest::{RestParams, rest_machining_toolpath};
use rs_cam_core::scallop::{ScallopDirection as CoreScalDir, ScallopParams, scallop_toolpath};
use rs_cam_core::steep_shallow::{SteepShallowParams, steep_shallow_toolpath};
use rs_cam_core::tool::{
    BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, VBitEndmill,
};
use rs_cam_core::toolpath::{MoveType, Toolpath, raster_toolpath_from_grid};
use rs_cam_core::vcarve::{VCarveParams, vcarve_toolpath};
use rs_cam_core::waterline::{WaterlineParams, waterline_toolpath};
use rs_cam_core::zigzag::{ZigzagParams, zigzag_toolpath};
use rs_cam_core::face::{FaceParams, FaceDirection as CoreFaceDir, face_toolpath};
use rs_cam_core::trace::{TraceParams, TraceCompensation as CoreTraceComp, trace_toolpath};
use rs_cam_core::drill::{DrillParams, DrillCycle, drill_toolpath};
use rs_cam_core::chamfer::{ChamferParams, chamfer_toolpath};
use rs_cam_core::spiral_finish::{SpiralFinishParams, SpiralDirection as CoreSpiralDir, spiral_finish_toolpath};
use rs_cam_core::radial_finish::{RadialFinishParams, radial_finish_toolpath};
use rs_cam_core::horizontal_finish::{HorizontalFinishParams, horizontal_finish_toolpath};
use rs_cam_core::project_curve::{ProjectCurveParams, project_curve_toolpath};
// boundary module available for future use: clip_toolpath_to_boundary, effective_boundary

use crate::state::job::{ToolConfig, ToolType};
use crate::state::toolpath::*;

/// A request to generate a toolpath.
pub struct ComputeRequest {
    pub toolpath_id: ToolpathId,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
    pub mesh: Option<Arc<TriangleMesh>>,
    pub operation: OperationConfig,
    pub dressups: DressupConfig,
    pub tool: ToolConfig,
    pub safe_z: f64,
    pub prev_tool_radius: Option<f64>,
    pub stock_bbox: Option<BoundingBox3>,
    pub boundary_enabled: bool,
    pub boundary_containment: crate::state::toolpath::BoundaryContainment,
    /// Resolved heights (5-level system). retract_z replaces safe_z for operations,
    /// top_z replaces start_z=0 in depth stepping.
    pub heights: crate::state::toolpath::ResolvedHeights,
}

pub struct ComputeResult {
    pub toolpath_id: ToolpathId,
    pub result: Result<ToolpathResult, String>,
}

/// Request to run material removal simulation.
pub struct SimulationRequest {
    pub toolpaths: Vec<(Arc<Toolpath>, ToolConfig)>,
    pub stock_bbox: BoundingBox3,
    pub stock_top_z: f64,
    pub resolution: f64,
}

/// Result of simulation computation.
pub struct SimulationResult {
    pub mesh: HeightmapMesh,
    pub total_moves: usize,
    /// Per-vertex deviation from model surface (sim_z - model_z).
    /// Positive = material remaining, negative = overcut.
    /// `None` when no model mesh is available for comparison.
    pub deviations: Option<Vec<f32>>,
}

/// Request to run collision detection on a toolpath.
pub struct CollisionRequest {
    pub toolpath: Arc<Toolpath>,
    pub tool: ToolConfig,
    pub mesh: Arc<TriangleMesh>,
}

/// Collision detection result: list of collision positions.
pub struct CollisionResult {
    pub report: CollisionReport,
    pub positions: Vec<[f32; 3]>,
}

enum WorkerRequest {
    Toolpath(ComputeRequest),
    Simulation(SimulationRequest),
    Collision(CollisionRequest),
}

enum WorkerResult {
    Toolpath(ComputeResult),
    Simulation(Result<SimulationResult, String>),
    Collision(Result<CollisionResult, String>),
}

pub struct ComputeManager {
    request_tx: mpsc::Sender<WorkerRequest>,
    result_rx: mpsc::Receiver<WorkerResult>,
}

impl ComputeManager {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<WorkerRequest>();
        let (result_tx, result_rx) = mpsc::channel::<WorkerResult>();

        std::thread::spawn(move || {
            while let Ok(req) = request_rx.recv() {
                match req {
                    WorkerRequest::Toolpath(r) => {
                        let result = run_compute(&r);
                        let _ = result_tx.send(WorkerResult::Toolpath(ComputeResult {
                            toolpath_id: r.toolpath_id,
                            result,
                        }));
                    }
                    WorkerRequest::Simulation(r) => {
                        let result = run_simulation(&r);
                        let _ = result_tx.send(WorkerResult::Simulation(result));
                    }
                    WorkerRequest::Collision(r) => {
                        let result = run_collision_check(&r);
                        let _ = result_tx.send(WorkerResult::Collision(result));
                    }
                }
            }
        });

        Self { request_tx, result_rx }
    }

    pub fn submit(&self, request: ComputeRequest) {
        let _ = self.request_tx.send(WorkerRequest::Toolpath(request));
    }

    pub fn submit_simulation(&self, request: SimulationRequest) {
        let _ = self.request_tx.send(WorkerRequest::Simulation(request));
    }

    pub fn submit_collision(&self, request: CollisionRequest) {
        let _ = self.request_tx.send(WorkerRequest::Collision(request));
    }

    pub fn drain_results(
        &self,
    ) -> (
        Vec<ComputeResult>,
        Vec<Result<SimulationResult, String>>,
        Vec<Result<CollisionResult, String>>,
    ) {
        let mut tp_results = Vec::new();
        let mut sim_results = Vec::new();
        let mut col_results = Vec::new();
        while let Ok(r) = self.result_rx.try_recv() {
            match r {
                WorkerResult::Toolpath(r) => tp_results.push(r),
                WorkerResult::Simulation(r) => sim_results.push(r),
                WorkerResult::Collision(r) => col_results.push(r),
            }
        }
        (tp_results, sim_results, col_results)
    }
}

fn run_simulation(req: &SimulationRequest) -> Result<SimulationResult, String> {
    let mut heightmap = Heightmap::from_bounds(&req.stock_bbox, Some(req.stock_top_z), req.resolution);

    let mut total_moves = 0;
    for (toolpath, tool_config) in &req.toolpaths {
        let cutter = build_cutter(tool_config);
        simulate_toolpath(toolpath, cutter.as_ref(), &mut heightmap);
        total_moves += toolpath.moves.len();
    }

    let mesh = heightmap_to_mesh(&heightmap);
    Ok(SimulationResult {
        mesh,
        total_moves,
        deviations: None,
    })
}

fn build_cutter(tool: &ToolConfig) -> Box<dyn MillingCutter> {
    match tool.tool_type {
        ToolType::EndMill => Box::new(FlatEndmill::new(tool.diameter, tool.cutting_length)),
        ToolType::BallNose => Box::new(BallEndmill::new(tool.diameter, tool.cutting_length)),
        ToolType::BullNose => Box::new(BullNoseEndmill::new(tool.diameter, tool.corner_radius, tool.cutting_length)),
        ToolType::VBit => Box::new(VBitEndmill::new(tool.diameter, tool.included_angle, tool.cutting_length)),
        ToolType::TaperedBallNose => Box::new(TaperedBallEndmill::new(
            tool.diameter, tool.taper_half_angle, tool.shaft_diameter, tool.cutting_length,
        )),
    }
}

/// The effective retract height for operations (from heights system, falls back to safe_z).
fn effective_safe_z(req: &ComputeRequest) -> f64 {
    req.heights.retract_z
}

fn run_compute(req: &ComputeRequest) -> Result<ToolpathResult, String> {
    let mut tp = match &req.operation {
        OperationConfig::Face(c) => run_face(req, c),
        OperationConfig::Pocket(c) => run_pocket(req, c),
        OperationConfig::Profile(c) => run_profile(req, c),
        OperationConfig::Adaptive(c) => run_adaptive(req, c),
        OperationConfig::VCarve(c) => run_vcarve(req, c),
        OperationConfig::Rest(c) => run_rest(req, c),
        OperationConfig::Inlay(c) => run_inlay(req, c),
        OperationConfig::Zigzag(c) => run_zigzag(req, c),
        OperationConfig::Trace(c) => run_trace(req, c),
        OperationConfig::Drill(c) => run_drill(req, c),
        OperationConfig::Chamfer(c) => run_chamfer(req, c),
        OperationConfig::DropCutter(c) => run_dropcutter(req, c),
        OperationConfig::Adaptive3d(c) => run_adaptive3d(req, c),
        OperationConfig::Waterline(c) => run_waterline(req, c),
        OperationConfig::Pencil(c) => run_pencil(req, c),
        OperationConfig::Scallop(c) => run_scallop(req, c),
        OperationConfig::SteepShallow(c) => run_steep_shallow(req, c),
        OperationConfig::RampFinish(c) => run_ramp_finish(req, c),
        OperationConfig::SpiralFinish(c) => run_spiral_finish(req, c),
        OperationConfig::RadialFinish(c) => run_radial_finish(req, c),
        OperationConfig::HorizontalFinish(c) => run_horizontal_finish(req, c),
        OperationConfig::ProjectCurve(c) => run_project_curve(req, c),
    }?;

    tp = apply_dressups(tp, &req.dressups, &req.tool, effective_safe_z(req));

    // Apply machining boundary clipping if enabled
    if req.boundary_enabled {
        if let Some(bbox) = &req.stock_bbox {
            use rs_cam_core::boundary::{ToolContainment, effective_boundary, clip_toolpath_to_boundary};
            let stock_poly = rs_cam_core::polygon::Polygon2::rectangle(
                bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y,
            );
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
    }

    let stats = compute_stats(&tp);
    Ok(ToolpathResult { toolpath: Arc::new(tp), stats })
}

fn require_polygons(req: &ComputeRequest) -> Result<&[Polygon2], String> {
    req.polygons.as_ref().map(|p| p.as_slice()).ok_or_else(|| "No 2D geometry (import SVG)".to_string())
}

fn require_mesh(req: &ComputeRequest) -> Result<(&TriangleMesh, SpatialIndex), String> {
    let mesh = req.mesh.as_ref().ok_or("No mesh (import STL)")?;
    let index = SpatialIndex::build_auto(mesh);
    Ok((mesh, index))
}

// ── 2.5D operations ──────────────────────────────────────────────────────

fn run_pocket(req: &ComputeRequest, cfg: &PocketConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let depth = make_depth_with_finishing(cfg.depth, cfg.depth_per_pass, cfg.finishing_passes);
    let mut out = Toolpath::new();
    for p in polys {
        let tp = depth_stepped_toolpath(&depth, effective_safe_z(req), |z| match cfg.pattern {
            PocketPattern::Contour => pocket_toolpath(p, &PocketParams {
                tool_radius: tr, stepover: cfg.stepover, cut_depth: z, feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate, safe_z: effective_safe_z(req), climb: cfg.climb }),
            PocketPattern::Zigzag => zigzag_toolpath(p, &ZigzagParams {
                tool_radius: tr, stepover: cfg.stepover, cut_depth: z, feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate, safe_z: effective_safe_z(req), angle: cfg.angle.to_radians() }),
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
        let mut tp = depth_stepped_toolpath(&depth, effective_safe_z(req), |z| profile_toolpath(p, &ProfileParams {
            tool_radius: tr, side, cut_depth: z, feed_rate: cfg.feed_rate,
            plunge_rate: cfg.plunge_rate, safe_z: effective_safe_z(req), climb: cfg.climb }));
        if cfg.tab_count > 0 {
            tp = apply_tabs(&tp, &even_tabs(cfg.tab_count, cfg.tab_width, cfg.tab_height), -cfg.depth.abs());
        }
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

fn run_adaptive(req: &ComputeRequest, cfg: &AdaptiveConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let tr = req.tool.diameter / 2.0;
    let depth = make_depth(cfg.depth, cfg.depth_per_pass);
    let mut out = Toolpath::new();
    for p in polys {
        let tp = depth_stepped_toolpath(&depth, effective_safe_z(req), |z| adaptive_toolpath(p, &AdaptiveParams {
            tool_radius: tr, stepover: cfg.stepover, cut_depth: z, feed_rate: cfg.feed_rate,
            plunge_rate: cfg.plunge_rate, safe_z: effective_safe_z(req), tolerance: cfg.tolerance,
            slot_clearing: cfg.slot_clearing, min_cutting_radius: cfg.min_cutting_radius }));
        out.moves.extend(tp.moves);
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
        let tp = vcarve_toolpath(p, &VCarveParams { half_angle: ha, max_depth: cfg.max_depth,
            stepover: cfg.stepover, feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate,
            safe_z: effective_safe_z(req), tolerance: cfg.tolerance });
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
        let tp = depth_stepped_toolpath(&depth, effective_safe_z(req), |z| rest_machining_toolpath(p, &RestParams {
            prev_tool_radius: ptr, tool_radius: tr, cut_depth: z, stepover: cfg.stepover,
            feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate, safe_z: effective_safe_z(req),
            angle: cfg.angle.to_radians() }));
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
        let r = inlay_toolpaths(p, &InlayParams { half_angle: ha, pocket_depth: cfg.pocket_depth,
            glue_gap: cfg.glue_gap, flat_depth: cfg.flat_depth, boundary_offset: cfg.boundary_offset,
            stepover: cfg.stepover, flat_tool_radius: cfg.flat_tool_radius, feed_rate: cfg.feed_rate,
            plunge_rate: cfg.plunge_rate, safe_z: effective_safe_z(req), tolerance: cfg.tolerance });
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
        let tp = depth_stepped_toolpath(&depth, effective_safe_z(req), |z| zigzag_toolpath(p, &ZigzagParams {
            tool_radius: tr, stepover: cfg.stepover, cut_depth: z, feed_rate: cfg.feed_rate,
            plunge_rate: cfg.plunge_rate, safe_z: effective_safe_z(req), angle: cfg.angle.to_radians() }));
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

// ── 3D operations ────────────────────────────────────────────────────────

fn run_dropcutter(req: &ComputeRequest, cfg: &DropCutterConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let grid = batch_drop_cutter(mesh, &index, cutter.as_ref(), cfg.stepover, 0.0, cfg.min_z);
    Ok(raster_toolpath_from_grid(&grid, cfg.feed_rate, cfg.plunge_rate, effective_safe_z(req)))
}

fn run_adaptive3d(req: &ComputeRequest, cfg: &Adaptive3dConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let entry = match cfg.entry_style {
        EntryStyle::Plunge => EntryStyle3d::Plunge,
        EntryStyle::Helix => EntryStyle3d::Helix { radius: req.tool.diameter * 0.3, pitch: 2.0 },
        EntryStyle::Ramp => EntryStyle3d::Ramp { max_angle_deg: 10.0 },
    };
    let params = Adaptive3dParams {
        tool_radius: req.tool.diameter / 2.0, stepover: cfg.stepover,
        depth_per_pass: cfg.depth_per_pass, stock_to_leave: cfg.stock_to_leave_axial,
        feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate, safe_z: effective_safe_z(req),
        tolerance: cfg.tolerance, min_cutting_radius: cfg.min_cutting_radius,
        stock_top_z: cfg.stock_top_z, entry_style: entry,
        fine_stepdown: if cfg.fine_stepdown > 0.0 { Some(cfg.fine_stepdown) } else { None },
        detect_flat_areas: cfg.detect_flat_areas,
        max_stay_down_dist: None,
        region_ordering: match cfg.region_ordering {
            crate::state::toolpath::RegionOrdering::Global => rs_cam_core::adaptive3d::RegionOrdering::Global,
            crate::state::toolpath::RegionOrdering::ByArea => rs_cam_core::adaptive3d::RegionOrdering::ByArea,
        },
    };
    Ok(adaptive_3d_toolpath(mesh, &index, cutter.as_ref(), &params))
}

fn run_waterline(req: &ComputeRequest, cfg: &WaterlineConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let params = WaterlineParams {
        sampling: cfg.sampling, feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate, safe_z: effective_safe_z(req),
    };
    Ok(waterline_toolpath(mesh, &index, cutter.as_ref(), cfg.start_z, cfg.final_z, cfg.z_step, &params))
}

fn run_pencil(req: &ComputeRequest, cfg: &PencilConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let params = PencilParams {
        bitangency_angle: cfg.bitangency_angle, min_cut_length: cfg.min_cut_length,
        hookup_distance: cfg.hookup_distance, num_offset_passes: cfg.num_offset_passes,
        offset_stepover: cfg.offset_stepover, sampling: cfg.sampling,
        feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req), stock_to_leave: cfg.stock_to_leave_axial,
    };
    Ok(pencil_toolpath(mesh, &index, cutter.as_ref(), &params))
}

fn run_scallop(req: &ComputeRequest, cfg: &ScallopConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let params = ScallopParams {
        scallop_height: cfg.scallop_height, tolerance: cfg.tolerance,
        direction: match cfg.direction {
            self::ScallopDirection::OutsideIn => CoreScalDir::OutsideIn,
            self::ScallopDirection::InsideOut => CoreScalDir::InsideOut,
        },
        continuous: cfg.continuous, slope_from: cfg.slope_from, slope_to: cfg.slope_to,
        feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req), stock_to_leave: cfg.stock_to_leave_axial,
    };
    Ok(scallop_toolpath(mesh, &index, cutter.as_ref(), &params))
}

fn run_steep_shallow(req: &ComputeRequest, cfg: &SteepShallowConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let params = SteepShallowParams {
        threshold_angle: cfg.threshold_angle, overlap_distance: cfg.overlap_distance,
        wall_clearance: cfg.wall_clearance, steep_first: cfg.steep_first,
        stepover: cfg.stepover, z_step: cfg.z_step,
        feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req), sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave_axial, tolerance: cfg.tolerance,
    };
    Ok(steep_shallow_toolpath(mesh, &index, cutter.as_ref(), &params))
}

fn run_ramp_finish(req: &ComputeRequest, cfg: &RampFinishConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let params = RampFinishParams {
        max_stepdown: cfg.max_stepdown, slope_from: cfg.slope_from, slope_to: cfg.slope_to,
        direction: match cfg.direction {
            self::CutDirection::Climb => CoreCutDir::Climb,
            self::CutDirection::Conventional => CoreCutDir::Conventional,
            self::CutDirection::BothWays => CoreCutDir::BothWays,
        },
        order_bottom_up: cfg.order_bottom_up,
        feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req), sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave_axial, tolerance: cfg.tolerance,
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
        feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req), stock_to_leave: cfg.stock_to_leave_axial,
    };
    Ok(spiral_finish_toolpath(mesh, &index, cutter.as_ref(), &params))
}

fn run_radial_finish(req: &ComputeRequest, cfg: &RadialFinishConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let params = RadialFinishParams {
        angular_step: cfg.angular_step, point_spacing: cfg.point_spacing,
        feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req), stock_to_leave: cfg.stock_to_leave_axial,
    };
    Ok(radial_finish_toolpath(mesh, &index, cutter.as_ref(), &params))
}

fn run_horizontal_finish(req: &ComputeRequest, cfg: &HorizontalFinishConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let params = HorizontalFinishParams {
        angle_threshold: cfg.angle_threshold, stepover: cfg.stepover,
        feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate,
        safe_z: effective_safe_z(req), stock_to_leave: cfg.stock_to_leave_axial,
    };
    Ok(horizontal_finish_toolpath(mesh, &index, cutter.as_ref(), &params))
}

fn run_project_curve(req: &ComputeRequest, cfg: &ProjectCurveConfig) -> Result<Toolpath, String> {
    let polys = require_polygons(req)?;
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let params = ProjectCurveParams {
        depth: cfg.depth, point_spacing: cfg.point_spacing,
        feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate,
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
    let bbox = req.stock_bbox.ok_or("No stock defined for face operation")?;
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
                tool_radius: tr, depth: z.abs(), depth_per_pass: cfg.depth_per_pass,
                feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate, safe_z: effective_safe_z(req),
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
    // Drill uses polygon centers as hole positions (circles from SVG/DXF)
    let polys = require_polygons(req)?;
    let mut holes = Vec::new();
    for p in polys {
        if p.exterior.is_empty() { continue; }
        // Use centroid of each polygon as drill point
        let (sx, sy) = p.exterior.iter().fold((0.0, 0.0), |(ax, ay), pt| (ax + pt.x, ay + pt.y));
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
        self::DrillCycleType::ChipBreak => DrillCycle::ChipBreak(cfg.peck_depth, cfg.retract_amount),
    };
    let params = DrillParams {
        depth: cfg.depth, cycle, feed_rate: cfg.feed_rate,
        safe_z: effective_safe_z(req), retract_z: cfg.retract_z,
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
            chamfer_width: cfg.chamfer_width, tip_offset: cfg.tip_offset,
            tool_half_angle: ha, tool_radius: req.tool.diameter / 2.0,
            feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate, safe_z: effective_safe_z(req),
        };
        let tp = chamfer_toolpath(p, &params);
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn apply_dressups(mut tp: Toolpath, cfg: &DressupConfig, tool: &ToolConfig, safe_z: f64) -> Toolpath {
    match cfg.entry_style {
        DressupEntryStyle::Ramp => {
            tp = apply_entry(&tp, CoreEntryStyle::Ramp { max_angle_deg: cfg.ramp_angle }, tool.diameter / 2.0);
        }
        DressupEntryStyle::Helix => {
            tp = apply_entry(&tp, CoreEntryStyle::Helix { radius: cfg.helix_radius, pitch: cfg.helix_pitch }, tool.diameter / 2.0);
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
        tp = apply_link_moves(&tp, &LinkMoveParams {
            max_link_distance: cfg.link_max_distance,
            link_feed_rate: cfg.link_feed_rate,
            safe_z_threshold: safe_z * 0.9,
        });
    }
    if cfg.arc_fitting {
        tp = fit_arcs(&tp, cfg.arc_tolerance);
    }
    if cfg.feed_optimization {
        // Feed optimization requires a heightmap for engagement estimation.
        // For 2.5D ops we create a flat heightmap at stock top; for 3D it would
        // need the actual mesh heightmap. This is a simplified version.
        let nominal = tp.moves.iter().find_map(|m| match m.move_type {
            MoveType::Linear { feed_rate } => Some(feed_rate),
            _ => None,
        }).unwrap_or(1000.0);
        let cutter = build_cutter(tool);
        let mut hm = Heightmap::from_stock(-100.0, -100.0, 100.0, 100.0, 0.0, 1.0);
        let params = FeedOptParams {
            nominal_feed_rate: nominal,
            max_feed_rate: cfg.feed_max_rate,
            min_feed_rate: nominal * 0.5,
            ramp_rate: cfg.feed_ramp_rate,
            air_cut_threshold: 0.05,
        };
        tp = optimize_feed_rates(&tp, cutter.as_ref(), &mut hm, &params);
    }
    if cfg.optimize_rapid_order {
        tp = rs_cam_core::tsp::optimize_rapid_order(&tp, safe_z);
    }
    tp
}

fn make_depth(depth: f64, per_pass: f64) -> DepthStepping {
    make_depth_ext(depth, per_pass, 0, 0.0)
}

fn make_depth_with_finishing(depth: f64, per_pass: f64, finishing_passes: usize) -> DepthStepping {
    make_depth_ext(depth, per_pass, finishing_passes, 0.0)
}

fn make_depth_ext(depth: f64, per_pass: f64, finishing_passes: usize, top_z: f64) -> DepthStepping {
    DepthStepping {
        start_z: top_z, final_z: top_z - depth.abs(), max_step_down: per_pass,
        distribution: DepthDistribution::Even, finish_allowance: 0.0,
        finishing_passes,
    }
}

/// Create depth stepping using the resolved heights system.
#[allow(dead_code)]
fn make_depth_from_heights(heights: &crate::state::toolpath::ResolvedHeights, per_pass: f64, finishing_passes: usize) -> DepthStepping {
    DepthStepping {
        start_z: heights.top_z,
        final_z: heights.bottom_z,
        max_step_down: per_pass,
        distribution: DepthDistribution::Even,
        finish_allowance: 0.0,
        finishing_passes,
    }
}

fn run_collision_check(req: &CollisionRequest) -> Result<CollisionResult, String> {
    let index = SpatialIndex::build_auto(&req.mesh);
    let assembly = ToolAssembly {
        cutter_radius: req.tool.diameter / 2.0,
        cutter_length: req.tool.cutting_length,
        shank_diameter: req.tool.shank_diameter,
        shank_length: req.tool.shank_length,
        holder_diameter: req.tool.holder_diameter,
        holder_length: req.tool.stickout - req.tool.cutting_length - req.tool.shank_length,
    };
    let report = check_collisions_interpolated(&req.toolpath, &assembly, &req.mesh, &index, 1.0);
    let positions: Vec<[f32; 3]> = report
        .collisions
        .iter()
        .map(|c| [c.position.x as f32, c.position.y as f32, c.position.z as f32])
        .collect();
    Ok(CollisionResult { report, positions })
}

fn compute_stats(tp: &Toolpath) -> ToolpathStats {
    let mut cutting = 0.0;
    let mut rapid = 0.0;
    for i in 1..tp.moves.len() {
        let from = tp.moves[i - 1].target;
        let to = tp.moves[i].target;
        let d = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2) + (to.z - from.z).powi(2)).sqrt();
        match tp.moves[i].move_type {
            MoveType::Rapid => rapid += d,
            _ => cutting += d,
        }
    }
    ToolpathStats { move_count: tp.moves.len(), cutting_distance: cutting, rapid_distance: rapid }
}
