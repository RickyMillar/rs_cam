//! Regression tests for the drop-cutter 3D-finish correctness.
//!
//! 1. `drop_cutter_does_not_cut_outside_mesh_footprint` — guards against
//!    rim-contact carving a trench around the mesh (the "red border"
//!    symptom that was fixed in an earlier commit).
//! 2. `drop_cutter_over_peak_stays_at_peak_height` — sanity check that
//!    the drop-cutter math reaches a mesh peak with a tapered tool.
//! 3. `drop_cutter_toolpath_stamp_no_dive_below_mesh` — FAILING: stamps
//!    3D Finish 8 onto a dexel stock and detects stock columns carved
//!    significantly below the mesh surface. This catches the user-
//!    reported "flattening" where the tool's shaft radius (3.175 mm for
//!    a tapered ball) forces the tool into a valley next to a ridge, and
//!    the stamp's LUT extends across the ridge column, carving it down
//!    to valley depth even though the mesh there is a peak. Kept as a
//!    failing test so the fix is actionable.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::geo::{P2, P3};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::{MoveType, Toolpath};

fn fixture_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

fn pyramid_mesh(cx: f64, cy: f64, r: f64, peak_z: f64, segments: usize) -> TriangleMesh {
    let mut verts = vec![P3::new(cx, cy, peak_z)];
    for i in 0..segments {
        let theta = 2.0 * std::f64::consts::PI * i as f64 / segments as f64;
        verts.push(P3::new(cx + r * theta.cos(), cy + r * theta.sin(), 0.0));
    }
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for i in 0..segments {
        let a = 1 + i as u32;
        let b = 1 + ((i + 1) % segments) as u32;
        tris.push([0, a, b]);
    }
    TriangleMesh::from_raw(verts, tris)
}

fn point_over_mesh(x: f64, y: f64, mesh: &TriangleMesh, index: &SpatialIndex) -> bool {
    for &idx in &index.query(x, y, 0.0) {
        let tri = &mesh.faces[idx];
        if tri.contains_point_xy(x, y) {
            return true;
        }
    }
    false
}

#[test]
fn drop_cutter_does_not_cut_outside_mesh_footprint() {
    let path = fixture_path("test_job.toml");
    let mut session = ProjectSession::load(&path).expect("project loads");
    let (finish_idx, _) = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .find(|(_, tc)| tc.id == 17)
        .expect("3D Finish 8 exists");

    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(finish_idx, &cancel)
        .expect("3D Finish generates");

    let mesh =
        TriangleMesh::from_stl_scaled(&fixture_path("terrain.stl"), 1.0).expect("stl loads");
    let index = SpatialIndex::build_auto(&mesh);

    let final_tp: &Toolpath = &result.toolpath;
    const CUT_Z_THRESHOLD: f64 = 9.5;

    let mut off_mesh = Vec::new();
    let mut prev: Option<P3> = None;
    for mv in &final_tp.moves {
        let target = mv.target;
        if matches!(mv.move_type, MoveType::Linear { .. })
            && let Some(p) = prev
            && p.z < CUT_Z_THRESHOLD
            && target.z < CUT_Z_THRESHOLD
        {
            let dxy = ((target.x - p.x).powi(2) + (target.y - p.y).powi(2)).sqrt();
            let samples = ((dxy / 0.5).ceil() as usize).max(1);
            for i in 0..=samples {
                let t = i as f64 / samples as f64;
                let x = p.x + t * (target.x - p.x);
                let y = p.y + t * (target.y - p.y);
                if !point_over_mesh(x, y, &mesh, &index) {
                    off_mesh.push(P2::new(x, y));
                    break;
                }
            }
        }
        prev = Some(target);
    }
    let total_linear = final_tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .count();
    let pct = (off_mesh.len() as f64 / total_linear.max(1) as f64) * 100.0;
    println!(
        "{}/{} Linear moves off-mesh ({:.2}%)",
        off_mesh.len(),
        total_linear,
        pct
    );
    assert!(
        pct < 1.0,
        "3D finish cutting outside the mesh footprint: {}/{total_linear} ({pct:.2}%)",
        off_mesh.len()
    );
}

