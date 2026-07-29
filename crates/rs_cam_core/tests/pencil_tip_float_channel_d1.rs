//! Wave D1 sentry — tip float on a pencil centreline must be REPORTED.
//!
//! ## The defect (Checkpoint A evidence §5 / §9.4, ledger)
//!
//! A pencil pass traces the valley the detector found and re-solves Z with
//! the drop cutter. When the valley is narrower than the tool can enter, the
//! cutter wedges on the walls and the emitted line sits ABOVE the floor —
//! the pass runs, the machine cuts air (or the wall), and the residual stays
//! in the part. The Checkpoint A matrix measured 24 of 176 taper cells like
//! this, with up to **5.248 mm** of residual, and found no channel of any
//! kind reporting it: not stats, not narration, not diagnostics.
//!
//! This file is the channel. It does not change routing — H2 owns that.
//!
//! ## Contract
//!
//! * **Kind**: GATE (default CI, synthetic, seconds).
//! * **Metric domain**: a VERTICAL residual (mm) at a point —
//!   `MeasurementDomain::VerticalResidualMm`. It is not an area, not a path
//!   length, and not comparable to either.
//! * **Measurement**: the cutter's drop-solved resting Z at an emitted
//!   centreline point, minus the valley-floor Z re-solved at the same XY
//!   with the Ø0.1 mm surface probe ball the rest-depth reference chain
//!   already uses. It is NOT differenced against the detector's own
//!   polyline Z, which for `RestCenterline` is *already the pencil drop*
//!   and would measure zero by construction.
//! * **Report-only**: no gate consumes it, severity stays `Caution`.
//!
//! ## Why it cannot pass vacuously
//!
//! The two grooves are cut by the SAME Ø3 ball. Gate 1's tool physically
//! cannot enter its groove and Gate 2's can enter its own, so a stubbed
//! measurement that always fired, or always stayed quiet, fails one of them.
//! Both gates assert the pass actually emitted cutting first.
//!
//! ## Red-first evidence
//!
//! At `e9f9554`, on Gate 1's fixture through
//! `ProjectSession::generate_toolpath`: 37 moves / 56.3 mm of cutting were
//! emitted, the lowest commanded Z was **-0.1252 mm** against a valley floor
//! at **-2.5 mm** (2.37 mm of residual the pass can never remove), and
//! `narrate_toolpath` contained no occurrence of "float" while
//! `diagnose_toolpath_with_trace` returned only feeds/load diagnostics.
//! Recorded in the commit body.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, StockSource, TIP_FLOAT_THRESHOLD_MM,
};
use rs_cam_core::compute::operation_configs::{PencilConfig, WaterlineConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::diagnostics::{Diagnostic, Severity, ids};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::measurement::{MeasurementDomain, MeasurementStage};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};

// ── Fixture ──────────────────────────────────────────────────────────────

/// The Ø3 ball both gates use. `cusp_radius() == radius() == 1.5`, so no
/// tapered-tool scale question is in play here — only reach.
const BALL_DIAMETER_MM: f64 = 3.0;

fn ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: BALL_DIAMETER_MM,
        ..ToolConfig::new_default(ToolId(0), ToolType::BallNose)
    }
}

