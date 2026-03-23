//! Enriched mesh: a triangle mesh with BREP face group metadata.
//!
//! When a STEP file is imported, each BREP face is tessellated independently
//! and the results are merged into a single `TriangleMesh`. The `EnrichedMesh`
//! preserves the mapping from triangles back to their source faces, enabling
//! face-level picking, highlighting, and boundary extraction for CAM operations.

use std::ops::Range;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::geo::{BoundingBox3, P2, P3, V3};
use crate::mesh::TriangleMesh;
use crate::polygon::Polygon2;

/// Identifier for a face group within an `EnrichedMesh`.
///
/// Assigned by topological order during STEP import (deterministic for a given file).
/// Supports up to 65535 faces per model, which is more than sufficient for CAM parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FaceGroupId(pub u16);

/// Classification of the underlying surface geometry for a BREP face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceType {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    BSpline,
    Unknown,
}

/// Parametric description of a surface, for known analytic types.
#[derive(Debug, Clone)]
pub enum SurfaceParams {
    Plane {
        normal: V3,
        d: f64,
    },
    Cylinder {
        axis_origin: P3,
        axis_dir: V3,
        radius: f64,
    },
    Cone {
        apex: P3,
        axis: V3,
        half_angle: f64,
    },
    Sphere {
        center: P3,
        radius: f64,
    },
    Torus {
        center: P3,
        axis: V3,
        major_radius: f64,
        minor_radius: f64,
    },
    BSpline,
    Unknown,
}

/// A group of mesh triangles corresponding to a single BREP face.
#[derive(Debug, Clone)]
pub struct FaceGroup {
    pub id: FaceGroupId,
    pub surface_type: SurfaceType,
    pub surface_params: SurfaceParams,
    /// Range of indices into `EnrichedMesh.mesh.triangles`. Always contiguous.
    pub triangle_range: Range<usize>,
    /// Axis-aligned bounding box of this face group's triangles.
    pub bbox: BoundingBox3,
    /// 3D polyline boundary loops. First is the outer loop, rest are holes.
    pub boundary_loops: Vec<Vec<P3>>,
    /// 2D projected boundary loops (only for approximately-horizontal planar faces).
    pub boundary_loops_2d: Option<Vec<Vec<P2>>>,
}

/// A BREP edge between two adjacent faces, with geometry and classification.
#[derive(Debug, Clone)]
pub struct BrepEdge {
    /// Index of this edge (for reference).
    pub id: usize,
    /// First adjacent face.
    pub face_a: FaceGroupId,
    /// Second adjacent face.
    pub face_b: FaceGroupId,
    /// 3D polyline vertices along this edge (tessellated from BREP curve).
    pub vertices: Vec<P3>,
    /// 2D projected vertices (only for approximately-horizontal edges).
    pub vertices_2d: Option<Vec<P2>>,
    /// True if the two faces form a concave crease at this edge.
    pub is_concave: bool,
    /// Dihedral angle between face normals at this edge (radians).
    /// 0 = faces are coplanar, PI = faces point in opposite directions.
    pub dihedral_angle: f64,
}

/// A triangle mesh enriched with BREP face group metadata.
///
/// The inner `TriangleMesh` can be extracted for use with existing operations
/// that don't need face-level information. The face groups enable face picking,
/// highlighting, and boundary extraction for CAM operations.
#[derive(Debug, Clone)]
pub struct EnrichedMesh {
    /// The tessellated geometry. Triangles are ordered by face group (contiguous).
    pub mesh: Arc<TriangleMesh>,
    /// One entry per BREP face, in topological order.
    pub face_groups: Vec<FaceGroup>,
    /// Maps triangle index → face group index. Length == mesh.triangles.len().
    pub triangle_to_face: Vec<u16>,
    /// Pairs of face groups that share at least one edge.
    pub adjacency: Vec<(FaceGroupId, FaceGroupId)>,
    /// BREP edges between adjacent faces with geometry and classification.
    pub edges: Vec<BrepEdge>,
}

impl EnrichedMesh {
    /// Get a reference to the inner triangle mesh.
    pub fn as_mesh(&self) -> &TriangleMesh {
        &self.mesh
    }

    /// Get a shared reference to the inner mesh (for storing in `LoadedModel.mesh`).
    pub fn mesh_arc(&self) -> Arc<TriangleMesh> {
        Arc::clone(&self.mesh)
    }

