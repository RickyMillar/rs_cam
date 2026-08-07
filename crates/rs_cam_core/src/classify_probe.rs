//! Samplers for the fine true-surface classification grid.
//!
//! # This module IS production (since M3 wave 7b, 2026-08-02)
//!
//! [`crate::finish_setup::build_classification_surface_with_policy_and_cancel`]
//! builds its height grid by calling [`sample_classification_grid`] with
//! [`ClassificationSampler::PRODUCTION`] — [`ClassificationSampler::TileRaster`],
//! the direct true-surface arm the M3 study recommended and the
//! 2026-08-02 checkpoint approved ("Adopt, COLUMNS-gated"). Every other
//! variant remains callable through
//! [`crate::finish_setup::build_classification_surface_with_sampler_and_cancel`]:
//! [`ClassificationSampler::DropCutterProbe`] is both the **fallback**
//! (flip `PRODUCTION` back and nothing else changes) and the **oracle** the
//! parity sentries score against.
//!
//! See `planning/review_2026-07-29/CLASSIFICATION_PERF_STUDY.md` — §9.1 for
//! why the tiled arm won over the two arms that produce identical labels
//! (parallelism correct by construction, cancellation bounded by a tile band,
//! no dependence on the index's per-cell registration being complete), and
//! §5.3 for the measurement that made the switch a product decision rather
//! than an optimisation.
//!
//! # What the two families compute, and why they differ
//!
//! The **probe** family samples the **tool-center (CL) surface of a Ø0.05 mm
//! ball**, not the model surface. For a facet of unit normal `n` the
//! drop-cutter puts the CL at `z_plane(x, y) + R·(1 − n.z)/n.z` — 10 µm above
//! the surface on a 45° face, 119 µm on 80°, 1.4 mm on 89°. The offset is
//! **slope-dependent**, so it does not cancel in a gradient: it adds gradient
//! of its own wherever slope varies, and at the production `cusp/4` cell
//! (0.125 mm for the shipped Ø1 tip) that is 8–20% of a cell in Z.
//!
//! The **direct** family evaluates the model surface itself. The two cannot
//! be bit-identical, and §5.3 measured which one is the outlier: shrinking the
//! probe 10× drives label disagreement to **zero on every fixture** and shrinks
//! the maximum Z difference by exactly 10× per 10× of radius — the
//! `R·(1 − n.z)/n.z` law and nothing else. The direct arms are not
//! approximating the surface; they *are* the surface, and the divergence is
//! the probe's own artefact.
//!
//! # Source lineage
//!
//! - `DropCutterProbe` — the shipped path, unmodified.
//! - `DropCutterProbeScratch` — same drop-cutter math, hoisted allocations.
//!   Standard scratch-buffer plumbing; the dedup-clear-by-walking-the-hits
//!   trick is the usual alternative to a generation-stamped array.
//! - `TriangleRaster` — scanline/bbox rasterisation of a triangle soup into a
//!   regular height grid, the classic "top surface z-buffer" from mesh
//!   voxelisation and terrain DEM burn-in. `max`-reduce per cell gives the
//!   topmost surface without needing an orientation test.
//! - `VerticalRay` — point-in-triangle ray casting against a uniform-grid
//!   broadphase; the gather-shaped dual of the scatter-shaped raster.
//! - `TileRaster` — the same raster decomposed into **disjoint output tiles**
//!   so the `max`-reduce needs no atomics; the standard way to parallelise a
//!   scatter (binning by output, as in tiled software rasterisers).

use crate::finish_setup::CLASSIFICATION_PROBE_DIAMETER_MM;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{QueryScratch, SpatialIndex, TriangleMesh};
use crate::slope::SurfaceHeightmap;
use crate::tool::{BallEndmill, CLPoint, MillingCutter};

/// How many cells one cancellation window covers.
///
/// The production classifier polls between 8192-cell rayon batches; every
/// candidate here keeps a window of the same order so the M3 acceptance gate
/// "cancellation latency remains bounded by a tile/chunk" is measured against
/// like for like.
pub const CANCEL_WINDOW_CELLS: usize = 8192;

/// Cancellation window for the scatter-shaped arm, whose loop runs over the
/// triangle soup rather than over cells.
///
/// Smaller than [`CANCEL_WINDOW_CELLS`] on purpose: one face can burn into an
/// unbounded number of cells (a single ground quad spans the whole grid), so
/// a face-counted window of the same nominal size would have a much longer
/// worst-case latency than a cell-counted one.
pub const CANCEL_WINDOW_FACES: usize = 1024;

/// Edge length, in cells, of one [`ClassificationSampler::TileRaster`] output
/// tile. 64 × 64 = 4096 cells, i.e. half a cancellation window, so a cancel
/// lands within one tile's work.
pub const TILE_EDGE_CELLS: usize = 64;

