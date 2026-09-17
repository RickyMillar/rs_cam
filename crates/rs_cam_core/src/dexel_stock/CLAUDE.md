# `dexel_stock/` — the tri-dexel simulation engine

The engine that stamps a tool along a toolpath into a tri-dexel stock. The
entry point is `TriDexelStock::simulate_*` in `simulation.rs`.

## Files

- `mod.rs` — the stock type and the public facade.
- `simulation.rs` — the `simulate_*` methods.
- `stamping.rs` — the free-function stamping helpers.
- `swept.rs` — swept-volume stamping.
- `band.rs`, `whole_path.rs`, `playback.rs` — row-band decomposition and the
  two dispatch routes: the metric run and the non-metric playback replay.
- `tile_mip.rs` — the coarse mip over the grid, for tile rejection.
- `cut_direction.rs` — `StockCutDirection`, the side the tool approaches from.

## Invariants

- A 2D operation cuts at negative Z. `StockConfig.origin_z` in
  `compute/stock_config.rs` is therefore negative, and the stock top sits at
  Z equal to zero. A positive `origin_z` puts the retract plane inside the
  stock.
- The simulation cell must be much smaller than the tool tip radius. A cell
  near the tip radius cannot resolve the cut it is asked to measure.
- The metric route and the playback route must agree on the stamped volume.
  They differ only in what they record.

## Sentries

- `cargo test -p rs_cam_core -q --test dexel_stock_z_frame_f024`
- `cargo test -p rs_cam_core -q --test playback_band_dispatch_s6`
- `cargo test -p rs_cam_core -q --test swept_stamping_s1`
- `cargo test -p rs_cam_core -q --test band_stamping_determinism_s3`
- `cargo test -p rs_cam_core -q --test sub_cell_stamping_fa`

## Do not

- Do not infer the cut direction from the order of a hole's Z pair. Read the
  supplied `StockCutDirection`.
