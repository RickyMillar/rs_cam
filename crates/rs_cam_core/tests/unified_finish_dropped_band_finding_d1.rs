//! Wave D1 sentry — a `UnifiedFinish` band whose planned cutting is
//! entirely erased by height resolution must SAY SO.
//!
//! ## The defect (found by the M2.1 agent, ledger task #15)
//!
//! `UnifiedFinishConfig::depth_semantics()` is `DepthSemantics::None`, so
//! `HeightContext::op_depth` is `0.0`. `HeightsConfig::resolve` then makes an
//! Auto `bottom_z` resolve to `top_z - 0.0` — i.e. `bottom_z == top_z ==
//! stock_top_z`. The `VerySteep` arm of `unified_finish_toolpath_with_cancel`
//! ladders `start_z = band_max_z.min(top_z)` down to
//! `final_z = band_min_z.max(bottom_z)`, so with those Auto heights `final_z`
//! is pinned at the STOCK TOP and the whole waterline ladder collapses to at
//! most the rim contour. On the M2.1 two-groove plateau that leaves the 85°
//! groove — a real feature with real area — completely unmachined, and the
//! only trace was `region_count += 1` in a struct nothing prints.
//!
//! **This file does not fix that.** Changing `depth_semantics` is a
//! behavioural change and belongs to a behavioural wave. What Wave D1 owns is
//! the instrument: an unmachined feature must never again be silent.
//!
//! ## Contract
//!
//! * **Kind**: GATE (default CI, synthetic, seconds).
//! * **Fixture**: the M2.1 `two_groove_plateau` mesh, deliberately run with
//!   `HeightsConfig::default()` (Auto/Auto). M2.1 pins `top_z`/`bottom_z`
//!   precisely to AVOID this trap and says so in its own comment; this file
//!   does the opposite on purpose, and the `pinned_heights_*` control proves
//!   the finding is a response to the heights and not to the mesh.
//! * **Report-only**: generation still succeeds, the verdict severity stays
//!   `Caution`, and no gate consumes the number.
//!
//! ## Red-first evidence
//!
//! At `e9f9554`, on the Auto-heights run below:
//! `session.generate_toolpath` succeeded, the VerySteep band produced no
//! region node at all, and `narrate_toolpath` / `diagnose_toolpath_with_trace`
//! contained ZERO occurrences of "unmachined", "dropped band" or
//! `geom.unmachined_band`. Recorded in the commit body.

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
    BoundaryConfig, DressupConfig, HeightMode, HeightsConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::diagnostics::{Diagnostic, Severity, ids};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};

// ── Fixture (adapted verbatim from `unified_finish_tapered_end_to_end_m21`) ──

const TIP_DIAMETER_MM: f64 = 1.0;
const SHANK_DIAMETER_MM: f64 = 6.0;
const TAPER_HALF_ANGLE_DEG: f64 = 7.0;

fn tapered_ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: TIP_DIAMETER_MM,
        taper_half_angle: TAPER_HALF_ANGLE_DEG,
        shaft_diameter: SHANK_DIAMETER_MM,
        ..ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose)
    }
}

fn extrude_profile(profile: &[(f64, f64)], y0: f64, y1: f64) -> TriangleMesh {
    let mut vertices = Vec::with_capacity(profile.len() * 2);
    for &(x, z) in profile {
        vertices.push(P3::new(x, y0, z));
        vertices.push(P3::new(x, y1, z));
    }
    let mut triangles = Vec::with_capacity((profile.len() - 1) * 2);
    for i in 0..profile.len() - 1 {
        let (a, b) = (2 * i as u32, 2 * i as u32 + 2);
        let (c, d) = (2 * i as u32 + 3, 2 * i as u32 + 1);
        triangles.push([a, b, c]);
        triangles.push([a, c, d]);
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// 30 × 30 mm plateau, a 60° mid-steep groove and an 85° very-steep groove.
fn two_groove_plateau() -> TriangleMesh {
    let profile = [
        (-15.0_f64, 0.0_f64),
        (-6.0, 0.0),
        (-5.0, -1.732_050_8),
        (-4.0, 0.0),
        (4.0, 0.0),
        (4.75, -8.572_539),
        (5.5, 0.0),
        (15.0, 0.0),
    ];
    extrude_profile(&profile, -15.0, 15.0)
}

fn mesh_model(mesh: TriangleMesh) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: "two_groove_plateau".to_owned(),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://two_groove_plateau.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn unified_finish_op() -> OperationConfig {
    OperationConfig::UnifiedFinish(UnifiedFinishConfig {
        steep_threshold_deg: 45.0,
        waterline_threshold_deg: 75.0,
        overlap_mm: 0.0,
        tolerance: 0.05,
        sampling: 0.5,
        scallop_height: 0.15,
        raster_stepover: 1.5,
        z_step: 1.5,
        ..UnifiedFinishConfig::default()
    })
}

fn toolpath(heights: HeightsConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
    let op = unified_finish_op();
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: "Unified Finish".to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights,
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
        x: 34.0,
        y: 34.0,
        z: 9.0,
        origin_x: -17.0,
        origin_y: -17.0,
        origin_z: -9.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn session_with(heights: HeightsConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock());
    let tool_idx = session.add_tool(tapered_ball_tool());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(two_groove_plateau()));
    session
        .add_toolpath(0, toolpath(heights, tool_id, model_id))
        .expect("add unified-finish toolpath");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("UnifiedFinish must still GENERATE — this channel is report-only");
    session
}

