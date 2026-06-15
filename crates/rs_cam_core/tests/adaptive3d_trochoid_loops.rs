//! Nibble visualisation — the 3D ContourSpiral path collects trochoidal
//! relief-loop centres end to end (spiral → clearing slice lift →
//! per-toolpath accumulation → structured-generator return), so the GUI
//! "Nibble" widget can draw the real loop count + placement.
//!
//! On open/convex terrain the spiral holds engagement flat on wrap spacing
//! alone and fires few or zero loops (correct — relief is load-triggered,
//! not guaranteed). To exercise the collection deterministically this test
//! drives a CONCAVE L-shape at a sub-target cap, where the frontier
//! collapses at the inner corner and forces relief loops.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::manual_range_contains
)]

use rs_cam_core::{
    adaptive3d::{
        Adaptive3dParams, ClearingStrategy3d, EntryStyle3d, RegionOrdering,
        adaptive_3d_toolpath_structured_annotated_traced_with_cancel,
    },
    geo::P3,
    mesh::{SpatialIndex, TriangleMesh},
    tool::{FlatEndmill, MillingCutter},
};

const LX: f64 = 60.0;
const LY: f64 = 60.0;
const LARM: f64 = 24.0;
const ZMAX: f64 = 2.0;

fn inside_l_shape(x: f64, y: f64) -> bool {
    if !(0.0..=LX).contains(&x) || !(0.0..=LY).contains(&y) {
        return false;
    }
    if y <= LARM { x <= LX } else { x <= LARM }
}

/// Concave L-shape mesh with a shallow sine-bump terrain (same fixture
/// family as `agent_search_coverage.rs`).
fn make_l_terrain_mesh() -> (TriangleMesh, SpatialIndex) {
    let n: usize = 24;
    let dx = LX / n as f64;
    let dy = LY / n as f64;
    let height = |x: f64, y: f64| -> f64 {
        let kx = std::f64::consts::TAU * x / LX;
        let ky = std::f64::consts::TAU * y / LY;
        ((kx.sin() * ky.sin()).abs()) * ZMAX
    };
    let mut verts: Vec<P3> = Vec::with_capacity((n + 1) * (n + 1));
    for j in 0..=n {
        for i in 0..=n {
            let x = i as f64 * dx;
            let y = j as f64 * dy;
            verts.push(P3::new(x, y, height(x, y)));
        }
    }
    let idx = |i: usize, j: usize| -> u32 { (j * (n + 1) + i) as u32 };
    let center_inside = |i: usize, j: usize| -> bool {
        inside_l_shape((i as f64 + 0.5) * dx, (j as f64 + 0.5) * dy)
    };
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for j in 0..n {
        for i in 0..n {
            if !center_inside(i, j) {
                continue;
            }
            let a = idx(i, j);
            let b = idx(i + 1, j);
            let c = idx(i + 1, j + 1);
            let d = idx(i, j + 1);
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    let mesh = TriangleMesh::from_raw(verts, tris);
    let si = SpatialIndex::build(&mesh, LX / 4.0);
    (mesh, si)
}

fn spiral_params(cutter: &FlatEndmill, cap: f64) -> Adaptive3dParams {
    let stock_top_z = 12.0;
    Adaptive3dParams {
        tool_radius: cutter.radius(),
        envelope_radius: cutter.radius(),
        stepover: cutter.radius() * 0.5,
        depth_per_pass: 3.0,
        stock_to_leave: 0.5,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        safe_z: stock_top_z + 5.0,
        tolerance: 0.25,
        min_cutting_radius: 0.0,
        stock_top_z,
        z_floor: None,
        entry_style: EntryStyle3d::Plunge,
        fine_stepdown: None,
        detect_flat_areas: false,
        max_stay_down_dist: None,
        region_ordering: RegionOrdering::Global,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::ContourSpiral,
        trochoid_cap_mult: cap,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::LeadingArc,
        z_blend: false,
        boundary: None,
        mill_shallow_areas: false,
        shallow_angle_rad: None,
        shallow_stepdown: None,
        world_stock_xy_bbox: None,
        min_region_cut_length_mm: 0.0,
        max_stay_down_distance_mm: Some(0.0),
        stay_down_clearance_mm: 0.5,
    }
}

fn run(cap: f64) -> (TriangleMesh, Vec<P3>) {
    let (mesh, si) = make_l_terrain_mesh();
    let cutter = FlatEndmill::new(6.0, 25.0);
    let params = spiral_params(&cutter, cap);
    let never_cancel = || false;
    let (_tp, _ann, _eng, loops) = adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
        &mesh,
        &si,
        &cutter,
        &params,
        &never_cancel,
        None,
    )
    .expect("spiral generation should not cancel");
    (mesh, loops)
}

#[test]
fn contour_spiral_3d_collects_trochoid_loop_centres() {
    // Sub-target cap on a concave L ⇒ relief loops MUST fire and the full
    // spiral→clearing→accumulate→return path must surface them.
    let (mesh, tight) = run(0.05);
    assert!(
        !tight.is_empty(),
        "3D ContourSpiral at a sub-target cap recorded no trochoid loops — \
         the loop-centre plumbing is broken (spiral fired loops but they \
         never reached the toolpath)"
    );
    // Centres are finite and inside the mesh XY footprint (+ tool radius
    // margin for boundary wraps).
    let b = &mesh.bbox;
    for c in &tight {
        assert!(c.x.is_finite() && c.y.is_finite() && c.z.is_finite());
        assert!(
            c.x >= b.min.x - 3.5
                && c.x <= b.max.x + 3.5
                && c.y >= b.min.y - 3.5
                && c.y <= b.max.y + 3.5,
            "loop centre ({}, {}) outside the part bbox",
            c.x,
            c.y
        );
    }

    // Relaxing the cap can only reduce the loop count (monotone lever).
    let (_, relaxed) = run(3.0);
    assert!(
        relaxed.len() <= tight.len(),
        "relaxed cap fired more loops ({}) than tight ({})",
        relaxed.len(),
        tight.len()
    );
}
