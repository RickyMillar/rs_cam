//! **Checkpoint F1 exhibit — arcfit's run key ignores `MoveIntent`.**
//!
//! Oracle: `planning/review_2026-08-04/TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md`
//! §H2 / R7 fix-shape 2 and Checkpoint F1; evidence package in
//! `planning/review_2026-08-04/ARCFIT_INTENT_EVIDENCE.md`.
//!
//! ## What these tests assert, and why they look backwards
//!
//! Every assertion below pins the **current, defective** behaviour on
//! purpose. `arcfit::fit_arcs` groups a candidate run by
//! `(MoveType::Linear, feed_rate ± FEED_EPS)` and by span barrier only —
//! `Move::intent` is not part of the key — then takes the collapsed arc's
//! intent from the FIRST source move (`arcfit.rs:194`). So a homogeneous
//! `FinishingCut` run followed by the `LeadOut` arc `apply_lead_in_out`
//! appended, and by whatever comes next at the same feed, collapses into
//! ONE arc labelled `FinishingCut` whose target is the *lead-out's*
//! endpoint — a position that is not on the machined surface.
//!
//! That relabelling is what invalidated wave 11's "21 dropped cut
//! positions" reading (see `scallop_intra_pass_relink_am7.rs`'s header):
//! the gate selected its population by a label that a downstream transform
//! had rewritten.
//!
//! ## This file is red-first, inverted-later
//!
//! It is deliberately NOT `#[ignore]`d: an ignored characterization rots in
//! silence. These assertions must **fail loudly** the moment the H2.2
//! intent-boundary fix lands — that failure is the fix's own red-first
//! evidence. When PR-6 lands, invert each assertion as its doc comment
//! directs; do not delete the test.
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

// ── Exhibit 1: the run key, in isolation ─────────────────────────────────

/// **F1 exhibit 1 — `FinishingCut → LeadOut → Entry` collapses into one
/// `FinishingCut` arc.**
///
/// Eighteen co-circular moves at ONE feed, tagged in three homogeneous
/// blocks: 6 `FinishingCut`, then 6 `LeadOut`, then 6 `LeadIn` (which
/// `compute::spans::spans_from_move_intents` maps to `SpanKind::Entry`).
/// Geometry is deliberately uniform so nothing but the intent distinguishes
/// the blocks — this isolates the run key from any geometric cause.
///
/// TODAY: one arc, intent `FinishingCut`, landing on the last `LeadIn`
/// point. Twelve moves' worth of non-cutting geometry is now labelled as
/// finishing cut, and the `LeadOut` / `LeadIn` populations are empty.
///
/// AFTER THE FIX, invert to: at least three arcs (or linear fallbacks), no
/// arc whose source moves span two intents, and `LeadOut` / `LeadIn` counts
/// preserved at 1 arc each (or 6 linears each).
#[test]
fn f1_exhibit_mixed_intent_run_collapses_into_one_finishing_arc() {
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
        1,
        "F1 exhibit: the whole mixed-intent run collapses into ONE arc \
         (got {} arcs across {} moves)",
        arcs.len(),
        out.moves.len()
    );

    // DEFECT, PINNED: the arc that swallowed six LeadOut and six LeadIn
    // moves is labelled a finishing cut.
    assert_eq!(
        arcs[0].intent,
        MoveIntent::FinishingCut,
        "F1 exhibit: collapsed arc inherits the FIRST source move's intent"
    );

    // DEFECT, PINNED: it lands on the last LeadIn point — 60° of arc past
    // where the finishing cut actually ended.
    let end = pt(18);
    assert!(
        (arcs[0].target.x - end.x).abs() < 1e-9 && (arcs[0].target.y - end.y).abs() < 1e-9,
        "F1 exhibit: the 'FinishingCut' arc targets the LeadIn endpoint \
         ({:?}), not the cut's own end {:?}",
        arcs[0].target,
        pt(6)
    );

    // DEFECT, PINNED: both transit populations vanish from the output.
    assert_eq!(
        count_intent(&out, MoveIntent::LeadOut),
        0,
        "F1 exhibit: every LeadOut move is relabelled away"
    );
    assert_eq!(
        count_intent(&out, MoveIntent::LeadIn),
        0,
        "F1 exhibit: every LeadIn move is relabelled away"
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

// ── Exhibit 2: the production path that produces the mix ─────────────────

/// **F1 exhibit 2 — the same collapse via the shipped dressup chain.**
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
/// AFTER THE FIX, invert to: `LeadOut` moves survive as their own arc (or
/// as linears), and no arc's target is a lead-out point.
#[test]
fn f1_exhibit_lead_out_is_swallowed_by_the_finishing_arc() {
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

    // DEFECT, PINNED: an arc labelled `FinishingCut` lands on a LEAD-OUT
    // source point — geometry the finishing pass never cut.
    //
    // Measured mechanism: the greedy fitter extends the cut's own circle
    // as far into the lead-out as `tolerance` allows, which here is exactly
    // one lead-out segment. That one move is relabelled `FinishingCut`; the
    // remaining seven start a fresh run and collapse into an arc that DOES
    // keep `LeadOut`. One lead-out position relabelled per pass is precisely
    // the "1 lost cut position per junction" arithmetic wave 11 mis-read as
    // a relinker defect (`scallop_intra_pass_relink_am7.rs` header).
    let swallowing_arc = out
        .moves
        .iter()
        .find(|m| {
            m.intent == MoveIntent::FinishingCut
                && is_arc(m)
                && lead_out_pts
                    .iter()
                    .any(|p| (m.target.x - p.x).abs() < 1e-9 && (m.target.y - p.y).abs() < 1e-9)
        })
        .expect(
            "F1 exhibit 2: expected an arc labelled FinishingCut whose target \
             is a LEAD-OUT source point (the boundary-crossing collapse)",
        );

    // …and it is off the machined surface by a millimetre-scale distance,
    // not a rounding artefact. The field reading was "up to 1.2 mm off the
    // machined surface"; this fixture reproduces ~1.18 mm.
    let off_surface = ((swallowing_arc.target.x - true_cut_end.x).powi(2)
        + (swallowing_arc.target.y - true_cut_end.y).powi(2))
    .sqrt();
    assert!(
        off_surface > 1.0,
        "F1 exhibit 2: the relabelled arc should land ~1.18 mm past the true \
         cut end; measured {off_surface:.3} mm"
    );

    // DEFECT, PINNED: the lead-out population shrinks. Eight source moves
    // go in; the survivors collapse into ONE arc still labelled LeadOut,
    // and one source move is gone from the population entirely.
    assert_eq!(
        count_intent(&out, MoveIntent::LeadOut),
        1,
        "F1 exhibit 2: {lead_outs} LeadOut source moves collapse to one \
         surviving LeadOut arc"
    );
}

/// **F1 exhibit 3 — the `LeadOut` span and the arc's intent disagree about
/// the same move.**
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
/// AFTER THE FIX both channels must agree; invert to assert agreement.
#[test]
fn f1_exhibit_span_and_intent_contradict_on_the_collapsed_arc() {
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

    // DEFECT, PINNED: at least one move is inside a LeadOut span while its
    // own intent says FinishingCut.
    let contradictory = covered
        .iter()
        .filter(|&&i| out.toolpath.moves[i].intent == MoveIntent::FinishingCut)
        .count();
    assert!(
        contradictory > 0,
        "F1 exhibit 3: expected at least one move inside a LeadOut span \
         whose intent reads FinishingCut; spans and intents already agree"
    );
}
