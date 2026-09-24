//! The one stored simulation resolution, and the rule that says when a
//! rest snapshot or a rest result is current (G-RESTRES, G-RESTSTALE).
//!
//! `planning/rest_stock_identity_2026-09-24/PLAN.md` §3, with the operator
//! rulings of 2026-09-24 in its §7:
//!
//! - ONE project value sets the cell of EVERY simulation: a plan prefix,
//!   the closing full simulation, a plain Run Simulation, the CLI. The
//!   project file stores it; `Auto` is a mode and is worked out over the
//!   whole project, never per plan scope or per request.
//! - A rest result whose recorded source stock no longer matches the
//!   project is DROPPED, so its row reads WAIT.
//!
//! # The rule
//!
//! A [`SourceStock`] is current for a consumer when all three hold:
//!
//! 1. its cell is [`ProjectSession::simulation_resolution_mm`], and the
//!    simulation carved with the cutting-metrics kernel (the one the CLI and
//!    MCP always use);
//! 2. every toolpath it carved is still enabled, still holds a result, and
//!    that result carries the same moves;
//! 3. every enabled toolpath before the consumer (all earlier setups, then
//!    the rows above it) whose result has simulated motion is in the list.
//!
//! [`ProjectSession::snapshot_is_current`] asks it of a `prior_stocks`
//! snapshot, and the plan, the card and `start` read that answer.
//! [`ProjectSession::rest_results_out_of_date`] asks it of every rest
//! result, and `try_with_effects` drops the hits after every command.

use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::compute::config::StockSource;
use crate::compute::simulate::contributes_simulated_motion;
use crate::compute::source_stock::SourceStock;
use crate::ids::ToolpathId;
use crate::session::ProjectSession;

/// The stored simulation cell size.
///
/// The project file writes `[job.simulation] resolution_mm = <mm>` for
/// [`Self::Fixed`] and omits the key for [`Self::Auto`]. A file with no key
/// therefore loads as `Auto`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationResolution {
    /// Work the cell out from the project: see
    /// [`ProjectSession::simulation_resolution_mm`].
    #[default]
    Auto,
    /// This cell size, in mm.
    Fixed(f64),
}

/// The finest cell the rules may ask for, in mm. Below it the dexel grid
/// costs more than the detail it resolves.
pub const RESOLUTION_FLOOR_MM: f64 = 0.02;

/// The coarsest cell the tool rule may give, in mm.
const TOOL_RULE_CEILING_MM: f64 = 0.5;

/// The dexel column budget the tool rule keeps the grid under.
const MAX_GRID_CELLS: f64 = 8_000_000.0;

/// Why a snapshot cannot seed a rest operation.
#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotMiss {
    /// The session holds no snapshot for the consumer.
    Missing,
    /// The snapshot was simulated at another cell size.
    OtherCell {
        /// The cell the snapshot was simulated at, in mm.
        snapshot_mm: f64,
        /// The project's cell, in mm.
        project_mm: f64,
    },
    /// The simulation carved without the cutting-metrics kernel, which
    /// leaves different stock from the one the CLI and MCP carve.
    NoMetricsCarve,
    /// A toolpath the snapshot carved has changed, or a toolpath it did not
    /// carve now cuts before the consumer.
    SourceMoved {
        /// The name of the first toolpath that differs.
        toolpath: String,
    },
}

impl SnapshotMiss {
    /// One sentence for the refusal and the hover.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Missing => "no simulated stock snapshot exists for it".to_owned(),
            Self::OtherCell {
                snapshot_mm,
                project_mm,
            } => format!(
                "the simulated stock is at {snapshot_mm:.3} mm cells and the project \
                 resolution is {project_mm:.3} mm"
            ),
            Self::NoMetricsCarve => {
                "the simulated stock was carved without cutting metrics".to_owned()
            }
            Self::SourceMoved { toolpath } => {
                format!("'{toolpath}' changed after the stock was simulated")
            }
        }
    }
}

impl ProjectSession {
    /// The stored simulation resolution.
    #[must_use]
    pub fn simulation_resolution(&self) -> SimulationResolution {
        self.simulation_resolution
    }

