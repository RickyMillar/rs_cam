//! May the tool stay down here? The 3D keep-down reads of every adaptive3d
//! strategy (INTEGRATION_PLAN D1, integration step 7.2).
//!
//! One read underlies them: the conservative stock top over the tool's disc,
//! sampled every half cell along the move ([`disc_tops`]). Two forms read it:
//! - [`corridor_ceiling_z`]: the highest top on the corridor (`plan_entry`'s
//!   stay-down proof);
//! - [`keep_down_excess`]: where, and by how much, the corridor stands above
//!   a ceiling (the proof of a held link).
//!
//! [`keep_down_route`] is the routed keep-down travel at an entry (research
//! §16 Part B, `planning/adaptive_research_2026-10-23/P3_C22_3d_turns.md`):
//! an A* path over the cells whose disc is free. **The rule (§16.7): a
//! keep-down travel replaces the travel, never the entry's cut.** An entry
//! into stock cuts its column; a horizontal approach into that column is a
//! slot. So a caller routes only to an entry whose own disc is free, and
//! ends the route ON the entry.
//!
//! - A grid cell is FREE when the conservative stock top over the tool's
//!   disc there is at most `ceiling_z`.
//! - The search is A* over the free cells, 8 neighbours, from the free cells
//!   within two cells of the tool to the first free cell whose centre lies
//!   within `side_max` of the entry and from which the side leg to the entry
//!   passes `side_ok` (the caller's pass rule).
//! - Its length is held to `max_len` (the caller's stay-down distance), so
//!   the search window and the work per entry are bounded.
//!
//! No new constant: the ceiling, the side-leg rule and the length bound are
//! the caller's; two cells is the grid's own diagonal reach of a point.
//!
//! The level side link of ContourParallel (`clearing.rs`,
//! `link_footprint_ok`) does not read this file yet: INTEGRATION_PLAN step 2
//! moves it here, with its own re-bless.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::dexel_stock::TriDexelStock;
use crate::geo::{P2, P3};

/// The conservative stock top over the disc of `radius` at each sample of
/// the move from `from` to `to` (XY), both ends included, with the sample's
/// XY. Samples every half cell, so no column the disc sweeps is skipped. A
/// sample off the grid gives nothing.
fn disc_tops(
    stock: &TriDexelStock,
    from: P3,
    to: P3,
    radius: f64,
) -> impl Iterator<Item = (P2, f64)> + '_ {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    let step = (stock.z_grid.cell_size * 0.5).max(1e-3);
    let n = (len / step).ceil().max(1.0) as usize;
    (0..=n).filter_map(move |k| {
        let t = k as f64 / n as f64;
        let p = P2::new(from.x + dx * t, from.y + dy * t);
        stock
            .max_conservative_top_z_in_disc(p.x, p.y, radius)
            .map(|top| (p, top))
    })
}

/// The highest conservative stock top on the swept disc of `radius` from
/// `from` to `to` (XY), both ends included ([`disc_tops`]). `NEG_INFINITY`
/// when the whole corridor is off the grid.
pub(super) fn corridor_ceiling_z(stock: &TriDexelStock, from: P3, to: P3, radius: f64) -> f64 {
    disc_tops(stock, from, to, radius)
        .map(|(_, top)| top)
        .fold(f64::NEG_INFINITY, f64::max)
}

