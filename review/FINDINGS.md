# Consolidated Review Findings

Collated from 48 individual code reviews of the rs_cam codebase (~56K LOC, 3 crates).
Categorized by implementation area to support writing implementation plans.

---

## A. Confirmed Bugs (Wrong Output / Data Corruption)

Issues where the software produces incorrect results today.

| # | Severity | Description | Source | Location |
|---|----------|-------------|--------|----------|
| A1 | **Critical** | CLI tool radius mismatch: job.rs passes `diameter/2.0` to Adaptive3dParams while main.rs passes `cutter.radius()`. Job file Adaptive3d operations may compute at wrong tool radius. | R39 | job.rs:294,550 vs main.rs:2221 |
| A2 | **Critical** | Rest machining GUI double-converts scan angle: `cfg.angle.to_radians()` passed to `RestParams::angle` (documented as degrees), which then calls `zigzag_lines(angle_deg)` (converts internally). 45deg becomes ~0.785deg. | R06 | execute.rs:570 |
| A3 | **High** | Tabs applied to ALL depth passes, not just final pass. With depth_per_pass < depth, tabs appear on roughing passes too. | R03 | execute.rs:475-480, dressup.rs:229 |
| A4 | **High** | FaceDirection parameter (OneWay vs Zigzag) completely ignored in toolpath generation. All facing produces zigzag regardless of UI selection. | R01 | execute.rs:1521-1608 |
| A5 | **High** | Inlay male region construction ignores polygon holes. Only uses `polygon.exterior`, not `polygon.holes`. Designs with islands (letter "O") produce incorrect male ridges. | R07 | inlay.rs:131 |
| A6 | **Medium** | VCarve max_depth=0.0: comment says "uses full cone depth" but `min(anything, 0.0)` always returns 0.0 — actually clamps all depths to zero. | R05 | vcarve.rs:23, 108 |
| A7 | **Medium** | Potential NaN in VBit edge_drop: `ccu_sq.sqrt()` without `.max(0.0)` guard. Negative floating-point rounding produces NaN. | R15 | vbit.rs:232 |
| A8 | **Medium** | Potential NaN in TaperedBall edge_drop: same unsafeguarded sqrt as VBit. | R15 | tapered_ball.rs:322 |
| A9 | **Medium** | Bounding box not recomputed after mesh winding fix — downstream consumers may use stale bounds. | R18 | mesh.rs |
| A10 | **Medium** | GUI merges inlay female + male moves into single Toolpath. Users cannot separate male/female in GUI output (CLI correctly produces two files). | R07 | execute.rs:603-604 |

### Quick Fixes (< 30 min each)
- **A2**: Remove `.to_radians()` from execute.rs:570
- **A7/A8**: Add `.max(0.0)` before `.sqrt()` in VBit and TaperedBall edge_drop
- **A6**: Fix comment or implement "unlimited depth" semantics for max_depth=0.0

---

## B. Unwired / Dead Features

Code that exists but isn't connected end-to-end, or features with no effect.

