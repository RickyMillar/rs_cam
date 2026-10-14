//! W2.5 (D10) — which feed stage a stored toolpath result holds.
//!
//! Generation stores a DRESSED path: the planned feeds. The adaptive feed
//! modulation pass (`session/compute/simulation.rs`) reads one simulation's
//! cut trace and writes a MODULATED path back into the same result slot.
//! Export emits whichever path the slot holds. Before this tag nothing said
//! which, and nothing said when the simulation the feeds came from was gone.
//!
//! The tag states the fact only. It changes no motion and no feed, and it
//! refuses no export.
//!
//! # The simulation identity
//!
//! `simulation_epoch` moves only on an edit, so two runs of one project
//! state share an epoch, and a run at a different cell size shares it too.
//! The tag therefore names the simulation RUN:
//! [`ProjectSession::simulation_run`] counts the simulations the session
//! has stored. The modulation pass runs on a run that the caller stores
//! next (core: `run_simulation_memoized`; GUI: the live-epoch adopt), so it
//! tags with [`ProjectSession::next_simulation_run`].

use super::ProjectSession;

/// The feed stage of a stored toolpath result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultProvenance {
    /// The generated and dressed path: the planned feeds.
    Dressed,
    /// The adaptive feed modulation pass rewrote the feeds from the cut
    /// trace of simulation run `simulation_run`.
    Modulated {
        /// The [`ProjectSession::simulation_run`] the modulation read.
        simulation_run: u64,
    },
}

/// A [`ResultProvenance`] read against the session's current simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedStage {
    /// The planned feeds. No modulation wrote this result.
    Planned,
    /// Modulated from the simulation the session holds now.
    ModulatedCurrent {
        /// The simulation run the modulation read.
        simulation_run: u64,
    },
    /// Modulated from a simulation the session no longer holds.
    ModulatedSuperseded {
        /// The simulation run the modulation read.
        simulation_run: u64,
        /// The run the session holds now; `None` when it holds none.
        current_run: Option<u64>,
    },
}

impl FeedStage {
    /// The one sentence every surface prints: the G-code header line, the
    /// narration, MCP and the GUI hover. It holds no parenthesis, because
    /// a G-code comment cannot nest one.
    pub fn statement(&self) -> String {
        match *self {
            Self::Planned => "feeds: as planned, not modulated".to_owned(),
            Self::ModulatedCurrent { simulation_run } => {
                format!("feeds: modulated from simulation run {simulation_run}")
            }
            Self::ModulatedSuperseded {
                simulation_run,
                current_run: Some(current),
            } => format!(
                "feeds: modulated from simulation run {simulation_run}; \
                 the current simulation is run {current}"
            ),
            Self::ModulatedSuperseded {
                simulation_run,
                current_run: None,
            } => format!(
                "feeds: modulated from simulation run {simulation_run}; \
                 no simulation is current"
            ),
        }
    }

    /// One short word for a badge: `planned` or `modulated`.
    pub fn badge(&self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::ModulatedCurrent { .. } | Self::ModulatedSuperseded { .. } => "modulated",
        }
    }

    /// The simulation run the modulation read; `None` for planned feeds.
    pub fn simulation_run(&self) -> Option<u64> {
        match *self {
            Self::Planned => None,
            Self::ModulatedCurrent { simulation_run }
            | Self::ModulatedSuperseded { simulation_run, .. } => Some(simulation_run),
        }
    }

    /// True when the feeds came from a simulation the session no longer
    /// holds. An export states it as a caution; it does not refuse.
    pub fn is_superseded(&self) -> bool {
        matches!(self, Self::ModulatedSuperseded { .. })
    }
}

impl ProjectSession {
    /// How many simulations the session has stored, when it holds one now.
    ///
    /// `None` when the session holds no simulation. Every store (the
    /// `run_simulation_memoized` store and [`super::Command::AdoptSimulation`])
    /// takes the next number, so two runs of one project state differ.
    pub fn simulation_run(&self) -> Option<u64> {
        self.simulation.as_ref().map(|_| self.simulation_runs)
    }

    /// The run number the next stored simulation takes.
    pub(crate) fn next_simulation_run(&self) -> u64 {
        self.simulation_runs + 1
    }

    /// Store a simulation. The one door that numbers the run.
    pub(crate) fn store_simulation(&mut self, result: super::SimulationResult) {
        self.simulation_runs += 1;
        self.simulation = Some(result);
    }

    /// Read `provenance` against the simulation the session holds now.
    pub fn feed_stage_of(&self, provenance: ResultProvenance) -> FeedStage {
        match provenance {
            ResultProvenance::Dressed => FeedStage::Planned,
            ResultProvenance::Modulated { simulation_run } => {
                let current_run = self.simulation_run();
                if current_run == Some(simulation_run) {
                    FeedStage::ModulatedCurrent { simulation_run }
                } else {
                    FeedStage::ModulatedSuperseded {
                        simulation_run,
                        current_run,
                    }
                }
            }
        }
    }

    /// The feed stage of toolpath `index`'s stored result; `None` when the
    /// toolpath has no result.
    pub fn feed_stage(&self, index: usize) -> Option<FeedStage> {
        self.results
            .get(&index)
            .map(|result| self.feed_stage_of(result.provenance))
    }
}
