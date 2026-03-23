use egui_wgpu::wgpu;
use rs_cam_core::stock_mesh::StockMesh;

use super::gpu_safety::{self, GpuLimits};
use super::LineVertex;

/// GPU vertex for per-vertex colored mesh rendering (simulation stock).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColoredMeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

impl ColoredMeshVertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ColoredMeshVertex>() as u64,
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
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

/// Simulation result mesh uploaded to GPU.
///
/// Includes a generation counter so callers can avoid redundant re-uploads.
/// Bump `generation` whenever the mesh geometry or colors change; callers
/// compare against their last-seen generation to skip work.
pub struct SimMeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    /// Monotonically increasing counter bumped on every geometry or color update.
    /// Callers cache the last-seen value and skip re-upload when it matches.
    pub generation: u64,
    /// Cached color fingerprint: (num_vertices, viz_mode_tag, first+last color).
    /// Used by `update_colors_if_changed` to skip redundant color re-uploads.
    cached_color_fingerprint: ColorFingerprint,
}

/// Lightweight fingerprint to detect whether the color array has changed.
#[derive(Clone, Copy, PartialEq)]
struct ColorFingerprint {
    len: usize,
    first: [f32; 3],
    last: [f32; 3],
    /// Simple hash: sum of every 64th color entry for fast change detection.
    sample_sum: f32,
}

impl ColorFingerprint {
    fn from_colors(colors: &[[f32; 3]]) -> Self {
        let len = colors.len();
        let first = colors.first().copied().unwrap_or([0.0; 3]);
        let last = colors.last().copied().unwrap_or([0.0; 3]);
        let mut sample_sum: f32 = 0.0;
        let stride = 64;
        let mut i = 0;
        while i < len {
            let c = colors[i];
            sample_sum += c[0] + c[1] + c[2];
            i += stride;
        }
        Self {
            len,
            first,
            last,
            sample_sum,
        }
    }
}

/// Compute per-vertex colors based on deviation from model surface.
///
/// Each entry in `deviations` is `sim_z - model_z` for that vertex:
/// - Positive = material remaining (stock not yet cut away)
/// - Negative = overcut (cut below the model surface)
/// - Zero = on target
///
/// Color mapping:
/// - Green: on target (deviation in ±0.1 mm)
/// - Blue: material remaining (deviation > 0.1 mm)
/// - Yellow: slight overcut (deviation in -0.5..−0.1 mm)
/// - Red: major overcut (deviation < -0.5 mm)
///
/// Returns one `[r, g, b]` per vertex.
pub fn deviation_colors(deviations: &[f32]) -> Vec<[f32; 3]> {
    deviations
        .iter()
        .map(|&d| {
            if d > 0.1 {
                // Material remaining — blue, intensity by amount
                let t = ((d - 0.1) / 1.0).min(1.0); // 0..1 over 0.1..1.1 mm
                [0.0, 0.3 * (1.0 - t), 0.5 + 0.5 * t] // darker blue as more remaining
            } else if d < -0.5 {
                // Major overcut — red
                let t = ((-d - 0.5) / 1.0).min(1.0);
                [0.6 + 0.4 * t, 0.0, 0.0]
            } else if d < -0.1 {
                // Slight overcut — yellow, blending toward red
                let t = ((-d - 0.1) / 0.4).min(1.0); // 0..1 over -0.1..-0.5
                [0.8 + 0.2 * t, 0.8 * (1.0 - t), 0.0]
            } else {
                // On target — green
                [0.1, 0.75, 0.1]
            }
        })
        .collect()
}

/// Generate per-vertex colors for height-gradient mode.
/// Maps Z values from min_z to max_z: blue (low) -> green (mid) -> red (high).
pub fn height_gradient_colors(vertices: &[f32]) -> Vec<[f32; 3]> {
    let num_verts = vertices.len() / 3;
    if num_verts == 0 {
        return Vec::new();
    }

    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;
    for i in 0..num_verts {
        let z = vertices[i * 3 + 2];
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }
    let range = (max_z - min_z).max(0.001);

    (0..num_verts)
        .map(|i| {
            let z = vertices[i * 3 + 2];
            let t = ((z - min_z) / range).clamp(0.0, 1.0);
            if t < 0.5 {
                // Blue to green
                let s = t * 2.0;
                [0.0, s, 1.0 - s]
            } else {
                // Green to red
                let s = (t - 0.5) * 2.0;
                [s, 1.0 - s, 0.0]
            }
        })
        .collect()
}