| # | Severity | Description | Source | Location |
|---|----------|-------------|--------|----------|
| B1 | **High** | Pre/post G-code injection: editable text fields in UI, stored in project JSON, but export pipeline never reads or emits them. User's custom M-codes silently dropped. | R42 | entry.rs:28-29, export.rs |
| B2 | **High** | Undo only records StockChange. Tool/Post/Machine/ToolpathParam undo handlers exist as dead code — UndoAction enum has 5 types but only 1 is ever pushed. | R29 | history.rs, properties/mod.rs:31-54, events.rs:1054-1130 |
| B3 | **Medium** | `auto_regen` field exists in ToolpathEntry; `process_auto_regen()` has stale_since logic, but nothing ever triggers auto-regeneration. Field always true for 2D ops, false for 3D. | R37 | entry.rs:135, controller.rs:116-137 |
| B4 | **Medium** | G41/G42 cutter compensation: Profile operation UI exposes InControl mode and logs selection, but G-code never emits G41/G42 commands. | R42 | FEATURE_CATALOG.md:111 |
| B5 | **Medium** | Deviation coloring: `deviation_colors()` helper exists, `StockVizMode::Deviation` variant exists, but `display_deviations` set to `None` on fresh sim results — deviation data never computed from model surface. | R38 | app.rs:417-429, events.rs:949 |
| B6 | **Medium** | `ToggleSimToolpath` and `RecalculateFeeds` events have empty match arms — handlers never implemented. | R32 | events.rs:346, 427 |
| B7 | **Medium** | 3D adaptive parameters not exposed in GUI: `entry_style`, `detect_flat_areas`, `region_ordering`, `fine_stepdown`, `min_cutting_radius`, `stock_to_leave_radial`. | R04 | properties/mod.rs, configs.rs |
| B8 | **Medium** | `finishing_passes` field exists in PocketConfig but not exposed in GUI properties panel. | R02 | configs.rs:187 |
| B9 | **Low** | Workholding rigidity: backend supports Low/Medium/High in feed calculations, but UI hardcodes Medium. No ComboBox exposed. | R42 | properties/mod.rs:613 |
| B10 | **Low** | `StockVizMode::ByOperation` placeholder returns uniform wood-tone color instead of per-operation cell tracking. | R38 | sim_render.rs:124-127 |
| B11 | **Low** | Dead `run_face()` function marked `#[allow(dead_code)]` alongside newer semantic implementation. | R01 | execute.rs:968-989 |
| B12 | **Low** | Dead `geo` conversion code (`to_geo_polygon()`/`from_geo_polygon()`) tested but never called in production. | R25 | polygon.rs:79-90 |
| B13 | **Low** | Waterline `continuous` flag stored but never used. | R11 | waterline.rs |
| B14 | **Low** | Scallop `stock_to_leave_radial` field defined but never used. | R12 | scallop configs |
| B15 | **Low** | CLI setup definitions have unused `face_up`/`z_rotation` fields marked dead_code. | R39 | job.rs:68-73 |
| B16 | **Low** | `NoPaths` (SVG) and `NoEntities` (DXF) error variants defined but never raised. | R19 | svg_input.rs, dxf_input.rs |

### Grouping for Implementation
- **B1**: Wire pre/post G-code into `emit_gcode_phased()` — add optional string fields, emit between preamble/postamble
- **B2**: Extend undo to all 5 types by adding snapshot+push patterns matching the stock pattern
- **B3**: Wire auto_regen — set `stale_since` on property edit, process in `process_auto_regen()`
- **B7**: Add parameter UI fields in properties/mod.rs for missing 3D adaptive params
- **B9**: Trivial — add one ComboBox for workholding rigidity

---

## C. Error Handling & Robustness

Safety issues: panics, missing validation, error propagation.

