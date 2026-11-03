//! K2b (feeds contract P5b): the envelope plot of the Passes tab marks the
//! operator's point and the proposal, and names each limit it draws.
//!
//! # The claim
//!
//! On the fixture Pocket (a plane envelope, both axes stated), one frame of
//! `ui::feeds::envelope::draw_envelope` paints:
//!
//! - one filled dot at the operator's stated point ("Now"), neutral ink
//!   while the point sits inside (ruling R23), `tokens::CAUTION` outside;
//! - P5c (operator review 2026-11-04): no candidate ring at rest (the ring
//!   exists only after a click or a drag), and a proposal mark (an ACCENT
//!   diamond) only when the proposal is not at Now. P5b drew the proposal
//!   as the ring, so a drag seemed to move the recommendation;
//! - P5c: no limit name at rest (the hover names a limit), and every drawn
//!   edge neutral (`tokens::TEXT_MUTED`); the edge that binds Now is among
//!   the drawn edges. P5b named each edge on the line and drew the binding
//!   edge solid in DANGER (decision D2 kept: lines, no fill).
//!
//! The scene is a pure function of the envelope (`EnvelopeScene::of`), so
//! the arms read it beside the painted shapes.
//!
//! Red before P5b: `ui::feeds::envelope` does not exist.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod envelope_k2b;

use envelope_k2b::{ctx, envelope_for, frame, solid, texts};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_viz::state::panels::EnvelopeCandidates;
use rs_cam_viz::ui::feeds::envelope::{EnvelopeScene, EnvelopeShape, short_label};
use rs_cam_viz::ui::tokens;

/// The circles of a frame: (centre, radius, fill, stroke colour).
fn circles(
    shapes: &[egui::epaint::ClippedShape],
) -> Vec<(egui::Pos2, f32, egui::Color32, egui::Color32)> {
    let mut out = Vec::new();
    let walk = |s: &egui::Shape, out: &mut Vec<_>| {
        if let egui::Shape::Circle(c) = s {
            out.push((c.center, c.radius, c.fill, c.stroke.color));
        }
    };
    for c in shapes {
        if let egui::Shape::Vec(v) = &c.shape {
            v.iter().for_each(|s| walk(s, &mut out));
        } else {
            walk(&c.shape, &mut out);
        }
    }
    out
}

