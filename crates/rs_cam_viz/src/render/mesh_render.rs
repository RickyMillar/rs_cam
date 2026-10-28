use egui_wgpu::wgpu;
use rs_cam_core::geometry::enriched_mesh::EnrichedMesh;
use rs_cam_core::mesh::TriangleMesh;

use super::gpu_safety::{self, GpuLimits};
use super::sim_render::ColoredMeshVertex;

/// GPU vertex for mesh rendering.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl MeshVertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeshVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

/// Mesh data uploaded to the GPU.
///
/// Uses indexed rendering with smooth vertex normals. Each unique vertex in the
/// source mesh is uploaded once with an area-weighted average normal, and the
/// mesh's triangle index buffer is uploaded directly. This gives ~3x VRAM
/// savings compared to duplicating vertices for per-face flat normals.
///
/// **Shading note**: the WGSL shader already interpolates normals smoothly
/// across each triangle, so smooth vertex normals produce correct Phong shading.
/// If flat shading is ever needed, it means three dedicated vertices per
/// triangle carrying the face normal — roughly 3x the VRAM of this indexed
/// smooth-normal path. A `from_mesh_flat` that did exactly that shipped with
/// zero callers and an `#[allow(dead_code)]`, and P6 deleted it
/// vertices to assign per-face normals.
pub struct MeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl MeshGpuData {
    /// Upload a TriangleMesh with indexed smooth-normal rendering.
    ///
    /// Uploads each unique vertex once with an area-weighted average normal,
    /// then uses the mesh's native triangle indices directly. This reduces
    /// vertex count from `3 * num_triangles` to `num_vertices` (typically ~3x
    /// smaller for typical meshes).
    ///
    /// Returns `None` if the buffer exceeds GPU device limits.
    #[expect(
        clippy::indexing_slicing,
        reason = "vertex/triangle indices bounded by mesh invariants"
    )]
    pub fn from_mesh(
        device: &wgpu::Device,
        limits: &GpuLimits,
        mesh: &TriangleMesh,
    ) -> Option<Self> {
        let num_verts = mesh.vertices.len();

        // Accumulate area-weighted face normals per vertex.
        let mut normals = vec![[0.0f32; 3]; num_verts];
        for (i, tri) in mesh.triangles.iter().enumerate() {
            let n = mesh.faces[i].normal;
            let nf = [n.x as f32, n.y as f32, n.z as f32];
            for &vi in tri {
                let slot = &mut normals[vi as usize];
                slot[0] += nf[0];
                slot[1] += nf[1];
                slot[2] += nf[2];
            }
        }

        // Normalize accumulated normals and build vertex array.
        let mut vertices = Vec::with_capacity(num_verts);
        for (i, v) in mesh.vertices.iter().enumerate() {
            let n = &normals[i];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let normal = if len > 1e-8 {
                [n[0] / len, n[1] / len, n[2] / len]
            } else {
                UP_NORMAL
            };
            vertices.push(MeshVertex {
                position: [v.x as f32, v.y as f32, v.z as f32],
                normal,
            });
        }

        // Flatten triangle indices to a flat u32 array for the GPU.
        let indices: Vec<u32> = mesh
            .triangles
            .iter()
            .flat_map(|tri| tri.iter().copied())
            .collect();

        let vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "mesh_vertices",
            bytemuck::cast_slice(&vertices),
            wgpu::BufferUsages::VERTEX,
        )?;

        let index_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "mesh_indices",
            bytemuck::cast_slice(&indices),
            wgpu::BufferUsages::INDEX,
        )?;

        Some(Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        })
    }
}

