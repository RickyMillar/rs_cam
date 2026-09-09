//! G-LINKSTAGE — the shared surface-link stage for finishing fragments.
//!
//! # What this pins
//!
//! A finishing pass confined to the multi-tool planner's islands is
//! RETRACT-COUNT-BOUND, and the pencil is ENTRY-COUNT-BOUND. One stage has to
//! serve both, which is why the arms below measure two different things:
//! whether a link is SAFE (arms a, b) and whether it removed an ENTRY rather
//! than merely a retract (arm e).
//!
//! # The defect the safety arms are about
//!
//! Before the stage, the scallop passed `link_ceiling: None`
//! (`scallop.rs`, the A/M7 relink). With no ceiling a "surface link" rides the
//! MESH — and on a `FromRemainingStock` island pass the mesh sits BELOW the
//! standing rest material, so the link is a LATERAL FEED AT CUT DEPTH straight
//! through stock. That is exactly the shape G-ISOCLIPRAPID names, one layer
//! down: both endpoints legitimately under the stock surface, the span between
//! them buried. The rapid checker cannot see it, because the move is a feed.
//!
//! The measure is therefore CHORD-SAMPLED, borrowed from
//! `isoclip_link_rapid_g_isocliprapid::rapid_burial_mm` and arm g of
//! `isoclip_entry_ramp_g_isoclipentry`: a target-only reading is 0.000 on a
//! leg that dives under the material between its endpoints and comes back out.
//!
//! # Arms
//!
//! * `a_a_link_across_standing_stock_clears_it` — green, with the ceiling.
//! * `b_without_the_ceiling_the_same_link_is_buried` — RED, kept green by
//!   asserting the DEFECT, so arm a cannot quietly stop measuring anything.
//! * `c_a_rotated_closed_loop_keeps_every_cut_position` — the rotation is a
//!   permutation of the loop, never a resampling.
//! * `d_a_rotated_loop_starts_near_the_previous_exit` — and it does the thing
//!   it exists for.
//! * `e_the_two_link_tiers_are_counted_apart` — the acceptance measure: a
//!   lifted hop is not an at-depth link, because only the latter removes an
//!   entry.
//! * `f_declaring_no_fragment_kinds_is_byte_identical` — every family that has
//!   not opted in.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::surface_link::{
    FragmentKind, LinkCeiling, RelinkParams, relink_fragments, relink_fragments_with_kinds,
};
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::transform_provenance::ReconcileSet;

// ── Fixture ─────────────────────────────────────────────────────────────

const CELL_MM: f64 = 0.2;
const BOARD_MM: f64 = 26.0;
const STOCK_BOTTOM_Z: f64 = -5.0;
/// The top of the rib of rest material the link has to get across.
const RIB_TOP_Z: f64 = 2.0;
/// What the upstream tool cut the ground down to everywhere else.
const FLOOR_Z: f64 = 0.0;
const SAFE_Z: f64 = 12.0;
const CUT_FEED: f64 = 800.0;
const PLUNGE_RATE: f64 = 200.0;
/// The rib, in X. Both cut fragments sit clear of it.
const RIB_X: (f64, f64) = (12.0, 14.0);
/// One cell, plus the half-cell dilation `max_conservative_top_z_in_disc`
/// adds on purpose. Same allowance as the G-ISOCLIPRAPID sentry.
const TOL_MM: f64 = 0.05;

/// A FLAT endmill on purpose. `LinkCeiling::required_tip_z` is byte-identical
/// to the flat-cylinder `material_top` for this profile (the documented safety
/// anchor), so the flat-disc burial measure below is EXACTLY the quantity the
/// stage reasons about — no profile allowance to argue over.
fn cutter() -> FlatEndmill {
    FlatEndmill::new(2.0, 20.0)
}

