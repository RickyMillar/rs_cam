//! Unit tests for 3D adaptive clearing.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

use super::clearing::detect_material_regions;
use super::path::Adaptive3dSegment;
use super::*;
use crate::dexel_stock::StockCutDirection;
use crate::geo::P3;
use crate::ids::ToolpathId;
use crate::mesh::SpatialIndex;
use crate::stock::dexel::{DexelSegment, MaterialSlot, ray_subtract_above};
use crate::stock::radial_profile::RadialProfileLUT;
use crate::surface::slope::SurfaceHeightmap;
use crate::tool::FlatEndmill;
use crate::toolpath::simplify_path_3d;

/// Helper: create a TriDexelStock from explicit dimensions (matching old Heightmap::from_stock).
/// Uses `z_min = -10.0` as default bottom Z unless specified.
fn make_stock(
    x_min: f64,
    y_min: f64,
    x_max: f64,
    y_max: f64,
    z_top: f64,
    cell_size: f64,
) -> TriDexelStock {
    TriDexelStock::from_stock(x_min, y_min, x_max, y_max, -10.0, z_top, cell_size)
}

/// Helper: create a TriDexelStock with custom per-cell Z-top values.
/// `cell_top_z` is row-major; each cell gets a single segment [z_min, cell_z].
pub(super) fn make_stock_with_cells(
    rows: usize,
    cols: usize,
    origin_x: f64,
    origin_y: f64,
    cell_size: f64,
    z_min: f64,
    cell_top_z: &[f64],
) -> TriDexelStock {
    use smallvec::SmallVec;
    let z_max = cell_top_z.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let bbox = crate::geo::BoundingBox3 {
        min: P3::new(origin_x, origin_y, z_min),
        max: P3::new(
            origin_x + (cols - 1) as f64 * cell_size,
            origin_y + (rows - 1) as f64 * cell_size,
            z_max,
        ),
    };
    let mut rays = Vec::with_capacity(rows * cols);
    for &z in cell_top_z {
        if z <= z_min + 1e-9 {
            // No material
            rays.push(SmallVec::new());
        } else {
            let seg = DexelSegment::new(z_min as f32, z as f32, MaterialSlot::STOCK);
            rays.push(SmallVec::from_buf([seg]));
        }
    }
    // A/M10: this grid is built from an explicit per-cell top array, so
    // each cell's top IS the stated height — there is no sub-cell blend
    // to be conservative about. Seed the sliver-safe bound to the same
    // values rather than to the bbox top, which would make every descent
    // over this fixture clear the full stock height.
    let conservative_top: Vec<f32> = cell_top_z.iter().map(|&z| z as f32).collect();
    let grid = crate::stock::dexel::DexelGrid {
        rays,
        rows,
        cols,
        origin_u: origin_x,
        origin_v: origin_y,
        cell_size,
        axis: crate::stock::dexel::DexelAxis::Z,
        conservative_top,
        has_added_material: false,
    };
    TriDexelStock {
        z_grid: grid,
        x_grid: None,
        y_grid: None,
        stock_bbox: bbox,
        stamp_dispatch: Default::default(),
        last_stamp_dispatch: Default::default(),
        playback_dispatch: Default::default(),
        last_playback_dispatch: Default::default(),
        materials: Default::default(),
    }
}

fn make_flat_mesh() -> (TriangleMesh, SpatialIndex) {
    let mesh = crate::mesh::make_test_flat(50.0);
    let si = SpatialIndex::build(&mesh, 10.0);
    (mesh, si)
}

fn make_hemisphere_mesh() -> (TriangleMesh, SpatialIndex) {
    let mesh = crate::mesh::make_test_hemisphere(20.0, 16);
    let si = SpatialIndex::build(&mesh, 10.0);
    (mesh, si)
}

fn flat_cutter() -> FlatEndmill {
    FlatEndmill::new(6.35, 25.0)
}

fn default_params() -> Adaptive3dParams {
    Adaptive3dParams {
        entry_clearance_mm: 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: 3.175,
            envelope_radius: 3.175,
            stepover: 2.0,
            tolerance: 0.1,
            segment_merge_tolerance: None,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
            centre_boundary: Vec::new(),
            gouge_probe: None,
        },
        depth: Adaptive3dDepth {
            depth_per_pass: 3.0,
            stock_to_leave: 0.5,
            stock_top_z: 25.0,
            z_floor: None,
            detect_flat_areas: false,
            rest_top_z: None,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global, // Disabled by default in tests — tests that need to exercise
            // the F-038 fragmentation filter set this explicitly.
            min_region_cut_length_mm: 0.0, // F-038b: tests opt out by default (None ⇒ 8×diam in production,
            // but the unit-test fixtures here exercise the legacy code path
            // unless they specifically target the keep-tool-down logic).
            max_stay_down_distance_mm: Some(0.0),
            stay_down_clearance_mm: 0.5,
            step_split: None,
            keep_down_route: None,
            level_contour: None,
            rest_region_filter: None,
            one_pass_hold: super::OnePassHold::Off,
        },
        trochoid_cap_mult: 1.6,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 30.0,
        entry_style: EntryStyle3d::Plunge,
        engagement_measure: crate::adaptive::EngagementMeasure::DiskArea,
        initial_stock: None, // Matches the GUI/MCP default (ContourParallel) so the bulk of
        // adaptive3d unit tests exercise the code path most users reach
        // in production. Tests that specifically validate GeodesicAdaptive
        // override this field explicitly.
        clearing_strategy: ClearingStrategy3d::ContourParallel,
    }
}

// ── Surface heightmap tests ──────────────────────────────────────

#[test]
fn test_surface_heightmap_flat() {
    let (mesh, si) = make_flat_mesh();
    let cutter = flat_cutter();
    // Grid within mesh footprint (mesh is 50x50, centered at origin)
    let shm = SurfaceHeightmap::from_mesh(&mesh, &si, &cutter, -20.0, -20.0, 8, 8, 5.0, -10.0);
    // Interior cells should have surface Z near 0 (flat mesh at z=0)
    // Edge cells might get min_z if outside mesh footprint
    let mut interior_count = 0;
    for row in 1..shm.rows - 1 {
        for col in 1..shm.cols - 1 {
            let z = shm.z_or_bbox_floor_at(row, col);
            assert!(
                (-1.0..=1.0).contains(&z),
                "Interior flat mesh Z should be near 0, got {:.2} at ({}, {})",
                z,
                row,
                col
            );
            interior_count += 1;
        }
    }
    assert!(interior_count > 10, "Should have checked interior cells");
}

#[test]
fn test_surface_heightmap_hemisphere() {
    let (mesh, si) = make_hemisphere_mesh();
    let cutter = flat_cutter();
    let bbox = &mesh.bbox;
    let shm = SurfaceHeightmap::from_mesh(
        &mesh,
        &si,
        &cutter,
        bbox.min.x - 5.0,
        bbox.min.y - 5.0,
        20,
        20,
        3.0,
        bbox.min.z,
    );

    // Center should be higher than edges
    let center_row = shm.rows / 2;
    let center_col = shm.cols / 2;
    let center_z = shm.z_or_bbox_floor_at(center_row, center_col);
    let edge_z = shm.z_or_bbox_floor_at(0, 0);
    assert!(
        center_z > edge_z,
        "Hemisphere center ({:.1}) should be higher than edge ({:.1})",
        center_z,
        edge_z
    );
}

// ── Z level computation ─────────────────────────────────────────────

#[test]
fn test_z_level_computation() {
    let stock_top = 20.0;
    let depth_per_pass = 5.0;
    let surface_bottom = 0.0;
    let stock_to_leave = 0.5;
    let z_bottom = surface_bottom + stock_to_leave;

    let mut z_levels: Vec<f64> = Vec::new();
    let mut z = stock_top - depth_per_pass;
    while z > z_bottom {
        z_levels.push(z);
        z -= depth_per_pass;
    }
    z_levels.push(z_bottom);

    assert_eq!(z_levels.len(), 4, "Should have 4 levels: [15, 10, 5, 0.5]");
    assert!((z_levels[0] - 15.0_f64).abs() < 0.01);
    assert!((z_levels[1] - 10.0_f64).abs() < 0.01);
    assert!((z_levels[2] - 5.0_f64).abs() < 0.01);
    assert!((z_levels[3] - 0.5_f64).abs() < 0.01);
}

// ── Path simplification ─────────────────────────────────────────────

#[test]
fn test_simplify_path_3d() {
    // Collinear 3D points should simplify
    let path = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(1.0, 0.0, 1.0),
        P3::new(2.0, 0.0, 2.0),
        P3::new(3.0, 0.0, 3.0),
    ];
    let simplified = simplify_path_3d(&path, 0.01);
    assert_eq!(
        simplified.len(),
        2,
        "Collinear 3D points should reduce to 2"
    );

    // Non-collinear should be preserved
    let path2 = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(1.0, 5.0, 1.0),
        P3::new(2.0, 0.0, 2.0),
    ];
    let simplified2 = simplify_path_3d(&path2, 0.01);
    assert_eq!(simplified2.len(), 3, "Non-collinear should be preserved");
}

// ── Integration tests ───────────────────────────────────────────────

#[test]
fn test_adaptive_3d_flat_produces_toolpath() {
    let (mesh, si) = make_flat_mesh();
    let cutter = flat_cutter();
    let params = {
        // Coarse for speed.
        let mut p = default_params();
        p.depth.stock_top_z = 5.0;
        // 5mm above flat mesh at z=0
        p.depth.depth_per_pass = 5.0;
        // Single level
        p.depth.stock_to_leave = 0.0;
        p.geometry.tolerance = 0.5;
        p
    };

    let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
    assert!(
        tp.moves.len() > 10,
        "Should produce a non-trivial toolpath, got {} moves",
        tp.moves.len()
    );
    assert!(
        tp.total_cutting_distance() > 10.0,
        "Should have meaningful cutting distance, got {:.1}mm",
        tp.total_cutting_distance()
    );
}

#[test]
fn test_adaptive_3d_hemisphere_multi_level() {
    let (mesh, si) = make_hemisphere_mesh();
    let cutter = flat_cutter();
    let params = {
        // Coarse for speed.
        let mut p = default_params();
        p.depth.stock_top_z = 25.0;
        // Above hemisphere peak (~20)
        p.depth.depth_per_pass = 5.0;
        p.depth.stock_to_leave = 0.5;
        p.geometry.tolerance = 0.5;
        p
    };

    let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
    assert!(
        tp.moves.len() > 20,
        "Hemisphere should produce multi-level passes, got {} moves",
        tp.moves.len()
    );

    // Z values should span from near stock_top down to near surface
    let min_z = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    assert!(
        min_z < 15.0,
        "Should cut down to lower Z levels, min feed Z = {:.1}",
        min_z
    );
}

/// Removal step R6: `trochoid_cap_mult` and `engagement_measure` are
/// load-only. Two values of each give the same toolpath on both
/// strategies. Each pair holds the other field at its default, so a
/// difference names the field that a planner reads.
#[test]
fn the_load_only_fields_do_not_change_the_toolpath_r6() {
    let (mesh, si) = make_hemisphere_mesh();
    let cutter = flat_cutter();
    for strategy in [
        ClearingStrategy3d::ContourParallel,
        ClearingStrategy3d::GeodesicAdaptive,
    ] {
        let base = || {
            let mut p = default_params();
            p.depth.depth_per_pass = 5.0;
            p.geometry.tolerance = 0.5;
            p.clearing_strategy = strategy;
            p
        };
        let reference = format!(
            "{:?}",
            adaptive_3d_toolpath(&mesh, &si, &cutter, &base()).moves
        );
        let mut cap = base();
        cap.trochoid_cap_mult *= 2.0;
        let mut measure = base();
        measure.engagement_measure = crate::adaptive::EngagementMeasure::LeadingArc;
        for (field, params) in [("trochoid_cap_mult", cap), ("engagement_measure", measure)] {
            let moves = format!(
                "{:?}",
                adaptive_3d_toolpath(&mesh, &si, &cutter, &params).moves
            );
            assert!(
                moves == reference,
                "{strategy:?}: `{field}` changed the toolpath; a planner reads it"
            );
        }
    }
}

/// DIAGNOSTIC: dump per-Cut-segment Z statistics for the slice pipeline
/// (Geodesic Adaptive since removal step R4; AgentSearch before) on a
/// hemisphere. Used to verify that the Z-drop split fix in
/// `clear_z_level_slice` is working — and to detect cases
/// the threshold misses (e.g., cumulative descent across many small
/// dz steps).
///
/// Prints to stdout via `println!` (test-mode only; not subject to
/// the production-code `print_stdout` lint). Run with:
///   cargo test -p rs_cam_core --lib geodesic_adaptive_z_drop_diag -- --nocapture
#[test]
fn geodesic_adaptive_z_drop_diag() {
    let (mesh, si) = make_hemisphere_mesh();
    let cutter = flat_cutter();
    let params = {
        let mut p = default_params();
        p.depth.stock_top_z = 25.0;
        p.depth.depth_per_pass = 3.0;
        p.depth.stock_to_leave = 0.5;
        p.geometry.tolerance = 0.5;
        p.geometry.stepover = 1.0;
        p.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
        p
    };

    let never_cancel = || false;
    let segments = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
        .expect("segments")
        .segments;

    let mut max_per_step_dz = 0.0f64;
    let mut max_path_total_descent = 0.0f64;
    let mut paths_with_descent_gt_dpp = 0usize;
    let mut total_cut_paths = 0usize;
    let mut largest_path_len = 0usize;
    let mut largest_path_descent_summary = String::new();

    for seg in &segments {
        if let Adaptive3dSegment::Cut(path) = seg {
            if path.len() < 2 {
                continue;
            }
            total_cut_paths += 1;
            let mut path_max_dz = 0.0f64;
            let mut path_total_descent = 0.0f64;
            let mut path_max_contig_descent = 0.0f64;
            let mut current_descent = 0.0f64;
            for w in path.windows(2) {
                let dz = w[1].z - w[0].z;
                path_max_dz = path_max_dz.max(dz.abs());
                if dz < 0.0 {
                    path_total_descent += -dz;
                    current_descent += -dz;
                    path_max_contig_descent = path_max_contig_descent.max(current_descent);
                } else {
                    current_descent = 0.0;
                }
            }
            max_per_step_dz = max_per_step_dz.max(path_max_dz);
            if path_total_descent > max_path_total_descent {
                max_path_total_descent = path_total_descent;
                largest_path_len = path.len();
                largest_path_descent_summary = format!(
                    "len={} max_step_dz={:.3} total_descent={:.3} max_contig={:.3}",
                    path.len(),
                    path_max_dz,
                    path_total_descent,
                    path_max_contig_descent
                );
            }
            if path_total_descent > params.depth.depth_per_pass {
                paths_with_descent_gt_dpp += 1;
            }
        }
    }

    println!("\n=== GeodesicAdaptive Z-drop diagnostics (hemisphere) ===");
    println!("total Cut paths: {}", total_cut_paths);
    println!("max per-step |dz|: {:.3} mm", max_per_step_dz);
    println!(
        "max path total descent: {:.3} mm (over {} pts)",
        max_path_total_descent, largest_path_len
    );
    println!(
        "paths with total descent > depth_per_pass ({:.1}): {}",
        params.depth.depth_per_pass, paths_with_descent_gt_dpp
    );
    println!("worst path: {}", largest_path_descent_summary);

    // The split fix should keep per-step |dz| under depth_per_pass × 1.1.
    let threshold = params.depth.depth_per_pass * 1.1;
    assert!(
        max_per_step_dz <= threshold + 0.01,
        "split fix failed: max per-step |dz| = {:.3} mm exceeds threshold {:.3} mm",
        max_per_step_dz,
        threshold
    );
}

#[test]
fn test_adaptive_3d_z_follows_surface() {
    let (mesh, si) = make_flat_mesh();
    let cutter = flat_cutter();
    let params = {
        let mut p = default_params();
        p.depth.stock_top_z = 5.0;
        p.depth.depth_per_pass = 5.0;
        p.depth.stock_to_leave = 0.5;
        p.geometry.tolerance = 0.5;
        p
    };

    let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);

    // All cutting moves should be at or above stock_to_leave
    for m in &tp.moves {
        if let crate::toolpath::MoveType::Linear { .. } = m.move_type
            && m.target.z < params.safe_z - 1.0
        {
            assert!(
                m.target.z >= params.depth.stock_to_leave - 1.0,
                "Cut Z ({:.2}) should be >= stock_to_leave ({:.1}) - tolerance",
                m.target.z,
                params.depth.stock_to_leave
            );
        }
    }
}

// ── Fix 1: Z-rate clamping test ────────────────────────────────────

