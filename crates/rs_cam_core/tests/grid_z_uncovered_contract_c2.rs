//! C2 — "not measured / not covered" is not a plain number.
//!
//! `SurfaceHeightmap` stores one `f64` per cell, but that number means two
//! different things: a real drop-cutter contact height on cells whose ray hits
//! the mesh, and the `min_z` clamp (the mesh bbox floor) on cells where it
//! does not. PR-8b's plane-wide ramp clamp came from a consumer reading the
//! second as if it were the first, and the backlog item's charge was that
//! *every other* consumer of the old `min_z()` was unaudited.
//!
//! These are the contract sentries for the typed replacement (`GridZ`,
//! `min_z_or_bbox_floor` / `min_covered_z`) and — the reason the audit exists
//! — for the one consumer PR-8b named as the live suspect: `steep_shallow`'s
//! Z-ladder bottom. The audit verdict there is **NOT DEFECTIVE**: the ladder
//! wants the bbox floor, and the sentry below pins WHY, so nobody "fixes" it
//! into a regression later.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

// C6: `plateau` (a `size × size` top face at z = 0 with vertical walls down to
// `z = -depth` and a bottom face) moved to `tests/common/meshes.rs` unchanged.
// Its whole point is still that a drop-cutter grid can only ever see the TOP
// face — every wall Z is below `min_covered_z()` — while the mesh has real
// material all the way down.
use common::meshes::plateau;
use common::tools::ball_cutter;

use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::slope::{GridZ, SurfaceHeightmap};
use rs_cam_core::steep_shallow::{SteepShallowParams, steep_shallow_toolpath_split_with_cancel};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::toolpath::MoveType;

/// The finish-surface grid geometry every op builds: padded by one envelope
/// radius per side, clamped at the mesh bbox floor.
fn padded_surface(mesh: &TriangleMesh, cutter: &dyn MillingCutter, cell: f64) -> SurfaceHeightmap {
    let index = SpatialIndex::build_auto(mesh);
    let r = cutter.envelope_radius_mm();
    let bbox = &mesh.bbox;
    let origin_x = bbox.min.x - r;
    let origin_y = bbox.min.y - r;
    let cols = (((bbox.max.x + r) - origin_x) / cell).ceil() as usize + 1;
    let rows = (((bbox.max.y + r) - origin_y) / cell).ceil() as usize + 1;
    SurfaceHeightmap::from_mesh(
        mesh, &index, cutter, origin_x, origin_y, rows, cols, cell, bbox.min.z,
    )
}

// ── The two minima are different numbers, and the type says which ──────

#[test]
fn padded_grids_always_have_uncovered_cells_so_the_two_minima_diverge() {
    let mesh = plateau(20.0, 5.0);
    let tool = ball_cutter(6.0);
    let hm = padded_surface(&mesh, &tool, 1.0);

    let uncovered = hm.covered_flags().iter().filter(|c| !**c).count();
    assert!(
        uncovered > 0,
        "a grid padded by one envelope radius must have uncovered corner cells"
    );

    let floor = hm.min_z_or_bbox_floor();
    let covered_min = hm.min_covered_z().expect("plateau top face is covered");

    assert!(
        (floor - mesh.bbox.min.z).abs() < 1e-9,
        "min_z_or_bbox_floor is the MESH BBOX FLOOR on any padded grid: {floor} vs {}",
        mesh.bbox.min.z
    );
    assert!(
        (covered_min - 0.0).abs() < 1e-6,
        "the deepest the cutter's reference point reaches on a plateau is its \
         TOP FACE, got {covered_min}"
    );
    assert!(
        covered_min - floor > 4.9,
        "the two readings must differ by the wall height (5 mm), got {}",
        covered_min - floor
    );
}

#[test]
fn grid_z_names_the_three_cases_the_old_f64_hid() {
    let mesh = plateau(20.0, 5.0);
    let tool = ball_cutter(6.0);
    let hm = padded_surface(&mesh, &tool, 1.0);

    // Centre cell: over the top face.
    let (cr, cc) = (hm.rows / 2, hm.cols / 2);
    match hm.z_at(cr, cc) {
        GridZ::Covered(z) => assert!((z - 0.0).abs() < 1e-6, "top face reads 0, got {z}"),
        other => panic!("centre of a plateau must be Covered, got {other:?}"),
    }

    // Corner cell: one envelope radius outside the footprint, and far enough
    // out that not even rim contact reaches it.
    match hm.z_at(0, 0) {
        GridZ::Uncovered { z_or_bbox_floor } => assert!(
            (z_or_bbox_floor - mesh.bbox.min.z).abs() < 1e-9,
            "an uncovered corner carries the bbox-floor clamp, got {z_or_bbox_floor}"
        ),
        other => panic!("corner cell must be Uncovered, got {other:?}"),
    }

    // Off-grid.
    assert_eq!(hm.z_at_world(-1e6, -1e6), GridZ::OutOfBounds);
    assert_eq!(hm.z_at_world(-1e6, -1e6).covered(), None);
    assert_eq!(hm.z_at_world(-1e6, -1e6).z_or_bbox_floor(), None);
    assert!(!hm.z_at(0, 0).is_covered());
    assert!(hm.z_at(cr, cc).is_covered());
}

