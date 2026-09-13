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
//!   outlet. They are counted and reported SEPARATELY as `coastal`.
//! * **Merge, stage 1 (W0):** any basin whose territory XY area is under
//!   [`MIN_BASIN_AREA_MM2`] is merged into the basin containing its outlet
//!   cell's receiver — its downstream neighbour — smallest first.
//! * **Merge, stage 2 (W0b):** any COASTAL basin still under the floor merges
//!   into **the neighbour it shares the longest divide with**, repeated to
//!   fixpoint, so the map holds only trunk-keyed catchments plus coastal
//!   segments at or above the floor. The rule is the pre-registered one; the
//!   instrument implements it, it does not choose it.
//!
//! # W0b — the corrected base level (this file now runs W0b)
//!
//! **The defect W0b fixes.** W0's land view was a GLOBAL threshold, `z > 0`.
//! That treats an interior LAKE exactly like the sea: a hole in the world.
//! Ground draining into a lake then died at a fake lake-edge outlet, and the
//! lake bed itself carried no label at all — which is most of the confetti in
//! the W0 map, ringing lakes as well as the coast.
//!
//! **The correction, per the operator's geographic truth:** a lake fills and
//! overflows, so everything drains to the sea and each lake belongs to
//! exactly one catchment. So the base level is **only water CONNECTED TO THE
//! BOARD BORDER** — the sea plus the coastline trench band — found by a
//! 4-connected flood fill inward from the border across cells at or below the
//! water level, never by a global threshold. Everything the fill does not
//! reach is land, lake beds included, and the priority flood then fills each
//! lake as an ordinary depression so flow continues through it to the sea.
//!
//! **The water level is not guessed.** `rivmap_data.toml` pins
//! `base_height_mm = 0.0`; the wave trench is cut below it
//! (`[mesh.waves] offset 0.9, depth 2.0`) and the mesh bottoms out at
//! z = −2.81. So [`WATER_LEVEL_MM`] is 0.0 — the same LEVEL W0 used. Only the
//! CONNECTIVITY requirement is new, and that is the whole defect.
//!
//! The count of cells at or below the water level that the fill does NOT
//! reach — the interior lake beds recovered as land — is printed, because it
//! is the direct measure of what the correction changed.
//!
//! # W0b, second correction — FLAT RESOLUTION
//!
//! **The second defect the operator caught.** The drainage network broke on
//! flat ground: lines stopped at pale flat valley floors and restarted
//! downstream. The `+epsilon` priority flood gives a gradient only to cells
//! it RAISES — the inside of a depression. A cell on a NATURAL flat keeps its
//! own height, so a run of equal-height cells has no strictly-lower
//! neighbour, `d8_receivers` returns `None`, and routing dies there: the
//! trunk is chopped at every flat and each stall becomes a spurious outlet
//! basin. That is a plausible cause of both the 18-trunk-link oddity and part
//! of the confetti.
//!
//! **The fix: Garbrecht–Martz (1997) combined flat gradient**, implemented
//! with the two breadth-first sweeps of Barnes, Lehman & Soille (2014). For
//! each flat — a connected group of equal-height cells containing at least one
//! cell with no lower neighbour:
//!
//! ```text
//!   d_low  = BFS distance from the flat's LOW edge  (cells beside lower ground)
//!   d_high = BFS distance from the flat's HIGH edge (cells beside higher ground)
//!   increment = (max d_low - d_low) + d_high
//!   z += FLAT_EPSILON_MM * increment
//! ```
//!
//! The `max d_low - d_low` term drains the flat toward its spill edge; the
//! `d_high` term pushes flow away from the surrounding high ground, which is
//! what makes flow across a wide flat run parallel rather than fan radially
//! onto the spill point. Both terms are Garbrecht–Martz; taking only the first
//! would restore continuity but would place the trunk badly inside a lake,
//! and the map is meant to be read.
//!
//! Coastal cells adjacent to the sea, and cells on the grid border, are NOT
//! treated as flats: they drain off the board and are legitimate outlets.
//!
//! `FLAT_EPSILON_MM` is 1e-6 mm and the total imposed rise is capped at
//! `FLAT_MAX_RISE_MM`, so flat resolution can never reorder real terrain.
//! **Trunk-link counts are reported at every rung BEFORE and AFTER flat
//! resolution**, so the correction's effect is measured rather than asserted.
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
//!   wanaka_catchment_basin_census_w0b -- --ignored --nocapture
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