| # | Severity | Description | Source | Location |
|---|----------|-------------|--------|----------|
| C1 | **High** | 164 `unwrap()` calls in core library (CLAUDE.md says "library code should avoid unwrap()"). Concentrated: dexel_stock.rs (52), simulation.rs (19), adaptive.rs (8). Most in `#[cfg(test)]` but some in library paths. | R48 | crates/rs_cam_core/ |
| C2 | **High** | No `catch_unwind()` on worker threads — a panic kills the lane permanently, poisons all mutexes. | R30 | worker.rs:428,487 |
| C3 | **High** | 10+ `.expect("lane mutex poisoned")` — cascade crash after any thread panic. | R30 | worker.rs:213,255,319,353+ |
| C4 | **High** | No bounds validation on triangle indices after STL parsing — potential panic on malformed mesh. | R18 | mesh.rs |
| C5 | **High** | Tool deletion leaves orphaned `tool_id` references in toolpath entries. G-code generation will fail. No validation or cascading removal. | R36 | events.rs:75-80 |
| C6 | **Medium** | No divide-by-zero guards on `cell_size` / `segment_length` divisions in hot paths. | R48 | dexel.rs, simulation.rs |
| C7 | **Medium** | Inconsistent error types: 2D ops return `Result<Toolpath, String>`, 3D ops return `Result<Toolpath, ComputeError>`. | R46,R48 | execute.rs |
| C8 | **Medium** | Only ~4 NaN checks in entire core — minimal explicit NaN/Inf guard. Geometry calculations assume valid floating-point. | R48 | Various |
| C9 | **Medium** | `.last().unwrap()` / `.first().unwrap()` on potentially empty result vectors. | R48 | simulation.rs:761,773 |
| C10 | **Medium** | No atomic file writes for project save — crash during write corrupts project file. | R33 | project.rs:391 |
| C11 | **Medium** | `ResetSimulation` doesn't cancel in-flight compute — stale results can overwrite reset state. | R32 | events.rs:337-345 |
| C12 | **Medium** | No backpressure on toolpath compute queue — unlimited growth possible. | R30 | worker.rs:314-336 |
| C13 | **Medium** | Dressup errors silently swallowed (except feed optimization warning). | R30 | helpers.rs:47-304 |
| C14 | **Medium** | No tool-type pre-validation for Scallop (assumes BallNose) — error only at runtime. | R46 | execute.rs |
| C15 | **Medium** | Silent polygon hole re-pairing fallback: if containment test fails, hole attaches to first polygon without warning. | R25 | polygon.rs:192-194 |
| C16 | **Medium** | `unwrap_or(ToolId(0))` / `unwrap_or(ModelId(0))` silently defaults to nonexistent IDs when adding toolpath. | R32 | events.rs:207,214 |
| C17 | **Medium** | Unwrap() in waterline library chaining code — not in test block. | R11 | waterline.rs:223,252 |
| C18 | **Medium** | `result_tx.send()` failures silently dropped in worker thread. | R30 | worker.rs:475+ |
| C19 | **Medium** | Preset TOML parsing uses manual string slicing — fragile to whitespace/comment changes. | R33 | presets.rs:107-140 |
| C20 | **Low** | Profile assumes closed polygon for tab perimeter calculation — open contours produce incorrect tab positions. | R03 | profile.rs:97-101 |
| C21 | **Low** | CLI silently overwrites existing output files without warning or `--force` flag. | R39 | main.rs:1261 |
| C22 | **Low** | `indices.len() as u32` could panic for meshes with >2^32 indices. | R18 | mesh rendering |
| C23 | **Low** | Stale selection persists when referenced item's parent is deleted — only clears exact match, no cascade. | R29 | events.rs:76-100 |
| C24 | **Low** | 3 `todo!()` stubs remaining in core. | R48 | simulation_cut, dropcutter, contour_extract |

### Grouping for Implementation
- **Thread safety (C2, C3)**: Add `catch_unwind()` wrapper, replace `.expect()` with graceful degradation
- **Core unwrap audit (C1, C9, C17)**: Systematic unwrap → proper error handling pass
- **Validation at boundaries (C4, C5, C6, C8, C14, C16)**: Add input validation layer
- **Error type unification (C7)**: Define unified `CamError` enum, convert 2D ops from String
- **File I/O safety (C10, C19)**: Atomic writes via temp+rename, use `toml` crate for presets

---

## D. Performance & Parallelism

Optimization opportunities and resource waste.

