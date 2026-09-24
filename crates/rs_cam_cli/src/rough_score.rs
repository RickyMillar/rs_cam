#![allow(clippy::print_stdout)] // CLI surface: the JSON records go to stdout

//! `rough-score`: the scoring instrument of the step-ladder roughing plan.
//!
//! `planning/adaptive3d_step_ladder_roughing_2026-09-24/PLAN.md` section 4
//! and Phase 0 item 4. The command loads a project, applies `--set`
//! overrides, walks the generation plan that `project` walks, runs the
//! closing simulation, and prints one JSON object per toolpath and one
//! total object.
//!
//! # One set of numbers on every surface
//!
//! Operator rule 2026-09-24: the GUI, MCP and CLI give IDENTICAL numbers for
//! the same project state, under the SAME names. The command therefore
//! computes no time and no engagement of its own. It serialises the core
//! structs the session publishes:
//!
//! - `diagnostic` is the core `ToolpathDiagnostic` from
//!   `ProjectSession::diagnostics`, the row of MCP `get_project_diagnostics`
//!   `per_toolpath`: `move_count`, `cutting_distance_mm`,
//!   `rapid_distance_mm` and the finding areas.
//! - `cut_summary` is the core `SimulationToolpathCutSummary` from the cut
//!   trace, the row of MCP `get_cut_trace` `toolpath_summaries`:
//!   `total_runtime_s`, `cutting_runtime_s`, `runtime_by_intent`,
//!   `average_engagement`, `total_removed_volume_est_mm3` and the rest. The
//!   MCP row omits some of these fields; the core struct has all of them.
//!
//! The derived fields (`whole_cycle_engagement`, `cutting_duty`) and the
//! move `counts` have no GUI twin. Each one names the published fields it
//! reads.
//!
//! # None is not zero
//!
//! A `null` field is NOT MEASURED. A `0.0` field is a measured zero.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

use rs_cam_core::ToolpathId;
use rs_cam_core::session::generation_plan::Scope;
use rs_cam_core::session::{Command, ProjectSession, SetToolpathParamArgs, ToolpathDiagnostic};
use rs_cam_core::stock::simulation_cut::{
    SimulationCutSummary, SimulationCutTrace, SimulationToolpathCutSummary,
};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

use crate::command::apply_command;
use crate::project::{BlockedEntry, PlanWalkOptions, run_generation_plan};

/// A time below this value is zero for a division.
const TIME_EPS_S: f64 = 1e-9;

/// A Z within this distance of the top Z is at the top Z.
const TOP_Z_EPS_MM: f64 = 1e-3;

/// The options of one `rough-score` run.
pub(crate) struct ScoreOptions {
    /// Score only this toolpath: its id, or its exact name.
    pub toolpath: Option<String>,
    /// The simulation cell size. `None` is refused when the plan simulates.
    pub resolution: Option<f64>,
    /// Skip the closing simulation. The plan still runs the simulations
    /// that a `from_remaining_stock` operation needs before it generates.
    pub no_sim: bool,
    /// `<index>.<param>=<value>` overrides, applied before generation.
    pub set: Vec<String>,
    pub adaptive_feed_modulation: bool,
}

/// The machine of the run.
#[derive(Debug, Serialize)]
pub(crate) struct MachineReport {
    pub name: String,
    /// False when the profile has no `[kinematics]` block. The simulation
    /// then publishes no `runtime_by_intent` and keeps the nominal
    /// `length / feed` time in `total_runtime_s` (N7).
    pub kinematics_declared: bool,
}

