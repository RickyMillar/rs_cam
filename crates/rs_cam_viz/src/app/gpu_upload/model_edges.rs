//! The face edge slot (D4, F132): the boundary loops of every drawn STEP
//! face, as one `LineList` buffer in `colors::MODEL_EDGE`.
//!
//! The inputs come from the walk of `meshes::inputs`, so a hidden model and
//! an obstacle model draw no edges, as they draw no face mesh. The key is
//! the frame and the meshes; a hover or a selection builds nothing here.
//! The buffer draws with `line_edge_bind_group`, which moves the edges
//! toward the eye (`render::EdgeOffset`).

use std::sync::Arc;

use rs_cam_core::geometry::enriched_mesh::EnrichedMesh;
use rs_cam_core::ids::ModelId;

use crate::render::upload_cache::{self, Refresh, SlotInputs};
use crate::render::{PolygonGpuData, RenderResources, gpu_safety, mesh_render};

use super::meshes::display_transform;
use super::{Gpu, PassFrame};

/// Everything `model_edge_lines` is built from.
pub(super) struct ModelEdgeInputs<'a> {
    pub(super) frame: &'a PassFrame,
    /// Each drawn STEP model's id and enriched mesh, in model order.
    pub(super) meshes: Vec<(ModelId, &'a Arc<EnrichedMesh>)>,
}

impl SlotInputs for ModelEdgeInputs<'_> {
    type Key = upload_cache::ModelEdgesKey;

    fn key(&self) -> Self::Key {
        let Self { frame, meshes } = self;
        upload_cache::ModelEdgesKey {
            frame: frame.key(),
            meshes: meshes
                .iter()
                .map(|(id, mesh)| (*id, upload_cache::ArcId::new(mesh)))
                .collect(),
        }
    }
}

/// Rebuild `model_edge_lines` when the key moved.
pub(super) fn upload(inputs: ModelEdgeInputs<'_>, gpu: &Gpu<'_>, resources: &mut RenderResources) {
    let Refresh::Build(inputs) =
        upload_cache::refresh(&mut resources.model_edges_key, Some(inputs))
    else {
        return;
    };
    resources.model_edge_lines = None;
    let Some(ModelEdgeInputs { frame, meshes }) = inputs else {
        return;
    };
    let transform = display_transform(frame);
    let vertices: Vec<crate::render::LineVertex> = meshes
        .iter()
        .flat_map(|(_, enriched)| mesh_render::face_edge_vertices(enriched, &transform))
        .collect();
    if vertices.is_empty() {
        return;
    }
    resources.model_edge_lines = gpu_safety::try_create_buffer(
        gpu.device,
        &resources.gpu_limits,
        "model_edge_lines",
        bytemuck::cast_slice(&vertices),
        egui_wgpu::wgpu::BufferUsages::VERTEX,
    )
    .map(|vertex_buffer| PolygonGpuData {
        vertex_buffer,
        vertex_count: vertices.len() as u32,
    });
}
