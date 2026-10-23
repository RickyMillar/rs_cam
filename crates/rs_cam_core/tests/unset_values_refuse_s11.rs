//! S11 (operator ruling 2026-10-23): a value with no derivation is not set,
//! and the op refuses to generate until the operator sets it.
//!
//! "If there is no logical reason for a default, then dont have it."
//! `planning/geometry_selection_2026-10-16/PLAN.md` (the S11 ruling) and
//! `UNDERIVED_DEFAULTS.md` (the inventory).
//!
//! # The claims
//!
//! 1. (a) A drawing (DXF) has no thickness: after the import the stock
//!    thickness is not set, and generation refuses with the stock text. A
//!    thickness set through the patch keeps the stock top at the drawing.
//!    A new Pocket (THE draft door, `ProjectSession::draft_toolpath`) has
//!    no floor: `Floor::Unset`, `unset_fields` names `floor`, and
//!    generation refuses with the floor text, which names the field. After
//!    `SetToolpathFloor`, the op generates to that floor.
//! 2. (a) A new Trace has no depth: generation refuses with the depth text
//!    that names `depth`; after the setter it generates.
//! 3. (c) A tool with no feeds data (a ball nose Pocket in plywood, a
//!    `FeedsError::Unbacked` cell of FM5) drafts an op with no feeds: the
//!    draft carries the Suggest refusal, `unset_fields` names `feed_rate`,
//!    `plunge_rate` and `spindle_rpm`, and generation refuses with
//!    `FEEDS_NOT_SET_TEXT`. After the three setters the op generates.
//! 4. A saved project with no `floor` key and no stock `z` key loads them
//!    as not set; a saved value stays a value.
//!
//! Red proofs (run by hand, 2026-10-23; each went red, then was restored):
//! - the Pocket arm back in `apply_stock_defaults` (min(half, 5)): claim 1;
//! - `required_depth` and the prologue check removed (`depth.unwrap_or(1.0)`):
//!   claim 2 generates instead of refusing;
//! - the raw 1000/500 feeds back in `PocketConfig::default`: claim 3.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test unset_values_refuse_s11
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::floor::{FLOOR_UNSET_TEXT, Floor};
use rs_cam_core::compute::not_set::FEEDS_NOT_SET_TEXT;
use rs_cam_core::compute::stock_config::{STOCK_THICKNESS_NOT_SET_TEXT, StockConfigPatch};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::ids::ModelId;
use rs_cam_core::material::{Material, PlywoodGrade};
use rs_cam_core::session::{
    AddModelArgs, AddToolArgs, AddToolpathArgs, Command, LoadedModel, ProjectSession,
    SetStockConfigArgs, SetToolpathFloorArgs, SetToolpathParamArgs,
};

const DRAWING: &str = "tests/fixtures/selection_p2_7/layers.dxf";
/// The thickness (mm) the operator states for the board.
const THICKNESS_MM: f64 = 12.0;

/// A session with the drawing and one tool; the stock is the auto stock of
/// the drawing.
fn drawing_session(tool: ToolConfig) -> ProjectSession {
    let mut s = ProjectSession::new_empty();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model = LoadedModel::from_file(
        ModelId::placeholder(),
        "layers",
        &dir.join(DRAWING),
        None,
        None,
        &dir,
    )
    .expect("the drawing loads");
    let _ = s
        .apply(Command::AddModel(AddModelArgs {
            model: Box::new(model),
        }))
        .expect("the add");
    let _ = s
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(tool),
        }))
        .expect("the tool");
    s
}

fn set_thickness(s: &mut ProjectSession, z: f64) {
    let mut stock = s.stock_config().clone();
    let _ = StockConfigPatch {
        z: Some(z),
        ..StockConfigPatch::default()
    }
    .apply_to(&mut stock);
    let _ = s
        .apply(Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(stock),
        }))
        .expect("the stock door");
}

fn set_material(s: &mut ProjectSession, material: Material) {
    let mut stock = s.stock_config().clone();
    stock.material = material;
    let _ = s
        .apply(Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(stock),
        }))
        .expect("the stock door");
}

/// Draft `op_type` through THE door and add it. The index of the op.
fn add_drafted(s: &mut ProjectSession, op_type: OperationType) -> usize {
    let draft = s
        .draft_toolpath(
            op_type,
            s.tools()[0].id,
            s.models()[0].id,
            op_type.label().to_owned(),
        )
        .expect("the tool fits the op");
    s.apply(Command::AddToolpath(AddToolpathArgs {
        setup_index: 0,
        config: Box::new(draft.config),
    }))
    .expect("the add")
    .created
    .expect("the add creates a toolpath")
}

fn set_param(s: &mut ProjectSession, index: usize, param: &str, value: serde_json::Value) {
    let _ = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index,
            param: param.to_owned(),
            value,
        }))
        .unwrap_or_else(|e| panic!("{param}: {e}"));
}

fn generate_err(s: &mut ProjectSession, index: usize) -> String {
    let cancel = AtomicBool::new(false);
    match s.generate_toolpath(index, &cancel) {
        Ok(_) => panic!("the op generated, but a value is not set"),
        Err(e) => e.to_string(),
    }
}

fn generate_moves(s: &mut ProjectSession, index: usize) -> usize {
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(index, &cancel)
        .unwrap_or_else(|e| panic!("generation: {e}"))
        .op_data
        .annotated()
        .toolpath
        .moves
        .len()
}

fn end_mill() -> ToolConfig {
    ToolConfig {
        diameter: 3.0,
        ..ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill)
    }
}

