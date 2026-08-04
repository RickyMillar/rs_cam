//! **PR-6 / Checkpoint F1 Q3 — the measured arc-count cost of the intent
//! run-key term, and of treating `MoveIntent::Unknown` as STRICT.**
//!
//! Q3 was ruled strict on the condition that PR-6 measure the cost rather
//! than argue it. This is that measurement, pinned so it cannot silently
//! drift.
//!
//! Five fixtures run the SHIPPED dressup chain. For each, the file pins
//! `(moves, arcs)` after the chain, plus a **seam census** of the toolpath
//! handed to `fit_arcs` — every adjacent `(Linear, Linear)` same-feed pair
//! (i.e. every pair the OLD key would have joined) classified by which new
//! key term breaks it:
//!
//! * `intent_breaks_semantic` — two different tagged intents;
//! * `intent_breaks_unknown_strict` — one side is `Unknown`. These are the
//!   breaks that exist **only** because Q3 was ruled strict; a permissive
//!   `Unknown` wildcard would not make them;
//! * `region_breaks` — same intent, same feed, but a `SpanKind::Region` edge
//!   (Checkpoint F1 Q2).
//!
//! **Measured result at PR-6 (parent `246b7ae`).** Seven strict-`Unknown`
//! seams exist across the five fixtures (3 in `three_pass`, 4 in
//! `arc_raster`) and they cost **zero** extra arcs and **zero** extra moves:
//! every one of them sits where `try_fit_arc` already refused the geometry
//! or where the greedy run had already ended. Total arc count is identical
//! before and after the fix on all five fixtures. What DOES move is labels
//! (`arc_raster`: `Unknown` 24 → 20, `LeadOut` 0 → 4) and, on the Finish-role
//! fixtures, the arc split POINT — the `FinishingCut` arc no longer runs one
//! segment into the lead-out. Both are the fix working.
//!
//! Run with `-- --nocapture` to see the raw census lines.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::too_many_arguments
)]

use rs_cam_core::{
    compute::catalog::{OperationType, UiProcessRole},
    compute::config::{DressupConfig, DressupEntryStyle},
    compute::execute::apply_dressups,
    geo::P3,
    toolpath::{Move, MoveIntent, MoveType, Toolpath},
    toolpath_spans::{AnnotatedToolpath, Span, SpanKind},
    transform_provenance::ReconcileSet,
};

const FEED_EPS: f64 = 1e-6;

fn is_arc(m: &Move) -> bool {
    matches!(
        m.move_type,
        MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
    )
}

fn counts(tp: &Toolpath) -> (usize, usize) {
    (
        tp.moves.len(),
        tp.moves.iter().filter(|m| is_arc(m)).count(),
    )
}

fn fingerprint(tp: &Toolpath) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in format!("{:?}", tp.moves).bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn intent_histogram(tp: &Toolpath) -> String {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for m in &tp.moves {
        *counts.entry(format!("{:?}", m.intent)).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Classify every adjacent (Linear, Linear) same-feed pair — the pairs
/// today's key would join — by which NEW key term breaks it.
fn seam_census(at: &AnnotatedToolpath) -> (usize, usize, usize, usize) {
    let region_edges: std::collections::BTreeSet<usize> = at
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::Region)
        .flat_map(|s| [s.start_move, s.end_move])
        .collect();
    let moves = &at.toolpath.moves;
    let (mut joins, mut semantic, mut unknown_strict, mut region) = (0, 0, 0, 0);
    for k in 1..moves.len() {
        let (MoveType::Linear { feed_rate: fa }, MoveType::Linear { feed_rate: fb }) =
            (moves[k - 1].move_type, moves[k].move_type)
        else {
            continue;
        };
        if (fa - fb).abs() >= FEED_EPS {
            continue;
        }
        joins += 1;
        let (ia, ib) = (moves[k - 1].intent, moves[k].intent);
        if ia != ib {
            if ia == MoveIntent::Unknown || ib == MoveIntent::Unknown {
                unknown_strict += 1;
            } else {
                semantic += 1;
            }
        } else if region_edges.contains(&k) {
            region += 1;
        }
    }
    (joins, semantic, unknown_strict, region)
}

fn full_dressups() -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: 15.0,
        dogbone: true,
        dogbone_angle: 90.0,
        lead_in_out: true,
        lead_radius: 1.5,
        link_moves: true,
        link_max_distance: 50.0,
        arc_fitting: true,
        arc_tolerance: 0.05,
        segment_merge: true,
        segment_merge_tolerance: 0.02,
        optimize_rapid_order: true,
        ..DressupConfig::default()
    }
}

