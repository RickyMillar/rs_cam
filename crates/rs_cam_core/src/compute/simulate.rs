//! Core simulation orchestration -- runs tri-dexel stock simulation over
//! one or more setup groups without any GUI dependencies.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::collision::{RapidCollision, check_rapid_collisions_against_stock};
use crate::compute::transform::SetupTransformInfo;
use crate::dexel_mesh::dexel_stock_to_mesh;
use crate::dexel_stock::{StockCutDirection, TriDexelStock};
use crate::geo::{BoundingBox3, P3};
use crate::interrupt::Cancelled;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::radial_profile::RadialProfileLUT;
use crate::semantic_trace::ToolpathSemanticTrace;
use crate::simulation_cut::{
    SIMULATION_CUT_TRACE_SCHEMA_VERSION, SimulationCutSample, SimulationCutTrace,
    SimulationMetricOptions, SimulationProvenance,
};
use crate::stock_mesh::StockMesh;
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath::Toolpath;

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
    /// Per-toolpath spindle RPM override. When `None`, falls back to
    /// `SimulationRequest.spindle_rpm`. Set this from
    /// `effective_spindle_rpm(&tc.operation, &post)` so that per-op
    /// overrides propagate into `SimulationCutSample.spindle_rpm`.
    pub spindle_rpm: Option<u32>,
}

