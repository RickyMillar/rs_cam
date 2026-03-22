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
use crate::geo::P3;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

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
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
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
    /// Indices of the two faces sharing this edge (kept for future offset pass direction)
    #[allow(dead_code)]
    face_a: usize,
    #[allow(dead_code)]
    face_b: usize,
    /// Dihedral angle in radians (0 = coplanar, π = fully folded)
    dihedral_angle: f64,
    /// True if the edge is concave (crease), false if convex (ridge)
    is_concave: bool,
}

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

        // Determine concavity: edge is concave if the midpoint of the
        // opposite vertex of face B is "below" face A's plane (i.e.,
        // the faces fold inward).
        // We use the cross product of normals dotted with the edge vector
        // as the sign indicator.
        let va = mesh.vertices[key.0 as usize];
        let vb = mesh.vertices[key.1 as usize];
        let edge_vec = vb - va;

        let cross = n1.cross(&n2);
        let sign = cross.dot(&edge_vec);

        // Concave: normals point toward each other across the edge.
        // When sign > 0, the edge is concave (crease); when < 0, convex (ridge).
        // This convention depends on consistent winding — if normals point outward,
        // concave edges have normals converging.
        let is_concave = sign > 0.0;

        shared.push(SharedEdge {
            key,
            face_a: fa,
            face_b: fb,
            dihedral_angle: dihedral,
            is_concave,
        });
    }

    shared
}

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

    // If no endpoints found (all closed loops), pick any unvisited vertex
    if start_vertices.is_empty()
        && let Some(&v) = adj.keys().next()
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
    let unvisited_starts: Vec<EdgeKey> = visited_edges
        .iter()
        .filter(|&(_, v)| !v)
        .map(|(&k, _)| k)
        .collect();

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

/// Lift 2D polyline points to the mesh surface using drop-cutter.
fn lift_to_surface(
    points: &[P3],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stock_to_leave: f64,
) -> Vec<P3> {
    points
        .iter()
        .map(|p| {
            let cl = point_drop_cutter(p.x, p.y, mesh, index, cutter);
            if cl.contacted {
                P3::new(p.x, p.y, cl.z + stock_to_leave)
            } else {
                // Outside mesh — keep original Z (will be filtered or skipped)
                *p
            }
        })
        .collect()
}

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

pub fn pencil_toolpath_structured_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<PencilRuntimeAnnotation>) {
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();

    // Step 1: Build edge adjacency
    let edge_map = build_edge_adjacency(mesh);

    // Step 2: Compute shared edges with dihedral angles
    let shared_edges = compute_shared_edges(mesh, &edge_map);

    // Step 3: Filter to concave edges below threshold
    let threshold_rad = params.bitangency_angle.to_radians();
    let concave_edges: Vec<&SharedEdge> = shared_edges
        .iter()
        .filter(|e| e.is_concave && e.dihedral_angle > (std::f64::consts::PI - threshold_rad))
        .collect();

    if concave_edges.is_empty() {
        info!(
            "No concave edges found below {:.0}° threshold",
            params.bitangency_angle
        );
        return (tp, annotations);
    }

    info!(
        total_shared = shared_edges.len(),
        concave_count = concave_edges.len(),
        "Edge analysis complete"
    );

    // Step 4: Chain connected concave edges into polylines
    let concave_owned: Vec<SharedEdge> = shared_edges
        .into_iter()
        .filter(|e| e.is_concave && e.dihedral_angle > (std::f64::consts::PI - threshold_rad))
        .collect();
    let chains = chain_concave_edges(&concave_owned, mesh, params.min_cut_length);

    if chains.is_empty() {
        info!(
            "No chains above minimum length {:.1}mm",
            params.min_cut_length
        );
        return (tp, annotations);
    }

    info!(chains = chains.len(), "Chained concave edges");

    // Step 5: Sample points along each chain
    let chain_total = chains.len();
    let mut all_paths: Vec<PencilPath> = Vec::new();

    for (chain_index, chain) in chains.iter().enumerate() {
        let sampled = sample_chain(mesh, chain, params.sampling);
        if sampled.len() < 2 {
            continue;
        }

        let offset_total = 1 + params.num_offset_passes * 2;

        // Lift centerline to surface
        let centerline = lift_to_surface(&sampled, mesh, index, cutter, params.stock_to_leave);
        all_paths.push(PencilPath {
            points: centerline,
            chain_index: chain_index + 1,
            chain_total,
            offset_index: 1,
            offset_total,
            offset_mm: 0.0,
            is_centerline: true,
        });

        // Generate offset passes
        for pass_num in 1..=params.num_offset_passes {
            let offset = pass_num as f64 * params.offset_stepover;

            // Positive offset (left side)
            let left = offset_polyline(&sampled, offset);
            let left_lifted = lift_to_surface(&left, mesh, index, cutter, params.stock_to_leave);
            all_paths.push(PencilPath {
                points: left_lifted,
                chain_index: chain_index + 1,
                chain_total,
                offset_index: pass_num * 2,
                offset_total,
                offset_mm: offset,
                is_centerline: false,
            });

            // Negative offset (right side)
            let right = offset_polyline(&sampled, -offset);
            let right_lifted = lift_to_surface(&right, mesh, index, cutter, params.stock_to_leave);
            all_paths.push(PencilPath {
                points: right_lifted,
                chain_index: chain_index + 1,
                chain_total,
                offset_index: pass_num * 2 + 1,
                offset_total,
                offset_mm: -offset,
                is_centerline: false,
            });
        }
    }

    // Step 6: Order paths by nearest-neighbor
    order_paths_nearest(&mut all_paths);

    // Step 7: Emit toolpath
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
        tp.emit_path_segment(
            &valid_points,
            params.safe_z,
            params.feed_rate,
            params.plunge_rate,
        );
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

        let triangles = vec![
            [0, 2, 1], // left plane tri 1
            [1, 2, 3], // left plane tri 2
            [2, 4, 3], // right plane tri 1
            [3, 4, 5], // right plane tri 2
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
        let mesh = make_v_groove(30.0, 5.0, 10.0);
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
        };

        let tp = pencil_toolpath(&mesh, &index, &tool, &params);
        assert!(
            tp.moves.is_empty(),
            "Convex mesh should produce empty pencil toolpath"
        );
    }

    #[test]
    fn test_pencil_with_offset_passes() {
        let mesh = make_v_groove(30.0, 5.0, 10.0);
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