#[test]
fn test_z_rate_clamp_limits_descent() {
    // Verify that Z-rate clamping works in the internal stepping loop.
    // We test by calling adaptive_3d_segments directly and inspecting Cut paths
    // before simplification/blending.
    let (mesh, si) = make_hemisphere_mesh();
    let cutter = flat_cutter();
    let depth_per_pass = 3.0;
    let params = {
        let mut p = default_params();
        p.depth.stock_top_z = 25.0;
        p.depth.depth_per_pass = depth_per_pass;
        p.depth.stock_to_leave = 0.5;
        p.geometry.tolerance = 0.5;
        p
    };

    let never_cancel = || false;
    let segments = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
        .expect("test helper should not cancel")
        .segments;

    // Check raw Cut segments: consecutive points should not drop > depth_per_pass
    let mut checked = 0;
    for seg in &segments {
        if let Adaptive3dSegment::Cut(path) = seg {
            for window in path.windows(2) {
                let z_drop = window[0].z - window[1].z;
                if z_drop > 0.0 {
                    assert!(
                        z_drop <= depth_per_pass + 0.1,
                        "Raw path Z drop {:.2} exceeds depth_per_pass {:.1}",
                        z_drop,
                        depth_per_pass,
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 0, "Should have checked some downward Z moves");
}

// ── Fix 2: Helix entry test ────────────────────────────────────────

#[test]
fn test_helix_entry_no_vertical_plunge() {
    let (mesh, si) = make_flat_mesh();
    let cutter = flat_cutter();
    let params = {
        let mut p = default_params();
        p.depth.stock_top_z = 5.0;
        p.depth.depth_per_pass = 5.0;
        p.depth.stock_to_leave = 0.0;
        p.geometry.tolerance = 0.5;
        p.entry_style = EntryStyle3d::Helix {
            radius: cutter.radius() * 0.8,
            pitch: 1.0,
            // The fallback angle of every helix before P29-8.
            fallback_ramp_angle_deg: 3.0,
        };
        p
    };

    let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
    assert!(tp.moves.len() > 5, "Should produce a toolpath");

    // With helix entry, there should be feed moves that descend while
    // moving in XY (helix spiral). Individual helix steps are small,
    // so check for any downward-feed with XY motion.
    let mut has_helix_moves = false;
    for window in tp.moves.windows(2) {
        if let crate::toolpath::MoveType::Linear { .. } = window[1].move_type {
            let dx = (window[1].target.x - window[0].target.x).abs();
            let dy = (window[1].target.y - window[0].target.y).abs();
            let dz = window[0].target.z - window[1].target.z;
            // A helix step descends while moving in XY
            if dz > 0.005 && (dx > 0.01 || dy > 0.01) {
                has_helix_moves = true;
                break;
            }
        }
    }
    assert!(
        has_helix_moves,
        "Helix entry should produce moves with simultaneous XY+Z motion"
    );
}

// ── The Z levels ──────────────────────────────────────────────────────

/// The level plan must be the old level loop, bit for bit: the byte-parity
/// fixtures of every existing job depend on it. The step 0.7 and the odd
/// stock top make the repeated subtraction drift, so a `top - k * step`
/// form would fail here.
#[test]
fn step_levels_are_the_legacy_loop_bitwise() {
    let stock_top = 12.345_f64;
    let dpp = 0.7_f64;
    let z_bottom = -3.21_f64;
    let mut legacy = Vec::new();
    let mut z = stock_top - dpp;
    while z > z_bottom {
        legacy.push(z);
        z -= dpp;
    }
    if z_bottom < stock_top {
        legacy.push(z_bottom);
    }

    let plan = super::path::step_levels(stock_top, z_bottom, dpp);
    assert_eq!(plan.len(), legacy.len());
    for (l, z) in plan.iter().zip(&legacy) {
        assert_eq!(l.to_bits(), z.to_bits(), "level {l} vs legacy {z}");
    }
}

/// D50 (integration design §3.1): the source's left stock top lowers only
/// the START of a rest consumer's level plan, never its floor. On
/// remaining stock the plan starts at the top; its last level is the
/// floor with or without the top; a top at or below the floor, and any top
/// on fresh stock, leave the plan as it was.
#[test]
fn a_rest_top_lowers_the_start_and_keeps_the_floor_d50() {
    use super::path::{rest_capped_level_top, step_levels};
    let (anchor, top, z_bottom, dpp) = (8.0_f64, 7.0_f64, 4.0_f64, 3.0_f64);
    let plain = step_levels(anchor, z_bottom, dpp);
    let capped = step_levels(
        rest_capped_level_top(anchor, Some(top), z_bottom, true),
        z_bottom,
        dpp,
    );
    assert_eq!(plain, vec![5.0, 4.0]);
    assert_eq!(capped, vec![4.0], "the plan does not start below the top");
    assert_eq!(plain.last(), capped.last(), "the top moved the floor");
    // A top at or below the floor would leave no level: it is not applied.
    for low in [z_bottom, z_bottom - dpp] {
        assert_eq!(
            rest_capped_level_top(anchor, Some(low), z_bottom, true),
            anchor
        );
    }
    // Fresh stock, or no top: the anchor stays.
    assert_eq!(
        rest_capped_level_top(anchor, Some(top), z_bottom, false),
        anchor
    );
    assert_eq!(rest_capped_level_top(anchor, None, z_bottom, true), anchor);
    // A top above the anchor does not raise the start.
    assert_eq!(
        rest_capped_level_top(anchor, Some(anchor + dpp), z_bottom, true),
        anchor
    );
}

// ── Fix 5: Flat area detection test ────────────────────────────────

#[test]
fn test_flat_area_detection_finds_shelf() {
    // Build a surface heightmap where many cells sit at z=10 (a shelf)
    // and the rest sit at z=0 (floor)
    let cell_size = 1.0;
    let rows = 20;
    let cols = 20;
    let mut z_values = vec![0.0; rows * cols];
    // Create a shelf: rows 5..15, cols 5..15 at z=10
    for row in 5..15 {
        for col in 5..15 {
            z_values[row * cols + col] = 10.0;
        }
    }

    let covered = vec![true; z_values.len()];
    let shm = SurfaceHeightmap::from_parts(z_values, covered, rows, cols, 0.0, 0.0, cell_size);

    // The PRODUCTION detector, not a copy of it. This test held a
    // verbatim re-implementation of the histogram until 2026-09-17, so a
    // change to the real detector left it green (Q6).
    let tolerance: f64 = 0.1;
    let stock_to_leave: f64 = 0.5;
    let stock_top: f64 = 25.0;
    let z_bottom = 0.0 + stock_to_leave;
    let flat_levels = super::path::flat_shelf_levels(
        &shm,
        0.0,
        stock_top,
        tolerance,
        stock_to_leave,
        z_bottom,
        &[],
    );

    // Should detect the shelf at z≈10 (+stock_to_leave=0.5 → 10.5)
    let found_shelf = flat_levels.iter().any(|&z| (z - 10.5).abs() < 1.0);
    assert!(
        found_shelf,
        "Should detect shelf near z=10.5, found levels: {:?}",
        flat_levels
    );
}

// ── Region detection tests ───────────────────────────────────────────

#[test]
fn test_detect_regions_single_block() {
    // Full material → 1 region covering entire grid
    let (mesh, si) = make_flat_mesh();
    let cutter = flat_cutter();
    let cell_size = 2.0;

    let material_stock = make_stock(-30.0, -30.0, 30.0, 30.0, 20.0, cell_size);
    let surface_hm = SurfaceHeightmap::from_mesh(
        &mesh,
        &si,
        &cutter,
        material_stock.z_grid.origin_u,
        material_stock.z_grid.origin_v,
        material_stock.z_grid.rows,
        material_stock.z_grid.cols,
        cell_size,
        -10.0,
    );

    let regions = detect_material_regions(&material_stock, &surface_hm, 0.5);
    assert!(
        !regions.is_empty(),
        "Full material should produce at least 1 region"
    );
    // Largest region should cover most of the grid
    let total_cells = material_stock.z_grid.rows * material_stock.z_grid.cols;
    assert!(
        regions[0].cell_count > total_cells / 2,
        "Largest region should cover most cells: {} / {}",
        regions[0].cell_count,
        total_cells
    );
}

#[test]
fn test_detect_regions_two_islands() {
    // Two separated blocks → 2 regions, sorted by area
    let cell_size = 1.0;
    let material_stock = make_stock(0.0, 0.0, 30.0, 10.0, 20.0, cell_size);
    let rows = material_stock.z_grid.rows;
    let cols = material_stock.z_grid.cols;

    // Surface at z=0 everywhere
    let surface_hm = SurfaceHeightmap::from_parts(
        vec![0.0; rows * cols],
        vec![true; rows * cols],
        rows,
        cols,
        material_stock.z_grid.origin_u,
        material_stock.z_grid.origin_v,
        cell_size,
    );

    // Create two islands by clearing a gap in the middle
    let mut hm = material_stock;
    for row in 0..rows {
        for col in 0..cols {
            let (x, _y) = hm.z_grid.cell_to_world(row, col);
            if (13.0..=17.0).contains(&x) {
                // Clear the gap — remove all material
                ray_subtract_above(hm.z_grid.ray_mut(row, col), hm.stock_bbox.min.z as f32);
            }
        }
    }

    let regions = detect_material_regions(&hm, &surface_hm, 0.5);
    assert!(
        regions.len() >= 2,
        "Should detect at least 2 separate regions, got {}",
        regions.len()
    );
    // Sorted by area descending
    assert!(
        regions[0].cell_count >= regions[1].cell_count,
        "Regions should be sorted by area descending"
    );
}

#[test]
fn test_detect_regions_diagonal_connected() {
    // Diagonal-touching blocks → 1 region (8-connected)
    let cell_size = 1.0;
    let rows = 10;
    let cols = 10;

    // Surface at z=0, material at z=20 only on diagonal cells
    let mut mat_cells = vec![0.0f64; rows * cols];
    for i in 0..rows.min(cols) {
        mat_cells[i * cols + i] = 20.0;
    }

    let hm = make_stock_with_cells(rows, cols, 0.0, 0.0, cell_size, -10.0, &mat_cells);
    let surface_hm = SurfaceHeightmap::from_parts(
        vec![0.0; rows * cols],
        vec![true; rows * cols],
        rows,
        cols,
        0.0,
        0.0,
        cell_size,
    );

    let regions = detect_material_regions(&hm, &surface_hm, 0.5);
    assert_eq!(
        regions.len(),
        1,
        "Diagonal cells should form 1 region with 8-connectivity, got {}",
        regions.len()
    );
}

#[test]
fn test_detect_regions_small_filtered() {
    // Isolated cells (< 4) should be filtered out
    let cell_size = 1.0;
    let rows = 10;
    let cols = 10;

    // Only 2 adjacent cells have material
    let mut mat_cells = vec![0.0f64; rows * cols];
    mat_cells[0] = 20.0;
    mat_cells[1] = 20.0;

    let hm = make_stock_with_cells(rows, cols, 0.0, 0.0, cell_size, -10.0, &mat_cells);
    let surface_hm = SurfaceHeightmap::from_parts(
        vec![0.0; rows * cols],
        vec![true; rows * cols],
        rows,
        cols,
        0.0,
        0.0,
        cell_size,
    );

    let regions = detect_material_regions(&hm, &surface_hm, 0.5);
    assert!(
        regions.is_empty(),
        "Tiny regions (< 4 cells) should be filtered out, got {} regions",
        regions.len()
    );
}

// ── Integration: ByArea ordering ─────────────────────────────────────

#[test]
fn test_adaptive_3d_by_area_flat() {
    let (mesh, si) = make_flat_mesh();
    let cutter = flat_cutter();
    let params = {
        let mut p = default_params();
        p.depth.stock_top_z = 5.0;
        p.depth.depth_per_pass = 5.0;
        p.depth.stock_to_leave = 0.0;
        p.geometry.tolerance = 0.5;
        p.linking.region_ordering = RegionOrdering::ByArea;
        p
    };

    let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
    assert!(
        tp.moves.len() > 10,
        "ByArea on flat mesh should produce toolpath, got {} moves",
        tp.moves.len()
    );
    assert!(
        tp.total_cutting_distance() > 10.0,
        "ByArea should have meaningful cutting distance, got {:.1}mm",
        tp.total_cutting_distance()
    );
}

#[test]
fn test_adaptive_3d_by_area_hemisphere() {
    let (mesh, si) = make_hemisphere_mesh();
    let cutter = flat_cutter();
    let params = {
        let mut p = default_params();
        p.depth.stock_top_z = 25.0;
        p.depth.depth_per_pass = 5.0;
        p.depth.stock_to_leave = 0.5;
        p.geometry.tolerance = 0.5;
        p.linking.region_ordering = RegionOrdering::ByArea;
        p
    };

    let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
    assert!(
        tp.moves.len() > 20,
        "ByArea on hemisphere should produce multi-level passes, got {} moves",
        tp.moves.len()
    );

    // Z values should span a useful range
    let min_z = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    assert!(
        min_z < 15.0,
        "ByArea should cut down to lower Z levels, min feed Z = {:.1}",
        min_z
    );
}

// ── Widening coverage test ──────────────────────────────────────────

#[test]
fn test_widening_covers_stepover() {
    // Verify that path widening stamps cells at stepover distance.
    let (mesh, si) = make_flat_mesh();
    let cutter = flat_cutter();
    let cell_size = 0.5;
    let stepover = 2.0;

    let mut material_stock = make_stock(-30.0, -30.0, 30.0, 30.0, 20.0, cell_size);
    let surface_hm = SurfaceHeightmap::from_mesh(
        &mesh,
        &si,
        &cutter,
        material_stock.z_grid.origin_u,
        material_stock.z_grid.origin_v,
        material_stock.z_grid.rows,
        material_stock.z_grid.cols,
        cell_size,
        -10.0,
    );

    // Simulate a straight horizontal path at y=0, from x=-10 to x=10
    let z_level = 10.0;
    let path: Vec<P3> = (0..=40)
        .map(|i| P3::new(-10.0 + i as f64 * 0.5, 0.0, z_level))
        .collect();

    // Stamp along the path itself
    let lut = RadialProfileLUT::from_cutter(&cutter, crate::stock::radial_profile::LUT_SAMPLES);
    for p in &path {
        material_stock.stamp_tool_at(
            &lut,
            cutter.radius(),
            p.x,
            p.y,
            p.z,
            StockCutDirection::FromTop,
        );
    }

    // Now apply widening with double ring at stepover distance
    for i in 1..path.len() {
        let prev = &path[i - 1];
        let curr = &path[i];
        let dx = curr.x - prev.x;
        let dy = curr.y - prev.y;
        let seg_len = (dx * dx + dy * dy).sqrt();
        if seg_len < 1e-10 {
            continue;
        }
        let nx = -dy / seg_len;
        let ny = dx / seg_len;
        for &mult in &[1.0f64, 2.0] {
            for &sign in &[1.0f64, -1.0] {
                let px = curr.x + sign * mult * stepover * nx;
                let py = curr.y + sign * mult * stepover * ny;
                let sz = surface_hm.z_or_bbox_floor_at_world(px, py);
                if sz != f64::NEG_INFINITY {
                    let pz = (sz + 0.5).max(z_level);
                    material_stock.stamp_tool_at(
                        &lut,
                        cutter.radius(),
                        px,
                        py,
                        pz,
                        StockCutDirection::FromTop,
                    );
                }
            }
        }
    }

    // Check that cells at y = +/- stepover are cleared (material lowered from 20)
    for &y_off in &[stepover, -stepover, 2.0 * stepover, -2.0 * stepover] {
        if let Some((row, col)) = material_stock.z_grid.world_to_cell(0.0, y_off) {
            let z = stock_top_z_at(&material_stock, row, col);
            assert!(
                z < 20.0 - 0.1,
                "Cell at y={:.1} should be widened (z lowered from 20), got z={:.2}",
                y_off,
                z
            );
        }
    }
}

// ── Low-yield bail test ─────────────────────────────────────────────

#[test]
fn test_low_yield_bail() {
    // Thin-film material (just above floor) — adaptive should bail quickly.
    let (mesh, si) = make_flat_mesh();
    let cutter = flat_cutter();

    // Stock barely above surface: 0.2mm of material (below thin_threshold)
    // Pre-stamp should eliminate this, so adaptive should do minimal work.
    let params = {
        let mut p = default_params();
        p.depth.stock_top_z = 0.2;
        // Only 0.2mm above flat mesh at z=0
        p.depth.depth_per_pass = 3.0;
        p.depth.stock_to_leave = 0.0;
        p.geometry.tolerance = 0.5;
        p
    };

    let never_cancel = || false;
    let segments = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
        .expect("test helper should not cancel")
        .segments;

    // Count actual cutting passes
    let cut_count = segments
        .iter()
        .filter(|s| matches!(s, Adaptive3dSegment::Cut(_)))
        .count();

    // With thin-film material, should bail quickly (few or no passes)
    assert!(
        cut_count < 20,
        "Thin film should produce few cutting passes, got {}",
        cut_count
    );
}

#[test]
fn traced_adaptive3d_emits_spans_hotspots_and_annotations() {
    let (mesh, si) = make_hemisphere_mesh();
    let cutter = flat_cutter();
    // This test validates the slice pipeline's observability (Geodesic
    // Adaptive since removal step R4, AgentSearch before): z_level and
    // agent2d_region spans are emitted by the slice dispatch, not by
    // clear_z_level_contour_parallel. Removal step R4: the 2D agent
    // walker's adaptive_pass spans and hotspots came from AgentSearch
    // planning each region with it; Geodesic Adaptive plans with its own
    // field, so no 3D strategy emits them now (the 2D Adaptive op does).
    let params = {
        let mut p = default_params();
        p.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
        p
    };
    let recorder = crate::trace::debug_trace::ToolpathDebugRecorder::new("Adaptive 3D", "3D Rough");
    let ctx = recorder.root_context();
    let never_cancel = || false;

    let tp = adaptive_3d_toolpath_traced_with_cancel(
        &mesh,
        &si,
        &cutter,
        &params,
        &never_cancel,
        Some(&ctx),
    )
    .expect("debug run should complete");
    let trace = recorder.finish();

    assert!(!tp.moves.is_empty(), "expected a non-empty toolpath");
    assert!(
        trace
            .spans
            .iter()
            .any(|span| span.kind == "surface_heightmap"),
        "trace should include surface heightmap timing"
    );
    assert!(
        trace.spans.iter().any(|span| span.kind == "z_level"),
        "trace should include Z-level spans"
    );
    assert!(
        trace.spans.iter().any(|span| span.kind == "agent2d_region"),
        "trace should include the slice-region spans"
    );
    assert!(
        !trace.annotations.is_empty(),
        "adaptive 3D trace should carry generated annotations"
    );
}

// ── Contour-parallel EDT tests ───────────────────────────────────

#[test]
fn test_contour_parallel_edt_flat_mesh() {
    let (mesh, si) = make_flat_mesh();
    let cutter = flat_cutter();
    let params = {
        let mut p = default_params();
        p.depth.stock_top_z = 5.0;
        p.depth.depth_per_pass = 5.0;
        p.depth.stock_to_leave = 0.0;
        p.geometry.tolerance = 0.5;
        p.clearing_strategy = ClearingStrategy3d::ContourParallel;
        p
    };

    let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
    assert!(
        tp.moves.len() > 5,
        "Contour-parallel EDT on flat mesh should produce moves, got {}",
        tp.moves.len()
    );

    // Check that there are actual cutting moves (Linear with non-rapid feed)
    let cut_moves = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
        .count();
    assert!(
        cut_moves > 0,
        "Contour-parallel EDT should produce cutting moves, got 0"
    );
}

#[test]
fn test_contour_parallel_edt_hemisphere() {
    let (mesh, si) = make_hemisphere_mesh();
    let cutter = flat_cutter();
    let params = {
        let mut p = default_params();
        p.depth.stock_top_z = 25.0;
        p.depth.depth_per_pass = 5.0;
        p.depth.stock_to_leave = 0.5;
        p.geometry.tolerance = 0.5;
        p.clearing_strategy = ClearingStrategy3d::ContourParallel;
        p
    };

    let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
    assert!(
        tp.moves.len() > 10,
        "Contour-parallel EDT on hemisphere should produce multi-level passes, got {} moves",
        tp.moves.len()
    );

    // Z values should span multiple levels
    let min_z = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    let max_z = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
        .map(|m| m.target.z)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max_z - min_z > 3.0,
        "Hemisphere contour-parallel should span multiple Z levels, range = {:.1}",
        max_z - min_z
    );
}

/// Validate that contour-parallel clearing produces complete material removal
/// on a flat mesh (single Z level, no terrain variation).
#[test]
fn test_contour_parallel_complete_clearing() {
    use crate::dexel_stock::StockCutDirection;
    use crate::stock::radial_profile::RadialProfileLUT;

    let (mesh, si) = make_flat_mesh(); // 50x50mm flat at z=0
    let cutter = flat_cutter(); // 6.35mm diameter
    let tool_radius = cutter.radius();
    let stock_top_z = 5.0;
    let stock_to_leave = 0.0;

    let params = {
        let mut p = default_params();
        p.geometry.tool_radius = tool_radius;
        p.geometry.stepover = 2.5;
        p.depth.depth_per_pass = 5.0;
        p.depth.stock_to_leave = stock_to_leave;
        p.geometry.tolerance = 0.3;
        p.depth.stock_top_z = stock_top_z;
        p.clearing_strategy = ClearingStrategy3d::ContourParallel;
        p
    };

    let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
    assert!(tp.moves.len() > 10, "Should produce moves");

    // Simulate the toolpath on a fresh stock
    let cell_size = 0.3;
    let mut sim_stock =
        TriDexelStock::from_stock(-25.5, -25.5, 25.5, 25.5, -1.0, stock_top_z, cell_size);
    let lut = RadialProfileLUT::from_cutter(&cutter, crate::stock::radial_profile::LUT_SAMPLES);
    sim_stock
        .simulate_toolpath_with_lut_cancel(
            &tp,
            &lut,
            tool_radius,
            StockCutDirection::FromTop,
            &|| false,
        )
        .unwrap();

    assert_clearing_complete(&sim_stock, tool_radius, cell_size, stock_to_leave, "flat");
}

/// Validate clearing completeness on a hemisphere (multi-Z-level, terrain).
///
/// Only checks cells WITHIN the mesh bounding box (the adaptive3d internal stock
/// only covers mesh_bbox ± tool_radius). Cells outside that range are expected uncleared.
#[test]
fn test_contour_parallel_complete_clearing_hemisphere() {
    use crate::dexel_stock::StockCutDirection;
    use crate::stock::radial_profile::RadialProfileLUT;

    let (mesh, si) = make_hemisphere_mesh(); // radius=20, centered at origin
    let cutter = flat_cutter(); // 6.35mm diameter
    let tool_radius = cutter.radius();
    let stock_top_z = 25.0;
    let stock_to_leave = 0.5;

    let params = {
        let mut p = default_params();
        p.geometry.tool_radius = tool_radius;
        p.geometry.stepover = 2.5;
        p.depth.depth_per_pass = 3.0;
        p.depth.stock_to_leave = stock_to_leave;
        p.geometry.tolerance = 0.3;
        p.depth.stock_top_z = stock_top_z;
        p.clearing_strategy = ClearingStrategy3d::ContourParallel;
        p
    };

    let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
    assert!(tp.moves.len() > 10, "Should produce moves");

    // Use the mesh bbox for the simulation stock (matching internal adaptive3d stock).
    let bbox = &mesh.bbox;
    let r = cutter.radius();
    let sim_x_min = bbox.min.x - r - 0.5;
    let sim_x_max = bbox.max.x + r + 0.5;
    let sim_y_min = bbox.min.y - r - 0.5;
    let sim_y_max = bbox.max.y + r + 0.5;

    let cell_size = 0.3;
    let mut sim_stock = TriDexelStock::from_stock(
        sim_x_min,
        sim_y_min,
        sim_x_max,
        sim_y_max,
        -1.0,
        stock_top_z,
        cell_size,
    );
    let lut = RadialProfileLUT::from_cutter(&cutter, crate::stock::radial_profile::LUT_SAMPLES);
    sim_stock
        .simulate_toolpath_with_lut_cancel(
            &tp,
            &lut,
            tool_radius,
            StockCutDirection::FromTop,
            &|| false,
        )
        .unwrap();

    let grid = &sim_stock.z_grid;
    let margin_cells = (tool_radius / cell_size).ceil() as usize + 2;
    let mut uncleared_count = 0usize;
    let mut total_checked = 0usize;
    let max_excess = 3.5; // allow up to depth_per_pass + tolerance
    let mut worst_excess = 0.0f64;

    for row in margin_cells..grid.rows.saturating_sub(margin_cells) {
        for col in margin_cells..grid.cols.saturating_sub(margin_cells) {
            let x = grid.origin_u + col as f64 * cell_size;
            let y = grid.origin_v + row as f64 * cell_size;
            total_checked += 1;
            if let Some(tz) = grid.top_z_at(row, col) {
                let r_sq = 20.0 * 20.0 - x * x - y * y;
                let surface_z = if r_sq > 0.0 { r_sq.sqrt() } else { 0.0 };
                let expected_max = (surface_z + stock_to_leave + max_excess) as f32;
                if tz > expected_max {
                    uncleared_count += 1;
                    let excess = (tz as f64) - (surface_z + stock_to_leave);
                    if excess > worst_excess {
                        worst_excess = excess;
                    }
                }
            }
        }
    }

    let uncleared_pct = if total_checked > 0 {
        uncleared_count as f64 / total_checked as f64 * 100.0
    } else {
        0.0
    };

    // Allow up to 5% uncleared — remaining cells are near steep/vertical
    // walls at the hemisphere edge where the coarse mesh (16 subdivisions)
    // and tool geometry limit clearing accuracy.
    assert!(
        uncleared_pct < 5.0,
        "Hemisphere contour-parallel should clear >95% of cells, but {:.1}% ({}/{}) have excess material",
        uncleared_pct,
        uncleared_count,
        total_checked,
    );
}

/// Small-DPP island regression: a small hemisphere cap at DPP=0.5
/// should clear cleanly (no unmilled core).
///
/// Pre-fix (May 2026), three filters in `clear_z_level_slice`
/// used DPP-blind area/fraction thresholds. At small DPP the per-level
/// slab contributes few cells, the perimeter sweep erodes islands
/// across many levels, and real islands fall under the threshold and
/// never get milled — leaving visible unmilled cores. The fixes:
///  - `clearing.rs` sub-tool region area threshold scaled with
///    `depth_per_pass / tool_radius` (P44, 2026-11-21: the threshold is
///    removed; the cut-length dial is the one drop rule of a region).
///  - per-level early-exit changed from `fraction < 0.005` to
///    absolute `cells_with_material < MIN_CELLS_TO_CLEAR`.
///  - Z-drop path-split threshold gets a 1mm floor so small DPP
///    doesn't over-split.
///
/// Test dimensions deliberately small (radius 5mm, stock_top_z 6mm,
/// cell_size 0.4) so DPP=0.5 → 12 Z levels rather than 50, keeping
/// runtime comparable to the existing DPP=3.0 hemisphere test even
/// when cargo runs the full suite in parallel.
#[test]
fn test_small_dpp_hemisphere_clears_without_islands() {
    use crate::dexel_stock::StockCutDirection;
    use crate::stock::radial_profile::RadialProfileLUT;

    let radius = 5.0_f64;
    let mesh = crate::mesh::make_test_hemisphere(radius, 12);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = flat_cutter();
    let tool_radius = cutter.radius();
    let stock_top_z = 6.0;
    let stock_to_leave = 0.3;
    let depth_per_pass = 0.5;

    let params = {
        let mut p = default_params();
        p.geometry.tool_radius = tool_radius;
        p.geometry.stepover = 2.0;
        p.depth.depth_per_pass = depth_per_pass;
        p.depth.stock_to_leave = stock_to_leave;
        p.geometry.tolerance = 0.3;
        p.depth.stock_top_z = stock_top_z;
        p.clearing_strategy = ClearingStrategy3d::ContourParallel;
        p
    };

    let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
    assert!(tp.moves.len() > 10, "Should produce moves");

    let bbox = &mesh.bbox;
    let r = cutter.radius();
    let sim_x_min = bbox.min.x - r - 0.5;
    let sim_x_max = bbox.max.x + r + 0.5;
    let sim_y_min = bbox.min.y - r - 0.5;
    let sim_y_max = bbox.max.y + r + 0.5;

    let cell_size = 0.4;
    let mut sim_stock = TriDexelStock::from_stock(
        sim_x_min,
        sim_y_min,
        sim_x_max,
        sim_y_max,
        -1.0,
        stock_top_z,
        cell_size,
    );
    let lut = RadialProfileLUT::from_cutter(&cutter, crate::stock::radial_profile::LUT_SAMPLES);
    sim_stock
        .simulate_toolpath_with_lut_cancel(
            &tp,
            &lut,
            tool_radius,
            StockCutDirection::FromTop,
            &|| false,
        )
        .unwrap();

    let grid = &sim_stock.z_grid;
    let margin_cells = (tool_radius / cell_size).ceil() as usize + 2;
    let max_excess = depth_per_pass + 0.5;
    let mut uncleared_count = 0usize;
    let mut total_checked = 0usize;
    let mut worst_excess = 0.0f64;

    for row in margin_cells..grid.rows.saturating_sub(margin_cells) {
        for col in margin_cells..grid.cols.saturating_sub(margin_cells) {
            let x = grid.origin_u + col as f64 * cell_size;
            let y = grid.origin_v + row as f64 * cell_size;
            total_checked += 1;
            if let Some(tz) = grid.top_z_at(row, col) {
                let r_sq = radius * radius - x * x - y * y;
                let surface_z = if r_sq > 0.0 { r_sq.sqrt() } else { 0.0 };
                let expected_max = (surface_z + stock_to_leave + max_excess) as f32;
                if tz > expected_max {
                    uncleared_count += 1;
                    let excess = (tz as f64) - (surface_z + stock_to_leave);
                    if excess > worst_excess {
                        worst_excess = excess;
                    }
                }
            }
        }
    }

    let uncleared_pct = if total_checked > 0 {
        uncleared_count as f64 / total_checked as f64 * 100.0
    } else {
        0.0
    };

    // Same 5% allowance as the DPP=3.0 hemisphere test. Pre-fix the
    // small-DPP case left a visible un-milled apex core (>15%).
    assert!(
        uncleared_pct < 5.0,
        "Small-DPP hemisphere should clear >95% of cells, but {:.1}% ({}/{}) have excess material (worst {:.2}mm)",
        uncleared_pct,
        uncleared_count,
        total_checked,
        worst_excess,
    );
}

/// Helper: assert that a simulated stock has been fully cleared (for flat surfaces).
fn assert_clearing_complete(
    stock: &TriDexelStock,
    tool_radius: f64,
    cell_size: f64,
    stock_to_leave: f64,
    label: &str,
) {
    let grid = &stock.z_grid;
    let margin_cells = (tool_radius / cell_size).ceil() as usize + 2;
    let z_threshold = (stock_to_leave + 0.5) as f32;
    let mut uncleared_count = 0usize;
    let mut total_checked = 0usize;

    for row in margin_cells..grid.rows.saturating_sub(margin_cells) {
        for col in margin_cells..grid.cols.saturating_sub(margin_cells) {
            total_checked += 1;
            if let Some(tz) = grid.top_z_at(row, col)
                && tz > z_threshold
            {
                uncleared_count += 1;
            }
        }
    }

    let uncleared_pct = if total_checked > 0 {
        uncleared_count as f64 / total_checked as f64 * 100.0
    } else {
        0.0
    };

    assert!(
        uncleared_pct < 1.0,
        "[{label}] Contour-parallel should clear >99% of interior, but {:.1}% ({}/{}) remain above z={:.1}",
        uncleared_pct,
        uncleared_count,
        total_checked,
        z_threshold,
    );
}

// ── Planner ↔ simulator dexel parity ───────────────────────────────
//
// Probe for the wanaka Back Rough Z=10/Z=7 anomaly: the planner
// believes it cleared each Z level (`material_remaining_post → 0`)
// and emits 4585mm of Cut path per level, but a fresh simulator
// replay of the same toolpath against the same initial stock
// reads zero engagement on those passes — meaning the planner's
// INTERNAL stock state diverges from the state the emitted moves
// would actually produce.
//
// These tests run the planner and an independent simulator
// against the same initial dexel and assert cell-by-cell parity
// of the resulting stock tops. They're ignored by default
// (slow + currently-failing) — use:
//   cargo test -p rs_cam_core --lib planner_sim_dexel -- --ignored --nocapture
// for full divergence reports; attach gdb/lldb via
//   cargo test -p rs_cam_core --lib planner_sim_dexel --no-run
// to localize the bug.

/// Outcome of one planner-vs-simulator dexel parity run.
///
/// Was a 5-tuple whose doc comment listed its own fields in the wrong
/// order (`total` and `interior` swapped); named fields now.
struct ParityResult {
    /// Cells whose planner and simulator stock tops differ by more
    /// than the run's tolerance.
    divergent: u64,
    /// …restricted to the interior population.
    interior_divergent: u64,
    /// Size of the interior population — the denominator the interior
    /// bar is stated against. Registered 2026-08-04: before that both
    /// parity tests gated `interior_divergent` against a tenth of the
    /// WHOLE-GRID cell count, which includes the boundary ring the
    /// interior count deliberately excludes. The bar was therefore
    /// looser than it read (W0 §3.4).
    interior_total: u64,
    total_cells: u64,
    /// Planner's stock top is HIGHER: the simulator removed more than
    /// the planner's bookkeeping claims.
    planner_higher: u64,
    /// Simulator's stock top is HIGHER: the planner's bookkeeping
    /// claims material its own emitted toolpath leaves standing. This
    /// is the direction an emitter-side transform missing from the
    /// planner's mirror must produce, and it is the sensitive
    /// instrument for that defect class — see `assert_parity_bars`.
    sim_higher: u64,
    /// …restricted to the interior population. Added SIM w5b: the
    /// whole-grid split above is dominated by the boundary ring, and
    /// the skew bar was reading that ring rather than the defect
    /// class it was written for.
    interior_planner_higher: u64,
    /// …likewise.
    interior_sim_higher: u64,
    /// `sim_higher` cells that fall inside the **F-027 border-clear
    /// zone** — the cells `path.rs` pre-clears in the planner's
    /// internal stock because they sit outside `mesh.bbox ±
    /// cutter.radius() * 0.5` *and* outside the declared world stock
    /// footprint.
    ///
    /// Added W5B-F1: this is the mechanism-identifying counter. When
    /// it equals `sim_higher`, every cell where the planner claims
    /// removal its emitted path does not deliver is a cell the
    /// planner was explicitly told carried no stock — which is not a
    /// stamping-mirror defect at all. See `assert_parity_bars`.
    sim_higher_in_border_clear_zone: u64,
    /// Size of the border-clear zone, the denominator for the above.
    border_clear_zone_total: u64,
    max_dz: f64,
}

impl ParityResult {
    /// How lopsided the divergence is. 1.0 = balanced (consistent
    /// with symmetric discretisation noise); large = one side is
    /// systematically removing material the other does not.
    fn directional_skew(&self) -> f64 {
        let hi = self.planner_higher.max(self.sim_higher) as f64;
        let lo = self.planner_higher.min(self.sim_higher).max(1) as f64;
        hi / lo
    }

    /// The same ratio over the INTERIOR population only — the one the
    /// skew bar is actually stated against since SIM w5b.
    fn interior_directional_skew(&self) -> f64 {
        let hi = self.interior_planner_higher.max(self.interior_sim_higher) as f64;
        let lo = self
            .interior_planner_higher
            .min(self.interior_sim_higher)
            .max(1) as f64;
        hi / lo
    }

    /// Divergent cells outside the interior window, and how they split.
    fn boundary_split(&self) -> (u64, u64) {
        (
            self.planner_higher - self.interior_planner_higher,
            self.sim_higher - self.interior_sim_higher,
        )
    }
}

/// Interior divergence bar, as a percentage of the INTERIOR
/// population (not of the whole grid — see `ParityResult`).
///
/// Re-measured 2026-08-04 against fixed code, per plan rule 7: a
/// changed instrument requires re-measured thresholds, not a copied
/// pin. Old bar: `interior <= total_cells / 10` = 792 on this
/// fixture, against a population of 5300-ish interior cells — i.e. a
/// nominal "10%" that was really ~15% of what it counted, and 79%
/// spent on the day it was granted.
const INTERIOR_DIVERGENCE_BAR_PCT: f64 = 20.0;

/// Directional skew bar. This is the bar that catches the defect
/// class the count misses.
///
/// Pre-registered from measurement either side of the fix (see the
/// re-registration commit body): the skew was 4.80x sim-side on
/// AgentSearch and 4.27x sim-side on ContourParallel before the
/// planner's stamp learned about the emitter's drape, and 1.55x /
/// 1.30x after. ContourParallel's gated interior count went UP across
/// that fix (429 -> 647) while its defect was repaired, which is
/// exactly why the count alone was a weak detector and why this bar
/// exists.
///
/// **SIM w5b: the bar is unchanged at 2.5x; what changed is the
/// population it is stated against.** It used to read the WHOLE grid,
/// which is dominated by the boundary ring, and the swept default made
/// that visible by deleting the other side of the interior
/// disagreement — see [`assert_parity_bars`].
const DIRECTIONAL_SKEW_BAR: f64 = 2.5;

/// Fewest interior divergent cells before the skew ratio means
/// anything.
///
/// A ratio over a handful of cells is not a direction. Under the swept
/// default the interior disagreement is 21/5184 and 8/5184, and
/// `directional_skew`'s `.max(1)` on the low side would turn "21 cells,
/// all one way" into "21x lopsided" and fail a bar written to detect a
/// systematic transform mismatch across hundreds of cells. This is the
/// mirror of the empty-population trap in `CLAUDE.md` ("a gate handed an
/// empty population passes and looks healthy") — here a near-empty
/// population would FAIL and look meaningful. Below this floor the bar
/// abstains and the count bar underneath it is asserted instead, tighter.
const SKEW_MIN_INTERIOR_POPULATION: u64 = 50;

/// What fraction of the interior population may diverge while the skew
/// bar is abstaining. Guards the abstention: "too few to have a
/// direction" must also mean "few".
const ABSTAIN_MAX_INTERIOR_PCT: f64 = 1.0;

/// Boundary-ring one-sided divergence cap, as a fraction of the
/// boundary population.
///
/// Outside the interior window the planner's stock reads lower than
/// the simulator's on 1360/2737 cells (49.7%) for AgentSearch and
/// 568/2737 (20.8%) for ContourParallel, bit-for-bit identical under
/// `whole_path` and under `swept`. That is W5B-F1, and it was pinned
/// at 55% as an unexplained open finding.
///
/// **Root-caused 2026-08-21 (`DELTA_w5b_f1_planner_boundary.md`): it
/// is the F-027 border clear, and it is a fixture artifact.** Every
/// one of those cells — 1360 of 1360 on AgentSearch, 568 of 568 on
/// ContourParallel, zero elsewhere on either strategy — sits in the
/// zone `path.rs` deliberately pre-clears because the cell is outside
/// `mesh.bbox ± cutter.radius() * 0.5` and no world stock footprint
/// was declared to rescue it. The planner is not mis-mirroring its
/// emitter there; it was told there is no stock there and believed
/// it. Re-run with `world_stock_xy_bbox` supplied — the configuration
/// `compute::execute` has always used — and the count is **exactly
/// zero** on both strategies. See
/// `planner_sim_dexel_parity_geodesic_adaptive_world_stock_declared` (the
/// AgentSearch test until removal step R4).
///
/// The cap survives as a growth pin, tightened from 55% to 51% now
/// that the value is understood and deterministic, and it is now the
/// *secondary* bar: `assert_parity_bars` first requires that every
/// such cell be inside the border-clear zone, which is the assertion
/// that would actually catch a mirror defect appearing here.
const BOUNDARY_ONE_SIDED_CAP_PCT: f64 = 51.0;

/// Both bars, applied identically to every strategy so a green
/// sibling can never again be mistaken for a clean one (W0 §4.5).
///
/// # The whole-grid skew bar was masking two divergences, not none
///
/// Measured SIM w5b, both dispatches, same session:
///
/// | | interior planner_higher | interior sim_higher | boundary sim_higher | whole-grid skew |
/// |---|---:|---:|---:|---:|
/// | AgentSearch, `whole_path` | 704 | **0** | 1360 | 1.55x — *passed* |
/// | AgentSearch, `swept` | 21 | **0** | 1360 | 34.87x — failed |
/// | ContourParallel, `whole_path` | 647 | **0** | 568 | 1.30x — *passed* |
/// | ContourParallel, `swept` | 8 | **0** | 568 | 71.00x — failed |
///
/// Interior `sim_higher` is **exactly zero in all four cells of that
/// table**. So there were never two mixed directions inside the
/// window — there were two *separate*, each perfectly one-sided
/// populations: an interior one (simulator over-removing, the old
/// kernel's per-subsegment `f`-blend artifact) and a boundary one
/// (planner over-claiming, invariant under the dispatch). Summed over
/// the whole grid they nearly cancelled, and 1.55x/1.30x is what a
/// cancellation looks like from the outside.
///
/// Restating the bar over the interior population is therefore a
/// **tightening**: under `whole_path` the interior skew reads 704x and
/// 647x against the same 2.5x bar. `RS_CAM_STAMP_DISPATCH=whole_path`
/// no longer runs this pair green, and that is the correct outcome —
/// the old kernel really does disagree with the planner on 704
/// interior cells, all in one direction.
fn assert_parity_bars(r: &ParityResult, label: &str) {
    let interior_bar =
        (r.interior_total as f64 * INTERIOR_DIVERGENCE_BAR_PCT / 100.0).round() as u64;
    assert!(
        r.interior_divergent <= interior_bar,
        "[{label}] planner and simulator dexels diverged on {} of {} INTERIOR cells \
         ({:.1}%, bar {:.1}% = {interior_bar}; whole grid {}, max Δ {:.3}mm). The \
         planner's internal stamping is producing a stock state inconsistent with \
         replaying its own emitted moves — this is the wanaka Back Rough Z=10/Z=7 \
         anomaly. See `stamp_emitted_segment`: it must apply EVERY transformation \
         `segments_to_toolpath` applies.",
        r.interior_divergent,
        r.interior_total,
        r.interior_divergent as f64 / r.interior_total.max(1) as f64 * 100.0,
        INTERIOR_DIVERGENCE_BAR_PCT,
        r.total_cells,
        r.max_dz,
    );
    // SIM w5b: the skew bar reads the INTERIOR population now.
    //
    // It used to read the whole grid and it failed under the swept
    // default — not because anything got worse, but because swept
    // deleted one whole side of the disagreement. `planner_higher`
    // (the simulator removing more than the planner claims) is exactly
    // the boundary-cell over-removal the old kernel's per-subsegment
    // f-blend produced, and it collapsed 880 -> 39 and 741 -> 8.
    // `sim_higher` did not move by a single cell: 1360 and 568 under
    // both dispatches. Interior disagreement fell 33x and 81x.
    //
    // So the bar's own reasoning is sound and its conclusion was
    // inverted here: removing the symmetric noise is what made the
    // asymmetry visible. Widening it would have buried that. Instead
    // it is restated over the population it was written for, and the
    // surviving one-sided divergence gets its own bar below.
    if r.interior_divergent >= SKEW_MIN_INTERIOR_POPULATION {
        assert!(
            r.interior_directional_skew() <= DIRECTIONAL_SKEW_BAR,
            "[{label}] INTERIOR divergence is {:.2}x lopsided (bar \
             {DIRECTIONAL_SKEW_BAR:.2}x): interior planner_higher {} (simulator \
             removed more), interior sim_higher {} (planner's bookkeeping claims \
             more than its emitted path removes), over {} interior divergent cells. \
             Symmetric discretisation noise is balanced; a systematic skew means one \
             side is applying a transformation the other is not.",
            r.interior_directional_skew(),
            r.interior_planner_higher,
            r.interior_sim_higher,
            r.interior_divergent,
        );
    } else {
        // The skew bar ABSTAINS — and says so, rather than passing
        // quietly. Guard the abstention so "too few to have a
        // direction" cannot cover a large divergence.
        let pct = r.interior_divergent as f64 / r.interior_total.max(1) as f64 * 100.0;
        assert!(
            pct <= ABSTAIN_MAX_INTERIOR_PCT,
            "[{label}] the interior skew bar abstained on a population of {} \
             (< {SKEW_MIN_INTERIOR_POPULATION}) but that population is {pct:.2}% of \
             {} interior cells — over the {ABSTAIN_MAX_INTERIOR_PCT:.1}% an abstention \
             is allowed to cover. Either the interior population shrank or the bar's \
             floor is wrong.",
            r.interior_divergent,
            r.interior_total,
        );
        eprintln!(
            "[{label}] interior skew bar ABSTAINS: {} interior divergent cells \
             (planner_higher {}, sim_higher {}) is below the {SKEW_MIN_INTERIOR_POPULATION}-cell \
             floor a direction needs. Interior divergence is {pct:.2}% of {}.",
            r.interior_divergent,
            r.interior_planner_higher,
            r.interior_sim_higher,
            r.interior_total,
        );
    }

    // W5B-F1, root-caused. See `BOUNDARY_ONE_SIDED_CAP_PCT`.
    //
    // PRIMARY bar — mechanism, not magnitude. Every cell where the
    // planner's stock reads lower than the simulator's must be a cell
    // the F-027 border clear was told to empty. A single cell outside
    // that zone is a genuine mirror defect: `stamp_emitted_segment`
    // failing to apply a transformation `segments_to_toolpath`
    // applies. That is the defect class `sim_higher`'s docstring
    // names, and this bar — unlike the percentage cap under it — is
    // sensitive to one cell of it.
    assert_eq!(
        r.sim_higher,
        r.sim_higher_in_border_clear_zone,
        "[{label}] {} of {} sim_higher cells lie OUTSIDE the F-027 border-clear zone \
         ({} cells). Inside that zone the planner reads lower because `path.rs` cleared \
         the ray on purpose (no declared world stock there) — that is W5B-F1 and it is \
         understood. Outside it, the planner is claiming removal its own emitted path \
         does not deliver, which means `stamp_emitted_segment` is missing a \
         transformation `segments_to_toolpath` applies. Do not relax this into a \
         percentage.",
        r.sim_higher - r.sim_higher_in_border_clear_zone,
        r.sim_higher,
        r.border_clear_zone_total,
    );

    // SECONDARY bar — a growth pin on the understood population.
    let boundary_total = r.total_cells.saturating_sub(r.interior_total);
    let (boundary_planner_higher, boundary_sim_higher) = r.boundary_split();
    let boundary_pct = boundary_sim_higher as f64 / boundary_total.max(1) as f64 * 100.0;
    assert!(
        boundary_pct <= BOUNDARY_ONE_SIDED_CAP_PCT,
        "[{label}] W5B-F1 grew: {boundary_sim_higher} of {boundary_total} \
         boundary-ring cells ({boundary_pct:.1}%, cap {BOUNDARY_ONE_SIDED_CAP_PCT:.1}%) \
         have the planner reading lower than the simulator (boundary planner_higher \
         {boundary_planner_higher}, for contrast). The mechanism is the F-027 border \
         clear over a fixture that declares no world stock footprint; if this grew, \
         either the border-clear predicate moved or the fixture's grid did. Do not \
         raise this cap; see DELTA_w5b_f1_planner_boundary.md. Whole-grid skew for \
         reference: {:.2}x — the figure this bar used to be stated against, and the one \
         that read a comfortable 1.55x/1.30x while both populations were maximally \
         one-sided.",
        r.directional_skew(),
    );
}

fn run_planner_sim_parity(strategy: ClearingStrategy3d, label: &str) -> ParityResult {
    run_planner_sim_parity_with_mesh(strategy, label, make_hemisphere_mesh(), None)
}

fn run_planner_sim_parity_flat(strategy: ClearingStrategy3d, label: &str) -> ParityResult {
    run_planner_sim_parity_with_mesh(strategy, label, make_flat_mesh(), None)
}

/// `world_xy` is forwarded to [`Adaptive3dParams::world_stock_xy_bbox`].
/// The historical call sites pass `None`, which is what makes the
/// F-027 border clear in `path.rs` fire over this fixture's whole
/// off-mesh rim; production (`compute/execute.rs`) always passes
/// `Some`. Set `RS_CAM_PARITY_MAP=1` to dump an ASCII cell map.
fn run_planner_sim_parity_with_mesh(
    strategy: ClearingStrategy3d,
    label: &str,
    mesh_pair: (TriangleMesh, SpatialIndex),
    world_xy: Option<(f64, f64, f64, f64)>,
) -> ParityResult {
    let (mesh, si) = mesh_pair;
    let mesh_bbox_for_interior = mesh.bbox;
    let cutter = flat_cutter();
    let r = cutter.radius();
    let bbox = mesh.bbox;
    let origin_x = bbox.min.x - r;
    let origin_y = bbox.min.y - r;
    let extent_x = bbox.max.x + r;
    let extent_y = bbox.max.y + r;
    let tool_radius = 3.175_f64;
    let stock_top_z = 25.0_f64;
    let cell_size = (tool_radius / 6.0).max(0.1);

    let initial_stock = TriDexelStock::from_stock(
        origin_x,
        origin_y,
        extent_x,
        extent_y,
        bbox.min.z,
        stock_top_z,
        cell_size,
    );

    let params = {
        let mut p = default_params();
        p.initial_stock = Some(initial_stock.clone());
        p.clearing_strategy = strategy;
        p.geometry.tool_radius = tool_radius;
        p.geometry.envelope_radius = tool_radius;
        p.depth.stock_top_z = stock_top_z;
        p.depth.depth_per_pass = 3.0;
        p.depth.stock_to_leave = 0.5;
        p.geometry.stepover = 1.0;
        p.geometry.tolerance = 0.5;
        p.geometry.world_stock_xy_bbox = world_xy;
        p
    };

    let never_cancel = || false;
    let result = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
        .expect("planner should succeed");
    let planner_stock = result.final_material_stock;
    let surface_hm = result.surface_heightmap;
    let segments = result.segments;
    let (toolpath, _) = segments_to_toolpath(&segments, &params, &mesh, &si, &cutter);

    let mut sim_stock = initial_stock;
    sim_stock
        .simulate_toolpath_with_metrics_with_cancel(
            &toolpath,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            &[],
            true,
            &never_cancel,
        )
        .expect("simulator should succeed");

    // Cell-by-cell ray-top comparison. Tolerance = cell_size
    // (sub-cell stamping noise is OK; bigger gaps are real bugs).
    let tol_mm = cell_size;
    let grid = &planner_stock.z_grid;
    let total_cells = (grid.rows * grid.cols) as u64;
    let mut divergent = 0u64;
    let mut interior_divergent = 0u64;
    let mut interior_total = 0u64;
    let mut planner_higher = 0u64; // sim removed more
    let mut sim_higher = 0u64; // planner removed more
    let mut interior_planner_higher = 0u64;
    let mut interior_sim_higher = 0u64;
    let mut sim_higher_in_border_clear_zone = 0u64;
    let mut border_clear_zone_total = 0u64;
    let mut max_dz = 0.0_f64;
    let mut violations: Vec<(usize, usize, f64, f64, f64, f64)> = Vec::new();
    let interior_x_lo = mesh_bbox_for_interior.min.x + 1.0;
    let interior_x_hi = mesh_bbox_for_interior.max.x - 1.0;
    let interior_y_lo = mesh_bbox_for_interior.min.y + 1.0;
    let interior_y_hi = mesh_bbox_for_interior.max.y - 1.0;
    // W5B-F1: mirror of the F-027 border clear in `path.rs`. A cell is
    // in the zone when it is outside `mesh.bbox ± cutter.radius() * 0.5`
    // AND not rescued by a declared world stock footprint — exactly the
    // two conditions `path.rs` tests before it calls
    // `ray_subtract_above` on the planner's ray.
    let border_margin = r * 0.5;
    let dump_map = std::env::var("RS_CAM_PARITY_MAP").is_ok();
    let mut map: Vec<String> = Vec::new();
    for row in 0..grid.rows {
        let mut map_line = String::new();
        for col in 0..grid.cols {
            let (x, y) = grid.cell_to_world(row, col);
            let outside_mesh = x < bbox.min.x - border_margin
                || x > bbox.max.x + border_margin
                || y < bbox.min.y - border_margin
                || y > bbox.max.y + border_margin;
            let rescued_by_world_stock = world_xy
                .is_some_and(|(wx0, wy0, wx1, wy1)| x >= wx0 && x <= wx1 && y >= wy0 && y <= wy1);
            let in_border_clear_zone = outside_mesh && !rescued_by_world_stock;
            if in_border_clear_zone {
                border_clear_zone_total += 1;
            }
            let is_interior =
                x > interior_x_lo && x < interior_x_hi && y > interior_y_lo && y < interior_y_hi;
            if is_interior {
                // Counted for EVERY interior cell, divergent or not:
                // this is the denominator the interior bar is stated
                // against.
                interior_total += 1;
            }
            let p = stock_top_z_at(&planner_stock, row, col);
            let s = stock_top_z_at(&sim_stock, row, col);
            let dz = (p - s).abs();
            if dump_map {
                map_line.push(match (dz > tol_mm, p > s, in_border_clear_zone) {
                    (false, _, _) => '.',
                    (true, true, _) => 'p',
                    (true, false, true) => 'S',
                    (true, false, false) => 's',
                });
            }
            if dz > tol_mm {
                divergent += 1;
                max_dz = max_dz.max(dz);
                if p > s + tol_mm {
                    planner_higher += 1;
                    if is_interior {
                        interior_planner_higher += 1;
                    }
                } else if s > p + tol_mm {
                    sim_higher += 1;
                    if is_interior {
                        interior_sim_higher += 1;
                    }
                    if in_border_clear_zone {
                        sim_higher_in_border_clear_zone += 1;
                    }
                }
                if is_interior {
                    interior_divergent += 1;
                    // Only collect INTERIOR violations — boundary
                    // pre-clear noise drowns out the real bug
                    // otherwise.
                    if violations.len() < 20 {
                        let i = row * grid.cols + col;
                        let surf = surface_hm.z_or_bbox_floor_values()[i];
                        violations.push((row, col, p, s, surf, dz));
                    }
                }
            }
        }
        if dump_map {
            map.push(map_line);
        }
    }

    eprintln!(
        "[{label}] PARITY: {divergent}/{total_cells} cells differ > {tol_mm:.2}mm; \
         interior {interior_divergent}/{interior_total}; planner_higher {planner_higher} \
         (sim removed more, interior {interior_planner_higher}); sim_higher \
         {sim_higher} (planner removed more, interior {interior_sim_higher}); \
         max dz {max_dz:.3}mm",
    );
    eprintln!(
        "[{label}] W5B-F1: {sim_higher_in_border_clear_zone}/{sim_higher} sim_higher cells \
         lie in the F-027 border-clear zone ({border_clear_zone_total} cells, \
         world_stock_xy_bbox {})",
        if world_xy.is_some() {
            "supplied"
        } else {
            "ABSENT"
        },
    );
    if dump_map {
        eprintln!(
            "[{label}] map ('.' agree, 'p' planner_higher, 'S' sim_higher in border-clear \
             zone, 's' sim_higher elsewhere):"
        );
        for line in &map {
            eprintln!("{line}");
        }
    }
    for (row, col, p, s, surf, dz) in &violations {
        let (x, y) = grid.cell_to_world(*row, *col);
        eprintln!(
            "  ({row:>3}, {col:>3}) world ({x:>6.2}, {y:>6.2}) surface {surf:>6.2}: planner top {p:>6.2}  sim top {s:>6.2}  Δ {dz:>5.2}mm",
        );
    }

    ParityResult {
        divergent,
        interior_divergent,
        interior_total,
        total_cells,
        planner_higher,
        sim_higher,
        interior_planner_higher,
        interior_sim_higher,
        sim_higher_in_border_clear_zone,
        border_clear_zone_total,
        max_dz,
    }
}

#[test]
#[ignore = "Probe: emitted Cut path point spacing on GeodesicAdaptive flat — distinguishes \
            spacing-too-coarse vs stamp-at-wrong-location"]
fn geodesic_adaptive_cut_path_point_spacing_probe() {
    let (mesh, si) = make_flat_mesh();
    let cutter = flat_cutter();
    let tool_radius = 3.175_f64;
    let stock_top_z = 25.0_f64;
    let cell_size = (tool_radius / 6.0).max(0.1);
    let r = cutter.radius();
    let bbox = mesh.bbox;
    let initial_stock = TriDexelStock::from_stock(
        bbox.min.x - r,
        bbox.min.y - r,
        bbox.max.x + r,
        bbox.max.y + r,
        bbox.min.z,
        stock_top_z,
        cell_size,
    );
    let params = {
        let mut p = default_params();
        p.initial_stock = Some(initial_stock);
        p.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
        p.geometry.tool_radius = tool_radius;
        p.geometry.envelope_radius = tool_radius;
        p.depth.stock_top_z = stock_top_z;
        p.depth.depth_per_pass = 3.0;
        p.depth.stock_to_leave = 0.5;
        p.geometry.stepover = 1.0;
        p.geometry.tolerance = 0.5;
        p
    };

    let never_cancel = || false;
    let result = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
        .expect("planner should succeed");
    let segments = result.segments;

    // Histogram bins for consecutive-point planar spacing in units of
    // tool_radius. The planner's stamp uses a point stamp at each
    // sub-sampled location with `step_len = cell_size * 1.5` ≈
    // tool_radius * 0.25. If consecutive Cut path points are
    // > 2 * tool_radius apart there's a real gap between point
    // stamps that the simulator's swept stamp would fill.
    let mut bins = [0u64; 8]; // 0..0.25R, ..0.5R, ..1R, ..2R, ..4R, ..8R, ..16R, >16R
    let bin_thresholds = [0.25_f64, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, f64::INFINITY];
    let mut total_gaps = 0u64;
    let mut max_gap_mm = 0.0_f64;
    let mut max_gap_loc: Option<(P3, P3)> = None;
    let mut total_path_points = 0u64;
    let mut cut_segments_seen = 0u64;
    for seg in &segments {
        if let Adaptive3dSegment::Cut(path) = seg {
            cut_segments_seen += 1;
            total_path_points += path.len() as u64;
            for pair in path.windows(2) {
                if let [a, b] = pair {
                    let dx = b.x - a.x;
                    let dy = b.y - a.y;
                    let gap_mm = (dx * dx + dy * dy).sqrt();
                    if gap_mm > max_gap_mm {
                        max_gap_mm = gap_mm;
                        max_gap_loc = Some((*a, *b));
                    }
                    total_gaps += 1;
                    let gap_in_radii = gap_mm / tool_radius;
                    for (i, &thr) in bin_thresholds.iter().enumerate() {
                        if gap_in_radii < thr {
                            bins[i] += 1;
                            break;
                        }
                    }
                }
            }
        }
    }

    eprintln!(
        "GeodesicAdaptive flat-plane Cut-path spacing probe (tool_radius = {tool_radius:.3} mm):"
    );
    eprintln!(
        "  Cut segments: {cut_segments_seen}, total path points: {total_path_points}, total gaps: {total_gaps}"
    );
    eprintln!(
        "  Max consecutive-point gap: {max_gap_mm:.3} mm = {:.2}× tool_radius",
        max_gap_mm / tool_radius
    );
    if let Some((a, b)) = max_gap_loc {
        eprintln!(
            "    from ({:.2}, {:.2}, {:.2}) → ({:.2}, {:.2}, {:.2})",
            a.x, a.y, a.z, b.x, b.y, b.z
        );
    }
    eprintln!("  Gap distribution (in units of tool_radius):");
    let bin_labels = [
        "  < 0.25R",
        " < 0.5R ",
        " < 1.0R ",
        " < 2.0R ",
        " < 4.0R ",
        " < 8.0R ",
        " < 16.0R",
        " >= 16R ",
    ];
    for (label, count) in bin_labels.iter().zip(bins.iter()) {
        let pct = if total_gaps > 0 {
            *count as f64 / total_gaps as f64 * 100.0
        } else {
            0.0
        };
        eprintln!("    {label}: {count:>6}  ({pct:>5.1}%)");
    }
    let large_gaps = bins[3] + bins[4] + bins[5] + bins[6] + bins[7];
    let large_pct = if total_gaps > 0 {
        large_gaps as f64 / total_gaps as f64 * 100.0
    } else {
        0.0
    };
    eprintln!(
        "  Gaps > 1.0× tool_radius (point-stamp leaves bigger holes than radius): {large_gaps} ({large_pct:.1}%)"
    );
}

#[test]
#[ignore = "Hypothesis 1: pure-flat mesh — should reveal whether the divergence is surface-coupled"]
fn planner_sim_dexel_parity_flat_geodesic_adaptive() {
    let r = run_planner_sim_parity_flat(
        ClearingStrategy3d::GeodesicAdaptive,
        "GeodesicAdaptive flat",
    );
    eprintln!(
        "FLAT GeodesicAdaptive: {}/{} cells diverge (max Δ {:.3}mm)",
        r.divergent, r.total_cells, r.max_dz
    );
    // No assertion — diagnostic output. The hemisphere variants
    // assert; this test just prints so we can compare flat vs
    // hemisphere divergence rates side-by-side.
}

#[test]
#[ignore = "Hypothesis 1: pure-flat mesh — should reveal whether the divergence is surface-coupled"]
fn planner_sim_dexel_parity_flat_contour_parallel() {
    let r =
        run_planner_sim_parity_flat(ClearingStrategy3d::ContourParallel, "ContourParallel flat");
    eprintln!(
        "FLAT ContourParallel: {}/{} cells diverge (max Δ {:.3}mm)",
        r.divergent, r.total_cells, r.max_dz
    );
}

/// Probe A — Bug 2 isolation. Build a toolpath that contains ONLY
/// the Cut segments (skipping Rapid/Link/Marker), so the simulator
/// only stamps cells the planner also stamped. If the divergence
/// drops to near-zero on this filtered toolpath, the original
/// divergence is dominated by Rapid/Link-derived feed moves
/// (peck-plunges, stay-down links) which the simulator stamps but
/// the planner does not.
fn segments_to_cut_only_toolpath(segments: &[Adaptive3dSegment]) -> Toolpath {
    let mut tp = Toolpath::new();
    for seg in segments {
        if let Adaptive3dSegment::Cut(path) = seg
            && let Some(first) = path.first()
        {
            tp.rapid_to(*first);
            for p in path.iter().skip(1) {
                tp.feed_to(*p, 1000.0);
            }
        }
    }
    tp
}

fn run_planner_sim_parity_cut_only(
    strategy: ClearingStrategy3d,
    label: &str,
    mesh_pair: (TriangleMesh, SpatialIndex),
) -> (u64, u64, u64) {
    let (mesh, si) = mesh_pair;
    let mesh_bbox = mesh.bbox;
    let cutter = flat_cutter();
    let r = cutter.radius();
    let tool_radius = 3.175_f64;
    let stock_top_z = 25.0_f64;
    let cell_size = (tool_radius / 6.0).max(0.1);
    let initial_stock = TriDexelStock::from_stock(
        mesh_bbox.min.x - r,
        mesh_bbox.min.y - r,
        mesh_bbox.max.x + r,
        mesh_bbox.max.y + r,
        mesh_bbox.min.z,
        stock_top_z,
        cell_size,
    );

    let params = {
        let mut p = default_params();
        p.initial_stock = Some(initial_stock.clone());
        p.clearing_strategy = strategy;
        p.geometry.tool_radius = tool_radius;
        p.geometry.envelope_radius = tool_radius;
        p.depth.stock_top_z = stock_top_z;
        p.depth.depth_per_pass = 3.0;
        p.depth.stock_to_leave = 0.5;
        p.geometry.stepover = 1.0;
        p.geometry.tolerance = 0.5;
        p
    };

    let never_cancel = || false;
    let result = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
        .expect("planner should succeed");
    let planner_stock = result.final_material_stock;
    let segments = result.segments;

    // CUT-ONLY toolpath: skip Rapid/Link/Marker entirely.
    let cut_only_toolpath = segments_to_cut_only_toolpath(&segments);

    let mut sim_stock = initial_stock;
    sim_stock
        .simulate_toolpath_with_metrics_with_cancel(
            &cut_only_toolpath,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            &[],
            true,
            &never_cancel,
        )
        .expect("simulator should succeed");

    let tol_mm = cell_size;
    let grid = &planner_stock.z_grid;
    let total = (grid.rows * grid.cols) as u64;
    let mut divergent = 0u64;
    let mut interior = 0u64;
    let interior_x_lo = mesh_bbox.min.x + 1.0;
    let interior_x_hi = mesh_bbox.max.x - 1.0;
    let interior_y_lo = mesh_bbox.min.y + 1.0;
    let interior_y_hi = mesh_bbox.max.y - 1.0;
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let p = stock_top_z_at(&planner_stock, row, col);
            let s = stock_top_z_at(&sim_stock, row, col);
            if (p - s).abs() > tol_mm {
                divergent += 1;
                let (x, y) = grid.cell_to_world(row, col);
                if x > interior_x_lo && x < interior_x_hi && y > interior_y_lo && y < interior_y_hi
                {
                    interior += 1;
                }
            }
        }
    }
    eprintln!("[{label} CUT-ONLY] {divergent}/{total} cells diverge; interior {interior}",);
    (divergent, total, interior)
}

#[test]
#[ignore = "Probe A: Cut-only toolpath isolates Bug 2 (Rapid/Link/peck-plunge stamping)"]
fn planner_sim_parity_cut_only_geodesic_adaptive_flat() {
    let _ = run_planner_sim_parity_cut_only(
        ClearingStrategy3d::GeodesicAdaptive,
        "GeodesicAdaptive flat",
        make_flat_mesh(),
    );
}

#[test]
#[ignore = "Probe A: Cut-only toolpath isolates Bug 2 (Rapid/Link/peck-plunge stamping)"]
fn planner_sim_parity_cut_only_geodesic_adaptive_hemisphere() {
    let _ = run_planner_sim_parity_cut_only(
        ClearingStrategy3d::GeodesicAdaptive,
        "GeodesicAdaptive hemisphere",
        make_hemisphere_mesh(),
    );
}

#[test]
#[ignore = "Probe A: Cut-only toolpath isolates Bug 2 (Rapid/Link/peck-plunge stamping)"]
fn planner_sim_parity_cut_only_contour_parallel_flat() {
    let _ = run_planner_sim_parity_cut_only(
        ClearingStrategy3d::ContourParallel,
        "ContourParallel flat",
        make_flat_mesh(),
    );
}

#[test]
#[ignore = "Probe A: Cut-only toolpath isolates Bug 2 (Rapid/Link/peck-plunge stamping)"]
fn planner_sim_parity_cut_only_contour_parallel_hemisphere() {
    let _ = run_planner_sim_parity_cut_only(
        ClearingStrategy3d::ContourParallel,
        "ContourParallel hemisphere",
        make_hemisphere_mesh(),
    );
}

/// Boundary divergence (cells outside the mesh footprint) is the
/// F-027 border clear, not a stamping defect — see
/// `BOUNDARY_ONE_SIDED_CAP_PCT` and the mechanism assertion in
/// `assert_parity_bars`. What these tests guard is INSIDE-the-mesh
/// stamping consistency.
///
/// Residual interior divergence that both bars deliberately tolerate:
/// (a) F.a sub-cell blend drift — the simulator subdivides each
/// emitted segment at `sample_step_mm` and stamps each subsegment
/// separately while the planner stamps whole segments, so cells
/// straddling a subsegment boundary see compound `(1-f₁)(1-f₂)`
/// blends that under-saturate versus the single whole-segment `f`
/// (DEXEL_Z_ONLY_INVESTIGATION.md §6.F / §8 Step 4 predicts exactly
/// this); (b) was: `Cut` segments whose first emitted feed sweeps from
/// the emitter's true tool position rather than from the planner's raw
/// `last_pos`. G-PHANTOMSTAMP (2026-09-25) fixed (b): the mirror now
/// stamps the first feed from `PlannerCursor::tool_pos`. (a) is not
/// directional, which is why the skew bar can be tight while the count
/// bar cannot.
///
/// W5B-F1, 2026-08-21: (b) was the standing candidate for the
/// boundary-ring over-claim and it is **not** the cause — the
/// boundary population is 100% F-027 border clear, and (b) would
/// produce cells at cut-segment starts, which is not where any of
/// them are. (b) remains a live candidate for part of the 21/8
/// interior `planner_higher` cells; it has never been measured
/// separately from (a).
#[test]
fn planner_sim_dexel_parity_geodesic_adaptive() {
    let r = run_planner_sim_parity(
        ClearingStrategy3d::GeodesicAdaptive,
        "GeodesicAdaptive hemisphere",
    );
    assert_parity_bars(&r, "GeodesicAdaptive hemisphere");
}

/// The same two bars as `planner_sim_dexel_parity_geodesic_adaptive`, and
/// deliberately not a weaker set. This test was green throughout the
/// seven weeks its sibling was red while carrying the identical
/// defect at the identical 25 mm `max dz` — its old bar gated an
/// interior count against a tenth of the whole-grid count and was
/// only 54% spent, so it never reported. If one of the pair fails and
/// the other does not, the divergence is in that strategy's stamping
/// path — still a useful first bisection.
#[test]
fn planner_sim_dexel_parity_contour_parallel() {
    let r = run_planner_sim_parity(
        ClearingStrategy3d::ContourParallel,
        "ContourParallel hemisphere",
    );
    assert_parity_bars(&r, "ContourParallel hemisphere");
}

/// W5B-F1's decisive control, and the only parity pair that runs the
/// configuration the product actually ships.
///
/// `compute::execute::execute_operation` has supplied
/// `world_stock_xy_bbox` on every adaptive3d call since F-027; the two
/// tests above, and every other unit-test call site, leave it `None`.
/// That single difference is the whole of W5B-F1: with the stock
/// footprint declared, `path.rs` stops border-clearing the rim, the
/// planner plans passes over it instead, and `sim_higher` — the
/// planner claiming removal its emitted path does not deliver — is
/// **exactly zero over the whole grid**, on both strategies, where it
/// was 1360 and 568.
///
/// The bar is zero, not a percentage, deliberately: in the shipped
/// configuration there is no known mechanism that produces even one
/// such cell, so any is news. `world_xy` is set far wider than the
/// grid so every cell is declared stock — the fixture hands the
/// planner an `initial_stock` covering `mesh.bbox ± r`, and this is
/// what telling it so looks like.
fn assert_no_over_claim_with_world_stock(strategy: ClearingStrategy3d, label: &str) {
    let r = run_planner_sim_parity_with_mesh(
        strategy,
        label,
        make_hemisphere_mesh(),
        Some((-1.0e4, -1.0e4, 1.0e4, 1.0e4)),
    );
    assert_eq!(
        r.border_clear_zone_total, 0,
        "[{label}] the world stock footprint should rescue every cell from the F-027 \
         border clear; {} cells were still in the zone, so this control is not testing \
         what it claims to.",
        r.border_clear_zone_total,
    );
    assert_eq!(
        r.sim_higher, 0,
        "[{label}] with the world stock footprint declared — the configuration \
         `compute::execute` ships — the planner claimed removal its emitted path does \
         not deliver on {} of {} cells. This was zero when W5B-F1 was root-caused \
         (2026-08-21); it is the bar that would catch a real \
         `stamp_emitted_segment`/`segments_to_toolpath` mirror divergence, with none of \
         the border-clear population masking it.",
        r.sim_higher, r.total_cells,
    );
    assert_parity_bars(&r, label);
}

#[test]
fn planner_sim_dexel_parity_geodesic_adaptive_world_stock_declared() {
    assert_no_over_claim_with_world_stock(
        ClearingStrategy3d::GeodesicAdaptive,
        "GeodesicAdaptive hemisphere / world stock declared",
    );
}

#[test]
fn planner_sim_dexel_parity_contour_parallel_world_stock_declared() {
    assert_no_over_claim_with_world_stock(
        ClearingStrategy3d::ContourParallel,
        "ContourParallel hemisphere / world stock declared",
    );
}

/// G-LADDERANCHOR (plan Phase 2c): on prior stock the levels start at the
/// top of the material that the operation can still cut, not at the stock
/// box top. On fresh stock they start at `stock_top_z`.
#[test]
fn level_anchor_reads_the_top_of_the_prior_stock() {
    let (mesh, si) = make_flat_mesh(); // 50x50mm flat at z=0
    let cutter = flat_cutter();
    let cell_size = 0.5;
    let mut stock = TriDexelStock::from_stock(-20.0, -20.0, 20.0, 20.0, -1.0, 14.0, cell_size);
    let hm = SurfaceHeightmap::from_mesh(
        &mesh,
        &si,
        &cutter,
        stock.z_grid.origin_u,
        stock.z_grid.origin_v,
        stock.z_grid.rows,
        stock.z_grid.cols,
        cell_size,
        -1.0,
    );
    let anchor = |stock: &TriDexelStock, prior: bool| {
        super::path::level_anchor_z(stock, &hm, 0.5, 14.0, prior)
    };
    // Fresh stock: the box top, whatever the stock holds.
    assert_eq!(anchor(&stock, false), 14.0);
    assert_eq!(anchor(&stock, true), 14.0);
    // A Face took the stock to Z 12: the levels start at Z 12.
    for row in 0..stock.z_grid.rows {
        for col in 0..stock.z_grid.cols {
            crate::stock::dexel::ray_subtract_above(stock.z_grid.ray_mut(row, col), 12.0);
        }
    }
    assert!((anchor(&stock, true) - 12.0).abs() < 1e-6);
    assert_eq!(anchor(&stock, false), 14.0);
    // Nothing left above the leave: the anchor falls back to the box top.
    for row in 0..stock.z_grid.rows {
        for col in 0..stock.z_grid.cols {
            crate::stock::dexel::ray_subtract_above(stock.z_grid.ray_mut(row, col), 0.5);
        }
    }
    assert_eq!(anchor(&stock, true), 14.0);
}

/// D50: the rest top may lower the first level only while that level cuts
/// no more than one Depth/Pass where the tool centre goes. A skin of 1.5
/// over the leave is too thick for a Depth/Pass of 1.0 and thin enough for
/// 2.0. A thick skin outside the centre containment does not count.
#[test]
fn a_rest_ceiling_keeps_one_depth_per_pass_d50() {
    let (mesh, si) = make_flat_mesh(); // 50x50mm flat at z=0
    let cutter = flat_cutter();
    let cell_size = 0.5;
    let mut stock = TriDexelStock::from_stock(-20.0, -20.0, 20.0, 20.0, -1.0, 14.0, cell_size);
    let hm = SurfaceHeightmap::from_mesh(
        &mesh,
        &si,
        &cutter,
        stock.z_grid.origin_u,
        stock.z_grid.origin_v,
        stock.z_grid.rows,
        stock.z_grid.cols,
        cell_size,
        -1.0,
    );
    let leave = 0.5_f64;
    let skin_top = leave + 1.5;
    // The prior stock: a skin over the leave inside x < 0, a thick block
    // (the box top) at x >= 0.
    for row in 0..stock.z_grid.rows {
        for col in 0..stock.z_grid.cols {
            let (x, _) = stock.z_grid.cell_to_world(row, col);
            if x < 0.0 {
                crate::stock::dexel::ray_subtract_above(
                    stock.z_grid.ray_mut(row, col),
                    skin_top as f32,
                );
            }
        }
    }
    let params_with = |dpp: f64, centre: Vec<crate::polygon::Polygon2>| {
        let mut p = default_params();
        p.depth.stock_to_leave = leave;
        p.depth.depth_per_pass = dpp;
        p.geometry.centre_boundary = centre;
        p
    };
    let skin_side = vec![crate::polygon::Polygon2::rectangle(
        -15.0, -15.0, -5.0, 15.0,
    )];
    // The first level under the skin: the cell floor is the leave.
    let first = 0.0;
    assert!(!super::path::rest_ceiling_keeps_doc(
        &stock,
        &hm,
        &params_with(1.0, skin_side.clone()),
        first
    ));
    assert!(super::path::rest_ceiling_keeps_doc(
        &stock,
        &hm,
        &params_with(2.0, skin_side),
        first
    ));
    // No containment: every cell counts, the block too.
    assert!(!super::path::rest_ceiling_keeps_doc(
        &stock,
        &hm,
        &params_with(2.0, Vec::new()),
        first
    ));
}

// ── By Area region map ───────────────────────────────────────────

/// A heightfield mesh on a 1 mm grid over `[0, w] x [0, h]`: Z = 10 on the
/// top face, Z = 0 inside each `(x0, y0, x1, y1)` pocket.
fn make_pocket_plate(w: usize, h: usize, pockets: &[(f64, f64, f64, f64)]) -> TriangleMesh {
    let z_at = |x: f64, y: f64| -> f64 {
        if pockets
            .iter()
            .any(|&(x0, y0, x1, y1)| x >= x0 && x <= x1 && y >= y0 && y <= y1)
        {
            0.0
        } else {
            10.0
        }
    };
    let mut vertices = Vec::with_capacity((w + 1) * (h + 1));
    for j in 0..=h {
        for i in 0..=w {
            let (x, y) = (i as f64, j as f64);
            vertices.push(P3::new(x, y, z_at(x, y)));
        }
    }
    let mut triangles = Vec::with_capacity(w * h * 2);
    let stride = (w + 1) as u32;
    for j in 0..h as u32 {
        for i in 0..w as u32 {
            let a = j * stride + i;
            let b = a + 1;
            let c = a + stride + 1;
            let d = a + stride;
            triangles.push([a, b, c]);
            triangles.push([a, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// The By Area planner records its detection. Two pockets under a plate
/// whose top is the stock top are two regions; the larger one is region 1;
/// each region's span carries its order as the `region_id`; the moves of
/// region 1 stay in the region 1 box.
#[test]
fn by_area_exports_two_pocket_region_map() {
    use crate::trace::toolpath_spans::{SpanKind, SpanPayload};

    // Pocket A (30 x 30 mm) is larger than pocket B (25 x 20 mm).
    let mesh = make_pocket_plate(80, 40, &[(5.0, 5.0, 35.0, 35.0), (45.0, 10.0, 70.0, 30.0)]);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = flat_cutter();
    let mut params = default_params();
    params.depth.stock_top_z = 10.0;
    params.depth.depth_per_pass = 4.0;
    params.linking.region_ordering = RegionOrdering::ByArea;

    let out =
        adaptive_3d_toolpath_output_with_cancel(&mesh, &si, &cutter, &params, &|| false, None)
            .unwrap();
    let map = out
        .produced
        .area_regions
        .as_ref()
        .expect("By Area records its regions");
    assert_eq!(map.regions.len(), 2, "two pockets are two regions");
    assert_eq!(map.labels.len(), map.rows * map.cols);
    let (r1, r2) = (&map.regions[0], &map.regions[1]);
    assert_eq!((r1.order, r2.order), (1, 2));
    assert!(r1.cell_count > r2.cell_count, "the larger region is first");
    // Region 1 is pocket A (x < 40), region 2 is pocket B (x > 40).
    assert!(r1.bbox_xy[2] < 40.0, "region 1 box {:?}", r1.bbox_xy);
    assert!(r2.bbox_xy[0] > 40.0, "region 2 box {:?}", r2.bbox_xy);
    for r in &map.regions {
        let cells = map.labels.iter().filter(|&&l| l == r.order).count();
        assert_eq!(cells, r.cell_count, "label count of region {}", r.order);
        assert!(r.level_count > 0, "region {} has levels", r.order);
        let [z_top, z_bottom] = r.level_z_range.unwrap();
        assert!(z_top >= z_bottom);
    }
    let run_cells: usize = map
        .row_runs()
        .iter()
        .map(|run| run.col_end - run.col_start)
        .sum();
    assert_eq!(run_cells, r1.cell_count + r2.cell_count);

    // The spans name the same orders, in the same sequence.
    let spans = crate::compute::spans::spans_from_adaptive3d_annotations(
        &out.annotations,
        out.toolpath.moves.len(),
    );
    let region_spans: Vec<_> = spans
        .iter()
        .filter(|s| s.kind == SpanKind::Region && !s.is_boundary())
        .collect();
    let ids: Vec<u32> = region_spans
        .iter()
        .filter_map(|s| match &s.payload {
            Some(SpanPayload::Region { region_id, .. }) => Some(*region_id),
            _ => None,
        })
        .collect();
    assert_eq!(ids, vec![1, 2]);
    assert_eq!(&*region_spans[0].label, "Adaptive region 1");

    // Region 1's feed moves stay in the region 1 box plus one tool radius.
    let slack = params.geometry.tool_radius + 0.5;
    let first = region_spans[0];
    for m in &out.toolpath.moves[first.start_move..first.end_move] {
        if m.move_type == crate::toolpath::MoveType::Rapid {
            continue;
        }
        assert!(
            m.target.x <= r1.bbox_xy[2] + slack,
            "a region 1 move at x {:.2} is outside the region 1 box",
            m.target.x
        );
    }

    // Global ordering records no regions.
    params.linking.region_ordering = RegionOrdering::Global;
    let global =
        adaptive_3d_toolpath_output_with_cancel(&mesh, &si, &cutter, &params, &|| false, None)
            .unwrap();
    assert!(global.produced.area_regions.is_none());
}

/// Integration step 7.2 (`StepSplit::StockDepth`): the step reads on a
/// planted stock. A move one Depth/Pass deep through fresh stock reads one
/// full pass (load 1, its first sample a full slot of the one load
/// `chip_load_fraction`) and its depth; the same move over a layer `t`
/// thinner than a pass reads `t / DPP` and `t`, under the pass ceiling, so
/// the stock-read rule cuts it at feed. The move has an XY length; a move
/// with none reads load 0.
#[test]
fn the_step_reads_tell_a_full_pass_from_a_thin_layer() {
    use super::clearing::{StepLoadCutter, step_axial_depth, step_load};
    const CELL: f64 = 0.5;
    const DPP: f64 = 6.0;
    const TOP: f64 = 10.0;
    const LAYER: f64 = 0.5;
    let cutter = FlatEndmill::new(6.0, 25.0);
    let lut = RadialProfileLUT::from_cutter(&cutter, crate::stock::radial_profile::LUT_SAMPLES);
    let load_cutter = StepLoadCutter {
        lut: &lut,
        diameter: 6.0,
        depth_per_pass: DPP,
    };
    let z = TOP - DPP;
    let (p, q) = (P3::new(20.0, 30.0, z), P3::new(40.0, 30.0, z));

    let fresh = TriDexelStock::from_stock(0.0, 0.0, 60.0, 60.0, 0.0, TOP, CELL);
    let full = step_load(&fresh, p, q, 3.0, 0.0, &load_cutter);
    assert!((full - 1.0).abs() < 1e-9, "full pass load {full}");
    assert!((step_axial_depth(&fresh, p, q, 3.0) - DPP).abs() < 1e-6);

    let thin = TriDexelStock::from_stock(0.0, 0.0, 60.0, 60.0, 0.0, z + LAYER, CELL);
    let light = step_load(&thin, p, q, 3.0, 0.0, &load_cutter);
    assert!(
        (light - LAYER / DPP).abs() < 1e-9,
        "thin layer load {light}, expected {}",
        LAYER / DPP
    );
    assert!((step_axial_depth(&thin, p, q, 3.0) - LAYER).abs() < 1e-6);
    let ceiling = crate::ops::adaptive_shared::radial_woc_fraction_from_leading_arc(
        crate::adaptive::pass_engagement_limit(2.0, 3.0),
    );
    assert!(light < ceiling && full > ceiling, "ceiling {ceiling}");

    assert_eq!(step_load(&fresh, p, p, 3.0, 0.0, &load_cutter), 0.0);
}

/// D65: the stock-read split of the slice admits a draped step as a capped
/// cut only when it is not deep, only when its load is over the ceiling and
/// at most the full slot, and only inside one engaged run. A deep step and
/// a step over the slot still split. Removal step R5: the slice runs only
/// Geodesic Adaptive, so the rule takes no strategy (Contour Parallel has
/// no slice and never reaches it).
#[test]
fn a_draped_step_is_capped_only_up_to_a_slot_and_never_when_deep_d65() {
    use super::clearing::admits_capped_split;
    use crate::adaptive::path::FULL_SLOT_LOAD;
    let ceiling = 0.38;
    let over = (ceiling + FULL_SLOT_LOAD) / 2.0;
    assert!(admits_capped_split(false, over, ceiling, true));
    assert!(admits_capped_split(false, FULL_SLOT_LOAD, ceiling, true));
    // A deep step splits, whatever its load.
    assert!(!admits_capped_split(true, over, ceiling, true));
    // A step over the slot splits.
    assert!(!admits_capped_split(
        false,
        2.0 * FULL_SLOT_LOAD,
        ceiling,
        true
    ));
    // A step at or under the ceiling is not split at all: no admission.
    assert!(!admits_capped_split(false, ceiling, ceiling, true));
    // An engagement change keeps the split.
    assert!(!admits_capped_split(false, over, ceiling, false));
}

/// D65: the factor of a capped draped step is `c / L`, at most the factor
/// that holds its descent at the plunge rate, at most the open 2D cap, and
/// never above 1.
#[test]
fn a_capped_draped_step_descends_no_faster_than_the_plunge_d65() {
    use super::clearing::capped_step_factor;
    let (feed, plunge) = (1000.0, 500.0);
    let (ceiling, load) = (0.38, 0.5);
    // A shallow step: the load factor binds.
    let (a, b) = (P3::new(0.0, 0.0, 5.0), P3::new(10.0, 0.0, 4.5));
    let f = capped_step_factor(ceiling, load, a, b, feed, plunge, None);
    assert!((f - ceiling / load).abs() < 1e-12, "shallow: {f}");
    // A steep step: its descent rate F·f·|dz|/|step| is at most the plunge.
    let (a, b) = (P3::new(0.0, 0.0, 10.0), P3::new(1.0, 0.0, 4.0));
    let f = capped_step_factor(ceiling, load, a, b, feed, plunge, None);
    let len = (1.0_f64 + 36.0).sqrt();
    assert!(feed * f * 6.0 / len <= plunge + 1e-9, "steep: {f}");
    assert!(f < ceiling / load);
    // An open 2D cap lower than both binds.
    let f = capped_step_factor(ceiling, load, a, b, feed, plunge, Some(0.1));
    assert!((f - 0.1).abs() < 1e-12);
    // A climbing step has no descent term.
    let f = capped_step_factor(ceiling, load, b, a, feed, plunge, None);
    assert!((f - ceiling / load).abs() < 1e-12, "climb: {f}");
}

/// G-DRAPEGAP (2026-11-02): a raised patch beside a straight cut, within one
/// tool radius of the tool between two drape samples and farther than a
/// radius from both samples. The drape (samples at the tool radius) misses
/// it and the chord cuts 1 mm into the model; the shipped path
/// (`emitted_cut_path`, ending in `lift_chords_to_leave`) holds the leave to
/// within the chord tolerance at every point along it. Red proof: the first
/// assertion, on the pipeline before the chord lift.
#[test]
fn a_chord_beside_a_peak_holds_the_leave_g_drapegap() {
    use super::path::{drape_path_to_leave, emitted_cut_path};
    use crate::surface::dropcutter::point_drop_cutter;
    // A flat floor at z 0 and a 0.2 x 0.2 mm patch at z 1, at x 4.5, y 2.8.
    let floor = crate::mesh::make_test_flat(50.0);
    let mut vertices = floor.vertices.clone();
    let mut triangles = floor.triangles;
    let base = vertices.len() as u32;
    for (x, y) in [(4.4, 2.7), (4.6, 2.7), (4.6, 2.9), (4.4, 2.9)] {
        vertices.push(P3::new(x, y, 1.0));
    }
    triangles.push([base, base + 1, base + 2]);
    triangles.push([base, base + 2, base + 3]);
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = FlatEndmill::new(6.0, 25.0);
    let r = cutter.radius();
    let (leave, tol) = (0.0, 0.1);
    let path = [P3::new(0.0, 0.0, 0.0), P3::new(12.0, 0.0, 0.0)];
    // The patch is more than one radius from every drape sample (x = 0, 3, 6,
    // 9, 12) and inside one radius of the tool at x 4.5.
    assert!(((4.5_f64 - 3.0).powi(2) + 2.7_f64.powi(2)).sqrt() > r);
    let rest = |x: f64, y: f64| {
        let cl = point_drop_cutter(x, y, &mesh, &index, &cutter);
        if cl.contacted {
            cl.z + leave
        } else {
            f64::NEG_INFINITY
        }
    };
    assert!(
        (rest(4.5, 0.0) - 1.0).abs() < 1e-9,
        "fixture: the tool at x 4.5 rests on the patch"
    );
    // The deepest dip of a polyline under the rest height, sampled every 0.02 mm.
    let deepest_dip = |poly: &[P3]| -> f64 {
        let mut worst = f64::NEG_INFINITY;
        for w in poly.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            let n = ((len / 0.02).ceil() as usize).max(1);
            for i in 0..=n {
                let t = i as f64 / n as f64;
                let (x, y, z) = (
                    a.x + t * (b.x - a.x),
                    a.y + t * (b.y - a.y),
                    a.z + t * (b.z - a.z),
                );
                worst = worst.max(rest(x, y) - z);
            }
        }
        worst
    };
    // Red proof: the drape alone leaves the chord on the floor under the patch.
    let draped = drape_path_to_leave(&path, &mesh, &index, &cutter, leave, r);
    assert!(
        deepest_dip(&draped) > 0.9,
        "the drape misses the patch beside the path"
    );
    // The shipped path passes the model gouge guard's own measure
    // (`compute::model_gouge`, `Allowance`): a probe a chord tolerance smaller
    // in radius, the limit its rest height + leave - tolerance, and the guard's
    // Z tolerance. A full-tool measure would demand a vertical move at the
    // edge of the patch, which no straight chord makes.
    let probe = FlatEndmill::new(2.0 * (r - tol), 25.0);
    let shipped = emitted_cut_path(
        &path,
        &mesh,
        &index,
        &cutter,
        Some(&probe),
        leave,
        tol,
        0.0,
        None,
        &mut crate::tool::body::FedLead::default(),
    );
    let mut worst = f64::NEG_INFINITY;
    for w in shipped.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        let n = ((len / 0.02).ceil() as usize).max(1);
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let (x, y, z) = (
                a.x + t * (b.x - a.x),
                a.y + t * (b.y - a.y),
                a.z + t * (b.z - a.z),
            );
            let cl = point_drop_cutter(x, y, &mesh, &index, &probe);
            if cl.contacted {
                worst = worst.max(cl.z + leave - tol - z);
            }
        }
    }
    assert!(
        worst <= crate::compute::model_gouge::MODEL_GOUGE_Z_TOL_MM,
        "the shipped chord is {worst:.4} mm under the guard's limit"
    );
    // It only raises the path: no point below its own rest height.
    for p in &shipped {
        assert!(p.z >= rest(p.x, p.y) - 1e-9);
    }
}

// ── Rest region filter ──────────────────────────────────────────

/// A rest stock on the flat mesh: material from the floor to Z 6 in two
/// leftovers, and cut below the leave everywhere else. The broad leftover
/// is 28 x 30 mm at X -24..4; the narrow leftover is 6 x 30 mm at X 14..20.
/// The 10 mm gap is wider than the tool, so a cut of one leftover does not
/// reach the other. The cut points west of X 9 cut the broad leftover.
fn rest_two_leftovers() -> TriDexelStock {
    let cell_size = 0.5;
    let mut stock = TriDexelStock::from_stock(-25.0, -25.0, 25.0, 25.0, -1.0, 6.0, cell_size);
    let grid = &mut stock.z_grid;
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let (x, y) = grid.cell_to_world(row, col);
            let in_y = (-15.0..=15.0).contains(&y);
            let broad = in_y && (-24.0..=4.0).contains(&x);
            let narrow = in_y && (14.0..=20.0).contains(&x);
            if !broad && !narrow {
                ray_subtract_above(grid.ray_mut(row, col), 0.3);
            }
        }
    }
    stock
}

/// The cut points of a rest rough with `filter`: (west of X 9, east of X 9).
fn rest_cut_points(filter: Option<RestRegionFilter>) -> (usize, usize) {
    let (mesh, si) = make_flat_mesh();
    let cutter = flat_cutter();
    let mut p = default_params();
    p.initial_stock = Some(rest_two_leftovers());
    p.depth.stock_top_z = 6.0;
    p.linking.rest_region_filter = filter;
    let never_cancel = || false;
    let result = adaptive_3d_segments(&mesh, &si, &cutter, &p, None, &never_cancel)
        .expect("planner should succeed");
    let mut west = 0;
    let mut east = 0;
    for seg in &result.segments {
        if let Adaptive3dSegment::Cut(path) = seg {
            for pt in path {
                if pt.x < 9.0 {
                    west += 1;
                } else {
                    east += 1;
                }
            }
        }
    }
    (west, east)
}

#[test]
fn a_rest_filter_cuts_only_the_small_leftover_rest_filter() {
    // Control: with no filter, both leftovers are cut.
    let (west, east) = rest_cut_points(None);
    assert!(west > 0 && east > 0, "no filter: west {west}, east {east}");

    // Width bound 12 mm: the 6 mm strip passes, the 28 mm block fails.
    let (west, east) = rest_cut_points(Some(RestRegionFilter {
        max_area_mm2: None,
        max_width_mm: Some(12.0),
    }));
    assert_eq!(west, 0, "width filter: the broad leftover is cut");
    assert!(east > 0, "width filter: the narrow leftover is not cut");

    // Area bound 400 mm²: the strip (180 mm²) passes, the block (840 mm²)
    // fails.
    let (west, east) = rest_cut_points(Some(RestRegionFilter {
        max_area_mm2: Some(400.0),
        max_width_mm: None,
    }));
    assert_eq!(west, 0, "area filter: the broad leftover is cut");
    assert!(east > 0, "area filter: the small leftover is not cut");
}

/// The probe tool of G-SIMCORE: an 18 mm bit with an 8 mm centre core and a
/// 1 mm blade protrusion (wanaka tool 6).
fn simcore_probe_tool(non_centre_cutting: bool) -> crate::tool::ToolDefinition {
    use crate::compute::tool_config::{
        CentreCutting, NonCentreCuttingGeometry, ToolConfig, ToolFact, ToolId, ToolType,
    };
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 18.0;
    tool.set_cutting_length(ToolFact::Stated(30.0));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_shank_diameter(ToolFact::Stated(12.0));
    tool.set_flute_count(ToolFact::Stated(2));
    if non_centre_cutting {
        tool.set_centre_cutting(ToolFact::Stated(CentreCutting::No(
            NonCentreCuttingGeometry {
                centre_core_diameter_mm: 8.0,
                blade_protrusion_mm: 1.0,
            },
        )));
    }
    crate::compute::cutter::build_cutter(&tool).expect("a valid tool")
}

/// G-SIMCORE (P22-0b): the planner stock after a 2 mm helix of the probe
/// tool keeps the post under the core, as the simulation does
/// (`a_helix_inside_the_core_leaves_a_post_g_simcore`). The planner stamps a
/// cut with its cut table (`PlannerLuts::cut`). Red before P22-0b: the
/// planner stamped the envelope and cut the post. The entry table stays the
/// envelope, and a centre-cutting tool has one table.
#[test]
fn the_planner_stock_keeps_the_post_of_a_small_helix_g_simcore() {
    let tool = simcore_probe_tool(true);
    let luts = super::path::PlannerLuts::of(&tool);
    let (top, floor, radius) = (12.0, 7.0, 2.0);
    let mut stock = make_stock(-30.0, -30.0, 30.0, 30.0, top, 0.25);
    let slope = 1.0 / 8.0;
    let chord = 10f64.to_radians();
    let mut prev = P3::new(radius, 0.0, top + 0.5);
    let (mut a, mut z) = (0.0f64, top + 0.5);
    for _ in 0..400 {
        a += chord;
        z = (z - radius * chord * slope).max(floor);
        let next = P3::new(radius * a.cos(), radius * a.sin(), z);
        stock.stamp_linear_segment(
            &luts.cut,
            tool.radius(),
            prev,
            next,
            StockCutDirection::FromTop,
        );
        prev = next;
    }
    let centre = stock.max_top_z_in_disc(0.0, 0.0, 0.3).expect("material");
    assert!(
        centre >= top - 0.25,
        "the planner stock cut the post under the core: the centre reads Z {centre:.3}"
    );
    assert!(
        luts.entry().blade_ring_core_sq().is_none(),
        "the entry table is the envelope"
    );
    let centre_cutting = super::path::PlannerLuts::of(&simcore_probe_tool(false));
    assert!(
        std::ptr::eq(centre_cutting.entry(), &centre_cutting.cut),
        "a centre-cutting tool has one table"
    );
}

/// P36 B (`planning/feeds_contract_2026-11-03/phases/P36_DRILL_RAPID_CENTRE_DESCENT_2026-11-16.md`
/// section 2): the ONE cut pipeline (`emitted_cut_path`) holds the core dwell
/// rule across the join of two cut pieces, because the pieces share the fed
/// chain of the tool (`FedLead`).
///
/// Two pieces of the probe tool (core diameter 8, protrusion 1) high over a
/// flat model: 10 mm along +X, then 10 mm along +Y from the same corner, each
/// a descent at the straight-line limit (1 mm in 8 mm) in 0.5 mm steps. Each piece alone
/// holds the rule. In the window of XY diameter 8 mm that ends on the second
/// piece and starts on the first, the path goes down more than 1 mm.
///
/// - Red proof: a new lead for each piece (the read before P36) leaves a
///   window descent above the protrusion.
/// - One lead for both pieces: at most the protrusion at every vertex.
/// - A centre-cutting tool keeps both pieces as they are, and its lead stays
///   empty.
#[test]
fn two_cut_pieces_share_the_fed_chain_p36() {
    use super::path::emitted_cut_path;
    use crate::tool::body::{FedLead, core_of};
    let mesh = crate::mesh::make_test_flat(60.0);
    let index = SpatialIndex::build(&mesh, 10.0);
    let slope = 1.0 / 8.0;
    // 0.5 mm steps; the model is at Z 0, far below, so the drape moves
    // nothing. Each point is 0.02 mm to one side of its leg in turn, so the
    // simplification keeps every vertex (the clamp reads the rule at the
    // vertices).
    let side = |k: i32| if k % 2 == 0 { 0.0 } else { 0.02 };
    let first: Vec<P3> = (0..=20)
        .map(|k| {
            let d = 0.5 * f64::from(k);
            P3::new(d, side(k), 20.0 - slope * d)
        })
        .collect();
    let second: Vec<P3> = (0..=20)
        .map(|k| {
            let d = 0.5 * f64::from(k);
            P3::new(10.0 + side(k), d, 20.0 - slope * (10.0 + d))
        })
        .collect();
    // No simplification and no blend, so the vertices stay.
    let ship = |tool: &crate::tool::ToolDefinition,
                path: &[P3],
                anchor: Option<P3>,
                lead: &mut FedLead| {
        emitted_cut_path(path, &mesh, &index, tool, None, 0.0, 0.0, 0.0, anchor, lead)
    };
    // The largest descent, over every vertex, from the highest point of the
    // path before it inside one XY diameter of `reach` (0.01 mm samples).
    let worst_window_descent = |poly: &[P3], reach: f64| -> f64 {
        let mut dense: Vec<P3> = vec![poly[0]];
        for w in poly.windows(2) {
            let n = (((w[1].x - w[0].x).hypot(w[1].y - w[0].y) / 0.01).ceil() as usize).max(1);
            for i in 1..=n {
                let t = i as f64 / n as f64;
                dense.push(w[0] + (w[1] - w[0]) * t);
            }
        }
        let mut worst = 0.0_f64;
        for end in poly {
            let e = dense
                .iter()
                .position(|p| (p - end).norm() < 1e-9)
                .expect("a vertex is a sample");
            let mut window: Vec<P3> = vec![dense[e]];
            for q in dense[..e].iter().rev() {
                if window
                    .iter()
                    .step_by(25)
                    .chain(window.last())
                    .any(|w| (w.x - q.x).hypot(w.y - q.y) > reach)
                {
                    break;
                }
                worst = worst.max(q.z - end.z);
                window.push(*q);
            }
        }
        worst
    };

    let tool = simcore_probe_tool(true);
    let core = core_of(&tool).expect("the probe tool has a core");
    let (p, reach) = (core.protrusion_mm, core.diameter_mm());

    // Red proof: each piece with its own lead.
    let alone_a = ship(&tool, &first, None, &mut FedLead::default());
    let alone_b = ship(
        &tool,
        &second,
        alone_a.last().copied(),
        &mut FedLead::default(),
    );
    let alone: Vec<P3> = alone_a
        .iter()
        .chain(alone_b.iter().skip(1))
        .copied()
        .collect();
    let before = worst_window_descent(&alone, reach);
    assert!(
        before > p + 0.05,
        "control: two pieces read alone go down {before:.3} mm in one window"
    );

    // One lead for the tool.
    let mut lead = FedLead::default();
    let chain_a = ship(&tool, &first, None, &mut lead);
    let chain_b = ship(&tool, &second, chain_a.last().copied(), &mut lead);
    let chained: Vec<P3> = chain_a
        .iter()
        .chain(chain_b.iter().skip(1))
        .copied()
        .collect();
    let after = worst_window_descent(&chained, reach);
    assert!(
        after <= p + 0.01,
        "with the fed chain a window goes down {after:.3} mm, above the protrusion {p}"
    );
    for (a, b) in alone.iter().zip(&chained) {
        assert!(b.z >= a.z, "the chain read only raises: {a:?} -> {b:?}");
    }
    assert_eq!(
        lead.position(),
        chained.last().copied(),
        "the lead follows the tool"
    );

    // A centre-cutting tool: no clamp, no lead.
    let plain = simcore_probe_tool(false);
    let mut plain_lead = FedLead::default();
    let plain_a = ship(&plain, &first, None, &mut plain_lead);
    let plain_b = ship(&plain, &second, plain_a.last().copied(), &mut plain_lead);
    assert_eq!((plain_a, plain_b), (first, second));
    assert_eq!(plain_lead, FedLead::default());
}

/// P37 fixture: a rest stock on a flat floor (Z 0) with a block of 20 x 24 mm
/// and, when `island`, a column of 4 x 4 mm, both 6 mm tall. Cell 0.5 mm.
fn left_stock_of(island: bool) -> Option<LeftStock> {
    left_stock_with_dial(island, 15.0, OnePassHold::Off)
}

/// [`left_stock_of`] with `min_region_cut_length_mm` and the one-pass dial
/// stated (the region contour of P44 is on with `RemainingStock`).
fn left_stock_with_dial(island: bool, min_cut_mm: f64, hold: OnePassHold) -> Option<LeftStock> {
    let mesh = crate::mesh::make_test_flat(60.0);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = flat_cutter();
    let (cell, n) = (0.5, 121usize);
    let mut tops = vec![0.0f64; n * n];
    for row in 0..n {
        for col in 0..n {
            let (x, y) = (-30.0 + col as f64 * cell, -30.0 + row as f64 * cell);
            let block = (5.0..=25.0).contains(&x) && (-12.0..=12.0).contains(&y);
            let column = island && (-17.0..=-13.0).contains(&x) && (-2.0..=2.0).contains(&y);
            if block || column {
                tops[row * n + col] = 6.0;
            }
        }
    }
    let mut params = default_params();
    params.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
    params.linking.min_region_cut_length_mm = min_cut_mm;
    params.linking.one_pass_hold = hold;
    params.depth.stock_top_z = 6.0;
    params.initial_stock = Some(make_stock_with_cells(
        n, n, -30.0, -30.0, cell, -10.0, &tops,
    ));
    adaptive_3d_toolpath_output_with_cancel(&mesh, &si, &cutter, &params, &|| false, None)
        .expect("not cancelled")
        .left_stock
}

/// P37 (prediction P-5), P44: a region whose cut is shorter than the
/// operator dial `min_region_cut_length_mm` (15 mm) is recorded, and the op
/// states that it still holds stock at the end of the plan: its count, its
/// area, its height and its position. The column is 4 x 4 mm: its contour
/// line (0.6 mm inside the outline of its cells) is 4 x 3.3 mm long, under
/// the dial. P44 removed the small-region bound that dropped it before.
#[test]
fn a_region_under_the_cut_length_dial_is_stated_p37() {
    let left = left_stock_of(true).expect("the plan dropped a region");
    assert!(left.short_forecast >= 1, "{left:?}");
    assert_eq!(left.no_field, 0, "{left:?}");
    assert_eq!(left.standing_pieces, left.short_forecast, "{left:?}");
    // The column: 9 x 9 cells of 0.25 mm2, less the cells on the outline.
    assert!(
        (12.0..=20.25).contains(&left.standing_area_mm2),
        "the standing area is the column, not the block: {left:?}"
    );
    // 6 mm of stock above the floor of the lowest level (the stock to
    // leave, 0.5).
    assert!((left.tallest_mm - 5.5).abs() < 1e-6, "{left:?}");
    let [x, y, z] = left.tallest_at;
    assert!(
        (-17.0..=-13.0).contains(&x) && (-2.0..=2.0).contains(&y),
        "{left:?}"
    );
    assert!((z - 6.0).abs() < 1e-6, "{left:?}");
    // P44: the column is a named place, with the reason.
    let place = left
        .places
        .iter()
        .find(|p| p.lo[0] >= -17.5 && p.hi[0] <= -12.5 && p.lo[1] >= -2.5 && p.hi[1] <= 2.5)
        .expect("the column is a place");
    assert_eq!(place.why, PlaceWhy::ShortForecast, "{left:?}");
    assert!((place.tallest_mm - 5.5).abs() < 1e-3, "{left:?}");
    assert!((12.0..=20.25).contains(&place.area_mm2), "{left:?}");
}

/// P37: with no dropped region the op states no dropped piece.
#[test]
fn a_plan_that_drops_nothing_states_no_standing_stock_p37() {
    let left = left_stock_of(false);
    assert!(
        left.as_ref()
            .is_none_or(|l| l.standing_pieces == 0 && l.short_forecast == 0),
        "{left:?}"
    );
}

/// P44 (H1, H3): with the dial off (0) the same column is cut. No area
/// bound drops it, and the region contour cuts a region where the geodesic
/// field has no room. The control is the tree with no region contour: the
/// column stands. Seen red with the bound back (H1 reverted alone: the
/// column is 16 mm2, the bound was 152.4 mm2) and with no region contour
/// (H3 reverted alone).
#[test]
fn a_small_region_is_cut_when_no_dial_drops_it_p44() {
    // The place of the column holds the centres whose disc covers it: the
    // column and one tool radius (3.175 mm) round it.
    let in_column =
        |p: &LeftPlace| p.lo[0] >= -20.5 && p.hi[0] <= -9.5 && p.lo[1] >= -5.5 && p.hi[1] <= 5.5;
    let hold = || OnePassHold::RemainingStock {
        link_kinematics: None,
    };
    let left = left_stock_with_dial(true, 0.0, hold());
    assert!(
        left.as_ref()
            .is_none_or(|l| !l.places.iter().any(|p| in_column(p) && p.tallest_mm > 1.0)),
        "the column is cut: {left:?}"
    );
    super::level_contour::REGION_CONTOUR_OFF_FOR_CONTROL.with(|c| c.set(true));
    let control = left_stock_with_dial(true, 0.0, hold());
    super::level_contour::REGION_CONTOUR_OFF_FOR_CONTROL.with(|c| c.set(false));
    assert!(
        control
            .as_ref()
            .is_some_and(|l| l.places.iter().any(|p| in_column(p) && p.tallest_mm > 5.0)),
        "the control: with no region contour the column stands: {control:?}"
    );
}

/// P44 fixture: a plate 60 x 60 mm, top Z 10, with a slot pocket 40 x 9 mm
/// (X 10..50, Y 20..29) and a square pocket 8 x 8 mm (X 10..18, Y 40..48),
/// floors at Z 0. The stock is the full block on a cell of 0.2 mm, given as
/// remaining stock. Tool 6.35 mm, Depth/Pass 3 mm, stock to leave 0.5: the
/// levels are 7, 4, 1 and 0.5. The tool-centre region of the slot is 2.65 mm
/// wide at the floor: the geodesic field has no room in it.
///
/// Returns the deepest simulated sample, the highest stock on the centre
/// line of the slot (Y 24.5, X 16..44) after the simulation, and the record.
fn slot_fixture_p44(min_cut_mm: f64) -> (f64, f64, Option<LeftStock>) {
    let mesh = make_pocket_plate(
        60,
        60,
        &[(10.0, 20.0, 50.0, 29.0), (10.0, 40.0, 18.0, 48.0)],
    );
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = flat_cutter();
    let (cell, n) = (0.2, 301usize);
    let tops = vec![10.0f64; n * n];
    let stock = make_stock_with_cells(n, n, 0.0, 0.0, cell, -10.0, &tops);
    let mut params = default_params();
    params.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
    params.linking.min_region_cut_length_mm = min_cut_mm;
    params.linking.one_pass_hold = OnePassHold::RemainingStock {
        link_kinematics: None,
    };
    params.depth.stock_top_z = 10.0;
    params.initial_stock = Some(stock.clone());
    let out =
        adaptive_3d_toolpath_output_with_cancel(&mesh, &si, &cutter, &params, &|| false, None)
            .expect("not cancelled");
    let mut sim_stock = stock;
    let samples = sim_stock
        .simulate_toolpath_with_metrics_with_cancel(
            &out.toolpath,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            &[],
            false,
            &|| false,
        )
        .expect("the simulation is not cancelled");
    let deepest = samples
        .iter()
        .map(|s| s.axial_engagement_mm)
        .fold(0.0, f64::max);
    let row = 122; // Y 24.4
    let highest = (80..=220) // X 16..44
        .map(|col| stock_top_z_at(&sim_stock, row, col))
        .fold(f64::NEG_INFINITY, f64::max);
    (deepest, highest, out.left_stock)
}

/// P44 (H1, H3): a slot that the tool fits in is cut to its floor, one
/// Depth/Pass at a time, on remaining stock. The geodesic plan has no field
/// in its region, so the region contour cuts it (H3). The control (no
/// region contour): the slot keeps the stock top. Seen red with H3 reverted
/// alone (the slot stands at Z 10.00), and with the small-region bound back
/// (H1 reverted alone: the slot stands at Z 1.00; the bound of 152.4 mm2
/// drops its region at the last level).
#[test]
fn a_slot_narrower_than_the_field_is_cut_to_its_floor_p44() {
    let (deepest, highest, left) = slot_fixture_p44(0.0);
    assert!(
        highest <= 0.5 + 0.3,
        "the centre line of the slot stands at Z {highest:.2}; the floor of the plan is 0.5          ({left:?})"
    );
    assert!(
        deepest <= 3.0 + 0.5 + 1e-6,
        "a sample cuts {deepest:.2} mm deep; one Depth/Pass and the clearance are 3.5 mm"
    );
    // P44b (lead ruling D-15 c): a region that the region contour cut is
    // not left, so the record holds no "no field" piece.
    assert!(
        left.as_ref().is_none_or(|l| l.no_field == 0),
        "the slot is cut, so it is not a left piece: {left:?}"
    );
    super::level_contour::REGION_CONTOUR_OFF_FOR_CONTROL.with(|c| c.set(true));
    let (_, control, control_left) = slot_fixture_p44(0.0);
    assert!(
        control_left.as_ref().is_some_and(|l| l.no_field > 0),
        "the control: with no region contour the slot is left: {control_left:?}"
    );
    super::level_contour::REGION_CONTOUR_OFF_FOR_CONTROL.with(|c| c.set(false));
    assert!(
        control >= 10.0 - 1e-6,
        "the control: with no region contour the slot keeps the stock top: {control:.2}"
    );
}

/// P44 (H2): the F-038 forecast reads a region in its own frame. With the
/// dial at 15 mm the slot is cut (its contour line is about 70 mm long;
/// the forecast before P44 moved the outline inside by the tool radius
/// again and read 0). The square pocket is dropped by the dial (its line is
/// under 15 mm at each level), and the record names it with that reason.
/// Seen red with H2 reverted alone (the slot stands at Z 10.00).
#[test]
fn the_forecast_reads_a_slot_region_in_its_own_frame_p44() {
    let (deepest, highest, left) = slot_fixture_p44(15.0);
    assert!(
        highest <= 0.5 + 0.3,
        "the centre line of the slot stands at Z {highest:.2} ({left:?})"
    );
    assert!(deepest <= 3.5 + 1e-6, "{deepest:.2}");
    let left = left.expect("the square pocket is dropped");
    let place = left
        .places
        .iter()
        .find(|p| p.lo[0] >= 9.0 && p.hi[0] <= 19.0 && p.lo[1] >= 39.0 && p.hi[1] <= 49.0)
        .expect("the square pocket is a place");
    assert_eq!(place.why, PlaceWhy::ShortForecast, "{left:?}");
    assert!(place.tallest_mm > 9.0, "{left:?}");
    let text = crate::compute::toolpath_stats::RoughLeftStockFinding {
        left: left.clone(),
        regions_published: false,
    }
    .to_string();
    assert!(
        text.contains("a cut there is shorter than Min Region Cut Length"),
        "{text}"
    );
}

/// P44 (lead ruling 2026-11-21): a place where the tool fits and stock
/// stands with no drop record does not raise the finding alone. It is one
/// count and one area. A place that a drop record covers is named, and it
/// raises the finding.
///
/// The fixture: a flat floor at Z 0 on a cell of 0.5 mm, and two strips of
/// stock 4 mm tall: strip A (X 2..3, Y 2..8) and strip B (X 12..13, Y 2..8).
/// The standing read holds both. The plan recorded one dropped piece.
#[test]
fn stock_with_no_drop_record_does_not_raise_the_finding_p44() {
    let (n, cell) = (40usize, 0.5);
    let in_a = |x: f64, y: f64| (2.0..=3.0).contains(&x) && (2.0..=8.0).contains(&y);
    let in_b = |x: f64, y: f64| (12.0..=13.0).contains(&x) && (2.0..=8.0).contains(&y);
    let mut tops = vec![0.0f64; n * n];
    let mut padded = vec![false; (n + 2) * (n + 2)];
    let mut height = vec![0.0f32; n * n];
    for row in 0..n {
        for col in 0..n {
            let (x, y) = (col as f64 * cell, row as f64 * cell);
            if in_a(x, y) || in_b(x, y) {
                tops[row * n + col] = 4.0;
                padded[(row + 1) * (n + 2) + (col + 1)] = true;
                height[row * n + col] = 4.0;
            }
        }
    }
    let stock = make_stock_with_cells(n, n, 0.0, 0.0, cell, -10.0, &tops);
    let surface_hm =
        SurfaceHeightmap::from_parts(vec![0.0; n * n], vec![true; n * n], n, n, 0.0, 0.0, cell);
    let one_pass = super::one_pass::OnePass::new(3.0, 0.5, true);
    let standing = || {
        Some(super::left_stock::StandingRead {
            standing: super::standing::Standing {
                padded: padded.clone(),
                padded_rows: n + 2,
                padded_cols: n + 2,
                height: height.clone(),
                count: padded.iter().filter(|&&c| c).count(),
            },
            z_level: 0.0,
            line_inset_mm: super::level_contour::contour_line_inset(cell, 0.1),
        })
    };
    let piece = |x0: f64, x1: f64| super::left_stock::Unplanned {
        z_level: 0.0,
        outline: crate::polygon::Polygon2::new(vec![
            crate::geo::P2::new(x0, 1.0),
            crate::geo::P2::new(x1, 1.0),
            crate::geo::P2::new(x1, 9.0),
            crate::geo::P2::new(x0, 9.0),
        ]),
        why: LeftWhy::ShortForecast,
    };
    let read = |unplanned: &[super::left_stock::Unplanned]| {
        super::left_stock::left_stock(
            unplanned,
            0,
            &one_pass,
            &stock,
            &surface_hm,
            0.0,
            standing,
            3.0,
        )
    };
    // The plan recorded nothing: no record, as before P44.
    assert!(read(&[]).is_none());
    // The dropped piece lies on cut stock (X 20..25): both strips stand
    // with no drop record. The finding is not raised.
    let none = read(&[piece(20.0, 25.0)]).expect("the plan recorded a piece");
    assert!(none.places.is_empty(), "{none:?}");
    assert_eq!(
        (none.places_total, none.unrecorded_places),
        (0, 2),
        "{none:?}"
    );
    assert!(
        (none.unrecorded_area_mm2 - 2.0 * 3.0 * 13.0 * 0.25).abs() < 1e-9,
        "{none:?}"
    );
    assert_eq!(none.standing_pieces, 0, "{none:?}");
    assert!(
        !none.stands(),
        "no drop record: the finding is not raised: {none:?}"
    );
    // The dropped piece covers strip A: strip A is a named place, and the
    // finding is raised. Strip B stays in the sum.
    let one = read(&[piece(1.0, 4.0)]).expect("the plan recorded a piece");
    assert_eq!((one.places_total, one.unrecorded_places), (1, 1), "{one:?}");
    assert_eq!(one.places[0].why, PlaceWhy::ShortForecast, "{one:?}");
    assert!(one.stands(), "{one:?}");
    // P44b (lead ruling D-11): a dropped cut group gives its reason to the
    // place on its box. The box of a straight cut has no area (X 12.5, Y 2
    // to 8); it is not a standing piece.
    let group = super::left_stock::Unplanned {
        z_level: 0.0,
        outline: crate::polygon::Polygon2::new(vec![
            crate::geo::P2::new(12.5, 2.0),
            crate::geo::P2::new(12.5, 2.0),
            crate::geo::P2::new(12.5, 8.0),
            crate::geo::P2::new(12.5, 8.0),
        ]),
        why: LeftWhy::ShortGroup,
    };
    let grouped = read(&[group]).expect("the plan recorded a group");
    assert_eq!(
        (grouped.places_total, grouped.unrecorded_places),
        (1, 1),
        "{grouped:?}"
    );
    assert_eq!(grouped.places[0].why, PlaceWhy::ShortGroup, "{grouped:?}");
    assert!((grouped.places[0].lo[0] - 12.0).abs() < 1e-9, "{grouped:?}");
    assert_eq!(grouped.standing_pieces, 0, "{grouped:?}");
    assert!(grouped.stands(), "{grouped:?}");
    let text = crate::compute::toolpath_stats::RoughLeftStockFinding {
        left: one,
        regions_published: false,
    }
    .to_string();
    assert!(
        text.contains("stock still stands at 1 place that the plan did not cut"),
        "{text}"
    );
    assert!(
        text.contains("Stock also stands at 1 more place with no drop record (9.8 mm²"),
        "{text}"
    );
    assert_eq!(
        text.matches("X12.0").count(),
        0,
        "strip B is not listed: {text}"
    );
}

/// K5 CentreCut (P40 A4): the descent clamp of the cut and link pipelines
/// (`clamp_descents_to_limit`) reads only a tool with a centre core.
///
/// The path goes down 5 mm in 5 mm of XY, in 0.5 mm steps: steeper than the
/// straight-line limit of the probe tool (1 mm in 8 mm).
///
/// - A tool that cuts at its centre keeps the path, bit for bit, and its
///   fed lead stays empty.
/// - The same tool with a centre core gets the path raised: no step is
///   steeper than its limit, no point goes down, and the path changes.
///
/// It replaces `a_centre_cutting_tool_keeps_the_surface_slope` of the K5
/// sentry, which compared the steepest fed descent of two session roughs.
/// The steep descents it read were the falls of ramp legs that the model
/// clip lifted, and P39 took those legs out of the pocket fixture.
///
/// Red: a clamp that gives every tool the core of the probe tool
/// (`core_of(cutter).or(..)`) changes the first path.
#[test]
fn the_descent_clamp_changes_only_a_tool_with_a_centre_core_k5() {
    use super::path::clamp_descents_to_limit;
    use crate::tool::body::{FedLead, core_of};
    let steep: Vec<P3> = (0..=10)
        .map(|k| {
            let d = 0.5 * f64::from(k);
            P3::new(d, 0.0, 20.0 - d)
        })
        .collect();

    let yes = simcore_probe_tool(false);
    let mut yes_lead = FedLead::default();
    let kept = clamp_descents_to_limit(steep.clone(), None, &mut yes_lead, &yes);
    assert_eq!(
        kept, steep,
        "a centre-cutting tool keeps its steep path as it is"
    );
    assert!(
        yes_lead.position().is_none(),
        "a centre-cutting tool writes no lead"
    );

    let no = simcore_probe_tool(true);
    let slope = core_of(&no)
        .expect("the probe tool has a core")
        .max_descent_slope();
    let clamped = clamp_descents_to_limit(steep.clone(), None, &mut FedLead::default(), &no);
    assert_eq!(clamped.len(), steep.len());
    assert_ne!(clamped, steep, "the clamp raises the path of a `No` tool");
    for (before, after) in steep.iter().zip(&clamped) {
        assert!(
            after.z >= before.z && after.x == before.x && after.y == before.y,
            "the clamp only raises a point: {before:?} to {after:?}"
        );
    }
    for pair in clamped.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let dxy = (b.x - a.x).hypot(b.y - a.y);
        assert!(
            a.z - b.z <= dxy * slope * (1.0 + 1e-9) + 1e-9,
            "a step from {a:?} to {b:?} is steeper than the limit {slope:.4}"
        );
    }
}

