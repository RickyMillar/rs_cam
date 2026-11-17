//! The chord lift: raise every chord of a fed path until its interior holds
//! the model plus the stock to leave (G-DRAPEGAP).
//!
//! Moved out of `adaptive3d/path.rs` (feeds contract P40) so that the cuts
//! and links of a 3D Rough and the ramp and helix entries of every door
//! (`dressup::entry_descent`) use one implementation. The algorithm and its
//! constants are unchanged.

use crate::geo::P3;

/// G-DRAPEGAP (2026-11-02): lift every chord of an emitted path so that its
/// interior holds the leave against the radius-aware surface, to within half
/// the chord tolerance.
///
/// The drape of a 3D Rough (`adaptive3d::path::drape_path_to_leave`) samples at the tool radius and argues that no peak
/// can hide between two samples. The argument is false for a point beside the
/// path: the tool between two samples reaches one radius sideways, and a point
/// at that distance from the midpoint is `sqrt(R^2 + (s/2)^2) > R` from both
/// samples. The model gouge guard (`compute::model_gouge`, `Allowance`)
/// samples every emitted move at a quarter of the diameter and finds the dip
/// (measured: 0.026 to 0.049 mm past its limit on a 358 x 443 relief, an 18 mm
/// and a 6 mm adaptive rough). This pass samples each chord at a quarter of
/// the radius with the full tool, and where the chord is more than half the
/// chord tolerance under the rest height plus the leave it inserts a point at
/// that rest height and tests the two halves again. It only ever raises the
/// path. Every point is first lifted to the rest height (a corner blend can
/// move a point off its draped position). `probe` is the guard's reduced tool
/// (`Adaptive3dGeometry::gouge_probe`): with the full tool a cut tangent to a
/// wall reads the wall top and would be lifted over the wall.
///
/// P40: an entry ships its chords through this too
/// (`dressup::entry_descent::hold_entry_chords`). A clip that reads the floor
/// at samples ships a chord between a low sample and a lifted sample; over a
/// wall of the model that chord went through the top edge of the wall.
pub(crate) fn lift_chords_to_leave(
    path: &[P3],
    mesh: &crate::mesh::TriangleMesh,
    index: &crate::mesh::SpatialIndex,
    probe: &dyn crate::tool::MillingCutter,
    stock_to_leave: f64,
    tolerance: f64,
) -> Vec<P3> {
    /// The deepest bisection of one chord. Each level halves the chord, so
    /// 16 levels reach far below the sample step on any path the planner makes.
    const MAX_DEPTH: u32 = 16;
    if path.len() < 2 {
        return path.to_vec();
    }
    let step = (probe.radius() / 4.0).max(0.05);
    let margin = 0.5 * tolerance.max(0.0);
    let rest = |x: f64, y: f64| -> Option<f64> {
        let cl = crate::surface::dropcutter::point_drop_cutter(x, y, mesh, index, probe);
        (cl.contacted && cl.z.is_finite()).then_some(cl.z + stock_to_leave)
    };
    let lift = |p: &P3| -> P3 { rest(p.x, p.y).map_or(*p, |r| P3::new(p.x, p.y, p.z.max(r))) };
    // Bisect one chord: insert the worst sample at its rest height until no
    // sample is more than `margin` under it. An explicit stack keeps the
    // order (a before b) without recursion.
    let mut out: Vec<P3> = Vec::with_capacity(path.len() * 2);
    let lifted: Vec<P3> = path.iter().map(lift).collect();
    if let Some(first) = lifted.first() {
        out.push(*first);
    }
    // The rest height between two samples can jump (a wall, the edge of a
    // shelf). Where two neighbours differ by more than the margin, sample in
    // between until they are closer than half the chord tolerance: the guard's
    // probe is a chord tolerance smaller in radius, so a lift placed that close
    // to the jump holds its limit.
    let min_gap = (0.5 * tolerance).max(0.01);
    let samples_of = |p: &P3, q: &P3| -> Vec<(P3, Option<f64>)> {
        let (dx, dy, dz) = (q.x - p.x, q.y - p.y, q.z - p.z);
        let len = (dx * dx + dy * dy).sqrt();
        let at = |t: f64| P3::new(p.x + t * dx, p.y + t * dy, p.z + t * dz);
        // At least the midpoint: a chord shorter than the step can still
        // cross a rise of the rest height.
        let n = (len / step).ceil().max(2.0) as usize;
        let mut coarse: Vec<(f64, Option<f64>)> = (0..=n)
            .map(|i| {
                let t = i as f64 / n as f64;
                let c = at(t);
                (t, rest(c.x, c.y))
            })
            .collect();
        let jumps = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (Some(a), Some(b)) => (a - b).abs() > margin,
            (None, None) => false,
            _ => true,
        };
        let mut fine: Vec<(f64, Option<f64>)> = Vec::with_capacity(coarse.len());
        for w in coarse.windows(2) {
            let [(t0, r0), (t1, r1)] = w else { continue };
            fine.push((*t0, *r0));
            let mut stack = vec![(*t0, *r0, *t1, *r1)];
            let mut found: Vec<(f64, Option<f64>)> = Vec::new();
            while let Some((ta, ra, tb, rb)) = stack.pop() {
                if !jumps(ra, rb) || (tb - ta) * len <= min_gap {
                    continue;
                }
                let tm = 0.5 * (ta + tb);
                let cm = at(tm);
                let rm = rest(cm.x, cm.y);
                found.push((tm, rm));
                stack.push((tm, rm, tb, rb));
                stack.push((ta, ra, tm, rm));
            }
            found.sort_by(|a, b| a.0.total_cmp(&b.0));
            fine.extend(found);
        }
        if let Some(last) = coarse.pop() {
            fine.push(last);
        }
        fine.into_iter().map(|(t, r)| (at(t), r)).collect()
    };
    for w in lifted.windows(2) {
        let [a, b] = w else { continue };
        let mut stack: Vec<(P3, P3, u32)> = vec![(*a, *b, 0)];
        while let Some((p, q, depth)) = stack.pop() {
            let mut worst: Option<(f64, P3)> = None;
            for (c, r) in samples_of(&p, &q) {
                if let Some(r) = r {
                    let deficit = r - margin - c.z;
                    if deficit > 0.0 && worst.is_none_or(|(d, _)| deficit > d) {
                        worst = Some((deficit, P3::new(c.x, c.y, r)));
                    }
                }
            }
            match worst {
                Some((_, m)) if depth < MAX_DEPTH => {
                    // q is pushed first so p..m is taken first.
                    stack.push((m, q, depth + 1));
                    stack.push((p, m, depth + 1));
                }
                _ => out.push(q),
            }
        }
    }
    out
}
