# Feature Catalog

Canonical product-surface reference for `rs_cam`.

For source attribution and upstream lineage, see [`CREDITS.md`](CREDITS.md).

## Product surface

| Component | Role |
|-----------|------|
| `rs_cam_core` | CAM library: geometry, import, tool modeling, toolpath generation, dressups, simulation, feeds/speeds, and G-code |
| `rs_cam_cli` | Batch CLI and TOML job runner |
| `rs_cam_viz` / `rs_cam_gui` | Desktop CAM application built with `egui` and `wgpu` |

## Operations

| Category | Operation | Core module | GUI | Direct CLI | Status |
|----------|-----------|-------------|-----|------------|--------|
| 2.5D | Face | `face.rs` | Yes | No | Shipped |
| 2.5D | Pocket | `pocket.rs` | Yes | Yes | Shipped |
| 2.5D | Profile | `profile.rs` | Yes | Yes | Shipped |
| 2.5D | Adaptive | `adaptive.rs` | Yes | Yes | Shipped |
| 2.5D | VCarve | `vcarve.rs` | Yes | Yes | Shipped |
| 2.5D | Rest Machining | `rest.rs` | Yes | Yes | Shipped |
| 2.5D | Inlay | `inlay.rs` | Yes | Yes | Shipped |
| 2.5D | Zigzag | `zigzag.rs` | Yes | No | Shipped |
| 2.5D | Trace | `trace.rs` | Yes | No | Shipped |
| 2.5D | Drill | `drill.rs` | Yes | No | Shipped — first-class `OperationFamily` with peck cycles, diameter-scaled `peck_depth` / `plunge_rate_base`, and drill-native metrics (`DrillToolpathSummary` + `drill_gates`) in place of engagement axes. Hole targets can be picked from imported DXF (POINT entities + circle/arc centres, with layer attribution) via viewport click or per-layer "select all"; default (no selection) drills every closed-polygon centroid |
| 2.5D | Chamfer | `chamfer.rs` | Yes | No | Shipped |
| 3D | 3D Finish | `dropcutter.rs` | Yes | Yes | Shipped |
| 3D | 3D Rough | `adaptive3d.rs` | Yes | Yes | Shipped |
| 3D | Waterline | `waterline.rs` | Yes | Yes | Shipped |
| 3D | Pencil Finish | `pencil.rs` | Yes | Yes | Shipped |
| 3D | Scallop Finish | `scallop.rs` | Yes | Yes | Shipped |
| 3D | Unified Finish | `unified_finish.rs` + `finish_planner.rs` | Yes | Yes | Experimental (P2.c — bands by true-surface slope, waterline/scallop/raster per band; naive band order, router lands in P2.d) |
| 3D | Steep/Shallow | `steep_shallow.rs` | Yes | Yes | Shipped |
| 3D | Ramp Finish | `ramp_finish.rs` | Yes | Yes | Shipped |
| 3D | Spiral Finish | `spiral_finish.rs` | Yes | No | Shipped |
| 3D | Radial Finish | `radial_finish.rs` | Yes | No | Shipped |
| 3D | Horizontal Finish | `horizontal_finish.rs` | Yes | No | Shipped |
| 3D | Project Curve | `project_curve.rs` | Yes | No | Shipped |

## Tooling and setup

### Tool families

- Flat end mill
- Ball nose
- Bull nose
- V-bit
- Tapered ball nose

### Tool metadata exposed in the GUI

- geometry: diameter, cutting length, corner radius, included angle, taper angle
- collision envelope: holder diameter, shank diameter, shank length, stickout
- cutting metadata: flute count, tool material, cut direction
- catalog metadata: vendor, product ID

### Machine and material models

- stock material library in `rs_cam_core::material`
- machine profiles in `rs_cam_core::machine`
- feeds/speeds calculator in `rs_cam_core::feeds`
- vendor LUT seeding from embedded observations in `crates/rs_cam_core/data/vendor_lut`

## Toolpath modifiers and control layers

- heights system: clearance, retract, feed, top, bottom
- entry dressups: plunge replacement via ramp or helix
- dogbone overcuts
- lead-in / lead-out arcs
- link moves / keep-tool-down linking
- arc fitting to `G2` / `G3`
- feed optimization dressup with stock-aware engagement estimation on supported workflows
- air-cut filter dressup: removes cutting moves through cleared stock when using remaining-stock mode
- stock-aware generation: per-toolpath "Use remaining stock" toggle pre-simulates prior operations to build actual material state
- per-operation manual pre/post G-code blocks (editable in GUI, emitted in export)
- TSP rapid-order optimization
- stock-boundary clipping with center / inside / outside containment
- dual compute lanes: toolpath generation plus analysis (simulation / collision)
- lane-status chips and a single `Cancel All` overlay action

