# Review: Operation Consistency

## Summary
All 22 operations produce the same Toolpath IR and receive uniform dressup application. However, there are inconsistencies in error return types (2D ops return `Result<_, String>`, 3D ops return `Result<_, ComputeError>`), height parameter semantics differ between 2D and 3D operations, and tool-type validation is only performed for VCarve/Inlay but not for tool-sensitive operations like Scallop. Dressup application is centralized and uniform, which is architecturally sound.

## Findings

### Operation Inventory (22 total)
**2D/2.5D (11):** Face, Pocket, Profile, Adaptive, VCarve, Rest, Inlay, Zigzag, Trace, Drill, Chamfer
**3D (11):** DropCutter, Adaptive3d, Waterline, Pencil, Scallop, SteepShallow, RampFinish, SpiralFinish, RadialFinish, HorizontalFinish, ProjectCurve

### Height Parameter Handling

| Concern | 2D Operations | 3D Operations |
|---------|--------------|---------------|
| Z control | Explicit `cut_depth` + `depth_per_pass` | Z derived from mesh geometry |
| Safe Z | `safe_z` (retract) | `safe_z` (retract) |
| Multi-pass | `depth_stepped_toolpath()` wrapper | Single pass at mesh-derived Z |
| Stock-to-leave | N/A (cut at exact depth) | `stock_to_leave` offset |

Operations using depth stepping: Face, Pocket, Profile, Trace, Rest, Inlay, Zigzag (via `make_depth()` / `make_depth_with_finishing()`)

### Stock Boundary Clipping
- Applied **uniformly as post-processing** in worker (`execute.rs:292-342`)
- No operation implements its own boundary clipping
- Architecturally sound — matches stated design principle

### Dressup Support Matrix

| Dressup | Applied To | Method |
|---------|-----------|--------|
| Entry (ramp/helix) | All operations | Universal post-processing in helpers.rs |
| Dogbone | All operations | Universal — detects sharp inside corners |
| Lead-in/out | All operations | Universal — adds arc approach/departure |
| Link moves | All operations | Universal — connects separated cutting runs |
| Tabs | **Profile only** | Intentional restriction (execute.rs:475-481) |
| Arc fitting | All operations | Via `simplify_path_3d()` in toolpath.rs |
| Feed optimization | All operations (gated) | Blocked for rest/remaining-stock/3D mesh ops |

Dressup application is centralized in `helpers.rs:47-200` with identical tracing boilerplate per dressup.

### Error Handling

| Category | Return Type | Operations |
|----------|------------|------------|
| 2D operations | `Result<Toolpath, String>` | Face, Pocket, Profile, VCarve, Rest, Inlay, Zigzag, Trace, Drill, Chamfer |
| 3D operations | `Result<Toolpath, ComputeError>` | Adaptive, DropCutter, Adaptive3d, Waterline, Pencil, Scallop, SteepShallow, RampFinish, SpiralFinish, RadialFinish, HorizontalFinish, ProjectCurve |

**Error messages are stringly-typed** for 2D ops: "No 2D geometry (import SVG)", "Previous tool not set", "VCarve requires V-Bit tool"

**No operations use unwrap()** — all propagate errors properly.

### Tool Type Validation

| Operation | Validates Tool Type? | Required Tool |
|-----------|---------------------|---------------|
| VCarve | Yes (execute.rs:529-532) | V-Bit |
| Inlay | Yes | V-Bit |
| Scallop | **No** | BallNose (assumed) |
| All others | No | Any |

### Output Format
- **All 22 operations produce `Toolpath`** with `Vec<Move>` (target P3 + MoveType)
- **Statistics computed uniformly** in `helpers.rs:412-430`: move_count, cutting_distance, rapid_distance
- **Semantic tracing** dispatched via `SemanticToolpathOp` trait for all 22 operations

### GUI Config
- All operations share property panel infrastructure in `properties/mod.rs`
- Common patterns: tool selector, model selector, parameter grid with `dv()` calls
- Per-operation parameter sets differ appropriately (stepover for pockets, V-angle for VCarve, etc.)

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Med | Error type mismatch: 2D ops return `String`, 3D ops return `ComputeError` | execute.rs:414-2456 |
| 2 | Med | Tool type validation only for VCarve/Inlay; Scallop doesn't check for BallNose | execute.rs:529-532 |
| 3 | Low | Tabs only supported for Profile; no other operation benefits from tab support | execute.rs:475-481 |
| 4 | Low | Height parameter semantics differ between 2D and 3D (explicit depth vs mesh-derived) | execute.rs |

## Test Gaps
- No test verifying that all 22 operations produce valid Toolpath IR
- No test for tool-type mismatch scenarios (e.g., flat endmill on Scallop)
- No consistency test for dressup application across all operations

## Suggestions

### Medium Priority
1. **Standardize error types** — Convert 2D operations to `Result<Toolpath, ComputeError>` for uniform error handling
2. **Add tool-type pre-flight validation** — Check required tool types before running operations (especially Scallop → BallNose)
3. **Document dressup support matrix** — Make explicit which dressups apply to which operations

### Low Priority
4. **Consider tab support for Pocket** — Users sometimes want workpiece retention in pocket operations
5. **Audit height handling in UI** — Ensure property panels show the right height fields per operation type (2D vs 3D)
