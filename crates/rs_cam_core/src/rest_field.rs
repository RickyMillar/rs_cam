//! Rest-depth-field pencil detector (detector #4) — the tool-offset-space one.
//!
//! Pencil is a TOOL-INTERACTION feature, not a design-surface feature: it is
//! the locus where the finish tool cannot reach but a smaller tool can. The
//! previous three detectors (dihedral, drainage, curvature crest) all read
//! features of the DESIGN surface and were tool-radius-blind. This module
//! instead computes the dual-tool rest field every commercial CAM uses:
//!
//! ```text
//! rest(x, y) = drop_z(reference_tool, x, y) − drop_z(pencil_tool, x, y)
//! ```
//!
//! evaluated on a regular XY grid with [`crate::dropcutter::point_drop_cutter`]
//! (the same machinery the old drainage DEM used). `rest > 0` exactly where the
//! pencil tool reaches deeper than the bigger reference tool could — i.e. the
//! rest material the finish pass left. Flats, everything the reference already
//! reached, and texture finer than the pencil radius all read ≈ 0 *by
//! construction* (the tool radius is the morphological low-pass filter). No
//! smoothing, no denoise gates.
//!
//! Pipeline: grid → rest field → threshold mask → 8-connected components →
//! chamfer distance transform (local half-width) → Zhang-Suen thinning →
//! skeleton tracing → route each skeleton polyline by width (narrow → pencil
//! centreline, wide → clearing region). See
//! `planning/pencil_restdepth_detector_prompt.md`.
//!
//! Not to be confused with [`crate::rest`], the 2D polygon rest op (offset a
//! polygon inward by the previous tool's radius, scan-line the leftover
//! ring). This module works on mesh dexel/heightmap depth comparisons; that
//! one works on 2D polygon offsets. Zero code overlap between the two.

use tracing::info;

use crate::dexel_stock::TriDexelStock;
use crate::dropcutter::point_drop_cutter;
use crate::geo::{P3, polyline_length};
use crate::grid2::Grid2;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;

/// What the pencil rest is measured "deeper than". `Copy` so the sample closure
/// (possibly parallel) can capture it by value.
#[derive(Clone, Copy)]
pub enum RestReference<'a> {
    /// A milling cutter: a genuinely bigger finish tool, the nominal ball, or —
    /// when `is_surface_probe` — a tiny bare-surface probe whose sign is flipped
    /// (`rest = pencil_z − probe_z`, how far the pencil floats above bare stock).
    Cutter {
        tool: &'a dyn MillingCutter,
        is_surface_probe: bool,
    },
    /// The ACTUAL machined stock left by prior toolpaths (R2): the rest is the
    /// remaining material height above the pencil drop,
    /// `rest = stock_top_z(x, y) − pencil_z`. Strictly better than any nominal
    /// tool drop — it bakes in the prior TOOLPATH pattern (scallop cusps,
    /// skipped boundaries, walls the finish never visited), not just the prior
    /// tool's shape.
    Stock(&'a TriDexelStock),
}

impl RestReference<'_> {
    /// Radius by which the boundary trust region is eroded because the reference
    /// overhangs the part edge. A cutter's ball hangs off and reads false-high
    /// rest; stock has no overhanging ball, so it contributes nothing (the
    /// pencil radius still erodes via the caller's `max`).
    fn erosion_radius(&self) -> f64 {
        match self {
            RestReference::Cutter { tool, .. } => tool.radius(),
            RestReference::Stock(_) => 0.0,
        }
    }
}

/// Minimum cell count for a rest region to be kept (drop isolated noise cells).
const MIN_REGION_CELLS: usize = 4;

/// 8-neighbour offsets `(dr, dc)`, clockwise from North. This order IS the
/// Zhang-Suen p2..p9 sequence, so the same array drives thinning, flood fill,
/// distance-transform neighbourhoods, and skeleton degree/tracing. Iterating it
/// in a fixed order keeps every downstream path deterministic.
const NB8: [(isize, isize); 8] = [
    (-1, 0),  // p2  N
    (-1, 1),  // p3  NE
    (0, 1),   // p4  E
    (1, 1),   // p5  SE
    (1, 0),   // p6  S
    (1, -1),  // p7  SW
    (0, -1),  // p8  W
    (-1, -1), // p9  NW
];

/// Inputs to the rest-field detector.
pub struct RestFieldParams {
    /// XY grid cell size (mm). Smaller = finer regions, more drops.
    pub cell_mm: f64,
    /// Rest-depth threshold (mm): a cell is in the mask when `rest` exceeds this.
    /// This IS the pencil op's `min_valley_depth` — the field threshold and the
    /// user dial are the same quantity by construction.
    pub min_valley_depth: f64,
    /// A skeleton polyline routes to pencil when its region half-width
    /// `≤ route_width_factor × pencil_radius`; wider routes to clearing.
    pub route_width_factor: f64,
    /// Pencil-tool radius (mm) — the routing yardstick.
    pub pencil_radius: f64,
    /// Minimum kept-cut length (mm) — used only for the coverage report; the
    /// caller applies the real `min_cut_length` filter downstream.
    pub min_cut_length: f64,
}

/// A wide rest region routed to clearing rather than pencil centrelines. v1
/// carries just the region bbox + significance; Phase D turns these into
/// adaptive3d `FromRemainingStock` boundaries.
#[derive(Debug, Clone)]
pub struct ClearingRegion {
    /// XY bounding box `[min_x, min_y, max_x, max_y]` (mm).
    pub bbox: [f64; 4],
    /// Number of mask cells in the region.
    pub cell_count: usize,
    /// Peak rest depth anywhere in the region (mm) — significance ranking.
    pub peak_rest_mm: f64,
}

/// Quantitative "is this pencil path good?" summary, logged and attached to the
/// debug trace. The rest field is the ground truth the previous detectors never
/// had.
#[derive(Debug, Clone, Default)]
pub struct RestFieldReport {
    /// Σ rest × cell² over the mask (mm³) — material the finish pass left.
    pub total_rest_volume_mm3: f64,
    /// Distinct components that produced ≥1 pencil-routed skeleton polyline.
    pub pencil_region_count: usize,
    /// Distinct components routed (wholly or partly) to clearing by width.
    pub clearing_region_count: usize,
    /// Total length of every traced skeleton polyline (mm).
    pub skeleton_length_mm: f64,
    /// Length of pencil-routed polylines that clear `min_cut_length` (mm).
    pub traced_length_mm: f64,
    /// Peak rest depth per kept component (mm), descending — significance rank.
    pub region_peak_rest_mm: Vec<f64>,
    /// Grid dimensions actually evaluated.
    pub grid_nx: usize,
    pub grid_ny: usize,
}

