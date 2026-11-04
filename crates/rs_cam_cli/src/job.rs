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
//! type = "end_mill"
//! diameter = 6.35
//!
//! [[operation]]
//! type = "pocket"
//! input = "design.svg"
//! tool = "flat_6mm"
//! stepover = 2.0
//! floor = { from = "top", depth = 6.0 }
//! depth_per_pass = 3.0
//! feed_rate = 1000
//! plunge_rate = 500
//! pattern = "zigzag"
//!
//! [operation.dressups]
//! entry_style = "helix"
//! ```
//!
//! C2: `type` is the registry token (`run --list-ops`), and every other
//! operation key that is not a job key is a schema parameter
//! (`run --list-params <op>`). A parameter that the file does not set
//! keeps the registry default.

use anyhow::{Context, Result, bail};
use rs_cam_core::compute::tool_config::ToolFact;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use tracing::{debug, info, warn};

use rs_cam_core::{
    compute::catalog::OperationType,
    compute::config::DressupConfig,
    compute::tool_config::{ToolConfig, ToolId, ToolType},
    compute::{ModelUnits, build_cutter},
    dexel_stock::{StockCutDirection, TriDexelStock},
    gcode::{
        CoolantMode, ExportPhases, GcodePhase, PhaseTool, PostFormat, WizardOverlay, emit_checked,
    },
    geo::BoundingBox3,
    session::{
        AddModelArgs, AddToolArgs, AddToolpathArgs, Command, EditSelectionArgs, LoadedModel,
        ProjectSession, SetDressupFieldArgs, SetPostConfigArgs, SetStockConfigArgs,
        SetToolpathFloorArgs, SetToolpathParamArgs, ToolpathConfig,
    },
    stock::simulation_cut::{
        AirCutRatios as _, SimulationCutArtifact, SimulationCutIssueKind, SimulationCutTrace,
    },
    tool::MillingCutter as _,
    toolpath::Toolpath,
    trace::debug_trace::ToolpathDebugOptions,
    trace::semantic_trace::ToolpathTraceArtifact,
};

// ── TOML types ─────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize)]
pub struct JobFile {
    pub job: JobConfig,
    #[serde(default)]
    pub tools: HashMap<String, ToolDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub setup: Vec<SetupDef>,
    #[serde(default)]
    pub operation: Vec<OperationDef>,
}

/// A setup definition for multi-setup jobs. Each setup can have its own output file.
#[derive(Deserialize, Serialize)]
pub(crate) struct SetupDef {
    pub name: String,
    /// Per-setup output file. If absent, uses the global job output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<PathBuf>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct JobConfig {
    pub output: PathBuf,
    /// The post dialect. F028: the token goes through
    /// `PostFormat::from_token` at parse time, so an unknown token refuses
    /// the job file; no door downstream can fall back to GRBL.
    #[serde(default, deserialize_with = "post_format_from_token")]
    pub post: PostFormat,
    #[serde(default = "default_spindle_speed")]
    pub spindle_speed: u32,
    #[serde(default = "default_safe_z")]
    pub safe_z: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub svg: Option<PathBuf>,
    #[serde(default)]
    pub simulate: bool,
    #[serde(default = "default_sim_resolution")]
    pub sim_resolution: f64,
    #[serde(default)]
    pub diagnostics: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics_json: Option<PathBuf>,
    /// G11: the grblHAL export options, the keys MCP `export_gcode` and
    /// the export dialog write. Omitted: off (a job file has no machine
    /// profile to default from).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller_waits_for_spindle: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mist_output: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_drill_cycles: Option<bool>,
    /// G3: a job with `[[setup]]` tables writes one file per setup
    /// (omitted or `true`, the default since setups existed). `false`
    /// writes one file with an M0 pause between the setups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub one_file_per_setup: Option<bool>,
}

/// Read the `[job] post` token through the one core resolver, and refuse a
/// token that it does not know.
fn post_format_from_token<'de, D>(deserializer: D) -> Result<PostFormat, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let token = String::deserialize(deserializer)?;
    PostFormat::from_token(&token).ok_or_else(|| {
        let valid: Vec<&str> = PostFormat::ALL.iter().map(|f| f.to_token()).collect();
        serde::de::Error::custom(format!(
            "unknown post-processor '{token}'; supported: {}",
            valid.join(", ")
        ))
    })
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

/// One `[tools.<key>]` table. An unknown key refuses to parse: C1 (B18)
/// deleted `shank_length` and `holder_length`, and a silent ignore would
/// hide the edit.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDef {
    /// Cutter shape. CLI-08: one vocabulary for every surface — the
    /// canonical [`ToolType`] serde token, the same token `run --tool`
    /// and MCP `add_tool` take: `end_mill`, `ball_nose`, `bull_nose`,
    /// `v_bit`, `tapered_ball_nose`.
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    pub diameter: f64,
    /// K3: the number of cutting flutes, a fact table (`{ stated = 2 }`,
    /// the catalogue or estimated form). Absent: `Unknown` (planning uses
    /// the legacy default and the lineage warns).
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    pub flute_count: ToolFact<u32>,
    /// Corner radius for bull nose
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
    /// Included angle in degrees for V-bit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub included_angle: Option<f64>,
    /// K3: the taper half-angle of a tapered ball (degrees), a fact table.
    /// Required for `tapered_ball_nose`.
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    pub taper_angle: ToolFact<f64>,
    /// Shaft diameter for tapered ball
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shaft_diameter: Option<f64>,
    /// K3: the shank diameter above the cutting flutes (mm), a fact table.
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    pub shank_diameter: ToolFact<f64>,
    /// K3: the length from the holder face to the tip (mm), a fact table.
    /// Absent: `Unknown`, which the op's session and the job's simulation
    /// cutter both plan at the legacy default (F116 step 1).
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    pub stickout: ToolFact<f64>,
    /// K3: centre cutting, `{ stated = "yes" }` or `{ stated = "no",
    /// centre_core_diameter_mm = .., blade_protrusion_mm = .. }`. Absent:
    /// `Unknown` (the motion of a centre-cutting tool). The old key
    /// `non_centre_cutting` refuses.
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown",
        with = "rs_cam_core::compute::tool_config::centre_cutting_serde"
    )]
    pub centre_cutting: ToolFact<rs_cam_core::compute::tool_config::CentreCutting>,
}

/// One `[[operation]]` table.
///
/// C2: the job file reads the W5 operation schema. `type` is the registry
/// token of the op (`run --list-ops`). A key that is not one of the job
/// keys below ([`OperationDef::JOB_KEYS`]) is an operation parameter by
/// its schema name (`run --list-params <op>`), and the core setter
/// applies it. A parameter that the file does not set keeps the registry
/// default (`OperationConfig::new_default`). The `[operation.dressups]`
/// table holds dressup fields by their `DressupConfig::FIELD_DEFS` name.
/// [`parse_job_file`] refuses an unknown key.
#[derive(Deserialize, Serialize)]
pub struct OperationDef {
    /// The registry token of the op (`OperationType::kind_str`).
    #[serde(rename = "type")]
    pub op_type: String,
    pub input: PathBuf,
    pub tool: String,
    /// Which setup this operation belongs to. If absent, belongs to a default setup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    /// The post safe Z for this operation. Empty: the `[job]` value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safe_z: Option<f64>,
    #[serde(default)]
    pub coolant: CoolantMode,
    /// The model scale (`ModelUnits::Custom`). Empty: the file units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    /// The stock top Z. Empty: the session stock, which the model sizes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock_top_z: Option<f64>,
    /// The stock thickness (mm): the `z` of `set_stock_config` (S11). A
    /// drawing (DXF, SVG) has no thickness, so a job on a drawing states
    /// it; with no value every op of the job refuses. The thickness keeps
    /// the stock top at the drawing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock_z: Option<f64>,
    /// Dressup fields by their `DressupConfig::FIELD_DEFS` name.
    #[serde(default, skip_serializing_if = "toml::Table::is_empty")]
    pub dressups: toml::Table,
    /// The selection of the op (geometry selection P2.7): the
    /// `[operation.selection]` table in the shape of the project file's
    /// `operation.params.selection`. The `edit_toolpath_selection` command
    /// applies it, so an op that holds no selection refuses it by name.
    /// Absent: the batch rule ([`batch_selection`]). A new op on the GUI
    /// and MCP starts with nothing selected (W3h), but a batch has no
    /// picker, so the job cuts every item the op can cut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<rs_cam_core::selection::Selection>,
    /// The machining boundary of the op (F7): the `[operation.boundary]`
    /// table in the shape of the project file's `boundary` block
    /// (`enabled`, `source`, `containment`, `offset`). Empty: the draft
    /// boundary. The `set_boundary_config` command applies it, before the
    /// selections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary: Option<rs_cam_core::compute::config::BoundaryConfig>,
    /// The boundary selection of the op (F7, the twin of `selection`): the
    /// `[operation.boundary_selection]` table in the shape of the project
    /// file's selection. The `edit_toolpath_selection` command applies it
    /// with `target: boundary` after the boundary, so it composes with a
    /// `boundary` whose `source` is `selection`. On a boundary whose source
    /// is not a selection it refuses, as MCP does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary_selection: Option<rs_cam_core::selection::Selection>,
    /// The floor of a Pocket, Profile, Zigzag or Adaptive 2D (geometry
    /// selection P3.3), in the project-file spelling:
    /// `floor = { from = "top", depth = 6.0 }`. Empty: the default of the
    /// op. The `set_toolpath_floor` command applies it after the
    /// parameters, so an op with no floor refuses it by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor: Option<rs_cam_core::compute::floor::Floor>,
    /// Operation parameters by their schema name.
    #[serde(flatten)]
    pub params: toml::Table,
}

