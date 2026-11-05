//! F033 (tech-debt review 2026-10-19, operator ruling 2): one height-order
//! rule for every surface.
//!
//! # The defect
//!
//! Two checkers tested the plane order with different anchors and
//! severities. The GUI badge compared the raw Top Z (the rule R7 called
//! false for Adaptive 3D) and rated Clearance below Retract DANGER; the
//! diagnostics rated it Caution. No edit door and no export door read
//! either check, so an inverted stack exported.
//!
//! # The claim
//!
//! THE rule is `compute::config::height_order` (through
//! `ToolpathHeights::height_order`). For one inverted stack (a Retract Z
//! below the Top Z):
//!
//! 1. the badge (`height_order_badge`) and the diagnostic
//!    (`diagnose_toolpath_with_trace`, the MCP route) give the same
//!    severity;
//! 2. the MCP door (`SetToolpathHeights`), the GUI and CLI door
//!    (`ReplaceToolpathConfig`) and generation take the stack, and THE rule
//!    names the break with ONE text;
//! 3. the export door writes the program and names each break id in the
//!    result and the header;
//! 4. the Heights tab gate (`admit_heights_edit`, which the rows and the
//!    Auto buttons write through) and the diagram drag write the edit and
//!    keep the break text as the row state.
//!
//! K6 (feeds contract P2, ruling Q6, lead decision 2026-11-03: the height
//! edit door accepts the edit and warns): before P2 claims 2 to 4 were
//! refusals. The one-rule claim (one text, one severity) is unchanged.
//!
//! Red proofs (inject, see red, revert) are in the F033 report.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test one_height_order_rule_for_every_surface_f033
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    HeightMode, HeightOrderViolation, HeightReference, HeightsConfig, ReferenceOffset,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::Severity;
use rs_cam_core::diagnostics::ids::{
    GEOM_BOTTOM_ABOVE_TOP_Z, GEOM_CLEARANCE_Z_BELOW_RETRACT_Z, GEOM_FEED_Z_BELOW_TOP_Z,
    GEOM_RETRACT_Z_BELOW_FEED_Z, GEOM_RETRACT_Z_BELOW_TOP_Z,
};
use rs_cam_core::geo::P3;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AddToolpathArgs, AdoptResultArgs, Command, LoadedModel, ProjectSession, ProjectSessionBuilder,
    ReplaceToolpathConfigArgs, SetToolpathHeightsArgs, ToolpathComputeResult, ToolpathConfig,
    ToolpathHeights,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::state::job::{ModelKind, ModelUnits};
use rs_cam_viz::state::panels::HeightsRefusal;
use rs_cam_viz::state::toolpath::{HeightContext, OperationType};
use rs_cam_viz::ui::properties::{admit_heights_edit, draw_height_diagram, height_order_badge};
use rs_cam_viz::ui::tokens;

const TOOL: usize = 1;
const MODEL_2D: usize = 4;
const STOCK_THICKNESS_MM: f64 = 18.0;

/// The ids of the plane-order findings.
const ORDER_IDS: &[&str] = &[
    GEOM_CLEARANCE_Z_BELOW_RETRACT_Z,
    GEOM_RETRACT_Z_BELOW_TOP_Z,
    GEOM_RETRACT_Z_BELOW_FEED_Z,
    GEOM_FEED_Z_BELOW_TOP_Z,
    GEOM_BOTTOM_ABOVE_TOP_Z,
];

// ── fixture ─────────────────────────────────────────────────────────────

