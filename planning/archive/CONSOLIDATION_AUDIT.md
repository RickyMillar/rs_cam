# Logic Consolidation Audit

Completed 2026-04-03. Agent team investigation across 5 domains.

## Done (this session)

1. **OperationParams trait** — replaced 10 accessor match blocks (~200 arms) in `catalog.rs` with trait delegation via `as_params()`/`as_params_mut()`. Adding a new operation now requires implementing the trait on the config struct, not updating 10 match blocks.

2. **Deleted 3 duplicate `op_feed_rate()` functions** — `preflight.rs`, `sim_timeline.rs`, `sim_diagnostics.rs` each had their own 23-arm copy. Now use `operation.feed_rate()` directly.

3. **Centralized color constants** — `render/colors.rs` module replaces 40+ hardcoded color literals across `sim_render.rs`, `toolpath_render.rs`, `grid_render.rs`, `stock_render.rs`, `height_planes.rs`.

4. **MoveType helpers** — `is_cutting()` and `feed_rate()` methods on `MoveType` reduce scattered match blocks for rapid-vs-cutting classification.

5. **Compilation/clippy fixes** — fixed test compilation errors and ~30 clippy violations left by the tool geometry session.

## Done (session 2, 2026-04-05)

6. **HTML/Three.js scaffold refactor** — extracted 7 shared helper functions (`html_head`, `html_importmap`, `html_scene_setup`, `html_toolpath_objects`, `html_grid_axes`, `html_tail`, `serialize_toolpath_lines`) from `viz.rs`. The first two HTML generators now use the shared scaffold. Simulation generator (1000+ lines with animation engine) left as-is.

7. **Feeds hardcodes moved to Material** — `base_cutting_speed_m_min()` and `plunge_rate_base()` methods added to `Material`, replacing hardcoded match blocks in `feeds/mod.rs`. Magic numbers replaced with named constants: `FLUTE_GUARD_FACTOR`, `MIN_AP_MM`, `MIN_AE_MM`, `SLOTTING_THRESHOLD`, `SLOTTING_DOC_CAP`, `LD_SEVERE_THRESHOLD`, `LD_MODERATE_THRESHOLD`, `WORKHOLDING_LOW_FACTOR`, `WORKHOLDING_HIGH_FACTOR`, `FALLBACK_RPM`.

## Remaining (future sessions)

### ~~Tier 3 (depends on ToolDefinition stabilizing)~~ — DONE (2026-04-05)

8. **CLI unified to use ToolDefinition** — `build_tool()` in CLI now returns `ToolDefinition` instead of `Box<dyn MillingCutter>`. CLI tool construction uses assembly fields (shank, holder, stickout) from TOML when available. `OpResult.cutter` changed from `Box<dyn MillingCutter>` to `ToolDefinition`. Dead code warnings on assembly fields removed.

9. **Tool shape rendering consolidation** — Not needed. The UI preview already uses `profile_points()` from ToolDefinition (done in tool geometry session). The GPU wireframe's per-shape code produces superior wireframes at low vertex counts and is not duplicated with the UI code.

10. **Move viz.rs out of core** — Deferred. Low priority since no functional impact.

### ~~Tier 4 (lower priority)~~ — Assessed (2026-04-05)

11. **Project file round-trips simplified** — `into_runtime()` now constructs `ToolConfig` directly instead of creating a default and overwriting all fields. The compiler enforces completeness (missing fields = compile error).

12. **`new_default()` match** — Investigated, already clean. Each arm is one line calling `.default()`. Compiler-enforced exhaustiveness. No simplification needed.

13. **Test builder helpers** — Investigated, not needed. Tests already use `ToolConfig::new_default()` and `OperationConfig::new_default()` factory methods, not fragile struct literals.

## Key metrics

| Before | After | Change |
|--------|-------|--------|
| 3 duplicate op_feed_rate() functions (69 arms) | 0 | -69 match arms |
| 10 accessor match blocks in catalog.rs (~200 arms) | 2 dispatch methods + trait impls | -200 match arms, +trait |
| 40+ hardcoded color literals | Centralized in colors.rs | Single source of truth |
| 0 MoveType helper methods | 2 (is_cutting, feed_rate) | Reduces future match blocks |
| 2 HTML generators with inline boilerplate | Shared scaffold helpers | ~150 lines of duplication eliminated |
| 12+ magic numbers in feeds/mod.rs | Named constants + Material methods | Self-documenting, single source of truth |
| CLI build_tool() returns Box&lt;dyn MillingCutter&gt; | Returns ToolDefinition | CLI gains assembly info, matches viz pattern |
