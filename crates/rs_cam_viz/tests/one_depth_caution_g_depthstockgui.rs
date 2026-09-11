//! G-DEPTHSTOCKGUI: the GUI holds no second copy of the depth-beyond-stock
//! rule, and one toolpath never carries the same diagnostic id twice.
//!
//! # The defect this guards against
//!
//! F1.18 moved the depth-beyond-stock predicate into core
//! (`diagnostics::adapters::from_static_checks::depth_beyond_stock`). The GUI
//! kept its own copy and appended it to the list core had already produced.
//! Both fired, both stamped `geom.depth_beyond_stock`, and the inspector
//! ribbon printed the identical Safety sentence TWICE. A Safety row shown
//! twice is itself a surface saying something untrue about how many problems
//! the operator has.
//!
//! The orchestrator's gate on the merged head `9292287a` caught it through
//! the F1.6 sentry's `== 1` assertions. This file makes the guard generic and
//! independent of that fixture, so the next surface that appends a finding
//! core already produces fails here.
//!
//! # The three arms
//!
//! 1. no id repeats in a `collect_diagnostics` list, over a walk of fixtures;
//! 2. the Operations card row and the Safety header agree case by case —
//!    they are one predicate, not two that happen to agree;
//! 3. a Top Z pinned below the stock top still deepens the caution. That is
//!    the reading the GUI must NOT lose in the switchover: the generators cut
//!    `top_z - depth`, and `profile_through_cut` in the same form reads the
//!    same pin.
//!
//! The pinned BOTTOM Z is the one reading that was deliberately dropped —
//! see `depth_beyond_stock_cautions_g_depthstock.rs` case (d).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{HeightMode, HeightReference, HeightsConfig, ReferenceOffset};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::Diagnostic;
use rs_cam_core::diagnostics::ids::GEOM_DEPTH_BEYOND_STOCK;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::job::{ModelKind, ModelUnits};
use rs_cam_viz::state::runtime::GuiState;
use rs_cam_viz::ui::properties::{
    collect_diagnostics, depth_beyond_stock, toolpath_panel_snapshot,
};

const TOOL: usize = 1;
const MODEL_2D: usize = 4;
const MODEL_3D: usize = 5;
const STOCK_THICKNESS_MM: f64 = 18.0;

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

/// One tool, one 2D model, one 3D model, an 18 mm board, one setup.
fn session() -> ProjectSession {
    let mut tool = ToolConfig::new_default(ToolId(TOOL), ToolType::EndMill);
    tool.diameter = 6.0;
    let stock = StockConfig {
        z: STOCK_THICKNESS_MM,
        auto_from_model: false,
        ..StockConfig::default()
    };
    ProjectSessionBuilder::new()
        .tool(tool)
        .model(polygon_model(MODEL_2D))
        .model(mesh_model(MODEL_3D))
        .stock(stock)
        .build()
}

/// One fixture: a name, the operation, its heights, and the model it binds.
struct Case {
    name: &'static str,
    op: OperationConfig,
    model_id: usize,
    heights: HeightsConfig,
}

fn pinned(reference: HeightReference, offset: f64) -> HeightMode {
    HeightMode::FromReference(ReferenceOffset { reference, offset })
}

/// The walk. Every case names what it is for.
fn cases() -> Vec<Case> {
    let bottom_pinned = HeightsConfig {
        bottom_z: pinned(HeightReference::StockBottom, -3.0),
        ..HeightsConfig::default()
    };
    let top_pinned = HeightsConfig {
        top_z: pinned(HeightReference::StockTop, -5.0),
        ..HeightsConfig::default()
    };

    vec![
        Case {
            name: "pocket 25 mm, heights auto — through the board",
            op: pocket(25.0),
            model_id: MODEL_2D,
            heights: HeightsConfig::default(),
        },
        Case {
            name: "pocket 6 mm, heights auto — inside the board",
            op: pocket(6.0),
            model_id: MODEL_2D,
            heights: HeightsConfig::default(),
        },
        Case {
            name: "pocket 6 mm, Bottom Z pinned 3 mm under the board",
            op: pocket(6.0),
            model_id: MODEL_2D,
            heights: bottom_pinned,
        },
        Case {
            name: "pocket 15 mm, Top Z pinned 5 mm down — 2 mm through",
            op: pocket(15.0),
            model_id: MODEL_2D,
            heights: top_pinned,
        },
        Case {
            name: "drop cutter — the mesh is the floor, the rule abstains",
            op: OperationConfig::DropCutter(Default::default()),
            model_id: MODEL_3D,
            heights: HeightsConfig::default(),
        },
    ]
}

