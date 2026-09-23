//! The live state of each analysis that a dock row draws (viewport
//! redesign, phase 3).
//!
//! Four analyses have a life cycle that the registry precondition does not
//! show: the reach map, the rest grid, the collision check and the
//! simulation. This module gives each one three extra answers — computing,
//! stale and failed — and derives each answer each frame from the live
//! field that already owns it. It stores nothing, so it cannot drift from
//! the thing that it describes (`state/CLAUDE.md`, invariants 1 and 2).
//!
//! | Analysis | Computing | Stale | Failed |
//! |---|---|---|---|
//! | Reach | `ReachStatus::Computing` for the selection | — | `ReachStatus::Failed` for the selection |
//! | Rest | `FreshnessState::Regenerating` | `FreshnessState::EditedSince` with a grid | `FreshnessState::Error` |
//! | Collision | `submitted_collision_epoch` is set | `collision_check_is_stale` | — |
//! | Simulation | `SimFreshness::Running` | `AppState::simulation_is_stale` | — |
//!
//! The collision check and the simulation store no error text: the drain
//! clears the submit stamp and shows a toast. So neither one has a Failed
//! answer here.

use crate::state::freshness::{FreshnessState, SimFreshness, freshness_at};
use crate::state::runtime::ReachStatus;
use crate::state::selection::Selection;
use crate::state::toolpath::ToolpathId;
use crate::state::{AppState, Workspace};

use super::registry::{self, OverlayRow};

/// The analysis whose data a row draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Analysis {
    /// The per-tool reach map of the selected toolpath.
    Reach,
    /// The rest grid of the selected toolpath.
    Rest,
    /// The dedicated collision check.
    Collision,
    /// The simulation.
    Simulation,
}

impl Analysis {
    /// The analysis that the row `id` draws, or `None` for a row that draws
    /// geometry or settings only.
    pub fn of(id: &str) -> Option<Self> {
        match id {
            "reach_map" => Some(Self::Reach),
            "rest_heatmap" => Some(Self::Rest),
            "collisions" => Some(Self::Collision),
            "simulated_stock"
            | "stock_colour_solid"
            | "stock_colour_deviation"
            | "stock_colour_by_height"
            | "tool_deflection"
            | "move_colour_advance_per_tooth" => Some(Self::Simulation),
            _ => None,
        }
    }
}

/// The status bar lane that runs the job of an analysis. The status bar lane
/// chips are the jobs surface today (MOCKUPS §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobLane {
    /// `ComputeLane::Toolpath`: a generation.
    Toolpath,
    /// `ComputeLane::Analysis`: a simulation or a collision check.
    Analysis,
    /// `ComputeLane::Reach`: a reach walk.
    Reach,
}

impl JobLane {
    /// The pointer to the lane chip on the status bar.
    pub fn pointer(self) -> &'static str {
        match self {
            Self::Toolpath => "Status bar: Toolpath lane",
            Self::Analysis => "Status bar: Analysis lane",
            Self::Reach => "Status bar: Reach lane",
        }
    }
}

/// The button that recovers a stale or a failed row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovery {
    /// Forget the failed reach key. The scheduler then asks again; the
    /// click starts no compute itself (MOCKUPS §9, G2).
    RetryReach,
    /// Generate one toolpath again.
    Generate(ToolpathId),
    /// Run the collision check again.
    RunCollisionCheck,
}

/// The live answer for one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Live {
    /// The analysis has no job, no stale result and no failure for this
    /// row. The registry precondition decides the row.
    Idle,
    /// A job for this analysis runs now.
    Computing(JobLane),
    /// The analysis holds a result that is older than its inputs.
    Stale {
        note: String,
        recovery: Option<Recovery>,
    },
    /// The last run of this analysis ended in an error.
    Failed { message: String, recovery: Recovery },
}

/// The line that a stale simulation row shows. The row names the Simulation
/// workspace; it does not run the simulation itself, because the Simulation
/// workspace keeps ONE run route (`ui/CLAUDE.md`).
pub const SIMULATION_STALE_NOTE: &str = "from a simulation that is older than the project \u{2014} run it again in the Simulation workspace";

/// The line that a stale collision check shows.
pub const COLLISION_STALE_NOTE: &str = "from a check that is older than the last edit";

/// The selected toolpath, if the selection is a toolpath.
pub fn selected_toolpath(state: &AppState) -> Option<ToolpathId> {
    match state.selection {
        Selection::Toolpath(id) => Some(id),
        _ => None,
    }
}

