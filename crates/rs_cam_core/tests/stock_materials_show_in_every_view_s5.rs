//! S5 (`planning/stock_additions_2026-10-09/PLAN.md`): the stock views show
//! the material of each dexel segment.
//!
//! - A two-material stock gives two colours at the right cells: in the mesh
//!   slots, in the material colours, and in the pixels of the software
//!   renderer (`screenshot_simulation`).
//! - A one-material stock records no slot, and its colours do not change.
//! - An added material above the stock top (an overfill) and below the
//!   stock bottom is in the mesh: the extraction does not clip it to the
//!   stock box.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::export::fingerprint::{
    composite_panel_layout, render_stock_composite_in_frame,
    render_stock_composite_in_frame_with_palette,
};
use rs_cam_core::export::material_colour::{MaterialPalette, material_colors};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::material::Material;
use rs_cam_core::stock::dexel_mesh::{
    dexel_stock_to_entry_surface_mesh, dexel_stock_to_mesh, dexel_stock_to_mesh_strided,
};
use rs_cam_core::stock::material_slot::MaterialSlot;
use rs_cam_core::stock::stock_mesh::StockMesh;

/// The stock box: 40 x 40 x 10 mm, 1 mm cells.
fn plain_stock() -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(40.0, 40.0, 10.0),
    };
    TriDexelStock::from_bounds(&bbox, 1.0)
}

/// Add `[a, b]` of a second material on every column whose centre `x`
/// passes `keep`. Returns the slot.
fn add_material(
    stock: &mut TriDexelStock,
    a: f32,
    b: f32,
    keep: impl Fn(f64) -> bool,
) -> MaterialSlot {
    let slot = stock.materials.slot_for(&Material::default()).unwrap();
    let grid = &mut stock.z_grid;
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let (u, _) = grid.cell_to_world(row, col);
            if keep(u) {
                grid.union_interval_at(row * grid.cols + col, a, b, slot);
            }
        }
    }
    slot
}

/// The left half (x < 20) carries a 1 mm overfill of slot 1 on the stock
/// top: material from z = 10 to z = 11.
fn two_material_stock() -> (TriDexelStock, MaterialSlot) {
    let mut stock = plain_stock();
    let slot = add_material(&mut stock, 10.0, 11.0, |x| x < 20.0);
    (stock, slot)
}

fn z_range(mesh: &StockMesh) -> (f32, f32) {
    let zs = mesh.vertices.iter().skip(2).step_by(3);
    let lo = zs.clone().fold(f32::INFINITY, |m, &z| m.min(z));
    let hi = zs.fold(f32::NEG_INFINITY, |m, &z| m.max(z));
    (lo, hi)
}

#[test]
fn a_two_material_stock_gives_two_slots_and_two_colours_at_the_right_cells() {
    let (stock, slot) = two_material_stock();
    assert_eq!(slot, MaterialSlot(1));
    let mesh = dexel_stock_to_mesh(&stock);
    assert_eq!(mesh.material_slots.len(), mesh.vertex_count());

    let palette = MaterialPalette::default();
    let colours = material_colors(&mesh, &palette);
    let wood_top = palette.colour(MaterialSlot::STOCK);
    let added_top = palette.colour(slot);

    let (mut left_top, mut right_top) = (0, 0);
    for (i, colour) in colours.iter().enumerate() {
        let (x, z) = (mesh.vertices[i * 3], mesh.vertices[i * 3 + 2]);
        let s = mesh.slot_at(i);
        // A vertex at the top of the overfill, well inside the left half.
        if z > 10.9 && x < 18.0 {
            assert_eq!(s, slot, "vertex {i} at x {x}, z {z}");
            assert_eq!(*colour, added_top, "vertex {i}");
            left_top += 1;
        }
        // The top of the right half: the stock material.
        if (z - 10.0).abs() < 1e-4 && x > 22.0 {
            assert_eq!(s, MaterialSlot::STOCK, "vertex {i} at x {x}, z {z}");
            assert_eq!(*colour, wood_top, "vertex {i}");
            right_top += 1;
        }
        // No vertex of the added material lies right of the boundary cell.
        if s == slot {
            assert!(x < 21.0, "slot {s:?} vertex at x {x}");
        }
    }
    assert!(left_top > 100, "{left_top} overfill top vertices");
    assert!(right_top > 100, "{right_top} stock top vertices");
    assert_ne!(added_top, wood_top);

    // The playback preview and the strided mesh carry the same slots.
    let preview = dexel_stock_to_entry_surface_mesh(&stock, StockCutDirection::FromTop);
    assert_eq!(preview.material_slots.len(), preview.vertex_count());
    for i in 0..preview.vertex_count() {
        let x = preview.vertices[i * 3];
        let want = if x < 20.0 { slot } else { MaterialSlot::STOCK };
        assert_eq!(preview.slot_at(i), want, "preview vertex {i} at x {x}");
    }
    let strided = dexel_stock_to_mesh_strided(&stock, 2);
    assert_eq!(strided.material_slots.len(), strided.vertex_count());
    assert!(strided.material_slots.contains(&slot));
    assert!(strided.material_slots.contains(&MaterialSlot::STOCK));
}

