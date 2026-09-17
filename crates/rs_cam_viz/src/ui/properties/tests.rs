//! Unit tests for the inspector's tab enum and its diagnostic-row merge.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
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
            id: rs_cam_core::ToolpathId(0),
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
/// `ProjectSession::model_bbox`.
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
        rs_cam_core::compute::ToolId(1),
        rs_cam_core::compute::tool_config::ToolType::EndMill,
    );
    let mut builder = ProjectSessionBuilder::new().tool(tool);
    let mesh = Arc::new(rs_cam_core::mesh::make_test_flat(40.0));
    let _ = builder.add_model(LoadedModel {
        id: 0,
        path: std::path::PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::clone(&mesh)),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });
    let _ = builder
        .add_toolpath(
            0,
            ToolpathConfig {
                id: rs_cam_core::compute::ToolpathId(0),
                name: "Pocket".to_owned(),
                enabled: true,
                operation: crate::state::toolpath::OperationConfig::new_default(
                    crate::state::toolpath::OperationType::Pocket,
                ),
                dressups: Default::default(),
                heights: Default::default(),
                tool_id: 1,
                model_id: 0,
                pre_gcode: None,
                post_gcode: None,
                boundary: Default::default(),
                boundary_inherit: true,
                stock_source: Default::default(),
                coolant: Default::default(),
                face_selection: None,
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
        .model_bbox(tc.model_id)
        .expect("the fixture model carries a finite bbox");
    let carried = snapshot
        .model_bbox
        .expect("the snapshot must carry the model bbox, not None");
    assert_eq!(
        format!("{carried:?}"),
        format!("{expected:?}"),
        "the snapshot bbox must be `ProjectSession::model_bbox` of the stored model id"
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
        inputs.height_ctx.is_some(),
        "a toolpath with a model resolves a height context"
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
        super::operations::draw_drill_params(ui, &mut drill, &[], &[], None, None);
    });

    let mut pin = AlignmentPinDrillConfig {
        cycle: DrillCycleType::Peck,
        ..Default::default()
    };
    let pin_texts = rendered(|ui| {
        super::operations::draw_alignment_pin_drill_params(ui, &mut pin, &[], &[], None);
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
