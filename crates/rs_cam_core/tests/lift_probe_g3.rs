//! G3 probe — is the sporadic 0.49 mm burial on the rim chamfer a
//! `point_drop_cutter` defect or an emission-layer one? Prints the exact
//! query z at the five worst gouge XYs from the v4 G-code analysis vs the
//! ball-standoff z the 47.44° chamfer plane demands. Evidence run.

#![allow(clippy::print_stderr, clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use rs_cam_core::dropcutter::point_drop_cutter;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::TaperedBallEndmill;

const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_lift_probe_g3() {
    if !Path::new(WANAKA_MESH).exists() {
        eprintln!("SKIP: mesh not present");
        return;
    }
    let mesh = TriangleMesh::from_stl_scaled(Path::new(WANAKA_MESH), 1.0).expect("mesh");
    let index = SpatialIndex::build_auto(&mesh);
    let r15 = TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5);
    // Model-frame XYs of the worst buried points (v4 analysis), plus two
    // healthy mid-chamfer references.
    let probes = [
        (4.8, 196.5, "gouge 0.488"),
        (131.3, 197.4, "gouge 0.477"),
        (142.2, 196.6, "gouge 0.475"),
        (137.3, 199.2, "gouge 0.450"),
        (100.0, 197.5, "reference mid-chamfer"),
        (60.0, 197.5, "reference mid-chamfer"),
    ];
    for (x, y, label) in probes {
        let cl = point_drop_cutter(x, y, &mesh, &index, &r15);
        let z_surf = 7.0 - (200.0 - y) * 47.44_f64.to_radians().tan();
        eprintln!(
            "({x:6.1},{y:6.1}) {label:>24}: query z {:+.3}, tip-above-line {:+.3}",
            cl.z,
            cl.z - z_surf
        );
    }

    // Failure-shape map: fine grid around the two bad spots. Prints a
    // character map of tip-above-line so an index-aligned stripe is
    // visible at a glance.
    for (cx, cy) in [(4.8_f64, 196.5_f64), (131.3, 197.4)] {
        eprintln!("map around ({cx},{cy}) — 0.1 mm grid, '#'=deficit>0.3, 'o'=0.1..0.3, '.'=normal:");
        for row in 0..17 {
            let y = cy + 0.8 - row as f64 * 0.1;
            let mut line = String::new();
            for col in 0..33 {
                let x = cx - 1.6 + col as f64 * 0.1;
                let cl = point_drop_cutter(x, y, &mesh, &index, &r15);
                let z_line = 7.0 - (200.0 - y) * 47.44_f64.to_radians().tan();
                let deficit = 0.65 - (cl.z - z_line);
                line.push(if deficit > 0.3 {
                    '#'
                } else if deficit > 0.1 {
                    'o'
                } else {
                    '.'
                });
            }
            eprintln!("  y={y:7.2} {line}");
        }
    }
}