/// P41 fixture: a rest stock on a flat floor (Z 0). A block of 40 x 24 mm
/// stands 3 mm tall. A rib 1.5 mm wide stands 12 mm tall across it (when
/// `rib`). Cell 0.5 mm, tool 6.35 mm, Depth/Pass 3 mm, stock to leave 0.5,
/// stay-down clearance 0.5.
///
/// The levels are 9, 6, 3 and 0.5. At 9 the rib is the only material: a
/// region with no room for a contour line (the F-038 forecast reads 0), so
/// the level drops it. So the rib is stock that an upper level leaves. At 0.5 the
/// block is a region that holds the rib.
///
/// Returns the deepest simulated depth of a sample (`axial_engagement_mm`,
/// the depth of every gate) and the record of the plan.
fn deepest_cut_on_rib_fixture_p41(
    rib: bool,
    strategy: ClearingStrategy3d,
    ordering: RegionOrdering,
) -> (f64, Option<LeftStock>) {
    let mesh = crate::mesh::make_test_flat(80.0);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = flat_cutter();
    let (cell, n) = (0.5, 141usize);
    let mut tops = vec![0.0f64; n * n];
    for row in 0..n {
        for col in 0..n {
            let (x, y) = (-35.0 + col as f64 * cell, -35.0 + row as f64 * cell);
            if (-20.0..=20.0).contains(&x) && (-12.0..=12.0).contains(&y) {
                tops[row * n + col] = if rib && (-0.5..=1.0).contains(&x) {
                    12.0
                } else {
                    3.0
                };
            }
        }
    }
    let stock = make_stock_with_cells(n, n, -35.0, -35.0, cell, -10.0, &tops);
    let mut params = default_params();
    params.clearing_strategy = strategy;
    params.linking.region_ordering = ordering;
    // The rule is a dial; the session sets it on remaining stock.
    params.linking.one_pass_hold = OnePassHold::RemainingStock {
        link_kinematics: None,
    };
    params.depth.stock_top_z = if rib { 12.0 } else { 3.0 };
    params.initial_stock = Some(stock.clone());
    // P44: the region contour cuts the rib at its own levels. This fixture
    // needs stock that the levels above LEFT, so it runs with the region
    // contour off.
    super::level_contour::REGION_CONTOUR_OFF_FOR_CONTROL.with(|c| c.set(true));
    let out =
        adaptive_3d_toolpath_output_with_cancel(&mesh, &si, &cutter, &params, &|| false, None);
    super::level_contour::REGION_CONTOUR_OFF_FOR_CONTROL.with(|c| c.set(false));
    let out = out.expect("not cancelled");
    let mut sim_stock = stock;
    let samples = sim_stock
        .simulate_toolpath_with_metrics_with_cancel(
            &out.toolpath,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            &[],
            false,
            &|| false,
        )
        .expect("the simulation is not cancelled");
    let deepest = samples
        .iter()
        .map(|s| s.axial_engagement_mm)
        .fold(0.0, f64::max);
    (deepest, out.left_stock)
}

