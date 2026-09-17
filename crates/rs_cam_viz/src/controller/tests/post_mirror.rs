//! SHL-01 — one door rebuilds the whole post mirror.

use super::*;

/// The controller door that every GUI post route takes rebuilds the whole
/// mirror. W9 / P-1 is the `format` half of this: a reload reset a
/// grblHAL project's dropdown to GRBL because one copy site was wrong.
#[test]
fn the_post_door_rebuilds_the_whole_mirror_shl01() {
    let mut controller = sample_controller();

    let mut post = controller.state.session.post_config().clone();
    post.format = "grblhal".to_owned();
    post.safe_z = 42.0;
    post.spindle_speed = 21_000;
    let effects = controller
        .state
        .session
        .apply(Command::SetPostConfig(SetPostConfigArgs {
            post: Box::new(post),
        }))
        .expect("the session accepts the post block");

    drift_the_post_mirror(&mut controller);
    controller.adopt_post_effects(&effects);

    assert_post_mirror_matches_session(&controller, "adopt_post_effects");
    assert_eq!(
        controller.state.gui.post.format,
        rs_cam_core::gcode::PostFormat::GrblHal,
        "SHL-01 / W9-P-1: the mirror kept the drifted post format"
    );
    assert!(
        (controller.state.gui.post.safe_z - 42.0).abs() < 1e-9,
        "SHL-01: the mirror kept the drifted safe-Z"
    );
}

/// The Feeds & Speeds route writes the session and then reads the mirror
/// back through the same door. Before SHL-01 this arm copied
/// `spindle_strategy` alone, and it copied it even when the command was
/// refused.
#[test]
fn the_spindle_strategy_event_rebuilds_the_whole_mirror_shl01() {
    let mut controller = sample_controller();
    let strategy = rs_cam_core::feeds::SpindleStrategy::MaxSpeed;
    assert_ne!(
        controller.state.session.post_config().spindle_strategy,
        strategy,
        "the fixture must start on the other strategy, or the arm short-circuits"
    );

    drift_the_post_mirror(&mut controller);
    // The drift set the strategy too; put it back so the arm's own
    // guard sees a real change to make.
    controller.state.gui.post.spindle_strategy = rs_cam_core::feeds::SpindleStrategy::MatchChart;
    controller.handle_internal_event(AppEvent::SetSpindleStrategy(strategy));

    assert_eq!(
        controller.state.gui.post.spindle_strategy, strategy,
        "SHL-01: the strategy the operator picked did not reach the mirror"
    );
    assert_post_mirror_matches_session(&controller, "the SetSpindleStrategy event");
}
