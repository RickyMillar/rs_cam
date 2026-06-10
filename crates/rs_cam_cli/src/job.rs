//! TOML job file parsing and execution.
//!
//! A job file defines tools, operations, and output settings in a single
//! TOML file. This replaces long CLI invocations with a declarative config.
//!
//! Example:
//! ```toml
//! [job]
//! output = "part.nc"
//! post = "grbl"
//! spindle_speed = 18000
//! safe_z = 10.0
//!
//! [tools.flat_6mm]
//! type = "flat"
//! diameter = 6.35
//!
//! [[operation]]
//! type = "pocket"
//! input = "design.svg"
//! tool = "flat_6mm"
//! stepover = 2.0
//! depth = 6.0
//! depth_per_pass = 3.0
//! feed_rate = 1000
//! plunge_rate = 500
//! pattern = "zigzag"
//! ```

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use tracing::{debug, info, warn};

use rs_cam_core::{
    compute::ModelUnits,
    compute::catalog::{OperationConfig, OperationType},
    compute::config::{
        BoundaryConfig, DressupConfig, DressupEntryStyle, HeightsConfig, StockSource,
    },
    compute::tool_config::{ToolConfig, ToolId},
    debug_trace::ToolpathDebugOptions,
    gcode::CoolantMode,
    semantic_trace::ToolpathTraceArtifact,
    session::{LoadedModel, ProjectSession, ToolpathConfig},
    toolpath::Toolpath,
};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliToolType {
    #[serde(alias = "endmill")]
    Flat,
    #[serde(alias = "ballnose")]
    Ball,
    #[serde(rename = "bullnose")]
    BullNose,
    #[serde(rename = "vbit")]
    VBit,
    TaperedBall,
}

impl fmt::Display for CliToolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flat => write!(f, "flat"),
            Self::Ball => write!(f, "ball"),
            Self::BullNose => write!(f, "bullnose"),
            Self::VBit => write!(f, "vbit"),
            Self::TaperedBall => write!(f, "tapered_ball"),
        }
    }
}

impl CliToolType {
    /// Map the job-file tool vocabulary onto the core `ToolType`.
    fn to_tool_type(self) -> rs_cam_core::compute::tool_config::ToolType {
        use rs_cam_core::compute::tool_config::ToolType;
        match self {
            Self::Flat => ToolType::EndMill,
            Self::Ball => ToolType::BallNose,
            Self::BullNose => ToolType::BullNose,
            Self::VBit => ToolType::VBit,
            Self::TaperedBall => ToolType::TaperedBallNose,
        }
    }
}

// ── TOML types ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct JobFile {
    pub job: JobConfig,
    #[serde(default)]
    pub tools: HashMap<String, ToolDef>,
    #[serde(default)]
    pub setup: Vec<SetupDef>,
    #[serde(default)]
    pub operation: Vec<OperationDef>,
}

/// A setup definition for multi-setup jobs. Each setup can have its own output file.
#[derive(Deserialize)]
pub struct SetupDef {
    pub name: String,
    /// Per-setup output file. If absent, uses the global job output.
    pub output: Option<PathBuf>,
}

#[derive(Deserialize)]
pub struct JobConfig {
    pub output: PathBuf,
    #[serde(default = "default_post")]
    pub post: String,
    #[serde(default = "default_spindle_speed")]
    pub spindle_speed: u32,
    #[serde(default = "default_safe_z")]
    pub safe_z: f64,
    pub view: Option<PathBuf>,
    pub svg: Option<PathBuf>,
    #[serde(default)]
    pub simulate: bool,
    #[serde(default = "default_sim_resolution")]
    pub sim_resolution: f64,
    #[serde(default)]
    pub diagnostics: bool,
    pub diagnostics_json: Option<PathBuf>,
}

fn default_post() -> String {
    "grbl".into()
}
fn default_spindle_speed() -> u32 {
    18000
}
fn default_safe_z() -> f64 {
    10.0
}
fn default_sim_resolution() -> f64 {
    0.25
}

