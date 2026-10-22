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
                tool_id: rs_cam_core::ids::ToolId::from_raw(1),
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
        super::operations::draw_drill_params(ui, &mut drill, None, None, None);
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
        let mut entry =
            ToolpathEntry::for_operation(tc.id, "Case".to_owned(), tc.tool_id, tc.model_id, op);
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
    let mut entry =
        ToolpathEntry::for_operation(tc.id, "Case".to_owned(), tc.tool_id, tc.model_id, op);
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
                tool_id: rs_cam_core::ids::ToolId::from_raw(1),
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
    let start_from = position_of(&cut, "Start from row", |t| t.starts_with("Start from"));
    let boundary = position_of(&cut, "boundary header", |t| {
        t.eq_ignore_ascii_case("machining boundary")
    });
    assert!(
        input < start_from && start_from < boundary,
        "want Input < Start from < Machining Boundary: {cut:?}"
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
                tool_id: rs_cam_core::ids::ToolId::from_raw(1),
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
    assert!(
        picked.label.starts_with("\u{00D8} 10.00 at (400.0, 0.0)"),
        "{}",
        picked.label
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
        tc.tool_id,
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
                    if t.starts_with("\u{00D8} 10.00 at") {
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
        !texts.iter().any(|t| t.starts_with("\u{00D8} 6.00 at")),
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