/// Which sampler fills the classification height grid.
///
/// [`Self::PRODUCTION`] is what `finish_setup` selects when a caller does not
/// name one; every variant is reachable through
/// [`crate::finish_setup::build_classification_surface_with_sampler_and_cancel`].
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationSampler {
    /// **The oracle and the fallback.** Tiny-ball drop-cutter at every cell,
    /// exactly as `finish_setup` shipped it before M3 wave 7b. Every other
    /// variant is scored against this by the parity sentries, and production
    /// returns to it by changing [`Self::PRODUCTION`] alone.
    DropCutterProbe,
    /// Candidate 0. The same drop-cutter contact math over the same triangle
    /// sets, with the per-query result `Vec` and dedup bitset hoisted out of
    /// the cell loop ([`SpatialIndex::query_into`]).
    ///
    /// **Bit-identical to [`Self::DropCutterProbe`] by construction**: the
    /// candidate triangle sets are equal index-for-index and the reduction
    /// (`max` of Z) is order-independent. It changes no geometry decision at
    /// all — which is why it is the low-risk arm.
    DropCutterProbeScratch,
    /// Candidate 1 (plan item 1). Direct top-surface triangle rasterisation:
    /// walk the faces, burn each one into the cells its XY bbox covers, keep
    /// the maximum Z per cell. Sequential scatter.
    TriangleRaster,
    /// Candidate 2 (plan item 2). Vertical-ray / topmost-face sampling: per
    /// cell, take the triangles registered in that one index cell, keep the
    /// highest plane Z among those whose XY projection contains the cell
    /// centre. No cutter-contact math, no dedup, no allocation.
    VerticalRay,
    /// Candidate 3 (plan item 3). [`Self::TriangleRaster`] parallelised over
    /// **disjoint output tiles**: each tile gathers its own candidate
    /// triangles from the spatial index and rasterises into its own slice, so
    /// no two threads ever write the same cell and no atomics are needed.
    ///
    /// **This is [`Self::PRODUCTION`].**
    #[default]
    TileRaster,
}

impl ClassificationSampler {
    /// The sampler production classification uses.
    ///
    /// One constant, one place: the M3 fallback documented in
    /// `CLASSIFICATION_PERF_STUDY.md` §9.1 is "set this back to
    /// [`Self::DropCutterProbe`]", and nothing else in the pipeline names a
    /// sampler. `ClassificationSampler::default()` is the same value, so a
    /// config field that omits the sampler gets production.
    pub const PRODUCTION: Self = Self::TileRaster;

    /// True when `self` is what production runs — used by serde to keep the
    /// sampler out of every project file that has not deliberately pinned a
    /// non-default one.
    #[must_use]
    pub const fn is_production(&self) -> bool {
        matches!(*self, Self::PRODUCTION)
    }

    /// Every sampler, oracle first.
    pub const ALL: [Self; 5] = [
        Self::DropCutterProbe,
        Self::DropCutterProbeScratch,
        Self::TriangleRaster,
        Self::VerticalRay,
        Self::TileRaster,
    ];

    /// Short label for tables and failure messages.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DropCutterProbe => "drop-cutter probe (shipped)",
            Self::DropCutterProbeScratch => "drop-cutter probe + scratch query",
            Self::TriangleRaster => "triangle raster",
            Self::VerticalRay => "vertical ray",
            Self::TileRaster => "tile raster (parallel)",
        }
    }

    /// True when the sampler evaluates the **CL surface of the probe ball**
    /// (what production classifies today) rather than the model surface.
    ///
    /// The two families cannot be equivalent to better than the probe's own
    /// offset, so a harness must not demand bit-equality across the boundary.
    #[must_use]
    pub const fn samples_probe_cl_surface(self) -> bool {
        match self {
            Self::DropCutterProbe | Self::DropCutterProbeScratch => true,
            Self::TriangleRaster | Self::VerticalRay | Self::TileRaster => false,
        }
    }
}

/// The regular grid a classification surface is sampled onto.
///
/// Deliberately a plain value rather than something derived inside the
/// sampler: the whole point of the harness is that every arm samples the
/// **same** cells, so the grid is decided once by the caller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassificationGridSpec {
    pub origin_x: f64,
    pub origin_y: f64,
    pub rows: usize,
    pub cols: usize,
    pub cell_size: f64,
    /// Floor every cell is clamped up to — the mesh bbox floor in production.
    /// Uncovered cells keep it; see [`crate::slope::GridZ`].
    pub min_z: f64,
}

impl ClassificationGridSpec {
    /// The grid `finish_setup`'s classification builder would produce for this
    /// mesh, cutter and cell size — same origin, same padding rule, same
    /// row/column arithmetic.
    ///
    /// Reproduced here rather than shared with `finish_setup` on purpose: this
    /// module must be deletable without touching production. The
    /// `grid_spec_matches_production_builder` sentry pins the two together.
    #[must_use]
    pub fn for_mesh(mesh: &TriangleMesh, cutter: &dyn MillingCutter, cell_size: f64) -> Self {
        let tool_radius = cutter.envelope_radius_mm();
        let bbox = &mesh.bbox;
        let origin_x = bbox.min.x - tool_radius;
        let origin_y = bbox.min.y - tool_radius;
        let extent_x = bbox.max.x + tool_radius;
        let extent_y = bbox.max.y + tool_radius;
        let cols = ((extent_x - origin_x) / cell_size).ceil() as usize + 1;
        let rows = ((extent_y - origin_y) / cell_size).ceil() as usize + 1;
        Self {
            origin_x,
            origin_y,
            rows,
            cols,
            cell_size,
            min_z: bbox.min.z,
        }
    }

