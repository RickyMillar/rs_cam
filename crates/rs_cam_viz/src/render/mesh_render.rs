use egui_wgpu::wgpu;
use rs_cam_core::mesh::TriangleMesh;

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
    pub fn from_mesh(device: &wgpu::Device, mesh: &TriangleMesh) -> Self {
        use wgpu::util::DeviceExt;

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

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }

    /// Upload a TriangleMesh with flat shading (per-face normals).
    ///
    /// Each triangle gets 3 dedicated vertices with the face normal, resulting
    /// in `3 * num_triangles` GPU vertices. Use `from_mesh` for the indexed
    /// smooth-normal path which uses ~3x less VRAM.
    #[allow(dead_code)]
    pub fn from_mesh_flat(device: &wgpu::Device, mesh: &TriangleMesh) -> Self {
        use wgpu::util::DeviceExt;

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

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }
}
