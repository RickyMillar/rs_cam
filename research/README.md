# Research Directory

Comprehensive research for building a 3-axis wood router CAM program in Rust.

## Synthesized Documents

| File | Contents |
|------|----------|
| [01_workflows.md](01_workflows.md) | All CAM workflows to support (2.5D, V-carving, 3D surface, adaptive) |
| [02_algorithms.md](02_algorithms.md) | Algorithm reference with math (drop-cutter, push-cutter, waterline, adaptive, etc.) |
| [03_tool_geometry.md](03_tool_geometry.md) | Tool geometry math for all cutter types (flat, ball, bull, cone, tapered, composite) |
| [04_open_source_reference.md](04_open_source_reference.md) | Analysis of OpenCAMLib, libactp, PyCAM, FreeCAD, Kiri:Moto, Clipper2 |
| [05_rust_ecosystem.md](05_rust_ecosystem.md) | Evaluated Rust crates for every layer of the stack |
| [06_mesh_and_gcode.md](06_mesh_and_gcode.md) | STL processing, spatial indexing, G-code format, post-processors, visualization |
| [07_blue_sky.md](07_blue_sky.md) | Future ideas: GPU acceleration, WASM, physics-based feeds, live monitoring |
| [08_ux_terminology.md](08_ux_terminology.md) | Maps algorithm terms to user-facing terms across 7 CAM tools (Fusion, VCarve, Carbide Create, etc.) |

## Raw Research Dumps

Detailed output from deep-dive research agents. More verbose than the synthesized docs.

| File | Contents |
|------|----------|
| [raw_algorithms.md](raw_algorithms.md) | Full algorithm research with pseudocode and equations |
| [raw_open_source.md](raw_open_source.md) | Detailed open-source CAM project analysis |
| [raw_rust_ecosystem.md](raw_rust_ecosystem.md) | Complete Rust crate evaluations with download stats |
| [raw_opencamlib_math.md](raw_opencamlib_math.md) | OpenCAMLib source code extraction - every cutter, algorithm, data structure |

## Key Findings

1. **No Rust CAM library exists** -- the space is completely open
2. **The Rust ecosystem has strong building blocks** -- geo, parry3d, rayon, nalgebra, clipper2-rust, cavalier_contours
3. **OpenCAMLib is the gold standard** for cutter-mesh interaction math (trait-based architecture maps perfectly to Rust)
4. **Adaptive clearing is the highest-value advanced feature** (Freesteel/libactp approach)
5. **Integer-based polygon operations** (Clipper2 approach) are essential for robustness
6. **Arc preservation** through offset operations produces significantly better G-code
7. **Heightmap approach** (Kiri:Moto) is a valid fast alternative to analytical drop-cutter
8. **F3D format is NOT feasible** for output -- use G-code + CAMotics for verification
