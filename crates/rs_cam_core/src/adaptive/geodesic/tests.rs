//! Unit tests of the Geodesic Adaptive plan: the field, the chaining, the
//! level spacing, the star-shape test and the core's own loops. They moved
//! from the research module in integration step 7.3d, with the code they
//! test. The helpers below give a test the field of a part on its own
//! lattice, which the door does not need.

#![expect(
    clippy::panic,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::f64::consts::{PI, SQRT_2, TAU};

use crate::adaptive::path::{pass_load_for, planner_cell_size};
use crate::adaptive::search::ToolCentreRegion;
use crate::adaptive::{AdaptiveParams, CleanupStrategy, KeepDownLinks, PathStrategy2d};
use crate::geo::P2;
use crate::interrupt::CancelCheck;
use crate::polygon::Polygon2;

use super::field::{GeodesicField, Lattice, Walls, chain_whole};
use super::wraps::{Chain, SpiralCore, beside_core, is_closed};

/// The 6 mm end mill of the 2D fixtures (`test_data/ux_2d_*.toml`).
const R: f64 = 3.0;

/// The lattice `MaterialGrid::from_polygon` builds over `region`: its
/// bounding box with a margin of one cell on each side.
fn lattice_over(region: &Polygon2, cell: f64) -> Lattice {
    let (mut lo_x, mut lo_y) = (f64::INFINITY, f64::INFINITY);
    let (mut hi_x, mut hi_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in &region.exterior {
        lo_x = lo_x.min(p.x);
        lo_y = lo_y.min(p.y);
        hi_x = hi_x.max(p.x);
        hi_y = hi_y.max(p.y);
    }
    let cols = ((hi_x - lo_x + 2.0 * cell) / cell).ceil() as usize + 1;
    let rows = ((hi_y - lo_y + 2.0 * cell) / cell).ceil() as usize + 1;
    Lattice {
        rows,
        cols,
        origin_x: lo_x - cell,
        origin_y: lo_y - cell,
        cell,
    }
}

/// The field of `part` (its tool-centre region for a tool of
/// `tool_radius`) on its own lattice, from the seed disc of `seed_radius`
/// about `seed`.
fn over_part(
    part: &Polygon2,
    tool_radius: f64,
    cell: f64,
    stepover: f64,
    seed: P2,
    seed_radius: f64,
) -> Option<GeodesicField> {
    let region = ToolCentreRegion::new(part, tool_radius);
    let lattice = lattice_over(part, cell);
    let inside = (0..lattice.rows * lattice.cols)
        .map(|i| region.contains(&lattice.node(i)))
        .collect();
    let walls = Walls { region: &region };
    GeodesicField::on_lattice(lattice, inside, seed, seed_radius, stepover, &walls)
}

/// The node of `field` nearest `p`.
fn nearest_node(field: &GeodesicField, p: P2) -> Option<usize> {
    let f = &field.field;
    let col = ((p.x - f.origin_x) / f.cell).round();
    let row = ((p.y - f.origin_y) / f.cell).round();
    if col < 0.0 || row < 0.0 {
        return None;
    }
    let (row, col) = (row as usize, col as usize);
    (row < f.rows && col < f.cols).then_some(row * f.cols + col)
}

/// `T` (mm) at the node nearest `p`; `None` at an unreachable node.
fn distance_mm(field: &GeodesicField, p: P2) -> Option<f64> {
    let i = nearest_node(field, p)?;
    let t = *field.field.passes.get(i)?;
    (field.inside.get(i).copied() == Some(true) && t.is_finite()).then_some(t * field.stepover)
}

/// The front branches that hold a reachable node.
fn branch_count(field: &GeodesicField) -> usize {
    let mut seen: Vec<u32> = field
        .branch
        .iter()
        .zip(&field.inside)
        .filter(|&(_, &reach)| reach)
        .map(|(&b, _)| b)
        .collect();
    seen.sort_unstable();
    seen.dedup();
    seen.len()
}

/// The level values of `field` in mm.
fn level_values_mm(field: &GeodesicField) -> Vec<f64> {
    field.levels.iter().map(|t| t * field.stepover).collect()
}

/// True when the level set `T = level` passes the star-shape test of
/// `GeodesicField::star_levels` about `seed`.
fn level_is_star_shaped(field: &GeodesicField, seed: P2, r0: f64, level: usize) -> bool {
    let n = field.star_rays();
    (0..n).all(|i| {
        let ray = field.ray(seed, TAU * i as f64 / n as f64, r0);
        GeodesicField::ray_crosses_once(&ray, field.level_value(level))
    })
}

/// The law's next pass radius from `rho` (module doc, "The spacing law"):
/// `ρ' = -(R - cD) + √((R - cD)² + (ρ + R)² - R²)`, capped at `ρ + s`.
fn law_next_radius(tool_radius: f64, stepover: f64, ceiling_width: f64, rho: f64) -> f64 {
    let r = tool_radius;
    let b = r - ceiling_width;
    (-b + (b * b + (rho + r).powi(2) - r * r).sqrt()).min(rho + stepover)
}

/// The 2D Adaptive defaults (`AdaptiveConfig::default`), with the stepover
/// the fixture states (P10: `fixture_values`).
fn op_defaults() -> (f64, f64) {
    let cfg = crate::compute::operation_configs::AdaptiveConfig::default();
    let stepover = crate::compute::fixture_values::fixture_geometry(
        crate::compute::catalog::OperationType::Adaptive,
    )
    .iter()
    .find_map(|&(field, v)| (field == "stepover").then_some(v))
    .unwrap_or(f64::NAN);
    (stepover, cfg.tolerance)
}

/// A circle of radius `radius` about `c`, with enough sides that a chord
/// lies within `sag` of the arc: `n = ⌈π / acos(1 - sag / radius)⌉`.
fn circle(c: P2, radius: f64, sag: f64) -> Vec<P2> {
    let n = (PI / (1.0 - sag / radius).acos()).ceil() as usize;
    (0..n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            P2::new(c.x + radius * a.cos(), c.y + radius * a.sin())
        })
        .collect()
}