/// P41 (prediction 9): no simulated sample of a 3D Rough cuts deeper than
/// one Depth/Pass plus the stay-down clearance, also where the levels above
/// left stock. The rib of the fixture stands 11.5 mm above the floor of the
/// last level; without the rule (`OnePassHold::Off`, the control below) the
/// last level cuts it at that depth. The operation states the stock that it
/// kept off.
#[test]
fn a_level_region_keeps_off_what_the_levels_above_left_p41() {
    let (deepest, left) = deepest_cut_on_rib_fixture_p41(
        true,
        ClearingStrategy3d::GeodesicAdaptive,
        RegionOrdering::Global,
    );
    let limit = 3.0 + 0.5;
    assert!(
        deepest <= limit + 1e-6,
        "a sample cuts {deepest:.2} mm deep; one Depth/Pass and the clearance are {limit} mm"
    );
    // The block is cut on both sides of the rib: the rule holds off the
    // centres at the rib, not the region.
    assert!(deepest >= 2.0, "the block is one pass deep: {deepest:.2}");
    let left = left.expect("the plan dropped the rib and kept off it");
    assert!(left.held_off_centres >= 1, "{left:?}");
    assert!(left.stands(), "{left:?}");
    // The rib: 4 cells x 49 cells of 0.25 mm2 at most.
    assert!(
        left.held_off_area_mm2 > 0.0 && left.held_off_area_mm2 <= 49.0,
        "{left:?}"
    );
    // 12 mm of stock above the cutter of the last level (Z 0.5).
    assert!((left.held_off_tallest_mm - 11.5).abs() < 1e-3, "{left:?}");
    let [x, _, z] = left.held_off_at;
    assert!(
        (-0.5..=1.0).contains(&x) && (z - 12.0).abs() < 1e-3,
        "{left:?}"
    );
}

