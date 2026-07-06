//! Unified ProjectSession API — a single entry point for project state + compute
//! that GUI, CLI, and a future MCP server can all use.
//!
//! # Usage
//!
//! ```ignore
//! let mut session = ProjectSession::load(Path::new("my_project.toml"))?;
//! let cancel = AtomicBool::new(false);
//! session.generate_all(&[], &cancel)?;
//! session.run_simulation(SimulationOptions::default(), &cancel)?;
//! let diag = session.diagnostics();
//! ```

mod compute;
mod eval_context;
mod mutation;
pub mod project_file;
mod save;
pub mod wizard;

pub use compute::{MutationKind, StaleSet, compute_stale_set};
pub use eval_context::SetupEvalContext;
pub use wizard::{OutputLayout, WizardState};

// Re-export all public project_file types so external crates see no path change.
pub use project_file::{
    ProjectFile, ProjectFixtureSection, ProjectJobSection, ProjectKeepOutSection,
    ProjectModelSection, ProjectPostConfig, ProjectSetupSection, ProjectStockConfig,
    ProjectToolSection, ProjectToolpathSection,
};

use crate::ids::ToolpathId;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, StockSource, ToolpathStats,
};
use crate::compute::simulate::SimulationResult;
use crate::compute::stock_config::{FixtureId, KeepOutId, ModelKind, ModelUnits, StockConfig};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::transform::{FaceUp, ZRotation};
use crate::debug_trace::{ToolpathDebugOptions, ToolpathDebugTrace};
use crate::dxf_input::DrillTarget;
use crate::enriched_mesh::{EnrichedMesh, FaceGroupId};
use crate::gcode::CoolantMode;
use crate::geo::{BoundingBox3, P3};
use crate::mesh::TriangleMesh;
use crate::polygon::Polygon2;
use crate::semantic_trace::ToolpathSemanticTrace;
use crate::toolpath::Toolpath;

use crate::compute::collision_check::CollisionCheckError;
use crate::compute::simulate::SimulationError;

// ── Error types ────────────────────────────────────────────────────────

/// Errors that can occur during session operations.
#[derive(Debug)]
pub enum SessionError {
    /// I/O error (file not found, permission denied, etc.).
    Io(std::io::Error),
    /// TOML parsing error.
    TomlParse(String),
    /// TOML serialization error.
    TomlSerialize(String),
    /// Model loading failure.
    ModelLoad { name: String, detail: String },
    /// Toolpath not found by index.
    ToolpathNotFound(usize),
    /// Toolpath not found by stable id (R3 — distinct from the
    /// index-carrying `ToolpathNotFound` so id/index can't conflate).
    ToolpathIdNotFound(ToolpathId),
    /// Tool not found by id.
    ToolNotFound(ToolId),
    /// Setup not found by index.
    SetupNotFound(usize),
    /// Tool still referenced by toolpaths — cannot remove.
    ToolInUse(ToolId),
    /// Setup still has toolpaths — cannot remove.
    SetupHasToolpaths(usize),
    /// Geometry missing for the requested operation.
    MissingGeometry(String),
    /// Operation execution failure.
    OperationFailed(String),
    /// Simulation error.
    Simulation(SimulationError),
    /// Collision check error.
    CollisionCheck(CollisionCheckError),
    /// Export error.
    Export(String),
    /// Invalid parameter name or value.
    InvalidParam(String),
    /// Parsed TOML doesn't look like an rs_cam project.
    NotACamProject(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::TomlParse(e) => write!(f, "TOML parse error: {e}"),
            Self::TomlSerialize(e) => write!(f, "TOML serialize error: {e}"),
            Self::ModelLoad { name, detail } => {
                write!(f, "Failed to load model '{name}': {detail}")
            }
            Self::ToolpathNotFound(index) => write!(f, "Toolpath {index} not found"),
            Self::ToolpathIdNotFound(id) => write!(f, "Toolpath id {id} not found"),
            Self::ToolNotFound(id) => write!(f, "Tool {} not found", id.0),
            Self::SetupNotFound(id) => write!(f, "Setup {id} not found"),
            Self::ToolInUse(id) => write!(f, "Tool {} is still referenced by toolpaths", id.0),
            Self::SetupHasToolpaths(id) => {
                write!(f, "Setup {id} still has toolpaths — remove them first")
            }
            Self::MissingGeometry(msg) => write!(f, "Missing geometry: {msg}"),
            Self::OperationFailed(msg) => write!(f, "Operation failed: {msg}"),
            Self::Simulation(e) => write!(f, "Simulation error: {e}"),
            Self::CollisionCheck(e) => write!(f, "Collision check error: {e}"),
            Self::Export(msg) => write!(f, "Export error: {msg}"),
            Self::InvalidParam(msg) => write!(f, "Invalid parameter: {msg}"),
            Self::NotACamProject(detail) => write!(f, "Not an rs_cam project: {detail}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<std::io::Error> for SessionError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<SimulationError> for SessionError {
    fn from(e: SimulationError) -> Self {
        Self::Simulation(e)
    }
}

impl From<CollisionCheckError> for SessionError {
    fn from(e: CollisionCheckError) -> Self {
        Self::CollisionCheck(e)
    }
}

// ── Loaded state types ─────────────────────────────────────────────────

/// Geometry loaded from a model file.
pub(crate) enum LoadedGeometry {
    Mesh(TriangleMesh),
    /// 2D polygons plus pickable drill targets and their layer names
    /// (DXF imports; SVG passes empty target/layer lists).
    Polygons(Vec<Polygon2>, Vec<DrillTarget>, Vec<String>),
    /// Mesh + BREP face groups (STEP / CAD models). The enriched form
    /// is required for face-selective operations; downgrading to a
    /// flat `Mesh` silently strips topology and breaks face pickers.
    /// Only constructed when the `step` cargo feature is on; the
    /// match arm consuming it is also gated, but the variant lives
    /// outside the cfg so callers needn't sprinkle cfg-pattern matches.
    #[cfg_attr(not(feature = "step"), allow(dead_code))]
    Enriched(EnrichedMesh),
}

/// A loaded model with its geometry.
pub struct LoadedModel {
    pub id: usize,
    pub name: String,
    pub mesh: Option<Arc<TriangleMesh>>,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
    /// Pickable drill targets extracted from the source (DXF POINT entities
    /// and circle/arc centres). Empty for meshes and SVG.
    pub drill_targets: Arc<Vec<DrillTarget>>,
    /// Distinct layer names that contain drill targets (sorted). Empty for
    /// formats without layers.
    pub layers: Arc<Vec<String>>,
    /// Original file path (for save round-trip).
    pub path: std::path::PathBuf,
    /// File kind (stl, svg, dxf, step).
    pub kind: Option<ModelKind>,
    /// Assumed units of the source file (determines scale factor to mm).
    pub units: Option<ModelUnits>,
    /// Enriched mesh with BREP face groups (for STEP/CAD models).
    pub enriched_mesh: Option<Arc<EnrichedMesh>>,
    /// Percentage of inconsistent winding edges. `None` if not STL.
    pub winding_report: Option<f64>,
    /// Load/import failure preserved so broken references can round-trip.
    pub load_error: Option<String>,
}

impl LoadedModel {
    /// Load a model directly from a file path, using the SAME geometry
    /// pipeline as the project loader (STL/SVG/DXF/STEP dispatch, unit
    /// scaling, BREP enrichment) — added for the registry-driven CLI
    /// `run` subcommand (T9). One-off model loads should come through
    /// here rather than re-rolling the format dispatch.
    ///
    /// `kind`/`units` of `None` infer from the file extension / assume
    /// millimeters, matching the project loader's defaults. Relative
    /// paths resolve against `base_dir`.
    pub fn from_file(
        id: usize,
        name: &str,
        path: &std::path::Path,
        kind: Option<ModelKind>,
        units: Option<ModelUnits>,
        base_dir: &std::path::Path,
    ) -> Result<Self, SessionError> {
        let section = project_file::ProjectModelSection {
            id: Some(id),
            path: path.to_string_lossy().into_owned(),
            name: name.to_owned(),
            kind,
            units,
        };
        let resolved_kind = kind.or_else(|| project_file::infer_model_kind(path));
        let geometry = project_file::load_model_geometry(&section, base_dir)?;
        let mut drill_targets: Arc<Vec<DrillTarget>> = Arc::new(Vec::new());
        let mut layers: Arc<Vec<String>> = Arc::new(Vec::new());
        let (mesh, polygons, enriched_mesh) = match geometry {
            LoadedGeometry::Mesh(mesh) => (Some(Arc::new(mesh)), None, None),
            LoadedGeometry::Polygons(polys, targets, layer_names) => {
                drill_targets = Arc::new(targets);
                layers = Arc::new(layer_names);
                (None, Some(Arc::new(polys)), None)
            }
            LoadedGeometry::Enriched(enriched) => {
                let mesh_arc = Arc::clone(&enriched.mesh);
                (Some(mesh_arc), None, Some(Arc::new(enriched)))
            }
        };
        Ok(Self {
            id,
            name: name.to_owned(),
            mesh,
            polygons,
            drill_targets,
            layers,
            path: path.to_path_buf(),
            kind: resolved_kind,
            units,
            enriched_mesh,
            winding_report: None,
            load_error: None,
        })
    }

    /// Construct a placeholder model for a file that failed to load.
    ///
    /// The path, name, kind, and units are preserved so the broken reference
    /// can round-trip through save/load.
    pub fn placeholder(
        id: usize,
        path: std::path::PathBuf,
        name: String,
        kind: ModelKind,
        units: ModelUnits,
        load_error: String,
    ) -> Self {
        Self {
            id,
            name,
            mesh: None,
            polygons: None,
            drill_targets: Arc::new(Vec::new()),
            layers: Arc::new(Vec::new()),
            path,
            kind: Some(kind),
            units: Some(units),
            enriched_mesh: None,
            winding_report: None,
            load_error: Some(load_error),
        }
    }

    /// Compute the bounding box of the model's geometry.
    ///
    /// For mesh models, returns the stored mesh bbox. For 2D polygon models,
    /// computes the bbox from exterior + hole points at Z=0. Returns `None`
    /// if no geometry is loaded.
    pub fn bbox(&self) -> Option<BoundingBox3> {
        if let Some(mesh) = &self.mesh {
            return Some(mesh.bbox);
        }

        let polygons = self.polygons.as_deref()?;
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for polygon in polygons {
            for point in polygon
                .exterior
                .iter()
                .chain(polygon.holes.iter().flat_map(|hole| hole.iter()))
            {
                min_x = min_x.min(point.x);
                min_y = min_y.min(point.y);
                max_x = max_x.max(point.x);
                max_y = max_y.max(point.y);
            }
        }

        if !min_x.is_finite() {
            return None;
        }

        Some(BoundingBox3 {
            min: crate::geo::P3::new(min_x, min_y, 0.0),
            max: crate::geo::P3::new(max_x, max_y, 0.0),
        })
    }
}

/// Kind of workholding fixture (compute-relevant subset of viz `FixtureKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
    #[default]
    Clamp,
    Vise,
    VacuumPod,
    Custom,
}

impl FixtureKind {
    pub fn from_key(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "vise" => Self::Vise,
            "vacuum_pod" | "vacuumpod" => Self::VacuumPod,
            "custom" => Self::Custom,
            _ => Self::Clamp,
        }
    }
}

