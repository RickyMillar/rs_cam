//! **F216 — the toolpath panel runs the feeds calculator once per input.**
//!
//! # The defect
//!
//! The Geometry tab ran `feeds_result_for_operation` (the calculator and a
//! vendor LUT lookup) and `PillSuggestions::new` (a dry run of the apply
//! funnel) on EVERY frame. The comment said "Compute + cache", but no key
//! guarded the call, and no production path cleared `feeds_result`.
//!
//! # The repair
//!
//! `state::runtime::FeedsMemo` holds a key beside `feeds_result`. The key
//! derives from ONE `FeedsInputs` value, and the build reads only that
//! value (the W11 `SlotInputs` shape). `FeedsMemo::builds` counts the
//! calculator runs; this sentry reads it.
//!
//! # The arms
//!
//! 1. The real inspector: 100 frames with no edit run the calculator once,
//!    the Feeds tab reads the same memo, and one session edit of each input
//!    runs it once more. An edit that is not an input runs nothing.
//! 2. The memo itself: a move of each of the six `FeedsInputs` fields runs
//!    the build again (the model box has no session edit in this fixture).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::SpindleStrategy;
use rs_cam_core::feeds::suggest::FieldApplyPreviews;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::material::{Material, PlywoodGrade};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSessionBuilder, ReplaceToolArgs, SetMachineArgs,
    SetPostConfigArgs, SetStockConfigArgs, SetToolpathParamArgs, ToolpathConfig,
};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::{FeedsInputs, FeedsMemo, ToolpathRuntime};
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::properties::ToolpathTab;
use rs_cam_viz::ui::{properties, tokens};

const PANEL_WIDTH: f32 = 280.0;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    ctx
}

fn tool() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.flute_count = 2;
    tool
}

fn fixture() -> AppState {
    let stock = StockConfig {
        material: Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        },
        ..Default::default()
    };
    let config = ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId(0),
        name: "F216 fixture".to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(OperationType::Pocket),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 1,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let model = LoadedModel {
        id: 1,
        path: PathBuf::from("f216_fixture.svg"),
        name: "F216 fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::rectangle(0.0, 0.0, 40.0, 40.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool())
        .model(model);
    builder
        .add_toolpath(0, config)
        .expect("add the F216 fixture");
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let id = state.session().toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, ToolpathTab::Geometry));
    state
}

/// Draw the real inspector `n` times.
fn frames(ctx: &egui::Context, state: &mut AppState, n: usize) {
    for _ in 0..n {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("f216_properties")
                .default_size(PANEL_WIDTH)
                .max_size(PANEL_WIDTH)
                .resizable(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut events = Vec::new();
                        properties::draw(ui, state, &mut events);
                    });
                });
        });
        out.textures_delta.clear();
    }
}

fn runtime(state: &AppState) -> &ToolpathRuntime {
    let id = state.session().toolpath_configs()[0].id;
    state
        .gui
        .toolpath_rt
        .get(&id)
        .expect("the fixture toolpath has a runtime")
}

fn builds(state: &AppState) -> u64 {
    runtime(state).feeds_memo.builds()
}

fn apply(state: &mut AppState, command: Command) {
    // The panel reads the session again on the next frame; the GUI stamp
    // of the effects is not under test here.
    let _effects = state
        .apply(command)
        .unwrap_or_else(|error| panic!("the fixture edit must apply: {error:?}"));
}

/// Draw `n` frames after an edit and return the builds they ran.
fn builds_after(ctx: &egui::Context, state: &mut AppState, n: usize) -> u64 {
    let before = builds(state);
    frames(ctx, state, n);
    builds(state) - before
}

// ── arm 1 — the real inspector ───────────────────────────────────────────

