//! **M4 research candidate 3 — scallop as an iso-field contour extractor.**
//!
//! The plan asks a question the offset cascade cannot answer from inside
//! itself: *"whether scallop remains an offset-ring algorithm or should become
//! an iso-field contour extractor."* This module is the second half of that
//! comparison. Nothing here is on a production path; it is reachable only
//! through [`crate::scallop::RingSource::IsoField`], which no shipped caller
//! selects.
//!
//! # The idea
//!
//! An offset cascade must pick **one scalar** per iteration, because
//! `offset_polygon` takes one distance. Everything M4 is investigating —
//! min-across-ring, min-across-polygons, the fixed-20 sampling, the
//! flat-ground `max_rings` budget — descends from that single constraint.
//!
//! Drop the constraint and the algorithm changes shape. Let `s(x, y)` be the
//! stepover the geometry allows *at that point*, and define
//!
//! ```text
//! |∇D| = 1 / s(x, y),    D = 0 on the region boundary
//! ```
//!
//! `D` is then "how many passes in from the edge am I", measured in local
//! stepovers, and the passes are the integer level sets `D = 1, 2, 3, …`.
//! Adjacent level sets are one *local* stepover apart everywhere by
//! construction — which is precisely the iso-scallop property the cascade
//! approximates with a per-ring constant.
//!
//! This is the standard geodesic-distance / fast-marching formulation of
//! iso-scallop machining, and it is the same architecture `adaptive3d`'s
//! `clear_z_level_adaptive` already uses in this repo (EDT → curvature field
//! → per-cell threshold), so it is not a foreign idea to the codebase.
//!
//! # What it buys, structurally
//!
//! * **No reduction.** There is no per-ring scalar to collapse, so
//!   min-across-ring and min-across-polygons have nothing to reduce. A flat
//!   span of a ring advances at the flat rate while a steep span of the *same
//!   ring* advances at the steep rate.
//! * **A termination invariant instead of a cap.** The ring count is
//!   `⌊max D⌋`, known before a single ring is emitted. M4's fix-sequence item
//!   4 asks for exactly this: *"replace `max_rings` with a defensible
//!   termination invariant"*. The cascade's `max_rings` is a guess about how
//!   many offsets it will take to collapse; `⌊max D⌋` is the answer.
//! * **No repeated offsetting.** `offset_polygon` adds vertices on every call
//!   and the cascade compounds them (M5's item; scallop already carries a
//!   decimation pass just to keep the compounding linear). Level sets are
//!   extracted independently from one field, so there is no compounding to
//!   contain and the decimation compensation has nothing to do.
//!
//! # What it costs, honestly
//!
//! * The rings are **grid contours**, so their XY placement carries the
//!   field's cell as an error floor, where an offset ring is exact in XY.
//!   Whether that matters is a measurement, not an argument — the M4 harness
//!   makes it.
//! * The Eikonal solve is `O(cells)` per sweep and needs a handful of sweeps;
//!   on a fine grid that is real work the cascade does not do.
//! * A saddle in `D` merges or splits level sets. Marching squares handles
//!   that correctly and silently, which is a feature for coverage and a
//!   hazard for anything downstream that assumes ring identity.
//!
//! # Solver
//!
//! Fast sweeping with the Godunov upwind update — four alternating sweep
//! directions to convergence. Chosen over fast marching because it needs no
//! heap, is trivially deterministic, and converges in a fixed small number of
//! sweeps on fields without deep spiral topology.

use crate::geo::P2;
use crate::marching_squares::{
    CHAIN_EPS, EDGE_BOTTOM, EDGE_LEFT, EDGE_RIGHT, EDGE_TOP, cell_case, cell_segments,
    chain_segments,
};
use crate::polygon::Polygon2;
use crate::slope::SlopeMap;

/// The per-cell stepover field and the geodesic pass index solved from it.
pub struct IsoScallopField {
    pub rows: usize,
    pub cols: usize,
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell: f64,
    /// Locally allowed stepover, mm. `NaN` outside the region.
    pub stepover_mm: Vec<f64>,
    /// Passes-from-the-boundary — **signed**: positive inside the region,
    /// negative outside, and zero exactly on the region edge. `INFINITY`
    /// where the solve never reached (disconnected interior — which cannot
    /// happen for a region whose boundary encloses it, and is asserted by the
    /// harness rather than assumed).
    ///
    /// The sign is not decoration. Level sets are extracted by interpolating
    /// this field along cell edges, and the boundary almost never lands on a
    /// grid node — so a field that is flat-zero everywhere outside makes the
    /// level-1 crossing in a boundary cell depend on where the *node* is
    /// rather than where the *region edge* is. Carrying the outside distance
    /// as a negative makes the interpolated zero land on the true edge, and
    /// every level above it fall the right distance inside. See
    /// [`seed_boundary`].
    pub passes: Vec<f64>,
    /// `⌊max passes⌋` — the exact ring count, known before extraction.
    pub ring_count: usize,
}

