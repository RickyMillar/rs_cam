//! PR-7 sentry (H2.5) — the op-agnostic rest analysis routes through the
//! CANONICAL reach policy, and `RestAnalysisConfig` carries a real fan.
//!
//! ## Contract
//!
//! * **Kind**: GATE (default CI, synthetic, ~30 s).
//! * **Guards**: the H2.5 slice. `compute::execute::attach_generic_rest_
//!   analysis` built its `RestFieldParams` with `..Default::default()`, so
//!   the coverage routing criterion `X_reach ≤ cap × stepover` was evaluated
//!   against a LITERAL 0.5 mm stepover and a 0-pass cap — a fan no operation
//!   on a tapered tool would ever emit, and the last parallel answer to a
//!   question `crate::reach` owns. `RestAnalysisConfig` carried neither dial
//!   (wave-A note in `planning/review_2026-07-29/ORCHESTRATION_LOG.md`).
//! * **Variables held fixed**: one mesh per gate, one tool, one cell size,
//!   one `min_valley_depth`. Gate 2's two arms differ ONLY in the stepover
//!   scalar.
//! * **Metric domains**: stepovers and reaches are LATERAL mm on the rest
//!   grid's XY plane; `traced_length_mm` is centreline arc length in mm.
//!   No areas appear, so nothing here can cross a domain.
//!
//! ## What this change does NOT do — stated, because the alternative is a
//! claim that is not true
//!
//! This pass attaches `rest_grid` + `region_polygons` and DISCARDS the
//! routing verdict. `region_polygons` is extracted from the cleaned rest
//! mask BEFORE any branch is routed (`rest_field.rs` step 2b), so it is
//! routing-INDEPENDENT: the artifacts an operator sees do not move. Gate 3
//! pins exactly that, so the day this pass starts consuming the verdict (or
//! emitting paths) the pin trips and the difference gate 2 measures becomes
//! visible output. The operator-visible change PR-7 does ship is the
//! derived-stepover diagnostic (gate 1) and the two new dials.
//!
//! ## Why this cannot pass vacuously
//!
//! Gate 2 asserts the DEFAULT arm produced real pencil centrelines and real
//! traced length before asserting the policy arm refuses them; an empty
//! detection fails it. Gate 1 demands the finding exist before reading it.

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
    BoundaryConfig, DressupConfig, HeightMode, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::ScallopConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::rest_field::{RestFieldParams, RestReference, detect_rest_valleys};
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_core::tool::{BallEndmill, TaperedBallEndmill};

fn wanaka_taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)
}

/// The literal `RestFieldParams::default()` stepover this pass used to route
/// against, pinned here so the differential cannot silently become a
/// comparison of the policy with itself.
const RETIRED_DETECTOR_STEPOVER_MM: f64 = 0.5;

/// Trapezoidal groove along Y at x = 0, in a 40 × 24 block whose top is
/// z = 0. `skew` tilts the two walls apart so the trough is asymmetric.
fn grooved_block(rim_half_width: f64, wall_deg: f64, depth: f64, skew: f64) -> TriangleMesh {
    let tan_l = wall_deg.to_radians().tan();
    let tan_r = (wall_deg * skew).to_radians().tan();
    let z_at = |x: f64| -> f64 {
        let floor_l = rim_half_width - depth / tan_l;
        let floor_r = rim_half_width - depth / tan_r;
        if x <= -rim_half_width || x >= rim_half_width {
            0.0
        } else if x < 0.0 {
            let ax = -x;
            if ax <= floor_l {
                -depth
            } else {
                -depth + (ax - floor_l) * tan_l
            }
        } else if x <= floor_r {
            -depth
        } else {
            -depth + (x - floor_r) * tan_r
        }
    };
    let mut xs: Vec<f64> = Vec::new();
    let mut x = -20.0;
    while x < -5.0 {
        xs.push(x);
        x += 1.0;
    }
    let mut x = -5.0;
    while x <= 5.0 + 1e-9 {
        xs.push(x);
        x += 0.05;
    }
    let mut x = 6.0;
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


// ── Session wiring ──────────────────────────────────────────────────────

fn tapered_ball_tool(id: usize) -> ToolConfig {
    ToolConfig {
        diameter: 1.0,
        taper_half_angle: 7.0,
        shaft_diameter: 6.0,
        ..ToolConfig::new_default(ToolId(id), ToolType::TaperedBallNose)
    }
}

fn ball_tool(id: usize, diameter: f64) -> ToolConfig {
    ToolConfig {
        diameter,
        ..ToolConfig::new_default(ToolId(id), ToolType::BallNose)
    }
}

fn mesh_model(mesh: TriangleMesh) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: "grooved_block".to_owned(),
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

fn toolpath(tool_id: usize, model_id: usize, rest_analysis: RestAnalysisConfig) -> ToolpathConfig {
    let op = OperationConfig::Scallop(ScallopConfig {
        scallop_height: 0.2,
        ..ScallopConfig::default()
    });
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: "Scallop".to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig {
            top_z: HeightMode::Manual(0.0),
            bottom_z: HeightMode::Manual(-8.0),
            ..HeightsConfig::default()
        },
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
        rest_analysis,
    }
}

