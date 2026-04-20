//! Regression test for the "3D finish cuts beyond the mesh footprint"
//! symptom: the drop-cutter's cutter-radius query returns triangles at
//! the edge of the mesh when the sample (x, y) is just outside it, and
//! the tool picks up a contact on the rim. Those CLs are ABOVE min_z
//! so `min_z_filter` keeps them, and the finish toolpath carves a
//! trench around the model that isn't in the mesh.
//!
//! Detection strategy: generate 3D Finish 8 on the user's live project,
//! then for every Linear feed-move endpoint at cut depth, check whether
//! the point's vertical ray actually hits a triangle of the mesh. Any
//! feed endpoint *outside* the mesh's XY footprint is a phantom cut —
//! the tool is riding the rim.

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

/// True iff (x, y) lies inside the XY footprint of at least one triangle.
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

    // Find 3D Finish 8 (id 17).
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
    println!(
        "3D Finish: {} moves, cut={:.1}mm, rapid={:.1}mm",
        result.stats.move_count,
        result.stats.cutting_distance,
        result.stats.rapid_distance,
    );

    // Load the surface mesh (terrain.stl).
    let mesh =
        TriangleMesh::from_stl_scaled(&fixture_path("terrain.stl"), 1.0).expect("stl loads");
    let index = SpatialIndex::build_auto(&mesh);

    let final_tp: &Toolpath = &result.toolpath;

    // Safe Z is 10 per project file. A feed move is "at cut depth" when
    // both endpoints are well below safe_z.
    const SAFE_Z: f64 = 10.0;
    const CUT_Z_THRESHOLD: f64 = 9.5;

    let mut off_mesh_cuts: Vec<(P2, f64)> = Vec::new();
    let mut prev: Option<P3> = None;
    for mv in &final_tp.moves {
        let target = mv.target;
        if matches!(mv.move_type, MoveType::Linear { .. }) {
            if let Some(p) = prev
                && p.z < CUT_Z_THRESHOLD
                && target.z < CUT_Z_THRESHOLD
            {
                // Sample a handful of points along the move: if any
                // lies outside the mesh XY footprint, that move is
                // cutting air (or carving rim material) — a phantom
                // at-depth cut.
                let dxy = ((target.x - p.x).powi(2) + (target.y - p.y).powi(2)).sqrt();
                let samples = ((dxy / 0.5).ceil() as usize).max(1);
                for i in 0..=samples {
                    let t = i as f64 / samples as f64;
                    let x = p.x + t * (target.x - p.x);
                    let y = p.y + t * (target.y - p.y);
                    if !point_over_mesh(x, y, &mesh, &index) {
                        off_mesh_cuts.push((P2::new(x, y), t.mul_add(target.z - p.z, p.z)));
                        break;
                    }
                }
            }
        }
        prev = Some(target);
    }

    let _ = SAFE_Z;
    println!(
        "3D Finish off-mesh feed-move endpoints at cut depth: {}",
        off_mesh_cuts.len()
    );
    for (pt, z) in off_mesh_cuts.iter().take(20) {
        println!("  ({:>7.2},{:>7.2}) z={:.2}", pt.x, pt.y, z);
    }

    // Compute a useful summary: the *fraction* of feed-move endpoints
    // that sit off the mesh. A clean 3D finish should stay entirely
    // inside the mesh silhouette; anything > ~1% is the rim-contact
    // phantom bug.
    let total_linear = final_tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .count();
    let pct = (off_mesh_cuts.len() as f64 / total_linear.max(1) as f64) * 100.0;
    println!(
        "{} of {} Linear moves have an off-mesh endpoint ({:.2}%)",
        off_mesh_cuts.len(),
        total_linear,
        pct,
    );

    // Fail the test when the symptom is present. Use a loose threshold
    // for now — a small amount of drift near the rim is expected from
    // spatial indexing.
    assert!(
        pct < 1.0,
        "3D finish drop-cutter is cutting outside the mesh footprint: \
         {}/{total_linear} Linear moves ({pct:.2}%) have at least one \
         sample point outside the mesh silhouette.",
        off_mesh_cuts.len()
    );
}