/// The lattice node `i` of `field`.
fn field_node(field: &GeodesicField, i: usize) -> P2 {
    let f = &field.field;
    P2::new(
        f.origin_x + (i % f.cols) as f64 * f.cell,
        f.origin_y + (i / f.cols) as f64 * f.cell,
    )
}

/// The 2D Adaptive params of the unit tests: the op defaults, the 6 mm
/// end mill, the default helix, the 2D pass-load rule.
fn test_params() -> AdaptiveParams {
    let (s, tol) = op_defaults();
    let helix = crate::compute::config::HELIX_RADIUS_OVER_D * 2.0 * R;
    AdaptiveParams {
        tool_radius: R,
        stepover: s,
        cut_depth: -1.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 5.0,
        tolerance: tol,
        slot_clearing: false,
        min_cutting_radius: 0.0,
        standing_material: None,
        cleanup_strategy: CleanupStrategy::ContourParallelHybrid,
        engagement_measure: super::super::EngagementMeasure::DiskArea,
        path_strategy: PathStrategy2d::Agent,
        trochoid_cap_mult: 1.2,
        keep_down_links: KeepDownLinks::WithinPassLoad,
        entry_helix_radius: helix,
        entry_helix_core_floor: 0.0,
    }
}

/// The outline of `fixtures/demo_star.svg` (the G1 star), as drawn: it is
/// mirror-symmetric about x = 50 and NOT a regular star (its tips lie
/// 45.5, 47.6, 49.8, 49.8 and 47.6 mm from (50, 50.5)).
fn demo_star() -> Polygon2 {
    let pts = [
        (50.0, 5.0),
        (61.0, 35.0),
        (95.0, 35.0),
        (68.0, 57.0),
        (79.0, 91.0),
        (50.0, 70.0),
        (21.0, 91.0),
        (32.0, 57.0),
        (5.0, 35.0),
        (39.0, 35.0),
    ];
    let mut ring: Vec<P2> = pts.iter().map(|&(x, y)| P2::new(x, y)).collect();
    // Counter-clockwise exterior, the polygon convention.
    let twice_area: f64 = ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum();
    if twice_area < 0.0 {
        ring.reverse();
    }
    Polygon2::new(ring)
}

