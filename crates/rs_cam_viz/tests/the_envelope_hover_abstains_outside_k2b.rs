//! K2b (feeds contract P5b): the hover readout of the envelope plot
//! abstains outside the envelope.
//!
//! # The claim
//!
//! The readout is one line. At a point inside the envelope it gives ap, ae
//! and the speeds from the `SpeedsMap` (a lookup, no Suggest run): rpm,
//! feed and power. At a point outside it names the crossed limit and gives
//! no speeds, as `the_nomogram_readout_abstains_g_hoverbound` holds for the
//! nomogram. A rendered arm parks the pointer outside on the plot and reads
//! the painted tooltip.
//!
//! Red before P5b: `ui::feeds::envelope::readout` does not exist.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod envelope_k2b;

use envelope_k2b::{ctx, envelope_for, frame, texts};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::feeds::envelope::{CutPoint, Placement};
use rs_cam_viz::state::panels::EnvelopeCandidates;
use rs_cam_viz::ui::feeds::envelope::{EnvelopeScene, EnvelopeShape, readout, short_label};

/// A point past the top of the depth span: outside the flute guard.
/// A point inside the plot and outside the envelope: the top of the view
/// over a slot (97 % of the width range, past the slot threshold). P13c
/// (O1): it was the top of the view at the proposal width, but the
/// force-budget rigidity curve now passes the top of the view there; the
/// slot step stays inside the view.
fn outside(env: &rs_cam_core::feeds::envelope::SafeEnvelope) -> CutPoint {
    let scene = EnvelopeScene::of(env, None);
    CutPoint {
        ap_mm: scene.ap_range.1 * 0.98,
        ae_mm: scene.ae_range.1 * 0.97,
    }
}

#[test]
fn the_envelope_hover_abstains_outside_k2b() {
    let (config, env) = envelope_for(OperationType::Pocket).expect("the Pocket is backed");
    let inside = env.proposal.get();
    assert!(matches!(env.placement_of(inside), Placement::Inside { .. }));
    let text = readout(&env, inside);
    assert!(
        text.contains("rpm") && text.contains("mm/min") && text.contains("kW"),
        "inside, the readout gives the speeds: {text}"
    );
    let speeds = env
        .speeds
        .speeds_at(inside.ap_mm, inside.ae_mm)
        .expect("the grid answers");
    assert!(
        text.contains(&format!("{:.0} rpm", speeds.rpm)),
        "the readout is the SpeedsMap lookup: {text}"
    );

    let out = outside(&env);
    let Placement::Outside { binding, .. } = env.placement_of(out) else {
        panic!("fixture: {out:?} must sit outside the envelope");
    };
    let text = readout(&env, out);
    assert!(
        text.contains(short_label(binding)),
        "outside, the readout names `{}`: {text}",
        short_label(binding)
    );
    for word in ["rpm", "mm/min", "kW"] {
        assert!(!text.contains(word), "outside, no speeds: {text}");
    }

    // Rendered: the pointer parked outside on the plot paints the
    // abstaining line.
    let ctx = ctx();
    let mut candidates = EnvelopeCandidates::default();
    let _ = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    let _ = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    let first = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    let pos = first
        .response
        .screen_of(out, EnvelopeShape::Plane)
        .expect("the plot has a frame");
    let mut painted = Vec::new();
    for _ in 0..4 {
        let f = frame(
            &ctx,
            &config,
            Some(&env),
            &mut candidates,
            vec![egui::Event::PointerMoved(pos)],
        );
        painted = texts(&f.shapes);
    }
    let line = painted
        .iter()
        .find(|t| t.starts_with("ap "))
        .unwrap_or_else(|| panic!("no readout painted: {painted:?}"));
    assert!(line.contains(short_label(binding)), "{line}");
    assert!(
        !line.contains("rpm"),
        "the painted readout abstains: {line}"
    );
    assert!(
        candidates.get(envelope_k2b::ID).is_none(),
        "a hover moves no ring"
    );
}