/// A group of toolpaths from one setup, sharing a cut direction.
pub struct SimGroupEntry {
    pub toolpaths: Vec<SimToolpathEntry>,
    /// Cut direction derived from the setup's face-up orientation.
    pub direction: StockCutDirection,
    /// Per-setup local stock bounding box. When `Some`, the simulation uses
    /// per-group stocks (always stamped FromTop) and composites the results.
    pub local_stock_bbox: Option<BoundingBox3>,
    /// Transform from setup-local coordinates to global stock frame.
    /// Required when `local_stock_bbox` is `Some` and the setup is non-identity.
    pub local_to_global: Option<SetupTransformInfo>,
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
    /// True when the requested resolution was coarsened to fit within grid limits.
    pub resolution_clamped: bool,
    /// Per-toolpath snapshots of the material stock *before* that toolpath
    /// carves. Keyed by toolpath id. Used by the dressup air-cut filter and
    /// rest-machining-aware generators.
    pub prior_stocks: std::collections::HashMap<usize, Arc<TriDexelStock>>,
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

fn hash_with<F>(write: F) -> u64
where
    F: FnOnce(&mut std::collections::hash_map::DefaultHasher),
{
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    write(&mut hasher);
    hasher.finish()
}

fn hash_f64(hasher: &mut std::collections::hash_map::DefaultHasher, value: f64) {
    value.to_bits().hash(hasher);
}

fn hash_toolpath(toolpath: &Toolpath) -> u64 {
    hash_with(|hasher| {
        toolpath.moves.len().hash(hasher);
        for motion in &toolpath.moves {
            hash_f64(hasher, motion.target.x);
            hash_f64(hasher, motion.target.y);
            hash_f64(hasher, motion.target.z);
            match motion.move_type {
                crate::toolpath::MoveType::Rapid => {
                    0_u8.hash(hasher);
                }
                crate::toolpath::MoveType::Linear { feed_rate } => {
                    1_u8.hash(hasher);
                    hash_f64(hasher, feed_rate);
                }
                crate::toolpath::MoveType::ArcCW { i, j, feed_rate } => {
                    2_u8.hash(hasher);
                    hash_f64(hasher, i);
                    hash_f64(hasher, j);
                    hash_f64(hasher, feed_rate);
                }
                crate::toolpath::MoveType::ArcCCW { i, j, feed_rate } => {
                    3_u8.hash(hasher);
                    hash_f64(hasher, i);
                    hash_f64(hasher, j);
                    hash_f64(hasher, feed_rate);
                }
            }
        }
    })
}
fn build_simulation_provenance(request: &SimulationRequest) -> SimulationProvenance {
    let mut toolpath_hashes = BTreeMap::new();
    let mut tool_hashes = BTreeMap::new();
    for group in &request.groups {
        for entry in &group.toolpaths {
            toolpath_hashes.insert(entry.id, hash_toolpath(entry.toolpath.as_ref()));
            tool_hashes.insert(
                entry.id,
                hash_with(|hasher| {
                    hash_f64(hasher, entry.tool.diameter());
                    hash_f64(hasher, entry.tool.length());
                    hash_f64(hasher, entry.tool.shank_diameter);
                    hash_f64(hasher, entry.tool.shank_length);
                    hash_f64(hasher, entry.tool.holder_diameter);
                    hash_f64(hasher, entry.tool.stickout);
                    entry.tool.flute_count.hash(hasher);
                }),
            );
        }
    }
    let stock_hash = hash_with(|hasher| {
        hash_f64(hasher, request.stock_bbox.min.x);
        hash_f64(hasher, request.stock_bbox.min.y);
        hash_f64(hasher, request.stock_bbox.min.z);
        hash_f64(hasher, request.stock_bbox.max.x);
        hash_f64(hasher, request.stock_bbox.max.y);
        hash_f64(hasher, request.stock_bbox.max.z);
        hash_f64(hasher, request.stock_top_z);
        hash_f64(hasher, request.resolution);
    });
    let machine_hash = hash_with(|hasher| {
        request.spindle_rpm.hash(hasher);
        hash_f64(hasher, request.rapid_feed_mm_min);
    });
    SimulationProvenance {
        trace_schema_version: SIMULATION_CUT_TRACE_SCHEMA_VERSION,
        captured_arc_engagement: request.metric_options.capture_arc_engagement,
        toolpath_hashes,
        tool_hashes,
        stock_hash,
        machine_hash,
    }
}

impl From<Cancelled> for SimulationError {
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

/// Transform a stock mesh from setup-local to global coordinates.
fn transform_stock_mesh_to_global(
    mesh: &StockMesh,
    transform: &Option<SetupTransformInfo>,
) -> StockMesh {
    let Some(info) = transform else {
        return mesh.clone();
    };
    let mut out = StockMesh::empty();
    out.append_transformed(mesh, |x, y, z| {
        let p = info.local_to_global(P3::new(f64::from(x), f64::from(y), f64::from(z)));
        (p.x as f32, p.y as f32, p.z as f32)
    });
    out
}

/// Run a full stock simulation over one or more setup groups.
///
/// This is the headless (no GUI) version of the simulation pipeline:
///
/// 1. Creates per-setup local stocks (always simulated FromTop)
/// 2. Simulates each toolpath, collecting metrics if enabled
/// 3. Transforms local meshes to global frame and composites them
/// 4. Maintains a parallel global stock for checkpoint/playback support
/// 5. Checks for rapid-through-stock collisions
/// 6. Assembles cut trace from samples (if metrics enabled)
/// 7. Computes per-vertex deviation against a reference model (if provided)
///
/// When `local_stock_bbox` is `None` for a group (old callers that
/// don't set per-setup fields), falls back to `request.stock_bbox`.
///
/// The `cancel` flag is polled during simulation; if set, returns
/// `SimulationError::Cancelled`.
pub fn run_simulation(
    request: &SimulationRequest,
    cancel: &AtomicBool,
) -> Result<SimulationResult, SimulationError> {
    run_simulation_with_phase(request, cancel, |_| {})
}

/// Run a full stock simulation with a phase callback for progress reporting.
///
/// Identical to [`run_simulation`] but calls `set_phase` with a human-readable
/// label at each major step (e.g. "Initialize stock", "Simulate Pocket1",
/// "Scan rapid collisions", "Build simulation mesh", "Compute deviations").
pub fn run_simulation_with_phase<F>(
    request: &SimulationRequest,
    cancel: &AtomicBool,
    mut set_phase: F,
) -> Result<SimulationResult, SimulationError>
where
    F: FnMut(&str),
{
    set_phase("Initialize stock");

    // Detect whether the grid will be coarsened beyond the requested resolution.
    let resolution_clamped = {
        let sx = request.stock_bbox.max.x - request.stock_bbox.min.x;
        let sy = request.stock_bbox.max.y - request.stock_bbox.min.y;
        crate::dexel::DexelGrid::would_exceed_grid(request.resolution, sx, sy).is_some()
    };
    let sample_step_mm = request.resolution.max(0.25);

    let mut total_moves = 0;
    let mut boundary_index = 0;
    let mut boundaries = Vec::new();
    let mut checkpoints = Vec::new();
    let mut cut_samples: Vec<SimulationCutSample> = Vec::new();
    // Composited mesh from all per-setup simulations.
    let mut composite_mesh = StockMesh::empty();
    // Parallel global stock for checkpoint/playback support.
    // Use zero-origin bbox (stock dims only) because local_to_global
    // returns stock-relative coordinates (0→stock_x, 0→stock_y, 0→stock_z),
    // NOT world coordinates with origin offsets.
    let global_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(
            request.stock_bbox.max.x - request.stock_bbox.min.x,
            request.stock_bbox.max.y - request.stock_bbox.min.y,
            request.stock_bbox.max.z - request.stock_bbox.min.z,
        ),
    };
    let mut global_stock = TriDexelStock::from_bounds(&global_bbox, request.resolution);

