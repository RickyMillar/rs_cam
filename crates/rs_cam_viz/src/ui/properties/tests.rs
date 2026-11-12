//! Unit tests for the inspector's tab enum, its header and its rows.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use super::ToolpathTab;

/// The MCP `set_ui_view` tool documents these tab keys — every
/// documented key must parse, every tab must be reachable, and
/// unknown keys must stay `None` (set_ui_view validates upstream,
/// this is the backstop).
#[test]
fn toolpath_tab_parse_covers_all_documented_keys() {
    assert!(matches!(
        ToolpathTab::parse("geometry"),
        Some(ToolpathTab::Geometry)
    ));
    assert!(matches!(
        ToolpathTab::parse("feeds"),
        Some(ToolpathTab::Passes)
    ));
    // P5b: the Feeds tab merged into Passes; its keys open Passes.
    assert!(matches!(
        ToolpathTab::parse("feeds_speeds"),
        Some(ToolpathTab::Passes)
    ));
    assert!(matches!(
        ToolpathTab::parse("linking"),
        Some(ToolpathTab::Linking)
    ));
    assert!(matches!(
        ToolpathTab::parse("heights"),
        Some(ToolpathTab::Heights)
    ));
    // W2b: the old keys are gone (no-legacy ruling 2026-09-16).
    assert!(ToolpathTab::parse("dressup").is_none());
    assert!(ToolpathTab::parse("cut").is_none());
    assert!(matches!(
        ToolpathTab::parse("passes"),
        Some(ToolpathTab::Passes)
    ));
    assert!(ToolpathTab::parse("not_a_tab").is_none());
    // UI-06: `key()` is the ONE key table. The hand-written match that used
    // to sit here was a third copy of the list, beside the parser and the
    // MCP refusal message. Every canonical key must name its own variant.
    assert!(
        // P5b: four tabs (the Feeds tab merged into Passes).
        ToolpathTab::ALL.len() >= 4,
        "the tab list shrank; this loop would pass for the wrong reason"
    );
    for &tab in ToolpathTab::ALL {
        assert_eq!(
            ToolpathTab::parse(tab.key()),
            Some(tab),
            "`{}` does not parse back to the tab it names",
            tab.key()
        );
    }
}

/// Q1: the panel snapshot carries the model bounding box.
///
/// `SuggestContext::model_bbox` gates the runtime-sanity stepover
/// back-off. The inspector's two Suggest sites — the Geometry-tab pill
/// funnel and the Feeds-tab card — passed `SuggestContext::default()`,
/// so the GUI recommendation could differ from the controller and the
/// MCP recommendation for the same toolpath. Both now read this field,
/// so the assembly is the thing to pin: the snapshot reports the box of
/// the model the STORED `tc.model_id` names, and it agrees with
/// `ProjectSession::model_bbox_by_id`.
/// One pocket over one flat STL model, with one end mill.
///
/// UI-01 gave this fixture a second reader, so it is a function rather than
/// the body of one test.
fn one_pocket_session() -> (
    rs_cam_core::session::ProjectSession,
    std::sync::Arc<rs_cam_core::mesh::TriangleMesh>,
) {
    use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
    use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
    use std::sync::Arc;

    let tool = rs_cam_core::compute::tool_config::ToolConfig::test_default(
        rs_cam_core::compute::ToolId::from_raw(1),
        rs_cam_core::compute::tool_config::ToolType::EndMill,
    );
    let mut builder = ProjectSessionBuilder::new().tool(tool);
    let mesh = Arc::new(rs_cam_core::mesh::make_test_flat(40.0));
    let _ = builder.add_model(LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        path: std::path::PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::clone(&mesh)),
        items: Default::default(),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    });
    let _ = builder
        .add_toolpath(
            0,
            ToolpathConfig {
                shoe: Default::default(),
                feeds_lock: Default::default(),
                id: rs_cam_core::compute::ToolpathId::from_raw(0),
                name: "Pocket".to_owned(),
                enabled: true,
                operation: crate::state::toolpath::OperationConfig::new_default(
                    crate::state::toolpath::OperationType::Pocket,
                ),
                dressups: Default::default(),
                heights: Default::default(),
                tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
                model_id: rs_cam_core::ids::ModelId::from_raw(0),
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
            },
        )
        .expect("the fixture toolpath is addable");
    (builder.build(), mesh)
}

/// Q1: the snapshot must carry the bounding box of the model the toolpath
/// machines, because both of the panel's Suggest sites read it.
#[test]
fn the_panel_snapshot_carries_the_model_bbox() {
    let (session, mesh) = one_pocket_session();
    let tc = &session.toolpath_configs()[0];
    let snapshot = super::toolpath_panel_snapshot(
        tc.id,
        &session,
        &crate::state::runtime::GuiState::default(),
    )
    .expect("the toolpath resolves for the properties panel");

    let expected = session
        .model_bbox_by_id(tc.model_id)
        .expect("the fixture model carries a finite bbox");
    // P29-9: the bbox reaches the panel inside the session door's slots.
    let carried = snapshot
        .suggest_slots
        .model_bbox()
        .expect("the snapshot must carry the model bbox, not None");
    assert_eq!(
        format!("{carried:?}"),
        format!("{expected:?}"),
        "the snapshot bbox must be `ProjectSession::model_bbox_by_id` of the stored model id"
    );
    assert_eq!(
        format!("{:?}", carried.min),
        format!("{:?}", mesh.bbox.min),
        "the bbox must be the model's own, not a placeholder"
    );
}

/// UI-01: the panel's read side is ASSEMBLED, not passed in 21 pieces.
///
/// `draw_toolpath_panel` took 25 parameters; 21 of them were this struct.
/// The risk the struct removes is a caller that builds the list by hand and
/// substitutes an empty vector or a default for one entry — the panel still
/// renders, and the wrong thing is silently missing. So this asserts that
/// the builder fills the fields FROM THE SESSION, not that the struct exists.
#[test]
fn the_panel_inputs_carry_the_session_lists_ui01() {
    let (session, _mesh) = one_pocket_session();
    let tc = &session.toolpath_configs()[0];
    let inputs = super::toolpath_panel_inputs(
        tc.id,
        &session,
        &crate::state::runtime::GuiState::default(),
        None,
        Some(ToolpathTab::Heights),
        false,
    );

    assert_eq!(
        inputs.tools.len(),
        session.tools().len(),
        "the tool dropdown must list the session's tools"
    );
    assert_eq!(
        inputs.models.len(),
        session.models().len(),
        "the model dropdown must list the session's models"
    );
    assert_eq!(
        inputs.tool_configs.len(),
        session.tools().len(),
        "the feeds card reads the tool configs, one per session tool"
    );
    assert_eq!(
        format!("{:?}", inputs.material),
        format!("{:?}", session.stock_config().material),
        "the material must be the stock's own, not a default"
    );
    assert_eq!(
        inputs.tab_override,
        Some(ToolpathTab::Heights),
        "the consumed MCP tab must reach the panel"
    );
    assert!(
        matches!(inputs.toolpath_heights, Some(Ok(_))),
        "a toolpath with a model resolves its heights"
    );
    assert!(
        inputs.boundary_source_candidates.is_empty(),
        "one toolpath has no other toolpath to derive a rest boundary from"
    );
}
/// D7, changed by operator ruling R2 (2026-10-30): the tab strip draws
/// Geometry, Passes, Feeds, Heights, Linking, from left to right. The Cut
/// tab is Passes; the Dressup tab is deleted (its rows are on Linking);
/// the tool is the header Tool row. The test renders the strip and reads
/// the x of each label, so a change of `ALL` or of the strip order turns
/// it red.
///
/// Red proof: with the pre-R2 `ALL` order, the drawn order was Geometry,
/// Cut, Heights, Feeds, Linking, Dressup.
#[test]
fn the_tab_strip_is_the_r2_tab_set_d7() {
    // P5b (operator ruling 2026-11-03): the Feeds tab merged into Passes.
    const WANT: [&str; 4] = ["Geometry", "Passes", "Heights", "Linking"];
    let ctx = egui::Context::default();
    crate::ui::tokens::apply(&ctx);
    crate::ui::tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    let mut active = ToolpathTab::Geometry;
    let badges = super::tab_badges::TabBadges::none();
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        // Wide enough for one row: the order is then the x order.
        ui.set_min_width(1200.0);
        super::tab_badges::draw_toolpath_tabs(
            ui,
            &mut active,
            &badges,
            &crate::state::toolpath::OperationConfig::new_default(
                crate::state::toolpath::OperationType::Pocket,
            ),
        );
    });
    let mut drawn: Vec<(f32, f32, String)> = Vec::new();
    for clipped in &out.shapes {
        if let egui::epaint::Shape::Text(text) = &clipped.shape {
            let word = text.galley.job.text.clone();
            if WANT.contains(&word.as_str()) {
                drawn.push((text.pos.y, text.pos.x, word));
            }
        }
    }
    out.textures_delta.clear();
    assert_eq!(
        drawn.len(),
        WANT.len(),
        "non-vacuity: the strip must draw each tab label once: {drawn:?}"
    );
    let first_y = drawn[0].0;
    assert!(
        drawn.iter().all(|d| (d.0 - first_y).abs() < 1.0),
        "fixture: the strip must draw one row: {drawn:?}"
    );
    drawn.sort_by(|a, b| a.1.total_cmp(&b.1));
    let order: Vec<&str> = drawn.iter().map(|d| d.2.as_str()).collect();
    assert_eq!(order, WANT, "the tab strip order");
}

/// UI-11: both drill editors draw the SAME cycle rows.
///
/// The four-way cycle combo and the peck row were written out twice,
/// verbatim, in `operations/drill.rs`. A shared helper draws them now; this
/// renders both editors on a Peck cycle and asserts the same labels appear
/// in both, so an edit to one cannot stop reaching the other.
#[test]
fn both_drill_editors_draw_the_same_cycle_rows_ui11() {
    use crate::state::toolpath::{AlignmentPinDrillConfig, DrillConfig, DrillCycleType};

    fn rendered(mut draw: impl FnMut(&mut egui::Ui)) -> Vec<String> {
        let ctx = egui::Context::default();
        crate::ui::tokens::apply(&ctx);
        crate::ui::tokens::apply_fonts(&ctx);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        warmup.textures_delta.clear();

        let mut out = ctx.run_ui(egui::RawInput::default(), &mut draw);
        let mut texts = Vec::new();
        for clipped in &out.shapes {
            if let egui::epaint::Shape::Text(text) = &clipped.shape {
                texts.push(text.galley.job.text.clone());
            }
        }
        out.textures_delta.clear();
        texts
    }

    let mut drill = DrillConfig {
        cycle: DrillCycleType::Peck,
        ..Default::default()
    };
    let drill_texts = rendered(|ui| {
        super::operations::draw_drill_params(ui, &mut drill, None, None, Default::default());
    });

    let mut pin = AlignmentPinDrillConfig {
        cycle: DrillCycleType::Peck,
        ..Default::default()
    };
    let pin_texts = rendered(|ui| {
        super::operations::draw_alignment_pin_drill_params(
            ui,
            &mut pin,
            None,
            None,
            Default::default(),
        );
    });

    for label in ["Peck (G83)", "Peck depth", "Feed rate", "Retract (R)"] {
        assert!(
            drill_texts.iter().any(|t| t.contains(label)),
            "the drill editor no longer draws `{label}`; it rendered {drill_texts:?}"
        );
        assert!(
            pin_texts.iter().any(|t| t.contains(label)),
            "the pin-drill editor no longer draws `{label}`; it rendered {pin_texts:?}"
        );
    }
}

