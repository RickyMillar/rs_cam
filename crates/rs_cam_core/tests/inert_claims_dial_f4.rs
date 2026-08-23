//! F4 sentry — a rest-claims dial that steers nothing must SAY SO, and
//! saying so must not move one byte of emitted motion.
//!
//! ## The defect (measured 2026-08-23, multi-tool T4 arm)
//!
//! `UnifiedFinishConfig::min_rest_depth_mm` is consumed in exactly one place:
//! the S4 mask-AND in `unified_finish`'s step 2.6, which is guarded on
//! `territory_clip`. With `territory_clip = false` the keep-mask is never
//! built and the number steers nothing at all.
//!
//! The T4 two-tier arm branched the shipped C2 keeper, gave its fine tier
//! `min_rest_depth_mm = 0.03`, and then sharpened it to `0.05` — above the
//! coarse tier's own cusp height, which should have cut the fine tier's
//! territory hard. The emitted toolpath came back **byte-identical**. The
//! "rest tier" was a full-board R1.0 finish wearing a rest pass's parameters,
//! and it ran 23 916 s before anything said so
//! (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` §0).
//!
//! ## What this file pins, and what it deliberately does NOT
//!
//! **It does not make the dial apply, and it does not refuse.** Shipped
//! projects carry this shape — the C2 keeper itself is `min_rest_depth 0.02`
//! with `claims_reference = auto` and `territory_clip = false` — so applying
//! the mask would silently re-cut them and refusing would black them out.
//! The geometry stays exactly as it was; what changes is that the operator
//! is told which dial is inert and which switch would make it live.
//!
//! * **Kind**: GATE (default CI, synthetic, seconds).
//! * **Fixture**: the M2.1 `two_groove_plateau`, borrowed from
//!   `unified_finish_dropped_band_finding_d1.rs` with pinned heights.
//! * **Report-only**: generation succeeds, severity is `Caution`, no gate
//!   reads the finding, and `emission_is_untouched_by_the_dial_and_by_this_
//!   finding` asserts the emitted move list is identical across the arm that
//!   reports and the arm that does not.
//!
//! ## Red-first evidence
//!
//! Before F4, `ToolpathStats` had no `inert_claims_dial` slot at all, so
//! `reports_when_the_dial_is_set_and_territory_clip_is_off` did not compile;
//! with the slot present but never written it fails on the `expect`, and
//! narration and the diagnostics list contain zero occurrences of
//! `territory_clip` or `config.inert_claims_dial`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
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
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_core::unified_finish::ClaimsReference;

/// A value the operator would plausibly dial and the shipped default is not.
const DIALLED_MM: f64 = 0.05;

// ── Fixture ──────────────────────────────────────────────────────────────

fn tapered_ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: 1.0,
        taper_half_angle: 7.0,
        shaft_diameter: 6.0,
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

fn base_config() -> UnifiedFinishConfig {
    UnifiedFinishConfig {
        steep_threshold_deg: 45.0,
        waterline_threshold_deg: 75.0,
        overlap_mm: 0.0,
        tolerance: 0.05,
        sampling: 0.5,
        scallop_height: 0.15,
        raster_stepover: 1.5,
        z_step: 1.5,
        ..UnifiedFinishConfig::default()
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

fn toolpath(cfg: UnifiedFinishConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
    let op = OperationConfig::UnifiedFinish(cfg);
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: "Unified Finish".to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig {
            top_z: HeightMode::Manual(0.0),
            bottom_z: HeightMode::Manual(-9.0),
            ..HeightsConfig::default()
        },
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: rs_cam_core::gcode::CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    }
}

fn session_with(cfg: UnifiedFinishConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock());
    let tool_idx = session.add_tool(tapered_ball_tool());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(two_groove_plateau()));
    session
        .add_toolpath(0, toolpath(cfg, tool_id, model_id))
        .expect("add unified-finish toolpath");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("UnifiedFinish must still GENERATE — this channel is report-only");
    session
}

/// A deterministic rendering of the emitted motion, for the byte-identity
/// arm. `Move` derives `Debug` and no `PartialEq`, so this is the available
/// exact comparison — target, move type and intent, every move, in order.
fn emitted_motion(session: &ProjectSession) -> String {
    let result = session.get_result(0).expect("generated result");
    format!("{:?}", result.annotated().toolpath.moves)
}

// ── Gates ────────────────────────────────────────────────────────────────

/// GATE 1 — the dial is set, `territory_clip` is off, and every surface says
/// the dial did nothing: the typed channel, narration, and the diagnostics
/// list. The message must name `territory_clip`, because that is the switch
/// the operator has to find.
#[test]
fn reports_when_the_dial_is_set_and_territory_clip_is_off() {
    let session = session_with(UnifiedFinishConfig {
        min_rest_depth_mm: DIALLED_MM,
        territory_clip: false,
        ..base_config()
    });

    // (a) The typed channel.
    let result = session.get_result(0).expect("generated result");
    let finding = result
        .stats
        .inert_claims_dial
        .expect("a dialled min_rest_depth with territory_clip off steers NOTHING — say so");
    assert!(finding.min_rest_depth_inert);
    assert!(
        (finding.min_rest_depth_mm - DIALLED_MM).abs() < 1e-12,
        "the finding must carry the value the operator set"
    );
    assert!(
        (finding.default_min_rest_depth_mm - 0.02).abs() < 1e-12,
        "and the default it was measured against"
    );
    assert!(
        !finding.claims_reference_inert,
        "`claims_reference` was left at its default — only the dial that MOVED is inert"
    );
    assert!(finding.dials().contains("min_rest_depth_mm"));
    assert!(finding.why().contains("territory_clip"));

    // (b) Narration — the surface an agent reads.
    let narration = session.narrate_toolpath(0).expect("narrate");
    let line = narration
        .lines()
        .find(|l| l.starts_with("Inert claims dial:"))
        .expect("narration must carry an Inert claims dial line");
    assert!(line.contains("min_rest_depth_mm"), "{line}");
    assert!(line.contains("territory_clip"), "{line}");

    // (c) The diagnostics list — the surface a router operator reads.
    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    let d = diags
        .iter()
        .find(|d| d.id.as_str() == ids::CONFIG_INERT_CLAIMS_DIAL)
        .expect("an inert dial must raise a diagnostic");
    assert_eq!(
        d.severity,
        Severity::Caution,
        "report-only: no verdict may change on this"
    );
    assert!(d.message.contains("territory_clip"), "{}", d.message);
}