use std::collections::VecDeque;
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

/// Water level (mm). `base_height_mm = 0.0` in `rivmap_data.toml`; the wave
/// trench is cut below it and the mesh bottoms out at −2.81. Only water
/// CONNECTED TO THE BORDER at or below this level is base level — see the
/// header.
const WATER_LEVEL_MM: f64 = 0.0;

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
use rs_cam_core::flow_accum::{
    FILL_EPSILON_MM, FLAT_EPSILON_MM, FLAT_MAX_RISE_MM, FlowField as Field, d8_accumulation,
    d8_receivers, priority_flood_epsilon, resolve_flats,
};

/// File-local geometry and the border-connected sea mask on the shared
/// [`Field`]. The rasterised heightfield and its D8 hydrology now live in
/// `rs_cam_core::flow_accum` (promoted from three verbatim copies); this census
/// keeps these helpers local so the call sites below do not change.
trait FieldExt {
    fn index_at(&self, x: f64, y: f64) -> Option<usize>;
    fn xy(&self, i: usize) -> P2;
    fn land_view_border_connected(&self, water_level: f64) -> (Field, SeaReport);
}

impl FieldExt for Field {
    fn index_at(&self, x: f64, y: f64) -> Option<usize> {
        let col = ((x - self.ox) / self.cell).round();
        let row = ((y - self.oy) / self.cell).round();
        if col < 0.0 || row < 0.0 {
            return None;
        }
        let (col, row) = (col as usize, row as usize);
        (col < self.nx && row < self.ny).then_some(row * self.nx + col)
    }

    fn xy(&self, i: usize) -> P2 {
        let row = i / self.nx;
        let col = i % self.nx;
        P2::new(
            self.ox + col as f64 * self.cell,
            self.oy + row as f64 * self.cell,
        )
    }

    /// The LAND view: everything except water CONNECTED TO THE BOARD BORDER at
    /// or below `water_level`. The sea is found by a 4-connected flood fill
    /// inward from the grid border, never by a global `z <= level` test, so an
    /// interior lake that merely touches the trench at a corner is not drained.
    /// Returns the view plus a [`SeaReport`]. See the file header for why the
    /// whole-board flood is restricted to land.
    fn land_view_border_connected(&self, water_level: f64) -> (Field, SeaReport) {
        let n = self.len();
        let is_water = |i: usize| -> bool { !self.nodata[i] && self.z[i] <= water_level };
        let mut comp = vec![u32::MAX; n];
        let mut sizes: Vec<usize> = Vec::new();
        let mut touches_border: Vec<bool> = Vec::new();
        let mut queue: VecDeque<usize> = VecDeque::new();
        for start in 0..n {
            if !is_water(start) || comp[start] != u32::MAX {
                continue;
            }
            let id = sizes.len() as u32;
            let mut size = 0usize;
            let mut border = false;
            comp[start] = id;
            queue.push_back(start);
            while let Some(c) = queue.pop_front() {
                size += 1;
                let (r, col) = (c / self.nx, c % self.nx);
                if r == 0 || col == 0 || r + 1 == self.ny || col + 1 == self.nx {
                    border = true;
                }
                let visit = |j: usize, comp: &mut Vec<u32>, q: &mut VecDeque<usize>| {
                    if is_water(j) && comp[j] == u32::MAX {
                        comp[j] = id;
                        q.push_back(j);
                    }
                };
                if col > 0 {
                    visit(c - 1, &mut comp, &mut queue);
                }
                if col + 1 < self.nx {
                    visit(c + 1, &mut comp, &mut queue);
                }
                if r > 0 {
                    visit(c - self.nx, &mut comp, &mut queue);
                }
                if r + 1 < self.ny {
                    visit(c + self.nx, &mut comp, &mut queue);
                }
            }
            sizes.push(size);
            touches_border.push(border);
        }
        let border_cells: usize = (0..sizes.len())
            .filter(|&k| touches_border[k])
            .map(|k| sizes[k])
            .sum();
        let largest = (0..sizes.len()).max_by_key(|&k| sizes[k]);
        let used_fallback = border_cells == 0;
        let sea: Vec<bool> = (0..n)
            .map(|i| {
                let Some(k) = (comp[i] != u32::MAX).then(|| comp[i] as usize) else {
                    return false;
                };
                if used_fallback {
                    Some(k) == largest
                } else {
                    touches_border[k]
                }
            })
            .collect();
        let mut ranked: Vec<(usize, usize, bool)> = (0..sizes.len())
            .map(|k| (sizes[k], k, touches_border[k]))
            .collect();
        ranked.sort_by_key(|a| std::cmp::Reverse(a.0));
        ranked.truncate(6);
        let report = SeaReport {
            bodies: sizes.len(),
            border_connected_cells: border_cells,
            used_largest_body_fallback: used_fallback,
            sea_cells: sea.iter().filter(|&&w| w).count(),
            interior_water_cells: (0..n).filter(|&i| is_water(i)).count()
                - sea.iter().filter(|&&w| w).count(),
            largest_bodies: ranked,
        };
        let nodata = (0..n).map(|i| self.nodata[i] || sea[i]).collect();
        (
            Field {
                nx: self.nx,
                ny: self.ny,
                ox: self.ox,
                oy: self.oy,
                cell: self.cell,
                z: self.z.clone(),
                nodata,
            },
            report,
        )
    }
}