/// P41: a plan that meets one pass of stock holds no centre off, and it
/// cuts the block (the control of the fixture).
#[test]
fn a_plan_with_one_pass_of_stock_holds_no_centre_off_p41() {
    let (deepest, left) = deepest_cut_on_rib_fixture_p41(
        false,
        ClearingStrategy3d::GeodesicAdaptive,
        RegionOrdering::Global,
    );
    assert!(
        (2.0..=3.5).contains(&deepest),
        "the block is one pass deep: {deepest:.2}"
    );
    assert!(
        left.as_ref().is_none_or(|l| l.held_off_centres == 0),
        "{left:?}"
    );
}

/// P41: the rule is in the ONE level grid, so it holds for Contour Parallel
/// and in the By Area order too.
#[test]
fn every_strategy_and_order_reads_the_one_pass_grid_p41() {
    for (strategy, ordering) in [
        (ClearingStrategy3d::ContourParallel, RegionOrdering::Global),
        (ClearingStrategy3d::GeodesicAdaptive, RegionOrdering::ByArea),
        (ClearingStrategy3d::ContourParallel, RegionOrdering::ByArea),
    ] {
        let (deepest, left) = deepest_cut_on_rib_fixture_p41(true, strategy, ordering);
        let limit = 3.0 + 0.5;
        assert!(
            deepest <= limit + 1e-6,
            "{strategy:?} {ordering:?}: a sample cuts {deepest:.2} mm deep; one Depth/Pass and \
             the clearance are {limit} mm ({left:?})"
        );
    }
}

