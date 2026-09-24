//! Unit tests for operation execution. Moved out of `compute/execute.rs` by
//! P4; the module body is unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use crate::compute::config::{DressupConfig, DressupEntryStyle};
use crate::tool::MillingCutter;
use crate::trace::semantic_trace::SemanticKey;
use crate::trace::transform_provenance::ReconcileSet;

use crate::toolpath::Toolpath;

use super::drilling::drill_holes_for_config;
use super::findings::record_derived_stepover;
use super::finish_3d::adaptive3d_effective_stock_to_leave;
use super::*;

// ── C8: findings collections ────────────────────────────────────────

fn stepover_finding(
    site: &'static str,
    mm: f64,
) -> crate::compute::toolpath_stats::DerivedStepoverFinding {
    crate::compute::toolpath_stats::DerivedStepoverFinding {
        site,
        stepover_mm: mm,
        reference_depth_mm: 0.5,
        reference_depth_basis: "test",
        envelope_rule_mm: mm * 3.0,
        slope_derate: None,
    }
}

/// C8, red-first against the pre-wave code: `record_derived_stepover`
/// returned early when the slot was occupied, so the SECOND derivation
/// — PR-7's generic rest-analysis post-pass — was discarded with no
/// trace. This asserts both survive, in the order they fired, each
/// carrying its own `site`.
#[test]
fn two_derivations_on_one_toolpath_are_both_recorded() {
    let cell = std::cell::RefCell::new(GenerationFindings::default());
    assert!(
        cell.borrow().derived_stepovers.is_empty(),
        "empty means nothing derived — the 'not measured' state"
    );

    record_derived_stepover(
        &cell,
        stepover_finding("UnifiedFinish crease/pencil claims", 0.20),
    );
    record_derived_stepover(
        &cell,
        stepover_finding("generic rest analysis routing", 0.35),
    );

    let findings = cell.into_inner();
    assert_eq!(
        findings.derived_stepovers.len(),
        2,
        "first-writer-wins would have kept only one: {:?}",
        findings.derived_stepovers
    );
    // Emission order is the firing order: the operation's own site runs
    // during generation, the post-pass afterwards.
    assert_eq!(
        findings.derived_stepovers[0].site,
        "UnifiedFinish crease/pencil claims"
    );
    assert_eq!(
        findings.derived_stepovers[1].site,
        "generic rest analysis routing"
    );
    assert!((findings.derived_stepovers[0].stepover_mm - 0.20).abs() < 1e-12);
    assert!((findings.derived_stepovers[1].stepover_mm - 0.35).abs() < 1e-12);
}

/// The diagnostic adapter fans out per derivation, and still stays quiet
/// on the ones that agree with the retired envelope rule — the
/// per-derivation form of the "a notice on every toolpath is a notice
/// nobody reads" rule. Before C8 the whole operation's diagnostic was
/// decided by whichever derivation happened to be recorded first, so a
/// noteworthy second one could be silenced by an unremarkable first.
#[test]
fn the_diagnostic_adapter_reports_each_derivation_separately() {
    let mut stats = crate::compute::toolpath_stats::ToolpathStats::default();
    // First agrees with the envelope rule (a plain ball) — silent.
    stats
        .derived_stepovers
        .push(crate::compute::toolpath_stats::DerivedStepoverFinding {
            envelope_rule_mm: 0.20,
            ..stepover_finding("quiet site", 0.20)
        });
    // Second does not — must be reported even though the first was not.
    stats
        .derived_stepovers
        .push(stepover_finding("loud site", 0.35));

    let out = crate::diagnostics::adapters::from_generation::diagnostics_from_generation(
        crate::ids::ToolpathId(0),
        &stats,
    );
    let stepover_diags: Vec<_> = out
        .iter()
        .filter(|d| d.id.as_str() == crate::diagnostics::ids::CONFIG_DERIVED_STEPOVER)
        .collect();
    assert_eq!(
        stepover_diags.len(),
        1,
        "one diagnostic per NOTEWORTHY derivation: {stepover_diags:?}"
    );
    assert!(
        stepover_diags[0].message.contains("loud site"),
        "the reported one must be the second: {}",
        stepover_diags[0].message
    );
}
use std::sync::atomic::AtomicBool;

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::config::ResolvedHeights;
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::geo::{BoundingBox3, P3};
use crate::mesh::{SpatialIndex, TriangleMesh, make_test_flat, make_test_hemisphere};
use crate::polygon::Polygon2;
use crate::trace::toolpath_spans::SpanKind;

#[test]
fn drill_holes_come_from_targets_or_picks_never_centroids() {
    use crate::compute::operation_configs::DrillConfig;
    use crate::io::dxf_input::DrillTargetKind;
    // Two targets, as a DXF with one POINT and one CIRCLE imports to.
    let targets = [
        DrillTarget {
            x: 1.0,
            y: 2.0,
            layer: "pts".to_owned(),
            kind: DrillTargetKind::Point,
        },
        DrillTarget {
            x: 7.0,
            y: 8.0,
            layer: "holes".to_owned(),
            kind: DrillTargetKind::CircleCenter { diameter: 4.0 },
        },
    ];

    // None => every target the model exposes.
    let default = DrillConfig::default();
    let holes = drill_holes_for_config(&default, &targets, None).unwrap();
    assert_eq!(holes, vec![[1.0, 2.0], [7.0, 8.0]]);

    // None + no targets => refusal naming the missing input, never a
    // polygon centroid (G-DRILLCENTROID).
    let err = drill_holes_for_config(&default, &[], None).unwrap_err();
    assert!(
        matches!(&err, OperationError::MissingGeometry(m) if m == NO_DRILL_TARGETS_MSG),
        "got {err:?}"
    );

    // Some(picks) => drill exactly the picks.
    //
    // This arm read `selected_holes: Some(vec![[3.0, 4.0]])` — a
    // coordinate naming NEITHER target — and asserted the op drilled it
    // "ignoring the targets". That is the G-DRILLPICKSTALE defect stated
    // as a contract: a pick is a copy of a target's own `(x, y)`, so a
    // pick that names no target is a pick whose target moved or was
    // deleted. The pick here now names a real target, which is what the
    // picker can produce; the stale case is the refusal below.
    let picked = DrillConfig {
        selected_holes: Some(vec![[7.0, 8.0]]),
        ..DrillConfig::default()
    };
    let holes = drill_holes_for_config(&picked, &targets, None).unwrap();
    assert_eq!(holes, vec![[7.0, 8.0]]);

    // Some(picks) naming no target => refusal, never the frozen
    // coordinate (G-DRILLPICKSTALE, F4.8).
    let stale = DrillConfig {
        selected_holes: Some(vec![[3.0, 4.0]]),
        ..DrillConfig::default()
    };
    let err = drill_holes_for_config(&stale, &targets, None).unwrap_err();
    let OperationError::MissingGeometry(msg) = &err else {
        panic!("expected MissingGeometry, got {err:?}");
    };
    assert!(msg.contains(STALE_DRILL_PICKS_PHRASE), "got {msg:?}");

    // The same pick with NO targets to compare against still drills:
    // an empty list also means "the caller resolved no model", which is
    // what `execute_operation_annotated` passes.
    let holes = drill_holes_for_config(&stale, &[], None).unwrap();
    assert_eq!(holes, vec![[3.0, 4.0]]);

    // Some(empty) => explicit "nothing selected" error, not all targets.
    let empty = DrillConfig {
        selected_holes: Some(Vec::new()),
        ..DrillConfig::default()
    };
    let err = drill_holes_for_config(&empty, &targets, None).unwrap_err();
    assert!(
        matches!(&err, OperationError::MissingGeometry(m) if m == NO_DRILL_TARGETS_SELECTED_MSG),
        "got {err:?}"
    );
}

