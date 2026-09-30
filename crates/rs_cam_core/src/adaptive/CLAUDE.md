# `adaptive/` — 2D adaptive clearing

The 2D clearing engine; the entry point is `adaptive_toolpath` (`mod.rs`).

## Files

- `mod.rs` — facade, parameters; `path.rs` — orchestrator, capped walker,
  frontier hop, mop, links, emission; `spiral.rs` — contour spiral.
- `search.rs` — step measures, `PassLoad`, `ToolCentreRegion`, direction
  search, entries; `material_grid.rs` — 2D stock grid, fringe sub-points.

## Invariants

- 2D (`KeepDownLinks::WithinPassLoad`): every step of every producer and
  every link step holds `step_within_pass_load`: swept width <= the ceiling
  (0.3626 at s 2, R 3), sub-point slivers <= ceiling + cell/D, at the end and
  at each cell along the step (`measure_step`). No fallback. A move is legal
  only when the whole segment is inside `ToolCentreRegion` (every piece).
- Planner stock is the emitted path: a step stamps its capsule, a re-entry
  R + the contained helix radius (flat lap), emission drops only collinear
  points, no dressup merge; arcs are held inside the region.
- Exempt: opt-in slot lines, the steps out of a straight plunge. Adaptive3d
  keeps the historical rule (byte parity). Run order is part of the plan
  (no rapid reorder); a `Rapid` retracts Z-only before XY travel.

## Sentries

- `cargo test -p rs_cam_core -q --lib adaptive` (S2 in `search.rs`), and
  `--test` each of: `a_clearing_cut_holds_the_pass_load_g_adaptpassload`,
  `a_link_feeds_only_within_the_pass_load_g_adaptlinkload`,
  `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift`,
  `adaptive_keeps_its_planner_order_g_adaptorder`, `adaptive_property_harness`,
  `contour_spiral_gcode_validity_phase0`, `adaptive3d_emission_byte_parity`

## Do not

- Do not stamp stock no emitted move cuts: a later step reads it wide.
- Do not trust one simulated sample over the bound: read it on the exact
  geometry first (`print_exact_width_along_move`; the sim blends partial
  cells). The accel model, not a plan, decides a wall-clock win.
