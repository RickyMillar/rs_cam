//! `MeshBvh`: a bounding-volume hierarchy over the triangles of one
//! `TriangleMesh`, for ray picks (SEL-2).
//!
//! The BVH holds triangle indices and padded node boxes only. Each cast
//! takes the mesh, and each leaf runs the same `Triangle::ray_intersect` as
//! the brute-force `ray_pick_triangle`. The build is single-thread and uses
//! no hash map, so two builds of one mesh give the same layout.
//!
//! # Why a cast equals brute force
//!
//! `cast_ray` returns the same `(index, t)` as `ray_pick_triangle`, bit for
//! bit: the nearest `t`, and the lowest index when two `t` values are
//! bit-equal. The argument has four steps.
//!
//! 1. The gate. The cast first calls `mesh.bbox.ray_intersect`, the same
//!    call that brute force makes. So the two reject the same rays before a
//!    triangle test.
//! 2. The pad. The triangle test accepts a point `w0·v0 + w1·v1 + w2·v2`
//!    with `Σw = 1` and each `w ≥ −RAY_BARY_EPS`. On one axis, a negative
//!    weight moves the point at most `RAY_BARY_EPS` times the triangle
//!    extent outside the triangle box, and at most two weights are
//!    negative. Each node box grows by `pad(lo, hi)`: four times
//!    `RAY_BARY_EPS` times the node extent, plus eight ulps of the largest
//!    coordinate for the slab arithmetic. A node extent is never smaller
//!    than the extent of a triangle in it. So the slab test never rejects a
//!    node that holds a triangle that brute force hits. Limit: when `|det|`
//!    is near the `RAY_BARY_EPS` floor, the rounding of `u`, `v` and `t`
//!    grows as `1/det` and can pass the pad. The proof does not cover such
//!    a graze.
//! 3. The prune. A hit point lies inside its padded leaf box, so the true
//!    box entry is at most the true hit `t`. The computed entry and the
//!    computed hit `t` each carry a rounding error. On a flat axis at the
//!    coordinate 0 the pad is 0, so the box face and the triangle are one
//!    plane, and the computed entry can be one ulp above the computed hit
//!    `t` (the parity sentry found this case on a flat sheet at `z = 0`).
//!    So the cast skips a node only when its entry is STRICTLY above
//!    `best · (1 + RAY_PRUNE_REL)`, with `RAY_PRUNE_REL = 4 · RAY_BARY_EPS`.
//!    This slack is far above the few ulps of the slab arithmetic. It keeps
//!    every node that can hold a triangle with `t <= best`, a tie included.
//!    Limit: as in step 2, a graze with `|det|` near the floor can carry a
//!    larger error in `t`; the proof does not cover it. The entry
//!    comes from `slab_entry`, which gives `max(entry, 0)`. It does not use
//!    `BoundingBox3::ray_intersect`: that function gives the EXIT `t` when
//!    the origin is inside the box, and a prune on the exit skips a nearer
//!    triangle when the eye is inside the model box.
//! 4. The order. Each `t` comes from the same function with the same
//!    arguments, so it is bit-equal to the brute-force `t`. `better` is a
//!    total order on `(t, index)`, and the minimum of a total order does not
//!    change with the visit order. So the leaf size, the split rule and the
//!    child order change the speed only.
//!
//! `cast_ray_all` uses the same gate and the same slab test with no prune.
//! It sorts the hits by `(t, index)`.
//!
//! The memory of the BVH is `heap_bytes`, from `size_of` of the real types;
//! the test `heap_bytes_counts_the_real_types` holds it.

use std::mem::size_of;

use thiserror::Error;

use crate::geo::{P3, RAY_BARY_EPS, Triangle, V3};
use crate::mesh::{TriangleMesh, ray_pick_triangle};

/// The default number of triangles in a leaf. It sets the speed of a cast
/// and the memory of the BVH (the node count is about `2n / L`).
pub const DEFAULT_LEAF_SIZE: usize = 8;

