//! STEP file import via the `truck` crate.
//!
//! Parses AP203/AP214 STEP files, extracts BREP topology, tessellates each
//! face independently, and builds an `EnrichedMesh` with face group metadata.
//!
//! Requires the `step` feature flag.

use std::path::Path;

use thiserror::Error;
use tracing::{info, warn};

use truck_meshalgo::tessellation::{MeshedShape, RobustMeshableShape};
use truck_stepio::r#in::Table;

use crate::enriched_mesh::{
    FaceGroupId, FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh, EnrichedMesh,
};
use crate::geo::{BoundingBox3, P2, P3, V3};

#[derive(Error, Debug)]
pub enum StepImportError {
    #[error("Failed to read STEP file: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to parse STEP file (invalid format or unsupported entities)")]
    ParseError,
    #[error("No shell or solid geometry found in STEP file")]
    NoSolidFound,
    #[error("Tessellation failed for shell {shell_index}: {message}")]
    TessellationFailed { shell_index: usize, message: String },
    #[error("Failed to convert shell {shell_index} to compressed form: {message}")]
    ShellConversionFailed { shell_index: usize, message: String },
    #[error("Built mesh has no triangles")]
    EmptyResult,
}

/// Load a STEP file and produce an `EnrichedMesh`.
///
/// `tolerance` controls tessellation resolution (chord height in mm).
/// For wood routing, 0.1 mm is appropriate. Smaller values produce more triangles.
pub fn load_step(path: &Path, tolerance: f64) -> Result<EnrichedMesh, StepImportError> {
    let step_string = std::fs::read_to_string(path)?;

    let table = Table::from_step(&step_string).ok_or(StepImportError::ParseError)?;

    let shell_count = table.shell.len();
    info!(shells = shell_count, "Parsed STEP file");

    if shell_count == 0 {
        return Err(StepImportError::NoSolidFound);
    }

    let mut face_tessellations: Vec<FaceTessellation> = Vec::new();
    let mut adjacency_pairs: Vec<(FaceGroupId, FaceGroupId)> = Vec::new();
    let mut brep_edges: Vec<crate::enriched_mesh::BrepEdge> = Vec::new();

    for (shell_idx, step_shell) in table.shell.values().enumerate() {
        let cshell = table.to_compressed_shell(step_shell).map_err(|e| {
            StepImportError::ShellConversionFailed {
                shell_index: shell_idx,
                message: format!("{e:?}"),
            }
        })?;

        let face_count = cshell.faces.len();
        info!(shell = shell_idx, faces = face_count, "Processing shell");

        // Tessellate the whole shell into a single PolygonMesh first,
        // then also tessellate each face's CompressedShell individually for per-face data.
        // We create single-face shells for per-face tessellation.
        let face_base_idx = face_tessellations.len();

        for face_idx in 0..cshell.faces.len() {
            // Create a single-face shell to tessellate individually
            let single_face_shell = truck_topology::compress::CompressedShell {
                vertices: cshell.vertices.clone(),
                edges: cshell.edges.clone(),
                faces: vec![cshell.faces[face_idx].clone()],
            };

            let face_poly = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                single_face_shell.robust_triangulation(tolerance).to_polygon()
            })) {
                Ok(p) => p,
                Err(_) => {
                    warn!(shell = shell_idx, face = face_idx, "Face tessellation panicked, skipping");
                    continue;
                }
            };

            let positions = face_poly.positions();
            if positions.is_empty() {
                continue;
            }

            // Convert truck points to our P3
            let vertices: Vec<P3> = positions
                .iter()
                .map(|p| P3::new(p.x, p.y, p.z))
                .collect();

            // Extract triangles
            let mut triangles: Vec<[u32; 3]> = Vec::new();
            for tri in face_poly.tri_faces() {
                triangles.push([
                    tri[0].pos as u32,
                    tri[1].pos as u32,
                    tri[2].pos as u32,
                ]);
            }
            for quad in face_poly.quad_faces() {
                triangles.push([
                    quad[0].pos as u32,
                    quad[1].pos as u32,
                    quad[2].pos as u32,
                ]);
                triangles.push([
                    quad[0].pos as u32,
                    quad[2].pos as u32,
                    quad[3].pos as u32,
                ]);
            }

            if triangles.is_empty() {
                continue;
            }

            // Classify surface type heuristically from vertex positions
            let (surface_type, surface_params) = classify_face_surface(&vertices);

            // Extract boundary loops from mesh edges
            let boundary_loops = extract_boundary_loops(&vertices, &triangles);

            face_tessellations.push(FaceTessellation {
                vertices,
                triangles,
                surface_type,
                surface_params,
                boundary_loops,
            });
        }

        // Build adjacency and BREP edges for faces within this shell
        let face_end_idx = face_tessellations.len();
        for i in face_base_idx..face_end_idx {
            for j in (i + 1)..face_end_idx {
                if let Some(shared_verts) =
                    find_shared_edge_vertices(&face_tessellations[i], &face_tessellations[j])
                {
                    let face_a = FaceGroupId(i as u16);
                    let face_b = FaceGroupId(j as u16);
                    adjacency_pairs.push((face_a, face_b));

                    // Build a BrepEdge from the shared vertices
                    let edge = build_brep_edge(
                        brep_edges.len(),
                        face_a,
                        face_b,
                        shared_verts,
                        &face_tessellations[i],
                        &face_tessellations[j],
                    );
                    brep_edges.push(edge);
                }
            }
        }
    }

    if face_tessellations.is_empty() {
        return Err(StepImportError::EmptyResult);
    }

    let face_count = face_tessellations.len();
    let tri_count: usize = face_tessellations.iter().map(|f| f.triangles.len()).sum();
    info!(
        faces = face_count,
        triangles = tri_count,
        edges = brep_edges.len(),
        "Building enriched mesh"
    );

    build_enriched_mesh(face_tessellations, adjacency_pairs, brep_edges)
        .map_err(|e| StepImportError::TessellationFailed {
            shell_index: 0,
            message: e,
        })
}