    /// Look up the face group for a given triangle index.
    ///
    /// # Panics
    /// Panics if `tri_idx` is out of bounds.
    pub fn face_for_triangle(&self, tri_idx: usize) -> FaceGroupId {
        FaceGroupId(self.triangle_to_face[tri_idx])
    }

    /// Get a face group by ID. Returns `None` if the ID is out of range.
    pub fn face_group(&self, id: FaceGroupId) -> Option<&FaceGroup> {
        self.face_groups.get(id.0 as usize)
    }

    /// Number of face groups.
    pub fn face_count(&self) -> usize {
        self.face_groups.len()
    }

    /// Get all concave edges with dihedral angle below a threshold (in radians).
    ///
    /// Used by pencil-like operations to find crease edges to trace.
    pub fn concave_edges(&self, max_dihedral: f64) -> Vec<&BrepEdge> {
        self.edges
            .iter()
            .filter(|e| e.is_concave && e.dihedral_angle < max_dihedral)
            .collect()
    }

    /// Get all edges adjacent to a specific face.
    pub fn edges_for_face(&self, face_id: FaceGroupId) -> Vec<&BrepEdge> {
        self.edges
            .iter()
            .filter(|e| e.face_a == face_id || e.face_b == face_id)
            .collect()
    }

    /// Get edges shared between two specific faces.
    pub fn edges_between(&self, a: FaceGroupId, b: FaceGroupId) -> Vec<&BrepEdge> {
        self.edges
            .iter()
            .filter(|e| (e.face_a == a && e.face_b == b) || (e.face_a == b && e.face_b == a))
            .collect()
    }

    /// Get all edges as 2D polylines (for trace/engrave operations).
    /// Only returns edges that have 2D projections available.
    pub fn edge_chains_2d(&self) -> Vec<Vec<P2>> {
        self.edges
            .iter()
            .filter_map(|e| e.vertices_2d.clone())
            .filter(|pts| pts.len() >= 2)
            .collect()
    }

    /// Project a single planar face's boundary loops to a 2D `Polygon2`.
    ///
    /// Only works for approximately-horizontal planar faces (|normal.z| > 0.95).
    /// Returns `None` if the face is non-planar or not horizontal.
    pub fn face_boundary_as_polygon(&self, id: FaceGroupId) -> Option<Polygon2> {
        let group = self.face_group(id)?;

        // Must be a planar face with pre-computed 2D loops
        let loops_2d = group.boundary_loops_2d.as_ref()?;
        if loops_2d.is_empty() {
            return None;
        }

        // First loop is the exterior, rest are holes
        let exterior = loops_2d[0].clone();
        if exterior.len() < 3 {
            return None;
        }

        let holes: Vec<Vec<P2>> = loops_2d[1..]
            .iter()
            .filter(|h| h.len() >= 3)
            .cloned()
            .collect();

        Some(Polygon2 { exterior, holes })
    }

