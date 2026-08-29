//! Drop-cutter algorithms for 3D surface finishing.
//!
//! Given a cutter at (x,y), find the maximum Z where it contacts the mesh
//! without gouging. The cutter is "dropped" along Z until first contact.

#[cfg(not(feature = "parallel"))]
use crate::interrupt::check_cancel;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::{CLPoint, MillingCutter, drop_cutter_can_contact};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Drop a single cutter at position (x, y) onto the mesh.
pub fn point_drop_cutter<C: MillingCutter + ?Sized>(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
) -> CLPoint {
    let mut cl = CLPoint::new(x, y);
    // Hoisted once per CL point rather than re-fetched per triangle: on a
    // `Box<dyn MillingCutter>` (the `ToolDefinition` wrapper) `radius()` is
    // itself an indirect call. Rejecting here rather than inside
    // `drop_cutter` also skips the per-triangle virtual dispatch entirely
    // for the triangles that cannot contribute — PERF_REVIEW G3.
    let envelope_radius = cutter.radius();
    let tri_indices = index.query(x, y, envelope_radius);

    for &idx in &tri_indices {
        // SAFETY: idx comes from SpatialIndex which only stores valid face indices
        #[allow(clippy::indexing_slicing)]
        let tri = &mesh.faces[idx];
        if !drop_cutter_can_contact(&cl, tri, envelope_radius) {
            continue;
        }
        cutter.drop_cutter(&mut cl, tri);
    }

    cl
}

/// Result of a batch drop-cutter operation: a grid of CL points.
///
/// `u_start`/`v_start` are the grid origin **in the rotated sampling
/// frame** (aligned with `direction_deg`), not necessarily world X/Y.
/// For `direction_deg == 0.0` (the only value any production call site
/// passes today — see `steep_shallow.rs`/`compute/execute.rs`) the rotated
/// frame coincides with world frame, so `u_start`/`v_start` read as world
/// mins. For any other angle they are rotated-frame minima and must be
/// inverse-rotated by `direction_deg` (as `batch_drop_cutter_with_cancel`
/// does internally) to recover world coordinates — do not reconstruct
/// world (x,y) from `row`/`col`/`u_start`/`v_start`/`x_step`/`y_step`
/// directly; read `points[i].x`/`.y` instead, which are always world-frame.
///
/// A grid may be a WINDOW onto a larger lattice
/// (`batch_drop_cutter_windowed_with_cancel`). A windowed grid is a smaller
/// grid whose `u_start`/`v_start` are **lattice-aligned to the parent** —
/// `parent_origin + index0 * step` — and whose points are bit-for-bit the
/// parent's. That alignment is the load-bearing property: consumers that
/// reconstruct geometry from the grid frame (`monotone_cells`) and consumers
/// that iterate `rows`/`cols` (`toolpath::raster_toolpath_from_grid`) both
/// stay consistent, because a window changes only WHICH lattice points
/// exist here, never where the lattice sits.
#[derive(Debug)]
pub struct DropCutterGrid {
    pub points: Vec<CLPoint>,
    pub rows: usize,
    pub cols: usize,
    /// Rotated-frame column origin (see struct docs — world X only when
    /// `direction_deg == 0.0`).
    pub u_start: f64,
    /// Rotated-frame row origin (see struct docs — world Y only when
    /// `direction_deg == 0.0`).
    pub v_start: f64,
    pub x_step: f64,
    pub y_step: f64,
    /// Raster scan direction, in degrees, that this grid was sampled at.
    pub direction_deg: f64,
}

impl DropCutterGrid {
    /// Get the CL point at grid position (row, col).
    pub fn get(&self, row: usize, col: usize) -> &CLPoint {
        // SAFETY: callers use row < self.rows and col < self.cols from grid iteration
        #[allow(clippy::indexing_slicing)]
        &self.points[row * self.cols + col]
    }
}

/// Run batch drop-cutter across a grid of points, parallelized with rayon.
///
/// Generates a regular grid covering the mesh XY extent (plus one cutter radius margin),
/// with the specified step-over distance.
// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
pub fn batch_drop_cutter<C: MillingCutter + ?Sized>(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
    step_over: f64,
    direction_deg: f64,
    min_z: f64,
) -> DropCutterGrid {
    let never_cancel = || false;
    batch_drop_cutter_with_cancel(
        mesh,
        index,
        cutter,
        step_over,
        direction_deg,
        min_z,
        &never_cancel,
    )
    .expect("non-cancellable drop-cutter should never be cancelled")
}