impl IsoScallopField {
    #[inline]
    #[must_use]
    const fn idx(&self, row: usize, col: usize) -> usize {
        row * self.cols + col
    }
}

/// Build the stepover field over `boundary` and solve the pass-index field.
///
/// `stepover_at` is supplied by the caller so this module states no opinion on
/// the stepover law — the M4 harness passes the corrected
/// `StepoverGeometry::CosineSlope` form, and passing the shipped form instead
/// isolates "iso-field vs cascade" from "which slope law".
#[must_use]
pub fn build_field(
    boundary: &Polygon2,
    slope_map: &SlopeMap,
    stepover_at: &dyn Fn(f64, f64) -> f64,
) -> IsoScallopField {
    let rows = slope_map.rows;
    let cols = slope_map.cols;
    let cell = slope_map.cell_size;
    let mut stepover_mm = vec![f64::NAN; rows * cols];
    let mut passes = vec![0.0_f64; rows * cols];

    // Rasterise the region. Interior cells start at INFINITY (unsolved);
    // everything outside stays 0 and is the Dirichlet boundary condition.
    let mut inside = vec![false; rows * cols];
    #[allow(clippy::indexing_slicing)] // SAFETY: i = row*cols + col with both bounded by the loops
    for row in 0..rows {
        for col in 0..cols {
            let x = slope_map.origin_x + col as f64 * cell;
            let y = slope_map.origin_y + row as f64 * cell;
            let i = row * cols + col;
            if boundary.contains_point_eps(&P2::new(x, y), 1e-9) {
                inside[i] = true;
                passes[i] = f64::INFINITY;
                stepover_mm[i] = stepover_at(x, y);
            }
        }
    }

    // Sub-cell Dirichlet condition on the true region edge (see `passes`).
    let fixed = seed_boundary(
        boundary,
        &mut passes,
        &inside,
        &stepover_mm,
        rows,
        cols,
        cell,
        slope_map.origin_x,
        slope_map.origin_y,
    );

    fast_sweep(&mut passes, &inside, &fixed, &stepover_mm, rows, cols, cell);

    let max_pass = passes
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .fold(0.0_f64, f64::max);
    let ring_count = max_pass.floor() as usize;

    IsoScallopField {
        rows,
        cols,
        origin_x: slope_map.origin_x,
        origin_y: slope_map.origin_y,
        cell,
        stepover_mm,
        passes,
        ring_count,
    }
}