#[derive(Deserialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub tool_type: CliToolType,
    pub number: Option<u32>,
    pub diameter: f64,
    /// Number of cutting flutes (default: 2). Used for chipload calculation in diagnostics.
    pub flute_count: Option<u32>,
    /// Corner radius for bull nose
    pub corner_radius: Option<f64>,
    /// Included angle in degrees for V-bit
    pub included_angle: Option<f64>,
    /// Taper half-angle for tapered ball
    pub taper_angle: Option<f64>,
    /// Shaft diameter for tapered ball
    pub shaft_diameter: Option<f64>,
    /// Shank diameter above the cutting flutes (mm).
    pub shank_diameter: Option<f64>,
    /// Shank length above the cutting flutes (mm).
    pub shank_length: Option<f64>,
    /// Holder diameter (mm).
    pub holder_diameter: Option<f64>,
    /// Holder length (mm).
    #[allow(dead_code)]
    pub holder_length: Option<f64>,
}

#[derive(Deserialize)]
pub struct OperationDef {
    #[serde(rename = "type")]
    pub op_type: String,
    pub input: PathBuf,
    pub tool: String,
    /// Which setup this operation belongs to. If absent, belongs to a default setup.
    #[serde(default)]
    pub setup: Option<String>,

    // Common parameters (override job defaults if present)
    pub stepover: Option<f64>,
    pub depth: Option<f64>,
    pub depth_per_pass: Option<f64>,
    pub feed_rate: Option<f64>,
    pub plunge_rate: Option<f64>,
    pub safe_z: Option<f64>,
    pub spindle_speed: Option<u32>,
    #[serde(default)]
    pub coolant: CoolantMode,

    // Pocket-specific
    pub pattern: Option<String>,
    pub angle: Option<f64>,
    pub climb: Option<bool>,
    pub entry: Option<String>,

    // Profile-specific
    pub side: Option<String>,
    pub tabs: Option<usize>,
    pub tab_width: Option<f64>,
    pub tab_height: Option<f64>,

    // Dogbone
    pub dogbone: Option<bool>,

    // Adaptive-specific
    pub tolerance: Option<f64>,
    pub slot_clearing: Option<bool>,
    pub min_cutting_radius: Option<f64>,
    pub z_blend: Option<bool>,

    // Rest machining-specific
    pub prev_tool: Option<String>,

    // STL scaling
    pub scale: Option<f64>,

    // 3D adaptive-specific
    pub stock_top_z: Option<f64>,
    pub stock_to_leave: Option<f64>,
    /// Entry style for 3D ops. Accepts both `entry` and legacy `entry_style`.
    #[serde(alias = "entry_style")]
    pub entry_3d: Option<String>,
    pub fine_stepdown: Option<f64>,
    pub detect_flat_areas: Option<bool>,
    pub max_stay_down_dist: Option<f64>,
    pub order_by: Option<String>,
    /// Clearing strategy: "agent" (default) or "contour"/"contour_parallel".
    pub strategy: Option<String>,
    pub mill_shallow_areas: Option<bool>,
    pub shallow_angle_deg: Option<f64>,
    pub shallow_stepdown: Option<f64>,
    /// F-038: minimum forecast cut length (mm) for a marching-squares region
    /// to be retained in AgentSearch. Default 5.0 mm. Set to 0.0 to disable.
    pub min_region_cut_length_mm: Option<f64>,
    /// F-038b: maximum XY stay-down link distance (mm) between cut groups.
    /// `None` ⇒ planner defaults to 8 × tool diameter. `Some(0.0)` disables.
    pub max_stay_down_distance_mm: Option<f64>,
    /// F-038b: vertical clearance (mm) above the heightfield sample max
    /// when emitting a keep-tool-down link. Default 0.5 mm.
    pub stay_down_clearance_mm: Option<f64>,
}

// ── Parsing ────────────────────────────────────────────────────────────

