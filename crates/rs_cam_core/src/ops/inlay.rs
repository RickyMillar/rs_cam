//! Inlay operations: the V walls of the female pocket and the male plug.
//!
//! An inlay has two parts that are cut on two boards:
//!
//! - **The pocket (female)**, cut into the workpiece by the `Inlay` op. The
//!   op cuts only the V walls with the V-bit. A normal Pocket with a flat end
//!   mill clears the floor (ruling R2,
//!   `planning/op_architecture_2026-10-14/PLAN.md`).
//! - **The plug (male)**, cut into a second board by the `InlayPlug` op, in
//!   its own project. The plug is the mirrored design. The operator turns
//!   the plug board face down and glues the plug into the pocket.
//!
//! # The geometry (one V-bit, both parts)
//!
//! Let the V-bit have the included angle `2α`. The flank of the bit makes
//! the angle `α` with the vertical. A horizontal step `h` along the flank
//! therefore comes with a vertical step `h / tan α`, and a vertical step `v`
//! comes with a horizontal step `v · tan α`.
//!
//! **The pocket.** Let `d` be the distance of a point inside the design
//! from the design outline. The bit tip at `d` and at the depth `d / tan α`
//! puts the flank exactly through the outline at the stock top. So the
//! pocket wall at the depth `t` lies `t · tan α` inside the outline. The
//! wall stops at the pocket depth `D`. The floor is the design inset by
//! `D · tan α`.
//!
//! **The plug.** Let `e` be the signed distance of a point from the
//! mirrored outline, positive outside the design. Let `u` be the depth below
//! the plug board top. The plug op cuts the surface
//! `u(e) = S + e / tan α`, clamped to `0 ≤ u ≤ S + F`, where `S` is the
//! start depth and `F` is the height that the plug stands proud of the
//! workpiece. The plug face (`u = 0`) is the mirrored design inset by
//! `S · tan α`.
//!
//! **The fit.** Turn the plug face down and mirror it back. The plug face
//! sits at the pocket depth `t = S`, and a plug point at the depth `u` sits
//! at `t = S − u`. A plug wall point at `e` then has `t = −e / tan α`, so it
//! lies `t · tan α` inside the outline at the depth `t`. That is the pocket
//! wall: the two walls are the same surface and are parallel at every V
//! angle. The plug face is at `S` and the pocket floor is at `D`, so the
//! vertical gap is `D − S`. The plug op therefore sets
//! `S = D − glue_gap` ([`plug_start_depth`]). The glue gap is a vertical
//! length and never meets `tan α`.
//!
//! Before ruling R2 the male pass shifted its depth by
//! `glue_gap / tan² α`. That is `glue_gap` only for a 90° bit (`tan 45° = 1`)
//! and three times `glue_gap` for a 60° bit.

use crate::geo::{P2, P3};
use crate::geometry::edge_distance::EdgeDistanceField;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::ops::depth::{LadderMode, z_ladder};
use crate::ops::vcarve::SCAN_CHUNK;
use crate::ops::zigzag::zigzag_lines;
use crate::polygon::Polygon2;
use crate::toolpath::{MoveIntent, Toolpath};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Parameters of the female pocket walls (`Inlay`).
pub struct InlayPocketParams {
    /// V-bit half-angle `α` in radians.
    pub half_angle: f64,
    /// Pocket depth `D` below the stock top (mm).
    pub pocket_depth: f64,
    /// Width (mm) of the band beyond the wall foot that the V-bit also cuts
    /// at the pocket depth. The band removes the strip and the corner
    /// material that the clipped floor Pocket leaves at the wall foot.
    pub floor_overlap: f64,
    /// Largest ridge (mm) that the V-bit may leave on the floor band. It
    /// sets the ring pitch ([`floor_ring_pitch`]).
    pub floor_ridge: f64,
    /// Width (mm) of the flat on the V-bit tip; 0 for a sharp bit.
    pub tip_flat: f64,
    /// Scan line spacing (mm) over the wall band.
    pub stepover: f64,
    /// Feed rate (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid moves (mm).
    pub safe_z: f64,
    /// Sample spacing along a scan line (mm).
    pub tolerance: f64,
    /// Stock-top Z in the emission frame (F-028 / S.2).
    pub top_z: f64,
}

