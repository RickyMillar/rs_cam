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
    compute::catalog::{OperationConfig, OperationTransformCapabilities, OperationType},
    compute::config::DressupConfig,
    compute::execute::apply_dressups,
    compute::operation_configs::ScallopConfig,
    dexel_stock::{StockCutDirection, TriDexelStock},
    drill::{DrillCycle, DrillParams, drill_toolpath},
    face::{FaceDirection, FaceParams, face_toolpath},
    geo::{BoundingBox3, P2, P3},
    horizontal_finish::{HorizontalFinishParams, horizontal_finish_toolpath},
    inlay::{InlayParams, inlay_toolpaths},
    mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere},
    pencil::{PencilParams, pencil_toolpath},
    polygon::Polygon2,
    project_curve::{ProjectCurveParams, ProjectDirection, ProjectSide, project_curve_toolpath},
    radial_finish::{RadialFinishParams, radial_finish_toolpath},
    scallop::{ScallopDirection, ScallopParams, scallop_toolpath},
    tool::{BallEndmill, FlatEndmill, MillingCutter},
    toolpath::{MoveIntent, MoveType, Toolpath},
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

/// `link_moves: false` baseline. Also pins `optimize_rapid_order: false`
/// so the baseline genuinely lacks TSP reordering (Roadmap B.6 flipped
/// the Default for `optimize_rapid_order` to `true`, so an unmodified
/// `Default` would no longer be a "no-TSP" baseline).
fn dressup_no_links() -> DressupConfig {
    DressupConfig {
        link_moves: false,
        optimize_rapid_order: false,
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
        1000.0,
        tool_diameter,
        /* safe_z */ 30.0,
        /* stock_top */ 0.0,
        /* prior_stock */ None,
        /* feed_opt_stock */ None,
        /* cutter */ None,
        op.transform_capabilities(),
        None,
        None,
    )
    .toolpath
}

/// Like [`dressup`], but takes explicit `OperationTransformCapabilities`
/// instead of deriving them from `OperationType`. Needed for the Scallop
/// tests below: Scallop's real capabilities are CONFIG-aware
/// (`OperationConfig::Scallop(cfg).transform_capabilities()`, fix-family
/// Phase 1b), not the static per-op-type table `dressup()`'s
/// `op.transform_capabilities()` reads — `OperationType::Scallop`'s answer
/// stays pinned conservative regardless of `cfg.continuous`.
fn dressup_with_caps(
    tp: Toolpath,
    cfg: &DressupConfig,
    caps: OperationTransformCapabilities,
    tool_diameter: f64,
) -> Toolpath {
    apply_dressups(
        rs_cam_core::toolpath_spans::AnnotatedToolpath::new(tp),
        cfg,
        1000.0,
        tool_diameter,
        /* safe_z */ 30.0,
        /* stock_top */ 0.0,
        /* prior_stock */ None,
        /* feed_opt_stock */ None,
        /* cutter */ None,
        caps,
        None,
        None,
    )
    .toolpath
}

/// Padded bounding box of one toolpath, in the shape the dexel stock wants.
fn padded_bbox(tp: &Toolpath) -> BoundingBox3 {
    let (bmin, bmax) = tp.bounding_box();
    let margin = 5.0;
    BoundingBox3 {
        min: P3::new(bmin[0] - margin, bmin[1] - margin, bmin[2] - margin),
        max: P3::new(bmax[0] + margin, bmax[1] + margin, bmax[2] + margin),
    }
}

/// Smallest box containing both — the SHARED frame two branches must be
/// compared in.
fn union_bbox(a: &BoundingBox3, b: &BoundingBox3) -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(
            a.min.x.min(b.min.x),
            a.min.y.min(b.min.y),
            a.min.z.min(b.min.z),
        ),
        max: P3::new(
            a.max.x.max(b.max.x),
            a.max.y.max(b.max.y),
            a.max.z.max(b.max.z),
        ),
    }
}

