//! **P29-9 — the GUI feeds doors read the session Suggest door.**
//!
//! # The defect
//!
//! The session door (`ProjectSession::suggest_slots` and
//! `SuggestSlots::context`) fills the plan facts of a `SuggestContext`: the
//! setup fact (9b), the earlier op's leave (9c) and the toolpath's dressups
//! (the configured entry, P29-8 8d). MCP and the CLI read it. The panel
//! built its own context with the model box only, so the GUI envelope
//! (`SafeEnvelope::cut_checks`) and the pill values differed from MCP on a
//! from-below Project Curve, on a finish after a rough and on an op with a
//! ramp entry.
//!
//! # The repair
//!
//! `SuggestContext` has no `Default` and private plan facts. The panel
//! snapshot carries the session's `SuggestSlots`; every panel door builds
//! its context with `SuggestSlots::context` and the entry's dressups.
//!
//! # The claim
//!
//! For each of the three cases, the real inspector (`properties::draw`, the
//! feeds memo) gives the same `cut_checks` as `ProjectSession::safe_envelope`
//! and the same pill values (`FieldApplyPreviews`) as the session door.
//! Non-vacuity: for each case, the old viz context (the model box only)
//! gives other `cut_checks`, so the fixture exercises the fact.
//!
//! # The scan
//!
//! The compiler holds every constructor but one: it cannot stop a product
//! door from calling `SuggestContext::no_project`, the context with no
//! project. `no_project_is_not_a_product_door_p29_9` holds that rule by a
//! scan of the product source of the GUI (with the embedded MCP server)
//! and the CLI. A test module and a test file may call it.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

#[path = "../../rs_cam_core/tests/common/source_scan.rs"]
mod source_scan;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::operation_configs::ProjectCurveDirection;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{EnvelopeInput, SafeEnvelope};
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::properties::ToolpathTab;
use rs_cam_viz::ui::{properties, tokens};

const PANEL_WIDTH: f32 = 280.0;
const END_MILL: ToolId = ToolId::from_raw(1);
const BALL: ToolId = ToolId::from_raw(2);

fn tool(id: ToolId, kind: ToolType) -> ToolConfig {
    let mut tool = ToolConfig::test_default(id, kind);
    tool.diameter = 6.0;
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

fn config(name: &str, tool_id: ToolId, operation: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: rs_cam_core::ToolpathId::placeholder(),
        name: name.to_owned(),
        enabled: true,
        dressups: DressupConfig::for_op(operation.op_type()),
        operation,
        heights: Default::default(),
        tool_id: Some(tool_id),
        model_id: rs_cam_core::ids::ModelId::from_raw(1),
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
    }
}

/// The names of the three cases, in toolpath order after the rough.
const PROJECT_CURVE: &str = "a from-below Project Curve";
const FINISH: &str = "a finish after a rough";
const RAMP: &str = "a ramp entry";

/// One setup (top up, not flipped) with a rough that leaves 0.5 mm, a
/// Scallop finish after it, a Project Curve from below and a 2D Adaptive
/// with a ramp entry.
fn fixture() -> AppState {
    let model = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(1),
        path: PathBuf::from("p29_9_fixture.svg"),
        name: "P29-9 fixture".to_owned(),
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
    let stock = StockConfig {
        // A material with a force line, so the force budget judges.
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..Default::default()
    };
    let mut rough = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Adaptive3d);
    if let OperationConfig::Adaptive3d(c) = &mut rough {
        c.stock_to_leave_axial = 0.5;
    }
    let finish = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Scallop);
    let mut curve = rs_cam_core::compute::fixture_values::fixture_op(OperationType::ProjectCurve);
    if let OperationConfig::ProjectCurve(c) = &mut curve {
        c.depth = Some(-1.0);
        c.direction = ProjectCurveDirection::FromBelow;
    }
    // A 2D Adaptive starts with a helix (the registry default), so a ramp
    // is a configured entry that the registry default does not give.
    let mut ramp = config(
        RAMP,
        END_MILL,
        rs_cam_core::compute::fixture_values::fixture_op(OperationType::Adaptive),
    );
    assert_eq!(ramp.dressups.entry_style, DressupEntryStyle::Helix);
    ramp.dressups.entry_style = DressupEntryStyle::Ramp;

    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool(END_MILL, ToolType::EndMill))
        .tool(tool(BALL, ToolType::BallNose))
        .model(model);
    for cfg in [
        config("the rough", END_MILL, rough),
        config(FINISH, BALL, finish),
        config(PROJECT_CURVE, END_MILL, curve),
        ramp,
    ] {
        builder
            .add_toolpath(0, cfg)
            .expect("add a fixture toolpath");
    }
    let mut state = AppState::new();
    state.replace_session(builder.build());
    state
}

/// Draw the real inspector on the toolpath `id` until its memo is full.
fn draw(state: &mut AppState, id: rs_cam_core::ToolpathId) {
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    let _ = state.set_focus(Selection::Toolpath(id));
    state.gui.pending_toolpath_tab = Some((id, ToolpathTab::Passes));
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    for _ in 0..3 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("p29_9_properties")
                .default_size(PANEL_WIDTH)
                .max_size(PANEL_WIDTH)
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

/// The envelope of `tc` with the old viz context: the model box only.
fn old_viz_envelope(session: &ProjectSession, tc: &ToolpathConfig) -> SafeEnvelope {
    let tool = session.toolpath_tool(tc).expect("the case tool");
    let slots = session.suggest_slots(tc);
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: &tc.operation,
        tool,
        machine: session.machine(),
        material: &session.stock_config().material,
        lut: rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy: session.post_config().spindle_policy(),
        context: SuggestContext::no_project(NoProject {
            model_bbox: slots.model_bbox(),
            ..NoProject::NONE
        }),
    })
    .expect("the old viz envelope builds")
}

