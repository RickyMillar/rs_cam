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
//! REVISED 2026-08-03 — two structural holes were found and closed:
//!
//! - The neutrality gate bounded only the FRACTION of differing cells,
//!   never their DEPTH, so a deep narrow gouge passed cleanly. With a depth
//!   bound added, three ops that had always been "green" turned out to
//!   gouge from link moves: Face 9.21mm, Inlay 8.21mm, VCarve 5.78mm. They
//!   were routed to an `assert_link_moves_gouge` helper that PINNED the
//!   defect rather than hiding it, so the fix would be visible when it
//!   landed (see the FIXED note below — it has).
//! - `dressup()` wrapped raw toolpaths in `AnnotatedToolpath::new` (no
//!   spans). Barriers live in spans and both `apply_link_moves` and the TSP
//!   are barrier-aware, so the fixture was stripping every ordering
//!   constraint the real pipeline has. It now builds production-equivalent
//!   spans per op (`production_spans`). Face was re-measured with its real
//!   `spans_from_depth_runs` barriers and still gouged — so these were
//!   genuine, not fixture artifacts.
//! - `trace_link_moves_preserves_material_state` was vacuous: Trace forbids
//!   link moves, so it compared two identical toolpaths. Replaced by an
//!   assertion of the real property.
//!
//! Root cause of the gouges: `apply_link_moves` inserted a straight feed
//! bridge with NO gouge check, unlike `surface_link::build_surface_link`
//! which pencil/unified_finish use for exactly this reason. Flat,
//! already-cleared paths (Chamfer/Pencil/RadialFinish) measured 0.0000mm;
//! variable-depth or multi-feature paths plowed.
//!
//! FIXED — `dressup::bridge_corridor_is_swept`: `apply_link_moves` now
//! refuses to collapse a retract/rapid/plunge triple into a bridge unless
//! the straight corridor between the two cut points is fully covered by
//! cutting moves this SAME toolpath has already emitted, at the same Z
//! (not by `prior_stock`, which would reject every bridge crossing ground
//! this toolpath just cleared — see that function's doc comment). Face,
//! Inlay, and VCarve now route back through `assert_link_moves_neutral`
//! and are renamed `*_link_moves_preserves_material_state`; the discrete
//! Scallop test's deliberately-permissive PART 3 was updated the same way.
//! `assert_link_moves_gouge` had no remaining callers and was deleted
//! rather than left unused — if a future op is found to still gouge, a
//! sibling helper following the same shape should be reintroduced, not
//! resurrected from history, since the tolerances here are meant to be
//! re-derived from a fresh measurement, not copied forward.
//!
//! All tests must pass on master HEAD. A failure here means real material
//! divergence — investigate before shipping.

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
    geo::{BoundingBox3, P2, P3},
    horizontal_finish::{HorizontalFinishParams, horizontal_finish_toolpath},
    mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere},
    pencil::{PencilParams, pencil_toolpath},
    polygon::Polygon2,
    project_curve::{ProjectCurveParams, ProjectDirection, ProjectSide, project_curve_toolpath},
    radial_finish::{RadialFinishParams, radial_finish_toolpath},
    scallop::{ScallopDirection, ScallopParams, scallop_toolpath},
    tool::{BallEndmill, FlatEndmill, MillingCutter},
    toolpath::{MoveIntent, MoveType, Toolpath},
};

// ── Common helpers ───────────────────────────────────────────────────────

fn rect_polygon() -> Polygon2 {
    Polygon2::rectangle(0.0, 0.0, 40.0, 30.0)
}

