# `stock/` — the stock data model and the cut record

The stock types beside the simulation engine, the cut trace and the triage
that reads it. The entry point for an operator answer is
`stock::sim_triage::SimulationTriage`.

## Files

- `mod.rs`, `dexel.rs` — the facade and the core tri-dexel data types.
- `simulation_cut.rs` and `simulation_cut/` — the cut trace: sample
  accumulation, trace assembly, reporting and the summaries.
- `sim_triage.rs` — one typed answer to "what should I act on?".
- `sim_measurability.rs` — can this run measure the metric you will gate on?
- `collision.rs` — holder and shank collision detection, and the three-state
  `HolderCollisionCheck` the triage reads.
- `stock_mesh.rs`, `dexel_mesh.rs`, `dexel_mesh_mc.rs` — mesh extraction. The
  `StockMesh` container only; ribbons and colour ramps are `export/ribbon.rs`.
- `radial_profile.rs` — the precomputed radial profile lookup table.

## Invariants

- A `NotMeasurable` metric must abstain. Collision detection stays enabled.
- On every summary field, `None` means not measured and `Some(0.0)` means
  measured and zero. Never publish `None` for a measured zero.
- The air-cut threshold reads `air_cut_pct_of_total_runtime`. Do not
  substitute the cutting-time denominator. Prefer absolute air-cut time when
  you compare arms.
- `average_engagement` is a comparative radial-WOC signal, not an absolute
  pass or fail. On a non-flat tool it uses the engaged radius.
- In a cascade, `claims_reference` must be `machined_stock`. Re-simulating
  does not stale a toolpath; regenerate the rest operations afterwards.

## Sentries

- `cargo test -p rs_cam_core -q --test measurability_abstention_r8`
- `cargo test -p rs_cam_core -q --test air_cut_one_time_base_g_airdenom`
- `cargo test -p rs_cam_core -q --test narration_denominator_and_hints_d7`
- `cargo test -p rs_cam_core -q --test engagement_denominator_m3`
