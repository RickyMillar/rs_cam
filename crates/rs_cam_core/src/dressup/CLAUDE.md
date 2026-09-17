# `dressup/` — post-generation transforms

The passes that run after an operation generates a toolpath.
`apply_dressups` in `compute/execute/dressup_apply.rs` drives them; it takes
a `DressupContext`, and `DRESSUP_PIPELINE` there lists every stage in run
order. Three run outside it: `apply_tabs`, in the per-level closure of
`compute/execute/clearing_2d.rs` (it needs that level's cut depth), and
`optimize_entry_descents_annotated` and `adaptive_feed_modulate`, in
`session/compute.rs` (they need prior stock and engagement). Add no fourth.

## Files

- `mod.rs` — the dressup configuration and the facade. `entry_descent.rs` —
  the ramp and helix emitters. `entry_audit.rs` — the burial audit of a fed
  move against the surface.
- `link.rs` — the swept-corridor test that decides link against retract.
  `arcfit.rs` — linear segments to arcs. `condition.rs` — segment merge:
  dense cut runs into fewer, longer moves.
- `feedopt.rs`, `feed_modulation.rs` — feed-rate optimisation and the
  per-move adaptive modulation. `air_cut.rs` — air-cut classification.
  `tsp.rs` — rapid order. `tests.rs` — the unit tests.

## Invariants

- G-RETRACTDIAL is closed: `RetractStrategy` went on 2026-09-17 (CUT-03).
  The retract COUNT is the lever. Do not add a retract-strategy dial again.
- Every linking arm calls the shared `relink_fragments` kernel in
  `finish/surface_link.rs`. Do not add a second linking implementation.
- Accel-friendly segment-merge conditioning is default-on for roughing. A
  spiral alarm is usually an accel problem, not a syntax problem.
- The feed modulator skips a plunge by geometry, not by intent tag.

## Sentries

- `cargo test -p rs_cam_core -q --test capability_link_moves_safety`
- `cargo test -p rs_cam_core -q --test dressup_span_invariants`
- `cargo test -p rs_cam_core -q --test constrained_max_modulation_f039`
- `cargo test -p rs_cam_core -q --test entry_moves_stock_aware_g_rampterrain`
- `cargo test -p rs_cam_core -q --test lead_in_out_feed_rates_f040`
- `cargo test -p rs_cam_core -q --test plunge_guard_p3`