/// Generate per-vertex colors based on which operation index removed material.
/// `op_colors` maps boundary index to palette color for each vertex.
/// Since we don't track per-vertex op ownership in the heightmap, this returns the
/// wood-tone default (operations color requires per-cell tracking in a future pass).
pub fn operation_placeholder_colors(num_verts: usize) -> Vec<[f32; 3]> {
    // Placeholder: uniform color until per-vertex op tracking is implemented
    vec![[0.65, 0.45, 0.25]; num_verts]
}

/// Counter for unique generation IDs across all `SimMeshGpuData` instances.
static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl SimMeshGpuData {
    /// Upload a StockMesh to the GPU using its embedded wood-tone colors.
    /// Returns `None` if the buffer exceeds GPU device limits.
    pub fn from_heightmap_mesh(
        device: &wgpu::Device,
        limits: &GpuLimits,
        hm: &StockMesh,
    ) -> Option<Self> {
        let num_verts = hm.vertices.len() / 3;
        let colors: Vec<[f32; 3]> = if hm.colors.len() >= num_verts * 3 {
            (0..num_verts)
                .map(|i| [hm.colors[i * 3], hm.colors[i * 3 + 1], hm.colors[i * 3 + 2]])
                .collect()
        } else {
            vec![[0.65, 0.45, 0.25]; num_verts]
        };
        Self::from_heightmap_mesh_colored(device, limits, hm, &colors)
    }

    /// Upload a StockMesh with per-vertex custom colors.
    /// `colors` is one `[r, g, b]` per vertex (from deviation_colors, height_gradient_colors, etc.).
    /// Returns `None` if the buffer exceeds GPU device limits.
    pub fn from_heightmap_mesh_colored(
        device: &wgpu::Device,
        limits: &GpuLimits,
        hm: &StockMesh,
        colors: &[[f32; 3]],
    ) -> Option<Self> {
        let mesh_verts = Self::build_vertex_data(hm, colors);
        let fingerprint = ColorFingerprint::from_colors(colors);

        let vertex_bytes = bytemuck::cast_slice::<_, u8>(&mesh_verts);
        let index_bytes = bytemuck::cast_slice::<_, u8>(&hm.indices);

        let vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "sim_mesh_vertices",
            vertex_bytes,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        )?;

        let index_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "sim_mesh_indices",
            index_bytes,
            wgpu::BufferUsages::INDEX,
        )?;

        Some(Self {
            vertex_buffer,
            index_buffer,
            index_count: hm.indices.len() as u32,
            generation: NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            cached_color_fingerprint: fingerprint,
        })
    }

    /// Re-upload colors only if they differ from the cached fingerprint.
    ///
    /// When only the viz mode changes (not the geometry), this avoids rebuilding
    /// the full vertex buffer from scratch when the colors haven't actually changed.
    /// Returns `true` if the buffer was updated, `false` if skipped.
    pub fn update_colors_if_changed(
        &mut self,
        queue: &wgpu::Queue,
        hm: &StockMesh,
        colors: &[[f32; 3]],
    ) -> bool {
        let new_fingerprint = ColorFingerprint::from_colors(colors);
        if new_fingerprint == self.cached_color_fingerprint {
            return false;
        }

        let mesh_verts = Self::build_vertex_data(hm, colors);
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&mesh_verts));
        self.cached_color_fingerprint = new_fingerprint;
        self.generation = NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        true
    }

    /// Build the full vertex array (positions + normals + colors) from a StockMesh.
    fn build_vertex_data(hm: &StockMesh, colors: &[[f32; 3]]) -> Vec<ColoredMeshVertex> {
        let num_verts = hm.vertices.len() / 3;
        let mut mesh_verts = Vec::with_capacity(num_verts);

        for i in 0..num_verts {
            mesh_verts.push(ColoredMeshVertex {
                position: [
                    hm.vertices[i * 3],
                    hm.vertices[i * 3 + 1],
                    hm.vertices[i * 3 + 2],
                ],
                normal: [0.0, 0.0, 1.0],
                color: colors.get(i).copied().unwrap_or([0.65, 0.45, 0.25]),
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

        mesh_verts
    }
}

/// Tool model visualization: a simple wireframe representation of the tool at a position.
/// Uses line segments to draw the tool outline (simpler than a mesh, no new pipeline needed).
pub struct ToolModelGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
}

/// Which geometric shape to draw for the tool wireframe.
#[derive(Debug, Clone, Copy)]
pub enum ToolShape {
    FlatEnd,
    BallNose,
    BullNose,
    VBit,
    TaperedBallNose,
}

/// Full tool geometry for wireframe rendering.
#[derive(Debug, Clone, Copy)]
pub struct ToolGeometry {
    pub radius: f32,
    pub cutting_length: f32,
    pub shape: ToolShape,
    /// Corner radius for BullNose tools.
    pub corner_radius: f32,
    /// Full included angle in degrees for VBit tools.
    pub included_angle: f32,
    /// Half-angle in degrees for TaperedBallNose tools.
    pub taper_half_angle: f32,
}

/// Full tool assembly dimensions for wireframe rendering (shank + holder above cutter).
/// All lengths in mm.
pub struct ToolAssemblyInfo {
    /// Shank cylinder radius (shank_diameter / 2).
    pub shank_radius: f32,
    /// Shank cylinder length above cutter.
    pub shank_length: f32,
    /// Holder cylinder radius (holder_diameter / 2).
    pub holder_radius: f32,
    /// Total stickout from holder face to tool tip; holder length = stickout - cutting_length - shank_length.
    pub stickout: f32,
}

impl ToolModelGpuData {
    /// Generate tool wireframe lines at the given position (cutter only, no shank/holder).
    pub fn from_tool_geometry(
        device: &wgpu::Device,
        geom: &ToolGeometry,
        position: [f32; 3],
    ) -> Self {
        Self::from_tool_assembly(
            device,
            geom,
            &ToolAssemblyInfo {
                shank_radius: 0.0,
                shank_length: 0.0,
                holder_radius: 0.0,
                stickout: 0.0,
            },
            position,
        )
    }

    /// Generate wireframe for the complete tool assembly: cutter + shank + holder.
    /// `position`: [x, y, z] of the tool tip (lowest point of the cutter).
    pub fn from_tool_assembly(
        device: &wgpu::Device,
        geom: &ToolGeometry,
        info: &ToolAssemblyInfo,
        position: [f32; 3],
    ) -> Self {
        use wgpu::util::DeviceExt;

        let cutter_color = [0.8, 0.8, 0.3]; // yellow-ish for cutter
        let shank_color = [0.6, 0.6, 0.5]; // lighter gray for shank
        let holder_color = [0.4, 0.4, 0.35]; // darker gray for holder
        let segments: usize = 24;
        let mut verts = Vec::new();

        let tip_z = position[2];
        let r = geom.radius;

        // --- Cutter body (per-shape wireframe) ---
        let cutter_ctx = ToolWireCtx {
            cx: position[0],
            cy: position[1],
            segments,
            color: cutter_color,
        };

        match geom.shape {
            ToolShape::FlatEnd => {
                cutter_ctx.draw_flat_end(&mut verts, tip_z, r, geom.cutting_length);
            }
            ToolShape::BallNose => {
                cutter_ctx.draw_ball_nose(&mut verts, tip_z, r, geom.cutting_length);
            }
            ToolShape::BullNose => {
                let cr = geom.corner_radius.min(r);
                cutter_ctx.draw_bull_nose(&mut verts, tip_z, r, cr, geom.cutting_length);
            }
            ToolShape::VBit => {
                cutter_ctx.draw_vbit(
                    &mut verts,
                    tip_z,
                    r,
                    geom.included_angle,
                    geom.cutting_length,
                );
            }
            ToolShape::TaperedBallNose => {
                cutter_ctx.draw_tapered_ball_nose(
                    &mut verts,
                    tip_z,
                    r,
                    geom.taper_half_angle,
                    geom.cutting_length,
                );
            }
        }

        let cutter_top_z = tip_z + geom.cutting_length;

        // --- Shank cylinder ---
        if info.shank_radius > 0.01 && info.shank_length > 0.01 {
            let shank_bottom_z = cutter_top_z;
            let shank_top_z = cutter_top_z + info.shank_length;
            let sr = info.shank_radius;

            let shank_ctx = ToolWireCtx {
                cx: position[0],
                cy: position[1],
                segments,
                color: shank_color,
            };
            shank_ctx.push_circle(&mut verts, shank_bottom_z, sr);
            shank_ctx.push_circle(&mut verts, shank_top_z, sr);
            shank_ctx.push_verticals(&mut verts, sr, shank_bottom_z, sr, shank_top_z);
        }

        // --- Holder cylinder ---
        // Holder extends from top of shank upward by the remaining stickout distance
        let holder_bottom_z = cutter_top_z + info.shank_length;
        let holder_length = (info.stickout - geom.cutting_length - info.shank_length).max(0.0);
        if info.holder_radius > 0.01 && holder_length > 0.01 {
            let holder_top_z = holder_bottom_z + holder_length;
            let hr = info.holder_radius;

            let holder_ctx = ToolWireCtx {
                cx: position[0],
                cy: position[1],
                segments,
                color: holder_color,
            };
            holder_ctx.push_circle(&mut verts, holder_bottom_z, hr);
            holder_ctx.push_circle(&mut verts, holder_top_z, hr);
            holder_ctx.push_verticals(&mut verts, hr, holder_bottom_z, hr, holder_top_z);
        }

        if verts.is_empty() {
            verts.push(LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            });
        }

        let vertex_count = verts.len() as u32;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tool_model"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            vertex_buffer,
            vertex_count,
        }
    }
}

