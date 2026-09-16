//! **Track H phase V0 — the valley prize census.** Is there a prize inside
//! valley territory on the operator's wanaka board?
//!
//! Pre-registration: `planning/valley_tracing_2026-09-02/FINDINGS.md` (Q1).
//! Context and priors: `planning/valley_tracing_2026-09-02/TRACK.md`.
//! This file measures; it writes NO verdict. The decision rules live in
//! FINDINGS and the orchestrator applies them.
//!
//! # What it computes
//!
//! * **mask A** — the `rivers_aligned.dxf` polyline network buffered in XY by
//!   the local valley half-width, clipped to the finish territory. Reported
//!   for the DXF-probe caveat only.
//! * **mask B** (the DECIDING mask) — valley lines from flow accumulation on
//!   a rasterised heightfield of the mesh (priority-flood +epsilon depression
//!   fill, then D8 accumulation, threshold picked by network-length
//!   stability), buffered by the same local half-width, clipped the same way.
//!   The `crest_lines.rs` kappa2 criterion is NOT used: FINDINGS forbids it
//!   for the deciding mask, because it reads near-zero on the broad valley
//!   floors that are the geometry under question.
//! * **V0-pre** — the direction-prize ceiling of
//!   `tests/wanaka_curvature_anisotropy.rs`'s census, restricted to mask B,
//!   with the whole-territory ceiling beside it for scale.
//! * **M1** — the fraction of finish cutting time inside the mask.
//! * **M2** — the x-floor ratio inside the mask, `L_min = integral dA / s_max`.
//! * **M3** — the time-weighted angle between each region's C2 lattice
//!   direction and the local valley axis, inside the mask.
//! * **M4** — per region: `theta_max` with mask cells excised vs included,
//!   and the implied `cos theta_max` derate refund.
//! * **V0-att** (amendment, pre-registered in FINDINGS before its run) —
//!   **M2_spacing**, the x-floor a PERFECT raster at the SHIPPED derated
//!   spacing would read in the mask: `integral dA/s_shipped ÷ integral
//!   dA/s_max`, where `s_shipped` is the owning region's
//!   `cos(theta_max)`-derated stepover; and the **residual**
//!   `M2 - M2_spacing`, which is the path-topology part — the only part
//!   tracing can win. M4 saturated at the steep clamp on this board, so the
//!   derate-vs-direction attribution had to come from somewhere else.
//!
//! * **V0-att v2** (amendment, pre-registered in FINDINGS before its run) —
//!   the DIRECTION-AWARE XY model, which reproduces what actually ships.
//!   v1's surface-pitch formula was ill-posed on this measurement (see
//!   below); v2 replaces it as the attribution arm and v1 survives only as
//!   the bracket's upper bound.
//!
//! # V0-att v2 — the along-track secant, stated before the run
//!
//! The shipped raster spaces its passes at `s_shipped` in **XY projection**,
//! not on the surface. Over an XY patch of area `a_xy` its XY-projected path
//! length is `a_xy / s_shipped`, and its 3-D length is that times the secant
//! of the slope **along the pass**, not of the full slope.
//!
//! The classification surface is a heightfield, so from its unit upward
//! normal `n = (n_x, n_y, n_z)`:
//!
//! ```text
//!     grad z = (f_x, f_y) = (-n_x / n_z,  -n_y / n_z)
//! ```
//!
//! and for a unit XY pass direction `d = (d_x, d_y)`:
//!
//! ```text
//!     tan phi = |grad z . d| = |f_x d_x + f_y d_y|
//!     sec phi = sqrt(1 + (grad z . d)^2)
//!
//!     M2_xy(d) = SUM (a_xy / s_shipped(region)) * sec phi   /   L_min
//! ```
//!
//! `sec phi` is invariant under `d -> -d`, so a pass direction being an AXIS
//! (180-degree periodic) raises no sign question. Two worked checks, which
//! are the whole reason the formula is written down here first:
//!
//! * **flat ground** — `sec phi = 1` in every direction, so
//!   `M2_xy = s_max / s_shipped = 1.4142`;
//! * **a 45-degree wall, pass along the contour** — `grad z . d = 0`, so
//!   `sec phi = 1`, while `dA = a_xy / cos 45`, and
//!   `M2_xy = (1/s_shipped) / (1.4142/s_max) = 1.000`. The same wall climbed
//!   straight up reads `1.4142` again.
//!
//! That is exactly the "1.41x on a flat floor, 1.00x on a 45-degree wall"
//! behaviour v1 could not express.
//!
//! **Two directions are evaluated.** `d_shipped` is the owning region's C2
//! lattice direction — the perfect, turn-free shipped raster. `d_valley` is
//! the local valley-line tangent from the nearest network line, and it is
//! evaluated on MASK populations ONLY: outside a mask the valley direction is
//! undefined, so no territory figure is printed. A mask cell whose nearest
//! line carries no tangent falls back to `d_shipped` — it then contributes
//! exactly zero to `D_pot`, which can only SHRINK the direction prize, and
//! the count of such cells is reported.
//!
//! **The bracket.** `A_xy/s_shipped / L_min` (every `sec phi` forced to 1) is
//! a direction-independent LOWER bound; v1's surface-pitch figure is the
//! UPPER bound. The measured `M2` must land between them on every
//! population. A violation is a model error, not noise, and is printed as
//! `BRACKET VIOLATED`.
//!
//! # Why v1 was retired (recorded, not hidden)
//!
//! v1 was `integral dA/s_shipped / integral dA/s_max`. With a constant
//! `s_max` and a per-region `s_shipped`, `dA` CANCELS inside each region, so
//! the ratio is an area-weighted mean of `s_max / s_region` and carries no
//! geometry at all. On this board `theta_max` saturates at the 45-degree
//! steep clamp in 15 of 16 regions, so v1 collapsed to the single scalar
//! `0.486210 / 0.343802 = sqrt(2) = 1.4142` on all seven populations, and its
//! residual came out NEGATIVE everywhere because it models a surface-pitch
//! raster that does not ship. It is kept as the bracket's upper bound, which
//! is the one thing it is still good for.
//!
//! **The two sides of the residual are the same measure.** `M2_spacing` and
//! `M2` are accumulated in ONE loop over the SAME mask cells against the SAME
//! `L_min = integral dA/s_max` denominator; only the numerator differs (an
//! ideal length at `s_shipped` against the measured cut-intent distance).
//! One honest asymmetry, stated rather than hidden: `M2`'s numerator is
//! gathered per emitted MOVE by its midpoint, `M2_spacing`'s per grid CELL.
//! The distance/time cross-check already shows that membership test agreeing
//! with itself to 0.04 pp.
//!
//! It also writes SVG overlays to `target/valley_census_h0/`. Standing repo
//! rule: never gate on an aggregate without rendering the surface.
//!
//! # Caveats this run carries, stated before the numbers
//!
//! * **The DXF is map hydrography, not incised geometry.** The wanaka export
//!   sets `river_through_cut = false` (`rivmap_data.toml`), so the
//!   `rivers_aligned.dxf` network is NOT cut into the mesh and need not sit
//!   in the mesh's geometric valleys. That is exactly why FINDINGS makes
//!   mask B the deciding mask.
//! * **wanaka200's facet rule fails.** Median facet edge ~0.35 mm against a
//!   0.486 mm stepover. Nothing here is a spacing claim.
//! * **The M1 denominator is narrower than the spec's.** FINDINGS says
//!   "front-finish cutting time", which on `wanaka200_mt2.toml` is tier 0
//!   (R1.5) plus tier 1 (R1.0) across all three bands. This instrument costs
//!   the **tier-1 Shallow band** only — the §0i regime the orchestrator
//!   pointed at, and the only one with a measured machined-stock ceiling. A
//!   smaller denominator makes the reported M1 an **upper bound** on the
//!   spec's M1. The measured share of the Shallow band's own area is printed
//!   so the gap is visible.
//!
//! # Operationalisations FINDINGS left open (declared here, before the run)
//!
//! FINDINGS fixes the *convention* (chamfer DT, the `rest_field.rs` one) but
//! not the mask that DT runs on, nor the accumulation threshold rule. Both
//! are pinned here and neither moves after the first run:
//!
//! 1. **The low-ground mask** the DT measures against is a topographic
//!    position index: a covered cell is "low" when its height is below the
//!    mean height of a square window of half-width [`TPI_HALF_WINDOW_MM`]
//!    centred on it. The window is DELIBERATELY large (30 mm on a 200 mm
//!    board): a window narrower than the valley makes a broad floor read
//!    *above* its own local mean, which would hollow out exactly the
//!    geometry the census is about. Area share is reported at
//!    [`TPI_SENSITIVITY_MM`] as the sensitivity check.
//! 2. **The half-width** at a network sample is `chamfer_DT * cell`, floored
//!    at one cell so a sample that lands outside the low mask still buffers
//!    its own cell rather than vanishing. The count of such samples is
//!    reported.
//! 3. **The accumulation threshold** is picked from the geometric ladder
//!    [`ACC_LADDER_MM2`] as the interior rung with the FLATTEST log-log
//!    network-length slope `|d ln L / d ln T|` — the stability rule FINDINGS
//!    names. `L(T)` at every rung is printed as the evidence.
//! 4. **The hydrology runs on the LAND view only** — `z > 0`
//!    (`base_height_mm = 0.0` in `rivmap_data.toml`), so the sea and the wave
//!    trench are the base level and the coastline is the outlet. This was
//!    CORRECTED after the first run, which flooded the whole board (99.50 % of
//!    cells raised) because the export writes a raised outer edge band; see
//!    [`Field::land_view`]. The whole-board flood is still run and its raised
//!    fraction printed as the evidence. The masks are additionally clipped to
//!    the finish territory.
//!
//! # Running it
//!
//! ```text
//! cargo test -p rs_cam_core --test valley_prize_census_h0 \
//!   wanaka_valley_prize_census_h0 -- --ignored --nocapture
//! ```
//!
//! `#[ignore]` because it needs the operator's wanaka mesh and rivers DXF,
//! neither of which is in the repo — the same meaning `#[ignore]` carries
//! everywhere else here. It SKIPS rather than fails when they are absent, and
//! it never substitutes `fixtures/terrain_small.stl` (banned by the
//! pre-registration).

