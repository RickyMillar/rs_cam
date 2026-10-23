//! F19 (operator ruling 2026-10-23, the UX-1 reading) — the Heights panel
//! owns the Drill top. The value the panel shows is the value the Drill
//! cuts from, and the static check reads the same floor. One rule, no
//! second top in the Drill.
//!
//! Before F19, on the blind-hole block in stock with its top at Z 25, the
//! panel read Top Z 20 ("auto: faces"), the Drill measured its depth from
//! the stock top (Z 25), and the static check measured it from Z 20 and
//! said "Depth exceeds stock thickness by 5.00 mm".
//!
//! 1. A Drill on the rim with the floor 8 mm below the Top Z: the panel
//!    Top Z is 25 (the higher of the rim and the stock top), the depth
//!    counts from the rim (Z 20), so the floor is Z 12. The motion stops at
//!    Z 12, and the static check gives no caution.
//! 2. With the floor 30 mm below the Top Z, the panel floor, the motion and
//!    the static check all read Z -10 (10 mm under the stock bottom).
//! 3. Lead decision 2026-10-23: a PINNED Top Z on a DXF Drill now drills
//!    from that top (before F19 the Drill read the stock top).
//! 4. The saved Drill `depth` key and a pinned Drill `bottom_z` refuse at
//!    load, and the text gives the `stock_top` floor of the old cut.
//!
//! Red proof (run by hand, 2026-10-23): claim 1 goes red when
//! `drill_hole_columns` measures the depth from the stock box top again.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test the_drill_cuts_from_the_top_the_heights_show_f19
//! ```

#![cfg(feature = "step")]
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use common::session::{single_op_session, single_op_session_with, stock_under, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::HeightMode;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::DrillConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::diagnostics::adapters::from_static_checks::{
    ResolvedHeights as StaticHeights, depth_beyond_stock,
};
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{Item, ItemKind, ItemSet, Selection, SelectionEdit, Shape, SourceRef};
use rs_cam_core::session::{
    Command, EditSelectionArgs, LoadedModel, ProjectFile, ProjectSession, ProjectSessionBuilder,
    SelectionTarget, SessionError,
};
use rs_cam_core::toolpath::MoveIntent;

const HOLE: &str = "tests/fixtures/step/block_with_blind_hole.step";
const RIM_Z: f64 = 20.0;
const STOCK_TOP: f64 = 25.0;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

fn load() -> LoadedModel {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(HOLE);
    rs_cam_core::io::load_model_file(
        &path,
        ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap()
}

/// A Drill with `floor` that picks the rim of the hole, in stock with its
/// top at Z 25.
fn rim_drill(floor: Floor) -> ProjectSession {
    let model = load();
    let rim = model
        .items
        .iter()
        .find(|i| i.kind() == ItemKind::Circle && i.z().is_some_and(|z| near(z, RIM_Z)))
        .unwrap()
        .clone();
    let stock = StockConfig {
        x: 50.0,
        y: 40.0,
        z: Some(STOCK_TOP),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut s = single_op_session(
        stock,
        endmill_tool_config(6.0),
        model,
        "f19 drill",
        OperationConfig::Drill(DrillConfig {
            floor,
            selection: Selection::empty(),
            ..DrillConfig::default()
        }),
    );
    let model_id = s.models()[0].id;
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(rim.key().clone().for_toolpath_model(model_id)),
        }))
        .unwrap();
    s
}

/// The lowest Z of the drilling moves of toolpath 0.
fn lowest_drill_z(s: &mut ProjectSession) -> f64 {
    let result = s.generate_toolpath(0, &AtomicBool::new(false)).unwrap();
    result
        .op_data
        .annotated()
        .toolpath
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Drilling)
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min)
}

/// The panel floor, the static check floor and its finding.
fn panel_and_check(s: &ProjectSession) -> (f64, f64, f64, Option<f64>) {
    let tc = s.toolpath_configs()[0].clone();
    let answer = s.toolpath_heights(&tc).unwrap();
    let floor_z = answer.floor_z(&tc.heights).unwrap().unwrap();
    let snapshot = StaticHeights::from_toolpath_heights(&answer, &tc.heights);
    let check = depth_beyond_stock(&tc.operation, &snapshot);
    (
        answer.heights.top_z,
        floor_z,
        snapshot.op_floor_z.unwrap(),
        check.map(|c| c.excess_mm),
    )
}

/// Claim 1.
#[test]
fn the_drill_cuts_from_the_top_the_heights_show_f19() {
    let mut s = rim_drill(Floor::Top { depth: 8.0 });
    let (top, floor, check_floor, excess) = panel_and_check(&s);
    assert!(near(top, STOCK_TOP), "the panel Top Z: {top}");
    assert!(near(floor, 12.0), "the panel floor: {floor}");
    assert!(
        near(check_floor, 12.0),
        "the static check floor: {check_floor}"
    );
    assert_eq!(excess, None, "no false depth caution");
    let lowest = lowest_drill_z(&mut s);
    assert!(near(lowest, 12.0), "the motion floor: {lowest}");
}

