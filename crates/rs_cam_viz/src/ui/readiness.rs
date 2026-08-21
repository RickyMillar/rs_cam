//! Shared readiness-check computation (W3.8).
//!
//! The export pre-flight modal (`preflight.rs`) and the Readiness workspace
//! panel (`readiness_panel.rs`) both answer "is this safe to cut?" from the
//! same scattered producers — computed-op count, simulation freshness, rapid +
//! holder collisions, tool-load verdicts. Before W3.8 the pass/warn/fail
//! thresholds lived only inside the pre-flight modal; the dashboard would have
//! had to re-derive them, the exact duplication the IA audit fought. This
//! module is the single home for those thresholds, so the two surfaces can
//! never disagree on whether a check passes. It owns no rendering — each
//! surface renders the shared verdict its own way.
//!
//! G-TIMEEST (2026-08-22) put cycle time under the same roof for the same
//! reason. Four surfaces each carried their own `cutting_distance / feed`, and
//! on a real job they agreed with each other at 25 min while the machine took
//! ~3 h. [`toolpath_cycle_time`] is now the single decision; a surface that
//! wants a different *population* sums it differently, it does not compute
//! time differently. [`CycleTimeBasis`] travels with the number so no surface
//! can print it under a name it hasn't earned.

use crate::state::AppState;
use crate::state::runtime::GuiState;
use rs_cam_core::ToolpathId;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::simulation_cut::SimulationCutTrace;
use rs_cam_core::tool_load::ToolLoadReport;

/// Pass/warn/fail tier shared by every readiness check and both surfaces.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CheckStatus {
    Pass,
    Warning,
    Fail,
}

impl CheckStatus {
    /// Combine two statuses, keeping the more severe (Fail > Warning > Pass).
    /// Used to fold per-check tiers into a single headline verdict.
    pub fn worse(self, other: Self) -> Self {
        match (self, other) {
            (CheckStatus::Fail, _) | (_, CheckStatus::Fail) => CheckStatus::Fail,
            (CheckStatus::Warning, _) | (_, CheckStatus::Warning) => CheckStatus::Warning,
            _ => CheckStatus::Pass,
        }
    }
}

/// Operations check — `(status, computed, enabled)`. Warning while any enabled
/// op has no result yet.
pub fn operations_check(state: &AppState) -> (CheckStatus, usize, usize) {
    let enabled = state
        .session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .count();
    let computed = state
        .session
        .toolpath_configs()
        .iter()
        .filter(|tc| {
            tc.enabled
                && state
                    .gui
                    .toolpath_rt
                    .get(&tc.id)
                    .and_then(|rt| rt.result.as_ref())
                    .is_some()
        })
        .count();
    let status = if computed < enabled {
        CheckStatus::Warning
    } else {
        CheckStatus::Pass
    };
    (status, computed, enabled)
}

/// Simulation freshness — Pass when fresh results exist, Warning when missing
/// or stale.
pub fn simulation_check(state: &AppState) -> CheckStatus {
    let sim = &state.simulation;
    if sim.has_results() {
        if sim.is_stale(state.gui.edit_counter) {
            CheckStatus::Warning
        } else {
            CheckStatus::Pass
        }
    } else {
        CheckStatus::Warning
    }
}

/// Rapid-traverse collisions — Fail on any, Warning until a sim has run.
pub fn rapid_collision_check(state: &AppState) -> CheckStatus {
    let sim = &state.simulation;
    if !sim.has_results() {
        CheckStatus::Warning
    } else if sim.checks.rapid_collisions.is_empty() {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail
    }
}

/// Holder/collet clearance — Fail on any holder collision, Pass once a safe
/// stickout is known, Warning when not yet checked.
pub fn holder_clearance_check(state: &AppState) -> CheckStatus {
    let sim = &state.simulation;
    if sim.checks.holder_collision_count > 0 {
        CheckStatus::Fail
    } else if sim.checks.min_safe_stickout.is_some() {
        CheckStatus::Pass
    } else {
        CheckStatus::Warning
    }
}

