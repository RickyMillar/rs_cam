# `finish/` — 3D finishing strategies

Every 3D finishing strategy and the unified planner. The entry point is
`finish::unified_finish`, or a named strategy module for a solo operation.

## Files

- `mod.rs`, `finish_setup.rs` (facade, surface sampling); `finish_planner.rs`, `unified_finish.rs`,
  `unified_finish/` (planner; the composed op; `band_kernels.rs`, `region_routing.rs`).
- `composed.rs` — the W6.4 composed shape: `compose` (classify, cut each region, route).
  `rest_detector.rs` — the rest DETECTOR stage (W6.4.1) and the probe ball.
- `scallop*.rs`, `scallop/` — rings, height formulas, iso-field. `pencil*.rs`, `pencil/` —
  detectors, chaining, emission. `crest_lines.rs`, `crease_paths.rs`, `classify_probe.rs`.
- `steep_shallow.rs`, `horizontal_finish.rs`, `ramp_finish.rs`, `radial_finish.rs`, `spiral_finish*.rs`
  — single-strategy finishes. `surface_link.rs` — `relink_fragments`, `shape_under_ceiling`.
- Research (`--features research,test-support`, `-- --ignored`): `conformal_spiral*`, `direction_field.rs`, `spiral_finish_compact.rs`.

## Invariants

- A finishing-strategy verdict recorded before 2026-08-04 is superseded. See
  `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`.
- The pencil NMS detector stands; no flow-accum spine (`../surface/CLAUDE.md`).
- The iso-scallop `iso_field` dial is live; it beats raster at a matched finish.
- Never gate on an aggregate without rendering the surface.
- Every straight feed tracks the drop-cutter surface (G-TIERBURIAL,
  `surface/chord_refine.rs`): ring chords and connectors, the raster and
  waterline bands (`refine_band_chords`), a link with a tolerance.
- W6.4: a composed op (Unified Finish, Steep and Shallow) states its barriers only through
  `order_constraints`; its rest rule (`composed::ComposedRest`) and link stage come from the runner. A product rest field comes only from `rest_detector::detect_rest`; a caller keeps its
  own tier list. Every finishing link under a ceiling takes `surface_link::shape_under_ceiling` (B16);
  its last leg is `push_link_landing`: into standing stock it is an `EntryPlunge` at the plunge feed (B20).

## Sentries

- `cargo test -p rs_cam_core -q --test finish_resolution_policy_pr3 --test classification_strategy_m3
  --test a_scallop_ring_connector_rides_the_surface_g_tierburial --test fed_chords_ride_the_surface_g_tierburial`
- `... --test scallop_iso_field_config --test the_research_arms_are_feature_gated_fin01`; F030: `--lib finish::horizontal_finish` (`HorizontalFinishParams::refuse`)
- `wanaka_decomposes_to_order_ten_regions` (`#[ignore]`; long) in `--test finish_planner_wanaka_decompose`; its tables: `..._wanaka_diagnostics`.
- W6.4: `--test a_link_between_regions_clears_standing_stock_b16`, `--lib finish::unified_finish`; B20: `--test a_lifted_link_lands_as_an_entry_b20`; D1 (iso-field links read the one ceiling): `--test a_scallop_iso_field_link_clears_standing_stock_d1`;
  W6.4.6 (Steep and Shallow rest + `hookup_mm` links): `--lib finish::steep_shallow`, `--test steep_shallow_cuts_only_the_rest_w6_4_6`.
