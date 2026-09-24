//! GUI project file (format_version=3) diagnostic executor.
//!
//! Loads the project TOML via [`ProjectSession`], executes all enabled
//! toolpaths, runs tri-dexel simulation with cut metrics, checks
//! collisions, and writes structured JSON diagnostics.

use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use tracing::{debug, info, warn};

use rs_cam_core::compute::config::AwaitingPriorStock;
use rs_cam_core::session::generation_plan::{self, Scope};
use rs_cam_core::session::{
    Command, ProjectSession, ReplaceToolpathConfigArgs, SetMachineKinematicsArgs,
    SetPostConfigArgs, SetSimulationResolutionArgs, SimulationOptions, SimulationResolution,
    dependencies,
};
use rs_cam_core::stock::simulation_cut::SimulationCutArtifact;

use crate::command::apply_command;

// ── JSON output types ───────────────────────────────────────────────────

/// The CLI's per-toolpath JSON record — a serde VIEW over
/// [`rs_cam_core::session::ToolpathDiagnostic`], not a second copy of it.
///
/// It cannot simply BE the core struct for two reasons that are not going
/// away: it carries the full debug + semantic traces and the collision-check
/// stickout, which the core diagnostic (a GUI/MCP summary) does not; and its
/// key names (`toolpath_name`, `tool`) differ from the core struct's (`name`,
/// `tool_name`) and are read by existing scripts, so `#[serde(flatten)]`
/// would move the wire.
///
/// What it must NOT do is *diverge on the fields both have*. Wave D3 caught
/// the first instance: the A/M9 standing-material and Wave-D1 dropped-band /
/// tip-float channels were published to the GUI and MCP but silently missing
/// here, so a CLI batch run could not see a finding the same session's GUI
/// would show. D3 fixed those five fields by hand; the struct itself stayed
/// a parallel implementation, which is what
/// `ANTIPATTERNS_BACKLOG.md` P3 logged.
///
/// C3 (2026-08-02) closes it structurally. [`Self::from_core`] EXHAUSTIVELY
/// DESTRUCTURES the core diagnostic, so adding a field there is a compile
/// error here until the CLI decides whether to publish it. That is the
/// mechanism — not vigilance, and not a doc comment asking for it. The wire
/// is pinned byte-for-byte by `tests::the_per_toolpath_json_is_byte_stable`,
/// so the derivation could be rebuilt underneath without the JSON moving.
#[derive(Serialize)]
struct ToolpathDiagnostic<'a> {
    toolpath_id: rs_cam_core::ToolpathId,
    toolpath_name: &'a str,
    operation_type: &'a str,
    /// Stable op-kind tag, mirroring
    /// [`rs_cam_core::session::ToolpathDiagnostic::op_kind`]. `Option` only
    /// because it has always serialised as `null` when the session published
    /// no diagnostic; see [`Self::from_core`].
    op_kind: Option<&'a str>,
    tool: &'a str,
    move_count: usize,
    cutting_distance_mm: f64,
    rapid_distance_mm: f64,
    debug_trace: Option<&'a rs_cam_core::trace::debug_trace::ToolpathDebugTrace>,
    semantic_trace: Option<&'a rs_cam_core::trace::semantic_trace::ToolpathSemanticTrace>,
    /// CLI-LOCAL, deliberately. The core diagnostic's `collision_count` is
    /// whatever holder-collision evidence its caller supplied; the CLI runs
    /// its own per-toolpath [`rs_cam_core::session::ProjectSession::collision_check`]
    /// and reports that. Better data, not a second opinion on the same data.
    collision_count: usize,
    rapid_collision_count: usize,
    /// CLI-local for the same reason: it comes off the collision report the
    /// core diagnostic never sees.
    min_safe_stickout: Option<f64>,
    /// A/M9, renamed by wave 16 (Checkpoint E ruling A6 — the old name said
    /// *standing*, the number means *untouched*). `null` = **not measured**
    /// (this operation runs no ring cascade), never "nothing left uncut".
    truncated_core_mm2: Option<f64>,
    /// B8: hole-aware sibling of [`Self::truncated_core_mm2`] — the same
    /// truncated core with islands netted out. `null` = not measured.
    untouched_material_mm2: Option<f64>,
    /// B8: area the cascade reached and then dropped every point on — the
    /// oracle's *standing*. An ESTIMATOR of a DIFFERENT quantity from the
    /// core above; never sum or compare the two. `null` = not measured.
    reached_uncut_estimate_mm2: Option<f64>,
    /// Wave D1. `null` = nothing dropped, or nothing that plans bands ran.
    unmachined_band_area_mm2: Option<f64>,
    /// Wave D1. `null` = the operation emits no centrelines (not measured);
    /// `0` = measured and clean.
    tip_float_points: Option<usize>,
    /// Wave D1. `null` under exactly the same condition as
    /// [`Self::tip_float_points`].
    max_tip_float_mm: Option<f64>,
    /// C2 follow-up 2: the shallow band's monotone-cell decomposition
    /// telemetry, carried as ONE object (its five counters only mean
    /// anything together). `null` = **not measured** — the operation is not
    /// a `unified_finish`, or its `monotone_cell_decomposition` dial is off,
    /// or it emitted no Shallow region.
    ///
    /// This is the wire the FALLBACK counts were invisible on: a region
    /// whose reconstructed cells did not select its own emitted lattice
    /// emits the pre-C2 undivided raster and increments
    /// `membership_fallbacks`, and nothing in this report said so.
    monotone_cells: Option<rs_cam_core::finish::unified_finish::MonotoneCellTotals>,
    /// G-RESTRES: the stock a rest operation read — the simulation cell,
    /// the snapshot digest, and the toolpaths carved before it. `null` for
    /// an operation that read no simulated stock. The core record, as MCP
    /// `get_diagnostics` publishes it.
    source_stock: Option<&'a rs_cam_core::compute::source_stock::SourceStockWire>,
}

