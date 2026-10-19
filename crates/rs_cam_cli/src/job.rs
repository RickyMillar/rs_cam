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
//! depth = 6.0
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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use tracing::{debug, info, warn};

use rs_cam_core::{
    compute::catalog::{OperationConfig, OperationType},
    compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource},
    compute::tool_config::{ToolConfig, ToolId, ToolType},
    compute::{ModelUnits, build_cutter},
    dexel_stock::{StockCutDirection, TriDexelStock},
    gcode::{
        CoolantMode, GcodePhase, PhaseTool, PostFormat, ToolLoadExportPolicy, WizardOverlay,
        export_gcode_phases_with_overlay_checked,
    },
    geo::BoundingBox3,
    session::{
        AddModelArgs, AddToolArgs, AddToolpathArgs, Command, EditSelectionArgs, LoadedModel,
        ProjectSession, SetDressupFieldArgs, SetPostConfigArgs, SetStockConfigArgs,
        SetToolpathParamArgs, ToolpathConfig,
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
    /// Number of cutting flutes (default: 2). Used for chipload calculation in diagnostics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flute_count: Option<u32>,
    /// Corner radius for bull nose
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
    /// Included angle in degrees for V-bit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub included_angle: Option<f64>,
    /// Taper half-angle for tapered ball
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taper_angle: Option<f64>,
    /// Shaft diameter for tapered ball
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shaft_diameter: Option<f64>,
    /// Shank diameter above the cutting flutes (mm).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shank_diameter: Option<f64>,
    /// The length from the holder face to the tip (mm). Empty: the
    /// `ToolConfig::new_default` stickout, the value the op's session and
    /// the job's simulation cutter both take (F116 step 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stickout: Option<f64>,
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
    /// Dressup fields by their `DressupConfig::FIELD_DEFS` name.
    #[serde(default, skip_serializing_if = "toml::Table::is_empty")]
    pub dressups: toml::Table,
    /// The selection of the op (geometry selection P2.7): the
    /// `[operation.selection]` table in the shape of the project file's
    /// `operation.params.selection`. Empty: the default of the op. The
    /// `edit_toolpath_selection` command applies it, so an op that holds
    /// no selection refuses it by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<rs_cam_core::selection::Selection>,
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
        "dressups",
        "selection",
    ];

    /// The registry op that `type` names, after the C2 check of every key.
    ///
    /// # Errors
    ///
    /// Refuses an unknown `type`, a parameter that the op's schema does not
    /// publish and a dressup key that `DressupConfig::FIELD_DEFS` does not
    /// name. The refusal names each bad key and the valid set.
    pub(crate) fn operation_type(&self) -> Result<OperationType> {
        let op_type = crate::run::parse_op_type(&self.op_type)?;
        let defs = op_type.registry_entry().params.defs;
        let unknown: Vec<&str> = self
            .params
            .keys()
            .map(String::as_str)
            .filter(|key| !defs.iter().any(|d| d.name == *key))
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
    // G11: the controller options, resolved by the one core rule. The job
    // file has no machine profile, so an omitted option is off.
    let overlay = {
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
    };
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
        let gcode = rs_cam_core::gcode::export_gcode_multi_setup_with_overlay_checked(
            &groups,
            post_def,
            job_file.job.safe_z,
            &rs_cam_core::tool_load::ToolLoadReport {
                per_toolpath: vec![],
            },
            ToolLoadExportPolicy::default(),
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

            // The job-file pipeline has no ProjectSession / load
            // evaluation context: pass an explicitly empty report
            // ("no load evaluation performed") so the gate is
            // visible at the call site rather than skippable.
            let gcode = export_gcode_phases_with_overlay_checked(
                &setup_phases,
                post_def,
                &rs_cam_core::tool_load::ToolLoadReport {
                    per_toolpath: vec![],
                },
                ToolLoadExportPolicy::default(),
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
        // See the per-setup branch above: the job-file path has no
        // load-evaluation context — explicitly empty report.
        let gcode = export_gcode_phases_with_overlay_checked(
            &phases,
            post_def,
            &rs_cam_core::tool_load::ToolLoadReport {
                per_toolpath: vec![],
            },
            ToolLoadExportPolicy::default(),
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
                rs_cam_core::ToolpathId(idx),
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
                    .map(rs_cam_core::ToolpathId)
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
            .get(ts.toolpath_id.0)
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

// ── Tool construction ──────────────────────────────────────────────────

/// Default cutting length when not specified in the job file.
const DEFAULT_CUTTING_LENGTH_FACTOR: f64 = 4.0;

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
    pub tool_id: Option<usize>,
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

/// The emitter phase of one job operation.
fn job_phase(phase: &OpResult) -> GcodePhase<'_> {
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
        let tool_id = Some(*tool_ids.entry(op.tool.clone()).or_insert(next_tool_id));
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
        let flute_count = tool_config.flute_count;
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
            ("taper_angle", def.taper_angle),
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
    let mut tc = ToolConfig::new_default(ToolId(0), def.tool_type);
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
    if let Some(stickout) = def.stickout {
        tc.stickout = stickout;
    }
    Ok(tc)
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
    let mut session = ProjectSession::new_empty();

    // \u{2500}\u{2500} Model \u{2500}\u{2500}
    let model_name = op
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("op_{i}_model"));
    let units = op.scale.map(ModelUnits::Custom);
    let model = LoadedModel::from_file(0, &model_name, &op.input, None, units, job_dir)
        .map_err(|e| anyhow::anyhow!("loading input '{}': {e}", op.input.display()))?;
    let _ = crate::command::apply_command(
        &mut session,
        Command::AddModel(AddModelArgs {
            model: Box::new(model),
        }),
    )?;

    // \u{2500}\u{2500} Stock: a set top only; the session sizes the rest \u{2500}\u{2500}
    if let Some(top) = op.stock_top_z {
        let mut stock = session.stock_config().clone();
        stock.auto_from_model = false;
        stock.z = (top - stock.origin_z).max(0.0);
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
        .id
        .0;
    let tp_index = crate::command::apply_command(
        &mut session,
        Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(ToolpathConfig {
                shoe: Default::default(),
                id: rs_cam_core::ToolpathId(0),
                name: format!("op_{}_{}", i, op_type.kind_str()),
                enabled: true,
                operation: OperationConfig::new_default(op_type),
                dressups: DressupConfig::default(),
                heights: HeightsConfig::default(),
                tool_id,
                model_id: 0,
                pre_gcode: None,
                post_gcode: None,
                boundary: BoundaryConfig::default(),
                boundary_inherit: true,
                rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
                stock_source: StockSource::default(),
                coolant: op.coolant,
                debug_options,
                feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
                planner_origin: None,
            }),
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
    if let Some(selection) = &op.selection {
        let command = Command::EditSelection(EditSelectionArgs {
            index: tp_index,
            target: rs_cam_core::session::SelectionTarget::Op,
            edit: rs_cam_core::selection::SelectionEdit::Replace(Box::new(selection.clone())),
        });
        let _ = crate::command::apply_command(&mut session, command)
            .map_err(|e| anyhow::anyhow!("selection: {e}"))?;
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
        // The one size convention (`Ø` diameter, `R` radius) shared by
        // every surface; see `ToolConfig::size_label`.
        let tool_summary = tool_config_from_def(tool_def, &op.tool)?.summary();
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
            result.semantic_trace.as_deref().cloned(),
        ))
    } else {
        None
    };

    let config = session.get_toolpath_config(tp_index);
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// The cutter of a job tool, through the one core builder.
    fn cutter_of(def: &ToolDef) -> Result<rs_cam_core::tool::ToolDefinition> {
        phase_cutter(&tool_config_from_def(def, "t")?)
    }

    fn end_mill_def() -> ToolDef {
        ToolDef {
            tool_type: ToolType::EndMill,
            number: None,
            diameter: 6.0,
            flute_count: None,
            corner_radius: None,
            included_angle: None,
            taper_angle: None,
            shaft_diameter: None,
            shank_diameter: None,
            stickout: None,
        }
    }

    /// F116 step 1: the job's cutter comes from core `build_cutter`, so a
    /// tool that `ToolConfig::validate` refuses is refused here too. The
    /// deleted CLI `build_tool` built a cutter with zero flutes and a
    /// negative stickout.
    #[test]
    fn f116_the_job_cutter_refuses_an_out_of_domain_tool() {
        let zero_flutes = ToolDef {
            flute_count: Some(0),
            ..end_mill_def()
        };
        let err = cutter_of(&zero_flutes).err().map(|e| format!("{e:#}"));
        assert!(
            err.as_deref().is_some_and(|e| e.contains("flute_count")),
            "{err:?}"
        );
        let negative_stickout = ToolDef {
            stickout: Some(-1.0),
            ..end_mill_def()
        };
        assert!(cutter_of(&negative_stickout).is_err());

        // The job cutter is the session tool: the same stickout and flutes.
        let tool = tool_config_from_def(&end_mill_def(), "t").unwrap();
        let cutter = cutter_of(&end_mill_def()).unwrap();
        assert_eq!(cutter.stickout.to_bits(), tool.stickout.to_bits());
        assert_eq!(cutter.flute_count, tool.flute_count);
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
    /// An unknown token refuses the job file; the aliases still parse.
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
        assert_eq!(job(Some("GRBL_HAL")).unwrap().job.post, PostFormat::GrblHal);
        assert_eq!(job(None).unwrap().job.post, PostFormat::Grbl);
        // The sweep writes the job back and reads it again.
        let round = toml::to_string(&job(Some("linux_cnc")).unwrap()).unwrap();
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
            flute_count: None,
            corner_radius: None,
            included_angle: None,
            taper_angle: None,
            shaft_diameter: None,
            shank_diameter: None,
            stickout: None,
        };
        let cutter = cutter_of(&flat_def).unwrap();
        assert!((cutter.radius() - 3.0).abs() < 1e-10);

        let ball_def = ToolDef {
            tool_type: ToolType::BallNose,
            number: None,
            diameter: 10.0,
            flute_count: None,
            corner_radius: None,
            included_angle: None,
            taper_angle: None,
            shaft_diameter: None,
            shank_diameter: None,
            stickout: None,
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
            flute_count: None,
            corner_radius: None,
            included_angle: None,
            taper_angle: Some(10.0),
            shaft_diameter: Some(12.0),
            shank_diameter: None,
            stickout: None,
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
                "type = \"pocket\"\ninput = \"m.svg\"\ntool = \"a\"\ndepth = 3.0\n\
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

    /// C2: a parameter and a dressup field of the job file reach the
    /// generated toolpath through the core setters.
    #[test]
    fn job_parameters_and_dressups_reach_the_toolpath() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let run = |extra: &str| {
            let source = format!(
                "[job]\noutput = \"o.nc\"\n[tools.t]\ntype = \"end_mill\"\ndiameter = 6.0\n\
                 [[operation]]\ntype = \"pocket\"\ninput = \"demo_pocket.svg\"\ntool = \"t\"\n\
                 depth = 3.0\n{extra}"
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
                #[allow(dead_code)]
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
