//! P39: the ONE shape of a ramp entry of a 3D Rough.
//!
//! Record: `planning/feeds_contract_2026-11-03/phases/P39_RAMP_ENTRY_STOCK_2026-11-17.md`.
//!
//! Before P39 the planner read the stock on the tool disc at the entry XY
//! and stamped a column there, and the emitter shipped a zigzag along +X up
//! to [`crate::dressup::ramp_leg_cap_mm`] long. A leg met stock outside the
//! read disc at its full height, and the model clip (G-RAMPTERRAIN) lifted
//! a leg onto a ledge of the model, where it rode under stock that no lap
//! had lowered (Wanaka back, toolpath id 2: 14.5 mm).
//!
//! The rule:
//!
//! - [`plan`] chooses the legs ([`crate::dressup::RampLegs`]) where the
//!   stock, the model and the centre boundary are in scope. A leg runs only
//!   where the tool centre is inside the centre boundary and over the
//!   planner stock grid. It prefers a leg where the model floor is at or
//!   under the entry Z (the clip lifts no point of it).
//! - The planner reads the stock top over the swept footprint of that leg,
//!   so the ramp starts above every column that a leg meets.
//! - A leg that the clip lifts ships its whole clipped profile
//!   (`clip_polyline_to_floor`, `whole_profile`). So each lap lowers a
//!   column by one lap or less, lifted or not.
//! - [`emit`] is the one writer of the moves. The emitter calls it, and the
//!   planner stamps the moves that it writes (`clearing.rs`,
//!   `stamp_emitted_segment`), as the P22b bore does.
//!
//! With no run at all (a centre boundary narrower than the shortest leg) a
//! tool that cuts at its centre takes the peck ladder of the op. A tool that does not cut at its centre keeps the ramp of the
//! emitter along +X ([`EntryRamp::OpStyle`]): it cannot plunge. That case is
//! not covered by the rule.

use crate::dexel_stock::TriDexelStock;
use crate::dressup::RampLegs;
use crate::geo::P3;
use crate::toolpath::Toolpath;

use super::clearing::StampDrape;
use super::path::EntryFrame;

/// The ramp of the op: the angle that its entries ramp at and the
/// clearance above the contact top where a ramp starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RampRule {
    pub(super) angle_deg: f64,
    pub(super) entry_clearance_mm: f64,
}

/// The ramp of one entry, as the planner chose it ([`plan`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum EntryRamp {
    /// The planner chose no shape: the op states a plunge or a helix, the
    /// entry has no stock read, or the tool does not cut at its centre and
    /// no direction has a free run. A ramp then goes along +X with the cap
    /// of the angle, and the planner stamps the entry column.
    OpStyle,
    /// The zigzag that ships. The planner read the stock under it and
    /// stamped its moves.
    Legs { legs: RampLegs, rule: RampRule },
    /// No direction has a free run: the peck ladder of the op at the entry
    /// XY (`path.rs::emit_peck_plunge`). The read disc is its footprint and
    /// the entry column is its stamp.
    Peck,
}

/// What [`plan`] gives `plan_entry`: the shape and, for
/// [`EntryRamp::Legs`], the two stock reads over the swept footprint.
pub(super) struct PlannedRamp {
    pub(super) ramp: EntryRamp,
    /// The conservative stock top and the cell-centre stock top over the
    /// swept footprint of the longest leg that the shape permits.
    pub(super) footprint_tops: Option<(f64, f64)>,
}

/// The out-leg directions, +X first (the direction of every ramp before
/// P39, so an entry with room along +X keeps its moves), then the set of
/// `dressup::entry_descent::replan_descent_at_limit`.
const DIRECTIONS: [(f64, f64); 8] = {
    let d = std::f64::consts::FRAC_1_SQRT_2;
    [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (d, d),
        (d, -d),
        (-d, d),
        (-d, -d),
    ]
};

/// The slack of the "floor at or under the entry Z" test: the slack of the
/// model clip (`clip_polyline_to_floor` lifts a sample over `z + 1e-6`).
const FLOOR_EPS_MM: f64 = 1e-6;

