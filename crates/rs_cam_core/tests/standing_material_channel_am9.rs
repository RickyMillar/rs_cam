//! A/M9 sentry — standing material must have a diagnostic CHANNEL, not a
//! `tracing::warn!` nobody installs a subscriber for.
//!
//! Background (`planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`
//! §A/M9; `MEASUREMENT_DOMAINS.md` rows 37-42 and X-19): the scallop ring
//! cascade has always measured the region interior it failed to reach
//! (`ScallopReport::uncut_core_mm2`), and `0d1f307` carried that figure to
//! `ToolpathStats` plus the `geom.standing_material` diagnostic. Two gaps
//! remained, and this file pins both shut:
//!
//! 1. `narrate_toolpath` — the agent-facing surface — never mentioned it, so
//!    an agent narrating a truncated cascade saw nothing at all.
//! 2. `ToolpathStats::standing_material_mm2` was a bare `f64` whose `0.0`
//!    meant BOTH "a cascade ran and left nothing" and "no cascade ran, so
//!    nothing was measured" (X-19, the silent-zero trap). Any "% left
//!    standing" a reader built on the second case was unfounded.
//!
//! Everything here runs the REAL production path — `ProjectSession::
//! generate_toolpath`, the entry point the GUI worker and the CLI share.
//!
//! Report-only by design: nothing gates on the figure and no verdict moves.

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
    BoundaryConfig, DressupConfig, HeightsConfig, STANDING_MATERIAL_DOMAIN,
    STANDING_MATERIAL_RESOLUTION, STANDING_MATERIAL_STAGE, StockSource,
};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern, ScallopConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::diagnostics::{Diagnostic, ids};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::{TriangleMesh, make_test_flat};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};

// ── Fixtures ────────────────────────────────────────────────────────────

fn ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: 3.0,
        ..ToolConfig::new_default(ToolId(0), ToolType::BallNose)
    }
}

/// Tapered ball: the class where `radius()` (shank) and `cusp_radius()`
/// (tip sphere) diverge. Used on the CONTROL fixture — the cheap half of
/// the ball/tapered pair — to prove the channel is wired for it too.
fn tapered_ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: 1.0,
        taper_half_angle: 10.0,
        shaft_diameter: 6.0,
        ..ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose)
    }
}

