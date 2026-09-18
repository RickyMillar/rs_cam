//! Entry-descent step machinery: the ramp and helix emitters and the
//! fold-path walk that plans a ramp along the cut that follows a plunge.
//!
//! Split out of `dressup/mod.rs` (P4). The section's public entry points
//! (`apply_entry`, `optimize_entry_descents*`) stay in the parent.

use crate::geo::P3;
use crate::toolpath::{Move, MoveType, Toolpath};

use super::{EntrySafety, EntrySurfaceProbe, OffMeshEntry, is_plunge};

/// The polyline one entry-lap window is planned along: the plunge's own
/// target, then the cutting moves that continue from it.
///
/// Bounded in points and in arclength. The planner truncates at its own window
/// target (about 1.2 mm at the shipped dials), so reading further is waste,
/// and an unbounded read would walk a whole 90 000-move ring set once per
/// entry.
pub(super) fn upcoming_run(
    followers: &[(P3, bool, crate::toolpath::MoveIntent)],
    plunge_index: usize,
) -> Vec<P3> {
    const MAX_POINTS: usize = 256;
    const MAX_ARC_MM: f64 = 5.0;

    let Some(&(first, _, _)) = followers.get(plunge_index) else {
        return Vec::new();
    };
    let mut run = vec![first];
    let mut arc = 0.0_f64;
    let mut prev = first;
    for &(point, cutting, _) in followers.iter().skip(plunge_index + 1) {
        if !cutting {
            break;
        }
        arc += ((point.x - prev.x).powi(2) + (point.y - prev.y).powi(2)).sqrt();
        run.push(point);
        prev = point;
        if run.len() >= MAX_POINTS || arc >= MAX_ARC_MM {
            break;
        }
    }
    run
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub(super) fn find_next_xy_direction(moves: &[Move], from_idx: usize) -> (f64, f64) {
    let base = &moves[from_idx].target;
    for m in &moves[from_idx + 1..] {
        let dx = m.target.x - base.x;
        let dy = m.target.y - base.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > 0.1 {
            return (dx / dist, dy / dist);
        }
    }
    (1.0, 0.0) // fallback: ramp along X
}

/// How much of the following cut a ramp fold can consume, in mm of XY.
///
/// The fold walks out for half the total ramp length and retraces it, so
/// half is all it can use. An invalid angle gets zero: [`emit_ramp`] falls
/// back to a straight plunge before it looks at the fold.
pub(super) fn fold_walk_budget(max_angle_deg: f64) -> f64 {
    if max_angle_deg <= 0.0 || max_angle_deg >= 90.0 {
        return 0.0;
    }
    ENTRY_CLEARANCE / max_angle_deg.to_radians().tan() / 2.0
}

/// Sample spacing (mm) used to linearise an arc in the following cut.
const FOLD_ARC_SPACING_MM: f64 = 0.5;

/// Collect the fed cut moves that follow the plunge at `from_idx`.
///
/// The walk starts AT the plunge target and stops at the first rapid, the
/// first following plunge, or once it has `want` mm of XY in hand — the
/// most a fold can consume. Arc moves are linearised.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub(super) fn collect_following_cut(moves: &[Move], from_idx: usize, want: f64) -> Vec<P3> {
    let mut points = vec![moves[from_idx].target];
    if want <= 0.0 {
        return points;
    }
    let mut acc = 0.0;
    let mut prev = moves[from_idx].target;
    for idx in from_idx + 1..moves.len() {
        let n = &moves[idx];
        match n.move_type {
            MoveType::Rapid => break,
            MoveType::Linear { .. } => {
                if is_plunge(&moves[idx - 1], n) {
                    break;
                }
                points.push(n.target);
            }
            MoveType::ArcCW { i, j, .. } => {
                points.extend(crate::geometry::arc_util::linearize_arc(
                    prev,
                    n.target,
                    i,
                    j,
                    true,
                    FOLD_ARC_SPACING_MM,
                ));
            }
            MoveType::ArcCCW { i, j, .. } => {
                points.extend(crate::geometry::arc_util::linearize_arc(
                    prev,
                    n.target,
                    i,
                    j,
                    false,
                    FOLD_ARC_SPACING_MM,
                ));
            }
        }
        let dx = n.target.x - prev.x;
        let dy = n.target.y - prev.y;
        acc += (dx * dx + dy * dy).sqrt();
        prev = n.target;
        if acc >= want {
            break;
        }
    }
    points
}

