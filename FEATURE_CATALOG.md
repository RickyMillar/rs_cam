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
| 2.5D | Drill | `drill.rs` | Yes | No | Shipped |
| 2.5D | Chamfer | `chamfer.rs` | Yes | No | Shipped |
| 3D | 3D Finish | `dropcutter.rs` | Yes | Yes | Shipped |
| 3D | 3D Rough | `adaptive3d.rs` | Yes | Yes | Shipped |
| 3D | Waterline | `waterline.rs` | Yes | Yes | Shipped |
| 3D | Pencil Finish | `pencil.rs` | Yes | Yes | Shipped |
| 3D | Scallop Finish | `scallop.rs` | Yes | Yes | Shipped |
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
- feed optimization dressup with stock-aware heightmap engagement estimation on supported workflows
- TSP rapid-order optimization
- stock-boundary clipping with center / inside / outside containment
- dual compute lanes: toolpath generation plus analysis (simulation / collision)
- lane-status chips and a single `Cancel All` overlay action

## Simulation, verification, and export

### Import

- STL mesh import
- SVG vector import
- DXF vector import

### Export

- G-code: GRBL, LinuxCNC, Mach3
- SVG toolpath preview
- HTML setup sheet
- TOML project/job persistence with editable-state round-trip

### Verification

- heightmap stock simulation
- playback, scrub, and checkpoints
- tool visualization during playback
- holder/shank collision checks
- deterministic renderless GUI regression harness with stable automation IDs

## Known partial areas

These features exist in state, UI, or helper code, but are not yet end-to-end complete:

| Area | Current state |
|------|---------------|
| Manual per-operation G-code | `pre_gcode` / `post_gcode` are editable in the GUI but not emitted during export |
| Project save/load | editable state round-trips and model files are re-imported on load, but computed toolpaths, simulation checkpoints, and collision outputs are not persisted |
| Controller-side compensation | Profile UI exposes “In Control” compensation, but `G41` / `G42` output is not emitted |
| Feed-optimization dressup | Supported only for fresh-stock, flat-stock workflows with known stock bounds; remaining-stock and mesh-derived cases are intentionally disabled |
| Rapid collision rendering | Core collision detection exists, but rapid collisions are not yet rendered in the viewport |
| Simulation deviation colors | Helper exists, but deviation data is not currently fed into the renderer |
| Vendor LUT loading UI | Backend loader exists, but no GUI entry point yet |
| Workholding rigidity UI | Feeds calculator supports it, but the GUI still hardcodes `Medium` |

## CLI surface

Verified direct CLI commands:

- `job`
- `drop-cutter`
- `pocket`
- `profile`
- `adaptive`
- `vcarve`
- `rest`
- `adaptive3d`
- `waterline`
- `ramp-finish`
- `steep-shallow`
- `inlay`
- `pencil`
- `scallop`

The GUI exposes a broader operation surface than the current direct CLI.
