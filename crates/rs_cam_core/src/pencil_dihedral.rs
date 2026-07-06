//! Mesh-dihedral crease detection for pencil finishing (detector #1, the
//! historical default — see [`crate::pencil::PencilDetector::Dihedral`]).
//!
//! Robust on clean CAD-style meshes with sharp internal corners: it detects
//! edges where two faces meet at a concave angle below the bitangency
//! threshold, then chains connected concave edges into polylines via graph
//! traversal. On dense noisy relief it fires on every triangulation crease
//! and fragments — see [`crate::crest_lines`] (curvature crest lines) or
//! [`crate::rest_field`] (the tool-radius-aware rest-depth field) for the
//! organic-relief alternatives.
//!
//! Pipeline (owned entirely by this module):
//! 1. [`build_edge_adjacency`] — edge-to-face adjacency map.
//! 2. [`compute_shared_edges`] — dihedral angle + concavity per shared edge.
//! 3. [`chain_concave_edges`] — graph-walk concave edges into vertex chains.
//! 4. [`sample_chain_bisected`] — sample each chain at cut spacing, shifted
//!    by the per-edge bisector offset ([`bisector_offset_xy`]) so the trace
//!    nestles into asymmetric corners.
//!
//! The rest-depth gate, path ordering, and toolpath emission are detector-
//! agnostic and stay in [`crate::pencil`] as the shared pipeline.

use std::collections::HashMap;

use crate::geo::{P3, V3};
use crate::mesh::TriangleMesh;

/// A single mesh edge identified by sorted vertex indices.
/// `Ord` so order-sensitive consumers can sort collections that were
/// built via `HashMap` iteration — see [`compute_shared_edges`] /
/// [`chain_concave_edges`] (T14 determinism fix).
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct EdgeKey(u32, u32);

impl EdgeKey {
    pub(crate) fn new(a: u32, b: u32) -> Self {
        if a <= b { Self(a, b) } else { Self(b, a) }
    }
}

