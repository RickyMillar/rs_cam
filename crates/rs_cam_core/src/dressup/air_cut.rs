//! Air-cut sample classification: how deep the tip sits under the stock
//! surface, and whether a sample or a whole swept move is in air.
//!
//! Split out of `dressup/mod.rs` (P4). The section's entry points
//! (`filter_air_cuts*`, `reference_engagement_of_cutting_moves`) stay in the
//! parent.

use crate::dexel_stock::TriDexelStock;
use crate::geo::P3;
use crate::toolpath::{Move, MoveType};

/// How far the tool tip at `(x, y, z)` sits BELOW the stock surface, mm.
///
/// Positive = the tip is under the surface, i.e. in material. Negative = it
/// is above it, in air. `None` = there is nothing to be in or out of: the
/// column is empty (a through-hole) or the position is off the grid.
///
/// Nearest-cell lookup, never interpolated — the same rule
/// `rest_field::measure_cross_section` states for the same reason: averaging
/// dexel tops across a steep wall smears the cliff.
///
/// This is the ONE convention for "is the tool in material against this
/// stock". [`is_in_air`] and A4's zero-removal measurement are both phrased
/// on it rather than each carrying their own lookup, because two surfaces
/// answering that question differently is how they come to disagree about
/// what a pass did.
fn tip_depth_below_surface(stock: &TriDexelStock, x: f64, y: f64, z: f64) -> Option<f64> {
    let (row, col) = stock.z_grid.world_to_cell(x, y)?;
    let top = stock.z_grid.top_z_at(row, col)?;
    Some(top as f64 - z)
}

/// Check if position (x, y, z) is in air (no material above z at this XY).
///
/// ZERO-RADIUS: the tool's centerline column only. It is the cheap first
/// stage of [`sample_is_air_for_tool`], never a clearance answer on its own —
/// see that function for why.
fn is_in_air(stock: &TriDexelStock, x: f64, y: f64, z: f64, tolerance: f64) -> bool {
    // Empty ray or off-grid = definitely air.
    tip_depth_below_surface(stock, x, y, z).is_none_or(|depth| depth < -tolerance)
}

/// Is a tip at `(x, y, z)` in air **for this cutter** — nothing under its
/// whole envelope that its own profile does not clear?
///
/// S3 (`planning/rapid_safety_2026-08-28/`). The question the air-cut filter
/// has to answer is whether the TOOL passes through air, and the tool is not
/// a point. S1 measured the consequence of asking the centerline instead: on
/// wanaka the exact-XY column of a finish plunge read clear by +0.1 mm while
/// the taper's flank stood −1.3 mm inside an inter-pass crest 0.5–2.9 mm
/// off-axis, so the filter reclassified a safe fed plunge as all-air and
/// [`filter_air_cuts`] replaced it with a rapid descending to the resume Z.
/// 982 such descents shipped in two G-code programs, counted by nothing.
///
/// The clearance question is the same one S2 gave the detector, asked of the
/// same primitive: `max_clearance_tip_z_for_profile` returns the lowest tip Z
/// that clears every cell under the envelope disc, evaluating the cutter's
/// own height at each cell's nearest possible material. `None` = no cell
/// under the disc constrains the tip = air.
///
/// # Two stages, and why the cheap one is sound
///
/// Stage 1 is the centerline point test; stage 2 (the disc) runs ONLY when
/// stage 1 says air. Skipping stage 2 on a material verdict does not change
/// the answer: the sample's own cell (the same `round()` on both sides) sits
/// at a cell-centre distance ≤ one half-diagonal, so the dilated disc visits
/// it, evaluates it at `r_near = 0` where `height_at_radius(0) = 0` for every
/// shipped shape, and reads `conservative_top ≥ ray_top`. So
/// `clearance ≥ ray_top`, and `clearance + tolerance <= z` therefore implies
/// `ray_top + tolerance <= z` — stage-1 air. Contrapositive: stage-1 material
/// ⟹ stage-2 material.
///
/// The one place the identity could fail is a cutter so small the dilated
/// disc no longer reaches its own centre cell — `envelope_radius` below
/// 0.21 × the cell — and there the early return is the CONSERVATIVE side
/// ("material"), so the safety claim holds for every radius.
///
/// Cost follows from that implication: every sample that touches material at
/// the centerline — most samples of a finishing pass — pays one grid lookup,
/// and only the airborne minority pays the disc.
pub(super) fn sample_is_air_for_tool(
    stock: &TriDexelStock,
    cutter: &dyn crate::tool::MillingCutter,
    x: f64,
    y: f64,
    z: f64,
    tolerance: f64,
) -> bool {
    if !is_in_air(stock, x, y, z, tolerance) {
        return false;
    }
    let radius = cutter.envelope_radius_mm();
    stock
        .max_clearance_tip_z_for_profile(x, y, radius, cutter)
        .is_none_or(|clearance| clearance + tolerance <= z)
}

