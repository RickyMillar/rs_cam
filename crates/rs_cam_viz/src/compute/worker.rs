use std::sync::mpsc;
use std::sync::Arc;

use rs_cam_core::adaptive::{AdaptiveParams, adaptive_toolpath};
use rs_cam_core::adaptive3d::{Adaptive3dParams, EntryStyle3d, adaptive_3d_toolpath};
use rs_cam_core::depth::{DepthDistribution, DepthStepping, depth_stepped_toolpath};
use rs_cam_core::dropcutter::batch_drop_cutter;
use rs_cam_core::dressup::{apply_tabs, even_tabs};
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

use crate::state::job::{ToolConfig, ToolType};
use crate::state::toolpath::*;

/// A request to generate a toolpath.
pub struct ComputeRequest {
    pub toolpath_id: ToolpathId,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
    pub mesh: Option<Arc<TriangleMesh>>,
    pub operation: OperationConfig,
    pub tool: ToolConfig,
    pub safe_z: f64,
    pub prev_tool_radius: Option<f64>,
}

pub struct ComputeResult {
    pub toolpath_id: ToolpathId,
    pub result: Result<ToolpathResult, String>,
}

pub struct ComputeManager {
    request_tx: mpsc::Sender<ComputeRequest>,
    result_rx: mpsc::Receiver<ComputeResult>,
}

impl ComputeManager {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<ComputeRequest>();
        let (result_tx, result_rx) = mpsc::channel::<ComputeResult>();

        std::thread::spawn(move || {
            while let Ok(req) = request_rx.recv() {
                let result = run_compute(&req);
                let _ = result_tx.send(ComputeResult {
                    toolpath_id: req.toolpath_id,
                    result,
                });
            }
        });

        Self { request_tx, result_rx }
    }

    pub fn submit(&self, request: ComputeRequest) {
        let _ = self.request_tx.send(request);
    }

    pub fn drain_results(&self) -> Vec<ComputeResult> {
        let mut results = Vec::new();
        while let Ok(r) = self.result_rx.try_recv() {
            results.push(r);
        }
        results
    }
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

fn run_compute(req: &ComputeRequest) -> Result<ToolpathResult, String> {
    let tp = match &req.operation {
        OperationConfig::Pocket(c) => run_pocket(req, c),
        OperationConfig::Profile(c) => run_profile(req, c),
        OperationConfig::Adaptive(c) => run_adaptive(req, c),
        OperationConfig::VCarve(c) => run_vcarve(req, c),
        OperationConfig::Rest(c) => run_rest(req, c),
        OperationConfig::Inlay(c) => run_inlay(req, c),
        OperationConfig::Zigzag(c) => run_zigzag(req, c),
        OperationConfig::DropCutter(c) => run_dropcutter(req, c),
        OperationConfig::Adaptive3d(c) => run_adaptive3d(req, c),
        OperationConfig::Waterline(c) => run_waterline(req, c),
        OperationConfig::Pencil(c) => run_pencil(req, c),
        OperationConfig::Scallop(c) => run_scallop(req, c),
        OperationConfig::SteepShallow(c) => run_steep_shallow(req, c),
        OperationConfig::RampFinish(c) => run_ramp_finish(req, c),
    }?;

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
    let depth = make_depth(cfg.depth, cfg.depth_per_pass);
    let mut out = Toolpath::new();
    for p in polys {
        let tp = depth_stepped_toolpath(&depth, req.safe_z, |z| match cfg.pattern {
            PocketPattern::Contour => pocket_toolpath(p, &PocketParams {
                tool_radius: tr, stepover: cfg.stepover, cut_depth: z, feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate, safe_z: req.safe_z, climb: cfg.climb }),
            PocketPattern::Zigzag => zigzag_toolpath(p, &ZigzagParams {
                tool_radius: tr, stepover: cfg.stepover, cut_depth: z, feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate, safe_z: req.safe_z, angle: cfg.angle.to_radians() }),
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
    let depth = make_depth(cfg.depth, cfg.depth_per_pass);
    let mut out = Toolpath::new();
    for p in polys {
        let mut tp = depth_stepped_toolpath(&depth, req.safe_z, |z| profile_toolpath(p, &ProfileParams {
            tool_radius: tr, side, cut_depth: z, feed_rate: cfg.feed_rate,
            plunge_rate: cfg.plunge_rate, safe_z: req.safe_z, climb: cfg.climb }));
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
        let tp = depth_stepped_toolpath(&depth, req.safe_z, |z| adaptive_toolpath(p, &AdaptiveParams {
            tool_radius: tr, stepover: cfg.stepover, cut_depth: z, feed_rate: cfg.feed_rate,
            plunge_rate: cfg.plunge_rate, safe_z: req.safe_z, tolerance: cfg.tolerance,
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
            safe_z: req.safe_z, tolerance: cfg.tolerance });
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
        let tp = depth_stepped_toolpath(&depth, req.safe_z, |z| rest_machining_toolpath(p, &RestParams {
            prev_tool_radius: ptr, tool_radius: tr, cut_depth: z, stepover: cfg.stepover,
            feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate, safe_z: req.safe_z,
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
            plunge_rate: cfg.plunge_rate, safe_z: req.safe_z, tolerance: cfg.tolerance });
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
        let tp = depth_stepped_toolpath(&depth, req.safe_z, |z| zigzag_toolpath(p, &ZigzagParams {
            tool_radius: tr, stepover: cfg.stepover, cut_depth: z, feed_rate: cfg.feed_rate,
            plunge_rate: cfg.plunge_rate, safe_z: req.safe_z, angle: cfg.angle.to_radians() }));
        out.moves.extend(tp.moves);
    }
    Ok(out)
}

// ── 3D operations ────────────────────────────────────────────────────────

fn run_dropcutter(req: &ComputeRequest, cfg: &DropCutterConfig) -> Result<Toolpath, String> {
    let (mesh, index) = require_mesh(req)?;
    let cutter = build_cutter(&req.tool);
    let grid = batch_drop_cutter(mesh, &index, cutter.as_ref(), cfg.stepover, 0.0, cfg.min_z);
    Ok(raster_toolpath_from_grid(&grid, cfg.feed_rate, cfg.plunge_rate, req.safe_z))
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
        depth_per_pass: cfg.depth_per_pass, stock_to_leave: cfg.stock_to_leave,
        feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate, safe_z: req.safe_z,
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
        plunge_rate: cfg.plunge_rate, safe_z: req.safe_z,
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
        safe_z: req.safe_z, stock_to_leave: cfg.stock_to_leave,
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
        safe_z: req.safe_z, stock_to_leave: cfg.stock_to_leave,
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
        safe_z: req.safe_z, sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave, tolerance: cfg.tolerance,
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
        safe_z: req.safe_z, sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave, tolerance: cfg.tolerance,
    };
    Ok(ramp_finish_toolpath(mesh, &index, cutter.as_ref(), &params))
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn make_depth(depth: f64, per_pass: f64) -> DepthStepping {
    DepthStepping {
        start_z: 0.0, final_z: -depth.abs(), max_step_down: per_pass,
        distribution: DepthDistribution::Even, finish_allowance: 0.0,
    }
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
