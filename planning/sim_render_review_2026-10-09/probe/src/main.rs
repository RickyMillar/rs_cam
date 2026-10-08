use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::stock::dexel::{ray_subtract_above};
use rs_cam_core::stock::dexel_mesh::{dexel_stock_to_entry_surface_mesh, dexel_stock_to_mesh};
use rs_cam_core::stock::dexel_mesh_mc::z_grid_marching_cubes_strided;
use rs_cam_core::stock::stock_mesh::StockMesh;
use std::time::Instant;

fn normals(m: &StockMesh) -> Vec<[f32; 3]> {
    // Copy of sim_render::build_vertex_data normal pass (area-weighted, shared vertices).
    let nv = m.vertices.len() / 3;
    let mut n = vec![[0f32; 3]; nv];
    for t in m.indices.chunks_exact(3) {
        let p = |i: u32| { let i = i as usize * 3; [m.vertices[i], m.vertices[i+1], m.vertices[i+2]] };
        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
        let e1 = [b[0]-a[0], b[1]-a[1], b[2]-a[2]];
        let e2 = [c[0]-a[0], c[1]-a[1], c[2]-a[2]];
        let f = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
        for &i in t { for k in 0..3 { n[i as usize][k] += f[k]; } }
    }
    for v in n.iter_mut() { let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt(); if l > 1e-8 { for k in 0..3 { v[k] /= l; } } }
    n
}

fn chunk_remap(m: &StockMesh, max_buffer: usize) -> (usize, f64) {
    // Copy of sim_render::upload_chunked CPU work (HashMap remap), no GPU.
    let t0 = Instant::now();
    let vert_size = 36usize; let usable = (max_buffer as f64 * 0.95) as usize;
    let max_tris = ((usable / vert_size) / 3).min(((usable / 4) / 3 * 3) / 3);
    let ntri = m.indices.len() / 3; let mut off = 0; let mut chunks = 0;
    while off < ntri {
        let ct = max_tris.min(ntri - off);
        let idx = &m.indices[off*3..(off+ct)*3];
        let mut seen = std::collections::HashMap::new(); let mut lv: Vec<u32> = Vec::new(); let mut li = Vec::with_capacity(idx.len());
        for &i in idx { let l = *seen.entry(i).or_insert_with(|| { lv.push(i); (lv.len()-1) as u32 }); li.push(l); }
        std::hint::black_box((&lv, &li)); chunks += 1; off += ct;
    }
    (chunks, t0.elapsed().as_secs_f64()*1000.0)
}

