//! **Checkpoint F1 sentries — arcfit's run key carries `MoveIntent`.**
//!
//! Oracle: `planning/review_2026-08-04/TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md`
//! §H2 / R7 fix-shape 2 and Checkpoint F1; evidence package in
//! `planning/review_2026-08-04/ARCFIT_INTENT_EVIDENCE.md`.
//!
//! ## What these tests guard
//!
//! Before PR-6, `arcfit::fit_arcs` grouped a candidate run by
//! `(MoveType::Linear, feed_rate ± FEED_EPS)` and by span barrier only —
//! `Move::intent` was not part of the key — then took the collapsed arc's
//! intent from the FIRST source move. So a homogeneous `FinishingCut` run
//! continuing at the same feed into the `LeadOut` arc `apply_lead_in_out`
//! had just appended collapsed into ONE arc labelled `FinishingCut` whose
//! target was the *lead-out's* endpoint — a position that is not on the
//! machined surface. That relabelling is what invalidated wave 11's "21
//! dropped cut positions" reading (see `scallop_intra_pass_relink_am7.rs`'s
//! header): the gate selected its population by a label that a downstream
//! transform had rewritten.
//!
//! PR-6 added `intent ==` to the run key (Checkpoint F1 Q1), made
//! `MoveIntent::Unknown` STRICT rather than a wildcard (Q3), and joined
//! `SpanKind::Region` boundaries to arc-fit's barrier set (Q2). Every
//! assertion below now pins the CORRECT behaviour:
//!
//! * no fitted arc spans an intent boundary;
//! * no fitted arc spans a `Region` boundary;
//! * homogeneous runs still collapse — the fix costs no arc it should keep;
//! * a move's span kind and its own intent agree.
//!
//! ## History — this file was red-first
//!
//! Wave W2 shipped these as four positive assertions of the DEFECT at
//! `8963b75`, deliberately not `#[ignore]`d, so that the H2.2 fix would
//! break them loudly. It did: at PR-6's parent `246b7ae` the four passed;
//! with the fix applied three failed (the fourth, `f1_control_…`, is the
//! control and passed unchanged both times). That failure is PR-6's
//! red-first evidence and is recorded in its commit body. The assertions
//! were then inverted in place, exactly as each doc comment directed —
//! not deleted.
//!
//! No production code is touched by this file. It is an instrument.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::{
    arcfit::fit_arcs,
    dressup::apply_lead_in_out_with_feeds,
    geo::P3,
    toolpath::{Move, MoveIntent, MoveType, Toolpath},
    toolpath_spans::AnnotatedToolpath,
};

/// Arc-fit tolerance used throughout. Matches `DressupConfig::default()`.
const TOL: f64 = 0.05;
/// Envelope radius of a Ø6 end mill. The F.10 cap is
/// `LARGE_ARC_RADIUS_MULTIPLIER` (30) × this = 90 mm, far above the 6–8 mm
/// radii these fixtures fit, so the cap never decides an outcome here.
const TOOL_R: f64 = 3.0;

fn is_arc(m: &Move) -> bool {
    matches!(
        m.move_type,
        MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
    )
}

fn count_intent(tp: &Toolpath, intent: MoveIntent) -> usize {
    tp.moves.iter().filter(|m| m.intent == intent).count()
}

// ── Sentry 1: the run key, in isolation ──────────────────────────────────

