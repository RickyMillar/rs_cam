use std::path::Path;

use rs_cam_core::geo::BoundingBox3;

use crate::compute::ComputeBackend;
use crate::error::VizError;
use crate::io::import;
use crate::state::selection::Selection;
use crate::state::simulation::SimulationState;

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
        Ok(())
    }

    pub fn open_job_from_path(&mut self, path: &Path) -> Result<(), VizError> {
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
        self.state.selection = Selection::None;
        self.state.simulation = SimulationState::new();
        self.collision_positions.clear();
        self.pending_upload = true;
        self.load_warnings = warning_messages;
        self.show_load_warnings = !self.load_warnings.is_empty();
        Ok(())
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