/// Classify a face's surface type heuristically from vertex positions.
fn classify_face_surface(vertices: &[P3]) -> (SurfaceType, SurfaceParams) {
    if vertices.len() < 3 {
        return (SurfaceType::Unknown, SurfaceParams::Unknown);
    }

    let bbox = BoundingBox3::from_points(vertices.iter().copied());
    let x_range = bbox.max.x - bbox.min.x;
    let y_range = bbox.max.y - bbox.min.y;
    let z_range = bbox.max.z - bbox.min.z;
    let max_extent = x_range.max(y_range).max(z_range).max(1e-10);

    // Horizontal plane: Z range is negligible
    if z_range / max_extent < 0.01 {
        let z_avg = vertices.iter().map(|v| v.z).sum::<f64>() / vertices.len() as f64;
        return (
            SurfaceType::Plane,
            SurfaceParams::Plane {
                normal: V3::new(0.0, 0.0, if z_avg >= 0.0 { 1.0 } else { -1.0 }),
                d: z_avg,
            },
        );
    }

    // X-normal plane
    if x_range / max_extent < 0.01 {
        let x_avg = vertices.iter().map(|v| v.x).sum::<f64>() / vertices.len() as f64;
        return (
            SurfaceType::Plane,
            SurfaceParams::Plane {
                normal: V3::new(1.0, 0.0, 0.0),
                d: x_avg,
            },
        );
    }

    // Y-normal plane
    if y_range / max_extent < 0.01 {
        let y_avg = vertices.iter().map(|v| v.y).sum::<f64>() / vertices.len() as f64;
        return (
            SurfaceType::Plane,
            SurfaceParams::Plane {
                normal: V3::new(0.0, 1.0, 0.0),
                d: y_avg,
            },
        );
    }

    (SurfaceType::Unknown, SurfaceParams::Unknown)
}

/// Extract boundary loops from mesh edges (boundary = edges in only one triangle).
fn extract_boundary_loops(vertices: &[P3], triangles: &[[u32; 3]]) -> Vec<Vec<P3>> {
    use std::collections::HashMap;

    let mut edge_count: HashMap<(u32, u32), u32> = HashMap::new();
    for tri in triangles {
        for i in 0..3 {
            let a = tri[i];
            let b = tri[(i + 1) % 3];
            let key = if a < b { (a, b) } else { (b, a) };
            *edge_count.entry(key).or_insert(0) += 1;
        }
    }

    let boundary_edges: Vec<(u32, u32)> = edge_count
        .into_iter()
        .filter(|&(_, count)| count == 1)
        .map(|(edge, _)| edge)
        .collect();

    if boundary_edges.is_empty() {
        return Vec::new();
    }

    // Build adjacency list for boundary vertices
    let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
    for &(a, b) in &boundary_edges {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    }

    let mut visited_edges: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
    let mut loops = Vec::new();

    for &(start_a, _) in &boundary_edges {
        let key = boundary_edges
            .iter()
            .find(|&&(a, b)| {
                a == start_a
                    && !visited_edges.contains(&if a < b { (a, b) } else { (b, a) })
            })
            .map(|&(a, b)| if a < b { (a, b) } else { (b, a) });

        let key = match key {
            Some(k) => k,
            None => continue,
        };

        if visited_edges.contains(&key) {
            continue;
        }

        let mut loop_pts = vec![vertices[start_a as usize]];
        let mut current = start_a;
        let mut prev = u32::MAX;

        while let Some(neighbors) = adj.get(&current) {

            let next = neighbors.iter().find(|&&n| n != prev).copied();
            match next {
                Some(n) => {
                    let edge_key = if current < n { (current, n) } else { (n, current) };
                    if visited_edges.contains(&edge_key) {
                        break;
                    }
                    visited_edges.insert(edge_key);
                    loop_pts.push(vertices[n as usize]);
                    prev = current;
                    current = n;
                    if current == start_a {
                        break;
                    }
                }
                None => break,
            }
        }

        if loop_pts.len() >= 3 {
            loops.push(loop_pts);
        }
    }

    loops
}

