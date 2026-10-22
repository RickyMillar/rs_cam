//! The reference-line buffer: the obstacles (K2), the alignment pins, the
//! flip axis and the datum crosshair share one line buffer.
//!
//! The CPU builders below take no GPU; the upload pass and the tests call
//! them. [`FixtureGpuData::from_lines`] makes the buffer.

use super::LineVertex;
use super::gpu_safety::{GpuLimits, try_create_buffer};
use egui_wgpu::wgpu;
use rs_cam_core::geo::P2;

/// The most vertical edges one prism draws. A silhouette ring from a
/// 0.5 mm raster has hundreds of vertices; a vertical at each one is noise.
pub const MAX_PRISM_VERTICALS: usize = 64;

/// A ring vertex is a corner when the ring turns there by more than this
/// angle (degrees). Only a corner gets a vertical edge.
const CORNER_TURN_DEG: f64 = 30.0;

/// The vertical edges a ring with no corner (a smooth curve) still draws,
/// so the prism reads as a solid.
const SMOOTH_RING_VERTICALS: usize = 4;

/// GPU data for the reference-line buffer.
pub struct FixtureGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
}

impl FixtureGpuData {
    /// A line buffer from line-list vertices. `None` when the list is empty
    /// or when the buffer is larger than the device limit.
    pub fn from_lines(
        device: &wgpu::Device,
        limits: &GpuLimits,
        vertices: &[LineVertex],
    ) -> Option<Self> {
        if vertices.is_empty() {
            return None;
        }
        let vertex_buffer = try_create_buffer(
            device,
            limits,
            "fixture_vertices",
            bytemuck::cast_slice(vertices),
            wgpu::BufferUsages::VERTEX,
        )?;
        Some(Self {
            vertex_buffer,
            vertex_count: u32::try_from(vertices.len()).ok()?,
        })
    }
}

/// The ring without a repeated closing vertex.
fn open_ring(ring: &[P2]) -> &[P2] {
    match ring {
        [first, .., last] if first == last => ring.get(..ring.len() - 1).unwrap_or(ring),
        _ => ring,
    }
}

fn vertex(p: &P2, z: f64, rgb: [f32; 3], shift: [f64; 3]) -> LineVertex {
    LineVertex {
        position: [
            (p.x + shift[0]) as f32,
            (p.y + shift[1]) as f32,
            (z + shift[2]) as f32,
        ],
        color: rgb,
    }
}

/// Push a closed ring at height `z` as line-list segments. Every vertex
/// takes `shift` (the setup's emission-to-display shift).
pub fn push_ring_lines(
    out: &mut Vec<LineVertex>,
    ring: &[P2],
    z: f64,
    rgb: [f32; 3],
    shift: [f64; 3],
) {
    let ring = open_ring(ring);
    if ring.len() < 2 {
        return;
    }
    for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
        out.push(vertex(a, z, rgb, shift));
        out.push(vertex(b, z, rgb, shift));
    }
}

/// The indices of the ring vertices that get a vertical edge: the corners
/// (a turn of more than [`CORNER_TURN_DEG`]), at most
/// [`MAX_PRISM_VERTICALS`]. A ring with no corner gets
/// [`SMOOTH_RING_VERTICALS`] evenly spaced edges.
fn vertical_indices(ring: &[P2]) -> Vec<usize> {
    let n = ring.len();
    if n < 3 {
        return (0..n).collect();
    }
    let cos_limit = CORNER_TURN_DEG.to_radians().cos();
    let prevs = ring.iter().cycle().skip(n - 1);
    let nexts = ring.iter().cycle().skip(1);
    let corners: Vec<usize> = ring
        .iter()
        .zip(prevs.zip(nexts))
        .enumerate()
        .filter(|(_, (here, (prev, next)))| {
            let a = *here - *prev;
            let b = *next - *here;
            let (la, lb) = (a.norm(), b.norm());
            if la <= f64::EPSILON || lb <= f64::EPSILON {
                return false;
            }
            a.dot(&b) / (la * lb) < cos_limit
        })
        .map(|(i, _)| i)
        .collect();
    let picked = if corners.is_empty() {
        let count = SMOOTH_RING_VERTICALS.min(n);
        (0..count).map(|k| k * n / count).collect()
    } else {
        corners
    };
    if picked.len() <= MAX_PRISM_VERTICALS {
        return picked;
    }
    let stride = picked.len().div_ceil(MAX_PRISM_VERTICALS);
    picked.into_iter().step_by(stride).collect()
}