/// A 40 × 24 mm block, top at z = 0, with one straight trapezoidal groove
/// along Y at x = 0. Adapted from `checkpoint_a_valley_matrix::grooved_block`
/// — same construction, denser X sampling across the groove.
fn grooved_block(rim_half_width: f64, wall_deg: f64, depth: f64) -> TriangleMesh {
    let tan = wall_deg.to_radians().tan();
    let floor_half = rim_half_width - depth / tan;
    assert!(floor_half > 0.0, "groove is a V, not a trapezoid");
    let z_at = |x: f64| -> f64 {
        let ax = x.abs();
        if ax >= rim_half_width {
            0.0
        } else if ax <= floor_half {
            -depth
        } else {
            -depth + (ax - floor_half) * tan
        }
    };

    let mut xs: Vec<f64> = Vec::new();
    let mut x = -20.0;
    while x < -3.0 {
        xs.push(x);
        x += 1.0;
    }
    let mut x = -3.0;
    while x <= 3.0 + 1e-9 {
        xs.push(x);
        x += 0.05;
    }
    for b in [-rim_half_width, -floor_half, floor_half, rim_half_width] {
        xs.push(b);
    }
    let mut x = 4.0;
    while x <= 20.0 + 1e-9 {
        xs.push(x);
        x += 1.0;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    let ys: Vec<f64> = (0..=24).map(|i| -12.0 + i as f64).collect();
    let mut verts = Vec::with_capacity(xs.len() * ys.len());
    for &yv in &ys {
        for &xv in &xs {
            verts.push(P3::new(xv, yv, z_at(xv)));
        }
    }
    let nx = xs.len();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for j in 0..ys.len() - 1 {
        for i in 0..nx - 1 {
            let a = (j * nx + i) as u32;
            let b = (j * nx + i + 1) as u32;
            let c = ((j + 1) * nx + i + 1) as u32;
            let d = ((j + 1) * nx + i) as u32;
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// Gate 1's groove: 0.6 mm rim half-width, 80° walls, 2.5 mm deep. The Ø3
/// ball wedges on the walls at `-(1.5 - √(1.5² - 0.6²)) = -0.1252 mm`,
/// 2.37 mm above the floor, and cannot be driven any deeper.
fn unreachable_groove() -> TriangleMesh {
    grooved_block(0.6, 80.0, 2.5)
}

/// Gate 2's groove: 2.0 mm rim half-width, 45° walls, 1.0 mm deep, giving a
/// 1.0 mm half-width flat floor. The same Ø3 ball sits ON the floor.
fn reachable_groove() -> TriangleMesh {
    grooved_block(2.0, 45.0, 1.0)
}

fn mesh_model(mesh: TriangleMesh, name: &str) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: name.to_owned(),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://grooved_block.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

/// The rest-depth detector — the arm whose centrelines the Checkpoint A
/// probes drove, and the one production recommends on relief.
fn pencil_op() -> OperationConfig {
    OperationConfig::Pencil(PencilConfig {
        detector: "rest_depth".to_owned(),
        rest_cell_mm: 0.2,
        min_valley_depth: 0.05,
        min_cut_length: 2.0,
        num_offset_passes: 0,
        sampling: 0.5,
        // A big reference tool, so the whole groove reads as rest material.
        reference_tool_diameter: 12.0,
        ..PencilConfig::default()
    })
}

fn toolpath(op: OperationConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: "Pencil".to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    }
}

fn stock() -> StockConfig {
    StockConfig {
        x: 40.0,
        y: 24.0,
        z: 6.0,
        origin_x: -20.0,
        origin_y: -12.0,
        origin_z: -6.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn session_with(mesh: TriangleMesh, name: &str, op: OperationConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock());
    let tool_idx = session.add_tool(ball_tool());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(mesh, name));
    session
        .add_toolpath(0, toolpath(op, tool_id, model_id))
        .expect("add toolpath");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("must generate — this channel is report-only");
    session
}

fn float_diagnostic(diags: &[Diagnostic]) -> Option<&Diagnostic> {
    diags.iter().find(|d| d.id.as_str() == ids::GEOM_TIP_FLOAT)
}

// ── Gates ────────────────────────────────────────────────────────────────

/// GATE 1 — the tool cannot enter the groove, and every surface says so.
#[test]
fn an_unreachable_valley_reports_its_float_on_every_surface() {
    let session = session_with(unreachable_groove(), "unreachable", pencil_op());
    let result = session.get_result(0).expect("generated result");

    // Non-vacuity: the pass must actually have emitted cutting, or "float"
    // would just be describing an empty toolpath.
    assert!(
        result.stats.cutting_distance > 0.0,
        "the pencil pass must emit cutting for its float to mean anything"
    );

    // (a) The typed channel, with its measurement contract attached.
    let (float, provenance) = result
        .stats
        .tip_float_measured()
        .expect("a centreline op ALWAYS measures float — even when the answer is zero");
    println!(
        "D1 tip float: {}/{} points floating, worst {:.4} mm [{}]",
        float.floating_points,
        float.centreline_points,
        float.max_float_mm,
        provenance.describe()
    );
    assert_eq!(provenance.domain, MeasurementDomain::VerticalResidualMm);
    assert_eq!(provenance.stage, MeasurementStage::CentrelineDropSolve);
    assert!(
        float.centreline_points > 0,
        "no centreline points were examined"
    );
    assert!(
        float.floating_points > 0,
        "the Ø3 ball cannot reach a 1.2 mm-wide, 2.5 mm-deep groove — \
         {} of {} points measured as reaching it",
        float.centreline_points - float.floating_points,
        float.centreline_points
    );
    // Closed form: the ball rests on the rims at x = ±0.6, so its tip sits
    // at -(1.5 - √(1.5² - 0.6²)) = -0.1252 mm, 2.3748 mm above the -2.5 mm
    // floor. Allow for the grid-sampled centreline XY and the probe's own
    // (tiny) float, both of which can only make the reading SMALLER.
    let expected = 2.5 - (1.5 - (1.5_f64.powi(2) - 0.6_f64.powi(2)).sqrt());
    assert!(
        float.max_float_mm > 0.9 * expected && float.max_float_mm <= expected + 1e-6,
        "float {:.4} mm should approach the closed-form {expected:.4} mm from below",
        float.max_float_mm
    );

    // (b) Narration — the surface an agent reads.
    let narration = session.narrate_toolpath(0).expect("narrate");
    let line = narration
        .lines()
        .find(|l| l.starts_with("Tip float:"))
        .expect("narration must carry a Tip float line");
    println!("D1 narration: {line}");
    assert!(line.contains("CANNOT reach"), "{line}");
    assert!(
        line.contains("vertical residual depth (mm)"),
        "an unlabelled mm is exactly the cross-domain trap M1 exists for: {line}"
    );

    // (c) The diagnostics list — the surface a router operator reads.
    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    let d = float_diagnostic(&diags).expect("unreachable material must raise a diagnostic");
    println!("D1 diagnostic: {}", d.message);
    assert_eq!(
        d.severity,
        Severity::Caution,
        "report-only: no verdict may change on this"
    );
    assert!(d.message.contains("centreline points"), "{}", d.message);

    // (d) The MCP wire.
    let project = session.diagnostics();
    let row = project
        .per_toolpath
        .iter()
        .find(|r| r.toolpath_id == ToolpathId(0))
        .expect("per-toolpath diagnostic row");
    assert_eq!(row.tip_float_points, Some(float.floating_points));
    assert_eq!(row.max_tip_float_mm, Some(float.max_float_mm));
}

/// GATE 2 — same tool, a groove it CAN bottom out in: measured, and
/// measured clean. This is the half that a stub which always reports float
/// would fail.
#[test]
fn a_reachable_valley_measures_zero_float_rather_than_staying_silent() {
    let session = session_with(reachable_groove(), "reachable", pencil_op());
    let result = session.get_result(0).expect("generated result");
    assert!(
        result.stats.cutting_distance > 0.0,
        "the control must actually cut, or it proves nothing"
    );

    let float = result
        .stats
        .tip_float
        .expect("a centreline op always measures float");
    println!(
        "D1 control: {}/{} points floating, worst {:.4} mm",
        float.floating_points, float.centreline_points, float.max_float_mm
    );
    assert!(float.centreline_points > 0);
    assert_eq!(
        float.floating_points, 0,
        "a Ø3 ball sits on a 2 mm-wide flat floor 1 mm down; nothing may \
         read as floating above {TIP_FLOAT_THRESHOLD_MM} mm"
    );

    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Tip float: none"),
        "narration must state the measured-clean case explicitly:\n{narration}"
    );
    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    assert!(
        float_diagnostic(&diags).is_none(),
        "a reachable valley must not raise the diagnostic"
    );

    let project = session.diagnostics();
    let row = project
        .per_toolpath
        .iter()
        .find(|r| r.toolpath_id == ToolpathId(0))
        .expect("per-toolpath diagnostic row");
    assert_eq!(
        row.tip_float_points,
        Some(0),
        "Some(0) is a MEASURED zero — it must not serialise as null"
    );
}

/// GATE 3 — the X-19 control. An operation that emits no valley centrelines
/// reports **not measured**, never "nothing floated".
#[test]
fn an_operation_without_centrelines_reports_not_measured() {
    let session = session_with(
        unreachable_groove(),
        "unreachable",
        OperationConfig::Waterline(WaterlineConfig::default()),
    );
    let result = session.get_result(0).expect("generated result");
    assert!(
        result.stats.tip_float.is_none(),
        "waterline traces no valley centreline — nothing was measured"
    );
    assert!(
        result.stats.tip_float_measured().is_none(),
        "and no provenance may be claimed for an absent measurement"
    );

    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Tip float: not measured"),
        "absence of a number is not a zero — narration must say which:\n{narration}"
    );

    let project = session.diagnostics();
    let row = project
        .per_toolpath
        .iter()
        .find(|r| r.toolpath_id == ToolpathId(0))
        .expect("per-toolpath diagnostic row");
    assert_eq!(row.tip_float_points, None, "null, never 0");
    assert_eq!(row.max_tip_float_mm, None, "null, never 0.0");
}