/// On a disc the geodesic distance is the Euclidean distance from the
/// seed disc. Source tracking reads it exactly (every node sees the
/// seed centre): within rounding, and never an under-read.
#[test]
fn the_field_on_a_disc_is_the_euclidean_distance() {
    let (s, tol) = op_defaults();
    let cell = planner_cell_size(R, tol);
    let disc_radius = 20.0;
    // The part is the disc grown by R, so its tool-centre region is the
    // disc of `disc_radius`.
    let part = Polygon2::new(circle(P2::new(0.0, 0.0), disc_radius + R, cell));
    let region = ToolCentreRegion::new(&part, R);
    {
        for seed in [P2::new(0.0, 0.0), P2::new(7.3, -4.1)] {
            let field = over_part(&part, R, cell, s, seed, R)
                .unwrap_or_else(|| panic!("the seed {seed:?} lies in the disc"));
            assert_eq!(branch_count(&field), 1, "a disc has one front branch");
            let lattice = lattice_over(&part, cell);
            let mut read = 0usize;
            for i in 0..lattice.rows * lattice.cols {
                let p = lattice.node(i);
                if !region.contains(&p) {
                    continue;
                }
                let Some(t) = distance_mm(&field, p) else {
                    panic!("every node of the disc is reached: {p:?}");
                };
                let euclid = (p.x - seed.x).hypot(p.y - seed.y) - R;
                let over = 1e-9;
                assert!(
                    t >= euclid - 1e-9 && t - euclid <= over,
                    "T {t:.4} vs Euclidean {euclid:.4} at {p:?} (cell {cell})"
                );
                read += 1;
            }
            // Every node at least one cell inside the rim is read: the
            // lattice holds about one node per cell² of the disc shrunk
            // to the radius `disc_radius - cell`.
            let interior = std::f64::consts::PI * (disc_radius - cell).powi(2) / cell.powi(2);
            assert!(
                read as f64 >= interior,
                "the test read {read} nodes; it must read the disc (>= {interior:.0})"
            );
        }
    }
}