    /// The cell size, in mm, every simulation of this project runs at.
    ///
    /// `Fixed` answers its value. `Auto` answers the finer of two rules,
    /// both over the WHOLE project:
    ///
    /// - the tool rule: the smallest tool radius over enabled operations
    ///   (generated or not) / 5, clamped to [0.02, 0.5] mm, then made no
    ///   finer than the ~8M-column grid budget allows;
    /// - the rest rule ([`Self::rest_resolution_required_mm`]).
    ///
    /// The answer depends on no plan scope and no request, so the GUI, MCP
    /// and the CLI read one number for one project state.
    #[must_use]
    pub fn simulation_resolution_mm(&self) -> f64 {
        match self.simulation_resolution {
            SimulationResolution::Fixed(mm) => mm,
            SimulationResolution::Auto => {
                let tool_rule = self.auto_tool_rule_mm();
                match self.rest_resolution_required_mm() {
                    Some(rest) => tool_rule.min(rest),
                    None => tool_rule,
                }
            }
        }
    }

    /// The coarsest cell that still resolves the rest the project machines,
    /// in mm, or `None` when no enabled operation starts from remaining
    /// stock.
    ///
    /// R1 (2026-09-18): the rest tool's TIP radius / 5, floor
    /// [`RESOLUTION_FLOOR_MM`], the finest over every enabled
    /// `FromRemainingStock` operation in the project.
    #[must_use]
    pub fn rest_resolution_required_mm(&self) -> Option<f64> {
        let mut finest: Option<f64> = None;
        for tc in self
            .toolpath_configs
            .iter()
            .filter(|tc| tc.enabled && tc.stock_source == StockSource::FromRemainingStock)
        {
            let Some(tool) = self.tools.iter().find(|t| t.id.0 == tc.tool_id) else {
                continue;
            };
            let needed = ((tool.diameter / 2.0) / 5.0).max(RESOLUTION_FLOOR_MM);
            finest = Some(finest.map_or(needed, |held: f64| held.min(needed)));
        }
        finest
    }

    /// The tool rule of [`Self::simulation_resolution_mm`].
    fn auto_tool_rule_mm(&self) -> f64 {
        let used: HashSet<usize> = self
            .toolpath_configs
            .iter()
            .filter(|tc| tc.enabled)
            .map(|tc| tc.tool_id)
            .collect();
        let min_radius = self
            .tools
            .iter()
            .filter(|t| used.contains(&t.id.0))
            .map(|t| t.diameter / 2.0)
            .fold(f64::INFINITY, f64::min);
        let from_tool = if min_radius.is_finite() {
            (min_radius / 5.0).clamp(RESOLUTION_FLOOR_MM, TOOL_RULE_CEILING_MM)
        } else {
            TOOL_RULE_CEILING_MM
        };
        let bbox = self.stock_bbox();
        let sx = bbox.max.x - bbox.min.x;
        let sy = bbox.max.y - bbox.min.y;
        let from_grid = ((sx * sy) / MAX_GRID_CELLS).sqrt().max(RESOLUTION_FLOOR_MM);
        from_tool.max(from_grid)
    }

    /// Is the session's `prior_stocks` snapshot for `id` the one the
    /// current project would make?
    ///
    /// # Errors
    /// The [`SnapshotMiss`] that says why not.
    pub fn snapshot_is_current(&self, id: ToolpathId) -> Result<(), SnapshotMiss> {
        let Some(sim) = self.simulation.as_ref() else {
            return Err(SnapshotMiss::Missing);
        };
        if !sim.prior_stocks.contains_key(&id) {
            return Err(SnapshotMiss::Missing);
        }
        let Some(source) = sim.prior_stock_sources.get(&id) else {
            return Err(SnapshotMiss::Missing);
        };
        self.source_is_current(id, source)
    }

