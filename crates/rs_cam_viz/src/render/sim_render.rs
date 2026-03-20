use egui_wgpu::wgpu;
use rs_cam_core::simulation::HeightmapMesh;

use super::mesh_render::MeshVertex;
use super::LineVertex;

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

/// Tool model visualization: a simple wireframe representation of the tool at a position.
/// Uses line segments to draw the tool outline (simpler than a mesh, no new pipeline needed).
pub struct ToolModelGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
}

impl ToolModelGpuData {
    /// Generate tool wireframe lines at the given position.
    /// `tool_radius`: radius of the cutting tool.
    /// `tool_length`: cutting length.
    /// `is_ball`: true for ball nose (hemisphere bottom), false for flat end.
    /// `position`: [x, y, z] of the tool tip.
    pub fn from_tool(device: &wgpu::Device, tool_radius: f32, tool_length: f32, is_ball: bool, position: [f32; 3]) -> Self {
        use wgpu::util::DeviceExt;

        let color = [0.8, 0.8, 0.3]; // yellow-ish tool color
        let segments = 24;
        let mut verts = Vec::new();

        let cx = position[0];
        let cy = position[1];
        let tip_z = position[2];
        let r = tool_radius;

        // Bottom circle (at tip for flat, at center of ball for ball nose)
        let bottom_z = if is_ball { tip_z + r } else { tip_z };
        for i in 0..segments {
            let a0 = std::f32::consts::TAU * (i as f32) / (segments as f32);
            let a1 = std::f32::consts::TAU * ((i + 1) as f32) / (segments as f32);
            verts.push(LineVertex { position: [cx + r * a0.cos(), cy + r * a0.sin(), bottom_z], color });
            verts.push(LineVertex { position: [cx + r * a1.cos(), cy + r * a1.sin(), bottom_z], color });
        }

        // Top circle (at top of cutting length)
        let top_z = tip_z + tool_length;
        for i in 0..segments {
            let a0 = std::f32::consts::TAU * (i as f32) / (segments as f32);
            let a1 = std::f32::consts::TAU * ((i + 1) as f32) / (segments as f32);
            verts.push(LineVertex { position: [cx + r * a0.cos(), cy + r * a0.sin(), top_z], color });
            verts.push(LineVertex { position: [cx + r * a1.cos(), cy + r * a1.sin(), top_z], color });
        }

        // Vertical lines connecting top and bottom (4 lines at 90-degree intervals)
        for i in 0..4 {
            let a = std::f32::consts::TAU * (i as f32) / 4.0;
            verts.push(LineVertex { position: [cx + r * a.cos(), cy + r * a.sin(), bottom_z], color });
            verts.push(LineVertex { position: [cx + r * a.cos(), cy + r * a.sin(), top_z], color });
        }

        // Ball nose hemisphere (arcs in XZ and YZ planes)
        if is_ball {
            for i in 0..segments {
                let a0 = std::f32::consts::PI * (i as f32) / (segments as f32);
                let a1 = std::f32::consts::PI * ((i + 1) as f32) / (segments as f32);
                // XZ arc
                verts.push(LineVertex { position: [cx + r * a0.sin(), cy, tip_z + r - r * a0.cos()], color });
                verts.push(LineVertex { position: [cx + r * a1.sin(), cy, tip_z + r - r * a1.cos()], color });
                // YZ arc
                verts.push(LineVertex { position: [cx, cy + r * a0.sin(), tip_z + r - r * a0.cos()], color });
                verts.push(LineVertex { position: [cx, cy + r * a1.sin(), tip_z + r - r * a1.cos()], color });
            }
        }

        if verts.is_empty() {
            verts.push(LineVertex { position: [0.0; 3], color: [0.0; 3] });
        }

        let vertex_count = verts.len() as u32;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tool_model"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self { vertex_buffer, vertex_count }
    }
}