/// The keep-down proof of a link that feeds at depth from `from` to `to`
/// (any strategy's held link): the point of the swept disc of `radius`
/// ([`disc_tops`]) where the conservative stock top stands highest above
/// `ceiling_z`, and that height; `None` when no column of the disc stands
/// above `ceiling_z`, so the link may stay down.
///
/// The ceiling a caller passes is the top of the slab the link's level cuts
/// plus the stay-down clearance (the dexel resolution, `plan_entry`'s
/// allowance). Inside the slab a link held to the 2D pass load may cut a
/// side band up to `c·D` wide (G-ADAPTLINKLOAD): that is planned. A column
/// above the slab is stock the level above did not cut, which no planned
/// move of this level takes (C38 §14: the fed-link bars).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "integration step 7.2: the geodesic arm's held links call it in step 7.4"
    )
)]
pub(super) fn keep_down_excess(
    stock: &TriDexelStock,
    from: P3,
    to: P3,
    radius: f64,
    ceiling_z: f64,
) -> Option<(P2, f64)> {
    disc_tops(stock, from, to, radius)
        .filter(|&(_, top)| top > ceiling_z)
        .map(|(p, top)| (p, top - ceiling_z))
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// What a route must hold.
pub(super) struct RouteRequest<'a> {
    pub(super) from: P2,
    pub(super) to: P2,
    pub(super) radius: f64,
    pub(super) ceiling_z: f64,
    pub(super) max_len: f64,
    pub(super) side_max: f64,
    /// The side leg from a free point to the entry: true when it may be cut.
    pub(super) side_ok: &'a dyn Fn(P2) -> bool,
    /// Where a route may run at all (the session's boundary clip): a cell
    /// outside it is never free, so a route needs no clip.
    pub(super) allowed: &'a dyn Fn(P2) -> bool,
}

#[derive(Clone, Copy, PartialEq)]
struct Open {
    f: f64,
    g: f64,
    cell: (isize, isize),
}

impl Eq for Open {}

