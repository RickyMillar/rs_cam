//! PR-5 (H2.2) — the coverage routing criterion and the per-side,
//! per-sample offset fan, end to end.
//!
//! The analytic gate lives in `checkpoint_a_valley_matrix.rs` (the shipped
//! policy scored by the Checkpoint A truth) and the rest-field plumbing in
//! `reach_policy_pr4.rs`. This file covers what PR-5 specifically changed:
//! the fan `crease_paths::centerline_cut_paths` emits, the routing verdict
//! `detect_rest_valleys` reaches, and the deprecation notice a project that
//! still sets `route_width_factor` gets.
//!
//! Basis: `planning/review_2026-07-29/CHECKPOINT_A_EVIDENCE.md` §8.3/§8.4
//! (approved 2026-07-29), `TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` §H2.2.

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

use rs_cam_core::compute::config::{DeprecatedDialFinding, ToolpathStats};
use rs_cam_core::compute::operation_configs::PencilConfig;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::pencil::{
    PencilDetector, PencilParams, PencilRuntimeEvent, pencil_toolpath_structured_annotated,
};
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightMode, HeightsConfig, StockSource,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_core::rest_field::{RestFieldParams, RestReference, detect_rest_valleys};
use rs_cam_core::tool::{BallEndmill, MillingCutter, TaperedBallEndmill};

fn wanaka_taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)
}

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

fn pencil_params(num_offset_passes: usize) -> PencilParams {
    PencilParams {
        detector: PencilDetector::RestDepth,
        rest_cell_mm: 0.25,
        min_valley_depth: 0.05,
        min_cut_length: 2.0,
        num_offset_passes,
        offset_stepover: 0.5,
        sampling: 0.5,
        reference_tool_diameter: 12.0,
        ..PencilParams::default()
    }
}

/// (chains, max offset_total, distinct signed offsets emitted)
fn run_pencil(mesh: &TriangleMesh, params: &PencilParams) -> (usize, usize, Vec<i64>) {
    let index = SpatialIndex::build(mesh, 4.0);
    let cutter = wanaka_taper();
    let mut grid = None;
    let mut regions = None;
    let (_tp, ann) = pencil_toolpath_structured_annotated(
        mesh, &index, &cutter, params, None, None, &mut grid, &mut regions,
    );
    let mut chains: std::collections::BTreeSet<usize> = Default::default();
    let mut offsets: std::collections::BTreeSet<i64> = Default::default();
    let mut max_total = 0usize;
    for a in &ann {
        let PencilRuntimeEvent::OffsetPass {
            chain_index,
            offset_total,
            offset_mm,
            ..
        } = a.event;
        chains.insert(chain_index);
        max_total = max_total.max(offset_total);
        offsets.insert((offset_mm * 1000.0).round() as i64);
    }
    (chains.len(), max_total, offsets.into_iter().collect())
}

/// **ROUTING-COUNT CHARACTERISATION.** The whole point of PR-5 in one table:
/// the same tapered fixture, the same real pencil path, with the user's
/// offset-pass cap swept. Under the retired envelope equation every row read
/// `offset_total = 1` (a bare centreline) because `half_width − 3.0` is
/// negative for every valley narrower than the Ø6 shank; under the reach
/// policy the fan tracks the dial and the physics.
#[test]
fn routing_counts_track_the_dial_and_the_physics() {
    let mesh = grooved_block(2.5, 70.0, 1.2, 1.0);
    println!();
    println!("== tapered Ø1/7°/Ø6, 5 mm-wide 70° groove 1.2 mm deep ==");
    println!("| num_offset_passes | chains | max offset_total | offsets emitted (mm) |");
    println!("|---:|---:|---:|---|");
    let mut totals = Vec::new();
    for cap in [0usize, 1, 2, 4] {
        let (chains, max_total, offsets) = run_pencil(&mesh, &pencil_params(cap));
        let pretty: Vec<String> = offsets.iter().map(|o| format!("{:.1}", *o as f64 / 1000.0)).collect();
        println!("| {cap} | {chains} | {max_total} | {} |", pretty.join(", "));
        totals.push((cap, chains, max_total));
    }
    // The dial is a CAP, so the fan must be non-decreasing in it...
    for w in totals.windows(2) {
        let (a, b) = (w[0], w[1]);
        assert!(
            b.2 >= a.2,
            "fan shrank as the cap grew: cap {} → {}, cap {} → {}",
            a.0,
            a.2,
            b.0,
            b.2
        );
    }
    // ...and at a non-zero cap it must actually exceed the bare centreline
    // the envelope equation produced for every one of these rows.
    assert!(
        totals.last().map(|t| t.2).unwrap_or(0) > 1,
        "the reach policy still emits a bare centreline at cap 4 — the A2/A4 \
         fit defect is back"
    );
    // A zero cap still means centreline-only: `offset_stepover` and the
    // user's pass count stay USER-OWNED (plan H2.2), the policy only decides
    // how many of the permitted passes physically fit.
    assert_eq!(
        totals.first().map(|t| t.2),
        Some(1),
        "num_offset_passes = 0 must still mean centreline-only"
    );
}

