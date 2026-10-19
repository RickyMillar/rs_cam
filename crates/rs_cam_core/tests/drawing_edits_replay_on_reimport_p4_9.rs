//! P4.9 — the join and close tool: a per-model edit ledger (geometry
//! selection, `planning/geometry_selection_2026-10-16/P4_DESIGN.md`,
//! "P4.9"; lead ruling L-11).
//!
//! The claims:
//!
//! 1. Each edit works on a drawing: a join within a tolerance, a close with
//!    a line, a move of the ends together, a join of two paths.
//! 2. The ledger goes into the project file with the model, and a save and
//!    a load give the same edits and the same item keys. A file with no key
//!    loads an empty ledger, and an empty ledger writes no key.
//! 3. A re-import (the `AdoptModelGeometry` row: reload, rescale, relink,
//!    import options) replays the ledger, so a join and a close stay.
//! 4. A pick on a joined path survives a reload: the joined item has a
//!    stable key.
//! 5. An edit that cannot replay (its path is gone from the changed file)
//!    stays in the ledger and is reported by name, never dropped.
//! 6. A join across layers refuses (D14).
//! 7. Every edit stales the toolpaths that read the model.
//! 8. The ledger is in file units (lead ruling on P4.9): a rescale changes
//!    no stored edit, the edits replay at the new scale, and after a
//!    rescale back the pick on the joined path resolves again.
//!
//! The fixtures are DXF files that the test writes ([`write_dxf`]).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test drawing_edits_replay_on_reimport_p4_9
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::path::{Path, PathBuf};

