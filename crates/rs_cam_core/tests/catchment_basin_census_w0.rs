//! **Track H phase W0 — the catchment BASIN census.**
//!
//! Pre-registration: `planning/valley_tracing_2026-09-02/FINDINGS.md`, §"Phase
//! W" and §W0 (commit `431e5eaa`). A census ONLY — no toolpath generation and
//! no new strategy code. It writes NO verdict: bar W0-a and the seam budget
//! are applied by the orchestrator.
//!
//! Upstream: `tests/valley_prize_census_h0.rs` (V0) and
//! `tests/valley_branch_falsifier_h1.rs` (V1). Helpers are COPIED, not
//! shared — an integration test cannot `use` another one.
//!
//! # The reframe, and the one assumption it removes
//!
//! A catchment here is a ZONE: one basin, ridge divide to trunk, the FULL
//! slope range. **There is no Shallow-band clip.** V0 and V1 ran on the
//! tier-1 Shallow band, which is why they measured flat valley floors; W runs
//! on the whole front finish territory at every slope.
//!
//! **Territory, stated precisely:** the union of ALL planned regions of the
//! tier-1 finish decomposition — every band, not just `Shallow`, and every
//! region, not V0's largest sixteen. That is exactly V0's construction with
//! the band filter deleted. The covered-cell mask the decomposition itself
//! consumes is printed beside it so the two are comparable.
//!
//! # Definitions fixed BEFORE the run
//!
//! ## Trunks, links and basins
//!
//! * **Trunk network** — flow-accumulation cells at or above the pre-
//!   registered rung `T = 512 mm²`, on the LAND view (the board carries a
//!   raised outer edge band, so a whole-board flood drowns the interior; V0
//!   measured that at 99.50 % of cells raised).
//! * **A trunk LINK** is a maximal chain of trunk cells between junctions: a
//!   new link starts at a trunk head (no trunk donor) and at every junction
//!   (a trunk cell with two or more trunk donors), and runs downstream until
//!   the next junction. This is the classical channel-link decomposition, and
//!   it is what makes "one basin, ridge divide to trunk" well posed — a basin
//!   per link, not a basin per trunk CELL.
//! * **A basin** is the set of cells whose steepest-descent path first meets
//!   the trunk network inside one link. Labelling runs over cells in ASCENDING
//!   filled height, so a cell's receiver is always already labelled.
//! * **Cells that reach a land outlet without ever meeting a trunk** (coastal
//!   ground draining straight to the sea) get their own basin keyed by that
//!   outlet. They are counted and reported SEPARATELY as `coastal` — the
//!   pre-registration says "to a trunk outlet", and this is the case it did
//!   not name, so it is surfaced rather than folded in silently.
//! * **Merge:** any basin whose territory XY area is under
//!   [`MIN_BASIN_AREA_MM2`] is merged into the basin containing its outlet
//!   cell's receiver — its downstream neighbour — smallest first, repeated
//!   until no basin is under the floor or has nowhere to go. A basin with no
//!   downstream neighbour (a terminal link) cannot merge and is reported.
//!
//! ## Shape
//!
//! * **Simple connectivity** — marching squares over the basin ∩ territory
//!   mask. A simply connected basin has exactly ONE boundary loop AND one
//!   connected component. Both are reported, because "two loops" is a hole
//!   and "two components" is a split basin and they are different failures.
//! * **PCA aspect ratio** — `sqrt(lambda_major / lambda_minor)` of the basin
//!   ∩ territory cell cloud, i.e. the ratio of standard deviations along the
//!   principal axes. Same convention as `monotone_cells::
//!   pca_minor_and_elongation`, so a number here is comparable with the C2
//!   elongation gate.
//! * **Slope-band mix** — share of the basin's 3-D area in
//!   0–15 / 15–30 / 30–45 / 45+ degrees, from the classification normals.
//!   3-D area, not XY: the bands are about machining, and XY would understate
//!   every steep share.
//! * **Clipped-by-territory** — true when the basin has cells OUTSIDE the
//!   territory, i.e. the territory boundary cuts it. Such a basin's shape
//!   numbers describe the clipped piece, not the landform.
//!
//! ## Divide length
//!
//! The total length of boundaries BETWEEN different basins, counted as shared
//! 4-neighbour cell edges with both cells in the territory, each contributing
//! one cell edge. **This is a staircase measure**: a divide running diagonally
//! is over-measured by up to `4/pi` = 1.27x against its true polyline length.
//! It is reported raw because it feeds a seam BUDGET, where over-measuring is
//! the conservative direction.
//!
//! # Running it
//!
//! ```text
//! cargo test -p rs_cam_core --test catchment_basin_census_w0 \
//!   wanaka_catchment_basin_census_w0 -- --ignored --nocapture
//! ```
//!
//! `#[ignore]` — needs the operator's wanaka mesh, not in the repo. SKIPS
//! rather than fails when it is absent, and never substitutes
//! `fixtures/terrain_small.stl` (banned by the pre-registration).

#![allow(
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use rs_cam_core::classify_probe::ClassificationSampler;
use rs_cam_core::contour_extract::marching_squares_bool_grid;
use rs_cam_core::finish_planner::{FinishPlannerParams, decompose};
use rs_cam_core::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::tier_map::{ResidualTreatment, TierLadder, TierMapParams, compute_tier_map};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::unified_finish::unified_finish_classification_resolution;

const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

// ── dials, from `planning/multitool_2026-08-23/wanaka200_mt2.toml` ──────

const CELL_MM: f64 = 0.3;
const TOLERANCE_MM: f64 = 0.05;
const MARGIN_MM: f64 = 0.5;
const COARSENESS: f64 = 1.0;
const OVERLAP_MM: f64 = 2.0;
const MAX_REGIONS_PER_TIER: usize = 24;
const CUSP_HEIGHT_MM: f64 = 0.03;
const OP_TOLERANCE_MM: f64 = 0.05;

// ── V0 conventions carried forward ──────────────────────────────────────

const LAND_Z_MM: f64 = 0.0;
const FILL_EPSILON_MM: f64 = 1.0e-6;

