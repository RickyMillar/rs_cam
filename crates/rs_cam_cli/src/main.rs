#![deny(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
#![allow(clippy::print_stderr)] // CLI uses eprintln! for user-facing diagnostic output
#![allow(clippy::print_stdout)] // CLI `version` prints build info to stdout

mod job;
mod nc_replay;
mod project;
mod run;
mod smoke;
mod sweep;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rs_cam_core::{
    dexel_stock::{StockCutDirection, TriDexelStock},
    gcode::{
        GcodePhase, PhaseTool, ToolLoadExportPolicy, export_gcode_phases_checked,
        get_post_definition,
    },
    geo::BoundingBox3,
    simulation_cut::{
        AirCutRatios as _, SimulationCutArtifact, SimulationCutIssueKind, SimulationCutTrace,
    },
    tool::MillingCutter as _,
    toolpath::Toolpath,
};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Parser)]
#[command(name = "rs_cam", about = "3-axis wood router CAM toolpath generator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Print build/version info for all workspace crates
    Version,
    /// Run a TOML job file with multiple tools and operations
    Job {
        /// Path to the .toml job file
        input: PathBuf,

        /// Enable cutting metrics analysis (requires simulate = true in job file)
        #[arg(long)]
        diagnostics: bool,

        /// Write diagnostics JSON artifact to this path
        #[arg(long)]
        diagnostics_json: Option<PathBuf>,

        /// Write toolpath debug trace JSON artifacts to this directory
        #[arg(long)]
        debug_trace: Option<PathBuf>,
    },

    /// Run any operation through the registry-driven session pipeline
    ///
    /// ONE generic subcommand for all operations: parameters come from the
    /// operation registry (`run <op> --list-params`), values apply through the
    /// same validation path the GUI/MCP use, and execution routes through the
    /// session compute pipeline. New operations appear here automatically.
    Run {
        /// Operation kind (snake_case, e.g. pocket, profile, adaptive3d).
        /// See `run --list-ops` for the full set.
        op: Option<String>,

        /// List all operations with labels and menu categories, then exit
        #[arg(long)]
        list_ops: bool,

        /// List this operation's settable parameters, then exit
        #[arg(long)]
        list_params: bool,

        /// Input model file (.stl, .svg, .dxf, .step)
        #[arg(long)]
        input: Option<PathBuf>,

        /// Model units: mm, cm, m, inch, or a numeric scale factor
        #[arg(long, default_value = "mm")]
        units: String,

        /// Tool spec `type:diameter`, e.g. end_mill:6.35, ball_nose:6.0
        #[arg(long)]
        tool: Option<String>,

        /// Tool parameter override `key=value` (repeatable), e.g.
        /// --tool-set corner_radius=1.0 --tool-set flute_count=2
        #[arg(long = "tool-set")]
        tool_set: Vec<String>,

        /// Operation parameter `key=value` (repeatable; names from
        /// --list-params), e.g. --set depth=6.0 --set stepover=2.0
        #[arg(long = "set")]
        set: Vec<String>,

        /// Output G-code file
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Optional SVG toolpath preview
        #[arg(long)]
        svg: Option<PathBuf>,

        /// Post-processor: grbl, linuxcnc, mach3
        #[arg(long, default_value = "grbl")]
        post: String,

        /// Safe Z height for rapid moves
        #[arg(long, default_value = "10.0")]
        safe_z: f64,

        /// Spindle speed in RPM
        #[arg(long, default_value = "18000")]
        spindle_speed: u32,
    },

    /// Run a parameter sweep on a TOML job file
    ///
    /// Varies one parameter across multiple values, running the full job pipeline
    /// for each, and produces JSON fingerprints, diffs, SVGs, and G-code.
    Sweep {
        /// Base TOML job file to sweep over
        input: PathBuf,

        /// Parameter name to vary (e.g. stepover, depth, feed_rate)
        #[arg(long)]
        param: String,

        /// Comma-separated values to sweep (e.g. "0.5,1.0,2.0,4.0")
        #[arg(long)]
        values: String,

        /// Output directory for sweep results
        #[arg(long)]
        output_dir: PathBuf,

        /// Run simulation and produce stock heightmap SVGs
        #[arg(long)]
        simulate: bool,
    },

    /// Analyze a GUI project file with full diagnostics
    ///
    /// Loads the GUI project TOML format (format_version=3), executes all
    /// enabled toolpaths through the core algorithms with full debug and
    /// semantic tracing, runs tri-dexel simulation with cut metrics, checks
    /// collisions, and writes structured JSON diagnostics.
    Project {
        /// Path to the project .toml file (GUI format, format_version=3)
        input: PathBuf,

        /// Output directory for diagnostic artifacts
        #[arg(long, default_value = "diagnostics")]
        output_dir: PathBuf,

        /// Run only this setup (by name or ID)
        #[arg(long)]
        setup: Option<String>,

        /// Skip these toolpath IDs (comma-separated)
        #[arg(long)]
        skip: Option<String>,

        /// Simulation resolution in mm
        #[arg(long, default_value = "0.5")]
        resolution: f64,

        /// Print human-readable summary to stderr
        #[arg(long)]
        summary: bool,

        /// F-036c calibration helper: emit G-code (concatenated across
        /// enabled toolpaths) to this path after simulation. Reads the
        /// modulated `Toolpath` IR when `--adaptive-feed-modulation`
        /// is set, so the emitted feeds match what the GUI export
        /// would produce in the same state.
        #[arg(long)]
        emit_gcode: Option<PathBuf>,

        /// Disable F-036's per-segment adaptive feed modulation during
        /// simulation.
        ///
        /// **Checkpoint K (g1), 2026-08-13 — the default flipped to
        /// ON.** Checkpoint J flipped
        /// `SimulationOptions::default().adaptive_feed_modulation` to
        /// `true` and the GUI pins `true`, but this flag defaulted to
        /// `false`, so the same project got a *different operating
        /// point* — and therefore different `Within`/`Exceeds`
        /// verdicts and different reported feeds — depending on which
        /// front end asked. Modulation is what parks a feed on the band
        /// ceiling (A-5 measured `ConstrainedMax` rewriting 100 % of
        /// moves), so the divergence was not cosmetic.
        ///
        /// This **moves CLI numbers for every existing invocation**.
        /// `--no-adaptive-feed-modulation` reproduces the old behaviour
        /// exactly, which A-5's own evidence needed. `cli smoke` pins
        /// `false` explicitly and does not inherit this default (it is a
        /// fingerprint harness).
        ///
        /// Modulation still requires `MachineProfile.kinematics` — for
        /// projects predating F-034 combine with
        /// `--inject-shapeoko-kinematics`, or the pass is a no-op.
        #[arg(long = "no-adaptive-feed-modulation", action = clap::ArgAction::SetTrue)]
        no_adaptive_feed_modulation: bool,

        /// F-039 — modulation algorithm: `constrained-max` (default;
        /// per-move binding-constraint solver) or `band-mid` (F-036's
        /// "target band-mid" heuristic, kept as a fallback for one
        /// release cycle).
        #[arg(long, default_value = "constrained-max")]
        modulation_strategy: String,

        /// F-039 — modulation aggressiveness scalar (default 1.0 =
        /// emit at the binding constraint). 0.7 backs off 30 % for
        /// safety margin; 1.1+ pushes past the limit (chipload-min
        /// still applies). Ignored under `--modulation-strategy
        /// band-mid`.
        #[arg(long, default_value_t = 1.0)]
        modulation_aggressiveness: f64,

        /// Inject the Shapeoko XXL stock-kinematics preset
        /// (250 mm/s² accel, full-stop junction, max-feed cap) into
        /// the loaded `MachineProfile`. Used for F-036c calibration
        /// against the user's machine without editing the project file.
        #[arg(long)]
        inject_shapeoko_kinematics: bool,

        /// Apply LUT-suggested feeds/speeds to every enabled toolpath
        /// before generation. Replaces each operation's feed_rate,
        /// plunge_rate, stepover, depth_per_pass, and spindle_rpm
        /// with feeds::suggest_for_operation output (vendor-LUT-driven,
        /// machine-aware). Project file on disk is unchanged; only
        /// the in-memory session before generate + emit. Pairs with
        /// --emit-gcode for "what would suggest produce" without
        /// editing the .toml.
        #[arg(long)]
        apply_suggest: bool,

        /// Override the project's spindle policy for this run.
        /// "match_chart" (default in code; preserved when unset) uses
        /// the vendor LUT row's chart RPM verbatim. "max_speed" walks
        /// the constant-chipload line up to the spindle ceiling and
        /// scales feed proportionally. When unset, the strategy from
        /// the loaded project's ProjectPostConfig is used.
        #[arg(long, value_parser = ["match_chart", "max_speed"])]
        spindle_strategy: Option<String>,
    },

    /// Run the F-037 smoke baseline suite.
    ///
    /// Iterates `planning/toolpath_acceptance/cases_agent_smoke.csv`,
    /// generates + simulates each case, and writes per-toolpath verdicts.
    /// Use `--diff` to compare a captured run against a checked-in baseline
    /// (exits non-zero on regression).
    Smoke {
        /// Path to the smoke case matrix CSV.
        #[arg(
            long,
            default_value = "planning/toolpath_acceptance/cases_agent_smoke.csv"
        )]
        input: PathBuf,

        /// Output baseline CSV (one row per case).
        #[arg(long)]
        output: Option<PathBuf>,

        /// Diff mode: compare two baselines. Pass `--baseline` (prior) and
        /// `--output` (current). Exits non-zero if any verdict regressed.
        #[arg(long)]
        diff: bool,

        /// Baseline CSV to diff against (only used with --diff).
        #[arg(long)]
        baseline: Option<PathBuf>,

        /// Simulation resolution in mm.
        #[arg(long, default_value = "0.5")]
        resolution: f64,
    },

    /// Parse a .nc G-code file and predict cycle time via the F-034
    /// kinematics integrator. Independent cross-check on the time the
    /// `project` subcommand printed during generation — useful before a
    /// bench session to confirm the emitter didn't drop / add moves.
    NcTime {
        /// One or more .nc files to analyze.
        inputs: Vec<PathBuf>,

        /// Maximum feed (mm/min). Default 4000 matches the typical
        /// MachineProfile cap; override per-file if the project uses
        /// something else.
        #[arg(long, default_value_t = 4000.0)]
        max_feed: f64,

        /// Rapid feed (mm/min) for `G0` moves. Default 10000 matches the
        /// Shapeoko XXL tuned `$110/$111`.
        #[arg(long, default_value_t = 10000.0)]
        rapid_feed: f64,
    },
}

