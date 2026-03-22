# Review: Duplication & Abstraction Opportunities

## Summary
Most duplication lives in the GUI layer (rs_cam_viz), not across crate boundaries. The top hotspots are dressup tracing boilerplate (~360 LOC), operation dispatch match arms (~200 LOC), and repeated UI parameter grid patterns (~120 LOC). The core/CLI/GUI separation is well respected — the duplication that exists is within-layer repetition of scaffolding patterns.

## Findings

### 1. Dressup Tracing Boilerplate (~360 LOC)
- **Location:** `crates/rs_cam_viz/src/compute/worker/helpers.rs:57-303`
- Every dressup (ramp, helix, dogbone, lead-in/out, link moves, arc fit, feed opt, rapid order) repeats identical debug/semantic tracing setup: `start_span` → `start_item` → `set_debug_span_id` → `set_param` → apply → `bind_to_toolpath` → `set_move_range`
- ~45-50 LOC per dressup × 8 dressups = ~360 LOC of identical scaffolding
- **Fix:** Extract `apply_dressup_with_tracing(name, kind, debug, semantic, |tp| ...)` helper

### 2. Operation Dispatch Match Arms (~200 LOC)
- **Location:** `crates/rs_cam_viz/src/compute/worker/execute.rs:23-55` (semantic_op), `crates/rs_cam_viz/src/ui/properties/mod.rs:804-827` (draw_toolpath_panel)
- Three separate match expressions must be updated for every new operation: semantic_op dispatch (22 arms), UI parameter drawing (22 arms), tool dispatch in build_cutter (5 arms)
- Each arm is a simple cast/delegation with no logic
- **Fix:** Macro-generated dispatch or trait-based registry

### 3. Feed Parameter UI Pattern (~120 LOC)
- **Location:** `crates/rs_cam_viz/src/ui/properties/mod.rs` across 15+ operation editors
- Nearly identical "Feed Rate + Plunge Rate + Climb" blocks repeated in draw_pocket_params, draw_profile_params, draw_adaptive_params, draw_inlay_params, etc.
- **Fix:** Extract `draw_feed_params(ui, cfg)` helper

### 4. SemanticToolpathOp Tracing Setup (~440 LOC)
- **Location:** `crates/rs_cam_viz/src/compute/worker/execute.rs:1521-2452`
- Each of 22 operations repeats ~20 LOC of identical scope/tracing infrastructure in `generate_with_tracing()`
- **Note:** Fixing this requires rethinking the operation trait interface — high cost, low ROI

### 5. Import Path Handlers (~90 LOC)
- **Location:** `crates/rs_cam_viz/src/io/import.rs:11-101`
- Three nearly identical functions: `import_stl()`, `import_svg()`, `import_dxf()` with same structure (load → name → build LoadedModel)
- **Fix:** Generic `load_model(path, kind, loader_fn)` wrapper

### 6. Operation UI Parameter Grids
- **Location:** `crates/rs_cam_viz/src/ui/properties/mod.rs:1059-2400`
- ~41 `dv()` calls across 23 operation editors, each with identical Grid setup pattern
- Could benefit from a declarative macro but current approach is readable

### 7. Depth Stepping Iteration (~70 LOC)
- **Location:** `crates/rs_cam_viz/src/compute/worker/execute.rs` across ~6 operations
- Operations doing depth-stepped cutting repeat the same `make_depth` → `for z in levels` → extend pattern (~12 LOC each)
- **Fix:** Helper `for_each_depth_level(cfg, |z| -> Toolpath)`

### 8. Tool Type Dispatch (20 LOC — already good)
- **Location:** `crates/rs_cam_viz/src/compute/worker/helpers.rs:7-28`
- Single match with 5 arms, each a constructor call. Idiomatic Rust, no change needed.

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Med | Dressup tracing boilerplate: 8 copies of identical 45-line scaffolding | helpers.rs:57-303 |
| 2 | Med | Operation dispatch requires updating 3 match statements for each new op | execute.rs:23-55, properties/mod.rs:804-827 |
| 3 | Low | Feed/plunge/climb UI pattern duplicated across 15+ operation editors | properties/mod.rs (multiple) |
| 4 | Low | Import handlers structurally identical across 3 formats | import.rs:11-101 |
| 5 | Low | Depth stepping iteration repeated in 6+ operations | execute.rs (multiple) |

## Test Gaps
- N/A (duplication review, not test review)

## Suggestions

### High priority (best ROI)
1. **Extract `apply_dressup_with_tracing()` helper** in helpers.rs — low risk, isolated to one file, ~320 LOC reduction
2. **Macro-generate operation dispatch** — prevents errors when adding operations, ~200 LOC reduction

### Medium priority
3. **Extract `draw_feed_params()` UI helper** — ~120 LOC reduction across property editors
4. **Generic `load_model()` import wrapper** — ~60 LOC reduction, centralizes error handling
5. **Depth stepping helper** — ~60 LOC reduction, cleaner operation code

### Leave as-is
- Tool type dispatch (already idiomatic)
- SemanticToolpathOp tracing setup (too architectural to refactor for modest gains)
- Import error wrapping (already concise)