/// Behind an island the front goes round: T at a point behind the island
/// is more than its Euclidean distance by more than a cell, and not less
/// than the exact geodesic (two tangents and the arc between). Source
/// tracking reads it within one cell diagonal over the exact geodesic
/// (its bend points are lattice nodes; prototype: 0.55 cell at most over
/// the region, 0.29 at the test point) and splits the front into the two
/// sides of the island.
#[test]
fn the_field_goes_round_an_island() {
    let (s, tol) = op_defaults();
    let cell = planner_cell_size(R, tol);
    let island = 6.0;
    // The part grown by R round the region of a 60 x 40 rectangle with
    // a round island of radius `island`: its tool-centre region is that.
    let mut hole = circle(P2::new(0.0, 0.0), island - R, cell);
    hole.reverse();
    let part = Polygon2::with_holes(
        vec![
            P2::new(-30.0 - R, -20.0 - R),
            P2::new(30.0 + R, -20.0 - R),
            P2::new(30.0 + R, 20.0 + R),
            P2::new(-30.0 - R, 20.0 + R),
        ],
        vec![hole],
    );
    let seed = P2::new(-22.0, 0.0);
    let behind = P2::new(12.0, 0.0);
    // Exact geodesic round a circle of radius `island` between two
    // points on its axis at distances `a` and `b`: two tangents and the
    // arc between their touch points.
    let (a, b) = (seed.x.abs(), behind.x.abs());
    let tangents = (a * a - island * island).sqrt() + (b * b - island * island).sqrt();
    let arc = island * (PI - (island / a).acos() - (island / b).acos());
    let exact = tangents + arc - R;
    let euclid = (behind.x - seed.x).hypot(behind.y - seed.y) - R;
    {
        let field = over_part(&part, R, cell, s, seed, R).expect("seeded");
        let t = distance_mm(&field, behind).expect("the point behind is reached");
        assert!(
            t > euclid + cell,
            "T {t:.3} must exceed the Euclidean {euclid:.3} by more than a cell"
        );
        assert!(
            t >= exact - cell,
            "T {t:.3} must not under-read the exact geodesic {exact:.3}"
        );
        {
            assert!(
                t >= exact - 1e-9 && t <= exact + SQRT_2 * cell,
                "source tracking: T {t:.3} vs exact {exact:.3}, bound one cell diagonal"
            );
            assert!(
                branch_count(&field) >= 3,
                "the seed side and the two sides of the island: {}",
                branch_count(&field)
            );
            // The meeting line: behind the island, on its axis (the
            // seed is on the axis, so the two fronts meet there, at
            // y = 0, within one cell diagonal of it). The shadow lines
            // (where the paths start to bend, from the island's top and
            // bottom) are not meeting lines.
            let f = &field.field;
            let mut shock_cells = 0usize;
            for row in 0..f.rows - 1 {
                for col in 0..f.cols - 1 {
                    if !(field.cell_reachable(row, col) && field.cell_on_shock(row, col)) {
                        continue;
                    }
                    let x = f.origin_x + (col as f64 + 0.5) * f.cell;
                    let y = f.origin_y + (row as f64 + 0.5) * f.cell;
                    assert!(
                        x > island && y.abs() <= SQRT_2 * cell,
                        "a meeting-line cell at ({x:.2}, {y:.2}) is not behind the island on its axis"
                    );
                    shock_cells += 1;
                }
            }
            assert!(shock_cells > 0, "the two fronts meet behind the island");
        }
    }
}

/// G1-3: on the demo star (mirror-symmetric about x = 50) the field
/// along the five arm axes, at equal distance from the seed, equals the
/// Euclidean distance, so the five arms read alike. The star is
/// star-shaped about its centre, so source tracking is exact there
/// (bound: rounding, 1e-9 mm).
#[test]
fn the_five_arm_axes_of_the_star_read_alike() {
    let (s, tol) = op_defaults();
    let cell = planner_cell_size(R, tol);
    let part = demo_star();
    let seed = P2::new(50.0, 50.5);
    let tips = [
        (50.0, 5.0),
        (95.0, 35.0),
        (79.0, 91.0),
        (21.0, 91.0),
        (5.0, 35.0),
    ];
    {
        let field = over_part(&part, R, cell, s, seed, R).expect("seeded");
        let bound = 1e-9;
        // Equal distances out from the seed, one stepover apart, from the
        // seed disc to where an arm axis leaves the region (the shortest
        // arm's tool-centre tip is 45.5 - R / sin 20.1° = 36.8 mm out).
        // The prototype (2026-10-23) read every axis to 33 mm; nearer the
        // tips the axis nodes lie within a cell of a wall.
        let read_limit = 33.0;
        let mut rho = R + s;
        let mut read_to = 0.0_f64;
        while rho <= read_limit {
            let mut errors = Vec::new();
            for &(tx, ty) in &tips {
                let len = (tx - seed.x).hypot(ty - seed.y);
                let p = P2::new(
                    seed.x + rho * (tx - seed.x) / len,
                    seed.y + rho * (ty - seed.y) / len,
                );
                // The node the field holds nearest `p`, and its exact
                // Euclidean distance.
                let Some(node) = nearest_node(&field, p).map(|i| field_node(&field, i)) else {
                    continue;
                };
                let Some(t) = distance_mm(&field, node) else {
                    continue;
                };
                errors.push(t - ((node.x - seed.x).hypot(node.y - seed.y) - R));
            }
            if errors.len() < tips.len() {
                break;
            }
            let lo = errors.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = errors.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert!(
                hi - lo <= bound && lo >= -1e-9,
                "the arm axes spread {:.4} cell at {rho} mm (bound {:.4} cell)",
                (hi - lo) / cell,
                bound / cell
            );
            read_to = rho;
            rho += s;
        }
        assert!(
            read_to >= 25.0,
            "the five axes must be read out to 25 mm at least, read to {read_to}"
        );
    }
}