#![allow(
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use rs_cam_core::classify_probe::ClassificationSampler;
use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use rs_cam_core::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::geo::P2;
use rs_cam_core::geometry::contour_extract::marching_squares_bool_grid;
use rs_cam_core::geometry::grid_field::distance_transform_2d;
use rs_cam_core::geometry::monotone_cells::{lattice_monotone_cells, region_frame};
use rs_cam_core::maps::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::maps::tier_map::{ResidualTreatment, TierLadder, TierMapParams, compute_tier_map};
use rs_cam_core::mesh::{QueryScratch, SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::unified_finish::unified_finish_classification_resolution;

// ── fixtures (outside the repo, by nature) ──────────────────────────────

const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";
const RIVERS_DXF: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/rivers_aligned.dxf";

// ── dials, copied from `planning/multitool_2026-08-23/wanaka200_mt2.toml` ──
//
// Verbatim from `tests/thin_organic_island_widths.rs`, which reads the same
// file. Restated rather than imported: an integration test cannot `use`
// another one.

const CELL_MM: f64 = 0.3;
const TOLERANCE_MM: f64 = 0.05;
const MARGIN_MM: f64 = 0.5;
const COARSENESS: f64 = 1.0;
const OVERLAP_MM: f64 = 2.0;
const MAX_REGIONS_PER_TIER: usize = 24;
const CUSP_HEIGHT_MM: f64 = 0.03;
const OP_TOLERANCE_MM: f64 = 0.05;

const MACHINE_ACCEL_XYZ: [f64; 3] = [500.0, 500.0, 270.0];
const MACHINE_ACCEL_SCALAR: f64 = 423.333_333_333_333_3;
const JUNCTION_DEVIATION_MM: f64 = 0.02;
const MAX_FEED_MM_MIN: f64 = 10_000.0;
const RAPID_FEED_MM_MIN: f64 = 5_000.0;
const FEED_MM_MIN: f64 = 735.0;
const PLUNGE_MM_MIN: f64 = 180.0;

const ROUGH_DIAMETER_MM: f64 = 6.0;
const ROUGH_CUTTING_LENGTH_MM: f64 = 25.0;
const ROUGH_STOCK_TO_LEAVE_AXIAL_MM: f64 = 0.5;
const CEILING_STOCK_MARGIN_MM: f64 = 30.0;

// ── mask operationalisation (pre-declared, see the module doc) ──────────

/// Half-width (mm) of the square TPI window the low-ground mask uses.
const TPI_HALF_WINDOW_MM: f64 = 30.0;
/// Neighbouring windows reported as the sensitivity check.
const TPI_SENSITIVITY_MM: [f64; 3] = [15.0, 30.0, 50.0];
/// Geometric ladder of D8 upstream-area thresholds (mm^2).
const ACC_LADDER_MM2: [f64; 11] = [
    4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 512.0, 1024.0, 2048.0, 4096.0,
];
/// Rungs at which M1-M4 and V0-pre are ALSO reported, beside the rung the
/// pre-declared stability rule picks.
///
/// The rule is not re-picked and no bar moves: this is a sensitivity table.
/// It exists because the first sane-hydrology run showed `|d ln L / d ln T|`
/// rising MONOTONICALLY across the whole ladder — there is no plateau on this
/// board, so "flattest rung" degenerates to "the lowest interior rung", and a
/// single rung would be a weak place to read a decision off.
const ACC_SENSITIVITY_RUNGS_MM2: [f64; 5] = [32.0, 128.0, 256.0, 512.0, 1024.0];
/// Land floor (mm). `base_height_mm = 0.0` in `rivmap_data.toml`.
const LAND_Z_MM: f64 = 0.0;
/// Along-chain half-window (mm) the valley tangent is measured over. Raw D8
/// links quantise to 45 degrees; a multi-cell chord does not.
const TANGENT_HALF_WINDOW_MM: f64 = 3.0;
/// Arc tessellation tolerance handed to the DXF importer (degrees).
const DXF_ARC_TOLERANCE_DEG: f64 = 1.0;

// ── census dials, from `tests/wanaka_curvature_anisotropy.rs` ───────────

/// The verdict cell's fit radius: the finest scale at which a quadric fit is
/// about the landscape rather than the tessellation (~3x the median facet).
const CENSUS_FIT_RADIUS_MM: f64 = 1.0;
/// The shipped multitool's two cusp radii.
const CENSUS_TOOL_RADII_MM: [f64; 2] = [1.0, 1.5];
/// Census sampling lattice (mm).
const CENSUS_LATTICE_MM: f64 = 1.0;
/// Scallop spec the strip widths are taken at.
const SCALLOP_H_MM: f64 = 0.03;
const MIN_FIT_POINTS: usize = 7;
const MIN_PIVOT_RATIO: f64 = 1e-9;
const EPS_DENOM: f64 = 1e-12;

/// How many Shallow regions the costing arm measures, largest area first. A
/// derated or rotated region needs its OWN whole-mesh drop-cutter lattice
/// (`LatticeSampling` is `pub(crate)`, so a test has no windowed door), and
/// that is the run's cost driver. The achieved area coverage is printed.
const MAX_COSTED_REGIONS: usize = 16;

// ═══════════════════════════════════════════════════════════════════════
// Part 0 — a rasterised heightfield and the hydrology on it
// ═══════════════════════════════════════════════════════════════════════

/// The rasterised heightfield and its D8 hydrology now live in
/// `rs_cam_core::surface::flow_accum` (promoted from three verbatim copies for the
/// pencil watershed-spine experiment). This census keeps its file-local
/// helpers as an extension trait so the call sites below do not change.
use rs_cam_core::surface::flow_accum::{
    FILL_EPSILON_MM, FlowField as Field, d8_accumulation, d8_receivers, priority_flood_epsilon,
};

/// File-local geometry and masking helpers on the shared [`Field`].
trait FieldExt {
    fn xy(&self, i: usize) -> P2;
    fn index_at(&self, x: f64, y: f64) -> Option<usize>;
    fn land_view(&self, land_z: f64) -> Field;
}

impl FieldExt for Field {
    fn xy(&self, i: usize) -> P2 {
        let row = i / self.nx;
        let col = i % self.nx;
        P2::new(
            self.ox + col as f64 * self.cell,
            self.oy + row as f64 * self.cell,
        )
    }

    fn index_at(&self, x: f64, y: f64) -> Option<usize> {
        let col = ((x - self.ox) / self.cell).round();
        let row = ((y - self.oy) / self.cell).round();
        if col < 0.0 || row < 0.0 {
            return None;
        }
        let (col, row) = (col as usize, row as usize);
        (col < self.nx && row < self.ny).then_some(row * self.nx + col)
    }

    /// The same lattice with everything at or below `land_z` marked nodata —
    /// the LAND view, on which the hydrology runs. See the census header for
    /// why the whole-board flood is restricted to land.
    fn land_view(&self, land_z: f64) -> Field {
        let nodata = (0..self.len())
            .map(|i| self.nodata[i] || self.z[i] <= land_z)
            .collect();
        Field {
            nx: self.nx,
            ny: self.ny,
            ox: self.ox,
            oy: self.oy,
            cell: self.cell,
            z: self.z.clone(),
            nodata,
        }
    }
}

/// Total length (mm) of the D8 links whose BOTH ends are in the network.
fn network_length_mm(field: &Field, receivers: &[Option<u32>], network: &[bool]) -> f64 {
    let mut total = 0.0;
    for i in 0..field.len() {
        if !network[i] {
            continue;
        }
        let Some(r) = receivers[i] else { continue };
        if !network[r as usize] {
            continue;
        }
        let (r0, c0) = (i / field.nx, i % field.nx);
        let (r1, c1) = (r as usize / field.nx, r as usize % field.nx);
        let step = if r0 != r1 && c0 != c1 {
            std::f64::consts::SQRT_2
        } else {
            1.0
        };
        total += step * field.cell;
    }
    total
}

// ── chamfer DT, the `src/rest_field.rs` convention ──────────────────────

/// Two-pass chamfer distance transform: for each set cell, distance **in
/// cells** to the nearest unset cell — its local half-width on a region mask.
/// Orthogonal cost 1, diagonal sqrt(2). Restated from
/// `rest_field::chamfer_distance`, which is private to the crate.
fn chamfer_distance(mask: &[bool], nx: usize, ny: usize) -> Vec<f64> {
    let big = (nx + ny) as f64 + 10.0;
    let sqrt2 = std::f64::consts::SQRT_2;
    let mut d: Vec<f64> = mask.iter().map(|&m| if m { big } else { 0.0 }).collect();
    let at = |d: &[f64], r: isize, c: isize| -> f64 {
        if r < 0 || c < 0 || r >= ny as isize || c >= nx as isize {
            big
        } else {
            d[r as usize * nx + c as usize]
        }
    };
    for r in 0..ny as isize {
        for c in 0..nx as isize {
            let i = r as usize * nx + c as usize;
            if !mask[i] {
                continue;
            }
            let mut best = d[i];
            best = best.min(at(&d, r, c - 1) + 1.0);
            best = best.min(at(&d, r - 1, c) + 1.0);
            best = best.min(at(&d, r - 1, c - 1) + sqrt2);
            best = best.min(at(&d, r - 1, c + 1) + sqrt2);
            d[i] = best;
        }
    }
    for r in (0..ny as isize).rev() {
        for c in (0..nx as isize).rev() {
            let i = r as usize * nx + c as usize;
            if !mask[i] {
                continue;
            }
            let mut best = d[i];
            best = best.min(at(&d, r, c + 1) + 1.0);
            best = best.min(at(&d, r + 1, c) + 1.0);
            best = best.min(at(&d, r + 1, c + 1) + sqrt2);
            best = best.min(at(&d, r + 1, c - 1) + sqrt2);
            d[i] = best;
        }
    }
    d
}

/// The low-ground mask: a data cell whose height is below the mean of a
/// square window of half-width `half_window_mm`. Computed through a summed
/// area table, so the window size costs nothing.
///
/// Nodata cells contribute nothing to either sum and are never "low".
fn low_ground_mask(field: &Field, half_window_mm: f64) -> Vec<bool> {
    let nx = field.nx;
    let ny = field.ny;
    // (ny+1) x (nx+1) integral images of z and of the data count.
    let mut sum = vec![0.0f64; (ny + 1) * (nx + 1)];
    let mut cnt = vec![0.0f64; (ny + 1) * (nx + 1)];
    for r in 0..ny {
        let mut row_sum = 0.0;
        let mut row_cnt = 0.0;
        for c in 0..nx {
            let i = r * nx + c;
            if !field.nodata[i] {
                row_sum += field.z[i];
                row_cnt += 1.0;
            }
            sum[(r + 1) * (nx + 1) + c + 1] = sum[r * (nx + 1) + c + 1] + row_sum;
            cnt[(r + 1) * (nx + 1) + c + 1] = cnt[r * (nx + 1) + c + 1] + row_cnt;
        }
    }
    let half = (half_window_mm / field.cell).round().max(1.0) as usize;
    let mut out = vec![false; field.len()];
    for r in 0..ny {
        let r0 = r.saturating_sub(half);
        let r1 = (r + half + 1).min(ny);
        for c in 0..nx {
            let i = r * nx + c;
            if field.nodata[i] {
                continue;
            }
            let c0 = c.saturating_sub(half);
            let c1 = (c + half + 1).min(nx);
            let idx = |rr: usize, cc: usize| rr * (nx + 1) + cc;
            let s = sum[idx(r1, c1)] - sum[idx(r0, c1)] - sum[idx(r1, c0)] + sum[idx(r0, c0)];
            let k = cnt[idx(r1, c1)] - cnt[idx(r0, c1)] - cnt[idx(r1, c0)] + cnt[idx(r0, c0)];
            if k > 0.0 && field.z[i] < s / k {
                out[i] = true;
            }
        }
    }
    out
}

// ── buffering a sample set into a mask ──────────────────────────────────

/// One buffered mask plus the statistics the report prints.
struct BufferedMask {
    inside: Vec<bool>,
    /// Number of samples whose chamfer-DT read zero (floored to one cell).
    floored_samples: usize,
    /// Number of samples whose disc was skipped as redundant (see below).
    skipped_samples: usize,
    /// Half-widths actually read, mm, sorted.
    half_widths_mm: Vec<f64>,
}

/// A disc whose centre is within this fraction of its own radius of an
/// already-stamped disc of AT LEAST the same radius is skipped.
///
/// The samples are one grid cell apart along a river, so every disc would
/// otherwise be re-painted `radius` times over. The union of discs of radius
/// `r` whose centres are `d` apart along a line differs from the continuous
/// buffer by a boundary ripple of at most `r - sqrt(r^2 - d^2/4)`, which at
/// `d = 0.25 r` is `0.008 r` — under one grid cell for every half-width this
/// board carries. The skipped count is reported.
const BUFFER_REDUNDANCY_FRACTION: f64 = 0.25;

/// Stamp a disc of the local half-width around every sample.
fn buffer_samples(field: &Field, dt_cells: &[f64], samples: &[P2]) -> BufferedMask {
    let mut inside = vec![false; field.len()];
    // Distance from each cell to the centre of the disc that covers it, and
    // that disc's radius — the redundancy test's two inputs.
    let mut cover_dist = vec![f64::INFINITY; field.len()];
    let mut cover_radius = vec![0.0f64; field.len()];
    let mut floored = 0usize;
    let mut skipped = 0usize;
    let mut widths = Vec::with_capacity(samples.len());
    for sample in samples {
        let Some(centre) = field.index_at(sample.x, sample.y) else {
            continue;
        };
        let raw = dt_cells[centre];
        if raw <= 0.0 {
            floored += 1;
        }
        let radius_cells = raw.max(1.0);
        widths.push(radius_cells * field.cell);
        if cover_radius[centre] >= radius_cells
            && cover_dist[centre] <= BUFFER_REDUNDANCY_FRACTION * radius_cells
        {
            skipped += 1;
            continue;
        }
        let reach = radius_cells.ceil() as isize;
        let (cr, cc) = ((centre / field.nx) as isize, (centre % field.nx) as isize);
        for dr in -reach..=reach {
            let r = cr + dr;
            if r < 0 || r >= field.ny as isize {
                continue;
            }
            for dc in -reach..=reach {
                let c = cc + dc;
                if c < 0 || c >= field.nx as isize {
                    continue;
                }
                let dist = ((dr * dr + dc * dc) as f64).sqrt();
                if dist > radius_cells {
                    continue;
                }
                let i = r as usize * field.nx + c as usize;
                if field.nodata[i] {
                    continue;
                }
                inside[i] = true;
                if dist < cover_dist[i] {
                    cover_dist[i] = dist;
                    cover_radius[i] = radius_cells;
                }
            }
        }
    }
    widths.sort_by(f64::total_cmp);
    BufferedMask {
        inside,
        floored_samples: floored,
        skipped_samples: skipped,
        half_widths_mm: widths,
    }
}

/// Nearest-source transform: for every cell, the index of the nearest sample
/// in `sample_cells` (as an index INTO THAT SLICE), by a two-pass chamfer
/// propagation. `u32::MAX` where nothing propagated.
///
/// Kept separate from [`buffer_samples`] on purpose: the mask is a union of
/// discs and may skip a redundant one, but M3's "nearest valley line" must
/// see every sample, and a redundancy skip must not be able to move it.
fn nearest_source(field: &Field, sample_cells: &[usize]) -> Vec<u32> {
    let nx = field.nx;
    let ny = field.ny;
    let big = (nx + ny) as f64 + 10.0;
    let sqrt2 = std::f64::consts::SQRT_2;
    let mut dist = vec![big; field.len()];
    let mut src = vec![u32::MAX; field.len()];
    for (s, &cell) in sample_cells.iter().enumerate() {
        dist[cell] = 0.0;
        src[cell] = s as u32;
    }
    let relax = |i: usize, j: isize, step: f64, dist: &mut [f64], src: &mut [u32]| {
        if j < 0 || j as usize >= dist.len() {
            return;
        }
        let j = j as usize;
        if dist[j] + step < dist[i] {
            dist[i] = dist[j] + step;
            src[i] = src[j];
        }
    };
    for r in 0..ny {
        for c in 0..nx {
            let i = r * nx + c;
            let ii = i as isize;
            if c > 0 {
                relax(i, ii - 1, 1.0, &mut dist, &mut src);
            }
            if r > 0 {
                relax(i, ii - nx as isize, 1.0, &mut dist, &mut src);
                if c > 0 {
                    relax(i, ii - nx as isize - 1, sqrt2, &mut dist, &mut src);
                }
                if c + 1 < nx {
                    relax(i, ii - nx as isize + 1, sqrt2, &mut dist, &mut src);
                }
            }
        }
    }
    for r in (0..ny).rev() {
        for c in (0..nx).rev() {
            let i = r * nx + c;
            let ii = i as isize;
            if c + 1 < nx {
                relax(i, ii + 1, 1.0, &mut dist, &mut src);
            }
            if r + 1 < ny {
                relax(i, ii + nx as isize, 1.0, &mut dist, &mut src);
                if c + 1 < nx {
                    relax(i, ii + nx as isize + 1, sqrt2, &mut dist, &mut src);
                }
                if c > 0 {
                    relax(i, ii + nx as isize - 1, sqrt2, &mut dist, &mut src);
                }
            }
        }
    }
    src
}

// ═══════════════════════════════════════════════════════════════════════
// Part 1 — the curvature census (restated from
// `tests/wanaka_curvature_anisotropy.rs`; an integration test cannot import
// another one, and that file's own header says so of its siblings)
// ═══════════════════════════════════════════════════════════════════════

struct Scratch {
    query: QueryScratch,
    tris: Vec<usize>,
    stamp: Vec<u32>,
    generation: u32,
}

impl Scratch {
    fn new(vertex_count: usize) -> Self {
        Self {
            query: QueryScratch::default(),
            tris: Vec::new(),
            stamp: vec![0u32; vertex_count],
            generation: 0,
        }
    }
}

/// Local differential geometry at one sample, convex-positive (Zou).
#[derive(Clone, Copy)]
struct Fit {
    kappa1: f64,
    kappa2: f64,
    form_e: f64,
    form_f: f64,
    form_g: f64,
    form_l: f64,
    form_m: f64,
    form_n: f64,
    area_weight: f64,
}

enum Outcome {
    Fitted(Fit),
    UnderDetermined,
    IllConditioned,
}

/// Height of the heightfield at `(x, y)`. Sound because this mesh is a
/// single-valued heightfield and `cell_triangles_at` is a superset of the
/// triangles a vertical ray can pierce.
fn surface_z(mesh: &TriangleMesh, index: &SpatialIndex, at: P2) -> Option<f64> {
    const BARY_EPS: f64 = 1e-9;
    let mut best: Option<f64> = None;
    for &t in index.cell_triangles_at(at.x, at.y) {
        let tri = mesh.triangles[t];
        let p0 = mesh.vertices[tri[0] as usize];
        let p1 = mesh.vertices[tri[1] as usize];
        let p2 = mesh.vertices[tri[2] as usize];
        let den = (p1.y - p2.y) * (p0.x - p2.x) + (p2.x - p1.x) * (p0.y - p2.y);
        if den.abs() < 1e-14 {
            continue;
        }
        let l0 = ((p1.y - p2.y) * (at.x - p2.x) + (p2.x - p1.x) * (at.y - p2.y)) / den;
        let l1 = ((p2.y - p0.y) * (at.x - p2.x) + (p0.x - p2.x) * (at.y - p2.y)) / den;
        let l2 = 1.0 - l0 - l1;
        if l0 < -BARY_EPS || l1 < -BARY_EPS || l2 < -BARY_EPS {
            continue;
        }
        let z = l0 * p0.z + l1 * p1.z + l2 * p2.z;
        best = Some(best.map_or(z, |b: f64| b.max(z)));
    }
    best
}

#[allow(clippy::needless_range_loop)]
fn solve_sym6(a: &[[f64; 6]; 6], b: &[f64; 6]) -> Option<([f64; 6], f64)> {
    let mut m = [[0.0f64; 7]; 6];
    for row in 0..6 {
        for col in 0..6 {
            m[row][col] = a[row][col];
        }
        m[row][6] = b[row];
    }
    let mut pivot_min = f64::INFINITY;
    let mut pivot_max = 0.0f64;
    for col in 0..6 {
        let mut best = col;
        for row in (col + 1)..6 {
            if m[row][col].abs() > m[best][col].abs() {
                best = row;
            }
        }
        m.swap(col, best);
        let pivot = m[col][col];
        let mag = pivot.abs();
        pivot_min = pivot_min.min(mag);
        pivot_max = pivot_max.max(mag);
        if mag < f64::MIN_POSITIVE {
            return None;
        }
        let inv = 1.0 / pivot;
        for k in col..7 {
            m[col][k] *= inv;
        }
        for row in 0..6 {
            if row == col {
                continue;
            }
            let factor = m[row][col];
            if factor == 0.0 {
                continue;
            }
            for k in col..7 {
                let pivot_row = m[col][k];
                m[row][k] -= factor * pivot_row;
            }
        }
    }
    let mut x = [0.0f64; 6];
    for row in 0..6 {
        x[row] = m[row][6];
    }
    let ratio = if pivot_max > 0.0 {
        pivot_min / pivot_max
    } else {
        0.0
    };
    Some((x, ratio))
}

fn fit_from_derivatives((f_x, f_y): (f64, f64), (f_xx, f_xy, f_yy): (f64, f64, f64)) -> Fit {
    let area_weight = (1.0 + f_x * f_x + f_y * f_y).sqrt();
    let form_e = 1.0 + f_x * f_x;
    let form_f = f_x * f_y;
    let form_g = 1.0 + f_y * f_y;
    let form_l = f_xx / area_weight;
    let form_m = f_xy / area_weight;
    let form_n = f_yy / area_weight;
    let det = form_e * form_g - form_f * form_f;
    let gauss = (form_l * form_n - form_m * form_m) / det;
    let mean = (form_e * form_n - 2.0 * form_f * form_m + form_g * form_l) / (2.0 * det);
    let spread = (mean * mean - gauss).max(0.0).sqrt();
    Fit {
        kappa1: -(mean - spread),
        kappa2: -(mean + spread),
        form_e,
        form_f,
        form_g,
        form_l,
        form_m,
        form_n,
        area_weight,
    }
}

#[allow(clippy::needless_range_loop)]
fn finalise_normal_equations(normal: &mut [[f64; 6]; 6], rhs: &mut [f64; 6], inv: f64) {
    for i in 0..6 {
        for j in 0..6 {
            if j < i {
                let mirrored = normal[j][i];
                normal[i][j] = mirrored;
            } else {
                normal[i][j] *= inv;
            }
        }
        rhs[i] *= inv;
    }
}

fn fit_quadric(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    scratch: &mut Scratch,
    (at, z0): (P2, f64),
    radius: f64,
) -> Outcome {
    let mut tris = std::mem::take(&mut scratch.tris);
    index.query_rect_into(
        at.x - radius,
        at.x + radius,
        at.y - radius,
        at.y + radius,
        &mut scratch.query,
        &mut tris,
    );
    scratch.generation = scratch.generation.wrapping_add(1);
    let generation = scratch.generation;

    let mut normal = [[0.0f64; 6]; 6];
    let mut rhs = [0.0f64; 6];
    let mut count = 0usize;
    let r2 = radius * radius;

    for &t in &tris {
        for &vi in &mesh.triangles[t] {
            let vi = vi as usize;
            if scratch.stamp[vi] == generation {
                continue;
            }
            scratch.stamp[vi] = generation;
            let p = mesh.vertices[vi];
            let dx = p.x - at.x;
            let dy = p.y - at.y;
            if dx * dx + dy * dy > r2 {
                continue;
            }
            let u = dx / radius;
            let v = dy / radius;
            let w = p.z - z0;
            let basis = [u * u, u * v, v * v, u, v, 1.0];
            for (i, &bi) in basis.iter().enumerate() {
                for (j, &bj) in basis.iter().enumerate().skip(i) {
                    normal[i][j] += bi * bj;
                }
                rhs[i] += bi * w;
            }
            count += 1;
        }
    }
    scratch.tris = tris;

    if count < MIN_FIT_POINTS {
        return Outcome::UnderDetermined;
    }
    let inv = 1.0 / count as f64;
    finalise_normal_equations(&mut normal, &mut rhs, inv);

    let Some((beta, pivot_ratio)) = solve_sym6(&normal, &rhs) else {
        return Outcome::IllConditioned;
    };
    if !pivot_ratio.is_finite() || pivot_ratio < MIN_PIVOT_RATIO {
        return Outcome::IllConditioned;
    }
    if !beta.iter().all(|c| c.is_finite()) {
        return Outcome::IllConditioned;
    }
    let f_x = beta[3] / radius;
    let f_y = beta[4] / radius;
    let f_xx = 2.0 * beta[0] / (radius * radius);
    let f_xy = beta[1] / (radius * radius);
    let f_yy = 2.0 * beta[2] / (radius * radius);
    Outcome::Fitted(fit_from_derivatives((f_x, f_y), (f_xx, f_xy, f_yy)))
}

/// Normal curvature perpendicular to the XY sweep direction `(p, q)`,
/// convex-positive.
fn kappa_perp_zou(fit: &Fit, dir: (f64, f64)) -> f64 {
    let (p, q) = dir;
    let pp = -(fit.form_f * p + fit.form_g * q);
    let qq = fit.form_e * p + fit.form_f * q;
    let num = fit.form_l * pp * pp + 2.0 * fit.form_m * pp * qq + fit.form_n * qq * qq;
    let den = fit.form_e * pp * pp + 2.0 * fit.form_f * pp * qq + fit.form_g * qq * qq;
    if den.abs() < EPS_DENOM {
        return 0.5 * (fit.kappa1 + fit.kappa2);
    }
    -num / den
}

/// `W(kappa_n) = sqrt(8h / (kappa_n + 1/R))`, or `None` at the gouge
/// condition (a concavity tighter than the ball). Never clamped.
fn strip_width(kappa_n: f64, tool_radius: f64) -> Option<f64> {
    let denom = kappa_n + 1.0 / tool_radius;
    if !denom.is_finite() || denom <= EPS_DENOM {
        return None;
    }
    Some((8.0 * SCALLOP_H_MM / denom).sqrt())
}

/// The prize bounds for one tool radius over one population of fits.
struct CensusCell {
    tool_radius_mm: f64,
    fits: usize,
    gouge_area_frac: f64,
    /// `integral dA/W(fixed dir) / integral dA/W_max`, `>= 1`.
    bound_x: f64,
    bound_y: f64,
    bound_pca: f64,
    /// Area-weighted median of `W_max / W_min`.
    ratio_p50: f64,
    ratio_p90: f64,
}

fn census_cell(fits: &[Fit], cell_area: f64, tool_radius: f64, axis: (f64, f64)) -> CensusCell {
    let mut ratios: Vec<(f64, f64)> = Vec::with_capacity(fits.len());
    let mut gouge_area = 0.0f64;
    let mut total_area = 0.0f64;
    let mut floor_opt = 0.0f64;
    let mut fixed = [0.0f64; 3];
    let directions = [(1.0, 0.0), (0.0, 1.0), axis];

    for fit in fits {
        let area = fit.area_weight * cell_area;
        total_area += area;
        let (Some(w_max), Some(w_min)) = (
            strip_width(fit.kappa2, tool_radius),
            strip_width(fit.kappa1, tool_radius),
        ) else {
            gouge_area += area;
            continue;
        };
        ratios.push((w_max / w_min, area));
        floor_opt += area / w_max;
        for (slot, &dir) in fixed.iter_mut().zip(directions.iter()) {
            match strip_width(kappa_perp_zou(fit, dir), tool_radius) {
                Some(width) => *slot += area / width,
                None => *slot += area / w_min,
            }
        }
    }
    let bound = |value: f64| {
        if floor_opt > 0.0 {
            value / floor_opt
        } else {
            f64::NAN
        }
    };
    ratios.sort_by(|a, b| a.0.total_cmp(&b.0));
    let ratio_total: f64 = ratios.iter().map(|r| r.1).sum();
    let pick = |q: f64| -> f64 {
        if ratios.is_empty() {
            return f64::NAN;
        }
        let target = ratio_total * q;
        let mut run = 0.0;
        for &(value, weight) in &ratios {
            run += weight;
            if run >= target {
                return value;
            }
        }
        ratios[ratios.len() - 1].0
    };
    CensusCell {
        tool_radius_mm: tool_radius,
        fits: fits.len(),
        gouge_area_frac: if total_area > 0.0 {
            gouge_area / total_area
        } else {
            0.0
        },
        bound_x: bound(fixed[0]),
        bound_y: bound(fixed[1]),
        bound_pca: bound(fixed[2]),
        ratio_p50: pick(0.50),
        ratio_p90: pick(0.90),
    }
}

/// PCA axis of a sample population's XY positions.
fn pca_axis(points: &[P2]) -> (f64, f64) {
    let n = points.len() as f64;
    if n < 2.0 {
        return (1.0, 0.0);
    }
    let mx = points.iter().map(|p| p.x).sum::<f64>() / n;
    let my = points.iter().map(|p| p.y).sum::<f64>() / n;
    let (mut cxx, mut cxy, mut cyy) = (0.0, 0.0, 0.0);
    for p in points {
        let dx = p.x - mx;
        let dy = p.y - my;
        cxx += dx * dx;
        cxy += dx * dy;
        cyy += dy * dy;
    }
    let theta = 0.5 * (2.0 * cxy / n).atan2((cxx - cyy) / n);
    (theta.cos(), theta.sin())
}

/// Fit the census over one sample population.
fn census_population(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    samples: &[(P2, f64)],
    spacing: f64,
) -> (Vec<CensusCell>, usize, usize) {
    let cell_area = spacing * spacing;
    let outcomes: Vec<Outcome> = samples
        .par_iter()
        .with_min_len(512)
        .map_init(
            || Scratch::new(mesh.vertices.len()),
            |scratch, &sample| fit_quadric(mesh, index, scratch, sample, CENSUS_FIT_RADIUS_MM),
        )
        .collect();
    let mut fits = Vec::new();
    let mut rejected = 0usize;
    for outcome in outcomes {
        match outcome {
            Outcome::Fitted(fit) => fits.push(fit),
            Outcome::UnderDetermined | Outcome::IllConditioned => rejected += 1,
        }
    }
    let positions: Vec<P2> = samples.iter().map(|&(p, _)| p).collect();
    let axis = pca_axis(&positions);
    let cells = CENSUS_TOOL_RADII_MM
        .iter()
        .map(|&r| census_cell(&fits, cell_area, r, axis))
        .collect();
    (cells, samples.len(), rejected)
}

// ═══════════════════════════════════════════════════════════════════════
// Part 2 — costing (restated from `tests/thin_organic_island_widths.rs`,
// §0i's machined-stock `link_ceiling` regime)
// ═══════════════════════════════════════════════════════════════════════

struct CandidateCost {
    cutting_mm: f64,
    time_s: f64,
    fragments: usize,
    linked: usize,
    kept_retracts: usize,
    toolpath: rs_cam_core::toolpath::Toolpath,
}

struct LinkRegime<'a> {
    safe_z: f64,
    ceiling: Option<rs_cam_core::surface_link::LinkCeiling<'a>>,
    flush_ride: bool,
    airborne: bool,
}