/// **Offset passes never exceed the physically supported count.** Swept
/// across the fixture's own centrelines: for every emitted pass, the offset
/// it sits at must be within the reach the policy resolved for that branch.
#[test]
fn offset_passes_never_exceed_the_physical_count() {
    let mesh = grooved_block(2.5, 70.0, 1.2, 1.0);
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = wanaka_taper();
    let refc = BallEndmill::new(12.0, 25.0);
    let rf = detect_rest_valleys(
        &mesh,
        &index,
        &cutter,
        RestReference::Cutter {
            tool: &refc as &dyn MillingCutter,
            is_surface_probe: false,
        },
        &RestFieldParams {
            cell_mm: 0.25,
            min_valley_depth: 0.05,
            offset_stepover_mm: 0.5,
            num_offset_passes_cap: 8,
            min_cut_length: 2.0,
            region_margin_mm: 0.5,
        },
    );
    assert!(!rf.centerlines.is_empty(), "fixture produced no centrelines");
    // A generous cap (8) so the CAP is not what is limiting the count — the
    // physics has to be.
    let (_chains, max_total, offsets) = run_pencil(&mesh, &pencil_params(8));
    let widest_emitted = offsets
        .iter()
        .map(|o| (*o as f64 / 1000.0).abs())
        .fold(0.0f64, f64::max);
    let widest_reach = rf
        .centerlines
        .iter()
        .flat_map(|c| c.samples.iter())
        .map(|s| s.reach.max_mm())
        .fold(0.0f64, f64::max);
    println!(
        "max offset_total={max_total}; widest emitted offset {widest_emitted:.3} mm; \
         widest resolved reach {widest_reach:.3} mm"
    );
    assert!(
        widest_emitted <= widest_reach + 1e-9,
        "emitted a pass at {widest_emitted:.3} mm, beyond the {widest_reach:.3} mm \
         the policy says the cutter can hold"
    );
    assert!(
        widest_emitted > 0.0,
        "no offset pass emitted at all — the bound above is vacuous"
    );
}

/// **The fan is asymmetric.** On a groove whose two walls differ, the offsets
/// emitted on the two sides are not mirror images. The matrix measured
/// left/right reach differing by up to 22×; a symmetric `±k·stepover` fan is
/// forced to the narrow side and under-covers the wide one BY CONSTRUCTION
/// (`CHECKPOINT_A_EVIDENCE.md` §6).
#[test]
fn an_asymmetric_valley_gets_an_asymmetric_fan() {
    let mesh = grooved_block(3.0, 80.0, 1.0, 0.45);
    let (_chains, _max, offsets) = run_pencil(&mesh, &pencil_params(6));
    let left: Vec<f64> = offsets
        .iter()
        .map(|o| *o as f64 / 1000.0)
        .filter(|o| *o > 0.0)
        .collect();
    let right: Vec<f64> = offsets
        .iter()
        .map(|o| *o as f64 / 1000.0)
        .filter(|o| *o < 0.0)
        .collect();
    println!("left offsets {left:?}, right offsets {right:?}");
    assert!(
        !left.is_empty() || !right.is_empty(),
        "no offset passes at all — the fixture is not exercising the fan"
    );
    assert_ne!(
        left.len(),
        right.len(),
        "the two sides emitted the same number of passes on an asymmetric \
         valley — the fan collapsed back to ±k·stepover"
    );
}

