//! Every stock mesh builder winds its triangles so that each normal points
//! out of the material (render review 2026-10-09, F1).
//!
//! Pre-registration: `planning/sim_render_review_2026-10-09/REVIEW.md` F1 and
//! package P1. Before the fix, the top face of an uncut block had normals
//! that pointed down, the signed volume of the solid was negative, and the
//! 6-view composite drew the top of an uncut block at the ambient level
//! (52 of 255 against 137 for the flipped mesh, `probe/RESULTS.txt`).
//!
//! The claims:
//!
//! 1. The marching-cubes envelope (full and strided) has top normals with
//!    z > 0, bottom normals with z < 0, outward skirt normals, and a signed
//!    volume equal to the box volume.
//! 2. A cut block (pocket and through-hole) keeps a positive signed volume
//!    that is less than the uncut volume.
//! 3. The cavity pass gives a floor (+Z) at the bottom of a gap and a
//!    ceiling (-Z) at the top of the gap.
//! 4. The playback preview faces the tool: +Z for `FromTop`, -Z for
//!    `FromBottom`.
//! 5. The drill tube, its flat cap and its cone face out of the material.
//! 6. The composite draws the TOP and the BOTTOM panel of an uncut block lit
//!    (the `probe/src/bin/shade.rs` check, as a test).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::export::fingerprint::{composite_panel_layout, render_mesh_composite};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::material::Material;
use rs_cam_core::ops::drill::DrillCycle;
use rs_cam_core::ops::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};
use rs_cam_core::stock::dexel::{ray_subtract_above, ray_subtract_interval};
use rs_cam_core::stock::dexel_mesh::{
    append_drill_cylinders, dexel_stock_to_entry_surface_mesh, dexel_stock_to_mesh,
    dexel_stock_to_mesh_strided,
};
use rs_cam_core::stock::stock_mesh::StockMesh;

const TOP: f64 = 20.0;

fn block(cell: f64) -> TriDexelStock {
    TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(40.0, 30.0, TOP),
        },
        cell,
    )
}

fn vertex(mesh: &StockMesh, i: u32) -> [f64; 3] {
    let i = i as usize * 3;
    [
        f64::from(mesh.vertices[i]),
        f64::from(mesh.vertices[i + 1]),
        f64::from(mesh.vertices[i + 2]),
    ]
}

/// `(centroid, unnormalised normal)` of every triangle.
fn triangles(mesh: &StockMesh) -> Vec<([f64; 3], [f64; 3])> {
    mesh.indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let (a, b, c) = (vertex(mesh, t[0]), vertex(mesh, t[1]), vertex(mesh, t[2]));
            let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let centroid = [
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ];
            (centroid, n)
        })
        .collect()
}

/// The signed volume `sum(v0 . (v1 x v2)) / 6`. Positive when every
/// triangle of a closed mesh faces out.
fn signed_volume(mesh: &StockMesh) -> f64 {
    mesh.indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let (a, b, c) = (vertex(mesh, t[0]), vertex(mesh, t[1]), vertex(mesh, t[2]));
            let bxc = [
                b[1] * c[2] - b[2] * c[1],
                b[2] * c[0] - b[0] * c[2],
                b[0] * c[1] - b[1] * c[0],
            ];
            (a[0] * bxc[0] + a[1] * bxc[1] + a[2] * bxc[2]) / 6.0
        })
        .sum()
}

fn bbox_volume(mesh: &StockMesh) -> f64 {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for v in mesh.vertices.as_chunks::<3>().0 {
        for k in 0..3 {
            lo[k] = lo[k].min(f64::from(v[k]));
            hi[k] = hi[k].max(f64::from(v[k]));
        }
    }
    (hi[0] - lo[0]) * (hi[1] - lo[1]) * (hi[2] - lo[2])
}

/// Claim 1 on one mesh of an uncut block: the faces on each side of the box
/// point out of it, and the signed volume is the box volume.
fn assert_uncut_box_faces_out(name: &str, mesh: &StockMesh) {
    let tris = triangles(mesh);
    assert!(!tris.is_empty(), "{name}: the mesh has no triangle");
    let (mut top, mut bottom, mut sides) = (0, 0, 0);
    for (c, n) in &tris {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len < 1e-12 {
            continue;
        }
        let n = [n[0] / len, n[1] / len, n[2] / len];
        if (c[2] - TOP).abs() < 1e-6 {
            top += 1;
            assert!(
                n[2] > 0.99,
                "{name}: top triangle at {c:?} has normal {n:?}"
            );
        } else if c[2].abs() < 1e-6 {
            bottom += 1;
            assert!(
                n[2] < -0.99,
                "{name}: bottom triangle at {c:?} has normal {n:?}"
            );
        } else {
            sides += 1;
            // A skirt triangle: the normal points away from the box centre.
            let out = [c[0] - 20.0, c[1] - 15.0];
            assert!(
                n[0] * out[0] + n[1] * out[1] > 0.0,
                "{name}: skirt triangle at {c:?} has normal {n:?}"
            );
        }
    }
    assert!(
        top > 0 && bottom > 0 && sides > 0,
        "{name}: {top} {bottom} {sides}"
    );
    let vol = signed_volume(mesh);
    let bbox = bbox_volume(mesh);
    assert!(
        (vol - bbox).abs() < 1e-6 * bbox,
        "{name}: signed volume {vol} is not the box volume {bbox}"
    );
}

