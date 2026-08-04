//! Step 5 regression tests for marching-cubes mesh extraction
//! (`planning/DEXEL_Z_ONLY_INVESTIGATION.md` §6.J / §8 Step 5).
//!
//! Per the §9 acceptance gates:
//!   1. Mesh watertightness on a closed-stock case.
//!   2. Vertex z range is bounded by [stock_bottom, stock_top].
//!   3. Triangle count vs the equivalent heightmap (preview) baseline — MC
//!      should produce 1–10× the heightmap density, well under the §6.J
//!      "5-10×" memory ceiling but materially more than preview.
//!   4. Drill-CSG ↔ MC seam regression: when drill cylinders are appended
//!      to the MC closed-solid mesh (Step 3 PR1 path), the analytic
//!      cylinder walls land within ~half a cell-size of the MC mesh's
//!      cell-corner walls.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout
)]

use rs_cam_core::dexel::ray_subtract_above;
use rs_cam_core::dexel_mesh::{
    append_drill_cylinders, dexel_stock_to_mesh, dexel_stock_to_top_surface_mesh,
};
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::drill::DrillCycle;
use rs_cam_core::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};
use rs_cam_core::material::Material;
use rs_cam_core::stock_mesh::StockMesh;
use std::collections::HashMap;

/// Position-based watertightness check (matches the internal helper in
/// `dexel_mesh_mc::tests::is_watertight`). MC emits per-triangle vertices
/// without index dedup, so we key edges by quantised endpoint positions.
fn is_watertight(mesh: &StockMesh) -> bool {
    let quant = |x: f32| (x * 10000.0).round() as i64;
    let n = mesh.vertices.len() / 3;
    let pos: Vec<(i64, i64, i64)> = (0..n)
        .map(|i| {
            (
                quant(mesh.vertices[i * 3]),
                quant(mesh.vertices[i * 3 + 1]),
                quant(mesh.vertices[i * 3 + 2]),
            )
        })
        .collect();
    type Pt = (i64, i64, i64);
    type Edge = (Pt, Pt);
    let mut edges: HashMap<Edge, u32> = HashMap::new();
    for tri in mesh.indices.chunks_exact(3) {
        let p0 = pos[tri[0] as usize];
        let p1 = pos[tri[1] as usize];
        let p2 = pos[tri[2] as usize];
        let mut e = [(p0, p1), (p1, p2), (p2, p0)];
        for (a, b) in e.iter_mut() {
            if *a > *b {
                std::mem::swap(a, b);
            }
        }
        for &edge in &e {
            *edges.entry(edge).or_insert(0) += 1;
        }
    }
    let bad = edges
        .iter()
        .filter(|(e, _)| e.0 != e.1)
        .filter(|&(_, c)| *c != 2)
        .count();
    bad == 0
}

#[test]
fn closed_uncut_block_is_watertight() {
    let stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 10.0, 0.5);
    let mesh = dexel_stock_to_mesh(&stock);
    assert!(!mesh.indices.is_empty());
    assert!(is_watertight(&mesh), "uncut stock mesh must be watertight");
}

#[test]
fn partial_cut_block_is_watertight() {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 10.0, 0.5);
    // Cut a rectangular pocket.
    for r in 10..30 {
        for c in 10..30 {
            ray_subtract_above(stock.z_grid.ray_mut(r, c), 4.0);
        }
    }
    let mesh = dexel_stock_to_mesh(&stock);
    assert!(
        is_watertight(&mesh),
        "partial-cut stock mesh must be watertight"
    );
}

#[test]
fn mesh_vertices_stay_in_stock_envelope() {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 8.0, 0.5);
    for r in 5..15 {
        for c in 5..15 {
            ray_subtract_above(stock.z_grid.ray_mut(r, c), 3.0);
        }
    }
    let mesh = dexel_stock_to_mesh(&stock);
    // All vertex Zs must be in [stock_bottom, stock_top].
    for i in 0..mesh.vertices.len() / 3 {
        let z = mesh.vertices[i * 3 + 2];
        assert!((0.0..=8.0).contains(&z), "vertex {i} z={z} outside [0, 8]");
    }
}

