# Remediation Tracker

Progress tracker for the 6-phase implementation plan from `review/FINDINGS.md`.
Each task references FINDINGS.md issue IDs (e.g., A1, C2) for traceability.

## Prerequisites

- [ ] Merge all in-flight branches into master
- [ ] Clean up stale worktrees (`git worktree prune`)
- [ ] Verify CI green on merged master (`cargo test -q`, `cargo clippy`)
- [ ] Commit `review/` directory

---

## Phase 1: Bug Fixes & Safety

**Goal**: Eliminate wrong-output bugs and crash-on-panic paths.
**Estimated effort**: 2-3 sessions
**Files most touched**: execute.rs, worker.rs, mesh.rs, events.rs, vbit.rs, tapered_ball.rs, inlay.rs, dressup.rs, job.rs (CLI)

### 1.1 Critical & high-severity bugs

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | A1 | Fix CLI Adaptive3d tool radius: `job.rs` passes `diameter/2.0`, should pass `cutter.radius()` consistently with `main.rs` | 15m | `rs_cam_cli/src/job.rs` |
| [ ] | A2 | Fix rest machining angle double-conversion: remove `.to_radians()` at execute.rs:570 | 5m | `rs_cam_viz/src/compute/worker/execute.rs` |
| [ ] | A3 | Fix tabs on all depth passes: apply tabs only to final depth level, not roughing passes | 1h | `execute.rs`, `dressup.rs` |
| [ ] | A4 | Wire FaceDirection parameter: implement OneWay mode in face toolpath generation | 30m | `execute.rs` |
| [ ] | A5 | Fix inlay male region: include `polygon.holes` in male region construction | 30m | `rs_cam_core/src/inlay.rs` |

### 1.2 Medium-severity bugs

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | A6 | Fix VCarve max_depth=0.0 semantics: either implement "unlimited" or update comment + add validation | 15m | `rs_cam_core/src/vcarve.rs` |
| [ ] | A7 | Guard VBit edge_drop sqrt: add `.max(0.0)` before `.sqrt()` | 5m | `rs_cam_core/src/tool/vbit.rs` |
| [ ] | A8 | Guard TaperedBall edge_drop sqrt: same fix as A7 | 5m | `rs_cam_core/src/tool/tapered_ball.rs` |
| [ ] | A9 | Recompute bounding box after mesh winding fix | 15m | `rs_cam_core/src/mesh.rs` |
| [ ] | A10 | Split inlay GUI output into separate female/male toolpaths (or add sub-path markers) | 1h | `execute.rs` |

### 1.3 Worker thread safety

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | C2 | Add `catch_unwind()` wrapper around worker thread main loops | 1h | `rs_cam_viz/src/compute/worker.rs` |
| [ ] | C3 | Replace `.expect("lane mutex poisoned")` with graceful recovery (log + reset lane to Idle) | 1h | `rs_cam_viz/src/compute/worker.rs` |

### 1.4 Input validation at boundaries

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | C4 | Validate triangle indices are in-bounds after STL parse | 30m | `rs_cam_core/src/mesh.rs` |
| [ ] | C5 | Add tool-in-use check before deletion: warn user, offer cascade or cancel | 1h | `rs_cam_viz/src/controller/events.rs` |
| [ ] | C6 | Add zero-guard on cell_size at grid creation time (return error instead of allowing div-by-zero) | 30m | `rs_cam_core/src/dexel.rs`, `simulation.rs` |
| [ ] | C14 | Add tool-type pre-validation for Scallop (require BallNose) | 15m | `execute.rs` |
| [ ] | C16 | Replace `unwrap_or(ToolId(0))` with proper validation when adding toolpath | 15m | `events.rs` |

### 1.5 Atomic project saves

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | C10 | Implement temp-file + rename pattern for project save | 30m | `rs_cam_viz/src/io/project.rs` |

### Phase 1 exit criteria
- All A1-A5 bugs verified fixed with regression tests
- Worker thread panic no longer kills the application
- Tool deletion warns if tool is in use
- STL with bad indices doesn't panic
- Project save is crash-safe

---

## Phase 2: Wire Existing Code

**Goal**: Connect code that already exists but isn't end-to-end functional.
**Estimated effort**: 2-3 sessions
**Files most touched**: export.rs, gcode.rs, history.rs, properties/mod.rs, events.rs, controller.rs, configs.rs

### 2.1 G-code emission

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | B1 | Wire pre/post G-code: read fields from ToolpathEntry, emit in `emit_gcode_phased()` between preamble/postamble | 1h | `rs_cam_core/src/gcode.rs`, `rs_cam_viz/src/io/export.rs` |