    #[must_use]
    pub const fn total_cells(&self) -> usize {
        self.rows * self.cols
    }

    #[inline]
    #[must_use]
    fn cell_centre(&self, index: usize) -> (f64, f64) {
        let row = index / self.cols;
        let col = index % self.cols;
        (
            self.origin_x + col as f64 * self.cell_size,
            self.origin_y + row as f64 * self.cell_size,
        )
    }
}

/// Fill a classification height grid with the chosen sampler.
///
/// Returns the same [`SurfaceHeightmap`] shape production builds, so a caller
/// can drive `slope_map_max_gradient()` and the planner's decomposition over
/// any arm without special-casing.
pub fn sample_classification_grid(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    spec: ClassificationGridSpec,
    sampler: ClassificationSampler,
    cancel: &dyn CancelCheck,
) -> Result<SurfaceHeightmap, Cancelled> {
    match sampler {
        ClassificationSampler::DropCutterProbe => drop_cutter_probe(mesh, index, spec, cancel),
        ClassificationSampler::DropCutterProbeScratch => {
            drop_cutter_probe_scratch(mesh, index, spec, cancel)
        }
        ClassificationSampler::TriangleRaster => triangle_raster(mesh, spec, cancel),
        ClassificationSampler::VerticalRay => vertical_ray(mesh, index, spec, cancel),
        ClassificationSampler::TileRaster => tile_raster(mesh, index, spec, cancel),
    }
}

/// The probe the classification grid samples with — one place, so every arm
/// and every bench uses the identical cutter.
#[must_use]
pub fn classification_probe() -> BallEndmill {
    BallEndmill::new(CLASSIFICATION_PROBE_DIAMETER_MM, 1.0)
}

/// The shipped sampler with the probe **diameter** as a free variable.
///
/// The experiment this exists for: the shipped classifier and the direct
/// candidates disagree, and the disagreement has two possible causes —
/// sampling error in the candidate, or the probe's own slope-dependent CL
/// offset in the oracle. Shrinking the probe discriminates between them. If
/// the oracle converges onto the candidates as the diameter goes to zero, the
/// difference is the probe, measured rather than argued.
///
/// `d = CLASSIFICATION_PROBE_DIAMETER_MM` reproduces
/// [`ClassificationSampler::DropCutterProbe`] exactly.
pub fn sample_with_probe_diameter(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    spec: ClassificationGridSpec,
    probe_diameter_mm: f64,
    cancel: &dyn CancelCheck,
) -> Result<SurfaceHeightmap, Cancelled> {
    let probe = BallEndmill::new(probe_diameter_mm, 1.0);
    SurfaceHeightmap::from_mesh_with_cancel(
        mesh,
        index,
        &probe,
        spec.origin_x,
        spec.origin_y,
        spec.rows,
        spec.cols,
        spec.cell_size,
        spec.min_z,
        cancel,
    )
}

// ── Arm 0: the shipped path ─────────────────────────────────────────────

fn drop_cutter_probe(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    spec: ClassificationGridSpec,
    cancel: &dyn CancelCheck,
) -> Result<SurfaceHeightmap, Cancelled> {
    let probe = classification_probe();
    SurfaceHeightmap::from_mesh_with_cancel(
        mesh,
        index,
        &probe,
        spec.origin_x,
        spec.origin_y,
        spec.rows,
        spec.cols,
        spec.cell_size,
        spec.min_z,
        cancel,
    )
}

// ── Arm 0b: same math, hoisted allocations ──────────────────────────────

fn drop_cutter_probe_scratch(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    spec: ClassificationGridSpec,
    cancel: &dyn CancelCheck,
) -> Result<SurfaceHeightmap, Cancelled> {
    let probe = classification_probe();
    let radius = probe.radius();
    let cell = move |i: usize, scratch: &mut QueryScratch, tris: &mut Vec<usize>| -> (f64, bool) {
        let (x, y) = spec.cell_centre(i);
        let mut cl = CLPoint::new(x, y);
        index.query_into(x, y, radius, scratch, tris);
        for &idx in tris.iter() {
            // SAFETY: indices come from the SpatialIndex, which only stores
            // valid face indices (same contract as `point_drop_cutter`).
            #[allow(clippy::indexing_slicing)]
            probe.drop_cutter(&mut cl, &mesh.faces[idx]);
        }
        if cl.z < spec.min_z {
            cl.z = spec.min_z;
        }
        // Coverage predicate, verbatim from `dropcutter::sample_grid_cell`:
        // a zero-radius query, i.e. the single-cell fast path.
        index.query_into(x, y, 0.0, scratch, tris);
        let covered = tris.iter().any(|&idx| {
            // SAFETY: as above.
            #[allow(clippy::indexing_slicing)]
            mesh.faces[idx].contains_point_xy(x, y)
        });
        (cl.z, covered)
    };
    map_cells_parallel(spec, cancel, cell)
}

// ── The reduction ───────────────────────────────────────────────────────

