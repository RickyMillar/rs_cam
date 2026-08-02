//! The M4 scallop oracle — an **analytic tool-envelope** surface scorer.
//!
//! # Why this exists
//!
//! M4's research phase A says, in the plan's words: *"improve the oracle
//! first"*. Every scallop quality judgement on record has been made through
//! an instrument with a known artefact:
//!
//! | prior instrument | its artefact |
//! |---|---|
//! | Checkpoint B `cusp p50/p95` (`measured_cusp`) | infers cusp from the spacing between points on *adjacent rings* and pushes it through the FLAT-ground formula `h = R − √(R² − (d/2)²)`. It is a statement about the **tool-centre field**, never about stock. On a slope the same spacing leaves a different cusp, which is exactly the variable M4 is investigating. |
//! | Checkpoint B `residual_*` (`path_metrics`) | compares emitted move Z against a **sampled 0.05 mm reference grid**. `CHECKPOINT_B_EVIDENCE.md` §5.1 states the trap in full: scallop's own Z is an exact per-point drop-cutter query, so the residual column measures the *reference grid's* interpolation error at whatever points the path visits — and a denser path visits more sharp cells, so *denser paths score worse for free*. |
//! | `ScallopReport::uncut_core_mm2` | the cascade's own self-report of the polygon area it never reached. Hole-blind, and it can only see material the *ring cascade* knows it skipped — never material a ring passed over but did not remove. |
//! | dexel COLUMNS | trustworthy (the v3 campaign proved it three ways) but grid-quantised, stock-history dependent, and it costs a full simulation per arm. |
//!
//! None of those can answer M4's actual question, which is **"what cusp did
//! the surface actually get, where, and on what slope"**.
//!
//! # What this oracle computes
//!
//! The machined surface produced by a milling cutter following a path is the
//! lower envelope of the swept cutter. For a cutter whose profile is given by
//! [`MillingCutter::height_at_radius`] — the height of the cutting edge above
//! the tool TIP at horizontal distance `ρ` from the axis — a single cutter
//! position with its tip at `(cx, cy, cz)` leaves, at ground position
//! `(x, y)`:
//!
//! ```text
//! z_machined(x, y) = cz + height_at_radius(ρ),   ρ = hypot(x − cx, y − cy)
//! ```
//!
//! and the surface after the whole path is the **minimum** of that over every
//! cutter position (each pass can only ever lower the stock). That is exact
//! analytic geometry: no dexel grid, no Z quantisation, no probe ball, no
//! sampled reference field. Its only discretisations are
//!
//! 1. the XY grid the answer is reported on (`cell`), and
//! 2. how finely the path's chords are resampled into cutter positions
//!    (`path_step`),
//!
//! and [`validation`] measures both against closed-form ground truth.
//!
//! Writing it through `height_at_radius` rather than a hardcoded sphere is
//! deliberate: it is the same profile accessor C3 unified the width math on,
//! so the oracle scores a tapered ball with its real cone flank rather than a
//! ball twin's.
//!
//! # The residual sign convention, once
//!
//! ```text
//! residual = z_machined − z_true − stock_to_leave
//! ```
//!
//! * `residual > 0` — material LEFT above the requested surface. Small
//!   positive values across a field are the **cusp**; large connected
//!   positive values are **standing material**.
//! * `residual < 0` — material removed BELOW the requested surface: **gouge**
//!   / deep over-cut.
//! * `residual` is `NaN` where there is no model surface (open mesh, outside
//!   the footprint) or where no cutter position ever covered the cell — the
//!   latter is reported separately as [`OracleReport::untouched_mm2`], because
//!   "never visited" and "visited and left high" are different defects.
//!
//! # Reading "achieved cusp"
//!
//! On a field of passes at uniform spacing `d`, the residual over the
//! cross-pass coordinate `u ∈ [−d/2, d/2]` is `r(u) = R − √(R² − u²)`, so the
//! residual's *quantiles* have a closed form: `r_q = R − √(R² − (q·d/2)²)`,
//! i.e. `r_q ≈ q² · h_cusp` for the small cusps finishing works at. The peak
//! (`h_cusp`) is therefore the residual's **maximum**, and `p99 ≈ 0.98 ·
//! h_cusp` is a robust stand-in for it. [`OracleReport::achieved_cusp_um`] is
//! the p99, and `validation::flat_ground_cusp_quantiles_match_closed_form`
//! pins that 0.98 rather than assuming it.
//!
//! # What it is NOT
//!
//! It scores the **cutter envelope against the model**, so it says nothing
//! about stock history, rest material from a previous operation, or machine
//! dynamics. It is a surface-quality oracle, not a simulator. Where a claim
//! needs stock history, the dexel COLUMNS instrument
//! (`classification_columns_ab_m3.rs`) remains the right tool, and the M4
//! evidence run cross-checks the two on the headline comparison.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use rs_cam_core::classify_probe::{
    ClassificationGridSpec, ClassificationSampler, sample_classification_grid,
};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::toolpath::{MoveType, Toolpath};

