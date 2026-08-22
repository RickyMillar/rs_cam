//! TOML project file types and loading/saving helpers.

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{
    DatumConfig, Fixture, FixtureKind, KeepOutZone, LoadedGeometry, LoadedModel, SessionError,
    SetupData, ToolpathConfig, XYDatum, ZDatum,
};
use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use crate::compute::stock_config::{FixtureId, KeepOutId, ModelKind, ModelUnits, StockConfig};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::transform::{FaceUp, ZRotation};
use crate::debug_trace::ToolpathDebugOptions;
use crate::enriched_mesh::FaceGroupId;
use crate::gcode::CoolantMode;
use crate::mesh::TriangleMesh;

// ── Project file types (TOML deserialization) ──────────────────────────

/// Top-level project file structure (format_version=3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    #[serde(default = "default_format_version")]
    pub format_version: u32,
    #[serde(default)]
    pub job: ProjectJobSection,
    #[serde(default)]
    pub tools: Vec<ProjectToolSection>,
    #[serde(default)]
    pub models: Vec<ProjectModelSection>,
    #[serde(default)]
    pub setups: Vec<ProjectSetupSection>,
    /// Legacy: top-level toolpaths (pre-setup format).
    #[serde(default)]
    pub toolpaths: Vec<ProjectToolpathSection>,
}

fn default_format_version() -> u32 {
    1
}

/// Job-level settings (name, stock, post).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectJobSection {
    #[serde(default = "default_job_name")]
    pub name: String,
    #[serde(default)]
    pub stock: ProjectStockConfig,
    #[serde(default)]
    pub post: ProjectPostConfig,
    #[serde(default)]
    pub machine: crate::machine::MachineProfile,
    /// LEGACY. Machines now use snapshot semantics (the inline `machine`
    /// is authoritative; see `machine_library`). This field is retained
    /// only so old project files still parse — it is read then dropped on
    /// load, and never written back (snapshot files omit it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_ref: Option<String>,
}

fn default_job_name() -> String {
    "Untitled".to_owned()
}

/// Stock dimensions as saved in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStockConfig {
    #[serde(default = "default_stock_dim")]
    pub x: f64,
    #[serde(default = "default_stock_dim")]
    pub y: f64,
    #[serde(default = "default_stock_z")]
    pub z: f64,
    #[serde(default)]
    pub origin_x: f64,
    #[serde(default)]
    pub origin_y: f64,
    #[serde(default)]
    pub origin_z: f64,
    #[serde(default = "default_stock_padding")]
    pub padding: f64,
    #[serde(default = "default_workholding_rigidity")]
    pub workholding_rigidity: crate::feeds::WorkholdingRigidity,
    #[serde(default = "default_true")]
    pub auto_from_model: bool,
    #[serde(default)]
    pub material: crate::material::Material,
    #[serde(default)]
    pub alignment_pins: Vec<crate::compute::stock_config::AlignmentPin>,
    #[serde(default)]
    pub flip_axis: Option<crate::compute::stock_config::FlipAxis>,
}

impl Default for ProjectStockConfig {
    fn default() -> Self {
        Self {
            x: 100.0,
            y: 100.0,
            z: 25.0,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            padding: 5.0,
            workholding_rigidity: crate::feeds::WorkholdingRigidity::Medium,
            auto_from_model: true,
            material: crate::material::Material::default(),
            alignment_pins: Vec::new(),
            flip_axis: None,
        }
    }
}

fn default_stock_dim() -> f64 {
    100.0
}
fn default_stock_z() -> f64 {
    25.0
}
fn default_stock_padding() -> f64 {
    5.0
}
fn default_workholding_rigidity() -> crate::feeds::WorkholdingRigidity {
    crate::feeds::WorkholdingRigidity::Medium
}

/// Post-processor configuration from the project file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectPostConfig {
    #[serde(default)]
    pub format: String,
    #[serde(default = "default_spindle_speed")]
    pub spindle_speed: u32,
    #[serde(default = "default_safe_z")]
    pub safe_z: f64,
    #[serde(default)]
    pub high_feedrate_mode: bool,
    #[serde(default = "default_high_feedrate")]
    pub high_feedrate: f64,
    /// Project-level spindle policy used by the Feeds & Speeds
    /// suggest path. `MatchChart` (default) uses the vendor LUT row's
    /// `rpm_nominal` verbatim — chart fidelity. `MaxSpeed` walks the
    /// constant-chipload line up to the spindle ceiling, scaling feed
    /// proportionally. See [`crate::feeds::SpindleStrategy`].
    #[serde(default)]
    pub spindle_strategy: crate::feeds::SpindleStrategy,
}

