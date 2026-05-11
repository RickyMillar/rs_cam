# Progress

## Current snapshot

`rs_cam` is now a desktop CAM application plus shared engine, not just an algorithm sandbox.

### Shipped surface

- 4-crate Rust workspace: core library, CLI, desktop GUI, and MCP server
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
- unified service layer: `ProjectSession` API in core, shared `execute_operation()` dispatch for all 23 ops
- MCP server (`rs_cam_mcp`) exposing `ProjectSession` tools for AI agent integration

## Recent work (2026-05-11)

### Wanaka optimizer verification + Roadmap F authored

Live MCP session on `wanaka_full_tuned.toml` (2 setups, 8 toolpaths,
6 of 8 BURN-risk at chipload-low baseline). Verified that the
optimizer IS producing real wins where geometry allows:

- TP4 Back Rough: 773s → 475s (-38.6%), all gates green after Apply
- TP10 3D Rough 6: 200s → 180s (-9.8%) at the corrected stepover=2.0
  (optimizer's stage-2 suggestion of stepover=2.6 made deflection
  exceed; stepover=2.0 with the same feed/rpm change is the
  sweet spot)
- Project total: 3796s → 3478s (-8.4%), TPs within bounds 0 → 2/8
- Project-curve / drop-cutter ops (TP5/6/11/12) honestly return
  `no_safe_improvement` — machine `max_feed=4000` is the binding
  constraint, optimizer can't conjure feed headroom that doesn't
  exist

Findings written up as **Roadmap F** in
`planning/UX_PAIN_POINTS_2026-05-11.md`:
- 🔴 F.1 Optimizer predicted verdict diverges from live re-sim (70%
  on TP10 deflection; root cause needs RCA — likely span-boundary
  drift between cached project results and applied regen)
- 🔴 F.2 Apply doesn't auto-verify; user has to manually regen+resim
  to discover the gate flipped red, by which point the modal has
  closed and the prediction is lost
- 🟡 F.3 Deflection gate trips on single-sample lift-bridge
  transients in Waterline-cleanup spans
- 🟡 F.4 Suggestions ignore machine envelope
- 🟡 F.5 Optimizer-locked `feeds_auto.*` fields have no UI indicator
- 🟢 F.6 Project-level Optimize undiscoverable

Suggested PR sequencing 7-13 in the roadmap; F.1 + F.3 are the
trust-critical pair (without F.1's RCA, F.2's auto-verify can't be
calibrated, and without F.3 the auto-verify will fire on false
positives). ~5-6 dev-days for the F stack.

### UX roadmap PR 6 — MCP type coercion (Roadmap E.6)

Three small coercion gaps that made the MCP set_*_param surfaces
fragile to JSON-RPC clients that vary in how they encode scalars:

- **E.6.a** `set_toolpath_param`'s wildcard arm now coerces numeric
  strings ("7", "12.5") into JSON numbers when the existing field is
  numeric or absent. The four explicit fields (`feed_rate`, `plunge_rate`,
  `stepover`, `depth_per_pass`) already had this; op-specific fields
  like `depth`, `cut_depth`, `min_z` previously failed serde with
  `"7"`.
- **E.6.b** `set_dressup_field` adds the symmetric `0/1 → bool` and
  `numeric-string → number` coercions that `set_toolpath_param`'s
  wildcard already had.
- **E.6.c** `SetDressupFieldParam` schema doc explicitly states that
  enum values arrive as bare JSON strings (`"ramp"`, not
  `"\"ramp\""`), and that the server tolerates `0/1` for booleans
  and numeric strings for numbers.

### UX roadmap PR 5 — Operation defaults (Roadmap B.1–B.7)

Stock-aware per-op defaults so a fresh toolpath ships with sensible
depth and dressup choices instead of generic constants:

- **B.1** drop_cutter `min_z` now defaults to the stock-bottom Z
  (was hard-coded `-50.0`, which clipped any stock not at exactly
  that depth).