impl Ord for Open {
    fn cmp(&self, other: &Self) -> Ordering {
        // A min-heap on f, then on g.
        other
            .f
            .total_cmp(&self.f)
            .then_with(|| other.g.total_cmp(&self.g))
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The route from `req.from` to the free point the side leg leaves from:
/// the free cell centres in order, the first within two cells of
/// `req.from`, the last within `req.side_max` of `req.to`. `None` when no
/// route holds within `req.max_len`.
pub(super) fn keep_down_route(stock: &TriDexelStock, req: &RouteRequest<'_>) -> Option<Vec<P2>> {
    let grid = &stock.z_grid;
    let cs = grid.cell_size;
    if cs.is_nan() || cs <= 0.0 || req.max_len <= 0.0 {
        return None;
    }
    let to_world = |c: (isize, isize)| {
        P2::new(
            grid.origin_u + c.0 as f64 * cs,
            grid.origin_v + c.1 as f64 * cs,
        )
    };
    let on_grid = |c: (isize, isize)| {
        c.0 >= 0 && c.1 >= 0 && (c.0 as usize) < grid.cols && (c.1 as usize) < grid.rows
    };
    let mut free_memo: HashMap<(isize, isize), bool> = HashMap::new();
    let mut free = |c: (isize, isize)| -> bool {
        *free_memo.entry(c).or_insert_with(|| {
            on_grid(c) && (req.allowed)(to_world(c)) && {
                let p = to_world(c);
                stock
                    .max_conservative_top_z_in_disc(p.x, p.y, req.radius)
                    .is_none_or(|top| top <= req.ceiling_z)
            }
        })
    };
    let dist = |a: P2, b: P2| (b.x - a.x).hypot(b.y - a.y);
    let cell_of = |p: P2| {
        (
            ((p.x - grid.origin_u) / cs).round() as isize,
            ((p.y - grid.origin_v) / cs).round() as isize,
        )
    };
    let h = |c: (isize, isize)| dist(to_world(c), req.to);

    let mut open: BinaryHeap<Open> = BinaryHeap::new();
    let mut best_g: HashMap<(isize, isize), f64> = HashMap::new();
    let mut came: HashMap<(isize, isize), (isize, isize)> = HashMap::new();
    // Sources: the free cells within two cells of the tool, at their true
    // distance from it.
    let start = cell_of(req.from);
    for dx in -2..=2 {
        for dy in -2..=2 {
            let c = (start.0 + dx, start.1 + dy);
            let g = dist(req.from, to_world(c));
            if g <= 2.0 * cs * std::f64::consts::SQRT_2 + 1e-9 && free(c) {
                best_g.insert(c, g);
                open.push(Open {
                    f: g + h(c),
                    g,
                    cell: c,
                });
            }
        }
    }
    while let Some(Open { g, cell, .. }) = open.pop() {
        if best_g.get(&cell).is_some_and(|&b| g > b + 1e-12) {
            continue;
        }
        let here = to_world(cell);
        let side = dist(here, req.to);
        if side <= req.side_max && g + side <= req.max_len && (req.side_ok)(here) {
            let mut path = vec![here];
            let mut c = cell;
            while let Some(&prev) = came.get(&c) {
                path.push(to_world(prev));
                c = prev;
            }
            path.reverse();
            return Some(path);
        }
        for (dx, dy) in [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ] {
            let next = (cell.0 + dx, cell.1 + dy);
            let step = if dx != 0 && dy != 0 {
                cs * std::f64::consts::SQRT_2
            } else {
                cs
            };
            let ng = g + step;
            // A route longer than the bound, even straight on from here,
            // cannot hold.
            if ng + h(next) > req.max_len + req.side_max {
                continue;
            }
            if best_g.get(&next).is_some_and(|&b| ng >= b) || !free(next) {
                continue;
            }
            best_g.insert(next, ng);
            came.insert(next, cell);
            open.push(Open {
                f: ng + h(next),
                g: ng,
                cell: next,
            });
        }
    }
    None
}

/// `path` less its interior points that lie on the line of their
/// neighbours (the grid's straight runs), so a route is one move per turn.
pub(super) fn straight_runs(path: &[P2]) -> Vec<P2> {
    let mut out: Vec<P2> = Vec::with_capacity(path.len());
    for &p in path {
        while out.len() >= 2 {
            let (Some(&a), Some(&b)) = (out.get(out.len() - 2), out.last()) else {
                break;
            };
            let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            if cross.abs() <= 1e-9 {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    out
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    const CELL: f64 = 0.5;
    const TOP: f64 = 10.0;
    const FLOOR: f64 = 5.0;
    const R: f64 = 3.0;

    /// A 60 × 60 mm block, `TOP` high, on a `CELL` lattice.
    fn block() -> TriDexelStock {
        TriDexelStock::from_stock(0.0, 0.0, 60.0, 60.0, 0.0, TOP, CELL)
    }

    /// Clear every cell whose centre lies in the box down to `z`.
    fn clear_box(stock: &mut TriDexelStock, x: (f64, f64), y: (f64, f64), z: f64) {
        let (rows, cols) = (stock.z_grid.rows, stock.z_grid.cols);
        let (ou, ov, cs) = (
            stock.z_grid.origin_u,
            stock.z_grid.origin_v,
            stock.z_grid.cell_size,
        );
        for row in 0..rows {
            for col in 0..cols {
                let (cx, cy) = (ou + col as f64 * cs, ov + row as f64 * cs);
                if (x.0..=x.1).contains(&cx) && (y.0..=y.1).contains(&cy) {
                    stock.clear_above_at(row, col, z as f32);
                }
            }
        }
    }

    /// A planted link through stock, and the same link over cut ground: the
    /// proof names the excess of the first, and passes the second. The
    /// corridor ceiling, the max form of the same read, agrees.
    #[test]
    fn a_link_through_stock_is_refused_and_over_cut_ground_is_kept() {
        let ceiling = FLOOR + CELL;
        let (from, to) = (P3::new(10.0, 30.0, FLOOR), P3::new(50.0, 30.0, FLOOR));

        let fresh = block();
        let (_, excess) = keep_down_excess(&fresh, from, to, R, ceiling).expect("stock above");
        assert!((excess - (TOP - ceiling)).abs() < 1e-6, "excess {excess}");
        assert!((corridor_ceiling_z(&fresh, from, to, R) - TOP).abs() < 1e-6);

        let mut cut = block();
        clear_box(&mut cut, (0.0, 60.0), (20.0, 40.0), FLOOR);
        assert_eq!(keep_down_excess(&cut, from, to, R, ceiling), None);
        assert!((corridor_ceiling_z(&cut, from, to, R) - FLOOR).abs() < 1e-6);

        // One column left standing at x = 30: the proof names a point whose
        // disc holds it, and the full excess.
        let mut column = block();
        clear_box(&mut column, (0.0, 28.0), (20.0, 40.0), FLOOR);
        clear_box(&mut column, (32.0, 60.0), (20.0, 40.0), FLOOR);
        clear_box(&mut column, (28.0, 32.0), (20.0, 26.0), FLOOR);
        clear_box(&mut column, (28.0, 32.0), (34.0, 40.0), FLOOR);
        let (at, excess) = keep_down_excess(&column, from, to, R, ceiling).expect("column");
        assert!((at.x - 30.0).abs() <= 2.0 + R, "point {at:?}");
        assert!((excess - (TOP - ceiling)).abs() < 1e-6, "excess {excess}");
    }

    fn route(stock: &TriDexelStock, max_len: f64) -> Option<Vec<P2>> {
        let side_ok = |_: P2| true;
        let allowed = |_: P2| true;
        keep_down_route(
            stock,
            &RouteRequest {
                from: P2::new(10.0, 20.0),
                to: P2::new(50.0, 20.0),
                radius: R,
                ceiling_z: FLOOR + CELL,
                max_len,
                side_max: CELL,
                side_ok: &side_ok,
                allowed: &allowed,
            },
        )
    }

    /// Cut ground with a standing wall across the straight line and a gap at
    /// one end: the route goes round through the gap on free cells only,
    /// and ends within one cell of the entry. A length bound under the
    /// detour, or a wall with no gap, gives no route.
    #[test]
    fn the_route_goes_round_a_wall_on_free_cells_only() {
        // The wall: x 28..32 stands to the top for y 0..44; the gap is y 44..60.
        let mut gap = block();
        clear_box(&mut gap, (0.0, 27.5), (0.0, 60.0), FLOOR);
        clear_box(&mut gap, (32.5, 60.0), (0.0, 60.0), FLOOR);
        clear_box(&mut gap, (28.0, 32.0), (44.0, 60.0), FLOOR);
        let path = route(&gap, 200.0).expect("a route round the wall");
        let last = *path.last().unwrap();
        assert!((last.x - 50.0).hypot(last.y - 20.0) <= CELL + 1e-9);
        for p in &path {
            let top = gap.max_conservative_top_z_in_disc(p.x, p.y, R);
            assert!(
                top.is_none_or(|t| t <= FLOOR + CELL),
                "route point {p:?} has stock at {top:?}"
            );
        }
        assert!(
            path.iter().any(|p| p.y > 44.0),
            "the route did not use the gap"
        );
        // The straight line is 40 mm; the detour is longer.
        assert!(route(&gap, 45.0).is_none());

        let mut closed = block();
        clear_box(&mut closed, (0.0, 27.5), (0.0, 60.0), FLOOR);
        clear_box(&mut closed, (32.5, 60.0), (0.0, 60.0), FLOOR);
        assert!(route(&closed, 200.0).is_none());
    }

    /// The straight runs of a route keep only its turns.
    #[test]
    fn straight_runs_keep_only_the_turns() {
        let path: Vec<P2> = (0..5)
            .map(|i| P2::new(f64::from(i), 0.0))
            .chain((1..4).map(|j| P2::new(4.0, f64::from(j))))
            .collect();
        let runs = straight_runs(&path);
        assert_eq!(
            runs,
            vec![P2::new(0.0, 0.0), P2::new(4.0, 0.0), P2::new(4.0, 3.0)]
        );
    }
}