/// Upload a `TriangleMesh` with one caller-supplied colour per vertex (P5).
///
/// [`MeshGpuData::from_mesh`]'s construction exactly — the same area-weighted
/// smooth normals and the same native index buffer — but emitting
/// [`ColoredMeshVertex`], so the result draws with `colored_opaque_pipeline`
/// instead of the plain mesh pipeline whose diffuse colour is a WGSL literal.
///
/// Returns `None` when `colors` does not carry exactly one entry per mesh
/// vertex, and when a buffer exceeds the device limits. A length mismatch is
/// a caller error — the colours are indexed by vertex — so it refuses rather
/// than padding, which would mis-colour the part silently.
#[expect(
    clippy::indexing_slicing,
    reason = "vertex/triangle indices bounded by mesh invariants"
)]
pub fn colored_mesh_gpu_data(
    device: &wgpu::Device,
    limits: &GpuLimits,
    mesh: &TriangleMesh,
    colors: &[[f32; 3]],
) -> Option<EnrichedMeshGpuData> {
    let num_verts = mesh.vertices.len();
    if colors.len() != num_verts {
        return None;
    }

    // Accumulate area-weighted face normals per vertex.
    let mut normals = vec![[0.0f32; 3]; num_verts];
    for (i, tri) in mesh.triangles.iter().enumerate() {
        let n = mesh.faces[i].normal;
        let nf = [n.x as f32, n.y as f32, n.z as f32];
        for &vi in tri {
            let slot = &mut normals[vi as usize];
            slot[0] += nf[0];
            slot[1] += nf[1];
            slot[2] += nf[2];
        }
    }

    let mut vertices = Vec::with_capacity(num_verts);
    for (i, v) in mesh.vertices.iter().enumerate() {
        let n = &normals[i];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let normal = if len > 1e-8 {
            [n[0] / len, n[1] / len, n[2] / len]
        } else {
            UP_NORMAL
        };
        vertices.push(ColoredMeshVertex {
            position: [v.x as f32, v.y as f32, v.z as f32],
            normal,
            color: colors[i],
        });
    }

    let indices: Vec<u32> = mesh
        .triangles
        .iter()
        .flat_map(|tri| tri.iter().copied())
        .collect();

    let vertex_buffer = gpu_safety::try_create_buffer(
        device,
        limits,
        "reach_overlay_vertices",
        bytemuck::cast_slice(&vertices),
        wgpu::BufferUsages::VERTEX,
    )?;

    let index_buffer = gpu_safety::try_create_buffer(
        device,
        limits,
        "reach_overlay_indices",
        bytemuck::cast_slice(&indices),
        wgpu::BufferUsages::INDEX,
    )?;

    Some(EnrichedMeshGpuData {
        vertex_buffer,
        index_buffer,
        index_count: indices.len() as u32,
    })
}

/// The normal of a degenerate vertex or triangle: straight up.
const UP_NORMAL: [f32; 3] = [0.0, 0.0, 1.0];

/// A transform that the upload applies to each vertex position (the setup
/// frame). `None` keeps the model frame.
pub type VertexTransform<'a> =
    Option<Box<dyn Fn(rs_cam_core::geo::P3) -> rs_cam_core::geo::P3 + 'a>>;

/// GPU vertex of the face pipeline (STEP / BREP models, D2).
///
/// `face_id` is the `FaceGroupId` of the face that owns the vertex. The
/// fragment stage reads the colour of that face from the per-face state
/// buffer and the palette, so a hover or a selection change rewrites the
/// state words and never this buffer.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FaceMeshVertex {
    pub position: [f32; 3],
    /// The area-weighted normal of the triangles of one face.
    pub normal: [f32; 3],
    pub face_id: u32,
}

impl FaceMeshVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Uint32];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<FaceMeshVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

/// GPU data of one STEP model for the face pipeline.
///
/// The geometry buffers hold no colour. `face_state` holds one `u32` per
/// face (`app/gpu_upload/face_state.rs` writes it); `state_bind_group` binds
/// it with the palette as group 1 of `face_mesh_pipeline`.
pub struct FaceMeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    /// The number of faces, and of words in `face_state`.
    pub face_count: u32,
    /// One state word per face (`STORAGE | COPY_DST`).
    pub face_state: wgpu::Buffer,
    /// Group 1: `face_state` and the palette uniform.
    pub state_bind_group: wgpu::BindGroup,
    pub model_id: rs_cam_core::ids::ModelId,
}

/// The vertices and indices of the face pipeline for `enriched`.
///
/// The index buffer is the mesh's own `triangles`. Core gives each face its
/// own vertex list (`build_enriched_mesh`), so no vertex belongs to two
/// faces: the normals are smooth inside a face and a crease between two
/// faces stays sharp. The normal of a vertex is the sum of the cross
/// products of its triangles (area-weighted), taken after the transform,
/// so the normal is in the drawn frame.
#[expect(
    clippy::indexing_slicing,
    reason = "vertex/triangle indices bounded by mesh invariants"
)]
pub fn face_mesh_vertices(
    enriched: &EnrichedMesh,
    transform: &VertexTransform<'_>,
) -> (Vec<FaceMeshVertex>, Vec<u32>) {
    let mesh = enriched.as_mesh();
    let positions: Vec<[f32; 3]> = mesh
        .vertices
        .iter()
        .map(|&v| {
            let v = if let Some(xf) = transform { xf(v) } else { v };
            [v.x as f32, v.y as f32, v.z as f32]
        })
        .collect();
    let mut normals = vec![[0.0f32; 3]; positions.len()];
    let mut face_ids = vec![0u32; positions.len()];
    for (t, tri) in mesh.triangles.iter().enumerate() {
        let [a, b, c] = tri.map(|i| positions[i as usize]);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * w[2] - u[2] * w[1],
            u[2] * w[0] - u[0] * w[2],
            u[0] * w[1] - u[1] * w[0],
        ];
        let face = u32::from(enriched.face_for_triangle(t).0);
        for &vi in tri {
            let slot = &mut normals[vi as usize];
            slot[0] += n[0];
            slot[1] += n[1];
            slot[2] += n[2];
            face_ids[vi as usize] = face;
        }
    }
    let vertices = positions
        .iter()
        .zip(&normals)
        .zip(&face_ids)
        .map(|((&position, n), &face_id)| {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let normal = if len > 1e-12 {
                [n[0] / len, n[1] / len, n[2] / len]
            } else {
                UP_NORMAL
            };
            FaceMeshVertex {
                position,
                normal,
                face_id,
            }
        })
        .collect();
    let indices = mesh
        .triangles
        .iter()
        .flat_map(|tri| tri.iter().copied())
        .collect();
    (vertices, indices)
}