// ---------------------------------------------------------------------------
// Grid
// ---------------------------------------------------------------------------

/// The regular XY grid the oracle reports on.
///
/// Decided once by the caller and shared by every arm of a comparison, exactly
/// like [`ClassificationGridSpec`] — the whole point is that two candidates
/// are scored on the *same cells*, so results are differenced cell-for-cell
/// rather than through a resampling step that would add its own error.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OracleGrid {
    pub origin_x: f64,
    pub origin_y: f64,
    pub rows: usize,
    pub cols: usize,
    pub cell: f64,
}

impl OracleGrid {
    /// A grid covering the mesh footprint padded by the cutter's envelope
    /// radius — the same padding rule `finish_setup` and
    /// `ClassificationGridSpec::for_mesh` use, so oracle cells line up with
    /// classification cells when the sizes match.
    #[must_use]
    pub fn for_mesh(mesh: &TriangleMesh, cutter: &dyn MillingCutter, cell: f64) -> Self {
        let r = cutter.envelope_radius_mm();
        let bbox = &mesh.bbox;
        let origin_x = bbox.min.x - r;
        let origin_y = bbox.min.y - r;
        let cols = ((bbox.max.x + r - origin_x) / cell).ceil() as usize + 1;
        let rows = ((bbox.max.y + r - origin_y) / cell).ceil() as usize + 1;
        Self {
            origin_x,
            origin_y,
            rows,
            cols,
            cell,
        }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.rows * self.cols
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rows == 0 || self.cols == 0
    }

    #[must_use]
    pub fn cell_area_mm2(&self) -> f64 {
        self.cell * self.cell
    }

    #[must_use]
    pub fn centre(&self, row: usize, col: usize) -> (f64, f64) {
        (
            self.origin_x + col as f64 * self.cell,
            self.origin_y + row as f64 * self.cell,
        )
    }

    #[must_use]
    pub const fn index(&self, row: usize, col: usize) -> usize {
        row * self.cols + col
    }

    /// The `ClassificationGridSpec` that samples exactly these cells, so the
    /// true-surface reference is built by the shipped M3 sampler rather than
    /// by a private copy of its arithmetic.
    #[must_use]
    pub const fn classification_spec(&self, min_z: f64) -> ClassificationGridSpec {
        ClassificationGridSpec {
            origin_x: self.origin_x,
            origin_y: self.origin_y,
            rows: self.rows,
            cols: self.cols,
            cell_size: self.cell,
            min_z,
        }
    }
}

// ---------------------------------------------------------------------------
// The envelope stamp kernel
// ---------------------------------------------------------------------------

/// Precomputed cutter profile as a grid stamp: for every cell offset within
/// the cutter's envelope radius, the height of the cutting edge above the tip.
///
/// Built once per (cutter, cell) so the inner loop is a flat `min` scatter.
#[derive(Debug, Clone)]
pub struct StampKernel {
    /// `(d_row, d_col, height_above_tip_mm)`.
    offsets: Vec<(i32, i32, f64)>,
    pub radius_mm: f64,
    pub cell: f64,
}

impl StampKernel {
    /// Sample the cutter profile onto a grid of pitch `cell`, out to
    /// `radius_cap` (defaults to the cutter's envelope radius).
    ///
    /// `radius_cap` exists because a tapered ball's envelope is its SHANK: a
    /// Ø6 shank on a Ø1 tip stamps 11 300 cells at 0.05 mm pitch, of which the
    /// outer ring only ever matters when the flank is genuinely cutting. A
    /// harness that caps the radius is trading exactness for speed and must
    /// say so; `validation::taper_flank_survives_the_radius_cap` measures the
    /// price on the fixture that has flank contact.
    #[must_use]
    pub fn new(cutter: &dyn MillingCutter, cell: f64, radius_cap: Option<f64>) -> Self {
        let radius = radius_cap.unwrap_or_else(|| cutter.envelope_radius_mm());
        let span = (radius / cell).ceil() as i32;
        let mut offsets = Vec::new();
        for dr in -span..=span {
            for dc in -span..=span {
                let dx = f64::from(dc) * cell;
                let dy = f64::from(dr) * cell;
                let rho = dx.hypot(dy);
                if rho > radius {
                    continue;
                }
                // `height_at_radius` is the profile accessor C3 unified the
                // width math on; it returns None past the envelope.
                if let Some(h) = cutter.height_at_radius(rho) {
                    offsets.push((dr, dc, h));
                }
            }
        }
        Self {
            offsets,
            radius_mm: radius,
            cell,
        }
    }

    #[must_use]
    pub fn cells(&self) -> usize {
        self.offsets.len()
    }
}

// ---------------------------------------------------------------------------
// The oracle
// ---------------------------------------------------------------------------

/// Slope class, matching the repo's shipped band thresholds
/// (`checkpoint_b_resolution_ab::class_of`, `finish_planner`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SlopeBand {
    Shallow,
    MidSteep,
    VerySteep,
}

