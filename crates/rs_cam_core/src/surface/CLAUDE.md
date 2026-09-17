# `surface/` — what the model surface looks like at a point

The two cutter walks and the derived surface fields. The entry point is
`surface::dropcutter::batch_drop_cutter`.

## Files

- `mod.rs` — the facade.
- `dropcutter.rs` — the drop-cutter walk for 3D finishing.
- `pushcutter.rs` — the horizontal push along a fiber at constant Z.
- `slope.rs` — surface slope analysis and the heightmap infrastructure.
- `rest_field.rs` — the rest-depth-field pencil detector, in tool-offset
  space.
- `reach.rs` — the canonical valley-reach policy: how wide a band around a
  rest centreline a tool can reach.
- `flow_accum.rs` — D8 flow routing on a rasterised heightfield.

## Invariants

- A drop-cutter query off the mesh must abstain, not return zero.
- `flow_accum` traces surface drainage. That is a different curve family from
  a rest ridge. Keep the module; do not route pencil through it.
- The valley-reach policy is canonical. Do not derive a second band width.
- `RestGrid` states its grid once, as `grid.grid` — a `maps::grid::GridSpec`.
  `SurfaceHeightmap` and `SlopeMap` keep their own `rows`/`cols`/`cell_size`
  layout on purpose (anisotropic steps, rotated frame).
- `GridZ::is_covered` is a test door behind the `test-support` feature. A
  product path reads a cell through `z_at`, and matches the variant.

## Sentries

- `cargo test -p rs_cam_core -q --test reach_policy_pr4`
- `... --features test-support -q --test grid_z_uncovered_contract_c2`
- `cargo test -p rs_cam_core -q --test rest_routing_probe_e9`
- `cargo test -p rs_cam_core -q --test catchment_basin_census_w0`
- `cargo test -p rs_cam_core -q --test drop_cutter_off_mesh`