pub fn batch_drop_cutter_with_cancel<C: MillingCutter + ?Sized>(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
    step_over: f64,
    direction_deg: f64,
    min_z: f64,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<DropCutterGrid, Cancelled> {
    // The whole-mesh lattice IS the windowed lattice with no window. There
    // is deliberately no second transcription of the origin/step arithmetic
    // here: a windowed grid whose origin drifted from its parent's by one
    // ulp would re-phase the raster, and §0j priced phase alone at
    // 0.954–0.962× — a cost, silently.
    batch_drop_cutter_windowed_with_cancel(
        mesh,
        index,
        cutter,
        &LatticeSampling {
            step_over,
            direction_deg,
            min_z,
            window: None,
        },
        cancel,
    )
}

/// How one drop-cutter lattice is to be sampled.
///
/// Bundled rather than passed positionally because the windowed builder
/// would otherwise take eight arguments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LatticeSampling {
    pub step_over: f64,
    pub direction_deg: f64,
    pub min_z: f64,
    /// World-frame `[x0, y0, x1, y1]` box outside which lattice points are
    /// **not sampled**. `None` samples the whole mesh extent.
    ///
    /// This is a restriction on WHICH points of the whole-mesh lattice get
    /// sampled — never on where that lattice sits. See
    /// [`batch_drop_cutter_windowed_with_cancel`].
    pub window: Option<[f64; 4]>,
}

/// An inclusive index window into a parent lattice, expressed in the
/// PARENT's own `(row, col)` numbering plus the resulting extent.
///
/// `rows`/`cols` are `0` only when the parent itself is empty:
/// [`lattice_index_window`] never narrows a non-empty lattice to nothing,
/// deliberately (see its doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LatticeWindow {
    pub row0: usize,
    pub col0: usize,
    pub rows: usize,
    pub cols: usize,
}

/// Which parent lattice indices a sampling-frame box covers.
///
/// `frame_window` is `[u0, v0, u1, v1]` in the lattice's OWN sampling frame
/// (world XY for the axis-aligned path; forward-rotated `u`/`v` for the
/// rotated one), so the caller owns the frame transform and this function
/// owns only the index arithmetic.
///
/// Two properties are load-bearing and must not be "simplified" away:
///
/// * **The window only ever grows.** `floor` on the low side, `ceil` on the
///   high side, then clamp — never a rounding that could drop a lattice
///   point the parent would have sampled.
/// * **`row0` is always EVEN.** `toolpath::raster_toolpath_from_grid`
///   alternates each row's scan direction on `row % 2`, using the grid's
///   own row index. Starting a windowed grid on an odd parent row would
///   flip the serpentine phase of every row in it and re-order the emitted
///   points — a different toolpath from the same lattice. Snapping down
///   costs one extra sampled row and buys byte-identity.
///
/// Column indices need no such snap: within a row the scan visits columns
/// in ascending or descending index order either way, so a shifted `col0`
/// preserves the order of the points it contains.
pub(crate) fn lattice_index_window(
    parent_rows: usize,
    parent_cols: usize,
    origin: (f64, f64),
    step_over: f64,
    frame_window: Option<[f64; 4]>,
) -> LatticeWindow {
    let full = LatticeWindow {
        row0: 0,
        col0: 0,
        rows: parent_rows,
        cols: parent_cols,
    };
    let Some([u0, v0, u1, v1]) = frame_window else {
        return full;
    };
    let degenerate = parent_rows == 0
        || parent_cols == 0
        || step_over <= 0.0
        || !step_over.is_finite()
        || [u0, v0, u1, v1].iter().any(|c| !c.is_finite());
    if degenerate {
        // A window that cannot be reasoned about restricts nothing. The
        // whole-mesh lattice is always the CORRECT answer here — only the
        // expensive one — so an unusable window degrades to cost, never to
        // dropped coverage.
        return full;
    }
    let (u_origin, v_origin) = origin;

    /// Inclusive `[first, last]` parent index range covering every lattice
    /// point in `[low, high]`.
    ///
    /// Deliberately over-inclusive: `floor`/`ceil` widen to the enclosing
    /// lattice cell, and an interval that misses the lattice entirely
    /// collapses onto the nearest single index rather than to nothing. One
    /// wasted column is a cost; a dropped one is uncut material.
    fn index_range(
        low: f64,
        high: f64,
        origin: f64,
        step_over: f64,
        parent: usize,
    ) -> (usize, usize) {
        let last_parent = parent.saturating_sub(1) as f64;
        let first = ((low.min(high) - origin) / step_over)
            .floor()
            .clamp(0.0, last_parent);
        let last = ((high.max(low) - origin) / step_over)
            .ceil()
            .clamp(first, last_parent);
        (first as usize, last as usize)
    }

    let (col0, col1) = index_range(u0, u1, u_origin, step_over, parent_cols);
    let (row0, row1) = index_range(v0, v1, v_origin, step_over, parent_rows);
    // Serpentine phase — see this function's doc. `row0` is snapped DOWN, so
    // the window can only gain a row, never lose one.
    let row0 = row0 - (row0 % 2);
    LatticeWindow {
        row0,
        col0,
        rows: row1.saturating_sub(row0).saturating_add(1),
        cols: col1.saturating_sub(col0).saturating_add(1),
    }
}

