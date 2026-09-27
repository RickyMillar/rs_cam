//! G-ADAPTRAPIDLIFT — a 2D Adaptive rapid travels in XY only at the rapid
//! plane; it leaves cut depth straight up, and no rapid enters standing
//! material.
//!
//! `adaptive/path.rs` `segments_to_toolpath` emitted a `Rapid` segment as one
//! G0 from the cutter's position at cut depth straight to the next entry at
//! safe Z, so the cutter climbed on a diagonal through whatever stood beside
//! it: the pocket wall, an island, residue. It now retracts Z-only first with
//! the shared `Toolpath::final_retract`, the shape the rest of the codebase
//! emits (retract, XY rapid at the retract height, descend).
//!
//! Before, on the six-island fixture (`common::adaptive_islands`, 91bb006a):
//! 38 diagonal rapids leave cut depth, and the live rapid check of the
//! session simulation (`RapidClearanceCheck`, the stock as it stands at that
//! point of playback, with the cutter's profile) reports 34 strikes, every
//! one on a diagonal rapid (the first: from (57, -37, -3) past the pocket
//! wall toward (-41.4, -10.3, 10)).
//!
//! The oracle is exact geometry, not the simulation (lead decision
//! 2026-09-27, G-ADAPTPASSLOAD round 4). The fixture's standing material is
//! stated exactly: the part's walls (the pocket exterior and the six island
//! polygons, `common::adaptive_islands::islands_pocket`) stand to the stock
//! top, and the pocket stands at the floor of the last depth level the
//! emitted path has finished cutting (the stock top before the first). A
//! rapid collides when the cutter's profile at its lowest tip Z meets
//! either, judged at the exact distance from its XY path to the walls. The
//! only slack is the flattening of the tool-centre region the planner
//! reads, `FlattenPolicy::UNTOLERANCED_MM` (10 um).
//!
//! The simulation's live check is kept as a second reader: every rapid it
//! flags must be one the oracle clears, printed with its exact clearance,
//! and the count of such sim-only flags is pinned ([`SIM_ONLY_FLAGS`]) so a
//! new flag of either kind shows. Why it can flag a clear rapid: its high
//! channel reads each cell's `conservative_top` at the cell's near point,
//! so a 0.5 mm cell holding a sliver of a real wall counts that wall about
//! half a cell diagonal plus the half-cell dilation (0.35 + 0.25 mm) closer
//! than it stands. Move 4514 (a descent beside island (0, 16)) clears the
//! island by 0.317 mm and is flagged (COVERAGE_UNION_PLAN Step 0, round 4:
//! not union residue). The fix is the sub-cell-exact clearance query
//! (`planning/rapid_safety_2026-08-28/RAPIDPLUNGETOL_PLAN.md` Option C).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use common::adaptive_islands::{adaptive_session, stock, toolpath_of, wall_distance_of_segment};
use rs_cam_core::geo::P2;
use rs_cam_core::toolpath::MoveType;

/// Rapids the simulation's live check flags although the exact oracle
/// clears them: move 4514, 0.317 mm clear of island (0, 16), measured
/// 2026-09-27 (see the header).
const SIM_ONLY_FLAGS: usize = 1;

/// The slack of the exact oracle, mm: the flattening of the tool-centre
/// region (see the header).
const ORACLE_EPS_MM: f64 = rs_cam_core::polygon::FlattenPolicy::UNTOLERANCED_MM;