/// A physical workholding fixture — compute-relevant fields only.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Fixture {
    pub id: FixtureId,
    pub name: String,
    pub kind: FixtureKind,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Position of the fixture's min corner in workpiece coordinates (mm).
    #[serde(default)]
    pub origin_x: f64,
    #[serde(default)]
    pub origin_y: f64,
    #[serde(default)]
    pub origin_z: f64,
    /// Dimensions of the fixture bounding box (mm).
    #[serde(default = "default_fixture_size")]
    pub size_x: f64,
    #[serde(default = "default_fixture_size")]
    pub size_y: f64,
    #[serde(default = "default_fixture_height")]
    pub size_z: f64,
    /// Extra clearance around the fixture for tool avoidance (mm).
    #[serde(default = "default_fixture_clearance")]
    pub clearance: f64,
}

fn default_true() -> bool {
    true
}
fn default_fixture_size() -> f64 {
    30.0
}
fn default_fixture_height() -> f64 {
    20.0
}
fn default_fixture_clearance() -> f64 {
    3.0
}

impl Fixture {
    /// XY footprint polygon (with clearance) for boundary subtraction.
    pub fn footprint(&self) -> Polygon2 {
        let min_x = self.origin_x - self.clearance;
        let min_y = self.origin_y - self.clearance;
        let max_x = self.origin_x + self.size_x + self.clearance;
        let max_y = self.origin_y + self.size_y + self.clearance;
        Polygon2::rectangle(min_x, min_y, max_x, max_y)
    }
}

/// A rectangular region the tool must avoid (XY only, full Z extent).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeepOutZone {
    pub id: KeepOutId,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Position of the zone's min corner (mm).
    #[serde(default)]
    pub origin_x: f64,
    #[serde(default)]
    pub origin_y: f64,
    /// Dimensions of the zone (mm).
    #[serde(default = "default_keep_out_size")]
    pub size_x: f64,
    #[serde(default = "default_keep_out_size")]
    pub size_y: f64,
}

fn default_keep_out_size() -> f64 {
    20.0
}

impl KeepOutZone {
    /// XY footprint polygon for boundary subtraction.
    pub fn footprint(&self) -> Polygon2 {
        Polygon2::rectangle(
            self.origin_x,
            self.origin_y,
            self.origin_x + self.size_x,
            self.origin_y + self.size_y,
        )
    }
}

/// A setup's orientation and toolpath indices.
pub struct SetupData {
    pub id: usize,
    pub name: String,
    pub face_up: FaceUp,
    /// Rotation of the stock about the vertical (Z) axis.
    pub z_rotation: ZRotation,
    /// Workholding fixtures in this setup.
    pub fixtures: Vec<Fixture>,
    /// Keep-out zones in this setup.
    pub keep_out_zones: Vec<KeepOutZone>,
    /// Indices into the session's `toolpath_configs` vec.
    pub toolpath_indices: Vec<usize>,
    /// Optional override for the M0 pause message emitted before this setup.
    /// `None` falls back to the default `Setup change: <name>` text. Used to
    /// instruct the operator (e.g. "Run Z Probe macro then Resume") between
    /// setups; the actual probe / home gcode lives in the sender's macro.
    pub pause_message: Option<String>,
}

/// Configuration for a single toolpath within the session.
pub struct ToolpathConfig {
    pub id: ToolpathId,
    pub name: String,
    pub enabled: bool,
    pub operation: OperationConfig,
    pub dressups: DressupConfig,
    pub heights: HeightsConfig,
    pub tool_id: usize,
    pub model_id: usize,
    /// Raw G-code to emit before this toolpath's moves.
    pub pre_gcode: Option<String>,
    /// Raw G-code to emit after this toolpath's moves.
    pub post_gcode: Option<String>,
    /// Machining boundary configuration.
    pub boundary: BoundaryConfig,
    /// When true, inherit boundary from stock default.
    pub boundary_inherit: bool,
    /// Where this toolpath's stock material comes from.
    pub stock_source: StockSource,
    /// Coolant mode for G-code output.
    pub coolant: CoolantMode,
    /// Optional BREP face selection (for STEP/CAD models).
    pub face_selection: Option<Vec<FaceGroupId>>,
    /// Debug trace options for this toolpath.
    pub debug_options: ToolpathDebugOptions,
    /// Per-dimension provenance of the feeds stored on `operation` — how each
    /// applied value was produced (vendor LUT / formula / manual / optimizer /
    /// auto-correct). Stamped at write time and read back by the UI instead of
    /// recomputing a fresh lookup. Defaults to all-`None` ("config default").
    pub feeds_provenance: crate::feeds::FeedsProvenance,
}

/// Result of generating a single toolpath.
///
/// `op_data` carries either a plain [`crate::toolpath_spans::AnnotatedToolpath`]
/// or a [`crate::drill_op::DrillOp`] + `AnnotatedToolpath` pair (the
/// dual-representation invariant from §6.E of the dexel-fidelity roadmap).
/// Spans on the annotated toolpath are emitted by operation generators;
/// transforms (dressups, boundary clip, TSP, arc-fit, feed optimisation)
/// either remap them or set
/// [`AnnotatedToolpath::spans_valid`](crate::toolpath_spans::AnnotatedToolpath::spans_valid)
/// to `false` when they can't.
pub struct ToolpathComputeResult {
    pub op_data: crate::drill_op::OpData,
    pub stats: ToolpathStats,
    pub debug_trace: Option<ToolpathDebugTrace>,
    pub semantic_trace: Option<ToolpathSemanticTrace>,
}

impl ToolpathComputeResult {
    /// Convenience accessor for the linearized annotated toolpath.
    /// Returns a reference regardless of whether `op_data` is the plain
    /// `Toolpath` or the `DrillOp` variant.
    pub fn annotated(&self) -> &Arc<crate::toolpath_spans::AnnotatedToolpath> {
        self.op_data.annotated()
    }

    /// Convenience accessor for the underlying [`Toolpath`].
    pub fn toolpath(&self) -> &Toolpath {
        &self.op_data.annotated().toolpath
    }