/// Information about a shared mesh edge.
pub(crate) struct SharedEdge {
    /// Sorted vertex indices
    pub(crate) key: EdgeKey,
    /// Indices of the two faces sharing this edge (used for bisector positioning)
    pub(crate) face_a: usize,
    pub(crate) face_b: usize,
    /// Dihedral angle in radians (0 = coplanar, π = fully folded)
    pub(crate) dihedral_angle: f64,
    /// True if the edge is concave (crease), false if convex (ridge)
    pub(crate) is_concave: bool,
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Build the edge-to-face adjacency map.
/// Returns a map from sorted vertex pair to list of face indices.
pub(crate) fn build_edge_adjacency(mesh: &TriangleMesh) -> HashMap<EdgeKey, Vec<usize>> {
    let mut edge_map: HashMap<EdgeKey, Vec<usize>> = HashMap::new();

    for (face_idx, tri_indices) in mesh.triangles.iter().enumerate() {
        for i in 0..3 {
            let a = tri_indices[i];
            let b = tri_indices[(i + 1) % 3];
            let key = EdgeKey::new(a, b);
            edge_map.entry(key).or_default().push(face_idx);
        }
    }

    edge_map
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Compute shared edge info for all edges with exactly 2 adjacent faces.
pub(crate) fn compute_shared_edges(
    mesh: &TriangleMesh,
    edge_map: &HashMap<EdgeKey, Vec<usize>>,
) -> Vec<SharedEdge> {
    let mut shared = Vec::new();

    for (&key, faces) in edge_map {
        if faces.len() != 2 {
            continue; // boundary or non-manifold edge
        }

        let fa = faces[0];
        let fb = faces[1];
        let n1 = mesh.faces[fa].normal;
        let n2 = mesh.faces[fb].normal;

        // Dihedral angle: the angle between the two face normals
        // cos(angle) = n1 · n2, clamped for numerical safety
        let cos_angle = n1.dot(&n2).clamp(-1.0, 1.0);
        let dihedral = cos_angle.acos(); // 0 = coplanar, π = fully folded

        // Determine concavity geometrically: the edge is concave (a valley) if
        // face B's far vertex (the one not on the shared edge) lies ABOVE face
        // A's outward plane — the two facets fold up toward each other across the
        // seam. Below the plane → convex (a ridge). This is orientation-correct
        // for consistently-wound (outward-normal) meshes.
        //
        // NB: the previous `cross(n1, n2) · edge_vec > 0` sign test was NOT
        // orientation-stable — `n1`/`n2` come from arbitrary face-insertion
        // order and `edge_vec` from arbitrary vertex-index order, with no
        // geometric link between them. On a dense mesh it disagreed with this
        // test on ~46% of sharp edges (near-random), so pencil traced a mix of
        // ridges, valleys and noise instead of just the concave creases.
        let tri_b = mesh.triangles[fb];
        let apex_b = tri_b.iter().copied().find(|&v| v != key.0 && v != key.1);
        let is_concave = match apex_b {
            Some(ai) => {
                let apex = mesh.vertices[ai as usize];
                let edge_pt = mesh.vertices[key.0 as usize];
                (apex - edge_pt).dot(&n1) > 0.0
            }
            None => false, // degenerate triangle (repeated vertex)
        };

        shared.push(SharedEdge {
            key,
            face_a: fa,
            face_b: fb,
            dihedral_angle: dihedral,
            is_concave,
        });
    }

    // T14 — `edge_map` is a `HashMap`, so the loop above visits edges in
    // RandomState order (different every run). Everything downstream is
    // order-sensitive: the chain walker's adjacency lists are built by
    // pushing in this Vec's order, and at degenerate bitangency angles
    // (~175°) junction-heavy graphs chain differently per visit order —
    // observed as 7404-vs-7350-move toolpaths from the same build. Sort
    // by edge key so the pipeline is a pure function of the mesh.
    shared.sort_unstable_by_key(|e| e.key);

    shared
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Chain connected concave edges into polylines.
/// Returns a list of vertex-index chains, where each chain is an ordered
/// sequence of vertex indices forming a polyline along concave edges.
pub(crate) fn chain_concave_edges(
    concave_edges: &[SharedEdge],
    mesh: &TriangleMesh,
    min_length: f64,
) -> Vec<Vec<u32>> {
    if concave_edges.is_empty() {
        return Vec::new();
    }

    // Build adjacency graph: vertex -> list of connected vertices via concave edges
    let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
    for edge in concave_edges {
        adj.entry(edge.key.0).or_default().push(edge.key.1);
        adj.entry(edge.key.1).or_default().push(edge.key.0);
    }

    // Walk the graph to extract chains. At each junction (degree > 2),
    // we start/end chains. Simple paths (degree <= 2) are walked end-to-end.
    let mut visited_edges: HashMap<EdgeKey, bool> = HashMap::new();
    for edge in concave_edges {
        visited_edges.insert(edge.key, false);
    }

    let mut chains: Vec<Vec<u32>> = Vec::new();

    // Find chain starting points: vertices with degree != 2 (endpoints/junctions)
    // or any unvisited vertex if all have degree 2 (closed loops)
    let mut start_vertices: Vec<u32> = adj
        .keys()
        .filter(|&&v| {
            let deg = adj.get(&v).map_or(0, |n| n.len());
            deg != 2
        })
        .copied()
        .collect();
    // T14 — `adj.keys()` iterates in HashMap RandomState order; walk
    // priority decides how junction-heavy graphs split into chains.
    // Sort so the start order is a pure function of the mesh.
    start_vertices.sort_unstable();

    // If no endpoints found (all closed loops), pick the smallest vertex
    // (deterministic — `.next()` on a HashMap varies per run, T14)
    if start_vertices.is_empty()
        && let Some(&v) = adj.keys().min()
    {
        start_vertices.push(v);
    }

    for &start in &start_vertices {
        // Try walking from this vertex in each unvisited direction
        if let Some(neighbors) = adj.get(&start) {
            for &next in neighbors {
                let edge_key = EdgeKey::new(start, next);
                if let Some(visited) = visited_edges.get(&edge_key)
                    && *visited
                {
                    continue;
                }

                // Walk the chain
                let mut chain = vec![start, next];
                if let Some(v) = visited_edges.get_mut(&edge_key) {
                    *v = true;
                }

                loop {
                    let current = *chain.last().unwrap_or(&start);
                    let prev = chain[chain.len() - 2];

                    // Find next unvisited neighbor (not the one we came from)
                    let next_opt = adj.get(&current).and_then(|neighbors| {
                        neighbors
                            .iter()
                            .find(|&&n| {
                                if n == prev {
                                    return false;
                                }
                                let ek = EdgeKey::new(current, n);
                                visited_edges.get(&ek).is_some_and(|&v| !v)
                            })
                            .copied()
                    });

                    match next_opt {
                        Some(next_v) => {
                            let ek = EdgeKey::new(current, next_v);
                            if let Some(v) = visited_edges.get_mut(&ek) {
                                *v = true;
                            }
                            chain.push(next_v);
                        }
                        None => break,
                    }
                }

                chains.push(chain);
            }
        }
    }

    // Also find any remaining closed loops (all edges visited by endpoints check above
    // may miss pure loops)
    let mut unvisited_starts: Vec<EdgeKey> = visited_edges
        .iter()
        .filter(|&(_, v)| !v)
        .map(|(&k, _)| k)
        .collect();
    // T14 — same HashMap-order leak as above: which edge seeds a loop
    // walk decides where closed loops are split open.
    unvisited_starts.sort_unstable();

    for edge_key in unvisited_starts {
        if visited_edges.get(&edge_key) == Some(&true) {
            continue; // May have been visited during a previous loop walk
        }
        // Start a loop walk from this edge
        let mut chain = vec![edge_key.0, edge_key.1];
        if let Some(v) = visited_edges.get_mut(&edge_key) {
            *v = true;
        }

        loop {
            let current = chain[chain.len() - 1];
            let prev = chain[chain.len() - 2];
            let next_opt = adj.get(&current).and_then(|neighbors| {
                neighbors
                    .iter()
                    .find(|&&n| {
                        if n == prev {
                            return false;
                        }
                        let ek = EdgeKey::new(current, n);
                        visited_edges.get(&ek).is_some_and(|&v| !v)
                    })
                    .copied()
            });
            match next_opt {
                Some(next_v) => {
                    let ek = EdgeKey::new(current, next_v);
                    if let Some(v) = visited_edges.get_mut(&ek) {
                        *v = true;
                    }
                    chain.push(next_v);
                }
                None => break,
            }
        }
        chains.push(chain);
    }

    // Filter by minimum length
    chains
        .into_iter()
        .filter(|chain| {
            if chain.len() < 2 {
                return false;
            }
            let mut total_len = 0.0;
            for i in 0..chain.len() - 1 {
                let a = mesh.vertices[chain[i] as usize];
                let b = mesh.vertices[chain[i + 1] as usize];
                total_len += (b - a).norm();
            }
            total_len >= min_length
        })
        .collect()
}

/// Horizontal (XY) bisector offset that nestles a radius-`r` ball into the concave
/// corner between two faces with outward unit normals `n1`,`n2`. For a *symmetric*
/// valley the seam line already is the bisector and `(n1+n2)` points straight up,
/// so this is ~zero; for an *asymmetric* corner (steep wall + flat floor) it shifts
/// the trace out over the shallower face. Derivation: the ball-centre in two-point
/// tangency is `C = S + r·(n1+n2)/(1 + n1·n2)`; we keep its X,Y and let drop-cutter
/// re-solve Z gouge-safely. Magnitude is capped at `r` (a sharper asymmetric corner
/// a ball can't enter anyway) and scaled by `strength` for tuning. Near-opposite
/// normals (a near-flat fold, denominator → 0) yield no shift.
fn bisector_offset_xy(n1: &V3, n2: &V3, r: f64, strength: f64) -> (f64, f64) {
    let denom = 1.0 + n1.dot(n2);
    if denom <= 1e-3 || strength <= 0.0 || r <= 0.0 {
        return (0.0, 0.0);
    }
    let sum = n1 + n2;
    let mut ox = r * sum.x / denom * strength;
    let mut oy = r * sum.y / denom * strength;
    let mag = (ox * ox + oy * oy).sqrt();
    if mag > r {
        let s = r / mag;
        ox *= s;
        oy *= s;
    }
    (ox, oy)
}

/// Sample points along a vertex chain at the given spacing, applying the per-edge
/// bisector offset (see [`bisector_offset_xy`]) to each point's X,Y so the trace
/// nestles into asymmetric corners. Shifts each segment's points by that
/// segment's offset; Z is left as the interpolated seam height (the later
/// `lift_to_surface` drop re-solves it). Offset jumps at vertices are smoothed
/// by the subsequent fairing pass. With `strength == 0` (or no edge normals)
/// this reduces to plain unweighted chain sampling (see `pencil`'s private
/// `resample_polyline` for the equivalent walk over already-materialized
/// points).
#[allow(clippy::indexing_slicing)] // chain entries are valid vertex indices
pub(crate) fn sample_chain_bisected(
    mesh: &TriangleMesh,
    chain: &[u32],
    spacing: f64,
    edge_norms: &HashMap<EdgeKey, (V3, V3)>,
    radius: f64,
    strength: f64,
) -> Vec<P3> {
    if chain.len() < 2 {
        return Vec::new();
    }
    let off_for = |i: usize, j: usize| -> (f64, f64) {
        edge_norms
            .get(&EdgeKey::new(chain[i], chain[j]))
            .map(|(n1, n2)| bisector_offset_xy(n1, n2, radius, strength))
            .unwrap_or((0.0, 0.0))
    };

    let mut points = Vec::new();
    let (ox0, oy0) = off_for(0, 1);
    let p0 = mesh.vertices[chain[0] as usize];
    points.push(P3::new(p0.x + ox0, p0.y + oy0, p0.z));

    let mut accumulated = 0.0;
    for i in 0..chain.len() - 1 {
        let a = mesh.vertices[chain[i] as usize];
        let b = mesh.vertices[chain[i + 1] as usize];
        let seg_len = (b - a).norm();
        if seg_len < 1e-10 {
            continue;
        }
        let (ox, oy) = off_for(i, i + 1);
        let dir = (b - a) / seg_len;
        let mut dist_along = spacing - accumulated;
        while dist_along <= seg_len {
            let pt = a + dir * dist_along;
            points.push(P3::new(pt.x + ox, pt.y + oy, pt.z));
            dist_along += spacing;
        }
        accumulated = seg_len - (dist_along - spacing);
    }

    if let Some(&last_idx) = chain.last() {
        let last = mesh.vertices[last_idx as usize];
        let (oxl, oyl) = off_for(chain.len() - 2, chain.len() - 1);
        let lp = P3::new(last.x + oxl, last.y + oyl, last.z);
        if let Some(prev) = points.last()
            && (lp - prev).norm() > spacing * 0.1
        {
            points.push(lp);
        }
    }
    points
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    /// Create a V-groove mesh: two planes meeting at a concave edge along X axis.
    fn make_v_groove(length: f64, depth: f64, width: f64) -> TriangleMesh {
        // V-groove: two inclined planes meeting at y=0
        // Left plane: from (0, -width, 0) down to (0, 0, -depth) and back up
        // Right plane: from (0, 0, -depth) up to (0, width, 0)
        let vertices = vec![
            P3::new(0.0, -width, 0.0),    // 0: left-back top
            P3::new(length, -width, 0.0), // 1: left-front top
            P3::new(0.0, 0.0, -depth),    // 2: back center (groove bottom)
            P3::new(length, 0.0, -depth), // 3: front center (groove bottom)
            P3::new(0.0, width, 0.0),     // 4: right-back top
            P3::new(length, width, 0.0),  // 5: right-front top
        ];

        // CCW winding so the wall facets face UP/outward (+Z normals) — a valid
        // machinable surface. (The original winding produced downward normals,
        // which drop_cutter rightly skips and the geometric concavity test reads
        // inverted.)
        let triangles = vec![
            [0, 1, 2], // left plane tri 1
            [1, 3, 2], // left plane tri 2
            [2, 3, 4], // right plane tri 1
            [3, 5, 4], // right plane tri 2
        ];

        TriangleMesh::from_raw(vertices, triangles)
    }

    /// Create a flat-only mesh (convex-only, no concave edges).
    fn make_convex_box(size: f64) -> TriangleMesh {
        // Simple flat square — no concave edges possible with 2 triangles
        let vertices = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(size, 0.0, 0.0),
            P3::new(size, size, 0.0),
            P3::new(0.0, size, 0.0),
        ];
        let triangles = vec![[0, 1, 2], [0, 2, 3]];
        TriangleMesh::from_raw(vertices, triangles)
    }

    #[test]
    fn test_edge_adjacency_basic() {
        let mesh = make_v_groove(20.0, 5.0, 10.0);
        let edge_map = build_edge_adjacency(&mesh);

        // 4 triangles × 3 edges = 12 half-edges
        // Some edges are shared (interior), some are boundary
        assert!(!edge_map.is_empty());

        // The center edge (vertices 2-3) should be shared by 2 faces
        let center_key = EdgeKey::new(2, 3);
        assert_eq!(
            edge_map.get(&center_key).map(|v| v.len()),
            Some(2),
            "Center groove edge should be shared by 2 faces"
        );
    }

    #[test]
    fn test_v_groove_detects_concave_edge() {
        let mesh = make_v_groove(20.0, 5.0, 10.0);
        let edge_map = build_edge_adjacency(&mesh);
        let shared = compute_shared_edges(&mesh, &edge_map);

        // Should find at least one concave edge (the groove bottom)
        let concave_count = shared.iter().filter(|e| e.is_concave).count();
        assert!(
            concave_count >= 1,
            "V-groove should have at least 1 concave edge, found {}",
            concave_count
        );

        // The center edge (2-3) should be concave
        let center_edge = shared
            .iter()
            .find(|e| (e.key.0 == 2 && e.key.1 == 3) || (e.key.0 == 3 && e.key.1 == 2));
        assert!(center_edge.is_some(), "Should find center groove edge");
        if let Some(edge) = center_edge {
            assert!(edge.is_concave, "Center groove edge should be concave");
            // V-groove with depth=5, width=10 → half-angle = atan(5/10) ≈ 26.6°
            // Dihedral should be around 180° - 2*26.6° = 126.8° → about 0.72π radians
            assert!(
                edge.dihedral_angle > 0.5,
                "Dihedral angle should be significant, got {:.2} rad ({:.1}°)",
                edge.dihedral_angle,
                edge.dihedral_angle.to_degrees()
            );
        }
    }

    #[test]
    fn test_convex_mesh_no_concave_edges() {
        let mesh = make_convex_box(50.0);
        let edge_map = build_edge_adjacency(&mesh);
        let shared = compute_shared_edges(&mesh, &edge_map);

        // Flat mesh should have no concave edges
        let _concave_count = shared.iter().filter(|e| e.is_concave).count();
        // For coplanar faces, dihedral angle ≈ 0, concavity is ambiguous (sign ≈ 0)
        // Either concave_count == 0, or any "concave" edges have angle ≈ 0
        for edge in &shared {
            if edge.is_concave {
                assert!(
                    edge.dihedral_angle < 0.1,
                    "Flat mesh concave edge should have near-zero dihedral, got {:.2}",
                    edge.dihedral_angle
                );
            }
        }
        // With threshold of 160°, none should pass
        let threshold_rad = 160.0_f64.to_radians();
        let filtered = shared
            .iter()
            .filter(|e| e.is_concave && e.dihedral_angle > (std::f64::consts::PI - threshold_rad))
            .count();
        assert_eq!(
            filtered, 0,
            "Flat mesh should produce no pencil edges at 160° threshold"
        );
    }

    #[test]
    fn test_chain_concave_edges_v_groove() {
        let mesh = make_v_groove(20.0, 5.0, 10.0);
        let edge_map = build_edge_adjacency(&mesh);
        let shared = compute_shared_edges(&mesh, &edge_map);

        let concave: Vec<SharedEdge> = shared
            .into_iter()
            .filter(|e| e.is_concave && e.dihedral_angle > 0.1)
            .collect();

        let chains = chain_concave_edges(&concave, &mesh, 1.0);
        assert!(
            !chains.is_empty(),
            "V-groove should produce at least one chain"
        );

        // Chain should follow the groove bottom (vertices 2 and 3)
        for chain in &chains {
            assert!(chain.len() >= 2, "Chain should have at least 2 vertices");
        }
    }

    // `test_sample_chain_spacing` (the un-offset chain-walk coverage) moved to
    // `pencil::tests::test_resample_polyline_spacing` alongside the retired
    // `sample_chain`'s replacement, `resample_polyline`.

    /// Regression for the concavity-sign bug: a tent (ridge peaking along y=0,
    /// falling to both sides) is sharply folded but CONVEX — none of its seam
    /// edges may be reported concave. The old `cross·edge_vec` test got this
    /// wrong ~46% of the time on dense meshes, so pencil traced ridges too.
    #[test]
    fn test_ridge_seam_is_convex_not_concave() {
        let (nx, ny) = (20usize, 32usize);
        let (len_x, half_y, slope) = (20.0, 4.0, 2.0);
        let sx = len_x / nx as f64;
        let sy = 2.0 * half_y / ny as f64;
        let mut verts = Vec::new();
        for j in 0..=ny {
            for i in 0..=nx {
                let x = i as f64 * sx;
                let y = -half_y + j as f64 * sy;
                let z = slope * half_y - slope * y.abs(); // peak at y=0 → ridge
                verts.push(P3::new(x, y, z));
            }
        }
        let idx = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
        let mut tris = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                tris.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
                tris.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
            }
        }
        let mesh = TriangleMesh::from_raw(verts, tris);
        let edge_map = build_edge_adjacency(&mesh);
        let shared = compute_shared_edges(&mesh, &edge_map);
        let sharp_concave = shared
            .iter()
            .filter(|e| e.dihedral_angle.to_degrees() > 20.0 && e.is_concave)
            .count();
        assert_eq!(
            sharp_concave, 0,
            "ridge seam must be convex; got {sharp_concave} sharp concave edges"
        );
    }

