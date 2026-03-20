use std::path::Path;
use std::sync::Arc;

use rs_cam_core::mesh::TriangleMesh;

use crate::state::job::{LoadedModel, ModelId, ModelKind};

/// Import an STL file, returning a LoadedModel.
pub fn import_stl(path: &Path, id: ModelId) -> Result<LoadedModel, String> {
    let mesh = TriangleMesh::from_stl(path).map_err(|e| format!("Failed to load STL: {e}"))?;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.stl".to_string());

    Ok(LoadedModel {
        id,
        path: path.to_path_buf(),
        name,
        kind: ModelKind::Stl,
        mesh: Some(Arc::new(mesh)),
    })
}
