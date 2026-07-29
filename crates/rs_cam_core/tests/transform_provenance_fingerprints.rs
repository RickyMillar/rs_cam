//! C1 — refactor-invariance harness for the transform provenance contract.
//!
//! The C1 wave replaces ae10cb2's `SemanticLinkCarrier` convention (semantic
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
//! Every constant below was captured at HEAD 5d32150, BEFORE any C1 edit.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_arguments
)]

use rs_cam_core::{
    boundary::clip_annotated_to_boundary_set,
    compute::catalog::OperationType,
    compute::config::{DressupConfig, DressupEntryStyle},
    compute::execute::apply_dressups,
    dressup::optimize_entry_descents_annotated,
    geo::P3,
    polygon::Polygon2,
    semantic_trace::{ToolpathSemanticKind, ToolpathSemanticRecorder, ToolpathSemanticTrace},
    toolpath::Toolpath,
    toolpath_spans::{AnnotatedToolpath, Span, SpanKind},
    transform_provenance::ReconcileSet,
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
        6.0,
        /* safe_z */ 30.0,
        /* stock_top */ 0.0,
        None,
        None,
        None,
        OperationType::Adaptive3d.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::new(Some(&recorder)),
    );

    assert_eq!(
        fingerprint(&out.toolpath),
        (23, 14_756_822_782_673_573_601),
        "three_pass geometry moved; captured at HEAD 5d32150 before C1"
    );
    assert_eq!(
        link_sites(&recorder.finish()),
        expect_sites(&[
            ("head", Some((0, 4))),
            ("body", Some((5, 13))),
            ("tail", Some((14, 22))),
            ("whole", Some((0, 22))),
        ]),
        "three_pass semantic link landing sites moved; captured at HEAD 5d32150 before C1"
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
        6.0,
        /* safe_z */ 30.0,
        /* stock_top */ 0.0,
        None,
        None,
        None,
        OperationType::Adaptive3d.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::new(Some(&recorder)),
    );

    assert_eq!(
        fingerprint(&out.toolpath),
        (40, 9_877_459_821_106_430_315),
        "arc_raster geometry moved; captured at HEAD 5d32150 before C1"
    );
    assert_eq!(
        link_sites(&recorder.finish()),
        expect_sites(&[
            ("head", Some((0, 16))),
            ("body", Some((16, 27))),
            ("tail", Some((27, 39))),
            ("whole", Some((0, 39))),
        ]),
        "arc_raster semantic link landing sites moved; captured at HEAD 5d32150 before C1"
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
        6.0,
        /* safe_z */ 30.0,
        /* stock_top */ 0.0,
        None,
        None,
        None,
        OperationType::Face.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::new(Some(&recorder)),
    );
    assert_eq!(
        fingerprint(&current.toolpath),
        (74, 9_692_869_450_022_244_402),
        "face stage-1 (dressups) geometry moved; captured at HEAD 5d32150 before C1"
    );

    // Stage 2 — boundary clip against a rectangle that actually cuts the
    // path (inset from the faced area, so moves leave and re-enter).
    let boundary = Polygon2::rectangle(2.0, 2.0, 34.0, 26.0);
    current = clip_annotated_to_boundary_set(current, &[boundary], 30.0)
        .reconcile(&mut ReconcileSet::new(Some(&recorder)))
        .into_inner();
    assert_eq!(
        fingerprint(&current.toolpath),
        (97, 3_258_911_278_473_560_309),
        "face stage-2 (boundary clip) geometry moved; captured at HEAD 5d32150 before C1"
    );

    // Stage 3 — entry-descent split (no dexel stock: the fresh-stock top is
    // the ceiling, which is what the session passes for a first op).
    let (transformed, split_count) = optimize_entry_descents_annotated(current, None, 0.0, 3.0);
    current = transformed
        .reconcile(&mut ReconcileSet::new(Some(&recorder)))
        .into_inner();
    assert_eq!(
        (split_count, fingerprint(&current.toolpath)),
        (6, (103, 2_154_614_841_165_484_301)),
        "face stage-3 (entry-descent split) geometry moved; captured at HEAD 5d32150 before C1"
    );

    assert_eq!(
        link_sites(&recorder.finish()),
        expect_sites(&[
            ("head", Some((0, 32))),
            ("body", Some((33, 74))),
            ("tail", Some((75, 102))),
            ("whole", Some((0, 102))),
        ]),
        "face full-chain semantic link landing sites moved; captured at HEAD 5d32150 before C1"
    );
}
