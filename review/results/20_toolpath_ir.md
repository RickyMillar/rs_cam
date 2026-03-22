# Review: Toolpath IR (Intermediate Representation)

## Summary

The Toolpath IR is a clean, minimal flat `Vec<Move>` structure carrying 3D positions and move types (Rapid, Linear, ArcCW, ArcCCW) with per-move feed rates. The architecture boundary between planning and post-processing is **exceptionally well respected** — no downstream consumer (G-code, simulation, dressups, visualization) reaches back into operation data. Toolpath results are ephemeral (not serialized); only configuration is persisted in TOML, and toolpaths recompute on project load.

## Findings

### Data Structure

- **`Toolpath`** (`toolpath.rs:31-33`): Single struct with `moves: Vec<Move>` — flat vector, no tree/segment hierarchy
- **`Move`** (`toolpath.rs:24-27`): Contains `target: P3` (3D position) and `move_type: MoveType`
- **`MoveType`** (`toolpath.rs:11-20`): Four variants:
  - `Rapid` — G0
  - `Linear { feed_rate: f64 }` — G1, feed in mm/min
  - `ArcCW { i, j, feed_rate }` — G2, XY-plane arcs with I/J offsets
  - `ArcCCW { i, j, feed_rate }` — G3
- **Feed rate**: Per-move (not global), enabling dressup transforms like ramp/helix without re-planning
- **Tool info**: NOT embedded — stored separately via `tool_id` in job state

### Helper Methods

- `emit_path_segment()` (`toolpath.rs:87-109`): Wraps 3D paths with rapid→plunge→feed→retract pattern
- `final_retract()` (`toolpath.rs:112-118`): Conditional retract to safe_z (0.001mm epsilon)
- `total_cutting_distance()` (`toolpath.rs:68-81`): Sums linear and arc move distances
- `total_rapid_distance()` (`toolpath.rs:120-130`): Sums rapid move distances
- `simplify_path_3d()` (`toolpath.rs:138-183`): Douglas-Peucker 3D simplification
- `raster_toolpath_from_grid()` (`toolpath.rs:186-231`): DropCutterGrid → zigzag raster conversion

### Sufficiency — All Operations Use the IR

| Operation | File | Status |
|-----------|------|--------|
| Pocket | `pocket.rs:38-41` | Clean — `contours_to_toolpath()` |
| Profile | `profile.rs:44-49` | Clean — `contour_to_toolpath()` |
| Drill | `drill.rs:46-74` | Clean — builds moves directly |
| Waterline | `waterline.rs:89-150` | Clean — loops over contours |
| Adaptive | `adaptive.rs:1354-1422` | Clean — processes segments into moves |
| Drop-Cutter | via `raster_toolpath_from_grid()` | Clean — grid to zigzag |
| Adaptive3D | GUI worker `semantic_op()` | Clean |

No operation bypasses the IR or bolts on extra data.

### Boundary Role — Strongly Respected

All downstream consumers import **only** `Toolpath`, `Move`, and `MoveType`:

- **G-code** (`gcode.rs:147`): `emit_gcode(toolpath: &Toolpath, post: &dyn PostProcessor, spindle_rpm)` — zero operation imports
- **Simulation** (`simulation.rs:431`): `simulate_toolpath_with_cancel(toolpath: &Toolpath, cutter: &MillingCutter, ...)` — zero operation imports
- **Dressups** (`dressup.rs:31`): `apply_entry(toolpath: &Toolpath, ...)` → `Toolpath` — pure IR-to-IR transforms
- **Visualization** (`viz.rs:14`): `toolpath_to_svg(toolpath: &Toolpath, ...)` — zero operation imports
- **GUI worker** (`execute.rs:271-342`): Explicit pipeline: Generate → Dressup → Boundary Clip, each producing `Toolpath`

No reach-back from post-processing to operation data exists anywhere in the codebase.

### Serialization

- **`Toolpath` has NO `Serialize`/`Deserialize` derives** (`toolpath.rs:30` — only `Debug, Clone, Default`)
- Computed toolpath results are **ephemeral** — stored as `Arc<Toolpath>` in memory only
- **Only configuration** is persisted: operation type, config, heights, dressups, boundary settings (`project.rs:171-210`)
- **Format**: TOML via `toml::to_string_pretty()` (`project.rs:390`), human-editable
- **On load**: Toolpaths marked `Pending` and recomputed on demand (`entry.rs:173, 214-222`)

### Code Quality

- **Zero `unwrap()` in core `toolpath.rs`** — library code is clean
- GUI `project.rs` has ~11 `unwrap()` calls but **all in test code**; production uses `Result` with `.map_err()`
- **Excellent documentation**: Every public item has doc comments explaining semantics (G0/G1/G2/G3), plunge/retract patterns, edge cases
- Tests at `toolpath.rs:233-342` validate all helper methods

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Low | No metadata on toolpath (operation name, tool used, timestamp) — consumers must look this up externally | `toolpath.rs:31-33` |

## Test Gaps

- None significant. Helper methods well-tested. IR consumption tested indirectly through operation and G-code tests.

## Suggestions

- The IR's minimality is a strength. No changes recommended — it correctly serves as a clean boundary between planning and post-processing.
