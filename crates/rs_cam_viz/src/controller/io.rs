use std::path::Path;
use std::time::Instant;

use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::session::ProjectSession;

use crate::compute::ComputeBackend;
use crate::error::VizError;
use crate::io::import;
use crate::state::job::{ModelId, ModelKind, ModelUnits};
use crate::state::runtime::{GuiState, ToolpathRuntime};
use crate::state::selection::Selection;
use crate::state::simulation::SimulationState;

use super::AppController;

// ── Helper: convert viz LoadedModel → session LoadedModel ────────────
fn viz_model_to_session(m: &crate::state::job::LoadedModel) -> rs_cam_core::session::LoadedModel {
    rs_cam_core::session::LoadedModel {
        id: m.id.0,
        name: m.name.clone(),
        mesh: m.mesh.clone(),
        polygons: m.polygons.clone(),
        path: m.path.clone(),
        kind: Some(m.kind),
        units: Some(m.units),
        enriched_mesh: m.enriched_mesh.clone(),
        winding_report: m.winding_report,
        load_error: m.load_error.clone(),
    }
}

impl<B: ComputeBackend> AppController<B> {
    pub fn import_stl_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let id = ModelId(0); // placeholder — session assigns real ID
        let model = import::import_stl(path, id, 1.0)?;
        let bbox = model.bbox();
        let auto_stock = self.state.session.stock_config().auto_from_model;
        if let Some(mesh) = &model.mesh
            && auto_stock
        {
            self.state.session.stock_mut().update_from_bbox(&mesh.bbox);
        }
        let sm = viz_model_to_session(&model);
        let assigned_id = self.state.session.add_model(sm);
        self.state.selection = Selection::Model(ModelId(assigned_id));
        self.state.gui.mark_edited();
        self.pending_upload = true;
        Ok(bbox)
    }

    pub fn import_svg_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let id = ModelId(0);
        let model = import::import_svg(path, id, 1.0)?;
        let bbox = model.bbox();
        let sm = viz_model_to_session(&model);
        let assigned_id = self.state.session.add_model(sm);
        self.state.selection = Selection::Model(ModelId(assigned_id));
        self.state.gui.mark_edited();
        self.pending_upload = true;
        Ok(bbox)
    }

    pub fn import_dxf_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let id = ModelId(0);
        let model = import::import_dxf(path, id, 1.0)?;
        let bbox = model.bbox();
        let sm = viz_model_to_session(&model);
        let assigned_id = self.state.session.add_model(sm);
        self.state.selection = Selection::Model(ModelId(assigned_id));
        self.state.gui.mark_edited();
        self.pending_upload = true;
        Ok(bbox)
    }

    pub fn import_step_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let id = ModelId(0);
        let model = import::import_step(path, id, 1.0)?;
        let bbox = model.bbox();
        let auto_stock = self.state.session.stock_config().auto_from_model;
        if let Some(mesh) = &model.mesh
            && auto_stock
        {
            self.state.session.stock_mut().update_from_bbox(&mesh.bbox);
        }
        let sm = viz_model_to_session(&model);
        let assigned_id = self.state.session.add_model(sm);
        self.state.selection = Selection::Model(ModelId(assigned_id));
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
        let placeholder_id = ModelId(model_id.0);
        let new_model = import::import_model(&path, placeholder_id, kind, new_units)?;
        let bbox = new_model.bbox();
        let auto_stock = self.state.session.stock_config().auto_from_model;
        let mut stock_bbox_update: Option<rs_cam_core::geo::BoundingBox3> = None;
        if let Some(model) = self
            .state
            .session
            .models_mut()
            .iter_mut()
            .find(|m| m.id == model_id.0)
        {
            model.mesh = new_model.mesh.clone();
            model.polygons = new_model.polygons.clone();
            model.units = Some(new_model.units);
            model.winding_report = new_model.winding_report;
            if auto_stock {
                if let Some(mesh) = &model.mesh {
                    stock_bbox_update = Some(mesh.bbox);
                }
            }
        }
        if let Some(mesh_bbox) = stock_bbox_update {
            self.state.session.stock_mut().update_from_bbox(&mesh_bbox);
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

        let placeholder_id = ModelId(model_id.0);
        let reloaded = import::import_model(&path, placeholder_id, kind, units)?;

        if let Some(model) = self
            .state
            .session
            .models_mut()
            .iter_mut()
            .find(|m| m.id == model_id.0)
        {
            model.mesh = reloaded.mesh.clone();
            model.polygons = reloaded.polygons.clone();
            model.enriched_mesh = reloaded.enriched_mesh.clone();
            model.winding_report = reloaded.winding_report;
            model.load_error = reloaded.load_error.clone();
        }

        self.pending_upload = true;
        self.state.gui.mark_edited();
        Ok(())
    }

    pub fn save_job_to_path(&mut self, path: &Path) -> Result<(), VizError> {
        // Sync the viz post config into the session before saving.
        let session_post = GuiState::post_to_session(&self.state.gui.post);
        self.state.session.set_post_config(session_post);

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
                    let mut rt = ToolpathRuntime::new(true);
                    rt.stale_since = Some(loaded_at);
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
                        warning_messages.push(format!(
                            "Model '{}' could not be loaded because '{}' was not found.",
                            m.name,
                            m.path.display()
                        ));
                    }
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
                    let mut rt = ToolpathRuntime::new(tp.auto_regen);
                    rt.stale_since = Some(loaded_at);
                    gui.toolpath_rt.insert(tp.id.0, rt);
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
                rt.result.as_ref().map(|result| &*result.toolpath)
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
        crate::io::setup_sheet::generate_setup_sheet_from_session(
            &self.state.session,
            &self.state.gui,
        )
    }
}

// ── Legacy fallback: build session from viz JobState ─────────────────

/// Build a `ProjectSession` from a legacy-loaded `JobState`.
fn build_session_from_legacy_job(
    job: &crate::state::job::JobState,
) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_name(job.name.clone());
    session.set_stock_config(job.stock.clone());
    session.set_post_config(GuiState::post_to_session(&job.post));
    session.set_machine(job.machine.clone());
    session.replace_tools(job.tools.clone());

    let mut session_setups = Vec::new();
    let mut session_tp_configs = Vec::new();

    for setup in &job.setups {
        let mut tp_indices = Vec::new();
        for tp in &setup.toolpaths {
            let tp_index = session_tp_configs.len();
            tp_indices.push(tp_index);
            session_tp_configs.push(rs_cam_core::session::ToolpathConfig {
                id: tp.id.0,
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
                boundary_inherit: tp.boundary_inherit,
                stock_source: tp.stock_source,
                coolant: tp.coolant,
                face_selection: tp.face_selection.clone(),
                feeds_auto: tp.feeds_auto.clone(),
                debug_options: tp.debug_options,
            });
        }

        session_setups.push(rs_cam_core::session::SetupData {
            id: setup.id.0,
            name: setup.name.clone(),
            face_up: setup.face_up,
            z_rotation: setup.z_rotation,
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
        });
    }

    // Models
    for m in &job.models {
        let sm = viz_model_to_session(m);
        // Directly push to avoid re-assigning IDs
        session.models_mut().push(sm);
    }

    session.replace_setups_and_toolpaths(session_setups, session_tp_configs);
    session
}