fn polygon_model(id: usize) -> LoadedModel {
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(id),
        path: PathBuf::from(format!("model_{id}.svg")),
        name: format!("2D {id}"),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::rectangle(-10.0, -10.0, 10.0, 10.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

fn session() -> ProjectSession {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(TOOL), ToolType::EndMill);
    tool.diameter = 6.0;
    let stock = StockConfig {
        z: Some(STOCK_THICKNESS_MM),
        auto_from_model: false,
        ..StockConfig::default()
    };
    ProjectSessionBuilder::new()
        .tool(tool)
        .model(polygon_model(MODEL_2D))
        .stock(stock)
        .build()
}

fn pocket_config(heights: HeightsConfig) -> ToolpathConfig {
    let mut op = OperationConfig::Pocket(Default::default());
    if let OperationConfig::Pocket(cfg) = &mut op {
        cfg.floor = rs_cam_core::compute::floor::Floor::top(6.0);
    }
    // K6: the order no longer refuses first, so the op states its feeds
    // (S11) to reach generation.
    rs_cam_core::compute::fixture_values::fill_unset(&mut op);
    ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: ToolpathId::from_raw(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: op,
        dressups: Default::default(),
        heights,
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(TOOL)),
        model_id: rs_cam_core::ids::ModelId::from_raw(MODEL_2D),
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

/// `AddToolpath` stores the config as given. It stands for a project file
/// that holds an inverted stack.
fn add(session: &mut ProjectSession, heights: HeightsConfig) -> usize {
    session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(pocket_config(heights)),
        }))
        .expect("AddToolpath stores the config")
        .created
        .expect("the row reports the new index")
}

/// THE inverted stack of this file: Retract Z 2 mm below the stock top,
/// which is the auto Top Z. Clearance and Feed stay Auto and follow it.
fn inverted() -> HeightsConfig {
    HeightsConfig {
        retract_z: HeightMode::FromReference(ReferenceOffset {
            reference: HeightReference::StockTop,
            offset: -2.0,
        }),
        ..HeightsConfig::default()
    }
}

/// The one text of the break, from THE rule.
fn expected_text(session: &ProjectSession, idx: usize) -> String {
    let tc = &session.toolpath_configs()[idx];
    session
        .height_order_refusal(tc)
        .expect("the inverted stack is a break")
        .to_string()
}

// ── arm 1: the badge and the diagnostic ────────────────────────────────

#[test]
fn the_badge_and_the_diagnostic_give_one_severity_f033() {
    let mut s = session();
    let idx = add(&mut s, inverted());
    let tc = s.toolpath_configs()[idx].clone();
    let answer = s.toolpath_heights(&tc).expect("the heights resolve");

    // Non-vacuity: the rule sees the break.
    let order = answer.height_order(&tc.heights, tc.operation.op_type());
    assert!(
        order
            .iter()
            .any(|v| matches!(v, HeightOrderViolation::RetractBelowTop { .. })),
        "the fixture must break the order: {order:?}"
    );

    let badge = height_order_badge(&answer, &tc.heights, &tc.operation);
    let diags = s
        .diagnose_toolpath_with_trace(idx, None)
        .expect("the toolpath exists");
    let worst = diags
        .iter()
        .filter(|d| ORDER_IDS.contains(&d.id.0.as_str()))
        .map(|d| d.severity)
        .max()
        .expect("the MCP route reports the break");
    assert!(
        diags
            .iter()
            .any(|d| d.id.0 == GEOM_RETRACT_Z_BELOW_TOP_Z && d.severity == Severity::Critical),
        "the MCP route names the retract below the top as Critical: {diags:?}"
    );
    let badge_of_worst = match worst {
        Severity::Severe | Severity::Critical => Some(tokens::DANGER),
        Severity::Caution => Some(tokens::CAUTION),
        Severity::Info | Severity::Hint => None,
    };
    assert_eq!(
        badge, badge_of_worst,
        "the badge and the diagnostic disagree on the severity"
    );
}

// ── arm 2: the edit doors and generation give one text ─────────────────