/// D3: the Cut tab (GT-1; the Geometry tab before) draws no boundary controls for an op whose
/// capability is `TerritoryXy::SetupMap` (Pin Drill). No boundary moves
/// its holes, so a boundary control would claim an effect that generation
/// never applies. A Pocket (`ClipAfter`) still draws them, which is the
/// non-vacuity anchor.
#[test]
fn a_setup_map_op_draws_no_boundary_controls_d3() {
    use crate::state::toolpath::{OperationType, ToolpathEntry};
    use rs_cam_core::compute::catalog::TerritoryXy;

    fn rendered(op: OperationType) -> Vec<String> {
        let (session, _mesh) = one_pocket_session();
        let tc = &session.toolpath_configs()[0];
        let inputs = super::toolpath_panel_inputs(
            tc.id,
            &session,
            &crate::state::runtime::GuiState::default(),
            None,
            Some(ToolpathTab::Geometry),
            false,
        );
        let mut entry = ToolpathEntry::for_operation(
            tc.id,
            "Case".to_owned(),
            tc.tool_id.expect("the case tool"),
            tc.model_id,
            op,
        );
        let ctx = egui::Context::default();
        crate::ui::tokens::apply(&ctx);
        crate::ui::tokens::apply_fonts(&ctx);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        warmup.textures_delta.clear();
        let mut events = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            super::toolpath_panel::draw_geometry_tab(
                ui,
                &mut entry,
                &inputs,
                &mut crate::state::pick::SelectionUi::default(),
                &mut crate::state::panels::ContainmentAutoCorrect::default(),
                &mut super::toolpath_panel::ReachView::default(),
                &[],
                &mut events,
            );
        });
        let mut texts = Vec::new();
        for clipped in &out.shapes {
            if let egui::epaint::Shape::Text(text) = &clipped.shape {
                texts.push(text.galley.job.text.clone());
            }
        }
        out.textures_delta.clear();
        texts
    }

    assert_eq!(
        crate::state::toolpath::OperationConfig::new_default(OperationType::AlignmentPinDrill)
            .capabilities()
            .territory_xy,
        TerritoryXy::SetupMap,
        "the case op must be the SetupMap op"
    );
    // W3a: the boundary controls start with ONE Boundary row (Off or a
    // source); no section header.
    let pocket = rendered(OperationType::Pocket);
    assert!(
        pocket.iter().any(|t| t == "Boundary"),
        "a Pocket must draw the boundary controls; it rendered {pocket:?}"
    );
    let pin = rendered(OperationType::AlignmentPinDrill);
    assert!(
        !pin.iter()
            .any(|t| t == "Boundary" || t == "Containment" || t == "Surface"),
        "a Pin Drill must draw no boundary controls; it rendered {pin:?}"
    );
}

/// Render the Geometry tab of an entry of `op` against the first toolpath
/// of `session`, and give the painted texts in draw order.
fn geometry_tab_texts(
    session: &rs_cam_core::session::ProjectSession,
    op: crate::state::toolpath::OperationType,
) -> Vec<String> {
    tab_texts(session, op, ToolpathTab::Geometry)
}

/// Render a tab of an entry of `op` against the first toolpath of
/// `session`, and give the painted texts in draw order. `Geometry` draws
/// the selection rows only (`geometry_tab.rs`); `Passes` draws the Passes
/// tab; `Linking` here stands for the whole Geometry tab (R2: the Input
/// row, the selection and the Machining Boundary section). The entry holds
/// the stored op config when its type is `op`.
fn tab_texts(
    session: &rs_cam_core::session::ProjectSession,
    op: crate::state::toolpath::OperationType,
    tab: ToolpathTab,
) -> Vec<String> {
    use crate::state::toolpath::ToolpathEntry;
    let tc = &session.toolpath_configs()[0];
    let inputs = super::toolpath_panel_inputs(
        tc.id,
        session,
        &crate::state::runtime::GuiState::default(),
        None,
        Some(ToolpathTab::Geometry),
        false,
    );
    let mut entry = ToolpathEntry::for_operation(
        tc.id,
        "Case".to_owned(),
        tc.tool_id.expect("the case tool"),
        tc.model_id,
        op,
    );
    entry.boundary = tc.boundary.clone();
    if tc.operation.op_type() == op {
        entry.operation = tc.operation.clone();
    }
    let ctx = egui::Context::default();
    crate::ui::tokens::apply(&ctx);
    crate::ui::tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    let mut events = Vec::new();
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| match tab {
        ToolpathTab::Passes => super::toolpath_panel::draw_passes_tab(
            ui,
            &mut entry,
            &inputs,
            None,
            &mut crate::state::pick::SelectionUi::default(),
            super::linking_dressup::Fields::default(),
            &session.suggest_slots(tc),
            &mut crate::state::panels::EnvelopeCandidates::default(),
            &mut events,
        ),
        // R2: the whole Geometry tab: the Input row, the selection, the
        // Machining Boundary section and the Reach row.
        ToolpathTab::Linking => super::toolpath_panel::draw_geometry_tab(
            ui,
            &mut entry,
            &inputs,
            &mut crate::state::pick::SelectionUi::default(),
            &mut crate::state::panels::ContainmentAutoCorrect::default(),
            &mut super::toolpath_panel::ReachView::default(),
            &[],
            &mut events,
        ),
        ToolpathTab::Geometry | ToolpathTab::Heights => {
            super::geometry_tab::draw_geometry_tab(
                ui,
                &entry,
                &inputs,
                &mut crate::state::pick::SelectionUi::default(),
                &[],
                &mut events,
            );
        }
    });
    let mut texts = Vec::new();
    for clipped in &out.shapes {
        if let egui::epaint::Shape::Text(text) = &clipped.shape {
            texts.push(text.galley.job.text.clone());
        }
    }
    out.textures_delta.clear();
    texts
}

/// A STEP-only project (`fixtures/gui_step/stepped_block.step`) with one
/// Pocket on an empty selection. Since UX-7 (ruling Q3) the default rule of
/// a STEP Pocket holds the face rings, so the empty selection stands in
/// for "nothing to cut".
fn step_pocket_session() -> rs_cam_core::session::ProjectSession {
    use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
    use rs_cam_core::session::{ProjectSessionBuilder, ToolpathConfig};

    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/gui_step/stepped_block.step");
    let model = rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the STEP fixture loads");
    let tool = rs_cam_core::compute::tool_config::ToolConfig::test_default(
        rs_cam_core::compute::ToolId::from_raw(1),
        rs_cam_core::compute::tool_config::ToolType::EndMill,
    );
    let mut builder = ProjectSessionBuilder::new().tool(tool);
    let model_id = builder.add_model(model);
    let _ = builder
        .add_toolpath(
            0,
            ToolpathConfig {
                shoe: Default::default(),
                feeds_lock: Default::default(),
                id: rs_cam_core::compute::ToolpathId::from_raw(0),
                name: "Pocket".to_owned(),
                enabled: true,
                operation: {
                    let mut op = crate::state::toolpath::OperationConfig::new_default(
                        crate::state::toolpath::OperationType::Pocket,
                    );
                    if let crate::state::toolpath::OperationConfig::Pocket(pocket) = &mut op {
                        pocket.selection = rs_cam_core::selection::Selection::empty();
                    }
                    op
                },
                dressups: Default::default(),
                heights: Default::default(),
                tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
                model_id,
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
            },
        )
        .unwrap();
    builder.build()
}

/// The position of the first painted text that `pred` accepts.
fn position_of(texts: &[String], what: &str, pred: impl Fn(&str) -> bool) -> usize {
    texts
        .iter()
        .position(|t| pred(t))
        .unwrap_or_else(|| panic!("the Geometry tab paints no {what}: {texts:?}"))
}

/// Geometry selection P4.D, fix 1, then W3h: a Pocket on a STEP-only
/// project with nothing to cut (an empty selection) names the fix in the
/// placeholder row of its list: "Select faces …", from the kinds of the
/// pick bar chips. Red proof: `placeholder_text` without the face kind:
/// the row reads "Select loops".
#[test]
fn a_step_pocket_names_the_face_pick_fix_p4_d() {
    let session = step_pocket_session();
    let texts = geometry_tab_texts(&session, crate::state::toolpath::OperationType::Pocket);
    assert!(
        texts.iter().any(|t| t.starts_with("Select faces")),
        "the placeholder must name the face pick: {texts:?}"
    );
}

/// Geometry selection P4.D follow-up, then W3h: the section shows ONE
/// Pick, a quiet act of the list header, in the empty state and after a
/// face pick. Red proof: a placeholder with its own `Pick` button: the
/// empty state paints two.
#[test]
fn the_selection_section_shows_one_pick_p4_d() {
    // A button paints its label more than once in this harness, so the
    // test counts Pick against the header menu act (also a quiet button).
    let count = |texts: &[String], label: &str| texts.iter().filter(|t| *t == label).count();
    let menu = crate::ui::components::list_box::MENU;

    let mut session = step_pocket_session();
    let empty = geometry_tab_texts(&session, crate::state::toolpath::OperationType::Pocket);
    let per_button = count(&empty, menu);
    assert!(
        per_button > 0,
        "fixture: the header paints its menu: {empty:?}"
    );
    assert_eq!(
        count(&empty, "Pick"),
        per_button,
        "the empty section must show one Pick: {empty:?}"
    );

    let face = session.models()[0]
        .items
        .iter()
        .find(|i| i.kind() == rs_cam_core::selection::ItemKind::Face)
        .expect("fixture: the STEP model holds a face")
        .key()
        .clone();
    let _ = session
        .apply(rs_cam_core::session::Command::EditSelection(
            rs_cam_core::session::EditSelectionArgs {
                index: 0,
                target: rs_cam_core::session::SelectionTarget::Op,
                edit: rs_cam_core::selection::SelectionEdit::AddPick(face),
            },
        ))
        .expect("the face pick applies");
    let picked = geometry_tab_texts(&session, crate::state::toolpath::OperationType::Pocket);
    assert!(
        !picked.iter().any(|t| t.starts_with("Select faces")),
        "fixture: a face pick fills the list: {picked:?}"
    );
    assert_eq!(
        count(&picked, "Pick"),
        per_button,
        "the section must show one Pick: {picked:?}"
    );
}

/// Geometry selection P4.D fix 4, then GT-1: the op selection is the
/// Geometry tab, the first tab; its list (W3h: the header "Selection")
/// opens the tab. The
/// Passes tab holds Input, Start from and the Machining Boundary source, and
/// no Pick. A 3D op shows "Pick faces" in the Geometry tab (fix 2). Red
/// proof: the selection drawn in the Passes tab again: the Passes tab paints
/// "Pick".
#[test]
fn the_selection_section_is_at_the_top_p4_d() {
    let session = step_pocket_session();
    let geometry = geometry_tab_texts(&session, crate::state::toolpath::OperationType::Pocket);
    // W3h: Pick is a quiet act of the list header (the header paints its
    // acts before its title, right to left).
    let _ = position_of(&geometry, "list header", |t| t == "Selection");
    let _ = position_of(&geometry, "Pick act", |t| t == "Pick");
    // R2: the whole Geometry tab opens with the Input row and holds the
    // Boundary row under the selection (W3a: no section header).
    let whole = tab_texts(
        &session,
        crate::state::toolpath::OperationType::Pocket,
        ToolpathTab::Linking,
    );
    let input = position_of(&whole, "Input row", |t| t.starts_with("Input"));
    let cuts = position_of(&whole, "list header", |t| t == "Selection");
    let boundary = position_of(&whole, "Boundary row", |t| t == "Boundary");
    assert!(
        input < cuts && cuts < boundary,
        "want Input < Selection < Boundary: {whole:?}"
    );
    let passes = tab_texts(
        &session,
        crate::state::toolpath::OperationType::Pocket,
        ToolpathTab::Passes,
    );
    let _ = position_of(&passes, "Rest machining row", |t| {
        t == super::REST_MACHINING_LABEL
    });
    assert!(
        !passes.iter().any(|t| t == "Pick"
            || t == "Selection"
            || t.starts_with("Input")
            || t.eq_ignore_ascii_case("machining boundary")),
        "the Passes tab holds no selection, no Input row and no boundary: {passes:?}"
    );

    // W3a: a 3D op holds the Surface row; Pick is its action.
    let rough = geometry_tab_texts(&session, crate::state::toolpath::OperationType::Adaptive3d);
    let surface = position_of(&rough, "Surface row", |t| t == "Surface");
    let pick_faces = position_of(&rough, "Pick faces action", |t| {
        t == super::geometry_tab::PICK_FACES_LABEL
    });
    assert!(
        surface < pick_faces,
        "the action follows the row: {rough:?}"
    );
}