    /// First-class drill-op view, if this is a drilling operation.
    pub fn drill_op(&self) -> Option<&Arc<crate::drill_op::DrillOp>> {
        self.op_data.drill_op()
    }

    /// True if this result represents a drilling operation
    /// (`OperationConfig::Drill` or `AlignmentPinDrill`).
    pub fn is_drill_op(&self) -> bool {
        self.op_data.is_drill_op()
    }
}

/// Summary of a toolpath for listing.
#[derive(serde::Serialize)]
pub struct ToolpathSummary {
    pub index: usize,
    pub id: ToolpathId,
    pub name: String,
    pub operation_label: String,
    pub enabled: bool,
    pub tool_name: String,
}

/// Summary of a tool for listing.
///
/// UX dial-in A8 — `diameter` is the cutter's named (tip) diameter. The
/// LUT chipload lookup uses an *effective* diameter that depends on
/// engagement depth (`feeds::geometry::ball_effective_diameter` /
/// `tapered_ball_effective_diameter`), which can differ substantially
/// for tapered / ball / bullnose tools. Geometry context is included so
/// consumers can correlate the named diameter with the effective
/// LUT-lookup diameter rather than reading a single number that doesn't
/// tell the whole story.
#[derive(serde::Serialize)]
pub struct ToolSummary {
    pub id: ToolId,
    pub name: String,
    pub tool_type: ToolType,
    /// Named cutter diameter (mm). For tapered / ball tools this is the
    /// tip diameter; the LUT-effective diameter scales up with axial
    /// depth.
    pub diameter: f64,
    /// Cutting flute length (mm) — sets the upper bound on engagement
    /// depth and on how much a tapered tool's effective diameter can
    /// grow.
    pub cutting_length: f64,
    /// Taper half-angle in degrees (TaperedBallNose only — 0 for other
    /// tools).
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub taper_half_angle_deg: f64,
    /// Corner radius (mm) for BullNose tools — 0 for endmills / vbits.
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub corner_radius_mm: f64,
    /// Included angle (degrees) for V-bits — 0 for non-V tools.
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub included_angle_deg: f64,
    /// Flute count — used by both feeds/speeds and chipload-per-tooth.
    pub flute_count: u32,
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

/// Options for running simulation.
pub struct SimulationOptions {
    /// Resolution in mm for tri-dexel stock.
    pub resolution: f64,
    /// Toolpath IDs to skip.
    pub skip_ids: Vec<ToolpathId>,
    /// Whether to collect detailed cut metrics.
    pub metrics_enabled: bool,
    /// When `true`, override `resolution` with an auto-computed value based
    /// on the smallest tool radius and the stock footprint (matching the GUI's
    /// auto-resolution logic).
    pub auto_resolution: bool,
    /// F-035: when `true` **and** the active `MachineProfile` carries
    /// `kinematics`, the simulator builds a per-move `PredictedFeedMap`
    /// (peak achievable feed under accel/jerk limits) and the
    /// chipload + power gates consume that predicted feed instead of
    /// the commanded `feed_rate_mm_min` on each sample.
    ///
    /// Default `false`. With the flag off **or** kinematics absent,
    /// gate evaluation is byte-identical to pre-F-035 — the loop's
    /// acceptance tests (`_f0{24,26,27,28,31}.rs`) and the smoke
    /// baseline at `planning/toolpath_acceptance/baselines/2026-05-26.csv`
    /// continue to pass unchanged.
    ///
    /// The bug this catches: hobby-class controllers decelerate
    /// through corners. Commanded 4000 mm/min, achieved 2000 mm/min,
    /// chipload gate at 2000 fires `Exceeds_LOW` (rubbing / burning)
    /// while the gate at 4000 reads `Within`. See
    /// `planning/acceptance_loop/findings/F-035-predicted-feed-in-gates.md`.
    pub use_predicted_feed_in_gates: bool,
    /// F-036b: when `true` **and** the active `MachineProfile` carries
    /// `kinematics`, the simulator runs a post-sim modulation pass that
    /// rewrites per-move `feed_rate` on every cutting move so the
    /// commanded chipload-per-tooth lands inside the LUT band's
    /// geometric midpoint (corrected for chip thinning) — Fusion HSM's
    /// "adaptive feed control" equivalent.
    ///
    /// Default `false`. Behavior with the flag off (or with kinematics
    /// absent, or with no vendor LUT row for the active
    /// tool/material/op) is byte-identical to pre-F-036b — the
    /// loop's acceptance tests (`_f0{24,26,27,28,31}.rs`) and the
    /// smoke baseline at
    /// `planning/toolpath_acceptance/baselines/2026-05-26.csv`
    /// continue to pass unchanged.
    ///
    /// Modulation requires a vendor `ChiploadBand` (LUT
    /// `chip_load_min_mm` + `chip_load_max_mm`) for the active
    /// `(tool family, material, op family, pass role, diameter)` tuple.
    /// Custom materials, unsupported op families, and toolpaths whose
    /// LUT row is missing either bound fall through as a no-op (the
    /// per-toolpath feed_rate stays at the commanded value).
    ///
    /// The algorithm itself lives in
    /// `crate::feed_modulation::adaptive_feed_modulate`; see
    /// `planning/acceptance_loop/findings/F-036b-feed-modulation-flag-plumbing.md`.
    pub adaptive_feed_modulation: bool,
    /// F-039 — which modulation algorithm runs when
    /// `adaptive_feed_modulation == true`.
    ///
    /// `ConstrainedMax` (the new default) solves the per-move
    /// constrained optimisation problem (chipload-max / deflection-
    /// max / power-max / machine-max / kinematic-reach / chipload-min)
    /// and emits at the binding constraint. `BandMid` is the F-036
    /// "target band-mid" heuristic, kept as a fallback for one
    /// release cycle. Either way, the modulator only fires when the
    /// active `MachineProfile` carries `kinematics` and the LUT has
    /// a chipload band for the toolpath; otherwise both strategies
    /// short-circuit to a no-op.
    pub modulation_strategy: crate::feed_modulation::ModulationStrategy,
    /// F-039 — aggressiveness scalar applied to the constrained-max
    /// limit before the chipload-min floor.
    ///
    /// `1.0` = emit at the binding constraint (production CAM
    /// default — Fusion HSM 100 %). `0.7` = back off 30 % for
    /// safety margin. `1.1` = push 10 % past the limit (NOT
    /// recommended; chipload-min still applies and the diagnostic
    /// readout flags the over-aggressive setting). Ignored when
    /// `modulation_strategy == BandMid`.
    pub modulation_aggressiveness: f64,
}

impl Default for SimulationOptions {
    fn default() -> Self {
        Self {
            resolution: 0.5,
            skip_ids: Vec::new(),
            metrics_enabled: true,
            auto_resolution: false,
            use_predicted_feed_in_gates: false,
            adaptive_feed_modulation: false,
            modulation_strategy: crate::feed_modulation::ModulationStrategy::ConstrainedMax,
            modulation_aggressiveness: 1.0,
        }
    }
}

/// Per-toolpath diagnostic summary.
#[derive(Debug, Clone)]
pub struct ToolpathDiagnostic {
    pub toolpath_id: ToolpathId,
    pub name: String,
    pub operation_type: String,
    /// Stable op-kind tag (e.g. `"drill"`, `"alignment_pin_drill"`, `"pocket"`).
    /// Lets consumers filter Z-only kinematics ops out of rapid:cut-ratio
    /// signals — see [`crate::compute::catalog::OperationType`].
    pub op_kind: String,
    pub tool_name: String,
    pub move_count: usize,
    pub cutting_distance_mm: f64,
    pub rapid_distance_mm: f64,
    pub collision_count: usize,
    pub rapid_collision_count: usize,
}

/// Severity bucket for a [`Verdict`]. Ordered: `Critical < Important < Polish`
/// so a sort on severity puts the most important first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VerdictSeverity {
    /// Holder/shank collision, rapid-through-stock collision — likely to
    /// damage the part or the tool. Block export until resolved.
    Critical,
    /// Gate exceeded, toolpath generated zero in-material cut, unsafe plunge —
    /// the operator must consciously decide before proceeding.
    Important,
    /// Air-cut high, low engagement, slow cycle time — cosmetic / efficiency.
    Polish,
}

/// What triggered a [`Verdict`]. Stable tag so MCP consumers can branch on
/// kind without parsing the human-readable headline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictKind {
    HolderCollision,
    RapidCollision,
    PlungeStress,
    AirCut,
    GeneratedEmpty,
}

impl VerdictKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HolderCollision => "holder_collision",
            Self::RapidCollision => "rapid_collision",
            Self::PlungeStress => "plunge_stress",
            Self::AirCut => "air_cut",
            Self::GeneratedEmpty => "generated_empty",
        }
    }
}

