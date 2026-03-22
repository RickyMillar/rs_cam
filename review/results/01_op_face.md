# Review: Face Operation

## Summary

The Face Operation is a simple facing/leveling operation for stock surfaces using a zigzag raster pattern. The core algorithm correctly generates multi-level depth stepping with proper offset handling. However, there is a high-severity integration issue: the `FaceDirection` parameter (OneWay vs Zigzag) is defined in the config and exposed in the UI but is completely ignored during toolpath generation. A dead `run_face` function also exists alongside the newer semantic implementation.

## Findings

### Correctness

- **Depth stepping**: `face.rs:59-94` correctly branches on `depth <= 0.0` for single-pass vs multi-pass. Uses `DepthStepping` with `DepthDistribution::Even`. Tests verify correct Z levels: `face.rs:184-192` — 6mm at 2mm/pass produces 3 levels at Z=-2, -4, -6.
- **Rectangle construction**: `face.rs:52-57` correctly applies `stock_offset` on all sides.
- **Zigzag safety**: `zigzag.rs:59-62` returns empty lines if inset produces empty polygon (tool larger than stock). `zigzag.rs:101-103, 145` gracefully handles degenerate cases.
- **Heights**: `execute.rs:1592` uses `effective_safe_z(ctx.req)` → `req.heights.retract_z`. Line toolpath correctly applies plunge_rate and feed_rate at `execute.rs:1589-1596`.
- **Tool type support**: Uses `tool.diameter / 2.0` only (line 1571) — works with all 5 tool types.
- **Edge cases**: Zero depth tested at `face.rs:233-255` (single-pass at Z=0). Stock wider than tool works via offset + inset. Tool larger than stock handled by empty inset check.

### Integration

- **FaceDirection ignored (HIGH)**: `configs.rs:50-53` defines `FaceDirection` enum (OneWay, Zigzag). `configs.rs:88-96` includes it in `FaceConfig`. `mod.rs:2006-2015` has a fully functional UI dropdown. But `execute.rs:1521-1608` semantic `generate_with_tracing` implementation **never uses `self.direction`**. Line 1569: `zigzag_lines()` called with hard-coded `0.0` angle, no direction parameter. Selecting "One Way" vs "Zigzag" in GUI has zero effect.
- **Dead code**: `execute.rs:968-989` has `#[allow(dead_code)]` `run_face()` function that is never called. The semantic trait implementation supersedes it.
- **CLI not exposed**: Face operation is NOT in CLI (`job.rs:310-448` shows only pocket, profile, adaptive, rest). GUI-only.
- **Parameter wiring**: stepover, depth, depth_per_pass, stock_offset, feed_rate, plunge_rate all correctly wired. Only `direction` is broken.

### Code Quality

- **Error handling**: `execute.rs:1526-1528` properly errors if stock_bbox undefined. Core `face.rs` has no unwrap() in production code (only in tests). Zigzag returns empty Vec on degenerate cases.
- **Duplication**: Pocket uses similar depth-stepped zigzag pattern but supports both contour and zigzag via dispatch. Face is hardcoded to zigzag, which is correct for facing.
- **Documentation**: `face.rs:1-5` has clear module-level docs. `face.rs:45-49` explains rectangle + zigzag approach.
- **Semantic tracing**: Properly uses tracing infrastructure with Operation → DepthLevel → Row hierarchy.

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | FaceDirection parameter (OneWay/Zigzag) defined in config and exposed in UI but completely ignored in toolpath generation — all facing produces zigzag regardless of selection | `execute.rs:1521-1608` ignores `self.direction` |
| 2 | Medium | Dead code: `run_face()` marked `#[allow(dead_code)]` exists but is never called; semantic trait impl supersedes it | `execute.rs:968-989` |
| 3 | Low | Face operation not exposed in CLI, only available through GUI | `job.rs:310-448` — no "face" branch |
| 4 | Low | No validation that stepover is reasonable relative to tool diameter | Core logic accepts any stepover > 0 |

## Test Gaps

- No test for FaceDirection::OneWay behavior (no test verifies one-way passes are generated)
- No test for tool radius >= stock area (inset produces empty)
- No test for stock_offset with zero or negative depth
- No test for very large stock_offset relative to stock size
- No integration test: GUI config → compute → toolpath verification

## Suggestions

1. **Implement FaceDirection in semantic impl** (High): Wire `self.direction` into `zigzag_lines()` call — use perpendicular angle or separate one-way pass logic for OneWay mode
2. **Remove dead `run_face()`** (Medium): Delete `execute.rs:968-989` if fully superseded by semantic impl
3. **Document CLI limitation** (Low): Add note to FEATURE_CATALOG if Face is GUI-only, or add "face" branch to `job.rs` dispatch
4. **Add stepover validation** (Low): Warn if stepover > tool_diameter * 0.95 or < 0.25mm
5. **Add direction test**: Once implemented, test that OneWay produces unidirectional passes
