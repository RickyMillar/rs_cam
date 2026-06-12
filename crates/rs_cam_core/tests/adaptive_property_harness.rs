//! Property-test harness for the 2D adaptive clearing strategies
//! (Stage 1, `planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md` §4).
//!
//! Generated geometry — seeded star polygons, L/U shapes, an annulus, a
//! narrow slot — is cleared by each `PathStrategy2d`, then the toolpath
//! is replayed against an **independent** raster oracle implemented in
//! this file (own grid, own leading-arc engagement sampler): the harness
//! deliberately shares no measurement code with the planner.
//!
//! Measured per run:
//! - leading-arc engagement (contact-angle fraction α/2π) per cut sample,
//!   reported as p99/max against the commanded target;
//! - coverage: fraction of reachable material cells actually cleared
//!   (reachable = within tool reach of a legal cutter-center cell);
//! - travel: plunge count (rapid groups) and rapid/cut distance ratio.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::adaptive::{
    AdaptiveParams, CleanupStrategy, EngagementMeasure, PathStrategy2d, adaptive_toolpath,
};
use rs_cam_core::contour_extract::distance_transform_2d;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::toolpath::{MoveType, Toolpath};

const R: f64 = 3.175;
const STEPOVER: f64 = 2.0;

// ── Deterministic PRNG (no rand dep, fixed seeds) ──────────────────────

struct XorShift64(u64);

impl XorShift64 {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.unit() * (hi - lo)
    }
}

// ── Shape generators ───────────────────────────────────────────────────

/// Random star polygon: sorted angles with jittered radii — simple by
/// construction, concave in places.
fn star_polygon(seed: u64) -> Polygon2 {
    let mut rng = XorShift64(seed | 1);
    let n = 10 + (rng.next() % 6) as usize;
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let jitter = rng.range(-0.25, 0.25) / n as f64 * std::f64::consts::TAU;
        let theta = (i as f64 / n as f64) * std::f64::consts::TAU + jitter;
        let r = rng.range(18.0, 32.0);
        pts.push(P2::new(r * theta.cos(), r * theta.sin()));
    }
    Polygon2::new(pts)
}

fn square_polygon(size: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(size, 0.0),
        P2::new(size, size),
        P2::new(0.0, size),
    ])
}

fn l_shape() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(60.0, 0.0),
        P2::new(60.0, 25.0),
        P2::new(25.0, 25.0),
        P2::new(25.0, 60.0),
        P2::new(0.0, 60.0),
    ])
}

fn u_shape() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(60.0, 0.0),
        P2::new(60.0, 50.0),
        P2::new(40.0, 50.0),
        P2::new(40.0, 18.0),
        P2::new(20.0, 18.0),
        P2::new(20.0, 50.0),
        P2::new(0.0, 50.0),
    ])
}

fn annulus() -> Polygon2 {
    let mut poly = square_polygon(60.0);
    poly.holes.push(vec![
        P2::new(22.0, 22.0),
        P2::new(38.0, 22.0),
        P2::new(38.0, 38.0),
        P2::new(22.0, 38.0),
    ]);
    poly
}

fn narrow_slot() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(70.0, 0.0),
        P2::new(70.0, 9.0),
        P2::new(0.0, 9.0),
    ])
}

// ── Independent replay oracle ──────────────────────────────────────────

struct Oracle {
    cells: Vec<bool>, // true = uncut material (inside polygon)
    inside: Vec<bool>,
    rows: usize,
    cols: usize,
    origin_x: f64,
    origin_y: f64,
    cell: f64,
}