impl RestFieldReport {
    /// Traced skeleton fraction that survives `min_cut_length` — the coverage
    /// metric from the plan. 1.0 when nothing was lost; 0.0 with no skeleton.
    pub fn coverage(&self) -> f64 {
        if self.skeleton_length_mm <= 1e-9 {
            0.0
        } else {
            self.traced_length_mm / self.skeleton_length_mm
        }
    }
}

/// The continuous rest-depth field on the XY sampling grid — the ground truth
/// for the GUI heatmap overlay. Row-major `r*nx + c`; cell centre at
/// `(origin_x + c*cell_mm, origin_y + r*cell_mm)`. Cells outside the trusted
/// region (non-contact, or within the eroded boundary band) are `NaN` in both
/// `rest` and `surface_z`.
#[derive(Debug, Clone)]
pub struct RestGrid {
    pub nx: usize,
    pub ny: usize,
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell_mm: f64,
    /// Per-cell rest depth (mm). `NaN` = untrusted / outside the part.
    pub rest: Vec<f32>,
    /// Per-cell surface Z (pencil drop, mm) for draping the overlay on the
    /// surface. `NaN` = untrusted / outside.
    pub surface_z: Vec<f32>,
    /// The `min_valley_depth` threshold (mm) below which a cell is not routed
    /// to a cut — for the legend / neutral shading under the threshold.
    pub threshold: f64,
}

/// Output of [`detect_rest_valleys`].
pub struct RestFieldResult {
    /// Pencil-routed centrelines (world XY at cell centres, Z from the pencil
    /// drop). Feed straight into the existing pencil pipeline (`min_cut_length`
    /// filter → `resample_polyline` → `paths_from_sampled`).
    pub centerlines: Vec<Vec<P3>>,
    /// Wide regions routed to clearing (not emitted as pencil in v1).
    pub clearing_regions: Vec<ClearingRegion>,
    /// Detection significance / quality metrics.
    pub report: RestFieldReport,
    /// The continuous rest-depth grid, for visualisation (heatmap overlay).
    pub rest_grid: RestGrid,
}

/// True set-cells with 8-neighbourhood bounds handling (out of bounds = unset).
#[inline]
fn mask_at(mask: &Grid2<bool>, r: isize, c: isize) -> bool {
    mask.get_signed_or(r, c, false)
}

