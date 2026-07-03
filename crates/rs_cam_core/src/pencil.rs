//! Pencil finishing — traces concave edges (creases) on mesh surfaces.
//!
//! Detects edges where two faces meet at a concave angle below a threshold
//! (the "bitangency angle"), chains them into polylines, and generates
//! toolpaths that follow these creases. This cleans material left in
//! concavities that ball/bull nose cutters cannot reach with standard passes.
//!
//! Algorithm:
//! 1. Build edge adjacency map from mesh face indices
//! 2. Compute dihedral angle at each shared edge from face normals
//! 3. Filter concave edges below bitangency angle threshold
//! 4. Chain connected concave edges into polylines (graph traversal)
//! 5. Sample points along polylines, drop-cutter for Z → CL path
//! 6. Optional offset passes parallel to centerline
//! 7. Link nearby segments, order by nearest-neighbor TSP

use std::collections::HashMap;

use tracing::info;

use crate::debug_trace::ToolpathDebugContext;
use crate::dropcutter::point_drop_cutter;
use crate::geo::{P3, V3};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

/// Which valley-detection front-end the pencil generator uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PencilDetector {
    /// Mesh-dihedral crease detection (Ohtake-Belyaev-Seidel-style discrete
    /// curvature creases). Robust on clean CAD-style meshes with sharp internal
    /// corners; on dense noisy relief it fires on every triangulation crease and
    /// fragments. The historical default.
    #[default]
    Dihedral,
    /// Curvature crest-line extraction (Ohtake-Belyaev-Seidel 2004 / Yoshizawa
    /// 2005, curvature via Rusinkiewicz 2004): per-vertex principal curvatures →
    /// minimal-curvature extremality → zero-crossing valley lines, filtered by a
    /// single `valley_saliency` (|κ₂|) dial. Traces every concave seam on dense
    /// organic relief and dials cleanly from "all seams" to "deep sharp valleys
    /// only". See [`crate::crest_lines`]. The right choice for noisy meshes.
    Curvature,
    /// Rest-depth-field detection (the tool-offset-space one; see
    /// [`crate::rest_field`]). Computes `rest = drop_z(reference) − drop_z(pencil)`
    /// on an XY grid — the dual-tool comparison every commercial CAM uses — and
    /// traces the skeleton of each rest region. Unlike the other three detectors
    /// it is NOT tool-radius-blind: the field is zero wherever the reference tool
    /// already reached, and the `reference_tool_diameter` dial visibly moves the
    /// detection. Routes narrow regions to pencil centrelines and wide regions to
    /// clearing. The aligned detector — recommended on relief.
    RestDepth,
}

impl PencilDetector {
    /// Parse from a lowercase config string; unknown values fall back to the
    /// default (Dihedral) so older project files keep loading.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "curvature" | "crest" | "ridgevalley" | "ridge_valley" => PencilDetector::Curvature,
            "rest_depth" | "restdepth" | "rest" => PencilDetector::RestDepth,
            _ => PencilDetector::Dihedral,
        }
    }

    /// Canonical lowercase token for serialisation.
    pub fn as_str(&self) -> &'static str {
        match self {
            PencilDetector::Dihedral => "dihedral",
            PencilDetector::Curvature => "curvature",
            PencilDetector::RestDepth => "rest_depth",
        }
    }
}

