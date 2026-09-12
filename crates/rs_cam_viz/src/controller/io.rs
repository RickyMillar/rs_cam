use std::path::Path;
use std::time::Instant;

use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::session::{
    AddModelArgs, AdoptModelGeometryArgs, Command, Effects, ProjectSession, ProjectSessionBuilder,
    ReplaceSetupsAndToolpathsArgs, SetPostConfigArgs, SetProjectNameArgs, SetStockConfigArgs,
};

use crate::compute::ComputeBackend;
use crate::error::VizError;
use crate::io::import;
use crate::state::job::{ModelId, ModelKind, ModelUnits};
use crate::state::runtime::{GuiState, ToolpathRuntime};
use crate::state::selection::Selection;
use crate::state::simulation::SimulationState;

use super::AppController;

impl<B: ComputeBackend> AppController<B> {
    /// Fit the stock around a bounding box, through the command door.
    ///
    /// **This is a behaviour change** (§19 ruling 7). The hatch call
    /// `stock_mut().update_from_bbox(..)` wrote the stock and dropped
    /// nothing, so every cached result survived a stock that had just
    /// changed size. `Command::SetStockConfig` drops every result, which
    /// is the rule every other stock edit follows (G-FRESHSTATE).
    fn fit_stock_to_bbox(&mut self, bbox: &BoundingBox3) {
        let mut stock = self.state.session.stock_config().clone();
        stock.update_from_bbox(bbox);
        let command = Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(stock),
        });
        match self.state.session.apply(command) {
            Ok(effects) => {
                crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
                // WP19: the stock row drops every result and the
                // session's simulation with them. The viewport holds the
                // same one simulation, so it follows.
                if effects.simulation_cleared {
                    self.invalidate_simulation();
                }
            }
            Err(error) => {
                tracing::warn!("the automatic stock fit was refused: {error}");
            }
        }
    }

    /// Replace one model's geometry, through the command door.
    ///
    /// The one door of the three refresh routes — rescale, reload and
    /// relink. The row adopts the geometry AND drops the results bound
    /// to the model in one mutation, so the two halves cannot drift
    /// apart the way they did in G-RELOADTARGETS and G-RESCALESTALE.
    ///
    /// `rt.result` is KEPT on every stamped row, as F2.4 keeps a late
    /// result: the geometry it holds is the previous generation's
    /// answer, which is what the STALE chip and the dimmed viewport path
    /// are for.
    ///
    /// The SIMULATION is not kept. A reload replaces the geometry every
    /// result was generated against, so WP19 mirrors
    /// `Effects::simulation_cleared` into the view here: the session and
    /// the viewport hold ONE simulation (WP11b, N12 item 10).
    fn adopt_model_geometry(
        &mut self,
        model_id: ModelId,
        geometry: rs_cam_core::session::LoadedModel,
        units: Option<ModelUnits>,
    ) -> Result<(), VizError> {
        let command = Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id: model_id.0,
            geometry: Box::new(geometry),
            units,
        });
        let effects = self
            .state
            .session
            .apply(command)
            .map_err(|error| VizError::Other(error.to_string()))?;
        crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
        if effects.simulation_cleared {
            self.invalidate_simulation();
        }
        Ok(())
    }

    pub fn import_stl_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let model = import::import_stl(path, 0, 1.0)?; // ID reassigned by session
        let bbox = model.bbox();
        let auto_stock = self.state.session.stock_config().auto_from_model;
        let mesh_bbox = model.mesh.as_ref().map(|mesh| mesh.bbox);
        if let Some(mesh_bbox) = mesh_bbox
            && auto_stock
        {
            self.fit_stock_to_bbox(&mesh_bbox);
        }
        let command = Command::AddModel(AddModelArgs {
            model: Box::new(model),
        });
        let assigned_id = self.apply_created(command);
        if let Some(assigned_id) = assigned_id {
            self.state.selection = Selection::Model(ModelId(assigned_id));
        }
        self.state.gui.mark_edited();
        self.pending_upload = true;
        Ok(bbox)
    }

    pub fn import_svg_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let model = import::import_svg(path, 0, 1.0)?;
        let bbox = model.bbox();
        let command = Command::AddModel(AddModelArgs {
            model: Box::new(model),
        });
        let assigned_id = self.apply_created(command);
        if let Some(assigned_id) = assigned_id {
            self.state.selection = Selection::Model(ModelId(assigned_id));
        }
        self.state.gui.mark_edited();
        self.pending_upload = true;
        Ok(bbox)
    }

    pub fn import_dxf_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let model = import::import_dxf(path, 0, 1.0)?;
        let bbox = model.bbox();
        let command = Command::AddModel(AddModelArgs {
            model: Box::new(model),
        });
        let assigned_id = self.apply_created(command);
        if let Some(assigned_id) = assigned_id {
            self.state.selection = Selection::Model(ModelId(assigned_id));
        }
        self.state.gui.mark_edited();
        self.pending_upload = true;
        Ok(bbox)
    }

    pub fn import_step_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let model = import::import_step(path, 0, 1.0)?;
        let bbox = model.bbox();
        let auto_stock = self.state.session.stock_config().auto_from_model;
        let mesh_bbox = model.mesh.as_ref().map(|mesh| mesh.bbox);
        if let Some(mesh_bbox) = mesh_bbox
            && auto_stock
        {
            self.fit_stock_to_bbox(&mesh_bbox);
        }
        let command = Command::AddModel(AddModelArgs {
            model: Box::new(model),
        });
        let assigned_id = self.apply_created(command);
        if let Some(assigned_id) = assigned_id {
            self.state.selection = Selection::Model(ModelId(assigned_id));
        }
        self.state.gui.mark_edited();
        self.pending_upload = true;
        Ok(bbox)
    }

    pub fn rescale_model(
        &mut self,
        model_id: ModelId,
        new_units: ModelUnits,
    ) -> Result<Option<BoundingBox3>, VizError> {
        let Some(model) = self
            .state
            .session
            .models()
            .iter()
            .find(|m| m.id == model_id.0)
        else {
            return Ok(None);
        };
        if model.kind == Some(ModelKind::Step) {
            return Ok(None);
        }
        let path = model.path.clone();
        let kind = model.kind.unwrap_or(ModelKind::Stl);
        let new_model = import::import_model(&path, model_id.0, kind, new_units)?;
        let bbox = new_model.bbox();
        let auto_stock = self.state.session.stock_config().auto_from_model;

        // G-RELOADTARGETS (F4.4): one shared field split, so this door
        // cannot skip `drill_targets` and `layers` again. Pre-fix a
        // rescale moved the polygons by the unit scale and left the
        // targets where they were, so the drawn circle and the hole the
        // machine cut sat in different places.
        //
        // G-RESCALESTALE (F4.7): the row drops the results of every
        // toolpath bound to this model. A rescale moves every polygon by
        // the unit scale — 25.4x from millimetres to inches — so every
        // cached result answers the PREVIOUS size. This door ran no sweep
        // at all, so the cards stayed green and export emitted those
        // paths. The `ModelKind::Step` arm returns above, before the
        // import, so a door that moves nothing still invalidates nothing.
        //
        // A rescale IS the declared-units change, so this door supplies
        // the new units; `adopt_geometry` keeps the ones on the record.
        self.adopt_model_geometry(model_id, new_model, Some(new_units))?;

        let mesh_bbox = self
            .state
            .session
            .models()
            .iter()
            .find(|m| m.id == model_id.0)
            .and_then(|m| m.mesh.as_ref())
            .map(|mesh| mesh.bbox);
        if let Some(mesh_bbox) = mesh_bbox
            && auto_stock
        {
            self.fit_stock_to_bbox(&mesh_bbox);
        }

        self.pending_upload = true;
        self.state.gui.mark_edited();
        Ok(bbox)
    }

    pub fn reload_model(&mut self, model_id: ModelId) -> Result<(), VizError> {
        let Some(model) = self
            .state
            .session
            .models()
            .iter()
            .find(|m| m.id == model_id.0)
        else {
            return Err(VizError::Other(format!("Model {model_id:?} not found")));
        };

        let path = model.path.clone();
        let kind = model.kind.unwrap_or(ModelKind::Stl);
        let units = model.units.unwrap_or(ModelUnits::Millimeters);

        let reloaded = import::import_model(&path, model_id.0, kind, units)?;

        // G-RELOADTARGETS (F4.4). This door used to assign five fields
        // by hand and never `drill_targets` or `layers`, so the previous
        // import's targets survived beside the new polygons. A drill
        // operation reads the record at generation time, so a reloaded
        // drawing drilled the previous version's holes. `path` and `kind`
        // are the ones this call was given, so they do not move.
        //
        // G-FRESHSTATE: the geometry every dependent result was generated
        // against has just been replaced. The row drops those results and
        // reports them, so the sweep requests their regeneration; before
        // this the cards stayed green and export emitted paths for the
        // previous file contents.
        self.adopt_model_geometry(model_id, reloaded, None)?;

        self.pending_upload = true;
        self.state.gui.mark_edited();
        Ok(())
    }

    /// Point an existing model at a different file, keeping its identity.
    ///
    /// G-MODELRELINK (F4.3). Before this there was no browse-for-a-new-path
    /// route in the GUI at all: a project whose model had moved offered
    /// "Reload from disk" (the same path that just failed) or "Delete"
    /// (refused while any toolpath references it), so a project moved between
    /// machines had no repair route.
    ///
    /// **The id, name and declared units are kept and the geometry is
    /// replaced.** Keeping the id is the point — every
    /// `ToolpathConfig::model_id` goes on naming this model, so the operations
    /// survive the repair. Keeping the units is R0.7's ruling: the declared
    /// units describe the operator's source, not the bytes on disk, and
    /// `reload_model` already keeps them.
    ///
    /// **The kind must match.** A relink is "this file moved", not "use a
    /// different model" — the Input combo is the tool for the second. A mesh
    /// operation cannot run on polygons, so a kind change is refused with a
    /// notification rather than silently producing a project whose every
    /// toolpath is unrunnable.
    pub fn relink_model(&mut self, model_id: ModelId, new_path: &Path) -> Result<(), VizError> {
        let Some(model) = self
            .state
            .session
            .models()
            .iter()
            .find(|m| m.id == model_id.0)
        else {
            return Err(VizError::Other(format!("Model {model_id:?} not found")));
        };

        let units = model.units.unwrap_or(ModelUnits::Millimeters);
        let previous_kind = model.kind;
        let name = model.name.clone();

        let Some(kind) = kind_from_extension(new_path) else {
            self.push_notification(
                format!(
                    "Cannot relink '{name}': '{}' is not a model file rs_cam reads",
                    new_path.display()
                ),
                crate::controller::Severity::Warning,
            );
            return Ok(());
        };
        if let Some(previous_kind) = previous_kind
            && previous_kind != kind
        {
            self.push_notification(
                format!(
                    "Cannot relink '{name}' to a {kind:?} file: it is a {previous_kind:?} model, and the operations built on it expect that geometry. Use the operation's Input control to point it at a different model."
                ),
                crate::controller::Severity::Warning,
            );
            return Ok(());
        }

        // The interactive door, as Import and Reload use — the door the
        // `model_units_survive_reload_g_unitsreload` sentry pins against the
        // project door.
        let relinked = import::import_model(new_path, model_id.0, kind, units)?;

        // Moved, not cloned: `relinked` is this function's own import and
        // is dropped here. The field split this door wrote by hand is now
        // `LoadedModel::adopt_geometry`, shared with reload and rescale
        // (G-RELOADTARGETS, F4.4) — `path` and `kind` come from the import
        // of `new_path`, which is what a relink wants.
        //
        // The id did NOT change, and that is exactly why the row's drop is
        // required. `generation_inputs_signature` includes `model_id`, so
        // the signature comparison that catches an operator re-pointing the
        // Input combo sees NOTHING here — same id, same everything,
        // different geometry. The row keys on the id rather than on a
        // signature, which is what makes it the right instrument.
        self.adopt_model_geometry(model_id, relinked, None)?;

        // The repair is done, so the complaint goes with it. Pruned by the
        // model name because `load_warnings` is a `Vec<String>`; typing it is
        // R0.7 §7 Q4 and reaches `app.rs`, `app/mcp.rs` and the harness.
        let prefix = format!("Model '{name}' could not be loaded");
        self.load_warnings.retain(|w| !w.starts_with(&prefix));
        self.show_load_warnings = !self.load_warnings.is_empty();

        self.pending_upload = true;
        self.state.gui.mark_edited();
        Ok(())
    }

    /// Mirror the effects of a post-config write into viz state.
    ///
    /// Two doors write that block before a save — this controller and
    /// the MCP `save_project` route — and viz state follows the session
    /// on both. `Effects::stale` reaches the toolpath cards.
    /// `Effects::simulation_cleared` reaches the viewport, because the
    /// session and the viewport hold ONE simulation (WP11b, N12 item
    /// 10): a session that drops it must not leave the viewport showing
    /// one.
    pub(crate) fn adopt_post_effects(&mut self, effects: &Effects) {
        crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
        if effects.simulation_cleared {
            self.invalidate_simulation();
        }
    }

    /// Write the project to `path`.
    ///
    /// WP17: the sync of the viz post block into the session runs ONLY
    /// when the two blocks differ, and it takes the command door. This
    /// method called `set_post_config` on every save, with the block the
    /// session already held, and that setter dropped the simulation.
    /// `ProjectSession::start` then refused every `FromRemainingStock`
    /// operation, because it reads the rest snapshot from the simulation
    /// (WP11b). The GUI post panel guards the same way
    /// (`ui/properties/mod.rs`).
    pub fn save_job_to_path(&mut self, path: &Path) -> Result<(), VizError> {
        let session_post = GuiState::post_to_session(&self.state.gui.post);
        if *self.state.session.post_config() != session_post {
            let command = Command::SetPostConfig(SetPostConfigArgs {
                post: Box::new(session_post),
            });
            match self.state.session.apply(command) {
                Ok(effects) => self.adopt_post_effects(&effects),
                Err(error) => {
                    return Err(VizError::Other(format!(
                        "Save failed: the post block was refused: {error}"
                    )));
                }
            }
        }

        self.state
            .session
            .save(path)
            .map_err(|e| VizError::Other(format!("Save failed: {e}")))?;
        self.state.gui.file_path = Some(path.to_path_buf());
        self.state.gui.dirty = false;
        Ok(())
    }

    pub fn open_job_from_path(&mut self, path: &Path) -> Result<(), VizError> {
        match ProjectSession::load(path) {
            Ok(session) => {
                // Populate GUI state from session.
                let mut gui = GuiState::new();
                gui.file_path = Some(path.to_path_buf());
                gui.dirty = false;
                gui.post = GuiState::post_from_session(session.post_config());

                let loaded_at = Instant::now();
                let mut warning_messages = Vec::new();

                // Populate toolpath runtime entries.
                for tc in session.toolpath_configs() {
                    // G-LOADREGEN (F2.6, operator ruling on R0.1 §7 Q2):
                    // a load requests regeneration for 2.5D operations
                    // only, respecting each operation's own dial. Opening
                    // a 3D job should not silently start minutes of
                    // compute the operator did not ask for.
                    //
                    // This used to force `auto_regen = true` on EVERY
                    // operation regardless of `default_auto_regen()`, so
                    // `process_auto_regen` submitted all of them 500 ms
                    // after load — including the 3D families the card
                    // labels MAN. That load-time sweep is the precondition
                    // the G-REGEN-RACE reproduction is built on (see
                    // `controller/tests.rs`, the G-REGEN-RACE section): an
                    // agent's `generate_all` arriving while one of those
                    // submits is still the lane's ACTIVE job resubmits it.
                    // The race itself stays fixed by
                    // `ToolpathSubmitOutcome` and is still needed, because
                    // 2.5D operations still auto-regenerate on load.
                    //
                    // A manual-regen operation therefore loads with no
                    // result and no request: its freshness reads
                    // `NoResult`, which every surface renders as pending
                    // work, never as a failure.
                    let auto_regen = tc.operation.default_auto_regen();
                    let mut rt = ToolpathRuntime::new(auto_regen);
                    if auto_regen {
                        rt.stale_since = Some(loaded_at);
                    }
                    gui.toolpath_rt.insert(tc.id, rt);

                    // Warn about missing tool/model references
                    let tool_exists = session.tools().iter().any(|t| t.id.0 == tc.tool_id);
                    if !tool_exists {
                        warning_messages.push(format!(
                            "Toolpath '{}' references missing tool id {} and needs reassignment.",
                            tc.name, tc.tool_id
                        ));
                    }
                    let model_exists = session.models().iter().any(|m| m.id == tc.model_id);
                    if !model_exists {
                        warning_messages.push(format!(
                            "Toolpath '{}' references missing model id {} and needs reassignment.",
                            tc.name, tc.model_id
                        ));
                    }
                }

                // Warn about models that failed to load.
                for m in session.models() {
                    let has_geometry = m.mesh.is_some() || m.polygons.is_some();
                    if !has_geometry {
                        // G-MODELRELINK: say what actually went wrong.
                        // `load_error` holds the loader's own reason and was
                        // rendered NOWHERE — so a corrupt STL, an unreadable
                        // DXF and a genuinely absent file all reported "was
                        // not found", sending the operator to look for a file
                        // that was sitting right there.
                        warning_messages.push(match &m.load_error {
                            Some(detail) => format!(
                                "Model '{}' could not be loaded from '{}': {detail}",
                                m.name,
                                m.path.display()
                            ),
                            None => format!(
                                "Model '{}' could not be loaded because '{}' was not found.",
                                m.name,
                                m.path.display()
                            ),
                        });
                    }
                }

                // Warn when the alignment pins cannot register the flip they
                // are for. This is the check that would have caught a real
                // project whose pins were centre-symmetric rather than
                // mirror-symmetric: under `FaceUp::Bottom`'s `y -> D-y`
                // neither hole landed on a dowel, so the part could not have
                // re-seated — and nothing said so until the operator was at
                // the machine. Deduped because a `Top` setup contributes only
                // the bounds line, and a project with several setups would
                // otherwise repeat it.
                for setup in session.list_setups() {
                    warning_messages.extend(
                        session
                            .stock_config()
                            .validate_pins_for_flip(setup.face_up)
                            .warnings(),
                    );
                }
                // NOT `dedup()` — that only collapses ADJACENT duplicates, and
                // a two-setup project interleaves them: setup 1 contributes
                // bounds + flip + keying, setup 2 contributes bounds again, so
                // the two identical bounds lines are never neighbours. Keep
                // first occurrence, drop later repeats, preserve order.
                {
                    let mut seen = std::collections::HashSet::new();
                    warning_messages.retain(|m| seen.insert(m.clone()));
                }

                for message in &warning_messages {
                    tracing::warn!("{message}");
                }

                self.state.session = session;
                self.state.gui = gui;
                self.state.selection = Selection::None;
                self.state.simulation = SimulationState::new();
                self.collision_positions.clear();
                self.pending_upload = true;
                self.load_warnings = warning_messages;
                self.show_load_warnings = !self.load_warnings.is_empty();
                tracing::info!("Loaded project via unified session path");
                Ok(())
            }
            Err(session_err) => {
                tracing::warn!("Session load failed ({session_err}), falling back to viz loader");
                let loaded = crate::io::project::load_project(path)?;
                let warning_messages: Vec<_> = loaded
                    .warnings
                    .iter()
                    .map(|warning| warning.message())
                    .collect();
                for message in &warning_messages {
                    tracing::warn!("{message}");
                }

                // Build session from the legacy-loaded job, then populate gui
                let job = loaded.job;
                let session = build_session_from_legacy_job(&job);
                let mut gui = GuiState::new();
                gui.file_path = Some(path.to_path_buf());
                gui.dirty = false;
                gui.post = job.post.clone();

                let loaded_at = Instant::now();
                for tp in job.all_toolpaths() {
                    // G-LOADREGEN: the legacy loader already carried a
                    // per-operation `auto_regen` from the file; the
                    // regeneration REQUEST now follows it too, instead of
                    // being set on every operation and then ignored by the
                    // sweep for the manual ones.
                    let mut rt = ToolpathRuntime::new(tp.auto_regen);
                    if tp.auto_regen {
                        rt.stale_since = Some(loaded_at);
                    }
                    gui.toolpath_rt.insert(tp.id, rt);
                }

                self.state.session = session;
                self.state.gui = gui;
                self.state.selection = Selection::None;
                self.state.simulation = SimulationState::new();
                self.collision_positions.clear();
                self.pending_upload = true;
                self.load_warnings = warning_messages;
                self.show_load_warnings = !self.load_warnings.is_empty();
                Ok(())
            }
        }
    }

    pub fn export_gcode(&self) -> Result<String, VizError> {
        crate::io::export::export_gcode_from_session(
            &self.state.session,
            &self.state.gui,
            &self.state.simulation,
        )
    }

    pub fn export_svg_preview(&self) -> Result<String, VizError> {
        use rs_cam_core::viz::toolpath_to_svg;

        let toolpaths: Vec<_> = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .filter(|tc| tc.enabled)
            .filter_map(|tc| {
                let rt = self.state.gui.toolpath_rt.get(&tc.id)?;
                rt.result.as_ref().map(|result| result.toolpath())
            })
            .collect();

        if toolpaths.is_empty() {
            return Err(VizError::Export(
                "No computed toolpaths for SVG export".to_owned(),
            ));
        }

        #[allow(clippy::indexing_slicing)]
        Ok(toolpath_to_svg(toolpaths[0], 800.0, 600.0))
    }

    pub fn export_setup_sheet_html(&self) -> String {
        // G-TIMEEST — the sheet prints a cycle time and is carried to the
        // machine, so it must be told what the on-screen surfaces are told:
        // the trace decides whether that number is a machine-model wall clock
        // or a cutting-only figure, and the sheet says which.
        crate::io::setup_sheet::generate_setup_sheet_from_session(
            &self.state.session,
            &self.state.gui,
            self.state
                .simulation
                .results
                .as_ref()
                .and_then(|r| r.cut_trace.as_ref()),
        )
    }
}

