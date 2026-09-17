# `finish/` — 3D finishing strategies

Every 3D finishing strategy and the unified planner. The entry point is
`finish::unified_finish`, or a named strategy module for a solo operation.

## Files

- `mod.rs`, `finish_setup.rs` — the facade and the shared surface sampling.
- `finish_planner.rs`, `unified_finish.rs` and `unified_finish/` — the
  planner, the pass composer and the region router.
- `scallop.rs`, `scallop_math.rs`, `scallop_isofield.rs` and `scallop/` —
  constant-scallop rings, the height formulas and the iso-field arm.
- `pencil.rs`, `pencil_dihedral.rs` and `pencil/` — pencil finishing: the
  detectors, the chaining and the pass emission.
- `crest_lines.rs`, `crease_paths.rs`, `classify_probe.rs` — crest lines,
  centreline paths, the fine classification grid.
- `steep_shallow.rs`, `horizontal_finish.rs`, `ramp_finish.rs`,
  `radial_finish.rs`, `spiral_finish*.rs` — the single-strategy finishes.
- `conformal_spiral.rs` + `conformal_spiral/` (`spiral_build.rs`, `tests.rs`)
  and `direction_field.rs` — research arms, both measured.
- `surface_link.rs` — the gouge-safe link kernel `relink_fragments`.

## Invariants

- A finishing-strategy verdict recorded before 2026-08-04 is superseded. See
  `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`.
- The pencil NMS detector stands. Do not route pencil through a flow-accum
  spine; see `../surface/CLAUDE.md`.
- The iso-scallop `iso_field` dial is live; iso-scallop beats raster at a
  matched finish.

## Sentries

- `cargo test -p rs_cam_core -q --test finish_resolution_policy_pr3`
- `cargo test -p rs_cam_core -q --test classification_strategy_m3`
- `cargo test -p rs_cam_core -q --test finish_planner_wanaka_decompose`
- `cargo test -p rs_cam_core -q --test scallop_iso_field_config`

## Do not

- Never gate on an aggregate without rendering the surface.