/// A session whose single Scallop toolpath runs the op-agnostic rest
/// analysis. `finisher` is the toolpath's own tool (the FINE cutter the
/// policy is asked about); a Ø12 ball is always present as the rest
/// REFERENCE, so the resolution order lands on a real reference tool rather
/// than a self-referenced probe.
fn generate_with_rest_analysis(
    finisher: ToolConfig,
    rest_analysis: RestAnalysisConfig,
) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: 44.0,
        y: 28.0,
        z: 8.0,
        origin_x: -22.0,
        origin_y: -14.0,
        origin_z: -8.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let fine_idx = session.add_tool(finisher);
    let fine_id = session.tools()[fine_idx].id.0;
    let ref_idx = session.add_tool(ball_tool(1, 12.0));
    let ref_id = session.tools()[ref_idx].id;
    let model_id = session.add_model(mesh_model(grooved_block(8.0, 50.0, 5.0, 1.0)));
    let mut ra = rest_analysis;
    ra.reference_tool_id = Some(ref_id);
    session
        .add_toolpath(0, toolpath(fine_id, model_id, ra))
        .expect("add scallop toolpath");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("Scallop must generate on the grooved block");
    session
}

fn routing_rest_analysis() -> RestAnalysisConfig {
    RestAnalysisConfig {
        enabled: true,
        cell_mm: 0.25,
        min_valley_depth: 0.05,
        // A real fan: four offset passes per side. Before PR-7 there was no
        // way to say this at all.
        num_offset_passes: Some(4),
        ..RestAnalysisConfig::default()
    }
}

// ── Gate 1: the policy sized it, and the operator is told ───────────────

#[test]
fn the_generic_pass_routes_on_the_policy_stepover_and_reports_it() {
    let session = generate_with_rest_analysis(tapered_ball_tool(0), routing_rest_analysis());
    let stats = session
        .get_result(0)
        .expect("a generated result")
        .stats
        .clone();
    let finding = stats
        .derived_stepover
        .as_deref()
        .copied()
        .expect("the generic rest pass sized a stepover, so it must be recorded");
    println!(
        "PR-7 generic rest: site {:?}, derived {:.4} mm, envelope rule {:.4} mm, depth {:.4}",
        finding.site, finding.stepover_mm, finding.envelope_rule_mm, finding.reference_depth_mm
    );
    assert_eq!(finding.site, "generic rest analysis routing");

    let t = wanaka_taper();
    let expected = rs_cam_core::reach::suggested_offset_stepover_mm(&t, 0.05);
    assert!(
        (finding.stepover_mm - expected).abs() < 1e-12,
        "the site must CALL the policy, not restate it: {} vs {expected}",
        finding.stepover_mm
    );
    assert!(
        (finding.stepover_mm - RETIRED_DETECTOR_STEPOVER_MM).abs() > 1e-9,
        "on this taper the policy must not coincide with the retired literal, \
         or gate 2's differential is vacuous"
    );
    assert!((finding.envelope_rule_mm - 1.5).abs() < 1e-9);

    let diags = rs_cam_core::diagnostics::adapters::from_generation::diagnostics_from_generation(
        ToolpathId(0),
        &stats,
    );
    let d = diags
        .iter()
        .find(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::CONFIG_DERIVED_STEPOVER)
        .expect("the derived value differs from the envelope rule, so it must report");
    assert_eq!(d.severity, rs_cam_core::diagnostics::Severity::Info);
    assert!(d.message.contains("generic rest analysis routing"), "{}", d.message);
}