// SAFETY: `parts.len() < 2` guard above ensures indices 0 and 1 are in-bounds.
#[allow(clippy::indexing_slicing)]
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

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Version => {
            // Single source of truth for the git desc + build time is
            // rs_cam_core's build.rs; each crate reports its own semver.
            println!(
                "rs_cam_cli {}\nrs_cam_core {}\ngit {}\nbuilt {}",
                env!("CARGO_PKG_VERSION"),
                rs_cam_core::build_info::CORE_VERSION,
                rs_cam_core::build_info::GIT_DESC,
                rs_cam_core::build_info::BUILD_TIMESTAMP,
            );
            return Ok(());
        }
        Commands::Job {
            input,
            diagnostics,
            diagnostics_json,
            debug_trace,
        } => {
            let job_path = input
                .canonicalize()
                .context(format!("Job file not found: {}", input.display()))?;
            let job_dir = job_path.parent().unwrap_or(Path::new("."));
            debug!(path = %job_path.display(), "Loading job file");

            let mut job_file = job::parse_job_file(&job_path)?;
            // CLI flags override TOML config
            if diagnostics {
                job_file.job.diagnostics = true;
            }
            if diagnostics_json.is_some() {
                job_file.job.diagnostics_json = diagnostics_json;
            }
            let debug_trace_dir =
                debug_trace.map(|p| if p.is_absolute() { p } else { job_dir.join(p) });

            info!(
                tools = job_file.tools.len(),
                operations = job_file.operation.len(),
                output = %job_file.job.output.display(),
                "Job loaded"
            );

            let job_result = job::execute_job(&job_file, job_dir, debug_trace_dir.is_some())?;
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
                    match rs_cam_core::semantic_trace::write_toolpath_trace_artifact(
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
                            tool: phase.tool_id.zip(phase.tool_number).map(|(id, number)| {
                                PhaseTool {
                                    id,
                                    number,
                                    label: &phase.tool_name,
                                }
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
                    std::fs::write(&setup_output, &gcode)
                        .context("Failed to write setup output file")?;
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
                let svg_content = rs_cam_core::viz::toolpath_to_svg(toolpath, 800.0, 600.0);
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
                    let mut stock =
                        TriDexelStock::from_bounds(&sim_bbox, job_file.job.sim_resolution);

                    debug!(
                        phases = job_result.phases.len(),
                        resolution_mm = job_file.job.sim_resolution,
                        "Running stacked simulation"
                    );

                    // Simulate each phase with its own cutter
                    for phase in &job_result.phases {
                        stock.simulate_toolpath(
                            &phase.toolpath,
                            &phase.cutter,
                            StockCutDirection::FromTop,
                        );
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

                    use rs_cam_core::viz::SimPhase;
                    let sim_phases: Vec<SimPhase> = job_result
                        .phases
                        .iter()
                        .map(|p| SimPhase {
                            toolpath: &p.toolpath,
                            cutter: &p.cutter,
                            label: p.label.clone(),
                        })
                        .collect();

                    rs_cam_core::viz::stacked_simulation_3d_html(
                        &sim_phases,
                        &stock,
                        source_mesh.as_ref(),
                    )
                } else {
                    rs_cam_core::viz::toolpath_standalone_3d_html(toolpath, None)
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
        }

        Commands::Run {
            op,
            list_ops,
            list_params,
            input,
            units,
            tool,
            tool_set,
            set,
            output,
            svg,
            post,
            safe_z,
            spindle_speed,
        } => {
            run::run_generic(&run::RunArgs {
                op,
                list_ops,
                list_params,
                input,
                units,
                tool,
                tool_set,
                set,
                output,
                svg,
                post,
                safe_z,
                spindle_speed,
            })?;
        }

        Commands::Sweep {
            input,
            param,
            values,
            output_dir,
            simulate,
        } => {
            let job_path = input
                .canonicalize()
                .context(format!("Job file not found: {}", input.display()))?;
            sweep::run_sweep(&job_path, &param, &values, &output_dir, simulate)?;
        }

        Commands::Project {
            input,
            output_dir,
            setup,
            skip,
            resolution,
            summary,
            emit_gcode,
            no_adaptive_feed_modulation,
            modulation_strategy,
            modulation_aggressiveness,
            inject_shapeoko_kinematics,
            apply_suggest,
            spindle_strategy,
        } => {
            let skip_ids: Vec<rs_cam_core::ToolpathId> = skip
                .as_deref()
                .unwrap_or("")
                .split(',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.trim().parse().ok().map(rs_cam_core::ToolpathId))
                .collect();
            let strategy = match modulation_strategy.as_str() {
                "band-mid" | "bandmid" | "band_mid" => {
                    rs_cam_core::feed_modulation::ModulationStrategy::BandMid
                }
                _ => rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
            };
            let spindle_strat_override = spindle_strategy.as_deref().and_then(|s| match s {
                "match_chart" => Some(rs_cam_core::feeds::SpindleStrategy::MatchChart),
                "max_speed" => Some(rs_cam_core::feeds::SpindleStrategy::MaxSpeed),
                _ => None,
            });
            project::run_project_command(
                &input,
                &output_dir,
                setup.as_deref(),
                &skip_ids,
                resolution,
                summary,
                emit_gcode.as_deref(),
                // Checkpoint K (g1) — default ON, opt out explicitly.
                !no_adaptive_feed_modulation,
                strategy,
                modulation_aggressiveness,
                inject_shapeoko_kinematics,
                apply_suggest,
                spindle_strat_override,
            )?;
        }
        Commands::Smoke {
            input,
            output,
            diff,
            baseline,
            resolution,
        } => {
            if diff {
                let baseline_path = baseline
                    .as_ref()
                    .context("--diff requires --baseline <path>")?;
                let current_path = output
                    .as_ref()
                    .context("--diff requires --output <path> (the current run)")?;
                let regressed = smoke::run_diff(baseline_path, current_path)?;
                if regressed {
                    std::process::exit(1);
                }
            } else {
                let output_path =
                    output.context("--output <path> required when not in --diff mode")?;
                smoke::run_smoke(&input, &output_path, resolution)?;
            }
        }
        Commands::NcTime {
            inputs,
            max_feed,
            rapid_feed,
        } => {
            nc_replay::run_nc_time(&inputs, max_feed, rapid_feed)?;
        }
    }

    Ok(())
}
