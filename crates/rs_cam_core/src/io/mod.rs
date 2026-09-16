//! Model file import helpers shared between `rs_cam_viz` and `rs_cam_mcp`.
//!
//! These functions load mesh/polygon geometry from disk and build a
//! [`LoadedModel`] with the correct metadata. They intentionally live in core
//! so that headless callers (CLI, MCP) can import files without depending on
//! the GUI crate.
//!
//! The folder holds every door that reads a file from disk: the three
//! importers and the two on-disk TOML libraries.

pub mod dxf_input;
pub mod machine_library;
mod named_toml_library;
#[cfg(feature = "step")]
pub mod step_input;
pub mod svg_input;
pub mod tool_library;

use std::path::Path;
use std::sync::Arc;

use crate::compute::stock_config::{ModelKind, ModelUnits};
use crate::mesh::TriangleMesh;
use crate::session::{LoadedGeometry, LoadedModel, SessionError};

/// Read the geometry of one model file, at one scale.
///
/// **C13: one door.** There used to be two readers of a model file, and
/// they had drifted four times (G-UNITSRELOAD, G-STEPUNITS, a BREP
/// downgrade, and a dropped DXF scale). Each fix had to be made twice,
/// and the fourth was found only because a test compared the two. This is
/// the one reader. [`load_model_file`] wraps it for the interactive door,
/// and `session::project_file::load_model_geometry` for the project door.
///
/// `scale` comes from the declared [`ModelUnits`] and reaches EVERY kind.
/// The error is the reason, as a sentence; each door names the model and
/// picks its own [`SessionError`] variant.
pub(crate) fn load_geometry(
    path: &Path,
    kind: ModelKind,
    scale: f64,
) -> Result<LoadedGeometry, String> {
    match kind {
        ModelKind::Stl => {
            let mesh = TriangleMesh::from_stl_scaled(path, scale)
                .map_err(|e| format!("STL load failed: {e}"))?;
            Ok(LoadedGeometry::Mesh(mesh))
        }
        ModelKind::Svg => {
            let mut polygons = crate::io::svg_input::load_svg(path, 0.1)
                .map_err(|e| format!("SVG load failed: {e}"))?;
            apply_uniform_scale_2d(&mut polygons, scale);
            // G-DRILLCENTROID: circle-like closed rings are the SVG's drill
            // targets (usvg has already flattened every `<circle>`).
            // Classified AFTER the unit scale so the floor is in mm.
            let drill_targets = crate::io::svg_input::circle_like_drill_targets(&polygons);
            let layers = crate::io::svg_input::circle_like_layers(&drill_targets);
            Ok(LoadedGeometry::Polygons(polygons, drill_targets, layers))
        }
        ModelKind::Dxf => {
            let import = crate::io::dxf_input::load_dxf_full(path, 5.0)
                .map_err(|e| format!("DXF load failed: {e}"))?;
            let mut polygons = import.polygons;
            let mut drill_targets = import.drill_targets;
            apply_uniform_scale_2d(&mut polygons, scale);
            apply_uniform_scale_targets(&mut drill_targets, scale);
            Ok(LoadedGeometry::Polygons(
                polygons,
                drill_targets,
                import.layers,
            ))
        }
        #[cfg(feature = "step")]
        ModelKind::Step => {
            // G-STEPUNITS: `truck-stepio` performs no unit conversion, so
            // the declared units are the ONLY scale a STEP file gets.
            // `apply_uniform_scale` moves the BREP data as well as the
            // mesh, so face selection survives.
            let mut enriched = crate::io::step_input::load_step(path, 0.1)
                .map_err(|e| format!("STEP load failed: {e}"))?;
            if (scale - 1.0).abs() > 1e-9 {
                enriched.apply_uniform_scale(scale);
            }
            // The BREP topology is kept. Downgrading to a flat mesh here
            // was the prior bug: it silently broke face-selective
            // operations.
            Ok(LoadedGeometry::Enriched(enriched))
        }
        #[cfg(not(feature = "step"))]
        ModelKind::Step => {
            Err("STEP support not compiled into this build (enable the `step` feature)".to_owned())
        }
    }
}

/// Load a model file into a [`LoadedModel`] with the given id.
///
/// The interactive import door. It reads the geometry through
/// [`load_geometry`] and names the model after the file.
///
/// The `units` parameter controls the scale factor applied to the
/// imported geometry, and is persisted on the returned model. It is the
/// DECLARED units, and it is kept as declared for every kind. The STEP
/// arm used to record `Millimeters` after scaling; a save then wrote
/// `units = millimeters` for an inch-authored file, and the next load
/// re-imported the source 25.4 times too small.
pub fn load_model_file(
    path: &Path,
    id: usize,
    kind: ModelKind,
    units: ModelUnits,
) -> Result<LoadedModel, SessionError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("unknown.{kind:?}").to_lowercase());
    let geometry = load_geometry(path, kind, units.scale_factor()).map_err(|detail| {
        SessionError::ModelLoad {
            name: name.clone(),
            detail,
        }
    })?;
    Ok(model_from_geometry(
        geometry,
        id,
        name,
        path,
        Some(kind),
        Some(units),
    ))
}

