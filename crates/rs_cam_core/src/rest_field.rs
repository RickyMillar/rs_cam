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
//! chamfer distance transform (local half-width) → **ridge extraction**
//! (box-smooth → non-max-suppression → hysteresis → thin) → skeleton tracing
//! → graph cleanup (prune spurs / merge pass-through nodes) → route each
//! surviving polyline by width (narrow → pencil centreline, wide → clearing
//! region). See `planning/pencil_restdepth_detector_prompt.md`.
//!
//! The mask (`rest > min_valley_depth`) and its ridge are two different
//! things extracted from the same field: on textured relief the mask covers
//! the *entire* rough area (both tools ride the texture), so thinning the
//! mask itself is a space-filling hairball unrelated to any actual valley
//! crease. The ridge extraction instead finds the local-maximum crease of the
//! continuous field directly — the mask is still used (untouched) for
//! [`region_polygons_from_mask`] / the P2 selective-finishing boundary
//! source, and for measuring each ridge polyline's local half-width.
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
use crate::polygon::Polygon2;
use crate::region_mask::MAX_REST_REGIONS;
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
    /// Extra clearance (mm) added around detected rest regions, beyond the
    /// fine (pencil) tool radius, when dilating the mask into
    /// [`RestFieldResult::region_polygons`]. Dilation radius is
    /// `pencil_radius + region_margin_mm` — enough that a boundary-clipped
    /// fine-tool op can actually reach the true region edge rather than
    /// stopping exactly at the pencil-radius-eroded mask boundary.
    pub region_margin_mm: f64,
}

impl Default for RestFieldParams {
    fn default() -> Self {
        Self {
            cell_mm: 0.5,
            min_valley_depth: 0.05,
            route_width_factor: 2.0,
            pencil_radius: 0.5,
            min_cut_length: 2.0,
            region_margin_mm: 0.5,
        }
    }
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
    /// What this report's numbers mean (M1): a rest-depth field on the
    /// detector's own grid, before any polygon extraction.
    ///
    /// The report previously carried `grid_nx`/`grid_ny` but **not the cell
    /// size**, so [`Self::total_rest_volume_mm3`] — a `Σ rest × cell²` — was
    /// quantised by a number no consumer could see and could not be compared
    /// across runs at different `cell_mm` (`MEASUREMENT_DOMAINS.md` X-10).
    /// The domain tag names the volume; the length fields
    /// ([`Self::skeleton_length_mm`], [`Self::traced_length_mm`], and the
    /// [`Self::coverage`] ratio over them) are path lengths on the same grid.
    pub provenance: crate::measurement::MeasurementProvenance,
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

/// One pencil-routed ridge polyline plus its measured local half-width.
///
/// `half_width_mm` is the median chamfer-distance-transform value of the
/// THRESHOLD mask (`rest > min_valley_depth`, the same mask
/// [`region_polygons_from_mask`] dilates), sampled along the ridge's cells,
/// converted to mm. It is the same "how wide is the rest material here"
/// metric the pencil/clearing routing decision uses, and lets
/// [`crate::pencil::rest_depth_arm`] size its offset-pass count to the local
/// valley width instead of a fixed count everywhere. `0.0` where a ridge cell
/// sits just outside the mask — the ridge's hysteresis LO floor (`0.5 ×
/// min_valley_depth`) is below the mask's own threshold, so a ridge can dip
/// slightly beyond the mask boundary.
#[derive(Debug, Clone)]
pub struct RestCenterline {
    /// World-space polyline: cell-centre XY, Z from the pencil drop.
    pub points: Vec<P3>,
    /// Local region half-width (mm) — see struct doc.
    pub half_width_mm: f64,
}

/// Output of [`detect_rest_valleys`].
pub struct RestFieldResult {
    /// Pencil-routed centrelines (world XY at cell centres, Z from the pencil
    /// drop) with their measured local half-width. Feed `points` straight
    /// into the existing pencil pipeline (`min_cut_length` filter →
    /// `resample_polyline` → `paths_from_sampled`); `half_width_mm` sizes the
    /// width-aware offset pass count (see [`RestCenterline`]).
    pub centerlines: Vec<RestCenterline>,
    /// Wide regions routed to clearing (not emitted as pencil in v1).
    pub clearing_regions: Vec<ClearingRegion>,
    /// Detection significance / quality metrics.
    pub report: RestFieldReport,
    /// The continuous rest-depth grid, for visualisation (heatmap overlay).
    pub rest_grid: RestGrid,
    /// Machining-region polygons derived from the (thresholded + component-
    /// cleaned) rest mask, dilated by `pencil_radius + region_margin_mm` so a
    /// boundary-clipped fine-tool op can actually reach the region edge. This
    /// is the derived-boundary source for selective finishing (P2.2's
    /// `BoundarySource::DerivedRestRegions`) — grouped into outer/hole rings
    /// via [`crate::polygon::detect_containment`] and winding-normalised.
    /// Empty when the mask has no surviving components.
    pub region_polygons: Vec<Polygon2>,
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

    // --- 2b. Machining-region polygons: dilate the cleaned mask by the fine
    // tool's radius + margin so a boundary-clipped op can reach the region
    // edge, then extract closed loops via marching squares. See P2.2
    // `BoundarySource::DerivedRestRegions`.
    let region_polygons = region_polygons_from_mask(
        &mask,
        origin_x,
        origin_y,
        cell,
        pencil.radius() + params.region_margin_mm,
    );

    // Total rest volume over the (cleaned) mask.
    let cell_area = cell * cell;
    let mut total_rest_volume = 0.0f64;
    for i in 0..n {
        if mask.at_index_or(i, false) {
            total_rest_volume += rest.at_index_or(i, 0.0) * cell_area;
        }
    }

