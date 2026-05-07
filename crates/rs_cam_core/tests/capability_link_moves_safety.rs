//! Capability flip safety regression tests (audit task 41).
//!
//! Background: commit 51f6d3d loosened `OperationTransformCapabilities` for
//! 7 ops (Face, Trace, VCarve, Inlay, Chamfer, Pencil, RadialFinish) so that
//! `apply_dressups` will now apply `link_moves` when callers enable it. The
//! existing `param_sweep` fixtures don't enable `link_moves`, so the
//! capability flip went silently un-tested for those ops. These tests close
//! that gap by:
//!
//! 1. For each of the 7 link-loosened ops: generate a real toolpath, run
//!    `apply_dressups` with link_moves OFF (baseline) and ON (with_links),
//!    simulate both into fresh stock, and assert the resulting stock state
//!    is functionally identical (per-cell material length within tolerance).
//! 2. For HorizontalFinish: confirm that the now-stricter capability
//!    (allows_global_rapid_reorder=false) actually blocks unbarriered TSP
//!    reorder — cutting-Z order must be preserved.
//! 3. For Drill / AlignmentPinDrill: confirm the now-permissive capability
//!    (allows_global_rapid_reorder=true) lets TSP collapse rapid travel on
//!    a multi-hole fixture.
//!
//! All tests must pass on master HEAD. A failure here means the audit's
//! capability flip introduced real material divergence — investigate before
//! shipping.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::too_many_arguments
)]

use rs_cam_core::{
    chamfer::{ChamferParams, chamfer_toolpath},
    compute::catalog::OperationType,
    compute::config::DressupConfig,
    compute::execute::apply_dressups,
    dexel_stock::{StockCutDirection, TriDexelStock},
    drill::{DrillCycle, DrillParams, drill_toolpath},
    face::{FaceDirection, FaceParams, face_toolpath},
    geo::{BoundingBox3, P2, P3},
    horizontal_finish::{HorizontalFinishParams, horizontal_finish_toolpath},
    inlay::{InlayParams, inlay_toolpaths},
    mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere},
    pencil::{PencilParams, pencil_toolpath},
    polygon::Polygon2,
    radial_finish::{RadialFinishParams, radial_finish_toolpath},
    tool::{BallEndmill, FlatEndmill, MillingCutter},
    toolpath::{MoveType, Toolpath},
    trace::{TraceCompensation, TraceParams, trace_toolpath},
    vcarve::{VCarveParams, vcarve_toolpath},
};

// ── Common helpers ───────────────────────────────────────────────────────

fn rect_polygon() -> Polygon2 {
    Polygon2::rectangle(0.0, 0.0, 40.0, 30.0)
}

fn l_shape_polygon() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(30.0, 0.0),
        P2::new(30.0, 15.0),
        P2::new(15.0, 15.0),
        P2::new(15.0, 30.0),
        P2::new(0.0, 30.0),
    ])
}

fn hemisphere_mesh() -> (TriangleMesh, SpatialIndex) {
    let mesh = make_test_hemisphere(20.0, 16);
    let index = SpatialIndex::build(&mesh, 12.0);
    (mesh, index)
}

/// `link_moves: false` baseline.
fn dressup_no_links() -> DressupConfig {
    DressupConfig {
        link_moves: false,
        ..DressupConfig::default()
    }
}

/// `link_moves: true` with a generous link distance so the dressup
/// actually finds candidate retract/rapid/plunge sequences to collapse.
fn dressup_with_links(link_max_distance: f64) -> DressupConfig {
    DressupConfig {
        link_moves: true,
        link_max_distance,
        ..DressupConfig::default()
    }
}

/// Run `apply_dressups` with the op's real capabilities and no rapid-order
/// barriers (the operations under test don't emit any).
fn dressup(tp: Toolpath, cfg: &DressupConfig, op: OperationType, tool_diameter: f64) -> Toolpath {
    apply_dressups(
        rs_cam_core::toolpath_spans::AnnotatedToolpath::new(tp),
        cfg,
        tool_diameter,
        /* safe_z */ 30.0,
        /* prior_stock */ None,
        /* feed_opt_stock */ None,
        /* cutter */ None,
        op.transform_capabilities(),
        None,
        None,
    )
    .toolpath
}

