#![allow(clippy::print_stdout)] // CLI surface: the JSON records go to stdout

//! `rough-score`: the scoring instrument of the step-ladder roughing plan.
//!
//! `planning/adaptive3d_step_ladder_roughing_2026-09-24/PLAN.md` section 4
//! and Phase 0 item 4. The command loads a project, applies `--set`
//! overrides, walks the generation plan that `project` walks, runs the
//! closing simulation, and prints one JSON object per toolpath and one
//! total object.
//!
//! # Two time bases, never mixed in one ratio
//!
//! - `accel_time` is the kinematics integrator
//!   (`machine::kinematics::compute_cycle_time_breakdown`) on the toolpath
//!   IR that the G-code export reads. It includes the acceleration.
//! - `sim_nominal` is the simulation cut-trace samples, accumulated again
//!   through the shipped `SummaryAccumulator`. Every time there is the
//!   nominal `length / feed` of one sample. The published trace summary is
//!   not read for time: on a machine with a `kinematics` block the
//!   simulation writes the integrator time into `total_runtime_s`, so the
//!   published fields can hold two time bases.
//!
//! The one cross-base number is `removed_mm3_per_accel_s`. Its numerator is
//! a volume, not a time, so it mixes no time base.
//!
//! # None is not zero
//!
//! A `null` field is NOT MEASURED. A `0.0` field is a measured zero.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

use rs_cam_core::ToolpathId;
use rs_cam_core::machine::kinematics::{CycleTimeBreakdown, compute_cycle_time_breakdown};
use rs_cam_core::session::generation_plan::Scope;
use rs_cam_core::session::{Command, ProjectSession, SetToolpathParamArgs};
use rs_cam_core::stock::simulation_cut::{SimulationCutTrace, SummaryAccumulator};
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

/// The machine the accel time integrates on.
#[derive(Debug, Serialize)]
pub(crate) struct MachineReport {
    pub name: String,
    /// False when the profile has no `[kinematics]` block. The integrator
    /// then uses the generic wood-router default, which is not the named
    /// machine's answer.
    pub kinematics_declared: bool,
    /// The cap on commanded cutting feeds.
    pub max_feed_mm_min: f64,
    /// The rate of `G0` moves.
    pub rapid_feed_mm_min: f64,
}

/// The kinematics integrator buckets for one toolpath, in seconds.
#[derive(Debug, Default, Clone, Copy, Serialize)]
pub(crate) struct AccelTime {
    pub total_s: f64,
    pub cutting_s: f64,
    pub entry_s: f64,
    pub linking_s: f64,
    pub retract_s: f64,
    pub rapid_s: f64,
    /// Untagged feed moves (`MoveIntent::Unknown`). Adaptive 3D emits some
    /// vertical descents untagged, so this bucket is reported, not dropped.
    pub unknown_s: f64,
    /// `cutting_s / total_s`. `null` when `total_s` is zero.
    pub duty_cutting_over_total: Option<f64>,
}

impl AccelTime {
    fn from_breakdown(b: &CycleTimeBreakdown) -> Self {
        Self {
            total_s: b.total_s,
            cutting_s: b.cutting_s,
            entry_s: b.entry_s,
            linking_s: b.linking_s,
            retract_s: b.retract_s,
            rapid_s: b.rapid_s,
            unknown_s: b.unknown_s,
            duty_cutting_over_total: ratio(b.cutting_s, b.total_s),
        }
    }
}

/// The simulation figures for one toolpath, all on the nominal
/// `length / feed` time base of the cut-trace samples.
#[derive(Debug, Serialize)]
pub(crate) struct SimNominal {
    pub total_runtime_s: f64,
    pub cutting_runtime_s: f64,
    /// Time-weighted mean radial engagement over the CUTTING time. The
    /// shipped `average_engagement`. `null` when the toolpath has no
    /// cutting time or its kinematics make the metric not applicable.
    pub average_engagement_over_cutting_s: Option<f64>,
    /// `average_engagement * cutting_runtime_s / total_runtime_s`: the mean
    /// engagement over the WHOLE cycle, rapids and links included.
    pub whole_cycle_engagement_over_total_s: Option<f64>,
    /// Time-weighted mean axial DOC fraction over the cutting samples that
    /// carry one, pooled over every kinematics class.
    pub average_axial_doc_fraction_over_cutting_s: Option<f64>,
    /// Peak axial engagement. Transit-span samples do not count.
    pub peak_axial_doc_mm: f64,
    pub total_removed_volume_est_mm3: f64,
    pub sample_count: usize,
}