/// Shared context for tool wireframe drawing to reduce argument count.
struct ToolWireCtx {
    cx: f32,
    cy: f32,
    segments: usize,
    color: [f32; 3],
}

impl ToolWireCtx {
    /// Add a circle of line segments at the given z height and radius.
    fn push_circle(&self, verts: &mut Vec<LineVertex>, z: f32, radius: f32) {
        for i in 0..self.segments {
            let a0 = std::f32::consts::TAU * (i as f32) / (self.segments as f32);
            let a1 = std::f32::consts::TAU * ((i + 1) as f32) / (self.segments as f32);
            verts.push(LineVertex {
                position: [self.cx + radius * a0.cos(), self.cy + radius * a0.sin(), z],
                color: self.color,
            });
            verts.push(LineVertex {
                position: [self.cx + radius * a1.cos(), self.cy + radius * a1.sin(), z],
                color: self.color,
            });
        }
    }

    /// Add 4 vertical lines connecting two circles at 90-degree intervals.
    fn push_verticals(
        &self,
        verts: &mut Vec<LineVertex>,
        bottom_r: f32,
        bottom_z: f32,
        top_r: f32,
        top_z: f32,
    ) {
        for i in 0..4 {
            let a = std::f32::consts::TAU * (i as f32) / 4.0;
            verts.push(LineVertex {
                position: [
                    self.cx + bottom_r * a.cos(),
                    self.cy + bottom_r * a.sin(),
                    bottom_z,
                ],
                color: self.color,
            });
            verts.push(LineVertex {
                position: [self.cx + top_r * a.cos(), self.cy + top_r * a.sin(), top_z],
                color: self.color,
            });
        }
    }