#[test]
fn every_door_takes_an_inverted_stack_and_names_it_with_one_text_f033() {
    // The text from a stored inverted stack.
    let mut stored = session();
    let bad = add(&mut stored, inverted());
    let expected = expected_text(&stored, bad);
    assert!(
        expected.contains("Retract Z") && expected.contains("Pocket"),
        "the text names the plane and the toolpath: {expected}"
    );

    // K6: generation goes ahead; the rule still names the break.
    stored
        .generate_toolpath(bad, &AtomicBool::new(false))
        .expect("no finding refuses generation");
    assert_eq!(expected_text(&stored, bad), expected);

    // The MCP door (`set_toolpath_heights` sends this row) takes the edit.
    let mut s = session();
    let idx = add(&mut s, HeightsConfig::default());
    assert_eq!(idx, bad, "both sessions hold one toolpath at one index");
    let _ = s
        .apply(Command::SetToolpathHeights(SetToolpathHeightsArgs {
            index: idx,
            heights: inverted(),
        }))
        .expect("the MCP door takes the edit and warns");
    assert_eq!(s.toolpath_configs()[idx].heights, inverted());
    assert_eq!(expected_text(&s, idx), expected, "the MCP door");

    // The GUI panel and the CLI feeds apply (`ReplaceToolpathConfig`).
    let mut other = session();
    let idx = add(&mut other, HeightsConfig::default());
    let mut candidate = other.toolpath_configs()[idx].clone();
    candidate.heights = inverted();
    let _ = other
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: idx,
            config: Box::new(candidate),
        }))
        .expect("the GUI and CLI door takes the edit and warns");
    assert_eq!(expected_text(&other, idx), expected, "the GUI and CLI door");
}

// ── arm 3: the export door ──────────────────────────────────────────────

#[test]
fn the_export_door_names_an_inverted_stack_f033() {
    let mut s = session();
    let idx = add(&mut s, inverted());
    let expected = expected_text(&s, idx);

    // A hand-built result: the door reads the config, not the path.
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 25.0));
    path.feed_to(P3::new(5.0, 0.0, 12.0), 600.0);
    let revision = s.toolpath_revision_at(idx);
    let _ = s
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: idx,
            revision,
            result: Box::new(ToolpathComputeResult {
                op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(
                    AnnotatedToolpath::new(path),
                )),
                stats: Default::default(),
                debug_trace: None,
                semantic_trace: None,
                provenance: rs_cam_core::session::ResultProvenance::Dressed,
                lineage: std::sync::Arc::new(rs_cam_core::trace::lineage::Lineage::unrecorded()),
            }),
        }))
        .expect("adopt the hand-built result");

    let exported = s
        .export_gcode_checked(None)
        .expect("K6: the export door writes an inverted stack");
    let warning = exported
        .warnings
        .iter()
        .find(|w| w.id == GEOM_RETRACT_Z_BELOW_TOP_Z)
        .expect("the export names the break");
    assert!(
        expected.contains(&warning.detail) || warning.detail.contains("Retract Z"),
        "the warning carries the rule text\nexport: {}\nrule: {expected}",
        warning.detail
    );
    assert!(
        exported
            .gcode
            .lines()
            .any(|l| l.contains("WARNING") && l.contains(GEOM_RETRACT_Z_BELOW_TOP_Z)),
        "the header names the break"
    );
}

// ── arm 4: the Heights tab writes the edit and warns ────────────────────

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// The rows and the Auto buttons write through `admit_heights_edit`.
#[test]
fn the_heights_tab_writes_an_edit_that_inverts_the_stack_and_warns_f033() {
    let mut s = session();
    let idx = add(&mut s, HeightsConfig::default());
    let tc = s.toolpath_configs()[idx].clone();
    let answer = s.toolpath_heights(&tc).expect("the heights resolve");
    let ctx = context();
    let mut heights = tc.heights;
    let mut refusal = HeightsRefusal::default();
    let mut written = None;
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        written = Some(admit_heights_edit(
            ui,
            &mut heights,
            inverted(),
            &answer,
            OperationType::Pocket,
            &mut refusal,
        ));
    });
    out.textures_delta.clear();
    assert_eq!(written, Some(true), "K6: the gate writes the edit");
    assert_eq!(heights, inverted(), "the edit is written");
    // UI-09: the warning text is GUI state, not egui memory.
    let shown = refusal.text(0.0).expect("the gate keeps the warning");
    assert!(shown.contains("Retract Z"), "{shown}");

    // Non-vacuity: an edit in order is written.
    let fine = HeightsConfig {
        clearance_z: HeightMode::Manual(100.0),
        ..HeightsConfig::default()
    };
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        written = Some(admit_heights_edit(
            ui,
            &mut heights,
            fine.clone(),
            &answer,
            OperationType::Pocket,
            &mut refusal,
        ));
    });
    out.textures_delta.clear();
    assert_eq!(written, Some(true));
    assert_eq!(heights, fine);
    assert_eq!(refusal.text(0.0), None, "an edit in order clears it");
}

