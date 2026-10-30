//! R1 (operator ruling 2026-10-30): an EMPTY feed field takes the Suggest
//! value.
//!
//! # The claim
//!
//! On op add, on a tool change (`SetToolpathTool`, the GUI Tool combo
//! through `ReplaceToolpathConfig`) and on the P3 auto-bind (`AddTool`),
//! each empty feed field (`feed_rate`, `plunge_rate`, `spindle_rpm`,
//! `ramp_feed_rate`) takes the value of the Apply funnel under
//! `ApplyScope::Speeds`, with a Suggest stamp
//! (`ValueProvenance::is_suggested`).
//!
//! 1. A new op with a backed tool has its feeds, stamped suggested.
//! 2. A typed feed stays through a tool change; an empty plunge fills.
//! 3. An unbacked tool (`FeedsError::Unbacked`) leaves the fields empty, and
//!    generation refuses with the feeds text.
//! 4. The auto-bind of a new tool fills the feeds of a waiting op.
//! 5. (Lead follow-up) A feed that Suggest wrote for the old tool takes the
//!    Suggest value of the new tool; a typed plunge stays.
//! 6. (W3f) A change to a tool with no Suggest basis (Unbacked) CLEARS each
//!    feed that Suggest wrote for the old tool, with its stamp ("no
//!    underived defaults"); a typed plunge stays, and generation refuses.
//!
//! Red proof: delete the `seed_feeds` call in `bind_tool_impl`; tests 2, 4
//! and 5 fail on the empty plunge, the empty feed and the old feed. Delete
//! the `refused` arm of `seed_feeds`; test 6 fails on the old feed.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test empty_feeds_take_suggest_r1
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
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::not_set::FEEDS_NOT_SET_TEXT;
use rs_cam_core::compute::stock_config::StockConfigPatch;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::{FeedsField, ProvenanceSource};
use rs_cam_core::ids::ModelId;
use rs_cam_core::material::{Material, PlywoodGrade};
use rs_cam_core::session::{
    AddModelArgs, AddToolArgs, AddToolpathArgs, Command, DraftRequest, LoadedModel, ProjectSession,
    ReplaceToolpathConfigArgs, SetStockConfigArgs, SetToolpathFloorArgs, SetToolpathParamArgs,
    SetToolpathToolArgs,
};

const DRAWING: &str = "tests/fixtures/selection_p2_7/layers.dxf";

/// A session with the drawing, a 12 mm board and `tools`.
fn drawing_session(tools: Vec<ToolConfig>, material: Option<Material>) -> ProjectSession {
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
    let mut stock = s.stock_config().clone();
    let _ = StockConfigPatch {
        z: Some(12.0),
        ..StockConfigPatch::default()
    }
    .apply_to(&mut stock);
    if let Some(material) = material {
        stock.material = material;
    }
    let _ = s
        .apply(Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(stock),
        }))
        .expect("the stock door");
    for tool in tools {
        add_tool(&mut s, tool);
    }
    s
}

fn add_tool(s: &mut ProjectSession, tool: ToolConfig) {
    let _ = s
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(tool),
        }))
        .expect("the tool");
}

fn end_mill(diameter: f64) -> ToolConfig {
    ToolConfig {
        diameter,
        ..ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill)
    }
}

/// A ball nose in Baltic birch on a Pocket is an Unbacked cell (FM5).
fn ball() -> ToolConfig {
    ToolConfig {
        diameter: 6.35,
        ..ToolConfig::new_default(ToolId::from_raw(0), ToolType::BallNose)
    }
}

fn baltic_birch() -> Option<Material> {
    Some(Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    })
}

