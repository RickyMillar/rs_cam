# Research Directory

Research, background reading, provenance notes, and exploratory design material for `rs_cam`.

For public-facing attribution, algorithm lineage, and bundled-source credits, see [`../CREDITS.md`](../CREDITS.md).

## Synthesized documents

| File | Contents |
|------|----------|
| [01_workflows.md](01_workflows.md) | CAM workflows to support across 2.5D, adaptive, and 3D machining |
| [02_algorithms.md](02_algorithms.md) | Algorithm reference with math for drop-cutter, push-cutter, waterline, adaptive, and related strategies |
| [03_tool_geometry.md](03_tool_geometry.md) | Cutter-geometry math for flat, ball, bull, V-bit, tapered, and composite tools |
| [04_open_source_reference.md](04_open_source_reference.md) | Comparative notes on OpenCAMLib, libactp, PyCAM, FreeCAD CAM, Kiri:Moto, and Clipper2 |
| [05_rust_ecosystem.md](05_rust_ecosystem.md) | Rust crate evaluation and dependency scouting |
| [06_mesh_and_gcode.md](06_mesh_and_gcode.md) | Mesh handling, spatial indexing, G-code, and visualization notes |
| [07_blue_sky.md](07_blue_sky.md) | Future-facing ideas: GPU, WASM, caching, monitoring, and higher-end CAM workflows |
| [08_ux_terminology.md](08_ux_terminology.md) | Mapping between algorithm terms and user-facing CAM terminology |

## Archive

Legacy scratch notes, raw source captures, and verbose research extractions live in `research/archive/`:

- `raw_algorithms.md` — Detailed algorithm research, pseudocode, and citations
- `raw_open_source.md` — Extended open-source CAM project analysis
- `raw_rust_ecosystem.md` — Full Rust crate evaluation notes
- `raw_opencamlib_math.md` — OpenCAMLib-specific math extraction and code-structure notes
- `ramp_docs.md`, `references.md`, `scallop_reference.md`, `steep_shallow_docs.md` — Legacy scratch notes

## Working assumptions preserved here

- OpenCAMLib remains the main reference for cutter/triangle contact math
- Freesteel/libactp remain the main adaptive-clearing lineage
- robust 2D work depends on high-quality boolean/offset tooling
- heightmaps are a valid and useful approximation layer for simulation and some roughing/visualization tasks
