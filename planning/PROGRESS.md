# Progress

## Current snapshot

`rs_cam` is now a desktop CAM application plus shared engine, not just an algorithm sandbox.

### Shipped surface

- 3-crate Rust workspace: core library, CLI, and desktop GUI
- 23 GUI-exposed operations
- 14 direct CLI commands plus TOML job execution
- STL, SVG, DXF, and STEP import with BREP face selection
- 5 cutter families
- GRBL, LinuxCNC, and Mach3 post-processors
- feeds/speeds calculator with machine, material, and vendor-LUT inputs
- tri-dexel stock simulation (Z/X/Y grids, all 6 cardinal faces) and holder/shank collision checks
- typed GUI project persistence with missing-model warnings and editable-state round-trip
- dual-lane compute backend with lane-status reporting and active cancel support
- deterministic renderless `rs_cam_viz` regression harness in CI
- controller-first GUI architecture with canonical operation metadata and split compute/controller modules
- shared adaptive support module used by both 2D and 3D adaptive search/control code

## Recent work (2026-04-05)

### Deep architecture audit

Six-domain audit covering type design, error handling, API surface, module organization, state mutation, and concurrency. Implemented high-priority fixes across all domains.

**Undo correctness (HIGH):**
- Simulation state (`results`, `playback`, `checks`) now invalidated on undo/redo of stock, tool, and machine changes — previously showed stale mesh after undo
- Toolpath `stale_since` now set on undo/redo of operation parameter changes — previously left old computed result displayed
- Extracted `invalidate_simulation()` helper method replacing duplicated 5-line pattern

**Parameter bounds checks (HIGH):**
- `emit_ramp()` guards against `max_angle_deg <= 0` or `>= 90` (prevents NaN from `tan()`)
- `emit_helix()` guards against `radius <= 0` (prevents degenerate spiral)
- `spiral_finish_toolpath_structured_annotated()` guards against `stepover <= 0` (prevents division by zero)
- 5 new edge-case tests for boundary parameter values

**Error handling:**
- `OperationError` enum (`MissingGeometry`, `InvalidTool`, `Cancelled`, `Other`) replaces `Result<T, String>` throughout 20+ operation functions in `operations_2d.rs` and `operations_3d.rs`
- Swallowed errors now logged: CLI mesh overlay import, presets directory creation

**API surface cleanup:**
- Deleted dead `pipeline` module (239 lines, zero external callers)
- Renamed viz `CutDirection` (UpCut/DownCut/Compression) to `BitCutDirection` to resolve naming collision with core `ramp_finish::CutDirection` (Climb/Conventional/BothWays)

**Module organization:**
- `operations.rs` (2948 lines) split into 7 per-family files under `operations/`
- `events.rs` (1721 lines) split into 6 handler-group files under `events/`

**Concurrency & lifecycle:**
- Worker threads now have graceful shutdown via `AtomicBool` shutdown flag + `Drop` impl that joins threads
- Simplified `cancel` from redundant `Arc<AtomicBool>` to `AtomicBool` (already inside `Arc<LaneQueue>`)
- Result channel bounded to 64 entries (`sync_channel`) to cap memory under heavy load

**Enum conversion ownership:**
- `DrillCycleType::to_core()` method owns the conversion from viz unit enum → core associated-data enum
- `DressupEntryStyle::to_core()` method owns conversion from viz config → core `EntryStyle`
- Renamed viz `EntryStyle` to `Adaptive3dEntryStyle` to disambiguate from `DressupEntryStyle`
- Extracted adaptive3d entry defaults to named constants (`ADAPTIVE3D_HELIX_RADIUS_FACTOR`, etc.)

**Performance:**
- Dressup pipeline takes ownership (`Toolpath` instead of `&Toolpath`) — eliminates clone-on-early-return in `apply_tabs`, `apply_dogbones`, `apply_link_moves`
- Adaptive3d path segments moved instead of cloned — reordered stamp-then-push to avoid `path_3d.clone()`

### Architecture & reuse audit

