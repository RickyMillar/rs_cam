//! Core simulation orchestration -- runs tri-dexel stock simulation over
//! one or more setup groups without any GUI dependencies.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::collision::{RapidCollision, check_rapid_collisions};
use crate::dexel_mesh::dexel_stock_to_mesh;
use crate::dexel_stock::{StockCutDirection, TriDexelStock};
use crate::geo::BoundingBox3;
use crate::interrupt::Cancelled;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::radial_profile::RadialProfileLUT;
use crate::semantic_trace::ToolpathSemanticTrace;
use crate::simulation_cut::{SimulationCutSample, SimulationCutTrace, SimulationMetricOptions};
use crate::stock_mesh::StockMesh;
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath::Toolpath;

/// Transform a toolpath from setup-local frame to global stock frame.
///
/// For non-identity directions (e.g. `FromBottom`), the toolpath's depth
/// axis is inverted: `depth_global = stock_extent - depth_local`.  The
/// grid-plane coordinates (u, v) stay the same because the grid covers
/// the same XY/XZ/YZ range regardless of approach side.
fn transform_toolpath_for_direction(
    toolpath: &Toolpath,
    direction: StockCutDirection,
    stock_bbox: &BoundingBox3,
) -> Toolpath {
    if direction.cuts_from_high_side() {
        // FromTop / FromBack / FromRight — no inversion needed, local = global.
        return toolpath.clone();
    }
    let extent = direction.depth_extent(stock_bbox);
    let mut tp = Toolpath::new();
    tp.moves = toolpath
        .moves
        .iter()
        .map(|m| {
            let mut moved = m.clone();
            // Invert the depth axis of the target position.
            match direction {
                StockCutDirection::FromBottom | StockCutDirection::FromTop => {
                    moved.target.z = extent - m.target.z;
                }
                StockCutDirection::FromFront | StockCutDirection::FromBack => {
                    moved.target.y = extent - m.target.y;
                }
                StockCutDirection::FromLeft | StockCutDirection::FromRight => {
                    moved.target.x = extent - m.target.x;
                }
            }
            moved
        })
        .collect();
    tp
}

/// A single toolpath prepared for simulation.
pub struct SimToolpathEntry {
    /// Opaque identifier echoed back in boundaries.
    pub id: usize,
    /// Human-readable name (for boundary labels).
    pub name: String,
    /// The toolpath moves to simulate.
    pub toolpath: Arc<Toolpath>,
    /// Pre-built tool definition (cutter + holder geometry).
    pub tool: ToolDefinition,
    /// Number of cutting flutes (for metric sampling).
    pub flute_count: u32,
    /// Short description of the tool for boundary labels.
    pub tool_summary: String,
    /// Optional semantic trace for metric enrichment.
    pub semantic_trace: Option<Arc<ToolpathSemanticTrace>>,
}

/// A group of toolpaths from one setup, sharing a cut direction.
pub struct SimGroupEntry {
    pub toolpaths: Vec<SimToolpathEntry>,
    /// Cut direction derived from the setup's face-up orientation.
    pub direction: StockCutDirection,
}

/// Request for a full stock simulation.
pub struct SimulationRequest {
    /// Per-setup groups, processed sequentially on one stock.
    pub groups: Vec<SimGroupEntry>,
    pub stock_bbox: BoundingBox3,
    pub stock_top_z: f64,
    pub resolution: f64,
    pub metric_options: SimulationMetricOptions,
    pub spindle_rpm: u32,
    pub rapid_feed_mm_min: f64,
    /// Optional model mesh for deviation computation (sim_z vs model_z).
    pub model_mesh: Option<Arc<TriangleMesh>>,
}

/// Metadata for one toolpath boundary in the simulation timeline.
pub struct SimBoundary {
    pub id: usize,
    pub name: String,
    pub tool_name: String,
    pub start_move: usize,
    pub end_move: usize,
    /// Cut direction for this toolpath's setup.
    pub direction: StockCutDirection,
}

/// A per-toolpath checkpoint capturing the stock state after simulation.
pub struct SimCheckpointMesh {
    pub boundary_index: usize,
    pub mesh: StockMesh,
    pub stock: TriDexelStock,
}

/// Full result from a stock simulation run.
pub struct SimulationResult {
    pub mesh: StockMesh,
    pub total_moves: usize,
    pub deviations: Option<Vec<f32>>,
    pub boundaries: Vec<SimBoundary>,
    pub checkpoints: Vec<SimCheckpointMesh>,
    /// Rapid-through-stock collisions detected during simulation.
    pub rapid_collisions: Vec<RapidCollision>,
    /// Move indices with rapid collisions (for timeline markers).
    pub rapid_collision_move_indices: Vec<usize>,
    pub cut_trace: Option<Arc<SimulationCutTrace>>,
}

/// Error type for simulation failures.
#[derive(Debug, Clone)]
pub enum SimulationError {
    /// The simulation was cancelled via the cancel flag.
    Cancelled,
}

impl std::fmt::Display for SimulationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("Simulation cancelled"),
        }
    }
}