impl Default for ProjectPostConfig {
    fn default() -> Self {
        Self {
            format: "grbl".to_owned(),
            spindle_speed: 18000,
            safe_z: 10.0,
            high_feedrate_mode: false,
            high_feedrate: 5000.0,
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        }
    }
}

fn default_spindle_speed() -> u32 {
    18000
}
fn default_safe_z() -> f64 {
    10.0
}
fn default_high_feedrate() -> f64 {
    5000.0
}

/// Tool definition in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectToolSection {
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default = "default_tool_name")]
    pub name: String,
    #[serde(rename = "type", default = "default_tool_type_str")]
    pub tool_type: String,
    #[serde(default = "default_tool_diameter")]
    pub diameter: f64,
    #[serde(default = "default_cutting_length")]
    pub cutting_length: f64,
    #[serde(default = "default_helix_deg")]
    pub helix_deg: f64,
    #[serde(default)]
    pub corner_radius_mm: f64,
    #[serde(default = "default_corner_radius")]
    pub corner_radius: f64,
    #[serde(default = "default_included_angle")]
    pub included_angle: f64,
    #[serde(default = "default_taper_half_angle")]
    pub taper_half_angle: f64,
    #[serde(default = "default_shaft_diameter")]
    pub shaft_diameter: f64,
    #[serde(default = "default_holder_diameter")]
    pub holder_diameter: f64,
    #[serde(default = "default_shank_diameter")]
    pub shank_diameter: f64,
    #[serde(default = "default_shank_length")]
    pub shank_length: f64,
    #[serde(default = "default_stickout")]
    pub stickout: f64,
    #[serde(default = "default_flute_count")]
    pub flute_count: u32,
    #[serde(default)]
    pub tool_number: Option<usize>,
    #[serde(default = "default_tool_material")]
    pub tool_material: String,
    #[serde(default = "default_cut_direction")]
    pub cut_direction: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub product_id: String,
}

fn default_tool_name() -> String {
    "Tool".to_owned()
}
fn default_tool_type_str() -> String {
    "end_mill".to_owned()
}
fn default_tool_diameter() -> f64 {
    6.35
}
fn default_cutting_length() -> f64 {
    25.0
}
fn default_helix_deg() -> f64 {
    30.0
}
fn default_corner_radius() -> f64 {
    2.0
}
fn default_included_angle() -> f64 {
    90.0
}
fn default_taper_half_angle() -> f64 {
    15.0
}
fn default_shaft_diameter() -> f64 {
    6.35
}
fn default_holder_diameter() -> f64 {
    25.0
}
fn default_shank_diameter() -> f64 {
    6.35
}
fn default_shank_length() -> f64 {
    20.0
}
fn default_stickout() -> f64 {
    45.0
}
fn default_flute_count() -> u32 {
    2
}
fn default_tool_material() -> String {
    "carbide".to_owned()
}
fn default_cut_direction() -> String {
    "up_cut".to_owned()
}

/// Model reference in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectModelSection {
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: Option<ModelKind>,
    #[serde(default)]
    pub units: Option<ModelUnits>,
}

/// Setup section in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSetupSection {
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default = "default_setup_name")]
    pub name: String,
    #[serde(default = "default_face_up")]
    pub face_up: String,
    #[serde(default)]
    pub z_rotation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_message: Option<String>,
    /// Setup datum — how the operator zeroes the machine (W9 / P-2).
    /// Key names and spellings match what the viz fallback schema
    /// already wrote (`crates/rs_cam_viz/src/io/project.rs`), so the two
    /// loaders agree instead of drifting. All three are written only
    /// when non-default, so files saved before this landed and projects
    /// that never touched the datum stay byte-identical.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub xy_datum: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub z_datum: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub datum_notes: String,
    /// Models in scope for this setup; empty = all (W9 / P-2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_ids: Vec<usize>,
    #[serde(default)]
    pub fixtures: Vec<ProjectFixtureSection>,
    #[serde(default)]
    pub keep_out_zones: Vec<ProjectKeepOutSection>,
    #[serde(default)]
    pub toolpaths: Vec<ProjectToolpathSection>,
}

/// Fixture section in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFixtureSection {
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default = "default_fixture_name")]
    pub name: String,
    #[serde(default = "default_fixture_kind")]
    pub kind: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub origin_x: f64,
    #[serde(default)]
    pub origin_y: f64,
    #[serde(default)]
    pub origin_z: f64,
    #[serde(default = "default_fixture_size_x")]
    pub size_x: f64,
    #[serde(default = "default_fixture_size_y")]
    pub size_y: f64,
    #[serde(default = "default_fixture_size_z")]
    pub size_z: f64,
    #[serde(default = "default_fixture_clearance")]
    pub clearance: f64,
}