pub fn parse_job_file(path: &Path) -> Result<JobFile> {
    let content = std::fs::read_to_string(path)
        .context(format!("Failed to read job file: {}", path.display()))?;
    let job: JobFile = toml::from_str(&content).context("Failed to parse TOML job file")?;

    if job.operation.is_empty() {
        bail!("Job file has no [[operation]] entries");
    }
    // Validate all tools referenced by operations exist
    for (i, op) in job.operation.iter().enumerate() {
        if !job.tools.contains_key(&op.tool) {
            bail!(
                "Operation {} references unknown tool '{}'. Available: {:?}",
                i,
                op.tool,
                job.tools.keys().collect::<Vec<_>>()
            );
        }
        if let Some(ref setup_name) = op.setup
            && !job.setup.is_empty()
            && !job.setup.iter().any(|setup| setup.name == *setup_name)
        {
            bail!(
                "Operation {} references unknown setup '{}'. Available: {:?}",
                i,
                setup_name,
                job.setup
                    .iter()
                    .map(|setup| &setup.name)
                    .collect::<Vec<_>>()
            );
        }
    }

    Ok(job)
}

// ── Tool construction ──────────────────────────────────────────────────

/// Default cutting length when not specified in the job file.
const DEFAULT_CUTTING_LENGTH_FACTOR: f64 = 4.0;

fn build_tool(def: &ToolDef) -> Result<rs_cam_core::tool::ToolDefinition> {
    use rs_cam_core::tool::{
        BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill,
        ToolDefinition, VBitEndmill,
    };
    let d = def.diameter;
    let cl = d * DEFAULT_CUTTING_LENGTH_FACTOR;
    let cutter: Box<dyn MillingCutter> = match def.tool_type {
        CliToolType::Flat => Box::new(FlatEndmill::new(d, cl)),
        CliToolType::Ball => Box::new(BallEndmill::new(d, cl)),
        CliToolType::BullNose => {
            let cr = def
                .corner_radius
                .context("Bull nose tool requires 'corner_radius'")?;
            Box::new(BullNoseEndmill::new(d, cr, cl))
        }
        CliToolType::VBit => {
            let angle = def
                .included_angle
                .context("V-bit tool requires 'included_angle'")?;
            Box::new(VBitEndmill::new(d, angle, cl))
        }
        CliToolType::TaperedBall => {
            let taper = def
                .taper_angle
                .context("Tapered ball requires 'taper_angle'")?;
            let shaft = def
                .shaft_diameter
                .context("Tapered ball requires 'shaft_diameter'")?;
            Box::new(TaperedBallEndmill::new(d, taper, shaft, cl))
        }
    };
    Ok(ToolDefinition::new(
        cutter,
        def.shank_diameter.unwrap_or(d),
        def.shank_length.unwrap_or(0.0),
        def.holder_diameter.unwrap_or(d * 2.0),
        def.shank_length.unwrap_or(0.0) + cl + 20.0, // default stickout
        def.flute_count.unwrap_or(2),
        rs_cam_core::compute::ToolMaterial::Carbide,
    ))
}

// ── Operation execution ────────────────────────────────────────────────

/// Result of a single operation within a job.
pub struct OpResult {
    pub toolpath: Toolpath,
    pub cutter: rs_cam_core::tool::ToolDefinition,
    pub label: String,
    pub spindle_speed: u32,
    pub tool_number: Option<u32>,
    pub coolant: CoolantMode,
    /// Number of cutting flutes on the tool (for chipload calculation).
    pub flute_count: u32,
    /// Which setup this operation belongs to (None = default/single setup).
    pub setup_name: Option<String>,
}

/// Result of executing a full job: combined toolpath + per-operation results.
pub struct JobResult {
    pub combined: Toolpath,
    pub phases: Vec<OpResult>,
    pub trace_artifacts: Vec<ToolpathTraceArtifact>,
}