/// The per-cell reduce every direct arm uses: keep `candidate` **only if it is
/// strictly greater** than what is already there.
///
/// A named function with its own test rather than an inline `>`, because the
/// study's §9.2 risk 4 is precisely that a later refactor "tidies" this into
/// `*current = current.max(candidate)` — which is a different function on two
/// inputs that reach it:
///
/// * **signed zero.** `(-0.0f64).max(0.0)` is `+0.0`; `0.0 > -0.0` is false,
///   so this keeps the `-0.0` that was seen first. Where a cell centre lands on
///   an edge shared by two coplanar triangles — all 386 profile-breakpoint
///   cells of the grooved-block fixture — the two candidates tie, and
///   *first-seen-wins* is what makes each arm internally deterministic
///   (`CLASSIFICATION_PERF_STUDY.md` §6.6).
/// * **NaN.** `f64::max` *ignores* a NaN operand, so it would let a degenerate
///   plane evaluation be silently replaced by, or silently replace, a real
///   height depending on which side it landed on. `>` is false against NaN in
///   both directions: a NaN never wins, and a NaN already in the cell is never
///   overwritten — it stays visible instead of being laundered.
#[inline]
fn keep_higher(current: &mut f64, candidate: f64) {
    if candidate > *current {
        *current = candidate;
    }
}

// ── Arm 1: direct top-surface triangle rasterisation ────────────────────

/// Burn `faces` into the tile `[row0, row1) × [col0, col1)` of `spec`.
///
/// `z` / `covered` are the tile's own buffers, `(row1-row0)*(col1-col0)` long.
/// Cells never touched keep `f64::NEG_INFINITY` / `false`.
#[allow(clippy::indexing_slicing)] // SAFETY: every index is derived from the tile bounds below
fn burn_faces_into_tile(
    mesh: &TriangleMesh,
    faces: impl Iterator<Item = usize>,
    spec: ClassificationGridSpec,
    (row0, row1): (usize, usize),
    (col0, col1): (usize, usize),
    z: &mut [f64],
    covered: &mut [bool],
) {
    let tile_cols = col1 - col0;
    let inv_cell = 1.0 / spec.cell_size;
    for face_idx in faces {
        let Some(face) = mesh.faces.get(face_idx) else {
            continue;
        };
        // Cell-centre lattice indices whose centre can lie inside the face's
        // XY bbox, **rounded outward by one cell on every side**.
        //
        // The tight `ceil`/`floor` bounds are wrong: `(min.x − origin) / cell`
        // is a division whose result can land a fraction of an ulp above an
        // integer, and `ceil` then skips a cell whose centre really is inside
        // the triangle. The gather-shaped arm has no such arithmetic — it asks
        // the index which triangles are near the point — so the two arms
        // disagreed on a handful of rim cells of the grooved block, which is
        // exactly what the cross-arm bit-identity test exists to catch.
        // `contains_point_xy` below is the real filter; these bounds only have
        // to be a superset, and a one-cell skirt costs a few extra
        // point-in-triangle tests per face.
        let c_lo = ((face.bbox.min.x - spec.origin_x) * inv_cell).floor() - 1.0;
        let c_hi = ((face.bbox.max.x - spec.origin_x) * inv_cell).ceil() + 1.0;
        let r_lo = ((face.bbox.min.y - spec.origin_y) * inv_cell).floor() - 1.0;
        let r_hi = ((face.bbox.max.y - spec.origin_y) * inv_cell).ceil() + 1.0;
        if !(c_lo.is_finite() && c_hi.is_finite() && r_lo.is_finite() && r_hi.is_finite()) {
            continue;
        }
        if c_hi < 0.0 || r_hi < 0.0 {
            continue;
        }
        let c_start = (c_lo.max(0.0) as usize).max(col0);
        let c_end = ((c_hi as usize) + 1).min(col1);
        let r_start = (r_lo.max(0.0) as usize).max(row0);
        let r_end = ((r_hi as usize) + 1).min(row1);
        if c_start >= c_end || r_start >= r_end {
            continue;
        }
        for row in r_start..r_end {
            let y = spec.origin_y + row as f64 * spec.cell_size;
            let out_base = (row - row0) * tile_cols;
            for col in c_start..c_end {
                let x = spec.origin_x + col as f64 * spec.cell_size;
                if !face.contains_point_xy(x, y) {
                    continue;
                }
                // Coverage is `contains_point_xy` ALONE — the exact predicate
                // `dropcutter::sample_grid_cell` uses. An exactly vertical
                // facet still counts as covered there, so it must here.
                let out = out_base + (col - col0);
                covered[out] = true;
                let Some(plane_z) = face.z_at_xy(x, y) else {
                    // Exactly vertical facet: projects to a line, contributes
                    // no height to a vertical ray. Same call `facet_drop`
                    // makes and the same answer it gets.
                    continue;
                };
                keep_higher(&mut z[out], plane_z);
            }
        }
    }
}

/// `NEG_INFINITY` sentinel → the `min_z` floor, matching the shipped
/// classifier's clamp for cells the probe never contacted.
fn finish_raster(
    spec: ClassificationGridSpec,
    mut z: Vec<f64>,
    covered: Vec<bool>,
) -> SurfaceHeightmap {
    for value in &mut z {
        if *value < spec.min_z {
            *value = spec.min_z;
        }
    }
    SurfaceHeightmap::from_parts(
        z,
        covered,
        spec.rows,
        spec.cols,
        spec.origin_x,
        spec.origin_y,
        spec.cell_size,
    )
}