/// `relink_and_cost_under` (`thin_organic_island_widths.rs:1968`), with the
/// production relink parameters that are NOT the regime: `hookup_distance`
/// 25.0 (`wanaka200_mt2.toml:949`), `sampling` 0.5, tier-1 feeds,
/// `reorder: true`, the region's own polygon as boundary.
fn relink_and_cost_under(
    raw: rs_cam_core::toolpath::Toolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    boundary: &rs_cam_core::geometry::region_set::RegionSet<'_>,
    kinematics: &rs_cam_core::machine_kinematics::MachineKinematics,
    regime: &LinkRegime<'_>,
) -> CandidateCost {
    use rs_cam_core::machine_kinematics::{LinkKinematics, compute_cycle_time};

    let link_kinematics = LinkKinematics {
        kinematics: *kinematics,
        max_feed_mm_min: MAX_FEED_MM_MIN,
        rapid_feed_mm_min: RAPID_FEED_MM_MIN,
    };
    let params = rs_cam_core::surface_link::RelinkParams {
        hookup_distance: 25.0,
        stock_to_leave: 0.0,
        sampling: 0.5,
        feed_rate: FEED_MM_MIN,
        plunge_rate: PLUNGE_MM_MIN,
        safe_z: regime.safe_z,
        link_kinematics: Some(&link_kinematics),
        reorder: true,
        boundary: Some(boundary),
        link_ceiling: regime.ceiling,
        flush_ride: regime.flush_ride,
        airborne_links_may_leave_territory: regime.airborne,
    };
    let (linked, report) = rs_cam_core::surface_link::relink_fragments(
        rs_cam_core::toolpath_spans::AnnotatedToolpath::new(raw),
        mesh,
        index,
        cutter,
        &params,
    );
    let mut channels = rs_cam_core::transform_provenance::ReconcileSet::new(None, None);
    let toolpath = linked.reconcile(&mut channels).into_inner().toolpath;
    let time_s = compute_cycle_time(&toolpath, kinematics, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
    CandidateCost {
        cutting_mm: toolpath.total_cutting_distance(),
        time_s,
        fragments: report.fragments,
        linked: report.surface_links,
        kept_retracts: report.retract_links,
        toolpath,
    }
}

fn raster_candidate(
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    regions: &[Polygon2],
    safe_z: f64,
    effective_min_z: f64,
) -> rs_cam_core::toolpath::Toolpath {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::toolpath::{Toolpath, raster_toolpath_from_grid};

    let mut out = Toolpath::new();
    for polygon in regions {
        let region = RegionSet::new(vec![polygon.clone()]);
        let toolpath = raster_toolpath_from_grid(
            grid,
            FEED_MM_MIN,
            PLUNGE_MM_MIN,
            safe_z,
            Some(effective_min_z),
            Some(&region),
        );
        out.moves.extend(toolpath.moves);
    }
    out
}

/// Nearest sample of an axis-aligned drop-cutter grid. Only valid at 0 deg.
fn nearest_contact_z(
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    x: f64,
    y: f64,
    min_z: f64,
) -> Option<f64> {
    if grid.rows == 0 || grid.cols == 0 {
        return None;
    }
    let col = ((x - grid.u_start) / grid.x_step)
        .round()
        .clamp(0.0, (grid.cols - 1) as f64) as usize;
    let row = ((y - grid.v_start) / grid.y_step)
        .round()
        .clamp(0.0, (grid.rows - 1) as f64) as usize;
    let z = grid.get(row, col).z;
    (z > min_z + 0.001).then_some(z)
}

/// The machined-stock ceiling: the rough (Ø6 flat, axial leave 0.5) then the
/// tier-0 R1.5 where tier 0 machines. Restated from `a3_machined_stock`;
/// the same three honest departures apply (the rough's toolpath is not
/// replayed, the coarse layer is a per-column surface, and the back-face ops
/// are not modelled), all of which make this the OPTIMISTIC bracket.
fn machined_stock(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    coarse: &TaperedBallEndmill,
    tier_map: &rs_cam_core::maps::tier_map::TierMap,
    bounds: [f64; 4],
) -> rs_cam_core::dexel_stock::TriDexelStock {
    use rs_cam_core::dexel_stock::TriDexelStock;
    use rs_cam_core::tool::FlatEndmill;

    let min_z = mesh.bbox.min.z - 0.1;
    let block_top_z = mesh.bbox.max.z;
    let [x0, y0, x1, y1] = bounds;
    let mut stock =
        TriDexelStock::from_stock(x0, y0, x1, y1, mesh.bbox.min.z - 1.0, block_top_z, CELL_MM);

    let rough_tool = FlatEndmill::new(ROUGH_DIAMETER_MM, ROUGH_CUTTING_LENGTH_MM);
    let rough_grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
        mesh,
        index,
        &rough_tool,
        CELL_MM,
        0.0,
        min_z,
    );
    let coarse_grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
        mesh, index, coarse, CELL_MM, 0.0, min_z,
    );

    let tier_zero: Vec<bool> = tier_map.labels.iter().map(|&label| label == 0).collect();
    let distance_to_tier_zero = distance_transform_2d(&tier_zero, tier_map.ny, tier_map.nx);
    let reach = OVERLAP_MM + coarse.cusp_radius_mm();

    let rows = stock.z_grid.rows;
    let cols = stock.z_grid.cols;
    let origin_x = stock.z_grid.origin_u;
    let origin_y = stock.z_grid.origin_v;
    let cell = stock.z_grid.cell_size;
    for row in 0..rows {
        let y = origin_y + row as f64 * cell;
        for col in 0..cols {
            let x = origin_x + col as f64 * cell;
            let Some(rough_z) = nearest_contact_z(&rough_grid, x, y, min_z) else {
                continue;
            };
            let mut top = rough_z + ROUGH_STOCK_TO_LEAVE_AXIAL_MM;
            let coarse_here = tier_map.nearest_cell(x, y).is_some_and(|(r, c)| {
                distance_to_tier_zero[r * tier_map.nx + c] * tier_map.cell_mm <= reach
            });
            if coarse_here && let Some(coarse_z) = nearest_contact_z(&coarse_grid, x, y, min_z) {
                top = top.min(coarse_z);
            }
            if top < block_top_z {
                stock.clear_above_at(row, col, top as f32);
            }
        }
    }
    stock
}

