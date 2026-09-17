# `dressup/` — post-generation transforms

The passes that run after an operation generates a toolpath. They are driven
from `compute/execute/dressup_apply.rs`, never called ad hoc.

## Files

- `mod.rs` — the dressup configuration and the facade.
- `entry_descent.rs` — the ramp and helix emitters.
- `entry_audit.rs` — the burial audit of a fed move against the surface.
- `link.rs` — the swept-corridor test that decides link against retract.
- `arcfit.rs` — linear segments to G2 and G3 arcs.
- `condition.rs` — segment merge: dense cut runs into fewer, longer moves.
- `feedopt.rs`, `feed_modulation.rs` — feed-rate optimisation and the per-move
  adaptive modulation.
- `air_cut.rs` — air-cut sample classification.
- `tsp.rs` — rapid-order optimisation.
- `tests.rs` — the unit tests.

## Invariants

- The retract-strategy dial is dead (G-RETRACTDIAL). The retract count is the
  lever, not the dial. Do not build a feature on the dial.
- Every linking arm calls the shared `relink_fragments` kernel in
  `finish/surface_link.rs`. Do not add a second linking implementation.
- Accel-friendly segment-merge conditioning is default-on for roughing. A
  spiral alarm is usually an accel problem, not a syntax problem.
- The feed modulator skips a plunge by geometry, not by intent tag.

## Sentries

- `cargo test -p rs_cam_core -q --test capability_link_moves_safety`
- `cargo test -p rs_cam_core -q --test constrained_max_modulation_f039`
- `cargo test -p rs_cam_core -q --test entry_moves_stock_aware_g_rampterrain`
- `cargo test -p rs_cam_core -q --test lead_in_out_feed_rates_f040`
- `cargo test -p rs_cam_core -q --test plunge_guard_p3`