/// Move counts on the toolpath IR that the export reads. The GUI publishes
/// no twin of these counts.
#[derive(Debug, Default, Clone, Serialize)]
pub(crate) struct MoveCounts {
    pub rapid_moves: usize,
    /// Moves with an entry intent (plunge, helix, ramp, lead-in).
    pub entry_moves: usize,
    /// Entries. A helix or a ramp is many moves and one entry, so the
    /// command counts runs: a move with an entry intent after a move
    /// without one starts a run.
    pub entry_runs: usize,
    /// `entry_runs` split by the intent of the first move of the run.
    pub entry_runs_by_kind: BTreeMap<String, usize>,
    /// Runs of `MoveIntent::Retract` moves, at feed or rapid. The
    /// integrator puts a rapid retract in `runtime_by_intent.rapid_s`, so
    /// `runtime_by_intent.retract_s` holds only the feed retracts.
    pub retract_runs: usize,
    /// Moves that rise from below `top_z_mm` to `top_z_mm`. The toolpath
    /// does not carry its safe Z, so the command uses the highest Z the
    /// toolpath reaches.
    pub rises_to_top_z: usize,
    /// The highest target Z of the toolpath. `null` for an empty toolpath.
    pub top_z_mm: Option<f64>,
}

/// One scored toolpath.
#[derive(Serialize)]
pub(crate) struct ToolpathScore {
    pub record: &'static str,
    pub index: usize,
    pub id: ToolpathId,
    pub name: String,
    /// The core per-toolpath diagnostic, as MCP `get_project_diagnostics`
    /// publishes it. `null` when the core builds no row for the toolpath.
    pub diagnostic: Option<ToolpathDiagnostic>,
    /// The core cut-trace summary, as the session publishes it. `null` when
    /// the closing simulation did not run, or when the trace has no summary
    /// row for the toolpath (a drill has none).
    pub cut_summary: Option<SimulationToolpathCutSummary>,
    /// `cut_summary.average_engagement * cut_summary.cutting_runtime_s /
    /// cut_summary.total_runtime_s`: the mean engagement over the whole
    /// cycle, rapids and links included. Both times are published fields on
    /// one clock. `null` when the metric does not apply or a time is zero.
    pub whole_cycle_engagement: Option<f64>,
    /// `cut_summary.runtime_by_intent.cutting_s /
    /// cut_summary.runtime_by_intent.total_s`. `null` when the machine
    /// declares no kinematics (no `runtime_by_intent`) or the total is zero.
    pub cutting_duty: Option<f64>,
    pub counts: MoveCounts,
}

/// The total record.
#[derive(Serialize)]
pub(crate) struct TotalScore {
    pub record: &'static str,
    pub project: String,
    /// `project`, or the one toolpath the `--toolpath` filter names and its
    /// ancestors. `project_diagnostics` and `cut_summary` below cover every
    /// toolpath with a result, so under a filter they include the
    /// ancestors.
    pub scope: String,
    pub toolpaths_scored: usize,
    pub machine: MachineReport,
    /// Which feeds the published figures read.
    pub feeds_basis: &'static str,
    /// The cell size of the simulations. `null` when no simulation ran.
    pub resolution_mm: Option<f64>,
    /// G-RESTRES: the cell, its mode, and whether it is the project file's
    /// value or `--resolution`.
    pub simulation_resolution: crate::project::ResolutionUsed,
    pub closing_simulation: bool,
    pub set_overrides: Vec<String>,
    /// The core `ProjectDiagnostics`, as MCP `get_project_diagnostics`
    /// publishes it, without the `per_toolpath` rows (each toolpath record
    /// carries its own). `null` when the closing simulation did not run.
    pub project_diagnostics: Option<serde_json::Value>,
    /// The core project cut-trace summary. `null` when the closing
    /// simulation did not run.
    pub cut_summary: Option<SimulationCutSummary>,
    /// The sum of the per-toolpath counts of the scored toolpaths.
    pub counts: MoveCounts,
    /// Operations that did not generate because an upstream simulated
    /// stock was missing. An EMPTY list means none were blocked.
    pub awaiting_prior_stock: Vec<BlockedEntry>,
}

/// The whole answer of one run.
#[derive(Serialize)]
pub(crate) struct ScoreReport {
    pub toolpaths: Vec<ToolpathScore>,
    pub total: TotalScore,
}

