//! P5c (feeds contract, operator review of P5b on 2026-11-04, and the live
//! review of P5c, round 2): the envelope plot shows only what limits the
//! cut, and a drag moves the setpoint, never the recommendation.
//!
//! # The claims
//!
//! 1. A drag moves the white dot (the setpoint draft); a hollow
//!    `TEXT_MUTED` ghost marks the stored point while the candidate exists,
//!    and Esc puts the dot back. No mark that moves is blue: blue is only
//!    the proposal diamond (P5d D3: quiet ACCENT, drawn when the proposal is
//!    not at the setpoint), and it stays where it is during the drag (operator: "dragging
//!    the dot drags the recommendation"; round 2: "I still see the
//!    recommendation move (blue circle) not the setpoint (white)").
//! 2. A drag that leaves the plot holds the candidate on the frame edge.
//!    The view range does not change, and the scroll offset of the
//!    inspector's scroll area does not change (item 2: "it just fires into
//!    the void").
//! 3. The frame is the hard stops: the width axis ends at the tool
//!    diameter, the depth axis never goes past the cutting length or the
//!    flute guard. No line is drawn for these limits (items 3, 4).
//! 4. On the rivmap100 3D Rough envelope (the operator's review job), each
//!    drawn line has ONE label, its short name; no line is DANGER red
//!    (items 5 to 7). P5d D1 (2026-11-04) reversed the P5c rule that drew
//!    only a limit near a mark: every limit in view draws (3 lines here),
//!    the binding one `TEXT_BODY`, the others `TEXT_MUTED`.
//!
//! Red before P5c: `EnvelopeScene` has no `candidate`, `proposal_shown` or
//! `edge_color`; the P5b view range followed the candidate, so a drag at the
//! frame edge grew the axes on each frame.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod envelope_k2b;

use envelope_k2b::{ID, ctx, envelope_for, envelope_of, frame, input, solid, texts};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::feeds::envelope::{CutPoint, Edge, LimitKind, Placement, SafeEnvelope};
use rs_cam_viz::state::panels::EnvelopeCandidates;
use rs_cam_viz::ui::feeds::envelope::{
    EnvelopeResponse, EnvelopeScene, EnvelopeShape, draw_envelope, recommendation_color,
};
use rs_cam_viz::ui::tokens;

/// The marks a frame painted: the filled circles (dots), the unfilled
/// circles (rings: the ghost) and the closed four-point paths (diamonds),
/// each with its centre and colour.
#[derive(Debug, Default)]
struct Marks {
    dots: Vec<(egui::Pos2, egui::Color32)>,
    rings: Vec<(egui::Pos2, egui::Color32)>,
    diamonds: Vec<(egui::Pos2, egui::Color32)>,
}

impl Marks {
    /// No circle is blue: blue marks only the recommendation.
    fn no_blue_circle(&self) -> bool {
        self.dots
            .iter()
            .chain(&self.rings)
            .all(|(_, c)| *c != tokens::ACCENT && *c != recommendation_color())
    }
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn marks(shapes: &[egui::epaint::ClippedShape]) -> Marks {
    let mut out = Marks::default();
    let mut walk = |s: &egui::Shape| match s {
        egui::Shape::Circle(c) if c.fill == egui::Color32::TRANSPARENT => {
            out.rings.push((c.center, c.stroke.color));
        }
        egui::Shape::Circle(c) => out.dots.push((c.center, c.fill)),
        egui::Shape::Path(p)
            if p.closed
                && p.points.len() == 4
                && solid(&p.stroke.color) != egui::Color32::TRANSPARENT =>
        {
            let n = p.points.len() as f32;
            let c = p
                .points
                .iter()
                .fold(egui::Vec2::ZERO, |acc, q| acc + q.to_vec2())
                / n;
            out.diamonds.push((c.to_pos2(), solid(&p.stroke.color)));
        }
        _ => {}
    };
    for c in shapes {
        if let egui::Shape::Vec(v) = &c.shape {
            v.iter().for_each(&mut walk);
        } else {
            walk(&c.shape);
        }
    }
    out
}

/// The line colours a frame painted (egui_plot draws a line as a `Path` or
/// a `LineSegment`): every stroke colour of an open path.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn line_colours(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::Color32> {
    let mut out = Vec::new();
    let mut walk = |s: &egui::Shape| match s {
        egui::Shape::Path(p) if !p.closed => out.push(solid(&p.stroke.color)),
        _ => {}
    };
    for c in shapes {
        if let egui::Shape::Vec(v) = &c.shape {
            v.iter().for_each(&mut walk);
        } else {
            walk(&c.shape);
        }
    }
    out
}