### 2.2 Undo system

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | B2a | Wire ToolChange undo: add snapshot+push in tool property editor (matching stock pattern) | 1h | `properties/mod.rs`, `history.rs` |
| [ ] | B2b | Wire PostChange undo: add snapshot+push in post-processor editor | 30m | `properties/mod.rs`, `history.rs` |
| [ ] | B2c | Wire MachineChange undo: add snapshot+push in machine editor | 30m | `properties/mod.rs`, `history.rs` |
| [ ] | B2d | Wire ToolpathParamChange undo: add snapshot+push in toolpath property editors | 1h | `properties/mod.rs`, `history.rs` |

### 2.3 Route UI mutations through controller

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | G1a | Route stock slider mutations through AppEvent instead of direct state mutation | 1h | `properties/mod.rs`, `events.rs` |
| [ ] | G1b | Route toolpath param mutations through AppEvent | 1h | `properties/mod.rs`, `events.rs` |
| [ ] | G1c | Route machine preset assignment through AppEvent | 30m | `properties/mod.rs`, `events.rs` |

### 2.4 Expose missing GUI parameters

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | B7 | Add UI fields for 3D adaptive: entry_style, detect_flat_areas, region_ordering, fine_stepdown, min_cutting_radius, stock_to_leave_radial | 2h | `properties/mod.rs`, `configs.rs` |
| [ ] | B8 | Expose finishing_passes in pocket properties panel | 15m | `properties/mod.rs` |
| [ ] | B9 | Add workholding rigidity ComboBox to setup properties | 15m | `properties/mod.rs` |

### 2.5 Auto-regeneration

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | B3a | Set `stale_since` timestamp on every toolpath property edit | 30m | `properties/mod.rs` |
| [ ] | B3b | Implement `process_auto_regen()` to submit stale toolpaths after debounce period | 1h | `controller.rs`, `events.rs` |

### 2.6 Dead code cleanup

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | B6 | Implement or remove `ToggleSimToolpath` and `RecalculateFeeds` empty handlers | 15m | `events.rs` |
| [ ] | B11 | Remove dead `run_face()` function | 5m | `execute.rs` |
| [ ] | B15 | Remove or wire `face_up`/`z_rotation` dead fields in CLI job.rs | 15m | `rs_cam_cli/src/job.rs` |

### Phase 2 exit criteria
- Pre/post G-code text appears in exported .nc files
- Undo works for tool, post, machine, and toolpath parameter changes
- All property edits flow through controller (no direct state mutation)
- 3D adaptive has full parameter UI
- Stale toolpaths auto-regenerate after edit

---

## Phase 3: Performance & Render Quality

**Goal**: Speed up the main bottlenecks and improve visual output.
**Estimated effort**: 2 sessions
**Files most touched**: dropcutter.rs, mesh.rs, sim_render.rs, mesh_render.rs, toolpath_render.rs

### 3.1 Parallelism

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | D4a | Parallelize dropcutter batch grid loop with rayon | 1h | `rs_cam_core/src/dropcutter.rs` |
| [ ] | D4b | Parallelize adaptive material grid computation | 2h | `rs_cam_core/src/adaptive.rs` or `adaptive3d.rs` |
| [ ] | D5 | Replace spatial index `Vec<bool>` dedup with bitset | 30m | `rs_cam_core/src/mesh.rs` |

### 3.2 GPU rendering

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | D3 | Cache sim mesh color variants in GPU; switch via uniform instead of re-uploading each frame | 2h | `rs_cam_viz/src/render/sim_render.rs` |
| [ ] | D1 | Implement configurable line width via geometry expansion (line → 2-tri quad) | 3h | `rs_cam_viz/src/render/mod.rs`, shader source |
| [ ] | D2 | Switch mesh upload to indexed rendering with shared vertices | 2h | `rs_cam_viz/src/render/mesh_render.rs` |

### 3.3 Cleanup

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | D6 | Remove unused `kiddo` dependency from rs_cam_core/Cargo.toml | 5m | `Cargo.toml` |

### Phase 3 exit criteria
- Dropcutter 4-8x faster on large meshes
- Simulation playback doesn't re-upload mesh colors every frame
- Toolpath lines visible at all zoom levels
- STL rendering uses ~1/3 the VRAM

---

## Phase 4: Code Quality & Maintainability

**Goal**: Reduce file sizes, eliminate duplication, standardize patterns.
**Estimated effort**: 2-3 sessions
**Files most touched**: execute.rs, properties/mod.rs, events.rs, helpers.rs, gcode.rs