/// Load `input` and score it. The CLI entry point.
pub fn run_rough_score(input: &Path, opts: &ScoreOptions) -> Result<()> {
    let path = input
        .canonicalize()
        .context(format!("Project file not found: {}", input.display()))?;
    let mut session = ProjectSession::load(&path).context("Failed to load project session")?;
    let report = score_session(&mut session, opts)?;
    for tp in &report.toolpaths {
        println!("{}", serde_json::to_string(tp)?);
    }
    println!("{}", serde_json::to_string(&report.total)?);
    Ok(())
}

/// Apply the overrides, generate, simulate and score `session`.
///
/// # Errors
/// - when an override or the toolpath filter names nothing;
/// - when the generation plan refuses the cell size or a simulation fails.
pub(crate) fn score_session(
    session: &mut ProjectSession,
    opts: &ScoreOptions,
) -> Result<ScoreReport> {
    // 1. The overrides, before the plan reads the session. The value goes
    // through the CLI's one coercer and the session's one setter door.
    for spec in &opts.set {
        let (index, param, value) = split_set(spec)?;
        let _ = apply_command(
            session,
            Command::SetToolpathParam(SetToolpathParamArgs {
                index,
                param: param.to_owned(),
                value: crate::job::param_value_from_str(value),
            }),
        )
        .map_err(|e| anyhow::anyhow!("--set {spec}: {e}"))?;
    }

    // 2. The scope. A filter makes the one toolpath and its ancestors
    // current, so a rough is not charged for the finish that follows it.
    let target = match opts.toolpath.as_deref() {
        Some(key) => Some(find_toolpath(session, key)?),
        None => None,
    };
    let scope = target.map_or(Scope::Project, |(_, id)| Scope::Ancestors(id));

    let walk = run_generation_plan(
        session,
        &PlanWalkOptions {
            scope,
            resolution: opts.resolution,
            skip_ids: Vec::new(),
            adaptive_feed_modulation: opts.adaptive_feed_modulation,
            modulation_strategy:
                rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
            modulation_feed_scale: 1.0,
            closing_simulation: !opts.no_sim,
        },
    )?;

    // 3. The toolpaths to score.
    let indices: Vec<usize> = match target {
        Some((index, _)) => vec![index],
        None => (0..session.toolpath_count())
            .filter(|&i| {
                session
                    .toolpath_configs()
                    .get(i)
                    .is_some_and(|tc| tc.enabled)
                    && session.get_result(i).is_some()
            })
            .collect(),
    };
    if let Some((index, _)) = target
        && session.get_result(index).is_none()
    {
        bail!(
            "toolpath {index} has no result after the generation plan. Read the log and \
             `awaiting_prior_stock` for the reason."
        );
    }

    // 4. The published figures. Without the closing simulation the session
    // can hold a prefix simulation that does not include the scored
    // toolpaths, so the command reads no simulation figure then.
    let trace: Option<&SimulationCutTrace> = if opts.no_sim {
        None
    } else {
        session
            .simulation_result()
            .and_then(|sim| sim.cut_trace.as_deref())
    };
    let diagnostics = session.diagnostics();

    let mut toolpaths = Vec::with_capacity(indices.len());
    let mut total_counts = MoveCounts::default();
    for index in indices {
        let (Some(tc), Some(result)) = (
            session.toolpath_configs().get(index),
            session.get_result(index),
        ) else {
            continue;
        };
        let cut_summary = trace.and_then(|t| {
            t.toolpath_summaries
                .iter()
                .find(|s| s.toolpath_id == tc.id)
                .cloned()
        });
        let counts = count_moves(result.toolpath());
        total_counts.merge(&counts);
        toolpaths.push(ToolpathScore {
            record: "toolpath",
            index,
            id: tc.id,
            name: tc.name.clone(),
            diagnostic: diagnostics
                .per_toolpath
                .iter()
                .find(|d| d.toolpath_id == tc.id)
                .cloned(),
            whole_cycle_engagement: cut_summary.as_ref().and_then(whole_cycle_engagement),
            cutting_duty: cut_summary.as_ref().and_then(cutting_duty),
            cut_summary,
            counts,
        });
    }

    let project_diagnostics = if opts.no_sim {
        None
    } else {
        let mut value = serde_json::to_value(&diagnostics)?;
        if let Some(map) = value.as_object_mut() {
            let _ = map.remove("per_toolpath");
        }
        Some(value)
    };
    let feeds_basis = match (opts.no_sim, opts.adaptive_feed_modulation) {
        (true, _) => "planned: no closing simulation, so the feeds are the generated feeds",
        (false, true) => "emitted: the IR the export reads, after the closing modulation",
        (false, false) => "emitted: the IR the export reads, modulation off",
    };
    let scope_label = match target {
        Some((index, id)) => format!("toolpath index {index} (id {id}) and its ancestors"),
        None => "project".to_owned(),
    };
    let total = TotalScore {
        record: "total",
        project: session.name().to_owned(),
        scope: scope_label,
        toolpaths_scored: toolpaths.len(),
        machine: MachineReport {
            name: session.machine().name.clone(),
            kinematics_declared: session.machine().kinematics.is_some(),
        },
        feeds_basis,
        resolution_mm: (walk.simulations > 0).then_some(walk.resolution.mm),
        simulation_resolution: walk.resolution.clone(),
        closing_simulation: !opts.no_sim,
        set_overrides: opts.set.clone(),
        project_diagnostics,
        cut_summary: trace.map(|t| t.summary.clone()),
        counts: total_counts,
        awaiting_prior_stock: walk.blocked,
    };
    Ok(ScoreReport { toolpaths, total })
}