/// C16: the tail-only chaining of `chain_segments` drops the segment
/// left between two chains of an open arc; [`chain_whole`] keeps every
/// segment. The segments are listed in the order that loses one: the
/// middle chain is started first, then the end segment has no free tail.
/// A loop comes back closed, without its closing point.
#[test]
fn the_whole_chaining_keeps_the_segment_the_tail_chaining_drops() {
    let [q0, q1, q2, q3] = [
        P2::new(0.0, 0.0),
        P2::new(1.0, 0.0),
        P2::new(2.0, 0.5),
        P2::new(3.0, 1.5),
    ];
    let segments = [(q1, q2), (q2, q3), (q0, q1)];
    let tail = crate::geometry::marching_squares::chain_segments(&segments);
    let kept: usize = tail.iter().map(|c| c.len().saturating_sub(1)).sum();
    assert_eq!(kept, 2, "the tail chaining keeps 2 of 3 segments: {tail:?}");
    let whole = chain_whole(&segments);
    assert_eq!(whole.len(), 1, "one arc: {whole:?}");
    let arc = whole.first().expect("one arc");
    assert_eq!(arc.len(), 4, "every segment kept: {arc:?}");
    let ends = (arc.first().copied(), arc.last().copied());
    assert!(
        ends == (Some(q0), Some(q3)) || ends == (Some(q3), Some(q0)),
        "the arc runs end to end: {arc:?}"
    );
    let square = [
        P2::new(0.0, 0.0),
        P2::new(1.0, 0.0),
        P2::new(1.0, 1.0),
        P2::new(0.0, 1.0),
    ];
    let ring: Vec<(P2, P2)> = square
        .iter()
        .copied()
        .zip(square.iter().copied().cycle().skip(1))
        .collect();
    let loops = chain_whole(&ring);
    assert_eq!(loops.len(), 1, "one loop: {loops:?}");
    let looped = loops.first().expect("one loop");
    assert_eq!(looped.len(), 4, "a loop without its closing point");
    assert!(is_closed(looped, 1.0), "the loop reads as closed");
}

/// C16: on a disc about the seed every level set is a circle of radius
/// `ρ = R + T`, so [`GeodesicField::use_level_spacing`] gives the
/// curvature law's steps ([`law_next_radius`]): each step within the
/// op tolerance (the bisection bracket) plus one cell (the contour's
/// placement on the lattice, which the three-point radius reads) of the
/// law step from the same level, and never more than one stepover.
#[test]
fn the_local_level_spacing_follows_the_law_on_a_disc() {
    let (s, tol) = op_defaults();
    let cell = planner_cell_size(R, tol);
    let cd = pass_load_for(&test_params(), cell).ceiling * 2.0 * R;
    let disc_radius = 20.0;
    let part = Polygon2::new(circle(P2::new(0.0, 0.0), disc_radius + R, cell));
    let mut field =
        over_part(&part, R, cell, s, P2::new(0.0, 0.0), R).expect("the seed lies in the disc");
    let never: &dyn CancelCheck = &|| false;
    field
        .use_level_spacing(R, cd, tol, never)
        .expect("not cancelled");
    let mm = level_values_mm(&field);
    assert!(mm.len() > 5, "levels: {mm:?}");
    for pair in mm.windows(2) {
        let &[t0, t1] = pair else {
            continue;
        };
        let step = t1 - t0;
        let law = law_next_radius(R, s, cd, R + t0) - (R + t0);
        assert!(step <= s + 1e-9, "a step is at most s: {step} at {t0}");
        assert!(
            (step - law).abs() <= tol + cell,
            "the step {step:.4} from {t0:.3} mm vs the law {law:.4} (levels {mm:?})"
        );
    }
}