Follow-up codebase-wide audit focused on ownership clarity, coupling, and extension friction. Addressed 9 findings across 6 work streams.

**Silent-break footguns fixed:**
- `OperationType::AlignmentPinDrill` was missing from `ALL` array — silently excluded from iteration. Fixed and added exhaustiveness tests for all 10 enums with manually-maintained `ALL` constants (`OperationType`, `ToolType`, `PostFormat`, `ToolMaterial`, `CutDirection`, `FaceUp`, `ZRotation`, `Corner`, `FixtureKind`, `HeightReference`)

**Enum deduplication (core ↔ viz):**
- 6 operation parameter enums (`ProfileSide`, `FaceDirection`, `TraceCompensation`, `ScallopDirection`, `CutDirection`, `SpiralDirection`) were identically defined in both `rs_cam_core` and `rs_cam_viz` with trivial conversion code. Added `Serialize`/`Deserialize` derives to core, deleted viz duplicates, removed conversion matches from `operations_2d.rs`/`operations_3d.rs`

**PostFormat ownership:**
- `PostFormat` enum moved from viz to `rs_cam_core::gcode` with `post_processor()` method. Export functions now call `format.post_processor()` directly instead of string-matching through `get_post_processor()`. Eliminates 3 identical format→string→processor match blocks in `export.rs`

**CLI consolidation:**
- Extracted `run_collision_check()` helper replacing 6 identical 45-line collision-check blocks in CLI `main.rs` (~240 lines removed)
- `CliToolType` enum replaces stringly-typed tool parsing in TOML job files — compile-time exhaustive matching with `serde(alias)` for backward compatibility
- `VendorLut` re-exported from `feeds` module — viz no longer reaches into internal `feeds::vendor_lut::VendorLut` path

### Logic consolidation audit

Full-codebase audit across 5 domains (operations, rendering, feeds/speeds, core/viz boundary, serialization) to reduce scattered logic and improve extensibility. Findings and implementation documented in `planning/CONSOLIDATION_AUDIT.md`.

**Operation extensibility:**
- `OperationParams` trait eliminates ~200 match arms from `catalog.rs` — common accessors (feed_rate, plunge_rate, stepover, depth_per_pass, depth_semantics) now dispatch through `as_params()`/`as_params_mut()` instead of 10 separate 23-arm match blocks
- Deleted 3 duplicate `op_feed_rate()` functions from `preflight.rs`, `sim_timeline.rs`, `sim_diagnostics.rs`

**Rendering consolidation:**
- Centralized 40+ hardcoded color literals into `render/colors.rs` module (toolpath palette, tool assembly, height planes, grid axes, stock, deviation)
- `MoveType::is_cutting()` and `MoveType::feed_rate()` helpers for toolpath move classification

**HTML/Three.js scaffold:**
- Extracted 7 shared helper functions from `viz.rs` (`html_head`, `html_importmap`, `html_scene_setup`, `html_toolpath_objects`, `html_grid_axes`, `html_tail`, `serialize_toolpath_lines`)

**Feeds/speeds ownership:**
- `Material::base_cutting_speed_m_min()` and `Material::plunge_rate_base()` replace hardcoded match blocks in `feeds/mod.rs`
- 12+ magic numbers replaced with named constants (`SLOTTING_THRESHOLD`, `LD_SEVERE_THRESHOLD`, `FLUTE_GUARD_FACTOR`, etc.)

**CLI/viz unification:**
- CLI `build_tool()` now returns `ToolDefinition` instead of `Box<dyn MillingCutter>`, matching the viz crate pattern
- `OpResult.cutter` upgraded to `ToolDefinition`, giving CLI access to assembly info and `to_assembly()` for collision detection

**Project file simplification:**
- `ProjectToolSection::into_runtime()` now constructs `ToolConfig` directly instead of creating a default and overwriting all fields — compiler enforces completeness

## Current priorities