/// The origin of a windowed lattice along one axis.
///
/// Returns the parent origin UNCHANGED at index 0 rather than
/// `origin + 0.0 * step`, so an unwindowed build is arithmetically inert —
/// `-0.0 + 0.0` is `+0.0`, and the whole point of this module's windowing is
/// that nothing about the lattice moves.
fn windowed_origin(parent_origin: f64, index0: usize, step_over: f64) -> f64 {
    if index0 == 0 {
        parent_origin
    } else {
        parent_origin + index0 as f64 * step_over
    }
}

/// Sample a drop-cutter lattice, optionally restricted to a world-frame
/// window.
///
/// **The window moves no lattice point.** Every sampled `(x, y)` is computed
/// from the PARENT's index (`col0 + col`, `row0 + row`) against the parent's
/// own origin, so a windowed grid's points are bit-for-bit the parent's — the
/// grid is simply smaller. `u_start`/`v_start` are therefore lattice-ALIGNED
/// to the parent (`parent_origin + index0 * step_over`, the same expression
/// that produces local index 0's coordinate), which is the property every
/// consumer of the grid frame depends on:
/// `monotone_cells::polygons_for_lattice_cell` reconstructs cells at
/// `u_start - x_step`, and `cells_select_same_lattice` compares them against
/// the same points.
///
/// The caller is responsible for the window being a superset of every
/// lattice point its downstream filters can emit — see
/// `unified_finish::region_sampling_window`, which carries that proof.
pub(crate) fn batch_drop_cutter_windowed_with_cancel<C: MillingCutter + ?Sized>(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
    sampling: &LatticeSampling,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<DropCutterGrid, Cancelled> {
    let LatticeSampling {
        step_over,
        direction_deg,
        min_z,
        window,
    } = *sampling;
    let r = cutter.radius();
    let bbox = mesh.bbox.expand_by(r);

    let angle_rad = direction_deg.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    // For angles that coincide exactly with the identity rotation (0°, or
    // 360° which is the same angle), and for 90°/180° (kept for backward
    // compatibility — see the struct-level doc caveat: those two do not
    // actually re-derive a rotated grid, they reuse the axis-aligned
    // rectangle), skip rotation overhead.
    let use_rotation = direction_deg.abs() > 0.01
        && (direction_deg - 90.0).abs() > 0.01
        && (direction_deg - 180.0).abs() > 0.01
        && (direction_deg - 360.0).abs() > 0.01;

    if !use_rotation {
        // Axis-aligned fast path (original behavior)
        let x_start = bbox.min.x;
        let x_end = bbox.max.x;
        let y_start = bbox.min.y;
        let y_end = bbox.max.y;

        let parent_cols = ((x_end - x_start) / step_over).ceil() as usize + 1;
        let parent_rows = ((y_end - y_start) / step_over).ceil() as usize + 1;

        // This path samples in WORLD x/y whatever angle it is labelled with
        // (0/90/180/360 all land here — see `use_rotation` above and the
        // struct doc), so the world-frame window is used unrotated. That is
        // honest here precisely because the sampling is honest here.
        let w = lattice_index_window(
            parent_rows,
            parent_cols,
            (x_start, y_start),
            step_over,
            window,
        );
        let (rows, cols) = (w.rows, w.cols);
        let (row0, col0) = (w.row0, w.col0);
        let u_start = windowed_origin(x_start, col0, step_over);
        let v_start = windowed_origin(y_start, row0, step_over);
        if rows == 0 || cols == 0 {
            return Ok(DropCutterGrid {
                points: Vec::new(),
                rows: 0,
                cols: 0,
                u_start,
                v_start,
                x_step: step_over,
                y_step: step_over,
                direction_deg,
            });
        }

        let (points, _covered) =
            batch_sample_grid(rows, cols, cancel, mesh, index, cutter, min_z, false, |i| {
                let row = i / cols;
                let col = i % cols;
                // PARENT indices: `(col0 + col)` collapses to `col` when
                // unwindowed, so this is the same arithmetic to the last bit.
                let x = x_start + (col0 + col) as f64 * step_over;
                let y = y_start + (row0 + row) as f64 * step_over;
                (x, y)
            })?;

        return Ok(DropCutterGrid {
            points,
            rows,
            cols,
            u_start,
            v_start,
            x_step: step_over,
            y_step: step_over,
            direction_deg,
        });
    }

    // Rotated grid: transform bbox corners into rotated frame to find bounds
    let corners = [
        (bbox.min.x, bbox.min.y),
        (bbox.max.x, bbox.min.y),
        (bbox.max.x, bbox.max.y),
        (bbox.min.x, bbox.max.y),
    ];

    let mut u_min = f64::INFINITY;
    let mut u_max = f64::NEG_INFINITY;
    let mut v_min = f64::INFINITY;
    let mut v_max = f64::NEG_INFINITY;

    for &(x, y) in &corners {
        // Rotate into aligned frame (forward rotation)
        let u = x * cos_a + y * sin_a;
        let v = -x * sin_a + y * cos_a;
        u_min = u_min.min(u);
        u_max = u_max.max(u);
        v_min = v_min.min(v);
        v_max = v_max.max(v);
    }

    let parent_cols = ((u_max - u_min) / step_over).ceil() as usize + 1;
    let parent_rows = ((v_max - v_min) / step_over).ceil() as usize + 1;

    // The window arrives in WORLD XY; this lattice is sampled in the rotated
    // frame, so all FOUR corners are forward-rotated and the frame-aligned
    // box is their min/max. Two corners would only be correct at a multiple
    // of 90°, which this branch never sees.
    let frame_window = window.map(|[x0, y0, x1, y1]| {
        let mut wu_min = f64::INFINITY;
        let mut wu_max = f64::NEG_INFINITY;
        let mut wv_min = f64::INFINITY;
        let mut wv_max = f64::NEG_INFINITY;
        for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
            let u = x * cos_a + y * sin_a;
            let v = -x * sin_a + y * cos_a;
            wu_min = wu_min.min(u);
            wu_max = wu_max.max(u);
            wv_min = wv_min.min(v);
            wv_max = wv_max.max(v);
        }
        [wu_min, wv_min, wu_max, wv_max]
    });
    let w = lattice_index_window(
        parent_rows,
        parent_cols,
        (u_min, v_min),
        step_over,
        frame_window,
    );
    let (rows, cols) = (w.rows, w.cols);
    let (row0, col0) = (w.row0, w.col0);
    let u_start = windowed_origin(u_min, col0, step_over);
    let v_start = windowed_origin(v_min, row0, step_over);
    if rows == 0 || cols == 0 {
        return Ok(DropCutterGrid {
            points: Vec::new(),
            rows: 0,
            cols: 0,
            u_start,
            v_start,
            x_step: step_over,
            y_step: step_over,
            direction_deg,
        });
    }

    let (points, _covered) =
        batch_sample_grid(rows, cols, cancel, mesh, index, cutter, min_z, false, |i| {
            let row = i / cols;
            let col = i % cols;
            // PARENT indices — see the fast path's identical note.
            let u = u_min + (col0 + col) as f64 * step_over;
            let v = v_min + (row0 + row) as f64 * step_over;
            // Inverse rotation: (u,v) -> (x,y)
            let x = u * cos_a - v * sin_a;
            let y = u * sin_a + v * cos_a;
            (x, y)
        })?;

    Ok(DropCutterGrid {
        points,
        rows,
        cols,
        u_start,
        v_start,
        x_step: step_over,
        y_step: step_over,
        direction_deg,
    })
}