/// Three frames settle the plot bounds (set on every frame).
fn settle(
    ctx: &egui::Context,
    config: &OperationConfig,
    env: &SafeEnvelope,
    candidates: &mut EnvelopeCandidates,
) -> envelope_k2b::Frame {
    let _ = frame(ctx, config, Some(env), candidates, Vec::new());
    let _ = frame(ctx, config, Some(env), candidates, Vec::new());
    frame(ctx, config, Some(env), candidates, Vec::new())
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// The frames of a drag from `from` to `to`, in ten moves, with no release.
fn drag_frames(from: egui::Pos2, to: egui::Pos2) -> Vec<Vec<egui::Event>> {
    let mut out = vec![
        vec![egui::Event::PointerMoved(from)],
        vec![button(from, true)],
    ];
    for k in 1..=10 {
        let at = from + (to - from) * (k as f32 / 10.0);
        out.push(vec![egui::Event::PointerMoved(at)]);
    }
    out
}

/// The Pocket fixture with a stated depth of `ap_mm`.
fn pocket_at_depth(ap_mm: f64) -> (OperationConfig, SafeEnvelope) {
    let mut config = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Pocket);
    if let OperationConfig::Pocket(c) = &mut config {
        c.depth_per_pass = Some(ap_mm);
    }
    envelope_of(config).expect("the Pocket is backed")
}

/// The rivmap100 3D Rough envelope (the operator's review job), at the
/// stated point of the fixture, or at `(ap, ae)` when given (the live GUI
/// point of the review was ap 4.217, ae 2.303).
fn rivmap_rough(point: Option<(f64, f64)>) -> (OperationConfig, SafeEnvelope) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../rs_cam_core/tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml"
    );
    let session = rs_cam_core::session::ProjectSession::load(std::path::Path::new(path))
        .expect("the rivmap100 fixture loads");
    let mut tc = session
        .toolpath_configs()
        .iter()
        .find(|t| t.name == "3D Rough")
        .expect("rivmap100 has the 3D Rough")
        .clone();
    if let (OperationConfig::Adaptive3d(c), Some((ap, ae))) = (&mut tc.operation, point) {
        c.depth_per_pass = Some(ap);
        c.stepover = Some(ae);
    }
    let env = session
        .safe_envelope(&tc)
        .expect("a tool")
        .expect("a backed envelope");
    (tc.operation, env)
}

/// The `mm` of the first `kind` ceiling.
fn ceiling(env: &SafeEnvelope, kind: LimitKind) -> Option<f64> {
    env.boundary
        .iter()
        .filter(|l| l.kind == kind)
        .find_map(|l| match l.edge {
            Edge::ApCeiling { mm } | Edge::AeCeiling { mm } => Some(mm),
            Edge::ApCeilingAbove { .. }
            | Edge::ApCurve { .. }
            | Edge::ApFloor { .. }
            | Edge::AeFloor { .. }
            | Edge::PerPoint
            | Edge::Unmodelled { .. } => None,
        })
}

/// Claim 3: the frame of `env` is the hard stops.
fn assert_hard_stops(tag: &str, env: &SafeEnvelope) {
    let scene = EnvelopeScene::of(env, None);
    let d = ceiling(env, LimitKind::Diameter).expect("fixture: a diameter limit");
    assert_eq!(scene.ae_range, (0.0, d), "{tag}: the width axis is 0..D");
    for kind in [LimitKind::CuttingLength, LimitKind::FluteGuard] {
        if let Some(top) = ceiling(env, kind) {
            assert!(
                scene.ap_range.1 <= top + 1e-9,
                "{tag}: the depth axis {:?} passes the {kind:?} at {top}",
                scene.ap_range
            );
        }
    }
    for e in &scene.edges {
        assert!(
            !matches!(
                e.kind,
                LimitKind::Diameter
                    | LimitKind::CuttingLength
                    | LimitKind::FluteGuard
                    | LimitKind::EngagementFloor
            ),
            "{tag}: the frame limit {:?} is drawn as a line",
            e.kind
        );
    }
}