/// A waterline: retract 20 (the safe Z), top 9.8 (the stock top).
fn waterline_ctx() -> HeightContext {
    HeightContext {
        safe_z: 20.0,
        op_depth: 10.0,
        stock_top_z: 9.8,
        stock_bottom_z: -17.2,
        model_top_z: Some(9.8),
        model_bottom_z: Some(0.0),
        floor: None,
    }
}

fn frame(
    ctx: &egui::Context,
    heights: &mut HeightsConfig,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let input = egui::RawInput {
        events,
        ..Default::default()
    };
    let hctx = waterline_ctx();
    let mut out = ctx.run_ui(input, |ui| {
        ui.set_max_width(280.0);
        let answer = ToolpathHeights::from_context(heights, hctx);
        draw_height_diagram(
            ui,
            heights,
            &answer,
            OperationType::Waterline,
            None,
            &mut HeightsRefusal::default(),
        );
    });
    let shapes = std::mem::take(&mut out.shapes);
    out.textures_delta.clear();
    shapes
}

fn line_centre(shapes: &[egui::epaint::ClippedShape], color: egui::Color32) -> egui::Pos2 {
    for clipped in shapes {
        if let egui::Shape::LineSegment { points, stroke } = &clipped.shape
            && stroke.color == color
            && (points[0].y - points[1].y).abs() < 1e-3
        {
            return egui::pos2((points[0].x + points[1].x) * 0.5, points[0].y);
        }
    }
    panic!("the diagram drew no horizontal line in {color:?}");
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// A drag of the Top line far up, past the retract plane, stops below it.
#[test]
fn the_diagram_drag_passes_the_retract_plane_f033() {
    let ctx = context();
    let mut heights = HeightsConfig::default();
    let auto = ToolpathHeights::from_context(&heights, waterline_ctx()).heights;
    assert!(auto.top_z < auto.retract_z, "the fixture starts in order");

    let shapes = frame(&ctx, &mut heights, Vec::new());
    let start = line_centre(&shapes, tokens::CAUTION);
    let _ = frame(&ctx, &mut heights, vec![egui::Event::PointerMoved(start)]);
    let _ = frame(&ctx, &mut heights, vec![button(start, true)]);
    let mut at = start;
    for _ in 0..40 {
        at = egui::pos2(at.x, at.y - 8.0);
        let _ = frame(&ctx, &mut heights, vec![egui::Event::PointerMoved(at)]);
    }
    let _ = frame(&ctx, &mut heights, vec![button(at, false)]);

    let after = ToolpathHeights::from_context(&heights, waterline_ctx()).heights;
    // Non-vacuity: the drag moved the Top up.
    assert!(
        after.top_z > auto.top_z + 1.0,
        "the drag must raise the Top: {} vs {}",
        after.top_z,
        auto.top_z
    );
    // K6: the drag is written past the retract plane; the badge and the
    // row state name the break.
    assert!(
        after.top_z > after.retract_z,
        "the drag stopped the Top Z ({}) at the Retract Z ({})",
        after.top_z,
        after.retract_z
    );
}

// ── arm 5: the floor rule owns a floor at the top ───────────────────────

/// On Pocket, Profile, Zigzag and Adaptive 2D the bottom is the floor. A
/// floor at or above the top is the floor rule's refusal (L-16): the GUI
/// Floor row and MCP `set_toolpath_floor` both write it, the Floor line
/// names it, and generation refuses it with the floor text. The order door
/// does not refuse that edit first, so the two floor routes agree.
#[test]
fn a_floor_above_the_top_is_left_to_the_floor_rule_f033() {
    let mut s = session();
    let idx = add(&mut s, HeightsConfig::default());
    let mut candidate = s.toolpath_configs()[idx].clone();
    if let OperationConfig::Pocket(cfg) = &mut candidate.operation {
        cfg.floor = rs_cam_core::compute::floor::Floor::Z {
            z: STOCK_THICKNESS_MM + 50.0,
        };
    }
    let _ = s
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: idx,
            config: Box::new(candidate),
        }))
        .expect("the GUI floor route takes the floor, as set_toolpath_floor does");
    // Non-vacuity: THE order rule sees the break; generation and export
    // refuse it.
    assert!(
        s.height_order_refusal(&s.toolpath_configs()[idx]).is_some(),
        "the order rule sees the floor above the top"
    );
    match s.generate_toolpath(idx, &AtomicBool::new(false)) {
        Ok(_) => panic!("the floor rule refuses: the ladder is empty"),
        Err(other) => assert!(
            other.to_string().contains("floor"),
            "the floor rule names the floor: {other}"
        ),
    }
}