/// `n` in `#n` for toolpath `id`: its 1-based position in the config list.
pub fn toolpath_number(state: &AppState, id: ToolpathId) -> Option<usize> {
    state
        .session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == id)
        .map(|index| index + 1)
}

/// `#n` for toolpath `id`, or `#?` when the config list does not hold it.
pub fn toolpath_tag(state: &AppState, id: ToolpathId) -> String {
    toolpath_number(state, id).map_or_else(|| "#?".to_owned(), |n| format!("#{n}"))
}

/// The live answer for `row`. It is `Idle` for a row that draws no
/// analysis, and for a row whose own gate (workspace, model, selection)
/// is closed: the registry precondition then says why.
pub fn live(state: &AppState, row: &OverlayRow) -> Live {
    match Analysis::of(row.id) {
        Some(Analysis::Reach) => reach(state),
        Some(Analysis::Rest) => rest(state),
        Some(Analysis::Collision) => collision(state),
        Some(Analysis::Simulation) => simulation(state, row),
        None => Live::Idle,
    }
}

fn reach(state: &AppState) -> Live {
    if state.workspace != Workspace::Toolpaths || !state.viewport.show_model {
        return Live::Idle;
    }
    let Some(id) = selected_toolpath(state) else {
        return Live::Idle;
    };
    let overlay = &state.gui.reach_overlay;
    if overlay.toolpath != Some(id) {
        return Live::Idle;
    }
    match &overlay.status {
        ReachStatus::Computing => Live::Computing(JobLane::Reach),
        ReachStatus::Failed(message) => Live::Failed {
            message: message.clone(),
            recovery: Recovery::RetryReach,
        },
        ReachStatus::Idle | ReachStatus::Ready(_) => Live::Idle,
    }
}

fn rest(state: &AppState) -> Live {
    if state.workspace != Workspace::Toolpaths {
        return Live::Idle;
    }
    let Some(id) = selected_toolpath(state) else {
        return Live::Idle;
    };
    let Some((index, _)) = state.session.find_toolpath_config_by_id(id) else {
        return Live::Idle;
    };
    match freshness_at(&state.session, &state.gui, index) {
        Some(FreshnessState::Regenerating) => Live::Computing(JobLane::Toolpath),
        Some(FreshnessState::Error(message)) => Live::Failed {
            message,
            recovery: Recovery::Generate(id),
        },
        Some(FreshnessState::EditedSince) if registry::rest_grid_info(state).is_some() => {
            Live::Stale {
                note: format!("from the last generation of {}", toolpath_tag(state, id)),
                recovery: Some(Recovery::Generate(id)),
            }
        }
        _ => Live::Idle,
    }
}

fn collision(state: &AppState) -> Live {
    let sim = &state.simulation;
    if sim.submitted_collision_epoch.is_some() {
        return Live::Computing(JobLane::Analysis);
    }
    let checks = &sim.checks;
    if checks.collision_report.is_some()
        && sim.collision_check_is_stale(state.session.simulation_epoch())
    {
        return Live::Stale {
            note: COLLISION_STALE_NOTE.to_owned(),
            recovery: Some(Recovery::RunCollisionCheck),
        };
    }
    // Without a dedicated check, the row draws the rapid collisions of the
    // last simulation. Those follow the simulation's own freshness.
    if checks.collision_report.is_none()
        && !checks.rapid_collisions.is_empty()
        && state.simulation_is_stale()
    {
        return Live::Stale {
            note: SIMULATION_STALE_NOTE.to_owned(),
            recovery: None,
        };
    }
    Live::Idle
}

fn simulation(state: &AppState, row: &OverlayRow) -> Live {
    // The row's own gate. A row that draws in the Simulation workspace only
    // stays Blocked elsewhere, and the moves need a generated toolpath
    // before advance per tooth can mean anything.
    let gate = match row.id {
        "simulated_stock" | "tool_deflection" => state.workspace == Workspace::Simulation,
        "move_colour_advance_per_tooth" => registry::any_generated(state),
        _ => true,
    };
    if !gate {
        return Live::Idle;
    }
    let freshness = state.simulation_freshness();
    if freshness == SimFreshness::Running {
        Live::Computing(JobLane::Analysis)
    } else if freshness.is_stale() {
        Live::Stale {
            note: SIMULATION_STALE_NOTE.to_owned(),
            recovery: None,
        }
    } else {
        Live::Idle
    }
}