/// Tool-load verdict tier — Fail on any exceeded bound, Warning on unmodeled
/// criteria or an empty report, Pass when every modeled bound holds.
pub fn tool_load_check(report: &ToolLoadReport) -> CheckStatus {
    if report.any_exceeded() {
        CheckStatus::Fail
    } else if report.any_unmodeled() || report.per_toolpath.is_empty() {
        CheckStatus::Warning
    } else {
        CheckStatus::Pass
    }
}

/// Build the project tool-load report against the current simulation trace.
/// (The cut trace lives in viz sim state, not `session.simulation`.)
pub fn load_report(state: &AppState) -> ToolLoadReport {
    let trace = state
        .simulation
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_deref());
    rs_cam_core::gcode::project_load_report(&state.session, trace)
}

/// What a cycle-time number actually measures — G-TIMEEST.
///
/// Four surfaces (readiness panel, pre-flight gate, timeline readout, playback
/// speed baseline) each carried their own `cutting_distance / nominal_feed`
/// and printed it under the word "cycle time". On the 2026-08-22 wanaka run
/// that read **25 min** for a job the simulator measured at **10,781.7 s**.
///
/// The gap is acceleration, and it is arithmetic rather than fudge: the finish
/// pass emits a **0.40 mm** mean segment, and a machine at 500 mm/s² that
/// starts and ends each segment near rest averages `√(a·L)/2` = **7.1 mm/s**
/// over it, against a commanded F3000 = 50 mm/s. `50 ÷ 7.1 = 7.1×` — the
/// observed factor, from the segment length and the accel limit alone. So
/// `distance / feed` is not an approximation of cycle time on a corner-heavy
/// path; it is a different quantity.
///
/// Variants are ordered best-modelled first; [`CycleTimeBasis::worse`] folds a
/// mixed project down to its weakest contributor, the same worst-of rule
/// [`CheckStatus::worse`] uses for check tiers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CycleTimeBasis {
    /// The F-034 trapezoidal integrator
    /// ([`rs_cam_core::machine_kinematics::compute_cycle_time`]): accel/decel
    /// ramps, GRBL junction-deviation cornering, rapids, and the emitted
    /// per-move feed. The simulator publishes it by *overwriting*
    /// `total_runtime_s` in `apply_kinematics_cycle_time`, so the trace field
    /// carries the integrated value rather than the dexel-sample sum it
    /// accumulated first. The only basis that is a wall-clock prediction.
    MachineModel,
    /// The simulator's dexel-sample sum: every move, rapids included, at its
    /// emitted feed. Real distance and real per-move feeds; **no acceleration
    /// model**, because the active machine profile carries no
    /// `MachineKinematics` and `kinematics: None` bypasses the integrator
    /// entirely (every shipped preset ships `None` — see `machine.rs`). Reads
    /// optimistic by a factor that grows as segments shorten.
    SimulatedNoAccel,
    /// No simulation covers this toolpath: cutting distance ÷ the operation's
    /// nominal feed. No rapids, no acceleration, no feed modulation. This is
    /// the pre-G-TIMEEST number, kept so a never-simulated project still shows
    /// something — and now labelled as what it is rather than as a cycle time.
    CuttingOnly,
}

impl CycleTimeBasis {
    /// Keep the weaker of two bases. A project total is only as trustworthy as
    /// its least-modelled contributor, so a single un-simulated op drags the
    /// whole figure down to `CuttingOnly` and says so.
    pub fn worse(self, other: Self) -> Self {
        match (self, other) {
            (CycleTimeBasis::CuttingOnly, _) | (_, CycleTimeBasis::CuttingOnly) => {
                CycleTimeBasis::CuttingOnly
            }
            (CycleTimeBasis::SimulatedNoAccel, _) | (_, CycleTimeBasis::SimulatedNoAccel) => {
                CycleTimeBasis::SimulatedNoAccel
            }
            _ => CycleTimeBasis::MachineModel,
        }
    }