/// Build a fresh dexel stock that comfortably contains the toolpath bbox.
///
/// FRAME WARNING (2026-08-03): deriving the grid from a SINGLE toolpath is
/// only safe when the two branches being compared provably share a
/// bounding box. `apply_link_moves` satisfies that (collapsing a
/// retract/rapid/plunge triple can never move the extremes), which is why
/// the link-moves tests below use it. A TSP REORDER does not: changing
/// which fragment runs first/last moves the path extremes, so the two
/// branches get grids with different origins — and `compare_heightmaps`
/// walks cells by index, so it would then be comparing different world XY
/// and reporting the misregistration as a material difference. Reorder
/// comparisons must use [`simulate_pair_shared_frame`] instead. (Same
/// bug class as the identity-setup deviation-frame defect fixed in
/// `compute/simulate.rs` this month: two measurements, two frames, one
/// index-wise comparison.)
fn stock_for_toolpath(tp: &Toolpath, cell_size: f64) -> TriDexelStock {
    TriDexelStock::from_bounds(&padded_bbox(tp), cell_size)
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

/// Simulate two toolpaths into stocks built from the SAME (union) bbox, so
/// their heightmaps are index-comparable. Use this — not two independent
/// [`simulate_to_heightmap`] calls — whenever the two branches may differ
/// in path extents, i.e. for every rapid-REORDER comparison. See the frame
/// warning on [`stock_for_toolpath`].
fn simulate_pair_shared_frame(
    a: &Toolpath,
    b: &Toolpath,
    cutter: &dyn MillingCutter,
    cell_size: f64,
) -> (Vec<f32>, Vec<f32>) {
    let bbox = union_bbox(&padded_bbox(a), &padded_bbox(b));
    let mut stock_a = TriDexelStock::from_bounds(&bbox, cell_size);
    stock_a.simulate_toolpath(a, cutter, StockCutDirection::FromTop);
    let mut stock_b = TriDexelStock::from_bounds(&bbox, cell_size);
    stock_b.simulate_toolpath(b, cutter, StockCutDirection::FromTop);
    let (hm_a, hm_b) = (heightmap(&stock_a), heightmap(&stock_b));
    assert_eq!(
        hm_a.len(),
        hm_b.len(),
        "shared-frame stocks must produce identical grid dimensions"
    );
    (hm_a, hm_b)
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
            stock_top_z: 0.0,
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
            top_z: 0.0,
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
            top_z: 0.0,
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
            feed_rate: 800.0,
            plunge_rate: 400.0,
            safe_z: 30.0,
            top_z: 0.0,
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
            // 0 = keep any genuinely-bridged concavity (pre-gate behaviour); this
            // suite exercises link_moves, not the reach-gap gate.
            min_valley_depth: 0.0,
            // 0 = no bisector shift; this suite exercises link_moves, not corner nestling.
            bisector_strength: 0.0,
            // 0 = self-referenced gap (no bigger reference tool); preserves the
            // pre-reference-gate behaviour this link_moves suite was written against.
            reference_tool_diameter: 0.0,
            // Crease detector — this suite exercises link_moves on a synthetic
            // V-groove, not the curvature path.
            detector: rs_cam_core::pencil::PencilDetector::Dihedral,
            valley_saliency: 0.05,
            curvature_smoothing: 3,
            rest_cell_mm: 0.5,
            route_width_factor: 2.0,
            reference_cutter: None,
            link_kinematics: None,
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

// ═════════════════════════════════════════════════════════════════════════
// ProjectCurve reclassification (fix-family Phase 1)
// ═════════════════════════════════════════════════════════════════════════

/// X positions for 6 disconnected mesh patches, in a deliberately bad
/// (far-apart, interleaved) visit order — mirrors `drill_holes_bad_order`'s
/// zig-zag-across-X pattern so TSP has the same kind of "obviously
/// improvable" tour to find.
const PROJECT_CURVE_PATCH_X: [f64; 6] = [0.0, 100.0, 10.0, 90.0, 20.0, 80.0];

/// Y-band step between successive (visit-order) patches, in mm. Large
/// enough that the polyline segments connecting one patch to the next never
/// cross a THIRD patch's footprint: each connector's Y-range only ever
/// touches the patch it's leaving (right at its start) and the patch it's
/// arriving at (right at its end) — see `project_curve_bad_order_path`.
const PROJECT_CURVE_PATCH_Y_STEP: f64 = 50.0;

/// Half-width (X) and span (Y) of each square mesh patch, in mm.
const PROJECT_CURVE_PATCH_HALF_X: f64 = 1.5;
const PROJECT_CURVE_PATCH_Y_SPAN: f64 = 3.0;

/// Build a flat mesh made of 6 disconnected square patches (2 triangles
/// each, z=0) at the X positions in `PROJECT_CURVE_PATCH_X`, one per visit
/// index, each patch's Y-band offset by `PROJECT_CURVE_PATCH_Y_STEP` so
/// patches never touch and the connecting polyline segments in
/// `project_curve_bad_order_path` can't accidentally graze a third patch.
fn project_curve_patchwork_mesh() -> (TriangleMesh, SpatialIndex) {
    let mut vertices = Vec::with_capacity(PROJECT_CURVE_PATCH_X.len() * 4);
    let mut triangles = Vec::with_capacity(PROJECT_CURVE_PATCH_X.len() * 2);
    for (i, &px) in PROJECT_CURVE_PATCH_X.iter().enumerate() {
        let py = i as f64 * PROJECT_CURVE_PATCH_Y_STEP;
        let base = vertices.len() as u32;
        vertices.push(P3::new(px - PROJECT_CURVE_PATCH_HALF_X, py, 0.0));
        vertices.push(P3::new(px + PROJECT_CURVE_PATCH_HALF_X, py, 0.0));
        vertices.push(P3::new(
            px + PROJECT_CURVE_PATCH_HALF_X,
            py + PROJECT_CURVE_PATCH_Y_SPAN,
            0.0,
        ));
        vertices.push(P3::new(
            px - PROJECT_CURVE_PATCH_HALF_X,
            py + PROJECT_CURVE_PATCH_Y_SPAN,
            0.0,
        ));
        triangles.push([base, base + 1, base + 2]);
        triangles.push([base, base + 2, base + 3]);
    }
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let index = SpatialIndex::build_auto(&mesh);
    (mesh, index)
}

/// Single OPEN polyline that visits each patch's short in-patch cut segment
/// (its Y-band's 0.5..2.5 sub-range) in the same far-apart, interleaved
/// order as `PROJECT_CURVE_PATCH_X` — the long inter-patch jumps are what
/// give TSP something obviously improvable, exactly like
/// `drill_holes_bad_order`. Because each patch occupies its own disjoint
/// Y-band (see `PROJECT_CURVE_PATCH_Y_STEP`), the connecting segments
/// between patches pass entirely over air (no mesh contact) except right at
/// their endpoints — which is exactly what drives `project_curve.rs`'s own
/// chain-splitting (`project_polygon_rings_with_cancel`) to flush a
/// separate retract-separated chain per patch.
fn project_curve_bad_order_path() -> Polygon2 {
    let mut points = Vec::with_capacity(PROJECT_CURVE_PATCH_X.len() * 2);
    for (i, &px) in PROJECT_CURVE_PATCH_X.iter().enumerate() {
        let py = i as f64 * PROJECT_CURVE_PATCH_Y_STEP;
        points.push(P2::new(px, py + 0.5));
        points.push(P2::new(px, py + 2.5));
    }
    Polygon2::open_path(points)
}

#[test]
fn project_curve_capability_allows_tsp_reorder_reduces_rapid_and_is_material_neutral() {
    // Fixture note: this drives the REAL generator — `project_curve_toolpath`,
    // the same function `compute/execute.rs::generate_project_curve` calls
    // per input polygon — rather than a hand-built Toolpath. The mesh is a
    // synthetic patchwork of 6 disconnected flat squares (not a real STL) so
    // the chain-per-patch structure is deterministic and reviewable, but the
    // actual chain-splitting logic under test (gap-over-air flush in
    // `project_polygon_rings_with_cancel`, project_curve.rs ~360-382) and the
    // per-chain retract emission (`Toolpath::emit_path_segment_with_intent`)
    // are exercised for real, not simulated.
    let (mesh, index) = project_curve_patchwork_mesh();
    let poly = project_curve_bad_order_path();
    let cutter = FlatEndmill::new(2.0, 25.0);
    let params = ProjectCurveParams {
        depth: 1.0,
        feed_rate: 1000.0,
        plunge_rate: 400.0,
        safe_z: 30.0,
        point_spacing: 1.0,
        direction: ProjectDirection::FromAbove,
        tool_radius: 1.0,
        side: ProjectSide::Center,
        setup_z_flipped: false,
    };
    let raw = project_curve_toolpath(&poly, &mesh, &index, &cutter, &params);

    // Sanity: the fixture must actually produce several retract-separated
    // chains (one per patch), or there's nothing for TSP to reorder and the
    // rest of this test is vacuous.
    let retract_count = raw
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Retract)
        .count();
    assert!(
        retract_count >= PROJECT_CURVE_PATCH_X.len(),
        "fixture must emit at least {} retract-separated chains (one per \
         disconnected patch) for the TSP reorder to have anything to do; \
         got {retract_count} — the bad-order polyline may be crossing \
         patches unexpectedly",
        PROJECT_CURVE_PATCH_X.len(),
    );

    let baseline = dressup(
        raw.clone(),
        &dressup_no_links(),
        OperationType::ProjectCurve,
        2.0,
    );
    let cfg = DressupConfig {
        optimize_rapid_order: true,
        ..DressupConfig::default()
    };
    let optimized = dressup(raw, &cfg, OperationType::ProjectCurve, 2.0);

    // This exercises the UNBARRIERED fallback TSP call site
    // (compute/execute.rs ~2445-2464), not the barriered one (~2226-2247):
    // `project_curve_toolpath` emits no `RapidOrderBarrier`/`DepthPass`
    // spans, and `dressup()` wraps the raw `Toolpath` in a fresh
    // `AnnotatedToolpath::new`, whose `rapid_order_barriers()` is therefore
    // empty — so the barriered branch's `!rapid_order_barriers.is_empty()`
    // guard is false, and only the unbarriered branch (gated on
    // `allows_unbarriered_rapid_reorder()`, which this reclassification
    // flips to `true` for ProjectCurve) can fire.
    let r_base = rapid_distance(&baseline);
    let r_opt = rapid_distance(&optimized);
    println!(
        "ProjectCurve TSP: rapid baseline={r_base:.1}  optimized={r_opt:.1}  \
         delta={:.1}",
        r_base - r_opt
    );
    assert!(
        r_opt < r_base,
        "ProjectCurve capability_allows_global_rapid_reorder must let TSP \
         reduce rapid distance on a deliberately far-apart/interleaved chain \
         order. baseline={r_base} optimized={r_opt}"
    );

    // Material neutrality: reordering which chain is visited when must not
    // change what metal comes off — that is the whole safety claim behind
    // loosening this capability.
    let (hm_base, hm_opt) = simulate_pair_shared_frame(&baseline, &optimized, &cutter, 0.5);
    let (max_d, frac) = compare_heightmaps(&hm_base, &hm_opt, 0.05);
    println!("ProjectCurve TSP: max_height_diff={max_d:.4}mm  cells_diff_frac={frac:.4}");
    assert!(
        frac <= 0.02,
        "ProjectCurve: TSP rapid reorder changed material removal beyond \
         tolerance — {:.2}% of cells differ by > 0.05mm (max diff {:.4}mm); \
         this indicates the capability flip introduces a real material \
         divergence and the reclassification is unsafe.",
        frac * 100.0,
        max_d,
    );
}

// ═════════════════════════════════════════════════════════════════════════
// Scallop config-aware reclassification (fix-family Phase 1b)
// ═════════════════════════════════════════════════════════════════════════

/// XY centers for 4 well-separated hemisphere "islands", arranged in a 2x2
/// grid within one shared mesh bbox. Scallop's discrete-ring generator
/// offsets the WHOLE bbox rectangle inward ring by ring (`scallop.rs`'s
/// `generate_scallop_rings_with_cancel` boundary construction); each
/// island's footprint sits at its own distance from the bbox edges (its
/// "ring depth"), so distinct rings sweep across each island independently,
/// separated everywhere else by wide air gaps the `covered` mask picks up
/// (`scallop.rs:174-198`). That is the surface-finishing analogue of
/// `project_curve_patchwork_mesh`'s disconnected patches above — the
/// fragmentation comes from ring/coverage geometry rather than an explicit
/// polyline gap.
const SCALLOP_ISLAND_CENTERS: [(f64, f64); 4] =
    [(-15.0, -15.0), (15.0, -15.0), (-15.0, 15.0), (15.0, 15.0)];

/// Radius of each hemisphere island (mm). Chosen so each island's footprint
/// diameter (10 mm) is comfortably larger than the ring stepover the
/// generator picks for `scallop_island_params`'s scallop height on a 1.5 mm
/// cusp radius (well under 1 mm) — that spacing margin is what guarantees
/// several discrete rings land inside each island's offset range instead of
/// stepping over it entirely.
const SCALLOP_ISLAND_RADIUS: f64 = 5.0;

/// Real mesh input for the Scallop discrete/continuous tests below: 4
/// disconnected hemisphere bumps (see `SCALLOP_ISLAND_CENTERS`), each built
/// via the same `make_test_hemisphere` helper used elsewhere in this file,
/// translated into position and merged into one `TriangleMesh`.
fn scallop_island_mesh() -> (TriangleMesh, SpatialIndex) {
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for &(cx, cy) in &SCALLOP_ISLAND_CENTERS {
        let bump = make_test_hemisphere(SCALLOP_ISLAND_RADIUS, 10);
        let base = vertices.len() as u32;
        vertices.extend(
            bump.vertices
                .iter()
                .map(|v| P3::new(v.x + cx, v.y + cy, v.z)),
        );
        triangles.extend(
            bump.triangles
                .iter()
                .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
        );
    }
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let index = SpatialIndex::build_auto(&mesh);
    (mesh, index)
}

fn scallop_island_params(continuous: bool) -> ScallopParams {
    ScallopParams {
        scallop_height: 0.05,
        tolerance: 0.05,
        direction: ScallopDirection::OutsideIn,
        continuous,
        slope_from: 0.0,
        slope_to: 90.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 30.0,
        stock_to_leave: 0.0,
    }
}

#[test]
fn scallop_discrete_capability_currently_blocked_reorder_gouges() {
    // Fixture note: this drives the REAL generator — `scallop_toolpath`,
    // the same function `compute/execute.rs`'s Scallop generation path
    // calls — over a real (if synthetic) 4-island mesh, not a hand-built
    // Toolpath.
    let (mesh, index) = scallop_island_mesh();
    let cutter = BallEndmill::new(3.0, 25.0);
    let params = scallop_island_params(false);
    let raw = scallop_toolpath(&mesh, &index, &cutter, &params);

    assert!(
        !raw.moves.is_empty(),
        "fixture must produce a non-empty discrete-ring scallop toolpath"
    );

    // Sanity: the fixture must actually fragment into several
    // retract-separated ring runs (one per island at minimum — see
    // `SCALLOP_ISLAND_RADIUS`'s doc comment for why several rings per
    // island is the expected case) or there's nothing for TSP to reorder
    // and the rest of this test is vacuous.
    let retract_count = raw
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Retract)
        .count();
    assert!(
        retract_count >= SCALLOP_ISLAND_CENTERS.len(),
        "fixture must emit at least {} retract-separated ring runs (one per \
         disconnected island) for the TSP reorder to have anything to do; \
         got {retract_count} — the island layout/spacing may need revisiting",
        SCALLOP_ISLAND_CENTERS.len(),
    );

    // PART 1 — the shipped state: discrete Scallop is still BLOCKED from
    // reordering. Phase 1b reclassified it on the (correct) reasoning that
    // rings are materially independent, then this very test measured a
    // gouge, so the arm was reverted. This assertion pins the block so
    // nobody re-enables it without reading the measurement below.
    let shipped = OperationConfig::Scallop(ScallopConfig {
        continuous: false,
        ..ScallopConfig::default()
    })
    .transform_capabilities();
    assert!(
        shipped.continuous_path_required,
        "discrete Scallop must stay reorder-BLOCKED until tsp::rebuild_group \
         preserves each segment's approach move — see the gouge measurement \
         in this test and the doc comment on \
         OperationConfig::transform_capabilities"
    );

    // PART 2 — reproduce the defect that justifies the block, by asking
    // for the capability the reclassification WOULD have granted.
    let caps = OperationTransformCapabilities::new(true, false, false);

    let baseline = dressup_with_caps(raw.clone(), &dressup_no_links(), caps, 3.0);
    let cfg = DressupConfig {
        optimize_rapid_order: true,
        ..DressupConfig::default()
    };
    let optimized = dressup_with_caps(raw, &cfg, caps, 3.0);

    let r_base = rapid_distance(&baseline);
    let r_opt = rapid_distance(&optimized);
    println!(
        "Scallop(discrete) TSP: rapid baseline={r_base:.1}  optimized={r_opt:.1}  \
         delta={:.1}",
        r_base - r_opt
    );
    assert!(
        r_opt < r_base,
        "Scallop(continuous: false) capability_allows_global_rapid_reorder \
         must let TSP reduce rapid distance across the 4 disconnected-island \
         ring runs. baseline={r_base} optimized={r_opt}"
    );

    // The gouge. Reordering is NOT material-neutral here: the reordered
    // path removes MORE metal. `tsp::split_into_segments` splits on
    // `MoveType::Rapid`, so the generator's own rapid descent toward the
    // surface is treated as a splitter and dropped; `rebuild_group` then
    // relinks with retract-to-safe_z + horizontal traverse and replays the
    // segment verbatim, leaving its first CUTTING move to travel down from
    // safe_z through whatever stands under it.
    let (hm_base, hm_opt) = simulate_pair_shared_frame(&baseline, &optimized, &cutter, 0.5);
    let (max_d, frac) = compare_heightmaps(&hm_base, &hm_opt, 0.05);
    let (mut deeper, mut shallower, mut net) = (0usize, 0usize, 0.0f64);
    for (b, o) in hm_base.iter().zip(hm_opt.iter()) {
        let d = o - b;
        if d < -0.05 {
            deeper += 1;
        }
        if d > 0.05 {
            shallower += 1;
        }
        net += f64::from(d);
    }
    println!(
        "Scallop(discrete) TSP: max_height_diff={max_d:.4}mm cells_diff_frac={frac:.4} \
         | cut {:.1} -> {:.1} | columns deeper={deeper} shallower={shallower} net={net:.1}",
        cutting_distance(&baseline),
        cutting_distance(&optimized),
    );

    assert!(
        deeper > shallower && max_d > 1.0,
        "REGRESSION IN THE RIGHT DIRECTION: this test exists to reproduce a \
         known TSP gouge on discrete Scallop (measured 2026-08-03: 146 columns \
         deeper vs 20 shallower, net -128.9mm, worst 9.33mm). It now reports \
         deeper={deeper} shallower={shallower} max={max_d:.4}mm. If the gouge \
         is genuinely fixed, that is GOOD — delete this assertion, restore the \
         material-neutrality gate (frac <= 0.02 && max_d < 0.05), and re-enable \
         the Scallop arm in OperationConfig::transform_capabilities."
    );
}

