use egui_wgpu::wgpu;
use rs_cam_core::simulation::HeightmapMesh;

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
pub struct SimMeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
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

impl SimMeshGpuData {
    /// Upload a HeightmapMesh to the GPU using its embedded wood-tone colors.
    pub fn from_heightmap_mesh(device: &wgpu::Device, hm: &HeightmapMesh) -> Self {
        let num_verts = hm.vertices.len() / 3;
        let colors: Vec<[f32; 3]> = if hm.colors.len() >= num_verts * 3 {
            (0..num_verts)
                .map(|i| [hm.colors[i * 3], hm.colors[i * 3 + 1], hm.colors[i * 3 + 2]])
                .collect()
        } else {
            vec![[0.65, 0.45, 0.25]; num_verts]
        };
        Self::from_heightmap_mesh_colored(device, hm, &colors)
    }

    /// Upload a HeightmapMesh with per-vertex custom colors.
    /// `colors` is one `[r, g, b]` per vertex (from deviation_colors, height_gradient_colors, etc.).
    pub fn from_heightmap_mesh_colored(
        device: &wgpu::Device,
        hm: &HeightmapMesh,
        colors: &[[f32; 3]],
    ) -> Self {
        use wgpu::util::DeviceExt;

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

/// Full tool assembly dimensions for wireframe rendering.
/// All lengths in mm. The position is the tool TIP.
pub struct ToolAssemblyInfo {
    /// Cutter radius (diameter / 2).
    pub tool_radius: f32,
    /// Cutting flute length.
    pub cutting_length: f32,
    /// Whether the cutter has a ball nose hemisphere.
    pub is_ball: bool,
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
    /// Generate tool wireframe lines at the given position.
    /// `tool_radius`: radius of the cutting tool.
    /// `tool_length`: cutting length.
    /// `is_ball`: true for ball nose (hemisphere bottom), false for flat end.
    /// `position`: [x, y, z] of the tool tip.
    pub fn from_tool(
        device: &wgpu::Device,
        tool_radius: f32,
        tool_length: f32,
        is_ball: bool,
        position: [f32; 3],
    ) -> Self {
        // Delegate to full assembly with no shank/holder (backward compat)
        Self::from_tool_assembly(
            device,
            &ToolAssemblyInfo {
                tool_radius,
                cutting_length: tool_length,
                is_ball,
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
        info: &ToolAssemblyInfo,
        position: [f32; 3],
    ) -> Self {
        use wgpu::util::DeviceExt;

        let cutter_color = [0.8, 0.8, 0.3]; // yellow-ish for cutter
        let shank_color = [0.6, 0.6, 0.5]; // lighter gray for shank
        let holder_color = [0.4, 0.4, 0.35]; // darker gray for holder
        let segments = 24;
        let mut verts = Vec::new();

        let cx = position[0];
        let cy = position[1];
        let tip_z = position[2];
        let r = info.tool_radius;

        // --- Cutter body ---
        // Bottom circle (at tip for flat, at center of ball for ball nose)
        let cutter_bottom_z = if info.is_ball { tip_z + r } else { tip_z };
        draw_circle(
            &mut verts,
            cx,
            cy,
            cutter_bottom_z,
            r,
            segments,
            cutter_color,
        );

        // Top of cutter
        let cutter_top_z = tip_z + info.cutting_length;
        draw_circle(&mut verts, cx, cy, cutter_top_z, r, segments, cutter_color);

        // Vertical connectors for cutter
        draw_vertical_connectors(
            &mut verts,
            cx,
            cy,
            cutter_bottom_z,
            cutter_top_z,
            r,
            cutter_color,
        );

        // Ball nose hemisphere (arcs in XZ and YZ planes)
        if info.is_ball {
            for i in 0..segments {
                let a0 = std::f32::consts::PI * (i as f32) / (segments as f32);
                let a1 = std::f32::consts::PI * ((i + 1) as f32) / (segments as f32);
                // XZ arc
                verts.push(LineVertex {
                    position: [cx + r * a0.sin(), cy, tip_z + r - r * a0.cos()],
                    color: cutter_color,
                });
                verts.push(LineVertex {
                    position: [cx + r * a1.sin(), cy, tip_z + r - r * a1.cos()],
                    color: cutter_color,
                });
                // YZ arc
                verts.push(LineVertex {
                    position: [cx, cy + r * a0.sin(), tip_z + r - r * a0.cos()],
                    color: cutter_color,
                });
                verts.push(LineVertex {
                    position: [cx, cy + r * a1.sin(), tip_z + r - r * a1.cos()],
                    color: cutter_color,
                });
            }
        }

        // --- Shank cylinder ---
        if info.shank_radius > 0.01 && info.shank_length > 0.01 {
            let shank_bottom_z = cutter_top_z;
            let shank_top_z = cutter_top_z + info.shank_length;
            let sr = info.shank_radius;

            draw_circle(
                &mut verts,
                cx,
                cy,
                shank_bottom_z,
                sr,
                segments,
                shank_color,
            );
            draw_circle(&mut verts, cx, cy, shank_top_z, sr, segments, shank_color);
            draw_vertical_connectors(
                &mut verts,
                cx,
                cy,
                shank_bottom_z,
                shank_top_z,
                sr,
                shank_color,
            );
        }

        // --- Holder cylinder ---
        // Holder extends from top of shank upward by the remaining stickout distance
        let holder_bottom_z = cutter_top_z + info.shank_length;
        let holder_length = (info.stickout - info.cutting_length - info.shank_length).max(0.0);
        if info.holder_radius > 0.01 && holder_length > 0.01 {
            let holder_top_z = holder_bottom_z + holder_length;
            let hr = info.holder_radius;

            draw_circle(
                &mut verts,
                cx,
                cy,
                holder_bottom_z,
                hr,
                segments,
                holder_color,
            );
            draw_circle(&mut verts, cx, cy, holder_top_z, hr, segments, holder_color);
            draw_vertical_connectors(
                &mut verts,
                cx,
                cy,
                holder_bottom_z,
                holder_top_z,
                hr,
                holder_color,
            );
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

/// Draw a circle of line segments at the given center (cx, cy, z) with radius r.
fn draw_circle(
    verts: &mut Vec<LineVertex>,
    cx: f32,
    cy: f32,
    z: f32,
    r: f32,
    segments: u32,
    color: [f32; 3],
) {
    for i in 0..segments {
        let a0 = std::f32::consts::TAU * (i as f32) / (segments as f32);
        let a1 = std::f32::consts::TAU * ((i + 1) as f32) / (segments as f32);
        verts.push(LineVertex {
            position: [cx + r * a0.cos(), cy + r * a0.sin(), z],
            color,
        });
        verts.push(LineVertex {
            position: [cx + r * a1.cos(), cy + r * a1.sin(), z],
            color,
        });
    }
}

/// Draw 4 vertical connector lines between bottom_z and top_z at 90-degree intervals.
fn draw_vertical_connectors(
    verts: &mut Vec<LineVertex>,
    cx: f32,
    cy: f32,
    bottom_z: f32,
    top_z: f32,
    r: f32,
    color: [f32; 3],
) {
    for i in 0..4 {
        let a = std::f32::consts::TAU * (i as f32) / 4.0;
        verts.push(LineVertex {
            position: [cx + r * a.cos(), cy + r * a.sin(), bottom_z],
            color,
        });
        verts.push(LineVertex {
            position: [cx + r * a.cos(), cy + r * a.sin(), top_z],
            color,
        });
    }
}