/// P3.4: the Cuts row names the down faces of the selection (setup frame),
/// and prints no down count when every face points up.
#[test]
fn the_counts_line_names_the_down_faces_p3_4() {
    use rs_cam_core::selection::{FacesByFacing, ItemKind};
    use rs_cam_core::session::SelectionReport;

    let report = |up: usize, down: usize| SelectionReport {
        counts: [(ItemKind::Face, up + down)].into_iter().collect(),
        faces_by_facing: FacesByFacing { up, down },
        ..SelectionReport::default()
    };
    assert_eq!(
        super::selection_counts_text(&report(1, 1)).as_deref(),
        Some("2 faces (1 down)")
    );
    assert_eq!(
        super::selection_counts_text(&report(3, 0)).as_deref(),
        Some("3 faces")
    );
}

// ── GT-2: the selected list of the Geometry tab ─────────────────────────

/// A drawing of 24 circles Ø 6 and one circle Ø 10 (layer HOLES), a 6 mm
/// end mill, and a default Drill (toolpath 0).
fn holes_session() -> rs_cam_core::session::ProjectSession {
    use rs_cam_core::compute::stock_config::StockConfig;
    use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
    use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
    use std::sync::Arc;

    let circle = |x: f64, d: f64| {
        Item::new(
            SourceRef::ToolpathModel,
            Some("HOLES".into()),
            Shape::circle(rs_cam_core::geo::P2::new(x, 0.0), d),
        )
        .unwrap()
    };
    let mut items: Vec<Item> = (0..24).map(|i| circle(f64::from(i) * 12.0, 6.0)).collect();
    items.push(circle(400.0, 10.0));
    let mut tool = rs_cam_core::compute::tool_config::ToolConfig::test_default(
        rs_cam_core::compute::ToolId::from_raw(1),
        rs_cam_core::compute::tool_config::ToolType::EndMill,
    );
    tool.diameter = 6.0;
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig::default())
        .tool(tool);
    let model_id = builder.add_model(LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        path: std::path::PathBuf::from("synthetic://holes.dxf"),
        name: "Holes".to_owned(),
        kind: None,
        mesh: None,
        items: Arc::new(ItemSet::new(items)),
        enriched_mesh: None,
        units: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    });
    let _ = builder
        .add_toolpath(
            0,
            ToolpathConfig {
                shoe: Default::default(),
                feeds_lock: Default::default(),
                id: rs_cam_core::compute::ToolpathId::from_raw(0),
                name: "Drill".to_owned(),
                enabled: true,
                operation: crate::state::toolpath::OperationConfig::new_default(
                    crate::state::toolpath::OperationType::Drill,
                ),
                dressups: Default::default(),
                heights: Default::default(),
                tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
                model_id,
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
            },
        )
        .unwrap();
    let mut session = builder.build();
    // W3h: the Drill starts at its new-op default (no item).
    edit_op(&mut session, rs_cam_core::selection::SelectionEdit::Reset);
    session
}

fn edit_op(
    session: &mut rs_cam_core::session::ProjectSession,
    edit: rs_cam_core::selection::SelectionEdit,
) {
    let _ = session
        .apply(rs_cam_core::session::Command::EditSelection(
            rs_cam_core::session::EditSelectionArgs {
                index: 0,
                target: rs_cam_core::session::SelectionTarget::Op,
                edit,
            },
        ))
        .unwrap();
}

/// The inputs of the op selection of toolpath 0, as the tab builds them,
/// handed to `with`.
fn with_section_inputs<R>(
    session: &rs_cam_core::session::ProjectSession,
    with: impl FnOnce(
        &super::selection_section::SelectionSectionInputs<'_>,
        &rs_cam_core::session::SelectionReport,
    ) -> R,
) -> R {
    use rs_cam_core::session::SelectionTarget;
    let tc = &session.toolpath_configs()[0];
    let report = session.selection_status(tc, SelectionTarget::Op).unwrap();
    let read: super::SelectionRead = Ok(std::sync::Arc::clone(&report));
    let op_default = tc.operation.default_selection().unwrap();
    let kinds = crate::state::pick::held_kinds(&session.models()[0]);
    let inputs = super::selection_section::SelectionSectionInputs {
        toolpath_id: tc.id,
        toolpath_model: tc.model_id,
        target: SelectionTarget::Op,
        selection: tc.operation.selection().unwrap(),
        op_default: &op_default,
        report: Some(&read),
        item_use: tc.operation.capabilities().item_use,
        models: &[],
        offers: &[],
        op_label: tc.operation.op_type().label(),
        floor_op: tc.operation.floor().is_some(),
        note_items: &[],
        model_kinds: &kinds,
        tool_diameter: Some(6.0),
        findings: &[],
    };
    with(&inputs, &report)
}

/// The rows of the list of toolpath 0, as the tab builds them.
fn rows_of(
    session: &rs_cam_core::session::ProjectSession,
) -> (
    Vec<super::selection_section::ListRow>,
    std::sync::Arc<rs_cam_core::session::SelectionReport>,
) {
    use rs_cam_core::session::SelectionTarget;
    let tc = &session.toolpath_configs()[0];
    let report = session.selection_status(tc, SelectionTarget::Op).unwrap();
    let rows = with_section_inputs(session, |inputs, report| {
        super::selection_section::list_rows(inputs, report)
    });
    (rows, report)
}

/// The holes session after a pick of the Ø 10 circle, a pick of one Ø 6
/// circle and its first "Rule…" offer (all circles Ø 6).
fn holes_with_a_rule_and_a_pick() -> rs_cam_core::session::ProjectSession {
    use rs_cam_core::selection::{MatchKey, SelectionEdit, SourceRef};
    let mut session = holes_session();
    let model = session.toolpath_configs()[0].model_id;
    let key_of = |session: &rs_cam_core::session::ProjectSession, at: usize| MatchKey {
        source: SourceRef::Model(model),
        ..session.models()[0]
            .items
            .iter()
            .nth(at)
            .unwrap()
            .key()
            .clone()
    };
    let large = key_of(&session, 24);
    let small = key_of(&session, 3);
    edit_op(&mut session, SelectionEdit::AddPick(large));
    edit_op(&mut session, SelectionEdit::AddPick(small.clone()));
    let offers = session
        .rule_offers(
            &session.toolpath_configs()[0].clone(),
            rs_cam_core::session::SelectionTarget::Op,
            &small,
        )
        .unwrap();
    edit_op(&mut session, offers[0].edit.clone().unwrap());
    session
}

/// GT-2, W3h: the list is one row per selected item, the rule members and
/// the picks alike, so the rows ARE the report rows; every row has `×`
/// and the `…` menu holds "Rule…".
///
/// - A new Drill holds no item: no row. The header menu offers the rule
///   preset of the kind it cuts ("Every circle"), "Clear selection" (off:
///   nothing to clear) and no "Reset to default" (its default holds no
///   rule).
/// - After a pick of the Ø 10 circle and the Ø 6 rule offer: 24 rule rows
///   (`RowFrom::Rule(0)`), then 1 pick row. The menu offers "Remove rule"
///   and "Clear selection" writes the empty selection.
///
/// Red proof: let the rule rows also take the `Pick` rows: the row count
/// is more than the report rows.
#[test]
fn the_list_rows_hold_each_selected_item_once_gt2() {
    use super::selection_section::{LIST_CAP, RowFrom, header_menu_edit_of, header_menu_labels};
    use rs_cam_core::selection::{Selection, SelectionEdit};

    let session = holes_session();
    let (rows, report) = rows_of(&session);
    assert!(rows.is_empty() && report.rows.is_empty(), "{rows:?}");
    let labels = with_section_inputs(&session, |inputs, _| header_menu_labels(inputs));
    assert!(labels.iter().any(|l| l == "Every circle"), "{labels:?}");
    assert!(labels.iter().any(|l| l == "Clear selection"), "{labels:?}");
    assert!(
        !labels.iter().any(|l| l == "Reset to default"),
        "{labels:?}"
    );
    assert_eq!(
        with_section_inputs(&session, |inputs, _| header_menu_edit_of(
            inputs,
            "Clear selection"
        )),
        None,
        "nothing to clear"
    );

    let session = holes_with_a_rule_and_a_pick();
    let (rows, report) = rows_of(&session);
    assert_eq!(rows.len(), report.rows.len(), "each item in one row");
    assert_eq!(rows.len(), 25);
    assert!(rows.len() > LIST_CAP, "the list scrolls");
    assert!(rows[..24].iter().all(|r| r.from == RowFrom::Rule(0)));
    let picked = &rows[24];
    assert_eq!(picked.from, RowFrom::Pick);
    // FR-2 F22: the row text is short; the full text (the place, the
    // layer) is the hover, and the filter reads it.
    assert_eq!(picked.label, "\u{00D8}10.00", "{}", picked.label);
    assert!(
        picked.full.starts_with("\u{00D8} 10.00 at (400.0, 0.0)"),
        "{}",
        picked.full
    );
    assert!(rows.iter().all(|r| r.acts.rule && !r.acts.floor));
    let labels = with_section_inputs(&session, |inputs, _| header_menu_labels(inputs));
    assert!(
        labels.iter().any(|l| l.starts_with("Remove rule: ")),
        "{labels:?}"
    );
    assert_eq!(
        with_section_inputs(&session, |inputs, _| header_menu_edit_of(
            inputs,
            "Clear selection"
        )),
        Some(SelectionEdit::Replace(Box::new(Selection::empty())))
    );
}

/// Draw the Geometry tab of toolpath 0 of `session` (an op of `op_type`)
/// headless, move the pointer onto the first painted text that `pick`
/// accepts, and return the painted texts and the key that the hover sets
/// (`SelectionUi::row_hover`).
fn hover_a_row(
    session: &rs_cam_core::session::ProjectSession,
    op_type: crate::state::toolpath::OperationType,
    pick: impl Fn(&str) -> bool,
) -> (Vec<String>, Option<rs_cam_core::selection::MatchKey>) {
    let tc = session.toolpath_configs()[0].clone();
    let inputs = super::toolpath_panel_inputs(
        tc.id,
        session,
        &crate::state::runtime::GuiState::default(),
        None,
        Some(ToolpathTab::Geometry),
        false,
    );
    let mut entry = crate::state::toolpath::ToolpathEntry::for_operation(
        tc.id,
        "Case".to_owned(),
        tc.tool_id.expect("the case tool"),
        tc.model_id,
        op_type,
    );
    entry.operation = tc.operation;
    let ctx = egui::Context::default();
    crate::ui::tokens::apply(&ctx);
    crate::ui::tokens::apply_fonts(&ctx);
    let mut selection_ui = crate::state::pick::SelectionUi::default();
    let mut row_at: Option<egui::Pos2> = None;
    let mut texts: Vec<String> = Vec::new();
    for pass in 0..4 {
        let mut input = egui::RawInput::default();
        if pass == 3
            && let Some(pos) = row_at
        {
            input.events.push(egui::Event::PointerMoved(pos));
        }
        let mut events = Vec::new();
        let mut out = ctx.run_ui(input, |ui| {
            super::geometry_tab::draw_geometry_tab(
                ui,
                &entry,
                &inputs,
                &mut selection_ui,
                &[],
                &mut events,
            );
        });
        if pass == 2 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    let t = text.galley.job.text.clone();
                    if row_at.is_none() && pick(&t) {
                        row_at = Some(clipped.shape.visual_bounding_rect().center());
                    }
                    texts.push(t);
                }
            }
        }
        out.textures_delta.clear();
    }
    assert!(row_at.is_some(), "the tab paints the row: {texts:?}");
    (texts, selection_ui.row_hover)
}