// ── W0 dials, pre-registered ────────────────────────────────────────────

/// The pre-registered trunk rung.
const TRUNK_T_MM2: f64 = 512.0;
/// The neighbouring rungs the aggregates are repeated at, so the pick is not
/// load-bearing.
const TRUNK_SENSITIVITY_MM2: [f64; 3] = [256.0, 512.0, 1024.0];
/// A basin under this territory XY area is merged into its downstream
/// neighbour.
const MIN_BASIN_AREA_MM2: f64 = 50.0;
/// Bar W0-a's shape class: simply connected AND PCA aspect at or under this.
const COMPACT_ASPECT_MAX: f64 = 2.0;
/// The seam budget's two declared predictors (`FINDINGS` §W0).
const SEAM_JUNCTION_S: f64 = 0.38;
/// The open-pass predictor divides the divide length by the spec stepover.
const SPEC_STEPOVER_MM: f64 = 0.486_210;

/// `s = 2·sqrt(2Rh − h²)` — the equal-cusp law, restated so the instrument
/// shows its own arithmetic.
fn equal_cusp_stepover_mm(cusp_radius_mm: f64, h: f64) -> f64 {
    if h <= 0.0 || h >= cusp_radius_mm {
        return 0.0;
    }
    2.0 * (2.0 * cusp_radius_mm * h - h * h).sqrt()
}

// ═══════════════════════════════════════════════════════════════════════
// Part 0 — heightfield + hydrology (copied verbatim from the V0 census)
// ═══════════════════════════════════════════════════════════════════════
/// A rasterised heightfield: row-major, `nodata` where nothing verified there
/// is surface.
struct Field {
    nx: usize,
    ny: usize,
    ox: f64,
    oy: f64,
    cell: f64,
    z: Vec<f64>,
    nodata: Vec<bool>,
}

impl Field {
    fn len(&self) -> usize {
        self.nx * self.ny
    }

    fn xy(&self, i: usize) -> P2 {
        let row = i / self.nx;
        let col = i % self.nx;
        P2::new(
            self.ox + col as f64 * self.cell,
            self.oy + row as f64 * self.cell,
        )
    }

