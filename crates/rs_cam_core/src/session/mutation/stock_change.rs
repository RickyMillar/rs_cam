//! S2: the stock-change CRUD of a setup (`planning/stock_additions_2026-10-09/PLAN.md`).
//!
//! Four doors, one per command row: add, replace (the edit, the enable and
//! disable flip included), move and remove. Each door writes the setup's
//! list, then compares the list's EFFECT
//! ([`crate::compute::stock_change::stock_change_effects`]) before and after
//! the write. When the effect moved, the door runs the one invalidation rule
//! (`ProjectSession::drop_stock_change_dependents` in `mutation.rs`). A
//! rename, or an edit to a change that stays disabled, drops nothing.

use tracing::instrument;

use crate::compute::stock_change::{
    ModelGeometryFacts, StockChange, StockChangeRefusal, is_closed_outline, stock_change_effects,
    validate_stock_change,
};
use crate::ids::{ModelId, StockChangeId};
use crate::session::{Effects, ProjectSession, SessionError};

/// The geometry facts of one loaded model, or `None` when `models` holds no
/// model with that id. One rule for the command doors and the project
/// loader: a mesh, and at least one closed 2D outline by the rule the
/// "Model Outline" boundary uses.
pub(crate) fn loaded_model_facts(
    models: &[crate::session::LoadedModel],
    model_id: ModelId,
) -> Option<ModelGeometryFacts<'_>> {
    models
        .iter()
        .find(|m| m.id == model_id.0)
        .map(|m| ModelGeometryFacts {
            name: &m.name,
            has_mesh: m.mesh.is_some(),
            has_closed_outline: m
                .polygons
                .as_deref()
                .is_some_and(|polys| polys.iter().any(is_closed_outline)),
        })
}

impl ProjectSession {
    /// Check one stock change against the project models.
    ///
    /// The model facts come from the loaded models: a mesh, and at least
    /// one closed 2D outline by the rule the "Model Outline" boundary uses.
    fn validate_stock_change(&self, change: &StockChange) -> Result<(), SessionError> {
        validate_stock_change(change, |model_id| {
            loaded_model_facts(&self.models, model_id)
        })
        .map_err(SessionError::StockChangeRefused)
    }

    /// Add a stock change to the end of a setup's list.
    ///
    /// The door of the `AddStockChange` command row. It refuses a change
    /// that breaks a validation rule, or whose id the setup already holds.
    #[instrument(skip(self, change))]
    pub(crate) fn add_stock_change(
        &mut self,
        setup_index: usize,
        change: StockChange,
    ) -> Result<Effects, SessionError> {
        self.validate_stock_change(&change)?;
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            if setup.stock_changes.iter().any(|c| c.id == change.id) {
                return Err(SessionError::StockChangeRefused(
                    StockChangeRefusal::DuplicateId { id: change.id },
                ));
            }
            let before = stock_change_effects(&setup.stock_changes);
            setup.stock_changes.push(change);
            let moved = stock_change_effects(&setup.stock_changes) != before;
            if moved {
                session.drop_stock_change_dependents(setup_index);
            }
            Ok(())
        })
    }

    /// Replace one stock change of a setup, keeping its position.
    ///
    /// The door of the `ReplaceStockChange` command row, and the twin of
    /// [`Self::replace_fixture`]. The write is unconditional; the DROP is
    /// gated on the list's effect, so a rename drops nothing. An enable or
    /// disable flip moves the effect, so it drops.
    #[instrument(skip(self, change))]
    pub(crate) fn replace_stock_change(
        &mut self,
        setup_index: usize,
        change_id: StockChangeId,
        change: StockChange,
    ) -> Result<Effects, SessionError> {
        if change.id != change_id {
            return Err(SessionError::InvalidParam(format!(
                "the replacement stock change carries id {}, and the command names id {}",
                change.id.0, change_id.0
            )));
        }
        self.validate_stock_change(&change)?;
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            let before = stock_change_effects(&setup.stock_changes);
            let Some(slot) = setup.stock_changes.iter_mut().find(|c| c.id == change_id) else {
                return Err(unknown_change(setup_index, change_id));
            };
            *slot = change;
            let moved = stock_change_effects(&setup.stock_changes) != before;
            if moved {
                session.drop_stock_change_dependents(setup_index);
            }
            Ok(())
        })
    }

    /// Move one stock change of a setup to a new position in its list.
    ///
    /// The door of the `MoveStockChange` command row. The setup applies its
    /// changes in list order, so a reorder of two enabled changes drops.
    #[instrument(skip(self))]
    pub(crate) fn move_stock_change(
        &mut self,
        setup_index: usize,
        change_id: StockChangeId,
        to_position: usize,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            let Some(from) = setup.stock_changes.iter().position(|c| c.id == change_id) else {
                return Err(unknown_change(setup_index, change_id));
            };
            let count = setup.stock_changes.len();
            if to_position >= count {
                return Err(SessionError::InvalidParam(format!(
                    "setup {setup_index} holds {count} stock changes, so position \
                     {to_position} is out of range"
                )));
            }
            let before = stock_change_effects(&setup.stock_changes);
            let change = setup.stock_changes.remove(from);
            setup.stock_changes.insert(to_position, change);
            let moved = stock_change_effects(&setup.stock_changes) != before;
            if moved {
                session.drop_stock_change_dependents(setup_index);
            }
            Ok(())
        })
    }

    /// Remove one stock change from a setup.
    ///
    /// The door of the `RemoveStockChange` command row. It refuses an id
    /// the setup does not hold.
    #[instrument(skip(self))]
    pub(crate) fn remove_stock_change(
        &mut self,
        setup_index: usize,
        change_id: StockChangeId,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            let Some(at) = setup.stock_changes.iter().position(|c| c.id == change_id) else {
                return Err(unknown_change(setup_index, change_id));
            };
            let before = stock_change_effects(&setup.stock_changes);
            let _ = setup.stock_changes.remove(at);
            let moved = stock_change_effects(&setup.stock_changes) != before;
            if moved {
                session.drop_stock_change_dependents(setup_index);
            }
            Ok(())
        })
    }
}

fn unknown_change(setup_index: usize, change_id: StockChangeId) -> SessionError {
    SessionError::InvalidParam(format!(
        "setup {setup_index} carries no stock change with id {}",
        change_id.0
    ))
}
