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

/// Generate entry point marker vertices for a toolpath.
/// Returns line vertices forming a small arrowhead at the first cutting move position,
/// pointing in the direction of the first cut.
/// `palette_color`: the toolpath's palette color.
pub fn entry_marker_vertices(tp: &Toolpath, palette_color: [f32; 3]) -> Vec<LineVertex> {
    // Find the first non-Rapid move at index > 0
    let first_cut_idx = match tp.moves.iter().enumerate().position(|(i, m)| {
        i > 0 && !matches!(m.move_type, MoveType::Rapid)
    }) {
        Some(idx) => idx,
        None => return Vec::new(),
    };

    let approach = tp.moves[first_cut_idx - 1].target;
    let entry = tp.moves[first_cut_idx].target;

    // Direction from approach point to first cut position
    let dx = entry.x - approach.x;
    let dy = entry.y - approach.y;
    let len = (dx * dx + dy * dy).sqrt();

    // If approach and entry are coincident in XY, try to use (1, 0) as default direction
    let (dir_x, dir_y) = if len < 1e-9 {
        (1.0, 0.0)
    } else {
        (dx / len, dy / len)
    };

    // Arrowhead size in mm
    let size: f64 = 2.0;
    let wing_len = size * 0.6;

    // Tip of the arrow is at the entry point; tail is behind it
    let tip = [entry.x as f32, entry.y as f32, entry.z as f32];
    let tail = [
        (entry.x - dir_x * size) as f32,
        (entry.y - dir_y * size) as f32,
        entry.z as f32,
    ];

    // Wing vectors: rotate direction by ±30 degrees, pointing backward from tip
    let angle = std::f64::consts::FRAC_PI_6; // 30 degrees
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    // Backward direction (from tip toward tail)
    let back_x = -dir_x;
    let back_y = -dir_y;

    // Left wing: rotate backward direction by +30 degrees
    let lw_x = back_x * cos_a - back_y * sin_a;
    let lw_y = back_x * sin_a + back_y * cos_a;
    let left_wing = [
        (entry.x + lw_x * wing_len) as f32,
        (entry.y + lw_y * wing_len) as f32,
        entry.z as f32,
    ];

    // Right wing: rotate backward direction by -30 degrees
    let rw_x = back_x * cos_a + back_y * sin_a;
    let rw_y = -back_x * sin_a + back_y * cos_a;
    let right_wing = [
        (entry.x + rw_x * wing_len) as f32,
        (entry.y + rw_y * wing_len) as f32,
        entry.z as f32,
    ];

    vec![
        // Center line: tail to tip
        LineVertex { position: tail, color: palette_color },
        LineVertex { position: tip, color: palette_color },
        // Left wing: tip to left wing end
        LineVertex { position: tip, color: palette_color },
        LineVertex { position: left_wing, color: palette_color },
        // Right wing: tip to right wing end
        LineVertex { position: tip, color: palette_color },
        LineVertex { position: right_wing, color: palette_color },
    ]
}