use common::session::{single_op_session, stock_under};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::TraceConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::io::{ImportOptions, load_model_file};
use rs_cam_core::selection::{
    DrawingEdit, EditLost, ItemKind, MatchKey, Selection, SelectionEdit, open_shapes,
};
use rs_cam_core::session::{
    AdoptModelGeometryArgs, Command, EditModelDrawingArgs, EditSelectionArgs, LedgerEdit,
    LoadedModel, ProjectSession, SelectionTarget,
};

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("rs_cam_ledger_p4_9_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// One LINE: layer, start, end.
type Line = (&'static str, (f64, f64), (f64, f64));

/// The drawing of the claims, as LINE entities. The door chains the lines
/// that meet exactly (0.01 mm), so the items are:
///
/// 0. TEXT: a path (0,0)-(10,0).
/// 1. TEXT: a path (10.3,0)-(20,0): 0.3 mm from item 0.
/// 2. TEXT: a U of three lines, (30,0)-(40,0)-(40,10)-(30,10): its ends
///    are 10 mm apart. Without `with_u` the U is not in the file.
/// 3. 0: a path (20,0.2)-(20,5): 0.2 mm from item 1, on another layer.
fn lines(with_u: bool) -> Vec<Line> {
    let mut lines: Vec<Line> = vec![
        ("TEXT", (0.0, 0.0), (10.0, 0.0)),
        ("TEXT", (10.3, 0.0), (20.0, 0.0)),
    ];
    if with_u {
        lines.extend([
            ("TEXT", (30.0, 0.0), (40.0, 0.0)),
            ("TEXT", (40.0, 0.0), (40.0, 10.0)),
            ("TEXT", (40.0, 10.0), (30.0, 10.0)),
        ]);
    }
    lines.push(("0", (20.0, 0.2), (20.0, 5.0)));
    lines
}

/// Write the drawing to `dir/drawing.dxf`.
fn write_dxf(dir: &Path, with_u: bool) -> PathBuf {
    let mut drawing = dxf::Drawing::new();
    for (layer, (x1, y1), (x2, y2)) in lines(with_u) {
        let mut entity =
            dxf::entities::Entity::new(dxf::entities::EntityType::Line(dxf::entities::Line {
                p1: dxf::Point::new(x1, y1, 0.0),
                p2: dxf::Point::new(x2, y2, 0.0),
                ..Default::default()
            }));
        entity.common.layer = layer.to_owned();
        drawing.add_entity(entity);
    }
    let path = dir.join("drawing.dxf");
    drawing
        .save_file(path.to_str().unwrap())
        .expect("write the DXF");
    path
}

fn import(path: &Path) -> LoadedModel {
    load_model_file(
        path,
        0,
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        ImportOptions::default(),
    )
    .unwrap()
}

/// The key of the open path that starts at `(x, y)`.
fn path_at(model: &LoadedModel, x: f64, y: f64) -> MatchKey {
    open_shapes(&model.items)
        .into_iter()
        .find(|s| (s.key.anchor[0] - x).abs() < 1e-9 && (s.key.anchor[1] - y).abs() < 1e-9)
        .unwrap_or_else(|| panic!("no open path starts at ({x}, {y})"))
        .key
}

fn keys(model: &LoadedModel) -> Vec<String> {
    model
        .items
        .iter()
        .map(|i| format!("{:?}", i.key()))
        .collect()
}

fn kinds(model: &LoadedModel) -> Vec<ItemKind> {
    model.items.iter().map(|i| i.kind()).collect()
}

/// A session with the drawing and one Trace op on it.
fn session(path: &Path) -> ProjectSession {
    single_op_session(
        stock_under(80.0, 10.0),
        endmill_tool_config(3.0),
        import(path),
        "trace",
        OperationConfig::Trace(TraceConfig {
            selection: Selection::empty(),
            ..TraceConfig::default()
        }),
    )
}

fn model_id(s: &ProjectSession) -> usize {
    s.models()[0].id
}

fn edit(s: &mut ProjectSession, edit: LedgerEdit) -> Result<Vec<usize>, String> {
    let model_id = model_id(s);
    s.apply(Command::EditModelDrawing(EditModelDrawingArgs {
        model_id,
        edit,
    }))
    .map(|effects| effects.stale.into_iter().collect())
    .map_err(|e| e.to_string())
}

/// The two edits of the claims: join TEXT at 0.5 mm, close the U.
fn join_and_close(s: &mut ProjectSession) {
    let _ = edit(
        s,
        LedgerEdit::Add(DrawingEdit::JoinWithin {
            layer: Some("TEXT".into()),
            tolerance_mm: 0.5,
        }),
    )
    .expect("the join applies");
    let u = path_at(&s.models()[0], 30.0, 0.0);
    let _ = edit(s, LedgerEdit::Add(DrawingEdit::CloseWithLine { path: u }))
        .expect("the close applies");
}

/// The rescale door: read the file again at `units` and adopt it with
/// the new units (the GUI model panel scale row).
fn rescale(s: &mut ProjectSession, path: &Path, units: ModelUnits) {
    let model_id = model_id(s);
    let geometry =
        load_model_file(path, 0, ModelKind::Dxf, units, ImportOptions::default()).unwrap();
    let _ = s
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id,
            geometry: Box::new(geometry),
            units: Some(units),
        }))
        .expect("the rescale adopts");
}

/// The reload door: read the file again and adopt it (the GUI reload, the
/// MCP `set_model_import_options`).
fn reload(s: &mut ProjectSession, path: &Path) {
    let model_id = model_id(s);
    let _ = s
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id,
            geometry: Box::new(import(path)),
            units: None,
        }))
        .expect("the reload adopts");
}

// ── 1. Each edit ────────────────────────────────────────────────────────