#[test]
fn the_panel_runs_the_calculator_once_per_input_f216() {
    let ctx = ctx();
    let mut state = fixture();

    frames(&ctx, &mut state, 100);
    assert_eq!(
        builds(&state),
        1,
        "100 Geometry-tab frames with no edit must run the feeds calculator \
         once. More than one is the per-frame cost F216 removed; zero means \
         the panel no longer reads the memo."
    );
    // Non-vacuity: the pills have a result and a dry run to read.
    assert!(
        runtime(&state).feeds_result.is_some(),
        "the fixture pairing must give a feeds result, or the count proves nothing"
    );
    assert!(
        runtime(&state).feeds_memo.previews().is_some(),
        "the memo must hold the funnel dry run the pills read"
    );

    // The Feeds tab reads the same memo: no second calculator run.
    let id = state.session().toolpath_configs()[0].id;
    state.gui.pending_toolpath_tab = Some((id, ToolpathTab::FeedsSpeeds));
    assert_eq!(
        builds_after(&ctx, &mut state, 100),
        0,
        "the Feeds tab must read the memo the Geometry tab filled"
    );
    state.gui.pending_toolpath_tab = Some((id, ToolpathTab::Geometry));
    assert_eq!(builds_after(&ctx, &mut state, 10), 0);

    // An edit that is not an input runs nothing.
    if let Some(rt) = state.gui.toolpath_rt.get_mut(&id) {
        rt.locked = !rt.locked;
    }
    assert_eq!(
        builds_after(&ctx, &mut state, 10),
        0,
        "a lock toggle is not a feeds input"
    );

    // The operation.
    let stepover = state.session().toolpath_configs()[0]
        .operation
        .stepover()
        .expect("a pocket has a stepover");
    apply(
        &mut state,
        Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "stepover".to_owned(),
            value: serde_json::json!(stepover + 0.5),
        }),
    );
    assert_eq!(
        builds_after(&ctx, &mut state, 10),
        1,
        "an op edit runs once"
    );

    // The tool.
    let mut bigger = tool();
    bigger.diameter = 8.0;
    apply(
        &mut state,
        Command::ReplaceTool(ReplaceToolArgs {
            tool_id: 1,
            config: Box::new(bigger),
        }),
    );
    assert_eq!(
        builds_after(&ctx, &mut state, 10),
        1,
        "a tool edit runs once"
    );

    // The material.
    let mut stock = state.session().stock_config().clone();
    stock.material = Material::Plywood {
        grade: PlywoodGrade::HardwoodFaced,
    };
    apply(
        &mut state,
        Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(stock),
        }),
    );
    assert_eq!(
        builds_after(&ctx, &mut state, 10),
        1,
        "a material edit runs once"
    );

    // The machine.
    let mut machine = state.session().machine().clone();
    machine.aggressiveness = 0.6;
    apply(
        &mut state,
        Command::SetMachine(SetMachineArgs {
            machine: Box::new(machine),
        }),
    );
    assert_eq!(
        builds_after(&ctx, &mut state, 10),
        1,
        "a machine edit runs once"
    );

    // The spindle strategy.
    let mut post = state.session().post_config().clone();
    post.spindle_strategy = match post.spindle_strategy {
        SpindleStrategy::MatchChart => SpindleStrategy::MaxSpeed,
        SpindleStrategy::MaxSpeed => SpindleStrategy::MatchChart,
    };
    apply(
        &mut state,
        Command::SetPostConfig(SetPostConfigArgs {
            post: Box::new(post),
        }),
    );
    assert_eq!(
        builds_after(&ctx, &mut state, 10),
        1,
        "a spindle-strategy edit runs once"
    );

    // And back to quiet.
    assert_eq!(builds_after(&ctx, &mut state, 100), 0);
}

// ── arm 2 — the memo: each input is in the key ───────────────────────────

/// Arm 2 measures the key, not the dry run. Arm 1 drives the production
/// dry run (`ui::properties::pills::pill_previews`) through the panel.
fn no_dry_run(
    _operation: &OperationConfig,
    _result: &rs_cam_core::feeds::FeedsResult,
    _tool: &ToolConfig,
    _machine: &rs_cam_core::machine::MachineProfile,
    _material: &Material,
    _model_bbox: Option<&BoundingBox3>,
) -> FieldApplyPreviews {
    FieldApplyPreviews::default()
}

#[test]
fn every_feeds_input_moves_the_memo_key_f216() {
    let operation = OperationConfig::new_default(OperationType::Pocket);
    let tool = tool();
    let material = Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    };
    let machine = rs_cam_core::session::ProjectSession::new_empty()
        .machine()
        .clone();
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, -10.0),
        max: P3::new(40.0, 40.0, 0.0),
    };
    let base = || FeedsInputs {
        operation: &operation,
        tool: &tool,
        material: &material,
        machine: &machine,
        spindle_strategy: SpindleStrategy::MatchChart,
        model_bbox: Some(&bbox),
    };

    let mut memo = FeedsMemo::default();
    let mut result = None;
    for _ in 0..100 {
        memo.refresh(&mut result, base(), no_dry_run);
    }
    assert_eq!(
        memo.builds(),
        1,
        "100 refreshes of one input value build once"
    );
    assert!(result.is_some(), "the fixture pairing must give a result");

    let mut moved_op = operation.clone();
    moved_op.set_feed_rate(operation.feed_rate() + 100.0);
    let mut moved_tool = tool.clone();
    moved_tool.flute_count = 3;
    let moved_material = Material::Plywood {
        grade: PlywoodGrade::HardwoodFaced,
    };
    let mut moved_machine = machine.clone();
    moved_machine.aggressiveness = 0.6;
    let moved_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, -10.0),
        max: P3::new(80.0, 40.0, 0.0),
    };

    let cases: Vec<(&str, FeedsInputs<'_>)> = vec![
        (
            "operation",
            FeedsInputs {
                operation: &moved_op,
                ..base()
            },
        ),
        (
            "tool",
            FeedsInputs {
                tool: &moved_tool,
                ..base()
            },
        ),
        (
            "material",
            FeedsInputs {
                material: &moved_material,
                ..base()
            },
        ),
        (
            "machine",
            FeedsInputs {
                machine: &moved_machine,
                ..base()
            },
        ),
        (
            "spindle_strategy",
            FeedsInputs {
                spindle_strategy: SpindleStrategy::MaxSpeed,
                ..base()
            },
        ),
        (
            "model_bbox",
            FeedsInputs {
                model_bbox: Some(&moved_bbox),
                ..base()
            },
        ),
        (
            "model_bbox gone",
            FeedsInputs {
                model_bbox: None,
                ..base()
            },
        ),
    ];
    for (name, moved) in cases {
        let before = memo.builds();
        memo.refresh(&mut result, moved, no_dry_run);
        assert_eq!(
            memo.builds(),
            before + 1,
            "a move of {name} must build again"
        );
        memo.refresh(&mut result, base(), no_dry_run);
        assert_eq!(
            memo.builds(),
            before + 2,
            "the return to the base value after {name} must build again"
        );
        memo.refresh(&mut result, base(), no_dry_run);
        assert_eq!(
            memo.builds(),
            before + 2,
            "a repeat of {name}'s base is a hit"
        );
    }
}