/// An operator who PINS the stepover owns that number: the policy is not
/// consulted and no notice is raised.
#[test]
fn an_explicitly_pinned_stepover_is_honoured_and_silent() {
    let cfg = RestAnalysisConfig {
        offset_stepover_mm: Some(RETIRED_DETECTOR_STEPOVER_MM),
        ..routing_rest_analysis()
    };
    let session = generate_with_rest_analysis(tapered_ball_tool(0), cfg);
    let stats = session.get_result(0).expect("a generated result").stats.clone();
    assert!(
        stats.derived_stepover.is_none(),
        "the policy did not size this run, so nothing was derived: {:?}",
        stats.derived_stepover
    );
}

// ── Gate 2: the routing/fan differential (red-first) ────────────────────

/// The verdict FLIPS on the shipped taper once the policy is live.
///
/// Same mesh, same tool, same reference, same cell, same 4-pass cap — the
/// only variable is the stepover the criterion multiplies that cap by. The
/// detector call is the one `attach_generic_rest_analysis` makes.
#[test]
fn the_policy_stepover_changes_the_routing_verdict() {
    let mesh = grooved_block(8.0, 50.0, 5.0, 1.0);
    let index = SpatialIndex::build(&mesh, 4.0);
    let t = wanaka_taper();
    let reference = BallEndmill::new(12.0, 25.0);
    let min_valley_depth = 0.05;
    let policy = rs_cam_core::reach::suggested_offset_stepover_mm(&t, min_valley_depth);

    let run = |stepover: f64| {
        let params = RestFieldParams {
            cell_mm: 0.25,
            min_valley_depth,
            offset_stepover_mm: stepover,
            num_offset_passes_cap: 4,
            ..RestFieldParams::default()
        };
        detect_rest_valleys(
            &mesh,
            &index,
            &t,
            RestReference::Cutter {
                tool: &reference,
                is_surface_probe: false,
            },
            &params,
        )
    };

    let retired = run(RETIRED_DETECTOR_STEPOVER_MM);
    let live = run(policy);
    println!(
        "PR-7 routing: retired 0.5 -> {} centrelines / {} pencil comps / {:.1} mm traced; \
         policy {policy:.3} -> {} centrelines / {} clearing comps / {:.1} mm traced",
        retired.centerlines.len(),
        retired.report.pencil_region_count,
        retired.report.traced_length_mm,
        live.centerlines.len(),
        live.report.clearing_region_count,
        live.report.traced_length_mm,
    );

    // The measured reach that decides it, printed so the fixture explains
    // itself rather than being a magic geometry.
    let median_reach = {
        let cl = retired
            .centerlines
            .first()
            .expect("non-vacuity: the retired arm must produce a centreline");
        let mut v: Vec<f64> = cl.samples.iter().map(|s| s.reach.min_mm()).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    println!("PR-7 median branch reach {median_reach:.3} mm");
    assert!(
        median_reach > 4.0 * policy && median_reach <= 4.0 * RETIRED_DETECTOR_STEPOVER_MM,
        "the fixture must sit BETWEEN the two thresholds ({:.3} .. {:.3}); got {median_reach:.3}",
        4.0 * policy,
        4.0 * RETIRED_DETECTOR_STEPOVER_MM
    );

    // Non-vacuity: the retired arm really did route these branches to pencil.
    assert!(retired.centerlines.len() >= 2);
    assert!(retired.report.pencil_region_count >= 1);
    assert!(retired.report.traced_length_mm > 1.0);

    // The claim: the fan this operation can actually emit does NOT cover the
    // reachable band, so the branches belong to a clearing strategy. The
    // retired literal over-claimed the fan by 2× and handed them to a pencil
    // pass that cannot clear them.
    assert!(
        live.centerlines.is_empty(),
        "the policy fan covers {:.3} mm; a {median_reach:.3} mm band is not a pencil job",
        4.0 * policy
    );
    assert!(live.report.clearing_region_count >= 1);
    assert!(live.report.traced_length_mm < 1e-9);
}

// ── Gate 3: what the operator's artifacts do — which is not move ────────

/// The attached artifacts are routing-INDEPENDENT today.
///
/// `region_polygons` comes off the cleaned rest MASK before any branch is
/// routed, and this pass discards `centerlines` / `clearing_regions`
/// entirely. So gate 2's flip changes nothing an operator sees — and this
/// test says so out loud rather than letting a reader infer an output delta
/// from the commit message. When this goes red, the pass has started
/// consuming the verdict and gate 2's difference has become real output.
#[test]
fn the_attached_artifacts_do_not_move_with_the_routing_verdict() {
    let polygons_for = |stepover: Option<f64>| {
        let cfg = RestAnalysisConfig {
            offset_stepover_mm: stepover,
            ..routing_rest_analysis()
        };
        let session = generate_with_rest_analysis(tapered_ball_tool(0), cfg);
        let result = session.get_result(0).expect("a generated result");
        let annotated = result.annotated();
        let regions = annotated
            .rest_regions
            .as_ref()
            .expect("the generic pass must attach region polygons");
        let grid_cells = annotated
            .rest_grid
            .as_ref()
            .expect("the generic pass must attach a rest grid")
            .rest
            .len();
        (
            regions.len(),
            regions.iter().map(|p| p.area()).sum::<f64>(),
            grid_cells,
        )
    };

    let policy_arm = polygons_for(None);
    let retired_arm = polygons_for(Some(RETIRED_DETECTOR_STEPOVER_MM));
    println!("PR-7 artifacts: policy {policy_arm:?} vs retired {retired_arm:?}");

    assert!(policy_arm.0 > 0, "non-vacuity: the pass must attach regions");
    assert!(policy_arm.2 > 0, "non-vacuity: the pass must attach a grid");
    assert_eq!(
        policy_arm.0, retired_arm.0,
        "region COUNT is mask-derived, not routing-derived"
    );
    assert!(
        (policy_arm.1 - retired_arm.1).abs() < 1e-9,
        "region AREA is mask-derived, not routing-derived"
    );
    assert_eq!(policy_arm.2, retired_arm.2, "the grid is not routed at all");
}

// ── Gate 4: serde compatibility, additive only ──────────────────────────

/// A project written before PR-7 loads unchanged, and the new dials neither
/// appear in output nor change any existing value when unset.
#[test]
fn the_new_dials_are_serde_additive() {
    // Exactly the shape a pre-PR-7 project carries.
    let legacy = r#"{
        "enabled": true,
        "cell_mm": 0.75,
        "min_valley_depth": 0.12,
        "region_margin_mm": 1.5
    }"#;
    let cfg: RestAnalysisConfig = serde_json::from_str(legacy).expect("legacy payload must load");
    assert!(cfg.enabled);
    assert!((cfg.cell_mm - 0.75).abs() < 1e-12);
    assert!(
        cfg.offset_stepover_mm.is_none(),
        "an absent dial means ASK THE POLICY, never a fabricated number"
    );
    assert!(cfg.num_offset_passes.is_none());

    // Defaults are `None`, and an unset dial is not written out.
    let default_json = serde_json::to_string(&RestAnalysisConfig::default()).unwrap();
    assert!(
        !default_json.contains("offset_stepover_mm") && !default_json.contains("num_offset_passes"),
        "unset optional dials must not bloat every saved project: {default_json}"
    );

    // A SET dial round-trips.
    let tuned = RestAnalysisConfig {
        offset_stepover_mm: Some(0.31),
        num_offset_passes: Some(4),
        ..RestAnalysisConfig::default()
    };
    let round: RestAnalysisConfig =
        serde_json::from_str(&serde_json::to_string(&tuned).unwrap()).unwrap();
    assert_eq!(round, tuned);
}

// ── Gate 5: the ball control ────────────────────────────────────────────

/// A plain ball's cusp radius IS its envelope radius, so the policy value
/// and the retired envelope rule coincide and no notice is raised. The
/// derivation still RAN — "not measured" would hide a regression.
#[test]
fn the_ball_control_derives_the_same_number_and_reports_nothing() {
    let session = generate_with_rest_analysis(ball_tool(0, 3.0), routing_rest_analysis());
    let stats = session.get_result(0).expect("a generated result").stats.clone();
    let finding = stats
        .derived_stepover
        .as_deref()
        .copied()
        .expect("the pass ran on the ball too");
    println!(
        "PR-7 ball: derived {:.4} mm, envelope rule {:.4} mm",
        finding.stepover_mm, finding.envelope_rule_mm
    );
    assert!(finding.matches_the_envelope_rule());
    let diags = rs_cam_core::diagnostics::adapters::from_generation::diagnostics_from_generation(
        ToolpathId(0),
        &stats,
    );
    assert!(
        !diags
            .iter()
            .any(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::CONFIG_DERIVED_STEPOVER),
        "nothing moved on a ball, so nothing may be reported"
    );
}