/// Push a prism: the ring at `z0`, the ring at `z1`, and a vertical edge at
/// each corner of the ring (see [`MAX_PRISM_VERTICALS`]). Every vertex
/// takes `shift`.
#[expect(
    clippy::indexing_slicing,
    reason = "the indices come from `vertical_indices`, which reads only `0..ring.len()`."
)]
pub fn push_prism_lines(
    out: &mut Vec<LineVertex>,
    ring: &[P2],
    z0: f64,
    z1: f64,
    rgb: [f32; 3],
    shift: [f64; 3],
) {
    let ring = open_ring(ring);
    push_ring_lines(out, ring, z0, rgb, shift);
    push_ring_lines(out, ring, z1, rgb, shift);
    for i in vertical_indices(ring) {
        out.push(vertex(&ring[i], z0, rgb, shift));
        out.push(vertex(&ring[i], z1, rgb, shift));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_prism_is_two_rings_and_four_verticals_k2() {
        let ring = [
            P2::new(0.0, 0.0),
            P2::new(10.0, 0.0),
            P2::new(10.0, 5.0),
            P2::new(0.0, 5.0),
        ];
        let shift = [1.0, 2.0, 3.0];
        let mut out = Vec::new();
        push_prism_lines(&mut out, &ring, 0.0, 20.0, [1.0, 0.0, 0.0], shift);
        assert_eq!(out.len(), 2 * 4 * 2 + 4 * 2);
        assert_eq!(out.len(), 24);
        for v in &out {
            let x = f64::from(v.position[0]) - shift[0];
            let y = f64::from(v.position[1]) - shift[1];
            let z = f64::from(v.position[2]) - shift[2];
            assert!(
                ring.iter()
                    .any(|p| (p.x - x).abs() < 1e-6 && (p.y - y).abs() < 1e-6),
                "every vertex carries the shift: {:?}",
                v.position
            );
            assert!(z.abs() < 1e-6 || (z - 20.0).abs() < 1e-6);
        }
    }

    #[test]
    fn a_dense_ring_gets_at_most_the_vertical_cap_k2() {
        // A 400-vertex star: every other vertex is a sharp corner.
        let ring: Vec<P2> = (0..400)
            .map(|i| {
                let a = f64::from(i) * std::f64::consts::TAU / 400.0;
                let r = if i % 2 == 0 { 10.0 } else { 7.0 };
                P2::new(r * a.cos(), r * a.sin())
            })
            .collect();
        let mut out = Vec::new();
        push_prism_lines(&mut out, &ring, 0.0, 1.0, [1.0; 3], [0.0; 3]);
        let verticals = (out.len() - 2 * 400 * 2) / 2;
        assert!(verticals > 0, "a ring with corners draws verticals");
        assert!(verticals <= MAX_PRISM_VERTICALS, "{verticals}");

        // A smooth 400-vertex circle has no corner and draws a few edges.
        let circle: Vec<P2> = (0..400)
            .map(|i| {
                let a = f64::from(i) * std::f64::consts::TAU / 400.0;
                P2::new(10.0 * a.cos(), 10.0 * a.sin())
            })
            .collect();
        let mut out = Vec::new();
        push_prism_lines(&mut out, &circle, 0.0, 1.0, [1.0; 3], [0.0; 3]);
        assert_eq!((out.len() - 2 * 400 * 2) / 2, SMOOTH_RING_VERTICALS);
    }
}
