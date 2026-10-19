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

use super::rough_3d::adaptive3d_effective_stock_to_leave;
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

/// C8, red-first against the pre-wave code: the derived-stepover writer
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

    cell.borrow_mut().record(&stepover_finding(
        "UnifiedFinish crease/pencil claims",
        0.20,
    ));
    cell.borrow_mut()
        .record(&stepover_finding("generic rest analysis routing", 0.35));

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

/// G-DRILLCENTROID on the selection (P2.3c): a Drill drills the points
/// and circle centres of its selection, never the centroid of a loop. With
/// no point and no circle it refuses `Empty` in the prologue.
#[test]
fn drill_holes_come_from_points_and_circles_never_centroids() {
    use crate::geo::P2;
    use crate::selection::{Item, RefusalCause, Shape, SourceRef};
    let item = |layer: &str, shape: Shape| {
        Item::new(SourceRef::ToolpathModel, Some(layer.into()), shape).unwrap()
    };
    let op = OperationConfig::new_default(OperationType::Drill);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);
    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let run = |selection: SelectionInputs<'static>| {
        let ctx = ExecutionContext {
            selection,
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
        execute_operation_annotated(&ctx, &op)
    };

    // A point, a circle and a loop: two holes, at the point and the centre.
    let generated = run(SelectionInputs::from_items([
        item("pts", Shape::Point(P2::new(21.0, 22.0))),
        item("holes", Shape::circle(P2::new(37.0, 38.0), 4.0)),
        item(
            "outline",
            Shape::Loop(Polygon2::rectangle(10.0, 10.0, 60.0, 60.0)),
        ),
    ]))
    .unwrap();
    let plunges: Vec<[f64; 2]> = generated
        .toolpath
        .moves
        .windows(2)
        .filter(|w| w[1].target.z < w[0].target.z && w[1].target.x == w[0].target.x)
        .map(|w| [w[1].target.x, w[1].target.y])
        .collect();
    assert!(plunges.contains(&[21.0, 22.0]), "{plunges:?}");
    assert!(plunges.contains(&[37.0, 38.0]), "{plunges:?}");
    assert!(
        plunges
            .iter()
            .all(|p| *p == [21.0, 22.0] || *p == [37.0, 38.0]),
        "no hole at the loop centroid: {plunges:?}"
    );

    // Only a loop: nothing to drill. The prologue refuses `Empty`.
    let err = run(SelectionInputs::from_items([item(
        "outline",
        Shape::Loop(Polygon2::rectangle(10.0, 10.0, 60.0, 60.0)),
    )]))
    .unwrap_err();
    assert!(
        matches!(&err, OperationError::Selection(r) if matches!(r.cause, RefusalCause::Empty { .. })),
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
/// The default rule over one point per hole: the hole source of a
/// `Drill` op in the in-file tests, which reach the generator without a
/// model (G-DRILLCENTROID: polygon centroids are no hole source).
fn drill_points(holes: &[[f64; 2]]) -> SelectionInputs<'static> {
    SelectionInputs::from_items(holes.iter().map(|&[x, y]| {
        crate::selection::Item::new(
            crate::selection::SourceRef::ToolpathModel,
            None,
            crate::selection::Shape::Point(crate::geo::P2::new(x, y)),
        )
        .unwrap()
    }))
}

/// The default rule over `polygons` and one point at (30, 30): every op
/// family has an item it cuts (a Drill drills the point; a region or curve
/// op skips the point with no note).
fn rectangle_and_a_point(polygons: &[Polygon2]) -> SelectionInputs<'static> {
    let point = crate::selection::Item::new(
        crate::selection::SourceRef::ToolpathModel,
        None,
        crate::selection::Shape::Point(crate::geo::P2::new(30.0, 30.0)),
    )
    .unwrap();
    SelectionInputs::from_items(
        crate::selection::ItemSet::from_polygons(polygons.to_vec())
            .iter()
            .cloned()
            .chain(std::iter::once(point)),
    )
}

