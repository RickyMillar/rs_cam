# Review: Feed Optimization

## Summary
Feed optimization is a post-processing dressup that adjusts cutting feed rates based on material engagement estimated from a heightmap simulation. The implementation is algorithmically sound (RCTF-based chip thinning compensation) with no critical bugs, but has intentional limitations to fresh-stock, flat-stock 2.5D workflows. The feature is opt-in in the GUI, not available in the CLI.

## Findings

### Algorithm
- **Heightmap construction** (`compute/worker/helpers.rs:306-320`): initialized flat at `bbox.max.z` (stock top surface), cell size = `tool_diameter / 4.0` clamped to `[0.25, 2.0]` mm
- **Engagement estimation** (`feedopt.rs:49-85`): samples 24 points evenly around tool circumference, looks up each point's Z in heightmap, counts how many are above material (threshold 0.01 mm). Engagement fraction = engaged_count / n_samples
- **Feed scaling via RCTF** (`feedopt.rs:34-43, 116-123`): if engagement < 0.05 (air cut), uses max_feed_rate; otherwise applies Radial Chip Thinning Factor from `feeds/geometry.rs:17-31`. Formula: `1 / sqrt(1 - (1 - 2*ae_ratio)^2)`, clamped to [1.0, 4.0]
- **Material removal simulation** (`feedopt.rs:127-128`): after computing engagement for move i, stamps tool into heightmap via `stamp_tool_at()` (`simulation.rs:179-222`) so subsequent moves see updated stock
- **Feed smoothing** (`feedopt.rs:158-184`): two-pass (forward + backward) ramp-rate limiter prevents abrupt feed changes. Skips rapids gracefully

### Limitations (By Design)
- **Fresh stock only**: rejects `StockSource::FromRemainingStock` (`catalog.rs:721-724`). Heightmap is initialized flat; multi-setup remaining-stock workflows have complex prior geometry not modeled. Fixable by loading prior dexel stock state
- **Flat stock only**: disabled for all 3D operations (dropcutter, adaptive3d, waterline, pencil, scallop, steep/shallow, ramp_finish, spiral_finish, radial_finish, horizontal_finish, project_curve) (`catalog.rs:731-734`). Engagement estimation assumes planar Z=stock_top initially — wrong for curved surfaces. Would need full 6-DOF tool profile interaction with mesh
- **Rest machining disabled** (`catalog.rs:726-729`): rest ops inherit prior geometry; heightmap state unknown
- **Supported ops** (2.5D only): face, pocket, profile, adaptive, vcarve, inlay, zigzag, trace, drill, chamfer

### Non-Fundamental Limitations
- **Discrete sampling**: 24 circumferential points may miss fine geometry on small overhangs/thin walls
- **Radial engagement only**: RCTF accounts for ae (radial) but ignores axial engagement (depth of cut)
- **Circular tool assumption**: engagement uses `tool_radius * angle.cos/sin` directly — approximation for V-bits and tapered tools (`feedopt.rs:62-63`)
- **One-way heightmap stamping**: `stamp_tool_at()` only lowers cells, doesn't model material recovery on retracts (`simulation.rs:216-221`)

### Integration
- **GUI** (`ui/properties/mod.rs:2608-2627`): opt-in checkbox "Feed rate optimization", disabled with hover-text when unavailable. Exposes `feed_max_rate` and `feed_ramp_rate` sliders
- **Dressup pipeline** (`compute/worker/helpers.rs:228-278`): applied after entry, dogbones, lead-in/out, link moves, arc fitting, before rapid ordering. If heightmap fails, warning logged and optimization skipped gracefully
- **CLI**: not exposed. `optimize_feed_rates()` is public in core but not called by rs_cam_cli

### Testing
- **7 tests** in `feedopt.rs:193-343`:
  - 4 RCTF formula validation tests
  - Air cut → max feed test
  - Full engagement → nominal test
  - Ramp rate smoothing test
- **No unwrap()** in production code
- **Graceful degradation**: errors don't crash; warnings logged

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Low | Nominal feed defaults to 1000 mm/min if toolpath has no cutting moves (only rapids) — could scale incorrectly | `helpers.rs:245-252` |
| 2 | Low | Heightmap cell lookup uses nearest-cell rounding instead of bilinear interpolation — coarse for small tools | `feedopt.rs:66-67` |
| 3 | Low | Air cut threshold (5%) hardcoded, not exposed as parameter — feed jumps to max below this | `feedopt.rs:31`, `helpers.rs:259` |
| 4 | Med | `feed_ramp_rate` parameter name/units confusing — actual unit is mm/min per mm of travel, user may think it's time-based | GUI tooltip |

## Test Gaps
- No test for engagement estimation with partial material (e.g., 50% engaged)
- No test for heightmap stamping correctness
- No integration test with realistic toolpath and feeds/speeds
- No test verifying behavior when toolpath has only rapids (nominal feed fallback)

## Suggestions
- Document `feed_ramp_rate` units clearly in UI tooltip ("mm/min per mm of travel")
- Consider accepting `nominal_feed_rate` as an explicit parameter instead of inferring from first cutting move
- Add integration tests with realistic wood-routing toolpaths
- Phase 2: support remaining-stock workflows by loading prior dexel state
- Phase 3: extend to 3D operations via mesh-aware engagement estimation
- Add CLI support via job config extension
