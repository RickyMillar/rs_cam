//! A/M7 — the A/B for ring-to-ring surface links, and the safety gate that
//! stopped them shipping.
//!
//! Oracle: `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`
//! §A/M7, and `MEASUREMENT_DOMAINS.md` §7 for the metric.
//!
//! ## What was converted, and why this class
//!
//! Scallop's discrete-ring branch emitted `retract → rapid at safe_z →
//! replunge` at EVERY ring junction, unconditionally, however close the next
//! ring started. Finishing air is **count-bound** — a hop pays two ~safe_z Z
//! legs whatever its XY length — so on a pass of a few hundred rings those
//! round trips, not the cutting, set the wall clock. It is also the class
//! nothing else was relinking: `unified_finish` handles its own regions at
//! the region level, and pencil has had `hookup_distance` since P1, but the
//! standalone all-over scallop pass had no relink at all.
//!
//! ## What the gate must not let through
//!
//! A link that stays low is only safe if it cannot touch material it was
//! never asked to cut, so the interesting assertions here are the negative
//! ones:
//!
//! * **no cut position may be lost** — `relink_fragments` is supposed to
//!   copy fragment INTERIORS verbatim and replace only airborne junctions.
//!   It does not: it drops exactly ONE cut position per link. That gate
//!   fired, and it is why the dial ships default-OFF with the prize
//!   measured and unclaimed;
//! * **no new collisions at the FINEST resolution** (0.1 mm), not the
//!   default. A/M10 is what makes that check meaningful: before it, the
//!   collision count was a property of the grid, so "no new collisions"
//!   could be bought by checking coarsely;
//! * **throughput improves**, measured with an honest area numerator —
//!   `measurement::swept_footprint_area`, a radius-aware XY disc at
//!   emission stage, NOT the centreline bins that voided the original
//!   0.476-vs-0.938 figure (`MEASUREMENT_DOMAINS.md` §7, X-14).
//!
//! ## Reading the throughput number honestly
//!
//! Both branches cut essentially the same geometry, so the swept footprint
//! barely moves (1910 → 1906 mm², the 21 dropped positions) and mm²/s
//! improves almost entirely because the DENOMINATOR falls. That is stated
//! here so nobody reads the ratio as "more area finished". The numerator is
//! a footprint, not fresh area: it never consults the stock.
//!
//! The throughput number is therefore real but UNBANKABLE at present — it
//! is what the conversion is worth once the position loss is fixed, not
//! what today's default delivers, because today's default is off.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use common::meshes::sawtooth_plate;
use common::session::{generate, mesh_model, single_op_session_with, stock_over};
use common::tools::ball_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::ScallopConfig;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine_kinematics::{MachineKinematics, compute_cycle_time};
use rs_cam_core::measurement::{
    DEFAULT_FOOTPRINT_CELL_MM, swept_footprint_area, swept_footprint_mm2_per_s,
};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::session::SimulationOptions;
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType};

use std::sync::atomic::AtomicBool;

/// Ø3 ball — the tool the scallop op requires a ball tip for, and the one
/// `CANDIDATE_HOOKUP_MM` is sized against (a little over one diameter).
const BALL_DIAMETER_MM: f64 = 3.0;

/// The hookup the A/B is measured at — and the value the config default
/// would take if `relink_fragments` stopped losing a cut position per link.
/// It is deliberately NOT the shipped default, which is 0.0; see
/// `default_scallop_intra_pass_hookup_mm`.
const CANDIDATE_HOOKUP_MM: f64 = 3.0;

/// A corrugated plate: many short rings with near-touching ends, which is
/// exactly the topology where per-ring retracts dominate. Not wanaka.
fn corrugated_session(hookup_mm: f64) -> ProjectSession {
    let mesh = sawtooth_plate(20.0, 6.0, 2.0);
    let mut session = single_op_session_with(
        stock_over(20.0, 6.0),
        ball_tool_config(BALL_DIAMETER_MM),
        mesh_model(mesh, "corrugated"),
        "Scallop",
        OperationConfig::Scallop(ScallopConfig {
            scallop_height: 0.1,
            tolerance: 0.05,
            continuous: false,
            intra_pass_hookup_mm: hookup_mm,
            ..ScallopConfig::default()
        }),
        |_| {},
    );
    generate(&mut session, 0);
    session
}

