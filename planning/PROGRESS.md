# Progress

## Current snapshot

`rs_cam` is now a desktop CAM application plus shared engine, not just an algorithm sandbox.

### Shipped surface

- 3-crate Rust workspace: core library, CLI, and desktop GUI
- 22 GUI-exposed operations
- 14 direct CLI commands plus TOML job execution
- STL, SVG, and DXF import
- 5 cutter families
- GRBL, LinuxCNC, and Mach3 post-processors
- feeds/speeds calculator with machine, material, and vendor-LUT inputs
- heightmap stock simulation and holder/shank collision checks
- typed GUI project persistence with missing-model warnings and editable-state round-trip
- dual-lane compute backend with lane-status reporting and active cancel support
- deterministic renderless `rs_cam_viz` regression harness in CI
- controller-first GUI architecture with canonical operation metadata and split compute/controller modules
- shared adaptive support module used by both 2D and 3D adaptive search/control code

## Current priorities

- keep public docs aligned with the actual code surface
- preserve explicit attribution for algorithms, datasets, and runtime assets
- focus remaining work on user-facing gaps rather than structural cleanup
- maintain the lint/test gate as the default merge bar

## Known open work

- emit per-operation manual pre/post G-code in export
- wire profile controller compensation (`G41` / `G42`)
- surface rapid-collision rendering and simulation deviation coloring
- expose workholding rigidity and vendor-LUT management in the GUI
- continue optional cleanup in `adaptive.rs` / `adaptive3d.rs`, but structural blockers are no longer the active tranche

## Verification

- `cargo run -q -p rs_cam_cli -- --help` succeeds
- `cargo fmt --check` passes
- `cargo test -q` passes on the workspace
- `cargo clippy --workspace --all-targets -- -D warnings` passes

Update this file when the shipped surface or verification status changes materially.
