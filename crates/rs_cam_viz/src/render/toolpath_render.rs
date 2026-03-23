use super::LineVertex;
use super::gpu_safety::{self, GpuLimits};
use egui_wgpu::wgpu;
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// 8-color deterministic palette for per-toolpath coloring.
pub const TOOLPATH_PALETTE: [[f32; 3]; 8] = [
    [0.2, 0.5, 0.95],   // blue
    [0.2, 0.8, 0.3],    // green
    [0.95, 0.6, 0.15],  // orange
    [0.7, 0.3, 0.9],    // purple
    [0.9, 0.85, 0.2],   // yellow
    [0.2, 0.85, 0.85],  // cyan
    [0.95, 0.25, 0.25], // red
    [0.5, 0.9, 0.2],    // lime
];

/// Get the palette color for a toolpath at the given index.
#[allow(clippy::indexing_slicing)] // modulo indexing into constant-length palette
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
    /// Entry path preview lines (ramp/helix/lead-in indicator) for selected toolpaths.
    pub entry_preview_buffer: Option<wgpu::Buffer>,
    pub entry_preview_count: u32,
}

impl ToolpathGpuData {
    /// Compute how many cut and rapid vertices to draw for the first `n_moves` moves.
    #[allow(clippy::indexing_slicing)] // n - 1 is safe: n > 0 and n <= len
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
    ///
    /// Very large toolpaths are downsampled to fit within GPU buffer limits.
    /// The toolpath data itself is unchanged — only the visual representation
    /// is simplified.
    #[allow(clippy::indexing_slicing)] // loop index i bounded by tp.moves.len()
    pub fn from_toolpath(
        device: &wgpu::Device,
        limits: &GpuLimits,
        tp: &Toolpath,
        index: usize,
        selected: bool,
    ) -> Self {
        use wgpu::util::DeviceExt;

        // Use actual device limit with 6% headroom instead of hardcoded value.
        let max_buffer_bytes: usize = (limits.max_buffer_size as f64 * 0.94) as usize;
        let vertex_size: usize = std::mem::size_of::<LineVertex>(); // 24 bytes
        let max_verts: usize = max_buffer_bytes / vertex_size;

        // Estimate total vertices (2 per move for line-list).
        let total_moves = tp.moves.len().saturating_sub(1);
        // Downsample stride: show every Nth move if too many vertices.
        let stride = if total_moves * 2 > max_verts {
            (total_moves * 2 / max_verts) + 1
        } else {
            1
        };

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

        if stride > 1 {
            tracing::warn!(
                moves = total_moves,
                stride,
                "Toolpath too large for GPU buffer — downsampling for display"
            );
        }

        for i in 1..tp.moves.len() {
            // When downsampling, skip intermediate moves but always keep
            // rapid moves (they define retract/approach structure) and
            // the first/last moves.
            let keep = stride == 1
                || i % stride == 0
                || i == 1
                || i == tp.moves.len() - 1
                || tp.moves[i].move_type == MoveType::Rapid;

            if keep {
                let from = tp.moves[i - 1].target;
                let to = tp.moves[i].target;
                let p0 = [from.x as f32, from.y as f32, from.z as f32];
                let p1 = [to.x as f32, to.y as f32, to.z as f32];

                match tp.moves[i].move_type {
                    MoveType::Rapid => {
                        rapid_verts.push(LineVertex {
                            position: p0,
                            color: rapid_color,
                        });
                        rapid_verts.push(LineVertex {
                            position: p1,
                            color: rapid_color,
                        });
                    }
                    _ => {
                        let c0 = z_color(from.z);
                        let c1 = z_color(to.z);
                        cut_verts.push(LineVertex {
                            position: p0,
                            color: c0,
                        });
                        cut_verts.push(LineVertex {
                            position: p1,
                            color: c1,
                        });
                    }
                }
            }

            move_cut_counts.push(cut_verts.len() as u32);
            move_rapid_counts.push(rapid_verts.len() as u32);
        }

        // Ensure non-empty buffers
        if cut_verts.is_empty() {
            cut_verts.push(LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            });
        }
        if rapid_verts.is_empty() {
            rapid_verts.push(LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            });
        }

        let cut_vertex_count = cut_verts.len() as u32;
        let rapid_vertex_count = rapid_verts.len() as u32;

        // Use guarded buffer creation; fall back to placeholder on overflow
        // (shouldn't happen due to stride logic above, but defense-in-depth).
        let cut_vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "toolpath_cut",
            bytemuck::cast_slice(&cut_verts),
            wgpu::BufferUsages::VERTEX,
        )
        .unwrap_or_else(|| {
            let placeholder = [LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            }];
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("toolpath_cut_placeholder"),
                contents: bytemuck::cast_slice(&placeholder),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        let rapid_vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "toolpath_rapid",
            bytemuck::cast_slice(&rapid_verts),
            wgpu::BufferUsages::VERTEX,
        )
        .unwrap_or_else(|| {
            let placeholder = [LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            }];
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("toolpath_rapid_placeholder"),
                contents: bytemuck::cast_slice(&placeholder),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        Self {
            cut_vertex_buffer,
            cut_vertex_count,
            rapid_vertex_buffer,
            rapid_vertex_count,
            move_cut_counts,
            move_rapid_counts,
            entry_preview_buffer: None,
            entry_preview_count: 0,
        }
    }

    /// Attach entry path preview geometry for a selected toolpath.
    /// Call after `from_toolpath` to add the entry indicator overlay.
    pub fn attach_entry_preview(
        &mut self,
        device: &wgpu::Device,
        limits: &GpuLimits,
        verts: Vec<LineVertex>,
    ) {
        if verts.is_empty() {
            return;
        }
        self.entry_preview_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "entry_preview",
            bytemuck::cast_slice(&verts),
            wgpu::BufferUsages::VERTEX,
        );
        self.entry_preview_count = if self.entry_preview_buffer.is_some() {
            verts.len() as u32
        } else {
            0
        };
    }
}

