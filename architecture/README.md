# Architecture

## Documents

| File | Contents |
|------|----------|
| [user_stories.md](user_stories.md) | User stories by persona and priority tier |
| [requirements.md](requirements.md) | Functional and non-functional requirements |
| [high_level_design.md](high_level_design.md) | Crate structure, module design, data flow, key decisions, implementation phases |

## Architecture Summary

**rs_cam** is a Rust workspace with three crates:

- **rs_cam_core** -- Library crate. All CAM algorithms, tool definitions, toolpath generation, G-code emission. No GUI dependency. WASM-compatible.
- **rs_cam_cli** -- Binary crate. Thin CLI wrapper using clap. Reads TOML job files.
- **rs_cam_viz** -- Optional binary crate. 3D toolpath viewer using egui + wgpu.

### Core Design Principles

1. **Trait-based extensibility** -- Tools, operations, dressups, and post-processors are all traits. Add new types without modifying existing code.
2. **Toolpath IR** -- Operations produce typed toolpath moves (rapid, linear, arc, helix). G-code is a final serialization step.
3. **2D/3D independence** -- 2.5D operations use polygon offset/boolean engines. 3D operations use mesh + drop-cutter. They compose but don't depend on each other.
4. **Performance by default** -- Spatial indexing (KD-tree/BVH) and parallelism (rayon) are built into every algorithm that touches the mesh.

### Implementation Priority

Phase 1 (Foundation): STL loading, flat+ball cutters, drop-cutter, basic G-code output
Phase 2 (2.5D): Pocketing, profiling, SVG/DXF input, tabs, ramp entry
Phase 3 (Advanced tools): All cutter types, waterline, heightmap ops, arc fitting
Phase 4 (High-value): Adaptive clearing, V-carving, rest machining
Phase 5 (Polish): 3D viewer, simulation, GPU acceleration, WASM
