//! Engagement computation, direction search, and entry-point finding.
//!
//! Shared by the adaptive main loop in `path.rs` (via `pub(super)` free
//! functions) and by `mod.rs` tests.

use super::material_grid::{CELL_FRINGE, CELL_MATERIAL, SUB, sub_offset};
use super::{
    EngagementMeasure, MaterialGrid, angle_diff, refine_angle_bracket, target_engagement_fraction,
};
use crate::geo::P2;
use crate::ops::adaptive_shared::radial_woc_fraction_from_leading_arc;
use crate::polygon::Polygon2;
use crate::trace::debug_trace::ToolpathDebugBounds2;

use std::f64::consts::{PI, TAU};

// ── The tool-centre region (2D rule) ───────────────────────────────────

/// Where the cutter centre may stand under the 2D rule: the part inset by
/// the tool radius, every piece of it, its arc joins flattened once at
/// [`crate::polygon::FlattenPolicy::untoleranced`] (10 µm, points on the
/// true arc). The planner reads it exactly, and a straight move is legal
/// only when the whole segment stays inside it: two legal ends on either
/// side of an island's inset arc have a chord that cuts the island
/// (G-ADAPTPASSLOAD round 3, measured 0.026 mm per 1.5 mm step on the
/// six-island pocket).
#[derive(Clone, Debug, Default)]
pub(crate) struct ToolCentreRegion {
    pieces: Vec<Polygon2>,
}

impl ToolCentreRegion {
    /// The region of `polygon` for a cutter of radius `tool_radius`. The
    /// helix-containment dressup reads the same pieces
    /// ([`tool_centre_pieces`]).
    pub(crate) fn new(polygon: &Polygon2, tool_radius: f64) -> Self {
        Self {
            pieces: tool_centre_pieces(polygon, tool_radius),
        }
    }

    pub(crate) fn pieces(&self) -> &[Polygon2] {
        &self.pieces
    }

    /// True when `p` lies inside a piece.
    pub(crate) fn contains(&self, p: &P2) -> bool {
        self.pieces.iter().any(|q| q.contains_point(p))
    }

    /// True when the straight move `a → b` stays inside the region: its end
    /// and midpoint inside, and no edge of any ring crossed. `a` is where
    /// the cutter stands already; it may sit on the boundary (a wall pass),
    /// which the crossing test reads as touching, not crossing.
    pub(crate) fn contains_segment(&self, a: P2, b: P2) -> bool {
        self.contains(&b)
            && self.contains(&P2::new(0.5 * (a.x + b.x), 0.5 * (a.y + b.y)))
            && !self.crossed_by(a, b)
    }

    /// True when the segment `a → b` properly crosses an edge of a ring.
    pub(crate) fn crossed_by(&self, a: P2, b: P2) -> bool {
        let orient = |p: P2, q: P2, r: P2| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
        let (lo_x, hi_x) = (a.x.min(b.x), a.x.max(b.x));
        let (lo_y, hi_y) = (a.y.min(b.y), a.y.max(b.y));
        self.pieces.iter().any(|piece| {
            std::iter::once(&piece.exterior)
                .chain(piece.holes.iter())
                .any(|ring| {
                    let n = ring.len();
                    (0..n).any(|i| {
                        let (Some(&c), Some(&d)) = (ring.get(i), ring.get((i + 1) % n)) else {
                            return false;
                        };
                        if c.x.max(d.x) < lo_x
                            || c.x.min(d.x) > hi_x
                            || c.y.max(d.y) < lo_y
                            || c.y.min(d.y) > hi_y
                        {
                            return false;
                        }
                        let (o1, o2) = (orient(a, b, c), orient(a, b, d));
                        let (o3, o4) = (orient(c, d, a), orient(c, d, b));
                        o1 * o2 < 0.0 && o3 * o4 < 0.0
                    })
                })
        })
    }
}

/// The pieces of [`ToolCentreRegion`]: `polygon` offset inward by
/// `tool_radius` on the arc-carrying cascade and flattened once. The one
/// builder the planner and the entry dressup's helix containment share.
pub(crate) fn tool_centre_pieces(polygon: &Polygon2, tool_radius: f64) -> Vec<Polygon2> {
    crate::polygon::OffsetRingSet::from_polygon(polygon)
        .offset(tool_radius)
        .to_polygons(crate::polygon::FlattenPolicy::untoleranced())
}

// ── Engagement computation ─────────────────────────────────────────────

/// Compute engagement fraction at position (cx, cy) with tool of given radius.
///
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Uses disk-area sampling: counts the fraction of grid cells within the
/// tool circle that contain uncut material. This is more precise than
/// circumference-only sampling (which only measures the engagement angle)
/// because it measures the actual cut area fraction.
///
/// Returns a value in [0.0, 1.0].
pub(crate) fn compute_engagement(grid: &MaterialGrid, cx: f64, cy: f64, radius: f64) -> f64 {
    let r_sq = radius * radius;
    let col_min = ((cx - radius - grid.origin_x) / grid.cell_size)
        .floor()
        .max(0.0) as usize;
    let col_max = ((cx + radius - grid.origin_x) / grid.cell_size).ceil() as usize;
    let row_min = ((cy - radius - grid.origin_y) / grid.cell_size)
        .floor()
        .max(0.0) as usize;
    let row_max = ((cy + radius - grid.origin_y) / grid.cell_size).ceil() as usize;

    let col_max = col_max.min(grid.cols.saturating_sub(1));
    let row_max = row_max.min(grid.rows.saturating_sub(1));

    let mut material_cells = 0u32;
    let mut total_cells = 0u32;

    for row in row_min..=row_max {
        let cell_y = grid.origin_y + row as f64 * grid.cell_size;
        let dy = cell_y - cy;
        let dy_sq = dy * dy;
        if dy_sq > r_sq {
            continue;
        }
        for col in col_min..=col_max {
            let cell_x = grid.origin_x + col as f64 * grid.cell_size;
            let dx = cell_x - cx;
            if dx * dx + dy_sq <= r_sq {
                total_cells += 1;
                if grid.cells[row * grid.cols + col] == CELL_MATERIAL {
                    material_cells += 1;
                }
            }
        }
    }

    if total_cells == 0 {
        return 0.0;
    }
    material_cells as f64 / total_cells as f64
}

