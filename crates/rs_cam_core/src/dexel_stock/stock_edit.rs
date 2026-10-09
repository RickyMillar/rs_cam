//! S3 (`planning/stock_additions_2026-10-09/PLAN.md`): the cell scans of a
//! stock change.
//!
//! A stock change becomes a list of [`ColumnEdit`]s: per Z-grid cell, the
//! intervals along the ray to add or to remove. One scan per geometry makes
//! the list; [`apply_column_edits`] writes it to a grid. The orchestration
//! (the frames, the material slot, the refusals, the playback stock) is in
//! `compute/stock_change_apply.rs`.
//!
//! Every scan walks its cells through `stamping::walk_cell_box`, the cell
//! walk that the cutter scan (`for_each_covered_cell`, STK-01) also uses.
//! Each cell is sampled at its node, as a stamp samples it: the cell centre
//! of a dexel column is the node `origin + k * cell`.

use smallvec::SmallVec;

use super::stamping::{CellBox, clamped_cell_bbox, walk_cell_box};
use crate::geo::{P2, P3};
use crate::mesh::{QueryScratch, SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::stock::dexel::{DexelGrid, ray_material_length, ray_top};
use crate::stock::material_slot::MaterialSlot;

/// The intervals of one column, sorted and without overlap.
pub type ColumnIntervals = SmallVec<[(f32, f32); 2]>;

/// The edit of one Z-grid column: the intervals to add or to remove.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnEdit {
    /// The flat cell index (`row * cols + col`).
    pub idx: usize,
    /// The intervals along the ray, in the grid's Z.
    pub intervals: ColumnIntervals,
}

/// What [`apply_column_edits`] does with each interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnEditOp {
    /// Union the interval, as material of the slot. Existing material keeps
    /// its own slot.
    Union(MaterialSlot),
    /// Subtract the interval from every material.
    Subtract,
}

/// The share of the hit cells that may have an odd hit count before
/// [`mesh_cast_edits`] calls a mesh open.
///
/// A vertical ray through a closed mesh crosses the surface an even number
/// of times. The half-open edge rule in the cast counts a hit on a shared
/// edge once, so a closed mesh gives an odd count only where float noise
/// beats the rule (a ray through a vertex, a sliver triangle). An open mesh
/// gives an odd count over the whole shadow of each hole, which is a large
/// share for any hole bigger than a few cells. One percent separates the
/// two cases with a wide margin on both sides.
pub const MESH_ODD_CELL_SHARE_LIMIT: f64 = 0.01;

/// The result of a vertical ray cast through a mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshCast {
    /// The even-odd intervals of each cell with an even hit count.
    pub edits: Vec<ColumnEdit>,
    /// The cells whose ray hit the mesh at least once.
    pub hit_cells: usize,
    /// The cells with an odd hit count. They get no edit.
    pub odd_cells: usize,
}

impl MeshCast {
    /// The share of the hit cells with an odd hit count (0 when no cell
    /// was hit).
    #[must_use]
    pub fn odd_share(&self) -> f64 {
        if self.hit_cells == 0 {
            0.0
        } else {
            self.odd_cells as f64 / self.hit_cells as f64
        }
    }

    /// `true` when the odd share is above [`MESH_ODD_CELL_SHARE_LIMIT`].
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.odd_share() > MESH_ODD_CELL_SHARE_LIMIT
    }
}

/// The cell rectangle of `grid` whose nodes can lie inside the XY box
/// `[u_min, u_max] x [v_min, v_max]`, or `None` when the box misses the grid.
fn cell_box_over(
    grid: &DexelGrid,
    u_min: f64,
    u_max: f64,
    v_min: f64,
    v_max: f64,
) -> Option<CellBox> {
    if !(u_min.is_finite() && u_max.is_finite() && v_min.is_finite() && v_max.is_finite()) {
        return None;
    }
    let cs = grid.cell_size;
    let col_min = ((u_min - grid.origin_u) / cs).floor() as isize;
    let col_max = ((u_max - grid.origin_u) / cs).ceil() as isize;
    let row_min = ((v_min - grid.origin_v) / cs).floor() as isize;
    let row_max = ((v_max - grid.origin_v) / cs).ceil() as isize;
    let (col_lo, col_hi, row_lo, row_hi) =
        clamped_cell_bbox(col_min, col_max, row_min, row_max, grid.cols, grid.rows)?;
    Some(CellBox {
        origin_u: grid.origin_u,
        origin_v: grid.origin_v,
        cell_size: cs,
        col_lo,
        col_hi,
        row_lo,
        row_hi,
    })
}

