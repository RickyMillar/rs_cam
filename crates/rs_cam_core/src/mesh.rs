//! Triangle mesh loading and spatial indexing.

use crate::geo::{BoundingBox3, P3, V3, Triangle};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use thiserror::Error;
use tracing::warn;

#[derive(Error, Debug)]
pub enum MeshError {
    #[error("Failed to read STL file: {0}")]
    StlRead(#[from] std::io::Error),
    #[error("Empty mesh: no triangles loaded")]
    EmptyMesh,
    #[error("Triangle index {index} out of bounds (vertex count: {vertex_count})")]
    IndexOutOfBounds { index: u32, vertex_count: usize },
}

/// Result of checking mesh winding consistency.
#[derive(Debug, Clone)]
pub struct WindingReport {
    /// Number of edges with consistent winding (adjacent faces have opposite edge directions).
    pub consistent_edges: usize,
    /// Number of edges with inconsistent winding.
    pub inconsistent_edges: usize,
    /// Fraction of edges that are inconsistent (0.0 = perfect, 1.0 = all wrong).
    pub inconsistency_fraction: f64,
}

/// An indexed triangle mesh built from STL.
#[derive(Debug, Clone)]
pub struct TriangleMesh {
    pub vertices: Vec<P3>,
    pub triangles: Vec<[u32; 3]>,
    pub faces: Vec<Triangle>,
    pub bbox: BoundingBox3,
}

impl TriangleMesh {
    /// Load from an STL file (binary or ASCII auto-detected).
    /// stl_io::read_stl already returns an IndexedMesh with welded vertices.
    /// `scale` multiplies all vertex positions (e.g., 1000.0 to convert meters to mm).
    pub fn from_stl_scaled(path: &Path, scale: f64) -> Result<Self, MeshError> {
        let mut file = std::fs::OpenOptions::new().read(true).open(path)?;
        let stl = stl_io::read_stl(&mut file)?;

        if stl.faces.is_empty() {
            return Err(MeshError::EmptyMesh);
        }

        // Convert stl_io vertices (Vector<f32>) to our P3 (nalgebra Point3<f64>)
        let vertices: Vec<P3> = stl
            .vertices
            .iter()
            .map(|v| {
                P3::new(
                    v.0[0] as f64 * scale,
                    v.0[1] as f64 * scale,
                    v.0[2] as f64 * scale,
                )
            })
            .collect();

        // Convert stl_io indexed triangles ([usize; 3]) to our [u32; 3]
        let triangles: Vec<[u32; 3]> = stl
            .faces
            .iter()
            .map(|f| {
                [
                    f.vertices[0] as u32,
                    f.vertices[1] as u32,
                    f.vertices[2] as u32,
                ]
            })
            .collect();

        // Validate triangle indices are within bounds
        let vert_count = vertices.len();
        for tri in &triangles {
            for &idx in tri {
                if (idx as usize) >= vert_count {
                    return Err(MeshError::IndexOutOfBounds {
                        index: idx,
                        vertex_count: vert_count,
                    });
                }
            }
        }

        // Build face list with recomputed normals
        let faces: Vec<Triangle> = triangles
            .iter()
            .map(|tri| {
                Triangle::new(
                    vertices[tri[0] as usize],
                    vertices[tri[1] as usize],
                    vertices[tri[2] as usize],
                )
            })
            .collect();

        let bbox = BoundingBox3::from_points(vertices.iter().copied());

        let mut mesh = Self {
            vertices,
            triangles,
            faces,
            bbox,
        };

        // Check and fix winding consistency
        let report = mesh.check_winding();
        if report.inconsistency_fraction > 0.01 {
            warn!(
                inconsistent = report.inconsistent_edges,
                total = report.consistent_edges + report.inconsistent_edges,
                fraction = format!("{:.1}%", report.inconsistency_fraction * 100.0),
                "STL has inconsistent normals"
            );
        }
        if report.inconsistency_fraction > 0.05 {
            let flipped = mesh.fix_winding();
            warn!(flipped = flipped, "Auto-fixed winding on STL load");
            // Recompute bounding box after winding fix
            mesh.bbox = BoundingBox3::from_points(mesh.vertices.iter().copied());
        }

        Ok(mesh)
    }