#[test]
fn drop_cutter_over_peak_stays_at_peak_height() {
    use rs_cam_core::tool::TaperedBallEndmill;
    let mesh = pyramid_mesh(0.0, 0.0, 10.0, 5.0, 32);
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = TaperedBallEndmill::new(1.0, 15.0, 6.35, 25.0);
    let cl = rs_cam_core::dropcutter::point_drop_cutter(0.0, 0.0, &mesh, &index, &cutter);
    assert!(cl.contacted);
    assert!(
        (cl.z - 5.0).abs() < 0.05,
        "tool tip should reach peak at z=5.0, got {:.3}",
        cl.z
    );
}

#[test]
#[ignore = "known-failing: drop_cutter lateral stamping carves ridges next to valleys — tracked as the '3D Finish flattening' bug"]
fn drop_cutter_toolpath_stamp_no_dive_below_mesh() {
    use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
    use rs_cam_core::geo::BoundingBox3;
    use rs_cam_core::tool::TaperedBallEndmill;

    let path = fixture_path("test_job.toml");
    let mut session = ProjectSession::load(&path).expect("project loads");
    let (finish_idx, _) = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .find(|(_, tc)| tc.id == 17)
        .expect("3D Finish 8 exists");

    let cancel = AtomicBool::new(false);
    let tp = std::sync::Arc::clone(
        &session
            .generate_toolpath(finish_idx, &cancel)
            .expect("generates")
            .toolpath,
    );

    let world_mesh = TriangleMesh::from_stl_scaled(&fixture_path("terrain.stl"), 1.0)
        .expect("stl loads");
    let setup = session
        .list_setups()
        .iter()
        .find(|s| s.toolpath_indices.contains(&finish_idx))
        .expect("setup found")
        .clone();
    let xform = session.setup_transform_info(setup.face_up, setup.z_rotation);
    let needs_transform = setup.face_up != rs_cam_core::compute::transform::FaceUp::Top
        || setup.z_rotation != rs_cam_core::compute::transform::ZRotation::Deg0;
    let mesh = if needs_transform {
        xform.apply_to_mesh(&world_mesh)
    } else {
        world_mesh.clone()
    };
    let index = SpatialIndex::build_auto(&mesh);

    let bbox = BoundingBox3 {
        min: P3::new(-5.0, -5.0, -20.0),
        max: P3::new(115.0, 115.0, 12.0),
    };
    let mut stock = TriDexelStock::from_bounds(&bbox, 0.5);
    let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.35, 25.0);
    let _ = stock.simulate_toolpath(&tp, &cutter, StockCutDirection::FromTop);

    let grid = &stock.z_grid;
    const MAX_DIVE_MM: f64 = 0.6;
    let mut dives = 0usize;
    let mut worst = (0.0f64, 0.0, 0.0, 0.0, 0.0);
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let Some(carved_top) = grid.top_z_at(row, col) else { continue };
            let x = grid.origin_u + col as f64 * grid.cell_size + grid.cell_size * 0.5;
            let y = grid.origin_v + row as f64 * grid.cell_size + grid.cell_size * 0.5;
            let mut ray_z: Option<f64> = None;
            for &tri_idx in &index.query(x, y, 0.0) {
                let tri = &mesh.faces[tri_idx];
                if tri.contains_point_xy(x, y)
                    && let Some(z) = tri.z_at_xy(x, y)
                {
                    ray_z = Some(ray_z.map_or(z, |prev: f64| prev.max(z)));
                }
            }
            let Some(ray_z) = ray_z else { continue };
            let dive = ray_z - carved_top as f64;
            if dive > MAX_DIVE_MM {
                dives += 1;
                if dive > worst.0 {
                    worst = (dive, x, y, carved_top as f64, ray_z);
                }
            }
        }
    }
    println!(
        "3D Finish stock dive columns > {:.1}mm below mesh: {} (worst dive={:.2}mm at ({:.2},{:.2}) carved={:.2} mesh={:.2})",
        MAX_DIVE_MM, dives, worst.0, worst.1, worst.2, worst.3, worst.4
    );
    assert_eq!(
        dives, 0,
        "drop_cutter stamp carves stock >{MAX_DIVE_MM} mm below the mesh surface at {} columns",
        dives
    );
}
