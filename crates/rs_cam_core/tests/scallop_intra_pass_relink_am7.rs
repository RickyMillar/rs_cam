//! A/M7 — the A/B for ring-to-ring surface links, and the gates that decide
//! whether they ship.
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
//! ## The gate that fired, and what it was actually measuring
//!
//! Wave 11 blocked this conversion on a third gate: `relink_fragments`
//! appeared to drop **exactly one cut position per link**, 21 across 21
//! junctions. That reading was wrong, and the way it was wrong is the
//! durable lesson here.
//!
//! Wave 11 had already corrected the gate once — from comparing
//! `FinishingCut`-*labelled* moves (2 cosmetic hits) to comparing cut
//! POSITIONS (21 apparently real ones). What it did not correct is that the
//! POPULATION was still selected by that label. And `arcfit::fit_arcs`
//! groups a run by FEED RATE, not by intent, taking the collapsed arc's
//! intent from its first source move — so a run of `FinishingCut` moves
//! followed by the `LeadOut` arc that `apply_lead_in_out` had just appended
//! collapses into ONE arc labelled `FinishingCut`, whose target is the
//! lead-out's endpoint, up to 1.2 mm off the machined surface.
//!
//! All 21 "lost cut positions" are exactly that: `LeadOut` endpoints
//! (measured — see `the_positions_the_relink_removes_are_lead_outs`). And
//! one lead-out disappears per link BY CONSTRUCTION, because a link removes
//! a fragment boundary and a fragment boundary is what a lead-out
//! terminates. The suspiciously exact 1:1 ratio was that arithmetic, not an
//! off-by-one.
//!
//! Bisected three ways, all reported by this file's sentries and by
//! `surface_link`'s own unit tests: the relinker alone loses nothing, the
//! whole pipeline with dressups off loses nothing, and every dressup
//! individually loses nothing. Only `arc_fitting` + `lead_in_out` together
//! produce the phantom.
//!
//! **The rule earned**: when a gate selects its population by a LABEL, it
//! inherits every relabelling any transform downstream performs. Assert on
//! what reaches the workpiece — here, membership of the machined SURFACE,
//! checked with the drop cutter that defines it.
//!
//! ## What the gates must not let through
//!
//! * **no cut position may be lost** — `relink_fragments` copies fragment
//!   INTERIORS verbatim and replaces only airborne junctions. Pinned both
//!   structurally (dressup-free, exact set membership) and semantically
//!   (whatever the production stack does drop is off-surface lead geometry);
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
//! Both branches cut the same geometry, so the swept footprint barely moves
//! (1910 → 1906 mm², the removed lead-out excursions) and mm²/s improves
//! almost entirely because the DENOMINATOR falls. That is stated here so
//! nobody reads the ratio as "more area finished". The numerator is a
//! footprint, not fresh area: it never consults the stock.

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

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::DressupConfig;
use rs_cam_core::compute::operation_configs::ScallopConfig;
use rs_cam_core::dropcutter::point_drop_cutter;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine_kinematics::{MachineKinematics, compute_cycle_time};
use rs_cam_core::measurement::{
    DEFAULT_FOOTPRINT_CELL_MM, swept_footprint_area, swept_footprint_mm2_per_s,
};
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::session::SimulationOptions;
use rs_cam_core::tool::BallEndmill;
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType};

use std::sync::atomic::AtomicBool;

/// Ø3 ball — the tool the scallop op requires a ball tip for, and the one
/// the shipped hookup default is sized against (a little over one diameter).
const BALL_DIAMETER_MM: f64 = 3.0;

/// The hookup the A/B is measured at — and, since wave 12, the shipped
/// default. Read from the config so a change to one moves the other.
fn candidate_hookup_mm() -> f64 {
    ScallopConfig::default().intra_pass_hookup_mm
}

fn corrugated_session_with(hookup_mm: f64, dressups: Option<DressupConfig>) -> ProjectSession {
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
        |cfg| {
            if let Some(d) = dressups {
                cfg.dressups = d;
            }
        },
    );
    generate(&mut session, 0);
    session
}

/// A corrugated plate: many short rings with near-touching ends, which is
/// exactly the topology where per-ring retracts dominate. Not wanaka.
fn corrugated_session(hookup_mm: f64) -> ProjectSession {
    corrugated_session_with(hookup_mm, None)
}

