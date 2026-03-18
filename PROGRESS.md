# Progress Tracker

Read this FIRST at the start of every session. Update LAST before ending.

## Current Phase: 1 - Foundation (IN PROGRESS)

### What Exists
- [x] Research complete (research/ directory - 8 synthesized docs + 4 raw dumps)
- [x] Architecture complete (architecture/ directory - user stories, requirements, high-level design)
- [x] CLAUDE.md guardrails in place
- [x] Cargo workspace initialized
- [x] Core library + CLI compiling, 22 tests passing

### Phase 1: Foundation
Goal: Load an STL, drop a ball cutter onto it, emit G-code.

- [x] 1.1 Cargo workspace setup (core, cli crates)
- [x] 1.2 Type aliases (P2, P3, V2, V3) and basic geometry (BoundingBox3, Triangle)
- [x] 1.3 STL loading -> indexed TriangleMesh (via stl_io IndexedMesh)
- [x] 1.4 Spatial index over triangles (uniform grid, not KD-tree yet - works fine)
- [x] 1.5 MillingCutter trait definition
- [x] 1.6 FlatEndmill implementation (height/width/vertex_drop/facet_drop/edge_drop)
- [x] 1.7 BallEndmill implementation
- [x] 1.8 PointDropCutter algorithm (single point against mesh)
- [x] 1.9 BatchDropCutter with rayon (parallel grid)
- [x] 1.10 Toolpath IR types (Move, MoveType, Toolpath)
- [x] 1.11 Raster/parallel toolpath from drop-cutter grid (zigzag pattern)
- [x] 1.12 G-code emitter (G0/G1, GRBL + LinuxCNC post-processors)
- [x] 1.13 CLI skeleton: `rs_cam drop-cutter input.stl --tool ball:6.35 --stepover 1.0 -o output.nc`
- [ ] 1.14 End-to-end test: STL -> drop-cutter -> G-code file (needs a test STL fixture)

### Phase 2: 2.5D Operations
- [ ] 2.1 Polygon2 type with geo-types conversion
- [ ] 2.2 Polygon offsetting (cavalier_contours or clipper2-rust)
- [ ] 2.3 Pocket clearing (offset pattern)
- [ ] 2.4 Profile cutting with tool radius compensation
- [ ] 2.5 Zigzag infill pattern
- [ ] 2.6 Depth stepping (multi-pass)
- [ ] 2.7 SVG input (usvg)
- [ ] 2.8 DXF input (dxf crate)
- [ ] 2.9 Helix/ramp entry
- [ ] 2.10 Tab/bridge dressup
- [ ] 2.11 CLI: pocket/profile subcommands

### Phase 3: Advanced Tools & 3D
- [ ] 3.1 BullNoseEndmill implementation
- [ ] 3.2 VBit/ConeCutter implementation
- [ ] 3.3 TaperedBallEndmill (CompositeCutter)
- [ ] 3.4 Push-cutter algorithm
- [ ] 3.5 Fiber and Interval types
- [ ] 3.6 Weave graph (half-edge)
- [ ] 3.7 Waterline algorithm
- [ ] 3.8 Heightmap-based surface operations
- [ ] 3.9 Arc fitting dressup (biarc)
- [ ] 3.10 G2/G3 arc output
- [ ] 3.11 LinuxCNC and Mach3 post-processors

### Phase 4: High-Value Features
- [ ] 4.1 Adaptive clearing (constant engagement)
- [ ] 4.2 V-carving
- [ ] 4.3 Rest machining
- [ ] 4.4 TOML job file parsing
- [ ] 4.5 Dogbone dressup
- [ ] 4.6 Lead-in/lead-out dressup

### Phase 5: Visualization & Polish
- [ ] 5.1 egui + wgpu 3D viewer (rs_cam_viz crate)
- [ ] 5.2 Material removal simulation
- [ ] 5.3 Inlay operations

## Decisions Log
Record non-obvious decisions here so future sessions don't re-debate them.

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-03-19 | nalgebra over glam for primary math | Native parry3d interop, richer type system |
| 2026-03-19 | Toolpath IR not raw G-code | Enables dressups, viz, analysis without G-code parsing |
| 2026-03-19 | "Toolpath" in UX, "Operation" in code | Target audience is hobbyist wood CNC operators |
| 2026-03-19 | F3D output not feasible | Proprietary format, no public spec |
| 2026-03-19 | KD-tree for drop-cutter, BVH for ray queries | Follows OpenCAMLib's proven approach |

## Known Issues / Tech Debt
(none yet)

## Test Fixtures Needed
- Small STL: simple cube or hemisphere (~100 triangles)
- Medium STL: something with curves (~10K triangles)
- SVG: simple pocket shape, text for V-carving
- DXF: basic profile contour