/// Entry/exit style configuration passed from the toolpath dressup settings.
pub struct EntryPreviewConfig {
    pub entry_style: EntryStyle,
    pub ramp_angle_deg: f64,
    pub helix_radius: f64,
    pub helix_pitch: f64,
    pub lead_in_out: bool,
    pub lead_radius: f64,
    /// Resolved feed_z and top_z heights for the entry path.
    pub feed_z: f64,
    pub top_z: f64,
}

/// Simplified entry style enum (mirrors DressupEntryStyle without serde dependency).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStyle {
    None,
    Ramp,
    Helix,
}

/// Bright cyan color for entry preview lines.
const ENTRY_PREVIEW_COLOR: [f32; 3] = [0.2, 0.9, 0.9];

/// Generate schematic entry path preview vertices for a toolpath.
///
/// Draws a visual indicator of the configured entry strategy at the first plunge point:
/// - Ramp: a sloped line descending from feed_z to top_z at the configured ramp angle
/// - Helix: a helical spiral descending from feed_z to top_z at the first move position
/// - Lead-in arc: a quarter-circle arc leading into the first cutting direction
///
/// Returns empty if entry_style is None or there are no cutting moves.
#[allow(clippy::indexing_slicing)] // first_cut_idx validated by position() and bounds > 0
pub fn entry_preview_vertices(tp: &Toolpath, config: &EntryPreviewConfig) -> Vec<LineVertex> {
    let color = ENTRY_PREVIEW_COLOR;

    // Find the first non-Rapid move position (the first plunge/cut point)
    let first_cut_idx = match tp
        .moves
        .iter()
        .enumerate()
        .position(|(i, m)| i > 0 && !matches!(m.move_type, MoveType::Rapid))
    {
        Some(idx) => idx,
        None => return Vec::new(),
    };

    let entry_pos = tp.moves[first_cut_idx].target;
    let approach_pos = tp.moves[first_cut_idx - 1].target;
    let mut verts = Vec::new();

    let feed_z = config.feed_z;
    let top_z = config.top_z;
    let z_drop = (feed_z - top_z).abs();

    match config.entry_style {
        EntryStyle::None => {}
        EntryStyle::Ramp => {
            if z_drop > 0.01 {
                // Compute horizontal distance from ramp angle
                let angle_rad = config.ramp_angle_deg.to_radians().max(0.1_f64.to_radians());
                let horiz_dist = z_drop / angle_rad.tan();

                // Direction of approach in XY
                let dx = entry_pos.x - approach_pos.x;
                let dy = entry_pos.y - approach_pos.y;
                let len = (dx * dx + dy * dy).sqrt();
                let (dir_x, dir_y) = if len < 1e-9 {
                    (1.0, 0.0)
                } else {
                    (dx / len, dy / len)
                };

                // Ramp starts offset backward from entry point at feed_z
                let start = [
                    (entry_pos.x - dir_x * horiz_dist) as f32,
                    (entry_pos.y - dir_y * horiz_dist) as f32,
                    feed_z as f32,
                ];
                let end = [entry_pos.x as f32, entry_pos.y as f32, top_z as f32];

                verts.push(LineVertex {
                    position: start,
                    color,
                });
                verts.push(LineVertex {
                    position: end,
                    color,
                });
            }
        }
        EntryStyle::Helix => {
            if z_drop > 0.01 && config.helix_radius > 0.01 {
                let segments = 16;
                let cx = entry_pos.x;
                let cy = entry_pos.y;
                let r = config.helix_radius;

                // Helix descends from feed_z to top_z over one or more turns
                // Number of turns based on pitch
                let pitch = config.helix_pitch.max(0.1);
                let turns = z_drop / pitch;
                let total_angle = turns * std::f64::consts::TAU;

                for i in 0..segments {
                    let t0 = i as f64 / segments as f64;
                    let t1 = (i + 1) as f64 / segments as f64;

                    let a0 = t0 * total_angle;
                    let a1 = t1 * total_angle;
                    let z0 = feed_z - t0 * z_drop;
                    let z1 = feed_z - t1 * z_drop;

                    verts.push(LineVertex {
                        position: [
                            (cx + r * a0.cos()) as f32,
                            (cy + r * a0.sin()) as f32,
                            z0 as f32,
                        ],
                        color,
                    });
                    verts.push(LineVertex {
                        position: [
                            (cx + r * a1.cos()) as f32,
                            (cy + r * a1.sin()) as f32,
                            z1 as f32,
                        ],
                        color,
                    });
                }
            }
        }
    }

    // Lead-in arc: quarter-circle arc before the first cutting move
    if config.lead_in_out && config.lead_radius > 0.01 {
        let dx = entry_pos.x - approach_pos.x;
        let dy = entry_pos.y - approach_pos.y;
        let len = (dx * dx + dy * dy).sqrt();
        let (dir_x, dir_y) = if len < 1e-9 {
            (1.0, 0.0)
        } else {
            (dx / len, dy / len)
        };

        // Arc center is offset perpendicular to approach direction by lead_radius
        let perp_x = -dir_y;
        let perp_y = dir_x;
        let arc_cx = entry_pos.x + perp_x * config.lead_radius;
        let arc_cy = entry_pos.y + perp_y * config.lead_radius;
        let r = config.lead_radius;

        // Quarter-circle arc from tangent approach to entry point
        let arc_segments = 8;
        let start_angle = std::f64::consts::PI; // start opposite to perpendicular
        let sweep = std::f64::consts::FRAC_PI_2; // 90 degrees

        for i in 0..arc_segments {
            let t0 = i as f64 / arc_segments as f64;
            let t1 = (i + 1) as f64 / arc_segments as f64;
            let a0 = start_angle + t0 * sweep;
            let a1 = start_angle + t1 * sweep;

            let z = entry_pos.z; // lead-in at entry Z level
            verts.push(LineVertex {
                position: [
                    (arc_cx + r * a0.cos()) as f32,
                    (arc_cy + r * a0.sin()) as f32,
                    z as f32,
                ],
                color,
            });
            verts.push(LineVertex {
                position: [
                    (arc_cx + r * a1.cos()) as f32,
                    (arc_cy + r * a1.sin()) as f32,
                    z as f32,
                ],
                color,
            });
        }
    }

    verts
}

/// Generate entry point marker vertices for a toolpath.
/// Returns line vertices forming a small arrowhead at the first cutting move position,
/// pointing in the direction of the first cut.
/// `palette_color`: the toolpath's palette color.
#[allow(clippy::indexing_slicing)] // first_cut_idx validated by position() and bounds > 0
pub fn entry_marker_vertices(tp: &Toolpath, palette_color: [f32; 3]) -> Vec<LineVertex> {
    // Find the first non-Rapid move at index > 0
    let first_cut_idx = match tp
        .moves
        .iter()
        .enumerate()
        .position(|(i, m)| i > 0 && !matches!(m.move_type, MoveType::Rapid))
    {
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
        LineVertex {
            position: tail,
            color: palette_color,
        },
        LineVertex {
            position: tip,
            color: palette_color,
        },
        // Left wing: tip to left wing end
        LineVertex {
            position: tip,
            color: palette_color,
        },
        LineVertex {
            position: left_wing,
            color: palette_color,
        },
        // Right wing: tip to right wing end
        LineVertex {
            position: tip,
            color: palette_color,
        },
        LineVertex {
            position: right_wing,
            color: palette_color,
        },
    ]
}
