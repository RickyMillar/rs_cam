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

use crate::state::AppState;
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

/// Estimated cutting-only cycle time, in seconds, summed over computed enabled
/// toolpaths.
pub fn estimate_total_time(state: &AppState) -> f64 {
    let mut total_secs = 0.0;
    for tc in state.session.toolpath_configs() {
        if tc.enabled
            && let Some(rt) = state.gui.toolpath_rt.get(&tc.id)
            && let Some(result) = &rt.result
        {
            let feed = tc.operation.feed_rate();
            if feed > 0.0 {
                total_secs += (result.stats.cutting_distance / feed) * 60.0;
            }
        }
    }
    total_secs
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
