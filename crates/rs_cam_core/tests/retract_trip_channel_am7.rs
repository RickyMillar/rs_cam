//! A/M7 gate 1 sentry — `ToolpathStats` must carry a retract round-trip
//! COUNT channel, split by in-routing-node vs between-nodes.
//!
//! Background (`planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`
//! §A/M7): air cost in finishing is COUNT-bound, not distance-bound — a hop
//! pays two ~`safe_z` Z legs whatever its XY length. Before this channel,
//! `ToolpathStats` carried only `rapid_distance` (millimetres), so no gate,
//! GUI surface, or MCP field could see the number that actually matters. The
//! v3 process-proof campaign measured 15 311 of 15 363 trips landing INSIDE
//! a single routing node on a real job — which is why the in/out split has
//! to travel WITH the count rather than being left to a reader to derive.
//!
//! The counting rule is copied — not reinvented — from
//! `tests/v3_cascade_ab.rs`'s `rapid_round_trips` helper: a "trip" is one
//! maximal contiguous run of `MoveType::Rapid` moves, and a run is
//! classified by whether its FIRST move sits inside a planner territory
//! `Region` node (`RegionSpanRole::Node`). See
//! `crate::compute::stats::compute_retract_trips`, which reproduces that
//! rule bit-for-bit so the production channel and that harness can never
//! disagree.
//!
//! Four things are pinned here:
//!
//! (a) an operation that retracts between passes reports a non-zero total;
//! (b) `None` is reachable and means "never measured" — at the whole-field
//!     level (a stats struct that never walked a move list) AND at the
//!     split level (spans were absent/untrusted) — never a fabricated zero
//!     (`MEASUREMENT_DOMAINS.md` X-19);
//! (c) the in-node + between-node split sums to the total when spans are
//!     valid;
//! (d) the production count matches an INDEPENDENT in-test counter walking
//!     the same emitted moves — an oracle, not a restatement of
//!     `compute_retract_trips`'s own logic.
//!
//! Report-only by design: nothing gates on this figure and no verdict moves.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::{StockConfig, ToolpathStats, compute_retract_trips};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::{MoveType, Toolpath};
use rs_cam_core::toolpath_spans::{RegionSpanRole, Span, SpanKind, SpanPayload};

use common::session::{generate, mesh_model, pinned_heights, single_op_session_with};

// ── Unit-level fixtures (no session, no mesh) ──────────────────────────────
//
// A small, hand-traced move list: two retract trips whose move indices and
// distances are known by inspection, so the expected `RetractTripCount` can
// be asserted exactly rather than re-derived.
//
// idx  move            kind
//  0   feed (0,0,0)     cutting
//  1   rapid (1,0,0)    trip A start
//  2   rapid (2,0,0)    trip A continues
//  3   feed (3,0,0)     cutting — ends trip A
//  4   feed (4,0,0)     cutting
//  5   rapid (5,0,0)    trip B (single move)
//  6   feed (6,0,0)     cutting — ends trip B
//
// Trip A starts at move 1 (1 mm + 1 mm = 2 mm of rapid); trip B starts at
// move 5 (1 mm of rapid).
fn two_trip_toolpath() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.feed_to(P3::new(0.0, 0.0, 0.0), 500.0);
    tp.rapid_to(P3::new(1.0, 0.0, 0.0));
    tp.rapid_to(P3::new(2.0, 0.0, 0.0));
    tp.feed_to(P3::new(3.0, 0.0, 0.0), 500.0);
    tp.feed_to(P3::new(4.0, 0.0, 0.0), 500.0);
    tp.rapid_to(P3::new(5.0, 0.0, 0.0));
    tp.feed_to(P3::new(6.0, 0.0, 0.0), 500.0);
    tp
}

/// A single `Region` node span covering moves `[0, 3)` — trip A's start
/// move (1) sits inside it, trip B's start move (5) does not.
fn node_span_covering_trip_a() -> Span {
    Span::new(0, 3, SpanKind::Region).with_payload(SpanPayload::Region {
        region_id: 0,
        role: RegionSpanRole::Node,
    })
}