/// Move counts on the accel-time IR.
#[derive(Debug, Default, Clone, Serialize)]
pub(crate) struct MoveCounts {
    pub moves: usize,
    pub rapid_moves: usize,
    /// Moves with an entry intent (plunge, helix, ramp, lead-in).
    pub entry_moves: usize,
    /// Entries. A helix or a ramp is many moves and one entry, so the
    /// command counts runs: a move with an entry intent after a move
    /// without one starts a run.
    pub entry_runs: usize,
    /// `entry_runs` split by the intent of the first move of the run.
    pub entry_runs_by_kind: BTreeMap<String, usize>,
    /// Runs of `MoveIntent::Retract` moves.
    pub retract_runs: usize,
    /// Moves that rise from below `top_z_mm` to `top_z_mm`. The toolpath
    /// does not carry its safe Z, so the command uses the highest Z the
    /// toolpath reaches.
    pub rises_to_top_z: usize,
    /// The highest target Z of the toolpath. `null` for an empty toolpath.
    pub top_z_mm: Option<f64>,
}

/// One scored toolpath.
#[derive(Debug, Serialize)]
pub(crate) struct ToolpathScore {
    pub record: &'static str,
    pub index: usize,
    pub id: ToolpathId,
    pub name: String,
    pub operation: String,
    pub accel_time: AccelTime,
    /// `null` when the closing simulation did not run, or when the trace
    /// holds no sample of this toolpath.
    pub sim_nominal: Option<SimNominal>,
    /// `sim_nominal.total_removed_volume_est_mm3 / accel_time.total_s`.
    pub removed_mm3_per_accel_s: Option<f64>,
    pub counts: MoveCounts,
}

/// The total over the scored toolpaths.
#[derive(Serialize)]
pub(crate) struct TotalScore {
    pub record: &'static str,
    pub project: String,
    /// `project`, or the one toolpath the `--toolpath` filter names.
    pub scope: String,
    pub toolpaths_scored: usize,
    pub machine: MachineReport,
    /// Which feeds the accel time reads.
    pub feeds_basis: &'static str,
    /// The cell size of the simulations. `null` when no simulation ran.
    pub resolution_mm: Option<f64>,
    pub closing_simulation: bool,
    pub set_overrides: Vec<String>,
    pub accel_time: AccelTime,
    pub sim_nominal: Option<SimNominal>,
    pub removed_mm3_per_accel_s: Option<f64>,
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

    // 3. The machine. `machine_from_profile` is the `LinkKinematics`
    // mapping. The rapid rate follows the session's own clock (the
    // modulation re-time and `kinematic_utilization_of`), which reads
    // `post.high_feedrate` when `high_feedrate_mode` is on.
    let profile = crate::nc_replay::machine_from_profile(session.machine());
    let post = session.post_config();
    let rapid_feed_mm_min = if post.high_feedrate_mode {
        post.high_feedrate.max(1.0)
    } else {
        profile.rapid_feed_mm_min
    };
    let machine = MachineReport {
        name: profile.label.clone(),
        kinematics_declared: profile.kinematics_declared,
        max_feed_mm_min: profile.max_feed_mm_min,
        rapid_feed_mm_min,
    };

    // 4. The toolpaths to score.
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

    let trace: Option<&SimulationCutTrace> = if opts.no_sim {
        None
    } else {
        session
            .simulation_result()
            .and_then(|sim| sim.cut_trace.as_deref())
    };

    // 5. Score each toolpath, and pool the totals from the same parts.
    let mut toolpaths = Vec::with_capacity(indices.len());
    let mut total_breakdown = CycleTimeBreakdown::default();
    let mut total_acc = SimPool::default();
    let mut total_counts = MoveCounts::default();
    for index in indices {
        let (Some(tc), Some(result)) = (
            session.toolpath_configs().get(index),
            session.get_result(index),
        ) else {
            continue;
        };
        let toolpath = result.toolpath();
        let breakdown = compute_cycle_time_breakdown(
            toolpath,
            &profile.kinematics,
            profile.max_feed_mm_min,
            rapid_feed_mm_min,
        );
        let pool = trace.and_then(|t| SimPool::of_toolpath(t, tc.id));
        let sim_nominal = pool.as_ref().map(SimPool::report);
        let counts = count_moves(toolpath);

        total_breakdown += breakdown;
        if let Some(pool) = pool {
            total_acc.merge(&pool);
        }
        total_counts.merge(&counts);

        toolpaths.push(ToolpathScore {
            record: "toolpath",
            index,
            id: tc.id,
            name: tc.name.clone(),
            operation: tc.operation.op_type().label().to_owned(),
            accel_time: AccelTime::from_breakdown(&breakdown),
            removed_mm3_per_accel_s: sim_nominal
                .as_ref()
                .and_then(|s| ratio(s.total_removed_volume_est_mm3, breakdown.total_s)),
            sim_nominal,
            counts,
        });
    }

