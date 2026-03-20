use std::sync::mpsc;
use std::sync::Arc;

use rs_cam_core::adaptive::{AdaptiveParams, adaptive_toolpath};
use rs_cam_core::depth::{DepthDistribution, DepthStepping, depth_stepped_toolpath};
use rs_cam_core::dressup::{apply_tabs, even_tabs};
use rs_cam_core::inlay::{InlayParams, inlay_toolpaths};
use rs_cam_core::pocket::{PocketParams, pocket_toolpath};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::profile::{ProfileParams, ProfileSide, profile_toolpath};
use rs_cam_core::rest::{RestParams, rest_machining_toolpath};
use rs_cam_core::toolpath::{MoveType, Toolpath};
use rs_cam_core::vcarve::{VCarveParams, vcarve_toolpath};
use rs_cam_core::zigzag::{ZigzagParams, zigzag_toolpath};

use crate::state::job::ToolConfig;
use crate::state::toolpath::*;

/// A request to generate a toolpath.
pub struct ComputeRequest {
    pub toolpath_id: ToolpathId,
    pub polygons: Arc<Vec<Polygon2>>,
    pub operation: OperationConfig,
    pub tool: ToolConfig,
    pub safe_z: f64,
    /// Previous tool radius for rest machining.
    pub prev_tool_radius: Option<f64>,
}

/// Result sent back from worker thread.
pub struct ComputeResult {
    pub toolpath_id: ToolpathId,
    pub result: Result<ToolpathResult, String>,
}

/// Manages background computation of toolpaths.
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

        Self {
            request_tx,
            result_rx,
        }
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

fn run_compute(req: &ComputeRequest) -> Result<ToolpathResult, String> {
    if req.polygons.is_empty() {
        return Err("No polygons loaded".to_string());
    }

    let tp = match &req.operation {
        OperationConfig::Pocket(cfg) => run_pocket(req, cfg),
        OperationConfig::Profile(cfg) => run_profile(req, cfg),
        OperationConfig::Adaptive(cfg) => run_adaptive(req, cfg),
        OperationConfig::VCarve(cfg) => run_vcarve(req, cfg),
        OperationConfig::Rest(cfg) => run_rest(req, cfg),
        OperationConfig::Inlay(cfg) => run_inlay(req, cfg),
        OperationConfig::Zigzag(cfg) => run_zigzag(req, cfg),
    }?;

    let stats = compute_stats(&tp);
    Ok(ToolpathResult {
        toolpath: Arc::new(tp),
        stats,
    })
}

