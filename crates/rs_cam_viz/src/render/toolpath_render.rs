use egui_wgpu::wgpu;
use rs_cam_core::toolpath::{MoveType, Toolpath};
use super::LineVertex;

/// Toolpath line data uploaded to GPU.
/// Vertices are in move-sequence order so partial drawing works for simulation scrubbing.
pub struct ToolpathGpuData {
    /// Cutting move vertices (line list, 2 verts per segment).
    pub cut_vertex_buffer: wgpu::Buffer,
    pub cut_vertex_count: u32,
    /// Rapid/link move vertices (line list, 2 verts per segment).
    pub rapid_vertex_buffer: wgpu::Buffer,
    pub rapid_vertex_count: u32,
    /// All moves interleaved in sequence order (for partial sim display).
    /// Each entry: (is_cutting, vertex_pair_index_in_respective_buffer).
    /// Used to compute how many cut/rapid verts to draw up to move N.
    pub move_cut_counts: Vec<u32>,
    pub move_rapid_counts: Vec<u32>,
}

impl ToolpathGpuData {
    /// Compute how many cut and rapid vertices to draw for the first `n_moves` moves.
    pub fn vertices_for_moves(&self, n_moves: usize) -> (u32, u32) {
        let n = n_moves.min(self.move_cut_counts.len());
        if n == 0 {
            return (0, 0);
        }
        (self.move_cut_counts[n - 1], self.move_rapid_counts[n - 1])
    }

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
        let mut move_cut_counts = Vec::new();
        let mut move_rapid_counts = Vec::new();

        fn z_color(z: f64, z_min: f64, z_range: f64) -> [f32; 3] {
            let t = ((z - z_min) / z_range).clamp(0.0, 1.0) as f32;
            [0.1 + t * 0.1, 0.3 + t * 0.6, 0.9 + t * 0.1]
        }

        for i in 1..tp.moves.len() {
            let from = tp.moves[i - 1].target;
            let to = tp.moves[i].target;
            let p0 = [from.x as f32, from.y as f32, from.z as f32];
            let p1 = [to.x as f32, to.y as f32, to.z as f32];

            match tp.moves[i].move_type {
                MoveType::Rapid => {
                    let color = [0.4, 0.3, 0.3];
                    rapid_verts.push(LineVertex { position: p0, color });
                    rapid_verts.push(LineVertex { position: p1, color });
                }
                _ => {
                    let c0 = z_color(from.z, z_min, z_range);
                    let c1 = z_color(to.z, z_min, z_range);
                    cut_verts.push(LineVertex { position: p0, color: c0 });
                    cut_verts.push(LineVertex { position: p1, color: c1 });
                }
            }

            move_cut_counts.push(cut_verts.len() as u32);
            move_rapid_counts.push(rapid_verts.len() as u32);
        }

        // Ensure non-empty buffers
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
            move_cut_counts,
            move_rapid_counts,
        }
    }
}