/// Draft a Pocket through THE door with the first fitting tool and add it.
fn add_pocket(s: &mut ProjectSession) -> usize {
    let draft = s
        .draft_toolpath(
            DraftRequest::new(OperationType::Pocket, 0, "Pocket").model(s.models()[0].id),
        )
        .expect("the draft");
    s.apply(Command::AddToolpath(AddToolpathArgs {
        setup_index: 0,
        config: Box::new(draft.config),
    }))
    .expect("the add")
    .created
    .expect("created")
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

/// True when `field` of the op at `index` has a Suggest stamp.
fn suggested(s: &ProjectSession, index: usize, field: FeedsField) -> bool {
    s.toolpath_configs()[index]
        .feeds_provenance
        .get(field)
        .is_some_and(rs_cam_core::feeds::ValueProvenance::is_suggested)
}

fn assert_speeds_suggested(s: &ProjectSession, index: usize) {
    let op = &s.toolpath_configs()[index].operation;
    assert!(op.feed_rate().is_some(), "the feed is set");
    assert!(op.plunge_rate().is_some(), "the plunge is set");
    assert!(op.spindle_rpm().is_some(), "the spindle speed is set");
    for field in [
        FeedsField::FeedRate,
        FeedsField::PlungeRate,
        FeedsField::SpindleRpm,
    ] {
        assert!(suggested(s, index, field), "{field:?} is stamped suggested");
    }
}

/// Claim 1.
#[test]
fn a_new_op_with_a_tool_has_suggested_feeds_r1() {
    let mut s = drawing_session(vec![end_mill(3.0)], None);
    let index = add_pocket(&mut s);
    assert_speeds_suggested(&s, index);
}

/// Claim 2, both tool doors.
#[test]
fn a_tool_change_keeps_a_typed_feed_and_fills_an_empty_plunge_r1() {
    for door in ["set_toolpath_tool", "replace_toolpath_config"] {
        let mut s = drawing_session(vec![end_mill(3.0), end_mill(6.0)], None);
        let index = add_pocket(&mut s);
        set_param(&mut s, index, "feed_rate", serde_json::json!(1234.0));
        set_param(&mut s, index, "plunge_rate", serde_json::Value::Null);
        let tc = &s.toolpath_configs()[index];
        assert_eq!(
            tc.operation.plunge_rate(),
            None,
            "{door}: the plunge is empty"
        );
        assert_eq!(
            tc.feeds_provenance
                .get(FeedsField::FeedRate)
                .map(|p| p.source),
            Some(ProvenanceSource::Manual),
            "{door}: a typed feed is stamped manual"
        );
        let other = s.tools()[1].id;
        assert_ne!(tc.tool_id, Some(other), "{door}: the op has the first tool");

        if door == "set_toolpath_tool" {
            let _ = s
                .apply(Command::SetToolpathTool(SetToolpathToolArgs {
                    index,
                    tool_id: other,
                }))
                .expect("the tool door");
        } else {
            let mut config = s.toolpath_configs()[index].clone();
            config.tool_id = Some(other);
            let _ = s
                .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
                    index,
                    config: Box::new(config),
                }))
                .expect("the replace door");
        }

        let tc = &s.toolpath_configs()[index];
        assert_eq!(tc.tool_id, Some(other));
        assert_eq!(
            tc.operation.feed_rate(),
            Some(1234.0),
            "{door}: the typed feed stays"
        );
        assert_eq!(
            tc.feeds_provenance
                .get(FeedsField::FeedRate)
                .map(|p| p.source),
            Some(ProvenanceSource::Manual),
            "{door}: the typed feed keeps its stamp"
        );
        assert!(
            tc.operation.plunge_rate().is_some(),
            "{door}: the empty plunge takes the Suggest value"
        );
        assert!(
            suggested(&s, index, FeedsField::PlungeRate),
            "{door}: the seeded plunge is stamped suggested"
        );
    }
}

/// Claim 3.
#[test]
fn an_unbacked_tool_leaves_the_feeds_empty_and_refuses_r1() {
    let mut s = drawing_session(vec![ball()], baltic_birch());
    let index = add_pocket(&mut s);
    let op = &s.toolpath_configs()[index].operation;
    assert_eq!(op.feed_rate(), None, "non-vacuity: the cell is Unbacked");
    add_tool(&mut s, ball());
    let other = s.tools()[1].id;
    let _ = s
        .apply(Command::SetToolpathTool(SetToolpathToolArgs {
            index,
            tool_id: other,
        }))
        .expect("the tool door binds an unbacked tool");
    let tc = &s.toolpath_configs()[index];
    assert_eq!(tc.tool_id, Some(other));
    assert_eq!(tc.operation.feed_rate(), None);
    assert_eq!(tc.operation.plunge_rate(), None);
    assert_eq!(tc.operation.spindle_rpm(), None);
    let _ = s
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index,
            floor: Floor::top(2.0),
        }))
        .expect("the floor door");
    let cancel = AtomicBool::new(false);
    let text = match s.generate_toolpath(index, &cancel) {
        Ok(_) => panic!("an op with no feeds generated"),
        Err(e) => e.to_string(),
    };
    assert!(text.contains(FEEDS_NOT_SET_TEXT), "{text}");
}

/// Claim 4.
#[test]
fn the_auto_bind_fills_the_feeds_r1() {
    let mut s = drawing_session(Vec::new(), None);
    let index = add_pocket(&mut s);
    let tc = &s.toolpath_configs()[index];
    assert_eq!(tc.tool_id, None, "non-vacuity: no tool fits yet");
    assert_eq!(tc.operation.feed_rate(), None, "no tool, no feeds");
    add_tool(&mut s, end_mill(3.0));
    assert_eq!(
        s.toolpath_configs()[index].tool_id,
        Some(s.tools()[0].id),
        "the auto-bind binds the new tool"
    );
    assert_speeds_suggested(&s, index);
}

