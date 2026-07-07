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
use crate::ids::ToolpathId;
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
use crate::toolpath_spans::AnnotatedToolpath;

/// A single toolpath prepared for simulation.
pub struct SimToolpathEntry {
    /// Opaque identifier echoed back in boundaries.
    pub id: ToolpathId,
    /// Human-readable name (for boundary labels).
    pub name: String,
    /// The toolpath moves to simulate, bundled with the structural spans
    /// produced by generation. Spans are stamped onto each
    /// `SimulationCutSample` so downstream MCP/UI tooling can filter by
    /// `SpanKind` / `pass_index` without re-parsing the move list.
    pub annotated: Arc<AnnotatedToolpath>,
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
    /// P4: true for drill / alignment-pin-drill kinds whose Z-only
    /// kinematics the dexel's XY-cylinder engagement model can't see.
    /// The trace builder uses this to suppress per-sample air-cut /
    /// low-engagement `SimulationCutIssue` emission for these toolpaths
    /// and to mark their per-TP summary as `metrics_not_applicable`.
    pub metrics_not_applicable: bool,
    /// First-class drill-op view (§6.E dual-representation invariant).
    /// When `Some`, the simulator bypasses per-segment stamping for this
    /// entry and applies [`crate::dexel_stock::TriDexelStock::apply_drill_op`]
    /// for analytical cone/cylinder removal. The `annotated` toolpath is
    /// still used for G-code export, rapid-collision checks, and move
    /// indexing.
    pub drill_op: Option<Arc<crate::drill_op::DrillOp>>,
    /// Hash of the toolpath's `OperationConfig` at sim-build time.
    /// Carried so the provenance builder can stamp it without having
    /// access to the raw config. Used by [`sim_trace_is_fresh`] to
    /// detect config-only edits (e.g. `feed_rate` changes that don't
    /// alter move geometry) that should invalidate cached load
    /// verdicts.
    pub operation_config_hash: u64,
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
    /// F.4 — phantom `prior_stocks` snapshot for a not-yet-generated
    /// toolpath, breaking the `FromRemainingStock` regeneration catch-22.
    ///
    /// `(k, id)`: record a `prior_stocks` entry for the ungenerated
    /// toolpath `id`, taken immediately BEFORE simulating this group's
    /// `toolpaths[k]` (i.e. after `toolpaths[0..k]` have carved).
    /// `k == toolpaths.len()` means the snapshot is taken after the whole
    /// group has carved (the pending op is the group's last position).
    ///
    /// Validity rule — why only ONE pending op per group may get a
    /// snapshot: the stock "before op P" is only trustworthy when every
    /// enabled toolpath before P *in this group* has actually been
    /// generated (and is therefore present in `toolpaths`, contributing
    /// its cuts to this snapshot). The builder that populates this field
    /// walks the setup's toolpath configs in plan order and stops at the
    /// FIRST enabled config with no generated result — that's the only
    /// position where "everything before me is real" still holds. Every
    /// later pending op is left gated: seeding it here would silently
    /// omit the cuts of the op ahead of it, which for a rest-machining
    /// op means real overcut risk, not just a stale preview.
    ///
    /// Before this field existed, an ungenerated toolpath never appeared
    /// in a `SimGroupEntry` at all (groups are built only from generated
    /// results), so it could never receive a `prior_stocks` entry and
    /// `FromRemainingStock` ops were permanently stuck in `Error` after a
    /// fresh project load. This field turns that into a ladder: each
    /// simulation run unlocks exactly one more pending op.
    pub phantom_prior_stock: Option<(usize, ToolpathId)>,
}

/// Incremental scan for the single [`SimGroupEntry::phantom_prior_stock`]
/// candidate within one simulation group.
///
/// Both request builders (the core session's `run_simulation` and the GUI
/// controller's `build_simulation_groups`) walk a setup's toolpath configs
/// in plan order to assemble one group's `toolpaths` vec, resolving "has
/// this been generated yet" differently (core reads `self.results`, the
/// GUI reads `gui.toolpath_rt`). This scan factors out the shared decision
/// so the two walks can't drift: feed every toolpath config via
/// [`Self::visit`], in plan order, passing how many entries have already
/// been pushed into the group's `toolpaths` vec so far (`entries_so_far`).
/// The scan locks in its answer — a phantom slot, or none — at the FIRST
/// enabled config with no generated result, matching the validity rule
/// documented on `phantom_prior_stock`: every later pending op is left
/// alone, `resolved()` just keeps returning `true` for it.
#[derive(Default)]
pub struct PhantomPriorStockScan {
    resolved: bool,
    phantom: Option<(usize, ToolpathId)>,
}