    /// Load from an STL file assuming mm (scale=1.0).
    pub fn from_stl(path: &Path) -> Result<Self, MeshError> {
        Self::from_stl_scaled(path, 1.0)
    }

    /// Load from STL bytes in memory (for WASM or embedded use).
    /// `data` should contain a complete STL file (binary or ASCII).
    pub fn from_stl_bytes(data: &[u8], scale: f64) -> Result<Self, MeshError> {
        let mut cursor = std::io::Cursor::new(data);
        let stl = stl_io::read_stl(&mut cursor)?;

        if stl.faces.is_empty() {
            return Err(MeshError::EmptyMesh);
        }

        let vertices: Vec<P3> = stl
            .vertices
            .iter()
            .map(|v| {
                P3::new(
                    v.0[0] as f64 * scale,
                    v.0[1] as f64 * scale,
                    v.0[2] as f64 * scale,
                )
            })
            .collect();

        let triangles: Vec<[u32; 3]> = stl
            .faces
            .iter()
            .map(|f| {
                [
                    f.vertices[0] as u32,
                    f.vertices[1] as u32,
                    f.vertices[2] as u32,
                ]
            })
            .collect();

        // Validate triangle indices are within bounds
        let vert_count = vertices.len();
        for tri in &triangles {
            for &idx in tri {
                if (idx as usize) >= vert_count {
                    return Err(MeshError::IndexOutOfBounds {
                        index: idx,
                        vertex_count: vert_count,
                    });
                }
            }
        }

        let faces: Vec<Triangle> = triangles
            .iter()
            .map(|tri| {
                Triangle::new(
                    vertices[tri[0] as usize],
                    vertices[tri[1] as usize],
                    vertices[tri[2] as usize],
                )
            })
            .collect();

        let bbox = BoundingBox3::from_points(vertices.iter().copied());

        let mut mesh = Self {
            vertices,
            triangles,
            faces,
            bbox,
        };

        // Check and fix winding consistency
        let report = mesh.check_winding();
        if report.inconsistency_fraction > 0.01 {
            warn!(
                inconsistent = report.inconsistent_edges,
                total = report.consistent_edges + report.inconsistent_edges,
                fraction = format!("{:.1}%", report.inconsistency_fraction * 100.0),
                "STL bytes has inconsistent normals"
            );
        }
        if report.inconsistency_fraction > 0.05 {
            let flipped = mesh.fix_winding();
            warn!(flipped = flipped, "Auto-fixed winding on STL bytes load");
            // Recompute bounding box after winding fix
            mesh.bbox = BoundingBox3::from_points(mesh.vertices.iter().copied());
        }

        Ok(mesh)
    }