    let total_sim = (total_acc.sample_count > 0).then(|| total_acc.report());
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
        machine,
        feeds_basis,
        resolution_mm: (walk.simulations > 0).then_some(walk.resolution),
        closing_simulation: !opts.no_sim,
        set_overrides: opts.set.clone(),
        accel_time: AccelTime::from_breakdown(&total_breakdown),
        removed_mm3_per_accel_s: total_sim
            .as_ref()
            .and_then(|s| ratio(s.total_removed_volume_est_mm3, total_breakdown.total_s)),
        sim_nominal: total_sim,
        counts: total_counts,
        awaiting_prior_stock: walk.blocked,
    };
    Ok(ScoreReport { toolpaths, total })
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

/// Count the moves, the entry runs and the retracts of one toolpath.
fn count_moves(toolpath: &Toolpath) -> MoveCounts {
    let top_z = toolpath
        .moves
        .iter()
        .map(|m| m.target.z)
        .fold(None, |acc: Option<f64>, z| {
            Some(acc.map_or(z, |a| a.max(z)))
        });
    let mut counts = MoveCounts {
        moves: toolpath.moves.len(),
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
        self.moves += other.moves;
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

/// The sums behind [`SimNominal`], on the nominal time base.
///
/// The sums come from the shipped `SummaryAccumulator::observe`, so the
/// engagement, the transit-span peak rule and the removed volume are the
/// shipped definitions. Only the time base is fixed here.
#[derive(Default)]
struct SimPool {
    total_runtime_s: f64,
    cutting_runtime_s: f64,
    engagement_weighted_s: f64,
    /// Cutting time of the toolpaths whose engagement metric applies.
    engagement_cutting_s: f64,
    /// Whole time of the toolpaths whose engagement metric applies.
    engagement_total_s: f64,
    axial_weighted_s: f64,
    axial_observed_s: f64,
    peak_axial_doc_mm: f64,
    removed_mm3: f64,
    sample_count: usize,
}

impl SimPool {
    /// Accumulate the samples of `id`. `None` when the trace holds none.
    fn of_toolpath(trace: &SimulationCutTrace, id: ToolpathId) -> Option<Self> {
        let mut acc = SummaryAccumulator::default();
        for sample in trace.samples.iter().filter(|s| s.toolpath_id == id) {
            acc.observe(sample);
        }
        if acc.sample_count == 0 {
            return None;
        }
        // A drill cycle has no engagement summary row, or a row that says
        // the metric does not apply. Either way the engagement is not
        // measured for it.
        let applicable = trace
            .toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == id)
            .is_some_and(|s| !s.metrics_not_applicable);
        let (axial_weighted_s, axial_observed_s) =
            acc.per_kinematics.iter().fold((0.0, 0.0), |(w, o), k| {
                (
                    w + k.axial_doc_fraction_time_weighted_sum,
                    o + k.axial_doc_observed_runtime_s,
                )
            });
        Some(Self {
            total_runtime_s: acc.total_runtime_s,
            cutting_runtime_s: acc.cutting_runtime_s,
            engagement_weighted_s: if applicable {
                acc.engagement_time_weighted_sum
            } else {
                0.0
            },
            engagement_cutting_s: if applicable {
                acc.cutting_runtime_s
            } else {
                0.0
            },
            engagement_total_s: if applicable { acc.total_runtime_s } else { 0.0 },
            axial_weighted_s,
            axial_observed_s,
            peak_axial_doc_mm: acc.peak_axial_doc_mm,
            removed_mm3: acc.total_removed_volume_est_mm3,
            sample_count: acc.sample_count,
        })
    }

    fn merge(&mut self, other: &Self) {
        self.total_runtime_s += other.total_runtime_s;
        self.cutting_runtime_s += other.cutting_runtime_s;
        self.engagement_weighted_s += other.engagement_weighted_s;
        self.engagement_cutting_s += other.engagement_cutting_s;
        self.engagement_total_s += other.engagement_total_s;
        self.axial_weighted_s += other.axial_weighted_s;
        self.axial_observed_s += other.axial_observed_s;
        self.peak_axial_doc_mm = self.peak_axial_doc_mm.max(other.peak_axial_doc_mm);
        self.removed_mm3 += other.removed_mm3;
        self.sample_count += other.sample_count;
    }

    fn report(&self) -> SimNominal {
        // `average_engagement * cutting / total` is `weighted / total`. The
        // command divides the weighted sum by the total time directly, so
        // the two figures share one numerator and one time base.
        SimNominal {
            total_runtime_s: self.total_runtime_s,
            cutting_runtime_s: self.cutting_runtime_s,
            average_engagement_over_cutting_s: ratio(
                self.engagement_weighted_s,
                self.engagement_cutting_s,
            ),
            whole_cycle_engagement_over_total_s: ratio(
                self.engagement_weighted_s,
                self.engagement_total_s,
            ),
            average_axial_doc_fraction_over_cutting_s: ratio(
                self.axial_weighted_s,
                self.axial_observed_s,
            ),
            peak_axial_doc_mm: self.peak_axial_doc_mm,
            total_removed_volume_est_mm3: self.removed_mm3,
            sample_count: self.sample_count,
        }
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
    use rs_cam_core::session::{AddToolpathArgs, ToolpathConfig};

    /// The 2D pocket fixture with one pocket toolpath added through the
    /// session door, as `run` adds one.
    fn pocket_session() -> ProjectSession {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test_data/ux_2d_pocket.toml");
        let mut session = ProjectSession::load(&path).expect("load ux_2d_pocket.toml");
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

    fn get<'a>(v: &'a serde_json::Value, path: &[&str]) -> &'a serde_json::Value {
        let mut at = v;
        for key in path {
            at = at
                .get(*key)
                .unwrap_or_else(|| panic!("the JSON has no field {path:?} (missing '{key}')"));
        }
        at
    }

    #[test]
    fn the_score_has_every_field_and_its_ratios_hold() {
        let mut session = pocket_session();
        let report = score_session(
            &mut session,
            &ScoreOptions {
                toolpath: None,
                resolution: Some(1.0),
                no_sim: false,
                set: vec!["0.depth_per_pass=3".to_owned()],
                adaptive_feed_modulation: true,
            },
        )
        .expect("score the pocket");
        assert_eq!(report.toolpaths.len(), 1, "one pocket, one record");

        let tp = serde_json::to_value(&report.toolpaths[0]).unwrap();
        let total = serde_json::to_value(&report.total).unwrap();
        for record in [&tp, &total] {
            for key in [
                "total_s",
                "cutting_s",
                "entry_s",
                "linking_s",
                "retract_s",
                "rapid_s",
                "unknown_s",
                "duty_cutting_over_total",
            ] {
                let _ = get(record, &["accel_time", key]);
            }
            for key in [
                "total_runtime_s",
                "cutting_runtime_s",
                "average_engagement_over_cutting_s",
                "whole_cycle_engagement_over_total_s",
                "average_axial_doc_fraction_over_cutting_s",
                "peak_axial_doc_mm",
                "total_removed_volume_est_mm3",
            ] {
                let _ = get(record, &["sim_nominal", key]);
            }
            for key in [
                "entry_runs",
                "retract_runs",
                "rapid_moves",
                "rises_to_top_z",
            ] {
                let _ = get(record, &["counts", key]);
            }
            let _ = get(record, &["removed_mm3_per_accel_s"]);

            let total_s = get(record, &["accel_time", "total_s"]).as_f64().unwrap();
            let cutting_s = get(record, &["accel_time", "cutting_s"]).as_f64().unwrap();
            assert!(total_s > 0.0, "the pocket must take time: {total_s}");
            assert!(total_s >= cutting_s, "{total_s} < {cutting_s}");

            let avg = get(
                record,
                &["sim_nominal", "average_engagement_over_cutting_s"],
            )
            .as_f64()
            .expect("a pocket measures engagement");
            let whole = get(
                record,
                &["sim_nominal", "whole_cycle_engagement_over_total_s"],
            )
            .as_f64()
            .expect("a pocket measures the whole-cycle engagement");
            assert!(
                whole <= avg + 1e-12,
                "the whole-cycle engagement {whole} exceeds the cutting-time one {avg}"
            );
        }
        for key in [
            "name",
            "kinematics_declared",
            "max_feed_mm_min",
            "rapid_feed_mm_min",
        ] {
            let _ = get(&total, &["machine", key]);
        }
        assert_eq!(
            get(&total, &["machine", "kinematics_declared"]).as_bool(),
            Some(false),
            "the fixture machine declares no kinematics"
        );
        let _ = get(&total, &["awaiting_prior_stock"]);
    }

    #[test]
    fn a_set_override_is_refused_when_it_names_no_toolpath() {
        let mut session = pocket_session();
        let outcome = score_session(
            &mut session,
            &ScoreOptions {
                toolpath: None,
                resolution: Some(1.0),
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
