//! SEL-2: a ray pick through `MeshBvh` gives the same answer as the
//! brute-force `ray_pick_triangle`, bit for bit.
//!
//! The claim (design C §2.4): `MeshBvh::cast_ray` returns the same triangle
//! index and the same `t` (equal `to_bits`) as `ray_pick_triangle` on every
//! ray, and `MeshBvh::cast_ray_all` returns the brute-force hit list sorted
//! by `(t, index)`. The test casts these rays:
//!
//! - on `tests/fixtures/terrain.stl` (about 215k triangles): an eye outside
//!   the box over a 10 × 10 grid; an eye INSIDE the box over 8 × 8
//!   directions, shallow ones included; 8 × 8 rays straight down (two zero
//!   direction components);
//! - on a grid hemisphere (70 × 70 quads) and a ring hemisphere with an apex
//!   fan: rays through mesh vertices and edge midpoints, where two or more
//!   triangles share the hit point.
//!
//! Each ray runs against four leaf sizes. The picks are compared on every
//! ray; the hit lists on every fourth ray (`ALL_EVERY`), for the run time.
//! On the hemispheres, every 31st (grid) and every 3rd (ring) vertex and
//! edge midpoint is a target. Non-vacuity: some rays hit and
//! some rays miss, and some misses pass the box gate.
//!
//! The hemispheres are built here. `mesh::make_test_hemisphere` is a
//! `test-support` door, and a binary that binds a door needs a
//! `required-features` row in `Cargo.toml`.
//!
//! Red proofs (run once, red, reverted): a prune on
//! `BoundingBox3::ray_intersect` in place of `slab_entry` makes the
//! inside-eye arm fail; a prune with no slack (`entry > best`) makes the
//! grid hemisphere fail (a flat sheet at `z = 0` has a pad of 0 in `z`).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use rs_cam_core::geo::{P3, V3};
use rs_cam_core::mesh::bvh::{DEFAULT_LEAF_SIZE, MeshBvh};
use rs_cam_core::mesh::{TriangleMesh, ray_pick_triangle};

/// The leaf sizes each ray runs against.
const LEAVES: [usize; 4] = [1, 4, DEFAULT_LEAF_SIZE, 16];

/// The hit lists are compared on every fourth ray: two brute-force passes
/// on every ray made the binary run for over 60 s in the unoptimised test
/// profile.
const ALL_EVERY: usize = 4;

/// The counts of one arm.
#[derive(Debug)]
struct Tally {
    /// The rays cast so far.
    rays: usize,
    /// The hit lists are compared on every `all_every`-th ray.
    all_every: usize,
    hits: usize,
    misses: usize,
    /// Misses where the ray passed the mesh box gate.
    gated_misses: usize,
}

impl Tally {
    fn new(all_every: usize) -> Self {
        Self {
            rays: 0,
            all_every,
            hits: 0,
            misses: 0,
            gated_misses: 0,
        }
    }
}

fn bits(hits: &[(usize, f64)]) -> Vec<(usize, u64)> {
    hits.iter().map(|&(i, t)| (i, t.to_bits())).collect()
}

/// The brute-force hit list: the same gate as `ray_pick_triangle`, every
/// face, sorted by `(t, index)`.
fn brute_all(mesh: &TriangleMesh, o: &P3, d: &V3) -> Vec<(usize, f64)> {
    if mesh.bbox.ray_intersect(o, d).is_none() {
        return Vec::new();
    }
    let mut hits: Vec<(usize, f64)> = mesh
        .faces
        .iter()
        .enumerate()
        .filter_map(|(i, f)| f.ray_intersect(o, d).map(|t| (i, t)))
        .collect();
    hits.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    hits
}

/// Cast one ray through brute force and every BVH; assert equal picks.
/// The hit lists are compared on every `all_every`-th ray of the arm.
fn check(mesh: &TriangleMesh, bvhs: &[MeshBvh], o: &P3, d: &V3, tally: &mut Tally, what: &str) {
    let brute = ray_pick_triangle(mesh, o, d).map(|(i, t)| (i, t.to_bits()));
    let all = tally
        .rays
        .is_multiple_of(tally.all_every)
        .then(|| bits(&brute_all(mesh, o, d)));
    tally.rays += 1;
    for (bvh, leaf) in bvhs.iter().zip(LEAVES) {
        let fast = bvh.cast_ray(mesh, o, d).map(|(i, t)| (i, t.to_bits()));
        assert_eq!(
            fast, brute,
            "{what}: cast_ray differs from brute force (leaf {leaf}) on origin {o:?} dir {d:?}"
        );
        if let Some(all) = &all {
            assert_eq!(
                &bits(&bvh.cast_ray_all(mesh, o, d)),
                all,
                "{what}: cast_ray_all differs from brute force (leaf {leaf}) on origin {o:?} dir {d:?}"
            );
        }
    }
    if brute.is_some() {
        tally.hits += 1;
    } else {
        tally.misses += 1;
        if mesh.bbox.ray_intersect(o, d).is_some() {
            tally.gated_misses += 1;
        }
    }
}