/// Claim 5 (lead follow-up 2026-10-30): a suggested value is not a typed
/// value, so a tool change replaces it with the new tool's Suggest value.
#[test]
fn a_tool_change_replaces_a_suggested_feed_and_keeps_a_typed_plunge_r1() {
    let mut s = drawing_session(vec![end_mill(3.0), end_mill(6.0)], None);
    let index = add_pocket(&mut s);
    assert!(
        suggested(&s, index, FeedsField::FeedRate),
        "non-vacuity: the feed of tool A is a Suggest value"
    );
    set_param(&mut s, index, "plunge_rate", serde_json::json!(321.0));
    let feed_a = s.toolpath_configs()[index].operation.feed_rate();
    let tool_b = s.tools()[1].id;

    // B's suggestion: THE funnel under `Speeds` on the op with tool B.
    let mut config_b = s.toolpath_configs()[index].clone();
    config_b.tool_id = Some(tool_b);
    let slots = s.suggest_slots(&config_b);
    let input = rs_cam_core::feeds::suggest::OperationApplyInput {
        tool: &s.tools()[1],
        machine: s.machine(),
        material: &s.stock_config().material,
        spindle_strategy: s.post_config().spindle_strategy,
        lut: rs_cam_core::feeds::embedded_vendor_lut(),
        suggest: slots.context(&config_b.dressups),
    };
    let mut expected = config_b.operation.clone();
    let mut stamps = config_b.feeds_provenance.clone();
    let _ = rs_cam_core::feeds::suggest::apply_to_operation(
        input,
        rs_cam_core::feeds::suggest::ApplyScope::Speeds,
        None,
        &mut expected,
        &mut stamps,
    )
    .expect("tool B is a backed cell");
    assert_ne!(
        expected.feed_rate(),
        feed_a,
        "non-vacuity: tool B has a different suggested feed"
    );

    let _ = s
        .apply(Command::SetToolpathTool(SetToolpathToolArgs {
            index,
            tool_id: tool_b,
        }))
        .expect("the tool door");
    let tc = &s.toolpath_configs()[index];
    assert_eq!(
        tc.operation.feed_rate(),
        expected.feed_rate(),
        "the suggested feed takes B's suggestion"
    );
    assert!(
        suggested(&s, index, FeedsField::FeedRate),
        "the new feed is stamped suggested"
    );
    assert_eq!(
        tc.operation.plunge_rate(),
        Some(321.0),
        "the typed plunge stays"
    );
    assert_eq!(
        tc.feeds_provenance
            .get(FeedsField::PlungeRate)
            .map(|p| p.source),
        Some(ProvenanceSource::Manual)
    );
}

/// Claim 6 (W3f): a suggested feed has no basis for an Unbacked tool, so
/// the tool change clears it. A typed value stays.
#[test]
fn a_change_to_an_unbacked_tool_clears_the_suggested_feeds_r1() {
    let mut s = drawing_session(vec![end_mill(3.0)], baltic_birch());
    let index = add_pocket(&mut s);
    assert_speeds_suggested(&s, index);
    set_param(&mut s, index, "plunge_rate", serde_json::json!(321.0));
    add_tool(&mut s, ball());
    let unbacked = s.tools()[1].id;
    let _ = s
        .apply(Command::SetToolpathTool(SetToolpathToolArgs {
            index,
            tool_id: unbacked,
        }))
        .expect("the tool door binds an unbacked tool");
    let tc = &s.toolpath_configs()[index];
    assert_eq!(tc.tool_id, Some(unbacked));
    assert_eq!(tc.operation.feed_rate(), None, "the suggested feed clears");
    assert_eq!(
        tc.operation.spindle_rpm(),
        None,
        "the suggested spindle speed clears"
    );
    for field in [FeedsField::FeedRate, FeedsField::SpindleRpm] {
        assert!(
            tc.feeds_provenance.get(field).is_none(),
            "{field:?}: the stamp goes with the value"
        );
    }
    assert_eq!(
        tc.operation.plunge_rate(),
        Some(321.0),
        "the typed plunge stays"
    );
    assert_eq!(
        tc.feeds_provenance
            .get(FeedsField::PlungeRate)
            .map(|p| p.source),
        Some(ProvenanceSource::Manual)
    );
    let _ = s
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index,
            floor: Floor::top(2.0),
        }))
        .expect("the floor door");
    let cancel = AtomicBool::new(false);
    let text = match s.generate_toolpath(index, &cancel) {
        Ok(_) => panic!("an op with a cleared feed generated"),
        Err(e) => e.to_string(),
    };
    assert!(text.contains(FEEDS_NOT_SET_TEXT), "{text}");
}
