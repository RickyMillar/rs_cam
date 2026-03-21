use super::LineVertex;
use super::sim_render::ColoredMeshVertex;
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

/// Semi-transparent solid stock box for spatial comprehension.
/// Uses the colored mesh pipeline with alpha blending.
pub struct SolidStockGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl SolidStockGpuData {
    pub fn from_bbox(device: &wgpu::Device, bbox: &BoundingBox3) -> Self {
        use wgpu::util::DeviceExt;

        let mn = [bbox.min.x as f32, bbox.min.y as f32, bbox.min.z as f32];
        let mx = [bbox.max.x as f32, bbox.max.y as f32, bbox.max.z as f32];
        let color = [0.65, 0.50, 0.30]; // warm wood tone

        // 8 unique positions, but we need per-face normals so 24 vertices (4 per face)
        let vertices = [
            // Top face (Z+) — normal [0,0,1]
            ColoredMeshVertex {
                position: [mn[0], mn[1], mx[2]],
                normal: [0.0, 0.0, 1.0],
                color,
            },
            ColoredMeshVertex {
                position: [mx[0], mn[1], mx[2]],
                normal: [0.0, 0.0, 1.0],
                color,
            },
            ColoredMeshVertex {
                position: [mx[0], mx[1], mx[2]],
                normal: [0.0, 0.0, 1.0],
                color,
            },
            ColoredMeshVertex {
                position: [mn[0], mx[1], mx[2]],
                normal: [0.0, 0.0, 1.0],
                color,
            },
            // Bottom face (Z-) — normal [0,0,-1]
            ColoredMeshVertex {
                position: [mn[0], mx[1], mn[2]],
                normal: [0.0, 0.0, -1.0],
                color,
            },
            ColoredMeshVertex {
                position: [mx[0], mx[1], mn[2]],
                normal: [0.0, 0.0, -1.0],
                color,
            },
            ColoredMeshVertex {
                position: [mx[0], mn[1], mn[2]],
                normal: [0.0, 0.0, -1.0],
                color,
            },
            ColoredMeshVertex {
                position: [mn[0], mn[1], mn[2]],
                normal: [0.0, 0.0, -1.0],
                color,
            },
            // Front face (Y-) — normal [0,-1,0]
            ColoredMeshVertex {
                position: [mn[0], mn[1], mn[2]],
                normal: [0.0, -1.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mx[0], mn[1], mn[2]],
                normal: [0.0, -1.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mx[0], mn[1], mx[2]],
                normal: [0.0, -1.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mn[0], mn[1], mx[2]],
                normal: [0.0, -1.0, 0.0],
                color,
            },
            // Back face (Y+) — normal [0,1,0]
            ColoredMeshVertex {
                position: [mx[0], mx[1], mn[2]],
                normal: [0.0, 1.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mn[0], mx[1], mn[2]],
                normal: [0.0, 1.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mn[0], mx[1], mx[2]],
                normal: [0.0, 1.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mx[0], mx[1], mx[2]],
                normal: [0.0, 1.0, 0.0],
                color,
            },
            // Right face (X+) — normal [1,0,0]
            ColoredMeshVertex {
                position: [mx[0], mn[1], mn[2]],
                normal: [1.0, 0.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mx[0], mx[1], mn[2]],
                normal: [1.0, 0.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mx[0], mx[1], mx[2]],
                normal: [1.0, 0.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mx[0], mn[1], mx[2]],
                normal: [1.0, 0.0, 0.0],
                color,
            },
            // Left face (X-) — normal [-1,0,0]
            ColoredMeshVertex {
                position: [mn[0], mx[1], mn[2]],
                normal: [-1.0, 0.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mn[0], mn[1], mn[2]],
                normal: [-1.0, 0.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mn[0], mn[1], mx[2]],
                normal: [-1.0, 0.0, 0.0],
                color,
            },
            ColoredMeshVertex {
                position: [mn[0], mx[1], mx[2]],
                normal: [-1.0, 0.0, 0.0],
                color,
            },
        ];

        // 6 faces × 2 triangles × 3 indices = 36
        let indices: [u32; 36] = [
            0, 1, 2, 0, 2, 3, // top
            4, 5, 6, 4, 6, 7, // bottom
            8, 9, 10, 8, 10, 11, // front
            12, 13, 14, 12, 14, 15, // back
            16, 17, 18, 16, 18, 19, // right
            20, 21, 22, 20, 22, 23, // left
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("solid_stock_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("solid_stock_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count: 36,
        }
    }
}