/// What the water census found — printed in full, because which body is "the
/// sea" is the load-bearing choice of the whole W0b correction.
struct SeaReport {
    bodies: usize,
    /// Cells in bodies that touch the grid border — the PRE-REGISTERED rule's
    /// answer. Zero on this board: the perimeter is the raised machining rim,
    /// not water.
    border_connected_cells: usize,
    /// True when the pre-registered rule seeded nothing and the largest water
    /// body was used instead. Reported loudly; it is NOT the pre-registered
    /// rule.
    used_largest_body_fallback: bool,
    sea_cells: usize,
    interior_water_cells: usize,
    /// `(cells, id, touches_border)` for the six largest bodies.
    largest_bodies: Vec<(usize, usize, bool)>,
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

/// **W0b stage 2, the pre-registered coastal merge.** Every COASTAL basin
/// still under [`MIN_BASIN_AREA_MM2`] merges into the neighbour it shares the
/// longest divide with, repeated to fixpoint, so the map holds only
/// trunk-keyed catchments plus coastal segments at or above the floor.
///
/// Adjacency is measured over the whole LAND domain, not the territory clip:
/// a basin is a landform, and a micro-basin whose only neighbour lies outside
/// the territory would otherwise have nowhere to go. Shared 4-neighbour cell
/// edges are the divide measure, the same staircase convention
/// [`divide_length_mm`] reports.
///
/// Smallest-first each round, and ties on divide length break on the lower
/// basin id, so the result does not depend on scan order.
fn merge_coastal_micro_basins(
    field: &Field,
    label: &[u32],
    coastal: &[bool],
    territory: &[bool],
    cell_area: f64,
    basin_count: usize,
) -> (Vec<u32>, usize, usize) {
    use std::collections::BTreeMap;

    let mut parent: Vec<u32> = (0..basin_count as u32).collect();
    fn find(parent: &mut [u32], mut b: u32) -> u32 {
        while parent[b as usize] != b {
            let grand = parent[parent[b as usize] as usize];
            parent[b as usize] = grand;
            b = grand;
        }
        b
    }

    // Shared-edge counts between distinct basins, over the land domain.
    let mut adjacency: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    let bump = |a: u32, b: u32, adjacency: &mut BTreeMap<(u32, u32), usize>| {
        if a != b && a != NO_BASIN && b != NO_BASIN {
            let key = if a < b { (a, b) } else { (b, a) };
            *adjacency.entry(key).or_insert(0) += 1;
        }
    };
    for r in 0..field.ny {
        for c in 0..field.nx {
            let i = r * field.nx + c;
            if label[i] == NO_BASIN {
                continue;
            }
            if c + 1 < field.nx {
                bump(label[i], label[i + 1], &mut adjacency);
            }
            if r + 1 < field.ny {
                bump(label[i], label[i + field.nx], &mut adjacency);
            }
        }
    }
    // Per-basin neighbour lists, kept as (neighbour, shared edges).
    let mut neighbours: Vec<BTreeMap<u32, usize>> = vec![BTreeMap::new(); basin_count];
    for (&(a, b), &w) in &adjacency {
        *neighbours[a as usize].entry(b).or_insert(0) += w;
        *neighbours[b as usize].entry(a).or_insert(0) += w;
    }
    // Territory area is the floor's measure, matching W0 stage 1.
    let mut area = vec![0.0f64; basin_count];
    for i in 0..field.len() {
        if territory[i] && label[i] != NO_BASIN {
            area[label[i] as usize] += cell_area;
        }
    }
    let mut is_coastal: Vec<bool> = coastal.to_vec();

    let mut merged = 0usize;
    let mut stranded = 0usize;
    loop {
        // The smallest coastal root still under the floor.
        let mut pick: Option<(f64, u32)> = None;
        for b in 0..basin_count as u32 {
            if find(&mut parent, b) != b || !is_coastal[b as usize] {
                continue;
            }
            if area[b as usize] >= MIN_BASIN_AREA_MM2 {
                continue;
            }
            if pick.is_none_or(|(a, _)| area[b as usize] < a) {
                pick = Some((area[b as usize], b));
            }
        }
        let Some((_, root)) = pick else { break };
        // Its longest-divide neighbour, ties on the lower id.
        let mut best: Option<(usize, u32)> = None;
        let candidates: Vec<(u32, usize)> = neighbours[root as usize]
            .iter()
            .map(|(&k, &v)| (k, v))
            .collect();
        for (nb, w) in candidates {
            let target = find(&mut parent, nb);
            if target == root {
                continue;
            }
            if best.is_none_or(|(bw, bt)| w > bw || (w == bw && target < bt)) {
                best = Some((w, target));
            }
        }
        let Some((_, target)) = best else {
            // No neighbour at all: it cannot merge, and leaving it is honest.
            is_coastal[root as usize] = false;
            stranded += 1;
            continue;
        };
        parent[root as usize] = target;
        let carried = area[root as usize];
        area[target as usize] += carried;
        area[root as usize] = 0.0;
        // A union with a trunk-keyed basin is trunk-keyed.
        let both_coastal = is_coastal[target as usize] && is_coastal[root as usize];
        is_coastal[target as usize] = both_coastal;
        let moved: Vec<(u32, usize)> = neighbours[root as usize]
            .iter()
            .map(|(&k, &v)| (k, v))
            .collect();
        for (nb, w) in moved {
            if find(&mut parent, nb) == target {
                continue;
            }
            *neighbours[target as usize].entry(nb).or_insert(0) += w;
        }
        neighbours[root as usize].clear();
        merged += 1;
    }
    let flat: Vec<u32> = (0..basin_count as u32)
        .map(|b| find(&mut parent, b))
        .collect();
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
    coastal_merges: usize,
    coastal_stranded: usize,
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
    // Stage 1 (W0): downstream merge under the floor.
    let (remap1, merges, stranded) =
        merge_small_basins(&ws, receivers, territory_cells, ctx.cell_area);
    let staged: Vec<u32> = ws
        .label
        .iter()
        .map(|&b| {
            if b == NO_BASIN {
                NO_BASIN
            } else {
                remap1[b as usize]
            }
        })
        .collect();
    // Stage 2 (W0b): coastal micro-basins merge into their longest-divide
    // neighbour, to fixpoint. A stage-1 group is coastal only when every
    // constituent was.
    let staged_coastal: Vec<bool> = (0..ws.outlet.len())
        .map(|b| {
            let mut any = false;
            let mut all = true;
            for (k, &was_coastal) in ws.coastal.iter().enumerate() {
                if remap1[k] as usize == b {
                    any = true;
                    all &= was_coastal;
                }
            }
            any && all
        })
        .collect();
    let (remap2, coastal_merges, coastal_stranded) = merge_coastal_micro_basins(
        field,
        &staged,
        &staged_coastal,
        ctx.territory,
        ctx.cell_area,
        ws.outlet.len(),
    );
    let label: Vec<u32> = staged
        .iter()
        .map(|&b| {
            if b == NO_BASIN {
                NO_BASIN
            } else {
                remap2[b as usize]
            }
        })
        .collect();
    let remap: Vec<u32> = (0..ws.outlet.len())
        .map(|k| remap2[remap1[k] as usize])
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
        coastal_merges,
        coastal_stranded,
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

/// Trace the trunk network head-to-outlet into polylines — the V0 census's
/// valley-lines layer, reused so the two maps draw the SAME object.
///
/// Drawn over the basin map, this is the operator's visual check: one tree
/// wholly inside one basin colour means the decomposition follows the
/// drainage, and a stream crossing a divide mid-run is a labelling defect.
fn drainage_polylines(
    field: &Field,
    receivers: &[Option<u32>],
    acc: &[f64],
    network: &[bool],
) -> Vec<Vec<P2>> {
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

struct BasinSvg<'a> {
    field: &'a Field,
    territory: &'a [bool],
    label: &'a [u32],
    ordered: &'a [u32],
    divides: &'a [bool],
    drainage: &'a [Vec<P2>],
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
    // The extracted drainage network: a light halo so it reads over any basin
    // colour, then navy on top.
    let mut dd = String::new();
    for line in inputs.drainage {
        if line.len() >= 2 {
            dd.push_str(&path_from_loop(line, y_flip, false));
        }
    }
    let _ = writeln!(
        svg,
        "<g id=\"drainage\" fill=\"none\" stroke-linecap=\"round\" \
         stroke-linejoin=\"round\">\n\
         <path d=\"{dd}\" stroke=\"#fbfbf8\" stroke-width=\"1.20\" \
         stroke-opacity=\"0.95\"/>\n\
         <path d=\"{dd}\" stroke=\"#0b2a63\" stroke-width=\"0.62\"/></g>"
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
fn wanaka_catchment_basin_census_w0b() {
    eprintln!(
        "\n========== Track H W0b — CATCHMENT BASIN CENSUS (lake- and flat-corrected) ==========\n\
         Pre-registration: planning/valley_tracing_2026-09-02/FINDINGS.md, Phase W + W0 + W0b.\n\
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

    // ── W0b hydrology: base level is BORDER-CONNECTED water only ──
    let (hydro, sea) = field.land_view_border_connected(WATER_LEVEL_MM);
    let (sea_cells, interior_water_cells) = (sea.sea_cells, sea.interior_water_cells);
    if sea.used_largest_body_fallback {
        eprintln!(
            "\n***** PRE-REGISTERED RULE SEEDED NOTHING — READ THIS BEFORE ANY NUMBER *****\n\
             \x20  W0b says: base level = water CONNECTED TO THE BOARD BORDER. On this board\n\
             \x20  NO water body touches the grid border: the perimeter is the raised MACHINING\n\
             \x20  RIM (`edge_profile = 3`, `edge_wall_deg = 41` in rivmap_data.toml), and the\n\
             \x20  coastline trench sits INSIDE it. Border-connected water = {} cells.\n\
             \x20  Every census number below therefore uses the LARGEST water body as the sea,\n\
             \x20  which is an ALTERNATIVE this agent did not invent a justification for and\n\
             \x20  the orchestrator has not pre-registered. Rule on the water census first.\n\
             ****************************************************************************",
            sea.border_connected_cells
        );
    }
    eprintln!(
        "\nWATER CENSUS at or below {WATER_LEVEL_MM} mm — {} separate bodies; six largest:",
        sea.bodies
    );
    for (cells, id, border) in &sea.largest_bodies {
        eprintln!(
            "\x20  body {id:>5}: {cells:>8} cells = {:>9.1} mm²   touches grid border: {}",
            *cells as f64 * cell_area,
            if *border { "YES" } else { "no" }
        );
    }
    let raw_filled = priority_flood_epsilon(&hydro);
    // BEFORE flat resolution — kept so the correction's effect is measured.
    let raw_receivers = d8_receivers(&hydro, &raw_filled);
    let raw_acc = d8_accumulation(&hydro, &raw_filled, &raw_receivers);
    let raw_stalls = (0..hydro.len())
        .filter(|&i| !hydro.nodata[i] && raw_receivers[i].is_none())
        .count();
    // AFTER: Garbrecht–Martz combined flat gradient.
    let (filled, flats, flat_cells, max_flat_inc) = resolve_flats(&hydro, &raw_filled);
    let receivers = d8_receivers(&hydro, &filled);
    let acc = d8_accumulation(&hydro, &filled, &receivers);
    let stalls = (0..hydro.len())
        .filter(|&i| !hydro.nodata[i] && receivers[i].is_none())
        .count();
    let land_cells = (0..hydro.len()).filter(|&i| !hydro.nodata[i]).count();
    let raised = (0..hydro.len())
        .filter(|&i| !hydro.nodata[i] && filled[i] > field.z[i] + FILL_EPSILON_MM)
        .count();
    let territory_off_land = territory_cells.iter().filter(|&&i| hydro.nodata[i]).count();
    eprintln!(
        "W0b hydrology — base level is BORDER-CONNECTED water only (4-connected flood fill \
         inward from\n\
         \x20  the grid border, at or below WATER_LEVEL = {WATER_LEVEL_MM} mm = \
         `base_height_mm` in rivmap_data.toml):\n\
         \x20  sea cells {sea_cells};  land cells {land_cells}\n\
         \x20  INTERIOR water at or below the level the fill did NOT reach — the lake beds W0's\n\
         \x20    global threshold discarded and W0b recovers as LAND: {interior_water_cells} \
         cells = {:.1} mm²\n\
         \x20  priority flood raised {raised} cells ({:.2} % of land): lakes now fill as \
         depressions and drain through\n\
         \x20  territory cells still unlabelled (true sea inside a territory polygon): {} \
         = {:.2} % of territory",
        interior_water_cells as f64 * cell_area,
        100.0 * raised as f64 / land_cells.max(1) as f64,
        territory_off_land,
        100.0 * territory_off_land as f64 / territory_cells.len().max(1) as f64,
    );
    eprintln!(
        "W0b FLAT RESOLUTION (Garbrecht–Martz combined gradient, two-BFS after Barnes 2014):\n\
         \x20  {flats} flats covering {flat_cells} cells = {:.1} mm²; largest BFS increment \
         {max_flat_inc} rings\n\
         \x20    -> max imposed rise {:.6} mm (cap {FLAT_MAX_RISE_MM}), against ~9.8 mm of \
         board relief\n\
         \x20  land cells where routing STALLED (no receiver): {raw_stalls} BEFORE -> {stalls} \
         AFTER  ({} recovered)",
        flat_cells as f64 * cell_area,
        (FLAT_EPSILON_MM * max_flat_inc as f64).min(FLAT_MAX_RISE_MM),
        raw_stalls.saturating_sub(stalls),
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
        // The same link decomposition on the PRE-flat-resolution surface, so
        // the correction's effect on trunk continuity is measured, not claimed.
        let raw_trunk: Vec<bool> = (0..hydro.len())
            .map(|i| !hydro.nodata[i] && raw_acc[i] * cell_area >= t)
            .collect();
        let raw_ws = label_basins(&hydro, &raw_filled, &raw_receivers, &raw_trunk);
        let r = run_rung(&hy, &ctx, &territory_cells, t);
        eprintln!(
            "\n     TRUNK LINKS at T = {t:.0} mm²: {} BEFORE flat resolution -> {} AFTER \
             ({} trunk cells before, {} after)",
            raw_ws.link_basins, r.link_basins, raw_ws.trunk_cells, r.trunk_cells,
        );
        eprintln!(
            "\n---------- trunk rung T = {t:.0} mm²  ({:.1}s) ----------\n\
             \x20  trunk cells {} ({} junctions); basins before merge {} = {} trunk LINKS + \
             {} land outlets;\n\
             \x20    after merge {} (stage 1 downstream: {} merged, {} had no downstream \
             neighbour;\n\
             \x20      stage 2 coastal longest-divide: {} merged, {} had no neighbour at \
             all)\n\
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
            r.coastal_merges,
            r.coastal_stranded,
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
        // DIAGNOSTIC SPLIT, not the bar. The pre-registration keys basins on
        // TRUNK outlets; the coastal class is the case it did not name, and on
        // this territory it dominates. Reporting W0-a's ingredients over each
        // class separately is what makes that dominance actionable — it is a
        // split of an already-computed population, and it chooses nothing.
        let mut trunk_area = 0.0f64;
        let mut trunk_compact = 0.0f64;
        let mut trunk_n = 0usize;
        let mut coast_area = 0.0f64;
        let mut coast_compact = 0.0f64;
        for (_, sh) in &r.shapes {
            let compact = sh.loops == 1 && sh.components == 1 && sh.aspect <= COMPACT_ASPECT_MAX;
            if sh.coastal {
                coast_area += sh.area_xy_mm2;
                if compact {
                    coast_compact += sh.area_xy_mm2;
                }
            } else {
                trunk_n += 1;
                trunk_area += sh.area_xy_mm2;
                if compact {
                    trunk_compact += sh.area_xy_mm2;
                }
            }
        }
        eprintln!(
            "\x20  DIAGNOSTIC SPLIT (not the bar — see the report):\n\
             \x20    TRUNK-keyed basins: {trunk_n} covering {trunk_area:.1} mm²; compact \
             {trunk_compact:.1} mm² = {:.2} % of that class\n\
             \x20    COASTAL basins:     {} covering {coast_area:.1} mm²; compact \
             {coast_compact:.1} mm² = {:.2} % of that class",
            100.0 * trunk_compact / trunk_area.max(1e-9),
            r.shapes.len() - trunk_n,
            100.0 * coast_compact / coast_area.max(1e-9),
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
    // ── the operator's visual check, MEASURED ──
    //
    // A drainage polyline segment whose two ends sit in DIFFERENT basins is a
    // stream crossing a divide. A few are expected and correct: a trunk LINK
    // ends where the next link begins, so every junction is one legitimate
    // crossing. A large count would mean the labelling does not follow the
    // drainage, which is exactly what the eye is being asked to check.
    let drainage_probe = drainage_polylines(&field, &receivers, &acc, &trunk);
    let mut segments = 0usize;
    let mut crossings = 0usize;
    for line in &drainage_probe {
        for w in line.windows(2) {
            let (Some(a), Some(b)) = (
                field.index_at(w[0].x, w[0].y),
                field.index_at(w[1].x, w[1].y),
            ) else {
                continue;
            };
            if !territory[a] || !territory[b] {
                continue;
            }
            segments += 1;
            if label[a] != label[b] {
                crossings += 1;
            }
        }
    }
    eprintln!(
        "\n---------- drainage vs divides (the visual check, measured) ----------\n\
         \x20  {} in-territory drainage segments; {crossings} cross a basin divide \
         ({:.3} %).\n\
         \x20  A trunk LINK ends where the next begins, so one crossing per junction is \
         correct;\n\
         \x20  a large share would mean the labelling does not follow the drainage.",
        segments,
        100.0 * crossings as f64 / segments.max(1) as f64,
    );

    // ── avenue F: the spacing-variation prize field ──
    //
    // `p(x) = s_max(spec h) · cos(theta(x))` — the locally allowed XY pass
    // pitch. With a fixed tool and a fixed scallop, `s_max` has no spatial
    // term, so all the variation is the slope's, which is the only spatial
    // field this census holds. REPORTED, no bar: it gates whether a
    // variable-spacing (Eikonal-weighted) ring candidate is worth
    // pre-registering at all.
    //
    // `mean/min` is reported twice: against the true minimum, and against the
    // 1st percentile. A single near-vertical cell drives `min` to ~0 and the
    // raw ratio to infinity, so the p1 form is the one that survives an
    // outlier; both are named rather than one being chosen.
    let pick = |v: &[f64], q: f64| -> f64 {
        if v.is_empty() {
            f64::NAN
        } else {
            v[(((v.len() - 1) as f64) * q).round() as usize]
        }
    };
    let spacing_row = |cells: &[usize]| -> Option<(f64, f64, f64, f64, f64, f64, f64)> {
        let mut p: Vec<f64> = cells
            .iter()
            .filter_map(|&i| slope_deg.get(i).map(|d| stepover * d.to_radians().cos()))
            .filter(|v| v.is_finite())
            .collect();
        if p.is_empty() {
            return None;
        }
        p.sort_by(f64::total_cmp);
        let mean = p.iter().sum::<f64>() / p.len() as f64;
        let min = p[0];
        let p1 = pick(&p, 0.01);
        Some((
            mean,
            min,
            p1,
            pick(&p, 0.10),
            pick(&p, 0.50),
            pick(&p, 0.90),
            mean / p1.max(1e-9),
        ))
    };
    eprintln!(
        "\n---------- AVENUE F: spacing-variation prize field ----------\n\
         \x20  p(x) = s_max · cos(theta(x)), s_max = {stepover:.6} mm. REPORTED, NO BAR.\n\
         \x20  `mean/min` uses the true minimum; `mean/p1` guards it at the 1st percentile.\n"
    );
    eprintln!(
        "     {:>26}  {:>9}  {:>9}  {:>9}  {:>9}  {:>9}  {:>9}  {:>10}  {:>9}",
        "population", "mean", "min", "p1", "p10", "p50", "p90", "mean/min", "mean/p1"
    );
    let report_spacing = |name: &str, cells: &[usize]| {
        if let Some((mean, min, p1, p10, p50, p90, ratio_p1)) = spacing_row(cells) {
            eprintln!(
                "     {name:>26}  {mean:>9.5}  {min:>9.5}  {p1:>9.5}  {p10:>9.5}  {p50:>9.5}  \
                 {p90:>9.5}  {:>10.3}  {ratio_p1:>9.3}",
                mean / min.max(1e-12)
            );
        } else {
            eprintln!("     {name:>26}  EMPTY POPULATION — nothing measured");
        }
    };
    report_spacing("WHOLE TERRITORY", &territory_cells);
    for (n, (b, _)) in main.shapes.iter().enumerate().take(20) {
        let cells: Vec<usize> = territory_cells
            .iter()
            .copied()
            .filter(|&i| label[i] == *b)
            .collect();
        report_spacing(&format!("basin {}", n + 1), &cells);
    }

    let dir = svg_output_dir();
    let path = dir.join("wanaka_basins_w0b.svg");
    let drainage = drainage_polylines(&field, &receivers, &acc, &trunk);
    let inputs = BasinSvg {
        field: &field,
        territory: &territory,
        label: &label,
        ordered: &ordered,
        divides: &divides,
        drainage: &drainage,
    };
    match write_basin_svg(
        &inputs,
        &path,
        "Track H W0b — lake- and flat-corrected catchment basins, full wanaka territory",
    ) {
        Ok(()) => eprintln!(
            "\nSVG: {} ({} basins, {} drainage polylines drawn)",
            path.display(),
            ordered.len(),
            drainage.len()
        ),
        Err(e) => eprintln!("\nSVG write FAILED: {e}"),
    }

    eprintln!(
        "\n========== END — total {:.1}s. No verdict is written here. ==========\n",
        started.elapsed().as_secs_f64()
    );
}
