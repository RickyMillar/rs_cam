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
use rs_cam_core::geo::P2;
use rs_cam_core::geometry::grid_field::distance_transform_2d;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::toolpath::{MoveType, Toolpath};

const R: f64 = 3.175;

/// The default helix entry radius the product's dressup cuts for 2D
/// Adaptive: `HELIX_RADIUS_OVER_D` x D.
fn helix_radius() -> f64 {
    rs_cam_core::compute::config::HELIX_RADIUS_OVER_D * 2.0 * R
}
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
    // `Polygon2` holds holes clockwise. This hole was written
    // counter-clockwise, and the offset then read the island as machinable:
    // the planner entered inside it (found 2026-09-26 when the oracle began
    // to read each entry's wall distance). Normalised here.
    poly.ensure_winding();
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
    /// Exact distance from each inside lattice point to the walls.
    wall: Vec<f64>,
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
        let wall = inside
            .iter()
            .enumerate()
            .map(|(idx, &inside)| {
                if !inside {
                    return 0.0;
                }
                let p = P2::new(
                    origin_x + (idx % cols) as f64 * cell,
                    origin_y + (idx / cols) as f64 * cell,
                );
                wall_distance(polygon, p)
            })
            .collect();
        Self {
            cells: inside.clone(),
            inside,
            wall,
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

    /// Radial immersion at the cutter position moving in `dir`: the
    /// sideways extent (perpendicular to `dir`) of the material cells in the
    /// cutter disc, over D. Independent reimplementation of the product's
    /// load measure (G-ADAPTPASSLOAD, `step_within_pass_load`), the
    /// simulator's radial model.
    fn swept_width(&self, cx: f64, cy: f64, dir: f64) -> f64 {
        let (sx, sy) = (-dir.sin(), dir.cos());
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let c0 = (((cx - R - self.origin_x) / self.cell).floor()).max(0.0) as usize;
        let c1 = ((((cx + R - self.origin_x) / self.cell).ceil()) as usize)
            .min(self.cols.saturating_sub(1));
        let r0 = (((cy - R - self.origin_y) / self.cell).floor()).max(0.0) as usize;
        let r1 = ((((cy + R - self.origin_y) / self.cell).ceil()) as usize)
            .min(self.rows.saturating_sub(1));
        for row in r0..=r1 {
            let dy = self.origin_y + row as f64 * self.cell - cy;
            for col in c0..=c1 {
                let dx = self.origin_x + col as f64 * self.cell - cx;
                if dx * dx + dy * dy <= R * R && self.cells[row * self.cols + col] {
                    let u = dx * sx + dy * sy;
                    lo = lo.min(u);
                    hi = hi.max(u);
                }
            }
        }
        if hi < lo { 0.0 } else { (hi - lo) / (2.0 * R) }
    }

    /// Fraction of *reachable* material cells cleared. Reachable = within R
    /// of a legal cutter centre, legal = a lattice point whose exact
    /// distance to the walls (`wall_distance`, this file's own) is at least
    /// R. A cell reachable only from an off-lattice centre is left out (the
    /// reading is lenient by under one oracle cell at the edge of reach).
    ///
    /// Before 2026-09-26 (round 3) this read the distance transform of the
    /// NON-machinable cells (`distance_transform_2d` measures the distance
    /// to the nearest `true` cell), so "reachable" was the band within R of
    /// the unreachable set, including the unreachable star tips themselves
    /// (43 of 110 uncut cells on `star_a` lay over R from any legal
    /// centre, up to 4.6 mm), and the interior was not read at all.
    fn coverage(&self) -> f64 {
        let legal: Vec<bool> = self
            .inside
            .iter()
            .zip(&self.wall)
            .map(|(&inside, &w)| inside && w >= R)
            .collect();
        let mut dmach = distance_transform_2d(&legal, self.rows, self.cols);
        for d in &mut dmach {
            *d *= self.cell;
        }
        let mut reachable = 0usize;
        let mut cleared = 0usize;
        for ((&cell_material, &inside), &dist) in self.cells.iter().zip(&self.inside).zip(&dmach) {
            if !inside {
                continue;
            }
            // A legal centre within R reaches the cell.
            if dist > R {
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
    /// Swept width (radial immersion) over the cut samples that meet
    /// stock: p99, max, and the fraction over the pass ceiling plus one
    /// oracle cell over D.
    p99_width: f64,
    max_width: f64,
    over_ceiling_fraction: f64,
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
    let mut widths: Vec<f64> = Vec::new();
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
                // A vertical feed is an entry. The product's default entry
                // for 2D Adaptive is the helix dressup (radius 0.3 x D,
                // ending on a flat lap, shrunk to stay off the walls since
                // G-ADAPTPASSLOAD round 3), which cuts a disc of R + that
                // radius; the planner stamps the same hole. This harness
                // calls the planner without dressups, so it cuts the hole
                // here, with its own wall distance.
                if len <= 1e-9 && m.target.z < -1e-9 {
                    let fit = (wall_distance(polygon, to) - R).clamp(0.0, helix_radius());
                    oracle.clear_circle(to.x, to.y, R + fit);
                }
                if len > 1e-9 {
                    let dir = dy.atan2(dx);
                    let n = (len / (cell * 1.0)).ceil() as usize;
                    for i in 0..=n {
                        let t = i as f64 / n.max(1) as f64;
                        let x = from.x + t * dx;
                        let y = from.y + t * dy;
                        engagements.push(oracle.leading_arc(x, y, dir));
                        let w = oracle.swept_width(x, y, dir);
                        if w > 0.0 {
                            widths.push(w);
                        }
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

    widths.sort_by(f64::total_cmp);
    let p99_width = widths
        .get(((widths.len() as f64) * 0.99) as usize)
        .copied()
        .unwrap_or(0.0);
    let max_width = widths.last().copied().unwrap_or(0.0);
    let bar = pass_ceiling_radial() + cell / (2.0 * R);
    let over_ceiling_fraction = if widths.is_empty() {
        0.0
    } else {
        widths.iter().filter(|&&w| w > bar).count() as f64 / widths.len() as f64
    };
    Metrics {
        p99_width,
        max_width,
        over_ceiling_fraction,
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
    run_strategy_capped(polygon, strategy, 1.2)
}

fn run_strategy_capped(polygon: &Polygon2, strategy: PathStrategy2d, cap_mult: f64) -> Metrics {
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
        trochoid_cap_mult: cap_mult,
        keep_down_links: rs_cam_core::adaptive::KeepDownLinks::WithinPassLoad,
        entry_helix_radius: helix_radius(),
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

/// Distance from `p` to the nearest edge of `polygon` (exterior or hole).
fn wall_distance(polygon: &Polygon2, p: P2) -> f64 {
    let mut d = f64::INFINITY;
    for ring in std::iter::once(&polygon.exterior).chain(polygon.holes.iter()) {
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let l2 = dx * dx + dy * dy;
            let t = if l2 > 0.0 {
                (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            d = d.min((p.x - a.x - t * dx).hypot(p.y - a.y - t * dy));
        }
    }
    d
}

/// The product's pass ceiling as a radial fraction (G-ADAPTPASSLOAD).
fn pass_ceiling_radial() -> f64 {
    rs_cam_core::ops::adaptive_shared::radial_woc_fraction_from_leading_arc(
        rs_cam_core::adaptive::pass_engagement_limit(STEPOVER, R),
    )
}

/// The target α/2π for the harness tool/stepover, for context in output.
fn target() -> f64 {
    ((1.0 - STEPOVER / R).clamp(-1.0, 1.0)).acos() / std::f64::consts::TAU
}

/// The load both 2D strategies are held to, on identical geometry, under
/// the product's one load definition (G-ADAPTPASSLOAD, operator decision
/// 2026-09-26): radial immersion = swept width (sideways stock extent / D),
/// read here by this file's own oracle.
///
/// Until 2026-09-26 this test was `contour_spiral_dominates_agent` and read
/// the leading-arc fraction alpha/2pi, the quantity the spiral's trochoid
/// trigger used. Under the operator's rule both strategies are held to the
/// same swept-width ceiling, so the load comparison is made in that measure:
///
/// - **load, absolute**: p99 swept width <= the pass ceiling plus the
///   discretisation between the planner and this oracle, (planner cell +
///   oracle cell + tolerance) / D (the derivation of the six-island
///   sentry's tolerance, `common::adaptive_islands`), for both strategies;
/// - **load, comparative**: the spiral's p99 is not worse than the agent's
///   by more than one oracle cell over D (the reading resolution);
/// - **coverage**: >= 0.975 absolute for both, and the spiral never more
///   than 1.5 % below the agent. Reachable is read exactly since round 3
///   (`Oracle::coverage`); before, it read the band within R of the
///   unreachable cells, star tips included.
///
/// Slot-class shapes (machinable width below the narrow gate) are exempt
/// from the load bars: no helix fits them, so their entries are straight
/// plunges whose first steps are exempt (`PassLoad::departing`).
///
/// The spiral's former travel contract (at most 3 plunges, rapids at most
/// 5 % of the cut) is retired: see `contour_spiral_travel_contract`.
#[test]
fn both_strategies_hold_the_pass_load() {
    let mut failures: Vec<String> = Vec::new();
    let cell = (R / 6.0).min(0.5);
    let planner_cell = (R / 6.0).max(0.2);
    let absolute = pass_ceiling_radial() + (planner_cell + cell + 0.2) / (2.0 * R);
    let resolution = cell / (2.0 * R);
    for (name, poly) in shapes() {
        let s = run_strategy(&poly, PathStrategy2d::ContourSpiral);
        let a = run_strategy(&poly, PathStrategy2d::Agent);
        println!(
            "width  {name}: spiral p99 {:.3} max {:.3} over {:.4} | agent p99 {:.3} max {:.3} over {:.4} (ceiling {:.3}, bar {absolute:.3})",
            s.p99_width,
            s.max_width,
            s.over_ceiling_fraction,
            a.p99_width,
            a.max_width,
            a.over_ceiling_fraction,
            pass_ceiling_radial()
        );
        println!(
            "spiral {name}: cov {:.4} plunges {} rapid {:.0} cut {:.0} (alpha/2pi p99 {:.3} max {:.3})",
            s.coverage, s.plunges, s.rapid_mm, s.cut_mm, s.p99_engagement, s.max_engagement
        );
        println!(
            "agent  {name}: cov {:.4} plunges {} rapid {:.0} cut {:.0} (alpha/2pi p99 {:.3} max {:.3})",
            a.coverage, a.plunges, a.rapid_mm, a.cut_mm, a.p99_engagement, a.max_engagement
        );
        let slot_class = name == "narrow_slot";
        if !slot_class {
            for (who, m) in [("spiral", &s), ("agent", &a)] {
                if m.p99_width > absolute {
                    failures.push(format!(
                        "{name}: {who} p99 swept width {:.3} > {absolute:.3}",
                        m.p99_width
                    ));
                }
            }
            if s.p99_width > a.p99_width + resolution {
                failures.push(format!(
                    "{name}: spiral p99 swept width {:.3} worse than agent {:.3}",
                    s.p99_width, a.p99_width
                ));
            }
        }
        for (who, m) in [("spiral", &s), ("agent", &a)] {
            if m.coverage < 0.975 {
                failures.push(format!("{name}: {who} coverage {:.4} < 0.975", m.coverage));
            }
        }
        if s.coverage < a.coverage - 0.015 {
            failures.push(format!(
                "{name}: spiral coverage {:.4} more than 1.5% below agent {:.4}",
                s.coverage, a.coverage
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "pass-load property violations:\n{}",
        failures.join("\n")
    );
}

/// The spiral's Stage 1 travel contract: one continuous stay-down pass, at
/// most 3 plunges, rapids at most 5 % of the cut. It held under the
/// leading-arc trigger (cap 1.2 x target). Under the swept-width pass load
/// (G-ADAPTPASSLOAD) the spiral's trochoid inserts do not bound the swept
/// width (a loop meets the frontier across a chord of the pitch-deep cap,
/// 0.8 of D at pitch 0.6 s), so the replay splits them and the spiral
/// re-enters: 56-119 plunges on the harness shapes (2026-09-26, round 3).
/// The claim is RETIRED (lead decision, G-ADAPTPASSLOAD round 3): no doc
/// or help text states it; this test is kept, ignored, as the record of
/// what the spiral once held and why it no longer does.
#[test]
#[ignore = "retired claim: under G-ADAPTPASSLOAD the spiral's trochoids exceed the swept-width ceiling and it re-enters"]
fn contour_spiral_travel_contract() {
    let mut failures: Vec<String> = Vec::new();
    for (name, poly) in shapes() {
        let s = run_strategy(&poly, PathStrategy2d::ContourSpiral);
        if s.plunges > 3 {
            failures.push(format!("{name}: {} plunges (cap 3)", s.plunges));
        }
        if s.rapid_mm > s.cut_mm * 0.05 + 1.0 {
            failures.push(format!(
                "{name}: rapid {:.0}mm > 5% of cut {:.0}mm",
                s.rapid_mm, s.cut_mm
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "travel contract:\n{}",
        failures.join("\n")
    );
}

#[test]
#[ignore]
fn trochoid_cap_distance_load_tradeoff() {
    let tgt = target();
    println!("\n=== trochoid cap sweep (target a/2pi = {tgt:.3}, over% = >1.3x target) ===");
    println!(
        "{:<13} {:>8} {:>9} {:>6} {:>6} {:>6}",
        "shape", "cap_mult", "cut_mm", "p99", "over%", "cov"
    );
    for (name, poly) in shapes() {
        for cap in [1.0_f64, 1.2, 1.5, 2.0, 3.0, 10.0] {
            let s = run_strategy_capped(&poly, PathStrategy2d::ContourSpiral, cap);
            println!(
                "{name:<13} {cap:>8.1} {:>9.0} {:>6.3} {:>6.1} {:>6.3}",
                s.cut_mm,
                s.p99_engagement,
                s.over_target_fraction * 100.0,
                s.coverage
            );
        }
        let a = run_strategy(&poly, PathStrategy2d::Agent);
        println!(
            "{name:<13} {:>8} {:>9.0} {:>6.3} {:>6.1} {:>6.3}",
            "agent",
            a.cut_mm,
            a.p99_engagement,
            a.over_target_fraction * 100.0,
            a.coverage
        );
    }
}