    // Rapid collision accumulators — populated per-toolpath BEFORE each
    // simulation step so we compare against the stock state left by all
    // *previous* operations.
    let mut rapid_collisions: Vec<RapidCollision> = Vec::new();
    let mut rapid_collision_move_indices: Vec<usize> = Vec::new();
    let mut prior_stocks: std::collections::HashMap<usize, Arc<TriDexelStock>> =
        std::collections::HashMap::new();

    for group in &request.groups {
        // Per-setup stock: use local bbox if available, else fall back to global.
        let local_bbox = group
            .local_stock_bbox
            .as_ref()
            .unwrap_or(&request.stock_bbox);
        let mut group_stock = TriDexelStock::from_bounds(local_bbox, request.resolution);
        // Per-setup stocks are always simulated from the top (Z-axis).
        let direction = StockCutDirection::FromTop;

        // Direction for global-frame playback/boundaries.
        let playback_direction = group
            .local_to_global
            .as_ref()
            .map_or(StockCutDirection::FromTop, |info| info.cut_direction());

        for entry in &group.toolpaths {
            // Snapshot the stock *before* this toolpath carves so the dressup
            // air-cut filter and rest-machining-aware generators can use it.
            prior_stocks.insert(entry.id, Arc::new(group_stock.clone()));

            // Check rapid collisions against the *current* stock state
            // (after all previous toolpaths, before this one carves).
            {
                let rapids =
                    check_rapid_collisions_against_stock(&entry.toolpath, &group_stock.z_grid);
                for rc in &rapids {
                    rapid_collision_move_indices.push(total_moves + rc.move_index);
                }
                rapid_collisions.extend(rapids);
            }

            set_phase(&format!("Simulate {}", entry.name));
            let lut = RadialProfileLUT::from_cutter(&entry.tool, 256);
            let radius = entry.tool.radius();
            let start_move = total_moves;

            if request.metric_options.enabled {
                let entry_rpm = entry.spindle_rpm.unwrap_or(request.spindle_rpm);
                let mut samples = group_stock
                    .simulate_toolpath_with_lut_metrics_cancel(
                        &entry.toolpath,
                        &lut,
                        &entry.tool,
                        radius,
                        direction,
                        entry.id,
                        entry_rpm,
                        entry.flute_count,
                        request.rapid_feed_mm_min,
                        sample_step_mm,
                        entry.semantic_trace.as_deref(),
                        request.metric_options.capture_arc_engagement,
                        &|| cancel.load(Ordering::SeqCst),
                    )
                    .map_err(|_cancelled| SimulationError::Cancelled)?;
                cut_samples.append(&mut samples);
            } else {
                group_stock
                    .simulate_toolpath_with_lut_cancel(
                        &entry.toolpath,
                        &lut,
                        radius,
                        direction,
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
                direction: playback_direction,
            });

            // Transform toolpath to global frame for parallel global stock.
            let global_tp = if let Some(info) = &group.local_to_global {
                Arc::new(info.transform_toolpath(&entry.toolpath))
            } else {
                Arc::clone(&entry.toolpath)
            };

            // Stamp the global stock in parallel for checkpoint/playback support.
            // This uses the same global-frame toolpath + direction as playback.
            {
                let playback_lut = RadialProfileLUT::from_cutter(&entry.tool, 256);
                let _ = global_stock.simulate_toolpath_with_lut_cancel(
                    &global_tp,
                    &playback_lut,
                    radius,
                    playback_direction,
                    &|| cancel.load(Ordering::SeqCst),
                );
            }

            // Checkpoint: composited mesh for display + global stock for playback resume.
            let local_mesh = dexel_stock_to_mesh(&group_stock);
            let checkpoint_mesh =
                transform_stock_mesh_to_global(&local_mesh, &group.local_to_global);
            checkpoints.push(SimCheckpointMesh {
                boundary_index,
                mesh: checkpoint_mesh,
                stock: global_stock.checkpoint(),
            });

            boundary_index += 1;
        }

        // After all toolpaths in this group, extract mesh and composite.
        let group_mesh = dexel_stock_to_mesh(&group_stock);
        if let Some(info) = &group.local_to_global {
            composite_mesh.append_transformed(&group_mesh, |x, y, z| {
                let p = info.local_to_global(P3::new(f64::from(x), f64::from(y), f64::from(z)));
                (p.x as f32, p.y as f32, p.z as f32)
            });
        } else {
            composite_mesh.append_transformed(&group_mesh, |x, y, z| (x, y, z));
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
        let mut trace = SimulationCutTrace::from_samples_with_semantics(
            sample_step_mm,
            cut_samples,
            semantic_traces,
        );
        trace.provenance = Some(build_simulation_provenance(request));
        Some(Arc::new(trace))
    } else {
        None
    };

    set_phase("Build simulation mesh");
    let mesh = composite_mesh;

    // Compute per-vertex deviation (sim_z - model_z) if a reference model is available.
    let deviations = if request.model_mesh.is_some() {
        set_phase("Compute deviations");
        request
            .model_mesh
            .as_ref()
            .map(|model| compute_deviations(&mesh.vertices, model))
    } else {
        None
    };

    Ok(SimulationResult {
        mesh,
        total_moves,
        deviations,
        boundaries,
        checkpoints,
        rapid_collisions,
        rapid_collision_move_indices,
        cut_trace,
        resolution_clamped,
        prior_stocks,
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
            crate::compute::tool_config::ToolMaterial::Carbide,
        );

        let entry = SimToolpathEntry {
            id: 1,
            name: "Test".to_owned(),
            toolpath: Arc::new(tp),
            tool,
            flute_count: 2,
            tool_summary: "6mm Flat".to_owned(),
            semantic_trace: None,
            spindle_rpm: None,
        };

        let group = SimGroupEntry {
            toolpaths: vec![entry],
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
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
    fn per_entry_spindle_rpm_overrides_request_default() {
        // When `SimToolpathEntry.spindle_rpm` is `Some(X)`, every cut sample
        // for that entry should report `X` for `spindle_rpm`, regardless of
        // the request-level default. When `None`, the request-level default
        // is used.
        let mut req = simple_request();
        req.metric_options = SimulationMetricOptions {
            enabled: true,
            capture_arc_engagement: false,
        };
        // Request default is 18_000 (set in `simple_request`); override the
        // single entry to 12_000.
        req.groups[0].toolpaths[0].spindle_rpm = Some(12_000);

        let cancel = AtomicBool::new(false);
        let result = run_simulation(&req, &cancel).unwrap();
        let trace = result
            .cut_trace
            .as_ref()
            .expect("metric trace should be present");
        assert!(
            !trace.samples.is_empty(),
            "trace should contain at least one sample"
        );
        for sample in &trace.samples {
            assert_eq!(
                sample.spindle_rpm, 12_000,
                "per-entry override should drive the sample's spindle_rpm"
            );
        }

        // And the inverse: with `None`, samples report the request default.
        let mut req = simple_request();
        req.metric_options = SimulationMetricOptions {
            enabled: true,
            capture_arc_engagement: false,
        };
        req.groups[0].toolpaths[0].spindle_rpm = None;
        let cancel = AtomicBool::new(false);
        let result = run_simulation(&req, &cancel).unwrap();
        let trace = result
            .cut_trace
            .as_ref()
            .expect("metric trace should be present");
        for sample in &trace.samples {
            assert_eq!(
                sample.spindle_rpm, 18_000,
                "no per-entry override should fall back to request default"
            );
        }
    }

    #[test]
    fn simulation_cancel_returns_error() {
        let req = simple_request();
        let cancel = AtomicBool::new(true);
        let result = run_simulation(&req, &cancel);
        assert!(result.is_err());
    }

    /// End-to-end validation for F-2 (April 2026 adaptive review):
    /// a 2D polygon pocket operation must produce non-zero average
    /// engagement and non-zero removed volume. Before Package N
    /// (commit 12dca81), `StockConfig::update_from_bbox` placed the
    /// stock above the 2D cut plane — the stock spanned [0, stock_z]
    /// while the pocket cut at negative Z, so every cutting sample
    /// fell below the stock floor and the simulator reported 0
    /// engagement despite the stock being visibly cut.
    ///
    /// This test exercises the full chain:
    ///   2D polygon bbox → update_from_bbox → stock at [−z, 0]
    ///   → pocket_toolpath at cut_depth=−3 → run_simulation with
    ///     metric_options.enabled → cut_trace → average_engagement > 0
    ///
    /// See planning/adaptive_review_2026-04.md F-2.
    #[test]
    fn two_d_pocket_simulation_reports_engagement() {
        use crate::compute::stock_config::StockConfig;
        use crate::pocket::{PocketParams, pocket_toolpath};
        use crate::polygon::Polygon2;

        // 30×30 mm square polygon at z=0 — the 2D pocket geometry.
        let polygon = Polygon2 {
            exterior: vec![
                crate::geo::P2::new(0.0, 0.0),
                crate::geo::P2::new(30.0, 0.0),
                crate::geo::P2::new(30.0, 30.0),
                crate::geo::P2::new(0.0, 30.0),
            ],
            holes: vec![],
            closed: true,
        };

        // Auto-size stock from the polygon bbox via the same code path
        // that the GUI and MCP use. This is what Package N fixed.
        let mut stock = StockConfig {
            x: 100.0, // pre-attach default
            y: 100.0,
            z: 10.0, // user's 10mm thick stock
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            padding: 2.0,
            ..StockConfig::default()
        };
        let poly_bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(30.0, 30.0, 0.0),
        };
        stock.update_from_bbox(&poly_bbox);

        // After the fix: stock spans [−10, 0] in Z so a pocket cutting
        // at negative Z engages material at the top.
        let stock_bbox = stock.bbox();
        assert!(
            (stock_bbox.max.z - 0.0).abs() < 1e-9,
            "expected stock top at z=0, got {}",
            stock_bbox.max.z
        );
        assert!(
            (stock_bbox.min.z - (-10.0)).abs() < 1e-9,
            "expected stock bottom at z=-10, got {}",
            stock_bbox.min.z
        );

        // Generate a pocket toolpath. cut_depth is negative: the pocket
        // cuts from z=0 down to z=-3, staying inside the stock.
        let tp = pocket_toolpath(
            &polygon,
            &PocketParams {
                tool_radius: 3.175,
                stepover: 2.0,
                cut_depth: -3.0,
                feed_rate: 1500.0,
                plunge_rate: 500.0,
                safe_z: 10.0,
                climb: true,
            },
        );
        assert!(!tp.moves.is_empty(), "pocket toolpath should be non-empty");

        let tool_def = ToolDefinition::new(
            Box::new(crate::tool::FlatEndmill::new(6.35, 25.0)),
            6.35,
            20.0,
            25.0,
            45.0,
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        );

        let entry = SimToolpathEntry {
            id: 1,
            name: "Pocket F-2 validation".to_owned(),
            toolpath: Arc::new(tp),
            tool: tool_def,
            flute_count: 2,
            tool_summary: "6.35mm Flat".to_owned(),
            semantic_trace: None,
            spindle_rpm: None,
        };

        let group = SimGroupEntry {
            toolpaths: vec![entry],
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
        };

        let req = SimulationRequest {
            groups: vec![group],
            stock_bbox,
            stock_top_z: stock_bbox.max.z,
            resolution: 0.5,
            metric_options: SimulationMetricOptions {
                enabled: true,
                capture_arc_engagement: true,
            },
            spindle_rpm: 18000,
            rapid_feed_mm_min: 5000.0,
            model_mesh: None,
        };

        let cancel = AtomicBool::new(false);
        let result = run_simulation(&req, &cancel).expect("simulation should succeed");

        let trace = result
            .cut_trace
            .as_ref()
            .expect("metric_options.enabled=true should produce a cut_trace");

        // Core F-2 assertion: the simulator SEES the tool engaging material.
        // Before Package N these were all exactly 0.
        assert!(
            trace.summary.average_engagement > 0.0,
            "expected non-zero average_engagement (F-2 closure), got {}",
            trace.summary.average_engagement
        );
        assert!(
            trace.summary.total_removed_volume_est_mm3 > 0.0,
            "expected non-zero removed volume (F-2 closure), got {}",
            trace.summary.total_removed_volume_est_mm3
        );
        // Sanity: peak chipload should be non-zero (the tool is cutting).
        assert!(
            trace.summary.peak_chipload_mm_per_tooth > 0.0,
            "expected non-zero peak chipload, got {}",
            trace.summary.peak_chipload_mm_per_tooth
        );
    }

    #[test]
    fn per_setup_multi_stock_simulation() {
        use crate::compute::transform::SetupTransformInfo;

        let make_tool = || {
            ToolDefinition::new(
                Box::new(crate::tool::FlatEndmill::new(6.0, 25.0)),
                6.0,
                20.0,
                25.0,
                45.0,
                2,
                crate::compute::tool_config::ToolMaterial::Carbide,
            )
        };

        // Stock: 50x50x20 at origin
        let stock_bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(50.0, 50.0, 20.0),
        };

        // Setup 1: Top (identity) — cut a groove at Z=15 (5mm depth from top)
        let mut top_tp = Toolpath::new();
        top_tp.rapid_to(P3::new(25.0, 25.0, 25.0));
        for i in 0..20 {
            let x = 20.0 + (i as f64) * 0.5;
            top_tp.feed_to(P3::new(x, 25.0, 15.0), 600.0);
        }

        // Setup 2: Bottom (flipped) — cut at local Z=15 (= 5mm from bottom)
        // In local frame the stock is still 50x50x20.
        let mut bottom_tp = Toolpath::new();
        bottom_tp.rapid_to(P3::new(25.0, 25.0, 25.0));
        for i in 0..20 {
            let x = 20.0 + (i as f64) * 0.5;
            bottom_tp.feed_to(P3::new(x, 25.0, 15.0), 600.0);
        }

        let top_group = SimGroupEntry {
            toolpaths: vec![SimToolpathEntry {
                id: 1,
                name: "Top Cut".into(),
                toolpath: Arc::new(top_tp),
                tool: make_tool(),
                flute_count: 2,
                tool_summary: "6mm Flat".into(),
                semantic_trace: None,
                spindle_rpm: None,
            }],
            direction: StockCutDirection::FromTop,
            local_stock_bbox: Some(stock_bbox),
            local_to_global: None, // identity setup
        };

        let bottom_group = SimGroupEntry {
            toolpaths: vec![SimToolpathEntry {
                id: 2,
                name: "Bottom Cut".into(),
                toolpath: Arc::new(bottom_tp),
                tool: make_tool(),
                flute_count: 2,
                tool_summary: "6mm Flat".into(),
                semantic_trace: None,
                spindle_rpm: None,
            }],
            direction: StockCutDirection::FromBottom,
            local_stock_bbox: Some(BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(50.0, 50.0, 20.0), // effective_stock for Bottom
            }),
            local_to_global: Some(SetupTransformInfo {
                face_up: crate::compute::transform::FaceUp::Bottom,
                z_rotation: crate::compute::transform::ZRotation::Deg0,
                stock_x: 50.0,
                stock_y: 50.0,
                stock_z: 20.0,
                ..Default::default()
            }),
        };

        let req = SimulationRequest {
            groups: vec![top_group, bottom_group],
            stock_bbox,
            stock_top_z: 20.0,
            resolution: 0.5,
            metric_options: SimulationMetricOptions::default(),
            spindle_rpm: 18_000,
            rapid_feed_mm_min: 5_000.0,
            model_mesh: None,
        };

        let cancel = AtomicBool::new(false);
        let result = run_simulation(&req, &cancel).unwrap();

        // Should have 2 boundaries (one per toolpath)
        assert_eq!(result.boundaries.len(), 2);

        // Should have 2 checkpoints
        assert_eq!(result.checkpoints.len(), 2);

        // The composite mesh should be non-empty
        assert!(
            !result.mesh.vertices.is_empty(),
            "composited mesh should not be empty"
        );

        // After both cuts: material should remain in the middle
        // Top cut removes above Z≈15, bottom cut (in global frame) removes below Z≈5
        // Expected remaining: Z=5 to Z=15
        let cp1 = &result.checkpoints[1].stock;
        let cell = cp1.z_grid.world_to_cell(25.0, 25.0);
        assert!(cell.is_some(), "center cell should exist in global stock");
        let (r, c) = cell.unwrap();
        let ray = cp1.z_grid.ray(r, c);
        assert_eq!(
            ray.len(),
            1,
            "after top+bottom cuts: one segment remaining, got {}",
            ray.len()
        );
        assert!(
            (ray[0].enter - 5.0).abs() < 2.0,
            "bottom of remaining material near Z=5, got {}",
            ray[0].enter
        );
        assert!(
            (ray[0].exit - 15.0).abs() < 2.0,
            "top of remaining material near Z=15, got {}",
            ray[0].exit
        );

        // Render composite PNG for visual verification
        let pixels =
            crate::fingerprint::render_stock_composite(&result.checkpoints[1].stock, 600, 400);
        assert!(
            pixels.len() == 600 * 400 * 4,
            "composite PNG has expected pixel count"
        );
    }
}
