use super::LineVertex;
use egui_wgpu::wgpu;

/// Ground grid + axis indicator GPU data.
pub struct GridGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
}

impl GridGpuData {
    /// Create a ground grid at Z=0 with axis indicators.
    pub fn new(device: &wgpu::Device, extent: f32, spacing: f32) -> Self {
        use wgpu::util::DeviceExt;

        let mut vertices = Vec::new();
        let grid_color = [0.25, 0.25, 0.28];
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
            color: [0.9, 0.2, 0.2],
        });
        vertices.push(LineVertex {
            position: [axis_len, 0.0, 0.0],
            color: [0.9, 0.2, 0.2],
        });
        // Y axis (green)
        vertices.push(LineVertex {
            position: [0.0, 0.0, 0.0],
            color: [0.2, 0.9, 0.2],
        });
        vertices.push(LineVertex {
            position: [0.0, axis_len, 0.0],
            color: [0.2, 0.9, 0.2],
        });
        // Z axis (blue)
        vertices.push(LineVertex {
            position: [0.0, 0.0, 0.0],
            color: [0.3, 0.4, 0.95],
        });
        vertices.push(LineVertex {
            position: [0.0, 0.0, axis_len * 0.5],
            color: [0.3, 0.4, 0.95],
        });

        let vertex_count = vertices.len() as u32;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            vertex_buffer,
            vertex_count,
        }
    }
}