/// GT-2, W3h: the list paints its header with the counts at the right and
/// a scroll of rule rows; a RULE MEMBER row under the pointer lights its
/// item in the viewport (`SelectionUi::row_hover`, the key that
/// `selection_highlight::hover_target` rings).
///
/// Red proof: drop the `row_hover` write of `draw_row`: the hover arm
/// fails.
#[test]
fn hovering_a_rule_member_row_lights_its_item_w3h() {
    let session = holes_with_a_rule_and_a_pick();
    let (texts, hover) = hover_a_row(
        &session,
        crate::state::toolpath::OperationType::Drill,
        |t| t.starts_with("\u{00D8}6 hole") || t.starts_with("\u{00D8}6.00 \u{00B7}"),
    );
    assert!(texts.iter().any(|t| t == "Selection"), "{texts:?}");
    assert!(
        texts.iter().any(|t| t.starts_with("25 circles")),
        "the header holds the counts: {texts:?}"
    );
    let hover = hover.expect("the row under the pointer lights its item");
    assert_eq!(hover.kind, rs_cam_core::selection::ItemKind::Circle);
    assert!((hover.measure - 6.0).abs() < 1e-9, "{hover:?}");
}

/// W3h: a FACE row under the pointer lights its face (the hover key is the
/// face key), and the empty list paints its placeholder.
#[test]
fn hovering_a_face_row_lights_the_face_w3h() {
    let mut session = step_pocket_session();
    let face = session.models()[0]
        .items
        .iter()
        .find(|i| i.kind() == rs_cam_core::selection::ItemKind::Face)
        .expect("fixture: the STEP model holds a face")
        .key()
        .clone();
    edit_op(
        &mut session,
        rs_cam_core::selection::SelectionEdit::Replace(Box::new(
            rs_cam_core::selection::Selection::from_picks(vec![face.clone()]),
        )),
    );
    let (_, hover) = hover_a_row(
        &session,
        crate::state::toolpath::OperationType::Pocket,
        |t| t.starts_with("Face Z"),
    );
    let hover = hover.expect("the face row lights its item");
    assert_eq!(hover.kind, rs_cam_core::selection::ItemKind::Face);
    assert!(
        hover.matches(&face) || face.matches(&hover),
        "{hover:?} vs {face:?}"
    );
}

/// W2b (was F24): a field finding puts its dot on the tab that draws its
/// field (the anchor names the field, `placement::field_tab` names the
/// tab). A selection refusal dots Geometry; a load finding on the feed dots
/// Feeds; a pin finding (a project anchor) and an operation finding dot no
/// tab.
///
/// Red proof: dot every current finding (the F24 id table read every id):
/// the operation finding dots a tab and the Passes arm fails.
#[test]
fn a_field_finding_dots_the_tab_of_its_field_w2b() {
    use rs_cam_core::diagnostics::{
        Category, Confidence, Diagnostic, DiagnosticState, Scope, Severity, Source, ids,
    };
    let finding = |id: &str, severity: Severity| Diagnostic {
        id: id.into(),
        scope: Scope::Toolpath {
            id: rs_cam_core::ToolpathId::from_raw(0),
        },
        category: Category::Safety,
        severity,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        message: String::new(),
        face: String::new(),
        anchor: rs_cam_core::diagnostics::presentation::anchor(id),
        evidence: None,
        fix: None,
        supersedes: Vec::new(),
        suppressed_diagnostics: Vec::new(),
    };
    let (session, _) = one_pocket_session();
    let tc = &session.toolpath_configs()[0];
    let entry = crate::state::toolpath::ToolpathEntry::for_operation(
        tc.id,
        "Case".to_owned(),
        tc.tool_id.expect("the case tool"),
        tc.model_id,
        crate::state::toolpath::OperationType::Pocket,
    );
    let list = [
        finding(ids::PRECOND_SELECTION_REFUSED, Severity::Severe),
        finding(ids::LOAD_CHIPLOAD_HIGH, Severity::Caution),
        finding(ids::PROJECT_PIN_WALL_THIN, Severity::Critical),
        finding(ids::PROJECT_GENERATED_EMPTY, Severity::Critical),
    ];
    let placement = super::placement::place(&list, &entry.operation);
    let badges = super::tab_badges::compute_tab_badges(&entry, &placement, None);
    assert_eq!(
        badges.for_tab(ToolpathTab::Geometry),
        Some(crate::ui::tokens::DANGER)
    );
    assert_eq!(
        badges.for_tab(ToolpathTab::Passes),
        Some(crate::ui::tokens::CAUTION)
    );
    for tab in [ToolpathTab::Linking, ToolpathTab::Heights] {
        assert_eq!(badges.for_tab(tab), None, "{tab:?} gets no dot");
    }
}

/// FR-2 F22 (coordinator ruling): "Done · <danger>" (K6; "Done · BLOCKED"
/// before P2) is the F20 safety signal and never truncates. The Generate row gives it its full width
/// first; the move count drops, then the feed stage, when there is no
/// room. Nothing paints past the row.
///
/// - At the inspector width 280, with a move count and a feed stage, the
///   full word paints, not elided, inside 280.
/// - At 200 the full word still paints; the move count drops.
///
/// Red proof: the F22 first version (the state word truncates in the
/// width the figures leave): at 280 the word is elided.
#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn the_blocked_state_word_fits_its_slot_f22() {
    use rs_cam_core::compute::catalog::ModelGougeRule;
    use rs_cam_core::compute::model_gouge::ModelGougeFinding;
    use rs_cam_core::compute::toolpath_stats::ToolpathStats;
    let stats = ToolpathStats {
        model_gouge: Some(ModelGougeFinding {
            rule: ModelGougeRule::Surface {
                fix: "Set the floor to Each picked face",
            },
            judged: 100,
            gouging: 7,
            deepest_mm: 10.0,
            at: [38.9, -2.8, -2.0],
            surface_z: 8.0,
            move_index: 12,
            depth_top_z: None,
            below_surface_mm: 10.0,
            at_wall: false,
        }),
        ..ToolpathStats::default()
    };
    let (finding, danger) = crate::ui::toolpath_panel::warning_finding(
        Some(&stats),
        rs_cam_core::ToolpathId::from_raw(0),
    )
    .expect("a gouge warns");
    let hover = super::placement::warning_hover(&finding);
    let done = rs_cam_core::session::RunState::Done;
    let row = super::toolpath_panel::HeaderRow {
        run_state: Some(&done),
        can_run: true,
        manual_generation: false,
        waiting_detail: None,
        warning: Some((hover.as_str(), danger)),
        moves: Some(12_345),
        feed_stage: Some(rs_cam_core::session::FeedStage::ModulatedSuperseded {
            simulation_run: 3,
            current_run: Some(4),
        }),
        caps: None,
    };
    // (text, right edge, elided) of each painted text at `width`.
    let paint = |width: f32| -> Vec<(String, f32, bool)> {
        let ctx = egui::Context::default();
        crate::ui::tokens::apply(&ctx);
        crate::ui::tokens::apply_fonts(&ctx);
        let mut texts = Vec::new();
        for _ in 0..3 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.set_max_width(width);
                let _ = super::toolpath_panel::draw_generate_row(ui, &row, &[]);
            });
            texts = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::epaint::Shape::Text(t) => Some((
                        t.galley.job.text.clone(),
                        c.shape.visual_bounding_rect().max.x,
                        t.galley.elided,
                    )),
                    _ => None,
                })
                .collect();
            out.textures_delta.clear();
        }
        texts
    };
    let blocked = crate::ui::toolpath_panel::done_header(danger);
    for width in [280.0, 200.0] {
        let texts = paint(width);
        let word: Vec<&(String, f32, bool)> = texts.iter().filter(|t| t.0 == blocked).collect();
        assert_eq!(word.len(), 1, "at {width}: one state word: {texts:?}");
        assert!(
            !word[0].2,
            "at {width}: the state word is not elided: {texts:?}"
        );
        let wide: Vec<&(String, f32, bool)> = texts.iter().filter(|t| t.1 > width + 1.0).collect();
        assert!(
            wide.is_empty(),
            "at {width}: nothing past the row: {wide:?}"
        );
        if width < 250.0 {
            assert!(
                !texts.iter().any(|t| t.0.ends_with("moves")),
                "at {width}: the move count drops first: {texts:?}"
            );
        }
    }
}

/// G-B: the Generate row's figures hover names the feed caps of the
/// stored result: the count and the lowest cap, as planned moves.
#[test]
fn the_figures_hover_names_the_feed_caps_g_b() {
    use crate::state::runtime::CapFigures;
    use rs_cam_core::geo::P3;
    use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};
    let mut tp = Toolpath::new();
    assert_eq!(CapFigures::of(&tp), None, "no move, no caps");
    for (x, cap) in [(1.0, None), (2.0, Some(1500.0)), (3.0, Some(1200.0))] {
        tp.moves.push(Move {
            target: P3::new(x, 0.0, 0.0),
            move_type: MoveType::Linear { feed_rate: 2000.0 },
            intent: MoveIntent::Unknown,
            feed_cap: cap,
        });
    }
    let caps = CapFigures::of(&tp).expect("two capped moves");
    assert_eq!(
        caps.line(),
        "2 moves at a load cap (planned), lowest 1200 mm/min"
    );

    // G-B2: the runtime memo counts once per result and follows a new one.
    let result = |tp: Toolpath| {
        crate::state::runtime::ToolpathRun::Done(crate::state::toolpath::ToolpathResult {
            annotated: std::sync::Arc::new(
                rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(tp),
            ),
            stats: Default::default(),
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
            drill_op: None,
            lineage: std::sync::Arc::new(rs_cam_core::trace::lineage::Lineage::unrecorded()),
        })
    };
    let mut rt = crate::state::runtime::ToolpathRuntime::new(false);
    assert_eq!(rt.caps(), None, "no result, no caps");
    rt.run = result(tp.clone());
    assert_eq!(rt.caps(), Some(caps));
    assert_eq!(rt.caps(), Some(caps), "the memo answers the second read");
    rt.run = result(Toolpath::new());
    assert_eq!(rt.caps(), None, "a new result with no cap clears the memo");
}