/// Does the vertical ray at `(x, y)` pass through the mesh footprint?
///
/// The **exact** coverage predicate: a zero-radius spatial-index query plus
/// `Triangle::contains_point_xy`, with no grid, no cell and no rounding. It
/// is the one thing `point_drop_cutter` cannot tell you — the cutter has a
/// radius, so it reports a contact whenever it touches ANY nearby triangle,
/// including the *rim* of a mesh that does not cover that XY. A CL taken
/// there rests on the mesh's end edge and its tip sits `r − √(r² − d²)`
/// **below** the surface: a rim-riding overcut, or (over a hole) a trench
/// carved right around the part.
///
/// Every consumer that needs "is this point over real surface" should call
/// this rather than consulting a sampled coverage mask. A mask answers for
/// the nearest CELL, so it admits points up to half a cell outside the true
/// footprint — which is D-16.1 (`planning/review_2026-08-04/`
/// `FINISHING_OPEN_DEFECTS_EVIDENCE.md` §2.2): scallop's ring lift read a
/// 0.75 mm generation heightmap and cut 0.375 mm past the part edge.
///
/// Cost is one extra `index.query` at radius 0 — the single-cell fast path,
/// and cheap next to the `point_drop_cutter` call it accompanies.
pub fn point_is_over_mesh_xy(x: f64, y: f64, mesh: &TriangleMesh, index: &SpatialIndex) -> bool {
    index.query(x, y, 0.0).iter().any(|&idx| {
        // SAFETY: idx comes from SpatialIndex which only stores valid face indices
        #[allow(clippy::indexing_slicing)]
        mesh.faces[idx].contains_point_xy(x, y)
    })
}

/// One grid cell's CL point (clamped to `min_z`) and, when requested,
/// whether its vertical ray passes through a mesh triangle footprint —
/// the `covered` predicate `SurfaceHeightmap` needs and `DropCutterGrid`
/// does not. Computing it is an extra `index.query` per cell, so callers
/// that don't need it (the drop-cutter grid path) pass `with_coverage =
/// false` and pay nothing beyond the branch check.
#[inline]
pub(crate) fn sample_grid_cell<C: MillingCutter + ?Sized>(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
    min_z: f64,
    with_coverage: bool,
) -> (CLPoint, bool) {
    let mut cl = point_drop_cutter(x, y, mesh, index, cutter);
    if cl.z < min_z {
        cl.z = min_z;
    }
    let covered = with_coverage && point_is_over_mesh_xy(x, y, mesh, index);
    (cl, covered)
}

