//! Surface slope analysis and heightmap infrastructure for 3D finishing strategies.
//!
//! Provides `SurfaceHeightmap` (precomputed mesh Z heights) and `SlopeMap` (surface normals,
//! slope angles, curvature). These are the shared foundation for scallop finishing,
//! steep & shallow, ramp finishing, and slope-aware adaptive improvements.

use crate::geo::V3;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;

// ── Surface heightmap ─────────────────────────────────────────────────

/// One cell's Z reading off a [`SurfaceHeightmap`], with its coverage state
/// carried in the type (C2, 2026-07-30).
///
/// The grid stores a plain `f64` per cell, but that number means two
/// different things depending on whether the cell's vertical ray actually
/// passes through the mesh. Reading it as a bare `f64` is the silent-sentinel
/// class that produced PR-8b's plane-wide clamp: uncovered cells carry the
/// `min_z` clamp (the mesh bbox floor) and every consumer that treated that
/// as "the surface is down there" was wrong.
///
/// A consumer that genuinely wants the clamped number — clearing strategies
/// that must rough stock *beside* the model down to a floor — says so by
/// name: [`GridZ::z_or_bbox_floor`], or the whole-grid escape hatches
/// [`SurfaceHeightmap::z_or_bbox_floor_at`] /
/// [`SurfaceHeightmap::z_or_bbox_floor_at_world`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridZ {
    /// The cell's vertical ray passes through the mesh: this is a real
    /// drop-cutter contact height for the sampling cutter.
    Covered(f64),
    /// The ray misses every triangle. The stored number is whatever
    /// `point_drop_cutter` returned clamped up from the `min_z` floor —
    /// i.e. the **mesh bbox floor** for cells further than a tool radius
    /// from the model, and a **rim contact** height for cells just outside
    /// the footprint (the cutter has radius, so it still touches the edge).
    /// Either way nothing has verified there is material here.
    Uncovered {
        /// The stored, clamped value. Named for what it usually is.
        z_or_bbox_floor: f64,
    },
    /// The query point lies outside the grid entirely.
    OutOfBounds,
}

impl GridZ {
    /// The Z of a cell whose ray hits the mesh; `None` for uncovered cells
    /// and out-of-bounds queries.
    #[must_use]
    #[inline]
    pub fn covered(self) -> Option<f64> {
        match self {
            Self::Covered(z) => Some(z),
            Self::Uncovered { .. } | Self::OutOfBounds => None,
        }
    }

    /// The stored number regardless of coverage — the explicit escape hatch.
    /// `None` only for [`GridZ::OutOfBounds`], which has no stored number at
    /// all (the pre-C2 API spelled that `f64::NEG_INFINITY`).
    #[must_use]
    #[inline]
    pub fn z_or_bbox_floor(self) -> Option<f64> {
        match self {
            Self::Covered(z) | Self::Uncovered { z_or_bbox_floor: z } => Some(z),
            Self::OutOfBounds => None,
        }
    }

    /// Whether this reading is backed by mesh under the cell.
    #[must_use]
    #[inline]
    pub fn is_covered(self) -> bool {
        matches!(self, Self::Covered(_))
    }
}

/// Precomputed mesh surface Z heights at grid resolution.
/// One parallel batch of drop-cutter queries at init, then O(1) lookups.
///
/// `z_values` and `covered` are private and always the same length: a Z
/// without its coverage flag is the defect this type exists to prevent.
/// Read cells through [`Self::z_at`] / [`Self::z_at_world`] (typed) or the
/// honestly-named [`Self::z_or_bbox_floor_at`] family (untyped escape hatch).
pub struct SurfaceHeightmap {
    z_values: Vec<f64>,
    /// True when the vertical ray at the cell center passes through at
    /// least one triangle footprint — false for holes in open meshes and
    /// for cells outside the mesh XY extent. `point_drop_cutter` happily
    /// reports rim contact on such cells (the cutter has radius), so the
    /// drop-cutter Z alone can't distinguish "surface" from "no surface".
    /// Uncovered cells keep the `min_z` clamp in `z_values` — clearing
    /// strategies rely on that floor to rough out stock beside/around
    /// the model (and through mesh holes; the user-facing lever to stop
    /// a rough descending into holes is a pinned heights `bottom_z`).
    /// The mask lets consumers tell the two cases apart — e.g. full-ray
    /// border clears, or future enclosed-hole handling (heights audit
    /// 2026-06-12, finding 3).
    covered: Vec<bool>,
    pub rows: usize,
    pub cols: usize,
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell_size: f64,
}

