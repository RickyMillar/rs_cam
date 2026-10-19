//! Geometry selection P2.6 — a selection survives a save and a load, and a
//! default selection writes no key.
//!
//! P2.6 moved the face picks into the op selection (`MatchKey` picks of
//! kind `Face`) and deleted `ToolpathConfig.face_selection`. The project
//! file then holds every selection input in two places: the op selection
//! (`operation.params.selection`) and the boundary selection
//! (`boundary.source.selection`). DESIGN 10.1.
//!
//! The test saves a project with two Pocket ops and loads it back:
//!
//! 1. Op 0 holds a non-default selection: two rules (a layer rule and a
//!    rule that names its kinds, with a diameter and a face filter), a
//!    circle pick, a face pick and an exclude, and a boundary selection
//!    of one face pick of another model. Each part loads equal.
//! 2. Op 1 holds the defaults. Its file table has no `selection` key and
//!    its boundary table no selection. No toolpath table has the retired
//!    `face_selection` key.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test selection_round_trips_p2_6
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::toolpath_config;

use std::path::PathBuf;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{
    DiameterRange, FaceFilter, Facing, Filter, ItemKind, KindFilter, MatchKey, Rule, Selection,
    SelectionEdit, SourceRef, ZRange,
};
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder};

fn temp_path() -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "rs_cam_selection_round_trip_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.push("project.toml");
    dir
}

fn key(
    source: SourceRef,
    kind: ItemKind,
    layer: Option<&str>,
    x: f64,
    brep: Option<u16>,
) -> MatchKey {
    MatchKey {
        source,
        kind,
        layer: layer.map(str::to_owned),
        anchor: [x, 12.5],
        measure: 6.0,
        brep,
    }
}

/// Two rules, a circle pick, a face pick and an exclude.
fn op_selection() -> Selection {
    let mut selection = Selection::from_rules(vec![
        Rule {
            source: SourceRef::ToolpathModel,
            filter: Filter {
                layers: vec!["CUT".to_owned()],
                ..Filter::default()
            },
        },
        Rule {
            source: SourceRef::Model(ModelId(7)),
            filter: Filter {
                layers: Vec::new(),
                kinds: KindFilter::Only(vec![ItemKind::Circle, ItemKind::Face]),
                diameter: Some(DiameterRange {
                    nominal_mm: 6.0,
                    tolerance_mm: 0.1,
                }),
                face: Some(FaceFilter {
                    facing: Facing::Up,
                    z: Some(ZRange {
                        min: -3.0,
                        max: 0.0,
                    }),
                }),
            },
        },
    ]);
    let default = Selection::drawing();
    let edits = [
        SelectionEdit::AddPick(key(
            SourceRef::ToolpathModel,
            ItemKind::Circle,
            Some("PINS"),
            40.0,
            None,
        )),
        // The face pick of P2.6: a key with a BREP id and no Z.
        SelectionEdit::AddPick(key(
            SourceRef::ToolpathModel,
            ItemKind::Face,
            None,
            20.0,
            Some(4),
        )),
        // No pick names this loop, so the edit stores an exclude.
        SelectionEdit::RemovePick(key(
            SourceRef::ToolpathModel,
            ItemKind::Loop,
            Some("CUT"),
            60.0,
            None,
        )),
    ];
    for edit in edits {
        selection.apply(edit, &default).unwrap();
    }
    assert_eq!(selection.rules().len(), 2);
    assert_eq!(selection.picks().len(), 2);
    assert_eq!(selection.excludes().len(), 1, "fixture: one exclude");
    assert_eq!(selection.face_picks().count(), 1, "fixture: one face pick");
    selection
}

/// A boundary of one face of model 7, holes kept.
fn boundary() -> BoundaryConfig {
    BoundaryConfig {
        enabled: true,
        source: BoundarySource::Selection {
            selection: Selection::from_picks(vec![key(
                SourceRef::Model(ModelId(7)),
                ItemKind::Face,
                None,
                30.0,
                Some(2),
            )]),
            holes: true,
        },
        containment: BoundaryContainment::Inside,
        offset: 1.5,
    }
}

fn saved(session: &ProjectSession) -> (String, ProjectSession) {
    let path = temp_path();
    session.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let loaded = ProjectSession::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir(dir);
    }
    (text, loaded)
}

#[test]
fn selection_round_trips_p2_6() {
    let mut builder = ProjectSessionBuilder::new();
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.0;
    let mut picked = toolpath_config(
        "picked",
        OperationConfig::Pocket(PocketConfig {
            selection: op_selection(),
            ..PocketConfig::default()
        }),
        tool_id,
        0,
    );
    picked.boundary = boundary();
    picked.boundary_inherit = false;
    let plain = toolpath_config(
        "plain",
        OperationConfig::Pocket(PocketConfig::default()),
        tool_id,
        0,
    );
    let _ = builder.add_toolpath(0, picked).unwrap();
    let _ = builder.add_toolpath(0, plain).unwrap();
    let session = builder.build();

    let (text, loaded) = saved(&session);

    // 1. Each part of the non-default selection loads equal.
    let before = &session.toolpath_configs()[0];
    let after = &loaded.toolpath_configs()[0];
    assert_eq!(
        after.operation.selection(),
        before.operation.selection(),
        "the op selection changed across a save and a load"
    );
    assert_eq!(
        after.operation.selection().map(|s| s.face_picks().count()),
        Some(1),
        "the face pick did not survive"
    );
    assert_eq!(
        after.boundary, before.boundary,
        "the boundary selection changed across a save and a load"
    );

    // 2. The defaults write no key, and no toolpath writes the retired key.
    let raw: toml::Value = toml::from_str(&text).unwrap();
    let toolpaths = raw["setups"][0]["toolpaths"].as_array().unwrap();
    assert_eq!(toolpaths.len(), 2);
    let picked_params = toolpaths[0]["operation"]["params"].as_table().unwrap();
    assert!(
        picked_params.contains_key("selection"),
        "control: the non-default selection must be in the file:\n{text}"
    );
    let plain_params = toolpaths[1]["operation"]["params"].as_table().unwrap();
    assert!(
        !plain_params.contains_key("selection"),
        "the default selection wrote a key:\n{text}"
    );
    let plain_boundary = toolpaths[1]["boundary"].to_string();
    assert!(
        !plain_boundary.contains("selection"),
        "the default boundary wrote a selection: {plain_boundary}"
    );
    for tp in toolpaths {
        assert!(
            !tp.as_table().unwrap().contains_key("face_selection"),
            "the writer emitted the retired key `face_selection`:\n{text}"
        );
    }
    assert_eq!(
        loaded.toolpath_configs()[1].operation.selection(),
        Some(&Selection::drawing()),
        "the default selection did not load as the op default"
    );
}