fn bvhs(mesh: &TriangleMesh) -> Vec<MeshBvh> {
    LEAVES
        .iter()
        .map(|&leaf| MeshBvh::build_with_leaf(mesh, leaf).expect("the mesh fits a u32 index"))
        .collect()
}

/// The box centre and the largest box extent.
fn centre_and_extent(mesh: &TriangleMesh) -> (P3, f64) {
    let b = &mesh.bbox;
    let c = P3::new(
        0.5 * (b.min.x + b.max.x),
        0.5 * (b.min.y + b.max.y),
        0.5 * (b.min.z + b.max.z),
    );
    let e = (b.max.x - b.min.x)
        .max(b.max.y - b.min.y)
        .max(b.max.z - b.min.z);
    (c, e)
}

/// The point at the grid fractions `(fx, fy)` of the box, at height `z`.
fn box_point(mesh: &TriangleMesh, fx: f64, fy: f64, z: f64) -> P3 {
    let b = &mesh.bbox;
    P3::new(
        b.min.x + fx * (b.max.x - b.min.x),
        b.min.y + fy * (b.max.y - b.min.y),
        z,
    )
}

/// The cell centres of an `n × n` grid over the unit square.
fn grid(n: usize) -> impl Iterator<Item = (f64, f64)> {
    (0..n).flat_map(move |i| {
        (0..n).map(move |j| ((i as f64 + 0.5) / n as f64, (j as f64 + 0.5) / n as f64))
    })
}

fn terrain() -> TriangleMesh {
    let path = common::repo_root().join("crates/rs_cam_core/tests/fixtures/terrain.stl");
    TriangleMesh::from_stl(common::require_fixture(&path)).expect("the terrain fixture loads")
}

#[test]
fn a_pick_on_the_terrain_equals_brute_force_sel2() {
    let mesh = terrain();
    assert!(
        mesh.faces.len() > 100_000,
        "the fixture is the large terrain"
    );
    let bvhs = bvhs(&mesh);
    let (c, e) = centre_and_extent(&mesh);

    // The probe eye, outside the box, over a 10 × 10 grid through the box.
    let mut outside = Tally::new(ALL_EVERY);
    let eye = c + V3::new(1.2 * e, -1.2 * e, 1.5 * e);
    for (fx, fy) in grid(10) {
        let d = (box_point(&mesh, fx, fy, c.z) - eye).normalize();
        check(&mesh, &bvhs, &eye, &d, &mut outside, "outside eye");
    }
    assert!(outside.hits > 0, "outside eye: no ray hit: {outside:?}");

    // An eye INSIDE the box, over 8 azimuths × 8 elevations from −80° to
    // +80°. The shallow rays cross the sheet far from the eye; the upward
    // rays miss after the gate.
    let mut inside = Tally::new(ALL_EVERY);
    let b = &mesh.bbox;
    let eye = P3::new(c.x, c.y, b.min.z + 0.75 * (b.max.z - b.min.z));
    for (fa, fe) in grid(8) {
        let azimuth = std::f64::consts::TAU * fa;
        let elevation = (fe - 0.5) * 160.0_f64.to_radians();
        let d = V3::new(
            elevation.cos() * azimuth.cos(),
            elevation.cos() * azimuth.sin(),
            elevation.sin(),
        );
        check(&mesh, &bvhs, &eye, &d, &mut inside, "inside eye");
    }
    assert!(inside.hits > 0, "inside eye: no ray hit: {inside:?}");
    assert!(
        inside.gated_misses > 0,
        "inside eye: no miss after the gate: {inside:?}"
    );

    // 8 × 8 rays straight down: two zero direction components.
    let mut down = Tally::new(ALL_EVERY);
    let d = V3::new(0.0, 0.0, -1.0);
    for (fx, fy) in grid(8) {
        let o = box_point(&mesh, fx, fy, b.max.z + 1.0);
        check(&mesh, &bvhs, &o, &d, &mut down, "straight down");
    }
    assert!(down.hits > 0, "straight down: no ray hit: {down:?}");

    // Two horizontal rays: one just below the box top, one just above the
    // box bottom. They pass the gate in the z slab and graze the sheet.
    let mut edge = Tally::new(1);
    for z in [b.max.z - 1e-9 * e, b.min.z + 1e-9 * e] {
        let o = P3::new(b.min.x - e, c.y, z);
        check(
            &mesh,
            &bvhs,
            &o,
            &V3::new(1.0, 0.0, 0.0),
            &mut edge,
            "horizontal",
        );
    }
    assert_eq!(edge.hits + edge.misses, 2);
}

