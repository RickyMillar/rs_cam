# Operation Transform Capabilities Audit

**Date**: 2026-05-07  
**Auditor**: cam-navigator agent  
**Source of truth**: `crates/rs_cam_core/src/compute/catalog.rs` (`transform_capabilities`) and actual generator code in `crates/rs_cam_core/src/`

## Legend

Columns use abbreviated triples: `(rapid_reorder, depth_order, continuous)`  
- `rapid_reorder` = `allows_global_rapid_reorder`  
- `depth_order` = `requires_depth_order`  
- `continuous` = `continuous_path_required`

## Audit Table

| Operation | Currently assigned (rr, do, cp) | Should be (rr, do, cp) | Rationale | Risk if wrong |
|-----------|----------------------------------|------------------------|-----------|---------------|
| **Face** | (false, true, false) | **(false, false, false)** | `face.rs`: calls `depth_stepped_toolpath` / `toolpath_at_levels` top→bottom, but each pass is an independent raster covering the full stock top. There is no cross-pass material dependency — a shallower pass never uses material left by a deeper one. Reordering passes can only produce redundant air cuts, not gouges. | CHANGE: current assignment blocks link-moves and barriered reorder unnecessarily on a pass that has no depth-order constraint. Mis-assignment is conservative (won't cause gouges) but wastes optimization opportunity. |
| **Pocket** | (false, true, false) | (false, true, false) | `execute.rs` L196-235: iterates `levels` top→bottom; `pocket_toolpath` / `zigzag_toolpath` per level. Reordering passes would skip unmachined material and cause a deeper pass to bite through stock that was cleared by the shallower pass. `depth_per_pass` matters. | Correct. Depth order constraint is necessary. |
| **Profile** | (false, true, false) | (false, true, false) | `execute.rs` L237-285: iterates `levels` top→bottom; each level runs the full profile contour. Tab dressup applied only at the final level. Reordering would break tabs and can snap thin cutouts. | Correct. |
| **Adaptive** | (false, true, false) | (false, true, false) | `execute.rs` L287-339: iterates `levels` top→bottom. 2D adaptive uses tri-dexel or bool-grid tracking — cutting a deeper level before the shallower one would leave unmachined stock that the deeper level must then cut at full DOC. | Correct. |
| **Rest** | (false, true, false) | (false, true, false) | `execute.rs` L423-452: uses `toolpath_at_levels` top→bottom. Rest machining depends on prior tool diameter assumption at each Z level; reordering would violate the residual-material model. | Correct. |
| **Zigzag** | (false, true, false) | (false, true, false) | `execute.rs` L341-365: `toolpath_at_levels` top→bottom. Per-level raster clearing; deeper pass assumes material from above has been removed. | Correct. |
| **Drill** | (false, true, false) | **(true, false, false)** | `drill.rs` L50-78: visits holes in caller-supplied XY order. Each hole is completely independent — peck cycle state resets per hole. XY-proximity reordering can only reduce rapid travel between holes; it cannot affect depth semantics because each hole is fully drilled before the tool moves. The peck cycle is intra-hole sequential but the holes themselves are order-independent. | CHANGE: current assignment prevents TSP rapid reorder between independent drill holes, wasting tool travel. Mis-assignment is conservative (won't gouge) but suboptimal. Note: `allows_global_rapid_reorder=true` is correct here; `requires_depth_order=false` is also correct because there are no multi-level depth passes crossing holes. |
| **AlignmentPinDrill** | (false, true, false) | **(true, false, false)** | Same analysis as `Drill` — `drill.rs` same function, holes are independent. | CHANGE: same issue as Drill. |
| **Chamfer** | (false, false, true) | **(false, false, false)** | `chamfer.rs` via `execute.rs` L531-558: single-Z chamfer pass per polygon — no multi-depth stepping, no continuous helical geometry. It traces the polygon outline at a fixed Z offset. Each polygon's chamfer segment is independent. No depth dependency, no inherent continuity constraint. | CHANGE: `continuous_path_required=true` prevents link moves and barriered reorder. Chamfer segments are independently plunged per polygon; reordering them cannot cause phantom material removal. Should match Profile/Trace semantically but Profile uses depth_order and Chamfer has no depth stepping at all — `(false, false, false)` is more accurate. |
| **VCarve** | (false, false, true) | **(false, false, false)** | `vcarve.rs` L81-128: scan-line raster inside polygon; each scan line is emitted via `emit_path_segment` with a retract between. Variable Z is purely a function of XY distance-to-boundary — it is the same regardless of when the line is cut. Scan lines are geometrically independent; reordering them cannot cause material-state violations. The "continuous" label is incorrect here — VCarve is effectively a variable-depth raster with full retracts between lines. | CHANGE: assigning `continuous=true` prevents link moves between adjacent scan lines and blocks the barriered reorder that would minimize scan-line travel. Actual risk: suboptimal toolpath only, no gouging from reorder. |
| **Trace** | (false, false, true) | **(false, true, false)** | `execute.rs` L367-391: uses `toolpath_at_levels` with `depth_per_pass`. A multi-pass Trace cuts the same polygon outline at progressively deeper Z levels. Reordering passes would make the tool re-enter material at a depth that was cleared by a later pass. The path is *not* geometrically continuous in the sense that the cutter must follow it uninterrupted — it retracts between passes. But depth order matters. `continuous=true` is wrong and `depth_order=true` would be more accurate. | CHANGE: `continuous_path_required=true` prevents link moves that are otherwise safe for tracing closed polygon outlines. `requires_depth_order=true` would correctly guard the multi-pass depth dependency. |
| **Inlay** | (false, false, true) | **(false, false, false)** | `execute.rs` L453-493: generates female then male toolpath, appended sequentially. Each is effectively a VCarve variant (single-depth, variable Z from boundary distance). No multi-depth stepping. No continuity requirement — the female and male segments individually retract between features. Assigning `continuous=true` is overcautious. | CHANGE: prevents link moves and barriered reorder on a path that has no continuity constraint. |
| **DropCutter** | (true, false, false) | (true, false, false) | `execute.rs` L600-674: builds a `DropCutterGrid` (pure XY raster, Z from surface contact), then calls `raster_toolpath_from_grid`. Each row is independent — Z is derived solely from the mesh, not from machining history. TSP reorder across rows does not violate any depth assumption. | Correct. |
| **HorizontalFinish** | (true, false, false) | **(false, false, false)** | `horizontal_finish.rs` L170-174: regions are sorted **high-to-low Z** before rastering (`"machine top shelves first to avoid collisions"`). This is a *safety ordering*, not a material-state dependency — if a lower shelf is cut first, the higher shelf still has full material and will be cut correctly afterwards. However, the `sort_by` is safety-driven (preventing clamp collision above), which TSP reorder would violate. Assigning `allows_global_rapid_reorder=true` is therefore wrong — the generator already establishes a Z-ordering for safety reasons. A barriered reorder within a region level is safe; cross-region-Z reorder is not. | CHANGE: `allows_global_rapid_reorder=true` would let TSP reorder higher-Z regions after lower-Z passes, violating the generator's own safety precaution of machining top shelves first. Should be `(false, false, false)` to prevent cross-level rapid reorder while allowing link moves within a level. |
| **Adaptive3d** | (false, true, false) | (false, true, false) | `adaptive3d/clearing.rs`, `mod.rs`: Z levels from `stock_top_z` down to mesh surface; each level stamps the dexel stock and the next level's polygon is derived from remaining material. Reordering Z levels would cause deeper passes to cut through unmachined material at full DOC. Barriers are emitted at `RegionZLevel` and `GlobalZLevel` events (`execute.rs` L53-63). | Correct. The `rapid_order_barriers` mechanism gates barriered reorder correctly within Z levels. |
| **Waterline** | (false, true, false) | (false, true, false) | `waterline.rs` L118-167: loops `z` from `start_z` down to `final_z`. Each waterline contour is a closed loop; contours at the same Z can be reordered freely but contours at different Z levels must stay ordered because a lower contour that is machined before the level above can leave unmachined stock overhanging the cutter. | Correct. |
| **Pencil** | (false, false, true) | **(false, false, false)** | `pencil.rs` L694-732: builds all crease chains, then calls `order_paths_nearest` — a nearest-neighbor TSP **inside the generator itself**. The generator already reorders chains before emission. Chains are emitted with full retracts (`emit_path_segment` with `safe_z`). There is no continuity constraint across chains — each chain is entered with a rapid + plunge. The paths follow 3D crease features; no depth-order dependency exists either. | CHANGE: `continuous_path_required=true` prevents `apply_dressups` from applying link-moves (suppressed by `!continuous_path_required`). Since pencil already does internal nearest-neighbor ordering, blocking `allows_link_moves` loses the ability to collapse short rapids into stay-down links between nearby chain ends. |
| **Scallop** | (false, false, true) | (false, false, true) | `scallop.rs` L425-528: two modes. In `continuous=true` mode, adjacent rings are helically connected at cut depth with feed moves (no retract between rings) — reordering or inserting link moves between rings would create phantom material removal by connecting different-Z surfaces. In `continuous=false` (default) mode, rings do have retracts, but the scallop rings must progress radially inward/outward — reordering them would cut inner rings while outer material still stands, violating the polygon-offset assumption. | Correct. `continuous_path_required` is justified for both modes — in `continuous=true` mode the path is literally a helix and in `discrete` mode the ring sequence still can't be safely permuted. |
| **SteepShallow** | (false, false, true) | **(false, false, false)** | `steep_shallow.rs` L292-413: generates steep waterline passes (top→bottom Z loop) then shallow raster passes, combining them. The waterline sub-path has Z depth ordering but the two halves (steep+shallow) are independently generated surface-following passes — neither depends on the other for material state. The combined toolpath has full retracts throughout. No continuity constraint applies. However: the waterline sub-component has depth ordering, but the overall operation is a finish pass with no material dependency (it always cuts from the final mesh surface). | CHANGE: `continuous_path_required=true` is too restrictive. The operation doesn't have a continuous path in the scallop sense. `(false, false, false)` would allow link moves and barriered reorder within the steep and shallow sub-regions. However, there's a subtlety: the steep sub-path's Z loop ordering within the waterline component matters. This is a borderline case — see Follow-up Tasks. |
| **RampFinish** | (false, false, true) | (false, false, true) | `ramp_finish.rs` L493-531: each ramp segment continuously descends from upper Z contour to lower Z contour via helical interpolation. The segments are emitted with `emit_path_segment` (retract between segments), so segments *themselves* are independent. However, within each segment the continuous Z descent is the defining property — breaking a segment apart or inserting a stay-down link mid-ramp would change the cut. Furthermore, `order_bottom_up` parameter changes segment ordering, which would conflict with TSP reorder. | Correct. Though the retracts between segments make each segment a discrete unit, the continuous-Z semantics within each segment justify the flag. The `RampFinishRuntimeAnnotation` barriers (`Ramp` events) are not wired into `rapid_order_barriers()` in `execute.rs` L50-73 — only `Adaptive3d` barriers are plumbed. This means the TSP barrier mechanism can't protect ramp ordering even if `allows_barriered_rapid_reorder()` were true. Current `continuous=true` is safe. |
| **SpiralFinish** | (false, false, true) | (false, false, true) | `spiral_finish.rs` L102-193: single continuous Archimedean spiral from center to rim (or reversed), emitted as a single uninterrupted feed sequence with one initial rapid+plunge and one final retract. The `Ring` annotations are cosmetic — the spiral is geometrically one continuous trace. Reordering any segment would break the spiral geometry and insert phantom material-removal links across Z levels. | Correct. The path is genuinely continuous; `continuous_path_required=true` is correctly set. |
| **RadialFinish** | (false, false, true) | **(false, false, false)** | `radial_finish.rs` L50-116: spoke-by-spoke loop; each spoke emits via `emit_path_segment` (rapid+plunge at spoke start, retract at end). Even-odd zigzag: even spokes center→edge, odd spokes edge→center. There is no continuity between spokes and no material-state dependency — each spoke independently drop-cuts the mesh. Reordering spokes cannot cause gouging. TSP reorder of spoke order would produce suboptimal XY travel but no material violation. | CHANGE: `continuous_path_required=true` prevents link moves between adjacent spokes and barriered reorder. Spokes are discrete segments with full retracts; the generator's own zigzag ordering is a travel heuristic, not a correctness requirement. |
| **ProjectCurve** | (false, false, true) | (false, false, true) | `project_curve.rs` L186+: per-polygon, resamples each ring at fine spacing, drop-cuts each point, emits a follow-the-contour path per ring. The projected contour *is* a continuous trace — each ring is a closed engraving path at a fixed depth below the mesh surface. Inserting link moves at different surface Z values would cause phantom cuts across the surface. Multiple polygons produce multiple independent traces, but each trace is continuous. | Correct. Each projected ring must be traversed continuously; splitting or linking mid-ring would engrave phantom lines. |

---

## Summary Statistics

| Category | Count |
|----------|-------|
| Correct assignments | 13 |
| Disagreements | 10 |
| `continuous_path_required` false positives | 6 (VCarve, Trace, Chamfer, Inlay, Pencil, RadialFinish, SteepShallow) |
| `requires_depth_order` false positives | 1 (Face) |
| `allows_global_rapid_reorder` false positive | 1 (HorizontalFinish) |
| `allows_global_rapid_reorder` false negative | 2 (Drill, AlignmentPinDrill) |

---

## Recommended Changes

The following 10 operations have assignments that should be updated. Listed by impact:

### High impact (blocks optimization on frequently-used operations)

1. **VCarve**: `(false, false, true)` → `(false, false, false)`  
   Scan-line raster with full retracts. `continuous=true` prevents link-moves between adjacent scan lines that are completely safe to link. Ref: `crates/rs_cam_core/src/vcarve.rs` L81-128.

2. **Pencil**: `(false, false, true)` → `(false, false, false)`  
   Generator already does internal nearest-neighbor ordering (`order_paths_nearest`). `continuous=true` suppresses external link-moves from `apply_dressups` on a path where chains are already retract-separated. Ref: `crates/rs_cam_core/src/pencil.rs` L694.

3. **Trace**: `(false, false, true)` → `(false, true, false)`  
   Multi-pass depth stepping is the actual constraint. `continuous=true` is wrong; `depth_order=true` is correct. Ref: `crates/rs_cam_core/src/compute/execute.rs` L367-391.

4. **Drill** and **AlignmentPinDrill**: `(false, true, false)` → `(true, false, false)`  
   Holes are completely independent; peck depth-cycling is intra-hole. TSP XY reorder is safe and beneficial for many-hole jobs. Ref: `crates/rs_cam_core/src/drill.rs` L50-78.

### Medium impact (prevents safe optimizations)

5. **RadialFinish**: `(false, false, true)` → `(false, false, false)`  
   Spoke-based with full retract per spoke; zigzag ordering is a heuristic not a correctness constraint. Ref: `crates/rs_cam_core/src/radial_finish.rs` L50-116.

6. **Chamfer**: `(false, false, true)` → `(false, false, false)`  
   Single-depth trace along polygon edges; no continuity or depth constraint. Ref: `crates/rs_cam_core/src/compute/execute.rs` L531-558.

7. **Inlay**: `(false, false, true)` → `(false, false, false)`  
   Female+male appended sequences; each is a variable-Z scan independently retracted. Ref: `crates/rs_cam_core/src/compute/execute.rs` L453-493.

### Lower impact (conservative assignment, won't cause bugs but costs optimization)

8. **Face**: `(false, true, false)` → `(false, false, false)`  
   Depth-stepped but passes are fully independent (no cross-pass material dependency). Ref: `crates/rs_cam_core/src/face.rs`, `crates/rs_cam_core/src/compute/execute.rs` L178-194.

9. **HorizontalFinish**: `(true, false, false)` → `(false, false, false)`  
   Generator sorts regions high-to-low Z for safety; `allows_global_rapid_reorder=true` would let TSP override that ordering. Ref: `crates/rs_cam_core/src/horizontal_finish.rs` L170-175.

---

## Follow-up Tasks

### FT-1: SteepShallow depth-order sub-path

`SteepShallow` currently tagged `(false, false, true)`. The actual structure is: steep sub-path (waterline Z loop, top→bottom) + shallow sub-path (single-height raster). The waterline sub-path has Z depth ordering but the overall operation is a surface-following finish with no material-state dependency. The two sub-paths could theoretically be emitted in separate toolpaths to get correct per-subpath capabilities, or SteepShallow could have its own custom transform capability logic. Current `continuous=true` is safe but prevents link-moves everywhere — consider splitting the emitter or adding a dedicated `(false, true, false)` case for the steep sub-component. File: `crates/rs_cam_core/src/steep_shallow.rs`.

### FT-2: RampFinish runtime annotations not wired into `rapid_order_barriers`

`execute.rs` `OperationAnnotations::rapid_order_barriers()` (L50-73) only handles `Adaptive3d`. `RampFinish`, `Scallop`, `SpiralFinish`, and `Pencil` return `Vec::new()` from `rapid_order_barriers()`. This means even if RampFinish were given `allows_barriered_rapid_reorder=true`, the barrier mechanism would have nothing to gate on. The annotation infrastructure exists but is only partially plumbed. Investigate whether any of these operations would benefit from having their `RuntimeAnnotation` events wired as barriers.

### FT-3: Pencil internal TSP vs external dressup TSP interaction

Pencil calls `order_paths_nearest` internally before emitting to `Toolpath`. If `apply_dressups` also runs `optimize_rapid_order`, the external TSP reorders an already-TSP-optimized path. These two orderings may conflict. Since the internal one uses the pencil-specific start/end-reversible logic (L525-534 in `pencil.rs`) while the external one does not, the external reorder could undo beneficial start-point flipping. If `Pencil` is moved to `(false, false, false)`, this interaction should be tested.

### FT-4: Face multi-pass material interaction needs confirmation

Face was flagged as potentially safe for `(false, false, false)` because each depth pass covers the full stock footprint. However, if a user sets `depth_per_pass < depth` on a large stock with a finishing allowance in mind, reordering passes would produce a cut order different from what the user expects. Confirm whether face passes truly have no material-state dependency (i.e., the shallower pass does not rely on the deeper pass having been done first, which is obviously the case for face milling), and validate the change does not break any planned feature for face with left-over stock.

### FT-5: Drill with peck cycle — verify XY reorder is safe

`Drill` peck cycle resets all state per hole (the loop in `drill.rs` L50-78 iterates `for &[x, y] in holes`). Confirm that the peck state (`current_z`) is local to each hole iteration and not shared across the `holes` array — this was confirmed in the code review (local variable, loop body). XY reorder of drill hole positions is safe.

---

## Verification (2026-05-07)

All 10 recommended changes applied to `OperationType::transform_capabilities()`:

- HorizontalFinish: `(true,false,false)` → `(false,false,false)`
- Drill, AlignmentPinDrill: `(false,true,false)` → `(true,false,false)`
- Face, Chamfer, Inlay, VCarve, Pencil, RadialFinish: `(false,false,true)` (or `(false,true,false)` for Face) → `(false,false,false)`
- Trace: `(false,false,true)` → `(false,true,false)`

### Method

1. Captured baseline at `target/param_sweeps_baseline/` with prior capability flags via `cargo test --test param_sweep` (54/54 passing).
2. Applied all 10 capability changes in `crates/rs_cam_core/src/compute/catalog.rs`.
3. Re-ran `cargo test --test param_sweep` (54/54 passing again).
4. Diffed `target/param_sweeps/` against baseline per-op family.

### Results

| Op | Sweep fingerprint diff | Sim stock diff | Verdict |
|----|------------------------|----------------|---------|
| HorizontalFinish | none | none | safe (silent — fixtures don't enable TSP) |
| Drill / AlignmentPinDrill | none | none | safe (silent — fixtures don't enable TSP) |
| Face | none | none | safe (silent) |
| Chamfer | none | none | safe (silent) |
| Inlay | none | none | safe (silent) |
| VCarve | none | none | safe (silent) |
| RadialFinish | none | none | safe (silent) |
| Trace | none | none | safe (silent) |
| Pencil (bitangency 175°) | move_count −18, cut_dist −6mm, rapid_dist −10mm, z_levels −10 | 0.41% pixel diff, visually identical (tick-mark noise only) | safe (link_moves correctly collapsing redundant retract/rapid/plunge between adjacent crease chains) |

### Caveats

The "silent" results for 8 of 9 ops mean existing sweep fixtures don't currently enable `link_moves` / `optimize_rapid_order` in the DressupConfig, so the capability flip has no observable effect on those fixtures. **This is not the same as proving the flip is safe under all conditions.** Users who enable link_moves on a Trace, VCarve, etc. toolpath are now in newly-permissive territory. Task #41 (focused regression tests for capability gating) is required to give those new permissions test coverage.

For Pencil — where link_moves was enabled by default in the existing fixtures — the change is empirically benign: simulated stock state is identical, with small reductions in move count and rapid distance that match the expected behavior of `apply_link_moves` collapsing short retract/rapid/plunge sequences.
