# `stock/` — the stock data model and the cut record

The stock types beside the simulation engine, the cut trace and its triage.
The entry point for an operator answer: `stock::sim_triage::SimulationTriage`.

## Files

- `mod.rs`, `dexel.rs` — the facade and the core tri-dexel data types (`LatticeWindow`: a grid window, G-RESTLEVELS).
- `simulation_cut.rs` + `simulation_cut/` — the cut trace, assembly, reports.
- `sim_triage.rs` — "what should I act on?"; `sim_measurability.rs` — can it be measured?
- `collision.rs` — three-state `HolderCollisionCheck`, the result types (`CollisionEvent::segment` is a `tool::body::ZoneKind`); the mesh holder check `check_collisions*` (P22-0c-5: `MountedBody` zones through `ZoneProbe::on_mesh` and `zone_top_at` on `ModelMesh`: whole disc, no probe for a zone no wider than the envelope); `RapidClearanceCheck` = the rapid arm only. `contact.rs` (P22-0c-3) — THE contact kernel `zone_contact` / `zone_top_at` (a body contact zone vs a `ContactSolid`: the dexel stock, `ModelMesh`): `StockContactCheck` (`Core` with `MoveSweep`; `FluteTop`, P22-0c-4 `Shank`/`Holder`/`Shoe` by `SHIPPED_RULE` = pre-move, `DiscSweep` = in-move, ruling Q2; a zone above the flutes no wider than the envelope is shadowed; rapids test the zones above the flutes), `WalkChecks::mounted` (rapid + contact), `drill_flute_overreach`; `radial_profile.rs` — the LUT.
  `stock_mesh.rs`, `dexel_mesh.rs`, `dexel_mesh_mc.rs` — mesh extraction; colours: `export/`.
- `material_slot.rs` — S1 slots; `cut_as.rs` — S6 `CutAs`; `standing_islands.rs` — THE walk of material above a target and its islands (union audit, 3D Rough coverage).

## Invariants

- A `NotMeasurable` metric must abstain. Collision detection stays enabled.
- On every summary field, `None` means not measured and `Some(0.0)` means
  measured and zero. Never publish `None` for a measured zero.
- The air-cut threshold reads `air_cut_pct_of_total_runtime`, not the
  cutting-time denominator. Compare arms by absolute air-cut time. The air /
  low split is `SimulationCutSample::radial_engagement_class` (F200).
- `average_engagement` is a comparative radial-WOC signal, not an absolute
  pass or fail. On a non-flat tool it uses the engaged radius.
- In a cascade, `claims_reference` must be `machined_stock`. Re-simulating
  does not stale a toolpath; regenerate the rest operations afterwards.
- Only a union writes a `MaterialSlot` (0 = stock); it never merges two.
- `SimulationCutArtifact.trace` is an `Arc` SHARED with the result: pass
  `Arc::clone`, never a deep copy of the trace (G-SIMMEM).
- `crosses_standing_material` (G-OVERCUTREF): a stepped op is read against its
  realised step + one cell (`commanded_step_mm`), never its median bite.
- `rough_leaves_islands` (G-ROUGHCOV A3): target = the plan's floor per centre (`adaptive3d::intended_floor_grid`, never a copy) eroded by the tool disc; no centre in reach = not compared; bar = stock to leave + one cell (the leave is in the target once); `touches_wall` = a neighbour wall at least as tall as the island (op-independent, A4); free islands list first; `cell > stock to leave` abstains.

## Sentries (`cargo test -p rs_cam_core -q <args>`)

- `--test material_per_segment_s1`, `--test added_material_cuts_as_stock_s6`, `--test measurability_abstention_r8`,
  `--test narration_denominator_and_hints_d7`, `--test air_cut_one_time_base_g_airdenom`, `--test engagement_denominator_m3`,
  `--test a_fresh_stock_pass_at_its_own_depth_is_not_an_overcut_g_overcutref`,
  `--test a_link_through_stock_is_flagged_g_linkplough` (`link_cuts_stock`; `links_on_mesh` is out), `--test a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov`
- `--test rapid_check_catches_real_side_strikes_g_rapid6497`, `--test rapid_check_catches_shallow_plunges_g_rapidplungetol` (strike classes to keep), `--features test-support --test one_contact_finding_on_every_surface_p22_0c`, `--test a_body_zone_meets_the_stock_p22_0c`.