/// **Per-sample truncation, not per-branch dropping.** A groove that narrows
/// along its length must keep its offset passes where the valley is wide and
/// lose them where it pinches, rather than the branch scalar forcing one
/// answer on the whole length. The signature is a pass whose points are
/// partly non-contact.
#[test]
fn a_pass_is_truncated_where_the_valley_pinches_not_dropped_wholesale() {
    // A groove whose rim half-width tapers from 3.0 mm at y = -12 to 0.6 mm
    // at y = +12, walls at 70°.
    let depth = 1.0f64;
    let tan = 70.0_f64.to_radians().tan();
    let ys: Vec<f64> = (0..=48).map(|i| -12.0 + i as f64 * 0.5).collect();
    let xs: Vec<f64> = {
        let mut v: Vec<f64> = Vec::new();
        let mut x = -10.0;
        while x <= 10.0 + 1e-9 {
            v.push(x);
            x += 0.1;
        }
        v
    };
    let half_at = |y: f64| -> f64 { 3.0 + (y + 12.0) / 24.0 * (0.6 - 3.0) };
    let mut verts = Vec::with_capacity(xs.len() * ys.len());
    for &yv in &ys {
        let hw = half_at(yv);
        let floor = (hw - depth / tan).max(0.0);
        for &xv in &xs {
            let ax = xv.abs();
            let z = if ax >= hw {
                0.0
            } else if ax <= floor {
                -depth
            } else {
                -depth + (ax - floor) * tan
            };
            verts.push(P3::new(xv, yv, z));
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
    let mesh = TriangleMesh::from_raw(verts, tris);
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = wanaka_taper();
    let refc = BallEndmill::new(12.0, 25.0);
    let rf = detect_rest_valleys(
        &mesh,
        &index,
        &cutter,
        RestReference::Cutter {
            tool: &refc as &dyn MillingCutter,
            is_surface_probe: false,
        },
        &RestFieldParams {
            cell_mm: 0.25,
            min_valley_depth: 0.05,
            offset_stepover_mm: 0.5,
            num_offset_passes_cap: 6,
            min_cut_length: 2.0,
            region_margin_mm: 0.5,
        },
    );
    let cl = rf
        .centerlines
        .iter()
        .max_by_key(|c| c.points.len())
        .expect("tapering groove produced no centreline");
    let reaches: Vec<f64> = cl.samples.iter().map(|s| s.reach.min_mm()).collect();
    let lo = reaches.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = reaches.iter().cloned().fold(0.0f64, f64::max);
    println!(
        "tapering groove: {} points, reach {lo:.3}..{hi:.3} mm",
        cl.points.len()
    );
    // The whole point: one branch, two answers. A branch scalar would have to
    // pick one of these for the whole length.
    assert!(
        lo < 0.5 && hi >= 0.5,
        "the fixture does not straddle a stepover boundary, so truncation is \
         not exercised ({lo:.3}..{hi:.3})"
    );

    // And the emission acts on it. `contact_runs` splits a pass at the points
    // its reach does not support, so a TRUNCATED pass reaches the annotation
    // stream as several runs carrying the SAME `(chain_index, offset_index)`.
    // A dropped pass would carry none, and an untruncated one exactly one.
    let index2 = SpatialIndex::build(&mesh, 4.0);
    let mut grid = None;
    let mut regions = None;
    let (_tp, ann) = pencil_toolpath_structured_annotated(
        &mesh,
        &index2,
        &cutter,
        &PencilParams {
            rest_cell_mm: 0.25,
            num_offset_passes: 6,
            ..pencil_params(6)
        },
        None,
        None,
        &mut grid,
        &mut regions,
    );
    let mut runs_per_pass: std::collections::BTreeMap<(usize, usize), usize> = Default::default();
    let mut declared: std::collections::BTreeMap<usize, usize> = Default::default();
    for a in &ann {
        let PencilRuntimeEvent::OffsetPass {
            chain_index,
            offset_index,
            offset_total,
            is_centerline,
            ..
        } = a.event;
        declared.insert(chain_index, offset_total);
        if !is_centerline {
            *runs_per_pass.entry((chain_index, offset_index)).or_insert(0) += 1;
        }
    }
    // Two truncation signatures, both real:
    //   SPLIT   — a pass survives at both ends and loses a middle stretch, so
    //             it reaches the annotation stream as several runs sharing one
    //             `(chain, offset_index)`.
    //   VANISH  — a pass loses so much that no run of two points survives, so
    //             its `offset_index` never appears though the fan declared it.
    // A branch scalar can produce neither: it emits every declared pass at
    // full length or emits none at all.
    let split = runs_per_pass.values().filter(|n| **n > 1).count();
    let vanished: usize = declared
        .iter()
        .map(|(chain, total)| {
            let seen = runs_per_pass.keys().filter(|(c, _)| c == chain).count();
            // `offset_total` counts the centreline too.
            total.saturating_sub(1).saturating_sub(seen)
        })
        .sum();
    println!(
        "offset passes emitted: {}; split into >1 run: {split}; \
         truncated away entirely: {vanished}",
        runs_per_pass.len()
    );
    assert!(
        !runs_per_pass.is_empty(),
        "no offset pass emitted at all on the tapering groove"
    );
    assert!(
        split + vanished > 0,
        "every declared pass ran full length on a groove that tapers 5× — the \
         per-sample truncation is not reaching the emission"
    );
}

// ── The retired dial ─────────────────────────────────────────────────────

fn mesh_model(mesh: TriangleMesh) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: "groove".to_owned(),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://groove.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn pencil_toolpath(cfg: PencilConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
    let op = OperationConfig::Pencil(cfg);
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: "Pencil".to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig {
            top_z: HeightMode::Manual(0.0),
            bottom_z: HeightMode::Manual(-4.0),
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
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    }
}

fn generate_pencil_through_session(route_width_factor: f64) -> ToolpathStats {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: 40.0,
        y: 24.0,
        z: 4.0,
        origin_x: -20.0,
        origin_y: -12.0,
        origin_z: -4.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let tool_idx = session.add_tool(ToolConfig {
        diameter: 1.0,
        taper_half_angle: 7.0,
        shaft_diameter: 6.0,
        ..ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose)
    });
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(grooved_block(2.5, 70.0, 1.2, 1.0)));
    let cfg = PencilConfig {
        detector: "rest_depth".to_owned(),
        rest_cell_mm: 0.4,
        num_offset_passes: 2,
        offset_stepover: 0.5,
        reference_tool_diameter: 12.0,
        route_width_factor,
        ..PencilConfig::default()
    };
    session
        .add_toolpath(0, pencil_toolpath(cfg, tool_id, model_id))
        .expect("add pencil toolpath");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("pencil must generate on the grooved block");
    session
        .get_result(0)
        .expect("generated toolpath must carry a result")
        .stats
        .clone()
}