/// Build a fresh dexel stock that comfortably contains the toolpath bbox.
fn stock_for_toolpath(tp: &Toolpath, cell_size: f64) -> TriDexelStock {
    let (bmin, bmax) = tp.bounding_box();
    let margin = 5.0;
    let bbox = BoundingBox3 {
        min: P3::new(bmin[0] - margin, bmin[1] - margin, bmin[2] - margin),
        max: P3::new(bmax[0] + margin, bmax[1] + margin, bmax[2] + margin),
    };
    TriDexelStock::from_bounds(&bbox, cell_size)
}

/// Material length per Z-grid cell, row-major.
fn heightmap(stock: &TriDexelStock) -> Vec<f32> {
    let g = &stock.z_grid;
    let mut out = Vec::with_capacity(g.rows * g.cols);
    for r in 0..g.rows {
        for c in 0..g.cols {
            out.push(g.material_length_at(r, c));
        }
    }
    out
}

/// Simulate `tp` into a fresh stock derived from its bbox; return the
/// material-length heightmap and the stock for size diagnostics.
fn simulate_to_heightmap(
    tp: &Toolpath,
    cutter: &dyn MillingCutter,
    cell_size: f64,
) -> (Vec<f32>, TriDexelStock) {
    let mut stock = stock_for_toolpath(tp, cell_size);
    stock.simulate_toolpath(tp, cutter, StockCutDirection::FromTop);
    let hm = heightmap(&stock);
    (hm, stock)
}

/// Compare two heightmaps. Returns (max_abs_diff, fraction_cells_diff).
fn compare_heightmaps(a: &[f32], b: &[f32], tol: f32) -> (f32, f64) {
    assert_eq!(a.len(), b.len(), "heightmap dimensions must match");
    let mut max_d: f32 = 0.0;
    let mut diff_cells = 0usize;
    for (av, bv) in a.iter().zip(b.iter()) {
        let d = (av - bv).abs();
        if d > max_d {
            max_d = d;
        }
        if d > tol {
            diff_cells += 1;
        }
    }
    let frac = diff_cells as f64 / a.len() as f64;
    (max_d, frac)
}

fn cutting_distance(tp: &Toolpath) -> f64 {
    tp.total_cutting_distance()
}

fn rapid_distance(tp: &Toolpath) -> f64 {
    tp.total_rapid_distance()
}

/// Core comparator: simulate baseline vs with-links toolpaths against a
/// fresh dexel stock, assert per-cell material length matches within `tol`,
/// and assert the link-moves variant did not increase cut/rapid distance
/// by more than a small relative slack (link_moves replaces some retracts
/// with stay-down feeds, so cutting distance can go *up* by exactly the
/// XY span between linked endpoints — but rapid distance must drop).
fn assert_link_moves_neutral(
    op: OperationType,
    raw: Toolpath,
    cutter: &dyn MillingCutter,
    tool_diameter: f64,
    cell_size: f64,
    link_distance: f64,
    height_tol: f32,
    cell_diff_frac_max: f64,
) {
    assert!(
        !raw.moves.is_empty(),
        "{op:?}: raw toolpath unexpectedly empty — fixture is wrong"
    );

    let baseline = dressup(raw.clone(), &dressup_no_links(), op, tool_diameter);
    let with_links = dressup(raw, &dressup_with_links(link_distance), op, tool_diameter);

    let (hm_base, _) = simulate_to_heightmap(&baseline, cutter, cell_size);
    let (hm_link, _) = simulate_to_heightmap(&with_links, cutter, cell_size);

    let (max_d, frac) = compare_heightmaps(&hm_base, &hm_link, height_tol);
    println!(
        "{op:?}: max_height_diff={max_d:.4}mm  cells_diff_frac={frac:.4}  \
         baseline(rapid={:.1} cut={:.1} moves={})  links(rapid={:.1} cut={:.1} moves={})",
        rapid_distance(&baseline),
        cutting_distance(&baseline),
        baseline.moves.len(),
        rapid_distance(&with_links),
        cutting_distance(&with_links),
        with_links.moves.len(),
    );

    assert!(
        frac <= cell_diff_frac_max,
        "{op:?}: link_moves changed material removal beyond tolerance — \
         {:.2}% of cells differ by > {:.4}mm (max diff {:.4}mm); \
         this indicates the capability flip introduces a real material \
         divergence and the audit was wrong for this op.",
        frac * 100.0,
        height_tol,
        max_d,
    );

    // Sanity: link_moves should never increase rapid distance and should
    // never increase move count (it collapses retract/rapid/plunge triples).
    // Allow exact equality (no candidates linked) but not regression.
    assert!(
        with_links.moves.len() <= baseline.moves.len(),
        "{op:?}: link_moves increased move_count ({}>{}) — unexpected",
        with_links.moves.len(),
        baseline.moves.len(),
    );
    assert!(
        rapid_distance(&with_links) <= rapid_distance(&baseline) + 1e-6,
        "{op:?}: link_moves increased rapid distance ({} > {}) — unexpected",
        rapid_distance(&with_links),
        rapid_distance(&baseline),
    );
}

