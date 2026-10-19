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
mod import_options;
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
use crate::selection::ItemSet;
use crate::session::{LoadedGeometry, LoadedModel, SessionError};

pub use import_options::{DrawingImport, ImportOptions, ImportReport};

/// The largest file any import door reads, in bytes.
///
/// A file over this limit is refused before the parser sees it, so a stray
/// multi-gigabyte file cannot exhaust memory inside a third-party parser.
/// STEP has held this guard since it landed; DXF and SVG hold it too
/// (EDG-03, 2026-09-17).
pub const MAX_IMPORT_FILE_SIZE: u64 = 500 * 1024 * 1024;

/// A file that is over the import size limit, in whole MB.
pub(crate) struct OversizeFile {
    /// The measured file size.
    pub size_mb: u64,
    /// The limit it passed.
    pub limit_mb: u64,
}

/// Measure the file at `path` against `limit_bytes`.
///
/// `None` means the file is within the limit. Each door keeps its own error
/// type and turns a `Some` into its own `FileTooLarge` variant: the three
/// importers share the mechanism, not the error.
pub(crate) fn file_size_over_limit(
    path: &Path,
    limit_bytes: u64,
) -> Result<Option<OversizeFile>, std::io::Error> {
    let size_bytes = std::fs::metadata(path)?.len();
    if size_bytes > limit_bytes {
        Ok(Some(OversizeFile {
            size_mb: size_bytes / (1024 * 1024),
            limit_mb: limit_bytes / (1024 * 1024),
        }))
    } else {
        Ok(None)
    }
}

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
/// `options` (P4.8) reach the drawing doors: the join tolerance. The error
/// is the reason, as a sentence; each door names the model and picks its
/// own [`SessionError`] variant.
pub(crate) fn load_geometry(
    path: &Path,
    kind: ModelKind,
    scale: f64,
    options: &ImportOptions,
) -> Result<LoadedGeometry, String> {
    options.validate()?;
    match kind {
        ModelKind::Stl => {
            let mesh = TriangleMesh::from_stl_scaled(path, scale)
                .map_err(|e| format!("STL load failed: {e}"))?;
            Ok(LoadedGeometry::Mesh(mesh))
        }
        ModelKind::Svg => {
            // P2.1: the door scales before it classifies circle-like rings,
            // so the circle floor is in mm (G-DRILLCENTROID). P4.8: the
            // join runs in user units, before the scale.
            let import =
                crate::io::svg_input::load_svg(path, 0.1, scale, options.join_tolerance_mm)
                    .map_err(|e| format!("SVG load failed: {e}"))?;
            Ok(LoadedGeometry::Drawing(import.items, import.report))
        }
        ModelKind::Dxf => {
            // Two scales, in this order: `$INSUNITS` inside the door, then
            // the declared units here. The join runs inside the door, in
            // mm of the `$INSUNITS` frame, before the declared units.
            let import = crate::io::dxf_input::load_dxf(
                path,
                crate::selection::item::CIRCLE_ARC_STEP_DEG,
                options.join_tolerance_mm,
            )
            .map_err(|e| format!("DXF load failed: {e}"))?;
            let items = import
                .items
                .scaled(scale)
                .map_err(|e| format!("DXF load failed: {e}"))?;
            Ok(LoadedGeometry::Drawing(items, import.report))
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

/// The default declared units of a new import (P4.8). An SVG takes
/// [`svg_input::svg_default_units`] of the size it declares, every other
/// kind mm. The GUI import dialog and MCP `import_model` both read it, so
/// the operator ruling O-2 (the SVG scale) changes one place.
pub fn default_import_units(path: &Path, kind: ModelKind) -> ModelUnits {
    match kind {
        ModelKind::Svg => svg_input::svg_declared_size_of_file(path)
            .map(|declared| svg_input::svg_default_units(&declared))
            .unwrap_or(svg_input::SVG_DEFAULT_UNITS),
        _ => ModelUnits::Millimeters,
    }
}

/// Load a model file into a [`LoadedModel`] with the given id.
///
/// The interactive import door. It reads the geometry through
/// [`load_geometry`] and names the model after the file.
///
/// `options` (P4.8) are the import options; the model keeps them, so a
/// save writes them and a reload reads the file with them again.
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
    options: ImportOptions,
) -> Result<LoadedModel, SessionError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("unknown.{kind:?}").to_lowercase());
    let geometry = load_geometry(path, kind, units.scale_factor(), &options).map_err(|detail| {
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
        options,
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
    import_options: ImportOptions,
) -> LoadedModel {
    let mut model = LoadedModel {
        id,
        name,
        mesh: None,
        items: Arc::new(ItemSet::default()),
        path: path.to_path_buf(),
        kind,
        units,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options,
        import_report: None,
        drawing: Default::default(),
    };
    match geometry {
        LoadedGeometry::Mesh(mesh) => {
            let winding = mesh.check_winding();
            model.winding_report = Some(winding.inconsistency_fraction * 100.0);
            model.mesh = Some(Arc::new(mesh));
        }
        LoadedGeometry::Drawing(items, report) => {
            model.items = Arc::new(items);
            model.import_report = Some(report);
        }
        LoadedGeometry::Enriched(enriched) => {
            model.items = Arc::new(brep_face_items(&enriched));
            model.mesh = Some(Arc::clone(&enriched.mesh));
            model.enriched_mesh = Some(Arc::new(enriched));
        }
    }
    model.bind_items();
    model
}

/// The STEP source (P2.1): one `Face` item for each planar BREP face that
/// has 2D loops (the faces within the `|normal.z| >= 0.95` gate of
/// `enriched_mesh::compute_2d_boundary`), in face order.
///
/// - The rings are the face's own loops: the outer loop first, then each
///   inner loop of 3 or more points (as `face_boundary_as_polygon`).
/// - The normal is SIGNED. It is the area-weighted normal of the face's
///   triangles, so an up face points to +Z and a down face to -Z. The
///   plane normal of `SurfaceParams` is not used: the STEP door sets its
///   sign from the sign of the plane Z, not from the face.
/// - `plane_z` is the top of the face (`bbox.max.z`), the Z that the face
///   pick reads today.
///
/// A face whose triangles have no area gives no item: it has no normal and
/// nothing to cut.
pub fn brep_face_items(enriched: &crate::geometry::enriched_mesh::EnrichedMesh) -> ItemSet {
    use crate::selection::{FaceItem, Item, Shape, SourceRef};
    let mesh = &enriched.mesh;
    let mut items = Vec::new();
    for group in &enriched.face_groups {
        let Some(loops) = group.boundary_loops_2d.as_ref() else {
            continue;
        };
        let Some((outer, inner)) = loops.split_first() else {
            continue;
        };
        if outer.len() < 3 {
            continue;
        }
        let mut sum = crate::geo::V3::zeros();
        for tri in mesh
            .triangles
            .get(group.triangle_range.clone())
            .unwrap_or(&[])
        {
            let corner = |k: usize| {
                tri.get(k)
                    .and_then(|&v| mesh.vertices.get(v as usize))
                    .copied()
            };
            if let (Some(a), Some(b), Some(c)) = (corner(0), corner(1), corner(2)) {
                sum += (b - a).cross(&(c - a));
            }
        }
        let norm = sum.norm();
        if !(norm > 0.0 && norm.is_finite()) {
            continue;
        }
        let holes: Vec<Vec<crate::geo::P2>> =
            inner.iter().filter(|h| h.len() >= 3).cloned().collect();
        let face = FaceItem {
            rings: crate::polygon::Polygon2::with_holes(outer.clone(), holes),
            plane_z: group.bbox.max.z,
            normal: sum / norm,
            brep: Some(group.id),
        };
        // The checks above meet every face invariant.
        if let Ok(item) = Item::new(SourceRef::ToolpathModel, None, Shape::Face(face)) {
            items.push(item);
        }
    }
    ItemSet::new(items)
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
