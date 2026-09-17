//! Model, tool, setup, fixture, keep-out and alignment-pin CRUD.
//!
//! Split out of `session/mutation.rs` by P4. Every method here is an
//! inherent method on [`ProjectSession`], so its path does not change.

use tracing::instrument;

use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::{ToolConfig, ToolId};
use crate::compute::transform::{FaceUp, ZRotation};
use crate::geometry::enriched_mesh::FaceGroupId;
use crate::ids::{FixtureId, KeepOutId};
use crate::session::{Effects, Fixture, KeepOutZone, ProjectSession, SessionError, SetupData};

use super::{fixture_collision_inputs_moved, keep_out_collision_inputs_moved, polygons_bbox};

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

        // Compute bbox before moving the model into self.models.
        let auto_from_model = self.stock.auto_from_model;
        let model_bbox = if auto_from_model {
            model.mesh.as_ref().map(|mesh| mesh.bbox).or_else(|| {
                model
                    .polygons
                    .as_ref()
                    .and_then(|polys| polygons_bbox(polys))
            })
        } else {
            None
        };

        self.models.push(model);

        if let Some(bbox) = model_bbox {
            self.stock.update_from_bbox(&bbox);
        }

        id
    }

    /// Remove a model by index.
    ///
    /// The removal drops no toolpath result. A toolpath binds a model by
    /// id, so a bound toolpath refuses to generate rather than reading
    /// the wrong geometry.
    #[instrument(skip(self))]
    pub(crate) fn remove_model(&mut self, index: usize) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            if index >= session.models.len() {
                return Err(SessionError::MissingGeometry(format!(
                    "Model index {index} not found"
                )));
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
    #[instrument(skip(self, config))]
    pub(crate) fn add_tool(&mut self, config: ToolConfig) -> Effects {
        let mut created = None;
        let mut effects = self.with_effects(None, |session| {
            created = Some(session.add_tool_impl(config));
        });
        effects.created = created;
        effects
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
            fixtures: Vec::new(),
            keep_out_zones: Vec::new(),
            toolpath_indices: Vec::new(),
            pause_message: None,
        });
        idx
    }

    /// Set the BREP face selection for a toolpath, invalidating its cached result.
    #[instrument(skip(self, face_ids))]
    pub(crate) fn set_face_selection(
        &mut self,
        index: usize,
        face_ids: Option<Vec<FaceGroupId>>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.face_selection = face_ids;
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    /// Update the alignment pin drill holes for a toolpath.
    ///
    /// Errors if the toolpath's operation is not `AlignmentPinDrill`.
    #[instrument(skip(self, holes))]
    pub(crate) fn set_alignment_pin_drill_holes(
        &mut self,
        index: usize,
        holes: Vec<[f64; 2]>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            match tc.operation {
                OperationConfig::AlignmentPinDrill(ref mut cfg) => {
                    cfg.holes = holes;
                }
                _ => {
                    return Err(SessionError::InvalidParam(
                        "Toolpath is not an AlignmentPinDrill operation".to_owned(),
                    ));
                }
            }
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    /// Set the explicitly-selected drill holes (DXF point / circle-centre
    /// picks) for a `Drill` or `AlignmentPinDrill` toolpath, invalidating its
    /// cached result and the results of every operation downstream of it.
    /// `None` reverts a `Drill` op to its legacy all-polygon-centroids
    /// behaviour.
    ///
    /// A pick changes which holes the operation cuts, so it changes the
    /// stock a downstream `StockSource::FromRemainingStock` operation
    /// reads. This called `drop_result` alone until WP8 (N6), while its
    /// sibling [`Self::set_alignment_pin_drill_holes`] 30 lines above
    /// walked the chain. The two now answer alike.
    ///
    /// Errors if the toolpath's operation is not a drilling op.
    #[instrument(skip(self, selected_holes))]
    pub(crate) fn set_drill_selected_holes(
        &mut self,
        index: usize,
        selected_holes: Option<Vec<[f64; 2]>>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            // Read before the match: `tc` is borrowed mutably inside it.
            let enabled = tc.enabled;
            match tc.operation {
                OperationConfig::Drill(ref mut cfg) => {
                    cfg.selected_holes = selected_holes;
                }
                OperationConfig::AlignmentPinDrill(ref mut cfg) => {
                    cfg.selected_holes = selected_holes;
                }
                _ => {
                    return Err(SessionError::InvalidParam(
                        "Toolpath is not a drilling operation".to_owned(),
                    ));
                }
            }
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

    /// Replace one fixture of a setup, keeping its position in the list.
    ///
    /// The door of the `ReplaceFixture` command row. The GUI fixture
    /// panel edits every field of one fixture, so the payload is the
    /// whole record.
    ///
    /// **The write is unconditional; the DROP is gated**, on the
    /// `ReplaceToolpathConfig` precedent. The panel applies one command
    /// per finished edit, and the name is not a collision input — so a
    /// ten-character rename would otherwise drop the setup's results ten
    /// times. Every OTHER field moves the obstacle the holder must
    /// clear, so it drops the setup's results exactly as
    /// [`Self::add_fixture`] and [`Self::remove_fixture`] do.
    ///
    /// **This is a behaviour change.** The panel wrote the fixture in
    /// place and dropped nothing, so a clamp could move under a cached
    /// holder-clearance verdict (G-FRESHSTATE).
    #[instrument(skip(self, fixture))]
    pub(crate) fn replace_fixture(
        &mut self,
        setup_index: usize,
        fixture_id: FixtureId,
        fixture: Fixture,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            let Some(slot) = setup.fixtures.iter_mut().find(|f| f.id == fixture_id) else {
                return Err(SessionError::InvalidParam(format!(
                    "setup {setup_index} carries no fixture with id {}",
                    fixture_id.0
                )));
            };
            let moved = fixture_collision_inputs_moved(slot, &fixture);
            *slot = fixture;
            if moved {
                session.drop_setup_results(setup_index);
            }
            Ok(())
        })
    }

    /// Replace one keep-out zone of a setup, keeping its position.
    ///
    /// The door of the `ReplaceKeepOut` command row, and the twin of
    /// [`Self::replace_fixture`]. It writes unconditionally and drops
    /// the setup's results only when a field other than the name moved,
    /// for the same reason.
    #[instrument(skip(self, zone))]
    pub(crate) fn replace_keep_out(
        &mut self,
        setup_index: usize,
        zone_id: KeepOutId,
        zone: KeepOutZone,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            let Some(slot) = setup.keep_out_zones.iter_mut().find(|z| z.id == zone_id) else {
                return Err(SessionError::InvalidParam(format!(
                    "setup {setup_index} carries no keep-out zone with id {}",
                    zone_id.0
                )));
            };
            let moved = keep_out_collision_inputs_moved(slot, &zone);
            *slot = zone;
            if moved {
                session.drop_setup_results(setup_index);
            }
            Ok(())
        })
    }

    /// Add a fixture to a setup, invalidating all toolpath results in that setup.
    #[instrument(skip(self, fixture))]
    pub(crate) fn add_fixture(
        &mut self,
        setup_index: usize,
        fixture: Fixture,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.fixtures.push(fixture);
            let indices: Vec<usize> = setup.toolpath_indices.clone();
            for &tp_idx in &indices {
                session.drop_result(tp_idx);
            }
            session.drop_simulation();
            Ok(())
        })
    }

    /// Remove a fixture from a setup by its ID.
    #[instrument(skip(self))]
    pub(crate) fn remove_fixture(
        &mut self,
        setup_index: usize,
        fixture_id: FixtureId,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.fixtures.retain(|f| f.id != fixture_id);
            let indices: Vec<usize> = setup.toolpath_indices.clone();
            for &tp_idx in &indices {
                session.drop_result(tp_idx);
            }
            session.drop_simulation();
            Ok(())
        })
    }

    /// Add a keep-out zone to a setup, invalidating all toolpath results in that setup.
    #[instrument(skip(self, zone))]
    pub(crate) fn add_keep_out(
        &mut self,
        setup_index: usize,
        zone: KeepOutZone,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.keep_out_zones.push(zone);
            let indices: Vec<usize> = setup.toolpath_indices.clone();
            for &tp_idx in &indices {
                session.drop_result(tp_idx);
            }
            session.drop_simulation();
            Ok(())
        })
    }

    /// Remove a keep-out zone from a setup by its ID.
    #[instrument(skip(self))]
    pub(crate) fn remove_keep_out(
        &mut self,
        setup_index: usize,
        zone_id: KeepOutId,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.keep_out_zones.retain(|z| z.id != zone_id);
            let indices: Vec<usize> = setup.toolpath_indices.clone();
            for &tp_idx in &indices {
                session.drop_result(tp_idx);
            }
            session.drop_simulation();
            Ok(())
        })
    }

    /// Add an alignment pin, deduping against existing pins within 0.01mm.
    ///
    /// Reports the effects when the pin was added, and `None` when a
    /// duplicate was skipped. `None` says the call changed nothing.
    #[instrument(skip(self))]
    pub(crate) fn add_alignment_pin(&mut self, x: f64, y: f64, diameter: f64) -> Option<Effects> {
        const PIN_DEDUP_EPSILON_MM: f64 = 0.01;
        let exists = self.stock.alignment_pins.iter().any(|p| {
            (p.x - x).abs() < PIN_DEDUP_EPSILON_MM && (p.y - y).abs() < PIN_DEDUP_EPSILON_MM
        });
        if exists {
            return None;
        }
        Some(self.with_effects(None, move |session| {
            session
                .stock
                .alignment_pins
                .push(crate::compute::alignment_pins::AlignmentPin::new(
                    x, y, diameter,
                ));
            session.drop_all_results();
        }))
    }

    /// Remove an alignment pin by index. Returns `Err` if out of bounds.
    ///
    /// `index` names a PIN, not a toolpath, so [`Effects::revision`] is
    /// `None`.
    #[instrument(skip(self))]
    pub(crate) fn remove_alignment_pin(&mut self, index: usize) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            if index >= session.stock.alignment_pins.len() {
                return Err(SessionError::InvalidParam(format!(
                    "alignment pin index {index} out of range ({} pins)",
                    session.stock.alignment_pins.len()
                )));
            }
            session.stock.alignment_pins.remove(index);
            session.drop_all_results();
            Ok(())
        })
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