/// Every XY rapid runs at the rapid plane; no rapid enters standing
/// material by the exact oracle; every rapid the simulation flags is one
/// the oracle clears, and their count is pinned.
#[test]
fn a_rapid_leaves_cut_depth_straight_up() {
    use rs_cam_core::compute::cutter::build_cutter;
    use rs_cam_core::tool::MillingCutter;

    let session = adaptive_session(false);
    let tp = toolpath_of(&session);
    let stock = stock();
    let stock_top = stock.origin_z + stock.z;
    // The rapid plane: the retract height the path climbs to. It must
    // clear the stock top, or the check below is vacuous.
    let plane = tp
        .moves
        .iter()
        .map(|m| m.target.z)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        plane > stock_top,
        "rapid plane {plane} is not above the stock top {stock_top}"
    );
    let mut xy_rapids = 0;
    let mut low = Vec::new();
    for (i, w) in tp.moves.windows(2).enumerate() {
        let (a, b) = (w[0].target, w[1].target);
        if !matches!(w[1].move_type, MoveType::Rapid) || (b.x - a.x).hypot(b.y - a.y) < 1e-9 {
            continue;
        }
        xy_rapids += 1;
        if a.z.min(b.z) < plane - 1e-6 {
            low.push((i + 1, a.z, b.z));
        }
    }
    assert!(xy_rapids >= 1, "no XY rapid: the fixture tests nothing");
    assert!(
        low.is_empty(),
        "{} rapids travel in XY below the rapid plane {plane} (move, from z, to z): {:?}",
        low.len(),
        &low[..low.len().min(5)]
    );

    let tool = build_cutter(&common::make_endmill_6mm());
    let r = tool.radius();
    let cutting = cut_pieces(&tp);
    // The exact clearance of rapid `i` (from move i - 1 to move i), mm:
    // negative when it enters standing material.
    let clearance = |i: usize| -> f64 {
        let (a, b) = (tp.moves[i - 1].target, tp.moves[i].target);
        let tip = a.z.min(b.z);
        if tip >= stock_top {
            return tip - stock_top;
        }
        // The walls stand to the stock top. The profile is non-decreasing
        // in r, so the nearest wall point is the one the cutter meets
        // first: it clears by its lift over that point or by its distance
        // past the envelope.
        let d = wall_distance_of_segment(P2::new(a.x, a.y), P2::new(b.x, b.y));
        let wall = match tool.height_at_radius(d) {
            Some(h) => (tip + h - stock_top).max(d - r),
            None => d - r,
        };
        // The pocket: stock stands above the tip wherever no earlier move
        // cut down to it. The rapids that reach below the stock top are
        // vertical (asserted above), so the footprint is one disc.
        assert!(
            (b.x - a.x).hypot(b.y - a.y) < 1e-9,
            "rapid {i} moves in XY below the stock top"
        );
        let pocket = match uncut_point_in_disc(P2::new(a.x, a.y), r, tip, &cutting[..i]) {
            Some(q) => tip - top_at(q, &cutting[..i], stock_top),
            None => f64::INFINITY,
        };
        wall.min(pocket)
    };
    let mut entering = Vec::new();
    let mut rapids = 0usize;
    for (i, m) in tp.moves.iter().enumerate().skip(1) {
        if !matches!(m.move_type, MoveType::Rapid) {
            continue;
        }
        rapids += 1;
        let c = clearance(i);
        if c < -ORACLE_EPS_MM {
            entering.push((i, c));
        }
    }
    let flagged: Vec<usize> = session
        .simulation_result()
        .expect("simulated")
        .rapid_collisions
        .iter()
        .map(|c| c.move_index)
        .collect();
    let sim_only: Vec<(usize, f64)> = flagged
        .iter()
        .map(|&i| (i, clearance(i)))
        .filter(|(_, c)| *c >= -ORACLE_EPS_MM)
        .collect();
    eprintln!(
        "G-ADAPTRAPIDLIFT: {xy_rapids} XY rapids, {} below the plane {plane}; {rapids} rapids, \
         {} enter material (exact); the simulation flags {}, clear by the oracle (move, \
         exact clearance mm): {sim_only:?}",
        low.len(),
        entering.len(),
        flagged.len()
    );
    assert!(
        entering.is_empty(),
        "{} rapids enter standing material (move, clearance mm): {:?}",
        entering.len(),
        &entering[..entering.len().min(5)]
    );
    assert_eq!(
        sim_only.len(),
        flagged.len(),
        "the simulation flags a rapid the oracle also finds in material"
    );
    assert_eq!(
        sim_only.len(),
        SIM_ONLY_FLAGS,
        "sim-only rapid flags moved (move, exact clearance mm): {sim_only:?}"
    );
}

