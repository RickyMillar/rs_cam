//! K2b (feeds contract P5b): an op with one free value draws a bar, an op
//! with none draws nothing.
//!
//! # The claim
//!
//! For every op of `OperationType::ALL` whose fixture envelope is backed,
//! the section that `draw_envelope` draws follows the envelope axes:
//!
//! - `Plane`: the header and the plot;
//! - `AxialBar` / `RadialBar`: the header and a bar of one row, which ends
//!   at the binding limit and names it;
//! - `Fixed`: nothing, no header and no text.
//!
//! Non-vacuity: Trace and Drill draw the depth bar (the Drill's free value
//! is its peck), the Pocket draws the plot, and at least one op draws
//! nothing.
//!
//! Red before P5b: `ui::feeds::envelope` does not exist.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod envelope_k2b;

use envelope_k2b::{ctx, envelope_for, frame, texts};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_viz::state::panels::EnvelopeCandidates;
use rs_cam_viz::ui::feeds::envelope::{
    EnvelopeShape, TITLE_DEPTH, TITLE_PLANE, TITLE_WIDTH, limit_name,
};

#[test]
fn a_one_axis_op_draws_a_bar_k2b() {
    let ctx = ctx();
    let mut shapes_seen: Vec<(OperationType, EnvelopeShape)> = Vec::new();
    for &op in OperationType::ALL {
        let Some((config, env)) = envelope_for(op) else {
            continue;
        };
        let shape = EnvelopeShape::of(&env.axes);
        shapes_seen.push((op, shape));
        let mut candidates = EnvelopeCandidates::default();
        let f = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
        let painted = texts(&f.shapes);
        let title = |t: &str| painted.iter().any(|p| p == &t.to_uppercase());
        match shape {
            EnvelopeShape::Plane => {
                assert!(title(TITLE_PLANE), "{op:?}: the plot header: {painted:?}");
                assert!(f.response.frame.is_some(), "{op:?}: the plot");
            }
            EnvelopeShape::DepthBar | EnvelopeShape::WidthBar => {
                let want = if shape == EnvelopeShape::DepthBar {
                    TITLE_DEPTH
                } else {
                    TITLE_WIDTH
                };
                assert!(title(want), "{op:?}: the bar header: {painted:?}");
                let bar = f.response.frame.expect("a bar rect");
                assert!(bar.height() <= 20.0, "{op:?}: a bar of one row: {bar:?}");
                // The bar ends at a named limit when one caps the free axis.
                let ceiling = if shape == EnvelopeShape::DepthBar {
                    env.ap_ceiling_at(env.proposal.get().ae_mm).map(|c| c.1)
                } else {
                    None
                };
                // P5d D4: the bar names the limit as the plot does.
                if let Some(kind) = ceiling {
                    let name = limit_name(&env, kind);
                    assert!(
                        painted.contains(&name),
                        "{op:?}: the bar names `{name}` at its end: {painted:?}"
                    );
                }
            }
            EnvelopeShape::None => {
                assert!(painted.is_empty(), "{op:?}: no section: {painted:?}");
                assert!(f.response.frame.is_none(), "{op:?}: no plot");
            }
        }
    }
    let shape_of = |op: OperationType| {
        shapes_seen
            .iter()
            .find(|(o, _)| *o == op)
            .map(|(_, s)| *s)
            .unwrap_or_else(|| panic!("{op:?} is not backed: {shapes_seen:?}"))
    };
    assert_eq!(shape_of(OperationType::Trace), EnvelopeShape::DepthBar);
    assert_eq!(shape_of(OperationType::Drill), EnvelopeShape::DepthBar);
    assert_eq!(shape_of(OperationType::Pocket), EnvelopeShape::Plane);
    assert!(
        shapes_seen.iter().any(|(_, s)| *s == EnvelopeShape::None),
        "non-vacuity: no op draws nothing: {shapes_seen:?}"
    );
}