- **B.2** face `depth` defaults to `max(stock_padding, 1.0)` — a
  sensible "skim the surface" depth (was `0`, a do-nothing toolpath).
- **B.3** profile / drill `depth` default to `stock_z` (full-through);
  pocket `depth = (stock_z * 0.5).min(5.0)`; adaptive `depth = stock_z * 0.5`.
- **B.4** MCP `add_toolpath` now runs the same feeds calculator
  the GUI applies on Feeds-tab render. Without this, MCP-only
  sessions shipped with the static default `feed_rate`.
- **B.5** Roughing role gets `entry_style: Ramp` by default
  (was `None` → vertical plunge for every Pocket / Profile / Adaptive
  / Face / Adaptive3d). Adaptive/Adaptive3d → Helix override; Drill/
  Trace forced back to None — both via `normalize_for_op`, which
  `for_op` now also calls so fresh creates apply the same constraints
  as loaded ones.
- **B.6** `DressupConfig::default` flips `link_moves`,
  `feed_optimization`, `optimize_rapid_order` to `true` (pure wins;
  ops that can't tolerate them are stripped by `normalize_for_op`).
  Test fixtures updated to make their "no TSP" baseline explicit.
- **B.7** Boundary auto-enable to `ModelSilhouette` for 3D ops on
  mesh models so the cutter doesn't sweep over the whole stock area.

Two helpers added in core: `NewDefaultCtx` and
`OperationConfig::new_default_with_ctx` / `apply_stock_defaults`.
The `new_default(op_type)` no-context constructor stays for tests.

Two viz helpers exposed (`compute_feeds_for_op`,
`apply_feeds_result_to_op`) so MCP can mirror the GUI feeds path
without duplicating the FeedsInput plumbing.

B.8 (stepover units hint) and B.9 (advanced collapsibles for
adaptive3d / steep_shallow) are UI-only polish — deferred to PR 7+.

### UX roadmap PR 4 — Sim diagnostics framing (Roadmap C)

Six related fixes to the simulation diagnostics surface:

- **C.1** Issue count partition into "Must address" (collisions /
  hotspots) vs "Informational" (low engagement / air cut). Stops
  ~24 800 air-cut emission-noise issues from drowning out the 14
  hotspots that actually matter.
- **C.2** `verdict_counts_local` swap to `ToolLoadReport.summary()`
  for toolpath-counted denominators ("TPs within bounds" /
  "TPs exceeding" / "TPs fully unmodeled"). Removes the stale
  re-counter that produced "Within bounds: 0" on healthy projects.
- **C.3** BURN-risk chipload tooltip + badge now use the LUT
  `min_mm_per_tooth` floor instead of the breakage cap, so
  "peak / floor" reads correctly. Tooltip prose extended with
  the why ("rubbing → glazing → burns").
- **C.4** Top-N hotspot triage list at the project level (sorted by
  `wasted_runtime_s`), and the in-scope span list is now sorted too.
- **C.5** Burn / breakage tooltip prose now names the controls a
  user can change (raise feed / lower RPM, or vice versa).
- **C.6** Verdict banner mirroring the MCP `run_simulation` rule
  (collisions → ERROR, air > 20% → WARNING, else SUCCESS) above the
  Findings grid for an at-a-glance "is this run good?" answer.

### UX roadmap PR 3 — MCP layer cleanups (Roadmap E.1–E.5)

Five small edits that close opaque-error pain across the MCP surface:

- **E.1** `mcp_load_project` now appends `controller.load_warnings()`
  to its response so missing-model / migration warnings reach an MCP
  client (the GUI already shows them in a modal).
- **E.2** Toolpath panel ERR chip now shows the underlying error
  string on hover instead of being a mute three-letter symbol.
- **E.3** `mcp_list_toolpaths` injects `stale` and `status` fields per
  row by zipping core summaries with `gui.toolpath_rt` — answers
  "does this need regeneration?" without a second round-trip.
- **E.4** generate_all's "No result produced" fallthrough now matches
  on `ComputeStatus`: Done → "completed with no moves — check depth,
  stock, or model assignment"; in-flight statuses include the label.
- **E.5** Doc-only: `ModelIdParam` and `inspect_brep_faces`
  description clarify that `model_id` is the opaque ID from
  `inspect_model`, not a 0-based index.

### UX roadmap PR 2 — STEP/BREP loader (Roadmap D)

Closed the "two project file loaders disagree" pattern for STEP. The
session loader (`project_file::load_model_geometry`'s Step arm) was
downgrading to a flat `TriangleMesh` and setting `enriched_mesh: None`,
silently breaking `inspect_brep_faces` and the GUI face picker for any
project loaded via the session path. The parallel `io::load_model_file`
loader has always preserved the BREP — they're now in sync.

Added a `LoadedGeometry::Enriched` variant + a third arm in the model
loop that constructs `LoadedModel` with `enriched_mesh: Some(...)` and
mesh derived from `enriched.mesh`. Defensive UI: a STEP model loaded
without its enriched mesh now surfaces a "BREP topology not loaded"
warning row above the (absent) face picker, so the symptom isn't a
silent UX gap. Regression test in
`crates/rs_cam_core/tests/step_project_load.rs`.

### UX roadmap PR 1 — MCP export end-to-end (Roadmap A)

Fixed the GUI-embedded MCP `export_gcode` path that was producing 0-byte
output and tripping the chipload gate even when `get_tool_load_report`
showed real data. Root cause: `mcp_export_gcode` called
`session.export_gcode_with_policy` (core), which reads from
`session.results` / `session.simulation` — neither of which the GUI/MCP
path ever populates. Viz worker results live in `gui.toolpath_rt[id]`
and viz simulation in `state.simulation.results.cut_trace`.

Fix: route MCP export through a new
`export_gcode_from_session_with_policy` variant in `io/export.rs`, then
write the file at the MCP layer. Closes both 🔴s in
`planning/UX_PAIN_POINTS_2026-05-11.md` Roadmap A.

## Recent work (2026-05-08)

### Optimizer gap-doc burst — six closures + one new gap opened

Closed six of the seven optimizer gaps tracked in
`planning/cutting-calcs-data-gaps.md`, end-to-end live-validated
against the wanaka project via the MCP `get_tool_load_report` and
`optimize_toolpath` tools.

- **G5 + G6 + G7** (`d09001e`) — vendor LUT lookup widened to support
  engaged-edge geometry on tapered tools, with linear chipload scaling
  by diameter ratio and hardness ratio. Verdict carries
  `Confidence::Approximate(detail)` past ±40 % divergence with the
  scaling factors named in the detail string. Material-family changed
  from a hard match (wood / plastic / metal) to a category gate;
  hardness moved from a reject filter to a soft-scoring lever.
- **G1** (`11e0f9f`) — Profile + Zigzag added to the optimizer's
  `has_doc_knob` allowlist so Stage 1 collapses the stepover dim when
  the op lacks the knob. Bipolar prescription reordered so Contour /
  Trace family ops point at geometry-driven levers instead of DOC.
- **G2** (`c40795b`) — `scallop_height` added as a third axis to
  Stage 1's grid; gate widened from "has DOC knob" to "has any sweep
  knob". Live-validated against wanaka TP 7 (1 attempted → 4
  attempted).
- **G3** (`2926a15`) — Trace, RampFinish, Waterline added to
  `has_doc_knob`; Pencil gets conditional stepover when
  `num_offset_passes > 1`. RadialFinish split out as the new G3a
  (deferred).
- **G14** (`13a469e`) — engaged-diameter usage audit across every
  tool-load gate path; cam-navigator subagent confirmed no code fixes
  needed. Closed audit-only.
- **G13** (`1fe3292`) — replaced the geometric L/D > 6 deflection
  gate with a force-aware tip-deflection estimator. New
  `ToolDefinition::tip_deflection_mm` integrates a stepped cantilever
  (shank + cutting region) using each cutter's existing
  `lookup_diameter_at` profile; `δ = F·L³/(3EI)` from per-sample
  `F = Kc · axial_doc · radial_width` (same arc-equivalent slab as
  the power gate). Verdict thresholds 50 µm Within / 200 µm Exceeds.
  Live wanaka MCP confirmed the End-Mill TPs that previously refused
  pre-flight on `Exceeds(L/D=7.5)` now reach Stage F as
  `Within(Approximate)` 157–175 µm; TaperedBall TPs that previously
  read `Approximate(L/D=5.83)` now read `Validated` at 5–9 µm.

**Opened.** `G15` — investigate Stage F retarget skip on TaperedBall
chipload-Exceeds(Approximate) with extrapolated LUT rows. Surfaced as
a side observation during G2 validation; needs an end-to-end
`optimize_toolpath` MCP run with `attempted`-list inspection before a
fix shape lands.

### Simulation span coverage

Audited structural span and semantic trace coverage for simulation diagnostics. Added `planning/SIMULATION_SPAN_COVERAGE.md` as the coverage tracker. Generation now derives structural spans for operations that previously emitted only a top-level `Operation` span: depth-stepped 2.5D ops get `DepthPass` + cutting-run `Region` spans; drill-like ops get hole/plunge `Region` spans without adding depth-order barriers; other operations get generic cutting-run regions. Adaptive3D keeps its richer annotation-derived spans with labeled z-level/region spans, and Pencil/Scallop/Ramp/Spiral runtime annotations now convert into labeled structural spans. Trace emits semantic `Chain` children under depth levels; drill emits semantic `Hole`/`Cycle` children. `get_cut_trace` now includes `span_summaries` so selected structural spans have aggregate metrics. Simulation outline fallback now shows semantic traces when structural spans are operation-only. Added broad span coverage tests across all 23 operation families, including system-only alignment-pin drilling.

## Recent work (2026-04-11)

### Tech debt audit — post service layer + MCP refactor

Six-domain deep audit using specialist agents across all 110K lines. Full report: [`TECH_DEBT_AUDIT.md`](TECH_DEBT_AUDIT.md).

**Key findings:**
- **Session bypasses (CRITICAL)**: GUI controller still mutates state directly via `_mut()` accessors in 11+ places, skipping cache/simulation invalidation. Fix: add missing session methods, migrate handlers, restrict accessors.
- **Test gaps (CRITICAL)**: Session API (2,297 LOC) has 3 tests, MCP server (1,173 LOC) has 0. Algorithm coverage is excellent (705+ inline tests). Fix: mutation CRUD tests, serde round-trip tests, project file round-trip tests.
- **Tracing (CRITICAL)**: 5.9% of files have tracing. Session layer has zero. Fix: `#[instrument]` on all session public methods, replicate adaptive3d pattern.
- **Data duplication (HIGH)**: `LoadedModel` defined in both core and viz with slightly different fields. Fix: viz wraps core type.
- **Oversized modules (HIGH)**: adaptive3d (4.7K lines), adaptive (2.8K), dexel_stock (1.8K). Fix: split into sub-modules.
- **MCP asymmetry (HIGH)**: 10 non-GUI tools missing from standalone MCP. Fix: add missing tools, extract shared parsing.
- **Working well**: Compute ownership (zero production duplication), MCP thinness (no business logic leaks), algorithm tests, clippy compliance.

## Recent work (2026-04-09)

### Post-extraction cleanup (Phases 1–5)

Systematic cleanup following the service layer extraction and GUI dispatch rewire.

**Phase 1 — Quick wins**: Removed dead code (`pipeline` module, stale `run_*` helpers), unified `OperationError` enum across operation functions (replacing `Result<T, String>`), added MCP server tools, fixed simulation diagnostics bug.

**Phase 2 — Core execution parity**: Wired slope filter, `initial_stock` (prior stock for air-cut filtering), dressup pipeline, and input validations into `rs_cam_core::compute::execute::execute_operation()` so core dispatch matches the full viz compute path.

**Phase 3 — GUI dispatch rewired to core**: Replaced 23 per-operation `SemanticToolpathOp` trait implementations in viz with a single `generate_via_core()` bridge function that delegates to `execute_operation()`. Deleted ~2900 lines of duplicate dispatch code from `operations_2d.rs` and `operations_3d.rs`. Viz now only handles threading, phase tracking, dressups, boundary clipping, and debug/semantic tracing wrapper.

**Phase 4 — Arc\<Toolpath\>, borrowed collision, 12 new tests**: Changed `ToolpathResult.toolpath` from owned `Toolpath` to `Arc<Toolpath>` to eliminate clones on the simulation and collision check paths. Collision check now borrows the toolpath instead of cloning. Added 12 new integration tests covering all 23 operations through `run_compute`.

**Phase 5 — session.rs split, dead code cleanup**: Split `session.rs` (1400+ lines) into focused submodules (`session/mod.rs`, `session/loading.rs`, `session/execution.rs`, `session/export.rs`). Removed dead `run_simulation` wrapper and unused `assert_cutting_moves_are_semantically_covered` test helper from viz. Fixed incorrect `#[allow(dead_code)]` on `circle_from_3_points` in `arcfit.rs` (function is actively called). Tightened `#[allow(dead_code)]` to `#[cfg_attr(not(test), allow(dead_code))]` on three test-only functions (`search_direction`, `adaptive_segments`, `search_direction_3d`).

## Recent work (2026-04-07)

### Service layer extraction (Phases 1–6)

Unified compute engine across GUI, CLI, and future MCP server. One `ProjectSession` in `rs_cam_core` owns project state + compute; one `execute_operation()` dispatches all 23 operations.

**Phase 1–3** (prior session): Moved config types, execution helpers, simulation, and collision checking from `rs_cam_viz` to `rs_cam_core/src/compute/`.

**Phase 4** (prior session): Created `ProjectSession` API in `rs_cam_core/src/session.rs` — load project TOML, generate toolpaths, run simulation, check collisions, export G-code and diagnostics.

**Phase 5** (this session): Rewired CLI `project.rs` from ~2750 lines of duplicate execution code to ~340 lines delegating to `ProjectSession`. CLI now shares the same compute path as the GUI.

**Phase 6** (this session): Created `rs_cam_core/src/compute/execute.rs` with public `execute_operation()` supporting all 23 operations including cutting_levels, pocket patterns, profile tabs, and 7 operations previously missing from core (VCarve, Rest, Inlay, Drill, Chamfer, ProjectCurve, AlignmentPinDrill). Rewired `session.rs` to use this shared dispatch (deleted ~485 lines). Deleted duplicate `build_cutter`, `compute_stats`, and `semantic.rs` from viz (deleted ~218 lines).

**Test fixes**: Fixed 3 pre-existing test failures — stale operation label (`"3D Raster Finish"` → `"3D Finish"`), wrong operation type in UI widget test (Adaptive3d → Scallop for "Stock to Leave" widget), incorrect height resolution expectations in `w6_auto_height_defaults`.

**Known remaining**: 2 pre-existing simulation pipeline failures (`multi_setup_top_bottom_simulation`, `multi_setup_backward_scrub_uses_checkpoints`) — the bottom-up tri-dexel cut produces empty stock. These predate the service layer work.

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

- **G16 layered scoring (in flight)** — multi-commit follow-on to the G16 reorg, softens binary gates and adds composite scoring. Design doc §11: `planning/OPTIMIZER_REFACTOR_G16.md`. **Tracker (read first): `planning/G16_LAYERED_SCORING_PROGRESS.md`**.
- **MCP server polish** — MCP server (`rs_cam_mcp`) is shipped with 16 tools; ongoing work to integrate with running GUI session for real-time AI agent access. Design doc: `planning/SERVICE_LAYER_EXTRACTION.md`
- **Fix 2 remaining simulation test failures** — `multi_setup_top_bottom_simulation` and `multi_setup_backward_scrub_uses_checkpoints` fail because bottom-up tri-dexel cuts produce empty stock
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
