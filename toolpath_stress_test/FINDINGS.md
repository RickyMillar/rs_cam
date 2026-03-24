# Parameter Sweep Findings

Results from physical inspection of 105 parameter-variant toolpaths across 22 operations.

## Critical / High Severity

### 1. Pocket core function has no depth stepping
**Severity**: HIGH (test-only — GUI wraps with depth stepping)
**Operation**: Pocket (core `pocket_toolpath()`)
**Evidence**: `cut_depth=-10.0` produces identical 42 moves and only 2 z_levels (`[-10, 10]`) as `cut_depth=-3.0`. The tool plunges 10mm in a single pass.
**Root cause**: `pocket_toolpath()` generates a single-Z-level pocket. The GUI/CLI wraps it with `depth_stepped_toolpath()` for multi-pass. The core API is dangerous if called directly without the wrapper.
**Risk**: If depth_per_pass is set equal to total depth (or the wrapper is bypassed), the tool will plunge full depth in one shot.
**Action**: Verify the GUI ALWAYS applies depth stepping. Consider adding a guard to the core function.

## Medium Severity

### 2. Adaptive slot_clearing=true produces fewer moves than false
**Severity**: MEDIUM
**Operation**: Adaptive 2D
**Evidence**: slot_clearing=true gives 98 moves / 258mm rapid. slot_clearing=false gives 151 moves / 207mm rapid. This is backwards — slot clearing should ADD a center slot pass, not suppress offset passes.
**Root cause**: The slot clearing pass may be replacing inner contour passes rather than supplementing them.
**Action**: Review adaptive slot clearing logic. The center slot should be in addition to offset passes, not instead of them.

### 3. Adaptive min_cutting_radius emits zero arc moves
**Severity**: MEDIUM
**Operation**: Adaptive 2D
**Evidence**: min_cutting_radius=3.0 produces 233 moves (vs 134 baseline) but arc_cw_count=0 and arc_ccw_count=0. All corner smoothing is linearized into hundreds of tiny G1 segments.
**Impact**: CNC controller must buffer/process 80% more segments. Corner surface finish will show faceting.
**Root cause**: The contour smoother linearizes arcs before emitting toolpath moves.
**Action**: Emit G2/G3 arcs for min_cutting_radius corners. The arc fitting dressup could be applied, but the base operation should emit arcs natively.

### 4. Adaptive3D z_blend=true: more Z levels but less total work
**Severity**: MEDIUM
**Operation**: Adaptive 3D
**Evidence**: z_blend=true produces +58% more z_levels (341 vs 216) but -19% moves (655 vs 806) and -25% cutting distance (1749mm vs 2321mm). More levels with less work suggests possible incomplete coverage at some levels.
**Action**: Visual inspection of z_blend stock heightmap. Verify no uncut regions remain between blend levels.

## Low Severity

### 5. Adaptive rapid fraction 2-3x higher than pocket
**Severity**: LOW
**Operation**: Adaptive 2D
**Evidence**: Adaptive baseline rapid_fraction=0.319 vs pocket's 0.149 for the same 40x30mm rectangle. The adaptive burns 258mm in rapids vs pocket's 92mm.
**Root cause**: EDT-based offset generation may create disconnected contour fragments requiring frequent retract-and-reposition. Stay-down linking may not be effective.
**Action**: Review link-move and stay-down logic for adaptive. Consider more aggressive pass linking.

### 6. Steep/Shallow high rapid fraction at threshold=30
**Severity**: LOW
**Operation**: Steep/Shallow
**Evidence**: rapid_fraction=0.397 (nearly 40% wasted travel) at threshold_angle=30. The steep pass generates highly fragmented cutting segments.
**Action**: Review steep pass generation for contour continuity. Consider TSP optimization of disconnected steep segments.

### 7. Adaptive3D clearing strategy has higher rapids
**Severity**: LOW
**Operation**: Adaptive 3D
**Evidence**: Adaptive strategy rapid_fraction=0.357 vs ContourParallel's 0.308. The curvature-adjusted strategy retracts more often between passes.
**Action**: Review pass linking in the curvature-adjusted clearing mode.

## Verified Correct (selected highlights)

| Operation | Check | Result |
|-----------|-------|--------|
| Chamfer | Z depth = -(width * tan(angle) + tip_offset) | Exact: -1.1 and -2.1 |
| Profile | Side shift = 2 * tool_radius | Exact: 6.35mm |
| Trace | Compensation = tool_radius | Exact: 3.175mm |
| Face | stock_offset shifts boundary | Exact to the mm |
| Face | depth=6, depth_per_pass=1 → 6 Z levels at 1mm | Correct |
| Trace | depth=3, depth_per_pass=0.5 → 6 Z levels | Correct |
| Drill peck | 4 pecks at 3mm increment to -10mm | Correct |
| Drill simple | Only safe_z, retract_z, final_depth | Correct |
| DropCutter | min_z floor clamp | No Z values below min_z |
| Waterline | Z levels evenly spaced at z_step intervals | Perfect |
| Ramp Finish | Climb vs Conventional: same cutting coverage (<0.4% diff) | Correct |
| Adaptive3D | stock_top_z=22 but stops at hemisphere peak z=20 | No air cutting |
| Radial | angular_step=2 → 2.5x spokes vs step=5 | Proportional |
| Scallop | height halved → ~2x contour rings | Proportional |

## NO_EFFECT Parameters (explained)

| Parameter | Reason |
|-----------|--------|
| pocket/climb, profile/climb | Direction reversal invisible to aggregate metrics |
| face/direction | OneWay vs Zigzag identical for single-pass skim |
| scallop/direction | Symmetric hemisphere — identical either way |
| pencil/bitangency_angle, num_offset_passes | Hemisphere has no creases |
| inlay/glue_gap | Only affects male plug; test fingerprints female |
