// Does the 6-view composite shade the MC mesh's top face as lit or as unlit?
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::export::fingerprint::render_mesh_composite;
use rs_cam_core::stock::dexel_mesh::dexel_stock_to_mesh;

fn mean_panel(px: &[u8], w: usize, x0: usize, y0: usize, pw: usize, ph: usize) -> f64 {
    let mut s = 0.0; let mut n = 0.0;
    for y in y0 + ph / 3..y0 + 2 * ph / 3 { for x in x0 + pw / 3..x0 + 2 * pw / 3 {
        let i = (y * w + x) * 4; s += f64::from(px[i]) + f64::from(px[i + 1]) + f64::from(px[i + 2]); n += 3.0; } }
    s / n
}

fn main() {
    let s = TriDexelStock::from_stock(0.0, 0.0, 100.0, 100.0, 0.0, 20.0, 1.0);
    let mesh = dexel_stock_to_mesh(&s);
    let mut flipped = mesh.clone();
    for t in flipped.indices.chunks_exact_mut(3) { t.swap(1, 2); }
    let (w, h) = (1200usize, 800usize);
    let a = render_mesh_composite(&mesh, w as u32, h as u32);
    let b = render_mesh_composite(&flipped, w as u32, h as u32);
    // TOP panel: column 1, row 0. The panels are about w/3 wide and under h/2 high.
    let (pw, ph) = (w / 3, h * 4 / 10);
    println!("TOP panel centre mean, shipped winding: {:.1}", mean_panel(&a, w, pw, 0, pw, ph));
    println!("TOP panel centre mean, flipped winding: {:.1}", mean_panel(&b, w, pw, 0, pw, ph));
    let c = mesh.colors[0..3].to_vec();
    println!("vertex colour of a top vertex x255: {:.1}", (c[0]+c[1]+c[2]) / 3.0 * 255.0);
}
