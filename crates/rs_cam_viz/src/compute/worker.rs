use std::sync::mpsc;
use std::sync::Arc;

use rs_cam_core::depth::{DepthDistribution, DepthStepping, depth_stepped_toolpath};
use rs_cam_core::pocket::{PocketParams, pocket_toolpath};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::toolpath::{MoveType, Toolpath};
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

        // Spawn a single worker thread
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

    /// Drain all available results (non-blocking).
    pub fn drain_results(&self) -> Vec<ComputeResult> {
        let mut results = Vec::new();
        while let Ok(r) = self.result_rx.try_recv() {
            results.push(r);
        }
        results
    }
}

fn run_compute(req: &ComputeRequest) -> Result<ToolpathResult, String> {
    match &req.operation {
        OperationConfig::Pocket(cfg) => run_pocket(req, cfg),
    }
}

fn run_pocket(req: &ComputeRequest, cfg: &PocketConfig) -> Result<ToolpathResult, String> {
    if req.polygons.is_empty() {
        return Err("No polygons to pocket".to_string());
    }

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
            PocketPattern::Contour => {
                let params = PocketParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: req.safe_z,
                    climb: cfg.climb,
                };
                pocket_toolpath(polygon, &params)
            }
            PocketPattern::Zigzag => {
                let params = ZigzagParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z: req.safe_z,
                    angle: cfg.angle.to_radians(),
                };
                zigzag_toolpath(polygon, &params)
            }
        });
        combined.moves.extend(tp.moves);
    }

    let stats = compute_stats(&combined);

    Ok(ToolpathResult {
        toolpath: Arc::new(combined),
        stats,
    })
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
