//! Pick-latency probe (research only, 2026-10-28). Times one CPU ray pick
//! on a mesh: the product's brute-force `ray_pick_triangle` against a
//! parry3d `TriMesh` (BVH) ray cast. Usage: pick_probe <stl> [rays]
use parry3d_f64::query::{Ray, RayCast};
use parry3d_f64::shape::TriMesh;
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::mesh::{TriangleMesh, ray_pick_triangle};
use std::path::Path;
use std::time::Instant;

fn rays(mesh: &TriangleMesh, n: usize) -> Vec<(P3, V3)> {
    // Rays as a viewport sees them: the eye above and to one side of the
    // part, a grid of directions across the bbox (like hover samples).
    let b = &mesh.bbox;
    let c = P3::new((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0, (b.min.z + b.max.z) / 2.0);
    let ext = (b.max.x - b.min.x).max(b.max.y - b.min.y).max(b.max.z - b.min.z);
    let eye = c + V3::new(ext * 1.2, -ext * 1.2, ext * 1.5);
    let side = (n as f64).sqrt().ceil() as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..side {
        for j in 0..side {
            if out.len() == n { break; }
            let fx = (i as f64 + 0.5) / side as f64;
            let fy = (j as f64 + 0.5) / side as f64;
            let target = P3::new(b.min.x + fx * (b.max.x - b.min.x), b.min.y + fy * (b.max.y - b.min.y), c.z);
            out.push((eye, (target - eye).normalize()));
        }
    }
    out
}

fn stats(mut t: Vec<f64>) -> (f64, f64, f64) {
    t.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = t.iter().sum::<f64>() / t.len() as f64;
    (mean, t[t.len() / 2], t[t.len() - 1])
}

fn run(name: &str, mesh: &TriangleMesh, n: usize) {
    let rs = rays(mesh, n);
    // Brute force (the product path).
    let mut bf = Vec::new();
    let mut bf_hits = Vec::new();
    for (o, d) in &rs {
        let t0 = Instant::now();
        let h = ray_pick_triangle(mesh, o, d);
        bf.push(t0.elapsed().as_secs_f64() * 1e3);
        bf_hits.push(h);
    }
    // parry3d TriMesh with its BVH.
    let t0 = Instant::now();
    let verts: Vec<parry3d_f64::math::Point<f64>> = mesh.vertices.iter().map(|v| parry3d_f64::math::Point::new(v.x, v.y, v.z)).collect();
    let tri = TriMesh::new(verts, mesh.triangles.clone()).expect("trimesh");
    let build_ms = t0.elapsed().as_secs_f64() * 1e3;
    let mut pb = Vec::new();
    let mut agree = 0usize;
    for (k, (o, d)) in rs.iter().enumerate() {
        let ray = Ray::new(parry3d_f64::math::Point::new(o.x, o.y, o.z), parry3d_f64::math::Vector::new(d.x, d.y, d.z));
        let t0 = Instant::now();
        let h = tri.cast_local_ray_and_get_normal(&ray, f64::MAX, true);
        pb.push(t0.elapsed().as_secs_f64() * 1e3);
        let bh = bf_hits[k].map(|(_, t)| t);
        let ph = h.map(|r| r.time_of_impact);
        match (bh, ph) {
            (None, None) => agree += 1,
            (Some(a), Some(b)) if (a - b).abs() < 1e-6 * (1.0 + a.abs()) => agree += 1,
            _ => {
                let pf = h.map(|r| format!("{:?}", r.feature));
                println!("  DISAGREE ray {k}: brute {:?} (tri, t); parry t {:?} feature {:?}", bf_hits[k], ph, pf);
                if let Some((ti, t)) = bf_hits[k] {
                    // Every brute-force triangle within 1e-9 of the best t: a tie?
                    let ties: Vec<usize> = mesh.faces.iter().enumerate().filter_map(|(i, f)| f.ray_intersect(o, d).filter(|tt| (tt - t).abs() < 1e-9).map(|_| i)).collect();
                    println!("  DISAGREE brute ties at t: {ties:?} (best {ti})");
                }
            }
        }
    }
    let hits = bf_hits.iter().filter(|h| h.is_some()).count();
    let (m1, md1, mx1) = stats(bf);
    let (m2, md2, mx2) = stats(pb);
    println!("{name}: {} triangles, {} rays ({hits} hit)", mesh.triangles.len(), rs.len());
    println!("  brute force ray_pick_triangle: mean {m1:.3} ms, median {md1:.3} ms, max {mx1:.3} ms");
    println!("  parry3d TriMesh BVH build: {build_ms:.1} ms; cast: mean {m2:.4} ms, median {md2:.4} ms, max {mx2:.4} ms");
    println!("  same nearest hit (t within 1e-6 rel): {agree}/{}", rs.len());
}

/// A synthetic hemisphere of `div` x `div` quads, two triangles each.
fn hemisphere(radius: f64, div: usize) -> TriangleMesh {
    let mut v = Vec::new();
    for i in 0..=div {
        for j in 0..=div {
            let x = (i as f64 / div as f64) * 2.0 - 1.0;
            let y = (j as f64 / div as f64) * 2.0 - 1.0;
            let z = (1.0 - (x * x + y * y)).max(0.0).sqrt();
            v.push(P3::new(x * radius, y * radius, z * radius));
        }
    }
    let mut t = Vec::new();
    let w = (div + 1) as u32;
    for i in 0..div as u32 {
        for j in 0..div as u32 {
            let a = i * w + j;
            t.push([a, a + 1, a + w]);
            t.push([a + 1, a + w + 1, a + w]);
        }
    }
    TriangleMesh::from_raw(v, t)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(400);
    if let Some(p) = args.get(1) {
        let mesh = TriangleMesh::from_stl(Path::new(p)).expect("stl");
        run(p, &mesh, n);
    }
    for div in [70usize, 200, 500, 800] {
        let mesh = hemisphere(100.0, div);
        run(&format!("hemisphere div={div}"), &mesh, n);
    }
}