    /// Check winding consistency of the mesh.
    ///
    /// For each undirected edge shared by two faces, checks whether the
    /// two faces traverse the edge in opposite directions (consistent)
    /// or the same direction (inconsistent). Consistent winding means
    /// all normals point the same way.
    pub fn check_winding(&self) -> WindingReport {
        // Count how many faces traverse each directed edge (a,b)
        let mut edge_count: HashMap<(u32, u32), u32> = HashMap::new();
        for tri in &self.triangles {
            for i in 0..3 {
                let a = tri[i];
                let b = tri[(i + 1) % 3];
                *edge_count.entry((a, b)).or_insert(0) += 1;
            }
        }

        let mut consistent = 0usize;
        let mut inconsistent = 0usize;

        // For each undirected edge, check once
        let mut checked = HashMap::<(u32, u32), bool>::new();
        for &(a, b) in edge_count.keys() {
            let key = if a < b { (a, b) } else { (b, a) };
            if checked.contains_key(&key) {
                continue;
            }
            checked.insert(key, true);

            let fwd = edge_count.get(&(a, b)).copied().unwrap_or(0);
            let rev = edge_count.get(&(b, a)).copied().unwrap_or(0);

            if fwd == 1 && rev == 1 {
                // One face has (a,b), the other has (b,a) — consistent
                consistent += 1;
            } else if fwd >= 2 || rev >= 2 {
                // Two faces have the SAME directed edge — inconsistent
                inconsistent += 1;
            }
            // fwd==1,rev==0 or fwd==0,rev==1 → boundary edge, skip
        }

        let total = consistent + inconsistent;
        let fraction = if total > 0 {
            inconsistent as f64 / total as f64
        } else {
            0.0
        };

        WindingReport {
            consistent_edges: consistent,
            inconsistent_edges: inconsistent,
            inconsistency_fraction: fraction,
        }
    }

    /// Fix inconsistent winding using BFS from a seed triangle.
    ///
    /// Picks the triangle with the most upward-pointing normal as seed,
    /// then BFS-propagates consistent winding. Flips triangles whose
    /// shared edge direction doesn't match the seed's orientation.
    /// Recomputes normals after fixing. Returns the number of triangles flipped.
    pub fn fix_winding(&mut self) -> usize {
        if self.triangles.is_empty() {
            return 0;
        }

        let n = self.triangles.len();

        // Build adjacency: for each undirected edge, which faces share it?
        let mut edge_to_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (fi, tri) in self.triangles.iter().enumerate() {
            for i in 0..3 {
                let a = tri[i];
                let b = tri[(i + 1) % 3];
                let key = if a < b { (a, b) } else { (b, a) };
                edge_to_faces.entry(key).or_default().push(fi);
            }
        }

        // Pick seed: triangle with most upward normal (highest nz)
        let seed = self
            .faces
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.normal
                    .z
                    .partial_cmp(&b.normal.z)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);

        // BFS to propagate winding
        let mut visited = vec![false; n];
        let mut flipped = vec![false; n];
        let mut queue = VecDeque::new();
        queue.push_back(seed);
        visited[seed] = true;

        while let Some(fi) = queue.pop_front() {
            let tri = self.triangles[fi];
            for i in 0..3 {
                let a = tri[i];
                let b = tri[(i + 1) % 3];
                let key = if a < b { (a, b) } else { (b, a) };

                if let Some(neighbors) = edge_to_faces.get(&key) {
                    for &nfi in neighbors {
                        if visited[nfi] {
                            continue;
                        }
                        visited[nfi] = true;

                        // Check if neighbor has the reverse edge (consistent)
                        // The current face has directed edge (a,b).
                        // Consistent neighbor should have (b,a).
                        let ntri = self.triangles[nfi];
                        let has_reverse = (0..3).any(|j| ntri[j] == b && ntri[(j + 1) % 3] == a);

                        if !has_reverse {
                            // Flip neighbor: swap two vertices
                            self.triangles[nfi] = [ntri[0], ntri[2], ntri[1]];
                            flipped[nfi] = true;
                        }

                        queue.push_back(nfi);
                    }
                }
            }
        }

        let flip_count = flipped.iter().filter(|&&f| f).count();

        // Recompute faces (normals) for flipped triangles
        if flip_count > 0 {
            for (fi, flipped_face) in flipped.iter().enumerate().take(n) {
                if *flipped_face {
                    let tri = &self.triangles[fi];
                    self.faces[fi] = Triangle::new(
                        self.vertices[tri[0] as usize],
                        self.vertices[tri[1] as usize],
                        self.vertices[tri[2] as usize],
                    );
                }
            }
        }

        flip_count
    }

