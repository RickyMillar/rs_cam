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

mod builder;
mod command;
mod compute;
mod cycle_time;
pub mod dependencies;
mod diagnostics_types;
mod eval_context;
pub mod generation_plan;
pub mod multitool;
mod mutation;
pub mod project_file;
mod reach;
mod save;

pub use builder::ProjectSessionBuilder;
pub use command::{
    AddAlignmentPinArgs, AddFixtureArgs, AddKeepOutArgs, AddModelArgs, AddSetupArgs, AddToolArgs,
    AddToolpathArgs, AdoptModelGeometryArgs, AdoptResultArgs, AdoptSimulationArgs,
    AutoEnableRestAnalysisArgs, Command, CommandId, CommandKind, Effects, ForgetResultArgs,
    GenerateToolpathArgs, GetOperationSchemaAnswer, GetOperationSchemaArgs,
    ImportMachineSettingsArgs, InvalidateMachineArgs, InvalidateModelArgs, InvalidateStockArgs,
    InvalidateToolArgs, InvalidateToolpathInputsArgs, Job, JobAnswer, JobHandle,
    MoveToolpathToSetupArgs, OptimizeToolpathArgs, PreviewTierMapArgs, Query, QueryAnswer, Reach,
    RecommendClearingStrategyArgs, RemoveAlignmentPinArgs, RemoveFixtureArgs, RemoveKeepOutArgs,
    RemoveModelArgs, RemoveSetupArgs, RemoveToolArgs, RemoveToolpathArgs, ReorderToolpathArgs,
    ReplaceFixtureArgs, ReplaceKeepOutArgs, ReplaceToolArgs, ReplaceToolpathConfigArgs,
    ReplaceToolsArgs, RestoreToolpathSnapshotArgs, SaveProjectArgs, SetAlignmentPinDrillHolesArgs,
    SetBoundaryConfigArgs, SetDressupConfigArgs, SetDressupFieldArgs, SetDrillSelectedHolesArgs,
    SetFaceSelectionArgs, SetFeedsProvenanceArgs, SetMachineArgs, SetMachineKinematicsArgs,
    SetPostConfigArgs, SetRestAnalysisConfigArgs, SetSetupDatumArgs, SetSetupFaceArgs,
    SetSetupModelsArgs, SetSetupNameArgs, SetSetupPauseMessageArgs, SetSetupRotationArgs,
    SetStockConfigArgs, SetStockSourceArgs, SetToolParamArgs, SetToolpathDebugOptionsArgs,
    SetToolpathEnabledArgs, SetToolpathHeightsArgs, SetToolpathModelArgs, SetToolpathOperationArgs,
    SetToolpathParamArgs, SetToolpathToolArgs, Surfaces, ToolpathCycleTimeAnswer,
    ToolpathCycleTimeArgs, UpdateStockFromBboxArgs,
};
pub use compute::{
    GenContext, GenObserver, GenerateToolpathHandle, OptimizeToolpathHandle,
    RecommendClearingStrategyHandle, ResolvedGenInputs, execute_generation, execute_job,
    execute_optimize_toolpath, execute_recommend_clearing_strategy,
};
pub use cycle_time::{CycleTime, CycleTimeBasis, toolpath_cycle_time};
// SES-03: the diagnostic result types live beside no logic of their own,
// so they sit in their own module and reach every caller through this
// one re-export. The path `crate::session::ProjectDiagnostics` does not
// move.
pub use dependencies::{Edge, EdgeKind, EdgeState};
pub use diagnostics_types::{
    ProjectDiagnostics, ProjectEvidence, ToolpathDiagnostic, Verdict, VerdictEvidence, VerdictKind,
    VerdictSeverity,
};
pub use eval_context::SetupEvalContext;
pub use multitool::{
    MultitoolPlanOutcome, MultitoolPlanSpec, MultitoolPreview, PreviewTierMapHandle, TierStrategy,
    equal_cusp_stepover_mm, execute_preview_tier_map,
};

pub use mutation::polygons_bbox;

// Re-export all public project_file types so external crates see no path change.
pub use project_file::{
    ProjectFile, ProjectFixtureSection, ProjectJobSection, ProjectKeepOutSection,
    ProjectLoadWarning, ProjectModelSection, ProjectSetupSection, ProjectStockConfig,
    ProjectToolSection, ProjectToolpathSection, SUPPORTED_FORMAT_VERSION,
};