fn default_fixture_name() -> String {
    "Fixture".to_owned()
}
fn default_fixture_kind() -> String {
    "clamp".to_owned()
}
fn default_fixture_size_x() -> f64 {
    30.0
}
fn default_fixture_size_y() -> f64 {
    15.0
}
fn default_fixture_size_z() -> f64 {
    20.0
}
fn default_fixture_clearance() -> f64 {
    3.0
}

/// Keep-out zone section in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectKeepOutSection {
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default = "default_keep_out_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub origin_x: f64,
    #[serde(default)]
    pub origin_y: f64,
    #[serde(default = "default_keep_out_size")]
    pub size_x: f64,
    #[serde(default = "default_keep_out_size")]
    pub size_y: f64,
}

fn default_keep_out_name() -> String {
    "Keep-Out".to_owned()
}
fn default_keep_out_size() -> f64 {
    20.0
}

fn default_setup_name() -> String {
    "Setup 1".to_owned()
}
fn default_face_up() -> String {
    "top".to_owned()
}

/// Toolpath section in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectToolpathSection {
    #[serde(default)]
    pub id: Option<crate::ids::ToolpathId>,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type", default)]
    pub op_type: Option<OperationType>,
    #[serde(default)]
    pub operation: Option<OperationConfig>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tool_id: Option<usize>,
    #[serde(default)]
    pub model_id: Option<usize>,
    #[serde(default)]
    pub dressups: DressupConfig,
    #[serde(default)]
    pub heights: HeightsConfig,
    /// Raw G-code to emit before this toolpath's moves.
    #[serde(default)]
    pub pre_gcode: Option<String>,
    /// Raw G-code to emit after this toolpath's moves.
    #[serde(default)]
    pub post_gcode: Option<String>,
    /// Machining boundary configuration.
    #[serde(default)]
    pub boundary: BoundaryConfig,
    /// When true, inherit boundary from stock default.
    #[serde(default = "default_true")]
    pub boundary_inherit: bool,
    /// Where this toolpath's stock material comes from.
    #[serde(default)]
    pub stock_source: StockSource,
    /// Coolant mode for G-code output.
    #[serde(default)]
    pub coolant: CoolantMode,
    /// Optional BREP face selection (raw u16 IDs).
    #[serde(default)]
    pub face_selection: Option<Vec<u16>>,
    /// Legacy field — older projects emit a `feeds_auto = {...}` block.
    /// Read and discarded; never written. Roadmap F.5 deleted the
    /// background auto-fill behaviour these flags described.
    #[serde(default, rename = "feeds_auto", skip_serializing)]
    pub _legacy_feeds_auto: Option<toml::Value>,
    /// Debug trace options.
    #[serde(default)]
    pub debug_options: ToolpathDebugOptions,
    /// Per-dimension provenance of the stored feeds values (W2.1). Absent in
    /// projects saved before provenance tracking — defaults to all-`None`.
    #[serde(default, skip_serializing_if = "feeds_provenance_is_empty")]
    pub feeds_provenance: crate::feeds::FeedsProvenance,
    /// Op-agnostic rest analysis (P2.5). Absent in projects saved before this
    /// config existed — defaults to disabled.
    #[serde(default, skip_serializing_if = "rest_analysis_is_default")]
    pub rest_analysis: crate::compute::config::RestAnalysisConfig,
}

fn rest_analysis_is_default(r: &crate::compute::config::RestAnalysisConfig) -> bool {
    *r == crate::compute::config::RestAnalysisConfig::default()
}

fn feeds_provenance_is_empty(p: &crate::feeds::FeedsProvenance) -> bool {
    *p == crate::feeds::FeedsProvenance::default()
}

fn default_true() -> bool {
    true
}

// ── Free functions: project file to session state ─────────────────────

pub(crate) fn stock_from_project(ps: &ProjectStockConfig) -> StockConfig {
    StockConfig {
        x: ps.x,
        y: ps.y,
        z: ps.z,
        origin_x: ps.origin_x,
        origin_y: ps.origin_y,
        origin_z: ps.origin_z,
        auto_from_model: ps.auto_from_model,
        padding: ps.padding,
        material: ps.material.clone(),
        alignment_pins: ps.alignment_pins.clone(),
        flip_axis: ps.flip_axis,
        workholding_rigidity: ps.workholding_rigidity,
    }
}

/// Q4 warn-and-default file-loading policy over the unified
/// [`ToolType::parse_lenient`] vocabulary (T8). Pre-T8 this had its own
/// alias table and a SILENT `_ => EndMill` — an unknown token (or a viz
/// legacy alias like `ball`) became an end mill with no trace.
pub(crate) fn parse_tool_type(s: &str) -> ToolType {
    ToolType::parse_lenient(s).unwrap_or_else(|| {
        tracing::warn!(
            tool_type = s,
            "unknown tool type in project file — defaulting to end_mill"
        );
        ToolType::EndMill
    })
}