| # | Severity | Description | Source | Location |
|---|----------|-------------|--------|----------|
| D1 | **High** | Line rendering always 1-pixel width — dense toolpaths become illegible. | R31 | LINE_SHADER_SRC |
| D2 | **High** | Mesh vertex duplication: 3x memory overhead per triangle. No index reuse in GPU upload. | R31 | mesh_render.rs:45-70 |
| D3 | **High** | Simulation mesh colors recomputed + re-uploaded every frame even when geometry unchanged. | R31 | sim_render.rs:62-84 |
| D4 | **Medium** | Only 1 of ~4 parallelizable hot paths uses rayon (waterline only). Dropcutter batch grid: 4-8x speedup potential. Adaptive material grid: 2-4x. Pocket offset layers: 2-4x. | R43 | waterline.rs:72 |
| D5 | **Medium** | Spatial index deduplication allocates `Vec<bool>` per query. 100k-triangle mesh: 100KB per point. Batch 200 points: 20MB total. Fix: bitset for 8x reduction. | R43 | mesh.rs:464 |
| D6 | **Medium** | `kiddo` crate in Cargo.toml but appears unused. | R18 | rs_cam_core/Cargo.toml |
| D7 | **Medium** | No frustum culling in render pipeline. | R31 | render/ |
| D8 | **Low** | Arc data lost through offset pipeline despite using arc-preserving library (`cavalier_contours`). | R25 | polygon.rs:103-108 |
| D9 | **Low** | Segment chaining O(n^2) worst case in contour extraction. | R27 | contour_extract.rs:310-325 |
| D10 | **Low** | TSP 2-opt O(100n^2) worst case — undocumented; could be slow for hundreds of segments. | R26 | tsp.rs:133-167 |
| D11 | **Low** | `Vec::remove(0)` for undo stack overflow is O(n) — could use VecDeque. | R29 | history.rs:54 |
| D12 | **Low** | Tool wireframe generation verbose (~400 lines), no LOD. | R31 | sim_render.rs:267-633 |

### Highest-ROI Fixes
1. **D4 (dropcutter parallelism)**: ~1-2 hours, 4-8x speedup on largest bottleneck
2. **D5 (bitset dedup)**: ~30 min, 8x memory reduction on spatial queries
3. **D3 (sim mesh caching)**: Cache color variants in GPU, switch via uniform
4. **D2 (indexed rendering)**: ~2-3 hours, halves VRAM for large STL

---

## E. Testing Gaps

Areas with insufficient test coverage, organized by priority.

### Critical Gaps (0 or near-0 tests)

| Area | Tests | Impact | Source |
|------|-------|--------|--------|
| CLI crate | 0 | Entire batch processing untested | R40 |
| Interaction/picking | 0 | All 3D picking, face detection, priority ordering untested | R34 |
| Undo/redo system | 0 | History push/pop/invalidation untested | R29 |
| Controller CRUD events | 0 isolated | Add/remove/rename for all entity types untested | R32 |
| Dropcutter | 3 | Core surface generation algorithm barely tested | R40 |
| simulation_cut | 2 | Simulation analytics barely tested | R40 |
| Face milling | 4 | Core 2.5D operation undertested | R40 |
| FlatEndmill | 4 | Most common tool type severely undertested | R15 |
| GPU rendering | 0 | No headless GPU tests, no visual regression | R31 |

### Systematic Gaps (apply across codebase)

| Gap Type | Description | Source |
|----------|-------------|--------|
| Property-based testing | No proptest/quickcheck for geometric invariants | R40 |
| Fuzz testing | No cargo-fuzz for file parsers (STL/SVG/DXF) | R40 |
| End-to-end integration | Only 2 tests (both dropcutter-based) | R40 |
| Multi-operation sequencing | No test for operation ordering + dependencies | R40 |
| Cross-setup simulation | No test for multi-setup carry-forward | R38 |
| CLI integration | No test running demo_job.toml as CI step | R39 |
| Export validation | G-code syntax not validated in tests | R33 |
| Cancellation behavior | Minimal cancellation testing across operations | R30 |
| Degenerate geometry | No malformed mesh, self-intersecting polygon, NaN input tests | R48 |

### Per-Module Gaps (selected high-value)