- **Stock-level alignment pins** — moving pins from per-setup to the stock definition so they persist across flips. Design doc: `planning/ALIGNMENT_PINS_DESIGN.md`
- **Tri-dexel simulation** — Phases 1–6 complete (core types, stamping, mesh extraction, viz wiring, multi-setup carry-forward, side-face grids). Design doc: `architecture/TRI_DEXEL_SIMULATION.md`, implementation plan: `planning/VOXEL_SIM_DESIGN.md`
- keep public docs aligned with the actual code surface
- preserve explicit attribution for algorithms, datasets, and runtime assets
- maintain the lint/test gate as the default merge bar

## Recent work (2026-03-23)

### BREP/STEP post-merge improvements

Addressed findings from the independent BREP/STEP review (`review/BREP_STEP_REVIEW.md`):
- **State safety**: face selection cleared on model removal; face_selection IDs validated against enriched mesh on project load with `FaceSelectionStale` warning; u16 face count overflow guard
- **Controller routing**: face pick toggle moved from inline `app.rs` mutation to `AppEvent::ToggleFaceSelection` through the controller event system
- **Undo support**: toolpath param snapshot extended to include `face_selection`, enabling undo/redo of face selection changes
- **User feedback**: STEP import now shows status messages (success or error) in the status bar; ImportStep handling moved to app-level for camera fitting; face polygon fallback warns when selected faces are not horizontal planes
- **UX polish**: operation category labels changed from "from SVG"/"from STL" to "Boundary"/"Surface"; face properties panel shows model name instead of debug `ModelId`

## Recent work (2026-03-22)

### Continuous multi-setup simulation display

Fixed four visual bugs that made the simulation look like separate per-setup runs instead of one continuous process on a block of material:
- **Checkpoint mesh frame**: checkpoint meshes were displayed in global frame without the setup transform — now `load_checkpoint_for_move` applies the same global→local transform as live playback
- **Bottom surface visibility**: `dexel_stock_to_mesh` now generates a bottom-surface mesh (from `ray_bottom`) when bottom cuts exist, so `FromBottom` cuts are visible
- **Playback starts from uncut block**: simulation results now initialize with `current_move=0, playing=true` instead of jumping to the end — the user sees the tool progressively cutting from an uncut block
- **Stock carry-forward in partial re-sim**: `run_simulation_with_ids` now includes all enabled toolpaths from preceding setups as additional groups, so Setup 2 shows Setup 1's residual stock (through-holes, prior cuts)
- Extracted `transform_mesh_to_local_frame` helper for shared mesh frame transform logic

### Semantic simulation debugger

Added a generic trace-driven debugger surface in the Simulation workspace:
- toolpaths can emit both performance traces and move-linked semantic traces, persisted at runtime with JSON artifacts
- Simulation now exposes semantic trees, linked spans, annotations, issue navigation, viewport picking, and inspect-in-simulation flow
- adaptive/adaptive3d emit richer semantic structure and math-stage attribution instead of only generic pass buckets
- runtime hotspots now estimate cutting/rapid time per semantic item so expensive runtime regions are visible alongside compute hotspots

### Tri-dexel Phase 6: Side-face grids and multi-grid mesh

Extended the tri-dexel simulation to support all six cardinal face orientations:
- `DexelGrid::x_grid_from_bounds` and `y_grid_from_bounds` constructors (rays along X/Y, indexed by YZ/XZ)
- `StockCutDirection` extended with `FromFront`, `FromBack`, `FromLeft`, `FromRight`
- Lazy grid initialization: X/Y grids created on first side-face stamp from `stock_bbox`
- Factored out axis-agnostic `stamp_point_on_grid` / `stamp_segment_on_grid` — Z/X/Y grids share the same inner loop with axis decomposition
- `face_up_to_direction` now returns the correct direction for all `FaceUp` variants
- Multi-grid mesh extraction: `dexel_stock_to_mesh` combines Z-grid surface with side-grid surfaces (using `ray_top` heightmap per grid, vertex positions mapped back to world XYZ)
- Checkpoint correctly deep-copies lazily-created side grids
- 15 new tests covering X/Y grid constructors, all four side-face stamp directions, linear segment on Y-grid, multi-grid simulation isolation, checkpoint with side grids, and multi-grid mesh vertex count