/// Corrugated plate — a triangular-wave surface with sharp convex apexes.
///
/// This is the deliberate truncation. `max_rings` is budgeted from the
/// FLAT-ground stepover, but `ring_stepover` takes the MIN across each
/// ring's samples, and a sharp convex apex on the tool-CENTRE surface
/// shrinks the cusp-limited advance by ~25%. Every ring therefore advances
/// slower than the budget assumed, and the cascade runs out of rings with
/// the region interior still standing — the mechanism `scallop.rs`
/// documents and wanaka hit for real.
///
/// The texture has to be coarse enough that the ball FOLLOWS it: a ball
/// bridges any feature below its own radius and the offset surface reads
/// flat (`finish_setup.rs`'s documented blind spot), which is why the
/// period is ~1.3x the tool diameter rather than as fine as possible.
fn sawtooth_plate(half: f64, period: f64, amplitude: f64) -> TriangleMesh {
    let step_x = period / 16.0;
    let step_y = 2.0;
    let nx = ((2.0 * half) / step_x).round() as usize + 1;
    let ny = ((2.0 * half) / step_y).round() as usize + 1;
    let mut vertices = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        let y = -half + j as f64 * step_y;
        for i in 0..nx {
            let x = -half + i as f64 * step_x;
            let phase = x / period - (x / period).floor();
            let ridge = 1.0 - (2.0 * phase - 1.0).abs();
            vertices.push(P3::new(x, y, amplitude * ridge));
        }
    }
    let mut triangles = Vec::with_capacity((nx - 1) * (ny - 1) * 2);
    for j in 0..(ny - 1) {
        for i in 0..(nx - 1) {
            let a = (j * nx + i) as u32;
            let b = a + 1;
            let c = ((j + 1) * nx + i) as u32;
            let d = c + 1;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

fn mesh_model(mesh: TriangleMesh, name: &str) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: name.to_owned(),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from(format!("synthetic://{name}.stl")),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn polygon_model(name: &str) -> LoadedModel {
    let square = Polygon2::new(vec![
        P2::new(-10.0, -10.0),
        P2::new(10.0, -10.0),
        P2::new(10.0, 10.0),
        P2::new(-10.0, 10.0),
    ]);
    LoadedModel {
        id: 0,
        name: name.to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![square])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from(format!("synthetic://{name}.svg")),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn toolpath(name: &str, op: OperationConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: name.to_owned(),
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

fn stock_over(half: f64, height: f64) -> StockConfig {
    StockConfig {
        x: 2.0 * half + 4.0,
        y: 2.0 * half + 4.0,
        z: height,
        origin_x: -half - 2.0,
        origin_y: -half - 2.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn scallop_op(scallop_height: f64) -> OperationConfig {
    OperationConfig::Scallop(ScallopConfig {
        scallop_height,
        tolerance: 0.1,
        ..ScallopConfig::default()
    })
}

/// The deliberately-truncated cascade of the A/M9 acceptance gate.
///
/// Measured at the time of writing: `max_rings = 119`, 120 rings emitted,
/// 13.3 mm² left standing.
fn truncated_cascade_session() -> ProjectSession {
    let half = 25.0;
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_over(half, 1.5));
    let tool_idx = session.add_tool(ball_tool());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(sawtooth_plate(half, 2.0, 1.0), "corrugation"));
    session
        .add_toolpath(0, toolpath("Scallop", scallop_op(0.005), tool_id, model_id))
        .expect("add scallop toolpath");
    session
}

/// Control: the same operation on flat ground, where the cascade collapses
/// well inside its cap. Cheap, and the other half of the X-19 distinction.
fn collapsing_cascade_session(tool: ToolConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_over(25.0, 1.0));
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(make_test_flat(50.0), "plate"));
    session
        .add_toolpath(0, toolpath("Scallop", scallop_op(0.1), tool_id, model_id))
        .expect("add scallop toolpath");
    session
}

/// Control: an operation family that runs no ring cascade at all, so the
/// measure does not exist for it.
fn no_cascade_session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_over(15.0, 10.0));
    let tool_idx = session.add_tool(ToolConfig {
        diameter: 3.0,
        ..ToolConfig::new_default(ToolId(0), ToolType::EndMill)
    });
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(polygon_model("square"));
    let op = OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 2.0,
        depth_per_pass: 2.0,
        feed_rate: 800.0,
        plunge_rate: 400.0,
        climb: true,
        pattern: PocketPattern::Contour,
        angle: 0.0,
        finishing_passes: 0,
        spindle_rpm: Some(18_000),
    });
    session
        .add_toolpath(0, toolpath("Pocket", op, tool_id, model_id))
        .expect("add pocket toolpath");
    session
}

fn generate(session: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generation must succeed");
}

fn measured(session: &ProjectSession) -> Option<f64> {
    session
        .get_result(0)
        .expect("generated result")
        .stats
        .standing_material_mm2
}

fn standing_diagnostic(diags: &[Diagnostic]) -> Option<&Diagnostic> {
    diags
        .iter()
        .find(|d| d.id.as_str() == ids::GEOM_STANDING_MATERIAL)
}

/// Every user-visible standing-material string must declare what it
/// measured, when, and at what resolution (M1). A bare `mm²` is exactly the
/// unlabelled area the audit found being compared across domains.
fn declares_domain_stage_and_resolution(text: &str) {
    for needle in [
        STANDING_MATERIAL_DOMAIN,
        STANDING_MATERIAL_STAGE,
        STANDING_MATERIAL_RESOLUTION,
    ] {
        assert!(
            text.contains(needle),
            "standing-material text must declare {needle:?}:\n{text}"
        );
    }
}

// ── Acceptance ──────────────────────────────────────────────────────────