/// Choose the legs of the ramp into `entry` (the draped entry) and read the
/// stock under them.
pub(super) fn plan(
    stock: &TriDexelStock,
    tool_radius: f64,
    entry: P3,
    rule: RampRule,
    drape: &StampDrape<'_>,
) -> PlannedRamp {
    let none = |ramp| PlannedRamp {
        ramp,
        footprint_tops: None,
    };
    let core = crate::tool::body::core_of(drape.cutter);
    let Some(leg_cap) = crate::dressup::ramp_leg_cap_mm(rule.angle_deg, core) else {
        // `emit_ramp` plunges at an angle that is no ramp angle.
        return none(EntryRamp::OpStyle);
    };
    // A full leg is always a run: at a steep angle the cap is under the
    // shortest run of the fold gate (the trap of P29-8e, Q8).
    let min_run = crate::dressup::ramp_min_run_mm(tool_radius, core).min(leg_cap);
    // The planner knows the stock only on its grid: a leg stays over it.
    let grid = &stock.z_grid;
    let on_grid = |x: f64, y: f64| {
        let u = (x - grid.origin_u) / grid.cell_size;
        let v = (y - grid.origin_v) / grid.cell_size;
        u >= 0.0
            && v >= 0.0
            && u <= grid.cols.saturating_sub(1) as f64
            && v <= grid.rows.saturating_sub(1) as f64
    };
    // Where the tool centre may stand at all, and where the model clip
    // lifts no point of a leg.
    let allowed =
        |x: f64, y: f64| on_grid(x, y) && drape.centre_clip.is_none_or(|clip| clip.contains(x, y));
    let unlifted = |x: f64, y: f64| {
        let cl = crate::surface::dropcutter::point_drop_cutter(
            x,
            y,
            drape.mesh,
            drape.index,
            drape.cutter,
        );
        !(cl.contacted && cl.z + drape.stock_to_leave > entry.z + FLOOR_EPS_MM)
    };
    let samples = crate::util::steps::steps(leg_cap, crate::dressup::ENTRY_CLIP_SPACING_MM)
        .unwrap_or(0)
        .max(1);
    // The run of `dir` from the entry while `ok` holds (mm).
    let run_of = |dir: (f64, f64), ok: &dyn Fn(f64, f64) -> bool| {
        let mut run = 0.0;
        for k in 1..=samples {
            let d = leg_cap * k as f64 / samples as f64;
            if !ok(entry.x + dir.0 * d, entry.y + dir.1 * d) {
                break;
            }
            run = d;
        }
        run
    };
    let top_of = |dir: (f64, f64), run: f64| {
        contact_top(stock, entry, leg_end(entry, dir, run), tool_radius)
    };
    // (direction, run, cell-centre top over the footprint)
    let mut best: Option<((f64, f64), f64, f64)> = None;
    // 1. A full leg that the clip does not lift: the lowest stock under it,
    //    so the shortest ramp. A tie keeps the first direction.
    // 2. Else the longest run that the clip does not lift.
    for dir in DIRECTIONS {
        let run = run_of(dir, &|x, y| allowed(x, y) && unlifted(x, y));
        if run < min_run {
            continue;
        }
        let top = top_of(dir, run);
        let full = run >= leg_cap - 1e-9;
        let better = best.is_none_or(|(_, best_run, best_top)| {
            let best_full = best_run >= leg_cap - 1e-9;
            match (full, best_full) {
                (true, false) => true,
                (false, true) => false,
                (true, true) => top < best_top - 1e-6,
                (false, false) => run > best_run + 1e-9,
            }
        });
        if better {
            best = Some((dir, run, top));
        }
    }
    // 3. Else (the entry stands on a slope of the model: every direction
    //    lifts) the longest run that the tool centre may take. The clip
    //    lifts the leg; it ships the whole profile, so a lap still lowers
    //    a column by one lap or less (`clip_polyline_to_floor`).
    if best.is_none() {
        for dir in DIRECTIONS {
            let run = run_of(dir, &allowed);
            if run >= min_run && best.is_none_or(|(_, best_run, _)| run > best_run + 1e-9) {
                best = Some((dir, run, top_of(dir, run)));
            }
        }
    }
    let Some((dir, run, contact)) = best else {
        return none(if core.is_some() {
            EntryRamp::OpStyle
        } else {
            EntryRamp::Peck
        });
    };
    let floor =
        super::keep_down::corridor_ceiling_z(stock, entry, leg_end(entry, dir, run), tool_radius);
    PlannedRamp {
        ramp: EntryRamp::Legs {
            legs: RampLegs {
                dir,
                max_leg_mm: Some(run),
            },
            rule,
        },
        footprint_tops: Some((floor, contact)),
    }
}

fn leg_end(entry: P3, dir: (f64, f64), run: f64) -> P3 {
    P3::new(entry.x + dir.0 * run, entry.y + dir.1 * run, entry.z)
}

/// The highest cell-centre stock top under the tool disc along the leg from
/// `from` to `to` (`NEG_INFINITY` with no material). The discs stand half a
/// cell apart, and the radius grows by the sagitta of that step, so every
/// cell centre inside the swept footprint is read.
fn contact_top(stock: &TriDexelStock, from: P3, to: P3, radius: f64) -> f64 {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let len = dx.hypot(dy);
    let step = (stock.z_grid.cell_size * 0.5).max(1e-3);
    let n = crate::util::steps::steps(len, step).unwrap_or(0).max(1);
    let reach = radius.hypot(step * 0.5);
    (0..=n)
        .filter_map(|k| {
            let t = k as f64 / n as f64;
            stock.max_top_z_in_disc(from.x + dx * t, from.y + dy * t, reach)
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

/// The ramp of one entry, from `(entry.xy, descent_floor)` down to the
/// draped `entry`. The emitter cuts it and the planner stamps the moves it
/// writes, so plan equals cut (G-SIMCORE).
#[expect(
    clippy::too_many_arguments,
    reason = "the entry, its descent floor, the entry reads, the legs, the angle and the feed are each one input"
)]
pub(super) fn emit(
    tp: &mut Toolpath,
    frame: &EntryFrame<'_>,
    entry: &P3,
    descent_floor: f64,
    (material_top, measured, contact_top, core_top): (f64, bool, Option<f64>, Option<f64>),
    legs: RampLegs,
    angle_deg: f64,
    feed_rate: f64,
) -> Option<crate::compute::tool_config::NonCentreCuttingRefusal> {
    let start = P3::new(entry.x, entry.y, descent_floor);
    crate::dressup::emit_ramp(
        tp,
        &start,
        entry,
        legs,
        angle_deg,
        feed_rate,
        &frame.safety(material_top, measured, contact_top, core_top, false),
        // G-RAMPCONTAIN: no fold. The legs are the planner's.
        None,
    )
}
