//! Model file import helpers shared between `rs_cam_viz` and `rs_cam_mcp`.
//!
//! These functions load mesh/polygon geometry from disk and build a
//! [`LoadedModel`] with the correct metadata. They intentionally live in core
//! so that headless callers (CLI, MCP) can import files without depending on
//! the GUI crate.

use std::path::Path;
use std::sync::Arc;

use crate::compute::stock_config::{ModelKind, ModelUnits};
use crate::mesh::TriangleMesh;
use crate::session::{LoadedModel, SessionError};

/// Load a model file into a [`LoadedModel`] with the given id.
///
/// Dispatches on `kind` to the underlying loader:
/// - `Stl` → [`TriangleMesh::from_stl_scaled`]
/// - `Svg` → [`crate::svg_input::load_svg`]
/// - `Dxf` → [`crate::dxf_input::load_dxf`]
/// - `Step` → [`crate::step_input::load_step`] (requires `step` feature)
///
/// The `units` parameter controls the scale factor applied to the imported
/// geometry (and is persisted on the returned model).
pub fn load_model_file(
    path: &Path,
    id: usize,
    kind: ModelKind,
    units: ModelUnits,
) -> Result<LoadedModel, SessionError> {
    let scale = units.scale_factor();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("unknown.{kind:?}").to_lowercase());

    match kind {
        ModelKind::Stl => {
            let mesh = TriangleMesh::from_stl_scaled(path, scale)
                .map_err(|e| SessionError::Io(std::io::Error::other(e.to_string())))?;
            let winding = mesh.check_winding();
            Ok(LoadedModel {
                id,
                name,
                mesh: Some(Arc::new(mesh)),
                polygons: None,
                path: path.to_path_buf(),
                kind: Some(kind),
                units: Some(units),
                enriched_mesh: None,
                winding_report: Some(winding.inconsistency_fraction * 100.0),
                load_error: None,
            })
        }
        ModelKind::Svg => {
            let mut polygons = crate::svg_input::load_svg(path, 0.1)
                .map_err(|e| SessionError::Io(std::io::Error::other(e.to_string())))?;
            apply_uniform_scale_2d(&mut polygons, scale);
            Ok(LoadedModel {
                id,
                name,
                mesh: None,
                polygons: Some(Arc::new(polygons)),
                path: path.to_path_buf(),
                kind: Some(kind),
                units: Some(units),
                enriched_mesh: None,
                winding_report: None,
                load_error: None,
            })
        }
        ModelKind::Dxf => {
            let mut polygons = crate::dxf_input::load_dxf(path, 5.0)
                .map_err(|e| SessionError::Io(std::io::Error::other(e.to_string())))?;
            apply_uniform_scale_2d(&mut polygons, scale);
            Ok(LoadedModel {
                id,
                name,
                mesh: None,
                polygons: Some(Arc::new(polygons)),
                path: path.to_path_buf(),
                kind: Some(kind),
                units: Some(units),
                enriched_mesh: None,
                winding_report: None,
                load_error: None,
            })
        }
        #[cfg(feature = "step")]
        ModelKind::Step => {
            let mut enriched = crate::step_input::load_step(path, 0.1)
                .map_err(|e| SessionError::Io(std::io::Error::other(e.to_string())))?;
            if (scale - 1.0).abs() > 1e-9 {
                enriched.apply_uniform_scale(scale);
            }
            let mesh_arc = enriched.mesh_arc();
            Ok(LoadedModel {
                id,
                name,
                mesh: Some(mesh_arc),
                polygons: None,
                path: path.to_path_buf(),
                kind: Some(kind),
                units: Some(ModelUnits::Millimeters),
                enriched_mesh: Some(Arc::new(enriched)),
                winding_report: None,
                load_error: None,
            })
        }
        #[cfg(not(feature = "step"))]
        ModelKind::Step => Err(SessionError::InvalidParam(
            "STEP support not compiled into this build".to_owned(),
        )),
    }
}

/// Infer a model kind from a file extension.
pub fn infer_kind_from_path(path: &Path) -> Option<ModelKind> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .and_then(|ext| match ext.as_str() {
            "stl" => Some(ModelKind::Stl),
            "svg" => Some(ModelKind::Svg),
            "dxf" => Some(ModelKind::Dxf),
            "step" | "stp" => Some(ModelKind::Step),
            _ => None,
        })
}

fn apply_uniform_scale_2d(polygons: &mut [crate::polygon::Polygon2], scale: f64) {
    if (scale - 1.0).abs() < 1e-9 {
        return;
    }
    for poly in polygons {
        for pt in &mut poly.exterior {
            pt.x *= scale;
            pt.y *= scale;
        }
        for hole in &mut poly.holes {
            for pt in hole {
                pt.x *= scale;
                pt.y *= scale;
            }
        }
    }
}