// ═══════════════════════════════════════════════════════════════════════
// Part 3 — per-move time attribution
// ═══════════════════════════════════════════════════════════════════════

/// Per-move time and distance, split by mask membership.
///
/// TWO numerator conventions are carried, because `M2` is a ratio and the two
/// sides must be the same measure:
///
/// * `*_cut_*` counts only `ClearingCut | FinishingCut` moves — the
///   `cutting_s` bucket definition, the material-removal path;
/// * `*_all_*` counts every non-rapid move, which is what
///   `Toolpath::total_cutting_distance` returns and therefore what the
///   synthesis §1 table's "raster cut" column is. On this board the
///   difference is the surface links, ~15 % of distance.
///
/// Reporting only one of them next to a territory figure computed the other
/// way would compare two different quantities.
#[derive(Default, Clone, Copy)]
struct Split {
    in_time_s: f64,
    out_time_s: f64,
    in_mm: f64,
    out_mm: f64,
    in_all_time_s: f64,
    out_all_time_s: f64,
    in_all_mm: f64,
    out_all_mm: f64,
}

/// One cutting move's contribution, kept for M3.
struct CutSample {
    mid: P2,
    dir_deg: f64,
    time_s: f64,
}

/// Attribute a costed toolpath's CUTTING time (intent `ClearingCut` or
/// `FinishingCut`, the `cutting_s` bucket definition) to inside/outside a
/// mask, by move midpoint.
///
/// **The per-move time is an approximation and it is printed as one.** The
/// production integrator (`compute_cycle_time`) publishes only a total and an
/// intent-bucketed breakdown, never a per-move vector, and its junction
/// solver is private. This uses the public `predicted_feeds_for_toolpath`
/// (each move's PEAK velocity under the same trapezoidal model), takes
/// `t_i = len_i / v_peak_i`, and rescales the whole vector so the sum equals
/// the production total. The rescale factor is reported; the distance split
/// is reported beside the time split, and if the two agree the approximation
/// is not load-bearing.
fn attribute_cutting(
    toolpath: &rs_cam_core::toolpath::Toolpath,
    kinematics: &rs_cam_core::machine_kinematics::MachineKinematics,
    total_time_s: f64,
    inside: &dyn Fn(P2) -> bool,
) -> (Split, Vec<CutSample>, f64) {
    use rs_cam_core::machine_kinematics::predicted_feeds_for_toolpath;
    use rs_cam_core::toolpath::{MoveIntent, MoveType};

    let feeds =
        predicted_feeds_for_toolpath(toolpath, kinematics, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
    let mut raw_total = 0.0f64;
    let mut rows: Vec<(usize, f64, f64)> = Vec::new(); // (index, length, raw time)
    for i in 1..toolpath.moves.len() {
        let p0 = toolpath.moves[i - 1].target;
        let p1 = toolpath.moves[i].target;
        let length = (p1 - p0).norm();
        if length <= 1e-9 {
            continue;
        }
        let Some(&v_mm_min) = feeds.get(&i) else {
            continue;
        };
        let v = (v_mm_min / 60.0).max(1e-6);
        let t = length / v;
        raw_total += t;
        rows.push((i, length, t));
    }
    let scale = if raw_total > 0.0 {
        total_time_s / raw_total
    } else {
        1.0
    };

    let mut split = Split::default();
    let mut samples = Vec::new();
    for (i, length, t) in rows {
        let move_ref = &toolpath.moves[i];
        if matches!(move_ref.move_type, MoveType::Rapid) {
            continue;
        }
        let p0 = toolpath.moves[i - 1].target;
        let p1 = move_ref.target;
        let mid = P2::new(0.5 * (p0.x + p1.x), 0.5 * (p0.y + p1.y));
        let time_s = t * scale;
        let is_in = inside(mid);
        if is_in {
            split.in_all_time_s += time_s;
            split.in_all_mm += length;
        } else {
            split.out_all_time_s += time_s;
            split.out_all_mm += length;
        }
        if !matches!(
            move_ref.intent,
            MoveIntent::ClearingCut | MoveIntent::FinishingCut
        ) {
            continue;
        }
        if is_in {
            split.in_time_s += time_s;
            split.in_mm += length;
        } else {
            split.out_time_s += time_s;
            split.out_mm += length;
        }
        let dir_deg = (p1.y - p0.y)
            .atan2(p1.x - p0.x)
            .to_degrees()
            .rem_euclid(180.0);
        samples.push(CutSample {
            mid,
            dir_deg,
            time_s,
        });
    }
    (split, samples, scale)
}

/// Axis angle between two directions given in degrees, in `[0, 90]`.
fn axis_angle_deg(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(180.0);
    if d > 90.0 { 180.0 - d } else { d }
}

// ═══════════════════════════════════════════════════════════════════════
// Part 4 — theta_max, restated with a mask filter (M4)
// ═══════════════════════════════════════════════════════════════════════

/// `shallow_region_max_slope_deg` (`unified_finish.rs:2813`), restated with
/// one extra filter: `keep` decides whether a qualifying cell counts.
///
/// Both guards are kept verbatim — a cell counts only when it AND its in-grid
/// 4-neighbours are geometrically covered, and the result is clamped to the
/// planner's `steep_threshold_deg`.
fn region_max_slope_deg(
    surface: &rs_cam_core::finish_setup::FinishSurface,
    covered: &[bool],
    polygon: &Polygon2,
    clamp_deg: f64,
    keep: &dyn Fn(P2) -> bool,
) -> (f64, usize) {
    let cols = surface.cols();
    if cols == 0 {
        return (0.0, 0);
    }
    let rows = surface.slope_map.rows;
    let cell = surface.cell_size();
    let origin_x = surface.slope_map.origin_x;
    let origin_y = surface.slope_map.origin_y;
    let geom = surface.heightmap.covered_flags();
    let geom_at =
        |row: usize, col: usize| -> bool { geom.get(row * cols + col).copied().unwrap_or(false) };
    let [bx0, by0, bx1, by1] = polygon.bbox();

    let mut max_rad = 0.0_f64;
    let mut population = 0usize;
    for (i, &angle) in surface.slope_map.angles.iter().enumerate() {
        if !covered.get(i).copied().unwrap_or(false) {
            continue;
        }
        let row = i / cols;
        let col = i % cols;
        let px = origin_x + col as f64 * cell;
        let py = origin_y + row as f64 * cell;
        if px < bx0 || px > bx1 || py < by0 || py > by1 {
            continue;
        }
        let neighbours_ok = geom_at(row, col)
            && (row == 0 || geom_at(row - 1, col))
            && (row + 1 >= rows || geom_at(row + 1, col))
            && (col == 0 || geom_at(row, col - 1))
            && (col + 1 >= cols || geom_at(row, col + 1));
        if !neighbours_ok {
            continue;
        }
        let point = P2::new(px, py);
        if !polygon.contains_point(&point) || !keep(point) {
            continue;
        }
        population += 1;
        max_rad = max_rad.max(angle);
    }
    (max_rad.to_degrees().min(clamp_deg), population)
}

// ═══════════════════════════════════════════════════════════════════════
// Part 5 — self-checks (run BEFORE the 661k-triangle load)
// ═══════════════════════════════════════════════════════════════════════

fn synthetic_field(nx: usize, ny: usize, cell: f64, height: impl Fn(f64, f64) -> f64) -> Field {
    let mut z = Vec::with_capacity(nx * ny);
    for r in 0..ny {
        for c in 0..nx {
            z.push(height(c as f64 * cell, r as f64 * cell));
        }
    }
    Field {
        nx,
        ny,
        ox: 0.0,
        oy: 0.0,
        cell,
        z,
        nodata: vec![false; nx * ny],
    }
}

/// A V-valley running along +Y down a gentle grade: accumulation must peak on
/// the axis, and the peak must be at least an order of magnitude above the
/// walls one valley half-width out.
fn self_check_v_valley() {
    let nx = 61;
    let ny = 61;
    let cell = 1.0;
    let axis_x = 30.0;
    let field = synthetic_field(nx, ny, cell, |x, y| {
        0.20 * (x - axis_x).abs() - 0.02 * y + 5.0
    });
    let filled = priority_flood_epsilon(&field);
    let receivers = d8_receivers(&field, &filled);
    let acc = d8_accumulation(&field, &filled, &receivers);
    let row = ny - 2;
    let on_axis = acc[row * nx + 30];
    let off_axis = acc[row * nx + 40];
    assert!(
        on_axis > 10.0 * off_axis.max(1.0),
        "V-valley self-check: axis accumulation {on_axis} not >10x the wall's {off_axis}"
    );
    eprintln!("   self-check V-valley: axis acc {on_axis:.0} vs wall acc {off_axis:.0} — PASS");
}

/// Two closed pits on a grade. Plain D8 dies inside a filled flat; the
/// +epsilon fill must route flow across both and out of the domain.
fn self_check_two_lakes() {
    let nx = 61;
    let ny = 61;
    let cell = 1.0;
    let field = synthetic_field(nx, ny, cell, |x, y| {
        let mut z = 10.0 - 0.05 * y;
        for cy in [20.0, 42.0] {
            let d = ((x - 30.0).powi(2) + (y - cy).powi(2)).sqrt();
            if d < 6.0 {
                z -= 3.0 * (1.0 - d / 6.0);
            }
        }
        z
    });
    let filled = priority_flood_epsilon(&field);
    let receivers = d8_receivers(&field, &filled);
    let acc = d8_accumulation(&field, &filled, &receivers);
    let lake_cells: Vec<usize> = (0..field.len())
        .filter(|&i| {
            let p = field.xy(i);
            ((p.x - 30.0).powi(2) + (p.y - 20.0).powi(2)).sqrt() < 5.0
        })
        .collect();
    let stalled = lake_cells
        .iter()
        .filter(|&&i| receivers[i].is_none())
        .count();
    assert_eq!(
        stalled, 0,
        "two-lake self-check: {stalled} lake cells have no D8 receiver after the +eps fill"
    );
    // Mass conservation: every data cell reaches an outlet exactly once, so
    // the outlet accumulations sum to the data-cell count. This is the check
    // that the fill + D8 graph is complete and acyclic — a stronger and less
    // brittle statement than "cell X carries the lake", which depends on
    // WHICH rim the flood spilled over and is not a property of the method.
    let outlet_total: f64 = (0..field.len())
        .filter(|&i| receivers[i].is_none())
        .map(|i| acc[i])
        .sum();
    let data_cells = field.len() as f64;
    assert!(
        (outlet_total - data_cells).abs() < 0.5,
        "two-lake self-check: outlet accumulation {outlet_total} != {data_cells} data cells"
    );
    // Downstream of the upper lake something must carry more than the lake.
    let downstream = (0..field.len())
        .filter(|&i| field.xy(i).y > 26.0)
        .map(|i| acc[i])
        .fold(0.0f64, f64::max);
    assert!(
        downstream > lake_cells.len() as f64,
        "two-lake self-check: peak downstream accumulation {downstream} did not carry the \
         upper lake's {} cells",
        lake_cells.len()
    );
    eprintln!(
        "   self-check two lakes: 0 stalled cells, outlet mass {outlet_total:.0} of \
         {data_cells:.0}, peak downstream acc {downstream:.0} vs lake {} cells — PASS",
        lake_cells.len()
    );
}

/// A strip of known width: the chamfer DT's maximum must be that width / 2.
fn self_check_chamfer_strip() {
    let nx = 41;
    let ny = 41;
    let mut mask = vec![false; nx * ny];
    // Rows 15..=25 set: an 11-cell strip, half-width (11+1)/2 = 6 cells at
    // the centre row under the chamfer convention (distance to the nearest
    // UNSET cell, which is row 14 / row 26).
    for r in 15..=25 {
        for c in 0..nx {
            mask[r * nx + c] = true;
        }
    }
    let dt = chamfer_distance(&mask, nx, ny);
    let centre = dt[20 * nx + 20];
    assert!(
        (centre - 6.0).abs() < 1e-9,
        "chamfer self-check: 11-cell strip centre read {centre}, expected 6"
    );
    eprintln!("   self-check chamfer DT: 11-cell strip centre {centre:.3} cells — PASS");
}

// ═══════════════════════════════════════════════════════════════════════
// Part 6 — SVG
// ═══════════════════════════════════════════════════════════════════════

fn svg_output_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/valley_census_h0");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn path_from_loop(points: &[P2], y_flip: f64, close: bool) -> String {
    let mut out = String::new();
    for (i, p) in points.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(out, "{cmd}{:.3},{:.3} ", p.x, y_flip - p.y);
    }
    if close {
        out.push('Z');
    }
    out
}

/// Downsample a bool grid by an integer factor (any set cell wins), so the
/// SVG's marching squares run on a few hundred cells per side instead of a
/// few thousand.
fn downsample_bool(
    mask: &[bool],
    nx: usize,
    ny: usize,
    factor: usize,
) -> (Vec<bool>, usize, usize) {
    let ox = nx.div_ceil(factor);
    let oy = ny.div_ceil(factor);
    let mut out = vec![false; ox * oy];
    // An absent layer is passed as an empty slice (the per-mask SVGs do that
    // for the mask they are not showing), which is "nothing set", not a
    // shape mismatch.
    if mask.len() < nx * ny {
        return (out, ox, oy);
    }
    for r in 0..ny {
        for c in 0..nx {
            if mask[r * nx + c] {
                out[(r / factor) * ox + c / factor] = true;
            }
        }
    }
    (out, ox, oy)
}

