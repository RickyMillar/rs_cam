# `dexel_stock/` — the tri-dexel simulation engine

Stamps a tool along a toolpath into a tri-dexel stock. Entry point:
`TriDexelStock::simulate_*` in `simulation.rs`.

## Files

- `mod.rs` — the stock type and the public facade.
- `simulation.rs` — the `simulate_*` methods.
- `stamping.rs`, `swept.rs` — the stamping helpers and swept-volume stamping.
- `band.rs`, `band_batch.rs` — row bands, and the ONE batch dispatcher.
- `whole_path.rs`, `playback.rs` — each route's caps, job shape and serial tail.
- `tile_mip.rs` — the coarse mip, for tile rejection.
- `cut_direction.rs` — `StockCutDirection`, the side the tool approaches from.

## Invariants

- A 2D operation cuts at negative Z. `StockConfig.origin_z` in
  `compute/stock_config.rs` is therefore negative and the stock top sits at
  Z zero. A positive `origin_z` puts the retract plane inside the stock.
- The simulation cell must be much smaller than the tool tip radius. It cannot
  resolve a cut at its own size.
- The metric route and the playback route must agree on the stamped volume.
  They share `band_batch.rs` and differ only in what they record.
- Production stamps the Z grid only; the side grids are test-reachable. Keep
  them: a global stock that shows every face needs them (STK-13).
- One axis permutation table: `DexelAxis::decompose`. Both
  `DexelGrid::from_bounds` and `StockCutDirection::decompose` read it (STK-02).

## Sentries

- `cargo test -p rs_cam_core -q --test dexel_stock_z_frame_f024`
- `cargo test -p rs_cam_core -q --test playback_band_dispatch_s6`
- `cargo test -p rs_cam_core -q --test swept_stamping_s1`
- `cargo test -p rs_cam_core -q --test band_stamping_determinism_s3`
- `cargo test -p rs_cam_core -q --test sub_cell_stamping_fa`

## Do not

- `playback.rs` is the NON-metric replay route. Do not read a metric from it.