fn main() {
    let cell = 0.2; let (w, d, h) = (350.0, 500.0, 40.0);
    let mut s = TriDexelStock::from_stock(0.0, 0.0, w, d, 0.0, h, cell);
    let (rows, cols) = (s.z_grid.rows, s.z_grid.cols);
    println!("grid {rows} x {cols} = {} cells", rows*cols);
    for r in 0..rows { for c in 0..cols {
        let (x, y) = s.z_grid.cell_to_world(r, c);
        let mut z = 20.0 + 8.0 * (x / 15.0).sin() * (y / 20.0).cos();
        if (100.0..150.0).contains(&x) && (100.0..200.0).contains(&y) { z = 5.0; } // vertical-wall pocket
        if x + y < 150.0 { z = z.min(10.0); }                                        // 45 deg wall
        let ray = s.z_grid.ray_mut(r, c);
        if ((x-250.0).powi(2) + (y-300.0).powi(2)).sqrt() < 10.0 { ray.clear(); continue; } // through hole
        ray_subtract_above(ray, z as f32);
    }}
    let t = Instant::now(); let full = dexel_stock_to_mesh(&s); let t_full = t.elapsed().as_secs_f64()*1000.0;
    let t = Instant::now(); let s2 = z_grid_marching_cubes_strided(&s.z_grid, h, 0.0, 2); let t_s2 = t.elapsed().as_secs_f64()*1000.0;
    let t = Instant::now(); let prev = dexel_stock_to_entry_surface_mesh(&s, StockCutDirection::FromTop); let t_prev = t.elapsed().as_secs_f64()*1000.0;
    for (name, m, ms) in [("full MC", &full, t_full), ("stride 2", &s2, t_s2), ("preview", &prev, t_prev)] {
        let nv = m.vertices.len()/3; let ni = m.indices.len();
        let cpu = (m.vertices.len()+m.colors.len())*4 + ni*4; let gpu_v = nv*36; let gpu_i = ni*4;
        println!("{name}: build {ms:.0} ms, verts {nv}, tris {}, CPU {:.0} MB, GPU vertex {:.0} MB + index {:.0} MB", ni/3, cpu as f64/1e6, gpu_v as f64/1e6, gpu_i as f64/1e6);
    }
    let t = Instant::now(); let n_full = normals(&full); println!("normals full: {:.0} ms", t.elapsed().as_secs_f64()*1000.0);
    let t = Instant::now(); let _n_prev = normals(&prev); println!("normals preview: {:.0} ms", t.elapsed().as_secs_f64()*1000.0);
    let t = Instant::now(); let mut clone = full.clone(); std::hint::black_box(&mut clone); println!("mesh clone: {:.0} ms", t.elapsed().as_secs_f64()*1000.0);
    let (nc, ms) = chunk_remap(&full, 256 << 20); println!("chunk remap at 256 MiB: {nc} chunks, {ms:.0} ms");
    let t = Instant::now(); let sc = s.clone(); std::hint::black_box(&sc); println!("stock clone: {:.0} ms", t.elapsed().as_secs_f64()*1000.0);

    // Through-hole in the playback preview: the vertex at the hole centre.
    let (hr, hc) = ((300.0/cell) as usize, (250.0/cell) as usize);
    let pi = hr*cols + hc; println!("preview z at through-hole centre: {} (stock top {h}, bottom 0)", prev.vertices[pi*3+2]);
    // Full mesh: any vertex inside the hole radius - 1 mm?
    let inside = (0..full.vertices.len()/3).filter(|&i| { let (x,y)=(full.vertices[i*3],full.vertices[i*3+1]); ((x-250.0).powi(2)+(y-300.0).powi(2)).sqrt() < 9.0 }).count();
    println!("full mesh vertices inside hole r<9: {inside}");
    // Rim normal: top vertex at the pocket rim x=100, y=150, near z=top surface.
    let mut best: Option<(usize, f32)> = None;
    for i in 0..full.vertices.len()/3 { let (x,y,z)=(full.vertices[i*3],full.vertices[i*3+1],full.vertices[i*3+2]);
        if (x-99.9).abs()<0.15 && (y-150.0).abs()<0.15 && z > 10.0 { best = Some((i, z)); } }
    if let Some((i,z)) = best { println!("rim corner vertex z {z:.2}: smoothed normal {:?}", n_full[i]); }
    for dx in [-0.6f32, -0.4, -0.2, 0.0, 0.2, 0.4] {
        for i in 0..full.vertices.len()/3 { let (x,y,z)=(full.vertices[i*3],full.vertices[i*3+1],full.vertices[i*3+2]);
            if (x-(99.9+dx)).abs()<0.05 && (y-150.1).abs()<0.06 && z > 1.0 && z < 39.0 { println!("  pocket-edge x {x:.1} z {z:.2} normal {:?}", n_full[i]); } }
    }
    // Perimeter top edge vertex normal (stock edge x=0, y=250).
    for i in 0..full.vertices.len()/3 { let (x,y,z)=(full.vertices[i*3],full.vertices[i*3+1],full.vertices[i*3+2]);
        if x < 0.0 && (y-250.0).abs()<0.11 && z > 10.0 { println!("perimeter top vertex x {x:.2} z {z:.2} normal {:?}", n_full[i]); } }
}
