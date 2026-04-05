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

### 2. ~~Adaptive slot_clearing=true produces fewer moves than false~~ RESOLVED — expected behavior
**Severity**: ~~MEDIUM~~ NOT A BUG
**Operation**: Adaptive 2D
**Evidence**: slot_clearing=true gives 98 moves / 258mm rapid. slot_clearing=false gives 151 moves / 207mm rapid.
**Root cause**: Not a bug. The slot clearing phase (lines 1141-1182 of `adaptive.rs`) pre-clears cells via `grid.clear_circle()` along 3 zigzag seed lines. This lets the adaptive spiral loop converge faster (material_fraction drops below 0.01 sooner), resulting in fewer total passes. The boundary cleanup pass always runs unconditionally regardless of the flag. The higher rapid distance comes from retract moves between the seed lines themselves.
**Resolution**: Working as designed. Fewer moves with slot_clearing=true is the expected efficiency gain from pre-seeding.

### 3. ~~Adaptive min_cutting_radius emits zero arc moves~~ FIXED
**Severity**: ~~MEDIUM~~ FIXED
**Operation**: Adaptive 2D
**Evidence**: min_cutting_radius=3.0 produced 233 moves but arc_cw_count=0 and arc_ccw_count=0.
**Root cause**: `blend_corners()` in `adaptive_shared.rs` computed correct arc geometry (center, radius, sweep) but immediately sampled it into linearized P2 points. The emitter only called `feed_to()` (G1).
**Fix**: Added `BlendedMove` enum and `blend_corners_to_moves()` that preserves arc descriptors. The 2D adaptive emitter now dispatches `BlendedMove::Arc` to `arc_cw_to()`/`arc_ccw_to()` (G2/G3). The 3D emitter keeps linearization since Z varies per point. 3 new tests validate arc emission, center-on-radius geometry, and straight-line passthrough.

### 4. ~~Adaptive3D z_blend=true: more Z levels but less total work~~ RESOLVED — expected behavior
**Severity**: ~~MEDIUM~~ NOT A BUG
**Operation**: Adaptive 3D
**Evidence**: z_blend=true produces +58% more z_levels (341 vs 216) but -19% moves (655 vs 806) and -25% cutting distance (1749mm vs 2321mm).
**Root cause**: Not a coverage gap. The fingerprint's `z_levels` metric counts distinct Z values in output moves (deduped at 0.001). With z_blend, each contour ring cuts at a different blended Z (outer rings at z_level, inner rings descending toward terrain), producing many more distinct Z heights in the output — this is the intended behavior. The -25% cutting distance is an efficiency gain: blended passes follow the terrain surface more closely, so less redundant flat cutting at each Z level. Z level generation itself (lines 2435-2528) is unaffected by z_blend.
**Resolution**: Working as designed. The +58% z_levels reflects more distinct Z heights per ring (expected blending), not extra clearing passes.

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