pub(crate) fn tool_from_project_section(ts: &ProjectToolSection, idx: usize) -> ToolConfig {
    let tool_id = ts.id.unwrap_or(idx);
    let tool_number = ts.tool_number.unwrap_or(tool_id + 1) as u32;
    ToolConfig {
        id: ToolId(tool_id),
        name: ts.name.clone(),
        tool_number,
        tool_type: parse_tool_type(&ts.tool_type),
        diameter: ts.diameter,
        cutting_length: ts.cutting_length,
        helix_deg: ts.helix_deg,
        corner_radius_mm: ts.corner_radius_mm,
        corner_radius: ts.corner_radius,
        included_angle: ts.included_angle,
        taper_half_angle: ts.taper_half_angle,
        shaft_diameter: ts.shaft_diameter,
        holder_diameter: ts.holder_diameter,
        shank_diameter: ts.shank_diameter,
        shank_length: ts.shank_length,
        stickout: ts.stickout,
        flute_count: ts.flute_count,
        tool_material: parse_tool_material(&ts.tool_material),
        cut_direction: parse_cut_direction(&ts.cut_direction),
        vendor: ts.vendor.clone(),
        product_id: ts.product_id.clone(),
    }
}

fn parse_tool_material(s: &str) -> crate::compute::tool_config::ToolMaterial {
    use crate::compute::tool_config::ToolMaterial;
    match s.to_ascii_lowercase().as_str() {
        "hss" => ToolMaterial::Hss,
        _ => ToolMaterial::Carbide,
    }
}

fn parse_cut_direction(s: &str) -> crate::compute::tool_config::BitCutDirection {
    use crate::compute::tool_config::BitCutDirection;
    match s.to_ascii_lowercase().as_str() {
        "down_cut" | "downcut" => BitCutDirection::DownCut,
        "compression" => BitCutDirection::Compression,
        _ => BitCutDirection::UpCut,
    }
}

pub(crate) fn infer_model_kind(path: &Path) -> Option<ModelKind> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .and_then(|ext| match ext.to_ascii_lowercase().as_str() {
            "stl" => Some(ModelKind::Stl),
            "svg" => Some(ModelKind::Svg),
            "dxf" => Some(ModelKind::Dxf),
            "step" | "stp" => Some(ModelKind::Step),
            _ => None,
        })
}

pub(crate) fn load_model_geometry(
    model: &ProjectModelSection,
    base_dir: &Path,
) -> Result<LoadedGeometry, SessionError> {
    let raw_path = Path::new(&model.path);
    let full_path = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        base_dir.join(raw_path)
    };

    let kind = model
        .kind
        .or_else(|| infer_model_kind(&full_path))
        .ok_or_else(|| SessionError::ModelLoad {
            name: model.name.clone(),
            detail: format!("Cannot determine file type for '{}'", full_path.display()),
        })?;

    // G-UNITSRELOAD (2026-08-22). This scale must reach EVERY geometry kind,
    // not just the mesh. `ModelUnits`' own doc still says "units of the
    // imported STL" — it predates 2D import, and when the SVG/DXF arms were
    // added below they simply never consumed it, while the interactive door
    // (`io::load_model_file`) always did. A project file stores the model's
    // *path* and its declared units, not its geometry, so both doors
    // re-import the same file and must agree: an inch-authored DXF used to
    // come back 25.4x smaller after a save/reload, silently, with the stock
    // still at its saved size because `update_from_bbox` runs on import and
    // not on load. Sentried by `model_units_survive_reload_g_unitsreload.rs`,
    // which asserts the two doors agree rather than asserting a magic size.
    let scale = model
        .units
        .as_ref()
        .map(|u| u.scale_factor())
        .unwrap_or(1.0);

    match kind {
        ModelKind::Stl => {
            let mesh = TriangleMesh::from_stl_scaled(&full_path, scale).map_err(|e| {
                SessionError::ModelLoad {
                    name: model.name.clone(),
                    detail: format!("STL load failed: {e}"),
                }
            })?;
            Ok(LoadedGeometry::Mesh(mesh))
        }
        ModelKind::Dxf => {
            let import = crate::dxf_input::load_dxf_full(&full_path, 5.0).map_err(|e| {
                SessionError::ModelLoad {
                    name: model.name.clone(),
                    detail: format!("DXF load failed: {e}"),
                }
            })?;
            let mut polygons = import.polygons;
            let mut drill_targets = import.drill_targets;
            crate::io::apply_uniform_scale_2d(&mut polygons, scale);
            crate::io::apply_uniform_scale_targets(&mut drill_targets, scale);
            Ok(LoadedGeometry::Polygons(
                polygons,
                drill_targets,
                import.layers,
            ))
        }
        ModelKind::Svg => {
            let polys = crate::svg_input::load_svg(&full_path, 0.1).map_err(|e| {
                SessionError::ModelLoad {
                    name: model.name.clone(),
                    detail: format!("SVG load failed: {e}"),
                }
            })?;
            let mut polys = polys;
            crate::io::apply_uniform_scale_2d(&mut polys, scale);
            Ok(LoadedGeometry::Polygons(polys, Vec::new(), Vec::new()))
        }
        ModelKind::Step => {
            #[cfg(feature = "step")]
            {
                let enriched = crate::step_input::load_step(&full_path, 0.1).map_err(|e| {
                    SessionError::ModelLoad {
                        name: model.name.clone(),
                        detail: format!("STEP load failed: {e}"),
                    }
                })?;
                // Preserve BREP topology — the parallel `io::load_model_file`
                // loader already does this. Downgrading to a flat mesh here
                // (the prior bug) silently broke face-selective operations.
                Ok(LoadedGeometry::Enriched(enriched))
            }
            #[cfg(not(feature = "step"))]
            {
                Err(SessionError::ModelLoad {
                    name: model.name.clone(),
                    detail: "STEP support not enabled (compile with --features step)".to_owned(),
                })
            }
        }
    }
}