/// **F1 sentry 1 — `FinishingCut → LeadOut → LeadIn` fits as THREE arcs,
/// one per intent.**
///
/// Eighteen co-circular moves at ONE feed, tagged in three homogeneous
/// blocks: 6 `FinishingCut`, then 6 `LeadOut`, then 6 `LeadIn` (which
/// `compute::spans::spans_from_move_intents` maps to `SpanKind::Entry`).
/// Geometry is deliberately uniform so nothing but the intent distinguishes
/// the blocks — this isolates the run key from any geometric cause.
///
/// Before PR-6: ONE arc, intent `FinishingCut`, landing on the last
/// `LeadIn` point; twelve moves' worth of non-cutting geometry relabelled
/// as finishing cut and both transit populations emptied.
///
/// Now: three arcs, each ending exactly where its own block ends, each
/// keeping its own label. The homogeneous halves still collapse fully —
/// six source moves per arc — so the added key term costs nothing on a
/// run it should have kept.
#[test]
fn f1_sentry_each_intent_block_fits_its_own_arc() {
    let (cx, cy, r, z, feed) = (0.0, 0.0, 8.0, -2.0, 1000.0);
    // 5° steps: per-segment sagitta ≈ c²/(8r) ≈ 0.0060 mm, well inside TOL.
    let pt = |k: usize| {
        let a = (k as f64) * 5.0_f64.to_radians();
        P3::new(cx + r * a.cos(), cy + r * a.sin(), z)
    };

    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(pt(0), MoveIntent::Linking);
    for k in 1..=6 {
        tp.feed_to_with_intent(pt(k), feed, MoveIntent::FinishingCut);
    }
    for k in 7..=12 {
        tp.feed_to_with_intent(pt(k), feed, MoveIntent::LeadOut);
    }
    for k in 13..=18 {
        tp.feed_to_with_intent(pt(k), feed, MoveIntent::LeadIn);
    }

    assert_eq!(count_intent(&tp, MoveIntent::FinishingCut), 6);
    assert_eq!(count_intent(&tp, MoveIntent::LeadOut), 6);
    assert_eq!(count_intent(&tp, MoveIntent::LeadIn), 6);

    let out = fit_arcs(AnnotatedToolpath::new(tp), TOL, TOOL_R).toolpath;

    let arcs: Vec<&Move> = out.moves.iter().filter(|m| is_arc(m)).collect();
    assert_eq!(
        arcs.len(),
        3,
        "F1 sentry: one arc per intent block (got {} arcs across {} moves)",
        arcs.len(),
        out.moves.len()
    );

    // Each arc keeps its own block's label — no arc inherits a neighbour's.
    assert_eq!(arcs[0].intent, MoveIntent::FinishingCut);
    assert_eq!(arcs[1].intent, MoveIntent::LeadOut);
    assert_eq!(arcs[2].intent, MoveIntent::LeadIn);

    // …and each ends exactly where its own block ends. The `FinishingCut`
    // arc stops at the true cut end pt(6); before PR-6 it ran to pt(18).
    for (arc, k) in arcs.iter().zip([6usize, 12, 18]) {
        let end = pt(k);
        assert!(
            (arc.target.x - end.x).abs() < 1e-9 && (arc.target.y - end.y).abs() < 1e-9,
            "F1 sentry: {:?} arc must end on its own block's last point {:?}, got {:?}",
            arc.intent,
            end,
            arc.target
        );
    }

    // Both transit populations survive — exactly one collapsed arc each.
    assert_eq!(
        count_intent(&out, MoveIntent::LeadOut),
        1,
        "F1 sentry: the LeadOut block survives as its own arc"
    );
    assert_eq!(
        count_intent(&out, MoveIntent::LeadIn),
        1,
        "F1 sentry: the LeadIn block survives as its own arc"
    );

    // The fix must not fragment a homogeneous run: rapid + 3 arcs, nothing
    // left as a residual linear.
    assert_eq!(
        out.moves.len(),
        4,
        "F1 sentry: each 6-move homogeneous block still collapses fully"
    );
}

/// **F1 control — a feed change already breaks the run.**
///
/// The same geometry with the `LeadOut` block at a different feed splits
/// into separate arcs and keeps its `LeadOut` label. This is the whole
/// reason the defect is intermittent in the field: the boundary is
/// protected when, and only when, the feeds happen to differ.
///
/// AFTER THE FIX this test must still pass unchanged — the fix adds a
/// boundary, it must not remove this one.
#[test]
fn f1_control_a_feed_change_already_splits_the_run() {
    let (r, z) = (8.0, -2.0);
    let pt = |k: usize| {
        let a = (k as f64) * 5.0_f64.to_radians();
        P3::new(r * a.cos(), r * a.sin(), z)
    };

    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(pt(0), MoveIntent::Linking);
    for k in 1..=6 {
        tp.feed_to_with_intent(pt(k), 1000.0, MoveIntent::FinishingCut);
    }
    for k in 7..=12 {
        // Different feed — the only thing today's run key can see.
        tp.feed_to_with_intent(pt(k), 1800.0, MoveIntent::LeadOut);
    }

    let out = fit_arcs(AnnotatedToolpath::new(tp), TOL, TOOL_R).toolpath;

    let arcs: Vec<&Move> = out.moves.iter().filter(|m| is_arc(m)).collect();
    assert_eq!(
        arcs.len(),
        2,
        "control: distinct feeds must produce two arcs, got {}",
        arcs.len()
    );
    assert_eq!(arcs[0].intent, MoveIntent::FinishingCut);
    assert_eq!(
        arcs[1].intent,
        MoveIntent::LeadOut,
        "control: with a feed break the LeadOut label survives"
    );
}

