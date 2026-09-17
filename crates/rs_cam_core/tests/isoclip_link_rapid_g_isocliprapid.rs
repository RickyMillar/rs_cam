//! G-ISOCLIPRAPID — a lead-in repositions in XY at the retract plane, never at
//! the height the previous move happened to stop at.
//!
//! # The defect
//!
//! `dressup::apply_lead_in_out` replaces a generator plunge with three moves:
//! a pre-position RAPID over the lead start, a pure-Z descent onto it, then the
//! tangent arc into the cut. The first of those is the only move in the
//! manoeuvre that travels in XY, so it is the only one that has to clear the
//! stock — and it read its height from `moves[i - 1].target.z`, "the preceding
//! rapid's Z".
//!
//! That preceding move is not always a rapid, and not always safe. Where the
//! generator's own retract has been consumed or lowered by an earlier dressup —
//! or where the plunge simply follows a cut, which is what a stepped pass and a
//! lead-out/lead-in chain both produce — the inherited height is a CUTTING
//! depth, and the lead-in then traverses the work at it.
//!
//! Measured on the wanaka200 board (`planning/deep_doc_modulation_2026-09-08/`,
//! `T2_r20_raster_then_r10_iso_islands.toml`, 0.2 mm, headless): one rapid
//! through stock on tier 1 at move 68538, world (109.1, 13.1, 3.31) to
//! (104.9, 15.3, 3.31) — 4.7 mm of lateral travel with both ends at the same
//! sub-stock height, and the emitted program shows the rest of the lead-in
//! signature right behind it: a pure-Z move at the lead feed onto a target the
//! surface probe had LIFTED (so it climbs, not descends), then eight lead arc
//! samples, then the cut.
//!
//! # The rule
//!
//! A rapid that repositions in XY belongs at the operation's retract plane.
//! That plane is clear of the stock by construction
//! (`compute::config::effective_safe_z` floors it at `stock_top +
//! SAFE_Z_CLEARANCE_MM`), and it is the only height available at this layer
//! that is: the surface probe answers about the MODEL, which on a rest-driven
//! pass sits below the material — the same lesson G-ISOCLIPENTRY records.
//!
//! # Arms
//!
//! * `a_the_lead_in_repositions_at_the_retract_plane` — green.
//! * `b_the_inherited_height_would_have_been_inside_the_stock` — RED, kept
//!   green by asserting the DEFECT. It re-derives the height the old rule chose
//!   and asserts it is buried, so the fixture cannot quietly stop exercising
//!   the case and leave the green arm measuring nothing.
//! * `c_no_rapid_crosses_the_rest_stock` — end to end through the real dressup
//!   chain and the island clip: every XY-travelling rapid clears the rest stock
//!   over its whole span.
//! * `d_a_safe_predecessor_leaves_the_emission_untouched` — parity.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::execute::apply_dressups;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::dressup::apply_lead_in_out;
use rs_cam_core::geo::P3;
use rs_cam_core::geometry::boundary::clip_toolpath_to_boundary_set_with_provenance;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::trace::transform_provenance::ReconcileSet;

// ── Fixture ─────────────────────────────────────────────────────────────

const CELL_MM: f64 = 0.2;
const STOCK_BOTTOM_Z: f64 = -5.0;
/// The top of the rest material standing inside an island.
const STOCK_TOP_Z: f64 = 2.0;
/// What the upstream tool cut the ground down to everywhere it could reach.
const FLOOR_Z: f64 = 0.0;

/// The operation's retract plane. Clear of the stock by construction.
const RETRACT_Z: f64 = 12.0;
const CUT_FEED: f64 = 782.0;
const PLUNGE_RATE: f64 = 171.0;
const LEAD_RADIUS_MM: f64 = 2.0;

/// Two islands, four millimetres apart in X — the shape the multitool
/// planner's tier regions have, and far enough apart that a link between them
/// is a real traverse.
const ISLAND_A: (f64, f64, f64, f64) = (4.0, 4.0, 14.0, 20.0);
const ISLAND_B: (f64, f64, f64, f64) = (17.0, 4.0, 24.0, 20.0);

/// One cell, plus the half-cell dilation `max_conservative_top_z_in_disc`
/// adds on purpose.
const TOL_MM: f64 = 0.05;

fn tapered_ball() -> TaperedBallEndmill {
    TaperedBallEndmill::new(2.0, 7.1, 6.0, 20.0)
}

