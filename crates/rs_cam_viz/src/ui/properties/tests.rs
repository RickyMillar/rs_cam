//! Unit tests for the inspector's tab enum and its diagnostic-row merge.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use super::ToolpathTab;
use super::tab_badges::merge_stateful_gate_rows;

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
        Some(ToolpathTab::FeedsSpeeds)
    ));
    assert!(matches!(
        ToolpathTab::parse("feeds_speeds"),
        Some(ToolpathTab::FeedsSpeeds)
    ));
    assert!(matches!(
        ToolpathTab::parse("linking"),
        Some(ToolpathTab::Linking)
    ));
    assert!(matches!(
        ToolpathTab::parse("heights"),
        Some(ToolpathTab::Heights)
    ));
    assert!(matches!(
        ToolpathTab::parse("dressup"),
        Some(ToolpathTab::Dressup)
    ));
    assert!(ToolpathTab::parse("not_a_tab").is_none());
    // UI-06: `key()` is the ONE key table. The hand-written match that used
    // to sit here was a third copy of the list, beside the parser and the
    // MCP refusal message. Every canonical key must name its own variant.
    assert!(
        ToolpathTab::ALL.len() >= 5,
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

fn stateful_diag(
    source: rs_cam_core::diagnostics::Source,
    message: &str,
) -> rs_cam_core::diagnostics::Diagnostic {
    use rs_cam_core::diagnostics as dx;
    dx::Diagnostic {
        id: dx::DiagnosticId::new("test.gate"),
        scope: dx::Scope::Toolpath {
            id: rs_cam_core::ToolpathId::from_raw(0),
        },
        category: dx::Category::State,
        severity: dx::Severity::Info,
        confidence: dx::Confidence::Static,
        state: dx::DiagnosticState::StaleEvidence,
        source,
        message: message.to_owned(),
        evidence: None,
        fix: None,
        supersedes: Vec::new(),
        suppressed_diagnostics: Vec::new(),
    }
}

/// Density pass V3 — three per-gate copies of the same stale-sim
/// sentence must collapse into one "Gates: …" row; gate rows with a
/// unique status and non-gate rows pass through untouched.
#[test]
fn stateful_gate_rows_with_shared_status_merge_into_one_gates_row() {
    use rs_cam_core::diagnostics::Source;
    let stale = "simulation stale — re-run to verify";
    let chipload = stateful_diag(Source::ToolLoad, &format!("Chipload: {stale}"));
    let power = stateful_diag(Source::ToolLoad, &format!("Power: {stale}"));
    let deflection = stateful_diag(Source::ToolLoad, &format!("Deflection: {stale}"));
    let unique_gate = stateful_diag(
        Source::ToolLoad,
        "Drill: no vendor LUT row matches this tool/material — supply vendor data \
         to enable this gate",
    );
    let non_gate = stateful_diag(Source::StaticValidation, "Heights: needs simulation");

    let stateful = vec![&chipload, &power, &deflection, &unique_gate, &non_gate];
    let (merged, rest) = merge_stateful_gate_rows(&stateful);

    assert_eq!(merged.len(), 1, "three shared-status gates → one row");
    assert_eq!(merged[0].message, format!("Gates: {stale}"));
    assert_eq!(rest.len(), 2, "unique gate + non-gate pass through");
    assert!(rest.iter().any(|d| d.message.starts_with("Drill:")));
    assert!(rest.iter().any(|d| d.message.starts_with("Heights:")));
}

/// Both wordings ship today — "run simulation to evaluate" (never
/// simulated) must merge independently of the stale wording.
#[test]
fn stateful_gate_rows_merge_groups_by_exact_status_text() {
    use rs_cam_core::diagnostics::Source;
    let chipload = stateful_diag(Source::ToolLoad, "Chipload: run simulation to evaluate");
    let power = stateful_diag(Source::ToolLoad, "Power: run simulation to evaluate");
    let deflection = stateful_diag(
        Source::ToolLoad,
        "Deflection: simulation stale — re-run to verify",
    );

    let stateful = vec![&chipload, &power, &deflection];
    let (merged, rest) = merge_stateful_gate_rows(&stateful);

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].message, "Gates: run simulation to evaluate");
    assert_eq!(rest.len(), 1, "differently-worded gate stays separate");
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

    let tool = rs_cam_core::compute::tool_config::ToolConfig::new_default(
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
    let carried = snapshot
        .model_bbox
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
        inputs.project_default_rpm,
        session.post_config().spindle_speed,
        "the project default rpm must come from the post config"
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
/// D7 (reference audit 2026-10-23, operator approved): the tab strip
/// draws Geometry, Cut, Heights, Feeds, Linking, Dressup, from
/// left to right. The test renders the strip and reads the x of each
/// label, so a change of `ALL` or of the strip order turns it red.
///
/// Red proof: with the old `ALL` order (Heights fifth), the drawn order
/// was Geometry, Cut, Feeds & Speeds, Linking, Heights, Dressup.
#[test]
fn the_tab_strip_puts_heights_after_cut_d7() {
    const WANT: [&str; 6] = ["Geometry", "Cut", "Heights", "Feeds", "Linking", "Dressup"];
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
        super::tab_badges::draw_toolpath_tabs(ui, &mut active, &badges);
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
        super::operations::draw_drill_params(ui, &mut drill, None, None);
    });

    let mut pin = AlignmentPinDrillConfig {
        cycle: DrillCycleType::Peck,
        ..Default::default()
    };
    let pin_texts = rendered(|ui| {
        super::operations::draw_alignment_pin_drill_params(ui, &mut pin, None, None);
    });

    for label in ["Peck (G83)", "Peck Depth:", "Feed Rate:", "Retract (R):"] {
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
            super::toolpath_panel::draw_cut_tab(
                ui,
                &mut entry,
                &inputs,
                None,
                None,
                &mut crate::state::pick::SelectionUi::default(),
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
    let pocket = rendered(OperationType::Pocket);
    assert!(
        pocket.iter().any(|t| t.contains("Machining Boundary")),
        "a Pocket must draw the boundary controls; it rendered {pocket:?}"
    );
    let pin = rendered(OperationType::AlignmentPinDrill);
    assert!(
        !pin.iter()
            .any(|t| t.contains("Machining Boundary") || t.contains("Enable boundary")),
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

/// Render the Geometry or the Cut tab of an entry of `op` against the
/// first toolpath of `session`, and give the painted texts in draw order.
/// The entry holds the stored op config when its type is `op`.
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
        ToolpathTab::Cut => super::toolpath_panel::draw_cut_tab(
            ui,
            &mut entry,
            &inputs,
            None,
            None,
            &mut crate::state::pick::SelectionUi::default(),
            &mut events,
        ),
        _ => super::geometry_tab::draw_geometry_tab(
            ui,
            &entry,
            &inputs,
            &mut crate::state::pick::SelectionUi::default(),
            &mut events,
        ),
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
    let tool = rs_cam_core::compute::tool_config::ToolConfig::new_default(
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

/// Geometry selection P4.D, fix 1: a Pocket on a STEP-only project with
/// nothing to cut (an empty selection; since UX-7 the default rule takes
/// the face rings) names the fix: pick a face. Red proof: `wants_a_face_pick`
/// returning false: the line was not painted.
#[test]
fn a_step_pocket_names_the_face_pick_fix_p4_d() {
    let session = step_pocket_session();
    let texts = geometry_tab_texts(&session, crate::state::toolpath::OperationType::Pocket);
    assert!(
        texts
            .iter()
            .any(|t| t == super::selection_section::PICK_A_FACE_FIX),
        "the section must name the face pick fix: {texts:?}"
    );
}

/// Geometry selection P4.D follow-up: the section shows ONE Pick button,
/// in the action row, in the refused state (the fix line names the fix
/// and has no button of its own) and after a face pick (no fix line).
/// Red proof: the fix line with its own `Pick` button again: the refused
/// state painted two.
#[test]
fn the_selection_section_shows_one_pick_p4_d() {
    // A button paints its label more than once in this harness, so the
    // test counts Pick against Reset, which the action row holds once.
    let count = |texts: &[String], label: &str| texts.iter().filter(|t| *t == label).count();

    let mut session = step_pocket_session();
    let refused = geometry_tab_texts(&session, crate::state::toolpath::OperationType::Pocket);
    assert!(
        refused
            .iter()
            .any(|t| t == super::selection_section::PICK_A_FACE_FIX),
        "fixture: the section is refused and names the fix: {refused:?}"
    );
    assert!(
        count(&refused, "Reset") > 0,
        "fixture: the action row paints Reset: {refused:?}"
    );
    assert_eq!(
        count(&refused, "Pick"),
        count(&refused, "Reset"),
        "the refused section must show one Pick: {refused:?}"
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
        !picked
            .iter()
            .any(|t| t == super::selection_section::PICK_A_FACE_FIX),
        "fixture: a face pick clears the fix line: {picked:?}"
    );
    assert_eq!(
        count(&picked, "Pick"),
        count(&picked, "Reset"),
        "the section must show one Pick: {picked:?}"
    );
}

/// Geometry selection P4.D fix 4, then GT-1: the op selection is the
/// Geometry tab, the first tab; its summary row ("Cuts") opens the tab. The
/// Cut tab holds Input, Start from and the Machining Boundary source, and
/// no Pick. A 3D op shows "Pick faces" in the Geometry tab (fix 2). Red
/// proof: the selection drawn in the Cut tab again: the Cut tab paints
/// "Pick".
#[test]
fn the_selection_section_is_at_the_top_p4_d() {
    let session = step_pocket_session();
    let geometry = geometry_tab_texts(&session, crate::state::toolpath::OperationType::Pocket);
    let cuts = position_of(&geometry, "Cuts row", |t| t.starts_with("Cuts"));
    let pick = position_of(&geometry, "Pick button", |t| t == "Pick");
    assert!(cuts < pick, "the summary opens the tab: {geometry:?}");
    let cut = tab_texts(
        &session,
        crate::state::toolpath::OperationType::Pocket,
        ToolpathTab::Cut,
    );
    let input = position_of(&cut, "Input row", |t| t.starts_with("Input"));
    let start_from = position_of(&cut, "Rest machining row", |t| {
        t == super::REST_MACHINING_LABEL
    });
    let boundary = position_of(&cut, "boundary header", |t| {
        t.eq_ignore_ascii_case("machining boundary")
    });
    assert!(
        input < start_from && start_from < boundary,
        "want Input < Rest machining < Machining Boundary: {cut:?}"
    );
    assert!(
        !cut.iter().any(|t| t == "Pick" || t.starts_with("Cuts")),
        "the Cut tab holds no selection: {cut:?}"
    );

    let rough = geometry_tab_texts(&session, crate::state::toolpath::OperationType::Adaptive3d);
    let _ = position_of(&rough, "Pick faces button", |t| {
        t == super::geometry_tab::PICK_FACES_LABEL
    });
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
    let mut tool = rs_cam_core::compute::tool_config::ToolConfig::new_default(
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
    builder.build()
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

/// The groups of the list of toolpath 0, as the tab builds them.
fn groups_of(
    session: &rs_cam_core::session::ProjectSession,
) -> (
    Vec<super::selection_section::ListGroup>,
    std::sync::Arc<rs_cam_core::session::SelectionReport>,
) {
    use rs_cam_core::session::SelectionTarget;
    let tc = &session.toolpath_configs()[0];
    let report = session.selection_status(tc, SelectionTarget::Op).unwrap();
    let read: super::SelectionRead = Ok(std::sync::Arc::clone(&report));
    let op_default = tc.operation.default_selection().unwrap();
    let kinds = std::collections::BTreeSet::new();
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
    };
    (
        super::selection_section::list_groups(&inputs, &report),
        report,
    )
}

/// GT-2 (L-R1, L-R2): every selected item is in one group only, so the
/// rows of the groups are the report rows.
///
/// - The default Drill: one default row, closed, with the default rule's
///   members.
/// - After a pick of the Ø 10 circle and the Ø 6 rule offer: a rule group of
///   24 rows (closed: more than 8) and a picked group of 1 row with
///   "Rule…" and "×" (a Drill holds no floor). The filter keeps the full
///   count on the header ("1 of 24").
///
/// Red proof: let a rule group also take the `Pick` rows: the row sum is
/// more than the report rows.
#[test]
fn the_list_groups_hold_each_selected_item_once_gt2() {
    use super::selection_section::{GROUP_CAP, GroupKind};
    use rs_cam_core::selection::{ItemKind, MatchKey, SelectionEdit, SourceRef};

    let mut session = holes_session();
    let (groups, report) = groups_of(&session);
    assert_eq!(groups.len(), 1, "the default row only: {groups:?}");
    assert_eq!(groups[0].kind, GroupKind::Default);
    assert!(groups[0].starts_closed(), "L-R2: the default row is closed");
    assert!(
        groups[0].title.starts_with("Default: every circle"),
        "{}",
        groups[0].title
    );
    assert_eq!(groups[0].rows.len(), report.rows.len());
    assert!(groups[0].rows.iter().all(|r| r.acts == Default::default()));

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

    let (groups, report) = groups_of(&session);
    let kinds: Vec<GroupKind> = groups.iter().map(|g| g.kind).collect();
    assert_eq!(
        kinds,
        [GroupKind::Rule(0), GroupKind::Picked(ItemKind::Circle)]
    );
    let total: usize = groups.iter().map(|g| g.rows.len()).sum();
    assert_eq!(total, report.rows.len(), "each item in one group only");
    assert_eq!(groups[0].rows.len(), 24);
    assert!(groups[0].rows.len() > GROUP_CAP && groups[0].starts_closed());
    assert_eq!(groups[1].rows.len(), 1);
    let picked = &groups[1].rows[0];
    // FR-2 F22: the row text is short; the full text (the place, the
    // layer) is the hover, and the filter reads it.
    assert_eq!(picked.label, "\u{00D8}10.00", "{}", picked.label);
    assert!(
        picked.full.starts_with("\u{00D8} 10.00 at (400.0, 0.0)"),
        "{}",
        picked.full
    );
    assert!(picked.acts.rule && picked.acts.remove && !picked.acts.floor);
    assert!(groups[0].rows.iter().all(|r| r.acts.remove && !r.acts.rule));
    assert_eq!(groups[0].count_text(""), "24");
    assert_eq!(groups[0].count_text("(36.0,"), "1 of 24");
}

/// GT-2: the tab paints the closed rule group as its header only, the open
/// picked group with its row, and a row under the pointer lights its item
/// in the viewport (`SelectionUi::row_hover`).
///
/// Red proof: drop the `row_hover` write of `draw_row`: the hover arm fails.
#[test]
fn the_geometry_tab_paints_the_groups_and_the_row_hover_gt2() {
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

    let tc = session.toolpath_configs()[0].clone();
    let inputs = super::toolpath_panel_inputs(
        tc.id,
        &session,
        &crate::state::runtime::GuiState::default(),
        None,
        Some(ToolpathTab::Geometry),
        false,
    );
    let mut entry = crate::state::toolpath::ToolpathEntry::for_operation(
        tc.id,
        "Drill".to_owned(),
        tc.tool_id.expect("the case tool"),
        tc.model_id,
        tc.operation.op_type(),
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
                &mut events,
            );
        });
        if pass == 2 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    let t = text.galley.job.text.clone();
                    if t.starts_with("\u{00D8}10.00 \u{00B7}") {
                        row_at = Some(clipped.shape.visual_bounding_rect().center());
                    }
                    texts.push(t);
                }
            }
        }
        out.textures_delta.clear();
    }
    assert!(
        texts
            .iter()
            .any(|t| t.contains("\u{00D8} 6.00") && t.ends_with("(24)")),
        "the rule header holds its count: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t.starts_with("\u{00D8}6.00 \u{00B7}")),
        "the closed group paints no row: {texts:?}"
    );
    assert!(
        row_at.is_some(),
        "the open picked group paints its row: {texts:?}"
    );
    assert_eq!(
        selection_ui.row_hover.as_ref().map(|k| k.measure),
        Some(10.0),
        "the row under the pointer lights its item"
    );
}

