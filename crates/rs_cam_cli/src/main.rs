#![deny(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
#![allow(clippy::print_stderr)] // CLI uses eprintln! for user-facing diagnostic output
#![allow(clippy::print_stdout)] // CLI `version` prints build info to stdout

mod command;
mod job;
mod nc_replay;
mod project;
mod run;
mod smoke;
mod sweep;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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

        /// Simulation resolution in mm.
        ///
        /// W5 item (f): NOT defaulted. clap cannot see the project, so the
        /// refusal lives in `run_project_command`: a project whose plan has
        /// to simulate refuses without this flag, and refuses a cell size
        /// coarser than the rest it machines needs (R1). A project that
        /// plans no simulation still simulates once for the diagnostics, at
        /// 0.5 mm.
        #[arg(long)]
        resolution: Option<f64>,

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

        /// F-039 — modulation feed scale: the multiplier the modulator
        /// applies to the binding feed limit (default 1.0 = emit at the
        /// binding constraint). 0.7 backs off 30 % for safety margin;
        /// 1.1+ pushes past the limit (chipload-min still applies).
        /// Ignored under `--modulation-strategy band-mid`.
        #[arg(long, default_value_t = 1.0)]
        modulation_feed_scale: f64,

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
        /// the loaded project's post config is used.
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

        /// Machine profile to replay against, by name in the shared
        /// machine library (the one the GUI and MCP
        /// `load_machine_from_library` read). Without this flag the
        /// command uses the built-in `shapeoko_xxl_ricky_tuned` preset.
        #[arg(long)]
        machine: Option<String>,

        /// Maximum feed (mm/min). Overrides the loaded machine's
        /// cutting-feed ceiling. Without `--machine` the default is
        /// 4000, as it has always been.
        #[arg(long)]
        max_feed: Option<f64>,

        /// Rapid feed (mm/min) for `G0` moves. Overrides the loaded
        /// machine's travel rate. Without `--machine` the default is
        /// 10000, matching the Shapeoko XXL tuned `$110/$111`.
        #[arg(long)]
        rapid_feed: Option<f64>,
    },
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
                rs_cam_core::util::build_info::CORE_VERSION,
                rs_cam_core::util::build_info::GIT_DESC,
                rs_cam_core::util::build_info::BUILD_TIMESTAMP,
            );
            return Ok(());
        }
        Commands::Job {
            input,
            diagnostics,
            diagnostics_json,
            debug_trace,
        } => {
            job::run_job_command(&input, diagnostics, diagnostics_json, debug_trace)?;
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
            modulation_feed_scale,
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
                    rs_cam_core::dressup::feed_modulation::ModulationStrategy::BandMid
                }
                _ => rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
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
                modulation_feed_scale,
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
            machine,
            max_feed,
            rapid_feed,
        } => {
            nc_replay::run_nc_time(&inputs, machine.as_deref(), max_feed, rapid_feed)?;
        }
    }

    Ok(())
}
