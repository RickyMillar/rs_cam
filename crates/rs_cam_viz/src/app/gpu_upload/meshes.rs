//! The two model-mesh slots: the plain STL meshes and the STEP/BREP face
//! pipeline meshes (V8, F132, D2).
//!
//! The two lists are keyed separately. `mesh_data_list` (plain STL,
//! `transform_mesh` + indexed smooth-normal upload — the 661k-triangle cost
//! on the reference workload) reads nothing selection- or hover-dependent.
//! `face_mesh_data_list` holds geometry and a face id per vertex, and no
//! colour. So no slot here reads the hover or the selection: those feed the
//! face state words (`face_state.rs`, D3), and a pointer move over a STEP
//! part builds no mesh.
//!
//! Geometry identity is `upload_cache::ArcId` — a held `Weak`, not a raw
//! address. The cache holds no strong reference, so a freed allocation could
//! otherwise be reused at the same address by a replacement mesh; the
//! earlier mitigation paired the address with the element count, which only
//! made that coincidence require an identically sized replacement. A `Weak`
//! reserves the allocation outright, so the address cannot be reissued at
//! all.

use std::sync::Arc;

use rs_cam_core::geometry::enriched_mesh::EnrichedMesh;
use rs_cam_core::ids::ModelId;
use rs_cam_core::mesh::TriangleMesh;

use crate::render::RenderResources;
use crate::render::mesh_render::MeshGpuData;
use crate::render::upload_cache::{self, Refresh, SlotInputs};
use crate::state::AppState;
use crate::state::job::transform_mesh;

use super::model_edges::ModelEdgeInputs;
use super::wireframe::WireframeInputs;
use super::{Gpu, PassFrame};

/// Everything `mesh_data_list` is built from.
pub(super) struct MeshInputs<'a> {
    frame: &'a PassFrame,
    /// Each part model's id and plain mesh, in model order. The id rides
    /// to the GPU data, so the reach overlay can skip its own model.
    meshes: Vec<(ModelId, &'a Arc<TriangleMesh>)>,
}

impl SlotInputs for MeshInputs<'_> {
    type Key = upload_cache::MeshUploadKey;

    fn key(&self) -> Self::Key {
        let Self { frame, meshes } = self;
        upload_cache::MeshUploadKey {
            frame: frame.key(),
            meshes: meshes
                .iter()
                // The id is not in the key: a mesh `Arc` belongs to one
                // model, so the pinned identity already names the model.
                .map(|(_, mesh)| upload_cache::ArcId::new(mesh))
                .collect(),
        }
    }
}

/// Everything `face_mesh_data_list` is built from.
pub(super) struct EnrichedInputs<'a> {
    frame: &'a PassFrame,
    /// Each part model's id and enriched mesh, in model order.
    meshes: Vec<(ModelId, &'a Arc<EnrichedMesh>)>,
}

impl SlotInputs for EnrichedInputs<'_> {
    type Key = upload_cache::EnrichedUploadKey;

    fn key(&self) -> Self::Key {
        let Self { frame, meshes } = self;
        upload_cache::EnrichedUploadKey {
            frame: frame.key(),
            meshes: meshes
                .iter()
                .map(|(id, mesh)| (*id, upload_cache::ArcId::new(mesh)))
                .collect(),
        }
    }
}