/// `HeightsConfig::default()` — every level Auto. The trap.
fn auto_heights() -> HeightsConfig {
    HeightsConfig::default()
}

/// M2.1's pinned pair — the control.
fn pinned_heights() -> HeightsConfig {
    HeightsConfig {
        top_z: HeightMode::Manual(0.0),
        bottom_z: HeightMode::Manual(-9.0),
        ..HeightsConfig::default()
    }
}

fn unmachined_diagnostic(diags: &[Diagnostic]) -> Option<&Diagnostic> {
    diags
        .iter()
        .find(|d| d.id.as_str() == ids::GEOM_UNMACHINED_BAND)
}

// ── Gates ────────────────────────────────────────────────────────────────

/// GATE 1 — with Auto heights the very-steep groove is dropped, and every
/// user-visible surface says so: stats, narration, diagnostics, and the
/// project-level `ToolpathDiagnostic`.
#[test]
fn auto_heights_drop_the_very_steep_band_and_every_surface_reports_it() {
    let session = session_with(auto_heights());

    // (a) The typed channel. `Some` = measured; the band, its area and the
    //     clipping height all travel with it.
    let result = session.get_result(0).expect("generated result");
    let dropped = result
        .stats
        .dropped_band
        .as_deref()
        .expect("Auto heights collapse the VerySteep ladder — this MUST be measured");
    println!(
        "D1 measured: band={} regions={} area={:.2} mm² clipped at Z{:.3} ({})",
        dropped.band_label,
        dropped.region_count,
        dropped.area_mm2,
        dropped.clip_z_mm,
        dropped.clip_label
    );
    assert_eq!(
        dropped.band_label, "VerySteep",
        "the 85° groove is the very-steep band"
    );
    assert!(dropped.region_count >= 1);
    assert!(
        dropped.area_mm2 > 1.0,
        "a dropped band with no area is not a feature: {}",
        dropped.area_mm2
    );
    // Auto `bottom_z` resolves to `top_z - op_depth` = the stock top (0.0).
    assert!(
        (dropped.clip_z_mm - 0.0).abs() < 1e-6,
        "the clipping height IS the resolved bottom_z: {}",
        dropped.clip_z_mm
    );
    assert_eq!(dropped.clip_label, "bottom_z");

    // (b) Narration — the surface an agent reads.
    let narration = session.narrate_toolpath(0).expect("narrate");
    let line = narration
        .lines()
        .find(|l| l.starts_with("Unmachined band:"))
        .expect("narration must carry an Unmachined band line");
    println!("D1 narration: {line}");
    assert!(line.contains("VerySteep"), "{line}");
    assert!(line.contains("bottom_z"), "{line}");

    // (c) The diagnostics list — the surface a router operator reads.
    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    let d = unmachined_diagnostic(&diags).expect("an unmachined band must raise a diagnostic");
    println!("D1 diagnostic: {}", d.message);
    assert_eq!(
        d.severity,
        Severity::Caution,
        "report-only: no verdict may change on this"
    );
    assert!(d.message.contains("VerySteep"), "{}", d.message);
    // M1: an area with no declared domain is the cross-domain comparison the
    // audit found. The band polygon area is XY-PROJECTED.
    assert!(d.message.contains("XY-projected area"), "{}", d.message);

    // (d) The project-level per-toolpath diagnostic (the MCP wire).
    let project = session.diagnostics();
    let row = project
        .per_toolpath
        .iter()
        .find(|r| r.toolpath_id == ToolpathId(0))
        .expect("per-toolpath diagnostic row");
    assert!(
        row.unmachined_band_area_mm2.is_some_and(|a| a > 1.0),
        "the MCP wire must carry the area, got {:?}",
        row.unmachined_band_area_mm2
    );
}

/// GATE 2 — non-vacuity: the finding is a response to the HEIGHTS, not to
/// the mesh. Pin the heights the way M2.1 does and the same fixture reports
/// nothing dropped, while actually cutting the very-steep groove.
#[test]
fn pinned_heights_machine_the_band_and_report_nothing_dropped() {
    let session = session_with(pinned_heights());
    let result = session.get_result(0).expect("generated result");
    assert!(
        result.stats.dropped_band.is_none(),
        "pinned heights machine the band — nothing may be reported dropped: {:?}",
        result.stats.dropped_band
    );
    assert!(
        result.stats.cutting_distance > 0.0,
        "control must actually cut, or it proves nothing"
    );

    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Unmachined band: none"),
        "narration must state the measured-clean case explicitly, not stay silent"
    );

    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    assert!(
        unmachined_diagnostic(&diags).is_none(),
        "a clean run must not raise the diagnostic"
    );
}

/// GATE 3 — the X-19 silent-zero control. An operation that plans no bands
/// at all reports **not measured**, never "nothing dropped".
#[test]
fn an_operation_with_no_bands_reports_not_measured() {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock());
    let tool_idx = session.add_tool(tapered_ball_tool());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(two_groove_plateau()));
    let mut tc = toolpath(pinned_heights(), tool_id, model_id);
    tc.operation = OperationConfig::Waterline(
        rs_cam_core::compute::operation_configs::WaterlineConfig::default(),
    );
    tc.dressups = DressupConfig::for_op(tc.operation.op_type());
    session.add_toolpath(0, tc).expect("add waterline toolpath");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("waterline must generate");

    let result = session.get_result(0).expect("generated result");
    assert!(
        result.stats.dropped_band.is_none(),
        "a non-banded op measures nothing"
    );
    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Unmachined band: not measured"),
        "absence of a number is not a zero — narration must say which:\n{narration}"
    );
}