/// C4: the star-shape test. On a disc seeded at its centre with a round
/// island off the seed, the circles round the seed that do not reach
/// the island are star-shaped; a level set that reaches round the
/// island fails, since the rays toward the island leave the region
/// before they cross it. The same disc with no island passes those
/// levels (the control: the island is the cause).
#[test]
fn the_star_shape_test_rejects_a_level_set_behind_an_island() {
    let (s, tol) = op_defaults();
    let cell = planner_cell_size(R, tol);
    let disc_radius = 30.0;
    let outer = circle(P2::new(0.0, 0.0), disc_radius + R, cell);
    // The island of the part: radius 2 at (14, 0); its hole in the
    // tool-centre region has radius 2 + R = 5, so the front reaches it
    // 9 mm from the seed, at T = 9 - R = 6 mm, level 3.
    let mut hole = circle(P2::new(14.0, 0.0), 2.0, cell);
    hole.reverse();
    let seed = P2::new(0.0, 0.0);
    let with_island = Polygon2::with_holes(outer.clone(), vec![hole]);
    let without = Polygon2::new(outer);
    let solve =
        |part: &Polygon2| over_part(part, R, cell, s, seed, R).expect("the seed lies in the disc");
    let island = solve(&with_island);
    let open = solve(&without);
    for level in [1, 2] {
        assert!(
            level_is_star_shaped(&island, seed, R, level),
            "level {level} closes before the island"
        );
    }
    for level in [4, 7] {
        assert!(
            !level_is_star_shaped(&island, seed, R, level),
            "level {level} reaches round the island: not star-shaped"
        );
        assert!(
            level_is_star_shaped(&open, seed, R, level),
            "control: level {level} is a circle without the island"
        );
    }
    let k = island.star_levels(seed, R);
    assert!((2..=3).contains(&k), "core levels with the island: {k}");
}

/// §12.9: beside a core spiral, only the core's own loop (closed, of a
/// level up to K, round the seed) leaves the wraps. A pocket loop of a
/// core level that does not enclose the seed, an open arc of a core
/// level, and a loop of a level past K all stay, so they are planned.
#[test]
fn only_the_core_loop_leaves_the_wraps_beside_a_core_spiral() {
    let seed = P2::new(0.0, 0.0);
    let square = |c: P2, h: f64| {
        vec![
            P2::new(c.x - h, c.y - h),
            P2::new(c.x + h, c.y - h),
            P2::new(c.x + h, c.y + h),
            P2::new(c.x - h, c.y + h),
            P2::new(c.x - h, c.y - h),
        ]
    };
    let core = SpiralCore { seed, levels: 3 };
    let chains = vec![
        // The core loop of level 2: closed, round the seed.
        Chain::new(2, square(seed, 10.0), true),
        // A pocket loop of level 2 inside the core radius, cut off from
        // the seed: closed, but not round the seed.
        Chain::new(2, square(P2::new(6.0, 0.0), 1.0), true),
        // An open arc of level 3.
        Chain::new(3, vec![P2::new(20.0, 0.0), P2::new(20.0, 5.0)], false),
        // A loop of level 4, past K.
        Chain::new(4, square(seed, 30.0), true),
    ];
    let kept = beside_core(chains, core);
    let shape: Vec<(usize, bool)> = kept.iter().map(|c| (c.level, c.closed)).collect();
    assert_eq!(shape, vec![(2, true), (3, false), (4, true)]);
    assert!(
        kept.first()
            .is_some_and(|c| !Polygon2::new(c.pts.clone()).contains_point(&seed)),
        "the kept level-2 loop must be the pocket, not the core loop"
    );
}