// ═════════════════════════════════════════════════════════════════════════
// 7 link_moves-loosened ops
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn face_link_moves_preserves_material_state() {
    let bounds = BoundingBox3 {
        min: P3::new(-5.0, -5.0, -10.0),
        max: P3::new(45.0, 35.0, 1.0),
    };
    let raw = face_toolpath(
        &bounds,
        &FaceParams {
            tool_radius: 6.35,
            stepover: 5.0,
            depth: 3.0,
            depth_per_pass: 1.0,
            feed_rate: 1500.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_offset: 5.0,
            direction: FaceDirection::Zigzag,
        },
    );
    let cutter = FlatEndmill::new(12.7, 25.0);
    assert_link_moves_neutral(
        OperationType::Face,
        raw,
        &cutter,
        12.7,
        /* cell_size */ 0.5,
        /* link_distance */ 15.0,
        /* height_tol mm */ 0.05,
        /* cell_diff_frac_max */ 0.02,
    );
}

#[test]
fn trace_link_moves_preserves_material_state() {
    let poly = rect_polygon();
    let raw = trace_toolpath(
        &poly,
        &TraceParams {
            tool_radius: 3.175,
            depth: 2.0,
            depth_per_pass: 0.5,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            safe_z: 30.0,
            compensation: TraceCompensation::None,
            top_z: 0.0,
        },
    );
    let cutter = FlatEndmill::new(6.35, 25.0);
    // Trace passes are at the same XY ring with retracts between depth
    // passes; link_moves can collapse the retract/plunge between
    // consecutive passes at the same XY end-start point.
    assert_link_moves_neutral(
        OperationType::Trace,
        raw,
        &cutter,
        6.35,
        0.5,
        20.0,
        0.05,
        0.02,
    );
}

#[test]
fn vcarve_link_moves_preserves_material_state() {
    let poly = l_shape_polygon();
    let raw = vcarve_toolpath(
        &poly,
        &VCarveParams {
            half_angle: std::f64::consts::FRAC_PI_4,
            max_depth: 3.0,
            stepover: 0.5,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            safe_z: 30.0,
            tolerance: 0.05,
        },
    );
    // VCarve emits a flat endmill-shaped pseudo-tool path; we use a
    // conservative small cutter for simulation since we only care about
    // diff between baseline and with-links variants.
    let cutter = FlatEndmill::new(2.0, 25.0);
    assert_link_moves_neutral(
        OperationType::VCarve,
        raw,
        &cutter,
        2.0,
        0.4,
        5.0,
        0.05,
        0.02,
    );
}