impl std::error::Error for SimulationError {}

impl From<Cancelled> for SimulationError {
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

/// Run a full stock simulation over one or more setup groups.
///
/// This is the headless (no GUI) version of the simulation pipeline:
/// 1. Creates a tri-dexel stock from the bounding box
/// 2. Simulates each toolpath in order, collecting metrics if enabled
/// 3. Creates checkpoint meshes at toolpath boundaries
/// 4. Checks for rapid-through-stock collisions
/// 5. Assembles cut trace from samples (if metrics enabled)
/// 6. Computes per-vertex deviation against a reference model (if provided)
///
/// The `cancel` flag is polled during simulation; if set, returns
/// `SimulationError::Cancelled`.
pub fn run_simulation(
    request: &SimulationRequest,
    cancel: &AtomicBool,
) -> Result<SimulationResult, SimulationError> {
    let mut stock = TriDexelStock::from_bounds(&request.stock_bbox, request.resolution);
    let sample_step_mm = request.resolution.max(0.25);

    let mut total_moves = 0;
    let mut boundary_index = 0;
    let mut boundaries = Vec::new();
    let mut checkpoints = Vec::new();
    let mut cut_samples: Vec<SimulationCutSample> = Vec::new();

    for group in &request.groups {
        for entry in &group.toolpaths {
            // ToolDefinition implements MillingCutter, so we can pass &entry.tool
            // directly to RadialProfileLUT::from_cutter.
            let lut = RadialProfileLUT::from_cutter(&entry.tool, 256);
            let radius = entry.tool.radius();
            let start_move = total_moves;

            // Transform toolpath from setup-local frame to global stock frame.
            // For FromTop this is a no-op clone; for FromBottom it inverts Z.
            let sim_tp = transform_toolpath_for_direction(
                &entry.toolpath,
                group.direction,
                &request.stock_bbox,
            );

            if request.metric_options.enabled {
                let mut samples = stock
                    .simulate_toolpath_with_lut_metrics_cancel(
                        &sim_tp,
                        &lut,
                        radius,
                        group.direction,
                        entry.id,
                        request.spindle_rpm,
                        entry.flute_count,
                        request.rapid_feed_mm_min,
                        sample_step_mm,
                        entry.semantic_trace.as_deref(),
                        &|| cancel.load(Ordering::SeqCst),
                    )
                    .map_err(|_cancelled| SimulationError::Cancelled)?;
                cut_samples.append(&mut samples);
            } else {
                stock
                    .simulate_toolpath_with_lut_cancel(
                        &sim_tp,
                        &lut,
                        radius,
                        group.direction,
                        &|| cancel.load(Ordering::SeqCst),
                    )
                    .map_err(|_cancelled| SimulationError::Cancelled)?;
            }
            total_moves += entry.toolpath.moves.len();

            boundaries.push(SimBoundary {
                id: entry.id,
                name: entry.name.clone(),
                tool_name: entry.tool_summary.clone(),
                start_move,
                end_move: total_moves,
                direction: group.direction,
            });

            checkpoints.push(SimCheckpointMesh {
                boundary_index,
                mesh: dexel_stock_to_mesh(&stock),
                stock: stock.checkpoint(),
            });

            boundary_index += 1;
        }
    }

    // Check for rapid-through-stock collisions on each toolpath
    let mut rapid_collisions = Vec::new();
    let mut rapid_collision_move_indices = Vec::new();
    {
        let mut cumulative_offset = 0;
        for group in &request.groups {
            for entry in &group.toolpaths {
                let rapids = check_rapid_collisions(&entry.toolpath, &request.stock_bbox);
                for rc in &rapids {
                    rapid_collision_move_indices.push(cumulative_offset + rc.move_index);
                }
                rapid_collisions.extend(rapids);
                cumulative_offset += entry.toolpath.moves.len();
            }
        }
    }

    let cut_trace = if request.metric_options.enabled {
        let semantic_traces: Vec<_> = request
            .groups
            .iter()
            .flat_map(|group| {
                group.toolpaths.iter().filter_map(|entry| {
                    entry
                        .semantic_trace
                        .as_deref()
                        .map(|trace| (entry.id, trace))
                })
            })
            .collect();
        let trace = SimulationCutTrace::from_samples_with_semantics(
            sample_step_mm,
            cut_samples,
            semantic_traces,
        );
        Some(Arc::new(trace))
    } else {
        None
    };

    // Build the final simulation mesh
    let mesh = dexel_stock_to_mesh(&stock);

    // Compute per-vertex deviation if a reference model is available
    let deviations = request
        .model_mesh
        .as_ref()
        .map(|model| compute_deviations(&mesh.vertices, model));

    Ok(SimulationResult {
        mesh,
        total_moves,
        deviations,
        boundaries,
        checkpoints,
        rapid_collisions,
        rapid_collision_move_indices,
        cut_trace,
    })
}

/// Compute per-vertex deviation between simulated stock and a reference model.
///
/// Returns one `f32` per vertex. Positive = material remaining, negative = overcut.
/// Vertices far from any model surface or outside the model footprint get 0.0.
// SAFETY: indexing with `i * 3 + {0,1,2}` where `i < num_verts` and
// `num_verts = stock_vertices.len() / 3`, so all accesses are in bounds.
#[allow(clippy::indexing_slicing)]
fn compute_deviations(stock_vertices: &[f32], model_mesh: &TriangleMesh) -> Vec<f32> {
    let num_verts = stock_vertices.len() / 3;
    let index = SpatialIndex::build_auto(model_mesh);

    // Model thickness sets a relevance threshold. Vertices further than this
    // from any model surface have no meaningful deviation (e.g. the flat
    // bottom of a stock beneath a single-surface terrain model).
    let model_thickness = model_mesh.bbox.max.z - model_mesh.bbox.min.z;
    let relevance_threshold = (model_thickness * 0.5).max(2.0); // mm

    let compute_vertex_deviation = |i: usize| -> f32 {
        let x = stock_vertices[i * 3] as f64;
        let y = stock_vertices[i * 3 + 1] as f64;
        let sim_z = stock_vertices[i * 3 + 2] as f64;
        let Some((model_min_z, model_max_z)) = query_model_z_range(&index, model_mesh, x, y) else {
            return 0.0; // outside model footprint
        };

        let dist_to_top = (sim_z - model_max_z).abs();
        let dist_to_bottom = (sim_z - model_min_z).abs();
        let nearest_dist = dist_to_top.min(dist_to_bottom);

        // If the vertex is far from any model surface, it's not a surface
        // the model defines. Return 0 to show neutral color instead of false overcut.
        if nearest_dist > relevance_threshold {
            return 0.0;
        }

        if dist_to_top <= dist_to_bottom {
            (sim_z - model_max_z) as f32
        } else {
            (sim_z - model_min_z) as f32
        }
    };

    // Process vertices in parallel for large meshes
    #[cfg(feature = "parallel")]
    if num_verts > 5000 {
        use rayon::prelude::*;
        return (0..num_verts)
            .into_par_iter()
            .map(compute_vertex_deviation)
            .collect();
    }

    (0..num_verts).map(compute_vertex_deviation).collect()
}

/// Find the model Z range (min, max) at a given XY by querying nearby triangles.
///
/// Returns `(bottom_z, top_z)` -- the lowest and highest model surface at this point.
/// Only considers triangles whose 2D footprint contains (x, y).
#[allow(clippy::indexing_slicing)] // triangle indices bounded by mesh
fn query_model_z_range(
    index: &SpatialIndex,
    mesh: &TriangleMesh,
    x: f64,
    y: f64,
) -> Option<(f64, f64)> {
    let candidates = index.query(x, y, 0.0);
    let mut min_z: Option<f64> = None;
    let mut max_z: Option<f64> = None;
    for tri_idx in candidates {
        let face = &mesh.faces[tri_idx];
        if face.contains_point_xy(x, y)
            && let Some(z) = face.z_at_xy(x, y)
        {
            min_z = Some(min_z.map_or(z, |mz: f64| mz.min(z)));
            max_z = Some(max_z.map_or(z, |mz: f64| mz.max(z)));
        }
    }
    min_z.zip(max_z)
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
    use crate::geo::P3;

    fn simple_request() -> SimulationRequest {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, -1.0), 1000.0);

