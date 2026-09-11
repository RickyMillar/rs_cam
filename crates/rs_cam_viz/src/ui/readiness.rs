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
//! ~3 h. [`toolpath_cycle_time`] is the single decision; a surface that wants
//! a different *population* sums it differently, it does not compute time
//! differently. [`CycleTimeBasis`] travels with the number so no surface can
//! print it under a name it hasn't earned.
//!
//! WP9 moved the decision itself, and the [`CycleTime`] / [`CycleTimeBasis`]
//! types it travels on, into `rs_cam_core::session` — the day-to-day cycle
//! time is one number every surface reads the same way, GUI or MCP. This
//! module re-exports both names so the many `readiness::CycleTime` and
//! `readiness::CycleTimeBasis` paths across viz keep resolving, and it adds
//! [`toolpath_cycle_time`] back as a thin wrapper over
//! [`rs_cam_core::session::ProjectSession::query`]. [`CycleTimeBasisExt`]
//! carries the two pieces that cannot move — `remedy()` names a GUI
//! affordance, and `status()` returns [`CheckStatus`], a viz-only type — as
//! an extension trait, because Rust forbids an inherent `impl` on a type
//! from another crate.

use std::sync::Arc;

use crate::state::AppState;
use crate::state::runtime::GuiState;
use crate::state::simulation::HolderCheckScope;
use rs_cam_core::ToolpathId;
use rs_cam_core::session::{ProjectSession, Query, QueryAnswer, ToolpathCycleTimeArgs};
use rs_cam_core::simulation_cut::SimulationCutTrace;
use rs_cam_core::tool_load::ToolLoadReport;

pub use rs_cam_core::session::{CycleTime, CycleTimeBasis};

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

/// `(stale, pending)` over every toolpath, from the one freshness model.
///
/// Shared by the two workspace chips and the Readiness operations row so the
/// three cannot disagree about how many operations are outstanding (F2.2).
///
/// `Error` is in neither bucket: it will not resolve by waiting, and A/M11's
/// rule that a sequencing block is never counted as a failure has its mirror
/// here — a failure is never counted as a block. `Disabled` is excluded
/// because a switched-off operation is not work outstanding.
pub fn freshness_counts(state: &AppState) -> (usize, usize) {
    use crate::state::freshness::FreshnessState;

    let mut stale = 0usize;
    let mut pending = 0usize;
    for index in 0..state.session.toolpath_configs().len() {
        match crate::state::freshness::freshness_at(&state.session, &state.gui, index) {
            Some(FreshnessState::EditedSince) => stale += 1,
            Some(
                FreshnessState::NoResult
                | FreshnessState::Regenerating
                | FreshnessState::WaitingOnUpstream(_),
            ) => pending += 1,
            _ => {}
        }
    }
    (stale, pending)
}

