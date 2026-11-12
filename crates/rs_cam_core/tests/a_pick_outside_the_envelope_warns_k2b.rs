//! CONTRACT K2b and K6 (feeds contract P5, ruling Q6): a pick outside the
//! safe envelope is written, and it warns.
//!
//! The operator picks a depth past the flute guard of a 3D Rough. The
//! apply:
//!
//! 1. writes the picked point (no refusal: Q6);
//! 2. writes the speeds that the envelope computed at that point (one
//!    closed-form `calculate` call at the pick, rounded down to 1 mm/min as
//!    every feed);
//! 3. files exactly one `OutsideEnvelope` warning, which names the flute
//!    guard, and its diagnostic is the K6 row `project.outside_envelope`
//!    (a warning).
//!
//! Red before P5: the pick API does not exist (P5 report).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::adapters::from_feeds::diagnostic_from_suggest_warning;
use rs_cam_core::diagnostics::ids;
use rs_cam_core::feeds::envelope::{
    EnvelopeInput, LimitKind, PickRequest, Placement, SafeEnvelope,
};
use rs_cam_core::feeds::suggest::{
    NoProject, OperationApplyInput, SuggestContext, SuggestWarning, apply_to_operation,
    round_suggestion_value_down,
};
use rs_cam_core::feeds::{FeedsProvenance, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The picked depth: past the 0.8 x 6 = 4.8 mm flute guard of the tool, by
/// the largest share of every edge it crosses.
const PICK_AP_MM: f64 = 7.0;
/// The picked width.
const PICK_AE_MM: f64 = 0.5;

fn tool() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    t.diameter = 6.0;
    t.set_cutting_length(ToolFact::Stated(6.0));
    t
}

fn rough() -> OperationConfig {
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Adaptive3d);
    assert!(op.set_depth_per_pass(2.0));
    assert!(op.set_stepover(1.0));
    op
}

#[test]
fn a_pick_outside_the_envelope_is_written_and_warns_k2b() {
    let tool = tool();
    let machine = MachineProfile::default();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let base = rough();
    let request = PickRequest::Point {
        ap_mm: PICK_AP_MM,
        ae_mm: PICK_AE_MM,
    };

    // The envelope's own answer at the pick.
    let env = SafeEnvelope::for_cut(EnvelopeInput {
        operation: &base,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::no_project(NoProject::NONE),
    })
    .expect("a 6 mm end mill on a 3D Rough is backed");
    let pick = env.pick(request);
    assert!(
        matches!(
            pick.placement(),
            Placement::Outside {
                binding: LimitKind::FluteGuard,
                ..
            }
        ),
        "{:?}",
        pick.placement()
    );

    let mut op = base;
    let mut prov = FeedsProvenance::default();
    let warnings = apply_to_operation(
        OperationApplyInput {
            tool: &tool,
            machine: &machine,
            material: &material,
            spindle_strategy: SpindleStrategy::default(),
            lut: embedded_vendor_lut(),
            suggest: SuggestContext::no_project(NoProject::NONE),
        },
        Some(request),
        None,
        &mut op,
        &mut prov,
    )
    .expect("a pick is never refused (Q6)");

    // 1. The point is written. A 3D Rough has no total depth, so no snap.
    assert_eq!(op.depth_per_pass(), Some(PICK_AP_MM));
    assert_eq!(op.stepover(), Some(PICK_AE_MM));

    // 2. The speeds come from the envelope at the point. The calculator
    // holds its own depth at the flute guard, so pass 9 carries the
    // envelope's feed to the picked depth (G-SPEEDSAPPLY): the funnel
    // starts from the envelope's feed, rounded down to 1 mm/min.
    let at_pick = pick.result();
    let start = round_suggestion_value_down(at_pick.feed_rate_mm_min, 1.0);
    let rescaled_from = warnings.iter().find_map(|w| {
        if let SuggestWarning::FeedRescaledToFinalGeometry {
            requested_mm_per_min,
            ..
        } = w
        {
            Some(*requested_mm_per_min)
        } else {
            None
        }
    });
    match rescaled_from {
        Some(from) => assert_eq!(from, start, "pass 9 did not start at the envelope's feed"),
        None => assert_eq!(
            op.feed_rate(),
            Some(start),
            "the feed is not the envelope's"
        ),
    }
    assert_eq!(
        op.spindle_rpm().map(f64::from),
        Some(at_pick.rpm.round()),
        "the RPM is not the envelope's RPM at the pick"
    );

    // 3. One warning that names the flute guard; its diagnostic is the K6 row.
    let outside: Vec<&SuggestWarning> = warnings
        .iter()
        .filter(|w| matches!(w, SuggestWarning::OutsideEnvelope { .. }))
        .collect();
    assert_eq!(outside.len(), 1, "{warnings:?}");
    let w = outside.first().copied().expect("one warning");
    assert!(matches!(
        w,
        SuggestWarning::OutsideEnvelope {
            binding: LimitKind::FluteGuard,
            ..
        }
    ));
    let diag = diagnostic_from_suggest_warning(ToolpathId::from_raw(1), w)
        .expect("the warning has a diagnostic");
    assert_eq!(diag.id.as_str(), ids::PROJECT_OUTSIDE_ENVELOPE);
    assert!(diag.message.contains("flute guard"), "{}", diag.message);
}