/// A hemisphere over a `div × div` grid of quads, two triangles each; the
/// height is 0 outside the unit disc.
fn grid_hemisphere(radius: f64, div: usize) -> TriangleMesh {
    let mut vertices = Vec::new();
    for i in 0..=div {
        for j in 0..=div {
            let x = (i as f64 / div as f64) * 2.0 - 1.0;
            let y = (j as f64 / div as f64) * 2.0 - 1.0;
            let z = (1.0 - (x * x + y * y)).max(0.0).sqrt();
            vertices.push(P3::new(x * radius, y * radius, z * radius));
        }
    }
    let w = (div + 1) as u32;
    let mut triangles = Vec::new();
    for i in 0..div as u32 {
        for j in 0..div as u32 {
            let a = i * w + j;
            triangles.push([a, a + 1, a + w]);
            triangles.push([a + 1, a + w + 1, a + w]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// A hemisphere of latitude rings, with a fan of triangles at the apex:
/// `rings` rings of `4 · rings` vertices each.
fn ring_hemisphere(radius: f64, rings: usize) -> TriangleMesh {
    let around = 4 * rings;
    let mut vertices = vec![P3::new(0.0, 0.0, radius)];
    for i in 1..=rings {
        let phi = std::f64::consts::FRAC_PI_2 * (1.0 - i as f64 / rings as f64);
        for j in 0..around {
            let theta = std::f64::consts::TAU * j as f64 / around as f64;
            vertices.push(P3::new(
                radius * phi.cos() * theta.cos(),
                radius * phi.cos() * theta.sin(),
                radius * phi.sin(),
            ));
        }
    }
    let mut triangles = Vec::new();
    for j in 0..around {
        triangles.push([0, (1 + j) as u32, (1 + (j + 1) % around) as u32]);
    }
    for i in 0..rings - 1 {
        let (top, low) = (1 + i * around, 1 + (i + 1) * around);
        for j in 0..around {
            let k = (j + 1) % around;
            let (a, b) = ((top + j) as u32, (top + k) as u32);
            let (c, d) = ((low + j) as u32, (low + k) as u32);
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// The vertices of `mesh` and the midpoints of its triangle edges, every
/// `stride`-th one.
fn vertex_and_edge_targets(mesh: &TriangleMesh, stride: usize) -> Vec<P3> {
    let mut out: Vec<P3> = mesh.vertices.iter().copied().step_by(stride).collect();
    for f in mesh.faces.iter().step_by(stride) {
        out.push(nalgebra::center(&f.v[0], &f.v[1]));
        out.push(nalgebra::center(&f.v[1], &f.v[2]));
        out.push(nalgebra::center(&f.v[2], &f.v[0]));
    }
    out
}

/// Cast through each target: straight down, and from a tilted eye.
fn shared_point_rays(mesh: &TriangleMesh, stride: usize, what: &str) -> Tally {
    let bvhs = bvhs(mesh);
    let (c, e) = centre_and_extent(mesh);
    let top = mesh.bbox.max.z + 1.0;
    let eye = c + V3::new(1.2 * e, -1.2 * e, 1.5 * e);
    let mut tally = Tally::new(ALL_EVERY);
    for p in vertex_and_edge_targets(mesh, stride) {
        let down = V3::new(0.0, 0.0, -1.0);
        check(
            mesh,
            &bvhs,
            &P3::new(p.x, p.y, top),
            &down,
            &mut tally,
            what,
        );
        check(mesh, &bvhs, &eye, &(p - eye).normalize(), &mut tally, what);
        // From under the sheet, pointing away from it: a miss in the box.
        let away = V3::new(0.3, -0.2, -1.0).normalize();
        let below = P3::new(p.x, p.y, mesh.bbox.min.z + 1e-6 * e);
        check(mesh, &bvhs, &below, &away, &mut tally, what);
    }
    tally
}

#[test]
fn a_pick_on_shared_points_of_a_hemisphere_equals_brute_force_sel2() {
    let grid = shared_point_rays(&grid_hemisphere(100.0, 70), 31, "grid hemisphere");
    assert!(grid.hits > 0, "grid hemisphere: no ray hit: {grid:?}");
    assert!(
        grid.gated_misses > 0,
        "grid hemisphere: no miss after the gate: {grid:?}"
    );

    let ring = shared_point_rays(&ring_hemisphere(10.0, 16), 3, "ring hemisphere");
    assert!(ring.hits > 0, "ring hemisphere: no ray hit: {ring:?}");
    assert!(ring.misses > 0, "ring hemisphere: no ray missed: {ring:?}");
}
