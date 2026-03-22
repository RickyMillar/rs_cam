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