    /// Project multiple coplanar face boundary loops into a union `Polygon2`.
    ///
    /// All faces must be approximately-horizontal planes. Returns `None` if
    /// any face is non-planar or not horizontal, or if faces are empty.
    pub fn faces_boundary_as_polygon(&self, ids: &[FaceGroupId]) -> Option<Polygon2> {
        if ids.is_empty() {
            return None;
        }
        if ids.len() == 1 {
            return self.face_boundary_as_polygon(ids[0]);
        }

        // Collect all individual polygons
        let mut polygons: Vec<Polygon2> = Vec::with_capacity(ids.len());
        for &id in ids {
            polygons.push(self.face_boundary_as_polygon(id)?);
        }

        // Boolean union of multiple face polygons using the geo crate.
        use geo::BooleanOps;

        let mut result_geo = polygons[0].to_geo_polygon();
        for poly in &polygons[1..] {
            let other = poly.to_geo_polygon();
            let union = result_geo.union(&other);
            // BooleanOps returns a MultiPolygon; take the largest polygon
            if let Some(largest) = union.iter().max_by(|a, b| {
                geo::Area::unsigned_area(*a)
                    .partial_cmp(&geo::Area::unsigned_area(*b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                result_geo = largest.clone();
            }
        }

        Some(Polygon2::from_geo_polygon(&result_geo))
    }
}

/// Build an `EnrichedMesh` from per-face tessellation results.
///
/// This is the constructor used by the STEP import pipeline. Each face's
/// tessellation (vertices + triangles) is provided separately, and this
/// function merges them into a single mesh with contiguous face groups.
pub fn build_enriched_mesh(
    face_data: Vec<FaceTessellation>,
    adjacency: Vec<(FaceGroupId, FaceGroupId)>,
    edges: Vec<BrepEdge>,
) -> Result<EnrichedMesh, String> {
    if face_data.is_empty() {
        return Err("No faces to build enriched mesh from".to_string());
    }

    let mut all_vertices: Vec<P3> = Vec::new();
    let mut all_triangles: Vec<[u32; 3]> = Vec::new();
    let mut triangle_to_face: Vec<u16> = Vec::new();
    let mut face_groups: Vec<FaceGroup> = Vec::new();

    if face_data.len() > u16::MAX as usize + 1 {
        return Err(format!(
            "Too many faces ({}) — maximum supported is {}",
            face_data.len(),
            u16::MAX as usize + 1
        ));
    }

    for (face_idx, face) in face_data.into_iter().enumerate() {
        let vertex_offset = all_vertices.len() as u32;
        let tri_start = all_triangles.len();

        // Add vertices
        all_vertices.extend_from_slice(&face.vertices);

        // Add triangles with offset vertex indices
        for tri in &face.triangles {
            all_triangles.push([
                tri[0] + vertex_offset,
                tri[1] + vertex_offset,
                tri[2] + vertex_offset,
            ]);
            triangle_to_face.push(face_idx as u16);
        }

        let tri_end = all_triangles.len();

        // Compute bbox for this face group
        let bbox = BoundingBox3::from_points(face.vertices.iter().copied());

        // Compute 2D boundary loops for planar faces
        let boundary_loops_2d = compute_2d_boundary(
            &face.surface_type,
            &face.surface_params,
            &face.boundary_loops,
        );

        face_groups.push(FaceGroup {
            id: FaceGroupId(face_idx as u16),
            surface_type: face.surface_type,
            surface_params: face.surface_params,
            triangle_range: tri_start..tri_end,
            bbox,
            boundary_loops: face.boundary_loops,
            boundary_loops_2d,
        });
    }

    let mesh = TriangleMesh::from_raw(all_vertices, all_triangles);

    Ok(EnrichedMesh {
        mesh: Arc::new(mesh),
        face_groups,
        triangle_to_face,
        adjacency,
        edges,
    })
}

/// Input data for a single face's tessellation, provided by the STEP import pipeline.
pub struct FaceTessellation {
    pub vertices: Vec<P3>,
    pub triangles: Vec<[u32; 3]>,
    pub surface_type: SurfaceType,
    pub surface_params: SurfaceParams,
    pub boundary_loops: Vec<Vec<P3>>,
}

/// Compute 2D projected boundary loops for approximately-horizontal planar faces.
fn compute_2d_boundary(
    surface_type: &SurfaceType,
    surface_params: &SurfaceParams,
    boundary_loops: &[Vec<P3>],
) -> Option<Vec<Vec<P2>>> {
    // Only for planar faces
    if *surface_type != SurfaceType::Plane {
        return None;
    }

    // Check that the plane is approximately horizontal
    if let SurfaceParams::Plane { normal, .. } = surface_params {
        if normal.z.abs() < 0.95 {
            return None; // Not horizontal enough
        }
    } else {
        return None;
    }

    // Project 3D loops to 2D by dropping Z
    let loops_2d: Vec<Vec<P2>> = boundary_loops
        .iter()
        .map(|loop_3d| loop_3d.iter().map(|p| P2::new(p.x, p.y)).collect())
        .collect();

    if loops_2d.is_empty() || loops_2d[0].len() < 3 {
        return None;
    }

    Some(loops_2d)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Build a simple box EnrichedMesh with 6 planar faces for testing.
    fn make_test_box() -> EnrichedMesh {
        // Box: 100x50x25 mm at origin
        let (sx, sy, sz) = (100.0, 50.0, 25.0);

        // 8 corners
        let v000 = P3::new(0.0, 0.0, 0.0);
        let v100 = P3::new(sx, 0.0, 0.0);
        let v110 = P3::new(sx, sy, 0.0);
        let v010 = P3::new(0.0, sy, 0.0);
        let v001 = P3::new(0.0, 0.0, sz);
        let v101 = P3::new(sx, 0.0, sz);
        let v111 = P3::new(sx, sy, sz);
        let v011 = P3::new(0.0, sy, sz);

        let faces = vec![
            // Bottom face (Z=0, normal -Z)
            FaceTessellation {
                vertices: vec![v000, v100, v110, v010],
                triangles: vec![[0, 2, 1], [0, 3, 2]],
                surface_type: SurfaceType::Plane,
                surface_params: SurfaceParams::Plane {
                    normal: V3::new(0.0, 0.0, -1.0),
                    d: 0.0,
                },
                boundary_loops: vec![vec![v000, v100, v110, v010]],
            },
            // Top face (Z=25, normal +Z)
            FaceTessellation {
                vertices: vec![v001, v101, v111, v011],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
                surface_type: SurfaceType::Plane,
                surface_params: SurfaceParams::Plane {
                    normal: V3::new(0.0, 0.0, 1.0),
                    d: sz,
                },
                boundary_loops: vec![vec![v001, v101, v111, v011]],
            },
            // Front face (Y=0, normal -Y)
            FaceTessellation {
                vertices: vec![v000, v100, v101, v001],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
                surface_type: SurfaceType::Plane,
                surface_params: SurfaceParams::Plane {
                    normal: V3::new(0.0, -1.0, 0.0),
                    d: 0.0,
                },
                boundary_loops: vec![vec![v000, v100, v101, v001]],
            },
            // Back face (Y=50, normal +Y)
            FaceTessellation {
                vertices: vec![v010, v110, v111, v011],
                triangles: vec![[0, 2, 1], [0, 3, 2]],
                surface_type: SurfaceType::Plane,
                surface_params: SurfaceParams::Plane {
                    normal: V3::new(0.0, 1.0, 0.0),
                    d: sy,
                },
                boundary_loops: vec![vec![v010, v110, v111, v011]],
            },
            // Right face (X=100, normal +X)
            FaceTessellation {
                vertices: vec![v100, v110, v111, v101],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
                surface_type: SurfaceType::Plane,
                surface_params: SurfaceParams::Plane {
                    normal: V3::new(1.0, 0.0, 0.0),
                    d: sx,
                },
                boundary_loops: vec![vec![v100, v110, v111, v101]],
            },
            // Left face (X=0, normal -X)
            FaceTessellation {
                vertices: vec![v000, v010, v011, v001],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
                surface_type: SurfaceType::Plane,
                surface_params: SurfaceParams::Plane {
                    normal: V3::new(-1.0, 0.0, 0.0),
                    d: 0.0,
                },
                boundary_loops: vec![vec![v000, v010, v011, v001]],
            },
        ];

        let adjacency = vec![
            (FaceGroupId(0), FaceGroupId(2)), // bottom-front
            (FaceGroupId(0), FaceGroupId(3)), // bottom-back
            (FaceGroupId(0), FaceGroupId(4)), // bottom-right
            (FaceGroupId(0), FaceGroupId(5)), // bottom-left
            (FaceGroupId(1), FaceGroupId(2)), // top-front
            (FaceGroupId(1), FaceGroupId(3)), // top-back
            (FaceGroupId(1), FaceGroupId(4)), // top-right
            (FaceGroupId(1), FaceGroupId(5)), // top-left
            (FaceGroupId(2), FaceGroupId(4)), // front-right
            (FaceGroupId(2), FaceGroupId(5)), // front-left
            (FaceGroupId(3), FaceGroupId(4)), // back-right
            (FaceGroupId(3), FaceGroupId(5)), // back-left
        ];

        build_enriched_mesh(faces, adjacency, Vec::new()).expect("Failed to build test box")
    }

    #[test]
    fn test_enriched_mesh_face_count() {
        let em = make_test_box();
        assert_eq!(em.face_count(), 6);
    }

    #[test]
    fn test_enriched_mesh_triangle_count() {
        let em = make_test_box();
        // 6 faces × 2 triangles each = 12
        assert_eq!(em.as_mesh().triangles.len(), 12);
    }

    #[test]
    fn test_triangle_to_face_lookup() {
        let em = make_test_box();
        assert_eq!(em.triangle_to_face.len(), 12);

        // First 2 triangles → face 0 (bottom)
        assert_eq!(em.face_for_triangle(0), FaceGroupId(0));
        assert_eq!(em.face_for_triangle(1), FaceGroupId(0));

        // Next 2 → face 1 (top)
        assert_eq!(em.face_for_triangle(2), FaceGroupId(1));
        assert_eq!(em.face_for_triangle(3), FaceGroupId(1));

        // Last 2 → face 5 (left)
        assert_eq!(em.face_for_triangle(10), FaceGroupId(5));
        assert_eq!(em.face_for_triangle(11), FaceGroupId(5));
    }

    #[test]
    fn test_face_group_retrieval() {
        let em = make_test_box();
        let top = em
            .face_group(FaceGroupId(1))
            .expect("face group 1 should exist");
        assert_eq!(top.surface_type, SurfaceType::Plane);
        assert_eq!(top.triangle_range, 2..4);

        // Out of range returns None
        assert!(em.face_group(FaceGroupId(99)).is_none());
    }

    #[test]
    fn test_as_mesh_returns_correct_data() {
        let em = make_test_box();
        let mesh = em.as_mesh();
        assert_eq!(mesh.triangles.len(), 12);
        // Box has 8 corners but each face has its own 4 vertices → 6×4 = 24
        assert_eq!(mesh.vertices.len(), 24);
        // Bbox should cover the full box
        assert!((mesh.bbox.min.x - 0.0).abs() < 1e-10);
        assert!((mesh.bbox.max.x - 100.0).abs() < 1e-10);
        assert!((mesh.bbox.max.z - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_mesh_arc_shares_allocation() {
        let em = make_test_box();
        let arc1 = em.mesh_arc();
        let arc2 = em.mesh_arc();
        assert!(Arc::ptr_eq(&arc1, &arc2));
    }

    #[test]
    fn test_horizontal_face_boundary_as_polygon() {
        let em = make_test_box();

        // Top face (horizontal, normal +Z) should produce a Polygon2
        let poly = em.face_boundary_as_polygon(FaceGroupId(1));
        assert!(poly.is_some(), "Horizontal face should produce polygon");
        let poly = poly.expect("asserted Some above");
        assert_eq!(poly.exterior.len(), 4);
        assert!(poly.holes.is_empty());
    }

    #[test]
    fn test_vertical_face_no_polygon() {
        let em = make_test_box();

        // Front face (vertical, normal -Y) should NOT produce a polygon
        let poly = em.face_boundary_as_polygon(FaceGroupId(2));
        assert!(poly.is_none(), "Vertical face should not produce polygon");
    }

    #[test]
    fn test_bottom_face_boundary_as_polygon() {
        let em = make_test_box();

        // Bottom face (horizontal, normal -Z) should produce a polygon
        let poly = em.face_boundary_as_polygon(FaceGroupId(0));
        assert!(poly.is_some(), "Bottom face should produce polygon");
    }

    #[test]
    fn test_faces_boundary_empty_returns_none() {
        let em = make_test_box();
        assert!(em.faces_boundary_as_polygon(&[]).is_none());
    }

    #[test]
    fn test_faces_boundary_single_face() {
        let em = make_test_box();
        let poly = em.faces_boundary_as_polygon(&[FaceGroupId(1)]);
        assert!(poly.is_some());
    }

    #[test]
    fn test_faces_boundary_mixed_returns_none() {
        let em = make_test_box();
        // Top (horizontal) + Front (vertical) → None because front is not horizontal
        let poly = em.faces_boundary_as_polygon(&[FaceGroupId(1), FaceGroupId(2)]);
        assert!(
            poly.is_none(),
            "Mixed horizontal/vertical should return None"
        );
    }

    #[test]
    fn test_adjacency_stored() {
        let em = make_test_box();
        assert_eq!(em.adjacency.len(), 12);
        assert!(em.adjacency.contains(&(FaceGroupId(0), FaceGroupId(2))));
    }

    #[test]
    fn test_face_group_bbox() {
        let em = make_test_box();
        let top = em
            .face_group(FaceGroupId(1))
            .expect("face group 1 should exist");
        // Top face at z=25, spanning 0..100 x 0..50
        assert!((top.bbox.min.z - 25.0).abs() < 1e-10);
        assert!((top.bbox.max.z - 25.0).abs() < 1e-10);
        assert!((top.bbox.min.x - 0.0).abs() < 1e-10);
        assert!((top.bbox.max.x - 100.0).abs() < 1e-10);
    }
}