/// Distance from `(x, y)` to the nearest point of `poly`'s boundary —
/// exterior **and** holes, since a hole edge bounds the region just as the
/// outer ring does.
#[must_use]
fn distance_to_boundary(poly: &Polygon2, x: f64, y: f64) -> f64 {
    let mut best = f64::INFINITY;
    let mut ring_dist = |ring: &[P2]| {
        if ring.len() < 2 {
            return;
        }
        for i in 0..ring.len() {
            // SAFETY: i and (i + 1) % len are both in 0..len.
            #[allow(clippy::indexing_slicing)]
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len_sq = dx * dx + dy * dy;
            let t = if len_sq <= f64::EPSILON {
                0.0
            } else {
                (((x - a.x) * dx + (y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
            };
            let d = (x - (a.x + dx * t)).hypot(y - (a.y + dy * t));
            if d < best {
                best = d;
            }
        }
    };
    ring_dist(&poly.exterior);
    for hole in &poly.holes {
        ring_dist(hole);
    }
    best
}

/// Pin every grid node within one cell of the region edge to its **exact**
/// signed distance from that edge, expressed in local stepovers, and report
/// which nodes were pinned so the sweep leaves them alone.
///
/// # Why this is not a refinement
///
/// Without it the Dirichlet condition is "`D = 0` at every node outside the
/// polygon", which is a statement about the GRID, not about the region. The
/// true edge lies somewhere inside the boundary cell, so:
///
/// * every level set is pushed **outward** by up to a full cell, and
/// * the level-1 contour can be interpolated to a position outside the
///   region entirely.
///
/// That second failure is not cosmetic. A ring point outside the model
/// footprint gets a drop-cutter answer from the cutter's rim riding the mesh
/// edge — around a millimetre low on the M4 grooved block — and the finish
/// grid's coverage mask cannot veto it, because at a 0.75 mm cell the nearest
/// cell to a point 19 µm past the edge is a covered one. That is the
/// localised −995/−1115 µm gouge of `CHECKPOINT_C_EVIDENCE.md` §3.8: not
/// stepover, not chord sag, but rings placed off the part.
///
/// Cost is bounded by the region PERIMETER, not its area: only nodes with a
/// neighbour on the other side of the edge are ever measured.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
// SAFETY: every index is bounded by the loop ranges below.
fn seed_boundary(
    boundary: &Polygon2,
    passes: &mut [f64],
    inside: &[bool],
    stepover_mm: &[f64],
    rows: usize,
    cols: usize,
    cell: f64,
    origin_x: f64,
    origin_y: f64,
) -> Vec<bool> {
    let mut fixed = vec![false; rows * cols];
    // The stepover field is only defined inside; a node just outside the edge
    // still needs one to express its distance in passes, so it borrows the
    // nearest inside neighbour's.
    for row in 0..rows {
        for col in 0..cols {
            let i = row * cols + col;
            let here = inside[i];
            let mut straddles = false;
            let mut neighbour_step = f64::NAN;
            for (dr, dc) in [(-1_i64, 0_i64), (1, 0), (0, -1), (0, 1)] {
                let (r, c) = (row as i64 + dr, col as i64 + dc);
                if r < 0 || c < 0 || r >= rows as i64 || c >= cols as i64 {
                    // Off-grid counts as outside: the grid is padded past the
                    // footprint, so this only fires on a region that runs to
                    // the very edge of it.
                    straddles |= here;
                    continue;
                }
                let j = (r as usize) * cols + (c as usize);
                if inside[j] != here {
                    straddles = true;
                }
                if inside[j] && stepover_mm[j].is_finite() {
                    neighbour_step = stepover_mm[j];
                }
            }
            if !straddles {
                continue;
            }
            let s = if here { stepover_mm[i] } else { neighbour_step };
            if !s.is_finite() || s <= 0.0 {
                continue;
            }
            let x = origin_x + col as f64 * cell;
            let y = origin_y + row as f64 * cell;
            let d = distance_to_boundary(boundary, x, y) / s;
            passes[i] = if here { d } else { -d };
            fixed[i] = true;
        }
    }
    fixed
}

/// Godunov upwind fast sweeping for `|∇D| = 1/s`.
///
/// The local slowness is `f = cell / s`, i.e. the cost in *passes* of crossing
/// one cell. Four sweep directions, repeated until no cell moves by more than
/// a tolerance — capped, because a pathological field must not hang a
/// generator.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
// SAFETY: every index is bounded by the loop ranges
fn fast_sweep(
    passes: &mut [f64],
    inside: &[bool],
    fixed: &[bool],
    stepover_mm: &[f64],
    rows: usize,
    cols: usize,
    cell: f64,
) {
    const MAX_ROUNDS: usize = 24;
    const TOL: f64 = 1e-6;

    // (row ascending?, col ascending?) — the four sweep orders.
    let orders = [(true, true), (true, false), (false, true), (false, false)];

    for _round in 0..MAX_ROUNDS {
        let mut moved = 0.0_f64;
        for &(row_up, col_up) in &orders {
            for ri in 0..rows {
                let row = if row_up { ri } else { rows - 1 - ri };
                for ci in 0..cols {
                    let col = if col_up { ci } else { cols - 1 - ci };
                    let i = row * cols + col;
                    // A seeded node already holds its exact distance to the
                    // region edge; propagating over it would replace a
                    // measurement with an approximation.
                    if !inside[i] || fixed[i] {
                        continue;
                    }
                    let s = stepover_mm[i];
                    if s <= 0.0 || s.is_nan() {
                        continue;
                    }
                    let f = cell / s;

                    let a = neighbour_min(passes, rows, cols, row, col, true);
                    let b = neighbour_min(passes, rows, cols, row, col, false);
                    if !a.is_finite() && !b.is_finite() {
                        continue;
                    }
                    let candidate = if !a.is_finite() {
                        b + f
                    } else if !b.is_finite() {
                        a + f
                    } else if (a - b).abs() >= f {
                        a.min(b) + f
                    } else {
                        // The two-sided Godunov root.
                        let disc = 2.0 * f * f - (a - b) * (a - b);
                        (a + b + disc.max(0.0).sqrt()) * 0.5
                    };
                    if candidate < passes[i] {
                        moved = moved.max(passes[i] - candidate);
                        passes[i] = candidate;
                    }
                }
            }
        }
        if moved <= TOL {
            break;
        }
    }
}

#[allow(clippy::indexing_slicing)] // SAFETY: bounds checked before each read
fn neighbour_min(
    passes: &[f64],
    rows: usize,
    cols: usize,
    row: usize,
    col: usize,
    vertical: bool,
) -> f64 {
    let mut best = f64::INFINITY;
    if vertical {
        if row > 0 {
            best = best.min(passes[(row - 1) * cols + col]);
        }
        if row + 1 < rows {
            best = best.min(passes[(row + 1) * cols + col]);
        }
    } else {
        if col > 0 {
            best = best.min(passes[row * cols + col - 1]);
        }
        if col + 1 < cols {
            best = best.min(passes[row * cols + col + 1]);
        }
    }
    best
}

/// Extract the ring polylines at pass levels `1 ..= field.ring_count`.
///
/// The boundary itself (level 0) is **not** emitted here — the caller already
/// has it as an exact polygon and uses that, so the two candidates share an
/// identical first ring and the comparison is about placement of the rest.
#[must_use]
pub fn extract_rings(field: &IsoScallopField) -> Vec<Vec<P2>> {
    let mut out = Vec::new();
    for k in 1..=field.ring_count {
        let level = k as f64;
        for loop_pts in iso_contour(field, level) {
            if loop_pts.len() >= 3 {
                out.push(loop_pts);
            }
        }
    }
    out
}

/// Linear-interpolated marching squares on the pass field.
///
/// Reuses the shipped case tables (`cell_case` / `cell_segments` /
/// `chain_segments`) rather than a private copy, and differs from
/// `contour_extract::marching_squares_bool_grid` in exactly one way: the
/// crossing point is **interpolated** along the edge instead of taken at its
/// midpoint. A boolean grid has no sub-cell information to interpolate with; a
/// scalar field does, and throwing it away would hand the iso-field candidate
/// a half-cell placement error it does not actually have.
#[allow(clippy::indexing_slicing)] // SAFETY: row/col bounded by the loop ranges
fn iso_contour(field: &IsoScallopField, level: f64) -> Vec<Vec<P2>> {
    if field.rows < 2 || field.cols < 2 {
        return Vec::new();
    }
    let cell = field.cell;
    let at = |row: usize, col: usize| -> f64 {
        let v = field.passes[field.idx(row, col)];
        if v.is_finite() { v } else { f64::MAX }
    };
    let world = |row: usize, col: usize| -> P2 {
        P2::new(
            field.origin_x + col as f64 * cell,
            field.origin_y + row as f64 * cell,
        )
    };
    // Crossing point between two corners, by linear interpolation of the
    // field. Falls back to the midpoint when the two values are equal.
    let lerp = |pa: P2, va: f64, pb: P2, vb: f64| -> P2 {
        let denom = vb - va;
        let t = if denom.abs() < 1e-12 {
            0.5
        } else {
            ((level - va) / denom).clamp(0.0, 1.0)
        };
        P2::new(pa.x + (pb.x - pa.x) * t, pa.y + (pb.y - pa.y) * t)
    };

    let mut segments: Vec<(P2, P2)> = Vec::new();
    for row in 0..field.rows - 1 {
        for col in 0..field.cols - 1 {
            // Local corner labels match `ms_bool_segments`: increasing row
            // moves DOWN in the local frame, so (r, c) is `tl`.
            let (tl, tr) = (at(row, col), at(row, col + 1));
            let (bl, br) = (at(row + 1, col), at(row + 1, col + 1));
            // "Inside" = at least this many passes in.
            let case = cell_case(bl >= level, br >= level, tr >= level, tl >= level);
            let edges = cell_segments(case);
            if edges.is_empty() {
                continue;
            }
            let (p_tl, p_tr) = (world(row, col), world(row, col + 1));
            let (p_bl, p_br) = (world(row + 1, col), world(row + 1, col + 1));
            let point_on = |edge: u8| -> P2 {
                match edge {
                    EDGE_LEFT => lerp(p_bl, bl, p_tl, tl),
                    EDGE_BOTTOM => lerp(p_bl, bl, p_br, br),
                    EDGE_RIGHT => lerp(p_br, br, p_tr, tr),
                    EDGE_TOP => lerp(p_tl, tl, p_tr, tr),
                    _ => p_bl,
                }
            };
            for &(a, b) in edges {
                let (pa, pb) = (point_on(a), point_on(b));
                if (pa.x - pb.x).abs() > CHAIN_EPS || (pa.y - pb.y).abs() > CHAIN_EPS {
                    segments.push((pa, pb));
                }
            }
        }
    }
    chain_segments(&segments)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// A slope map over a flat surface — every cell reads angle 0, curvature 0.
    fn flat_slope_map(half: f64, cell: f64) -> SlopeMap {
        let n = ((2.0 * half) / cell).round() as usize + 1;
        SlopeMap::from_z_grid(&vec![0.0; n * n], n, n, -half, -half, cell)
    }

    fn square(half: f64) -> Polygon2 {
        Polygon2::new(vec![
            P2::new(-half, -half),
            P2::new(half, -half),
            P2::new(half, half),
            P2::new(-half, half),
        ])
    }

    /// On a flat square with a constant stepover the pass field IS the
    /// Chebyshev-free Euclidean distance divided by the stepover, so the ring
    /// count is the inradius over the stepover — known in closed form, and the
    /// whole point of the termination invariant.
    #[test]
    fn flat_square_ring_count_is_the_inradius_over_the_stepover() {
        let half = 5.0;
        let cell = 0.1;
        let stepover = 0.5;
        let sm = flat_slope_map(6.0, cell);
        let field = build_field(&square(half), &sm, &|_, _| stepover);

        let expected = (half / stepover).floor() as usize;
        assert!(
            field.ring_count.abs_diff(expected) <= 1,
            "flat square inradius {half} at stepover {stepover} should give \
             ~{expected} rings, got {}",
            field.ring_count
        );

        // Nothing may be left unsolved inside the region: an unreached cell
        // would be silent uncut material, the exact defect `max_rings`
        // produces in the cascade.
        let unsolved = field
            .passes
            .iter()
            .zip(&field.stepover_mm)
            .filter(|(p, s)| !s.is_nan() && !p.is_finite())
            .count();
        assert_eq!(unsolved, 0, "{unsolved} interior cells never solved");
    }

    /// Adjacent level sets must sit one local stepover apart — the iso-scallop
    /// property the whole construction exists for.
    #[test]
    fn adjacent_levels_are_one_stepover_apart() {
        let cell = 0.05;
        let stepover = 0.4;
        let sm = flat_slope_map(4.0, cell);
        let field = build_field(&square(3.0), &sm, &|_, _| stepover);
        let rings = extract_rings(&field);
        assert!(rings.len() >= 5, "{} rings", rings.len());

        // Ring k is the level set at k passes; on a flat square that is the
        // square inset by k * stepover, so its half-width is 3 - k*stepover.
        for (k, ring) in rings.iter().enumerate().take(5) {
            let half_width = ring
                .iter()
                .map(|p| p.x.abs().max(p.y.abs()))
                .fold(0.0_f64, f64::max);
            let expected = 3.0 - (k + 1) as f64 * stepover;
            assert!(
                (half_width - expected).abs() < 2.0 * cell,
                "ring {k}: half-width {half_width:.3} vs expected {expected:.3}"
            );
        }
    }

    /// The field must respond to a spatially varying stepover — otherwise the
    /// candidate is just a slow way to draw uniform offsets.
    #[test]
    fn a_varying_stepover_field_produces_unevenly_spaced_levels() {
        let cell = 0.05;
        let sm = flat_slope_map(4.0, cell);
        // Tight on the left half, loose on the right half.
        let field = build_field(&square(3.0), &sm, &|x, _| if x < 0.0 { 0.15 } else { 0.6 });
        let rings = extract_rings(&field);
        assert!(rings.len() > 5, "{} rings", rings.len());

        // The first ring must be closer to the boundary on the tight side.
        let first = &rings[0];
        let left_inset = first
            .iter()
            .filter(|p| p.x < -1.0)
            .map(|p| 3.0 + p.x)
            .fold(f64::INFINITY, f64::min);
        let right_inset = first
            .iter()
            .filter(|p| p.x > 1.0)
            .map(|p| 3.0 - p.x)
            .fold(f64::INFINITY, f64::min);
        assert!(
            right_inset > left_inset * 2.0,
            "ring 1 should sit {:.3} mm in on the tight side and ~4x further \
             on the loose side; got left {left_inset:.3} right {right_inset:.3}",
            0.15
        );
    }
}
