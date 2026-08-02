//! Synthetic mesh generators shared by the finishing / reach / rest sentries.
//!
//! Every generator here was copied between test files before it lived in one
//! place, so the top priority is **bit-for-bit reproduction of the originals**:
//! several consumers pin toolpath fingerprints computed over these vertices,
//! and a re-derived-but-slightly-different mesh would move a pinned constant
//! for no reason anyone could later explain. Vertex order, triangle winding
//! and floating-point operand order are all deliberate. Change them only with
//! a fingerprint re-capture and a note saying why.
//!
//! Reference consumers:
//!
//! | fixture | reference consumers |
//! |---|---|
//! | [`plateau`] | `grid_z_uncovered_contract_c2.rs` |
//! | [`grooved_block`] / [`GroovedBlock`] | `checkpoint_a_valley_matrix.rs`; also matches the local copies in `reach_policy_pr4.rs`, `pencil_tip_float_channel_d1.rs`, `coverage_routing_pr5.rs`, `generic_rest_routing_pr7.rs` |
//! | [`sawtooth_plate`] | `standing_material_channel_am9.rs` |
//! | [`height_field`] / [`height_field_grid`] | `checkpoint_b_resolution_ab.rs` fixtures |
//! | [`extrude_profile`] | `unified_finish_tapered_end_to_end_m21.rs` |
//! | [`plate_with_hole`] / [`stacked_shelf`] / [`non_manifold_fin`] | `classification_strategy_m3.rs` |

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::TriangleMesh;

// ── Height fields ───────────────────────────────────────────────────────