impl SurfaceHeightmap {
    /// Build via rayon-parallelized drop-cutter queries at each grid cell.
    // infallible: cancel closure always returns false, so Cancelled is unreachable
    #[allow(clippy::too_many_arguments, clippy::expect_used)]
    pub fn from_mesh(
        mesh: &TriangleMesh,
        index: &SpatialIndex,
        cutter: &dyn MillingCutter,
        origin_x: f64,
        origin_y: f64,
        rows: usize,
        cols: usize,
        cell_size: f64,
        min_z: f64,
    ) -> Self {
        let never_cancel = || false;
        Self::from_mesh_with_cancel(
            mesh,
            index,
            cutter,
            origin_x,
            origin_y,
            rows,
            cols,
            cell_size,
            min_z,
            &never_cancel,
        )
        .expect("non-cancellable surface heightmap should never be cancelled")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_mesh_with_cancel(
        mesh: &TriangleMesh,
        index: &SpatialIndex,
        cutter: &dyn MillingCutter,
        origin_x: f64,
        origin_y: f64,
        rows: usize,
        cols: usize,
        cell_size: f64,
        min_z: f64,
        cancel: &dyn CancelCheck,
    ) -> Result<Self, Cancelled> {
        let total = rows * cols;
        // Shared per-cell sampling (planning/finishing_stack_review_2026-07.md
        // P1.3): the same "drop-cutter + min_z clamp (+ optional coverage)"
        // cell computation `DropCutterGrid`'s batch layer uses, so the two
        // grid layers no longer carry independent copies of the drop-cutter
        // call + clamp. Coverage is computed in this same per-cell pass (not
        // a second loop) — the one thing `DropCutterGrid` doesn't need, so it
        // passes `with_coverage = false` at its own call sites.
        //
        // The outer parallel-batch dispatch stays heightmap-local rather
        // than also routing through `dropcutter::batch_sample_grid`: that
        // routine requires `cancel: &(dyn CancelCheck + Sync)` for its
        // rayon closures, while every call site up the finish-op chain
        // (`finish_setup.rs`, `scallop.rs`, `ramp_finish.rs`, `waterline.rs`,
        // `adaptive3d/path.rs`) passes a plain `&dyn CancelCheck` — widening
        // that bound would ripple signature changes through files outside
        // this pass's scope. `compute_cell` below is the only remaining
        // heightmap-specific glue; everything else is shared.
        let compute_cell = |i: usize| -> (f64, bool) {
            let row = i / cols;
            let col = i % cols;
            let x = origin_x + col as f64 * cell_size;
            let y = origin_y + row as f64 * cell_size;
            let (cl, covered) =
                crate::dropcutter::sample_grid_cell(x, y, mesh, index, cutter, min_z, true);
            (cl.z, covered)
        };

        // Parallel drop-cutter: each cell is independent.
        #[cfg(not(target_arch = "wasm32"))]
        let (z_values, covered) = {
            use rayon::prelude::*;
            // Poll cancellation BETWEEN batches, not once after the whole
            // grid. A single `(0..total).into_par_iter()` is uninterruptible
            // for its full duration, and the classification grid is no longer
            // small: the §14q cusp-radius fix took wanaka's grid from 143² to
            // 849², where one batch runs ~47 s and cancel reads as dead for
            // all of it. The batch is large enough that rayon still has real
            // work to spread (8192 drop-cutter samples), so the split costs
            // scheduling overhead only.
            const CANCEL_BATCH_CELLS: usize = 8192;
            let mut z_values = Vec::with_capacity(total);
            let mut covered = Vec::with_capacity(total);
            let mut start = 0usize;
            while start < total {
                crate::interrupt::check_cancel(cancel)?;
                let end = (start + CANCEL_BATCH_CELLS).min(total);
                let batch: Vec<(f64, bool)> =
                    (start..end).into_par_iter().map(compute_cell).collect();
                for (z, c) in batch {
                    z_values.push(z);
                    covered.push(c);
                }
                start = end;
            }
            (z_values, covered)
        };
        #[cfg(target_arch = "wasm32")]
        let (z_values, covered) = {
            let mut zs = Vec::with_capacity(total);
            let mut cov = Vec::with_capacity(total);
            for i in 0..total {
                if i % 64 == 0 {
                    crate::interrupt::check_cancel(cancel)?;
                }
                let (z, c) = compute_cell(i);
                zs.push(z);
                cov.push(c);
            }
            (zs, cov)
        };

        Ok(Self {
            z_values,
            covered,
            rows,
            cols,
            origin_x,
            origin_y,
            cell_size,
        })
    }

    /// Assemble a heightmap from already-computed cells — the constructor
    /// for fixtures and for callers that sample the grid themselves.
    ///
    /// # Panics
    /// When `z_values.len() != covered.len()` or either differs from
    /// `rows * cols`: the pairing is this type's whole invariant.
    #[allow(clippy::too_many_arguments, clippy::panic)]
    #[must_use]
    pub fn from_parts(
        z_values: Vec<f64>,
        covered: Vec<bool>,
        rows: usize,
        cols: usize,
        origin_x: f64,
        origin_y: f64,
        cell_size: f64,
    ) -> Self {
        if z_values.len() != covered.len() || z_values.len() != rows * cols {
            panic!(
                "SurfaceHeightmap::from_parts: {} z values / {} coverage flags for a {rows}x{cols} grid",
                z_values.len(),
                covered.len(),
            );
        }
        Self {
            z_values,
            covered,
            rows,
            cols,
            origin_x,
            origin_y,
            cell_size,
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// O(1) typed cell read: the Z **and** whether mesh backs it.
    ///
    /// # Panics
    /// On out-of-range `row`/`col` (bounded-index contract, same as the
    /// pre-C2 `surface_z_at`). Use [`Self::z_at_world`] for unvalidated
    /// coordinates — that one reports [`GridZ::OutOfBounds`].
    #[inline]
    #[must_use]
    pub fn z_at(&self, row: usize, col: usize) -> GridZ {
        let i = row * self.cols + col;
        if self.covered[i] {
            GridZ::Covered(self.z_values[i])
        } else {
            GridZ::Uncovered {
                z_or_bbox_floor: self.z_values[i],
            }
        }
    }

    /// Typed cell read by flat row-major index — for the loops that walk a
    /// parallel grid (the dexel stock's `z_grid`) and index both by the same
    /// `row * cols + col`. [`GridZ::OutOfBounds`] past the end.
    #[allow(clippy::indexing_slicing)] // bounds checked on the line above
    #[inline]
    #[must_use]
    pub fn z_at_index(&self, i: usize) -> GridZ {
        if i >= self.z_values.len() {
            return GridZ::OutOfBounds;
        }
        if self.covered[i] {
            GridZ::Covered(self.z_values[i])
        } else {
            GridZ::Uncovered {
                z_or_bbox_floor: self.z_values[i],
            }
        }
    }

    /// Typed cell read at world coordinates; [`GridZ::OutOfBounds`] off-grid.
    #[must_use]
    pub fn z_at_world(&self, x: f64, y: f64) -> GridZ {
        match self.world_cell(x, y) {
            Some((row, col)) => self.z_at(row, col),
            None => GridZ::OutOfBounds,
        }
    }

    /// Nearest cell to a world XY, or `None` when the point is off-grid.
    #[inline]
    fn world_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let col_f = (x - self.origin_x) / self.cell_size;
        let row_f = (y - self.origin_y) / self.cell_size;
        if col_f < -0.5 || row_f < -0.5 {
            return None;
        }
        let col = col_f.round() as isize;
        let row = row_f.round() as isize;
        if col < 0 || row < 0 || col >= self.cols as isize || row >= self.rows as isize {
            return None;
        }
        Some((row as usize, col as usize))
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// O(1) **untyped** Z lookup: the stored number whether or not mesh backs
    /// the cell. The escape hatch, named so the choice is visible at the call
    /// site — clearing strategies that must take stock beside the model down
    /// to the bbox floor want exactly this. Everyone else wants [`Self::z_at`].
    #[inline]
    #[must_use]
    pub fn z_or_bbox_floor_at(&self, row: usize, col: usize) -> f64 {
        self.z_values[row * self.cols + col]
    }

    /// Untyped Z at world coordinates. Returns `NEG_INFINITY` off-grid —
    /// itself a sentinel, kept only for the call sites that already branch on
    /// it; [`Self::z_at_world`] is the typed replacement.
    #[must_use]
    pub fn z_or_bbox_floor_at_world(&self, x: f64, y: f64) -> f64 {
        self.z_at_world(x, y)
            .z_or_bbox_floor()
            .unwrap_or(f64::NEG_INFINITY)
    }

    /// The raw Z storage, uncovered cells included (see [`GridZ`]). Named for
    /// what it holds; pair it with [`Self::covered_flags`], which is always
    /// the same length.
    #[must_use]
    #[inline]
    pub fn z_or_bbox_floor_values(&self) -> &[f64] {
        &self.z_values
    }

    /// Per-cell mesh-coverage mask, row-major, same length as
    /// [`Self::z_or_bbox_floor_values`].
    #[must_use]
    #[inline]
    pub fn covered_flags(&self) -> &[bool] {
        &self.covered
    }

    /// Minimum Z across all cells, uncovered ones **included** — so on any
    /// padded grid (every finish surface: the grid runs one envelope radius
    /// beyond the mesh bbox in XY, and those corner cells never hit the mesh)
    /// this is the mesh bbox floor, a depth nothing has checked the cutter can
    /// hold. That is by design for clearing: adaptive plans its deepest level
    /// from this value and stock beside/around the model must come down to the
    /// floor (see the hemisphere clearing tests), and waterline ladders need
    /// the full mesh height because vertical walls live below every
    /// drop-cutter-reachable Z.
    ///
    /// For "the deepest this cutter can actually descend on this surface" use
    /// [`Self::min_covered_z`]. Renamed from `min_z()` in C2 (2026-07-30):
    /// the old name read like the latter and was audited as the former at
    /// every consumer.
    #[must_use]
    pub fn min_z_or_bbox_floor(&self) -> f64 {
        self.z_values.iter().copied().fold(f64::INFINITY, f64::min)
    }

    /// Minimum Z across cells the cutter's ray actually reaches — i.e. the
    /// deepest this cutter's reference point can descend anywhere on this
    /// surface. `None` when no cell is covered.
    ///
    /// The counterpart [`Self::min_z_or_bbox_floor`] warns about, made
    /// available instead of left to each caller to re-derive. The distinction
    /// is not cosmetic: on a 22 mm grid padded by one envelope radius there
    /// are ALWAYS uncovered cells carrying the `min_z` clamp, so
    /// `min_z_or_bbox_floor()` is the MESH bbox floor
    /// on essentially every finish surface — a depth nothing has checked the
    /// cutter can hold. Measured on the Checkpoint B `patches + hole`
    /// fixture (62° conical pit, Ø1-tip / 7° / Ø6-shank taper):
    /// `min_z_or_bbox_floor()` = −3.000 (the pit apex, unreachable),
    /// `min_covered_z()` =
    /// −2.407 (where the profile actually wedges). The closed-form contact
    /// solution for that cone and that taper is −2.439, so the grid answer
    /// is the same number to a third of a cell.
    ///
    /// A clearing op that must take stock BESIDE the model down to a floor
    /// still wants [`Self::min_z_or_bbox_floor`] — that is why both exist and
    /// why neither is the other's default.
    #[must_use]
    pub fn min_covered_z(&self) -> Option<f64> {
        self.z_values
            .iter()
            .zip(self.covered.iter())
            .filter_map(|(z, covered)| covered.then_some(*z))
            .fold(None, |acc: Option<f64>, z| {
                Some(acc.map_or(z, |a| a.min(z)))
            })
    }

    /// Whether the cell's vertical ray actually passes through the mesh.
    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    #[inline]
    pub fn covered_at(&self, row: usize, col: usize) -> bool {
        self.covered[row * self.cols + col]
    }

    /// Compute a slope map from this surface heightmap.
    pub fn slope_map(&self) -> SlopeMap {
        SlopeMap::from_z_grid(
            &self.z_values,
            self.rows,
            self.cols,
            self.origin_x,
            self.origin_y,
            self.cell_size,
        )
    }

    /// Classification-stencil slope map (max of one-sided gradients per
    /// axis) — registers single-cell steps that central differences smear.
    /// See [`SlopeMap::from_z_grid_max_gradient`] for when (and when NOT)
    /// to use this.
    pub fn slope_map_max_gradient(&self) -> SlopeMap {
        SlopeMap::from_z_grid_max_gradient(
            &self.z_values,
            self.rows,
            self.cols,
            self.origin_x,
            self.origin_y,
            self.cell_size,
        )
    }
}

// ── Slope map ─────────────────────────────────────────────────────────

/// Grid of slope angles, surface normals, and mean curvature computed from a Z heightmap.
pub struct SlopeMap {
    /// Unit surface normal at each cell. Row-major.
    pub normals: Vec<V3>,
    /// Slope angle from horizontal at each cell, in radians [0, PI/2].
    /// 0 = horizontal flat, PI/2 = vertical wall.
    pub angles: Vec<f64>,
    /// Mean curvature at each cell, computed straight from the raw
    /// `(d2z/dx2 + d2z/dy2) * 0.5` second derivatives.
    /// Negative = physically convex (e.g. a dome peak, where the surface
    /// curves downward away from the high point); positive = physically
    /// concave (e.g. a bowl). This is the opposite of the convention used by
    /// `scallop_math` (positive = convex there), so callers crossing that
    /// boundary — see `scallop::average_stepover_for_ring` — must negate
    /// this value before passing it to `scallop_math::variable_stepover`.
    pub curvatures: Vec<f64>,
    pub rows: usize,
    pub cols: usize,
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell_size: f64,
}

/// Compute normal, angle, curvature from derivatives and store at `idx`.
/// Shared by the [`SlopeMap`] constructors.
#[inline(always)]
#[allow(clippy::indexing_slicing)] // SAFETY: idx bounded by caller loop ranges
#[allow(clippy::needless_pass_by_value)] // tuple of mut refs is the natural pattern
fn store_slope_cell(
    out: (&mut [V3], &mut [f64], &mut [f64]),
    idx: usize,
    dz_dx: f64,
    dz_dy: f64,
    d2z_dx2: f64,
    d2z_dy2: f64,
) {
    let n = V3::new(-dz_dx, -dz_dy, 1.0).normalize();
    out.0[idx] = n;
    out.1[idx] = n.z.clamp(0.0, 1.0).acos();
    out.2[idx] = (d2z_dx2 + d2z_dy2) * 0.5;
}

impl SlopeMap {
    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Build a SlopeMap from a grid of Z values using finite differences.
    ///
    /// Central differences for interior cells, forward/backward at boundaries.
    pub fn from_z_grid(
        z_values: &[f64],
        rows: usize,
        cols: usize,
        origin_x: f64,
        origin_y: f64,
        cell_size: f64,
    ) -> Self {
        let total = rows * cols;
        let mut normals = vec![V3::new(0.0, 0.0, 1.0); total];
        let mut angles = vec![0.0f64; total];
        let mut curvatures = vec![0.0f64; total];

        let cs = cell_size;
        let inv_cs = 1.0 / cs;
        let inv_cs2 = 1.0 / (cs * 2.0);
        let inv_cs_sq = 1.0 / (cs * cs);

        // ── Interior cells: no boundary checks, full central differences ──
        for row in 1..rows.saturating_sub(1) {
            let row_base = row * cols;
            let row_above = (row - 1) * cols;
            let row_below = (row + 1) * cols;
            for col in 1..cols.saturating_sub(1) {
                // SAFETY: row in 1..rows-1 and col in 1..cols-1 ensures all offsets are in bounds
                #[allow(clippy::indexing_slicing)]
                let (dz_dx, dz_dy, d2z_dx2, d2z_dy2) = {
                    let zc = z_values[row_base + col];
                    let zl = z_values[row_base + col - 1];
                    let zr = z_values[row_base + col + 1];
                    let zu = z_values[row_above + col];
                    let zd = z_values[row_below + col];
                    (
                        (zr - zl) * inv_cs2,
                        (zd - zu) * inv_cs2,
                        (zr - 2.0 * zc + zl) * inv_cs_sq,
                        (zd - 2.0 * zc + zu) * inv_cs_sq,
                    )
                };
                store_slope_cell(
                    (&mut normals, &mut angles, &mut curvatures),
                    row_base + col,
                    dz_dx,
                    dz_dy,
                    d2z_dx2,
                    d2z_dy2,
                );
            }
        }

        // ── Boundary cells: use forward/backward differences ──────────────
        for row in 0..rows {
            for col in 0..cols {
                // Skip interior (already processed)
                if row > 0 && row < rows - 1 && col > 0 && col < cols - 1 {
                    continue;
                }
                #[allow(clippy::indexing_slicing)]
                let dz_dx = if col == 0 && cols > 1 {
                    (z_values[row * cols + 1] - z_values[row * cols]) * inv_cs
                } else if col == cols - 1 && cols > 1 {
                    (z_values[row * cols + col] - z_values[row * cols + col - 1]) * inv_cs
                } else if cols > 1 {
                    (z_values[row * cols + col + 1] - z_values[row * cols + col - 1]) * inv_cs2
                } else {
                    0.0
                };

                #[allow(clippy::indexing_slicing)]
                let dz_dy = if row == 0 && rows > 1 {
                    (z_values[cols + col] - z_values[col]) * inv_cs
                } else if row == rows - 1 && rows > 1 {
                    (z_values[row * cols + col] - z_values[(row - 1) * cols + col]) * inv_cs
                } else if rows > 1 {
                    (z_values[(row + 1) * cols + col] - z_values[(row - 1) * cols + col]) * inv_cs2
                } else {
                    0.0
                };

                let d2z_dx2 = if col > 0 && col < cols - 1 {
                    #[allow(clippy::indexing_slicing)]
                    let v = (z_values[row * cols + col + 1] - 2.0 * z_values[row * cols + col]
                        + z_values[row * cols + col - 1])
                        * inv_cs_sq;
                    v
                } else {
                    0.0
                };

                let d2z_dy2 = if row > 0 && row < rows - 1 {
                    #[allow(clippy::indexing_slicing)]
                    let v = (z_values[(row + 1) * cols + col] - 2.0 * z_values[row * cols + col]
                        + z_values[(row - 1) * cols + col])
                        * inv_cs_sq;
                    v
                } else {
                    0.0
                };

                store_slope_cell(
                    (&mut normals, &mut angles, &mut curvatures),
                    row * cols + col,
                    dz_dx,
                    dz_dy,
                    d2z_dx2,
                    d2z_dy2,
                );
            }
        }

        Self {
            normals,
            angles,
            curvatures,
            rows,
            cols,
            origin_x,
            origin_y,
            cell_size,
        }
    }

    /// Build a SlopeMap for CLASSIFICATION: per axis, the gradient is the
    /// one-sided forward/backward difference with the LARGER magnitude,
    /// not their average (the central difference).
    ///
    /// Central differences smear a discontinuity confined to one cell
    /// across two — a step of height `h` reads `atan(h / (2·cell))`, so a
    /// ~1 mm lake-shore step at a 0.75 mm classification grid reads ~34°
    /// and never crosses a 45° steep threshold (the wanaka coastline gap,
    /// user-observed 2026-07-08; see `planning/unified_finish_planner_design.md`
    /// "Known classification gap"). With this stencil the same step reads
    /// `atan(h / cell)` from both adjacent cells. On smooth surfaces the
    /// two stencils agree to first order.
    ///
    /// This is a slope-band CLASSIFICATION stencil only: max-of-one-sided
    /// is not the calculus gradient and biases steep at noise/creases, so
    /// GENERATION surfaces (drop-cutter offset heightmaps feeding scallop /
    /// raster / waterline) must keep [`SlopeMap::from_z_grid`]. On ties the
    /// forward difference wins, which keeps normals deterministic; slope
    /// angle is unaffected by the choice. Curvature keeps the same central
    /// second differences as `from_z_grid` (zero where a neighbour is
    /// missing).
    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    pub fn from_z_grid_max_gradient(
        z_values: &[f64],
        rows: usize,
        cols: usize,
        origin_x: f64,
        origin_y: f64,
        cell_size: f64,
    ) -> Self {
        let total = rows * cols;
        let mut normals = vec![V3::new(0.0, 0.0, 1.0); total];
        let mut angles = vec![0.0f64; total];
        let mut curvatures = vec![0.0f64; total];

        let inv_cs = 1.0 / cell_size;
        let inv_cs_sq = 1.0 / (cell_size * cell_size);

        // Pick the one-sided difference with the larger magnitude; forward
        // wins ties. Missing sides (grid edges) fall back to the other.
        let pick = |forward: Option<f64>, backward: Option<f64>| -> f64 {
            match (forward, backward) {
                (Some(f), Some(b)) => {
                    if f.abs() >= b.abs() {
                        f
                    } else {
                        b
                    }
                }
                (Some(f), None) => f,
                (None, Some(b)) => b,
                (None, None) => 0.0,
            }
        };

        for row in 0..rows {
            for col in 0..cols {
                let idx = row * cols + col;
                let zc = z_values[idx];
                let fwd_x = (col + 1 < cols).then(|| (z_values[idx + 1] - zc) * inv_cs);
                let bwd_x = (col > 0).then(|| (zc - z_values[idx - 1]) * inv_cs);
                let fwd_y = (row + 1 < rows).then(|| (z_values[idx + cols] - zc) * inv_cs);
                let bwd_y = (row > 0).then(|| (zc - z_values[idx - cols]) * inv_cs);

                let dz_dx = pick(fwd_x, bwd_x);
                let dz_dy = pick(fwd_y, bwd_y);

                let d2z_dx2 = if col > 0 && col + 1 < cols {
                    (z_values[idx + 1] - 2.0 * zc + z_values[idx - 1]) * inv_cs_sq
                } else {
                    0.0
                };
                let d2z_dy2 = if row > 0 && row + 1 < rows {
                    (z_values[idx + cols] - 2.0 * zc + z_values[idx - cols]) * inv_cs_sq
                } else {
                    0.0
                };

                store_slope_cell(
                    (&mut normals, &mut angles, &mut curvatures),
                    idx,
                    dz_dx,
                    dz_dy,
                    d2z_dx2,
                    d2z_dy2,
                );
            }
        }

        Self {
            normals,
            angles,
            curvatures,
            rows,
            cols,
            origin_x,
            origin_y,
            cell_size,
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Slope angle at cell indices (radians from horizontal).
    #[inline]
    pub fn angle_at(&self, row: usize, col: usize) -> f64 {
        self.angles[row * self.cols + col]
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Surface normal at cell indices.
    #[inline]
    pub fn normal_at(&self, row: usize, col: usize) -> V3 {
        self.normals[row * self.cols + col]
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Mean curvature at cell indices.
    #[inline]
    pub fn curvature_at(&self, row: usize, col: usize) -> f64 {
        self.curvatures[row * self.cols + col]
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Slope angle at world coordinates. Returns None for out-of-bounds.
    pub fn angle_at_world(&self, x: f64, y: f64) -> Option<f64> {
        let (row, col) = self.world_to_cell(x, y)?;
        Some(self.angles[row * self.cols + col])
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Mean curvature at world coordinates. Returns None for out-of-bounds.
    pub fn curvature_at_world(&self, x: f64, y: f64) -> Option<f64> {
        let (row, col) = self.world_to_cell(x, y)?;
        Some(self.curvatures[row * self.cols + col])
    }

    pub fn world_to_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let col_f = (x - self.origin_x) / self.cell_size;
        let row_f = (y - self.origin_y) / self.cell_size;
        if col_f < -0.5 || row_f < -0.5 {
            return None;
        }
        let col = col_f.round() as isize;
        let row = row_f.round() as isize;
        if col < 0 || row < 0 || col >= self.cols as isize || row >= self.rows as isize {
            return None;
        }
        Some((row as usize, col as usize))
    }
}

/// Classify cells into steep vs shallow based on threshold angle.
///
/// Returns a boolean grid (row-major): `true` = steep (angle >= threshold).
/// `threshold_deg` is in degrees from horizontal (0-90).
pub fn classify_steep_shallow(slope_map: &SlopeMap, threshold_deg: f64) -> Vec<bool> {
    let threshold_rad = threshold_deg.to_radians();
    slope_map
        .angles
        .iter()
        .map(|&a| a >= threshold_rad)
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_4;

    fn make_flat_z_grid(rows: usize, cols: usize) -> Vec<f64> {
        vec![0.0; rows * cols]
    }

    fn make_ramp_z_grid(rows: usize, cols: usize, cell_size: f64) -> Vec<f64> {
        // Linear ramp: z = x, so dz/dx = 1 → 45-degree slope
        let mut z = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                z[row * cols + col] = col as f64 * cell_size;
            }
        }
        z
    }

    fn make_dome_z_grid(rows: usize, cols: usize, cell_size: f64, radius: f64) -> Vec<f64> {
        // Hemisphere dome: z = sqrt(R^2 - x^2 - y^2), centered in grid
        let cx = (cols - 1) as f64 * cell_size * 0.5;
        let cy = (rows - 1) as f64 * cell_size * 0.5;
        let mut z = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let x = col as f64 * cell_size - cx;
                let y = row as f64 * cell_size - cy;
                let r_sq = radius * radius - x * x - y * y;
                z[row * cols + col] = if r_sq > 0.0 { r_sq.sqrt() } else { 0.0 };
            }
        }
        z
    }

    // ── SurfaceHeightmap tests ──────────────────────────────────────

    #[test]
    fn test_surface_heightmap_z_lookup() {
        let z_values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 2 rows × 3 cols
        let shm = SurfaceHeightmap {
            covered: vec![true; z_values.len()],
            z_values,
            rows: 2,
            cols: 3,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size: 1.0,
        };
        assert_eq!(shm.z_or_bbox_floor_at(0, 0), 1.0);
        assert_eq!(shm.z_or_bbox_floor_at(0, 2), 3.0);
        assert_eq!(shm.z_or_bbox_floor_at(1, 1), 5.0);
    }

    #[test]
    fn test_surface_heightmap_world_lookup() {
        let z_values = vec![10.0, 20.0, 30.0, 40.0];
        let shm = SurfaceHeightmap {
            covered: vec![true; z_values.len()],
            z_values,
            rows: 2,
            cols: 2,
            origin_x: 5.0,
            origin_y: 10.0,
            cell_size: 2.0,
        };
        assert_eq!(shm.z_or_bbox_floor_at_world(5.0, 10.0), 10.0);
        assert_eq!(shm.z_or_bbox_floor_at_world(7.0, 10.0), 20.0);
        assert_eq!(shm.z_or_bbox_floor_at_world(0.0, 0.0), f64::NEG_INFINITY); // out of bounds
    }

    #[test]
    fn test_surface_heightmap_min_z() {
        let shm = SurfaceHeightmap {
            z_values: vec![5.0, 2.0, 8.0, 1.0],
            covered: vec![true; 4],
            rows: 2,
            cols: 2,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size: 1.0,
        };
        assert_eq!(shm.min_z_or_bbox_floor(), 1.0);
    }

    // ── SlopeMap tests ──────────────────────────────────────────────

    #[test]
    fn test_slope_flat_surface() {
        let z = make_flat_z_grid(10, 10);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);
        for row in 0..10 {
            for col in 0..10 {
                let angle = sm.angle_at(row, col);
                assert!(
                    angle.abs() < 0.01,
                    "Flat surface angle at ({},{}) should be ~0, got {:.4}",
                    row,
                    col,
                    angle
                );
                let n = sm.normal_at(row, col);
                assert!(
                    (n.z - 1.0).abs() < 0.01,
                    "Flat normal Z at ({},{}) should be ~1, got {:.4}",
                    row,
                    col,
                    n.z
                );
            }
        }
    }

    #[test]
    fn test_slope_45_degree_ramp() {
        let z = make_ramp_z_grid(10, 10, 1.0);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);
        // Interior cells should have ~45 degree slope (dz/dx = 1)
        for row in 1..9 {
            for col in 1..9 {
                let angle = sm.angle_at(row, col);
                assert!(
                    (angle - FRAC_PI_4).abs() < 0.01,
                    "Ramp angle at ({},{}) should be ~45° ({:.4}), got {:.4} ({:.1}°)",
                    row,
                    col,
                    FRAC_PI_4,
                    angle,
                    angle.to_degrees()
                );
            }
        }
    }

    #[test]
    fn max_gradient_registers_single_cell_step() {
        // A 1 mm step confined to one cell boundary: z = 0 for col < 5,
        // z = 1 for col >= 5, at 1 mm cells. Central differences read
        // atan(1/2) ≈ 26.6° on the cells flanking the step; the
        // max-gradient stencil must read the full atan(1/1) = 45° on both.
        let rows = 10;
        let cols = 10;
        let mut z = vec![0.0f64; rows * cols];
        for row in 0..rows {
            for col in 5..cols {
                z[row * cols + col] = 1.0;
            }
        }
        let central = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, 1.0);
        let sharp = SlopeMap::from_z_grid_max_gradient(&z, rows, cols, 0.0, 0.0, 1.0);

        for &col in &[4usize, 5usize] {
            let c = central.angle_at(5, col).to_degrees();
            let s = sharp.angle_at(5, col).to_degrees();
            assert!(
                (c - 26.57).abs() < 0.5,
                "central diff at step col {col} should smear to ~26.6°, got {c:.1}°"
            );
            assert!(
                (s - 45.0).abs() < 0.5,
                "max-gradient at step col {col} should read 45°, got {s:.1}°"
            );
        }
        // Away from the step both stencils agree on flat.
        assert!(sharp.angle_at(5, 2) < 1e-9);
        assert!(sharp.angle_at(5, 8) < 1e-9);
    }

    #[test]
    fn max_gradient_matches_central_on_uniform_ramp() {
        // On a smooth (linear) surface forward, backward, and central
        // differences are identical — the stencils must agree everywhere,
        // including grid edges.
        let z = make_ramp_z_grid(10, 10, 1.0);
        let central = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);
        let sharp = SlopeMap::from_z_grid_max_gradient(&z, 10, 10, 0.0, 0.0, 1.0);
        for row in 0..10 {
            for col in 0..10 {
                let c = central.angle_at(row, col);
                let s = sharp.angle_at(row, col);
                assert!(
                    (c - s).abs() < 1e-9,
                    "stencils diverge on a uniform ramp at ({row},{col}): central {c:.6}, max-gradient {s:.6}"
                );
            }
        }
    }

