# rs_cam - 3-Axis Wood Router CAM in Rust

## What This Is
A Rust CAM program for 3-axis wood CNC routers. Supports tapered ball end mills, adaptive clearing with flat ends, mesh-based 3D milling, and 2D vector path operations. CLI-first, library-first.

## Architecture (DO NOT DEVIATE)
- Workspace: `crates/rs_cam_core` (library), `crates/rs_cam_cli` (binary), `crates/rs_cam_viz` (optional 3D viewer)
- Trait-based extensibility: `MillingCutter`, `CamOperation`, `Dressup`, `PostProcessor`
- Toolpath IR is the central representation (NOT raw G-code). Operations produce Toolpath, G-code is final serialization.
- 2D ops use polygon engine (i_overlay/cavalier_contours). 3D ops use mesh + drop-cutter. They compose but don't depend on each other.
- nalgebra types everywhere (P2, P3, V2, V3 aliases). geo-types only at 2D polygon operation boundaries.
- Every mesh operation MUST use spatial indexing (kiddo KD-tree or bvh crate).
- Parallelism via rayon for grid/batch operations.

## Key Dependencies
nalgebra, geo, i_overlay, cavalier_contours, clipper2-rust, parry3d, kiddo, rstar, stl_io, dxf, usvg, rayon, clap, serde, toml, thiserror, tracing

## UX Terminology (user-facing terms)
- User-facing: "Toolpath" (not "Operation"), "Stock" (not "Workpiece"), "Pocket/Profile/Adaptive/3D Rough/3D Finish/Waterline/VCarve/Drill/Face/Engrave"
- Parameters: stepover, depth_per_pass, feed_rate, plunge_rate, spindle_speed, safe_z, stock_to_leave, scallop_height
- Tools: End Mill, Ball Nose, Bull Nose, V-Bit, Tapered Ball Nose
- See research/08_ux_terminology.md for full mapping

## Session Workflow
1. FIRST: Read `PROGRESS.md` to understand current state
2. Pick ONE bounded task from the current phase
3. Write code + tests for that task
4. Update `PROGRESS.md` before ending
5. Commit with descriptive message

## Code Conventions
- Tests alongside code in same file (`#[cfg(test)] mod tests`)
- Use `thiserror` for error types per module
- Public API gets doc comments. Internal code: only comment non-obvious logic.
- No unwrap() in library code. CLI can use anyhow.
- Prefer concrete types over trait objects unless polymorphism is needed at runtime.

## Reference Docs
- `research/` - Algorithm math, tool geometry, ecosystem analysis
- `architecture/` - User stories, requirements, high-level design
- `research/raw_opencamlib_math.md` - Exact equations for every cutter type
- `research/02_algorithms.md` - Algorithm pseudocode
- `research/08_ux_terminology.md` - User-facing naming conventions

## What NOT to Do
- Don't reorganize the crate structure without discussion
- Don't add dependencies not in the Key Dependencies list without discussion
- Don't skip spatial indexing "for now" - it's not optional
- Don't emit G-code directly from operations - always go through Toolpath IR
- Don't use f32 for geometry (always f64)
- Don't implement algorithms without tests against known values