#[test]
fn the_uncut_block_faces_out_full_and_strided() {
    let stock = block(1.0);
    assert_uncut_box_faces_out("full", &dexel_stock_to_mesh(&stock));
    assert_uncut_box_faces_out("stride 2", &dexel_stock_to_mesh_strided(&stock, 2));
    assert_uncut_box_faces_out("stride 3", &dexel_stock_to_mesh_strided(&stock, 3));
}

#[test]
fn a_cut_block_keeps_a_positive_volume_below_the_uncut_one() {
    let mut stock = block(1.0);
    let uncut = signed_volume(&dexel_stock_to_mesh(&stock));
    let grid = &mut stock.z_grid;
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            // A pocket 8 mm deep, and a through-hole inside it.
            if (5..15).contains(&row) && (5..20).contains(&col) {
                ray_subtract_above(grid.ray_mut(row, col), (TOP - 8.0) as f32);
            }
            if (8..12).contains(&row) && (8..12).contains(&col) {
                grid.ray_mut(row, col).clear();
            }
        }
    }
    for (name, mesh) in [
        ("full", dexel_stock_to_mesh(&stock)),
        ("stride 2", dexel_stock_to_mesh_strided(&stock, 2)),
    ] {
        let vol = signed_volume(&mesh);
        assert!(
            vol > 0.0 && vol < uncut,
            "{name}: signed volume {vol} is not in (0, {uncut})"
        );
        // The pocket floor is an up-facing face.
        let floor = triangles(&mesh)
            .into_iter()
            .filter(|(c, _)| (c[2] - (TOP - 8.0)).abs() < 1e-6)
            .count();
        assert!(floor > 0, "{name}: no pocket floor triangle");
        for (c, n) in triangles(&mesh) {
            if (c[2] - (TOP - 8.0)).abs() < 1e-6 {
                assert!(
                    n[2] > 0.0,
                    "{name}: floor triangle at {c:?} has normal {n:?}"
                );
            }
        }
    }
}

#[test]
fn the_cavity_pass_gives_a_floor_up_and_a_ceiling_down() {
    let mut stock = block(1.0);
    let (gap_bot, gap_top) = (6.0_f32, 12.0_f32);
    let grid = &mut stock.z_grid;
    for row in 5..15 {
        for col in 5..20 {
            ray_subtract_interval(grid.ray_mut(row, col), gap_bot, gap_top);
        }
    }
    let mesh = dexel_stock_to_mesh(&stock);
    let (mut floors, mut ceilings) = (0, 0);
    for (c, n) in triangles(&mesh) {
        // The cavity triangles are the only ones strictly inside the box at
        // the gap heights.
        let inside = c[0] > 2.0 && c[0] < 38.0 && c[1] > 2.0 && c[1] < 28.0;
        if !inside {
            continue;
        }
        if (c[2] - f64::from(gap_bot)).abs() < 1e-6 {
            floors += 1;
            assert!(n[2] > 0.0, "gap floor at {c:?} has normal {n:?}");
        } else if (c[2] - f64::from(gap_top)).abs() < 1e-6 {
            ceilings += 1;
            assert!(n[2] < 0.0, "gap ceiling at {c:?} has normal {n:?}");
        }
    }
    assert!(
        floors > 0 && ceilings > 0,
        "floors {floors}, ceilings {ceilings}"
    );
}

#[test]
fn the_playback_preview_faces_the_tool() {
    let stock = block(1.0);
    for (direction, sign) in [
        (StockCutDirection::FromTop, 1.0),
        (StockCutDirection::FromBottom, -1.0),
    ] {
        let mesh = dexel_stock_to_entry_surface_mesh(&stock, direction);
        let tris = triangles(&mesh);
        assert!(!tris.is_empty(), "{direction:?}: empty preview");
        for (c, n) in tris {
            assert!(
                n[2] * sign > 0.0,
                "{direction:?}: preview triangle at {c:?} has normal {n:?}"
            );
        }
    }
}