/// Convert a TOML toolpath section into a session `ToolpathConfig`.
fn toolpath_config_from_section(
    tp: &ProjectToolpathSection,
    tp_id: crate::ids::ToolpathId,
    operation: &OperationConfig,
) -> ToolpathConfig {
    // One-shot migration: projects saved before operation-specific dressup
    // restrictions shipped can have geometrically-invalid dressups (e.g.
    // entry_style=Ramp on a ProjectCurve with hundreds of small rings,
    // producing phantom diagonal cuts across the stock). Normalize on load
    // so the UI state and compute behaviour stay in lockstep.
    let mut dressups = tp.dressups.clone();
    let op_type = operation.op_type();
    if dressups.normalize_for_op(op_type) {
        tracing::info!(
            toolpath = %tp.name,
            op = ?op_type,
            "Normalized incompatible dressups on load"
        );
    }
    ToolpathConfig {
        id: tp_id,
        name: tp.name.clone(),
        enabled: tp.enabled,
        operation: operation.clone(),
        dressups,
        heights: tp.heights.clone(),
        tool_id: tp.tool_id.unwrap_or(0),
        model_id: tp.model_id.unwrap_or(0),
        pre_gcode: tp.pre_gcode.clone(),
        post_gcode: tp.post_gcode.clone(),
        boundary: tp.boundary.clone(),
        boundary_inherit: tp.boundary_inherit,
        stock_source: tp.stock_source,
        coolant: tp.coolant,
        face_selection: tp
            .face_selection
            .as_ref()
            .map(|ids| ids.iter().copied().map(FaceGroupId).collect()),
        debug_options: tp.debug_options,
        feeds_provenance: tp.feeds_provenance.clone(),
        rest_analysis: tp.rest_analysis.clone(),
    }
}

/// Read the setup datum off a TOML setup section (W9 / P-2).
///
/// An **absent or empty** key keeps the type default rather than being
/// handed to `from_key`. The two are equivalent today — both `from_key`
/// implementations fall through to the default — but writing the guard
/// makes "the file said nothing" and "the file said something we don't
/// recognise" distinguishable if either fall-through ever changes.
fn datum_from_section(section: &ProjectSetupSection) -> DatumConfig {
    DatumConfig {
        xy_method: if section.xy_datum.is_empty() {
            XYDatum::default()
        } else {
            XYDatum::from_key(&section.xy_datum)
        },
        z_method: if section.z_datum.is_empty() {
            ZDatum::default()
        } else {
            ZDatum::from_key(&section.z_datum)
        },
        notes: section.datum_notes.clone(),
    }
}

/// Convert TOML fixture sections into session `Fixture` values.
fn build_fixtures(sections: &[ProjectFixtureSection]) -> Vec<Fixture> {
    sections
        .iter()
        .enumerate()
        .map(|(idx, fs)| Fixture {
            id: FixtureId(fs.id.unwrap_or(idx)),
            name: fs.name.clone(),
            kind: FixtureKind::from_key(&fs.kind),
            enabled: fs.enabled,
            origin_x: fs.origin_x,
            origin_y: fs.origin_y,
            origin_z: fs.origin_z,
            size_x: fs.size_x,
            size_y: fs.size_y,
            size_z: fs.size_z,
            clearance: fs.clearance,
        })
        .collect()
}