/// Clearance height (mm) above cut depth to start ramping/helixing.
pub(crate) const ENTRY_CLEARANCE: f64 = 2.0;

/// The operation's own following cut moves, for a ramp to fold along
/// (G-RAMPCONTAIN, 2026-09-10).
///
/// [`emit_ramp`] used to draw two blind straight legs of
/// `ENTRY_CLEARANCE / tan(angle) / 2` mm — 19.08 mm at the shipped 3
/// degrees, for every depth per pass — from the entry column along the
/// first following chord. The emitter holds no region, no polygon and no
/// tool radius, so it could not constrain them: on
/// `fixtures/demo_pocket.svg` the return leg ran 11 mm past the pocket
/// wall and cut the surrounding stock, under a clean verdict
/// (UX-R03-001).
///
/// The fold rides [`Self::follow`] instead. Every point of that polyline
/// is a tool-centre point the generator itself placed at this level, so
/// the entry is contained wherever the operation's own cut is contained —
/// with no region polygon, no offset call and no tool radius needed at
/// emit time. The slope stays `tan(angle)` because Z is interpolated by
/// cumulative XY distance, and a chord is never longer than the arc it
/// replaces, so a folded leg is never steeper than the planned one.
///
/// `None` at the call site keeps the legacy straight legs. The adaptive3d
/// door passes `None`: it enters prism stock with `dir = (1.0, 0.0)` and a
/// leg past the mesh footprint cuts stock that operation is allowed to
/// cut (R0.2 §2.2).
pub(crate) struct RampFold<'a> {
    /// The fed cut moves that follow the plunge, in emission order,
    /// starting AT the plunge target. Arc moves are already linearised.
    /// The walk stops at the next rapid or the next plunge.
    pub follow: &'a [P3],
    /// True where `follow` closes back onto the plunge target in XY, so
    /// the walk laps the ring rather than bouncing off its far end.
    pub closed: bool,
    /// Below this XY run length the fold degrades to a plunge. A run
    /// shorter than the tool radius is a scrub in place, not a ramp.
    pub min_run_mm: f64,
}

/// Guard on the lap / bounce loop in [`extend_fold_path`]. A run of any
/// usable length reaches 19 mm in a handful of laps; the cap only stops a
/// degenerate run of near-zero-length segments from spinning.
const FOLD_EXTEND_MAX_ROUNDS: usize = 64;

/// The most laps a ramp fold may lay over its run (R10, 2026-09-18).
///
/// A lap is one traverse of the run. The count is the whole ramp's XY
/// length — out AND back, `2 * half_len` — divided by the run's XY length.
/// On a closed ring the run is the ring, so a lap is one circuit.
///
/// A fold exists to fit a ramp into a run of a few tool diameters. It does
/// not exist to saw a slot. The Corne waterline (§4.5 of the case
/// analysis) had a 2 mm run inside a 3 mm wall: a 2 mm drop at 3 degrees
/// is 38 mm of XY, so the fold laid 19 laps over that run at every level,
/// and the simulation showed the wall sawn to a row of pins. Above this
/// cap the fold returns empty and [`emit_ramp`] degrades to the plunge.
///
/// [`collect_following_cut`] stops at `half_len` mm of XY, so a run that
/// reaches the budget measures at most 2 laps here. The cap bites only on
/// a run SHORTER than half a ramp: 12.7 mm at the shipped 3 degrees.
pub(crate) const RAMP_FOLD_MAX_LAPS: f64 = 3.0;