/// Parameters of the male plug (`InlayPlug`).
pub struct InlayPlugParams {
    /// V-bit half-angle `α` in radians. Use the bit of the pocket.
    pub half_angle: f64,
    /// Depth `D` of the female pocket that the plug fits (mm).
    pub pocket_depth: f64,
    /// Vertical gap (mm) between the plug face and the pocket floor.
    pub glue_gap: f64,
    /// Height `F` (mm) that the plug stands proud of the workpiece top after
    /// it is seated. The background around the plug is cut to `S + F`.
    pub proud_height: f64,
    /// Width (mm) of the flat background ring beyond the wall foot.
    pub outer_margin: f64,
    /// Maximum depth of one pass (mm).
    pub depth_per_pass: f64,
    /// Scan line spacing (mm).
    pub stepover: f64,
    /// Feed rate (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid moves (mm).
    pub safe_z: f64,
    /// Sample spacing along a scan line (mm).
    pub tolerance: f64,
    /// Stock-top Z of the plug board in the emission frame.
    pub top_z: f64,
}

/// The plug start depth `S = D − glue_gap` (see the module doc).
#[must_use]
pub fn plug_start_depth(pocket_depth: f64, glue_gap: f64) -> f64 {
    pocket_depth - glue_gap
}

/// The horizontal inset (mm) of the pocket floor from the design outline:
/// `pocket_depth · tan(half_angle)`. The floor Pocket uses this number as a
/// negative boundary offset.
#[must_use]
pub fn floor_inset(pocket_depth: f64, half_angle: f64) -> f64 {
    pocket_depth * half_angle.tan()
}

/// The pitch (mm) of the floor rings for a ridge of at most `ridge` mm.
///
/// Two passes of a sharp V-bit at the same depth, `s` apart, leave a ridge
/// between them. Each flank rises `h` over a horizontal run `h · tan α`, and
/// the two flanks meet halfway, so `s = 2 h tan α`, that is
/// `h = s / (2 tan α)`. A bit with a tip flat of width `w` cuts the flat
/// first: `h = (s − w) / (2 tan α)` for `s > w`, and no ridge for `s <= w`.
/// So `s = 2 h tan α + w`. The pitch is never below 0.01 mm.
#[must_use]
pub fn floor_ring_pitch(ridge: f64, half_angle: f64, tip_flat: f64) -> f64 {
    (2.0 * ridge.max(0.0) * half_angle.tan() + tip_flat.max(0.0)).max(0.01)
}

/// Mirror a design about the vertical centre line of its whole bounding box
/// in X: `x' = x_min + x_max − x`. The plug board turns face down about
/// this line, so the mirrored plug fits the pocket. The mirror reverses the
/// winding; this function reverses each ring again to keep it.
#[must_use]
pub fn mirror_design(polygons: &[Polygon2]) -> Vec<Polygon2> {
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    for poly in polygons {
        for p in &poly.exterior {
            x_min = x_min.min(p.x);
            x_max = x_max.max(p.x);
        }
    }
    let sum = x_min + x_max;
    let flip =
        |ring: &[P2]| -> Vec<P2> { ring.iter().rev().map(|p| P2::new(sum - p.x, p.y)).collect() };
    polygons
        .iter()
        .map(|poly| {
            Polygon2::with_holes(
                flip(&poly.exterior),
                poly.holes.iter().map(|h| flip(h)).collect(),
            )
        })
        .collect()
}

/// One sampled scan line: runs of points that the bit cuts. A point that
/// the bit does not cut splits the line.
type Runs = Vec<Vec<P3>>;