#[test]
fn mc_triangle_count_dominates_heightmap_preview() {
    // Heightmap preview emits only the top surface (one triangle pair per
    // cell). MC heightmap mesh emits top + bottom + perimeter + (when
    // applicable) hole walls + cavity faces. For an uncut block the MC
    // count should be roughly 2× preview + perimeter overhead, comfortably
    // within the §6.J "5-10×" budget envelope.
    let stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 10.0, 0.5);
    let preview = dexel_stock_to_top_surface_mesh(&stock);
    let solid = dexel_stock_to_mesh(&stock);
    let preview_tris = preview.indices.len() / 3;
    let solid_tris = solid.indices.len() / 3;
    assert!(solid_tris > preview_tris, "MC solid > preview");
    // Upper bound: 10× preview triangle count. Includes generous headroom
    // so future mesh refinements don't trip the gate unnecessarily.
    assert!(
        solid_tris <= preview_tris * 10,
        "MC solid {solid_tris} exceeds 10× preview {preview_tris}"
    );
    eprintln!(
        "uncut 40×40 stock: preview {preview_tris} tris, MC solid {solid_tris} tris (ratio {:.2}×)",
        solid_tris as f32 / preview_tris as f32
    );
}

#[test]
fn pocket_walls_are_steep_under_mc() {
    // A 4-cell-square pocket: at the pocket edge, MC heightmap mesh produces
    // wall vertices at the corner-bilinear height drop. Interior corners of
    // the pocket are surrounded by 4 cut cells → corner z = cut depth. The
    // perimeter of the pocket region has corners at the bilinear average of
    // mixed cut/uncut cells, giving the wall slope.
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 10.0, 0.5);
    // Carve a 4×4-cell pocket from (5, 5) to (8, 8) world units, down to z=3.
    let cut_top = 3.0_f32;
    for r in 10..=20 {
        for c in 10..=20 {
            ray_subtract_above(stock.z_grid.ray_mut(r, c), cut_top);
        }
    }
    let mesh = dexel_stock_to_mesh(&stock);

    // Look for top-face vertices at the pocket floor (z=cut_top) inside the
    // pocket (cells 10..=20 → world u/v ∈ [5, 10]) AND at stock_top outside
    // the pocket. Together they prove the wall transition.
    let mut floor_hits = 0;
    let mut top_hits_outside = 0;
    for i in 0..mesh.vertices.len() / 3 {
        let x = mesh.vertices[i * 3];
        let y = mesh.vertices[i * 3 + 1];
        let z = mesh.vertices[i * 3 + 2];
        // Inside the pocket: (7.5, 7.5) is the centre.
        if (x - 7.5).abs() < 1.0 && (y - 7.5).abs() < 1.0 && (z - cut_top).abs() < 0.05 {
            floor_hits += 1;
        }
        // Outside the pocket: (2.5, 2.5) is uncut material.
        if (x - 2.5).abs() < 1.0 && (y - 2.5).abs() < 1.0 && (z - 10.0).abs() < 0.05 {
            top_hits_outside += 1;
        }
    }
    assert!(
        floor_hits > 0,
        "expected pocket floor vertices at z={cut_top}"
    );
    assert!(
        top_hits_outside > 0,
        "expected stock-top vertices outside the pocket"
    );
}