fn islands() -> Vec<Polygon2> {
    vec![
        Polygon2::rectangle(ISLAND_A.0, ISLAND_A.1, ISLAND_A.2, ISLAND_A.3),
        Polygon2::rectangle(ISLAND_B.0, ISLAND_B.1, ISLAND_B.2, ISLAND_B.3),
    ]
}

fn in_rect(r: (f64, f64, f64, f64), x: f64, y: f64) -> bool {
    (r.0..=r.2).contains(&x) && (r.1..=r.3).contains(&y)
}

/// Ground cut to [`FLOOR_Z`] everywhere the upstream tool reached, with both
/// islands still standing at the raw stock top.
fn rest_stock() -> TriDexelStock {
    let mut stock =
        TriDexelStock::from_stock(0.0, 0.0, 26.0, 26.0, STOCK_BOTTOM_Z, STOCK_TOP_Z, CELL_MM);
    let (rows, cols) = (stock.z_grid.rows, stock.z_grid.cols);
    let (cs, ou, ov) = (
        stock.z_grid.cell_size,
        stock.z_grid.origin_u,
        stock.z_grid.origin_v,
    );
    for row in 0..rows {
        let y = ov + row as f64 * cs;
        for col in 0..cols {
            let x = ou + col as f64 * cs;
            let standing = in_rect(ISLAND_A, x, y) || in_rect(ISLAND_B, x, y);
            let top = if standing { STOCK_TOP_Z } else { FLOOR_Z };
            stock.clear_above_at(row, col, top as f32);
        }
    }
    stock
}

/// A stepped pass: cut a level, then plunge to the next one WITHOUT retracting.
///
/// This is the input shape the defect needs and the one the wanaka trail shows:
/// the move before the plunge is a cutting move, so "the preceding rapid's Z"
/// is a cutting depth.
fn stepped_pass() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(8.0, 12.0, RETRACT_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(
        P3::new(8.0, 12.0, FLOOR_Z),
        PLUNGE_RATE,
        MoveIntent::EntryPlunge,
    );
    // First level.
    for k in 1..=8 {
        tp.feed_to_with_intent(
            P3::new(8.0 + k as f64 * 0.25, 12.0, FLOOR_Z),
            CUT_FEED,
            MoveIntent::FinishingCut,
        );
    }
    // Step down in place. `is_plunge` matches this, and its predecessor is the
    // cut above — at FLOOR_Z, not at the retract plane.
    tp.feed_to_with_intent(
        P3::new(10.0, 12.0, FLOOR_Z - 1.0),
        PLUNGE_RATE,
        MoveIntent::EntryPlunge,
    );
    for k in 1..=8 {
        tp.feed_to_with_intent(
            P3::new(10.0 - k as f64 * 0.25, 12.0, FLOOR_Z - 1.0),
            CUT_FEED,
            MoveIntent::FinishingCut,
        );
    }
    tp
}

/// The same pass with a retract before the second plunge — the shape a
/// generator writes when nothing has consumed its retract.
fn retracted_pass() -> Toolpath {
    let mut tp = Toolpath::new();
    let moves = stepped_pass().moves;
    for (idx, m) in moves.iter().enumerate() {
        if idx == 10 {
            tp.rapid_to_with_intent(P3::new(10.0, 12.0, RETRACT_Z), MoveIntent::Retract);
        }
        tp.moves.push(m.clone());
    }
    tp
}

fn led(tp: Toolpath, retract_z: Option<f64>) -> Toolpath {
    apply_lead_in_out(
        AnnotatedToolpath::new(tp),
        LEAD_RADIUS_MM,
        None,
        None,
        None,
        retract_z,
    )
    .reconcile(&mut ReconcileSet::empty())
    .into_inner()
    .toolpath
}

// ── Measures ────────────────────────────────────────────────────────────

/// Every rapid that travels in XY, as `(index, start, end)`.
fn travelling_rapids(tp: &Toolpath) -> Vec<(usize, P3, P3)> {
    let mut out = Vec::new();
    for (i, pair) in tp.moves.windows(2).enumerate() {
        let (prev, cur) = (&pair[0], &pair[1]);
        if cur.move_type != MoveType::Rapid {
            continue;
        }
        let dxy = ((cur.target.x - prev.target.x).powi(2) + (cur.target.y - prev.target.y).powi(2))
            .sqrt();
        if dxy > 1e-6 {
            out.push((i + 1, prev.target, cur.target));
        }
    }
    out
}