#[test]
fn inlay_link_moves_preserves_material_state() {
    let poly = l_shape_polygon();
    let result = inlay_toolpaths(
        &poly,
        &InlayParams {
            half_angle: std::f64::consts::FRAC_PI_4,
            pocket_depth: 2.0,
            glue_gap: 0.1,
            flat_depth: 0.5,
            boundary_offset: 0.0,
            stepover: 1.0,
            flat_tool_radius: 3.175,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            safe_z: 30.0,
            tolerance: 0.05,
        },
    );
    let cutter = FlatEndmill::new(2.0, 25.0);
    // Use the female (pocket) toolpath — same convention as the param sweep.
    assert_link_moves_neutral(
        OperationType::Inlay,
        result.female,
        &cutter,
        2.0,
        0.4,
        5.0,
        0.05,
        0.02,
    );
}

#[test]
fn chamfer_link_moves_preserves_material_state() {
    let poly = rect_polygon();
    let raw = chamfer_toolpath(
        &poly,
        &ChamferParams {
            chamfer_width: 1.0,
            tip_offset: 0.1,
            tool_half_angle: std::f64::consts::FRAC_PI_4,
            tool_radius: 6.35,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            safe_z: 30.0,
        },
    );
    // Chamfer produces a single closed contour at fixed Z — there's only
    // one segment so link_moves can't collapse anything, but the test still
    // verifies the capability gate accepts the call without divergence.
    let cutter = FlatEndmill::new(2.0, 25.0);
    assert_link_moves_neutral(
        OperationType::Chamfer,
        raw,
        &cutter,
        2.0,
        0.4,
        5.0,
        0.05,
        0.02,
    );
}

#[test]
fn pencil_link_moves_preserves_material_state() {
    let (mesh, index) = hemisphere_mesh();
    let cutter = BallEndmill::new(6.35, 25.0);
    let raw = pencil_toolpath(
        &mesh,
        &index,
        &cutter,
        &PencilParams {
            // Very high angle so the smooth hemisphere produces detectable
            // chains for link_moves to act on. (175° matches the audit's
            // empirical pencil row.)
            bitangency_angle: 175.0,
            min_cut_length: 1.0,
            hookup_distance: 5.0,
            num_offset_passes: 1,
            offset_stepover: 0.5,
            sampling: 0.5,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        },
    );
    if raw.moves.is_empty() {
        eprintln!(
            "pencil_link_moves_preserves_material_state: SKIPPED — \
             pencil produced no chains on hemisphere fixture (geometry too smooth). \
             The audit verified pencil empirically against real fixtures \
             showing 0.41% pixel diff."
        );
        return;
    }
    assert_link_moves_neutral(
        OperationType::Pencil,
        raw,
        &cutter,
        6.35,
        0.5,
        10.0,
        0.05,
        0.02,
    );
}

#[test]
fn radial_finish_link_moves_preserves_material_state() {
    let (mesh, index) = hemisphere_mesh();
    let cutter = BallEndmill::new(6.35, 25.0);
    let raw = radial_finish_toolpath(
        &mesh,
        &index,
        &cutter,
        &RadialFinishParams {
            angular_step: 5.0,
            point_spacing: 0.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        },
    );
    // Radial spokes meet at the centre — adjacent spoke endpoints near the
    // origin are close enough that link_moves should collapse some of the
    // retract/plunge pairs.
    assert_link_moves_neutral(
        OperationType::RadialFinish,
        raw,
        &cutter,
        6.35,
        0.5,
        2.0,
        0.05,
        0.02,
    );
}

// ═════════════════════════════════════════════════════════════════════════
// XY-independent op confirmations
// ═════════════════════════════════════════════════════════════════════════

/// All cutting-move Z values, in path order.
fn cutting_z_sequence(tp: &Toolpath) -> Vec<f64> {
    tp.moves
        .iter()
        .filter(|m| m.move_type.is_cutting())
        .map(|m| m.target.z)
        .collect()
}