/// Backing evidence for a verdict — the move / Z that triggered the call.
/// Optional because some verdicts (e.g. holder collision summed across a
/// project) don't have a single representative move.
#[derive(Debug, Clone, Default)]
pub struct VerdictEvidence {
    pub move_index: Option<usize>,
    pub z_value: Option<f64>,
    pub count: Option<usize>,
}

/// Borrow view over the simulation evidence that
/// [`ProjectSession::diagnostics_with_evidence`] (and friends) need.
///
/// Both the core session and the GUI hold the same evidence — but the
/// GUI's copy lives in a different struct (viz `SimulationResults` +
/// `SimulationChecks`). Both sides key boundaries by the shared
/// strongly-typed [`ToolpathId`] (R3 unified the previously-`usize`
/// core side). This shared borrow view lets both callers feed
/// `diagnostics_with_evidence` without forcing a copy of the full
/// simulation result.
#[derive(Default)]
pub struct ProjectEvidence<'a> {
    /// `(toolpath_id, start_move, end_move)` per simulation boundary —
    /// the rapid-collision counters use this to attribute counts to
    /// the right toolpath. The viz side flattens its
    /// `Vec<ToolpathBoundary>` into this shape.
    pub boundaries: Vec<(ToolpathId, usize, usize)>,
    pub rapid_collisions: &'a [crate::collision::RapidCollision],
    pub rapid_collision_move_indices: &'a [usize],
    pub cut_trace: Option<&'a crate::simulation_cut::SimulationCutTrace>,
    /// Per-toolpath holder/shank collision counts, supplied by the
    /// caller from its most recent dedicated collision check. Empty =
    /// no holder evidence (no holder verdicts are emitted).
    ///
    /// Evidence is an INPUT here on purpose: `diagnostics_with_evidence`
    /// used to run `collision_check` (spatial-index build + full
    /// toolpath sweep) per toolpath internally, and the GUI's setup
    /// panel calls project diagnostics every frame — on a generated
    /// project that recomputed every toolpath's collision sweep at
    /// frame rate (the 2026-06-11 setup-tab lag). Batch callers that
    /// want the sweep use [`ProjectSession::holder_collision_counts`].
    pub holder_collisions: Vec<(ToolpathId, usize)>,
}

impl<'a> ProjectEvidence<'a> {
    /// Build evidence from a core [`SimulationResult`].
    pub fn from_simulation(sim: &'a SimulationResult) -> Self {
        let boundaries = sim
            .boundaries
            .iter()
            .map(|b| (b.id, b.start_move, b.end_move))
            .collect();
        Self {
            boundaries,
            rapid_collisions: &sim.rapid_collisions,
            rapid_collision_move_indices: &sim.rapid_collision_move_indices,
            cut_trace: sim.cut_trace.as_deref(),
            holder_collisions: Vec::new(),
        }
    }

    /// Same as [`Self::from_simulation`] plus holder-collision counts
    /// (see the `holder_collisions` field docs for why these are an
    /// input rather than computed internally).
    pub fn from_simulation_with_holder_collisions(
        sim: &'a SimulationResult,
        holder_collisions: Vec<(ToolpathId, usize)>,
    ) -> Self {
        Self {
            holder_collisions,
            ..Self::from_simulation(sim)
        }
    }
}

/// Structured project-level verdict. Replaces the legacy single-line
/// `ProjectDiagnostics::verdict` string — that field is still populated
/// (with the highest-severity verdict's headline) for backward compatibility
/// but new consumers should read [`ProjectDiagnostics::verdicts`].
#[derive(Debug, Clone)]
pub struct Verdict {
    pub severity: VerdictSeverity,
    pub kind: VerdictKind,
    /// One-line human-readable headline, names the offending TPs.
    pub headline: String,
    /// Toolpath ids this verdict refers to (may be empty for project-wide
    /// signals).
    pub offender_toolpath_ids: Vec<ToolpathId>,
    /// Suggested next action ("increase retract_z…", "set boundary…").
    pub fix_hint: String,
    pub evidence: VerdictEvidence,
}

/// Project-level diagnostics summary.
#[derive(Debug, Clone)]
pub struct ProjectDiagnostics {
    pub total_runtime_s: f64,
    pub air_cut_percentage: f64,
    pub average_engagement: f64,
    pub collision_count: usize,
    pub rapid_collision_count: usize,
    pub per_toolpath: Vec<ToolpathDiagnostic>,
    /// Legacy single-line verdict. Derived from the highest-severity entry
    /// in [`Self::verdicts`]; `"OK"` when no verdicts fire. Kept for old
    /// consumers — new code should read `verdicts` directly.
    pub verdict: String,
    /// Severity-ranked list of structured verdicts (critical → polish).
    /// Empty when the project has no findings.
    pub verdicts: Vec<Verdict>,
}

// ── ProjectSession ─────────────────────────────────────────────────────

/// Unified project session that owns state and provides compute methods.
///
/// Use [`ProjectSession::load`] to load from a TOML project file, or
/// [`ProjectSession::from_project_file`] to construct from a parsed file.
pub struct ProjectSession {
    // Project metadata
    pub(crate) name: String,
    pub(crate) stock: StockConfig,
    pub(crate) post: ProjectPostConfig,
    pub(crate) machine: crate::machine::MachineProfile,
    /// Name of the library machine this project references, if any. When
    /// set, `machine` was resolved from the library on load and is
    /// re-persisted as an offline fallback alongside the ref.
    pub(crate) machine_ref: Option<String>,

    // Loaded state
    pub(crate) models: Vec<LoadedModel>,
    pub(crate) tools: Vec<ToolConfig>,
    pub(crate) setups: Vec<SetupData>,
    pub(crate) toolpath_configs: Vec<ToolpathConfig>,

    // Computed results (keyed by toolpath index)
    pub(crate) results: HashMap<usize, ToolpathComputeResult>,
    pub(crate) simulation: Option<SimulationResult>,

    // Resumable export-wizard settings.
    pub(crate) wizard: WizardState,

    // ID generators (max existing ID + 1)
    pub(crate) next_toolpath_id: usize,
    pub(crate) next_tool_id: usize,
    pub(crate) next_setup_id: usize,
    pub(crate) next_model_id: usize,
}

impl ProjectSession {
    // ── Lifecycle ───────────────────────────────────────────────────

    /// Create an empty session (for untitled / new projects).
    pub fn new_empty() -> Self {
        Self {
            name: String::new(),
            stock: StockConfig::default(),
            post: ProjectPostConfig::default(),
            machine: crate::machine::MachineProfile::default(),
            machine_ref: None,
            models: Vec::new(),
            tools: Vec::new(),
            setups: vec![SetupData {
                id: 0,
                name: "Setup 1".to_owned(),
                face_up: FaceUp::default(),
                z_rotation: ZRotation::default(),
                fixtures: Vec::new(),
                keep_out_zones: Vec::new(),
                toolpath_indices: Vec::new(),
                pause_message: None,
            }],
            toolpath_configs: Vec::new(),
            results: HashMap::new(),
            simulation: None,
            wizard: WizardState::default(),
            next_toolpath_id: 0,
            next_tool_id: 0,
            next_setup_id: 1,
            next_model_id: 0,
        }
    }

    /// Load a project from a TOML file path.
    pub fn load(path: &Path) -> Result<Self, SessionError> {
        let content = std::fs::read_to_string(path)?;
        let project: ProjectFile =
            toml::from_str(&content).map_err(|e| SessionError::TomlParse(e.to_string()))?;
        project_file::validate_looks_like_cam_project(&project, Some(path))?;
        let base_dir = path.parent().unwrap_or(Path::new("."));
        let mut session = Self::from_project_file(project, base_dir)?;
        if (session.name == "Untitled" || session.name.trim().is_empty())
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        {
            session.name = stem.to_owned();
        }
        Ok(session)
    }

    /// Construct a session from a parsed project file.
    pub fn from_project_file(project: ProjectFile, base_dir: &Path) -> Result<Self, SessionError> {
        project_file::build_session_from_project(project, base_dir)
    }

    // ── Queries ────────────────────────────────────────────────────

    /// Project name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Stock configuration.
    pub fn stock_config(&self) -> &StockConfig {
        &self.stock
    }

    /// Bounding box of the stock.
    pub fn stock_bbox(&self) -> BoundingBox3 {
        self.stock.bbox()
    }