/// One straight piece of a cutting move (an arc is split into chords within
/// a tenth of [`ORACLE_EPS_MM`] of it): its XY ends and Z ends.
struct CutPiece {
    a: P2,
    b: P2,
    za: f64,
    zb: f64,
}

/// Every fed move of `tp` as straight pieces, with Z linear along each.
fn cut_pieces(tp: &rs_cam_core::toolpath::Toolpath) -> Vec<Vec<CutPiece>> {
    let mut out: Vec<Vec<CutPiece>> = vec![Vec::new()];
    for w in tp.moves.windows(2) {
        let (from, m) = (w[0].target, &w[1]);
        let mut pieces = Vec::new();
        let arc = match m.move_type {
            MoveType::Rapid => None,
            MoveType::ArcCW { i, j, .. } => Some((i, j, true)),
            MoveType::ArcCCW { i, j, .. } => Some((i, j, false)),
            _ => {
                pieces.push(CutPiece {
                    a: P2::new(from.x, from.y),
                    b: P2::new(m.target.x, m.target.y),
                    za: from.z,
                    zb: m.target.z,
                });
                None
            }
        };
        if let Some((i, j, cw)) = arc {
            let (cx, cy) = (from.x + i, from.y + j);
            let rad = i.hypot(j);
            let a0 = (from.y - cy).atan2(from.x - cx);
            let a1 = (m.target.y - cy).atan2(m.target.x - cx);
            let tau = std::f64::consts::TAU;
            let sweep = if cw {
                -((a0 - a1).rem_euclid(tau))
            } else {
                (a1 - a0).rem_euclid(tau)
            };
            // Chord sagitta rad (1 - cos(step / 2)) within eps / 10.
            let sag = ORACLE_EPS_MM / 10.0;
            let step = if rad > sag {
                2.0 * (1.0 - sag / rad).acos()
            } else {
                tau
            };
            let n = ((sweep.abs() / step).ceil() as usize).max(1);
            let at = |k: usize| {
                let t = k as f64 / n as f64;
                let ang = a0 + t * sweep;
                (
                    P2::new(cx + rad * ang.cos(), cy + rad * ang.sin()),
                    from.z + t * (m.target.z - from.z),
                )
            };
            for k in 0..n {
                let ((pa, za), (pb, zb)) = (at(k), at(k + 1));
                pieces.push(CutPiece {
                    a: pa,
                    b: pb,
                    za,
                    zb,
                });
            }
        }
        out.push(pieces);
    }
    out
}