/// GATE 2 — non-vacuity, arm A: the shipped defaults are SILENT.
///
/// This is the C2 keeper's own shape (`min_rest_depth 0.02`,
/// `claims_reference = auto`, `territory_clip = false`). A notice on every
/// unified-finish toolpath is a notice nobody reads, and a keeper that starts
/// warning about a value it never touched is a false alarm.
#[test]
fn the_shipped_defaults_are_silent() {
    let session = session_with(base_config());
    let result = session.get_result(0).expect("generated result");
    assert!(
        result.stats.inert_claims_dial.is_none(),
        "an untouched claims block has nothing to report: {:?}",
        result.stats.inert_claims_dial
    );
    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    let id = ids::CONFIG_INERT_CLAIMS_DIAL;
    let raised = diags.iter().any(|d| d.id.as_str() == id);
    assert!(!raised, "the defaults must raise nothing");

    let narration = session.narrate_toolpath(0).expect("narrate");
    let quiet = !narration.contains("Inert claims dial:");
    assert!(quiet, "and narration must stay quiet too");
}

/// GATE 2 — non-vacuity, arm B: with `territory_clip` ON the dial has a
/// consumer, so nothing is reported inert. The finding is a response to the
/// SWITCH, not to the value.
#[test]
fn territory_clip_on_reports_nothing_inert() {
    let session = session_with(UnifiedFinishConfig {
        min_rest_depth_mm: DIALLED_MM,
        territory_clip: true,
        ..base_config()
    });
    let result = session.get_result(0).expect("generated result");
    assert!(
        result.stats.inert_claims_dial.is_none(),
        "with the S4 mask-AND enabled the dial is live — reporting it inert would be a lie: {:?}",
        result.stats.inert_claims_dial
    );
}

/// GATE 3 — per-dial precision. `claims_reference` is NOT inert merely
/// because `territory_clip` is off: with the claims pipeline running it still
/// chooses which field the crease detector reads, which is the single largest
/// lever on what the operation cuts (A/M6 measured −88.7% cutting from that
/// one choice). It is only reported when `pencil_claims` is off too.
#[test]
fn a_pinned_reference_is_only_inert_when_the_claims_pipeline_is_off() {
    // Claims ON: the reference is live, so nothing is reported.
    let live = session_with(UnifiedFinishConfig {
        pencil_claims: true,
        claims_reference: ClaimsReference::MachinedStock,
        territory_clip: false,
        ..base_config()
    });
    let live_result = live.get_result(0).expect("generated result");
    assert!(
        live_result.stats.inert_claims_dial.is_none(),
        "a running claims pipeline reads `claims_reference` — it is not inert"
    );

    // Claims OFF: nothing reads it at all.
    let dead = session_with(UnifiedFinishConfig {
        pencil_claims: false,
        claims_reference: ClaimsReference::MachinedStock,
        territory_clip: false,
        ..base_config()
    });
    let dead_result = dead.get_result(0).expect("generated result");
    let finding = dead_result
        .stats
        .inert_claims_dial
        .expect("with claims off a pinned reference steers nothing");
    assert!(finding.claims_reference_inert);
    assert!(
        !finding.min_rest_depth_inert,
        "min_rest_depth was left at its default here"
    );
    assert!(finding.dials().contains("claims_reference"));
    assert!(
        finding.why().contains("pencil_claims"),
        "the message must name the stronger cause too: {}",
        finding.why()
    );
}

/// GATE 4 — the whole point: **the report changes no motion.**
///
/// This is the T4 measurement, reproduced as a sentry. The arm that now
/// raises a finding and the arm at the default emit the identical move list,
/// because the dial never had a consumer. If a later wave makes the dial
/// apply, this test fails — which is correct: that is a behavioural change
/// and it must not arrive as a side effect of a report.
#[test]
fn emission_is_untouched_by_the_dial_and_by_this_finding() {
    let reporting = session_with(UnifiedFinishConfig {
        min_rest_depth_mm: DIALLED_MM,
        territory_clip: false,
        ..base_config()
    });
    let silent = session_with(base_config());

    let a_result = reporting.get_result(0).expect("generated result");
    let b_result = silent.get_result(0).expect("generated result");
    assert!(
        a_result.stats.inert_claims_dial.is_some(),
        "the reporting arm must actually report, or this proves nothing"
    );
    assert!(b_result.stats.inert_claims_dial.is_none());

    let a = emitted_motion(&reporting);
    let b = emitted_motion(&silent);
    assert!(
        a.len() > 100,
        "a fixture that emits nothing cannot prove emission is unchanged"
    );
    assert_eq!(
        a, b,
        "the dial steers nothing and the finding steers nothing: the emitted \
         motion must be identical"
    );
}
