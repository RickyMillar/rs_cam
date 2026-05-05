# 3D Finish Toolpath Bugs — From Guided Fuzz Test (2026-04-10)

## Test Setup

- **Model**: terrain.stl — 100x100mm, Z range 0–6.5mm, 211K triangles, open mesh
- **Stock**: 110x110x12mm softwood
- **Roughing**: Adaptive3D, 6mm End Mill, stock_top_z=12
- **Finishing**: Drop Cutter, 2mm Ball Nose, stepover=1.0 (baseline)
- **Tested via**: Embedded MCP tools (live GUI), systematic parameter sweep

---

## Bug 1: Slope filtering generates full raster grid with retracts (Medium)

### Reproduction
1. Load terrain project
2. Add drop_cutter finish with 2mm ball nose
3. Set `slope_from=30`, `slope_to=90`
4. Generate toolpath
5. Observe: 14,413mm rapids vs 6,215mm cutting (2.3x ratio)
6. Set `slope_from=45`, `slope_to=90` — ratio worsens to 4.5x

### Expected behavior
The raster grid should only cover regions where surface slope falls within the
`[slope_from, slope_to]` range. Passes over excluded regions should be eliminated
entirely, not traversed as rapids.

### Actual behavior
The full raster grid is generated regardless of slope filtering. Excluded regions
produce rapid retracts — the tool flies over them at safe Z, plunges back down when
it reaches an included region. This creates massive rapid distances and rapid
collisions.

### Data
| slope_from | slope_to | Cutting (mm) | Rapids (mm) | Rapid/Cut |
|-----------|----------|-------------|------------|-----------|
| 0 | 90 | 12,111 | 949 | 0.08x |
| 0 | 30 | 12,965 | 5,656 | 0.44x |
| 30 | 90 | 6,215 | 14,413 | 2.32x |
| 45 | 90 | 2,577 | 11,472 | 4.45x |
| 60 | 90 | 575 | 1,338 | 2.33x |

### Where to look
The drop_cutter implementation is in `crates/rs_cam_core/src/compute/`. The raster
grid generation creates passes across the full stock XY range. Slope filtering
currently only determines whether each individual move is a cutting move or a retract
— it doesn't trim the grid boundaries.

### Fix approach
Option A: After generating raster lines, partition each line into segments that are
within the slope range. Only emit cutting moves for in-range segments, and use a
single retract between discontinuous segments (instead of per-grid-step retracts).

Option B: Pre-compute a slope mask from the mesh surface, then trim raster lines to
only span regions where the mask indicates in-range slopes. This is more complex but
produces minimal rapids.

---

## Bug 2: min_z doesn't reduce move count (Low)

### Reproduction
1. Load terrain project (Z range 0–6.5mm)
2. Add drop_cutter finish with 2mm ball nose
3. Generate with default min_z=-50 → 10,815 moves
4. Set `min_z=6` (at terrain peak) → still 10,815 moves
5. Set `min_z=3` → still 10,815 moves
6. Cutting distance decreases slightly but move count never changes

### Expected behavior
When `min_z` clamps the tool above most of the surface, passes where the tool would
just fly at the clamped Z height with zero engagement should be eliminated entirely.

### Actual behavior
The full raster grid is always generated. `min_z` only clamps each move's Z
coordinate — passes at the clamped height still exist as zero-engagement cutting
moves. The tool traverses the entire surface at Z=min_z where clamped.

### Data
| min_z | Moves | Cutting (mm) |
|-------|-------|-------------|
| -50 | 10,815 | 12,111 |
| -1 | 10,815 | 12,111 |
| 0 | 10,815 | 12,070 |
| 3 | 10,815 | 11,691 |
| 6 | 10,815 | 10,918 |

### Where to look
Same drop_cutter implementation. The `min_z` clamping happens per-move after grid
generation. Should filter out entire raster lines (or line segments) where all
points are at or above the clamp height.

### Fix approach
After generating the raster grid with Z values, scan each line: if the entire line
(or a contiguous segment) would be at min_z (i.e. the surface is below the clamp
everywhere along that segment), skip those moves or retract over them.

---

## Bug 3: Systemic rapid collisions across all 3D finish operations (Medium)

### Reproduction
1. Load terrain project
2. Generate any roughing + any 3D finish combination
3. Run simulation
4. Observe rapid_collision_count > 0 in every case

### Data (all with Adaptive3D roughing + 2mm ball nose finish)
| Finish Operation | Rapid Collisions |
|-----------------|-----------------|
| Drop Cutter (stepover=1.0) | 252 |
| Scallop | 144 |
| Spiral | 48 |
| Waterline | 68 |
| Steep+Shallow | 1,104 |

### Investigation needed
This could be:
1. **Retract height too low**: Rapids between passes clip remaining stock left by
   roughing (stock-to-leave = 0.3mm radial + 0.3mm axial)
2. **Roughing doesn't fully clear**: The adaptive3d may leave ridges between
   stepover passes that the finishing retract height doesn't clear
3. **Stock boundary clipping**: Rapids at stock edges pass through the stock
   corners where roughing didn't reach
4. **Safe Z calculation bug**: The retract Z is computed incorrectly

### Where to look
- Retract height calculation in the finish operations
- `safe_z` / `retract_z` in the heights config
- How `stock_to_leave` from roughing interacts with finish retract heights
- The simulation's rapid collision detection in `crates/rs_cam_core/src/compute/simulate.rs`

---

## Additional Observations (not bugs)

### Waterline is ineffective on shallow terrain
Only 615mm cutting / 697 moves on a 100x100mm terrain with 6.5mm relief. This is
expected — waterline produces horizontal contours, and shallow terrain has few
Z-level crossings. Not a bug, but the tool could warn when waterline is selected
on a model with low Z variation.

### Steep+Shallow generates high rapids on gentle terrain
8,768mm rapids on this model. The threshold angle (default 30°) creates many small
steep regions scattered across the terrain, and each transition from steep→shallow
requires a retract. Similar root cause to Bug 1 (slope filtering). May benefit from
the same grid-trimming fix.

### Stepover 0.1 has worse air cutting than 0.3
41.8% vs 26.2%. At very fine stepovers, many passes barely engage the remaining
material after the previous pass removed it. This is inherent to raster finishing
but could be flagged as a warning when the user picks stepover < tool_radius/5.
