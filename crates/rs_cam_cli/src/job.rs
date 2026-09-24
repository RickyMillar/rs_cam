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
//! ```

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use tracing::{debug, info, warn};

use rs_cam_core::{
    compute::ModelUnits,
    compute::catalog::{OperationConfig, OperationType},
    compute::config::{
        BoundaryConfig, DressupConfig, DressupEntryStyle, HeightsConfig, StockSource,
    },
    compute::tool_config::{ToolConfig, ToolId, ToolType},
    dexel_stock::{StockCutDirection, TriDexelStock},
    gcode::{
        CoolantMode, GcodePhase, PhaseTool, ToolLoadExportPolicy, export_gcode_phases_checked,
        get_post_definition,
    },
    geo::BoundingBox3,
    session::{
        AddModelArgs, AddToolArgs, AddToolpathArgs, Command, LoadedModel, ProjectSession,
        SetPostConfigArgs, SetStockConfigArgs, SetToolpathParamArgs, ToolpathConfig,
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
    #[serde(default = "default_post")]
    pub post: String,
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

#[derive(Deserialize, Serialize)]
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
    /// Shank length above the cutting flutes (mm).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shank_length: Option<f64>,
    /// Holder diameter (mm).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder_diameter: Option<f64>,
    /// Holder length (mm).
    #[allow(dead_code)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder_length: Option<f64>,
}

#[derive(Deserialize, Serialize)]
pub struct OperationDef {
    #[serde(rename = "type")]
    pub op_type: String,
    pub input: PathBuf,
    pub tool: String,
    /// Which setup this operation belongs to. If absent, belongs to a default setup.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,

    // Common parameters (override job defaults if present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stepover: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_per_pass: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plunge_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_z: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spindle_speed: Option<u32>,
    #[serde(default)]
    pub coolant: CoolantMode,

    // Pocket-specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub climb: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,

    // Profile-specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_height: Option<f64>,

    // Dogbone
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dogbone: Option<bool>,

    // Adaptive-specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_clearing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_cutting_radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z_blend: Option<bool>,

    // Rest machining-specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_tool: Option<String>,

    // STL scaling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,

    // 3D adaptive-specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock_top_z: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock_to_leave: Option<f64>,
    /// Entry style for 3D ops. CLI-01: the `entry_style` alias is
    /// deleted; the key is `entry_3d`, and the 2D `entry` key is still
    /// the fallback.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_3d: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detect_flat_areas: Option<bool>,
    /// Region order for 3D Rough: `global` (the default) or `by_area`.
    /// Another value is refused, not read as `global` (F7).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_by: Option<String>,
    /// Clearing strategy: "agent" (default) or "contour"/"contour_parallel".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    /// F-038: minimum forecast cut length (mm) for a marching-squares region
    /// to be retained in AgentSearch. Default 5.0 mm. Set to 0.0 to disable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_region_cut_length_mm: Option<f64>,
    /// F-038b: maximum XY stay-down link distance (mm) between cut groups.
    /// `None` ⇒ planner defaults to 8 × tool diameter. `Some(0.0)` disables.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_stay_down_distance_mm: Option<f64>,
    /// F-038b: vertical clearance (mm) above the heightfield sample max
    /// when emitting a keep-tool-down link. Default 0.5 mm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stay_down_clearance_mm: Option<f64>,
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
    let post_def = get_post_definition(&job_file.job.post).context(format!(
        "Unknown post-processor '{}'. Supported: grbl, linuxcnc, mach3",
        job_file.job.post
    ))?;
    if !job_file.setup.is_empty() {
        for setup_def in &job_file.setup {
            let setup_phases: Vec<GcodePhase<'_>> = job_result
                .phases
                .iter()
                .filter(|phase| phase.setup_name.as_deref() == Some(&setup_def.name))
                .map(|phase| GcodePhase {
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
                })
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
            let gcode = export_gcode_phases_checked(
                &setup_phases,
                post_def,
                &rs_cam_core::tool_load::ToolLoadReport {
                    per_toolpath: vec![],
                },
                ToolLoadExportPolicy::default(),
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
        let phases: Vec<GcodePhase<'_>> = job_result
            .phases
            .iter()
            .map(|phase| GcodePhase {
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
            })
            .collect();
        info!("Emitting G-code ({})...", post_def.name);
        // See the per-setup branch above: the job-file path has no
        // load-evaluation context — explicitly empty report.
        let gcode = export_gcode_phases_checked(
            &phases,
            post_def,
            &rs_cam_core::tool_load::ToolLoadReport {
                per_toolpath: vec![],
            },
            ToolLoadExportPolicy::default(),
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
            let json = serde_json::to_string_pretty(&artifact)
                .context("Failed to serialize diagnostics artifact")?;
            std::fs::write(&json_out, &json).context("Failed to write diagnostics JSON")?;
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
        ToolType::EndMill => Box::new(FlatEndmill::new(d, cl)),
        ToolType::BallNose => Box::new(BallEndmill::new(d, cl)),
        ToolType::BullNose => {
            let cr = def
                .corner_radius
                .context("Bull nose tool requires 'corner_radius'")?;
            Box::new(BullNoseEndmill::new(d, cr, cl))
        }
        ToolType::VBit => {
            let angle = def
                .included_angle
                .context("V-bit tool requires 'included_angle'")?;
            Box::new(VBitEndmill::new(d, angle, cl))
        }
        ToolType::TaperedBallNose => {
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
        let phase_cutter = build_tool(tool_def)?;
        let flute_count = tool_def.flute_count.unwrap_or(2);
        phases.push(OpResult {
            toolpath: tp,
            cutter: phase_cutter,
            label,
            spindle_speed: op.spindle_speed.unwrap_or(job.job.spindle_speed),
            tool_id,
            tool_number,
            tool_name: op.tool.clone(),
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
    let _ = crate::command::apply_command(
        &mut session,
        Command::AddModel(AddModelArgs {
            model: Box::new(model),
        }),
    )?;

    // Adaptive3d stock-frame fidelity: the pre-T9 router defaulted the
    // stock top to `model_top + 5.0` when `stock_top_z` was unset.
    if op_type == OperationType::Adaptive3d {
        let bbox = session.models().first().and_then(LoadedModel::bbox);
        if let Some(bbox) = bbox {
            let top = op.stock_top_z.unwrap_or(bbox.max.z + 5.0);
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
    }

    // \u{2500}\u{2500} Tools \u{2500}\u{2500}
    let tool_idx = crate::command::apply_command(
        &mut session,
        Command::AddTool(AddToolArgs {
            tool: Box::new(tool_config_from_def(tool_def, &op.tool)),
        }),
    )?
    .created
    .context("add_tool reports no new tool index")?;
    let prev_tool_id = if op_type == OperationType::Rest {
        let prev_name = op
            .prev_tool
            .as_ref()
            .context("Rest requires 'prev_tool' referencing the larger tool")?;
        let prev_def = job.tools.get(prev_name).context(format!(
            "Rest 'prev_tool' references unknown tool '{prev_name}'"
        ))?;
        Some(
            crate::command::apply_command(
                &mut session,
                Command::AddTool(AddToolArgs {
                    tool: Box::new(tool_config_from_def(prev_def, prev_name)),
                }),
            )?
            .created
            .context("add_tool reports no new tool index")?,
        )
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
        dressups.dogbone = Some(rs_cam_core::compute::config::DogboneParams { angle: 170.0 });
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
    let tp_index = crate::command::apply_command(
        &mut session,
        Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(ToolpathConfig {
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
                rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
                stock_source: StockSource::default(),
                coolant: op.coolant,
                face_selection: None,
                debug_options,
                feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
                planner_origin: None,
            }),
        }),
    )
    .map_err(|e| anyhow::anyhow!("adding toolpath: {e}"))?
    .created
    .context("add_toolpath reports no new toolpath index")?;

    // \u{2500}\u{2500} Parameters: registry-validated serde round-trip \u{2500}\u{2500}
    let drop_cutter_min_z = (op_type == OperationType::DropCutter)
        .then(|| session.models().first().and_then(LoadedModel::bbox))
        .flatten()
        .map(|bbox| bbox.min.z);
    let params = job_params_for(op, op_type, prev_tool_id, drop_cutter_min_z)?;
    let table = op_type.registry_entry().param_defs;
    for (key, value) in params {
        if table.iter().any(|d| d.name == key) {
            let command = Command::SetToolpathParam(SetToolpathParamArgs {
                index: tp_index,
                param: key.to_owned(),
                value,
            });
            let _ = crate::command::apply_command(&mut session, command)
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

    let mut post = session.post_config().clone();
    post.format = rs_cam_core::gcode::PostFormat::from_token(&job.job.post)
        .unwrap_or(rs_cam_core::gcode::PostFormat::Grbl);
    post.safe_z = op.safe_z.unwrap_or(job.job.safe_z);
    post.spindle_speed = op.spindle_speed.unwrap_or(job.job.spindle_speed);
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
        let tool_summary = tool_config_from_def(tool_def, &op.tool).summary();
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
            // L2 deleted the inert radial leave. The job file's single
            // `stock_to_leave` key now feeds the axial dial alone,
            // which is the only one the planner reads.
            let stl = op.stock_to_leave.unwrap_or(0.5);
            p.push(("stock_to_leave_axial", json!(stl)));
            p.push(("tolerance", json!(op.tolerance.unwrap_or(0.1))));
            p.push((
                "min_cutting_radius",
                json!(op.min_cutting_radius.unwrap_or(0.0)),
            ));
            // entry_3d falls back to the shared 2D `entry` field,
            // matching the pre-T9 router.
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
            // F7: an unknown value is refused. It was read as `global` in
            // silence, so a typo ran the other order with no message. The
            // tokens are the `region_ordering` tokens of the GUI and MCP.
            let ordering = match op.order_by.as_deref() {
                None | Some("global") => "global",
                Some("by_area") => "by_area",
                Some(other) => bail!(
                    "order_by = \"{other}\" is not a region order. Use \"global\" or \"by_area\"."
                ),
            };
            p.push(("region_ordering", json!(ordering)));
            let strategy = match op.strategy.as_deref().unwrap_or("contour") {
                "adaptive" => "adaptive",
                "agent" | "agent_search" => "agent_search",
                "spiral" | "contour_spiral" => "contour_spiral",
                _ => "contour_parallel",
            };
            p.push(("clearing_strategy", json!(strategy)));
            p.push(("z_blend", json!(op.z_blend.unwrap_or(false))));
            p.push((
                "detect_flat_areas",
                json!(op.detect_flat_areas.unwrap_or(false)),
            ));
            p.push((
                "min_region_cut_length_mm",
                json!(op.min_region_cut_length_mm.unwrap_or(15.0)),
            ));
            if let Some(d) = op.max_stay_down_distance_mm {
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

    #[test]
    fn test_tool_radius_uses_cutter_radius() {
        // Verify that build_tool returns cutters whose radius() matches
        // what the job executor would use (cutter.radius(), not diameter/2.0
        // from the TOML definition, which could differ for composite tools).
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
            shank_length: None,
            holder_diameter: None,
            holder_length: None,
        };
        let cutter = build_tool(&flat_def).unwrap();
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
            tool_type: ToolType::TaperedBallNose,
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

    /// CLI-01 sentry: the job file has ONE stay-down key, and the
    /// deleted one no longer feeds it.
    ///
    /// `OperationDef` carried both `max_stay_down_dist` and
    /// `max_stay_down_distance_mm`, and `job_params_for` coalesced them
    /// with `.or()`. A job file setting BOTH silently lost one of them,
    /// with no warning. The same string also named an unrelated, dead
    /// `Adaptive3dParams` field, so the two could not be told apart by
    /// name. Only `max_stay_down_distance_mm` — the name every
    /// `OperationConfig` registry, the GUI and MCP `set_toolpath_param`
    /// use — reaches the core now.
    #[test]
    fn the_job_file_has_one_stay_down_key() {
        let with_live_key: OperationDef = toml::from_str(
            "type = \"adaptive3d\"\ninput = \"m.stl\"\ntool = \"a\"\nmax_stay_down_distance_mm = 18.0\n",
        )
        .unwrap();
        let params = job_params_for(&with_live_key, OperationType::Adaptive3d, None, None).unwrap();
        assert_eq!(
            params
                .iter()
                .find(|(k, _)| *k == "max_stay_down_distance_mm")
                .map(|(_, v)| v.clone()),
            Some(serde_json::json!(18.0)),
            "the live key must still reach the core"
        );

        // The deleted key is not a second door into the same param.
        let with_deleted_key: OperationDef = toml::from_str(
            "type = \"adaptive3d\"\ninput = \"m.stl\"\ntool = \"a\"\nmax_stay_down_dist = 30.0\n",
        )
        .unwrap();
        let params =
            job_params_for(&with_deleted_key, OperationType::Adaptive3d, None, None).unwrap();
        assert!(
            !params
                .iter()
                .any(|(k, _)| *k == "max_stay_down_distance_mm"),
            "the deleted key still sets the param; the coalesce survived"
        );
    }

    /// The step ladder was removed on 2026-09-24 (operator ruling). A job
    /// file that sets `coarse_steps` still parses; the key is ignored and
    /// no ladder reaches the core.
    #[test]
    fn the_coarse_steps_key_is_ignored() {
        let op: OperationDef = toml::from_str(
            "type = \"adaptive3d\"\ninput = \"m.stl\"\ntool = \"a\"\ncoarse_steps = [10.0]\n",
        )
        .unwrap();
        let params = job_params_for(&op, OperationType::Adaptive3d, None, None).unwrap();
        assert!(!params.iter().any(|(k, _)| *k == "coarse_steps"));
        assert!(
            !OperationType::Adaptive3d
                .registry_entry()
                .param_defs
                .iter()
                .any(|d| d.name == "coarse_steps"),
            "the registry still names coarse_steps"
        );
    }

    /// F7: an unknown `order_by` is refused with a message that names the
    /// value and the two tokens. It was read as `global` in silence.
    #[test]
    fn an_unknown_order_by_is_refused() {
        for (value, expected) in [("global", "global"), ("by_area", "by_area")] {
            let op: OperationDef = toml::from_str(&format!(
                "type = \"adaptive3d\"\ninput = \"m.stl\"\ntool = \"a\"\norder_by = \"{value}\"\n"
            ))
            .unwrap();
            let params = job_params_for(&op, OperationType::Adaptive3d, None, None).unwrap();
            assert_eq!(
                params
                    .iter()
                    .find(|(k, _)| *k == "region_ordering")
                    .map(|(_, v)| v.clone()),
                Some(serde_json::json!(expected))
            );
        }
        for value in ["depth", "by-area", "byarea"] {
            let op: OperationDef = toml::from_str(&format!(
                "type = \"adaptive3d\"\ninput = \"m.stl\"\ntool = \"a\"\norder_by = \"{value}\"\n"
            ))
            .unwrap();
            let err = job_params_for(&op, OperationType::Adaptive3d, None, None)
                .unwrap_err()
                .to_string();
            assert!(
                err.contains(value) && err.contains("by_area") && err.contains("global"),
                "the refusal must name the value and the two tokens: {err}"
            );
        }
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

    /// CLI-01 sentry: `entry_style` is not a second spelling of
    /// `entry_3d` any more.
    #[test]
    fn the_entry_style_alias_is_gone() {
        let op: OperationDef = toml::from_str(
            "type = \"adaptive3d\"\ninput = \"m.stl\"\ntool = \"a\"\nentry_style = \"helix\"\n",
        )
        .unwrap();
        assert_eq!(op.entry_3d, None, "the deleted alias still fills entry_3d");

        let op: OperationDef = toml::from_str(
            "type = \"adaptive3d\"\ninput = \"m.stl\"\ntool = \"a\"\nentry_3d = \"helix\"\n",
        )
        .unwrap();
        assert_eq!(op.entry_3d.as_deref(), Some("helix"));
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
