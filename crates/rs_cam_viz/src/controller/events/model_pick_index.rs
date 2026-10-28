//! SEL-2: the GUI job that builds the ray-pick BVH of each mesh model.
//!
//! A ray pick on a mesh (a face click, the hover point, the drag anchor of
//! the orbit) calls `LoadedModel::ray_pick`. It runs on the BVH when the
//! pick table of `maps::geom_cache` holds one, else by brute force: the
//! same answer, only slower. A read on the frame loop never builds. This
//! module starts the `build_model_pick_index` `Job` on the Job lane (the
//! faces pattern: `submit_gui_job` stamps `AppState::job_run`,
//! `deliver_gui_job` clears it). The answer needs no upload, because no
//! draw changes.
//!
//! The pump builds for every mesh model, an STL too, and does not read
//! visibility: the orbit pivot and the hover point cast on a mesh outside a
//! pick mode, and `ctx.shows` lives in the app state of the viewport.

use crate::compute::ComputeBackend;

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    /// Start the pick-index job for the first mesh model, in session order,
    /// whose BVH is not built and whose job has not failed (SEL-2). Runs on
    /// every pump beside [`Self::process_model_faces`].
    ///
    /// One GUI job runs at a time; a model whose job failed is not tried
    /// again in this session, so a failure cannot loop.
    pub fn process_model_pick_index(&mut self) {
        if self.state.is_job_running() {
            return;
        }
        let Some(model_id) = self.model_needing_pick_index() else {
            return;
        };
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started = self.state.start_job(
            rs_cam_core::session::Job::BuildModelPickIndex(
                rs_cam_core::session::BuildModelPickIndexArgs { model_id },
            ),
            &cancel,
        );
        match started {
            Ok(handle) => self.submit_gui_job(
                handle,
                cancel,
                crate::controller::GuiJobTarget::ModelPickIndex { model_id },
            ),
            Err(error) => self.model_pick_index_failed(model_id, &error.to_string()),
        }
    }

    /// The first model in session order that needs a pick index, or `None`.
    fn model_needing_pick_index(&self) -> Option<rs_cam_core::ids::ModelId> {
        self.state
            .session()
            .models()
            .iter()
            .find(|m| {
                m.pick_mesh().is_some()
                    && !m.pick_index_ready()
                    && !self.failed_pick_index_models.contains(&m.id)
            })
            .map(|m| m.id)
    }

    /// Land the answer of a pick-index job. The pick table holds the BVH,
    /// so the next pick reads it; nothing is drawn again. A failure marks
    /// the model and says why.
    pub(crate) fn handle_model_pick_index_result(
        &mut self,
        model_id: rs_cam_core::ids::ModelId,
        result: Result<(), String>,
    ) {
        if let Err(error) = result {
            self.model_pick_index_failed(model_id, &error);
        }
    }

    fn model_pick_index_failed(&mut self, model_id: rs_cam_core::ids::ModelId, error: &str) {
        let _ = self.failed_pick_index_models.insert(model_id);
        self.push_notification(
            format!("Could not build the pick index of model {model_id}: {error}"),
            crate::controller::Severity::Warning,
        );
    }
}
