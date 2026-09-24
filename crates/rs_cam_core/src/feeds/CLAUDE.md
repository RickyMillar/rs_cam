# `feeds/` — feeds and speeds

The calculator for RPM, feed, plunge rate, DOC and WOC. The validated entry point is `feeds::suggest`.

## Files
- `mod.rs`, `tests.rs` — the calculator and its unit tests.
- `suggest.rs` + `suggest/` — the Suggest model and read doors; children `apply.rs` (write-back
  funnel, previews, defaults), `invariants.rs` (clamps, back-offs), `aggressiveness.rs` (pass 6b,
  the dial), `axial_envelope.rs`, `adaptive_entry.rs`, `tests.rs`.
- `geometry.rs`, `geometry_class.rs`, `cutter_constraints.rs`, `force.rs`, `predict.rs`,
  `efficiency.rs`, `operating_point.rs` — effective diameter, chip thinning, the DOC scale, the axial
  envelope, force, deflection, efficiency, and spindle power at the shipped operating point.
- `vendor_lut.rs`, `vendor_lookup.rs`, `vendor_normalize.rs` — vendor table, lookup, type mapping.
- `extrapolation.rs` + `extrapolation/{size,hardness,family,drill}.rs` — `SizeLaw` (G1 size claim, form A/B/C or refusal),
  `hardness_basis` (G2 Janka + soft/hard cap), `family_basis` (G3 `FAMILY_RULES`), `drill_basis` (G6 `DRILL_RULES`) run in `build_result`.
- `profile.rs`, `quantities.rs`, `provenance.rs`, `support.rs` — the pre-sim profile, the boundary newtypes, the per-value provenance, the `FeedsSupport` arm.
- `rationale.rs`, `feed_explanation.rs`, `explain_payload.rs` — rationale tree, stage record, UI contract.

## Invariants
- `ChiploadBounds` in Suggest mirrors the gate's piecewise-linear DOC derating (canonical scale: `feeds::geometry`). A V-bit row is read, and its band de-rated, at its printed key (the cutting diameter, or the included angle when the chart prints none), never the engaged width (ruling B4).
- One Janka table (`WoodSpecies` generics); no hardness scale on plywood/MDF; in solid wood an upward scale stops at the row family's printed soft/hard max.
- G6 (B5): no drill rows, no drill multiplier. `DRILL_RULES` (Amana Spektra flat, 3.175-6.0 mm, 2/3 F) read the (Pocket, Roughing) side row x 1/Z (a point stays a point); the drill RPM cap replaces the chart's 18 000; every other drill cell refuses, in every material.
- Suggest is the validated application path; a raw parameter write is an override and stales the result. On an adaptive rough the simulated chipload outranks the Suggest verdict.
- R4: the rubbing floor `min(0.025, band min)` warns, never lifts; no machine or L/D feed factor. The
  dial `MachineProfile::aggressiveness` (0.85) scales depth and stepover to hold load at
  `k × L/D share`; it never cuts the feed. Power ceiling = `power_at_rpm`, no fraction.
- G3 (A3): a chart that names no operation gives one row per printed cell under (Pocket, Roughing); `FAMILY_RULES` (tapered: Onsrud 77-100; bull: Amana corner radius) serve adaptive, contour, parallel, scallop and trace at x1.00. No copy rows, no ball rule (B3). `ProjectCurve` routes ball, tapered and bull to (Parallel, Finish).
- One claimed band: every consumer reads `LookupResult::size_basis`; a `Refused` row has no band anywhere. A2: one printed value is a point (`printed_chipload()`); no band is derived; the modulator caps at it with no floor; the gate is hard above it and advisory below it.

## Sentries
Run one with `cargo test -p rs_cam_core -q --test <name>`: `lut_resolver_census_a6`, `lookup_parity`, `rubbing_floor_warns_and_never_lifts`,
`long_tool_derate_is_shown_ld1`, `a_refused_deflection_is_not_a_zero_g_t4` (a refusal is an `Err`), `a_feed_lift_caps_at_the_cutting_ceiling_g_t18`,
`a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown` (feed rounds DOWN), `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`,
`a_published_power_is_at_the_depth_that_cuts_g_s2`, `every_cell_declares_its_feeds_support_fm0`, `one_depth_derate_for_feed_and_band_fm3`,
`suggest_refuses_what_the_registry_refuses_fm4`, `clueless_cells_refuse_and_backed_cells_ship_fm5`, `finish_depth_is_reported_not_capped_fm6`,
`the_dial_holds_the_load_and_never_cuts_the_feed_fm7`, `rpm_follows_the_feed_ceiling_fm8`, `micro_extrapolation_refuses_fm9`,
`a_tapered_row_is_read_at_the_tip_a1`, `the_micro_tapered_finish_ships_the_printed_tip_row_g1`, `a_size_claim_states_its_rule_range_and_residual_g1`,
`every_consumer_reads_one_claimed_band_g1`, `the_onsrud_vbit_rows_serve_mdf_and_plywood_g2`, `one_janka_table_for_row_and_query_g2`,
`a_hardness_transfer_caps_at_the_printed_soft_hard_ratio_g2`, `a_printed_value_is_held_as_a_point_a2`, `a_printed_cell_serves_every_family_through_one_claim_g3`,
`a_flat_end_plunge_is_the_side_chip_over_z_g6`, `a_vbit_row_is_read_at_its_printed_key_b4`. `feeds_matrix_instrument_fm1` is an `#[ignore]` instrument (`-- --ignored`) that writes planning/feeds_matrix_2026-09-23/. `wanaka_suggest_integration` takes minutes: ask first.
