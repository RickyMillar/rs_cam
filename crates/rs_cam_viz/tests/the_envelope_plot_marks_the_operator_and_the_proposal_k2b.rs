//! K2b (feeds contract P5b): the envelope plot of the Passes tab marks the
//! operator's point and the proposal, and names each limit it draws.
//!
//! # The claim
//!
//! On the fixture Pocket (a plane envelope, both axes stated), one frame of
//! `ui::feeds::envelope::draw_envelope` paints:
//!
//! - one ring (an unfilled circle) in `tokens::ACCENT` at the proposal;
//! - one filled dot at the operator's stated point, neutral ink while the
//!   point sits inside (ruling R23), `tokens::CAUTION` outside;
//! - one short limit name per labelled edge, each the name of a drawn
//!   edge, none twice, the binding edge's name among them;
//! - the binding edge solid and every other edge faint (decision D2).
//!
//! The scene is a pure function of the envelope (`EnvelopeScene::of`), so
//! the arms read it beside the painted shapes.
//!
//! Red before P5b: `ui::feeds::envelope` does not exist.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod envelope_k2b;

use envelope_k2b::{ctx, envelope_for, frame, texts};
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
    let (config, env) = envelope_for(OperationType::Pocket).expect("the Pocket is backed");
    let scene = EnvelopeScene::of(&env, None);
    assert_eq!(scene.shape, EnvelopeShape::Plane, "fixture: a plane op");
    assert_eq!(scene.ring, env.proposal.get(), "the ring is the proposal");
    let stated = env.stated.expect("fixture: the Pocket states both axes");
    assert_eq!(scene.dot.map(|(p, _)| p), Some(stated));
    assert!(!scene.edges.is_empty(), "non-vacuity: the plot draws edges");

    // Decision D2: the binding edge solid, every other edge faint.
    let binding = env.proposal_binding;
    for e in &scene.edges {
        assert_eq!(e.binding, Some(e.kind) == binding, "{:?}", e.kind);
    }

    let ctx = ctx();
    let mut candidates = EnvelopeCandidates::default();
    let mut last = None;
    for _ in 0..3 {
        last = Some(frame(
            &ctx,
            &config,
            Some(&env),
            &mut candidates,
            Vec::new(),
        ));
    }
    let f = last.expect("three frames");
    let drawn = circles(&f.shapes);
    let rings: Vec<_> = drawn
        .iter()
        .filter(|c| c.2 == egui::Color32::TRANSPARENT && c.3 == tokens::ACCENT)
        .collect();
    assert_eq!(rings.len(), 1, "one ACCENT ring: {drawn:?}");
    let dot_color = scene.dot_color().expect("a stated point has a dot");
    let dots: Vec<_> = drawn.iter().filter(|c| c.2 == dot_color).collect();
    assert_eq!(dots.len(), 1, "one dot in {dot_color:?}: {drawn:?}");
    if matches!(
        env.operator,
        Some(rs_cam_core::feeds::envelope::Placement::Inside { .. })
    ) {
        assert_eq!(dot_color, tokens::TEXT_STRONG, "R23: a neutral dot inside");
    } else {
        assert_eq!(dot_color, tokens::CAUTION, "Q6: a CAUTION dot outside");
    }
    // The ring is at the proposal on the screen.
    let at = f
        .response
        .screen_of(scene.ring, EnvelopeShape::Plane)
        .expect("the plot has a frame");
    let ring = rings.first().expect("one ring");
    assert!(
        ring.0.distance(at) < 2.0,
        "the ring at {:?}, the proposal at {at:?}",
        ring.0
    );

    // One label per labelled edge, each the name of a drawn edge.
    let names: Vec<&str> = scene.edges.iter().map(|e| short_label(e.kind)).collect();
    let painted = texts(&f.shapes);
    let labels: Vec<&String> = painted
        .iter()
        .filter(|t| names.contains(&t.as_str()))
        .collect();
    assert!(!labels.is_empty(), "non-vacuity: the plot names its edges");
    let mut seen = labels.clone();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), labels.len(), "a label twice: {labels:?}");
    if let Some(kind) = binding
        && scene.edges.iter().any(|e| e.kind == kind)
    {
        assert!(
            labels.iter().any(|t| t.as_str() == short_label(kind)),
            "the binding edge `{}` has no label: {labels:?}",
            short_label(kind)
        );
    }
}