/// P42 fixture: a floor at Z 0 and a vertical wall at X 10 that stands to
/// Z 9. The remaining stock is a block 9 mm tall from X -20 to the wall, Y
/// -12 to 12. Cell 0.2 mm, tool 6.35 mm, Depth/Pass 3 mm, stock to leave
/// 0.5, stay-down clearance 0.5: the levels are 6, 3 and 0.5.
///
/// The plan of each level keeps its centre the margin of the plan from the
/// tool-centre limit of the wall (X 6.825), so a strip stands at the wall.
/// Returns, after a simulation of the toolpath on the stock: the deepest
/// sample (`axial_engagement_mm`), and the width (mm) of the wood on the
/// line Y 0 that stands more than 1 mm above the last floor (Z 0.5) in
/// front of the wall (X under 10).
fn wall_strip_fixture_p42(hold: OnePassHold) -> (f64, f64) {
    let (wall_x, wall_top) = (10.0, 9.0);
    let vertices = vec![
        P3::new(-40.0, -40.0, 0.0),
        P3::new(wall_x, -40.0, 0.0),
        P3::new(wall_x, 40.0, 0.0),
        P3::new(-40.0, 40.0, 0.0),
        P3::new(wall_x, -40.0, wall_top),
        P3::new(wall_x, 40.0, wall_top),
        P3::new(40.0, -40.0, wall_top),
        P3::new(40.0, 40.0, wall_top),
    ];
    let triangles = vec![
        [0, 1, 2],
        [0, 2, 3],
        [1, 4, 5],
        [1, 5, 2],
        [4, 6, 7],
        [4, 7, 5],
    ];
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = flat_cutter();
    let (cell, n) = (0.2, 351usize);
    let mut tops = vec![0.0f64; n * n];
    for row in 0..n {
        for col in 0..n {
            let (x, y) = (-35.0 + col as f64 * cell, -35.0 + row as f64 * cell);
            tops[row * n + col] = if x > wall_x {
                wall_top
            } else if x >= -20.0 && (-12.0..=12.0).contains(&y) {
                9.0
            } else {
                0.0
            };
        }
    }
    let stock = make_stock_with_cells(n, n, -35.0, -35.0, cell, -10.0, &tops);
    let mut params = default_params();
    params.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
    params.linking.one_pass_hold = hold;
    params.depth.stock_top_z = 9.0;
    params.initial_stock = Some(stock.clone());
    let out =
        adaptive_3d_toolpath_output_with_cancel(&mesh, &si, &cutter, &params, &|| false, None)
            .expect("not cancelled");
    let mut sim_stock = stock;
    let samples = sim_stock
        .simulate_toolpath_with_metrics_with_cancel(
            &out.toolpath,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            &[],
            false,
            &|| false,
        )
        .expect("the simulation is not cancelled");
    let deepest = samples
        .iter()
        .map(|s| s.axial_engagement_mm)
        .fold(0.0, f64::max);
    let row = 175; // Y 0
    let standing = (0..n)
        .filter(|&col| {
            let x = -35.0 + col as f64 * cell;
            (0.0..wall_x).contains(&x) && stock_top_z_at(&sim_stock, row, col) > 0.5 + 1.0
        })
        .count();
    (deepest, standing as f64 * cell)
}