/// Leading-arc engagement: the fraction of the full cutter circle whose
/// **leading semicircle** (relative to the move direction `dir_angle`)
/// lies in uncut material.
///
/// This is the same physical quantity as `target_engagement_fraction`
/// (contact angle α / 2π): in steady state cutting alongside a cleared
/// swath at radial stepover `s`, the reading is `acos(1 − s/R) / 2π`
/// exactly. `compute_engagement` above measures disk-*area* fraction,
/// which is a different quantity and only coincides with the angle
/// fraction at full slot — comparing it against the α/2π target makes
/// the effective stepover deviate from the commanded one (see
/// planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md, finding F1).
///
/// Sampling is on the flute circle itself (radius R): the trailing
/// semicircle is excluded because it only ever passes through material
/// already counted as the leading edge swept it. Points are taken at
/// arc midpoints so the estimate is unbiased w.r.t. quantisation.
pub(crate) fn compute_engagement_arc(
    grid: &MaterialGrid,
    cx: f64,
    cy: f64,
    radius: f64,
    dir_angle: f64,
) -> f64 {
    // ~2 samples per grid cell along the leading arc, bounded for cost.
    let n = ((2.0 * PI * radius / grid.cell_size).ceil() as usize).clamp(32, 128);
    let mut hits = 0usize;
    for i in 0..n {
        let t = (i as f64 + 0.5) / n as f64;
        let theta = dir_angle - std::f64::consts::FRAC_PI_2 + t * PI;
        let x = cx + radius * theta.cos();
        let y = cy + radius * theta.sin();
        if grid.is_material(x, y) {
            hits += 1;
        }
    }
    // The leading semicircle is half the circle: scale the in-material
    // fraction of the semicircle to a fraction of the full circle so the
    // value compares directly against α/2π.
    0.5 * (hits as f64 / n as f64)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod engagement_measure_tests {
    //! Closed-form oracle for the engagement measures (adaptive algorithm
    //! review 2026-06-12, finding F1).
    //!
    //! Steady-state scene, built exactly on the cell lattice so the
    //! oracle carries no fixture-rasterisation slack: the cutter at
    //! `(cx, cy)` moves in +x alongside a previous parallel pass whose
    //! swath cleared everything at `y ≥ y_edge`, with its own swath
    //! (`|y − cy| ≤ R, x ≤ cx`) cleared behind it. The radial stepover
    //! is `s = y_edge − (cy − R)` (chord height of the material cap),
    //! the contact angle is `α = acos(1 − s/R)` analytically, and a
    //! measure in the same units as `target_engagement_fraction` must
    //! read `α/2π` at the next position.

    use super::super::material_grid::{CELL_CLEARED, MaterialGrid};
    use super::{
        EngagementMeasure, PassLoad, compute_engagement, compute_engagement_arc,
        compute_swept_width, search_direction_with_metrics,
    };
    use crate::geo::P2;
    use crate::ops::adaptive_shared::target_engagement_fraction;
    use crate::polygon::Polygon2;

    const R: f64 = 3.0;
    const CELL: f64 = R / 6.0; // production floor: max(R/6, tolerance)
    const STEP: f64 = CELL * 3.0; // production step length (path.rs)

    /// Build the exact steady-state scene for radial stepover `s`
    /// (must be a multiple of CELL so `y_edge` lands on the lattice).
    /// Returns the grid and the *next* candidate position — the search
    /// evaluates candidates there, against the pre-move grid.
    fn steady_state_grid(stepover: f64) -> (MaterialGrid, f64, f64) {
        let size = 60.0;
        let square = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(size, 0.0),
            P2::new(size, size),
            P2::new(0.0, size),
        ]);
        let mut grid = MaterialGrid::from_polygon(&square, CELL);

        let (cx, cy) = (size / 2.0, size / 2.0);
        let y_edge = cy - R + stepover;

        // MaterialGrid samples cell (row, col) at the lattice point
        // (origin + col·cell, origin + row·cell) — mark cells by that
        // same convention so the material boundary is exact.
        for row in 0..grid.rows {
            let cell_y = grid.origin_y + row as f64 * grid.cell_size;
            for col in 0..grid.cols {
                let cell_x = grid.origin_x + col as f64 * grid.cell_size;
                let prev_swath = cell_y >= y_edge - 1e-9;
                let own_swath = cell_x <= cx + 1e-9 && (cell_y - cy).abs() <= R + 1e-9;
                if prev_swath || own_swath {
                    let idx = row * grid.cols + col;
                    if grid.cells[idx] != CELL_CLEARED {
                        grid.cells[idx] = CELL_CLEARED;
                    }
                }
            }
        }
        (grid, cx + STEP, cy)
    }

    #[test]
    fn leading_arc_matches_contact_angle_oracle() {
        // s = CELL multiples: 0.5 (s/R ≈ 0.17), 1.5 (0.5R), 3.0 (R),
        // 6.0 (2R, full slot).
        for stepover in [0.5, 1.5, 3.0, 6.0] {
            let expected = target_engagement_fraction(stepover, R);
            let (grid, nx, ny) = steady_state_grid(stepover);
            let got = compute_engagement_arc(&grid, nx, ny, R, 0.0);
            assert!(
                (got - expected).abs() < 0.02,
                "leading-arc measure must read the contact-angle fraction: \
                 stepover {stepover:.2} expected {expected:.4}, got {got:.4}"
            );
        }
    }

    #[test]
    fn disk_area_measure_deviates_from_contact_angle_target() {
        // Documents finding F1: the area measure is a different physical
        // quantity from the α/2π target it is compared against. At a
        // commanded ~0.17R stepover the controller's target is ~0.094
        // but the area reading sits far below it — so the search steers
        // toward a much wider radial cut than commanded.
        let stepover = 0.5;
        let target = target_engagement_fraction(stepover, R);
        let (grid, nx, ny) = steady_state_grid(stepover);
        let area = compute_engagement(&grid, nx, ny, R);
        assert!(
            area < target * 0.7,
            "expected the disk-area reading ({area:.4}) to sit well below the \
             contact-angle target ({target:.4}); if this starts passing, the \
             area measure changed and F1 should be re-evaluated"
        );

        // And the arc measure does hit the target on the identical scene.
        let arc = compute_engagement_arc(&grid, nx, ny, R, 0.0);
        assert!(
            (arc - target).abs() < 0.02,
            "arc measure should hit the target on the same scene: \
             target {target:.4}, got {arc:.4}"
        );
    }

    // ── G-ADAPTPASSLOAD (plan §1 and sentry S2) ────────────────────────

    /// One planner cell as a fraction of D: the lattice resolution of every
    /// swept-width reading.
    const CELL_OVER_D: f64 = CELL / (2.0 * R);

    /// Oracle: the swept width of a steady side cut at stepover `s` is
    /// `s / D` to within one cell over D (the material edge falls between
    /// two lattice rows), from a sliver to a full slot.
    #[test]
    fn swept_width_reads_the_stepover_over_the_diameter() {
        for stepover in [0.5, 1.5, 3.0, 6.0] {
            let (grid, nx, ny) = steady_state_grid(stepover);
            let got = compute_swept_width(&grid, nx, ny, R, 0.0);
            let want = stepover / (2.0 * R);
            assert!(
                (got - want).abs() <= CELL_OVER_D + 1e-9,
                "stepover {stepover}: swept width {got:.4}, s/D {want:.4}, allowed \
                 +- {CELL_OVER_D:.4}"
            );
        }
    }

    /// The fine lattice the plan's quadrature claims are checked on: the
    /// disk-area reading at R / 60 is the continuous area fraction to a few
    /// thousandths; the production lattice (R / 6) moves it by up to half a
    /// column of the disc, depending on where the material edge falls.
    const FINE: f64 = R / 60.0;

    /// A steady side cut on a lattice of `cell`: the cutter at `(cx, cy)`
    /// moving +x has swept everything within R of its path up to `cx` (the
    /// swath ends in the round cap of its own disc), beside an earlier pass
    /// that cleared `y >= cy - R + stepover`. Returns the grid and the next
    /// position, one pass step (R / 2) ahead.
    fn side_cut_grid(stepover: f64, cell: f64) -> (MaterialGrid, f64, f64) {
        let size = 30.0;
        let square = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(size, 0.0),
            P2::new(size, size),
            P2::new(0.0, size),
        ]);
        let mut grid = MaterialGrid::from_polygon(&square, cell);
        let (cx, cy) = (size / 2.0, size / 2.0);
        let y_edge = cy - R + stepover;
        for row in 0..grid.rows {
            let y = grid.origin_y + row as f64 * grid.cell_size;
            for col in 0..grid.cols {
                let x = grid.origin_x + col as f64 * grid.cell_size;
                let swath = (x <= cx && (y - cy).abs() <= R) || (x - cx).hypot(y - cy) <= R;
                let idx = row * grid.cols + col;
                if (y >= y_edge || swath) && grid.cells[idx] != CELL_CLEARED {
                    grid.cells[idx] = CELL_CLEARED;
                }
            }
        }
        (grid, cx + R / 2.0, cy)
    }

    /// Plan §1, the quadrature behind finding F1: the disk-area reading of
    /// a steady side cut with step R/2 is about (w/pi) x fill, far under the
    /// alpha/2pi target, so the historical band [0.186, 0.206] (s 2, R 3) is
    /// met only at a stepover of 3.6-3.9 mm (the plan's scratch figure was
    /// 3.7-3.9; this fine lattice reads 0.188 at 3.6), 1.8-1.95x the
    /// commanded 2 mm.
    #[test]
    fn disk_area_meets_the_band_only_near_twice_the_stepover() {
        for (stepover, quadrature) in [
            (1.0, 0.051),
            (2.0, 0.104),
            (3.0, 0.158),
            (4.0, 0.211),
            (6.0, 0.315),
        ] {
            let (grid, nx, ny) = side_cut_grid(stepover, FINE);
            let got = compute_engagement(&grid, nx, ny, R);
            assert!(
                (got - quadrature).abs() < 0.005,
                "stepover {stepover}: disk area {got:.4}, plan quadrature {quadrature:.3}"
            );
        }
        let target = target_engagement_fraction(2.0, R);
        let (floor, ceiling) = (target * 0.95, super::pass_engagement_ceiling(target));
        let read = |s: f64| {
            let (grid, nx, ny) = side_cut_grid(s, FINE);
            compute_engagement(&grid, nx, ny, R)
        };
        for (s, inside) in [(3.5, false), (3.6, true), (3.9, true), (4.0, false)] {
            let got = read(s);
            assert_eq!(
                (floor..=ceiling).contains(&got),
                inside,
                "stepover {s}: disk area {got:.4} against the band [{floor:.4}, {ceiling:.4}]"
            );
        }
    }

    /// A head-on bite into a flat wall: the cutter at `(cx, cy)` has its
    /// disc touching the wall `x >= cx + R`; the next position, one pass
    /// step ahead, bites `STEP` = 1.5 mm into it. Returns the grid, the
    /// machinable mask (all true) and the current position.
    fn head_on_wall(cell: f64) -> (MaterialGrid, Vec<bool>, f64, f64) {
        let size = 30.0;
        let square = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(size, 0.0),
            P2::new(size, size),
            P2::new(0.0, size),
        ]);
        let mut grid = MaterialGrid::from_polygon(&square, cell);
        let (cx, cy) = (size / 2.0, size / 2.0);
        let wall = cx + R;
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let x = grid.origin_x + col as f64 * grid.cell_size;
                let idx = row * grid.cols + col;
                if x < wall - 1e-9 && grid.cells[idx] != CELL_CLEARED {
                    grid.cells[idx] = CELL_CLEARED;
                }
            }
        }
        let mask = vec![true; grid.rows * grid.cols];
        (grid, mask, cx, cy)
    }

    /// Plan §1: a head-on bite of p = 1.5 mm reads 0.1955 on the disk-area
    /// measure (the circular segment over the disc), inside the historical
    /// band, while its sideways spread is the chord, 2 sqrt(2Rp - p^2) / D
    /// = 0.866: the disk area admits a near-full slot.
    #[test]
    fn a_head_on_bite_reads_in_band_on_disk_area_and_near_a_slot_in_width() {
        let (grid, _mask, cx, cy) = head_on_wall(FINE);
        let (nx, ny) = (cx + STEP, cy);
        let target = target_engagement_fraction(2.0, R);
        let (floor, ceiling) = (target * 0.95, super::pass_engagement_ceiling(target));
        let area = compute_engagement(&grid, nx, ny, R);
        let segment =
            R * R * (1.0 - STEP / R).acos() - (R - STEP) * (2.0 * R * STEP - STEP * STEP).sqrt();
        let quadrature = segment / (std::f64::consts::PI * R * R);
        assert!(
            (quadrature - 0.1955).abs() < 5e-4,
            "closed form {quadrature:.4}"
        );
        assert!(
            (area - quadrature).abs() < 0.005 && area >= floor && area <= ceiling,
            "disk area {area:.4} (closed form {quadrature:.4}) should sit in the band \
             [{floor:.4}, {ceiling:.4}]"
        );
        let chord = (2.0 * R * STEP - STEP * STEP).sqrt() / R;
        let width = compute_swept_width(&grid, nx, ny, R, 0.0);
        assert!((chord - 0.866).abs() < 5e-4, "closed form {chord:.4}");
        // Both ends of the chord fall between lattice points: two cells.
        assert!(
            (width - chord).abs() <= 2.0 * FINE / (2.0 * R) + 1e-9,
            "swept width {width:.4}, chord {chord:.4}"
        );
    }

    /// Sentry S2: facing the head-on wall, the 2D rule never takes a step
    /// wider than the pass ceiling (0.3626 at s 2, R 3) plus one cell over
    /// D. The historical disk-area search (the red-before rule) takes a
    /// step 35.6 deg off the wall normal whose swept width is 0.61; it is
    /// pinned below as the documented defect.
    #[test]
    fn no_accepted_step_is_wider_than_the_pass_ceiling() {
        let (grid, mask, cx, cy) = head_on_wall(CELL);
        let distances = vec![100.0; grid.rows * grid.cols];
        let width_of = |angle: f64| {
            compute_swept_width(
                &grid,
                cx + STEP * angle.cos(),
                cy + STEP * angle.sin(),
                R,
                angle,
            )
        };

        let capped = PassLoad::swept_width(2.0, R, CELL);
        assert!(
            (capped.ceiling - 0.3626).abs() < 5e-5,
            "ceiling {}",
            capped.ceiling
        );
        if let Ok(step) = search_direction_with_metrics(
            &grid, &mask, cx, cy, R, STEP, &capped, 0.0, &distances, None,
        ) {
            let width = width_of(step.angle);
            assert!(
                width <= capped.ceiling + CELL_OVER_D,
                "the 2D rule took a step {:.1} deg wide {width:.4}, over {:.4} + {CELL_OVER_D:.4}",
                step.angle.to_degrees(),
                capped.ceiling
            );
        }

        let historical = PassLoad::historical(2.0, R, EngagementMeasure::DiskArea);
        let step = search_direction_with_metrics(
            &grid,
            &mask,
            cx,
            cy,
            R,
            STEP,
            &historical,
            0.0,
            &distances,
            None,
        )
        .expect("the historical search finds a step");
        let width = width_of(step.angle);
        assert!(
            width > capped.ceiling + CELL_OVER_D,
            "the historical disk-area search took a step {width:.4} wide; if this no \
             longer exceeds the ceiling, re-evaluate the documented defect"
        );
    }
}