impl<'a> ToolpathDiagnostic<'a> {
    /// Project a core diagnostic onto the CLI wire, adding the three
    /// CLI-only channels.
    ///
    /// The `let ... = core;` destructure below is load-bearing: it has no
    /// `..`, so a new field on
    /// [`rs_cam_core::session::ToolpathDiagnostic`] breaks this build. Bind
    /// it to `_name_unused`-style names if the CLI genuinely should not
    /// publish it — but make that a decision someone wrote down, which is
    /// exactly what D3 found nobody had.
    fn from_core(
        core: &'a rs_cam_core::session::ToolpathDiagnostic,
        debug_trace: Option<&'a rs_cam_core::trace::debug_trace::ToolpathDebugTrace>,
        semantic_trace: Option<&'a rs_cam_core::trace::semantic_trace::ToolpathSemanticTrace>,
        collision_count: usize,
        min_safe_stickout: Option<f64>,
    ) -> Self {
        let rs_cam_core::session::ToolpathDiagnostic {
            toolpath_id,
            name,
            operation_type,
            op_kind,
            tool_name,
            move_count,
            cutting_distance_mm,
            rapid_distance_mm,
            // Superseded by the CLI's own collision check — see the field doc.
            collision_count: _evidence_collision_count,
            rapid_collision_count,
            truncated_core_mm2,
            untouched_material_mm2,
            reached_uncut_estimate_mm2,
            unmachined_band_area_mm2,
            tip_float_points,
            max_tip_float_mm,
            monotone_cells,
            source_stock,
        } = core;

        Self {
            toolpath_id: *toolpath_id,
            toolpath_name: name,
            operation_type,
            op_kind: Some(op_kind),
            tool: tool_name,
            move_count: *move_count,
            cutting_distance_mm: *cutting_distance_mm,
            rapid_distance_mm: *rapid_distance_mm,
            debug_trace,
            semantic_trace,
            collision_count,
            rapid_collision_count: *rapid_collision_count,
            min_safe_stickout,
            truncated_core_mm2: *truncated_core_mm2,
            untouched_material_mm2: *untouched_material_mm2,
            reached_uncut_estimate_mm2: *reached_uncut_estimate_mm2,
            unmachined_band_area_mm2: *unmachined_band_area_mm2,
            tip_float_points: *tip_float_points,
            max_tip_float_mm: *max_tip_float_mm,
            monotone_cells: *monotone_cells,
            source_stock: source_stock.as_ref(),
        }
    }
}

#[derive(Serialize)]
struct ToolpathSummaryEntry {
    id: rs_cam_core::ToolpathId,
    name: String,
    operation: String,
    status: String,
    move_count: usize,
    /// `null` = **the collision check did not run or failed**, never "no
    /// collisions". `Some(0)` is a check that ran and found none.
    ///
    /// Mirrors `truncated_core_mm2` above and the core rule that `None` means
    /// not measured while `Some(0)` means measured clean. Until 2026-09-17
    /// this was a bare `usize` and a failed check shipped `0`.
    collision_count: Option<usize>,
}

/// One operation that never generated because an upstream simulated stock was
/// missing (W5 item f).
///
/// The same six keys the MCP surfaces report: the waiting operation's id,
/// index and name, with [`AwaitingPriorStock`] flattened under them. The CLI
/// held no typed blocked answer at all before this, so a blocked operation
/// appeared NOWHERE in `summary.json` — the core builds a diagnostic only for
/// an operation that has a result, and both per-toolpath loops skip the same
/// set. An absent row cannot say "waiting".
#[derive(Serialize)]
pub(crate) struct BlockedEntry {
    toolpath_id: rs_cam_core::ToolpathId,
    toolpath_index: usize,
    name: String,
    #[serde(flatten)]
    block: AwaitingPriorStock,
}

/// Was this generation failure a WAIT on upstream simulated stock, or a fault?
///
/// The CLI holds no `ComputeStatus`, so it asks the dependency edges: a Stock
/// edge out of this operation whose state is `Pending` is the blocked case.
/// `None` means a genuine failure, which the caller logs and carries.
///
/// The blocker is the nearest enabled source `primary_edges` picks, which is
/// the operation the GUI's own message names.
fn blocked_entry(
    session: &ProjectSession,
    toolpath: rs_cam_core::ToolpathId,
    index: usize,
    error: &rs_cam_core::session::SessionError,
) -> Option<BlockedEntry> {
    let edges = dependencies::primary_edges(session);
    let stock = edges.iter().find(|e| {
        e.from == toolpath
            && e.kind == dependencies::EdgeKind::Stock
            && dependencies::state(e, session) == dependencies::EdgeState::Pending
    })?;
    let blocker = stock
        .on
        .and_then(|id| session.find_toolpath_config_by_id(id));
    let name = session
        .find_toolpath_config_by_id(toolpath)
        .map_or_else(|| format!("toolpath {index}"), |(_, tc)| tc.name.clone());
    let message = match blocker.as_ref() {
        Some((_, tc)) => format!(
            "waiting on the simulated stock '{}' leaves. Generate it, simulate, then generate this operation. The generator refused with: {error}",
            tc.name
        ),
        None => format!(
            "waiting on simulated stock, and no enabled operation above it can leave any. The generator refused with: {error}"
        ),
    };
    Some(BlockedEntry {
        toolpath_id: toolpath,
        toolpath_index: index,
        name,
        block: AwaitingPriorStock {
            blocking_toolpath_id: stock.on,
            blocking_toolpath_index: blocker.map(|(i, _)| i),
            message,
        },
    })
}

/// Where the cell size of a run came from (G-RESTRES).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResolutionSource {
    /// The project file's stored value: `[job.simulation] resolution_mm`,
    /// or auto when the key is absent.
    Project,
    /// `--resolution` replaced the stored value for this run.
    Override,
}

/// The cell size a run simulated at, as `summary.json` states it.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResolutionUsed {
    /// The cell every simulation of the run used, in mm.
    pub mm: f64,
    /// `"auto"` or `"fixed"`: the mode the run used.
    pub mode: &'static str,
    /// Whether the value is the project's own or `--resolution`.
    pub source: ResolutionSource,
    /// The project file's value, in mm, before any override.
    pub project_mm: f64,
}

/// Settle the cell size of a run on the session (G-RESTRES).
///
/// Operator ruling 2026-09-24: the project file stores the ONE simulation
/// resolution, and the GUI, MCP and the CLI all read it, so a rest cascade
/// needs no `--resolution`. An explicit `--resolution` still overrides: it
/// SETS the in-memory session value through `SetSimulationResolution`, the
/// door the GUI panel and MCP take, and the output says `override`.
///
/// # Errors
/// - when `--resolution` is not a positive cell size;
/// - when the plan simulates and the cell is coarser than the rest needs,
///   with the sentence MCP and the GUI give
///   (`ProjectSession::rest_resolution_refusal`).
pub(crate) fn settle_cell_size(
    session: &mut ProjectSession,
    supplied: Option<f64>,
    plans_a_simulation: bool,
) -> Result<ResolutionUsed> {
    let project_mm = session.simulation_resolution_mm();
    let source = match supplied {
        None => ResolutionSource::Project,
        Some(cell) => {
            anyhow::ensure!(
                cell.is_finite() && cell > 0.0,
                "--resolution {cell} is not a positive cell size in mm"
            );
            let _ = session
                .apply(Command::SetSimulationResolution(
                    SetSimulationResolutionArgs {
                        resolution: SimulationResolution::Fixed(cell),
                    },
                ))
                .map_err(|e| anyhow::anyhow!("--resolution {cell}: {e}"))?;
            ResolutionSource::Override
        }
    };
    if plans_a_simulation && let Some(refusal) = session.rest_resolution_refusal() {
        anyhow::bail!("{refusal}");
    }
    Ok(ResolutionUsed {
        mm: session.simulation_resolution_mm(),
        mode: match session.simulation_resolution() {
            SimulationResolution::Auto => "auto",
            SimulationResolution::Fixed(_) => "fixed",
        },
        source,
        project_mm,
    })
}