fn three_pass() -> AnnotatedToolpath {
    let mut tp = Toolpath::new();
    let mut pass_starts = Vec::new();
    let z_levels = [-3.0, -6.0, -9.0];
    for (idx, &z) in z_levels.iter().enumerate() {
        if idx == 0 {
            tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        } else {
            tp.rapid_to(P3::new(0.0, 0.0, 10.0));
            tp.rapid_to(P3::new(2.0 + idx as f64, 0.0, 10.0));
        }
        pass_starts.push(tp.moves.len());
        tp.feed_to(P3::new(2.0 + idx as f64, 0.0, z), 500.0);
        tp.feed_to(P3::new(20.0 + idx as f64, 0.0, z), 1000.0);
        tp.rapid_to(P3::new(20.0 + idx as f64, 0.0, 10.0));
    }
    let n = tp.moves.len();
    let mut spans = vec![Span::new(0, n, SpanKind::Operation)];
    for (i, &start) in pass_starts.iter().enumerate() {
        let end = pass_starts.get(i + 1).copied().unwrap_or(n);
        spans.push(Span::new(start, end, SpanKind::DepthPass));
        if start > 0 {
            spans.push(Span::boundary(start, SpanKind::RapidOrderBarrier));
        }
    }
    AnnotatedToolpath::with_spans(tp, spans)
}

fn arc_raster() -> AnnotatedToolpath {
    let mut tp = Toolpath::new();
    let mut region_starts = Vec::new();
    for row in 0..4 {
        let y0 = row as f64 * 4.0;
        let z = -1.0 - row as f64;
        tp.rapid_to(P3::new(0.0, y0, 10.0));
        region_starts.push(tp.moves.len());
        tp.feed_to(P3::new(0.0, y0, z), 400.0);
        for step in 0..=24 {
            let theta = std::f64::consts::PI * (step as f64) / 24.0;
            tp.feed_to(
                P3::new(10.0 - 8.0 * theta.cos(), y0 + 8.0 * theta.sin(), z),
                1200.0,
            );
        }
        for step in 1..=20 {
            tp.feed_to(P3::new(18.0 + step as f64 * 0.2, y0, z), 1200.0);
        }
        tp.rapid_to(P3::new(22.0, y0, 10.0));
    }
    let n = tp.moves.len();
    let mut spans = vec![Span::new(0, n, SpanKind::Operation)];
    for (i, &start) in region_starts.iter().enumerate() {
        let end = region_starts.get(i + 1).copied().unwrap_or(n);
        spans.push(Span::new(start, end, SpanKind::Region).with_label(format!("row-{i}")));
    }
    AnnotatedToolpath::with_spans(tp, spans)
}

fn face_fixture() -> AnnotatedToolpath {
    use rs_cam_core::face::{FaceParams, face_toolpath};
    use rs_cam_core::geo::BoundingBox3;

    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(40.0, 30.0, 5.0),
    };
    let params = FaceParams {
        tool_radius: 3.0,
        stepover: 4.0,
        depth: 2.0,
        depth_per_pass: 2.0,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        safe_z: 30.0,
        stock_offset: 0.0,
        direction: rs_cam_core::face::FaceDirection::OneWay,
        stock_top_z: 0.0,
    };
    let raw = face_toolpath(&bbox, &params);
    let n = raw.moves.len();
    AnnotatedToolpath::with_spans(raw, vec![Span::new(0, n, SpanKind::Operation)])
}

/// Four circular finishing passes tagged `FinishingCut`, run through the
/// SHIPPED `DressupConfig::for_role(Finish)` chain — the population §1.3 of
/// the evidence doc names (11 Finish-role ops, lead_in_out + arc_fitting on).
fn finish_passes(radii: [f64; 4]) -> AnnotatedToolpath {
    let mut tp = Toolpath::new();
    for (pass, &r) in radii.iter().enumerate() {
        let z = -1.0 - pass as f64;
        let pt = |k: usize, r: f64| {
            let a = (k as f64) * 5.0_f64.to_radians();
            P3::new(r * a.cos(), r * a.sin(), z)
        };
        tp.rapid_to_with_intent(P3::new(pt(0, r).x, pt(0, r).y, 10.0), MoveIntent::Linking);
        tp.feed_to_with_intent(pt(0, r), 400.0, MoveIntent::EntryPlunge);
        for k in 1..=36 {
            tp.feed_to_with_intent(pt(k, r), 1000.0, MoveIntent::FinishingCut);
        }
        tp.rapid_to_with_intent(P3::new(pt(36, r).x, pt(36, r).y, 10.0), MoveIntent::Retract);
    }
    let n = tp.moves.len();
    AnnotatedToolpath::with_spans(tp, vec![Span::new(0, n, SpanKind::Operation)])
}