/// (b), split half: the total is measured even when the caller supplies no
/// spans — the split alone reads as unmeasured, never as a confident zero.
#[test]
fn total_is_measured_even_when_the_split_is_not() {
    let tp = two_trip_toolpath();

    let unsplit = compute_retract_trips(&tp, None);
    assert_eq!(unsplit.total, 2, "two contiguous rapid runs, spans or not");
    assert_eq!(
        unsplit.in_node, None,
        "no spans supplied — the split must read as unmeasured, not zero"
    );
    assert_eq!(unsplit.between_nodes, None);
    assert_eq!(unsplit.in_node_rapid_mm, None);
    assert_eq!(unsplit.between_nodes_rapid_mm, None);
    assert!(!unsplit.has_split());
}

/// (c): with a trustworthy node span supplied, the split classifies each
/// trip by its FIRST move and sums back to the total — checked against
/// hand-computed expectations, not just internal consistency.
#[test]
fn split_classifies_by_first_move_and_sums_to_total() {
    let tp = two_trip_toolpath();
    let spans = [node_span_covering_trip_a()];

    let split = compute_retract_trips(&tp, Some(spans.as_slice()));
    assert_eq!(split.total, 2);
    assert!(split.has_split());
    assert_eq!(split.in_node, Some(1), "trip A starts inside the node span");
    assert_eq!(
        split.between_nodes,
        Some(1),
        "trip B starts outside the node span"
    );
    assert_eq!(
        split.in_node.unwrap() + split.between_nodes.unwrap(),
        split.total,
        "the split must sum to the total"
    );
    assert_eq!(split.in_node_rapid_mm, Some(2.0), "trip A covers 1mm + 1mm");
    assert_eq!(split.between_nodes_rapid_mm, Some(1.0), "trip B covers 1mm");
}

/// (b), whole-field half: a stats struct that never walked a move list
/// (the shape of `ToolpathComputeResult` placeholders and
/// `ToolpathStats::default()`) reports `None`, never `Some(RetractTripCount
/// { total: 0, .. })` — the two are different claims.
#[test]
fn default_stats_report_retract_trips_as_not_measured() {
    let stats = ToolpathStats::default();
    assert_eq!(stats.retract_trips, None);
    assert_eq!(
        stats.retract_trip_measurement(),
        None,
        "the typed accessor must mirror the raw field"
    );
}

// ── End-to-end fixture: a multi-node UnifiedFinish pass ───────────────────
//
// A 30 mm plateau cut by two grooves of different wall slope, which
// UnifiedFinish's slope classifier splits into (at least) a Shallow raster
// node and steeper scallop/waterline groove nodes — proven multi-node
// shape, copied from `unified_finish_tapered_end_to_end_m21.rs` (same
// profile, same tool, same dials) rather than reinvented, since this file's
// job is the retract-trip channel, not re-litigating band classification.

fn extrude_profile(profile: &[(f64, f64)], y0: f64, y1: f64) -> TriangleMesh {
    let mut vertices = Vec::with_capacity(profile.len() * 2);
    for &(x, z) in profile {
        vertices.push(P3::new(x, y0, z));
        vertices.push(P3::new(x, y1, z));
    }
    let mut triangles = Vec::with_capacity((profile.len() - 1) * 2);
    for i in 0..profile.len() - 1 {
        let (a, b) = (2 * i as u32, 2 * i as u32 + 2);
        let (c, d) = (2 * i as u32 + 3, 2 * i as u32 + 1);
        triangles.push([a, b, c]);
        triangles.push([a, c, d]);
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// tan(60°) = 1.7320508, tan(85°) = 11.430052 — same profile as the M21
/// two-groove-plateau fixture.
fn two_groove_plateau() -> TriangleMesh {
    let profile = [
        (-15.0_f64, 0.0_f64),
        (-6.0, 0.0),
        (-5.0, -1.732_050_8),
        (-4.0, 0.0),
        (4.0, 0.0),
        (4.75, -8.572_539),
        (5.5, 0.0),
        (15.0, 0.0),
    ];
    extrude_profile(&profile, -15.0, 15.0)
}

fn tapered_ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: 1.0,
        taper_half_angle: 7.0,
        shaft_diameter: 6.0,
        ..ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose)
    }
}

fn unified_finish_op() -> OperationConfig {
    OperationConfig::UnifiedFinish(UnifiedFinishConfig {
        steep_threshold_deg: 45.0,
        waterline_threshold_deg: 75.0,
        overlap_mm: 0.0,
        tolerance: 0.05,
        sampling: 0.5,
        scallop_height: 0.15,
        raster_stepover: 1.5,
        z_step: 1.5,
        ..UnifiedFinishConfig::default()
    })
}

