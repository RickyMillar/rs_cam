# High-Level Design

## System shape

`rs_cam` is organized as a Rust workspace with a library-first core and two product surfaces layered on top of it.

### `rs_cam_core`

This crate owns the CAM engine:

- geometry primitives and mesh/vector import
- cutter definitions and cutter-contact math
- toolpath generation for 2.5D and 3D operations
- dressups and post-generation transforms
- stock simulation and collision checks
- machine/material models and feeds/speeds
- G-code emission and SVG/HTML visualization helpers

### `rs_cam_cli`

This crate is the batch interface:

- direct one-shot commands for the most important operations
- TOML job execution for scripted workflows
- minimal orchestration around `rs_cam_core`

### `rs_cam_viz`

This crate is the desktop CAM app:

- project state for stock, tools, models, and toolpaths
- parameter editors and operation creation
- worker-thread orchestration for compute, simulation, and collision checks
- viewport rendering, playback, and export flows

## Primary data flow

The dominant runtime path in the GUI is:

1. Import geometry into job state.
2. Define stock, tools, machine, material, and toolpath entries.
3. Build a compute request in `rs_cam_viz`.
4. Execute the requested operation in `rs_cam_core`.
5. Apply dressups and optional stock-boundary clipping.
6. Cache the resulting `Toolpath` in GUI state.
7. Reuse that toolpath for rendering, simulation, collision checks, and export.

The CLI follows the same core path without GUI state or viewport layers.

## Core data model

### Geometry inputs

- triangle meshes from STL
- vector paths and polygons from SVG / DXF
- stock bounding boxes derived from job state

### Machining state

The desktop app organizes machining data around:

- tools
- stock
- machine profile
- material
- toolpath entries

Each toolpath entry combines:

- an operation config
- dressup config
- height config
- execution status
- an optional computed `Toolpath`

### Toolpath IR

All operations emit a shared toolpath representation in `rs_cam_core`.

That IR is the contract between:

- toolpath generation
- dressups
- G-code emission
- simulation
- collision checking
- viewport rendering

## Subsystem notes

### Operation families

The current product surface includes:

- 11 GUI-exposed 2.5D operations
- 11 GUI-exposed 3D operations

The direct CLI exposes the most mature batch-friendly subset, while the GUI exposes the full desktop surface.

### Feeds and speeds

Feeds/speeds are computed in `rs_cam_core::feeds` from:

- tool geometry and flute metadata
- material model
- machine profile
- operation family / pass role
- optional vendor LUT observations

The GUI stores auto/manual toggles per field and writes calculated values back into the live operation config.

### Simulation and collision

Simulation is currently heightmap-based:

- stock is rasterized to a heightmap
- tool motion stamps removal into the grid
- the result is converted back into a renderable mesh

Collision checks focus on holder/shank clearance against stock and toolpath motion.

## Extension path

Adding a new operation typically requires changes in four places:

1. Implement the core algorithm in `rs_cam_core`.
2. Add GUI state types in `rs_cam_viz/src/state/toolpath.rs`.
3. Wire worker dispatch in `rs_cam_viz/src/compute/worker.rs`.
4. Add parameter UI in `rs_cam_viz/src/ui/properties/mod.rs`.

CLI exposure is optional and can be added later if the operation is useful in batch form.

## Current intentional gaps

These are known design-level gaps rather than accidental omissions:

- project persistence is not yet a full-fidelity round-trip of GUI state
- per-operation manual G-code injection is stored in state but not exported
- controller-side tool compensation exists in UI/state but not G-code output
- some GUI toggles are ahead of the worker wiring for the underlying feature

Those gaps are tracked in `planning/` and summarized in `FEATURE_CATALOG.md`.