    #[test]
    fn max_gradient_single_cell_trench_reads_steep_from_both_rims() {
        // A trench one cell wide (a lake shore seen from both sides):
        // every involved cell must read the full one-sided slope. Central
        // differences read 0° at the trench BOTTOM (left and right
        // neighbours are level with each other) — the max-gradient stencil
        // must not.
        let rows = 5;
        let cols = 9;
        let mut z = vec![0.0f64; rows * cols];
        for row in 0..rows {
            z[row * cols + 4] = -2.0;
        }
        let central = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, 1.0);
        let sharp = SlopeMap::from_z_grid_max_gradient(&z, rows, cols, 0.0, 0.0, 1.0);

        let expect = 2.0f64.atan().to_degrees(); // atan(h/cell) ≈ 63.4°
        for &col in &[3usize, 4usize, 5usize] {
            let s = sharp.angle_at(2, col).to_degrees();
            assert!(
                (s - expect).abs() < 0.5,
                "max-gradient at trench col {col} should read {expect:.1}°, got {s:.1}°"
            );
        }
        let bottom_central = central.angle_at(2, 4).to_degrees();
        assert!(
            bottom_central < 0.5,
            "central diff should read ~0° at the trench bottom (documenting the gap), got {bottom_central:.1}°"
        );
    }

    #[test]
    fn test_slope_hemisphere() {
        let z = make_dome_z_grid(20, 20, 1.0, 8.0);
        let sm = SlopeMap::from_z_grid(&z, 20, 20, 0.0, 0.0, 1.0);

        // Center should be nearly flat
        let center_angle = sm.angle_at(10, 10);
        assert!(
            center_angle < 15.0_f64.to_radians(),
            "Hemisphere center should be nearly flat, got {:.1}°",
            center_angle.to_degrees()
        );

        // Edge cells (near radius boundary) should be steep
        // Find a cell that's on the slope
        let edge_angle = sm.angle_at(3, 10); // Near the edge in Y direction
        assert!(
            edge_angle > 30.0_f64.to_radians(),
            "Hemisphere edge should be steep, got {:.1}°",
            edge_angle.to_degrees()
        );
    }

    #[test]
    fn test_curvature_convex() {
        // Dome (convex upward) should have negative d2z/dx2 at the peak
        // (surface curves downward from peak → concave in math terms,
        // but convex in the physical sense of a hill)
        let z = make_dome_z_grid(20, 20, 1.0, 8.0);
        let sm = SlopeMap::from_z_grid(&z, 20, 20, 0.0, 0.0, 1.0);
        let center_k = sm.curvature_at(10, 10);
        // For a dome, d2z/dx2 < 0 (concave down), so kappa < 0
        // This means "convex" in physical terms = negative curvature in our heightmap convention
        assert!(
            center_k < 0.0,
            "Dome peak curvature should be negative (convex surface), got {:.6}",
            center_k
        );
    }

    #[test]
    fn test_curvature_flat() {
        let z = make_flat_z_grid(10, 10);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);
        for row in 1..9 {
            for col in 1..9 {
                let k = sm.curvature_at(row, col);
                assert!(
                    k.abs() < 0.001,
                    "Flat surface curvature at ({},{}) should be ~0, got {:.6}",
                    row,
                    col,
                    k
                );
            }
        }
    }

    #[test]
    fn test_curvature_bowl() {
        // Bowl (concave): z = x^2 + y^2 → d2z/dx2 = 2, d2z/dy2 = 2 → kappa = 2
        let rows = 10;
        let cols = 10;
        let cs = 1.0;
        let cx = 4.5;
        let cy = 4.5;
        let mut z = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let x = col as f64 * cs - cx;
                let y = row as f64 * cs - cy;
                z[row * cols + col] = x * x + y * y;
            }
        }
        let sm = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cs);
        // Interior cells should have positive curvature (concave up = bowl)
        let k = sm.curvature_at(5, 5);
        assert!(
            (k - 2.0).abs() < 0.1,
            "Bowl curvature should be ~2.0, got {:.4}",
            k
        );
    }

    // ── Classification tests ────────────────────────────────────────

    #[test]
    fn test_classify_hemisphere() {
        let z = make_dome_z_grid(20, 20, 1.0, 8.0);
        let sm = SlopeMap::from_z_grid(&z, 20, 20, 0.0, 0.0, 1.0);
        let steep = classify_steep_shallow(&sm, 40.0);

        // Center should be shallow (not steep)
        assert!(
            !steep[10 * 20 + 10],
            "Hemisphere center should be classified as shallow"
        );

        // Count steep vs shallow
        let steep_count = steep.iter().filter(|&&s| s).count();
        let shallow_count = steep.iter().filter(|&&s| !s).count();
        assert!(
            steep_count > 0 && shallow_count > 0,
            "Should have both steep ({}) and shallow ({}) cells",
            steep_count,
            shallow_count
        );
    }

    #[test]
    fn test_classify_flat_all_shallow() {
        let z = make_flat_z_grid(10, 10);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);
        let steep = classify_steep_shallow(&sm, 10.0);
        assert!(
            steep.iter().all(|&s| !s),
            "Flat surface should be all shallow"
        );
    }

    #[test]
    fn test_classify_ramp_threshold() {
        // 45-degree ramp, threshold at 30 → all steep interior cells
        let z = make_ramp_z_grid(10, 10, 1.0);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);
        let steep_30 = classify_steep_shallow(&sm, 30.0);
        // Interior cells are ~45°, which is > 30° → steep
        let interior_steep = (1..9).all(|r| (1..9).all(|c| steep_30[r * 10 + c]));
        assert!(interior_steep, "45° ramp should be steep at 30° threshold");

        // Threshold at 50 → all shallow
        let steep_50 = classify_steep_shallow(&sm, 50.0);
        let interior_shallow = (1..9).all(|r| (1..9).all(|c| !steep_50[r * 10 + c]));
        assert!(
            interior_shallow,
            "45° ramp should be shallow at 50° threshold"
        );
    }

    // ── World coordinate accessor tests ─────────────────────────────

    #[test]
    fn test_slope_world_accessors() {
        let z = make_ramp_z_grid(10, 10, 2.0);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 2.0);

        // Interior point
        let angle = sm
            .angle_at_world(10.0, 10.0)
            .expect("interior point should return Some");
        assert!(
            (angle - FRAC_PI_4).abs() < 0.05,
            "Should be ~45°, got {:.1}°",
            angle.to_degrees()
        );

        // Out of bounds
        assert!(sm.angle_at_world(-100.0, -100.0).is_none());

        // Curvature accessor
        let k = sm.curvature_at_world(10.0, 10.0);
        assert!(k.is_some());
    }
    // ── Hole-mask tests (heights audit 2026-06-12, finding 3) ──────

    /// Flat ring plate at z=5: outer square (0,0)-(30,30) with a square
    /// hole (10,10)-(20,20). Open mesh — the hole has no geometry.
    fn ring_plate_mesh(z: f64) -> crate::mesh::TriangleMesh {
        use crate::geo::P3;
        let v = vec![
            P3::new(0.0, 0.0, z),
            P3::new(30.0, 0.0, z),
            P3::new(30.0, 30.0, z),
            P3::new(0.0, 30.0, z),
            P3::new(10.0, 10.0, z),
            P3::new(20.0, 10.0, z),
            P3::new(20.0, 20.0, z),
            P3::new(10.0, 20.0, z),
        ];
        let t = vec![
            [0u32, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        crate::mesh::TriangleMesh::from_raw(v, t)
    }

    #[test]
    fn hole_cells_are_uncovered_plate_cells_are_covered() {
        use crate::mesh::SpatialIndex;
        use crate::tool::FlatEndmill;

        let mesh = ring_plate_mesh(5.0);
        let index = SpatialIndex::build_auto(&mesh);
        let cutter = FlatEndmill::new(4.0, 10.0);
        // 31x31 grid at 1mm covering the plate; drop-cutter clamp floor 0.
        // Cutter radius 2: rim-riding reaches 2mm into the hole, while the
        // hole center (5mm from the rim) stays out of reach.
        let shm = SurfaceHeightmap::from_mesh(&mesh, &index, &cutter, 0.0, 0.0, 31, 31, 1.0, 0.0);

        // Plate cell: covered, on the surface.
        assert!(shm.covered_at(5, 5), "plate cell should be covered");
        assert!(
            (shm.z_or_bbox_floor_at(5, 5) - 5.0).abs() < 1e-6,
            "plate cell should read the surface at 5.0"
        );

        // Hole center: NOT covered (the vertical ray passes through the
        // hole). Its Z keeps the clamp floor — clearing strategies rely
        // on that to rough stock where there is no model surface; the
        // covered mask is what distinguishes "hole/no-surface" from
        // "real surface" for consumers that need the difference.
        assert!(
            !shm.covered_at(15, 15),
            "hole-center cell must be uncovered"
        );
        assert!(
            (shm.z_or_bbox_floor_at(15, 15) - 0.0).abs() < 1e-6,
            "hole cell keeps the clamp floor, got {}",
            shm.z_or_bbox_floor_at(15, 15)
        );

        // Hole rim within cutter radius: the cutter rides the rim, so the
        // CL height is the plate height even though the ray may miss —
        // exactly why coverage can't be derived from the Z value.
        assert!(
            (shm.z_or_bbox_floor_at(15, 19) - 5.0).abs() < 1e-6,
            "rim-riding cell reads the plate height, got {}",
            shm.z_or_bbox_floor_at(15, 19)
        );
    }
}
