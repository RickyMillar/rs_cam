//! Fixtures shared by more than one bench target.
//!
//! Each bench is its own crate, so a generator two of them need was copied
//! rather than shared. Only fixtures that two targets already build the same
//! way belong here; a fixture one target owns stays with it.

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::TriangleMesh;

/// A rolling height field with enough triangles that the spatial index has
/// real work to do, but small enough to build in milliseconds.
///
/// Two incommensurate ripples plus a ridge: slopes from flat to near-vertical,
/// so a classifier sees every band. `n` vertices per side → `2·(n−1)²`
/// triangles. `n = 121` gives 28 800.
pub fn rolling_field(half: f64, n: usize) -> TriangleMesh {
    let step = 2.0 * half / (n - 1) as f64;
    let mut vertices = Vec::with_capacity(n * n);
    for iy in 0..n {
        let y = -half + iy as f64 * step;
        for ix in 0..n {
            let x = -half + ix as f64 * step;
            let z = 1.6 * (x * 0.9).sin() * (y * 0.7).cos() + 0.9 * (x * 2.3 + y * 1.7).sin()
                - 0.35 * (x * x + y * y).sqrt();
            vertices.push(P3::new(x, y, z));
        }
    }
    let mut triangles = Vec::with_capacity(2 * (n - 1) * (n - 1));
    for iy in 0..n - 1 {
        for ix in 0..n - 1 {
            let a = (iy * n + ix) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}