impl PhantomPriorStockScan {
    /// Consider one toolpath config in plan order. A no-op once the scan
    /// has already resolved (found the first enabled-but-ungenerated
    /// config, whether or not it needed a phantom).
    pub fn visit(
        &mut self,
        entries_so_far: usize,
        enabled: bool,
        has_generated_result: bool,
        id: ToolpathId,
        stock_source: crate::compute::config::StockSource,
    ) {
        if self.resolved || !enabled || has_generated_result {
            return;
        }
        self.resolved = true;
        if stock_source == crate::compute::config::StockSource::FromRemainingStock {
            self.phantom = Some((entries_so_far, id));
        }
    }

    /// True once the scan has locked in its answer (found the first
    /// enabled-but-ungenerated config). Callers can use this to skip the
    /// bookkeeping cheaply once nothing more can change the outcome.
    pub fn resolved(&self) -> bool {
        self.resolved
    }

    /// Consume the scan, returning the phantom candidate (if any) to store
    /// on the group's [`SimGroupEntry::phantom_prior_stock`].
    pub fn finish(self) -> Option<(usize, ToolpathId)> {
        self.phantom
    }
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
    /// F-034: when `Some`, the simulator post-processes the cut trace
    /// and replaces each toolpath's naive `distance / feed`
    /// `total_runtime_s` with a kinematics-aware integrator estimate
    /// (`compute_cycle_time`). When `None`, runtime accounting stays
    /// byte-identical to pre-F-034. Carries the machine's
    /// `max_feed_mm_min` cap so the integrator can clamp commanded
    /// feeds inside the same envelope the controller would.
    pub kinematics: Option<KinematicsContext>,
}

/// F-034 cycle-time integrator inputs. Bundles the kinematics limits
/// with the machine-wide max-feed cap so the integrator has the same
/// envelope information the controller would.
///
/// F-035 extends this with `use_predicted_feed_in_gates`: when set,
/// the simulator also builds a per-move `PredictedFeedMap` and the
/// chipload + power gates read predicted feed for each sample instead
/// of the commanded value. Default `false` keeps the loop's
/// calibration byte-identical.
#[derive(Debug, Clone, Copy)]
pub struct KinematicsContext {
    pub kinematics: crate::machine_kinematics::MachineKinematics,
    pub max_feed_mm_min: f64,
    /// F-035 — when `true`, additionally stamp predicted achieved
    /// feeds on the resulting `SimulationCutTrace::predicted_feeds`
    /// for the chipload/power gates to consume. Default `false`.
    pub use_predicted_feed_in_gates: bool,
}