fn run_pocket(req: &ComputeRequest, cfg: &PocketConfig) -> Result<Toolpath, String> {
    let tool_radius = req.tool.diameter / 2.0;
    let depth = DepthStepping {
        start_z: 0.0,
        final_z: -cfg.depth.abs(),
        max_step_down: cfg.depth_per_pass,
        distribution: DepthDistribution::Even,
        finish_allowance: 0.0,
    };

    let mut combined = Toolpath::new();
    for polygon in req.polygons.iter() {
        let tp = depth_stepped_toolpath(&depth, req.safe_z, |z| match cfg.pattern {
            PocketPattern::Contour => pocket_toolpath(
                polygon,
                &PocketParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: req.safe_z,
                    climb: cfg.climb,
                },
            ),
            PocketPattern::Zigzag => zigzag_toolpath(
                polygon,
                &ZigzagParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: req.safe_z,
                    angle: cfg.angle.to_radians(),
                },
            ),
        });
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_profile(req: &ComputeRequest, cfg: &ProfileConfig) -> Result<Toolpath, String> {
    let tool_radius = req.tool.diameter / 2.0;
    let side = match cfg.side {
        crate::state::toolpath::ProfileSide::Outside => ProfileSide::Outside,
        crate::state::toolpath::ProfileSide::Inside => ProfileSide::Inside,
    };
    let depth = DepthStepping {
        start_z: 0.0,
        final_z: -cfg.depth.abs(),
        max_step_down: cfg.depth_per_pass,
        distribution: DepthDistribution::Even,
        finish_allowance: 0.0,
    };

    let mut combined = Toolpath::new();
    for polygon in req.polygons.iter() {
        let mut tp = depth_stepped_toolpath(&depth, req.safe_z, |z| {
            profile_toolpath(
                polygon,
                &ProfileParams {
                    tool_radius,
                    side,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: req.safe_z,
                    climb: cfg.climb,
                },
            )
        });
        // Apply tabs if configured
        if cfg.tab_count > 0 {
            let tabs = even_tabs(cfg.tab_count, cfg.tab_width, cfg.tab_height);
            tp = apply_tabs(&tp, &tabs, -cfg.depth.abs());
        }
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_adaptive(req: &ComputeRequest, cfg: &AdaptiveConfig) -> Result<Toolpath, String> {
    let tool_radius = req.tool.diameter / 2.0;
    let depth = DepthStepping {
        start_z: 0.0,
        final_z: -cfg.depth.abs(),
        max_step_down: cfg.depth_per_pass,
        distribution: DepthDistribution::Even,
        finish_allowance: 0.0,
    };

    let mut combined = Toolpath::new();
    for polygon in req.polygons.iter() {
        let tp = depth_stepped_toolpath(&depth, req.safe_z, |z| {
            adaptive_toolpath(
                polygon,
                &AdaptiveParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: req.safe_z,
                    tolerance: cfg.tolerance,
                    slot_clearing: cfg.slot_clearing,
                    min_cutting_radius: cfg.min_cutting_radius,
                },
            )
        });
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_vcarve(req: &ComputeRequest, cfg: &VCarveConfig) -> Result<Toolpath, String> {
    let half_angle = match req.tool.tool_type {
        crate::state::job::ToolType::VBit => (req.tool.included_angle / 2.0).to_radians(),
        _ => return Err("VCarve requires a V-Bit tool".to_string()),
    };

    let mut combined = Toolpath::new();
    for polygon in req.polygons.iter() {
        let tp = vcarve_toolpath(
            polygon,
            &VCarveParams {
                half_angle,
                max_depth: cfg.max_depth,
                stepover: cfg.stepover,
                feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate,
                safe_z: req.safe_z,
                tolerance: cfg.tolerance,
            },
        );
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_rest(req: &ComputeRequest, cfg: &RestConfig) -> Result<Toolpath, String> {
    let tool_radius = req.tool.diameter / 2.0;
    let prev_tool_radius = req
        .prev_tool_radius
        .ok_or("Previous tool not set for rest machining")?;

    let depth = DepthStepping {
        start_z: 0.0,
        final_z: -cfg.depth.abs(),
        max_step_down: cfg.depth_per_pass,
        distribution: DepthDistribution::Even,
        finish_allowance: 0.0,
    };

    let mut combined = Toolpath::new();
    for polygon in req.polygons.iter() {
        let tp = depth_stepped_toolpath(&depth, req.safe_z, |z| {
            rest_machining_toolpath(
                polygon,
                &RestParams {
                    prev_tool_radius,
                    tool_radius,
                    cut_depth: z,
                    stepover: cfg.stepover,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: req.safe_z,
                    angle: cfg.angle.to_radians(),
                },
            )
        });
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_inlay(req: &ComputeRequest, cfg: &InlayConfig) -> Result<Toolpath, String> {
    let half_angle = match req.tool.tool_type {
        crate::state::job::ToolType::VBit => (req.tool.included_angle / 2.0).to_radians(),
        _ => return Err("Inlay requires a V-Bit tool".to_string()),
    };

    let mut combined = Toolpath::new();
    for polygon in req.polygons.iter() {
        let result = inlay_toolpaths(
            polygon,
            &InlayParams {
                half_angle,
                pocket_depth: cfg.pocket_depth,
                glue_gap: cfg.glue_gap,
                flat_depth: cfg.flat_depth,
                boundary_offset: cfg.boundary_offset,
                stepover: cfg.stepover,
                flat_tool_radius: cfg.flat_tool_radius,
                feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate,
                safe_z: req.safe_z,
                tolerance: cfg.tolerance,
            },
        );
        // Combine female + male into one toolpath
        combined.moves.extend(result.female.moves);
        combined.moves.extend(result.male.moves);
    }
    Ok(combined)
}

fn run_zigzag(req: &ComputeRequest, cfg: &ZigzagConfig) -> Result<Toolpath, String> {
    let tool_radius = req.tool.diameter / 2.0;
    let depth = DepthStepping {
        start_z: 0.0,
        final_z: -cfg.depth.abs(),
        max_step_down: cfg.depth_per_pass,
        distribution: DepthDistribution::Even,
        finish_allowance: 0.0,
    };

    let mut combined = Toolpath::new();
    for polygon in req.polygons.iter() {
        let tp = depth_stepped_toolpath(&depth, req.safe_z, |z| {
            zigzag_toolpath(
                polygon,
                &ZigzagParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: req.safe_z,
                    angle: cfg.angle.to_radians(),
                },
            )
        });
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn compute_stats(tp: &Toolpath) -> ToolpathStats {
    let mut cutting = 0.0;
    let mut rapid = 0.0;
    for i in 1..tp.moves.len() {
        let from = tp.moves[i - 1].target;
        let to = tp.moves[i].target;
        let d = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2) + (to.z - from.z).powi(2))
            .sqrt();
        match tp.moves[i].move_type {
            MoveType::Rapid => rapid += d,
            _ => cutting += d,
        }
    }
    ToolpathStats {
        move_count: tp.moves.len(),
        cutting_distance: cutting,
        rapid_distance: rapid,
    }
}