fn point_segment_distance(p: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

/// The part of `piece` at or below `z` (Z is linear along it), if any.
fn below(piece: &CutPiece, z: f64) -> Option<(P2, P2)> {
    let (lo, hi) = (piece.za.min(piece.zb), piece.za.max(piece.zb));
    if lo > z {
        return None;
    }
    if hi <= z {
        return Some((piece.a, piece.b));
    }
    let t = (z - piece.za) / (piece.zb - piece.za);
    let at = P2::new(
        piece.a.x + t * (piece.b.x - piece.a.x),
        piece.a.y + t * (piece.b.y - piece.a.y),
    );
    Some(if piece.za <= z {
        (piece.a, at)
    } else {
        (at, piece.b)
    })
}

/// A point of the pocket within `r` of `c` that no earlier move cut down to
/// `tip`, if one exists. Exact up to [`ORACLE_EPS_MM`]: the swept discs are
/// read at `R + eps` and the probe at `r - eps`, and a square is split until
/// one swept capsule (convex) holds its four corners, it leaves the probe
/// disc or the pocket, or its side is under eps (its centre then decides).
fn uncut_point_in_disc(c: P2, r: f64, tip: f64, moves: &[Vec<CutPiece>]) -> Option<P2> {
    let reach = common::adaptive_islands::TOOL_RADIUS_MM + ORACLE_EPS_MM;
    let probe = r - ORACLE_EPS_MM;
    let caps: Vec<(P2, P2)> = moves
        .iter()
        .flatten()
        .filter_map(|p| below(p, tip + ORACLE_EPS_MM))
        .filter(|(a, b)| point_segment_distance(c, *a, *b) <= probe + reach)
        .collect();
    let part = common::adaptive_islands::islands_pocket();
    let covered = |p: P2| {
        caps.iter()
            .any(|(a, b)| point_segment_distance(p, *a, *b) <= reach)
    };
    let mut stack = vec![(c, probe)];
    while let Some((q, h)) = stack.pop() {
        // Square centred on q, half side h.
        let near = P2::new(c.x.clamp(q.x - h, q.x + h), c.y.clamp(q.y - h, q.y + h));
        if (near.x - c.x).hypot(near.y - c.y) > probe {
            continue;
        }
        let half_diag = h * std::f64::consts::SQRT_2;
        let in_pocket = part.contains_point(&q);
        let wall = common::adaptive_islands::wall_distance(q.x, q.y);
        if !in_pocket && wall >= half_diag {
            continue;
        }
        let corners = [
            P2::new(q.x - h, q.y - h),
            P2::new(q.x + h, q.y - h),
            P2::new(q.x - h, q.y + h),
            P2::new(q.x + h, q.y + h),
        ];
        if caps.iter().any(|(a, b)| {
            corners
                .iter()
                .all(|p| point_segment_distance(*p, *a, *b) <= reach)
        }) {
            continue;
        }
        if h < ORACLE_EPS_MM / 2.0 {
            if in_pocket && (q.x - c.x).hypot(q.y - c.y) <= probe && !covered(q) {
                return Some(q);
            }
            continue;
        }
        let k = h / 2.0;
        for (dx, dy) in [(-k, -k), (k, -k), (-k, k), (k, k)] {
            stack.push((P2::new(q.x + dx, q.y + dy), k));
        }
    }
    None
}

/// The stock top at `q` after `moves`: the lowest Z any of them cut there,
/// or the stock top.
fn top_at(q: P2, moves: &[Vec<CutPiece>], stock_top: f64) -> f64 {
    let reach = common::adaptive_islands::TOOL_RADIUS_MM;
    moves
        .iter()
        .flatten()
        .filter(|p| point_segment_distance(q, p.a, p.b) <= reach)
        .map(|p| p.za.min(p.zb))
        .fold(stock_top, f64::min)
}

/// Instrument (COVERAGE_UNION_PLAN Step 0 on this fixture): replay the
/// toolpath up to each live rapid strike, print the two channels at the
/// first striking sample, the cell(s) that set them, and the per-move
/// coverage history of those cells.
#[test]
#[ignore = "instrument: prints the strike cell's channels and stamp history"]
fn print_rapid_strike_cells() {
    use rs_cam_core::compute::cutter::build_cutter;
    use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
    use rs_cam_core::stock::collision::RapidClearanceCheck;
    use rs_cam_core::stock::dexel::ray_top;
    use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
    use rs_cam_core::tool::MillingCutter;

    let session = adaptive_session(false);
    let tp = toolpath_of(&session);
    let hits: Vec<usize> = session
        .simulation_result()
        .expect("simulated")
        .rapid_collisions
        .iter()
        .map(|c| c.move_index)
        .collect();
    eprintln!("session strikes at moves {hits:?}");
    let tool = build_cutter(&common::make_endmill_6mm());
    let radius = tool.radius();
    let lut = RadialProfileLUT::from_cutter(&tool, LUT_SAMPLES);
    let s = stock();
    let fresh = || {
        TriDexelStock::from_stock(
            s.origin_x,
            s.origin_y,
            s.origin_x + s.x,
            s.origin_y + s.y,
            s.origin_z,
            s.origin_z + s.z,
            common::adaptive_islands::SIM_RESOLUTION_MM,
        )
    };
    let check = RapidClearanceCheck::new(&tool);
    for &mi in &hits {
        let mut st = fresh();
        st.simulate_toolpath_range(&tp, &tool, StockCutDirection::FromTop, 1, mi);
        let (a, b) = (tp.moves[mi - 1].target, tp.moves[mi].target);
        eprintln!(
            "move {mi} {:?}: ({:.3},{:.3},{:.3}) -> ({:.3},{:.3},{:.3}); replay strikes: {}",
            tp.moves[mi].move_type,
            a.x,
            a.y,
            a.z,
            b.x,
            b.y,
            b.z,
            check.strikes(&st, a, b, radius)
        );
        let g = &st.z_grid;
        let cs = g.cell_size;
        let tau = cs * (std::f64::consts::FRAC_1_SQRT_2 + 1.5);
        let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
        let n = ((dx * dx + dy * dy + dz * dz).sqrt()).ceil().max(1.0) as usize;
        let mut watch = Vec::new();
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let (px, py, pz) = (a.x + t * dx, a.y + t * dy, a.z + t * dz);
            let Some((low, high)) = st.clearance_bounds_for_profile(px, py, radius, &tool, &lut)
            else {
                continue;
            };
            let eps = 2.0 * f64::from(f32::EPSILON) * low.map_or(1.0, |l| l.abs().max(1.0));
            let low_hit = low.is_some_and(|l| pz < l - eps);
            let high_hit = pz < high - tau;
            if !(low_hit || high_hit) {
                continue;
            }
            eprintln!(
                "  sample {k}/{n} at ({px:.3},{py:.3},{pz:.3}): low {low:?} high {high:.4} \
                 tau {tau:.4} eps {eps:.2e}; flags low {low_hit} high {high_hit}; depth below \
                 low {:?}, below high-tau {:.4}",
                low.map(|l| l - eps - pz),
                high - tau - pz
            );
            // Re-derive the argmax cells of both channels.
            let reach = radius + cs * 0.5;
            let half_diag = cs * std::f64::consts::FRAC_1_SQRT_2;
            let inner_r = radius.min(lut.radius_sq().sqrt());
            let mut best_high: Option<(f64, usize, usize)> = None;
            let mut best_low: Option<(f64, usize, usize)> = None;
            for row in 0..g.rows {
                let cy = g.origin_v + row as f64 * cs;
                if (cy - py).abs() > reach {
                    continue;
                }
                for col in 0..g.cols {
                    let cx = g.origin_u + col as f64 * cs;
                    let d = (cx - px).hypot(cy - py);
                    if d > reach {
                        continue;
                    }
                    let far = d + half_diag;
                    if far <= inner_r
                        && let Some(top) = ray_top(g.ray(row, col))
                        && let Some(h) = lut.height_at_dist_sq((far * far).min(lut.radius_sq()))
                    {
                        let f = f64::from(top) - h;
                        if best_low.is_none_or(|(m, _, _)| f > m) {
                            best_low = Some((f, row, col));
                        }
                    }
                    let Some(h) = tool.height_at_radius((d - half_diag).max(0.0)) else {
                        continue;
                    };
                    let need = f64::from(g.conservative_top_at(row, col)) - h;
                    if best_high.is_none_or(|(m, _, _)| need > m) {
                        best_high = Some((need, row, col));
                    }
                }
            }
            for (name, best) in [("high", best_high), ("low", best_low)] {
                let Some((v, row, col)) = best else { continue };
                let (cx, cy) = g.cell_to_world(row, col);
                eprintln!(
                    "    {name} argmax {v:.4}: cell row {row} col {col} at ({cx:.3},{cy:.3}), \
                     centre distance {:.4}, conservative_top {:.4}, ray_top {:?}",
                    (cx - px).hypot(cy - py),
                    g.conservative_top_at(row, col),
                    ray_top(g.ray(row, col))
                );
                if !watch.contains(&(row, col)) {
                    watch.push((row, col));
                }
            }
            break;
        }
        // Per-move history of the watched cells: every move whose swept
        // stadium reaches a sub-sample, with its coverage mask and depth.
        let sub = [-3.0 / 8.0, -1.0 / 8.0, 1.0 / 8.0, 3.0 / 8.0];
        for &(row, col) in &watch {
            let (cx, cy) = g.cell_to_world(row, col);
            eprintln!("  history of cell ({row},{col}) at ({cx:.3},{cy:.3}):");
            let mut h = fresh();
            let mut prev = (
                h.z_grid.conservative_top_at(row, col),
                ray_top(h.z_grid.ray(row, col)),
            );
            for m in 1..mi {
                let (p, q) = (tp.moves[m - 1].target, tp.moves[m].target);
                // Flatten an arc for the coverage read only.
                let pts: Vec<(f64, f64)> = match tp.moves[m].move_type {
                    MoveType::ArcCW { i, j, .. } | MoveType::ArcCCW { i, j, .. } => {
                        let (ox, oy) = (p.x + i, p.y + j);
                        let r = i.hypot(j);
                        let a0 = (p.y - oy).atan2(p.x - ox);
                        let mut a1 = (q.y - oy).atan2(q.x - ox);
                        let cw = matches!(tp.moves[m].move_type, MoveType::ArcCW { .. });
                        if cw && a1 >= a0 {
                            a1 -= std::f64::consts::TAU;
                        }
                        if !cw && a1 <= a0 {
                            a1 += std::f64::consts::TAU;
                        }
                        (0..=64)
                            .map(|k| {
                                let t = a0 + (a1 - a0) * k as f64 / 64.0;
                                (ox + r * t.cos(), oy + r * t.sin())
                            })
                            .collect()
                    }
                    _ => vec![(p.x, p.y), (q.x, q.y)],
                };
                let mut mask = 0u16;
                for (vi, vo) in sub.iter().enumerate() {
                    for (ui, uo) in sub.iter().enumerate() {
                        let (sx, sy) = (cx + uo * cs, cy + vo * cs);
                        let inside = pts.windows(2).any(|w| {
                            let (ux, uy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
                            let l2 = ux * ux + uy * uy;
                            let t = if l2 > 0.0 {
                                (((sx - w[0].0) * ux + (sy - w[0].1) * uy) / l2).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            (sx - w[0].0 - t * ux).hypot(sy - w[0].1 - t * uy) <= radius
                        });
                        if inside {
                            mask |= 1 << (vi * 4 + ui);
                        }
                    }
                }
                h.simulate_toolpath_range(&tp, &tool, StockCutDirection::FromTop, m, m + 1);
                let now = (
                    h.z_grid.conservative_top_at(row, col),
                    ray_top(h.z_grid.ray(row, col)),
                );
                if mask != 0 && (p.z.min(q.z) < f64::from(prev.0) || now != prev) {
                    eprintln!(
                        "    move {m} {:?} z {:.3}->{:.3}: coverage {}/16 mask {mask:016b}; CT \
                         {:.4} -> {:.4}, ray_top {:?} -> {:?}",
                        tp.moves[m].intent,
                        p.z,
                        q.z,
                        mask.count_ones(),
                        prev.0,
                        now.0,
                        prev.1,
                        now.1
                    );
                } else if now != prev {
                    eprintln!(
                        "    move {m}: changed with no sub-sample covered: {prev:?} -> {now:?}"
                    );
                }
                prev = now;
            }
            let whole = (g.conservative_top_at(row, col), ray_top(g.ray(row, col)));
            eprintln!("    per-move replay {prev:?}, whole-range replay {whole:?}");
        }
    }
}