/// `average_engagement * cutting_runtime_s / total_runtime_s`, from the
/// published summary only.
fn whole_cycle_engagement(s: &SimulationToolpathCutSummary) -> Option<f64> {
    if s.metrics_not_applicable || s.cutting_runtime_s <= TIME_EPS_S {
        return None;
    }
    ratio(
        s.average_engagement * s.cutting_runtime_s,
        s.total_runtime_s,
    )
}

/// `runtime_by_intent.cutting_s / runtime_by_intent.total_s`.
fn cutting_duty(s: &SimulationToolpathCutSummary) -> Option<f64> {
    let b = s.runtime_by_intent?;
    ratio(b.cutting_s, b.total_s)
}

/// `a / b`, or `None` when `b` is zero or the answer is not finite.
fn ratio(a: f64, b: f64) -> Option<f64> {
    if b.abs() <= TIME_EPS_S {
        return None;
    }
    let r = a / b;
    r.is_finite().then_some(r)
}

/// Split `<index>.<param>=<value>`.
fn split_set(spec: &str) -> Result<(usize, &str, &str)> {
    let (key, value) = spec
        .split_once('=')
        .with_context(|| format!("--set expects <index>.<param>=<value>, got '{spec}'"))?;
    let (index, param) = key
        .split_once('.')
        .with_context(|| format!("--set expects <index>.<param>=<value>, got '{spec}'"))?;
    let index: usize = index
        .trim()
        .parse()
        .with_context(|| format!("--set '{spec}': '{index}' is not a toolpath index"))?;
    Ok((index, param.trim(), value.trim()))
}

/// Find a toolpath by id, then by exact name. Returns its index and id.
fn find_toolpath(session: &ProjectSession, key: &str) -> Result<(usize, ToolpathId)> {
    if let Ok(raw) = key.trim().parse::<usize>()
        && let Some((index, tc)) = session.find_toolpath_config_by_id(ToolpathId(raw))
    {
        return Ok((index, tc.id));
    }
    if let Some((index, tc)) = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .find(|(_, tc)| tc.name == key)
    {
        return Ok((index, tc.id));
    }
    let known: Vec<String> = session
        .toolpath_configs()
        .iter()
        .map(|tc| format!("{} '{}'", tc.id, tc.name))
        .collect();
    bail!(
        "--toolpath '{key}' names no toolpath id or name. The project holds: {}",
        known.join(", ")
    )
}