/// F-XXX regression, re-based by L2: adaptive3d's planner supports a
/// vertical (Z) leave and nothing else. The reader passes the axial
/// dial through untouched. It must never raise the value — the
/// deleted radial dial used to bleed into it through a `max()`.
#[test]
fn adaptive3d_stock_to_leave_is_axial_only() {
    use crate::compute::operation_configs::Adaptive3dConfig;

    let no_leave = Adaptive3dConfig {
        stock_to_leave_axial: 0.0,
        ..Adaptive3dConfig::default()
    };
    assert_eq!(adaptive3d_effective_stock_to_leave(&no_leave), 0.0);

    let axial_only = Adaptive3dConfig {
        stock_to_leave_axial: 0.3,
        ..Adaptive3dConfig::default()
    };
    assert_eq!(adaptive3d_effective_stock_to_leave(&axial_only), 0.3);
}

/// Build a default tool definition and config for a given tool type.
/// A default `Drill` op with an explicit pick. The in-file tests reach
/// the generator without a model, so the picks are their hole source
/// (G-DRILLCENTROID: polygon centroids no longer are).
fn drill_op_picking(holes: &[[f64; 2]]) -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::Drill);
    if let OperationConfig::Drill(cfg) = &mut op {
        cfg.selected_holes = Some(holes.to_vec());
    }
    op
}

fn make_tool(tool_type: ToolType) -> (crate::tool::ToolDefinition, ToolConfig) {
    let cfg = ToolConfig::new_default(ToolId(0), tool_type);
    let def = build_cutter(&cfg);
    (def, cfg)
}

/// Sensible resolved heights for testing.
fn test_heights() -> ResolvedHeights {
    ResolvedHeights {
        clearance_z: 40.0,
        retract_z: 30.0,
        feed_z: 28.0,
        top_z: 25.0,
        bottom_z: 0.0,
        top_pinned: false,
        bottom_pinned: false,
    }
}

/// A stock bbox suitable for most tests.
fn test_stock_bbox() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(100.0, 100.0, 25.0),
    }
}

// F.3: CCW winding so the wall facets face UP/outward (+Z normals) — a
// valid machinable surface, matching pencil.rs's own corrected copy of
// this fixture. The original winding produced downward normals, which
// drop_cutter rightly skips — starving the Pencil span-coverage case of
// any contacted geometry (the pre-existing red at HEAD, see F.3 in
// planning/finishing_stack_review_2026-07.md).
fn make_v_groove_mesh(length: f64, depth: f64, width: f64) -> TriangleMesh {
    TriangleMesh::from_raw(
        vec![
            P3::new(0.0, -width, 0.0),
            P3::new(length, -width, 0.0),
            P3::new(0.0, 0.0, -depth),
            P3::new(length, 0.0, -depth),
            P3::new(0.0, width, 0.0),
            P3::new(length, width, 0.0),
        ],
        vec![[0, 1, 2], [1, 3, 2], [2, 3, 4], [3, 5, 4]],
    )
}

#[derive(Clone, Copy)]
enum MeshFixture {
    Flat,
    Hemisphere,
    Groove,
}

#[derive(Clone, Copy)]
enum PolygonFixture {
    Standard,
    Drill,
    ProjectCurve,
}

struct SpanCoverageCase {
    name: &'static str,
    op: OperationConfig,
    tool_type: ToolType,
    mesh: Option<MeshFixture>,
    polygons: Option<PolygonFixture>,
    prev_tool_radius: Option<f64>,
    expected_kinds: Vec<SpanKind>,
    forbidden_kinds: Vec<SpanKind>,
    expected_label_fragments: Vec<&'static str>,
}

fn op_with_updates(
    op_type: OperationType,
    update: impl FnOnce(&mut OperationConfig),
) -> OperationConfig {
    let mut op = OperationConfig::new_default(op_type);
    update(&mut op);
    op
}

