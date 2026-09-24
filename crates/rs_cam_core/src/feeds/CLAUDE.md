# `feeds/` — feeds and speeds

The calculator for RPM, feed, plunge rate, DOC and WOC. The validated entry point is `feeds::suggest`.

## Files
- `mod.rs`, `tests.rs` — the calculator and its unit tests.
- `suggest.rs` + `suggest/` — the Suggest model and read doors; children `apply.rs` (write-back
  funnel, previews, defaults), `invariants.rs` (clamps, back-offs), `aggressiveness.rs` (pass 6b,
  the dial), `axial_envelope.rs`, `adaptive_entry.rs`, `ladder.rs` (3D Rough step ladder writes), `tests.rs`.
- `geometry.rs`, `geometry_class.rs`, `cutter_constraints.rs`, `force.rs`, `predict.rs`,
  `efficiency.rs`, `operating_point.rs` — effective diameter, chip thinning, the DOC scale, the axial
  envelope, force, deflection, efficiency, and spindle power at the shipped operating point.
- `vendor_lut.rs`, `vendor_lookup.rs`, `vendor_normalize.rs` — vendor table, lookup, type mapping.
- `extrapolation.rs` + `extrapolation/{size,hardness}.rs` — `Extrapolation`, `Claim`; `SizeLaw` (G1 size claim,
  form A/B/C or a refusal) and `hardness_basis` (G2 Janka law + soft/hard cap) run in `build_result` on every row.
- `profile.rs`, `quantities.rs`, `provenance.rs`, `support.rs` — the pre-sim profile, the boundary
  newtypes, the per-value provenance, the `FeedsSupport` arm.
- `rationale.rs`, `feed_explanation.rs`, `explain_payload.rs` — rationale tree, stage record, UI contract.

## Invariants
- `ChiploadBounds` in Suggest mirrors the gate's piecewise-linear DOC derating (canonical scale: `feeds::geometry`).
- One Janka table (`WoodSpecies` generics); no hardness scale on plywood/MDF; in solid wood an upward scale stops at the row family's printed soft/hard max.
- Suggest is the validated application path; a raw parameter write is an override and stales the result.
- R4: the rubbing floor `min(0.025, band min)` warns, never lifts; no machine or L/D feed factor. The
  dial `MachineProfile::aggressiveness` (0.85) scales depth and stepover to hold load at
  `k × L/D share`; it never cuts the feed. Power ceiling = `power_at_rpm`, no fraction.
- On an adaptive rough the simulated chipload outranks the Suggest verdict. Step ladder (D7): a reader of the deepest bite reads `deepest_axial_step()`; a depth write goes through `suggest/ladder.rs` (cap every step, or scale the whole ladder). With a ladder Suggest keeps the operator's steps and only lowers them (ruling 1, 2026-09-24); a removed step files `CoarseStepRemoved`.
- One claimed band: every consumer reads `LookupResult::size_basis`; a `Refused` row has no band anywhere.

## Sentries
Run one with `cargo test -p rs_cam_core -q --test <name>`: `lut_resolver_census_a6`, `lookup_parity`, `rubbing_floor_warns_and_never_lifts`,
`long_tool_derate_is_shown_ld1`, `a_refused_deflection_is_not_a_zero_g_t4` (a refusal is an `Err`), `a_feed_lift_caps_at_the_cutting_ceiling_g_t18`,
`a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown` (feed rounds DOWN), `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`,
`a_published_power_is_at_the_depth_that_cuts_g_s2`, `every_cell_declares_its_feeds_support_fm0`, `one_depth_derate_for_feed_and_band_fm3`,
`suggest_refuses_what_the_registry_refuses_fm4`, `clueless_cells_refuse_and_backed_cells_ship_fm5`, `finish_depth_is_reported_not_capped_fm6`,
`the_dial_holds_the_load_and_never_cuts_the_feed_fm7`, `rpm_follows_the_feed_ceiling_fm8`, `micro_extrapolation_refuses_fm9`,
`a_tapered_row_is_read_at_the_tip_a1`, `the_micro_tapered_finish_ships_the_printed_tip_row_g1`, `a_size_claim_states_its_rule_range_and_residual_g1`,
`every_consumer_reads_one_claimed_band_g1`, `the_onsrud_vbit_rows_serve_mdf_and_plywood_g2`, `one_janka_table_for_row_and_query_g2`,
`a_hardness_transfer_caps_at_the_printed_soft_hard_ratio_g2`, `a_coarse_step_is_seen_by_suggest_and_the_envelope_g_ladder`. `feeds_matrix_instrument_fm1` is an `#[ignore]` instrument (`-- --ignored`)
that writes planning/feeds_matrix_2026-09-23/. `wanaka_suggest_integration` takes minutes: ask first.
