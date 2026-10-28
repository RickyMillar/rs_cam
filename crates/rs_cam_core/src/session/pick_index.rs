//! The `build_model_pick_index` job (SEL-2): it builds the ray-pick BVH of
//! one mesh model off the frame loop and puts it in the pick table of
//! `maps::geom_cache`. Until the job lands, `LoadedModel::ray_pick` runs
//! brute force and gives the same answer, only slower.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{ProjectSession, SessionError};
use crate::ids::ModelId;
use crate::mesh::TriangleMesh;

/// What the `build_model_pick_index` job captured: the pick mesh of one
/// model. It owns the mesh, so [`execute_build_model_pick_index`] holds no
/// session and runs off the frame loop.
pub struct BuildModelPickIndexHandle {
    model_id: ModelId,
    mesh: Arc<TriangleMesh>,
}

impl BuildModelPickIndexHandle {
    /// The model id the job builds the pick index of.
    #[must_use]
    pub fn model_id(&self) -> ModelId {
        self.model_id
    }
}

/// The answer of the `build_model_pick_index` job. The BVH itself goes into
/// the pick table, which `LoadedModel::ray_pick` peeks; the answer only
/// names the model and the node count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPickIndexBuilt {
    pub model_id: ModelId,
    /// The node count of the BVH.
    pub nodes: usize,
}

impl ProjectSession {
    /// Step (i) of the `build_model_pick_index` job: capture the pick mesh
    /// of model `model_id` (`LoadedModel::pick_mesh`). It is a READ: it
    /// drops no result and moves no revision. A model with no mesh (a
    /// drawing) refuses.
    pub(crate) fn capture_build_model_pick_index(
        &self,
        model_id: ModelId,
    ) -> Result<BuildModelPickIndexHandle, SessionError> {
        let model = self
            .model_by_id(model_id)
            .ok_or_else(|| SessionError::model_not_found(model_id, self))?;
        let mesh = model.pick_mesh().ok_or_else(|| {
            SessionError::InvalidParam(format!(
                "Model {model_id} has no mesh; it has no pick index to build"
            ))
        })?;
        Ok(BuildModelPickIndexHandle {
            model_id,
            mesh: Arc::clone(mesh),
        })
    }
}

/// Step (ii) of the `build_model_pick_index` job: fill the pick table with
/// the BVH of the captured mesh (`maps::geom_cache::cached_bvh`). It reads
/// no session. A set cancel flag stops it before the build; the build
/// itself does not poll the flag.
pub fn execute_build_model_pick_index(
    handle: &BuildModelPickIndexHandle,
    cancel: &AtomicBool,
) -> Result<ModelPickIndexBuilt, SessionError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(SessionError::Operation(
            crate::compute::execute::OperationError::Cancelled,
        ));
    }
    let bvh = crate::maps::geom_cache::cached_bvh(&handle.mesh).ok_or_else(|| {
        SessionError::InvalidParam(format!(
            "Model {}: the mesh has too many triangles for the pick index",
            handle.model_id
        ))
    })?;
    Ok(ModelPickIndexBuilt {
        model_id: handle.model_id,
        nodes: bvh.node_count(),
    })
}