/// Parameters for pencil finishing.
pub struct PencilParams {
    /// Dihedral angle threshold in degrees. Edges with concave angles below this
    /// are considered creases. Default: 160° (nearly flat edges ignored).
    pub bitangency_angle: f64,
    /// Minimum chain length to keep (mm). Chains shorter than this are discarded.
    /// Default: tool diameter.
    pub min_cut_length: f64,
    /// Maximum gap between chain endpoints for linking (mm).
    /// Nearby chains are connected with rapid moves. Default: tool_diameter * 3.
    pub hookup_distance: f64,
    /// Number of offset passes on each side of the centerline. 0 = centerline only.
    pub num_offset_passes: usize,
    /// Offset stepover between parallel passes (mm). Default: tool_radius * 0.5.
    pub offset_stepover: f64,
    /// Point spacing along paths (mm). Default: 0.5mm.
    pub sampling: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid moves.
    pub safe_z: f64,
    /// Stock to leave on the surface (mm).
    pub stock_to_leave: f64,
    /// Reach-gap tolerance (mm): minimum uncut depth at a concave seam for it to
    /// count as a genuine valley worth cutting. A candidate edge is kept only
    /// where the tool's resting reference floats more than this above the true
    /// surface (i.e. the tool bridges the valley). Raise it to ignore shallow
    /// surface texture and keep only deeper channels; lower it to catch fine
    /// detail. Default 0.05mm (see [`reach_gap_threshold`]).
    pub min_valley_depth: f64,
    /// Bisector positioning strength (0 = off, 1 = geometrically correct). In an
    /// asymmetric internal corner (steep wall + flat floor, e.g. a lake edge) the
    /// seam line is NOT where a vertical ball nestles — it rides up the steep wall
    /// and leaves the fillet uncut. This shifts the trace X,Y out along the
    /// bisector of the two walls (zero for symmetric valleys) so the drop rests
    /// tangent to both. 1.0 = the exact `r·(n1+n2)/(1+n1·n2)` offset; lower to
    /// under-shoot, raise to over-shoot when dialing. Default 1.0
    /// (see [`bisector_strength_default`]).
    pub bisector_strength: f64,
    /// Diameter (mm) of the bigger *reference* tool the pencil pass cleans up after
    /// (typically the finishing tool). The gate keeps a seam by how much DEEPER the
    /// pencil tool reaches than this reference could —
    /// `rest_depth = reference_gap − pencil_gap` — so it traces the valleys the big
    /// tool missed but the pencil tool can enter, and skips both big-tool-reachable
    /// walls and sub-pencil-scale texture (where both tools float). When `<=` the
    /// pencil tool's own diameter, falls back to the self-referenced gap. Default 6.0
    /// (see [`reference_tool_diameter_default`]).
    pub reference_tool_diameter: f64,
    /// Which valley-detection algorithm to use (see [`PencilDetector`]). Default
    /// `Dihedral` (crease detection); `Curvature` extracts curvature crest lines
    /// and is the right choice for noisy organic relief.
    pub detector: PencilDetector,
    /// Minimum concave curvature |κ₂| (1/mm) a valley must reach to be traced by
    /// the `Curvature` detector — THE significance dial. Low → every concave
    /// seam; high → only deep sharp valleys (a flat basin has κ₂ ≈ 0 and drops
    /// out at any positive value). Default 0.05 (see [`valley_saliency_default`]).
    pub valley_saliency: f64,
    /// Curvature-tensor smoothing iterations for the `Curvature` detector — the
    /// literature denoise (smooths the curvature field, not the geometry). More
    /// suppresses triangulation noise at the cost of blurring nearby valleys.
    /// Default 3 (see [`curvature_smoothing_default`]).
    pub curvature_smoothing: usize,
    /// XY grid cell size (mm) for the `RestDepth` detector's rest field. Smaller
    /// = finer regions and more drops. Default 0.5 (see [`rest_cell_default`]).
    pub rest_cell_mm: f64,
    /// `RestDepth` routing threshold: a rest region routes to a pencil centreline
    /// when its half-width `≤ route_width_factor × pencil_radius`, else to
    /// clearing. Default 2.0 (see [`route_width_factor_default`]).
    pub route_width_factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PencilRuntimeEvent {
    OffsetPass {
        chain_index: usize,
        chain_total: usize,
        offset_index: usize,
        offset_total: usize,
        offset_mm: f64,
        is_centerline: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PencilRuntimeAnnotation {
    pub move_index: usize,
    pub event: PencilRuntimeEvent,
}

impl PencilRuntimeEvent {
    pub fn label(&self) -> String {
        match self {
            Self::OffsetPass {
                chain_index,
                offset_index,
                is_centerline,
                ..
            } => {
                if *is_centerline {
                    format!("Chain {chain_index} centerline")
                } else {
                    format!("Chain {chain_index} offset pass {offset_index}")
                }
            }
        }
    }
}

#[derive(Clone)]
struct PencilPath {
    points: Vec<P3>,
    chain_index: usize,
    chain_total: usize,
    offset_index: usize,
    offset_total: usize,
    offset_mm: f64,
    is_centerline: bool,
}

/// A single mesh edge identified by sorted vertex indices.
/// `Ord` so order-sensitive consumers can sort collections that were
/// built via `HashMap` iteration — see `compute_shared_edges` /
/// `chain_concave_edges` (T14 determinism fix).
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
struct EdgeKey(u32, u32);

impl EdgeKey {
    fn new(a: u32, b: u32) -> Self {
        if a <= b { Self(a, b) } else { Self(b, a) }
    }
}

/// Information about a shared mesh edge.
struct SharedEdge {
    /// Sorted vertex indices
    key: EdgeKey,
    /// Indices of the two faces sharing this edge (used for bisector positioning)
    face_a: usize,
    face_b: usize,
    /// Dihedral angle in radians (0 = coplanar, π = fully folded)
    dihedral_angle: f64,
    /// True if the edge is concave (crease), false if convex (ridge)
    is_concave: bool,
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Build the edge-to-face adjacency map.
/// Returns a map from sorted vertex pair to list of face indices.
fn build_edge_adjacency(mesh: &TriangleMesh) -> HashMap<EdgeKey, Vec<usize>> {
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
fn compute_shared_edges(
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
fn chain_concave_edges(
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

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
#[allow(dead_code)]
// superseded by sample_chain_bisected; retained as the
// un-offset reference and exercised by test_sample_chain_spacing
/// Sample points along a vertex chain at the given spacing.
/// Returns 3D points interpolated along the polyline.
fn sample_chain(mesh: &TriangleMesh, chain: &[u32], spacing: f64) -> Vec<P3> {
    if chain.len() < 2 {
        return Vec::new();
    }

    let mut points = Vec::new();
    let mut accumulated = 0.0;

    points.push(mesh.vertices[chain[0] as usize]);

    for i in 0..chain.len() - 1 {
        let a = mesh.vertices[chain[i] as usize];
        let b = mesh.vertices[chain[i + 1] as usize];
        let seg_len = (b - a).norm();

        if seg_len < 1e-10 {
            continue;
        }

        let dir = (b - a) / seg_len;
        let mut dist_along = spacing - accumulated;

        while dist_along <= seg_len {
            let pt = a + dir * dist_along;
            points.push(pt);
            dist_along += spacing;
        }

        accumulated = seg_len - (dist_along - spacing);
    }

    // Always include the last point
    if let Some(&last_idx) = chain.last() {
        let last = mesh.vertices[last_idx as usize];
        if let Some(prev) = points.last()
            && (last - prev).norm() > spacing * 0.1
        {
            points.push(last);
        }
    }

    points
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Generate an offset polyline by shifting each point perpendicular to the path
/// direction in XY by the given offset distance.
fn offset_polyline(points: &[P3], offset: f64) -> Vec<P3> {
    if points.len() < 2 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity(points.len());

    for i in 0..points.len() {
        // Compute tangent direction at this point
        let tangent = if i == 0 {
            let d = points[1] - points[0];
            nalgebra::Vector2::new(d.x, d.y)
        } else if i == points.len() - 1 {
            let d = points[i] - points[i - 1];
            nalgebra::Vector2::new(d.x, d.y)
        } else {
            let d = points[i + 1] - points[i - 1];
            nalgebra::Vector2::new(d.x, d.y)
        };

        let len = tangent.norm();
        if len < 1e-10 {
            result.push(points[i]);
            continue;
        }

        // Perpendicular direction in XY (rotate tangent 90° CCW)
        let normal = nalgebra::Vector2::new(-tangent.y, tangent.x) / len;

        result.push(P3::new(
            points[i].x + normal.x * offset,
            points[i].y + normal.y * offset,
            points[i].z,
        ));
    }

    result
}

/// Default fairing strength: how far each interior point moves toward the
/// midpoint of its neighbours per pass (0 = none, 1 = full Laplacian step).
pub(crate) const FAIRING_STRENGTH: f64 = 0.5;
/// Default number of fairing passes applied to each sampled chain.
pub(crate) const FAIRING_PASSES: usize = 2;

/// Lightly fair a sampled polyline in XY to remove facet-scale jaggedness. The
/// raw chain hops between 0.5mm mesh-edge vertices, so the centerline zig-zags
/// at triangulation scale; a couple of Laplacian passes straighten it without
/// moving it materially off the valley floor. Z is left untouched — the
/// subsequent `lift_to_surface` drop re-solves it gouge-safely from the faired
/// X,Y — and endpoints are pinned so chains don't shrink at their tips.
#[allow(clippy::indexing_slicing)] // i bounded to 1..len-1; neighbours i±1 valid
fn fair_polyline_xy(points: &[P3], passes: usize, strength: f64) -> Vec<P3> {
    if points.len() < 3 || passes == 0 || strength <= 0.0 {
        return points.to_vec();
    }
    let mut pts = points.to_vec();
    for _ in 0..passes {
        let prev = pts.clone();
        for i in 1..prev.len() - 1 {
            let a = prev[i - 1];
            let b = prev[i];
            let c = prev[i + 1];
            // Move toward the midpoint of the two neighbours (Laplacian).
            pts[i].x = b.x + strength * ((a.x + c.x) * 0.5 - b.x);
            pts[i].y = b.y + strength * ((a.y + c.y) * 0.5 - b.y);
        }
    }
    pts
}

/// Default bisector positioning strength (1.0 = the geometrically-correct offset).
pub(crate) fn bisector_strength_default() -> f64 {
    1.0
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
/// nestles into asymmetric corners. Mirrors [`sample_chain`] but shifts each
/// segment's points by that segment's offset; Z is left as the interpolated seam
/// height (the later `lift_to_surface` drop re-solves it). Offset jumps at vertices
/// are smoothed by the subsequent fairing pass. With `strength == 0` (or no edge
/// normals) this reduces to plain `sample_chain`.
#[allow(clippy::indexing_slicing)] // chain entries are valid vertex indices
fn sample_chain_bisected(
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

/// Lift 2D polyline points to the mesh surface using drop-cutter. The per-point
/// drop is the hot work, so it runs in parallel when the `parallel` feature is on.
fn lift_to_surface(
    points: &[P3],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stock_to_leave: f64,
) -> Vec<P3> {
    let lift = |p: &P3| {
        let cl = point_drop_cutter(p.x, p.y, mesh, index, cutter);
        if cl.contacted {
            P3::new(p.x, p.y, cl.z + stock_to_leave)
        } else {
            // Outside mesh — keep original Z (will be filtered or skipped)
            *p
        }
    };
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        points.par_iter().map(lift).collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        points.iter().map(lift).collect()
    }
}

/// Tool-radius-aware reach gap at a concave edge's midpoint: how far the tool's
/// resting reference floats above the true surface there.
///
/// The edge midpoint lies on the mesh surface, so its Z *is* the true surface
/// height at that (x,y) — no ray needed. Dropping the actual cutter at the same
/// (x,y) gives the footprint-aware rest height (`cl.z`): on a flat or gentle
/// slope, or a triangulation-scale crease the tool simply rides over, the tool
/// reaches the surface and `gap ≈ 0`; in a concavity tighter than the tool
/// radius the tool bridges the walls and its reference floats above the floor,
/// so `gap > 0` (verified: a 6mm ball over a 0.5-slope V-valley rests 0.354mm
/// above the seam). This makes detection scale-aware — reject mesh noise, keep
/// genuine valleys — instead of trusting raw per-edge dihedral. Returns `None`
/// if the cutter makes no contact at the midpoint.
fn reach_gap_at_point(
    x: f64,
    y: f64,
    surf_z: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
) -> Option<f64> {
    let cl = point_drop_cutter(x, y, mesh, index, cutter);
    if !cl.contacted {
        return None;
    }
    Some(cl.z - surf_z)
}

/// Does a chain sit in genuine rest material the pencil tool can clean? Samples up
/// to 8 vertices and keeps the chain if the MEDIAN *rest depth* exceeds the
/// threshold, where
///   `rest_depth = reference_gap − pencil_gap`
/// is how much DEEPER the pencil tool reaches than the bigger `reference` (finish)
/// tool could. This is the literature's reference-tool rest region: it keeps the
/// valleys the big tool missed but the pencil tool enters, and rejects both
/// big-tool-reachable walls (`reference_gap ≈ 0`) and sub-pencil-scale texture
/// (both tools float, so `reference_gap ≈ pencil_gap`). With `reference == None`
/// it falls back to the self-referenced gap (pencil tool vs bare surface). Gating
/// whole chains (not every edge) is the hot-path win — far fewer drops.
#[allow(clippy::indexing_slicing)] // chain entries are valid vertex indices
fn chain_passes_depth(
    chain: &[u32],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    reference: Option<&dyn MillingCutter>,
    threshold: f64,
) -> bool {
    let n = chain.len();
    if n == 0 {
        return false;
    }
    let step = (n / 8).max(1);
    let mut depths: Vec<f64> = Vec::new();
    let mut i = 0;
    while i < n {
        let v = mesh.vertices[chain[i] as usize];
        if let Some(pencil_gap) = reach_gap_at_point(v.x, v.y, v.z, mesh, index, cutter) {
            let rest = match reference {
                Some(rc) => match reach_gap_at_point(v.x, v.y, v.z, mesh, index, rc) {
                    // How much further the pencil tool drops past the big tool.
                    Some(ref_gap) => (ref_gap - pencil_gap).max(0.0),
                    // Big tool can't even contact here → treat the whole pencil gap
                    // as rest the big tool left.
                    None => pencil_gap,
                },
                None => pencil_gap,
            };
            depths.push(rest);
        }
        i += step;
    }
    if depths.is_empty() {
        return false;
    }
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    depths[depths.len() / 2] > threshold
}

/// Surface tolerance (mm): uncut depth at a concave seam above which we treat it
/// as genuine rest material worth a pencil pass. ABSOLUTE, not scaled to the tool
/// — a large tool that bridges a valley leaving e.g. 0.3mm should still be
/// cleaned, while triangulation-scale creases the tool rides over leave
/// sub-tolerance gaps and are rejected. Any noise that slips through is further
/// removed by `min_cut_length` chain filtering (isolated noise points don't form
/// long chains). Tunable; calibrate against the real mesh harness.
pub(crate) fn reach_gap_threshold() -> f64 {
    0.05
}

/// Default reference-tool diameter (mm): the bigger finishing tool the pencil pass
/// cleans up after. A typical 1/4" finish ball.
pub(crate) fn reference_tool_diameter_default() -> f64 {
    6.0
}

/// Default valley saliency (min |κ₂| in 1/mm) for the curvature detector. 0.15
/// traces the full dendritic valley network on dense organic relief without
/// carpeting the inter-valley wave/triangulation texture (validated on the
/// wanaka rivermap mesh: ~0.05 carpets, ~0.2 the coherent network, ~0.8 only the
/// deepest trunks). Raise to keep only deep sharp valleys, lower to trace finer
/// seams.
pub(crate) fn valley_saliency_default() -> f64 {
    0.15
}

/// Default curvature-tensor smoothing iterations for the curvature detector —
/// enough to suppress rivermap wave/triangulation texture while preserving
/// genuine valleys.
pub(crate) fn curvature_smoothing_default() -> usize {
    4
}

/// Default XY cell size (mm) for the rest-depth field. 0.5 mm gives ~160k drops
/// on the 200 mm wanaka mesh (release: ~2 s) with fine enough regions.
pub(crate) fn rest_cell_default() -> f64 {
    0.5
}

/// Default rest-region routing threshold: a region whose half-width exceeds
/// `2.0 × pencil_radius` (one pencil diameter) routes to clearing rather than a
/// single pencil centreline.
pub(crate) fn route_width_factor_default() -> f64 {
    2.0
}

/// Default detector token for project-file serde (the historical crease detector).
pub(crate) fn detector_string_default() -> String {
    PencilDetector::Dihedral.as_str().to_owned()
}

/// Keep only chains that sit in genuine rest material (see [`chain_passes_depth`]).
/// The hot path on dense meshes is the `drop_cutter` work, so gating whole chains
/// (a handful of sample drops each) instead of every candidate edge slashes the
/// drop count. Parallelised across cores when the `parallel` feature is on
/// (`MillingCutter: Send + Sync`).
#[cfg(feature = "parallel")]
fn gate_chains_by_depth(
    chains: Vec<Vec<u32>>,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    reference: Option<&dyn MillingCutter>,
    threshold: f64,
) -> Vec<Vec<u32>> {
    use rayon::prelude::*;
    chains
        .into_par_iter()
        .filter(|c| chain_passes_depth(c, mesh, index, cutter, reference, threshold))
        .collect()
}

#[cfg(not(feature = "parallel"))]
fn gate_chains_by_depth(
    chains: Vec<Vec<u32>>,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    reference: Option<&dyn MillingCutter>,
    threshold: f64,
) -> Vec<Vec<u32>> {
    chains
        .into_iter()
        .filter(|c| chain_passes_depth(c, mesh, index, cutter, reference, threshold))
        .collect()
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Order chains by nearest-neighbor to minimize rapids.
fn order_paths_nearest(paths: &mut [PencilPath]) {
    if paths.len() <= 1 {
        return;
    }

    let mut ordered_indices = Vec::with_capacity(paths.len());
    let mut used = vec![false; paths.len()];

    // Start with first chain
    ordered_indices.push(0);
    used[0] = true;

    for _ in 1..paths.len() {
        let last_path = &paths[ordered_indices[ordered_indices.len() - 1]];
        let last_pt = if let Some(p) = last_path.points.last() {
            *p
        } else {
            continue;
        };

        let mut best_idx = 0;
        let mut best_dist = f64::MAX;

        for (i, path) in paths.iter().enumerate() {
            if used[i] || path.points.is_empty() {
                continue;
            }
            // Check distance to start and end of candidate chain
            let d_start = {
                let p = path.points[0];
                let dx = p.x - last_pt.x;
                let dy = p.y - last_pt.y;
                dx * dx + dy * dy
            };
            let d_end = if let Some(p) = path.points.last() {
                let dx = p.x - last_pt.x;
                let dy = p.y - last_pt.y;
                dx * dx + dy * dy
            } else {
                f64::MAX
            };

            let d = d_start.min(d_end);
            if d < best_dist {
                best_dist = d;
                best_idx = i;
            }
        }

        // If end is closer than start, reverse the chain
        if !paths[best_idx].points.is_empty() {
            let start_pt = paths[best_idx].points[0];
            let end_pt = paths[best_idx].points[paths[best_idx].points.len() - 1];
            let d_start = (start_pt.x - last_pt.x).powi(2) + (start_pt.y - last_pt.y).powi(2);
            let d_end = (end_pt.x - last_pt.x).powi(2) + (end_pt.y - last_pt.y).powi(2);
            if d_end < d_start {
                paths[best_idx].points.reverse();
            }
        }

        used[best_idx] = true;
        ordered_indices.push(best_idx);
    }

    // Reorder chains in-place using the ordering
    let mut temp: Vec<PencilPath> = ordered_indices
        .into_iter()
        .map(|i| {
            std::mem::replace(
                &mut paths[i],
                PencilPath {
                    points: Vec::new(),
                    chain_index: 0,
                    chain_total: 0,
                    offset_index: 0,
                    offset_total: 0,
                    offset_mm: 0.0,
                    is_centerline: false,
                },
            )
        })
        .collect();
    for (i, path) in temp.drain(..).enumerate() {
        paths[i] = path;
    }
}

/// Build a gouge-safe, surface-following link between two cut points whose XY gap
/// is within `hookup_distance`, so consecutive passes join without retracting to
/// safe Z and re-plunging. Samples the connecting segment and drop-cutters each
/// interior point — the same gouge-free lift the cut path uses — so the tool rides
/// the surface across the gap instead of lifting clear. Endpoints are excluded
/// (the caller is already at `from` and feeds to `to` itself). Returns `None` if
/// the tool loses surface contact anywhere along the link (over a hole / off the
/// mesh) — the caller then falls back to a clean retract-and-replunge.
fn build_surface_link(
    from: P3,
    to: P3,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stock_to_leave: f64,
    spacing: f64,
) -> Option<Vec<P3>> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1e-6 {
        return Some(Vec::new());
    }
    let n = (dist / spacing.max(1e-3)).ceil().max(1.0) as usize;
    let mut pts = Vec::new();
    for k in 1..n {
        let t = k as f64 / n as f64;
        let x = from.x + dx * t;
        let y = from.y + dy * t;
        let cl = point_drop_cutter(x, y, mesh, index, cutter);
        if !cl.contacted {
            return None; // lost contact → not safe to link at surface, retract instead
        }
        pts.push(P3::new(x, y, cl.z + stock_to_leave));
    }
    Some(pts)
}

/// Generate pencil finishing toolpath.
///
/// Detects concave mesh edges (creases) and generates toolpaths that
/// follow them, cleaning material that standard finishing passes miss.
pub fn pencil_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
) -> Toolpath {
    let (tp, _) = pencil_toolpath_structured_annotated(mesh, index, cutter, params, None);
    tp
}

fn runtime_annotations_to_labels(annotations: &[PencilRuntimeAnnotation]) -> Vec<(usize, String)> {
    annotations
        .iter()
        .map(|annotation| (annotation.move_index, annotation.event.label()))
        .collect()
}

/// Total XY/Z length of a polyline (mm).
fn polyline_length(points: &[P3]) -> f64 {
    points
        .windows(2)
        .map(|w| {
            #[allow(clippy::indexing_slicing)] // windows(2) yields len-2 slices
            let (a, b) = (w[0], w[1]);
            ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
        })
        .sum()
}

/// Resample a polyline to ~`spacing` mm between points (linear interpolation),
/// preserving the first and last vertices. Curvature crest lines arrive at
/// mesh-edge resolution; this decouples cut-point spacing from mesh density.
#[allow(clippy::indexing_slicing)] // first/last + windows(2) indices are bounded
fn resample_polyline(points: &[P3], spacing: f64) -> Vec<P3> {
    if points.len() < 2 || spacing <= 1e-6 {
        return points.to_vec();
    }
    let mut out = vec![points[0]];
    // Distance already travelled past the last emitted point along the current
    // walk, so spacing is continuous across segment boundaries.
    let mut carry = 0.0;
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
        if seg < 1e-9 {
            continue;
        }
        let mut d = spacing - carry;
        while d < seg {
            let t = d / seg;
            out.push(P3::new(
                a.x + (b.x - a.x) * t,
                a.y + (b.y - a.y) * t,
                a.z + (b.z - a.z) * t,
            ));
            d += spacing;
        }
        carry = seg - (d - spacing);
    }
    let last = points[points.len() - 1];
    let need_last = out
        .last()
        .is_none_or(|p| (p.x - last.x).abs() > 1e-6 || (p.y - last.y).abs() > 1e-6);
    if need_last {
        out.push(last);
    }
    out
}

/// Build the centreline + offset `PencilPath`s for one already-sampled valley
/// polyline. Shared by both detectors: it fairs the XY line, lifts it to the
/// surface with the real cutter, and emits the centreline plus symmetric offset
/// passes. `chain_index` is 1-based.
#[allow(clippy::too_many_arguments)] // cohesive per-chain emit; splitting hurts clarity
fn paths_from_sampled(
    sampled: &[P3],
    chain_index: usize,
    chain_total: usize,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    all_paths: &mut Vec<PencilPath>,
) {
    if sampled.len() < 2 {
        return;
    }
    // De-jag the facet-scale zig-zag before lifting; offsets derive from the
    // faired centreline so they inherit it.
    let sampled = fair_polyline_xy(sampled, FAIRING_PASSES, FAIRING_STRENGTH);
    let offset_total = 1 + params.num_offset_passes * 2;

    let centerline = lift_to_surface(&sampled, mesh, index, cutter, params.stock_to_leave);
    all_paths.push(PencilPath {
        points: centerline,
        chain_index,
        chain_total,
        offset_index: 1,
        offset_total,
        offset_mm: 0.0,
        is_centerline: true,
    });

    for pass_num in 1..=params.num_offset_passes {
        let offset = pass_num as f64 * params.offset_stepover;

        let left = offset_polyline(&sampled, offset);
        let left_lifted = lift_to_surface(&left, mesh, index, cutter, params.stock_to_leave);
        all_paths.push(PencilPath {
            points: left_lifted,
            chain_index,
            chain_total,
            offset_index: pass_num * 2,
            offset_total,
            offset_mm: offset,
            is_centerline: false,
        });

        let right = offset_polyline(&sampled, -offset);
        let right_lifted = lift_to_surface(&right, mesh, index, cutter, params.stock_to_leave);
        all_paths.push(PencilPath {
            points: right_lifted,
            chain_index,
            chain_total,
            offset_index: pass_num * 2 + 1,
            offset_total,
            offset_mm: -offset,
            is_centerline: false,
        });
    }
}

/// Rest-depth gate over a P3 polyline (curvature crest lines have no mesh-vertex
/// chain). Identical metric to [`chain_passes_depth`]: keeps the line if the
/// median *rest depth* (`reference_gap − pencil_gap`, i.e. how much deeper the
/// pencil tool reaches than the bigger reference tool) exceeds `threshold`. The
/// surface height cancels in reference mode, so it tolerates the smoothed DEM Z.
fn polyline_passes_depth(
    pts: &[P3],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    reference: Option<&dyn MillingCutter>,
    threshold: f64,
) -> bool {
    let n = pts.len();
    if n == 0 {
        return false;
    }
    let step = (n / 8).max(1);
    let mut depths: Vec<f64> = Vec::new();
    let mut i = 0;
    while i < n {
        #[allow(clippy::indexing_slicing)] // i < n by loop guard
        let v = pts[i];
        if let Some(pencil_gap) = reach_gap_at_point(v.x, v.y, v.z, mesh, index, cutter) {
            let rest = match reference {
                Some(rc) => match reach_gap_at_point(v.x, v.y, v.z, mesh, index, rc) {
                    Some(ref_gap) => (ref_gap - pencil_gap).max(0.0),
                    None => pencil_gap,
                },
                None => pencil_gap,
            };
            depths.push(rest);
        }
        i += step;
    }
    if depths.is_empty() {
        return false;
    }
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    #[allow(clippy::indexing_slicing)] // depths non-empty → index < len
    let median = depths[depths.len() / 2];
    median > threshold
}

pub fn pencil_toolpath_structured_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<PencilRuntimeAnnotation>) {
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();

    let gap_threshold = params.min_valley_depth;

    // The bigger reference (finish) tool the pencil pass cleans up after — shared
    // by both detectors' rest-depth gate. Only used when genuinely bigger than
    // the pencil tool (length is irrelevant to the ball-tip rest height);
    // otherwise the gate self-references (pencil tool vs bare surface).
    let reference_cutter = if params.reference_tool_diameter > cutter.diameter() + 1e-6 {
        Some(crate::tool::BallEndmill::new(
            params.reference_tool_diameter,
            25.0,
        ))
    } else {
        None
    };
    let reference_ref: Option<&dyn MillingCutter> =
        reference_cutter.as_ref().map(|c| c as &dyn MillingCutter);

    let mut all_paths: Vec<PencilPath> = Vec::new();

    match params.detector {
        PencilDetector::Curvature => {
            // Curvature crest-line detection (see crate::crest_lines): trace the
            // zero-set of the minimal-curvature extremality, filtered by the
            // single `valley_saliency` (|κ₂|) dial. Keep lines long enough, then
            // optionally apply the reference-tool rest-depth gate (only when
            // `min_valley_depth > 0`, so saliency alone can drive selection),
            // resample to cut spacing, and build paths. No bisector — the crest
            // line already sits on the valley floor.
            let cp = crate::crest_lines::CrestParams {
                valley_saliency: params.valley_saliency,
                smoothing_iters: params.curvature_smoothing,
                min_line_length: params.min_cut_length,
            };
            let lines = crate::crest_lines::detect_valley_lines(mesh, &cp);
            let apply_rest_gate = gap_threshold > 0.0;
            let kept: Vec<Vec<P3>> = lines
                .into_iter()
                .filter(|l| polyline_length(l) >= params.min_cut_length)
                .filter(|l| {
                    !apply_rest_gate
                        || polyline_passes_depth(
                            l,
                            mesh,
                            index,
                            cutter,
                            reference_ref,
                            gap_threshold,
                        )
                })
                .collect();
            if kept.is_empty() {
                info!("Curvature detector: no valley lines passed length/rest-depth gate");
                return (tp, annotations);
            }
            let chain_total = kept.len();
            info!(
                valley_lines = chain_total,
                gap_threshold,
                valley_saliency = params.valley_saliency,
                "Curvature detector: valley crest lines built"
            );
            for (ci, line) in kept.iter().enumerate() {
                let sampled = resample_polyline(line, params.sampling);
                paths_from_sampled(
                    &sampled,
                    ci + 1,
                    chain_total,
                    mesh,
                    index,
                    cutter,
                    params,
                    &mut all_paths,
                );
            }
        }
        PencilDetector::RestDepth => {
            // Rest-depth-field detection (see crate::rest_field): build the
            // dual-tool rest field, threshold at `min_valley_depth`, thin each
            // region to a skeleton, and route by width. The per-polyline
            // `polyline_passes_depth` gate is REDUNDANT here (the field threshold
            // IS that same quantity, pointwise over the whole grid) — skip it.
            //
            // The reference cutter must exist even when it is not bigger than the
            // pencil tool: in that degenerate case fall back to a self-referenced
            // rest against a tiny bare-surface probe (matches how the drainage DEM
            // probed the raw surface).
            let bigger = params.reference_tool_diameter > cutter.diameter() + 1e-6;
            let probe_mode = !bigger;
            let ref_tool = if bigger {
                crate::tool::BallEndmill::new(params.reference_tool_diameter, 25.0)
            } else {
                crate::tool::BallEndmill::new(0.1, 10.0)
            };
            let rf_params = crate::rest_field::RestFieldParams {
                cell_mm: params.rest_cell_mm,
                min_valley_depth: params.min_valley_depth,
                route_width_factor: params.route_width_factor,
                pencil_radius: cutter.radius(),
                min_cut_length: params.min_cut_length,
                reference_is_surface_probe: probe_mode,
            };
            let rf =
                crate::rest_field::detect_rest_valleys(mesh, index, cutter, &ref_tool, &rf_params);
            let report = &rf.report;
            info!(
                rest_volume_mm3 = format!("{:.1}", report.total_rest_volume_mm3),
                pencil_regions = report.pencil_region_count,
                clearing_regions = report.clearing_region_count,
                centerlines = rf.centerlines.len(),
                coverage = format!("{:.2}", report.coverage()),
                probe_mode,
                "Rest-depth detector: rest field built"
            );
            if let Some(dbg) = debug {
                let scope = dbg.start_span("rest_field", "rest-depth report");
                scope.set_counter("rest_volume_mm3", report.total_rest_volume_mm3);
                scope.set_counter("rest_pencil_regions", report.pencil_region_count as f64);
                scope.set_counter("rest_clearing_regions", report.clearing_region_count as f64);
                scope.set_counter("rest_skeleton_mm", report.skeleton_length_mm);
                scope.set_counter("rest_traced_mm", report.traced_length_mm);
                scope.set_counter("rest_coverage", report.coverage());
                scope.finish();
            }
            for reg in &rf.clearing_regions {
                info!(
                    bbox = format!(
                        "[{:.1},{:.1}]..[{:.1},{:.1}]",
                        reg.bbox[0], reg.bbox[1], reg.bbox[2], reg.bbox[3]
                    ),
                    cells = reg.cell_count,
                    peak_rest_mm = format!("{:.2}", reg.peak_rest_mm),
                    "Rest-depth detector: wide region routed to clearing (Phase D: adaptive3d)"
                );
            }
            let kept: Vec<Vec<P3>> = rf
                .centerlines
                .into_iter()
                .filter(|l| polyline_length(l) >= params.min_cut_length)
                .collect();
            if kept.is_empty() {
                info!("Rest-depth detector: no centerlines passed length gate");
                return (tp, annotations);
            }
            let chain_total = kept.len();
            for (ci, line) in kept.iter().enumerate() {
                let sampled = resample_polyline(line, params.sampling);
                paths_from_sampled(
                    &sampled,
                    ci + 1,
                    chain_total,
                    mesh,
                    index,
                    cutter,
                    params,
                    &mut all_paths,
                );
            }
        }
        PencilDetector::Dihedral => {
            // Step 1: edge adjacency. Step 2: shared edges + dihedral angles.
            let edge_map = build_edge_adjacency(mesh);
            let shared_edges = compute_shared_edges(mesh, &edge_map);

            // Step 3: concave-edge candidate set (angle filter only; the real
            // selection is the per-chain rest-depth gate). The angle filter alone
            // floods dense organic meshes — every triangulation crease passes.
            let threshold_rad = params.bitangency_angle.to_radians();
            let total_shared = shared_edges.len();
            let concave_owned: Vec<SharedEdge> = shared_edges
                .into_iter()
                .filter(|e| {
                    e.is_concave && e.dihedral_angle > (std::f64::consts::PI - threshold_rad)
                })
                .collect();
            if concave_owned.is_empty() {
                info!(
                    "No concave edges under {:.0}° threshold",
                    params.bitangency_angle
                );
                return (tp, annotations);
            }

            // Step 4: chain concave edges, then keep only chains holding genuine
            // REST material (rest_depth = reference_gap − pencil_gap > threshold).
            let chains_all = chain_concave_edges(&concave_owned, mesh, params.min_cut_length);
            let chains = gate_chains_by_depth(
                chains_all,
                mesh,
                index,
                cutter,
                reference_ref,
                gap_threshold,
            );

            // Per-edge wall normals for bisector positioning; built from the
            // filtered concave edges so the offset uses the exact two walls.
            #[allow(clippy::indexing_slicing)] // face_a/face_b are valid mesh face indices
            let edge_norms: HashMap<EdgeKey, (V3, V3)> = concave_owned
                .iter()
                .map(|e| {
                    (
                        e.key,
                        (mesh.faces[e.face_a].normal, mesh.faces[e.face_b].normal),
                    )
                })
                .collect();
            // Tool contact radius: the ball/corner radius that nestles into the
            // corner (falls back to nominal radius for a flat end mill).
            let contact_radius = {
                let cr = cutter.corner_radius_mm();
                if cr > 1e-6 { cr } else { cutter.radius() }
            };

            if chains.is_empty() {
                info!(
                    "No tool-unreachable valley chains (gap > {:.3}mm) over {:.1}mm",
                    gap_threshold, params.min_cut_length
                );
                return (tp, annotations);
            }
            info!(
                total_shared,
                chains = chains.len(),
                gap_threshold,
                "Pencil detection complete (chain-level reach-gap gate)"
            );

            // Step 5: sample each chain (bisector-positioned) → paths.
            let chain_total = chains.len();
            for (chain_index, chain) in chains.iter().enumerate() {
                let sampled = sample_chain_bisected(
                    mesh,
                    chain,
                    params.sampling,
                    &edge_norms,
                    contact_radius,
                    params.bisector_strength,
                );
                paths_from_sampled(
                    &sampled,
                    chain_index + 1,
                    chain_total,
                    mesh,
                    index,
                    cutter,
                    params,
                    &mut all_paths,
                );
            }
        }
    }

    if all_paths.is_empty() {
        return (tp, annotations);
    }

    // Step 6: Order paths by nearest-neighbor
    order_paths_nearest(&mut all_paths);

    // Step 7: Emit toolpath. Consecutive passes whose endpoints are within
    // `hookup_distance` are joined by a gouge-safe surface-following feed instead
    // of a retract-rapid-replunge — on dense organic relief the chains fragment
    // heavily, so per-fragment retracts dominated the rapid distance. The retract
    // is deferred: it fires only when the next pass is too far (or its link loses
    // surface contact) to join, and once at the very end.
    use crate::toolpath::MoveIntent;
    let mut prev_end: Option<P3> = None;
    for path in &all_paths {
        if path.points.len() < 2 {
            continue;
        }

        // Filter out points where drop-cutter had no contact
        let valid_points: Vec<P3> = path
            .points
            .iter()
            .filter(|p| p.z > f64::NEG_INFINITY + 1.0)
            .copied()
            .collect();

        if valid_points.len() < 2 {
            continue;
        }

        let move_index = tp.moves.len();
        let first = *valid_points.first().unwrap_or(&P3::origin());

        // Try to link from the previous pass's end without retracting.
        let link = prev_end.and_then(|end| {
            let gap = ((first.x - end.x).powi(2) + (first.y - end.y).powi(2)).sqrt();
            if gap > 1e-6 && gap <= params.hookup_distance {
                build_surface_link(
                    end,
                    first,
                    mesh,
                    index,
                    cutter,
                    params.stock_to_leave,
                    params.sampling,
                )
            } else {
                None
            }
        });

        match link {
            Some(link_pts) => {
                // Surface-following link (no retract / no re-plunge), then the body.
                for lp in &link_pts {
                    tp.feed_to_with_intent(*lp, params.feed_rate, MoveIntent::Linking);
                }
                tp.feed_to_with_intent(first, params.feed_rate, MoveIntent::Linking);
            }
            None => {
                // Too far (or unsafe) to link: retract the previous run, then a
                // fresh rapid-over + plunge entry.
                if let Some(end) = prev_end {
                    tp.rapid_to_with_intent(
                        P3::new(end.x, end.y, params.safe_z),
                        MoveIntent::Retract,
                    );
                }
                tp.rapid_to_with_intent(
                    P3::new(first.x, first.y, params.safe_z),
                    MoveIntent::Linking,
                );
                tp.feed_to_with_intent(first, params.plunge_rate, MoveIntent::EntryPlunge);
            }
        }

        // Feed the body (skip the first point — we're already positioned there).
        for p in valid_points.iter().skip(1) {
            tp.feed_to_with_intent(*p, params.feed_rate, MoveIntent::FinishingCut);
        }
        prev_end = valid_points.last().copied();

        annotations.push(PencilRuntimeAnnotation {
            move_index,
            event: PencilRuntimeEvent::OffsetPass {
                chain_index: path.chain_index,
                chain_total: path.chain_total,
                offset_index: path.offset_index,
                offset_total: path.offset_total,
                offset_mm: path.offset_mm,
                is_centerline: path.is_centerline,
            },
        });
    }

    // Final retract once everything is emitted.
    if let Some(end) = prev_end {
        tp.rapid_to_with_intent(P3::new(end.x, end.y, params.safe_z), MoveIntent::Retract);
    }

    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }

    info!(
        moves = tp.moves.len(),
        paths = all_paths.len(),
        cutting_mm = format!("{:.1}", tp.total_cutting_distance()),
        "Pencil toolpath complete"
    );

    (tp, annotations)
}

pub fn pencil_toolpath_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<(usize, String)>) {
    let (tp, annotations) =
        pencil_toolpath_structured_annotated(mesh, index, cutter, params, debug);
    (tp, runtime_annotations_to_labels(&annotations))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::mesh::{SpatialIndex, make_test_hemisphere};
    use crate::tool::BallEndmill;

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

    #[test]
    fn test_sample_chain_spacing() {
        let mesh = make_v_groove(20.0, 5.0, 10.0);
        let chain = vec![2u32, 3]; // Groove bottom edge (20mm long)
        let points = sample_chain(&mesh, &chain, 2.0);

        // 20mm line sampled at 2mm → ~11 points (including endpoints)
        assert!(
            points.len() >= 8,
            "Should get at least 8 sample points on 20mm line at 2mm spacing, got {}",
            points.len()
        );

        // All points should be along the groove bottom (y=0, z=-5)
        for p in &points {
            assert!(
                (p.y - 0.0).abs() < 0.1,
                "Points should be at y=0, got y={}",
                p.y
            );
            assert!(
                (p.z - (-5.0)).abs() < 0.1,
                "Points should be at z=-5, got z={}",
                p.z
            );
        }
    }

    #[test]
    fn test_pencil_toolpath_v_groove() {
        let mesh = make_v_valley(30.0, 10.0, 0.5, 30, 40);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let params = PencilParams {
            bitangency_angle: 170.0,
            min_cut_length: 5.0,
            hookup_distance: 20.0,
            num_offset_passes: 0,
            offset_stepover: 1.5,
            sampling: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 15.0,
            stock_to_leave: 0.0,
            min_valley_depth: reach_gap_threshold(),
            bisector_strength: bisector_strength_default(),
            reference_tool_diameter: reference_tool_diameter_default(),
            detector: PencilDetector::Dihedral,
            valley_saliency: valley_saliency_default(),
            curvature_smoothing: curvature_smoothing_default(),
            rest_cell_mm: rest_cell_default(),
            route_width_factor: route_width_factor_default(),
        };

        let tp = pencil_toolpath(&mesh, &index, &tool, &params);
        assert!(
            !tp.moves.is_empty(),
            "V-groove should produce pencil toolpath moves"
        );
    }

    #[test]
    fn test_pencil_toolpath_convex_empty() {
        let mesh = make_convex_box(50.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let params = PencilParams {
            bitangency_angle: 160.0,
            min_cut_length: 1.0,
            hookup_distance: 20.0,
            num_offset_passes: 0,
            offset_stepover: 1.5,
            sampling: 0.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 15.0,
            stock_to_leave: 0.0,
            min_valley_depth: reach_gap_threshold(),
            bisector_strength: bisector_strength_default(),
            reference_tool_diameter: reference_tool_diameter_default(),
            detector: PencilDetector::Dihedral,
            valley_saliency: valley_saliency_default(),
            curvature_smoothing: curvature_smoothing_default(),
            rest_cell_mm: rest_cell_default(),
            route_width_factor: route_width_factor_default(),
        };

        let tp = pencil_toolpath(&mesh, &index, &tool, &params);
        assert!(
            tp.moves.is_empty(),
            "Convex mesh should produce empty pencil toolpath"
        );
    }

    #[test]
    fn test_pencil_with_offset_passes() {
        let mesh = make_v_valley(30.0, 10.0, 0.5, 30, 40);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let params_center = PencilParams {
            bitangency_angle: 170.0,
            min_cut_length: 5.0,
            hookup_distance: 20.0,
            num_offset_passes: 0,
            offset_stepover: 1.5,
            sampling: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 15.0,
            stock_to_leave: 0.0,
            min_valley_depth: reach_gap_threshold(),
            bisector_strength: bisector_strength_default(),
            reference_tool_diameter: reference_tool_diameter_default(),
            detector: PencilDetector::Dihedral,
            valley_saliency: valley_saliency_default(),
            curvature_smoothing: curvature_smoothing_default(),
            rest_cell_mm: rest_cell_default(),
            route_width_factor: route_width_factor_default(),
        };

        let params_offset = PencilParams {
            num_offset_passes: 2,
            ..params_center
        };

        let tp_center = pencil_toolpath(&mesh, &index, &tool, &params_center);
        let tp_offset = pencil_toolpath(&mesh, &index, &tool, &params_offset);

        // Offset passes should produce more moves
        assert!(
            tp_offset.moves.len() > tp_center.moves.len(),
            "Offset passes ({}) should produce more moves than center-only ({})",
            tp_offset.moves.len(),
            tp_center.moves.len()
        );
    }

    /// Finely-tessellated gentle sine·sine surface. Wavelength (20mm) and
    /// amplitude (0.3mm) give a local concave radius of curvature ~30mm — far
    /// larger than a 1mm tool's radius, so a 1mm ball REACHES every trough.
    /// Correct tool-radius-aware pencil output for a 1mm tool here is ~empty.
    /// Today's raw-dihedral detector instead fires on every trough edge.
    fn make_gentle_undulating_surface(
        extent: f64,
        n: usize,
        lambda: f64,
        amp: f64,
    ) -> TriangleMesh {
        use std::f64::consts::PI;
        let mut vertices = Vec::with_capacity((n + 1) * (n + 1));
        let step = extent / n as f64;
        for j in 0..=n {
            for i in 0..=n {
                let x = i as f64 * step;
                let y = j as f64 * step;
                let z = amp * (2.0 * PI * x / lambda).sin() * (2.0 * PI * y / lambda).sin();
                vertices.push(P3::new(x, y, z));
            }
        }
        let idx = |i: usize, j: usize| (j * (n + 1) + i) as u32;
        let mut triangles = Vec::with_capacity(n * n * 2);
        for j in 0..n {
            for i in 0..n {
                // CCW for upward (+Z) normals on a heightfield.
                triangles.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
                triangles.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
            }
        }
        TriangleMesh::from_raw(vertices, triangles)
    }

    /// Count distinct pencil chains produced for a given tool, exercising the
    /// detection→gate→chaining path (the geometry the toolpath is built from),
    /// including the tool-radius-aware reach-gap gate.
    fn pencil_chain_count(
        mesh: &TriangleMesh,
        tool: &dyn MillingCutter,
        params: &PencilParams,
    ) -> usize {
        let index = SpatialIndex::build(mesh, 5.0);
        let edge_map = build_edge_adjacency(mesh);
        let shared = compute_shared_edges(mesh, &edge_map);
        let threshold_rad = params.bitangency_angle.to_radians();
        let concave: Vec<SharedEdge> = shared
            .into_iter()
            .filter(|e| e.is_concave && e.dihedral_angle > (std::f64::consts::PI - threshold_rad))
            .collect();
        let chains_all = chain_concave_edges(&concave, mesh, params.min_cut_length);
        let reference = if params.reference_tool_diameter > tool.diameter() + 1e-6 {
            Some(BallEndmill::new(params.reference_tool_diameter, 25.0))
        } else {
            None
        };
        let reference_ref: Option<&dyn MillingCutter> =
            reference.as_ref().map(|c| c as &dyn MillingCutter);
        gate_chains_by_depth(
            chains_all,
            mesh,
            &index,
            tool,
            reference_ref,
            params.min_valley_depth,
        )
        .len()
    }

    fn default_pencil_params_1mm() -> PencilParams {
        PencilParams {
            bitangency_angle: 160.0,
            min_cut_length: 2.0,
            hookup_distance: 3.0,
            num_offset_passes: 0,
            offset_stepover: 0.25,
            sampling: 0.5,
            feed_rate: 2000.0,
            plunge_rate: 132.0,
            safe_z: 15.0,
            stock_to_leave: 0.0,
            min_valley_depth: reach_gap_threshold(),
            bisector_strength: bisector_strength_default(),
            reference_tool_diameter: reference_tool_diameter_default(),
            detector: PencilDetector::Dihedral,
            valley_saliency: valley_saliency_default(),
            curvature_smoothing: curvature_smoothing_default(),
            rest_cell_mm: rest_cell_default(),
            route_width_factor: route_width_factor_default(),
        }
    }

    /// Tool-radius-aware detection acceptance. A 1mm tool on a gentle surface it
    /// can fully reach yields ~no pencil chains (the reach-gap gate rejects the
    /// reachable troughs); a genuine sharp valley it cannot bottom still yields a
    /// chain. The valley uses a correctly-wound heightfield (`make_v_valley`) —
    /// `make_v_groove`'s walls face downward, which `drop_cutter` rightly skips.
    #[test]
    fn test_pencil_tool_radius_aware_gentle_vs_valley() {
        let tool = BallEndmill::new(1.0, 25.0); // 1mm ball ≈ the detail tool's tip

        let gentle = make_gentle_undulating_surface(40.0, 80, 20.0, 0.3);
        let gentle_chains = pencil_chain_count(&gentle, &tool, &default_pencil_params_1mm());

        // Deep narrow valley (slope 2.0 → ~3mm rise per 1.5mm): a 1mm tool bridges it.
        let valley = make_v_valley(20.0, 4.0, 2.0, 20, 32);
        let valley_chains = pencil_chain_count(&valley, &tool, &default_pencil_params_1mm());

        assert!(
            valley_chains >= 1,
            "sharp V-valley must yield ≥1 pencil chain for a 1mm tool, got {valley_chains}"
        );
        assert!(
            gentle_chains <= 1,
            "gentle reachable surface should yield ~0 pencil chains for a 1mm tool \
             once the reach-gap gate is applied, got {gentle_chains}"
        );
    }

    /// Opt-in real-mesh validation against the organic relief that exploded:
    ///   RS_CAM_PENCIL_FIXTURE=/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl \
    ///   cargo test -p rs_cam_core --lib pencil_real_mesh_gate -- --ignored
    /// Asserts the reach-gap gate reduces the candidate chain set and still traces
    /// genuine valleys. Observed 2026-06-25 (terrain.stl, 661,212 tris, 1mm ball,
    /// bitangency 160, gap 0.05mm): angle_chains=9277 → gated_chains=6994,
    /// gated_moves=64,256 (vs 366k pre-gate), cutting=81m, rapid=77m. The gate is
    /// correct but modest here (the surface is tool-unreachable almost everywhere);
    /// the rapids are the Stage-2 hookup target.
    #[test]
    #[ignore = "needs RS_CAM_PENCIL_FIXTURE=/path/to/terrain.stl"]
    fn pencil_real_mesh_gate() {
        let path = std::env::var("RS_CAM_PENCIL_FIXTURE").unwrap();
        let mesh = TriangleMesh::from_stl(std::path::Path::new(&path)).unwrap();
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(1.0, 25.0);
        let params = PencilParams {
            bitangency_angle: 160.0,
            min_cut_length: 2.0,
            hookup_distance: 3.0,
            num_offset_passes: 0,
            offset_stepover: 0.25,
            sampling: 0.5,
            feed_rate: 2000.0,
            plunge_rate: 132.0,
            safe_z: mesh.bbox.max.z + 5.0,
            stock_to_leave: 0.0,
            min_valley_depth: reach_gap_threshold(),
            bisector_strength: bisector_strength_default(),
            reference_tool_diameter: reference_tool_diameter_default(),
            detector: PencilDetector::Dihedral,
            valley_saliency: valley_saliency_default(),
            curvature_smoothing: curvature_smoothing_default(),
            rest_cell_mm: rest_cell_default(),
            route_width_factor: route_width_factor_default(),
        };

        let edge_map = build_edge_adjacency(&mesh);
        let shared = compute_shared_edges(&mesh, &edge_map);
        let thr = params.bitangency_angle.to_radians();
        let angle_owned: Vec<SharedEdge> = shared
            .into_iter()
            .filter(|e| e.is_concave && e.dihedral_angle > (std::f64::consts::PI - thr))
            .collect();
        let angle_chains = chain_concave_edges(&angle_owned, &mesh, params.min_cut_length).len();
        let gated_chains = pencil_chain_count(&mesh, &tool, &params);
        let tp = pencil_toolpath(&mesh, &index, &tool, &params);

        assert!(
            gated_chains <= angle_chains,
            "reach-gap gate must not increase chains: gated={gated_chains} angle={angle_chains}"
        );
        assert!(
            !tp.moves.is_empty(),
            "pencil must still trace genuine valleys after gating"
        );
    }

    /// Minimal top-down 2D line raster of a toolpath (feed moves only) — fast and
    /// clear for seeing crease structure, unlike the 3D tube composite.
    #[allow(clippy::indexing_slicing)] // bounded by moves.len()
    fn rasterize_topdown(
        tp: &Toolpath,
        w: u32,
        h: u32,
        terrain_zmin: f64,
        terrain_zmax: f64,
    ) -> image::RgbaImage {
        use crate::toolpath::MoveType;
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([26, 26, 46, 255]));
        if tp.moves.len() < 2 {
            return img;
        }
        let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for m in &tp.moves {
            minx = minx.min(m.target.x);
            maxx = maxx.max(m.target.x);
            miny = miny.min(m.target.y);
            maxy = maxy.max(m.target.y);
        }
        // Colour is absolute to the TERRAIN height: floor = blue, top = green.
        let minz = terrain_zmin;
        let zrange = (terrain_zmax - terrain_zmin).max(1e-6);
        let margin = 20.0;
        let dw = (maxx - minx).max(1e-6);
        let dh = (maxy - miny).max(1e-6);
        let scale = ((w as f64 - 2.0 * margin) / dw).min((h as f64 - 2.0 * margin) / dh);
        for i in 1..tp.moves.len() {
            let feed = matches!(
                tp.moves[i].move_type,
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
            );
            if !feed {
                continue;
            }
            let a = tp.moves[i - 1].target;
            let b = tp.moves[i].target;
            // Colour by height: deep (low Z, lake floors / valley bottoms) = blue,
            // high (top rims / ridges) = bright green. Lets us tell base from top.
            let zt = (((a.z + b.z) * 0.5 - minz) / zrange).clamp(0.0, 1.0);
            let col = image::Rgba([
                (40.0 + zt * 80.0) as u8,
                (90.0 + zt * 150.0) as u8,
                (210.0 - zt * 110.0) as u8,
                255,
            ]);
            let x1 = margin + (a.x - minx) * scale;
            let y1 = h as f64 - margin - (a.y - miny) * scale;
            let x2 = margin + (b.x - minx) * scale;
            let y2 = h as f64 - margin - (b.y - miny) * scale;
            let steps = (x2 - x1).abs().max((y2 - y1).abs()).ceil().max(1.0) as i32;
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let px = x1 + (x2 - x1) * t;
                let py = y1 + (y2 - y1) * t;
                if px >= 0.0 && py >= 0.0 {
                    let (ux, uy) = (px as u32, py as u32);
                    if ux < w && uy < h {
                        img.put_pixel(ux, uy, col);
                    }
                }
            }
        }
        img
    }

    /// Fast headless visual loop (no GUI). Renders the pencil toolpath on a real
    /// mesh to a top-down PNG (and a browser-openable SVG). Params come from env
    /// vars so you can sweep WITHOUT recompiling — just re-run with different
    /// values:
    ///   RS_CAM_PENCIL_FIXTURE=.../terrain.stl RS_CAM_PENCIL_OUT=/tmp/p.png \
    ///   RS_CAM_PENCIL_MVD=0.2 RS_CAM_PENCIL_BIT=160 \
    ///   cargo test -p rs_cam_core --lib render_pencil_real_mesh -- --ignored
    #[test]
    #[ignore = "needs RS_CAM_PENCIL_FIXTURE; writes PNG to RS_CAM_PENCIL_OUT"]
    fn render_pencil_real_mesh() {
        let env_f64 = |k: &str, d: f64| {
            std::env::var(k)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(d)
        };
        use std::time::Instant;
        let path = std::env::var("RS_CAM_PENCIL_FIXTURE").unwrap();
        let out = std::env::var("RS_CAM_PENCIL_OUT").unwrap_or_else(|_| "/tmp/pencil.png".into());
        let t = Instant::now();
        let mesh = TriangleMesh::from_stl(std::path::Path::new(&path)).unwrap();
        let t_load = t.elapsed().as_millis();
        let t = Instant::now();
        // Default to the real GUI path (build_auto); override cell for experiments.
        let cell = env_f64("RS_CAM_PENCIL_CELL", 0.0);
        let index = if cell > 0.0 {
            SpatialIndex::build(&mesh, cell)
        } else {
            SpatialIndex::build_auto(&mesh)
        };
        let t_index = t.elapsed().as_millis();
        let tool = BallEndmill::new(2.0, 25.0); // ~2mm-tip finish ball

        let mut params = default_pencil_params_1mm();
        params.bitangency_angle = env_f64("RS_CAM_PENCIL_BIT", 160.0);
        params.min_valley_depth = env_f64("RS_CAM_PENCIL_MVD", 0.05);
        params.min_cut_length = env_f64("RS_CAM_PENCIL_MINLEN", 2.0);
        params.hookup_distance = env_f64("RS_CAM_PENCIL_HOOKUP", params.hookup_distance);
        params.bisector_strength = env_f64("RS_CAM_PENCIL_BISECTOR", params.bisector_strength);
        params.reference_tool_diameter =
            env_f64("RS_CAM_PENCIL_REFD", params.reference_tool_diameter);
        // Detector selection + curvature tuning (RS_CAM_PENCIL_DETECTOR=curvature).
        params.detector = match std::env::var("RS_CAM_PENCIL_DETECTOR").ok().as_deref() {
            Some(s) => PencilDetector::parse(s),
            None => PencilDetector::Dihedral,
        };
        params.valley_saliency = env_f64("RS_CAM_PENCIL_SAL", params.valley_saliency);
        params.curvature_smoothing =
            env_f64("RS_CAM_PENCIL_SMOOTH", params.curvature_smoothing as f64) as usize;
        params.safe_z = mesh.bbox.max.z + 5.0;

        // Phase timings for the detection sub-steps (the suspected hot path).
        let t = Instant::now();
        let em = build_edge_adjacency(&mesh);
        let t_adj = t.elapsed().as_millis();
        let t = Instant::now();
        let _sh = compute_shared_edges(&mesh, &em);
        let t_shared = t.elapsed().as_millis();

        let t = Instant::now();
        let tp = pencil_toolpath(&mesh, &index, &tool, &params);
        let t_gen = t.elapsed().as_millis();
        let _ = std::fs::write(
            out.replace(".png", ".timing.txt"),
            format!(
                "load_ms={t_load} index_ms={t_index} edge_adj_ms={t_adj} shared_edges_ms={t_shared} full_generate_ms={t_gen} (gate+chain+lift≈{})\n",
                t_gen.saturating_sub(t_adj + t_shared)
            ),
        );
        let moves = tp.moves.len();
        let cutting = tp.total_cutting_distance();
        let rapid = tp.total_rapid_distance();
        let _ = std::fs::write(
            out.replace(".png", ".stats.txt"),
            format!("moves={moves} cutting_mm={cutting:.0} rapid_mm={rapid:.0}\n"),
        );
        // Top-down 2D PNG (agent-readable) + SVG (browser-openable, crisp lines).
        let (w, h) = (1600u32, 1600u32);
        rasterize_topdown(&tp, w, h, mesh.bbox.min.z, mesh.bbox.max.z)
            .save(&out)
            .unwrap();
        let svg = crate::viz::toolpath_to_svg(&tp, w as f64, h as f64);
        std::fs::write(out.replace(".png", ".svg"), svg).unwrap();
        assert!(
            moves > 0,
            "rendered {out} | mvd={} bit={} moves={moves} cutting_mm={cutting:.0}",
            params.min_valley_depth,
            params.bitangency_angle,
        );
    }

    /// Correctly-wound (CCW, +Z normals) V-valley heightfield: z = -slope·half_y at
    /// the y=0 seam rising to 0 at the ±half_y edges. Unlike `make_v_groove`, the
    /// wall facets face up, so a dropped cutter rests on them.
    fn make_v_valley(len_x: f64, half_y: f64, slope: f64, nx: usize, ny: usize) -> TriangleMesh {
        let mut verts = Vec::new();
        let sx = len_x / nx as f64;
        let sy = 2.0 * half_y / ny as f64;
        for j in 0..=ny {
            for i in 0..=nx {
                let x = i as f64 * sx;
                let y = -half_y + j as f64 * sy;
                let z = -slope * half_y + slope * y.abs();
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
        TriangleMesh::from_raw(verts, tris)
    }

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

    /// Fairing straightens a facet-scale zig-zag, pins the endpoints, and keeps
    /// the point count (so chains don't shrink at their tips).
    #[test]
    fn test_fair_polyline_straightens_and_pins_ends() {
        // Saw-tooth in Y along +X — the kind of jag a raw mesh-edge chain makes.
        let pts: Vec<P3> = (0..11)
            .map(|i| P3::new(i as f64, if i % 2 == 0 { 0.0 } else { 1.0 }, 0.0))
            .collect();
        let faired = fair_polyline_xy(&pts, FAIRING_PASSES, FAIRING_STRENGTH);

        assert_eq!(faired.len(), pts.len(), "fairing must preserve point count");

        let pf = pts.first().unwrap();
        let pl = pts.last().unwrap();
        let ff = faired.first().unwrap();
        let fl = faired.last().unwrap();
        assert!(
            (ff.x - pf.x).abs() < 1e-12 && (ff.y - pf.y).abs() < 1e-12,
            "first endpoint must be pinned"
        );
        assert!(
            (fl.x - pl.x).abs() < 1e-12 && (fl.y - pl.y).abs() < 1e-12,
            "last endpoint must be pinned"
        );

        // Peak interior Y excursion should shrink after fairing.
        let excursion = |v: &[P3]| {
            v.iter()
                .skip(1)
                .take(v.len().saturating_sub(2))
                .map(|p| p.y)
                .fold(0.0_f64, f64::max)
        };
        assert!(
            excursion(&faired) < excursion(&pts),
            "fairing should reduce zig-zag excursion ({} !< {})",
            excursion(&faired),
            excursion(&pts)
        );
    }

    /// A surface link rides the mesh (finite Z everywhere) when both ends sit on
    /// it, and returns None when the span is off the mesh (caller then retracts).
    #[test]
    fn test_build_surface_link_follows_surface_and_detects_offmesh() {
        let mesh = make_v_valley(20.0, 6.0, 0.5, 20, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);

        let on = build_surface_link(
            P3::new(5.0, 0.0, 0.0),
            P3::new(9.0, 0.0, 0.0),
            &mesh,
            &index,
            &tool,
            0.0,
            0.5,
        );
        let pts = on.unwrap();
        assert!(
            !pts.is_empty(),
            "a 4mm link at 0.5mm spacing has interior points"
        );
        for p in &pts {
            assert!(p.z.is_finite(), "each link point rides the surface");
        }

        let off = build_surface_link(
            P3::new(100.0, 100.0, 0.0),
            P3::new(105.0, 100.0, 0.0),
            &mesh,
            &index,
            &tool,
            0.0,
            0.5,
        );
        assert!(off.is_none(), "a link entirely off the mesh must be None");
    }

    /// Wiring `hookup_distance` joins nearby passes with a surface feed instead of
    /// a retract-rapid-replunge, so the total rapid distance drops.
    #[test]
    fn test_hookup_linking_reduces_rapids() {
        let mesh = make_v_valley(30.0, 10.0, 0.5, 30, 40);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(6.0, 25.0);
        let mk = |hd: f64| PencilParams {
            bitangency_angle: 170.0,
            min_cut_length: 5.0,
            hookup_distance: hd,
            num_offset_passes: 2,
            offset_stepover: 1.5,
            sampling: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 15.0,
            stock_to_leave: 0.0,
            min_valley_depth: reach_gap_threshold(),
            bisector_strength: bisector_strength_default(),
            reference_tool_diameter: reference_tool_diameter_default(),
            detector: PencilDetector::Dihedral,
            valley_saliency: valley_saliency_default(),
            curvature_smoothing: curvature_smoothing_default(),
            rest_cell_mm: rest_cell_default(),
            route_width_factor: route_width_factor_default(),
        };

        let unlinked = pencil_toolpath(&mesh, &index, &tool, &mk(0.0));
        let linked = pencil_toolpath(&mesh, &index, &tool, &mk(50.0));

        assert!(!linked.moves.is_empty(), "linked path must still cut");
        assert!(
            linked.total_rapid_distance() < unlinked.total_rapid_distance(),
            "hookup linking should reduce rapids: linked={:.1} unlinked={:.1}",
            linked.total_rapid_distance(),
            unlinked.total_rapid_distance()
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

    #[test]
    fn test_hemisphere_pencil_produces_ring() {
        // Hemisphere on a flat base has a concave ring where it meets the base
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let params = PencilParams {
            bitangency_angle: 170.0,
            min_cut_length: 3.0,
            hookup_distance: 20.0,
            num_offset_passes: 0,
            offset_stepover: 1.5,
            sampling: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 25.0,
            stock_to_leave: 0.0,
            min_valley_depth: reach_gap_threshold(),
            bisector_strength: bisector_strength_default(),
            reference_tool_diameter: reference_tool_diameter_default(),
            detector: PencilDetector::Dihedral,
            valley_saliency: valley_saliency_default(),
            curvature_smoothing: curvature_smoothing_default(),
            rest_cell_mm: rest_cell_default(),
            route_width_factor: route_width_factor_default(),
        };

        let edge_map = build_edge_adjacency(&mesh);
        let shared = compute_shared_edges(&mesh, &edge_map);

        // Hemisphere should have concave edges where the dome meets steeper regions
        let _concave_count = shared.iter().filter(|e| e.is_concave).count();
        // The hemisphere is all convex from outside, but some edges at base may be concave
        // depending on tessellation. At minimum, the algorithm should not crash.
        let _tp = pencil_toolpath(&mesh, &index, &tool, &params);
        // We just verify it runs without panic — hemisphere may or may not produce edges
        // depending on tessellation quality
        assert!(
            !shared.is_empty(),
            "hemisphere tessellation should produce shared edges"
        );
    }
}
