# Adaptive3D Incomplete Clearing — Uncut Bands Between Contour Passes

## Bug Description

The adaptive3d operation with `contour_parallel` clearing strategy leaves uncut
bands of material at each Z level before stepping down to the next depth. The tool
doesn't fully clear the current level — visible as ridges/strips of remaining stock
between concentric contour passes.

## Reproduction

1. Load terrain project: `terrain.stl` — 100x100mm, Z range 0–6.5mm, 211K triangles
2. Stock: 110x110x12mm
3. Adaptive3D roughing with 6mm end mill:
   - stepover: 2.5mm (42% of tool diameter)
   - depth_per_pass: 3.0mm
   - stock_to_leave_radial: 0.3mm
   - stock_to_leave_axial: 0.3mm
   - stock_top_z: 12.0
   - clearing_strategy: contour_parallel
4. Generate toolpath → 1,865 moves, 8,522mm cutting
5. Simulate at 0.3mm resolution
6. Screenshot checkpoint 0 (roughing only) → visible uncleared bands

The bands are most visible in the top-down view. Dark strips of remaining material
run between the concentric contour passes, especially in the rectangular clearing
region at the center of the stock.

## Expected Behavior

Each Z level should be fully cleared (within stock_to_leave tolerance) before the
tool steps down. The contour_parallel strategy should offset inward until the
remaining island is smaller than the tool, then move to the next Z level.

## Actual Behavior

The contour offsets leave gaps between passes. The tool steps down to the next Z
level with material still remaining at the current level. This creates ridges that
the finishing pass has to remove — increasing finish cycle time and potentially
leaving scallops taller than expected.

## Investigation Guidance

### Where to look

The adaptive3d implementation is in `crates/rs_cam_core/src/compute/`. Find:

1. **Contour parallel clearing logic**: How are contour offsets generated? Is the
   stepover applied as the offset distance between successive contours? If so, a
   2.5mm stepover with a 6mm tool means 2.5mm between contour centers — the tool
   should overlap by 3.5mm (6mm diameter - 2.5mm stepover), which should leave no
   gaps. Unless the offset is computed differently.

2. **Region detection**: How are clearable regions identified at each Z level?
   Are they computed from the stock-model intersection? Could some regions be
   missed or split incorrectly?

3. **Completion check**: Is there a check that verifies the current Z level is
   fully cleared before stepping down? Or does it just run N contour offsets and
   move on?

4. **Stock-to-leave interaction**: Does stock_to_leave affect the contour offset
   calculation? Could it cause the effective stepover to be wider than intended?

### Possible causes

1. **Stepover applied to contour boundary, not tool center**: If the 2.5mm offset
   is applied to the contour boundary rather than the tool center path, the actual
   center-to-center distance would be 2.5mm + tool_radius = 5.5mm, leaving a
   2.5mm-wide uncut strip (since the tool only cuts 3mm to each side of center).

2. **Off-by-one in contour offset loop**: The innermost contour might terminate
   one step too early, leaving a small island.

3. **Polygon offset numerical issues**: The cavalier_contours polygon offset
   library might produce slightly undersized offsets at certain geometries, causing
   the next offset to miss a thin strip.

4. **Clearing strategy not iterating to completion**: The contour loop might be
   count-limited rather than area-limited.

### Test approach

1. Create a simple test case: flat square stock (100x100x10mm), no model (or a
   simple flat model), adaptive3d with known stepover
2. Generate toolpath, simulate, check for complete clearing
3. Vary stepover from 50% to 90% of tool diameter and check each
4. If the bug only appears at certain stepovers or stock sizes, that narrows the
   cause

## Key files

| File | What to check |
|------|---------------|
| `crates/rs_cam_core/src/compute/` | Find adaptive3d / contour_parallel impl |
| `crates/rs_cam_core/src/compute/operation_configs.rs` | Adaptive3dConfig struct (line ~400) |
| `crates/rs_cam_core/src/compute/catalog.rs` | Operation dispatch |
| `crates/rs_cam_core/src/polygon.rs` | `pocket_offsets()` — contour offset generation |

## Priority

Medium — the finishing pass covers the missed areas, but it increases finish cycle
time and can produce visible artifacts on steep walls where the finish stepover
doesn't fully overlap the roughing ridges.
