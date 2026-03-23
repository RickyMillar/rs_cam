use std::path::Path;
use std::sync::Arc;

use rs_cam_core::dxf_input::load_dxf;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::svg_input::load_svg;

use crate::error::VizError;
use crate::state::job::{LoadedModel, ModelId, ModelKind, ModelUnits};

/// Import an STL file with a given scale factor.
pub fn import_stl(path: &Path, id: ModelId, scale: f64) -> Result<LoadedModel, VizError> {
    let mesh = TriangleMesh::from_stl_scaled(path, scale)?;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.stl".to_string());

    let units = if (scale - 1.0).abs() < 1e-9 {
        ModelUnits::Millimeters
    } else {
        ModelUnits::Custom(scale)
    };

    // Check winding consistency for normal flip warning
    let winding = mesh.check_winding();
    let winding_pct = winding.inconsistency_fraction * 100.0;

    Ok(LoadedModel {
        id,
        path: path.to_path_buf(),
        name,
        kind: ModelKind::Stl,
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        enriched_mesh: None,
        units,
        winding_report: Some(winding_pct),
        load_error: None,
    })
}

/// Import an SVG file, returning a LoadedModel with polygons.
pub fn import_svg(path: &Path, id: ModelId) -> Result<LoadedModel, VizError> {
    let polygons = load_svg(path, 0.1)?;

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
        enriched_mesh: None,
        units: ModelUnits::Millimeters,
        winding_report: None,
        load_error: None,
    })
}

/// Import a DXF file, returning a LoadedModel with polygons.
pub fn import_dxf(path: &Path, id: ModelId) -> Result<LoadedModel, VizError> {
    let polygons = load_dxf(path, 5.0)?;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.dxf".to_string());

    Ok(LoadedModel {
        id,
        path: path.to_path_buf(),
        name,
        kind: ModelKind::Dxf,
        mesh: None,
        polygons: Some(Arc::new(polygons)),
        enriched_mesh: None,
        units: ModelUnits::Millimeters,
        winding_report: None,
        load_error: None,
    })
}

/// Import a STEP file, returning a LoadedModel with enriched mesh.
pub fn import_step(path: &Path, id: ModelId) -> Result<LoadedModel, VizError> {
    let enriched = rs_cam_core::step_input::load_step(path, 0.1)?;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.step".to_string());

    let mesh_arc = enriched.mesh_arc();
    Ok(LoadedModel {
        id,
        path: path.to_path_buf(),
        name,
        kind: ModelKind::Step,
        mesh: Some(mesh_arc),
        polygons: None,
        enriched_mesh: Some(Arc::new(enriched)),
        units: ModelUnits::Millimeters,
        winding_report: None,
        load_error: None,
    })
}

/// Import a model using persisted kind/units metadata.
pub fn import_model(
    path: &Path,
    id: ModelId,
    kind: ModelKind,
    units: ModelUnits,
) -> Result<LoadedModel, VizError> {
    let mut model = match kind {
        ModelKind::Stl => import_stl(path, id, units.scale_factor())?,
        ModelKind::Svg => import_svg(path, id)?,
        ModelKind::Dxf => import_dxf(path, id)?,
        ModelKind::Step => import_step(path, id)?,
    };
    model.units = units;
    Ok(model)
}
