//! CRUD mutation methods on [`ProjectSession`].
//!
//! Every mutation here reports [`Effects`] — the toolpath indices whose
//! generation inputs moved, whether it cleared the simulation, and the
//! revision of the one toolpath it names. It builds none of that itself:
//! it runs its body inside
//! [`ProjectSession::try_with_effects`](ProjectSession::try_with_effects),
//! the one construction site (`session/command.rs`).
//!
//! P4 split the body into three children, one per entity group:
//! `mutation/toolpath.rs`, `mutation/entities.rs` and
//! `mutation/config.rs`. Each holds inherent methods on
//! [`ProjectSession`], so no path moved and no re-export is needed.

// Path anchors for the children. A child names each of these as
// `super::Name`, in code or in a doc link, so the binding must sit in
// this module.
// They stay PRIVATE. A private item is visible to the module that holds it
// and to every descendant, so a child names it through `super::`. A
// `pub(super)` binding would publish these names into `session` as well,
// which the split does not need.
use super::{DatumConfig, LoadedModel, ToolpathComputeResult};

mod config;
mod entities;
mod stock_change;
mod toolpath;

pub(crate) use stock_change::loaded_model_facts;

impl super::ProjectSession {
    /// S2: the one invalidation rule of a stock change in setup
    /// `from_setup`.
    ///
    /// A setup applies its stock changes before its first toolpath, and
    /// since S0 a setup starts from the final stock of the Z-axis setup
    /// before it. So a change in setup N moves the stock of setup N and of
    /// every later setup. The rule:
    ///
    /// 1. drop the result of every `StockSource::FromRemainingStock`
    ///    operation of setup N and of every later setup, enabled or not
    ///    (a disabled one keeps a result for a re-enable, and that result
    ///    read the old stock);
    /// 2. clear the simulation, with [`super::SimulationDropCause::StockChanges`];
    /// 3. walk the dependents of the dropped operations to fixpoint, with
    ///    the edges every other edit uses (`walk_output_dependents`). The
    ///    enabled ones are chain seeds: their regenerated output may cut
    ///    differently.
    ///
    /// An operation that does not read the remaining stock keeps its
    /// result: the stock change does not move its generation inputs.
    /// Setups before N keep everything; the S5 prefix memo keys every group
    /// by its own stock changes, so their simulation prefix stays a hit.
    ///
    /// A lateral setup starts from a fresh stock and passes no stock on, so
    /// for a change in a lateral setup the rule over-approximates. That is
    /// the safe side.
    ///
    /// Private: the four stock-change doors and the model refresh call it.
    /// It is not a setter and takes no command row.
    fn drop_stock_change_dependents(&mut self, from_setup: usize) {
        let indices: Vec<usize> = self
            .setups
            .iter()
            .skip(from_setup)
            .flat_map(|s| s.toolpath_indices.iter().copied())
            .collect();
        let mut seeds = std::collections::BTreeSet::new();
        let mut chain_seeds = std::collections::BTreeSet::new();
        for index in indices {
            let Some(tc) = self.toolpath_configs.get(index) else {
                continue;
            };
            if tc.stock_source != crate::compute::config::StockSource::FromRemainingStock {
                continue;
            }
            let enabled = tc.enabled;
            let _ = self.drop_result(index);
            let _ = seeds.insert(index);
            if enabled {
                let _ = chain_seeds.insert(index);
            }
        }
        self.drop_simulation(super::SimulationDropCause::StockChanges);
        let _ = self.walk_output_dependents(seeds, chain_seeds);
    }

    /// K1: drop what an obstacle edit of `setup_index` moved. Every
    /// toolpath of the setup reads the obstacles, so each result drops,
    /// then every output that depends on one of them (a rest op in a later
    /// setup reads the stock those toolpaths cut), then the simulation.
    ///
    /// Private: the three obstacle doors and the model refresh call it.
    fn drop_obstacle_dependents(&mut self, setup_index: usize) {
        let indices: Vec<usize> = self
            .setups
            .get(setup_index)
            .map(|s| s.toolpath_indices.clone())
            .unwrap_or_default();
        let mut seeds = std::collections::BTreeSet::new();
        let mut chain_seeds = std::collections::BTreeSet::new();
        for index in indices {
            let enabled = self
                .toolpath_configs
                .get(index)
                .is_some_and(|tc| tc.enabled);
            let _ = self.drop_result(index);
            let _ = seeds.insert(index);
            if enabled {
                let _ = chain_seeds.insert(index);
            }
        }
        self.drop_simulation(super::SimulationDropCause::Obstacles);
        let _ = self.walk_output_dependents(seeds, chain_seeds);
    }
}

/// K1: the effect of a setup's obstacle list, the id and the digest of
/// each obstacle in order ([`crate::compute::obstacle::Obstacle::effect_digest`]).
/// A rename leaves it equal, so a rename drops nothing.
fn obstacle_effects(
    obstacles: &[crate::compute::obstacle::Obstacle],
) -> Vec<(crate::ids::ObstacleId, u64)> {
    obstacles
        .iter()
        .map(|o| (o.id, o.effect_digest()))
        .collect()
}

/// Whether a post-config edit moved a field that reaches emitted motion
/// or the simulated clock (WP17).
///
/// `true` names the fields whose old value is baked into a cached
/// artefact — the stored toolpaths, or the cut trace. The two fields it
/// exempts are read LIVE, so no cached artefact can hold a stale copy of
/// either:
///
/// - `format`, the post flavour, which
///   [`crate::gcode::export_gcode_checked`] resolves from
///   [`ProjectSession::post_config`] at emit time;
/// - `spindle_strategy`, which the feeds suggest path reads at suggest
///   time (`ProjectSession::feeds_result_for_toolpath`).
///
/// It follows [`fixture_collision_inputs_moved`]: it equalises the two
/// exempt fields and compares the WHOLE record, so a field ADDED to
/// [`crate::gcode::PostConfig`] later counts as a motion input until someone
/// states otherwise. That is the safe default.
fn post_change_reaches_motion(
    before: &crate::gcode::PostConfig,
    after: &crate::gcode::PostConfig,
) -> bool {
    let mut exempt_before = before.clone();
    exempt_before.format.clone_from(&after.format);
    exempt_before.spindle_strategy = after.spindle_strategy;
    // G5-G7: the controller options are read at emit time only. The
    // simulation always runs the expanded drill moves.
    exempt_before.controller_waits_for_spindle = after.controller_waits_for_spindle;
    exempt_before.mist_output = after.mist_output;
    exempt_before.native_drill_cycles = after.native_drill_cycles;
    exempt_before != *after
}

#[cfg(test)]
mod tests;