/// R-E (operator ruling 2026-10-23): one hole is ONE row, with one text,
/// on every surface. A default Drill on `block_with_blind_hole.step` (one
/// Ø6 hole, rim Z 20, floor Z 12):
///
/// - the GUI Geometry tab list (`list_rows`) gives one row,
///   "Ø6 hole, 8 deep";
/// - the MCP `selection_status` reply gives one `rows` entry with that
///   `hole.label` and one `holes` entry with that `label`;
/// - the MCP `list_model_items` reply gives one circle row with that
///   `hole.label`, and no row for the floor circle;
/// - the depth number is the same on each surface (8).
///
/// Red proof (run by hand, 2026-10-23): with the fold of the floor circle
/// off (`fold_hole_floors` returns `rows`), `list_model_items` gives two
/// circle rows.
#[test]
fn one_hole_is_one_row_re() {
    use rs_cam_core::compute::StockConfig;
    use rs_cam_core::compute::catalog::OperationConfig;
    use rs_cam_core::compute::operation_configs::DrillConfig;
    use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
    use rs_cam_core::session::{ProjectSessionBuilder, SelectionTarget, ToolpathConfig};
    const LABEL: &str = "\u{00D8}6 hole, 8 deep";
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/step/block_with_blind_hole.step");
    let model = rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap();
    let mut tool = rs_cam_core::compute::tool_config::ToolConfig::test_default(
        rs_cam_core::compute::ToolId::from_raw(1),
        rs_cam_core::compute::tool_config::ToolType::EndMill,
    );
    tool.diameter = 6.0;
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig {
            x: 50.0,
            y: 40.0,
            z: Some(25.0),
            origin_x: -5.0,
            origin_y: -5.0,
            origin_z: 0.0,
            auto_from_model: false,
            ..StockConfig::default()
        })
        .tool(tool);
    let model_id = builder.add_model(model);
    let _ = builder
        .add_toolpath(
            0,
            ToolpathConfig {
                shoe: Default::default(),
                feeds_lock: Default::default(),
                id: rs_cam_core::compute::ToolpathId::from_raw(0),
                name: "re drill".to_owned(),
                enabled: true,
                operation: OperationConfig::Drill(DrillConfig::default()),
                dressups: Default::default(),
                heights: Default::default(),
                tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
                model_id,
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
            },
        )
        .unwrap();
    let session = builder.build();

    // GUI: the Geometry tab list.
    let (rows, _) = rows_of(&session);
    let rows: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
    assert_eq!(rows, vec![LABEL], "the GUI list: one row per hole");

    // MCP: selection_status.
    let status =
        crate::app::mcp::commands::selection_status_reply(&session, 0, SelectionTarget::Op);
    let status_rows = status["rows"].as_array().unwrap();
    assert_eq!(status_rows.len(), 1, "{status}");
    assert_eq!(status_rows[0]["hole"]["label"], serde_json::json!(LABEL));
    assert_eq!(status_rows[0]["hole"]["depth_mm"], serde_json::json!(8.0));
    let holes = status["holes"].as_array().unwrap();
    assert_eq!(holes.len(), 1, "{status}");
    assert_eq!(holes[0]["label"], serde_json::json!(LABEL));
    assert_eq!(holes[0]["depth_mm"], serde_json::json!(8.0));

    // MCP: list_model_items, the circles only.
    let listed = crate::app::mcp::commands::list_model_items_reply(
        &session,
        &rs_cam_mcp::server::ListModelItemsParam {
            model_id: model_id.raw(),
            layer: None,
            kind: Some(rs_cam_mcp::server::ItemKindParam::Circle),
            near: None,
        },
    );
    let items = listed["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        1,
        "the floor circle folds into its rim: {listed}"
    );
    assert_eq!(items[0]["hole"]["label"], serde_json::json!(LABEL));
    assert_eq!(items[0]["hole"]["depth_mm"], serde_json::json!(8.0));
    assert!(
        items[0]["hole"]["floor_key"].is_object(),
        "the rim row names its floor circle: {listed}"
    );
}

/// R-A (operator ruling 2026-10-23): the part outline row has one name on
/// every surface. A default Profile (side Outside) on `stepped_block.step`:
///
/// - the GUI Geometry tab list (`list_rows`) gives one row,
///   "Part outline Z0" (the drawing rule of the config default);
/// - the MCP `selection_status` reply gives one `rows` entry with `role`
///   `"part_outline"`;
/// - the MCP `list_model_items` reply gives one row with that `role`, the
///   last row, and every other row gives `role: null`.
///
/// Red proof (run by hand, 2026-10-23): with `row_of` back on
/// `row_label` for the outline, the GUI row reads "Loop Z0 49.0 mm".
#[test]
fn the_part_outline_row_reads_part_outline_ra() {
    use rs_cam_core::compute::StockConfig;
    use rs_cam_core::compute::catalog::OperationConfig;
    use rs_cam_core::compute::operation_configs::ProfileConfig;
    use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
    use rs_cam_core::session::{ProjectSessionBuilder, SelectionTarget, ToolpathConfig};
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/step/stepped_block.step");
    let model = rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap();
    let mut tool = rs_cam_core::compute::tool_config::ToolConfig::test_default(
        rs_cam_core::compute::ToolId::from_raw(1),
        rs_cam_core::compute::tool_config::ToolType::EndMill,
    );
    tool.diameter = 6.0;
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig {
            x: 70.0,
            y: 50.0,
            z: Some(30.0),
            origin_x: -5.0,
            origin_y: -5.0,
            origin_z: 0.0,
            auto_from_model: false,
            ..StockConfig::default()
        })
        .tool(tool);
    let model_id = builder.add_model(model);
    let _ = builder
        .add_toolpath(
            0,
            ToolpathConfig {
                shoe: Default::default(),
                feeds_lock: Default::default(),
                id: rs_cam_core::compute::ToolpathId::from_raw(0),
                name: "ra profile".to_owned(),
                enabled: true,
                operation: OperationConfig::Profile(ProfileConfig::default()),
                dressups: Default::default(),
                heights: Default::default(),
                tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
                model_id,
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
            },
        )
        .unwrap();
    let session = builder.build();

    // GUI: the Geometry tab list.
    let (rows, _) = rows_of(&session);
    let rows: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(rows, vec!["Part outline Z0"], "the GUI list");

    // MCP: selection_status.
    let status =
        crate::app::mcp::commands::selection_status_reply(&session, 0, SelectionTarget::Op);
    let status_rows = status["rows"].as_array().unwrap();
    assert_eq!(status_rows.len(), 1, "{status}");
    assert_eq!(status_rows[0]["role"], serde_json::json!("part_outline"));

    // MCP: list_model_items.
    let listed = crate::app::mcp::commands::list_model_items_reply(
        &session,
        &rs_cam_mcp::server::ListModelItemsParam {
            model_id: model_id.raw(),
            layer: None,
            kind: None,
            near: None,
        },
    );
    let items = listed["items"].as_array().unwrap();
    let roles: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, i)| i["role"] == serde_json::json!("part_outline"))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(
        roles,
        vec![items.len() - 1],
        "one outline row, last: {listed}"
    );
    assert!(
        items[..items.len() - 1].iter().all(|i| i["role"].is_null()),
        "{listed}"
    );
}

/// R2 / 6.F4 (Fusion, operator ruling 2026-10-23): the Tool combo greys a
/// tool that the op refuses, by core `ProjectSession::tool_fit` (the
/// acceptance door of the add). R2 (2026-10-30): the combo is the header
/// Tool row; with no tool it reads "Choose a tool", and the Passes tab
/// draws no tool row and no no-tool line.
///
/// Red proof: build `tool_refusals` as an empty list in
/// `toolpath_panel_inputs`. The first assertion fails.
#[test]
fn the_tool_picker_greys_refused_tools_r2() {
    use crate::state::toolpath::{OperationType, ToolpathEntry};
    use rs_cam_core::compute::tool_config::{ToolConfig, ToolType};
    use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
    use rs_cam_core::session::{AddToolpathArgs, Command, LoadedModel, ProjectSessionBuilder};
    use std::sync::Arc;

    let v_bit = ToolConfig::test_default(rs_cam_core::compute::ToolId::from_raw(0), ToolType::VBit);
    let end_mill =
        ToolConfig::test_default(rs_cam_core::compute::ToolId::from_raw(1), ToolType::EndMill);
    let circle = Item::new(
        SourceRef::ToolpathModel,
        None,
        Shape::circle(rs_cam_core::geo::P2::new(0.0, 0.0), 10.0),
    )
    .unwrap();
    let mut session = ProjectSessionBuilder::new()
        .tool(v_bit.clone())
        .tool(end_mill.clone())
        .model(LoadedModel {
            id: rs_cam_core::ids::ModelId::from_raw(1),
            path: std::path::PathBuf::from("holes.svg"),
            name: "Holes".to_owned(),
            kind: Some(rs_cam_core::compute::stock_config::ModelKind::Svg),
            mesh: None,
            items: Arc::new(ItemSet::new(vec![circle])),
            enriched_mesh: None,
            units: Some(rs_cam_core::compute::stock_config::ModelUnits::Millimeters),
            winding_report: None,
            load_error: None,
            import_options: Default::default(),
            import_report: None,
            drawing: Default::default(),
        })
        .build();
    let mut config = session
        .draft_toolpath(rs_cam_core::session::DraftRequest::new(
            OperationType::Bore,
            0,
            "Bore",
        ))
        .unwrap()
        .config;
    assert_eq!(config.tool_id, Some(end_mill.id), "the add skips the V-bit");
    // The state of an op with no tool (a file can hold it).
    config.tool_id = None;
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(config),
        }))
        .unwrap();
    let tc = session.toolpath_configs()[0].clone();
    let inputs = super::toolpath_panel_inputs(
        tc.id,
        &session,
        &crate::state::runtime::GuiState::default(),
        None,
        Some(ToolpathTab::Passes),
        false,
    );
    let greyed: Vec<_> = inputs.tool_refusals.iter().map(|(id, _)| *id).collect();
    assert_eq!(greyed, vec![v_bit.id], "only the V-bit is grey");
    let text = super::tab_badges::tool_refusal(&inputs, v_bit.id).unwrap();
    assert!(text.contains("Bore"), "the hover names the refusal: {text}");
    assert!(super::tab_badges::tool_refusal(&inputs, end_mill.id).is_none());
    // A fitting tool exists, so the state is "No tool set".
    assert_eq!(inputs.no_tool_state.as_deref(), Some("No tool set"));

    let mut entry = ToolpathEntry::for_operation(
        tc.id,
        "Bore".to_owned(),
        end_mill.id,
        tc.model_id,
        OperationType::Bore,
    );
    entry.tool_id = None;
    let ctx = egui::Context::default();
    crate::ui::tokens::apply(&ctx);
    crate::ui::tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    let mut events = Vec::new();
    let slots = session.suggest_slots(&tc);
    let mut painted = |entry: &mut ToolpathEntry, passes: bool| {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            if passes {
                super::toolpath_panel::draw_passes_tab(
                    ui,
                    entry,
                    &inputs,
                    None,
                    &mut crate::state::pick::SelectionUi::default(),
                    super::linking_dressup::Fields::default(),
                    &slots,
                    &mut crate::state::panels::EnvelopeCandidates::default(),
                    &mut events,
                );
            } else {
                super::tab_badges::draw_tool_row(ui, entry, &inputs, &[], &mut events);
            }
        });
        let mut texts = Vec::new();
        for clipped in &out.shapes {
            if let egui::epaint::Shape::Text(text) = &clipped.shape {
                texts.push(text.galley.job.text.clone());
            }
        }
        out.textures_delta.clear();
        texts
    };
    let header = painted(&mut entry, false);
    assert!(
        header.iter().any(|t| t == "Tool")
            && header.iter().any(|t| t == super::tab_badges::CHOOSE_A_TOOL),
        "the header Tool row reads `Choose a tool`; it rendered {header:?}"
    );
    let passes = painted(&mut entry, true);
    let old_line = format!("No tool set. {}", rs_cam_core::session::CHOOSE_A_TOOL_FIX);
    assert!(
        !passes
            .iter()
            .any(|t| t == "Tool:" || t == &old_line || t == super::tab_badges::CHOOSE_A_TOOL),
        "the Passes tab holds no tool control and no no-tool line: {passes:?}"
    );
}

