//! Triangle mesh data exported from stock simulation, suitable for 3D rendering.

use crate::arc_util::linearize_arc;
use crate::toolpath::{MoveType, Toolpath};

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

    /// Replace vertex colors with a height-gradient colormap (Blue→Green→Red)
    /// based on vertex Z positions. Much more readable than wood-tone for
    /// understanding depth variations.
    pub fn apply_height_gradient(&mut self) {
        let colors = height_gradient_colors(&self.vertices);
        self.colors.clear();
        self.colors.reserve(colors.len() * 3);
        for [r, g, b] in colors {
            self.colors.push(r);
            self.colors.push(g);
            self.colors.push(b);
        }
    }

    /// Append another mesh (identity transform).
    pub fn append(&mut self, other: &Self) {
        self.append_transformed(other, |x, y, z| (x, y, z));
    }

    /// Create a copy with all vertex colors scaled by `factor` (for dimming backgrounds).
    pub fn with_dimmed_colors(&self, factor: f32) -> Self {
        let colors = self.colors.iter().map(|c| c * factor).collect();
        Self {
            vertices: self.vertices.clone(),
            indices: self.indices.clone(),
            colors,
        }
    }
}

// ── Height gradient coloring ─────────────────────────────────────────

/// Generate per-vertex height-gradient colors from flat vertex positions.
///
/// Maps Z values: Blue (low) → Green (mid) → Red (high).
/// Input: flat `[x, y, z, ...]` vertex array. Returns one `[r, g, b]` per vertex.
#[allow(clippy::indexing_slicing)] // stride-3 loop bounded by num_verts
pub fn height_gradient_colors(vertices: &[f32]) -> Vec<[f32; 3]> {
    let num_verts = vertices.len() / 3;
    if num_verts == 0 {
        return Vec::new();
    }

    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;
    for i in 0..num_verts {
        let z = vertices[i * 3 + 2];
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }
    let range = (max_z - min_z).max(0.001);

    (0..num_verts)
        .map(|i| {
            let z = vertices[i * 3 + 2];
            let t = ((z - min_z) / range).clamp(0.0, 1.0);
            if t < 0.5 {
                let s = t * 2.0;
                [0.0, s, 1.0 - s]
            } else {
                let s = (t - 0.5) * 2.0;
                [s, 1.0 - s, 0.0]
            }
        })
        .collect()
}

// ── Toolpath → tube mesh conversion ──────────────────────────────────

/// Cutting move color: bright green.
const CUT_COLOR: [f32; 3] = [0.1, 0.85, 0.2];
/// Rapid move color: orange-red.
const RAPID_COLOR: [f32; 3] = [0.9, 0.3, 0.1];

/// Convert a toolpath into a tube mesh for software rasterization.
///
/// Each line segment becomes a thin rectangular prism visible from any angle.
/// Cutting moves are green, rapids are orange-red.
/// `ribbon_radius` controls half-width of each tube in model units (mm).
pub fn toolpath_to_tube_mesh(
    toolpath: &Toolpath,
    ribbon_radius: f32,
    include_rapids: bool,
) -> StockMesh {
    let mut mesh = StockMesh::empty();
    if toolpath.moves.len() < 2 {
        return mesh;
    }

    // Pre-allocate (12 tris × 3 indices = 36 per segment, 8 verts per segment)
    let est_segs = toolpath.moves.len();
    mesh.vertices.reserve(est_segs * 8 * 3);
    mesh.indices.reserve(est_segs * 36);
    mesh.colors.reserve(est_segs * 8 * 3);

    let r = f64::from(ribbon_radius);
    let mut prev = toolpath.moves.first().map(|m| m.target);

    for m_idx in 1..toolpath.moves.len() {
        // SAFETY: m_idx is 1..len, always valid; prev set from index 0
        #[allow(clippy::indexing_slicing)]
        let mv = &toolpath.moves[m_idx];
        let Some(from) = prev else { continue };
        prev = Some(mv.target);

        let is_cutting = mv.move_type.is_cutting();
        if !is_cutting && !include_rapids {
            continue;
        }

        let color = if is_cutting { CUT_COLOR } else { RAPID_COLOR };

        // Handle arc moves by linearizing
        match mv.move_type {
            MoveType::ArcCW { i, j, .. } | MoveType::ArcCCW { i, j, .. } => {
                let cw = matches!(mv.move_type, MoveType::ArcCW { .. });
                let pts = linearize_arc(from, mv.target, i, j, cw, r * 4.0);
                let mut seg_from = from;
                for pt in pts {
                    push_tube_segment(&mut mesh, seg_from, pt, r, color);
                    seg_from = pt;
                }
                push_tube_segment(&mut mesh, seg_from, mv.target, r, color);
            }
            _ => {
                push_tube_segment(&mut mesh, from, mv.target, r, color);
            }
        }
    }

    mesh
}