/// P42: each level of a rough on remaining stock cuts to the wall that it
/// can reach (`level_contour::region_wall_pass`). The wall pass runs one
/// planner cell plus the cut tolerance inside the boundary of the region
/// that the level planned, so it cuts the part of the margin strip of the
/// plan that it reaches, and no sample cuts deeper than one Depth/Pass plus
/// the clearance. The wall is three passes deep, so the pass ships at each
/// level (`wall_point_ships`): at 6 and 3 the next level would meet the
/// strip two passes tall, and at the last level (0.5) the wall stands 8.5 mm
/// above the floor. Measured: 0.6 mm of wood at the foot of the wall (0.8 mm
/// with the next-level rule alone, so the bound is red without rule 2).
///
/// The control: the one-pass rule with no wall pass (the tree before P42).
/// The strip is the margin of the plan at the first level, and each lower
/// level stands further off (measured: 1.2 mm at the foot). So the claim
/// is red without the wall pass.
#[test]
fn each_level_cuts_to_the_wall_that_it_can_reach_p42() {
    let cell = 0.2;
    let hold = || OnePassHold::RemainingStock {
        link_kinematics: None,
    };
    let (deepest, strip_mm) = wall_strip_fixture_p42(hold());
    let limit = 3.0 + 0.5;
    assert!(
        deepest <= limit + 1e-6,
        "a sample cuts {deepest:.2} mm deep; one Depth/Pass and the clearance are {limit} mm"
    );
    // P45: the control has no strip pass either. With the wall pass off the
    // strip pass cuts the strip at the end of the op (measured: 0.2 mm
    // against 0.2 mm), so it hides what the wall pass does per level.
    super::level_contour::WALL_PASS_OFF_FOR_CONTROL.with(|c| c.set(true));
    super::level_contour::STRIP_PASS_OFF_FOR_CONTROL.with(|c| c.set(true));
    let (deepest_held, strip_held_mm) = wall_strip_fixture_p42(hold());
    super::level_contour::WALL_PASS_OFF_FOR_CONTROL.with(|c| c.set(false));
    super::level_contour::STRIP_PASS_OFF_FOR_CONTROL.with(|c| c.set(false));
    assert!(deepest_held <= limit + 1e-6, "{deepest_held}");
    assert!(
        strip_mm <= 3.0 * cell + 1e-9,
        "{strip_mm} mm of wood stands in front of the wall with the wall pass \
         ({strip_held_mm} mm with none); three planner cells are {} mm",
        3.0 * cell
    );
    assert!(
        strip_held_mm >= strip_mm + 2.0 * cell - 1e-9,
        "the control: with no wall pass the strip at the foot of the wall is wider by two \
         cells or more: {strip_held_mm} mm against {strip_mm} mm"
    );
}

/// P42 (lead rulings D-1): the ship rule of the wall pass
/// (`level_contour::wall_point_ships`), Depth/Pass 3 mm and clearance 0.5 mm
/// (limit 3.5 mm).
/// - A wall two passes deep (top Z 6, levels 3 and 0): the pass ships at
///   both levels. At level 3 the next level would meet the strip 6 mm tall
///   (rule 1). At the last level the strip is 3 mm tall, under the limit,
///   and the wall stands 6 mm above the floor (rule 2): red without rule 2.
/// - A stair tread one pass tall on a draped slope ships none: the point has
///   the same floor at the next level, and the surface over the strip is one
///   pass above it.
/// - A pocket one pass deep ships none.
/// - With the rule off (no limit holds) nothing ships.
#[test]
fn a_wall_deeper_than_one_pass_ships_the_pass_at_each_level_p42() {
    use super::level_contour::{WallHeights, wall_point_ships};
    let rule = super::one_pass::OnePass::new(3.0, 0.5, true);
    let upper = WallHeights {
        strip_top: 6.0,
        wall_top: 6.0,
        floor: 3.0,
        next_floor: 0.0,
    };
    let last = WallHeights {
        strip_top: 3.0,
        wall_top: 6.0,
        floor: 0.0,
        next_floor: 0.0,
    };
    assert!(wall_point_ships(&rule, upper), "the upper level");
    assert!(wall_point_ships(&rule, last), "the last level of the wall");
    // Rule 1 alone does not ship the last level: the claim needs rule 2.
    assert!(!rule.over_limit(last.strip_top - last.next_floor));
    let tread = WallHeights {
        strip_top: 8.0,
        wall_top: 8.0,
        floor: 5.0,
        next_floor: 5.0,
    };
    assert!(!wall_point_ships(&rule, tread), "a stair tread of one pass");
    let shallow_pocket = WallHeights {
        strip_top: 3.0,
        wall_top: 3.0,
        floor: 0.0,
        next_floor: 0.0,
    };
    assert!(!wall_point_ships(&rule, shallow_pocket), "one pass deep");
    // A strip one pass tall over a floor that the next level lowers by one
    // pass more: rule 1.
    let over_a_drop = WallHeights {
        strip_top: 3.0,
        wall_top: 3.0,
        floor: 0.0,
        next_floor: -3.0,
    };
    assert!(wall_point_ships(&rule, over_a_drop));
}

/// P41: the rule is a dial (`Adaptive3dLinking::one_pass_hold`). With the
/// dial off the same fixture cuts the rib at its full height: the red state
/// of the test above, and the proof that a caller who does not set the dial
/// keeps the plan without the rule.
#[test]
fn the_one_pass_dial_off_keeps_the_plan_without_the_rule_p41() {
    let mesh = crate::mesh::make_test_flat(80.0);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = flat_cutter();
    let (cell, n) = (0.5, 141usize);
    let mut tops = vec![0.0f64; n * n];
    for row in 0..n {
        for col in 0..n {
            let (x, y) = (-35.0 + col as f64 * cell, -35.0 + row as f64 * cell);
            if (-20.0..=20.0).contains(&x) && (-12.0..=12.0).contains(&y) {
                tops[row * n + col] = if (-0.5..=1.0).contains(&x) { 12.0 } else { 3.0 };
            }
        }
    }
    let stock = make_stock_with_cells(n, n, -35.0, -35.0, cell, -10.0, &tops);
    let run = |hold: OnePassHold| {
        let mut params = default_params();
        params.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
        params.linking.one_pass_hold = hold;
        params.depth.stock_top_z = 12.0;
        params.initial_stock = Some(stock.clone());
        // P44: the region contour cuts the rib at its own levels; this test
        // needs the rib as stock that the levels above left (see the
        // fixture `deepest_cut_on_rib_fixture_p41`).
        super::level_contour::REGION_CONTOUR_OFF_FOR_CONTROL.with(|c| c.set(true));
        let out =
            adaptive_3d_toolpath_output_with_cancel(&mesh, &si, &cutter, &params, &|| false, None);
        super::level_contour::REGION_CONTOUR_OFF_FOR_CONTROL.with(|c| c.set(false));
        out.expect("not cancelled")
    };
    let off = run(OnePassHold::Off);
    let on = run(OnePassHold::RemainingStock {
        link_kinematics: None,
    });
    assert!(
        off.left_stock
            .as_ref()
            .is_none_or(|l| l.held_off_centres == 0),
        "the dial is off: {:?}",
        off.left_stock
    );
    assert!(
        on.left_stock
            .as_ref()
            .is_some_and(|l| l.held_off_centres > 0),
        "the dial is on: {:?}",
        on.left_stock
    );
    assert_ne!(
        off.toolpath.moves.len(),
        on.toolpath.moves.len(),
        "the rule changes the plan of this fixture"
    );
}

/// P41 (R6): the mask of the wall contour points (`OnePass::blocks`). A
/// point whose disc covers a cell that stands more than the limit above the
/// cutter is masked; the same point beside a cell one pass tall is not; with
/// the rule off nothing is masked. Tool 6.35 mm, Depth/Pass 3, clearance
/// 0.5: the limit is 3.5 mm.
#[test]
fn the_wall_contour_mask_holds_a_point_off_tall_stock_p41() {
    use super::one_pass::{OnePass, OnePassTool};
    let cutter = flat_cutter();
    let lut = RadialProfileLUT::from_cutter(&cutter, crate::stock::radial_profile::LUT_SAMPLES);
    let tool = OnePassTool {
        lut: &lut,
        radius: 3.175,
    };
    let (cell, n) = (0.5, 41usize);
    // A strip of cells at x 2.0 to 2.5 (inside the disc of a tool at the
    // origin), `strip_top` tall, on stock at Z 0.
    let stock_with = |strip_top: f64| {
        let mut tops = vec![0.0f64; n * n];
        for row in 0..n {
            for col in 0..n {
                let x = -10.0 + col as f64 * cell;
                if (2.0..=2.5).contains(&x) {
                    tops[row * n + col] = strip_top;
                }
            }
        }
        make_stock_with_cells(n, n, -10.0, -10.0, cell, -10.0, &tops)
    };
    let point = P3::new(0.0, 0.0, 0.0);
    let rule = OnePass::new(3.0, 0.5, true);
    assert!(
        rule.blocks(&stock_with(9.0), tool, point),
        "9 mm of stock above the cutter inside the disc: the point is masked"
    );
    assert_eq!(
        rule.held.borrow().centres,
        1,
        "the masked point is recorded"
    );
    assert!(
        !rule.blocks(&stock_with(3.0), tool, point),
        "one pass of stock: the point is cut"
    );
    // A tall strip outside the disc does not mask the point.
    assert!(
        !rule.blocks(&stock_with(9.0), tool, P3::new(-4.0, 0.0, 0.0)),
        "the strip is 6 mm from the centre, past the tool radius"
    );
    let off = OnePass::new(3.0, 0.5, false);
    assert!(
        !off.blocks(&stock_with(9.0), tool, point),
        "the rule is off: nothing is masked"
    );
    assert_eq!(off.held.borrow().centres, 0);
}

/// P43 fixture: a fresh block 12 mm tall over a floor at Z 0, with a
/// vertical wall at X 30 that stands to Z 12 (the stock top). Tool 18 mm
/// flat (planner cell 1.5 mm), Depth/Pass 3 mm, no stock to leave,
/// stay-down clearance 0.5 mm, tolerance 0.1 mm: the levels are 9, 6, 3 and
/// 0, and the wall is four passes deep.
///
/// The toolpath is simulated on a stock with a cell of 0.25 mm, a sixth of
/// the planner cell: a strip narrower than a planner cell is whole cells
/// there. Returns the deepest sample (`axial_engagement_mm`) and the width
/// (mm) of the wood on the line Y -20 that stands more than 1 mm above the
/// floor in front of the wall.
fn wall_fixture_p43(hold: OnePassHold) -> (f64, f64) {
    let (wall_x, top) = (30.0, 12.0);
    let vertices = vec![
        P3::new(-60.0, -60.0, 0.0),
        P3::new(wall_x, -60.0, 0.0),
        P3::new(wall_x, 60.0, 0.0),
        P3::new(-60.0, 60.0, 0.0),
        P3::new(wall_x, -60.0, top),
        P3::new(wall_x, 60.0, top),
        P3::new(60.0, -60.0, top),
        P3::new(60.0, 60.0, top),
    ];
    let triangles = vec![
        [0, 1, 2],
        [0, 2, 3],
        [1, 4, 5],
        [1, 5, 2],
        [4, 6, 7],
        [4, 7, 5],
    ];
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = FlatEndmill::new(18.0, 25.0);
    let mut params = default_params();
    params.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
    params.geometry.tool_radius = 9.0;
    params.geometry.envelope_radius = 9.0;
    params.geometry.stepover = 5.5;
    params.depth.stock_to_leave = 0.0;
    params.depth.stock_top_z = top;
    params.linking.one_pass_hold = hold;
    let out =
        adaptive_3d_toolpath_output_with_cancel(&mesh, &si, &cutter, &params, &|| false, None)
            .expect("not cancelled");
    let cell = 0.25;
    let mut sim_stock = make_stock(-69.0, -69.0, 69.0, 69.0, top, cell);
    let samples = sim_stock
        .simulate_toolpath_with_metrics_with_cancel(
            &out.toolpath,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            &[],
            false,
            &|| false,
        )
        .expect("the simulation is not cancelled");
    let deepest = samples
        .iter()
        .map(|s| s.axial_engagement_mm)
        .fold(0.0, f64::max);
    let grid = &sim_stock.z_grid;
    let row = ((-20.0 - grid.origin_v) / cell).round() as usize;
    let standing = (0..grid.cols)
        .filter(|&col| {
            let x = grid.origin_u + col as f64 * cell;
            (0.0..wall_x).contains(&x) && stock_top_z_at(&sim_stock, row, col) > 1.0
        })
        .count();
    (deepest, standing as f64 * cell)
}

/// P43: each level of a fresh Geodesic rough cuts to one line at a wall
/// deeper than one pass (`level_contour::region_wall_pass`,
/// `WallRule::FreshStock`). The line is one planner cell less
/// `fresh_line_shift` inside the raw region boundary: nearer to the wall
/// than the plan of any level, which stands one planner cell or more inside
/// it. So no later level cuts, with the edge of its disc, a strip that the
/// levels above left.
///
/// Measured on a 0.25 mm cell (the planner cell is 1.5 mm): the wood at the
/// foot of the wall is 1.75 mm wide with the rule and 2.25 mm with the dial
/// off, and no sample is deeper than one Depth/Pass plus the clearance.
/// The second assert is the red state: without the wall pass the two widths
/// are the same.
///
/// What this fixture does not show: the 9.0 mm cut of the Wanaka back.
/// There a level with another region shape ran its plan on the limit that
/// two levels above did not reach; where the last wrap of a 2D plan stands
/// depends on the region, and this fixture has one region shape.
#[test]
fn a_fresh_level_cuts_to_the_wall_line_that_no_plan_passes_p43() {
    let sim_cell = 0.25;
    let (deepest, strip_mm) = wall_fixture_p43(OnePassHold::FreshStock {
        link_kinematics: None,
    });
    let limit = 3.0 + 0.5;
    assert!(
        deepest <= limit + 1e-6,
        "a sample cuts {deepest:.2} mm deep; one Depth/Pass and the clearance are {limit} mm"
    );
    let (deepest_off, strip_off_mm) = wall_fixture_p43(OnePassHold::Off);
    assert!(deepest_off <= limit + 1e-6, "{deepest_off}");
    let shift = super::level_contour::fresh_line_shift(9.0 / 6.0, 0.1);
    assert!(shift > sim_cell, "the fixture can show the shift: {shift}");
    assert!(
        strip_mm <= strip_off_mm - sim_cell + 1e-9,
        "the wall line is nearer to the wall than the plan by {shift} mm, so the wood at the \
         foot of the wall is narrower by one simulation cell or more: {strip_mm} mm with the \
         rule, {strip_off_mm} mm with the dial off"
    );
}

/// P43: the shift of the wall line of a fresh rough
/// (`level_contour::fresh_line_shift`). It is three cut tolerances, it
/// leaves the line half a planner cell after the two tolerances of the
/// line, and it is never negative.
#[test]
fn the_fresh_wall_line_keeps_half_a_cell_from_the_raw_boundary_p43() {
    use super::level_contour::fresh_line_shift;
    // The 18 mm tool of the Wanaka back: cell 1.5 mm, tolerance 0.1 mm.
    let shift = fresh_line_shift(1.5, 0.1);
    assert!((shift - 0.3).abs() < 1e-12, "{shift}");
    assert!(1.5 - shift - 2.0 * 0.1 >= 1.5 / 2.0);
    // A 6 mm tool (cell 0.5 mm): the half cell binds.
    let small = fresh_line_shift(0.5, 0.1);
    assert!((small - 0.05).abs() < 1e-12, "{small}");
    assert!(0.5 - small - 2.0 * 0.1 >= 0.5 / 2.0 - 1e-12);
    // A cell under four tolerances: no shift, never a negative one.
    assert_eq!(fresh_line_shift(0.2, 0.1), 0.0);
    assert_eq!(fresh_line_shift(1.5, 0.0), 0.0);
}

/// P43 fixture (the perimeter sweep class): a fresh block 12 mm tall over a
/// floor at Z 0. Two rooms are joined by a throat 30 mm long and
/// `throat_mm` wide between two wall blocks that stand to Z 12 (X 10 to
/// 40). The tool-centre region of the throat is narrower than twice the
/// shortfall inset of `grown_outlines`, so the inset splits the outline and
/// the region keeps the perimeter sweep at each level. Tool 18 mm flat
/// (planner cell 1.5 mm), Depth/Pass 3 mm, no stock to leave: the levels
/// are 9, 6, 3 and 0.
///
/// Returns, after a simulation on a 0.25 mm cell: the deepest sample, the
/// width (mm) of the wood on the line Y 35 that stands more than 1 mm above
/// the floor in front of the wall block (X under 10), and the number of
/// region-levels that kept the sweep.
fn sweep_wall_fixture_p43(hold: OnePassHold, throat_mm: f64) -> (f64, f64, usize) {
    let top = 12.0;
    let half = throat_mm / 2.0;
    let mut vertices: Vec<P3> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut quad = |x0: f64, y0: f64, x1: f64, y1: f64, z: f64| {
        let i = vertices.len() as u32;
        vertices.extend([
            P3::new(x0, y0, z),
            P3::new(x1, y0, z),
            P3::new(x1, y1, z),
            P3::new(x0, y1, z),
        ]);
        triangles.extend([[i, i + 1, i + 2], [i, i + 2, i + 3]]);
    };
    quad(-60.0, -60.0, 90.0, 60.0, 0.0);
    quad(10.0, half, 40.0, 60.0, top);
    quad(10.0, -60.0, 40.0, -half, top);
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = FlatEndmill::new(18.0, 25.0);
    let mut params = default_params();
    params.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
    params.geometry.tool_radius = 9.0;
    params.geometry.envelope_radius = 9.0;
    params.geometry.stepover = 5.5;
    params.depth.stock_to_leave = 0.0;
    params.depth.stock_top_z = top;
    params.linking.one_pass_hold = hold;
    let out =
        adaptive_3d_toolpath_output_with_cancel(&mesh, &si, &cutter, &params, &|| false, None)
            .expect("not cancelled");
    let sweeps = out
        .produced
        .region_openings
        .iter()
        .filter(|o| o.cuts_sweep())
        .count();
    let cell = 0.25;
    let mut sim_stock = make_stock(-69.0, -69.0, 99.0, 69.0, top, cell);
    let samples = sim_stock
        .simulate_toolpath_with_metrics_with_cancel(
            &out.toolpath,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            &[],
            false,
            &|| false,
        )
        .expect("the simulation is not cancelled");
    let deepest = samples
        .iter()
        .map(|s| s.axial_engagement_mm)
        .fold(0.0, f64::max);
    let grid = &sim_stock.z_grid;
    let row = ((35.0 - grid.origin_v) / cell).round() as usize;
    let standing = (0..grid.cols)
        .filter(|&col| {
            let x = grid.origin_u + col as f64 * cell;
            (-20.0..10.0).contains(&x) && stock_top_z_at(&sim_stock, row, col) > 1.0
        })
        .count();
    (deepest, standing as f64 * cell, sweeps)
}

/// P43 (round 3, the perimeter sweep class): the wall line of a fresh
/// rough runs on a region that keeps the perimeter sweep too. On the sweep
/// path the plan stands a tool radius inside the region, so each level
/// leaves a band at the wall, and a lower level that opens on its grown
/// outline takes that band in one cut (measured on the Wanaka front: 5.73 mm
/// at 4.166 mm per pass). With the line each level cuts its band, one pass.
///
/// The fixture (a throat 19.5 mm wide for an 18 mm tool) keeps the sweep at
/// each of its four levels. Measured on a 0.25 mm cell: the wood at the
/// foot of the wall block is 1.25 mm wide with the rule and 9.5 mm with the
/// dial off, and no sample is deeper than one Depth/Pass plus the
/// clearance. The bound of the wood is the line (one planner cell less the
/// shift inside the raw boundary), half a planner cell (the raw boundary
/// against the true limit) and one simulation cell. Red without the line on
/// a sweep region: the band stays.
///
/// What it does not show: a throat 20.5 mm wide. There the wall line meets
/// stock in the throat that no level plans, and one sample of the last
/// level reads 7.79 mm (3.0 mm with the dial off). The fixture is in the
/// P43 note, section 10.
#[test]
fn a_fresh_level_cuts_the_band_of_a_sweep_region_p43() {
    let (cell, sim_cell) = (9.0 / 6.0, 0.25);
    let (deepest, wood_mm, sweeps) = sweep_wall_fixture_p43(
        OnePassHold::FreshStock {
            link_kinematics: None,
        },
        19.5,
    );
    assert!(
        sweeps >= 1,
        "the fixture holds a region that keeps the perimeter sweep"
    );
    let limit = 3.0 + 0.5;
    assert!(
        deepest <= limit + 1e-6,
        "a sample cuts {deepest:.2} mm deep; one Depth/Pass and the clearance are {limit} mm"
    );
    let bound = cell - super::level_contour::fresh_line_shift(cell, 0.1) + cell / 2.0 + sim_cell;
    let (_, wood_off_mm, sweeps_off) = sweep_wall_fixture_p43(OnePassHold::Off, 19.5);
    assert!(sweeps_off >= 1);
    assert!(
        wood_mm <= bound + 1e-9,
        "{wood_mm} mm of wood stands at the foot of the wall with the rule ({wood_off_mm} mm \
         with the dial off); the bound is {bound} mm"
    );
    assert!(
        wood_off_mm > bound,
        "the control: with no line the band of the sweep stays: {wood_off_mm} mm"
    );
}