use crate::ids::ToolpathId;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{
    BoundaryConfig, BoundarySource, DressupConfig, HeightsConfig, StockSource,
};
use crate::compute::simulate::SimulationResult;
use crate::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::toolpath_stats::ToolpathStats;
use crate::compute::transform::{FaceUp, ZRotation};
use crate::gcode::CoolantMode;
use crate::geo::{BoundingBox3, P3};
use crate::geometry::enriched_mesh::{EnrichedMesh, FaceGroupId};
use crate::ids::{FixtureId, KeepOutId, ModelId};
use crate::io::dxf_input::DrillTarget;
use crate::mesh::TriangleMesh;
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;
use crate::trace::debug_trace::{ToolpathDebugOptions, ToolpathDebugTrace};
use crate::trace::semantic_trace::ToolpathSemanticTrace;

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
    /// The project file declares a format this build does not read.
    ///
    /// rs_cam writes `format_version = 3`. It reads that one shape. No
    /// converter exists for an older shape, and none is planned. A second
    /// reader is how the two loaders in I01 drifted apart. An older file
    /// therefore fails to open, and the message names its version.
    ///
    /// A file with no `format_version` key reads as version 1. The loader
    /// refuses that file too.
    UnsupportedFormatVersion { found: u32 },
    /// The setup asks for something this build does not support, and the
    /// honest answer is a refusal rather than a plausible-looking result.
    ///
    /// Introduced 2026-08-22 for the two lateral-setup preconditions (see
    /// [`ProjectSession::check_lateral_setup_support`]). Distinct from
    /// [`Self::MissingGeometry`] on purpose: "you did not import a mesh" is
    /// a thing the operator can fix by importing one, whereas these say the
    /// combination itself has no defined meaning.
    UnsupportedSetup(String),
    /// G-ENTRYEMPTY — the generator emitted no cutting motion at all from a
    /// non-empty input region.
    ///
    /// A **genuine** failure, deliberately distinct from every other variant
    /// here so no consumer has to string-match for it:
    ///
    /// * not [`Self::MissingGeometry`] — the geometry was present and was
    ///   handed to the generator; what came back was nothing.
    /// * not [`Self::OperationFailed`] — the generator itself returned `Ok`.
    ///   The failure is that its output cannot be machined, and it was
    ///   invisible until this variant existed.
    /// * not [`crate::compute::config::AwaitingPriorStock`], which is a
    ///   *sequencing* state `generate_all`'s fixpoint loop retries. This one
    ///   is terminal: re-running the same configuration produces the same
    ///   nothing.
    ///
    /// Built by [`crate::compute::generated_empty::classify`], which also
    /// owns every "an empty result is legitimate here" exemption — see that
    /// module's doc.
    GeneratedEmpty(String),
    /// WP14b — the job needs a baseline cut trace and the session holds
    /// none.
    ///
    /// Distinct from [`Self::MissingGeometry`] on purpose: the geometry
    /// is present, and what is absent is a MEASUREMENT of it. The
    /// operator's repair is a simulation run, not an import.
    ///
    /// The payload names what needs the trace, so the sentence reads as
    /// one instruction. Raised by
    /// [`ProjectSession::start`] for the `optimize_toolpath` row, which
    /// reads the trace off the session (§28 ruling 5). A session
    /// mutation clears that slot, so an optimize issued after an edit
    /// refuses here rather than scoring against a measurement the edit
    /// already invalidated.
    SimulationRequired(String),
    /// WP3 — a completion answers a superseded parameter set.
    ///
    /// A generation runs off the frame loop. The lane stamps the
    /// generation-input revision it starts from, and
    /// [`Command::AdoptResult`] carries that stamp back. When the
    /// toolpath's revision has moved since, the computed geometry
    /// answers inputs the project no longer holds, so the door refuses
    /// it and inserts nothing.
    ///
    /// `submitted` is the revision the lane started from. `current` is
    /// the revision the toolpath carries now.
    StaleCompletion {
        index: usize,
        submitted: u64,
        current: u64,
    },
    /// D7 (W0c): a simulation answers a project state the session has
    /// left.
    ///
    /// A simulation runs off the frame loop. The submit door stamps the
    /// [`ProjectSession::simulation_epoch`] it reads, and
    /// [`Command::AdoptSimulation`] carries that stamp back. Every edit
    /// that clears the simulation bumps the epoch, so an unequal epoch
    /// means the run describes material the project no longer cuts. The
    /// door refuses it and stores nothing.
    ///
    /// This is a safety matter, not only a display one:
    /// `StockSource::FromRemainingStock` reads its prior stock from that
    /// field, so a stored late run hands a rest operation a superseded
    /// snapshot.
    ///
    /// `submitted` is the epoch the lane started from. `current` is the
    /// epoch the session carries now.
    StaleSimulation { submitted: u64, current: u64 },
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
            Self::MissingGeometry(msg) => write!(f, "Missing geometry: {msg}"),
            Self::OperationFailed(msg) => write!(f, "Operation failed: {msg}"),
            Self::Simulation(e) => write!(f, "Simulation error: {e}"),
            Self::CollisionCheck(e) => write!(f, "Collision check error: {e}"),
            Self::Export(msg) => write!(f, "Export error: {msg}"),
            Self::InvalidParam(msg) => write!(f, "Invalid parameter: {msg}"),
            Self::NotACamProject(detail) => write!(f, "Not an rs_cam project: {detail}"),
            Self::UnsupportedFormatVersion { found } => write!(
                f,
                "Project format version {found} is not supported. rs_cam reads format_version 3."
            ),
            Self::UnsupportedSetup(msg) => write!(f, "Unsupported setup: {msg}"),
            // No prefix: the message is already a full operator-facing
            // sentence naming the toolpath and the operation, and a
            // "Generated empty: " prefix would only push the toolpath name
            // further from the start of a truncated GUI badge.
            Self::GeneratedEmpty(msg) => write!(f, "{msg}"),
            Self::SimulationRequired(what) => {
                write!(f, "Run a simulation first — {what}")
            }
            Self::StaleCompletion {
                index,
                submitted,
                current,
            } => write!(
                f,
                "Toolpath {index} changed while it was generating: the \
                 result answers revision {submitted} and the toolpath is \
                 at revision {current}. Generate it again."
            ),
            Self::StaleSimulation { submitted, current } => write!(
                f,
                "The project changed while it was simulating: the run \
                 answers simulation epoch {submitted} and the project is \
                 at epoch {current}. Simulate it again."
            ),
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
///
/// The record derives `Debug` and `Clone` because
/// [`Command::AddModel`] carries one and the generated `Command` enum
/// derives both. A clone copies the `Arc` on each geometry field, not
/// the geometry behind it.
#[derive(Debug, Clone)]
pub struct LoadedModel {
    pub id: usize,
    pub name: String,
    pub mesh: Option<Arc<TriangleMesh>>,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
    /// Pickable drill targets extracted from the source (DXF POINT entities
    /// and circle/arc centres; circle-like closed shapes in an SVG, see
    /// `svg_input::circle_like_drill_targets`). Empty for meshes.
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
        let resolved_kind = kind.or_else(|| crate::io::infer_kind_from_path(path));
        let geometry = project_file::load_model_geometry(&section, base_dir)?;
        // C13: one builder. This arm used to spell the three geometry
        // cases out again, and it wrote `winding_report: None` for a mesh.
        Ok(crate::io::model_from_geometry(
            geometry,
            id,
            name.to_owned(),
            path,
            resolved_kind,
            units,
        ))
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

    /// Adopt the geometry a fresh import produced, keeping this record's
    /// identity.
    ///
    /// **KEPT**: `id`, `name`, `units`. Every `ToolpathConfig::model_id`
    /// names the id, so the operations built on this model survive the
    /// refresh. The name is the operator's label, not the file's. The
    /// declared units describe the operator's source rather than the bytes
    /// on disk — and `io::load_model_file` reports `Millimeters` for a STEP
    /// file whatever the record declares, so a door that CHANGES the units
    /// sets them itself, after this call.
    ///
    /// **REPLACED**: every field the file content determines — `mesh`,
    /// `polygons`, `enriched_mesh`, `drill_targets`, `layers`,
    /// `winding_report`, `load_error` — plus `path` and `kind`, which name
    /// the file the geometry came from. A reload and a rescale re-import the
    /// path and kind they were given, so those two do not move there; a
    /// relink points the record at a different file, and they do.
    ///
    /// G-RELOADTARGETS (F4.4). Three GUI doors refresh a model record in
    /// place — rescale, reload and relink — and each hand-copied its own
    /// subset of these fields. Two of the three left `drill_targets` and
    /// `layers` behind, so the previous import's targets survived beside the
    /// new polygons. A drill operation reads the record at generation time,
    /// so a reloaded drawing drilled the previous version's holes.
    ///
    /// The destructuring is the guard. Add a field to `LoadedModel` and this
    /// function stops compiling until someone decides which side of the
    /// split it belongs on. Three hand-written copies could not offer that.
    pub fn adopt_geometry(&mut self, fresh: Self) {
        let Self {
            mesh,
            polygons,
            drill_targets,
            layers,
            path,
            kind,
            enriched_mesh,
            winding_report,
            load_error,
            id: _,
            name: _,
            units: _,
        } = fresh;
        self.mesh = mesh;
        self.polygons = polygons;
        self.drill_targets = drill_targets;
        self.layers = layers;
        self.path = path;
        self.kind = kind;
        self.enriched_mesh = enriched_mesh;
        self.winding_report = winding_report;
        self.load_error = load_error;
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
///
/// The record derives `PartialEq` because the GUI fixture panel edits a
/// CLONE of it and must not apply a command for an edit that moved
/// nothing (WP6).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    #[serde(default = "default_fixture_size_x")]
    pub size_x: f64,
    #[serde(default = "default_fixture_size_y")]
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
/// A clamp is wider than it is deep, so the two spans differ.
///
/// C02: one `default_fixture_size` of 30.0 used to serve both spans. The
/// GUI creates a clamp 30 by 15, and `ProjectFixtureSection` defaults to
/// 30 by 15 as well, so a file that omitted `size_y` restored a fixture
/// the GUI would never have drawn.
fn default_fixture_size_x() -> f64 {
    30.0
}
fn default_fixture_size_y() -> f64 {
    15.0
}
fn default_fixture_height() -> f64 {
    20.0
}
fn default_fixture_clearance() -> f64 {
    3.0
}

impl Fixture {
    /// A new clamp, at the stock origin, with the default spans.
    ///
    /// C02: the GUI used to build this record from six hard-coded numbers
    /// in `controller/events/model.rs`. The numbers are the serde
    /// defaults, so they belong beside them.
    pub fn new_default(id: FixtureId) -> Self {
        Self {
            name: format!("Fixture {}", id.0 + 1),
            id,
            kind: FixtureKind::Clamp,
            enabled: default_true(),
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            size_x: default_fixture_size_x(),
            size_y: default_fixture_size_y(),
            size_z: default_fixture_height(),
            clearance: default_fixture_clearance(),
        }
    }

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
///
/// The record derives `PartialEq` for the reason [`Fixture`] does.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

// ── Setup datum (W9 / P-2) ─────────────────────────────────────────────
//
// The datum is how the operator ties the model's origin to the physical
// machine before pressing start. It lived only in the GUI overlay
// (`rs_cam_viz`'s `SetupRuntime`) and had no home on the wire, so every
// save dropped it: set "Z Datum = Machine Table", reload, and the panel
// read "Stock Top" again with nothing in the file to say otherwise. It
// is operator-set and safety-relevant, so it belongs to the project, not
// to a window. `to_key`/`from_key` follow `FixtureKind`'s convention and
// use the same spellings the viz fallback schema already wrote.

/// Which corner of the stock the operator probes for the XY datum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Corner {
    #[default]
    FrontLeft,
    FrontRight,
    BackLeft,
    BackRight,
}

impl Corner {
    pub const ALL: &[Corner] = &[
        Corner::FrontLeft,
        Corner::FrontRight,
        Corner::BackLeft,
        Corner::BackRight,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Corner::FrontLeft => "Front-Left",
            Corner::FrontRight => "Front-Right",
            Corner::BackLeft => "Back-Left",
            Corner::BackRight => "Back-Right",
        }
    }

    pub fn to_key(&self) -> &'static str {
        match self {
            Corner::FrontLeft => "fl",
            Corner::FrontRight => "fr",
            Corner::BackLeft => "bl",
            Corner::BackRight => "br",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "fr" => Corner::FrontRight,
            "bl" => Corner::BackLeft,
            "br" => Corner::BackRight,
            _ => Corner::FrontLeft,
        }
    }
}

