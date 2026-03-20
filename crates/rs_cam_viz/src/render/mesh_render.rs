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
pub struct MeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl MeshGpuData {
    /// Upload a TriangleMesh to the GPU with flat shading (per-face normals).
    pub fn from_mesh(device: &wgpu::Device, mesh: &TriangleMesh) -> Self {
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