/// Behaviour invariance: the escape hatch returns exactly what the pre-C2
/// `surface_z_at` / `surface_z_at_world` returned, uncovered cells included,
/// and `z_at*` is the same number wrapped.
#[test]
fn escape_hatch_is_byte_identical_to_the_untyped_read_everywhere() {
    let mesh = plateau(20.0, 5.0);
    let tool = ball_cutter(6.0);
    let hm = padded_surface(&mesh, &tool, 1.0);

    for row in 0..hm.rows {
        for col in 0..hm.cols {
            let raw = hm.z_or_bbox_floor_at(row, col);
            assert_eq!(raw, hm.z_or_bbox_floor_values()[row * hm.cols + col]);
            assert_eq!(hm.z_at(row, col).z_or_bbox_floor(), Some(raw));
            assert_eq!(hm.z_at_index(row * hm.cols + col), hm.z_at(row, col));
            assert_eq!(hm.z_at(row, col).is_covered(), hm.covered_at(row, col));

            let x = hm.origin_x + col as f64 * hm.cell_size;
            let y = hm.origin_y + row as f64 * hm.cell_size;
            assert_eq!(hm.z_or_bbox_floor_at_world(x, y), raw);
        }
    }
    // Out-of-bounds keeps the documented NEG_INFINITY spelling on the
    // untyped world read — several clearing call sites branch on it.
    assert_eq!(hm.z_or_bbox_floor_at_world(-1e6, -1e6), f64::NEG_INFINITY);
    assert_eq!(hm.z_at_index(usize::MAX), GridZ::OutOfBounds);
}

/// On a FULLY covered grid every accessor agrees and the two minima collapse
/// to one number — the invariance half of the C2 gate: nothing that reads a
/// grid without uncovered cells can observe this change.
#[test]
fn fully_covered_grids_make_the_two_minima_identical() {
    let rows = 6;
    let cols = 7;
    let z: Vec<f64> = (0..rows * cols).map(|i| -(i as f64) * 0.25).collect();
    let hm = SurfaceHeightmap::from_parts(
        z.clone(),
        vec![true; rows * cols],
        rows,
        cols,
        0.0,
        0.0,
        1.0,
    );

    let expect_min = z.iter().copied().fold(f64::INFINITY, f64::min);
    assert_eq!(hm.min_z_or_bbox_floor(), expect_min);
    assert_eq!(hm.min_covered_z(), Some(expect_min));
    for (i, &zi) in z.iter().enumerate() {
        assert_eq!(hm.z_at_index(i), GridZ::Covered(zi));
        assert_eq!(hm.z_at_index(i).covered(), Some(zi));
    }
}

#[test]
#[should_panic(expected = "coverage flags")]
fn from_parts_rejects_a_z_without_its_coverage_flag() {
    let _ = SurfaceHeightmap::from_parts(vec![0.0; 4], vec![true; 3], 2, 2, 0.0, 0.0, 1.0);
}

// ── The named suspect: steep_shallow's Z-ladder bottom ─────────────────

/// **C2 audit verdict for `steep_shallow`: bbox floor is CORRECT — not a
/// defect.** The steep half is a waterline pass that slices the MESH at each
/// level; the plateau's walls run from the top face down to the bbox floor and
/// no drop-cutter grid can see them, so `min_covered_z()` (the top face) would
/// collapse the ladder to a single level and emit no wall passes at all.
///
/// This sentry pins the consequence, not the call: cutting moves must exist
/// well below `min_covered_z()`.
#[test]
fn steep_ladder_bottom_is_the_bbox_floor_not_the_covered_minimum() {
    let mesh = plateau(20.0, 5.0);
    let index = SpatialIndex::build_auto(&mesh);
    let tool = ball_cutter(3.0);
    let hm = padded_surface(&mesh, &tool, 0.5);
    let covered_min = hm.min_covered_z().expect("top face is covered");

    let params = SteepShallowParams {
        threshold_angle: 45.0,
        z_step: 1.0,
        stepover: 1.0,
        sampling: 0.5,
        stock_to_leave: 0.0,
        safe_z: 10.0,
        ..Default::default()
    };
    let never_cancel = || false;
    let (tp, split) = steep_shallow_toolpath_split_with_cancel(
        &mesh,
        &index,
        &tool,
        &params,
        None,
        &never_cancel,
    )
    .expect("not cancellable");

    // The STEEP half only. The shallow raster also reaches the bbox floor —
    // it drop-cutters the padded grid, whose uncovered cells carry the same
    // clamp — so measuring the merged path would pass no matter what the
    // ladder bottom is. (Learned the hard way: the first version of this
    // sentry passed under the counterfactual below.)
    let steep = tp.moves.get(split.steep).expect("split range is in bounds");
    assert!(!steep.is_empty(), "the plateau must produce steep passes");
    let deepest_cut = steep
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);

    assert!(
        deepest_cut < covered_min - 2.0,
        "steep waterline passes must reach the plateau's WALLS, which live \
         below the covered minimum {covered_min:.3}; deepest cut was \
         {deepest_cut:.3}. If this fails because someone swapped the ladder \
         bottom to min_covered_z(), that is the regression this sentry exists \
         to catch — read the C2 note at the call site."
    );
    assert!(
        deepest_cut >= mesh.bbox.min.z - 1e-6,
        "and it must not go below the mesh: {deepest_cut:.3}"
    );
}