/// The relative part of the box pad: see step 2 of the module doc.
const RAY_PAD_REL: f64 = 4.0 * RAY_BARY_EPS;
/// The absolute part of the box pad, in ulps of the largest coordinate.
const RAY_PAD_ULPS: f64 = 8.0 * f64::EPSILON;
/// The relative slack of the prune, as a fraction of the best `t`: see
/// step 3 of the module doc. It takes the same fraction as the pad.
const RAY_PRUNE_REL: f64 = RAY_PAD_REL;

/// The pad of one axis of a node box with the bounds `lo..hi`.
fn pad(lo: f64, hi: f64) -> f64 {
    RAY_PAD_REL * (hi - lo) + RAY_PAD_ULPS * lo.abs().max(hi.abs())
}

/// One node, in depth-first pre-order: the left child of an interior node
/// is the next node.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Node {
    /// The low corner of the padded box.
    lo: [f64; 3],
    /// The high corner of the padded box.
    hi: [f64; 3],
    /// A leaf: the first slot in `order`. An interior node: the index of
    /// the right child.
    start: u32,
    /// A leaf: the triangle count (above 0). An interior node: 0.
    count: u32,
}

/// A bounding-volume hierarchy over the triangles of one `TriangleMesh`,
/// for ray picks. It holds indices only; every cast takes the mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshBvh {
    nodes: Vec<Node>,
    /// The triangle indices, leaf by leaf.
    order: Vec<u32>,
    /// `mesh.faces.len()` at the build. A cast on a mesh with another
    /// count falls back to brute force.
    triangle_count: usize,
}

/// The reason a BVH build fails.
#[derive(Debug, Error)]
pub enum BvhError {
    /// The node indices of the BVH are `u32`, and the node count is below
    /// twice the triangle count.
    #[error("{0} triangles exceed the u32 index of the pick BVH")]
    TooManyTriangles(usize),
}

impl MeshBvh {
    /// Build the BVH of `mesh` with `DEFAULT_LEAF_SIZE`.
    pub fn build(mesh: &TriangleMesh) -> Result<Self, BvhError> {
        Self::build_with_leaf(mesh, DEFAULT_LEAF_SIZE)
    }

    /// Build the BVH of `mesh` with at most `leaf_size` triangles in a leaf.
    /// A `leaf_size` of 0 builds as 1.
    pub fn build_with_leaf(mesh: &TriangleMesh, leaf_size: usize) -> Result<Self, BvhError> {
        let n = mesh.faces.len();
        // The node count is at most 2n - 1, and a node index is a u32.
        if u32::try_from(n.saturating_mul(2)).is_err() {
            return Err(BvhError::TooManyTriangles(n));
        }
        if n == 0 {
            // No node at all: a leaf with count 0 reads as an interior node
            // whose right child is itself, and a cast would not end.
            return Ok(Self {
                nodes: Vec::new(),
                order: Vec::new(),
                triangle_count: 0,
            });
        }
        let centroids: Vec<P3> = mesh
            .faces
            .iter()
            .map(|f| {
                P3::new(
                    0.5 * (f.bbox.min.x + f.bbox.max.x),
                    0.5 * (f.bbox.min.y + f.bbox.max.y),
                    0.5 * (f.bbox.min.z + f.bbox.max.z),
                )
            })
            .collect();
        let mut builder = Builder {
            faces: &mesh.faces,
            centroids: &centroids,
            order: (0..n).map(|i| i as u32).collect(),
            nodes: Vec::new(),
            leaf: leaf_size.max(1),
        };
        builder.node(0, n);
        let Builder {
            mut nodes,
            mut order,
            ..
        } = builder;
        nodes.shrink_to_fit();
        order.shrink_to_fit();
        Ok(Self {
            nodes,
            order,
            triangle_count: n,
        })
    }

