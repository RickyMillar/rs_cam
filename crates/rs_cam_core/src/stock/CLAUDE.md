# `stock/` — the stock data model and the cut record

The stock types beside the simulation engine, the cut trace and its triage.
The entry point for an operator answer: `stock::sim_triage::SimulationTriage`.

## Files

- `mod.rs`, `dexel.rs` — the facade and the core tri-dexel data types (`LatticeWindow`: a grid window, G-RESTLEVELS).
- `simulation_cut.rs` + `simulation_cut/` — the cut trace, assembly, reports.
- `sim_triage.rs` — "what should I act on?"; `sim_measurability.rs` — can it be measured?
- `collision.rs` — three-state `HolderCollisionCheck`; `radial_profile.rs` — the LUT.
  `stock_mesh.rs`, `dexel_mesh.rs`, `dexel_mesh_mc.rs` — mesh extraction; colours: `export/`.
- `material_slot.rs` — S1 slots; `cut_as.rs` — S6 `CutAs` and the one helper.

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
- `crosses_standing_material` (G-OVERCUTREF): a stepped op is read against its
  realised step + one cell (`commanded_step_mm`), never its median bite.

## Sentries (`cargo test -p rs_cam_core -q <args>`)

- `--test material_per_segment_s1`, `--test added_material_cuts_as_stock_s6`,
  `--test measurability_abstention_r8`, `--test narration_denominator_and_hints_d7`,
  `--test air_cut_one_time_base_g_airdenom`, `--test engagement_denominator_m3`,
  `--test a_fresh_stock_pass_at_its_own_depth_is_not_an_overcut_g_overcutref`,
  `--test a_link_through_stock_is_flagged_g_linkplough` (`link_cuts_stock`; `links_on_mesh` is out)
- `--test rapid_check_catches_real_side_strikes_g_rapid6497` and `--test
  rapid_check_catches_shallow_plunges_g_rapidplungetol`: the strike classes to keep.
