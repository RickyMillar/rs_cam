# Review: Dependency Health

## Summary
All dependencies are well-chosen, actively maintained, and at recent versions. No critical issues detected. The project uses caret versioning throughout (appropriate for CAM software). Two niche crates (cavalier_contours, dxf) warrant monitoring but are acceptable choices. Rayon is feature-gated as optional. Overall dependency health is excellent.

## Findings

### Dependency Table

#### rs_cam_core

| Crate | Version Spec | Resolved | Role | Status |
|-------|-------------|----------|------|--------|
| nalgebra | 0.33 | 0.33.2 | Math foundation (vectors, matrices) | Excellent — actively maintained |
| geo | 0.32 | 0.32.0 | Polygon operations (point-in-polygon, area) | Excellent — geo-types ecosystem |
| cavalier_contours | 0.7 | 0.7.0 | Polygon offset (parallel_offset) | Niche — core to CAM; monitor |
| stl_io | 0.11 | 0.11.0 | STL file parsing | Good — stable, focused |
| kiddo | 4 | 4.2.1 | Spatial indexing (KD-tree) | Excellent — actively maintained |
| usvg | 0.47 | 0.47.0 | SVG parsing (resvg ecosystem) | Good — active ecosystem |
| dxf | 0.6 | 0.6.1 | DXF file parsing | Niche — fewer alternatives |
| serde + serde_json | 1 | 1.0.228 / 1.0.149 | Serialization | Excellent — ecosystem standard |
| smallvec [union] | 1 | 1.15.1 | Stack-allocated small vectors | Good — widely used |
| rayon (optional) | 1.10 | 1.11.0 | Parallelism | Excellent — feature-gated |
| thiserror | 2 | 2.0.18 | Error derive macros | Good |

#### rs_cam_cli

| Crate | Version Spec | Role | Status |
|-------|-------------|------|--------|
| clap [derive] | 4 | CLI argument parsing | Excellent |
| anyhow | 1 | Error handling | Good |
| serde + toml | 1 / 0.8 | Config parsing | Good |
| tracing + tracing-subscriber | 0.1 / 0.3 | Logging | Excellent |

#### rs_cam_viz

| Crate | Version Spec | Role | Status |
|-------|-------------|------|--------|
| eframe [wgpu] | 0.30 | GUI framework | Excellent — actively maintained |
| egui | 0.30 | Immediate-mode GUI | Excellent |
| egui-wgpu | 0.30 | GPU rendering for egui | Excellent |
| egui_plot | 0.30 | Plot widgets | Good |
| rfd | 0.15 | Native file dialogs | Good |
| image | 0.25 | Image processing | Good |
| bytemuck [derive] | 1 | Safe transmutation | Good |
| serde + serde_json + toml | 1/1/0.8 | Serialization | Excellent |

### Version Pinning Strategy
- **Caret versioning** (^X.Y) used throughout — allows patch updates, blocks breaking changes
- Appropriate for CAM software where stability matters
- Cargo.lock pins exact versions for reproducible builds

### Key Observations

#### Niche Dependencies (Monitor)
1. **cavalier_contours 0.7** — Specialized polygon offset library. Core to CAM toolpath generation. Few alternatives in Rust ecosystem. Should monitor maintenance status.
2. **dxf 0.6** — DXF format parser. Less visible than stl_io but stable. Limited alternatives.

#### Good Choices
- **kiddo over rstar** — Intentional choice for spatial indexing; both are viable
- **rayon as optional** — Feature-gated `parallel` allows opt-out for WASM or constrained targets
- **egui/eframe 0.30** — Latest stable; active development cadence
- **smallvec with union feature** — Optimized for stack allocation of small collections (dexel rays)

#### thiserror Dual Versions
- Workspace specifies v2 (2.0.18); some transitive deps pull v1 (1.0.69)
- Both coexist without conflict — normal for error handling libraries in transition

#### Rust Edition
- Edition 2024 (released Feb 2025 in Rust 1.85.0)
- Running on rustc 1.92.0 — fully supported

### No Issues Found
- No unmaintained dependencies
- No known vulnerabilities
- No unnecessary features enabled
- No duplicated functionality across deps
- Dependency tree depth is reasonable

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| — | — | No issues found | — |

## Test Gaps
- No `cargo audit` integration in CI (could add as workflow step)

## Suggestions
1. **Consider adding `cargo audit` to CI** — automated vulnerability checking
2. **Monitor cavalier_contours and dxf** — niche crates with fewer maintainers
3. **No action needed** — dependency health is solid