// ── Sentry 2: the production path that produces the mix ──────────────────

/// **F1 sentry 2 — the lead-out survives the shipped dressup chain.**
///
/// This is exhibit 1 without the hand-tagging: a curved finishing pass goes
/// through `apply_lead_in_out` (which appends the `LeadOut` quarter-arc and
/// tags it) and then through `fit_arcs`, exactly the order
/// `compute::execute::apply_dressups` runs them (steps 3 then 5).
///
/// Two production facts make this reachable on defaults, not a contrivance:
///
/// * `DressupConfig::for_role(Finish)` ships `lead_in_out: true` AND
///   `arc_fitting: true` — the combination is the DEFAULT for every
///   finishing operation;
/// * `lead_out_feed_rate` defaults to `None`, and `apply_lead_in_out` then
///   falls back to the cut pass's own feed — so the `FinishingCut → LeadOut`
///   boundary carries NO feed change, and today's run key cannot see it.
///
/// The geometry is a CCW arc of radius equal to `lead_radius`, which is the
/// case where the lead-out quarter-circle is an exact continuation of the
/// cut's own circle. Arc-fit's `tolerance` means near-matches collapse too;
/// this fixture just makes the collapse deterministic.
///
/// Before PR-6 the greedy fitter extended the cut's own circle into the
/// lead-out by as many segments as `tolerance` allowed — here exactly one —
/// and relabelled that segment `FinishingCut`, landing ~1.18 mm off the
/// machined surface. One lead-out position relabelled per pass is precisely
/// the "1 lost cut position per junction" arithmetic wave 11 mis-read as a
/// relinker defect (`scallop_intra_pass_relink_am7.rs` header).
///
/// Now: no arc labelled `FinishingCut` targets a lead-out source point, and
/// the `FinishingCut` arc ends exactly on the true cut end.
#[test]
fn f1_sentry_lead_out_is_not_swallowed_by_the_finishing_arc() {
    let (r, z, feed, plunge) = (6.0, -2.0, 1000.0, 300.0);
    let safe_z = 10.0;
    // CCW quarter turn, 5° steps.
    let pt = |k: usize| {
        let a = (k as f64) * 5.0_f64.to_radians();
        P3::new(r * a.cos(), r * a.sin(), z)
    };

    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(pt(0).x, pt(0).y, safe_z), MoveIntent::Linking);
    // Plunge at a DIFFERENT feed — mirrors production, where the entry side
    // is protected only by that accident.
    tp.feed_to_with_intent(pt(0), plunge, MoveIntent::EntryPlunge);
    for k in 1..=18 {
        tp.feed_to_with_intent(pt(k), feed, MoveIntent::FinishingCut);
    }
    tp.rapid_to_with_intent(P3::new(pt(18).x, pt(18).y, safe_z), MoveIntent::Retract);

    // Step 3 of the dressup chain. `lead_radius == r` and both feed
    // overrides `None` — the shipped defaults.
    let led = apply_lead_in_out_with_feeds(AnnotatedToolpath::new(tp), r, None, None);
    let lead_outs = count_intent(&led.toolpath, MoveIntent::LeadOut);
    assert_eq!(
        lead_outs, 8,
        "fixture precondition: apply_lead_in_out emits an 8-segment lead-out"
    );
    // The lead-out's OWN source points — every position that the machined
    // finishing pass does not contain.
    let lead_out_pts: Vec<P3> = led
        .toolpath
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::LeadOut)
        .map(|m| m.target)
        .collect();
    // Where the machined pass actually ends.
    let true_cut_end = pt(18);

    // Step 5 of the dressup chain.
    let out = fit_arcs(led, TOL, TOOL_R).toolpath;

    // No move labelled `FinishingCut` may land on a LEAD-OUT source point.
    // The `is_arc` filter is deliberately absent: a residual linear carrying
    // the wrong label would be the same defect in a different move type.
    let swallowed: Vec<&Move> = out
        .moves
        .iter()
        .filter(|m| {
            m.intent == MoveIntent::FinishingCut
                && lead_out_pts
                    .iter()
                    .any(|p| (m.target.x - p.x).abs() < 1e-9 && (m.target.y - p.y).abs() < 1e-9)
        })
        .collect();
    assert!(
        swallowed.is_empty(),
        "F1 sentry 2: {} move(s) labelled FinishingCut target a LEAD-OUT \
         source point — the boundary-crossing collapse is back",
        swallowed.len()
    );

    // The last cutting-labelled move stops exactly on the machined surface,
    // not ~1.18 mm past it.
    let last_cut = out
        .moves
        .iter()
        .rfind(|m| m.intent == MoveIntent::FinishingCut)
        .expect("the finishing pass survives arc-fitting");
    let off_surface = ((last_cut.target.x - true_cut_end.x).powi(2)
        + (last_cut.target.y - true_cut_end.y).powi(2))
    .sqrt();
    assert!(
        off_surface < 1e-9,
        "F1 sentry 2: the last FinishingCut move must end on the true cut \
         end; measured {off_surface:.6} mm past it"
    );

    // The lead-out population is intact: all eight source moves collapse
    // into arcs that all still read `LeadOut`, none relabelled away.
    assert!(
        count_intent(&out, MoveIntent::LeadOut) >= 1,
        "F1 sentry 2: {lead_outs} LeadOut source moves must survive as \
         LeadOut-labelled output"
    );
}

