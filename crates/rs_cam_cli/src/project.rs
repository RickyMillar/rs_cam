//! GUI project file (format_version=3) parser and diagnostic executor.
//!
//! Loads the same TOML format that the GUI uses, executes every enabled
//! toolpath through the core algorithms with full debug + semantic tracing,
//! runs tri-dexel simulation with cut metrics, checks collisions, and
//! writes structured JSON diagnostics.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info, warn};

use rs_cam_core::{
    adaptive::{AdaptiveParams, adaptive_toolpath},
    adaptive3d::{
        Adaptive3dParams, ClearingStrategy3d, EntryStyle3d, RegionOrdering,
        adaptive_3d_toolpath_annotated_traced_with_cancel,
    },
    arcfit::fit_arcs,
    chamfer::{ChamferParams, chamfer_toolpath},
    collision::{CollisionReport, check_collisions_interpolated, check_rapid_collisions},
    debug_trace::{ToolpathDebugRecorder, ToolpathDebugTrace},
    depth::{DepthStepping, depth_stepped_toolpath},
    dexel_stock::{StockCutDirection, TriDexelStock},
    dressup::{
        EntryStyle, LinkMoveParams, apply_dogbones, apply_entry, apply_lead_in_out,
        apply_link_moves,
    },
    drill::{DrillCycle, DrillParams, drill_toolpath},
    dropcutter::batch_drop_cutter_with_cancel,
    face::{FaceDirection, face_toolpath},
    geo::{BoundingBox3, P3},
    horizontal_finish::{HorizontalFinishParams, horizontal_finish_toolpath},
    inlay::{InlayParams, inlay_toolpaths},
    mesh::{SpatialIndex, TriangleMesh},
    pencil::{PencilParams, pencil_toolpath},
    pocket::{PocketParams, pocket_toolpath},
    profile::{ProfileParams, ProfileSide, profile_toolpath},
    project_curve::{ProjectCurveParams, project_curve_toolpath},
    radial_finish::{RadialFinishParams, radial_finish_toolpath},
    radial_profile::RadialProfileLUT,
    ramp_finish::{CutDirection, RampFinishParams, ramp_finish_toolpath},
    rest::{RestParams, rest_machining_toolpath},
    scallop::{ScallopDirection, ScallopParams, scallop_toolpath},
    semantic_trace::{ToolpathSemanticRecorder, ToolpathSemanticTrace, enrich_traces},
    simulation_cut::{SimulationCutArtifact, SimulationCutTrace},
    spiral_finish::{SpiralDirection, SpiralFinishParams, spiral_finish_toolpath},
    steep_shallow::{SteepShallowParams, steep_shallow_toolpath},
    tool::{
        BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill,
        ToolDefinition, VBitEndmill,
    },
    toolpath::{Toolpath, raster_toolpath_from_grid},
    trace::{TraceCompensation, TraceParams, trace_toolpath},
    tsp::optimize_rapid_order,
    vcarve::{VCarveParams, vcarve_toolpath},
    waterline::{WaterlineParams, waterline_toolpath_with_cancel},
    zigzag::{ZigzagParams, zigzag_toolpath},
};

// ── ID newtypes (mirrors rs_cam_viz/src/state/job.rs) ──────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct ToolId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct ModelId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct SetupId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct ToolpathId(pub usize);

// ── Project TOML serde types ────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProjectJobSection {
    #[serde(default = "default_job_name")]
    pub name: String,
    #[serde(default)]
    pub stock: StockConfig,
    #[serde(default)]
    pub post: PostConfig,
    #[serde(default)]
    pub machine: rs_cam_core::machine::MachineProfile,
}

fn default_job_name() -> String {
    "Untitled".to_owned()
}

// ── Stock ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StockConfig {
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
    #[serde(default)]
    pub material: rs_cam_core::material::Material,
}

impl Default for StockConfig {
    fn default() -> Self {
        Self {
            x: 100.0,
            y: 100.0,
            z: 25.0,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            material: rs_cam_core::material::Material::default(),
        }
    }
}

fn default_stock_dim() -> f64 {
    100.0
}
fn default_stock_z() -> f64 {
    25.0
}

// ── Post ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct PostConfig {
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
}

