# `adaptive/` — 2D adaptive clearing

The 2D constant-engagement clearing engine. The entry point is
`adaptive::adaptive_toolpath` in `mod.rs`.

## Files

- `mod.rs` — the public facade and the clearing parameters.
- `path.rs` — the main path generation: the orchestrator and its utilities.
- `search.rs` — engagement computation, direction search, entry-point finding.
- `spiral.rs` — the inside-out contour spiral, stage 1 of the engine.
- `material_grid.rs` — the 2D material grid the engagement calculation reads.

## Invariants

- The engine clears a 2D region. A 3D mesh clear belongs in `adaptive3d/`.
- The contour spiral and the trochoid arm share one material grid. Keep the
  grid update in one place when you change either arm.

## Sentries

- `cargo test -p rs_cam_core -q --test adaptive_property_harness`
- `cargo test -p rs_cam_core -q --test contour_spiral_gcode_validity_phase0`

## Do not

- Do not measure a wall-clock win from a plan alone. The accel model decides
  parallel against spiral; see `machine/strategy_advisor.rs`.