/// The options of one walk over the generation plan.
pub(crate) struct PlanWalkOptions {
    /// The part of the project the walk makes current.
    pub scope: Scope,
    /// `--resolution`, when the caller passed it. `None` reads the stored
    /// project value (G-RESTRES).
    pub resolution: Option<f64>,
    /// Toolpaths the walk does not generate and the simulation skips.
    pub skip_ids: Vec<rs_cam_core::ToolpathId>,
    pub adaptive_feed_modulation: bool,
    pub modulation_strategy: rs_cam_core::dressup::feed_modulation::ModulationStrategy,
    pub modulation_feed_scale: f64,
    /// Run one simulation over the whole project after the last step. The
    /// plan emits no trailing full step, so every reading of metrics,
    /// collisions and diagnostics needs this simulation.
    pub closing_simulation: bool,
}

/// What one walk over the generation plan did.
pub(crate) struct PlanWalk {
    /// The cell size the walk simulated at, and where it came from.
    pub resolution: ResolutionUsed,
    /// The number of simulations the walk ran, the closing one included.
    pub simulations: usize,
    /// Operations that never generated because an upstream simulated stock
    /// was missing. An EMPTY list means none were blocked.
    pub blocked: Vec<BlockedEntry>,
}

/// Walk `session::generation_plan::plan` for `opts.scope`, and optionally run
/// the closing simulation.
///
/// # Errors
/// - when [`resolve_cell_size`] refuses the cell size;
/// - when a simulation step fails.
pub(crate) fn run_generation_plan(
    session: &mut ProjectSession,
    opts: &PlanWalkOptions,
) -> Result<PlanWalk> {
    let combined_skip = &opts.skip_ids;
    let resolution = opts.resolution;
    let adaptive_feed_modulation = opts.adaptive_feed_modulation;
    let modulation_strategy = opts.modulation_strategy;
    let modulation_feed_scale = opts.modulation_feed_scale;
    // 2c. W5 item (f): the cell size is REFUSED, never guessed, whenever the
    // plan has to simulate. `--resolution` no longer carries a clap default,
    // because clap cannot see the project and the question only has an answer
    // once the edges are known.
    let steps = generation_plan::plan(session, opts.scope);
    let plans_a_simulation = steps
        .iter()
        .any(|step| matches!(step, generation_plan::Step::Simulate { .. }));
    let resolution = settle_cell_size(session, resolution, plans_a_simulation)?;

    // 3. Walk the plan core owns. Setup order, then `toolpath_indices`, with
    // a Simulate step immediately before every operation that starts from
    // remaining stock and has no snapshot. There is no cap and no ladder: the
    // list is finite and no step is retried, so the GUI, the MCP server and
    // this command cannot disagree about what "make the project current"
    // means.
    let cancel = AtomicBool::new(false);
    let sim_opts = SimulationOptions {
        resolution: resolution.mm,
        skip_ids: combined_skip.clone(),
        metrics_enabled: true,
        auto_resolution: false,
        // F-035: predicted-feed plumbing off by default for CLI runs;
        // protects the smoke baseline from spurious verdict drift.
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation,
        modulation_strategy,
        modulation_feed_scale,
    };
    // CMP-23: the S5 prefix memo. The plan's simulations are prefixes of one
    // another — exactly the shape the memo exists for. The cache is local to
    // this run: it holds one snapshot, it is single-slot and in-process, and
    // it dies with the command.
    let mut sim_cache = rs_cam_core::compute::sim_prefix::SimPrefixCache::new();
    let mut simulations = 0usize;
    let mut blocked: Vec<BlockedEntry> = Vec::new();
    for step in &steps {
        match *step {
            generation_plan::Step::Simulate { upto, .. } => {
                // A setup whose earlier operation is already blocked cannot
                // be unlocked by another simulation: the phantom scan latches
                // on the first enabled operation with no result, which is
                // that one. Skip the step instead of paying for it.
                if blocked.iter().any(|b| b.toolpath_id == upto) {
                    continue;
                }
                simulations += 1;
                session.run_simulation_memoized(
                    &sim_opts,
                    &cancel,
                    Some(rs_cam_core::compute::sim_prefix::SimMemo {
                        cache: &mut sim_cache,
                        store: true,
                    }),
                )?;
            }
            generation_plan::Step::Generate { toolpath, index } => {
                if combined_skip.contains(&toolpath) {
                    continue;
                }
                if let Err(error) = session.generate_toolpath(index, &cancel) {
                    match blocked_entry(session, toolpath, index, &error) {
                        Some(entry) => {
                            warn!(
                                toolpath = %entry.name,
                                "operation is waiting on upstream simulated stock"
                            );
                            blocked.push(entry);
                        }
                        None => warn!(index, error = %error, "toolpath generation failed"),
                    }
                }
            }
        }
    }

    // 4. The closing simulation. The plan emits no trailing full step, and
    // every reading below (metrics, collisions, the diagnostics) is taken
    // from one simulation over the whole project.
    if opts.closing_simulation {
        simulations += 1;
        session.run_simulation_memoized(
            &sim_opts,
            &cancel,
            Some(rs_cam_core::compute::sim_prefix::SimMemo {
                cache: &mut sim_cache,
                store: true,
            }),
        )?;
    }
    info!(
        steps = steps.len(),
        simulations,
        blocked = blocked.len(),
        "generation plan finished"
    );

    let sim_memo_stats = sim_cache.stats();
    tracing::info!(
        lookups = sim_memo_stats.lookups,
        hits = sim_memo_stats.hits,
        entries_reused = sim_memo_stats.entries_reused,
        "S5 prefix memo over the generation plan"
    );
    Ok(PlanWalk {
        resolution,
        simulations,
        blocked,
    })
}

#[derive(Serialize)]
struct ProjectSummary {
    project: String,
    setup_count: usize,
    toolpath_count: usize,
    total_cutting_distance_mm: f64,
    total_rapid_distance_mm: f64,
    total_runtime_s: f64,
    /// LH-1: air cut over TOTAL runtime (cutting + rapids) - the measure
    /// every threshold in the codebase uses. The cutting-time reading of the
    /// same seconds ships beside it so neither travels unnamed.
    air_cut_pct_of_total_runtime: f64,
    air_cut_pct_of_cutting_time: f64,
    average_engagement: f64,
    /// Holder/shank collisions summed over the toolpaths whose check RAN.
    /// Read it with `collision_checks_failed` — a zero here means nothing on
    /// its own if some checks did not complete.
    collision_count: usize,
    /// How many per-toolpath collision checks failed to run. `0` means
    /// `collision_count` is a sum over every toolpath.
    collision_checks_failed: usize,
    rapid_collision_count: usize,
    /// CMP-25: the validator's findings for this project.
    ///
    /// The validator reached the GUI, MCP and the core export precondition
    /// and not this command, so the batch CLI's only machine-readable
    /// artifact carried no row of this class at all.
    ///
    /// W5 item (g) renamed the key from `stale_defaults`. `stale` names one
    /// state on this wire: a row whose inputs changed after it generated. A
    /// finding here says the parameter was NEVER CHOSEN, and still sits at a
    /// default that does not suit the tool or the material. Two meanings
    /// under one stem made the word useless.
    ///
    /// An EMPTY list means none of the validator's four rules fired. It does
    /// NOT mean the project carries no such parameter — the rule library is
    /// closed at four, and `compute::validate`'s header says why.
    default_findings: Vec<rs_cam_core::compute::validate::StaleDefault>,
    /// W5 item (f): operations that never generated because an upstream
    /// simulated stock was missing. An EMPTY list means none were blocked.
    awaiting_prior_stock: Vec<BlockedEntry>,
    /// G-RESTRES: the ONE cell every simulation of the run used, its mode,
    /// and whether it is the project file's value or `--resolution`.
    simulation_resolution: ResolutionUsed,
    per_toolpath: Vec<ToolpathSummaryEntry>,
    verdict: String,
    /// Checkpoint K (g2) — the operating point `verdict` and every
    /// per-toolpath gate reading were taken at. Default flipped to
    /// `true` at (g1); `--no-adaptive-feed-modulation` opts out.
    adaptive_feed_modulation: bool,
    /// Human-readable form of the same fact, including the strategy and
    /// feed scale that shaped the rewritten feeds.
    modulation_state: String,
}