#[test]
fn the_envelope_plot_marks_the_operator_and_the_proposal_k2b() {
    // The fixture Pocket states ap 1.5 over a rigidity cap of 1.2: outside.
    // At ap 1.0 the stated point sits inside.
    let (fixture, _) = envelope_for(OperationType::Pocket).expect("the Pocket is backed");
    let mut inside = fixture;
    if let rs_cam_core::compute::catalog::OperationConfig::Pocket(c) = &mut inside {
        c.depth_per_pass = 1.0;
    }
    let (config, env) = envelope_k2b::envelope_of(inside).expect("the Pocket is backed");
    let scene = EnvelopeScene::of(&env, None);
    assert_eq!(scene.shape, EnvelopeShape::Plane, "fixture: a plane op");
    assert_eq!(scene.proposal, env.proposal.get(), "the scene's proposal");
    assert!(scene.candidate.is_none(), "no candidate at rest");
    let stated = env.stated.expect("fixture: the Pocket states both axes");
    assert_eq!(scene.dot.map(|(p, _)| p), Some(stated));
    assert!(!scene.edges.is_empty(), "non-vacuity: the plot draws edges");
    if let Some(kind) = scene.binding {
        assert!(
            scene.edges.iter().any(|e| e.kind == kind),
            "the binding limit {kind:?} is drawn: {:?}",
            scene.edges
        );
    }
    for e in &scene.edges {
        assert_eq!(scene.edge_color(e), tokens::TEXT_MUTED, "{:?}", e.kind);
    }

    let ctx = ctx();
    let mut candidates = EnvelopeCandidates::default();
    let f = settle(&ctx, &config, &env, &mut candidates);
    let drawn = circles(&f.shapes);
    let rings: Vec<_> = drawn
        .iter()
        .filter(|c| c.2 == egui::Color32::TRANSPARENT)
        .collect();
    assert!(rings.is_empty(), "no ring at rest: {drawn:?}");
    let dot_color = scene.dot_color().expect("a stated point has a dot");
    let dots: Vec<_> = drawn.iter().filter(|c| c.2 == dot_color).collect();
    assert_eq!(dots.len(), 1, "one dot in {dot_color:?}: {drawn:?}");
    assert!(
        matches!(
            env.operator,
            Some(rs_cam_core::feeds::envelope::Placement::Inside { .. })
        ),
        "fixture: the Pocket's stated point sits inside"
    );
    assert_eq!(dot_color, tokens::TEXT_STRONG, "R23: a neutral dot inside");
    assert!(!scene.proposal_shown, "the proposal is at Now: no mark");

    // No limit name at rest.
    let names: Vec<&str> = env.boundary.iter().map(|l| short_label(l.kind)).collect();
    let painted = texts(&f.shapes);
    let labels: Vec<&String> = painted
        .iter()
        .filter(|t| names.contains(&t.as_str()))
        .collect();
    assert!(labels.is_empty(), "no limit name at rest: {labels:?}");

    // The operator's point outside: a CAUTION dot, and the proposal is a
    // third mark at its own place
    // (the fixture Pocket itself). The edge that Now crosses is CAUTION.
    let (deep, env) = envelope_for(OperationType::Pocket).expect("the Pocket is backed");
    let scene = EnvelopeScene::of(&env, None);
    let Some(rs_cam_core::feeds::envelope::Placement::Outside { binding, .. }) = env.operator
    else {
        panic!(
            "fixture: the Pocket's stated point sits outside: {:?}",
            env.operator
        );
    };
    for e in &scene.edges {
        let want = if e.kind == binding {
            tokens::CAUTION
        } else {
            tokens::TEXT_MUTED
        };
        assert_eq!(scene.edge_color(e), want, "{:?}", e.kind);
    }
    assert!(
        scene.edges.iter().any(|e| e.kind == binding),
        "the crossed limit {binding:?} is drawn"
    );
    assert_eq!(
        scene.dot_color(),
        Some(tokens::CAUTION),
        "Q6: CAUTION outside"
    );
    assert!(scene.proposal_shown, "the proposal is not at Now");
    let mut candidates = EnvelopeCandidates::default();
    let f = settle(&ctx, &deep, &env, &mut candidates);
    let at = f
        .response
        .screen_of(scene.proposal, EnvelopeShape::Plane)
        .expect("the plot has a frame");
    let diamond = diamonds(&f.shapes);
    assert_eq!(diamond.len(), 1, "one proposal mark: {diamond:?}");
    assert_eq!(diamond[0].1, tokens::ACCENT, "an ACCENT proposal");
    assert!(
        diamond[0].0.distance(at) < 2.0,
        "the proposal mark at {:?}, the proposal at {at:?}",
        diamond[0].0
    );
}

/// Three frames settle the plot bounds.
fn settle(
    ctx: &egui::Context,
    config: &rs_cam_core::compute::catalog::OperationConfig,
    env: &rs_cam_core::feeds::envelope::SafeEnvelope,
    candidates: &mut EnvelopeCandidates,
) -> envelope_k2b::Frame {
    let _ = frame(ctx, config, Some(env), candidates, Vec::new());
    let _ = frame(ctx, config, Some(env), candidates, Vec::new());
    frame(ctx, config, Some(env), candidates, Vec::new())
}

/// The closed four-point paths of a frame (the proposal diamond): centre
/// and stroke colour.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn diamonds(shapes: &[egui::epaint::ClippedShape]) -> Vec<(egui::Pos2, egui::Color32)> {
    let mut out = Vec::new();
    let mut walk = |s: &egui::Shape| match s {
        egui::Shape::Path(p) if p.closed && p.points.len() == 4 => {
            let c = p
                .points
                .iter()
                .fold(egui::Vec2::ZERO, |acc, q| acc + q.to_vec2())
                / 4.0;
            out.push((c.to_pos2(), solid(&p.stroke.color)));
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