    /// The rule in the module doc, for one consumer and one record.
    ///
    /// # Errors
    /// The [`SnapshotMiss`] that says why the record is not current.
    pub fn source_is_current(
        &self,
        consumer: ToolpathId,
        source: &SourceStock,
    ) -> Result<(), SnapshotMiss> {
        let project_mm = self.simulation_resolution_mm();
        if source.cell_mm.to_bits() != project_mm.to_bits() {
            return Err(SnapshotMiss::OtherCell {
                snapshot_mm: source.cell_mm,
                project_mm,
            });
        }
        if !source.metrics {
            return Err(SnapshotMiss::NoMetricsCarve);
        }
        // (2) every carved toolpath is unchanged.
        for entry in &source.after {
            let moved = || SnapshotMiss::SourceMoved {
                toolpath: self
                    .find_toolpath_config_by_id(entry.id)
                    .map_or_else(|| format!("#{}", entry.id.0), |(_, tc)| tc.name.clone()),
            };
            let Some((index, tc)) = self.find_toolpath_config_by_id(entry.id) else {
                return Err(moved());
            };
            if !tc.enabled {
                return Err(moved());
            }
            let Some(result) = self.results.get(&index) else {
                return Err(moved());
            };
            if !entry.matches(result.annotated()) {
                return Err(moved());
            }
        }
        // (3) nothing new cuts before the consumer.
        let carved: HashSet<ToolpathId> = source.after.iter().map(|e| e.id).collect();
        for index in self.carve_order_before(consumer) {
            let Some(tc) = self.toolpath_configs.get(index) else {
                continue;
            };
            if !tc.enabled || carved.contains(&tc.id) {
                continue;
            }
            let cuts = self
                .results
                .get(&index)
                .is_some_and(|r| contributes_simulated_motion(r.annotated().toolpath.moves.len()));
            if cuts {
                return Err(SnapshotMiss::SourceMoved {
                    toolpath: tc.name.clone(),
                });
            }
        }
        Ok(())
    }

    /// The record a simulation of the project AS IT STANDS would write for
    /// `consumer`'s snapshot, or `None` when an enabled toolpath before it
    /// holds no result (the snapshot cannot exist yet).
    ///
    /// The rule's own answer, for a test that hands the session a snapshot
    /// it built by hand, and for a reader that wants to show what a current
    /// snapshot would carve.
    #[must_use]
    pub fn expected_source(&self, consumer: ToolpathId) -> Option<SourceStock> {
        let mut after = Vec::new();
        for index in self.carve_order_before(consumer) {
            let tc = self.toolpath_configs.get(index)?;
            if !tc.enabled {
                continue;
            }
            let result = self.results.get(&index)?;
            if contributes_simulated_motion(result.annotated().toolpath.moves.len()) {
                after.push(crate::compute::source_stock::SourceEntry::of(
                    tc.id,
                    result.annotated(),
                ));
            }
        }
        Some(SourceStock {
            cell_mm: self.simulation_resolution_mm(),
            metrics: true,
            after,
        })
    }

    /// Every toolpath index the simulator carves before `consumer`: all rows
    /// of every earlier setup, then the rows above it in its own setup.
    fn carve_order_before(&self, consumer: ToolpathId) -> Vec<usize> {
        let mut out = Vec::new();
        for setup in &self.setups {
            for &index in &setup.toolpath_indices {
                if self
                    .toolpath_configs
                    .get(index)
                    .is_some_and(|tc| tc.id == consumer)
                {
                    return out;
                }
                out.push(index);
            }
        }
        out
    }

    /// Every rest result whose recorded source stock no longer matches the
    /// project.
    ///
    /// A result with no record (`stats.source_stock == None`) is not
    /// judged: a real rest generation always records one, so `None` names
    /// a result built by hand in a test.
    #[must_use]
    pub fn rest_results_out_of_date(&self) -> BTreeSet<usize> {
        let mut out = BTreeSet::new();
        for (&index, result) in &self.results {
            let Some(tc) = self.toolpath_configs.get(index) else {
                continue;
            };
            if tc.stock_source != StockSource::FromRemainingStock {
                continue;
            }
            let Some(source) = result.stats.source_stock.as_ref() else {
                continue;
            };
            if self.source_is_current(tc.id, source).is_err() {
                let _ = out.insert(index);
            }
        }
        out
    }