pub fn execute_job(job: &JobFile, job_dir: &Path, debug_trace: bool) -> Result<JobResult> {
    let mut combined = Toolpath::new();
    let mut phases = Vec::new();
    let mut next_tool_number = 1u32;
    let mut tool_numbers = HashMap::new();
    let mut trace_artifacts = Vec::new();

    for (i, op) in job.operation.iter().enumerate() {
        info!(index = i, op_type = %op.op_type, "=== Operation ===");

        let tool_def = job
            .tools
            .get(&op.tool)
            .context(format!("Tool '{}' not found in [tools] table", op.tool))?;
        let tool_number = Some(*tool_numbers.entry(op.tool.clone()).or_insert_with(|| {
            tool_def.number.unwrap_or_else(|| {
                let assigned = next_tool_number;
                next_tool_number += 1;
                assigned
            })
        }));
        debug!(tool = %op.tool, diameter_mm = tool_def.diameter, tool_type = %tool_def.tool_type, "Tool");

        let output = execute_op_via_session(job, job_dir, i, op, tool_def, debug_trace)
            .with_context(|| format!("operation {i} ({})", op.op_type))?;
        if let Some(artifact) = output.trace {
            trace_artifacts.push(artifact);
        }
        let tp = output.toolpath;

        info!(
            moves = tp.moves.len(),
            cutting_mm = format!("{:.1}", tp.total_cutting_distance()),
            rapid_mm = format!("{:.1}", tp.total_rapid_distance()),
            "Operation result"
        );

        combined.moves.extend(tp.moves.clone());

        let label = format!(
            "Op {} \u{2014} {} ({:.2}mm {})",
            i, op.op_type, tool_def.diameter, tool_def.tool_type
        );
        let phase_cutter = build_tool(tool_def)?;
        let flute_count = tool_def.flute_count.unwrap_or(2);
        phases.push(OpResult {
            toolpath: tp,
            cutter: phase_cutter,
            label,
            spindle_speed: op.spindle_speed.unwrap_or(job.job.spindle_speed),
            tool_number,
            coolant: op.coolant,
            flute_count,
            setup_name: op.setup.clone(),
        });
    }

    Ok(JobResult {
        combined,
        phases,
        trace_artifacts,
    })
}

// \u{2500}\u{2500} Session-backed execution (T9 PR 2, plan \u{a7}7.1) \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}
//
// The pre-T9 router re-implemented six operations against the raw
// algorithm APIs (its own depth stepping, dressup application, entry
// handling) \u{2014} a third execution path beside the GUI worker and the
// session pipeline. Every operation now maps onto `OperationConfig`
// and executes through `ProjectSession::generate_toolpath` \u{2192}
// `execute_operation_annotated`: ONE execution path, and job params
// apply through the same registry-validated serde round-trip the
// GUI/MCP use.

struct SessionOpOutput {
    toolpath: Toolpath,
    trace: Option<ToolpathTraceArtifact>,
}

/// The job-file operation vocabulary. Anything else points the user at
/// the generic registry-driven `run` subcommand (all 23 ops).
fn op_type_for(token: &str) -> Result<OperationType> {
    Ok(match token {
        "pocket" => OperationType::Pocket,
        "profile" => OperationType::Profile,
        "adaptive" => OperationType::Adaptive,
        "rest" => OperationType::Rest,
        "adaptive3d" => OperationType::Adaptive3d,
        "drop-cutter" | "drop_cutter" | "finish" => OperationType::DropCutter,
        other => bail!(
            "Unknown operation type '{other}'. Job files support: pocket, profile, adaptive, \
             rest, adaptive3d, drop-cutter. For other operations use `rs_cam_cli run <op>` \
             (see `run --list-ops`)."
        ),
    })
}

