//! The wireframe slot (FOLLOW_UPS §1, F132): the unique triangle edges of
//! every drawn mesh model, STEP and STL, as one `LineList` buffer in
//! `colors::MODEL_WIREFRAME`.
//!
//! The inputs come from the walk of `meshes::inputs`, so a hidden model and
//! an obstacle model draw no wireframe, as they draw no mesh. The pass hands
//! the inputs only in the `Wireframe` display mode; in any other mode the
//! slot clears, so a shaded viewport holds no wireframe buffer.

use std::sync::Arc;

use rs_cam_core::mesh::TriangleMesh;

use crate::render::upload_cache::{self, Refresh, SlotInputs};
use crate::render::{PolygonGpuData, RenderResources, gpu_safety, mesh_render};

use super::meshes::display_transform;
use super::{Gpu, PassFrame};

/// Everything `model_wireframe_lines` is built from.
pub(super) struct WireframeInputs<'a> {
    pub(super) frame: &'a PassFrame,
    /// The triangle mesh of each drawn model, in model order (the STEP
    /// models first, as `meshes::inputs` walks them).
    pub(super) meshes: Vec<&'a Arc<TriangleMesh>>,
}

impl SlotInputs for WireframeInputs<'_> {
    // The same identity as the plain mesh slot: the frame and the meshes.
    type Key = upload_cache::MeshUploadKey;

    fn key(&self) -> Self::Key {
        let Self { frame, meshes } = self;
        upload_cache::MeshUploadKey {
            frame: frame.key(),
            meshes: meshes.iter().map(|m| upload_cache::ArcId::new(m)).collect(),
        }
    }
}

/// Rebuild `model_wireframe_lines` when the key moved. `None` clears it.
pub(super) fn upload(
    inputs: Option<WireframeInputs<'_>>,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    let Refresh::Build(inputs) = upload_cache::refresh(&mut resources.model_wireframe_key, inputs)
    else {
        return;
    };
    resources.model_wireframe_lines = None;
    let Some(WireframeInputs { frame, meshes }) = inputs else {
        return;
    };
    let transform = display_transform(frame);
    let vertices: Vec<crate::render::LineVertex> = meshes
        .iter()
        .flat_map(|mesh| mesh_render::wireframe_vertices(mesh, &transform))
        .collect();
    if vertices.is_empty() {
        return;
    }
    resources.model_wireframe_lines = gpu_safety::try_create_buffer(
        gpu.device,
        &resources.gpu_limits,
        "model_wireframe_lines",
        bytemuck::cast_slice(&vertices),
        egui_wgpu::wgpu::BufferUsages::VERTEX,
    )
    .map(|vertex_buffer| PolygonGpuData {
        vertex_buffer,
        vertex_count: vertices.len() as u32,
    });
}