impl SlopeBand {
    pub const ALL: [Self; 3] = [Self::Shallow, Self::MidSteep, Self::VerySteep];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Shallow => "shallow <45°",
            Self::MidSteep => "mid-steep 45–75°",
            Self::VerySteep => "very-steep >75°",
        }
    }

    #[must_use]
    pub fn of_angle_deg(deg: f64) -> Self {
        if deg >= 75.0 {
            Self::VerySteep
        } else if deg >= 45.0 {
            Self::MidSteep
        } else {
            Self::Shallow
        }
    }
}

/// A scored surface: the true model surface, the analytic machined envelope,
/// and the per-cell slope band the residual is attributed to.
pub struct EnvelopeOracle {
    pub grid: OracleGrid,
    /// Model surface Z per cell; `NaN` where the vertical ray hits nothing.
    pub true_z: Vec<f64>,
    /// Machined envelope Z per cell; `f64::INFINITY` where no cutter position
    /// ever covered the cell.
    pub machined_z: Vec<f64>,
    /// Slope angle (degrees) of the TRUE surface, from central differences on
    /// `true_z`. `NaN` where the stencil crosses uncovered ground.
    pub slope_deg: Vec<f64>,
    /// How many cutter positions the path was resampled into — the oracle's
    /// second discretisation, reported so a convergence claim can be checked.
    pub path_samples: usize,
    pub stamp_cells: usize,
}