    /// The nearest hit, with the contract of `ray_pick_triangle`: the
    /// nearest `t`, the lowest triangle index on a bit-equal `t`, and `None`
    /// on a miss. A mesh with another triangle count than the build mesh
    /// goes to `ray_pick_triangle`.
    pub fn cast_ray(&self, mesh: &TriangleMesh, origin: &P3, dir: &V3) -> Option<(usize, f64)> {
        if mesh.faces.len() != self.triangle_count {
            return ray_pick_triangle(mesh, origin, dir);
        }
        // The same gate as brute force.
        mesh.bbox.ray_intersect(origin, dir)?;
        let inv = inverse(dir);
        let mut best: Option<(usize, f64)> = None;
        let mut stack: Vec<usize> = Vec::with_capacity(64);
        if !self.nodes.is_empty() {
            stack.push(0);
        }
        while let Some(i) = stack.pop() {
            let Some(node) = self.nodes.get(i) else {
                continue;
            };
            let Some(entry) = slab_entry(node, origin, &inv) else {
                continue;
            };
            // STRICT, with a slack: a node whose entry ties the best `t`,
            // or rounds to just above it, is still read.
            if best.is_some_and(|(_, bt)| entry > bt + RAY_PRUNE_REL * bt) {
                continue;
            }
            if node.count > 0 {
                for &tri in self.leaf(node) {
                    let tri = tri as usize;
                    if let Some(t) = mesh
                        .faces
                        .get(tri)
                        .and_then(|f| f.ray_intersect(origin, dir))
                        && better(t, tri, best)
                    {
                        best = Some((tri, t));
                    }
                }
            } else {
                stack.push(node.start as usize);
                // The left child is popped first.
                stack.push(i + 1);
            }
        }
        best
    }

    /// Every hit, sorted by `(t, index)`. The gate and the slab test are the
    /// same as in `cast_ray`; no node is pruned.
    pub fn cast_ray_all(&self, mesh: &TriangleMesh, origin: &P3, dir: &V3) -> Vec<(usize, f64)> {
        let mut hits = Vec::new();
        if mesh.bbox.ray_intersect(origin, dir).is_none() {
            return hits;
        }
        if mesh.faces.len() != self.triangle_count {
            hits.extend(
                mesh.faces
                    .iter()
                    .enumerate()
                    .filter_map(|(i, f)| f.ray_intersect(origin, dir).map(|t| (i, t))),
            );
        } else {
            let inv = inverse(dir);
            let mut stack: Vec<usize> = Vec::with_capacity(64);
            if !self.nodes.is_empty() {
                stack.push(0);
            }
            while let Some(i) = stack.pop() {
                let Some(node) = self.nodes.get(i) else {
                    continue;
                };
                if slab_entry(node, origin, &inv).is_none() {
                    continue;
                }
                if node.count > 0 {
                    for &tri in self.leaf(node) {
                        let tri = tri as usize;
                        if let Some(t) = mesh
                            .faces
                            .get(tri)
                            .and_then(|f| f.ray_intersect(origin, dir))
                        {
                            hits.push((tri, t));
                        }
                    }
                } else {
                    stack.push(node.start as usize);
                    stack.push(i + 1);
                }
            }
        }
        hits.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        hits
    }

    /// The heap bytes of the BVH, from `size_of` of the real types.
    pub fn heap_bytes(&self) -> usize {
        self.nodes.capacity() * size_of::<Node>() + self.order.capacity() * size_of::<u32>()
    }

    /// The number of nodes. 0 for an empty mesh.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The triangle indices of a leaf. Empty for an interior node.
    fn leaf(&self, node: &Node) -> &[u32] {
        let start = node.start as usize;
        let end = start + node.count as usize;
        self.order.get(start..end).unwrap_or(&[])
    }
}

/// The brute-force order as a comparator: a nearer `t`, else a lower index.
fn better(t: f64, tri: usize, best: Option<(usize, f64)>) -> bool {
    best.is_none_or(|(bi, bt)| t < bt || (t == bt && tri < bi))
}

/// The inverse of each direction component. A zero component gives an
/// infinity, and `slab_entry` then uses a point-in-slab test.
fn inverse(dir: &V3) -> V3 {
    V3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z)
}