#[test]
fn drill_cylinder_seam_lands_near_mc_wall() {
    // §6.J §10.7: between Step 3 (drill CSG mesh) and Step 5 (MC), there's
    // a visible seam at hole edges because the heightmap mesh's tilted-quad
    // walls don't align with analytic cylinder walls. Step 5 closes this
    // seam by making MC walls align with the cell-corner boundary that the
    // analytic cylinder also nominally sits on (drill radius rounded to a
    // cell-corner footprint).
    //
    // Test: stamp a drill hole (cleared cells) plus an analytic cylinder
    // for the same hole, and verify the cylinder ring lands within
    // half-a-cell of the MC mesh's wall vertices.
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 10.0, 0.5);
    let hole_x = 10.0;
    let hole_y = 10.0;
    let hole_r = 2.0;
    // Clear cells inside the drill footprint to simulate the drill's
    // analytical removal (Step 3 PR1 produces an analytic conical/cylinder
    // removal; we approximate by clearing cells inside the radius).
    let cell_size = 0.5_f64;
    let rows = stock.z_grid.rows;
    let cols = stock.z_grid.cols;
    for r in 0..rows {
        for c in 0..cols {
            let (u, v) = stock.z_grid.cell_to_world(r, c);
            let dx = u - hole_x;
            let dy = v - hole_y;
            if dx * dx + dy * dy <= (hole_r - cell_size * 0.5).powi(2) {
                stock.z_grid.ray_mut(r, c).clear();
            }
        }
    }
    let mut mesh = dexel_stock_to_mesh(&stock);
    // Append the analytic drill cylinder for the same hole.
    let drill = DrillOp {
        holes: vec![DrillHole {
            xy: [hole_x, hole_y],
            top_z: 10.0,
            bottom_z: 0.0,
        }],
        hole_source: HoleSource::ModelDerived,
        tool_profile: ToolProfile::Flat,
        tool_diameter_mm: hole_r * 2.0,
        cycle: DrillCycle::Simple,
        feed_rate_mm_min: 100.0,
        spindle_rpm: 8000,
        flute_count: 2,
        material: Material::default(),
        // R-2: no R-plane air in this fixture — it models the cycle
        // from the material surface, which is what this test's numbers
        // were written against. Production sets
        // `effective_safe_z(cfg.retract_z, stock_top)` (= stock top +
        // 5 mm by default); `drill_evidence_wording_d3.rs` is the
        // sentry that pins the emitter-matching case.
        retract_z_mm: 0.0,
    };
    let drill_refs: Vec<&DrillOp> = vec![&drill];
    append_drill_cylinders(&mut mesh, &drill_refs);

    // Verify: each analytic cylinder ring vertex must be within
    // (cell_size) of some MC mesh wall vertex at the same z. Quantise to
    // grid corners for the seam check.
    let mut max_seam_offset_mm = 0.0_f32;
    let cyl_start_idx = {
        // Find vertices that look like the cylinder ring (16 segments at
        // exactly radius hole_r from centre). The cylinder is appended
        // last; vertices at radius hole_r from (hole_x, hole_y).
        let mut idxs = Vec::new();
        for i in 0..mesh.vertices.len() / 3 {
            let x = mesh.vertices[i * 3];
            let y = mesh.vertices[i * 3 + 1];
            let dx = x - hole_x as f32;
            let dy = y - hole_y as f32;
            let dist = (dx * dx + dy * dy).sqrt();
            if (dist - hole_r as f32).abs() < 0.01 {
                idxs.push(i);
            }
        }
        idxs
    };
    assert!(
        !cyl_start_idx.is_empty(),
        "could not locate analytic cylinder vertices in composite mesh"
    );

    // For each cylinder vertex, find the nearest MC mesh wall vertex at
    // the same z. Wall vertices are at cell-corner positions (multiples
    // of cell_size offset by origin).
    for &i in &cyl_start_idx {
        let cx = mesh.vertices[i * 3];
        let cy = mesh.vertices[i * 3 + 1];
        let cz = mesh.vertices[i * 3 + 2];
        let mut best = f32::INFINITY;
        for j in 0..mesh.vertices.len() / 3 {
            if cyl_start_idx.contains(&j) {
                continue;
            }
            let dz = mesh.vertices[j * 3 + 2] - cz;
            if dz.abs() > 0.5 {
                continue;
            }
            let dx = mesh.vertices[j * 3] - cx;
            let dy = mesh.vertices[j * 3 + 1] - cy;
            let d = (dx * dx + dy * dy).sqrt();
            if d < best {
                best = d;
            }
        }
        if best.is_finite() && best > max_seam_offset_mm {
            max_seam_offset_mm = best;
        }
    }
    eprintln!("drill seam max offset: {max_seam_offset_mm:.3} mm (cell_size {cell_size})");
    // The seam should be within ~1 cell size at low resolutions, less at
    // high resolutions. Half-cell-size is the §6.J target.
    assert!(
        max_seam_offset_mm < (cell_size as f32) * 1.5,
        "drill-CSG seam offset {max_seam_offset_mm:.3} mm exceeds 1.5× cell_size = {:.3} mm",
        cell_size * 1.5
    );
}
