//! C1 — refactor-invariance harness for the transform provenance contract.
//!
//! The C1 wave replaces f375045's `SemanticLinkCarrier` convention (semantic
//! move links ride the span vector through each transform's private
//! `MoveRemap`) with a compile-enforced typestate: a transform hands back a
//! `Transformed<Unreconciled>` and the only route to the toolpath inside is
//! `.reconcile(&mut ReconcileSet)`.
//!
//! That is a **refactor**: the shipped toolpath must not move by one byte,
//! and every index-carrying channel must land on exactly the same indices it
//! landed on before. This file pins both halves:
//!
//! 1. **Geometry** — FNV-1a over the `Debug` rendering of the move list
//!    (`finish_resolution_policy_pr3.rs`'s pattern; NOT `DefaultHasher`,
//!    whose algorithm is explicitly unstable across toolchains).
//! 2. **Channel landing sites** — the post-pipeline `(label, move_start,
//!    move_end)` of every semantic item. This is the direct oracle for the
//!    carrier retirement: if the typestate's provenance disagrees with what
//!    the carrier spans used to produce, these change.
//!
//! Three configurations, each running a different mix of transforms:
//!
//! | Fixture | Transforms exercised |
//! |---------|----------------------|
//! | `three_pass` | barriered TSP + ramp entry + dogbones + leads + link moves + arc fit + segment merge |
//! | `arc_raster` | same pipeline over collinear/arc-shaped runs (arc fit + segment merge actually fire) |
//! | `face_full_chain` | real `face` op → full dressups → boundary clip → entry-descent split |
//!
//! Every constant below was ORIGINALLY captured at HEAD 4787421, BEFORE any
//! C1 edit. Two re-pins have happened since; each names its own mechanism.
//!
//! ## Pin lineage
//!
//! | Wave | Commit | What moved | Which pins |
//! |------|--------|-----------|------------|
//! | C1 capture | `4787421` | — (original capture) | all five |
//! | PR-6 (H2.2 / Checkpoint F1) | `96717b0` | arc-fit's run key gained `intent`, so four dressup-inserted `LeadOut` segments stopped being swallowed into an `Unknown`-labelled arc — LABEL only | `arc_raster` |
//! | W8 / F23-impl (Checkpoints F2+F3, ruled 2026-08-06) | `f86a38c` | the closing retract now lifts from the lead-out ARC endpoint instead of the stale cut endpoint — XY of N rapids per fixture | **all five** |
//!
//! `f86a38c`'s move went un-re-pinned for eight days: that lane re-pinned
//! `crease_own_region_pr6b` in its own preceding commit (`3f2e84e`) but did not
//! re-run this suite, and TD2's closing green claim over the core tests hit the
//! first-failing-binary trap, so the red was never surfaced. Re-pinned under TD3
//! intake row **G-XFP** on 2026-08-14 with the archaeology recorded in
//! `planning/review_2026-08-08/ORCHESTRATION_LOG.md` §3.1.
//!
//! **The link sites did not move under the first two re-pins.** That was the
//! load-bearing half: `f86a38c` is a geometry fix, and the semantic-channel
//! landing sites this file exists to guard were byte-identical across it. Only
//! the geometry hashes moved, and every move COUNT (23 / 40 / 74 / 97 / 103) and
//! the stage-3 `split_count` (6) held.
//!
//! **The third re-pin (2026-09-09, G-ISOCLIPRAPID) DOES move the face counts and
//! its link sites**, and that is the correct outcome, not a regression: the fix
//! INSERTS a move — a pure vertical lift before each lead-in's pre-position
//! traverse — so the face chain goes 74 / 97 / 103 to 80 / 103 / 109 and every
//! landing site slides by the number of lifts ahead of it. What the assertion
//! still guards is what it always guarded: the four sites tile the whole path
//! contiguously, and `split_count` is unchanged at 6. The arc-raster and
//! three-pass fixtures carry no lead-in and did not move at all.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_arguments
)]

use rs_cam_core::{
    compute::catalog::OperationType,
    compute::config::{DressupConfig, DressupEntryStyle},
    compute::execute::apply_dressups,
    dressup::optimize_entry_descents_annotated,
    geo::P3,
    geometry::boundary::clip_annotated_to_boundary_set,
    polygon::Polygon2,
    toolpath::Toolpath,
    trace::semantic_trace::{
        ToolpathSemanticKind, ToolpathSemanticRecorder, ToolpathSemanticTrace,
    },
    trace::toolpath_spans::{AnnotatedToolpath, Span, SpanKind},
    trace::transform_provenance::ReconcileSet,
};