struct SvgInputs<'a> {
    field: &'a Field,
    territory: &'a [Polygon2],
    mask_a: &'a [bool],
    mask_b: &'a [bool],
    valley_lines: &'a [Vec<P2>],
    dxf_lines: &'a [Vec<P2>],
}

fn write_census_svg(inputs: &SvgInputs<'_>, path: &Path, title: &str) -> std::io::Result<()> {
    let field = inputs.field;
    let x0 = field.ox;
    let y0 = field.oy;
    let x1 = field.ox + (field.nx - 1) as f64 * field.cell;
    let y1 = field.oy + (field.ny - 1) as f64 * field.cell;
    let y_flip = y0 + y1;
    let pad = 4.0;

    let factor = ((field.nx as f64 / 400.0).ceil() as usize).max(1);
    let ds_cell = field.cell * factor as f64;

    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{:.2} {:.2} {:.2} {:.2}\" \
         width=\"1100\" height=\"1100\">\n\
         <rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"#f7f5f0\"/>\n\
         <title>{title}</title>\n",
        x0 - pad,
        y0 - pad,
        x1 - x0 + 2.0 * pad,
        y1 - y0 + 2.0 * pad,
        x0 - pad,
        y0 - pad,
        x1 - x0 + 2.0 * pad,
        y1 - y0 + 2.0 * pad,
    );

    // Height bands: eight nested "above level k" masks, drawn as filled
    // regions from the lowest level up so the higher bands paint over.
    let mut zmin = f64::INFINITY;
    let mut zmax = f64::NEG_INFINITY;
    for i in 0..field.len() {
        if !field.nodata[i] {
            zmin = zmin.min(field.z[i]);
            zmax = zmax.max(field.z[i]);
        }
    }
    const BANDS: usize = 8;
    let _ = writeln!(svg, "<g id=\"height-bands\">");
    for band in 0..BANDS {
        let level = zmin + (zmax - zmin) * band as f64 / BANDS as f64;
        let mask: Vec<bool> = (0..field.len())
            .map(|i| !field.nodata[i] && field.z[i] >= level)
            .collect();
        let (ds, dnx, dny) = downsample_bool(&mask, field.nx, field.ny, factor);
        let loops = marching_squares_bool_grid(&ds, dny, dnx, field.ox, field.oy, ds_cell);
        let shade = 235 - band * 16;
        let mut d = String::new();
        for lp in &loops {
            if lp.len() >= 3 {
                d.push_str(&path_from_loop(lp, y_flip, true));
            }
        }
        if !d.is_empty() {
            let _ = writeln!(
                svg,
                "<path d=\"{d}\" fill=\"rgb({shade},{},{})\" fill-rule=\"evenodd\" stroke=\"none\"/>",
                shade.saturating_sub(6),
                shade.saturating_sub(20)
            );
        }
    }
    let _ = writeln!(svg, "</g>");

    // Finish territory outline.
    let _ = writeln!(svg, "<g id=\"territory\">");
    for poly in inputs.territory {
        let mut d = path_from_loop(&poly.exterior, y_flip, true);
        for hole in &poly.holes {
            d.push_str(&path_from_loop(hole, y_flip, true));
        }
        let _ = writeln!(
            svg,
            "<path d=\"{d}\" fill=\"none\" fill-rule=\"evenodd\" stroke=\"#2b2b2b\" \
             stroke-width=\"0.35\"/>"
        );
    }
    let _ = writeln!(svg, "</g>");

    // Masks.
    for (label, mask, fill) in [
        ("mask-a-dxf", inputs.mask_a, "#e07b39"),
        ("mask-b-flow", inputs.mask_b, "#2f6fb5"),
    ] {
        let (ds, dnx, dny) = downsample_bool(mask, field.nx, field.ny, factor);
        let loops = marching_squares_bool_grid(&ds, dny, dnx, field.ox, field.oy, ds_cell);
        let mut d = String::new();
        for lp in &loops {
            if lp.len() >= 3 {
                d.push_str(&path_from_loop(lp, y_flip, true));
            }
        }
        let _ = writeln!(
            svg,
            "<g id=\"{label}\"><path d=\"{d}\" fill=\"{fill}\" fill-opacity=\"0.30\" \
             fill-rule=\"evenodd\" stroke=\"{fill}\" stroke-width=\"0.20\" \
             stroke-opacity=\"0.6\"/></g>"
        );
    }

    // Extracted lines.
    let _ = writeln!(
        svg,
        "<g id=\"valley-lines\" fill=\"none\" stroke=\"#123c66\" stroke-width=\"0.30\">"
    );
    for line in inputs.valley_lines {
        if line.len() >= 2 {
            let _ = writeln!(svg, "<path d=\"{}\"/>", path_from_loop(line, y_flip, false));
        }
    }
    let _ = writeln!(svg, "</g>");
    let _ = writeln!(
        svg,
        "<g id=\"dxf-rivers\" fill=\"none\" stroke=\"#8a3d0d\" stroke-width=\"0.30\" \
         stroke-dasharray=\"1.2 0.8\">"
    );
    for line in inputs.dxf_lines {
        if line.len() >= 2 {
            let _ = writeln!(svg, "<path d=\"{}\"/>", path_from_loop(line, y_flip, false));
        }
    }
    let _ = writeln!(svg, "</g>\n</svg>");
    std::fs::write(path, svg)
}

// ═══════════════════════════════════════════════════════════════════════
// The instrument
// ═══════════════════════════════════════════════════════════════════════

/// Everything the census needs about one costed Shallow region. Every field
/// here is MASK-INDEPENDENT, so the costing runs once and every mask is
/// evaluated against the same toolpaths.
struct RegionRow {
    index: usize,
    polygon: Polygon2,
    area_mm2: f64,
    direction_deg: f64,
    rotated: bool,
    elongation: Option<f64>,
    theta_max_incl_deg: f64,
    incl_cells: usize,
    derated_stepover_mm: f64,
    cost: Option<CandidateCost>,
}

/// The shared, mask-independent inputs [`evaluate_mask`] reads.
struct EvalCtx<'a> {
    mesh: &'a TriangleMesh,
    index: &'a SpatialIndex,
    field: &'a Field,
    surface: &'a rs_cam_core::finish_setup::FinishSurface,
    covered: &'a [bool],
    steep_clamp_deg: f64,
    stepover: f64,
    cell_area: f64,
    territory_cells: usize,
    territory_pts: &'a [P2],
    /// Index into `rows` of the region owning each grid cell, or `-1`.
    /// V0-att's `s_shipped` is a per-region dial, so the attribution arm
    /// cannot be computed without it.
    region_of: &'a [i32],
    kinematics: &'a rs_cam_core::machine_kinematics::MachineKinematics,
    rows: &'a [RegionRow],
}

/// How one mask was built — printed with it, so no number stands without the
/// construction that produced it.
struct MaskProvenance<'a> {
    label: String,
    samples: usize,
    floored: usize,
    skipped: usize,
    half_widths_mm: &'a [f64],
}

/// The valley tangent (degrees, axis in `[0, 180)`) at every network cell,
/// measured over a `reach_cells` window ALONG THE CHAIN in both directions —
/// never from a single D8 link, which quantises to 45 degrees.
///
/// Returns the tangent field and the main-donor map (the upstream neighbour
/// with the largest accumulation), which the polyline tracer also needs.
fn valley_tangents(
    field: &Field,
    receivers: &[Option<u32>],
    acc: &[f64],
    network: &[bool],
    reach_cells: usize,
) -> (Vec<f64>, Vec<u32>) {
    let mut main_donor = vec![u32::MAX; field.len()];
    let mut donor_acc = vec![f64::NEG_INFINITY; field.len()];
    for i in 0..field.len() {
        if !network[i] {
            continue;
        }
        if let Some(r) = receivers[i]
            && network[r as usize]
            && acc[i] > donor_acc[r as usize]
        {
            donor_acc[r as usize] = acc[i];
            main_donor[r as usize] = i as u32;
        }
    }
    let mut tangent_deg = vec![f64::NAN; field.len()];
    for i in 0..field.len() {
        if !network[i] {
            continue;
        }
        let mut down = i;
        for _ in 0..reach_cells {
            match receivers[down] {
                Some(r) if network[r as usize] => down = r as usize,
                _ => break,
            }
        }
        let mut up = i;
        for _ in 0..reach_cells {
            let d = main_donor[up];
            if d == u32::MAX {
                break;
            }
            up = d as usize;
        }
        if up == down {
            continue;
        }
        let a = field.xy(up);
        let b = field.xy(down);
        tangent_deg[i] = (b.y - a.y).atan2(b.x - a.x).to_degrees().rem_euclid(180.0);
    }
    (tangent_deg, main_donor)
}

/// Trace the network into polylines: from every head (a cell with no donor)
/// downstream until the network ends or the chain meets an already-traced
/// cell.
fn valley_polylines(
    field: &Field,
    receivers: &[Option<u32>],
    network: &[bool],
    main_donor: &[u32],
) -> Vec<Vec<P2>> {
    let mut out = Vec::new();
    let mut visited = vec![false; field.len()];
    for i in 0..field.len() {
        if !network[i] || visited[i] || main_donor[i] != u32::MAX {
            continue;
        }
        let mut line = Vec::new();
        let mut c = i;
        loop {
            visited[c] = true;
            line.push(field.xy(c));
            match receivers[c] {
                Some(r) if network[r as usize] && !visited[r as usize] => c = r as usize,
                Some(r) if network[r as usize] => {
                    line.push(field.xy(r as usize));
                    break;
                }
                _ => break,
            }
        }
        if line.len() >= 2 {
            out.push(line);
        }
    }
    out
}

fn census_samples(mesh: &TriangleMesh, index: &SpatialIndex, pts: &[P2]) -> Vec<(P2, f64)> {
    pts.par_iter()
        .filter_map(|&p| surface_z(mesh, index, p).map(|z| (p, z)))
        .collect()
}

fn print_census_header() {
    eprintln!(
        "     {:>12}  {:>6}  {:>7}  {:>9}  {:>9}  {:>9}  {:>9}  {:>9}",
        "population", "R mm", "fits", "gouge A", "bound X", "bound Y", "bound PCA", "ratio p50"
    );
}

fn print_census_rows(mesh: &TriangleMesh, index: &SpatialIndex, name: &str, samples: &[(P2, f64)]) {
    if samples.is_empty() {
        eprintln!("     {name:>12}  EMPTY POPULATION — nothing measured");
        return;
    }
    let (cells, total, rejected) = census_population(mesh, index, samples, CENSUS_LATTICE_MM);
    for cell in &cells {
        eprintln!(
            "     {name:>12}  {:>6.1}  {:>7}  {:>8.3}%  {:>9.4}  {:>9.4}  {:>9.4}  {:>9.4}",
            cell.tool_radius_mm,
            cell.fits,
            100.0 * cell.gouge_area_frac,
            cell.bound_x,
            cell.bound_y,
            cell.bound_pca,
            cell.ratio_p50,
        );
    }
    eprintln!(
        "     {name:>12}  ({rejected} of {total} samples rejected: under-determined or \
         ill-conditioned; ratio p90 {:.4} at R{:.1})",
        cells.last().map_or(f64::NAN, |c| c.ratio_p90),
        cells.last().map_or(f64::NAN, |c| c.tool_radius_mm),
    );
}

/// One pass of the V0-att v2 XY-pitch model over one population.
///
/// Every accumulation happens in ONE loop over the SAME cells, so the model,
/// the bracket and `L_min` share a population by construction.
#[derive(Default)]
struct XyModel {
    /// `SUM dA` — surface area (dA = a_xy / cos theta).
    surface_area_mm2: f64,
    /// `SUM dA / s_shipped` — v1's numerator, now the bracket's UPPER side.
    surface_len_mm: f64,
    /// `SUM a_xy / s_shipped` — every `sec phi` forced to 1, the
    /// direction-independent LOWER bound's numerator.
    flat_len_mm: f64,
    /// `SUM (a_xy / s_shipped) * sec phi` — the model itself.
    len_mm: f64,
    /// Cells whose direction chooser gave no direction and fell back to
    /// `d_shipped`. They contribute exactly zero to `D_pot`.
    fallback_cells: usize,
    /// Cells with no owning region — excluded from every numerator, never
    /// folded in at the spec spacing.
    unowned_cells: usize,
    /// Cells whose normal is too near horizontal-facing to invert.
    degenerate_cells: usize,
}

/// Accumulate [`XyModel`] over `mask`. `dir_deg_at(cell, shipped_deg)`
/// returns the pass direction in degrees for that cell; returning a
/// non-finite value asks for the `d_shipped` fallback and is counted.
fn xy_pitch_model(
    ctx: &EvalCtx<'_>,
    mask: &[bool],
    dir_deg_at: &dyn Fn(usize, f64) -> f64,
) -> XyModel {
    /// Below this the surface faces sideways and `grad z` cannot be formed.
    /// The classification mesh is 100 % up-facing, so this is a guard, not a
    /// population.
    const MIN_NZ: f64 = 1e-6;

    let mut out = XyModel::default();
    let slope = &ctx.surface.slope_map;
    for (i, &angle) in slope.angles.iter().enumerate() {
        if !mask.get(i).copied().unwrap_or(false) {
            continue;
        }
        let cos = angle.cos();
        if cos <= 1e-6 {
            continue;
        }
        let d_area = ctx.cell_area / cos;
        out.surface_area_mm2 += d_area;
        let owner = ctx.region_of.get(i).copied().unwrap_or(-1);
        let Some(row) = usize::try_from(owner).ok().and_then(|k| ctx.rows.get(k)) else {
            out.unowned_cells += 1;
            continue;
        };
        let s_shipped = row.derated_stepover_mm;
        out.surface_len_mm += d_area / s_shipped;
        out.flat_len_mm += ctx.cell_area / s_shipped;

        let Some(normal) = slope.normals.get(i) else {
            out.degenerate_cells += 1;
            out.len_mm += ctx.cell_area / s_shipped;
            continue;
        };
        if normal.z.abs() < MIN_NZ {
            out.degenerate_cells += 1;
            out.len_mm += ctx.cell_area / s_shipped;
            continue;
        }
        // Heightfield gradient from the upward unit normal.
        let f_x = -normal.x / normal.z;
        let f_y = -normal.y / normal.z;
        let mut deg = dir_deg_at(i, row.direction_deg);
        if !deg.is_finite() {
            out.fallback_cells += 1;
            deg = row.direction_deg;
        }
        let (sin_d, cos_d) = deg.to_radians().sin_cos();
        // tan phi = |grad z . d|; sec phi = sqrt(1 + (grad z . d)^2).
        let tan_phi = f_x * cos_d + f_y * sin_d;
        let sec_phi = (1.0 + tan_phi * tan_phi).sqrt();
        out.len_mm += (ctx.cell_area / s_shipped) * sec_phi;
    }
    out
}