impl OperationDef {
    /// The keys that the job file reads itself. Every other key of an
    /// `[[operation]]` table is an operation parameter.
    pub(crate) const JOB_KEYS: &'static [&'static str] = &[
        "type",
        "input",
        "tool",
        "setup",
        "safe_z",
        "coolant",
        "scale",
        "stock_top_z",
        "stock_z",
        "dressups",
        "selection",
        "boundary",
        "boundary_selection",
        "floor",
    ];

    /// The registry op that `type` names, after the C2 check of every key.
    ///
    /// # Errors
    ///
    /// Refuses an unknown `type`, a parameter that the op's schema does not
    /// publish and a dressup key that `DressupConfig::FIELD_DEFS` does not
    /// name. The refusal names each bad key and the valid set.
    pub(crate) fn operation_type(&self) -> Result<OperationType> {
        use rs_cam_core::session::project_file::{RetiredParam, retired_file_param};
        let op_type = crate::run::parse_op_type(&self.op_type)?;
        let defs = op_type.registry_entry().params.defs;
        // GA2: a retired parameter takes the project loader's rule (the one
        // `RETIRED_VALUES` table): a value that left the format refuses with
        // the fix, and a value that changes nothing (`z_blend = false`) is
        // dropped. The parameters are read by the schema names only, so a
        // dropped key reaches no op.
        for (key, value) in &self.params {
            if let Some(RetiredParam::Refuse(replacement)) = retired_file_param(key, value) {
                bail!("{key} = {value} is retired. {replacement}.");
            }
        }
        let unknown: Vec<&str> = self
            .params
            .iter()
            .filter(|(key, value)| {
                !defs.iter().any(|d| d.name == key.as_str())
                    && retired_file_param(key, value) != Some(RetiredParam::Drop)
            })
            .map(|(key, _)| key.as_str())
            .collect();
        if !unknown.is_empty() {
            let valid: Vec<&str> = defs.iter().map(|d| d.name).collect();
            bail!(
                "operation '{}' has no parameter {}. Its parameters are: {}. The job keys are: {}",
                op_type.kind_str(),
                unknown.join(", "),
                valid.join(", "),
                Self::JOB_KEYS.join(", ")
            );
        }
        let unknown: Vec<&str> = self
            .dressups
            .keys()
            .map(String::as_str)
            .filter(|key| !DressupConfig::FIELD_DEFS.iter().any(|d| d.name == *key))
            .collect();
        if !unknown.is_empty() {
            let valid: Vec<&str> = DressupConfig::FIELD_DEFS.iter().map(|d| d.name).collect();
            bail!(
                "[operation.dressups] has no field {}. The dressup fields are: {}",
                unknown.join(", "),
                valid.join(", ")
            );
        }
        Ok(op_type)
    }
}

// ── The `job` subcommand ───────────────────────────────────────────────

/// Run a TOML job file: parse it, execute every operation, then write the
/// G-code, the SVG preview, the 3D viewer and the diagnostics the file
/// asks for. This is the whole `job` subcommand; `main` only dispatches.
pub fn run_job_command(
    input: &Path,
    diagnostics: bool,
    diagnostics_json: Option<PathBuf>,
    debug_trace: Option<PathBuf>,
) -> Result<()> {
    let job_path = input
        .canonicalize()
        .context(format!("Job file not found: {}", input.display()))?;
    let job_dir = job_path.parent().unwrap_or(Path::new("."));
    debug!(path = %job_path.display(), "Loading job file");

    let mut job_file = parse_job_file(&job_path)?;
    // CLI flags override TOML config
    if diagnostics {
        job_file.job.diagnostics = true;
    }
    if diagnostics_json.is_some() {
        job_file.job.diagnostics_json = diagnostics_json;
    }
    let debug_trace_dir = debug_trace.map(|p| if p.is_absolute() { p } else { job_dir.join(p) });

    info!(
        tools = job_file.tools.len(),
        operations = job_file.operation.len(),
        output = %job_file.job.output.display(),
        "Job loaded"
    );

    let job_result = execute_job(&job_file, job_dir, debug_trace_dir.is_some())?;
    let toolpath = &job_result.combined;

    // Export debug trace artifacts if requested
    if let Some(ref trace_dir) = debug_trace_dir {
        for (idx, artifact) in job_result.trace_artifacts.iter().enumerate() {
            let file_stem = format!(
                "{}-{}",
                idx,
                artifact.toolpath_name.replace(
                    |c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_',
                    "_",
                )
            );
            match rs_cam_core::trace::semantic_trace::write_toolpath_trace_artifact(
                trace_dir, &file_stem, artifact,
            ) {
                Ok(path) => info!(path = %path.display(), "Wrote debug trace artifact"),
                Err(e) => {
                    eprintln!(
                        "Warning: failed to write trace artifact for op {}: {}",
                        idx, e
                    );
                }
            }
        }
    }

    info!(
        moves = toolpath.moves.len(),
        cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
        rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
        "Total toolpath"
    );

    let output = if job_file.job.output.is_absolute() {
        job_file.job.output.clone()
    } else {
        job_dir.join(&job_file.job.output)
    };
    let svg = job_file.job.svg.as_ref().map(|p| {
        if p.is_absolute() {
            p.clone()
        } else {
            job_dir.join(p)
        }
    });

    // Emit G-code with per-operation spindle speed support
    let post_def = job_file.job.post.definition();
    let overlay = job_overlay(&job_file);
    // A job file with setups has always written one file per setup, on
    // every post; that stays the default.
    let split = job_file.job.one_file_per_setup.unwrap_or(true);
    if !job_file.setup.is_empty() && !split {
        // G3: one file, with an M0 pause between the setups.
        let groups: Vec<rs_cam_core::gcode::GcodeSetupPhase<'_>> = job_file
            .setup
            .iter()
            .map(|setup_def| rs_cam_core::gcode::GcodeSetupPhase {
                setup_label: &setup_def.name,
                phases: job_result
                    .phases
                    .iter()
                    .filter(|phase| phase.setup_name.as_deref() == Some(&setup_def.name))
                    .map(job_phase)
                    .collect(),
                pause_message: None,
            })
            .filter(|g| !g.phases.is_empty())
            .collect();
        let gcode = emit_job_program(
            &ExportPhases::Setups(groups),
            post_def,
            job_file.job.safe_z,
            &overlay,
        )?;
        std::fs::write(&output, &gcode).context("Failed to write output file")?;
        info!(bytes = gcode.len(), path = %output.display(), "Wrote G-code");
    } else if !job_file.setup.is_empty() {
        for setup_def in &job_file.setup {
            let setup_phases: Vec<GcodePhase<'_>> = job_result
                .phases
                .iter()
                .filter(|phase| phase.setup_name.as_deref() == Some(&setup_def.name))
                .map(job_phase)
                .collect();
            if setup_phases.is_empty() {
                continue;
            }

            let setup_output = setup_def
                .output
                .as_ref()
                .map(|path| {
                    if path.is_absolute() {
                        path.clone()
                    } else {
                        job_dir.join(path)
                    }
                })
                .unwrap_or_else(|| {
                    let name = setup_def.name.replace(' ', "_").to_lowercase();
                    output.with_file_name(format!(
                        "{}_{}.nc",
                        output.file_stem().unwrap_or_default().to_string_lossy(),
                        name
                    ))
                });

            let gcode = emit_job_program(
                &ExportPhases::OneSetup(setup_phases),
                post_def,
                job_file.job.safe_z,
                &overlay,
            )?;
            std::fs::write(&setup_output, &gcode).context("Failed to write setup output file")?;
            info!(
                setup = %setup_def.name,
                bytes = gcode.len(),
                path = %setup_output.display(),
                "Wrote setup G-code"
            );
        }
    } else {
        let phases: Vec<GcodePhase<'_>> = job_result.phases.iter().map(job_phase).collect();
        info!("Emitting G-code ({})...", post_def.name);
        let gcode = emit_job_program(
            &ExportPhases::OneSetup(phases),
            post_def,
            job_file.job.safe_z,
            &overlay,
        )?;
        std::fs::write(&output, &gcode).context("Failed to write output file")?;
        info!(bytes = gcode.len(), path = %output.display(), "Wrote G-code");
    }

    if let Some(svg_out) = &svg {
        let svg_content = rs_cam_core::export::viz::toolpath_to_svg(toolpath, 800.0, 600.0);
        std::fs::write(svg_out, &svg_content).context("Failed to write SVG file")?;
        info!(path = %svg_out.display(), "Wrote SVG preview");
    }

    if let Some(view) = &job_file.job.view {
        let view_path = if view.is_absolute() {
            view.clone()
        } else {
            job_dir.join(view)
        };
        let html = if job_file.job.simulate {
            // Stacked simulation: each operation is a phase with its own cutter
            let tp_bbox = toolpath_bbox(toolpath);
            let max_margin = job_result
                .phases
                .iter()
                .map(|p| p.cutter.radius())
                .fold(0.0_f64, f64::max);
            // Determine stock top: use stock_top_z from first 3D op, or bbox max + 5
            let stock_top = job_file
                .operation
                .iter()
                .find_map(|op| op.stock_top_z)
                .unwrap_or(tp_bbox.max.z + 5.0);
            let sim_bbox = BoundingBox3 {
                min: rs_cam_core::geo::P3::new(
                    tp_bbox.min.x - max_margin,
                    tp_bbox.min.y - max_margin,
                    tp_bbox.min.z,
                ),
                max: rs_cam_core::geo::P3::new(
                    tp_bbox.max.x + max_margin,
                    tp_bbox.max.y + max_margin,
                    stock_top,
                ),
            };
            let mut stock = TriDexelStock::from_bounds(&sim_bbox, job_file.job.sim_resolution);

            debug!(
                phases = job_result.phases.len(),
                resolution_mm = job_file.job.sim_resolution,
                "Running stacked simulation"
            );

            // Simulate each phase with its own cutter
            for phase in &job_result.phases {
                stock.simulate_toolpath(&phase.toolpath, &phase.cutter, StockCutDirection::FromTop);
            }
            debug!(
                cols = stock.z_grid.cols,
                rows = stock.z_grid.rows,
                phases = job_result.phases.len(),
                "Simulation stock generated"
            );

            // Try to load source mesh for overlay (from first STL-based operation)
            let source_mesh = job_file.operation.iter().find_map(|op| {
                let p = if op.input.is_absolute() {
                    op.input.clone()
                } else {
                    job_dir.join(&op.input)
                };
                let ext = p.extension()?.to_str()?.to_lowercase();
                if ext == "stl" {
                    match rs_cam_core::mesh::TriangleMesh::from_stl_scaled(&p, 1.0) {
                        Ok(m) => Some(m),
                        Err(e) => {
                            warn!("Failed to load overlay mesh {}: {e}", p.display());
                            None
                        }
                    }
                } else {
                    None
                }
            });

            use rs_cam_core::export::viz::SimPhase;
            let sim_phases: Vec<SimPhase> = job_result
                .phases
                .iter()
                .map(|p| SimPhase {
                    toolpath: &p.toolpath,
                    cutter: &p.cutter,
                    label: p.label.clone(),
                })
                .collect();

            rs_cam_core::export::viz::stacked_simulation_3d_html(
                &sim_phases,
                &stock,
                source_mesh.as_ref(),
            )
        } else {
            rs_cam_core::export::viz::toolpath_standalone_3d_html(toolpath, None)
        };
        std::fs::write(&view_path, &html).context("Failed to write 3D viewer file")?;
        info!(path = %view_path.display(), "Wrote 3D viewer");
    }

    // Diagnostics: run metric simulation and print report
    if job_file.job.diagnostics && job_file.job.simulate {
        let tp_bbox = toolpath_bbox(toolpath);
        let max_margin = job_result
            .phases
            .iter()
            .map(|p| p.cutter.radius())
            .fold(0.0_f64, f64::max);
        let stock_top = job_file
            .operation
            .iter()
            .find_map(|op| op.stock_top_z)
            .unwrap_or(tp_bbox.max.z + 5.0);
        let sim_bbox = BoundingBox3 {
            min: rs_cam_core::geo::P3::new(
                tp_bbox.min.x - max_margin,
                tp_bbox.min.y - max_margin,
                tp_bbox.min.z,
            ),
            max: rs_cam_core::geo::P3::new(
                tp_bbox.max.x + max_margin,
                tp_bbox.max.y + max_margin,
                stock_top,
            ),
        };
        let resolution = job_file.job.sim_resolution;
        let sample_step = resolution;
        let never_cancel = || false;
        let mut diag_stock = TriDexelStock::from_bounds(&sim_bbox, resolution);

        debug!("Running diagnostics metric simulation");

        let mut all_samples = Vec::new();
        let mut labels = Vec::new();
        for (idx, phase) in job_result.phases.iter().enumerate() {
            labels.push(phase.label.clone());
            let rapid_feed = 3000.0_f64; // typical rapid rate for diagnostics
            match diag_stock.simulate_toolpath_with_metrics_with_cancel(
                &phase.toolpath,
                &phase.cutter,
                StockCutDirection::FromTop,
                // The job runner numbers its phases: the phase index is the id.
                rs_cam_core::ToolpathId::from_wire(idx),
                phase.spindle_speed,
                phase.flute_count,
                rapid_feed,
                sample_step,
                None,
                &[],
                &[],
                true,
                &never_cancel,
            ) {
                Ok(samples) => all_samples.extend(samples),
                Err(_) => {
                    eprintln!("Diagnostics simulation cancelled for phase {}", idx);
                }
            }
        }

        let trace = SimulationCutTrace::from_samples(sample_step, all_samples);
        print_diagnostics_report(&trace, &labels);

        if let Some(json_path) = &job_file.job.diagnostics_json {
            let json_out = if json_path.is_absolute() {
                json_path.clone()
            } else {
                job_dir.join(json_path)
            };
            let artifact = SimulationCutArtifact::new(
                resolution,
                sample_step,
                [sim_bbox.min.x, sim_bbox.min.y, sim_bbox.min.z],
                [sim_bbox.max.x, sim_bbox.max.y, sim_bbox.max.z],
                (0..job_result.phases.len())
                    .map(rs_cam_core::ToolpathId::from_wire)
                    .collect(),
                serde_json::json!({"source": "cli_diagnostics"}),
                trace,
            );
            // G-SIMMEM: streamed, not built as one in-memory string.
            rs_cam_core::stock::simulation_cut::write_simulation_cut_artifact_to(
                &json_out, &artifact,
            )
            .context("Failed to write diagnostics JSON")?;
            info!(path = %json_out.display(), "Wrote diagnostics JSON");
        }
    } else if job_file.job.diagnostics && !job_file.job.simulate {
        eprintln!("Warning: --diagnostics requires simulate = true in the job file");
    }

    Ok(())
}

