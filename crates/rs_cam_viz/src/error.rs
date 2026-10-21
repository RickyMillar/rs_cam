use rs_cam_core::session::SessionError;
use thiserror::Error;

/// Structured error type for the viz crate's I/O and controller layer.
///
/// Preserves typed error information from core while providing
/// user-facing messages for GUI display.
///
/// F087/F110: [`Self::Session`] is the one mapping of a core
/// [`SessionError`], so a caller matches the core variant and a
/// refusal keeps its own text. Only the project-load door builds
/// [`Self::ProjectLoad`]; a non-load refusal never reads "Load failed:".
#[derive(Error, Debug)]
pub enum VizError {
    /// The project file did not load. Built by the project-load door
    /// (`AppController::open_job_from_path`) alone.
    #[error("Failed to load project: {0}")]
    ProjectLoad(#[source] Box<SessionError>),

    #[error("Export failed: {0}")]
    Export(String),

    /// A core refusal or failure, with its own text. Boxed, as is
    /// [`Self::ProjectLoad`]: `SessionError` is 120 bytes, and an
    /// unboxed one breaks `clippy::result_large_err` on `VizError`.
    #[error("{0}")]
    Session(#[source] Box<SessionError>),

    /// A viz-layer refusal that has no core error below it.
    #[error("{0}")]
    Other(String),
}

impl From<SessionError> for VizError {
    fn from(e: SessionError) -> Self {
        Self::Session(Box::new(e))
    }
}

impl VizError {
    /// The project-load refusal, as a `map_err` argument.
    pub fn project_load(e: SessionError) -> Self {
        Self::ProjectLoad(Box::new(e))
    }

    /// Short user-facing message suitable for toast/notification display.
    pub fn user_message(&self) -> String {
        match self {
            Self::ProjectLoad(e) => format!("Load failed: {e}"),
            Self::Export(msg) => format!("Export failed: {msg}"),
            Self::Session(e) => e.to_string(),
            Self::Other(msg) => msg.clone(),
        }
    }
}
