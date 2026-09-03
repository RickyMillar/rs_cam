//! D8 flow routing on a rasterised heightfield: priority-flood pit filling,
//! Garbrecht–Martz flat resolution, steepest-descent receivers, and upslope
//! accumulation.
//!
//! # Why this module exists
//!
//! The Track H valley-tracing census built this hydrology three times, inline
//! and byte-identical, across `catchment_basin_census_w0.rs`,
//! `valley_prize_census_h0.rs`, and `valley_branch_falsifier_h1.rs`. The
//! pencil watershed-spine experiment (`planning/pencil_watershed_spine_2026-09-03/`)
//! needs the same primitives from a fourth site, so this promotes the shared
//! kernel to one place. The three census files consume it through a local
//! extension trait, so their call sites do not change.
//!
//! # References
//!
//! - Priority flood + epsilon: Barnes, Lehman & Soille (2014).
//! - Flat gradient: Garbrecht & Martz (1997), two-BFS form.
//!
//! # Indexing
//!
//! Every loop iterates `0..n` where `n == nx * ny == field.len()`, and
//! [`neighbour`] returns only in-range cell indices. The bare index accesses
//! are therefore bounded by construction; the `#[allow(clippy::indexing_slicing)]`
//! attributes carry a SAFETY note for the specific invariant.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

/// The strict rise the priority flood adds inside a filled flat — the
/// "+epsilon" of Barnes' priority flood.
pub const FILL_EPSILON_MM: f64 = 1.0e-6;
/// Height step imposed per BFS ring of the Garbrecht–Martz flat gradient.
pub const FLAT_EPSILON_MM: f64 = 1.0e-6;
/// Hard cap on the total rise flat resolution may impose on any cell, so it
/// can never reorder real terrain.
pub const FLAT_MAX_RISE_MM: f64 = 1.0e-3;

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

/// A rasterised heightfield: row-major (`r * nx + c`), `nodata` where nothing
/// verified there is surface. Cell centre at `(ox + c*cell, oy + r*cell)`.
#[derive(Debug, Clone)]
pub struct FlowField {
    pub nx: usize,
    pub ny: usize,
    pub ox: f64,
    pub oy: f64,
    pub cell: f64,
    pub z: Vec<f64>,
    pub nodata: Vec<bool>,
}

impl FlowField {
    /// Cell count (`nx * ny`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.nx * self.ny
    }

    /// True when the field carries no cells.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The D8 neighbour of cell `i` in direction `k` (`0..8`), with its step