fn print_diagnostics_report(trace: &SimulationCutTrace, toolpath_labels: &[String]) {
    eprintln!();
    eprintln!("=== Simulation Diagnostics ===");
    eprintln!();

    for ts in &trace.toolpath_summaries {
        // Job-pipeline toolpath ids are minted from the phase index
        // (see the simulate loop), so indexing labels by id.0 is sound here.
        let label = toolpath_labels
            .get(ts.toolpath_id.raw())
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let air_runtime = trace.summary.total_runtime_s - ts.cutting_runtime_s - ts.rapid_runtime_s;
        // LH-1: name the denominator. This is air cut over TOTAL runtime
        // (cutting + rapids) - the measure every threshold in the codebase
        // uses; the cutting-time reading of the same seconds is larger.
        let air_pct_of_total = ts.air_cut_pct_of_total_runtime();

        eprintln!("Toolpath: {}", label);
        eprintln!(
            "  Runtime: {:.1}s (cutting: {:.1}s, rapid: {:.1}s, air: {:.1}s)",
            ts.total_runtime_s,
            ts.cutting_runtime_s,
            ts.rapid_runtime_s,
            air_runtime.max(0.0),
        );
        eprintln!(
            "  Air cut: {:.1}% of total runtime ({:.1}% of cutting time)",
            air_pct_of_total,
            ts.air_cut_pct_of_cutting_time()
        );
        eprintln!("  Avg engagement: {:.2}", ts.average_engagement);
        eprintln!(
            "  Peak commanded advance/tooth: {:.3} mm/tooth",
            ts.peak_chipload_mm_per_tooth
        );
        eprintln!("  Peak DOC: {:.1} mm", ts.peak_axial_doc_mm);
        eprintln!("  MRR avg: {:.1} mm3/s", ts.average_mrr_mm3_s);

        // Count issues for this toolpath
        let air_issues = trace
            .issues
            .iter()
            .filter(|i| i.toolpath_id == ts.toolpath_id && i.kind == SimulationCutIssueKind::AirCut)
            .count();
        let low_eng_issues = trace
            .issues
            .iter()
            .filter(|i| {
                i.toolpath_id == ts.toolpath_id && i.kind == SimulationCutIssueKind::LowEngagement
            })
            .count();
        if air_issues > 0 || low_eng_issues > 0 {
            let mut parts = Vec::new();
            if air_issues > 0 {
                parts.push(format!("{} air cuts", air_issues));
            }
            if low_eng_issues > 0 {
                parts.push(format!("{} low engagement", low_eng_issues));
            }
            // Census §1.4: these are COALESCED RUNS, not per-sample tallies,
            // and the two populations differ by ~43x on a real cut. Printing
            // a bare "N air cuts" invited the reader to size the problem off
            // a number that mostly measures how often the cutter crosses a
            // boundary. Naming the population costs one word.
            eprintln!("  Issue runs: {}", parts.join(", "));
        }
        eprintln!();
    }

    // Top hotspots by wasted time
    if !trace.hotspots.is_empty() {
        const MAX_HOTSPOT_ROWS: usize = 10;
        eprintln!("Top issues by wasted time:");
        for (i, hs) in trace.hotspots.iter().take(MAX_HOTSPOT_ROWS).enumerate() {
            let kind_label = if hs.air_cut_time_s > hs.low_engagement_time_s {
                "AirCut"
            } else {
                "LowEngagement"
            };
            let [x, y, z] = hs.representative_position;
            eprintln!(
                "  {}. {} at ({:.1}, {:.1}, {:.1}) -- {:.1}s wasted, engagement {:.2}",
                i + 1,
                kind_label,
                x,
                y,
                z,
                hs.wasted_runtime_s,
                hs.average_engagement,
            );
        }
        // R-6 (census §3.5 D9): this list has always silently stopped at ten
        // while every GUI list says how many it withheld. A reader who saw
        // exactly ten rows had no way to know whether that was the whole
        // truth or the top of a much longer tail.
        if let Some(hidden) = trace.hotspots.len().checked_sub(MAX_HOTSPOT_ROWS)
            && hidden > 0
        {
            eprintln!("  ... {hidden} more not shown");
        }
        eprintln!();
    }
}

fn toolpath_bbox(toolpath: &Toolpath) -> BoundingBox3 {
    let mut bbox = BoundingBox3::empty();
    for m in &toolpath.moves {
        bbox.expand_to(m.target);
    }
    bbox
}

// ── Parsing ────────────────────────────────────────────────────────────

