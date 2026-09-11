//! G-DEPTHSTOCK sentry: a 2.5D depth below the stock bottom raises a
//! CAUTION, never a block.
//!
//! UX-R03-007 (`planning/ui_review_2026-09-09/results/R03/REPORT.md`): a
//! pocket 15 mm deep on a 12 mm board generated and simulated silently. The
//! row read OK, the header read Done, and the cut went 3 mm into the bed.
//!
//! Cases:
//! (a) Pocket depth 25 mm on 18 mm stock: one caution "7.00 mm", Generate
//!     stays enabled;
//! (b) depth 18 mm on 18 mm stock (a through cut): no caution;
//! (c) a 3D operation (DropCutter): the rule does not apply;
//! (d) a pinned bottom Z below the stock bottom: NO caution;
//! plus a flipped setup, which has the same thickness and the same excess.
//!
//! # Case (d) changed its expectation on 2026-09-10 (F1.18 / J8)
//!
//! It used to assert a 3.00 mm caution. The GUI rule this file was written
//! against took the DEEPER of two bottoms — the operation's depth dial and
//! the Heights tab's resolved Bottom Z — so a 6 mm pocket with Bottom Z
//! pinned 3 mm below an 18 mm board cautioned on the pin.
//!
//! F1.19 then measured the pin. It reaches emitted motion on three of the
//! twenty-four operations, and on none of the operations this rule answers
//! for: the pin moved 14 mm on a test pocket and the emitted floor did not
//! move at all. The caution therefore described a cut the machine does not
//! make. The core predicate reads the depth dial alone, the GUI now consumes
//! it, and 6 mm of cut into an 18 mm board is not a cut through the board.
//!
//! The Heights tab says the other half of that sentence beside the field
//! itself (`bottom_z_pin_note`, G-BOTTOMPIN): the pin is not used by this
//! operation. Two findings, two surfaces. Merging them produced the false
//! caution.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{HeightMode, HeightReference, ReferenceOffset};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::diagnostics::ids::GEOM_DEPTH_BEYOND_STOCK;
use rs_cam_core::diagnostics::{Diagnostic, Severity};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_viz::state::job::{ModelKind, ModelUnits};
use rs_cam_viz::state::runtime::GuiState;
use rs_cam_viz::state::toolpath::OperationType;
use rs_cam_viz::ui::properties::{
    ToolpathPanelSnapshot, ToolpathValidationContext, collect_diagnostics, depth_beyond_stock,
    toolpath_panel_snapshot, validate_toolpath,
};

const TOOL: usize = 1;
const MODEL_2D: usize = 4;
const MODEL_3D: usize = 5;
const STOCK_THICKNESS_MM: f64 = 18.0;

const CAUTION_TEXT: &str = "exceeds stock thickness";

fn polygon_model(id: usize) -> LoadedModel {
    LoadedModel {
        id,
        path: PathBuf::from(format!("model_{id}.svg")),
        name: format!("2D {id}"),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(
            -10.0, -10.0, 10.0, 10.0,
        )])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    }
}

fn mesh_model(id: usize) -> LoadedModel {
    LoadedModel {
        id,
        path: PathBuf::from(format!("model_{id}.stl")),
        name: format!("3D {id}"),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_flat(20.0))),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    }
}

fn toolpath(name: &str, model_id: usize, op: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0), // assigned by session.add_toolpath
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: TOOL,
        model_id,
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
    }
}

fn pocket(depth: f64) -> OperationConfig {
    let mut op = OperationConfig::Pocket(Default::default());
    if let OperationConfig::Pocket(cfg) = &mut op {
        cfg.depth = depth;
    }
    op
}

/// One tool, one 2D model, one 3D model, 18 mm stock, one setup.
fn session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(TOOL), ToolType::EndMill);
    tool.diameter = 6.0;
    let _ = session.replace_tools(vec![tool]);
    session.models_mut().push(polygon_model(MODEL_2D));
    session.models_mut().push(mesh_model(MODEL_3D));
    let stock = StockConfig {
        z: STOCK_THICKNESS_MM,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let _ = session.set_stock_config(stock);
    session
}

/// Build the same owned entry + static-context snapshot as the production
/// properties-panel caller.
fn panel_snapshot(session: &ProjectSession, idx: usize) -> ToolpathPanelSnapshot {
    let id = session.toolpath_configs()[idx].id;
    toolpath_panel_snapshot(id, session, &GuiState::default())
        .expect("the toolpath resolves for the properties panel")
}