/// One trip = one maximal contiguous run of `Rapid` moves. Same rule as
/// `compute::stats::compute_retract_trips` and `v3_cascade_ab`.
fn retract_trips(moves: &[Move]) -> usize {
    let mut trips = 0;
    let mut in_run = false;
    for m in moves {
        let is_rapid = matches!(m.move_type, MoveType::Rapid);
        if is_rapid && !in_run {
            trips += 1;
        }
        in_run = is_rapid;
    }
    trips
}

/// The RING geometry — the surface the operation exists to cut.
///
/// Deliberately `FinishingCut` only, not "every non-rapid move". A surface
/// link legitimately REPLACES the `EntryPlunge` that used to drop into each
/// ring, so asserting over all feed moves would flag the conversion's whole
/// purpose as a defect. What must not move is the cut itself.
fn ring_moves(moves: &[Move]) -> Vec<Move> {
    moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid) && m.intent == MoveIntent::FinishingCut)
        .cloned()
        .collect()
}

/// Every feed move, whatever its intent — the "after" population for the
/// identity check. See its call site for why the label is not the invariant.
fn fed_moves(moves: &[Move]) -> Vec<Move> {
    moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .cloned()
        .collect()
}

fn collisions_at(session: &mut ProjectSession, resolution: f64) -> usize {
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution,
        metrics_enabled: true,
        auto_resolution: false,
        ..SimulationOptions::default()
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");
    session
        .diagnostics()
        .per_toolpath
        .iter()
        .find(|d| d.toolpath_id == ToolpathId(0))
        .map_or(0, |d| d.rapid_collision_count)
}

#[test]
fn the_relink_drops_one_cut_position_per_link_which_is_why_it_ships_off() {
    let off = corrugated_session(0.0);
    let on = corrugated_session(CANDIDATE_HOOKUP_MM);

    let off_tp = off.get_result(0).expect("baseline generated");
    let on_tp = on.get_result(0).expect("relinked generated");
    let off_moves = &off_tp.toolpath().moves;
    let on_moves = &on_tp.toolpath().moves;

    let off_cut = ring_moves(off_moves);
    // The "after" side accepts ANY feed move, not just `FinishingCut`.
    // `relink_fragments` re-emits each fragment's first move as the tail of
    // the link that replaced its plunge, so a handful of positions change
    // INTENT from `FinishingCut` to `Linking`. They are still fed, still at
    // the surface, still cutting — the label moved, the cut did not. The
    // invariant that matters is that no cut POSITION is lost, which is what
    // this asserts.
    let on_cut = fed_moves(on_moves);

    // The load-bearing safety assertion. A surface link is a FEED move, so
    // the relinked path legitimately has MORE cutting moves — but every
    // move that was cutting before must still be there, unchanged and in
    // order. Fragment interiors are copied verbatim; only junctions differ.
    println!(
        "A/M7 identity: {} ring moves -> {} fed moves ({} rapid runs -> {}); \
         FinishingCut-labelled {} -> {}",
        off_cut.len(),
        on_cut.len(),
        retract_trips(off_moves),
        retract_trips(on_moves),
        off_cut.len(),
        ring_moves(on_moves).len()
    );
    assert!(
        on_cut.len() >= off_cut.len(),
        "a relink may add link feeds, never remove cut positions: {} -> {}",
        off_cut.len(),
        on_cut.len()
    );
    // Positional set-membership, order-independent: how many baseline cut
    // positions have no counterpart at all after the relink.
    let mut lost = Vec::new();
    for want in &off_cut {
        let found = on_cut.iter().any(|got| {
            (got.target.x - want.target.x).abs() < 1e-9
                && (got.target.y - want.target.y).abs() < 1e-9
                && (got.target.z - want.target.z).abs() < 1e-9
        });
        if !found {
            lost.push(want.target);
        }
    }
    let links = retract_trips(off_moves) - retract_trips(on_moves);
    println!(
        "A/M7 lost cut positions: {} of {} across {links} converted junctions",
        lost.len(),
        off_cut.len()
    );
    for p in lost.iter().take(4) {
        println!("  lost {p:?}");
    }

    // THE BLOCKER, pinned rather than papered over.
    //
    // `relink_fragments` drops EXACTLY ONE cut position per link it makes —
    // 21 losses across 21 converted junctions, never 20 and never 22. That
    // is not a rounding artifact; it is an off-by-one in the re-emit, which
    // re-emits `entry` to stand in for `frag.moves[0]` and then resumes the
    // fragment past a move that was carrying geometry.
    //
    // It is why `intra_pass_hookup_mm` ships DEFAULT OFF despite an A/B
    // this strong (see the other test: −87.5% trips, −59.3% seconds,
    // +144.9% mm²/s, zero new collisions at 0.1 mm). A gap in a finished
    // surface is not purchasable with wall-clock.
    //
    // This matters beyond scallop: the same function backs
    // `unified_finish::intra_region_hookup_mm`, which is shipped (also
    // default-off) and would carry the same loss the moment anyone enables
    // it. Whoever fixes the re-emit gets this test as the target: change
    // the expectation to zero, and the default to 3.0.
    assert_eq!(
        lost.len(),
        links,
        "A/M7: the relink loses exactly one cut position per link — {} lost \
         across {links} links. If this ratio changed, the defect changed: \
         re-diagnose before touching the number.",
        lost.len()
    );
}