| Module | Missing Tests | Source |
|--------|--------------|--------|
| Profile | Multi-pass + tabs, tab on holes, dogbone obtuse corners | R03 |
| Adaptive | Narrow slots, multi-island pockets, parameter validation | R04 |
| VCarve | max_depth=0, depth > cone height, thin features | R05 |
| Inlay | Male region with holes, complementarity, sharp corners | R07 |
| Waterline | Saddle cases, complex terrain, island/nesting | R11 |
| Pencil/Scallop | Inconsistent normals, continuous spiral, slope+curvature | R12 |
| Simulation | Bull/VBit/TaperedBall stamping, volume removal, side-grid | R16 |
| Dressups | Lead-in/out (only 2 tests), composition, tabs on first move | R21 |
| Collision | Tapered holder, multi-segment, performance | R24 |
| State management | Selection cascade, orphan detection, concurrent undo+compute | R29 |
| Compute worker | 18 of 22 operation types untested | R30 |
| Coordinate transforms | 24 orientation combos (6 faces x 4 rotations) not validated | R36 |

---

## F. Code Quality & Maintainability

Large files, duplication, and structural issues that slow development.

### Oversized Files (split candidates)

| File | Lines | Issue | Source |
|------|-------|-------|--------|
| `compute/worker/execute.rs` | 2492 | 22 operation impls + dispatch + helpers in one file | R30 |
| `ui/properties/mod.rs` | 2674 | 20+ operation param functions + dispatcher | R28 |
| `ui/sim_timeline.rs` | 1246 | Timeline rendering + playback + annotations | R28 |
| `controller/events.rs` | 1267 | 421-line monolithic `handle_internal_event` match | R32 |
| `adaptive.rs` | 2383 | Core algorithm — large but cohesive | R04 |
| `adaptive3d.rs` | 3556 | 3D roughing — large but cohesive | R10 |

### Duplication Hotspots

| Pattern | LOC | Copies | Fix | Source |
|---------|-----|--------|-----|--------|
| Dressup tracing boilerplate | 360 | 8 | Extract `apply_dressup_with_tracing()` | R41 |
| Operation dispatch match arms | 200 | 3 | Macro-generated dispatch or trait registry | R41 |
| Feed/plunge/climb UI pattern | 120 | 15+ | Extract `draw_feed_params(ui, cfg)` | R41 |
| Import handlers (STL/SVG/DXF) | 90 | 3 | Generic `load_model()` wrapper | R41 |
| Depth stepping iteration | 70 | 6 | Helper `for_each_depth_level()` | R41 |
| `run_simulation_with_all/ids` | ~200 | 2 | Extract `build_simulation_groups()` | R32 |
| Annotation boilerplate per operation | ~440 | 22 | Semantic tracing macro or helper | R30 |
| Parameter extraction + error wrapping | ~120 | 15+ | Extract parameter builder | R30 |

### Inconsistencies

| Area | Issue | Source |
|------|-------|--------|
| CLI parameter naming | `entry` vs `entry_style` for same concept across 2D/3D | R39 |
| Event emission | stock.rs batches changes, setup.rs emits per-field | R28 |
| State mutation | Controller vs direct UI mutation (properties bypass controller) | R29 |
| Error types | `String` (2D ops) vs `ComputeError` (3D ops) vs `thiserror` ADTs (imports) | R46,R48 |
| Epsilon values | 1e-6, 1e-8, 1e-10, 1e-12, 1e-15, 1e-20 used inconsistently | R09 |
| Magic numbers | 170.0 dogbone angle, 40.0 holder length, 12.0/15.0 pick threshold scattered | R39,R34,R28 |

---

## G. UI/UX Issues

User-facing interaction, usability, and visual concerns.

### Functional Issues