/// How deep a rapid runs below the stock's own conservative ceiling, sampled
/// along its span. Zero when the rapid is clear.
fn rapid_burial_mm(stock: &TriDexelStock, radius: f64, from: P3, to: P3) -> f64 {
    let (dx, dy, dz) = (to.x - from.x, to.y - from.y, to.z - from.z);
    let len = (dx * dx + dy * dy + dz * dz).sqrt();
    let steps = (len / (CELL_MM * 0.5)).ceil().max(1.0) as usize;
    let mut worst = 0.0_f64;
    for k in 0..=steps {
        let t = k as f64 / steps as f64;
        let (x, y, z) = (from.x + dx * t, from.y + dy * t, from.z + dz * t);
        if let Some(ceiling) = stock.max_conservative_top_z_in_disc(x, y, radius) {
            worst = worst.max(ceiling - z);
        }
    }
    worst
}

/// The pre-position rapid of the SECOND lead-in — the one whose predecessor is
/// a cutting move.
fn second_lead_in_rapid(tp: &Toolpath) -> P3 {
    let rapids: Vec<P3> = tp
        .moves
        .iter()
        .filter(|m| m.move_type == MoveType::Rapid && m.intent == MoveIntent::LeadIn)
        .map(|m| m.target)
        .collect();
    assert!(
        rapids.len() >= 2,
        "fixture precondition: two lead-ins, got {} ({:?})",
        rapids.len(),
        tp.moves
    );
    rapids[1]
}

// ── Arms ────────────────────────────────────────────────────────────────

#[test]
fn a_the_lead_in_repositions_at_the_retract_plane() {
    let out = led(stepped_pass(), Some(RETRACT_Z));
    let rapid = second_lead_in_rapid(&out);
    assert!(
        (rapid.z - RETRACT_Z).abs() < 1e-9,
        "the lead-in repositioned at Z {:.3}, not the {RETRACT_Z:.3} retract plane",
        rapid.z
    );
}

#[test]
fn b_the_inherited_height_would_have_been_inside_the_stock() {
    // RED ARM, kept green by asserting the DEFECT. The pre-fix rule was
    // `safe_z = moves[i - 1].target.z` — the height of the move before the
    // plunge. Re-derive it from the fixture's own input and show it is buried,
    // so the green arm above cannot pass by measuring a case that stopped
    // being dangerous.
    let input = stepped_pass();
    let plunge_idx = input
        .moves
        .iter()
        .enumerate()
        .filter(|(_, m)| m.intent == MoveIntent::EntryPlunge)
        .nth(1)
        .map(|(i, _)| i)
        .expect("fixture precondition: a second plunge");
    let inherited_z = input.moves[plunge_idx - 1].target.z;

    let stock = rest_stock();
    let cutter = tapered_ball();
    let radius = cutter.envelope_radius_mm();
    // The lead start is one lead radius off the cut, so the pre-position rapid
    // is a real XY traverse at `inherited_z`.
    let from = input.moves[plunge_idx].target;
    let to = P3::new(
        from.x - LEAD_RADIUS_MM,
        from.y + LEAD_RADIUS_MM,
        inherited_z,
    );
    let burial = rapid_burial_mm(&stock, radius, P3::new(from.x, from.y, inherited_z), to);
    assert!(
        burial > TOL_MM,
        "pre-fix reproduction lost: the inherited height {inherited_z:.3} clears \
         the rest stock by {burial:.3} mm"
    );
    // And it is a CUTTING depth, not a clearance height — the whole point.
    assert!(
        inherited_z < STOCK_TOP_Z,
        "fixture precondition: the inherited height must be under the stock top"
    );
}