/// One fixture's pinned reading.
struct Expect {
    /// `(moves, arcs)` after the shipped dressup chain.
    out: (usize, usize),
    /// `(same_feed_linear_joins, semantic, unknown_strict, region)`.
    census: (usize, usize, usize, usize),
}

fn run(
    name: &str,
    input: AnnotatedToolpath,
    cfg: &DressupConfig,
    feed: f64,
    op: OperationType,
    expect: &Expect,
) {
    // Chain with arcfit + merge OFF = exactly the toolpath handed to fit_arcs.
    let pre_cfg = DressupConfig {
        arc_fitting: false,
        segment_merge: false,
        ..cfg.clone()
    };
    let pre = apply_dressups(
        input.clone(),
        &pre_cfg,
        feed,
        6.0,
        30.0,
        0.0,
        None,
        None,
        None,
        op.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::empty(),
    );
    let (joins, semantic, unknown_strict, region) = seam_census(&pre);

    let out = apply_dressups(
        input,
        cfg,
        feed,
        6.0,
        30.0,
        0.0,
        None,
        None,
        None,
        op.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::empty(),
    );
    let (moves, arcs) = counts(&out.toolpath);
    println!(
        "PR6COST {name}: moves={moves} arcs={arcs} fnv={:020} | pre_arcfit_moves={} \
         same_feed_linear_joins={joins} intent_breaks_semantic={semantic} \
         intent_breaks_unknown_strict={unknown_strict} region_breaks={region}",
        fingerprint(&out.toolpath),
        pre.toolpath.moves.len()
    );
    println!("PR6HIST {name}: {}", intent_histogram(&out.toolpath));

    assert_eq!(
        (moves, arcs),
        expect.out,
        "{name}: dressup-chain (moves, arcs) moved. Arc count is the cost \
         the F1 Q3 ruling required PR-6 to measure — if it rose, the run key \
         got stricter; if it fell, a boundary was removed. Re-measure and \
         re-pin deliberately, do not just update the number."
    );
    assert_eq!(
        (joins, semantic, unknown_strict, region),
        expect.census,
        "{name}: run-key seam census moved. `unknown_strict` is the count of \
         breaks that exist ONLY because Checkpoint F1 Q3 ruled \
         `MoveIntent::Unknown` strict rather than a wildcard."
    );
}

/// Every `(moves, arcs)` below is IDENTICAL to the pre-fix reading taken at
/// `246b7ae` with the arcfit change stashed out. That equality IS the
/// measurement: the intent term, strict `Unknown` included, and the `Region`
/// barrier together cost zero arcs and zero moves on all five fixtures.
#[test]
fn pr6_measure_arcfit_intent_key_cost() {
    run(
        "three_pass",
        three_pass(),
        &full_dressups(),
        1000.0,
        OperationType::Adaptive3d,
        &Expect {
            out: (23, 3),
            census: (28, 2, 3, 0),
        },
    );
    run(
        "arc_raster",
        arc_raster(),
        &full_dressups(),
        1200.0,
        OperationType::Adaptive3d,
        &Expect {
            out: (40, 8),
            census: (216, 4, 4, 0),
        },
    );
    run(
        "face_full",
        face_fixture(),
        &full_dressups(),
        1500.0,
        OperationType::Face,
        &Expect {
            out: (74, 13),
            census: (118, 20, 0, 0),
        },
    );
    // Pass radii far from the shipped `lead_radius` (2.0): the lead arcs are
    // NOT co-circular with the cut, so `try_fit_arc` already refuses to
    // collapse across the boundary on geometry alone.
    run(
        "finish_role_wide",
        finish_passes([8.0, 9.0, 10.0, 11.0]),
        &DressupConfig::for_role(UiProcessRole::Finish),
        1000.0,
        OperationType::Scallop,
        &Expect {
            out: (32, 8),
            census: (180, 8, 0, 0),
        },
    );
    // Pass radii AT the shipped `lead_radius`: the lead-out quarter-circle is
    // an exact continuation of the cut's own circle, so only the run key can
    // stop the collapse. This is the shipped-defaults worst case, and the
    // one whose emitted GEOMETRY moves — same arc count, different split
    // point, because the `FinishingCut` arc no longer runs one segment into
    // the lead-out.
    run(
        "finish_role_tight",
        finish_passes([2.0, 2.0, 2.0, 2.0]),
        &DressupConfig::for_role(UiProcessRole::Finish),
        1000.0,
        OperationType::Scallop,
        &Expect {
            out: (32, 8),
            census: (180, 8, 0, 0),
        },
    );
}
