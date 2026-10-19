//! The simulation resources: the stock mesh and the collision markers.

use crate::render::RenderResources;
use crate::render::sim_render::SimMeshGpuData;
use crate::render::upload_cache::{self, SlotInputs};

use super::{Gpu, RsCamApp, shift_key};

/// Re-upload the sim mesh with the current viz mode colours, or clear it
/// when there are no results. If the geometry is already uploaded (the
/// typical stock-colour-mode change), update the single vertex buffer in
/// place instead of recreating GPU buffers. Unkeyed: the colour update
/// compares its own fingerprint.
pub(super) fn upload_sim_mesh(app: &RsCamApp, gpu: &Gpu<'_>, resources: &mut RenderResources) {
    let simulation = &app.controller.state().simulation;
    if simulation.has_results() {
        let playback = &simulation.playback;
        if let Some(mesh) = &playback.display_mesh {
            let colors = app.compute_sim_colors(mesh, playback.display_mesh_is_final);
            let can_update_existing = resources
                .sim_mesh_data
                .as_ref()
                .is_some_and(|data| data.chunks.len() == 1);
            if can_update_existing && let Some(data) = resources.sim_mesh_data.as_mut() {
                data.update_colors_if_changed(gpu.queue, mesh, &colors);
            } else {
                let new_data = SimMeshGpuData::from_heightmap_mesh_colored(
                    gpu.device,
                    &resources.gpu_limits,
                    mesh,
                    &colors,
                );
                resources.sim_mesh_data = new_data;
            }
        }
    } else {
        resources.sim_mesh_data = None;
        resources.tool_model_data = None;
    }
}

/// Everything the collision-marker buffer is built from.
pub(super) struct CollisionInputs<'a> {
    /// The marker positions, in the emission frame.
    pub(super) positions: &'a [[f32; 3]],
    /// The emission → display shift applied to every marker.
    pub(super) shift: rs_cam_core::geo::P3,
}

impl SlotInputs for CollisionInputs<'_> {
    type Key = upload_cache::CollisionUploadKey;

    fn key(&self) -> Self::Key {
        let Self { positions, shift } = self;
        upload_cache::CollisionUploadKey {
            positions: positions.to_vec(),
            shift: shift_key(*shift),
        }
    }
}

impl CollisionInputs<'_> {
    /// Does the stored key already describe these markers? It reads the
    /// stored positions against the borrowed slice and allocates nothing.
    /// The same fields as [`SlotInputs::key`], destructured the same way.
    fn is_fresh(&self, stored: Option<&upload_cache::CollisionUploadKey>) -> bool {
        let Self { positions, shift } = self;
        match (stored, positions.is_empty()) {
            // Nothing uploaded and nothing to upload.
            (None, true) => true,
            (Some(key), false) => key.positions == *positions && key.shift == shift_key(*shift),
            _ => false,
        }
    }
}

/// Upload the collision markers with density-based heatmap colouring.
/// Nearby collisions cluster to brighter red; isolated ones are dimmer
/// yellow.
///
/// V8: the density pass is O(n²) (V12), so it is worth an O(n) comparison
/// against the marker set the current buffer was built from. Collisions only
/// move when a simulation runs, so every other upload pass skips this
/// entirely.
///
/// SHL-05: this slot keeps its own compare rather than taking
/// `upload_cache::refresh`. The helper adopts an OWNED key, so routing this
/// site through it would copy the position vector on every pass, including
/// the fresh one — and the fresh pass is the one the keys exist to make
/// free. [`CollisionInputs::is_fresh`] reads the stored positions against a
/// borrowed slice and allocates nothing.
pub(super) fn upload_collisions(
    inputs: &CollisionInputs<'_>,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    if inputs.is_fresh(resources.collision_upload_key.as_ref()) {
        // Buffer already matches the current markers — leave it alone.
    } else if !inputs.positions.is_empty() {
        resources.upload_stats.collision_builds += 1;
        resources.collision_upload_key = Some(inputs.key());
        build_collisions(inputs, gpu, resources);
    } else {
        resources.upload_stats.collision_builds += 1;
        resources.collision_upload_key = None;
        resources.collision_vertex_buffer = None;
        resources.collision_vertex_count = 0;
    }
}

fn build_collisions(inputs: &CollisionInputs<'_>, gpu: &Gpu<'_>, resources: &mut RenderResources) {
    use crate::render::LineVertex;
    let &CollisionInputs {
        positions,
        shift: display_shift,
    } = inputs;
    let s = 1.0f32; // marker size in mm
    let cluster_radius = 5.0_f32; // mm radius for density estimation

    // Precompute density for each collision point
    let densities: Vec<usize> = positions
        .iter()
        .map(|p| {
            positions
                .iter()
                .filter(|q| {
                    let dx = p[0] - q[0];
                    let dy = p[1] - q[1];
                    let dz = p[2] - q[2];
                    dx * dx + dy * dy + dz * dz < cluster_radius * cluster_radius
                })
                .count()
        })
        .collect();
    let max_density = densities.iter().copied().max().unwrap_or(1).max(1);

    let vertex_size = std::mem::size_of::<LineVertex>();
    let mut verts = Vec::new();
    for (i, p) in positions.iter().enumerate() {
        if verts.len() * vertex_size >= resources.gpu_limits.max_buffer_size {
            tracing::warn!(
                markers = positions.len(),
                "Too many collision markers — truncating to fit GPU buffer"
            );
            break;
        }
        // Collision positions come from the sim trace in the emission
        // frame — shift identity-setup markers into the display frame
        // alongside the toolpath lines.
        let p = [
            p[0] + display_shift.x as f32,
            p[1] + display_shift.y as f32,
            p[2] + display_shift.z as f32,
        ];
        // SAFETY: densities has same length as positions
        #[allow(clippy::indexing_slicing)]
        let t = densities[i] as f32 / max_density as f32;
        // Yellow (isolated) → Red (clustered)
        let color = [0.95, 0.8 * (1.0 - t) + 0.1, 0.1 * (1.0 - t)];
        verts.push(LineVertex {
            position: [p[0] - s, p[1], p[2]],
            color,
        });
        verts.push(LineVertex {
            position: [p[0] + s, p[1], p[2]],
            color,
        });
        verts.push(LineVertex {
            position: [p[0], p[1] - s, p[2]],
            color,
        });
        verts.push(LineVertex {
            position: [p[0], p[1] + s, p[2]],
            color,
        });
        verts.push(LineVertex {
            position: [p[0], p[1], p[2] - s],
            color,
        });
        verts.push(LineVertex {
            position: [p[0], p[1], p[2] + s],
            color,
        });
    }
    resources.collision_vertex_buffer = crate::render::gpu_safety::try_create_buffer(
        gpu.device,
        &resources.gpu_limits,
        "collision_markers",
        bytemuck::cast_slice(&verts),
        egui_wgpu::wgpu::BufferUsages::VERTEX,
    );
    resources.collision_vertex_count = if resources.collision_vertex_buffer.is_some() {
        verts.len() as u32
    } else {
        0
    };
}
