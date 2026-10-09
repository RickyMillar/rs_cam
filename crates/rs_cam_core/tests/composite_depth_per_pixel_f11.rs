//! The 6-view composite depth-tests each pixel at its own depth, not at the
//! depth of the triangle centroid (render review 2026-10-09, F11).
//!
//! Pre-registration: `planning/sim_render_review_2026-10-09/REVIEW.md` F11
//! and package P1.
//!
//! The scene: a flat red square at z = 10, and a blue triangle that slopes
//! from z = 0 (one edge) to z = 27 (the apex), over the same footprint. The
//! centroid of the blue triangle is at z = 9, below the red square. Seen
//! from the TOP panel:
//!
//! - with a per-pixel depth, the blue triangle shows where it is above
//!   z = 10: the band from y = 37 to the apex, 40 % of its area
//!   (`(1 - 10 / 27)^2 = 0.395`);
//! - with the centroid depth, the red square covers all of it, and no blue
//!   pixel shows.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::export::fingerprint::{composite_panel_layout, render_mesh_composite};
use rs_cam_core::stock::stock_mesh::StockMesh;

const W: u32 = 900;
const H: u32 = 600;

fn scene() -> StockMesh {
    let red = [1.0_f32, 0.0, 0.0];
    let blue = [0.0_f32, 0.0, 1.0];
    let vertices = vec![
        // The red square at z = 10, CCW from above.
        -10.0, -10.0, 10.0, //
        110.0, -10.0, 10.0, //
        110.0, 110.0, 10.0, //
        -10.0, 110.0, 10.0, //
        // The blue sloped triangle, CCW from above.
        0.0, 0.0, 0.0, //
        100.0, 0.0, 0.0, //
        50.0, 100.0, 27.0,
    ];
    let mut colors = Vec::new();
    for _ in 0..4 {
        colors.extend_from_slice(&red);
    }
    for _ in 0..3 {
        colors.extend_from_slice(&blue);
    }
    StockMesh {
        vertices,
        indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6],
        colors,
        material_slots: Vec::new(),
    }
}

#[test]
fn a_sloped_face_shows_where_it_is_nearer_than_a_flat_face() {
    let px = render_mesh_composite(&scene(), W, H);
    let panels = composite_panel_layout(W, H);
    let p = panels.iter().find(|p| p.label == "TOP").unwrap();
    let (mut red, mut blue) = (0usize, 0usize);
    for y in p.y..p.y + p.height {
        for x in p.x..p.x + p.width {
            let i = (y * W as usize + x) * 4;
            let (r, b) = (i32::from(px[i]), i32::from(px[i + 2]));
            if r > b + 50 {
                red += 1;
            } else if b > r + 50 {
                blue += 1;
            }
        }
    }
    assert!(red > 0, "the red square does not show");
    // The blue band is 40 % of the triangle, and the triangle is about 35 %
    // of the red square's footprint (5000 of 14400 mm²): about 14 % of the
    // coloured pixels. The gate at 7 % allows for the panel edge.
    let share = blue as f64 / (red + blue) as f64;
    assert!(
        share > 0.07,
        "blue share {share:.3} ({blue} of {}); the per-pixel depth shows about 0.14, \
         the centroid depth shows 0",
        red + blue
    );
}
