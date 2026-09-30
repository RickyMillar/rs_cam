# `finish/` — 3D finishing strategies

Every 3D finishing strategy and the unified planner. The entry point is
`finish::unified_finish`, or a named strategy module for a solo operation.

## Files

- `mod.rs`, `finish_setup.rs` — facade, shared surface sampling. `finish_planner.rs`,
  `unified_finish.rs`, `unified_finish/` — planner, pass composer, region router.
- `scallop.rs`, `scallop_math.rs`, `scallop_isofield.rs` and `scallop/` —
  constant-scallop rings, the height formulas and the iso-field arm.
- `pencil.rs`, `pencil_dihedral.rs` and `pencil/` — pencil finishing: the
  detectors, the chaining and the pass emission.
- `crest_lines.rs`, `crease_paths.rs`, `classify_probe.rs` — crest lines,
  centreline paths, the fine classification grid. `steep_shallow.rs`,
  `horizontal_finish.rs`, `ramp_finish.rs`, `radial_finish.rs`,
  `spiral_finish*.rs` — the single-strategy finishes.
- `conformal_spiral.rs` + `conformal_spiral/`, `direction_field.rs`, `spiral_finish_compact.rs`
  — research arms (`research` only). `surface_link.rs` — the link kernel `relink_fragments`.

## Invariants

- A finishing-strategy verdict recorded before 2026-08-04 is superseded. See
  `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`.
- The pencil NMS detector stands. Do not route pencil through a flow-accum
  spine; see `../surface/CLAUDE.md`.
- The iso-scallop `iso_field` dial is live; it beats raster at a matched finish.
- Never gate on an aggregate without rendering the surface.
- Every straight feed tracks the drop-cutter surface (G-TIERBURIAL,
  `surface/chord_refine.rs`): ring chords and connectors, the raster and
  waterline bands (`refine_band_chords`), a link with a tolerance.

## Sentries

- `cargo test -p rs_cam_core -q --test finish_resolution_policy_pr3 --test classification_strategy_m3
  --test a_scallop_ring_connector_rides_the_surface_g_tierburial --test fed_chords_ride_the_surface_g_tierburial`
- `... --test scallop_iso_field_config --test the_research_arms_are_feature_gated_fin01`
- `wanaka_decomposes_to_order_ten_regions` (`#[ignore]`; long) in `--test
  finish_planner_wanaka_decompose`; its tables: `..._wanaka_diagnostics`.
- A research harness: `--features research` (off by default), `-- --ignored`.