    /// List all toolpaths with summary info.
    pub fn list_toolpaths(&self) -> Vec<ToolpathSummary> {
        self.toolpath_configs
            .iter()
            .enumerate()
            .map(|(idx, tc)| {
                let tool_name = self
                    .find_tool_by_raw_id(tc.tool_id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| "Unknown tool".to_owned());
                ToolpathSummary {
                    index: idx,
                    id: tc.id,
                    name: tc.name.clone(),
                    operation_label: tc.operation.label().to_owned(),
                    enabled: tc.enabled,
                    tool_name,
                }
            })
            .collect()
    }

    /// List all tools with summary info.
    pub fn list_tools(&self) -> Vec<ToolSummary> {
        self.tools
            .iter()
            .map(|t| ToolSummary {
                id: t.id,
                name: t.name.clone(),
                tool_type: t.tool_type,
                diameter: t.diameter,
                cutting_length: t.cutting_length,
                taper_half_angle_deg: t.taper_half_angle,
                corner_radius_mm: t.corner_radius_mm.max(t.corner_radius),
                included_angle_deg: t.included_angle,
                flute_count: t.flute_count,
            })
            .collect()
    }

    /// Get the operation config for a toolpath by index.
    pub fn get_toolpath_params(&self, index: usize) -> Option<&OperationConfig> {
        self.toolpath_configs.get(index).map(|tc| &tc.operation)
    }

    /// Get a tool by its `ToolId`.
    pub fn get_tool(&self, id: ToolId) -> Option<&ToolConfig> {
        self.tools.iter().find(|t| t.id == id)
    }

    /// Get a computed toolpath result by index.
    pub fn get_result(&self, index: usize) -> Option<&ToolpathComputeResult> {
        self.results.get(&index)
    }

    /// Get the simulation result, if one has been run.
    pub fn simulation_result(&self) -> Option<&SimulationResult> {
        self.simulation.as_ref()
    }

    /// Number of toolpath configs in the session.
    pub fn toolpath_count(&self) -> usize {
        self.toolpath_configs.len()
    }

    /// Number of setups.
    pub fn setup_count(&self) -> usize {
        self.setups.len()
    }

    /// Access all setups (for setup filtering, etc.).
    pub fn list_setups(&self) -> &[SetupData] {
        &self.setups
    }

    /// Get a toolpath config by index.
    pub fn get_toolpath_config(&self, index: usize) -> Option<&ToolpathConfig> {
        self.toolpath_configs.get(index)
    }

    /// Post-processor configuration.
    pub fn post_config(&self) -> &ProjectPostConfig {
        &self.post
    }

    /// Machine profile.
    pub fn machine(&self) -> &crate::machine::MachineProfile {
        &self.machine
    }

    /// Name of the library machine this project references, if any.
    pub fn machine_ref(&self) -> Option<&str> {
        self.machine_ref.as_deref()
    }

    /// Set (or clear) the library machine reference. Persisted on save;
    /// the referenced library file overrides the inline machine on load.
    pub fn set_machine_ref(&mut self, machine_ref: Option<String>) {
        self.machine_ref = machine_ref;
    }

    /// All loaded tools.
    pub fn tools(&self) -> &[ToolConfig] {
        &self.tools
    }

    /// All loaded models.
    pub fn models(&self) -> &[LoadedModel] {
        &self.models
    }

    /// Per-model `(model_id, bbox)` lookup table for callers that need
    /// to build a `SuggestContext::model_bbox` per toolpath without
    /// re-walking the model list each iteration. Skips models with no
    /// finite bbox (placeholder rows).
    pub fn collect_model_bboxes(&self) -> Vec<(usize, BoundingBox3)> {
        self.models
            .iter()
            .filter_map(|m| m.bbox().map(|b| (m.id, b)))
            .collect()
    }

    /// All toolpath configurations.
    pub fn toolpath_configs(&self) -> &[ToolpathConfig] {
        &self.toolpath_configs
    }

