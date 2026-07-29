//! Gouge-safe, drop-cutter-sampled surface links between two cut points.
//!
//! Promoted out of `crate::pencil` (where it joins consecutive pencil passes
//! without a retract-and-replunge) so the P2 unified-finish planner's global
//! router can reuse it as one of the two link-cost candidates (surface link
//! vs. retract link) between regions — see
//! `planning/unified_finish_planner_design.md` step 4.

use crate::dropcutter::point_drop_cutter;
use crate::geo::P3;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;

/// Build a gouge-safe, surface-following link between two cut points whose XY gap
/// is within `hookup_distance`, so consecutive passes join without retracting to
/// safe Z and re-plunging. Samples the connecting segment and drop-cutters each
/// interior point — the same gouge-free lift the cut path uses — so the tool rides
/// the surface across the gap instead of lifting clear. Endpoints are excluded
/// (the caller is already at `from` and feeds to `to` itself). Returns `None` if
/// the tool loses surface contact anywhere along the link (over a hole / off the
/// mesh) — the caller then falls back to a clean retract-and-replunge.
pub fn build_surface_link(
    from: P3,
    to: P3,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stock_to_leave: f64,
    spacing: f64,
) -> Option<Vec<P3>> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1e-6 {
        return Some(Vec::new());
    }
    let n = (dist / spacing.max(1e-3)).ceil().max(1.0) as usize;
    let mut pts = Vec::new();
    for k in 1..n {
        let t = k as f64 / n as f64;
        let x = from.x + dx * t;
        let y = from.y + dy * t;
        let cl = point_drop_cutter(x, y, mesh, index, cutter);
        if !cl.contacted {
            return None; // lost contact → not safe to link at surface, retract instead
        }
        pts.push(P3::new(x, y, cl.z + stock_to_leave));
    }
    Some(pts)
}

// ── Fragment relinking ───────────────────────────────────────────────────

/// Inputs for [`relink_fragments`].
#[derive(Debug, Clone, Copy)]
pub struct RelinkParams<'a> {
    /// Candidate CAP on the XY gap a surface link may span. `0.0` disables
    /// linking entirely (the pass then only reorders, if asked).
    pub hookup_distance: f64,
    pub stock_to_leave: f64,
    /// Sample spacing along a candidate link (drop-cutter probe density).
    pub sampling: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
    /// When `Some`, a candidate link is additionally costed against the
    /// retract it would replace with the F-034 integrator and only kept
    /// when it is actually faster — the same decision
    /// [`crate::pencil::emit_paths`] makes. `None` keeps any gouge-safe
    /// link within `hookup_distance`.
    pub link_kinematics: Option<&'a crate::machine_kinematics::LinkKinematics>,
    /// Visit fragments nearest-first (from the previous fragment's exit)
    /// instead of in emitted order. Forward-only: a fragment is never
    /// reversed, so its cut direction — and therefore climb/conventional —
    /// is preserved.
    pub reorder: bool,
    /// Territory the link may cross. Every sampled link point must lie
    /// inside it or the candidate is refused.
    ///
    /// NOT optional in spirit: a surface link is a CUTTING feed, so one
    /// that leaves the region it belongs to machines ground the op was
    /// confined away from — the selective-finishing gouge class the
    /// boundary exists to prevent. `unified_finish::choose_link` has
    /// enforced exactly this for region-to-region links since P2.d; the
    /// first version of THIS pass omitted it and the wanaka ×2 COLUMNS
    /// gate caught it (shallow `<-.5` over-cut columns 1 218 → 4 068,
    /// worst column −3.0 → −3.9 mm). On dendritic rest islands a straight
    /// line between two fragments of the SAME region leaves that region
    /// constantly, so intra-region linking needs the check just as much as
    /// inter-region linking does.
    ///
    /// `None` disables the check — only correct when the caller knows the
    /// fragments span no excluded territory.
    pub boundary: Option<&'a crate::region_set::RegionSet<'a>>,
}

/// What [`relink_fragments`] did.
#[derive(Debug, Clone, Default)]
pub struct RelinkReport {
    pub fragments: usize,
    /// Junctions joined by a surface-following link (no retract).
    pub surface_links: usize,
    /// Junctions that fell back to retract → traverse → plunge.
    pub retract_links: usize,
    /// Junctions rejected because the gap exceeded `hookup_distance`.
    pub too_far: usize,
    /// Junctions where the tool would have lost surface contact.
    pub off_surface: usize,
    /// Junctions where the link was gouge-safe but slower than retracting.
    pub slower_than_retract: usize,
    /// Junctions where the link would have left `RelinkParams::boundary`.
    pub outside_boundary: usize,
    /// `old_move_index -> new_move_index`. A dropped move (a retract or
    /// plunge a surface link replaced) maps to the first surviving move
    /// after it, so annotation indices remap without going backwards.
    pub move_remap: Vec<usize>,
}