/// A4: how far material at the sampled column rises ABOVE the cutter's own
/// surface there — i.e. how much this position actually removes.
///
/// The naive form of this measure (stock top minus TIP Z) reads a false
/// positive on every curved or sloped surface, and the size of the error is
/// exactly the grid's lateral quantisation: `world_to_cell` snaps to the
/// nearest ray, up to `cell/√2` away from the tool axis, and the machined
/// surface at that offset is legitimately higher than the tip by the
/// cutter's own profile. On the A4 fixture that artefact measured
/// **+21 µm** — the same order as the cusp the dials asked for, and enough
/// to hide a pass that removes nothing behind a number that looks like
/// engagement. It was found by the sentry failing, not by reasoning.
///
/// So the comparison is against `tip_z + height_at_radius(offset)`: the
/// height of the cutter's surface directly above that ray. Material above
/// THAT is material the cutter removes; material below it is the shape the
/// cutter leaves behind.
pub(super) fn material_above_cutter(
    stock: &TriDexelStock,
    cutter: &dyn crate::tool::MillingCutter,
    x: f64,
    y: f64,
    z: f64,
) -> Option<f64> {
    let (row, col) = stock.z_grid.world_to_cell(x, y)?;
    let top = stock.z_grid.top_z_at(row, col)?;
    let (cx, cy) = stock.z_grid.cell_to_world(row, col);
    let offset = ((cx - x).powi(2) + (cy - y).powi(2)).sqrt();
    // Beyond the cutter's own extent there is no surface to compare
    // against; the caller is asking about a ray the tool does not cover.
    let profile = cutter.height_at_radius(offset)?;
    Some(top as f64 - (z + profile))
}

/// Is EVERY point the tool passes through on this move in air?
///
/// Sampled along the swept path at the stock grid's own cell size, NOT at
/// the two endpoints.
///
/// Endpoint-only classification was a real defect: a cut whose ends are
/// both over cleared ground but whose middle ploughs through standing
/// material read as "air", and the filter then deleted it or bridged over
/// it — leaving an island and flying the tool across the gap. The bias is
/// one-directional (always toward leaving material) and it scales with
/// fragment count, so it fell hardest on exactly the fragmented rest passes
/// this repo was trying to measure (`planning/v3_workplan.md` defect P2).
/// The old doc comment claimed "moves that partially contact material are
/// preserved", which is the invariant this restores.
///
/// Arcs are walked with [`crate::geometry::arc_util::linearize_arc_into`] at the same
/// resolution the dexel simulator itself uses, so classification and
/// stamping agree about where the tool went. The previous code sampled the
/// arc's CENTRE — a point the tool never visits.
///
/// Cost: sampling is at grid resolution, so a finishing move shorter than
/// one cell costs the same two lookups it always did; only long roughing
/// moves sample more, and those are the ones that can hide an island.
///
/// Each sample is judged for the whole CUTTER, not its centerline — see
/// [`sample_is_air_for_tool`].
pub(super) fn swept_path_is_all_air(
    stock: &TriDexelStock,
    cutter: &dyn crate::tool::MillingCutter,
    prev: P3,
    m: &Move,
    tolerance: f64,
    arc_buf: &mut Vec<P3>,
) -> bool {
    let step = stock.z_grid.cell_size.max(1.0e-6);
    let air_at = |p: &P3| sample_is_air_for_tool(stock, cutter, p.x, p.y, p.z, tolerance);

    match m.move_type {
        MoveType::ArcCW { i, j, .. } | MoveType::ArcCCW { i, j, .. } => {
            let clockwise = matches!(m.move_type, MoveType::ArcCW { .. });
            crate::geometry::arc_util::linearize_arc_into(
                arc_buf, prev, m.target, i, j, clockwise, step,
            );
            // `linearize_arc_into` yields the endpoints too, so this covers
            // the whole move.
            arc_buf.iter().all(air_at)
        }
        _ => {
            if !air_at(&prev) || !air_at(&m.target) {
                return false;
            }
            let (dx, dy, dz) = (
                m.target.x - prev.x,
                m.target.y - prev.y,
                m.target.z - prev.z,
            );
            let len = (dx * dx + dy * dy + dz * dz).sqrt();
            let steps = (len / step).ceil() as usize;
            // 0 or 1 steps: the endpoints already are the whole segment.
            (1..steps).all(|k| {
                let t = k as f64 / steps as f64;
                air_at(&P3::new(prev.x + dx * t, prev.y + dy * t, prev.z + dz * t))
            })
        }
    }
}
