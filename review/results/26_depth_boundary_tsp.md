# Review: Depth, Boundary, and TSP

## Summary
Three well-separated supporting systems with no critical bugs. Depth stepping (`depth.rs`) is the most mature with 24 tests and two clean distribution modes. Boundary management (`boundary.rs`) provides correct tool containment offsets and a solid clipping state machine. TSP rapid optimization (`tsp.rs`) implements standard nearest-neighbor + 2-opt heuristic. All three modules have zero `unwrap()` in production code and integrate cleanly through the worker pipeline.

## Findings

### Depth (`depth.rs`, 720 lines, 24 tests)

#### Step-Down Calculation
- **Two distribution modes** (`depth.rs:15-22`):
  - `Even`: divides total depth equally across all passes — consistent chip load
  - `Constant`: max step down per pass, shallower final pass
- **Pass count** (`depth.rs:69`): `ceil(roughing_depth / max_step_down)` — standard ceiling division
- **Total depth** (`depth.rs:60`): `(start_z - final_z).max(0.0)` — handles inverted depths

#### Final Pass Handling
- **Even mode** (`depth.rs:105-108`): all passes identical depth (`rough_depth / n`)
- **Constant mode** (`depth.rs:109-116`): max_step_down for all passes except final which gets remainder, clamped to roughing floor
- **Finish allowance** (`depth.rs:78-80`): roughing stops at `final_z + finish_allowance`, then separate finish pass to `final_z`
- **Spring passes** (`depth.rs:127-130`): additional passes at finish depth for dimensional accuracy
- **`all_levels()`** (`depth.rs:121-132`): combines roughing + optional finish + spring passes with safe `unwrap_or(self.final_z)` fallback

#### Integration
- Used by: face.rs, trace.rs, pocket.rs, profile.rs, zigzag.rs
- Two public APIs: `depth_stepped_toolpath()` (single op per level) and `depth_stepped_with_finish()` (separate rough/finish ops)

### Boundary (`boundary.rs`, 430 lines, 12 tests)

#### Tool Containment Modes (`boundary.rs:12-19`)
- `Center`: tool center stays inside boundary (no offset)
- `Inside`: entire tool inside (inset by tool_radius via positive offset)
- `Outside`: tool edge extends outside (outset by negative offset)
- Delegates to `polygon::offset_polygon()` with correct sign convention (`boundary.rs:28-40`)

#### Keep-Out Zones (`boundary.rs:48-56`)
- Adds keep-out footprints as reversed (CW) holes in the boundary polygon
- `contains_point()` automatically excludes holes during clipping

#### Clipping State Machine (`boundary.rs:67-120`)
- 5-case match on `(prev_inside, cur_inside)` transitions:
  - Outside→Inside: rapid to safe_z + plunge with original feed
  - Inside→Outside: retract to safe_z + rapid
  - Inside→Inside: keep move unchanged
  - Outside→Outside: rapid at safe_z
- Preserves feed rates during plunge transitions
- XY-only containment test (ignores Z)

#### Integration (`execute.rs:292-342`)
- Creates stock boundary from bounding box, subtracts keep-outs, applies containment offset, clips toolpath
- Uses `.first()` on `effective_boundary()` result — silently skips if offset collapses geometry

### TSP (`tsp.rs`, 397 lines, 6 tests)

#### Algorithm
- **Phase 1 — Segmentation** (`tsp.rs:27-63`): splits toolpath at rapids into independent cutting segments
- **Phase 2 — Nearest-Neighbor** (`tsp.rs:102-131`): greedy initialization starting from segment 0, O(n²)
- **Phase 3 — 2-opt Improvement** (`tsp.rs:133-167`): iterative local search, max 100 iterations, accepts improvements with 1e-10 tolerance, early exit on no improvement
- **Phase 4 — Reassembly** (`tsp.rs:169-199`): rebuilds toolpath with proper retract/rapid/plunge transitions at safe_z

#### Distance Metric (`tsp.rs:17-21`)
- XY-plane only (Z ignored) — appropriate for rapid travel optimization

#### Integration (`helpers.rs:279-300`)
- Gated by `cfg.optimize_rapid_order` flag (opt-in)
- Applied post-dressups (after entry, dogbones, lead-in/out, arc fitting, feed optimization)

### Cross-Module Data Flow (execute.rs)
```
Operation generation → raw toolpath
  → Dressups (entry, dogbones, lead-in/out, arc fitting, feed opt)
  → Boundary clip (if enabled): subtract_keepouts → effective_boundary → clip_toolpath
  → TSP optimization (if enabled): optimize_rapid_order
```

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Low | Boundary clipping silently skips if `effective_boundary()` returns empty (tool too large for geometry) — no debug logging | `execute.rs:315` |
| 2 | Low | TSP 2-opt is O(100n²) worst case — not documented; could be slow for operations producing hundreds of segments | `tsp.rs:133-167` |
| 3 | Low | TSP greedy always starts from segment 0, no random restarts — can get trapped in local minima | `tsp.rs:102-131` |
| 4 | Low | Constant distribution can produce very shallow final pass if remainder is tiny (e.g., 0.1mm at 3mm max_step) — by design but undocumented | `depth.rs:112-113` |
| 5 | Low | Spring passes field exists in config and `all_levels()` but no integration test verifies they're used by operations | `depth.rs:127-130` |

## Test Gaps
- **Depth**: no test verifying spring passes are actually consumed by operations (only unit-level `all_levels()` test)
- **Boundary**: no test for boundary clipping when `effective_boundary()` returns empty
- **TSP**: no stress test with 100+ segments; no pathological case test; only 6 tests total
- **Cross-module**: no integration test combining depth stepping + boundary clipping + TSP on a realistic multi-pass pocket

## Suggestions
- Add optional debug logging when `effective_boundary()` returns empty (`boundary.rs` / `execute.rs`)
- Add TSP stress test with 100+ segments to validate performance characteristics
- Document Constant distribution's shallow-final-pass behavior with an example in code comments
- Consider adding an integration test that exercises depth → boundary → TSP pipeline end-to-end
- Document TSP algorithm complexity and approximation guarantees in module-level comments