impl EnvelopeOracle {
    /// Build the true-surface reference with the shipped M3 tile-raster
    /// sampler.
    ///
    /// M3 (`0711568`) made this 187× cheaper than the tiny-ball drop-cutter it
    /// replaced, which is precisely why the oracle can afford a grid fine
    /// enough to resolve a 20 µm cusp. It samples the MODEL surface, not a
    /// probe's CL offset surface — `ClassificationSampler::samples_probe_cl_surface`
    /// is false for it — so it is the right reference for "what did the stock
    /// get" as opposed to "where would a probe sit".
    #[must_use]
    pub fn true_surface_from_mesh(
        grid: OracleGrid,
        mesh: &TriangleMesh,
        index: &SpatialIndex,
    ) -> Vec<f64> {
        let spec = grid.classification_spec(mesh.bbox.min.z);
        let never = || false;
        let hm = sample_classification_grid(
            mesh,
            index,
            spec,
            ClassificationSampler::TileRaster,
            &never,
        )
        .expect("never-cancel sampler cannot be cancelled");
        let mut out = vec![f64::NAN; grid.len()];
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                // `GridZ::covered()` is `Some` only where the vertical ray
                // actually passed through the mesh — the C2 distinction the
                // oracle depends on, since an uncovered cell carries the bbox
                // FLOOR and would otherwise read as a huge fake cusp.
                if let Some(z) = hm.z_at(row, col).covered() {
                    out[grid.index(row, col)] = z;
                }
            }
        }
        out
    }

    /// Build the true-surface reference from a closed-form height function —
    /// the ground truth the sampler itself is validated against.
    #[must_use]
    pub fn true_surface_analytic(grid: OracleGrid, z: impl Fn(f64, f64) -> f64) -> Vec<f64> {
        let mut out = vec![f64::NAN; grid.len()];
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let (x, y) = grid.centre(row, col);
                out[grid.index(row, col)] = z(x, y);
            }
        }
        out
    }

    /// Score a toolpath against a true surface.
    ///
    /// `path_step` is the spacing cutting chords are resampled at. It must be
    /// no coarser than `grid.cell` or the envelope will show phantom ridges
    /// between samples; `validation::path_step_convergence` measures where it
    /// stops mattering.
    #[must_use]
    pub fn score(
        grid: OracleGrid,
        true_z: Vec<f64>,
        toolpath: &Toolpath,
        kernel: &StampKernel,
        path_step: f64,
    ) -> Self {
        // Pass 1: resample the cutting chords into cutter positions.
        //
        // The tool sweeps continuously along a chord, so this is where chord
        // SAG becomes visible: a chord that cuts a corner off a convex ridge
        // gouges, and stamping the intermediate positions is what makes the
        // gouge appear at all.
        let step = path_step.max(1e-6);
        let mut samples: Vec<P3> = Vec::new();
        let mut prev: Option<P3> = None;
        for mv in &toolpath.moves {
            let cutting = !matches!(mv.move_type, MoveType::Rapid);
            let target = mv.target;
            if let (Some(a), true) = (prev, cutting) {
                let d = ((target.x - a.x).powi(2)
                    + (target.y - a.y).powi(2)
                    + (target.z - a.z).powi(2))
                .sqrt();
                let n = (d / step).ceil().max(1.0) as usize;
                for i in 1..=n {
                    let t = i as f64 / n as f64;
                    samples.push(P3::new(
                        a.x + (target.x - a.x) * t,
                        a.y + (target.y - a.y) * t,
                        a.z + (target.z - a.z) * t,
                    ));
                }
            } else if cutting {
                samples.push(target);
            }
            prev = Some(target);
        }
        let path_samples = samples.len();

        // Pass 2: stamp, parallel over DISJOINT row bands.
        //
        // The reduction is a `min` scatter, so two threads writing the same
        // cell would need an atomic. Partitioning the OUTPUT instead removes
        // the question — the same trick M3's winning tile-raster classifier
        // uses. Samples are sorted by row so each band binary-searches the
        // slice that can reach it (its own rows, dilated by the stamp span)
        // rather than re-scanning every sample.
        let span = (kernel.radius_mm / grid.cell).ceil() as i64;
        let row_of = |p: &P3| ((p.y - grid.origin_y) / grid.cell).round() as i64;
        samples.sort_by_key(|p| row_of(p));
        let sample_rows: Vec<i64> = samples.iter().map(row_of).collect();

        let mut machined_z = vec![f64::INFINITY; grid.len()];
        let band_rows = (grid.rows / rayon::current_num_threads().max(1)).max(1);
        machined_z
            .par_chunks_mut(band_rows * grid.cols)
            .enumerate()
            .for_each(|(band, out)| {
                let r0 = (band * band_rows) as i64;
                let rows_here = out.len() / grid.cols;
                let r1 = r0 + rows_here as i64;
                let lo = sample_rows.partition_point(|&r| r < r0 - span);
                let hi = sample_rows.partition_point(|&r| r < r1 + span);
                for p in &samples[lo..hi] {
                    let col0 = ((p.x - grid.origin_x) / grid.cell).round() as i64;
                    let row0 = ((p.y - grid.origin_y) / grid.cell).round() as i64;
                    for &(dr, dc, h) in &kernel.offsets {
                        let r = row0 + i64::from(dr);
                        let c = col0 + i64::from(dc);
                        if r < r0 || r >= r1 || c < 0 || c >= grid.cols as i64 {
                            continue;
                        }
                        let idx = ((r - r0) as usize) * grid.cols + (c as usize);
                        let z = p.z + h;
                        if z < out[idx] {
                            out[idx] = z;
                        }
                    }
                }
            });

        let slope_deg = slope_from_heights(&grid, &true_z);
        Self {
            grid,
            true_z,
            machined_z,
            slope_deg,
            path_samples,
            stamp_cells: kernel.cells(),
        }
    }

    /// `z_machined − z_true − stock_to_leave` per cell; `NaN` where either
    /// side is undefined.
    #[must_use]
    pub fn residuals(&self, stock_to_leave: f64) -> Vec<f64> {
        self.true_z
            .iter()
            .zip(&self.machined_z)
            .map(|(&t, &m)| {
                if t.is_nan() || !m.is_finite() {
                    f64::NAN
                } else {
                    m - t - stock_to_leave
                }
            })
            .collect()
    }

    /// The best this cutter could possibly do on this surface: the envelope
    /// of the cutter dropped at **every cell of the oracle grid**.
    ///
    /// A ball of radius `R` cannot enter a valley narrower than `R`, so on
    /// relief finer than the tool a large residual is *geometry*, not a
    /// stepover defect — no ring placement can remove it. Without this floor
    /// a harness reads tool reach as algorithm quality and ranks candidates
    /// on a number none of them controls.
    ///
    /// Returned as a residual field on the same cells, so it subtracts
    /// directly from any arm's residual.
    #[must_use]
    pub fn tool_reach_floor(
        grid: OracleGrid,
        true_z: &[f64],
        mesh: &TriangleMesh,
        index: &SpatialIndex,
        cutter: &dyn MillingCutter,
        kernel: &StampKernel,
    ) -> Vec<f64> {
        let mut tips: Vec<P3> = Vec::with_capacity(grid.len());
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                // Only over ground that EXISTS. `point_drop_cutter` reports a
                // finite contact for a cell past the model edge too — the
                // cutter has radius and rides the rim — and stamping that
                // position drops a spuriously low envelope onto its
                // neighbours, which reads as the ideal path gouging. It is
                // the same rim-contact trap `ring_to_3d` guards with the
                // coverage mask.
                if true_z[grid.index(row, col)].is_nan() {
                    continue;
                }
                let (x, y) = grid.centre(row, col);
                let cl = rs_cam_core::dropcutter::point_drop_cutter(x, y, mesh, index, cutter);
                if cl.z.is_finite() {
                    tips.push(P3::new(x, y, cl.z));
                }
            }
        }
        let mut tp = Toolpath::new();
        for (i, p) in tips.iter().enumerate() {
            if i == 0 {
                tp.rapid_to(*p);
            } else {
                // Each cell is stamped as its own point; consecutive cells are
                // NOT joined into a swept chord, because this is the envelope
                // of every reachable position, not a path anyone would run.
                tp.rapid_to(*p);
                tp.feed_to(*p, 1000.0);
            }
        }
        let ideal = Self::score(grid, true_z.to_vec(), &tp, kernel, grid.cell);
        ideal
            .machined_z
            .iter()
            .zip(true_z)
            .map(|(&m, &t)| {
                if t.is_nan() || !m.is_finite() {
                    f64::NAN
                } else {
                    m - t
                }
            })
            .collect()
    }

    /// The residual field **for rendering**: identical to [`Self::residuals`]
    /// except that a cell no cutter position ever reached is `+INFINITY`
    /// rather than `NaN`, so [`render_field`] can colour it differently from
    /// "there is no model surface here".
    ///
    /// The first M4 diff maps did not have this and drew untouched cells
    /// black, which is the same colour as off-model — the exact confusion the
    /// oracle exists to prevent, reintroduced at the last step.
    #[must_use]
    pub fn residual_map(&self, stock_to_leave: f64) -> Vec<f64> {
        self.true_z
            .iter()
            .zip(&self.machined_z)
            .map(|(&t, &m)| {
                if t.is_nan() {
                    f64::NAN
                } else if !m.is_finite() {
                    f64::INFINITY
                } else {
                    m - t - stock_to_leave
                }
            })
            .collect()
    }

    /// Score the whole surface.
    ///
    /// `dial_mm` is the COMMANDED cusp height (`ScallopParams::scallop_height`).
    /// `standing_mult` is how many dials above target counts as *standing
    /// material* rather than cusp — the default the M4 harness uses is 5.0,
    /// chosen so a pass that merely overshot its cusp target by 4× is still
    /// reported as a cusp defect, not as uncut stock.
    #[must_use]
    pub fn report(&self, params: OracleParams) -> OracleReport {
        let resid = self.residuals(params.stock_to_leave);
        let cell_area = self.grid.cell_area_mm2();

        let mut scored: Vec<f64> = Vec::new();
        let mut scored_normal: Vec<f64> = Vec::new();
        let mut untouched_cells = 0usize;
        let mut standing_cells = 0usize;
        let mut gouge_cells = 0usize;
        let mut deepest_gouge = 0.0_f64;
        let mut on_dial = 0usize;
        let mut over_dial = 0usize;
        let mut per_band: HashMap<SlopeBand, Vec<f64>> = HashMap::new();

        let standing_threshold = params.dial_mm * params.standing_mult;

        // FOUR parallel grids are read at the same index (`true_z`,
        // `machined_z`, `resid`, `slope_deg`); an `enumerate()` over one of
        // them would only move the indexing to the other three.
        #[allow(clippy::needless_range_loop)]
        for idx in 0..self.grid.len() {
            let t = self.true_z[idx];
            if t.is_nan() {
                continue;
            }
            let m = self.machined_z[idx];
            if !m.is_finite() {
                untouched_cells += 1;
                continue;
            }
            let r = resid[idx];
            scored.push(r);
            if r > standing_threshold {
                standing_cells += 1;
            }
            if r < -params.gouge_threshold_mm {
                gouge_cells += 1;
            }
            if r < deepest_gouge {
                deepest_gouge = r;
            }
            if r.abs() <= params.dial_mm {
                on_dial += 1;
            }
            if r > params.dial_mm {
                over_dial += 1;
            }
            let s = self.slope_deg[idx];
            if !s.is_nan() {
                // The residual the oracle stamps is VERTICAL (column-frame,
                // like a dexel COLUMNS reading). The machinist's cusp is
                // measured NORMAL to the surface, which on a slope is
                // `vertical × cos θ`. Both are reported: the vertical one is
                // what a simulator would see, the normal one is what the
                // `scallop_height` dial actually promises.
                scored_normal.push(r * s.to_radians().cos());
                per_band
                    .entry(SlopeBand::of_angle_deg(s))
                    .or_default()
                    .push(r * s.to_radians().cos());
            }
        }

        let n = scored.len();
        scored.sort_by(f64::total_cmp);
        scored_normal.sort_by(f64::total_cmp);

        let bands = SlopeBand::ALL
            .iter()
            .map(|&b| {
                let mut v = per_band.remove(&b).unwrap_or_default();
                v.sort_by(f64::total_cmp);
                (b, BandScore::from_sorted(&v, cell_area, params.dial_mm))
            })
            .collect();

        OracleReport {
            scored_cells: n,
            cell_area_mm2: cell_area,
            resid_p50_um: quantile(&scored, 0.50) * 1000.0,
            resid_p90_um: quantile(&scored, 0.90) * 1000.0,
            resid_p99_um: quantile(&scored, 0.99) * 1000.0,
            resid_max_um: scored.last().copied().unwrap_or(f64::NAN) * 1000.0,
            achieved_cusp_um: quantile(&scored, 0.99) * 1000.0,
            achieved_cusp_normal_um: quantile(&scored_normal, 0.99) * 1000.0,
            dial_um: params.dial_mm * 1000.0,
            on_dial_frac: frac(on_dial, n),
            over_dial_frac: frac(over_dial, n),
            standing_mm2: standing_cells as f64 * cell_area,
            untouched_mm2: untouched_cells as f64 * cell_area,
            gouge_mm2: gouge_cells as f64 * cell_area,
            deepest_gouge_um: deepest_gouge * 1000.0,
            bands,
        }
    }
}