#[test]
fn a_one_material_stock_records_no_slot_and_keeps_its_colours() {
    let mut stock = plain_stock();
    // A cut, so the wood ramp has more than one shade.
    for row in 10..20 {
        for col in 10..20 {
            rs_cam_core::stock::dexel::ray_subtract_above(stock.z_grid.ray_mut(row, col), 6.0);
        }
    }
    for mesh in [
        dexel_stock_to_mesh(&stock),
        dexel_stock_to_mesh_strided(&stock, 3),
        dexel_stock_to_entry_surface_mesh(&stock, StockCutDirection::FromTop),
    ] {
        assert!(mesh.material_slots.is_empty());
        let colours: Vec<f32> = material_colors(&mesh, &MaterialPalette::default())
            .into_iter()
            .flatten()
            .collect();
        assert_eq!(
            colours, mesh.colors,
            "the palette changes a one-material mesh"
        );
    }
    // The software renderer: the palette door renders the same bytes.
    let frame = stock.stock_bbox;
    assert_eq!(
        render_stock_composite_in_frame(&stock, &frame, 300, 200),
        render_stock_composite_in_frame_with_palette(
            &stock,
            &frame,
            &MaterialPalette::default(),
            300,
            200
        )
    );
}

#[test]
fn an_overfill_above_the_top_and_a_fill_below_the_bottom_are_drawn() {
    let mut stock = plain_stock();
    let slot = add_material(&mut stock, 10.0, 12.0, |x| x < 20.0);
    add_material(&mut stock, -3.0, 0.0, |x| x >= 20.0);
    // The stock box does not grow: the extraction must not read it as a clip.
    assert!((stock.stock_bbox.max.z - 10.0).abs() < 1e-9);
    assert!(stock.stock_bbox.min.z.abs() < 1e-9);

    let mesh = dexel_stock_to_mesh(&stock);
    let (lo, hi) = z_range(&mesh);
    assert!(
        (hi - 12.0).abs() < 1e-4,
        "the mesh top is {hi}, the overfill top is 12"
    );
    assert!(
        (lo + 3.0).abs() < 1e-4,
        "the mesh bottom is {lo}, the fill bottom is -3"
    );
    // The underside of the right half is the added material.
    let under: Vec<MaterialSlot> = (0..mesh.vertex_count())
        .filter(|&i| mesh.vertices[i * 3 + 2] < -2.9 && mesh.vertices[i * 3] > 22.0)
        .map(|i| mesh.slot_at(i))
        .collect();
    assert!(!under.is_empty());
    assert!(under.iter().all(|&s| s == slot), "{under:?}");

    let strided = dexel_stock_to_mesh_strided(&stock, 2);
    let (lo, hi) = z_range(&strided);
    assert!(
        (hi - 12.0).abs() < 1e-4 && (lo + 3.0).abs() < 1e-4,
        "strided {lo}..{hi}"
    );

    let top = dexel_stock_to_entry_surface_mesh(&stock, StockCutDirection::FromTop);
    assert!((z_range(&top).1 - 12.0).abs() < 1e-4);
    let bottom = dexel_stock_to_entry_surface_mesh(&stock, StockCutDirection::FromBottom);
    assert!((z_range(&bottom).0 + 3.0).abs() < 1e-4);
}

/// The software renderer (`screenshot_simulation`): the Top panel shows the
/// added material colour over the left half and the stock height gradient
/// over the right half.
#[test]
fn the_software_renderer_draws_the_added_material_at_its_cells() {
    const W: u32 = 900;
    const H: u32 = 600;
    let (stock, slot) = two_material_stock();
    let palette = MaterialPalette::default();
    let frame = stock.stock_bbox;
    let pixels = render_stock_composite_in_frame_with_palette(&stock, &frame, &palette, W, H);
    let top = composite_panel_layout(W, H)
        .into_iter()
        .find(|p| p.label == "TOP")
        .unwrap();

    // The chromatic bounding box of the Top panel (the overlay is grey).
    let at = |px: usize, py: usize| -> [i32; 3] {
        let i = ((top.y + py) * W as usize + top.x + px) * 4;
        [
            i32::from(pixels[i]),
            i32::from(pixels[i + 1]),
            i32::from(pixels[i + 2]),
        ]
    };
    let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
    for py in 0..top.height {
        for px in 0..top.width {
            let [r, g, b] = at(px, py);
            if r.max(g).max(b) - r.min(g).min(b) >= 25 {
                x0 = x0.min(px);
                x1 = x1.max(px);
                y0 = y0.min(py);
                y1 = y1.max(py);
            }
        }
    }
    assert!(x1 > x0 && y1 > y0, "the Top panel drew no material");
    let mid_y = (y0 + y1) / 2;
    let quarter = |f: f64| x0 + ((x1 - x0) as f64 * f) as usize;

    // The added material colour is a blue-green: green and blue over red.
    let added = palette.colour(slot);
    assert!(added[1] > added[0] && added[2] > added[0], "{added:?}");
    let [r, g, b] = at(quarter(0.25), mid_y);
    assert!(
        g > r + 30 && b > r + 30,
        "left half (x < 20) is {r},{g},{b}"
    );
    // The right half keeps the stock height gradient: the top of the
    // range is red.
    let [r, g, b] = at(quarter(0.75), mid_y);
    assert!(
        r > g + 30 && r > b + 30,
        "right half (x > 20) is {r},{g},{b}"
    );
}