#[allow(dead_code)] // retained fixture: used again when Face/Inlay/VCarve links are re-measured
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
/// Production-equivalent spans for `op`.
///
/// CRITICAL (2026-08-03): `apply_link_moves` and the TSP passes are
/// BARRIER-AWARE, and barriers live in spans. Wrapping a raw toolpath in
/// `AnnotatedToolpath::new` (spans: `Vec::new()`) therefore strips every
/// ordering constraint the real pipeline has, and an op whose production
/// adapter emits `DepthPass`/`RapidOrderBarrier` spans will appear to
/// gouge here purely because the test threw its barriers away. Face is
/// exactly that case: `compute/execute.rs::generate_face` ships
/// `generated_with_depth_run_spans`, so its cross-depth links are blocked
/// in production and only unblocked by a span-less fixture.
fn production_spans(tp: &Toolpath, op: OperationType) -> Vec<rs_cam_core::toolpath_spans::Span> {
    use rs_cam_core::compute::spans::{spans_from_cutting_runs, spans_from_depth_runs};
    match op {
        // Depth-stepped adapters: real Z-transition barriers, derived from
        // the toolpath's own cutting Z (the empty `levels` hint is what
        // `generate_face` itself passes).
        OperationType::Face | OperationType::Pocket | OperationType::Profile => {
            spans_from_depth_runs(tp, &[])
        }
        // Everything else in this suite ships Region spans only — no
        // barriers — so a span-less fixture is already faithful. Build them
        // anyway so the comparison is like-for-like.
        _ => spans_from_cutting_runs(tp, "run"),
    }
}