#[allow(clippy::too_many_arguments)]
/// Shared helper: compute CL points (and, optionally, per-cell mesh-coverage
/// flags) for a grid, using rayon parallelism when available.
///
/// `coord_fn` maps a flat index to (x, y) world coordinates. `with_coverage`
/// controls whether the "does the vertical ray pass through a triangle"
/// check (`SurfaceHeightmap::covered`) also runs per cell; `DropCutterGrid`
/// doesn't need it and passes `false`.
///
/// With the `parallel` feature, rows are processed in parallel via rayon;
/// cancellation is checked once per row (short-circuiting further rows once
/// cancelled) and again after the whole batch completes. In the sequential
/// fallback, cancellation is checked every 64 points.
pub(crate) fn batch_sample_grid<C: MillingCutter + ?Sized>(
    rows: usize,
    cols: usize,
    cancel: &(dyn CancelCheck + Sync),
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
    min_z: f64,
    with_coverage: bool,
    coord_fn: impl Fn(usize) -> (f64, f64) + Sync,
) -> Result<(Vec<CLPoint>, Vec<bool>), Cancelled> {
    let total = rows * cols;

    #[cfg(feature = "parallel")]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        let cancelled = AtomicBool::new(false);

        // Process by rows: each row is `cols` points and is independent.
        let (points, covered): (Vec<CLPoint>, Vec<bool>) = (0..rows)
            .into_par_iter()
            .flat_map(|row| {
                // Check cancellation once per row
                if cancelled.load(Ordering::Relaxed) || cancel.cancelled() {
                    cancelled.store(true, Ordering::Relaxed);
                    return Vec::new();
                }
                let start = row * cols;
                (start..start + cols)
                    .map(|i| {
                        let (x, y) = coord_fn(i);
                        sample_grid_cell(x, y, mesh, index, cutter, min_z, with_coverage)
                    })
                    .collect::<Vec<_>>()
            })
            .unzip();

        if cancelled.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        debug_assert_eq!(points.len(), total);
        Ok((points, covered))
    }

    #[cfg(not(feature = "parallel"))]
    {
        let mut points = Vec::with_capacity(total);
        let mut covered = Vec::with_capacity(total);
        for i in 0..total {
            if i % 64 == 0 {
                check_cancel(cancel)?;
            }
            let (x, y) = coord_fn(i);
            let (cl, cov) = sample_grid_cell(x, y, mesh, index, cutter, min_z, with_coverage);
            points.push(cl);
            covered.push(cov);
        }
        Ok((points, covered))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::geo::P3;
    use crate::mesh::{SpatialIndex, make_test_flat, make_test_hemisphere};
    use crate::tool::{BallEndmill, BullNoseEndmill, FlatEndmill, TaperedBallEndmill, VBitEndmill};

    // ── C2 follow-up: lattice windowing ────────────────────────────────
    //
    // The index arithmetic behind `batch_drop_cutter_windowed_with_cancel`.
    // The move-list byte-identity sentry lives with the production builder
    // (`unified_finish::tests::a_windowed_shallow_lattice_emits_*`); these
    // pin the three properties that sentry cannot make fire on demand.

    #[test]
    fn no_window_is_the_whole_parent_lattice() {
        let w = lattice_index_window(7, 9, (0.0, 0.0), 1.0, None);
        assert_eq!(
            w,
            LatticeWindow {
                row0: 0,
                col0: 0,
                rows: 7,
                cols: 9
            }
        );
    }

    #[test]
    fn a_window_snaps_its_first_row_down_to_an_even_parent_index() {
        // Rows 3..=4 on a unit lattice rooted at 0. Row 3 is ODD, and
        // starting there would invert the serpentine phase of every row in
        // the window — see `lattice_index_window`'s doc.
        let w = lattice_index_window(20, 20, (0.0, 0.0), 1.0, Some([2.0, 3.0, 4.0, 4.0]));
        assert_eq!(w.row0 % 2, 0, "row0 must be even, got {}", w.row0);
        assert_eq!(w.row0, 2);
        // Snapping DOWN, so the window still reaches its top row.
        assert!(w.row0 + w.rows > 4);
        // …and still covers the requested columns.
        assert!(w.col0 <= 2 && w.col0 + w.cols > 4);
    }

    #[test]
    fn a_window_only_ever_grows_and_stays_inside_the_parent() {
        // A window whose edges fall mid-cell must round OUT, not in.
        let w = lattice_index_window(10, 10, (0.0, 0.0), 1.0, Some([2.4, 2.4, 5.6, 5.6]));
        assert!(w.col0 <= 2, "low edge rounded in: col0 = {}", w.col0);
        assert!(
            w.col0 + w.cols > 6,
            "high edge rounded in: last col = {}",
            w.col0 + w.cols - 1
        );
        // Never past the parent's own extent.
        assert!(w.col0 + w.cols <= 10);
        assert!(w.row0 + w.rows <= 10);
    }

    #[test]
    fn a_window_off_the_lattice_degrades_to_cost_not_to_dropped_coverage() {
        // Wholly beyond the parent: collapses onto the nearest index rather
        // than to nothing. One wasted column is a cost; a dropped one is
        // uncut material.
        let w = lattice_index_window(10, 10, (0.0, 0.0), 1.0, Some([90.0, 90.0, 95.0, 95.0]));
        assert!(w.rows >= 1 && w.cols >= 1);
        assert!(w.col0 + w.cols <= 10 && w.row0 + w.rows <= 10);
        // An unusable window restricts nothing at all.
        let nan = lattice_index_window(10, 10, (0.0, 0.0), 1.0, Some([f64::NAN, 0.0, 1.0, 1.0]));
        assert_eq!(nan.rows, 10);
        assert_eq!(nan.cols, 10);
    }

    /// The windowed grid's sampled points are the parent's, to the BIT — the
    /// property the whole phase-preservation argument rests on.
    #[test]
    fn a_windowed_lattice_reuses_the_parents_own_points() {
        let mesh = make_test_flat(60.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 30.0);
        let never_cancel = || false;
        for direction_deg in [0.0_f64, 37.0] {
            let sampling = |window: Option<[f64; 4]>| LatticeSampling {
                step_over: 1.3,
                direction_deg,
                min_z: -50.0,
                window,
            };
            let parent = batch_drop_cutter_windowed_with_cancel(
                &mesh,
                &index,
                &tool,
                &sampling(None),
                &never_cancel,
            )
            .unwrap();
            let windowed = batch_drop_cutter_windowed_with_cancel(
                &mesh,
                &index,
                &tool,
                &sampling(Some([-10.0, -6.0, 8.0, 12.0])),
                &never_cancel,
            )
            .unwrap();
            assert!(
                windowed.points.len() < parent.points.len(),
                "window did not restrict anything at {direction_deg}°"
            );
            // Lattice-aligned origin: local (0,0) is a parent lattice point.
            let col0 = ((windowed.u_start - parent.u_start) / parent.x_step).round();
            let row0 = ((windowed.v_start - parent.v_start) / parent.y_step).round();
            assert_eq!(
                row0 as usize % 2,
                0,
                "row0 parity broken at {direction_deg}°"
            );
            for row in 0..windowed.rows {
                for col in 0..windowed.cols {
                    let got = windowed.get(row, col);
                    let want = parent.get(row + row0 as usize, col + col0 as usize);
                    assert_eq!(got.x.to_bits(), want.x.to_bits(), "x at {row},{col}");
                    assert_eq!(got.y.to_bits(), want.y.to_bits(), "y at {row},{col}");
                    assert_eq!(got.z.to_bits(), want.z.to_bits(), "z at {row},{col}");
                }
            }
        }
    }

    #[test]
    fn test_batch_drop_cutter_flat() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);

        assert!(grid.rows > 0);
        assert!(grid.cols > 0);

        // Points over the flat surface should be at z=0 (ball tip touches z=0 flat surface)
        let center_row = grid.rows / 2;
        let center_col = grid.cols / 2;
        let cl = grid.get(center_row, center_col);
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "Center CL.z = {}, expected ~0.0",
            cl.z
        );
    }

    #[test]
    fn test_point_drop_cutter_contacted_flag() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        // Point over the mesh should be contacted
        let cl = point_drop_cutter(0.0, 0.0, &mesh, &index, &tool);
        assert!(cl.contacted, "Point over mesh should be contacted");
        assert!(cl.z > f64::NEG_INFINITY, "Z should be finite");

        // Point far outside mesh footprint should not be contacted
        let cl_outside = point_drop_cutter(500.0, 500.0, &mesh, &index, &tool);
        assert!(
            !cl_outside.contacted,
            "Point far outside mesh should not be contacted"
        );
    }

    #[test]
    fn test_batch_drop_cutter_rotated_45() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        let grid_0 = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);
        let grid_45 = batch_drop_cutter(&mesh, &index, &tool, 5.0, 45.0, -100.0);

        // Both should produce valid grids
        assert!(grid_0.rows > 0 && grid_0.cols > 0);
        assert!(grid_45.rows > 0 && grid_45.cols > 0);

        // The 45° grid should have contacted points over the mesh
        let center = grid_45.get(grid_45.rows / 2, grid_45.cols / 2);
        assert!(
            center.contacted,
            "Center of 45° grid should contact flat mesh"
        );
        assert!(
            (center.z - 0.0).abs() < 0.5,
            "Center CL.z on flat = {}, expected ~0.0",
            center.z
        );
    }

    // --- Task E-dc: All 5 tool types on flat mesh ---

    #[test]
    fn test_flat_endmill_on_flat_mesh() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = FlatEndmill::new(10.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);
        assert!(grid.rows > 0 && grid.cols > 0);

        let cl = grid.get(grid.rows / 2, grid.cols / 2);
        assert!(cl.contacted, "FlatEndmill center should contact flat mesh");
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "FlatEndmill CL.z = {}, expected ~0.0 on flat mesh",
            cl.z
        );
    }

    #[test]
    fn test_ball_endmill_on_flat_mesh() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        let cl = point_drop_cutter(0.0, 0.0, &mesh, &index, &tool);
        assert!(cl.contacted);
        // Ball on flat: facet_drop yields z = surface_z + R*nz - R = 0
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "BallEndmill CL.z = {}, expected ~0.0",
            cl.z
        );
    }

    #[test]
    fn test_bullnose_endmill_on_flat_mesh() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);
        assert!(grid.rows > 0 && grid.cols > 0);

        let cl = grid.get(grid.rows / 2, grid.cols / 2);
        assert!(
            cl.contacted,
            "BullNoseEndmill center should contact flat mesh"
        );
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "BullNoseEndmill CL.z = {}, expected ~0.0 on flat mesh",
            cl.z
        );
    }

    #[test]
    fn test_vbit_endmill_on_flat_mesh() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);
        assert!(grid.rows > 0 && grid.cols > 0);

        let cl = grid.get(grid.rows / 2, grid.cols / 2);
        assert!(cl.contacted, "VBitEndmill center should contact flat mesh");
        // V-bit tip contact on flat surface: z = 0
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "VBitEndmill CL.z = {}, expected ~0.0 on flat mesh",
            cl.z
        );
    }

    #[test]
    fn test_tapered_ball_endmill_on_flat_mesh() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = TaperedBallEndmill::new(6.0, 10.0, 12.0, 30.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);
        assert!(grid.rows > 0 && grid.cols > 0);

        let cl = grid.get(grid.rows / 2, grid.cols / 2);
        assert!(
            cl.contacted,
            "TaperedBallEndmill center should contact flat mesh"
        );
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "TaperedBallEndmill CL.z = {}, expected ~0.0 on flat mesh",
            cl.z
        );
    }

    // --- All 5 tool types on hemisphere mesh ---

    #[test]
    fn test_all_tools_on_hemisphere_produce_valid_heights() {
        let hemisphere_r = 20.0;
        let mesh = make_test_hemisphere(hemisphere_r, 16);
        let index = SpatialIndex::build(&mesh, 10.0);

        let flat = FlatEndmill::new(10.0, 25.0);
        let ball = BallEndmill::new(10.0, 25.0);
        let bull = BullNoseEndmill::new(10.0, 2.0, 25.0);
        let vbit = VBitEndmill::new(10.0, 90.0, 25.0);
        let tapered = TaperedBallEndmill::new(6.0, 10.0, 12.0, 30.0);

        // Drop each tool at the apex (0,0)
        let tools: Vec<(&str, &dyn crate::tool::MillingCutter)> = vec![
            ("flat", &flat),
            ("ball", &ball),
            ("bullnose", &bull),
            ("vbit", &vbit),
            ("tapered_ball", &tapered),
        ];

        for (name, tool) in &tools {
            let cl = point_drop_cutter(0.0, 0.0, &mesh, &index, *tool);
            assert!(cl.contacted, "{} should contact hemisphere at apex", name);
            assert!(
                cl.z.is_finite() && cl.z > 0.0,
                "{} CL.z = {} should be finite and positive on hemisphere apex",
                name,
                cl.z
            );
            // At the apex, all tools should land near hemisphere_r
            assert!(
                (cl.z - hemisphere_r).abs() < 1.0,
                "{} CL.z = {}, expected ~{} at hemisphere apex",
                name,
                cl.z,
                hemisphere_r
            );
        }
    }

    // --- Edge cases ---

    #[test]
    fn test_single_triangle_mesh() {
        // Build a mesh from a single triangle
        let vertices = vec![
            P3::new(-10.0, -10.0, 5.0),
            P3::new(10.0, -10.0, 5.0),
            P3::new(0.0, 10.0, 5.0),
        ];
        let triangles = vec![[0, 1, 2]];
        let mesh = crate::mesh::TriangleMesh::from_raw(vertices, triangles);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = FlatEndmill::new(10.0, 25.0);

        // Point inside the single triangle
        let cl = point_drop_cutter(0.0, 0.0, &mesh, &index, &tool);
        assert!(cl.contacted, "Should contact single triangle");
        assert!(
            (cl.z - 5.0).abs() < 1e-6,
            "Flat endmill on single triangle at z=5 should give CL.z=5, got {}",
            cl.z
        );
    }

    #[test]
    fn test_near_boundary_grid_points() {
        // Mesh is from -50 to +50. Points near the boundary should still
        // be contacted (tool radius extends the query range).
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        // Point at the mesh boundary edge: tool extends 5mm past edge,
        // so at x=49 we should still have contact via vertex/edge
        let cl = point_drop_cutter(49.0, 0.0, &mesh, &index, &tool);
        assert!(
            cl.contacted,
            "Near-boundary point at x=49 should be contacted (tool radius=5)"
        );
        assert!(
            cl.z.is_finite(),
            "Near-boundary CL.z should be finite, got {}",
            cl.z
        );
    }

    #[test]
    fn test_vertical_edge_mesh() {
        // Mesh with a vertical wall: two triangles forming a 90-degree step
        let vertices = vec![
            P3::new(-20.0, -20.0, 0.0),
            P3::new(0.0, -20.0, 0.0),
            P3::new(0.0, -20.0, 10.0),
            P3::new(-20.0, -20.0, 10.0),
            P3::new(0.0, 20.0, 0.0),
            P3::new(0.0, 20.0, 10.0),
            // Top surface
            P3::new(-20.0, 20.0, 10.0),
            P3::new(20.0, 20.0, 10.0),
            P3::new(20.0, -20.0, 10.0),
        ];
        let triangles = vec![
            [0, 1, 2],
            [0, 2, 3],
            [1, 4, 5],
            [1, 5, 2],
            // Top surface
            [3, 2, 5],
            [3, 5, 6],
            [2, 8, 7],
            [2, 7, 5],
        ];
        let mesh = crate::mesh::TriangleMesh::from_raw(vertices, triangles);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 20.0);

        // Drop on the top surface, well inside
        let cl_top = point_drop_cutter(5.0, 0.0, &mesh, &index, &tool);
        assert!(cl_top.contacted, "Should contact top surface");
        assert!(
            cl_top.z > 5.0,
            "CL on top surface should be above 5, got {}",
            cl_top.z
        );

        // Drop at the vertical wall boundary: should still get valid results
        let cl_wall = point_drop_cutter(0.0, 0.0, &mesh, &index, &tool);
        assert!(
            cl_wall.z.is_finite(),
            "CL at vertical wall should be finite, got {}",
            cl_wall.z
        );
    }

    #[test]
    fn test_min_z_clamping() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        // With min_z = 5.0, all points should be at least 5.0
        let grid = batch_drop_cutter(&mesh, &index, &tool, 10.0, 0.0, 5.0);
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cl = grid.get(row, col);
                assert!(
                    cl.z >= 5.0 - 1e-10,
                    "CL.z = {} should be >= min_z=5.0 at ({}, {})",
                    cl.z,
                    row,
                    col
                );
            }
        }
    }

    // --- Cancellation ---

    #[test]
    fn test_cancellation_returns_error() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        // Cancel immediately
        let always_cancel = || true;
        let result =
            batch_drop_cutter_with_cancel(&mesh, &index, &tool, 5.0, 0.0, -100.0, &always_cancel);
        assert!(
            result.is_err(),
            "Immediately-cancelling predicate should return Err(Cancelled)"
        );
    }

    #[test]
    fn test_no_cancellation_succeeds() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        let never_cancel = || false;
        let result =
            batch_drop_cutter_with_cancel(&mesh, &index, &tool, 5.0, 0.0, -100.0, &never_cancel);
        assert!(
            result.is_ok(),
            "Never-cancelling predicate should return Ok"
        );
        let grid = result.unwrap();
        assert!(grid.rows > 0 && grid.cols > 0);
    }

    #[test]
    fn test_cancellation_after_some_work() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        // Cancel after being checked a few times
        let check_count = AtomicUsize::new(0);
        let cancel_after_3 = || {
            check_count.fetch_add(1, Ordering::Relaxed);
            check_count.load(Ordering::Relaxed) > 3
        };

        let result =
            batch_drop_cutter_with_cancel(&mesh, &index, &tool, 5.0, 0.0, -100.0, &cancel_after_3);

        // Depending on parallelism, it may or may not cancel in time, but
        // the cancel predicate should have been called at least once
        assert!(
            check_count.load(Ordering::Relaxed) > 0,
            "Cancel check should have been invoked at least once"
        );
        // The result should be either Ok or Err(Cancelled) - never a panic
        let _ = result;
    }

    // --- Grid accessor ---

    #[test]
    fn test_drop_cutter_grid_get_accessor() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = FlatEndmill::new(10.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 10.0, 0.0, -100.0);

        // Verify that get(row, col) matches the underlying flat array
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cl = grid.get(row, col);
                let flat_cl = &grid.points[row * grid.cols + col];
                assert_eq!(cl.x, flat_cl.x);
                assert_eq!(cl.y, flat_cl.y);
                assert_eq!(cl.z, flat_cl.z);
            }
        }
    }

    #[test]
    fn test_batch_grid_dimensions_match_step_over() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = FlatEndmill::new(10.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 10.0, 0.0, -100.0);

        // Grid should have rows*cols points
        assert_eq!(
            grid.points.len(),
            grid.rows * grid.cols,
            "Grid points count should be rows * cols"
        );

        // Step sizes should match
        assert!(
            (grid.x_step - 10.0).abs() < 1e-10,
            "x_step should be 10.0, got {}",
            grid.x_step
        );
        assert!(
            (grid.y_step - 10.0).abs() < 1e-10,
            "y_step should be 10.0, got {}",
            grid.y_step
        );
    }
}