// ── Main entry point ────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn run_project_command(
    input: &Path,
    output_dir: &Path,
    setup_filter: Option<&str>,
    skip_ids: &[rs_cam_core::ToolpathId],
    resolution: Option<f64>,
    summary: bool,
    emit_gcode: Option<&Path>,
    adaptive_feed_modulation: bool,
    modulation_strategy: rs_cam_core::dressup::feed_modulation::ModulationStrategy,
    modulation_feed_scale: f64,
    inject_shapeoko_kinematics: bool,
    apply_suggest: bool,
    spindle_strategy_override: Option<rs_cam_core::feeds::SpindleStrategy>,
) -> Result<()> {
    // 1. Load project into a session
    let project_path = input
        .canonicalize()
        .context(format!("Project file not found: {}", input.display()))?;
    let mut session =
        ProjectSession::load(&project_path).context("Failed to load project session")?;

    // F-036c calibration helper: project TOMLs predating F-034 carry
    // no `kinematics` block, so the modulator + kinematic integrator
    // are both no-ops. Inject the Shapeoko XXL preset so the user can
    // compare wall-clock against the integrator's prediction and
    // exercise modulation end-to-end.
    if inject_shapeoko_kinematics {
        let _ = apply_command(
            &mut session,
            Command::SetMachineKinematics(SetMachineKinematicsArgs {
                kinematics: Box::new(
                    rs_cam_core::machine::kinematics::MachineKinematics::shapeoko_xxl_stock(),
                ),
            }),
        )?;
        info!("Injected Shapeoko XXL stock kinematics into MachineProfile");
    }

    if let Some(strategy) = spindle_strategy_override {
        let prev = session.post_config().spindle_strategy;
        let mut post = session.post_config().clone();
        post.spindle_strategy = strategy;
        let _ = apply_command(
            &mut session,
            Command::SetPostConfig(SetPostConfigArgs {
                post: Box::new(post),
            }),
        )?;
        info!(
            previous = ?prev,
            applied = ?strategy,
            "Overrode project spindle policy from CLI flag"
        );
    }

    info!(
        name = %session.name(),
        tools = session.list_tools().len(),
        models_loaded = true,
        setups = session.setup_count(),
        "Loaded project"
    );

    // 2. Map setup_filter to additional skip IDs
    let mut combined_skip: Vec<rs_cam_core::ToolpathId> = skip_ids.to_vec();
    if let Some(filter) = setup_filter {
        for setup in session.list_setups() {
            let matches = setup.name == filter || setup.id.to_string() == filter;
            if !matches {
                // Gather toolpath IDs from non-matching setups
                for &tp_idx in &setup.toolpath_indices {
                    if let Some(tc) = session.get_toolpath_config(tp_idx) {
                        combined_skip.push(tc.id);
                    }
                }
                debug!(setup = %setup.name, "Skipping setup (filter)");
            }
        }
    }

    // 2b. Optionally apply LUT-suggested feeds/speeds to every
    // enabled toolpath before generation. Replaces feed_rate /
    // plunge_rate / stepover / depth_per_pass via
    // `apply_feeds_result_to_op` and writes spindle_rpm from the
    // suggest result. Mutates the in-memory session only; the
    // project TOML on disk stays unchanged.
    if apply_suggest {
        apply_suggested_feeds_to_session(&mut session)?;
    }

    // 2c-4. Walk the generation plan core owns, then run the closing
    // simulation. `run_generation_plan` holds the steps; `rough-score` takes
    // the same function, so the two commands cannot walk the plan in two ways.
    let walk = run_generation_plan(
        &mut session,
        &PlanWalkOptions {
            scope: Scope::Project,
            resolution,
            skip_ids: combined_skip.clone(),
            adaptive_feed_modulation,
            modulation_strategy,
            modulation_feed_scale,
            closing_simulation: true,
        },
    )?;
    let resolution_used = walk.resolution.clone();
    let resolution = resolution_used.mm;
    let blocked = walk.blocked;
    let cancel = AtomicBool::new(false);

    // 5. Run collision checks per toolpath and collect results
    let tp_count = session.toolpath_count();
    let mut collision_reports: std::collections::HashMap<
        rs_cam_core::ToolpathId,
        rs_cam_core::stock::collision::CollisionReport,
    > = std::collections::HashMap::new();

    // A check that FAILED is not a check that found nothing. Before this set
    // existed, the `Err(e)` arm below logged a warning and inserted nothing,
    // both readers did `.unwrap_or(0)`, and the toolpath shipped
    // `collision_count: 0, status: "ok"` — a clean bill of health asserted
    // with no evidence, in the CLI's only machine-readable artifact.
    let mut collision_check_failed: std::collections::HashSet<rs_cam_core::ToolpathId> =
        std::collections::HashSet::new();

    for idx in 0..tp_count {
        if session.get_result(idx).is_none() {
            continue;
        }
        let tp_id = session
            .get_toolpath_config(idx)
            .map(|tc| tc.id)
            // Defensive fallback mirrors the project-file loader: when a
            // config is somehow absent, the position doubles as the id.
            .unwrap_or(rs_cam_core::ToolpathId(idx));
        match session.collision_check(idx, &cancel) {
            Ok(check) => {
                if !check.collision_report.is_clear() {
                    collision_reports.insert(tp_id, check.collision_report);
                }
            }
            Err(rs_cam_core::session::SessionError::MissingGeometry(_)) => {
                // 2D ops don't have meshes for collision checking — that's expected
            }
            Err(e) => {
                warn!(index = idx, error = %e, "Collision check failed");
                collision_check_failed.insert(tp_id);
            }
        }
    }

    // 6. Create output directory
    std::fs::create_dir_all(output_dir).context(format!(
        "Failed to create output dir: {}",
        output_dir.display()
    ))?;

    // 7. Write per-toolpath JSON
    let stock_bbox = session.stock_bbox();
    let diag = session.diagnostics();

    for idx in 0..tp_count {
        let Some(result) = session.get_result(idx) else {
            continue;
        };
        let Some(tc) = session.get_toolpath_config(idx) else {
            continue;
        };

        // The tool name is no longer looked up here: the core diagnostic
        // resolves it from the same `tool_id` through the same session, so
        // the CLI reading it off the core record is one derivation instead of
        // two identical ones (C3).
        let col_report = collision_reports.get(&tc.id);
        let collision_count = col_report.map(|r| r.collisions.len()).unwrap_or(0);
        let min_safe = col_report.map(|r| r.min_safe_stickout);

        // The core per-toolpath diagnostic for this id. EVERY shared field is
        // derived THERE, so the CLI and the GUI/MCP cannot report different
        // numbers for the same run (Wave D3; structural since C3).
        //
        // Both loops are guarded by the same `results` map — the core builds
        // a diagnostic for exactly the toolpaths that have a result, which is
        // the condition this loop already `continue`d on — so a miss is
        // unreachable. It is warned rather than defaulted because a silently
        // half-populated record is the failure mode D3 was cleaning up.
        let Some(core_diag) = diag.per_toolpath.iter().find(|d| d.toolpath_id == tc.id) else {
            warn!(
                toolpath = %tc.name,
                "No core diagnostic for a toolpath that has a result — skipping its JSON record"
            );
            continue;
        };

        let diagnostic = ToolpathDiagnostic::from_core(
            core_diag,
            result.debug_trace.as_ref(),
            result.semantic_trace.as_deref(),
            collision_count,
            min_safe,
        );

        let file_name = format!(
            "tp_{}_{}.json",
            tc.id,
            tc.name.replace(
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

    // 8. Write simulation.json
    if let Some(sim_result) = session.simulation_result()
        && let Some(trace) = &sim_result.cut_trace
    {
        let included_ids: Vec<rs_cam_core::ToolpathId> = (0..tp_count)
            .filter(|idx| session.get_result(*idx).is_some())
            .filter_map(|idx| session.get_toolpath_config(idx).map(|tc| tc.id))
            .collect();

        let sim_artifact = SimulationCutArtifact::new(
            resolution,
            resolution.max(0.25),
            [stock_bbox.min.x, stock_bbox.min.y, stock_bbox.min.z],
            [stock_bbox.max.x, stock_bbox.max.y, stock_bbox.max.z],
            included_ids,
            serde_json::json!({ "project": session.name() }),
            trace.as_ref().clone(),
        );

        let sim_path = output_dir.join("simulation.json");
        let sim_json = serde_json::to_string_pretty(&sim_artifact)
            .context("Failed to serialize simulation")?;
        std::fs::write(&sim_path, sim_json)
            .context(format!("Failed to write {}", sim_path.display()))?;
        info!(path = %sim_path.display(), "Wrote simulation artifact");
    }

    // 9. Write summary.json
    let total_cutting: f64 = diag
        .per_toolpath
        .iter()
        .map(|d| d.cutting_distance_mm)
        .sum();
    let total_rapid: f64 = diag.per_toolpath.iter().map(|d| d.rapid_distance_mm).sum();

    // Gather collision counts including holder checks
    let total_collision_count: usize = collision_reports.values().map(|r| r.collisions.len()).sum();

    let per_toolpath: Vec<ToolpathSummaryEntry> = diag
        .per_toolpath
        .iter()
        .map(|d| {
            // Three states, not two. A failed check is neither clear nor
            // dirty, and it must not read as either.
            let checked = !collision_check_failed.contains(&d.toolpath_id);
            let holder_collisions = checked.then(|| {
                collision_reports
                    .get(&d.toolpath_id)
                    .map(|r| r.collisions.len())
                    .unwrap_or(0)
            });
            let status = match holder_collisions {
                None => "collision_check_failed",
                Some(h) if h + d.rapid_collision_count > 0 => "error",
                Some(_) => "ok",
            };
            ToolpathSummaryEntry {
                id: d.toolpath_id,
                name: d.name.clone(),
                operation: d.operation_type.clone(),
                status: status.to_owned(),
                move_count: d.move_count,
                // Carries the absence through. `total_collisions` no longer
                // exists as a bare sum, because a sum cannot say "not checked".
                collision_count: holder_collisions.map(|h| h + d.rapid_collision_count),
            }
        })
        .collect();

    // The page-one answer, from the SAME `ProjectSession::simulation_triage`
    // the GUI panel, the MCP `get_diagnostics` response and narration read
    // (census §4 acceptance bar). Printed before the verdict line so the
    // reader sees the classes — safety, then actions, then a bounded
    // advisory list that says how much it withheld — rather than a single
    // string plus an unbounded pile of runs.
    print_triage_report(&session.triage());

    // **Checkpoint K (g2), 2026-08-13 — every verdict this command
    // reports names the operating point it was taken at.**
    //
    // A `Within` / `Exceeds` verdict is a statement about a feed, and
    // adaptive feed modulation rewrites feeds move by move before the
    // gates see them — A-5 measured `ConstrainedMax` rewriting 100 % of
    // moves on the DropCutter fixtures and landing the observation
    // exactly on the band maximum. A verdict that does not say whether
    // modulation ran is not reproducible, and until (g1) the CLI and the
    // GUI silently disagreed about it for the same project.
    //
    // Printed on stderr beside the verdict AND carried on summary.json,
    // so a script that only reads the JSON is not the one reader left
    // guessing.
    let modulation_state = if adaptive_feed_modulation {
        format!("on ({modulation_strategy:?}, feed scale {modulation_feed_scale:.2})",)
    } else {
        "off (--no-adaptive-feed-modulation)".to_owned()
    };

    // A failed check outranks a clean count, because the count is only clean
    // for the toolpaths that were actually checked. Ordered above the
    // collision arm on purpose: "some checks did not run" is a statement about
    // the evidence, and it must not be hidden by a finding from the rest of it.
    let verdict = if !collision_check_failed.is_empty() {
        format!(
            "UNKNOWN: {} of {} toolpath collision checks failed — the collision \
             result is incomplete",
            collision_check_failed.len(),
            tp_count
        )
    } else if total_collision_count > 0 {
        format!(
            "ERROR: {} holder/shank collisions detected",
            total_collision_count
        )
    } else if diag.rapid_collision_count > 0 {
        format!(
            "WARNING: {} rapid-through-stock collisions",
            diag.rapid_collision_count
        )
    } else if diag.air_cut_pct_of_total_runtime > 40.0 {
        // LH-1: the 40% band is on the TOTAL-runtime measure; the verdict
        // string says so rather than shipping a bare "air cutting %".
        format!(
            "WARNING: {:.1}% air cutting of total runtime",
            diag.air_cut_pct_of_total_runtime
        )
    } else {
        "OK".to_owned()
    };

    // CMP-25: the default-findings validator, on the one artifact a batch run
    // leaves behind. The adapter already turns these into `Diagnostic`s for
    // the GUI and MCP; the CLI simply had no call.
    let default_findings = rs_cam_core::compute::validate::validate_stale_defaults(&session);
    if !default_findings.is_empty() {
        for finding in &default_findings {
            warn!(
                toolpath = %finding.toolpath_name,
                rule = finding.rule_id.id(),
                "parameter still at a default that does not suit it: {} — {}",
                finding.title,
                finding.detail
            );
        }
    }

    let project_summary = ProjectSummary {
        project: session.name().to_owned(),
        setup_count: session.setup_count().max(1),
        toolpath_count: diag.per_toolpath.len(),
        total_cutting_distance_mm: total_cutting,
        total_rapid_distance_mm: total_rapid,
        total_runtime_s: diag.total_runtime_s,
        air_cut_pct_of_total_runtime: diag.air_cut_pct_of_total_runtime,
        air_cut_pct_of_cutting_time: diag.air_cut_pct_of_cutting_time,
        average_engagement: diag.average_engagement,
        collision_count: total_collision_count,
        // The denominator for `collision_count`. Non-zero means that count is
        // a sum over SOME toolpaths, not all of them.
        collision_checks_failed: collision_check_failed.len(),
        rapid_collision_count: diag.rapid_collision_count,
        default_findings,
        awaiting_prior_stock: blocked,
        simulation_resolution: resolution_used,
        per_toolpath,
        verdict: verdict.clone(),
        adaptive_feed_modulation,
        modulation_state: modulation_state.clone(),
    };

    let summary_path = output_dir.join("summary.json");
    let summary_json =
        serde_json::to_string_pretty(&project_summary).context("Failed to serialize summary")?;
    std::fs::write(&summary_path, summary_json)
        .context(format!("Failed to write {}", summary_path.display()))?;
    info!(path = %summary_path.display(), "Wrote project summary");

    // 10. Optional G-code emit (F-036c calibration helper)
    if let Some(gcode_path) = emit_gcode {
        let trace = session
            .simulation_result()
            .and_then(|s| s.cut_trace.as_deref());
        let policy = rs_cam_core::gcode::ToolLoadExportPolicy {
            accept_unmodeled: true,
            accept_exceeded: true,
        };
        let gcode = rs_cam_core::gcode::export_gcode_checked(&session, trace, policy)
            .context("Emit G-code from session")?;
        std::fs::write(gcode_path, &gcode)
            .context(format!("Failed to write {}", gcode_path.display()))?;
        info!(
            path = %gcode_path.display(),
            bytes = gcode.len(),
            modulated = adaptive_feed_modulation,
            kinematics_injected = inject_shapeoko_kinematics,
            "Wrote G-code"
        );
    }

    // 11. Print human-readable summary
    if summary {
        eprintln!("\n=== Project Diagnostics: {} ===", session.name());
        eprintln!(
            "Toolpaths: {}  |  Cutting: {:.0}mm  |  Rapid: {:.0}mm  |  Time: {:.0}s",
            diag.per_toolpath.len(),
            total_cutting,
            total_rapid,
            diag.total_runtime_s,
        );

        // Print engagement + peak COMMANDED advance/tooth from the sim trace.
        // `peak_chipload_mm_per_tooth` is a per-sample peak of the commanded
        // value, not the gate statistic and not a chip thickness (A-1 census
        // row N7). Named accordingly since 2026-08-08; the number is unchanged.
        if let Some(sim_result) = session.simulation_result()
            && let Some(trace) = &sim_result.cut_trace
        {
            eprintln!(
                "Air cutting: {:.1}% of total runtime  |  Avg engagement: {:.2}  |  \
                 Peak commanded advance/tooth: {:.3} mm/tooth",
                diag.air_cut_pct_of_total_runtime,
                diag.average_engagement,
                trace.summary.peak_chipload_mm_per_tooth,
            );
        }

        // Phase 4 — the kinematic reading, from the session's single
        // producer (the same one that fills the tool-load verdict slot and
        // feeds the triage), so the CLI cannot report a different trapezoid
        // from the GUI for the same project.
        let kinematics = session.kinematic_utilizations(
            session
                .simulation_result()
                .and_then(|sim| sim.cut_trace.as_deref()),
        );
        for entry in &project_summary.per_toolpath {
            let status_icon = if entry.status == "ok" { " " } else { "!" };
            // "not checked" is not "0 collisions". The operator reads this
            // line, so it must not round an absent check down to a clean one.
            let collisions = match entry.collision_count {
                Some(n) => format!("{n} collisions"),
                None => "collisions NOT CHECKED".to_owned(),
            };
            eprintln!(
                "  [{status_icon}] #{} {} ({}) — {} moves, {collisions}",
                entry.id, entry.name, entry.operation, entry.move_count,
            );
            if let Some(util) = kinematics.get(&entry.id)
                && let Some(line) = kinematics_report_line(util)
            {
                eprintln!("      {line}");
            }
        }
        eprintln!("Adaptive feed modulation: {modulation_state}");
        eprintln!("Verdict: {verdict}");
        eprintln!("Output: {}", output_dir.display());
    }

    Ok(())
}

/// Iterate every enabled toolpath in the session, run
/// `feeds::suggest_for_operation`, and replace the operation's
/// feed_rate / plunge_rate / stepover / depth_per_pass / spindle_rpm
/// with the suggested values. Mutates the session in place; does NOT
/// write back to the project file. Prints a before→after table to
/// stderr so the operator can see what shifted.
fn apply_suggested_feeds_to_session(session: &mut ProjectSession) -> Result<()> {
    // T16 — route through the canonical `ProjectSession::cutter_op_profile`
    // (T10) instead of a third hand-rolled `SuggestContext` assembly. The
    // previous CLI copy hardcoded `SpindleStrategy::default()` where the
    // GUI Suggest button and the MCP rationale endpoint read
    // `post_config().spindle_strategy`, so projects with a non-default
    // strategy got different RPM/feed from `--apply-suggest` than from
    // the GUI. That was a bug, not deliberate CLI semantics.
    //
    // Pass 1 (immutable): collect suggestions per enabled toolpath. The
    // profile borrows `session`, so the suggested operations are moved
    // into an owned list before the mutating pass.
    let mut suggestions: Vec<(
        usize,
        rs_cam_core::compute::catalog::OperationConfig,
        f64,
        rs_cam_core::feeds::FeedsProvenance,
    )> = Vec::new();
    for (idx, tc) in session.toolpath_configs().iter().enumerate() {
        if !tc.enabled {
            continue;
        }
        let Some(profile) = session.cutter_op_profile(tc) else {
            warn!(
                toolpath_id = tc.id.0,
                tool_id = tc.tool_id,
                "Tool not found, skipping suggest"
            );
            continue;
        };
        if let Err(e) = &profile.feasibility {
            warn!(
                toolpath_id = tc.id.0,
                tool_id = tc.tool_id,
                error = %e,
                "Suggest refused this toolpath, leaving existing values"
            );
            continue;
        }
        // Feasibility Ok ⟺ both Some (`CutterOpProfile::for_combo`).
        let (Some(operation), Some(feeds)) = (profile.suggested_operation, profile.feeds) else {
            continue;
        };
        // W2.1: per-field provenance of the suggested values, derived from the
        // same FeedsResult the calculator produced.
        let rpm_written = feeds.rpm.is_finite() && feeds.rpm > 0.0;
        let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
        provenance.apply_suggested(&feeds, &operation, rpm_written);
        suggestions.push((idx, operation, feeds.rpm, provenance));
    }

    eprintln!("\n=== Applying LUT-suggested feeds/speeds ===");
    eprintln!(
        "{:<3} {:<32} {:>10} {:>10} {:>10} {:>10} {:>8}",
        "id", "name", "feed", "plunge", "stepover", "dpp", "rpm"
    );

    // Pass 2 (mutable): apply + print the before→after table.
    for (idx, suggested_op, suggested_rpm, provenance) in suggestions {
        // A DRAFT of the whole configuration, applied through
        // `ReplaceToolpathConfig`. That row writes the configuration and
        // drops the chain only when `generation_inputs_signature` moved.
        let Some(mut draft) = session.toolpath_configs().get(idx).cloned() else {
            continue;
        };

        // Capture before values for the table.
        let feed_before = draft.operation.feed_rate();
        let plunge_before = draft.operation.plunge_rate();
        let stepover_before = draft.operation.stepover();
        let dpp_before = draft.operation.depth_per_pass();
        let rpm_before = draft.operation.spindle_rpm();

        // Replace operation with the suggested one (feed/plunge/
        // stepover/dpp already written by apply_feeds_result_to_op).
        draft.operation = suggested_op;
        // Suggest doesn't write spindle_rpm into the operation; the
        // RPM lives in `feeds_result.rpm`. Apply it explicitly so the
        // emitted M3 line matches the calculator's recommendation.
        if suggested_rpm.is_finite() && suggested_rpm > 0.0 {
            draft
                .operation
                .set_spindle_rpm(Some(suggested_rpm.round() as u32));
        }
        draft.feeds_provenance = provenance;

        let feed_after = draft.operation.feed_rate();
        let plunge_after = draft.operation.plunge_rate();
        let stepover_after = draft.operation.stepover();
        let dpp_after = draft.operation.depth_per_pass();
        let rpm_after = draft.operation.spindle_rpm();
        let id = draft.id;
        let name = draft.name.clone();

        let _ = apply_command(
            session,
            Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
                index: idx,
                config: Box::new(draft),
            }),
        )?;

        eprintln!(
            "{:<3} {:<32} {:>10} {:>10} {:>10} {:>10} {:>8}",
            id,
            truncate(&name, 32),
            format!("{:.0}→{:.0}", feed_before, feed_after),
            format!("{:.0}→{:.0}", plunge_before, plunge_after),
            format!(
                "{}→{}",
                fmt_opt(stepover_before, 2),
                fmt_opt(stepover_after, 2)
            ),
            format!("{}→{}", fmt_opt(dpp_before, 2), fmt_opt(dpp_after, 2)),
            format!("{}→{}", fmt_opt_u32(rpm_before), fmt_opt_u32(rpm_after)),
        );
    }
    eprintln!();
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let cut: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

fn fmt_opt(v: Option<f64>, p: usize) -> String {
    match v {
        Some(x) => format!("{:.*}", p, x),
        None => "-".to_owned(),
    }
}

fn fmt_opt_u32(v: Option<u32>) -> String {
    match v {
        Some(x) => x.to_string(),
        None => "-".to_owned(),
    }
}

/// Phase 4 — the two-sided kinematic reading for one toolpath, or `None`
/// when nothing was measurable.
///
/// Both sides carry equal weight, and each half is guarded on its own
/// `Option`: headroom ("feed-bound, so a feed rise lands time") and
/// over-command ("machine-bound, and a plunge-class descent outruns the op's
/// own plunge rate"). A metric that was not measured is OMITTED — this
/// function never prints a zero for an absent reading, because a zero
/// utilization and an unmeasured utilization are opposite facts.
fn kinematics_report_line(
    util: &rs_cam_core::machine::kinematic_utilization::ToolpathKinematicUtilization,
) -> Option<String> {
    let mut bound: Vec<String> = Vec::new();
    if let Some(bindings) = util.bindings.as_ref() {
        let mut feed = format!("{:.0}% feed-bound", bindings.feed_bound * 100.0);
        // The PRECOMPUTED field. This path is one-shot, not a render loop,
        // but the field is also the only headroom reading that survives
        // serialisation — `moves` is `#[serde(skip)]` — so every surface
        // reads the same one.
        if let Some(headroom) = util.headroom_at_1_30 {
            feed.push_str(&format!(" → +{:.0}% at ×1.30", headroom * 100.0));
        }
        bound.push(feed);
        bound.push(format!(
            "{:.0}% machine-bound",
            bindings.machine_bound * 100.0
        ));
    }
    let mut plunge: Option<String> = None;
    if util.plunge_is_measured()
        && let Some(ratio) = util.plunge.peak_ratio
    {
        plunge = Some(format!("plunge peak {ratio:.1}× plunge rate"));
    }
    let mut utilization: Option<f64> = None;
    if util.is_measured() {
        utilization = util.utilization;
    }
    if utilization.is_none() && bound.is_empty() && plunge.is_none() {
        return None;
    }

    let mut line = match utilization {
        Some(u) => format!("kinematics: {:.0}% of commanded", u * 100.0),
        // The utilization itself was not measurable; the binding split or
        // the plunge reading still may be, and saying which one is missing
        // beats printing a zero that reads as a measurement.
        None => "kinematics: utilization not measured".to_owned(),
    };
    let mut detail = bound.join(", ");
    if let Some(plunge) = plunge {
        if !detail.is_empty() {
            detail.push_str("; ");
        }
        detail.push_str(&plunge);
    }
    if !detail.is_empty() {
        line.push_str(&format!(" ({detail})"));
    }
    // Phase 3 — say WHICH feeds were read. The modulator runs after a
    // simulation, so before one this whole line describes the plan, not the
    // motion the post-processor emits (`feedback_measure_emitted_motion`).
    line.push_str(&format!(" ({})", util.feeds_provenance.qualifier()));
    Some(line)
}

/// Render the shared [`SimulationTriage`] contract.
///
/// The census found five surfaces each assembling, ranking and truncating
/// the issue channel their own way. This one renders the shared object and
/// adds nothing of its own — including the truncation notice, which comes
/// from `Bounded` rather than from a local `.take(10)` that forgets to say
/// so (R-6's defect, one layer up).
fn print_triage_report(triage: &rs_cam_core::stock::sim_triage::SimulationTriage) {
    use rs_cam_core::stock::sim_measurability::Measurability;

    // Measurability first: it qualifies everything below it.
    let unmeasured: Vec<_> = triage
        .measurability
        .entries
        .iter()
        .filter(|e| matches!(e.measurability, Measurability::NotMeasurable(_)))
        .collect();
    if !unmeasured.is_empty() {
        eprintln!("Measurability:");
        for e in &unmeasured {
            let reason = e
                .measurability
                .reason()
                .map(|r| r.describe())
                .unwrap_or_default();
            eprintln!(
                "  NOT MEASURED  {} on toolpath {} — {reason}",
                e.metric.label(),
                e.toolpath_id.0
            );
        }
        eprintln!();
    }

    if !triage.safety.is_empty() {
        eprintln!("Safety ({}):", triage.safety.len());
        for f in &triage.safety {
            eprintln!("  {}", f.diagnostic.message);
        }
        eprintln!();
    }

    if !triage.actions.is_empty() {
        eprintln!("Act on ({}):", triage.actions.len());
        for f in &triage.actions {
            eprintln!("  {}", f.diagnostic.message);
        }
        eprintln!();
    }

    if !triage.advisories.items.is_empty() {
        eprintln!("Advisories:");
        for f in &triage.advisories.items {
            let seen = if f.occurrences > 1 {
                format!(" (x{})", f.occurrences)
            } else {
                String::new()
            };
            eprintln!("  {}{seen}", f.diagnostic.message);
        }
        if triage.advisories.truncated {
            eprintln!("  ... {} more not shown", triage.advisories.hidden());
        }
        eprintln!();
    }

    // The class-D tallies, demoted to a footer and each named for the
    // population it counts — the 43x gap the census measured is visible here
    // instead of inferable.
    let c = &triage.counts;
    eprintln!(
        "Counts: {} samples | {} flagged-air + {} flagged-low (per SAMPLE) | \
         {} issue runs (COALESCED, the legacy \"issue_count\") | {} hotspots",
        c.samples_total,
        c.flagged_samples_air,
        c.flagged_samples_low,
        c.issue_segments,
        c.hotspots_total
    );
    eprintln!();
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// A core diagnostic with every field distinguishable, so a mis-wired
    /// field shows up as a wrong VALUE and not just a wrong key.
    fn core_diagnostic() -> rs_cam_core::session::ToolpathDiagnostic {
        rs_cam_core::session::ToolpathDiagnostic {
            toolpath_id: rs_cam_core::ToolpathId(7),
            name: "Finish pass".to_owned(),
            operation_type: "Waterline".to_owned(),
            op_kind: "waterline".to_owned(),
            tool_name: "Ø1 tapered ball".to_owned(),
            move_count: 1234,
            cutting_distance_mm: 5678.25,
            rapid_distance_mm: 90.5,
            collision_count: Some(3),
            rapid_collision_count: 2,
            truncated_core_mm2: Some(12.5),
            untouched_material_mm2: Some(9.75),
            reached_uncut_estimate_mm2: Some(1.5),
            unmachined_band_area_mm2: Some(3.25),
            tip_float_points: Some(4),
            max_tip_float_mm: Some(0.125),
            // Populated, and with five DISTINCT counters, so the nested
            // object's shape and field order are pinned below — not just the
            // fact that a key exists.
            monotone_cells: Some(rs_cam_core::finish::unified_finish::MonotoneCellTotals {
                regions: 11,
                regions_rotated: 5,
                cells_emitted: 23,
                membership_fallbacks: 2,
                empty_fallbacks: 1,
            }),
            // G-RESTRES: a rest record, so its shape is pinned below.
            source_stock: Some(rs_cam_core::compute::source_stock::SourceStockWire {
                cell_mm: 0.25,
                stock_digest: Some("00000000000000ab".to_owned()),
                after: vec![rs_cam_core::compute::source_stock::SourceEntryWire {
                    id: rs_cam_core::ToolpathId(3),
                    output: "00000000000000cd".to_owned(),
                }],
            }),
        }
    }

    /// C3 byte-stability sentry.
    ///
    /// This record's key names are a compatibility surface — the doc comment
    /// on [`ToolpathDiagnostic`] says existing scripts read `toolpath_name`
    /// and `tool`, which is precisely why it could not simply become the core
    /// struct. Pinning the serialized bytes is what let the derivation be
    /// rebuilt underneath without asking the wire to move.
    ///
    /// Captured from the pre-C3 struct and asserted unchanged after it became
    /// a view. If a future field is added, add it here in the same edit — a
    /// key appearing in output but not in this string is a wire change nobody
    /// declared.
    #[test]
    fn the_per_toolpath_json_is_byte_stable() {
        let core = core_diagnostic();
        let record = ToolpathDiagnostic::from_core(&core, None, None, 3, Some(21.5));
        let json = serde_json::to_string_pretty(&record).unwrap();
        let expected = r#"{
  "toolpath_id": 7,
  "toolpath_name": "Finish pass",
  "operation_type": "Waterline",
  "op_kind": "waterline",
  "tool": "Ø1 tapered ball",
  "move_count": 1234,
  "cutting_distance_mm": 5678.25,
  "rapid_distance_mm": 90.5,
  "debug_trace": null,
  "semantic_trace": null,
  "collision_count": 3,
  "rapid_collision_count": 2,
  "min_safe_stickout": 21.5,
  "truncated_core_mm2": 12.5,
  "untouched_material_mm2": 9.75,
  "reached_uncut_estimate_mm2": 1.5,
  "unmachined_band_area_mm2": 3.25,
  "tip_float_points": 4,
  "max_tip_float_mm": 0.125,
  "monotone_cells": {
    "regions": 11,
    "regions_rotated": 5,
    "cells_emitted": 23,
    "membership_fallbacks": 2,
    "empty_fallbacks": 1
  },
  "source_stock": {
    "cell_mm": 0.25,
    "stock_digest": "00000000000000ab",
    "after": [
      {
        "id": 3,
        "output": "00000000000000cd"
      }
    ]
  }
}"#;
        assert_eq!(json, expected, "CLI per-toolpath JSON wire changed");
    }

    /// The `null`-means-not-measured contract survives the view: an operation
    /// that measured nothing must emit `null`, never `0`.
    #[test]
    fn unmeasured_channels_stay_null() {
        let core = rs_cam_core::session::ToolpathDiagnostic {
            truncated_core_mm2: None,
            untouched_material_mm2: None,
            reached_uncut_estimate_mm2: None,
            unmachined_band_area_mm2: None,
            tip_float_points: None,
            max_tip_float_mm: None,
            monotone_cells: None,
            source_stock: None,
            ..core_diagnostic()
        };
        let record = ToolpathDiagnostic::from_core(&core, None, None, 0, None);
        let json = serde_json::to_string(&record).unwrap();
        for key in [
            "truncated_core_mm2",
            "untouched_material_mm2",
            "reached_uncut_estimate_mm2",
            "unmachined_band_area_mm2",
            "tip_float_points",
            "max_tip_float_mm",
            "min_safe_stickout",
            // C2 follow-up 2: `null` = the decomposition did not run. A
            // zeroed object would say it ran and fell back nowhere.
            "monotone_cells",
            // G-RESTRES: `null` = the operation read no simulated stock.
            "source_stock",
        ] {
            assert!(
                json.contains(&format!("\"{key}\":null")),
                "{key} must serialise as null, not 0:\n{json}"
            );
        }
    }
}
