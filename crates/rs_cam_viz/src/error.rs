use thiserror::Error;

/// Structured error type for the viz crate's I/O and controller layer.
///
/// Preserves typed error information from core while providing
/// user-facing messages for GUI display.
#[derive(Error, Debug)]
pub enum VizError {
    #[error("STL import failed: {0}")]
    StlImport(#[from] rs_cam_core::mesh::MeshError),

    #[error("SVG import failed: {0}")]
    SvgImport(#[from] rs_cam_core::svg_input::SvgError),

    #[error("DXF import failed: {0}")]
    DxfImport(#[from] rs_cam_core::dxf_input::DxfError),

    #[error("STEP import failed: {0}")]
    StepImport(#[from] rs_cam_core::step_input::StepImportError),

    #[error("Failed to save project: {0}")]
    ProjectSave(String),

    #[error("Failed to load project: {0}")]
    ProjectLoad(String),

    #[error("Export failed: {0}")]
    Export(String),

    /// Escape hatch for incremental migration.
    #[error("{0}")]
    Other(String),
}

impl VizError {
    /// Short user-facing message suitable for toast/notification display.
    pub fn user_message(&self) -> String {
        match self {
            Self::StlImport(e) => format!("Failed to import STL file: {e}"),
            Self::SvgImport(e) => format!("Failed to import SVG file: {e}"),
            Self::DxfImport(e) => format!("Failed to import DXF file: {e}"),
            Self::StepImport(e) => format!("Failed to import STEP file: {e}"),
            Self::ProjectSave(msg) => format!("Save failed: {msg}"),
            Self::ProjectLoad(msg) => format!("Load failed: {msg}"),
            Self::Export(msg) => format!("Export failed: {msg}"),
            Self::Other(msg) => msg.clone(),
        }
    }
}

impl From<String> for VizError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<rs_cam_core::session::SessionError> for VizError {
    fn from(e: rs_cam_core::session::SessionError) -> Self {
        Self::ProjectLoad(e.to_string())
    }
}