fn span_coverage_cases() -> Vec<SpanCoverageCase> {
    let depth_expected = vec![
        SpanKind::RapidOrderBarrier,
        SpanKind::DepthPass,
        SpanKind::Region,
    ];
    let region_expected = vec![SpanKind::Region];
    let drill_expected = vec![SpanKind::Region];

    let adaptive3d = op_with_updates(OperationType::Adaptive3d, |op| {
        let OperationConfig::Adaptive3d(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.depth_per_pass = 4.0;
        cfg.detect_flat_areas = true;
        cfg.region_ordering = crate::compute::operation_configs::RegionOrdering::ByArea;
    });
    let alignment_pin = op_with_updates(OperationType::AlignmentPinDrill, |op| {
        let OperationConfig::AlignmentPinDrill(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.holes = vec![[25.0, 25.0], [55.0, 55.0]];
    });
    let drop_cutter = op_with_updates(OperationType::DropCutter, |op| {
        let OperationConfig::DropCutter(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.stepover = 2.0;
        cfg.min_z = -5.0;
    });
    let waterline = op_with_updates(OperationType::Waterline, |op| {
        let OperationConfig::Waterline(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.z_step = 2.0;
        cfg.sampling = 1.0;
    });
    let scallop = op_with_updates(OperationType::Scallop, |op| {
        let OperationConfig::Scallop(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.scallop_height = 0.2;
        cfg.tolerance = 0.2;
        cfg.continuous = true;
    });
    let ramp_finish = op_with_updates(OperationType::RampFinish, |op| {
        let OperationConfig::RampFinish(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.max_stepdown = 2.0;
        cfg.sampling = 2.0;
        cfg.tolerance = 0.2;
    });
    let spiral_finish = op_with_updates(OperationType::SpiralFinish, |op| {
        let OperationConfig::SpiralFinish(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.stepover = 2.0;
    });
    let radial_finish = op_with_updates(OperationType::RadialFinish, |op| {
        let OperationConfig::RadialFinish(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.angular_step = 30.0;
        cfg.point_spacing = 2.0;
    });
    let horizontal_finish = op_with_updates(OperationType::HorizontalFinish, |op| {
        let OperationConfig::HorizontalFinish(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.stepover = 3.0;
    });
    let project_curve = op_with_updates(OperationType::ProjectCurve, |op| {
        let OperationConfig::ProjectCurve(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.depth = 0.75;
        cfg.point_spacing = 1.0;
    });

    vec![
        SpanCoverageCase {
            name: "Face",
            op: OperationConfig::new_default(OperationType::Face),
            tool_type: ToolType::EndMill,
            mesh: None,
            polygons: None,
            prev_tool_radius: None,
            expected_kinds: depth_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Depth pass", "Run"],
        },
        SpanCoverageCase {
            name: "Pocket",
            op: OperationConfig::new_default(OperationType::Pocket),
            tool_type: ToolType::EndMill,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
            prev_tool_radius: None,
            expected_kinds: depth_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Depth pass", "Run"],
        },
        SpanCoverageCase {
            name: "Profile",
            op: OperationConfig::new_default(OperationType::Profile),
            tool_type: ToolType::EndMill,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
            prev_tool_radius: None,
            expected_kinds: depth_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Depth pass", "Run"],
        },
        SpanCoverageCase {
            name: "Adaptive",
            op: OperationConfig::new_default(OperationType::Adaptive),
            tool_type: ToolType::EndMill,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
            prev_tool_radius: None,
            expected_kinds: depth_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Depth pass", "Run"],
        },
        SpanCoverageCase {
            name: "VCarve",
            op: OperationConfig::new_default(OperationType::VCarve),
            tool_type: ToolType::VBit,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
            prev_tool_radius: None,
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["V-carve run"],
        },
        SpanCoverageCase {
            name: "Rest",
            op: OperationConfig::new_default(OperationType::Rest),
            tool_type: ToolType::EndMill,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
            prev_tool_radius: Some(6.0),
            expected_kinds: depth_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Depth pass", "Run"],
        },
        SpanCoverageCase {
            name: "Inlay",
            op: OperationConfig::new_default(OperationType::Inlay),
            tool_type: ToolType::VBit,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
            prev_tool_radius: None,
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Inlay run"],
        },
        SpanCoverageCase {
            name: "Zigzag",
            op: OperationConfig::new_default(OperationType::Zigzag),
            tool_type: ToolType::EndMill,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
            prev_tool_radius: None,
            expected_kinds: depth_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Depth pass", "Run"],
        },
        SpanCoverageCase {
            name: "Trace",
            op: OperationConfig::new_default(OperationType::Trace),
            tool_type: ToolType::EndMill,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
            prev_tool_radius: None,
            expected_kinds: depth_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Depth pass", "Run"],
        },
        SpanCoverageCase {
            name: "Drill",
            // Picks are the hole source on this model-less path
            // (G-DRILLCENTROID); the `Drill` polygon fixture is kept so
            // the polygon slot still exercises a 2D op with geometry.
            op: drill_op_picking(&[[25.0, 25.0], [55.0, 55.0]]),
            tool_type: ToolType::EndMill,
            mesh: None,
            polygons: Some(PolygonFixture::Drill),
            prev_tool_radius: None,
            expected_kinds: drill_expected.clone(),
            forbidden_kinds: vec![SpanKind::DepthPass],
            expected_label_fragments: vec!["Hole", "plunge"],
        },
        SpanCoverageCase {
            name: "Chamfer",
            op: OperationConfig::new_default(OperationType::Chamfer),
            tool_type: ToolType::VBit,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
            prev_tool_radius: None,
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Chamfer run"],
        },
        SpanCoverageCase {
            name: "DropCutter",
            op: drop_cutter,
            tool_type: ToolType::BallNose,
            mesh: Some(MeshFixture::Flat),
            polygons: None,
            prev_tool_radius: None,
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Raster row"],
        },
        SpanCoverageCase {
            name: "Adaptive3d",
            op: adaptive3d,
            tool_type: ToolType::EndMill,
            mesh: Some(MeshFixture::Hemisphere),
            polygons: None,
            prev_tool_radius: None,
            // D4 — Adaptive3D now emits SpanKind::Entry per pass.
            expected_kinds: {
                let mut k = depth_expected.clone();
                k.push(SpanKind::Entry);
                k
            },
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Z level", "Adaptive region", "plunge entry"],
        },
        SpanCoverageCase {
            name: "Waterline",
            op: waterline,
            tool_type: ToolType::EndMill,
            mesh: Some(MeshFixture::Hemisphere),
            polygons: None,
            prev_tool_radius: None,
            expected_kinds: depth_expected,
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Depth pass", "Run"],
        },
        SpanCoverageCase {
            name: "Pencil",
            op: OperationConfig::new_default(OperationType::Pencil),
            tool_type: ToolType::BallNose,
            mesh: Some(MeshFixture::Groove),
            polygons: None,
            prev_tool_radius: None,
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Chain"],
        },
        SpanCoverageCase {
            name: "Scallop",
            op: scallop,
            tool_type: ToolType::BallNose,
            mesh: Some(MeshFixture::Hemisphere),
            polygons: None,
            prev_tool_radius: None,
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Ring"],
        },
        SpanCoverageCase {
            name: "SteepShallow",
            op: OperationConfig::new_default(OperationType::SteepShallow),
            tool_type: ToolType::BallNose,
            mesh: Some(MeshFixture::Hemisphere),
            polygons: None,
            prev_tool_radius: None,
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Steep/shallow run"],
        },
        SpanCoverageCase {
            name: "RampFinish",
            op: ramp_finish,
            tool_type: ToolType::BallNose,
            mesh: Some(MeshFixture::Hemisphere),
            polygons: None,
            prev_tool_radius: None,
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Terrace"],
        },
        SpanCoverageCase {
            name: "SpiralFinish",
            op: spiral_finish,
            tool_type: ToolType::BallNose,
            mesh: Some(MeshFixture::Hemisphere),
            polygons: None,
            prev_tool_radius: None,
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Ring"],
        },
        SpanCoverageCase {
            name: "RadialFinish",
            op: radial_finish,
            tool_type: ToolType::BallNose,
            mesh: Some(MeshFixture::Flat),
            polygons: None,
            prev_tool_radius: None,
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Radial ray"],
        },
        SpanCoverageCase {
            name: "HorizontalFinish",
            op: horizontal_finish,
            tool_type: ToolType::BallNose,
            mesh: Some(MeshFixture::Flat),
            polygons: None,
            prev_tool_radius: None,
            expected_kinds: region_expected,
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Horizontal slice"],
        },
        SpanCoverageCase {
            name: "ProjectCurve",
            op: project_curve,
            tool_type: ToolType::BallNose,
            mesh: Some(MeshFixture::Hemisphere),
            polygons: Some(PolygonFixture::ProjectCurve),
            prev_tool_radius: None,
            expected_kinds: vec![SpanKind::Region],
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Projected curve"],
        },
        SpanCoverageCase {
            name: "AlignmentPinDrill",
            op: alignment_pin,
            tool_type: ToolType::EndMill,
            mesh: None,
            polygons: None,
            prev_tool_radius: None,
            expected_kinds: drill_expected,
            forbidden_kinds: vec![SpanKind::DepthPass],
            expected_label_fragments: vec!["Hole", "plunge"],
        },
    ]
}

#[test]
fn missing_polygons_error_for_2d_operation() {
    let op = OperationConfig::new_default(OperationType::Profile);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        ..ExecutionContext::new(
            &findings,
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            &cancel,
        )
    };
    let result = execute_operation_annotated(&ctx, &op).map(|generated| generated.toolpath);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, OperationError::MissingGeometry(_)),
        "Expected MissingGeometry, got: {err:?}"
    );
}

#[test]
fn missing_mesh_error_for_3d_operation() {
    let op = OperationConfig::new_default(OperationType::DropCutter);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        ..ExecutionContext::new(
            &findings,
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            &cancel,
        )
    };
    let result = execute_operation_annotated(&ctx, &op).map(|generated| generated.toolpath);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, OperationError::MissingGeometry(_)),
        "Expected MissingGeometry, got: {err:?}"
    );
}

#[test]
fn invalid_tool_for_vcarve() {
    let op = OperationConfig::new_default(OperationType::VCarve);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill); // not a V-Bit
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);
    let polys = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        polygons: Some(&polys),
        ..ExecutionContext::new(
            &findings,
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            &cancel,
        )
    };
    let result = execute_operation_annotated(&ctx, &op).map(|generated| generated.toolpath);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, OperationError::InvalidTool(_)),
        "Expected InvalidTool, got: {err:?}"
    );
}

