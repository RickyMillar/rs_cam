use egui_wgpu::wgpu;
use rs_cam_core::toolpath::{MoveType, Toolpath};
use super::LineVertex;

/// 8-color deterministic palette for per-toolpath coloring.
pub const TOOLPATH_PALETTE: [[f32; 3]; 8] = [
    [0.2, 0.5, 0.95],  // blue
    [0.2, 0.8, 0.3],   // green
    [0.95, 0.6, 0.15],  // orange
    [0.7, 0.3, 0.9],   // purple
    [0.9, 0.85, 0.2],  // yellow
    [0.2, 0.85, 0.85], // cyan
    [0.95, 0.25, 0.25], // red
    [0.5, 0.9, 0.2],   // lime
];

/// Get the palette color for a toolpath at the given index.
pub fn palette_color(index: usize) -> [f32; 3] {
    TOOLPATH_PALETTE[index % TOOLPATH_PALETTE.len()]
}

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

    /// Build GPU data from a Toolpath, coloring by palette index with Z-depth blend.
    /// `index`: toolpath index for deterministic palette color.
    /// `selected`: if true, brighten the toolpath by +30%.
    pub fn from_toolpath(device: &wgpu::Device, tp: &Toolpath, index: usize, selected: bool) -> Self {
        use wgpu::util::DeviceExt;

        let base = palette_color(index);

        // Find Z range for depth blending
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

        // Blend palette color with Z-depth (30% depth influence)
        let z_color = |z: f64| -> [f32; 3] {
            let t = ((z - z_min) / z_range).clamp(0.0, 1.0) as f32;
            let depth_factor = 0.7 + t * 0.3; // darker at bottom, brighter at top
            let mut c = [
                base[0] * depth_factor,
                base[1] * depth_factor,
                base[2] * depth_factor,
            ];
            if selected {
                c[0] = (c[0] * 1.3).min(1.0);
                c[1] = (c[1] * 1.3).min(1.0);
                c[2] = (c[2] * 1.3).min(1.0);
            }
            c
        };

        // Dimmed version of palette color for rapids
        let rapid_color = {
            let dim = 0.35;
            let mut c = [base[0] * dim, base[1] * dim, base[2] * dim];
            if selected {
                c[0] = (c[0] * 1.3).min(1.0);
                c[1] = (c[1] * 1.3).min(1.0);
                c[2] = (c[2] * 1.3).min(1.0);
            }
            c
        };

        for i in 1..tp.moves.len() {
            let from = tp.moves[i - 1].target;
            let to = tp.moves[i].target;
            let p0 = [from.x as f32, from.y as f32, from.z as f32];
            let p1 = [to.x as f32, to.y as f32, to.z as f32];

            match tp.moves[i].move_type {
                MoveType::Rapid => {
                    rapid_verts.push(LineVertex { position: p0, color: rapid_color });
                    rapid_verts.push(LineVertex { position: p1, color: rapid_color });
                }
                _ => {
                    let c0 = z_color(from.z);
                    let c1 = z_color(to.z);
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
