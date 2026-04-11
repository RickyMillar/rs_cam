//! Thin viz-layer wrappers around `rs_cam_core::io::load_model_file`.
//!
//! Core owns the actual import logic; viz just maps `SessionError` into
//! `VizError` for the controller/GUI layer.

use std::path::Path;

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::io::load_model_file;

use crate::error::VizError;
use crate::state::job::LoadedModel;

fn core_err_to_viz(err: &rs_cam_core::session::SessionError) -> VizError {
    VizError::Other(err.to_string())
}

fn units_for_scale(scale: f64) -> ModelUnits {
    if (scale - 1.0).abs() < 1e-9 {
        ModelUnits::Millimeters
    } else {
        ModelUnits::Custom(scale)
    }
}

/// Import an STL file with a given scale factor.
pub fn import_stl(path: &Path, id: usize, scale: f64) -> Result<LoadedModel, VizError> {
    load_model_file(path, id, ModelKind::Stl, units_for_scale(scale))
        .map_err(|e| core_err_to_viz(&e))
}

/// Import an SVG file, returning a LoadedModel with polygons.
pub fn import_svg(path: &Path, id: usize, scale: f64) -> Result<LoadedModel, VizError> {
    load_model_file(path, id, ModelKind::Svg, units_for_scale(scale))
        .map_err(|e| core_err_to_viz(&e))
}

/// Import a DXF file, returning a LoadedModel with polygons.
pub fn import_dxf(path: &Path, id: usize, scale: f64) -> Result<LoadedModel, VizError> {
    load_model_file(path, id, ModelKind::Dxf, units_for_scale(scale))
        .map_err(|e| core_err_to_viz(&e))
}

/// Import a STEP file, returning a LoadedModel with enriched mesh.
pub fn import_step(path: &Path, id: usize, scale: f64) -> Result<LoadedModel, VizError> {
    load_model_file(path, id, ModelKind::Step, units_for_scale(scale))
        .map_err(|e| core_err_to_viz(&e))
}

/// Import a model using persisted kind/units metadata.
pub fn import_model(
    path: &Path,
    id: usize,
    kind: ModelKind,
    units: ModelUnits,
) -> Result<LoadedModel, VizError> {
    load_model_file(path, id, kind, units).map_err(|e| core_err_to_viz(&e))
}