/// The header surface: what the inspector ribbon shows for the toolpath at
/// `idx`, built from the production panel snapshot.
fn header_diagnostics(
    session: &ProjectSession,
    idx: usize,
    snapshot: &ToolpathPanelSnapshot,
) -> Vec<Diagnostic> {
    let tc = &session.toolpath_configs()[idx];
    let tool = session
        .tools()
        .iter()
        .find(|t| t.id.0 == tc.tool_id)
        .cloned()
        .expect("tool is in the session");
    let height_ctx = session.height_context_for_toolpath(tc);
    collect_diagnostics(
        &snapshot.entry,
        Some(&tool),
        &[],
        Some(&height_ctx),
        &snapshot.preconditions,
        &snapshot.model_refs,
        None,
    )
}

fn depth_cautions(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
    diags
        .iter()
        .filter(|d| d.message.contains(CAUTION_TEXT))
        .collect()
}

/// The rule itself, read the way both surfaces read it.
fn rule(
    session: &ProjectSession,
    idx: usize,
) -> Option<rs_cam_viz::ui::properties::DepthBeyondStock> {
    let tc = &session.toolpath_configs()[idx];
    let height_ctx = session.height_context_for_toolpath(tc);
    depth_beyond_stock(&tc.operation, &tc.heights, &height_ctx)
}

// ── (a) depth 25 mm on 18 mm stock ──────────────────────────────────────

