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

    // Sub-test bisection at the worst failing XY: run facet / vertex /
    // edge drops separately over every candidate triangle and report which
    // stage supplies the best z. Pins WHICH contact test loses the seam.
    {
        use rs_cam_core::tool::{CLPoint, MillingCutter};
        for (x, y) in [(4.8_f64, 196.5_f64), (131.3, 197.4), (100.0, 197.5)] {
            let tris = index.query(x, y, r15.radius());
            let mut facet = CLPoint::new(x, y);
            let mut vertex = CLPoint::new(x, y);
            let mut edge = CLPoint::new(x, y);
            for &ti in &tris {
                let tri = &mesh.faces[ti];
                let mut f = CLPoint::new(x, y);
                if r15.facet_drop(&mut f, tri) && f.z > facet.z {
                    facet.z = f.z;
                }
                for v in &tri.v {
                    let mut vv = CLPoint::new(x, y);
                    r15.vertex_drop(&mut vv, v);
                    if vv.z > vertex.z {
                        vertex.z = vv.z;
                    }
                }
                for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                    let mut e = CLPoint::new(x, y);
                    r15.edge_drop(&mut e, &tri.v[a], &tri.v[b]);
                    if e.z > edge.z {
                        edge.z = e.z;
                    }
                }
            }
            let full = point_drop_cutter(x, y, &mesh, &index, &r15);
            eprintln!(
                "bisect ({x:5.1},{y:6.1}): tris {} | facet-only z {:+.3} | vertex-only {:+.3} \
                 | edge-only {:+.3} | full {:+.3}",
                tris.len(),
                facet.z,
                vertex.z,
                edge.z,
                full.z
            );
        }
    }

    // Index-vs-brute-force: linear-scan ALL mesh triangles within the
    // envelope radius in XY and run the full drop over that set. If brute
    // force lifts higher than the indexed query, the spatial index is
    // returning an incomplete candidate set — proven, not inferred.
    {
        use rs_cam_core::tool::{CLPoint, MillingCutter};
        for (x, y) in [(4.8_f64, 196.5_f64), (131.3, 197.4), (100.0, 197.5)] {
            let r = r15.radius();
            let mut brute = CLPoint::new(x, y);
            let mut brute_count = 0usize;
            for tri in &mesh.faces {
                let (mut minx, mut miny, mut maxx, mut maxy) = (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                );
                for v in &tri.v {
                    minx = minx.min(v.x);
                    miny = miny.min(v.y);
                    maxx = maxx.max(v.x);
                    maxy = maxy.max(v.y);
                }
                if x < minx - r || x > maxx + r || y < miny - r || y > maxy + r {
                    continue;
                }
                brute_count += 1;
                r15.drop_cutter(&mut brute, tri);
            }
            let idx_set = index.query(x, y, r);
            let full = point_drop_cutter(x, y, &mesh, &index, &r15);
            eprintln!(
                "brute ({x:5.1},{y:6.1}): brute z {:+.3} over {brute_count} tris | indexed z \
                 {:+.3} over {} tris | index {}",
                brute.z,
                full.z,
                idx_set.len(),
                if brute.z > full.z + 0.01 {
                    "INCOMPLETE — GUILTY"
                } else {
                    "consistent"
                }
            );
        }
    }

    // Failure-shape map: fine grid around the two bad spots. Prints a
    // character map of tip-above-line so an index-aligned stripe is
    // visible at a glance.
    for (cx, cy) in [(4.8_f64, 196.5_f64), (131.3, 197.4)] {
        eprintln!(
            "map around ({cx},{cy}) — 0.1 mm grid, '#'=deficit>0.3, 'o'=0.1..0.3, '.'=normal:"
        );
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