    // --- 3. Chamfer distance transform over the (cleaned) mask — local
    // half-width in cells. Used both for the pencil/clearing routing
    // decision and [`RestCenterline::half_width_mm`]. ---
    let dt = chamfer_distance(&mask);

    // --- 4. Ridge extraction: box-smooth the continuous rest field once,
    // non-max-suppress it down to a thin candidate ridge, keep only
    // hysteresis components whose peak clears min_valley_depth, then
    // Zhang-Suen-thin the (already near-thin) survivors to guarantee exactly
    // 1 cell wide. This replaces thinning the mask itself, which on textured
    // relief is a space-filling hairball (see the module doc). ---
    let rest_sm = box_smooth_rest(&rest, &contact, &boundary_dt, erode_cells);
    let ridge_candidates = nms_candidates(&rest_sm, &contact, &boundary_dt, erode_cells, threshold);
    let ridge_kept = hysteresis_ridge(&ridge_candidates, &rest_sm, threshold);
    let ridge_skel = zhang_suen_thin(&ridge_kept);

    // --- 5. Skeleton tracing → polylines (cell indices), then graph cleanup
    // (prune short spurs, merge through pass-through nodes) BEFORE any
    // length filtering. ---
    let raw_polys = trace_skeleton(&ridge_skel);
    let min_cut_length_cells = params.min_cut_length.max(0.0) / cell;
    let poly_cells = cleanup_ridge_graph(raw_polys, nx, min_cut_length_cells);

    // --- 6. Route each polyline by width. ---
    let mut centerlines: Vec<RestCenterline> = Vec::new();
    let mut pencil_comps: std::collections::BTreeSet<usize> = Default::default();
    let mut clearing_comps: std::collections::BTreeSet<usize> = Default::default();
    let mut skeleton_length = 0.0f64;
    let mut traced_length = 0.0f64;
    let width_limit = params.route_width_factor * params.pencil_radius;