/// Rewrite a toolpath's inter-fragment junctions, replacing
/// retract → traverse → plunge with a gouge-checked surface-following link
/// wherever one is safe and (optionally) faster.
///
/// WHY THIS EXISTS, measured: on wanaka ×2 the unified rest-clearer spends
/// 17 077 s in rapids across 12 780 fragments — roughly 1.3 s per junction,
/// and almost all of it is the two ~30 mm Z legs, not the XY hop between
/// them. Reordering shortens the hop (measured 8.6 mm → 2.8 mm) but cannot
/// remove a single Z leg. Only keeping the tool down can. See
/// `planning/unified_v3_design.md` §9.
///
/// A "fragment" is a maximal run of non-`Rapid` moves — the same split
/// [`crate::tsp::optimize_rapid_order`] uses, so the two passes agree on
/// what is atomic. Fragment interiors are copied VERBATIM; only the
/// junctions between them are rewritten, which is what makes the cut
/// geometry provably unchanged except at the (previously airborne)
/// junctions themselves.
///
/// Linking is refused whenever the previous fragment ends at or above
/// `safe_z` — that fragment retracted with a FEED rather than a rapid, so
/// its exit is not a point on the surface and a link from it would descend
/// diagonally through material.
#[allow(clippy::too_many_arguments)]
pub fn relink_fragments(
    toolpath: &crate::toolpath::Toolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RelinkParams<'_>,
) -> (crate::toolpath::Toolpath, RelinkReport) {
    use crate::toolpath::{MoveIntent, MoveType, Toolpath};

    // ── split into fragments, remembering each move's original index ────
    struct Fragment {
        moves: Vec<(usize, crate::toolpath::Move)>,
    }
    let mut frags: Vec<Fragment> = Vec::new();
    let mut cur: Vec<(usize, crate::toolpath::Move)> = Vec::new();
    for (i, mv) in toolpath.moves.iter().enumerate() {
        if matches!(mv.move_type, MoveType::Rapid) {
            if !cur.is_empty() {
                frags.push(Fragment {
                    moves: std::mem::take(&mut cur),
                });
            }
        } else {
            cur.push((i, mv.clone()));
        }
    }
    if !cur.is_empty() {
        frags.push(Fragment { moves: cur });
    }

    let mut report = RelinkReport {
        fragments: frags.len(),
        move_remap: vec![usize::MAX; toolpath.moves.len()],
        ..RelinkReport::default()
    };
    if frags.len() < 2 {
        return (toolpath.clone(), {
            report.move_remap = (0..toolpath.moves.len()).collect();
            report
        });
    }

    let entry_of = |f: &Fragment| -> P3 { f.moves.first().map_or(P3::origin(), |(_, m)| m.target) };
    let exit_of = |f: &Fragment| -> P3 { f.moves.last().map_or(P3::origin(), |(_, m)| m.target) };
    let xy_gap = |a: P3, b: P3| ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();

    // ── visiting order ──────────────────────────────────────────────────
    // SAFETY: `visited` is sized from `frags.len()` and every index used to
    // read it comes from `frags.iter().enumerate()`, so both stay in range.
    #[allow(clippy::indexing_slicing)]
    let order: Vec<usize> = if params.reorder {
        let n = frags.len();
        let mut visited = vec![false; n];
        let mut order = Vec::with_capacity(n);
        order.push(0);
        visited[0] = true;
        let mut here = frags.first().map_or(P3::origin(), exit_of);
        for _ in 1..n {
            let mut best = usize::MAX;
            let mut best_d = f64::INFINITY;
            for (j, f) in frags.iter().enumerate() {
                if visited[j] {
                    continue;
                }
                let d = xy_gap(here, entry_of(f));
                if d < best_d {
                    best_d = d;
                    best = j;
                }
            }
            let Some(next) = frags.get(best) else { break };
            visited[best] = true;
            here = exit_of(next);
            order.push(best);
        }
        order
    } else {
        (0..frags.len()).collect()
    };

    // ── re-emit ─────────────────────────────────────────────────────────
    let mut out = Toolpath::new();
    let mut prev_exit: Option<P3> = None;
    for &fi in &order {
        let Some(frag) = frags.get(fi) else { continue };
        let entry = entry_of(frag);

        let link = prev_exit.and_then(|from| {
            if params.hookup_distance <= 0.0 {
                return None;
            }
            // The previous fragment retracted with a feed, so its exit is
            // not a surface point — refuse (see the doc comment).
            if from.z >= params.safe_z - 1e-6 {
                report.off_surface += 1;
                return None;
            }
            let gap = xy_gap(from, entry);
            if gap <= 1e-6 {
                return Some(Vec::new());
            }
            if gap > params.hookup_distance {
                report.too_far += 1;
                return None;
            }
            let Some(pts) = build_surface_link(
                from,
                entry,
                mesh,
                index,
                cutter,
                params.stock_to_leave,
                params.sampling,
            ) else {
                report.off_surface += 1;
                return None;
            };
            // Mirrors `unified_finish::choose_link`: a surface link is a
            // CUTTING feed, so it must not leave the territory this op is
            // confined to. Endpoints are cut positions and trivially
            // inside; the interior samples carry the test.
            if let Some(boundary) = params.boundary
                && !pts
                    .iter()
                    .all(|p| boundary.contains(&crate::geo::P2::new(p.x, p.y)))
            {
                report.outside_boundary += 1;
                return None;
            }
            match params.link_kinematics {
                Some(lk) => {
                    let mut costed = pts.clone();
                    costed.push(entry);
                    let surface_t = crate::machine_kinematics::surface_link_time(
                        from,
                        &costed,
                        params.feed_rate,
                        &lk.kinematics,
                        lk.max_feed_mm_min,
                        lk.rapid_feed_mm_min,
                    );
                    let retract_t = crate::machine_kinematics::retract_link_time(
                        from,
                        entry,
                        params.safe_z,
                        None,
                        params.plunge_rate,
                        &lk.kinematics,
                        lk.max_feed_mm_min,
                        lk.rapid_feed_mm_min,
                    );
                    if surface_t <= retract_t {
                        Some(pts)
                    } else {
                        report.slower_than_retract += 1;
                        None
                    }
                }
                None => Some(pts),
            }
        });

        match (link, prev_exit) {
            (Some(pts), Some(_)) => {
                report.surface_links += 1;
                for p in &pts {
                    out.feed_to_with_intent(*p, params.feed_rate, MoveIntent::Linking);
                }
                // The fragment's own first move lands on `entry`; re-emit it
                // as a link so the run reads as one stay-down chain.
                out.feed_to_with_intent(entry, params.feed_rate, MoveIntent::Linking);
            }
            (_, Some(from)) => {
                report.retract_links += 1;
                out.rapid_to_with_intent(
                    P3::new(from.x, from.y, params.safe_z),
                    MoveIntent::Retract,
                );
                out.rapid_to_with_intent(
                    P3::new(entry.x, entry.y, params.safe_z),
                    MoveIntent::Linking,
                );
                out.feed_to_with_intent(entry, params.plunge_rate, MoveIntent::EntryPlunge);
            }
            (_, None) => {
                // First fragment: approach exactly as the input did.
                out.rapid_to_with_intent(
                    P3::new(entry.x, entry.y, params.safe_z),
                    MoveIntent::Linking,
                );
                out.feed_to_with_intent(entry, params.plunge_rate, MoveIntent::EntryPlunge);
            }
        }

        // The first move re-emitted above stands in for `frag.moves[0]`.
        if let Some((old, _)) = frag.moves.first()
            && let Some(slot) = report.move_remap.get_mut(*old)
        {
            *slot = out.moves.len().saturating_sub(1);
        }
        for (old, mv) in frag.moves.iter().skip(1) {
            let new = out.moves.len();
            out.moves.push(mv.clone());
            if let Some(slot) = report.move_remap.get_mut(*old) {
                *slot = new;
            }
        }
        prev_exit = Some(exit_of(frag));
    }

    if let Some(end) = prev_exit {
        out.rapid_to_with_intent(P3::new(end.x, end.y, params.safe_z), MoveIntent::Retract);
    }

    // Rapids in the input have no counterpart in the output (they were
    // regenerated or replaced). Point each at the next surviving move so a
    // remapped annotation never moves backwards past its own fragment.
    let last = out.moves.len().saturating_sub(1);
    let mut next_known = last;
    for slot in report.move_remap.iter_mut().rev() {
        if *slot == usize::MAX {
            *slot = next_known;
        } else {
            next_known = *slot;
        }
    }

    (out, report)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::tool::BallEndmill;

    /// A symmetric V-valley running along X: two inclined planes meeting at y=0.
    fn make_v_valley(len_x: f64, half_y: f64, slope: f64, nx: usize, ny: usize) -> TriangleMesh {
        let mut verts = Vec::new();
        let sx = len_x / nx as f64;
        let sy = 2.0 * half_y / ny as f64;
        for j in 0..=ny {
            for i in 0..=nx {
                let x = i as f64 * sx;
                let y = -half_y + j as f64 * sy;
                let z = -slope * half_y + slope * y.abs();
                verts.push(P3::new(x, y, z));
            }
        }
        let idx = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
        let mut tris = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                tris.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
                tris.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
            }
        }
        TriangleMesh::from_raw(verts, tris)
    }

    /// Build a toolpath of `n` short cut runs along the valley floor,
    /// each separated by the retract → traverse → plunge triple
    /// `relink_fragments` is meant to remove.
    fn fragmented_valley_path(
        n: usize,
        run_len: f64,
        gap: f64,
        safe_z: f64,
    ) -> crate::toolpath::Toolpath {
        use crate::toolpath::{MoveIntent, Toolpath};
        let mut tp = Toolpath::new();
        let mut x = 1.0;
        for i in 0..n {
            if i > 0 {
                tp.rapid_to_with_intent(P3::new(x - gap, 0.0, safe_z), MoveIntent::Retract);
                tp.rapid_to_with_intent(P3::new(x, 0.0, safe_z), MoveIntent::Linking);
            } else {
                tp.rapid_to_with_intent(P3::new(x, 0.0, safe_z), MoveIntent::Linking);
            }
            tp.feed_to_with_intent(P3::new(x, 0.0, -3.0), 100.0, MoveIntent::EntryPlunge);
            let steps = 4;
            for k in 1..=steps {
                let t = k as f64 / steps as f64;
                tp.feed_to_with_intent(
                    P3::new(x + run_len * t, 0.0, -3.0),
                    500.0,
                    MoveIntent::FinishingCut,
                );
            }
            x += run_len + gap;
        }
        tp.rapid_to_with_intent(P3::new(x - gap, 0.0, safe_z), MoveIntent::Retract);
        tp
    }

    fn rapid_len(tp: &crate::toolpath::Toolpath) -> f64 {
        tp.total_rapid_distance()
    }

    #[test]
    fn relink_replaces_close_junctions_with_surface_links() {
        use crate::toolpath::MoveType;
        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;
        let tp = fragmented_valley_path(6, 4.0, 1.5, safe_z);

        let params = RelinkParams {
            hookup_distance: 3.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: None,
        };
        let (out, report) = relink_fragments(&tp, &mesh, &index, &tool, &params);

        assert_eq!(report.fragments, 6, "fixture must present 6 fragments");
        assert_eq!(
            report.surface_links, 5,
            "every 1.5mm junction is inside the 3mm hookup and rides the \
             valley floor, so all five must link: {report:?}"
        );
        assert_eq!(report.retract_links, 0, "{report:?}");
        assert!(
            rapid_len(&out) < rapid_len(&tp),
            "removing five retract round trips must cut rapid distance: \
             {} -> {}",
            rapid_len(&tp),
            rapid_len(&out)
        );
        // Exactly one approach and one final retract survive.
        let rapids = out
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Rapid))
            .count();
        assert_eq!(
            rapids, 2,
            "one approach + one final retract is all that should remain"
        );
        // Every link point rides the surface, never safe Z.
        for m in &out.moves {
            if m.intent == crate::toolpath::MoveIntent::Linking
                && !matches!(m.move_type, MoveType::Rapid)
            {
                assert!(
                    m.target.z < safe_z - 1.0,
                    "a surface link must stay down, got z={}",
                    m.target.z
                );
            }
        }
    }

    #[test]
    fn relink_refuses_gaps_beyond_hookup_and_keeps_cut_moves() {
        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;
        let tp = fragmented_valley_path(4, 4.0, 5.0, safe_z);

        let params = RelinkParams {
            hookup_distance: 3.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: None,
        };
        let (out, report) = relink_fragments(&tp, &mesh, &index, &tool, &params);
        assert_eq!(report.surface_links, 0, "5mm > 3mm hookup: {report:?}");
        assert_eq!(report.retract_links, 3, "{report:?}");
        assert_eq!(report.too_far, 3, "{report:?}");

        // The CUT moves must be untouched — the pass only rewrites the
        // airborne junctions between fragments.
        let cuts = |t: &crate::toolpath::Toolpath| -> Vec<(String, String)> {
            t.moves
                .windows(2)
                .filter(|w| w[1].intent == crate::toolpath::MoveIntent::FinishingCut)
                .map(|w| {
                    (
                        format!(
                            "{:.3},{:.3},{:.3}",
                            w[0].target.x, w[0].target.y, w[0].target.z
                        ),
                        format!(
                            "{:.3},{:.3},{:.3}",
                            w[1].target.x, w[1].target.y, w[1].target.z
                        ),
                    )
                })
                .collect()
        };
        assert_eq!(cuts(&tp), cuts(&out), "cut geometry must be identical");
    }

    #[test]
    fn relink_refuses_links_that_leave_the_boundary() {
        use crate::geo::P2;
        use crate::polygon::Polygon2;
        use crate::region_set::RegionSet;

        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;
        let tp = fragmented_valley_path(6, 4.0, 1.5, safe_z);

        let base = RelinkParams {
            hookup_distance: 3.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: None,
        };
        let (_, unbounded) = relink_fragments(&tp, &mesh, &index, &tool, &base);
        assert_eq!(
            unbounded.surface_links, 5,
            "control: without a boundary every junction links"
        );

        // A boundary that covers the cut runs but NOT the gaps between
        // them: each 1.5mm junction now crosses excluded territory.
        let mut rings: Vec<Polygon2> = Vec::new();
        let mut x = 1.0;
        for _ in 0..6 {
            rings.push(Polygon2::new(vec![
                P2::new(x - 0.2, -1.0),
                P2::new(x + 4.2, -1.0),
                P2::new(x + 4.2, 1.0),
                P2::new(x - 0.2, 1.0),
            ]));
            x += 4.0 + 1.5;
        }
        let region = RegionSet::new(rings);
        let bounded_params = RelinkParams {
            boundary: Some(&region),
            ..base
        };
        let (_, bounded) = relink_fragments(&tp, &mesh, &index, &tool, &bounded_params);
        assert_eq!(
            bounded.surface_links, 0,
            "a link crossing excluded territory is a CUTTING feed over ground \
             the op was confined away from — the selective-finishing gouge \
             class: {bounded:?}"
        );
        assert_eq!(bounded.outside_boundary, 5, "{bounded:?}");
        assert_eq!(bounded.retract_links, 5, "{bounded:?}");
    }

    #[test]
    fn relink_reorder_visits_nearest_first() {
        use crate::toolpath::{MoveIntent, Toolpath};
        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;

        // Three runs emitted in a deliberately bad order: near, far, middle.
        let mut tp = Toolpath::new();
        for x in [1.0_f64, 40.0, 20.0] {
            tp.rapid_to_with_intent(P3::new(x, 0.0, safe_z), MoveIntent::Linking);
            tp.feed_to_with_intent(P3::new(x, 0.0, -3.0), 100.0, MoveIntent::EntryPlunge);
            tp.feed_to_with_intent(P3::new(x + 3.0, 0.0, -3.0), 500.0, MoveIntent::FinishingCut);
            tp.rapid_to_with_intent(P3::new(x + 3.0, 0.0, safe_z), MoveIntent::Retract);
        }

        let base = RelinkParams {
            hookup_distance: 0.0, // linking off: isolate the ordering
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: None,
        };
        let (kept, _) = relink_fragments(&tp, &mesh, &index, &tool, &base);
        let reordered_params = RelinkParams {
            reorder: true,
            ..base
        };
        let (reordered, _) = relink_fragments(&tp, &mesh, &index, &tool, &reordered_params);
        assert!(
            rapid_len(&reordered) < rapid_len(&kept),
            "near→far→middle must reorder to near→middle→far: {} -> {}",
            rapid_len(&kept),
            rapid_len(&reordered)
        );
    }

    /// A surface link rides the mesh (finite Z everywhere) when both ends sit on
    /// it, and returns None when the span is off the mesh (caller then retracts).
    #[test]
    fn build_surface_link_follows_surface_and_detects_offmesh() {
        let mesh = make_v_valley(20.0, 6.0, 0.5, 20, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);

        let on = build_surface_link(
            P3::new(5.0, 0.0, 0.0),
            P3::new(9.0, 0.0, 0.0),
            &mesh,
            &index,
            &tool,
            0.0,
            0.5,
        );
        let pts = on.unwrap();
        assert!(
            !pts.is_empty(),
            "a 4mm link at 0.5mm spacing has interior points"
        );
        for p in &pts {
            assert!(p.z.is_finite(), "each link point rides the surface");
        }

        let off = build_surface_link(
            P3::new(100.0, 100.0, 0.0),
            P3::new(105.0, 100.0, 0.0),
            &mesh,
            &index,
            &tool,
            0.0,
            0.5,
        );
        assert!(off.is_none(), "a link entirely off the mesh must be None");
    }
}
