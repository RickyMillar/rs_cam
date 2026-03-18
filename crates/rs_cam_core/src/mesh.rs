//! Triangle mesh loading and spatial indexing.

use crate::geo::{BoundingBox3, P3, Triangle};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MeshError {
    #[error("Failed to read STL file: {0}")]
    StlRead(#[from] std::io::Error),
    #[error("Empty mesh: no triangles loaded")]
    EmptyMesh,
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
    pub fn from_stl(path: &Path) -> Result<Self, MeshError> {
        let mut file = std::fs::OpenOptions::new().read(true).open(path)?;
        let stl = stl_io::read_stl(&mut file)?;

        if stl.faces.is_empty() {
            return Err(MeshError::EmptyMesh);
        }

        // Convert stl_io vertices (Vector<f32>) to our P3 (nalgebra Point3<f64>)
        let vertices: Vec<P3> = stl
            .vertices
            .iter()
            .map(|v| P3::new(v.0[0] as f64, v.0[1] as f64, v.0[2] as f64))
            .collect();

        // Convert stl_io indexed triangles ([usize; 3]) to our [u32; 3]
        let triangles: Vec<[u32; 3]> = stl
            .faces
            .iter()
            .map(|f| [f.vertices[0] as u32, f.vertices[1] as u32, f.vertices[2] as u32])
            .collect();

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

        Ok(Self {
            vertices,
            triangles,
            faces,
            bbox,
        })
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

/// Spatial index for fast triangle lookup during drop-cutter.
///
/// Uses kiddo KD-tree over triangle centroids. For a given cutter at (x,y),
/// we find nearby triangles by querying centroids within cutter radius + max triangle extent.
pub struct SpatialIndex {
    /// Triangle indices grouped into grid cells for spatial lookup.
    /// Simple uniform grid in XY for now. Each cell stores triangle indices.
    cells: Vec<Vec<usize>>,
    cell_count_x: usize,
    cell_count_y: usize,
    cell_size: f64,
    origin_x: f64,
    origin_y: f64,
}

impl SpatialIndex {
    /// Build a uniform grid spatial index over the mesh triangles.
    /// `cell_size` controls the grid resolution (should be ~2x cutter diameter).
    pub fn build(mesh: &TriangleMesh, cell_size: f64) -> Self {
        let bbox = &mesh.bbox;
        let origin_x = bbox.min.x;
        let origin_y = bbox.min.y;

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
        let mut seen = Vec::new(); // could use a bitset for large meshes

        for cy_idx in y0..=y1 {
            for cx_idx in x0..=x1 {
                let cell_idx = cy_idx * self.cell_count_x + cx_idx;
                for &tri_idx in &self.cells[cell_idx] {
                    if !seen.contains(&tri_idx) {
                        seen.push(tri_idx);
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
}