    /// Short parenthetical for a row title — the operator-visible signal that
    /// the number changed meaning.
    pub fn qualifier(self) -> &'static str {
        match self {
            CycleTimeBasis::MachineModel => "wall clock",
            CycleTimeBasis::SimulatedNoAccel => "no accel",
            CycleTimeBasis::CuttingOnly => "cutting only, no accel",
        }
    }

    /// One sentence naming what the figure omits, and which way it errs.
    /// Rendered inline (not as a hover) on every surface — including the
    /// printed setup sheet, where there is no hover to fall back on: an
    /// operator planning a shift must not have to discover the caveat.
    pub fn caveat(self) -> &'static str {
        match self {
            CycleTimeBasis::MachineModel => {
                "Machine-model wall clock: accel/decel ramps, cornering, rapids and emitted feeds."
            }
            CycleTimeBasis::SimulatedNoAccel => {
                "Simulated path ÷ emitted feed, rapids included — but this machine profile has no \
                 acceleration limits, so spool-up and cornering are not modelled. The real cut \
                 will take LONGER."
            }
            CycleTimeBasis::CuttingOnly => {
                "Cutting distance ÷ nominal feed. Excludes rapids, acceleration and per-move feed \
                 changes. On a corner-heavy 3D finish this reads SEVERAL TIMES faster than the \
                 machine — a measured job read 25 min against 3 h."
            }
        }
    }

    /// The concrete next step that upgrades this basis, or `None` when there
    /// is nothing left to do.
    ///
    /// `SimulatedNoAccel` is the **common** case, not the exotic one: every
    /// shipped `MachineProfile` preset carries `kinematics: None`
    /// (`machine.rs:178, 204, 226`), so anyone who has not hand-authored or
    /// `$$`-imported a kinematics block gets an un-accelerated number even
    /// with a fresh simulation. Naming the remedy is worth more than naming
    /// the defect, and the remedy already ships.
    pub fn remedy(self) -> Option<&'static str> {
        match self {
            CycleTimeBasis::MachineModel => None,
            CycleTimeBasis::SimulatedNoAccel => Some(
                "No machine kinematics set — Machine properties ▸ Kinematics ▸ \"Import GRBL $$\" \
                 (paste your controller's settings) turns this into a wall-clock estimate.",
            ),
            CycleTimeBasis::CuttingOnly => {
                Some("Run a simulation to get a modelled estimate for this project.")
            }
        }
    }

    /// Row tier. Information only — no cycle-time row folds into any surface's
    /// headline verdict; the tier just colours the caveat.
    pub fn status(self) -> CheckStatus {
        match self {
            CycleTimeBasis::MachineModel => CheckStatus::Pass,
            CycleTimeBasis::SimulatedNoAccel | CycleTimeBasis::CuttingOnly => CheckStatus::Warning,
        }
    }
}

/// A cycle time and the basis it was measured on, always travelling together.
///
/// `basis: None` means **no estimate exists**, not "zero seconds" — render it
/// as a dash. That distinction is the same one the report-only generation
/// findings draw between `None` and `Some(0.0)`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CycleTime {
    pub seconds: f64,
    pub basis: Option<CycleTimeBasis>,
}

impl CycleTime {
    /// Nothing to estimate.
    pub const NONE: Self = Self {
        seconds: 0.0,
        basis: None,
    };

    /// An estimate of `seconds` on `basis`.
    pub fn of(seconds: f64, basis: CycleTimeBasis) -> Self {
        Self {
            seconds,
            basis: Some(basis),
        }
    }

    /// Accumulate another op's time, degrading the basis to the weaker of the
    /// two. Folding `NONE` is a no-op, so ops with no estimate neither add
    /// time nor claim modelling they don't have.
    pub fn fold(&mut self, other: Self) {
        let Some(other_basis) = other.basis else {
            return;
        };
        self.seconds += other.seconds;
        self.basis = Some(match self.basis {
            Some(mine) => mine.worse(other_basis),
            None => other_basis,
        });
    }
}

