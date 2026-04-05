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
        .unwrap_or_else(|| "unknown.stl".to_owned());

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
///
/// `scale` multiplies all point coordinates after loading (1.0 = no change).
pub fn import_svg(path: &Path, id: ModelId, scale: f64) -> Result<LoadedModel, VizError> {
    let mut polygons = load_svg(path, 0.1)?;

    if (scale - 1.0).abs() > 1e-9 {
        for poly in &mut polygons {
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

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.svg".to_owned());

    let units = if (scale - 1.0).abs() < 1e-9 {
        ModelUnits::Millimeters
    } else {
        ModelUnits::Custom(scale)
    };

    Ok(LoadedModel {
        id,
        path: path.to_path_buf(),
        name,
        kind: ModelKind::Svg,
        mesh: None,
        polygons: Some(Arc::new(polygons)),
        enriched_mesh: None,
        units,
        winding_report: None,
        load_error: None,
    })
}

/// Import a DXF file, returning a LoadedModel with polygons.
///
/// `scale` multiplies all point coordinates after loading (1.0 = no change).
/// The `5.0` arc tolerance is for tessellation and is unrelated to scale.
pub fn import_dxf(path: &Path, id: ModelId, scale: f64) -> Result<LoadedModel, VizError> {
    let mut polygons = load_dxf(path, 5.0)?;

    if (scale - 1.0).abs() > 1e-9 {
        for poly in &mut polygons {
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

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.dxf".to_owned());

    let units = if (scale - 1.0).abs() < 1e-9 {
        ModelUnits::Millimeters
    } else {
        ModelUnits::Custom(scale)
    };

    Ok(LoadedModel {
        id,
        path: path.to_path_buf(),
        name,
        kind: ModelKind::Dxf,
        mesh: None,
        polygons: Some(Arc::new(polygons)),
        enriched_mesh: None,
        units,
        winding_report: None,
        load_error: None,
    })
}

/// Import a STEP file, returning a LoadedModel with enriched mesh.
///
/// `scale` uniformly scales all tessellated vertices (1.0 = mm as-is).
pub fn import_step(path: &Path, id: ModelId, scale: f64) -> Result<LoadedModel, VizError> {
    let mut enriched = rs_cam_core::step_input::load_step(path, 0.1)?;

    // Apply uniform scale to tessellated geometry if not 1:1.
    if (scale - 1.0).abs() > 1e-9 {
        enriched.apply_uniform_scale(scale);
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.step".to_owned());

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
    let scale = units.scale_factor();
    let mut model = match kind {
        ModelKind::Stl => import_stl(path, id, scale)?,
        ModelKind::Svg => import_svg(path, id, scale)?,
        ModelKind::Dxf => import_dxf(path, id, scale)?,
        ModelKind::Step => import_step(path, id, scale)?,
    };
    model.units = units;
    Ok(model)
}
