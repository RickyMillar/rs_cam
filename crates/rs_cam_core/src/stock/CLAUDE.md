# `stock/` — the stock data model and the cut record

The stock types beside the simulation engine, the cut trace and its triage.
The entry point for an operator answer: `stock::sim_triage::SimulationTriage`.

## Files

- `mod.rs`, `dexel.rs` — the facade and the core tri-dexel data types.
- `simulation_cut.rs` + `simulation_cut/` — the cut trace, assembly, reports.
- `sim_triage.rs` — one typed answer to "what should I act on?".
- `sim_measurability.rs` — can this run measure the metric you will gate on?
- `collision.rs` — holder/shank collisions, three-state `HolderCollisionCheck`.
- `stock_mesh.rs`, `dexel_mesh.rs`, `dexel_mesh_mc.rs` (+ `/stride_tests.rs`)
  — mesh extraction (`StockMesh` + S5 vertex slots); colours: `export/`.
- `radial_profile.rs` — the precomputed radial profile lookup table.
- `material_slot.rs` — S1: the segment slot, slot table and stamp tally.

## Invariants

- A `NotMeasurable` metric must abstain. Collision detection stays enabled.
- On every summary field, `None` means not measured and `Some(0.0)` means
  measured and zero. Never publish `None` for a measured zero.
- The air-cut threshold reads `air_cut_pct_of_total_runtime`, not the
  cutting-time denominator. Compare arms by absolute air-cut time.
- `average_engagement` is a comparative radial-WOC signal, not an absolute
  pass or fail. On a non-flat tool it uses the engaged radius.
- In a cascade, `claims_reference` must be `machined_stock`. Re-simulating
  does not stale a toolpath; regenerate the rest operations afterwards.
- Only a union writes a `MaterialSlot` (0 = stock); it never merges two.
- `SimulationCutArtifact.trace` is an `Arc` SHARED with the result: pass
  `Arc::clone`, never a deep copy of the trace (G-SIMMEM).

## Sentries (`cargo test -p rs_cam_core -q <args>`)

- `--test material_per_segment_s1`, `--test measurability_abstention_r8`
- `--test air_cut_one_time_base_g_airdenom`, `--test engagement_denominator_m3`
- `--test narration_denominator_and_hints_d7`
- `--test rapid_check_catches_real_side_strikes_g_rapid6497` and `--test
  rapid_check_catches_shallow_plunges_g_rapidplungetol`: the strike classes
  (side, sliver, rim wall, shallow plunge) that the check must keep.