/// Knobs for [`EnvelopeOracle::report`].
#[derive(Debug, Clone, Copy)]
pub struct OracleParams {
    pub dial_mm: f64,
    pub stock_to_leave: f64,
    /// Multiples of the dial above which a residual is called *standing
    /// material* rather than an over-target cusp.
    pub standing_mult: f64,
    /// Below `−this`, a residual is called a gouge. 50 µm matches Checkpoint
    /// B's `gouge_over_50um` so the two instruments' gouge columns are
    /// comparable.
    pub gouge_threshold_mm: f64,
}

impl OracleParams {
    #[must_use]
    pub fn new(dial_mm: f64) -> Self {
        Self {
            dial_mm,
            stock_to_leave: 0.0,
            standing_mult: 5.0,
            gouge_threshold_mm: 0.050,
        }
    }
}

/// Per-slope-band residual score. M4's acceptance gate is *"achieved cusp
/// stays within tolerance on every slope class, not only average"*, so the
/// band split is a first-class output, not a drill-down.
#[derive(Debug, Clone, Copy)]
pub struct BandScore {
    pub cells: usize,
    pub area_mm2: f64,
    pub p50_um: f64,
    pub p99_um: f64,
    pub max_um: f64,
    pub over_dial_frac: f64,
}

impl BandScore {
    fn from_sorted(sorted: &[f64], cell_area: f64, dial_mm: f64) -> Self {
        let n = sorted.len();
        Self {
            cells: n,
            area_mm2: n as f64 * cell_area,
            p50_um: quantile(sorted, 0.50) * 1000.0,
            p99_um: quantile(sorted, 0.99) * 1000.0,
            max_um: sorted.last().copied().unwrap_or(f64::NAN) * 1000.0,
            over_dial_frac: frac(sorted.iter().filter(|r| **r > dial_mm).count(), n),
        }
    }
}