/// FR-2 F24: the owner map holds every core diagnostic id once, and holds
/// no id that core does not declare. A new core id with no row fails here.
///
/// Red proof: delete the `SAFETY_CUTS_INTO_MODEL` row: the first
/// assertion names it.
#[test]
fn owner_tab_map_covers_every_id_f24() {
    use rs_cam_core::diagnostics::ids;
    let rows: Vec<&str> = super::tab_badges::OWNER_TAB
        .iter()
        .map(|(id, _)| *id)
        .collect();
    let missing: Vec<&&str> = ids::ALL.iter().filter(|id| !rows.contains(id)).collect();
    assert!(missing.is_empty(), "ids with no owner row: {missing:?}");
    let unknown: Vec<&&str> = rows.iter().filter(|id| !ids::ALL.contains(id)).collect();
    assert!(unknown.is_empty(), "rows of no core id: {unknown:?}");
    let mut sorted = rows.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), rows.len(), "each id has one row");
}

/// FR-2 F24: a finding puts its dot on the tab that holds its fix. A
/// selection refusal and a model gouge go to Geometry; a load finding to
/// Feeds & Speeds; a pin finding (no toolpath tab) gives no dot. No
/// finding puts a dot on Dressup.
///
/// Red proof: the old "Mods" rule (every finding dots Dressup) gives no
/// Geometry dot and a Dressup dot, so the Geometry arm fails.
#[test]
fn a_finding_dots_the_tab_of_its_fix_f24() {
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
    let badges = super::tab_badges::compute_tab_badges(
        &entry,
        &[
            finding(ids::SAFETY_CUTS_INTO_MODEL, Severity::Blocking),
            finding(ids::LOAD_CHIPLOAD_HIGH, Severity::Caution),
            finding(ids::PROJECT_PIN_WALL_THIN, Severity::Critical),
        ],
        None,
        false,
    );
    assert_eq!(
        badges.for_tab(ToolpathTab::Geometry),
        Some(crate::ui::tokens::DANGER)
    );
    assert_eq!(
        badges.for_tab(ToolpathTab::FeedsSpeeds),
        Some(crate::ui::tokens::CAUTION)
    );
    for tab in [
        ToolpathTab::Cut,
        ToolpathTab::Linking,
        ToolpathTab::Heights,
        ToolpathTab::Dressup,
    ] {
        assert_eq!(badges.for_tab(tab), None, "{tab:?} gets no dot");
    }
}