    /// Add hemisphere arcs (XZ and YZ planes) centered at center_z.
    fn push_hemisphere_arcs(&self, verts: &mut Vec<LineVertex>, center_z: f32, radius: f32) {
        for i in 0..self.segments {
            let a0 = std::f32::consts::PI * (i as f32) / (self.segments as f32);
            let a1 = std::f32::consts::PI * ((i + 1) as f32) / (self.segments as f32);
            // XZ arc
            verts.push(LineVertex {
                position: [
                    self.cx + radius * a0.sin(),
                    self.cy,
                    center_z - radius * a0.cos(),
                ],
                color: self.color,
            });
            verts.push(LineVertex {
                position: [
                    self.cx + radius * a1.sin(),
                    self.cy,
                    center_z - radius * a1.cos(),
                ],
                color: self.color,
            });
            // YZ arc
            verts.push(LineVertex {
                position: [
                    self.cx,
                    self.cy + radius * a0.sin(),
                    center_z - radius * a0.cos(),
                ],
                color: self.color,
            });
            verts.push(LineVertex {
                position: [
                    self.cx,
                    self.cy + radius * a1.sin(),
                    center_z - radius * a1.cos(),
                ],
                color: self.color,
            });
        }
    }

    /// Flat end mill: bottom circle + top circle + 4 verticals.
    fn draw_flat_end(&self, verts: &mut Vec<LineVertex>, tip_z: f32, r: f32, cutting_length: f32) {
        let top_z = tip_z + cutting_length;
        self.push_circle(verts, tip_z, r);
        self.push_circle(verts, top_z, r);
        self.push_verticals(verts, r, tip_z, r, top_z);
    }