pub fn parse_job_file(path: &Path) -> Result<JobFile> {
    let content = std::fs::read_to_string(path)
        .context(format!("Failed to read job file: {}", path.display()))?;
    check_job_tool_facts(&content)?;
    let job: JobFile = toml::from_str(&content).context("Failed to parse TOML job file")?;

    if job.operation.is_empty() {
        bail!("Job file has no [[operation]] entries");
    }
    // Validate all tools referenced by operations exist
    for (i, op) in job.operation.iter().enumerate() {
        // C2: every key is a job key, a schema parameter or a dressup field.
        let _ = op
            .operation_type()
            .with_context(|| format!("operation {i} ({})", op.op_type))?;
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

/// K3 (ruling Q9, lead decision D4): a job file follows the project-file
/// rule. A tool fact as a bare value refuses, and the one message names
/// every bare key (the job key `taper_angle` too) and the old key
/// `non_centre_cutting`.
fn check_job_tool_facts(content: &str) -> Result<()> {
    let raw: toml::Value = toml::from_str(content).context("Failed to parse TOML job file")?;
    let tools: Vec<(String, &toml::Value)> = raw
        .get("tools")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flatten()
        .map(|(name, t)| (format!("tools.{name}"), t))
        .collect();
    let mut bare = rs_cam_core::compute::tool_config::bare_fact_keys(
        tools.iter().map(|(p, t)| (p.clone(), *t)),
    );
    bare.extend(
        tools
            .iter()
            .filter(|(_, t)| t.get("taper_angle").is_some_and(|v| !v.is_table()))
            .map(|(p, _)| format!("{p}.taper_angle")),
    );
    let mut parts = Vec::new();
    if !bare.is_empty() {
        parts.push(rs_cam_core::compute::tool_config::bare_facts_text(&bare));
    }
    let retired: Vec<String> = tools
        .iter()
        .filter(|(_, t)| t.get("non_centre_cutting").is_some())
        .map(|(p, _)| format!("{p}.non_centre_cutting"))
        .collect();
    if !retired.is_empty() {
        parts.push(format!(
            "retired key {}: write centre_cutting = {{ stated = \"no\", centre_core_diameter_mm \
             = .., blade_protrusion_mm = .. }} or {{ stated = \"yes\" }}",
            retired.join(", ")
        ));
    }
    if parts.is_empty() {
        Ok(())
    } else {
        bail!("job file: {}", parts.join("; "))
    }
}

// ── Tool construction ──────────────────────────────────────────────────

/// Default cutting length when not specified in the job file.
const DEFAULT_CUTTING_LENGTH_FACTOR: f64 = 4.0;

/// K3: the basis of the job tool's flute length.
const CUTTING_LENGTH_BASIS: &str = "job file: four times the diameter (no flute-length key)";

/// The cutter of a job-file tool, for the job's own simulation and
/// diagnostics.
///
/// F116 step 1: the CLI held its own cutter builder (`build_tool`). It set
/// no helix angle, hard-coded carbide and took a 4×D stickout, while the
/// session tool of the same op took the `ToolConfig` values. This function
/// builds the cutter with core [`build_cutter`] from the same record that
/// the op's session receives, so the job refuses an out-of-domain tool with
/// the `ToolConfig::validate` sentence that every other door gives.
fn phase_cutter(tool: &ToolConfig) -> Result<rs_cam_core::tool::ToolDefinition> {
    build_cutter(tool).with_context(|| format!("tool '{}'", tool.name))
}

// ── Operation execution ────────────────────────────────────────────────

/// Result of a single operation within a job.
pub(crate) struct OpResult {
    pub toolpath: Toolpath,
    pub cutter: rs_cam_core::tool::ToolDefinition,
    pub label: String,
    pub spindle_speed: u32,
    /// Stable tool identity within the job: index of first appearance
    /// of the tool's `[tools]` key. Distinct tools always get distinct
    /// ids even when their display `tool_number`s collide.
    pub tool_id: Option<rs_cam_core::ids::ToolId>,
    pub tool_number: Option<u32>,
    /// The tool's `[tools]` key — used as the operator-facing label in
    /// tool-change messages.
    pub tool_name: String,
    pub coolant: CoolantMode,
    /// Number of cutting flutes on the tool (for chipload calculation).
    pub flute_count: u32,
    /// Which setup this operation belongs to (None = default/single setup).
    pub setup_name: Option<String>,
    /// G7: the drill cycle, for the G82 dwell and native cycles.
    pub drill: Option<rs_cam_core::gcode::PhaseDrill>,
}

/// Result of executing a full job: combined toolpath + per-operation results.
pub struct JobResult {
    pub combined: Toolpath,
    pub phases: Vec<OpResult>,
    pub trace_artifacts: Vec<ToolpathTraceArtifact>,
}

/// G11: the controller options of a job file, resolved by the one core
/// rule. The job file has no machine profile, so an omitted option is off.
pub(crate) fn job_overlay(job_file: &JobFile) -> WizardOverlay {
    let post = rs_cam_core::gcode::PostConfig {
        controller_waits_for_spindle: job_file.job.controller_waits_for_spindle,
        mist_output: job_file.job.mist_output,
        native_drill_cycles: job_file.job.native_drill_cycles,
        ..rs_cam_core::gcode::PostConfig::default()
    };
    let mut overlay = WizardOverlay::default();
    post.apply_controller_options(
        &rs_cam_core::machine::MachineProfile::default(),
        &mut overlay,
    );
    overlay
}

/// The checked emit of a job program, the one job-file door (`run_job`
/// and `sweep`).
///
/// F116 (not done): the job-file pipeline has no `ProjectSession`, so it
/// judges no load and no finding (no warning line), and it runs neither
/// the machine-safety pass nor the high-feedrate rewrite: both read the
/// session (retract plane, machine feed limit, post config).
pub(crate) fn emit_job_program(
    phases: &ExportPhases<'_>,
    post: &rs_cam_core::gcode::PostDefinition,
    safe_z: f64,
    overlay: &WizardOverlay,
) -> Result<String> {
    // K6: the job file has no session, so no finding is judged here and
    // the program header holds no warning line.
    Ok(emit_checked(phases, post, safe_z, overlay, &[])?)
}

/// The emitter phase of one job operation.
pub(crate) fn job_phase(phase: &OpResult) -> GcodePhase<'_> {
    GcodePhase {
        toolpath: &phase.toolpath,
        spindle_rpm: phase.spindle_speed,
        label: &phase.label,
        pre_gcode: None,
        post_gcode: None,
        tool: phase
            .tool_id
            .zip(phase.tool_number)
            .map(|(id, number)| PhaseTool {
                id,
                number,
                label: &phase.tool_name,
            }),
        coolant: phase.coolant,
        controller_compensation: None,
        drill: phase.drill,
        // W2.5 (D10): a job file generates and emits; no simulation
        // modulates its feeds.
        feeds: Some(rs_cam_core::session::FeedStage::Planned.statement()),
    }
}

pub fn execute_job(job: &JobFile, job_dir: &Path, debug_trace: bool) -> Result<JobResult> {
    let mut combined = Toolpath::new();
    let mut phases = Vec::new();
    let mut next_tool_number = 1u32;
    let mut tool_numbers = HashMap::new();
    // Tool identity: one stable id per distinct `[tools]` key, assigned
    // in order of first use. Tool-change detection keys on this id, not
    // on the (possibly colliding) display tool_number.
    let mut tool_ids: HashMap<String, usize> = HashMap::new();
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
        let next_tool_id = tool_ids.len();
        // The job runner numbers its tools: a job-file name becomes an id here.
        let tool_id = Some(rs_cam_core::ids::ToolId::from_wire(
            *tool_ids.entry(op.tool.clone()).or_insert(next_tool_id),
        ));
        debug!(tool = %op.tool, diameter_mm = tool_def.diameter, tool_type = tool_def.tool_type.serde_token(), "Tool");

        let output = execute_op_via_session(job, job_dir, i, op, tool_def, debug_trace)
            .with_context(|| format!("operation {i} ({})", op.op_type))?;
        if let Some(artifact) = output.trace {
            trace_artifacts.push(artifact);
        }
        let tp = output.toolpath;
        let drill = output.drill;
        let spindle_speed = output.spindle_rpm.unwrap_or(job.job.spindle_speed);

        info!(
            moves = tp.moves.len(),
            cutting_mm = format!("{:.1}", tp.total_cutting_distance()),
            rapid_mm = format!("{:.1}", tp.total_rapid_distance()),
            "Operation result"
        );

        combined.moves.extend(tp.moves.clone());

        let label = format!(
            "Op {} \u{2014} {} ({:.2}mm {})",
            i,
            op.op_type,
            tool_def.diameter,
            tool_def.tool_type.serde_token()
        );
        let tool_config = tool_config_from_def(tool_def, &op.tool)?;
        let phase_cutter = phase_cutter(&tool_config)?;
        let flute_count = tool_config.flute_count_planning();
        phases.push(OpResult {
            toolpath: tp,
            cutter: phase_cutter,
            label,
            spindle_speed,
            tool_id,
            tool_number,
            tool_name: op.tool.clone(),
            coolant: op.coolant,
            flute_count,
            setup_name: op.setup.clone(),
            drill,
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
    /// G7: the drill cycle, for the G82 dwell and native cycles.
    drill: Option<rs_cam_core::gcode::PhaseDrill>,
    /// The op's own `spindle_rpm`. Empty: the `[job]` spindle speed.
    spindle_rpm: Option<u32>,
}

/// Build a session `ToolConfig` from a job-file tool definition.
///
/// The job file must state the geometry that defines its tool type (the
/// corner radius of a bull nose, the included angle of a V-bit, the taper
/// and shaft of a tapered ball). A missing key is refused; the job does not
/// take the `ToolConfig` default for it.
fn tool_config_from_def(def: &ToolDef, name: &str) -> Result<ToolConfig> {
    let required: &[(&str, Option<f64>)] = match def.tool_type {
        ToolType::EndMill | ToolType::BallNose => &[],
        ToolType::BullNose => &[("corner_radius", def.corner_radius)],
        ToolType::VBit => &[("included_angle", def.included_angle)],
        ToolType::TaperedBallNose => &[
            ("taper_angle", def.taper_angle.planning().map(|(v, _)| *v)),
            ("shaft_diameter", def.shaft_diameter),
        ],
    };
    let missing: Vec<&str> = required
        .iter()
        .filter(|(_, value)| value.is_none())
        .map(|(key, _)| *key)
        .collect();
    if !missing.is_empty() {
        bail!(
            "tool '{name}' ({}) requires {}",
            def.tool_type.serde_token(),
            missing.join(", ")
        );
    }
    let mut tc = ToolConfig::new_default(ToolId::placeholder(), def.tool_type);
    tc.name = name.to_owned();
    tc.diameter = def.diameter;
    // K3: the job file has no flute-length key; the length is an estimate.
    tc.set_cutting_length(ToolFact::estimated(
        def.diameter * DEFAULT_CUTTING_LENGTH_FACTOR,
        CUTTING_LENGTH_BASIS,
    ));
    if let Some(n) = def.number {
        tc.tool_number = n;
    }
    // K3: each fact as the job file states it; an absent key is Unknown.
    tc.set_flute_count(def.flute_count.clone());
    if let Some(cr) = def.corner_radius {
        tc.corner_radius = cr;
    }
    if let Some(ia) = def.included_angle {
        tc.included_angle = ia;
    }
    if !def.taper_angle.is_unknown() {
        tc.set_taper_half_angle(def.taper_angle.clone());
    }
    if let Some(sd) = def.shaft_diameter {
        tc.shaft_diameter = sd;
    }
    tc.set_shank_diameter(def.shank_diameter.clone());
    tc.set_stickout(def.stickout.clone());
    tc.set_centre_cutting(def.centre_cutting.clone());
    Ok(tc)
}

/// The session of one job op: the model, the stock keys, the tool, the op
/// (THE draft door, then the job keys through the core setters) and the
/// post. Nothing is generated. The S11 parity test reads it.
fn job_op_session(
    job: &JobFile,
    job_dir: &Path,
    i: usize,
    op: &OperationDef,
    tool_def: &ToolDef,
    debug_trace: bool,
) -> Result<(ProjectSession, usize)> {
    let op_type = op.operation_type()?;
    let mut session = ProjectSession::new_empty();

    // \u{2500}\u{2500} Model \u{2500}\u{2500}
    let model_name = op
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("op_{i}_model"));
    let units = op.scale.map(ModelUnits::Custom);
    let model = LoadedModel::from_file(
        rs_cam_core::ids::ModelId::placeholder(),
        &model_name,
        &op.input,
        None,
        units,
        job_dir,
    )
    .map_err(|e| anyhow::anyhow!("loading input '{}': {e}", op.input.display()))?;
    let _ = crate::command::apply_command(
        &mut session,
        Command::AddModel(AddModelArgs {
            model: Box::new(model),
        }),
    )?;
    // The add door gave the model its id; the toolpath binds that id.
    let model_id = session
        .models()
        .last()
        .map(|m| m.id)
        .context("add_model added no model")?;

    // S11: the stock thickness of a drawing, the patch MCP merges. On a
    // stock with no thickness it keeps the top and the auto fit.
    if let Some(z) = op.stock_z {
        let mut stock = session.stock_config().clone();
        let _ = rs_cam_core::compute::stock_config::StockConfigPatch {
            z: Some(z),
            ..Default::default()
        }
        .apply_to(&mut stock);
        let _ = crate::command::apply_command(
            &mut session,
            Command::SetStockConfig(SetStockConfigArgs {
                stock: Box::new(stock),
            }),
        )?;
    }

    // \u{2500}\u{2500} Stock: a set top only; the session sizes the rest \u{2500}\u{2500}
    if let Some(top) = op.stock_top_z {
        // F118: the same patch the MCP `set_stock_config` tool merges.
        let mut stock = session.stock_config().clone();
        let _ = rs_cam_core::compute::stock_config::StockConfigPatch {
            z: Some((top - stock.origin_z).max(0.0)),
            auto_from_model: Some(false),
            ..Default::default()
        }
        .apply_to(&mut stock);
        let _ = crate::command::apply_command(
            &mut session,
            Command::SetStockConfig(SetStockConfigArgs {
                stock: Box::new(stock),
            }),
        )?;
    }

    // \u{2500}\u{2500} Tools \u{2500}\u{2500}
    let tool_idx = crate::command::apply_command(
        &mut session,
        Command::AddTool(AddToolArgs {
            tool: Box::new(tool_config_from_def(tool_def, &op.tool)?),
        }),
    )?
    .created
    .context("add_tool reports no new tool index")?;

    let debug_options = ToolpathDebugOptions {
        enabled: debug_trace,
    };

    let tool_id = session
        .tools()
        .get(tool_idx)
        .context("tool index out of bounds after add_tool")?
        .id;
    // S11 parity: THE draft door that the GUI and MCP share
    // (`ProjectSession::draft_toolpath`): the stock defaults, Suggest and
    // `DressupConfig::for_op`. A key the job leaves unset keeps the draft
    // value; a value that no rule gives stays unset, and the op refuses as
    // it does on the GUI and MCP.
    // The job names its tool and its model, so both are exact.
    let draft = session
        .draft_toolpath(
            rs_cam_core::session::DraftRequest::new(
                op_type,
                0,
                format!("op_{}_{}", i, op_type.kind_str()),
            )
            .tool(tool_id)
            .model(model_id),
        )
        .map_err(|e| anyhow::anyhow!("adding toolpath: {e}"))?;
    if let Some(refusal) = &draft.feeds_refusal {
        warn!(
            op = i,
            op_type = op_type.kind_str(),
            %refusal,
            "no feeds data for the tool: set feed_rate, plunge_rate and spindle_rpm in the job"
        );
    }
    let batch = batch_selection(op_type, draft.config.operation.selection());
    let config = ToolpathConfig {
        coolant: op.coolant,
        debug_options,
        ..draft.config
    };
    let tp_index = crate::command::apply_command(
        &mut session,
        Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(config),
        }),
    )
    .map_err(|e| anyhow::anyhow!("adding toolpath: {e}"))?
    .created
    .context("add_toolpath reports no new toolpath index")?;

    // \u{2500}\u{2500} Parameters and dressups: the core setters, in schema order \u{2500}\u{2500}
    // C2: the schema order, not the file order. A dressup value field
    // refuses while its dressup is off, and `FIELD_DEFS` names each
    // enable key before its value fields.
    for def in op_type.registry_entry().params.defs {
        let Some(value) = op.params.get(def.name) else {
            continue;
        };
        let command = Command::SetToolpathParam(SetToolpathParamArgs {
            index: tp_index,
            param: def.name.to_owned(),
            value: serde_json::to_value(value)?,
        });
        let _ = crate::command::apply_command(&mut session, command)
            .map_err(|e| anyhow::anyhow!("param '{}': {e}", def.name))?;
    }
    for def in DressupConfig::FIELD_DEFS {
        let Some(value) = op.dressups.get(def.name) else {
            continue;
        };
        let command = Command::SetDressupField(SetDressupFieldArgs {
            index: tp_index,
            key: def.name.to_owned(),
            value: serde_json::to_value(value)?,
        });
        let _ = crate::command::apply_command(&mut session, command)
            .map_err(|e| anyhow::anyhow!("dressup '{}': {e}", def.name))?;
    }
    if let Some(boundary) = &op.boundary {
        let command = Command::SetBoundaryConfig(rs_cam_core::session::SetBoundaryConfigArgs {
            index: tp_index,
            boundary: boundary.clone(),
        });
        let _ = crate::command::apply_command(&mut session, command)
            .map_err(|e| anyhow::anyhow!("boundary: {e}"))?;
    }
    if let Some(selection) = op.selection.as_ref().or(batch.as_ref()) {
        let command = Command::EditSelection(EditSelectionArgs {
            index: tp_index,
            target: rs_cam_core::session::SelectionTarget::Op,
            edit: rs_cam_core::selection::SelectionEdit::Replace(Box::new(selection.clone())),
        });
        let _ = crate::command::apply_command(&mut session, command)
            .map_err(|e| anyhow::anyhow!("selection: {e}"))?;
    }
    if let Some(selection) = &op.boundary_selection {
        let command = Command::EditSelection(EditSelectionArgs {
            index: tp_index,
            target: rs_cam_core::session::SelectionTarget::Boundary,
            edit: rs_cam_core::selection::SelectionEdit::Replace(Box::new(selection.clone())),
        });
        let _ = crate::command::apply_command(&mut session, command)
            .map_err(|e| anyhow::anyhow!("boundary_selection: {e}"))?;
    }
    if let Some(floor) = op.floor.clone() {
        let command = Command::SetToolpathFloor(SetToolpathFloorArgs {
            index: tp_index,
            floor,
        });
        let _ = crate::command::apply_command(&mut session, command)
            .map_err(|e| anyhow::anyhow!("floor: {e}"))?;
    }

    let mut post = session.post_config().clone();
    post.format = job.job.post;
    post.safe_z = op.safe_z.unwrap_or(job.job.safe_z);
    post.spindle_speed = job.job.spindle_speed;
    let _ = crate::command::apply_command(
        &mut session,
        Command::SetPostConfig(SetPostConfigArgs {
            post: Box::new(post),
        }),
    )?;

    Ok((session, tp_index))
}