#[test]
fn the_ab_that_would_justify_the_default_if_the_relinker_were_sound() {
    let off = corrugated_session(0.0);
    let on = corrugated_session(CANDIDATE_HOOKUP_MM);

    let (off_trips, on_trips, off_area, on_area, off_s, on_s) = {
        let o = off.get_result(0).expect("baseline generated");
        let n = on.get_result(0).expect("relinked generated");
        let (oa, prov) = swept_footprint_area(
            &o.toolpath().moves,
            BALL_DIAMETER_MM * 0.5,
            DEFAULT_FOOTPRINT_CELL_MM,
        );
        let (na, _) = swept_footprint_area(
            &n.toolpath().moves,
            BALL_DIAMETER_MM * 0.5,
            DEFAULT_FOOTPRINT_CELL_MM,
        );
        println!("A/M7 footprint provenance: {prov}");
        (
            retract_trips(&o.toolpath().moves),
            retract_trips(&n.toolpath().moves),
            oa,
            na,
            cycle_time(o.toolpath()),
            cycle_time(n.toolpath()),
        )
    };

    let off_rate = swept_footprint_mm2_per_s(off_area, off_s);
    let on_rate = swept_footprint_mm2_per_s(on_area, on_s);

    println!(
        "A/M7 A/B (all-over scallop, corrugated Ø{BALL_DIAMETER_MM} ball)\n  \
         trips   {off_trips} -> {on_trips}  ({:+.1}%)\n  \
         seconds {off_s:.2} -> {on_s:.2}  ({:+.1}%)\n  \
         area    {off_area} -> {on_area}\n  \
         mm2/s   {off_rate:.4} -> {on_rate:.4}  ({:+.1}%)",
        pct(off_trips as f64, on_trips as f64),
        pct(off_s, on_s),
        pct(off_rate, on_rate),
    );

    assert!(
        on_trips < off_trips,
        "A/M7: the conversion must remove retract round trips; {off_trips} -> {on_trips}"
    );
    // Same geometry, so the numerator must not move. If it does, the relink
    // changed what gets cut and the ratio below would be comparing two
    // different surfaces.
    assert!(
        (off_area.mm2() - on_area.mm2()).abs() / off_area.mm2().max(1.0) < 0.02,
        "A/M7: swept footprint must be materially unchanged ({off_area} -> \
         {on_area}); a moved numerator means the relink altered coverage"
    );
    assert!(
        on_rate > off_rate,
        "A/M7 gate: mm²/s must improve on the all-over pass; \
         {off_rate:.4} -> {on_rate:.4}"
    );
}

#[test]
fn no_new_collisions_at_the_finest_resolution() {
    // The gate says FINEST, not default — and A/M10 is what makes that a
    // meaningful thing to ask, because before it the count was a property
    // of the grid rather than of the path.
    let mut off = corrugated_session(0.0);
    let mut on = corrugated_session(CANDIDATE_HOOKUP_MM);

    let off_c = collisions_at(&mut off, 0.1);
    let on_c = collisions_at(&mut on, 0.1);
    println!("A/M7 collisions @0.1mm: {off_c} -> {on_c}");

    assert!(
        on_c <= off_c,
        "A/M7 gate: the relink must introduce no new collisions at the \
         finest resolution; {off_c} -> {on_c} at 0.1 mm"
    );
}

/// F-034 integrator against one fixed envelope for both branches — the
/// denominator §7 specifies: per-op seconds INCLUDING rapids and entries.
fn cycle_time(tp: &rs_cam_core::toolpath::Toolpath) -> f64 {
    compute_cycle_time(tp, &MachineKinematics::default(), 3000.0, 6000.0)
}

fn pct(from: f64, to: f64) -> f64 {
    if from.abs() < 1e-12 {
        0.0
    } else {
        (to - from) / from * 100.0
    }
}