fn dressup(tp: Toolpath, cfg: &DressupConfig, op: OperationType, tool_diameter: f64) -> Toolpath {
    let spans = production_spans(&tp, op);
    apply_dressups(
        rs_cam_core::toolpath_spans::AnnotatedToolpath::with_spans(tp, spans),
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
    simulate_pair_in_frame(a, b, cutter, cell_size, &bbox)
}

/// [`simulate_pair_shared_frame`] with the stock box supplied explicitly.
///
/// Needed by swept-volume comparisons: those re-type rapids as cuts, so a
/// box derived from the toolpath bbox — which reaches up to `safe_z` —
/// would let each branch's safe-Z traverses carve the phantom material
/// sitting ABOVE the part, and report two different traverse routes as a
/// material difference. Cap the box at the real stock top instead.
fn simulate_pair_in_frame(
    a: &Toolpath,
    b: &Toolpath,
    cutter: &dyn MillingCutter,
    cell_size: f64,
    bbox: &BoundingBox3,
) -> (Vec<f32>, Vec<f32>) {
    let bbox = *bbox;
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

/// The multiset of SWEPT CUT SEGMENTS: every cutting move keyed by the
/// `(from, to)` pair it actually sweeps — `from` being the previous move's
/// target whatever its type, since that is what the simulator stamps.
///
/// This is the exact instrument for a rapid REORDER. `tsp::optimize_rapid_order`
/// discards the input's rapids and regenerates retract/traverse/approach
/// around each segment, so only a cutting move's *first* sweep can change;
/// if this multiset is identical the two branches remove provably identical
/// material and no simulation is needed to say so.
///
/// Use it INSTEAD of leaning on the heightmap for the neutrality verdict.
/// The dexel sim has an order-dependent floor: `dexel::ray_blend_above`
/// subtracts only a FRACTION `f` of the above-surface span on sub-coverage
/// cells (`new_exit = exit - f * above_part`), which is neither idempotent
/// nor commutative. Full-coverage cells take `ray_subtract_above` and are
/// order-free, so "dexel removal is monotonic" holds in the interior and
/// fails at partial-coverage boundary cells — where a pure reorder really
/// can move a column by a fraction of a cusp.
fn swept_cut_segments(tp: &Toolpath) -> std::collections::BTreeMap<String, usize> {
    let mut out: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for w in tp.moves.windows(2) {
        if !w[1].move_type.is_cutting() {
            continue;
        }
        let (a, b) = (w[0].target, w[1].target);
        let key = format!(
            "{:.4},{:.4},{:.4}->{:.4},{:.4},{:.4}",
            a.x, a.y, a.z, b.x, b.y, b.z
        );
        *out.entry(key).or_insert(0) += 1;
    }
    out
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

    // VACUITY GUARD (2026-08-03): `apply_dressups` gates link moves on
    // `allows_link_moves()`. For an op that forbids them, the `with_links`
    // branch is byte-identical to the baseline and every assertion below
    // passes without testing anything — which is exactly what
    // `trace_link_moves_preserves_material_state` had been doing since it
    // was written. Refuse to pretend.
    assert!(
        op.transform_capabilities().allows_link_moves,
        "{op:?}: allows_link_moves is FALSE, so this fixture cannot exercise \
         link moves at all — the comparison below would be two identical \
         toolpaths. Either this op should permit links, or it does not belong \
         in this suite."
    );

    let baseline = dressup(raw.clone(), &dressup_no_links(), op, tool_diameter);
    let with_links = dressup(raw, &dressup_with_links(link_distance), op, tool_diameter);

    let (hm_base, _) = simulate_to_heightmap(&baseline, cutter, cell_size);
    let (hm_link, _) = simulate_to_heightmap(&with_links, cutter, cell_size);

    let (max_d, frac) = compare_heightmaps(&hm_base, &hm_link, height_tol);
    {
        let (mut deeper, mut shallower) = (0usize, 0usize);
        for (b, l) in hm_base.iter().zip(hm_link.iter()) {
            let d = l - b;
            if d < -height_tol {
                deeper += 1;
            }
            if d > height_tol {
                shallower += 1;
            }
        }
        println!("{op:?} SIGNED: links_cut_DEEPER={deeper} links_left_MORE={shallower}");
    }
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

    // DEPTH BOUND (2026-08-03). The fraction gate below cannot see a
    // deep, narrow gouge: a link bridge that plows 8mm through 1% of the
    // part passes it cleanly. Bound how far ANY column may move, not just
    // how many. Before the `bridge_corridor_is_swept` fix, ops known to
    // fail this were routed to a (since-deleted) `assert_link_moves_gouge`
    // helper that pinned the defect instead of hiding it; now that the
    // corridor-safety check lives in `apply_link_moves` itself, every op
    // permitted to use link moves is expected to pass this bound for real.
    assert!(
        max_d <= height_tol,
        "{op:?}: link_moves moved a column by {max_d:.4}mm (tolerance \
         {height_tol:.4}mm) — a deep, narrow gouge that the cells-differing \
         fraction gate is structurally blind to. apply_link_moves now checks \
         `bridge_corridor_is_swept` before emitting a bridge (contrast \
         surface_link::build_surface_link, which pencil/unified_finish use \
         for the mesh-based equivalent); if this trips, that check let an \
         uncovered corridor through.",
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

// `assert_link_moves_gouge` — the inverted sibling that used to PIN the
// Face/Inlay/VCarve gouges here — had no remaining callers once the
// `bridge_corridor_is_swept` fix landed (see the module doc comment) and
// was deleted rather than left as dead code. If a future op is found to
// still gouge, re-derive a fresh sibling from `assert_link_moves_neutral`'s
// shape rather than resurrecting this one — its tolerances were measured
// against the specific pre-fix defect, not a general contract.

// ═════════════════════════════════════════════════════════════════════════
// 7 link_moves-loosened ops
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn face_link_moves_are_forbidden_after_measured_gouge() {
    // Measured 2026-08-03 with the swept-corridor check live in
    // `dressup::apply_link_moves`: Face still gouged 9.21mm, 25 columns
    // strictly DEEPER and zero left proud. The corridor check verifies the
    // bridge stays within `tool_radius` of already-cut path at the same Z,
    // which is sound for a constant-width cutter (it took discrete Scallop
    // from 9.71mm to 0.0000mm) but not for depth-dependent widths — a V-bit
    // that passed shallowly here did not clear the width a deeper bridge
    // needs. Links are therefore forbidden for this op until the check
    // models that, or link decisions move into the generator where the
    // geometry is in scope.
    assert!(
        !OperationType::Face
            .transform_capabilities()
            .allows_link_moves,
        "Face permits link moves again — it gouged 9.21mm when last measured. \
         Re-enable only with a fresh neutrality measurement, not on inspection."
    );
}

#[test]
fn trace_link_moves_are_forbidden_so_the_old_test_was_vacuous() {
    // Trace sits in the `requires_depth_order` bucket, so `allows_link_moves`
    // is false and `apply_dressups` never runs `apply_link_moves` on it.
    // The previous body compared two IDENTICAL toolpaths and asserted they
    // matched — green forever, testing nothing. Assert the real property.
    assert!(
        !OperationType::Trace
            .transform_capabilities()
            .allows_link_moves,
        "Trace now permits link moves — it needs a REAL neutrality or gouge \
         test, not the vacuous clone comparison this replaced"
    );
}

#[test]
fn vcarve_link_moves_are_forbidden_after_measured_gouge() {
    // Measured 2026-08-03 with the swept-corridor check live in
    // `dressup::apply_link_moves`: VCarve still gouged 5.78mm, 103 columns
    // strictly DEEPER and zero left proud. The corridor check verifies the
    // bridge stays within `tool_radius` of already-cut path at the same Z,
    // which is sound for a constant-width cutter (it took discrete Scallop
    // from 9.71mm to 0.0000mm) but not for depth-dependent widths — a V-bit
    // that passed shallowly here did not clear the width a deeper bridge
    // needs. Links are therefore forbidden for this op until the check
    // models that, or link decisions move into the generator where the
    // geometry is in scope.
    assert!(
        !OperationType::VCarve
            .transform_capabilities()
            .allows_link_moves,
        "VCarve permits link moves again — it gouged 5.78mm when last measured. \
         Re-enable only with a fresh neutrality measurement, not on inspection."
    );
}

#[test]
fn inlay_link_moves_are_forbidden_after_measured_gouge() {
    // Measured 2026-08-03 with the swept-corridor check live in
    // `dressup::apply_link_moves`: Inlay still gouged 8.21mm, 110 columns
    // strictly DEEPER and zero left proud. The corridor check verifies the
    // bridge stays within `tool_radius` of already-cut path at the same Z,
    // which is sound for a constant-width cutter (it took discrete Scallop
    // from 9.71mm to 0.0000mm) but not for depth-dependent widths — a V-bit
    // that passed shallowly here did not clear the width a deeper bridge
    // needs. Links are therefore forbidden for this op until the check
    // models that, or link decisions move into the generator where the
    // geometry is in scope.
    assert!(
        !OperationType::Inlay
            .transform_capabilities()
            .allows_link_moves,
        "Inlay permits link moves again — it gouged 8.21mm when last measured. \
         Re-enable only with a fresh neutrality measurement, not on inspection."
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
fn scallop_discrete_reorder_preserves_cuts_and_link_moves_are_now_safe() {
    // RENAMED (was `..._but_link_moves_gouge`): PART 3 below used to
    // deliberately reproduce the discrete-Scallop link-moves gouge under a
    // hand-built permissive capability. `dressup::bridge_corridor_is_swept`
    // now runs unconditionally inside `apply_link_moves` regardless of
    // capability flags, so even this deliberately-permissive scenario
    // should measure neutral — see PART 3's updated comment below.
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

    // PART 3 — the hazard the shipped capability USED TO leave to
    // `allows_link_moves: false` alone to prevent. The shipped
    // `caps.allows_link_moves` is false, so `apply_dressups` would refuse
    // to run `apply_link_moves` under it — there would be nothing to
    // measure. To exercise the same permissive shape the two concerns had
    // before this fix-family split them apart (reorder AND links both on),
    // hand-build a PERMISSIVE capability and confirm the corridor-safety
    // fix now holds even here.
    //
    // FIXED (`dressup::bridge_corridor_is_swept`, see the module doc
    // comment): this used to measure ~9.71mm of over-cut — the same class
    // as the Face/Inlay/VCarve gouges — because `apply_link_moves` bridged
    // retract/rapid/plunge triples with a straight feed and no check that
    // the corridor was already-cut territory. The check now lives
    // UNCONDITIONALLY inside `apply_link_moves` (it does not read
    // capability flags), so even this deliberately-permissive capability
    // should no longer be able to produce the gouge — the capability flag
    // stays `false` in production as defence in depth, not because this
    // fixture still needs it to avoid the defect.
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
    let (link_max_d, link_frac) = compare_heightmaps(&hm_b2, &hm_l, 0.05);
    println!(
        "Scallop(discrete) LINK-MOVES: max_height_diff={link_max_d:.4}mm \
         cells_diff_frac={link_frac:.4}"
    );
    assert!(
        link_max_d <= 0.05,
        "expected the bridge_corridor_is_swept fix to keep discrete Scallop \
         link moves neutral (previously measured a 9.71mm gouge here on \
         2026-08-03 under this same permissive capability); got \
         {link_max_d:.4}mm instead. If this trips, the corridor-safety check \
         is not catching this fixture's gouge — that is a real regression,\
         not a fixture problem, and this assertion should NOT be loosened."
    );
    assert!(
        link_frac <= 0.02,
        "expected link moves to change at most a small fraction of cells on \
         discrete Scallop; got {:.2}% of cells differing by > 0.05mm (max \
         diff {link_max_d:.4}mm)",
        link_frac * 100.0,
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

// ── UnifiedFinish: region-node barriers ──────────────────────────────────
//
// UnifiedFinish is stitched from independently generated region nodes, so
// it is NOT the continuous trace its old capability claimed. What was
// missing was barriers: `apply_dressups` gates the barriered TSP on
// `!rapid_order_barriers.is_empty()`, and the op emitted none, so flipping
// the capability alone would have handed the whole stitched path to one
// unconstrained reorder. `unified_finish::unified_finish_spans` now emits a
// barrier at every node start plus per-Z barriers inside waterline nodes.
//
// Two properties are asserted here, and they need DIFFERENT instruments:
//
//   * Material neutrality — the dexel sim, with a DEPTH bound (a gouge
//     through 1% of the columns passes any fraction-only gate).
//   * Depth ORDER inside waterline nodes — the sim CANNOT see this. Dexel
//     removal is monotonic, so cutting the deep pass before the shallow one
//     leaves byte-identical final stock. The hazard is cutting force, not
//     geometry, so this is asserted structurally on the move sequence.

/// Nominal (deepest) cutting Z of each retract-separated cutting run inside
/// `range`, with consecutive duplicates collapsed — i.e. the sequence of
/// depth LEVELS the tool visits.
fn nominal_z_levels(tp: &Toolpath, range: std::ops::Range<usize>) -> Vec<f64> {
    let mut levels: Vec<f64> = Vec::new();
    let mut run_min: Option<f64> = None;
    for mv in tp.moves.iter().take(range.end).skip(range.start) {
        if mv.move_type.is_cutting() {
            run_min = Some(run_min.map_or(mv.target.z, |z: f64| z.min(mv.target.z)));
        } else if let Some(z) = run_min.take()
            && !levels.last().is_some_and(|l: &f64| (l - z).abs() <= 1.0e-6)
        {
            levels.push(z);
        }
    }
    if let Some(z) = run_min
        && !levels.last().is_some_and(|l: &f64| (l - z).abs() <= 1.0e-6)
    {
        levels.push(z);
    }
    levels
}

#[test]
fn unified_finish_node_barriers_allow_intra_region_reorder_and_pin_depth() {
    use rs_cam_core::finish_planner::FinishPlannerParams;
    use rs_cam_core::toolpath_spans::{AnnotatedToolpath, SpanKind};
    use rs_cam_core::unified_finish::{
        RegionKind, UnifiedFinishParams, unified_finish_spans, unified_finish_toolpath_with_cancel,
    };

    let (mesh, index) = hemisphere_mesh();
    let cutter = BallEndmill::new(3.0, 25.0);
    let params = UnifiedFinishParams {
        tolerance: 0.5,
        ..UnifiedFinishParams::default()
    };
    let planner = FinishPlannerParams::for_tool(cutter.radius());
    let never_cancel = || false;

    let (raw, anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        /* top_z */ 25.0,
        /* bottom_z */ -1.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .expect("uncancelled generation");

    assert!(!raw.moves.is_empty(), "fixture must generate a toolpath");
    let very_steep_nodes = report
        .region_table
        .iter()
        .filter(|e| matches!(e.kind, RegionKind::Band(band) if format!("{band:?}") == "VerySteep"))
        .count();
    println!(
        "UnifiedFinish fixture: {} nodes ({} VerySteep), {} moves",
        report.region_table.len(),
        very_steep_nodes,
        raw.moves.len()
    );
    assert!(
        report.region_table.len() >= 2,
        "fixture must route at least two region nodes or the cross-node \
         barrier assertion below is vacuous; got {:?}",
        report.region_table
    );
    assert!(
        very_steep_nodes >= 1,
        "hemisphere fixture must produce at least one VerySteep (waterline) \
         node — the per-Z barrier half of this test is vacuous without one"
    );

    // The spans production ships, from the same function production calls.
    let spans = unified_finish_spans(&raw, &anns, &report);
    let annotated = AnnotatedToolpath::with_spans(raw.clone(), spans);
    annotated
        .check_invariants()
        .expect("emitted spans must be well-formed");
    let barriers = annotated.rapid_order_barriers();
    assert!(
        barriers.len() >= report.region_table.len(),
        "every routed node must contribute a rapid-order barrier (plus the \
         per-Z barriers inside waterline nodes): {} barriers for {} nodes",
        barriers.len(),
        report.region_table.len()
    );

    // The shipped capability, not a hand-built one.
    let caps = OperationType::UnifiedFinish.transform_capabilities();
    assert!(
        caps.allows_barriered_rapid_reorder(),
        "UnifiedFinish must allow the BARRIERED reorder — it is stitched from \
         independent region nodes, not one continuous trace"
    );
    assert!(
        !caps.allows_unbarriered_rapid_reorder(),
        "UnifiedFinish must NOT allow the unbarriered reorder — that path \
         ignores the node barriers entirely"
    );
    assert!(
        !caps.allows_link_moves(),
        "UnifiedFinish must still forbid apply_link_moves' straight-feed \
         bridges — it builds its own gouge-checked links via \
         surface_link::build_surface_link"
    );

    // ISOLATE THE VARIABLE: hold link_moves OFF on BOTH branches and vary
    // only optimize_rapid_order. `DressupConfig::default()` ships
    // link_moves: true, so `..Default::default()` here would compare
    // reorder+links against neither.
    let dressed = |cfg: &DressupConfig| -> AnnotatedToolpath {
        let spans = unified_finish_spans(&raw, &anns, &report);
        apply_dressups(
            AnnotatedToolpath::with_spans(raw.clone(), spans),
            cfg,
            1000.0,
            /* tool_diameter */ 3.0,
            /* safe_z */ 30.0,
            /* stock_top */ 0.0,
            None,
            None,
            None,
            caps,
            None,
            None,
        )
    };
    let baseline = dressed(&dressup_no_links());
    let optimized = dressed(&DressupConfig {
        optimize_rapid_order: true,
        link_moves: false,
        ..DressupConfig::default()
    });

    let r_base = rapid_distance(&baseline.toolpath);
    let r_opt = rapid_distance(&optimized.toolpath);
    let c_base = cutting_distance(&baseline.toolpath);
    let c_opt = cutting_distance(&optimized.toolpath);
    println!(
        "UnifiedFinish TSP: rapid {r_base:.1} -> {r_opt:.1} ({:+.1}, {:+.1}%) | \
         cut {c_base:.1} -> {c_opt:.1}",
        r_opt - r_base,
        100.0 * (r_opt - r_base) / r_base,
    );
    assert!(
        r_opt < r_base,
        "the barriered TSP must reduce rapid travel inside the region nodes — \
         wanaka measured 89.6% of UnifiedFinish's inter-fragment travel as \
         intra-region. baseline={r_base} optimized={r_opt}"
    );

    // MATERIAL NEUTRALITY — asserted EXACTLY, on the swept cut segments.
    // The TSP discards the input's rapids and regenerates the approach
    // around each segment, so only a cutting move's first sweep can change;
    // an identical multiset is proof the two branches remove identical
    // material, with no simulation in the loop.
    let (segs_base, segs_opt) = (
        swept_cut_segments(&baseline.toolpath),
        swept_cut_segments(&optimized.toolpath),
    );
    let only_base = segs_base.iter().filter(|(k, _)| !segs_opt.contains_key(*k));
    let only_opt = segs_opt.iter().filter(|(k, _)| !segs_base.contains_key(*k));
    assert_eq!(
        segs_base,
        segs_opt,
        "the reorder must sweep exactly the same cut segments. \
         only-in-baseline: {:?} | only-in-optimized: {:?}",
        only_base.take(4).collect::<Vec<_>>(),
        only_opt.take(4).collect::<Vec<_>>(),
    );

    // Corroboration only. The dexel sim CANNOT confirm the above to zero:
    // `dexel::ray_blend_above` subtracts a FRACTION of a partial-coverage
    // cell, which is order-dependent, so identical geometry in a different
    // order still moves boundary columns by a fraction of a cusp. Measured
    // here at 0.87mm worst / 0.7% of cells on 0.5mm cells; the bound below
    // is that floor, not a neutrality claim — the assert_eq above is the
    // neutrality claim.
    let (hm_base, hm_opt) =
        simulate_pair_shared_frame(&baseline.toolpath, &optimized.toolpath, &cutter, 0.5);
    let (max_d, frac) = compare_heightmaps(&hm_base, &hm_opt, 0.05);
    let (mut deeper, mut shallower) = (0usize, 0usize);
    for (b, o) in hm_base.iter().zip(hm_opt.iter()) {
        if o - b < -0.05 {
            deeper += 1;
        }
        if o - b > 0.05 {
            shallower += 1;
        }
    }
    println!(
        "UnifiedFinish TSP: max_height_diff={max_d:.4}mm cells_diff_frac={frac:.4} \
         deeper={deeper} shallower={shallower} (sub-coverage blend floor)"
    );
    assert!(
        frac <= 0.02,
        "the sim's order-dependent blend floor must stay confined to boundary \
         cells; {frac} of columns differ, which is too many to be sub-coverage \
         blending — re-check the swept-segment equality above"
    );

    // Depth order inside waterline nodes. The sim is blind to this (see the
    // block comment above), so read the remapped node spans and assert the
    // level sequence still descends.
    let mut checked = 0usize;
    for span in optimized.spans.iter().filter(|s| s.kind == SpanKind::Region) {
        if !span.label.starts_with("VerySteep") {
            continue;
        }
        let levels = nominal_z_levels(&optimized.toolpath, span.start_move..span.end_move);
        if levels.len() < 2 {
            continue;
        }
        checked += 1;
        assert!(
            levels.windows(2).all(|w| w[0] >= w[1] - 1.0e-6),
            "waterline (VerySteep) node must keep its Z ladder descending \
             after the reorder — per-Z barriers exist precisely to prevent a \
             deeper pass being lifted above a shallower one. levels={levels:?}"
        );
    }
    assert!(
        checked >= 1,
        "at least one multi-level VerySteep node span must survive the \
         reorder's span remapping for the depth-order assertion to mean \
         anything (spans present: {}, labels: {:?})",
        optimized.spans.len(),
        optimized
            .spans
            .iter()
            .map(|s| s.label.as_ref())
            .collect::<Vec<_>>()
    );
}

// Compile-time silencer for the `MoveType` import — referenced via
// trait-method `is_cutting` only, which keeps the import live.
const _: fn(MoveType) -> bool = |m: MoveType| m.is_cutting();