/// The oracle's verdict on one arm.
#[derive(Debug, Clone)]
pub struct OracleReport {
    pub scored_cells: usize,
    pub cell_area_mm2: f64,
    pub resid_p50_um: f64,
    pub resid_p90_um: f64,
    pub resid_p99_um: f64,
    pub resid_max_um: f64,
    /// p99 of the residual — see the module doc for why this, and not the
    /// max, is the headline cusp number.
    pub achieved_cusp_um: f64,
    /// The same p99 taken on the SURFACE-NORMAL residual (`vertical · cos θ`)
    /// — the cusp the `scallop_height` dial actually promises. On flat ground
    /// the two coincide; on a slope they differ by `cos θ`, and what happens
    /// to this one as slope rises is M4's central question.
    pub achieved_cusp_normal_um: f64,
    pub dial_um: f64,
    pub on_dial_frac: f64,
    pub over_dial_frac: f64,
    /// Area whose residual exceeds `dial × standing_mult` — material a pass
    /// went over and did not take down.
    pub standing_mm2: f64,
    /// Area no cutter position ever covered — material the path never
    /// reached at all. Distinct from `standing_mm2` on purpose: the ring
    /// cascade's `max_rings` truncation produces THIS one.
    pub untouched_mm2: f64,
    pub gouge_mm2: f64,
    pub deepest_gouge_um: f64,
    pub bands: Vec<(SlopeBand, BandScore)>,
}

impl OracleReport {
    /// Achieved cusp as a multiple of the dial — the number M4's acceptance
    /// gate is written in.
    #[must_use]
    pub fn cusp_ratio(&self) -> f64 {
        self.achieved_cusp_um / self.dial_um
    }

    /// Achieved surface-normal cusp as a multiple of the dial.
    #[must_use]
    pub fn cusp_ratio_normal(&self) -> f64 {
        self.achieved_cusp_normal_um / self.dial_um
    }

    /// All material not where it should be: never-reached plus left-standing.
    #[must_use]
    pub fn unfinished_mm2(&self) -> f64 {
        self.untouched_mm2 + self.standing_mm2
    }
}

// ---------------------------------------------------------------------------
// Path-side metrics: stepover distribution, ring structure, junction cost
// ---------------------------------------------------------------------------