/// The inputs of both slots, read in one walk of the models.
///
/// K2 (D13): a model that an obstacle of the active setup reads as its mesh
/// is table-fixed, in the setup frame. The part pass moves every mesh with
/// the part (`transform_mesh`), so it would draw the obstacle in the wrong
/// place on a flipped setup. The obstacle prism (`obstacle_marks`) stands in
/// for it. A disabled obstacle's model is left out too: it is still
/// table-fixed.
///
/// P4.D: a model that the operator hid (the eye of its row) draws no mesh.
/// Its id leaves the mesh keys, so the hide is a key change.
///
/// A model with an enriched mesh (STEP) takes the face pipeline; any other
/// model with a mesh takes the plain path. D4: the face edges take the same
/// STEP models from this walk, so they draw for exactly the models that
/// draw a face mesh.
pub(super) fn inputs<'a>(
    state: &'a AppState,
    frame: &'a PassFrame,
) -> (
    MeshInputs<'a>,
    EnrichedInputs<'a>,
    ModelEdgeInputs<'a>,
    WireframeInputs<'a>,
) {
    let obstacle_model_ids: Vec<rs_cam_core::ids::ModelId> = frame
        .setup_id()
        .and_then(|sid| {
            state
                .session()
                .find_setup_by_id(sid)
                .map(|(_, sd)| sd.obstacles.clone())
        })
        .unwrap_or_default()
        .iter()
        .filter_map(|o| match o.geometry {
            rs_cam_core::session::ObstacleGeometry::Model { model_id } => Some(model_id),
            rs_cam_core::session::ObstacleGeometry::Box { .. }
            | rs_cam_core::session::ObstacleGeometry::OutlineExtrude { .. } => None,
        })
        .collect();
    let model_visibility = &state.overlays.model_visibility;
    let mut plain = Vec::new();
    let mut enriched = Vec::new();
    for model in state
        .session()
        .models()
        .iter()
        .filter(|m| !obstacle_model_ids.contains(&m.id) && model_visibility.is_visible(m.id))
    {
        if let Some(mesh) = &model.enriched_mesh {
            enriched.push((model.id, mesh));
        } else if let Some(mesh) = &model.mesh {
            plain.push((model.id, mesh));
        }
    }
    // FOLLOW_UPS §1: the wireframe takes the triangle mesh of every model
    // that draws a mesh here, STEP and STL, in model order.
    let wire = WireframeInputs {
        frame,
        meshes: enriched
            .iter()
            .map(|(_, mesh)| &mesh.mesh)
            .chain(plain.iter().map(|(_, mesh)| *mesh))
            .collect(),
    };
    (
        MeshInputs {
            frame,
            meshes: plain,
        },
        EnrichedInputs {
            frame,
            meshes: enriched.clone(),
        },
        ModelEdgeInputs {
            frame,
            meshes: enriched,
        },
        wire,
    )
}

/// Refresh both slots. SHL-05: the one keyed guard, shared with the overlay
/// slots. The face state slot (`face_state::upload`) runs after this and
/// borrows `enriched`, so this function takes it by reference.
pub(super) fn upload(
    plain: MeshInputs<'_>,
    enriched: &EnrichedInputs<'_>,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    let plain = upload_cache::refresh(&mut resources.mesh_upload_key, Some(plain));
    let enriched = upload_cache::refresh(&mut resources.enriched_upload_key, Some(enriched));
    if let Refresh::Build(inputs) = enriched {
        resources.face_mesh_data_list.clear();
        resources.upload_stats.enriched_builds += 1;
        if let Some(inputs) = inputs {
            build_enriched(inputs, gpu, resources);
        }
    }
    if let Refresh::Build(inputs) = plain {
        resources.mesh_data_list.clear();
        resources.upload_stats.mesh_builds += 1;
        if let Some(inputs) = inputs {
            build_plain(inputs, gpu, resources);
        }
    }
}

fn build_enriched(inputs: &EnrichedInputs<'_>, gpu: &Gpu<'_>, resources: &mut RenderResources) {
    let EnrichedInputs { frame, meshes } = inputs;
    for &(model_id, enriched) in meshes {
        let transform = display_transform(frame);
        if let Some(gpu_data) = crate::render::mesh_render::face_mesh_gpu_data(
            gpu.device,
            &resources.gpu_limits,
            &resources.face_state_bgl,
            &resources.face_palette_buffer,
            model_id,
            enriched,
            &transform,
        ) {
            resources.face_mesh_data_list.push(gpu_data);
        }
    }
}

fn build_plain(inputs: MeshInputs<'_>, gpu: &Gpu<'_>, resources: &mut RenderResources) {
    let MeshInputs { frame, meshes } = inputs;
    for (model_id, mesh) in meshes {
        let gpu_data = match frame.frame() {
            Some(setup) => {
                let transformed = transform_mesh(mesh, setup, frame.stock());
                MeshGpuData::from_mesh(gpu.device, &resources.gpu_limits, &transformed)
            }
            None => MeshGpuData::from_mesh(gpu.device, &resources.gpu_limits, mesh),
        };
        if let Some(mut gpu_data) = gpu_data {
            gpu_data.model_id = Some(model_id);
            resources.mesh_data_list.push(gpu_data);
        }
    }
}

/// The transform from the model frame to the display frame of this pass.
/// The face meshes and the face edges take the same one, so an edge sits on
/// its face.
pub(super) fn display_transform(
    frame: &PassFrame,
) -> crate::render::mesh_render::VertexTransform<'_> {
    frame.frame().map(|setup| {
        let stock = frame.stock();
        Box::new(move |p| setup.transform_point(p, stock))
            as Box<dyn Fn(rs_cam_core::geo::P3) -> rs_cam_core::geo::P3 + '_>
    })
}
