#![deny(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
#![allow(clippy::print_stderr)] // CLI uses eprintln! for user-facing diagnostic output
#![allow(clippy::print_stdout)] // CLI `version` prints build info to stdout

mod command;
mod job;
mod memory_budget;
mod nc_replay;
mod project;
mod rough_score;
mod run;
mod smoke;
mod sweep;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use rs_cam_core::dressup::feed_modulation::ModulationStrategy;
use rs_cam_core::feeds::SpindleStrategy;
use std::path::PathBuf;

// ── Typed flag tokens (F014, F028, F057) ────────────────────────────────
//
// Each flag that names a core enum parses into that enum at the clap
// boundary. An unknown token is a clap error; no flag falls back to a
// default in silence.

/// The `project --modulation-strategy` token of [`ModulationStrategy`].
///
/// Core has no clap dependency, so the CLI holds this mirror. The two
/// `From` impls match every variant on both sides, so a new core variant
/// does not compile until it has a token here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum ModulationStrategyArg {
    ConstrainedMax,
    BandMid,
}

impl From<ModulationStrategyArg> for ModulationStrategy {
    fn from(arg: ModulationStrategyArg) -> Self {
        match arg {
            ModulationStrategyArg::ConstrainedMax => ModulationStrategy::ConstrainedMax,
            ModulationStrategyArg::BandMid => ModulationStrategy::BandMid,
        }
    }
}

impl From<ModulationStrategy> for ModulationStrategyArg {
    fn from(strategy: ModulationStrategy) -> Self {
        match strategy {
            ModulationStrategy::ConstrainedMax => ModulationStrategyArg::ConstrainedMax,
            ModulationStrategy::BandMid => ModulationStrategyArg::BandMid,
        }
    }
}

/// The `project --spindle-strategy` token of [`SpindleStrategy`]. The
/// token is the core serde token (`match_chart`, `max_speed`), the one the
/// project file holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "snake_case")]
enum SpindleStrategyArg {
    MatchChart,
    MaxSpeed,
}

impl From<SpindleStrategyArg> for SpindleStrategy {
    fn from(arg: SpindleStrategyArg) -> Self {
        match arg {
            SpindleStrategyArg::MatchChart => SpindleStrategy::MatchChart,
            SpindleStrategyArg::MaxSpeed => SpindleStrategy::MaxSpeed,
        }
    }
}

/// The reverse map. A test walks it, so a new core variant does not
/// compile in the test build until it has a token here.
#[cfg(test)]
impl From<SpindleStrategy> for SpindleStrategyArg {
    fn from(strategy: SpindleStrategy) -> Self {
        match strategy {
            SpindleStrategy::MatchChart => SpindleStrategyArg::MatchChart,
            SpindleStrategy::MaxSpeed => SpindleStrategyArg::MaxSpeed,
        }
    }
}

/// The `run --post` parser. It calls the one core resolver,
/// [`rs_cam_core::gcode::PostFormat::from_token`], so the flag takes the
/// same tokens and aliases as a project file. An unknown token is refused.
fn parse_post_format(token: &str) -> Result<rs_cam_core::gcode::PostFormat, String> {
    use rs_cam_core::gcode::PostFormat;
    PostFormat::from_token(token).ok_or_else(|| {
        let valid: Vec<&str> = PostFormat::ALL.iter().map(|f| f.to_token()).collect();
        format!(
            "unknown post-processor '{token}'; supported: {}",
            valid.join(", ")
        )
    })
}

/// The parser of a simulation cell size in mm. Core refuses a cell that is
/// not finite and positive (`SetSimulationResolution`); this parser gives
/// the same refusal at the flag.
fn parse_positive_mm(token: &str) -> Result<f64, String> {
    let mm: f64 = token
        .trim()
        .parse()
        .map_err(|e| format!("'{token}' is not a number of mm: {e}"))?;
    if mm.is_finite() && mm > 0.0 {
        Ok(mm)
    } else {
        Err(format!("'{token}' is not a positive cell size in mm"))
    }
}