#[test]
fn each_edit_changes_the_drawing() {
    let dir = scratch("each");
    let path = write_dxf(&dir, true);
    let fresh = import(&path);
    assert_eq!(
        kinds(&fresh),
        vec![ItemKind::Path; 4],
        "the door leaves four open paths"
    );

    // Join within 0.5 mm on TEXT: items 0 and 1 become one path at index 0.
    let mut s = session(&path);
    join_and_close(&mut s);
    let model = &s.models()[0];
    assert_eq!(
        kinds(model),
        vec![ItemKind::Path, ItemKind::Loop, ItemKind::Path]
    );
    // The kernel does not repeat the joined end: the chain runs
    // (0,0)-(10,0)-(20,0), 20 mm.
    let joined = path_at(model, 0.0, 0.0);
    assert!((joined.measure - 20.0).abs() < 1e-9, "{joined:?}");
    assert_eq!(joined.layer.as_deref(), Some("TEXT"));
    assert!(model.drawing.outcomes().iter().all(|o| o.result.is_ok()));

    // Move the ends of the U together: a loop of 3 points.
    let mut s = session(&path);
    let u = path_at(&s.models()[0], 30.0, 0.0);
    let _ = edit(
        &mut s,
        LedgerEdit::Add(DrawingEdit::MoveEndsTogether { path: u }),
    )
    .unwrap();
    assert_eq!(
        s.models()[0].items.iter().nth(2).unwrap().kind(),
        ItemKind::Loop
    );

    // Join two paths with a line: 0 and 1, 0.3 mm apart. The line keeps
    // both ends: 10 + 0.3 + 9.7 mm.
    let mut s = session(&path);
    let a = path_at(&s.models()[0], 0.0, 0.0);
    let b = path_at(&s.models()[0], 10.3, 0.0);
    let _ = edit(&mut s, LedgerEdit::Add(DrawingEdit::JoinTwo { a, b })).unwrap();
    let gap = s.models()[0].drawing.outcomes()[0]
        .result
        .as_ref()
        .unwrap()
        .gap_mm;
    assert!((gap - 0.3).abs() < 1e-9, "the line is 0.3 mm: {gap}");
    let joined = path_at(&s.models()[0], 0.0, 0.0);
    assert!((joined.measure - 20.0).abs() < 1e-9, "{joined:?}");
    assert_eq!(s.models()[0].items.len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

// ── 2. The project file ─────────────────────────────────────────────────

#[test]
fn the_ledger_round_trips_through_the_project_file() {
    let dir = scratch("round_trip");
    let path = write_dxf(&dir, true);
    let mut s = session(&path);
    let before_keys = keys(&s.models()[0]);
    join_and_close(&mut s);
    let edited_keys = keys(&s.models()[0]);
    assert_ne!(
        before_keys, edited_keys,
        "non-vacuity: the edits change the items"
    );

    let job = dir.join("job.toml");
    s.save(&job).expect("save");
    let written = std::fs::read_to_string(&job).unwrap();
    assert!(
        written.contains("drawing_edits") && written.contains("close_with_line"),
        "the save writes the ledger:\n{written}"
    );
    let loaded = ProjectSession::load(&job).expect("load");
    let model = &loaded.models()[0];
    assert_eq!(model.drawing.edits(), s.models()[0].drawing.edits());
    assert_eq!(keys(model), edited_keys, "a load gives the same item keys");
    assert!(model.drawing.outcomes().iter().all(|o| o.result.is_ok()));

    // An empty ledger writes no key, and a file with no key loads empty.
    let plain = session(&path);
    let job = dir.join("plain.toml");
    plain.save(&job).expect("save");
    let written = std::fs::read_to_string(&job).unwrap();
    assert!(!written.contains("drawing_edits"), "{written}");
    let loaded = ProjectSession::load(&job).expect("load");
    assert!(loaded.models()[0].drawing.edits().is_empty());
    assert_eq!(keys(&loaded.models()[0]), before_keys);
    let _ = std::fs::remove_dir_all(&dir);
}

// ── 3 and 4. A re-import keeps the edits, and a pick on a joined path ───

#[test]
fn a_reimport_keeps_the_join_and_the_close_and_the_pick() {
    let dir = scratch("reimport");
    let path = write_dxf(&dir, true);
    let mut s = session(&path);
    join_and_close(&mut s);
    let edited = keys(&s.models()[0]);

    // Pick the joined path on the Trace op.
    let joined = path_at(&s.models()[0], 0.0, 0.0);
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(joined),
        }))
        .unwrap();
    let status = |s: &ProjectSession| {
        let tc = s.toolpath_configs()[0].clone();
        s.selection_status(&tc, SelectionTarget::Op).unwrap()
    };
    assert!(status(&s).lost.is_empty());
    assert_eq!(
        status(&s).counts.get(&ItemKind::Path),
        Some(&1),
        "the pick selects the joined path"
    );

    reload(&mut s, &path);
    let model = &s.models()[0];
    assert_eq!(
        keys(model),
        edited,
        "the reload replays the join and the close"
    );
    assert_eq!(model.drawing.edits().len(), 2);
    assert!(model.drawing.outcomes().iter().all(|o| o.result.is_ok()));
    assert!(
        status(&s).lost.is_empty(),
        "the pick on the joined path survives the reload: {:?}",
        status(&s).lost
    );
    assert_eq!(status(&s).counts.get(&ItemKind::Path), Some(&1));
    let _ = std::fs::remove_dir_all(&dir);
}

