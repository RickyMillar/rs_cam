use super::LineVertex;
use egui_wgpu::wgpu;
use rs_cam_core::geo::BoundingBox3;

/// GPU data for fixture and keep-out zone wireframe boxes.
pub struct FixtureGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
}

impl FixtureGpuData {
    /// Create wireframe boxes from a list of (bounding_box, color) pairs.
    /// Each box generates 24 vertices (12 edges as line list).
    pub fn from_boxes(device: &wgpu::Device, boxes: &[(BoundingBox3, [f32; 3])]) -> Self {
        Self::from_boxes_and_lines(device, boxes, &[])
    }

    /// Create wireframe boxes plus additional line vertices (for pin markers).
    pub fn from_boxes_and_lines(
        device: &wgpu::Device,
        boxes: &[(BoundingBox3, [f32; 3])],
        extra_lines: &[LineVertex],
    ) -> Self {
        use wgpu::util::DeviceExt;

        let mut vertices = Vec::with_capacity(boxes.len() * 24 + extra_lines.len());

        for (bbox, color) in boxes {
            let min = [bbox.min.x as f32, bbox.min.y as f32, bbox.min.z as f32];
            let max = [bbox.max.x as f32, bbox.max.y as f32, bbox.max.z as f32];
            let color = *color;

            // Bottom face
            vertices.push(LineVertex {
                position: [min[0], min[1], min[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], min[1], min[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], min[1], min[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], max[1], min[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], max[1], min[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [min[0], max[1], min[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [min[0], max[1], min[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [min[0], min[1], min[2]],
                color,
            });

            // Top face
            vertices.push(LineVertex {
                position: [min[0], min[1], max[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], min[1], max[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], min[1], max[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], max[1], max[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], max[1], max[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [min[0], max[1], max[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [min[0], max[1], max[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [min[0], min[1], max[2]],
                color,
            });

            // Vertical edges
            vertices.push(LineVertex {
                position: [min[0], min[1], min[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [min[0], min[1], max[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], min[1], min[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], min[1], max[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], max[1], min[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [max[0], max[1], max[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [min[0], max[1], min[2]],
                color,
            });
            vertices.push(LineVertex {
                position: [min[0], max[1], max[2]],
                color,
            });
        }

        vertices.extend_from_slice(extra_lines);

        let vertex_count = vertices.len() as u32;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fixture_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            vertex_buffer,
            vertex_count,
        }
    }
}