/// P45 fixture: the wall of the P42 fixture (X 10, top Z 9) and a rest
/// stock where a larger tool left a strip at the foot of the wall (X 8..10,
/// Y -10..10). A block 3 mm tall stands away from the wall (X -20..-5), so
/// that the plan has a level. No tool centre can stand over the strip (the
/// tool-centre limit is X 6.825), so the strip makes no region at any level.
///
/// `sliver: None`: the strip is 6 mm tall.
///
/// `sliver: Some(_)`: the fixture of the replay failure of P44 build 2. The
/// strip is 4 mm tall and its first column (X 8.0) stands 9 mm tall. Three
/// earlier cuts (Z 6, 3 and 0.5) passed with the disc edge at X 7.95: the
/// centre of that column is 0.05 mm outside each disc, so the wood at the
/// centre keeps its 9 mm; the stamp covers a quarter of the cell and lowers
/// its ray top by a quarter of the stock above the cutter each time (to Z
/// 5.33).
///
/// Returns the deepest cut of a node by one fed move, the highest stock in
/// the strip on the line Y 0 between X 8.0 and 9.4 after the simulation,
/// and the ray top of the sliver column before the op. The deepest cut is a
/// point sample, as the replay instrument reads it
/// (`planning/feeds_contract_2026-11-03/phases/p43/deepc.py`): each cell
/// centre is a node; a fed move lowers each node under the flat tool to the
/// lowest Z of the move over it; a node counts when it stands the cut
/// tolerance or more inside the disc edge of the move. It does not read the
/// planner stock.
fn wall_strip_fixture_p45(sliver: bool) -> (f64, f64, f64) {
    let (wall_x, wall_top) = (10.0, 9.0);
    let vertices = vec![
        P3::new(-40.0, -40.0, 0.0),
        P3::new(wall_x, -40.0, 0.0),
        P3::new(wall_x, 40.0, 0.0),
        P3::new(-40.0, 40.0, 0.0),
        P3::new(wall_x, -40.0, wall_top),
        P3::new(wall_x, 40.0, wall_top),
        P3::new(40.0, -40.0, wall_top),
        P3::new(40.0, 40.0, wall_top),
    ];
    let triangles = vec![
        [0, 1, 2],
        [0, 2, 3],
        [1, 4, 5],
        [1, 5, 2],
        [4, 6, 7],
        [4, 7, 5],
    ];
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = flat_cutter();
    let (cell, n) = (0.2, 351usize);
    let xy = |i: usize| -35.0 + i as f64 * cell;
    let sliver_col = (0..n)
        .find(|&col| (xy(col) - 8.0).abs() < 1e-6)
        .expect("a column at X 8.0");
    let mut tops = vec![0.0f64; n * n];
    for row in 0..n {
        for col in 0..n {
            let (x, y) = (xy(col), xy(row));
            tops[row * n + col] = if x > wall_x {
                wall_top
            } else if x >= 8.0 - 1e-9 && (-10.0..=10.0).contains(&y) {
                match (sliver, col == sliver_col) {
                    (false, _) => 6.0,
                    (true, true) => 9.0,
                    (true, false) => 4.0,
                }
            } else if (-20.0..=-5.0).contains(&x) && (-12.0..=12.0).contains(&y) {
                3.0
            } else {
                0.0
            };
        }
    }
    let mut stock = make_stock_with_cells(n, n, -35.0, -35.0, cell, -10.0, &tops);
    let mut params = default_params();
    if sliver {
        let lut = RadialProfileLUT::from_cutter(&cutter, crate::stock::radial_profile::LUT_SAMPLES);
        let r = params.geometry.tool_radius;
        for z in [6.0, 3.0, 0.5] {
            stock.stamp_linear_segment(
                &lut,
                r,
                P3::new(7.95 - r, -14.0, z),
                P3::new(7.95 - r, 14.0, z),
                StockCutDirection::FromTop,
            );
        }
    }
    let ray_top_before = stock_top_z_at(&stock, 175, sliver_col);
    params.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
    params.linking.one_pass_hold = OnePassHold::RemainingStock {
        link_kinematics: None,
    };
    params.depth.stock_top_z = 9.0;
    params.initial_stock = Some(stock.clone());
    let out =
        adaptive_3d_toolpath_output_with_cancel(&mesh, &si, &cutter, &params, &|| false, None)
            .expect("not cancelled");
    // The point-sample replay on the nodes.
    let (r, tol) = (params.geometry.tool_radius, params.geometry.tolerance);
    let mut nodes = tops;
    let mut deepest = 0.0f64;
    let mut pos: Option<P3> = None;
    let index = |v: f64, up: bool| -> usize {
        let i = (v + 35.0) / cell;
        let i = if up { i.ceil() } else { i.floor() };
        i.clamp(0.0, (n - 1) as f64) as usize
    };
    for m in &out.toolpath.moves {
        let (a, b) = (pos.unwrap_or(m.target), m.target);
        pos = Some(b);
        if matches!(m.move_type, crate::toolpath::MoveType::Rapid) {
            continue;
        }
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len_sq = dx * dx + dy * dy;
        for row in index(a.y.min(b.y) - r, false)..=index(a.y.max(b.y) + r, true) {
            for col in index(a.x.min(b.x) - r, false)..=index(a.x.max(b.x) + r, true) {
                let (x, y) = (xy(col), xy(row));
                // The part of the move with the node under the disc.
                let (t_lo, t_hi, inside) = if len_sq > 1e-18 {
                    let t = ((x - a.x) * dx + (y - a.y) * dy) / len_sq;
                    let perp_sq = ((x - a.x) * dy - (y - a.y) * dx).powi(2) / len_sq;
                    if perp_sq > r * r {
                        continue;
                    }
                    let half = ((r * r - perp_sq) / len_sq).sqrt();
                    if t + half < 0.0 || t - half > 1.0 {
                        continue;
                    }
                    let tc = t.clamp(0.0, 1.0);
                    let d = (x - (a.x + tc * dx)).hypot(y - (a.y + tc * dy));
                    (
                        (t - half).clamp(0.0, 1.0),
                        (t + half).clamp(0.0, 1.0),
                        r - d,
                    )
                } else {
                    let d = (x - a.x).hypot(y - a.y);
                    if d > r {
                        continue;
                    }
                    (0.0, 1.0, r - d)
                };
                let z = (a.z + t_lo * (b.z - a.z)).min(a.z + t_hi * (b.z - a.z));
                let node = &mut nodes[row * n + col];
                if z < *node {
                    if inside >= tol {
                        deepest = deepest.max(*node - z);
                    }
                    *node = z;
                }
            }
        }
    }
    let row = 175; // Y 0
    let highest = (0..n)
        .filter(|&col| (8.0 - 1e-9..=9.4).contains(&xy(col)))
        .map(|col| nodes[row * n + col])
        .fold(f64::NEG_INFINITY, f64::max);
    (deepest, highest, ray_top_before)
}

/// P45: a rough on remaining stock cuts the strip that stands beside a wall
/// where no region of a level holds it (`level_contour::strip_pass`), one
/// Depth/Pass per round. The contour runs one planner cell plus the cut
/// tolerance inside the tool-centre limit, so the tool reaches X 9.7; the
/// read is to X 9.4. The control (no strip pass): the strip keeps its 6 mm.
#[test]
fn a_strip_beside_a_wall_with_no_region_is_cut_p45() {
    let (deepest, highest, _) = wall_strip_fixture_p45(false);
    assert!(
        highest <= 0.5 + 0.3,
        "the strip stands at Z {highest:.2} between X 8.0 and 9.4; the floor is 0.5"
    );
    assert!(
        deepest <= 3.0 + 0.5 + 1e-6,
        "a move cuts a node {deepest:.2} mm deep; one Depth/Pass and the clearance are 3.5 mm"
    );
    super::level_contour::STRIP_PASS_OFF_FOR_CONTROL.with(|c| c.set(true));
    let (_, control, _) = wall_strip_fixture_p45(false);
    super::level_contour::STRIP_PASS_OFF_FOR_CONTROL.with(|c| c.set(false));
    assert!(
        control >= 6.0 - 1e-6,
        "the control: with no strip pass the strip keeps its height: {control:.2}"
    );
}

/// P45, the fixture of the replay failure of P44 build 2: the strip pass
/// reads the stock at each cell centre in its disc (`StripTops`), not the
/// ray top. The ray top of the sliver column is a blend (Z 5.33; the wood
/// at its centre stands at Z 9). With the ray top the first round runs at Z
/// 2.33 and takes 6.67 mm from that centre at 3 mm per pass.
#[test]
fn the_strip_pass_reads_the_stock_at_the_cell_centre_p45() {
    let (deepest, _, ray_top) = wall_strip_fixture_p45(true);
    assert!(
        (5.0..6.0).contains(&ray_top),
        "the fixture: the ray top of the sliver column is a blend under its 9 mm: {ray_top:.2}"
    );
    assert!(
        deepest <= 3.0 + 0.5 + 1e-6,
        "a move cuts a node {deepest:.2} mm deep; one Depth/Pass and the clearance are 3.5 mm"
    );
}

/// P45: the strip state follows the cuts of each round
/// (`StripTops::scan`), so the sliver column goes down one Depth/Pass per
/// round to the floor. With no scan the state keeps the 9 mm of the column
/// after the first round, the Z of the second round is the Z of the first,
/// and the pass stops with the column at Z 6.
#[test]
fn the_strip_state_follows_each_round_p45() {
    let (_, highest, _) = wall_strip_fixture_p45(true);
    assert!(
        highest <= 0.5 + 0.3,
        "the strip stands at Z {highest:.2} between X 8.0 and 9.4; the floor is 0.5"
    );
}

/// P46 test fixture: the owner of a `ClearZLevelContext` for a test that
/// calls a private planner function of this folder (`level_contour`:
/// `wall_read`, `emit_runs`). The op is a Geodesic Adaptive rough on
/// remaining stock (the one-pass rule on) with a flat tool of 6 mm on a
/// flat model at Z 0, no stock to leave, 3 mm per pass and 0.5 mm of
/// stay-down clearance. Every other field is the neutral value: no boundary
/// clip, no entry rule, no stay-down link, no band.
pub(super) struct ContextFixture {
    mesh: crate::mesh::TriangleMesh,
    index: SpatialIndex,
    cutter: FlatEndmill,
    luts: super::path::PlannerLuts,
    params: Adaptive3dParams,
}

impl ContextFixture {
    pub(super) fn new() -> Self {
        let mesh = crate::mesh::make_test_flat(120.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let cutter = FlatEndmill::new(6.0, 25.0);
        let luts = super::path::PlannerLuts::of(&cutter);
        let mut params = default_params();
        params.geometry.tool_radius = 3.0;
        params.geometry.envelope_radius = 3.0;
        params.depth.stock_to_leave = 0.0;
        params.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
        params.linking.one_pass_hold = OnePassHold::RemainingStock {
            link_kinematics: None,
        };
        Self {
            mesh,
            index,
            cutter,
            luts,
            params,
        }
    }

    pub(super) fn ctx(&self) -> super::clearing::ClearZLevelContext<'_> {
        let p = &self.params;
        super::clearing::ClearZLevelContext {
            mesh: &self.mesh,
            index: &self.index,
            cutter: &self.cutter,
            probe: None,
            lut: &self.luts.cut,
            entry_lut: self.luts.entry(),
            debug: None,
            tool_radius: p.geometry.tool_radius,
            stepover: p.geometry.stepover,
            stock_to_leave: p.depth.stock_to_leave,
            depth_per_pass: p.depth.depth_per_pass,
            tolerance: p.geometry.tolerance,
            cut_tolerance: p.emission_chord_tolerance(),
            feed_rate: p.feed_rate,
            plunge_rate: p.plunge_rate,
            step_len: 0.3,
            max_link_dist: 0.0,
            safe_z: p.safe_z,
            clearing_strategy: p.clearing_strategy,
            trochoid_cap_mult: p.trochoid_cap_mult,
            engagement_measure: p.engagement_measure,
            min_cutting_radius: p.geometry.min_cutting_radius,
            min_region_cut_length_mm: p.linking.min_region_cut_length_mm,
            entry_floor_radius: p.geometry.tool_radius,
            entry_bore: None,
            entry_ramp: None,
            stay_down_mm: 0.0,
            centre_clip: None,
            stay_down_clearance_mm: p.linking.stay_down_clearance_mm,
            step_split: super::effective_step_split(p),
            keep_down_route: super::effective_keep_down_route(p),
            region_clip: None,
            left_pieces: std::cell::RefCell::new(Vec::new()),
            unplanned: std::cell::RefCell::new(Vec::new()),
            dropped_cut_groups: std::cell::Cell::new(0),
            region_openings: std::cell::RefCell::new(Vec::new()),
            level_probe: None,
            band_edge: std::cell::RefCell::new(None),
            rest_region_filter: None,
            one_pass: super::one_pass::OnePass::new(
                p.depth.depth_per_pass,
                p.linking.stay_down_clearance_mm,
                true,
            ),
            wall_pass: Some(super::level_contour::WallPass {
                link_kinematics: None,
                rule: super::level_contour::WallRule::RemainingStock,
            }),
        }
    }
}

/// P46 fixture: the wall of the P45 fixture (X 10, top Z 9; the limit of
/// the tool centre is X 6.825) and a rest stock 3 mm tall (under one
/// Depth/Pass above the floor Z 0.5) with:
/// - a strip at the foot of the wall (X 8..10, Y -10..10) that makes no
///   region;
/// - a "near" bump beside it (X 4.6..8, Y -2..2). Its region is 2.2 x 4 mm
///   of tool centres: the contour line is under the dial of 15 mm, so the
///   plan leaves it (a left piece);
/// - a "far" bump with the same region, away from every wall
///   (X -32..-29.8, Y 20..24): a left piece too;
/// - a block (X -20..-5, Y -12..12), so that the plan has a level.
///
/// The strip pass joins the near bump and the strip cells in one band and
/// cuts it. The band of the far bump is under the dial: no cut.
///
/// Returns the published rest regions and the left-stock record.
fn left_piece_fixture_p46() -> (Vec<crate::polygon::Polygon2>, Option<LeftStock>) {
    let (wall_x, wall_top) = (10.0, 9.0);
    let vertices = vec![
        P3::new(-40.0, -40.0, 0.0),
        P3::new(wall_x, -40.0, 0.0),
        P3::new(wall_x, 40.0, 0.0),
        P3::new(-40.0, 40.0, 0.0),
        P3::new(wall_x, -40.0, wall_top),
        P3::new(wall_x, 40.0, wall_top),
        P3::new(40.0, -40.0, wall_top),
        P3::new(40.0, 40.0, wall_top),
    ];
    let triangles = vec![
        [0, 1, 2],
        [0, 2, 3],
        [1, 4, 5],
        [1, 5, 2],
        [4, 6, 7],
        [4, 7, 5],
    ];
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = flat_cutter();
    let (cell, n) = (0.2, 351usize);
    let xy = |i: usize| -35.0 + i as f64 * cell;
    let mut tops = vec![0.0f64; n * n];
    for row in 0..n {
        for col in 0..n {
            let (x, y) = (xy(col), xy(row));
            let e = 1e-9;
            let strip = x >= 8.0 - e && (-10.0 - e..=10.0 + e).contains(&y);
            let near = (4.6 - e..8.0).contains(&x) && (-2.0 - e..=2.0 + e).contains(&y);
            let far = (-32.0 - e..=-29.8 + e).contains(&x) && (20.0 - e..=24.0 + e).contains(&y);
            let block = (-20.0..=-5.0).contains(&x) && (-12.0..=12.0).contains(&y);
            tops[row * n + col] = if x > wall_x {
                wall_top
            } else if strip || near || far || block {
                3.0
            } else {
                0.0
            };
        }
    }
    let mut params = default_params();
    params.clearing_strategy = ClearingStrategy3d::GeodesicAdaptive;
    params.linking.one_pass_hold = OnePassHold::RemainingStock {
        link_kinematics: None,
    };
    params.linking.min_region_cut_length_mm = 15.0;
    params.depth.stock_top_z = 9.0;
    params.initial_stock = Some(make_stock_with_cells(
        n, n, -35.0, -35.0, cell, -10.0, &tops,
    ));
    let out =
        adaptive_3d_toolpath_output_with_cancel(&mesh, &si, &cutter, &params, &|| false, None)
            .expect("not cancelled");
    (
        out.produced
            .rest_regions
            .map(|r| r.as_ref().clone())
            .unwrap_or_default(),
        out.left_stock,
    )
}

/// P46 (P45 decision D-10): a left piece that the strip pass cut is not a
/// left piece (`level_contour::unleave_cut_pieces`). The plan leaves the
/// near bump and the far bump (each under the dial). The strip pass cuts
/// the near bump to its floor with the strip beside it, so the near bump is
/// not published as a rest region. The far bump is not cut (its band is
/// under the dial too): it stays a left piece and is published. The control
/// (no strip pass): both are published.
#[test]
fn a_left_piece_that_the_strip_pass_cut_is_not_published_p46() {
    let (near, far) = (
        crate::geo::P2::new(5.7, 0.0),
        crate::geo::P2::new(-31.0, 22.0),
    );
    let holds = |regions: &[crate::polygon::Polygon2], p: &crate::geo::P2| {
        regions.iter().any(|r| r.contains_point(p))
    };
    super::level_contour::STRIP_PASS_OFF_FOR_CONTROL.with(|c| c.set(true));
    let (control, _) = left_piece_fixture_p46();
    super::level_contour::STRIP_PASS_OFF_FOR_CONTROL.with(|c| c.set(false));
    assert!(
        holds(&control, &near) && holds(&control, &far),
        "the control: with no strip pass the plan leaves both bumps: {control:?}"
    );
    let (regions, left) = left_piece_fixture_p46();
    assert!(
        holds(&regions, &far),
        "the far bump is not cut: it stays a published rest region: {regions:?}"
    );
    assert!(
        !holds(&regions, &near),
        "the strip pass cut the near bump: it is not a rest region: {regions:?}"
    );
    // The drop record: the far bump (the plan) and its band (the strip
    // pass). The record of the near bump is gone.
    let left = left.expect("the plan dropped a region");
    assert_eq!(left.short_forecast, 2, "{left:?}");
}

/// P46 (P45 decision D-4): a place whose cells are all rim cells (strip
/// cells nearer to a wall than a contour line stands) takes the reason
/// `PlaceWhy::LineMargin`, not the reason of the drop record on its cells.
/// Cells of 0.5 mm, cut tolerance 0.1 mm: the line stands 0.6 mm inside the
/// limit of the tool centre, so only the first column beside a wall is rim.
/// The wall: the tool-centre floor steps from 0 to 9 at X 10.
/// - Place A: the column X 9.5 (0.25 mm from the limit). Rim.
/// - Place B: the column X 3.0, no wall near. It keeps the record reason.
/// - Place C: the columns X 9.0 and 9.5 beside a second wall stretch. The
///   column X 9.0 is 0.75 mm from the limit, where a line can run: the
///   place keeps the record reason.
///
/// The count of the places is the same with and without the rule.
#[test]
fn a_rim_place_has_the_reason_of_the_line_margin_p46() {
    let (n, cell) = (60usize, 0.5);
    let x_of = |col: usize| col as f64 * cell;
    let in_a = |x: f64, y: f64| (x - 9.5).abs() < 1e-9 && (2.0..=8.0).contains(&y);
    let in_b = |x: f64, y: f64| (x - 3.0).abs() < 1e-9 && (2.0..=8.0).contains(&y);
    let in_c =
        |x: f64, y: f64| (9.0 - 1e-9..=9.5 + 1e-9).contains(&x) && (14.0..=20.0).contains(&y);
    let mut padded = vec![false; (n + 2) * (n + 2)];
    let mut height = vec![0.0f32; n * n];
    let mut floors = vec![0.0f64; n * n];
    for row in 0..n {
        for col in 0..n {
            let (x, y) = (x_of(col), row as f64 * cell);
            if x >= 10.0 - 1e-9 {
                floors[row * n + col] = 9.0;
            }
            if in_a(x, y) || in_b(x, y) || in_c(x, y) {
                padded[(row + 1) * (n + 2) + (col + 1)] = true;
                height[row * n + col] = 4.0;
            }
        }
    }
    // No material at a tool centre: each standing cell is a strip cell (the
    // stock stands in its disc).
    let no_material = vec![0.0f64; n * n];
    let stock = make_stock_with_cells(n, n, 0.0, 0.0, cell, -10.0, &no_material);
    let surface_hm = SurfaceHeightmap::from_parts(floors, vec![true; n * n], n, n, 0.0, 0.0, cell);
    let one_pass = super::one_pass::OnePass::new(3.0, 0.5, true);
    let standing = || {
        Some(super::left_stock::StandingRead {
            standing: super::standing::Standing {
                padded: padded.clone(),
                padded_rows: n + 2,
                padded_cols: n + 2,
                height: height.clone(),
                count: padded.iter().filter(|&&c| c).count(),
            },
            z_level: 0.0,
            line_inset_mm: super::level_contour::contour_line_inset(cell, 0.1),
        })
    };
    let record = |x0: f64, x1: f64, y0: f64, y1: f64| super::left_stock::Unplanned {
        z_level: 0.0,
        outline: crate::polygon::Polygon2::new(vec![
            crate::geo::P2::new(x0, y0),
            crate::geo::P2::new(x1, y0),
            crate::geo::P2::new(x1, y1),
            crate::geo::P2::new(x0, y1),
        ]),
        why: LeftWhy::ShortForecast,
    };
    let left = super::left_stock::left_stock(
        &[
            record(9.25, 9.75, 1.0, 9.0),
            record(2.75, 3.25, 1.0, 9.0),
            record(8.75, 9.75, 13.0, 21.0),
        ],
        0,
        &one_pass,
        &stock,
        &surface_hm,
        0.0,
        standing,
        3.0,
    )
    .expect("the plan recorded three runs");
    assert_eq!(
        (left.places_total, left.unrecorded_places),
        (3, 0),
        "{left:?}"
    );
    let why_at = |x: f64, y: f64| {
        left.places
            .iter()
            .find(|p| p.lo[0] <= x && x <= p.hi[0] && p.lo[1] <= y && y <= p.hi[1])
            .map(|p| p.why)
    };
    assert_eq!(why_at(9.5, 5.0), Some(PlaceWhy::LineMargin), "{left:?}");
    assert_eq!(why_at(3.0, 5.0), Some(PlaceWhy::ShortForecast), "{left:?}");
    assert_eq!(why_at(9.0, 17.0), Some(PlaceWhy::ShortForecast), "{left:?}");
    assert!(left.stands(), "{left:?}");
    let text = crate::compute::toolpath_stats::RoughLeftStockFinding {
        left,
        regions_published: false,
    }
    .to_string();
    assert!(
        text.contains("it is nearer to a wall than a contour line of the planner can run"),
        "{text}"
    );
}