/// length in cells (`1.0` orthogonal, `SQRT_2` diagonal). `None` off-grid.
///
/// SAFETY: every caller iterates `k` over `0..8` and `NB8` has 8 entries, so
/// the `NB8[k]` access is bounded.
#[allow(clippy::indexing_slicing)]
#[must_use]
pub fn neighbour(field: &FlowField, i: usize, k: usize) -> Option<(usize, f64)> {
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
/// accumulation dies inside it. The epsilon variant gives every filled cell a
/// strictly-lower path to its outlet. Seeds are the grid border plus every
/// data cell adjacent to nodata — the board's rim and any trench ring are the
/// outlets.
///
/// SAFETY: indices come from `0..n` (`n == field.len()`) or from [`neighbour`],
/// which returns only in-range cells; every `out`/`closed`/`field.*` access is
/// bounded.
#[allow(clippy::indexing_slicing)]
#[must_use]
pub fn priority_flood_epsilon(field: &FlowField) -> Vec<f64> {
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

/// **Garbrecht–Martz (1997) combined flat gradient**, two-BFS form after
/// Barnes, Lehman & Soille (2014).
///
/// Returns the corrected surface plus `(flats, flat_cells, max_increment)`.
/// A cell adjacent to nodata (the sea) or on the grid border is never part of
/// a flat: it drains off the board and is a legitimate outlet.
///
/// SAFETY: `cells`, `d_low`, `d_high` are indexed by positions produced from
/// the flat's own membership BFS; `out`/`filled`/`member` are indexed by cell
/// ids in `0..n`; `neighbour` returns only in-range cells.
#[allow(clippy::indexing_slicing)]
#[must_use]
pub fn resolve_flats(field: &FlowField, filled: &[f64]) -> (Vec<f64>, usize, usize, usize) {
    let n = field.len();
    let drains_off_board = |i: usize| -> bool {
        let (r, c) = (i / field.nx, i % field.nx);
        if r == 0 || c == 0 || r + 1 == field.ny || c + 1 == field.nx {
            return true;
        }
        (0..8).any(|k| neighbour(field, i, k).is_none_or(|(j, _)| field.nodata[j]))
    };
    // A cell with no strictly-lower land neighbour, that does not drain off
    // the board, is where routing currently dies.
    let mut stalls: Vec<usize> = Vec::new();
    for i in 0..n {
        if field.nodata[i] || drains_off_board(i) {
            continue;
        }
        let has_lower = (0..8).any(|k| {
            neighbour(field, i, k).is_some_and(|(j, _)| !field.nodata[j] && filled[j] < filled[i])
        });
        if !has_lower {
            stalls.push(i);
        }
    }

    let mut out = filled.to_vec();
    let mut member = vec![u32::MAX; n];
    let mut flats = 0usize;
    let mut flat_cells = 0usize;
    let mut max_inc = 0usize;
    let mut queue: VecDeque<usize> = VecDeque::new();

    for &seed in &stalls {
        if member[seed] != u32::MAX {
            continue;
        }
        // Grow the flat: cells of EQUAL height, 8-connected.
        let id = flats as u32;
        let level = filled[seed];
        let mut cells: Vec<usize> = Vec::new();
        member[seed] = id;
        queue.push_back(seed);
        while let Some(c) = queue.pop_front() {
            cells.push(c);
            for k in 0..8 {
                let Some((j, _)) = neighbour(field, c, k) else {
                    continue;
                };
                if field.nodata[j] || member[j] != u32::MAX {
                    continue;
                }
                if filled[j] == level {
                    member[j] = id;
                    queue.push_back(j);
                }
            }
        }
        flats += 1;
        flat_cells += cells.len();

        // Low and high edges of THIS flat.
        let mut d_low = vec![usize::MAX; cells.len()];
        let mut d_high = vec![usize::MAX; cells.len()];
        let mut slot = std::collections::HashMap::with_capacity(cells.len());
        for (idx, &c) in cells.iter().enumerate() {
            slot.insert(c, idx);
        }
        let mut low_q: VecDeque<usize> = VecDeque::new();
        let mut high_q: VecDeque<usize> = VecDeque::new();
        for (idx, &c) in cells.iter().enumerate() {
            let mut beside_lower = false;
            let mut beside_higher = false;
            for k in 0..8 {
                let Some((j, _)) = neighbour(field, c, k) else {
                    continue;
                };
                if field.nodata[j] {
                    continue;
                }
                if filled[j] < level {
                    beside_lower = true;
                } else if filled[j] > level {
                    beside_higher = true;
                }
            }
            if beside_lower {
                d_low[idx] = 0;
                low_q.push_back(idx);
            }
            if beside_higher {
                d_high[idx] = 0;
                high_q.push_back(idx);
            }
        }
        let sweep = |dist: &mut Vec<usize>, q: &mut VecDeque<usize>| {
            while let Some(idx) = q.pop_front() {
                let c = cells[idx];
                let d = dist[idx];
                for k in 0..8 {
                    let Some((j, _)) = neighbour(field, c, k) else {
                        continue;
                    };
                    let Some(&nidx) = slot.get(&j) else { continue };
                    if dist[nidx] == usize::MAX {
                        dist[nidx] = d + 1;
                        q.push_back(nidx);
                    }
                }
            }
        };
        sweep(&mut d_low, &mut low_q);
        sweep(&mut d_high, &mut high_q);

        let max_low = d_low.iter().filter(|&&d| d != usize::MAX).copied().max();
        let Some(max_low) = max_low else {
            // No low edge at all: a closed flat the priority flood should have
            // filled. Leave it — a silent gradient here would invent an
            // outlet that does not exist.
            continue;
        };
        for idx in 0..cells.len() {
            let toward_low = if d_low[idx] == usize::MAX {
                0
            } else {
                max_low - d_low[idx]
            };
            let from_high = if d_high[idx] == usize::MAX {
                0
            } else {
                d_high[idx]
            };
            let inc = toward_low + from_high;
            max_inc = max_inc.max(inc);
            let rise = (FLAT_EPSILON_MM * inc as f64).min(FLAT_MAX_RISE_MM);
            out[cells[idx]] += rise;
        }
    }
    (out, flats, flat_cells, max_inc)
}

/// D8 receivers on the filled surface: the steepest-descent neighbour, or
/// `None` where nothing is lower (an outlet).
///
/// SAFETY: `i` ranges over `0..n`; `filled`/`field.nodata` are `len == n`;
/// `neighbour` returns only in-range cells.
#[allow(clippy::indexing_slicing)]
#[must_use]
pub fn d8_receivers(field: &FlowField, filled: &[f64]) -> Vec<Option<u32>> {
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
///
/// SAFETY: `order` holds only data-cell ids in `0..n`; `acc`/`receivers` are
/// `len == n`; each receiver id came from [`d8_receivers`] and is in range.
#[allow(clippy::indexing_slicing)]
#[must_use]
pub fn d8_accumulation(field: &FlowField, filled: &[f64], receivers: &[Option<u32>]) -> Vec<f64> {
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