/// The production dressup stack with the two transforms that RELABEL
/// geometry switched off, so a position comparison means what it says.
fn dressups_without_relabelling() -> DressupConfig {
    DressupConfig {
        arc_fitting: false,
        lead_in_out: false,
        ..DressupConfig::for_op(OperationType::Scallop)
    }
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

/// The RING geometry as the production path LABELS it.
///
/// Deliberately `FinishingCut` only, not "every non-rapid move". A surface
/// link legitimately REPLACES the `EntryPlunge` that used to drop into each
/// ring, so asserting over all feed moves would flag the conversion's whole
/// purpose as a defect.
///
/// Read the module docs before trusting this population: after arc fitting,
/// the label also covers lead geometry.
fn ring_moves(moves: &[Move]) -> Vec<Move> {
    moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid) && m.intent == MoveIntent::FinishingCut)
        .cloned()
        .collect()
}

/// Every feed move, whatever its intent — the "after" population. A link is
/// a cutting feed, so the relinked path legitimately has MORE of them.
fn fed_moves(moves: &[Move]) -> Vec<Move> {
    moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .cloned()
        .collect()
}

fn same_position(a: &Move, b: &Move) -> bool {
    (a.target.x - b.target.x).abs() < 1e-9
        && (a.target.y - b.target.y).abs() < 1e-9
        && (a.target.z - b.target.z).abs() < 1e-9
}