/// G-D (§3): the Suggest advice to use Geodesic Adaptive is one ⚡ chip in
/// the Strategy row, not a text row. A click writes the strategy to the
/// draft (the panel commits it as one `ReplaceToolpathConfig`). With no
/// advice, or when the draft already holds it, there is no chip.
#[test]
fn the_strategy_row_carries_the_suggest_chip_g_d() {
    use crate::state::toolpath::{Adaptive3dConfig, ClearingStrategy};
    use rs_cam_core::feeds::suggest::SuggestWarning;
    let warning = SuggestWarning::StrategyRecommendedNotApplied {
        param: "clearing_strategy",
        current: "contour_parallel".to_owned(),
        recommended: "geodesic_adaptive".to_owned(),
        reason: "mixed_terrain_classifier",
    };
    let suggestion = super::toolpath_panel::strategy_suggestion(Some(&[warning]));
    assert_eq!(suggestion, Some(ClearingStrategy::GeodesicAdaptive));
    assert_eq!(super::toolpath_panel::strategy_suggestion(Some(&[])), None);

    // (the chips painted, the strategy after the frames) for one run.
    let run = |strategy: ClearingStrategy, suggestion, click: bool| {
        let ctx = egui::Context::default();
        crate::ui::tokens::apply(&ctx);
        crate::ui::tokens::apply_fonts(&ctx);
        let mut cfg = Adaptive3dConfig {
            clearing_strategy: strategy,
            ..Adaptive3dConfig::default()
        };
        let mut chip_at: Option<egui::Pos2> = None;
        let mut chips = 0;
        for pass in 0..5 {
            let mut input = egui::RawInput::default();
            if click && let Some(pos) = chip_at {
                match pass {
                    3 => input.events.push(egui::Event::PointerMoved(pos)),
                    4 => {
                        for pressed in [true, false] {
                            input.events.push(egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            });
                        }
                    }
                    _ => {}
                }
            }
            let mut out = ctx.run_ui(input, |ui| {
                ui.set_max_width(264.0);
                super::operations::draw_adaptive3d_params(
                    ui,
                    &mut cfg,
                    None,
                    suggestion,
                    false,
                    false,
                    Default::default(),
                );
            });
            if pass == 2 {
                for clipped in &out.shapes {
                    if let egui::epaint::Shape::Text(text) = &clipped.shape
                        && text.galley.job.text == "\u{26A1}"
                    {
                        chips += 1;
                        chip_at = Some(clipped.shape.visual_bounding_rect().center());
                    }
                }
            }
            out.textures_delta.clear();
        }
        (chips, cfg.clearing_strategy)
    };
    let cp = ClearingStrategy::ContourParallel;
    let ga = ClearingStrategy::GeodesicAdaptive;
    // The two ⚡ pills of Stepover and Depth/Pass need pill data; with
    // `pills: None` the only ⚡ is the Strategy chip.
    assert_eq!(run(cp, None, false), (0, cp), "no advice, no chip");
    assert_eq!(
        run(ga, Some(ga), false),
        (0, ga),
        "the draft holds it: no chip"
    );
    assert_eq!(run(cp, Some(ga), false), (1, cp), "one chip, no write");
    assert_eq!(run(cp, Some(ga), true), (1, ga), "a click writes the draft");
}

/// G-D (ruling D-1): with the boundary on, the Geometry tab draws the
/// Boundary row (W3a: the switch and the source in one combo) and no
/// `Boundary: …` summary row above it; the summary is the row hover.
#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn the_boundary_summary_is_the_source_hover_d1() {
    use crate::state::toolpath::{BoundarySource, OperationType, ToolpathEntry};
    let (session, _mesh) = one_pocket_session();
    let tc = &session.toolpath_configs()[0];
    let inputs = super::toolpath_panel_inputs(
        tc.id,
        &session,
        &crate::state::runtime::GuiState::default(),
        None,
        Some(ToolpathTab::Passes),
        false,
    );
    let mut entry = ToolpathEntry::for_operation(
        tc.id,
        "Case".to_owned(),
        tc.tool_id.expect("the case tool"),
        tc.model_id,
        OperationType::Pocket,
    );
    entry.boundary.enabled = true;
    entry.boundary.source = BoundarySource::Stock;
    let ctx = egui::Context::default();
    crate::ui::tokens::apply(&ctx);
    crate::ui::tokens::apply_fonts(&ctx);
    let mut texts = Vec::new();
    for _ in 0..2 {
        let mut events = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            super::toolpath_panel::draw_geometry_tab(
                ui,
                &mut entry,
                &inputs,
                &mut crate::state::pick::SelectionUi::default(),
                &mut crate::state::panels::ContainmentAutoCorrect::default(),
                &mut super::toolpath_panel::ReachView::default(),
                &[],
                &mut events,
            );
        });
        texts = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.job.text.clone()),
                _ => None,
            })
            .collect();
        out.textures_delta.clear();
    }
    assert!(
        texts.iter().any(|t| t == "Boundary") && texts.iter().any(|t| t == "Stock"),
        "non-vacuity: the Boundary row draws its source: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t.starts_with("Boundary:")),
        "the summary row is back: {texts:?}"
    );
}

/// W3e (F5): an op that plans its own entries (3D Rough) draws no dressup
/// Entry style row, so the inspector never shows a second Entry style
/// control. A Pocket draws it. "Lead-in / out" draws for both (the anchor).
#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn the_entry_style_row_hides_where_the_op_owns_the_entry_w3e() {
    use crate::state::toolpath::{OperationType, ToolpathEntry};
    let (session, _mesh) = one_pocket_session();
    let tc = &session.toolpath_configs()[0];
    let texts_for = |op: OperationType| {
        let mut entry = ToolpathEntry::for_operation(
            tc.id,
            "Case".to_owned(),
            tc.tool_id.expect("the case tool"),
            tc.model_id,
            op,
        );
        let ctx = egui::Context::default();
        crate::ui::tokens::apply(&ctx);
        crate::ui::tokens::apply_fonts(&ctx);
        let mut texts: Vec<String> = Vec::new();
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                super::linking_dressup::draw_linking_params(
                    ui,
                    &mut entry,
                    None,
                    None,
                    super::linking_dressup::Fields::default(),
                );
            });
            texts = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::epaint::Shape::Text(t) => Some(t.galley.job.text.clone()),
                    _ => None,
                })
                .collect();
            out.textures_delta.clear();
        }
        texts
    };
    let pocket = texts_for(OperationType::Pocket);
    let rough = texts_for(OperationType::Adaptive3d);
    for texts in [&pocket, &rough] {
        assert!(
            texts.iter().any(|t| t == "Lead-in / out"),
            "non-vacuity: the tab draws: {texts:?}"
        );
    }
    assert!(pocket.iter().any(|t| t == "Entry style"), "{pocket:?}");
    assert!(
        !rough.iter().any(|t| t.contains("Entry style")),
        "{rough:?}"
    );
}

/// G-FEEDMAX (C2): the Max Rate row caps every feed stage, so the Linking
/// tab's Advanced section draws it with "Feed rate optimization" off. The
/// Ramp Rate row belongs to the optimiser and stays hidden.
#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn the_max_rate_row_shows_without_feed_optimization_g_feedmax() {
    use crate::state::toolpath::{OperationType, ToolpathEntry};
    let (session, _mesh) = one_pocket_session();
    let tc = &session.toolpath_configs()[0];
    let mut entry = ToolpathEntry::for_operation(
        tc.id,
        "Case".to_owned(),
        tc.tool_id.expect("the case tool"),
        tc.model_id,
        OperationType::Pocket,
    );
    entry.dressups.feed_optimization = false;
    let ctx = egui::Context::default();
    crate::ui::tokens::apply(&ctx);
    crate::ui::tokens::apply_fonts(&ctx);
    let mut advanced_at: Option<egui::Pos2> = None;
    let mut texts: Vec<String> = Vec::new();
    // W3e: the section opens with an animation, and the frames have no
    // time, so the test runs enough frames for the open to end.
    for pass in 0..20 {
        let mut input = egui::RawInput::default();
        if pass == 2
            && let Some(pos) = advanced_at
        {
            input.events.push(egui::Event::PointerMoved(pos));
            for pressed in [true, false] {
                input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        let mut out = ctx.run_ui(input, |ui| {
            super::linking_dressup::draw_linking_advanced(
                ui,
                &mut entry,
                super::linking_dressup::Fields::default(),
            );
        });
        texts = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::epaint::Shape::Text(t) => {
                    if t.galley.job.text == "Advanced" {
                        advanced_at = Some(c.shape.visual_bounding_rect().center());
                    }
                    Some(t.galley.job.text.clone())
                }
                _ => None,
            })
            .collect();
        out.textures_delta.clear();
    }
    assert!(
        texts.iter().any(|t| t == "Feed optimization"),
        "non-vacuity: the Advanced section is open: {texts:?}"
    );
    assert!(texts.iter().any(|t| t == "Max rate"), "{texts:?}");
    assert!(!texts.iter().any(|t| t.contains("Ramp rate")), "{texts:?}");
}

/// G-E (3): the face of each rest-boundary refusal, and the `⟳` action
/// glyph is in the app fonts (a missing glyph paints a box).
#[test]
fn the_rest_refusal_faces_are_short_g_e() {
    use super::toolpath_panel::issues::{GENERATE_SOURCE_GLYPH, rest_refusal_face};
    use rs_cam_core::session::RestBoundaryRefusal;
    let source = rs_cam_core::ToolpathId::from_raw(7);
    let name = "Body rough".to_owned();
    let not_generated = rest_refusal_face(&RestBoundaryRefusal::NotGenerated {
        source_toolpath_id: source,
        source_name: name.clone(),
    })
    .expect("a face");
    assert_eq!(not_generated.text, "Generate 'Body rough' first");
    assert_eq!(not_generated.generate, Some((source, name.clone())));
    let no_regions = rest_refusal_face(&RestBoundaryRefusal::NoRegions {
        source_toolpath_id: source,
        source_name: name,
    })
    .expect("a face");
    assert_eq!(no_regions.text, "'Body rough' left no regions");
    let own = rest_refusal_face(&RestBoundaryRefusal::OwnRegions).expect("a face");
    assert_eq!(own.text, "The source is this op");
    assert_eq!(own.generate, None);
    assert_eq!(
        rest_refusal_face(&RestBoundaryRefusal::SourceMissing {
            source_toolpath_id: source
        }),
        None
    );

    let ctx = egui::Context::default();
    crate::ui::tokens::apply(&ctx);
    crate::ui::tokens::apply_fonts(&ctx);
    let mut out = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    out.textures_delta.clear();
    let glyph = GENERATE_SOURCE_GLYPH.chars().next().expect("one char");
    assert!(
        ctx.fonts_mut(|f| f.has_glyph(&egui::FontId::proportional(13.0), glyph)),
        "the app fonts have no {GENERATE_SOURCE_GLYPH}"
    );
}

/// A larger consumer of `source` (Ø10 against Ø6) with an `Inside` answer.
fn larger_consumer(
    source: rs_cam_core::ToolpathId,
    inside: crate::state::runtime::InsideAnswer,
) -> super::RestContainment {
    super::RestContainment {
        larger: rs_cam_core::session::LargerRestConsumer {
            op_name: "Edges".to_owned(),
            source_toolpath_id: source,
            source_name: "Body rough".to_owned(),
            tool_radius_mm: 5.0,
            source_tool_radius_mm: 3.0,
        },
        inside,
    }
}