    /// Build the pre-sim [`CutterOpProfile`] for one toolpath config —
    /// the canonical Suggest-rationale invocation shared by the GUI
    /// feeds modal and the MCP `get_suggest_rationale` surface (Phase 4
    /// dedup; both previously assembled this context by hand).
    ///
    /// Assembles the exact project-level [`SuggestContext`] those
    /// callers built: per-toolpath model bbox (matched on
    /// `tc.model_id`), stock context from the stock bbox + padding,
    /// and default policy/scope — then runs
    /// [`CutterOpProfile::for_combo`] against the session's machine,
    /// stock material, workholding, post-config spindle strategy, and
    /// the embedded vendor LUT.
    ///
    /// Returns `None` when the toolpath's tool id doesn't resolve —
    /// callers keep their own error surface for that case.
    ///
    /// [`CutterOpProfile`]: crate::feeds::profile::CutterOpProfile
    /// [`CutterOpProfile::for_combo`]: crate::feeds::profile::CutterOpProfile::for_combo
    /// [`SuggestContext`]: crate::feeds::suggest::SuggestContext
    pub fn cutter_op_profile<'a>(
        &'a self,
        tc: &'a ToolpathConfig,
    ) -> Option<crate::feeds::profile::CutterOpProfile<'a>> {
        use crate::feeds::profile::{CutterOpProfile, CutterOpProfileInput};
        use crate::feeds::suggest::{StockContext, SuggestContext};

        let tool = self.get_tool(ToolId(tc.tool_id))?;
        let stock = self.stock_config();
        let stock_ctx = StockContext::from_stock_bbox(self.stock_bbox(), stock.padding);
        let model_bboxes = self.collect_model_bboxes();
        let model_bbox = model_bboxes
            .iter()
            .find(|(id, _)| *id == tc.model_id)
            .map(|(_, b)| b);
        let context = SuggestContext {
            model_bbox,
            stock: Some(&stock_ctx),
            ..SuggestContext::default()
        };
        Some(CutterOpProfile::for_combo(CutterOpProfileInput {
            operation: &tc.operation,
            tool,
            machine: self.machine(),
            material: &stock.material,
            workholding: stock.workholding_rigidity,
            lut: crate::feeds::embedded_vendor_lut(),
            spindle_strategy: self.post_config().spindle_strategy,
            context,
        }))
    }

    // ── Mutable accessors ─────────────────────────────────────────
    //
    // These provide raw mutable access for immediate-mode UI binding and
    // internal bulk operations (undo, import). **Prefer named mutation
    // methods** in `mutation.rs` for state changes that require cache
    // invalidation. After using `stock_mut()` or `machine_mut()`, call
    // `invalidate_stock()` / `invalidate_machine()` to clear stale caches.

    /// Mutable access to stock configuration.
    ///
    /// Call [`invalidate_stock()`](Self::invalidate_stock) after edits to
    /// clear stale simulation caches.
    pub fn stock_mut(&mut self) -> &mut StockConfig {
        &mut self.stock
    }

    /// Mutable access to machine profile.
    ///
    /// Call [`invalidate_machine()`](Self::invalidate_machine) after edits to
    /// clear stale simulation caches.
    pub fn machine_mut(&mut self) -> &mut crate::machine::MachineProfile {
        &mut self.machine
    }

    /// Mutable access to all tools.
    ///
    /// Prefer [`add_tool()`](Self::add_tool), [`remove_tool()`](Self::remove_tool),
    /// or [`replace_tools()`](Self::replace_tools). Call
    /// [`invalidate_tool()`](Self::invalidate_tool) after in-place edits.
    pub fn tools_mut(&mut self) -> &mut Vec<ToolConfig> {
        &mut self.tools
    }

    /// Mutable access to all loaded models.
    ///
    /// Prefer [`add_model()`](Self::add_model) and
    /// [`remove_model()`](Self::remove_model).
    pub fn models_mut(&mut self) -> &mut Vec<LoadedModel> {
        &mut self.models
    }

    /// Mutable access to post-processor configuration.
    pub fn post_mut(&mut self) -> &mut ProjectPostConfig {
        &mut self.post
    }

    /// Resumable export-wizard settings.
    pub fn wizard(&self) -> &WizardState {
        &self.wizard
    }

    /// Mutable access to the export-wizard settings.
    pub fn wizard_mut(&mut self) -> &mut WizardState {
        &mut self.wizard
    }

    /// Replace the project name.
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    // ── ID lookup helpers ─────────────────────────────────────────

    /// Find a toolpath config by its semantic ID (not vec index).
    pub fn find_toolpath_config_by_id(&self, id: ToolpathId) -> Option<(usize, &ToolpathConfig)> {
        self.toolpath_configs
            .iter()
            .enumerate()
            .find(|(_, tc)| tc.id == id)
    }

    /// Find a mutable toolpath config by its semantic ID.
    pub fn find_toolpath_config_by_id_mut(
        &mut self,
        id: ToolpathId,
    ) -> Option<(usize, &mut ToolpathConfig)> {
        self.toolpath_configs
            .iter_mut()
            .enumerate()
            .find(|(_, tc)| tc.id == id)
    }

    /// Find which setup (by index) owns a toolpath with the given semantic ID.
    pub fn setup_of_toolpath_id(&self, tp_id: ToolpathId) -> Option<usize> {
        let tp_index = self.toolpath_configs.iter().position(|tc| tc.id == tp_id)?;
        self.setups
            .iter()
            .position(|s| s.toolpath_indices.contains(&tp_index))
    }

    /// Find a setup by its semantic ID (not vec index).
    pub fn find_setup_by_id(&self, id: usize) -> Option<(usize, &SetupData)> {
        self.setups.iter().enumerate().find(|(_, s)| s.id == id)
    }

    /// Find a mutable setup by its semantic ID.
    pub fn find_setup_by_id_mut(&mut self, id: usize) -> Option<(usize, &mut SetupData)> {
        self.setups.iter_mut().enumerate().find(|(_, s)| s.id == id)
    }

    /// Collect all toolpath semantic IDs.
    pub fn all_toolpath_ids(&self) -> Vec<ToolpathId> {
        self.toolpath_configs.iter().map(|tc| tc.id).collect()
    }

    /// Mutable access to all toolpath configs.
    ///
    /// Prefer named mutation methods: [`set_toolpath_enabled()`](Self::set_toolpath_enabled),
    /// [`set_face_selection()`](Self::set_face_selection),
    /// [`set_dressup_config()`](Self::set_dressup_config), etc.
    pub fn toolpath_configs_mut(&mut self) -> &mut Vec<ToolpathConfig> {
        &mut self.toolpath_configs
    }

    /// Mutable access to all setups.
    ///
    /// Prefer named mutation methods: [`rename_setup()`](Self::rename_setup),
    /// [`add_fixture()`](Self::add_fixture), [`remove_fixture()`](Self::remove_fixture),
    /// [`add_keep_out()`](Self::add_keep_out), [`remove_keep_out()`](Self::remove_keep_out),
    /// [`move_toolpath_to_setup()`](Self::move_toolpath_to_setup).
    pub fn setups_mut(&mut self) -> &mut Vec<SetupData> {
        &mut self.setups
    }

    // ── Internal helpers ───────────────────────────────────────────

    pub(crate) fn find_tool_by_raw_id(&self, raw_id: usize) -> Option<&ToolConfig> {
        self.tools
            .iter()
            .find(|t| t.id.0 == raw_id)
            .or_else(|| self.tools.first())
    }

    pub(crate) fn find_model_by_raw_id(&self, raw_id: usize) -> Option<&LoadedModel> {
        self.models
            .iter()
            .find(|m| m.id == raw_id)
            .or_else(|| self.models.first())
    }

    pub(crate) fn find_setup_for_toolpath_index(&self, tp_index: usize) -> Option<&SetupData> {
        self.setups
            .iter()
            .find(|s| s.toolpath_indices.contains(&tp_index))
    }

    /// Find the setup that owns a toolpath with the given semantic ID.
    /// Companion to [`Self::find_setup_for_toolpath_index`] for call sites
    /// that only have the [`ToolpathId`], not its vec index.
    pub(crate) fn find_setup_for_toolpath_id(&self, tp_id: ToolpathId) -> Option<&SetupData> {
        let tp_index = self.toolpath_configs.iter().position(|tc| tc.id == tp_id)?;
        self.find_setup_for_toolpath_index(tp_index)
    }

    // ── Geometry transforms for setup-local frame ────────────────

    /// Build a [`SetupTransformInfo`] for this session's stock and the given
    /// orientation. Callers that want to apply the transform themselves should
    /// use the methods on `SetupTransformInfo`.
    pub fn setup_transform_info(
        &self,
        face_up: FaceUp,
        z_rotation: ZRotation,
    ) -> crate::compute::transform::SetupTransformInfo {
        crate::compute::transform::SetupTransformInfo {
            face_up,
            z_rotation,
            stock_x: self.stock.x,
            stock_y: self.stock.y,
            stock_z: self.stock.z,
            stock_origin_x: self.stock.origin_x,
            stock_origin_y: self.stock.origin_y,
            stock_origin_z: self.stock.origin_z,
        }
    }

    /// Inverse transform: from setup-local frame back to global/world coordinates.
    ///
    /// Undoes ZRotation, then FaceUp, then translates back to world coords.
    pub fn inverse_transform_point_from_setup(
        &self,
        p: P3,
        face_up: FaceUp,
        z_rotation: ZRotation,
    ) -> P3 {
        // 1. Undo ZRotation
        let (eff_w, eff_d, _) = face_up.effective_stock(self.stock.x, self.stock.y, self.stock.z);
        let unrotated = z_rotation.inverse_transform_point(p, eff_w, eff_d);
        // 2. Undo FaceUp flip -> stock-relative
        let rel =
            face_up.inverse_transform_point(unrotated, self.stock.x, self.stock.y, self.stock.z);
        // 3. Translate stock-relative -> world
        P3::new(
            rel.x + self.stock.origin_x,
            rel.y + self.stock.origin_y,
            rel.z + self.stock.origin_z,
        )
    }

    /// Transform a triangle mesh from global to setup-local coordinates.
    pub(crate) fn transform_mesh_to_setup(
        &self,
        mesh: &TriangleMesh,
        face_up: FaceUp,
        z_rotation: ZRotation,
    ) -> TriangleMesh {
        self.setup_transform_info(face_up, z_rotation)
            .apply_to_mesh(mesh)
    }

    /// Transform 2D polygons from global to setup-local XY coordinates.
    pub(crate) fn transform_polygons_to_setup(
        &self,
        polygons: &[Polygon2],
        face_up: FaceUp,
        z_rotation: ZRotation,
    ) -> Vec<Polygon2> {
        self.setup_transform_info(face_up, z_rotation)
            .apply_to_polygons(polygons)
    }
}

// ── Serde for ProjectDiagnostics (for JSON export) ─────────────────────

impl serde::Serialize for ToolpathDiagnostic {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("ToolpathDiagnostic", 10)?;
        s.serialize_field("toolpath_id", &self.toolpath_id)?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("operation_type", &self.operation_type)?;
        s.serialize_field("op_kind", &self.op_kind)?;
        s.serialize_field("tool_name", &self.tool_name)?;
        s.serialize_field("move_count", &self.move_count)?;
        s.serialize_field("cutting_distance_mm", &self.cutting_distance_mm)?;
        s.serialize_field("rapid_distance_mm", &self.rapid_distance_mm)?;
        s.serialize_field("collision_count", &self.collision_count)?;
        s.serialize_field("rapid_collision_count", &self.rapid_collision_count)?;
        s.end()
    }
}

impl serde::Serialize for VerdictSeverity {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = match self {
            Self::Critical => "critical",
            Self::Important => "important",
            Self::Polish => "polish",
        };
        serializer.serialize_str(s)
    }
}

impl serde::Serialize for VerdictKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl serde::Serialize for VerdictEvidence {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("VerdictEvidence", 3)?;
        s.serialize_field("move_index", &self.move_index)?;
        s.serialize_field("z_value", &self.z_value)?;
        s.serialize_field("count", &self.count)?;
        s.end()
    }
}

impl serde::Serialize for Verdict {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Verdict", 6)?;
        s.serialize_field("severity", &self.severity)?;
        s.serialize_field("kind", &self.kind)?;
        s.serialize_field("headline", &self.headline)?;
        s.serialize_field("offender_toolpath_ids", &self.offender_toolpath_ids)?;
        s.serialize_field("fix_hint", &self.fix_hint)?;
        s.serialize_field("evidence", &self.evidence)?;
        s.end()
    }
}

impl serde::Serialize for ProjectDiagnostics {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("ProjectDiagnostics", 8)?;
        s.serialize_field("total_runtime_s", &self.total_runtime_s)?;
        s.serialize_field("air_cut_percentage", &self.air_cut_percentage)?;
        s.serialize_field("average_engagement", &self.average_engagement)?;
        s.serialize_field("collision_count", &self.collision_count)?;
        s.serialize_field("rapid_collision_count", &self.rapid_collision_count)?;
        s.serialize_field("per_toolpath", &self.per_toolpath)?;
        s.serialize_field("verdict", &self.verdict)?;
        s.serialize_field("verdicts", &self.verdicts)?;
        s.end()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::project_file::parse_tool_type;
    use super::*;

    fn named_empty_job() -> ProjectJobSection {
        ProjectJobSection {
            name: "Test Job".to_owned(),
            ..ProjectJobSection::default()
        }
    }

