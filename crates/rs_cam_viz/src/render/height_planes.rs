use super::sim_render::ColoredMeshVertex;
use egui_wgpu::wgpu;
use rs_cam_core::geo::BoundingBox3;

/// Resolved height values with their display colors.
struct HeightPlane {
    z: f32,
    color: [f32; 3],
}

/// GPU data for semi-transparent height plane overlays.
/// Renders 5 horizontal quads at resolved height Z values using the colored mesh pipeline.
pub struct HeightPlanesGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl HeightPlanesGpuData {
    /// Generate height plane quads spanning the stock XY bounds at each resolved height Z.
    ///
    /// Heights are: clearance_z (blue), retract_z (cyan), feed_z (green),
    /// top_z (yellow), bottom_z (red).
    pub fn from_heights(
        device: &wgpu::Device,
        stock_bbox: &BoundingBox3,
        clearance_z: f64,
        retract_z: f64,
        feed_z: f64,
        top_z: f64,
        bottom_z: f64,
    ) -> Self {
        use wgpu::util::DeviceExt;

        use super::colors;
        let planes = [
            HeightPlane {
                z: clearance_z as f32,
                color: colors::HEIGHT_CLEARANCE,
            },
            HeightPlane {
                z: retract_z as f32,
                color: colors::HEIGHT_RETRACT,
            },
            HeightPlane {
                z: feed_z as f32,
                color: colors::HEIGHT_FEED,
            },
            HeightPlane {
                z: top_z as f32,
                color: colors::HEIGHT_TOP,
            },
            HeightPlane {
                z: bottom_z as f32,
                color: colors::HEIGHT_BOTTOM,
            },
        ];

        let mn_x = stock_bbox.min.x as f32;
        let mn_y = stock_bbox.min.y as f32;
        let mx_x = stock_bbox.max.x as f32;
        let mx_y = stock_bbox.max.y as f32;

        let mut vertices = Vec::with_capacity(planes.len() * 4);
        let mut indices = Vec::with_capacity(planes.len() * 6);

        for (i, plane) in planes.iter().enumerate() {
            let base = (i * 4) as u32;
            let z = plane.z;
            let normal = [0.0, 0.0, 1.0];

            vertices.push(ColoredMeshVertex {
                position: [mn_x, mn_y, z],
                normal,
                color: plane.color,
            });
            vertices.push(ColoredMeshVertex {
                position: [mx_x, mn_y, z],
                normal,
                color: plane.color,
            });
            vertices.push(ColoredMeshVertex {
                position: [mx_x, mx_y, z],
                normal,
                color: plane.color,
            });
            vertices.push(ColoredMeshVertex {
                position: [mn_x, mx_y, z],
                normal,
                color: plane.color,
            });

            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("height_planes_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("height_planes_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }
}
