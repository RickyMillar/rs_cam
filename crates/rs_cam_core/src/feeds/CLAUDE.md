# `feeds/` — feeds and speeds

The calculator for RPM, feed, plunge rate, DOC and WOC. The validated entry
point is `feeds::suggest`.

## Files

- `mod.rs`, `tests.rs` — the calculator and its unit tests.
- `suggest.rs` + `suggest/` — the Suggest model and read doors; children
  `apply.rs` (write-back funnel, previews, defaults), `invariants.rs` (clamps,
  back-offs), `axial_envelope.rs`, `adaptive_entry.rs`, `tests.rs`.
- `geometry.rs`, `geometry_class.rs`, `cutter_constraints.rs`, `force.rs`,
  `predict.rs`, `efficiency.rs` — effective diameter, chip thinning, the DOC
  derating scale, the axial envelope, force, deflection, efficiency.
- `vendor_lut.rs`, `vendor_lookup.rs`, `vendor_normalize.rs` — the vendor
  observation table, its scored lookup and the type mapping.
- `profile.rs`, `quantities.rs`, `provenance.rs` — the pre-sim profile, the
  boundary newtypes, the per-value provenance.
- `rationale.rs`, `feed_explanation.rs`, `explain_payload.rs` — the
  rationale tree, the stage record, the UI data contract.

## Invariants

- `ChiploadBounds` in Suggest mirrors the post-simulation gate's
  piecewise-linear DOC derating. The canonical scale is in `feeds::geometry`.
- The vendor lookup is hardness-agnostic. Material and hardness dial the
  parameters; they do not hard-reject a row.
- Suggest is the validated application path. A raw parameter write is a
  deliberate override and must stale the result.
- A matched vendor band caps the rubbing floor. Without a matched row, a
  sub-2 mm diameter conclusion is provisional.
- On an adaptive rough the simulated chipload outranks the Suggest verdict.

## Sentries
Run one with `cargo test -p rs_cam_core -q --test <name>`: `lut_resolver_census_a6`,
`lookup_parity`, `rubbing_floor_never_exceeds_band`, `a_refused_deflection_is_not_a_zero_g_t4`
(a deflection refusal is an `Err`, never `0.0`), `a_feed_lift_caps_at_the_cutting_ceiling_g_t18`
(a post-Step-9 lift caps on the COMMANDED ceiling), `a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown`
(the feed quantisation rounds DOWN), `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15` (pass 10
re-checks power at the shipped point). `wanaka_suggest_integration` takes minutes: ask first.