// ── Fingerprints ─────────────────────────────────────────────────────────

/// Byte-level fingerprint of an emitted toolpath: move count plus a hash of
/// the `Debug` rendering (which round-trips every f64 exactly).
fn fingerprint(tp: &Toolpath) -> (usize, u64) {
    // FNV-1a, not `DefaultHasher`: the pinned constants must survive a
    // toolchain bump, and `DefaultHasher`'s algorithm is explicitly unstable.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in format!("{:?}", tp.moves).bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (tp.moves.len(), h)
}

/// Where every semantic item ended up: `(label, move_start, move_end)`, with
/// `move_end` INCLUSIVE as stored. `None` = the item was unlinked by the
/// deleted-move policy.
fn link_sites(trace: &ToolpathSemanticTrace) -> Vec<(String, Option<(usize, usize)>)> {
    let mut sites: Vec<(String, Option<(usize, usize)>)> = trace
        .items
        .iter()
        .map(|item| (item.label.clone(), item.move_start.zip(item.move_end)))
        .collect();
    sites.sort();
    sites
}

fn expect_sites(pairs: &[(&str, Option<(usize, usize)>)]) -> Vec<(String, Option<(usize, usize)>)> {
    let mut v: Vec<(String, Option<(usize, usize)>)> = pairs
        .iter()
        .map(|(label, link)| ((*label).to_owned(), *link))
        .collect();
    v.sort();
    v
}

// ── Fixtures ─────────────────────────────────────────────────────────────

/// 3-pass synthetic toolpath: 3 cutting strokes at descending Z separated by
/// retract → reposition → plunge, with an Operation span, per-pass DepthPass
/// spans and `RapidOrderBarrier` boundaries (so the BARRIERED TSP arm runs).
///
/// Same shape as `dressup_span_invariants.rs`'s fixture — deliberately, so a
/// change that trips one trips the other.
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

