# Review: Architecture Conformance

## Summary
The codebase is well-aligned with its stated architectural guardrails. All four guardrails are either fully compliant or have only minor, justified deviations. Core has zero GUI dependencies, the Toolpath IR is a clean boundary, layers are distinct with intentional (not accidental) cross-composition, and the wiring path is centralized through a single controller → compute → core → state → render pipeline.

## Findings

### Conformance Matrix

| Guardrail | Status | Details |
|-----------|--------|---------|
| 1. Core library independent from GUI | **Compliant** | `rs_cam_core/Cargo.toml` has zero GUI dependencies (no egui, eframe, wgpu). No `viz` types in core. Core's `viz` module is output-only (SVG/HTML generation from Toolpath IR). |
| 2. Toolpath IR as boundary between planning and post-processing | **Compliant** | `Toolpath` struct (toolpath.rs) is the sole contract: `Vec<Move>` where each Move has target `P3` and `MoveType` (Rapid/Linear/ArcCW/ArcCCW with feed rates). All 22 operations produce Toolpath only. Dressups transform Toolpath → Toolpath. G-code and simulation consume Toolpath. No extra metadata bypasses the IR. |
| 3. Distinct layers (import, tools, ops, dressups, sim, export) | **Partial Compliance** | All layers present and separate. Minor entanglement: some operations intentionally compose others (inlay calls pocket + vcarve, chamfer calls profile, depth is a meta-layer). These are justified code reuse, not violations. No cross-layer imports outside these intentional compositions. |
| 4. Extend core + worker + UI wiring path | **Compliant** | Single centralized path: UI event (`AppEvent`) → controller (`controller/events.rs`) → compute backend trait (`compute/mod.rs:ComputeBackend`) → worker thread (`compute/worker.rs, execute.rs`) → core operation → Toolpath result → state cache (`state/toolpath/entry.rs:ToolpathResult`) → render/export. No shortcuts — app.rs calls controller, not core directly. Simulation and collision flow through the same backend. |

### Core Independence (Guardrail 1)
- `rs_cam_core/Cargo.toml` dependencies: nalgebra, geo, stl_io, kiddo, cavalier_contours, usvg, dxf, serde, etc. — **no GUI crates**
- Core's `lib.rs` has 55 pub modules — none reference `viz`, `egui`, `eframe`, or `wgpu`
- The `viz` module in core generates SVG/HTML output from geometric types — this is output rendering, not GUI coupling
- No operation module depends on viz

### Toolpath IR Boundary (Guardrail 2)
- `Toolpath` struct is tight: ~50 lines of struct/enum code
- `Move` contains: target point (`P3`), `MoveType` enum (Rapid, Linear, ArcCW, ArcCCW with feed/arc params)
- All 22 operations return `Result<Toolpath, _>`
- Dressups (entry, dogbone, lead-in/out, link moves, arc fit, tab, feed opt) all take `&mut Toolpath` or `Toolpath → Toolpath`
- G-code generation (`gcode.rs`) consumes `&Toolpath`
- Simulation (`simulation.rs`) consumes `&Toolpath`
- No side-channel data passes around the IR

### Layer Separation (Guardrail 3)
**Module mapping:**
- **Import:** `dxf_input.rs`, `svg_input.rs`, `stl_io` via `mesh.rs` — standalone, no dependency on operations
- **Tool modeling:** `tool.rs`, `machine.rs`, `material.rs` — standalone, used by operations and simulation
- **Operations:** 22 modules (pocket, profile, adaptive, drill, vcarve, inlay, chamfer, etc.) — depend on tools, produce toolpaths
- **Dressups:** `dressup.rs` — consume and produce toolpaths, no operation knowledge
- **Simulation:** `simulation.rs`, `dexel_stock.rs` — consume toolpaths and tool geometry only
- **Export:** `gcode.rs`, `viz.rs` — consume toolpaths only

**Intentional cross-composition (not violations):**
- `inlay.rs` calls `pocket_toolpath()` and `vcarve_toolpath()` — inlay is defined as pocket + vcarve
- `chamfer.rs` calls `profile_toolpath()` — chamfer follows profile path with depth offset
- `depth.rs` is a meta-layer for depth stepping — used by multiple operations

### Wiring Path (Guardrail 4)
Complete path traced:
1. UI event (`ui/AppEvent`) →
2. Controller (`controller/events.rs`) →
3. Compute backend trait (`compute/mod.rs:ComputeBackend`) →
4. Worker thread (`compute/worker.rs` → `execute.rs`) →
5. Core operation (e.g., `pocket_toolpath()`) →
6. `Toolpath` result →
7. State cache (`state/toolpath/entry.rs:ToolpathResult`) →
8. Render / export

- CLI and GUI both reuse identical core paths
- No backdoors: app.rs never calls core operations directly
- Simulation and collision flow through the same backend architecture

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Low | Operation cross-composition (inlay→pocket+vcarve, chamfer→profile) could be considered layer blurring | inlay.rs, chamfer.rs |

**Note:** Issue #1 is an intentional design choice, not an accidental violation. Inlay is semantically pocket+vcarve; chamfer follows a profile path.

## Test Gaps
- No architectural test enforcing that core doesn't depend on GUI crates (could add a `cargo metadata` check)
- No test validating that all operations produce Toolpath IR and nothing else

## Suggestions
1. **Consider a CI check** that `rs_cam_core` has no dependency on `egui`/`eframe`/`wgpu` — prevents accidental coupling
2. **Document the intentional cross-compositions** (inlay→pocket+vcarve, chamfer→profile) in architecture docs so they aren't mistaken for violations
3. **Update `mesh.rs:367` comment** that says "KD-tree" — the code is actually a uniform grid (correct choice, wrong comment)