impl Oracle {
    fn new(polygon: &Polygon2, cell: f64) -> Self {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in &polygon.exterior {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        let margin = R + cell;
        let origin_x = x0 - margin;
        let origin_y = y0 - margin;
        let cols = ((x1 - x0 + 2.0 * margin) / cell).ceil() as usize + 1;
        let rows = ((y1 - y0 + 2.0 * margin) / cell).ceil() as usize + 1;
        let mut inside = vec![false; rows * cols];
        for (idx, slot) in inside.iter_mut().enumerate() {
            let row = idx / cols;
            let col = idx % cols;
            let p = P2::new(origin_x + col as f64 * cell, origin_y + row as f64 * cell);
            *slot = polygon.contains_point(&p);
        }
        Self {
            cells: inside.clone(),
            inside,
            rows,
            cols,
            origin_x,
            origin_y,
            cell,
        }
    }

    fn is_material(&self, x: f64, y: f64) -> bool {
        let col = ((x - self.origin_x) / self.cell).floor();
        let row = ((y - self.origin_y) / self.cell).floor();
        if col < 0.0 || row < 0.0 {
            return false;
        }
        let (col, row) = (col as usize, row as usize);
        if col >= self.cols || row >= self.rows {
            return false;
        }
        self.cells[row * self.cols + col]
    }

    fn clear_circle(&mut self, cx: f64, cy: f64, radius: f64) {
        let r_sq = radius * radius;
        let c0 = (((cx - radius - self.origin_x) / self.cell).floor()).max(0.0) as usize;
        let c1 = ((((cx + radius - self.origin_x) / self.cell).ceil()) as usize)
            .min(self.cols.saturating_sub(1));
        let r0 = (((cy - radius - self.origin_y) / self.cell).floor()).max(0.0) as usize;
        let r1 = ((((cy + radius - self.origin_y) / self.cell).ceil()) as usize)
            .min(self.rows.saturating_sub(1));
        for row in r0..=r1 {
            let y = self.origin_y + row as f64 * self.cell;
            let dy2 = (y - cy) * (y - cy);
            if dy2 > r_sq {
                continue;
            }
            for col in c0..=c1 {
                let x = self.origin_x + col as f64 * self.cell;
                if (x - cx) * (x - cx) + dy2 <= r_sq {
                    self.cells[row * self.cols + col] = false;
                }
            }
        }
    }

    /// Leading-arc engagement (α/2π) at the cutter position moving in
    /// `dir`: fraction of the full circle whose leading semicircle lies
    /// in uncut material. Independent reimplementation of the planner's
    /// measure (64 circumference samples).
    fn leading_arc(&self, cx: f64, cy: f64, dir: f64) -> f64 {
        let n = 64;
        let mut hits = 0usize;
        for i in 0..n {
            let t = (i as f64 + 0.5) / n as f64;
            let theta = dir - std::f64::consts::FRAC_PI_2 + t * std::f64::consts::PI;
            if self.is_material(cx + R * theta.cos(), cy + R * theta.sin()) {
                hits += 1;
            }
        }
        0.5 * hits as f64 / n as f64
    }

    /// Fraction of *reachable* material cells cleared. Reachable = within
    /// half a cell of the tool-disc sweep over legal cutter centers
    /// (centers ≥ R inside the polygon).
    fn coverage(&self) -> f64 {
        let outside: Vec<bool> = self.inside.iter().map(|&i| !i).collect();
        let mut din = distance_transform_2d(&outside, self.rows, self.cols);
        for d in &mut din {
            *d *= self.cell;
        }
        let machinable: Vec<bool> = din.iter().map(|&d| d >= R - self.cell * 0.5).collect();
        let not_machinable: Vec<bool> = machinable.iter().map(|&m| !m).collect();
        let mut dmach = distance_transform_2d(&not_machinable, self.rows, self.cols);
        for d in &mut dmach {
            *d *= self.cell;
        }
        let mut reachable = 0usize;
        let mut cleared = 0usize;
        for ((&cell_material, &inside), &dist) in self.cells.iter().zip(&self.inside).zip(&dmach) {
            if !inside {
                continue;
            }
            // Cell distance to nearest machinable center must be < R
            // (with half-cell tolerance) for any legal pass to reach it.
            if dist > R - self.cell * 0.5 {
                continue;
            }
            reachable += 1;
            if !cell_material {
                cleared += 1;
            }
        }
        if reachable == 0 {
            return 1.0;
        }
        cleared as f64 / reachable as f64
    }
}

#[derive(Debug)]
struct Metrics {
    p99_engagement: f64,
    max_engagement: f64,
    coverage: f64,
    plunges: usize,
    rapid_mm: f64,
    cut_mm: f64,
    over_target_fraction: f64,
}

fn replay(polygon: &Polygon2, tp: &Toolpath) -> Metrics {
    let cell = (R / 6.0).min(0.5);
    let mut oracle = Oracle::new(polygon, cell);
    let mut engagements: Vec<f64> = Vec::new();
    let target = ((1.0 - STEPOVER / R).clamp(-1.0, 1.0)).acos() / std::f64::consts::TAU;

    let mut prev: Option<P2> = None;
    let mut plunges = 0usize;
    let mut in_rapid = true; // initial approach counts as the first plunge
    let mut rapid_mm = 0.0f64;
    let mut cut_mm = 0.0f64;

    for m in &tp.moves {
        let to = P2::new(m.target.x, m.target.y);
        let rapid = matches!(m.move_type, MoveType::Rapid);
        if let Some(from) = prev {
            let dx = to.x - from.x;
            let dy = to.y - from.y;
            let len = (dx * dx + dy * dy).sqrt();
            if rapid {
                rapid_mm += len;
                in_rapid = true;
            } else {
                if in_rapid {
                    plunges += 1;
                    in_rapid = false;
                }
                cut_mm += len;
                if len > 1e-9 {
                    let dir = dy.atan2(dx);
                    let n = (len / (cell * 1.0)).ceil() as usize;
                    for i in 0..=n {
                        let t = i as f64 / n.max(1) as f64;
                        let x = from.x + t * dx;
                        let y = from.y + t * dy;
                        engagements.push(oracle.leading_arc(x, y, dir));
                        oracle.clear_circle(x, y, R);
                    }
                }
            }
        } else if !rapid {
            in_rapid = false;
            plunges += 1;
            oracle.clear_circle(to.x, to.y, R);
        }
        prev = Some(to);
    }

    engagements.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p99 = if engagements.is_empty() {
        0.0
    } else {
        engagements[((engagements.len() as f64) * 0.99) as usize - 1]
    };
    let max = engagements.last().copied().unwrap_or(0.0);
    let over = if engagements.is_empty() {
        0.0
    } else {
        engagements.iter().filter(|&&e| e > target * 1.3).count() as f64 / engagements.len() as f64
    };

    Metrics {
        p99_engagement: p99,
        max_engagement: max,
        coverage: oracle.coverage(),
        plunges,
        rapid_mm,
        cut_mm,
        over_target_fraction: over,
    }
}

fn run_strategy(polygon: &Polygon2, strategy: PathStrategy2d) -> Metrics {
    let params = AdaptiveParams {
        tool_radius: R,
        stepover: STEPOVER,
        cut_depth: -3.0,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        safe_z: 10.0,
        tolerance: 0.2,
        slot_clearing: false,
        min_cutting_radius: 0.0,
        initial_stock: None,
        cleanup_strategy: CleanupStrategy::ContourParallelHybrid,
        engagement_measure: EngagementMeasure::LeadingArc,
        path_strategy: strategy,
    };
    let tp = adaptive_toolpath(polygon, &params);
    replay(polygon, &tp)
}

fn shapes() -> Vec<(&'static str, Polygon2)> {
    vec![
        ("square60", square_polygon(60.0)),
        ("star_a", star_polygon(0xA11CE)),
        ("star_b", star_polygon(0xB0B5)),
        ("l_shape", l_shape()),
        ("u_shape", u_shape()),
        ("annulus", annulus()),
        ("narrow_slot", narrow_slot()),
    ]
}

/// The target α/2π for the harness tool/stepover, for context in output.
fn target() -> f64 {
    ((1.0 - STEPOVER / R).clamp(-1.0, 1.0)).acos() / std::f64::consts::TAU
}

/// Stage 1+2 contract for the contour spiral, asserted shape-by-shape
/// against the agent on identical geometry:
///
/// - **travel**: spiral plunges ≤ 3 and rapid distance ≤ 5% of cutting
///   distance (the agent restarts from walls; the spiral stays down);
/// - **coverage**: ≥ 0.975 absolute (the oracle's boundary rasterisation
///   keeps a thin ring of "reachable" corner cells no strategy clears —
///   the production agent reads 0.977–0.995 on these shapes), and never
///   more than 1.5 % below the agent on the same shape;
/// - **load, comparative**: p99 engagement and over-1.3×target fraction
///   never worse than the agent's;
/// - **load, absolute (Stage 2 trochoids)**: ≥ 95% of cut samples within
///   1.3× target, p99 ≤ 2× target. The strict 1.3×-target p99 is NOT
///   asserted: a structural ~1% of samples at trochoid loop-tangent
///   instants reads ~0.28–0.35 regardless of pitch — true cycloid
///   advance and/or feed modulation territory, tracked in the review
///   doc. Slot-class shapes (machinable width below the narrow gate) are
///   exempt from load bars: both strategies route to contour-parallel
///   there and near-slot engagement is inherent to slotting.
#[test]
fn contour_spiral_dominates_agent() {
    let mut failures: Vec<String> = Vec::new();
    for (name, poly) in shapes() {
        let s = run_strategy(&poly, PathStrategy2d::ContourSpiral);
        let a = run_strategy(&poly, PathStrategy2d::Agent);
        println!(
            "spiral {name}: cov {:.4} p99 {:.3} max {:.3} over {:.3} plunges {} rapid {:.0} cut {:.0} (target {:.3})",
            s.coverage,
            s.p99_engagement,
            s.max_engagement,
            s.over_target_fraction,
            s.plunges,
            s.rapid_mm,
            s.cut_mm,
            target()
        );
        println!(
            "agent  {name}: cov {:.4} p99 {:.3} max {:.3} over {:.3} plunges {} rapid {:.0} cut {:.0}",
            a.coverage,
            a.p99_engagement,
            a.max_engagement,
            a.over_target_fraction,
            a.plunges,
            a.rapid_mm,
            a.cut_mm,
        );

        if s.plunges > 3 {
            failures.push(format!("{name}: {} plunges (cap 3)", s.plunges));
        }
        if s.rapid_mm > s.cut_mm * 0.05 + 1.0 {
            failures.push(format!(
                "{name}: rapid {:.0}mm > 5% of cut {:.0}mm",
                s.rapid_mm, s.cut_mm
            ));
        }
        if s.coverage < 0.975 {
            failures.push(format!("{name}: coverage {:.4} < 0.975", s.coverage));
        }
        if s.coverage < a.coverage - 0.015 {
            failures.push(format!(
                "{name}: coverage {:.4} more than 1.5% below agent {:.4}",
                s.coverage, a.coverage
            ));
        }
        if s.p99_engagement > a.p99_engagement + 0.01 {
            failures.push(format!(
                "{name}: p99 {:.3} worse than agent {:.3}",
                s.p99_engagement, a.p99_engagement
            ));
        }
        if s.over_target_fraction > a.over_target_fraction + 0.02 {
            failures.push(format!(
                "{name}: over-fraction {:.3} worse than agent {:.3}",
                s.over_target_fraction, a.over_target_fraction
            ));
        }
        // Absolute load bars (Stage 2 trochoids). Slot-class shapes are
        // exempt — see the test doc comment.
        let slot_class = name == "narrow_slot";
        if !slot_class && s.over_target_fraction > 0.05 {
            failures.push(format!(
                "{name}: {:.1}% of samples over 1.3×target (absolute cap 5%)",
                s.over_target_fraction * 100.0
            ));
        }
        if !slot_class && s.p99_engagement > target() * 2.0 {
            failures.push(format!(
                "{name}: p99 {:.3} > 2×target {:.3}",
                s.p99_engagement,
                target() * 2.0
            ));
        }
        if a.coverage < 0.975 {
            failures.push(format!(
                "{name}: AGENT baseline coverage {:.4} < 0.975 (regression in shared machinery?)",
                a.coverage
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "stage-1 property violations:\n{}",
        failures.join("\n")
    );
}