fn execute_op_via_session(
    job: &JobFile,
    job_dir: &Path,
    i: usize,
    op: &OperationDef,
    tool_def: &ToolDef,
    debug_trace: bool,
) -> Result<SessionOpOutput> {
    let op_type = op.operation_type()?;
    let (mut session, tp_index) = job_op_session(job, job_dir, i, op, tool_def, debug_trace)?;
    // \u{2500}\u{2500} Generate \u{2500}\u{2500}
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(tp_index, &cancel)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let result = session
        .result_at(tp_index)
        .context("generation produced no result")?;
    let toolpath = result.toolpath().clone();

    let trace = if debug_trace {
        let tp_name = format!("op_{}_{}", i, op_type.kind_str());
        let op_label = format!("Op {} \u{2014} {}", i, op.op_type);
        // The one size convention (`Ø` diameter, `R` radius) shared by
        // every surface; see `ToolConfig::size_label`.
        let tool_summary = tool_config_from_def(tool_def, &op.tool)?.summary();
        let operation_json = session
            .toolpath_config_at(tp_index)
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
            // The job runner numbers its operations: the index is the id.
            rs_cam_core::ToolpathId::from_wire(i),
            &tp_name,
            &op_label,
            &tool_summary,
            request_snapshot,
            result.debug_trace.clone(),
            result.semantic_trace.as_deref().cloned(),
        ))
    } else {
        None
    };

    let config = session.toolpath_config_at(tp_index);
    let drill = config.and_then(rs_cam_core::gcode::phase_drill_for);
    let spindle_rpm = config.and_then(|tc| tc.operation.spindle_rpm());
    Ok(SessionOpOutput {
        toolpath,
        trace,
        drill,
        spindle_rpm,
    })
}

/// Best-effort typed JSON from a CLI parameter string: integer, then float,
/// then bool, else string.
///
/// **One coercer for the CLI.** `run --set` applies its answer through
/// `Command::SetToolpathParam`, whose own coercion layer (E.6.a) handles the
/// rest — numeric strings, 0/1 bools and enum tokens — and whose 0/1 → bool
/// step reads `Number::as_i64`, so the integer-first order is load-bearing
/// there. `sweep` records its answer in the report's `SweepVariant.value`.
/// The sweep held its own copy with the float test FIRST until 2026-09-17,
/// so the same `3` on the command line was `3` in one artifact and `3.0` in
/// the other, and a reader comparing them saw two types for one value.
pub(crate) fn param_value_from_str(s: &str) -> serde_json::Value {
    // A list value (`name=[10]`, `name=[]`) is a JSON array, as MCP
    // `set_toolpath_param` unwraps a stringified container. Text in
    // brackets that is not JSON stays a string, so the refusal quotes it.
    if s.trim_start().starts_with('[')
        && let Ok(list @ serde_json::Value::Array(_)) = serde_json::from_str(s.trim())
    {
        return list;
    }
    if let Ok(i) = s.parse::<i64>() {
        return serde_json::Value::Number(i.into());
    }
    if let Ok(f) = s.parse::<f64>()
        && let Some(n) = serde_json::Number::from_f64(f)
    {
        return serde_json::Value::Number(n);
    }
    match s {
        "true" => serde_json::Value::Bool(true),
        "false" => serde_json::Value::Bool(false),
        _ => serde_json::Value::String(s.to_owned()),
    }
}

