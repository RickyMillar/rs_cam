//! Geometry selection P4.7: the GUI job that computes the flat faces of an
//! STL.
//!
//! An STL has no BREP faces; its faces are the flat regions of its mesh,
//! which core builds once per mesh into `maps::geom_cache`. A read on the
//! frame loop never builds (the WP11b rule): the `selection_status` memo
//! reports `faces_not_ready` ("computing faces") and a face click refuses
//! `FacesNotReady`. This module starts the `build_model_faces` `Job` on the
//! Job lane (the tier-map preview pattern: `submit_gui_job` stamps
//! `AppState::job_run`, `deliver_gui_job` clears it). When the job lands,
//! the cache holds the faces and the memo key moves, so the next read
//! counts them. The job answer itself carries only a count.
//!
//! Generation does not wait for this job: it builds the faces itself.

use crate::compute::ComputeBackend;
use crate::state::selection::Selection;

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    /// Start the faces job for each STL that the selected toolpath needs
    /// now (geometry selection P4.7). Runs on every pump beside
    /// [`Self::process_reach_overlay`].
    ///
    /// A model needs its faces when:
    ///
    /// - a selection of the selected toolpath (op or boundary) names a
    ///   face on it and the report says `faces_not_ready`, or
    /// - a pick mode is on for the toolpath and the toolpath model is an
    ///   STL whose faces are not computed (a face click needs them).
    ///
    /// One GUI job runs at a time; a model whose job failed is not tried
    /// again in this session, so a failure cannot loop.
    pub fn process_model_faces(&mut self) {
        if self.state.is_job_running() {
            return;
        }
        let Selection::Toolpath(id) = self.state.selection else {
            return;
        };
        let Some(model_id) = self.model_needing_faces(id) else {
            return;
        };
        self.request_model_faces(model_id);
    }

    /// The first model the toolpath `id` needs the faces of now, or `None`.
    fn model_needing_faces(&self, id: crate::state::toolpath::ToolpathId) -> Option<usize> {
        use rs_cam_core::session::SelectionTarget;
        let session = &self.state.session;
        let (_, tc) = session.find_toolpath_config_by_id(id)?;
        let failed = |m: &usize| self.failed_face_models.contains(m);
        for target in [SelectionTarget::Op, SelectionTarget::Boundary] {
            // Ask the status only of a selection that names a face: a
            // boundary that is not a selection would miss the memo and
            // refuse on every pump.
            let names_faces = tc
                .selection_of(target)
                .ok()
                .flatten()
                .is_some_and(rs_cam_core::selection::Selection::names_faces);
            if !names_faces {
                continue;
            }
            let Ok(report) = session.selection_status(tc, target) else {
                continue;
            };
            if let Some(model) = report
                .faces_not_ready
                .iter()
                .map(|m| m.0)
                .find(|m| !failed(m))
            {
                return Some(model);
            }
        }
        let picking = self.state.gui.selection_ui.pick_mode(id).is_some();
        let model = session.models().iter().find(|m| m.id == tc.model_id)?;
        (picking && !model.faces_ready() && !failed(&model.id)).then_some(model.id)
    }

    /// Submit the `build_model_faces` job for `model_id` on the Job lane.
    /// A refusal at step (i) marks the model failed and says why.
    pub(crate) fn request_model_faces(&mut self, model_id: usize) {
        if self.state.is_job_running() {
            return;
        }
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started = self.state.session.start(
            rs_cam_core::session::Job::BuildModelFaces(rs_cam_core::session::BuildModelFacesArgs {
                model_id,
            }),
            &cancel,
        );
        match started {
            Ok(handle) => self.submit_gui_job(
                handle,
                cancel,
                crate::controller::GuiJobTarget::ModelFaces { model_id },
            ),
            Err(error) => self.model_faces_failed(model_id, &error.to_string()),
        }
    }

    /// Land the answer of a faces job: the cache holds the faces, so the
    /// viewport and the section read them on the next frame. A failure
    /// marks the model and says why.
    pub(crate) fn handle_model_faces_result(
        &mut self,
        model_id: usize,
        result: Result<(), String>,
    ) {
        match result {
            Ok(()) => self.pending_upload = true,
            Err(error) => self.model_faces_failed(model_id, &error),
        }
    }

    fn model_faces_failed(&mut self, model_id: usize, error: &str) {
        let _ = self.failed_face_models.insert(model_id);
        self.push_notification(
            format!("Could not compute the flat faces of model {model_id}: {error}"),
            crate::controller::Severity::Warning,
        );
    }
}