#[test]
fn c_no_rapid_crosses_the_rest_stock() {
    // End to end: the real dressup chain, then the island clip, over a stock
    // whose two islands still stand. Every rapid that travels in XY must clear
    // that stock.
    let stock = rest_stock();
    let cutter = tapered_ball();
    let radius = cutter.envelope_radius_mm();
    let cfg = DressupConfig {
        entry_style: DressupEntryStyle::None,
        lead_in_out: true,
        lead_radius: LEAD_RADIUS_MM,
        link_moves: false,
        arc_fitting: false,
        segment_merge: false,
        optimize_rapid_order: false,
        feed_optimization: false,
        ..DressupConfig::default()
    };
    let dressed = apply_dressups(
        AnnotatedToolpath::new(two_island_pass()),
        &cfg,
        CUT_FEED,
        // WP22: no operation in scope, so the plunge cap does not apply.
        None,
        cutter.diameter(),
        RETRACT_Z,
        STOCK_TOP_Z,
        None,
        None,
        Some(&cutter),
        None,
        OperationType::Scallop.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::empty(),
    );
    let (clipped, _mapping) = clip_toolpath_to_boundary_set_with_provenance(
        &dressed.toolpath,
        &islands(),
        RETRACT_Z,
        Some(PLUNGE_RATE),
    );

    // Precondition: the fixture must actually put a lead-in on the far side of
    // a cutting move. Without this the arm can pass by measuring nothing.
    // Counted on the DRESSED path: a lead whose start falls outside its island
    // is rewritten by the clip to a safe-Z link, which is a different — and
    // safe — outcome, and would hide the case rather than test it.
    let lead_rapids = dressed
        .toolpath
        .moves
        .iter()
        .filter(|m| m.move_type == MoveType::Rapid && m.intent == MoveIntent::LeadIn)
        .count();
    assert!(
        lead_rapids >= 2,
        "fixture precondition: expected at least two lead-in rapids, got {lead_rapids}"
    );

    let mut worst = (0usize, 0.0_f64, P3::default(), P3::default());
    for (idx, from, to) in travelling_rapids(&clipped) {
        let burial = rapid_burial_mm(&stock, radius, from, to);
        if burial > worst.1 {
            worst = (idx, burial, from, to);
        }
    }
    assert!(
        worst.1 <= TOL_MM,
        "rapid {} runs {:.3} mm inside the rest stock: ({:.2}, {:.2}, {:.3}) -> \
         ({:.2}, {:.2}, {:.3})",
        worst.0,
        worst.1,
        worst.2.x,
        worst.2.y,
        worst.2.z,
        worst.3.x,
        worst.3.y,
        worst.3.z
    );
}

#[test]
fn d_a_safe_predecessor_leaves_the_emission_untouched() {
    // Parity. Where the generator's retract survives, the inherited height WAS
    // the retract plane, so the rule change moves nothing.
    let with_plane = led(retracted_pass(), Some(RETRACT_Z));
    let legacy = led(retracted_pass(), None);
    assert_eq!(with_plane.moves.len(), legacy.moves.len());
    for (a, b) in with_plane.moves.iter().zip(legacy.moves.iter()) {
        assert_eq!(a.intent, b.intent);
        assert_eq!(a.move_type, b.move_type);
        assert!((a.target.x - b.target.x).abs() < 1e-12);
        assert!((a.target.y - b.target.y).abs() < 1e-12);
        assert!((a.target.z - b.target.z).abs() < 1e-12);
    }
}

/// Two passes, one per island, joined by the kind of step-down the defect
/// needs. The clip turns the crossing into its own re-entry, which is what
/// puts a lead-in on the far island with a cutting move behind it.
fn two_island_pass() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(7.0, 12.0, RETRACT_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(
        P3::new(7.0, 12.0, FLOOR_Z),
        PLUNGE_RATE,
        MoveIntent::EntryPlunge,
    );
    let mut x = 7.0;
    while x < 11.0 {
        x += 0.25;
        tp.feed_to_with_intent(
            P3::new(x, 12.0, FLOOR_Z),
            CUT_FEED,
            MoveIntent::FinishingCut,
        );
    }
    // Step down in place on island A, then run back — no retract, so the next
    // lead-in inherits a cutting depth. Both its lead start and its whole
    // pre-position traverse stay INSIDE the island, so the clip keeps the move
    // verbatim and the hazard reaches the emitted program.
    tp.feed_to_with_intent(
        P3::new(11.0, 12.0, FLOOR_Z - 1.0),
        PLUNGE_RATE,
        MoveIntent::EntryPlunge,
    );
    while x > 7.0 {
        x -= 0.25;
        tp.feed_to_with_intent(
            P3::new(x, 12.0, FLOOR_Z - 1.0),
            CUT_FEED,
            MoveIntent::FinishingCut,
        );
    }
    // Cross to island B at cut depth. The clip rewrites the crossing.
    while x < 22.0 {
        x += 0.25;
        tp.feed_to_with_intent(
            P3::new(x, 12.0, FLOOR_Z - 1.0),
            CUT_FEED,
            MoveIntent::FinishingCut,
        );
    }
    tp
}
