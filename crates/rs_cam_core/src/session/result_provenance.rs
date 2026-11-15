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
//!
//! # When modulated feeds are current (B5)
//!
//! The run number names the run. It does not decide currency: `generate_all`
//! stores one prefix simulation per rest operation, so the latest run moves
//! past the run that modulated an operation while nothing that operation
//! depends on changes. The modulation of one toolpath reads its own moves
//! and the stock that the simulation carved before it. The tag therefore
//! also records that stock as a [`SourceStock`], the record a rest result
//! keeps (`session/rest_stock.rs`). The feeds are current while the record
//! is current ([`ProjectSession::source_is_current`]). A regenerate of the
//! toolpath itself stores a new `Dressed` slot, so the own moves need no
//! record.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::ProjectSession;
use crate::compute::source_stock::SourceStock;
use crate::trace::toolpath_spans::AnnotatedToolpath;

/// **THE feed modulation switch of a project** (operator ruling
/// 2026-11-15: "Can we have it on by default. but shown as a warn. But
/// have option to disable it?"). One stored value per project, default
/// [`Self::On`]; `Command::SetFeedModulation` writes it, and the project
/// file saves it (`[job.simulation] feed_modulation`).
///
/// While it is off no simulation of the session modulates a feed
/// (`ProjectSession::run_simulation` and `Command::AdoptSimulation` both
/// read it), every result holds its dressed feeds, and
/// [`ProjectSession::run_modulation_of`] gives `Off` for every toolpath.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedModulationSwitch {
    /// A simulation sets each move to the highest feed that every limit
    /// allows. That feed can be above the stated feed.
    #[default]
    On,
    /// No simulation changes a feed: the export carries the dressed feeds.
    Off,
}

impl FeedModulationSwitch {
    /// `true` for [`Self::On`].
    #[must_use]
    pub const fn is_on(self) -> bool {
        matches!(self, Self::On)
    }

    /// The switch for a checkbox or a wire flag.
    #[must_use]
    pub const fn from_enabled(enabled: bool) -> Self {
        if enabled { Self::On } else { Self::Off }
    }

    /// `on` or `off`, the word that every surface prints.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

/// The dressed path that a modulated result replaced. The modulated tag
/// keeps it, so `Command::SetFeedModulation` with the switch off puts the
/// planned feeds back with no regeneration.
///
/// Two values are equal when they hold the same path (pointer identity).
#[derive(Clone)]
pub struct DressedPath(Arc<AnnotatedToolpath>);

impl DressedPath {
    /// Keep `path` as the dressed path of a result.
    pub(crate) const fn new(path: Arc<AnnotatedToolpath>) -> Self {
        Self(path)
    }

    /// The dressed path.
    #[must_use]
    pub fn path(&self) -> &Arc<AnnotatedToolpath> {
        &self.0
    }
}

impl PartialEq for DressedPath {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for DressedPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DressedPath({} moves)", self.0.toolpath.moves.len())
    }
}

/// The feed stage of a stored toolpath result.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultProvenance {
    /// The generated and dressed path: the planned feeds.
    Dressed,
    /// The adaptive feed modulation pass rewrote the feeds from the cut
    /// trace of simulation run `simulation_run`.
    Modulated {
        /// The [`ProjectSession::simulation_run`] the modulation read.
        simulation_run: u64,
        /// The stock the simulation carved before this toolpath. The
        /// feeds are current while this record is current.
        source: SourceStock,
        /// P30-2: the strategy the run modulated with. The limit rows read
        /// it (`ProjectSession::run_modulation_of`): the declared list of
        /// capped limits depends on it.
        strategy: crate::dressup::feed_modulation::ModulationStrategy,
        /// P30-2: the feed scale the run modulated with (a scale above 1
        /// empties the declared list).
        feed_scale: f64,
        /// The dressed path that the first modulation of this result
        /// replaced (operator ruling 2026-11-15). A later modulation of
        /// the same result keeps it.
        dressed: DressedPath,
    },
}