/// Path-side structure the envelope cannot see: how far apart neighbouring
/// passes actually ended up, how long the emitted segments are, and how many
/// rings there were.
///
/// The achieved-stepover measurement reuses Checkpoint B's `measured_cusp`
/// bucketing idea — nearest point on a DIFFERENT ring, found through a uniform
/// spatial hash — but reports the whole distribution in 3D rather than a
/// single flat-ground cusp conversion. The plan asks for *"stepover
/// distribution along each ring"* and a min-vs-median ratio is exactly the
/// statistic that exposes a min-across-ring collapse.
#[derive(Debug, Clone)]
pub struct PathStructure {
    pub cut_moves: usize,
    pub rings: usize,
    pub total_cut_mm: f64,
    pub min_segment_mm: f64,
    pub seg_p01_mm: f64,
    pub seg_p50_mm: f64,
    /// Achieved 3D spacing to the nearest point on another ring.
    pub stepover_p05_mm: f64,
    pub stepover_p50_mm: f64,
    pub stepover_p95_mm: f64,
    pub stepover_min_mm: f64,
    /// `p50 / p05` — how far the tightest spacing is below the typical one.
    /// A per-ring constant stepover chosen by MIN drives this toward 1.0 by
    /// dragging the typical spacing down to the tightest one.
    pub stepover_spread: f64,
}

/// Measure [`PathStructure`] from a toolpath and the ring-start move indices
/// the scallop generator annotates.
#[must_use]
pub fn path_structure(toolpath: &Toolpath, ring_starts: &[usize], bucket_mm: f64) -> PathStructure {
    // Which ring each move index belongs to.
    let mut ring_of = vec![usize::MAX; toolpath.moves.len()];
    if !ring_starts.is_empty() {
        let mut sorted = ring_starts.to_vec();
        sorted.sort_unstable();
        let mut r = 0usize;
        for (i, slot) in ring_of.iter_mut().enumerate() {
            while r + 1 < sorted.len() && i >= sorted[r + 1] {
                r += 1;
            }
            if i >= sorted[0] {
                *slot = r;
            }
        }
    }

    let mut segs: Vec<f64> = Vec::new();
    let mut total = 0.0;
    let mut pts: Vec<(P3, usize)> = Vec::new();
    let mut prev: Option<P3> = None;
    for (i, mv) in toolpath.moves.iter().enumerate() {
        let cutting = !matches!(mv.move_type, MoveType::Rapid);
        if cutting {
            if let Some(a) = prev {
                let d = ((mv.target.x - a.x).powi(2)
                    + (mv.target.y - a.y).powi(2)
                    + (mv.target.z - a.z).powi(2))
                .sqrt();
                if d > 1e-9 {
                    segs.push(d);
                    total += d;
                }
            }
            pts.push((mv.target, ring_of.get(i).copied().unwrap_or(usize::MAX)));
        }
        prev = Some(mv.target);
    }

    // Nearest point on a different ring, via a uniform spatial hash.
    let cell = bucket_mm.max(1e-3);
    let mut hash: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, (p, _)) in pts.iter().enumerate() {
        hash.entry(((p.x / cell).floor() as i64, (p.y / cell).floor() as i64))
            .or_default()
            .push(i);
    }
    let mut steps: Vec<f64> = Vec::new();
    for (i, (p, ring)) in pts.iter().enumerate() {
        if *ring == usize::MAX {
            continue;
        }
        let (bx, by) = ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
        let mut best = f64::INFINITY;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(bucket) = hash.get(&(bx + dx, by + dy)) else {
                    continue;
                };
                for &j in bucket {
                    if j == i {
                        continue;
                    }
                    let (q, qr) = pts[j];
                    if qr == *ring || qr == usize::MAX {
                        continue;
                    }
                    let d =
                        ((q.x - p.x).powi(2) + (q.y - p.y).powi(2) + (q.z - p.z).powi(2)).sqrt();
                    if d < best {
                        best = d;
                    }
                }
            }
        }
        if best.is_finite() {
            steps.push(best);
        }
    }

    segs.sort_by(f64::total_cmp);
    steps.sort_by(f64::total_cmp);
    let p05 = quantile(&steps, 0.05);
    let p50 = quantile(&steps, 0.50);
    PathStructure {
        cut_moves: pts.len(),
        rings: ring_starts.len(),
        total_cut_mm: total,
        min_segment_mm: segs.first().copied().unwrap_or(f64::NAN),
        seg_p01_mm: quantile(&segs, 0.01),
        seg_p50_mm: quantile(&segs, 0.50),
        stepover_p05_mm: p05,
        stepover_p50_mm: p50,
        stepover_p95_mm: quantile(&steps, 0.95),
        stepover_min_mm: steps.first().copied().unwrap_or(f64::NAN),
        stepover_spread: if p05 > 1e-9 { p50 / p05 } else { f64::NAN },
    }
}

// ---------------------------------------------------------------------------
// Rendering — the v3 rule: never gate on an aggregate without rendering it
// ---------------------------------------------------------------------------

/// `<workspace>/target/m4_scallop_oracle`.
#[must_use]
pub fn out_dir() -> PathBuf {
    let dir = super::repo_root().join("target/m4_scallop_oracle");
    std::fs::create_dir_all(&dir).expect("create oracle output dir");
    dir
}