    #[test]
    fn empty_project_loads() {
        let project = ProjectFile {
            format_version: 3,
            job: named_empty_job(),
            tools: Vec::new(),
            models: Vec::new(),
            setups: Vec::new(),
            toolpaths: Vec::new(),
        };
        let session = ProjectSession::from_project_file(project, Path::new(".")).unwrap();
        assert_eq!(session.name(), "Test Job");
        assert_eq!(session.toolpath_count(), 0);
        assert_eq!(session.setup_count(), 0);
        assert!(session.list_toolpaths().is_empty());
        assert!(session.list_tools().is_empty());
    }

    #[test]
    fn stock_bbox_from_defaults() {
        let project = ProjectFile {
            format_version: 3,
            job: named_empty_job(),
            tools: Vec::new(),
            models: Vec::new(),
            setups: Vec::new(),
            toolpaths: Vec::new(),
        };
        let session = ProjectSession::from_project_file(project, Path::new(".")).unwrap();
        let bbox = session.stock_bbox();
        assert!((bbox.max.x - bbox.min.x - 100.0).abs() < 1e-6);
        assert!((bbox.max.y - bbox.min.y - 100.0).abs() < 1e-6);
        assert!((bbox.max.z - bbox.min.z - 25.0).abs() < 1e-6);
    }

    #[test]
    fn diagnostics_empty_session() {
        let project = ProjectFile {
            format_version: 3,
            job: named_empty_job(),
            tools: Vec::new(),
            models: Vec::new(),
            setups: Vec::new(),
            toolpaths: Vec::new(),
        };
        let session = ProjectSession::from_project_file(project, Path::new(".")).unwrap();
        let diag = session.diagnostics();
        assert_eq!(diag.verdict, "OK");
        assert!(diag.per_toolpath.is_empty());
    }

    #[test]
    fn toolpath_missing_operation_falls_back_to_default() {
        use crate::compute::catalog::{OperationConfig, OperationType};

        let project = ProjectFile {
            format_version: 3,
            job: ProjectJobSection::default(),
            tools: Vec::new(),
            models: Vec::new(),
            setups: vec![ProjectSetupSection {
                id: Some(0),
                name: "Setup 1".to_owned(),
                face_up: "top".to_owned(),
                z_rotation: String::new(),
                pause_message: None,
                fixtures: Vec::new(),
                keep_out_zones: Vec::new(),
                toolpaths: vec![ProjectToolpathSection {
                    id: Some(ToolpathId(0)),
                    name: "Bare".to_owned(),
                    op_type: Some(OperationType::Profile),
                    operation: None,
                    enabled: true,
                    tool_id: None,
                    model_id: None,
                    dressups: crate::compute::config::DressupConfig::default(),
                    heights: crate::compute::config::HeightsConfig::default(),
                    pre_gcode: None,
                    post_gcode: None,
                    boundary: crate::compute::config::BoundaryConfig::default(),
                    boundary_inherit: true,
                    stock_source: crate::compute::config::StockSource::default(),
                    coolant: crate::gcode::CoolantMode::default(),
                    face_selection: None,
                    _legacy_feeds_auto: None,
                    debug_options: crate::debug_trace::ToolpathDebugOptions::default(),
                    feeds_provenance: crate::feeds::FeedsProvenance::default(),
                }],
            }],
            toolpaths: Vec::new(),
        };

        let session = ProjectSession::from_project_file(project, Path::new(".")).unwrap();
        assert_eq!(session.setup_count(), 1);
        assert_eq!(session.toolpath_count(), 1);
        let tp = &session.toolpath_configs[0];
        assert_eq!(tp.operation.op_type(), OperationType::Profile);
        let expected = OperationConfig::new_default(OperationType::Profile);
        assert_eq!(tp.operation.op_type(), expected.op_type());
    }

    /// Snapshot model: the `machine_ref` field is retained on the file
    /// struct only so legacy projects still PARSE — it is no longer a
    /// live link (the session loader drops it; see
    /// `legacy_machine_ref_dropped_on_session_load`).
    #[test]
    fn legacy_machine_ref_field_still_parses_for_backcompat() {
        use super::project_file::{ProjectFile, ProjectJobSection};
        let make = |job: ProjectJobSection| ProjectFile {
            format_version: 3,
            job,
            tools: Vec::new(),
            models: Vec::new(),
            setups: Vec::new(),
            toolpaths: Vec::new(),
        };
        // An old file with machine_ref must still deserialize (we read the
        // field, then drop it on load).
        let project = make(ProjectJobSection {
            name: "Ref Job".to_owned(),
            machine_ref: Some("shapeoko_pro_xxl".to_owned()),
            ..ProjectJobSection::default()
        });
        let toml_str = toml::to_string_pretty(&project).unwrap();
        let back: ProjectFile = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.job.machine_ref.as_deref(), Some("shapeoko_pro_xxl"));

