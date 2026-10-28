"""D68: writes the option 1 prototype (the region side) into the fixture worktree, on top of the option 2 prototype
(apply_option2.py + the pre-border band read). Option 1 runs only when RS_CAM_D68_OPTION1 is set:
- on prior stock, the boundary clear keeps the cells a disc centred inside the boundary reaches (the G-CLIPAIR reach clear),
  for every strategy;
- a level region (the CP material grid and the slice region grid) holds the cells inside the centre containment that have
  material under the centre, OR whose disc (R + cs/2) meets a cell OUTSIDE the containment with material above the level.
  Cells outside the containment are never region cells.
Usage: python3 apply_option1.py <worktree>"""
import sys
from pathlib import Path
A = Path(sys.argv[1]) / "crates/rs_cam_core/src/adaptive3d"
code = r'''
// ── D68 prototype, G-RESTDOC option 1: the region side ────────────────

/// D68 (prototype, option 1): restrict a padded level grid to the centre
/// containment, and add the containment cells whose disc meets material
/// outside it above the level.
pub(super) fn option1_regions(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    grid: &mut [bool],
    padded_rows: usize,
    padded_cols: usize,
) {
    let Some(inside) = ctx.option1_inside.as_ref() else {
        return;
    };
    let rows = material_stock.z_grid.rows;
    let cols = material_stock.z_grid.cols;
    if padded_rows != rows + 2 || padded_cols != cols + 2 || inside.len() != rows * cols {
        return;
    }
    let mut src = vec![false; padded_rows * padded_cols];
    let mut any = false;
    for r in 0..rows {
        for c in 0..cols {
            if !inside.get(r * cols + c).copied().unwrap_or(true)
                && cell_has_material(material_stock, surface_hm, r, c, z_level, ctx.stock_to_leave)
                && let Some(s) = src.get_mut((r + 1) * padded_cols + c + 1)
            {
                *s = true;
                any = true;
            }
        }
    }
    let reach_cells = (ctx.tool_radius + material_stock.z_grid.cell_size * 0.5)
        / material_stock.z_grid.cell_size;
    let dist = any.then(|| crate::geometry::grid_field::distance_transform_2d(&src, padded_rows, padded_cols));
    for r in 0..rows {
        for c in 0..cols {
            let i = (r + 1) * padded_cols + c + 1;
            let ins = inside.get(r * cols + c).copied().unwrap_or(true);
            let near = dist.as_ref().is_some_and(|d| d.get(i).copied().unwrap_or(f64::INFINITY) <= reach_cells);
            if let Some(g) = grid.get_mut(i) {
                *g = ins && (*g || near);
            }
        }
    }
}
'''
p = A / "clearing.rs"; s = p.read_text()
marker = "\n// ── D68 prototype, G-RESTDOC option 2"
assert marker in s
s = s.replace(marker, code + marker, 1)
old = """    pub(super) band_edge: std::cell::RefCell<Option<BandEdge>>,
}"""
assert old in s
s = s.replace(old, """    pub(super) band_edge: std::cell::RefCell<Option<BandEdge>>,
    /// D68 prototype (G-RESTDOC option 1): per planner cell, the centre is
    /// inside the containment.
    pub(super) option1_inside: Option<Vec<bool>>,
}""", 1)
# CP: both grid builds
old = """    let (material_grid, rows, cols, origin_x, origin_y, cell_size) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        area_mask,
    );

    let mat_count"""
assert old in s
s = s.replace(old, """    let (mut material_grid, rows, cols, origin_x, origin_y, cell_size) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        area_mask,
    );
    option1_regions(ctx, material_stock, surface_hm, z_level, &mut material_grid, rows, cols);

    let mat_count""", 1)
old = """    let (cleanup_grid, cr, cc, co_x, co_y, c_cs) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        area_mask,
    );"""
assert old in s
s = s.replace(old, """    let (mut cleanup_grid, cr, cc, co_x, co_y, c_cs) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        area_mask,
    );
    option1_regions(ctx, material_stock, surface_hm, z_level, &mut cleanup_grid, cr, cc);""", 1)
old = """        if let Some(clip) = ctx.region_clip.as_ref() {
            clip.restrict_regions(&mut grid, rows, cols);
        }
        grid"""
assert old in s
s = s.replace(old, """        if let Some(clip) = ctx.region_clip.as_ref() {
            clip.restrict_regions(&mut grid, rows, cols);
        }
        option1_regions(ctx, material_stock, surface_hm, z_level, &mut grid, rows, cols);
        grid""", 1)
p.write_text(s)

p = A / "path.rs"; s = p.read_text()
old = """        let reach = (params.clearing_strategy == ClearingStrategy3d::GeodesicAdaptive
            && matches!(params.linking.region_ordering, RegionOrdering::Global))"""
assert old in s
s = s.replace(old, """        let reach = ((params.clearing_strategy == ClearingStrategy3d::GeodesicAdaptive
            && matches!(params.linking.region_ordering, RegionOrdering::Global))
            || (std::env::var_os("RS_CAM_D68_OPTION1").is_some()
                && params.initial_stock.is_some()))""", 1)
old = """        band_edge: std::cell::RefCell::new("""
assert old in s
s = s.replace(old, """        option1_inside: (std::env::var_os("RS_CAM_D68_OPTION1").is_some()
            && params.initial_stock.is_some())
        .then(|| {
            centre_clip.as_ref().map(|clip| {
                let g = &material_stock.z_grid;
                let mut v = Vec::with_capacity(g.rows * g.cols);
                for row in 0..g.rows {
                    for col in 0..g.cols {
                        let (x, y) = g.cell_to_world(row, col);
                        let p = crate::geo::P2::new(x, y);
                        v.push(clip.polygons().iter().any(|q| q.contains_point(&p)));
                    }
                }
                v
            })
        })
        .flatten(),
""" + old, 1)
p.write_text(s)
print("ok")
