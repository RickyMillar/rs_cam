use egui_wgpu::wgpu;
use rs_cam_core::enriched_mesh::{EnrichedMesh, FaceGroupId};
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
/// If flat shading is ever needed, see `from_mesh_flat` which duplicates
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
    #[allow(clippy::indexing_slicing)] // vertex/triangle indices bounded by mesh invariants
    pub fn from_mesh(device: &wgpu::Device, limits: &GpuLimits, mesh: &TriangleMesh) -> Option<Self> {
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
                [0.0, 0.0, 1.0]
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

    /// Upload a TriangleMesh with flat shading (per-face normals).
    ///
    /// Each triangle gets 3 dedicated vertices with the face normal, resulting
    /// in `3 * num_triangles` GPU vertices. Use `from_mesh` for the indexed
    /// smooth-normal path which uses ~3x less VRAM.
    ///
    /// Returns `None` if the buffer exceeds GPU device limits.
    #[allow(dead_code, clippy::indexing_slicing)] // vertex/triangle indices bounded by mesh invariants
    pub fn from_mesh_flat(device: &wgpu::Device, limits: &GpuLimits, mesh: &TriangleMesh) -> Option<Self> {
        let mut vertices = Vec::with_capacity(mesh.triangles.len() * 3);
        let mut indices = Vec::with_capacity(mesh.triangles.len() * 3);

        for (i, tri) in mesh.triangles.iter().enumerate() {
            let v0 = mesh.vertices[tri[0] as usize];
            let v1 = mesh.vertices[tri[1] as usize];
            let v2 = mesh.vertices[tri[2] as usize];
            let n = mesh.faces[i].normal;

            let base = (i * 3) as u32;
            vertices.push(MeshVertex {
                position: [v0.x as f32, v0.y as f32, v0.z as f32],
                normal: [n.x as f32, n.y as f32, n.z as f32],
            });
            vertices.push(MeshVertex {
                position: [v1.x as f32, v1.y as f32, v1.z as f32],
                normal: [n.x as f32, n.y as f32, n.z as f32],
            });
            vertices.push(MeshVertex {
                position: [v2.x as f32, v2.y as f32, v2.z as f32],
                normal: [n.x as f32, n.y as f32, n.z as f32],
            });
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
        }

        let vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "mesh_vertices_flat",
            bytemuck::cast_slice(&vertices),
            wgpu::BufferUsages::VERTEX,
        )?;

        let index_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "mesh_indices_flat",
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

/// Deterministic pastel color for a face group ID.
#[allow(clippy::indexing_slicing)] // modulo indexing into constant-length palette
fn face_group_color(id: FaceGroupId) -> [f32; 3] {
    // Simple hash-to-color: spread face IDs across hue space
    const PALETTE: &[[f32; 3]] = &[
        [0.75, 0.85, 0.90], // light blue
        [0.85, 0.75, 0.90], // light purple
        [0.75, 0.90, 0.80], // light green
        [0.90, 0.85, 0.75], // light orange
        [0.90, 0.78, 0.80], // light pink
        [0.80, 0.90, 0.88], // light teal
        [0.88, 0.82, 0.75], // light tan
        [0.78, 0.82, 0.92], // periwinkle
        [0.88, 0.90, 0.75], // light yellow-green
        [0.82, 0.75, 0.85], // light mauve
    ];
    PALETTE[id.0 as usize % PALETTE.len()]
}

/// Build GPU data for an enriched mesh with per-face-group coloring.
///
/// Uses the `ColoredMeshVertex` pipeline (same as simulation stock rendering).
/// Selected faces get a highlight color, hovered face gets a hover tint,
/// and other faces get deterministic pastel colors.
/// Optional vertex transform applied during GPU upload.
pub type VertexTransform<'a> =
    Option<Box<dyn Fn(rs_cam_core::geo::P3) -> rs_cam_core::geo::P3 + 'a>>;

#[allow(clippy::indexing_slicing)] // vertex/triangle indices bounded by mesh invariants
pub fn enriched_mesh_gpu_data(
    device: &wgpu::Device,
    limits: &GpuLimits,
    enriched: &EnrichedMesh,
    selected_faces: &[FaceGroupId],
    hovered_face: Option<FaceGroupId>,
    transform: &VertexTransform<'_>,
) -> Option<EnrichedMeshGpuData> {
    let mesh = enriched.as_mesh();
    let highlight_color: [f32; 3] = [0.3, 0.5, 1.0]; // bright blue
    let hover_color: [f32; 3] = [0.4, 0.7, 0.85]; // soft cyan

    let mut vertices = Vec::with_capacity(mesh.triangles.len() * 3);
    let mut indices = Vec::with_capacity(mesh.triangles.len() * 3);

    for (tri_idx, tri) in mesh.triangles.iter().enumerate() {
        let face_id = enriched.face_for_triangle(tri_idx);
        let color = if selected_faces.contains(&face_id) {
            highlight_color
        } else if hovered_face == Some(face_id) {
            hover_color
        } else {
            face_group_color(face_id)
        };

        let n = mesh.faces[tri_idx].normal;
        let normal = [n.x as f32, n.y as f32, n.z as f32];

        let base = (tri_idx * 3) as u32;
        for &vi in tri {
            let v = mesh.vertices[vi as usize];
            let v = if let Some(xf) = transform { xf(v) } else { v };
            vertices.push(ColoredMeshVertex {
                position: [v.x as f32, v.y as f32, v.z as f32],
                normal,
                color,
            });
        }
        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 2);
    }

    let vertex_buffer = gpu_safety::try_create_buffer(
        device,
        limits,
        "enriched_mesh_vertices",
        bytemuck::cast_slice(&vertices),
        wgpu::BufferUsages::VERTEX,
    )?;

    let index_buffer = gpu_safety::try_create_buffer(
        device,
        limits,
        "enriched_mesh_indices",
        bytemuck::cast_slice(&indices),
        wgpu::BufferUsages::INDEX,
    )?;

    Some(EnrichedMeshGpuData {
        vertex_buffer,
        index_buffer,
        index_count: indices.len() as u32,
    })
}

/// GPU data for an enriched mesh with per-face coloring.
///
/// Uses the same `ColoredMeshVertex` layout as the simulation mesh pipeline,
/// so it can be rendered with `sim_mesh_pipeline`.
pub struct EnrichedMeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}
