use super::LineVertex;
use super::gpu_safety::{self, GpuLimits};
use egui_wgpu::wgpu;

/// Ground grid + axis indicator GPU data.
pub struct GridGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
}

impl GridGpuData {
    /// Create a ground grid at Z=0 with axis indicators.
    pub fn new(device: &wgpu::Device, limits: &GpuLimits, extent: f32, spacing: f32) -> Self {
        use super::colors;
        let mut vertices = Vec::new();
        let grid_color = colors::GRID_BASE;
        // Clamp spacing to prevent vertex explosion with tiny values
        let spacing = spacing.max(0.1);
        let half = extent / 2.0;
        let steps = (extent / spacing) as i32;

        for i in -steps / 2..=steps / 2 {
            let y = i as f32 * spacing;
            vertices.push(LineVertex {
                position: [-half, y, 0.0],
                color: grid_color,
            });
            vertices.push(LineVertex {
                position: [half, y, 0.0],
                color: grid_color,
            });
        }

        for i in -steps / 2..=steps / 2 {
            let x = i as f32 * spacing;
            vertices.push(LineVertex {
                position: [x, -half, 0.0],
                color: grid_color,
            });
            vertices.push(LineVertex {
                position: [x, half, 0.0],
                color: grid_color,
            });
        }

        // X axis (red)
        let axis_len = extent * 0.6;
        vertices.push(LineVertex {
            position: [0.0, 0.0, 0.0],
            color: colors::AXIS_X,
        });
        vertices.push(LineVertex {
            position: [axis_len, 0.0, 0.0],
            color: colors::AXIS_X,
        });
        // Y axis (green)
        vertices.push(LineVertex {
            position: [0.0, 0.0, 0.0],
            color: colors::AXIS_Y,
        });
        vertices.push(LineVertex {
            position: [0.0, axis_len, 0.0],
            color: colors::AXIS_Y,
        });
        // Z axis (blue)
        vertices.push(LineVertex {
            position: [0.0, 0.0, 0.0],
            color: colors::AXIS_Z,
        });
        vertices.push(LineVertex {
            position: [0.0, 0.0, axis_len * 0.5],
            color: colors::AXIS_Z,
        });

        let vertex_count = vertices.len() as u32;
        let vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "grid_vertices",
            bytemuck::cast_slice(&vertices),
            wgpu::BufferUsages::VERTEX,
        );
        // If the grid buffer somehow exceeds limits, fall back to an empty grid
        // by creating a minimal placeholder buffer.
        let vertex_buffer = match vertex_buffer {
            Some(buf) => buf,
            None => {
                use wgpu::util::DeviceExt;
                let placeholder = [LineVertex {
                    position: [0.0; 3],
                    color: [0.0; 3],
                }];
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("grid_vertices_placeholder"),
                    contents: bytemuck::cast_slice(&placeholder),
                    usage: wgpu::BufferUsages::VERTEX,
                })
            }
        };

        Self {
            vertex_buffer,
            vertex_count,
        }
    }
}

/// XYZ axes at the stock origin, rendered in the 3D scene.
pub struct OriginAxesGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
}

impl OriginAxesGpuData {
    /// Create XYZ axes at a given origin with a given length.
    pub fn new(device: &wgpu::Device, origin: [f32; 3], length: f32) -> Self {
        use wgpu::util::DeviceExt;

        let ox = origin[0];
        let oy = origin[1];
        let oz = origin[2];

        let vertices = [
            // X axis (red)
            LineVertex {
                position: [ox, oy, oz],
                color: [0.9, 0.2, 0.2],
            },
            LineVertex {
                position: [ox + length, oy, oz],
                color: [0.9, 0.2, 0.2],
            },
            // Y axis (green)
            LineVertex {
                position: [ox, oy, oz],
                color: [0.2, 0.9, 0.2],
            },
            LineVertex {
                position: [ox, oy + length, oz],
                color: [0.2, 0.9, 0.2],
            },
            // Z axis (blue)
            LineVertex {
                position: [ox, oy, oz],
                color: [0.3, 0.4, 0.95],
            },
            LineVertex {
                position: [ox, oy, oz + length],
                color: [0.3, 0.4, 0.95],
            },
        ];

        let vertex_count = vertices.len() as u32;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("origin_axes_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            vertex_buffer,
            vertex_count,
        }
    }
}