### Tri-dexel simulation backend (Phases 1–5)

Replaced the 2.5D heightmap simulation backend with a tri-dexel volumetric
representation throughout the viz crate:
- Core data types: `DexelSegment`, `DexelRay` (SmallVec), `DexelGrid`, `TriDexelStock` (`dexel.rs`, `dexel_stock.rs`)
- Tool stamping and toolpath simulation with `StockCutDirection` (FromTop / FromBottom)
- Mesh extraction via `dexel_stock_to_mesh` producing the same `HeightmapMesh` format (`dexel_mesh.rs`)
- Viz wiring: `SimulationRequest`, `SimulationResult`, `SimCheckpoint`, and `SimulationPlayback` all use `TriDexelStock` instead of `Heightmap`
- Live playback (`update_live_sim`) uses `simulate_toolpath_range` on `TriDexelStock`
- `StockCutDirection` derived from setup's `FaceUp` orientation
- GPU pipeline unchanged — `HeightmapMesh` remains the render format
- **Phase 5: Multi-setup sequential simulation** — `run_simulation_with_all` now simulates ALL setups sequentially on one `TriDexelStock` in the global stock frame. Toolpaths are pre-transformed from each setup's local frame to the global stock-relative frame using `FaceUp`/`ZRotation` inverse transforms (including arc direction correction). `SimulationRequest` carries per-setup `SetupSimGroup`s with direction. Per-boundary direction stored in results enables correct multi-direction live playback scrubbing. Checkpoints at each toolpath boundary support backward scrub across setup transitions.
- 60 core dexel tests + 59 viz tests pass

### Workspace UX redesign — multi-setup coordinate frames

Unified the coordinate frame pipeline so all toolpaths are generated and
displayed in setup-local coordinates. Previously, identity setups (Top+Deg0)
generated in global coords while non-identity setups used local, causing
intermittent alignment bugs.

Key changes:
- Generation always transforms mesh/stock to local frame (even for identity setups)
- All workspaces (including Simulation) display in the active setup's local frame
- Per-workspace display rules: solid stock in Setup only, model hidden in Simulation, etc.
- Setup panel shows effective stock dimensions for active orientation
- "Toolpaths use fresh stock" badge on non-first setups

### Multi-setup simulation — 2.5D heightmap limitation discovered

The 2.5D heightmap can only model cuts from one direction (top-down).
Attempting to simulate flipped setups on one heightmap causes gouging:
bottom cuts are misinterpreted as deep top-to-bottom cuts. This is a
fundamental data structure limitation, not a transform bug.

Current workaround: each setup simulates independently in its own local
frame with fresh stock. Correct for single-setup and same-orientation
multi-setup. Cross-setup material carry-forward deferred to tri-dexel.

### Tri-dexel simulation design

Completed research and design for replacing the heightmap with a tri-dexel
representation. Three orthogonal grids of ray segments (Z, X, Y) handle
cuts from any cardinal direction natively. SmallVec fast path keeps
single-setup performance within 20% of the current heightmap. Six-phase
implementation plan from core data types through multi-setup carry-forward.
See `architecture/TRI_DEXEL_SIMULATION.md`.

## Known open work

- **tri-dexel contour-tiling mesh** — full surface reconstruction for non-heightmap views (current side-grid mesh uses ray_top heightmap)
- emit per-operation manual pre/post G-code in export
- wire profile controller compensation (`G41` / `G42`)
- surface rapid-collision rendering and simulation deviation coloring
- expose workholding rigidity and vendor-LUT management in the GUI
- continue optional cleanup in `adaptive.rs` / `adaptive3d.rs`, but structural blockers are no longer the active tranche

## Verification

- `cargo run -q -p rs_cam_cli -- --help` succeeds
- `cargo fmt --check` passes
- `cargo test -q` passes on the workspace
- `cargo clippy --workspace --all-targets -- -D warnings` passes

Update this file when the shipped surface or verification status changes materially.