/// Auto-compute ribbon radius from toolpath bounding box.
pub fn auto_ribbon_radius(toolpath: &Toolpath) -> f32 {
    let mut min = [f64::MAX; 3];
    let mut max = [f64::MIN; 3];
    for m in &toolpath.moves {
        let p = m.target;
        min[0] = min[0].min(p.x);
        min[1] = min[1].min(p.y);
        min[2] = min[2].min(p.z);
        max[0] = max[0].max(p.x);
        max[1] = max[1].max(p.y);
        max[2] = max[2].max(p.z);
    }
    let extent = (max[0] - min[0]).max(max[1] - min[1]).max(max[2] - min[2]);
    (extent / 300.0).max(0.05) as f32
}

/// Push a single tube segment (rectangular prism) between two points.
fn push_tube_segment(
    mesh: &mut StockMesh,
    from: crate::geo::P3,
    to: crate::geo::P3,
    radius: f64,
    color: [f32; 3],
) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let dz = to.z - from.z;
    let len = (dx * dx + dy * dy + dz * dz).sqrt();
    if len < 1e-10 {
        return;
    }
    let d = [dx / len, dy / len, dz / len];

    // Perpendicular basis vectors
    let ref_axis = if d[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let ux = d[1] * ref_axis[2] - d[2] * ref_axis[1];
    let uy = d[2] * ref_axis[0] - d[0] * ref_axis[2];
    let uz = d[0] * ref_axis[1] - d[1] * ref_axis[0];
    let ulen = (ux * ux + uy * uy + uz * uz).sqrt().max(1e-10);
    let u = [ux / ulen * radius, uy / ulen * radius, uz / ulen * radius];

    let vx = d[1] * u[2] - d[2] * u[1];
    let vy = d[2] * u[0] - d[0] * u[2];
    let vz = d[0] * u[1] - d[1] * u[0];
    let v = [vx, vy, vz]; // already scaled by radius

    let base = mesh.vertex_count() as u32;

    // 8 vertices: 4 at `from`, 4 at `to`
    let corners = [
        // from face
        [
            from.x - u[0] - v[0],
            from.y - u[1] - v[1],
            from.z - u[2] - v[2],
        ],
        [
            from.x + u[0] - v[0],
            from.y + u[1] - v[1],
            from.z + u[2] - v[2],
        ],
        [
            from.x + u[0] + v[0],
            from.y + u[1] + v[1],
            from.z + u[2] + v[2],
        ],
        [
            from.x - u[0] + v[0],
            from.y - u[1] + v[1],
            from.z - u[2] + v[2],
        ],
        // to face
        [to.x - u[0] - v[0], to.y - u[1] - v[1], to.z - u[2] - v[2]],
        [to.x + u[0] - v[0], to.y + u[1] - v[1], to.z + u[2] - v[2]],
        [to.x + u[0] + v[0], to.y + u[1] + v[1], to.z + u[2] + v[2]],
        [to.x - u[0] + v[0], to.y - u[1] + v[1], to.z - u[2] + v[2]],
    ];

    for c in &corners {
        mesh.vertices.push(c[0] as f32);
        mesh.vertices.push(c[1] as f32);
        mesh.vertices.push(c[2] as f32);
        mesh.colors.push(color[0]);
        mesh.colors.push(color[1]);
        mesh.colors.push(color[2]);
    }

    // 12 triangles (6 faces)
    // SAFETY: indices offset by base which accounts for prior vertices
    #[allow(clippy::indexing_slicing)]
    {
        let idx: [[u32; 3]; 12] = [
            [0, 1, 5],
            [0, 5, 4], // front
            [1, 2, 6],
            [1, 6, 5], // right
            [2, 3, 7],
            [2, 7, 6], // back
            [3, 0, 4],
            [3, 4, 7], // left
            [0, 3, 2],
            [0, 2, 1], // start cap
            [4, 5, 6],
            [4, 6, 7], // end cap
        ];
        for tri in &idx {
            mesh.indices.push(base + tri[0]);
            mesh.indices.push(base + tri[1]);
            mesh.indices.push(base + tri[2]);
        }
    }
}