/// FR-2 F22 (coordinator ruling): "Done · BLOCKED" is the F20 safety
/// signal and never truncates. The Generate row gives it its full width
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
    let finding = crate::ui::toolpath_panel::blocking_finding(
        Some(&stats),
        rs_cam_core::ToolpathId::from_raw(0),
    )
    .expect("a gouge blocks");
    let current = crate::state::freshness::FreshnessState::Current;
    let row = super::toolpath_panel::HeaderRow {
        can_generate: true,
        freshness: &current,
        manual_generation: false,
        blocking: Some(&finding),
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
                let _ = super::toolpath_panel::draw_generate_row(ui, &row);
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
    let blocked = crate::ui::toolpath_panel::BLOCKED_HEADER;
    for width in [280.0, 200.0] {
        let texts = paint(width);
        let word: Vec<&(String, f32, bool)> = texts.iter().filter(|t| t.0 == blocked).collect();
        assert_eq!(word.len(), 1, "at {width}: one state word: {texts:?}");
        assert!(!word[0].2, "at {width}: BLOCKED is not elided: {texts:?}");
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
/// - the GUI Geometry tab list (`list_groups`) gives one row,
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
    let mut tool = rs_cam_core::compute::tool_config::ToolConfig::new_default(
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
    let (groups, _) = groups_of(&session);
    let rows: Vec<&str> = groups
        .iter()
        .flat_map(|g| g.rows.iter())
        .map(|row| row.label.as_str())
        .collect();
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
/// - the GUI Geometry tab list (`list_groups`) gives one row,
///   "Part outline Z0", under the title "Default: the part outline";
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
    let mut tool = rs_cam_core::compute::tool_config::ToolConfig::new_default(
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
    let (groups, _) = groups_of(&session);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].title, "Default: the part outline");
    let rows: Vec<&str> = groups[0].rows.iter().map(|r| r.label.as_str()).collect();
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
/// acceptance door of the add), and an op with no tool draws its state and
/// the fix on the Cut tab.
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

    let v_bit = ToolConfig::new_default(rs_cam_core::compute::ToolId::from_raw(0), ToolType::VBit);
    let end_mill =
        ToolConfig::new_default(rs_cam_core::compute::ToolId::from_raw(1), ToolType::EndMill);
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
        Some(ToolpathTab::Cut),
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
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        super::toolpath_panel::draw_cut_tab(
            ui,
            &mut entry,
            &inputs,
            None,
            None,
            &mut crate::state::pick::SelectionUi::default(),
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
    let line = format!("No tool set. {}", rs_cam_core::session::NO_TOOL_FIX);
    assert!(
        texts.iter().any(|t| t == &line),
        "the Cut tab draws the state; it rendered {texts:?}"
    );
    assert!(
        texts
            .iter()
            .any(|t| t == rs_cam_core::session::NO_TOOL_NAME),
        "the Tool combo reads `No tool`; it rendered {texts:?}"
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
                super::operations::draw_adaptive3d_params(ui, &mut cfg, None, suggestion);
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

/// G-D (ruling D-1): with the boundary on, the Cut tab draws the `Source:`
/// combo and no `Boundary: …` row above it; the row's text is the combo's
/// hover. The row repeated the combo one line above.
#[test]
fn the_boundary_summary_is_the_source_hover_d1() {
    use crate::state::toolpath::{BoundarySource, OperationType, ToolpathEntry};
    let (session, _mesh) = one_pocket_session();
    let tc = &session.toolpath_configs()[0];
    let inputs = super::toolpath_panel_inputs(
        tc.id,
        &session,
        &crate::state::runtime::GuiState::default(),
        None,
        Some(ToolpathTab::Cut),
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
            super::toolpath_panel::draw_cut_tab(
                ui,
                &mut entry,
                &inputs,
                None,
                None,
                &mut crate::state::pick::SelectionUi::default(),
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
        texts.iter().any(|t| t == "Source:"),
        "non-vacuity: the boundary controls draw: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t.starts_with("Boundary:")),
        "the summary row is back: {texts:?}"
    );
}

/// G-E (2): the footer findings are bounded. Five caution rows show three
/// and the remainder line "Showing 3 of 5 · Show all"; expanded, all five.
/// A blocking row past the cap still shows (the safety rule).
#[test]
fn the_footer_findings_are_bounded_g_e() {
    use super::toolpath_panel::{FOOTER_FINDINGS_CAP, bounded_findings};
    use rs_cam_core::diagnostics::Severity;
    let five = [Severity::Caution; 5];
    assert_eq!(FOOTER_FINDINGS_CAP, 3);
    assert_eq!(bounded_findings(&five, false), vec![0, 1, 2]);
    assert_eq!(bounded_findings(&five, true), vec![0, 1, 2, 3, 4]);
    assert_eq!(
        crate::ui::components::notice::overflow_line(3, 5),
        "Showing 3 of 5 \u{00B7} Show all"
    );
    let mut late_block = five;
    late_block[4] = Severity::Blocking;
    assert_eq!(bounded_findings(&late_block, false), vec![0, 1, 2, 4]);
    assert_eq!(
        bounded_findings(&five[..2], false),
        vec![0, 1],
        "under the cap"
    );
}

/// G-FEEDMAX (C2): the Max Rate row caps every feed stage, so the Linking
/// tab's Advanced section draws it with "Feed rate optimization" off. The
/// Ramp Rate row belongs to the optimiser and stays hidden.
#[test]
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
    for pass in 0..6 {
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
            super::linking_dressup::draw_linking_params(ui, &mut entry, None, None);
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
        texts.iter().any(|t| t == "Feed rate optimization"),
        "non-vacuity: the Advanced section is open: {texts:?}"
    );
    assert!(texts.iter().any(|t| t == "Max Rate:"), "{texts:?}");
    assert!(!texts.iter().any(|t| t.contains("Ramp Rate")), "{texts:?}");
}