// ── Legacy fallback: build session from viz JobState ─────────────────

/// Build a `ProjectSession` from a legacy-loaded `JobState`.
fn build_session_from_legacy_job(job: &crate::state::job::JobState) -> ProjectSession {
    // The WP7a builder is the door for verbatim construction: it keeps
    // every supplied tool id and model id, and it runs no bounding-box
    // fit. `add_model` renumbers, which would break every stored
    // `ToolpathConfig::model_id`, and that is why this function reached
    // for the `models_mut` hatch before.
    //
    // **The id counters move.** `build()` raises `next_model_id` and
    // `next_tool_id` above every supplied id. The hatch push raised
    // neither, so a later import could take an id a loaded model already
    // held.
    let mut builder = ProjectSessionBuilder::new()
        .stock(job.stock.clone())
        .post(GuiState::post_to_session(&job.post))
        .machine(job.machine.clone());
    for tool in &job.tools {
        builder = builder.tool(tool.clone());
    }
    for m in &job.models {
        builder = builder.model(rs_cam_core::session::LoadedModel {
            id: m.id,
            name: m.name.clone(),
            mesh: m.mesh.clone(),
            polygons: m.polygons.clone(),
            drill_targets: std::sync::Arc::clone(&m.drill_targets),
            layers: std::sync::Arc::clone(&m.layers),
            path: m.path.clone(),
            kind: m.kind,
            units: m.units,
            enriched_mesh: m.enriched_mesh.clone(),
            winding_report: m.winding_report,
            load_error: m.load_error.clone(),
        });
    }
    let mut session = builder.build();
    // WP19 `let _ =`: this function builds a session no surface has
    // adopted. The caller rebuilds `GuiState` from scratch, so no
    // runtime row exists to stamp and no viewport holds a simulation.
    let _ = session.apply(Command::SetProjectName(SetProjectNameArgs {
        name: job.name.clone(),
    }));

    let mut session_setups = Vec::new();
    let mut session_tp_configs = Vec::new();

    for setup in &job.setups {
        let mut tp_indices = Vec::new();
        for tp in &setup.toolpaths {
            let tp_index = session_tp_configs.len();
            tp_indices.push(tp_index);
            session_tp_configs.push(rs_cam_core::session::ToolpathConfig {
                id: tp.id,
                name: tp.name.clone(),
                enabled: tp.enabled,
                operation: tp.operation.clone(),
                dressups: tp.dressups.clone(),
                heights: tp.heights.clone(),
                tool_id: tp.tool_id.0,
                model_id: tp.model_id.0,
                pre_gcode: if tp.pre_gcode.is_empty() {
                    None
                } else {
                    Some(tp.pre_gcode.clone())
                },
                post_gcode: if tp.post_gcode.is_empty() {
                    None
                } else {
                    Some(tp.post_gcode.clone())
                },
                boundary: tp.boundary.clone(),
                // Dead dial (UX-R03-009): the GUI no longer carries it;
                // the core field is written `false` for file compatibility.
                boundary_inherit: false,
                rest_analysis: tp.rest_analysis.clone(),
                stock_source: tp.stock_source,
                coolant: tp.coolant,
                face_selection: tp.face_selection.clone(),
                debug_options: tp.debug_options,
                feeds_provenance: tp.feeds_provenance.clone(),
                // Carried the same way `feeds_provenance` is: the fallback
                // loader sets it on the entry when the file has one, so a
                // legacy-rescued plan keeps its tier provenance.
                planner_origin: tp.planner_origin.clone(),
            });
        }

        session_setups.push(rs_cam_core::session::SetupData {
            id: setup.id.0,
            name: setup.name.clone(),
            face_up: setup.face_up,
            z_rotation: setup.z_rotation,
            // W9 / P-2: the fallback loader always read these two off
            // the file and then dropped them here, so even the path
            // that DID parse the datum lost it. Carried through now.
            datum: setup.datum.clone(),
            model_ids: setup.model_ids.clone(),
            fixtures: setup
                .fixtures
                .iter()
                .map(|f| rs_cam_core::session::Fixture {
                    id: f.id,
                    name: f.name.clone(),
                    kind: match f.kind {
                        crate::state::job::FixtureKind::Clamp => {
                            rs_cam_core::session::FixtureKind::Clamp
                        }
                        crate::state::job::FixtureKind::Vise => {
                            rs_cam_core::session::FixtureKind::Vise
                        }
                        crate::state::job::FixtureKind::VacuumPod => {
                            rs_cam_core::session::FixtureKind::VacuumPod
                        }
                        crate::state::job::FixtureKind::Custom => {
                            rs_cam_core::session::FixtureKind::Custom
                        }
                    },
                    enabled: f.enabled,
                    origin_x: f.origin_x,
                    origin_y: f.origin_y,
                    origin_z: f.origin_z,
                    size_x: f.size_x,
                    size_y: f.size_y,
                    size_z: f.size_z,
                    clearance: f.clearance,
                })
                .collect(),
            keep_out_zones: setup
                .keep_out_zones
                .iter()
                .map(|k| rs_cam_core::session::KeepOutZone {
                    id: k.id,
                    name: k.name.clone(),
                    enabled: k.enabled,
                    origin_x: k.origin_x,
                    origin_y: k.origin_y,
                    size_x: k.size_x,
                    size_y: k.size_y,
                })
                .collect(),
            toolpath_indices: tp_indices,
            pause_message: setup.pause_message.clone(),
        });
    }

    // WP19 `let _ =`: the same builder, over the same unadopted session.
    // The row bumps every revision and drops the simulation; neither
    // answer has a surface to reach yet.
    let _ = session.apply(Command::ReplaceSetupsAndToolpaths(
        ReplaceSetupsAndToolpathsArgs {
            setups: session_setups,
            toolpath_configs: session_tp_configs,
        },
    ));
    session
}

/// The model kind a file extension names, or `None` when rs_cam does not
/// read that extension (G-MODELRELINK).
///
/// Mirrors the `match` in `app::mcp::commands`' `import_model` arm; core's
/// `project_file::infer_model_kind` is `pub(crate)` and not reachable from
/// this crate.
fn kind_from_extension(path: &Path) -> Option<ModelKind> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("stl") => Some(ModelKind::Stl),
        Some("dxf") => Some(ModelKind::Dxf),
        Some("svg") => Some(ModelKind::Svg),
        Some("step" | "stp") => Some(ModelKind::Step),
        _ => None,
    }
}