    /// The same lattice with everything at or below `land_z` marked nodata —
    /// the LAND view, on which the hydrology runs.
    ///
    /// **Why this exists.** The first run of this instrument flooded the whole
    /// board: the priority flood raised **99.50 %** of data cells, and the
    /// accumulation field it produced was an artefact of flood order rather
    /// than a drainage network. The cause is geometry, not code — the rivmap
    /// export writes a RAISED OUTER EDGE BAND (`edge_profile = 3`,
    /// `edge_wall_deg = 41`, `edge_top_offset_mm = 0.0` in `rivmap_data.toml`),
    /// so the board's interior is one closed basin with its rim as the only
    /// exit, and a correct priority flood fills it to that rim. The whole-board
    /// flood is still run and its raised fraction printed, as the evidence for
    /// this restriction; the DECIDING mask uses the land view, in which the
    /// coastline is the base level and lakes on land still fill to their own
    /// spill points.
    fn land_view(&self, land_z: f64) -> Self {
        let nodata = (0..self.len())
            // NaN at a nodata cell is already excluded by the first term, so
            // `<=` here is a total comparison in practice.
            .map(|i| self.nodata[i] || self.z[i] <= land_z)
            .collect();
        Self {
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

/// The eight D8 offsets and their step lengths in cells.
const NB8: [(isize, isize); 8] = [
    (-1, 0),
    (1, 0),
    (0, -1),
    (0, 1),
    (-1, -1),
    (-1, 1),
    (1, -1),
    (1, 1),
];

fn neighbour(field: &Field, i: usize, k: usize) -> Option<(usize, f64)> {
    let (dr, dc) = NB8[k];
    let row = (i / field.nx) as isize + dr;
    let col = (i % field.nx) as isize + dc;
    if row < 0 || col < 0 || row >= field.ny as isize || col >= field.nx as isize {
        return None;
    }
    let step = if dr != 0 && dc != 0 {
        std::f64::consts::SQRT_2
    } else {
        1.0
    };
    Some(((row as usize) * field.nx + col as usize, step))
}

/// Ordering shim: `f64` has no `Ord`, and the priority flood needs a min-heap.
#[derive(PartialEq)]
struct HeapItem(f64, usize);

impl Eq for HeapItem {}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0).then(self.1.cmp(&other.1))
    }
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Barnes/Lehman/Soille **priority flood + epsilon**. Returns the filled
/// height at every data cell (`f64::NAN` at nodata).
///
/// The plain fill leaves flats, and a flat has no D8 downslope neighbour, so
/// accumulation dies inside it. This board HAS lakes (`lakes.dxf` sits beside
/// the mesh in the export), so the epsilon variant is required, not optional:
/// every filled cell gets a strictly-lower path to its outlet.
///
/// Seeds are the grid border plus every data cell adjacent to nodata — the
/// board's rim and the trench ring are the outlets, which is correct.
fn priority_flood_epsilon(field: &Field) -> Vec<f64> {
    let n = field.len();
    let mut out = vec![f64::NAN; n];
    let mut closed = field.nodata.clone();
    let mut open: BinaryHeap<Reverse<HeapItem>> = BinaryHeap::new();
    let mut pit: VecDeque<usize> = VecDeque::new();

    for i in 0..n {
        if field.nodata[i] {
            continue;
        }
        let row = i / field.nx;
        let col = i % field.nx;
        let border = row == 0 || col == 0 || row + 1 == field.ny || col + 1 == field.nx;
        let beside_nodata =
            (0..8).any(|k| neighbour(field, i, k).is_none_or(|(j, _)| field.nodata[j]));
        if border || beside_nodata {
            closed[i] = true;
            out[i] = field.z[i];
            open.push(Reverse(HeapItem(field.z[i], i)));
        }
    }

    while !open.is_empty() || !pit.is_empty() {
        let c = if let Some(c) = pit.pop_front() {
            c
        } else {
            match open.pop() {
                Some(Reverse(HeapItem(_, c))) => c,
                None => break,
            }
        };
        let zc = out[c];
        for k in 0..8 {
            let Some((j, _)) = neighbour(field, c, k) else {
                continue;
            };
            if closed[j] {
                continue;
            }
            closed[j] = true;
            if field.z[j] <= zc + FILL_EPSILON_MM {
                out[j] = zc + FILL_EPSILON_MM;
                pit.push_back(j);
            } else {
                out[j] = field.z[j];
                open.push(Reverse(HeapItem(out[j], j)));
            }
        }
    }
    out
}

/// D8 receivers on the filled surface: the steepest-descent neighbour, or
/// `None` where nothing is lower (an outlet).
fn d8_receivers(field: &Field, filled: &[f64]) -> Vec<Option<u32>> {
    let n = field.len();
    let mut out = vec![None; n];
    for i in 0..n {
        if field.nodata[i] {
            continue;
        }
        let zi = filled[i];
        let mut best: Option<(f64, u32)> = None;
        for k in 0..8 {
            let Some((j, step)) = neighbour(field, i, k) else {
                continue;
            };
            if field.nodata[j] {
                continue;
            }
            let drop = (zi - filled[j]) / (step * field.cell);
            if drop > 0.0 && best.is_none_or(|(b, _)| drop > b) {
                best = Some((drop, j as u32));
            }
        }
        out[i] = best.map(|(_, j)| j);
    }
    out
}

/// Upstream cell count per cell, by processing cells in decreasing filled
/// height (a valid topological order for D8 on a strictly-descending field).
fn d8_accumulation(field: &Field, filled: &[f64], receivers: &[Option<u32>]) -> Vec<f64> {
    let n = field.len();
    let mut acc = vec![0.0f64; n];
    let mut order: Vec<u32> = (0..n as u32)
        .filter(|&i| !field.nodata[i as usize])
        .collect();
    order.sort_by(|&a, &b| filled[b as usize].total_cmp(&filled[a as usize]));
    for &i in &order {
        acc[i as usize] += 1.0;
    }
    for &i in &order {
        if let Some(r) = receivers[i as usize] {
            let carried = acc[i as usize];
            acc[r as usize] += carried;
        }
    }
    acc
}

// ═══════════════════════════════════════════════════════════════════════
// Part 1 — trunk links and watershed labelling
// ═══════════════════════════════════════════════════════════════════════

/// `u32::MAX` is the "no basin" sentinel throughout.
const NO_BASIN: u32 = u32::MAX;

/// The trunk network decomposed into links, plus the labelling that follows.
struct Watershed {
    /// Basin label per field cell, `NO_BASIN` where the cell is nodata.
    label: Vec<u32>,
    /// Per basin: the field cell its flow leaves through (the link's
    /// downstream end, or the terminal outlet for a coastal basin).
    outlet: Vec<usize>,
    /// True for a basin keyed by a land outlet rather than a trunk link — the
    /// case the pre-registration did not name.
    coastal: Vec<bool>,
    trunk_cells: usize,
    junctions: usize,
    /// Basins opened by a trunk LINK, and by a land outlet that never met a
    /// trunk. Printed, because the first run of this instrument passed the
    /// FULL field here while `receivers` came from the LAND view, so every
    /// sea cell read as a receiver-less outlet and opened its own basin —
    /// 100 398 of them, which swamped every aggregate. These two counts make
    /// that class of mistake visible instead of arithmetic.
    link_basins: usize,
    outlet_basins: usize,
}

/// Decompose the trunk network into links and label every land cell with the
/// link its steepest-descent path first meets.
///
/// **`field` MUST be the same domain `receivers` was computed on** — the LAND
/// view. Handing it the full heightfield makes every sea cell a receiver-less
/// outlet with a basin of its own.
fn label_basins(
    field: &Field,
    filled: &[f64],
    receivers: &[Option<u32>],
    trunk: &[bool],
) -> Watershed {
    let n = field.len();
    // Trunk donor counts -> junctions.
    let mut trunk_donors = vec![0u8; n];
    for i in 0..n {
        if !trunk[i] {
            continue;
        }
        if let Some(r) = receivers[i]
            && trunk[r as usize]
        {
            trunk_donors[r as usize] = trunk_donors[r as usize].saturating_add(1);
        }
    }
    let junctions = (0..n).filter(|&i| trunk[i] && trunk_donors[i] >= 2).count();

    // Link ids: a new link opens at a trunk head (no trunk donor) and at every
    // junction, and runs downstream until the next junction.
    let mut link = vec![NO_BASIN; n];
    let mut outlet: Vec<usize> = Vec::new();
    let mut coastal: Vec<bool> = Vec::new();
    for i in 0..n {
        if !trunk[i] {
            continue;
        }
        let starts_link = trunk_donors[i] != 1;
        if !starts_link || link[i] != NO_BASIN {
            continue;
        }
        let id = outlet.len() as u32;
        let mut c = i;
        loop {
            link[c] = id;
            match receivers[c] {
                Some(r) if trunk[r as usize] && trunk_donors[r as usize] == 1 => {
                    if link[r as usize] != NO_BASIN {
                        outlet.push(c);
                        break;
                    }
                    c = r as usize;
                }
                _ => {
                    outlet.push(c);
                    break;
                }
            }
        }
        coastal.push(false);
    }
    let link_basins = outlet.len();

    // Every land cell inherits its receiver's label unless it is itself trunk.
    // Ascending filled height guarantees the receiver is already resolved.
    let mut order: Vec<u32> = (0..n as u32)
        .filter(|&i| !field.nodata[i as usize])
        .collect();
    order.sort_by(|&a, &b| filled[a as usize].total_cmp(&filled[b as usize]));
    let mut label = vec![NO_BASIN; n];
    for &i in &order {
        let i = i as usize;
        if trunk[i] {
            label[i] = link[i];
            continue;
        }
        match receivers[i] {
            Some(r) => label[i] = label[r as usize],
            None => {
                // A land outlet that is not a trunk: coastal ground draining
                // straight to the sea. Its own basin, flagged.
                let id = outlet.len() as u32;
                outlet.push(i);
                coastal.push(true);
                label[i] = id;
            }
        }
    }
    let outlet_basins = outlet.len() - link_basins;
    Watershed {
        label,
        outlet,
        coastal,
        trunk_cells: (0..n).filter(|&i| trunk[i]).count(),
        junctions,
        link_basins,
        outlet_basins,
    }
}

/// Merge basins under the area floor into their downstream neighbour,
/// smallest first. Returns the remap and how many merges happened.
fn merge_small_basins(
    ws: &Watershed,
    receivers: &[Option<u32>],
    territory_cells: &[usize],
    cell_area: f64,
) -> (Vec<u32>, usize, usize) {
    let count = ws.outlet.len();
    let mut remap: Vec<u32> = (0..count as u32).collect();
    let resolve = |remap: &Vec<u32>, mut b: u32| -> u32 {
        while remap[b as usize] != b {
            b = remap[b as usize];
        }
        b
    };
    let mut area = vec![0.0f64; count];
    for &i in territory_cells {
        let b = ws.label[i];
        if b != NO_BASIN {
            area[b as usize] += cell_area;
        }
    }
    // The downstream neighbour of a basin: the basin holding its outlet's
    // receiver.
    let downstream: Vec<Option<u32>> = (0..count)
        .map(|b| {
            let o = ws.outlet[b];
            receivers[o].and_then(|r| {
                let target = ws.label[r as usize];
                (target != NO_BASIN && target != b as u32).then_some(target)
            })
        })
        .collect();

    let mut merged = 0usize;
    let mut stranded = 0usize;
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by(|&a, &b| area[a].total_cmp(&area[b]));
    for b in order {
        let root = resolve(&remap, b as u32);
        if area[root as usize] >= MIN_BASIN_AREA_MM2 {
            continue;
        }
        let Some(down) = downstream[b] else {
            stranded += 1;
            continue;
        };
        let target = resolve(&remap, down);
        if target == root {
            stranded += 1;
            continue;
        }
        remap[root as usize] = target;
        let carried = area[root as usize];
        area[target as usize] += carried;
        area[root as usize] = 0.0;
        merged += 1;
    }
    let flat: Vec<u32> = (0..count as u32).map(|b| resolve(&remap, b)).collect();
    (flat, merged, stranded)
}

// ═══════════════════════════════════════════════════════════════════════
// Part 2 — per-basin shape
// ═══════════════════════════════════════════════════════════════════════

struct BasinShape {
    area_xy_mm2: f64,
    area_3d_mm2: f64,
    loops: usize,
    components: usize,
    aspect: f64,
    slope_share: [f64; 4],
    clipped: bool,
    coastal: bool,
}

/// 4-connected component count over a boolean mask restricted to `nx * ny`.
fn component_count(mask: &[bool], nx: usize, ny: usize) -> usize {
    let mut seen = vec![false; mask.len()];
    let mut components = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        components += 1;
        seen[start] = true;
        stack.push(start);
        while let Some(c) = stack.pop() {
            let (r, col) = (c / nx, c % nx);
            let push = |j: usize, seen: &mut Vec<bool>, stack: &mut Vec<usize>| {
                if mask[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if col > 0 {
                push(c - 1, &mut seen, &mut stack);
            }
            if col + 1 < nx {
                push(c + 1, &mut seen, &mut stack);
            }
            if r > 0 {
                push(c - nx, &mut seen, &mut stack);
            }
            if r + 1 < ny {
                push(c + nx, &mut seen, &mut stack);
            }
        }
    }
    components
}

/// `sqrt(lambda_major / lambda_minor)` of a cell cloud — the ratio of
/// standard deviations along the principal axes, the same convention as
/// `monotone_cells::pca_minor_and_elongation`.
fn pca_aspect(points: &[P2]) -> f64 {
    let n = points.len() as f64;
    if n < 3.0 {
        return f64::INFINITY;
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
    cxx /= n;
    cxy /= n;
    cyy /= n;
    let trace = cxx + cyy;
    let det = cxx * cyy - cxy * cxy;
    let spread = ((trace * trace / 4.0) - det).max(0.0).sqrt();
    let large = trace / 2.0 + spread;
    let small = trace / 2.0 - spread;
    if small <= 1e-12 {
        return f64::INFINITY;
    }
    (large / small).sqrt()
}

/// Everything the shape census reads that is not the basin itself.
struct ShapeCtx<'a> {
    field: &'a Field,
    territory: &'a [bool],
    slope_deg: &'a [f64],
    cell_area: f64,
}

/// The hydrology, bundled so [`run_rung`] stays inside clippy's argument
/// bound — and so the LAND view travels WITH the arrays that were computed on
/// it. Passing `field` where `hydro` belongs is the bug this bundling is
/// meant to prevent from recurring.
struct Hydrology<'a> {
    hydro: &'a Field,
    filled: &'a [f64],
    receivers: &'a [Option<u32>],
    acc: &'a [f64],
}

/// The per-basin shape row, over `basin ∩ territory`.
fn basin_shape(ctx: &ShapeCtx<'_>, label: &[u32], basin: u32, coastal: bool) -> Option<BasinShape> {
    let field = ctx.field;
    let mut cells: Vec<usize> = Vec::new();
    let mut outside = false;
    for (i, &lab) in label.iter().enumerate().take(field.len()) {
        if lab != basin {
            continue;
        }
        if ctx.territory[i] {
            cells.push(i);
        } else {
            outside = true;
        }
    }
    if cells.is_empty() {
        return None;
    }
    // A local window keeps marching squares and the component scan on the
    // basin's own bbox rather than the whole board.
    let (mut r0, mut c0, mut r1, mut c1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for &i in &cells {
        let (r, c) = (i / field.nx, i % field.nx);
        r0 = r0.min(r);
        c0 = c0.min(c);
        r1 = r1.max(r);
        c1 = c1.max(c);
    }
    // One cell of padding on every side so a basin touching the window edge
    // still closes its loop.
    let (wr0, wc0) = (r0.saturating_sub(1), c0.saturating_sub(1));
    let (wr1, wc1) = ((r1 + 1).min(field.ny - 1), (c1 + 1).min(field.nx - 1));
    let wnx = wc1 - wc0 + 1;
    let wny = wr1 - wr0 + 1;
    let mut window = vec![false; wnx * wny];
    let mut area_xy = 0.0f64;
    let mut area_3d = 0.0f64;
    let mut slope_area = [0.0f64; 4];
    let mut cloud: Vec<P2> = Vec::with_capacity(cells.len());
    for &i in &cells {
        let (r, c) = (i / field.nx, i % field.nx);
        window[(r - wr0) * wnx + (c - wc0)] = true;
        area_xy += ctx.cell_area;
        let deg = ctx.slope_deg[i];
        let cos = deg.to_radians().cos();
        let d_area = if cos > 1e-6 {
            ctx.cell_area / cos
        } else {
            ctx.cell_area
        };
        area_3d += d_area;
        let band = if deg < 15.0 {
            0
        } else if deg < 30.0 {
            1
        } else if deg < 45.0 {
            2
        } else {
            3
        };
        slope_area[band] += d_area;
        cloud.push(field.xy(i));
    }
    let loops = marching_squares_bool_grid(
        &window,
        wny,
        wnx,
        field.ox + wc0 as f64 * field.cell,
        field.oy + wr0 as f64 * field.cell,
        field.cell,
    )
    .len();
    let components = component_count(&window, wnx, wny);
    let mut slope_share = [0.0f64; 4];
    for (b, slot) in slope_share.iter_mut().enumerate() {
        *slot = slope_area[b] / area_3d.max(1e-12);
    }
    Some(BasinShape {
        area_xy_mm2: area_xy,
        area_3d_mm2: area_3d,
        loops,
        components,
        aspect: pca_aspect(&cloud),
        slope_share,
        clipped: outside,
        coastal,
    })
}

/// Divide length: shared 4-neighbour cell edges between DIFFERENT basins with
/// both cells in the territory. A staircase measure — see the file header.
fn divide_length_mm(field: &Field, label: &[u32], territory: &[bool]) -> f64 {
    let mut edges = 0usize;
    for r in 0..field.ny {
        for c in 0..field.nx {
            let i = r * field.nx + c;
            if !territory[i] {
                continue;
            }
            if c + 1 < field.nx {
                let j = i + 1;
                if territory[j] && label[i] != label[j] {
                    edges += 1;
                }
            }
            if r + 1 < field.ny {
                let j = i + field.nx;
                if territory[j] && label[i] != label[j] {
                    edges += 1;
                }
            }
        }
    }
    edges as f64 * field.cell
}

// ═══════════════════════════════════════════════════════════════════════
// Part 3 — the aggregates, at one trunk rung
// ═══════════════════════════════════════════════════════════════════════

struct RungResult {
    trunk_t_mm2: f64,
    basins_before_merge: usize,
    basins_after_merge: usize,
    merges: usize,
    stranded: usize,
    coastal_basins: usize,
    coastal_area_mm2: f64,
    trunk_cells: usize,
    junctions: usize,
    link_basins: usize,
    outlet_basins: usize,
    divide_mm: f64,
    compact_area_mm2: f64,
    territory_area_mm2: f64,
    shapes: Vec<(u32, BasinShape)>,
}

/// Label, merge and measure at one trunk rung.
fn run_rung(
    hy: &Hydrology<'_>,
    ctx: &ShapeCtx<'_>,
    territory_cells: &[usize],
    trunk_t_mm2: f64,
) -> RungResult {
    let field = ctx.field;
    let (hydro, filled, receivers, acc) = (hy.hydro, hy.filled, hy.receivers, hy.acc);
    let trunk: Vec<bool> = (0..hydro.len())
        .map(|i| !hydro.nodata[i] && acc[i] * ctx.cell_area >= trunk_t_mm2)
        .collect();
    // The LAND view, not `field`: `receivers` was computed on it.
    let ws = label_basins(hydro, filled, receivers, &trunk);
    let before = ws.outlet.len();
    let (remap, merges, stranded) =
        merge_small_basins(&ws, receivers, territory_cells, ctx.cell_area);
    let label: Vec<u32> = ws
        .label
        .iter()
        .map(|&b| {
            if b == NO_BASIN {
                NO_BASIN
            } else {
                remap[b as usize]
            }
        })
        .collect();

    let mut present: Vec<u32> = territory_cells
        .iter()
        .map(|&i| label[i])
        .filter(|&b| b != NO_BASIN)
        .collect();
    present.sort_unstable();
    present.dedup();

    let mut shapes: Vec<(u32, BasinShape)> = Vec::new();
    let mut compact_area = 0.0f64;
    let mut territory_area = 0.0f64;
    let mut coastal_basins = 0usize;
    let mut coastal_area = 0.0f64;
    for &b in &present {
        // A merged basin is coastal only if EVERY constituent was; a trunk
        // basin that swallowed a coastal stub is a trunk basin.
        let coastal = (0..ws.coastal.len())
            .filter(|&k| remap[k] == b)
            .all(|k| ws.coastal[k]);
        let Some(shape) = basin_shape(ctx, &label, b, coastal) else {
            continue;
        };
        territory_area += shape.area_xy_mm2;
        if shape.coastal {
            coastal_basins += 1;
            coastal_area += shape.area_xy_mm2;
        }
        if shape.loops == 1 && shape.components == 1 && shape.aspect <= COMPACT_ASPECT_MAX {
            compact_area += shape.area_xy_mm2;
        }
        shapes.push((b, shape));
    }
    shapes.sort_by(|a, b| b.1.area_xy_mm2.total_cmp(&a.1.area_xy_mm2));
    RungResult {
        trunk_t_mm2,
        basins_before_merge: before,
        basins_after_merge: shapes.len(),
        merges,
        stranded,
        coastal_basins,
        coastal_area_mm2: coastal_area,
        trunk_cells: ws.trunk_cells,
        junctions: ws.junctions,
        link_basins: ws.link_basins,
        outlet_basins: ws.outlet_basins,
        divide_mm: divide_length_mm(field, &label, ctx.territory),
        compact_area_mm2: compact_area,
        territory_area_mm2: territory_area,
        shapes,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Part 4 — SVG
// ═══════════════════════════════════════════════════════════════════════

fn svg_output_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/catchment_census_w0");
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

fn downsample_bool(
    mask: &[bool],
    nx: usize,
    ny: usize,
    factor: usize,
) -> (Vec<bool>, usize, usize) {
    let ox = nx.div_ceil(factor);
    let oy = ny.div_ceil(factor);
    let mut out = vec![false; ox * oy];
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

/// Deterministic well-separated hue per basin — a golden-angle walk, so
/// neighbouring ids never share a colour.
fn basin_fill(seq: usize) -> String {
    let hue = (seq as f64 * 137.507_764_05) % 360.0;
    format!("hsl({hue:.1} 62% 58%)")
}

struct BasinSvg<'a> {
    field: &'a Field,
    territory: &'a [bool],
    label: &'a [u32],
    ordered: &'a [u32],
    divides: &'a [bool],
}

fn write_basin_svg(inputs: &BasinSvg<'_>, path: &Path, title: &str) -> std::io::Result<()> {
    let field = inputs.field;
    let x0 = field.ox;
    let y0 = field.oy;
    let x1 = field.ox + (field.nx - 1) as f64 * field.cell;
    let y1 = field.oy + (field.ny - 1) as f64 * field.cell;
    let y_flip = y0 + y1;
    let pad = 4.0;
    let factor = ((field.nx as f64 / 420.0).ceil() as usize).max(1);
    let ds_cell = field.cell * factor as f64;

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

    // Height bands under everything — the hillshade stand-in.
    let mut zmin = f64::INFINITY;
    let mut zmax = f64::NEG_INFINITY;
    for i in 0..field.len() {
        if !field.nodata[i] {
            zmin = zmin.min(field.z[i]);
            zmax = zmax.max(field.z[i]);
        }
    }
    const BANDS: usize = 9;
    let _ = writeln!(svg, "<g id=\"height-bands\">");
    for band in 0..BANDS {
        let level = zmin + (zmax - zmin) * band as f64 / BANDS as f64;
        let mask: Vec<bool> = (0..field.len())
            .map(|i| !field.nodata[i] && field.z[i] >= level)
            .collect();
        let (ds, dnx, dny) = downsample_bool(&mask, field.nx, field.ny, factor);
        let loops = marching_squares_bool_grid(&ds, dny, dnx, field.ox, field.oy, ds_cell);
        let shade = 238 - band * 15;
        let mut d = String::new();
        for lp in &loops {
            if lp.len() >= 3 {
                d.push_str(&path_from_loop(lp, y_flip, true));
            }
        }
        if !d.is_empty() {
            let _ = writeln!(
                svg,
                "<path d=\"{d}\" fill=\"rgb({shade},{},{})\" fill-rule=\"evenodd\" \
                 stroke=\"none\"/>",
                shade.saturating_sub(6),
                shade.saturating_sub(18)
            );
        }
    }
    let _ = writeln!(svg, "</g>\n<g id=\"basins\">");
    for (seq, &b) in inputs.ordered.iter().enumerate() {
        let mask: Vec<bool> = (0..field.len())
            .map(|i| inputs.territory[i] && inputs.label[i] == b)
            .collect();
        let (ds, dnx, dny) = downsample_bool(&mask, field.nx, field.ny, factor);
        let loops = marching_squares_bool_grid(&ds, dny, dnx, field.ox, field.oy, ds_cell);
        let mut d = String::new();
        for lp in &loops {
            if lp.len() >= 3 {
                d.push_str(&path_from_loop(lp, y_flip, true));
            }
        }
        if !d.is_empty() {
            let _ = writeln!(
                svg,
                "<path d=\"{d}\" fill=\"{}\" fill-opacity=\"0.62\" fill-rule=\"evenodd\" \
                 stroke=\"none\"/>",
                basin_fill(seq)
            );
        }
    }
    let _ = writeln!(svg, "</g>");

    // Divides, then the territory outline over everything.
    let (ds, dnx, dny) = downsample_bool(inputs.divides, field.nx, field.ny, factor);
    let loops = marching_squares_bool_grid(&ds, dny, dnx, field.ox, field.oy, ds_cell);
    let mut d = String::new();
    for lp in &loops {
        if lp.len() >= 2 {
            d.push_str(&path_from_loop(lp, y_flip, false));
        }
    }
    let _ = writeln!(
        svg,
        "<g id=\"divides\"><path d=\"{d}\" fill=\"none\" stroke=\"#1b1b1b\" \
         stroke-width=\"0.30\" stroke-opacity=\"0.85\"/></g>"
    );
    let (ts, tnx, tny) = downsample_bool(inputs.territory, field.nx, field.ny, factor);
    let loops = marching_squares_bool_grid(&ts, tny, tnx, field.ox, field.oy, ds_cell);
    let mut d = String::new();
    for lp in &loops {
        if lp.len() >= 3 {
            d.push_str(&path_from_loop(lp, y_flip, true));
        }
    }
    let _ = writeln!(
        svg,
        "<g id=\"territory\"><path d=\"{d}\" fill=\"none\" fill-rule=\"evenodd\" \
         stroke=\"#0d0d0d\" stroke-width=\"0.55\"/></g>\n</svg>"
    );
    std::fs::write(path, svg)
}

// ═══════════════════════════════════════════════════════════════════════
// The instrument
// ═══════════════════════════════════════════════════════════════════════

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_catchment_basin_census_w0() {
    eprintln!(
        "\n========== Track H W0 — CATCHMENT BASIN CENSUS ==========\n\
         Pre-registration: planning/valley_tracing_2026-09-02/FINDINGS.md, Phase W + W0.\n\
         A CENSUS ONLY — no toolpath generation, no strategy code. NUMBERS ONLY:\n\
         bar W0-a and the seam budget are the orchestrator's.\n"
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
    let cell_area = field.cell * field.cell;

    // ── TERRITORY: every planned region, EVERY band. No Shallow clip. ──
    let regions: Vec<&Polygon2> = planned.regions.iter().map(|r| &r.polygon).collect();
    let bboxes: Vec<[f64; 4]> = regions.iter().map(|p| p.bbox()).collect();
    let territory: Vec<bool> = (0..field.len())
        .into_par_iter()
        .map(|i| {
            if field.nodata[i] {
                return false;
            }
            let p = field.xy(i);
            regions.iter().zip(bboxes.iter()).any(|(poly, bb)| {
                p.x >= bb[0]
                    && p.x <= bb[2]
                    && p.y >= bb[1]
                    && p.y <= bb[3]
                    && poly.contains_point(&p)
            })
        })
        .collect();
    let territory_cells: Vec<usize> = (0..field.len()).filter(|&i| territory[i]).collect();
    let covered_cells = covered.iter().filter(|&&c| c).count();
    let mut band_tally: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for r in &planned.regions {
        *band_tally.entry(format!("{:?}", r.band)).or_default() += 1;
    }
    eprintln!(
        "mesh {} triangles; classification grid {} x {} at {:.4} mm; spec stepover \
         {stepover:.6} mm  ({:.1}s)\n\
         TERRITORY = the union of ALL {} planned regions, every band (no Shallow clip):\n\
         \x20  {} cells = {:.1} mm² XY;  the decomposition's own covered mask is {} cells \
         = {:.1} mm²\n\
         \x20  regions per band: {:?}",
        mesh.triangles.len(),
        field.nx,
        field.ny,
        field.cell,
        started.elapsed().as_secs_f64(),
        planned.regions.len(),
        territory_cells.len(),
        territory_cells.len() as f64 * cell_area,
        covered_cells,
        covered_cells as f64 * cell_area,
        band_tally,
    );

    // ── hydrology, on the land view (V0's correction) ──
    let hydro = field.land_view(LAND_Z_MM);
    let filled = priority_flood_epsilon(&hydro);
    let receivers = d8_receivers(&hydro, &filled);
    let acc = d8_accumulation(&hydro, &filled, &receivers);
    let land_cells = (0..hydro.len()).filter(|&i| !hydro.nodata[i]).count();
    let territory_off_land = territory_cells.iter().filter(|&&i| hydro.nodata[i]).count();
    eprintln!(
        "hydrology on the LAND view: {land_cells} land cells; {} territory cells sit BELOW the \
         land floor (z <= {LAND_Z_MM}) and can carry no basin label.",
        territory_off_land
    );

    let slope_deg: Vec<f64> = surface
        .slope_map
        .angles
        .iter()
        .map(|a| a.to_degrees())
        .collect();
    let ctx = ShapeCtx {
        field: &field,
        territory: &territory,
        slope_deg: &slope_deg,
        cell_area,
    };
    let hy = Hydrology {
        hydro: &hydro,
        filled: &filled,
        receivers: &receivers,
        acc: &acc,
    };

    // ── the pre-registered rung, then the two neighbours ──
    let mut results: Vec<RungResult> = Vec::new();
    for &t in &TRUNK_SENSITIVITY_MM2 {
        let t0 = std::time::Instant::now();
        let r = run_rung(&hy, &ctx, &territory_cells, t);
        eprintln!(
            "\n---------- trunk rung T = {t:.0} mm²  ({:.1}s) ----------\n\
             \x20  trunk cells {} ({} junctions); basins before merge {} = {} trunk LINKS + \
             {} land outlets;\n\
             \x20    after merge {} ({} merged, {} could not merge — no downstream \
             neighbour)\n\
             \x20  coastal basins (reach a LAND OUTLET without meeting a trunk — the case the\n\
             \x20    pre-registration did not name): {} covering {:.1} mm² = {:.2} % of territory\n\
             \x20  divide length (staircase) {:.1} mm;  territory in basins {:.1} mm²",
            t0.elapsed().as_secs_f64(),
            r.trunk_cells,
            r.junctions,
            r.basins_before_merge,
            r.link_basins,
            r.outlet_basins,
            r.basins_after_merge,
            r.merges,
            r.stranded,
            r.coastal_basins,
            r.coastal_area_mm2,
            100.0 * r.coastal_area_mm2 / r.territory_area_mm2.max(1e-9),
            r.divide_mm,
            r.territory_area_mm2,
        );
        eprintln!(
            "\x20  W0-a input: {:.1} mm² of {:.1} mm² = {:.2} % of territory area is in basins \
             that are\n\
             \x20    SIMPLY CONNECTED (1 loop, 1 component) AND PCA aspect <= \
             {COMPACT_ASPECT_MAX:.1}",
            r.compact_area_mm2,
            r.territory_area_mm2,
            100.0 * r.compact_area_mm2 / r.territory_area_mm2.max(1e-9),
        );
        eprintln!(
            "\x20  SEAM PREDICTOR INPUTS: N_basins = {}, L_divide = {:.1} mm\n\
             \x20    closed-ring model  {SEAM_JUNCTION_S} s x 2 x N = {:.1} s\n\
             \x20    open-pass model    {SEAM_JUNCTION_S} s x L/{SPEC_STEPOVER_MM:.6} mm = {:.1} s",
            r.basins_after_merge,
            r.divide_mm,
            SEAM_JUNCTION_S * 2.0 * r.basins_after_merge as f64,
            SEAM_JUNCTION_S * r.divide_mm / SPEC_STEPOVER_MM,
        );
        results.push(r);
    }

    // ── per-basin table at the pre-registered rung ──
    let Some(main) = results
        .iter()
        .find(|r| (r.trunk_t_mm2 - TRUNK_T_MM2).abs() < 1e-9)
    else {
        eprintln!("SKIP: the pre-registered rung did not run.");
        return;
    };
    eprintln!(
        "\n---------- PER-BASIN, at the pre-registered T = {TRUNK_T_MM2:.0} mm² ----------\n\
         \x20  slope shares are of the basin's 3-D area. `simple` needs 1 loop AND 1 \
         component.\n\
         \x20  `clip` marks a basin the territory boundary cuts — its shape describes the \
         clipped piece.\n"
    );
    eprintln!(
        "     {:>4}  {:>10}  {:>10}  {:>5}  {:>4}  {:>7}  {:>7}  {:>6}  {:>6}  {:>6}  {:>6}  \
         {:>4}  {:>4}",
        "#",
        "XY mm²",
        "3D mm²",
        "loops",
        "cmp",
        "aspect",
        "simple",
        "0-15",
        "15-30",
        "30-45",
        "45+",
        "clip",
        "cst"
    );
    for (n, (_, s)) in main.shapes.iter().enumerate().take(40) {
        let simple = s.loops == 1 && s.components == 1;
        eprintln!(
            "     {:>4}  {:>10.1}  {:>10.1}  {:>5}  {:>4}  {:>7}  {:>7}  {:>5.1}%  {:>5.1}%  \
             {:>5.1}%  {:>5.1}%  {:>4}  {:>4}",
            n + 1,
            s.area_xy_mm2,
            s.area_3d_mm2,
            s.loops,
            s.components,
            if s.aspect.is_finite() {
                format!("{:.2}", s.aspect)
            } else {
                "inf".to_owned()
            },
            if simple { "yes" } else { "NO" },
            100.0 * s.slope_share[0],
            100.0 * s.slope_share[1],
            100.0 * s.slope_share[2],
            100.0 * s.slope_share[3],
            if s.clipped { "yes" } else { "-" },
            if s.coastal { "yes" } else { "-" },
        );
    }
    if main.shapes.len() > 40 {
        eprintln!("     ... {} more basins", main.shapes.len() - 40);
    }
    let simple_count = main
        .shapes
        .iter()
        .filter(|(_, s)| s.loops == 1 && s.components == 1)
        .count();
    let clipped_count = main.shapes.iter().filter(|(_, s)| s.clipped).count();
    let mut aspects: Vec<f64> = main
        .shapes
        .iter()
        .map(|(_, s)| s.aspect)
        .filter(|a| a.is_finite())
        .collect();
    aspects.sort_by(f64::total_cmp);
    let mut band_total = [0.0f64; 4];
    let mut total_3d = 0.0f64;
    for (_, s) in &main.shapes {
        for (b, slot) in band_total.iter_mut().enumerate() {
            *slot += s.slope_share[b] * s.area_3d_mm2;
        }
        total_3d += s.area_3d_mm2;
    }
    eprintln!(
        "\n     totals at T = {TRUNK_T_MM2:.0}: {} basins, {simple_count} simply connected, \
         {clipped_count} clipped by the territory boundary;\n\
         \x20    PCA aspect min {:.2} p50 {:.2} p90 {:.2} max {:.2};\n\
         \x20    territory 3-D area {:.1} mm², slope mix {:.1}% / {:.1}% / {:.1}% / {:.1}% \
         (0-15 / 15-30 / 30-45 / 45+)",
        main.shapes.len(),
        aspects.first().copied().unwrap_or(f64::NAN),
        aspects[aspects.len() / 2],
        aspects[(aspects.len() as f64 * 0.9) as usize % aspects.len().max(1)],
        aspects.last().copied().unwrap_or(f64::NAN),
        total_3d,
        100.0 * band_total[0] / total_3d.max(1e-9),
        100.0 * band_total[1] / total_3d.max(1e-9),
        100.0 * band_total[2] / total_3d.max(1e-9),
        100.0 * band_total[3] / total_3d.max(1e-9),
    );

    eprintln!(
        "\n---------- SENSITIVITY: the aggregates at all three rungs ----------\n\
         \x20  {:>10}  {:>9}  {:>9}  {:>12}  {:>13}  {:>12}  {:>12}",
        "T mm²",
        "basins",
        "coastal",
        "L_divide mm",
        "compact area %",
        "ring model s",
        "pass model s"
    );
    for r in &results {
        eprintln!(
            "\x20  {:>10.0}  {:>9}  {:>9}  {:>12.1}  {:>12.2} %  {:>12.1}  {:>12.1}",
            r.trunk_t_mm2,
            r.basins_after_merge,
            r.coastal_basins,
            r.divide_mm,
            100.0 * r.compact_area_mm2 / r.territory_area_mm2.max(1e-9),
            SEAM_JUNCTION_S * 2.0 * r.basins_after_merge as f64,
            SEAM_JUNCTION_S * r.divide_mm / SPEC_STEPOVER_MM,
        );
    }

    // ── the VISUAL ──
    let trunk: Vec<bool> = (0..hydro.len())
        .map(|i| !hydro.nodata[i] && acc[i] * cell_area >= TRUNK_T_MM2)
        .collect();
    let ws = label_basins(&hydro, &filled, &receivers, &trunk);
    let (remap, _, _) = merge_small_basins(&ws, &receivers, &territory_cells, cell_area);
    let label: Vec<u32> = ws
        .label
        .iter()
        .map(|&b| {
            if b == NO_BASIN {
                NO_BASIN
            } else {
                remap[b as usize]
            }
        })
        .collect();
    let ordered: Vec<u32> = main.shapes.iter().map(|(b, _)| *b).collect();
    let mut divides = vec![false; field.len()];
    for r in 0..field.ny {
        for c in 0..field.nx {
            let i = r * field.nx + c;
            if !territory[i] {
                continue;
            }
            let mut edge = false;
            if c + 1 < field.nx && territory[i + 1] && label[i] != label[i + 1] {
                edge = true;
            }
            if r + 1 < field.ny && territory[i + field.nx] && label[i] != label[i + field.nx] {
                edge = true;
            }
            divides[i] = edge;
        }
    }
    let dir = svg_output_dir();
    let path = dir.join("wanaka_basins_w0.svg");
    let inputs = BasinSvg {
        field: &field,
        territory: &territory,
        label: &label,
        ordered: &ordered,
        divides: &divides,
    };
    match write_basin_svg(
        &inputs,
        &path,
        "Track H W0 — catchment basins on the full wanaka front finish territory",
    ) {
        Ok(()) => eprintln!("\nSVG: {} ({} basins drawn)", path.display(), ordered.len()),
        Err(e) => eprintln!("\nSVG write FAILED: {e}"),
    }

    eprintln!(
        "\n========== END — total {:.1}s. No verdict is written here. ==========\n",
        started.elapsed().as_secs_f64()
    );
}
