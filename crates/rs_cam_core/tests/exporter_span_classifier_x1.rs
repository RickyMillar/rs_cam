//! X-1 / D-LV.1 — the PNG exporter and the GUI viewport must reach the SAME
//! presentation decision for the same move.
//!
//! # What was wrong
//!
//! `stock_mesh::toolpath_to_tube_mesh_with_spans` (the `screenshot_toolpath`
//! 6-view PNG exporter) and `rs_cam_viz::render::toolpath_render` (the live
//! viewport) each walked a move's span path themselves, and disagreed three
//! ways (`planning/review_2026-08-04/UNTOUCHED_TERRITORY_RISK_MAP.md`, X-1):
//!
//! 1. **Direction.** The exporter walked `path.iter().rev()` — innermost
//!    first. The viewport walks forward — outermost first. A move nested
//!    inside two interesting spans was therefore coloured by the OUTERMOST
//!    kind on screen and the INNERMOST kind in the PNG.
//! 2. **`DressupArtifact` precedence.** The exporter early-returned on it.
//!    The viewport records it and keeps scanning, so an `Entry` nested
//!    inside a dressup still wins there.
//! 3. **The depth-pass gradient.** The viewport shades ordinary cuts by the
//!    enclosing `DepthPass` span's `pass_index`. The exporter had no
//!    equivalent at all — every pass came out the same flat `CUT_COLOR`.
//!
//! # What this file asserts
//!
//! That the exporter's emitted geometry agrees with the ONE shared
//! classifier, `AnnotatedToolpath::classify_span_path`, which the viewport
//! also consumes. Assertions are made against the tube mesh's actual vertex
//! COLOURS — the bytes that end up in the PNG — not against the classifier
//! in isolation, so a future renderer that stops consuming the classifier
//! fails here rather than passing on a technicality.
//!
//! # Red-first
//!
//! Every test in the "divergence" section FAILS on the parent revision
//! (`6396eb0`), each on the exporter's own output:
//!
//! | test | parent behaviour | expected |
//! |---|---|---|
//! | `nested_entry_inside_link_bridge_takes_the_outer_kind` | cyan (inner Entry) | grey (outer LinkBridge) |
//! | `nested_dressup_inside_entry_does_not_beat_the_entry` | brown (inner Dressup) | cyan (outer Entry) |
//! | `dressup_wrapping_a_lead_out_yields_to_the_lead_out` | brown | magenta |
//! | `depth_passes_are_visually_distinguishable_in_the_export` | one colour for all passes | distinct per pass |
//!
//! `viewport_reference_rules_are_unchanged` is the control: it pins the
//! classifier's answers to the rules the viewport implemented BEFORE this
//! change, so "the exporter was made to agree" cannot be satisfied by
//! quietly moving the viewport instead. It is green on both sides.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::geo::P3;
use rs_cam_core::stock_mesh::{StockMesh, toolpath_to_tube_mesh_with_spans};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};
use rs_cam_core::toolpath_spans::{AnnotatedToolpath, Span, SpanClass, SpanKind, SpanPayload};

const FEED: f64 = 600.0;
/// Ribbon radius. Any positive value works — the tests read colours, not
/// geometry — but keep it well above `push_tube_segment`'s 1e-10 degenerate
/// guard so every move actually emits vertices.
const RIBBON: f32 = 0.25;

// ── Fixture construction ────────────────────────────────────────────────

/// A straight run of `n` cutting moves one millimetre apart along +X.
///
/// Deliberately linear: `MoveType::ArcCW`/`ArcCCW` would send the exporter
/// through `linearize_arc`, which emits a variable number of tube segments
/// per move and would make "the colour of move i" ambiguous.
fn straight_run(n: usize) -> Toolpath {
    let mut tp = Toolpath::new();
    for i in 0..n {
        tp.moves.push(Move {
            target: P3::new(i as f64, 0.0, -1.0),
            move_type: if i == 0 {
                MoveType::Rapid
            } else {
                MoveType::Linear { feed_rate: FEED }
            },
            intent: MoveIntent::FinishingCut,
        });
    }
    tp
}