/// How the operator establishes XY zero for this setup.
#[derive(Debug, Clone, PartialEq)]
pub enum XYDatum {
    CornerProbe(Corner),
    CenterOfStock,
    AlignmentPins,
    Manual,
}

impl Default for XYDatum {
    fn default() -> Self {
        XYDatum::CornerProbe(Corner::FrontLeft)
    }
}

impl XYDatum {
    pub fn label(&self) -> &str {
        match self {
            XYDatum::CornerProbe(c) => match c {
                Corner::FrontLeft => "Corner Probe (Front-Left)",
                Corner::FrontRight => "Corner Probe (Front-Right)",
                Corner::BackLeft => "Corner Probe (Back-Left)",
                Corner::BackRight => "Corner Probe (Back-Right)",
            },
            XYDatum::CenterOfStock => "Center of Stock",
            XYDatum::AlignmentPins => "Alignment Pins",
            XYDatum::Manual => "Manual",
        }
    }

    pub fn to_key(&self) -> String {
        match self {
            XYDatum::CornerProbe(c) => format!("corner_{}", c.to_key()),
            XYDatum::CenterOfStock => "center".into(),
            XYDatum::AlignmentPins => "pins".into(),
            XYDatum::Manual => "manual".into(),
        }
    }

    pub fn from_key(s: &str) -> Self {
        if let Some(corner) = s.strip_prefix("corner_") {
            XYDatum::CornerProbe(Corner::from_key(corner))
        } else {
            match s {
                "center" => XYDatum::CenterOfStock,
                "pins" => XYDatum::AlignmentPins,
                "manual" => XYDatum::Manual,
                _ => XYDatum::default(),
            }
        }
    }
}

/// How the operator establishes Z zero for this setup.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ZDatum {
    #[default]
    StockTop,
    MachineTable,
    FixedOffset(f64),
    Manual,
}

impl ZDatum {
    pub fn label(&self) -> String {
        match self {
            ZDatum::StockTop => "Stock Top".into(),
            ZDatum::MachineTable => "Machine Table".into(),
            ZDatum::FixedOffset(z) => format!("Fixed Offset ({z:.1} mm)"),
            ZDatum::Manual => "Manual".into(),
        }
    }

    pub fn to_key(&self) -> String {
        match self {
            ZDatum::StockTop => "stock_top".into(),
            ZDatum::MachineTable => "table".into(),
            ZDatum::FixedOffset(z) => format!("offset:{z}"),
            ZDatum::Manual => "manual".into(),
        }
    }

    pub fn from_key(s: &str) -> Self {
        if let Some(val) = s.strip_prefix("offset:") {
            ZDatum::FixedOffset(val.parse().unwrap_or(0.0))
        } else {
            match s {
                "table" => ZDatum::MachineTable,
                "manual" => ZDatum::Manual,
                _ => ZDatum::StockTop,
            }
        }
    }
}

/// How to establish the work coordinate system for a setup.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DatumConfig {
    pub xy_method: XYDatum,
    pub z_method: ZDatum,
    pub notes: String,
}

impl DatumConfig {
    /// True when this is the untouched default — the writer skips the
    /// keys entirely in that case, so old files stay byte-identical.
    pub fn is_default(&self) -> bool {
        *self == DatumConfig::default()
    }
}

/// A setup's orientation and toolpath indices.
///
/// The record derives `Debug` and `Clone` because the GUI setup panel
/// edits a CLONE of it and applies the fields that moved through the
/// `SetSetupFace`, `SetSetupRotation`, `SetSetupDatum` and
/// `SetSetupModels` command rows (WP6).
#[derive(Debug, Clone)]
pub struct SetupData {
    pub id: usize,
    pub name: String,
    pub face_up: FaceUp,
    /// Rotation of the stock about the vertical (Z) axis.
    pub z_rotation: ZRotation,
    /// How the operator zeroes the machine for this setup. Persisted
    /// since W9 / P-2; before that it was GUI-only overlay state and
    /// was lost on every save.
    pub datum: DatumConfig,
    /// Models in scope for this setup. **Empty means "all models"** —
    /// it is not the same as an explicit list naming every model, and
    /// it is not derivable from the setup's toolpaths (a toolpath names
    /// exactly one model; the scope is what the operator allowed, not
    /// what got used). Persisted for that reason.
    pub model_ids: Vec<ModelId>,
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

/// Provenance stamp for a toolpath the multi-tool finishing planner emitted
/// (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` Phase O item 1).
///
/// Without it a planner-emitted tier is **indistinguishable from a hand-built
/// op** (T3 finding C1), which makes re-planning either impossible or
/// destructive: the reconciler cannot tell which ops it owns and may delete
/// the operator's own work, or leave a stale ladder accumulating beside a new
/// one. `None` — the default, and what every project written before this
/// existed deserializes to — means "the operator built this".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlannerOrigin {
    /// Which plan run emitted this op. Monotonic within a session; a
    /// re-plan allocates a fresh id so two ladders can never be conflated
    /// even if one survived a removal.
    pub plan_id: u64,
    /// Ladder index, coarse → fine. `0` is the coarsest tool.
    pub tier: u8,
    /// Ladder length the plan was built with, carried so a reader can say
    /// "tier 1 of 2" without holding the rest of the chain.
    pub tier_count: u8,
}

/// Configuration for a single toolpath within the session.
///
/// The record derives `Debug` and `Clone` because
/// [`Command::ReplaceToolpathConfig`] and [`Command::AddToolpath`] each
/// carry one and the generated `Command` enum derives both. The GUI
/// inspector's projection also clones the stored configuration before it
/// applies the entry's sixteen fields, which is what keeps `id`,
/// `boundary_inherit` and `planner_origin` alive across a panel frame.
#[derive(Debug, Clone)]
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
    /// Op-agnostic rest analysis (P2.5): when enabled, runs the rest-depth
    /// detector against this toolpath's own tool as the fine cutter after
    /// generation, attaching `rest_grid` / `rest_regions` to the result —
    /// available to every operation family, not just pencil's `RestDepth`
    /// detector arm. See `compute::config::RestAnalysisConfig`.
    pub rest_analysis: crate::compute::config::RestAnalysisConfig,
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
    /// `Some` when the multi-tool finishing planner emitted this op. See
    /// [`PlannerOrigin`]; `None` means the operator built it, and is what
    /// every pre-Phase-O project file loads as.
    pub planner_origin: Option<PlannerOrigin>,
}