/// A [`ResultProvenance`] read against the session's current simulation.
/// P30-1: MCP serialises it inside `EmittedFeeds` (`{"kind": "planned"}`,
/// `{"kind": "modulated_current", "simulation_run": n}`, ..).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FeedStage {
    /// The planned feeds. No modulation wrote this result.
    Planned,
    /// Modulated, and nothing the modulation read has changed since.
    ModulatedCurrent {
        /// The simulation run the modulation read.
        simulation_run: u64,
    },
    /// Modulated, and the stock the simulation carved before the toolpath
    /// has changed since.
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

    /// True when the stock the modulation read has changed since. An export
    /// states it as a caution; it does not refuse.
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

    /// Read the `provenance` of toolpath `index`'s result against the
    /// project as it stands.
    ///
    /// Modulated feeds are current while the stock record of the tag is
    /// current (module doc, B5). The run counter does not decide it.
    pub fn feed_stage_of(&self, index: usize, provenance: &ResultProvenance) -> FeedStage {
        match provenance {
            ResultProvenance::Dressed => FeedStage::Planned,
            ResultProvenance::Modulated {
                simulation_run,
                source,
                strategy: _,
                feed_scale: _,
                dressed: _,
            } => {
                let simulation_run = *simulation_run;
                let current = self
                    .toolpath_configs
                    .get(index)
                    .is_some_and(|tc| self.source_is_current(tc.id, source).is_ok());
                if current {
                    FeedStage::ModulatedCurrent { simulation_run }
                } else {
                    FeedStage::ModulatedSuperseded {
                        simulation_run,
                        current_run: self.simulation_run(),
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
            .map(|result| self.feed_stage_of(index, &result.provenance))
    }

    /// **P30-2 (lead ruling 2026-11-14): THE constructor of the feed
    /// modulation of the toolpath at `index`** (`RunModulation`), from its
    /// stored result. Every session door of the limit rows reads it
    /// (`limit_rows`, `safe_envelope`, `cutter_op_profile`), and the GUI
    /// feeds memo reads it through the panel inputs, so GUI, MCP and the CLI
    /// give the same rows for the same session state.
    ///
    /// - The feed modulation switch of the project is off (operator ruling
    ///   2026-11-15): `Off` for every toolpath. No simulation modulates.
    /// - A disabled toolpath (the run does not simulate it), or no toolpath
    ///   at `index`: `Off`.
    /// - A current modulated result: `Current` with the strategy and scale
    ///   that the run recorded.
    /// - Any other state: `NotRun`; the export carries the stated feed, or
    ///   the feeds of an older simulation for a superseded result.
    #[must_use]
    pub fn run_modulation_of(
        &self,
        index: usize,
    ) -> crate::dressup::feed_modulation::RunModulation {
        use crate::dressup::feed_modulation::RunModulation;
        use crate::limits::ExportFeed;
        let Some(tc) = self.toolpath_configs.get(index) else {
            return RunModulation::Off;
        };
        if !tc.enabled || !self.feed_modulation.is_on() {
            return RunModulation::Off;
        }
        let Some(result) = self.results.get(&index) else {
            return RunModulation::NotRun {
                export: ExportFeed::Stated,
            };
        };
        match (
            &result.provenance,
            self.feed_stage_of(index, &result.provenance),
        ) {
            (
                ResultProvenance::Modulated {
                    strategy,
                    feed_scale,
                    ..
                },
                FeedStage::ModulatedCurrent { .. },
            ) => RunModulation::Current {
                strategy: *strategy,
                feed_scale: *feed_scale,
                held: crate::dressup::feed_modulation::HeldByResult::read_from_result(),
            },
            (_, FeedStage::ModulatedSuperseded { .. }) => RunModulation::NotRun {
                export: ExportFeed::OlderModulation,
            },
            (_, FeedStage::Planned | FeedStage::ModulatedCurrent { .. }) => RunModulation::NotRun {
                export: ExportFeed::Stated,
            },
        }
    }

    /// [`Self::run_modulation_of`] for `tc`, by its id. A config that is not
    /// in the session (a what-if copy) reads as a toolpath with no result.
    #[must_use]
    pub fn run_modulation_for(
        &self,
        tc: &super::ToolpathConfig,
    ) -> crate::dressup::feed_modulation::RunModulation {
        match self.find_toolpath_config_by_id(tc.id) {
            Some((index, _)) => self.run_modulation_of(index),
            None if tc.enabled && self.feed_modulation.is_on() => {
                crate::dressup::feed_modulation::RunModulation::NotRun {
                    export: crate::limits::ExportFeed::Stated,
                }
            }
            None => crate::dressup::feed_modulation::RunModulation::Off,
        }
    }

    /// The feed modulation switch of the project (operator ruling
    /// 2026-11-15). Default on.
    #[must_use]
    pub const fn feed_modulation(&self) -> FeedModulationSwitch {
        self.feed_modulation
    }

    /// The door of `Command::SetFeedModulation`.
    ///
    /// A new value drops the simulation
    /// ([`super::SimulationDropCause::FeedModulation`]): its trace holds
    /// the feeds and the times of a run under the other value. With the
    /// switch off, each modulated result takes its dressed path back
    /// ([`ResultProvenance::Modulated::dressed`]) and reads
    /// [`FeedStage::Planned`], so an export carries the planned feeds at
    /// once. It returns the indices of those results; a surface that holds
    /// its own copy of a toolpath reads them again (`Effects::modulated`).
    pub(crate) fn set_feed_modulation_impl(
        &mut self,
        switch: FeedModulationSwitch,
    ) -> BTreeSet<usize> {
        let mut restored = BTreeSet::new();
        if self.feed_modulation == switch {
            return restored;
        }
        self.feed_modulation = switch;
        self.drop_simulation(super::SimulationDropCause::FeedModulation);
        if switch.is_on() {
            return restored;
        }
        for (index, slot) in &mut self.results {
            let ResultProvenance::Modulated { dressed, .. } = &slot.provenance else {
                continue;
            };
            let path = Arc::clone(dressed.path());
            slot.op_data = match &slot.op_data {
                crate::ops::drill_op::OpData::Toolpath(_) => {
                    crate::ops::drill_op::OpData::Toolpath(path)
                }
                crate::ops::drill_op::OpData::DrillOp(drill, _) => {
                    crate::ops::drill_op::OpData::DrillOp(Arc::clone(drill), path)
                }
            };
            slot.provenance = ResultProvenance::Dressed;
            let _ = restored.insert(*index);
        }
        self.refresh_source_pointers();
        restored
    }
}