/// Build the face-pipeline GPU data of one STEP model: the geometry and a
/// zeroed state buffer. It takes no selection and no hover input.
///
/// `layout` is `face_state_bgl` and `palette` is the palette uniform of
/// `RenderResources`. Returns `None` when a buffer exceeds the device
/// limits.
pub fn face_mesh_gpu_data(
    device: &wgpu::Device,
    limits: &GpuLimits,
    layout: &wgpu::BindGroupLayout,
    palette: &wgpu::Buffer,
    model_id: rs_cam_core::ids::ModelId,
    enriched: &EnrichedMesh,
    transform: &VertexTransform<'_>,
) -> Option<FaceMeshGpuData> {
    let (vertices, indices) = face_mesh_vertices(enriched, transform);
    let vertex_buffer = gpu_safety::try_create_buffer(
        device,
        limits,
        "face_mesh_vertices",
        bytemuck::cast_slice(&vertices),
        wgpu::BufferUsages::VERTEX,
    )?;
    let index_buffer = gpu_safety::try_create_buffer(
        device,
        limits,
        "face_mesh_indices",
        bytemuck::cast_slice(&indices),
        wgpu::BufferUsages::INDEX,
    )?;
    // A storage binding cannot be empty, so a mesh with no face still gets
    // one word.
    let face_count = enriched.face_groups.len() as u32;
    let words = vec![0u32; enriched.face_groups.len().max(1)];
    let face_state = gpu_safety::try_create_buffer(
        device,
        limits,
        "face_state",
        bytemuck::cast_slice(&words),
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    )?;
    let state_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("face_state_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: face_state.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: palette.as_entire_binding(),
            },
        ],
    });
    Some(FaceMeshGpuData {
        vertex_buffer,
        index_buffer,
        index_count: indices.len() as u32,
        face_count,
        face_state,
        state_bind_group,
        model_id,
    })
}

/// GPU data of an indexed `ColoredMeshVertex` mesh: the reach overlay
/// ([`colored_mesh_gpu_data`]). It draws with `colored_opaque_pipeline`.
pub struct EnrichedMeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The VRAM of the face path: 28 B per vertex, 12 B of index per
    /// triangle (D2 §2.1).
    #[test]
    fn a_face_vertex_is_28_bytes() {
        assert_eq!(std::mem::size_of::<FaceMeshVertex>(), 28);
        assert_eq!(FaceMeshVertex::layout().array_stride, 28);
        assert_eq!(std::mem::size_of::<[u32; 3]>(), 12);
    }

    /// Two faces meet at a right-angle crease. Each vertex takes the id of
    /// its own face and the normal of its own face, so the crease stays
    /// sharp: the shared edge has two vertex pairs with two normals.
    #[test]
    fn each_face_keeps_its_own_ids_and_normals() {
        use rs_cam_core::geo::P3;
        let p = |x: f64, y: f64, z: f64| P3::new(x, y, z);
        // Face 0 lies in z = 0; face 1 stands in x = 0. Both hold the edge
        // from (0, 0, 0) to (0, 1, 0), each with its own copy.
        let vertices = vec![
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(0.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(0.0, 0.0, 1.0),
        ];
        let mesh = TriangleMesh::from_raw(vertices, vec![[0, 1, 2], [3, 4, 5]]);
        let enriched = EnrichedMesh {
            mesh: std::sync::Arc::new(mesh),
            face_groups: Vec::new(),
            triangle_to_face: vec![0, 1],
            adjacency: Vec::new(),
            edges: Vec::new(),
        };
        let (vertices, indices) = face_mesh_vertices(&enriched, &None);
        assert_eq!(indices, vec![0, 1, 2, 3, 4, 5]);
        let ids: Vec<u32> = vertices.iter().map(|v| v.face_id).collect();
        assert_eq!(ids, vec![0, 0, 0, 1, 1, 1]);
        for v in vertices.iter().take(3) {
            assert_eq!(v.normal, [0.0, 0.0, 1.0]);
        }
        for v in vertices.iter().skip(3) {
            assert_eq!(v.normal, [1.0, 0.0, 0.0]);
        }
    }
}