/// Render a per-cell scalar field as a diverging heat map.
///
/// Red = above target (cusp / standing), blue = below target (gouge), grey =
/// on size, black = no data. `scale_um` saturates the ramp. Row 0 is min-Y and
/// is flipped to the image bottom, matching the M3 COLUMNS renderer so the two
/// instruments' maps can be laid side by side.
pub fn render_field(grid: &OracleGrid, values: &[f64], scale_um: f64, name: &str) -> PathBuf {
    let mut buf = vec![0u8; grid.len() * 4];
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let v = values[grid.index(row, col)];
            let out_row = grid.rows - 1 - row;
            let o = (out_row * grid.cols + col) * 4;
            let (r, g, b, a) = if v.is_nan() {
                (0, 0, 0, 255)
            } else if !v.is_finite() {
                // untouched: distinguish from "no surface" — magenta
                (255, 0, 255, 255)
            } else {
                let t = (v * 1000.0 / scale_um).clamp(-1.0, 1.0);
                if t.abs() < 0.05 {
                    (140, 140, 140, 255)
                } else if t > 0.0 {
                    (
                        255,
                        (255.0 * (1.0 - t)) as u8,
                        (255.0 * (1.0 - t)) as u8,
                        255,
                    )
                } else {
                    let s = -t;
                    (
                        (255.0 * (1.0 - s)) as u8,
                        (255.0 * (1.0 - s)) as u8,
                        255,
                        255,
                    )
                }
            };
            buf[o] = r;
            buf[o + 1] = g;
            buf[o + 2] = b;
            buf[o + 3] = a;
        }
    }
    let path = out_dir().join(format!("{name}.png"));
    save_rgba(&path, &buf, grid.cols as u32, grid.rows as u32);
    path
}

fn save_rgba(path: &Path, buf: &[u8], w: u32, h: u32) {
    image::save_buffer(path, buf, w, h, image::ColorType::Rgba8).expect("write oracle png");
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// `idx = round((len − 1) · q)` — the same estimator Checkpoint B and the M3
/// COLUMNS harness use, so quantiles are comparable across the three.
#[must_use]
pub fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn frac(n: usize, d: usize) -> f64 {
    if d == 0 {
        f64::NAN
    } else {
        n as f64 / d as f64
    }
}

/// Slope angle in degrees from central differences on a height grid.
fn slope_from_heights(grid: &OracleGrid, z: &[f64]) -> Vec<f64> {
    let mut out = vec![f64::NAN; grid.len()];
    if grid.rows < 3 || grid.cols < 3 {
        return out;
    }
    for row in 1..grid.rows - 1 {
        for col in 1..grid.cols - 1 {
            let zl = z[grid.index(row, col - 1)];
            let zr = z[grid.index(row, col + 1)];
            let zd = z[grid.index(row - 1, col)];
            let zu = z[grid.index(row + 1, col)];
            if zl.is_nan() || zr.is_nan() || zd.is_nan() || zu.is_nan() {
                continue;
            }
            let dzdx = (zr - zl) / (2.0 * grid.cell);
            let dzdy = (zu - zd) / (2.0 * grid.cell);
            out[grid.index(row, col)] = dzdx.hypot(dzdy).atan().to_degrees();
        }
    }
    out
}

/// A straight-line raster toolpath at a fixed XY pitch, draped onto an
/// analytic surface — the synthetic path the oracle's ground-truth validation
/// is run against, where the achieved cusp is known in closed form.
#[must_use]
pub fn analytic_raster(
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    pitch: f64,
    along_step: f64,
    z: impl Fn(f64, f64) -> f64,
) -> Toolpath {
    let mut tp = Toolpath::new();
    let mut x = x0;
    let mut flip = false;
    while x <= x1 + 1e-9 {
        let mut ys: Vec<f64> = Vec::new();
        let mut y = y0;
        while y <= y1 + 1e-9 {
            ys.push(y);
            y += along_step;
        }
        if flip {
            ys.reverse();
        }
        for (i, &yy) in ys.iter().enumerate() {
            let p = P3::new(x, yy, z(x, yy));
            if i == 0 {
                tp.rapid_to(p);
            } else {
                tp.feed_to(p, 1000.0);
            }
        }
        flip = !flip;
        x += pitch;
    }
    tp
}

/// The closed-form flat-ground cusp for a ball of radius `r` at pass spacing
/// `d` — `h = R − √(R² − (d/2)²)`. Ground truth for the validation suite.
#[must_use]
pub fn closed_form_cusp(r: f64, d: f64) -> f64 {
    let a = (d * 0.5).min(r);
    r - (r * r - a * a).sqrt()
}

/// Closed-form residual quantile on a uniform raster field: the residual over
/// the cross-pass coordinate is `R − √(R² − u²)`, `u` uniform on
/// `[−d/2, d/2]`, so the `q`-quantile of the residual is the value at
/// `|u| = q·d/2`.
#[must_use]
pub fn closed_form_cusp_quantile(r: f64, d: f64, q: f64) -> f64 {
    closed_form_cusp(r, d * q)
}

#[allow(dead_code)]
fn _p2_unused(_: P2) {}