/// Everything mask-dependent: area share, V0-pre, M4, M1, M2, M3.
fn evaluate_mask(
    ctx: &EvalCtx<'_>,
    prov: &MaskProvenance<'_>,
    mask: &[bool],
    axis_at: &dyn Fn(usize) -> f64,
) {
    let field = ctx.field;
    let cells = mask.iter().filter(|&&m| m).count();
    let pick = |w: &[f64], q: f64| -> f64 {
        if w.is_empty() {
            f64::NAN
        } else {
            w[((w.len() - 1) as f64 * q).round() as usize]
        }
    };
    eprintln!(
        "\n══════════ {} ══════════\n\
         \x20  AREA: {cells} cells = {:.1} mm² XY = {:.2} % of the finish territory \
         ({} cells)\n\
         \x20  built from {} samples, {} floored to one cell, {} redundant discs skipped;\n\
         \x20  half-width p10 {:.2}  p50 {:.2}  p90 {:.2}  max {:.2} mm",
        prov.label,
        cells as f64 * ctx.cell_area,
        100.0 * cells as f64 / ctx.territory_cells.max(1) as f64,
        ctx.territory_cells,
        prov.samples,
        prov.floored,
        prov.skipped,
        pick(prov.half_widths_mm, 0.10),
        pick(prov.half_widths_mm, 0.50),
        pick(prov.half_widths_mm, 0.90),
        prov.half_widths_mm.last().copied().unwrap_or(f64::NAN),
    );

    // ── V0-pre, restricted to this mask ──
    let in_mask_pts: Vec<P2> = ctx
        .territory_pts
        .iter()
        .copied()
        .filter(|p| field.index_at(p.x, p.y).is_some_and(|i| mask[i]))
        .collect();
    let in_mask_samples = census_samples(ctx.mesh, ctx.index, &in_mask_pts);
    eprintln!("\x20  V0-pre — direction-prize ceiling restricted to this mask:");
    print_census_header();
    print_census_rows(ctx.mesh, ctx.index, "in-mask", &in_mask_samples);

    // ── M4 ──
    let inside = |p: P2| -> bool { field.index_at(p.x, p.y).is_some_and(|i| mask[i]) };
    eprintln!(
        "\x20  M4 — theta_max with mask cells EXCISED vs INCLUDED \
         (clamp {:.1} deg); refund = cos(excised)/cos(included):",
        ctx.steep_clamp_deg
    );
    eprintln!(
        "     {:>4}  {:>10}  {:>8}  {:>7}  {:>10}  {:>9}  {:>9}  {:>9}  {:>8}  {:>16}",
        "rgn",
        "area mm²",
        "dir deg",
        "rotated",
        "elong",
        "th incl",
        "th excl",
        "th mask",
        "refund",
        "mask/incl cells"
    );
    for row in ctx.rows {
        let (excl, _) = region_max_slope_deg(
            ctx.surface,
            ctx.covered,
            &row.polygon,
            ctx.steep_clamp_deg,
            &|p| !inside(p),
        );
        let (only, only_n) = region_max_slope_deg(
            ctx.surface,
            ctx.covered,
            &row.polygon,
            ctx.steep_clamp_deg,
            &inside,
        );
        let refund = excl.to_radians().cos() / row.theta_max_incl_deg.to_radians().cos().max(1e-12);
        eprintln!(
            "     {:>4}  {:>10.1}  {:>8.2}  {:>7}  {:>10}  {:>9.3}  {:>9.3}  {:>9.3}  {:>8.4}  \
             {:>7} /{:>8}",
            row.index,
            row.area_mm2,
            row.direction_deg,
            row.rotated,
            row.elongation
                .map_or_else(|| "-".to_owned(), |e| format!("{e:.2}")),
            row.theta_max_incl_deg,
            excl,
            only,
            refund,
            only_n,
            row.incl_cells,
        );
    }

    // ── M1, M2, M3 ──
    let mut total = Split::default();
    let mut scales = Vec::new();
    let mut weighted_angle = 0.0f64;
    let mut weighted_angle_moves = 0.0f64;
    let mut angle_weight = 0.0f64;
    let mut no_tangent_time = 0.0f64;
    for row in ctx.rows {
        let Some(cost) = row.cost.as_ref() else {
            continue;
        };
        let (split, samples, scale) =
            attribute_cutting(&cost.toolpath, ctx.kinematics, cost.time_s, &inside);
        scales.push(scale);
        total.in_time_s += split.in_time_s;
        total.out_time_s += split.out_time_s;
        total.in_mm += split.in_mm;
        total.out_mm += split.out_mm;
        total.in_all_time_s += split.in_all_time_s;
        total.out_all_time_s += split.out_all_time_s;
        total.in_all_mm += split.in_all_mm;
        total.out_all_mm += split.out_all_mm;
        for sample in &samples {
            let Some(i) = field.index_at(sample.mid.x, sample.mid.y) else {
                continue;
            };
            if !mask[i] {
                continue;
            }
            let axis = axis_at(i);
            if !axis.is_finite() {
                no_tangent_time += sample.time_s;
                continue;
            }
            weighted_angle += axis_angle_deg(row.direction_deg, axis) * sample.time_s;
            weighted_angle_moves += axis_angle_deg(sample.dir_deg, axis) * sample.time_s;
            angle_weight += sample.time_s;
        }
    }
    let cutting_time = total.in_time_s + total.out_time_s;
    let cutting_mm = total.in_mm + total.out_mm;

    // The V0-att v2 model, the v1 upper bound and `L_min` all come out of one
    // pass over the SAME mask cells, so every ratio below shares a population
    // and a denominator by construction rather than by agreement.
    let shipped = xy_pitch_model(ctx, mask, &|_, shipped_deg| shipped_deg);
    let valley = xy_pitch_model(ctx, mask, &|i, _| axis_at(i));
    let surface_area = shipped.surface_area_mm2;
    let spacing_len = shipped.surface_len_mm;
    let unowned_cells = shipped.unowned_cells;
    let l_min = surface_area / ctx.stepover;
    let m2_spacing = spacing_len / l_min.max(1e-9);
    let lower_bound = shipped.flat_len_mm / l_min.max(1e-9);
    let m2_xy_shipped = shipped.len_mm / l_min.max(1e-9);
    let m2_xy_valley = valley.len_mm / l_min.max(1e-9);
    let measured_m2 = total.in_mm / l_min.max(1e-9);
    let r_topo = measured_m2 - m2_xy_shipped;
    let d_pot = m2_xy_shipped - m2_xy_valley;
    let bracket_ok = lower_bound <= measured_m2 && measured_m2 <= m2_spacing;
    let feed_mm_s = FEED_MM_MIN / 60.0;

    let all_time = total.in_all_time_s + total.out_all_time_s;
    let all_mm = total.in_all_mm + total.out_all_mm;
    eprintln!(
        "\x20  time attribution rescale factor (sum len/v_peak vs compute_cycle_time): \
         min {:.4} max {:.4}\n\
         \x20  M1 time share IN mask, CUT-INTENT moves: {:.3} % ({:.1} s of {:.1} s)\n\
         \x20     distance share, cut-intent: {:.3} % ({:.0} mm of {:.0} mm)  [cross-check]\n\
         \x20  M1 time share IN mask, ALL NON-RAPID moves (links included): {:.3} % \
         ({:.1} s of {:.1} s)\n\
         \x20     distance share, all non-rapid: {:.3} % ({:.0} mm of {:.0} mm)  [cross-check]\n\
         \x20  mask surface area (dA = dxdy/cos slope): {:.1} mm²; s_max = {:.6} mm\n\
         \x20     L_min = {:.1} mm;  L_min / feed = {:.1} s at {FEED_MM_MIN:.0} mm/min\n\
         \x20  M2 xfloor, DISTANCE / cut-intent only:        {:.4}x\n\
         \x20  M2 xfloor, DISTANCE / all non-rapid:          {:.4}x   <-- the synthesis §1 \
         convention (total_cutting_distance)\n\
         \x20  M2 xfloor, TIME / cut-intent only:            {:.4}x\n\
         \x20  M2 xfloor, TIME / all non-rapid:              {:.4}x   <-- the literal FINDINGS \
         wording\n\
         \x20  V0-att v1 (RETIRED, kept as the bracket's UPPER bound)\n\
         \x20     M2_spacing, surface-pitch: {:.4}x   \
         (integral dA/s_shipped = {:.1} mm, L_min {:.1} mm, {} unowned cells)\n\
         \x20     v1 residual M2 - M2_spacing = {:.4}x = {:.2} pp of M2's {:.2} pp excess\n\
         \x20  ---- V0-att v2, direction-aware XY-pitch model ----\n\
         \x20  BRACKET  lower {:.4}x  <=  measured M2 {:.4}x  <=  upper {:.4}x   {}\n\
         \x20  M2_xy(d_shipped)  = {:.4}x   (SUM (a_xy/s_shipped)*sec phi = {:.1} mm)\n\
         \x20  M2_xy(d_valley)   = {:.4}x   ({} cells fell back to d_shipped, \
         {} degenerate, {} unowned)\n\
         \x20  R_topo = M2 - M2_xy(d_shipped)          = {:+.4}x = {:+.2} pp   [context only]\n\
         \x20  D_pot  = M2_xy(d_shipped) - M2_xy(d_valley) = {:.4}x = {:.2} pp   \
         [UPPER bound on the direction prize]\n\
         \x20  M3 time-weighted misalignment, region C2 lattice vs local valley axis: {:.3} deg\n\
         \x20     (emitted-move direction vs valley axis: {:.3} deg)  [cross-check]\n\
         \x20     (an isotropic axis field would read 45.000 deg)\n\
         \x20     weight {:.1} s; {:.1} s of in-mask time had no valley tangent",
        scales.iter().copied().fold(f64::INFINITY, f64::min),
        scales.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        100.0 * total.in_time_s / cutting_time.max(1e-9),
        total.in_time_s,
        cutting_time,
        100.0 * total.in_mm / cutting_mm.max(1e-9),
        total.in_mm,
        cutting_mm,
        100.0 * total.in_all_time_s / all_time.max(1e-9),
        total.in_all_time_s,
        all_time,
        100.0 * total.in_all_mm / all_mm.max(1e-9),
        total.in_all_mm,
        all_mm,
        surface_area,
        ctx.stepover,
        l_min,
        l_min / feed_mm_s,
        total.in_mm / l_min.max(1e-9),
        total.in_all_mm / l_min.max(1e-9),
        total.in_time_s / (l_min / feed_mm_s).max(1e-9),
        total.in_all_time_s / (l_min / feed_mm_s).max(1e-9),
        m2_spacing,
        spacing_len,
        l_min,
        unowned_cells,
        measured_m2 - m2_spacing,
        100.0 * (measured_m2 - m2_spacing),
        100.0 * (measured_m2 - 1.0),
        lower_bound,
        measured_m2,
        m2_spacing,
        if bracket_ok {
            "BRACKET OK"
        } else {
            "BRACKET VIOLATED"
        },
        m2_xy_shipped,
        shipped.len_mm,
        m2_xy_valley,
        valley.fallback_cells,
        valley.degenerate_cells,
        valley.unowned_cells,
        r_topo,
        100.0 * r_topo,
        d_pot,
        100.0 * d_pot,
        weighted_angle / angle_weight.max(1e-9),
        weighted_angle_moves / angle_weight.max(1e-9),
        angle_weight,
        no_tangent_time,
    );
}

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh + rivers DXF (not in repo)"]
fn wanaka_valley_prize_census_h0() {
    eprintln!(
        "\n========== Track H V0 — VALLEY PRIZE CENSUS ==========\n\
         Pre-registration: planning/valley_tracing_2026-09-02/FINDINGS.md (Q1).\n\
         This instrument reports NUMBERS ONLY. It writes no verdict.\n"
    );

    eprintln!("---------- self-checks (before the mesh load) ----------");
    self_check_v_valley();
    self_check_two_lakes();
    self_check_chamfer_strip();
    eprintln!();

    let mesh_path = Path::new(WANAKA_MESH);
    let dxf_path = Path::new(RIVERS_DXF);
    if !mesh_path.exists() {
        eprintln!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }
    if !dxf_path.exists() {
        eprintln!("SKIP: {RIVERS_DXF} not present on this machine.");
        return;
    }

    // ── setup: the shipped tier ladder and the tier-1 Shallow band ──
    let started = std::time::Instant::now();
    let mesh = TriangleMesh::from_stl_scaled(mesh_path, 1.0).expect("load wanaka terrain");
    let index = SpatialIndex::build_auto(&mesh);
    eprintln!(
        "mesh: {} triangles, bbox x [{:.2}, {:.2}] y [{:.2}, {:.2}] z [{:.2}, {:.2}]  ({:.1}s)",
        mesh.triangles.len(),
        mesh.bbox.min.x,
        mesh.bbox.max.x,
        mesh.bbox.min.y,
        mesh.bbox.max.y,
        mesh.bbox.min.z,
        mesh.bbox.max.z,
        started.elapsed().as_secs_f64()
    );

    let r15 = TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5);
    let r10 = TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0);
    let never_cancel = || false;
    let (tier_map, cusp_radii) = {
        let tools: [&dyn MillingCutter; 2] = [&r15, &r10];
        let ladder = TierLadder::new(&tools).expect("ladder");
        let map = compute_tier_map(
            &mesh,
            &index,
            &ladder,
            &TierMapParams {
                cell_mm: CELL_MM,
                tolerance_mm: TOLERANCE_MM,
                margin_mm: MARGIN_MM,
                treatment: ResidualTreatment::SlopeCompensated,
            },
            &never_cancel,
        )
        .expect("tier map");
        let radii: Vec<f64> = tools.iter().map(|t| t.cusp_radius_mm()).collect();
        (map, radii)
    };
    let islands = extract_tier_islands(
        &tier_map,
        &TierIslandParams {
            coarseness: COARSENESS,
            overlap_mm: OVERLAP_MM,
            max_regions_per_tier: MAX_REGIONS_PER_TIER,
            ..TierIslandParams::default()
        },
        &cusp_radii,
    )
    .expect("islands");
    let Some(fine_island) = islands.per_tier.iter().find(|set| set.tier == 1) else {
        eprintln!("SKIP: no tier 1 machining region.");
        return;
    };

    let surface = build_classification_surface_with_sampler_and_cancel(
        &mesh,
        &index,
        &r10,
        unified_finish_classification_resolution(&r10, OP_TOLERANCE_MM),
        ClassificationSampler::PRODUCTION,
        &never_cancel,
    )
    .expect("classification surface");
    let heightmap = &surface.heightmap;
    let covered: Vec<bool> = heightmap
        .covered_flags()
        .iter()
        .enumerate()
        .map(|(i, &flag)| {
            if !flag {
                return false;
            }
            let row = i / heightmap.cols;
            let col = i % heightmap.cols;
            fine_island.machining.contains(&P2::new(
                heightmap.origin_x + col as f64 * heightmap.cell_size,
                heightmap.origin_y + row as f64 * heightmap.cell_size,
            ))
        })
        .collect();
    let mut planner = FinishPlannerParams::for_tool(cusp_radii[1]);
    planner.overlap_mm = OVERLAP_MM;
    let planned = decompose(&surface.slope_map, &covered, &[], &planner);
    let stepover =
        2.0 * (2.0 * cusp_radii[1] * CUSP_HEIGHT_MM - CUSP_HEIGHT_MM * CUSP_HEIGHT_MM).sqrt();

    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::Shallow)
        .map(|r| &r.polygon)
        .collect();
    shallow.sort_by(|a, b| b.area().total_cmp(&a.area()));
    let shallow_total_area: f64 = shallow.iter().map(|p| p.area()).sum();
    eprintln!(
        "setup: {} planned regions, {} in the Shallow band ({:.0} mm² XY total);\n\
         \x20  tier-1 equal-cusp stepover {stepover:.6} mm at h = {CUSP_HEIGHT_MM} mm.\n\
         \x20  classification grid {} x {} at {:.4} mm.",
        planned.regions.len(),
        shallow.len(),
        shallow_total_area,
        surface.rows(),
        surface.cols(),
        surface.cell_size(),
    );

    let costed: Vec<Polygon2> = shallow
        .iter()
        .take(MAX_COSTED_REGIONS)
        .map(|p| (*p).clone())
        .collect();
    let costed_area: f64 = costed.iter().map(Polygon2::area).sum();
    eprintln!(
        "\x20  COSTED SCOPE: the {} largest Shallow regions = {:.1} % of the band's XY area.\n\
         \x20  (M1's denominator is this scope, not tier0+tier1 all bands — see the header.)\n",
        costed.len(),
        100.0 * costed_area / shallow_total_area.max(1e-9)
    );

    // ── the rasterised heightfield: the classification grid itself ──
    let field = Field {
        nx: heightmap.cols,
        ny: heightmap.rows,
        ox: heightmap.origin_x,
        oy: heightmap.origin_y,
        cell: heightmap.cell_size,
        z: (0..heightmap.rows * heightmap.cols)
            .map(|i| heightmap.z_at_index(i).covered().unwrap_or(f64::NAN))
            .collect(),
        nodata: heightmap
            .covered_flags()
            .iter()
            .map(|&flag| !flag)
            .collect(),
    };
    let data_cells = field.nodata.iter().filter(|&&n| !n).count();
    let land: Vec<bool> = (0..field.len())
        .map(|i| !field.nodata[i] && field.z[i] > LAND_Z_MM)
        .collect();
    let land_cells = land.iter().filter(|&&l| l).count();
    eprintln!(
        "heightfield: {} x {} at {:.4} mm; {data_cells} data cells, {land_cells} of them land \
         (z > {LAND_Z_MM}).",
        field.nx, field.ny, field.cell
    );

    // ── HYDROLOGY ──
    //
    // Two floods. The WHOLE-BOARD one is the diagnostic that forced the land
    // restriction and is reported, never used; the LAND one is the deciding
    // arm. See `Field::land_view`.
    let t_hydro = std::time::Instant::now();
    let whole_filled = priority_flood_epsilon(&field);
    let whole_raised = (0..field.len())
        .filter(|&i| !field.nodata[i] && whole_filled[i] > field.z[i] + FILL_EPSILON_MM)
        .count();
    drop(whole_filled);

    let hydro = field.land_view(LAND_Z_MM);
    let filled = priority_flood_epsilon(&hydro);
    let raised = (0..hydro.len())
        .filter(|&i| !hydro.nodata[i] && filled[i] > hydro.z[i] + FILL_EPSILON_MM)
        .count();
    let receivers = d8_receivers(&hydro, &filled);
    let stalled = (0..hydro.len())
        .filter(|&i| !hydro.nodata[i] && receivers[i].is_none())
        .count();
    let acc = d8_accumulation(&hydro, &filled, &receivers);
    eprintln!(
        "hydrology, WHOLE-BOARD flood (diagnostic, NOT used): raised {whole_raised} cells \
         ({:.2} % of data).\n\
         \x20  The rivmap export writes a RAISED OUTER EDGE BAND (`edge_profile = 3`, \
         `edge_wall_deg = 41`),\n\
         \x20  so the interior is one closed basin and a correct priority flood fills it to \
         the rim.\n\
         \x20  The accumulation field that produces is an artefact of flood order, not a \
         drainage network.\n\
         hydrology, LAND flood (the DECIDING arm; sea/trench is base level, coastline is the \
         outlet):\n\
         \x20  raised {raised} cells ({:.2} % of land); {stalled} outlet cells; done in {:.1}s.",
        100.0 * whole_raised as f64 / data_cells.max(1) as f64,
        100.0 * raised as f64 / land_cells.max(1) as f64,
        t_hydro.elapsed().as_secs_f64()
    );

    // ── threshold ladder: network length stability ──
    let cell_area = field.cell * field.cell;
    eprintln!(
        "\n---------- accumulation threshold ladder (stability evidence) ----------\n\
         \x20  Rule, pre-declared: the chosen rung is the INTERIOR rung with the flattest\n\
         \x20  |d ln L / d ln T|. L is the total D8 link length whose both ends are in the\n\
         \x20  network, over LAND cells only.\n"
    );
    eprintln!(
        "     {:>12}  {:>12}  {:>12}  {:>14}",
        "T mm²", "cells", "L mm", "|dlnL/dlnT|"
    );
    let mut lengths = Vec::new();
    for &t in &ACC_LADDER_MM2 {
        let network: Vec<bool> = (0..hydro.len())
            .map(|i| !hydro.nodata[i] && acc[i] * cell_area >= t)
            .collect();
        let cells = network.iter().filter(|&&n| n).count();
        let length = network_length_mm(&hydro, &receivers, &network);
        lengths.push((t, cells, length));
    }
    let mut slopes = vec![f64::NAN; lengths.len()];
    for k in 1..lengths.len().saturating_sub(1) {
        let (t0, _, l0) = lengths[k - 1];
        let (t1, _, l1) = lengths[k + 1];
        if l0 > 0.0 && l1 > 0.0 {
            slopes[k] = ((l0 / l1).ln() / (t1 / t0).ln()).abs();
        }
    }
    for (k, &(t, cells, length)) in lengths.iter().enumerate() {
        let slope = slopes[k];
        let slope_text = if slope.is_finite() {
            format!("{slope:14.4}")
        } else {
            format!("{:>14}", "-")
        };
        eprintln!("     {t:12.0}  {cells:12}  {length:12.1}  {slope_text}");
    }
    let chosen_k = (0..lengths.len())
        .filter(|&k| slopes[k].is_finite())
        .min_by(|&a, &b| slopes[a].total_cmp(&slopes[b]))
        .unwrap_or(lengths.len() / 2);
    let threshold_mm2 = lengths[chosen_k].0;
    eprintln!(
        "\n     CHOSEN THRESHOLD: T = {threshold_mm2:.0} mm² (rung {chosen_k}), flattest slope \
         {:.4}; L = {:.1} mm over {} cells.",
        slopes[chosen_k], lengths[chosen_k].2, lengths[chosen_k].1
    );

    let network: Vec<bool> = (0..hydro.len())
        .map(|i| !hydro.nodata[i] && acc[i] * cell_area >= threshold_mm2)
        .collect();

    // ── the low-ground mask and its chamfer DT ──
    eprintln!(
        "\n---------- low-ground mask + chamfer DT (half-width source) ----------\n\
         \x20  A data cell is LOW when its height is below the mean of a square window of\n\
         \x20  half-width W centred on it. The DT then reads, in cells, the distance from a\n\
         \x20  low cell to the nearest non-low cell — the `rest_field.rs` convention.\n"
    );
    let mut dt_for_window = Vec::new();
    for &w in &TPI_SENSITIVITY_MM {
        let low = low_ground_mask(&hydro, w);
        let low_cells = low.iter().filter(|&&l| l).count();
        let dt = chamfer_distance(&low, field.nx, field.ny);
        let mut on_net: Vec<f64> = (0..field.len())
            .filter(|&i| network[i])
            .map(|i| dt[i] * field.cell)
            .collect();
        on_net.sort_by(f64::total_cmp);
        let pick = |q: f64| -> f64 {
            if on_net.is_empty() {
                f64::NAN
            } else {
                on_net[((on_net.len() - 1) as f64 * q).round() as usize]
            }
        };
        eprintln!(
            "     W = {w:5.1} mm: low cells {low_cells} ({:.1} % of data); half-width on the \
             network p10 {:.2}  p50 {:.2}  p90 {:.2}  max {:.2} mm",
            100.0 * low_cells as f64 / data_cells.max(1) as f64,
            pick(0.10),
            pick(0.50),
            pick(0.90),
            on_net.last().copied().unwrap_or(f64::NAN),
        );
        if (w - TPI_HALF_WINDOW_MM).abs() < 1e-9 {
            dt_for_window = dt;
        }
    }
    if dt_for_window.is_empty() {
        dt_for_window = chamfer_distance(
            &low_ground_mask(&hydro, TPI_HALF_WINDOW_MM),
            field.nx,
            field.ny,
        );
    }
    eprintln!("     (the DECIDING window is W = {TPI_HALF_WINDOW_MM} mm)");

    // ── mask A: buffer the DXF network (threshold-independent) ──
    let dxf_polys = rs_cam_core::dxf_input::load_dxf(dxf_path, DXF_ARC_TOLERANCE_DEG)
        .expect("load rivers_aligned.dxf");
    let open_count = dxf_polys.iter().filter(|p| !p.closed).count();
    let (mut dx0, mut dy0, mut dx1, mut dy1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for poly in &dxf_polys {
        let [a, b, c, d] = poly.bbox();
        dx0 = dx0.min(a);
        dy0 = dy0.min(b);
        dx1 = dx1.max(c);
        dy1 = dy1.max(d);
    }
    eprintln!(
        "\nDXF rivers: {} polylines ({open_count} OPEN — an open river must not close into a \
         chord), bbox x [{dx0:.2}, {dx1:.2}] y [{dy0:.2}, {dy1:.2}] mm against the mesh's \
         x [{:.2}, {:.2}] y [{:.2}, {:.2}].",
        dxf_polys.len(),
        mesh.bbox.min.x,
        mesh.bbox.max.x,
        mesh.bbox.min.y,
        mesh.bbox.max.y,
    );
    let mut dxf_samples = Vec::new();
    let mut dxf_lines: Vec<Vec<P2>> = Vec::new();
    for poly in &dxf_polys {
        let mut line = Vec::new();
        let pts = &poly.exterior;
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            let steps = ((len / field.cell).ceil() as usize).max(1);
            for s in 0..steps {
                let t = s as f64 / steps as f64;
                let p = P2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y));
                dxf_samples.push(p);
                line.push(p);
            }
        }
        if let Some(&last) = pts.last() {
            dxf_samples.push(last);
            line.push(last);
        }
        if line.len() >= 2 {
            dxf_lines.push(line);
        }
    }
    let buffered_a = buffer_samples(&hydro, &dt_for_window, &dxf_samples);
    let mut dxf_cells: Vec<usize> = Vec::with_capacity(dxf_samples.len());
    let mut dxf_cell_owner: Vec<usize> = Vec::with_capacity(dxf_samples.len());
    for (k, p) in dxf_samples.iter().enumerate() {
        if let Some(i) = field.index_at(p.x, p.y) {
            dxf_cells.push(i);
            dxf_cell_owner.push(k);
        }
    }
    let src_a = nearest_source(&hydro, &dxf_cells);
    let reach_cells = (TANGENT_HALF_WINDOW_MM / field.cell).round().max(1.0) as usize;

    // ── territory clip, and which region owns each cell ──
    //
    // `region_of` carries the index of the FIRST costed region containing the
    // cell, in the descending-area order the regions are costed in. V0-att
    // needs it because `s_shipped` is a per-region dial. The `overlap_mm = 2.0`
    // seam dilation makes neighbouring region polygons overlap, so a cell can
    // be claimed by more than one; the multiply-claimed count is reported, and
    // it barely matters here because the derate lands on the same value in
    // almost every region.
    let costed_bboxes: Vec<[f64; 4]> = costed.iter().map(Polygon2::bbox).collect();
    let region_of: Vec<i32> = (0..field.len())
        .into_par_iter()
        .map(|i| {
            if field.nodata[i] {
                return -1;
            }
            let p = field.xy(i);
            costed
                .iter()
                .zip(costed_bboxes.iter())
                .position(|(poly, bb)| {
                    p.x >= bb[0]
                        && p.x <= bb[2]
                        && p.y >= bb[1]
                        && p.y <= bb[3]
                        && poly.contains_point(&p)
                })
                .map_or(-1, |k| k as i32)
        })
        .collect();
    let territory: Vec<bool> = region_of.iter().map(|&k| k >= 0).collect();
    let territory_cells = territory.iter().filter(|&&t| t).count();
    let multi_claimed = (0..field.len())
        .into_par_iter()
        .filter(|&i| {
            if region_of[i] < 0 {
                return false;
            }
            let p = field.xy(i);
            costed
                .iter()
                .zip(costed_bboxes.iter())
                .filter(|(poly, bb)| {
                    p.x >= bb[0]
                        && p.x <= bb[2]
                        && p.y >= bb[1]
                        && p.y <= bb[3]
                        && poly.contains_point(&p)
                })
                .count()
                > 1
        })
        .count();
    eprintln!(
        "\nterritory ownership: {territory_cells} cells, {multi_claimed} claimed by more than \
         one region\n\x20  (the overlap_mm = {OVERLAP_MM:.1} seam dilation; the first region in \
         descending-area order wins)."
    );

    // ── the census lattice over the whole territory (the scale reference) ──
    let mut territory_pts = Vec::new();
    let lattice_step = CENSUS_LATTICE_MM;
    let first = |lo: f64| (lo / lattice_step).ceil() as i64;
    let last = |hi: f64| (hi / lattice_step + 1e-9).floor() as i64;
    for iy in first(mesh.bbox.min.y)..=last(mesh.bbox.max.y) {
        for ix in first(mesh.bbox.min.x)..=last(mesh.bbox.max.x) {
            let p = P2::new(ix as f64 * lattice_step, iy as f64 * lattice_step);
            if field.index_at(p.x, p.y).is_some_and(|i| territory[i]) {
                territory_pts.push(p);
            }
        }
    }
    eprintln!(
        "\n---------- V0-pre: WHOLE-TERRITORY direction-prize ceiling (for scale) ----------\n\
         \x20  bound(dir) = [integral dA / W(kappa_perp(dir))] / [integral dA / W_max] >= 1.\n\
         \x20  Fit radius {CENSUS_FIT_RADIUS_MM} mm, lattice {CENSUS_LATTICE_MM} mm, \
         scallop {SCALLOP_H_MM} mm;\n\
         \x20  the verdict cell of wanaka_curvature_anisotropy.rs.\n"
    );
    print_census_header();
    let territory_samples = census_samples(&mesh, &index, &territory_pts);
    print_census_rows(&mesh, &index, "territory", &territory_samples);

    // ── region frames + the shipped honest raster (both MASK-INDEPENDENT) ──
    let mut rows: Vec<RegionRow> = Vec::new();
    for (k, polygon) in costed.iter().enumerate() {
        let (incl, incl_n) = region_max_slope_deg(
            &surface,
            &covered,
            polygon,
            planner.steep_threshold_deg,
            &|_: P2| true,
        );
        let derated = if incl > 1.0 {
            stepover * incl.to_radians().cos()
        } else {
            stepover
        };
        let frame = region_frame(polygon, derated);
        rows.push(RegionRow {
            index: k + 1,
            polygon: polygon.clone(),
            area_mm2: polygon.area(),
            direction_deg: frame.direction_deg,
            rotated: frame.rotated,
            elongation: frame.elongation,
            theta_max_incl_deg: incl,
            incl_cells: incl_n,
            derated_stepover_mm: derated,
            cost: None,
        });
    }

    eprintln!(
        "\n---------- the shipped honest raster, costed under the MACHINED-STOCK ceiling ----------\n\
         \x20  Per region: C2 frame (PCA-minor above the elongation gate, else 0 deg) at the\n\
         \x20  cos(theta_max)-derated stepover, monotone-cell decomposition on that lattice,\n\
         \x20  production relink (hookup 25.0, sampling 0.5, reorder true, flush_ride true,\n\
         \x20  airborne exemption true), F-034 costing on the project kinematics.\n\
         \x20  This arm does not depend on either mask, so it is costed ONCE.\n"
    );
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::machine_kinematics::MachineKinematics;
    use rs_cam_core::surface_link::LinkCeiling;
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let block_top_z = mesh.bbox.max.z;

    let mut bx0 = f64::INFINITY;
    let mut by0 = f64::INFINITY;
    let mut bx1 = f64::NEG_INFINITY;
    let mut by1 = f64::NEG_INFINITY;
    for row in &rows {
        let [a, b, c, d] = row.polygon.bbox();
        bx0 = bx0.min(a - CEILING_STOCK_MARGIN_MM);
        by0 = by0.min(b - CEILING_STOCK_MARGIN_MM);
        bx1 = bx1.max(c + CEILING_STOCK_MARGIN_MM);
        by1 = by1.max(d + CEILING_STOCK_MARGIN_MM);
    }
    let bounds = [
        bx0.max(mesh.bbox.min.x),
        by0.max(mesh.bbox.min.y),
        bx1.min(mesh.bbox.max.x),
        by1.min(mesh.bbox.max.y),
    ];
    let t_stock = std::time::Instant::now();
    let stock = machined_stock(&mesh, &index, &r15, &tier_map, bounds);
    eprintln!(
        "     ceiling stock built over [{:.1}, {:.1}] x [{:.1}, {:.1}] in {:.1}s\n",
        bounds[0],
        bounds[2],
        bounds[1],
        bounds[3],
        t_stock.elapsed().as_secs_f64()
    );
    let regime = LinkRegime {
        safe_z,
        ceiling: Some(LinkCeiling {
            stock: Some(&stock),
            tool_radius: r10.envelope_radius_mm(),
            fallback_top_z: block_top_z,
        }),
        flush_ride: true,
        airborne: true,
    };

    eprintln!(
        "     {:>4}  {:>8}  {:>8}  {:>9}  {:>7}  {:>8}  {:>9}  {:>9}  {:>7}",
        "rgn", "step mm", "dir deg", "fragments", "linked", "retracts", "time s", "cut mm", "secs"
    );
    for row in &mut rows {
        let t_region = std::time::Instant::now();
        let grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
            &mesh,
            &index,
            &r10,
            row.derated_stepover_mm,
            row.direction_deg,
            effective_min_z,
        );
        let cells = lattice_monotone_cells(&grid, &row.polygon, effective_min_z);
        let polygons = if cells.cells.is_empty() {
            std::slice::from_ref(&row.polygon).to_vec()
        } else {
            cells.cells.clone()
        };
        let boundary = RegionSet::new(vec![row.polygon.clone()]);
        let raw = raster_candidate(&grid, &polygons, safe_z, effective_min_z);
        let cost = relink_and_cost_under(raw, &mesh, &index, &r10, &boundary, &kinematics, &regime);
        eprintln!(
            "     {:>4}  {:>8.4}  {:>8.2}  {:>9}  {:>7}  {:>8}  {:>9.1}  {:>8.0}  {:>7.1}",
            row.index,
            row.derated_stepover_mm,
            row.direction_deg,
            cost.fragments,
            cost.linked,
            cost.kept_retracts,
            cost.time_s,
            cost.cutting_mm,
            t_region.elapsed().as_secs_f64(),
        );
        row.cost = Some(cost);
    }

    // ── mask evaluation ──
    let ctx = EvalCtx {
        mesh: &mesh,
        index: &index,
        field: &field,
        surface: &surface,
        covered: &covered,
        steep_clamp_deg: planner.steep_threshold_deg,
        stepover,
        cell_area,
        territory_cells,
        territory_pts: &territory_pts,
        region_of: &region_of,
        kinematics: &kinematics,
        rows: &rows,
    };

    // ── the whole-territory x-floor, for scale ──
    {
        // The SAME one-pass model the masks use, so the territory row and the
        // in-mask rows are the same measure. `d_valley` is deliberately NOT
        // evaluated here: outside a mask the valley direction is undefined and
        // a territory figure for it would be meaningless.
        let model = xy_pitch_model(&ctx, &territory, &|_, shipped_deg| shipped_deg);
        let territory_surface = model.surface_area_mm2;
        let territory_spacing_len = model.surface_len_mm;
        let l_min = territory_surface / stepover;
        let territory_m2_spacing = territory_spacing_len / l_min.max(1e-9);
        let territory_lower = model.flat_len_mm / l_min.max(1e-9);
        let territory_m2_xy = model.len_mm / l_min.max(1e-9);
        let total_mm: f64 = rows
            .iter()
            .filter_map(|r| r.cost.as_ref())
            .map(|c| c.cutting_mm)
            .sum();
        let total_s: f64 = rows
            .iter()
            .filter_map(|r| r.cost.as_ref())
            .map(|c| c.time_s)
            .sum();
        // The SAME two numerator conventions the per-mask M2 prints, so the
        // territory figure and the in-mask figures are comparable measures.
        let mut cut_intent_mm = 0.0f64;
        for row in &rows {
            if let Some(cost) = row.cost.as_ref() {
                let (split, _, _) =
                    attribute_cutting(&cost.toolpath, &kinematics, cost.time_s, &|_| false);
                cut_intent_mm += split.out_mm;
            }
        }
        eprintln!(
            "\n---------- whole costed territory, for scale ----------\n\
             \x20  finish territory {territory_cells} cells = {:.1} mm² XY;\n\
             \x20  surface area {:.1} mm²; L_min = {:.1} mm at s_max {stepover:.6} mm\n\
             \x20  xfloor, DISTANCE / cut-intent only:   {:.0} mm => {:.4}x\n\
             \x20  xfloor, DISTANCE / all non-rapid:     {:.0} mm => {:.4}x   \
             (total_cutting_distance)\n\
             \x20  V0-att v1 (RETIRED, kept as the bracket's UPPER bound)\n\
             \x20     M2_spacing, surface-pitch: {:.4}x   \
             (integral dA/s_shipped = {:.1} mm, L_min {:.1} mm)\n\
             \x20     v1 residual = {:.4}x = {:.2} pp of the {:.2} pp excess\n\
             \x20  ---- V0-att v2, direction-aware XY-pitch model ----\n\
             \x20  BRACKET  lower {:.4}x  <=  measured M2 {:.4}x  <=  upper {:.4}x   {}\n\
             \x20  M2_xy(d_shipped) = {:.4}x   (SUM (a_xy/s_shipped)*sec phi = {:.1} mm; \
             {} fallback, {} degenerate, {} unowned cells)\n\
             \x20  R_topo = M2 - M2_xy(d_shipped) = {:+.4}x = {:+.2} pp   [context only]\n\
             \x20  (no d_valley on the territory — undefined outside a mask)\n\
             \x20  costed time (cut + links + rapids) {:.1} s over {} regions",
            territory_cells as f64 * cell_area,
            territory_surface,
            l_min,
            cut_intent_mm,
            cut_intent_mm / l_min.max(1e-9),
            total_mm,
            total_mm / l_min.max(1e-9),
            territory_m2_spacing,
            territory_spacing_len,
            l_min,
            cut_intent_mm / l_min.max(1e-9) - territory_m2_spacing,
            100.0 * (cut_intent_mm / l_min.max(1e-9) - territory_m2_spacing),
            100.0 * (cut_intent_mm / l_min.max(1e-9) - 1.0),
            territory_lower,
            cut_intent_mm / l_min.max(1e-9),
            territory_m2_spacing,
            if territory_lower <= cut_intent_mm / l_min.max(1e-9)
                && cut_intent_mm / l_min.max(1e-9) <= territory_m2_spacing
            {
                "BRACKET OK"
            } else {
                "BRACKET VIOLATED"
            },
            territory_m2_xy,
            model.len_mm,
            model.fallback_cells,
            model.degenerate_cells,
            model.unowned_cells,
            cut_intent_mm / l_min.max(1e-9) - territory_m2_xy,
            100.0 * (cut_intent_mm / l_min.max(1e-9) - territory_m2_xy),
            total_s,
            rows.len()
        );
    }

    let mask_a: Vec<bool> = (0..field.len())
        .map(|i| territory[i] && buffered_a.inside[i])
        .collect();
    let axis_a = |i: usize| -> f64 {
        let owner = src_a[i];
        if owner == u32::MAX {
            f64::NAN
        } else {
            dxf_axis_deg(&dxf_samples, dxf_cell_owner[owner as usize], reach_cells)
        }
    };
    evaluate_mask(
        &ctx,
        &MaskProvenance {
            label: "mask A — rivers_aligned.dxf, MAP HYDROGRAPHY (river_through_cut = false, \
                    so it is NOT incised into the mesh; reported for the DXF-probe caveat only)"
                .to_owned(),
            samples: dxf_samples.len(),
            floored: buffered_a.floored_samples,
            skipped: buffered_a.skipped_samples,
            half_widths_mm: &buffered_a.half_widths_mm,
        },
        &mask_a,
        &axis_a,
    );

    // Mask B at the pre-declared rung and at the sensitivity rungs beside it.
    let mut sweep: Vec<f64> = ACC_SENSITIVITY_RUNGS_MM2.to_vec();
    if !sweep.iter().any(|t| (t - threshold_mm2).abs() < 1e-9) {
        sweep.push(threshold_mm2);
    }
    sweep.sort_by(f64::total_cmp);
    let mut chosen_mask_b: Vec<bool> = Vec::new();
    let mut chosen_valley_lines: Vec<Vec<P2>> = Vec::new();
    for &t in &sweep {
        let network: Vec<bool> = (0..hydro.len())
            .map(|i| !hydro.nodata[i] && acc[i] * cell_area >= t)
            .collect();
        let net_cells: Vec<usize> = (0..hydro.len()).filter(|&i| network[i]).collect();
        if net_cells.is_empty() {
            eprintln!("\n     mask B at T = {t:.0} mm²: EMPTY NETWORK — nothing measured");
            continue;
        }
        let net_samples: Vec<P2> = net_cells.iter().map(|&i| field.xy(i)).collect();
        let buffered = buffer_samples(&hydro, &dt_for_window, &net_samples);
        let src = nearest_source(&hydro, &net_cells);
        let (tangent_deg, main_donor) =
            valley_tangents(&hydro, &receivers, &acc, &network, reach_cells);
        let mask: Vec<bool> = (0..field.len())
            .map(|i| territory[i] && buffered.inside[i])
            .collect();
        let axis_b = |i: usize| -> f64 {
            let owner = src[i];
            if owner == u32::MAX {
                f64::NAN
            } else {
                tangent_deg[net_cells[owner as usize]]
            }
        };
        let chosen = (t - threshold_mm2).abs() < 1e-9;
        let tag = if chosen {
            " <<< the rung the pre-declared stability rule picked"
        } else {
            " (sensitivity)"
        };
        evaluate_mask(
            &ctx,
            &MaskProvenance {
                label: format!("mask B — FLOW ACCUMULATION at T = {t:.0} mm²{tag}"),
                samples: net_samples.len(),
                floored: buffered.floored_samples,
                skipped: buffered.skipped_samples,
                half_widths_mm: &buffered.half_widths_mm,
            },
            &mask,
            &axis_b,
        );
        if chosen {
            let overlap = (0..field.len()).filter(|&i| mask[i] && mask_a[i]).count();
            let a_cells = mask_a.iter().filter(|&&m| m).count();
            let b_cells = mask.iter().filter(|&&m| m).count();
            eprintln!(
                "\x20  mask A / mask B OVERLAP at this rung: {overlap} cells = {:.2} % of mask A, \
                 {:.2} % of mask B",
                100.0 * overlap as f64 / a_cells.max(1) as f64,
                100.0 * overlap as f64 / b_cells.max(1) as f64,
            );
            chosen_valley_lines = valley_polylines(&field, &receivers, &network, &main_donor);
            chosen_mask_b = mask;
        }
    }

    // ── the VISUAL ──
    let dir = svg_output_dir();
    let empty: Vec<bool> = Vec::new();
    let no_lines: Vec<Vec<P2>> = Vec::new();
    for (name, ma, mb, la, lb, title) in [
        (
            "wanaka_valley_masks_h0.svg",
            &mask_a,
            &chosen_mask_b,
            &dxf_lines,
            &chosen_valley_lines,
            "Track H V0 — mask A + mask B on wanaka200",
        ),
        (
            "wanaka_valley_mask_a_h0.svg",
            &mask_a,
            &empty,
            &dxf_lines,
            &no_lines,
            "Track H V0 — mask A (rivers_aligned.dxf) on wanaka200",
        ),
        (
            "wanaka_valley_mask_b_h0.svg",
            &empty,
            &chosen_mask_b,
            &no_lines,
            &chosen_valley_lines,
            "Track H V0 — mask B (flow accumulation) on wanaka200",
        ),
    ] {
        let path = dir.join(name);
        let inputs = SvgInputs {
            field: &field,
            territory: &costed,
            mask_a: ma,
            mask_b: mb,
            valley_lines: lb,
            dxf_lines: la,
        };
        match write_census_svg(&inputs, &path, title) {
            Ok(()) => eprintln!("SVG: {}", path.display()),
            Err(e) => eprintln!("SVG write FAILED for {}: {e}", path.display()),
        }
    }
    eprintln!(
        "\x20  ({} extracted valley polylines at the chosen rung, {} DXF polylines)",
        chosen_valley_lines.len(),
        dxf_lines.len()
    );

    eprintln!(
        "\n========== END — total {:.1}s. No verdict is written here. ==========\n",
        started.elapsed().as_secs_f64()
    );
}

/// The local axis (degrees) of the DXF polyline sample list at `k`, over a
/// window of [`TANGENT_HALF_WINDOW_MM`]. The sample list is a concatenation
/// of polylines resampled at one-cell spacing, so a fixed index window IS a
/// fixed arc length; a window that crosses a polyline boundary is accepted
/// (it costs one sample of smoothing at each of the 249 joins).
fn dxf_axis_deg(samples: &[P2], k: usize, reach: usize) -> f64 {
    if samples.len() < 2 {
        return f64::NAN;
    }
    let a = samples[k.saturating_sub(reach)];
    let b = samples[(k + reach).min(samples.len() - 1)];
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    if dx.hypot(dy) < 1e-9 {
        return f64::NAN;
    }
    dy.atan2(dx).to_degrees().rem_euclid(180.0)
}