/// Convert TOML keep-out sections into session `KeepOutZone` values.
fn build_keep_out_zones(sections: &[ProjectKeepOutSection]) -> Vec<KeepOutZone> {
    sections
        .iter()
        .enumerate()
        .map(|(idx, ks)| KeepOutZone {
            id: KeepOutId(ks.id.unwrap_or(idx)),
            name: ks.name.clone(),
            enabled: ks.enabled,
            origin_x: ks.origin_x,
            origin_y: ks.origin_y,
            size_x: ks.size_x,
            size_y: ks.size_y,
        })
        .collect()
}

/// Reject TOML that parses but contains no recognizable CAM content. Without
/// this guard, `serde(default)` on every top-level field happily produces an
/// empty session from any well-formed TOML file (or a CSV opened by mistake),
/// and the user just sees an empty workspace with no error.
pub(super) fn validate_looks_like_cam_project(
    project: &ProjectFile,
    path: Option<&Path>,
) -> Result<(), SessionError> {
    // `[job]` field has #[serde(default)], so an absent table yields
    // ProjectJobSection::default() with an empty name; an empty `[job]` table
    // yields the field-level default (`default_job_name()`). Treat both as
    // "user did not supply a job name".
    let job_is_default = project.job.name.is_empty() || project.job.name == default_job_name();
    let nothing_loaded = project.setups.is_empty()
        && project.toolpaths.is_empty()
        && project.models.is_empty()
        && project.tools.is_empty();
    if job_is_default && nothing_loaded {
        let detail = match path {
            Some(p) => format!(
                "file '{}' does not look like an rs_cam project (no [job], setups, toolpaths, models, or tools)",
                p.display()
            ),
            None => "file does not look like an rs_cam project (no [job], setups, toolpaths, models, or tools)"
                .to_owned(),
        };
        return Err(SessionError::NotACamProject(detail));
    }
    Ok(())
}

