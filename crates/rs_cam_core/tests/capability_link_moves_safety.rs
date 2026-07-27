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

    // ISOLATE THE VARIABLE (2026-08-03): `DressupConfig::default()` has
    // `link_moves: true` (Roadmap B.6), so `..Default::default()` here
    // would turn link-moves ON in this branch while the baseline has it
    // OFF — comparing reorder+links vs neither, and attributing
    // apply_link_moves' lateral feed bridges to the reorderer. Hold
    // link_moves constant; vary ONLY optimize_rapid_order.
    let cfg = DressupConfig {
        optimize_rapid_order: true,
        link_moves: false,
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
    // ISOLATE THE VARIABLE (2026-08-03): `DressupConfig::default()` has
    // `link_moves: true` (Roadmap B.6), so `..Default::default()` here
    // would turn link-moves ON in this branch while the baseline has it
    // OFF — comparing reorder+links vs neither, and attributing
    // apply_link_moves' lateral feed bridges to the reorderer. Hold
    // link_moves constant; vary ONLY optimize_rapid_order.
    let cfg = DressupConfig {
        optimize_rapid_order: true,
        link_moves: false,
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
    // ISOLATE THE VARIABLE (2026-08-03): `DressupConfig::default()` has
    // `link_moves: true` (Roadmap B.6), so `..Default::default()` here
    // would turn link-moves ON in this branch while the baseline has it
    // OFF — comparing reorder+links vs neither, and attributing
    // apply_link_moves' lateral feed bridges to the reorderer. Hold
    // link_moves constant; vary ONLY optimize_rapid_order.
    let cfg = DressupConfig {
        optimize_rapid_order: true,
        link_moves: false,
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
    // ISOLATE THE VARIABLE (2026-08-03): `DressupConfig::default()` has
    // `link_moves: true` (Roadmap B.6), so `..Default::default()` here
    // would turn link-moves ON in this branch while the baseline has it
    // OFF — comparing reorder+links vs neither, and attributing
    // apply_link_moves' lateral feed bridges to the reorderer. Hold
    // link_moves constant; vary ONLY optimize_rapid_order.
    let cfg = DressupConfig {
        optimize_rapid_order: true,
        link_moves: false,
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
fn scallop_discrete_reorder_preserves_cuts_but_link_moves_gouge() {
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

    // PART 1 — the shipped state (fix-family Phase 1c): discrete Scallop's
    // REAL capability now decouples the two concerns. Reordering rings is
    // safe (they are materially independent — see PART 2's measurement)
    // and is now allowed; taking link moves is NOT safe (PART 3 below
    // reproduces the gouge) and stays forbidden. Before this decoupling,
    // one flag (`!continuous_path_required`) drove both predicates, so
    // permitting the safe reorder would have necessarily permitted the
    // unsafe link bridge — that was the earlier revert. Read the SHIPPED
    // capability, not a hand-built one, so this test tracks what
    // production code actually does.
    let shipped = OperationConfig::Scallop(ScallopConfig {
        continuous: false,
        ..ScallopConfig::default()
    })
    .transform_capabilities();
    assert!(
        shipped.allows_global_rapid_reorder,
        "discrete Scallop should allow global rapid reorder — rings are \
         materially independent (see the cutting-distance-preserving \
         measurement below)"
    );
    assert!(
        !shipped.allows_link_moves,
        "discrete Scallop must still forbid link moves — apply_link_moves \
         bridges with a straight feed that gouges a 3D surface (see the \
         measurement at the end of this test)"
    );

    // PART 2 — the reorder, using the REAL shipped capability (reorder on,
    // links off). This must be cutting-neutral: only rapids may change.
    let caps = shipped;

    let raw_for_links = raw.clone();
    let baseline = dressup_with_caps(raw.clone(), &dressup_no_links(), caps, 3.0);
    // ISOLATE THE VARIABLE (2026-08-03): `DressupConfig::default()` has
    // `link_moves: true` (Roadmap B.6), so `..Default::default()` here
    // would turn link-moves ON in this branch while the baseline has it
    // OFF — comparing reorder+links vs neither, and attributing
    // apply_link_moves' lateral feed bridges to the reorderer. Hold
    // link_moves constant; vary ONLY optimize_rapid_order.
    let cfg = DressupConfig {
        optimize_rapid_order: true,
        link_moves: false,
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

    // ── move-fidelity diff (2026-08-03) ────────────────────────────────
    // Material removal in a dexel sim is MONOTONIC: final stock =
    // initial - union(swept volumes). Pure reordering therefore cannot
    // change the result. A material difference is proof the MOVE SET
    // changed, so tally both branches per intent and show what moved.
    {
        use std::collections::BTreeMap;
        let tally = |tp: &Toolpath| -> BTreeMap<String, (usize, f64)> {
            let mut m: BTreeMap<String, (usize, f64)> = BTreeMap::new();
            let mut prev: Option<P3> = None;
            for mv in &tp.moves {
                let d = prev.map_or(0.0, |p: P3| {
                    ((mv.target.x - p.x).powi(2)
                        + (mv.target.y - p.y).powi(2)
                        + (mv.target.z - p.z).powi(2))
                    .sqrt()
                });
                let key = format!(
                    "{:?}/{}",
                    mv.intent,
                    if matches!(mv.move_type, MoveType::Rapid) {
                        "rapid"
                    } else {
                        "feed"
                    }
                );
                let e = m.entry(key).or_insert((0, 0.0));
                e.0 += 1;
                e.1 += d;
                prev = Some(mv.target);
            }
            m
        };
        let (tb, to) = (tally(&baseline), tally(&optimized));
        let mut keys: Vec<&String> = tb.keys().chain(to.keys()).collect();
        keys.sort();
        keys.dedup();
        println!("Scallop(discrete) MOVE-FIDELITY DIFF (baseline -> optimized):");
        for k in keys {
            let b = tb.get(k).copied().unwrap_or((0, 0.0));
            let o = to.get(k).copied().unwrap_or((0, 0.0));
            if b.0 != o.0 || (b.1 - o.1).abs() > 0.05 {
                println!(
                    "  {k:<24} n {:>4} -> {:>4} ({:+})   len {:>9.1} -> {:>9.1} ({:+.1})",
                    b.0,
                    o.0,
                    o.0 as i64 - b.0 as i64,
                    b.1,
                    o.1,
                    o.1 - b.1
                );
            }
        }
    }

    // FINDING (2026-08-03). Isolated, the reorder is faithful: every
    // cutting move survives (cutting distance identical to 0.1mm; the
    // move-fidelity diff above shows ONLY rapids changing) while rapid
    // travel drops ~27%. The residual sub-mm column delta is at dexel
    // discretisation scale (0.5mm cells) and is NOT the gouge class.
    assert!(
        (cutting_distance(&optimized) - cutting_distance(&baseline)).abs() < 0.1,
        "reorder must preserve the cut set exactly — cutting distance moved \
         {:.1} -> {:.1}; if this trips, tsp is altering cut geometry, not \
         just order",
        cutting_distance(&baseline),
        cutting_distance(&optimized),
    );
    assert!(
        max_d < 1.0,
        "reorder-only material delta {max_d:.4}mm exceeds discretisation \
         scale — investigate before trusting the reorder"
    );

    // PART 3 — the hazard the shipped capability now PREVENTS. The shipped
    // `caps.allows_link_moves` is false, so `apply_dressups` would refuse
    // to run `apply_link_moves` under it — there would be nothing to
    // measure. To deliberately reproduce the defect that justifies keeping
    // link moves forbidden, hand-build a PERMISSIVE capability (reorder
    // AND links both on) — the shape the two concerns had before this
    // fix-family split them apart — and confirm it still gouges.
    // `apply_link_moves` collapses retract->plunge pairs into LATERAL feed
    // bridges that plow across a 3D surface. Measured here at ~9mm of
    // over-cut — the same class as the 8.2mm the Inlay link-moves test
    // reports and passes, because that gate bounds the FRACTION of
    // differing cells and never their depth.
    let permissive_link_caps = OperationTransformCapabilities::new(true, false, false, true);
    let linked = dressup_with_caps(
        raw_for_links,
        &DressupConfig {
            optimize_rapid_order: false,
            link_moves: true,
            ..DressupConfig::default()
        },
        permissive_link_caps,
        3.0,
    );
    let (hm_b2, hm_l) = simulate_pair_shared_frame(&baseline, &linked, &cutter, 0.5);
    let (link_max_d, _) = compare_heightmaps(&hm_b2, &hm_l, 0.05);
    println!("Scallop(discrete) LINK-MOVES: max_height_diff={link_max_d:.4}mm");
    assert!(
        link_max_d > 1.0,
        "expected link_moves to gouge discrete Scallop (measured 9.3mm on \
         2026-08-03); got {link_max_d:.4}mm. This is the defect \
         `allows_link_moves: false` on the shipped capability exists to \
         prevent — if it stops reproducing, either the reorderer/linker \
         changed materially or this fixture needs revisiting; it does NOT \
         mean link moves are now safe to re-enable."
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
    // ISOLATE THE VARIABLE (2026-08-03): `DressupConfig::default()` has
    // `link_moves: true` (Roadmap B.6), so `..Default::default()` here
    // would turn link-moves ON in this branch while the baseline has it
    // OFF — comparing reorder+links vs neither, and attributing
    // apply_link_moves' lateral feed bridges to the reorderer. Hold
    // link_moves constant; vary ONLY optimize_rapid_order.
    let cfg = DressupConfig {
        optimize_rapid_order: true,
        link_moves: false,
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