/// Score weight per half turn of heading change in the direction search
/// (historical; the 2D mop walk scores its headings with the same weight).
pub(crate) const HEADING_CHANGE_WEIGHT: f64 = 0.03;

/// The band a pass step is accepted in: `target × (1 ± this)` (matches the
/// libactp reference).
pub(crate) const PASS_ENGAGEMENT_TOLERANCE: f64 = 0.05;

/// The highest engagement the direction search accepts for a pass step,
/// `target × (1 + PASS_ENGAGEMENT_TOLERANCE)`, as the contact fraction α/2π.
pub(crate) fn pass_engagement_ceiling(target_frac: f64) -> f64 {
    target_frac * (1.0 + PASS_ENGAGEMENT_TOLERANCE)
}

/// A swept-width reading of a disc that holds material but no sideways
/// extent (one material cell): positive, so "any material" tests see it,
/// and far below any band.
const SWEPT_WIDTH_PRESENT: f64 = 1e-6;

/// Radial immersion of a step to `(cx, cy)` heading `dir_angle`: the
/// sideways extent (perpendicular to the heading) of the material cells in
/// the cutter disc there, divided by the diameter (G-ADAPTPASSLOAD, operator
/// decision 2026-09-26). This is the simulator's radial width-of-cut model
/// (`dexel_stock/stamping.rs`: `perp_max − perp_min` of fresh cells in the
/// midpoint disc ÷ D) on the planner grid, read at the same lattice points
/// [`compute_engagement`] reads. Mean tangential force and MRR scale with
/// this extent (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §4), so it is
/// the load the commanded stepover names: a steady side cut at stepover `s`
/// reads `s / D` to within one cell over D.
///
/// Returns 0 on no material, and at least [`SWEPT_WIDTH_PRESENT`] when the
/// disc holds any material.
// SAFETY: rows and columns are clamped to the grid before indexing.
#[allow(clippy::indexing_slicing)]
pub(crate) fn compute_swept_width(
    grid: &MaterialGrid,
    cx: f64,
    cy: f64,
    radius: f64,
    dir_angle: f64,
) -> f64 {
    swept_width_on(grid, cx, cy, radius, dir_angle, false)
}