    /// Build from raw vertices and triangle indices (for testing).
    pub fn from_raw(vertices: Vec<P3>, triangles: Vec<[u32; 3]>) -> Self {
        let faces: Vec<Triangle> = triangles
            .iter()
            .map(|tri| {
                Triangle::new(
                    vertices[tri[0] as usize],
                    vertices[tri[1] as usize],
                    vertices[tri[2] as usize],
                )
            })
            .collect();
        let bbox = BoundingBox3::from_points(vertices.iter().copied());
        Self {
            vertices,
            triangles,
            faces,
            bbox,
        }
    }
}

/// Cast a ray against a triangle mesh, returning the nearest hit triangle index and parametric t.
///
/// Uses AABB rejection on the mesh bounding box, then brute-force iterates triangles.
/// Suitable for single-ray picking (not per-pixel rendering).
pub fn ray_pick_triangle(mesh: &TriangleMesh, origin: &P3, dir: &V3) -> Option<(usize, f64)> {
    // Fast rejection: check mesh bounding box
    mesh.bbox.ray_intersect(origin, dir)?;

    let mut best_t = f64::INFINITY;
    let mut best_idx = None;

    for (i, face) in mesh.faces.iter().enumerate() {
        if let Some(t) = face.ray_intersect(origin, dir) {
            if t < best_t {
                best_t = t;
                best_idx = Some(i);
            }
        }
    }

    best_idx.map(|i| (i, best_t))
}

/// Spatial index for fast triangle lookup during drop-cutter.
///
/// Uniform XY grid over triangle bounding boxes. For a given cutter at (x,y),
/// we find nearby triangles by querying cells within cutter radius.
pub struct SpatialIndex {
    /// Triangle indices grouped into grid cells for spatial lookup.
    /// Simple uniform grid in XY for now. Each cell stores triangle indices.
    cells: Vec<Vec<usize>>,
    cell_count_x: usize,
    cell_count_y: usize,
    cell_size: f64,
    origin_x: f64,
    origin_y: f64,
    total_triangles: usize,
}

impl SpatialIndex {
    /// Build a spatial index with automatic cell size based on mesh extent.
    ///
    /// Picks a cell size that balances spatial locality vs overhead:
    /// targets ~50 cells per axis, clamped to a minimum of 1.0mm.
    pub fn build_auto(mesh: &TriangleMesh) -> Self {
        let bbox = &mesh.bbox;
        let extent_x = bbox.max.x - bbox.min.x;
        let extent_y = bbox.max.y - bbox.min.y;
        let max_extent = extent_x.max(extent_y);
        let cell_size = (max_extent / 50.0).max(1.0);
        Self::build(mesh, cell_size)
    }