/// Grow `base` until its XY length reaches `want`, by lapping a closed
/// ring or bouncing off the far end of an open run.
///
/// Returns `base` unchanged when it is already long enough, and gives up
/// (returning what it has) on a run too degenerate to grow.
// SAFETY: every slice below is guarded by the `base.len() < 2` early return.
#[allow(clippy::indexing_slicing)]
fn extend_fold_path(base: &[P3], closed: bool, want: f64) -> Vec<P3> {
    let mut ext = base.to_vec();
    if base.len() < 2 {
        return ext;
    }
    let per_round = crate::geo::polyline_xy_length(base);
    if per_round <= 1e-9 {
        return ext;
    }
    let mut rounds = 0;
    while crate::geo::polyline_xy_length(&ext) < want && rounds < FOLD_EXTEND_MAX_ROUNDS {
        rounds += 1;
        if closed {
            // The ring returns to its own start, so replaying it from the
            // second point continues the lap without a duplicate vertex.
            ext.extend_from_slice(&base[1..]);
        } else {
            // Bounce: back down the run, then out along it again.
            let back: Vec<P3> = base.iter().rev().skip(1).copied().collect();
            ext.extend_from_slice(&back);
            ext.extend_from_slice(&base[1..]);
        }
    }
    ext
}

/// Walk `path` from its start for `want` mm of XY distance.
///
/// Returns every vertex crossed plus the interpolated turn-around point.
/// A path SHORTER than `want` is returned whole and therefore falls short.
/// That is not harmless — the same Z drop over less XY is a STEEPER ramp
/// than the angle dial asked for — so [`fold_ramp_points`] checks the
/// walked length and degrades to a plunge rather than emit it.
// SAFETY: `windows(2)` yields slices of exactly two elements.
#[allow(clippy::indexing_slicing)]
fn walk_fold_path(path: &[P3], want: f64) -> Vec<P3> {
    let mut walked = Vec::new();
    let Some(first) = path.first() else {
        return walked;
    };
    walked.push(*first);
    let mut acc = 0.0;
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let seg = (dx * dx + dy * dy).sqrt();
        if seg <= 1e-12 {
            continue;
        }
        if acc + seg >= want {
            let t = (want - acc) / seg;
            walked.push(P3::new(a.x + dx * t, a.y + dy * t, a.z + (b.z - a.z) * t));
            return walked;
        }
        acc += seg;
        walked.push(b);
    }
    walked
}

/// Build the folded ramp: out along the cut polyline for `half_len`, then
/// back over the same ground to the entry column.
///
/// Z is interpolated by cumulative XY distance over the whole out-and-back,
/// from `ramp_start_z` at the entry column down to `end_z` when it returns
/// there. Each point is then FLOORED at the cut polyline's own Z, which is
/// inert on a level pass and stops the fold diving under a shallow carve.
///
/// The returned points exclude the starting position — the caller already
/// stands at `(entry column, ramp_start_z)`.
///
/// Returns empty when the ramp would lap the run more than
/// [`RAMP_FOLD_MAX_LAPS`] times (R10). The caller degrades to a plunge.
// SAFETY: `windows(2)` yields slices of exactly two elements.
#[allow(clippy::indexing_slicing)]
fn fold_ramp_points(
    follow: &[P3],
    closed: bool,
    half_len: f64,
    ramp_start_z: f64,
    end_z: f64,
) -> Vec<P3> {
    // R10: the lap cap. `follow` starts AT the plunge target, and on a
    // closed ring it ends there too, so its XY length is the run length
    // in both cases: the open run, or one circuit of the ring.
    let run = crate::geo::polyline_xy_length(follow);
    if run <= 1e-9 || 2.0 * half_len > RAMP_FOLD_MAX_LAPS * run {
        return Vec::new();
    }
    let extended = extend_fold_path(follow, closed, half_len);
    let out = walk_fold_path(&extended, half_len);
    if out.len() < 2 {
        return Vec::new();
    }
    // The walk must reach the full half length. Falling short would drop the
    // same 2 mm over less XY, which is a ramp STEEPER than the angle dial
    // asked for. The caller degrades to a plunge instead.
    //
    // `extend_fold_path` cannot cap out on a run the caller admits — its
    // guard is 64 rounds and the caller refuses a run under 1 mm, so the
    // extension reaches at least 64 mm against a half length of 19.08 mm at
    // the shipped 3 degrees. The lap cap above refuses earlier still: a run
    // that passes it is at least `2 * half_len / RAMP_FOLD_MAX_LAPS` long.
    // This check stays as a second guard behind both.
    if crate::geo::polyline_xy_length(&out) < half_len - 1e-6 {
        return Vec::new();
    }
    // Out, then the same vertices in reverse: the classic zigzag ramp bent
    // onto the cut path.
    let mut path: Vec<P3> = out.clone();
    path.extend(out.iter().rev().skip(1).copied());

    let total = crate::geo::polyline_xy_length(&path);
    if total <= 1e-9 {
        return Vec::new();
    }
    let drop = ramp_start_z - end_z;
    let mut acc = 0.0;
    let mut points = Vec::with_capacity(path.len());
    for (idx, w) in path.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        acc += (dx * dx + dy * dy).sqrt();
        let planned_z = ramp_start_z - drop * (acc / total);
        // The last point must land exactly on the entry target, so the
        // cut that follows starts where the generator put it.
        let is_last = idx + 2 == path.len();
        let z = if is_last { end_z } else { planned_z.max(b.z) };
        points.push(P3::new(b.x, b.y, z));
    }
    points
}