/// [`compute_swept_width`] with the fringe's standing sub-points read too
/// ([`super::material_grid::CELL_FRINGE`]): the slivers the centre lattice
/// misses. Two such slivers either side of a disc are one wide step on the
/// stock (G-ADAPTPASSLOAD round 3, measured 0.75 of D in the simulation
/// on a step the centre lattice read in the band).
pub(crate) fn compute_swept_width_with_slivers(
    grid: &MaterialGrid,
    cx: f64,
    cy: f64,
    radius: f64,
    dir_angle: f64,
) -> f64 {
    swept_width_on(grid, cx, cy, radius, dir_angle, true)
}

// SAFETY: rows and columns are clamped to the grid before indexing.
#[allow(clippy::indexing_slicing)]
fn swept_width_on(
    grid: &MaterialGrid,
    cx: f64,
    cy: f64,
    radius: f64,
    dir_angle: f64,
    slivers: bool,
) -> f64 {
    let r_sq = radius * radius;
    let col_min = ((cx - radius - grid.origin_x) / grid.cell_size)
        .floor()
        .max(0.0) as usize;
    let col_max = (((cx + radius - grid.origin_x) / grid.cell_size).ceil() as usize)
        .min(grid.cols.saturating_sub(1));
    let row_min = ((cy - radius - grid.origin_y) / grid.cell_size)
        .floor()
        .max(0.0) as usize;
    let row_max = (((cy + radius - grid.origin_y) / grid.cell_size).ceil() as usize)
        .min(grid.rows.saturating_sub(1));
    // Unit vector perpendicular to the heading (the sideways axis).
    let (sx, sy) = (-dir_angle.sin(), dir_angle.cos());
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    let mut take = |dx: f64, dy: f64| {
        let u = dx * sx + dy * sy;
        lo = lo.min(u);
        hi = hi.max(u);
    };
    for row in row_min..=row_max {
        let dy = grid.origin_y + row as f64 * grid.cell_size - cy;
        for col in col_min..=col_max {
            let dx = grid.origin_x + col as f64 * grid.cell_size - cx;
            let idx = row * grid.cols + col;
            match grid.cells[idx] {
                CELL_MATERIAL if dx * dx + dy * dy <= r_sq => take(dx, dy),
                // A fringe cell's centre is cut; its standing sub-points
                // are the sliver the centre lattice misses.
                CELL_FRINGE if slivers => {
                    let bits = grid.fringe_bits(idx);
                    for j in 0..SUB {
                        let py = dy + sub_offset(j) * grid.cell_size;
                        for i in 0..SUB {
                            let px = dx + sub_offset(i) * grid.cell_size;
                            if bits & (1 << (j * SUB + i)) != 0 && px * px + py * py <= r_sq {
                                take(px, py);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if hi < lo {
        return 0.0;
    }
    ((hi - lo) / (2.0 * radius)).max(SWEPT_WIDTH_PRESENT)
}

/// Which quantity a step is measured in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepMeasure {
    /// The historical measures, compared against the contact fraction α/2π
    /// (Adaptive3d slices, `KeepDownLinks::RetractedByCaller`).
    Historical(EngagementMeasure),
    /// [`compute_swept_width`], compared against the radial fraction a_e/D
    /// (2D Adaptive, whatever `engagement_measure` a project saved).
    SweptWidth,
}

/// The load a planning call holds a step to: the measure, the band the
/// direction search aims for, and the ceiling no step may exceed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PassLoad {
    pub(crate) measure: StepMeasure,
    /// The reading the search aims for.
    pub(crate) target: f64,
    /// Band floor, `target × (1 − PASS_ENGAGEMENT_TOLERANCE)`.
    pub(crate) band_floor: f64,
    /// The pass ceiling. Under [`StepMeasure::SweptWidth`] a step above it
    /// is refused everywhere; the historical search may still fall back to
    /// the best out-of-band step.
    pub(crate) ceiling: f64,
    /// The ceiling on the sliver reading
    /// ([`compute_swept_width_with_slivers`]), under the 2D rule the pass
    /// ceiling plus one planner cell over D: the centre lattice reads an
    /// edge it sees up to one cell short (the band floor's own quantum),
    /// so a sliver reading more than a cell over the ceiling is stock the
    /// lattice hid, not the edge of the cut. Infinite where no sliver is
    /// read (the historical rule, a departure).
    pub(crate) sliver_ceiling: f64,
    /// The smallest reading that counts as "the step cuts material".
    pub(crate) presence: f64,
}

impl PassLoad {
    /// The historical rule: `measure` against the α/2π target, with the
    /// out-of-band fallback.
    pub(crate) fn historical(stepover: f64, tool_radius: f64, measure: EngagementMeasure) -> Self {
        let target = target_engagement_fraction(stepover, tool_radius);
        Self {
            measure: StepMeasure::Historical(measure),
            target,
            band_floor: (target * (1.0 - PASS_ENGAGEMENT_TOLERANCE)).max(0.005),
            ceiling: pass_engagement_ceiling(target),
            sliver_ceiling: f64::INFINITY,
            presence: 0.005,
        }
    }

    /// The 2D rule (G-ADAPTPASSLOAD): swept width against the commanded
    /// radial fraction `s / D`, capped at the pass ceiling expressed as a
    /// radial fraction, `radial_woc_fraction_from_leading_arc(
    /// pass_engagement_ceiling(α*/2π))` (0.3626 at s 2, R 3).
    ///
    /// The band floor is one planner cell under the target: the lattice
    /// reads a true side cut of width `s` anywhere in `[s - cell, s]` (the
    /// stock edge falls between two lattice rows; `swept_width_reads_the_
    /// stepover_over_the_diameter` pins it), so a step reading down to
    /// `(s - cell) / D` may be a cut at the full stepover. The historical
    /// `target x 0.95` floor is 0.017 of D under the target, a fifth of one
    /// lattice quantum (cell / D = 0.083 at R 3): an axis-aligned side cut
    /// could read in the band only at one exact lattice phase.
    pub(crate) fn swept_width(stepover: f64, tool_radius: f64, cell: f64) -> Self {
        let diameter = 2.0 * tool_radius;
        let target = stepover.min(diameter) / diameter;
        let ceiling = radial_woc_fraction_from_leading_arc(pass_engagement_ceiling(
            target_engagement_fraction(stepover, tool_radius),
        ));
        Self {
            measure: StepMeasure::SweptWidth,
            target,
            band_floor: (target - cell / diameter).max(SWEPT_WIDTH_PRESENT),
            ceiling,
            sliver_ceiling: ceiling + cell / diameter,
            presence: SWEPT_WIDTH_PRESENT * 0.5,
        }
    }

    /// The same load with no ceiling, for the steps out of a straight
    /// plunge (G-ADAPTPASSLOAD). Leaving a hole of the cutter's own size
    /// reads at least half the diameter whatever the heading, so a plunge
    /// entry could never be left under the ceiling; those steps still aim
    /// at the band. A helix entry leaves a wider hole and needs no
    /// exemption.
    pub(crate) fn departing(self) -> Self {
        Self {
            ceiling: f64::INFINITY,
            sliver_ceiling: f64::INFINITY,
            ..self
        }
    }

    /// True when a step above the ceiling is refused (no fallback).
    pub(crate) fn caps(&self) -> bool {
        self.measure == StepMeasure::SweptWidth
    }
}

/// The reading of a step to `(nx, ny)` heading `angle` on `grid`, in the
/// measure `measure`. The one reading every pass step and link step is held
/// to.
pub(super) fn measure_engagement(
    grid: &MaterialGrid,
    nx: f64,
    ny: f64,
    tool_radius: f64,
    angle: f64,
    measure: StepMeasure,
) -> f64 {
    match measure {
        StepMeasure::Historical(EngagementMeasure::DiskArea) => {
            compute_engagement(grid, nx, ny, tool_radius)
        }
        StepMeasure::Historical(EngagementMeasure::LeadingArc) => {
            compute_engagement_arc(grid, nx, ny, tool_radius, angle)
        }
        StepMeasure::SweptWidth => compute_swept_width(grid, nx, ny, tool_radius, angle),
    }
}

/// The reading a step is held to `load` on: [`measure_engagement`], or,
/// under the swept-width measure where the sliver reading
/// ([`compute_swept_width_with_slivers`]) exceeds `load.sliver_ceiling`,
/// that reading (over the ceiling, so the step is refused).
pub(crate) fn measure_step(
    grid: &MaterialGrid,
    nx: f64,
    ny: f64,
    tool_radius: f64,
    angle: f64,
    load: &PassLoad,
) -> f64 {
    let reading = measure_engagement(grid, nx, ny, tool_radius, angle, load.measure);
    if load.measure != StepMeasure::SweptWidth
        || reading > load.ceiling
        || !load.sliver_ceiling.is_finite()
    {
        return reading;
    }
    let slivers = compute_swept_width_with_slivers(grid, nx, ny, tool_radius, angle);
    if slivers > load.sliver_ceiling {
        slivers
    } else {
        reading
    }
}

/// G-ADAPTPASSLOAD: the one predicate every 2D producer of a cutting step
/// and every keep-down link step is held to. A step to `(nx, ny)` heading
/// `angle`, read on `grid` as cut so far, is within the pass load when its
/// reading does not exceed `load.ceiling`.
pub(crate) fn step_within_pass_load(
    grid: &MaterialGrid,
    nx: f64,
    ny: f64,
    tool_radius: f64,
    angle: f64,
    load: &PassLoad,
) -> bool {
    measure_step(grid, nx, ny, tool_radius, angle, load) <= load.ceiling
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SearchDirectionResult {
    pub(super) angle: f64,
    pub(super) evaluations: u32,
    /// False when the step was taken outside the band (the fallback).
    pub(super) in_band: bool,
}

/// Why the direction search found no step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NoStep {
    /// No candidate cuts material.
    NoMaterial,
    /// Every candidate that cuts material is over the pass ceiling
    /// (G-ADAPTPASSLOAD: the pass ends).
    OverPassLoad,
}

pub(super) fn path_bounds(path: &[P2]) -> Option<ToolpathDebugBounds2> {
    let points: Vec<(f64, f64)> = path.iter().map(|point| (point.x, point.y)).collect();
    ToolpathDebugBounds2::from_points(points.iter())
}

// ── Direction search ───────────────────────────────────────────────────

/// Search for the best direction to move from (cx, cy) that produces
/// engagement closest to `target_frac`.
///
/// Three-phase search:
/// 1. **Narrow interpolation** (7 candidates near prev_angle + bracket refinement)
/// 2. **Forward sweep** ±90° (19 candidates) — fallback
/// 3. **Full 360°** (36 candidates) — allows U-turns
///
/// Phase 1 uses history-predicted interpolation: tries a narrow spread
/// around the previous angle, finds engagement brackets (one above target,
/// one below), then linearly interpolates to converge in 2 extra evaluations.
/// This produces smoother paths (continuous angle function) and typically
/// needs only ~10 evaluations instead of 55.
///
/// When near a wall (boundary_distance < 2 × tool_radius), a tangential
/// bias steers the tool along the wall instead of into it.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn search_direction(
    grid: &MaterialGrid,
    machinable_mask: &[bool],
    cx: f64,
    cy: f64,
    tool_radius: f64,
    step_len: f64,
    target_frac: f64,
    prev_angle: f64,
    boundary_distances: &[f64],
) -> Option<f64> {
    let mut load = PassLoad::historical(1.0, tool_radius, EngagementMeasure::DiskArea);
    load.target = target_frac;
    load.band_floor = (target_frac * (1.0 - PASS_ENGAGEMENT_TOLERANCE)).max(0.005);
    load.ceiling = pass_engagement_ceiling(target_frac);
    search_direction_with_metrics(
        grid,
        machinable_mask,
        cx,
        cy,
        tool_radius,
        step_len,
        &load,
        prev_angle,
        boundary_distances,
        None,
    )
    .ok()
    .map(|result| result.angle)
}

/// Narrow-strip search: when the cutter is in a region too thin for the
/// engagement-target search to swing (boundary_distance ≤ tool_radius +
/// stepover, indicating no room for a stepover step away from any wall),
/// follow the gradient of the boundary-distance field. Picks the
/// direction perpendicular to ∇dt (tangent to the iso-distance curve =
/// the strip's centerline) that's closest to `prev_angle`. The cutter
/// rides the centerline of the strip with one continuous pass instead
/// of wiggling between walls.
///
/// Returns `None` if the next position would leave the machinable
/// region; otherwise returns the chosen angle.
#[allow(clippy::too_many_arguments)]
pub(super) fn search_direction_gradient(
    grid: &MaterialGrid,
    machinable_mask: &[bool],
    boundary_distances: &[f64],
    cx: f64,
    cy: f64,
    step_len: f64,
    prev_angle: f64,
) -> Option<SearchDirectionResult> {
    let (gx, gy) = grid.boundary_gradient(boundary_distances, cx, cy);
    let gmag2 = gx * gx + gy * gy;
    let angle = if gmag2 < 1e-12 {
        // At a local DT maximum (centerline of a strip with uniform
        // cross-section): no preferred direction. Continue in
        // `prev_angle` to keep momentum.
        prev_angle
    } else {
        let grad_angle = gy.atan2(gx);
        let perp_a = grad_angle + std::f64::consts::FRAC_PI_2;
        let perp_b = grad_angle - std::f64::consts::FRAC_PI_2;
        let da = angle_diff(perp_a, prev_angle).abs();
        let db = angle_diff(perp_b, prev_angle).abs();
        if da <= db { perp_a } else { perp_b }
    };

    let nx = cx + step_len * angle.cos();
    let ny = cy + step_len * angle.sin();
    if !grid.is_machinable(machinable_mask, nx, ny) {
        return None;
    }
    Some(SearchDirectionResult {
        angle,
        evaluations: 1,
        in_band: true,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn search_direction_with_metrics(
    grid: &MaterialGrid,
    machinable_mask: &[bool],
    cx: f64,
    cy: f64,
    tool_radius: f64,
    step_len: f64,
    load: &PassLoad,
    prev_angle: f64,
    boundary_distances: &[f64],
    region: Option<&ToolCentreRegion>,
) -> Result<SearchDirectionResult, NoStep> {
    let target_frac = load.target;
    let min_frac = load.band_floor;
    let max_frac = load.ceiling;
    let mut over_ceiling = false;

    let wall_threshold = 2.0 * tool_radius;
    let mut evaluations = 0u32;

    // Helper: evaluate a candidate angle, returns (angle, engagement, score) or None.
    let mut eval_candidate = |angle: f64| -> Option<(f64, f64, f64)> {
        evaluations += 1;
        let nx = cx + step_len * angle.cos();
        let ny = cy + step_len * angle.sin();

        // G-ADAPTPASSLOAD: the 2D rule reads the machinable region itself;
        // the lattice read admits a centre up to a cell diagonal outside it
        // (0.43 mm into an island on the six-island pocket).
        let machinable = match region {
            Some(region) => region.contains_segment(P2::new(cx, cy), P2::new(nx, ny)),
            None => grid.is_machinable(machinable_mask, nx, ny),
        };
        if !machinable {
            return None;
        }

        let engagement = measure_step(grid, nx, ny, tool_radius, angle, load);
        if engagement < load.presence {
            return None;
        }
        if load.caps() && engagement > max_frac {
            // G-ADAPTPASSLOAD: a step over the pass ceiling is not a
            // candidate at all (it may still bracket the target).
            over_ceiling = true;
        }

        let error = (engagement - target_frac).abs();
        let angle_penalty = angle_diff(angle, prev_angle).abs() / PI;

        let wall_bias = {
            let bd = grid.boundary_distance_at(boundary_distances, nx, ny);
            if bd < wall_threshold {
                let (gx, gy) = grid.boundary_gradient(boundary_distances, nx, ny);
                let glen = (gx * gx + gy * gy).sqrt();
                if glen > 1e-10 {
                    let tx = -gy / glen;
                    let ty = gx / glen;
                    let alignment = (angle.cos() * tx + angle.sin() * ty).abs();
                    (1.0 - alignment) * 0.15
                } else {
                    0.0
                }
            } else {
                0.0
            }
        };

        let score = error + angle_penalty * HEADING_CHANGE_WEIGHT + wall_bias;
        Some((angle, engagement, score))
    };

    // ── Phase 1: Narrow interpolation search ──────────────────────────
    // 7 candidates at ±0°, ±15°, ±30°, ±45° from prev_angle
    {
        let offsets = [
            0.0,
            PI / 12.0,
            -PI / 12.0,
            PI / 6.0,
            -PI / 6.0,
            PI / 4.0,
            -PI / 4.0,
        ];
        let mut best_good: Option<(f64, f64)> = None; // (score, angle)
        let mut lo_bracket: Option<(f64, f64, f64)> = None; // (angle, engagement, score)
        let mut hi_bracket: Option<(f64, f64, f64)> = None; // (angle, engagement, score)

        for &offset in &offsets {
            let angle = prev_angle + offset;
            if let Some((angle, eng, score)) = eval_candidate(angle) {
                // Track engagement brackets for interpolation
                if eng < target_frac {
                    if lo_bracket.is_none_or(|b| eng > b.1) {
                        lo_bracket = Some((angle, eng, score));
                    }
                } else if hi_bracket.is_none_or(|b| eng < b.1) {
                    hi_bracket = Some((angle, eng, score));
                }

                if eng >= min_frac && eng <= max_frac && best_good.is_none_or(|b| score < b.0) {
                    best_good = Some((score, angle));
                }
            }
        }

        if let (Some(lo), Some(hi)) = (lo_bracket, hi_bracket)
            && let Some((angle, eng, score)) =
                refine_angle_bracket(lo, hi, target_frac, 8, &mut eval_candidate)
            && eng >= min_frac
            && eng <= max_frac
            && best_good.is_none_or(|b| score < b.0)
        {
            best_good = Some((score, angle));
        }

        if let Some((_, angle)) = best_good {
            return Ok(SearchDirectionResult {
                angle,
                evaluations,
                in_band: true,
            });
        }
    }

    // ── Phase 2: Coarse 360° scan + bracket refinement ────────────────
    // 18 candidates at 20° intervals replaces the old Phase 2 (19 @ ±90°)
    // + Phase 3 (36 @ 360°) = 55 evals. Now ~21 evals total.
    {
        let n_coarse = 36;
        let mut best_good: Option<(f64, f64)> = None; // (score, angle)
        let mut best_any: Option<(f64, f64)> = None;
        let mut coarse_lo: Option<(f64, f64, f64)> = None; // (angle, engagement, score)
        let mut coarse_hi: Option<(f64, f64, f64)> = None; // (angle, engagement, score)

        for i in 0..n_coarse {
            let angle = (i as f64 / n_coarse as f64) * TAU;
            if let Some((angle, eng, score)) = eval_candidate(angle) {
                if eng >= min_frac && eng <= max_frac && best_good.is_none_or(|b| score < b.0) {
                    best_good = Some((score, angle));
                }
                // The historical fallback takes any step; under the 2D cap
                // only a step within the ceiling (a lighter cut).
                if (!load.caps() || eng <= max_frac) && best_any.is_none_or(|b| score < b.0) {
                    best_any = Some((score, angle));
                }
                if eng < target_frac {
                    if coarse_lo
                        .is_none_or(|b| (eng - target_frac).abs() < (b.1 - target_frac).abs())
                    {
                        coarse_lo = Some((angle, eng, score));
                    }
                } else if coarse_hi
                    .is_none_or(|b| (eng - target_frac).abs() < (b.1 - target_frac).abs())
                {
                    coarse_hi = Some((angle, eng, score));
                }
            }
        }

        if let (Some(lo), Some(hi)) = (coarse_lo, coarse_hi)
            && let Some((angle, eng, score)) =
                refine_angle_bracket(lo, hi, target_frac, 8, eval_candidate)
            && eng >= min_frac
            && eng <= max_frac
            && best_good.is_none_or(|b| score < b.0)
        {
            best_good = Some((score, angle));
        }

        if let Some((_, angle)) = best_good {
            return Ok(SearchDirectionResult {
                angle,
                evaluations,
                in_band: true,
            });
        }
        if let Some((_, angle)) = best_any {
            return Ok(SearchDirectionResult {
                angle,
                evaluations,
                in_band: false,
            });
        }
    }
    Err(if over_ceiling {
        NoStep::OverPassLoad
    } else {
        NoStep::NoMaterial
    })
}

// ── Entry point finding ────────────────────────────────────────────────

/// Spatial hash over pass endpoints for the exclusion-radius test.
///
/// The endpoint list is append-only over a run; the old `&[P2]` linear
/// scan made every probed cell / boundary sample O(endpoints), i.e.
/// O(passes × cells) over a job (algorithm review 2026-06-12, F5).
/// Bin size = exclusion radius, so a query only inspects the 3×3
/// neighbourhood of bins. Same membership decisions, bounded cost.
pub(crate) struct EndpointGrid {
    bin: f64,
    min_dist_sq: f64,
    bins: std::collections::HashMap<(i64, i64), Vec<P2>>,
    len: usize,
}

impl EndpointGrid {
    /// `min_dist` is the exclusion radius (callers use 3 × tool radius).
    pub(crate) fn new(min_dist: f64) -> Self {
        let bin = min_dist.max(1e-6);
        Self {
            bin,
            min_dist_sq: min_dist * min_dist,
            bins: std::collections::HashMap::new(),
            len: 0,
        }
    }

    fn key(&self, x: f64, y: f64) -> (i64, i64) {
        ((x / self.bin).floor() as i64, (y / self.bin).floor() as i64)
    }

    pub(crate) fn insert(&mut self, p: P2) {
        let key = self.key(p.x, p.y);
        self.bins.entry(key).or_default().push(p);
        self.len += 1;
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// True when any recorded endpoint lies within the exclusion radius.
    fn any_within(&self, x: f64, y: f64) -> bool {
        let (bx, by) = self.key(x, y);
        for dx in -1..=1i64 {
            for dy in -1..=1i64 {
                if let Some(points) = self.bins.get(&(bx + dx, by + dy))
                    && points.iter().any(|ep| {
                        let ex = x - ep.x;
                        let ey = y - ep.y;
                        ex * ex + ey * ey < self.min_dist_sq
                    })
                {
                    return true;
                }
            }
        }
        false
    }
}

/// Find the nearest material cell that is not near any of the given endpoints.
/// Uses growing-radius search. Falls back to plain nearest material if
/// everything is near an endpoint.
fn find_nearest_material_spread(
    grid: &MaterialGrid,
    x: f64,
    y: f64,
    pass_endpoints: &EndpointGrid,
) -> Option<(f64, f64)> {
    let initial_radius = grid.cell_size * 8.0;
    let max_radius =
        (grid.cols as f64 * grid.cell_size).max(grid.rows as f64 * grid.cell_size) * 1.5;

    let mut radius = initial_radius;
    while radius <= max_radius {
        if let Some(result) =
            find_nearest_material_spread_in_radius(grid, x, y, pass_endpoints, radius)
        {
            return Some(result);
        }
        radius *= 2.0;
    }
    find_nearest_material_spread_in_radius(grid, x, y, pass_endpoints, max_radius)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn find_nearest_material_spread_in_radius(
    grid: &MaterialGrid,
    x: f64,
    y: f64,
    pass_endpoints: &EndpointGrid,
    radius: f64,
) -> Option<(f64, f64)> {
    let col_min = ((x - radius - grid.origin_x) / grid.cell_size)
        .floor()
        .max(0.0) as usize;
    let col_max = ((x + radius - grid.origin_x) / grid.cell_size)
        .ceil()
        .min(grid.cols.saturating_sub(1) as f64) as usize;
    let row_min = ((y - radius - grid.origin_y) / grid.cell_size)
        .floor()
        .max(0.0) as usize;
    let row_max = ((y + radius - grid.origin_y) / grid.cell_size)
        .ceil()
        .min(grid.rows.saturating_sub(1) as f64) as usize;

    let mut best_dist_sq = f64::INFINITY;
    let mut best = None;

    for row in row_min..=row_max {
        let cy = grid.origin_y + row as f64 * grid.cell_size;
        for col in col_min..=col_max {
            if grid.cells[row * grid.cols + col] != CELL_MATERIAL {
                continue;
            }
            let cx = grid.origin_x + col as f64 * grid.cell_size;

            if pass_endpoints.any_within(cx, cy) {
                continue;
            }

            let dx = cx - x;
            let dy = cy - y;
            let d_sq = dx * dx + dy * dy;
            if d_sq < best_dist_sq {
                best_dist_sq = d_sq;
                best = Some((cx, cy));
            }
        }
    }
    best
}

/// Walk the machinable polygon boundary, sampling engagement at regular
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// intervals. Returns the boundary position with the best engagement
/// that isn't too close to a previous endpoint.
///
/// This is more systematic than grid scanning — it checks positions
/// directly on the tool's legal boundary contour, ensuring no regions
/// are missed. Inspired by Freesteel's EngagePoint boundary traversal.
fn walk_boundary_for_entry(
    boundary: &[P2],
    grid: &MaterialGrid,
    tool_radius: f64,
    step: f64,
    pass_endpoints: &EndpointGrid,
) -> Option<(P2, f64)> {
    let mut best: Option<(P2, f64)> = None; // (position, engagement)
    let engage_threshold = 0.005;

    for i in 0..boundary.len() {
        let a = boundary[i];
        let b = boundary[(i + 1) % boundary.len()];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-10 {
            continue;
        }

        let n_samples = (len / step).ceil() as usize;
        for j in 0..=n_samples {
            let t = j as f64 / n_samples.max(1) as f64;
            let x = a.x + t * dx;
            let y = a.y + t * dy;

            // Skip if near a previous endpoint
            if pass_endpoints.any_within(x, y) {
                continue;
            }

            let eng = compute_engagement(grid, x, y, tool_radius);
            if eng > engage_threshold && best.is_none_or(|b| eng > b.1) {
                best = Some((P2::new(x, y), eng));
            }
        }
    }

    best
}

/// Find an entry point at the largest inscribed circle inside the
/// machinable region. Returns the cell with the highest boundary-
/// distance value that's also a local maximum among 8-neighbours and
/// passes the `min_inscribed_radius` gate.
///
/// Used for pass-1 entry on shapes that have an interior open region:
/// starting at a DT maximum gives the cutter symmetric material on
/// all sides, eliminating the corner-entry wiggle that plagues
/// `find_entry_point`'s engagement-ranking approach.
///
/// Returns `None` if no cell meets the inscribed-radius gate — caller
/// should fall back to `find_entry_point` (boundary walk).
///
/// Reference: Bieterman & Sandström (Boeing, ~2003), Ren & Bi (Int. J.
/// Adv. Manuf. Tech., 2014). The "DT max" point is what BobCAD/
/// Fusion/HSMWorks use as their helical-plunge seed.
#[allow(clippy::indexing_slicing)] // bounded grid indexing
pub(crate) fn find_entry_via_distance_transform(
    grid: &MaterialGrid,
    machinable_mask: &[bool],
    boundary_distances: &[f64],
    tool_radius: f64,
) -> Option<P2> {
    // Gate: the cell's inscribed-disk radius must be at least 1.1× tool
    // radius for the helical/center entry to make sense. Below this the
    // region is "strip-mode" — fall back to boundary entry instead.
    let min_inscribed = tool_radius * 1.1;
    let mut best: Option<(f64, P2)> = None;

    for row in 1..grid.rows.saturating_sub(1) {
        for col in 1..grid.cols.saturating_sub(1) {
            let idx = row * grid.cols + col;
            let dt = boundary_distances[idx];
            if dt < min_inscribed || dt.is_infinite() {
                continue;
            }
            // Local-maximum check across the 8 neighbours (slight
            // tolerance to handle the discrete grid's plateaus —
            // require strictly greater than each neighbour, else we
            // can pick any one cell in a flat plateau).
            let mut is_max = true;
            'outer: for dr in [-1i32, 0, 1] {
                for dc in [-1i32, 0, 1] {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    let nr = (row as i32 + dr) as usize;
                    let nc = (col as i32 + dc) as usize;
                    let nidx = nr * grid.cols + nc;
                    if boundary_distances[nidx] > dt + 1e-9 {
                        is_max = false;
                        break 'outer;
                    }
                }
            }
            if !is_max {
                continue;
            }
            let cx = grid.origin_x + col as f64 * grid.cell_size;
            let cy = grid.origin_y + row as f64 * grid.cell_size;
            if !grid.is_machinable(machinable_mask, cx, cy) {
                continue;
            }
            if best.is_none_or(|(b, _)| dt > b) {
                best = Some((dt, P2::new(cx, cy)));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Find an entry point by walking the machinable boundary contours.
///
/// Uses systematic boundary traversal: walks the machinable polygon
/// exterior and hole contours, checking engagement at each position.
/// This ensures no uncleared regions along walls are missed.
/// Falls back to grid scan for interior material not reachable from boundary.
pub(crate) fn find_entry_point(
    grid: &MaterialGrid,
    machinable_mask: &[bool],
    machinable: &Polygon2,
    tool_radius: f64,
    last_pos: Option<P2>,
    pass_endpoints: &EndpointGrid,
) -> Option<P2> {
    let walk_step = grid.cell_size * 2.0;

    // Phase 1: Walk the machinable boundary contours
    // Check exterior
    let mut best_boundary: Option<(P2, f64)> = walk_boundary_for_entry(
        &machinable.exterior,
        grid,
        tool_radius,
        walk_step,
        pass_endpoints,
    );

    // Check hole boundaries
    for hole in &machinable.holes {
        if let Some((p, eng)) =
            walk_boundary_for_entry(hole, grid, tool_radius, walk_step, pass_endpoints)
            && best_boundary.is_none_or(|b| eng > b.1)
        {
            best_boundary = Some((p, eng));
        }
    }

    if let Some((p, _)) = best_boundary {
        return Some(p);
    }

    // Phase 2: Fallback to grid scan for interior material
    let search_from = last_pos.unwrap_or_else(|| {
        let cx = grid.origin_x + (grid.cols as f64 * grid.cell_size) / 2.0;
        let cy = grid.origin_y + (grid.rows as f64 * grid.cell_size) / 2.0;
        P2::new(cx, cy)
    });

    let (mx, my) = if !pass_endpoints.is_empty() {
        find_nearest_material_spread(grid, search_from.x, search_from.y, pass_endpoints)
            .or_else(|| grid.find_nearest_material(search_from.x, search_from.y))
    } else {
        grid.find_nearest_material(search_from.x, search_from.y)
    }?;

    if grid.is_machinable(machinable_mask, mx, my) {
        return Some(P2::new(mx, my));
    }

    // Search nearby for a machinable cell
    let search_r = tool_radius * 3.0;
    let step = grid.cell_size;
    let mut best_dist_sq = f64::INFINITY;
    let mut best = None;

    let steps = (search_r / step).ceil() as i32;
    for ri in -steps..=steps {
        let y = my + ri as f64 * step;
        for ci in -steps..=steps {
            let x = mx + ci as f64 * step;
            if grid.is_machinable(machinable_mask, x, y) {
                let engagement = compute_engagement(grid, x, y, tool_radius);
                if engagement > 0.005 {
                    let dx = x - mx;
                    let dy = y - my;
                    let d_sq = dx * dx + dy * dy;
                    if d_sq < best_dist_sq {
                        best_dist_sq = d_sq;
                        best = Some(P2::new(x, y));
                    }
                }
            }
        }
    }

    best
}