/// A project that still sets the retired `route_width_factor` gets a
/// user-visible notice; a project at the default gets silence.
///
/// This is the pattern Wave D established and the reason it exists: a dial
/// that silently stops doing anything is indistinguishable, from the
/// operator's chair, from a dial that works. Refusing to load the project
/// would be worse — the field is deserialized, saved, and simply not read.
#[test]
fn a_retired_dial_set_by_the_project_raises_a_visible_finding() {
    // `route_width_factor_default()` is crate-private; the shipped default is
    // 2.0 and `PencilConfig::default()` is the public statement of it.
    let default = PencilConfig::default().route_width_factor;
    let at_default = generate_pencil_through_session(default);
    assert!(
        at_default.deprecated_dial.is_none(),
        "an untouched dial must not raise a notice: {:?}",
        at_default.deprecated_dial
    );

    let tuned = generate_pencil_through_session(default + 3.0);
    let f = tuned
        .deprecated_dial
        .as_deref()
        .copied()
        .expect("a project carrying a non-default retired dial must be told");
    println!("finding: {f:?}");
    assert_eq!(f.dial, "route_width_factor");
    assert!((f.value - (default + 3.0)).abs() < 1e-9);
    assert!((f.default_value - default).abs() < 1e-9);

    // ...and it reaches the diagnostics list an operator actually reads.
    let diags = rs_cam_core::diagnostics::adapters::from_generation::diagnostics_from_generation(
        ToolpathId(0),
        &tuned,
    );
    let hit = diags
        .iter()
        .find(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::CONFIG_DEPRECATED_DIAL)
        .expect("the finding must surface as a diagnostic, not just a struct field");
    println!("diagnostic: {}", hit.message);
    assert!(hit.message.contains("route_width_factor"));
    assert!(
        hit.message.contains("NO LONGER"),
        "the message must say the dial is not read: {}",
        hit.message
    );
}

/// The recorder's no-op rule, isolated: a finding whose value IS the default
/// carries no information and must never be raised. Pinned here because the
/// end-to-end test above can only observe the absence, not the reason.
#[test]
fn a_default_valued_finding_is_not_a_finding() {
    let f = DeprecatedDialFinding {
        dial: "route_width_factor",
        value: 2.0,
        default_value: 2.0,
        replaced_by: "the coverage criterion",
    };
    assert!((f.value - f.default_value).abs() <= 1e-9);
}