/// **F1 sentry 3 — the `LeadOut` span and the arc's intent agree about the
/// same move.**
///
/// `apply_lead_in_out` pushes a `SpanKind::LeadOut` span over the moves it
/// appends. `fit_arcs` remaps that span through the N-to-1 collapse, so it
/// lands on the collapsed arc — while the arc's own `intent` says
/// `FinishingCut`. The two channels that both claim to say "what is this
/// move" now contradict each other on the same index.
///
/// This matters because the two channels have different consumers:
/// span-ancestry gate filters (`tool_load::locality::is_phantom_transit`)
/// see `LeadOut` and drop the sample; intent-selected populations
/// (`feed_modulation::should_skip_modulation`,
/// `machine_kinematics` cycle-time attribution) see `FinishingCut` and
/// include it.
///
/// Before PR-6 they contradicted each other on at least one index. Now
/// they agree.
#[test]
fn f1_sentry_span_and_intent_agree_on_the_fitted_arcs() {
    use rs_cam_core::toolpath_spans::{Span, SpanKind};

    let (r, z, feed, plunge) = (6.0, -2.0, 1000.0, 300.0);
    let safe_z = 10.0;
    let pt = |k: usize| {
        let a = (k as f64) * 5.0_f64.to_radians();
        P3::new(r * a.cos(), r * a.sin(), z)
    };

    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(pt(0).x, pt(0).y, safe_z), MoveIntent::Linking);
    tp.feed_to_with_intent(pt(0), plunge, MoveIntent::EntryPlunge);
    for k in 1..=18 {
        tp.feed_to_with_intent(pt(k), feed, MoveIntent::FinishingCut);
    }
    tp.rapid_to_with_intent(P3::new(pt(18).x, pt(18).y, safe_z), MoveIntent::Retract);
    let n = tp.moves.len();
    let annotated = AnnotatedToolpath::with_spans(tp, vec![Span::new(0, n, SpanKind::Operation)]);

    let led = apply_lead_in_out_with_feeds(annotated, r, None, None);
    assert!(
        led.spans.iter().any(|s| s.kind == SpanKind::LeadOut),
        "fixture precondition: apply_lead_in_out must push a LeadOut span"
    );
    let out = fit_arcs(led, TOL, TOOL_R);
    out.check_invariants()
        .expect("post-arc spans still pass invariants");

    // Find every move covered by a LeadOut span after the collapse.
    let covered: Vec<usize> = out
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::LeadOut)
        .flat_map(|s| s.start_move..s.end_move)
        .collect();
    assert!(
        !covered.is_empty(),
        "the LeadOut span survives the remap onto the collapsed arc"
    );

    // No move inside a LeadOut span may claim to be a finishing cut.
    let contradictory = covered
        .iter()
        .filter(|&&i| out.toolpath.moves[i].intent == MoveIntent::FinishingCut)
        .count();
    assert_eq!(
        contradictory, 0,
        "F1 sentry 3: {contradictory} move(s) sit inside a LeadOut span \
         while their own intent reads FinishingCut — the two channels that \
         both answer 'what is this move' disagree again"
    );
}