    /// Ball nose: hemisphere at bottom + cylinder above.
    fn draw_ball_nose(&self, verts: &mut Vec<LineVertex>, tip_z: f32, r: f32, cutting_length: f32) {
        let body_bottom = tip_z + r;
        let top_z = tip_z + cutting_length;
        self.push_circle(verts, body_bottom, r);
        self.push_circle(verts, top_z, r);
        self.push_verticals(verts, r, body_bottom, r, top_z);
        self.push_hemisphere_arcs(verts, body_bottom, r);
    }

    /// Bull nose: flat bottom with rounded corners (torus profile).
    fn draw_bull_nose(
        &self,
        verts: &mut Vec<LineVertex>,
        tip_z: f32,
        r: f32,
        corner_radius: f32,
        cutting_length: f32,
    ) {
        let cr = corner_radius;
        let inner_r = r - cr;
        let body_bottom = tip_z + cr;
        let top_z = tip_z + cutting_length;

        self.push_circle(verts, tip_z, inner_r);
        self.push_circle(verts, body_bottom, r);
        self.push_circle(verts, top_z, r);
        self.push_verticals(verts, r, body_bottom, r, top_z);

        // Corner radius arcs at 4 cardinal points
        let arc_segments = self.segments / 2;
        for i in 0..4 {
            let a = std::f32::consts::TAU * (i as f32) / 4.0;
            let arc_cx = self.cx + inner_r * a.cos();
            let arc_cy = self.cy + inner_r * a.sin();
            for j in 0..arc_segments {
                let t0 = std::f32::consts::FRAC_PI_2 * (j as f32) / (arc_segments as f32);
                let t1 = std::f32::consts::FRAC_PI_2 * ((j + 1) as f32) / (arc_segments as f32);
                let r0_offset = cr * t0.sin();
                let z0_offset = cr * t0.cos();
                let r1_offset = cr * t1.sin();
                let z1_offset = cr * t1.cos();
                verts.push(LineVertex {
                    position: [
                        arc_cx + r0_offset * a.cos(),
                        arc_cy + r0_offset * a.sin(),
                        body_bottom - z0_offset,
                    ],
                    color: self.color,
                });
                verts.push(LineVertex {
                    position: [
                        arc_cx + r1_offset * a.cos(),
                        arc_cy + r1_offset * a.sin(),
                        body_bottom - z1_offset,
                    ],
                    color: self.color,
                });
            }
        }
    }

    /// V-bit: cone from tip point to cutting diameter.
    fn draw_vbit(
        &self,
        verts: &mut Vec<LineVertex>,
        tip_z: f32,
        r: f32,
        included_angle_deg: f32,
        cutting_length: f32,
    ) {
        let half_angle = (included_angle_deg * 0.5).to_radians();
        let cone_height = if half_angle.tan().abs() > 1e-6 {
            r / half_angle.tan()
        } else {
            cutting_length
        };
        let cone_height = cone_height.min(cutting_length);
        let cone_top_z = tip_z + cone_height;
        let top_z = tip_z + cutting_length;

        self.push_circle(verts, cone_top_z, r);

        if cone_height < cutting_length - 0.01 {
            self.push_circle(verts, top_z, r);
            self.push_verticals(verts, r, cone_top_z, r, top_z);
        }

        for i in 0..4 {
            let a = std::f32::consts::TAU * (i as f32) / 4.0;
            verts.push(LineVertex {
                position: [self.cx, self.cy, tip_z],
                color: self.color,
            });
            verts.push(LineVertex {
                position: [self.cx + r * a.cos(), self.cy + r * a.sin(), cone_top_z],
                color: self.color,
            });
        }
    }

    /// Tapered ball nose: hemisphere at tip + tapered cone to cutting diameter.
    fn draw_tapered_ball_nose(
        &self,
        verts: &mut Vec<LineVertex>,
        tip_z: f32,
        r: f32,
        taper_half_angle_deg: f32,
        cutting_length: f32,
    ) {
        let taper_angle = taper_half_angle_deg.to_radians();
        let tip_r = r;
        let ball_center_z = tip_z + tip_r;

        self.push_hemisphere_arcs(verts, ball_center_z, tip_r);
        self.push_circle(verts, ball_center_z, tip_r);

        let taper_height = cutting_length - tip_r;
        let top_r = if taper_height > 0.0 {
            tip_r + taper_height * taper_angle.tan()
        } else {
            tip_r
        };
        let top_z = tip_z + cutting_length;

        self.push_circle(verts, top_z, top_r);
        self.push_verticals(verts, tip_r, ball_center_z, top_r, top_z);
    }
}