### 4.1 Split oversized files

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | F-exec | Split execute.rs: extract to `execute/operations_2d.rs`, `execute/operations_3d.rs`, `execute/operations_finish.rs`; keep dispatch in `execute/mod.rs` | 2h | `compute/worker/execute.rs` → `execute/` dir |
| [ ] | F-props | Split properties/mod.rs: move operation-specific param functions to `properties/operations/` with one file per type or 2D/3D grouping | 2h | `ui/properties/mod.rs` → `properties/` dir |
| [ ] | F-events | Split handle_internal_event: dispatch to domain-specific handlers (`handle_io_event`, `handle_tree_event`, `handle_compute_event`) | 1.5h | `controller/events.rs` |
| [ ] | F-timeline | Split sim_timeline.rs: extract timeline_scrubber, playback_controls, boundary_annotations | 1h | `ui/sim_timeline.rs` |

### 4.2 Extract duplication

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | F-dressup | Extract `apply_dressup_with_tracing()` helper — eliminates 8 copies of tracing boilerplate | 1h | `compute/worker/helpers.rs` |
| [ ] | F-sim | Consolidate `run_simulation_with_all/ids` into shared `build_simulation_groups()` | 1h | `controller/events.rs` |
| [ ] | F-feeds | Extract `draw_feed_params(ui, cfg)` UI helper — eliminates 15+ copies | 1h | `ui/properties/mod.rs` |

### 4.3 Standardize patterns

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | C7 | Unify error types: define `CamError` enum, convert 2D ops from `String` to structured error | 2h | `rs_cam_core/src/` (many files), `execute.rs` |
| [ ] | F-cli | Standardize CLI parameter naming: `entry` vs `entry_style` → pick one | 30m | `rs_cam_cli/src/main.rs`, `job.rs` |
| [ ] | F-magic | Extract scattered magic numbers to named constants (dogbone angle, holder length, pick thresholds, spacing) | 1h | Various |

### Phase 4 exit criteria
- No file over ~1200 lines (except adaptive.rs, adaptive3d.rs — cohesive algorithms)
- Dressup tracing boilerplate eliminated
- Adding a new operation requires editing one canonical location
- All operations return same error type

---

## Phase 5: Testing

**Goal**: Cover critical untested paths; add systematic test types.
**Estimated effort**: 3-4 sessions
**Files most touched**: new test files, existing `**/tests.rs` files

### 5.1 Zero-coverage critical areas

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | E-cli | Add CLI integration tests: parse demo_job.toml, run 3-4 operations, verify G-code output exists | 2h | `rs_cam_cli/tests/` (new) |
| [ ] | E-dc | Expand dropcutter tests: all 5 tool types, edge cases (vertical edges, degenerate triangles, near-boundary points) | 2h | `rs_cam_core/src/dropcutter.rs` |
| [ ] | E-flat | Expand FlatEndmill tests: edge_drop edge cases, vertical/horizontal/diagonal edges | 1h | `rs_cam_core/src/tool/flat.rs` |
| [ ] | E-face | Expand face milling tests: direction (OneWay once A4 is fixed), stepover edge cases, tool > stock | 1h | `rs_cam_core/src/face.rs` |
| [ ] | E-simcut | Expand simulation_cut tests: cut analytics, engagement computation, metric edge cases | 1h | `rs_cam_core/src/simulation_cut.rs` |

### 5.2 Integration tests

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | E-e2e1 | Add multi-operation integration test: pocket + profile + simulate on same stock | 2h | `tests/end_to_end.rs` |
| [ ] | E-e2e2 | Add import→generate→export test: load SVG → pocket → G-code → validate syntax | 1.5h | `tests/end_to_end.rs` |
| [ ] | E-e2e3 | Add multi-setup simulation test: 2 setups, verify stock carry-forward | 1.5h | `tests/end_to_end.rs` |
| [ ] | E-coord | Add coordinate transform test: parametrized over all 24 FaceUp × ZRotation combos | 1.5h | `rs_cam_viz/src/controller/tests.rs` or `state/` |

### 5.3 Undo & controller tests

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | E-undo | Add history system unit tests: push/pop, redo invalidation, stack overflow, snapshot lifecycle | 1.5h | `rs_cam_viz/src/state/history.rs` |
| [ ] | E-crud | Add controller CRUD tests: add/remove/rename for tool, setup, fixture, keepout, toolpath | 2h | `rs_cam_viz/src/controller/tests.rs` |
| [ ] | E-sel | Add selection cascade tests: delete entity → verify selection cleared/updated | 1h | `rs_cam_viz/src/controller/tests.rs` |