/// Serpentine raster whose rows are dense polylines around a circular arc —
/// gives `fit_arcs` and `merge_linear_runs` something to actually collapse,
/// so the N-to-1 provenance path is exercised (not just insertions).
fn arc_raster() -> AnnotatedToolpath {
    let mut tp = Toolpath::new();
    let mut region_starts = Vec::new();
    for row in 0..4 {
        let y0 = row as f64 * 4.0;
        let z = -1.0 - row as f64;
        tp.rapid_to(P3::new(0.0, y0, 10.0));
        region_starts.push(tp.moves.len());
        tp.feed_to(P3::new(0.0, y0, z), 400.0);
        // 24-segment half circle of radius 8 centred at (10, y0).
        for step in 0..=24 {
            let theta = std::f64::consts::PI * (step as f64) / 24.0;
            tp.feed_to(
                P3::new(10.0 - 8.0 * theta.cos(), y0 + 8.0 * theta.sin(), z),
                1200.0,
            );
        }
        // Long straight run split into many tiny collinear segments.
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

/// Real `face` op output — a generated (not hand-built) toolpath, so the
/// harness is not purely synthetic.
fn face_fixture() -> AnnotatedToolpath {
    use rs_cam_core::geo::BoundingBox3;
    use rs_cam_core::ops::face::{FaceParams, face_toolpath};

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
        direction: rs_cam_core::ops::face::FaceDirection::OneWay,
        stock_top_z: 0.0,
    };
    let raw = face_toolpath(&bbox, &params);
    let n = raw.moves.len();
    AnnotatedToolpath::with_spans(raw, vec![Span::new(0, n, SpanKind::Operation)])
}

/// Everything the pipeline can do, so a provenance regression in any step
/// shows up.
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

/// Record three generation-time semantic items over disjoint thirds of the
/// input, then hand the recorder to the pipeline as the link channel — the
/// exact wiring `ProjectSession::generate_toolpath` uses.
fn seed_items(recorder: &ToolpathSemanticRecorder, tp: &Toolpath) {
    let n = tp.moves.len();
    let root = recorder.root_context();
    let thirds = [
        ("head", 0, n / 3),
        ("body", n / 3, (2 * n) / 3),
        ("tail", (2 * n) / 3, n),
    ];
    for (label, start, end) in thirds {
        let scope = root.start_item(ToolpathSemanticKind::Region, label);
        scope.bind_to_toolpath(tp, start, end);
    }
    // A whole-path item: must survive every transform (an Operation-shaped
    // claim is the one thing a permutation cannot scatter).
    let whole = root.start_item(ToolpathSemanticKind::Pass, "whole");
    whole.bind_to_toolpath(tp, 0, n);
}

// ── Config 1: synthetic three-pass, full dressups ────────────────────────

#[test]
fn three_pass_full_dressups_fingerprint() {
    let input = three_pass();
    let recorder = ToolpathSemanticRecorder::new("three_pass", "Face");
    seed_items(&recorder, &input.toolpath);

    let out = apply_dressups(
        input,
        &full_dressups(),
        1000.0,
        // WP22: no operation in scope, so the plunge cap does not apply.
        None,
        6.0,
        /* safe_z */ 30.0,
        /* stock_top */ 0.0,
        None,
        None,
        None,
        None,
        OperationType::Adaptive3d.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::new(Some(&recorder), None),
    );

    // RE-PINNED 2026-08-14 (TD3 / G-XFP), mechanism `f86a38c` (W8 / F23-impl,
    // Checkpoints F2+F3 ruled 2026-08-06 — "the closing retract must lift from
    // where the tool IS"). Was `(23, 14_756_822_782_673_573_601)`.
    //
    // Mechanism: move count unchanged at 23; exactly THREE moves differ, the
    // closing rapid of each of the three passes (indices 4, 13, 22). Each was
    // emitted by the fixture at the cut endpoint and is now lifted from the
    // lead-out arc's own endpoint — XY only, Z and intent untouched:
    //
    //     move  4  [20.0, 0.0, 10.0] -> [21.5, 1.5, 10.0]
    //     move 13  [21.0, 0.0, 10.0] -> [22.5, 1.5, 10.0]
    //     move 22  [22.0, 0.0, 10.0] -> [23.5, 1.5, 10.0]
    //
    // The +(1.5, 1.5) is `full_dressups()`'s `lead_radius`. Link sites below
    // are UNCHANGED.
    assert_eq!(
        fingerprint(&out.toolpath),
        (23, 14_265_253_333_427_783_116),
        "three_pass geometry moved; re-pinned 2026-08-14 for the lead-out retract \
         lift (f86a38c), previously re-pinned by PR-6, originally captured at \
         HEAD 4787421 before C1"
    );
    assert_eq!(
        link_sites(&recorder.finish()),
        expect_sites(&[
            ("head", Some((0, 4))),
            ("body", Some((5, 13))),
            ("tail", Some((14, 22))),
            ("whole", Some((0, 22))),
        ]),
        "three_pass semantic link landing sites moved; captured at HEAD 4787421 before C1"
    );
}

// ── Config 2: arc/collinear raster, full dressups ────────────────────────

#[test]
fn arc_raster_full_dressups_fingerprint() {
    let input = arc_raster();
    let recorder = ToolpathSemanticRecorder::new("arc_raster", "Adaptive3d");
    seed_items(&recorder, &input.toolpath);

    let out = apply_dressups(
        input,
        &full_dressups(),
        1200.0,
        // WP22: no operation in scope, so the plunge cap does not apply.
        None,
        6.0,
        /* safe_z */ 30.0,
        /* stock_top */ 0.0,
        None,
        None,
        None,
        None,
        OperationType::Adaptive3d.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::new(Some(&recorder), None),
    );

    // RE-PINNED by PR-6 (H2.2 / Checkpoint F1), build `60316ec` + the arcfit
    // intent-key change. Was `(40, 9_877_459_821_106_430_315)`.
    //
    // Mechanism: the move COUNT is unchanged at 40 and no coordinate moved —
    // the hash is over the `Debug` rendering, which includes `Move::intent`,
    // and four moves changed LABEL only. `arc_raster`'s cut body is built with
    // `Toolpath::feed_to` (intent `Unknown`); with `intent` now in arc-fit's
    // run key the four dressup-inserted `LeadOut` segments are no longer
    // swallowed into an `Unknown`-labelled arc. Intent histogram, before →
    // after: `Unknown` 24 → 20, `LeadOut` 0 → 4; everything else identical.
    // This is exactly the relabelling H2.2 exists to stop, and it costs zero
    // arcs (8 before, 8 after) and zero moves.
    //
    // The other four pinned constants in this file did NOT move at PR-6:
    // `three_pass` and all three `face_full_chain` stages were byte-identical.
    //
    // RE-PINNED AGAIN 2026-08-14 (TD3 / G-XFP), mechanism `f86a38c` (W8 /
    // F23-impl, Checkpoints F2+F3 ruled 2026-08-06 — the lead-out retract lift).
    // Was `(40, 5_428_414_886_474_768_522)`.
    //
    // Mechanism: move count still 40; exactly FOUR moves differ, the closing
    // rapid of each of the four raster rows (indices 9, 19, 29, 39), lifted from
    // the lead-out arc endpoint instead of the stale cut endpoint — XY only:
    //
    //     move  9  [22.0,  0.0, 10.0] -> [23.5,  1.5, 10.0]
    //     move 19  [22.0,  4.0, 10.0] -> [23.5,  5.5, 10.0]
    //     move 29  [22.0,  8.0, 10.0] -> [23.5,  9.5, 10.0]
    //     move 39  [22.0, 12.0, 10.0] -> [23.5, 13.5, 10.0]
    //
    // Link sites below were UNCHANGED under the first two re-pins. The THIRD
    // one moves them, by a shift that is itself the measurement — see below.
    //
    // THIRD RE-PIN, 2026-09-10 (J2, G-RAMPCONTAIN): (40, 1344905273783580007)
    // -> (48, 10027966985113258553). A prism ramp now folds along the
    // operation's own following cut instead of drawing two blind straight
    // legs, and this fixture's following cut is an ARC raster, so each of the
    // four entries rides the arc and `fit_arcs` collapses it: EntryRamp 8 ->
    // 16, arcs 8 -> 16, eight added moves. The sibling pin in
    // `arcfit_intent_key_cost_f1` carries the same fingerprint and the seam
    // census beside it. The two fixtures in that file whose following cut is
    // STRAIGHT are byte-identical under the same change.
    assert_eq!(
        fingerprint(&out.toolpath),
        (48, 10_027_966_985_113_258_553),
        "arc_raster geometry moved; re-pinned 2026-09-10 for the G-RAMPCONTAIN \
         ramp fold, 2026-08-14 for the lead-out retract lift (f86a38c), before \
         that by PR-6 (arcfit intent key), originally captured at HEAD 4787421 \
         before C1"
    );
    // RE-PINNED 2026-09-10 (J2, G-RAMPCONTAIN). The ramp fold adds two moves
    // at each of this fixture's four entries, and the landing sites shift by
    // exactly the number of added moves that precede each one:
    //
    //     head   (0, 16) -> (0, 20)     +0 start, +4 end   (2 entries before its end)
    //     body  (16, 27) -> (20, 33)    +4 start, +6 end   (a third entry inside it)
    //     tail  (27, 39) -> (33, 47)    +6 start, +8 end   (the fourth)
    //     whole  (0, 39) -> (0, 47)     +0 start, +8 end   (all four)
    //
    // Nothing here is a re-pin of convenience: a 1 -> K entry expansion MUST
    // carry the spans after it forward by K - 1, and this arithmetic is the
    // provenance remap being right. A shift that did NOT accumulate in
    // entry order would be the defect.
    assert_eq!(
        link_sites(&recorder.finish()),
        expect_sites(&[
            ("head", Some((0, 20))),
            ("body", Some((20, 33))),
            ("tail", Some((33, 47))),
            ("whole", Some((0, 47))),
        ]),
        "arc_raster semantic link landing sites moved; captured at HEAD 4787421 before C1"
    );
}

// ── Config 3: real face op → dressups → clip → descent split ─────────────

#[test]
fn face_full_chain_fingerprint() {
    let input = face_fixture();
    let recorder = ToolpathSemanticRecorder::new("face", "Face");
    seed_items(&recorder, &input.toolpath);

    // Stage 1 — dressups.
    let mut current = apply_dressups(
        input,
        &full_dressups(),
        1500.0,
        // WP22: no operation in scope, so the plunge cap does not apply.
        None,
        6.0,
        /* safe_z */ 30.0,
        /* stock_top */ 0.0,
        None,
        None,
        None,
        None,
        OperationType::Face.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::new(Some(&recorder), None),
    );
    // RE-PINNED 2026-09-09 (G-ISOCLIPRAPID), mechanism: the lead-in's
    // pre-position rapid now goes to the operation's retract plane instead of
    // the height the move before the plunge stopped at, and it LIFTS before it
    // traverses. Was `(74, 8_357_027_825_945_903_145)`.
    //
    // Mechanism, verified move by move on this fixture: SIX `Retract` rapids
    // are inserted — indices 2, 14, 26, 38, 50, 62, one per faced row that has
    // a lead-in — each a pure vertical lift `[3.0, y, 2.0]` -> `[3.0, y, 30.0]`
    // from where the generator's descent left the tool; and the six `LeadIn`
    // pre-position rapids that follow them move from Z 2.0 to Z 30.0, XY
    // unchanged. Nothing else differs, and the move count goes 74 -> 80.
    //
    // RE-PINNED 2026-08-14 (TD3 / G-XFP), mechanism `f86a38c` (W8 / F23-impl,
    // Checkpoints F2+F3, the lead-out retract lift). Was
    // `(74, 9_692_869_450_022_244_402)`.
    //
    // Mechanism: move count unchanged at 74; exactly SEVEN moves differ —
    // indices 10, 21, 32, 43, 54, 65, 73, the closing `Retract` rapid of each
    // faced row, `[37.0, y, 30.0]` -> `[38.5, y + 1.5, 30.0]`. XY only; the
    // `Retract` intent and the 30.0 safe-Z are untouched.
    assert_eq!(
        fingerprint(&current.toolpath),
        (80, 6_526_175_378_945_769_562),
        "face stage-1 (dressups) geometry moved; re-pinned 2026-08-14 for the \
         lead-out retract lift (f86a38c), originally captured at HEAD 4787421 \
         before C1"
    );

    // Stage 2 — boundary clip against a rectangle that actually cuts the
    // path (inset from the faced area, so moves leave and re-enter).
    let boundary = Polygon2::rectangle(2.0, 2.0, 34.0, 26.0);
    // G-BOUNDARYPLUNGE: `None` keeps the re-entry at the crossing move's cut
    // feed, so the pinned fingerprint below still describes the same motion.
    current = clip_annotated_to_boundary_set(current, &[boundary], 30.0, None)
        .reconcile(&mut ReconcileSet::new(Some(&recorder), None))
        .into_inner();
    // RE-PINNED 2026-09-09 (G-ISOCLIPRAPID): stage 1's six inserted lifts carry
    // forward, 97 -> 103. Was `(97, 7_877_196_034_056_840_142)`.
    assert_eq!(
        fingerprint(&current.toolpath),
        (103, 10_899_331_192_678_125_387),
        "face stage-2 (boundary clip) geometry moved; re-pinned 2026-09-09 for the \
         lead-in retract-plane lift (G-ISOCLIPRAPID) carried forward from stage 1, \
         originally captured at HEAD 4787421 before C1"
    );

    // Stage 3 — entry-descent split (no dexel stock: the fresh-stock top is
    // the ceiling, which is what the session passes for a first op).
    //
    // B2: the cutter is a FLAT endmill of exactly the 3.0 mm search radius,
    // so `height_at_radius` is `Some(0.0)` throughout the envelope and the
    // profile-aware descent target reduces to the flat-disc one these
    // constants were pinned against. They must not move.
    let (transformed, split_count) = optimize_entry_descents_annotated(
        current,
        None,
        0.0,
        3.0,
        &rs_cam_core::tool::FlatEndmill::new(6.0, 25.0),
        None,
    );
    current = transformed
        .reconcile(&mut ReconcileSet::new(Some(&recorder), None))
        .into_inner();
    // RE-PINNED 2026-09-09 (G-ISOCLIPRAPID): stage 1's six inserted lifts carry
    // forward, 103 -> 109. The split COUNT is unchanged at 6 — the lift is a
    // rapid and never creates or removes an entry descent. Was
    // `(6, (103, 3_086_279_569_100_738_182))`.
    assert_eq!(
        (split_count, fingerprint(&current.toolpath)),
        (6, (109, 2_717_567_159_789_683_815)),
        "face stage-3 (entry-descent split) geometry moved; re-pinned 2026-09-09 for \
         the lead-in retract-plane lift (G-ISOCLIPRAPID) carried forward from stage 1, \
         originally captured at HEAD 4787421 before C1"
    );

    assert_eq!(
        link_sites(&recorder.finish()),
        // RE-PINNED 2026-09-09 (G-ISOCLIPRAPID): the six inserted lifts push
        // every landing site out by the number of them that precede it — two
        // in the head, four more by the tail — and the four sites still tile
        // 0..=108 contiguously with no gap and no overlap, which is the
        // property this assertion is really about. Was `head (0, 32)`,
        // `body (33, 74)`, `tail (75, 102)`, `whole (0, 102)`.
        expect_sites(&[
            ("head", Some((0, 34))),
            ("body", Some((35, 79))),
            ("tail", Some((80, 108))),
            ("whole", Some((0, 108))),
        ]),
        "face full-chain semantic link landing sites moved; re-pinned 2026-09-09 for \
         the lead-in retract-plane lift (G-ISOCLIPRAPID), originally captured at HEAD \
         4787421 before C1"
    );
}
