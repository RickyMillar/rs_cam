use std::path::Path;
use std::time::Instant;

use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::session::ProjectSession;

use crate::compute::ComputeBackend;
use crate::error::VizError;
use crate::io::import;
use crate::state::job::{
    Fixture, FixtureKind, JobState, KeepOutZone, LoadedModel, ModelId, ModelKind, ModelUnits,
    PostConfig, PostFormat, Setup, SetupId, ToolId,
};
use crate::state::selection::Selection;
use crate::state::simulation::SimulationState;
use crate::state::toolpath::{ToolpathEntry, ToolpathEntryInit, ToolpathId};

use super::AppController;

impl<B: ComputeBackend> AppController<B> {
    pub fn import_stl_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let id = self.state.job.next_model_id();
        let model = import::import_stl(path, id, 1.0)?;
        let bbox = model.bbox();
        if let Some(mesh) = &model.mesh
            && self.state.job.stock.auto_from_model
        {
            self.state.job.stock.update_from_bbox(&mesh.bbox);
        }
        self.state.selection = Selection::Model(model.id);
        self.state.job.models.push(model);
        self.state.job.dirty = true;
        self.pending_upload = true;
        Ok(bbox)
    }

    pub fn import_svg_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let id = self.state.job.next_model_id();
        let model = import::import_svg(path, id, 1.0)?;
        let bbox = model.bbox();
        self.state.selection = Selection::Model(model.id);
        self.state.job.models.push(model);
        self.state.job.dirty = true;
        self.pending_upload = true;
        Ok(bbox)
    }

    pub fn import_dxf_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let id = self.state.job.next_model_id();
        let model = import::import_dxf(path, id, 1.0)?;
        let bbox = model.bbox();
        self.state.selection = Selection::Model(model.id);
        self.state.job.models.push(model);
        self.state.job.dirty = true;
        self.pending_upload = true;
        Ok(bbox)
    }

    pub fn import_step_path(&mut self, path: &Path) -> Result<Option<BoundingBox3>, VizError> {
        let id = self.state.job.next_model_id();
        let model = import::import_step(path, id, 1.0)?;
        let bbox = model.bbox();
        if let Some(mesh) = &model.mesh
            && self.state.job.stock.auto_from_model
        {
            self.state.job.stock.update_from_bbox(&mesh.bbox);
        }
        self.state.selection = Selection::Model(model.id);
        self.state.job.models.push(model);
        self.state.job.dirty = true;
        self.pending_upload = true;
        Ok(bbox)
    }

    pub fn rescale_model(
        &mut self,
        model_id: crate::state::job::ModelId,
        new_units: crate::state::job::ModelUnits,
    ) -> Result<Option<BoundingBox3>, VizError> {
        let Some(model) = self
            .state
            .job
            .models
            .iter()
            .find(|model| model.id == model_id)
        else {
            return Ok(None);
        };
        if model.kind == crate::state::job::ModelKind::Step {
            return Ok(None);
        }
        let path = model.path.clone();
        let kind = model.kind;
        let new_model = import::import_model(&path, model_id, kind, new_units)?;
        let bbox = new_model.bbox();
        if let Some(model) = self
            .state
            .job
            .models
            .iter_mut()
            .find(|model| model.id == model_id)
        {
            model.mesh = new_model.mesh;
            model.polygons = new_model.polygons;
            model.units = new_model.units;
            model.winding_report = new_model.winding_report;
            if self.state.job.stock.auto_from_model
                && let Some(mesh) = &model.mesh
            {
                self.state.job.stock.update_from_bbox(&mesh.bbox);
            }
        }
        self.pending_upload = true;
        self.state.job.dirty = true;
        Ok(bbox)
    }

    pub fn reload_model(&mut self, model_id: crate::state::job::ModelId) -> Result<(), VizError> {
        let Some(model) = self
            .state
            .job
            .models
            .iter()
            .find(|model| model.id == model_id)
        else {
            return Err(VizError::Other(format!("Model {model_id:?} not found")));
        };

        let path = model.path.clone();
        let kind = model.kind;
        let units = model.units;

        let reloaded = import::import_model(&path, model_id, kind, units)?;

        if let Some(model) = self
            .state
            .job
            .models
            .iter_mut()
            .find(|model| model.id == model_id)
        {
            model.mesh = reloaded.mesh;
            model.polygons = reloaded.polygons;
            model.enriched_mesh = reloaded.enriched_mesh;
            model.winding_report = reloaded.winding_report;
            model.load_error = reloaded.load_error;
        }

        self.pending_upload = true;
        self.state.job.dirty = true;
        Ok(())
    }

    pub fn save_job_to_path(&mut self, path: &Path) -> Result<(), VizError> {
        crate::io::project::save_project(&self.state.job, path)?;
        self.state.job.file_path = Some(path.to_path_buf());
        self.state.job.dirty = false;
        // Keep the session in sync by reloading from the saved TOML.
        // This is simpler than a full sync-back function and ensures
        // the session always reflects the persisted state.
        match ProjectSession::load(path) {
            Ok(session) => self.session = Some(session),
            Err(e) => {
                tracing::warn!("Session reload after save failed: {e}");
            }
        }
        Ok(())
    }

    pub fn open_job_from_path(&mut self, path: &Path) -> Result<(), VizError> {
        // Try session-based loading first (unified path).
        match ProjectSession::load(path) {
            Ok(session) => {
                let (job, warning_messages) = build_job_from_session(&session, path);
                for message in &warning_messages {
                    tracing::warn!("{message}");
                }
                self.state.job = job;
                self.session = Some(session);
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
                // Fallback to the existing viz loader (handles legacy formats,
                // produces richer warnings, etc.).
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

                self.state.job = loaded.job;
                self.session = None;
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
        crate::io::export::export_gcode(&self.state.job)
    }

    pub fn export_svg_preview(&self) -> Result<String, VizError> {
        use rs_cam_core::viz::toolpath_to_svg;

        let toolpaths: Vec<_> = self
            .state
            .job
            .all_toolpaths()
            .filter(|toolpath| toolpath.enabled && toolpath.result.is_some())
            .filter_map(|toolpath| toolpath.result.as_ref().map(|result| &*result.toolpath))
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
        crate::io::setup_sheet::generate_setup_sheet(&self.state.job)
    }
}

// ── Session → JobState bridge ──────────────────────────────────────────

/// Build a `JobState` from a `ProjectSession`, mapping core session types
/// to GUI state types. This is the bridge between the unified session model
/// and the viz-specific `JobState`.
///
/// Returns the job and a list of warning messages (e.g. missing model files,
/// missing tool/model references).
fn build_job_from_session(session: &ProjectSession, path: &Path) -> (JobState, Vec<String>) {
    let mut job = JobState::new();
    let mut warnings = Vec::new();
    let loaded_at = Instant::now();

    // Project metadata
    job.name = session.name().to_owned();
    job.file_path = Some(path.to_path_buf());
    job.dirty = false;

    // Stock — same type, just clone
    job.stock = session.stock_config().clone();

    // Post — convert from ProjectPostConfig (string format) to PostConfig (enum format)
    let post_cfg = session.post_config();
    job.post = PostConfig {
        format: match post_cfg.format.to_ascii_lowercase().as_str() {
            "linuxcnc" => PostFormat::LinuxCnc,
            "mach3" => PostFormat::Mach3,
            _ => PostFormat::Grbl,
        },
        spindle_speed: post_cfg.spindle_speed,
        safe_z: post_cfg.safe_z,
        high_feedrate_mode: post_cfg.high_feedrate_mode,
        high_feedrate: post_cfg.high_feedrate,
    };

    // Machine — same type, just clone
    job.machine = session.machine().clone();

    // Tools — same type, just clone
    job.tools = session.tools().to_vec();

    // Models — map session::LoadedModel → viz's LoadedModel
    for m in session.models() {
        let has_geometry = m.mesh.is_some() || m.polygons.is_some();
        if !has_geometry {
            warnings.push(format!(
                "Model '{}' could not be loaded because '{}' was not found.",
                m.name,
                m.path.display()
            ));
        }
        job.models.push(LoadedModel {
            id: ModelId(m.id),
            path: m.path.clone(),
            name: m.name.clone(),
            kind: m.kind.unwrap_or(ModelKind::Stl),
            mesh: m.mesh.clone(),
            polygons: m.polygons.clone(),
            enriched_mesh: m.enriched_mesh.clone(),
            units: m.units.unwrap_or(ModelUnits::Millimeters),
            winding_report: None,
            load_error: if has_geometry {
                None
            } else {
                Some("Model file not found or failed to load".to_owned())
            },
        });
    }

    // Setups — map session::SetupData → viz's Setup, with toolpaths
    job.setups.clear();
    for setup_data in session.list_setups() {
        let mut setup = Setup::new(SetupId(setup_data.id), setup_data.name.clone());
        setup.face_up = setup_data.face_up;
        setup.z_rotation = setup_data.z_rotation;

        // Fixtures: map core session Fixture → viz Fixture
        setup.fixtures = setup_data
            .fixtures
            .iter()
            .map(|f| Fixture {
                id: f.id,
                name: f.name.clone(),
                kind: match f.kind {
                    rs_cam_core::session::FixtureKind::Clamp => FixtureKind::Clamp,
                    rs_cam_core::session::FixtureKind::Vise => FixtureKind::Vise,
                    rs_cam_core::session::FixtureKind::VacuumPod => FixtureKind::VacuumPod,
                    rs_cam_core::session::FixtureKind::Custom => FixtureKind::Custom,
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
            .collect();

        // Keep-out zones: map core session KeepOutZone → viz KeepOutZone
        setup.keep_out_zones = setup_data
            .keep_out_zones
            .iter()
            .map(|k| KeepOutZone {
                id: k.id,
                name: k.name.clone(),
                enabled: k.enabled,
                origin_x: k.origin_x,
                origin_y: k.origin_y,
                size_x: k.size_x,
                size_y: k.size_y,
            })
            .collect();

        // Toolpaths: map session ToolpathConfig → viz ToolpathEntry
        for &tp_idx in &setup_data.toolpath_indices {
            if let Some(tc) = session.toolpath_configs().get(tp_idx) {
                let tp_id = ToolpathId(tc.id);
                let tool_id = ToolId(tc.tool_id);
                let model_id = ModelId(tc.model_id);
                let name = if tc.name.is_empty() {
                    format!("{} {}", tc.operation.label(), tc.id + 1)
                } else {
                    tc.name.clone()
                };
                let mut init = ToolpathEntryInit::from_loaded_state(
                    tp_id,
                    name,
                    tool_id,
                    model_id,
                    tc.operation.clone(),
                );
                init.enabled = tc.enabled;
                init.dressups = tc.dressups.clone();
                init.heights = tc.heights.clone();
                init.boundary = tc.boundary.clone();
                init.boundary_inherit = tc.boundary_inherit;
                init.coolant = tc.coolant;
                init.pre_gcode = tc.pre_gcode.clone().unwrap_or_default();
                init.post_gcode = tc.post_gcode.clone().unwrap_or_default();
                init.stock_source = tc.stock_source;
                init.feeds_auto = tc.feeds_auto.clone();
                init.face_selection = tc.face_selection.clone();
                init.debug_options = tc.debug_options;

                let mut toolpath = ToolpathEntry::from_init(init);
                toolpath.clear_runtime_state();
                toolpath.stale_since = Some(loaded_at);

                // Warn about missing tool/model references
                if !job.tools.iter().any(|t| t.id == tool_id) {
                    warnings.push(format!(
                        "Toolpath '{}' references missing tool id {} and needs reassignment.",
                        toolpath.name, tool_id.0
                    ));
                }
                if !job.models.iter().any(|m| m.id == model_id) {
                    warnings.push(format!(
                        "Toolpath '{}' references missing model id {} and needs reassignment.",
                        toolpath.name, model_id.0
                    ));
                }

                setup.toolpaths.push(toolpath);
            }
        }

        job.setups.push(setup);
    }

    // If no setups were loaded, ensure the default setup exists
    if job.setups.is_empty() {
        job.setups.push(Setup::new(SetupId(0), "Setup 1".into()));
    }

    job.sync_next_ids();
    (job, warnings)
}