/// Baseline positions with no counterpart at all after the relink.
/// Set-membership, order-independent: a reordering passes, a hole does not.
fn positions_lost(before: &[Move], after: &[Move]) -> Vec<Move> {
    before
        .iter()
        .filter(|want| !after.iter().any(|got| same_position(got, want)))
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

/// THE STRUCTURAL GATE. With the two relabelling dressups off, the
/// comparison is between the relinker's own input and output populations,
/// and it must be exact: a relink rewrites junctions, so every position the
/// tool fed to before is a position it feeds to after.
///
/// This is the assertion wave 11 believed was failing. It was not; the
/// population it was reading had been contaminated one transform later.
#[test]
fn the_relink_loses_no_cut_position() {
    let off = corrugated_session_with(0.0, Some(dressups_without_relabelling()));
    let on = corrugated_session_with(candidate_hookup_mm(), Some(dressups_without_relabelling()));

    let off_tp = off.get_result(0).expect("baseline generated");
    let on_tp = on.get_result(0).expect("relinked generated");
    let off_cut = ring_moves(&off_tp.toolpath().moves);
    let on_cut = fed_moves(&on_tp.toolpath().moves);
    let lost = positions_lost(&off_cut, &on_cut);

    let links = retract_trips(&off_tp.toolpath().moves) - retract_trips(&on_tp.toolpath().moves);
    println!(
        "A/M7 structural: {} cut positions -> {} fed positions across {links} \
         converted junctions; lost {}",
        off_cut.len(),
        on_cut.len(),
        lost.len()
    );
    assert!(links > 0, "the fixture must actually convert junctions");
    assert!(
        on_cut.len() >= off_cut.len(),
        "a relink may add link feeds, never remove cut positions: {} -> {}",
        off_cut.len(),
        on_cut.len()
    );
    assert!(
        lost.is_empty(),
        "A/M7: the relink must not drop a single cut position — {} lost, \
         first at {:?}. Fragment interiors are copied verbatim; if this \
         fires, the re-emit changed.",
        lost.len(),
        lost.first().map(|m| m.target)
    );
}

/// THE SEMANTIC GATE, on the FULL production stack. Positions do disappear
/// there — one per link — and this pins what they are, so the day one of
/// them is a real cut the test fails instead of being re-baselined.
///
/// Two independent adjudications, because the point of wave 12 is that the
/// label alone is not evidence:
///
/// * pre-arc-fit intent: every disappearing position is a `LeadOut`;
/// * geometry: every disappearing position sits OFF the drop-cutter
///   surface, while the surviving cut positions sit exactly on it.
#[test]
fn the_positions_the_relink_removes_are_lead_outs() {
    let off = corrugated_session(0.0);
    let on = corrugated_session(candidate_hookup_mm());
    // Same baseline with the arc fitter off: the un-collapsed path still
    // carries each move's ORIGINAL intent.
    let unfitted = corrugated_session_with(
        0.0,
        Some(DressupConfig {
            arc_fitting: false,
            ..DressupConfig::for_op(OperationType::Scallop)
        }),
    );

    let off_tp = off.get_result(0).expect("baseline generated");
    let on_tp = on.get_result(0).expect("relinked generated");
    let raw_tp = unfitted
        .get_result(0)
        .expect("un-fitted baseline generated");

    let off_cut = ring_moves(&off_tp.toolpath().moves);
    let on_cut = fed_moves(&on_tp.toolpath().moves);
    let lost = positions_lost(&off_cut, &on_cut);
    let links = retract_trips(&off_tp.toolpath().moves) - retract_trips(&on_tp.toolpath().moves);

    println!(
        "A/M7 production: {} labelled cut positions, {links} links, {} \
         disappear",
        off_cut.len(),
        lost.len()
    );

    // (1) Every one of them is a lead-out before the arc fitter relabels it.
    let mut intents: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut unmatched = 0usize;
    for m in &lost {
        match raw_tp.toolpath().moves.iter().find(|r| same_position(r, m)) {
            Some(r) => *intents.entry(format!("{:?}", r.intent)).or_default() += 1,
            None => unmatched += 1,
        }
    }
    println!("A/M7 pre-arc-fit intents of the disappearing positions: {intents:?}");
    assert_eq!(unmatched, 0, "every position must exist un-fitted too");
    assert_eq!(
        intents.get("LeadOut").copied().unwrap_or(0),
        lost.len(),
        "A/M7: everything the relink removes must be LEAD-OUT geometry — a \
         link deletes a fragment boundary, and a lead-out is what terminates \
         one. Anything else in {intents:?} is a real cut going missing."
    );
    assert_eq!(
        lost.len(),
        links,
        "one lead-out per removed fragment boundary, by construction"
    );

    // (2) Geometry, independent of any label: the removed positions are not
    // on the surface being machined; the surviving ones are.
    let mesh = sawtooth_plate(20.0, 6.0, 2.0);
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = BallEndmill::new(BALL_DIAMETER_MM, 25.0);
    let dz = |m: &Move| -> Option<f64> {
        let cl = point_drop_cutter(m.target.x, m.target.y, &mesh, &index, &tool);
        cl.contacted.then(|| m.target.z - cl.z)
    };

    let survivors: Vec<&Move> = off_cut
        .iter()
        .filter(|m| !lost.iter().any(|l| same_position(l, m)))
        .collect();
    let on_surface = survivors
        .iter()
        .filter_map(|m| dz(m))
        .filter(|d| d.abs() <= 1e-6)
        .count();
    println!(
        "A/M7 surface check: {} of {} surviving cut positions sit exactly on \
         the drop-cutter surface",
        on_surface,
        survivors.len()
    );
    assert!(
        on_surface * 10 >= survivors.len() * 9,
        "control: the surviving population must be surface geometry"
    );
    for m in &lost {
        let d = dz(m).expect("lost positions are over the mesh");
        assert!(
            d.abs() > 1e-6,
            "A/M7: a removed position sitting ON the machined surface is a \
             HOLE, not a lead-out — {:?} at dz={d:.6}",
            m.target
        );
    }
}

#[test]
fn the_ab_that_justifies_the_default() {
    let off = corrugated_session(0.0);
    let on = corrugated_session(candidate_hookup_mm());

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
    let mut on = corrugated_session(candidate_hookup_mm());

    let off_c = collisions_at(&mut off, 0.1);
    let on_c = collisions_at(&mut on, 0.1);
    println!("A/M7 collisions @0.1mm: {off_c} -> {on_c}");

    assert!(
        on_c <= off_c,
        "A/M7 gate: the relink must introduce no new collisions at the \
         finest resolution; {off_c} -> {on_c} at 0.1 mm"
    );
}

/// The dial ships ON since wave 12. Pinned here rather than left implicit,
/// because the value is load-bearing for every test above (they read it) and
/// because turning it back off is a decision that should break a test.
#[test]
fn the_shipped_default_is_on() {
    assert!(
        (ScallopConfig::default().intra_pass_hookup_mm - 3.0).abs() < 1e-9,
        "A/M7: the shipped hookup is 3.0 mm — a little over one Ø3-ball \
         diameter: far enough to catch adjacent-ring junctions, short enough \
         that a link never crosses a feature it did not machine"
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