/// Build a session `ToolConfig` from a job-file tool definition.
fn tool_config_from_def(def: &ToolDef, name: &str) -> ToolConfig {
    let mut tc = ToolConfig::new_default(ToolId(0), def.tool_type.to_tool_type());
    tc.name = name.to_owned();
    tc.diameter = def.diameter;
    tc.cutting_length = def.diameter * DEFAULT_CUTTING_LENGTH_FACTOR;
    if let Some(n) = def.number {
        tc.tool_number = n;
    }
    if let Some(fc) = def.flute_count {
        tc.flute_count = fc;
    }
    if let Some(cr) = def.corner_radius {
        tc.corner_radius = cr;
    }
    if let Some(ia) = def.included_angle {
        tc.included_angle = ia;
    }
    if let Some(ta) = def.taper_angle {
        tc.taper_half_angle = ta;
    }
    if let Some(sd) = def.shaft_diameter {
        tc.shaft_diameter = sd;
    }
    if let Some(sd) = def.shank_diameter {
        tc.shank_diameter = sd;
    }
    if let Some(sl) = def.shank_length {
        tc.shank_length = sl;
    }
    if let Some(hd) = def.holder_diameter {
        tc.holder_diameter = hd;
    }
    tc
}

fn execute_op_via_session(
    job: &JobFile,
    job_dir: &Path,
    i: usize,
    op: &OperationDef,
    tool_def: &ToolDef,
    debug_trace: bool,
) -> Result<SessionOpOutput> {
    let op_type = op_type_for(&op.op_type)?;
    let mut session = ProjectSession::new_empty();

    // \u{2500}\u{2500} Model \u{2500}\u{2500}
    let model_name = op
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("op_{i}_model"));
    // Pre-T9 only the STL ops honored `scale`; keep that scoping.
    let units = op.scale.map(ModelUnits::Custom);
    let model = LoadedModel::from_file(0, &model_name, &op.input, None, units, job_dir)
        .map_err(|e| anyhow::anyhow!("loading input '{}': {e}", op.input.display()))?;
    session.add_model(model);

    // Adaptive3d stock-frame fidelity: the pre-T9 router defaulted the
    // stock top to `model_top + 5.0` when `stock_top_z` was unset.
    if op_type == OperationType::Adaptive3d {
        let bbox = session.models().first().and_then(LoadedModel::bbox);
        if let Some(bbox) = bbox {
            let top = op.stock_top_z.unwrap_or(bbox.max.z + 5.0);
            let stock = session.stock_mut();
            stock.auto_from_model = false;
            stock.z = (top - stock.origin_z).max(0.0);
        }
    }

    // \u{2500}\u{2500} Tools \u{2500}\u{2500}
    let tool_idx = session.add_tool(tool_config_from_def(tool_def, &op.tool));
    let prev_tool_id = if op_type == OperationType::Rest {
        let prev_name = op
            .prev_tool
            .as_ref()
            .context("Rest requires 'prev_tool' referencing the larger tool")?;
        let prev_def = job.tools.get(prev_name).context(format!(
            "Rest 'prev_tool' references unknown tool '{prev_name}'"
        ))?;
        Some(session.add_tool(tool_config_from_def(prev_def, prev_name)))
    } else {
        None
    };

    // \u{2500}\u{2500} Dressups (entry / dogbone were post-passes pre-T9) \u{2500}\u{2500}
    let mut dressups = DressupConfig::default();
    if matches!(
        op_type,
        OperationType::Pocket | OperationType::Profile | OperationType::Adaptive
    ) && let Some(entry) = op.entry.as_deref()
    {
        match entry {
            "plunge" => {}
            "ramp" => {
                dressups.entry_style = DressupEntryStyle::Ramp;
                dressups.ramp_angle = 3.0;
            }
            "helix" => {
                dressups.entry_style = DressupEntryStyle::Helix;
                dressups.helix_radius = 2.0;
                dressups.helix_pitch = 1.0;
            }
            other => bail!("Unknown entry style '{other}'. Supported: plunge, ramp, helix"),
        }
    }
    if op.dogbone.unwrap_or(false) {
        dressups.dogbone = true;
        dressups.dogbone_angle = 170.0;
    }

    let debug_options = ToolpathDebugOptions {
        enabled: debug_trace,
    };

    let tool_id = session
        .tools()
        .get(tool_idx)
        .context("tool index out of bounds after add_tool")?
        .id
        .0;
    let tp_index = session
        .add_toolpath(
            0,
            ToolpathConfig {
                id: rs_cam_core::ToolpathId(0),
                name: format!("op_{}_{}", i, op_type.kind_str()),
                enabled: true,
                operation: OperationConfig::new_default(op_type),
                dressups,
                heights: HeightsConfig::default(),
                tool_id,
                model_id: 0,
                pre_gcode: None,
                post_gcode: None,
                boundary: BoundaryConfig::default(),
                boundary_inherit: true,
                stock_source: StockSource::default(),
                coolant: op.coolant,
                face_selection: None,
                debug_options,
                feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
            },
        )
        .map_err(|e| anyhow::anyhow!("adding toolpath: {e}"))?;

    // \u{2500}\u{2500} Parameters: registry-validated serde round-trip \u{2500}\u{2500}
    let drop_cutter_min_z = (op_type == OperationType::DropCutter)
        .then(|| session.models().first().and_then(LoadedModel::bbox))
        .flatten()
        .map(|bbox| bbox.min.z);
    let params = job_params_for(op, op_type, prev_tool_id, drop_cutter_min_z)?;
    let table = op_type.registry_entry().param_defs;
    for (key, value) in params {
        if table.iter().any(|d| d.name == key) {
            session
                .set_toolpath_param(tp_index, key, value)
                .map_err(|e| anyhow::anyhow!("param '{key}': {e}"))?;
        } else {
            // Pre-T9 the flat OperationDef silently ignored fields the
            // op didn't read; keep that leniency but say so.
            warn!(
                param = key,
                op = op_type.kind_str(),
                "job param not applicable to this operation \u{2014} skipped"
            );
        }
    }

    let post = session.post_mut();
    post.format = job.job.post.clone();
    post.safe_z = op.safe_z.unwrap_or(job.job.safe_z);
    post.spindle_speed = op.spindle_speed.unwrap_or(job.job.spindle_speed);

    // \u{2500}\u{2500} Generate \u{2500}\u{2500}
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(tp_index, &cancel)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let result = session
        .get_result(tp_index)
        .context("generation produced no result")?;
    let toolpath = result.toolpath().clone();

    let trace = if debug_trace {
        let tp_name = format!("op_{}_{}", i, op_type.kind_str());
        let op_label = format!("Op {} \u{2014} {}", i, op.op_type);
        let tool_summary = format!("{:.2}mm {}", tool_def.diameter, tool_def.tool_type);
        let operation_json = session
            .get_toolpath_config(tp_index)
            .map(|tc| serde_json::to_value(&tc.operation).unwrap_or_default())
            .unwrap_or_default();
        let request_snapshot = serde_json::json!({
            "operation_index": i,
            "operation_type": op_type.kind_str(),
            "input": op.input.display().to_string(),
            "tool": op.tool,
            "tool_diameter": tool_def.diameter,
            "operation": operation_json,
        });
        Some(ToolpathTraceArtifact::new(
            rs_cam_core::ToolpathId(i),
            &tp_name,
            &op_label,
            &tool_summary,
            request_snapshot,
            result.debug_trace.clone(),
            result.semantic_trace.clone(),
        ))
    } else {
        None
    };

    Ok(SessionOpOutput { toolpath, trace })
}