/// A/M9 acceptance gate, end to end on one generation: a deliberately
/// truncated cascade produces a non-zero standing-material figure that is
/// visible in `ToolpathStats`, in `narrate_toolpath`, in the diagnostics
/// list and in the MCP per-toolpath summary — each declaring domain, stage
/// and resolution — and it changes no verdict.
///
/// One test, one generation: the fixture costs ~9 s, and splitting the
/// assertions would multiply that by the assertion count.
#[test]
fn truncated_cascade_is_visible_on_every_surface() {
    let mut session = truncated_cascade_session();
    generate(&mut session);

    // 1. Stats — `Some`, because a cascade ran and MEASURED this.
    let area = measured(&session)
        .expect("a scallop cascade measured its residual — this must not be `None`");
    println!("truncated cascade: standing material {area:.1} mm²");
    assert!(
        area > 1.0,
        "the corrugation forces every ring below the flat-ground budget, so the \
         cascade cannot reach the interior; got {area} mm²"
    );

    // 2. Narration — the agent-facing surface that was silent before A/M9.
    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Standing material:"),
        "narration must carry the figure:\n{narration}"
    );
    assert!(
        narration.contains(&format!("{area:.0} mm²")),
        "narration must print the measured area itself, not just a label:\n{narration}"
    );
    declares_domain_stage_and_resolution(&narration);

    // 3. Diagnostics — the GUI ribbon and MCP `get_toolpath_diagnostics`.
    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    let d = standing_diagnostic(&diags).expect("geom.standing_material diagnostic");
    assert!(
        d.message.contains(&format!("{area:.0} mm²")),
        "diagnostic must print the measured area: {}",
        d.message
    );
    declares_domain_stage_and_resolution(&d.message);

    // 4. Report-only (the A/M9 gate: report before enforcing).
    assert_ne!(
        d.severity,
        rs_cam_core::diagnostics::Severity::Blocking,
        "standing material must not block export yet"
    );
    assert_eq!(d.category, rs_cam_core::diagnostics::Category::Geometry);
    assert!(
        session.get_result(0).expect("result").stats.move_count > 0,
        "generation still succeeds — this is a report, not a rejection"
    );

    // 5. The MCP per-toolpath summary carries the number, not just prose.
    let project = session.diagnostics();
    let summary = project
        .per_toolpath
        .iter()
        .find(|t| t.toolpath_id == ToolpathId(0) || t.name == "Scallop")
        .expect("per-toolpath summary");
    assert_eq!(
        summary.standing_material_mm2,
        Some(area),
        "the summary must carry the same measurement, not a re-derivation"
    );
}

/// X-19, half one: `Some(0.0)` means "measured, nothing standing". A
/// cascade that collapses inside its cap raises no diagnostic and narrates
/// as a measured none — not as silence.
#[test]
fn collapsing_cascade_reports_a_measured_zero() {
    let mut session = collapsing_cascade_session(ball_tool());
    generate(&mut session);

    assert_eq!(
        measured(&session),
        Some(0.0),
        "a collapsing cascade MEASURED zero; `None` would claim it was never measured"
    );

    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Standing material: none"),
        "a measured zero must read as a measurement:\n{narration}"
    );
    assert!(
        standing_diagnostic(
            &session
                .diagnose_toolpath_with_trace(0, None)
                .expect("diagnose")
        )
        .is_none(),
        "nothing is standing — the list stays quiet"
    );
}

/// The tapered-ball control. Cheap half of the ball/tapered pair: the tool
/// class whose cusp and envelope radii diverge must reach the same channel.
#[test]
fn tapered_ball_reaches_the_same_channel() {
    let mut session = collapsing_cascade_session(tapered_ball_tool());
    generate(&mut session);

    assert_eq!(
        measured(&session),
        Some(0.0),
        "the tapered cascade ran and measured — the channel is not ball-only"
    );
    assert!(
        session
            .narrate_toolpath(0)
            .expect("narrate")
            .contains("Standing material: none")
    );
}

/// X-19, half two: an operation with no ring cascade reports `None` (not
/// measured), never `0.0`. Any ratio built on the latter is unfounded, and
/// narration must say so out loud rather than omitting the line.
#[test]
fn operation_without_a_cascade_reports_not_measured() {
    let mut session = no_cascade_session();
    generate(&mut session);

    assert_eq!(
        measured(&session),
        None,
        "a pocket runs no ring cascade — the measure does not exist for it"
    );

    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Standing material: not measured"),
        "narration must say the measure is absent rather than imply zero:\n{narration}"
    );
    assert!(
        standing_diagnostic(
            &session
                .diagnose_toolpath_with_trace(0, None)
                .expect("diagnose")
        )
        .is_none(),
        "an unmeasured cascade is not a defect claim"
    );

    let project = session.diagnostics();
    assert!(
        project
            .per_toolpath
            .iter()
            .all(|t| t.standing_material_mm2.is_none()),
        "the MCP summary must serialise `null`, not 0.0, for an unmeasured op"
    );
}