/// Sample a scan line. `depth_at(p)` gives the cut depth below the stock
/// top, or `None` where the bit does not cut.
fn sample_line(
    line: &[P2; 2],
    sample_step: f64,
    depth_at: &(dyn Fn(P2) -> Option<f64> + Sync),
    top_z: f64,
) -> Runs {
    let (a, b) = (line[0], line[1]);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-10 {
        return Vec::new();
    }
    let n = crate::util::steps::steps(len, sample_step).unwrap_or(0);
    let mut runs: Runs = Vec::new();
    let mut current: Vec<P3> = Vec::new();
    for i in 0..=n {
        let t = i as f64 / n.max(1) as f64;
        let p = P2::new(a.x + t * dx, a.y + t * dy);
        match depth_at(p) {
            Some(depth) => current.push(P3::new(p.x, p.y, top_z - depth)),
            None => {
                if !current.is_empty() {
                    runs.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs.into_iter().map(drop_collinear).collect()
}

/// Drop each interior sample whose Z is the mean of its two neighbours.
///
/// The samples of a run are equally spaced on one straight scan line, so a
/// zero second difference in Z means the three points are collinear in 3D.
/// A chain of such points is one straight segment, so the run keeps its
/// exact shape (to 1e-9 mm) with fewer moves: a flat floor and a wall along
/// a straight edge each become one segment.
fn drop_collinear(run: Vec<P3>) -> Vec<P3> {
    if run.len() < 3 {
        return run;
    }
    let mut out = Vec::with_capacity(run.len());
    for (i, p) in run.iter().enumerate() {
        let keep = match (i.checked_sub(1).and_then(|j| run.get(j)), run.get(i + 1)) {
            (Some(a), Some(b)) => (a.z - 2.0 * p.z + b.z).abs() > 1e-9,
            _ => true,
        };
        if keep {
            out.push(*p);
        }
    }
    out
}

/// Sample a batch of lines, in parallel when the `parallel` feature is on.
/// Order is kept, so the output is the same either way.
fn sample_batch(
    batch: &[[P2; 2]],
    sample_step: f64,
    depth_at: &(dyn Fn(P2) -> Option<f64> + Sync),
    top_z: f64,
) -> Vec<Runs> {
    #[cfg(feature = "parallel")]
    {
        batch
            .par_iter()
            .map(|line| sample_line(line, sample_step, depth_at, top_z))
            .collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        batch
            .iter()
            .map(|line| sample_line(line, sample_step, depth_at, top_z))
            .collect()
    }
}

/// Emit every run of every sampled line. Polls `cancel` once per
/// [`SCAN_CHUNK`] lines, the V-carve cadence (G9).
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
fn emit_scan(
    tp: &mut Toolpath,
    lines: &[[P2; 2]],
    sample_step: f64,
    depth_at: &(dyn Fn(P2) -> Option<f64> + Sync),
    top_z: f64,
    safe_z: f64,
    feed_rate: f64,
    plunge_rate: f64,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    for batch in lines.chunks(SCAN_CHUNK) {
        check_cancel(cancel)?;
        for runs in sample_batch(batch, sample_step, depth_at, top_z) {
            for run in runs {
                tp.emit_path_segment_with_intent(
                    &run,
                    safe_z,
                    feed_rate,
                    plunge_rate,
                    MoveIntent::FinishingCut,
                );
            }
        }
    }
    Ok(())
}

/// The female pocket walls of one design polygon.
///
/// Two parts, both with the V-bit:
///
/// 1. The wall band: scan lines at `stepover` over the design. The bit
///    cuts each point whose distance `d` from the outline is at most
///    `D · tan α`, at the depth `d / tan α`. This part also carves a
///    feature too narrow to reach the pocket depth.
/// 2. The floor band: closed rings at the pocket depth, on the outline
///    inset by `D · tan α + k · p` for `k = 0, 1, …` up to
///    `D · tan α + floor_overlap`, where `p` is [`floor_ring_pitch`]. The
///    first ring runs along the wall foot, so its flank cuts the whole wall
///    in one continuous pass. The rings leave floor ridges of at most
///    `floor_ridge`.
///
/// The op cuts nothing outside the design and no floor beyond the band.
///
/// Cancellable: checks `cancel` first, then once per [`SCAN_CHUNK`] lines
/// and once per ring.
pub fn inlay_pocket_toolpath_with_cancel(
    polygon: &Polygon2,
    params: &InlayPocketParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    check_cancel(cancel)?;
    let mut tp = Toolpath::new();
    let tan_half = params.half_angle.tan();
    if tan_half < 1e-10 || params.pocket_depth <= 0.0 {
        return Ok(tp);
    }
    let foot = floor_inset(params.pocket_depth, params.half_angle);
    let scan_inset = params.tolerance.min(0.05);
    let lines = zigzag_lines(polygon, scan_inset, params.stepover, 0.0);
    let field = EdgeDistanceField::from_polygon(polygon);
    let depth_at = move |p: P2| -> Option<f64> {
        let d = field.distance(&p);
        (d <= foot).then_some(d / tan_half)
    };
    emit_scan(
        &mut tp,
        &lines,
        params.tolerance.max(0.05),
        &depth_at,
        params.top_z,
        params.safe_z,
        params.feed_rate,
        params.plunge_rate,
        cancel,
    )?;

    let overlap = params.floor_overlap.max(0.0);
    let pitch = floor_ring_pitch(params.floor_ridge, params.half_angle, params.tip_flat);
    let rings = crate::util::steps::steps(overlap, pitch).unwrap_or(0);
    let z = params.top_z - params.pocket_depth;
    // The arc-carrying offset, flattened once to the op tolerance: a plain
    // `offset_polygon` turns the join arc at a reflex corner into its
    // chord, and a full-depth V on that chord gouges the wall (measured:
    // 0.879 mm on the L of G-INLAYSPLIT).
    let shape = crate::polygon::OffsetRingSet::from_polygon(polygon);
    let flatten = crate::polygon::FlattenPolicy::from_chord_tolerance(params.tolerance);
    for k in 0..=rings {
        check_cancel(cancel)?;
        let inset = foot + (k as f64 * pitch).min(overlap);
        // A POSITIVE offset distance is inward.
        for piece in shape.offset(inset).to_polygons(flatten) {
            for ring in std::iter::once(&piece.exterior).chain(piece.holes.iter()) {
                let Some(first) = ring.first() else {
                    continue;
                };
                let mut pts: Vec<P3> = ring.iter().map(|p| P3::new(p.x, p.y, z)).collect();
                pts.push(P3::new(first.x, first.y, z));
                tp.emit_path_segment_with_intent(
                    &pts,
                    params.safe_z,
                    params.feed_rate,
                    params.plunge_rate,
                    MoveIntent::FinishingCut,
                );
            }
        }
    }
    Ok(tp)
}

/// The male plug of a whole design (all its polygons at once).
///
/// The op mirrors the design ([`mirror_design`]) and cuts the surface
/// `u(e) = clamp(S + e / tan α, 0, S + F)` (see the module doc) over the
/// design bounding box grown by `F · tan α + outer_margin`. The V-bit cuts
/// the background too, in passes of at most `depth_per_pass`. The board
/// outside that rectangle stays at the plug face height; the operator saws
/// the plug free inside the flat background ring.
///
/// All polygons share one distance field, so one polygon is never cut as
/// the background of another.
///
/// Cancellable: checks `cancel` first, then once per [`SCAN_CHUNK`] lines.
pub fn inlay_plug_toolpath_with_cancel(
    polygons: &[Polygon2],
    params: &InlayPlugParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    check_cancel(cancel)?;
    let mut tp = Toolpath::new();
    let tan_half = params.half_angle.tan();
    let start = plug_start_depth(params.pocket_depth, params.glue_gap);
    let total = start + params.proud_height.max(0.0);
    if tan_half < 1e-10 || polygons.is_empty() || start <= 0.0 || total <= 0.0 {
        return Ok(tp);
    }
    let mirrored = mirror_design(polygons);

    // One field over every ring of every polygon.
    let mut rings: Vec<Vec<P2>> = Vec::new();
    let (mut x_min, mut y_min) = (f64::INFINITY, f64::INFINITY);
    let (mut x_max, mut y_max) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for poly in &mirrored {
        for p in &poly.exterior {
            x_min = x_min.min(p.x);
            y_min = y_min.min(p.y);
            x_max = x_max.max(p.x);
            y_max = y_max.max(p.y);
        }
        rings.push(poly.exterior.clone());
        rings.extend(poly.holes.iter().cloned());
    }
    let Some((first, rest)) = rings.split_first() else {
        return Ok(tp);
    };
    let field = EdgeDistanceField::from_rings(first, rest);

    let margin = params.proud_height.max(0.0) * tan_half + params.outer_margin.max(0.0);
    let region = Polygon2::rectangle(
        x_min - margin,
        y_min - margin,
        x_max + margin,
        y_max + margin,
    );
    let lines = zigzag_lines(&region, 0.0, params.stepover, 0.0);

    let plateau_inset = start * tan_half;
    let surface = |p: P2| -> Option<f64> {
        let dist = field.distance(&p);
        let inside = mirrored.iter().any(|poly| poly.contains_point(&p));
        let e = if inside { -dist } else { dist };
        if e <= -plateau_inset {
            return None; // the plug face: never cut
        }
        Some((start + e / tan_half).clamp(0.0, total))
    };

    let step = if params.depth_per_pass > 0.0 {
        params.depth_per_pass
    } else {
        total
    };
    // A point is cut on the first level that goes below its surface depth
    // and on each later level until its surface is reached. A point whose
    // surface an earlier level reached is not cut again.
    let mut prev_depth = 0.0_f64;
    for level in z_ladder(0.0, -total, step, LadderMode::EvenRedistributed) {
        let level_depth = -level;
        let above = prev_depth;
        let depth_at = |p: P2| -> Option<f64> {
            surface(p)
                .filter(|u| *u > above)
                .map(|u| u.min(level_depth))
        };
        emit_scan(
            &mut tp,
            &lines,
            params.tolerance.max(0.05),
            &depth_at,
            params.top_z,
            params.safe_z,
            params.feed_rate,
            params.plunge_rate,
            cancel,
        )?;
        prev_depth = level_depth;
    }
    Ok(tp)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;
    use std::f64::consts::FRAC_PI_4;

    /// The female pocket through the kernel the Inlay adapter calls.
    fn pocket_cut(polygon: &Polygon2, params: &InlayPocketParams) -> Toolpath {
        crate::interrupt::run_uncancellable(|cancel| {
            inlay_pocket_toolpath_with_cancel(polygon, params, cancel)
        })
    }

    /// The male plug through the kernel the Inlay Plug adapter calls.
    fn plug_cut(polygons: &[Polygon2], params: &InlayPlugParams) -> Toolpath {
        crate::interrupt::run_uncancellable(|cancel| {
            inlay_plug_toolpath_with_cancel(polygons, params, cancel)
        })
    }

    fn square(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    fn pocket_params() -> InlayPocketParams {
        InlayPocketParams {
            half_angle: FRAC_PI_4,
            pocket_depth: 3.0,
            floor_overlap: 1.0,
            floor_ridge: 0.05,
            tip_flat: 0.0,
            stepover: 0.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            tolerance: 0.1,
            top_z: 0.0,
        }
    }

    fn plug_params() -> InlayPlugParams {
        InlayPlugParams {
            half_angle: FRAC_PI_4,
            pocket_depth: 3.0,
            glue_gap: 0.2,
            proud_height: 2.0,
            outer_margin: 2.0,
            depth_per_pass: 1.5,
            stepover: 0.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            tolerance: 0.1,
            top_z: 0.0,
        }
    }

    fn cuts(tp: &Toolpath) -> Vec<P3> {
        tp.moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }) && m.target.z < 9.0)
            .map(|m| m.target)
            .collect()
    }

    #[test]
    fn the_pocket_walls_stay_in_the_band_and_reach_the_pocket_depth() {
        let sq = square(40.0);
        let p = pocket_params();
        let tp = pocket_cut(&sq, &p);
        let pts = cuts(&tp);
        assert!(!pts.is_empty());
        let band = floor_inset(p.pocket_depth, p.half_angle) + p.floor_overlap;
        for q in &pts {
            let d = sq.distance_to_boundary(&P2::new(q.x, q.y));
            assert!(
                d <= band + 1e-6,
                "a cut at d = {d} lies beyond the band {band}"
            );
            assert!(q.z >= -p.pocket_depth - 1e-9);
        }
        assert!(pts.iter().any(|q| (q.z + p.pocket_depth).abs() < 1e-9));
        // The centre of the floor stays uncut.
        assert!(!pts.iter().any(|q| q.x.abs() < 5.0 && q.y.abs() < 5.0));
    }

    #[test]
    fn the_pocket_cuts_relative_to_the_stock_top() {
        let p = InlayPocketParams {
            top_z: 5.0,
            ..pocket_params()
        };
        let pts = cuts(&pocket_cut(&square(20.0), &p));
        assert!(pts.iter().all(|q| q.z <= 5.0 + 1e-9 && q.z >= 2.0 - 1e-9));
    }

    #[test]
    fn the_plug_never_cuts_its_face_and_stops_at_its_full_depth() {
        let p = plug_params();
        let sq = square(20.0);
        let pts = cuts(&plug_cut(std::slice::from_ref(&sq), &p));
        assert!(!pts.is_empty());
        let start = plug_start_depth(p.pocket_depth, p.glue_gap);
        let total = start + p.proud_height;
        let inset = start * p.half_angle.tan();
        for q in &pts {
            let xy = P2::new(q.x, q.y);
            let inside = sq.contains_point(&xy);
            let d = sq.distance_to_boundary(&xy);
            assert!(
                !(inside && d > inset + 1e-6),
                "a cut on the plug face at {xy:?}"
            );
            assert!(q.z >= -total - 1e-9);
        }
        assert!(pts.iter().any(|q| (q.z + total).abs() < 1e-9));
    }

    #[test]
    fn the_mirror_keeps_the_winding_and_flips_x_about_the_box_centre() {
        let l = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(30.0, 0.0),
            P2::new(30.0, 10.0),
            P2::new(10.0, 10.0),
            P2::new(10.0, 30.0),
            P2::new(0.0, 30.0),
        ]);
        let m = mirror_design(std::slice::from_ref(&l)).pop().unwrap();
        assert!(m.signed_area() > 0.0);
        assert!((m.signed_area() - l.signed_area()).abs() < 1e-9);
        // The foot (x in 10..30, y < 10) moves to x in 0..20.
        assert!(m.contains_point(&P2::new(5.0, 5.0)));
        assert!(m.contains_point(&P2::new(25.0, 20.0)));
        assert!(!m.contains_point(&P2::new(5.0, 20.0)));
    }

    #[test]
    fn the_start_depth_is_the_pocket_depth_less_the_glue_gap() {
        assert!((plug_start_depth(3.0, 0.2) - 2.8).abs() < 1e-12);
    }
}