fn stock() -> StockConfig {
    StockConfig {
        x: 34.0,
        y: 34.0,
        z: 9.0,
        origin_x: -17.0,
        origin_y: -17.0,
        origin_z: -9.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn multi_node_session() -> ProjectSession {
    session_with_hookup(UnifiedFinishConfig::default().intra_region_hookup_mm)
}

fn session_with_hookup(intra_region_hookup_mm: f64) -> ProjectSession {
    let OperationConfig::UnifiedFinish(base) = unified_finish_op() else {
        panic!("fixture builds a UnifiedFinish op")
    };
    single_op_session_with(
        stock(),
        tapered_ball_tool(),
        mesh_model(two_groove_plateau(), "two_groove_plateau"),
        "Unified Finish",
        OperationConfig::UnifiedFinish(UnifiedFinishConfig {
            intra_region_hookup_mm,
            ..base
        }),
        |cfg| {
            // Pinned, not Auto: a surface op carries no depth dial, so
            // `bottom_z: Auto` would resolve to `top_z - 0.0` and collapse
            // every band's Z range.
            cfg.heights = pinned_heights(0.0, -9.0);
        },
    )
}

/// Independent oracle: count contiguous `MoveType::Rapid` runs by looking
/// for RISING EDGES in an `is_rapid` bitmap (a different algorithm from
/// `compute_retract_trips`'s run-start/run-end state machine), so this is a
/// genuine second opinion, not the same code read twice.
fn independent_round_trip_oracle(moves: &[rs_cam_core::toolpath::Move]) -> usize {
    let is_rapid: Vec<bool> = moves
        .iter()
        .map(|m| matches!(m.move_type, MoveType::Rapid))
        .collect();
    is_rapid
        .iter()
        .enumerate()
        .filter(|&(i, &rapid)| rapid && (i == 0 || !is_rapid[i - 1]))
        .count()
}

/// The end-to-end acceptance gate: (a) non-zero total on a real generation
/// through `ProjectSession::generate_toolpath` (the entry point the GUI
/// worker and the CLI share), (c) the split sums to the total, (d) an
/// independent oracle agrees with the production count, plus the accessor,
/// its provenance, and narration surfacing.
#[test]
fn multi_node_op_reports_nonzero_trips_with_a_trustworthy_split() {
    let mut session = multi_node_session();
    generate(&mut session, 0);

    let result = session.get_result(0).expect("generated result");
    let annotated = result.annotated();

    // Fixture sanity: the split assertions below are vacuous if this
    // fixture didn't actually produce multiple routing nodes.
    let node_count = annotated
        .spans
        .iter()
        .filter(|s| s.is_region_node())
        .count();
    assert!(
        node_count >= 2,
        "fixture must decompose into multiple routing nodes for the split \
         test to mean anything; got {node_count}"
    );
    assert!(
        annotated.spans_valid,
        "the split assertions below require trustworthy spans"
    );

    let trips = result
        .stats
        .retract_trips
        .expect("A/M7: a real generation must measure retract trips");

    // (a) non-zero total.
    assert!(
        trips.total > 0,
        "a multi-node finishing pass must retract between at least two \
         nodes; got 0 trips"
    );

    // (d) independent oracle over the SAME shipped moves.
    let oracle_total = independent_round_trip_oracle(&annotated.toolpath.moves);
    assert_eq!(
        trips.total, oracle_total,
        "the production count must match an independently-written walk \
         over the emitted moves"
    );

    // (c) the split sums to the total.
    let (in_n, out_n) = trips
        .in_node
        .zip(trips.between_nodes)
        .expect("spans_valid == true, so the split must be Some");
    assert_eq!(in_n + out_n, trips.total, "split must sum to the total");
    println!(
        "retract trips: {} total ({in_n} in-node, {out_n} between-nodes)",
        trips.total
    );

    // The typed accessor must mirror the raw field and carry the right
    // measurement contract.
    let (measured, provenance) = result
        .stats
        .retract_trip_measurement()
        .expect("accessor must mirror `Some` fields");
    assert_eq!(measured, trips);
    assert_eq!(
        provenance.domain,
        rs_cam_core::measurement::MeasurementDomain::RetractTripCount
    );
    assert_eq!(
        provenance.stage,
        rs_cam_core::measurement::MeasurementStage::Emission
    );

    // Narration — the agent-facing surface — must carry the figure.
    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Retract trips:"),
        "narration must carry a retract-trips line:\n{narration}"
    );
    assert!(
        narration.contains(&format!("{} round trip", trips.total)),
        "narration must print the measured total itself, not just a label:\n{narration}"
    );
}

// ── Report-only: the OTHER dial the relinker backs ────────────────────────

/// `UnifiedFinishConfig::intra_region_hookup_mm` is the second consumer of
/// `surface_link::relink_fragments`, and the second dial in this programme
/// shipped default-off ahead of the A/B that would have judged it.
///
/// Wave 12 cleared the reason wave 11 gave for leaving BOTH off — the
/// relinker does not drop cut positions — measured the number, and still
/// declined to flip it, because a region-level trade depends on how the
/// planner's router links regions AFTERWARDS and that belongs to an operator
/// with a real part in front of them, not to a synthetic two-groove plateau.
///
/// **Wave 14: that operator was asked, and said ON at 6.0.** The ruling was
/// made in the Checkpoint D session (2026-08-03) with wave 12's measured
/// table as the evidence, and is re-checked at the end-of-programme live
/// validation. `default_unified_finish_intra_region_hookup_mm` is now 6.0, so
/// this test changed job: it no longer documents a deferral, it holds the
/// shipped default and the safety invariant that travels with it.
///
/// The invariant is unchanged and is the whole point: **keeping the tool down
/// must never ADD retract round trips.** The A/B is still run here rather than
/// asserted from the default, because the comparison is what makes the
/// invariant meaningful.
#[test]
fn intra_region_hookup_ships_on_by_operator_ruling() {
    let off = {
        let mut s = session_with_hookup(0.0);
        generate(&mut s, 0);
        s
    };
    let on = {
        let mut s = session_with_hookup(6.0);
        generate(&mut s, 0);
        s
    };

    let o = off.get_result(0).expect("hookup-off generated");
    let n = on.get_result(0).expect("hookup-on generated");
    let (o_moves, n_moves) = (&o.toolpath().moves, &n.toolpath().moves);

    let o_trips = compute_retract_trips(o.toolpath(), None).total;
    let n_trips = compute_retract_trips(n.toolpath(), None).total;
    let kin = rs_cam_core::machine_kinematics::MachineKinematics::default();
    let o_s =
        rs_cam_core::machine_kinematics::compute_cycle_time(o.toolpath(), &kin, 3000.0, 6000.0);
    let n_s =
        rs_cam_core::machine_kinematics::compute_cycle_time(n.toolpath(), &kin, 3000.0, 6000.0);
    let (o_area, _) = rs_cam_core::measurement::swept_footprint_area(
        o_moves,
        0.5,
        rs_cam_core::measurement::DEFAULT_FOOTPRINT_CELL_MM,
    );
    let (n_area, _) = rs_cam_core::measurement::swept_footprint_area(
        n_moves,
        0.5,
        rs_cam_core::measurement::DEFAULT_FOOTPRINT_CELL_MM,
    );
    let o_rate = rs_cam_core::measurement::swept_footprint_mm2_per_s(o_area, o_s);
    let n_rate = rs_cam_core::measurement::swept_footprint_mm2_per_s(n_area, n_s);

    println!(
        "A/M7 report-only — unified_finish intra_region_hookup_mm 0.0 -> 6.0\n  \
         moves   {} -> {}\n  \
         trips   {o_trips} -> {n_trips}\n  \
         seconds {o_s:.2} -> {n_s:.2}\n  \
         area    {o_area} -> {n_area}\n  \
         mm2/s   {o_rate:.4} -> {n_rate:.4}",
        o_moves.len(),
        n_moves.len(),
    );

    assert!(
        n_trips <= o_trips,
        "keeping the tool down must not ADD retract round trips: \
         {o_trips} -> {n_trips}"
    );

    // The shipped default IS the `on` arm. Pinned here so a silent revert to
    // 0.0 — or a drift between the serde default and the core one — fails
    // with the reason attached rather than quietly halving the feed rate.
    assert_eq!(
        UnifiedFinishConfig::default().intra_region_hookup_mm,
        6.0,
        "operator ruling (Checkpoint D session, 2026-08-03): the region-level \
         hookup ships ON at 6.0 mm"
    );
    assert_eq!(
        rs_cam_core::unified_finish::UnifiedFinishParams::default().intra_region_hookup_mm,
        UnifiedFinishConfig::default().intra_region_hookup_mm,
        "the core default and the serde default must agree — a config layer \
         that disagrees with its own core is this programme's recurring \
         divergence class"
    );
}
