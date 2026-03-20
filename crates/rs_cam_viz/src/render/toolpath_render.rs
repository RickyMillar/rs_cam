use egui_wgpu::wgpu;
use rs_cam_core::toolpath::{MoveType, Toolpath};
use super::LineVertex;

/// Toolpath line data uploaded to GPU, split into cutting and rapid moves.
pub struct ToolpathGpuData {
    pub cut_vertex_buffer: wgpu::Buffer,
    pub cut_vertex_count: u32,
    pub rapid_vertex_buffer: wgpu::Buffer,
    pub rapid_vertex_count: u32,
}

impl ToolpathGpuData {
    /// Build GPU data from a Toolpath, coloring cuts by Z-depth.
    pub fn from_toolpath(device: &wgpu::Device, tp: &Toolpath) -> Self {
        use wgpu::util::DeviceExt;

        // Find Z range for coloring
        let mut z_min = f64::INFINITY;
        let mut z_max = f64::NEG_INFINITY;
        for m in &tp.moves {
            match m.move_type {
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                    z_min = z_min.min(m.target.z);
                    z_max = z_max.max(m.target.z);
                }
                _ => {}
            }
        }
        let z_range = (z_max - z_min).max(1e-6);

        let mut cut_verts = Vec::new();
        let mut rapid_verts = Vec::new();

        for i in 1..tp.moves.len() {
            let from = tp.moves[i - 1].target;
            let to = tp.moves[i].target;
            let p0 = [from.x as f32, from.y as f32, from.z as f32];
            let p1 = [to.x as f32, to.y as f32, to.z as f32];

            match tp.moves[i].move_type {
                MoveType::Rapid => {
                    let color = [0.4, 0.3, 0.3]; // dim red for rapids
                    rapid_verts.push(LineVertex { position: p0, color });
                    rapid_verts.push(LineVertex { position: p1, color });
                }
                _ => {
                    // Color by Z: low=blue, high=cyan
                    let t = ((to.z - z_min) / z_range).clamp(0.0, 1.0) as f32;
                    let color = [0.1 + t * 0.1, 0.3 + t * 0.6, 0.9 + t * 0.1];
                    rapid_verts.push(LineVertex {
                        position: p0,
                        color: {
                            let t0 = ((from.z - z_min) / z_range).clamp(0.0, 1.0) as f32;
                            [0.1 + t0 * 0.1, 0.3 + t0 * 0.6, 0.9 + t0 * 0.1]
                        },
                    });
                    // Actually use cut_verts, not rapid_verts:
                    cut_verts.push(LineVertex {
                        position: p0,
                        color: {
                            let t0 = ((from.z - z_min) / z_range).clamp(0.0, 1.0) as f32;
                            [0.1 + t0 * 0.1, 0.3 + t0 * 0.6, 0.9 + t0 * 0.1]
                        },
                    });
                    cut_verts.push(LineVertex { position: p1, color });
                }
            }
        }

        // Ensure non-empty buffers (wgpu doesn't like zero-size)
        if cut_verts.is_empty() {
            cut_verts.push(LineVertex { position: [0.0; 3], color: [0.0; 3] });
        }
        if rapid_verts.is_empty() {
            rapid_verts.push(LineVertex { position: [0.0; 3], color: [0.0; 3] });
        }

        let cut_vertex_count = cut_verts.len() as u32;
        let rapid_vertex_count = rapid_verts.len() as u32;

        let cut_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("toolpath_cut"),
            contents: bytemuck::cast_slice(&cut_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let rapid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("toolpath_rapid"),
            contents: bytemuck::cast_slice(&rapid_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            cut_vertex_buffer,
            cut_vertex_count,
            rapid_vertex_buffer,
            rapid_vertex_count,
        }
    }
}