/// Map the flat job-file fields onto registry param names, preserving
/// the pre-T9 router's per-op defaults verbatim.
fn job_params_for(
    op: &OperationDef,
    op_type: OperationType,
    prev_tool_id: Option<usize>,
    drop_cutter_min_z: Option<f64>,
) -> Result<Vec<(&'static str, serde_json::Value)>> {
    use serde_json::json;
    let mut p: Vec<(&'static str, serde_json::Value)> = vec![
        ("feed_rate", json!(op.feed_rate.unwrap_or(1000.0))),
        ("plunge_rate", json!(op.plunge_rate.unwrap_or(500.0))),
    ];
    if let Some(rpm) = op.spindle_speed {
        p.push(("spindle_rpm", json!(rpm)));
    }
    match op_type {
        OperationType::Pocket => {
            p.push(("depth", json!(op.depth.context("Pocket requires 'depth'")?)));
            p.push(("depth_per_pass", json!(op.depth_per_pass.unwrap_or(3.0))));
            p.push(("stepover", json!(op.stepover.unwrap_or(2.0))));
            p.push(("climb", json!(op.climb.unwrap_or(false))));
            p.push(("angle", json!(op.angle.unwrap_or(0.0))));
            p.push(("pattern", json!(op.pattern.as_deref().unwrap_or("contour"))));
        }
        OperationType::Profile => {
            p.push((
                "depth",
                json!(op.depth.context("Profile requires 'depth'")?),
            ));
            p.push(("depth_per_pass", json!(op.depth_per_pass.unwrap_or(3.0))));
            p.push(("climb", json!(op.climb.unwrap_or(false))));
            let side = match op.side.as_deref().unwrap_or("outside") {
                "inside" | "in" => "inside",
                _ => "outside",
            };
            p.push(("side", json!(side)));
            p.push(("tab_count", json!(op.tabs.unwrap_or(0))));
            p.push(("tab_width", json!(op.tab_width.unwrap_or(5.0))));
            p.push(("tab_height", json!(op.tab_height.unwrap_or(2.0))));
        }
        OperationType::Adaptive => {
            p.push((
                "depth",
                json!(op.depth.context("Adaptive requires 'depth'")?),
            ));
            p.push(("depth_per_pass", json!(op.depth_per_pass.unwrap_or(3.0))));
            p.push(("stepover", json!(op.stepover.unwrap_or(2.0))));
            p.push(("tolerance", json!(op.tolerance.unwrap_or(0.1))));
            p.push(("slot_clearing", json!(op.slot_clearing.unwrap_or(false))));
            p.push((
                "min_cutting_radius",
                json!(op.min_cutting_radius.unwrap_or(0.0)),
            ));
            // Pre-T9 router hardcoded the hybrid cleanup strategy.
            p.push(("cleanup_strategy", json!("ContourParallelHybrid")));
        }
        OperationType::Rest => {
            p.push(("depth", json!(op.depth.context("Rest requires 'depth'")?)));
            p.push(("depth_per_pass", json!(op.depth_per_pass.unwrap_or(3.0))));
            p.push(("stepover", json!(op.stepover.unwrap_or(1.0))));
            p.push(("angle", json!(op.angle.unwrap_or(0.0))));
            p.push(("prev_tool_id", json!(prev_tool_id)));
        }
        OperationType::Adaptive3d => {
            p.push(("stepover", json!(op.stepover.unwrap_or(2.0))));
            p.push(("depth_per_pass", json!(op.depth_per_pass.unwrap_or(3.0))));
            let stl = op.stock_to_leave.unwrap_or(0.5);
            p.push(("stock_to_leave_radial", json!(stl)));
            p.push(("stock_to_leave_axial", json!(stl)));
            p.push(("tolerance", json!(op.tolerance.unwrap_or(0.1))));
            p.push((
                "min_cutting_radius",
                json!(op.min_cutting_radius.unwrap_or(0.0)),
            ));
            // entry_3d (with legacy alias) falls back to the shared 2D
            // `entry` field, matching the pre-T9 router.
            let entry = op
                .entry_3d
                .as_deref()
                .or(op.entry.as_deref())
                .unwrap_or("plunge");
            match entry {
                "helix" => {
                    p.push(("entry_style", json!("helix")));
                    // Pre-T9: radius = 0.8 \u{d7} tool radius = 0.4 \u{d7} envelope
                    // diameter (the config factor is diameter-relative).
                    p.push(("helix_radius_factor", json!(0.4)));
                    p.push(("helix_pitch", json!(1.0)));
                }
                "ramp" => {
                    p.push(("entry_style", json!("ramp")));
                    p.push(("ramp_angle_deg", json!(3.0)));
                }
                _ => p.push(("entry_style", json!("plunge"))),
            }
            let ordering = match op.order_by.as_deref().unwrap_or("global") {
                "by-area" | "by_area" | "byarea" => "by_area",
                _ => "global",
            };
            p.push(("region_ordering", json!(ordering)));
            let strategy = match op.strategy.as_deref().unwrap_or("contour") {
                "adaptive" => "adaptive",
                "agent" | "agent_search" => "agent_search",
                _ => "contour_parallel",
            };
            p.push(("clearing_strategy", json!(strategy)));
            p.push(("z_blend", json!(op.z_blend.unwrap_or(false))));
            p.push((
                "detect_flat_areas",
                json!(op.detect_flat_areas.unwrap_or(false)),
            ));
            if let Some(fs) = op.fine_stepdown {
                p.push(("fine_stepdown", json!(fs)));
            }
            p.push((
                "mill_shallow_areas",
                json!(op.mill_shallow_areas.unwrap_or(false)),
            ));
            if let Some(a) = op.shallow_angle_deg {
                p.push(("shallow_angle_deg", json!(a)));
            }
            if let Some(s) = op.shallow_stepdown {
                p.push(("shallow_stepdown", json!(s)));
            }
            p.push((
                "min_region_cut_length_mm",
                json!(op.min_region_cut_length_mm.unwrap_or(15.0)),
            ));
            if let Some(d) = op.max_stay_down_distance_mm.or(op.max_stay_down_dist) {
                p.push(("max_stay_down_distance_mm", json!(d)));
            }
            p.push((
                "stay_down_clearance_mm",
                json!(op.stay_down_clearance_mm.unwrap_or(0.5)),
            ));
        }
        OperationType::DropCutter => {
            p.push(("stepover", json!(op.stepover.unwrap_or(1.0))));
            if let Some(min_z) = drop_cutter_min_z {
                p.push(("min_z", json!(min_z)));
            }
        }
        other => bail!("op_type_for returned unsupported {other:?}"),
    }
    Ok(p)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use rs_cam_core::tool::MillingCutter as _;

    #[test]
    fn test_tool_radius_uses_cutter_radius() {
        // Verify that build_tool returns cutters whose radius() matches
        // what the job executor would use (cutter.radius(), not diameter/2.0
        // from the TOML definition, which could differ for composite tools).
        let flat_def = ToolDef {
            tool_type: CliToolType::Flat,
            number: None,
            diameter: 6.0,
            flute_count: None,
            corner_radius: None,
            included_angle: None,
            taper_angle: None,
            shaft_diameter: None,
            shank_diameter: None,
            shank_length: None,
            holder_diameter: None,
            holder_length: None,
        };
        let cutter = build_tool(&flat_def).unwrap();
        assert!((cutter.radius() - 3.0).abs() < 1e-10);

        let ball_def = ToolDef {
            tool_type: CliToolType::Ball,
            number: None,
            diameter: 10.0,
            flute_count: None,
            corner_radius: None,
            included_angle: None,
            taper_angle: None,
            shaft_diameter: None,
            shank_diameter: None,
            shank_length: None,
            holder_diameter: None,
            holder_length: None,
        };
        let ball_cutter = build_tool(&ball_def).unwrap();
        assert!((ball_cutter.radius() - 5.0).abs() < 1e-10);

        // For tapered ball, diameter() returns shaft_diameter, so radius()
        // returns shaft_diameter / 2.0, which is correct for the effective
        // cutting envelope.
        let tapered_def = ToolDef {
            tool_type: CliToolType::TaperedBall,
            number: None,
            diameter: 6.0,
            flute_count: None,
            corner_radius: None,
            included_angle: None,
            taper_angle: Some(10.0),
            shaft_diameter: Some(12.0),
            shank_diameter: None,
            shank_length: None,
            holder_diameter: None,
            holder_length: None,
        };
        let tapered_cutter = build_tool(&tapered_def).unwrap();
        // radius() should return shaft_diameter / 2.0 = 6.0
        assert!((tapered_cutter.radius() - 6.0).abs() < 1e-10);
    }
}