/// A flat plane at [`FLOOR_Z`] covering the whole board — the design surface a
/// finishing pass drop-cutters against.
fn flat_mesh() -> TriangleMesh {
    let n = 26usize;
    let step = BOARD_MM / n as f64;
    let mut verts = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            verts.push(P3::new(i as f64 * step, j as f64 * step, FLOOR_Z));
        }
    }
    let idx = |i: usize, j: usize| (j * (n + 1) + i) as u32;
    let mut tris = Vec::new();
    for j in 0..n {
        for i in 0..n {
            tris.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
            tris.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// Ground cut to [`FLOOR_Z`] everywhere, with one rib of rest material still
/// standing at [`RIB_TOP_Z`] between the two cut fragments.
fn rest_stock() -> TriDexelStock {
    let mut stock = TriDexelStock::from_stock(
        0.0,
        0.0,
        BOARD_MM,
        BOARD_MM,
        STOCK_BOTTOM_Z,
        RIB_TOP_Z,
        CELL_MM,
    );
    let (rows, cols) = (stock.z_grid.rows, stock.z_grid.cols);
    let (cs, ou) = (stock.z_grid.cell_size, stock.z_grid.origin_u);
    for row in 0..rows {
        for col in 0..cols {
            let x = ou + col as f64 * cs;
            let standing = (RIB_X.0..=RIB_X.1).contains(&x);
            let top = if standing { RIB_TOP_Z } else { FLOOR_Z };
            stock.clear_above_at(row, col, top as f32);
        }
    }
    stock
}

/// Two cutting runs at [`FLOOR_Z`], one either side of the rib, joined the way
/// every discrete finishing generator joins them: retract, traverse at safe Z,
/// replunge.
fn two_runs_across_the_rib() -> Toolpath {
    let mut tp = Toolpath::new();
    for (x0, x1) in [(6.0_f64, 10.0_f64), (16.0, 20.0)] {
        tp.rapid_to_with_intent(P3::new(x0, 12.0, SAFE_Z), MoveIntent::Linking);
        tp.feed_to_with_intent(
            P3::new(x0, 12.0, FLOOR_Z),
            PLUNGE_RATE,
            MoveIntent::EntryPlunge,
        );
        for k in 1..=8 {
            let t = k as f64 / 8.0;
            tp.feed_to_with_intent(
                P3::new(x0 + (x1 - x0) * t, 12.0, FLOOR_Z),
                CUT_FEED,
                MoveIntent::FinishingCut,
            );
        }
        tp.rapid_to_with_intent(P3::new(x1, 12.0, SAFE_Z), MoveIntent::Retract);
    }
    tp
}

/// A closed ring at [`FLOOR_Z`], emitted the way the scallop's discrete branch
/// emits one: plunge onto the first point, feed round, close back onto it.
/// `phase` rotates the STORED start vertex without moving the ring. That is
/// the whole point of the fixture: the offset library picks a start vertex of
/// its own, and two radially adjacent rings then meet at unrelated points of
/// their circumference (`planning/linking_2026-09-09/SPEC.md` §2, cause 2).
fn ring(cx: f64, cy: f64, r: f64, n: usize, phase: f64) -> Vec<P3> {
    (0..n)
        .map(|k| {
            let a = phase + std::f64::consts::TAU * k as f64 / n as f64;
            P3::new(cx + r * a.cos(), cy + r * a.sin(), FLOOR_Z)
        })
        .collect()
}

fn emit_ring(tp: &mut Toolpath, pts: &[P3]) {
    let first = pts[0];
    tp.rapid_to_with_intent(P3::new(first.x, first.y, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(first, PLUNGE_RATE, MoveIntent::EntryPlunge);
    for p in pts.iter().skip(1) {
        tp.feed_to_with_intent(*p, CUT_FEED, MoveIntent::FinishingCut);
    }
    tp.feed_to_with_intent(first, CUT_FEED, MoveIntent::FinishingCut);
    tp.rapid_to_with_intent(P3::new(first.x, first.y, SAFE_Z), MoveIntent::Retract);
}

fn base_params<'a>() -> RelinkParams<'a> {
    RelinkParams {
        hookup_distance: 8.0,
        stock_to_leave: 0.0,
        sampling: 0.25,
        feed_rate: CUT_FEED,
        plunge_rate: PLUNGE_RATE,
        safe_z: SAFE_Z,
        link_kinematics: None,
        reorder: false,
        boundary: None,
        link_ceiling: None,
        flush_ride: false,
        airborne_links_may_leave_territory: false,
    }
}

// ── The measure ─────────────────────────────────────────────────────────

/// The deepest any FED LINKING move runs inside the rest stock, ANYWHERE
/// along its own chord.
///
/// Chord-sampled at half a cell, exactly as
/// `isoclip_link_rapid_g_isocliprapid::rapid_burial_mm` samples a rapid. The
/// target-only reading this replaces is 0.000 on a link whose two endpoints
/// sit on cut ground either side of a rib: both ends are legitimate cut
/// positions and the whole gouge is in the middle.
fn worst_link_burial_mm(tp: &Toolpath, stock: &TriDexelStock, radius: f64) -> f64 {
    let mut worst = 0.0_f64;
    for w in tp.moves.windows(2) {
        let (prev, cur) = (&w[0], &w[1]);
        if cur.intent != MoveIntent::Linking || matches!(cur.move_type, MoveType::Rapid) {
            continue;
        }
        let (a, b) = (prev.target, cur.target);
        let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        let steps = (len / (CELL_MM * 0.5)).ceil().max(1.0) as usize;
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            let (x, y, z) = (a.x + dx * t, a.y + dy * t, a.z + dz * t);
            if let Some(ceiling) = stock.max_conservative_top_z_in_disc(x, y, radius) {
                worst = worst.max(ceiling - z);
            }
        }
    }
    worst
}

fn relink(
    tp: Toolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    tool: &dyn MillingCutter,
    params: &RelinkParams<'_>,
    kinds: Option<&[FragmentKind]>,
) -> (Toolpath, rs_cam_core::surface_link::RelinkReport) {
    let (transformed, report) =
        relink_fragments_with_kinds(AnnotatedToolpath::new(tp), mesh, index, tool, params, kinds);
    (
        transformed
            .reconcile(&mut ReconcileSet::empty())
            .into_inner()
            .toolpath,
        report,
    )
}

// ── Arms ────────────────────────────────────────────────────────────────

#[test]
fn a_a_link_across_standing_stock_clears_it() {
    let mesh = flat_mesh();
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = cutter();
    let stock = rest_stock();
    let params = RelinkParams {
        link_ceiling: Some(LinkCeiling {
            stock: Some(&stock),
            tool_radius: tool.envelope_radius_mm(),
            fallback_top_z: RIB_TOP_Z,
        }),
        // The finishing prior. Flush ground is the prior pass's machined
        // output; the rib is NOT flush, so the flush arm cannot fire on it.
        flush_ride: true,
        ..base_params()
    };
    let (out, report) = relink(
        two_runs_across_the_rib(),
        &mesh,
        &index,
        &tool,
        &params,
        None,
    );
    assert_eq!(
        report.surface_links, 1,
        "fixture precondition: the one junction must be inside the 8 mm \
         hookup and must link — otherwise this arm measures nothing: {report:?}"
    );
    assert_eq!(
        report.clearance_hops, 1,
        "…and it must take the LIFTED tier, because riding the surface across \
         a 2 mm rib is the gouge: {report:?}"
    );
    assert_eq!(report.at_depth_links, 0, "{report:?}");

    let burial = worst_link_burial_mm(&out, &stock, tool.envelope_radius_mm());
    assert!(
        burial <= TOL_MM,
        "a fed link runs {burial:.3} mm inside the rest stock it crosses"
    );
}

#[test]
fn b_without_the_ceiling_the_same_link_is_buried() {
    // RED arm, kept green by asserting the DEFECT. This is the scallop's
    // pre-G-LINKSTAGE configuration, on the same fixture as arm a: the only
    // variable is `link_ceiling`.
    let mesh = flat_mesh();
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = cutter();
    let stock = rest_stock();
    let (out, report) = relink(
        two_runs_across_the_rib(),
        &mesh,
        &index,
        &tool,
        &base_params(),
        None,
    );
    assert_eq!(
        report.at_depth_links, 1,
        "without a ceiling the kernel has no reason to lift: {report:?}"
    );
    let burial = worst_link_burial_mm(&out, &stock, tool.envelope_radius_mm());
    assert!(
        burial > 1.0,
        "the fixture must actually put material in the link's way — the \
         un-ceilinged link should be about {RIB_TOP_Z} mm buried, read \
         {burial:.3}"
    );
}

#[test]
fn c_a_rotated_closed_loop_keeps_every_cut_position() {
    let mesh = flat_mesh();
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = cutter();

    // Two concentric rings. Ring 2's stored start vertex is on the FAR side
    // from where ring 1 ends, which is the breadth-first offset-cascade shape
    // the linking spec §2 measured.
    let a = ring(13.0, 13.0, 5.0, 48, 0.0);
    // Stored start on the FAR side of the ring from where ring `a` exits.
    let b = ring(13.0, 13.0, 4.0, 48, std::f64::consts::PI);
    let mut tp = Toolpath::new();
    emit_ring(&mut tp, &a);
    emit_ring(&mut tp, &b);

    let kinds = [FragmentKind::ClosedLoop, FragmentKind::ClosedLoop];
    let params = RelinkParams {
        hookup_distance: 3.0,
        reorder: true,
        ..base_params()
    };
    let (out, report) = relink(tp.clone(), &mesh, &index, &tool, &params, Some(&kinds));
    assert_eq!(
        report.rotated_loops, 1,
        "the second ring must be rotated toward the first ring's exit: \
         {report:?}"
    );

    // The rotation is a PERMUTATION of the loop, never a resampling: every
    // position the tool used to feed to is still a position it feeds to.
    let fed = |t: &Toolpath| -> Vec<(i64, i64, i64)> {
        t.moves
            .iter()
            .filter(|m| !matches!(m.move_type, MoveType::Rapid))
            .map(|m| {
                (
                    (m.target.x * 1e6).round() as i64,
                    (m.target.y * 1e6).round() as i64,
                    (m.target.z * 1e6).round() as i64,
                )
            })
            .collect()
    };
    let after: std::collections::BTreeSet<_> = fed(&out).into_iter().collect();
    let missing: Vec<_> = fed(&tp)
        .into_iter()
        .filter(|p| !after.contains(p))
        .collect();
    assert!(
        missing.is_empty(),
        "a rotation may re-order a loop; it may never drop a cut position: \
         {} lost, first {:?}",
        missing.len(),
        missing.first()
    );

    // And the CUT DIRECTION survives. Both input rings are emitted
    // counter-clockwise, so the signed area of the emitted cutting polygon
    // stays positive.
    let signed_area = |pts: &[P3]| -> f64 {
        let mut s = 0.0;
        for w in pts.windows(2) {
            s += w[0].x * w[1].y - w[1].x * w[0].y;
        }
        s * 0.5
    };
    let inner_cut: Vec<P3> = out
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.target)
        .filter(|p| ((p.x - 13.0).hypot(p.y - 13.0) - 4.0).abs() < 1e-6)
        .collect();
    assert!(
        signed_area(&inner_cut) > 0.0,
        "the rotated ring must still be cut counter-clockwise"
    );
}

#[test]
fn d_a_rotated_loop_starts_near_the_previous_exit() {
    let mesh = flat_mesh();
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = cutter();
    let a = ring(13.0, 13.0, 5.0, 48, 0.0);
    // Stored start on the FAR side of the ring from where ring `a` exits.
    let b = ring(13.0, 13.0, 4.0, 48, std::f64::consts::PI);
    let mut tp = Toolpath::new();
    emit_ring(&mut tp, &a);
    emit_ring(&mut tp, &b);
    let params = RelinkParams {
        // Linking OFF: this arm isolates the rotation from the link decision.
        hookup_distance: 0.0,
        reorder: true,
        ..base_params()
    };

    let (plain, _) = relink(tp.clone(), &mesh, &index, &tool, &params, None);
    let kinds = [FragmentKind::ClosedLoop, FragmentKind::ClosedLoop];
    let (rotated, report) = relink(tp, &mesh, &index, &tool, &params, Some(&kinds));
    assert_eq!(report.rotated_loops, 1, "{report:?}");

    // The junction gap: the XY distance from the first ring's exit to the
    // second ring's entry. Rotation exists to shrink exactly this.
    let junction_gap = |t: &Toolpath| -> f64 {
        let plunges: Vec<usize> = t
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| m.intent == MoveIntent::EntryPlunge)
            .map(|(i, _)| i)
            .collect();
        let second = plunges[1];
        // The last cut position before that plunge's own approach rapid.
        let exit = t.moves[..second]
            .iter()
            .rev()
            .find(|m| !matches!(m.move_type, MoveType::Rapid))
            .unwrap()
            .target;
        let entry = t.moves[second].target;
        (entry.x - exit.x).hypot(entry.y - exit.y)
    };
    let before = junction_gap(&plain);
    let after = junction_gap(&rotated);
    assert!(
        after < before - 1e-9,
        "rotating the loop must bring its start toward the tool: {before:.3} \
         mm -> {after:.3} mm"
    );
    // Ring spacing is 1 mm, so a rotation that found the nearest point brings
    // the junction inside a couple of millimetres — the band where a 3 mm
    // hookup can act at all.
    assert!(
        after < 2.0,
        "the rotated start should be about one ring spacing away, read \
         {after:.3} mm"
    );
}

#[test]
fn e_the_two_link_tiers_are_counted_apart() {
    // The ACCEPTANCE measure. Only an at-depth link removes the next
    // fragment's entry; a clearance hop removes the retract and leaves the
    // descent standing. A stage that reported one number for both could not
    // tell the island passes (retract-bound) from the pencil (entry-bound).
    let mesh = flat_mesh();
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = cutter();
    let stock = rest_stock();

    // Same ceiling, two junctions: one whose hop crosses the rib (lifted) and
    // one entirely on flush machined ground (rides at depth).
    let mut tp = Toolpath::new();
    for (x0, x1) in [(6.0_f64, 10.0_f64), (16.0, 20.0), (21.0, 24.0)] {
        tp.rapid_to_with_intent(P3::new(x0, 12.0, SAFE_Z), MoveIntent::Linking);
        tp.feed_to_with_intent(
            P3::new(x0, 12.0, FLOOR_Z),
            PLUNGE_RATE,
            MoveIntent::EntryPlunge,
        );
        for k in 1..=8 {
            let t = k as f64 / 8.0;
            tp.feed_to_with_intent(
                P3::new(x0 + (x1 - x0) * t, 12.0, FLOOR_Z),
                CUT_FEED,
                MoveIntent::FinishingCut,
            );
        }
        tp.rapid_to_with_intent(P3::new(x1, 12.0, SAFE_Z), MoveIntent::Retract);
    }
    let params = RelinkParams {
        link_ceiling: Some(LinkCeiling {
            stock: Some(&stock),
            tool_radius: tool.envelope_radius_mm(),
            fallback_top_z: RIB_TOP_Z,
        }),
        flush_ride: true,
        ..base_params()
    };
    let (_, report) = relink(tp, &mesh, &index, &tool, &params, None);
    assert_eq!(report.surface_links, 2, "{report:?}");
    assert_eq!(
        (report.at_depth_links, report.clearance_hops),
        (1, 1),
        "the flush hop rides at depth and removes an entry; the hop across \
         the rib lifts and does not: {report:?}"
    );
    assert_eq!(
        report.at_depth_links + report.clearance_hops,
        report.surface_links,
        "`surface_links` must stay the sum, for every existing reader"
    );
}

#[test]
fn f_declaring_no_fragment_kinds_is_byte_identical() {
    // The byte-identity gate for every family that has not opted in. The two
    // entry points must produce the same moves, in the same order, with the
    // same feeds and intents — with reordering both ON and OFF, because the
    // walk that picks the order was rewritten to interleave rotation with it.
    let mesh = flat_mesh();
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = cutter();
    let a = ring(13.0, 13.0, 5.0, 48, 0.0);
    // Stored start on the FAR side of the ring from where ring `a` exits.
    let b = ring(13.0, 13.0, 4.0, 48, std::f64::consts::PI);
    let mut tp = Toolpath::new();
    emit_ring(&mut tp, &a);
    emit_ring(&mut tp, &b);
    emit_ring(&mut tp, &ring(6.0, 6.0, 2.0, 24, 0.0));

    for reorder in [false, true] {
        let params = RelinkParams {
            hookup_distance: 3.0,
            reorder,
            ..base_params()
        };
        let (legacy, legacy_report) = {
            let (t, r) = relink_fragments(
                AnnotatedToolpath::new(tp.clone()),
                &mesh,
                &index,
                &tool,
                &params,
            );
            (
                t.reconcile(&mut ReconcileSet::empty())
                    .into_inner()
                    .toolpath,
                r,
            )
        };
        let (staged, staged_report) = relink(tp.clone(), &mesh, &index, &tool, &params, None);
        assert_eq!(
            legacy_report.rotated_loops, 0,
            "no kinds declared means no rotation, whatever the order"
        );
        assert_eq!(staged_report.rotated_loops, 0);
        assert_eq!(
            legacy.moves.len(),
            staged.moves.len(),
            "reorder={reorder}: move count moved"
        );
        for (i, (x, y)) in legacy.moves.iter().zip(staged.moves.iter()).enumerate() {
            assert_eq!(x.intent, y.intent, "reorder={reorder}, move {i}");
            assert_eq!(x.move_type, y.move_type, "reorder={reorder}, move {i}");
            assert!(
                (x.target.x - y.target.x).abs() < 1e-12
                    && (x.target.y - y.target.y).abs() < 1e-12
                    && (x.target.z - y.target.z).abs() < 1e-12,
                "reorder={reorder}, move {i}: {:?} vs {:?}",
                x.target,
                y.target
            );
        }
    }
}