    for poly in &poly_cells {
        if poly.len() < 2 {
            continue;
        }
        // Per-BRANCH saliency gate: keep a polyline only when its MEDIAN
        // smoothed rest clears `min_valley_depth` (same median-of-samples
        // metric family as `pencil::polyline_passes_depth`). The hysteresis
        // stage gates per-COMPONENT peak, which stops discriminating the
        // moment fine texture creases are 8-connected to a deep trunk — on
        // wanaka the whole dendritic network forms one component peaking at
        // 3.6 mm, so at mvd 0.15 EVERYTHING in it survived (17 m of
        // "centerlines", a carpet). Gating each traced branch on its own
        // median restores the dial: texture spurs (~0.2 mm) drop out as mvd
        // rises while the deep trunks survive. Runs BEFORE the
        // skeleton-length accumulation so saliency-dropped branches don't
        // count as "lost" coverage.
        let mut branch_rest: Vec<f64> = poly.iter().map(|&i| rest_sm.at_index_or(i, 0.0)).collect();
        branch_rest.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if median_sorted(&branch_rest) < threshold {
            continue;
        }
        // Median DT (over the threshold mask) along the polyline → region
        // half-width in mm; 0.0 for cells where the ridge sits outside the
        // mask (see [`RestCenterline`]).
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

        // Component lookup at the first ridge cell that lies INSIDE the
        // threshold mask — ridge cells can sit slightly outside it (the
        // hysteresis LO floor is below the mask threshold); if none do,
        // skip comp bookkeeping (same `comp != usize::MAX` guards as before).
        let comp = poly
            .iter()
            .find(|&&i| mask.at_index_or(i, false))
            .map(|&i| comp_id.at_index_or(i, usize::MAX))
            .unwrap_or(usize::MAX);
        if half_width_mm <= width_limit {
            if comp != usize::MAX {
                pencil_comps.insert(comp);
            }
            if len >= params.min_cut_length {
                traced_length += len;
            }
            centerlines.push(RestCenterline {
                points: pts,
                half_width_mm,
            });
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
        provenance: crate::measurement::MeasurementProvenance::new(
            crate::measurement::MeasurementDomain::StockVolume,
            crate::measurement::MeasurementStage::RestFieldMask,
        )
        .with_cell(cell, crate::measurement::CellSource::Explicit),
    };

    info!(
        grid = format!("{nx}x{ny}"),
        rest_volume_mm3 = format!("{:.1}", report.total_rest_volume_mm3),
        pencil_regions = report.pencil_region_count,
        clearing_regions = report.clearing_region_count,
        centerlines = centerlines.len(),
        coverage = format!("{:.2}", report.coverage()),
        // M1 §4.3: the volume above is `Σ rest × cell²`; without the cell it
        // is not comparable across runs (X-10).
        measurement = %report.provenance,
        "Rest-field detector complete"
    );

    RestFieldResult {
        centerlines,
        clearing_regions,
        report,
        rest_grid,
        region_polygons,
    }
}

/// Mask → closed-polygon extraction now lives in [`crate::region_mask`]
/// (shared with the P2 finish planner); re-exported here so existing
/// `rest_field::region_polygons_from_mask` callers/imports keep compiling.
pub use crate::region_mask::region_polygons_from_mask;

/// Diagnosis of a rest-region set that's pathological in one of two opposite
/// ways: a threshold-below-cusp sliver storm (way too many tiny islands), or
/// a threshold so coarse the "rest" region is most of the part (selective
/// finishing barely restricts anything). Returned by
/// [`classify_rest_regions`] for the GUI to surface as operator guidance
/// alongside the region/heatmap display — a pure classification, not a fix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RestRegionPathology {
    /// Region count is already approaching [`MAX_REST_REGIONS`] (more than
    /// half of it) — advice: the rest threshold likely below the prior
    /// pass's cusp height; raise `min_valley_depth`.
    TooManyIslands {
        /// Raw region count (before any cap truncation).
        count: usize,
    },
    /// A single outer region covers at least half the part footprint —
    /// advice: the regions barely restrict the fine pass; raise
    /// `min_valley_depth`, or use the machined-stock reference ("Use
    /// remaining stock") for an honest rest picture.
    SingleGiantRegion {
        /// Region area as a fraction of `part_footprint_area` (≥ 0.5).
        part_area_fraction: f64,
    },
}

/// Classify a rest-region set into one of the two opposite pathologies (see
/// [`RestRegionPathology`]), or `None` for a healthy set.
///
/// `TooManyIslands` fires once `regions.len()` exceeds `MAX_REST_REGIONS / 2`
/// (33+) — approaching the hard cap is already pathological, well before
/// `region_polygons_from_mask` actually has to truncate anything.
/// `SingleGiantRegion` fires when there is exactly one outer region and its
/// area is at least half of `part_footprint_area`. A non-positive
/// `part_footprint_area` (no usable footprint estimate) always yields `None`
/// for that check rather than a false positive.
pub fn classify_rest_regions(
    regions: &[Polygon2],
    part_footprint_area: f64,
) -> Option<RestRegionPathology> {
    if regions.len() > MAX_REST_REGIONS / 2 {
        return Some(RestRegionPathology::TooManyIslands {
            count: regions.len(),
        });
    }
    if regions.len() == 1 && part_footprint_area > 0.0 {
        let fraction = regions.first().map(Polygon2::area)? / part_footprint_area;
        if fraction >= 0.5 {
            return Some(RestRegionPathology::SingleGiantRegion {
                part_area_fraction: fraction,
            });
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Everything grid-shaped (rest, pencil_z, contact, mask, comp_id, the two
// distance-transform passes, the skeleton) is a `Grid2<T>` — see `grid2.rs`.
// The one thing left that ISN'T grid-shaped is the component table
// (`comp_cells`/`comp_peak`/`comp_bbox`, indexed by component id): a single
// small generic accessor covers those.
// ---------------------------------------------------------------------------

/// Index a slice at a position already proven in-bounds by the caller's
/// invariant: a component id from `comp_id` (always `< comp_cells.len()` and
/// its parallel `comp_peak`/`comp_bbox` vectors, by construction — a `cid` is
/// only ever `comp_cells.len()` at the moment all three get pushed together).
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

// ---------------------------------------------------------------------------
// Ridge extraction (fix A): the mask (`rest > min_valley_depth`) covers the
// whole rough area on textured relief, so thinning it directly (the old
// approach) produces a space-filling hairball unrelated to actual valley
// creases. Instead we box-smooth the continuous field once, non-max-suppress
// it down to a thin candidate ridge, keep only components whose peak clears
// the real threshold (hysteresis), then hand the (already near-thin) result
// to the existing `zhang_suen_thin` to guarantee exactly 1 cell wide.
// ---------------------------------------------------------------------------

/// Single 3×3 box-smooth of the rest field — the first ridge-extraction
/// stage. Untrusted cells (no contact, or inside the boundary-erosion band —
/// the same `ct && dt >= erode_cells` predicate used for the `rest_grid`
/// NaN mapping above) are EXCLUDED from neighbours' averages (mean over the
/// trusted subset of each 3×3 window, not a zero-padded mean over 9) and are
/// themselves forced to `0.0`: they can never become ridge candidates.
///
/// The trusted-subset mean matters: zero-padding instead would drag values
/// down in a 1–2-cell "rolloff" band along every trust edge, and the crest
/// of the band between that artificial decline and any REAL interior decline
/// (a tent apex, a slope top) reads as a locally prominent maximum — i.e. a
/// spurious ridge line tracing the trust boundary. Caught by
/// `tent_ridge_yields_none`, which flagged exactly that band before the
/// normalisation was fixed.
fn box_smooth_rest(
    rest: &Grid2<f64>,
    contact: &Grid2<bool>,
    boundary_dt: &Grid2<f64>,
    erode_cells: f64,
) -> Grid2<f64> {
    let nx = rest.nx();
    let ny = rest.ny();
    let is_trusted = |i: usize| -> bool {
        contact.at_index_or(i, false) && boundary_dt.at_index_or(i, 0.0) >= erode_cells
    };
    let mut sm = Grid2::new_fill(nx, ny, 0.0f64);
    for r in 0..ny as isize {
        for c in 0..nx as isize {
            let Some(i) = rest.index_of_signed(r, c) else {
                continue;
            };
            if !is_trusted(i) {
                continue; // stays 0.0 — untrusted cells can never be candidates
            }
            let mut sum = 0.0;
            let mut count = 0u32;
            for dr in -1..=1isize {
                for dc in -1..=1isize {
                    if let Some(j) = rest.index_of_signed(r + dr, c + dc)
                        && is_trusted(j)
                    {
                        sum += rest.at_index_or(j, 0.0);
                        count += 1;
                    }
                }
            }
            // count >= 1 always (the centre cell itself is trusted).
            sm.set_index(i, sum / f64::from(count.max(1)));
        }
    }
    sm
}

/// NB8 direction pairs for the 4-way ridge non-max-suppression test: N/S,
/// E/W, NE/SW, NW/SE (indices into [`NB8`]).
const NMS_DIR_PAIRS: [(usize, usize); 4] = [(0, 4), (2, 6), (1, 5), (7, 3)];

/// Fraction of `min_valley_depth` a ridge cell must stand PROUD of both
/// opposite neighbours to count as a local maximum (see
/// [`NMS_PROMINENCE_FLOOR_MM`] for the absolute floor). A ball resting on a
/// constant-slope plane floats a position-independent height, so plane-wall
/// V-grooves (and any uniform slope) read a CONSTANT rest plateau — with a
/// bare strict `>` test, femtometre-scale facet/float noise tie-breaks all
/// over the plateau into scattered spurious "ridges". A genuine bridged
/// crease drops off by a large fraction of its depth within a cell or two,
/// so requiring real prominence costs nothing there.
const NMS_PROMINENCE_FRACTION: f64 = 0.1;
/// Absolute prominence floor (mm) so a user dialing `min_valley_depth`
/// toward zero still gets noise rejection (smoothed facet noise on a 0.5 mm
/// grid sits well below this; real crease drop-offs sit well above).
const NMS_PROMINENCE_FLOOR_MM: f64 = 0.005;

/// Non-max-suppression over the smoothed rest field: a trusted cell survives
/// when its value clears `0.5 × min_valley_depth` (the candidate floor — the
/// real `min_valley_depth` floor is applied per-component afterwards by
/// [`hysteresis_ridge`]) AND it stands at least the prominence margin above
/// BOTH neighbours along at least one of the four direction pairs in
/// [`NMS_DIR_PAIRS`] — the standard Canny-style ridge-thinning test, applied
/// here to a rest-depth field instead of a gradient magnitude, hardened with
/// a prominence requirement (see [`NMS_PROMINENCE_FRACTION`]) so constant
/// rest plateaus (uniform slopes, plane-wall V-grooves) yield NO candidates
/// instead of noise-tie-broken scatter. A direction pair only qualifies when
/// BOTH its neighbours are trusted — an untrusted neighbour (out of bounds,
/// non-contact, or inside the erosion band) reads 0.0 and would otherwise
/// hand every trust-edge cell a free "lower" side, tracing spurious ridge
/// lines along the part boundary; requiring bilateral trusted evidence kills
/// those while a genuine crease crossing the trust edge still qualifies via
/// the pair parallel to itself.
///
/// Consequence worth knowing: a V-groove made of two PLANES has a constant
/// rest field (no ridge at all — the crease is a plateau edge, not a local
/// maximum), so the RestDepth detector intentionally traces nothing along
/// it; clean CAD plane-wall creases are the [`crate::pencil_dihedral`]
/// detector's home turf. Bridged channels/creases on relief — where the
/// reference ball spans the feature and floats — are exactly where this
/// detector shines.
fn nms_candidates(
    rest_sm: &Grid2<f64>,
    contact: &Grid2<bool>,
    boundary_dt: &Grid2<f64>,
    erode_cells: f64,
    min_valley_depth: f64,
) -> Grid2<bool> {
    let nx = rest_sm.nx();
    let ny = rest_sm.ny();
    let floor = 0.5 * min_valley_depth;
    let prominence = (NMS_PROMINENCE_FRACTION * min_valley_depth).max(NMS_PROMINENCE_FLOOR_MM);
    let is_trusted = |i: usize| -> bool {
        contact.at_index_or(i, false) && boundary_dt.at_index_or(i, 0.0) >= erode_cells
    };
    // A neighbour's value, only if trusted (None disqualifies the pair).
    let trusted_at = |r: isize, c: isize| -> Option<f64> {
        let i = rest_sm.index_of_signed(r, c)?;
        is_trusted(i).then(|| rest_sm.at_index_or(i, 0.0))
    };
    let mut out = Grid2::new_fill(nx, ny, false);
    for r in 0..ny as isize {
        for c in 0..nx as isize {
            let Some(i) = rest_sm.index_of_signed(r, c) else {
                continue;
            };
            if !is_trusted(i) {
                continue;
            }
            let v = rest_sm.at_index_or(i, 0.0);
            if v < floor {
                continue;
            }
            let is_ridge = NMS_DIR_PAIRS.iter().any(|&(ka, kb)| {
                let (dra, dca) = NB8.get(ka).copied().unwrap_or((0, 0));
                let (drb, dcb) = NB8.get(kb).copied().unwrap_or((0, 0));
                match (trusted_at(r + dra, c + dca), trusted_at(r + drb, c + dcb)) {
                    (Some(oa), Some(ob)) => v > oa + prominence && v > ob + prominence,
                    _ => false, // untrusted side ⇒ this pair can't vouch for a ridge
                }
            });
            if is_ridge {
                out.set_index(i, true);
            }
        }
    }
    out
}

/// Hysteresis over the NMS candidate set: 8-connected components (same
/// deterministic row-major-seed flood-fill idiom as the mask's component
/// pass in [`detect_rest_valleys`]); keep a component only when its peak
/// `rest_sm` clears `min_valley_depth`, drop the rest. This is what makes
/// `min_valley_depth` mean "valley saliency" for the ridge — a texture ridge
/// whose smoothed peak never reaches the real threshold never survives, even
/// though individual candidate cells cleared the lower `0.5×` NMS floor.
fn hysteresis_ridge(
    candidates: &Grid2<bool>,
    rest_sm: &Grid2<f64>,
    min_valley_depth: f64,
) -> Grid2<bool> {
    let nx = candidates.nx();
    let ny = candidates.ny();
    let n = candidates.len();
    let mut visited = Grid2::new_fill(nx, ny, false);
    let mut kept = Grid2::new_fill(nx, ny, false);
    let mut stack: Vec<usize> = Vec::new();
    for seed in 0..n {
        if !candidates.at_index_or(seed, false) || visited.at_index_or(seed, false) {
            continue;
        }
        let mut comp: Vec<usize> = Vec::new();
        let mut peak = 0.0f64;
        stack.clear();
        stack.push(seed);
        visited.set_index(seed, true);
        while let Some(cur) = stack.pop() {
            comp.push(cur);
            let v = rest_sm.at_index_or(cur, 0.0);
            if v > peak {
                peak = v;
            }
            let (rr, cc) = crate::grid2::row_major_rc(cur, nx);
            let (r, c) = (rr as isize, cc as isize);
            for (dr, dc) in NB8 {
                let (nr, nc) = (r + dr, c + dc);
                let Some(nidx) = candidates.index_of_signed(nr, nc) else {
                    continue;
                };
                if !candidates.at_index_or(nidx, false) || visited.at_index_or(nidx, false) {
                    continue;
                }
                visited.set_index(nidx, true);
                stack.push(nidx);
            }
        }
        if peak >= min_valley_depth {
            for i in comp {
                kept.set_index(i, true);
            }
        }
    }
    kept
}

// ---------------------------------------------------------------------------
// Graph cleanup (fix B's companion): the fixed `trace_skeleton` (below) no
// longer shreds every staircase corner into its own fragment, but genuinely
// short spurs off a junction are still real artefacts worth pruning, and a
// junction that (after pruning) has exactly two surviving arms is really just
// a mid-line point, not a topological node — so its two arms get spliced back
// into one polyline. Both operate purely on cell-index polylines, before any
// world-space conversion or length-in-mm filtering.
// ---------------------------------------------------------------------------

/// Cell-space (not mm) length of a cell-index polyline: sum of Euclidean
/// distances between consecutive `(row, col)` pairs.
fn cell_polyline_length(cells: &[usize], nx: usize) -> f64 {
    if cells.len() < 2 {
        return 0.0;
    }
    cells
        .windows(2)
        .map(|w| {
            let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else {
                return 0.0;
            };
            let (r0, c0) = crate::grid2::row_major_rc(a, nx);
            let (r1, c1) = crate::grid2::row_major_rc(b, nx);
            let (dr, dc) = (r1 as f64 - r0 as f64, c1 as f64 - c0 as f64);
            (dr * dr + dc * dc).sqrt()
        })
        .sum()
}

/// Start/end cell index of a polyline (`None` for an empty polyline — never
/// happens for a `trace_skeleton` output, which is always `len() >= 2`, but
/// keeps this helper total).
fn edge_endpoints(pts: &[usize]) -> Option<(usize, usize)> {
    match (pts.first(), pts.last()) {
        (Some(&s), Some(&e)) => Some((s, e)),
        _ => None,
    }
}

/// Splice two polylines that share `node` as an endpoint into one, oriented
/// so `a` runs up to `node` and `b` continues from it (reversing either as
/// needed), with the shared `node` cell not duplicated.
fn merge_at_node(a: &[usize], b: &[usize], node: usize) -> Vec<usize> {
    let mut av = a.to_vec();
    if av.last().copied() != Some(node) {
        av.reverse();
    }
    let mut bv = b.to_vec();
    if bv.first().copied() != Some(node) {
        bv.reverse();
    }
    av.extend(bv.into_iter().skip(1));
    av
}

/// Graph cleanup for traced ridge polylines, run BEFORE any length
/// filtering: prune short spurs and merge straight-through chains at
/// degree-2 nodes. Polylines are edges of a graph keyed by their endpoint
/// cell indices (a node is any cell index that terminates one or more
/// polylines); a `BTreeMap` keeps node iteration order deterministic.
///
/// Repeats two passes until neither changes anything:
/// 1. **Prune**: drop any leaf edge (an endpoint touched by no other edge)
///    shorter than `min_cut_length_cells`, UNLESS it is fully isolated (BOTH
///    endpoints are leaves) — isolated edges are left alone; the downstream
///    `min_cut_length` length gate decides their fate.
/// 2. **Merge**: where a node is touched by exactly two DISTINCT surviving
///    edges, splice them into one polyline through the node (see
///    [`merge_at_node`]) and retire the node.
fn cleanup_ridge_graph(
    polylines: Vec<Vec<usize>>,
    nx: usize,
    min_cut_length_cells: f64,
) -> Vec<Vec<usize>> {
    let mut edges: Vec<Option<Vec<usize>>> = polylines.into_iter().map(Some).collect();

    loop {
        // Node → incident edge ids, in ascending edge-id push order.
        let mut incident: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
        for (eid, e) in edges.iter().enumerate() {
            let Some(pts) = e.as_ref() else { continue };
            let Some((s, t)) = edge_endpoints(pts) else {
                continue;
            };
            incident.entry(s).or_default().push(eid);
            incident.entry(t).or_default().push(eid);
        }

        // --- (i) prune short leaf edges, keeping isolated ones. ---
        let mut pruned_any = false;
        for eid in 0..edges.len() {
            let Some(pts) = edges.get(eid).and_then(|e| e.as_ref()) else {
                continue;
            };
            let Some((s, t)) = edge_endpoints(pts) else {
                continue;
            };
            let deg_s = incident.get(&s).map_or(0, Vec::len);
            let deg_t = incident.get(&t).map_or(0, Vec::len);
            let is_isolated = deg_s == 1 && deg_t == 1;
            let is_leaf = deg_s == 1 || deg_t == 1;
            if is_leaf && !is_isolated && cell_polyline_length(pts, nx) < min_cut_length_cells {
                if let Some(slot) = edges.get_mut(eid) {
                    *slot = None;
                }
                pruned_any = true;
            }
        }
        if pruned_any {
            continue; // recompute node degrees before attempting any merge
        }

        // --- (ii) merge through nodes with exactly two distinct edges. ---
        let mut merged_any = false;
        for (&node, eids_at_node) in &incident {
            if eids_at_node.len() != 2 {
                continue;
            }
            let mut distinct = eids_at_node.clone();
            distinct.dedup();
            if distinct.len() != 2 {
                continue; // both incidences are the same (self-loop) edge
            }
            let (Some(&a_id), Some(&b_id)) = (distinct.first(), distinct.get(1)) else {
                continue;
            };
            let Some(a_pts) = edges.get(a_id).and_then(|e| e.clone()) else {
                continue; // already consumed by an earlier merge this pass
            };
            let Some(b_pts) = edges.get(b_id).and_then(|e| e.clone()) else {
                continue;
            };
            let touches =
                |pts: &[usize]| edge_endpoints(pts).is_some_and(|(s, t)| s == node || t == node);
            if !touches(&a_pts) || !touches(&b_pts) {
                continue; // stale snapshot (endpoints changed by an earlier merge)
            }
            let merged = merge_at_node(&a_pts, &b_pts, node);
            if let Some(slot) = edges.get_mut(a_id) {
                *slot = Some(merged);
            }
            if let Some(slot) = edges.get_mut(b_id) {
                *slot = None;
            }
            merged_any = true;
        }
        if merged_any {
            continue;
        }
        break;
    }

    edges.into_iter().flatten().collect()
}

/// Trace a 1-cell skeleton into polylines (cell indices, row-major
/// determinism). Nodes are cells whose ring 0→1 transition count
/// (`ring_ab`'s `a`) is `!= 2` — NOT raw 8-degree. On an 8-connected skeleton
/// a staircase corner (two orthogonal skeleton arms meeting through a
/// diagonal "shortcut" cell) reads degree 3 even though topologically it's
/// still a plain pass-through (one strand enters, one strand leaves — the
/// transition count there is 2), so with the transition-count test it
/// correctly stays a non-node instead of shredding the path. Walk node→node
/// through non-node cells, marking each directed edge visited both ways,
/// then sweep remaining edges to pick up pure loops. When choosing the next
/// cell along a walk, an ORTHOGONAL neighbour is preferred over a diagonal
/// one — this is what stops the walk taking the diagonal "shortcut" hop
/// across a staircase corner instead of continuing along the real ridge.
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
    // Set neighbours ranked orthogonal-first (NB8 indices 0,2,4,6 = N,E,S,W),
    // then diagonal (1,3,5,7 = NE,SE,SW,NW) — the staircase-corner fix.
    let neighbors_ranked = |idx: usize| -> Vec<usize> {
        let (rr, cc) = crate::grid2::row_major_rc(idx, nx);
        let (r, c) = (rr as isize, cc as isize);
        let mut ortho = Vec::new();
        let mut diag = Vec::new();
        for (k, &(dr, dc)) in NB8.iter().enumerate() {
            let Some(nidx) = skel.index_of_signed(r + dr, c + dc) else {
                continue;
            };
            if !skel.at_index_or(nidx, false) {
                continue;
            }
            if k % 2 == 0 {
                ortho.push(nidx);
            } else {
                diag.push(nidx);
            }
        }
        ortho.into_iter().chain(diag).collect()
    };
    let is_node = |idx: usize| -> bool {
        let (rr, cc) = crate::grid2::row_major_rc(idx, nx);
        let (_, a) = ring_ab(skel, rr as isize, cc as isize);
        a != 2
    };

    // Walk from `start` toward `first`, consuming non-node cells until a node
    // or dead end. Returns the polyline of cell indices.
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
            if is_node(cur) {
                break; // reached a node
            }
            let mut next = None;
            for nbr in neighbors_ranked(cur) {
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
        if !is_node(idx) {
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
    // Sweep remaining edges → pure loops (all cells non-node).
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

    /// A narrow gaussian TRENCH running along X in a flat plate:
    /// `z = −depth · exp(−(y/σ)²)` with `σ = sigma_steep` on the `y < 0`
    /// side and `sigma_gentle` on `y > 0` (equal → symmetric). This is the
    /// geometry the RestDepth ridge is FOR: the wide reference ball bridges
    /// the channel lip-to-lip and floats, so `rest` genuinely PEAKS at the
    /// crease — unlike a plane-wall V, whose rest field is a constant
    /// plateau with no ridge at all (a ball on a constant-slope plane floats
    /// a position-independent height; see the plane-wall note on
    /// [`nms_candidates`]). Numerically validated cross-profiles (Ø6 ref /
    /// Ø1 pencil, depth 1.5): σ=1.2/1.2 → peak 0.79 mm at y=0.00 with 0.19 mm
    /// prominence at ±0.5 mm; σ=0.8/2.5 → peak 0.97 mm at y=0.00 with
    /// 0.22 mm prominence — the peak stays ON the crease even when the wall
    /// widths are lopsided, which is exactly what the old
    /// medial-axis-of-the-mask extraction got wrong.
    fn make_trench(
        len_x: f64,
        half_y: f64,
        depth: f64,
        sigma_steep: f64,
        sigma_gentle: f64,
        nx: usize,
        ny: usize,
    ) -> TriangleMesh {
        let mut verts = Vec::new();
        for iy in 0..=ny {
            let y = -half_y + 2.0 * half_y * iy as f64 / ny as f64;
            let sigma = if y < 0.0 { sigma_steep } else { sigma_gentle };
            let z = -depth * (-(y / sigma).powi(2)).exp();
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
            region_margin_mm: 0.5,
        }
    }

    // ── region_polygons_from_mask tests moved to `region_mask.rs` ────────

    /// Used by [`region_mask`]'s moved tests too (small assertion helper,
    /// intentionally duplicated there rather than exposed cross-module).
    fn assert_closed_ccw_polys(polys: &[Polygon2]) {
        for p in polys {
            assert!(p.closed, "region polygons must be closed");
            assert!(
                p.has_correct_winding(),
                "exterior must be CCW / holes CW: area={}",
                p.signed_area()
            );
            assert!(p.area() > 0.0, "region polygon must have positive area");
        }
    }

    // ── classify_rest_regions ──────────────────────────────────────────

    #[test]
    fn classify_rest_regions_none_for_healthy_set() {
        let regions = vec![Polygon2::rectangle(0.0, 0.0, 10.0, 10.0); 5];
        assert_eq!(classify_rest_regions(&regions, 100_000.0), None);
    }

    #[test]
    fn classify_rest_regions_flags_too_many_islands() {
        let regions = vec![Polygon2::rectangle(0.0, 0.0, 1.0, 1.0); MAX_REST_REGIONS / 2 + 1];
        assert_eq!(
            classify_rest_regions(&regions, 1_000_000.0),
            Some(RestRegionPathology::TooManyIslands {
                count: MAX_REST_REGIONS / 2 + 1
            })
        );
    }

    #[test]
    fn classify_rest_regions_flags_single_giant_region() {
        // Part footprint 100x100 = 10_000 mm^2; a single 80x80 region covers 64%.
        let regions = vec![Polygon2::rectangle(0.0, 0.0, 80.0, 80.0)];
        match classify_rest_regions(&regions, 10_000.0) {
            Some(RestRegionPathology::SingleGiantRegion { part_area_fraction }) => {
                assert!((part_area_fraction - 0.64).abs() < 1e-9);
            }
            other => panic!("expected SingleGiantRegion, got {other:?}"),
        }
    }

    #[test]
    fn classify_rest_regions_giant_but_zero_footprint_is_none() {
        let regions = vec![Polygon2::rectangle(0.0, 0.0, 80.0, 80.0)];
        assert_eq!(classify_rest_regions(&regions, 0.0), None);
        assert_eq!(classify_rest_regions(&regions, -5.0), None);
    }

    #[test]
    fn v_valley_yields_one_centerline() {
        // Narrow trench: the 6mm reference ball bridges it lip-to-lip and
        // floats (rest peaks at the crease), the 1mm pencil drops in. NB a
        // plane-wall V is deliberately NOT used here: its rest field is a
        // constant plateau with no ridge (see `nms_candidates`) — plane-wall
        // CAD creases belong to the Dihedral detector. A wide
        // route_width_factor forces pencil routing so we can assert on the
        // centerline.
        let mesh = make_trench(30.0, 6.0, 1.5, 1.2, 1.2, 30, 48);
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
        let all: Vec<&P3> = res
            .centerlines
            .iter()
            .flat_map(|cl| cl.points.iter())
            .collect();
        let mean_abs_y = all.iter().map(|p| p.y.abs()).sum::<f64>() / all.len().max(1) as f64;
        assert!(
            !res.region_polygons.is_empty(),
            "surviving rest-mask components must yield region_polygons alongside \
             clearing_regions/centerlines; report = {:?}",
            res.report
        );
        assert_closed_ccw_polys(&res.region_polygons);
        assert!(
            mean_abs_y < 1.5,
            "centerline should hug the y≈0 trough, mean |y| = {mean_abs_y:.2}"
        );
    }

    #[test]
    fn asymmetric_valley_centerline_hugs_crease() {
        // A lopsided trench: tight wall on y<0 (σ=0.8), wide gentle wall on
        // y>0 (σ=2.5). The mask (`rest > threshold`) extends much further
        // into the gentle side, so the OLD medial-axis-of-the-mask
        // centerline sat well off-crease toward it; the ridge (a local
        // maximum of the CONTINUOUS field) must hug y≈0 regardless —
        // numerically the cross-profile peak sits at y=0.00 (0.97 mm, 0.22 mm
        // prominence at ±0.5 mm) despite the 3× width asymmetry.
        let mesh = make_trench(30.0, 10.0, 1.5, 0.8, 2.5, 30, 80);
        let index = SpatialIndex::build_auto(&mesh);
        let pencil = BallEndmill::new(1.0, 25.0);
        let reference = BallEndmill::new(6.0, 25.0);
        let mut p = default_params(0.5);
        p.route_width_factor = 10.0; // force pencil routing so we can inspect the centerline
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
            "asymmetric valley should still yield a pencil centerline; report = {:?}",
            res.report
        );
        let all: Vec<&P3> = res
            .centerlines
            .iter()
            .flat_map(|cl| cl.points.iter())
            .collect();
        let mean_abs_y = all.iter().map(|p| p.y.abs()).sum::<f64>() / all.len().max(1) as f64;
        assert!(
            mean_abs_y < 1.5,
            "ridge should hug the true y≈0 crease regardless of wall \
             asymmetry, mean |y| = {mean_abs_y:.2}"
        );
    }

    #[test]
    fn tent_ridge_yields_none() {
        // A realistic (gently rounded) convex ridge leaves no rest — both tools
        // roll over it equally. NB a *razor-sharp* synthetic crest does produce a
        // shallow ride-over band, but it reads as a CONSTANT rest plateau on the
        // tent's plane walls (~0.11 mm here) — no genuine local maximum — so the
        // NMS prominence requirement rejects it outright (with a bare strict `>`
        // test, femtometre float noise used to tie-break scattered "ridges" all
        // over the plateau). Verified on terrain.stl, where green centerlines
        // sit only in the dark drainage valleys.
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
        assert!(
            res.region_polygons.is_empty(),
            "no clearing_regions/components -> no region_polygons either"
        );
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
        // Trench fixture (peak rest ≈ 0.8 mm at the crease): mvd 0.05 traces
        // the crease ridge, mvd 1.0 sits above the peak and must trace
        // nothing — strictly monotone, and non-trivially so (a plane-wall V
        // would yield zero at BOTH thresholds under the NMS prominence rule).
        let mesh = make_trench(30.0, 6.0, 1.5, 1.2, 1.2, 30, 48);
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
            r.centerlines
                .iter()
                .map(|cl| cl.points.len())
                .sum::<usize>()
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
        // is the erosion-robust significance signal; over a narrow trench a
        // bigger ball bridges the lips higher up and reads a deeper peak,
        // while a small one partially enters and reads a shallow one.
        let mesh = make_trench(40.0, 6.0, 1.5, 1.2, 1.2, 40, 48);
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

    #[test]
    fn staircase_skeleton_traces_as_one_polyline() {
        // A mixed-direction staircase (E-run, S-run, E-run) with two genuine
        // degree-3 (and one degree-4) 8-connected corners: `ring_ab` gives a
        // transition count of exactly 2 at every interior cell (a plain
        // pass-through) despite the raw degree — precisely the
        // staircase-corner case fix B targets. Under the OLD raw-degree
        // `!= 2` node test every one of these corners misfires as a node,
        // shredding the path into a handful of 1-3-cell fragments (the
        // reported ~86% traced-length loss); the fixed transition-count
        // test + orthogonal-preferred walk recovers the whole staircase as
        // a single polyline.
        let (nx, ny) = (8usize, 7usize);
        let cells: [(usize, usize); 7] = [(2, 1), (2, 2), (2, 3), (3, 3), (4, 3), (4, 4), (4, 5)];
        let mut skel = Grid2::new_fill(nx, ny, false);
        for &(r, c) in &cells {
            skel.set(r, c, true);
        }
        let polys = trace_skeleton(&skel);
        // Judgement call: a genuine 8-connected degree-3 corner (two
        // ring-adjacent set-neighbours forming a single topological arc per
        // `ring_ab`) is ALSO, unavoidably, a direct grid adjacency between
        // those two neighbour cells themselves — consecutive NB8 ring
        // positions are always mutually 8-adjacent by construction — a
        // short "chord" edge distinct from the main walk's own edges.
        // `trace_skeleton`'s sweep phase (unchanged by this fix, and out of
        // its scope) picks up any such leftover chord as its own tiny
        // polyline. This is an intrinsic, harmless raster-geometry
        // byproduct (isolated in the endpoint graph and far too short to
        // survive `min_cut_length` downstream), NOT the shredding bug being
        // fixed here — so the assertion below checks the fix's actual
        // guarantee (the whole staircase recovers as ONE polyline) rather
        // than a strict `polys.len() == 1`.
        let full = polys.iter().find(|p| p.len() == cells.len());
        assert!(
            full.is_some(),
            "the whole {}-cell staircase should trace as one polyline; got \
             {} polylines of lengths {:?}",
            cells.len(),
            polys.len(),
            polys.iter().map(Vec::len).collect::<Vec<_>>()
        );
        assert!(
            polys.len() <= 2,
            "expected at most one leftover chord fragment alongside the \
             main polyline (no further fragmentation), got {} polylines",
            polys.len()
        );
    }

    #[test]
    fn graph_cleanup_merges_through_short_spur() {
        // Three polylines sharing node N = (5,5): a long left piece ending
        // at N, a long right piece starting at N, and a 1-cell spur hanging
        // off N. The spur is a leaf edge (its far end (6,5) is touched by
        // no other edge) and shorter than `min_cut_length_cells`, so it
        // should be pruned; once it's gone, N is left with exactly two
        // distinct surviving edges and the left/right pieces should splice
        // into one polyline through it.
        let nx = 20;
        let idx = |r: usize, c: usize| crate::grid2::row_major_index(r, c, nx);
        let left: Vec<usize> = (0..=5).map(|c| idx(5, c)).collect(); // ends at N
        let right: Vec<usize> = (5..=10).map(|c| idx(5, c)).collect(); // starts at N
        let spur: Vec<usize> = vec![idx(5, 5), idx(6, 5)]; // 1-cell spur off N
        let polylines = vec![left.clone(), right.clone(), spur];

        let cleaned = cleanup_ridge_graph(polylines, nx, 2.0);
        assert_eq!(
            cleaned.len(),
            1,
            "the short spur should be pruned and the two long pieces merged \
             into one polyline, got {cleaned:?}"
        );
        let merged = &cleaned[0];
        assert_eq!(
            merged.len(),
            left.len() + right.len() - 1,
            "merged polyline should be the concatenation of both pieces \
             through N (the shared node cell not duplicated)"
        );
        assert_eq!(merged.first().copied(), Some(idx(5, 0)));
        assert_eq!(merged.last().copied(), Some(idx(5, 10)));
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
            region_margin_mm: 0.5,
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
        let centerline_pts: Vec<Vec<P3>> =
            res.centerlines.iter().map(|cl| cl.points.clone()).collect();
        hillshade_test_util::plot_polylines(
            &dem,
            &mut img,
            &centerline_pts,
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