/// G-C (D-2): the `Rest Regions` pick writes `Inside` with the `⟲` badge
/// when the tool is larger and `Inside` keeps area. It writes nothing when
/// `Inside` would collapse, when the answer is unknown, for another source,
/// or when the containment is `Inside` already. An edit clears the badge;
/// another toolpath drops it.
#[test]
fn the_rest_pick_sets_inside_with_a_badge_d2() {
    use super::toolpath_panel::tabs::geometry::pick_sets_inside;
    use crate::state::runtime::InsideAnswer;
    use crate::state::toolpath::BoundaryContainment;
    let source = rs_cam_core::ToolpathId::from_raw(3);
    let other = rs_cam_core::ToolpathId::from_raw(4);
    let mut badge = crate::state::panels::ContainmentAutoCorrect::default();
    badge.scope(rs_cam_core::ToolpathId::from_raw(9));

    let keeps = larger_consumer(source, InsideAnswer::Keeps);
    let mut containment = BoundaryContainment::Center;
    assert!(pick_sets_inside(
        &mut containment,
        Some(&keeps),
        source,
        &mut badge
    ));
    assert_eq!(containment, BoundaryContainment::Inside);
    assert_eq!(
        badge.hover(),
        Some(
            "Set to Inside: this tool (\u{00D8}10.00) is larger than the tool of 'Body rough' (\u{00D8}6.00)."
        )
    );
    badge.clear();
    assert_eq!(badge.hover(), None, "an edit clears the badge");

    for (pick, to) in [
        (
            larger_consumer(source, InsideAnswer::Collapses("x".to_owned())),
            source,
        ),
        (
            larger_consumer(source, InsideAnswer::Unknown("y".to_owned())),
            source,
        ),
        (keeps.clone(), other),
    ] {
        let mut containment = BoundaryContainment::Outside;
        assert!(!pick_sets_inside(
            &mut containment,
            Some(&pick),
            to,
            &mut badge
        ));
        assert_eq!(containment, BoundaryContainment::Outside);
    }
    let mut containment = BoundaryContainment::Center;
    assert!(!pick_sets_inside(
        &mut containment,
        None,
        source,
        &mut badge
    ));

    badge.set("h".to_owned());
    badge.scope(rs_cam_core::ToolpathId::from_raw(10));
    assert_eq!(badge.hover(), None, "another toolpath drops the badge");
}

/// G-C (§2 c, d): the refused-option hover names the sizes, the overrun
/// and the fix; under a collapse the fix is the smaller tool. The region
/// count reads `· N regions` (one phrase, §6).
#[test]
fn the_containment_hover_and_the_region_count_g_c() {
    use super::toolpath_panel::tabs::geometry::{regions_phrase, rest_source_label};
    use crate::state::runtime::InsideAnswer;
    let source = rs_cam_core::ToolpathId::from_raw(3);
    assert_eq!(
        larger_consumer(source, InsideAnswer::Keeps).hover(),
        "This tool (\u{00D8}10.00) is larger than the tool of 'Body rough' (\u{00D8}6.00). \
         It could cut 2.00 mm past that op's boundary. Use Inside."
    );
    assert!(
        larger_consumer(source, InsideAnswer::Collapses("c".to_owned()))
            .hover()
            .ends_with(
                "Inside leaves no area for this tool here: use a tool no larger than \u{00D8}6.00."
            )
    );
    let square = rs_cam_core::polygon::Polygon2::rectangle(0.0, 0.0, 1.0, 1.0);
    assert_eq!(
        rest_source_label("Body rough", Some(&vec![square.clone(); 14])),
        "Body rough \u{00B7} 14 regions"
    );
    assert_eq!(
        rest_source_label("Body rough", Some(&vec![square])),
        "Body rough \u{00B7} 1 region"
    );
    assert_eq!(
        rest_source_label("Body rough", Some(&Vec::new())),
        "Body rough"
    );
    assert_eq!(rest_source_label("Body rough", None), "Body rough");
    assert_eq!(regions_phrase(2), "2 regions");
}

/// G-C (§2 c, d): with a larger consumer on a `Center` containment, the
/// Containment row draws `Use Inside`, and a click writes `Inside` to the
/// draft. When `Inside` would collapse, the row draws no act (W3a). The two options `Center` and `Outside` are refused in
/// the combo (their hover is the sizes).
#[test]
fn use_inside_sits_on_the_containment_row_g_c() {
    use crate::state::runtime::InsideAnswer;
    use crate::state::toolpath::{
        BoundaryContainment, BoundarySource, OperationType, ToolpathEntry,
    };
    let source = rs_cam_core::ToolpathId::from_raw(42);
    let run = |inside: InsideAnswer| {
        let (session, _mesh) = one_pocket_session();
        let tc = &session.toolpath_configs()[0];
        let mut inputs = super::toolpath_panel_inputs(
            tc.id,
            &session,
            &crate::state::runtime::GuiState::default(),
            None,
            Some(ToolpathTab::Passes),
            false,
        );
        inputs.rest_containment = Some(larger_consumer(source, inside));
        inputs.boundary_source_candidates =
            vec![(source, "Body rough".to_owned(), false, None, None, None)];
        let mut entry = ToolpathEntry::for_operation(
            tc.id,
            "Edges".to_owned(),
            tc.tool_id.expect("the case tool"),
            tc.model_id,
            OperationType::Pocket,
        );
        entry.boundary.enabled = true;
        entry.boundary.source = BoundarySource::DerivedRestRegions {
            source_toolpath_id: source,
        };
        entry.boundary.containment = BoundaryContainment::Center;
        let ctx = egui::Context::default();
        crate::ui::tokens::apply(&ctx);
        crate::ui::tokens::apply_fonts(&ctx);
        let mut badge = crate::state::panels::ContainmentAutoCorrect::default();
        let mut act_at: Option<egui::Pos2> = None;
        let mut painted = false;
        for pass in 0..5 {
            let mut input = egui::RawInput::default();
            if pass == 3
                && let Some(pos) = act_at
            {
                input.events.push(egui::Event::PointerMoved(pos));
                for pressed in [true, false] {
                    input.events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
            let mut events = Vec::new();
            let mut out = ctx.run_ui(input, |ui| {
                super::toolpath_panel::draw_geometry_tab(
                    ui,
                    &mut entry,
                    &inputs,
                    &mut crate::state::pick::SelectionUi::default(),
                    &mut badge,
                    &mut super::toolpath_panel::ReachView::default(),
                    &[],
                    &mut events,
                );
            });
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape
                    && text.galley.job.text == super::toolpath_panel::tabs::geometry::USE_INSIDE
                {
                    painted = true;
                    act_at = Some(clipped.shape.visual_bounding_rect().center());
                }
            }
            out.textures_delta.clear();
        }
        (painted, entry.boundary.containment)
    };
    assert_eq!(
        run(InsideAnswer::Keeps),
        (true, BoundaryContainment::Inside),
        "the act writes Inside"
    );
    // W3a: a quiet action has no disabled form, so the row offers no act
    // when Inside collapses; the Danger hover says why.
    assert_eq!(
        run(InsideAnswer::Collapses("c".to_owned())),
        (false, BoundaryContainment::Center),
        "no act when Inside collapses"
    );
}

/// W1b follow-up: the Waterline row names its field, `helical`.
#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn the_waterline_row_reads_helical_w1b() {
    let mut cfg = crate::state::toolpath::WaterlineConfig::default();
    let ctx = egui::Context::default();
    crate::ui::tokens::apply(&ctx);
    crate::ui::tokens::apply_fonts(&ctx);
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        super::operations::draw_waterline_params(ui, &mut cfg, None, Default::default());
    });
    let texts: Vec<String> = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.job.text.clone()),
            _ => None,
        })
        .collect();
    out.textures_delta.clear();
    assert!(texts.iter().any(|t| t == "Helical"), "{texts:?}");
    assert!(!texts.iter().any(|t| t == "Continuous"), "{texts:?}");
    assert!(
        super::linking_dressup::help_for(
            crate::state::toolpath::OperationType::Waterline,
            "helical"
        )
        .is_some(),
        "the row's hover is the registry help"
    );
}

/// C4: the 3D Rough row "Contour each level" shows where it acts: a rest
/// rough with a machining boundary (`offered`), a By Area rough (C4b), or
/// while it is on. A Global body rough (fresh stock) draws no row. The
/// hover is the registry help.
#[test]
fn contour_each_level_shows_where_it_acts_c4() {
    use crate::state::toolpath::Adaptive3dConfig;
    use crate::state::toolpath::RegionOrdering;
    let rows = |offered: bool, on: bool, ordering: RegionOrdering| {
        let ctx = egui::Context::default();
        crate::ui::tokens::apply(&ctx);
        crate::ui::tokens::apply_fonts(&ctx);
        let mut cfg = Adaptive3dConfig {
            contour_each_level: on,
            region_ordering: ordering,
            ..Adaptive3dConfig::default()
        };
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            super::operations::draw_adaptive3d_params(
                ui,
                &mut cfg,
                None,
                None,
                offered,
                false,
                Default::default(),
            );
        });
        let n = out
            .shapes
            .iter()
            .filter(|c| {
                matches!(&c.shape, egui::epaint::Shape::Text(t)
                    if t.galley.job.text == "Contour each level")
            })
            .count();
        out.textures_delta.clear();
        n
    };
    let global = RegionOrdering::Global;
    assert_eq!(
        rows(false, false, global),
        0,
        "a Global body rough draws no row"
    );
    assert_eq!(
        rows(true, false, global),
        1,
        "a rest rough with a boundary draws it"
    );
    assert_eq!(
        rows(false, true, global),
        1,
        "an op with the option on keeps the row"
    );
    assert_eq!(
        rows(false, false, RegionOrdering::ByArea),
        1,
        "C4b: a By Area rough, fresh or rest, draws it"
    );
    assert!(
        super::linking_dressup::help_for(
            crate::state::toolpath::OperationType::Adaptive3d,
            "contour_each_level"
        )
        .is_some_and(|h| h.contains("Depth/Pass") && h.contains("By Area")),
        "the hover is the registry help"
    );
}