        // No machine_ref → key omitted (skip_serializing_if), so files
        // written under the snapshot model never carry it.
        let plain = make(ProjectJobSection::default());
        let plain_toml = toml::to_string_pretty(&plain).unwrap();
        assert!(
            !plain_toml.contains("machine_ref"),
            "absent machine_ref should be omitted: {plain_toml}"
        );
    }

    /// Snapshot migration: loading a project that carries a legacy
    /// `machine_ref` drops the ref (session reports `None`) and keeps the
    /// inline `[job.machine]` as authoritative.
    #[test]
    fn legacy_machine_ref_dropped_on_session_load() {
        use super::project_file::{ProjectFile, ProjectJobSection};
        let mut inline = crate::machine::MachineProfile::generic_wood_router();
        inline.name = "Inline Wins".to_owned();
        let project = ProjectFile {
            format_version: 3,
            job: ProjectJobSection {
                name: "Legacy Ref".to_owned(),
                machine: inline,
                machine_ref: Some("some_library_machine".to_owned()),
                ..ProjectJobSection::default()
            },
            tools: Vec::new(),
            models: Vec::new(),
            setups: Vec::new(),
            toolpaths: Vec::new(),
        };
        let toml_str = toml::to_string_pretty(&project).unwrap();
        let dir = std::env::temp_dir().join(format!("rscam_snap_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("legacy_ref.toml");
        std::fs::write(&path, toml_str).unwrap();

        let session = ProjectSession::load(&path).unwrap();
        assert_eq!(
            session.machine_ref(),
            None,
            "legacy machine_ref must be dropped on load (snapshot model)"
        );
        assert_eq!(
            session.machine().name,
            "Inline Wins",
            "inline machine must remain authoritative"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_toml_without_cam_content() {
        let toml_str = "format_version = 3\n";
        let project: ProjectFile = toml::from_str(toml_str).unwrap();
        let result = ProjectSession::from_project_file(project, Path::new("."));
        match result {
            Err(SessionError::NotACamProject(detail)) => {
                assert!(
                    detail.contains("does not look like an rs_cam project"),
                    "unexpected detail: {detail}"
                );
            }
            Err(other) => panic!("expected NotACamProject, got {other:?}"),
            Ok(_) => panic!("loader should reject empty TOML"),
        }
    }

    /// Q4 tripwire, deliberately re-baselined in T8: the loader still
    /// defaults unknown tokens to EndMill (now with a tracing warning
    /// instead of silently), and the unified vocabulary additionally
    /// accepts the former viz-legacy aliases — pre-T8 `"ball"` parsed
    /// to EndMill here but BallNose in the viz legacy loader.
    #[test]
    fn tool_type_parsing() {
        assert!(matches!(parse_tool_type("end_mill"), ToolType::EndMill));
        assert!(matches!(parse_tool_type("ball_nose"), ToolType::BallNose));
        assert!(matches!(parse_tool_type("bull_nose"), ToolType::BullNose));
        assert!(matches!(parse_tool_type("v_bit"), ToolType::VBit));
        assert!(matches!(
            parse_tool_type("tapered_ball_nose"),
            ToolType::TaperedBallNose
        ));
        assert!(matches!(parse_tool_type("unknown"), ToolType::EndMill));
        // T8 unified-vocabulary additions (were EndMill via wildcard).
        assert!(matches!(parse_tool_type("ball"), ToolType::BallNose));
        assert!(matches!(
            parse_tool_type("tapered_ball"),
            ToolType::TaperedBallNose
        ));
    }

    /// Create a session with one tool and one Pocket toolpath for mutation tests.
    fn session_with_toolpath() -> ProjectSession {
        use crate::compute::catalog::OperationConfig;
        let project = ProjectFile {
            format_version: 3,
            job: ProjectJobSection::default(),
            tools: vec![ProjectToolSection {
                id: Some(0),
                name: "Test EndMill".to_owned(),
                tool_type: "end_mill".to_owned(),
                diameter: 6.35,
                cutting_length: 25.0,
                helix_deg: 30.0,
                corner_radius_mm: 0.0,
                corner_radius: 2.0,
                included_angle: 90.0,
                taper_half_angle: 15.0,
                shaft_diameter: 6.35,
                holder_diameter: 25.0,
                shank_diameter: 6.35,
                shank_length: 20.0,
                stickout: 45.0,
                flute_count: 2,
                tool_number: None,
                tool_material: "carbide".to_owned(),
                cut_direction: "up_cut".to_owned(),
                vendor: String::new(),
                product_id: String::new(),
            }],
            models: Vec::new(),
            setups: vec![ProjectSetupSection {
                id: Some(0),
                name: "Setup 1".to_owned(),
                face_up: "top".to_owned(),
                z_rotation: String::new(),
                pause_message: None,
                fixtures: Vec::new(),
                keep_out_zones: Vec::new(),
                toolpaths: vec![ProjectToolpathSection {
                    id: Some(ToolpathId(0)),
                    name: "Test Pocket".to_owned(),
                    op_type: Some(crate::compute::catalog::OperationType::Pocket),
                    operation: Some(OperationConfig::new_default(
                        crate::compute::catalog::OperationType::Pocket,
                    )),
                    enabled: true,
                    tool_id: Some(0),
                    model_id: Some(0),
                    dressups: crate::compute::config::DressupConfig::default(),
                    heights: crate::compute::config::HeightsConfig::default(),
                    pre_gcode: None,
                    post_gcode: None,
                    boundary: crate::compute::config::BoundaryConfig::default(),
                    boundary_inherit: true,
                    stock_source: crate::compute::config::StockSource::default(),
                    coolant: crate::gcode::CoolantMode::default(),
                    face_selection: None,
                    _legacy_feeds_auto: None,
                    debug_options: crate::debug_trace::ToolpathDebugOptions::default(),
                    feeds_provenance: crate::feeds::FeedsProvenance::default(),
                }],
            }],
            toolpaths: Vec::new(),
        };
        ProjectSession::from_project_file(project, Path::new(".")).unwrap()
    }

    #[test]
    fn set_common_param_feed_rate() {
        let mut session = session_with_toolpath();
        let original = session.toolpath_configs[0].operation.feed_rate();

        session
            .set_toolpath_param(0, "feed_rate", serde_json::json!(1500.0))
            .unwrap();

        let updated = session.toolpath_configs[0].operation.feed_rate();
        assert!(
            (updated - 1500.0).abs() < 1e-6,
            "feed_rate should be 1500.0, got {updated} (was {original})"
        );
    }

    #[test]
    fn set_config_specific_param() {
        let mut session = session_with_toolpath();

        // Pocket has a config-specific "angle" parameter
        session
            .set_toolpath_param(0, "angle", serde_json::json!(45.0))
            .unwrap();

        // Verify it changed via serde round-trip
        let json = serde_json::to_value(&session.toolpath_configs[0].operation).unwrap();
        let angle = json["params"]["angle"].as_f64().unwrap();
        assert!(
            (angle - 45.0).abs() < 1e-6,
            "angle should be 45.0, got {angle}"
        );
    }

    #[test]
    fn invalid_param_name_returns_error() {
        let mut session = session_with_toolpath();

        let result =
            session.set_toolpath_param(0, "nonexistent_param_xyz", serde_json::json!(42.0));

        assert!(result.is_err(), "Should fail for unknown param name");
        assert!(
            matches!(result.unwrap_err(), SessionError::InvalidParam(_)),
            "Should be InvalidParam error"
        );
    }

    #[test]
    fn set_tool_param_diameter() {
        let mut session = session_with_toolpath();

        session
            .set_tool_param(0, "diameter", &serde_json::json!(10.0))
            .unwrap();

        let updated = session.tools[0].diameter;
        assert!(
            (updated - 10.0).abs() < 1e-6,
            "diameter should be 10.0, got {updated}"
        );
    }

    #[test]
    fn set_toolpath_param_invalidates_cached_result() {
        let mut session = session_with_toolpath();

        // Manually insert a fake cached result
        session.results.insert(
            0,
            ToolpathComputeResult {
                op_data: crate::drill_op::OpData::Toolpath(std::sync::Arc::new(
                    crate::toolpath_spans::AnnotatedToolpath::new(crate::toolpath::Toolpath::new()),
                )),
                stats: crate::compute::config::ToolpathStats::default(),
                debug_trace: None,
                semantic_trace: None,
            },
        );
        assert!(
            session.results.contains_key(&0),
            "Precondition: result cached"
        );

        session
            .set_toolpath_param(0, "feed_rate", serde_json::json!(2000.0))
            .unwrap();

        assert!(
            !session.results.contains_key(&0),
            "Cached result should be invalidated after set_toolpath_param"
        );
    }

    // ── CRUD mutation tests ───────────────────────────────────────

    #[test]
    fn add_toolpath_then_list() {
        use crate::compute::catalog::{OperationConfig, OperationType};

        let mut session = session_with_toolpath();
        assert_eq!(session.toolpath_count(), 1);

        let new_tp = ToolpathConfig {
            id: ToolpathId(0), // will be overwritten by add_toolpath
            name: "New Profile".to_owned(),
            enabled: true,
            operation: OperationConfig::new_default(OperationType::Profile),
            dressups: crate::compute::config::DressupConfig::default(),
            heights: crate::compute::config::HeightsConfig::default(),
            tool_id: 0,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: crate::compute::config::BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::compute::config::StockSource::default(),
            coolant: crate::gcode::CoolantMode::default(),
            face_selection: None,
            debug_options: crate::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance: crate::feeds::FeedsProvenance::default(),
        };

        let idx = session.add_toolpath(0, new_tp).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(session.toolpath_count(), 2);

        let summaries = session.list_toolpaths();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[1].name, "New Profile");
    }

    #[test]
    fn remove_toolpath_then_list() {
        let mut session = session_with_toolpath();
        assert_eq!(session.toolpath_count(), 1);

        session.remove_toolpath(0).unwrap();
        assert_eq!(session.toolpath_count(), 0);
        assert!(session.list_toolpaths().is_empty());

        // Setup should have no more toolpath indices
        assert!(session.setups[0].toolpath_indices.is_empty());
    }

    #[test]
    fn save_reload_roundtrip() {
        let session = session_with_toolpath();
        let dir = std::env::temp_dir().join("rs_cam_test_roundtrip");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("roundtrip_test.toml");

        // Save
        session.save(&path).unwrap();

        // Reload
        let content = std::fs::read_to_string(&path).unwrap();
        let reloaded_project: ProjectFile = toml::from_str(&content).unwrap();
        let reloaded = ProjectSession::from_project_file(reloaded_project, Path::new(".")).unwrap();

        // Verify key state matches
        assert_eq!(reloaded.name(), session.name());
        assert_eq!(reloaded.toolpath_count(), session.toolpath_count());
        assert_eq!(reloaded.setup_count(), session.setup_count());
        assert_eq!(reloaded.list_tools().len(), session.list_tools().len());

        // Stock dimensions
        let orig_bbox = session.stock_bbox();
        let reload_bbox = reloaded.stock_bbox();
        assert!((orig_bbox.max.x - reload_bbox.max.x).abs() < 1e-6);
        assert!((orig_bbox.max.y - reload_bbox.max.y).abs() < 1e-6);
        assert!((orig_bbox.max.z - reload_bbox.max.z).abs() < 1e-6);

        // Toolpath name preserved
        let orig_tps = session.list_toolpaths();
        let reload_tps = reloaded.list_toolpaths();
        assert_eq!(orig_tps[0].name, reload_tps[0].name);
        assert_eq!(orig_tps[0].enabled, reload_tps[0].enabled);

        // Cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn set_stock_invalidates_simulation() {
        use crate::compute::simulate::SimulationResult;
        use crate::stock_mesh::StockMesh;

        let mut session = session_with_toolpath();

        // Manually set a fake simulation result
        session.simulation = Some(SimulationResult {
            mesh: StockMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                colors: Vec::new(),
            },
            total_moves: 0,
            deviations: None,
            boundaries: Vec::new(),
            checkpoints: Vec::new(),
            rapid_collisions: Vec::new(),
            rapid_collision_move_indices: Vec::new(),
            cut_trace: None,
            resolution_clamped: false,
            prior_stocks: std::collections::HashMap::new(),
        });
        assert!(
            session.simulation_result().is_some(),
            "Precondition: simulation present"
        );

        let new_stock = StockConfig {
            x: 200.0,
            y: 200.0,
            z: 50.0,
            ..StockConfig::default()
        };
        session.set_stock_config(new_stock);

        assert!(
            session.simulation_result().is_none(),
            "Simulation should be invalidated after set_stock_config"
        );
        assert!((session.stock_config().x - 200.0).abs() < 1e-6);
    }
}