fn triangle_raster(
    mesh: &TriangleMesh,
    spec: ClassificationGridSpec,
    cancel: &dyn CancelCheck,
) -> Result<SurfaceHeightmap, Cancelled> {
    let total = spec.total_cells();
    let mut z = vec![f64::NEG_INFINITY; total];
    let mut covered = vec![false; total];
    // Cancellation windows are measured in FACES here, not cells: this arm's
    // loop is over the triangle soup.
    let stride = CANCEL_WINDOW_FACES;
    let mut start = 0usize;
    while start < mesh.faces.len() {
        check_cancel(cancel)?;
        let end = (start + stride).min(mesh.faces.len());
        burn_faces_into_tile(
            mesh,
            start..end,
            spec,
            (0, spec.rows),
            (0, spec.cols),
            &mut z,
            &mut covered,
        );
        start = end;
    }
    Ok(finish_raster(spec, z, covered))
}

// ── Arm 2: vertical ray / topmost face ──────────────────────────────────

fn vertical_ray(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    spec: ClassificationGridSpec,
    cancel: &dyn CancelCheck,
) -> Result<SurfaceHeightmap, Cancelled> {
    let cell = move |i: usize, _s: &mut QueryScratch, _t: &mut Vec<usize>| -> (f64, bool) {
        let (x, y) = spec.cell_centre(i);
        let mut best = f64::NEG_INFINITY;
        let mut covered = false;
        for &idx in index.cell_triangles_at(x, y) {
            // SAFETY: indices come from the SpatialIndex.
            #[allow(clippy::indexing_slicing)]
            let face = &mesh.faces[idx];
            if !face.contains_point_xy(x, y) {
                continue;
            }
            // Coverage before the plane query, matching the oracle's
            // predicate: a vertical facet covers the cell but adds no height.
            covered = true;
            let Some(plane_z) = face.z_at_xy(x, y) else {
                continue;
            };
            keep_higher(&mut best, plane_z);
        }
        (best.max(spec.min_z), covered)
    };
    map_cells_parallel(spec, cancel, cell)
}

// ── Arm 3: tile-parallel rasterisation ──────────────────────────────────

#[allow(clippy::indexing_slicing)] // SAFETY: tile bounds are derived from spec.rows/cols
fn tile_raster(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    spec: ClassificationGridSpec,
    cancel: &dyn CancelCheck,
) -> Result<SurfaceHeightmap, Cancelled> {
    let tile_rows = spec.rows.div_ceil(TILE_EDGE_CELLS);
    let tile_cols = spec.cols.div_ceil(TILE_EDGE_CELLS);
    let tile_count = tile_rows * tile_cols;

    let one_tile = |tile: usize| -> (usize, usize, usize, usize, Vec<f64>, Vec<bool>) {
        let tr = tile / tile_cols;
        let tc = tile % tile_cols;
        let row0 = tr * TILE_EDGE_CELLS;
        let row1 = ((tr + 1) * TILE_EDGE_CELLS).min(spec.rows);
        let col0 = tc * TILE_EDGE_CELLS;
        let col1 = ((tc + 1) * TILE_EDGE_CELLS).min(spec.cols);
        let cells = (row1 - row0) * (col1 - col0);
        let mut z = vec![f64::NEG_INFINITY; cells];
        let mut covered = vec![false; cells];

        // Gather this tile's candidate triangles once, from the index, via a
        // query whose square covers the tile exactly.
        let x_lo = spec.origin_x + col0 as f64 * spec.cell_size;
        let x_hi = spec.origin_x + (col1 - 1) as f64 * spec.cell_size;
        let y_lo = spec.origin_y + row0 as f64 * spec.cell_size;
        let y_hi = spec.origin_y + (row1 - 1) as f64 * spec.cell_size;
        let cx = (x_lo + x_hi) * 0.5;
        let cy = (y_lo + y_hi) * 0.5;
        let radius = ((x_hi - x_lo).max(y_hi - y_lo)) * 0.5;
        let mut scratch = QueryScratch::new();
        let mut tris = Vec::new();
        index.query_into(cx, cy, radius, &mut scratch, &mut tris);

        burn_faces_into_tile(
            mesh,
            tris.iter().copied(),
            spec,
            (row0, row1),
            (col0, col1),
            &mut z,
            &mut covered,
        );
        (row0, row1, col0, col1, z, covered)
    };

    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    let tiles: Vec<_> = {
        use rayon::prelude::*;
        // One cancellation poll per tile row keeps latency bounded by a tile
        // band, matching the shipped batch granularity.
        let mut out = Vec::with_capacity(tile_count);
        for band in 0..tile_rows {
            check_cancel(cancel)?;
            let lo = band * tile_cols;
            let hi = ((band + 1) * tile_cols).min(tile_count);
            out.extend((lo..hi).into_par_iter().map(one_tile).collect::<Vec<_>>());
        }
        out
    };
    #[cfg(not(all(feature = "parallel", not(target_arch = "wasm32"))))]
    let tiles: Vec<_> = {
        let mut out = Vec::with_capacity(tile_count);
        for tile in 0..tile_count {
            if tile % 8 == 0 {
                check_cancel(cancel)?;
            }
            out.push(one_tile(tile));
        }
        out
    };

    let mut z = vec![f64::NEG_INFINITY; spec.total_cells()];
    let mut covered = vec![false; spec.total_cells()];
    for (row0, row1, col0, col1, tz, tcov) in tiles {
        let width = col1 - col0;
        for row in row0..row1 {
            let src = (row - row0) * width;
            let dst = row * spec.cols + col0;
            z[dst..dst + width].copy_from_slice(&tz[src..src + width]);
            covered[dst..dst + width].copy_from_slice(&tcov[src..src + width]);
        }
    }
    Ok(finish_raster(spec, z, covered))
}

