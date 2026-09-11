//! The CLI's door into the core command registry.
//!
//! `run`, `job` and `smoke` all write operation parameters. Each one
//! called `ProjectSession::set_toolpath_param` directly before WP1, so
//! the three sites could drift apart. Each one now builds a
//! [`rs_cam_core::session::Command`] and takes this door.
//!
//! `rs_cam_cli` declares no `[lib]`, so an integration test cannot reach
//! this module. The `#[cfg(test)]` module below is the sentry.

use rs_cam_core::session::{Command, Effects, ProjectSession, SessionError};

/// Apply one command to the session, and report what it changed.
///
/// The function is thin on purpose. It exists so the three CLI call
/// sites share one entry point into [`ProjectSession::apply`], and so
/// the bin can prove that entry point is the one they take.
pub fn apply_command(
    session: &mut ProjectSession,
    command: Command,
) -> Result<Effects, SessionError> {
    session.apply(command)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
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