/// The flat index of every cell of `grid` whose node is inside one of
/// `outlines`, in row-major order. The outlines are in the grid's XY frame.
#[must_use]
pub fn outline_cell_indices(grid: &DexelGrid, outlines: &[Polygon2]) -> Vec<usize> {
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for polygon in outlines {
        let [x_min, y_min, x_max, y_max] = polygon.bbox();
        bounds = [
            bounds[0].min(x_min),
            bounds[1].min(y_min),
            bounds[2].max(x_max),
            bounds[3].max(y_max),
        ];
    }
    let Some(cells) = cell_box_over(grid, bounds[0], bounds[2], bounds[1], bounds[3]) else {
        return Vec::new();
    };
    let cols = grid.cols;
    let mut inside = Vec::new();
    walk_cell_box(&cells, |row, col, u, v| {
        let p = P2::new(u, v);
        if outlines.iter().any(|polygon| polygon.contains_point(&p)) {
            inside.push(row * cols + col);
        }
    });
    inside
}

/// The edits of an outline fill: every space open to the top (+Z) inside
/// the outlines, up to `level_z`.
///
/// Per cell whose node is inside the outlines, `t` is the top of the
/// highest segment, or `floor_z` (the stock bottom) for an empty ray. When
/// `t < level_z`, the edit is `[t, level_z]`. A gap under material gets no
/// fill: the scan never looks below `t`. `level_z` may be above the stock
/// top: the top then rises.
#[must_use]
pub fn outline_fill_edits(
    grid: &DexelGrid,
    outlines: &[Polygon2],
    level_z: f32,
    floor_z: f32,
) -> Vec<ColumnEdit> {
    outline_cell_indices(grid, outlines)
        .into_iter()
        .filter_map(|idx| {
            let t = grid.rays.get(idx).and_then(ray_top).unwrap_or(floor_z);
            (t < level_z).then(|| ColumnEdit {
                idx,
                intervals: SmallVec::from_buf_and_len([(t, level_z), (0.0, 0.0)], 1),
            })
        })
        .collect()
}

/// The edits of an outline prism: `[z_bottom, z_top]` in every cell whose
/// node is inside the outlines.
#[must_use]
pub fn outline_extrude_edits(
    grid: &DexelGrid,
    outlines: &[Polygon2],
    z_bottom: f32,
    z_top: f32,
) -> Vec<ColumnEdit> {
    if z_bottom.is_nan() || z_top.is_nan() || z_bottom >= z_top {
        return Vec::new();
    }
    outline_cell_indices(grid, outlines)
        .into_iter()
        .map(|idx| ColumnEdit {
            idx,
            intervals: SmallVec::from_buf_and_len([(z_bottom, z_top), (0.0, 0.0)], 1),
        })
        .collect()
}

/// The edge function of the XY point `p` against the edge `a`-`b`, with the
/// two ends in a fixed (lexicographic) order.
///
/// The fixed order makes the value the same, bit for bit, for the two
/// triangles that share the edge. The half-open rule in [`vertical_hit`]
/// needs that.
#[inline]
fn canonical_edge(p: (f64, f64), a: &P3, b: &P3) -> f64 {
    let (lo, hi) = if (a.x, a.y) <= (b.x, b.y) {
        (a, b)
    } else {
        (b, a)
    };
    (hi.x - lo.x) * (p.1 - lo.y) - (hi.y - lo.y) * (p.0 - lo.x)
}

/// The Z where the vertical line through `p` crosses the triangle, or
/// `None` when it misses the triangle or the triangle is vertical.
///
/// Half-open rule: a point on an edge counts for the triangle whose third
/// vertex is on the positive side of that edge's canonical edge function.
/// Two triangles that share an edge lie on opposite sides of it, so a ray
/// through the edge hits exactly one of them.
fn vertical_hit(p: (f64, f64), tri: [&P3; 3]) -> Option<f64> {
    let [p0, p1, p2] = tri;
    let mut z = 0.0;
    for (opposite, a, b) in [(p0, p1, p2), (p1, p2, p0), (p2, p0, p1)] {
        let w_opposite = canonical_edge((opposite.x, opposite.y), a, b);
        if w_opposite == 0.0 {
            return None;
        }
        let w = canonical_edge(p, a, b);
        let inside = if w == 0.0 {
            w_opposite > 0.0
        } else {
            (w > 0.0) == (w_opposite > 0.0)
        };
        if !inside {
            return None;
        }
        z += opposite.z * (w / w_opposite);
    }
    Some(z)
}