// ── Shared cell-wise dispatch ───────────────────────────────────────────

/// Run a per-cell closure over the whole grid, batched for cancellation and
/// parallelised where available.
///
/// Batch size and poll placement mirror
/// [`SurfaceHeightmap::from_mesh_with_cancel`] exactly, so a timing difference
/// between an arm and the oracle is the cell work, not the scheduling.
fn map_cells_parallel<F>(
    spec: ClassificationGridSpec,
    cancel: &dyn CancelCheck,
    cell: F,
) -> Result<SurfaceHeightmap, Cancelled>
where
    F: Fn(usize, &mut QueryScratch, &mut Vec<usize>) -> (f64, bool) + Sync + Send,
{
    let total = spec.total_cells();
    let mut z_values = Vec::with_capacity(total);
    let mut covered = Vec::with_capacity(total);

    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        use rayon::prelude::*;
        let mut start = 0usize;
        while start < total {
            check_cancel(cancel)?;
            let end = (start + CANCEL_WINDOW_CELLS).min(total);
            let batch: Vec<(f64, bool)> = (start..end)
                .into_par_iter()
                .map_init(
                    || (QueryScratch::new(), Vec::<usize>::new()),
                    |(scratch, tris), i| cell(i, scratch, tris),
                )
                .collect();
            for (value, cov) in batch {
                z_values.push(value);
                covered.push(cov);
            }
            start = end;
        }
    }
    #[cfg(not(all(feature = "parallel", not(target_arch = "wasm32"))))]
    {
        let mut scratch = QueryScratch::new();
        let mut tris = Vec::new();
        for i in 0..total {
            if i % 64 == 0 {
                check_cancel(cancel)?;
            }
            let (value, cov) = cell(i, &mut scratch, &mut tris);
            z_values.push(value);
            covered.push(cov);
        }
    }

    Ok(SurfaceHeightmap::from_parts(
        z_values,
        covered,
        spec.rows,
        spec.cols,
        spec.origin_x,
        spec.origin_y,
        spec.cell_size,
    ))
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
    use crate::geo::P3;
    use crate::tool::TaperedBallEndmill;

    fn never() -> impl Fn() -> bool + Send + Sync {
        || false
    }

    /// Flat plate with a square hole — open mesh, so some interior cells have
    /// no surface at all.
    fn ring_plate(z: f64) -> TriangleMesh {
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
        TriangleMesh::from_raw(v, t)
    }

    #[test]
    fn the_reduction_keeps_the_first_of_a_tie_and_never_launders_a_nan() {
        // `CLASSIFICATION_PERF_STUDY.md` §9.2 risk 4: "pin the REDUCTION, not
        // just the result". Both assertions below FAIL if `keep_higher` is
        // rewritten as `*current = current.max(candidate)`, which is the exact
        // tidy-up the risk names.

        // Signed zero: an exact tie must leave the first-seen candidate in
        // place. `f64::max` would fold this to `+0.0`.
        let mut tie = -0.0f64;
        keep_higher(&mut tie, 0.0);
        assert!(
            tie.is_sign_negative(),
            "the reduction stopped keeping the FIRST of an exact tie; each arm's \
             internal determinism (study §6.6) rests on first-seen-wins"
        );
        // ...and the ordinary direction still works.
        let mut ordinary = -0.0f64;
        keep_higher(&mut ordinary, 1.0);
        assert_eq!(ordinary, 1.0);

        // NaN must never win a cell. `f64::max(NEG_INFINITY, NaN)` is
        // `NEG_INFINITY` too, so this direction alone would not discriminate —
        let mut fresh = f64::NEG_INFINITY;
        keep_higher(&mut fresh, f64::NAN);
        assert!(fresh.is_infinite() && fresh.is_sign_negative());
        // — but a NaN already in the cell must stay VISIBLE rather than being
        // silently replaced by the next real height. `f64::max(NaN, 5.0)` is
        // `5.0`, which would launder a degenerate plane evaluation into a
        // plausible surface.
        let mut poisoned = f64::NAN;
        keep_higher(&mut poisoned, 5.0);
        assert!(
            poisoned.is_nan(),
            "the reduction now launders a NaN cell into a real height"
        );
    }

    #[test]
    fn production_sampler_is_the_default_and_is_the_tile_raster() {
        // The one-line fallback lever the M3 checkpoint ruling names: flip
        // `PRODUCTION` and production classification returns to the shipped
        // drop-cutter path with nothing else moved. `Default` must track it,
        // because the serde field on `UnifiedFinishConfig` omits the sampler
        // whenever it is production.
        assert_eq!(
            ClassificationSampler::PRODUCTION,
            ClassificationSampler::TileRaster
        );
        assert_eq!(
            ClassificationSampler::default(),
            ClassificationSampler::PRODUCTION
        );
        assert!(ClassificationSampler::PRODUCTION.is_production());
        assert!(!ClassificationSampler::DropCutterProbe.is_production());
        // The production sampler reads the TRUE surface. If this ever flips,
        // `finish_setup`'s classification doc — and the §5.3 attribution the
        // switch rests on — are describing a different function.
        assert!(!ClassificationSampler::PRODUCTION.samples_probe_cl_surface());
    }

    #[test]
    fn the_grid_spec_holds_the_classification_grid_contract() {
        // `ClassificationGridSpec::for_mesh` is now the ONE copy of this
        // arithmetic — `finish_setup` calls it rather than repeating it — so
        // this test no longer polices a duplicate. It asserts the CONTRACT
        // instead, which is the thing that could still silently move:
        //
        //   * padding is one ENVELOPE radius on every side (physical sweep;
        //     `TOOL_SCALE_SEMANTICS.md` §8 row 2 — must stay envelope even
        //     though the CELL follows the cusp/tip scale), and
        //   * the cell is exactly what the resolution policy resolved to.
        let mesh = ring_plate(5.0);
        let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
        let policy = crate::finish_setup::FinishResolutionPolicy::cusp_quarter(&cutter, 0.4);
        let spec = ClassificationGridSpec::for_mesh(&mesh, &cutter, policy.cell_mm());

        let envelope = cutter.envelope_radius_mm();
        assert!((spec.origin_x - (mesh.bbox.min.x - envelope)).abs() < 1e-15);
        assert!((spec.origin_y - (mesh.bbox.min.y - envelope)).abs() < 1e-15);
        assert!((spec.cell_size - policy.cell_mm()).abs() < 1e-15);
        // The grid must REACH the far padded edge — one cell short leaves the
        // mask→polygon extractor without its non-contact margin ring.
        let far_x = spec.origin_x + (spec.cols - 1) as f64 * spec.cell_size;
        let far_y = spec.origin_y + (spec.rows - 1) as f64 * spec.cell_size;
        assert!(far_x >= mesh.bbox.max.x + envelope);
        assert!(far_y >= mesh.bbox.max.y + envelope);
        assert!((spec.min_z - mesh.bbox.min.z).abs() < 1e-15);
    }

    #[test]
    fn the_production_builder_runs_the_production_sampler() {
        // The M3 switch, asserted at the seam it happens at: what
        // `finish_setup` builds must be bit-identical to the PRODUCTION arm
        // run directly, and must NOT be the drop-cutter oracle (which is the
        // fallback — if the two ever coincide, either the fallback was taken
        // or this test has gone vacuous).
        let mesh = ring_plate(5.0);
        let index = SpatialIndex::build_auto(&mesh);
        let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
        let cancel = never();
        let policy = crate::finish_setup::FinishResolutionPolicy::cusp_quarter(&cutter, 0.4);
        let production = crate::finish_setup::build_classification_surface_with_policy_and_cancel(
            &mesh, &index, &cutter, policy, &cancel,
        )
        .unwrap();
        assert_eq!(
            production.sampler,
            crate::finish_setup::SurfaceSampler::Classification(ClassificationSampler::PRODUCTION),
            "the production classification grid must record which sampler built it"
        );
        assert!(
            production.sampler.is_true_surface(),
            "production classification must read the MODEL surface"
        );

        let spec = ClassificationGridSpec::for_mesh(&mesh, &cutter, policy.cell_mm());
        let direct = sample_classification_grid(
            &mesh,
            &index,
            spec,
            ClassificationSampler::PRODUCTION,
            &cancel,
        )
        .unwrap();
        assert_eq!(
            direct.z_or_bbox_floor_values(),
            production.heightmap.z_or_bbox_floor_values(),
            "the production builder is not running the production sampler"
        );
        assert_eq!(direct.covered_flags(), production.heightmap.covered_flags());

        // The fallback is still reachable, still different, and still tagged.
        let fallback = crate::finish_setup::build_classification_surface_with_sampler_and_cancel(
            &mesh,
            &index,
            &cutter,
            policy,
            ClassificationSampler::DropCutterProbe,
            &cancel,
        )
        .unwrap();
        assert_eq!(
            fallback.sampler,
            crate::finish_setup::SurfaceSampler::Classification(
                ClassificationSampler::DropCutterProbe
            )
        );
        assert!(!fallback.sampler.is_true_surface());
        // Coverage is the same predicate on both families, always.
        assert_eq!(
            fallback.heightmap.covered_flags(),
            production.heightmap.covered_flags()
        );
    }

    #[test]
    fn scratch_arm_is_bit_identical_to_the_shipped_arm() {
        let mesh = ring_plate(5.0);
        let index = SpatialIndex::build_auto(&mesh);
        let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
        let cancel = never();
        let spec = ClassificationGridSpec::for_mesh(&mesh, &cutter, 0.5);
        let oracle = sample_classification_grid(
            &mesh,
            &index,
            spec,
            ClassificationSampler::DropCutterProbe,
            &cancel,
        )
        .unwrap();
        let scratch = sample_classification_grid(
            &mesh,
            &index,
            spec,
            ClassificationSampler::DropCutterProbeScratch,
            &cancel,
        )
        .unwrap();
        assert_eq!(
            oracle.z_or_bbox_floor_values(),
            scratch.z_or_bbox_floor_values()
        );
        assert_eq!(oracle.covered_flags(), scratch.covered_flags());
    }

    #[test]
    fn raster_arms_agree_with_the_ray_arm_bit_for_bit() {
        // Scatter and gather must be the same function: same candidate set
        // after `contains_point_xy`, same `max` reduce. If these ever diverge
        // one of them has a bbox/lattice bug.
        //
        // Bit-equality holds on THIS fixture because no cell centre lands on
        // an edge shared by two triangles. Where one does — the grooved
        // block's profile breakpoints — the arms tie at ±0.0 or one ulp and
        // break it in visit order; `classification_strategy_m3`'s
        // `direct_arms_agree_to_the_last_ulp_and_on_every_label` is the
        // general statement, and it is the one to extend.
        let mesh = ring_plate(5.0);
        let index = SpatialIndex::build_auto(&mesh);
        let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
        let cancel = never();
        let spec = ClassificationGridSpec::for_mesh(&mesh, &cutter, 0.5);
        let ray = sample_classification_grid(
            &mesh,
            &index,
            spec,
            ClassificationSampler::VerticalRay,
            &cancel,
        )
        .unwrap();
        for arm in [
            ClassificationSampler::TriangleRaster,
            ClassificationSampler::TileRaster,
        ] {
            let got = sample_classification_grid(&mesh, &index, spec, arm, &cancel).unwrap();
            assert_eq!(
                ray.z_or_bbox_floor_values(),
                got.z_or_bbox_floor_values(),
                "{} disagrees with the vertical-ray arm on Z",
                arm.label()
            );
            assert_eq!(
                ray.covered_flags(),
                got.covered_flags(),
                "{} disagrees with the vertical-ray arm on coverage",
                arm.label()
            );
        }
    }

    #[test]
    fn every_arm_agrees_with_the_oracle_on_coverage() {
        // Coverage is the one thing that CAN be exact across the probe/true
        // surface boundary: it is the same `contains_point_xy` predicate over
        // the same triangles.
        let mesh = ring_plate(5.0);
        let index = SpatialIndex::build_auto(&mesh);
        let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
        let cancel = never();
        let spec = ClassificationGridSpec::for_mesh(&mesh, &cutter, 0.5);
        let oracle = sample_classification_grid(
            &mesh,
            &index,
            spec,
            ClassificationSampler::DropCutterProbe,
            &cancel,
        )
        .unwrap();
        for arm in ClassificationSampler::ALL {
            let got = sample_classification_grid(&mesh, &index, spec, arm, &cancel).unwrap();
            assert_eq!(
                oracle.covered_flags(),
                got.covered_flags(),
                "{} changed the coverage mask",
                arm.label()
            );
        }
    }

    #[test]
    fn every_arm_reports_cancellation() {
        let mesh = ring_plate(5.0);
        let index = SpatialIndex::build_auto(&mesh);
        let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
        let always = || true;
        let spec = ClassificationGridSpec::for_mesh(&mesh, &cutter, 0.5);
        for arm in ClassificationSampler::ALL {
            assert_eq!(
                sample_classification_grid(&mesh, &index, spec, arm, &always).err(),
                Some(Cancelled),
                "{} ignored a cancel that was already set",
                arm.label()
            );
        }
    }

    #[test]
    fn stacked_triangles_take_the_topmost_surface() {
        // Two overlapping plates, the upper one an overhang whose UNDERSIDE
        // also projects onto the same cells. The topmost surface is the upper
        // plate's top face and every arm must say so.
        let v = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(10.0, 10.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
            P3::new(2.0, 2.0, 4.0),
            P3::new(8.0, 2.0, 4.0),
            P3::new(8.0, 8.0, 4.0),
            P3::new(2.0, 8.0, 4.0),
        ];
        let t = vec![
            [0u32, 1, 2],
            [0, 2, 3],
            // upper shelf, wound face-down: orientation must not decide this
            [4, 6, 5],
            [4, 7, 6],
        ];
        let mesh = TriangleMesh::from_raw(v, t);
        let index = SpatialIndex::build_auto(&mesh);
        let cutter = BallEndmill::new(1.0, 10.0);
        let cancel = never();
        let spec = ClassificationGridSpec::for_mesh(&mesh, &cutter, 0.5);
        for arm in ClassificationSampler::ALL {
            let got = sample_classification_grid(&mesh, &index, spec, arm, &cancel).unwrap();
            let z = got.z_at_world(5.0, 5.0).z_or_bbox_floor().unwrap();
            assert!(
                (z - 4.0).abs() < 0.05,
                "{} read {z:.4} at the shelf centre; the topmost surface is 4.0",
                arm.label()
            );
        }
    }
}
