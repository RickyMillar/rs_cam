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

/// Build a pyramid mesh with peak at (cx, cy, peak_z), base radius r at z=0.
/// Triangulates as a fan from the peak to the base.
fn pyramid_mesh(cx: f64, cy: f64, r: f64, peak_z: f64, segments: usize) -> TriangleMesh {
    let mut verts = vec![P3::new(cx, cy, peak_z)]; // 0 = peak
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

#[test]
fn drop_cutter_reports_cl_deviation_vs_mesh() {
    // Scan every accepted grid point from 3D Finish 8 and compare its
    // CL z against the vertical-ray mesh height at the same (x, y).
    // For a faithful 3D finish the CL tip z should be AT OR ABOVE the
    // mesh ray z (ball tip touches the peak, or ball-side contact on
    // slopes pushes tip slightly below the ray z by at most
    // ball_radius * (1 - cos(slope)) ≈ 0.5 mm for a 1 mm ball at 60°).
    // Anything significantly below the ray z is the tool digging into
    // the mesh (the "flattening" symptom).
    use rs_cam_core::tool::TaperedBallEndmill;

    let mesh = TriangleMesh::from_stl_scaled(&fixture_path("terrain.stl"), 1.0)
        .expect("stl loads");
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = TaperedBallEndmill::new(1.0, 15.0, 6.35, 25.0);

    // Sample the full grid at 2 mm resolution (fast).
    let step = 2.0;
    let mut below_count = 0usize;
    let mut worst = (0.0f64, 0.0, 0.0, 0.0, 0.0);
    let nx = ((mesh.bbox.max.x - mesh.bbox.min.x) / step) as usize;
    let ny = ((mesh.bbox.max.y - mesh.bbox.min.y) / step) as usize;
    for iy in 0..=ny {
        let y = mesh.bbox.min.y + iy as f64 * step;
        for ix in 0..=nx {
            let x = mesh.bbox.min.x + ix as f64 * step;
            // Skip if not over mesh
            let mut ray_z: Option<f64> = None;
            for &tri_idx in &index.query(x, y, 0.0) {
                let tri = &mesh.faces[tri_idx];
                if tri.contains_point_xy(x, y)
                    && let Some(z) = tri.z_at_xy(x, y)
                {
                    ray_z = Some(ray_z.map_or(z, |prev| prev.max(z)));
                }
            }
            let Some(ray_z) = ray_z else { continue };
            let cl = rs_cam_core::dropcutter::point_drop_cutter(x, y, &mesh, &index, &cutter);
            if !cl.contacted {
                continue;
            }
            let dive = ray_z - cl.z;
            if dive > 1.0 {
                below_count += 1;
                if dive > worst.0 {
                    worst = (dive, x, y, cl.z, ray_z);
                }
            }
        }
    }
    println!(
        "{} grid points have CL z > 1 mm below vertical-ray mesh surface (flattening indicator)",
        below_count
    );
    if below_count > 0 {
        println!(
            "  worst: dive={:.2}mm at ({:.2},{:.2})  cl.z={:.2}  ray_z={:.2}",
            worst.0, worst.1, worst.2, worst.3, worst.4
        );
    }
    // Not asserting yet — this is diagnostic data to see whether the
    // "flattening" report is visible in the drop cutter itself.
}

#[test]
fn drop_cutter_over_peak_stays_at_peak_height() {
    // Synthetic pyramid with a peak at (0, 0, 5) and a 10mm base radius.
    // A 1mm ball + 6.35mm shaft tapered-ball tool can reach the peak (the
    // pyramid is gentle: tan(slope) = 5 / 10 = 0.5, ~27° — well within
    // the tool's taper half-angle). The tool CL at (0, 0) must be at z=5.
    use rs_cam_core::tool::TaperedBallEndmill;
    let mesh = pyramid_mesh(0.0, 0.0, 10.0, 5.0, 32);
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = TaperedBallEndmill::new(1.0, 15.0, 6.35, 25.0);
    let cl = rs_cam_core::dropcutter::point_drop_cutter(0.0, 0.0, &mesh, &index, &cutter);
    println!(
        "Peak CL: contacted={}, z={:.3} (expected 5.0)",
        cl.contacted, cl.z
    );
    assert!(cl.contacted, "tool should contact at peak");
    assert!(
        (cl.z - 5.0).abs() < 0.05,
        "tool tip should reach peak at z=5.0 (got {:.3})",
        cl.z
    );
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
