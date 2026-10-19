//! Thin viz-layer wrappers around `rs_cam_core::io::load_model_file`.
//!
//! Core owns the actual import logic; viz just maps `SessionError` into
//! `VizError` for the controller/GUI layer.

use std::path::Path;

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::io::{ImportOptions, load_model_file};

use crate::error::VizError;
use crate::state::job::LoadedModel;

fn core_err_to_viz(err: &rs_cam_core::session::SessionError) -> VizError {
    VizError::Other(err.to_string())
}

/// Import a model file at the given units, with the given import options
/// (P4.8).
///
/// Every door names its options: a new import takes the dialog's, and a
/// refresh (reload, rescale, relink) takes the record's, so a refresh
/// never resets the join tolerance to the default.
pub fn import_model(
    path: &Path,
    id: usize,
    kind: ModelKind,
    units: ModelUnits,
    options: ImportOptions,
) -> Result<LoadedModel, VizError> {
    load_model_file(path, id, kind, units, options).map_err(|e| core_err_to_viz(&e))
}