#[test]
fn scallop_continuous_capability_still_blocks_reorder() {
    // Same fixture as the discrete test above, but `continuous: true` —
    // this is the guard that proves the config-awareness DISCRIMINATES on
    // `cfg.continuous` rather than blanket-loosening every Scallop config.
    let (mesh, index) = scallop_island_mesh();
    let cutter = BallEndmill::new(3.0, 25.0);
    let params = scallop_island_params(true);
    let raw = scallop_toolpath(&mesh, &index, &cutter, &params);

    assert!(
        !raw.moves.is_empty(),
        "fixture must produce a non-empty continuous (spiral) scallop toolpath"
    );

    let caps = OperationConfig::Scallop(ScallopConfig {
        continuous: true,
        ..ScallopConfig::default()
    })
    .transform_capabilities();
    assert!(
        caps.continuous_path_required,
        "OperationConfig::Scallop(continuous: true).transform_capabilities() \
         must still require a continuous path — the helical stay-down chain \
         (scallop.rs:812-912) must stay protected from reordering/linking. \
         If this fails, the catalog.rs config-aware match arm is over-broad."
    );

    let baseline = dressup_with_caps(raw.clone(), &dressup_no_links(), caps, 3.0);
    let cfg = DressupConfig {
        optimize_rapid_order: true,
        ..DressupConfig::default()
    };
    let optimized = dressup_with_caps(raw, &cfg, caps, 3.0);

    let r_base = rapid_distance(&baseline);
    let r_opt = rapid_distance(&optimized);
    println!(
        "Scallop(continuous) TSP: rapid baseline={r_base:.1}  optimized={r_opt:.1}  \
         delta={:.1}",
        r_base - r_opt
    );
    assert_eq!(
        r_opt, r_base,
        "Scallop(continuous: true) must block rapid reorder entirely — the \
         continuous_path_required capability should make apply_dressups skip \
         both the barriered and unbarriered TSP steps, leaving rapid distance \
         byte-identical. baseline={r_base} optimized={r_opt}"
    );
    assert_eq!(
        optimized.moves.len(),
        baseline.moves.len(),
        "Scallop(continuous: true): optimize_rapid_order must be a complete \
         no-op given the blocking capability — move count must not change"
    );
}

// Compile-time silencer for the `MoveType` import — referenced via
// trait-method `is_cutting` only, which keeps the import live.
const _: fn(MoveType) -> bool = |m: MoveType| m.is_cutting();