#[test]
fn invalid_tool_for_scallop() {
    let op = OperationConfig::new_default(OperationType::Scallop);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill); // not ball nose
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);

    // Scallop requires a mesh, but the tool check happens before mesh access
    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        ..ExecutionContext::new(
            &findings,
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            &cancel,
        )
    };
    let result = execute_operation_annotated(&ctx, &op).map(|generated| generated.toolpath);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, OperationError::InvalidTool(_)),
        "Expected InvalidTool, got: {err:?}"
    );
}

#[test]
fn face_produces_output() {
    let op = OperationConfig::new_default(OperationType::Face);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        ..ExecutionContext::new(
            &findings,
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            &cancel,
        )
    };
    let result = execute_operation_annotated(&ctx, &op).map(|generated| generated.toolpath);

    assert!(result.is_ok(), "Face should succeed, got: {result:?}");
    let tp = result.unwrap();
    assert!(
        !tp.moves.is_empty(),
        "Face toolpath should contain at least one move"
    );
}

#[test]
fn drill_produces_output() {
    // This path resolves no model, so the hole is an explicit pick
    // (G-DRILLCENTROID: a polygon centroid is no longer a hole source).
    let op = drill_op_picking(&[[25.0, 25.0]]);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        ..ExecutionContext::new(
            &findings,
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            &cancel,
        )
    };
    let result = execute_operation_annotated(&ctx, &op).map(|generated| generated.toolpath);

    assert!(result.is_ok(), "Drill should succeed, got: {result:?}");
    let tp = result.unwrap();
    assert!(
        !tp.moves.is_empty(),
        "Drill toolpath should contain at least one move"
    );
}