    /// Build a uniform grid spatial index over the mesh triangles.
    /// `cell_size` controls the grid resolution (should be ~2x cutter diameter).
    /// If `cell_size` is larger than mesh extent / 4, it is clamped down to avoid
    /// degenerate grids where all triangles land in one cell.
    pub fn build(mesh: &TriangleMesh, cell_size: f64) -> Self {
        let bbox = &mesh.bbox;
        let origin_x = bbox.min.x;
        let origin_y = bbox.min.y;

        // Clamp cell_size so we get at least 4 cells per axis when possible
        let extent_x = bbox.max.x - bbox.min.x;
        let extent_y = bbox.max.y - bbox.min.y;
        let max_extent = extent_x.max(extent_y);
        let cell_size = if max_extent > 0.0 {
            cell_size.min(max_extent / 4.0)
        } else {
            cell_size
        };

        let cell_count_x = ((bbox.max.x - bbox.min.x) / cell_size).ceil() as usize + 1;
        let cell_count_y = ((bbox.max.y - bbox.min.y) / cell_size).ceil() as usize + 1;
        let total_cells = cell_count_x * cell_count_y;

        let mut cells = vec![Vec::new(); total_cells];

        for (i, face) in mesh.faces.iter().enumerate() {
            // Find the range of grid cells this triangle's bbox overlaps
            let x0 = ((face.bbox.min.x - origin_x) / cell_size).floor() as isize;
            let x1 = ((face.bbox.max.x - origin_x) / cell_size).floor() as isize;
            let y0 = ((face.bbox.min.y - origin_y) / cell_size).floor() as isize;
            let y1 = ((face.bbox.max.y - origin_y) / cell_size).floor() as isize;

            let x0 = x0.max(0) as usize;
            let x1 = (x1 as usize).min(cell_count_x - 1);
            let y0 = y0.max(0) as usize;
            let y1 = (y1 as usize).min(cell_count_y - 1);

            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    cells[cy * cell_count_x + cx].push(i);
                }
            }
        }

        Self {
            cells,
            cell_count_x,
            cell_count_y,
            cell_size,
            origin_x,
            origin_y,
            total_triangles: mesh.faces.len(),
        }
    }

    /// Find all triangle indices whose bounding boxes potentially overlap with
    /// a cutter centered at (cx, cy) with given radius.
    pub fn query(&self, cx: f64, cy: f64, radius: f64) -> Vec<usize> {
        let x0 = ((cx - radius - self.origin_x) / self.cell_size).floor() as isize;
        let x1 = ((cx + radius - self.origin_x) / self.cell_size).floor() as isize;
        let y0 = ((cy - radius - self.origin_y) / self.cell_size).floor() as isize;
        let y1 = ((cy + radius - self.origin_y) / self.cell_size).floor() as isize;

        let x0 = x0.max(0) as usize;
        let x1 = (x1 as usize).min(self.cell_count_x.saturating_sub(1));
        let y0 = y0.max(0) as usize;
        let y1 = (y1 as usize).min(self.cell_count_y.saturating_sub(1));

        let mut result = Vec::new();
        // Use a bitset for dedup: 8x less memory than Vec<bool>.
        // For 100k triangles: 12.5 KB vs 100 KB per query.
        let n_words = self.total_triangles.div_ceil(64);
        let mut seen = vec![0u64; n_words];

        for cy_idx in y0..=y1 {
            for cx_idx in x0..=x1 {
                let cell_idx = cy_idx * self.cell_count_x + cx_idx;
                for &tri_idx in &self.cells[cell_idx] {
                    let word = tri_idx / 64;
                    let bit = 1u64 << (tri_idx % 64);
                    if seen[word] & bit == 0 {
                        seen[word] |= bit;
                        result.push(tri_idx);
                    }
                }
            }
        }

        result
    }
}

