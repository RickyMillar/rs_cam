//! The two model-mesh slots: the plain STL meshes and the face-coloured
//! STEP/BREP meshes (V8, F132).
//!
//! The two lists are keyed separately. `mesh_data_list` (plain STL,
//! `transform_mesh` + indexed smooth-normal upload — the 661k-triangle cost
//! on the reference workload) reads nothing selection- or hover-dependent,
//! so a face click or a pointer move over a STEP part cannot drag it along.
//! `enriched_mesh_data_list` bakes the selected/hovered face colours into
//! its vertices, so those two are in its key and nothing else's.
//!
//! Geometry identity is `upload_cache::ArcId` — a held `Weak`, not a raw
//! address. The cache holds no strong reference, so a freed allocation could
//! otherwise be reused at the same address by a replacement mesh; the
//! earlier mitigation paired the address with the element count, which only
//! made that coincidence require an identically sized replacement. A `Weak`
//! reserves the allocation outright, so the address cannot be reissued at
//! all.

use std::sync::Arc;

use rs_cam_core::geometry::enriched_mesh::{EnrichedMesh, FaceGroupId};
use rs_cam_core::mesh::TriangleMesh;

use crate::render::RenderResources;
use crate::render::mesh_render::MeshGpuData;
use crate::render::upload_cache::{self, Refresh, SlotInputs};
use crate::state::AppState;
use crate::state::job::transform_mesh;

use super::{Gpu, PassFrame};

/// Everything `mesh_data_list` is built from.
pub(super) struct MeshInputs<'a> {
    frame: &'a PassFrame,
    /// Each part model's plain mesh, in model order.
    meshes: Vec<&'a Arc<TriangleMesh>>,
}

impl SlotInputs for MeshInputs<'_> {
    type Key = upload_cache::MeshUploadKey;

    fn key(&self) -> Self::Key {
        let Self { frame, meshes } = self;
        upload_cache::MeshUploadKey {
            frame: frame.key(),
            meshes: meshes
                .iter()
                .map(|mesh| upload_cache::ArcId::new(mesh))
                .collect(),
        }
    }
}

/// Everything `enriched_mesh_data_list` is built from.
pub(super) struct EnrichedInputs<'a> {
    frame: &'a PassFrame,
    /// Each part model's enriched mesh, in model order.
    meshes: Vec<&'a Arc<EnrichedMesh>>,
    selected_faces: Vec<FaceGroupId>,
    hovered_face: Option<FaceGroupId>,
}

impl SlotInputs for EnrichedInputs<'_> {
    type Key = upload_cache::EnrichedUploadKey;

    fn key(&self) -> Self::Key {
        let Self {
            frame,
            meshes,
            selected_faces,
            hovered_face,
        } = self;
        upload_cache::EnrichedUploadKey {
            frame: frame.key(),
            meshes: meshes
                .iter()
                .map(|mesh| upload_cache::ArcId::new(mesh))
                .collect(),
            selected_faces: selected_faces.clone(),
            hovered_face: *hovered_face,
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
/// A model with an enriched mesh (STEP) takes the face-coloured path; any
/// other model with a mesh takes the plain path.
pub(super) fn inputs<'a>(
    state: &'a AppState,
    frame: &'a PassFrame,
    selected_faces: Vec<FaceGroupId>,
    hovered_face: Option<FaceGroupId>,
) -> (MeshInputs<'a>, EnrichedInputs<'a>) {
    let obstacle_model_ids: Vec<usize> = frame
        .setup_id()
        .and_then(|sid| {
            state
                .session
                .find_setup_by_id(sid.0)
                .map(|(_, sd)| sd.obstacles.clone())
        })
        .unwrap_or_default()
        .iter()
        .filter_map(|o| match o.geometry {
            rs_cam_core::session::ObstacleGeometry::Model { model_id } => Some(model_id.0),
            _ => None,
        })
        .collect();
    let model_visibility = &state.overlays.model_visibility;
    let mut plain = Vec::new();
    let mut enriched = Vec::new();
    for model in state
        .session
        .models()
        .iter()
        .filter(|m| !obstacle_model_ids.contains(&m.id) && model_visibility.is_visible(m.id))
    {
        if let Some(mesh) = &model.enriched_mesh {
            enriched.push(mesh);
        } else if let Some(mesh) = &model.mesh {
            plain.push(mesh);
        }
    }
    (
        MeshInputs {
            frame,
            meshes: plain,
        },
        EnrichedInputs {
            frame,
            meshes: enriched,
            selected_faces,
            hovered_face,
        },
    )
}

/// Refresh both slots. SHL-05: the one keyed guard, shared with the overlay
/// slots.
pub(super) fn upload(
    plain: MeshInputs<'_>,
    enriched: EnrichedInputs<'_>,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    let plain = upload_cache::refresh(&mut resources.mesh_upload_key, Some(plain));
    let enriched = upload_cache::refresh(&mut resources.enriched_upload_key, Some(enriched));
    if let Refresh::Build(inputs) = enriched {
        resources.enriched_mesh_data_list.clear();
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

fn build_enriched(inputs: EnrichedInputs<'_>, gpu: &Gpu<'_>, resources: &mut RenderResources) {
    let EnrichedInputs {
        frame,
        meshes,
        selected_faces,
        hovered_face,
    } = inputs;
    for enriched in meshes {
        let transform: crate::render::mesh_render::VertexTransform<'_> =
            frame.frame().map(|setup| {
                let stock = frame.stock();
                Box::new(move |p| setup.transform_point(p, stock))
                    as Box<dyn Fn(rs_cam_core::geo::P3) -> rs_cam_core::geo::P3 + '_>
            });
        if let Some(gpu_data) = crate::render::mesh_render::enriched_mesh_gpu_data(
            gpu.device,
            &resources.gpu_limits,
            enriched,
            &selected_faces,
            hovered_face,
            &transform,
        ) {
            resources.enriched_mesh_data_list.push(gpu_data);
        }
    }
}

fn build_plain(inputs: MeshInputs<'_>, gpu: &Gpu<'_>, resources: &mut RenderResources) {
    let MeshInputs { frame, meshes } = inputs;
    for mesh in meshes {
        let gpu_data = match frame.frame() {
            Some(setup) => {
                let transformed = transform_mesh(mesh, setup, frame.stock());
                MeshGpuData::from_mesh(gpu.device, &resources.gpu_limits, &transformed)
            }
            None => MeshGpuData::from_mesh(gpu.device, &resources.gpu_limits, mesh),
        };
        if let Some(gpu_data) = gpu_data {
            resources.mesh_data_list.push(gpu_data);
        }
    }
}