impl ToolpathConfig {
    /// A signature of the fields that decide this toolpath's generated
    /// geometry.
    ///
    /// Two configurations with the same signature generate the same path,
    /// so a difference across a write is exactly the condition that must
    /// drop the cached result. Name, coolant, pre and post G-code and the
    /// debug options are deliberately absent — editing them dirties the
    /// project but changes no motion (R0.1 §4.3). `enabled` is absent
    /// too: its own transition keeps the toggled operation's result for a
    /// re-enable.
    ///
    /// Serialized rather than compared field by field because
    /// [`OperationConfig`], [`HeightsConfig`] and [`DressupConfig`] are
    /// not `PartialEq`.
    ///
    /// **This is the one definition.** WP5 moved it out of the GUI
    /// inspector (`rs_cam_viz/src/ui/properties/mod.rs`), where it was a
    /// free function with two callers. Both callers ask the question here
    /// now: [`Command::ReplaceToolpathConfig`] gates its invalidation on
    /// it, and the feeds Apply funnel
    /// (`controller/events/mod.rs::apply_feeds_through_funnel`) compares
    /// it either side of its own write before it calls
    /// [`ProjectSession::invalidate_toolpath_inputs`]. The two routes
    /// agree because they share this method.
    pub fn generation_inputs_signature(&self) -> String {
        format!(
            "{}|{}|{}|{:?}|{:?}|{}|{}|{:?}|{:?}",
            serde_json::to_string(&self.operation).unwrap_or_default(),
            serde_json::to_string(&self.dressups).unwrap_or_default(),
            serde_json::to_string(&self.heights).unwrap_or_default(),
            self.boundary,
            self.rest_analysis,
            self.tool_id,
            self.model_id,
            self.stock_source,
            self.face_selection,
        )
    }
}

/// Result of generating a single toolpath.
///
/// `op_data` carries either a plain [`crate::trace::toolpath_spans::AnnotatedToolpath`]
/// or a [`crate::ops::drill_op::DrillOp`] + `AnnotatedToolpath` pair (the
/// dual-representation invariant from §6.E of the dexel-fidelity roadmap).
/// Spans on the annotated toolpath are emitted by operation generators;
/// transforms (dressups, boundary clip, TSP, arc-fit, feed optimisation)
/// either remap them or set
/// [`AnnotatedToolpath::spans_valid`](crate::trace::toolpath_spans::AnnotatedToolpath::spans_valid)
/// to `false` when they can't.
///
/// The record derives `Debug` and `Clone` because
/// [`Command::AdoptResult`] carries one and the generated `Command` enum
/// derives both. A clone copies the `Arc` on the annotated toolpath, not
/// the geometry behind it.
#[derive(Debug, Clone)]
pub struct ToolpathComputeResult {
    pub op_data: crate::ops::drill_op::OpData,
    pub stats: ToolpathStats,
    pub debug_trace: Option<ToolpathDebugTrace>,
    /// CMP-23: an `Arc`, not an owned value, so a `SimulationRequest` built
    /// twice from the same result hands the S5 prefix memo the SAME pointer.
    /// `EntryKey` keys this by `Weak` identity; a fresh `Arc::new(t.clone())`
    /// per request build made the memo miss on every session ladder round.
    pub semantic_trace: Option<Arc<ToolpathSemanticTrace>>,
}

impl ToolpathComputeResult {
    /// Convenience accessor for the linearized annotated toolpath.
    /// Returns a reference regardless of whether `op_data` is the plain
    /// `Toolpath` or the `DrillOp` variant.
    pub fn annotated(&self) -> &Arc<crate::trace::toolpath_spans::AnnotatedToolpath> {
        self.op_data.annotated()
    }

    /// Convenience accessor for the underlying [`Toolpath`].
    pub fn toolpath(&self) -> &Toolpath {
        &self.op_data.annotated().toolpath
    }

    /// First-class drill-op view, if this is a drilling operation.
    pub fn drill_op(&self) -> Option<&Arc<crate::ops::drill_op::DrillOp>> {
        self.op_data.drill_op()
    }

    /// True if this result represents a drilling operation
    /// (`OperationConfig::Drill` or `AlignmentPinDrill`).
    pub fn is_drill_op(&self) -> bool {
        self.op_data.is_drill_op()
    }
}

/// Summary of a toolpath for listing.
///
/// Stays `pub`: `ProjectSession::list_toolpaths` returns it, so a
/// crate-private form raises `private_interfaces` (S29, 2026-09-16).
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
/// engagement depth ([`crate::feeds::ToolGeometryHint::engaged_diameter_at_doc`]
/// for the LUT row, `feeds::geometry::ball_effective_diameter` and friends
/// for the contact circle chip thinning uses), which can differ
/// substantially for tapered / ball / bullnose tools. Geometry context is
/// included so
/// consumers can correlate the named diameter with the effective
/// LUT-lookup diameter rather than reading a single number that doesn't
/// tell the whole story.
///
/// Stays `pub`: `ProjectSession::list_tools` returns it, and
/// `rs_cam_cli::project` reads that return (S29, 2026-09-16).
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
    /// The size as a person reads it, from [`ToolConfig::size_label`]:
    /// `Ø` prefixes a diameter and `R` a radius, for example
    /// "tip Ø2.00 mm (R1.00), shank Ø6.00". The numeric fields above stay
    /// the wire; this string is for display only.
    pub size_label: String,
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
    /// F-036b: when `true`, the simulator runs a post-sim modulation pass
    /// that rewrites per-move `feed_rate` on every cutting move so the
    /// commanded chipload-per-tooth lands inside the LUT band's
    /// geometric midpoint (corrected for chip thinning) — Fusion HSM's
    /// "adaptive feed control" equivalent.
    ///
    /// **Default `true` since 2026-08-13 — Checkpoint J-3 (operator,
    /// BINDING).** It was `false`. The flip is the second half of the
    /// Checkpoint J ruling: J-1 retired Suggest's blind pre-simulation
    /// feed lift (see the retirement note in [`crate::feeds::predict`]),
    /// and this is where the lift now lives — a correction made from a
    /// **measured** gate observation instead of an operation-family
    /// constant. Measured on A-5's four fixtures: Suggest's shipped feed
    /// is `Within` 2/4 unmodulated and **4/4 modulated**.
    ///
    /// **Correction, same commit (rule 5 — a changed instrument makes its
    /// own docstring a lie you then cite).** This doc used to say
    /// modulation fires only "when the active `MachineProfile` carries
    /// `kinematics`". That is **false**.
    /// `ProjectSession::apply_adaptive_feed_modulation` calls
    /// `self.machine.effective_kinematics()` — the generic-wood-router
    /// fallback — explicitly "so it applies on every machine, matching
    /// the strategy advisor". Every built-in `MachineProfile` preset has
    /// `kinematics: None`, so under the old claim A-5's modulated arms
    /// could not have fired; they did, on a default profile.
    ///
    /// What DOES gate it: chipload TARGETING requires a vendor
    /// `ChiploadBand` (LUT `chip_load_min_mm` + `chip_load_max_mm`) for the
    /// active `(tool family, material, op family, pass role, diameter)`
    /// tuple. Since 2026-09-19 (wanaka200 IMPLEMENTATION_PLAN work item
    /// A), a toolpath with no band modulates BANDLESS — machine
    /// cutting ceiling + geometric plunge guard — instead of skipping,
    /// and a simulation run with `metrics_enabled: false` (no cut trace)
    /// still falls through as a no-op.
    ///
    /// **Consumer census taken at the flip** — no shipped surface changed
    /// behaviour, because not one of them inherits this default:
    /// * GUI **and** MCP pin `true` already, at
    ///   `rs_cam_viz/src/controller/events/compute.rs`. Note they never
    ///   call `ProjectSession::run_simulation` at all — the dexel sim
    ///   runs through `compute::simulate::run_simulation_with_phase`,
    ///   whose request type carries no modulation fields, and modulation
    ///   arrives as a main-thread post-pass (`modulate_simulation_trace`).
    /// * CLI `project.rs` builds a full struct literal whose field comes
    ///   from `--adaptive-feed-modulation`. **That flag defaulted `false`
    ///   at the census and defaults `true` since (g1)**
    ///   (`crates/rs_cam_cli/src/project.rs`), so the GUI/CLI divergence
    ///   this bullet recorded is gone; J-3 ruled the library default only,
    ///   and (g1) closed the CLI side separately. Corrected 2026-09-08
    ///   (found alongside G-AIRDENOM).
    /// * CLI `smoke.rs` and `tool_load::optimize::candidate` pin `false`
    ///   in full struct literals, protecting the smoke baseline and
    ///   keeping the optimizer's candidate ranking un-conflated.
    ///
    /// The algorithm itself lives in
    /// `crate::dressup::feed_modulation::adaptive_feed_modulate`; see
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
    pub modulation_strategy: crate::dressup::feed_modulation::ModulationStrategy,
    /// F-039 — the modulator's feed scale. The constrained-max solver
    /// multiplies the smallest candidate feed limit by this value, then
    /// applies the chipload-min floor.
    ///
    /// `1.0` = emit at the binding constraint (production CAM
    /// default — Fusion HSM 100 %). `0.7` = back off 30 % for
    /// safety margin. `1.1` = push 10 % past the limit (NOT
    /// recommended; chipload-min still applies). Ignored when
    /// `modulation_strategy == BandMid`. This is not the machine dial
    /// `MachineProfile::aggressiveness` (ruling R4 Q6, 2026-09-24).
    pub modulation_feed_scale: f64,
}

