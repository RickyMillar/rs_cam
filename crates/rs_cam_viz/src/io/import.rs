use std::path::Path;
use std::sync::Arc;

use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::svg_input::load_svg;

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
        polygons: None,
    })
}

/// Import an SVG file, returning a LoadedModel with polygons.
pub fn import_svg(path: &Path, id: ModelId) -> Result<LoadedModel, String> {
    let polygons = load_svg(path, 0.1).map_err(|e| format!("Failed to load SVG: {e}"))?;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.svg".to_string());

    Ok(LoadedModel {
        id,
        path: path.to_path_buf(),
        name,
        kind: ModelKind::Svg,
        mesh: None,
        polygons: Some(Arc::new(polygons)),
    })
}