impl Default for PostConfig {
    fn default() -> Self {
        Self {
            format: "grbl".to_owned(),
            spindle_speed: 18000,
            safe_z: 10.0,
            high_feedrate_mode: false,
            high_feedrate: 5000.0,
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

// ── Tool ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    #[default]
    EndMill,
    BallNose,
    BullNose,
    VBit,
    TaperedBallNose,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectToolSection {
    #[serde(default)]
    pub id: Option<ToolId>,
    #[serde(default = "default_tool_name")]
    pub name: String,
    #[serde(rename = "type", default)]
    pub tool_type: ToolType,
    #[serde(default = "default_tool_diameter")]
    pub diameter: f64,
    #[serde(default = "default_cutting_length")]
    pub cutting_length: f64,
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
    // Skip GUI-only fields: tool_number, tool_material, cut_direction, vendor, product_id
}

fn default_tool_name() -> String {
    "Tool".to_owned()
}
fn default_tool_diameter() -> f64 {
    6.35
}
fn default_cutting_length() -> f64 {
    25.0
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

// ── Model ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Stl,
    Svg,
    Dxf,
    Step,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", content = "scale", rename_all = "snake_case")]
pub enum ModelUnits {
    Millimeters,
    Inches,
    Centimeters,
    Meters,
    Custom(f64),
}

impl ModelUnits {
    fn scale_factor(self) -> f64 {
        match self {
            Self::Millimeters => 1.0,
            Self::Inches => 25.4,
            Self::Centimeters => 10.0,
            Self::Meters => 1000.0,
            Self::Custom(s) => s,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectModelSection {
    #[serde(default)]
    pub id: Option<ModelId>,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: Option<ModelKind>,
    #[serde(default)]
    pub units: Option<ModelUnits>,
}

// ── Setup ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSetupSection {
    #[serde(default)]
    pub id: Option<SetupId>,
    #[serde(default = "default_setup_name")]
    pub name: String,
    #[serde(default = "default_face_up")]
    pub face_up: String,
    #[serde(default)]
    pub toolpaths: Vec<ProjectToolpathSection>,
}

fn default_face_up() -> String {
    "top".to_owned()
}

fn default_setup_name() -> String {
    "Setup 1".to_owned()
}

// ── Height system ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeightReference {
    StockTop,
    StockBottom,
    ModelTop,
    ModelBottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct ReferenceOffset {
    pub reference: HeightReference,
    pub offset: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum HeightMode {
    #[default]
    Auto,
    Manual(f64),
    FromReference(ReferenceOffset),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeightsConfig {
    #[serde(default)]
    pub clearance_z: HeightMode,
    #[serde(default)]
    pub retract_z: HeightMode,
    #[serde(default)]
    pub feed_z: HeightMode,
    #[serde(default)]
    pub top_z: HeightMode,
    #[serde(default)]
    pub bottom_z: HeightMode,
}

impl Default for HeightsConfig {
    fn default() -> Self {
        Self {
            clearance_z: HeightMode::Auto,
            retract_z: HeightMode::Auto,
            feed_z: HeightMode::Auto,
            top_z: HeightMode::Auto,
            bottom_z: HeightMode::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ResolvedHeights {
    clearance_z: f64,
    retract_z: f64,
    feed_z: f64,
    top_z: f64,
    bottom_z: f64,
}

impl ResolvedHeights {
    fn depth(&self) -> f64 {
        (self.top_z - self.bottom_z).abs()
    }
}

struct HeightContext {
    safe_z: f64,
    stock_top_z: f64,
    stock_bottom_z: f64,
    model_top_z: Option<f64>,
    model_bottom_z: Option<f64>,
}

impl HeightMode {
    fn resolve(&self, auto_value: f64, ctx: &HeightContext) -> f64 {
        match self {
            HeightMode::Auto => auto_value,
            HeightMode::Manual(v) => *v,
            HeightMode::FromReference(r) => {
                let base = match r.reference {
                    HeightReference::StockTop => ctx.stock_top_z,
                    HeightReference::StockBottom => ctx.stock_bottom_z,
                    HeightReference::ModelTop => ctx.model_top_z.unwrap_or(ctx.stock_top_z),
                    HeightReference::ModelBottom => {
                        ctx.model_bottom_z.unwrap_or(ctx.stock_bottom_z)
                    }
                };
                base + r.offset
            }
        }
    }
}

fn resolve_heights(cfg: &HeightsConfig, ctx: &HeightContext) -> ResolvedHeights {
    let retract = cfg.retract_z.resolve(ctx.stock_top_z + ctx.safe_z, ctx);
    let top = ctx.stock_top_z;
    let feed_ideal = top + 2.0;
    let feed_default = if feed_ideal < retract {
        feed_ideal
    } else {
        (top + retract) / 2.0
    };
    ResolvedHeights {
        clearance_z: cfg.clearance_z.resolve(retract + 10.0, ctx),
        retract_z: retract,
        feed_z: cfg.feed_z.resolve(feed_default, ctx),
        top_z: cfg.top_z.resolve(ctx.stock_top_z, ctx),
        bottom_z: cfg.bottom_z.resolve(ctx.stock_bottom_z, ctx),
    }
}

// ── Dressup config ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DressupEntryStyle {
    #[default]
    None,
    Ramp,
    Helix,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DressupConfig {
    #[serde(default)]
    pub entry_style: DressupEntryStyle,
    #[serde(default = "default_ramp_angle")]
    pub ramp_angle: f64,
    #[serde(default = "default_helix_radius")]
    pub helix_radius: f64,
    #[serde(default = "default_helix_pitch")]
    pub helix_pitch: f64,
    #[serde(default)]
    pub dogbone: bool,
    #[serde(default = "default_dogbone_angle")]
    pub dogbone_angle: f64,
    #[serde(default)]
    pub lead_in_out: bool,
    #[serde(default = "default_lead_radius")]
    pub lead_radius: f64,
    #[serde(default)]
    pub link_moves: bool,
    #[serde(default = "default_link_max_distance")]
    pub link_max_distance: f64,
    #[serde(default = "default_link_feed_rate")]
    pub link_feed_rate: f64,
    #[serde(default)]
    pub arc_fitting: bool,
    #[serde(default = "default_arc_tolerance")]
    pub arc_tolerance: f64,
    #[serde(default)]
    pub optimize_rapid_order: bool,
    // feed_optimization skipped (Nice-to-Have)
}

impl Default for DressupConfig {
    fn default() -> Self {
        Self {
            entry_style: DressupEntryStyle::None,
            ramp_angle: 3.0,
            helix_radius: 2.0,
            helix_pitch: 1.0,
            dogbone: false,
            dogbone_angle: 90.0,
            lead_in_out: false,
            lead_radius: 2.0,
            link_moves: false,
            link_max_distance: 10.0,
            link_feed_rate: 500.0,
            arc_fitting: false,
            arc_tolerance: 0.05,
            optimize_rapid_order: false,
        }
    }
}

fn default_ramp_angle() -> f64 {
    3.0
}
fn default_helix_radius() -> f64 {
    2.0
}
fn default_helix_pitch() -> f64 {
    1.0
}
fn default_dogbone_angle() -> f64 {
    90.0
}
fn default_lead_radius() -> f64 {
    2.0
}
fn default_link_max_distance() -> f64 {
    10.0
}
fn default_link_feed_rate() -> f64 {
    500.0
}
fn default_arc_tolerance() -> f64 {
    0.05
}

// ── Operation configs ───────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", content = "params", rename_all = "snake_case")]
pub enum OperationConfig {
    Face(FaceOpConfig),
    Pocket(PocketOpConfig),
    Profile(ProfileOpConfig),
    Adaptive(AdaptiveOpConfig),
    VCarve(VCarveOpConfig),
    Rest(RestOpConfig),
    Inlay(InlayOpConfig),
    Zigzag(ZigzagOpConfig),
    Trace(TraceOpConfig),
    Drill(DrillOpConfig),
    Chamfer(ChamferOpConfig),
    DropCutter(DropCutterOpConfig),
    Adaptive3d(Adaptive3dOpConfig),
    Waterline(WaterlineOpConfig),
    Pencil(PencilOpConfig),
    Scallop(ScallopOpConfig),
    SteepShallow(SteepShallowOpConfig),
    RampFinish(RampFinishOpConfig),
    SpiralFinish(SpiralFinishOpConfig),
    RadialFinish(RadialFinishOpConfig),
    HorizontalFinish(HorizontalFinishOpConfig),
    ProjectCurve(ProjectCurveOpConfig),
    AlignmentPinDrill(AlignmentPinDrillOpConfig),
}

impl OperationConfig {
    fn label(&self) -> &'static str {
        match self {
            Self::Face(_) => "Face",
            Self::Pocket(_) => "Pocket",
            Self::Profile(_) => "Profile",
            Self::Adaptive(_) => "Adaptive",
            Self::VCarve(_) => "V-Carve",
            Self::Rest(_) => "Rest",
            Self::Inlay(_) => "Inlay",
            Self::Zigzag(_) => "Zigzag",
            Self::Trace(_) => "Trace",
            Self::Drill(_) => "Drill",
            Self::Chamfer(_) => "Chamfer",
            Self::DropCutter(_) => "Drop Cutter",
            Self::Adaptive3d(_) => "Adaptive 3D",
            Self::Waterline(_) => "Waterline",
            Self::Pencil(_) => "Pencil",
            Self::Scallop(_) => "Scallop",
            Self::SteepShallow(_) => "Steep/Shallow",
            Self::RampFinish(_) => "Ramp Finish",
            Self::SpiralFinish(_) => "Spiral Finish",
            Self::RadialFinish(_) => "Radial Finish",
            Self::HorizontalFinish(_) => "Horizontal Finish",
            Self::ProjectCurve(_) => "Project Curve",
            Self::AlignmentPinDrill(_) => "Pin Drill",
        }
    }

    fn kind_str(&self) -> &'static str {
        match self {
            Self::Face(_) => "face",
            Self::Pocket(_) => "pocket",
            Self::Profile(_) => "profile",
            Self::Adaptive(_) => "adaptive",
            Self::VCarve(_) => "v_carve",
            Self::Rest(_) => "rest",
            Self::Inlay(_) => "inlay",
            Self::Zigzag(_) => "zigzag",
            Self::Trace(_) => "trace",
            Self::Drill(_) => "drill",
            Self::Chamfer(_) => "chamfer",
            Self::DropCutter(_) => "drop_cutter",
            Self::Adaptive3d(_) => "adaptive3d",
            Self::Waterline(_) => "waterline",
            Self::Pencil(_) => "pencil",
            Self::Scallop(_) => "scallop",
            Self::SteepShallow(_) => "steep_shallow",
            Self::RampFinish(_) => "ramp_finish",
            Self::SpiralFinish(_) => "spiral_finish",
            Self::RadialFinish(_) => "radial_finish",
            Self::HorizontalFinish(_) => "horizontal_finish",
            Self::ProjectCurve(_) => "project_curve",
            Self::AlignmentPinDrill(_) => "alignment_pin_drill",
        }
    }

    /// Whether this operation needs a 3D mesh from the primary model.
    /// Note: ProjectCurve gets its mesh from `surface_model_id`, not primary model.
    fn needs_mesh(&self) -> bool {
        matches!(
            self,
            Self::DropCutter(_)
                | Self::Adaptive3d(_)
                | Self::Waterline(_)
                | Self::Pencil(_)
                | Self::Scallop(_)
                | Self::SteepShallow(_)
                | Self::RampFinish(_)
                | Self::SpiralFinish(_)
                | Self::RadialFinish(_)
                | Self::HorizontalFinish(_)
        )
    }

    /// Whether this operation needs 2D polygons.
    fn needs_polygons(&self) -> bool {
        matches!(
            self,
            Self::Face(_)
                | Self::Pocket(_)
                | Self::Profile(_)
                | Self::Adaptive(_)
                | Self::VCarve(_)
                | Self::Rest(_)
                | Self::Inlay(_)
                | Self::Zigzag(_)
                | Self::Trace(_)
                | Self::Chamfer(_)
        )
    }

    fn feed_rate(&self) -> f64 {
        match self {
            Self::Face(c) => c.feed_rate,
            Self::Pocket(c) => c.feed_rate,
            Self::Profile(c) => c.feed_rate,
            Self::Adaptive(c) => c.feed_rate,
            Self::VCarve(c) => c.feed_rate,
            Self::Rest(c) => c.feed_rate,
            Self::Inlay(c) => c.feed_rate,
            Self::Zigzag(c) => c.feed_rate,
            Self::Trace(c) => c.feed_rate,
            Self::Drill(c) => c.feed_rate,
            Self::Chamfer(c) => c.feed_rate,
            Self::DropCutter(c) => c.feed_rate,
            Self::Adaptive3d(c) => c.feed_rate,
            Self::Waterline(c) => c.feed_rate,
            Self::Pencil(c) => c.feed_rate,
            Self::Scallop(c) => c.feed_rate,
            Self::SteepShallow(c) => c.feed_rate,
            Self::RampFinish(c) => c.feed_rate,
            Self::SpiralFinish(c) => c.feed_rate,
            Self::RadialFinish(c) => c.feed_rate,
            Self::HorizontalFinish(c) => c.feed_rate,
            Self::ProjectCurve(c) => c.feed_rate,
            Self::AlignmentPinDrill(c) => c.feed_rate,
        }
    }
}

// -- Individual operation config structs --

#[derive(Debug, Clone, Deserialize)]
pub struct FaceOpConfig {
    #[serde(default = "d5")]
    pub stepover: f64,
    #[serde(default)]
    pub depth: f64,
    #[serde(default = "d1")]
    pub depth_per_pass: f64,
    #[serde(default = "d1500")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default = "d5")]
    pub stock_offset: f64,
    #[serde(default = "default_face_direction")]
    pub direction: FaceDirection,
}

fn default_face_direction() -> FaceDirection {
    FaceDirection::Zigzag
}

#[derive(Debug, Clone, Deserialize)]
pub struct PocketOpConfig {
    #[serde(default = "d2")]
    pub stepover: f64,
    #[serde(default = "d3")]
    pub depth: f64,
    #[serde(default = "d1_5")]
    pub depth_per_pass: f64,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default = "default_true")]
    pub climb: bool,
    #[serde(default = "default_pocket_pattern")]
    pub pattern: PocketPattern,
    #[serde(default)]
    pub angle: f64,
    #[serde(default)]
    pub finishing_passes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PocketPattern {
    Contour,
    Zigzag,
}

fn default_pocket_pattern() -> PocketPattern {
    PocketPattern::Contour
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfileOpConfig {
    #[serde(default = "default_profile_side")]
    pub side: ProfileSide,
    #[serde(default = "d6")]
    pub depth: f64,
    #[serde(default = "d2")]
    pub depth_per_pass: f64,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default = "default_true")]
    pub climb: bool,
    #[serde(default)]
    pub tab_count: usize,
    #[serde(default = "d6")]
    pub tab_width: f64,
    #[serde(default = "d2")]
    pub tab_height: f64,
    #[serde(default)]
    pub finishing_passes: usize,
}

fn default_profile_side() -> ProfileSide {
    ProfileSide::Outside
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdaptiveOpConfig {
    #[serde(default = "d2")]
    pub stepover: f64,
    #[serde(default = "d6")]
    pub depth: f64,
    #[serde(default = "d2")]
    pub depth_per_pass: f64,
    #[serde(default = "d1500")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default = "d0_1")]
    pub tolerance: f64,
    #[serde(default = "default_true")]
    pub slot_clearing: bool,
    #[serde(default)]
    pub min_cutting_radius: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VCarveOpConfig {
    #[serde(default = "d5")]
    pub max_depth: f64,
    #[serde(default = "d0_5")]
    pub stepover: f64,
    #[serde(default = "d800")]
    pub feed_rate: f64,
    #[serde(default = "d400")]
    pub plunge_rate: f64,
    #[serde(default = "d0_05")]
    pub tolerance: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RestOpConfig {
    #[serde(default)]
    pub prev_tool_id: Option<ToolId>,
    #[serde(default = "d1")]
    pub stepover: f64,
    #[serde(default = "d6")]
    pub depth: f64,
    #[serde(default = "d2")]
    pub depth_per_pass: f64,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default)]
    pub angle: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InlayOpConfig {
    #[serde(default = "d3")]
    pub pocket_depth: f64,
    #[serde(default = "d0_1")]
    pub glue_gap: f64,
    #[serde(default = "d0_5")]
    pub flat_depth: f64,
    #[serde(default)]
    pub boundary_offset: f64,
    #[serde(default = "d1")]
    pub stepover: f64,
    #[serde(default)]
    pub flat_tool_radius: f64,
    #[serde(default = "d800")]
    pub feed_rate: f64,
    #[serde(default = "d400")]
    pub plunge_rate: f64,
    #[serde(default = "d0_05")]
    pub tolerance: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ZigzagOpConfig {
    #[serde(default = "d2")]
    pub stepover: f64,
    #[serde(default = "d3")]
    pub depth: f64,
    #[serde(default = "d1_5")]
    pub depth_per_pass: f64,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default)]
    pub angle: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TraceOpConfig {
    #[serde(default = "d1")]
    pub depth: f64,
    #[serde(default = "d0_5")]
    pub depth_per_pass: f64,
    #[serde(default = "d800")]
    pub feed_rate: f64,
    #[serde(default = "d400")]
    pub plunge_rate: f64,
    #[serde(default = "default_trace_compensation")]
    pub compensation: TraceCompensation,
}

fn default_trace_compensation() -> TraceCompensation {
    TraceCompensation::None
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliDrillCycleType {
    Simple,
    Dwell,
    #[default]
    Peck,
    ChipBreak,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DrillOpConfig {
    #[serde(default = "d10")]
    pub depth: f64,
    #[serde(default)]
    pub cycle: CliDrillCycleType,
    #[serde(default = "d3")]
    pub peck_depth: f64,
    #[serde(default = "d0_5")]
    pub dwell_time: f64,
    #[serde(default = "d0_5")]
    pub retract_amount: f64,
    #[serde(default = "d300")]
    pub feed_rate: f64,
    #[serde(default = "d2")]
    pub retract_z: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChamferOpConfig {
    #[serde(default = "d1")]
    pub chamfer_width: f64,
    #[serde(default = "d0_1")]
    pub tip_offset: f64,
    #[serde(default = "d800")]
    pub feed_rate: f64,
    #[serde(default = "d400")]
    pub plunge_rate: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DropCutterOpConfig {
    #[serde(default = "d1")]
    pub stepover: f64,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default = "default_min_z")]
    pub min_z: f64,
    #[serde(default)]
    pub slope_from: f64,
    #[serde(default = "d90")]
    pub slope_to: f64,
}

fn default_min_z() -> f64 {
    -50.0
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Adaptive3dEntryStyle {
    #[default]
    Plunge,
    Helix,
    Ramp,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliRegionOrdering {
    #[default]
    Global,
    ByArea,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliClearingStrategy {
    #[default]
    ContourParallel,
    Adaptive,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Adaptive3dOpConfig {
    #[serde(default = "d2")]
    pub stepover: f64,
    #[serde(default = "d3")]
    pub depth_per_pass: f64,
    #[serde(default = "d0_5")]
    pub stock_to_leave_radial: f64,
    #[serde(default = "d0_5")]
    pub stock_to_leave_axial: f64,
    #[serde(default = "d1500")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default = "d0_1")]
    pub tolerance: f64,
    #[serde(default)]
    pub min_cutting_radius: f64,
    #[serde(default = "d30")]
    pub stock_top_z: f64,
    #[serde(default)]
    pub entry_style: Adaptive3dEntryStyle,
    #[serde(default)]
    pub fine_stepdown: f64,
    #[serde(default)]
    pub detect_flat_areas: bool,
    #[serde(default)]
    pub region_ordering: CliRegionOrdering,
    #[serde(default)]
    pub clearing_strategy: CliClearingStrategy,
    #[serde(default)]
    pub z_blend: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WaterlineOpConfig {
    #[serde(default = "d1")]
    pub z_step: f64,
    #[serde(default = "d0_5")]
    pub sampling: f64,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default)]
    pub continuous: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PencilOpConfig {
    #[serde(default = "d160")]
    pub bitangency_angle: f64,
    #[serde(default = "d2")]
    pub min_cut_length: f64,
    #[serde(default = "d5")]
    pub hookup_distance: f64,
    #[serde(default = "d1_usize")]
    pub num_offset_passes: usize,
    #[serde(default = "d0_5")]
    pub offset_stepover: f64,
    #[serde(default = "d0_5")]
    pub sampling: f64,
    #[serde(default = "d800")]
    pub feed_rate: f64,
    #[serde(default = "d400")]
    pub plunge_rate: f64,
    #[serde(default)]
    pub stock_to_leave: f64,
}

fn d1_usize() -> usize {
    1
}
fn d160() -> f64 {
    160.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScallopOpConfig {
    #[serde(default = "d0_1")]
    pub scallop_height: f64,
    #[serde(default = "d0_05")]
    pub tolerance: f64,
    #[serde(default)]
    pub direction: ScallopDirection,
    #[serde(default)]
    pub continuous: bool,
    #[serde(default)]
    pub slope_from: f64,
    #[serde(default = "d90")]
    pub slope_to: f64,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default)]
    pub stock_to_leave: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SteepShallowOpConfig {
    #[serde(default = "d45")]
    pub threshold_angle: f64,
    #[serde(default = "d1")]
    pub overlap_distance: f64,
    #[serde(default = "d0_5")]
    pub wall_clearance: f64,
    #[serde(default = "default_true")]
    pub steep_first: bool,
    #[serde(default = "d1")]
    pub stepover: f64,
    #[serde(default = "d1")]
    pub z_step: f64,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default = "d0_5")]
    pub sampling: f64,
    #[serde(default)]
    pub stock_to_leave: f64,
    #[serde(default = "d0_05")]
    pub tolerance: f64,
}

fn d45() -> f64 {
    45.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct RampFinishOpConfig {
    #[serde(default = "d0_5")]
    pub max_stepdown: f64,
    #[serde(default = "d30")]
    pub slope_from: f64,
    #[serde(default = "d90")]
    pub slope_to: f64,
    #[serde(default)]
    pub direction: CutDirection,
    #[serde(default)]
    pub order_bottom_up: bool,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default = "d0_5")]
    pub sampling: f64,
    #[serde(default)]
    pub stock_to_leave: f64,
    #[serde(default = "d0_05")]
    pub tolerance: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpiralFinishOpConfig {
    #[serde(default = "d1")]
    pub stepover: f64,
    #[serde(default = "default_spiral_direction")]
    pub direction: SpiralDirection,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default)]
    pub stock_to_leave: f64,
}

fn default_spiral_direction() -> SpiralDirection {
    SpiralDirection::InsideOut
}

#[derive(Debug, Clone, Deserialize)]
pub struct RadialFinishOpConfig {
    #[serde(default = "d5")]
    pub angular_step: f64,
    #[serde(default = "d0_5")]
    pub point_spacing: f64,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default)]
    pub stock_to_leave: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HorizontalFinishOpConfig {
    #[serde(default = "d5")]
    pub angle_threshold: f64,
    #[serde(default = "d1")]
    pub stepover: f64,
    #[serde(default = "d1000")]
    pub feed_rate: f64,
    #[serde(default = "d500")]
    pub plunge_rate: f64,
    #[serde(default)]
    pub stock_to_leave: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectCurveOpConfig {
    #[serde(default = "d1")]
    pub depth: f64,
    #[serde(default = "d0_5")]
    pub point_spacing: f64,
    #[serde(default = "d800")]
    pub feed_rate: f64,
    #[serde(default = "d400")]
    pub plunge_rate: f64,
    /// Model ID of the 3D surface to project onto.
    pub surface_model_id: Option<usize>,
    /// Projection direction: "from_above" or "from_below".
    #[serde(default = "default_from_above")]
    pub direction: String,
}

fn default_from_above() -> String {
    "from_above".to_owned()
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlignmentPinDrillOpConfig {
    #[serde(default)]
    pub holes: Vec<[f64; 2]>,
    #[serde(default = "d2")]
    pub spoilboard_penetration: f64,
    #[serde(default)]
    pub cycle: CliDrillCycleType,
    #[serde(default = "d3")]
    pub peck_depth: f64,
    #[serde(default = "d300")]
    pub feed_rate: f64,
    #[serde(default = "d2")]
    pub retract_z: f64,
}

// Default value helpers
fn d0_05() -> f64 {
    0.05
}
fn d0_1() -> f64 {
    0.1
}
fn d0_5() -> f64 {
    0.5
}
fn d1() -> f64 {
    1.0
}
fn d1_5() -> f64 {
    1.5
}
fn d2() -> f64 {
    2.0
}
fn d3() -> f64 {
    3.0
}
fn d5() -> f64 {
    5.0
}
fn d6() -> f64 {
    6.0
}
fn d10() -> f64 {
    10.0
}
fn d30() -> f64 {
    30.0
}
fn d90() -> f64 {
    90.0
}
fn d300() -> f64 {
    300.0
}
fn d400() -> f64 {
    400.0
}
fn d500() -> f64 {
    500.0
}
fn d800() -> f64 {
    800.0
}
fn d1000() -> f64 {
    1000.0
}
fn d1500() -> f64 {
    1500.0
}
fn default_true() -> bool {
    true
}

// ── Toolpath section ────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectToolpathSection {
    #[serde(default)]
    pub id: Option<ToolpathId>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub operation: Option<OperationConfig>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tool_id: Option<ToolId>,
    #[serde(default)]
    pub model_id: Option<ModelId>,
    #[serde(default)]
    pub dressups: DressupConfig,
    #[serde(default)]
    pub heights: HeightsConfig,
    #[serde(default)]
    pub debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions,
    // Fields we parse but don't use for execution
    #[serde(rename = "type", default)]
    pub op_type: Option<String>,
}

// ── Loaded geometry ─────────────────────────────────────────────────────

enum LoadedGeometry {
    Mesh(TriangleMesh),
    Polygons(Vec<rs_cam_core::polygon::Polygon2>),
}

// ── Mesh transforms ─────────────────────────────────────────────────────

/// Transform a mesh from global frame to local frame for a bottom-up setup.
///
/// face_up=bottom flips Y and Z relative to the stock:
///   y_local = origin_y + stock_y - (y_world - origin_y)
///   z_local = origin_z + stock_z - (z_world - origin_z)
///
/// Double flip (Y+Z) preserves winding, so no triangle reversal needed.
fn transform_mesh_for_bottom(
    mesh: &TriangleMesh,
    origin_y: f64,
    stock_y: f64,
    origin_z: f64,
    stock_z: f64,
) -> TriangleMesh {
    let y_mirror = origin_y + stock_y + origin_y;
    let z_mirror = origin_z + stock_z + origin_z;
    let vertices: Vec<P3> = mesh
        .vertices
        .iter()
        .map(|v| P3::new(v.x, y_mirror - v.y, z_mirror - v.z))
        .collect();
    // Double flip (Y+Z) = rotation around X axis by 180° → winding preserved
    let triangles: Vec<[u32; 3]> = mesh.triangles.clone();
    TriangleMesh::from_raw(vertices, triangles)
}

// ── Tool building ───────────────────────────────────────────────────────

fn build_cutter(tool: &ProjectToolSection) -> ToolDefinition {
    let cutter: Box<dyn MillingCutter> = match tool.tool_type {
        ToolType::EndMill => Box::new(FlatEndmill::new(tool.diameter, tool.cutting_length)),
        ToolType::BallNose => Box::new(BallEndmill::new(tool.diameter, tool.cutting_length)),
        ToolType::BullNose => Box::new(BullNoseEndmill::new(
            tool.diameter,
            tool.corner_radius,
            tool.cutting_length,
        )),
        ToolType::VBit => Box::new(VBitEndmill::new(
            tool.diameter,
            tool.included_angle,
            tool.cutting_length,
        )),
        ToolType::TaperedBallNose => Box::new(TaperedBallEndmill::new(
            tool.diameter,
            tool.taper_half_angle,
            tool.shaft_diameter,
            tool.cutting_length,
        )),
    };
    ToolDefinition::new(
        cutter,
        tool.shank_diameter,
        tool.shank_length,
        tool.holder_diameter,
        tool.stickout,
        tool.flute_count,
    )
}

// ── Model loading ───────────────────────────────────────────────────────

fn infer_model_kind(path: &Path) -> Option<ModelKind> {
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

fn load_model(model: &ProjectModelSection, project_dir: &Path) -> Result<LoadedGeometry> {
    let raw_path = Path::new(&model.path);
    let full_path = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        project_dir.join(raw_path)
    };

    let kind = model
        .kind
        .or_else(|| infer_model_kind(&full_path))
        .context(format!(
            "Cannot determine file type for model '{}'",
            model.name
        ))?;

    let scale = model.units.map(|u| u.scale_factor()).unwrap_or(1.0);

    match kind {
        ModelKind::Stl => {
            let mesh = TriangleMesh::from_stl_scaled(&full_path, scale)
                .context(format!("Failed to load STL: {}", full_path.display()))?;
            Ok(LoadedGeometry::Mesh(mesh))
        }
        ModelKind::Dxf => {
            let polys = rs_cam_core::dxf_input::load_dxf(&full_path, 5.0)
                .context(format!("Failed to load DXF: {}", full_path.display()))?;
            Ok(LoadedGeometry::Polygons(polys))
        }
        ModelKind::Svg => {
            let polys = rs_cam_core::svg_input::load_svg(&full_path, 0.1)
                .context(format!("Failed to load SVG: {}", full_path.display()))?;
            Ok(LoadedGeometry::Polygons(polys))
        }
        ModelKind::Step => {
            let enriched = rs_cam_core::step_input::load_step(&full_path, 0.1)
                .context(format!("Failed to load STEP: {}", full_path.display()))?;
            Ok(LoadedGeometry::Mesh((*enriched.mesh).clone()))
        }
    }
}

// ── Dressup application ─────────────────────────────────────────────────

fn apply_dressups(
    mut tp: Toolpath,
    cfg: &DressupConfig,
    tool_diameter: f64,
    safe_z: f64,
) -> Toolpath {
    let tool_radius = tool_diameter / 2.0;

    // Determine a reasonable plunge rate from the toolpath itself, for entry dressup.
    let plunge_rate = tp
        .moves
        .iter()
        .find_map(|m| match m.move_type {
            rs_cam_core::toolpath::MoveType::Linear { feed_rate } => Some(feed_rate * 0.5),
            _ => None,
        })
        .unwrap_or(500.0);

    // 1. Entry style
    match cfg.entry_style {
        DressupEntryStyle::Ramp => {
            tp = apply_entry(
                tp,
                EntryStyle::Ramp {
                    max_angle_deg: cfg.ramp_angle,
                },
                plunge_rate,
            );
        }
        DressupEntryStyle::Helix => {
            tp = apply_entry(
                tp,
                EntryStyle::Helix {
                    radius: cfg.helix_radius,
                    pitch: cfg.helix_pitch,
                },
                plunge_rate,
            );
        }
        DressupEntryStyle::None => {}
    }

    // 2. Dogbones
    if cfg.dogbone {
        tp = apply_dogbones(tp, tool_radius, cfg.dogbone_angle);
    }

    // 3. Lead in/out
    if cfg.lead_in_out {
        tp = apply_lead_in_out(tp, cfg.lead_radius);
    }

    // 4. Link moves
    if cfg.link_moves {
        tp = apply_link_moves(
            tp,
            &LinkMoveParams {
                max_link_distance: cfg.link_max_distance,
                link_feed_rate: cfg.link_feed_rate,
                safe_z_threshold: safe_z * 0.9,
            },
        );
    }

    // 5. Arc fitting
    if cfg.arc_fitting {
        tp = fit_arcs(&tp, cfg.arc_tolerance);
    }

    // 6. Rapid order optimization
    if cfg.optimize_rapid_order {
        tp = optimize_rapid_order(&tp, safe_z);
    }

    tp
}

// ── Operation execution ─────────────────────────────────────────────────

struct ExecutionContext<'a> {
    mesh: Option<&'a TriangleMesh>,
    index: Option<SpatialIndex>,
    polygons: Option<&'a [rs_cam_core::polygon::Polygon2]>,
    tool_def: &'a ToolDefinition,
    tool_section: &'a ProjectToolSection,
    heights: ResolvedHeights,
    stock_bbox: &'a BoundingBox3,
    /// Surface mesh for ProjectCurve (resolved from surface_model_id).
    surface_mesh: Option<&'a TriangleMesh>,
}

fn execute_operation(
    op: &OperationConfig,
    ctx: &ExecutionContext<'_>,
    debug_ctx: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath> {
    match op {
        OperationConfig::Face(cfg) => run_face(cfg, ctx),
        OperationConfig::Pocket(cfg) => run_pocket(cfg, ctx),
        OperationConfig::Profile(cfg) => run_profile(cfg, ctx),
        OperationConfig::Adaptive(cfg) => run_adaptive(cfg, ctx),
        OperationConfig::VCarve(cfg) => run_vcarve(cfg, ctx),
        OperationConfig::Rest(cfg) => run_rest(cfg, ctx),
        OperationConfig::Inlay(cfg) => run_inlay(cfg, ctx),
        OperationConfig::Zigzag(cfg) => run_zigzag(cfg, ctx),
        OperationConfig::Trace(cfg) => run_trace(cfg, ctx),
        OperationConfig::Drill(cfg) => run_drill(cfg, ctx),
        OperationConfig::Chamfer(cfg) => run_chamfer(cfg, ctx),
        OperationConfig::DropCutter(cfg) => run_drop_cutter(cfg, ctx),
        OperationConfig::Adaptive3d(cfg) => run_adaptive3d(cfg, ctx, debug_ctx),
        OperationConfig::Waterline(cfg) => run_waterline(cfg, ctx),
        OperationConfig::Pencil(cfg) => run_pencil(cfg, ctx),
        OperationConfig::Scallop(cfg) => run_scallop(cfg, ctx),
        OperationConfig::SteepShallow(cfg) => run_steep_shallow(cfg, ctx),
        OperationConfig::RampFinish(cfg) => run_ramp_finish(cfg, ctx),
        OperationConfig::SpiralFinish(cfg) => run_spiral_finish(cfg, ctx),
        OperationConfig::RadialFinish(cfg) => run_radial_finish(cfg, ctx),
        OperationConfig::HorizontalFinish(cfg) => run_horizontal_finish(cfg, ctx),
        OperationConfig::ProjectCurve(cfg) => run_project_curve(cfg, ctx),
        OperationConfig::AlignmentPinDrill(cfg) => run_alignment_pin_drill(cfg, ctx),
    }
}

fn require_mesh<'a>(ctx: &'a ExecutionContext<'_>) -> Result<(&'a TriangleMesh, &'a SpatialIndex)> {
    let mesh = ctx
        .mesh
        .context("Operation requires a 3D mesh (STL/STEP)")?;
    let index = ctx.index.as_ref().context("No spatial index")?;
    Ok((mesh, index))
}

fn require_polygons<'a>(
    ctx: &'a ExecutionContext<'_>,
) -> Result<&'a [rs_cam_core::polygon::Polygon2]> {
    ctx.polygons
        .context("Operation requires 2D geometry (SVG/DXF)")
}

// -- 2D operations --

fn run_face(cfg: &FaceOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    use rs_cam_core::face::FaceParams;
    let params = FaceParams {
        tool_radius: ctx.tool_def.diameter() / 2.0,
        stepover: cfg.stepover,
        depth: cfg.depth,
        depth_per_pass: cfg.depth_per_pass,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: ctx.heights.retract_z,
        stock_offset: cfg.stock_offset,
        direction: cfg.direction,
    };
    Ok(face_toolpath(ctx.stock_bbox, &params))
}

fn run_pocket(cfg: &PocketOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let polys = require_polygons(ctx)?;
    let tool_radius = ctx.tool_def.diameter() / 2.0;
    let safe_z = ctx.heights.retract_z;
    let stepping = DepthStepping::new(
        ctx.heights.top_z,
        ctx.heights.top_z - cfg.depth,
        cfg.depth_per_pass,
    );
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = depth_stepped_toolpath(&stepping, safe_z, |z| match cfg.pattern {
            PocketPattern::Zigzag => zigzag_toolpath(
                poly,
                &ZigzagParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z,
                    angle: cfg.angle,
                },
            ),
            PocketPattern::Contour => pocket_toolpath(
                poly,
                &PocketParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z,
                    climb: cfg.climb,
                },
            ),
        });
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_profile(cfg: &ProfileOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let polys = require_polygons(ctx)?;
    let tool_radius = ctx.tool_def.diameter() / 2.0;
    let safe_z = ctx.heights.retract_z;
    let stepping = DepthStepping::new(
        ctx.heights.top_z,
        ctx.heights.top_z - cfg.depth,
        cfg.depth_per_pass,
    );
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = depth_stepped_toolpath(&stepping, safe_z, |z| {
            profile_toolpath(
                poly,
                &ProfileParams {
                    tool_radius,
                    side: cfg.side,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z,
                    climb: cfg.climb,
                },
            )
        });
        combined.moves.extend(tp.moves);
    }
    if cfg.tab_count > 0 {
        use rs_cam_core::dressup::{apply_tabs, even_tabs};
        let tabs = even_tabs(cfg.tab_count, cfg.tab_width, cfg.tab_height);
        let cut_depth = ctx.heights.top_z - cfg.depth;
        combined = apply_tabs(combined, &tabs, cut_depth);
    }
    Ok(combined)
}

fn run_adaptive(cfg: &AdaptiveOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let polys = require_polygons(ctx)?;
    let tool_radius = ctx.tool_def.diameter() / 2.0;
    let safe_z = ctx.heights.retract_z;
    let stepping = DepthStepping::new(
        ctx.heights.top_z,
        ctx.heights.top_z - cfg.depth,
        cfg.depth_per_pass,
    );
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = depth_stepped_toolpath(&stepping, safe_z, |z| {
            adaptive_toolpath(
                poly,
                &AdaptiveParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z,
                    tolerance: cfg.tolerance,
                    slot_clearing: cfg.slot_clearing,
                    min_cutting_radius: cfg.min_cutting_radius,
                    initial_stock: None,
                },
            )
        });
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_vcarve(cfg: &VCarveOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let polys = require_polygons(ctx)?;
    let half_angle = ctx.tool_section.included_angle.to_radians() / 2.0;
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = vcarve_toolpath(
            poly,
            &VCarveParams {
                half_angle,
                max_depth: cfg.max_depth,
                stepover: cfg.stepover,
                tolerance: cfg.tolerance,
                feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate,
                safe_z: ctx.heights.retract_z,
            },
        );
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_rest(cfg: &RestOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let polys = require_polygons(ctx)?;
    let tool_radius = ctx.tool_def.diameter() / 2.0;
    let safe_z = ctx.heights.retract_z;
    // Use a default prev_tool_radius slightly larger than current tool
    let prev_tool_radius = tool_radius + 2.0;
    let stepping = DepthStepping::new(
        ctx.heights.top_z,
        ctx.heights.top_z - cfg.depth,
        cfg.depth_per_pass,
    );
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = depth_stepped_toolpath(&stepping, safe_z, |z| {
            rest_machining_toolpath(
                poly,
                &RestParams {
                    tool_radius,
                    prev_tool_radius,
                    cut_depth: z,
                    stepover: cfg.stepover,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z,
                    angle: cfg.angle,
                },
            )
        });
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_inlay(cfg: &InlayOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let polys = require_polygons(ctx)?;
    let half_angle = ctx.tool_section.included_angle.to_radians() / 2.0;
    let mut combined = Toolpath::new();
    for poly in polys {
        let result = inlay_toolpaths(
            poly,
            &InlayParams {
                half_angle,
                pocket_depth: cfg.pocket_depth,
                glue_gap: cfg.glue_gap,
                flat_depth: cfg.flat_depth,
                boundary_offset: cfg.boundary_offset,
                stepover: cfg.stepover,
                flat_tool_radius: cfg.flat_tool_radius,
                tolerance: cfg.tolerance,
                feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate,
                safe_z: ctx.heights.retract_z,
            },
        );
        combined.moves.extend(result.female.moves);
    }
    Ok(combined)
}

fn run_zigzag(cfg: &ZigzagOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let polys = require_polygons(ctx)?;
    let tool_radius = ctx.tool_def.diameter() / 2.0;
    let safe_z = ctx.heights.retract_z;
    let stepping = DepthStepping::new(
        ctx.heights.top_z,
        ctx.heights.top_z - cfg.depth,
        cfg.depth_per_pass,
    );
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = depth_stepped_toolpath(&stepping, safe_z, |z| {
            zigzag_toolpath(
                poly,
                &ZigzagParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: z,
                    feed_rate: cfg.feed_rate,
                    plunge_rate: cfg.plunge_rate,
                    safe_z,
                    angle: cfg.angle,
                },
            )
        });
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_trace(cfg: &TraceOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let polys = require_polygons(ctx)?;
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = trace_toolpath(
            poly,
            &TraceParams {
                tool_radius: ctx.tool_def.diameter() / 2.0,
                depth: cfg.depth,
                depth_per_pass: cfg.depth_per_pass,
                feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate,
                safe_z: ctx.heights.retract_z,
                compensation: cfg.compensation,
                top_z: ctx.heights.top_z,
            },
        );
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_drill(cfg: &DrillOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let polys = require_polygons(ctx)?;
    let cycle = match cfg.cycle {
        CliDrillCycleType::Simple => DrillCycle::Simple,
        CliDrillCycleType::Dwell => DrillCycle::Dwell(cfg.dwell_time),
        CliDrillCycleType::Peck => DrillCycle::Peck(cfg.peck_depth),
        CliDrillCycleType::ChipBreak => DrillCycle::ChipBreak(cfg.peck_depth, cfg.retract_amount),
    };
    // Extract approximate center of each polygon as drill position
    let holes: Vec<[f64; 2]> = polys
        .iter()
        .filter_map(|p| {
            if p.exterior.is_empty() {
                return None;
            }
            let n = p.exterior.len() as f64;
            let cx: f64 = p.exterior.iter().map(|pt| pt.x).sum::<f64>() / n;
            let cy: f64 = p.exterior.iter().map(|pt| pt.y).sum::<f64>() / n;
            Some([cx, cy])
        })
        .collect();
    let params = DrillParams {
        depth: cfg.depth,
        cycle,
        feed_rate: cfg.feed_rate,
        safe_z: ctx.heights.retract_z,
        retract_z: ctx.heights.retract_z - 2.0,
    };
    Ok(drill_toolpath(&holes, &params))
}

fn run_chamfer(cfg: &ChamferOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let polys = require_polygons(ctx)?;
    let tool_half_angle = ctx.tool_section.included_angle.to_radians() / 2.0;
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = chamfer_toolpath(
            poly,
            &ChamferParams {
                chamfer_width: cfg.chamfer_width,
                tip_offset: cfg.tip_offset,
                tool_half_angle,
                tool_radius: ctx.tool_def.diameter() / 2.0,
                feed_rate: cfg.feed_rate,
                plunge_rate: cfg.plunge_rate,
                safe_z: ctx.heights.retract_z,
            },
        );
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

// -- 3D operations --

fn run_drop_cutter(cfg: &DropCutterOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let (mesh, index) = require_mesh(ctx)?;
    let grid = batch_drop_cutter_with_cancel(
        mesh,
        index,
        ctx.tool_def,
        cfg.stepover,
        0.0, // direction_deg
        cfg.min_z,
        &(|| false),
    )
    .map_err(|_cancelled| anyhow::anyhow!("Drop cutter cancelled"))?;
    Ok(raster_toolpath_from_grid(
        &grid,
        cfg.feed_rate,
        cfg.plunge_rate,
        ctx.heights.retract_z,
    ))
}

fn run_adaptive3d(
    cfg: &Adaptive3dOpConfig,
    ctx: &ExecutionContext<'_>,
    debug_ctx: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
) -> Result<Toolpath> {
    let (mesh, index) = require_mesh(ctx)?;
    let entry = match cfg.entry_style {
        Adaptive3dEntryStyle::Plunge => EntryStyle3d::Plunge,
        Adaptive3dEntryStyle::Helix => EntryStyle3d::Helix {
            radius: 2.0,
            pitch: 1.0,
        },
        Adaptive3dEntryStyle::Ramp => EntryStyle3d::Ramp { max_angle_deg: 3.0 },
    };
    let ordering = match cfg.region_ordering {
        CliRegionOrdering::Global => RegionOrdering::Global,
        CliRegionOrdering::ByArea => RegionOrdering::ByArea,
    };
    let clearing = match cfg.clearing_strategy {
        CliClearingStrategy::ContourParallel => ClearingStrategy3d::ContourParallel,
        CliClearingStrategy::Adaptive => ClearingStrategy3d::Adaptive,
    };
    let tool_radius = ctx.tool_def.diameter() / 2.0;
    let fine = if cfg.fine_stepdown > 0.0 {
        Some(cfg.fine_stepdown)
    } else {
        None
    };
    let params = Adaptive3dParams {
        tool_radius,
        stepover: cfg.stepover,
        depth_per_pass: cfg.depth_per_pass,
        stock_to_leave: cfg.stock_to_leave_radial,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        tolerance: cfg.tolerance,
        min_cutting_radius: cfg.min_cutting_radius,
        // Use actual stock top Z, not the config fallback (which may be stale/wrong)
        stock_top_z: ctx.stock_bbox.max.z,
        entry_style: entry,
        fine_stepdown: fine,
        detect_flat_areas: cfg.detect_flat_areas,
        max_stay_down_dist: None,
        region_ordering: ordering,
        initial_stock: None,
        safe_z: ctx.heights.retract_z,
        clearing_strategy: clearing,
        z_blend: cfg.z_blend,
    };
    let (tp, _annotations) = adaptive_3d_toolpath_annotated_traced_with_cancel(
        mesh,
        index,
        ctx.tool_def,
        &params,
        &|| false,
        debug_ctx,
    )?;
    Ok(tp)
}

fn run_waterline(cfg: &WaterlineOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let (mesh, index) = require_mesh(ctx)?;
    let params = WaterlineParams {
        sampling: cfg.sampling,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: ctx.heights.retract_z,
    };
    let tp = waterline_toolpath_with_cancel(
        mesh,
        index,
        ctx.tool_def,
        mesh.bbox.max.z,
        mesh.bbox.min.z,
        cfg.z_step,
        &params,
        &|| false,
    )
    .map_err(|_cancelled| anyhow::anyhow!("Waterline cancelled"))?;
    Ok(tp)
}

fn run_pencil(cfg: &PencilOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let (mesh, index) = require_mesh(ctx)?;
    let params = PencilParams {
        bitangency_angle: cfg.bitangency_angle,
        min_cut_length: cfg.min_cut_length,
        hookup_distance: cfg.hookup_distance,
        num_offset_passes: cfg.num_offset_passes,
        offset_stepover: cfg.offset_stepover,
        sampling: cfg.sampling,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: ctx.heights.retract_z,
        stock_to_leave: cfg.stock_to_leave,
    };
    Ok(pencil_toolpath(mesh, index, ctx.tool_def, &params))
}

fn run_scallop(cfg: &ScallopOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let (mesh, index) = require_mesh(ctx)?;
    let params = ScallopParams {
        scallop_height: cfg.scallop_height,
        tolerance: cfg.tolerance,
        direction: cfg.direction,
        continuous: cfg.continuous,
        slope_from: cfg.slope_from,
        slope_to: cfg.slope_to,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: ctx.heights.retract_z,
        stock_to_leave: cfg.stock_to_leave,
    };
    Ok(scallop_toolpath(mesh, index, ctx.tool_def, &params))
}

fn run_steep_shallow(cfg: &SteepShallowOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let (mesh, index) = require_mesh(ctx)?;
    let params = SteepShallowParams {
        threshold_angle: cfg.threshold_angle,
        overlap_distance: cfg.overlap_distance,
        wall_clearance: cfg.wall_clearance,
        steep_first: cfg.steep_first,
        stepover: cfg.stepover,
        z_step: cfg.z_step,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: ctx.heights.retract_z,
        sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave,
        tolerance: cfg.tolerance,
    };
    Ok(steep_shallow_toolpath(mesh, index, ctx.tool_def, &params))
}

fn run_ramp_finish(cfg: &RampFinishOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let (mesh, index) = require_mesh(ctx)?;
    let params = RampFinishParams {
        max_stepdown: cfg.max_stepdown,
        slope_from: cfg.slope_from,
        slope_to: cfg.slope_to,
        direction: cfg.direction,
        order_bottom_up: cfg.order_bottom_up,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: ctx.heights.retract_z,
        sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave,
        tolerance: cfg.tolerance,
    };
    Ok(ramp_finish_toolpath(mesh, index, ctx.tool_def, &params))
}

fn run_spiral_finish(cfg: &SpiralFinishOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let (mesh, index) = require_mesh(ctx)?;
    let params = SpiralFinishParams {
        stepover: cfg.stepover,
        direction: cfg.direction,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: ctx.heights.retract_z,
        stock_to_leave: cfg.stock_to_leave,
    };
    Ok(spiral_finish_toolpath(mesh, index, ctx.tool_def, &params))
}

fn run_radial_finish(cfg: &RadialFinishOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let (mesh, index) = require_mesh(ctx)?;
    let params = RadialFinishParams {
        angular_step: cfg.angular_step,
        point_spacing: cfg.point_spacing,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: ctx.heights.retract_z,
        stock_to_leave: cfg.stock_to_leave,
    };
    Ok(radial_finish_toolpath(mesh, index, ctx.tool_def, &params))
}

fn run_horizontal_finish(
    cfg: &HorizontalFinishOpConfig,
    ctx: &ExecutionContext<'_>,
) -> Result<Toolpath> {
    let (mesh, index) = require_mesh(ctx)?;
    let params = HorizontalFinishParams {
        angle_threshold: cfg.angle_threshold,
        stepover: cfg.stepover,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: ctx.heights.retract_z,
        stock_to_leave: cfg.stock_to_leave,
    };
    Ok(horizontal_finish_toolpath(
        mesh,
        index,
        ctx.tool_def,
        &params,
    ))
}

fn run_project_curve(cfg: &ProjectCurveOpConfig, ctx: &ExecutionContext<'_>) -> Result<Toolpath> {
    let polys = ctx
        .polygons
        .context("Project Curve requires 2D curves (DXF/SVG)")?;

    // Use surface_mesh (from surface_model_id) or fall back to primary mesh
    let mesh = ctx
        .surface_mesh
        .or(ctx.mesh)
        .context("Project Curve requires a surface mesh (set surface_model_id)")?;
    let index = SpatialIndex::build_auto(mesh);

    let direction = match cfg.direction.as_str() {
        "from_below" => rs_cam_core::project_curve::ProjectDirection::FromBelow,
        _ => rs_cam_core::project_curve::ProjectDirection::FromAbove,
    };
    let params = ProjectCurveParams {
        depth: cfg.depth,
        point_spacing: cfg.point_spacing,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: ctx.heights.retract_z,
        direction,
    };
    // Project all polygons onto the surface
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = project_curve_toolpath(poly, mesh, &index, ctx.tool_def, &params);
        combined.moves.extend(tp.moves);
    }
    Ok(combined)
}

fn run_alignment_pin_drill(
    cfg: &AlignmentPinDrillOpConfig,
    ctx: &ExecutionContext<'_>,
) -> Result<Toolpath> {
    if cfg.holes.is_empty() {
        return Ok(Toolpath::new());
    }
    // Build simple drill polygons from pin positions (point geometry).
    let cycle = match cfg.cycle {
        CliDrillCycleType::Simple => DrillCycle::Simple,
        CliDrillCycleType::Dwell => DrillCycle::Dwell(0.5),
        CliDrillCycleType::Peck => DrillCycle::Peck(cfg.peck_depth),
        CliDrillCycleType::ChipBreak => DrillCycle::ChipBreak(cfg.peck_depth, 0.5),
    };
    let depth = ctx.heights.depth() + cfg.spoilboard_penetration;
    let params = DrillParams {
        depth,
        cycle,
        feed_rate: cfg.feed_rate,
        safe_z: ctx.heights.retract_z,
        retract_z: cfg.retract_z,
    };
    Ok(drill_toolpath(&cfg.holes, &params))
}

// ── Per-toolpath execution result ───────────────────────────────────────

struct ToolpathExecutionResult {
    toolpath_id: usize,
    name: String,
    op_kind: String,
    op_label: String,
    tool_name: String,
    tool_def: ToolDefinition,
    toolpath: Toolpath,
    debug_trace: Option<ToolpathDebugTrace>,
    semantic_trace: Option<ToolpathSemanticTrace>,
    collision_report: Option<CollisionReport>,
    rapid_collisions: Vec<rs_cam_core::collision::RapidCollision>,
}

// ── JSON output types ───────────────────────────────────────────────────

#[derive(Serialize)]
struct ToolpathDiagnostic {
    toolpath_id: usize,
    toolpath_name: String,
    operation_type: String,
    tool: String,
    move_count: usize,
    cutting_distance_mm: f64,
    rapid_distance_mm: f64,
    debug_trace: Option<ToolpathDebugTrace>,
    semantic_trace: Option<ToolpathSemanticTrace>,
    collision_count: usize,
    rapid_collision_count: usize,
    min_safe_stickout: Option<f64>,
}

#[derive(Serialize)]
struct ToolpathSummaryEntry {
    id: usize,
    name: String,
    operation: String,
    status: String,
    move_count: usize,
    collision_count: usize,
}

#[derive(Serialize)]
struct ProjectSummary {
    project: String,
    setup_count: usize,
    toolpath_count: usize,
    total_cutting_distance_mm: f64,
    total_rapid_distance_mm: f64,
    total_runtime_s: f64,
    air_cut_percentage: f64,
    average_engagement: f64,
    collision_count: usize,
    rapid_collision_count: usize,
    per_toolpath: Vec<ToolpathSummaryEntry>,
    verdict: String,
}

// ── Main entry point ────────────────────────────────────────────────────

pub fn run_project_command(
    input: &Path,
    output_dir: &Path,
    setup_filter: Option<&str>,
    skip_ids: &[usize],
    resolution: f64,
    summary: bool,
) -> Result<()> {
    // 1. Load and parse project TOML
    let project_path = input
        .canonicalize()
        .context(format!("Project file not found: {}", input.display()))?;
    let project_dir = project_path.parent().unwrap_or(Path::new("."));
    let content = std::fs::read_to_string(&project_path).context(format!(
        "Failed to read project: {}",
        project_path.display()
    ))?;
    let project: ProjectFile = toml::from_str(&content).context("Failed to parse project TOML")?;

    info!(
        name = %project.job.name,
        format_version = project.format_version,
        tools = project.tools.len(),
        models = project.models.len(),
        setups = project.setups.len(),
        "Loaded project"
    );

    // 2. Build stock bounding box
    let stock = &project.job.stock;
    let stock_bbox = BoundingBox3 {
        min: P3::new(stock.origin_x, stock.origin_y, stock.origin_z),
        max: P3::new(
            stock.origin_x + stock.x,
            stock.origin_y + stock.y,
            stock.origin_z + stock.z,
        ),
    };

    // 3. Load all models
    let mut meshes: Vec<Option<TriangleMesh>> = Vec::new();
    let mut polygon_sets: Vec<Option<Vec<rs_cam_core::polygon::Polygon2>>> = Vec::new();
    let mut model_ids: Vec<Option<ModelId>> = Vec::new();

    for model in &project.models {
        let model_id = model.id;
        match load_model(model, project_dir) {
            Ok(LoadedGeometry::Mesh(mesh)) => {
                info!(name = %model.name, tris = mesh.triangles.len(), "Loaded mesh model");
                meshes.push(Some(mesh));
                polygon_sets.push(None);
            }
            Ok(LoadedGeometry::Polygons(polys)) => {
                info!(name = %model.name, polygons = polys.len(), "Loaded 2D model");
                meshes.push(None);
                polygon_sets.push(Some(polys));
            }
            Err(e) => {
                warn!(name = %model.name, error = %e, "Failed to load model, skipping");
                meshes.push(None);
                polygon_sets.push(None);
            }
        }
        model_ids.push(model_id);
    }

    // 4. Collect toolpaths from setups (with face_up info)
    let mut all_toolpaths: Vec<(String, &str, &ProjectToolpathSection)> = Vec::new();
    if !project.setups.is_empty() {
        for setup in &project.setups {
            if let Some(filter) = setup_filter
                && setup.name != filter
                && setup.id.map(|id| id.0.to_string()) != Some(filter.to_owned())
            {
                debug!(setup = %setup.name, "Skipping setup (filter)");
                continue;
            }
            for tp in &setup.toolpaths {
                all_toolpaths.push((setup.name.clone(), setup.face_up.as_str(), tp));
            }
        }
    } else {
        for tp in &project.toolpaths {
            all_toolpaths.push(("Default".to_owned(), "top", tp));
        }
    }

    // Pre-compute flipped meshes for bottom-up setups
    let is_bottom_up = all_toolpaths.iter().any(|(_, face, _)| *face == "bottom");
    let flipped_meshes: Vec<Option<TriangleMesh>> = if is_bottom_up {
        meshes
            .iter()
            .map(|m| {
                m.as_ref().map(|mesh| {
                    transform_mesh_for_bottom(mesh, stock.origin_y, stock.y, stock.origin_z, stock.z)
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    let local_stock_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(stock.x, stock.y, stock.z),
    };

    info!(total = all_toolpaths.len(), "Collected toolpaths");

    // 5. Create output directory
    std::fs::create_dir_all(output_dir).context(format!(
        "Failed to create output dir: {}",
        output_dir.display()
    ))?;

    // 6. Execute each toolpath
    let mut results: Vec<ToolpathExecutionResult> = Vec::new();
    let never_cancel = || false;

    for (_setup_name, face_up, tp_section) in &all_toolpaths {
        let is_bottom = *face_up == "bottom";
        let tp_id = tp_section.id.map(|id| id.0).unwrap_or(0);

        if !tp_section.enabled {
            debug!(id = tp_id, name = %tp_section.name, "Skipping disabled toolpath");
            continue;
        }
        if skip_ids.contains(&tp_id) {
            info!(id = tp_id, name = %tp_section.name, "Skipping toolpath (--skip)");
            continue;
        }

        let Some(operation) = &tp_section.operation else {
            warn!(id = tp_id, name = %tp_section.name, "No operation config, skipping");
            continue;
        };

        // Find the tool
        let tool_id = tp_section.tool_id.unwrap_or(ToolId(0));
        let tool_section = project
            .tools
            .iter()
            .find(|t| t.id == Some(tool_id))
            .or_else(|| project.tools.first());
        let Some(tool_section) = tool_section else {
            warn!(id = tp_id, "No tool found, skipping");
            continue;
        };

        // Find the model
        let model_id = tp_section.model_id.unwrap_or(ModelId(0));
        let model_idx = model_ids
            .iter()
            .position(|id| id.as_ref() == Some(&model_id))
            .or(if !model_ids.is_empty() { Some(0) } else { None });

        // Use flipped mesh for bottom-up setups, otherwise original
        let mesh_ref = if is_bottom {
            model_idx.and_then(|i| flipped_meshes.get(i).and_then(|m| m.as_ref()))
        } else {
            model_idx.and_then(|i| meshes.get(i).and_then(|m| m.as_ref()))
        };
        let polys_ref = model_idx.and_then(|i| polygon_sets.get(i).and_then(|p| p.as_deref()));

        // Use local stock bbox for bottom-up, global for top
        let effective_stock_bbox = if is_bottom { &local_stock_bbox } else { &stock_bbox };

        // Build tool
        let tool_def = build_cutter(tool_section);

        // Resolve heights
        let model_bbox = mesh_ref.map(|m| &m.bbox);
        let height_ctx = HeightContext {
            safe_z: project.job.post.safe_z,
            stock_top_z: effective_stock_bbox.max.z,
            stock_bottom_z: effective_stock_bbox.min.z,
            model_top_z: model_bbox.map(|b| b.max.z),
            model_bottom_z: model_bbox.map(|b| b.min.z),
        };
        let heights = resolve_heights(&tp_section.heights, &height_ctx);

        // Build spatial index
        let spatial_index = mesh_ref.map(SpatialIndex::build_auto);

        info!(
            id = tp_id,
            name = %tp_section.name,
            op = operation.label(),
            "Executing toolpath"
        );

        let start = std::time::Instant::now();

        // Create recorders
        let debug_recorder = ToolpathDebugRecorder::new(tp_section.name.clone(), operation.label());
        let semantic_recorder =
            ToolpathSemanticRecorder::new(tp_section.name.clone(), operation.label());
        let debug_root = debug_recorder.root_context();
        let _semantic_root = semantic_recorder.root_context();

        // Create operation scope
        let core_scope = debug_root.start_span("core_generate", operation.label());
        let core_ctx = core_scope.context();

        // Resolve surface_model_id for ProjectCurve (use flipped if bottom-up)
        let surface_mesh_ref = if let Some(OperationConfig::ProjectCurve(ref pc_cfg)) = tp_section.operation {
            pc_cfg.surface_model_id.and_then(|sid| {
                let mid = ModelId(sid);
                let idx = model_ids
                    .iter()
                    .position(|id| id.as_ref() == Some(&mid));
                idx.and_then(|i| {
                    if is_bottom {
                        flipped_meshes.get(i).and_then(|m| m.as_ref())
                    } else {
                        meshes.get(i).and_then(|m| m.as_ref())
                    }
                })
            })
        } else {
            None
        };

        let exec_ctx = ExecutionContext {
            mesh: mesh_ref,
            index: spatial_index,
            polygons: polys_ref,
            tool_def: &tool_def,
            tool_section,
            heights,
            stock_bbox: effective_stock_bbox,
            surface_mesh: surface_mesh_ref,
        };

        let tp_result = execute_operation(operation, &exec_ctx, Some(&core_ctx));

        match tp_result {
            Ok(mut toolpath) => {
                if !toolpath.moves.is_empty() {
                    core_scope.set_move_range(0, toolpath.moves.len().saturating_sub(1));
                }
                drop(core_scope);

                // Apply dressups
                toolpath = apply_dressups(
                    toolpath,
                    &tp_section.dressups,
                    tool_section.diameter,
                    heights.retract_z,
                );

                let elapsed = start.elapsed();
                info!(
                    id = tp_id,
                    moves = toolpath.moves.len(),
                    cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                    elapsed_ms = elapsed.as_millis(),
                    "Toolpath complete"
                );

                // Finish traces
                let mut debug_trace = debug_recorder.finish();
                let mut semantic_trace = semantic_recorder.finish();
                enrich_traces(&mut debug_trace, &mut semantic_trace);

                // Collision check
                let collision_report = if let Some(mesh) = mesh_ref {
                    if tool_section.holder_diameter > 0.0 {
                        let assembly = tool_def.to_assembly();
                        let idx = SpatialIndex::build_auto(mesh);
                        Some(check_collisions_interpolated(
                            &toolpath, &assembly, mesh, &idx, 2.0,
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                };

                let rapid_collisions = check_rapid_collisions(&toolpath, effective_stock_bbox);

                results.push(ToolpathExecutionResult {
                    toolpath_id: tp_id,
                    name: tp_section.name.clone(),
                    op_kind: operation.kind_str().to_owned(),
                    op_label: operation.label().to_owned(),
                    tool_name: tool_section.name.clone(),
                    tool_def,
                    toolpath,
                    debug_trace: Some(debug_trace),
                    semantic_trace: Some(semantic_trace),
                    collision_report,
                    rapid_collisions,
                });
            }
            Err(e) => {
                drop(core_scope);
                eprintln!("ERROR: Toolpath {} '{}': {}", tp_id, tp_section.name, e);
                // Finish recorders to avoid panic on drop
                let _ = debug_recorder.finish();
                let _ = semantic_recorder.finish();
            }
        }
    }

    // 7. Run simulation with cut metrics
    info!(resolution_mm = resolution, "Running simulation");
    let sample_step_mm = resolution.max(0.25);
    let mut sim_stock = TriDexelStock::from_bounds(&stock_bbox, resolution);
    let mut all_samples = Vec::new();

    for (sim_idx, result) in results.iter().enumerate() {
        if result.toolpath.moves.len() < 2 {
            continue;
        }
        let lut = RadialProfileLUT::from_cutter(&result.tool_def, 256);
        let radius = result.tool_def.radius();
        match sim_stock.simulate_toolpath_with_lut_metrics_cancel(
            &result.toolpath,
            &lut,
            radius,
            StockCutDirection::FromTop,
            sim_idx,
            project.job.post.spindle_speed,
            result.tool_def.flute_count,
            3000.0,
            sample_step_mm,
            result.semantic_trace.as_ref(),
            &never_cancel,
        ) {
            Ok(mut samples) => {
                all_samples.append(&mut samples);
            }
            Err(_) => {
                warn!(id = result.toolpath_id, "Simulation cancelled/failed");
            }
        }
    }

    // Build simulation trace with semantics
    let semantic_traces: Vec<_> = results
        .iter()
        .enumerate()
        .filter_map(|(idx, r)| r.semantic_trace.as_ref().map(|t| (idx, t)))
        .collect();
    let sim_trace = SimulationCutTrace::from_samples_with_semantics(
        sample_step_mm,
        all_samples,
        semantic_traces,
    );

    let included_ids: Vec<usize> = results.iter().map(|r| r.toolpath_id).collect();
    let sim_artifact = SimulationCutArtifact::new(
        resolution,
        sample_step_mm,
        [stock_bbox.min.x, stock_bbox.min.y, stock_bbox.min.z],
        [stock_bbox.max.x, stock_bbox.max.y, stock_bbox.max.z],
        included_ids,
        serde_json::json!({ "project": project.job.name }),
        sim_trace.clone(),
    );

    // 8. Write per-toolpath JSON
    for result in &results {
        let collision_count = result
            .collision_report
            .as_ref()
            .map(|r| r.collisions.len())
            .unwrap_or(0);
        let min_safe = result
            .collision_report
            .as_ref()
            .filter(|r| !r.is_clear())
            .map(|r| r.min_safe_stickout);

        let diagnostic = ToolpathDiagnostic {
            toolpath_id: result.toolpath_id,
            toolpath_name: result.name.clone(),
            operation_type: result.op_kind.clone(),
            tool: result.tool_name.clone(),
            move_count: result.toolpath.moves.len(),
            cutting_distance_mm: result.toolpath.total_cutting_distance(),
            rapid_distance_mm: result.toolpath.total_rapid_distance(),
            debug_trace: result.debug_trace.clone(),
            semantic_trace: result.semantic_trace.clone(),
            collision_count,
            rapid_collision_count: result.rapid_collisions.len(),
            min_safe_stickout: min_safe,
        };

        let file_name = format!(
            "tp_{}_{}.json",
            result.toolpath_id,
            result.name.replace(
                |c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_',
                "_"
            )
        );
        let file_path = output_dir.join(file_name);
        let json = serde_json::to_string_pretty(&diagnostic)
            .context("Failed to serialize toolpath diagnostic")?;
        std::fs::write(&file_path, json)
            .context(format!("Failed to write {}", file_path.display()))?;
        debug!(path = %file_path.display(), "Wrote toolpath diagnostic");
    }

    // 9. Write simulation.json
    let sim_path = output_dir.join("simulation.json");
    let sim_json =
        serde_json::to_string_pretty(&sim_artifact).context("Failed to serialize simulation")?;
    std::fs::write(&sim_path, sim_json)
        .context(format!("Failed to write {}", sim_path.display()))?;
    info!(path = %sim_path.display(), "Wrote simulation artifact");

    // 10. Write summary.json
    let total_collision_count: usize = results
        .iter()
        .map(|r| {
            r.collision_report
                .as_ref()
                .map(|rep| rep.collisions.len())
                .unwrap_or(0)
        })
        .sum();
    let total_rapid_collision_count: usize = results.iter().map(|r| r.rapid_collisions.len()).sum();
    let total_cutting: f64 = results
        .iter()
        .map(|r| r.toolpath.total_cutting_distance())
        .sum();
    let total_rapid: f64 = results
        .iter()
        .map(|r| r.toolpath.total_rapid_distance())
        .sum();

    let per_toolpath: Vec<ToolpathSummaryEntry> = results
        .iter()
        .map(|r| {
            let collisions = r
                .collision_report
                .as_ref()
                .map(|rep| rep.collisions.len())
                .unwrap_or(0);
            let status = if collisions > 0 || !r.rapid_collisions.is_empty() {
                "error"
            } else {
                "ok"
            };
            ToolpathSummaryEntry {
                id: r.toolpath_id,
                name: r.name.clone(),
                operation: r.op_label.clone(),
                status: status.to_owned(),
                move_count: r.toolpath.moves.len(),
                collision_count: collisions + r.rapid_collisions.len(),
            }
        })
        .collect();

    let verdict = if total_collision_count > 0 {
        format!(
            "ERROR: {} holder/shank collisions detected",
            total_collision_count
        )
    } else if total_rapid_collision_count > 0 {
        format!(
            "WARNING: {} rapid-through-stock collisions",
            total_rapid_collision_count
        )
    } else if sim_trace.summary.air_cut_time_s > sim_trace.summary.total_runtime_s * 0.4 {
        let pct = if sim_trace.summary.total_runtime_s > 0.0 {
            sim_trace.summary.air_cut_time_s / sim_trace.summary.total_runtime_s * 100.0
        } else {
            0.0
        };
        format!("WARNING: {pct:.1}% air cutting")
    } else {
        "OK".to_owned()
    };

    let air_cut_pct = if sim_trace.summary.total_runtime_s > 0.0 {
        sim_trace.summary.air_cut_time_s / sim_trace.summary.total_runtime_s * 100.0
    } else {
        0.0
    };
    let project_summary = ProjectSummary {
        project: project.job.name.clone(),
        setup_count: project.setups.len().max(1),
        toolpath_count: results.len(),
        total_cutting_distance_mm: total_cutting,
        total_rapid_distance_mm: total_rapid,
        total_runtime_s: sim_trace.summary.total_runtime_s,
        air_cut_percentage: air_cut_pct,
        average_engagement: sim_trace.summary.average_engagement,
        collision_count: total_collision_count,
        rapid_collision_count: total_rapid_collision_count,
        per_toolpath,
        verdict: verdict.clone(),
    };

    let summary_path = output_dir.join("summary.json");
    let summary_json =
        serde_json::to_string_pretty(&project_summary).context("Failed to serialize summary")?;
    std::fs::write(&summary_path, summary_json)
        .context(format!("Failed to write {}", summary_path.display()))?;
    info!(path = %summary_path.display(), "Wrote project summary");

    // 11. Print human-readable summary
    if summary {
        eprintln!("\n=== Project Diagnostics: {} ===", project.job.name);
        eprintln!(
            "Toolpaths: {}  |  Cutting: {:.0}mm  |  Rapid: {:.0}mm  |  Time: {:.0}s",
            results.len(),
            total_cutting,
            total_rapid,
            sim_trace.summary.total_runtime_s,
        );
        eprintln!(
            "Air cutting: {:.1}%  |  Avg engagement: {:.2}  |  Peak chipload: {:.3} mm/tooth",
            air_cut_pct,
            sim_trace.summary.average_engagement,
            sim_trace.summary.peak_chipload_mm_per_tooth,
        );
        for entry in &project_summary.per_toolpath {
            let status_icon = if entry.status == "ok" { " " } else { "!" };
            eprintln!(
                "  [{status_icon}] #{} {} ({}) — {} moves, {} collisions",
                entry.id, entry.name, entry.operation, entry.move_count, entry.collision_count,
            );
        }
        eprintln!("Verdict: {verdict}");
        eprintln!("Output: {}", output_dir.display());
    }

    Ok(())
}