/// The selection that a batch (a job op with no `selection` table, or
/// `rs_cam run`) writes on a new op. W3h: the draft door gives an op that
/// cuts model geometry an empty selection, and the GUI operator picks. A
/// batch has no picker, so it writes the explicit rule "every item of the
/// toolpath model that the op can cut" (`Selection::drawing`). `None` when
/// the drafted op holds no selection, holds one already (Face: the stock
/// outline), or is a Pin Drill (its holes are the stock pins, decision D6).
pub(crate) fn batch_selection(
    op_type: rs_cam_core::compute::catalog::OperationType,
    drafted: Option<&rs_cam_core::selection::Selection>,
) -> Option<rs_cam_core::selection::Selection> {
    let empty = drafted.is_some_and(rs_cam_core::selection::Selection::is_empty);
    (empty && op_type != rs_cam_core::compute::catalog::OperationType::AlignmentPinDrill)
        .then(rs_cam_core::selection::Selection::drawing)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    /// S11 parity (operator ruling 2026-10-23): a job op is THE core draft
    /// (`ProjectSession::draft_toolpath`, the GUI and MCP door), so a key
    /// the job leaves unset gives the unset state of the GUI and MCP. The
    /// job states the stock thickness (`stock_z`) of the drawing and no
    /// floor: the op equals the draft (params with nulls, dressups,
    /// boundary), its floor is not set, and generation refuses with the
    /// floor text. A job with no `stock_z` refuses with the stock text. The
    /// GUI twin is `rs_cam_viz/tests/one_draft_for_every_add_door_s11.rs`.
    ///
    /// Red proof (run by hand, 2026-10-23): the job op back on
    /// `DressupConfig::default()` goes red.
    /// R-A parity: a job Profile on a STEP model takes the part outline
    /// only, side Outside, to the model bottom, with no tabs, as the GUI
    /// and MCP add doors do (`the_default_mesh_profile_is_one_config_ra`).
    #[test]
    #[expect(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic,
        reason = "test code: a panic is how a test reports a failure"
    )]
    fn a_job_profile_on_a_step_model_cuts_the_outline_ra() {
        let step = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rs_cam_core/tests/fixtures/step/stepped_block.step")
            .canonicalize()
            .expect("the fixture exists");
        let dir = std::env::temp_dir().join(format!("rs_cam_ra_job_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let job_path = dir.join("job.toml");
        std::fs::write(
            &job_path,
            format!(
                "[job]\noutput = \"out.nc\"\n\n[tools.flat_6mm]\ntype = \"end_mill\"\n\
                 diameter = 6.0\n\n[[operation]]\ntype = \"profile\"\ninput = \"{}\"\n\
                 tool = \"flat_6mm\"\n",
                step.display()
            ),
        )
        .expect("write the job");
        let job = parse_job_file(&job_path).expect("the job parses");
        let op = &job.operation[0];
        let tool_def = &job.tools[&op.tool];
        let (session, index) =
            job_op_session(&job, &dir, 0, op, tool_def, false).expect("the job session");
        let _ = std::fs::remove_dir_all(&dir);
        let tc = &session.toolpath_configs()[index];
        let rs_cam_core::compute::catalog::OperationConfig::Profile(cfg) = &tc.operation else {
            panic!("a Profile");
        };
        assert_eq!(
            cfg.floor,
            rs_cam_core::compute::floor::Floor::ModelBottom { depth: 0.0 }
        );
        assert_eq!(cfg.side, rs_cam_core::ops::profile::ProfileSide::Outside);
        assert_eq!((cfg.tab_count, cfg.tab_points.len()), (0, 0));
        let report = session
            .selection_status(tc, rs_cam_core::session::SelectionTarget::Op)
            .expect("the status");
        assert_eq!(report.rows.len(), 1, "the outline only");
        assert!(report.rows[0].outline);
    }

    /// F7: the job key `boundary_selection` writes the boundary selection
    /// through `EditSelection { target: Boundary }`, after the job key
    /// `boundary`. The stored selection survives a save and a load, and
    /// `selection_status(Boundary)` equals the core door that MCP
    /// `edit_toolpath_selection target=boundary` drives (the same
    /// `Command::EditSelection` on the job session with no
    /// `boundary_selection` key).
    #[test]
    #[expect(
        clippy::expect_used,
        clippy::indexing_slicing,
        reason = "test code: a panic is how a test reports a failure"
    )]
    fn a_job_boundary_selection_replays_one_selection_f7() {
        use rs_cam_core::selection::{Filter, Rule, Selection, SelectionEdit, SourceRef};
        use rs_cam_core::session::{Command, EditSelectionArgs, ProjectSession, SelectionTarget};
        let drawing = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf")
            .canonicalize()
            .expect("the fixture exists");
        let dir = std::env::temp_dir().join(format!("rs_cam_f7_job_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let boundary = "[operation.boundary]\nenabled = true\ncontainment = \"center\"\n\
                        offset = 0.0\nsource = { selection = { selection = {} } }\n";
        let rule = "[[operation.boundary_selection.rules]]\nsource = \"toolpath_model\"\n\
                    filter = { layers = [\"CUT\"] }\n";
        let session_of = |tail: &str| {
            let job_path = dir.join("job.toml");
            std::fs::write(
                &job_path,
                format!(
                    "[job]\noutput = \"out.nc\"\n\n[tools.flat_3mm]\ntype = \"end_mill\"\n\
                     diameter = 3.0\n\n[[operation]]\ntype = \"pocket\"\ninput = \"{}\"\n\
                     tool = \"flat_3mm\"\nstock_z = 12.0\n{tail}",
                    drawing.display()
                ),
            )
            .expect("write the job");
            let job = parse_job_file(&job_path).expect("the job parses");
            let op = &job.operation[0];
            let tool_def = &job.tools[&op.tool];
            job_op_session(&job, &dir, 0, op, tool_def, false)
        };
        let want = Selection::from_rules(vec![Rule {
            source: SourceRef::ToolpathModel,
            filter: Filter {
                layers: vec!["CUT".to_owned()],
                ..Filter::default()
            },
        }]);

        // The job: boundary + boundary_selection.
        let (job_session, index) =
            session_of(&format!("{boundary}{rule}")).expect("the job session");
        let tc = &job_session.toolpath_configs()[index];
        assert_eq!(
            tc.selection_of(SelectionTarget::Boundary).expect("a slot"),
            Some(&want),
            "the job wrote its boundary selection"
        );

        // Save and load: the stored boundary selection is the job's.
        let path = dir.join("f7.toml");
        job_session.save(&path).expect("save");
        let loaded = ProjectSession::load(&path).expect("load");
        let loaded_tc = &loaded.toolpath_configs()[index];
        assert_eq!(
            loaded_tc
                .selection_of(SelectionTarget::Boundary)
                .expect("a slot"),
            Some(&want),
            "the boundary selection survives a save and a load"
        );

        // The core door (MCP `edit_toolpath_selection target=boundary`).
        let (mut twin, twin_index) = session_of(boundary).expect("the twin session");
        let _ = twin
            .apply(Command::EditSelection(EditSelectionArgs {
                index: twin_index,
                target: SelectionTarget::Boundary,
                edit: SelectionEdit::Replace(Box::new(want.clone())),
            }))
            .expect("the boundary is a selection");
        let twin_tc = twin.toolpath_configs()[twin_index].clone();
        let job_report = job_session
            .selection_status(tc, SelectionTarget::Boundary)
            .expect("the job status");
        let twin_report = twin
            .selection_status(&twin_tc, SelectionTarget::Boundary)
            .expect("the twin status");
        assert_eq!(*job_report, *twin_report, "one selection_status, two doors");
        assert!(
            !job_report.selected.is_empty(),
            "the rule selects the CUT square"
        );

        // A boundary that is not a selection refuses, with the core text.
        let refused = session_of(rule)
            .err()
            .expect("boundary_selection on a stock boundary refuses");
        let (mut plain, plain_index) = session_of("").expect("the plain session");
        let core = plain
            .apply(Command::EditSelection(EditSelectionArgs {
                index: plain_index,
                target: SelectionTarget::Boundary,
                edit: SelectionEdit::Replace(Box::new(want)),
            }))
            .expect_err("the core door refuses too");
        assert_eq!(
            refused.to_string(),
            format!("boundary_selection: {core}"),
            "the job gives the core (MCP) text"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[expect(
        clippy::expect_used,
        clippy::indexing_slicing,
        reason = "test code: a panic is how a test reports a failure"
    )]
    fn the_job_drafts_through_the_core_door_s11() {
        let drawing = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf")
            .canonicalize()
            .expect("the fixture exists");
        let dir = std::env::temp_dir().join(format!("rs_cam_s11_job_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let session_of = |stock_line: &str| {
            let job_path = dir.join("job.toml");
            std::fs::write(
                &job_path,
                format!(
                    "[job]\noutput = \"out.nc\"\n\n[tools.flat_3mm]\ntype = \"end_mill\"\n\
                     diameter = 3.0\n\n[[operation]]\ntype = \"pocket\"\ninput = \"{}\"\n\
                     tool = \"flat_3mm\"\n{stock_line}\n",
                    drawing.display()
                ),
            )
            .expect("write the job");
            let job = parse_job_file(&job_path).expect("the job parses");
            let op = &job.operation[0];
            let tool_def = &job.tools[&op.tool];
            job_op_session(&job, &dir, 0, op, tool_def, false).expect("the job session")
        };

        let (mut session, index) = session_of("stock_z = 12.0");
        let job_tc = session.toolpath_configs()[index].clone();
        let draft = session
            .draft_toolpath(
                rs_cam_core::session::DraftRequest::new(
                    OperationType::Pocket,
                    0,
                    job_tc.name.clone(),
                )
                .tool(job_tc.tool_id.expect("the job op has its tool"))
                .model(job_tc.model_id),
            )
            .expect("the draft");
        // W3h: the draft starts with nothing selected; a job with no
        // selection table writes the batch rule. The rest is the draft.
        let without_selection = |mut v: serde_json::Value| {
            if let Some(map) = v.as_object_mut() {
                let _ = map.remove("selection");
            }
            v
        };
        assert_eq!(
            without_selection(job_tc.operation.params_value_including_nulls()),
            without_selection(draft.config.operation.params_value_including_nulls()),
            "the job op is the core draft"
        );
        assert!(
            draft
                .config
                .operation
                .selection()
                .is_some_and(rs_cam_core::selection::Selection::is_empty),
            "the draft starts with nothing selected"
        );
        assert!(
            job_tc
                .operation
                .selection()
                .is_some_and(rs_cam_core::selection::Selection::is_drawing),
            "the job writes the batch rule"
        );
        assert_eq!(
            serde_json::to_value(&job_tc.dressups).expect("json"),
            serde_json::to_value(&draft.config.dressups).expect("json"),
            "the job takes `DressupConfig::for_op`, as the GUI and MCP do"
        );
        assert_eq!(job_tc.boundary, draft.config.boundary);
        assert_eq!(job_tc.operation.unset_fields(), vec!["floor"]);
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let text = session
            .generate_toolpath(index, &cancel)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(
            text.contains(rs_cam_core::compute::floor::FLOOR_UNSET_TEXT),
            "the job refuses as the GUI does: {text}"
        );

        let (mut session, index) = session_of("");
        let text = session
            .generate_toolpath(index, &cancel)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(
            text.contains(rs_cam_core::compute::stock_config::STOCK_THICKNESS_NOT_SET_TEXT),
            "a drawing job with no stock_z refuses: {text}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// "Cut past open sides" (operator ruling 2026-10-23): the job key is
    /// the schema name, as MCP `set_toolpath_param` takes it, and the core
    /// setter writes the field. A job with no key keeps the setting off.
    #[test]
    #[expect(
        clippy::expect_used,
        clippy::indexing_slicing,
        reason = "test code: a panic is how a test reports a failure"
    )]
    fn the_job_key_sets_cut_past_open_sides_opensides() {
        let drawing = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf")
            .canonicalize()
            .expect("the fixture exists");
        let dir = std::env::temp_dir().join(format!("rs_cam_opensides_job_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let flag_of = |line: &str| {
            let job_path = dir.join("job.toml");
            std::fs::write(
                &job_path,
                format!(
                    "[job]\noutput = \"out.nc\"\n\n[tools.flat_3mm]\ntype = \"end_mill\"\n\
                     diameter = 3.0\n\n[[operation]]\ntype = \"pocket\"\ninput = \"{}\"\n\
                     tool = \"flat_3mm\"\nstock_z = 12.0\n{line}\n",
                    drawing.display()
                ),
            )
            .expect("write the job");
            let job = parse_job_file(&job_path).expect("the job parses");
            let op = &job.operation[0];
            let tool_def = &job.tools[&op.tool];
            let (session, index) =
                job_op_session(&job, &dir, 0, op, tool_def, false).expect("the job session");
            session.toolpath_configs()[index]
                .operation
                .cuts_past_open_sides()
        };
        assert!(flag_of("cut_past_open_sides = true"));
        assert!(!flag_of(""));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The cutter of a job tool, through the one core builder.
    fn cutter_of(def: &ToolDef) -> Result<rs_cam_core::tool::ToolDefinition> {
        phase_cutter(&tool_config_from_def(def, "t")?)
    }

    fn end_mill_def() -> ToolDef {
        ToolDef {
            tool_type: ToolType::EndMill,
            number: None,
            diameter: 6.0,
            flute_count: ToolFact::Unknown,
            corner_radius: None,
            included_angle: None,
            taper_angle: ToolFact::Unknown,
            shaft_diameter: None,
            shank_diameter: ToolFact::Unknown,
            stickout: ToolFact::Unknown,
            centre_cutting: ToolFact::Unknown,
        }
    }

    /// F116 step 1: the job's cutter comes from core `build_cutter`, so a
    /// tool that `ToolConfig::validate` refuses is refused here too. The
    /// deleted CLI `build_tool` built a cutter with zero flutes and a
    /// negative stickout.
    #[test]
    fn f116_the_job_cutter_refuses_an_out_of_domain_tool() {
        let zero_flutes = ToolDef {
            flute_count: ToolFact::Stated(0),
            ..end_mill_def()
        };
        let err = cutter_of(&zero_flutes).err().map(|e| format!("{e:#}"));
        assert!(
            err.as_deref().is_some_and(|e| e.contains("flute_count")),
            "{err:?}"
        );
        let negative_stickout = ToolDef {
            stickout: ToolFact::Stated(-1.0),
            ..end_mill_def()
        };
        assert!(cutter_of(&negative_stickout).is_err());

        // The job cutter is the session tool: the same stickout and flutes.
        let tool = tool_config_from_def(&end_mill_def(), "t").unwrap();
        let cutter = cutter_of(&end_mill_def()).unwrap();
        assert_eq!(
            cutter.stickout.to_bits(),
            tool.stickout_planning_mm().to_bits()
        );
        assert_eq!(cutter.flute_count, tool.flute_count_planning());
    }

    #[test]
    fn non_centre_cutting_job_geometry_reaches_tool_config() {
        let def = ToolDef {
            centre_cutting: ToolFact::Stated(rs_cam_core::compute::tool_config::CentreCutting::No(
                rs_cam_core::compute::NonCentreCuttingGeometry {
                    centre_core_diameter_mm: 3.0,
                    blade_protrusion_mm: 1.0,
                },
            )),
            ..end_mill_def()
        };
        assert_eq!(
            tool_config_from_def(&def, "t")
                .unwrap()
                .centre_cutting_fact(),
            &def.centre_cutting
        );
    }

    /// A job tool must state the geometry that defines its type.
    #[test]
    fn a_job_bull_nose_without_a_corner_radius_is_refused() {
        let def = ToolDef {
            tool_type: ToolType::BullNose,
            ..end_mill_def()
        };
        let err = tool_config_from_def(&def, "t").err().map(|e| e.to_string());
        assert!(
            err.as_deref().is_some_and(|e| e.contains("corner_radius")),
            "{err:?}"
        );
    }

    /// F028: the `[job] post` token goes through `PostFormat::from_token`.
    /// An unknown token refuses the job file, and so does a retired alias (W4).
    /// Before the fix the field was a `String`, and `execute_job` (the door
    /// `sweep` calls) turned an unknown token into GRBL.
    #[test]
    fn f028_the_job_file_refuses_an_unknown_post_token() {
        let job = |post: Option<&str>| {
            let post = post
                .map(|p| format!("post = \"{p}\"\n"))
                .unwrap_or_default();
            toml::from_str::<JobFile>(&format!("[job]\noutput = \"a.nc\"\n{post}"))
        };
        let err = job(Some("grbll")).err().map(|e| e.to_string());
        assert!(
            err.as_deref().is_some_and(|e| e.contains("grbll")),
            "{err:?}"
        );
        assert_eq!(job(Some("GRBLHAL")).unwrap().job.post, PostFormat::GrblHal);
        assert!(job(Some("grbl_hal")).is_err(), "the retired alias refuses");
        assert_eq!(job(None).unwrap().job.post, PostFormat::Grbl);
        // The sweep writes the job back and reads it again.
        let round = toml::to_string(&job(Some("linuxcnc")).unwrap()).unwrap();
        assert_eq!(
            toml::from_str::<JobFile>(&round).unwrap().job.post,
            PostFormat::LinuxCnc
        );
    }

    #[test]
    fn test_tool_radius_uses_cutter_radius() {
        // Verify that the job cutter's radius() matches what the job
        // executor would use (cutter.radius(), not diameter/2.0 from the
        // TOML definition, which could differ for composite tools).
        let flat_def = ToolDef {
            tool_type: ToolType::EndMill,
            number: None,
            diameter: 6.0,
            flute_count: ToolFact::Unknown,
            corner_radius: None,
            included_angle: None,
            taper_angle: ToolFact::Unknown,
            shaft_diameter: None,
            shank_diameter: ToolFact::Unknown,
            stickout: ToolFact::Unknown,
            centre_cutting: ToolFact::Unknown,
        };
        let cutter = cutter_of(&flat_def).unwrap();
        assert!((cutter.radius() - 3.0).abs() < 1e-10);

        let ball_def = ToolDef {
            tool_type: ToolType::BallNose,
            number: None,
            diameter: 10.0,
            flute_count: ToolFact::Unknown,
            corner_radius: None,
            included_angle: None,
            taper_angle: ToolFact::Unknown,
            shaft_diameter: None,
            shank_diameter: ToolFact::Unknown,
            stickout: ToolFact::Unknown,
            centre_cutting: ToolFact::Unknown,
        };
        let ball_cutter = cutter_of(&ball_def).unwrap();
        assert!((ball_cutter.radius() - 5.0).abs() < 1e-10);

        // For tapered ball, diameter() returns shaft_diameter, so radius()
        // returns shaft_diameter / 2.0, which is correct for the effective
        // cutting envelope.
        let tapered_def = ToolDef {
            tool_type: ToolType::TaperedBallNose,
            number: None,
            diameter: 6.0,
            flute_count: ToolFact::Unknown,
            corner_radius: None,
            included_angle: None,
            taper_angle: ToolFact::Stated(10.0),
            shaft_diameter: Some(12.0),
            shank_diameter: ToolFact::Unknown,
            stickout: ToolFact::Unknown,
            centre_cutting: ToolFact::Unknown,
        };
        let tapered_cutter = cutter_of(&tapered_def).unwrap();
        // radius() should return shaft_diameter / 2.0 = 6.0
        assert!((tapered_cutter.radius() - 6.0).abs() < 1e-10);
    }

    /// CLI-08 sentry: one tool-type vocabulary on every surface.
    ///
    /// The job file used to carry its own five tokens (`flat`, `ball`,
    /// `bullnose`, `vbit`, `tapered_ball`) behind a `CliToolType` enum.
    /// `run --tool` and MCP `add_tool` both read
    /// [`ToolType::parse_lenient`], so a job TOML and an MCP call for the
    /// same cutter had to spell it differently. This test walks
    /// [`ToolType::ALL`] and asserts the ONE canonical token reaches all
    /// three surfaces: the job parser takes it, and `parse_lenient` — the
    /// parser `run` and `rs_cam_mcp::server::parse_tool_type` call — reads
    /// it back as the same variant.
    #[test]
    fn every_tool_type_round_trips_through_the_job_file_on_one_token() {
        for &expected in ToolType::ALL {
            let token = expected.serde_token();
            let source = format!("[tools.t]\ntype = \"{token}\"\ndiameter = 6.0\n");
            #[derive(Deserialize)]
            struct ToolsOnly {
                tools: HashMap<String, ToolDef>,
            }
            let parsed: ToolsOnly = toml::from_str(&source)
                .unwrap_or_else(|e| panic!("job file rejected the canonical token {token}: {e}"));
            let def = parsed.tools.get("t").unwrap();
            assert_eq!(def.tool_type, expected, "job TOML token {token}");

            // The `run` / MCP door reads the same token as the same type.
            assert_eq!(
                ToolType::parse_lenient(token),
                Some(expected),
                "parse_lenient token {token}"
            );

            // And the job file emits the token it accepts.
            let emitted = toml::to_string(&parsed.tools).unwrap();
            assert!(
                emitted.contains(&format!("type = \"{token}\"")),
                "job TOML emitted {emitted:?}, expected the canonical token {token}"
            );
        }
    }

    /// Parse one `[[operation]]` table and run the C2 check.
    fn checked(source: &str) -> Result<OperationType> {
        let op: OperationDef = toml::from_str(source).unwrap();
        op.operation_type()
    }

    /// C2: every op in the registry is a job op. The `type` token is the
    /// registry token, so no op needs CLI code.
    #[test]
    fn every_registry_op_is_a_job_op() {
        for &op in OperationType::ALL {
            let source = format!(
                "type = \"{}\"\ninput = \"m.stl\"\ntool = \"a\"\n",
                op.kind_str()
            );
            assert_eq!(checked(&source).unwrap(), op);
        }
        let err = checked("type = \"drop-cutter\"\ninput = \"m.stl\"\ntool = \"a\"\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("drop-cutter"), "{err}");
    }

    /// C2: a key that the op's schema does not publish refuses, and the
    /// refusal names EVERY such key. The pre-C2 job spellings are in this
    /// set: `order_by`, `strategy`, `tabs`, `entry_3d`, `stock_to_leave` on
    /// 3D Rough, the deleted `max_stay_down_dist` and `coarse_steps`.
    #[test]
    fn a_key_the_schema_does_not_publish_refuses() {
        let err = checked(
            "type = \"adaptive3d\"\ninput = \"m.stl\"\ntool = \"a\"\n\
             order_by = \"global\"\nstrategy = \"contour\"\nentry_3d = \"helix\"\n\
             stock_to_leave = 0.5\nmax_stay_down_dist = 30.0\ncoarse_steps = [10.0]\n",
        )
        .unwrap_err()
        .to_string();
        for key in [
            "order_by",
            "strategy",
            "entry_3d",
            "stock_to_leave",
            "max_stay_down_dist",
            "coarse_steps",
        ] {
            assert!(err.contains(key), "the refusal must name {key}: {err}");
        }
        assert!(
            err.contains("region_ordering"),
            "the refusal lists the schema: {err}"
        );
        let err = checked("type = \"profile\"\ninput = \"m.dxf\"\ntool = \"a\"\ntabs = 4\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("tabs") && err.contains("tab_count"), "{err}");
    }

    /// C2: the schema names pass, and a dressup field rides its own table.
    #[test]
    fn schema_names_and_dressup_fields_pass() {
        assert_eq!(
            checked(
                "type = \"adaptive3d\"\ninput = \"m.stl\"\ntool = \"a\"\n\
                 region_ordering = \"global\"\nclearing_strategy = \"contour_parallel\"\n\
                 entry_style = \"helix\"\nstock_to_leave_axial = 0.5\n\
                 max_stay_down_distance_mm = 18.0\n",
            )
            .unwrap(),
            OperationType::Adaptive3d
        );
        assert_eq!(
            checked(
                "type = \"pocket\"\ninput = \"m.svg\"\ntool = \"a\"\n\
                 [dressups]\nentry_style = \"helix\"\ndogbone = true\n",
            )
            .unwrap(),
            OperationType::Pocket
        );
        let err = checked(
            "type = \"pocket\"\ninput = \"m.svg\"\ntool = \"a\"\n\
             [dressups]\nentry = \"ramp\"\n",
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("entry") && err.contains("entry_style"),
            "{err}"
        );
    }

    /// C2: a job key never shadows a parameter. If an op published a
    /// parameter with a job-key name, the job file could not set it.
    #[test]
    fn no_job_key_is_a_parameter_name() {
        for &op in OperationType::ALL {
            for def in op.registry_entry().params.defs {
                assert!(
                    !OperationDef::JOB_KEYS.contains(&def.name),
                    "{}: the parameter {} is also a job key",
                    op.kind_str(),
                    def.name
                );
            }
        }
    }

    /// C2: the job files in the repo parse and pass the check, and the
    /// demo job runs. `fixtures/debug_adaptive/job.toml` reads a model
    /// outside the repo, so this test only parses it.
    #[test]
    fn the_job_files_in_the_repo_parse_and_the_demo_runs() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let _ = parse_job_file(&root.join("debug_adaptive/job.toml")).unwrap();
        let demo = parse_job_file(&root.join("demo_job.toml")).unwrap();
        let result = execute_job(&demo, &root, false).unwrap();
        assert_eq!(result.phases.len(), demo.operation.len());
        assert!(result.phases.iter().all(|p| !p.toolpath.moves.is_empty()));
    }

    /// GA2: the job door takes the project loader's rule for a retired
    /// parameter (the one `RETIRED_VALUES` table). An old 3D Rough with
    /// `z_blend = false` passes the key check and the key is dropped;
    /// `z_blend = true` refuses with the loader's fix text. An unknown key
    /// still refuses.
    #[test]
    fn a_job_takes_the_project_rule_for_z_blend_ga2() {
        let op = |extra: &str| -> OperationDef {
            let source = format!(
                "[job]\noutput = \"o.nc\"\n[tools.t]\ntype = \"end_mill\"\ndiameter = 6.0\n\
                 [[operation]]\ntype = \"adaptive3d\"\ninput = \"m.stl\"\ntool = \"t\"\n{extra}"
            );
            let mut job: JobFile = toml::from_str(&source).unwrap();
            job.operation.remove(0)
        };
        assert_eq!(
            op("z_blend = false\n").operation_type().unwrap(),
            OperationType::Adaptive3d,
            "z_blend = false is dropped, as the project loader drops it"
        );
        let refusal = op("z_blend = true\n")
            .operation_type()
            .unwrap_err()
            .to_string();
        assert!(
            refusal.contains("z_blend = true is retired")
                && refusal.contains("Depth/Pass")
                && refusal.contains("Delete the key"),
            "{refusal}"
        );
        let unknown = op("no_such_key = 1\n")
            .operation_type()
            .unwrap_err()
            .to_string();
        assert!(
            unknown.contains("has no parameter no_such_key"),
            "{unknown}"
        );
    }

    /// C2: a parameter and a dressup field of the job file reach the
    /// generated toolpath through the core setters.
    #[test]
    fn job_parameters_and_dressups_reach_the_toolpath() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let run = |extra: &str| {
            let source = format!(
                "[job]\noutput = \"o.nc\"\n[tools.t]\ntype = \"end_mill\"\ndiameter = 6.0\n\
                 [[operation]]\ntype = \"pocket\"\ninput = \"demo_pocket.svg\"\ntool = \"t\"\n\
                 stock_z = 25.0\nfloor = {{ from = \"top\", depth = 3.0 }}\n{extra}"
            );
            let job: JobFile = toml::from_str(&source).unwrap();
            let _ = job.operation.first().unwrap().operation_type().unwrap();
            execute_job(&job, &root, false).unwrap().combined
        };
        let plain = run("");
        assert!(!plain.moves.is_empty());
        let deeper = run("depth_per_pass = 0.5\n");
        assert!(
            deeper.moves.len() > plain.moves.len(),
            "depth_per_pass did not reach the op"
        );
        let helix = run("[operation.dressups]\nentry_style = \"helix\"\n");
        assert_ne!(
            helix.moves.len(),
            plain.moves.len(),
            "the helix entry did not reach the op"
        );
    }

    /// `--set name=[10]` and `=[]` reach the core as JSON arrays. They
    /// were strings, and the core refused them ("invalid type: string
    /// \"[]\", expected a sequence").
    #[test]
    fn a_set_value_in_brackets_is_a_list() {
        assert_eq!(param_value_from_str("[]"), serde_json::json!([]));
        assert_eq!(param_value_from_str("[10]"), serde_json::json!([10]));
        assert_eq!(
            param_value_from_str("[10.0, 5]"),
            serde_json::json!([10.0, 5])
        );
        // Not JSON: it stays the text the caller sent.
        assert_eq!(param_value_from_str("[10"), serde_json::json!("[10"));
        // Scalars are unchanged.
        assert_eq!(param_value_from_str("3"), serde_json::json!(3));
        assert_eq!(
            param_value_from_str("by_area"),
            serde_json::json!("by_area")
        );
    }

    /// CLI-08 teeth: the five job-only spellings are gone, not aliased.
    ///
    /// Operator ruling 2026-09-16 — no legacy support. A job file on the
    /// old vocabulary must hear a refusal, not silently get an end mill.
    #[test]
    fn the_job_file_refuses_the_deleted_cli_tool_tokens() {
        for token in ["flat", "ball", "bullnose", "vbit", "tapered_ball"] {
            let source = format!("[tools.t]\ntype = \"{token}\"\ndiameter = 6.0\n");
            #[derive(Deserialize)]
            struct ToolsOnly {
                #[expect(
                    dead_code,
                    reason = "the test only checks that the parse fails; it never reads the field"
                )]
                tools: HashMap<String, ToolDef>,
            }
            let parsed: Result<ToolsOnly, _> = toml::from_str(&source);
            assert!(
                parsed.is_err(),
                "the deleted token {token} still parses; the job file kept a second vocabulary"
            );
        }
    }
}