    /// Drop every out-of-date rest result, and walk its dependents.
    ///
    /// The drop bumps each revision, so the `Effects` of the command that
    /// ran this reports the hits. The walk is the pure one: it drops
    /// dependents and touches no simulation. Runs to a fixpoint, because a
    /// drop can make a later record name a toolpath that no longer holds a
    /// result.
    pub(crate) fn drop_out_of_date_rest_results(&mut self) {
        loop {
            let hits = self.rest_results_out_of_date();
            if hits.is_empty() {
                break;
            }
            for &index in &hits {
                let _ = self.drop_result(index);
            }
            let _ = self.walk_output_dependents(hits.clone(), hits);
        }
        self.refresh_source_pointers();
    }

    /// Point every recorded source entry at the `Arc` the session holds now,
    /// where that `Arc` carries the same moves.
    ///
    /// The comparison hashes a toolpath only when its pointer differs from
    /// the one recorded. A result that was swapped for an equal one (the
    /// feed modulation after each simulation, an equal regenerate) would
    /// otherwise be hashed on every command and on every frame the card
    /// reads an edge state.
    fn refresh_source_pointers(&mut self) {
        let current: std::collections::HashMap<
            ToolpathId,
            std::sync::Arc<crate::trace::toolpath_spans::AnnotatedToolpath>,
        > = self
            .results
            .iter()
            .filter_map(|(&index, result)| {
                self.toolpath_configs
                    .get(index)
                    .map(|tc| (tc.id, std::sync::Arc::clone(result.annotated())))
            })
            .collect();
        let refresh = |source: &mut SourceStock| {
            for entry in &mut source.after {
                if let Some(arc) = current.get(&entry.id) {
                    entry.refresh(arc);
                }
            }
        };
        for result in self.results.values_mut() {
            if let Some(source) = result.stats.source_stock.as_mut() {
                refresh(source);
            }
        }
        if let Some(sim) = self.simulation.as_mut() {
            for source in sim.prior_stock_sources.values_mut() {
                refresh(source);
            }
        }
    }

    /// The refusal every surface gives when the stored cell cannot resolve
    /// the rest the project machines, or `None` when it can.
    ///
    /// Only a `Fixed` value can be too coarse: `Auto` takes the finer of
    /// its tool rule and the rest rule. The GUI asks the operator (R1); MCP
    /// and the CLI refuse with this sentence, and never wait on a click.
    #[must_use]
    pub fn rest_resolution_refusal(&self) -> Option<String> {
        let SimulationResolution::Fixed(mm) = self.simulation_resolution else {
            return None;
        };
        let required = self.rest_resolution_required_mm()?;
        (mm > required).then(|| {
            format!(
                "the project simulation resolution is {mm} mm, coarser than the {required} mm \
                 the rest this project machines needs (the rest tool's TIP radius / 5). At that \
                 cell size the simulated stock cannot resolve what the coarse tool left. Set \
                 the resolution to {required} mm or finer, or to auto."
            )
        })
    }

    /// The door of `Command::SetSimulationResolution`.
    ///
    /// A new value drops the simulation: its evidence is at a cell the
    /// project no longer names. The rest results at the old cell drop in
    /// the sweep `try_with_effects` runs after this.
    pub(crate) fn set_simulation_resolution_impl(
        &mut self,
        resolution: SimulationResolution,
    ) -> Result<(), crate::session::SessionError> {
        if let SimulationResolution::Fixed(mm) = resolution
            && !(mm.is_finite() && mm > 0.0)
        {
            return Err(crate::session::SessionError::InvalidParam(format!(
                "simulation resolution {mm} is not a positive cell size in mm"
            )));
        }
        if self.simulation_resolution == resolution {
            return Ok(());
        }
        let before = self.simulation_resolution_mm();
        self.simulation_resolution = resolution;
        if self.simulation_resolution_mm().to_bits() != before.to_bits() {
            self.drop_simulation();
        }
        Ok(())
    }
}