    /// Bisector offset: zero for a symmetric valley, shifts +r over the floor for
    /// a 90° L-corner, and scales with strength.
    #[test]
    fn test_bisector_offset_symmetric_zero_asymmetric_shifts() {
        let r = 2.0;

        // Symmetric V-valley: normals mirror about Z → (n1+n2) is vertical → no shift.
        let a = 0.6_f64;
        let b = (1.0 - a * a).sqrt();
        let (sx, sy) = bisector_offset_xy(&V3::new(-a, 0.0, b), &V3::new(a, 0.0, b), r, 1.0);
        assert!(
            sx.abs() < 1e-9 && sy.abs() < 1e-9,
            "symmetric valley must not shift: ({sx},{sy})"
        );

        // Asymmetric 90° L-corner: vertical wall (+x) + flat floor (+z) → shift +r in x.
        let n_wall = V3::new(1.0, 0.0, 0.0);
        let n_floor = V3::new(0.0, 0.0, 1.0);
        let (lx, ly) = bisector_offset_xy(&n_wall, &n_floor, r, 1.0);
        assert!(
            (lx - r).abs() < 1e-9 && ly.abs() < 1e-9,
            "L-corner must shift +r out over the floor: ({lx},{ly})"
        );

        // Strength scales it; 0 disables.
        let (hx, _) = bisector_offset_xy(&n_wall, &n_floor, r, 0.5);
        assert!(
            (hx - r * 0.5).abs() < 1e-9,
            "strength should scale the offset"
        );
        let (zx, zy) = bisector_offset_xy(&n_wall, &n_floor, r, 0.0);
        assert!(zx == 0.0 && zy == 0.0, "strength 0 disables the shift");
    }
}