// ── 5. An edit that cannot replay is reported by name ───────────────────

#[test]
fn an_edit_that_cannot_replay_is_reported_by_name() {
    let dir = scratch("lost");
    let path = write_dxf(&dir, true);
    let mut s = session(&path);
    join_and_close(&mut s);
    let u = match &s.models()[0].drawing.edits()[1] {
        DrawingEdit::CloseWithLine { path } => path.clone(),
        other => panic!("the second edit is the close: {other:?}"),
    };

    // The file changes: the U is gone.
    let path = write_dxf(&dir, false);
    reload(&mut s, &path);
    let model = &s.models()[0];
    assert_eq!(model.drawing.edits().len(), 2, "the ledger keeps the edit");
    assert!(
        model.drawing.outcomes()[0].result.is_ok(),
        "the join replays"
    );
    let lost = model.drawing.outcomes()[1].result.as_ref().unwrap_err();
    assert_eq!(lost, &EditLost::NoItem { key: u });
    assert_eq!(model.drawing.lost_count(), 1);
    let label = lost.label();
    assert!(
        label.starts_with("Lost")
            && label.contains("layer TEXT")
            && label.contains("(30.00, 0.00)"),
        "the report names the path: {label}"
    );

    // The lost edit can be removed; the join stays.
    let _ = edit(&mut s, LedgerEdit::Remove(1)).unwrap();
    assert_eq!(s.models()[0].drawing.edits().len(), 1);
    assert_eq!(s.models()[0].drawing.lost_count(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

// ── 6. A join across layers refuses ─────────────────────────────────────

#[test]
fn a_join_across_layers_refuses() {
    let dir = scratch("layers");
    let path = write_dxf(&dir, true);
    let mut s = session(&path);
    let text = path_at(&s.models()[0], 10.3, 0.0);
    let zero = path_at(&s.models()[0], 20.0, 0.2);
    let before = keys(&s.models()[0]);
    let refused = edit(
        &mut s,
        LedgerEdit::Add(DrawingEdit::JoinTwo { a: text, b: zero }),
    );
    let error = refused.expect_err("a join across layers refuses");
    assert!(error.contains("cannot cross layers"), "{error}");
    assert!(
        s.models()[0].drawing.edits().is_empty(),
        "the refusal stores nothing"
    );
    assert_eq!(keys(&s.models()[0]), before);

    // A join within a tolerance never crosses layers: the 0.2 mm gap
    // between TEXT and 0 stays.
    let _ = edit(
        &mut s,
        LedgerEdit::Add(DrawingEdit::JoinWithin {
            layer: None,
            tolerance_mm: 0.5,
        }),
    )
    .unwrap();
    assert_eq!(
        s.models()[0].items.len(),
        3,
        "TEXT joins, layer 0 stays apart"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── 7. Every edit stales the readers ────────────────────────────────────

#[test]
fn every_edit_stales_the_toolpaths_that_read_the_model() {
    let dir = scratch("stale");
    let path = write_dxf(&dir, true);
    let mut s = session(&path);
    let stale = edit(
        &mut s,
        LedgerEdit::Add(DrawingEdit::JoinWithin {
            layer: None,
            tolerance_mm: 0.5,
        }),
    )
    .unwrap();
    assert_eq!(stale, vec![0], "the edit stales the Trace op");
    let stale = edit(&mut s, LedgerEdit::Replace(Vec::new())).unwrap();
    assert_eq!(stale, vec![0], "an undo of the ledger stales it too");
    assert!(s.models()[0].drawing.edits().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

// ── 8. A rescale keeps the edits ────────────────────────────────────────

#[test]
fn a_rescale_keeps_the_edits_and_the_pick() {
    let dir = scratch("rescale");
    let path = write_dxf(&dir, true);
    let mut s = session(&path);
    join_and_close(&mut s);
    let stored = s.models()[0].drawing.edits().to_vec();
    let edited_mm: Vec<(ItemKind, [f64; 2], f64)> = s.models()[0]
        .items
        .iter()
        .map(|i| (i.kind(), i.key().anchor, i.key().measure))
        .collect();
    let joined = path_at(&s.models()[0], 0.0, 0.0);
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(joined),
        }))
        .unwrap();
    let status = |s: &ProjectSession| {
        let tc = s.toolpath_configs()[0].clone();
        s.selection_status(&tc, SelectionTarget::Op).unwrap()
    };

    // mm -> inches: the stored edits do not change, every edit replays,
    // and each item is the mm item times 25.4.
    rescale(&mut s, &path, ModelUnits::Inches);
    let model = &s.models()[0];
    assert_eq!(
        model.drawing.edits(),
        stored.as_slice(),
        "a rescale changes no stored edit"
    );
    assert_eq!(
        model.drawing.lost_count(),
        0,
        "{:?}",
        model.drawing.outcomes()
    );
    let scale = ModelUnits::Inches.scale_factor();
    let inches: Vec<(ItemKind, [f64; 2], f64)> = model
        .items
        .iter()
        .map(|i| (i.kind(), i.key().anchor, i.key().measure))
        .collect();
    assert_eq!(inches.len(), edited_mm.len());
    for ((ka, aa, ma), (kb, ab, mb)) in inches.iter().zip(&edited_mm) {
        assert_eq!(ka, kb);
        assert!((aa[0] - ab[0] * scale).abs() < 1e-6 && (aa[1] - ab[1] * scale).abs() < 1e-6);
        assert!((ma - mb * scale).abs() < 1e-6, "{ma} vs {mb} x {scale}");
    }

    // Inches -> mm: the edits replay to the same items, and the pick on
    // the joined path resolves again.
    rescale(&mut s, &path, ModelUnits::Millimeters);
    let model = &s.models()[0];
    assert_eq!(model.drawing.edits(), stored.as_slice());
    assert_eq!(model.drawing.lost_count(), 0);
    let back: Vec<(ItemKind, [f64; 2], f64)> = model
        .items
        .iter()
        .map(|i| (i.kind(), i.key().anchor, i.key().measure))
        .collect();
    assert_eq!(back, edited_mm);
    assert!(status(&s).lost.is_empty(), "{:?}", status(&s).lost);
    assert_eq!(status(&s).counts.get(&ItemKind::Path), Some(&1));

    // A new edit at inches stores file units: the stored key equals the
    // key at mm.
    let mut fresh = session(&path);
    let _ = edit(
        &mut fresh,
        LedgerEdit::Add(DrawingEdit::JoinWithin {
            layer: Some("TEXT".into()),
            tolerance_mm: 0.5,
        }),
    )
    .unwrap();
    let u_mm = path_at(&fresh.models()[0], 30.0, 0.0);
    let _ = edit(
        &mut fresh,
        LedgerEdit::Add(DrawingEdit::CloseWithLine { path: u_mm }),
    )
    .unwrap();
    let mut at_inches = session(&path);
    rescale(&mut at_inches, &path, ModelUnits::Inches);
    let _ = edit(
        &mut at_inches,
        LedgerEdit::Add(DrawingEdit::JoinWithin {
            layer: Some("TEXT".into()),
            tolerance_mm: 0.5 * scale,
        }),
    )
    .unwrap();
    let u_in = path_at(&at_inches.models()[0], 30.0 * scale, 0.0);
    let _ = edit(
        &mut at_inches,
        LedgerEdit::Add(DrawingEdit::CloseWithLine { path: u_in }),
    )
    .unwrap();
    let close_key = |s: &ProjectSession| match &s.models()[0].drawing.edits()[1] {
        DrawingEdit::CloseWithLine { path } => path.clone(),
        other => panic!("the second edit is the close: {other:?}"),
    };
    let (a, b) = (close_key(&fresh), close_key(&at_inches));
    assert!(
        a.names_same_geometry(&b),
        "file units at both scales: {a:?} vs {b:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