fn is_entry(intent: MoveIntent) -> bool {
    matches!(
        intent,
        MoveIntent::EntryPlunge
            | MoveIntent::EntryHelix
            | MoveIntent::EntryRamp
            | MoveIntent::LeadIn
    )
}

fn intent_token(intent: MoveIntent) -> String {
    serde_json::to_value(intent)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{intent:?}"))
}

/// Count the entry runs and the retracts of one toolpath.
fn count_moves(toolpath: &Toolpath) -> MoveCounts {
    let top_z = toolpath
        .moves
        .iter()
        .map(|m| m.target.z)
        .fold(None, |acc: Option<f64>, z| {
            Some(acc.map_or(z, |a| a.max(z)))
        });
    let mut counts = MoveCounts {
        top_z_mm: top_z,
        ..MoveCounts::default()
    };
    let mut prev_entry = false;
    let mut prev_retract = false;
    let mut prev_z: Option<f64> = None;
    for m in &toolpath.moves {
        if m.move_type == MoveType::Rapid {
            counts.rapid_moves += 1;
        }
        // A rapid carries no entry intent: the integrator puts every rapid
        // in `rapid_s`, and the counts follow the same rule.
        let entry = m.move_type != MoveType::Rapid && is_entry(m.intent);
        if entry {
            counts.entry_moves += 1;
            if !prev_entry {
                counts.entry_runs += 1;
                *counts
                    .entry_runs_by_kind
                    .entry(intent_token(m.intent))
                    .or_insert(0) += 1;
            }
        }
        let retract = m.intent == MoveIntent::Retract;
        if retract && !prev_retract {
            counts.retract_runs += 1;
        }
        if let (Some(top), Some(z0)) = (top_z, prev_z)
            && z0 < top - TOP_Z_EPS_MM
            && m.target.z >= top - TOP_Z_EPS_MM
        {
            counts.rises_to_top_z += 1;
        }
        prev_entry = entry;
        prev_retract = retract;
        prev_z = Some(m.target.z);
    }
    counts
}