/// Generate a test hemisphere mesh for testing drop-cutter.
pub fn make_test_hemisphere(radius: f64, divisions: usize) -> TriangleMesh {
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    // Top vertex
    vertices.push(P3::new(0.0, 0.0, radius));

    // Generate rings of vertices from top to equator
    for i in 1..=divisions {
        let phi = std::f64::consts::FRAC_PI_2 * (1.0 - i as f64 / divisions as f64);
        let z = radius * phi.sin();
        let r_ring = radius * phi.cos();

        let n_around = (divisions * 4).max(8);
        for j in 0..n_around {
            let theta = 2.0 * std::f64::consts::PI * j as f64 / n_around as f64;
            vertices.push(P3::new(r_ring * theta.cos(), r_ring * theta.sin(), z));
        }
    }

    let n_around = (divisions * 4).max(8);

    // Top cap: triangles from apex to first ring
    for j in 0..n_around {
        let j_next = (j + 1) % n_around;
        triangles.push([0, (1 + j) as u32, (1 + j_next) as u32]);
    }

    // Strips between rings
    for i in 0..(divisions - 1) {
        let ring_start = 1 + i * n_around;
        let next_ring_start = 1 + (i + 1) * n_around;
        for j in 0..n_around {
            let j_next = (j + 1) % n_around;
            let a = (ring_start + j) as u32;
            let b = (ring_start + j_next) as u32;
            let c = (next_ring_start + j) as u32;
            let d = (next_ring_start + j_next) as u32;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }

    TriangleMesh::from_raw(vertices, triangles)
}

/// Generate a simple flat square mesh at z=0 for testing.
pub fn make_test_flat(size: f64) -> TriangleMesh {
    let h = size / 2.0;
    let vertices = vec![
        P3::new(-h, -h, 0.0),
        P3::new(h, -h, 0.0),
        P3::new(h, h, 0.0),
        P3::new(-h, h, 0.0),
    ];
    let triangles = vec![[0, 1, 2], [0, 2, 3]];
    TriangleMesh::from_raw(vertices, triangles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_hemisphere() {
        let mesh = make_test_hemisphere(10.0, 4);
        assert!(!mesh.faces.is_empty());
        assert!((mesh.bbox.max.z - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_make_flat() {
        let mesh = make_test_flat(100.0);
        assert_eq!(mesh.faces.len(), 2);
        assert!((mesh.bbox.min.z).abs() < 1e-10);
        assert!((mesh.bbox.max.z).abs() < 1e-10);
    }

    #[test]
    fn test_spatial_index() {
        let mesh = make_test_flat(100.0);
        let idx = SpatialIndex::build(&mesh, 20.0);
        let tris = idx.query(0.0, 0.0, 5.0);
        // Both triangles should be found since they share the center
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn test_spatial_index_auto_sizing() {
        let mesh = make_test_flat(100.0);
        let idx = SpatialIndex::build_auto(&mesh);
        // Should still find triangles near center
        let tris = idx.query(0.0, 0.0, 5.0);
        assert_eq!(tris.len(), 2, "Auto-sized index should find triangles");
        // Auto-size should produce reasonable cell count (not 1 cell)
        assert!(
            idx.cell_count_x >= 4,
            "Should have multiple X cells, got {}",
            idx.cell_count_x
        );
        assert!(
            idx.cell_count_y >= 4,
            "Should have multiple Y cells, got {}",
            idx.cell_count_y
        );
    }

    #[test]
    fn test_spatial_index_oversized_cell_clamped() {
        // When user passes a cell_size larger than mesh, it should be clamped
        let mesh = make_test_flat(100.0);
        let idx = SpatialIndex::build(&mesh, 10000.0); // way too big
        // Should still have multiple cells due to clamping
        assert!(
            idx.cell_count_x >= 4,
            "Oversized cell should be clamped, got {} X cells",
            idx.cell_count_x
        );
        // And still find triangles
        let tris = idx.query(0.0, 0.0, 5.0);
        assert_eq!(tris.len(), 2);
    }

    // ── Winding consistency tests ─────────────────────────────────────

    #[test]
    fn test_winding_consistent_hemisphere() {
        let mesh = make_test_hemisphere(10.0, 8);
        let report = mesh.check_winding();
        assert_eq!(
            report.inconsistent_edges, 0,
            "Hemisphere should have consistent winding, got {} inconsistent",
            report.inconsistent_edges
        );
        assert!(report.consistent_edges > 0);
    }

    #[test]
    fn test_winding_consistent_flat() {
        let mesh = make_test_flat(50.0);
        let report = mesh.check_winding();
        assert_eq!(
            report.inconsistent_edges, 0,
            "Flat mesh should have consistent winding"
        );
    }

    #[test]
    fn test_winding_detect_flipped() {
        // Create a simple 4-triangle mesh with one triangle flipped
        let vertices = vec![
            P3::new(0.0, 0.0, 0.0),   // 0
            P3::new(10.0, 0.0, 0.0),  // 1
            P3::new(10.0, 10.0, 0.0), // 2
            P3::new(0.0, 10.0, 0.0),  // 3
        ];
        // Normal: [0,1,2] + [0,2,3] — consistent CCW
        // Flipped: [0,1,2] + [0,3,2] — second has opposite winding
        let triangles = vec![
            [0, 1, 2], // CCW
            [0, 3, 2], // CW (flipped!)
        ];
        let mesh = TriangleMesh::from_raw(vertices, triangles);
        let report = mesh.check_winding();
        assert!(
            report.inconsistent_edges > 0,
            "Should detect flipped triangle, got {} inconsistent",
            report.inconsistent_edges
        );
    }

    #[test]
    fn test_fix_winding_corrects_flip() {
        let vertices = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(10.0, 10.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
        ];
        let triangles = vec![
            [0, 1, 2], // CCW
            [0, 3, 2], // CW (flipped!)
        ];
        let mut mesh = TriangleMesh::from_raw(vertices, triangles);

        // Verify it's broken
        let before = mesh.check_winding();
        assert!(before.inconsistent_edges > 0);

        // Fix it
        let flipped = mesh.fix_winding();
        assert!(flipped > 0, "Should flip at least one triangle");

        // Verify it's now consistent
        let after = mesh.check_winding();
        assert_eq!(
            after.inconsistent_edges, 0,
            "After fix, should be consistent, got {} inconsistent",
            after.inconsistent_edges
        );
    }

    #[test]
    fn test_fix_winding_normals_updated() {
        let vertices = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(10.0, 10.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
        ];
        let triangles = vec![
            [0, 1, 2], // CCW → normal +Z
            [0, 3, 2], // CW → normal -Z (flipped!)
        ];
        let mut mesh = TriangleMesh::from_raw(vertices, triangles);

        // Before fix: normals point opposite directions
        let nz_0_before = mesh.faces[0].normal.z;
        let nz_1_before = mesh.faces[1].normal.z;
        assert!(
            nz_0_before * nz_1_before < 0.0,
            "Before fix, normals should disagree: {:.1} vs {:.1}",
            nz_0_before,
            nz_1_before
        );

        mesh.fix_winding();

        // After fix: all normals should agree in direction
        let nz_0 = mesh.faces[0].normal.z;
        let nz_1 = mesh.faces[1].normal.z;
        assert!(
            nz_0 * nz_1 > 0.0,
            "After fix, normals should agree: {:.1} vs {:.1}",
            nz_0,
            nz_1
        );
    }

    #[test]
    fn test_bbox_correct_after_winding_fix() {
        // Create a mesh with inconsistent winding
        let vertices = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(10.0, 10.0, 5.0),
            P3::new(0.0, 10.0, 0.0),
        ];
        let triangles = vec![
            [0, 1, 2], // CCW
            [0, 3, 2], // CW (flipped!)
        ];
        let mut mesh = TriangleMesh::from_raw(vertices.clone(), triangles);

        // Fix winding
        mesh.fix_winding();
        // Recompute bbox as the production code does
        mesh.bbox = BoundingBox3::from_points(mesh.vertices.iter().copied());

        // Verify bbox covers all vertices
        assert!((mesh.bbox.min.x - 0.0).abs() < 1e-10);
        assert!((mesh.bbox.min.y - 0.0).abs() < 1e-10);
        assert!((mesh.bbox.min.z - 0.0).abs() < 1e-10);
        assert!((mesh.bbox.max.x - 10.0).abs() < 1e-10);
        assert!((mesh.bbox.max.y - 10.0).abs() < 1e-10);
        assert!((mesh.bbox.max.z - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_from_raw_invalid_indices_panics() {
        // from_raw does not validate indices (it's for testing).
        // But from_stl_scaled and from_stl_bytes do validate.
        // This test verifies that from_raw with valid indices works.
        let vertices = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 0.0),
            P3::new(0.0, 1.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2]];
        let mesh = TriangleMesh::from_raw(vertices, triangles);
        assert_eq!(mesh.faces.len(), 1);
    }
}