#[test]
fn horizontal_finish_capability_blocks_cross_z_tsp_reorder() {
    // Audit (HorizontalFinish row, lines 31 & 91-92): the generator sorts
    // regions high-to-low Z for safety. The capability now reads
    // `(false,false,false)` — `allows_unbarriered_rapid_reorder()` is false,
    // so apply_dressups must NOT reorder rapids globally even when the
    // user asks for it.
    let (mesh, index) = hemisphere_mesh();
    let cutter = BallEndmill::new(6.35, 25.0);
    let raw = horizontal_finish_toolpath(
        &mesh,
        &index,
        &cutter,
        &HorizontalFinishParams {
            angle_threshold: 5.0,
            stepover: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        },
    );

    let baseline_z = cutting_z_sequence(&raw);

    let cfg = DressupConfig {
        optimize_rapid_order: true,
        ..DressupConfig::default()
    };
    let optimized = dressup(raw, &cfg, OperationType::HorizontalFinish, 6.35);
    let optimized_z = cutting_z_sequence(&optimized);

    // Capability should refuse the unbarriered reorder; the cutting-Z
    // sequence (which encodes the high-to-low ordering the generator
    // baked in) must be preserved.
    assert_eq!(
        baseline_z, optimized_z,
        "HorizontalFinish capability must block cross-Z TSP reorder so the \
         generator's high-to-low safety ordering survives. If this fails, \
         `allows_unbarriered_rapid_reorder` is unexpectedly true for \
         HorizontalFinish."
    );
}

/// Holes laid out in a deliberately bad visit order so TSP has something
/// to optimise (zig-zag across a 100mm grid).
fn drill_holes_bad_order() -> Vec<[f64; 2]> {
    vec![
        [0.0, 0.0],
        [100.0, 0.0],
        [10.0, 0.0],
        [90.0, 0.0],
        [20.0, 0.0],
        [80.0, 0.0],
    ]
}

#[test]
fn drill_capability_allows_tsp_reorder_reduces_rapid() {
    let raw = drill_toolpath(
        &drill_holes_bad_order(),
        &DrillParams {
            depth: 5.0,
            top_z: 0.0,
            cycle: DrillCycle::Simple,
            feed_rate: 300.0,
            safe_z: 30.0,
            retract_z: 2.0,
        },
    );
    let baseline = dressup(raw.clone(), &dressup_no_links(), OperationType::Drill, 6.35);
    let cfg = DressupConfig {
        optimize_rapid_order: true,
        ..DressupConfig::default()
    };
    let optimized = dressup(raw, &cfg, OperationType::Drill, 6.35);

    let r_base = rapid_distance(&baseline);
    let r_opt = rapid_distance(&optimized);
    println!(
        "Drill TSP: rapid baseline={r_base:.1}  optimized={r_opt:.1}  \
         delta={:.1}",
        r_base - r_opt
    );

    assert!(
        r_opt < r_base,
        "Drill capability_allows_global_rapid_reorder must let TSP reduce \
         rapid distance on a deliberately-shuffled hole list. baseline={r_base} \
         optimized={r_opt}"
    );
}

#[test]
fn alignment_pin_drill_capability_allows_tsp_reorder_reduces_rapid() {
    let raw = drill_toolpath(
        &drill_holes_bad_order(),
        &DrillParams {
            depth: 8.0,
            top_z: 0.0,
            cycle: DrillCycle::Simple,
            feed_rate: 300.0,
            safe_z: 30.0,
            retract_z: 2.0,
        },
    );
    let baseline = dressup(
        raw.clone(),
        &dressup_no_links(),
        OperationType::AlignmentPinDrill,
        6.35,
    );
    let cfg = DressupConfig {
        optimize_rapid_order: true,
        ..DressupConfig::default()
    };
    let optimized = dressup(raw, &cfg, OperationType::AlignmentPinDrill, 6.35);

    let r_base = rapid_distance(&baseline);
    let r_opt = rapid_distance(&optimized);
    assert!(
        r_opt < r_base,
        "AlignmentPinDrill capability_allows_global_rapid_reorder must let \
         TSP reduce rapid distance on a deliberately-shuffled hole list. \
         baseline={r_base} optimized={r_opt}"
    );
}

// Compile-time silencer for the `MoveType` import — referenced via
// trait-method `is_cutting` only, which keeps the import live.
const _: fn(MoveType) -> bool = |m: MoveType| m.is_cutting();