fn drill(profile: ToolProfile) -> DrillOp {
    DrillOp {
        holes: vec![DrillHole {
            xy: [20.0, 15.0],
            top_z: TOP,
            bottom_z: 8.0,
        }],
        hole_source: HoleSource::ModelDerived,
        tool_profile: profile,
        tool_diameter_mm: 6.0,
        cycle: DrillCycle::Simple,
        feed_rate_mm_min: 300.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        material: Material::default(),
        retract_z_mm: TOP,
    }
}

#[test]
fn the_drill_tube_cap_and_cone_face_out_of_the_material() {
    for profile in [ToolProfile::Flat, ToolProfile::StandardTwist] {
        // Cell 1 mm at stride 4: the 4 mm display cell cannot resolve the
        // Ø6 hole (6 < 2 x 4), so the decoration is drawn. The uncut block
        // holds material on every wall column and below the floor.
        let mut mesh = StockMesh::empty();
        append_drill_cylinders(&mut mesh, &block(1.0), 4, &[&drill(profile)]);
        let tris = triangles(&mesh);
        assert!(!tris.is_empty(), "{profile:?}: no drill geometry");
        let (mut wall, mut tip) = (0, 0);
        for (c, n) in tris {
            let to_axis = [20.0 - c[0], 15.0 - c[1]];
            let radial = n[0] * to_axis[0] + n[1] * to_axis[1];
            let horizontal = (n[0] * n[0] + n[1] * n[1]).sqrt();
            if n[2].abs() < 1e-9 * horizontal.max(1.0) {
                // The tube wall: the void is toward the axis.
                wall += 1;
                assert!(radial > 0.0, "{profile:?}: wall at {c:?} has normal {n:?}");
            } else {
                // The flat cap or the cone: the void is above.
                tip += 1;
                assert!(n[2] > 0.0, "{profile:?}: tip at {c:?} has normal {n:?}");
                assert!(
                    radial >= -1e-9,
                    "{profile:?}: tip at {c:?} has normal {n:?}"
                );
            }
        }
        assert!(wall > 0 && tip > 0, "{profile:?}: wall {wall}, tip {tip}");
    }
}

/// Mean RGB of the middle third of a panel.
fn panel_centre_mean(px: &[u8], w: usize, label: &str) -> f64 {
    let panels = composite_panel_layout(W as u32, H as u32);
    let p = panels
        .iter()
        .find(|p| p.label == label)
        .unwrap_or_else(|| panic!("no panel {label}"));
    let (mut sum, mut n) = (0.0, 0.0);
    for y in p.y + p.height / 3..p.y + 2 * p.height / 3 {
        for x in p.x + p.width / 3..p.x + 2 * p.width / 3 {
            let i = (y * w + x) * 4;
            sum += f64::from(px[i]) + f64::from(px[i + 1]) + f64::from(px[i + 2]);
            n += 3.0;
        }
    }
    sum / n
}

const W: usize = 1200;
const H: usize = 800;

/// The `shade.rs` probe as a test. The vertex colour of the uncut top is
/// about 151 of 255 (`probe/RESULTS.txt`). A lit face reads about
/// 0.91 x 151 = 137; a face at the ambient level reads 0.35 x 151 = 53. The
/// gate at 120 sits between the two.
#[test]
fn the_composite_lights_the_top_and_the_bottom_of_an_uncut_block() {
    let stock = TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        },
        1.0,
    );
    let mesh = dexel_stock_to_mesh(&stock);
    let px = render_mesh_composite(&mesh, W as u32, H as u32);
    let top = panel_centre_mean(&px, W, "TOP");
    assert!(
        top > 120.0,
        "TOP panel centre reads {top:.1}; a lit face reads about 137"
    );
    // The bottom face has the dark end of the wood ramp (z = stock bottom),
    // so a lit bottom reads less than a lit top. Measure it against the
    // same mesh with every triangle reversed: the lit side must be the
    // brighter one.
    let mut reversed = mesh;
    for t in reversed.indices.as_chunks_mut::<3>().0 {
        t.swap(1, 2);
    }
    let px_rev = render_mesh_composite(&reversed, W as u32, H as u32);
    let bottom = panel_centre_mean(&px, W, "BOTTOM");
    let bottom_rev = panel_centre_mean(&px_rev, W, "BOTTOM");
    assert!(
        bottom > bottom_rev * 1.5,
        "BOTTOM panel centre reads {bottom:.1}; the inside-out mesh reads {bottom_rev:.1}"
    );
    let top_rev = panel_centre_mean(&px_rev, W, "TOP");
    assert!(
        top > top_rev * 1.5,
        "TOP {top:.1} against inside-out {top_rev:.1}"
    );
}