/// Add `case` to a fresh session. Return the Operations card row's sentence
/// and the Safety header's diagnostic list, both read the way the panel
/// reads them.
fn read_case(case: &Case) -> (Option<String>, Vec<Diagnostic>) {
    let mut session = session();
    let mut tc = toolpath(case.name, case.model_id, case.op.clone());
    tc.heights = case.heights.clone();
    let idx = session.add_toolpath(0, tc).unwrap();

    let tc = &session.toolpath_configs()[idx];
    let height_ctx = session.height_context_for_toolpath(tc);
    let snapshot = toolpath_panel_snapshot(tc.id, &session, &GuiState::default())
        .expect("the toolpath resolves for the properties panel");

    // The Operations card row.
    let found = depth_beyond_stock(&tc.operation, &tc.heights, &height_ctx);
    let row = found.map(|f| f.message());

    // The Safety header.
    let tool = session
        .tools()
        .iter()
        .find(|t| t.id.0 == tc.tool_id)
        .cloned()
        .expect("the tool is in the session");
    let header = collect_diagnostics(
        &snapshot.entry,
        Some(&tool),
        &[],
        Some(&height_ctx),
        &snapshot.preconditions,
        &snapshot.model_refs,
        None,
    );
    (row, header)
}

// ── arm 1: one toolpath, one id, once ───────────────────────────────────

#[test]
fn no_diagnostic_id_appears_twice_on_one_toolpath() {
    let mut walked = 0usize;
    let mut with_findings = 0usize;
    for case in cases() {
        walked += 1;
        let (_row, header) = read_case(&case);
        if !header.is_empty() {
            with_findings += 1;
        }
        let mut seen: HashMap<String, usize> = HashMap::new();
        for d in &header {
            *seen.entry(d.id.0.clone()).or_insert(0) += 1;
        }
        for (id, count) in &seen {
            assert_eq!(
                *count, 1,
                "{}: `{id}` is reported {count} times. Two producers stamp \
                 one id — the GUI is appending a finding core already \
                 produces.",
                case.name
            );
        }
    }
    eprintln!("G-DEPTHSTOCKGUI: {walked} cases walked, {with_findings} with findings");
    assert_eq!(walked, cases().len(), "the walk must cover every case");
    assert!(walked > 0, "an empty walk passes every assertion above");
    assert!(
        with_findings > 0,
        "no case produced a diagnostic, so nothing was actually checked"
    );
}

// ── arm 2: the row and the header are one predicate ─────────────────────

#[test]
fn the_row_and_the_header_answer_the_same_way() {
    let mut cautioned = 0usize;
    let mut silent = 0usize;
    for case in cases() {
        let (row, header) = read_case(&case);
        let header_says: Vec<&Diagnostic> = header
            .iter()
            .filter(|d| d.id.0 == GEOM_DEPTH_BEYOND_STOCK)
            .collect();
        assert_eq!(
            row.is_some(),
            header_says.len() == 1,
            "{}: the card row says {row:?}, the header carries {} rows",
            case.name,
            header_says.len()
        );
        if let (Some(message), Some(found)) = (row.as_ref(), header_says.first()) {
            assert_eq!(
                &found.message, message,
                "{}: the two surfaces print different sentences",
                case.name
            );
            cautioned += 1;
        } else {
            silent += 1;
        }
    }
    eprintln!("G-DEPTHSTOCKGUI: {cautioned} cautioned, {silent} silent");
    // Non-vacuity on both sides: a rule that never fires, or one that always
    // fires, would satisfy the equality above.
    assert!(cautioned > 0, "no case cautioned");
    assert!(silent > 0, "every case cautioned");
}

// ── arm 3: a pinned Top Z still deepens the cut ─────────────────────────

#[test]
fn a_top_z_pinned_below_the_stock_top_still_cautions() {
    let case = cases()
        .into_iter()
        .find(|c| c.name.contains("Top Z pinned"))
        .expect("the walk carries the pinned-top case");
    let (row, header) = read_case(&case);
    assert_eq!(
        row.as_deref(),
        Some("Depth exceeds stock thickness by 2.00 mm"),
        "a Top Z pinned 5 mm down turns a 15 mm pocket into a 2 mm cut \
         through an 18 mm board. The generators cut `top_z - depth`, so the \
         GUI must read the pinned top."
    );
    assert!(
        header.iter().any(|d| d.id.0 == GEOM_DEPTH_BEYOND_STOCK),
        "the header must carry it too"
    );
}