// ── arm 6: a top-included ladder takes top = bottom (F033b) ─────────────

/// F033b (lead ruling): the Waterline ladder includes its top
/// (`Capabilities::ladder_includes_top`), so top = bottom is one level.
/// The Heights tab gate writes it, and writes a bottom above the top with
/// the break as its warning (K6).
/// A Pocket ladder starts one pass below its top, so a Pocket floor at
/// the top still refuses at generation.
#[test]
fn a_top_included_ladder_takes_top_equal_to_bottom_f033b() {
    const Z_MM: f64 = 5.0;
    let one_level = HeightsConfig {
        top_z: HeightMode::Manual(Z_MM),
        bottom_z: HeightMode::Manual(Z_MM),
        ..HeightsConfig::default()
    };
    let answer = ToolpathHeights::from_context(&HeightsConfig::default(), waterline_ctx());
    assert_eq!(
        answer.height_order(&one_level, OperationType::Waterline),
        Vec::new(),
        "THE rule: top = bottom is one Waterline level"
    );

    // The Heights tab gate writes it.
    let ctx = context();
    let mut heights = HeightsConfig::default();
    let mut refusal = HeightsRefusal::default();
    let mut written = None;
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        written = Some(admit_heights_edit(
            ui,
            &mut heights,
            one_level.clone(),
            &answer,
            OperationType::Waterline,
            &mut refusal,
        ));
    });
    out.textures_delta.clear();
    assert_eq!(written, Some(true), "the gate takes top = bottom");
    assert_eq!(heights, one_level);

    // A bottom above the top is written, with the break as its warning.
    let above = HeightsConfig {
        bottom_z: HeightMode::Manual(Z_MM + 0.5),
        ..one_level
    };
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        written = Some(admit_heights_edit(
            ui,
            &mut heights,
            above.clone(),
            &answer,
            OperationType::Waterline,
            &mut refusal,
        ));
    });
    out.textures_delta.clear();
    assert_eq!(written, Some(true), "K6: the gate writes the edit");
    assert_eq!(heights, above, "the edit is written");
    assert!(
        refusal.text(0.0).is_some_and(|t| t.contains("Bottom")),
        "the gate keeps the break as the row state: {:?}",
        refusal.text(0.0)
    );

    // Pocket: a floor at the top is top = bottom on a top-excluded ladder.
    let mut s = session();
    let idx = add(&mut s, HeightsConfig::default());
    let top_z = s
        .toolpath_heights(&s.toolpath_configs()[idx])
        .expect("the heights resolve")
        .heights
        .top_z;
    let mut candidate = s.toolpath_configs()[idx].clone();
    if let OperationConfig::Pocket(cfg) = &mut candidate.operation {
        cfg.floor = rs_cam_core::compute::floor::Floor::Z { z: top_z };
    }
    let _ = s
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: idx,
            config: Box::new(candidate),
        }))
        .expect("the floor route takes the floor (arm 5)");
    let refusal = s
        .height_order_refusal(&s.toolpath_configs()[idx])
        .expect("the order rule refuses top = bottom on Pocket");
    assert_eq!(
        refusal.violations,
        vec![HeightOrderViolation::BottomNotBelowTop {
            bottom_z: top_z,
            top_z,
        }],
        "{refusal}"
    );
    assert!(
        s.generate_toolpath(idx, &AtomicBool::new(false)).is_err(),
        "generation refuses a Pocket floor at the top"
    );
}