#[test]
fn a_drag_moves_the_candidate_and_never_the_proposal_p5c() {
    // 1a. The operator's point inside (the fixture Pocket at ap 1.0, under
    // its rigidity cap of 1.2): no proposal mark.
    let (config, env) = pocket_at_depth(1.0);
    assert!(
        matches!(env.operator, Some(Placement::Inside { .. })),
        "fixture: the Pocket's stated point sits inside: {:?}",
        env.operator
    );
    let scene = EnvelopeScene::of(&env, None);
    assert!(!scene.proposal_shown, "the proposal is at Now: no mark");
    let ctx = ctx();
    let mut candidates = EnvelopeCandidates::default();
    let rest = settle(&ctx, &config, &env, &mut candidates);
    let m = marks(&rest.shapes);
    assert!(m.rings.is_empty(), "no ghost at rest: {m:?}");
    assert!(m.diamonds.is_empty(), "no proposal mark inside: {m:?}");
    let stated = env.stated.expect("fixture: a stated point");
    let at_rest_dot = rest
        .response
        .screen_of(stated, EnvelopeShape::Plane)
        .expect("a frame");
    assert_eq!(m.dots.len(), 1, "one white dot: {m:?}");
    assert_eq!(m.dots[0].1, tokens::TEXT_STRONG, "a white dot");
    assert!(
        m.dots[0].0.distance(at_rest_dot) < 2.0,
        "at the stored point"
    );

    let r = rest.response;
    let from = CutPoint {
        ap_mm: r.ap_range.1 * 0.3,
        ae_mm: r.ae_range.1 * 0.3,
    };
    let to = CutPoint {
        ap_mm: r.ap_range.1 * 0.5,
        ae_mm: r.ae_range.1 * 0.6,
    };
    let screen = |p| r.screen_of(p, EnvelopeShape::Plane).expect("a frame");
    let proposal_before = env.proposal.get();
    let mut last = None;
    for events in drag_frames(screen(from), screen(to)) {
        last = Some(frame(&ctx, &config, Some(&env), &mut candidates, events));
    }
    assert!(
        last.expect("frames").events.is_empty(),
        "a drag writes nothing"
    );
    // The plot paints before it reads the pointer: the dot shows the
    // candidate on the next frame.
    let f = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    assert!(f.events.is_empty(), "a drag writes nothing: {:?}", f.events);
    let got = candidates.get(ID).expect("the drag made a candidate");
    let tol = |range: (f64, f64)| (range.1 - range.0) * 0.02;
    assert!(
        (got.ap_mm - to.ap_mm).abs() < tol(r.ap_range)
            && (got.ae_mm - to.ae_mm).abs() < tol(r.ae_range),
        "the candidate at {got:?}, the drag ended at {to:?}"
    );
    assert_eq!(env.proposal.get(), proposal_before, "the proposal value");
    let scene = EnvelopeScene::of(&env, Some(got));
    assert_eq!(scene.proposal, proposal_before, "the scene's proposal");
    let m = marks(&f.shapes);
    assert!(m.no_blue_circle(), "nothing blue moves: {m:?}");
    // P5d D3: the proposal diamond paints when the proposal is not at the
    // setpoint; the draft moved the setpoint off it, so the quiet diamond
    // now marks the proposal, which did not move.
    assert_eq!(m.diamonds.len(), 1, "the quiet proposal mark: {m:?}");
    assert_eq!(m.diamonds[0].1, recommendation_color(), "quiet ACCENT");
    assert!(
        m.diamonds[0].0.distance(screen(proposal_before)) < 2.0,
        "the proposal mark stays at the proposal"
    );
    assert_eq!(m.dots.len(), 1, "one white dot: {m:?}");
    assert_eq!(m.dots[0].1, tokens::TEXT_STRONG, "a white dot inside");
    assert!(
        m.dots[0].0.distance(screen(got)) < 2.0,
        "the white dot moved to the candidate"
    );
    assert_eq!(m.rings.len(), 1, "one ghost: {m:?}");
    assert_eq!(m.rings[0].1, tokens::TEXT_MUTED, "a TEXT_MUTED ghost");
    assert!(
        m.rings[0].0.distance(at_rest_dot) < 2.0,
        "the ghost marks the stored point"
    );
    // Esc puts the dot back: no ghost, the dot at the stored point. The
    // button of the drag is still down; a release ends the drag first.
    let _ = frame(
        &ctx,
        &config,
        Some(&env),
        &mut candidates,
        vec![button(screen(to), false)],
    );
    let _ = frame(
        &ctx,
        &config,
        Some(&env),
        &mut candidates,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let m = marks(&frame(&ctx, &config, Some(&env), &mut candidates, Vec::new()).shapes);
    assert!(candidates.get(ID).is_none(), "Esc drops the candidate");
    assert!(m.rings.is_empty(), "no ghost after Esc: {m:?}");
    assert!(
        m.dots.len() == 1 && m.dots[0].0.distance(at_rest_dot) < 2.0,
        "the dot is back at the stored point: {m:?}"
    );

    // 1b. The operator's point outside: the proposal is a third mark, and
    // the drag does not move it. P11c (D13 item 6): the hobby grade cap is
    // 0.25 x 6 = 1.5 mm, equal to the fixture's stated ap 1.5 (it was 1.2),
    // so the point states ap 2.0 to sit outside.
    let (config, env) = pocket_at_depth(2.0);
    assert!(
        matches!(env.operator, Some(Placement::Outside { .. })),
        "fixture: the Pocket's stated point sits outside: {:?}",
        env.operator
    );
    let scene = EnvelopeScene::of(&env, None);
    assert!(scene.proposal_shown, "the proposal is not at Now: a mark");
    // A new context: the drag of 1a holds the button down.
    let ctx = envelope_k2b::ctx();
    let mut candidates = EnvelopeCandidates::default();
    let rest = settle(&ctx, &config, &env, &mut candidates);
    let at_rest = marks(&rest.shapes);
    assert_eq!(at_rest.diamonds.len(), 1, "one proposal mark: {at_rest:?}");
    // P5d D3: the recommendation is quiet ACCENT.
    assert_eq!(
        at_rest.diamonds[0].1,
        recommendation_color(),
        "quiet proposal"
    );
    assert!(at_rest.rings.is_empty(), "no ghost at rest");
    let r = rest.response;
    let screen = |p| r.screen_of(p, EnvelopeShape::Plane).expect("a frame");
    let a = CutPoint {
        ap_mm: r.ap_range.1 * 0.2,
        ae_mm: r.ae_range.1 * 0.2,
    };
    let b = CutPoint {
        ap_mm: r.ap_range.1 * 0.4,
        ae_mm: r.ae_range.1 * 0.7,
    };
    for events in drag_frames(screen(a), screen(b)) {
        let f = frame(&ctx, &config, Some(&env), &mut candidates, events);
        // The proposal mark stays still on every frame of the drag.
        let during = marks(&f.shapes);
        assert_eq!(during.diamonds.len(), 1, "one proposal mark: {during:?}");
        assert!(
            during.diamonds[0].0.distance(at_rest.diamonds[0].0) < 0.5,
            "the proposal mark moved during the drag"
        );
    }
    let dragged = marks(&frame(&ctx, &config, Some(&env), &mut candidates, Vec::new()).shapes);
    assert_eq!(dragged.diamonds.len(), 1, "one proposal mark: {dragged:?}");
    assert!(
        dragged.diamonds[0].0.distance(at_rest.diamonds[0].0) < 0.5,
        "the proposal mark moved: {:?} -> {:?}",
        at_rest.diamonds[0].0,
        dragged.diamonds[0].0
    );
    assert_eq!(dragged.rings.len(), 1, "one ghost: {dragged:?}");
    assert!(dragged.no_blue_circle(), "nothing blue moves: {dragged:?}");
    assert_eq!(dragged.dots.len(), 1, "one dot: {dragged:?}");
}

/// One frame of the envelope inside a scroll area 200 points high with 800
/// points of rows under it (the inspector), and its scroll offset.
fn scrolled_frame(
    ctx: &egui::Context,
    config: &OperationConfig,
    env: &SafeEnvelope,
    candidates: &mut EnvelopeCandidates,
    events: Vec<egui::Event>,
) -> (EnvelopeResponse, egui::Vec2) {
    let mut response = EnvelopeResponse::default();
    let mut offset = egui::Vec2::ZERO;
    let mut out = ctx.run_ui(input(events), |ui| {
        let scroll = egui::ScrollArea::vertical()
            .id_salt("p5c_inspector")
            .max_height(200.0)
            .show(ui, |ui| {
                let mut sent = Vec::new();
                response = draw_envelope(ui, ID, config, Some(env), None, candidates, &mut sent);
                ui.add_space(800.0);
            });
        offset = scroll.state.offset;
    });
    out.textures_delta.clear();
    (response, offset)
}

#[test]
fn a_drag_off_the_plot_stays_on_the_frame_p5c() {
    let (config, env) = envelope_for(OperationType::Pocket).expect("the Pocket is backed");
    let ctx = ctx();
    let mut candidates = EnvelopeCandidates::default();
    let mut rest = None;
    for _ in 0..3 {
        rest = Some(scrolled_frame(
            &ctx,
            &config,
            &env,
            &mut candidates,
            Vec::new(),
        ));
    }
    let (r, offset0) = rest.expect("frames");
    let frame_rect = r.frame.expect("the plot");
    let from = frame_rect.center();
    // Far past the top right of the plot, and off the screen.
    let to = egui::pos2(frame_rect.right() + 400.0, frame_rect.top() - 400.0);
    for events in drag_frames(from, to) {
        let (during, offset) = scrolled_frame(&ctx, &config, &env, &mut candidates, events);
        assert_eq!(
            (during.ae_range, during.ap_range),
            (r.ae_range, r.ap_range),
            "the view range moved during the drag"
        );
        assert_eq!(offset, offset0, "the inspector scrolled during the drag");
    }
    let got = candidates.get(ID).expect("the drag made a candidate");
    assert!(
        (got.ae_mm - r.ae_range.1).abs() < 1e-9 && (got.ap_mm - r.ap_range.1).abs() < 1e-9,
        "the candidate {got:?} is held on the top right corner {:?} x {:?}",
        r.ae_range,
        r.ap_range
    );
    // Past the bottom left: the candidate is held on the origin corner.
    let (_, offset) = scrolled_frame(
        &ctx,
        &config,
        &env,
        &mut candidates,
        vec![egui::Event::PointerMoved(egui::pos2(-300.0, 900.0))],
    );
    let got = candidates.get(ID).expect("a candidate");
    assert!(
        (got.ae_mm - r.ae_range.0).abs() < 1e-9 && (got.ap_mm - r.ap_range.0).abs() < 1e-9,
        "the candidate {got:?} is held on the origin corner"
    );
    assert_eq!(offset, offset0, "the inspector scrolled");
    let _ = scrolled_frame(
        &ctx,
        &config,
        &env,
        &mut candidates,
        vec![button(egui::pos2(-300.0, 900.0), false)],
    );
    assert!(candidates.get(ID).is_some(), "the release keeps it");
}

#[test]
fn the_frame_is_the_hard_stops_p5c() {
    let (_, pocket) = envelope_for(OperationType::Pocket).expect("the Pocket is backed");
    assert_hard_stops("Pocket", &pocket);
    let (_, rough) = rivmap_rough(None);
    assert_hard_stops("rivmap100 3D Rough", &rough);
    // A stated depth past the cutting length: the axis still stops there,
    // and the dot is drawn on the frame edge.
    let (_, deep) = pocket_at_depth(1000.0);
    assert_hard_stops("Pocket at 1000 mm", &deep);
    let scene = EnvelopeScene::of(&deep, None);
    let (dot, _) = scene.dot.expect("a stated point");
    assert_eq!(scene.on_frame(dot).ap_mm, scene.ap_range.1, "on the edge");
}

/// P5d D1 reversed the P5c "near a mark" rule of this arm: every limit
/// whose edge enters the view draws, with ONE label; a line that runs along
/// the frame draws none. The P5c arm drew at most 2 lines here.
#[test]
fn the_rivmap_rough_plot_draws_every_limit_in_view_p5c() {
    let ctx = ctx();
    // The fixture's point, and the live point of the operator's review.
    //
    // Feeds contract P11c (review item M4): the move-count model reads the
    // machine's `adaptive_woc_factor` (0.2 in this file) for the adaptive
    // area width, not the typed 0.3, so the predicted moves rise 1.5 x. At
    // the fixture's point the `Runtime` (move budget) width floor now enters
    // the view and draws. At the review point (ap 4.217) the cut has fewer
    // depth levels, so fewer moves, and the floor stays out of view.
    for (tag, point, runtime_in_view) in [
        ("fixture", None, true),
        ("review ap 4.217 x ae 2.303", Some((4.217, 2.303)), false),
    ] {
        let (config, env) = rivmap_rough(point);
        let scene = EnvelopeScene::of(&env, None);
        let mut candidates = EnvelopeCandidates::default();
        let f = settle(&ctx, &config, &env, &mut candidates);
        let plot = f.response.frame.expect("a plot");
        let drawn = scene.drawn_edges(plot.size());
        let mut kinds: Vec<LimitKind> = drawn.iter().map(|e| e.kind).collect();
        kinds.sort_by_key(|k| format!("{k:?}"));
        assert!(
            !kinds.contains(&LimitKind::Diameter),
            "{tag}: D is the frame"
        );
        let want = if runtime_in_view {
            vec![
                LimitKind::Rigidity,
                LimitKind::Runtime,
                LimitKind::Slot,
                LimitKind::VendorAxial,
            ]
        } else {
            vec![LimitKind::Rigidity, LimitKind::Slot, LimitKind::VendorAxial]
        };
        assert_eq!(
            kinds, want,
            "{tag}: P5d D1, the vendor, rigidity and slot limits draw (and the move budget \
             where it enters the view)"
        );
        assert_eq!(
            scene.binding,
            Some(LimitKind::VendorAxial),
            "{tag}: the vendor DOC binds this cut"
        );
        // One label per drawn line: its name, once (round 2; P5d D4).
        let names: Vec<String> = scene.edges.iter().map(|e| e.label.clone()).collect();
        let painted = texts(&f.shapes);
        let mut labels: Vec<&str> = painted
            .iter()
            .filter(|t| names.contains(t))
            .map(String::as_str)
            .collect();
        labels.sort_unstable();
        let mut want: Vec<&str> = drawn.iter().map(|e| e.label.as_str()).collect();
        want.sort_unstable();
        assert_eq!(labels, want, "{tag}: one label per drawn line");
        // The colour rule (P5d D4): the binding line TEXT_BODY, the others
        // TEXT_MUTED, no DANGER.
        for e in &drawn {
            let want = if Some(e.kind) == scene.binding {
                tokens::TEXT_BODY
            } else {
                tokens::TEXT_MUTED
            };
            assert_eq!(scene.edge_color(e), want, "{tag}: {e:?}");
        }
        let colours = line_colours(&f.shapes);
        assert!(
            colours.contains(&tokens::TEXT_BODY),
            "{tag}: non-vacuity: a neutral line paints: {colours:?}"
        );
        assert!(
            !colours.contains(&tokens::DANGER),
            "{tag}: no DANGER line: {colours:?}"
        );
    }
}

#[test]
fn a_crossed_limit_and_the_candidate_turn_amber_p5c() {
    let (_, env) = rivmap_rough(Some((4.217, 2.303)));
    let vendor = ceiling(&env, LimitKind::VendorAxial).expect("a vendor max DOC");
    let scene = EnvelopeScene::of(&env, None);
    let over = CutPoint {
        ap_mm: (vendor + scene.ap_range.1) / 2.0,
        ae_mm: 1.0,
    };
    assert!(over.ap_mm > vendor && over.ap_mm < scene.ap_range.1);
    let scene = EnvelopeScene::of(&env, Some(over));
    assert!(
        scene.candidate_outside(),
        "fixture: the candidate is outside"
    );
    assert_eq!(scene.dot_color(), Some(tokens::CAUTION), "the dot is amber");
    let crossed: Vec<LimitKind> = scene
        .edges
        .iter()
        .filter(|e| scene.edge_color(e) == tokens::CAUTION)
        .map(|e| e.kind)
        .collect();
    assert_eq!(crossed, vec![LimitKind::VendorAxial], "the crossed line");
    for e in &scene.edges {
        assert_ne!(scene.edge_color(e), tokens::DANGER);
    }
}