/// Build the rest field and extract routed valley centrelines + clearing regions.
///
/// `reference` is a [`RestReference`]: a bigger finish tool, the nominal ball, a
/// degenerate bare-surface probe, or (R2) the actual machined stock the prior
/// toolpaths left.
pub fn detect_rest_valleys(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    pencil: &dyn MillingCutter,
    reference: RestReference<'_>,
    params: &RestFieldParams,
) -> RestFieldResult {
    let cell = params.cell_mm.max(1e-3);
    let bbox = &mesh.bbox;
    // Grid margin must exceed the pencil radius so the outer ring of cells is
    // genuinely non-contact — the boundary distance-transform (below) erodes the
    // false-high rest band inward from there. `contact` is limited by the SMALLER
    // (pencil) tool through the AND, so it extends only ~pencil_radius past the
    // true surface; a margin > pencil_radius guarantees a non-contact ring.
    let margin_cells = (pencil.radius() / cell).ceil() as usize + 1;
    let margin = margin_cells as f64 * cell;
    let origin_x = bbox.min.x - margin;
    let origin_y = bbox.min.y - margin;
    let nx = ((bbox.max.x - bbox.min.x) / cell).ceil().max(0.0) as usize + 2 * margin_cells + 1;
    let ny = ((bbox.max.y - bbox.min.y) / cell).ceil().max(0.0) as usize + 2 * margin_cells + 1;
    let n = nx * ny;

    // --- 1. Grid build: drop the pencil (and the reference, or query stock) at
    // every cell centre. ---
    let sample = |i: usize| -> (f64, bool, f64) {
        let (r, c) = crate::grid2::row_major_rc(i, nx);
        let x = origin_x + c as f64 * cell;
        let y = origin_y + r as f64 * cell;
        let pc = point_drop_cutter(x, y, mesh, index, pencil);
        if !pc.contacted {
            return (0.0, false, f64::NAN);
        }
        let rest = match reference {
            RestReference::Cutter {
                tool,
                is_surface_probe,
            } => {
                let rc = point_drop_cutter(x, y, mesh, index, tool);
                if !rc.contacted {
                    // Reference non-contact ⇒ invalid (avoids boundary artefacts).
                    return (0.0, false, f64::NAN);
                }
                // Tool mode: bigger reference floats higher, rest = ref_z −
                // pencil_z > 0 where the pencil reaches deeper. Surface-probe
                // mode: reference ≈ bare surface (low), rest = pencil_z − probe_z
                // > 0 where the pencil floats.
                if is_surface_probe {
                    (pc.z - rc.z).max(0.0)
                } else {
                    (rc.z - pc.z).max(0.0)
                }
            }
            RestReference::Stock(stock) => {
                // Remaining material height above the pencil drop. No cell (out
                // of the stock grid) or no material in the column ⇒ invalid,
                // same as reference non-contact. Nearest-cell lookup only — do
                // NOT interpolate dexel tops across steep walls (smears cliffs).
                let Some((row, col)) = stock.z_grid.world_to_cell(x, y) else {
                    return (0.0, false, f64::NAN);
                };
                let Some(top_z) = stock.z_grid.top_z_at(row, col) else {
                    return (0.0, false, f64::NAN);
                };
                (top_z as f64 - pc.z).max(0.0)
            }
        };
        (rest, true, pc.z)
    };

    let samples: Vec<(f64, bool, f64)> = {
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            (0..n).into_par_iter().map(sample).collect()
        }
        #[cfg(not(feature = "parallel"))]
        {
            (0..n).map(sample).collect()
        }
    };

    let mut rest = Grid2::new_fill(nx, ny, 0.0f64);
    let mut pencil_z = Grid2::new_fill(nx, ny, f64::NAN);
    let mut contact = Grid2::new_fill(nx, ny, false);
    let threshold = params.min_valley_depth.max(0.0);
    for (i, &(rv, valid, pz)) in samples.iter().enumerate() {
        rest.set_index(i, rv);
        pencil_z.set_index(i, pz);
        contact.set_index(i, valid);
    }

    // Erode the trust region by the overhanging tool radius. Within roughly one
    // ball radius of the part's XY boundary the bigger tool hangs off the edge
    // and rests on it, reading a false-high `rest` that is not real material —
    // it would otherwise ring the part in a spurious rest "moat". The reading is
    // only trustworthy where the overhanging tool is fully supported.
    let erode_cells = (pencil.radius().max(reference.erosion_radius()) / cell).ceil();
    let boundary_dt = chamfer_distance(&contact);
    let mask_data: Vec<bool> = contact
        .as_slice()
        .iter()
        .zip(boundary_dt.as_slice())
        .zip(rest.as_slice())
        .map(|((&ct, &dt), &rv)| ct && dt >= erode_cells && rv > threshold)
        .collect();
    // SAFETY: mask_data is built by zipping three grids all constructed with
    // the same nx*ny above, so the length always matches.
    #[allow(clippy::unwrap_used)]
    let mut mask = Grid2::from_vec(nx, ny, mask_data).unwrap();

    // Snapshot the continuous rest field for the GUI heatmap overlay. Trusted
    // cells (contact + inside the eroded boundary band) keep their rest depth
    // and surface Z; untrusted cells become NaN so the overlay skips them
    // (avoids the spurious boundary "moat" the mask erosion already removes).
    let (grid_rest, grid_surface_z): (Vec<f32>, Vec<f32>) = rest
        .as_slice()
        .iter()
        .zip(pencil_z.as_slice())
        .zip(contact.as_slice())
        .zip(boundary_dt.as_slice())
        .map(|(((&rv, &pz), &ct), &dt)| {
            if ct && dt >= erode_cells {
                (rv as f32, pz as f32)
            } else {
                (f32::NAN, f32::NAN)
            }
        })
        .unzip();
    let rest_grid = RestGrid {
        nx,
        ny,
        origin_x,
        origin_y,
        cell_mm: cell,
        rest: grid_rest,
        surface_z: grid_surface_z,
        threshold,
    };

    // --- 2. Components: 8-connected flood fill; drop tiny ones. ---
    let mut comp_id: Grid2<usize> = Grid2::new_fill(nx, ny, usize::MAX);
    let mut comp_cells: Vec<usize> = Vec::new(); // cell count per component
    let mut comp_peak: Vec<f64> = Vec::new();
    let mut comp_bbox: Vec<[f64; 4]> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for seed in 0..n {
        if !mask.at_index_or(seed, false) || comp_id.at_index_or(seed, usize::MAX) != usize::MAX {
            continue;
        }
        let cid = comp_cells.len();
        let mut count = 0usize;
        let mut peak = 0.0f64;
        let mut bb = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        stack.clear();
        stack.push(seed);
        comp_id.set_index(seed, cid);
        while let Some(cur) = stack.pop() {
            count += 1;
            let rv = rest.at_index_or(cur, 0.0);
            if rv > peak {
                peak = rv;
            }
            let (rr, cc) = crate::grid2::row_major_rc(cur, nx);
            let (r, c) = (rr as isize, cc as isize);
            let x = origin_x + cc as f64 * cell;
            let y = origin_y + rr as f64 * cell;
            bb[0] = bb[0].min(x);
            bb[1] = bb[1].min(y);
            bb[2] = bb[2].max(x);
            bb[3] = bb[3].max(y);
            for (dr, dc) in NB8 {
                let (nr, nc) = (r + dr, c + dc);
                let Some(nidx) = mask.index_of_signed(nr, nc) else {
                    continue;
                };
                if !mask.at_index_or(nidx, false) {
                    continue;
                }
                if comp_id.at_index_or(nidx, usize::MAX) == usize::MAX {
                    comp_id.set_index(nidx, cid);
                    stack.push(nidx);
                }
            }
        }
        comp_cells.push(count);
        comp_peak.push(peak);
        comp_bbox.push(bb);
    }

    // Rebuild the mask keeping only components ≥ MIN_REGION_CELLS. Cells in
    // dropped components have their comp_id cleared so routing can't reference
    // them.
    for i in 0..n {
        if mask.at_index_or(i, false) {
            let cid = comp_id.at_index_or(i, usize::MAX);
            if cid == usize::MAX || at(&comp_cells, cid) < MIN_REGION_CELLS {
                mask.set_index(i, false);
                comp_id.set_index(i, usize::MAX);
            }
        }
    }

    // Total rest volume over the (cleaned) mask.
    let cell_area = cell * cell;
    let mut total_rest_volume = 0.0f64;
    for i in 0..n {
        if mask.at_index_or(i, false) {
            total_rest_volume += rest.at_index_or(i, 0.0) * cell_area;
        }
    }

    // --- 3. Chamfer distance transform (local half-width, in cells). ---
    let dt = chamfer_distance(&mask);

    // --- 4. Zhang-Suen thinning → 1-cell skeleton. ---
    let skel = zhang_suen_thin(&mask);

    // --- 5. Skeleton tracing → polylines (cell indices). ---
    let poly_cells = trace_skeleton(&skel);

    // --- 6. Route each polyline by width. ---
    let mut centerlines: Vec<Vec<P3>> = Vec::new();
    let mut pencil_comps: std::collections::BTreeSet<usize> = Default::default();
    let mut clearing_comps: std::collections::BTreeSet<usize> = Default::default();
    let mut skeleton_length = 0.0f64;
    let mut traced_length = 0.0f64;
    let width_limit = params.route_width_factor * params.pencil_radius;

    for poly in &poly_cells {
        if poly.len() < 2 {
            continue;
        }
        // Median DT along the polyline → region half-width in mm.
        let mut dts: Vec<f64> = poly.iter().map(|&i| dt.at_index_or(i, 0.0)).collect();
        dts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let half_width_cells = median_sorted(&dts);
        let half_width_mm = half_width_cells * cell;

        // World-space polyline (Z from the pencil drop; lift re-solves gouge-safely).
        let pts: Vec<P3> = poly
            .iter()
            .map(|&i| {
                let (r, c) = crate::grid2::row_major_rc(i, nx);
                let z = pencil_z.at_index_or(i, f64::NAN);
                P3::new(
                    origin_x + c as f64 * cell,
                    origin_y + r as f64 * cell,
                    if z.is_finite() { z } else { bbox.min.z },
                )
            })
            .collect();
        let len = polyline_length(&pts);
        skeleton_length += len;

        let comp = comp_id.at_index_or(at(poly, 0), usize::MAX);
        if half_width_mm <= width_limit {
            if comp != usize::MAX {
                pencil_comps.insert(comp);
            }
            if len >= params.min_cut_length {
                traced_length += len;
            }
            centerlines.push(pts);
        } else if comp != usize::MAX {
            clearing_comps.insert(comp);
        }
    }

    // --- 7. Clearing regions from the flagged components. ---
    let mut clearing_regions: Vec<ClearingRegion> = clearing_comps
        .iter()
        .map(|&cid| ClearingRegion {
            bbox: at(&comp_bbox, cid),
            cell_count: at(&comp_cells, cid),
            peak_rest_mm: at(&comp_peak, cid),
        })
        .collect();
    clearing_regions.sort_by(|a, b| {
        b.peak_rest_mm
            .partial_cmp(&a.peak_rest_mm)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut region_peaks: Vec<f64> = pencil_comps
        .iter()
        .chain(clearing_comps.iter())
        .map(|&cid| at(&comp_peak, cid))
        .collect();
    region_peaks.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let report = RestFieldReport {
        total_rest_volume_mm3: total_rest_volume,
        pencil_region_count: pencil_comps.len(),
        clearing_region_count: clearing_comps.len(),
        skeleton_length_mm: skeleton_length,
        traced_length_mm: traced_length,
        region_peak_rest_mm: region_peaks,
        grid_nx: nx,
        grid_ny: ny,
    };

    info!(
        grid = format!("{nx}x{ny}"),
        rest_volume_mm3 = format!("{:.1}", report.total_rest_volume_mm3),
        pencil_regions = report.pencil_region_count,
        clearing_regions = report.clearing_region_count,
        centerlines = centerlines.len(),
        coverage = format!("{:.2}", report.coverage()),
        "Rest-field detector complete"
    );

    RestFieldResult {
        centerlines,
        clearing_regions,
        report,
        rest_grid,
    }
}

// ---------------------------------------------------------------------------
// Everything grid-shaped (rest, pencil_z, contact, mask, comp_id, the two
// distance-transform passes, the skeleton) is a `Grid2<T>` — see `grid2.rs`.
// The one thing left that ISN'T grid-shaped is the component table
// (`comp_cells`/`comp_peak`/`comp_bbox`, indexed by component id, plus
// `poly[0]`): a single small generic accessor covers those.
// ---------------------------------------------------------------------------

/// Index a slice at a position already proven in-bounds by the caller's
/// invariant: a component id from `comp_id` (always `< comp_cells.len()` and
/// its parallel `comp_peak`/`comp_bbox` vectors, by construction — a `cid` is
/// only ever `comp_cells.len()` at the moment all three get pushed together),
/// or `poly[0]` after a `poly.len() >= 2` guard.
#[inline]
fn at<T: Copy>(v: &[T], i: usize) -> T {
    // SAFETY: see doc comment above.
    #[allow(clippy::indexing_slicing)]
    v[i]
}

fn median_sorted(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    // SAFETY: len > 0 → len/2 < len.
    #[allow(clippy::indexing_slicing)]
    sorted[sorted.len() / 2]
}

// ---------------------------------------------------------------------------
// Raster machinery (chamfer DT, Zhang-Suen thinning, skeleton tracing).
// Re-implemented from the spec in planning/pencil_restdepth_detector_prompt.md.
// ---------------------------------------------------------------------------

/// Two-pass chamfer distance transform: for each set cell, distance in cells to
/// the nearest unset cell (its local half-width on a region mask). Set cells
/// init to a large value, unset to 0; forward pass relaxes from W/N/NW/NE, the
/// backward pass from E/S/SE/SW, with orthogonal cost 1 and diagonal √2.
fn chamfer_distance(mask: &Grid2<bool>) -> Grid2<f64> {
    let nx = mask.nx();
    let ny = mask.ny();
    let big = (nx + ny) as f64 + 10.0;
    let sqrt2 = std::f64::consts::SQRT_2;
    let mut d = Grid2::new_fill(nx, ny, 0.0f64);
    for i in 0..mask.len() {
        d.set_index(i, if mask.at_index_or(i, false) { big } else { 0.0 });
    }
    // Forward: r ascending, c ascending — read already-updated upper/left cells.
    for r in 0..ny as isize {
        for c in 0..nx as isize {
            let Some(i) = d.index_of_signed(r, c) else {
                continue;
            };
            if !mask.at_index_or(i, false) {
                continue;
            }
            let mut best = d.at_index_or(i, big);
            best = best.min(d.get_signed_or(r, c - 1, big) + 1.0); // W
            best = best.min(d.get_signed_or(r - 1, c, big) + 1.0); // N
            best = best.min(d.get_signed_or(r - 1, c - 1, big) + sqrt2); // NW
            best = best.min(d.get_signed_or(r - 1, c + 1, big) + sqrt2); // NE
            d.set_index(i, best);
        }
    }
    // Backward: r descending, c descending — read already-updated lower/right cells.
    for r in (0..ny as isize).rev() {
        for c in (0..nx as isize).rev() {
            let Some(i) = d.index_of_signed(r, c) else {
                continue;
            };
            if !mask.at_index_or(i, false) {
                continue;
            }
            let mut best = d.at_index_or(i, big);
            best = best.min(d.get_signed_or(r, c + 1, big) + 1.0); // E
            best = best.min(d.get_signed_or(r + 1, c, big) + 1.0); // S
            best = best.min(d.get_signed_or(r + 1, c + 1, big) + sqrt2); // SE
            best = best.min(d.get_signed_or(r + 1, c - 1, big) + sqrt2); // SW
            d.set_index(i, best);
        }
    }
    d
}

/// Number of set neighbours (B) and 0→1 transitions (A) around the p2..p9 ring.
fn ring_ab(mask: &Grid2<bool>, r: isize, c: isize) -> (u32, u32) {
    let vals: [bool; 8] = NB8.map(|(dr, dc)| mask_at(mask, r + dr, c + dc));
    let b: u32 = vals.iter().filter(|&&v| v).count() as u32;
    // 0→1 transitions walking the ring; zip each value with its successor
    // (wrapping) instead of re-deriving `(k+1) % 8` indices.
    let a: u32 = vals
        .iter()
        .zip(vals.iter().cycle().skip(1))
        .filter(|&(&cur, &next)| !cur && next)
        .count() as u32;
    (b, a)
}

#[inline]
fn nb(mask: &Grid2<bool>, r: isize, c: isize, k: usize) -> bool {
    let (dr, dc) = NB8.get(k).copied().unwrap_or((0, 0));
    mask_at(mask, r + dr, c + dc)
}

/// Zhang-Suen morphological thinning to a 1-cell-wide skeleton. Two sub-passes
/// per iteration, deletions collected then applied synchronously, repeat until a
/// full iteration removes nothing.
fn zhang_suen_thin(mask: &Grid2<bool>) -> Grid2<bool> {
    let nx = mask.nx();
    let ny = mask.ny();
    let mut m = mask.clone();
    loop {
        let mut changed = false;
        for sub in 0..2 {
            let mut to_delete: Vec<usize> = Vec::new();
            for r in 0..ny as isize {
                for c in 0..nx as isize {
                    let Some(i) = m.index_of_signed(r, c) else {
                        continue;
                    };
                    if !m.at_index_or(i, false) {
                        continue;
                    }
                    let (b, a) = ring_ab(&m, r, c);
                    if !(2..=6).contains(&b) || a != 1 {
                        continue;
                    }
                    let p2 = nb(&m, r, c, 0);
                    let p4 = nb(&m, r, c, 2);
                    let p6 = nb(&m, r, c, 4);
                    let p8 = nb(&m, r, c, 6);
                    // Verbatim from the published Zhang-Suen conditions; the
                    // factored form clippy prefers is harder to check against the
                    // paper, so keep the literal triple-AND tests.
                    #[allow(clippy::nonminimal_bool)]
                    let ok = if sub == 0 {
                        !(p2 && p4 && p6) && !(p4 && p6 && p8)
                    } else {
                        !(p2 && p4 && p8) && !(p2 && p6 && p8)
                    };
                    if ok {
                        to_delete.push(i);
                    }
                }
            }
            if !to_delete.is_empty() {
                changed = true;
                for i in to_delete {
                    m.set_index(i, false);
                }
            }
        }
        if !changed {
            break;
        }
    }
    m
}

/// 8-connected degree of a skeleton cell.
fn skel_degree(skel: &Grid2<bool>, r: isize, c: isize) -> u32 {
    NB8.iter()
        .filter(|&&(dr, dc)| mask_at(skel, r + dr, c + dc))
        .count() as u32
}

/// Trace a 1-cell skeleton into polylines (cell indices, row-major determinism).
/// Nodes = cells with degree ≠ 2; walk node→node through degree-2 cells, marking
/// each directed edge visited both ways, then sweep remaining edges to pick up
/// pure loops.
fn trace_skeleton(skel: &Grid2<bool>) -> Vec<Vec<usize>> {
    let n = skel.len();
    let nx = skel.nx();
    let mut visited: std::collections::HashSet<(usize, usize)> = Default::default();
    let mut polys: Vec<Vec<usize>> = Vec::new();

    let neighbors = |idx: usize| -> Vec<usize> {
        let (rr, cc) = crate::grid2::row_major_rc(idx, nx);
        let (r, c) = (rr as isize, cc as isize);
        NB8.iter()
            .filter_map(|&(dr, dc)| {
                let nidx = skel.index_of_signed(r + dr, c + dc)?;
                skel.at_index_or(nidx, false).then_some(nidx)
            })
            .collect()
    };
    let degree = |idx: usize| -> u32 {
        let (rr, cc) = crate::grid2::row_major_rc(idx, nx);
        skel_degree(skel, rr as isize, cc as isize)
    };

    // Walk from `start` toward `first`, consuming degree-2 cells until a node or
    // dead end. Returns the polyline of cell indices.
    let walk = |start: usize,
                first: usize,
                visited: &mut std::collections::HashSet<(usize, usize)>|
     -> Vec<usize> {
        let mut poly = vec![start];
        visited.insert((start, first));
        visited.insert((first, start));
        poly.push(first);
        let mut prev = start;
        let mut cur = first;
        loop {
            if degree(cur) != 2 {
                break; // reached a node
            }
            let mut next = None;
            for nbr in neighbors(cur) {
                if nbr == prev {
                    continue;
                }
                if visited.contains(&(cur, nbr)) {
                    continue;
                }
                next = Some(nbr);
                break;
            }
            match next {
                Some(nn) => {
                    visited.insert((cur, nn));
                    visited.insert((nn, cur));
                    poly.push(nn);
                    prev = cur;
                    cur = nn;
                }
                None => break, // dead end (or closed back onto a visited edge)
            }
        }
        poly
    };

    // Node-anchored walks first.
    for idx in 0..n {
        if !skel.at_index_or(idx, false) {
            continue;
        }
        if degree(idx) == 2 {
            continue; // not a node
        }
        for nbr in neighbors(idx) {
            if visited.contains(&(idx, nbr)) {
                continue;
            }
            let poly = walk(idx, nbr, &mut visited);
            if poly.len() >= 2 {
                polys.push(poly);
            }
        }
    }
    // Sweep remaining edges → pure loops (all cells degree 2).
    for idx in 0..n {
        if !skel.at_index_or(idx, false) {
            continue;
        }
        for nbr in neighbors(idx) {
            if visited.contains(&(idx, nbr)) {
                continue;
            }
            let poly = walk(idx, nbr, &mut visited);
            if poly.len() >= 2 {
                polys.push(poly);
            }
        }
    }
    polys
}

/// Shared hillshade-rendering support for the opt-in, fixture-driven visual
/// test harnesses in this module (`render_restfield_hillshade`) and in
/// [`crate::crest_lines`] (`render_crest_hillshade`). Both drop a tiny probe
/// ball on a grid over a real mesh, NW-lit-shade the resulting DEM into a
/// PNG, and overlay algorithm output (centerlines / valley lines) as bright
/// plus-marks. `#[cfg(test)] pub(crate)` so both modules' `--ignored`
/// harnesses can share it without pulling PNG rendering into production code.
#[cfg(test)]
pub(crate) mod hillshade_test_util {
    use crate::dropcutter::point_drop_cutter;
    use crate::geo::{BoundingBox3, P3};
    use crate::mesh::{SpatialIndex, TriangleMesh};
    use crate::tool::BallEndmill;

    /// A rasterized digital-elevation-model over a mesh's XY bbox, probed by
    /// dropping a tiny ball at each grid cell.
    pub(crate) struct HillshadeDem {
        pub(crate) nx: usize,
        pub(crate) ny: usize,
        pub(crate) cell: f64,
        pub(crate) bbox: BoundingBox3,
        z: Vec<f64>,
        valid: Vec<bool>,
    }

    impl HillshadeDem {
        /// Build a DEM by dropping a `probe_diameter`mm / `probe_length`mm
        /// ball across `mesh`'s bbox on a `cell`mm grid (typically the
        /// canonical [`crate::pencil::SURFACE_PROBE_BALL_DIAMETER_MM`] /
        /// [`crate::pencil::SURFACE_PROBE_BALL_LENGTH_MM`] pair).
        pub(crate) fn build(
            mesh: &TriangleMesh,
            index: &SpatialIndex,
            cell: f64,
            probe_diameter: f64,
            probe_length: f64,
        ) -> Self {
            let bbox = mesh.bbox;
            let nx = (((bbox.max.x - bbox.min.x) / cell).ceil() as usize).max(1) + 1;
            let ny = (((bbox.max.y - bbox.min.y) / cell).ceil() as usize).max(1) + 1;
            let probe = BallEndmill::new(probe_diameter, probe_length);
            let mut z = vec![f64::NAN; nx * ny];
            let mut valid = vec![false; nx * ny];
            for r in 0..ny {
                for c in 0..nx {
                    let x = bbox.min.x + c as f64 * cell;
                    let y = bbox.min.y + r as f64 * cell;
                    let cl = point_drop_cutter(x, y, mesh, index, &probe);
                    if cl.contacted {
                        #[allow(clippy::indexing_slicing)] // r,c bounded by nx,ny above
                        {
                            z[r * nx + c] = cl.z;
                            valid[r * nx + c] = true;
                        }
                    }
                }
            }
            Self {
                nx,
                ny,
                cell,
                bbox,
                z,
                valid,
            }
        }

        /// NW-lit hillshade render of this DEM (dark background where the
        /// probe never contacted the mesh).
        #[allow(clippy::indexing_slicing)] // all indices bounded by nx/ny by construction
        pub(crate) fn render(&self) -> image::RgbImage {
            let (nx, ny, cell) = (self.nx, self.ny, self.cell);
            let ll = -1.0 / 3.0_f64.sqrt();
            let light = [ll, ll, 1.0 / 3.0_f64.sqrt()];
            let mut img =
                image::RgbImage::from_pixel(nx as u32, ny as u32, image::Rgb([10, 10, 20]));
            for r in 0..ny {
                for c in 0..nx {
                    if !self.valid[r * nx + c] {
                        continue;
                    }
                    let (a, b) = (c.saturating_sub(1), (c + 1).min(nx - 1));
                    let zx = (self.z[r * nx + b] - self.z[r * nx + a]) / (2.0 * cell);
                    let (a2, b2) = (r.saturating_sub(1), (r + 1).min(ny - 1));
                    let zy = (self.z[b2 * nx + c] - self.z[a2 * nx + c]) / (2.0 * cell);
                    let nrm = (zx * zx + zy * zy + 1.0).sqrt();
                    let dot = ((-zx * light[0] - zy * light[1] + light[2]) / nrm).clamp(0.0, 1.0);
                    let g = (40.0 + dot * 200.0) as u8;
                    let py = (ny - 1 - r) as u32;
                    img.put_pixel(c as u32, py, image::Rgb([g, g, g]));
                }
            }
            img
        }

        /// Convert a world XY point to pixel coordinates in this DEM's image
        /// (Y flipped so row 0 is the image top). Returns signed coordinates
        /// so out-of-bounds callers can clip without an underflowing
        /// subtraction.
        pub(crate) fn to_px(&self, x: f64, y: f64) -> (i32, i32) {
            let c = ((x - self.bbox.min.x) / self.cell).round() as i32;
            let r = ((y - self.bbox.min.y) / self.cell).round() as i32;
            (c, self.ny as i32 - 1 - r)
        }
    }

    /// Stamp a bright 5-pixel "plus" mark at world coords `(x, y)` on `img`,
    /// clipped to bounds. Used to overlay traced polylines (pencil
    /// centerlines / crest valley lines) on the hillshade render.
    pub(crate) fn plot_plus(
        dem: &HillshadeDem,
        img: &mut image::RgbImage,
        x: f64,
        y: f64,
        color: image::Rgb<u8>,
    ) {
        let (px, py) = dem.to_px(x, y);
        for (ox, oy) in [(0i32, 0i32), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (xx, yy) = (px + ox, py + oy);
            if xx >= 0 && yy >= 0 && (xx as usize) < dem.nx && (yy as usize) < dem.ny {
                img.put_pixel(xx as u32, yy as u32, color);
            }
        }
    }

    /// Stamp plus-marks for every point of every polyline in `lines`.
    pub(crate) fn plot_polylines(
        dem: &HillshadeDem,
        img: &mut image::RgbImage,
        lines: &[Vec<P3>],
        color: image::Rgb<u8>,
    ) {
        for line in lines {
            for p in line {
                plot_plus(dem, img, p.x, p.y, color);
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests {
    use super::*;
    use crate::mesh::SpatialIndex;
    use crate::tool::BallEndmill;

    /// A symmetric V-valley running along X: two inclined planes meeting at y=0.
    /// `half_y` is the half-width, `slope` the wall rise/run, sampled `nx`×`ny`.
    fn make_v_valley(len_x: f64, half_y: f64, slope: f64, nx: usize, ny: usize) -> TriangleMesh {
        let mut verts = Vec::new();
        for iy in 0..=ny {
            let y = -half_y + 2.0 * half_y * iy as f64 / ny as f64;
            let z = y.abs() * slope; // V trough at y=0
            for ix in 0..=nx {
                let x = len_x * ix as f64 / nx as f64;
                verts.push(P3::new(x, y, z));
            }
        }
        let mut tris = Vec::new();
        let stride = nx + 1;
        for iy in 0..ny {
            for ix in 0..nx {
                let a = (iy * stride + ix) as u32;
                let b = a + 1;
                let cc = a + stride as u32;
                let d = cc + 1;
                // CCW so facet normals point up.
                tris.push([a, b, d]);
                tris.push([a, d, cc]);
            }
        }
        TriangleMesh::from_raw(verts, tris)
    }

    /// A tent RIDGE (inverted V): trough replaced by a crest at y=0.
    fn make_tent_ridge(len_x: f64, half_y: f64, slope: f64, nx: usize, ny: usize) -> TriangleMesh {
        let mut verts = Vec::new();
        for iy in 0..=ny {
            let y = -half_y + 2.0 * half_y * iy as f64 / ny as f64;
            let z = (half_y - y.abs()) * slope; // crest at y=0
            for ix in 0..=nx {
                let x = len_x * ix as f64 / nx as f64;
                verts.push(P3::new(x, y, z));
            }
        }
        let mut tris = Vec::new();
        let stride = nx + 1;
        for iy in 0..ny {
            for ix in 0..nx {
                let a = (iy * stride + ix) as u32;
                let b = a + 1;
                let cc = a + stride as u32;
                let d = cc + 1;
                tris.push([a, b, d]);
                tris.push([a, d, cc]);
            }
        }
        TriangleMesh::from_raw(verts, tris)
    }

    fn make_flat(size: f64) -> TriangleMesh {
        let v = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(size, 0.0, 0.0),
            P3::new(size, size, 0.0),
            P3::new(0.0, size, 0.0),
        ];
        TriangleMesh::from_raw(v, vec![[0, 1, 2], [0, 2, 3]])
    }

    fn default_params(pencil_r: f64) -> RestFieldParams {
        RestFieldParams {
            cell_mm: 0.5,
            min_valley_depth: 0.05,
            route_width_factor: 2.0,
            pencil_radius: pencil_r,
            min_cut_length: 2.0,
        }
    }

    #[test]
    fn v_valley_yields_one_centerline() {
        // Sharp narrow V: a 6mm reference ball bridges it, a 1mm pencil enters.
        // A wide route_width_factor forces the (fairly wide) trough band to route
        // to pencil rather than clearing so we can assert on the centerline.
        let mesh = make_v_valley(30.0, 4.0, 1.2, 30, 24);
        let index = SpatialIndex::build_auto(&mesh);
        let pencil = BallEndmill::new(1.0, 25.0);
        let reference = BallEndmill::new(6.0, 25.0);
        let mut p = default_params(0.5);
        p.route_width_factor = 10.0;
        let res = detect_rest_valleys(
            &mesh,
            &index,
            &pencil,
            RestReference::Cutter {
                tool: &reference,
                is_surface_probe: false,
            },
            &p,
        );
        assert!(
            !res.centerlines.is_empty(),
            "sharp V-valley should yield a pencil centerline; report = {:?}",
            res.report
        );
        // The trough runs along X near y=0.
        let all: Vec<&P3> = res.centerlines.iter().flatten().collect();
        let mean_abs_y = all.iter().map(|p| p.y.abs()).sum::<f64>() / all.len().max(1) as f64;
        assert!(
            mean_abs_y < 1.5,
            "centerline should hug the y≈0 trough, mean |y| = {mean_abs_y:.2}"
        );
    }

    #[test]
    fn tent_ridge_yields_none() {
        // A realistic (gently rounded) convex ridge leaves no rest — both tools
        // roll over it equally. NB a *razor-sharp* synthetic crest does produce a
        // shallow ride-over band, but on real rounded relief (and above any
        // sensible min_valley_depth) it filters out — verified on terrain.stl,
        // where green centerlines sit only in the dark drainage valleys.
        let mesh = make_tent_ridge(30.0, 8.0, 0.3, 30, 24);
        let index = SpatialIndex::build_auto(&mesh);
        let pencil = BallEndmill::new(1.0, 25.0);
        let reference = BallEndmill::new(6.0, 25.0);
        let res = detect_rest_valleys(
            &mesh,
            &index,
            &pencil,
            RestReference::Cutter {
                tool: &reference,
                is_surface_probe: false,
            },
            &default_params(0.5),
        );
        assert!(
            res.centerlines.is_empty(),
            "a gentle ridge has no rest material; got {} centerlines (peaks {:?})",
            res.centerlines.len(),
            res.report.region_peak_rest_mm
        );
    }

    #[test]
    fn flat_plate_empty() {
        let mesh = make_flat(30.0);
        let index = SpatialIndex::build_auto(&mesh);
        let pencil = BallEndmill::new(1.0, 25.0);
        let reference = BallEndmill::new(6.0, 25.0);
        let res = detect_rest_valleys(
            &mesh,
            &index,
            &pencil,
            RestReference::Cutter {
                tool: &reference,
                is_surface_probe: false,
            },
            &default_params(0.5),
        );
        assert!(res.centerlines.is_empty(), "flat plate has no valleys");
        assert_eq!(res.report.total_rest_volume_mm3, 0.0);
    }

    #[test]
    fn gentle_reachable_valley_yields_none() {
        // A broad shallow valley both tools reach → rest ≈ 0.
        let mesh = make_v_valley(30.0, 12.0, 0.05, 30, 24);
        let index = SpatialIndex::build_auto(&mesh);
        let pencil = BallEndmill::new(1.0, 25.0);
        let reference = BallEndmill::new(6.0, 25.0);
        let res = detect_rest_valleys(
            &mesh,
            &index,
            &pencil,
            RestReference::Cutter {
                tool: &reference,
                is_surface_probe: false,
            },
            &default_params(0.5),
        );
        assert!(
            res.centerlines.is_empty(),
            "a gentle reachable valley should read ≈0 rest; got {} centerlines",
            res.centerlines.len()
        );
    }

    #[test]
    fn min_valley_depth_monotonically_thins() {
        let mesh = make_v_valley(30.0, 4.0, 1.2, 30, 24);
        let index = SpatialIndex::build_auto(&mesh);
        let pencil = BallEndmill::new(1.0, 25.0);
        let reference = BallEndmill::new(6.0, 25.0);
        let count = |mvd: f64| {
            let mut p = default_params(0.5);
            p.min_valley_depth = mvd;
            let r = detect_rest_valleys(
                &mesh,
                &index,
                &pencil,
                RestReference::Cutter {
                    tool: &reference,
                    is_surface_probe: false,
                },
                &p,
            );
            r.centerlines.iter().map(|l| l.len()).sum::<usize>()
        };
        let low = count(0.05);
        let high = count(1.0);
        assert!(
            high <= low,
            "raising min_valley_depth must not grow the trace: {low} → {high}"
        );
    }

    #[test]
    fn reference_diameter_moves_detection() {
        // Bigger reference ⇒ more rest ⇒ more/longer trace. The signature ability
        // no previous detector had.
        // Peak rest depth at the trough (deep interior, survives boundary erosion)
        // is the erosion-robust significance signal; a bigger reference bridges
        // more and reads a deeper peak.
        let mesh = make_v_valley(40.0, 10.0, 0.5, 40, 32);
        let index = SpatialIndex::build_auto(&mesh);
        let pencil = BallEndmill::new(1.0, 25.0);
        let peak = |ref_d: f64| {
            let reference = BallEndmill::new(ref_d, 25.0);
            let res = detect_rest_valleys(
                &mesh,
                &index,
                &pencil,
                RestReference::Cutter {
                    tool: &reference,
                    is_surface_probe: false,
                },
                &default_params(0.5),
            );
            res.report
                .region_peak_rest_mm
                .iter()
                .cloned()
                .fold(0.0, f64::max)
        };
        let small = peak(3.0);
        let big = peak(12.0);
        assert!(
            big > small,
            "a bigger reference tool must deepen the rest field: {small:.2} → {big:.2}"
        );
    }

    #[test]
    fn chamfer_dt_center_of_band() {
        // A 5-wide horizontal band: the centre row should have DT ≈ 3 (distance
        // to the unset row above/below is 3 cells).
        let (nx, ny) = (9usize, 9usize);
        let mut mask = Grid2::new_fill(nx, ny, false);
        for r in 2..7 {
            for c in 0..nx {
                mask.set(r, c, true);
            }
        }
        let dt = chamfer_distance(&mask);
        let center = *dt.get(4, 4).unwrap();
        assert!(
            (center - 3.0).abs() < 0.6,
            "centre of a 5-row band should be ~3 cells from the edge, got {center:.2}"
        );
    }

    #[test]
    fn thinning_reduces_a_thick_line_to_one_cell() {
        // A 3-cell-thick horizontal bar thins to a single row.
        let (nx, ny) = (12usize, 7usize);
        let mut mask = Grid2::new_fill(nx, ny, false);
        for r in 3..6 {
            for c in 1..11 {
                mask.set(r, c, true);
            }
        }
        let skel = zhang_suen_thin(&mask);
        // Each column in the interior keeps exactly one set cell.
        for c in 3..9 {
            let col_set: usize = (0..ny).filter(|&r| *skel.get(r, c).unwrap()).count();
            assert_eq!(
                col_set, 1,
                "column {c} should thin to one cell, got {col_set}"
            );
        }
    }

    #[test]
    fn trace_recovers_a_straight_skeleton() {
        let (nx, ny) = (12usize, 5usize);
        let mut skel = Grid2::new_fill(nx, ny, false);
        for c in 1..11 {
            skel.set(2, c, true);
        }
        let polys = trace_skeleton(&skel);
        assert_eq!(polys.len(), 1, "one straight skeleton → one polyline");
        assert_eq!(polys[0].len(), 10, "should trace all 10 cells");
    }

    /// Hillshade overlay harness (the key view). Renders slope-shaded terrain,
    /// pencil-routed centerlines in green, clearing-routed region bboxes in
    /// orange, and writes the RestFieldReport to a sidecar .txt.
    ///
    ///   RS_CAM_REST_FIXTURE=/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl \
    ///   RS_CAM_REST_OUT=/tmp/rest.png RS_CAM_REST_CELL=0.5 RS_CAM_REST_MVD=0.2 \
    ///   RS_CAM_REST_REFD=6 RS_CAM_REST_ROUTEW=2.0 RS_CAM_REST_PENCILD=2.0 \
    ///   cargo test -p rs_cam_core --lib render_restfield_hillshade -- --ignored --nocapture
    #[test]
    #[ignore = "needs RS_CAM_REST_FIXTURE; writes hillshade PNG to RS_CAM_REST_OUT"]
    fn render_restfield_hillshade() {
        let envf = |k: &str, d: f64| {
            std::env::var(k)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(d)
        };
        let path = std::env::var("RS_CAM_REST_FIXTURE").unwrap();
        let out = std::env::var("RS_CAM_REST_OUT").unwrap_or_else(|_| "/tmp/rest.png".into());
        let cell = envf("RS_CAM_REST_CELL", 0.5);
        let mvd = envf("RS_CAM_REST_MVD", 0.2);
        let refd = envf("RS_CAM_REST_REFD", 6.0);
        let routew = envf("RS_CAM_REST_ROUTEW", 2.0);
        let pencild = envf("RS_CAM_REST_PENCILD", 2.0);

        let mesh = TriangleMesh::from_stl(std::path::Path::new(&path)).unwrap();
        let index = SpatialIndex::build_auto(&mesh);
        let pencil = BallEndmill::new(pencild, crate::pencil::NOMINAL_REFERENCE_BALL_LENGTH_MM);
        let probe_mode = refd <= pencild + 1e-6;
        let reference = if probe_mode {
            BallEndmill::new(
                crate::pencil::SURFACE_PROBE_BALL_DIAMETER_MM,
                crate::pencil::SURFACE_PROBE_BALL_LENGTH_MM,
            )
        } else {
            BallEndmill::new(refd, crate::pencil::NOMINAL_REFERENCE_BALL_LENGTH_MM)
        };

        let params = RestFieldParams {
            cell_mm: cell,
            min_valley_depth: mvd,
            route_width_factor: routew,
            pencil_radius: pencil.radius(),
            min_cut_length: 2.0,
        };
        let reference = RestReference::Cutter {
            tool: &reference,
            is_surface_probe: probe_mode,
        };
        let t = std::time::Instant::now();
        let res = detect_rest_valleys(&mesh, &index, &pencil, reference, &params);
        let gen_ms = t.elapsed().as_millis();

        // Hillshade DEM (tiny-ball probe grid) — shared with crest_lines's
        // render_crest_hillshade via hillshade_test_util.
        let dem = hillshade_test_util::HillshadeDem::build(
            &mesh,
            &index,
            cell,
            crate::pencil::SURFACE_PROBE_BALL_DIAMETER_MM,
            crate::pencil::SURFACE_PROBE_BALL_LENGTH_MM,
        );
        let (gnx, gny) = (dem.nx, dem.ny);
        let mut img = dem.render();
        // Clearing-region bboxes in orange (drawn first, under the green lines).
        for reg in &res.clearing_regions {
            let (x0, y0, x1, y1) = (reg.bbox[0], reg.bbox[1], reg.bbox[2], reg.bbox[3]);
            let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
            for k in 0..4 {
                let (ax, ay) = corners[k];
                let (bx, by) = corners[(k + 1) % 4];
                let steps = 200;
                for s in 0..=steps {
                    let t = s as f64 / steps as f64;
                    let (px, py) = dem.to_px(ax + (bx - ax) * t, ay + (by - ay) * t);
                    if px >= 0 && py >= 0 && (px as usize) < gnx && (py as usize) < gny {
                        img.put_pixel(px as u32, py as u32, image::Rgb([255, 160, 40]));
                    }
                }
            }
        }
        // Pencil centerlines in bright green.
        hillshade_test_util::plot_polylines(
            &dem,
            &mut img,
            &res.centerlines,
            image::Rgb([60, 255, 90]),
        );
        img.save(&out).unwrap();
        std::fs::write(
            out.replace(".png", ".txt"),
            format!(
                "cell={cell} mvd={mvd} refd={refd} routew={routew} pencild={pencild} \
                 probe_mode={probe_mode}\n\
                 grid={}x{} gen_ms={gen_ms}\n\
                 rest_volume_mm3={:.1} pencil_regions={} clearing_regions={} \
                 centerlines={}\n\
                 skeleton_mm={:.1} traced_mm={:.1} coverage={:.3}\n\
                 region_peaks(mm)={:?}\n",
                res.report.grid_nx,
                res.report.grid_ny,
                res.report.total_rest_volume_mm3,
                res.report.pencil_region_count,
                res.report.clearing_region_count,
                res.centerlines.len(),
                res.report.skeleton_length_mm,
                res.report.traced_length_mm,
                res.report.coverage(),
                res.report
                    .region_peak_rest_mm
                    .iter()
                    .take(10)
                    .map(|v| format!("{v:.2}"))
                    .collect::<Vec<_>>(),
            ),
        )
        .ok();
        println!(
            "wrote {out} | grid {}x{} | {} centerlines | {} clearing | coverage {:.2} | {gen_ms}ms",
            res.report.grid_nx,
            res.report.grid_ny,
            res.centerlines.len(),
            res.clearing_regions.len(),
            res.report.coverage(),
        );
    }
}