#[test]
fn all_operation_families_emit_expected_structural_span_kinds() {
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);
    let standard_polygons = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];
    let drill_polygons = vec![
        Polygon2::rectangle(24.0, 24.0, 26.0, 26.0),
        Polygon2::rectangle(54.0, 54.0, 56.0, 56.0),
    ];
    let project_curve_polygons = vec![
        Polygon2::rectangle(12.0, 12.0, 40.0, 40.0),
        Polygon2::rectangle(18.0, 18.0, 32.0, 32.0),
    ];
    let flat_mesh = make_test_flat(80.0);
    let flat_index = SpatialIndex::build_auto(&flat_mesh);
    let hemisphere_mesh = make_test_hemisphere(25.0, 16);
    let hemisphere_index = SpatialIndex::build_auto(&hemisphere_mesh);
    let groove_mesh = make_v_groove_mesh(60.0, 8.0, 14.0);
    let groove_index = SpatialIndex::build_auto(&groove_mesh);

    for case in span_coverage_cases() {
        let (tool_def, tool_cfg) = make_tool(case.tool_type);
        let cutting_levels = case.op.cutting_levels(heights.top_z);
        let mesh_and_index = match case.mesh {
            Some(MeshFixture::Flat) => Some((&flat_mesh, &flat_index)),
            Some(MeshFixture::Hemisphere) => Some((&hemisphere_mesh, &hemisphere_index)),
            Some(MeshFixture::Groove) => Some((&groove_mesh, &groove_index)),
            None => None,
        };
        let polygons = match case.polygons {
            Some(PolygonFixture::Standard) => Some(standard_polygons.as_slice()),
            Some(PolygonFixture::Drill) => Some(drill_polygons.as_slice()),
            Some(PolygonFixture::ProjectCurve) => Some(project_curve_polygons.as_slice()),
            None => None,
        };

        let findings = std::cell::RefCell::new(GenerationFindings::default());
        let ctx = ExecutionContext {
            mesh: mesh_and_index.map(|(mesh, _)| mesh),
            index: mesh_and_index.map(|(_, index)| index),
            polygons,
            prev_tool_radius: case.prev_tool_radius,
            ..ExecutionContext::new(
                &findings,
                &tool_def,
                &tool_cfg,
                &heights,
                &cutting_levels,
                &bbox,
                &cancel,
            )
        };
        let result = execute_operation_annotated(&ctx, &case.op)
            .unwrap_or_else(|err| panic!("{} should generate: {err}", case.name));

        assert!(result.spans_valid, "{} spans should be valid", case.name);
        assert!(
            !result.toolpath.moves.is_empty(),
            "{} should generate moves for span coverage",
            case.name
        );
        result
            .check_invariants()
            .unwrap_or_else(|err| panic!("{} span invariants failed: {err}", case.name));
        assert!(
            result
                .spans
                .iter()
                .any(|span| span.kind == SpanKind::Operation),
            "{} should retain an Operation span",
            case.name
        );
        assert!(
            result
                .spans
                .iter()
                .any(|span| span.kind != SpanKind::Operation),
            "{} should expose structure below the Operation span",
            case.name
        );
        for expected in &case.expected_kinds {
            assert!(
                result.spans.iter().any(|span| span.kind == *expected),
                "{} should emit {expected:?} spans; got {:?}",
                case.name,
                result
                    .spans
                    .iter()
                    .map(|span| span.kind)
                    .collect::<Vec<_>>()
            );
        }
        for forbidden in &case.forbidden_kinds {
            assert!(
                result.spans.iter().all(|span| span.kind != *forbidden),
                "{} should not emit {forbidden:?} spans",
                case.name
            );
        }
        for fragment in &case.expected_label_fragments {
            assert!(
                result
                    .spans
                    .iter()
                    .any(|span| span.label.contains(fragment)),
                "{} should emit a span label containing {fragment:?}; labels: {:?}",
                case.name,
                result
                    .spans
                    .iter()
                    .map(|span| span.label.as_ref())
                    .collect::<Vec<_>>()
            );
        }

        // F2.1 — every move carrying a transit intent must sit
        // inside a span of the mapped kind (Linking → LinkBridge,
        // Entry*/LeadIn → Entry, LeadOut → LeadOut), for every
        // family and funnel. This is the per-op guarantee that
        // gate-side ancestry filters see ALL transients, not just
        // generator-tagged ones.
        for (move_idx, mv) in result.toolpath.moves.iter().enumerate() {
            use crate::toolpath::MoveIntent as I;
            let expected_kind = match mv.intent {
                I::Linking => Some(SpanKind::LinkBridge),
                I::EntryPlunge | I::EntryHelix | I::EntryRamp | I::LeadIn => Some(SpanKind::Entry),
                I::LeadOut => Some(SpanKind::LeadOut),
                _ => None,
            };
            if let Some(kind) = expected_kind {
                assert!(
                    result
                        .spans
                        .iter()
                        .any(|span| span.kind == kind && span.contains(move_idx)),
                    "{}: move {move_idx} has intent {:?} but no covering {kind:?} span",
                    case.name,
                    mv.intent,
                );
            }
        }
    }
}