/// Step 7.4a (lead ruling 2026-10-28): on a tight turn the area term of the
/// feed reading binds. The cutter has cut a trail along +x to the origin
/// beside material at `y ≥ R - s` (a side cut one stepover `s` wide), then turns
/// 30° into the material in one pass step (a turn radius of
/// `step / (2 sin 15°)`, under `R`). The removal per mm reads more than the
/// width across the step, so the factor is `c / (area_per_mm / D)`, lower
/// than the width alone gives.
#[test]
fn the_area_term_binds_on_a_tight_turn() {
    use crate::adaptive::MaterialGrid;
    use crate::adaptive::path::pass_step_len;
    use crate::adaptive::search::{PassLoad, measure_step};

    use super::slice::{feed_factor, step_reading};

    const STEPOVER: f64 = 2.0;
    let cell = planner_cell_size(R, 0.1);
    let swept = PassLoad::swept_width(STEPOVER, R, cell);
    let area = PassLoad::area_per_mm(STEPOVER, R, cell);
    let mut grid =
        MaterialGrid::from_polygon(&Polygon2::rectangle(-30.0, R - STEPOVER, 30.0, 30.0), cell);
    let mut x = -20.0;
    while x <= 0.0 {
        grid.clear_circle(x, 0.0, R);
        x += cell;
    }
    let from = P2::new(0.0, 0.0);
    let heading = 30.0_f64.to_radians();
    let step = pass_step_len(cell);
    let to = P2::new(step * heading.cos(), step * heading.sin());

    let width = measure_step(&grid, from, to.x, to.y, R, heading, &swept);
    let removal = measure_step(&grid, from, to.x, to.y, R, heading, &area);
    assert!(
        removal > width && width > swept.ceiling,
        "the fixture must read the area term over the width, both over the \
         ceiling {}: width {width}, area {removal}",
        swept.ceiling
    );
    let reading = step_reading(&grid, from, to, R, &swept, &area);
    assert_eq!(reading.to_bits(), removal.to_bits());
    let factor = feed_factor(swept.ceiling, reading);
    assert_eq!(factor.to_bits(), (swept.ceiling / removal).to_bits());
    assert!(factor < feed_factor(swept.ceiling, width));
}

/// D65: `feed_caps` caps a step the replay admitted over the ceiling under
/// the capped rule, at `c / L` below 1, and does not cap a step that reads
/// at or under the ceiling. A straight cut through standing stock from an
/// entry reads over the ceiling; a cut back half a radius beside it, inside
/// the cleared slot, reads a quarter of the diameter, under the ceiling.
/// (`feed_caps` matches a step to its run by position, so the walk back is
/// beside the first cut, not on it.)
#[test]
fn feed_caps_caps_a_step_over_the_ceiling_d65() {
    use crate::adaptive::AdaptiveSegment;

    let params = test_params();
    let square = Polygon2::rectangle(-30.0, -30.0, 30.0, 30.0);
    let (a, b) = (P2::new(-15.0, 0.0), P2::new(15.0, 0.0));
    let (b2, a2) = (P2::new(15.0, R / 2.0), P2::new(-15.0, R / 2.0));
    let segments = vec![
        AdaptiveSegment::Rapid(a),
        AdaptiveSegment::Cut(vec![a, b]),
        AdaptiveSegment::Link(b2),
        AdaptiveSegment::Cut(vec![b2, a2]),
    ];
    let (out, caps) = super::slice::feed_caps(&square, &params, segments, &[]);
    let cut_caps: Vec<Option<f64>> = out
        .iter()
        .zip(&caps)
        .filter(|(s, _)| matches!(s, AdaptiveSegment::Cut(_)))
        .map(|(_, c)| c.map(|(factor, _)| factor))
        .collect();
    let [first, second] = cut_caps.as_slice() else {
        panic!("two cuts: {cut_caps:?}");
    };
    let first = first.expect("the cut through stock is capped");
    assert!(first > 0.0 && first < 1.0, "{first}");
    assert_eq!(*second, None, "the cut beside it reads under the ceiling");
}