#[test]
fn the_gui_reads_the_session_suggest_door_p29_9() {
    let mut state = fixture();
    let cases = [PROJECT_CURVE, FINISH, RAMP];
    for name in cases {
        let tc = state
            .session()
            .toolpath_configs()
            .iter()
            .find(|tc| tc.name == name)
            .cloned()
            .unwrap_or_else(|| panic!("the fixture holds {name}"));
        draw(&mut state, tc.id);
        let session = state.session();
        let memo = &state
            .gui
            .toolpath_rt
            .get(&tc.id)
            .expect("the drawn toolpath has a runtime")
            .feeds_memo;
        assert!(
            memo.refusal().is_none(),
            "{name}: the pairing is served: {:?}",
            memo.refusal()
        );

        // 1. The envelope checks: the viz door against the session door.
        let viz = memo.envelope().expect("the memo holds the envelope");
        let door = session
            .safe_envelope(&tc)
            .expect("the case has a tool")
            .expect("the session envelope builds");
        assert!(!door.cut_checks.is_empty(), "{name}: non-vacuity");
        assert_eq!(
            format!("{:?}", viz.cut_checks),
            format!("{:?}", door.cut_checks),
            "{name}: the GUI envelope checks equal the session door's"
        );
        assert_eq!(
            format!("{:?}", viz.proposal),
            format!("{:?}", door.proposal),
            "{name}: the GUI proposal equals the session door's"
        );

        // 2. Non-vacuity: the old viz context gives other checks, so the
        //    case exercises its plan fact.
        let old = old_viz_envelope(session, &tc);
        assert_ne!(
            format!("{:?}", old.cut_checks),
            format!("{:?}", door.cut_checks),
            "{name}: the fixture must exercise its plan fact"
        );

        // 3. The Suggest values the pills offer (what `Apply all` writes):
        //    the viz door against the session door.
        let tool = session.toolpath_tool(&tc).expect("the case tool");
        let slots = session.suggest_slots(&tc);
        let door_previews = rs_cam_core::feeds::suggest::preview_field_applies(
            &tc.operation,
            tool,
            session.machine(),
            &session.stock_config().material,
            rs_cam_core::feeds::embedded_vendor_lut(),
            session.post_config().spindle_policy(),
            slots.context(&tc.dressups),
        );
        assert_eq!(
            memo.previews(),
            Some(&door_previews),
            "{name}: the GUI pill values equal the session door's"
        );
    }
}

/// The product text of one source file: no comments, and no top-level
/// `#[cfg(test)]` item (a test module, a test helper). An item is the
/// attribute lines, then one line that ends in `;`, or the lines up to the
/// brace that closes its body.
fn product_text(path: &Path) -> String {
    let src = std::fs::read_to_string(path).expect("read a source file");
    let text = source_scan::strip_comments(&src);
    let mut out = String::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if !line.starts_with("#[cfg(test)]") {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        // Skip the other attributes, then the item.
        let mut depth: i64 = 0;
        let mut opened = false;
        for item_line in lines.by_ref() {
            let trimmed = item_line.trim_start();
            if !opened && depth == 0 && trimmed.starts_with("#[") {
                continue;
            }
            for ch in item_line.chars() {
                match ch {
                    '{' => {
                        depth += 1;
                        opened = true;
                    }
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            if (opened && depth <= 0) || (!opened && trimmed.ends_with(';')) {
                break;
            }
        }
    }
    out
}

/// Whether `path` is a test file (`tests.rs`, or under a `tests` folder).
fn is_test_file(path: &Path) -> bool {
    path.file_name().is_some_and(|n| n == "tests.rs")
        || path.components().any(|c| c.as_os_str() == "tests")
}

#[test]
fn no_project_is_not_a_product_door_p29_9() {
    const NEEDLE: &str = "no_project(";
    let viz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates = viz.parent().expect("the crates folder");
    // Non-vacuity 1: the needle names the core constructor.
    let core = std::fs::read_to_string(crates.join("rs_cam_core/src/feeds/suggest.rs"))
        .expect("read core suggest.rs");
    assert!(
        source_scan::strip_comments(&core).contains("pub fn no_project("),
        "the needle must name the constructor"
    );
    let mut files = source_scan::rust_files(&viz.join("src"));
    files.extend(source_scan::rust_files(&crates.join("rs_cam_cli/src")));
    // Non-vacuity 2: the reader sees the needle in a test module of the
    // product tree, and the panel door reads the session slots.
    let pills = viz.join("src/ui/properties/pills.rs");
    assert!(
        source_scan::strip_comments(&std::fs::read_to_string(&pills).expect("read pills.rs"))
            .contains(NEEDLE),
        "the pills test module calls the needle: the reader must see it"
    );
    assert!(
        product_text(&viz.join("src/state/runtime.rs")).contains("suggest_slots.context("),
        "the panel door builds its context from the session slots"
    );
    // Non-vacuity 3: a file with a `#[cfg(test)]` item near its top keeps
    // the product text after it (the snapshot door sits far below it).
    let properties = product_text(&viz.join("src/ui/properties/mod.rs"));
    assert!(
        properties.contains("suggest_slots: session.suggest_slots(tc)"),
        "the reader keeps the product text after an early test item"
    );
    assert!(
        !product_text(&pills).contains(NEEDLE),
        "the reader drops the pills test module"
    );
    let offenders: Vec<String> = files
        .iter()
        .filter(|path| !is_test_file(path))
        .filter(|path| product_text(path).contains(NEEDLE))
        .map(|path| path.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "a product door builds a Suggest context with no project; read \
         `ProjectSession::suggest_slots` or `new_op_suggest_slots`: {offenders:?}"
    );
}