/// Build a [`LoadedModel`] around geometry the one reader returned.
///
/// **`winding_report` is set here, for every mesh.** The project door
/// used to write `None`, so a model reloaded from a project file lost the
/// inconsistency figure the import door had shown (I01 drift 9).
pub(crate) fn model_from_geometry(
    geometry: LoadedGeometry,
    id: usize,
    name: String,
    path: &Path,
    kind: Option<ModelKind>,
    units: Option<ModelUnits>,
) -> LoadedModel {
    let mut model = LoadedModel {
        id,
        name,
        mesh: None,
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: path.to_path_buf(),
        kind,
        units,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    match geometry {
        LoadedGeometry::Mesh(mesh) => {
            let winding = mesh.check_winding();
            model.winding_report = Some(winding.inconsistency_fraction * 100.0);
            model.mesh = Some(Arc::new(mesh));
        }
        LoadedGeometry::Polygons(polygons, drill_targets, layers) => {
            model.polygons = Some(Arc::new(polygons));
            model.drill_targets = Arc::new(drill_targets);
            model.layers = Arc::new(layers);
        }
        LoadedGeometry::Enriched(enriched) => {
            model.mesh = Some(Arc::clone(&enriched.mesh));
            model.enriched_mesh = Some(Arc::new(enriched));
        }
    }
    model
}

/// THE table of model file extensions, in lower case, with the kind each
/// one names.
///
/// [`infer_kind_from_path`] and [`MODEL_FILE_EXTENSIONS`] both read this
/// table, so a new format reaches the classifier and the file dialog in one
/// edit. The GUI used to keep a hand-written second list beside this one.
const MODEL_EXTENSION_KINDS: &[(&str, ModelKind)] = &[
    ("stl", ModelKind::Stl),
    ("svg", ModelKind::Svg),
    ("dxf", ModelKind::Dxf),
    ("step", ModelKind::Step),
    ("stp", ModelKind::Step),
];

/// File-dialog extensions accepted for model import.
///
/// Both cases of every entry in [`MODEL_EXTENSION_KINDS`] appear here.
/// Keep the upper-case spellings: a native dialog filter can otherwise
/// hide a valid file, even though [`infer_kind_from_path`] classifies an
/// extension case-insensitively.
/// `model_file_extensions_cover_the_table` pins the two against drift.
pub const MODEL_FILE_EXTENSIONS: &[&str] = &[
    "stl", "STL", "svg", "SVG", "dxf", "DXF", "step", "STEP", "stp", "STP",
];

/// Infer a model kind from a file extension.
pub fn infer_kind_from_path(path: &Path) -> Option<ModelKind> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())?
        .to_ascii_lowercase();
    MODEL_EXTENSION_KINDS
        .iter()
        .find(|(key, _)| *key == ext)
        .map(|(_, kind)| *kind)
}

pub(crate) fn apply_uniform_scale_2d(polygons: &mut [crate::polygon::Polygon2], scale: f64) {
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
        // `exterior` moved — see the mutation contract on `Polygon2`.
        poly.invalidate_bbox();
    }
}

pub(crate) fn apply_uniform_scale_targets(
    targets: &mut [crate::io::dxf_input::DrillTarget],
    scale: f64,
) {
    if (scale - 1.0).abs() < 1e-9 {
        return;
    }
    for t in targets {
        t.x *= scale;
        t.y *= scale;
        if let crate::io::dxf_input::DrillTargetKind::CircleCenter { diameter } = &mut t.kind {
            *diameter *= scale;
        }
    }
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

    /// The dialog list and the classifier read one table.
    ///
    /// The GUI kept a second, hand-written extension list beside this
    /// module until the C-series cleanup. A format added to one list and
    /// not to the other let the operator pick a file the classifier then
    /// refused, or hid a file the classifier accepts.
    #[test]
    fn model_file_extensions_cover_the_table() {
        for (key, kind) in MODEL_EXTENSION_KINDS {
            let lower = format!("model.{key}");
            let upper = format!("model.{}", key.to_ascii_uppercase());
            assert_eq!(
                infer_kind_from_path(Path::new(&lower)),
                Some(*kind),
                "the classifier must answer for `{key}`"
            );
            assert!(
                MODEL_FILE_EXTENSIONS.contains(key),
                "the dialog list must hold the lower-case `{key}`"
            );
            assert!(
                MODEL_FILE_EXTENSIONS.contains(&key.to_ascii_uppercase().as_str()),
                "the dialog list must hold the upper-case `{key}`"
            );
            assert_eq!(
                infer_kind_from_path(Path::new(&upper)),
                Some(*kind),
                "the classifier reads an extension case-insensitively"
            );
        }
        assert_eq!(
            MODEL_FILE_EXTENSIONS.len(),
            MODEL_EXTENSION_KINDS.len() * 2,
            "the dialog list must hold both cases of every table entry, and nothing else"
        );
        assert_eq!(infer_kind_from_path(Path::new("model.gcode")), None);
        assert_eq!(infer_kind_from_path(Path::new("model")), None);
    }
}