/// Operations check — `(status, current, enabled)`. Warning while any enabled
/// operation is not [`crate::state::freshness::FreshnessState::Current`].
pub fn operations_check(state: &AppState) -> (CheckStatus, usize, usize) {
    let enabled = state
        .session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .count();
    // F1.17, folded into F2.2 as PLAN §11 directs. This counted the GUI
    // store (`gui.toolpath_rt[..].result`), which an operation KEEPS after
    // an edit so the viewport can go on drawing it. The core result — the
    // thing an export actually emits — was already gone. So Readiness could
    // read a confident "2/2 computed" beside an export row that refuses,
    // which is the readiness dashboard contradicting the gate it exists to
    // predict. `is_current()` is true for exactly one state, and it is the
    // state the core cache defines.
    let computed = (0..state.session.toolpath_configs().len())
        .filter(|index| {
            state
                .session
                .get_toolpath_config(*index)
                .is_some_and(|tc| tc.enabled)
                && crate::state::freshness::freshness_at(&state.session, &state.gui, *index)
                    .is_some_and(|f| f.is_current())
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

/// Holder/collet clearance — Fail on any check that found a strike, current
/// or stale; Pass on a current check that found none; Warning when no check
/// has run, or when a check that found nothing has been overtaken by an edit.
///
/// F2.12, G-HOLDERSTALE. This row is the one that says the cutter will not
/// hit the workholding, and it used to answer from
/// [`crate::state::simulation::SimulationChecks`] alone. The check reads the
/// holder assembly, the workholding obstacles and the emitted motion; the
/// operator could change any of them and the row went on reporting the
/// verdict computed against the previous machine setup. Every other
/// readiness row declares itself out of date through `GuiState::edit_counter`
/// — [`crate::state::simulation::SimulationState::collision_check_is_stale`]
/// is that same counter, read the same way.
///
/// `min_safe_stickout` is no longer the freshness proxy. The drain writes it
/// only when the check FOUND collisions, so the old `Pass` arm was
/// unreachable from the shipped lane and a clean, current check reported
/// "Not checked".
/// [`crate::state::simulation::SimulationChecks::checked_at_edit_counter`]
/// answers "was it checked" directly, which is what that proxy stood in for.
///
/// **Staleness withdraws a CLEARANCE claim. It never withdraws a STRIKE.**
/// A stale clear verdict is an abstention — absence of a strike was never
/// proof of clearance, so "it was clear, then you edited" carries no evidence
/// about the state now, and `Warning` is the honest severity. A stale
/// colliding verdict is the opposite: it is positive evidence of a strike,
/// measured slightly earlier, and a feed-rate edit cannot move a holder out
/// of a clamp. De-escalating it to `Warning` would turn red into amber on the
/// one row an operator scans for red, so it stays `Fail` and the detail says
/// the evidence is old. This also keeps the row and the export gate in
/// agreement: `preflight.rs` computes `has_failures` from the raw collision
/// count, which a stale strike still trips.
///
/// **F2.13, G-HOLDERSCOPE — the row must not overstate its SCOPE either.**
/// `AppController::request_collision_check` examines ONE toolpath: the first
/// that has a result, a cutter and a mesh. One toolpath, one tool, one holder.
/// A job whose second operation carries a longer holder is never asked about,
/// and the row read `Clear` for it. The verdict now travels with the
/// population it covers ([`crate::state::simulation::HolderCheckScope`]), and
/// a clear reading that does not cover every enabled operation is
/// [`HolderClearance::PartialClear`] — `Warning`, naming the operation it
/// measured. A measured strike keeps `Fail` whatever the scope: a partial
/// check that found a strike understates the job, it does not overstate it.
pub fn holder_clearance_check(state: &AppState) -> CheckStatus {
    match holder_clearance_state(state) {
        HolderClearance::NotChecked | HolderClearance::StaleClear => CheckStatus::Warning,
        HolderClearance::PartialClear(_) => CheckStatus::Warning,
        HolderClearance::Clear => CheckStatus::Pass,
        HolderClearance::StaleCollisions(_) | HolderClearance::Collisions(_) => CheckStatus::Fail,
    }
}

/// What the holder-clearance evidence amounts to right now.
///
/// One derivation for both surfaces (the Readiness panel and the export
/// pre-flight gate), because the two used to build their own detail strings
/// out of `min_safe_stickout` and could disagree — the duplication this
/// module exists to prevent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HolderClearance {
    /// No collision check has run. Absence of a strike is not clearance.
    NotChecked,
    /// A check found no strike, and the project has been edited since it was
    /// submitted. An abstention: no evidence either way about the state now.
    StaleClear,
    /// A check found this many strikes, and the project has been edited since
    /// it was submitted. **Not** an abstention — the strikes were measured.
    StaleCollisions(usize),
    /// A current check found no strike, and it covered every enabled
    /// operation.
    Clear,
    /// A current check found no strike — in the part of the job it examined.
    /// The rest of the job was never asked about, so this is **not** a
    /// clearance claim for the job (F2.13, G-HOLDERSCOPE).
    PartialClear(HolderCheckScope),
    /// A current check found this many strikes.
    Collisions(usize),
}

/// Derive [`HolderClearance`] from the check record, the edit counter and the
/// population the check covered.
pub fn holder_clearance_state(state: &AppState) -> HolderClearance {
    let sim = &state.simulation;
    if sim.checks.checked_at_edit_counter.is_none() {
        return HolderClearance::NotChecked;
    }
    let stale = sim.collision_check_is_stale(state.gui.edit_counter);
    let scope = sim.checks.checked_scope;
    match (stale, sim.checks.holder_collision_count) {
        (true, 0) => HolderClearance::StaleClear,
        (true, count) => HolderClearance::StaleCollisions(count),
        // A stale verdict is withdrawn whatever its scope, so the two are not
        // crossed: the remedy is one re-check either way, and the scope shows
        // again on the answer.
        (false, 0) if scope.covers_the_job() => HolderClearance::Clear,
        (false, 0) => HolderClearance::PartialClear(scope),
        (false, count) => HolderClearance::Collisions(count),
    }
}

/// The row's detail text, shared by both surfaces.
///
/// A stale strike names BOTH facts, count first, so the severity and the
/// staleness are visible together without a hover. A partial verdict names
/// the operation it measured and how many it did not, for the same reason.
pub fn holder_clearance_detail(state: &AppState) -> String {
    let scope = state.simulation.checks.checked_scope;
    match holder_clearance_state(state) {
        HolderClearance::NotChecked => "Not checked".to_owned(),
        HolderClearance::StaleClear => "Checked, then edited — re-check".to_owned(),
        HolderClearance::StaleCollisions(count) => {
            format!("{count} collision(s) found, then edited — re-check")
        }
        HolderClearance::Clear => "Clear".to_owned(),
        HolderClearance::PartialClear(scope) => {
            let op = examined_operation(state, scope);
            let rest = scope.unexamined();
            format!("Clear in {op} only — {rest} more not checked")
        }
        HolderClearance::Collisions(count) if scope.covers_the_job() => {
            format!("{count} collision(s)")
        }
        HolderClearance::Collisions(count) => {
            let op = examined_operation(state, scope);
            let rest = scope.unexamined();
            format!("{count} collision(s) in {op} — {rest} more not checked")
        }
    }
}

/// Name the operation a partial verdict measured, the way the operation list
/// names it. Falls back to a count when the verdict covers more than one.
fn examined_operation(state: &AppState, scope: HolderCheckScope) -> String {
    let named = scope
        .position
        .and_then(|position| position.checked_sub(1))
        .and_then(|index| state.session.get_toolpath_config(index))
        .map(|tc| tc.name.clone());
    match named {
        Some(name) => name,
        None => format!("{} operation(s)", scope.examined),
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

/// The viz-side half of [`CycleTimeBasis`] — the pieces core cannot own.
///
/// `remedy()` names a GUI affordance, and `status()` returns [`CheckStatus`],
/// a viz-only tier type. WP9 moved [`CycleTimeBasis`] itself into
/// `rs_cam_core::session`, and Rust forbids an inherent `impl` on a type from
/// another crate — this trait is the extension in its place. Any file that
/// calls `.remedy()` or `.status()` on a `CycleTimeBasis` value needs this
/// trait in scope.
pub trait CycleTimeBasisExt {
    /// The concrete next step that upgrades this basis, or `None` when there
    /// is nothing left to do.
    ///
    /// `SimulatedNoAccel` is the **common** case, not the exotic one: every
    /// shipped `MachineProfile` preset carries `kinematics: None`
    /// (`machine.rs:178, 204, 226`), so anyone who has not hand-authored or
    /// `$$`-imported a kinematics block gets an un-accelerated number even
    /// with a fresh simulation. Naming the remedy is worth more than naming
    /// the defect, and the remedy already ships.
    fn remedy(self) -> Option<&'static str>;

    /// Row tier. Information only — no cycle-time row folds into any surface's
    /// headline verdict; the tier just colours the caveat.
    fn status(self) -> CheckStatus;
}

impl CycleTimeBasisExt for CycleTimeBasis {
    fn remedy(self) -> Option<&'static str> {
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

    fn status(self) -> CheckStatus {
        match self {
            CycleTimeBasis::MachineModel => CheckStatus::Pass,
            CycleTimeBasis::SimulatedNoAccel | CycleTimeBasis::CuttingOnly => CheckStatus::Warning,
        }
    }
}

/// **The** cycle-time decision, for one toolpath. Every operator-facing
/// surface routes through here, so the four re-implementations G-TIMEEST found
/// cannot come back: a caller that wants a different population sums this
/// differently, it does not compute time differently.
///
/// WP9 moved the decision itself into `rs_cam_core::session`, behind
/// [`ProjectSession::query`]. This function is the viz-side wrapper every
/// call site keeps calling: it resolves `id` to an index, builds the
/// `Query`, and unwraps the one `QueryAnswer` variant. A refusal — which
/// happens only when `id` names no toolpath in `session`, a case none of the
/// five call sites can hit today — reads as [`CycleTime::NONE`], the same
/// "no estimate" answer an uncovered toolpath already produced pre-WP9.
///
/// `trace` is the current simulation's cut trace, if any. It travels as an
/// `Arc` because that is what the GUI's own simulation state holds
/// (`state.simulation.results.cut_trace`). The GUI simulates off the frame
/// loop rather than through [`ProjectSession::run_simulation`], and since
/// N12 item 10 the drain adopts that answer into the session as well
/// (`Command::AdoptSimulation`). `simulation_result()` therefore holds
/// the same run and shares this `Arc`, from the drain's adopt until a
/// session mutation clears the slot.
pub fn toolpath_cycle_time(
    session: &ProjectSession,
    trace: Option<&Arc<SimulationCutTrace>>,
    id: ToolpathId,
    cutting_distance_mm: f64,
    nominal_feed_mm_min: f64,
) -> CycleTime {
    let Some((index, _)) = session.find_toolpath_config_by_id(id) else {
        return CycleTime::NONE;
    };
    let query = Query::ToolpathCycleTime(ToolpathCycleTimeArgs {
        index,
        trace: trace.map(Arc::clone),
        cutting_distance_mm: Some(cutting_distance_mm),
        nominal_feed_mm_min: Some(nominal_feed_mm_min),
    });
    match session.query(query) {
        Ok(QueryAnswer::ToolpathCycleTime(answer)) => answer.cycle_time,
        // WP13 added a second `Query` row. A different answer to this
        // read is not a measurement, so it reads NOT MEASURED.
        Ok(_) | Err(_) => CycleTime::NONE,
    }
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
    trace: Option<&Arc<SimulationCutTrace>>,
) -> CycleTime {
    let mut total = CycleTime::NONE;
    for tc in session.toolpath_configs() {
        if tc.enabled
            && let Some(rt) = gui.toolpath_rt.get(&tc.id)
            && let Some(result) = &rt.result
        {
            total.fold(toolpath_cycle_time(
                session,
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
            .and_then(|r| r.cut_trace.as_ref()),
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