impl Default for SimulationOptions {
    fn default() -> Self {
        Self {
            resolution: 0.5,
            skip_ids: Vec::new(),
            metrics_enabled: true,
            auto_resolution: false,
            use_predicted_feed_in_gates: false,
            // Checkpoint J-3, 2026-08-13 (operator, BINDING): was `false`.
            adaptive_feed_modulation: true,
            modulation_strategy:
                crate::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
            modulation_feed_scale: 1.0,
        }
    }
}

// ── ProjectSession ─────────────────────────────────────────────────────

/// Unified project session that owns state and provides compute methods.
///
/// Use [`ProjectSession::load`] to load from a TOML project file, or
/// [`ProjectSession::from_project_file`] to construct from a parsed file.
///
/// # The clone (WP14b, §24 ruling 2)
///
/// The session derives `Clone` so the `optimize_toolpath` job can own a
/// private copy. That row is the one job that MUTATES what it scores: it
/// writes the toolpath's parameters, regenerates it and re-simulates the
/// project once per candidate. It cannot reduce its inputs to a capture
/// list the way the two read rows do, so it copies the session instead of
/// borrowing the caller's.
///
/// **A clone is cheaper than the field list suggests.** Every geometry
/// field is behind an `Arc` and costs a refcount: the model meshes and
/// polygons ([`LoadedModel`]), the annotated toolpath of each cached
/// result ([`ToolpathComputeResult`]), and the simulation's checkpoints,
/// cut trace and prior stocks
/// ([`SimulationResult`](crate::compute::simulate::SimulationResult)).
/// The copied weight is the simulation's display mesh and its two
/// deviation vectors, which scale with the dexel column population.
///
/// The type derives no `Debug`: `SimulationResult` publishes none, by
/// design.
#[derive(Clone)]
pub struct ProjectSession {
    // Project metadata
    pub(crate) name: String,
    pub(crate) stock: StockConfig,
    pub(crate) post: crate::gcode::PostConfig,
    pub(crate) machine: crate::machine::MachineProfile,

    // Loaded state
    pub(crate) models: Vec<LoadedModel>,
    pub(crate) tools: Vec<ToolConfig>,
    pub(crate) setups: Vec<SetupData>,
    pub(crate) toolpath_configs: Vec<ToolpathConfig>,