/// Build a [`ProjectSession`](super::ProjectSession) from a parsed [`ProjectFile`].
///
/// This is the core of `ProjectSession::from_project_file` but lives here so
/// that all TOML-to-session conversion logic is co-located.
pub(super) fn build_session_from_project(
    project: ProjectFile,
    base_dir: &Path,
) -> Result<super::ProjectSession, SessionError> {
    validate_looks_like_cam_project(&project, None)?;

    let mut stock = stock_from_project(&project.job.stock);

    // Load tools
    let tools: Vec<ToolConfig> = project
        .tools
        .iter()
        .enumerate()
        .map(|(idx, ts)| tool_from_project_section(ts, idx))
        .collect();

    // Load models
    let mut models = Vec::new();
    for (idx, model_section) in project.models.iter().enumerate() {
        let model_id = model_section.id.unwrap_or(idx);
        let model_path = std::path::PathBuf::from(&model_section.path);
        let model_kind = model_section.kind.or_else(|| infer_model_kind(&model_path));
        let model_units = model_section.units;

        match load_model_geometry(model_section, base_dir) {
            Ok(LoadedGeometry::Mesh(mesh)) => {
                tracing::info!(
                    name = %model_section.name,
                    tris = mesh.triangles.len(),
                    "Loaded mesh model"
                );
                models.push(LoadedModel {
                    id: model_id,
                    name: model_section.name.clone(),
                    mesh: Some(Arc::new(mesh)),
                    polygons: None,
                    drill_targets: Arc::new(Vec::new()),
                    layers: Arc::new(Vec::new()),
                    path: model_path,
                    kind: model_kind,
                    units: model_units,
                    enriched_mesh: None,
                    winding_report: None,
                    load_error: None,
                });
            }
            Ok(LoadedGeometry::Polygons(polys, drill_targets, layers)) => {
                tracing::info!(
                    name = %model_section.name,
                    polygons = polys.len(),
                    drill_targets = drill_targets.len(),
                    "Loaded 2D model"
                );
                models.push(LoadedModel {
                    id: model_id,
                    name: model_section.name.clone(),
                    mesh: None,
                    polygons: Some(Arc::new(polys)),
                    drill_targets: Arc::new(drill_targets),
                    layers: Arc::new(layers),
                    path: model_path,
                    kind: model_kind,
                    units: model_units,
                    enriched_mesh: None,
                    winding_report: None,
                    load_error: None,
                });
            }
            Ok(LoadedGeometry::Enriched(enriched)) => {
                tracing::info!(
                    name = %model_section.name,
                    tris = enriched.mesh.triangles.len(),
                    faces = enriched.face_count(),
                    "Loaded enriched (BREP) model"
                );
                let mesh_arc = Arc::clone(&enriched.mesh);
                models.push(LoadedModel {
                    id: model_id,
                    name: model_section.name.clone(),
                    mesh: Some(mesh_arc),
                    polygons: None,
                    drill_targets: Arc::new(Vec::new()),
                    layers: Arc::new(Vec::new()),
                    path: model_path,
                    kind: model_kind,
                    units: model_units,
                    enriched_mesh: Some(Arc::new(enriched)),
                    winding_report: None,
                    load_error: None,
                });
            }
            Err(e) => {
                tracing::warn!(
                    name = %model_section.name,
                    error = %e,
                    "Failed to load model, skipping"
                );
                models.push(LoadedModel {
                    id: model_id,
                    name: model_section.name.clone(),
                    mesh: None,
                    polygons: None,
                    drill_targets: Arc::new(Vec::new()),
                    layers: Arc::new(Vec::new()),
                    path: model_path,
                    kind: model_kind,
                    units: model_units,
                    enriched_mesh: None,
                    winding_report: None,
                    load_error: Some(e.to_string()),
                });
            }
        }
    }

    // F-026 (2026-05-25): when `stock.auto_from_model = true`, the stock
    // dimensions on disk may be stale relative to the current model
    // bbox — the user edited a model externally, or the saved file
    // pre-dates a mesh edit. The MCP `import_model` path already re-
    // runs `update_from_bbox` via `add_model`, but the load path was
    // using the raw TOML values unchanged. For `auto_from_model` projects
    // where the model has grown above `stock.z` (or shifted in XY), this
    // produced a stock bbox that didn't enclose the model — the
    // adaptive3d planner warned about it ("stock_top_z is below mesh
    // top"), but the simulator's per-setup dexel grid then sat at the
    // stale Z height while the toolpath cuts ran in the model's frame.
    // 3D ops like adaptive3d / scallop walk the whole model surface,
    // and Cut moves into uncleared cells produced full-stock-height
    // pre-stamp rays that the simulator interpreted as 25-50 mm axial
    // engagement (single-stamp clearing of virgin stock instead of the
    // commanded depth_per_pass). Rapids cleared at adaptive3d's
    // expected safe_z then registered as collisions against the stale
    // grid Z range.
    //
    // Fix: at load time, if `auto_from_model = true` and any loaded
    // model has a finite bbox, re-derive stock from the union of all
    // model bboxes (mirrors what `add_model` does at runtime). This
    // restores the documented invariant — `auto_from_model = true`
    // means the stock bounds track the model bbox at the time the
    // session is in memory, regardless of whether the dimensions on
    // disk match.
    if stock.auto_from_model {
        let mut combined: Option<crate::geo::BoundingBox3> = None;
        for model in &models {
            let model_bbox = model.mesh.as_ref().map(|mesh| mesh.bbox).or_else(|| {
                model
                    .polygons
                    .as_ref()
                    .and_then(|polys| crate::session::mutation::polygons_bbox(polys))
            });
            if let Some(bb) = model_bbox {
                combined = Some(match combined {
                    None => bb,
                    Some(existing) => crate::geo::BoundingBox3 {
                        min: crate::geo::P3::new(
                            existing.min.x.min(bb.min.x),
                            existing.min.y.min(bb.min.y),
                            existing.min.z.min(bb.min.z),
                        ),
                        max: crate::geo::P3::new(
                            existing.max.x.max(bb.max.x),
                            existing.max.y.max(bb.max.y),
                            existing.max.z.max(bb.max.z),
                        ),
                    },
                });
            }
        }
        if let Some(bb) = combined {
            stock.update_from_bbox(&bb);
        }
    }

    // Collect toolpaths from setups
    let mut setups = Vec::new();
    let mut toolpath_configs = Vec::new();

    if !project.setups.is_empty() {
        for (setup_idx, setup_section) in project.setups.iter().enumerate() {
            let setup_id = setup_section.id.unwrap_or(setup_idx);
            let face_up = FaceUp::from_key(&setup_section.face_up);
            let z_rotation = ZRotation::from_key(&setup_section.z_rotation);
            let mut tp_indices = Vec::new();

            for tp_section in &setup_section.toolpaths {
                let tp_idx = toolpath_configs.len();
                let tp_id = tp_section.id.unwrap_or(crate::ids::ToolpathId(tp_idx));
                let operation = match &tp_section.operation {
                    Some(op) => op.clone(),
                    None => {
                        let op_type = tp_section.op_type.unwrap_or(OperationType::Pocket);
                        tracing::warn!(
                            toolpath_id = tp_id.0,
                            ?op_type,
                            "loaded toolpath with default operation; TOML was missing [setups.toolpaths.operation]"
                        );
                        OperationConfig::new_default(op_type)
                    }
                };
                toolpath_configs.push(toolpath_config_from_section(tp_section, tp_id, &operation));
                tp_indices.push(tp_idx);
            }

            let fixtures = build_fixtures(&setup_section.fixtures);
            let keep_out_zones = build_keep_out_zones(&setup_section.keep_out_zones);

            setups.push(SetupData {
                id: setup_id,
                name: setup_section.name.clone(),
                face_up,
                z_rotation,
                datum: datum_from_section(setup_section),
                model_ids: setup_section
                    .model_ids
                    .iter()
                    .map(|&id| crate::compute::stock_config::ModelId(id))
                    .collect(),
                fixtures,
                keep_out_zones,
                toolpath_indices: tp_indices,
                pause_message: setup_section.pause_message.clone(),
            });
        }
    } else {
        // Legacy: top-level toolpaths -> single default setup
        let mut tp_indices = Vec::new();
        for tp_section in &project.toolpaths {
            let tp_idx = toolpath_configs.len();
            let tp_id = tp_section.id.unwrap_or(crate::ids::ToolpathId(tp_idx));
            let operation = match &tp_section.operation {
                Some(op) => op.clone(),
                None => {
                    let op_type = tp_section.op_type.unwrap_or(OperationType::Pocket);
                    tracing::warn!(
                        toolpath_id = tp_id.0,
                        ?op_type,
                        "loaded toolpath with default operation; TOML was missing [setups.toolpaths.operation]"
                    );
                    OperationConfig::new_default(op_type)
                }
            };
            toolpath_configs.push(toolpath_config_from_section(tp_section, tp_id, &operation));
            tp_indices.push(tp_idx);
        }
        if !tp_indices.is_empty() {
            setups.push(SetupData {
                id: 0,
                name: "Default".to_owned(),
                face_up: FaceUp::Top,
                z_rotation: ZRotation::default(),
                datum: DatumConfig::default(),
                model_ids: Vec::new(),
                fixtures: Vec::new(),
                keep_out_zones: Vec::new(),
                toolpath_indices: tp_indices,
                pause_message: None,
            });
        }
    }

    // Compute next IDs by scanning existing maximums
    let next_toolpath_id = toolpath_configs
        .iter()
        .map(|tc| tc.id.0)
        .max()
        .map_or(0, |m| m + 1);
    let next_tool_id = tools.iter().map(|t| t.id.0).max().map_or(0, |m| m + 1);
    let next_setup_id = setups.iter().map(|s| s.id).max().map_or(0, |m| m + 1);
    let next_model_id = models.iter().map(|m| m.id).max().map_or(0, |m| m + 1);

    // Machines use snapshot semantics (like `[[tools]]`): the inline
    // `[job.machine]` copy is authoritative. A legacy `machine_ref` is no
    // longer a live link — it's dropped on load (the inline machine is
    // migrated forward as-is). Re-save to persist the snapshot.
    if let Some(name) = &project.job.machine_ref {
        tracing::info!(
            "project '{}': legacy machine_ref {name:?} dropped — machines are now stored \
             inline (snapshot model). Re-save to clear it from the file.",
            project.job.name
        );
    }

    Ok(super::ProjectSession {
        name: project.job.name.clone(),
        stock,
        post: project.job.post,
        machine: project.job.machine,
        machine_ref: None,
        models,
        tools,
        setups,
        toolpath_configs,
        results: std::collections::HashMap::new(),
        simulation: None,
        wizard: super::WizardState::default(),
        next_toolpath_id,
        next_tool_id,
        next_setup_id,
        next_model_id,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Parity freeze (architectural refactor §7.2): the CORE project-file
    /// tool section serializes its tool type under the TOML key `type`
    /// (serde rename), holds it as a lenient **String** (unknown values
    /// are accepted here and resolved later by the lenient parser), and
    /// defaults a missing key to `"end_mill"`. The viz loader has a
    /// parallel `ProjectToolSection` with the same rename but a
    /// serde-direct `ToolType` — a registry/X-macro change must not
    /// silently alter either shape.
    #[test]
    fn project_tool_section_type_key_shape_frozen() {
        // `type` key round-trips into the String field.
        let section: ProjectToolSection =
            toml::from_str(r#"type = "ball_nose""#).expect("parse tool section");
        assert_eq!(section.tool_type, "ball_nose");

        // Lenient layer: unknown tool types are accepted as-is here
        // (resolution to ToolType happens later, warn-and-default).
        let weird: ProjectToolSection =
            toml::from_str(r#"type = "definitely_not_a_tool""#).expect("lenient parse");
        assert_eq!(weird.tool_type, "definitely_not_a_tool");

        // Missing key defaults to end_mill.
        let defaulted: ProjectToolSection = toml::from_str("").expect("parse empty section");
        assert_eq!(defaulted.tool_type, "end_mill");

        // Serialization emits `type`, never the field name `tool_type`.
        let out = toml::to_string(&section).expect("serialize tool section");
        assert!(
            out.contains("type = \"ball_nose\""),
            "expected renamed `type` key, got:\n{out}"
        );
        assert!(
            !out.contains("tool_type"),
            "field name `tool_type` must not leak into TOML:\n{out}"
        );
    }
}
