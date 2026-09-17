//! **Track H phase V1 — the branch-tracing falsifier.** Does per-branch
//! bidirectional offset tracing beat a sweep inside valley territory?
//!
//! Pre-registration: `planning/valley_tracing_2026-09-02/FINDINGS.md`, §Q2 and
//! §"V1 — OPEN" (commits `11ac62d5`, `5013ad70`). This file measures; it
//! writes NO verdict. B3 / B4 / B5, the margin rule and the spec-meeting rule
//! are applied by the orchestrator.
//!
//! Upstream: `tests/valley_prize_census_h0.rs` (V0). Helpers are COPIED from
//! it, not shared — an integration test cannot `use` another one, and the
//! census must not move when this file changes.
//!
//! # The three arms
//!
//! * **S** — the shipped honest raster: C2 lattice, region derate exactly as
//!   shipped (`cos theta_max` over the whole region polygon).
//! * **S'** — the same raster with the derate recomputed after the
//!   overlap-dilation fringe is excised. **This is the control B5 binds
//!   against**: arm T's offsets locally run near the spec spacing on a valley
//!   floor while S is pinned at the region's derated pitch, so without S' a
//!   win could be spacing that was granted rather than tracing that was
//!   earned.
//! * **T** — the HYBRID: per-branch bidirectional offset tracing inside
//!   `mask B ∩ region`, and the S' raster outside it. Accounting is
//!   whole-arm, seams included — the seam cost is real and belongs to the
//!   candidate.
//!
//! # Conventions stated BEFORE the run
//!
//! ## Two secants, and they are different
//!
//! Both come from the classification heightfield's upward unit normal,
//! `grad z = (-n_x/n_z, -n_y/n_z)`:
//!
//! ```text
//!   ALONG-track (V0-att v2, the raster's path-length magnification):
//!       tan phi = |grad z . d|     d = the pass direction
//!
//!   CROSS-track (V1, arm T's pitch derate):
//!       tan psi = |grad z . p|     p = perpendicular to the pass, p ⟂ d
//! ```
//!
//! Arm T's **XY offset pitch derates PER BRANCH**:
//! `pitch = stepover × cos(psi_max)` where `psi_max` is the largest
//! cross-track slope angle along that branch's pass — the branch-local
//! worst-point rule, which is what a shipped tracing op would do.
//!
//! **Arms S and S' keep the SHIPPED derate**, `cos theta_max` on the FULL
//! slope, not the cross-track one. Switching them would be a fourth arm the
//! pre-registration did not order.
//!
//! ## The S' fringe rule (unambiguous — no fallback was needed)
//!
//! `region_polygons_from_mask_clamped` dilates the band mask by
//! `overlap_mm` = 2.0 mm before marching squares, so the region polygon lies
//! ~2 mm outside the true band and drags a fringe of neighbouring-band cells
//! in. The fringe is therefore identified as **the covered in-region cells
//! whose chamfer-DT distance to the region boundary is under `overlap_mm`**,
//! and `theta_max` is recomputed with them excised. That is the documented
//! dilation reversed, using the DT convention this programme already uses,
//! so the pre-registered "falls back to the undilated region polygon if
//! ambiguous" branch was NOT taken. The output says so.
//!
//! The pre-registration also permits "the mask as its own region" as the S'
//! derate. Both are computed and printed; if fringe excision does not move
//! `theta_max` the mask-derived one is run as well, so B5 always has a
//! control that is actually different from S.
//!
//! ## Branch pruning
//!
//! Branch centerlines are the flow network at the picked rung
//! (`T = 8 mm²`), traced head-to-outlet. A branch is KEPT when its polyline
//! length is at least `V1_BRANCH_MIN_STEPOVERS` × stepover
//! (3 × 0.486210 = 1.4586 mm), so stubs cannot fragment arm T for free.
//! Dropped branches are counted, and the territory they would have covered
//! is left to the raster part of the hybrid — it is not silently unmachined.
//!
//! ## The coverage audit — a RESTATEMENT, and why
//!
//! `conformal_spiral::CoverageAudit` is the pre-registered vehicle and this
//! file fills that exact public struct. The function that populates it in
//! production (`conformal_spiral::audit_coverage`) is **private and coupled
//! to the spiral planner's own types** (`RegionMesh`, `Flattening`,
//! `CentreCurve`), so it cannot be called on an arbitrary toolpath. The
//! convention is restated verbatim instead:
//!
//! * **population** — every region triangle's centroid, lifted `+h` along the
//!   averaged vertex normal to `S^h`. Region triangles are the mesh triangles
//!   whose centroid lies in the region polygon in XY.
//! * **test** — is that lifted point within the true cusp radius `K_c` of the
//!   arm's **tool-centre curve**? `CLPoint` is the tool TIP
//!   (`tool/mod.rs:27-28`), so the ball-centre curve is the emitted
//!   cutting-intent polyline raised by `K_c` in Z.
//! * **radius** — `cusp_radius_mm()` = 1.0, the TRUE tool, never
//!   `envelope_radius_mm()` = 3.0 and never a derated criterion. That mirrors
//!   the shipped audit's own comment.
//!
//! One caveat, stated and then dismissed: on a tapered ball the flank takes
//! over from the tip sphere past roughly `90 - taper` degrees of wall. The
//! band here is clamped at 45 degrees, so the tip sphere is the contacting
//! geometry everywhere in this population and the ball model is exact.
//!
//! **Coverage equality is a PRECONDITION, not a footnote.** An arm whose
//! `unmachined_area_fraction` exceeds [`COVERAGE_FAIL_FRACTION`] is marked
//! FAILS COVERAGE and cannot win, whatever its time.
//!
//! ## Spec-meeting
//!
//! Every arm reports its achieved surface spacing distribution and its
//! **exceed %** — the fraction of achieved-spacing samples above
//! `1.05 × spec`, where spec is the equal-cusp stepover 0.486210 mm for
//! EVERY arm, never the arm's own derated pitch. An arm above
//! [`SPACING_EXCEED_FAIL_PCT`] is marked FAILS SPEC.
//!
//! # Running it
//!
//! ```text
//! cargo test -p rs_cam_core --features research --test valley_branch_falsifier_h1 \
//!   wanaka_valley_branch_falsifier_h1 -- --ignored --nocapture
//! ```
//!
//! `#[ignore]` — it needs the operator's wanaka mesh, which is not in the
//! repo. It SKIPS rather than fails when the mesh is absent, and it never
//! substitutes `fixtures/terrain_small.stl` (banned by the pre-registration).

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
use rs_cam_core::finish::classify_probe::ClassificationSampler;
use rs_cam_core::finish::conformal_spiral::CoverageAudit;
use rs_cam_core::finish::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use rs_cam_core::finish::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::finish::unified_finish::unified_finish_classification_resolution;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::geometry::contour_extract::marching_squares_bool_grid;
use rs_cam_core::geometry::grid_field::distance_transform_2d;
use rs_cam_core::geometry::monotone_cells::{lattice_monotone_cells, region_frame};
use rs_cam_core::maps::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::maps::tier_map::{ResidualTreatment, TierLadder, TierMapParams, compute_tier_map};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::{CLPoint, MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

// ── fixtures ────────────────────────────────────────────────────────────

const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

// ── dials, from `planning/multitool_2026-08-23/wanaka200_mt2.toml` ──────
//
// Copied verbatim from `tests/valley_prize_census_h0.rs`, which reads the
// same file. Restated rather than imported: an integration test cannot `use`
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

const MAX_COSTED_REGIONS: usize = 16;

// ── V0 mask conventions, carried forward unchanged ──────────────────────

const TPI_HALF_WINDOW_MM: f64 = 30.0;
const LAND_Z_MM: f64 = 0.0;
const BUFFER_REDUNDANCY_FRACTION: f64 = 0.25;
/// The rung the V0 stability rule picked, and the one V1 is pre-registered
/// against.
const ACC_THRESHOLD_MM2: f64 = 8.0;

// ── V1 dials, declared before the run ───────────────────────────────────

/// A branch shorter than this many stepovers is pruned, so stubs cannot
/// fragment arm T for free. Its ground falls to the raster part of the
/// hybrid, never to nothing.
const V1_BRANCH_MIN_STEPOVERS: f64 = 3.0;
/// Arc step (mm) the branch centerlines are resampled at before offsetting.
const V1_BRANCH_SAMPLE_MM: f64 = 0.25;
/// `unmachined_area_fraction` above which an arm is marked FAILS COVERAGE.
/// A raster at a 0.344 mm pitch under a 2 mm overlap fringe has no business
/// leaving 1 % of a region uncut; the number is a gate, not a tolerance.
const COVERAGE_FAIL_FRACTION: f64 = 0.01;
/// Achieved spacing above `SPACING_EXCEED_RATIO × spec` would count as an
/// exceed, and an arm above `SPACING_EXCEED_FAIL_PCT` would be marked FAILS
/// SPEC. **Both are declared and NEITHER is consumed**: the measurement that
/// would feed them is blocked (see the run report). They are left here so the
/// gate's thresholds are on the record at the same commit as the blocker,
/// not invented later.
#[allow(dead_code)]
const SPACING_EXCEED_RATIO: f64 = 1.05;
#[allow(dead_code)]
const SPACING_EXCEED_FAIL_PCT: f64 = 5.0;

/// `s = 2·sqrt(2Rh − h²)` — the equal-cusp law, restated so the instrument
/// shows its own arithmetic.
fn equal_cusp_stepover_mm(cusp_radius_mm: f64, h: f64) -> f64 {
    if h <= 0.0 || h >= cusp_radius_mm {
        return 0.0;
    }
    2.0 * (2.0 * cusp_radius_mm * h - h * h).sqrt()
}

// ═══════════════════════════════════════════════════════════════════════
// Part 0 — the rasterised heightfield and its hydrology (copied from V0)
// ═══════════════════════════════════════════════════════════════════════

/// The rasterised heightfield and its D8 hydrology now live in
/// `rs_cam_core::surface::flow_accum` (promoted from three verbatim copies for the
/// pencil watershed-spine experiment). This census keeps its file-local
/// helpers as an extension trait so the call sites below do not change.
use rs_cam_core::surface::flow_accum::{
    FlowField as Field, d8_accumulation, d8_receivers, priority_flood_epsilon,
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

    /// The LAND view — everything at or below `land_z` marked nodata. The
    /// board carries a raised outer edge band, so a whole-board flood drowns
    /// the interior; V0 measured 99.50 % of cells raised. See the header.
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

/// Two-pass chamfer DT: for each set cell, distance in CELLS to the nearest
/// unset cell — the `rest_field.rs` convention.
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

/// The low-ground mask (topographic position index) via a summed-area table.
fn low_ground_mask(field: &Field, half_window_mm: f64) -> Vec<bool> {
    let nx = field.nx;
    let ny = field.ny;
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

/// Union of discs of the local half-width around every sample.
fn buffer_samples(field: &Field, dt_cells: &[f64], samples: &[P2]) -> Vec<bool> {
    let mut inside = vec![false; field.len()];
    let mut cover_dist = vec![f64::INFINITY; field.len()];
    let mut cover_radius = vec![0.0f64; field.len()];
    for sample in samples {
        let Some(centre) = field.index_at(sample.x, sample.y) else {
            continue;
        };
        let radius_cells = dt_cells[centre].max(1.0);
        if cover_radius[centre] >= radius_cells
            && cover_dist[centre] <= BUFFER_REDUNDANCY_FRACTION * radius_cells
        {
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
    inside
}

/// Trace the network head-to-outlet into polylines, with the cell index of
/// every point kept beside it (the DT half-width lookup needs it).
fn valley_polylines(
    field: &Field,
    receivers: &[Option<u32>],
    acc: &[f64],
    network: &[bool],
) -> Vec<Vec<usize>> {
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
            line.push(c);
            match receivers[c] {
                Some(r) if network[r as usize] && !visited[r as usize] => c = r as usize,
                Some(r) if network[r as usize] => {
                    line.push(r as usize);
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

// ═══════════════════════════════════════════════════════════════════════
// Part 1 — costing (copied from the census / thin_organic §0i regime)
// ═══════════════════════════════════════════════════════════════════════

struct CandidateCost {
    cutting_mm: f64,
    time_s: f64,
    fragments: usize,
    linked: usize,
    kept_retracts: usize,
    toolpath: Toolpath,
}

struct LinkRegime<'a> {
    safe_z: f64,
    ceiling: Option<rs_cam_core::finish::surface_link::LinkCeiling<'a>>,
    flush_ride: bool,
    airborne: bool,
}

fn relink_and_cost_under(
    raw: Toolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    boundary: &rs_cam_core::geometry::region_set::RegionSet<'_>,
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
    regime: &LinkRegime<'_>,
) -> CandidateCost {
    use rs_cam_core::machine::kinematics::{LinkKinematics, compute_cycle_time};

    let link_kinematics = LinkKinematics {
        kinematics: *kinematics,
        max_feed_mm_min: MAX_FEED_MM_MIN,
        rapid_feed_mm_min: RAPID_FEED_MM_MIN,
    };
    let params = rs_cam_core::finish::surface_link::RelinkParams {
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
    let (linked, report) = rs_cam_core::finish::surface_link::relink_fragments(
        rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(raw),
        mesh,
        index,
        cutter,
        &params,
    );
    let mut channels = rs_cam_core::trace::transform_provenance::ReconcileSet::new(None, None);
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
) -> Toolpath {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::toolpath::raster_toolpath_from_grid;

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
/// tier-0 R1.5 where tier 0 machines. Restated from `a3_machined_stock`.
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
    let distance_to_tier_zero =
        distance_transform_2d(&tier_zero, tier_map.grid.ny, tier_map.grid.nx);
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
                distance_to_tier_zero[r * tier_map.grid.nx + c] * tier_map.grid.cell_mm <= reach
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
// Part 2 — theta_max, restated with a keep filter (the S / S' derates)
// ═══════════════════════════════════════════════════════════════════════

/// `shallow_region_max_slope_deg` (`unified_finish.rs:2813`), restated with
/// one extra filter. Both coverage guards and the steep clamp are verbatim.
fn region_max_slope_deg(
    surface: &rs_cam_core::finish::finish_setup::FinishSurface,
    covered: &[bool],
    polygon: &Polygon2,
    clamp_deg: f64,
    keep: &dyn Fn(usize) -> bool,
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
        if !polygon.contains_point(&P2::new(px, py)) || !keep(i) {
            continue;
        }
        population += 1;
        max_rad = max_rad.max(angle);
    }
    (max_rad.to_degrees().min(clamp_deg), population)
}

// ═══════════════════════════════════════════════════════════════════════
// Part 3 — the coverage audit (a restatement; see the file header)
// ═══════════════════════════════════════════════════════════════════════

/// A coarse XY bucket index over the arm's tool-centre segments, so the
/// audit is O(centroids × segments-per-bucket) rather than quadratic.
struct SegmentIndex {
    ox: f64,
    oy: f64,
    cell: f64,
    nx: usize,
    ny: usize,
    buckets: Vec<Vec<u32>>,
    segments: Vec<(P3, P3)>,
}

impl SegmentIndex {
    /// Build over the CUTTING-INTENT moves only, raised by `lift` in Z so the
    /// polyline becomes the ball-CENTRE curve (`CLPoint` is the tool tip).
    fn build(toolpath: &Toolpath, lift: f64, cell: f64, bbox: [f64; 4]) -> Self {
        let [x0, y0, x1, y1] = bbox;
        let nx = (((x1 - x0) / cell).ceil() as usize).max(1) + 2;
        let ny = (((y1 - y0) / cell).ceil() as usize).max(1) + 2;
        let mut me = Self {
            ox: x0 - cell,
            oy: y0 - cell,
            cell,
            nx,
            ny,
            buckets: vec![Vec::new(); nx * ny],
            segments: Vec::new(),
        };
        for i in 1..toolpath.moves.len() {
            let m = &toolpath.moves[i];
            if matches!(m.move_type, MoveType::Rapid) {
                continue;
            }
            if !matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut) {
                continue;
            }
            let a = toolpath.moves[i - 1].target;
            let b = m.target;
            let seg = (P3::new(a.x, a.y, a.z + lift), P3::new(b.x, b.y, b.z + lift));
            let id = me.segments.len() as u32;
            me.segments.push(seg);
            // Stamp every bucket the segment's XY bbox touches.
            let (sx0, sx1) = (a.x.min(b.x), a.x.max(b.x));
            let (sy0, sy1) = (a.y.min(b.y), a.y.max(b.y));
            let c0 = (((sx0 - me.ox) / cell).floor().max(0.0) as usize).min(nx - 1);
            let c1 = (((sx1 - me.ox) / cell).ceil() as usize).min(nx - 1);
            let r0 = (((sy0 - me.oy) / cell).floor().max(0.0) as usize).min(ny - 1);
            let r1 = (((sy1 - me.oy) / cell).ceil() as usize).min(ny - 1);
            for r in r0..=r1 {
                for c in c0..=c1 {
                    me.buckets[r * nx + c].push(id);
                }
            }
        }
        me
    }

    /// Squared distance from `p` to the nearest indexed segment, searching
    /// the bucket under `p` and its 8 neighbours — sound because `cell` is
    /// chosen larger than the query radius.
    fn nearest_sq(&self, p: P3) -> f64 {
        let c = ((p.x - self.ox) / self.cell).floor();
        let r = ((p.y - self.oy) / self.cell).floor();
        let mut best = f64::INFINITY;
        for dr in -1isize..=1 {
            for dc in -1isize..=1 {
                let rr = r as isize + dr;
                let cc = c as isize + dc;
                if rr < 0 || cc < 0 || rr >= self.ny as isize || cc >= self.nx as isize {
                    continue;
                }
                for &id in &self.buckets[rr as usize * self.nx + cc as usize] {
                    let (a, b) = self.segments[id as usize];
                    best = best.min(point_segment_dist_sq(p, a, b));
                }
            }
        }
        best
    }
}

fn point_segment_dist_sq(p: P3, a: P3, b: P3) -> f64 {
    let ab = b - a;
    let denom = ab.norm_squared();
    let t = if denom <= 1e-18 {
        0.0
    } else {
        ((p - a).dot(&ab) / denom).clamp(0.0, 1.0)
    };
    (p - (a + ab * t)).norm_squared()
}

/// Distance statistics in the shape the shipped audit reports.
fn summarise(values: &mut [f64]) -> rs_cam_core::finish::conformal_spiral::DistanceStats {
    values.sort_by(f64::total_cmp);
    let pick = |q: f64| -> f64 {
        if values.is_empty() {
            0.0
        } else {
            values[((values.len() - 1) as f64 * q).round() as usize]
        }
    };
    rs_cam_core::finish::conformal_spiral::DistanceStats {
        samples: values.len(),
        min_mm: values.first().copied().unwrap_or(0.0),
        median_mm: pick(0.5),
        max_mm: values.last().copied().unwrap_or(0.0),
    }
}

/// The region's triangles: index, centroid lifted to `S^h`, and 3D area.
struct RegionTriangles {
    lifted: Vec<P3>,
    areas: Vec<f64>,
    area_mm2: f64,
    /// Slope angle (deg) of each triangle, so the coverage result can be
    /// ATTRIBUTED rather than asserted.
    slopes_deg: Vec<f64>,
}

/// 3D area by slope band, and the share at or above `clamp_deg`.
///
/// **Why this is printed beside the coverage audit.** A raster at XY pitch
/// `p` achieves surface spacing `p / cos theta`. The spec scallop is met only
/// while `p / cos theta <= s_max`, i.e. while `theta <= acos(p / s_max)` —
/// and with the shipped derate `p = s_max · cos(theta_max)` that bound is
/// exactly `theta_max`. Since `theta_max` is CLAMPED at the planner's
/// `steep_threshold_deg`, every square millimetre steeper than the clamp is
/// under-covered at spec scallop BY CONSTRUCTION, for every raster arm. This
/// table says how much of the region that is, so the audit's fraction can be
/// checked against it instead of taken on trust.
fn print_slope_census(tris: &RegionTriangles, clamp_deg: f64) {
    const BANDS: [f64; 7] = [0.0, 15.0, 30.0, 45.0, 60.0, 75.0, 90.0001];
    let mut band_area = [0.0f64; 6];
    let mut above_clamp = 0.0f64;
    for (i, &deg) in tris.slopes_deg.iter().enumerate() {
        let a = tris.areas[i];
        for b in 0..6 {
            if deg >= BANDS[b] && deg < BANDS[b + 1] {
                band_area[b] += a;
                break;
            }
        }
        if deg > clamp_deg {
            above_clamp += a;
        }
    }
    eprintln!(
        "\n---------- region 3D area by SLOPE (the coverage result's attribution) ----------\n\
         \x20  A raster at XY pitch p achieves surface spacing p/cos(theta), so the spec\n\
         \x20  scallop is met only up to theta = acos(p/s_max) — which under the shipped\n\
         \x20  derate IS theta_max, and theta_max is CLAMPED at {clamp_deg:.1} deg. Every mm²\n\
         \x20  steeper than the clamp is under-covered at spec scallop by construction, in\n\
         \x20  EVERY raster arm."
    );
    for b in 0..6 {
        eprintln!(
            "     {:>5.0}–{:<5.0} deg  {:>10.1} mm²  {:>7.2} %",
            BANDS[b],
            BANDS[b + 1].min(90.0),
            band_area[b],
            100.0 * band_area[b] / tris.area_mm2.max(1e-9)
        );
    }
    eprintln!(
        "     3D area STEEPER than the {clamp_deg:.1} deg clamp: {:.1} mm² = {:.2} % of the \
         region's {:.1} mm²",
        above_clamp,
        100.0 * above_clamp / tris.area_mm2.max(1e-9),
        tris.area_mm2
    );
}

fn region_triangles(mesh: &TriangleMesh, polygon: &Polygon2, scallop_h_mm: f64) -> RegionTriangles {
    let [bx0, by0, bx1, by1] = polygon.bbox();
    let rows: Vec<(P3, f64, f64)> = (0..mesh.triangles.len())
        .into_par_iter()
        .filter_map(|t| {
            let tri = mesh.triangles[t];
            let p0 = mesh.vertices[tri[0] as usize];
            let p1 = mesh.vertices[tri[1] as usize];
            let p2 = mesh.vertices[tri[2] as usize];
            let cx = (p0.x + p1.x + p2.x) / 3.0;
            let cy = (p0.y + p1.y + p2.y) / 3.0;
            if cx < bx0 || cx > bx1 || cy < by0 || cy > by1 {
                return None;
            }
            if !polygon.contains_point(&P2::new(cx, cy)) {
                return None;
            }
            let cross = (p1 - p0).cross(&(p2 - p0));
            let area = 0.5 * cross.norm();
            if area <= 0.0 || !area.is_finite() {
                return None;
            }
            // The shipped audit lifts along the AVERAGED VERTEX normal; on a
            // per-triangle population that average is the face normal, which
            // is what this uses.
            //
            // **Oriented UPWARD explicitly.** The lift must go OUT of the
            // material: `S^h` sits between the surface and the ball centre,
            // so the lifted point is `R - h·cos(alpha)` from a covering
            // centre. Taken the other way it is `R + h·cos(alpha)`, which on
            // FLAT ground is 1.03 against a 1.00 radius — every flat triangle
            // then reads uncovered by 30 microns. The first run of this
            // instrument did exactly that and reported 47.1 % of the SHIPPED
            // raster unmachined, which is a model error, not a raster
            // defect. This mesh is a single-valued heightfield (100 %
            // up-facing, min n_z = 0.0148), so "upward" is well defined here
            // and the winding cannot be relied on instead.
            let n = cross / cross.norm();
            let n = if n.z < 0.0 { -n } else { n };
            let cz = (p0.z + p1.z + p2.z) / 3.0;
            let centroid = P3::new(cx, cy, cz);
            let slope_deg = n.z.clamp(-1.0, 1.0).acos().to_degrees();
            Some((centroid + n * scallop_h_mm, area, slope_deg))
        })
        .collect();
    let mut lifted = Vec::with_capacity(rows.len());
    let mut areas = Vec::with_capacity(rows.len());
    let mut slopes_deg = Vec::with_capacity(rows.len());
    let mut area_mm2 = 0.0;
    for (p, a, s) in rows {
        lifted.push(p);
        areas.push(a);
        slopes_deg.push(s);
        area_mm2 += a;
    }
    RegionTriangles {
        lifted,
        areas,
        area_mm2,
        slopes_deg,
    }
}

/// Fill the pre-registered [`CoverageAudit`] for one arm.
fn audit_arm_coverage(
    tris: &RegionTriangles,
    toolpath: &Toolpath,
    cusp_radius_mm: f64,
    bbox: [f64; 4],
) -> CoverageAudit {
    // Bucket edge comfortably above the query radius, so the 3x3 search is
    // exhaustive.
    let index = SegmentIndex::build(toolpath, cusp_radius_mm, 4.0 * cusp_radius_mm, bbox);
    let r2 = cusp_radius_mm * cusp_radius_mm;
    let hits: Vec<(usize, f64)> = (0..tris.lifted.len())
        .into_par_iter()
        .filter_map(|i| {
            let d2 = index.nearest_sq(tris.lifted[i]);
            (d2 > r2).then_some((i, d2.sqrt()))
        })
        .collect();
    let mut unmachined = 0.0f64;
    let mut largest = 0.0f64;
    let mut distances: Vec<f64> = Vec::with_capacity(hits.len());
    for &(i, d) in &hits {
        unmachined += tris.areas[i];
        largest = largest.max(tris.areas[i]);
        distances.push(d);
    }
    CoverageAudit {
        centroids_tested: tris.lifted.len(),
        uncovered_centroid_triangles: hits.len(),
        unmachined_area_mm2: unmachined,
        unmachined_area_fraction: if tris.area_mm2 > 0.0 {
            unmachined / tris.area_mm2
        } else {
            0.0
        },
        region_area_mm2: tris.area_mm2,
        largest_unmachined_triangle_area_mm2: largest,
        coverage_radius_mm: cusp_radius_mm,
        uncovered_distance: summarise(&mut distances),
        ..CoverageAudit::default()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Part 4 — pre-relink fragments
// ═══════════════════════════════════════════════════════════════════════

/// Fragments in a RAW (pre-relink) toolpath, counted as the number of
/// `EntryPlunge` moves. Every emitter used here — `raster_toolpath_from_grid`
/// and `Toolpath::emit_path_segment_with_intent` — opens exactly one pass
/// with exactly one `EntryPlunge`, so this counts what is IN the toolpath
/// rather than what the construction believed it built. One definition,
/// applied identically to all three arms.
fn pre_relink_fragments(toolpath: &Toolpath) -> usize {
    toolpath
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryPlunge)
        .count()
}

// ═══════════════════════════════════════════════════════════════════════
// Part 5 — cutting runs, the common currency of all three arms
// ═══════════════════════════════════════════════════════════════════════

/// The maximal contiguous CUTTING sequences of a toolpath, as point runs.
/// A run starts at the point the first cutting move departs from.
fn cut_runs(toolpath: &Toolpath) -> Vec<Vec<P3>> {
    let mut out: Vec<Vec<P3>> = Vec::new();
    let mut current: Vec<P3> = Vec::new();
    for i in 1..toolpath.moves.len() {
        let m = &toolpath.moves[i];
        let cutting = !matches!(m.move_type, MoveType::Rapid)
            && matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut);
        if cutting {
            if current.is_empty() {
                current.push(toolpath.moves[i - 1].target);
            }
            current.push(m.target);
        } else if current.len() >= 2 {
            out.push(std::mem::take(&mut current));
        } else {
            current.clear();
        }
    }
    if current.len() >= 2 {
        out.push(current);
    }
    out
}

/// Split each run into the maximal sub-runs whose points satisfy `keep`.
fn split_runs(runs: &[Vec<P3>], keep: &dyn Fn(P3) -> bool) -> Vec<Vec<P3>> {
    let mut out = Vec::new();
    for run in runs {
        let mut current: Vec<P3> = Vec::new();
        for &p in run {
            if keep(p) {
                current.push(p);
            } else if current.len() >= 2 {
                out.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
        if current.len() >= 2 {
            out.push(current);
        }
    }
    out
}

/// Rebuild a raw toolpath from point runs, through the SAME emitter the
/// raster uses, so `EntryPlunge` tagging (and therefore
/// [`pre_relink_fragments`]) is identical across arms.
fn toolpath_from_runs(runs: &[Vec<P3>], safe_z: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    for run in runs {
        if run.len() >= 2 {
            tp.emit_path_segment_with_intent(
                run,
                safe_z,
                FEED_MM_MIN,
                PLUNGE_MM_MIN,
                MoveIntent::FinishingCut,
            );
        }
    }
    tp
}

// ═══════════════════════════════════════════════════════════════════════
// Part 6 — arm T's per-branch bidirectional offset tracing
// ═══════════════════════════════════════════════════════════════════════

/// What one branch cost and why — printed so the mechanism is visible before
/// any aggregate is read.
struct BranchStat {
    length_mm: f64,
    half_width_p50_mm: f64,
    psi_max_deg: f64,
    pitch_mm: f64,
    passes: usize,
    points: usize,
}

/// Resample an XY polyline at a fixed arc step.
fn resample_xy(points: &[P2], step: f64) -> Vec<P2> {
    if points.len() < 2 || step <= 0.0 {
        return points.to_vec();
    }
    let mut out = vec![points[0]];
    let mut carry = 0.0f64;
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        if seg <= 1e-12 {
            continue;
        }
        let mut t = step - carry;
        while t < seg {
            let f = t / seg;
            out.push(P2::new(a.x + f * (b.x - a.x), a.y + f * (b.y - a.y)));
            t += step;
        }
        carry = seg - (t - step);
    }
    if let Some(&last) = points.last() {
        out.push(last);
    }
    out
}

fn polyline_length_xy(points: &[P2]) -> f64 {
    points
        .windows(2)
        .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
        .sum()
}

/// Everything the tracer reads that is not the branch itself.
struct TracerCtx<'a> {
    mesh: &'a TriangleMesh,
    index: &'a SpatialIndex,
    cutter: &'a TaperedBallEndmill,
    surface: &'a rs_cam_core::finish::finish_setup::FinishSurface,
    field: &'a Field,
    /// Chamfer DT of the low-ground mask, in CELLS.
    dt_cells: &'a [f64],
    /// `mask B ∩ region`, on the field grid.
    mask: &'a [bool],
    stepover: f64,
    min_z: f64,
}

/// The heightfield gradient at a field cell, from the classification normal.
fn gradient_at(
    surface: &rs_cam_core::finish::finish_setup::FinishSurface,
    i: usize,
) -> Option<(f64, f64)> {
    const MIN_NZ: f64 = 1e-6;
    let n = surface.slope_map.normals.get(i)?;
    if n.z.abs() < MIN_NZ {
        return None;
    }
    Some((-n.x / n.z, -n.y / n.z))
}

/// Trace ONE branch into bidirectional width-capped offset passes.
///
/// The offset is applied in XY perpendicular to the pass tangent and the Z is
/// then re-solved by the drop cutter — the C1 gouge convention, and the same
/// two-stage convention `pencil::offset_polyline_variable` + `lift_to_surface`
/// use. A point whose drop cutter finds no contact, or which leaves the mask,
/// SPLITS the pass rather than being dropped from it.
fn trace_branch(ctx: &TracerCtx<'_>, cells: &[usize]) -> Option<(Vec<Vec<P3>>, BranchStat)> {
    let raw: Vec<P2> = cells.iter().map(|&i| ctx.field.xy(i)).collect();
    let samples = resample_xy(&raw, V1_BRANCH_SAMPLE_MM);
    if samples.len() < 2 {
        return None;
    }
    let length_mm = polyline_length_xy(&samples);
    if length_mm < V1_BRANCH_MIN_STEPOVERS * ctx.stepover {
        return None;
    }

    // Per-sample tangent, perpendicular, half-width and cross-track slope.
    let n = samples.len();
    let mut perp = Vec::with_capacity(n);
    let mut half_width = Vec::with_capacity(n);
    let mut cross_tan = Vec::with_capacity(n);
    for k in 0..n {
        let a = samples[k.saturating_sub(1)];
        let b = samples[(k + 1).min(n - 1)];
        let (tx, ty) = (b.x - a.x, b.y - a.y);
        let len = tx.hypot(ty);
        let (px, py) = if len > 1e-12 {
            (-ty / len, tx / len)
        } else {
            (0.0, 1.0)
        };
        perp.push((px, py));
        let cell = ctx.field.index_at(samples[k].x, samples[k].y);
        half_width.push(cell.map_or(0.0, |i| ctx.dt_cells[i] * ctx.field.cell));
        // CROSS-track slope: tan psi = |grad z . p|, p perpendicular to the
        // pass. Distinct from V0-att v2's ALONG-track secant, which uses d.
        let tan = cell
            .and_then(|i| gradient_at(ctx.surface, i))
            .map_or(0.0, |(fx, fy)| (fx * px + fy * py).abs());
        cross_tan.push(tan);
    }

    let psi_max = cross_tan.iter().copied().fold(0.0f64, f64::max).atan();
    // The branch-local worst-point rule: one pitch per branch, sized by that
    // branch's worst cross-track slope.
    let pitch = ctx.stepover * psi_max.cos();
    if pitch <= 1e-6 || !pitch.is_finite() {
        return None;
    }
    let widest = half_width.iter().copied().fold(0.0f64, f64::max);
    let k_max = (widest / pitch).floor() as i64;

    let mut runs: Vec<Vec<P3>> = Vec::new();
    let mut points_emitted = 0usize;
    let mut passes = 0usize;
    for k in -k_max..=k_max {
        let want = (k as f64).abs() * pitch;
        let mut current: Vec<P3> = Vec::new();
        let mut opened = false;
        for idx in 0..n {
            // Width cap: this offset exists here only if the local half-width
            // reaches it.
            if want > half_width[idx] {
                if current.len() >= 2 {
                    runs.push(std::mem::take(&mut current));
                    opened = true;
                } else {
                    current.clear();
                }
                continue;
            }
            let (px, py) = perp[idx];
            let x = samples[idx].x + k as f64 * pitch * px;
            let y = samples[idx].y + k as f64 * pitch * py;
            let inside = ctx
                .field
                .index_at(x, y)
                .is_some_and(|i| ctx.mask.get(i).copied().unwrap_or(false));
            if !inside {
                if current.len() >= 2 {
                    runs.push(std::mem::take(&mut current));
                    opened = true;
                } else {
                    current.clear();
                }
                continue;
            }
            let cl: CLPoint = rs_cam_core::surface::dropcutter::point_drop_cutter(
                x, y, ctx.mesh, ctx.index, ctx.cutter,
            );
            if !cl.contacted || cl.z <= ctx.min_z {
                if current.len() >= 2 {
                    runs.push(std::mem::take(&mut current));
                    opened = true;
                } else {
                    current.clear();
                }
                continue;
            }
            current.push(P3::new(x, y, cl.z));
        }
        if current.len() >= 2 {
            runs.push(current);
            opened = true;
        }
        if opened {
            passes += 1;
        }
    }
    for run in &runs {
        points_emitted += run.len();
    }
    if runs.is_empty() {
        return None;
    }
    let mut widths: Vec<f64> = half_width.clone();
    widths.sort_by(f64::total_cmp);
    Some((
        runs,
        BranchStat {
            length_mm,
            half_width_p50_mm: widths[widths.len() / 2],
            psi_max_deg: psi_max.to_degrees(),
            pitch_mm: pitch,
            passes,
            points: points_emitted,
        },
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// Part 7 — per-move time attribution (copied from the census)
// ═══════════════════════════════════════════════════════════════════════

#[derive(Default, Clone, Copy)]
struct Split {
    in_time_s: f64,
    out_time_s: f64,
}

/// Attribute a costed toolpath's CUTTING time to inside/outside a predicate,
/// by move midpoint. Per-move time is `len / v_peak` from the public
/// `predicted_feeds_for_toolpath`, rescaled so the sum equals the production
/// `compute_cycle_time` total — the census's stated approximation, whose
/// distance cross-check agreed to 0.04 pp.
fn attribute_cutting(
    toolpath: &Toolpath,
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
    total_time_s: f64,
    inside: &dyn Fn(P2) -> bool,
) -> Split {
    use rs_cam_core::machine::kinematics::predicted_feeds_for_toolpath;

    let feeds =
        predicted_feeds_for_toolpath(toolpath, kinematics, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
    let mut raw_total = 0.0f64;
    let mut rows: Vec<(usize, f64)> = Vec::new();
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
        let t = length / (v_mm_min / 60.0).max(1e-6);
        raw_total += t;
        rows.push((i, t));
    }
    let scale = if raw_total > 0.0 {
        total_time_s / raw_total
    } else {
        1.0
    };
    let mut split = Split::default();
    for (i, t) in rows {
        let m = &toolpath.moves[i];
        if matches!(m.move_type, MoveType::Rapid) {
            continue;
        }
        if !matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut) {
            continue;
        }
        let p0 = toolpath.moves[i - 1].target;
        let p1 = m.target;
        let mid = P2::new(0.5 * (p0.x + p1.x), 0.5 * (p0.y + p1.y));
        if inside(mid) {
            split.in_time_s += t * scale;
        } else {
            split.out_time_s += t * scale;
        }
    }
    split
}

// ═══════════════════════════════════════════════════════════════════════
// Part 8 — SVG
// ═══════════════════════════════════════════════════════════════════════

fn svg_output_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/valley_falsifier_h1");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn path_from_points(points: &[P2], y_flip: f64, close: bool) -> String {
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

/// One arm rendered over the picked region: the region outline, the mask, and
/// every cutting run. `branch_of` gives the branch index of a run (`None` for
/// the raster part), which colours the traced passes.
fn write_arm_svg(
    path: &Path,
    title: &str,
    region: &Polygon2,
    mask_loops: &[Vec<P2>],
    runs: &[Vec<P3>],
    branch_of: &[Option<usize>],
) -> std::io::Result<()> {
    let [x0, y0, x1, y1] = region.bbox();
    let y_flip = y0 + y1;
    let pad = 2.0;
    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{:.2} {:.2} {:.2} {:.2}\" \
         width=\"1200\" height=\"1200\">\n\
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
    let mut d = path_from_points(&region.exterior, y_flip, true);
    for hole in &region.holes {
        d.push_str(&path_from_points(hole, y_flip, true));
    }
    let _ = writeln!(
        svg,
        "<path d=\"{d}\" fill=\"#ffffff\" fill-rule=\"evenodd\" stroke=\"#2b2b2b\" \
         stroke-width=\"0.12\"/>"
    );
    let mut md = String::new();
    for lp in mask_loops {
        if lp.len() >= 3 {
            md.push_str(&path_from_points(lp, y_flip, true));
        }
    }
    let _ = writeln!(
        svg,
        "<path d=\"{md}\" fill=\"#2f6fb5\" fill-opacity=\"0.14\" fill-rule=\"evenodd\" \
         stroke=\"#2f6fb5\" stroke-width=\"0.08\" stroke-opacity=\"0.5\"/>"
    );
    // Raster runs first (grey), traced runs over them (coloured by branch).
    let _ = writeln!(
        svg,
        "<g id=\"raster\" fill=\"none\" stroke=\"#8a8a8a\" stroke-width=\"0.045\">"
    );
    for (i, run) in runs.iter().enumerate() {
        if branch_of.get(i).copied().flatten().is_some() {
            continue;
        }
        let pts: Vec<P2> = run.iter().map(|p| P2::new(p.x, p.y)).collect();
        let _ = writeln!(
            svg,
            "<path d=\"{}\"/>",
            path_from_points(&pts, y_flip, false)
        );
    }
    let _ = writeln!(
        svg,
        "</g>\n<g id=\"traced\" fill=\"none\" stroke-width=\"0.065\">"
    );
    const PALETTE: [&str; 8] = [
        "#c1272d", "#0b6e4f", "#7048a8", "#b8860b", "#1f6feb", "#d1495b", "#2a9d8f", "#8a5a00",
    ];
    for (i, run) in runs.iter().enumerate() {
        let Some(b) = branch_of.get(i).copied().flatten() else {
            continue;
        };
        let pts: Vec<P2> = run.iter().map(|p| P2::new(p.x, p.y)).collect();
        let _ = writeln!(
            svg,
            "<path d=\"{}\" stroke=\"{}\"/>",
            path_from_points(&pts, y_flip, false),
            PALETTE[b % PALETTE.len()]
        );
    }
    let _ = writeln!(svg, "</g>\n</svg>");
    std::fs::write(path, svg)
}

// ═══════════════════════════════════════════════════════════════════════
// The instrument
// ═══════════════════════════════════════════════════════════════════════

struct ArmReport {
    label: String,
    stepover_mm: f64,
    pre_relink_fragments: usize,
    cost: CandidateCost,
    coverage: CoverageAudit,
}

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_valley_branch_falsifier_h1() {
    use rs_cam_core::finish::surface_link::LinkCeiling;
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::machine::kinematics::MachineKinematics;

    eprintln!(
        "\n========== Track H V1 — BRANCH-TRACING FALSIFIER ==========\n\
         Pre-registration: planning/valley_tracing_2026-09-02/FINDINGS.md, Q2 + \"V1 — OPEN\".\n\
         NUMBERS ONLY. B3 / B4 / B5, the margin rule and the spec rule are the \
         orchestrator's.\n"
    );

    let mesh_path = Path::new(WANAKA_MESH);
    if !mesh_path.exists() {
        eprintln!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }
    let started = std::time::Instant::now();
    let mesh = TriangleMesh::from_stl_scaled(mesh_path, 1.0).expect("load wanaka terrain");
    let index = SpatialIndex::build_auto(&mesh);
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
    let stepover = equal_cusp_stepover_mm(cusp_radii[1], CUSP_HEIGHT_MM);
    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::Shallow)
        .map(|r| &r.polygon)
        .collect();
    shallow.sort_by(|a, b| b.area().total_cmp(&a.area()));
    let costed: Vec<Polygon2> = shallow
        .iter()
        .take(MAX_COSTED_REGIONS)
        .map(|p| (*p).clone())
        .collect();
    eprintln!(
        "setup: {} Shallow regions, {} costed; spec stepover s_max = {stepover:.6} mm; \
         classification grid {} x {} at {:.4} mm  ({:.1}s)",
        shallow.len(),
        costed.len(),
        surface.rows(),
        surface.cols(),
        surface.cell_size(),
        started.elapsed().as_secs_f64()
    );

    // ── field + hydrology + mask B at the picked rung ──
    let field = Field {
        nx: heightmap.cols,
        ny: heightmap.rows,
        ox: heightmap.origin_x,
        oy: heightmap.origin_y,
        cell: heightmap.cell_size,
        z: (0..heightmap.rows * heightmap.cols)
            .map(|i| heightmap.z_at_index(i).covered().unwrap_or(f64::NAN))
            .collect(),
        nodata: heightmap.covered_flags().iter().map(|&f| !f).collect(),
    };
    let hydro = field.land_view(LAND_Z_MM);
    let filled = priority_flood_epsilon(&hydro);
    let receivers = d8_receivers(&hydro, &filled);
    let acc = d8_accumulation(&hydro, &filled, &receivers);
    let cell_area = field.cell * field.cell;
    let network: Vec<bool> = (0..hydro.len())
        .map(|i| !hydro.nodata[i] && acc[i] * cell_area >= ACC_THRESHOLD_MM2)
        .collect();
    let dt_cells = chamfer_distance(
        &low_ground_mask(&hydro, TPI_HALF_WINDOW_MM),
        field.nx,
        field.ny,
    );
    let net_cells: Vec<usize> = (0..hydro.len()).filter(|&i| network[i]).collect();
    let net_samples: Vec<P2> = net_cells.iter().map(|&i| field.xy(i)).collect();
    let mask_b_board = buffer_samples(&hydro, &dt_cells, &net_samples);
    eprintln!(
        "mask B at T = {ACC_THRESHOLD_MM2:.0} mm²: {} network cells, {} buffered cells \
         board-wide.",
        net_cells.len(),
        mask_b_board.iter().filter(|&&m| m).count()
    );

    // ── region ownership + per-region derate + costing (the pick table) ──
    let bboxes: Vec<[f64; 4]> = costed.iter().map(Polygon2::bbox).collect();
    let region_of: Vec<i32> = (0..field.len())
        .into_par_iter()
        .map(|i| {
            if field.nodata[i] {
                return -1;
            }
            let p = field.xy(i);
            costed
                .iter()
                .zip(bboxes.iter())
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

    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let block_top_z = mesh.bbox.max.z;
    let stock = machined_stock(
        &mesh,
        &index,
        &r15,
        &tier_map,
        [
            mesh.bbox.min.x,
            mesh.bbox.min.y,
            mesh.bbox.max.x,
            mesh.bbox.max.y,
        ],
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
    let in_mask = |p: P2| -> bool {
        field
            .index_at(p.x, p.y)
            .is_some_and(|i| mask_b_board[i] && region_of[i] >= 0)
    };

    eprintln!(
        "\n---------- REGION PICK: mask-B time share, all {} costed regions ----------\n\
         \x20  Pre-registered as the FAVOURABLE case for arm T: the most valley-dominated\n\
         \x20  region has the fewest hybrid seams, the mechanism most likely to sink T.\n",
        costed.len()
    );
    eprintln!(
        "     {:>4}  {:>10}  {:>8}  {:>9}  {:>10}  {:>10}  {:>9}",
        "rgn", "area mm²", "dir deg", "th max", "step mm", "cut time s", "in-mask %"
    );
    let mut table: Vec<(usize, f64, f64, f64, f64)> = Vec::new();
    for (k, polygon) in costed.iter().enumerate() {
        let (theta, _) = region_max_slope_deg(
            &surface,
            &covered,
            polygon,
            planner.steep_threshold_deg,
            &|_| true,
        );
        let step = if theta > 1.0 {
            stepover * theta.to_radians().cos()
        } else {
            stepover
        };
        let frame = region_frame(polygon, step);
        let grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
            &mesh,
            &index,
            &r10,
            step,
            frame.direction_deg,
            effective_min_z,
        );
        let cells = lattice_monotone_cells(&grid, polygon, effective_min_z);
        let polygons = if cells.cells.is_empty() {
            std::slice::from_ref(polygon).to_vec()
        } else {
            cells.cells.clone()
        };
        let boundary = RegionSet::new(vec![polygon.clone()]);
        let raw = raster_candidate(&grid, &polygons, safe_z, effective_min_z);
        let cost = relink_and_cost_under(raw, &mesh, &index, &r10, &boundary, &kinematics, &regime);
        let split = attribute_cutting(&cost.toolpath, &kinematics, cost.time_s, &in_mask);
        let cut = split.in_time_s + split.out_time_s;
        let share = 100.0 * split.in_time_s / cut.max(1e-9);
        eprintln!(
            "     {:>4}  {:>10.1}  {:>8.2}  {:>9.3}  {:>10.4}  {:>10.1}  {:>8.2}%",
            k + 1,
            polygon.area(),
            frame.direction_deg,
            theta,
            step,
            cut,
            share
        );
        table.push((k, share, split.in_time_s, theta, step));
    }
    let Some(&(pick, pick_share, pick_in_s, pick_theta, _pick_step)) = table
        .iter()
        .max_by(|a, b| a.2.total_cmp(&b.2))
        .or_else(|| table.first())
    else {
        eprintln!("SKIP: no costed region.");
        return;
    };
    let region = costed[pick].clone();
    eprintln!(
        "\n     PICKED: region {} — largest mask-B in-mask cutting TIME ({pick_in_s:.1} s, \
         {pick_share:.2} % of its own cutting time).",
        pick + 1
    );

    // ── the three derates ──
    let region_mask: Vec<bool> = (0..field.len())
        .map(|i| region_of[i] == pick as i32)
        .collect();
    let region_dt = chamfer_distance(&region_mask, field.nx, field.ny);
    let fringe_keep = |i: usize| -> bool { region_dt[i] * field.cell >= OVERLAP_MM };
    let (theta_fringe, n_fringe) = region_max_slope_deg(
        &surface,
        &covered,
        &region,
        planner.steep_threshold_deg,
        &fringe_keep,
    );
    let mask_keep = |i: usize| -> bool { mask_b_board.get(i).copied().unwrap_or(false) };
    let (theta_mask, n_mask) = region_max_slope_deg(
        &surface,
        &covered,
        &region,
        planner.steep_threshold_deg,
        &mask_keep,
    );
    let derate = |theta: f64| {
        if theta > 1.0 {
            stepover * theta.to_radians().cos()
        } else {
            stepover
        }
    };
    eprintln!(
        "\n---------- the derates ----------\n\
         \x20  S  (shipped, whole dilated region polygon): theta_max {pick_theta:.3} deg \
         -> step {:.4} mm\n\
         \x20  S' (overlap-dilation FRINGE EXCISED, DT >= {OVERLAP_MM:.1} mm from the region \
         boundary):\n\
         \x20     theta_max {theta_fringe:.3} deg over {n_fringe} interior cells -> step {:.4} mm\n\
         \x20     RULE USED: chamfer-DT fringe excision. The pre-registered \
         \"undilated polygon\" FALLBACK was NOT taken.\n\
         \x20  S'-mask (the mask as its own region, the pre-registration's permitted \
         alternative):\n\
         \x20     theta_max {theta_mask:.3} deg over {n_mask} mask cells -> step {:.4} mm",
        derate(pick_theta),
        derate(theta_fringe),
        derate(theta_mask),
    );
    let fringe_moved = (theta_fringe - pick_theta).abs() > 1e-6;
    if !fringe_moved {
        eprintln!(
            "\x20  NOTE: fringe excision did NOT move theta_max — S' would be identical to S.\n\
             \x20  The mask-derived derate is therefore ALSO run, so B5 has a control that \
             differs from S."
        );
    }

    // ── build the arms ──
    let region_boundary = RegionSet::new(vec![region.clone()]);
    let mask_region: Vec<bool> = (0..field.len())
        .map(|i| region_mask[i] && mask_b_board[i])
        .collect();
    let step_s = derate(pick_theta);
    let step_sp = derate(theta_fringe);
    let step_spm = derate(theta_mask);

    let build_raster_runs = |step: f64| -> (Vec<Vec<P3>>, f64) {
        let frame = region_frame(&region, step);
        let grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
            &mesh,
            &index,
            &r10,
            step,
            frame.direction_deg,
            effective_min_z,
        );
        let cells = lattice_monotone_cells(&grid, &region, effective_min_z);
        let polys = if cells.cells.is_empty() {
            std::slice::from_ref(&region).to_vec()
        } else {
            cells.cells
        };
        let raw = raster_candidate(&grid, &polys, safe_z, effective_min_z);
        (cut_runs(&raw), frame.direction_deg)
    };

    eprintln!("\n---------- building the arms ----------");
    let t0 = std::time::Instant::now();
    let (runs_s, dir_s) = build_raster_runs(step_s);
    let (runs_sp, dir_sp) = build_raster_runs(step_sp);
    let (runs_spm, dir_spm) = build_raster_runs(step_spm);
    eprintln!(
        "     rasters built in {:.1}s  (S dir {dir_s:.2} deg, S' dir {dir_sp:.2} deg, \
         S'-mask dir {dir_spm:.2} deg)",
        t0.elapsed().as_secs_f64()
    );

    // ── arm T: tracing inside the mask, S' raster outside ──
    let t1 = std::time::Instant::now();
    let region_network: Vec<bool> = (0..field.len())
        .map(|i| network[i] && mask_region[i])
        .collect();
    let branches = valley_polylines(&hydro, &receivers, &acc, &region_network);
    let tracer = TracerCtx {
        mesh: &mesh,
        index: &index,
        cutter: &r10,
        surface: &surface,
        field: &field,
        dt_cells: &dt_cells,
        mask: &mask_region,
        stepover,
        min_z: effective_min_z,
    };
    let mut traced_runs: Vec<Vec<P3>> = Vec::new();
    let mut branch_of: Vec<Option<usize>> = Vec::new();
    let mut stats: Vec<BranchStat> = Vec::new();
    let mut pruned = 0usize;
    for cells in &branches {
        match trace_branch(&tracer, cells) {
            Some((runs, stat)) => {
                let bi = stats.len();
                for run in runs {
                    traced_runs.push(run);
                    branch_of.push(Some(bi));
                }
                stats.push(stat);
            }
            None => pruned += 1,
        }
    }
    // The raster part of the hybrid: S' runs with every point inside mask B
    // removed, re-chained. The gap this leaves along the mask boundary is the
    // SEAM, and it belongs to the candidate — it is not patched.
    let outside = |p: P3| -> bool {
        field
            .index_at(p.x, p.y)
            .is_none_or(|i| !mask_region.get(i).copied().unwrap_or(false))
    };
    let raster_part = split_runs(&runs_sp, &outside);
    let mut runs_t = traced_runs.clone();
    let mut branch_t = branch_of.clone();
    for run in raster_part {
        runs_t.push(run);
        branch_t.push(None);
    }
    eprintln!(
        "     arm T: {} branches traced, {pruned} pruded below \
         {V1_BRANCH_MIN_STEPOVERS:.0} x stepover; {} traced runs + {} raster runs; \
         built in {:.1}s",
        stats.len(),
        traced_runs.len(),
        runs_t.len() - traced_runs.len(),
        t1.elapsed().as_secs_f64()
    );
    if stats.is_empty() {
        eprintln!("     REFUSE: no branch survived pruning — arm T has no tracing to measure.");
    } else {
        eprintln!(
            "\n     per-branch mechanism (the pitch derate had room only where psi_max is \
             large):\n     {:>4}  {:>10}  {:>12}  {:>10}  {:>9}  {:>7}  {:>8}",
            "br", "length mm", "half-w p50", "psi_max", "pitch mm", "passes", "points"
        );
        let mut order: Vec<usize> = (0..stats.len()).collect();
        order.sort_by(|&a, &b| stats[b].length_mm.total_cmp(&stats[a].length_mm));
        for &b in order.iter().take(20) {
            let s = &stats[b];
            eprintln!(
                "     {:>4}  {:>10.3}  {:>12.3}  {:>9.3}°  {:>9.4}  {:>7}  {:>8}",
                b + 1,
                s.length_mm,
                s.half_width_p50_mm,
                s.psi_max_deg,
                s.pitch_mm,
                s.passes,
                s.points
            );
        }
        if stats.len() > 20 {
            eprintln!("     ... {} more branches", stats.len() - 20);
        }
        let mut pitches: Vec<f64> = stats.iter().map(|s| s.pitch_mm).collect();
        pitches.sort_by(f64::total_cmp);
        let mut psis: Vec<f64> = stats.iter().map(|s| s.psi_max_deg).collect();
        psis.sort_by(f64::total_cmp);
        eprintln!(
            "     branch pitch: min {:.4} p50 {:.4} max {:.4} mm  (spec {stepover:.4}, \
             S' {step_sp:.4});  psi_max: min {:.2} p50 {:.2} max {:.2} deg",
            pitches[0],
            pitches[pitches.len() / 2],
            pitches[pitches.len() - 1],
            psis[0],
            psis[psis.len() / 2],
            psis[psis.len() - 1],
        );
    }

    // ── cost + audit every arm identically ──
    let tris = region_triangles(&mesh, &region, CUSP_HEIGHT_MM);
    let [rx0, ry0, rx1, ry1] = region.bbox();
    let audit_bbox = [rx0 - 5.0, ry0 - 5.0, rx1 + 5.0, ry1 + 5.0];
    eprintln!(
        "\n---------- coverage population ----------\n\
         \x20  {} region triangles, 3D area {:.1} mm²; lifted +{CUSP_HEIGHT_MM} mm along the \
         face normal; tested against the ball-CENTRE curve at K_c = {:.3} mm.",
        tris.lifted.len(),
        tris.area_mm2,
        r10.cusp_radius_mm()
    );
    print_slope_census(&tris, planner.steep_threshold_deg);

    let mut arms: Vec<ArmReport> = Vec::new();
    for (label, step, runs) in [
        (
            format!("S   shipped derate ({pick_theta:.3} deg)"),
            step_s,
            &runs_s,
        ),
        (
            format!(
                "S'  fringe-excised derate ({theta_fringe:.3} deg){}",
                if fringe_moved { "" } else { "  == S" }
            ),
            step_sp,
            &runs_sp,
        ),
        (
            format!("S'm mask-as-region derate ({theta_mask:.3} deg)"),
            step_spm,
            &runs_spm,
        ),
        (
            "T   hybrid: per-branch tracing + S' raster".to_owned(),
            step_sp,
            &runs_t,
        ),
    ] {
        let raw = toolpath_from_runs(runs, safe_z);
        let pre = pre_relink_fragments(&raw);
        let cost = relink_and_cost_under(
            raw,
            &mesh,
            &index,
            &r10,
            &region_boundary,
            &kinematics,
            &regime,
        );
        let coverage = audit_arm_coverage(&tris, &cost.toolpath, r10.cusp_radius_mm(), audit_bbox);
        arms.push(ArmReport {
            label,
            stepover_mm: step,
            pre_relink_fragments: pre,
            cost,
            coverage,
        });
    }

    eprintln!(
        "\n---------- ARMS, costed identically under the MACHINED-STOCK ceiling ----------\n\
         \x20  production relink (hookup 25.0, sampling 0.5, reorder true, flush_ride true,\n\
         \x20  airborne exemption true), F-034 costing on the project kinematics.\n"
    );
    eprintln!(
        "     {:>44}  {:>8}  {:>8}  {:>9}  {:>7}  {:>8}  {:>9}  {:>9}",
        "arm", "step mm", "pre-frag", "fragments", "linked", "retracts", "time s", "cut mm"
    );
    for a in &arms {
        eprintln!(
            "     {:>44}  {:>8.4}  {:>8}  {:>9}  {:>7}  {:>8}  {:>9.1}  {:>9.0}",
            a.label,
            a.stepover_mm,
            a.pre_relink_fragments,
            a.cost.fragments,
            a.cost.linked,
            a.cost.kept_retracts,
            a.cost.time_s,
            a.cost.cutting_mm
        );
    }
    eprintln!(
        "\n     {:>44}  {:>14}  {:>12}  {:>14}  {:>13}  coverage gate",
        "arm", "uncovered tri", "unmach mm²", "unmach frac %", "largest mm²"
    );
    for a in &arms {
        let c = &a.coverage;
        let gate = if c.unmachined_area_fraction <= COVERAGE_FAIL_FRACTION {
            "ok"
        } else {
            "FAILS COVERAGE"
        };
        eprintln!(
            "     {:>44}  {:>14}  {:>12.3}  {:>13.4}%  {:>13.5}  {gate}",
            a.label,
            c.uncovered_centroid_triangles,
            c.unmachined_area_mm2,
            100.0 * c.unmachined_area_fraction,
            c.largest_unmachined_triangle_area_mm2
        );
        // The shipped audit's own discriminator, and the row that tells a
        // knife-edge apart from a hole: read it AGAINST the 1.000 mm radius.
        // Microns above is a cusp-midline grazing; a millimetre above is a
        // genuine gap.
        eprintln!(
            "     {:>44}    uncovered distance vs K_c {:.3}: min {:.4} median {:.4} max {:.4} mm \
             over {} samples",
            "",
            c.coverage_radius_mm,
            c.uncovered_distance.min_mm,
            c.uncovered_distance.median_mm,
            c.uncovered_distance.max_mm,
            c.uncovered_distance.samples,
        );
    }

    eprintln!(
        "\n---------- SPEC-MEETING (achieved spacing) — NOT MEASURED ----------\n\
         \x20  BLOCKED, reported rather than substituted. See the run report: the b1\n\
         \x20  instrument's measurement cannot be applied to these three arms as written.\n\
         \x20  No achieved-spacing distribution and no exceed % are printed, because a\n\
         \x20  gating statistic computed under a convention this agent invented would be\n\
         \x20  worse than an absent one."
    );

    // ── the VISUAL ──
    let mut mask_loops = marching_squares_bool_grid(
        &mask_region,
        field.ny,
        field.nx,
        field.ox,
        field.oy,
        field.cell,
    );
    mask_loops.retain(|l| l.len() >= 3);
    let dir = svg_output_dir();
    let sp_labels: Vec<Option<usize>> = vec![None; runs_sp.len()];
    for (name, runs, labels, title) in [
        (
            "arm_t_region.svg",
            &runs_t,
            &branch_t,
            "Track H V1 — arm T (per-branch tracing + S' raster) on the picked region",
        ),
        (
            "arm_s_prime_region.svg",
            &runs_sp,
            &sp_labels,
            "Track H V1 — arm S' (raster, fringe-excised derate) on the picked region",
        ),
    ] {
        let path = dir.join(name);
        match write_arm_svg(&path, title, &region, &mask_loops, runs, labels) {
            Ok(()) => eprintln!("\nSVG: {}", path.display()),
            Err(e) => eprintln!("\nSVG write FAILED for {}: {e}", path.display()),
        }
    }

    eprintln!(
        "\n========== END — total {:.1}s. No verdict is written here. ==========\n",
        started.elapsed().as_secs_f64()
    );
}
