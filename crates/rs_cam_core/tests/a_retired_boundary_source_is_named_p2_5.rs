//! Geometry selection P2.5 — a retired boundary source is named, never
//! read.
//!
//! P2.5 deleted `BoundarySource::ModelOutline` and `FaceSelection`. A
//! boundary that reads a selection is `[boundary.source.selection]` now.
//! Ruling of 2026-09-16 (no legacy): an old source does not convert. The
//! loader refuses it and names it and what replaced it.
//!
//! The two old sources have two shapes in the file (serde, externally
//! tagged, `snake_case`):
//!
//! - `ModelOutline { model_id, holes }` is a variant with fields, so it is
//!   a KEY: `[boundary.source.model_outline]`. A `RETIRED_KEYS` row finds
//!   it.
//! - `FaceSelection` is a unit variant, so it is a VALUE:
//!   `source = "face_selection"`. A key row cannot find it, and serde would
//!   refuse it with "unknown variant" and name no fix. The `RETIRED_VALUES`
//!   table (decision D8) finds it.
//!
//! The test proves the two shapes on the live writer: a `Selection` source
//! (fields) saves as a `selection` table under `source`, and
//! `ModelSilhouette` (unit) saves as a string value. Then it writes the two
//! old sources into a saved project and loads the text through the one text
//! door (`ProjectFile::from_toml_str`):
//!
//! 1. the unchanged text loads, and the selection boundary reads back equal
//!    (control);
//! 2. the text with the old sources refuses with `SessionError::RetiredKeys`,
//!    which names both (C2: one refusal names every retired key and value)
//!    and the replacement text of each.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_retired_boundary_source_is_named_p2_5
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;
use common::make_endmill_6mm;
use common::session::{polygon_model, square_polygon, toolpath_config};

use std::path::PathBuf;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::Selection;
use rs_cam_core::session::{ProjectFile, ProjectSessionBuilder, SessionError};

fn temp_path() -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "rs_cam_retired_boundary_source_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.push("project.toml");
    dir
}

/// The selection boundary of toolpath 0.
fn selection_boundary(model_id: usize) -> BoundaryConfig {
    BoundaryConfig {
        enabled: true,
        source: BoundarySource::Selection {
            selection: Selection::closed_shapes_of(ModelId::from_raw(model_id)),
            holes: true,
        },
        containment: BoundaryContainment::Inside,
        offset: 0.0,
    }
}

/// The text of a saved project with two pockets: toolpath 0 on a selection
/// boundary, toolpath 1 on a model silhouette boundary. Also the model id.
fn saved_project_text() -> (String, usize) {
    let mut builder = ProjectSessionBuilder::new();
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model_id = builder.add_model(polygon_model(vec![square_polygon(10.0)], "square"));
    for (name, boundary) in [
        ("Selection", selection_boundary(model_id.raw())),
        (
            "Silhouette",
            BoundaryConfig {
                enabled: true,
                source: BoundarySource::ModelSilhouette,
                ..BoundaryConfig::default()
            },
        ),
    ] {
        let mut tc = toolpath_config(
            name,
            OperationConfig::new_default(OperationType::Pocket),
            rs_cam_core::ids::ToolId::from_raw(tool_id),
            model_id,
        );
        tc.boundary = boundary;
        let _ = builder.add_toolpath(0, tc).expect("add toolpath");
    }
    let session = builder.build();
    let path = temp_path();
    session.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir(dir);
    }
    (text, model_id.raw())
}

#[test]
fn a_retired_boundary_source_is_named_p2_5() {
    let (text, model_id) = saved_project_text();
    assert!(
        !text.contains("model_outline") && !text.contains("face_selection"),
        "the writer must not emit a retired source:\n{text}"
    );

    // The two shapes, on the live writer.
    let raw: toml::Value = toml::from_str(&text).unwrap();
    let toolpaths = raw["setups"][0]["toolpaths"].as_array().unwrap();
    assert!(
        toolpaths[0]["boundary"]["source"]
            .get("selection")
            .is_some_and(toml::Value::is_table),
        "a variant with fields saves as a table KEY under `source`:\n{text}"
    );
    assert_eq!(
        toolpaths[1]["boundary"]["source"].as_str(),
        Some("model_silhouette"),
        "a unit variant saves as a string VALUE of `source`"
    );

    // Control: the text loads, and the selection boundary reads back equal.
    let file = ProjectFile::from_toml_str(&text).expect("control: the saved project loads");
    assert_eq!(
        file.setups[0].toolpaths[0].boundary,
        selection_boundary(model_id),
        "the selection boundary must round-trip through the file"
    );

    // The two old sources, in their old shapes.
    let mut old: toml::Value = raw.clone();
    let toolpaths = old["setups"][0]["toolpaths"].as_array_mut().unwrap();
    let mut outline = toml::map::Map::new();
    outline.insert(
        "model_id".to_owned(),
        toml::Value::Integer(i64::try_from(model_id).unwrap()),
    );
    outline.insert("holes".to_owned(), toml::Value::Boolean(false));
    let mut source = toml::map::Map::new();
    source.insert("model_outline".to_owned(), toml::Value::Table(outline));
    toolpaths[0]["boundary"]
        .as_table_mut()
        .unwrap()
        .insert("source".to_owned(), toml::Value::Table(source));
    toolpaths[1]["boundary"].as_table_mut().unwrap().insert(
        "source".to_owned(),
        toml::Value::String("face_selection".to_owned()),
    );
    let old = toml::to_string(&old).unwrap();

    let Err(err) = ProjectFile::from_toml_str(&old) else {
        panic!("a project with a retired boundary source must refuse, not load");
    };
    let SessionError::RetiredKeys(found) = &err else {
        panic!("expected SessionError::RetiredKeys, got {err}");
    };
    let mut paths: Vec<&str> = found.iter().map(|k| k.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        vec![
            "setups[0].toolpaths[0].boundary.source.model_outline",
            "setups[0].toolpaths[1].boundary.source = \"face_selection\"",
        ],
        "one refusal must name both retired sources"
    );
    let text = err.to_string();
    assert!(
        text.contains("[boundary.source.selection]") && text.contains("Pick the faces again"),
        "the refusal must name what replaced each source: {text}"
    );
}
