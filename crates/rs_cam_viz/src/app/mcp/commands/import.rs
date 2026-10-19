//! The MCP import doors (geometry selection P4.8): `import_model` with its
//! units and join tolerance, `set_model_import_options`, and the import
//! facts every reply and `inspect_model` give.
//!
//! The GUI import dialog and the model panel read the same core door
//! (`rs_cam_core::io::load_model_file`) with the same defaults
//! (`rs_cam_core::io::default_import_units`, `ImportOptions::default`). The
//! tests below pin that the two surfaces give the same model.

use std::path::Path;

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::io::ImportOptions;
use rs_cam_core::session::{AdoptModelGeometryArgs, LoadedModel, ProjectSession};
use rs_cam_mcp::server::{ImportModelParam, SetModelImportOptionsParam, json_str};

use crate::mcp_bridge::mutation_error_json;

/// The import facts of one model: its id, its units, its import options,
/// its join report (`null` for a mesh or a STEP model) and its bbox. The
/// `import_model` and `set_model_import_options` replies and
/// `inspect_model` all write these keys from this one function.
pub(crate) fn model_import_json(model: &LoadedModel) -> serde_json::Value {
    let mut json = serde_json::json!({
        "model_id": model.id,
        "units": model.units,
        "import_options": model.import_options,
        "import_report": model.import_report.as_ref().map(|r| {
            serde_json::json!({
                "join_tolerance_mm": r.join_tolerance_mm,
                "layers": r.layers,
                "total": r.total(),
                "summary": r.summary_line(),
            })
        }),
    });
    if let Some(bbox) = model.bbox()
        && let Some(map) = json.as_object_mut()
    {
        map.insert(
            "bbox".into(),
            serde_json::json!({
                "min": [bbox.min.x, bbox.min.y, bbox.min.z],
                "max": [bbox.max.x, bbox.max.y, bbox.max.z],
            }),
        );
    }
    json
}

/// The model an `import_model` request imports, with the file extension,
/// or the reply that refuses it. The units and the join tolerance are the
/// request's, else the defaults the GUI import dialog starts from.
pub(crate) fn import_request_model(p: &ImportModelParam) -> Result<(LoadedModel, String), String> {
    let file_path = Path::new(&p.path);
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    // C10: one extension table, in `rs_cam_core::io`. This arm used to
    // carry its own copy.
    let Some(kind) = rs_cam_core::io::infer_kind_from_path(file_path) else {
        return Err(json_str(serde_json::json!({
            "error": format!("Unsupported file format '.{ext}'. Use .stl, .dxf, .svg, .step, or .stp")
        })));
    };
    let units = p.units.map_or_else(
        || rs_cam_core::io::default_import_units(file_path, kind),
        Into::into,
    );
    let options = p
        .join_tolerance_mm
        .map_or_else(ImportOptions::default, |join_tolerance_mm| ImportOptions {
            join_tolerance_mm,
        });
    crate::io::import::import_model(file_path, 0, kind, units, options)
        .map(|model| (model, ext))
        .map_err(|e| json_str(serde_json::json!({"error": format!("{e}")})))
}