// ── Sentry 4: Checkpoint F1 Q2 — Region boundaries break an arc ──────────

/// **F1 sentry 4 — no fitted arc straddles a `SpanKind::Region` boundary.**
///
/// Twelve co-circular moves at one feed and ONE intent, split into two
/// `Region` spans at the midpoint. Nothing in the intent term can see that
/// boundary — only the barrier set can. Before PR-6 `Region` was not in
/// arc-fit's barrier set, so the whole run collapsed into a single arc, both
/// regions' `move_range`s landed on that one index, and they stopped tiling
/// — the exact invariant `region_node_ranges_tile_the_stitched_toolpath`
/// (`unified_finish.rs`) exists to protect.
///
/// Ruled at Checkpoint F1 Q2 and bundled into PR-6. Note this is a
/// LOCAL barrier: `AnnotatedToolpath::rapid_order_barriers()` is unchanged,
/// because TSP reordering and `execute`'s barrier-count branch also read it.
#[test]
fn f1_sentry_no_arc_straddles_a_region_boundary() {
    use rs_cam_core::toolpath_spans::{Span, SpanKind};

    let (r, z, feed) = (8.0, -2.0, 1000.0);
    let pt = |k: usize| {
        let a = (k as f64) * 5.0_f64.to_radians();
        P3::new(r * a.cos(), r * a.sin(), z)
    };

    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(pt(0), MoveIntent::Linking);
    for k in 1..=12 {
        tp.feed_to_with_intent(pt(k), feed, MoveIntent::FinishingCut);
    }
    let n = tp.moves.len();
    // Region A covers moves 1..7, region B covers 7..13. Same intent, same
    // feed, co-circular geometry: only the Region edge can break this run.
    let annotated = AnnotatedToolpath::with_spans(
        tp,
        vec![
            Span::new(0, n, SpanKind::Operation),
            Span::new(1, 7, SpanKind::Region).with_label("region-a"),
            Span::new(7, n, SpanKind::Region).with_label("region-b"),
        ],
    );

    let out = fit_arcs(annotated, TOL, TOOL_R);
    out.check_invariants()
        .expect("post-arc spans still pass invariants");

    let arcs: Vec<&Move> = out.toolpath.moves.iter().filter(|m| is_arc(m)).collect();
    assert_eq!(
        arcs.len(),
        2,
        "F1 sentry 4: the run must break at the Region edge — one arc per \
         region (got {} arcs across {} moves)",
        arcs.len(),
        out.toolpath.moves.len()
    );

    // The regions still tile: each Region span covers a non-empty, disjoint,
    // contiguous move range in the output.
    let mut ranges: Vec<(usize, usize)> = out
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::Region)
        .map(|s| (s.start_move, s.end_move))
        .collect();
    ranges.sort_unstable();
    assert_eq!(ranges.len(), 2, "both Region spans survive the remap");
    assert!(
        ranges[0].1 <= ranges[1].0,
        "F1 sentry 4: Region move_ranges must stay disjoint after the \
         collapse, got {ranges:?}"
    );
    assert!(
        ranges[0].0 < ranges[0].1 && ranges[1].0 < ranges[1].1,
        "F1 sentry 4: neither Region may collapse to an empty range, got \
         {ranges:?}"
    );
}
