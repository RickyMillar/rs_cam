//! F072: the post config has one stored copy, in the session.

use super::*;

/// The Feeds & Speeds route writes the WHOLE post block through the
/// session door. Before SHL-01 this arm copied `spindle_strategy` alone
/// into a GUI copy, and it copied it even when the command was refused.
/// F072 deleted the GUI copy; the session is the only reader now.
#[test]
fn the_spindle_strategy_event_writes_the_session_f072() {
    let mut controller = sample_controller();

    let mut post = controller.state.session().post_config().clone();
    post.format = rs_cam_core::gcode::PostFormat::GrblHal;
    post.safe_z = 42.0;
    let _ = controller
        .apply(Command::SetPostConfig(SetPostConfigArgs {
            post: Box::new(post),
        }))
        .expect("the session accepts the post block");

    // Feeds contract P21b: the event carries the token and the ceiling;
    // the ceiling is the block's own S word, so no number is typed here.
    let strategy = rs_cam_core::feeds::SpindleStrategyKind::ConstantChip;
    let rpm_ceiling = Some(controller.state.session().post_config().spindle_speed);
    assert_ne!(
        controller.state.session().post_config().spindle_strategy,
        strategy,
        "the fixture must start on the other strategy, or the arm short-circuits"
    );
    controller.handle_internal_event(AppEvent::SetSpindleStrategy {
        strategy,
        rpm_ceiling,
    });

    let session_post = controller.state.session().post_config();
    assert_eq!(
        session_post.spindle_strategy, strategy,
        "the strategy the operator picked did not reach the session"
    );
    assert_eq!(
        session_post.spindle_rpm_ceiling, rpm_ceiling,
        "the ceiling the operator set did not reach the session"
    );
    assert_eq!(
        session_post.format,
        rs_cam_core::gcode::PostFormat::GrblHal,
        "the event must keep every other field of the block (W9 / P-1)"
    );
    assert!(
        (session_post.safe_z - 42.0).abs() < 1e-9,
        "the event must keep the safe-Z of the block"
    );
}