/// R7a (operator ruling 2026-10-30): the Cut-tab boundary source buttons
/// write the offset of THE source-change rule,
/// `BoundaryConfig::with_source`, which the viewport click
/// (`click_edit`) reads too.
///
/// - Model Silhouette (offset one tool diameter) → Selection: offset 0.
/// - Selection → Model Silhouette: offset one tool diameter again
///   (`BoundaryConfig::for_3d_op`).
///
/// Red proof: the old Selection button wrote the source alone, so the
/// diameter offset stayed.
#[test]
fn the_selection_source_button_drops_the_diameter_offset_r7a() {
    use crate::state::toolpath::{BoundarySource, OperationType, ToolpathEntry};
    use rs_cam_core::compute::config::BoundaryConfig;

    /// Draw the Passes tab until `label` paints, click it on the next pass,
    /// and draw two more passes. Panics when `label` never paints.
    fn click_label(
        ctx: &egui::Context,
        entry: &mut ToolpathEntry,
        inputs: &super::ToolpathPanelInputs,
        label: &str,
    ) {
        let mut at: Option<egui::Pos2> = None;
        let mut clicked = false;
        for _ in 0..8 {
            let mut input = egui::RawInput::default();
            if let Some(pos) = at.filter(|_| !clicked) {
                clicked = true;
                input.events.push(egui::Event::PointerMoved(pos));
                for pressed in [true, false] {
                    input.events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
            let mut events = Vec::new();
            let mut out = ctx.run_ui(input, |ui| {
                super::toolpath_panel::draw_geometry_tab(
                    ui,
                    entry,
                    inputs,
                    &mut crate::state::pick::SelectionUi::default(),
                    &mut crate::state::panels::ContainmentAutoCorrect::default(),
                    &mut super::toolpath_panel::ReachView::default(),
                    &[],
                    &mut events,
                );
            });
            if !clicked {
                for shape in &out.shapes {
                    if let egui::epaint::Shape::Text(text) = &shape.shape
                        && text.galley.job.text == label
                    {
                        at = Some(shape.shape.visual_bounding_rect().center());
                    }
                }
            }
            out.textures_delta.clear();
        }
        assert!(clicked, "the label {label:?} never painted");
    }

    let (session, _mesh) = one_pocket_session();
    let tc = &session.toolpath_configs()[0];
    let mut inputs = super::toolpath_panel_inputs(
        tc.id,
        &session,
        &crate::state::runtime::GuiState::default(),
        None,
        Some(ToolpathTab::Passes),
        false,
    );
    // The flat STL holds no drawing item and no face; the button is drawn
    // enabled here so the click reaches it.
    inputs.boundary_selection_enabled = true;
    let tool_id = tc.tool_id.expect("the case tool");
    let diameter = inputs
        .tools
        .iter()
        .find(|(id, _, _)| *id == tool_id)
        .map(|(_, _, d)| *d)
        .expect("the case tool is in the inputs");
    assert!(diameter > 0.0, "non-vacuity: the tool has a diameter");
    let mut entry = ToolpathEntry::for_operation(
        tc.id,
        "Case".to_owned(),
        tool_id,
        tc.model_id,
        OperationType::Pocket,
    );
    entry.boundary = BoundaryConfig::for_3d_op(diameter);

    let ctx = egui::Context::default();
    crate::ui::tokens::apply(&ctx);
    crate::ui::tokens::apply_fonts(&ctx);

    // The combo shows the stored source; the click opens it.
    click_label(&ctx, &mut entry, &inputs, "Model silhouette");
    click_label(&ctx, &mut entry, &inputs, "Selection");
    assert!(
        matches!(entry.boundary.source, BoundarySource::Selection { .. }),
        "the click sets the source to Selection: {:?}",
        entry.boundary.source
    );
    assert_eq!(entry.boundary.offset, 0.0, "Selection takes offset 0");
    assert!(entry.boundary.enabled, "the boundary stays on");

    click_label(&ctx, &mut entry, &inputs, "Selection");
    click_label(&ctx, &mut entry, &inputs, "Model silhouette");
    assert_eq!(
        entry.boundary,
        BoundaryConfig::for_3d_op(diameter),
        "Model Silhouette takes the tool diameter offset again"
    );
}

/// One op of `config` with `dressups` over one flat STL model, with one end
/// mill. The W3f drawn-field sentry draws each tab of it.
fn one_op_session(
    config: crate::state::toolpath::OperationConfig,
    dressups: rs_cam_core::compute::config::DressupConfig,
) -> rs_cam_core::session::ProjectSession {
    use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
    use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
    use std::sync::Arc;

    let tool = rs_cam_core::compute::tool_config::ToolConfig::test_default(
        rs_cam_core::compute::ToolId::from_raw(1),
        rs_cam_core::compute::tool_config::ToolType::EndMill,
    );
    let mut builder = ProjectSessionBuilder::new().tool(tool);
    let _ = builder.add_model(LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        path: std::path::PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_flat(40.0))),
        items: Default::default(),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    });
    let _ = builder
        .add_toolpath(
            0,
            ToolpathConfig {
                shoe: Default::default(),
                feeds_lock: Default::default(),
                id: rs_cam_core::compute::ToolpathId::from_raw(0),
                name: "Case".to_owned(),
                enabled: true,
                operation: config,
                dressups,
                heights: Default::default(),
                tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
                model_id: rs_cam_core::ids::ModelId::from_raw(0),
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
            },
        )
        .expect("the fixture toolpath is addable");
    builder.build()
}

/// The field keys that each tab of the real inspector draws for an op of
/// `config` with `dressups`: the key, and the tab whose rows read its
/// state (`placement::record_drawn`). Only the keys a finding can anchor
/// to (`known_field_names`) count.
fn painted_fields(
    config: &crate::state::toolpath::OperationConfig,
    dressups: &rs_cam_core::compute::config::DressupConfig,
) -> std::collections::BTreeMap<String, Vec<ToolpathTab>> {
    use rs_cam_core::diagnostics::presentation::known_field_names;
    let session = one_op_session(config.clone(), dressups.clone());
    let id = session.toolpath_configs()[0].id;
    let gui = crate::state::runtime::GuiState::default();
    let mut out: std::collections::BTreeMap<String, Vec<ToolpathTab>> = Default::default();
    for &tab in ToolpathTab::ALL {
        if !tab.applies(config) {
            continue;
        }
        let mut snapshot =
            super::toolpath_panel_snapshot(id, &session, &gui).expect("the case toolpath");
        let inputs = super::toolpath_panel_inputs(id, &session, &gui, None, Some(tab), false);
        let ctx = egui::Context::default();
        crate::ui::tokens::apply(&ctx);
        crate::ui::tokens::apply_fonts(&ctx);
        let keys = super::placement::record_drawn(|| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let mut events = Vec::new();
                let _ = super::toolpath_panel::draw_toolpath_panel(
                    ui,
                    &mut snapshot,
                    &inputs,
                    &mut super::toolpath_panel::ReachView::default(),
                    &mut crate::state::pick::SelectionUi::default(),
                    &mut crate::state::panels::HeightsRefusal::default(),
                    &mut crate::state::panels::ContainmentAutoCorrect::default(),
                    &mut crate::state::panels::EnvelopeCandidates::default(),
                    &mut events,
                );
            });
            out.textures_delta.clear();
        });
        for key in keys {
            if known_field_names().any(|k| k == key) {
                out.entry(key).or_default().push(tab);
            }
        }
    }
    out
}

/// W3f: THE op-aware placement (`placement::drawn_fields`) states the
/// field rows that the real inspector paints.
///
/// For every op, and for each drill cycle of both drill ops, the test draws
/// every tab headless and records the rows that read a field state
/// (`placement::record_drawn`). For each key a finding can anchor to
/// (`presentation::anchor_field_names`):
///
/// - a key in `drawn_fields` is painted, on the tab that `field_tab` names;
/// - a painted key is in `drawn_fields`.
///
/// A key whose row the op does not paint is not in the table, so its
/// finding goes to the issue stack and is never lost.
///
/// Red proof: add `peck_depth` for every cycle, or drop the `scallop_height`
/// exception of the Drop Cutter: the test names the op and the key.
#[test]
fn drawn_field_rows_match_the_painted_rows_w3f() {
    use crate::state::toolpath::{OperationConfig, OperationType};
    use rs_cam_core::compute::operation_configs::DrillCycleType;
    use rs_cam_core::diagnostics::presentation::anchor_field_names;

    let mut cases: Vec<(String, OperationConfig)> = OperationType::ALL
        .iter()
        .map(|op| (format!("{op:?}"), OperationConfig::new_default(*op)))
        .collect();
    for cycle in [
        DrillCycleType::Simple,
        DrillCycleType::Dwell,
        DrillCycleType::Peck,
        DrillCycleType::ChipBreak,
    ] {
        for op in [OperationType::Drill, OperationType::AlignmentPinDrill] {
            let mut config = OperationConfig::new_default(op);
            match &mut config {
                OperationConfig::Drill(c) => c.cycle = cycle,
                OperationConfig::AlignmentPinDrill(c) => c.cycle = cycle,
                OperationConfig::Face(_)
                | OperationConfig::Pocket(_)
                | OperationConfig::Profile(_)
                | OperationConfig::Adaptive(_)
                | OperationConfig::VCarve(_)
                | OperationConfig::Inlay(_)
                | OperationConfig::InlayPlug(_)
                | OperationConfig::Zigzag(_)
                | OperationConfig::Trace(_)
                | OperationConfig::Bore(_)
                | OperationConfig::Chamfer(_)
                | OperationConfig::DropCutter(_)
                | OperationConfig::Adaptive3d(_)
                | OperationConfig::Waterline(_)
                | OperationConfig::Pencil(_)
                | OperationConfig::Scallop(_)
                | OperationConfig::UnifiedFinish(_)
                | OperationConfig::SteepShallow(_)
                | OperationConfig::RampFinish(_)
                | OperationConfig::SpiralFinish(_)
                | OperationConfig::RadialFinish(_)
                | OperationConfig::HorizontalFinish(_)
                | OperationConfig::ProjectCurve(_) => {}
            }
            cases.push((format!("{op:?} {cycle:?}"), config));
        }
    }
    let anchored = anchor_field_names();
    let mut errors = Vec::new();
    let mut drawn_count = 0_usize;
    for (name, config) in &cases {
        let painted = painted_fields(config, &Default::default());
        let drawn = super::placement::drawn_fields(config);
        drawn_count += drawn.len();
        for key in &anchored {
            match (drawn.get(key), painted.get(*key)) {
                (Some(tab), Some(tabs)) if !tabs.contains(tab) => errors.push(format!(
                    "{name}: `{key}` is on {tab:?} in the table, painted on {tabs:?}"
                )),
                (Some(tab), None) => errors.push(format!(
                    "{name}: `{key}` is on {tab:?} in the table, and no row paints it"
                )),
                (None, Some(tabs)) => errors.push(format!(
                    "{name}: a row on {tabs:?} paints `{key}`, and the table does not hold it"
                )),
                _ => {}
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert!(
        drawn_count > 200 && anchored.len() > 10,
        "non-vacuity: {drawn_count} drawn rows, {} anchored keys",
        anchored.len()
    );
}

/// W3g (fix 1): a field whose shown value is in tool diameters never paints
/// the unit "mm". Linking "Max distance" painted "8 x D" with "mm": an
/// unset rule value shows its rule text, and that text is not a length.
/// The same row with a value set paints "mm".
///
/// Red proof: give `opt_row` the unit whatever the value: the unset arm
/// paints "mm" beside "8 x D" and fails.
#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn a_tool_diameter_value_never_shows_mm_w3g() {
    let paint = |value: Option<f64>| -> Vec<String> {
        let ctx = egui::Context::default();
        crate::ui::tokens::apply(&ctx);
        crate::ui::tokens::apply_fonts(&ctx);
        let mut texts = Vec::new();
        for _ in 0..2 {
            let mut v = value;
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.set_max_width(340.0);
                super::linking_dressup::dv_dressup_rule(
                    ui,
                    "link_max_distance",
                    "Max distance",
                    &mut v,
                    " mm",
                    0.5,
                    1.0..=50.0,
                    "8 x D",
                    super::linking_dressup::Fields::default(),
                );
            });
            texts = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::epaint::Shape::Text(t) => Some(t.galley.job.text.clone()),
                    _ => None,
                })
                .collect();
            out.textures_delta.clear();
        }
        texts
    };
    let unset = paint(None);
    assert!(
        unset.iter().any(|t| t.contains("x D")),
        "non-vacuity: the rule text paints: {unset:?}"
    );
    assert!(
        !unset.iter().any(|t| t.contains("mm")),
        "a value in tool diameters paints \"mm\": {unset:?}"
    );
    let set = paint(Some(20.0));
    assert!(
        set.iter().any(|t| t == "mm"),
        "a set length paints its unit: {set:?}"
    );
}