| # | Severity | Description | Source | Location |
|---|----------|-------------|--------|----------|
| G1 | **High** | UI panels directly mutate state, bypassing controller — no undo, no mark_edited(), no event trail. | R29 | properties/mod.rs:36,127,452 |
| G2 | **Medium** | No model deletion UI — models can only be hidden via toolpath visibility, no remove option. | R35 | project_tree.rs |
| G3 | **Medium** | No re-import/update workflow — modifying source file requires delete+re-import (which is impossible per G2). | R35 | No controller method |
| G4 | **Medium** | SVG/DXF imports don't set `pending_upload = true` — GPU mesh not refreshed; no camera fit for 2D models. | R35 | controller/io.rs:29-45 |
| G5 | **Medium** | Toolpath picking undersamples large toolpaths (200 move max) — thin features missed. | R34 | picking.rs:220 |
| G6 | **Medium** | Scroll zoom direction inverts across platforms — no normalization. | R34 | camera.rs:100-102 |
| G7 | **Medium** | Escape in Simulation fires even when focus is in text field. | R34 | app.rs:1746 |
| G8 | **Medium** | Last setup deletion: button appears clickable but is silently ignored if only 1 setup. No feedback. | R36 | project_tree.rs:155-158 |
| G9 | **Medium** | Validation rules fragmented: UI validates VCarve/Inlay/Chamfer tool type, but geometry existence checked only at generation time. | R37 | properties/mod.rs:2408-2481 vs events.rs:786-799 |
| G10 | **Medium** | Rest machining validation checks prev_tool_id is set but NOT that the prior-tool operation exists earlier in the list. | R37 | properties/mod.rs:2462-2476 |

### Polish Issues

| # | Severity | Description | Source |
|---|----------|-------------|--------|
| G11 | Medium | Automation coverage ~2% — effectively non-functional for deterministic UI testing. | R28 |
| G12 | Medium | UI abbreviations lack tooltips: "Col", "Fix", "TP", "AN". | R28 |
| G13 | Medium | Magic spacing numbers (2.0, 4.0, 6.0, 8.0, 12.0) scattered with no constants. | R28 |
| G14 | Low | No staleness indicator in Simulation workspace outside preflight modal. | R38 |
| G15 | Low | No smooth transitions for camera preset views (instant snap). | R34 |
| G16 | Low | Operation defaults don't adapt to context (stock thickness, tool diameter). | R37 |
| G17 | Low | Panel widths not persisted across sessions. | R28 |
| G18 | Low | Workspace visibility changes on sim enter/exit with no user hint. | R28 |
| G19 | Low | No keyboard alternatives for pan/zoom/orbit; limited keyboard shortcuts overall. | R34 |
| G20 | Low | No orthographic projection mode in camera. | R31 |

---

## H. Documentation Drift

Documentation that doesn't match the actual codebase.

| # | Severity | Description | Source | Location |
|---|----------|-------------|--------|----------|
| H1 | **High** | architecture/high_level_design.md says "Simulation is currently heightmap-based" — actually tri-dexel. | R47 | high_level_design.md:112-119 |
| H2 | **High** | README.md says "heightmap stock simulation" — actually tri-dexel volumetric grids. | R47 | README.md:11 |
| H3 | **Medium** | 6 new core modules not documented in architecture: dexel_stock.rs, dexel_mesh.rs, dexel.rs, semantic_trace.rs, debug_trace.rs, simulation_cut.rs. | R47 | high_level_design.md |
| H4 | **Medium** | TRI_DEXEL_SIMULATION.md exists but not indexed in architecture/README.md. | R47 | architecture/README.md |
| H5 | **Medium** | Tri-dexel algorithm not attributed in CREDITS.md (violates project policy). | R47 | CREDITS.md |
| H6 | **Medium** | FEATURE_CATALOG claims vendor LUT not wired — actually is fully GUI-wired. | R17 | FEATURE_CATALOG.md |
| H7 | **Medium** | Dressup application order is hardcoded but undocumented. | R21 | helpers.rs |
| H8 | **Low** | Comment says "KD-tree" but code is uniform spatial grid. | R43 | mesh.rs:367 |
| H9 | **Low** | Various operation-specific parameter docs that don't match behavior (VCarve max_depth=0, flat_depth asymmetry in inlay). | R05,R07 | Various |

### Quick Doc Fixes (< 1 hour total)
- H1/H2: Replace "heightmap" with "tri-dexel" in architecture docs and README
- H4: Add TRI_DEXEL_SIMULATION.md to architecture/README.md index
- H5: Add tri-dexel entry to CREDITS.md
- H6: Update FEATURE_CATALOG vendor LUT status
- H8: Fix "KD-tree" comment to "uniform spatial grid"

