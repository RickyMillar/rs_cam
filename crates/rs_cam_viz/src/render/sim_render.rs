use egui_wgpu::wgpu;
use rs_cam_core::simulation::HeightmapMesh;

use super::mesh_render::MeshVertex;

/// Simulation result mesh uploaded to GPU.
pub struct SimMeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl SimMeshGpuData {
    /// Upload a HeightmapMesh (from heightmap_to_mesh) to the GPU.
    /// The HeightmapMesh has flat arrays: vertices [x,y,z,...], colors [r,g,b,...], indices [i0,i1,i2,...].
    pub fn from_heightmap_mesh(device: &wgpu::Device, hm: &HeightmapMesh) -> Self {
        use wgpu::util::DeviceExt;

        let num_verts = hm.vertices.len() / 3;
        let mut mesh_verts = Vec::with_capacity(num_verts);

        for i in 0..num_verts {
            let vx = hm.vertices[i * 3];
            let vy = hm.vertices[i * 3 + 1];
            let vz = hm.vertices[i * 3 + 2];

            // Compute normal from adjacent triangles (approximate with face normal from first triangle)
            // For heightmap meshes, a simple up-ish normal works; we'll compute per-face below
            mesh_verts.push(MeshVertex {
                position: [vx, vy, vz],
                normal: [0.0, 0.0, 1.0], // placeholder, overwritten below
            });
        }

        // Compute per-face normals and accumulate to vertices
        let num_tris = hm.indices.len() / 3;
        let mut normals = vec![[0.0f32; 3]; num_verts];

        for t in 0..num_tris {
            let i0 = hm.indices[t * 3] as usize;
            let i1 = hm.indices[t * 3 + 1] as usize;
            let i2 = hm.indices[t * 3 + 2] as usize;

            let v0 = &mesh_verts[i0].position;
            let v1 = &mesh_verts[i1].position;
            let v2 = &mesh_verts[i2].position;

            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];

            for idx in [i0, i1, i2] {
                normals[idx][0] += n[0];
                normals[idx][1] += n[1];
                normals[idx][2] += n[2];
            }
        }

        // Normalize and assign
        for (i, n) in normals.iter().enumerate() {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len > 1e-8 {
                mesh_verts[i].normal = [n[0] / len, n[1] / len, n[2] / len];
            }
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sim_mesh_vertices"),
            contents: bytemuck::cast_slice(&mesh_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sim_mesh_indices"),
            contents: bytemuck::cast_slice(&hm.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count: hm.indices.len() as u32,
        }
    }
}