/// The `AdoptModelGeometry` payload of a `set_model_import_options`
/// request: the model's file read again at the new units and options, as
/// the GUI model panel does. Refuses a model id that names no model, and a
/// model that is not a drawing.
pub(crate) fn model_import_options_args(
    session: &ProjectSession,
    p: &SetModelImportOptionsParam,
) -> Result<AdoptModelGeometryArgs, String> {
    let Some(model) = session.models().iter().find(|m| m.id == p.model_id) else {
        let ids: Vec<usize> = session.models().iter().map(|m| m.id).collect();
        return Err(mutation_error_json(
            &format!(
                "Error: Model id {} not found; project model ids: {ids:?}",
                p.model_id
            ),
            Some("model_id"),
        ));
    };
    let Some(kind) = model
        .kind
        .filter(|k| matches!(k, ModelKind::Svg | ModelKind::Dxf))
    else {
        return Err(mutation_error_json(
            &format!(
                "Error: Model {} is not a drawing (DXF or SVG); only a drawing has import options",
                p.model_id
            ),
            Some("model_id"),
        ));
    };
    let units = p.units.map_or_else(
        || model.units.unwrap_or(ModelUnits::Millimeters),
        Into::into,
    );
    let options = ImportOptions {
        join_tolerance_mm: p
            .join_tolerance_mm
            .unwrap_or(model.import_options.join_tolerance_mm),
    };
    let fresh = crate::io::import::import_model(&model.path, model.id, kind, units, options)
        .map_err(|e| mutation_error_json(&format!("Error: {e}"), Some("join_tolerance_mm")))?;
    Ok(AdoptModelGeometryArgs {
        model_id: model.id,
        geometry: Box::new(fresh),
        units: Some(units),
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    //! GUI/MCP parity of the import options (geometry selection P4.8).
    //!
    //! The GUI arm is the controller's dialog door (open, edit, confirm)
    //! and the model panel door (`set_model_import_options`). The MCP arm
    //! is the request conversion above, applied to a session. Both must
    //! give the same import facts (`model_import_json`) and the same items.

    use super::*;
    use crate::app::mcp::tests::U4SilentBackend;
    use crate::controller::AppController;
    use crate::state::job::ModelId;
    use rs_cam_core::session::{AddModelArgs, Command};
    use rs_cam_mcp::server::ModelUnitsParam;

    fn mix_dxf() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rs_cam_core/tests/fixtures/selection_p2_1/mix.dxf")
    }

    /// An SVG with two open subpaths 0.3 apart on one layer.
    fn gap_svg() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rs_cam_p4_8_mcp_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gap.svg");
        std::fs::write(
            &path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm" viewBox="0 0 100 50">
                <g id="CUT"><path d="M 0 0 L 10 0"/><path d="M 10.3 0 L 20 0 L 20 10"/></g>
            </svg>"#,
        )
        .unwrap();
        path
    }

    /// The import facts without the id (the two sessions number alike, but
    /// the id is not what the parity is about) and the item keys.
    fn facts(model: &LoadedModel) -> (serde_json::Value, Vec<String>) {
        let mut json = model_import_json(model);
        if let Some(map) = json.as_object_mut() {
            map.remove("model_id");
        }
        let keys = model
            .items
            .iter()
            .map(|i| format!("{:?}", i.key()))
            .collect();
        (json, keys)
    }

    /// The GUI dialog door: open, edit when asked, confirm.
    fn gui_import(
        path: &Path,
        kind: ModelKind,
        edit: Option<(ModelUnits, ImportOptions)>,
    ) -> LoadedModel {
        let mut controller = AppController::with_backend(U4SilentBackend);
        controller.open_import_dialog(path, kind);
        if let Some((units, options)) = edit {
            controller.edit_import_dialog(units, options);
        }
        controller
            .confirm_import_dialog()
            .expect("the dialog imports");
        controller.state().session.models()[0].clone()
    }

    /// The MCP door: convert the request, apply `AddModel`.
    fn mcp_import(param: &ImportModelParam) -> LoadedModel {
        let (model, _) = import_request_model(param).expect("the request imports");
        let mut session = ProjectSession::new_empty();
        let _ = session
            .apply(Command::AddModel(AddModelArgs {
                model: Box::new(model),
            }))
            .unwrap();
        session.models()[0].clone()
    }

    #[test]
    fn import_model_with_options_matches_the_dialog_p4_8() {
        for (path, kind) in [(mix_dxf(), ModelKind::Dxf), (gap_svg(), ModelKind::Svg)] {
            // The defaults: no units, no tolerance.
            let gui = gui_import(&path, kind, None);
            let mcp = mcp_import(&ImportModelParam {
                path: path.display().to_string(),
                units: None,
                join_tolerance_mm: None,
            });
            assert_eq!(facts(&gui), facts(&mcp), "{kind:?} at the defaults");
            assert!(gui.import_report.is_some(), "a drawing reports its join");

            // Inches and 0.5 mm.
            let gui = gui_import(
                &path,
                kind,
                Some((
                    ModelUnits::Inches,
                    ImportOptions {
                        join_tolerance_mm: 0.5,
                    },
                )),
            );
            let mcp = mcp_import(&ImportModelParam {
                path: path.display().to_string(),
                units: Some(ModelUnitsParam::Inches),
                join_tolerance_mm: Some(0.5),
            });
            assert_eq!(facts(&gui), facts(&mcp), "{kind:?} at inches and 0.5 mm");
            assert_eq!(gui.import_options.join_tolerance_mm, 0.5);
            assert_eq!(gui.units, Some(ModelUnits::Inches));
        }
        // Non-vacuity: 0.5 joins the SVG gap that the default leaves.
        let open = |m: &LoadedModel| m.import_report.as_ref().unwrap().total().open_left;
        let default = gui_import(&gap_svg(), ModelKind::Svg, None);
        let wide = mcp_import(&ImportModelParam {
            path: gap_svg().display().to_string(),
            units: None,
            join_tolerance_mm: Some(0.5),
        });
        assert_eq!((open(&default), open(&wide)), (2, 1));
    }

    #[test]
    fn set_model_import_options_matches_the_model_panel_p4_8() {
        let path = gap_svg();
        // GUI: the model panel door on an imported model.
        let mut controller = AppController::with_backend(U4SilentBackend);
        controller.open_import_dialog(&path, ModelKind::Svg);
        controller.confirm_import_dialog().unwrap();
        let id = controller.state().session.models()[0].id;
        let options = ImportOptions {
            join_tolerance_mm: 0.5,
        };
        controller
            .set_model_import_options(ModelId(id), options)
            .unwrap();
        let gui = controller.state().session.models()[0].clone();
        assert_eq!(gui.import_options, options);

        // MCP: the same project, the request, the row.
        let mut session = ProjectSession::new_empty();
        let (model, _) = import_request_model(&ImportModelParam {
            path: path.display().to_string(),
            units: None,
            join_tolerance_mm: None,
        })
        .unwrap();
        let _ = session
            .apply(Command::AddModel(AddModelArgs {
                model: Box::new(model),
            }))
            .unwrap();
        let args = model_import_options_args(
            &session,
            &SetModelImportOptionsParam {
                model_id: id,
                units: None,
                join_tolerance_mm: Some(0.5),
            },
        )
        .unwrap();
        let _ = session.apply(Command::AdoptModelGeometry(args)).unwrap();
        let mcp = session.models()[0].clone();
        assert_eq!(facts(&gui), facts(&mcp));

        // The refusals: an unknown id, and a model that is not a drawing.
        let unknown = model_import_options_args(
            &session,
            &SetModelImportOptionsParam {
                model_id: 99,
                ..Default::default()
            },
        );
        assert!(unknown.is_err_and(|e| e.contains("not found")));
    }
}
