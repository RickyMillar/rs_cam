//! Model, tool, setup, obstacle and alignment-pin CRUD.
//!
//! Split out of `session/mutation.rs` by P4. Every method here is an
//! inherent method on [`ProjectSession`], so its path does not change.

use tracing::instrument;

use crate::compute::catalog::OperationConfig;
use crate::compute::obstacle::{Obstacle, validate_obstacle};
use crate::compute::tool_config::{ToolConfig, ToolId};
use crate::compute::transform::{FaceUp, ZRotation};
use crate::ids::ObstacleId;
use crate::session::{Effects, ProjectSession, SessionError, SetupData, SimulationDropCause};

use super::obstacle_effects;

impl ProjectSession {
    // ── Model CRUD ────────────────────────────────────────────────

    /// Add a model and return its ID.
    ///
    /// When `stock.auto_from_model` is enabled, the stock dimensions are
    /// auto-updated from the new model's bounding box — mesh bbox for
    /// STL/STEP models, polygon bbox for SVG/DXF. Without this, MCP
    /// users who called `import_model` after loading a project saw the
    /// stock stay at its pre-import size (see F-13 in the April review).
    ///
    /// For 2D polygon models (zero Z extent), `update_from_bbox`
    /// preserves the existing stock Z dimension rather than collapsing
    /// it to 0.
    ///
    /// [`Effects::created`] carries the new model's ID — not its index.
    /// A model is named by id on every other surface, and this method
    /// answered with the id before WP4.
    #[instrument(skip(self, model))]
    pub(crate) fn add_model(&mut self, model: super::LoadedModel) -> Effects {
        let mut created = None;
        let mut effects = self.with_effects(None, |session| {
            created = Some(session.add_model_impl(model));
        });
        effects.created = created;
        effects
    }

    /// Append the model and report its ID. The raw half of
    /// [`Self::add_model`].
    pub(crate) fn add_model_impl(&mut self, mut model: super::LoadedModel) -> usize {
        model.id = self.next_model_id;
        self.next_model_id += 1;
        let id = model.id;
        model.bind_items();

        // Compute bbox before moving the model into self.models.
        let auto_from_model = self.stock.auto_from_model;
        let model_bbox = if auto_from_model { model.bbox() } else { None };

        self.models.push(model);

        if let Some(bbox) = model_bbox {
            self.stock.update_from_bbox(&bbox);
        }

        id
    }