// SAFETY: eight parameters, one over clippy's threshold. The eighth is
// `fold`, and grouping the rest into a struct would move the emitter's
// existing contract for one added argument.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_ramp(
    tp: &mut Toolpath,
    start: &P3,
    end: &P3,
    dir: (f64, f64),
    max_angle_deg: f64,
    feed_rate: f64,
    safety: &EntrySafety<'_>,
    fold: Option<&RampFold<'_>>,
) {
    use crate::toolpath::MoveIntent;
    let stock_top = safety.stock_top;
    if max_angle_deg <= 0.0 || max_angle_deg >= 90.0 {
        // Invalid angle — fall back to straight plunge
        tp.feed_to_with_intent(*end, feed_rate, MoveIntent::EntryPlunge);
        return;
    }

    // G-ISOCLIPENTRY: on a REST-DRIVEN pass the two-leg zigzag below is not
    // bite budgeted. Its closing leg returns to the start column and takes the
    // whole remaining depth off it in one pass, and the vertical pre-plunge
    // above it takes whatever stands between the clearance height and
    // `end.z + ENTRY_CLEARANCE`. Against the MODEL that depth is nothing —
    // the ramp starts 2 mm over the finished surface. Against the stock an
    // upstream tool left standing it is the rest depth, measured on the
    // wanaka200 tier-1 islands as a 1.67 mm bite at the entry column.
    //
    // Where a rest stock is in scope, plan the bite-budgeted lap ladder
    // instead — `pencil::plan_entry_ramp`, the same construction site and the
    // same physical model the pencil family uses against G-ENTRYLOAD. The run
    // is SYNTHESISED from `dir`, because this emitter holds a direction rather
    // than the polyline it is entering: two points one window apart, the far
    // one lifted to the probe floor so no lap chord can sag under the surface.
    if let Some(probe) = &safety.surface
        && let Some(stock) = probe.rest_stock
    {
        let contact = crate::finish::pencil::tip_contact_radius(probe.cutter);
        let window = crate::finish::pencil::entry_ramp_window_mm(contact);
        let far_xy = (end.x + dir.0 * window, end.y + dir.1 * window);
        // G-ISOCLIPRAMPFALL: where a rest stock is in scope this emitter is
        // LADDER OR PLUNGE, and never the legacy legs below. Both of the
        // planner's abstentions used to fall through to them, and the legs
        // are `ENTRY_CLEARANCE / tan(angle)` mm long — 38 mm at the shipped
        // 3 degrees — clipped only to the MODEL floor. On a rest-driven pass
        // that floor lies BELOW the material, so the return leg walks back
        // across standing rest stock at full depth. Measured on the
        // wanaka200 tier-1 islands at 0.2 mm: 1.394 mm off one column, 2.8x
        // the per-lap budget, on a move the post-clip door then declined to
        // re-plan because it already carried an `EntryRamp` tag.
        //
        // The far point's floor stops the lap chords sagging under the
        // model. When the probe has no answer there — the window leaves the
        // mesh footprint — the entry depth itself is the conservative floor:
        // the ladder never descends below `max(level, run z)`, so a flat far
        // point can only make the laps shallower.
        let far_floor = probe.floor_z(far_xy.0, far_xy.1).unwrap_or(end.z);
        let run = [*end, P3::new(far_xy.0, far_xy.1, far_floor.max(end.z))];
        match crate::finish::pencil::plan_entry_ramp(&run, stock, contact, start.z, true) {
            Some(plan) => {
                // Air only: the ladder starts at the conservative stock
                // ceiling read over the whole window.
                if plan.air_descent_z < start.z - 1e-9 {
                    tp.feed_to_with_intent(
                        P3::new(start.x, start.y, plan.air_descent_z),
                        feed_rate,
                        MoveIntent::EntryPlunge,
                    );
                }
                for p in &plan.points {
                    tp.feed_to_with_intent(*p, feed_rate, MoveIntent::EntryRamp);
                }
            }
            None => {
                // Nothing stands over the window that the budget does not
                // already cover, so descend at the entry column. The
                // generator put that target on the intended surface, which
                // is what `OffMeshEntry::PlungeFallback` states as the
                // policy for an entry the shaped manoeuvre cannot serve.
                // The rapid keeps the air part at rapid speed, as the legacy
                // pre-descent did.
                // The conservative ceiling can read BELOW the target where the
                // upstream tool already cut this column past it, so the rapid
                // floor is held one plunge clearance over the target: a rapid
                // to the target itself leaves a zero-length feed, and no rapid
                // at all feeds the whole descent from safe Z through air.
                let air_z = stock
                    .max_conservative_top_z_in_disc(end.x, end.y, contact)
                    .map_or(start.z, |top| top + crate::toolpath::PLUNGE_CLEARANCE_MM)
                    .max(end.z + crate::toolpath::PLUNGE_CLEARANCE_MM);
                if air_z < start.z - 1e-9 {
                    tp.rapid_to_with_intent(P3::new(start.x, start.y, air_z), MoveIntent::Linking);
                }
                tp.feed_to_with_intent(*end, feed_rate, MoveIntent::EntryPlunge);
            }
        }
        return;
    }

    // Only ramp the last portion of the descent (max 5mm above target).
    // Rapid down to clearance first, then ramp the rest.
    let clearance = ENTRY_CLEARANCE;
    let ramp_start_z = end.z + clearance;

    if start.z > ramp_start_z + 0.1 {
        // Split the descent so the rapid stays in air.
        //
        // The cutter starts at `start.z` (above stock, by `safe_z`
        // convention) and wants to be at `ramp_start_z` (which is
        // typically below `stock_top` for any cut into material). A
        // straight rapid would punch through uncut stock at the new
        // contour's XY column (UX-dial-in B1 — pre-fix this fired 28
        // collisions on a default-skeleton pocket). Instead, rapid
        // only down to `stock_top + clearance` (still in air), then
        // feed-plunge through material to `ramp_start_z` before the
        // angled ramp begins.
        let safe_rapid_floor = stock_top + clearance;
        let rapid_target_z = ramp_start_z.max(safe_rapid_floor).min(start.z);
        if start.z - rapid_target_z > 0.1 {
            tp.rapid_to_with_intent(
                P3::new(start.x, start.y, rapid_target_z),
                MoveIntent::Linking,
            );
        }
        if rapid_target_z - ramp_start_z > 0.1 {
            tp.feed_to_with_intent(
                P3::new(start.x, start.y, ramp_start_z),
                feed_rate,
                MoveIntent::EntryPlunge,
            );
        }
    }

    let ramp_dz = (ramp_start_z - end.z).abs().max(0.1);
    let max_angle_rad = max_angle_deg.to_radians();
    let ramp_xy_len = ramp_dz / max_angle_rad.tan();

    // Ramp out along direction, then back (zigzag ramp)
    let half_len = ramp_xy_len / 2.0;
    let mid_z = (ramp_start_z + end.z) / 2.0;

    let planned = [
        P3::new(start.x, start.y, ramp_start_z),
        P3::new(
            start.x + dir.0 * half_len,
            start.y + dir.1 * half_len,
            mid_z,
        ),
        P3::new(start.x, start.y, end.z),
    ];
    if let Some(probe) = &safety.surface {
        // G-RAMPTERRAIN: clip the legs to the drop-cutter floor. On a
        // lost surface contact, fall back to the straight plunge — the
        // plunge target sits on the intended surface by construction.
        match clip_polyline_to_floor(&planned, probe) {
            Some(points) => {
                for p in points {
                    tp.feed_to_with_intent(p, feed_rate, MoveIntent::EntryRamp);
                }
            }
            None => {
                tp.feed_to_with_intent(*end, feed_rate, MoveIntent::EntryPlunge);
            }
        }
    } else if let Some(fold) = fold {
        // G-RAMPCONTAIN: no surface probe, so this is a PRISM operation and
        // the legs below it were blind in XY as well as in Z. Fold them
        // along the operation's own following cut instead — see
        // [`RampFold`]. The degrade is a plunge, never a refusal: the entry
        // column is a cut point of the operation by construction.
        //
        // R10 (2026-09-18): containment alone is not enough. A fold that
        // stays on the cut but laps a 2 mm run 19 times is contained AND a
        // saw. `fold_ramp_points` also returns empty above
        // [`RAMP_FOLD_MAX_LAPS`], and that empty takes the same plunge.
        let run = crate::geo::polyline_xy_length(fold.follow);
        let folded = if fold.follow.len() >= 2 && run >= fold.min_run_mm {
            fold_ramp_points(fold.follow, fold.closed, half_len, ramp_start_z, end.z)
        } else {
            Vec::new()
        };
        if folded.is_empty() {
            tp.feed_to_with_intent(*end, feed_rate, MoveIntent::EntryPlunge);
        } else {
            for p in folded {
                tp.feed_to_with_intent(p, feed_rate, MoveIntent::EntryRamp);
            }
        }
    } else {
        // Legacy blind legs — honest only with no mesh surface to probe AND
        // no following cut to fold along. The adaptive3d door is the one
        // caller that lands here (R0.2 section 2.2).
        tp.feed_to_with_intent(planned[1], feed_rate, MoveIntent::EntryRamp);
        tp.feed_to_with_intent(planned[2], feed_rate, MoveIntent::EntryRamp);
    }
}