/// `Some(max(entry, 0))` when the ray meets the node box at a `t >= 0`,
/// else `None`. When `1/dir` is not finite on an axis (the component is 0
/// or subnormal), the origin must lie in the slab of that axis.
fn slab_entry(node: &Node, origin: &P3, inv: &V3) -> Option<f64> {
    let o = [origin.x, origin.y, origin.z];
    let inv = [inv.x, inv.y, inv.z];
    let (mut t0, mut t1) = (0.0_f64, f64::INFINITY);
    for (((&lo, &hi), &o), &inv) in node.lo.iter().zip(&node.hi).zip(&o).zip(&inv) {
        if !inv.is_finite() {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let near = (lo - o) * inv;
        let far = (hi - o) * inv;
        t0 = t0.max(near.min(far));
        t1 = t1.min(near.max(far));
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// The recursive builder. It owns the index order and the node vector
/// while they grow.
struct Builder<'a> {
    faces: &'a [Triangle],
    centroids: &'a [P3],
    order: Vec<u32>,
    nodes: Vec<Node>,
    leaf: usize,
}

impl Builder<'_> {
    /// Build the node of the slots `start..end` of `order` and its subtree.
    /// It returns the index of the node. The depth is at most about
    /// `log2(n)`, so the recursion is safe.
    fn node(&mut self, start: usize, end: usize) -> usize {
        let me = self.nodes.len();
        let (lo, hi) = self.padded_bounds(start, end);
        self.nodes.push(Node {
            lo,
            hi,
            start: 0,
            count: 0,
        });
        let len = end - start;
        if len <= self.leaf {
            if let Some(node) = self.nodes.get_mut(me) {
                node.start = start as u32;
                node.count = len as u32;
            }
            return me;
        }
        let axis = self.split_axis(start, end);
        let mid = start + len / 2;
        self.partition(start, end, mid, axis);
        self.node(start, mid);
        let right = self.node(mid, end);
        if let Some(node) = self.nodes.get_mut(me) {
            node.start = right as u32;
        }
        me
    }

    /// The union of the triangle boxes of `start..end`, padded on each axis
    /// by `pad`.
    fn padded_bounds(&self, start: usize, end: usize) -> ([f64; 3], [f64; 3]) {
        let mut lo = P3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = P3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        let slots = self.order.get(start..end).unwrap_or(&[]);
        for face in slots.iter().filter_map(|&i| self.faces.get(i as usize)) {
            lo = lo.inf(&face.bbox.min);
            hi = hi.sup(&face.bbox.max);
        }
        let (px, py, pz) = (pad(lo.x, hi.x), pad(lo.y, hi.y), pad(lo.z, hi.z));
        (
            [lo.x - px, lo.y - py, lo.z - pz],
            [hi.x + px, hi.y + py, hi.z + pz],
        )
    }

    /// The axis of the longest extent of the centroid bounds of
    /// `start..end`: 0, 1 or 2, the first axis on a tie.
    fn split_axis(&self, start: usize, end: usize) -> usize {
        let mut lo = P3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = P3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        let slots = self.order.get(start..end).unwrap_or(&[]);
        for c in slots.iter().filter_map(|&i| self.centroids.get(i as usize)) {
            lo = lo.inf(c);
            hi = hi.sup(c);
        }
        let e = hi - lo;
        if e.x >= e.y && e.x >= e.z {
            0
        } else if e.y >= e.z {
            1
        } else {
            2
        }
    }

    /// The object median on `axis`: after the call, the slots `start..mid`
    /// hold the lower centroids and `mid..end` the higher. The comparator
    /// breaks a tie by the triangle index, so it is a total order and the
    /// two sets do not depend on the select algorithm.
    #[expect(
        clippy::indexing_slicing,
        reason = "start..end is a slot range of this builder's order and each slot holds an index of 0..n of this mesh"
    )]
    fn partition(&mut self, start: usize, end: usize, mid: usize, axis: usize) {
        let centroids = self.centroids;
        let key = |i: u32| centroids[i as usize][axis];
        self.order[start..end].select_nth_unstable_by(mid - start, |&a, &b| {
            key(a).total_cmp(&key(b)).then(a.cmp(&b))
        });
    }
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    /// A height field of `cols × rows` cells, two triangles each, with
    /// heights from a fixed formula (no random source).
    fn terrain(cols: usize, rows: usize) -> TriangleMesh {
        let mut vertices = Vec::new();
        for j in 0..=rows {
            for i in 0..=cols {
                let (x, y) = (i as f64, j as f64);
                let z = (0.7 * x).sin() * 2.0 + (0.45 * y).cos() * 1.5 + 0.03 * x * y;
                vertices.push(P3::new(x, y, z));
            }
        }
        let w = (cols + 1) as u32;
        let mut triangles = Vec::new();
        for j in 0..rows as u32 {
            for i in 0..cols as u32 {
                let a = j * w + i;
                triangles.push([a, a + 1, a + w]);
                triangles.push([a + 1, a + w + 1, a + w]);
            }
        }
        TriangleMesh::from_raw(vertices, triangles)
    }

    /// `n` triangles: the first `n` faces of a terrain big enough to hold
    /// them.
    fn first_faces(n: usize) -> TriangleMesh {
        let big = terrain(n.div_ceil(2).max(1), 1);
        let triangles = big.triangles[..n].to_vec();
        TriangleMesh::from_raw(big.vertices, triangles)
    }

    fn bvh(mesh: &TriangleMesh, leaf: usize) -> MeshBvh {
        MeshBvh::build_with_leaf(mesh, leaf).expect("the test mesh is small")
    }

    fn box_holds(lo: &[f64; 3], hi: &[f64; 3], ilo: &[f64; 3], ihi: &[f64; 3]) -> bool {
        (0..3).all(|a| lo[a] <= ilo[a] && ihi[a] <= hi[a])
    }

    /// Check every structural invariant of `b` for the leaf size `leaf`.
    fn check_invariants(b: &MeshBvh, mesh: &TriangleMesh, leaf: usize) {
        let n = mesh.faces.len();
        assert_eq!(b.triangle_count, n);
        let mut seen = vec![false; n];
        for &i in &b.order {
            assert!(!seen[i as usize], "index {i} twice in order");
            seen[i as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "an index is missing from order");
        let mut leaves = 0;
        for (i, node) in b.nodes.iter().enumerate() {
            if node.count > 0 {
                leaves += 1;
                let count = node.count as usize;
                assert!(count <= leaf, "leaf {i} holds {count} > {leaf}");
                if n > leaf {
                    // The median split of a range above `leaf` gives each
                    // child at least `floor((leaf + 1) / 2)` triangles.
                    assert!(
                        count >= leaf.div_ceil(2),
                        "leaf {i} holds {count} < {} (n = {n})",
                        leaf.div_ceil(2)
                    );
                }
                for &t in b.leaf(node) {
                    let bb = &mesh.faces[t as usize].bbox;
                    let tlo = [bb.min.x, bb.min.y, bb.min.z];
                    let thi = [bb.max.x, bb.max.y, bb.max.z];
                    assert!(
                        box_holds(&node.lo, &node.hi, &tlo, &thi),
                        "leaf {i} box does not hold triangle {t}"
                    );
                }
            } else {
                let right = node.start as usize;
                assert!(
                    right > i,
                    "interior node {i}: right child {right} is not after it"
                );
                assert!(
                    right > i + 1,
                    "interior node {i}: no room for the left child"
                );
                for child in [i + 1, right] {
                    let c = &b.nodes[child];
                    assert!(
                        box_holds(&node.lo, &node.hi, &c.lo, &c.hi),
                        "node {i} box does not hold child {child}"
                    );
                }
            }
        }
        assert_eq!(b.nodes.len(), 2 * leaves - 1, "nodes against leaves");
    }

    #[test]
    fn an_empty_mesh_gives_no_node_and_no_hit() {
        let mesh = TriangleMesh::from_raw(Vec::new(), Vec::new());
        let b = bvh(&mesh, DEFAULT_LEAF_SIZE);
        assert_eq!(b.node_count(), 0);
        let (o, d) = (P3::new(0.1, 0.2, 5.0), V3::new(0.01, 0.02, -1.0));
        assert_eq!(b.cast_ray(&mesh, &o, &d), None);
        assert_eq!(ray_pick_triangle(&mesh, &o, &d), None);
        assert!(b.cast_ray_all(&mesh, &o, &d).is_empty());
    }

    #[test]
    fn a_leaf_size_of_zero_builds_as_one() {
        let mesh = terrain(6, 5);
        let zero = bvh(&mesh, 0);
        assert_eq!(zero, bvh(&mesh, 1));
        check_invariants(&zero, &mesh, 1);
    }

    #[test]
    fn the_node_invariants_hold() {
        for leaf in [1, 2, 3, 4, 7, 8, 16] {
            for n in [1, 2, leaf, leaf + 1, 2 * leaf + 1, 3 * leaf + 2, 97, 400] {
                let mesh = first_faces(n);
                check_invariants(&bvh(&mesh, leaf), &mesh, leaf);
            }
        }
        let mesh = terrain(30, 20);
        check_invariants(&bvh(&mesh, DEFAULT_LEAF_SIZE), &mesh, DEFAULT_LEAF_SIZE);
    }

    #[test]
    fn equal_centroids_still_split() {
        // Forty copies of one triangle: every centroid is equal, and the
        // index breaks every tie.
        let vertices = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 0.0),
            P3::new(0.0, 1.0, 0.0),
        ];
        let mesh = TriangleMesh::from_raw(vertices, vec![[0, 1, 2]; 40]);
        let b = bvh(&mesh, 4);
        check_invariants(&b, &mesh, 4);
        let (o, d) = (P3::new(0.25, 0.25, 3.0), V3::new(0.0, 0.0, -1.0));
        assert_eq!(b.cast_ray(&mesh, &o, &d), Some((0, 3.0)));
        assert_eq!(b.cast_ray(&mesh, &o, &d), ray_pick_triangle(&mesh, &o, &d));
        assert_eq!(b.cast_ray_all(&mesh, &o, &d).len(), 40);
    }

    #[test]
    fn two_builds_are_equal() {
        let mesh = terrain(25, 17);
        assert_eq!(bvh(&mesh, 8), bvh(&mesh, 8));
        assert_eq!(bvh(&mesh, 3), bvh(&mesh, 3));
    }

    #[test]
    fn heap_bytes_counts_the_real_types() {
        let mesh = terrain(25, 17);
        let b = bvh(&mesh, DEFAULT_LEAF_SIZE);
        assert!(b.node_count() > 1);
        assert_eq!(
            b.heap_bytes(),
            b.nodes.capacity() * size_of::<Node>() + b.order.capacity() * size_of::<u32>()
        );
        assert_eq!(b.order.capacity(), mesh.faces.len());
        assert_eq!(b.nodes.capacity(), b.node_count());
    }

    #[test]
    fn a_zero_area_triangle_gives_no_hit() {
        let vertices = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 1.0, 0.0),
            P3::new(2.0, 2.0, 0.0),
            P3::new(0.0, 2.0, -1.0),
            P3::new(2.0, 0.0, -1.0),
        ];
        let degenerate = TriangleMesh::from_raw(vertices.clone(), vec![[0, 1, 2]]);
        let (o, d) = (P3::new(1.0, 1.0, 4.0), V3::new(0.0, 0.0, -1.0));
        let b = bvh(&degenerate, DEFAULT_LEAF_SIZE);
        assert_eq!(b.cast_ray(&degenerate, &o, &d), None);
        assert_eq!(ray_pick_triangle(&degenerate, &o, &d), None);

        let mixed = TriangleMesh::from_raw(vertices, vec![[0, 1, 2], [0, 4, 3], [2, 3, 4]]);
        let b = bvh(&mixed, 1);
        let brute = ray_pick_triangle(&mixed, &o, &d);
        assert!(brute.is_some(), "the real triangles must take the ray");
        assert_eq!(b.cast_ray(&mixed, &o, &d), brute);
    }

    #[test]
    fn a_cast_on_another_mesh_falls_back() {
        let built = terrain(10, 10);
        let other = terrain(7, 9);
        let b = bvh(&built, DEFAULT_LEAF_SIZE);
        let d = V3::new(0.0, 0.0, -1.0);
        let mut hits = 0;
        for k in 0..20 {
            let o = P3::new(0.3 + 0.31 * k as f64, 0.2 + 0.43 * k as f64, 50.0);
            let brute = ray_pick_triangle(&other, &o, &d);
            hits += usize::from(brute.is_some());
            assert_eq!(b.cast_ray(&other, &o, &d), brute);
        }
        assert!(hits > 0, "no fallback ray hit: the check is vacuous");
    }
}