    /// Remove a model by index.
    ///
    /// The removal drops the results of every toolpath that reads the
    /// model, with the same rule as a re-import (`drop_results_for_model`,
    /// geometry selection P2.6). A toolpath binds a model by id, so a
    /// reader of the removed model refuses to generate rather than reading
    /// the wrong geometry.
    #[instrument(skip(self))]
    pub(crate) fn remove_model(&mut self, index: usize) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            if index >= session.models.len() {
                return Err(SessionError::ModelIndexNotFound(index));
            }
            // P2.6: the same rule as a re-import. Every toolpath that reads
            // the model (its own model, a rule, a pick, a surface or a
            // boundary: `ToolpathConfig::models_read`) loses its result
            // and the results downstream of it. A reader whose OWN model
            // is removed stays in the project; its next generation refuses
            // `MissingSource` with the model id (P2.4), so no stale result
            // of a removed model reaches an export.
            let model_id = session.models.get(index).map(|m| m.id);
            if let Some(model_id) = model_id {
                session.drop_results_for_model(model_id);
            }
            session.models.remove(index);
            Ok(())
        })
    }

    // ── Tool CRUD ─────────────────────────────────────────────────

    /// Add a tool.
    ///
    /// [`Effects::created`] carries the new tool's index in the tools
    /// list, the value this method returned before WP4. That index is
    /// NOT the tool's id: a project that has removed a tool separates
    /// the two.
    ///
    /// F029: refuses a tool outside its domain
    /// ([`SessionError::ToolOutOfDomain`]) and adds nothing.
    #[instrument(skip(self, config))]
    pub(crate) fn add_tool(&mut self, config: ToolConfig) -> Result<Effects, SessionError> {
        crate::session::validate_tool(&config)?;
        let mut created = None;
        let mut effects = self.with_effects(None, |session| {
            created = Some(session.add_tool_impl(config));
        });
        effects.created = created;
        Ok(effects)
    }

    /// Append the tool and report its index. The raw half of
    /// [`Self::add_tool`].
    pub(crate) fn add_tool_impl(&mut self, mut config: ToolConfig) -> usize {
        config.id = ToolId(self.next_tool_id);
        self.next_tool_id += 1;
        let idx = self.tools.len();
        self.tools.push(config);
        idx
    }

    /// Remove a tool by index. Errors if any toolpath still references it.
    #[instrument(skip(self))]
    pub(crate) fn remove_tool(&mut self, index: usize) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let tool = session
                .tools
                .get(index)
                .ok_or(SessionError::ToolNotFound(ToolId(index)))?;
            let tool_raw_id = tool.id.0;

            // Check no toolpaths reference this tool
            let in_use = session
                .toolpath_configs
                .iter()
                .any(|tc| tc.tool_id == tool_raw_id);
            if in_use {
                return Err(SessionError::ToolInUse(ToolId(tool_raw_id)));
            }

            session.tools.remove(index);
            Ok(())
        })
    }

    // ── Setup CRUD ────────────────────────────────────────────────

    /// Add a new setup.
    ///
    /// [`Effects::created`] carries the new setup's index in the setups
    /// list, the value this method returned before WP4.
    #[instrument(skip(self))]
    pub(crate) fn add_setup(&mut self, name: String, face_up: FaceUp) -> Effects {
        let mut created = None;
        let mut effects = self.with_effects(None, |session| {
            created = Some(session.add_setup_impl(name, face_up));
        });
        effects.created = created;
        effects
    }

    /// Remove a setup and all toolpaths it owns.
    ///
    /// The final setup is the project's required machining frame, so it
    /// cannot be removed. Owned toolpaths are invalidated before their
    /// indices disappear; simulation is dropped once for the whole cascade.
    #[instrument(skip(self))]
    pub(crate) fn remove_setup(&mut self, setup_index: usize) -> Result<Effects, SessionError> {
        self.try_with_effects(None, |session| {
            let Some(setup) = session.setups.get(setup_index) else {
                return Err(SessionError::SetupNotFound(setup_index));
            };
            if session.setups.len() == 1 {
                return Err(SessionError::InvalidParam(
                    "cannot remove the last remaining setup".to_owned(),
                ));
            }

            let removed: std::collections::BTreeSet<usize> =
                setup.toolpath_indices.iter().copied().collect();
            // S2: the setup's enabled stock changes leave with it, so the
            // stock of every later setup moves.
            let had_stock_changes =
                !crate::compute::stock_change::stock_change_effects(&setup.stock_changes)
                    .is_empty();
            if had_stock_changes {
                session.drop_stock_change_dependents(setup_index);
            }
            if removed.is_empty() {
                session.drop_simulation(SimulationDropCause::Operations);
            } else {
                session.drop_results_and_their_dependents(
                    &removed.iter().copied().collect::<Vec<_>>(),
                );
            }
            session.setups.remove(setup_index);
            if !removed.is_empty() {
                session.toolpath_configs = session
                    .toolpath_configs
                    .drain(..)
                    .enumerate()
                    .filter_map(|(index, config)| (!removed.contains(&index)).then_some(config))
                    .collect();
                let mut rekeyed_results = std::collections::HashMap::new();
                for (index, result) in session.results.drain() {
                    if !removed.contains(&index) {
                        rekeyed_results.insert(index - removed.range(..index).count(), result);
                    }
                }
                session.results = rekeyed_results;
                for setup in &mut session.setups {
                    for index in &mut setup.toolpath_indices {
                        *index -= removed.range(..*index).count();
                    }
                }
                session.bump_all_revisions();
            }
            Ok(())
        })
    }

    /// Append the setup and report its index. The raw half of
    /// [`Self::add_setup`].
    pub(crate) fn add_setup_impl(&mut self, name: String, face_up: FaceUp) -> usize {
        let id = self.next_setup_id;
        self.next_setup_id += 1;
        let idx = self.setups.len();
        self.setups.push(SetupData {
            id,
            name,
            face_up,
            z_rotation: ZRotation::default(),
            datum: crate::session::DatumConfig::default(),
            model_ids: Vec::new(),
            obstacles: Vec::new(),
            stock_changes: Vec::new(),
            toolpath_indices: Vec::new(),
            pause_message: None,
        });
        idx
    }

    /// Apply one edit to the selection `target` of a toolpath (geometry
    /// selection P2.3c; P4.1: the op selection or the boundary selection),
    /// invalidating its cached result and the results of every operation
    /// downstream of it.
    ///
    /// A selection edit changes which items the op cuts, or where the tool
    /// may go, so it changes the stock a downstream
    /// `StockSource::FromRemainingStock` operation reads (N6: the chain
    /// walk, not `drop_result` alone).
    ///
    /// Errors if the op holds no selection (`Op`), if the boundary source
    /// is not a selection (`Boundary`: the command never changes the
    /// source), or if the edit refuses (`SelectionEdit::RemoveRule` of an
    /// index that names no rule).
    #[instrument(skip(self, edit))]
    pub(crate) fn edit_toolpath_selection(
        &mut self,
        index: usize,
        target: crate::session::SelectionTarget,
        edit: crate::selection::SelectionEdit,
    ) -> Result<Effects, SessionError> {
        use crate::session::SelectionTarget;
        self.try_with_effects(Some(index), move |session| {
            // Read before the slot borrow: `tc` is borrowed mutably there.
            let boundary_default = session.boundary_selection_default();
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            let enabled = tc.enabled;
            let name = tc.name.clone();
            // P2.7: one stored spelling per item. A key or a rule on the
            // model of the toolpath is stored as `ToolpathModel`, in the
            // edit AND in the selection it edits, so a `list_model_items`
            // key and a viewport pick of one item are one pick. The
            // boundary selection takes the same rule (P4.1).
            let model = crate::ids::ModelId(tc.model_id);
            let (slot, op_default, label) = match target {
                SelectionTarget::Op => {
                    let no_selection = || {
                        SessionError::InvalidParam(format!(
                            "Toolpath '{name}' holds no selection: its operation cuts no \
                             drawing item"
                        ))
                    };
                    let op_default = tc.operation.default_selection().ok_or_else(no_selection)?;
                    let label = tc.operation.op_type().label();
                    let slot = tc.operation.selection_slot().ok_or_else(no_selection)?;
                    (slot, op_default, label)
                }
                SelectionTarget::Boundary => match &mut tc.boundary.source {
                    crate::compute::config::BoundarySource::Selection { selection, .. } => (
                        selection,
                        boundary_default,
                        crate::session::selection_status::BOUNDARY_LABEL,
                    ),
                    crate::compute::config::BoundarySource::Stock
                    | crate::compute::config::BoundarySource::ModelSilhouette
                    | crate::compute::config::BoundarySource::DerivedRestRegions { .. }
                    | crate::compute::config::BoundarySource::PlannedTierRegions { .. } => {
                        return Err(crate::session::selection_status::boundary_not_a_selection(
                            &name,
                        ));
                    }
                },
            };
            let mut next = slot.clone().for_toolpath_model(model);
            next.apply(
                edit.for_toolpath_model(model),
                &op_default.for_toolpath_model(model),
            )
            .map_err(|e| SessionError::Selection(e.for_op(label)))?;
            *slot = next;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    // ── Setup mutations ──────────────────────────────────────────

    /// Rename a setup. This is metadata-only and does not affect compute.
    ///
    /// The door of the `SetSetupName` command row. [`Effects::stale`] is
    /// empty by design: the name reaches the M0 pause message and the
    /// setup sheet, never a generation input.
    ///
    /// Crate-private since WP16; every surface renames through
    /// `ProjectSession::apply`.
    #[instrument(skip(self))]
    pub(crate) fn rename_setup(
        &mut self,
        index: usize,
        name: String,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(index)
                .ok_or(SessionError::SetupNotFound(index))?;
            setup.name = name;
            Ok(())
        })
    }

    /// Replace a setup's datum — how the operator zeroes the machine for
    /// it.
    ///
    /// The door of the `SetSetupDatum` command row. [`Effects::stale`]
    /// is empty by design: every result in a setup is generated in that
    /// setup's own local frame, and the datum reaches the EXPORT alone
    /// (`StockTop` puts Z0 at the stock top of the presented face). A
    /// datum edit therefore moves no geometry a result holds.
    #[instrument(skip(self))]
    pub(crate) fn set_setup_datum(
        &mut self,
        setup_index: usize,
        datum: super::DatumConfig,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.datum = datum;
            Ok(())
        })
    }

    /// Replace the message the operator reads at a setup change.
    ///
    /// The door of the `SetSetupPauseMessage` command row.
    /// [`Effects::stale`] is empty by design, on the
    /// [`Self::set_setup_datum`] precedent: the message reaches the
    /// EXPORT alone, beside the `M0` the post emits, so it moves no
    /// geometry a result holds.
    #[instrument(skip(self))]
    pub(crate) fn set_setup_pause_message(
        &mut self,
        setup_index: usize,
        message: Option<String>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.pause_message = message;
            Ok(())
        })
    }

    /// Replace the models in scope for a setup.
    ///
    /// The door of the `SetSetupModels` command row. An EMPTY list means
    /// "all models", which is not the same as a list naming every model.
    ///
    /// Drops every result in the setup, like [`Self::set_setup_face`]:
    /// the scope decides which geometry a generation in this setup may
    /// read.
    #[instrument(skip(self))]
    pub(crate) fn set_setup_models(
        &mut self,
        setup_index: usize,
        model_ids: Vec<crate::ids::ModelId>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.model_ids = model_ids;
            session.drop_setup_results(setup_index);
            Ok(())
        })
    }

    /// K1: check an obstacle against the project models, for the three
    /// obstacle doors.
    fn check_obstacle(&self, obstacle: &Obstacle) -> Result<(), SessionError> {
        validate_obstacle(obstacle, |id| {
            crate::compute::obstacle::model_geometry_facts(&self.models, id)
        })
        .map_err(SessionError::ObstacleRefused)
    }

    /// Add an obstacle to the end of a setup's list (K1). The door of the
    /// `AddObstacle` command row. An enabled obstacle drops the setup's
    /// results and their dependents.
    #[instrument(skip(self, obstacle))]
    pub(crate) fn add_obstacle(
        &mut self,
        setup_index: usize,
        obstacle: Obstacle,
    ) -> Result<Effects, SessionError> {
        self.check_obstacle(&obstacle)?;
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            if setup.obstacles.iter().any(|o| o.id == obstacle.id) {
                return Err(SessionError::ObstacleRefused(
                    crate::compute::obstacle::ObstacleRefusal::DuplicateId { id: obstacle.id },
                ));
            }
            let moved = obstacle.enabled;
            setup.obstacles.push(obstacle);
            if moved {
                session.drop_obstacle_dependents(setup_index);
            }
            Ok(())
        })
    }

    /// Replace one obstacle of a setup, keeping its position (K1). The
    /// door of the `ReplaceObstacle` command row (`edit_obstacle`).
    ///
    /// The write is unconditional; the DROP is gated on the list's effect,
    /// so a rename drops nothing.
    #[instrument(skip(self, obstacle))]
    pub(crate) fn replace_obstacle(
        &mut self,
        setup_index: usize,
        obstacle_id: ObstacleId,
        obstacle: Obstacle,
    ) -> Result<Effects, SessionError> {
        if obstacle.id != obstacle_id {
            return Err(SessionError::InvalidParam(format!(
                "the replacement obstacle carries id {}, and the command names id {}",
                obstacle.id.0, obstacle_id.0
            )));
        }
        self.check_obstacle(&obstacle)?;
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            let before = obstacle_effects(&setup.obstacles);
            let Some(slot) = setup.obstacles.iter_mut().find(|o| o.id == obstacle_id) else {
                return Err(SessionError::InvalidParam(format!(
                    "setup {setup_index} carries no obstacle with id {}",
                    obstacle_id.0
                )));
            };
            *slot = obstacle;
            let moved = obstacle_effects(&setup.obstacles) != before;
            if moved {
                session.drop_obstacle_dependents(setup_index);
            }
            Ok(())
        })
    }

    /// Remove an obstacle from a setup by its id (K1). The door of the
    /// `RemoveObstacle` command row.
    #[instrument(skip(self))]
    pub(crate) fn remove_obstacle(
        &mut self,
        setup_index: usize,
        obstacle_id: ObstacleId,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            let Some(position) = setup.obstacles.iter().position(|o| o.id == obstacle_id) else {
                return Err(SessionError::InvalidParam(format!(
                    "setup {setup_index} carries no obstacle with id {}",
                    obstacle_id.0
                )));
            };
            let removed = setup.obstacles.remove(position);
            if removed.enabled {
                session.drop_obstacle_dependents(setup_index);
            }
            Ok(())
        })
    }

    /// Replace the stock's alignment pins with `pins` (P4).
    ///
    /// The stale set follows what reads the pins (operator ruling
    /// 2026-10-12), not the whole project. The pins reach a generation
    /// only through an `AlignmentPinDrill` operation: the generation pass
    /// copies the live pins into its holes (`compute/generation.rs`). So
    /// the setter drops the result of every `AlignmentPinDrill` operation,
    /// in every setup, through [`Self::drop_results_and_their_dependents`].
    /// That walk also drops what depends on those results through the
    /// dependency edges (a later `FromRemainingStock` operation in the same
    /// setup, a rest consumer) and clears the simulation, which carved the
    /// old holes.
    ///
    /// With no pin-drill operation no result reads the pins, so nothing
    /// drops and the simulation stays. A set equal to the stored set
    /// changes nothing and reports an empty [`Effects`].
    #[instrument(skip(self, pins))]
    pub(crate) fn set_alignment_pins(
        &mut self,
        pins: Vec<crate::compute::alignment_pins::AlignmentPin>,
    ) -> Effects {
        self.with_effects(None, move |session| {
            if session.stock.alignment_pins == pins {
                return;
            }
            session.stock.alignment_pins = pins;
            let pin_drills: Vec<usize> = session
                .toolpath_configs
                .iter()
                .enumerate()
                .filter(|(_, tc)| matches!(tc.operation, OperationConfig::AlignmentPinDrill(_)))
                .map(|(index, _)| index)
                .collect();
            if !pin_drills.is_empty() {
                session.drop_results_and_their_dependents(&pin_drills);
            }
        })
    }

    /// Store which models count as the part for the pins (P7). `None` is
    /// the default rule. The choice moves no pin and no toolpath, so the
    /// stale set is empty.
    pub(crate) fn set_pin_part_models(&mut self, models: Option<Vec<usize>>) -> Effects {
        self.with_effects(None, move |session| {
            session.stock.pin_part_models = models;
        })
    }

    /// Add an alignment pin, deduping against existing pins within 0.01mm.
    ///
    /// A thin form of [`Self::set_alignment_pins`]: one more pin, the same
    /// stale set. Reports the effects when the pin was added, and `None`
    /// when a duplicate was skipped. `None` says the call changed nothing.
    #[instrument(skip(self))]
    pub(crate) fn add_alignment_pin(&mut self, x: f64, y: f64, diameter: f64) -> Option<Effects> {
        use crate::selection::CAD_ROUNDING_MM;
        let exists = self.stock.alignment_pins.iter().any(|p| {
            (p.at.x() - x).abs() < CAD_ROUNDING_MM && (p.at.y() - y).abs() < CAD_ROUNDING_MM
        });
        if exists {
            return None;
        }
        let mut pins = self.stock.alignment_pins.clone();
        pins.push(crate::compute::alignment_pins::AlignmentPin::new(
            x, y, diameter,
        ));
        Some(self.set_alignment_pins(pins))
    }

    /// Remove an alignment pin by index. Returns `Err` if out of bounds.
    ///
    /// A thin form of [`Self::set_alignment_pins`], with the same stale
    /// set. `index` names a PIN, not a toolpath, so [`Effects::revision`]
    /// is `None`.
    #[instrument(skip(self))]
    pub(crate) fn remove_alignment_pin(&mut self, index: usize) -> Result<Effects, SessionError> {
        if index >= self.stock.alignment_pins.len() {
            return Err(SessionError::InvalidParam(format!(
                "alignment pin index {index} out of range ({} pins)",
                self.stock.alignment_pins.len()
            )));
        }
        let mut pins = self.stock.alignment_pins.clone();
        pins.remove(index);
        Ok(self.set_alignment_pins(pins))
    }

    /// Set a setup's face-up orientation, dropping every result in that
    /// setup. The transform decides the frame the toolpaths are generated
    /// in, so none of them survives the change.
    ///
    /// Idempotent in the value: passing the face the setup already has
    /// still drops, because the GUI panel writes the field itself and then
    /// calls this to record the consequence.
    #[instrument(skip(self))]
    pub(crate) fn set_setup_face(
        &mut self,
        setup_index: usize,
        face_up: FaceUp,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.face_up = face_up;
            session.drop_setup_results(setup_index);
            Ok(())
        })
    }

    /// Set a setup's Z rotation, dropping every result in that setup.
    /// See [`Self::set_setup_face`].
    #[instrument(skip(self))]
    pub(crate) fn set_setup_rotation(
        &mut self,
        setup_index: usize,
        z_rotation: ZRotation,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.z_rotation = z_rotation;
            session.drop_setup_results(setup_index);
            Ok(())
        })
    }
}
