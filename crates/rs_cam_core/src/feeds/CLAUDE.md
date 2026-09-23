# `feeds/` — feeds and speeds

The calculator for RPM, feed, plunge rate, DOC and WOC. The validated entry
point is `feeds::suggest`.

## Files
- `mod.rs`, `tests.rs` — the calculator and its unit tests.
- `suggest.rs` + `suggest/` — the Suggest model and read doors; children
  `apply.rs` (write-back funnel, previews, defaults), `invariants.rs` (clamps,
  back-offs), `aggressiveness.rs` (pass 6b, the dial), `axial_envelope.rs`,
  `adaptive_entry.rs`, `tests.rs`.
- `geometry.rs`, `geometry_class.rs`, `cutter_constraints.rs`, `force.rs`,
  `predict.rs`, `efficiency.rs`, `operating_point.rs` — effective diameter, chip
  thinning, the DOC scale, the axial envelope, force, deflection, efficiency,
  and spindle power at the operating point that ships (the one public door).
- `vendor_lut.rs`, `vendor_lookup.rs`, `vendor_normalize.rs` — vendor table, lookup, type mapping.
- `profile.rs`, `quantities.rs`, `provenance.rs`, `support.rs` — the pre-sim
  profile, the boundary newtypes, the per-value provenance, the `FeedsSupport` arm.
- `rationale.rs`, `feed_explanation.rs`, `explain_payload.rs` — the
  rationale tree, the stage record, the UI data contract.

## Invariants
- `ChiploadBounds` in Suggest mirrors the post-simulation gate's
  piecewise-linear DOC derating. The canonical scale is in `feeds::geometry`.
- The lookup is hardness-agnostic within a family; wood families (soft, hard, ply, MDF) never substitute.
- Suggest is the validated application path; a raw parameter write is an override and stales the result.
- R4: the rubbing floor `min(0.025, band min)` warns, never lifts; no machine or L/D feed factor. The
  dial `MachineProfile::aggressiveness` (0.85) scales depth and stepover to hold load at
  `k × L/D share`; it never cuts the feed. Power ceiling = `power_at_rpm`, no fraction.
- On an adaptive rough the simulated chipload outranks the Suggest verdict.

## Sentries
Run one with `cargo test -p rs_cam_core -q --test <name>`: `lut_resolver_census_a6`, `lookup_parity`,
`rubbing_floor_warns_and_never_lifts`, `long_tool_derate_is_shown_ld1`, `a_refused_deflection_is_not_a_zero_g_t4`
(a refusal is an `Err`), `a_feed_lift_caps_at_the_cutting_ceiling_g_t18` (drill clamp caps on the cutting ceiling),
`a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown` (feed rounds DOWN), `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`
(pass 10), `a_published_power_is_at_the_depth_that_cuts_g_s2`, `every_cell_declares_its_feeds_support_fm0`, `one_depth_derate_for_feed_and_band_fm3`,
`suggest_refuses_what_the_registry_refuses_fm4`, `clueless_cells_refuse_and_backed_cells_ship_fm5`, `finish_depth_is_reported_not_capped_fm6`,
`the_dial_holds_the_load_and_never_cuts_the_feed_fm7` (R4 dial), `rpm_follows_the_feed_ceiling_fm8`, `micro_extrapolation_refuses_fm9`. `feeds_matrix_instrument_fm1` is an `#[ignore]` instrument
(`-- --ignored`) that writes planning/feeds_matrix_2026-09-23/. `wanaka_suggest_integration` takes minutes: ask first.