/// Sample spacing (mm) for the entry-leg clip against the probe floor.
/// Between two samples a chord can sag below a convex surface by
/// `curvature * spacing^2 / 8`; at 0.5 mm this stays far inside the
/// sentry's 0.2 mm tolerance for any surface a cutter can follow.
const ENTRY_CLIP_SPACING_MM: f64 = 0.5;

/// Clip a planned entry polyline to the probe floor.
///
/// Samples each segment every [`ENTRY_CLIP_SPACING_MM`] and lifts each
/// sample to `max(planned z, floor)`. Returns the points to emit: every
/// lifted sample plus each segment's own endpoint, so an unclipped
/// polyline round-trips to exactly the legacy moves. The first point of
/// `planned` is the current tool position — it seeds the sampling and
/// is never emitted. Returns `None` when the probe loses surface
/// contact at any sample.
///
/// A chord between an emitted lifted point and the next emitted point
/// stays at or above the planned straight line, and every skipped
/// sample was measured at or above the floor, so the emitted path
/// never dips below a measured sample.
#[allow(clippy::indexing_slicing)] // windows(2) pairs, bounded
fn clip_polyline_to_floor(planned: &[P3], probe: &EntrySurfaceProbe<'_>) -> Option<Vec<P3>> {
    let mut out = Vec::new();
    for pair in planned.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        let n = (len / ENTRY_CLIP_SPACING_MM).ceil().max(1.0) as usize;
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let q = P3::new(a.x + dx * t, a.y + dy * t, a.z + dz * t);
            let floor = match probe.floor_z(q.x, q.y) {
                Some(f) => f,
                None => match probe.off_mesh {
                    OffMeshEntry::PlungeFallback => return None,
                    OffMeshEntry::Unconstrained => f64::NEG_INFINITY,
                },
            };
            let lifted = floor > q.z + 1e-6;
            if lifted {
                out.push(P3::new(q.x, q.y, floor));
            } else if k == n {
                out.push(q);
            }
        }
    }
    Some(out)
}