/// Metadata for one toolpath boundary in the simulation timeline.
pub struct SimBoundary {
    pub id: ToolpathId,
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
    pub prior_stocks: std::collections::HashMap<ToolpathId, Arc<TriDexelStock>>,
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

/// Hash of an [`OperationConfig`]. Captures *every* parameter that
/// can influence cutting load — including ones whose edits don't
/// change move geometry (e.g. `feed_rate`, `plunge_rate`, depth
/// staging on drill ops). Used at sim-time to stamp
/// [`SimulationProvenance::operation_config_hashes`] and at
/// load-report time to detect config-only edits that should
/// invalidate cached verdicts.
///
/// Implementation note: serialises via serde-json (already a workspace
/// dep) and hashes the bytes. The op config enum derives
/// `Serialize`/`Deserialize` so this is reliable; it's also
/// future-proof against new fields on op-specific configs.
pub fn hash_operation_config(op: &crate::compute::catalog::OperationConfig) -> u64 {
    // serde-json output is stable for typed structs — field order is
    // determined by the struct definition, not by hash iteration.
    let bytes = serde_json::to_vec(op).unwrap_or_default();
    hash_with(|hasher| {
        bytes.hash(hasher);
    })
}

/// Hash of a linearized toolpath. Used both at sim-time (to populate
/// [`SimulationProvenance::toolpath_hashes`]) and at load-report time
/// (to detect a stale cached trace by comparing the current toolpath
/// hash against the value stored on the trace).
pub(crate) fn hash_toolpath(toolpath: &Toolpath) -> u64 {
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
    let mut operation_config_hashes = BTreeMap::new();
    for group in &request.groups {
        for entry in &group.toolpaths {
            toolpath_hashes.insert(entry.id, hash_toolpath(&entry.annotated.toolpath));
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
            operation_config_hashes.insert(entry.id, entry.operation_config_hash);
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
        operation_config_hashes,
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
    // §6.E PR2 accumulators for drill-native metrics emitted alongside the
    // engagement-side `cut_samples` stream. Each drill toolpath contributes
    // a per-peck sample vector + a per-toolpath summary; both are attached
    // to `SimulationCutTrace` after the per-group simulation loop.
    let mut drill_samples_all: Vec<crate::drill_metrics::DrillSample> = Vec::new();
    let mut drill_summaries_all: Vec<crate::drill_metrics::DrillToolpathSummary> = Vec::new();
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
    let mut prior_stocks: std::collections::HashMap<ToolpathId, Arc<TriDexelStock>> =
        std::collections::HashMap::new();
    // §6.E accumulators for analytic drill geometry. `group_drill_ops` is
    // reset per group (matches per-setup `group_stock` lifetime); the
    // global accumulator stores transformed copies so the final
    // composite mesh shows holes from all setups.
    let mut global_drill_ops: Vec<crate::drill_op::DrillOp> = Vec::new();

    for group in &request.groups {
        // Per-setup stock: use local bbox if available, else fall back to global.
        let local_bbox = group
            .local_stock_bbox
            .as_ref()
            .unwrap_or(&request.stock_bbox);
        let mut group_stock = TriDexelStock::from_bounds(local_bbox, request.resolution);
        // §6.E per-group accumulator: holes drilled into `group_stock`
        // get appended as analytic cylinders when this group's mesh
        // is extracted.
        let mut group_drill_ops: Vec<Arc<crate::drill_op::DrillOp>> = Vec::new();
        // Per-setup stocks are always simulated from the top (Z-axis).
        let direction = StockCutDirection::FromTop;

        // Direction for global-frame playback/boundaries.
        let playback_direction = group
            .local_to_global
            .as_ref()
            .map_or(StockCutDirection::FromTop, |info| info.cut_direction());

        for (k, entry) in group.toolpaths.iter().enumerate() {
            let entry_toolpath = &entry.annotated.toolpath;
            // Snapshot the stock *before* this toolpath carves so the dressup
            // air-cut filter and rest-machining-aware generators can use it.
            //
            // F.4: when this position is also this group's phantom-prior-
            // stock slot (the first pending FromRemainingStock op, recorded
            // by the request builder), the pending op's snapshot is taken at
            // this exact same sequence point — share the one stock clone via
            // `Arc::clone` rather than cloning the (potentially large) dexel
            // stock twice.
            let pre_carve_stock = Arc::new(group_stock.clone());
            if let Some((phantom_k, phantom_id)) = group.phantom_prior_stock
                && phantom_k == k
            {
                prior_stocks.insert(phantom_id, Arc::clone(&pre_carve_stock));
            }
            prior_stocks.insert(entry.id, pre_carve_stock);

            // Check rapid collisions against the *current* stock state
            // (after all previous toolpaths, before this one carves).
            {
                let rapids =
                    check_rapid_collisions_against_stock(entry_toolpath, &group_stock.z_grid);
                for rc in &rapids {
                    rapid_collision_move_indices.push(total_moves + rc.move_index);
                }
                rapid_collisions.extend(rapids);
            }

            set_phase(&format!("Simulate {}", entry.name));
            let lut = RadialProfileLUT::from_cutter(&entry.tool, 256);
            let radius = entry.tool.radius();
            let start_move = total_moves;

            if let Some(drill_op_arc) = entry.drill_op.as_ref() {
                // §6.E analytical drill removal: bypass per-segment
                // stamping. Cone/cylinder envelope is applied directly
                // to the dexel grid; the linearized toolpath remains
                // available for rapid-collision checks, G-code, and
                // wire-render.
                group_stock.apply_drill_op(drill_op_arc);
                group_drill_ops.push(Arc::clone(drill_op_arc));
                // PR2: emit per-peck drill samples + per-toolpath summary
                // (the analytical kernel doesn't produce
                // `SimulationCutSample`s, so the drill-native stream lands
                // here instead).
                let pecks = crate::drill_metrics::emit_drill_samples(entry.id, drill_op_arc);
                let summary = crate::drill_metrics::build_drill_toolpath_summary(
                    entry.id,
                    drill_op_arc,
                    &pecks,
                );
                drill_samples_all.extend(pecks);
                drill_summaries_all.push(summary);
            } else if request.metric_options.enabled {
                let entry_rpm = entry.spindle_rpm.unwrap_or(request.spindle_rpm);
                // F2.2 (defect class C3): honor `spans_valid`. When a
                // transform invalidated the spans (legacy invalidators;
                // TSP now drops exactly the spans it split instead),
                // stamping per-sample ancestry from the fragmented
                // vector gives samples WRONG ancestry — a tagged
                // Entry/LinkBridge sample can become effectively
                // untagged and drive a gate trip with phantom dexel
                // engagement (the WANAKA 622 µm DeflectionSetupLocked
                // mechanism). Degrade honestly: no span_path, transit
                // classification from per-move intents only.
                //
                // In the valid case the intent bitmap is UNIONED in:
                // a transit span dropped by TSP (split LinkBridge)
                // leaves its moves without ancestry, but their intents
                // still classify them as transit.
                let intent_transits = entry.annotated.transit_moves_bitmap_from_intents();
                let (span_paths_by_move, transit_moves) = if entry.annotated.spans_valid {
                    let mut transit = entry.annotated.transit_moves_bitmap();
                    for (slot, from_intent) in transit.iter_mut().zip(intent_transits) {
                        *slot = *slot || from_intent;
                    }
                    (entry.annotated.span_paths_by_move(), transit)
                } else {
                    (
                        vec![Vec::new(); entry_toolpath.moves.len()],
                        intent_transits,
                    )
                };
                let mut samples = group_stock
                    .simulate_toolpath_with_lut_metrics_cancel(
                        entry_toolpath,
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
                        &span_paths_by_move,
                        &transit_moves,
                        request.metric_options.capture_arc_engagement,
                        &|| cancel.load(Ordering::SeqCst),
                    )
                    .map_err(|_cancelled| SimulationError::Cancelled)?;
                cut_samples.append(&mut samples);
            } else {
                group_stock
                    .simulate_toolpath_with_lut_cancel(
                        entry_toolpath,
                        &lut,
                        radius,
                        direction,
                        &|| cancel.load(Ordering::SeqCst),
                    )
                    .map_err(|_cancelled| SimulationError::Cancelled)?;
            }
            total_moves += entry_toolpath.moves.len();

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
                Arc::new(info.transform_toolpath(entry_toolpath))
            } else {
                Arc::new(entry_toolpath.clone())
            };

            // Stamp the global stock in parallel for checkpoint/playback support.
            // This uses the same global-frame toolpath + direction as playback.
            // For drill ops, apply analytical removal in the global frame
            // — hole XYs are transformed when `local_to_global` is set.
            if let Some(drill_op_arc) = entry.drill_op.as_ref() {
                let global_drill_op = match &group.local_to_global {
                    Some(info) => {
                        let mut transformed = (**drill_op_arc).clone();
                        for hole in &mut transformed.holes {
                            let g_top = info.local_to_global(crate::geo::P3::new(
                                hole.xy[0], hole.xy[1], hole.top_z,
                            ));
                            let g_bot = info.local_to_global(crate::geo::P3::new(
                                hole.xy[0],
                                hole.xy[1],
                                hole.bottom_z,
                            ));
                            hole.xy = [g_top.x, g_top.y];
                            hole.top_z = g_top.z;
                            hole.bottom_z = g_bot.z;
                        }
                        transformed
                    }
                    None => (**drill_op_arc).clone(),
                };
                global_stock.apply_drill_op(&global_drill_op);
                global_drill_ops.push(global_drill_op);
            } else {
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
            let mut local_mesh = dexel_stock_to_mesh(&group_stock);
            // §6.E append analytic drill cylinders so checkpoint frames
            // show clean circular hole walls even at low dexel resolution.
            // Cylinders are emitted in local-frame coords; the
            // transform_stock_mesh_to_global call below handles re-framing.
            if !group_drill_ops.is_empty() {
                let refs: Vec<&crate::drill_op::DrillOp> =
                    group_drill_ops.iter().map(|d| d.as_ref()).collect();
                crate::dexel_mesh::append_drill_cylinders(&mut local_mesh, &refs);
            }
            let checkpoint_mesh =
                transform_stock_mesh_to_global(&local_mesh, &group.local_to_global);
            checkpoints.push(SimCheckpointMesh {
                boundary_index,
                mesh: checkpoint_mesh,
                stock: global_stock.checkpoint(),
            });

            boundary_index += 1;
        }

        // F.4: phantom slot at the tail of the group — the first pending
        // FromRemainingStock op sits after every already-generated toolpath
        // in this group, so its snapshot is the fully-carved group stock.
        if let Some((phantom_k, phantom_id)) = group.phantom_prior_stock
            && phantom_k == group.toolpaths.len()
        {
            prior_stocks.insert(phantom_id, Arc::new(group_stock.clone()));
        }

        // After all toolpaths in this group, extract mesh and composite.
        let mut group_mesh = dexel_stock_to_mesh(&group_stock);
        if !group_drill_ops.is_empty() {
            let refs: Vec<&crate::drill_op::DrillOp> =
                group_drill_ops.iter().map(|d| d.as_ref()).collect();
            crate::dexel_mesh::append_drill_cylinders(&mut group_mesh, &refs);
        }
        if let Some(info) = &group.local_to_global {
            composite_mesh.append_transformed(&group_mesh, |x, y, z| {
                let p = info.local_to_global(P3::new(f64::from(x), f64::from(y), f64::from(z)));
                (p.x as f32, p.y as f32, p.z as f32)
            });
        } else {
            composite_mesh.append_transformed(&group_mesh, |x, y, z| (x, y, z));
        }
    }

    // `global_drill_ops` is currently accumulated for future use by
    // alternative mesh extractions (e.g. checkpoint resume with cylinders
    // re-emitted in global coords). The composite mesh above is built
    // from per-group local meshes that already include their own
    // cylinders, so no additional append is needed here.
    let _ = global_drill_ops;

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
        // P4: collect drill-kind toolpath ids so the trace builder
        // suppresses air-cut / low-engagement issue spam for them.
        let metrics_not_applicable_ids: std::collections::BTreeSet<ToolpathId> = request
            .groups
            .iter()
            .flat_map(|g| g.toolpaths.iter())
            .filter(|e| e.metrics_not_applicable)
            .map(|e| e.id)
            .collect();
        let mut trace = SimulationCutTrace::from_samples_with_context(
            sample_step_mm,
            cut_samples,
            semantic_traces,
            &metrics_not_applicable_ids,
        );
        trace.provenance = Some(build_simulation_provenance(request));
        trace.drill_samples = std::mem::take(&mut drill_samples_all);
        trace.drill_summaries = std::mem::take(&mut drill_summaries_all);
        // F-034: kinematics-aware cycle time override. When the
        // caller supplied a `KinematicsContext`, recompute each
        // toolpath's `total_runtime_s` from its IR using the
        // trapezoidal integrator and resum the project-wide total.
        // Other fields on the summary (cutting_runtime_s, air_cut_s,
        // engagement averages …) are left untouched — they're
        // measured from dexel samples and aren't directly affected
        // by accel modelling. F-035 will revisit them once predicted
        // effective feed enters the gates.
        if let Some(ctx) = request.kinematics {
            apply_kinematics_cycle_time(&mut trace, request, ctx);
        }
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

/// F-034: walk every toolpath in the request, recompute its runtime
/// using [`crate::machine_kinematics::compute_cycle_time`], and rewrite
/// the per-toolpath + project-wide `total_runtime_s` slots on `trace`.
///
/// All other summary fields stay untouched — they're derived from the
/// dexel-sample stream and aren't sensitive to accel modelling. The
/// resulting trace therefore mixes a kinematics-aware runtime with
/// engagement / chipload / DOC metrics that still reflect the naive
/// segment timing. That's intentional: F-034 is purely additive on
/// the runtime axis. F-035 (predicted feed in gates) is the
/// finding that will reconcile the engagement-side metrics with the
/// kinematics model.
fn apply_kinematics_cycle_time(
    trace: &mut SimulationCutTrace,
    request: &SimulationRequest,
    ctx: KinematicsContext,
) {
    use crate::machine_kinematics::{compute_cycle_time, predicted_feeds_for_toolpath};

    let mut per_toolpath_runtime: BTreeMap<ToolpathId, f64> = BTreeMap::new();
    // F-035 — when the flag is on, also build a per-(toolpath, move)
    // predicted-feed map so the gates can read achieved feed rather
    // than commanded. The same walk that produces cycle time
    // (`compute_cycle_time`) drives the predicted-feed integrator
    // (`predicted_feeds_for_toolpath`) — they share `MoveDigest`
    // construction logic but are intentionally separate functions to
    // keep the runtime-only override (F-034) and the gate plumbing
    // (F-035) independently flag-gated.
    let mut predicted_feeds: crate::machine_kinematics::PredictedFeedMap = BTreeMap::new();
    for group in &request.groups {
        for entry in &group.toolpaths {
            let t = compute_cycle_time(
                &entry.annotated.toolpath,
                &ctx.kinematics,
                ctx.max_feed_mm_min,
                request.rapid_feed_mm_min,
            );
            per_toolpath_runtime.insert(entry.id, t);

            if ctx.use_predicted_feed_in_gates {
                let per_move = predicted_feeds_for_toolpath(
                    &entry.annotated.toolpath,
                    &ctx.kinematics,
                    ctx.max_feed_mm_min,
                    request.rapid_feed_mm_min,
                );
                for (move_idx, feed) in per_move {
                    predicted_feeds.insert((entry.id, move_idx), feed);
                }
            }
        }
    }

    let mut project_total = 0.0;
    for tp_summary in &mut trace.toolpath_summaries {
        if let Some(&t) = per_toolpath_runtime.get(&tp_summary.toolpath_id) {
            tp_summary.total_runtime_s = t;
            project_total += t;
        } else {
            project_total += tp_summary.total_runtime_s;
        }
    }
    trace.summary.total_runtime_s = project_total;

    if ctx.use_predicted_feed_in_gates && !predicted_feeds.is_empty() {
        trace.predicted_feeds = predicted_feeds;
    }
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

    /// PR-4 polish: feed-rate-only edits (which don't change move
    /// geometry) must produce a different `hash_operation_config`
    /// value. Without this, `sim_trace_is_fresh` reports a stale
    /// trace as fresh and the load verdicts evaluate against
    /// outdated samples.
    #[test]
    fn hash_operation_config_differs_on_feed_rate_only_edit() {
        use crate::compute::catalog::OperationConfig;
        use crate::compute::operation_configs::PocketConfig;
        let mut cfg = PocketConfig {
            feed_rate: 1000.0,
            ..PocketConfig::default()
        };
        let a = OperationConfig::Pocket(cfg.clone());
        cfg.feed_rate = 1500.0;
        let b = OperationConfig::Pocket(cfg);
        assert_ne!(
            hash_operation_config(&a),
            hash_operation_config(&b),
            "feed_rate change must invalidate config hash"
        );
    }

    #[test]
    fn hash_operation_config_stable_for_identical_configs() {
        use crate::compute::catalog::OperationConfig;
        use crate::compute::operation_configs::PocketConfig;
        let cfg = PocketConfig::default();
        let a = OperationConfig::Pocket(cfg.clone());
        let b = OperationConfig::Pocket(cfg);
        assert_eq!(hash_operation_config(&a), hash_operation_config(&b));
    }

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
            id: ToolpathId(1),
            name: "Test".to_owned(),
            annotated: Arc::new(AnnotatedToolpath::new(tp)),
            tool,
            flute_count: 2,
            tool_summary: "6mm Flat".to_owned(),
            semantic_trace: None,
            spindle_rpm: None,
            metrics_not_applicable: false,
            drill_op: None,
            operation_config_hash: 0,
        };

        let group = SimGroupEntry {
            toolpaths: vec![entry],
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: None,
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
            kinematics: None,
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
            id: ToolpathId(1),
            name: "Pocket F-2 validation".to_owned(),
            annotated: Arc::new(AnnotatedToolpath::new(tp)),
            tool: tool_def,
            flute_count: 2,
            tool_summary: "6.35mm Flat".to_owned(),
            semantic_trace: None,
            spindle_rpm: None,
            metrics_not_applicable: false,
            drill_op: None,
            operation_config_hash: 0,
        };

        let group = SimGroupEntry {
            toolpaths: vec![entry],
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: None,
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
            kinematics: None,
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
                id: ToolpathId(1),
                name: "Top Cut".into(),
                annotated: Arc::new(AnnotatedToolpath::new(top_tp)),
                tool: make_tool(),
                flute_count: 2,
                tool_summary: "6mm Flat".into(),
                semantic_trace: None,
                spindle_rpm: None,
                metrics_not_applicable: false,
                drill_op: None,
                operation_config_hash: 0,
            }],
            direction: StockCutDirection::FromTop,
            local_stock_bbox: Some(stock_bbox),
            local_to_global: None, // identity setup
            phantom_prior_stock: None,
        };

        let bottom_group = SimGroupEntry {
            toolpaths: vec![SimToolpathEntry {
                id: ToolpathId(2),
                name: "Bottom Cut".into(),
                annotated: Arc::new(AnnotatedToolpath::new(bottom_tp)),
                tool: make_tool(),
                flute_count: 2,
                tool_summary: "6mm Flat".into(),
                semantic_trace: None,
                spindle_rpm: None,
                metrics_not_applicable: false,
                drill_op: None,
                operation_config_hash: 0,
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
            phantom_prior_stock: None,
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
            kinematics: None,
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

    /// F.4 — the `FromRemainingStock` regeneration catch-22: a group with
    /// one generated entry (A) and a phantom slot for a not-yet-generated
    /// toolpath (B) at the tail position (`k == toolpaths.len()`) must
    /// populate `prior_stocks` for BOTH ids — A's own pre-carve snapshot
    /// (existing behavior, unaffected) and B's phantom snapshot taken
    /// after A has carved. Before this feature, B — never present in a
    /// `SimGroupEntry` because it was never generated — could never
    /// receive a `prior_stocks` entry at all, so a `FromRemainingStock`
    /// op could never regenerate after a fresh project load.
    #[test]
    fn phantom_prior_stock_populates_pending_op_snapshot() {
        let mut req = simple_request();
        let generated_id = req.groups[0].toolpaths[0].id;
        let phantom_id = ToolpathId(2);
        req.groups[0].phantom_prior_stock = Some((1, phantom_id));

        let cancel = AtomicBool::new(false);
        let result = run_simulation(&req, &cancel).unwrap();

        let a_stock = result
            .prior_stocks
            .get(&generated_id)
            .expect("A's own pre-carve snapshot must still be present");
        let b_stock = result
            .prior_stocks
            .get(&phantom_id)
            .expect("B's phantom post-A snapshot must be present");

        // A's move (rapid to (0,0,10), feed to (10,0,-1)) carves under its
        // path; sample a cell on that path and confirm the phantom (taken
        // after A carved) differs from A's own pre-carve snapshot.
        let (r, c) = a_stock
            .z_grid
            .world_to_cell(5.0, 0.0)
            .expect("sample cell should exist in the stock grid");
        let pre_a_ray = a_stock.z_grid.ray(r, c);
        let post_a_ray = b_stock.z_grid.ray(r, c);
        assert_ne!(
            pre_a_ray, post_a_ray,
            "phantom snapshot should reflect A's carve; A's own snapshot must not"
        );
    }
}