---

## I. Missing Features & Incomplete Implementations

Features not yet implemented but reasonable for a CAM application.

| # | Priority | Description | Source |
|---|----------|-------------|--------|
| I1 | Medium | No tool change (M6) support in G-code output. | R22 |
| I2 | Medium | No coolant support (M7/M8/M9) in G-code output. | R22 |
| I3 | Medium | No tool compensation in Project Curve operation — user must pre-offset curves. | R14 |
| I4 | Medium | DXF import: Lines, Arcs, Splines entity types silently ignored. | R19 |
| I5 | Medium | Collision check only processes first toolpath with STL mesh — multi-tool jobs may miss collisions. | R38 |
| I6 | Low | No boolean polygon operations (union/intersection/difference). | R25 |
| I7 | Low | No drill hole TSP ordering to minimize rapid travel. | R08 |
| I8 | Low | No multi-select support in UI. | R29 |
| I9 | Low | No mesh subdivision for coarse meshes with fine stepovers in dropcutter. | R09 |
| I10 | Low | No DXF INSUNIT header handling — assumes mm. SVG units fixed at mm, no px/in/cm conversion. | R19,R35 |
| I11 | Low | No streaming/chunking for large STL files. | R18 |
| I12 | Low | Face operation not exposed in CLI. | R01 |

---

## Summary Statistics

### Issue Counts by Severity

| Severity | Bugs (A) | Unwired (B) | Error Handling (C) | Perf (D) | UI/UX (G) | Docs (H) | Missing (I) | Total |
|----------|----------|-------------|-------------------|----------|-----------|----------|-------------|-------|
| Critical | 2 | 0 | 0 | 0 | 0 | 0 | 0 | **2** |
| High | 3 | 2 | 5 | 3 | 1 | 2 | 0 | **16** |
| Medium | 5 | 5 | 13 | 3 | 8 | 4 | 5 | **43** |
| Low | 0 | 9 | 6 | 6 | 11 | 2 | 7 | **41** |
| **Total** | **10** | **16** | **24** | **12** | **20** | **8** | **12** | **102** |

### Suggested Implementation Plan Ordering

**Phase 1 — Bug Fixes & Safety (highest risk, lowest effort)**
1. Fix confirmed bugs A1-A10 (especially A1, A2, A3 — wrong machining output)
2. Add `catch_unwind()` to worker threads (C2, C3)
3. Add input validation at boundaries (C4, C5, C6, C14, C16)
4. Atomic project saves (C10)

**Phase 2 — Wire Existing Code (medium effort, high value)**
5. Wire pre/post G-code emission (B1)
6. Extend undo to all 5 action types (B2)
7. Expose missing GUI parameters (B7, B8, B9)
8. Wire auto-regeneration (B3)
9. Route UI mutations through controller (G1)

**Phase 3 — Performance & Render Quality**
10. Parallelize dropcutter batch grid (D4)
11. Bitset dedup for spatial index (D5)
12. Cache simulation mesh colors (D3)
13. Configurable line width rendering (D1)
14. Indexed mesh rendering (D2)

**Phase 4 — Code Quality & Maintainability**
15. Split oversized files: execute.rs, properties/mod.rs, events.rs
16. Extract dressup tracing helper (saves 320 LOC)
17. Unify error types to CamError enum
18. Standardize CLI parameter naming

**Phase 5 — Testing**
19. CLI integration tests
20. Property-based tests for geometric invariants
21. Expand dropcutter, simulation_cut, face milling tests
22. End-to-end multi-operation tests
23. Coordinate transform tests (24 combos)

**Phase 6 — Documentation & Polish**
24. Update heightmap → tri-dexel in all docs (H1-H5)
25. UI: model deletion, re-import, tooltips, validation consolidation
26. G-code: M6 tool change, M7/M8/M9 coolant
27. DXF: Lines/Arcs/Splines support