#[test]
fn a_pocket_deeper_than_the_stock_cautions_on_the_header_and_does_not_block() {
    let mut session = session();
    let idx = session
        .add_toolpath(0, toolpath("Pocket", MODEL_2D, pocket(25.0)))
        .unwrap();
    let snapshot = panel_snapshot(&session, idx);

    // Header: exactly one caution, worded with the excess.
    let diags = header_diagnostics(&session, idx, &snapshot);
    let cautions = depth_cautions(&diags);
    assert_eq!(
        cautions.len(),
        1,
        "(a) expected one depth caution on the header, got {cautions:?}\nall: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    let caution = cautions[0];
    assert_eq!(
        caution.message, "Depth exceeds stock thickness by 7.00 mm",
        "(a) the caution names the excess"
    );
    assert_eq!(
        caution.severity,
        Severity::Caution,
        "(a) a caution, not a block"
    );

    // Generate stays enabled: the blocking validator has nothing to say.
    let errs = validate_toolpath(
        &snapshot.entry,
        &ToolpathValidationContext::from_session(&session),
    );
    assert!(
        errs.is_empty(),
        "(a) the blocking validator must stay silent, it said {errs:?}"
    );
}

#[test]
fn a_the_rule_reads_the_excess_and_the_diagnostic_carries_its_id() {
    let mut session = session();
    let idx = session
        .add_toolpath(0, toolpath("Pocket", MODEL_2D, pocket(25.0)))
        .unwrap();
    let finding = rule(&session, idx).expect("(a) the rule fires");
    assert!(
        (finding.excess_mm - 7.0).abs() < 1e-9,
        "(a) excess is 25 - 18 = 7, got {}",
        finding.excess_mm
    );
    assert!((finding.stock_thickness_mm - STOCK_THICKNESS_MM).abs() < 1e-9);
    assert_eq!(
        finding.message(),
        "Depth exceeds stock thickness by 7.00 mm"
    );

    let snapshot = panel_snapshot(&session, idx);
    let diags = header_diagnostics(&session, idx, &snapshot);
    let by_id: Vec<_> = diags
        .iter()
        .filter(|d| d.id.0 == GEOM_DEPTH_BEYOND_STOCK)
        .collect();
    assert_eq!(by_id.len(), 1, "(a) one diagnostic under the rule's id");
    assert!(
        !diags
            .iter()
            .any(|d| matches!(d.severity, Severity::Blocking | Severity::Critical)),
        "(a) nothing on the header blocks: {:?}",
        diags
            .iter()
            .map(|d| (&d.id.0, d.severity))
            .collect::<Vec<_>>()
    );
}

// ── (b) a through cut exactly at the stock thickness ────────────────────

#[test]
fn b_depth_equal_to_the_stock_thickness_is_not_a_caution() {
    let mut session = session();
    let idx = session
        .add_toolpath(
            0,
            toolpath("Pocket through", MODEL_2D, pocket(STOCK_THICKNESS_MM)),
        )
        .unwrap();
    assert_eq!(
        rule(&session, idx),
        None,
        "(b) a through cut is UX-R03-006's case"
    );
    let snapshot = panel_snapshot(&session, idx);
    let diags = header_diagnostics(&session, idx, &snapshot);
    assert!(
        depth_cautions(&diags).is_empty(),
        "(b) no depth caution on the header: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[test]
fn b2_a_shallower_pocket_is_not_a_caution() {
    let mut session = session();
    let idx = session
        .add_toolpath(0, toolpath("Pocket", MODEL_2D, pocket(6.0)))
        .unwrap();
    assert_eq!(rule(&session, idx), None);
}

// ── (c) a 3D operation ──────────────────────────────────────────────────

#[test]
fn c_a_3d_operation_is_outside_the_rule() {
    let mut session = session();
    // The 3D finish has no depth field; the mesh is its floor. Pin the
    // Bottom Z below the stock anyway: the rule must still not read it.
    let mut tc = toolpath(
        "3D finish",
        MODEL_3D,
        OperationConfig::DropCutter(Default::default()),
    );
    tc.heights.bottom_z = HeightMode::FromReference(ReferenceOffset {
        reference: HeightReference::StockBottom,
        offset: -30.0,
    });
    let idx = session.add_toolpath(0, tc).unwrap();
    assert_eq!(
        rule(&session, idx),
        None,
        "(c) the rule does not read a 3D op"
    );
    let snapshot = panel_snapshot(&session, idx);
    let diags = header_diagnostics(&session, idx, &snapshot);
    assert!(
        diags.iter().all(|d| d.id.0 != GEOM_DEPTH_BEYOND_STOCK),
        "(c) no depth-beyond-stock diagnostic on a 3D op"
    );
}

#[test]
fn c2_alignment_pin_drill_penetrates_the_spoilboard_by_design() {
    let mut session = session();
    let idx = session
        .add_toolpath(
            0,
            toolpath(
                "Pins",
                MODEL_2D,
                OperationConfig::AlignmentPinDrill(Default::default()),
            ),
        )
        .unwrap();
    assert_eq!(rule(&session, idx), None);
}

// ── (d) a pinned Bottom Z below the stock bottom ────────────────────────

/// RE-PINNED 2026-09-10 (F1.18 / J8). Before: the rule fired and the message
/// read `Depth exceeds stock thickness by 3.00 mm`. Now: silence, because the
/// pocket cuts 6 mm into an 18 mm board and the pin reaches no emitted motion
/// (F1.19). See this file's header for the full argument.
#[test]
fn d_a_bottom_z_pinned_below_the_stock_bottom_is_not_a_caution() {
    let mut session = session();
    let mut tc = toolpath("Pocket", MODEL_2D, pocket(6.0));
    tc.heights.bottom_z = HeightMode::FromReference(ReferenceOffset {
        reference: HeightReference::StockBottom,
        offset: -3.0,
    });
    let idx = session.add_toolpath(0, tc).unwrap();
    assert_eq!(
        rule(&session, idx),
        None,
        "(d) the pin moves no motion on a pocket, so it raises no caution"
    );

    // The header must agree: the rule and the ribbon are one predicate.
    let snapshot = panel_snapshot(&session, idx);
    let diags = header_diagnostics(&session, idx, &snapshot);
    assert!(
        diags.iter().all(|d| d.id.0 != GEOM_DEPTH_BEYOND_STOCK),
        "(d) no depth-beyond-stock diagnostic on the header: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

/// The other half of the F1.19 finding, on the surface that DOES read it: the
/// Heights tab annotates the Bottom row on a Pocket, so the operator is told
/// why the pin changed nothing.
#[test]
fn d_the_heights_tab_says_the_pin_is_not_used_on_a_pocket() {
    let note = rs_cam_viz::ui::properties::bottom_z_pin_note(OperationType::Pocket)
        .expect("(d) a Pocket ignores the pin, so the row carries a note");
    assert!(
        note.contains("floor"),
        "(d) the note names the dial that sets the floor: {note}"
    );
}

#[test]
fn d2_a_manual_bottom_z_at_the_stock_bottom_is_not_a_caution() {
    let mut session = session();
    let mut tc = toolpath("Pocket", MODEL_2D, pocket(6.0));
    let stock_bottom_z = session.stock_config().bbox().min.z;
    tc.heights.bottom_z = HeightMode::Manual(stock_bottom_z);
    let idx = session.add_toolpath(0, tc).unwrap();
    assert_eq!(rule(&session, idx), None);
}

// ── a flipped setup has the same thickness ──────────────────────────────

#[test]
fn a_flipped_setup_reads_the_same_excess() {
    let mut session = session();
    let flip = session.add_setup("Flip".to_owned(), FaceUp::Bottom);
    let idx = session
        .add_toolpath(flip, toolpath("Pocket", MODEL_2D, pocket(25.0)))
        .unwrap();
    let finding = rule(&session, idx).expect("the flipped pocket fires the rule");
    assert!(
        (finding.excess_mm - 7.0).abs() < 1e-9,
        "flip: excess is still 7, got {}",
        finding.excess_mm
    );
    assert!((finding.stock_thickness_mm - STOCK_THICKNESS_MM).abs() < 1e-9);
}
