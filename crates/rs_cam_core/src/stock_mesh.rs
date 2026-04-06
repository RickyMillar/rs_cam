//! Triangle mesh data exported from stock simulation, suitable for 3D rendering.

/// Triangle mesh data exported from stock simulation, suitable for 3D rendering.
#[derive(Clone)]
pub struct StockMesh {
    /// Vertex positions as flat [x, y, z, ...] in f32.
    pub vertices: Vec<f32>,
    /// Triangle indices as flat [i0, i1, ...].
    pub indices: Vec<u32>,
    /// Vertex colors as flat [r, g, b, ...] in f32.
    pub colors: Vec<f32>,
}

impl StockMesh {
    /// Create an empty mesh.
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        }
    }

    /// Number of vertices in this mesh.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / 3
    }

    /// Transform all vertex positions using a point transform function,
    /// then append the result to `self`.
    pub fn append_transformed<F>(&mut self, other: &StockMesh, transform: F)
    where
        F: Fn(f32, f32, f32) -> (f32, f32, f32),
    {
        let base_vertex = self.vertex_count() as u32;

        // Transform and append vertices
        let mut i = 0;
        while i + 2 < other.vertices.len() {
            // SAFETY: loop guard ensures i+2 is in bounds
            #[allow(clippy::indexing_slicing)]
            let (x, y, z) = (
                other.vertices[i],
                other.vertices[i + 1],
                other.vertices[i + 2],
            );
            let (tx, ty, tz) = transform(x, y, z);
            self.vertices.push(tx);
            self.vertices.push(ty);
            self.vertices.push(tz);
            i += 3;
        }

        // Offset and append indices
        for idx in &other.indices {
            self.indices.push(idx + base_vertex);
        }

        // Append colors unchanged
        self.colors.extend_from_slice(&other.colors);
    }
}