pub(crate) fn emit_helix(
    tp: &mut Toolpath,
    start: &P3,
    end: &P3,
    radius: f64,
    pitch: f64,
    feed_rate: f64,
    safety: &EntrySafety<'_>,
) {
    use crate::toolpath::MoveIntent;
    let stock_top = safety.stock_top;
    // Only helix the last portion — rapid down to clearance first
    let clearance = ENTRY_CLEARANCE;
    let helix_start_z = end.z + clearance;

    if start.z > helix_start_z + 0.1 {
        // Same lift-bridge guard as `emit_ramp` (UX-dial-in B1): only
        // rapid through air; plunge-feed any portion that would
        // otherwise punch through uncut material above `helix_start_z`.
        let safe_rapid_floor = stock_top + clearance;
        let rapid_target_z = helix_start_z.max(safe_rapid_floor).min(start.z);
        if start.z - rapid_target_z > 0.1 {
            tp.rapid_to_with_intent(
                P3::new(start.x, start.y, rapid_target_z),
                MoveIntent::Linking,
            );
        }
        if rapid_target_z - helix_start_z > 0.1 {
            tp.feed_to_with_intent(
                P3::new(start.x, start.y, helix_start_z),
                feed_rate,
                MoveIntent::EntryPlunge,
            );
        }
    }

    let dz = (helix_start_z.min(start.z) - end.z).abs();
    if dz < 0.01 || pitch < 0.01 || radius <= 0.0 {
        tp.feed_to_with_intent(*end, feed_rate, MoveIntent::EntryPlunge);
        return;
    }

    let revolutions = dz / pitch;
    let total_angle = revolutions * std::f64::consts::TAU;
    let steps_per_rev = 36; // 10° per step
    let total_steps = (revolutions * steps_per_rev as f64).ceil() as usize;
    if total_steps == 0 {
        tp.feed_to_with_intent(*end, feed_rate, MoveIntent::EntryPlunge);
        return;
    }

    let center_x = end.x;
    let center_y = end.y;
    let helix_top = helix_start_z.min(start.z);

    let mut turns = Vec::with_capacity(total_steps);
    for i in 1..=total_steps {
        let t = i as f64 / total_steps as f64;
        let angle = total_angle * t;
        let z = helix_top - dz * t;
        let (sin_a, cos_a) = angle.sin_cos();
        let x = center_x + radius * cos_a;
        let y = center_y + radius * sin_a;
        turns.push(P3::new(x, y, z));
    }

    // G-RAMPTERRAIN: clip every turn sample to the drop-cutter floor.
    // The 10-degree step is already finer than the leg-clip spacing.
    // On a lost surface contact, fall back to the straight plunge.
    if let Some(probe) = &safety.surface {
        for q in &mut turns {
            let Some(floor) = probe.floor_z(q.x, q.y) else {
                match probe.off_mesh {
                    OffMeshEntry::PlungeFallback => {
                        tp.feed_to_with_intent(*end, feed_rate, MoveIntent::EntryPlunge);
                        return;
                    }
                    OffMeshEntry::Unconstrained => continue,
                }
            };
            if floor > q.z {
                q.z = floor;
            }
        }
    }
    for q in turns {
        tp.feed_to_with_intent(q, feed_rate, MoveIntent::EntryHelix);
    }

    // Return to center at final Z
    tp.feed_to_with_intent(*end, feed_rate, MoveIntent::EntryHelix);
}