/// Claim 1.
#[test]
fn a_new_pocket_on_a_drawing_has_no_floor_and_refuses_s11() {
    let mut s = drawing_session(end_mill());
    assert_eq!(
        s.stock_config().thickness(),
        None,
        "a drawing has no thickness"
    );
    let index = add_drafted(&mut s, OperationType::Pocket);
    let op = &s.toolpath_configs()[index].operation;
    assert_eq!(op.floor(), Some(Floor::Unset), "no rule gives a floor");
    assert!(
        op.unset_fields().contains(&"floor"),
        "{:?}",
        op.unset_fields()
    );
    assert!(
        op.feed_rate().is_some(),
        "non-vacuity: the end mill in softwood is a backed cell, so Suggest wrote the feeds"
    );

    // The stock first: no op cuts a stock with no thickness.
    let text = generate_err(&mut s, index);
    assert!(text.contains(STOCK_THICKNESS_NOT_SET_TEXT), "{text}");

    // The thickness keeps the stock top at the drawing.
    let top_before = s.stock_config().top_z();
    set_thickness(&mut s, THICKNESS_MM);
    assert_eq!(s.stock_config().thickness(), Some(THICKNESS_MM));
    assert!(
        (s.stock_config().top_z() - top_before).abs() < 1e-9,
        "the stock top stays at the drawing"
    );
    assert!(
        s.stock_config().auto_from_model,
        "the first thickness completes the auto stock"
    );

    // Then the floor: the refusal names the field and the fix.
    let text = generate_err(&mut s, index);
    assert!(text.contains(FLOOR_UNSET_TEXT), "{text}");
    assert!(text.contains("`floor`"), "{text}");

    let _ = s
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index,
            floor: Floor::top(3.0),
        }))
        .expect("the floor door");
    assert!(
        s.toolpath_configs()[index]
            .operation
            .unset_fields()
            .is_empty(),
        "every value is set now"
    );
    assert!(generate_moves(&mut s, index) > 0, "the op cuts");
}

/// Claim 2.
#[test]
fn a_new_trace_has_no_depth_and_refuses_s11() {
    let mut s = drawing_session(end_mill());
    set_thickness(&mut s, THICKNESS_MM);
    let index = add_drafted(&mut s, OperationType::Trace);
    assert_eq!(
        s.toolpath_configs()[index].operation.unset_fields(),
        vec!["depth"]
    );
    let text = generate_err(&mut s, index);
    assert!(
        text.contains("the depth is not set") && text.contains("`depth`"),
        "{text}"
    );
    set_param(&mut s, index, "depth", serde_json::json!(1.0));
    assert!(generate_moves(&mut s, index) > 0, "the op cuts");
}

/// Claim 3.
#[test]
fn an_unbacked_tool_refuses_with_the_feeds_text_s11() {
    let ball = ToolConfig {
        diameter: 6.35,
        ..ToolConfig::new_default(ToolId::from_raw(0), ToolType::BallNose)
    };
    let mut s = drawing_session(ball);
    set_thickness(&mut s, THICKNESS_MM);
    set_material(
        &mut s,
        Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        },
    );
    let draft = s
        .draft_toolpath(
            OperationType::Pocket,
            s.tools()[0].id,
            s.models()[0].id,
            "ball pocket".to_owned(),
        )
        .expect("the tool fits the op; only the feeds data is missing");
    assert!(
        draft.feeds_refusal.is_some(),
        "non-vacuity: the cell is Unbacked (FM5)"
    );
    assert_eq!(
        draft.config.operation.unset_fields(),
        vec!["floor", "feed_rate", "plunge_rate", "spindle_rpm"],
        "no raw feeds ship"
    );
    let index = s
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(draft.config),
        }))
        .expect("the op stays in the project")
        .created
        .expect("created");
    let _ = s
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index,
            floor: Floor::top(2.0),
        }))
        .expect("the floor door");
    let text = generate_err(&mut s, index);
    assert!(text.contains(FEEDS_NOT_SET_TEXT), "{text}");
    for field in ["`feed_rate`", "`plunge_rate`", "`spindle_rpm`"] {
        assert!(text.contains(field), "{field}: {text}");
    }
    set_param(&mut s, index, "feed_rate", serde_json::json!(900.0));
    set_param(&mut s, index, "plunge_rate", serde_json::json!(300.0));
    set_param(&mut s, index, "spindle_rpm", serde_json::json!(16_000));
    assert!(generate_moves(&mut s, index) > 0, "the op cuts");
}

/// Claim 4: the file. A key that is not in the file loads not set; a
/// value in the file stays.
#[test]
fn a_saved_file_without_the_key_loads_not_set_s11() {
    let mut s = drawing_session(end_mill());
    let index = add_drafted(&mut s, OperationType::Pocket);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("unset_values_refuse_s11");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("unset.toml");
    // The file stores the drawing by path: copy it beside the project.
    let _ = std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DRAWING),
        dir.join("layers.dxf"),
    )
    .expect("copy the drawing");
    s.save(&path).expect("the save");
    let text = std::fs::read_to_string(&path).expect("the file");
    assert!(
        !text.contains("from = \"unset\""),
        "the file writes no unset floor"
    );
    let loaded = ProjectSession::load(&path).expect("the load");
    assert_eq!(loaded.stock_config().thickness(), None);
    assert_eq!(
        loaded.toolpath_configs()[index].operation.floor(),
        Some(Floor::Unset)
    );

    set_thickness(&mut s, THICKNESS_MM);
    let _ = s
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index,
            floor: Floor::top(4.0),
        }))
        .expect("the floor door");
    s.save(&path).expect("the save");
    let loaded = ProjectSession::load(&path).expect("the load");
    assert_eq!(loaded.stock_config().thickness(), Some(THICKNESS_MM));
    assert_eq!(
        loaded.toolpath_configs()[index].operation.floor(),
        Some(Floor::top(4.0))
    );
    let _ = std::fs::remove_dir_all(&dir);
}