impl MoveCounts {
    fn merge(&mut self, other: &Self) {
        self.rapid_moves += other.rapid_moves;
        self.entry_moves += other.entry_moves;
        self.entry_runs += other.entry_runs;
        for (kind, n) in &other.entry_runs_by_kind {
            *self.entry_runs_by_kind.entry(kind.clone()).or_insert(0) += n;
        }
        self.retract_runs += other.retract_runs;
        self.rises_to_top_z += other.rises_to_top_z;
        self.top_z_mm = match (self.top_z_mm, other.top_z_mm) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use rs_cam_core::compute::catalog::OperationType;
    use rs_cam_core::session::{
        AddToolpathArgs, SetMachineKinematicsArgs, SimulationOptions, ToolpathConfig,
    };
    use std::sync::atomic::AtomicBool;

    const RESOLUTION_MM: f64 = 1.0;
    const OVERRIDE: &str = "0.depth_per_pass=3";

    /// The 2D pocket fixture with one pocket toolpath added through the
    /// session door, as `run` adds one. The machine gets a kinematics block,
    /// so the simulation publishes `runtime_by_intent`.
    fn pocket_session() -> ProjectSession {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test_data/ux_2d_pocket.toml");
        let mut session = ProjectSession::load(&path).expect("load ux_2d_pocket.toml");
        let _ = apply_command(
            &mut session,
            Command::SetMachineKinematics(SetMachineKinematicsArgs {
                kinematics: Box::new(
                    rs_cam_core::machine::kinematics::MachineKinematics::shapeoko_xxl_stock(),
                ),
            }),
        )
        .expect("set the machine kinematics");
        let tool_id = session.tools()[0].id.0;
        let model_id = session.models()[0].id;
        let _ = apply_command(
            &mut session,
            Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(ToolpathConfig {
                    id: ToolpathId(0),
                    name: "Pocket (rough-score test)".to_owned(),
                    enabled: true,
                    operation: rs_cam_core::compute::catalog::OperationConfig::new_default(
                        OperationType::Pocket,
                    ),
                    dressups: rs_cam_core::compute::config::DressupConfig::default(),
                    heights: rs_cam_core::compute::config::HeightsConfig::default(),
                    tool_id,
                    model_id,
                    pre_gcode: None,
                    post_gcode: None,
                    boundary: rs_cam_core::compute::config::BoundaryConfig::default(),
                    boundary_inherit: true,
                    rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
                    stock_source: rs_cam_core::compute::config::StockSource::default(),
                    coolant: rs_cam_core::gcode::CoolantMode::Off,
                    face_selection: None,
                    debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions::default(),
                    feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
                    planner_origin: None,
                }),
            }),
        )
        .expect("add the pocket toolpath");
        session
    }

    fn score(session: &mut ProjectSession) -> ScoreReport {
        score_session(
            session,
            &ScoreOptions {
                toolpath: None,
                resolution: Some(RESOLUTION_MM),
                no_sim: false,
                set: vec![OVERRIDE.to_owned()],
                adaptive_feed_modulation: true,
            },
        )
        .expect("score the pocket")
    }

    fn same_f64(name: &str, gui: f64, cli: f64) {
        let tol = 1e-9 * gui.abs().max(cli.abs()).max(1.0);
        assert!(
            (gui - cli).abs() <= tol,
            "{name}: the GUI path gives {gui}, rough-score gives {cli}"
        );
    }

    /// The GUI path: set the parameter, generate, simulate with the GUI
    /// options (metrics on, modulation on). Then compare every published
    /// field with the rough-score record of the same state.
    #[test]
    fn rough_score_publishes_the_gui_numbers() {
        let mut gui = pocket_session();
        let (index, param, value) = split_set(OVERRIDE).unwrap();
        let _ = apply_command(
            &mut gui,
            Command::SetToolpathParam(SetToolpathParamArgs {
                index,
                param: param.to_owned(),
                value: crate::job::param_value_from_str(value),
            }),
        )
        .unwrap();
        let cancel = AtomicBool::new(false);
        gui.generate_toolpath(0, &cancel).expect("generate");
        let _ = gui
            .run_simulation(
                &SimulationOptions {
                    resolution: RESOLUTION_MM,
                    skip_ids: Vec::new(),
                    metrics_enabled: true,
                    auto_resolution: false,
                    use_predicted_feed_in_gates: false,
                    adaptive_feed_modulation: true,
                    modulation_strategy:
                        rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
                    modulation_feed_scale: 1.0,
                },
                &cancel,
            )
            .expect("simulate");
        let gui_diag = gui.diagnostics().per_toolpath[0].clone();
        let gui_sum = gui
            .simulation_result()
            .and_then(|s| s.cut_trace.as_deref())
            .expect("a cut trace")
            .toolpath_summaries[0]
            .clone();

        let mut cli_session = pocket_session();
        let report = score(&mut cli_session);
        assert_eq!(report.toolpaths.len(), 1, "one pocket, one record");
        let tp = &report.toolpaths[0];
        let cli_diag = tp.diagnostic.as_ref().expect("a diagnostic row");
        let cli_sum = tp.cut_summary.as_ref().expect("a cut summary");

        // Counts: bitwise.
        assert_eq!(gui_diag.move_count, cli_diag.move_count, "move_count");
        assert_eq!(gui_sum.sample_count, cli_sum.sample_count, "sample_count");
        assert!(gui_diag.move_count > 0, "the pocket must move");

        // Floats: 1e-9 relative.
        same_f64(
            "cutting_distance_mm",
            gui_diag.cutting_distance_mm,
            cli_diag.cutting_distance_mm,
        );
        same_f64(
            "rapid_distance_mm",
            gui_diag.rapid_distance_mm,
            cli_diag.rapid_distance_mm,
        );
        same_f64(
            "total_runtime_s",
            gui_sum.total_runtime_s,
            cli_sum.total_runtime_s,
        );
        same_f64(
            "cutting_runtime_s",
            gui_sum.cutting_runtime_s,
            cli_sum.cutting_runtime_s,
        );
        same_f64(
            "rapid_runtime_s",
            gui_sum.rapid_runtime_s,
            cli_sum.rapid_runtime_s,
        );
        same_f64(
            "average_engagement",
            gui_sum.average_engagement,
            cli_sum.average_engagement,
        );
        same_f64(
            "total_removed_volume_est_mm3",
            gui_sum.total_removed_volume_est_mm3,
            cli_sum.total_removed_volume_est_mm3,
        );
        same_f64(
            "peak_axial_doc_mm",
            gui_sum.peak_axial_doc_mm,
            cli_sum.peak_axial_doc_mm,
        );
        let gui_b = gui_sum.runtime_by_intent.expect("kinematics are declared");
        let cli_b = cli_sum.runtime_by_intent.expect("kinematics are declared");
        for (name, g, c) in [
            ("runtime_by_intent.total_s", gui_b.total_s, cli_b.total_s),
            (
                "runtime_by_intent.cutting_s",
                gui_b.cutting_s,
                cli_b.cutting_s,
            ),
            ("runtime_by_intent.entry_s", gui_b.entry_s, cli_b.entry_s),
            (
                "runtime_by_intent.linking_s",
                gui_b.linking_s,
                cli_b.linking_s,
            ),
            (
                "runtime_by_intent.retract_s",
                gui_b.retract_s,
                cli_b.retract_s,
            ),
            ("runtime_by_intent.rapid_s", gui_b.rapid_s, cli_b.rapid_s),
            (
                "runtime_by_intent.unknown_s",
                gui_b.unknown_s,
                cli_b.unknown_s,
            ),
        ] {
            same_f64(name, g, c);
        }

        // The derived fields read only the published fields.
        let whole = tp
            .whole_cycle_engagement
            .expect("a pocket measures engagement");
        same_f64(
            "whole_cycle_engagement",
            gui_sum.average_engagement * gui_sum.cutting_runtime_s / gui_sum.total_runtime_s,
            whole,
        );
        assert!(whole <= cli_sum.average_engagement + 1e-12);
        assert!(cli_b.total_s >= cli_b.cutting_s);
        assert!(tp.cutting_duty.is_some());

        // The JSON keeps the GUI names.
        let json = serde_json::to_value(tp).unwrap();
        for path in [
            ["diagnostic", "move_count"],
            ["diagnostic", "cutting_distance_mm"],
            ["diagnostic", "rapid_distance_mm"],
            ["cut_summary", "total_runtime_s"],
            ["cut_summary", "cutting_runtime_s"],
            ["cut_summary", "runtime_by_intent"],
            ["cut_summary", "average_engagement"],
            ["cut_summary", "total_removed_volume_est_mm3"],
        ] {
            assert!(
                json.get(path[0]).and_then(|v| v.get(path[1])).is_some(),
                "the JSON has no field {path:?}"
            );
        }
        let total = serde_json::to_value(&report.total).unwrap();
        assert_eq!(
            total
                .pointer("/machine/kinematics_declared")
                .and_then(|v| v.as_bool()),
            Some(true)
        );
        assert!(
            total
                .pointer("/project_diagnostics/total_runtime_s")
                .is_some()
        );
        assert!(total.pointer("/project_diagnostics/per_toolpath").is_none());
    }

    #[test]
    fn a_set_override_is_refused_when_it_names_no_toolpath() {
        let mut session = pocket_session();
        let outcome = score_session(
            &mut session,
            &ScoreOptions {
                toolpath: None,
                resolution: Some(RESOLUTION_MM),
                no_sim: true,
                set: vec!["7.depth_per_pass=3".to_owned()],
                adaptive_feed_modulation: true,
            },
        );
        assert!(outcome.is_err(), "index 7 does not exist");
    }

    #[test]
    fn split_set_reads_index_param_and_value() {
        assert_eq!(
            split_set("1.depth_per_pass=5").unwrap(),
            (1, "depth_per_pass", "5")
        );
        assert!(split_set("depth_per_pass=5").is_err());
        assert!(split_set("1.depth_per_pass").is_err());
    }
}