        let tool = ToolDefinition::new(
            Box::new(crate::tool::FlatEndmill::new(6.0, 25.0)),
            6.0,
            20.0,
            25.0,
            45.0,
            2,
        );

        let entry = SimToolpathEntry {
            id: 1,
            name: "Test".to_owned(),
            toolpath: Arc::new(tp),
            tool,
            flute_count: 2,
            tool_summary: "6mm Flat".to_owned(),
            semantic_trace: None,
        };

        let group = SimGroupEntry {
            toolpaths: vec![entry],
            direction: StockCutDirection::FromTop,
        };

        SimulationRequest {
            groups: vec![group],
            stock_bbox: BoundingBox3 {
                min: P3::new(-5.0, -5.0, -5.0),
                max: P3::new(15.0, 5.0, 5.0),
            },
            stock_top_z: 5.0,
            resolution: 1.0,
            metric_options: SimulationMetricOptions::default(),
            spindle_rpm: 18000,
            rapid_feed_mm_min: 5000.0,
            model_mesh: None,
        }
    }

    #[test]
    fn simulation_produces_mesh() {
        let req = simple_request();
        let cancel = AtomicBool::new(false);
        let result = run_simulation(&req, &cancel).unwrap();
        assert!(!result.mesh.vertices.is_empty());
        assert_eq!(result.boundaries.len(), 1);
        assert_eq!(result.checkpoints.len(), 1);
    }

    #[test]
    fn simulation_cancel_returns_error() {
        let req = simple_request();
        let cancel = AtomicBool::new(true);
        let result = run_simulation(&req, &cancel);
        assert!(result.is_err());
    }
}