## Simulation, verification, and export

### Import

- STL mesh import
- SVG vector import
- DXF vector import
- STEP file import (AP203/AP214 via truck crate, face-aware tessellation)

### BREP / face selection

- BREP face picking and selection in the viewport (click to toggle faces on/off)
- Per-face pastel coloring with selection highlighting on enriched meshes
- Face-derived 2D boundaries for 2.5D operations (horizontal planar faces)
- Face-derived containment boundaries for 3D operations
- Face selection persistence in project files (deterministic face IDs from STEP topology)
- BREP topology metadata panel (face count, adjacency, surface type breakdown)

### Export

- G-code: GRBL, LinuxCNC, Mach3
- SVG toolpath preview
- HTML setup sheet
- TOML project/job persistence with editable-state round-trip

### Verification

- tri-dexel stock simulation (Z/X/Y grids, all 6 cardinal face orientations)
- agent-readable MCP toolpath narration (`narrate_toolpath`) for Z-level structure, cut-run vs marching-squares region counts, engagement histogram, suspicious arcs, peak axial DOC, and air-cut summaries
- structural toolpath spans (`Operation`, `DepthPass`, `Region`, entry/lead/link/dressup artifacts) propagated into simulation cut samples for span-aware filtering and outline navigation
- playback, scrub, and checkpoints
- tool visualization during playback
- holder/shank collision checks
- deterministic renderless GUI regression harness with stable automation IDs
- literature-matrix feeds validation suite — 56 cited cells × 19 invariants exercising the feeds engine against vendor / handbook chipload, RPM, and drill envelopes (`crates/rs_cam_core/tests/literature_matrix/` plus the `_litmatrix_*` sentries)
- drill ops produce drill-native verification (`DrillToolpathSummary`, `drill_gates` for chip welding / peck adequacy / plunge feed sanity) instead of engagement metrics, which don't apply to Z-only kinematics

### Provenance gates

- source-freshness reporter — flags warn/stale vendor citations in `crates/rs_cam_core/tests/literature_matrix/sources.toml`
- `/refresh-lit-matrix` skill — guided re-verification or replacement of stale citations

## Known partial areas

These features exist in state, UI, or helper code, but are not yet end-to-end complete:

| Area | Current state |
|------|---------------|
| Project save/load | editable state round-trips and model files are re-imported on load, but computed toolpaths, simulation checkpoints, and collision outputs are not persisted |
| Controller-side compensation | `G41` / `G42` output is not yet implemented; the “In Control” option has been hidden from the Profile UI until it is wired |
| Feed-optimization dressup | Supported only for fresh-stock, flat-stock workflows with known stock bounds; remaining-stock workflows use the air-cut filter instead |
| Rapid collision rendering | Core collision detection exists, but rapid collisions are not yet rendered in the viewport |
| Simulation deviation colors | Helper exists, but deviation data is not currently fed into the renderer |
| Vendor LUT integration | Fully wired: embedded Amana vendor observations are auto-loaded at startup via `LazyLock` and passed into the feeds calculator for all GUI operations |
| BREP face selection scope | Face-derived boundaries work only for approximately-horizontal planar faces; non-planar and tilted faces produce no polygon (falls back to stock bounds). Surface classifier is heuristic (axis-aligned planes only). |
| BREP hover highlighting | Rendering path supports hover colors, but hover face tracking is not yet wired (face under cursor is not detected on mouse move) |
| ~~Workholding rigidity UI~~ | Fully wired: GUI ComboBox (Low/Medium/High) on stock panel, passed through to feeds calculator |

## CLI surface

Verified direct CLI commands (T9 cull, 2026-06-07 — the ~13 per-op
subcommands were replaced by the registry-driven generic `run`):

- `version` — build info for all workspace crates
- `job` — TOML job file (multi-tool/multi-op batch; executes through the session pipeline)
- `run` — ANY of the 23 operations: `run <op> --input model --tool type:diameter --set k=v --output out.nc`; `run --list-ops` / `run <op> --list-params` print the registry
- `sweep` — parameter sweep over a job file with fingerprint diffs
- `project` — GUI project file (format_version=3) full-diagnostics executor
- `smoke` — F-037 smoke baseline suite
- `nc-time` — G-code cycle-time prediction

Every operation in the registry is reachable from the CLI; a new
operation appears in `run` with zero CLI code.
