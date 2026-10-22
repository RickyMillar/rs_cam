//! The CLI's door into the core command registry.
//!
//! The subcommands write project state. Before WP1, `run`, `job` and
//! `smoke` each called `ProjectSession::set_toolpath_param` directly, so
//! the sites could drift apart. A CLI site now builds a
//! [`rs_cam_core::session::Command`] and takes this door.
//!
//! `rs_cam_cli` declares no `[lib]`, so an integration test cannot reach
//! this module. The `#[cfg(test)]` module below is the sentry.

use rs_cam_core::session::{Command, Effects, ProjectSession, SessionError};

/// Apply one command to the session, and report what it changed.
///
/// The function is thin on purpose. It exists so the CLI call sites
/// share one entry point into [`ProjectSession::apply`], and so the bin
/// can prove that entry point is the one they take.
///
/// # Why the CLI discards the answer
///
/// WP19. A caller here writes `let _ = apply_command(…)` or takes the
/// `Err` arm alone. That is correct for this crate and for this crate
/// only: `rs_cam_cli` holds no `ToolpathRuntime` and no `stale_since`
/// (`rg -n "stale_since" crates/rs_cam_cli/src` is empty), and it
/// generates every enabled operation in the run, so a stale set has no
/// consumer. It draws no viewport either, so `simulation_cleared` has
/// nothing to reach. The view surfaces mirror both halves; see
/// `crates/rs_cam_viz/tests/effects_are_stamped_wp19.rs`, which names
/// this doc as the one reason for every CLI site.
pub fn apply_command(
    session: &mut ProjectSession,
    command: Command,
) -> Result<Effects, SessionError> {
    session.apply(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs_cam_core::session::SetToolpathParamArgs;

    /// The door reaches `ProjectSession::apply`, and `apply` reaches the
    /// setter body.
    ///
    /// An empty session carries no toolpath, so the body's own index
    /// check answers. A door that stopped short of `apply` could not
    /// produce this error. `SessionError` derives no `PartialEq`, so the
    /// test matches the variant.
    #[test]
    fn apply_command_reaches_the_session_door() {
        let mut session = ProjectSession::new_empty();
        let command = Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(1234.0),
        });
        let outcome = apply_command(&mut session, command);
        assert!(
            matches!(outcome, Err(SessionError::ToolpathNotFound(0))),
            "the door must reach the setter body, which refuses an \
             absent toolpath index"
        );
    }
}