/// Cast a vertical ray through `mesh` at each node of `grid`, collect every
/// hit, sort the hits and pair them even-odd into intervals.
///
/// The mesh is in the grid's frame. The spatial index of the mesh gives the
/// triangle candidates of each ray. A cell with an odd hit count gets no
/// edit; [`MeshCast::is_open`] decides whether the mesh is closed.
#[must_use]
pub fn mesh_cast_edits(grid: &DexelGrid, mesh: &TriangleMesh) -> MeshCast {
    let mut cast = MeshCast {
        edits: Vec::new(),
        hit_cells: 0,
        odd_cells: 0,
    };
    if mesh.triangles.is_empty() {
        return cast;
    }
    let b = &mesh.bbox;
    let Some(cells) = cell_box_over(grid, b.min.x, b.max.x, b.min.y, b.max.y) else {
        return cast;
    };
    let index = SpatialIndex::build_auto(mesh);
    let mut scratch = QueryScratch::default();
    let mut candidates: Vec<usize> = Vec::new();
    let mut hits: Vec<f64> = Vec::new();
    let cols = grid.cols;
    walk_cell_box(&cells, |row, col, u, v| {
        index.query_into(u, v, 0.0, &mut scratch, &mut candidates);
        hits.clear();
        for &t in &candidates {
            let Some(tri) = mesh.triangles.get(t) else {
                continue;
            };
            let vertex = |i: u32| mesh.vertices.get(i as usize);
            let (Some(a), Some(b), Some(c)) = (vertex(tri[0]), vertex(tri[1]), vertex(tri[2]))
            else {
                continue;
            };
            if let Some(z) = vertical_hit((u, v), [a, b, c]) {
                hits.push(z);
            }
        }
        if hits.is_empty() {
            return;
        }
        cast.hit_cells += 1;
        if hits.len() % 2 == 1 {
            cast.odd_cells += 1;
            return;
        }
        hits.sort_by(f64::total_cmp);
        let intervals: ColumnIntervals = hits
            .as_chunks::<2>()
            .0
            .iter()
            .filter(|[lo, hi]| hi > lo)
            .map(|&[lo, hi]| (lo as f32, hi as f32))
            .collect();
        if !intervals.is_empty() {
            cast.edits.push(ColumnEdit {
                idx: row * cols + col,
                intervals,
            });
        }
    });
    cast
}

/// Write `edits` to `grid` and return the change of material volume, in
/// mm³: positive for a union, negative for a subtraction.
///
/// The volume of a column is its material length times the cell area, the
/// measure the stamp kernels use for removed volume.
pub fn apply_column_edits(grid: &mut DexelGrid, edits: &[ColumnEdit], op: ColumnEditOp) -> f64 {
    let area = grid.cell_size * grid.cell_size;
    let mut delta = 0.0_f64;
    for edit in edits {
        let Some(before) = grid.rays.get(edit.idx).map(ray_material_length) else {
            continue;
        };
        for &(a, b) in &edit.intervals {
            match op {
                ColumnEditOp::Union(slot) => grid.union_interval_at(edit.idx, a, b, slot),
                ColumnEditOp::Subtract => grid.subtract_interval_at(edit.idx, a, b),
            }
        }
        let after = grid.rays.get(edit.idx).map_or(before, ray_material_length);
        delta += f64::from(after - before) * area;
    }
    delta
}

#[cfg(test)]
#[allow(
    // SAFETY: test code; a failed lookup is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::geo::BoundingBox3;

    /// A 20 x 20 x 10 stock at 0.5 mm cells: nodes at 0, 0.5, .., 20.
    fn grid() -> DexelGrid {
        DexelGrid::z_grid_from_bounds(
            &BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(20.0, 20.0, 10.0),
            },
            0.5,
        )
    }

    /// A rectangle whose edges sit at half-cell offsets, so the node test
    /// is exact: `nx * ny` nodes inside.
    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
        Polygon2::new(vec![
            P2::new(x0, y0),
            P2::new(x1, y0),
            P2::new(x1, y1),
            P2::new(x0, y1),
        ])
    }

    /// A closed axis-aligned box: 8 vertices, 12 triangles.
    fn box_mesh(lo: P3, hi: P3) -> TriangleMesh {
        let v = |x: bool, y: bool, z: bool| {
            P3::new(
                if x { hi.x } else { lo.x },
                if y { hi.y } else { lo.y },
                if z { hi.z } else { lo.z },
            )
        };
        let vertices = vec![
            v(false, false, false),
            v(true, false, false),
            v(true, true, false),
            v(false, true, false),
            v(false, false, true),
            v(true, false, true),
            v(true, true, true),
            v(false, true, true),
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        TriangleMesh::from_raw(vertices, triangles)
    }

    #[test]
    fn the_outline_scan_finds_the_nodes_inside() {
        let g = grid();
        // x 4.75..8.25 holds nodes 5.0..8.0 (7); y 2.75..4.25 holds 3.0..4.0 (3).
        let cells = outline_cell_indices(&g, &[rect(4.75, 2.75, 8.25, 4.25)]);
        assert_eq!(cells.len(), 21);
    }

    #[test]
    fn a_closed_box_casts_one_interval_per_cell_and_is_not_open() {
        let g = grid();
        // Box corners ON nodes, so rays run along the top-face diagonal and
        // through vertices: the half-open rule must still count once.
        let mesh = box_mesh(P3::new(4.0, 4.0, 2.0), P3::new(10.0, 10.0, 6.0));
        let cast = mesh_cast_edits(&g, &mesh);
        assert!(!cast.is_open(), "odd share {}", cast.odd_share());
        for edit in &cast.edits {
            assert_eq!(edit.intervals.len(), 1);
            let (a, b) = edit.intervals[0];
            assert!((a - 2.0).abs() < 1e-5 && (b - 6.0).abs() < 1e-5, "{a}..{b}");
        }
    }
}