    // Computed results (keyed by toolpath index)
    pub(crate) results: HashMap<usize, ToolpathComputeResult>,
    /// Per-toolpath generation-input revision, keyed by toolpath index.
    ///
    /// Bumped by [`ProjectSession::drop_result`] — the single site that
    /// removes a cached result because an input changed — and never by
    /// [`ProjectSession::insert_result`], which records an answer rather
    /// than a change. A reader that recorded the revision when it asked
    /// for a generation can therefore tell "this answer is for the config
    /// I asked about" from "the config moved while I waited"; an absent
    /// `results` entry alone cannot say that, because the submit does not
    /// drop it and the drain inserts it. See
    /// `planning/ui_fix_2026-09-09/research/R0.1.md` §4.2.
    ///
    /// Values come from [`Self::next_revision`] and are unique across the
    /// whole session, so an index shift (a removal, a bulk replace) can
    /// invalidate every in-flight comparison by bumping each index.
    pub(crate) toolpath_revision: HashMap<usize, u64>,
    /// Monotonic source of [`Self::toolpath_revision`] values. Never reset.
    pub(crate) next_revision: u64,
    pub(crate) simulation: Option<SimulationResult>,
    /// How many times an edit has dropped the simulation.
    ///
    /// The generation half of the same rule `toolpath_revision` states:
    /// a run that answers a superseded project state must be refusable.
    /// `next_revision` cannot serve, because `invalidate_machine`,
    /// `set_machine`, `set_machine_kinematics`, `import_machine_settings`,
    /// `set_post_config`, `replace_tools` and `set_toolpath_enabled` all
    /// clear the simulation and move no toolpath revision.
    ///
    /// Bumped by [`Self::drop_simulation`] and by nothing else, so two
    /// edits move it twice. NOT persisted: a load builds a session with no
    /// simulation, and no in-flight run can outlive the process.
    pub(crate) simulation_epoch: u64,

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
            post: crate::gcode::PostConfig::default(),
            machine: crate::machine::MachineProfile::default(),
            models: Vec::new(),
            tools: Vec::new(),
            setups: vec![SetupData {
                id: 0,
                name: "Setup 1".to_owned(),
                face_up: FaceUp::default(),
                z_rotation: ZRotation::default(),
                datum: DatumConfig::default(),
                model_ids: Vec::new(),
                fixtures: Vec::new(),
                keep_out_zones: Vec::new(),
                toolpath_indices: Vec::new(),
                pause_message: None,
            }],
            toolpath_configs: Vec::new(),
            results: HashMap::new(),
            toolpath_revision: HashMap::new(),
            next_revision: 0,
            simulation_epoch: 0,
            simulation: None,
            next_toolpath_id: 0,
            next_tool_id: 0,
            next_setup_id: 1,
            next_model_id: 0,
        }
    }

    /// Load a project from a TOML file path.
    ///
    /// The load warnings are discarded. A surface that shows them calls
    /// [`Self::load_with_warnings`].
    pub fn load(path: &Path) -> Result<Self, SessionError> {
        Self::load_with_warnings(path).map(|(session, _)| session)
    }

    /// Load a project, and report what the loader noticed on the way.
    ///
    /// Each warning names something the loader could not do:
    ///
    /// - It could not read a model file.
    /// - It did not know a tool type, so it used an end mill.
    /// - A toolpath names a tool or a model the file does not define.
    ///
    /// None of these fails the load. The GUI shows them in its load
    /// warnings window.
    pub fn load_with_warnings(
        path: &Path,
    ) -> Result<(Self, Vec<ProjectLoadWarning>), SessionError> {
        let content = std::fs::read_to_string(path)?;
        let project: ProjectFile =
            toml::from_str(&content).map_err(|e| SessionError::TomlParse(e.to_string()))?;
        project_file::validate_looks_like_cam_project(&project, Some(path))?;
        let base_dir = path.parent().unwrap_or(Path::new("."));
        let (mut session, warnings) = Self::from_project_file_with_warnings(project, base_dir)?;
        if (session.name == "Untitled" || session.name.trim().is_empty())
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        {
            session.name = stem.to_owned();
        }
        Ok((session, warnings))
    }

    /// Construct a session from a parsed project file.
    pub fn from_project_file(project: ProjectFile, base_dir: &Path) -> Result<Self, SessionError> {
        Self::from_project_file_with_warnings(project, base_dir).map(|(session, _)| session)
    }

    /// Construct a session from a parsed project file, with the warnings
    /// [`Self::load_with_warnings`] describes.
    pub(crate) fn from_project_file_with_warnings(
        project: ProjectFile,
        base_dir: &Path,
    ) -> Result<(Self, Vec<ProjectLoadWarning>), SessionError> {
        let mut warnings = Vec::new();
        let session = project_file::build_session_from_project(project, base_dir, &mut warnings)?;
        Ok((session, warnings))
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
                size_label: t.size_label(),
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

    /// The toolpath's generation-input revision. Changes whenever an edit
    /// dropped its cached result; `0` for a toolpath no edit has touched.
    ///
    /// Read it beside a generation request and compare it when the answer
    /// arrives: an unequal revision means the config moved while the
    /// worker ran, so the answer describes inputs the project no longer
    /// has. See [`Self::toolpath_revision`] for why an absent result
    /// cannot answer that question.
    pub fn toolpath_revision(&self, index: usize) -> u64 {
        self.toolpath_revision.get(&index).copied().unwrap_or(0)
    }

    /// Get the simulation result, if one has been run.
    pub fn simulation_result(&self) -> Option<&SimulationResult> {
        self.simulation.as_ref()
    }

    /// How many times an edit has dropped the simulation.
    ///
    /// Read it beside a simulation request and hand it back on
    /// [`Command::AdoptSimulation`]. An unequal epoch when the answer
    /// arrives means the project moved while the worker ran, so the run
    /// describes material the project no longer cuts.
    pub fn simulation_epoch(&self) -> u64 {
        self.simulation_epoch
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
    pub fn post_config(&self) -> &crate::gcode::PostConfig {
        &self.post
    }

    /// Machine profile.
    pub fn machine(&self) -> &crate::machine::MachineProfile {
        &self.machine
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

    /// The bbox of one model, by id — the singular of
    /// [`Self::collect_model_bboxes`].
    ///
    /// Q1: a Suggest call site holds one `model_id` and needs the one
    /// bbox. `None` means the id names no model, or the model carries
    /// no finite geometry (a placeholder row).
    #[must_use]
    pub fn model_bbox(&self, model_id: usize) -> Option<BoundingBox3> {
        self.models.iter().find(|m| m.id == model_id)?.bbox()
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
    /// stock material, post-config spindle strategy, and
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
            lut: crate::feeds::embedded_vendor_lut(),
            spindle_strategy: self.post_config().spindle_strategy,
            context,
        }))
    }

    // ── Mutable accessors ─────────────────────────────────────────
    //
    // These provide raw mutable access for core-internal bulk operations.
    // WP7 closed all nine: a surface outside this crate mutates a session
    // through `ProjectSession::apply` with the matching `Command` row, or
    // builds one with `ProjectSessionBuilder`. Three hatches keep an
    // in-crate caller and are `pub(crate)`; the other six had no caller
    // at all and WP7 deleted them. Prefer a named mutation method in
    // `mutation.rs` in-crate too — a raw write runs no cache
    // invalidation, which is the defect G-FRESHSTATE and N13 each closed
    // one caller at a time. The sentry
    // `crates/rs_cam_core/tests/hatches_are_crate_private_wp7.rs` holds
    // the boundary: each of the nine names is `pub(crate)` or absent.
    //
    // WP7 deleted `stock_mut`, `machine_mut`, `tools_mut`, `models_mut`,
    // `post_mut` and `find_setup_by_id_mut`. A core-internal caller that
    // needs one writes the field directly, or adds a named mutation
    // method that invalidates.

    // ── ID lookup helpers ─────────────────────────────────────────

    /// Find a toolpath config by its semantic ID (not vec index).
    pub fn find_toolpath_config_by_id(&self, id: ToolpathId) -> Option<(usize, &ToolpathConfig)> {
        self.toolpath_configs
            .iter()
            .enumerate()
            .find(|(_, tc)| tc.id == id)
    }

    /// Find a mutable toolpath config by its semantic ID.
    ///
    /// Crate-private since WP7; every surface mutates through
    /// `ProjectSession::apply`.
    pub(crate) fn find_toolpath_config_by_id_mut(
        &mut self,
        id: ToolpathId,
    ) -> Option<(usize, &mut ToolpathConfig)> {
        self.toolpath_configs
            .iter_mut()
            .enumerate()
            .find(|(_, tc)| tc.id == id)
    }

    /// Toolpaths that currently consume `source_id`'s rest-depth analysis as
    /// their machining boundary — every toolpath with an *enabled*
    /// `BoundarySource::DerivedRestRegions { source_toolpath_id }` pointing at
    /// `source_id`. Used both to label the producer's UI ("Producing rest
    /// regions for: ...") and to decide whether the producer must run its
    /// rest-depth pass at all (demand-driven rest analysis — see
    /// `mutation::auto_enable_rest_analysis_for_source`).
    pub fn rest_region_consumers(&self, source_id: ToolpathId) -> Vec<ToolpathId> {
        self.toolpath_configs
            .iter()
            .filter(|tc| {
                tc.boundary.enabled
                    && matches!(
                        tc.boundary.source,
                        BoundarySource::DerivedRestRegions { source_toolpath_id }
                            if source_toolpath_id == source_id
                    )
            })
            .map(|tc| tc.id)
            .collect()
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

    /// Mutable access to all toolpath configs.
    ///
    /// Prefer named mutation methods: [`set_toolpath_enabled()`](Self::set_toolpath_enabled),
    /// [`set_face_selection()`](Self::set_face_selection),
    /// [`set_dressup_config()`](Self::set_dressup_config), etc.
    ///
    /// Crate-private since WP7; every surface mutates through
    /// `ProjectSession::apply`.
    pub(crate) fn toolpath_configs_mut(&mut self) -> &mut Vec<ToolpathConfig> {
        &mut self.toolpath_configs
    }

    /// Mutable access to all setups.
    ///
    /// Prefer named mutation methods: [`rename_setup()`](Self::rename_setup),
    /// [`add_fixture()`](Self::add_fixture), [`remove_fixture()`](Self::remove_fixture),
    /// [`add_keep_out()`](Self::add_keep_out), [`remove_keep_out()`](Self::remove_keep_out),
    /// [`move_toolpath_to_setup()`](Self::move_toolpath_to_setup).
    ///
    /// Crate-private since WP7; every surface mutates through
    /// `ProjectSession::apply`.
    ///
    /// **Test door.** The only caller is the `#[cfg(test)]` module of
    /// `session/save.rs`, so the door compiles under `cfg(test)` only.
    /// S30 (tech debt 2026-09-16) replaced the `allow(dead_code)` with
    /// this gate: a production build no longer carries the door at all.
    #[cfg(test)]
    pub(crate) fn setups_mut(&mut self) -> &mut Vec<SetupData> {
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

    // G8 removed `transform_mesh_to_setup`. Its one caller
    // (`resolve_generation_inputs`) now goes through
    // `geom_cache::cached_transform`, which needs the `SetupTransformInfo`
    // itself as part of the memo key, so the wrapper that hid it had no
    // remaining use. `setup_transform_info(..).apply_to_mesh(..)` is the
    // uncached spelling if one is ever needed again.

    /// Transform a model's **drawing** polygons into the setup's work plane.
    ///
    /// See
    /// [`SetupTransformInfo::apply_to_drawing_polygons`](crate::compute::transform::SetupTransformInfo::apply_to_drawing_polygons)
    /// — this is the door for SVG/DXF/STEP-face geometry that describes the
    /// part. World-anchored footprints take
    /// [`Self::transform_footprints_to_setup`] instead.
    pub(crate) fn transform_drawing_polygons_to_setup(
        &self,
        polygons: &[Polygon2],
        face_up: FaceUp,
        z_rotation: ZRotation,
    ) -> Vec<Polygon2> {
        self.setup_transform_info(face_up, z_rotation)
            .apply_to_drawing_polygons(polygons)
    }

    /// Transform **world-frame** footprints (fixtures, keep-out zones) from
    /// global to setup-local XY coordinates.
    ///
    /// Clamps and no-go zones are bolted to the machine table; they do not
    /// follow the part when it is turned on its side, so they keep the
    /// orthographic world→local projection that drawings no longer take.
    pub(crate) fn transform_footprints_to_setup(
        &self,
        polygons: &[Polygon2],
        face_up: FaceUp,
        z_rotation: ZRotation,
    ) -> Vec<Polygon2> {
        self.setup_transform_info(face_up, z_rotation)
            .apply_to_polygons(polygons)
    }

    /// The two lateral-setup preconditions, in ONE place — the single
    /// source of both refusal messages, called by every generation door
    /// (core `resolve_generation_inputs` for the session / CLI / headless
    /// MCP path, and the GUI controller before it submits to the worker).
    ///
    /// # Why this is not in `compute::execute`
    ///
    /// `execute_operation_annotated` is the narrower choke
    /// point and would have covered both doors on its own — but it can only
    /// see the mesh of the model *this toolpath references*. The case the
    /// rule exists to serve is a mortise bounded by a DXF cut into an
    /// STL-modelled part, where those are two different models, and a
    /// per-op check would refuse exactly that. "Does the project have a
    /// part?" is session-level knowledge, so the check is too.
    ///
    /// # The two refusals
    ///
    /// **No mesh.** `face_up` only means something when there is a 3D model
    /// to register against. On a 2D-only project the drawing *is* the
    /// design — there is no second face to turn to, because the part is
    /// whatever the drawing cuts from a block — so a side face there is a
    /// category error rather than an unimplemented feature. The operator
    /// ruling (2026-08-22) confirmed edge-authored artwork on a mesh-less
    /// project is not a real workflow: it is done as a Top setup with the
    /// edge as the stock face, which is what the message says.
    ///
    /// **Keep-outs (G-LATERALKEEPOUT).** A fixture or keep-out footprint is
    /// a world-XY rectangle. Projected into a vertical work plane it is a
    /// line, and a line subtracts nothing — so the keep-out would silently
    /// vanish and the tool would be free to drive through the clamp. That
    /// is a safety regression, so a lateral setup carrying an ENABLED
    /// fixture or keep-out zone refuses. Follow-up would be a proper 3D
    /// prism projection; refusing is the honest interim.
    pub fn check_lateral_setup_support(
        &self,
        setup: Option<&SetupData>,
        operation: &crate::compute::OperationConfig,
    ) -> Result<(), SessionError> {
        let Some(setup) = setup else {
            return Ok(());
        };
        if !setup.face_up.is_lateral() {
            return Ok(());
        }
        let face = setup.face_up.label();

        if setup.fixtures.iter().any(|f| f.enabled)
            || setup.keep_out_zones.iter().any(|z| z.enabled)
        {
            return Err(SessionError::UnsupportedSetup(format!(
                "setup '{}' is on the {face} face and carries a fixture or keep-out zone. \
                 Keep-out footprints are world-XY rectangles with no extent in a vertical \
                 work plane, so honouring them here would silently drop them and let the \
                 tool drive through the clamp. Lateral keep-outs are not supported yet \
                 (G-LATERALKEEPOUT) — disable them, or machine this face from a Top setup.",
                setup.name
            )));
        }

        // Only ops that actually consume a 2D drawing are affected; a 3D op
        // already requires the mesh, and a stock-based op reads no drawing.
        if operation.geometry_requirement()
            == crate::compute::catalog::GeometryRequirement::Polygons
            && self.models.iter().all(|m| m.mesh.is_none())
        {
            return Err(SessionError::UnsupportedSetup(format!(
                "setup '{}' is on the {face} face, but this project has no 3D model. \
                 A 2D drawing is consumed in the work plane of the setup that uses it, \
                 and 'which face is up' only means something when there is a part to \
                 register against — on a drawing-only project the drawing IS the design. \
                 Author this as a Top setup with the edge as the stock face, which is \
                 what you would physically do at the machine.",
                setup.name
            )));
        }

        Ok(())
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
        };
        let session = ProjectSession::from_project_file(project, Path::new(".")).unwrap();
        let diag = session.diagnostics();
        assert!(diag.verdicts.is_empty());
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
                xy_datum: String::new(),
                z_datum: String::new(),
                datum_notes: String::new(),
                model_ids: Vec::new(),
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
                    debug_options: crate::trace::debug_trace::ToolpathDebugOptions::default(),
                    feeds_provenance: crate::feeds::FeedsProvenance::default(),
                    rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
                    planner_origin: None,
                }],
            }],
        };

        let session = ProjectSession::from_project_file(project, Path::new(".")).unwrap();
        assert_eq!(session.setup_count(), 1);
        assert_eq!(session.toolpath_count(), 1);
        let tp = &session.toolpath_configs[0];
        assert_eq!(tp.operation.op_type(), OperationType::Profile);
        let expected = OperationConfig::new_default(OperationType::Profile);
        assert_eq!(tp.operation.op_type(), expected.op_type());
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

    /// The loader reads one project format (C03).
    ///
    /// rs_cam writes `format_version = 3`. An older file is refused
    /// rather than read through a second, drifting reader — the ruling
    /// the I01 sweep produced. A file with no `format_version` key reads
    /// as version 1 and is refused the same way.
    #[test]
    fn rejects_a_project_whose_format_version_is_not_three() {
        // Format 2 kept the alignment pins on the setup. Core stores them
        // on the stock, ignores the setup key, and used to load such a
        // file with the pins silently dropped.
        let toml_str = concat!(
            "format_version = 2\n",
            "[job]\n",
            "name = \"A Format 2 Project\"\n",
            "[[setups]]\n",
            "id = 0\n",
            "name = \"Setup 1\"\n",
            "[[setups.alignment_pins]]\n",
            "x = 10.0\n",
            "y = 10.0\n",
            "diameter = 6.0\n",
        );
        let project: ProjectFile = toml::from_str(toml_str).unwrap();
        match ProjectSession::from_project_file(project, Path::new(".")) {
            Err(SessionError::UnsupportedFormatVersion { found }) => assert_eq!(found, 2),
            Err(other) => panic!("expected UnsupportedFormatVersion, got {other:?}"),
            Ok(_) => panic!("the loader must refuse a format 2 project"),
        }
    }

    /// A missing key is a pre-key file, so it is refused as version 1.
    #[test]
    fn rejects_a_project_with_no_format_version_key() {
        let toml_str = concat!("[job]\n", "name = \"No Version Key\"\n");
        let project: ProjectFile = toml::from_str(toml_str).unwrap();
        match ProjectSession::from_project_file(project, Path::new(".")) {
            Err(SessionError::UnsupportedFormatVersion { found }) => assert_eq!(found, 1),
            Err(other) => panic!("expected UnsupportedFormatVersion, got {other:?}"),
            Ok(_) => panic!("the loader must refuse a project with no format_version"),
        }
    }

    /// L1. The load-time dressup migration rewrites a value the
    /// operator stored, so it reports through the warning channel and
    /// not only through `tracing::info!`. ProjectCurve is a strip-all
    /// operation: entry, lead-in/out and link moves all go.
    #[test]
    fn normalised_dressups_are_reported_to_the_operator() {
        let toml_str = concat!(
            "format_version = 3\n",
            "[job]\n",
            "name = \"Dressup Migration\"\n",
            "[[setups]]\n",
            "id = 0\n",
            "name = \"Setup 1\"\n",
            "[[setups.toolpaths]]\n",
            "id = 0\n",
            "name = \"River\"\n",
            "enabled = true\n",
            "type = \"project_curve\"\n",
            "tool_id = 0\n",
            "model_id = 0\n",
            "[setups.toolpaths.dressups]\n",
            "entry_style = \"ramp\"\n",
            "ramp_angle = 3.0\n",
            "helix_radius = 2.0\n",
            "helix_pitch = 1.0\n",
            "dogbone = false\n",
            "dogbone_angle = 90.0\n",
            "lead_in_out = true\n",
            "lead_radius = 2.0\n",
            "link_moves = true\n",
            "link_max_distance = 10.0\n",
            "link_feed_rate = 500.0\n",
            "arc_fitting = true\n",
            "arc_tolerance = 0.05\n",
            "feed_optimization = false\n",
            "feed_max_rate = 3000.0\n",
            "feed_ramp_rate = 200.0\n",
            "optimize_rapid_order = true\n",
        );
        let project: ProjectFile = toml::from_str(toml_str).unwrap();
        let (session, warnings) =
            ProjectSession::from_project_file_with_warnings(project, Path::new(".")).unwrap();

        // The migration still writes what it always wrote.
        let dressups = &session.toolpath_configs[0].dressups;
        assert_eq!(
            dressups.entry_style,
            crate::compute::config::DressupEntryStyle::None
        );
        assert!(dressups.lead_in_out.is_none());
        assert!(dressups.link_moves.is_none());

        let reported = warnings
            .iter()
            .find(|w| matches!(w, ProjectLoadWarning::DressupsNormalized { .. }))
            .expect("the loader must report the dressups it rewrote");
        let ProjectLoadWarning::DressupsNormalized {
            toolpath,
            op,
            changes,
        } = reported
        else {
            panic!("matched variant, then read another: {reported:?}");
        };
        assert_eq!(toolpath, "River");
        assert_eq!(op, "ProjectCurve");
        assert_eq!(
            changes,
            &vec![
                "entry_style Ramp to None".to_owned(),
                "lead_in_out true to false".to_owned(),
                "link_moves true to false".to_owned(),
            ],
            "the warning must name every value the loader rewrote"
        );
        let message = reported.message();
        assert!(
            message.contains("River") && message.contains("entry_style Ramp to None"),
            "the operator sentence must name the toolpath and the change: {message}"
        );
    }

    /// Q1: `model_bbox` answers for one id, and says `None` rather
    /// than guessing.
    ///
    /// The Suggest call sites read it to fill
    /// `SuggestContext::model_bbox`, which gates the runtime-sanity
    /// stepover back-off. A wrong-but-present bbox would be worse than
    /// none, so an id the session does not hold, and a model that
    /// carries no geometry, both read `None`.
    #[test]
    fn the_session_answers_for_one_model_bbox() {
        let session = ProjectSessionBuilder::new()
            .model(LoadedModel {
                id: 7,
                name: "Placeholder".to_owned(),
                mesh: None,
                polygons: None,
                drill_targets: std::sync::Arc::new(Vec::new()),
                layers: std::sync::Arc::new(Vec::new()),
                path: std::path::PathBuf::from("missing.stl"),
                kind: None,
                units: None,
                enriched_mesh: None,
                winding_report: None,
                load_error: None,
            })
            .build();

        assert!(
            session.model_bbox(7).is_none(),
            "a model with no geometry has no bbox to report"
        );
        assert!(
            session.model_bbox(99).is_none(),
            "an id the session does not hold reads None"
        );
    }

    /// Q4 tripwire, re-baselined by T8 and again by L9: the loader
    /// defaults an unknown token to EndMill and reports the
    /// substitution. L9 deleted the loader aliases, so `"ball"` and
    /// `"tapered_ball"` are unknown tokens now and each raises its own
    /// warning.
    #[test]
    fn tool_type_parsing() {
        let mut warnings = Vec::new();
        let mut parse = |token: &str| parse_tool_type(token, "Tool", &mut warnings);
        assert!(matches!(parse("end_mill"), ToolType::EndMill));
        assert!(matches!(parse("ball_nose"), ToolType::BallNose));
        assert!(matches!(parse("bull_nose"), ToolType::BullNose));
        assert!(matches!(parse("v_bit"), ToolType::VBit));
        assert!(matches!(
            parse("tapered_ball_nose"),
            ToolType::TaperedBallNose
        ));
        assert!(matches!(parse("unknown"), ToolType::EndMill));
        // L9: the deleted aliases take the unknown arm.
        assert!(matches!(parse("ball"), ToolType::EndMill));
        assert!(matches!(parse("tapered_ball"), ToolType::EndMill));

        // C10: the substitution is reported, not only traced. Three
        // tokens were unknown, so the channel carries three warnings.
        assert_eq!(
            warnings,
            vec![
                ProjectLoadWarning::UnknownToolType {
                    tool: "Tool".to_owned(),
                    token: "unknown".to_owned(),
                },
                ProjectLoadWarning::UnknownToolType {
                    tool: "Tool".to_owned(),
                    token: "ball".to_owned(),
                },
                ProjectLoadWarning::UnknownToolType {
                    tool: "Tool".to_owned(),
                    token: "tapered_ball".to_owned(),
                },
            ],
            "the loader must report every end-mill substitution"
        );
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
                xy_datum: String::new(),
                z_datum: String::new(),
                datum_notes: String::new(),
                model_ids: Vec::new(),
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
                    debug_options: crate::trace::debug_trace::ToolpathDebugOptions::default(),
                    feeds_provenance: crate::feeds::FeedsProvenance::default(),
                    rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
                    planner_origin: None,
                }],
            }],
        };
        ProjectSession::from_project_file(project, Path::new(".")).unwrap()
    }

    #[test]
    fn set_common_param_feed_rate() {
        let mut session = session_with_toolpath();
        let original = session.toolpath_configs[0].operation.feed_rate();

        let _ = session
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
        let _ = session
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

        let _ = session
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
                op_data: crate::ops::drill_op::OpData::Toolpath(std::sync::Arc::new(
                    crate::trace::toolpath_spans::AnnotatedToolpath::new(
                        crate::toolpath::Toolpath::new(),
                    ),
                )),
                stats: crate::compute::toolpath_stats::ToolpathStats::default(),
                debug_trace: None,
                semantic_trace: None,
            },
        );
        assert!(
            session.results.contains_key(&0),
            "Precondition: result cached"
        );

        let _ = session
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
            debug_options: crate::trace::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance: crate::feeds::FeedsProvenance::default(),
            rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
            planner_origin: None,
        };

        let idx = session
            .add_toolpath(0, new_tp)
            .unwrap()
            .created
            .expect("add_toolpath reports the new toolpath index");
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

        let _ = session.remove_toolpath(0).unwrap();
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
        use crate::stock::stock_mesh::StockMesh;

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
            column_deviations: None,
            boundaries: Vec::new(),
            checkpoints: Vec::new(),
            rapid_collisions: Vec::new(),
            rapid_collision_move_indices: Vec::new(),
            cut_trace: None,
            resolution_clamped: false,
            column_grid_cell_mm: 0.5,
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
        let _ = session.set_stock_config(new_stock);

        assert!(
            session.simulation_result().is_none(),
            "Simulation should be invalidated after set_stock_config"
        );
        assert!((session.stock_config().x - 200.0).abs() < 1e-6);
    }
}