/// Phase 5 (T11) cancellation net — written BEFORE the adapter
/// cutover per plan §Phase-5 task 6. Originally exactly four of the 23
/// arms cooperatively polled `cancel`: Adaptive, DropCutter, Adaptive3d,
/// Waterline. The 2026-07 mesh-finish incident (a fine-stepover
/// generation hung the GUI for two hours because none of the 3D
/// finishing families polled cancel) extended coverage to the seven
/// dense-heightmap/dense-loop finish families: Pencil, Scallop,
/// SteepShallow, RampFinish, SpiralFinish, RadialFinish,
/// HorizontalFinish — 11 of 23 arms total. The flat-2D S.5 fix
/// (planning/finishing_stack_review_2026-07.md — "Zero of 10 flat 2D
/// ops can be cancelled") extended coverage again to Pocket, Profile,
/// Zigzag, Trace, Face, ProjectCurve, VCarve, Inlay — 19 of 23 arms
/// total. A `GenerateFn` adapter that forgets to rebuild the
/// `|| cancel.load(Ordering::SeqCst)` closure silently makes the op
/// uncancellable — no compile error, invisible to fast unit tests.
/// This pins the contract: with `cancel` pre-set, each of those
/// families must return `Err(OperationError::Cancelled)` rather than
/// running to completion.
#[test]
fn cancellable_families_honour_a_preset_cancel_flag() {
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let standard_polygons = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];
    let hemisphere_mesh = make_test_hemisphere(25.0, 16);
    let hemisphere_index = SpatialIndex::build_auto(&hemisphere_mesh);

    let adaptive3d = op_with_updates(OperationType::Adaptive3d, |op| {
        let OperationConfig::Adaptive3d(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.depth_per_pass = 4.0;
    });
    let drop_cutter = op_with_updates(OperationType::DropCutter, |op| {
        let OperationConfig::DropCutter(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.stepover = 2.0;
        cfg.min_z = -5.0;
    });
    let waterline = op_with_updates(OperationType::Waterline, |op| {
        let OperationConfig::Waterline(cfg) = op else {
            unreachable!("default op kind mismatch");
        };
        cfg.z_step = 2.0;
        cfg.sampling = 1.0;
    });

    // (name, op, tool, needs_mesh, needs_polygons)
    let cases: Vec<(&str, OperationConfig, ToolType, bool, bool)> = vec![
        (
            "Adaptive",
            OperationConfig::new_default(OperationType::Adaptive),
            ToolType::EndMill,
            false,
            true,
        ),
        ("DropCutter", drop_cutter, ToolType::BallNose, true, false),
        ("Adaptive3d", adaptive3d, ToolType::EndMill, true, false),
        ("Waterline", waterline, ToolType::BallNose, true, false),
        // Mesh-finish families (2026-07 hang fix): cancel is checked as
        // the very first statement of each `*_with_cancel` entry point,
        // so a pre-set flag must short-circuit before any real work
        // regardless of default params/mesh shape.
        (
            "Pencil",
            OperationConfig::new_default(OperationType::Pencil),
            ToolType::BallNose,
            true,
            false,
        ),
        (
            "Scallop",
            OperationConfig::new_default(OperationType::Scallop),
            ToolType::BallNose,
            true,
            false,
        ),
        (
            "SteepShallow",
            OperationConfig::new_default(OperationType::SteepShallow),
            ToolType::BallNose,
            true,
            false,
        ),
        (
            "RampFinish",
            OperationConfig::new_default(OperationType::RampFinish),
            ToolType::BallNose,
            true,
            false,
        ),
        (
            "SpiralFinish",
            OperationConfig::new_default(OperationType::SpiralFinish),
            ToolType::BallNose,
            true,
            false,
        ),
        (
            "RadialFinish",
            OperationConfig::new_default(OperationType::RadialFinish),
            ToolType::BallNose,
            true,
            false,
        ),
        (
            "HorizontalFinish",
            OperationConfig::new_default(OperationType::HorizontalFinish),
            ToolType::BallNose,
            true,
            false,
        ),
        // Flat-2D families (S.5 fix, planning/finishing_stack_review_2026-07.md):
        // cancel is checked as the very first statement of every
        // `*_with_cancel` entry point (and of the shared
        // `depth::toolpath_at_levels_with_cancel` choke point pocket/
        // profile/zigzag/trace/face route through), so a pre-set flag
        // short-circuits regardless of default params/polygon shape.
        (
            "Pocket",
            OperationConfig::new_default(OperationType::Pocket),
            ToolType::EndMill,
            false,
            true,
        ),
        (
            "Profile",
            OperationConfig::new_default(OperationType::Profile),
            ToolType::EndMill,
            false,
            true,
        ),
        (
            "Zigzag",
            OperationConfig::new_default(OperationType::Zigzag),
            ToolType::EndMill,
            false,
            true,
        ),
        (
            "Trace",
            OperationConfig::new_default(OperationType::Trace),
            ToolType::EndMill,
            false,
            true,
        ),
        (
            "Face",
            OperationConfig::new_default(OperationType::Face),
            ToolType::EndMill,
            false,
            false,
        ),
        (
            "ProjectCurve",
            OperationConfig::new_default(OperationType::ProjectCurve),
            ToolType::EndMill,
            true,
            true,
        ),
        (
            "VCarve",
            OperationConfig::new_default(OperationType::VCarve),
            ToolType::VBit,
            false,
            true,
        ),
        (
            "Inlay",
            OperationConfig::new_default(OperationType::Inlay),
            ToolType::VBit,
            false,
            true,
        ),
        // Checkpoint C, Q3 (F-4). Added in the same commit that made them
        // cancellable — the doc on `ExecutionContext` says the coverage
        // claim goes stale otherwise, and this is what keeps it honest.
        (
            "Rest",
            OperationConfig::new_default(OperationType::Rest),
            ToolType::EndMill,
            false,
            true,
        ),
        (
            "Drill",
            OperationConfig::new_default(OperationType::Drill),
            ToolType::EndMill,
            false,
            true,
        ),
        // O-CANC (2026-08-14) — the last two registry families, plus
        // UnifiedFinish, which was cancellable all along and simply
        // never enumerated here (see `ExecutionContext`'s doc for why
        // that made the coverage count wrong in BOTH terms).
        // AlignmentPinDrill and Chamfer both poll as their first
        // statement, ahead of their own preconditions — the pin op's
        // default config has no holes and Chamfer's tool here is a
        // V-bit, so a check placed after those guards would report
        // MissingGeometry / InvalidTool instead of Cancelled and this
        // case would fail.
        (
            "UnifiedFinish",
            OperationConfig::new_default(OperationType::UnifiedFinish),
            ToolType::BallNose,
            true,
            false,
        ),
        (
            "AlignmentPinDrill",
            OperationConfig::new_default(OperationType::AlignmentPinDrill),
            ToolType::EndMill,
            false,
            false,
        ),
        (
            "Chamfer",
            OperationConfig::new_default(OperationType::Chamfer),
            ToolType::VBit,
            false,
            true,
        ),
    ];

    // The coverage claim on `ExecutionContext` is no longer a
    // hand-written count. Assert the case list IS the registry, so a
    // new family either polls the flag or turns this red.
    let covered: std::collections::BTreeSet<String> =
        cases.iter().map(|(name, ..)| (*name).to_owned()).collect();
    let registered: std::collections::BTreeSet<String> = OperationType::ALL
        .iter()
        .map(|op| format!("{op:?}"))
        .collect();
    assert_eq!(
        covered,
        registered,
        "every registered operation family must appear in this list. Missing \
         from the list: {:?}. In the list but not registered: {:?}",
        registered.difference(&covered).collect::<Vec<_>>(),
        covered.difference(&registered).collect::<Vec<_>>(),
    );

    for (name, op, tool_type, needs_mesh, needs_polygons) in cases {
        let (tool_def, tool_cfg) = make_tool(tool_type);
        let cutting_levels = op.cutting_levels(heights.top_z);
        let cancel = AtomicBool::new(true); // pre-set: cancel before any work

        let findings = std::cell::RefCell::new(GenerationFindings::default());
        let ctx = ExecutionContext {
            mesh: needs_mesh.then_some(&hemisphere_mesh),
            index: needs_mesh.then_some(&hemisphere_index),
            polygons: needs_polygons.then_some(standard_polygons.as_slice()),
            ..ExecutionContext::new(
                &findings,
                &tool_def,
                &tool_cfg,
                &heights,
                &cutting_levels,
                &bbox,
                &cancel,
            )
        };
        let result = execute_operation_annotated(&ctx, &op);

        match result {
            Err(OperationError::Cancelled) => {}
            Ok(generated) => panic!(
                "{name}: pre-set cancel was ignored — generation ran to \
                 completion ({} moves). The cancel closure is no longer \
                 wired through this family's generator.",
                generated.toolpath.moves.len()
            ),
            Err(other) => panic!("{name}: expected OperationError::Cancelled, got: {other:?}"),
        }
    }
}

#[test]
fn trace_annotated_output_has_depth_and_region_spans() {
    let op = OperationConfig::new_default(OperationType::Trace);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);
    let polys = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];
    let levels = op.cutting_levels(heights.top_z);

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        polygons: Some(&polys),
        ..ExecutionContext::new(
            &findings, &tool_def, &tool_cfg, &heights, &levels, &bbox, &cancel,
        )
    };
    let result = execute_operation_annotated(&ctx, &op).expect("trace should succeed");

    assert!(result.spans_valid);
    assert!(
        result
            .spans
            .iter()
            .any(|span| span.kind == crate::trace::toolpath_spans::SpanKind::DepthPass),
        "trace should emit DepthPass spans"
    );
    assert!(
        result
            .spans
            .iter()
            .any(|span| span.kind == crate::trace::toolpath_spans::SpanKind::Region),
        "trace should emit per-chain Region spans"
    );
    result
        .check_invariants()
        .expect("trace spans should satisfy invariants");
}