/// The height-field primitive: sample `z(x, y)` on a regular grid and
/// triangulate it.
///
/// Vertices run Y-outer / X-inner; each quad becomes `[a, c, b]` + `[b, c, d]`
/// so the normals point +Z. This is the exact winding the checkpoint-B
/// fixtures and the A/M9 corrugation were both built with.
pub fn height_field_grid(
    x0: f64,
    x_step: f64,
    nx: usize,
    y0: f64,
    y_step: f64,
    ny: usize,
    z: impl Fn(f64, f64) -> f64,
) -> TriangleMesh {
    let mut vertices = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        let y = y0 + j as f64 * y_step;
        for i in 0..nx {
            let x = x0 + i as f64 * x_step;
            vertices.push(P3::new(x, y, z(x, y)));
        }
    }
    let mut triangles = Vec::with_capacity((nx - 1) * (ny - 1) * 2);
    for j in 0..(ny - 1) {
        for i in 0..(nx - 1) {
            let a = (j * nx + i) as u32;
            let b = a + 1;
            let c = ((j + 1) * nx + i) as u32;
            let d = c + 1;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// Square height field over `[-half, half]²` at a uniform `step`.
pub fn height_field(half: f64, step: f64, z: impl Fn(f64, f64) -> f64) -> TriangleMesh {
    let n = ((2.0 * half) / step).round() as usize + 1;
    height_field_grid(-half, step, n, -half, step, n, z)
}

/// Corrugated plate — a triangular-wave surface with sharp convex apexes,
/// Y-invariant.
///
/// The texture has to be coarse enough that a ball FOLLOWS it: a ball bridges
/// any feature below its own radius and the offset surface reads flat
/// (`finish_setup.rs`'s documented blind spot), which is why callers pick a
/// period around 1.3× the tool diameter rather than as fine as possible.
///
/// Sampled at `period / 16` along X and 2 mm along Y — the along-Y spacing is
/// coarse on purpose, since nothing about the fixture varies with Y.
pub fn sawtooth_plate(half: f64, period: f64, amplitude: f64) -> TriangleMesh {
    let step_x = period / 16.0;
    let step_y = 2.0;
    let nx = ((2.0 * half) / step_x).round() as usize + 1;
    let ny = ((2.0 * half) / step_y).round() as usize + 1;
    height_field_grid(-half, step_x, nx, -half, step_y, ny, |x, _y| {
        let phase = x / period - (x / period).floor();
        let ridge = 1.0 - (2.0 * phase - 1.0).abs();
        amplitude * ridge
    })
}

// ── Closed solids ───────────────────────────────────────────────────────

/// A `size × size` top face at `z = 0` with vertical walls down to
/// `z = -depth` and a bottom face.
///
/// The point of the fixture is that a drop-cutter grid can only ever see the
/// TOP face — every wall Z is below `min_covered_z()` — while the mesh has
/// real material all the way down. That is what makes it the C2 grid-coverage
/// fixture and the steep/shallow ladder-bottom fixture.
pub fn plateau(size: f64, depth: f64) -> TriangleMesh {
    let h = size / 2.0;
    let b = -depth;
    let v = vec![
        P3::new(-h, -h, 0.0),
        P3::new(h, -h, 0.0),
        P3::new(h, h, 0.0),
        P3::new(-h, h, 0.0),
        P3::new(-h, -h, b),
        P3::new(h, -h, b),
        P3::new(h, h, b),
        P3::new(-h, h, b),
    ];
    let t = vec![
        // top
        [0, 1, 2],
        [0, 2, 3],
        // bottom
        [4, 6, 5],
        [4, 7, 6],
        // walls
        [0, 4, 5],
        [0, 5, 1],
        [1, 5, 6],
        [1, 6, 2],
        [2, 6, 7],
        [2, 7, 3],
        [3, 7, 4],
        [3, 4, 0],
    ];
    TriangleMesh::from_raw(v, t)
}

/// Extrude a Y-invariant cross-section profile into a mesh. Both faces point
/// +Z (winding taken from `tapered_cusp_radius_sentry::ribbon_mesh`).
pub fn extrude_profile(profile: &[(f64, f64)], y0: f64, y1: f64) -> TriangleMesh {
    let mut vertices = Vec::with_capacity(profile.len() * 2);
    for &(x, z) in profile {
        vertices.push(P3::new(x, y0, z));
        vertices.push(P3::new(x, y1, z));
    }
    let mut triangles = Vec::with_capacity((profile.len() - 1) * 2);
    for i in 0..profile.len() - 1 {
        let (a, b) = (2 * i as u32, 2 * i as u32 + 2);
        let (c, d) = (2 * i as u32 + 3, 2 * i as u32 + 1);
        triangles.push([a, b, c]);
        triangles.push([a, c, d]);
    }
    TriangleMesh::from_raw(vertices, triangles)
}

// ── Grooved block ───────────────────────────────────────────────────────

/// A straight trapezoidal groove cut into a flat block whose top is `z = 0`.
///
/// Five test files carried a private copy of this generator, differing only in
/// how densely they sampled X across the groove, whether they forced the
/// profile breakpoints into the sample set, and whether the two walls had the
/// same angle. Those are exactly the four knobs below; every other line —
/// including the `z_at` branch order and the operand order inside
/// `-depth + (ax - floor) * tan` — is reproduced verbatim so the meshes are
/// bit-identical to the copies they replace.
///
/// The default profile is the symmetric one from `checkpoint_a` /
/// `reach_policy_pr4` / `pencil_tip_float_channel_d1`; call
/// [`GroovedBlock::skew`] for the asymmetric variant `coverage_routing_pr5`
/// and `generic_rest_routing_pr7` use.
#[derive(Clone, Copy, Debug)]
pub struct GroovedBlock {
    rim_half_width: f64,
    wall_deg: f64,
    depth: f64,
    skew: f64,
    dense_half_width: f64,
    dense_step: f64,
    breakpoints: bool,
    x_extent: f64,
    coarse_step: f64,
    y_half: f64,
    y_step: f64,
}

impl GroovedBlock {
    /// Rim edges at `x = ±rim_half_width`, walls inclined `wall_deg` from
    /// horizontal, floor at `z = -depth`.
    pub fn new(rim_half_width: f64, wall_deg: f64, depth: f64) -> Self {
        Self {
            rim_half_width,
            wall_deg,
            depth,
            skew: 1.0,
            dense_half_width: 3.0,
            dense_step: 0.05,
            breakpoints: true,
            x_extent: 20.0,
            coarse_step: 1.0,
            y_half: 12.0,
            y_step: 1.0,
        }
    }

    /// Right-wall angle multiplier. `1.0` (the default) is symmetric; the
    /// asymmetric fixtures use e.g. `0.45` to make the two walls disagree.
    pub fn skew(mut self, skew: f64) -> Self {
        self.skew = skew;
        self
    }

    /// Half-width of the finely-sampled X window around the groove. Must
    /// comfortably contain `rim_half_width`.
    pub fn dense_half_width(mut self, half_width: f64) -> Self {
        self.dense_half_width = half_width;
        self
    }

    /// X sample spacing inside the dense window (default 0.05 mm;
    /// `checkpoint_a` uses 0.1).
    pub fn dense_step(mut self, step: f64) -> Self {
        self.dense_step = step;
        self
    }

    /// Whether to force the four profile breakpoints (`±rim`, `±floor`) into
    /// the X sample set so the mesh reproduces the corners exactly. The
    /// asymmetric fixtures deliberately do not.
    pub fn breakpoints(mut self, on: bool) -> Self {
        self.breakpoints = on;
        self
    }

    pub fn build(self) -> TriangleMesh {
        let tan_l = self.wall_deg.to_radians().tan();
        let tan_r = (self.wall_deg * self.skew).to_radians().tan();
        let floor_l = self.rim_half_width - self.depth / tan_l;
        let floor_r = self.rim_half_width - self.depth / tan_r;
        assert!(
            floor_l > 0.0 && floor_r > 0.0,
            "groove is a V, not a trapezoid"
        );
        let z_at = |x: f64| -> f64 {
            if x <= -self.rim_half_width || x >= self.rim_half_width {
                0.0
            } else if x < 0.0 {
                let ax = -x;
                if ax <= floor_l {
                    -self.depth
                } else {
                    -self.depth + (ax - floor_l) * tan_l
                }
            } else if x <= floor_r {
                -self.depth
            } else {
                -self.depth + (x - floor_r) * tan_r
            }
        };

        // X samples: dense across the groove, coarse outside, optionally with
        // every breakpoint present exactly.
        let mut xs: Vec<f64> = Vec::new();
        let mut x = -self.x_extent;
        while x < -self.dense_half_width {
            xs.push(x);
            x += self.coarse_step;
        }
        let mut x = -self.dense_half_width;
        while x <= self.dense_half_width + 1e-9 {
            xs.push(x);
            x += self.dense_step;
        }
        if self.breakpoints {
            for b in [-self.rim_half_width, -floor_l, floor_r, self.rim_half_width] {
                xs.push(b);
            }
        }
        let mut x = self.dense_half_width + self.coarse_step;
        while x <= self.x_extent + 1e-9 {
            xs.push(x);
            x += self.coarse_step;
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

        let ys: Vec<f64> = (0..=((2.0 * self.y_half / self.y_step).round() as usize))
            .map(|i| -self.y_half + i as f64 * self.y_step)
            .collect();

        let mut verts = Vec::with_capacity(xs.len() * ys.len());
        for &yv in &ys {
            for &xv in &xs {
                verts.push(P3::new(xv, yv, z_at(xv)));
            }
        }
        let nx = xs.len();
        let mut tris: Vec<[u32; 3]> = Vec::new();
        for j in 0..ys.len() - 1 {
            for i in 0..nx - 1 {
                let a = (j * nx + i) as u32;
                let b = (j * nx + i + 1) as u32;
                let c = ((j + 1) * nx + i + 1) as u32;
                let d = ((j + 1) * nx + i) as u32;
                tris.push([a, b, c]);
                tris.push([a, c, d]);
            }
        }
        TriangleMesh::from_raw(verts, tris)
    }
}

/// Shorthand for the default symmetric groove.
pub fn grooved_block(rim_half_width: f64, wall_deg: f64, depth: f64) -> TriangleMesh {
    GroovedBlock::new(rim_half_width, wall_deg, depth).build()
}

// ── Pathological-topology fixtures (M3) ─────────────────────────────────
//
// The M3 classification study needs meshes that break the "one clean
// upward-facing surface per XY point" assumption every grid sampler is
// implicitly written against. Each generator below isolates ONE way that
// assumption fails, so a sampler disagreement can be attributed instead of
// merely observed.

/// A square plate at `z = top` with a square hole through it — an **open**
/// mesh with genuinely uncovered interior cells.
///
/// The hole is the point: a vertical ray through it hits nothing, while a
/// cutter with radius still rides the rim and reports a contact height. Any
/// sampler that derives coverage from the Z value rather than from a
/// point-in-triangle test gets this fixture wrong. Generalised from the
/// private `ring_plate_mesh` in `slope.rs`'s coverage tests (same vertex
/// order and winding, so the two agree cell for cell).
pub fn plate_with_hole(size: f64, hole: f64, top: f64) -> TriangleMesh {
    let o = size;
    let lo = (size - hole) / 2.0;
    let hi = lo + hole;
    let v = vec![
        P3::new(0.0, 0.0, top),
        P3::new(o, 0.0, top),
        P3::new(o, o, top),
        P3::new(0.0, o, top),
        P3::new(lo, lo, top),
        P3::new(hi, lo, top),
        P3::new(hi, hi, top),
        P3::new(lo, hi, top),
    ];
    let t = vec![
        [0u32, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    TriangleMesh::from_raw(v, t)
}

/// A ground plate at `z = 0` with a smaller shelf floating `lift` above it,
/// wound **face down**.
///
/// Two surfaces project onto the same cells, and the upper one is the one an
/// orientation test would reject. The topmost surface is still the shelf, so
/// this fixture separates "take the highest surface" from "take the highest
/// upward-facing surface" — the M3 plan's stacked-triangle edge case.
pub fn stacked_shelf(size: f64, shelf: f64, lift: f64) -> TriangleMesh {
    let h = size / 2.0;
    let s = shelf / 2.0;
    let v = vec![
        P3::new(-h, -h, 0.0),
        P3::new(h, -h, 0.0),
        P3::new(h, h, 0.0),
        P3::new(-h, h, 0.0),
        P3::new(-s, -s, lift),
        P3::new(s, -s, lift),
        P3::new(s, s, lift),
        P3::new(-s, s, lift),
    ];
    let t = vec![
        [0u32, 1, 2],
        [0, 2, 3],
        // Reversed winding: normals point −Z.
        [4, 6, 5],
        [4, 7, 6],
    ];
    TriangleMesh::from_raw(v, t)
}

/// A plate at `z = 0` carrying a **non-manifold fin** plus a **detached
/// flyer**.
///
/// - The plate is two triangles sharing the diagonal `v0→v2`.
/// - The fin is a third triangle on that same diagonal, tilted up to
///   `height` — so the edge has **three** incident faces and the fin has a
///   free boundary.
/// - The flyer is an unconnected triangle floating at `2 × height`, part of
///   no shell.
///
/// A sampler that assumes a closed, orientable manifold (paired edges,
/// winding numbers, inside/outside tests) is wrong on all three; one that
/// reduces over faces is not. This is the M3 plan's non-manifold/open-mesh
/// edge case, and the flyer additionally checks that "topmost" really means
/// topmost rather than "topmost thing attached to the ground".
pub fn non_manifold_fin(size: f64, height: f64) -> TriangleMesh {
    let h = size / 2.0;
    let v = vec![
        P3::new(-h, -h, 0.0),
        P3::new(h, -h, 0.0),
        P3::new(h, h, 0.0),
        P3::new(-h, h, 0.0),
        // Fin apex, over the middle of the v0→v2 diagonal, offset in −X so
        // the fin is a genuine ramp rather than a vertical sheet.
        P3::new(-h * 0.4, h * 0.4, height),
        // Detached flyer.
        P3::new(-h * 0.3, -h * 0.3, height * 2.0),
        P3::new(h * 0.3, -h * 0.3, height * 2.0),
        P3::new(0.0, h * 0.3, height * 2.0),
    ];
    let t = vec![
        [0u32, 1, 2],
        [0, 2, 3],
        // Third face on the v0→v2 edge.
        [0, 2, 4],
        // Flyer.
        [5, 6, 7],
    ];
    TriangleMesh::from_raw(v, t)
}