/// Find shared vertices between two face tessellations.
/// Returns the shared vertices as P3 points if at least 2 are shared (forming an edge).
fn find_shared_edge_vertices(a: &FaceTessellation, b: &FaceTessellation) -> Option<Vec<P3>> {
    let a_positions: std::collections::HashSet<[i64; 3]> =
        a.vertices.iter().map(quantize_point).collect();

    let shared: Vec<P3> = b
        .vertices
        .iter()
        .filter(|v| a_positions.contains(&quantize_point(v)))
        .copied()
        .collect();

    if shared.len() >= 2 { Some(shared) } else { None }
}

/// Build a BrepEdge from shared vertices between two adjacent faces.
fn build_brep_edge(
    id: usize,
    face_a: FaceGroupId,
    face_b: FaceGroupId,
    shared_verts: Vec<P3>,
    tess_a: &FaceTessellation,
    tess_b: &FaceTessellation,
) -> crate::enriched_mesh::BrepEdge {
    // Compute average face normals for dihedral angle
    let normal_a = face_avg_normal(tess_a);
    let normal_b = face_avg_normal(tess_b);

    let dot = normal_a.dot(&normal_b).clamp(-1.0, 1.0);
    let dihedral_angle = dot.acos();

    // Determine concavity: if the edge midpoint is "inside" relative to both normals,
    // the crease is concave. Use the cross product method.
    let is_concave = if shared_verts.len() >= 2 {
        let edge_mid = P3::new(
            (shared_verts[0].x + shared_verts[shared_verts.len() - 1].x) / 2.0,
            (shared_verts[0].y + shared_verts[shared_verts.len() - 1].y) / 2.0,
            (shared_verts[0].z + shared_verts[shared_verts.len() - 1].z) / 2.0,
        );
        let face_a_center = face_center(tess_a);
        let face_b_center = face_center(tess_b);
        let to_a = face_a_center - edge_mid;
        let to_b = face_b_center - edge_mid;
        // Concave if normals point toward each other (face centers are on the same side)
        normal_a.dot(&to_b) > 0.0 || normal_b.dot(&to_a) > 0.0
    } else {
        false
    };

    // Project to 2D if edge is approximately horizontal
    let vertices_2d = if shared_verts.iter().all(|_| {
        let z_range = shared_verts.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
        let z_max = shared_verts.iter().map(|p| p.z).fold(f64::NEG_INFINITY, f64::max);
        (z_max - z_range) < 0.1 // within 0.1mm Z range
    }) {
        Some(shared_verts.iter().map(|p| P2::new(p.x, p.y)).collect())
    } else {
        None
    };

    crate::enriched_mesh::BrepEdge {
        id,
        face_a,
        face_b,
        vertices: shared_verts,
        vertices_2d,
        is_concave,
        dihedral_angle,
    }
}

/// Compute the average normal of a face tessellation from its surface params.
fn face_avg_normal(tess: &FaceTessellation) -> V3 {
    match &tess.surface_params {
        SurfaceParams::Plane { normal, .. } => *normal,
        _ => {
            // Compute from first triangle if surface params don't give a normal
            if tess.vertices.len() >= 3 && !tess.triangles.is_empty() {
                let tri = &tess.triangles[0];
                let v0 = tess.vertices[tri[0] as usize];
                let v1 = tess.vertices[tri[1] as usize];
                let v2 = tess.vertices[tri[2] as usize];
                let e1 = v1 - v0;
                let e2 = v2 - v0;
                let n = e1.cross(&e2);
                let len = n.norm();
                if len > 1e-15 { n / len } else { V3::new(0.0, 0.0, 1.0) }
            } else {
                V3::new(0.0, 0.0, 1.0)
            }
        }
    }
}

/// Compute the centroid of a face tessellation.
fn face_center(tess: &FaceTessellation) -> P3 {
    if tess.vertices.is_empty() {
        return P3::new(0.0, 0.0, 0.0);
    }
    let n = tess.vertices.len() as f64;
    let sum_x: f64 = tess.vertices.iter().map(|v| v.x).sum();
    let sum_y: f64 = tess.vertices.iter().map(|v| v.y).sum();
    let sum_z: f64 = tess.vertices.iter().map(|v| v.z).sum();
    P3::new(sum_x / n, sum_y / n, sum_z / n)
}

fn quantize_point(p: &P3) -> [i64; 3] {
    [
        (p.x * 1000.0).round() as i64,
        (p.y * 1000.0).round() as i64,
        (p.z * 1000.0).round() as i64,
    ]
}