### 5.4 Systematic test types

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | E-prop | Add proptest for geometric invariants: toolpath stays in bounds, no NaN in output, polygon winding preserved through offset | 2h | `rs_cam_core/src/` (new proptest module) |
| [ ] | E-fuzz | Add cargo-fuzz targets for STL, SVG, DXF parsers | 1.5h | `fuzz/` (new) |
| [ ] | E-bench | Run demo_job.toml as CI step; add benchmark regression detection | 1h | `.github/workflows/ci.yml` |

### Phase 5 exit criteria
- CLI crate has >0 tests
- Dropcutter has 15+ tests covering all tool types
- At least 6 end-to-end integration tests
- Undo/redo has test coverage
- Property-based tests exist for core geometric operations
- CI runs demo job as smoke test

---

## Phase 6: Documentation & Polish

**Goal**: Fix doc drift, improve UX, add missing G-code features.
**Estimated effort**: 2 sessions
**Files most touched**: docs, UI files, gcode.rs, dxf_input.rs

### 6.1 Documentation fixes

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | H1 | Update architecture/high_level_design.md: replace heightmap with tri-dexel in simulation section | 15m | `architecture/high_level_design.md` |
| [ ] | H2 | Update README.md line 11: reference tri-dexel instead of heightmap | 5m | `README.md` |
| [ ] | H3 | Add new core modules to architecture docs (dexel*, semantic_trace, debug_trace, simulation_cut) | 30m | `architecture/high_level_design.md` |
| [ ] | H4 | Index TRI_DEXEL_SIMULATION.md in architecture/README.md | 5m | `architecture/README.md` |
| [ ] | H5 | Add tri-dexel algorithm attribution to CREDITS.md | 15m | `CREDITS.md` |
| [ ] | H6 | Update FEATURE_CATALOG: mark vendor LUT as fully wired | 5m | `FEATURE_CATALOG.md` |
| [ ] | H7 | Document dressup application order in helpers.rs or a doc comment | 15m | `helpers.rs` |
| [ ] | H8 | Fix "KD-tree" comment → "uniform spatial grid" | 5m | `mesh.rs` |

### 6.2 UI polish

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | G2 | Add model deletion UI (RemoveModel event + context menu) | 1.5h | `project_tree.rs`, `events.rs` |
| [ ] | G3 | Add re-import workflow (ReloadModel event, re-read file, update mesh) | 1.5h | `controller/io.rs`, `events.rs` |
| [ ] | G4 | Fix SVG/DXF import: set `pending_upload = true` and trigger camera fit | 15m | `controller/io.rs` |
| [ ] | G6 | Normalize scroll zoom direction across platforms | 30m | `camera.rs` |
| [ ] | G7 | Guard Escape key against text field focus in Simulation | 15m | `app.rs` |
| [ ] | G8 | Disable/hide delete-setup button when only 1 setup exists | 15m | `project_tree.rs` |
| [ ] | G9 | Centralize validation: move operation-specific checks to unified function called from UI and submit | 1.5h | `properties/mod.rs`, `events.rs` |
| [ ] | G12 | Add `.on_hover_text()` tooltips for all abbreviated labels (Col, Fix, TP, AN) | 30m | `viewport_overlay.rs`, `status_bar.rs` |

### 6.3 G-code features

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | I1 | Add M6 tool change support: emit between toolpaths when tool changes | 1h | `rs_cam_core/src/gcode.rs` |
| [ ] | I2 | Add coolant support: M7/M8 on, M9 off, configurable per post-processor | 1h | `rs_cam_core/src/gcode.rs`, `configs` |

### 6.4 Import improvements

| Done | ID | Task | Est. | Files |
|------|----|------|------|-------|
| [ ] | I4 | Add DXF Line, Arc, Spline entity support | 2h | `rs_cam_core/src/dxf_input.rs` |
| [ ] | I10 | Add DXF INSUNIT header handling; add SVG unit awareness (px, in, cm) | 1h | `dxf_input.rs`, `svg_input.rs` |

### Phase 6 exit criteria
- All docs reference tri-dexel (not heightmap)
- CREDITS.md has tri-dexel attribution
- Models can be deleted and re-imported
- G-code supports tool changes and coolant
- DXF import handles common entity types
- No UI abbreviations without tooltips

---

## Progress Summary

| Phase | Description | Tasks | Done | Status |
|-------|-------------|-------|------|--------|
| Pre | Merge & cleanup | 4 | 0 | Not started |
| 1 | Bug Fixes & Safety | 17 | 0 | Not started |
| 2 | Wire Existing Code | 16 | 0 | Not started |
| 3 | Performance & Render | 7 | 0 | Not started |
| 4 | Code Quality | 10 | 0 | Not started |
| 5 | Testing | 14 | 0 | Not started |
| 6 | Documentation & Polish | 18 | 0 | Not started |
| **Total** | | **86** | **0** | |