#[derive(Parser)]
#[command(name = "rs_cam", about = "3-axis wood router CAM toolpath generator")]
struct Cli {
    /// The memory budget of this run: a binary size ("12GiB", "512MiB"),
    /// a byte count, "unlimited", or "default" (half of the system RAM).
    ///
    /// When the process uses more than the budget, the generation plan
    /// stops and says so, before the system kills the process.
    ///
    /// Precedence: this flag overrides `[memory] limit` in the settings
    /// file (`$RS_CAM_SETTINGS`, else `$XDG_CONFIG_HOME/rs_cam/settings.toml`,
    /// else `~/.config/rs_cam/settings.toml`). With no flag and no file the
    /// limit is half of the system RAM. The GUI sets the file in
    /// File > Preferences.
    #[arg(
        long,
        global = true,
        value_name = "SIZE",
        value_parser = memory_budget::parse_memory_limit
    )]
    memory_limit: Option<rs_cam_core::budget::MemoryLimit>,

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

        /// Post-processor: grbl, grblhal, linuxcnc, mach3. An unknown token
        /// is refused (F028).
        #[arg(long, default_value = "grbl", value_parser = parse_post_format)]
        post: rs_cam_core::gcode::PostFormat,

        /// Safe Z height for rapid moves
        #[arg(long, default_value = "10.0")]
        safe_z: f64,

        /// Spindle speed in RPM
        #[arg(long, default_value = "18000")]
        spindle_speed: u32,

        /// grblHAL: the controller waits for the spindle ($394/$340), so
        /// no warm-up dwell is written (true/false)
        #[arg(long)]
        controller_waits_for_spindle: Option<bool>,

        /// grblHAL: the board has a mist output, so M7 is written
        /// (true/false)
        #[arg(long)]
        mist_output: Option<bool>,

        /// grblHAL: write drill operations as native G81/G82/G83 cycles
        /// (true/false)
        #[arg(long)]
        native_drill_cycles: Option<bool>,

        /// F051: export even when a tool-load criterion could not be
        /// modelled (no simulation, no vendor row, a stale trace). The
        /// MCP `export_gcode` key `accept_unmodeled_tool_load`. Off: refuse.
        #[arg(long = rs_cam_core::gcode::LoadOverride::AcceptUnmodeled.cli_flag())]
        accept_unmodeled_tool_load: bool,

        /// F051: export even when a tool-load criterion exceeded a bound
        /// that gates an export (a known-dangerous override). The MCP
        /// `export_gcode` key `accept_exceeded_tool_load`. Off: refuse.
        #[arg(long = rs_cam_core::gcode::LoadOverride::AcceptExceeded.cli_flag())]
        accept_exceeded_tool_load: bool,
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

        /// Output directory for diagnostic artifacts. Required.
        ///
        /// U5 (memory programme 2026-10-01): the command wrote into
        /// `./diagnostics` by default. `simulation.json` holds the full cut
        /// trace, and on a large project it is gigabytes (8 GB on
        /// rivmap350). The caller now names the directory, so the command
        /// never fills the current directory without a request.
        #[arg(long)]
        output_dir: PathBuf,

        /// Run only this setup (by name or ID)
        #[arg(long)]
        setup: Option<String>,

        /// Skip these toolpath IDs (comma-separated). A token that is not an
        /// id, or an id that the project does not hold, is refused (F014).
        #[arg(long, value_delimiter = ',')]
        skip: Vec<usize>,

        /// Override the project's stored simulation resolution, in mm.
        ///
        /// G-RESTRES: the project file stores the ONE resolution every
        /// simulation uses (`[job.simulation] resolution_mm`, auto when
        /// absent), so a rest cascade needs no flag. A value here replaces it
        /// for this run, and `summary.json` says `override`. A cell coarser
        /// than the rest needs is refused, and so is a cell that is not a
        /// positive number of mm.
        #[arg(long, value_parser = parse_positive_mm)]
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

        /// F051: export even when a tool-load criterion could not be
        /// modelled (no simulation, no vendor row, a stale trace). The
        /// MCP `export_gcode` key `accept_unmodeled_tool_load`. Off: refuse.
        #[arg(long = rs_cam_core::gcode::LoadOverride::AcceptUnmodeled.cli_flag())]
        accept_unmodeled_tool_load: bool,

        /// F051: export even when a tool-load criterion exceeded a bound
        /// that gates an export (a known-dangerous override). The MCP
        /// `export_gcode` key `accept_exceeded_tool_load`. Off: refuse.
        #[arg(long = rs_cam_core::gcode::LoadOverride::AcceptExceeded.cli_flag())]
        accept_exceeded_tool_load: bool,

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
        /// release cycle). An unknown token is refused (F014).
        #[arg(
            long,
            value_enum,
            default_value_t = ModulationStrategyArg::from(ModulationStrategy::default())
        )]
        modulation_strategy: ModulationStrategyArg,

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
        #[arg(long, value_enum)]
        spindle_strategy: Option<SpindleStrategyArg>,

        /// Write `simulation.json`, the per-sample cut-trace artifact, into
        /// the output directory. Default off.
        ///
        /// The file holds every cut-trace sample and is gigabytes on a large
        /// project (6.7 GB on rivmap350). The GUI cut-trace file is off by
        /// default (operator ruling 2026-10-02); this flag gives the CLI the
        /// same default. Breaking (2026-10-02): a run without the flag
        /// writes no `simulation.json`. `summary.json` and the `tp_*.json`
        /// files do not change, and the cut-trace figures in them are still
        /// measured: the run captures the trace in memory either way.
        #[arg(long)]
        sim_artifact: bool,
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

        /// Simulation resolution in mm: a positive number (F057).
        #[arg(long, default_value = "0.5", value_parser = parse_positive_mm)]
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

    /// Score toolpaths for a roughing A/B with the numbers the GUI and MCP
    /// publish, plus the whole-cycle engagement and entry and retract counts.
    ///
    /// Loads a project, applies `--set` overrides, walks the generation plan
    /// the `project` command walks, runs the closing simulation, and prints
    /// one JSON object per toolpath and one total object on stdout. Each
    /// record carries the core `ToolpathDiagnostic` and cut-trace summary
    /// under their own field names. The log goes to stderr.
    RoughScore {
        /// Path to the project .toml file (GUI format, format_version=3)
        input: PathBuf,

        /// Score only this toolpath (its id, or its exact name). The plan
        /// then makes only this toolpath and its ancestors current.
        #[arg(long)]
        toolpath: Option<String>,

        /// Override the project's stored simulation resolution, in mm. The
        /// same rule as `project` (G-RESTRES): omit it to use the project
        /// file's value; the report says which one ran.
        #[arg(long, value_parser = parse_positive_mm)]
        resolution: Option<f64>,

        /// Skip the closing simulation: time and counts only. The plan still
        /// runs the simulations that a `from_remaining_stock` operation needs
        /// before it generates. Every simulation field is then null.
        #[arg(long)]
        no_sim: bool,

        /// Override one operation parameter before generation, as
        /// `<index>.<param>=<value>` (repeatable). `<index>` is the toolpath
        /// position in the session, the index `set_toolpath_param` takes.
        #[arg(long = "set")]
        set: Vec<String>,

        /// Disable the per-segment adaptive feed modulation of the closing
        /// simulation. Default ON, the same as `project` and the GUI.
        #[arg(long = "no-adaptive-feed-modulation", action = clap::ArgAction::SetTrue)]
        no_adaptive_feed_modulation: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // The log subscriber starts below, so the settings-file warning waits
    // for it.
    let settings_warning = memory_budget::set(cli.memory_limit);

    // `rough-score` prints JSON records on stdout, so its log goes to
    // stderr. The other commands keep the stdout log they always had.
    let log_to_stderr = matches!(cli.command, Commands::RoughScore { .. });
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_timer(tracing_subscriber::fmt::time::uptime());
    if log_to_stderr {
        subscriber.with_writer(std::io::stderr).init();
    } else {
        subscriber.init();
    }
    if let Some(warning) = settings_warning {
        tracing::warn!("{warning}");
    }

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
            controller_waits_for_spindle,
            mist_output,
            native_drill_cycles,
            accept_unmodeled_tool_load,
            accept_exceeded_tool_load,
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
                controller_waits_for_spindle,
                mist_output,
                native_drill_cycles,
                tool_load: rs_cam_core::gcode::ToolLoadExportPolicy {
                    accept_unmodeled: accept_unmodeled_tool_load,
                    accept_exceeded: accept_exceeded_tool_load,
                },
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
            accept_unmodeled_tool_load,
            accept_exceeded_tool_load,
            no_adaptive_feed_modulation,
            modulation_strategy,
            modulation_feed_scale,
            inject_shapeoko_kinematics,
            apply_suggest,
            spindle_strategy,
            sim_artifact,
        } => {
            let skip_ids: Vec<rs_cam_core::ToolpathId> = skip
                .into_iter()
                .map(rs_cam_core::ToolpathId::from_wire)
                .collect();
            project::run_project_command(
                &input,
                &output_dir,
                setup.as_deref(),
                &skip_ids,
                resolution,
                summary,
                emit_gcode.as_deref(),
                rs_cam_core::gcode::ToolLoadExportPolicy {
                    accept_unmodeled: accept_unmodeled_tool_load,
                    accept_exceeded: accept_exceeded_tool_load,
                },
                // Checkpoint K (g1) — default ON, opt out explicitly.
                !no_adaptive_feed_modulation,
                modulation_strategy.into(),
                modulation_feed_scale,
                inject_shapeoko_kinematics,
                apply_suggest,
                spindle_strategy.map(Into::into),
                sim_artifact,
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
        Commands::RoughScore {
            input,
            toolpath,
            resolution,
            no_sim,
            set,
            no_adaptive_feed_modulation,
        } => {
            rough_score::run_rough_score(
                &input,
                &rough_score::ScoreOptions {
                    toolpath,
                    resolution,
                    no_sim,
                    set,
                    adaptive_feed_modulation: !no_adaptive_feed_modulation,
                },
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // SAFETY: test module; a failed unwrap or a panic is a failed test.
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::{Cli, Commands};
    use clap::Parser;
    use rs_cam_core::budget::{MemoryBudget, MemoryLimit};

    const GIB: u64 = 1 << 30;

    #[test]
    fn memory_limit_parses_a_size_a_byte_count_and_unlimited() {
        let cli = Cli::try_parse_from(["rs_cam", "--memory-limit", "12GiB", "version"]).unwrap();
        assert_eq!(cli.memory_limit, Some(MemoryLimit::Bytes(12 * GIB)));
        assert!(matches!(cli.command, Commands::Version));

        // `global = true`: the flag also goes after the subcommand.
        let cli = Cli::try_parse_from(["rs_cam", "version", "--memory-limit", "4096"]).unwrap();
        assert_eq!(cli.memory_limit, Some(MemoryLimit::Bytes(4096)));

        let cli =
            Cli::try_parse_from(["rs_cam", "--memory-limit", "Unlimited", "version"]).unwrap();
        assert_eq!(cli.memory_limit, Some(MemoryLimit::Unlimited));
    }

    #[test]
    fn memory_limit_refuses_a_decimal_unit_and_junk() {
        assert!(Cli::try_parse_from(["rs_cam", "--memory-limit", "12GB", "version"]).is_err());
        assert!(Cli::try_parse_from(["rs_cam", "--memory-limit", "lots", "version"]).is_err());
    }

    /// Operator ruling 2026-10-02: with no flag and no file the limit is
    /// half of the system RAM. This test replaces
    /// `no_memory_limit_flag_means_no_limit`, which pinned the pending "no
    /// limit".
    #[test]
    fn no_memory_limit_flag_means_half_of_the_system_memory() {
        let cli = Cli::try_parse_from(["rs_cam", "version"]).unwrap();
        assert_eq!(cli.memory_limit, None);
        assert_eq!(rs_cam_core::budget::DEFAULT_SYSTEM_FRACTION, Some(0.5));
        let total = rs_cam_core::budget::system_memory_bytes();
        assert_eq!(
            MemoryBudget::from_setting(cli.memory_limit.unwrap_or_default()),
            MemoryBudget::from_setting_on(MemoryLimit::Default, total)
        );
        assert_eq!(
            MemoryBudget::from_setting_on(MemoryLimit::Default, Some(16 * GIB)),
            MemoryBudget::with_limit(8 * GIB)
        );
        // "unlimited" still removes the limit, and "default" names it.
        let cli =
            Cli::try_parse_from(["rs_cam", "--memory-limit", "unlimited", "version"]).unwrap();
        assert_eq!(cli.memory_limit, Some(MemoryLimit::Unlimited));
        let cli = Cli::try_parse_from(["rs_cam", "--memory-limit", "default", "version"]).unwrap();
        assert_eq!(cli.memory_limit, Some(MemoryLimit::Default));
    }

    /// B5: the flag overrides the settings file, and with a flag the CLI
    /// does not read the file at all.
    #[test]
    fn the_flag_overrides_the_settings_file() {
        use rs_cam_core::settings::{AppSettings, LoadedSettings};

        let file = || LoadedSettings {
            settings: AppSettings {
                memory_limit: MemoryLimit::Bytes(3 * GIB),
                ..AppSettings::default()
            },
            path: None,
            warning: None,
        };

        let (limit, warning) = super::memory_budget::limit_from(None, file);
        assert_eq!(limit, MemoryLimit::Bytes(3 * GIB), "no flag: the file");
        assert_eq!(warning, None);

        let cli = Cli::try_parse_from(["rs_cam", "--memory-limit", "1GiB", "version"]).unwrap();
        let (limit, _) = super::memory_budget::limit_from(cli.memory_limit, || {
            panic!("a flag must not read the settings file")
        });
        assert_eq!(limit, MemoryLimit::Bytes(GIB), "the flag wins");

        let (limit, _) = super::memory_budget::limit_from(Some(MemoryLimit::Unlimited), || {
            panic!("a flag must not read the settings file")
        });
        assert_eq!(limit, MemoryLimit::Unlimited);
    }

    /// A bad file gives the default limit, and its warning reaches `main`.
    #[test]
    fn a_bad_settings_file_warns_and_gives_the_default() {
        use rs_cam_core::settings::load_from;

        let path = std::env::temp_dir().join(format!(
            "rs_cam_cli_settings_bad_{}.toml",
            std::process::id()
        ));
        std::fs::write(&path, "[memory]\nlimit = \"lots\"\n").unwrap();
        let (limit, warning) = super::memory_budget::limit_from(None, || load_from(&path));
        std::fs::remove_file(&path).unwrap();
        assert_eq!(limit, MemoryLimit::Default);
        assert!(warning.is_some_and(|w| w.contains("[memory] limit")));
    }

    #[test]
    fn the_help_states_the_precedence() {
        use clap::CommandFactory;

        let help = Cli::command().render_long_help().to_string();
        for token in ["overrides", "settings.toml", "RS_CAM_SETTINGS"] {
            assert!(help.contains(token), "--help lacks {token}: {help}");
        }
    }

    fn project_args<'a>(extra: &[&'a str]) -> Vec<&'a str> {
        let mut args = vec!["rs_cam", "project", "p.toml", "--output-dir", "out"];
        args.extend_from_slice(extra);
        args
    }

    /// F014: a `--skip` token that is not an id is a clap error. Before the
    /// fix, `3,4a` dropped `4a` and emitted toolpath 4.
    #[test]
    fn f014_skip_refuses_a_token_that_is_not_an_id() {
        assert!(Cli::try_parse_from(project_args(&["--skip", "3,4a"])).is_err());
        let cli = Cli::try_parse_from(project_args(&["--skip", "3,4"])).unwrap();
        let Commands::Project { skip, .. } = cli.command else {
            panic!("not the project command");
        };
        assert_eq!(skip, vec![3, 4]);
        let cli = Cli::try_parse_from(project_args(&[])).unwrap();
        let Commands::Project { skip, .. } = cli.command else {
            panic!("not the project command");
        };
        assert!(skip.is_empty());
    }

    /// F014: a `--modulation-strategy` typo is a clap error. Before the fix
    /// it ran ConstrainedMax. The deleted aliases are refused too.
    #[test]
    fn f014_modulation_strategy_refuses_a_typo() {
        use rs_cam_core::dressup::feed_modulation::ModulationStrategy;
        for bad in ["band-midd", "bandmid", "band_mid"] {
            assert!(
                Cli::try_parse_from(project_args(&["--modulation-strategy", bad])).is_err(),
                "{bad} parsed"
            );
        }
        let strategy = |extra: &[&str]| {
            let Commands::Project {
                modulation_strategy,
                ..
            } = Cli::try_parse_from(project_args(extra)).unwrap().command
            else {
                panic!("not the project command");
            };
            ModulationStrategy::from(modulation_strategy)
        };
        assert_eq!(
            strategy(&["--modulation-strategy", "band-mid"]),
            ModulationStrategy::BandMid
        );
        assert_eq!(strategy(&[]), ModulationStrategy::default());
    }

    /// F014: the `--spindle-strategy` tokens are the core serde tokens, and
    /// an unknown token is a clap error.
    #[test]
    fn f014_spindle_strategy_tokens_are_the_core_serde_tokens() {
        use super::SpindleStrategyArg;
        use clap::ValueEnum;
        use rs_cam_core::feeds::SpindleStrategy;
        for &arg in SpindleStrategyArg::value_variants() {
            let core = SpindleStrategy::from(arg);
            assert_eq!(SpindleStrategyArg::from(core), arg);
            let token = arg.to_possible_value().unwrap().get_name().to_owned();
            assert_eq!(serde_json::to_value(core).unwrap(), token.as_str());
        }
        assert!(Cli::try_parse_from(project_args(&["--spindle-strategy", "max-speed"])).is_err());
    }

    /// F028: an unknown `run --post` token is a clap error. Before the fix
    /// it emitted GRBL. The core aliases still parse.
    #[test]
    fn f028_run_post_refuses_an_unknown_token() {
        use rs_cam_core::gcode::PostFormat;
        assert!(Cli::try_parse_from(["rs_cam", "run", "pocket", "--post", "grbll"]).is_err());
        let post = |token: &str| {
            let Commands::Run { post, .. } =
                Cli::try_parse_from(["rs_cam", "run", "pocket", "--post", token])
                    .unwrap()
                    .command
            else {
                panic!("not the run command");
            };
            post
        };
        assert_eq!(post("GRBLHAL"), PostFormat::GrblHal);
        assert_eq!(post("linuxcnc"), PostFormat::LinuxCnc);
        let Commands::Run { post, .. } = Cli::try_parse_from(["rs_cam", "run", "pocket"])
            .unwrap()
            .command
        else {
            panic!("not the run command");
        };
        assert_eq!(post, PostFormat::Grbl);
    }

    /// F051 (W5): the tool-load overrides are explicit flags, named from
    /// core's `LoadOverride::cli_flag`, and both default to refuse.
    #[test]
    fn f051_the_tool_load_overrides_are_explicit_flags() {
        use rs_cam_core::gcode::LoadOverride;
        let unmodeled = format!("--{}", LoadOverride::AcceptUnmodeled.cli_flag());
        let exceeded = format!("--{}", LoadOverride::AcceptExceeded.cli_flag());
        let flags = |args: &[&str]| {
            let mut argv = vec!["rs_cam", "run", "pocket"];
            argv.extend_from_slice(args);
            let Commands::Run {
                accept_unmodeled_tool_load,
                accept_exceeded_tool_load,
                ..
            } = Cli::try_parse_from(argv).unwrap().command
            else {
                panic!("not the run command");
            };
            (accept_unmodeled_tool_load, accept_exceeded_tool_load)
        };
        assert_eq!(flags(&[]), (false, false), "the default refuses");
        assert_eq!(flags(&[&unmodeled]), (true, false));
        assert_eq!(flags(&[&exceeded]), (false, true));
        let Commands::Project {
            accept_unmodeled_tool_load,
            accept_exceeded_tool_load,
            ..
        } = Cli::try_parse_from(project_args(&[&unmodeled, &exceeded]))
            .unwrap()
            .command
        else {
            panic!("not the project command");
        };
        assert!(accept_unmodeled_tool_load && accept_exceeded_tool_load);
    }

    /// F057: a cell size that is not a positive number of mm is a clap error
    /// on every `--resolution` flag. Before the fix, `smoke --resolution 0`
    /// reached core, which refused it, and the harness dropped the refusal.
    #[test]
    fn f057_resolution_refuses_a_cell_that_is_not_positive() {
        for bad in ["0", "-0.5", "nan", "inf", "fine"] {
            assert!(
                Cli::try_parse_from(["rs_cam", "smoke", "--resolution", bad]).is_err(),
                "smoke {bad}"
            );
            assert!(
                Cli::try_parse_from(project_args(&["--resolution", bad])).is_err(),
                "project {bad}"
            );
            assert!(
                Cli::try_parse_from(["rs_cam", "rough-score", "p.toml", "--resolution", bad])
                    .is_err(),
                "rough-score {bad}"
            );
        }
        let Commands::Smoke { resolution, .. } =
            Cli::try_parse_from(["rs_cam", "smoke", "--resolution", "0.25"])
                .unwrap()
                .command
        else {
            panic!("not the smoke command");
        };
        assert_eq!(resolution, 0.25);
    }

    #[test]
    fn the_over_budget_sentence_names_both_values_and_the_remedy() {
        let message = super::memory_budget::over_budget_message(2 * GIB, GIB);
        assert!(message.contains("2.00 GiB"), "{message}");
        assert!(message.contains("1.00 GiB"), "{message}");
        assert!(message.contains("--memory-limit"), "{message}");
    }
}