/// Claim 2.
#[test]
fn a_deep_drill_reads_one_floor_on_every_surface_f19() {
    let mut s = rim_drill(Floor::Top { depth: 30.0 });
    let (_, floor, check_floor, excess) = panel_and_check(&s);
    assert!(near(floor, -10.0), "the panel floor: {floor}");
    assert!(near(check_floor, -10.0), "the static check: {check_floor}");
    assert!(
        excess.is_some_and(|e| near(e, 10.0)),
        "10 mm under the stock bottom: {excess:?}"
    );
    let lowest = lowest_drill_z(&mut s);
    assert!(near(lowest, -10.0), "the motion floor: {lowest}");
}

/// Claim 3 (lead decision 2026-10-23).
#[test]
fn a_pinned_top_on_a_dxf_drill_drills_from_it_f19() {
    let circle = Item::new(
        SourceRef::ToolpathModel,
        Some("HOLES".into()),
        Shape::circle(P2::new(5.0, 5.0), 6.0),
    )
    .unwrap();
    let mut model = common::session::polygon_model(Vec::new(), "f19_dxf");
    model.items = std::sync::Arc::new(ItemSet::new(vec![circle]));
    let mut s = single_op_session_with(
        stock_under(40.0, 18.0),
        endmill_tool_config(6.0),
        model,
        "f19 dxf drill",
        OperationConfig::Drill(DrillConfig {
            floor: Floor::Top { depth: 5.0 },
            ..DrillConfig::default()
        }),
        |tc| tc.heights.top_z = HeightMode::Manual(-2.0),
    );
    let lowest = lowest_drill_z(&mut s);
    assert!(
        near(lowest, -7.0),
        "a pinned Top Z of -2 less a depth of 5: {lowest}"
    );
    // Control: with an auto top the depth counts from the stock top (Z 0),
    // as before F19.
    let circle = Item::new(
        SourceRef::ToolpathModel,
        Some("HOLES".into()),
        Shape::circle(P2::new(5.0, 5.0), 6.0),
    )
    .unwrap();
    let mut model = common::session::polygon_model(Vec::new(), "f19_dxf");
    model.items = std::sync::Arc::new(ItemSet::new(vec![circle]));
    let mut auto = single_op_session_with(
        stock_under(40.0, 18.0),
        endmill_tool_config(6.0),
        model,
        "f19 dxf drill",
        OperationConfig::Drill(DrillConfig {
            floor: Floor::Top { depth: 5.0 },
            ..DrillConfig::default()
        }),
        |_| {},
    );
    let lowest = lowest_drill_z(&mut auto);
    assert!(near(lowest, -5.0), "the stock top less 5: {lowest}");
}

/// Claim 4.
#[test]
fn a_retired_drill_depth_is_named_f19() {
    let mut builder = ProjectSessionBuilder::new();
    let tool = builder.add_tool(endmill_tool_config(6.0));
    let tool_id = builder.tools()[tool].id;
    let _ = builder
        .add_toolpath(
            0,
            toolpath_config(
                "Drill",
                OperationConfig::new_default(OperationType::Drill),
                tool_id,
                ModelId::from_raw(0),
            ),
        )
        .unwrap();
    let session = builder.build();
    let mut path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    path.push(format!("rs_cam_f19_retired_{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path.push("project.toml");
    session.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    ProjectFile::from_toml_str(&text).expect("control: the saved Drill loads");

    let mut raw: toml::Value = toml::from_str(&text).unwrap();
    let tp = &mut raw["setups"][0]["toolpaths"][0];
    let params = tp["operation"]["params"].as_table_mut().unwrap();
    assert!(params.contains_key("floor"), "the writer states the floor");
    let _ = params.remove("floor");
    params.insert("depth".to_owned(), toml::Value::Float(12.0));
    let bottom: toml::Value = toml::from_str("mode = \"manual\"\nvalue = -20.0").unwrap();
    tp["heights"]
        .as_table_mut()
        .unwrap()
        .insert("bottom_z".to_owned(), bottom);
    let old = toml::to_string(&raw).unwrap();
    let Err(err) = ProjectFile::from_toml_str(&old) else {
        panic!("a Drill with `depth` and a pinned `bottom_z` must refuse");
    };
    let SessionError::RetiredKeys(found) = &err else {
        panic!("expected RetiredKeys, got {err}");
    };
    let mut paths: Vec<&str> = found.iter().map(|k| k.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        vec![
            "setups[0].toolpaths[0].heights.bottom_z",
            "setups[0].toolpaths[0].operation.params.depth",
        ],
        "{err}"
    );
    let message = err.to_string();
    assert!(
        message.contains("floor = { from = \"stock_top\", depth = <this value> }")
            && message.contains("from = \"each_hole\""),
        "the text gives the floor of the old cut: {message}"
    );
    assert!(
        !message.contains("from = \"top\""),
        "the Pocket text (from the Top Z) is wrong for a Drill: {message}"
    );
}