fn make_tool(tool_type: ToolType) -> (crate::tool::ToolDefinition, ToolConfig) {
    let cfg = ToolConfig::new_default(ToolId(0), tool_type);
    let def = build_cutter(&cfg).unwrap();
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

/// Two stock pins of the default pin-drill diameter, stock-relative.
fn span_coverage_pins() -> Vec<crate::compute::alignment_pins::AlignmentPin> {
    let OperationConfig::AlignmentPinDrill(cfg) =
        OperationConfig::new_default(OperationType::AlignmentPinDrill)
    else {
        unreachable!("default op kind mismatch");
    };
    [[25.0, 25.0], [55.0, 55.0]]
        .into_iter()
        .map(|[x, y]| crate::compute::alignment_pins::AlignmentPin::new(x, y, cfg.diameter))
        .collect()
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
    // W6.5 (C7): the holes come from the pins of the context
    // (`span_coverage_pins`), not from the config.
    let alignment_pin = OperationConfig::new_default(OperationType::AlignmentPinDrill);
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
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["V-carve run"],
        },
        SpanCoverageCase {
            name: "Inlay",
            op: OperationConfig::new_default(OperationType::Inlay),
            tool_type: ToolType::VBit,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Inlay run"],
        },
        SpanCoverageCase {
            name: "InlayPlug",
            op: OperationConfig::new_default(OperationType::InlayPlug),
            tool_type: ToolType::VBit,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
            expected_kinds: region_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Inlay plug run"],
        },
        SpanCoverageCase {
            name: "Zigzag",
            op: OperationConfig::new_default(OperationType::Zigzag),
            tool_type: ToolType::EndMill,
            mesh: None,
            polygons: Some(PolygonFixture::Standard),
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
            expected_kinds: depth_expected.clone(),
            forbidden_kinds: Vec::new(),
            expected_label_fragments: vec!["Depth pass", "Run"],
        },
        SpanCoverageCase {
            name: "Drill",
            // Two points are the hole source on this model-less path
            // (G-DRILLCENTROID: a loop centroid is no hole source).
            op: OperationConfig::new_default(OperationType::Drill),
            tool_type: ToolType::EndMill,
            mesh: None,
            polygons: Some(PolygonFixture::Drill),
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

    // P2.3a: an op that reads items, with no selection resolved, refuses
    // through its selection: nothing to cut (`Empty`), with the op name.
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(
            &err,
            OperationError::Selection(refusal)
                if matches!(refusal.cause, crate::selection::RefusalCause::Empty { .. })
        ),
        "Expected a selection Empty refusal, got: {err:?}"
    );
    assert!(err.to_string().starts_with("Profile: "), "{err}");
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
        selection: crate::compute::execute::SelectionInputs::from_polygons(&polys),
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

/// W2.2: the prologue order is cancel, then geometry, then tool. Before
/// W2.2 the four ball-tip adapters checked the tool before the mesh and
/// the V-bit adapters checked the polygons first; the prologue gives every
/// operation the geometry-first order. The wrong tool on a mesh is
/// `every_tool_refusal_is_the_registry_rows_through_the_dispatch`.
#[test]
fn scallop_without_a_mesh_refuses_the_mesh_before_the_tool() {
    let op = OperationConfig::new_default(OperationType::Scallop);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill); // not ball nose
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext::new(
        &findings,
        &tool_def,
        &tool_cfg,
        &heights,
        &[],
        &bbox,
        &cancel,
    );
    let err = execute_operation_annotated(&ctx, &op)
        .map(|generated| generated.toolpath)
        .unwrap_err();
    assert!(
        matches!(err, OperationError::MissingGeometry(_)),
        "Expected MissingGeometry, got: {err:?}"
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
    // This path resolves no model, so the hole is one point item
    // (G-DRILLCENTROID: a polygon centroid is no longer a hole source).
    let op = OperationConfig::new_default(OperationType::Drill);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        selection: drill_points(&[[25.0, 25.0]]),
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

    let pins = span_coverage_pins();
    for case in span_coverage_cases() {
        let (tool_def, tool_cfg) = make_tool(case.tool_type);
        let cutting_levels = case.op.cutting_levels(heights.top_z);
        let mesh_and_index = match case.mesh {
            Some(MeshFixture::Flat) => Some((&flat_mesh, &flat_index)),
            Some(MeshFixture::Hemisphere) => Some((&hemisphere_mesh, &hemisphere_index)),
            Some(MeshFixture::Groove) => Some((&groove_mesh, &groove_index)),
            None => None,
        };
        let selection = match case.polygons {
            Some(PolygonFixture::Standard) => SelectionInputs::from_polygons(&standard_polygons),
            Some(PolygonFixture::Drill) => drill_points(&[[25.0, 25.0], [55.0, 55.0]]),
            Some(PolygonFixture::ProjectCurve) => {
                SelectionInputs::from_polygons(&project_curve_polygons)
            }
            None => SelectionInputs::default(),
        };

        let findings = std::cell::RefCell::new(GenerationFindings::default());
        let ctx = ExecutionContext {
            mesh: mesh_and_index.map(|(mesh, _)| mesh),
            index: mesh_and_index.map(|(_, index)| index),
            selection,
            alignment_pins: &pins,
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

/// The kind of the tool a W2.2 test hands an operation: an end mill when the
/// row admits one, else the first kind the row requires. Read off the
/// registry row, so no test carries a hand list of tool rules.
fn admitted_kind(op_type: OperationType) -> crate::feeds::CutterKind {
    let tc = op_type.registry_entry().tool_constraints;
    if tc.allows(crate::feeds::CutterKind::Flat) {
        return crate::feeds::CutterKind::Flat;
    }
    *tc.required_kinds
        .first()
        .unwrap_or_else(|| panic!("{op_type:?} refuses the end mill and requires no kind"))
}

/// A default config, with three dials made coarse so a generator that
/// ignores the flag still ends in test time.
fn quick_config(op_type: OperationType) -> OperationConfig {
    op_with_updates(op_type, |op| match op {
        OperationConfig::Adaptive3d(cfg) => cfg.depth_per_pass = 4.0,
        OperationConfig::DropCutter(cfg) => {
            cfg.stepover = 2.0;
            cfg.min_z = -5.0;
        }
        OperationConfig::Waterline(cfg) => {
            cfg.z_step = 2.0;
            cfg.sampling = 1.0;
        }
        _ => {}
    })
}

/// O-CANC / W2.2: the dispatch prologue reads the cancel flag BEFORE any
/// refusal. For every registered operation, with the flag pre-set, NO
/// geometry and a tool the row refuses (when it refuses any), the dispatch
/// must report `Cancelled` — never "requires a 3D mesh", never a tool
/// refusal, never a drill-target refusal. Generated from
/// `OperationType::ALL`, so a 25th operation is covered with no edit here.
///
/// That a GENERATOR polls the flag during its own work is a different
/// claim: `every_generator_polls_a_preset_cancel_flag_past_the_prologue`.
#[test]
fn cancellable_families_honour_a_preset_cancel_flag() {
    let heights = test_heights();
    let bbox = test_stock_bbox();
    for &op_type in OperationType::ALL {
        let op = OperationConfig::new_default(op_type);
        let refused = crate::feeds::CutterKind::ALL
            .iter()
            .copied()
            .find(|&k| op_type.tool_refusal(k).is_some());
        let kind = refused.unwrap_or(crate::feeds::CutterKind::Flat);
        let (tool_def, tool_cfg) = make_tool(kind.tool_type());
        let cutting_levels = op.cutting_levels(heights.top_z);
        let cancel = AtomicBool::new(true);
        let findings = std::cell::RefCell::new(GenerationFindings::default());
        let ctx = ExecutionContext::new(
            &findings,
            &tool_def,
            &tool_cfg,
            &heights,
            &cutting_levels,
            &bbox,
            &cancel,
        );
        match execute_operation_annotated(&ctx, &op) {
            Err(OperationError::Cancelled) => {}
            Ok(generated) => panic!(
                "{op_type:?}: a pre-set cancel was ignored; generation ran to \
                 completion ({} moves).",
                generated.toolpath.moves.len()
            ),
            Err(other) => {
                panic!("{op_type:?}: expected Cancelled ahead of every refusal, got: {other:?}")
            }
        }
    }
}

/// Phase 5 (T11) cancellation net, past the prologue. The 2026-07
/// mesh-finish incident: a fine-stepover generation hung the GUI for two
/// hours because its adapter never built the cancel closure. The dispatch
/// prologue (W2.2) checks the flag once, before the adapter; it cannot see
/// a generator that ignores the flag DURING its work. So this test calls
/// each row's `generate` directly, past the prologue, with the flag
/// pre-set and the inputs the row requires. The generator must return
/// `Cancelled` rather than run to completion.
///
/// Generated from `OperationType::ALL`. The one exemption is a registry
/// fact, not a hand list: a `Kinematics::DrillZOnly` row (Drill, Pin
/// Drill) runs a short cycle whose only poll is the prologue's.
#[test]
fn every_generator_polls_a_preset_cancel_flag_past_the_prologue() {
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let polygons = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];
    let mesh = make_test_hemisphere(25.0, 16);
    let index = SpatialIndex::build_auto(&mesh);

    let mut polled = 0;
    let mut drill_cycles = 0;
    for &op_type in OperationType::ALL {
        let entry = op_type.registry_entry();
        if entry.policy.kinematics == crate::compute::catalog::Kinematics::DrillZOnly {
            drill_cycles += 1;
            continue;
        }
        let op = quick_config(op_type);
        let (tool_def, tool_cfg) = make_tool(admitted_kind(op_type).tool_type());
        let cutting_levels = op.cutting_levels(heights.top_z);
        let cancel = AtomicBool::new(true);
        let findings = std::cell::RefCell::new(GenerationFindings::default());
        let ctx = ExecutionContext {
            mesh: Some(&mesh),
            index: Some(&index),
            selection: crate::compute::execute::SelectionInputs::from_polygons(polygons.as_slice()),
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
        let prepared = prologue::check_inputs(&ctx, &op)
            .unwrap_or_else(|e| panic!("{op_type:?}: the prologue refused its own inputs: {e}"));
        match (entry.generate)(&ctx, &prepared, &op) {
            Err(OperationError::Cancelled) => polled += 1,
            Ok(generated) => panic!(
                "{op_type:?}: pre-set cancel was ignored past the prologue; the \
                 generator ran to completion ({} moves). Its kernel no longer \
                 receives `ctx.cancel_fn()`.",
                generated.toolpath.moves.len()
            ),
            Err(other) => panic!("{op_type:?}: expected Cancelled, got: {other:?}"),
        }
    }
    assert_eq!(polled + drill_cycles, OperationType::ALL.len());
    assert!(polled > drill_cycles, "the exemption swallowed the test");
}

/// W2.2: every operation whose registry row requires a tool kind refuses
/// each other kind through the real dispatch, with the registry's text
/// (`OperationType::tool_refusal`), and admits each kind the row allows.
/// Generated from `OperationType::ALL` x `CutterKind::ALL`: no hand list.
#[test]
fn every_tool_refusal_is_the_registry_rows_through_the_dispatch() {
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let polygons = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];
    let mesh = make_test_hemisphere(25.0, 16);
    let index = SpatialIndex::build_auto(&mesh);

    let mut restricted = 0;
    for &op_type in OperationType::ALL {
        let tc = op_type.registry_entry().tool_constraints;
        let op = OperationConfig::new_default(op_type);
        let mut refused_here = 0;
        for &kind in crate::feeds::CutterKind::ALL {
            let (tool_def, tool_cfg) = make_tool(kind.tool_type());
            let cutting_levels = op.cutting_levels(heights.top_z);
            let cancel = AtomicBool::new(false);
            let findings = std::cell::RefCell::new(GenerationFindings::default());
            let ctx = ExecutionContext {
                mesh: Some(&mesh),
                index: Some(&index),
                selection: rectangle_and_a_point(&polygons),
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
            if tc.allows(kind) {
                // Admitted: the prologue passes. The generation itself is
                // not run here; G-OPGOLDEN owns its output.
                assert!(
                    prologue::check_inputs(&ctx, &op).is_ok(),
                    "{op_type:?} refused {kind:?}, which its row admits"
                );
                continue;
            }
            refused_here += 1;
            let expected = op_type
                .tool_refusal(kind)
                .unwrap_or_else(|| panic!("{op_type:?} x {kind:?}: no registry text"));
            match execute_operation_annotated(&ctx, &op) {
                Err(OperationError::InvalidTool(text)) => {
                    assert_eq!(text, expected, "{op_type:?} x {kind:?}");
                }
                Ok(_) => panic!("{op_type:?} generated with {kind:?}, which its row refuses"),
                Err(other) => panic!("{op_type:?} x {kind:?}: expected InvalidTool, got {other:?}"),
            }
        }
        if !tc.required_kinds.is_empty() {
            assert!(
                refused_here > 0,
                "{op_type:?} requires kinds but refuses none"
            );
            restricted += 1;
        }
    }
    assert!(
        restricted > 0,
        "no row requires a tool kind; the test proves nothing"
    );
}

/// W2.2: the static diagnostic and the generator refuse the SAME tools,
/// with the SAME text — both read `OperationType::tool_refusal`. Before
/// W2.2 the diagnostic carried its own list (an end mill on Scallop,
/// Unified Finish or Pencil) and missed Spiral Finish, the bull nose, the
/// V-bit and the four V-bit operations.
#[test]
fn the_static_check_and_the_generator_refuse_the_same_tools() {
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let polygons = vec![Polygon2::rectangle(10.0, 10.0, 50.0, 50.0)];
    let mesh = make_test_hemisphere(25.0, 16);
    let index = SpatialIndex::build_auto(&mesh);

    for &op_type in OperationType::ALL {
        let op = OperationConfig::new_default(op_type);
        for &kind in crate::feeds::CutterKind::ALL {
            let (tool_def, tool_cfg) = make_tool(kind.tool_type());
            let diagnostic: Vec<String> =
                crate::diagnostics::adapters::from_static_checks::diagnostics_from_static_checks(
                    crate::ids::ToolpathId(0),
                    &op,
                    &tool_cfg,
                    None,
                )
                .into_iter()
                .filter(|d| d.id.as_str() == crate::diagnostics::ids::COMPAT_TOOL_KIND_REFUSED)
                .map(|d| d.message)
                .collect();

            let cutting_levels = op.cutting_levels(heights.top_z);
            let cancel = AtomicBool::new(false);
            let findings = std::cell::RefCell::new(GenerationFindings::default());
            let ctx = ExecutionContext {
                mesh: Some(&mesh),
                index: Some(&index),
                selection: rectangle_and_a_point(&polygons),
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
            // The prologue is the generator's tool gate; a refusal stops the
            // dispatch there, so this is what generation returns.
            let generator: Vec<String> = match prologue::check_inputs(&ctx, &op) {
                Err(OperationError::InvalidTool(text)) => vec![text],
                Ok(_) => vec![],
                Err(other) => panic!("{op_type:?} x {kind:?}: {other:?}"),
            };
            assert_eq!(diagnostic, generator, "{op_type:?} x {kind:?}");
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
        selection: crate::compute::execute::SelectionInputs::from_polygons(&polys),
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
    let op = OperationConfig::new_default(OperationType::Drill);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        selection: drill_points(&[[25.0, 25.0], [55.0, 55.0]]),
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
        selection: crate::compute::execute::SelectionInputs::from_polygons(&polys),
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
    let op = OperationConfig::new_default(OperationType::Drill);
    let (tool_def, tool_cfg) = make_tool(ToolType::EndMill);
    let heights = test_heights();
    let bbox = test_stock_bbox();
    let cancel = AtomicBool::new(false);
    let recorder = crate::trace::semantic_trace::ToolpathSemanticRecorder::new("Drill", "Drill");
    let ctx = recorder.root_context();

    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        selection: drill_points(&[[25.0, 25.0]]),
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
    let cutter = build_cutter(&tool_cfg).unwrap();
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
            link_kinematics: None,
            feed_opt_stock: Some(&mut stock),
            cutter: Some(&cutter),
            entry_surface: None,
            transform_capabilities: OperationType::Pocket.capabilities().transform,
            debug_ctx: None,
            semantic_ctx: Some(&semantic_root),
            entry_containment: None,
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
            link_kinematics: None,
            feed_opt_stock: None,
            cutter: None,
            entry_surface: None,
            transform_capabilities: OperationType::DropCutter.capabilities().transform,
            debug_ctx: None,
            semantic_ctx: None,
            entry_containment: None,
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
        result.produced.rest_grid.is_some(),
        "enabled rest_analysis should attach a rest_grid to a non-pencil op"
    );
    assert!(
        result.produced.rest_regions.is_some(),
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

    assert!(result.produced.rest_grid.is_none());
    assert!(result.produced.rest_regions.is_none());

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
    assert!(result_none.produced.rest_grid.is_none());
    assert!(result_none.produced.rest_regions.is_none());
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
        .produced
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