#[test]
fn drill_annotated_output_has_hole_and_plunge_spans_without_depth_barriers() {
    let op = drill_op_picking(&[[25.0, 25.0], [55.0, 55.0]]);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        ..ExecutionContext::new(
            &findings,
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            &cancel,
        )
    };
    let result = execute_operation_annotated(&ctx, &op).expect("drill should succeed");

    assert!(result.spans_valid);
    // C4: role queries, not label parsing. A test that asserted on the
    // label shape would have kept passing if the roles were wrong, and
    // would break on a purely cosmetic label edit — exactly backwards.
    use crate::trace::toolpath_spans::RegionSpanRole;
    let hole_count = result
        .spans
        .iter()
        .filter(|span| span.has_region_role(RegionSpanRole::DrillHole))
        .count();
    let plunge_count = result
        .spans
        .iter()
        .filter(|span| span.has_region_role(RegionSpanRole::DrillPeck))
        .count();
    assert_eq!(hole_count, 2, "expected one hole span per input hole");
    assert!(plunge_count >= 2, "expected drill plunge child spans");
    assert!(
        result
            .spans
            .iter()
            .all(|span| span.kind != crate::trace::toolpath_spans::SpanKind::DepthPass),
        "drill must not emit DepthPass spans because they act as TSP barriers"
    );
    result
        .check_invariants()
        .expect("drill spans should satisfy invariants");
}

#[test]
fn trace_semantic_trace_has_depth_and_chain_children() {
    let op = OperationConfig::new_default(OperationType::Trace);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);
    let polys = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];
    let levels = op.cutting_levels(heights.top_z);
    let recorder = crate::trace::semantic_trace::ToolpathSemanticRecorder::new("Trace", "Trace");
    let ctx = recorder.root_context();

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        polygons: Some(&polys),
        semantic_ctx: Some(&ctx),
        ..ExecutionContext::new(
            &findings, &tool_def, &tool_cfg, &heights, &levels, &bbox, &cancel,
        )
    };
    let _ = execute_operation_annotated(&ctx, &op).expect("trace should succeed");
    let semantic = recorder.finish();

    assert!(
        semantic
            .items
            .iter()
            .any(|item| item.kind
                == crate::trace::semantic_trace::ToolpathSemanticKind::DepthLevel),
        "trace should emit DepthLevel semantic items"
    );
    assert!(
        semantic
            .items
            .iter()
            .any(|item| item.kind == crate::trace::semantic_trace::ToolpathSemanticKind::Chain),
        "trace should emit Chain semantic items"
    );
}

#[test]
fn drill_semantic_trace_has_hole_and_cycle_children() {
    let op = drill_op_picking(&[[25.0, 25.0]]);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);
    let recorder = crate::trace::semantic_trace::ToolpathSemanticRecorder::new("Drill", "Drill");
    let ctx = recorder.root_context();

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        semantic_ctx: Some(&ctx),
        ..ExecutionContext::new(
            &findings,
            &tool_def,
            &tool_cfg,
            &heights,
            &[],
            &bbox,
            &cancel,
        )
    };
    let _ = execute_operation_annotated(&ctx, &op).expect("drill should succeed");
    let semantic = recorder.finish();

    assert!(
        semantic
            .items
            .iter()
            .any(|item| item.kind == crate::trace::semantic_trace::ToolpathSemanticKind::Hole),
        "drill should emit Hole semantic items"
    );
    assert!(
        semantic
            .items
            .iter()
            .any(|item| item.kind == crate::trace::semantic_trace::ToolpathSemanticKind::Cycle),
        "drill should emit Cycle semantic items"
    );
}

#[test]
fn feed_optimization_uses_configured_nominal_feed_not_entry_plunge() {
    let configured_feed = 1200.0;
    let first_raw_feed = 300.0;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 10.0, 30.0));
    tp.feed_to(P3::new(10.0, 10.0, 0.0), first_raw_feed);
    tp.feed_to(P3::new(50.0, 10.0, 0.0), first_raw_feed);

    let cfg = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        feed_optimization: true,
        feed_max_rate: 5000.0,
        ..DressupConfig::default()
    };
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let mut stock = crate::dexel_stock::TriDexelStock::from_bounds(&test_stock_bbox(), 2.0);
    let cutter = build_cutter(&tool_cfg);
    let recorder =
        crate::trace::semantic_trace::ToolpathSemanticRecorder::new("FeedOpt nominal", "Pocket");
    let semantic_root = recorder.root_context();

    let _result = apply_dressups(
        AnnotatedToolpath::new(tp),
        crate::compute::execute::DressupContext {
            ramp_feed_rate_mm_min: None,
            cfg: &cfg,
            nominal_feed_rate: configured_feed,
            plunge_rate_mm_min: None,
            tool_diameter: tool_def.diameter(),
            safe_z: 30.0,
            stock_top: 25.0,
            prior_stock: None,
            feed_opt_stock: Some(&mut stock),
            cutter: Some(&cutter),
            entry_surface: None,
            transform_capabilities: OperationType::Pocket.transform_capabilities(),
            debug_ctx: None,
            semantic_ctx: Some(&semantic_root),
        },
        &mut ReconcileSet::empty(),
    );
    let semantic = recorder.finish();
    let nominal = semantic
        .items
        .iter()
        .find(|item| item.label == "Feed optimization")
        .and_then(|item| item.params.get(SemanticKey::NominalFeedRate))
        .and_then(serde_json::Value::as_f64)
        .expect("feed optimization trace should carry nominal_feed_rate");

    assert_eq!(nominal, configured_feed);
    assert_ne!(nominal, first_raw_feed * 0.5);
}