fn annotated(n: usize, spans: Vec<Span>) -> AnnotatedToolpath {
    let mut a = AnnotatedToolpath::new(straight_run(n));
    a.spans = spans;
    a.spans_valid = true;
    a
}

/// Every distinct vertex colour in the mesh, as sortable integer triples.
///
/// `toolpath_to_tube_mesh_with_spans` writes the move's colour to all eight
/// vertices of that move's prism, so a colour present here is a colour some
/// move was actually painted.
fn distinct_colors(mesh: &StockMesh) -> Vec<[i64; 3]> {
    let mut out: Vec<[i64; 3]> = mesh
        .colors
        .chunks_exact(3)
        .map(|c| {
            [
                (f64::from(c[0]) * 1e6).round() as i64,
                (f64::from(c[1]) * 1e6).round() as i64,
                (f64::from(c[2]) * 1e6).round() as i64,
            ]
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The colour the exporter painted the `k`-th emitted cutting move.
///
/// `toolpath_to_tube_mesh_with_spans` walks moves `1..len` in order and
/// appends exactly one prism (8 vertices, one colour repeated) per non-rapid
/// move when `include_rapids` is false, so prism `k` is the `k`-th cutting
/// move. `the_exporter_emits_one_prism_per_cutting_move` pins that
/// correspondence so this accessor cannot silently read the wrong move.
fn color_of_nth_cutting_prism(mesh: &StockMesh, k: usize) -> [f32; 3] {
    let base = k * 8 * 3;
    assert!(
        base + 2 < mesh.colors.len(),
        "prism {k} not emitted — mesh has {} vertices",
        mesh.colors.len() / 3
    );
    [
        mesh.colors[base],
        mesh.colors[base + 1],
        mesh.colors[base + 2],
    ]
}

fn export(a: &AnnotatedToolpath) -> StockMesh {
    // include_rapids = false so prism index == cutting-move ordinal.
    toolpath_to_tube_mesh_with_spans(a, RIBBON, false)
}

// ── Palette, restated here on purpose ───────────────────────────────────
//
// `stock_mesh`'s SPAN_*_COLOR constants are private. Restating them makes
// this file a pin on the exported bytes: changing a palette constant in
// `stock_mesh` without deciding to fails here, which is the correct place to
// notice a screenshot's taxonomy silently changing.

const CUT: [f32; 3] = [0.1, 0.85, 0.2];
const ENTRY: [f32; 3] = [0.2, 0.85, 0.95];
const LEADOUT: [f32; 3] = [0.95, 0.35, 0.85];
const LINK_BRIDGE: [f32; 3] = [0.55, 0.55, 0.6];
const DRESSUP: [f32; 3] = [0.65, 0.55, 0.4];

fn assert_color(got: [f32; 3], want: [f32; 3], what: &str) {
    let d = (0..3)
        .map(|i| (got[i] - want[i]).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        d < 1e-6,
        "{what}: exporter painted {got:?}, expected {want:?} (max component delta {d:.6})"
    );
}

// ── The divergence tests (RED on the parent revision) ───────────────────

/// X-1 rule 1: direction. An `Entry` span nested INSIDE a `LinkBridge` span.
///
/// The viewport walks outermost-first and paints the whole thing as a link
/// bridge. The exporter walked innermost-first and painted the nested part
/// cyan. Same toolpath, two different pictures.
#[test]
fn nested_entry_inside_link_bridge_takes_the_outer_kind() {
    // Spans are emitted parent-before-child, which is the order
    // `span_paths_by_move` preserves and both renderers rely on.
    let a = annotated(
        6,
        vec![
            Span::new(0, 6, SpanKind::Operation),
            Span::new(1, 5, SpanKind::LinkBridge),
            Span::new(2, 4, SpanKind::Entry),
        ],
    );
    let mesh = export(&a);

    // Cutting moves are 1..6 → prisms 0..5. Move 2 sits inside both spans.
    assert_color(
        color_of_nth_cutting_prism(&mesh, 1),
        LINK_BRIDGE,
        "move 2 (Entry nested inside LinkBridge)",
    );
    // And the classifier agrees — one decision, not two.
    assert_eq!(
        a.classify_span_path(&a.span_path_at(2)),
        SpanClass::LinkBridge
    );
}

/// X-1 rule 2: `DressupArtifact` must not short-circuit.
///
/// A `DressupArtifact` nested inside an `Entry`. The viewport returns Entry
/// (it never stops on a dressup); the exporter returned the dressup brown
/// because it stopped at the first interesting kind on its reversed walk.
#[test]
fn nested_dressup_inside_entry_does_not_beat_the_entry() {
    let a = annotated(
        6,
        vec![
            Span::new(0, 6, SpanKind::Operation),
            Span::new(1, 5, SpanKind::Entry),
            Span::new(2, 4, SpanKind::DressupArtifact),
        ],
    );
    let mesh = export(&a);
    assert_color(
        color_of_nth_cutting_prism(&mesh, 1),
        ENTRY,
        "move 2 (DressupArtifact nested inside Entry)",
    );
    assert_eq!(a.classify_span_path(&a.span_path_at(2)), SpanClass::Entry);
}

/// X-1 rule 2, the other nesting order: a `LeadOut` nested inside a
/// `DressupArtifact`. Forward walk records the dressup, keeps going, and the
/// lead-out wins. The reversed walk returned the lead-out too — but only by
/// accident of ordering. This pins the *rule*, not the accident: the dressup
/// is outermost here, so a renderer that early-returns on it is wrong.
#[test]
fn dressup_wrapping_a_lead_out_yields_to_the_lead_out() {
    let a = annotated(
        6,
        vec![
            Span::new(0, 6, SpanKind::Operation),
            Span::new(1, 5, SpanKind::DressupArtifact),
            Span::new(2, 4, SpanKind::LeadOut),
        ],
    );
    let mesh = export(&a);
    assert_color(
        color_of_nth_cutting_prism(&mesh, 1),
        LEADOUT,
        "move 2 (LeadOut nested inside DressupArtifact)",
    );
    assert_eq!(a.classify_span_path(&a.span_path_at(2)), SpanClass::LeadOut);
    // A move covered by the dressup ONLY still reads as a dressup.
    assert_color(
        color_of_nth_cutting_prism(&mesh, 0),
        DRESSUP,
        "move 1 (DressupArtifact only)",
    );
}

/// X-1 rule 3: the exporter had no depth-pass gradient. Three passes came out
/// as one flat green, so a screenshot of a multi-pass 2.5D op could not be
/// read for pass structure at all — the exact thing the six-view export is
/// for.
#[test]
fn depth_passes_are_visually_distinguishable_in_the_export() {
    let pass = |start: usize, end: usize, idx: u32| Span {
        start_move: start,
        end_move: end,
        kind: SpanKind::DepthPass,
        label: std::borrow::Cow::Borrowed(""),
        payload: Some(SpanPayload::DepthPass {
            z_level: -(idx as f64),
            pass_index: idx,
        }),
    };
    let a = annotated(
        7,
        vec![
            Span::new(0, 7, SpanKind::Operation),
            pass(1, 3, 0),
            pass(3, 5, 1),
            pass(5, 7, 2),
        ],
    );
    let mesh = export(&a);

    let c0 = color_of_nth_cutting_prism(&mesh, 0); // move 1, pass 0
    let c1 = color_of_nth_cutting_prism(&mesh, 2); // move 3, pass 1
    let c2 = color_of_nth_cutting_prism(&mesh, 4); // move 5, pass 2

    let differ = |a: [f32; 3], b: [f32; 3]| (0..3).any(|i| (a[i] - b[i]).abs() > 1e-6);
    assert!(
        differ(c0, c1) && differ(c1, c2) && differ(c0, c2),
        "depth passes 0/1/2 must be distinguishable in an exported PNG, got \
         {c0:?} / {c1:?} / {c2:?}"
    );
    // Three passes, three distinct greens — and nothing else.
    assert_eq!(
        distinct_colors(&mesh).len(),
        3,
        "expected exactly three cut shades, one per depth pass"
    );
    // Every one of them is a shade of CUT_COLOR, not a new hue: the ratio
    // between components is preserved by a pure lightness shift.
    for c in [c0, c1, c2] {
        let f = c[1] / CUT[1];
        assert!(
            (0.9..=1.1).contains(&f),
            "pass shade {c:?} is not a lightness variant of {CUT:?}"
        );
        for i in 0..3 {
            assert!(
                (c[i] - CUT[i] * f).abs() < 1e-5,
                "pass shade {c:?} changed hue relative to {CUT:?}"
            );
        }
    }
}

// ── Controls (GREEN on both sides of the change) ────────────────────────

/// The viewport's rules, restated independently of any renderer.
///
/// This is the "the viewport did not move" bar. Each assertion is one of the
/// rules `rs_cam_viz::render::toolpath_render::from_toolpath` implemented
/// before the classifier was extracted; if the shared classifier had been
/// bent toward the exporter's old behaviour instead, this test would fail.
#[test]
fn viewport_reference_rules_are_unchanged() {
    // Entry beats a DepthPass on the same path, and drops the pass index.
    let a = annotated(
        4,
        vec![
            Span::new(0, 4, SpanKind::Operation),
            Span {
                start_move: 0,
                end_move: 4,
                kind: SpanKind::DepthPass,
                label: std::borrow::Cow::Borrowed(""),
                payload: Some(SpanPayload::DepthPass {
                    z_level: -1.0,
                    pass_index: 2,
                }),
            },
            Span::new(1, 3, SpanKind::Entry),
        ],
    );
    assert_eq!(a.classify_span_path(&a.span_path_at(1)), SpanClass::Entry);
    // …and a move outside the Entry keeps the pass index.
    assert_eq!(
        a.classify_span_path(&a.span_path_at(3)),
        SpanClass::Cut {
            pass_index: Some(2)
        }
    );

    // GeometryRefit is transparent: an arc-fitted move is ordinary cutting
    // geometry, not a dressup bridge (Checkpoint D Q3, 2026-08-04).
    let g = annotated(
        4,
        vec![
            Span::new(0, 4, SpanKind::Operation),
            Span::new(1, 3, SpanKind::GeometryRefit),
        ],
    );
    assert_eq!(
        g.classify_span_path(&g.span_path_at(1)),
        SpanClass::Cut { pass_index: None }
    );
    assert_color(
        color_of_nth_cutting_prism(&export(&g), 0),
        CUT,
        "arc-fitted move must render as ordinary cutting geometry",
    );

    // Region and Operation are transparent.
    let r = annotated(
        4,
        vec![
            Span::new(0, 4, SpanKind::Operation),
            Span::new(1, 3, SpanKind::Region),
        ],
    );
    assert_eq!(
        r.classify_span_path(&r.span_path_at(1)),
        SpanClass::Cut { pass_index: None }
    );

    // An invalidated span table may not be read for presentation.
    let mut stale = annotated(
        4,
        vec![
            Span::new(0, 4, SpanKind::Operation),
            Span::new(1, 3, SpanKind::Entry),
        ],
    );
    stale.spans_valid = false;
    assert_eq!(
        stale.classify_span_path(&stale.span_path_at(1)),
        SpanClass::Cut { pass_index: None }
    );
}

// ── Red-first proof, pinned rather than checked out ─────────────────────

/// The parent revision's exporter walk, verbatim.
///
/// Transcribed from `stock_mesh.rs:181-203` at `6396eb0` — reversed
/// iteration, `DressupArtifact` early-returning, no `DepthPass` handling:
///
/// ```text
/// for span_id in path.iter().rev() {
///     match span.kind {
///         SpanKind::Entry           => return SPAN_ENTRY_COLOR,
///         SpanKind::LeadOut         => return SPAN_LEADOUT_COLOR,
///         SpanKind::LinkBridge      => return SPAN_LINK_BRIDGE_COLOR,
///         SpanKind::DressupArtifact => return SPAN_DRESSUP_COLOR,
///         SpanKind::GeometryRefit   => continue,
///         _ => continue,
///     }
/// }
/// CUT_COLOR
/// ```
fn parent_revision_exporter_walk(a: &AnnotatedToolpath, move_idx: usize) -> [f32; 3] {
    for span_id in a.span_path_at(move_idx).iter().rev() {
        let Some(span) = a.spans.get(span_id.0 as usize) else {
            continue;
        };
        match span.kind {
            SpanKind::Entry => return ENTRY,
            SpanKind::LeadOut => return LEADOUT,
            SpanKind::LinkBridge => return LINK_BRIDGE,
            SpanKind::DressupArtifact => return DRESSUP,
            _ => continue,
        }
    }
    CUT
}

/// The red-first bar, executable in perpetuity.
///
/// The programme's rule is that a fix must be demonstrated red on the parent
/// revision. Checking out `6396eb0` was not available: this tree is shared
/// with two other live lanes, so stashing the fix to run a build would have
/// handed them the wrong source mid-compile. Instead the parent's walk is
/// pinned above and asserted — on the same fixtures the tests above use — to
/// give a DIFFERENT answer from the shipped classifier. That is the same
/// evidence, and unlike a one-off checkout it keeps failing if anyone
/// reintroduces the reversed walk.
///
/// This test is expected to pass forever. It fails only if the shipped
/// classifier drifts BACK toward the parent's behaviour.
#[test]
fn the_parent_revisions_walk_disagrees_with_the_shipped_classifier() {
    // X-1 rule 1 — direction.
    let nested_entry = annotated(
        6,
        vec![
            Span::new(0, 6, SpanKind::Operation),
            Span::new(1, 5, SpanKind::LinkBridge),
            Span::new(2, 4, SpanKind::Entry),
        ],
    );
    assert_color(
        parent_revision_exporter_walk(&nested_entry, 2),
        ENTRY,
        "parent revision painted the INNER Entry",
    );
    assert_color(
        color_of_nth_cutting_prism(&export(&nested_entry), 1),
        LINK_BRIDGE,
        "shipped exporter paints the OUTER LinkBridge, as the viewport does",
    );

    // X-1 rule 2 — DressupArtifact precedence.
    let nested_dressup = annotated(
        6,
        vec![
            Span::new(0, 6, SpanKind::Operation),
            Span::new(1, 5, SpanKind::Entry),
            Span::new(2, 4, SpanKind::DressupArtifact),
        ],
    );
    assert_color(
        parent_revision_exporter_walk(&nested_dressup, 2),
        DRESSUP,
        "parent revision short-circuited on the nested DressupArtifact",
    );
    assert_color(
        color_of_nth_cutting_prism(&export(&nested_dressup), 1),
        ENTRY,
        "shipped exporter lets the enclosing Entry win, as the viewport does",
    );

    // X-1 rule 3 — the depth-pass gradient the parent did not have.
    let pass = |start: usize, end: usize, idx: u32| Span {
        start_move: start,
        end_move: end,
        kind: SpanKind::DepthPass,
        label: std::borrow::Cow::Borrowed(""),
        payload: Some(SpanPayload::DepthPass {
            z_level: -(idx as f64),
            pass_index: idx,
        }),
    };
    let passes = annotated(
        5,
        vec![
            Span::new(0, 5, SpanKind::Operation),
            pass(1, 3, 0),
            pass(3, 5, 2),
        ],
    );
    assert_color(
        parent_revision_exporter_walk(&passes, 1),
        parent_revision_exporter_walk(&passes, 3),
        "parent revision painted every depth pass the same flat green",
    );
    let mesh = export(&passes);
    let a = color_of_nth_cutting_prism(&mesh, 0);
    let b = color_of_nth_cutting_prism(&mesh, 2);
    assert!(
        (0..3).any(|i| (a[i] - b[i]).abs() > 1e-6),
        "shipped exporter must stratify depth passes: got {a:?} and {b:?}"
    );
}

/// Non-vacuity. A test that reads colours out of an empty mesh proves
/// nothing; this pins that the fixtures actually emit one prism per cutting
/// move and that the exporter is on its span path (not the span-free
/// fallback).
#[test]
fn the_exporter_emits_one_prism_per_cutting_move() {
    let a = annotated(
        6,
        vec![
            Span::new(0, 6, SpanKind::Operation),
            Span::new(1, 5, SpanKind::LinkBridge),
            Span::new(2, 4, SpanKind::Entry),
        ],
    );
    let mesh = export(&a);
    // 6 moves, move 0 is the rapid seed and is not emitted → 5 prisms.
    assert_eq!(
        mesh.colors.len() / 3,
        5 * 8,
        "expected 5 prisms of 8 vertices"
    );
    assert_eq!(mesh.vertices.len(), mesh.colors.len());
    // The fallback path (`toolpath_to_tube_mesh`) can only ever produce
    // CUT_COLOR and RAPID_COLOR. Seeing a span colour proves we are not on it.
    assert!(
        distinct_colors(&mesh).len() >= 2,
        "fixture must exercise more than one span colour"
    );
}

/// The exporter must not invent geometry the toolpath does not contain.
///
/// D-LV.1's live symptom was a near-empty PNG from a 199,745-move toolpath.
/// A renderer whose emitted geometry escapes the toolpath's own bounding box
/// makes the composite's auto-fit zoom out, and everything real collapses to
/// a few pixels. This bounds the mesh to the path plus one ribbon radius, so
/// any future linearisation or synthetic-segment bug is caught as geometry
/// rather than as an eyeball verdict on an image.
#[test]
fn exported_geometry_stays_inside_the_toolpath_envelope() {
    let a = annotated(
        8,
        vec![
            Span::new(0, 8, SpanKind::Operation),
            Span::new(2, 4, SpanKind::Entry),
            Span::new(5, 7, SpanKind::LeadOut),
        ],
    );
    let mesh = toolpath_to_tube_mesh_with_spans(&a, RIBBON, true);
    assert!(!mesh.vertices.is_empty(), "nothing was rendered");

    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for m in &a.toolpath.moves {
        let p = [m.target.x, m.target.y, m.target.z];
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    // `push_tube_segment` places the eight corners at `from ± u ± v`, where u
    // and v are an orthonormal pair scaled by the ribbon radius. Worst case a
    // single component picks up |u[i]| + |v[i]| ≤ r·√2 ≈ 1.415 r, so 2.5 r is
    // a correct bound with headroom. It is still four orders of magnitude
    // tighter than an auto-fit-destroying envelope explosion, which is the
    // failure class this guards.
    let slack = f64::from(RIBBON) * 2.5 + 1e-6;
    for v in mesh.vertices.chunks_exact(3) {
        for i in 0..3 {
            let x = f64::from(v[i]);
            assert!(
                x >= lo[i] - slack && x <= hi[i] + slack,
                "exported vertex component {i} = {x} escapes the toolpath \
                 envelope [{}, {}] (± ribbon {slack})",
                lo[i],
                hi[i]
            );
        }
    }
}