/// **The** cycle-time decision, for one toolpath. Every operator-facing
/// surface routes through here, so the four re-implementations G-TIMEEST found
/// cannot come back: a caller that wants a different population sums this
/// differently, it does not compute time differently.
///
/// `trace` is the current simulation's cut trace, if any. Whether the trace's
/// runtime is accel-aware is not a guess: `apply_kinematics_cycle_time` is the
/// only writer of `runtime_by_intent`, and it stamps that field in the same
/// pass that overwrites `total_runtime_s` with the integrator's answer. Its
/// presence therefore *is* the answer to "did the integrator walk this
/// toolpath?".
///
/// Freshness is deliberately not consulted here — every one of the four
/// surfaces already renders its own stale-simulation banner or check row, and
/// a stale wall-clock figure is still a better answer than a fresh
/// cutting-only one.
pub fn toolpath_cycle_time(
    trace: Option<&SimulationCutTrace>,
    id: ToolpathId,
    cutting_distance_mm: f64,
    nominal_feed_mm_min: f64,
) -> CycleTime {
    let summary = trace.and_then(|t| t.toolpath_summaries.iter().find(|s| s.toolpath_id == id));
    if let Some(summary) = summary {
        let basis = if summary.runtime_by_intent.is_some() {
            CycleTimeBasis::MachineModel
        } else {
            CycleTimeBasis::SimulatedNoAccel
        };
        return CycleTime::of(summary.total_runtime_s, basis);
    }
    // No simulated evidence for this toolpath. A zero-length cut still counts
    // as an estimate so the project basis records that this op was never
    // simulated, rather than silently omitting it.
    if nominal_feed_mm_min > 0.0 {
        return CycleTime::of(
            (cutting_distance_mm / nominal_feed_mm_min) * 60.0,
            CycleTimeBasis::CuttingOnly,
        );
    }
    CycleTime::NONE
}

/// Project cycle time over enabled, computed toolpaths — the figure the
/// readiness panel, the pre-flight gate, the export wizard's Save step and the
/// printed setup sheet all publish.
///
/// Takes its inputs loose rather than as an `AppState` so the setup-sheet
/// generator (which is handed `session` + `gui`, never the whole state) shares
/// this exact fold instead of writing its own.
///
/// The timeline is the one surface that does NOT call this: it sums
/// [`toolpath_cycle_time`] over the *simulated* population instead, because
/// its playback baseline divides by that population's move count. Different
/// population, same decision.
pub fn project_cycle_time(
    session: &ProjectSession,
    gui: &GuiState,
    trace: Option<&SimulationCutTrace>,
) -> CycleTime {
    let mut total = CycleTime::NONE;
    for tc in session.toolpath_configs() {
        if tc.enabled
            && let Some(rt) = gui.toolpath_rt.get(&tc.id)
            && let Some(result) = &rt.result
        {
            total.fold(toolpath_cycle_time(
                trace,
                tc.id,
                result.stats.cutting_distance,
                tc.operation.feed_rate(),
            ));
        }
    }
    total
}

/// [`project_cycle_time`] against the live app state.
pub fn estimate_total_time(state: &AppState) -> CycleTime {
    project_cycle_time(
        &state.session,
        &state.gui,
        state
            .simulation
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_deref()),
    )
}

/// `h:mm:ss` once past an hour, `m:ss` below it.
///
/// The pre-G-TIMEEST spellings were `m:ss` (timeline, setup sheet) and
/// `"{mins}m {secs}s"` (setup sheet's private copy), both of which render the
/// three-hour job that motivated this row as ~180 minutes — a number an
/// operator reads as minutes because that is what it says.
///
/// A non-finite input is a dash, matching how an absent estimate renders. The
/// setup sheet's deleted `format_time` returned `"N/A"` for the same case.
pub fn format_cycle_time(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "\u{2014}".to_owned();
    }
    let total = seconds.max(0.0).round() as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Number of tool changes across the enabled toolpath sequence.
pub fn count_tool_changes(state: &AppState) -> usize {
    let mut count = 0;
    let mut last_tool: Option<usize> = None;
    for tc in state.session.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        if let Some(last) = last_tool
            && tc.tool_id != last
        {
            count += 1;
        }
        last_tool = Some(tc.tool_id);
    }
    count
}
