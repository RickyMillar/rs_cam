use super::LineVertex;
use egui_wgpu::wgpu;
use rs_cam_core::geo::BoundingBox3;

/// Wireframe stock bounding box GPU data.
pub struct StockGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
}

impl StockGpuData {
    /// Create a wireframe box from a bounding box.
    pub fn from_bbox(device: &wgpu::Device, bbox: &BoundingBox3) -> Self {
        use wgpu::util::DeviceExt;

        let color = [0.4, 0.6, 0.8];
        let mn = [bbox.min.x as f32, bbox.min.y as f32, bbox.min.z as f32];
        let mx = [bbox.max.x as f32, bbox.max.y as f32, bbox.max.z as f32];

        // 12 edges of a box = 24 vertices (line list)
        let vertices = [
            // Bottom face
            LineVertex {
                position: [mn[0], mn[1], mn[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mn[1], mn[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mn[1], mn[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mx[1], mn[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mx[1], mn[2]],
                color,
            },
            LineVertex {
                position: [mn[0], mx[1], mn[2]],
                color,
            },
            LineVertex {
                position: [mn[0], mx[1], mn[2]],
                color,
            },
            LineVertex {
                position: [mn[0], mn[1], mn[2]],
                color,
            },
            // Top face
            LineVertex {
                position: [mn[0], mn[1], mx[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mn[1], mx[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mn[1], mx[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mx[1], mx[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mx[1], mx[2]],
                color,
            },
            LineVertex {
                position: [mn[0], mx[1], mx[2]],
                color,
            },
            LineVertex {
                position: [mn[0], mx[1], mx[2]],
                color,
            },
            LineVertex {
                position: [mn[0], mn[1], mx[2]],
                color,
            },
            // Vertical edges
            LineVertex {
                position: [mn[0], mn[1], mn[2]],
                color,
            },
            LineVertex {
                position: [mn[0], mn[1], mx[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mn[1], mn[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mn[1], mx[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mx[1], mn[2]],
                color,
            },
            LineVertex {
                position: [mx[0], mx[1], mx[2]],
                color,
            },
            LineVertex {
                position: [mn[0], mx[1], mn[2]],
                color,
            },
            LineVertex {
                position: [mn[0], mx[1], mx[2]],
                color,
            },
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("stock_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            vertex_buffer,
            vertex_count: 24,
        }
    }
}