#[test]
fn apply_dressups_preserves_moves() {
    // Build a simple toolpath with a few moves
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 30.0));
    tp.rapid_to(P3::new(10.0, 10.0, 30.0));
    tp.feed_to(P3::new(10.0, 10.0, 0.0), 1000.0);
    tp.feed_to(P3::new(50.0, 10.0, 0.0), 1000.0);
    tp.feed_to(P3::new(50.0, 50.0, 0.0), 1000.0);
    tp.rapid_to(P3::new(50.0, 50.0, 30.0));

    let cfg = DressupConfig::default();
    let result = apply_dressups(
        AnnotatedToolpath::new(tp),
        crate::compute::execute::DressupContext {
            ramp_feed_rate_mm_min: None,
            cfg: &cfg,
            nominal_feed_rate: 1000.0,
            plunge_rate_mm_min: None,
            tool_diameter: 6.35,
            safe_z: 30.0,
            stock_top: 0.0,
            prior_stock: None,
            feed_opt_stock: None,
            cutter: None,
            entry_surface: None,
            transform_capabilities: OperationType::DropCutter.transform_capabilities(),
            debug_ctx: None,
            semantic_ctx: None,
        },
        &mut ReconcileSet::empty(),
    );

    assert!(
        !result.toolpath.moves.is_empty(),
        "apply_dressups with default config should preserve moves"
    );
}

// ── P2.5: op-agnostic rest analysis ───────────────────────────────

/// A non-pencil op (Scallop) with `rest_analysis.enabled` gets
/// `rest_grid` / `rest_regions` attached generically, without emitting
/// a pencil centerline toolpath — the whole point of P2.5.
#[test]
fn rest_analysis_attaches_artifacts_for_non_pencil_op() {
    let mesh = make_test_hemisphere(25.0, 16);
    let index = SpatialIndex::build_auto(&mesh);
    let (tool_def, tool_cfg) = make_tool(ToolType::BallNose);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);
    let op = OperationConfig::new_default(OperationType::Scallop);
    let levels = op.cutting_levels(heights.top_z);
    let rest_analysis = crate::compute::config::RestAnalysisConfig {
        enabled: true,
        reference_tool_id: None,
        cell_mm: 1.0,
        min_valley_depth: 0.05,
        region_margin_mm: 0.5,
        ..Default::default()
    };

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        mesh: Some(&mesh),
        index: Some(&index),
        rest_analysis: Some(&rest_analysis),
        ..ExecutionContext::new(
            &findings, &tool_def, &tool_cfg, &heights, &levels, &bbox, &cancel,
        )
    };
    let result = execute_operation_annotated(&ctx, &op)
        .expect("scallop with rest_analysis enabled should succeed");

    assert!(
        result.rest_grid.is_some(),
        "enabled rest_analysis should attach a rest_grid to a non-pencil op"
    );
    assert!(
        result.rest_regions.is_some(),
        "enabled rest_analysis should attach rest_regions to a non-pencil op"
    );
}

/// `rest_analysis` disabled (or absent) is a byte-identical no-op:
/// neither `rest_grid` nor `rest_regions` gets attached.
#[test]
fn rest_analysis_disabled_leaves_artifacts_none() {
    let mesh = make_test_hemisphere(25.0, 16);
    let index = SpatialIndex::build_auto(&mesh);
    let (tool_def, tool_cfg) = make_tool(ToolType::BallNose);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);
    let op = OperationConfig::new_default(OperationType::Scallop);
    let levels = op.cutting_levels(heights.top_z);
    let rest_analysis = crate::compute::config::RestAnalysisConfig::default(); // disabled

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        mesh: Some(&mesh),
        index: Some(&index),
        rest_analysis: Some(&rest_analysis),
        ..ExecutionContext::new(
            &findings, &tool_def, &tool_cfg, &heights, &levels, &bbox, &cancel,
        )
    };
    let result = execute_operation_annotated(&ctx, &op).expect("scallop should succeed");

    assert!(result.rest_grid.is_none());
    assert!(result.rest_regions.is_none());

    // `None` for the whole param is the same no-op.
    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        mesh: Some(&mesh),
        index: Some(&index),
        ..ExecutionContext::new(
            &findings, &tool_def, &tool_cfg, &heights, &levels, &bbox, &cancel,
        )
    };
    let result_none = execute_operation_annotated(&ctx, &op).expect("scallop should succeed");
    assert!(result_none.rest_grid.is_none());
    assert!(result_none.rest_regions.is_none());
}

/// Precedence: a Pencil op whose OWN `RestDepth` detector arm already
/// attached `rest_grid` / `rest_regions` must NOT have those
/// overwritten by the generic P2.5 pass — one source of truth per
/// toolpath. Proven by using deliberately different `cell_mm` values
/// for the pencil detector vs. the generic `rest_analysis` config and
/// checking the surviving grid's `cell_mm` is pencil's, not generic's.
#[test]
fn pencil_rest_depth_precedence_skips_generic_pass() {
    use crate::compute::operation_configs::PencilConfig;

    let mesh = make_test_hemisphere(25.0, 16);
    let index = SpatialIndex::build_auto(&mesh);
    let (tool_def, tool_cfg) = make_tool(ToolType::BallNose);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);
    let pencil_cell_mm = 2.0;
    let generic_cell_mm = 9.75; // deliberately distinct sentinel
    let op = OperationConfig::Pencil(PencilConfig {
        detector: crate::finish::pencil::PencilDetector::RestDepth,
        rest_cell_mm: pencil_cell_mm,
        ..PencilConfig::default()
    });
    let levels = op.cutting_levels(heights.top_z);
    let rest_analysis = crate::compute::config::RestAnalysisConfig {
        enabled: true,
        reference_tool_id: None,
        cell_mm: generic_cell_mm,
        min_valley_depth: 0.05,
        region_margin_mm: 0.5,
        ..Default::default()
    };

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        mesh: Some(&mesh),
        index: Some(&index),
        rest_analysis: Some(&rest_analysis),
        ..ExecutionContext::new(
            &findings, &tool_def, &tool_cfg, &heights, &levels, &bbox, &cancel,
        )
    };
    let result = execute_operation_annotated(&ctx, &op).expect("pencil rest_depth should succeed");

    let grid = result
        .rest_grid
        .as_ref()
        .expect("pencil RestDepth detector should attach a rest_grid");
    assert!(
        (grid.grid.cell_mm - pencil_cell_mm).abs() < 1e-9,
        "generic rest_analysis pass must not overwrite pencil's own rest_grid \
         (got cell_mm={}, expected pencil's {pencil_cell_mm})",
        grid.grid.cell_mm
    );
}
