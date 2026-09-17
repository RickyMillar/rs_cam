# `tool_load/` — the cutting-load guardrails

Independent gates over a simulated cut. The entry point is
`tool_load::evaluate_toolpath`.

## Files

- `mod.rs` — the monitor and the report entry point.
- `chipload.rs`, `power.rs`, `deflection.rs`, `plunge_stress.rs` — the four
  milling gates.
- `drill_gates.rs` — chip welding, peck adequacy, plunge feed.
- `locality.rs` — the locality classifier and the steady-state predicate.
- `boundary.rs`, `verdict.rs`, `display.rs` — the gate boundary contract, the
  verdict types, the typed display quantities.
- `optimize/` and its children — the optimiser: search space and axes,
  candidate generation and ranking, strategies, retargeters, the pre-flight
  gate, the outcome narrative.

## Invariants

- Every gate filters `in_transit_span` samples through the one predicate
  `locality::is_steady_state_for_gate`. Never write a second copy.
- The chipload gate quantity is advance per tooth, `effective_feed / (rpm *
  flutes)`. It is not the dexel chip thickness.
- A gate with no population proved nothing. Check the sample count before you
  read `Within` as evidence.
- The drill gates model the R-plane-rooted emitted schedule and read cutting
  geometry, not fed distance. On a tapered drill they use the envelope
  diameter; do not exonerate a tapered drill on these gates alone.

## Sentries

- `cargo test -p rs_cam_core -q --test gate_population_vacuity_xvac`
- `cargo test -p rs_cam_core -q --test chipload_boundary_g_chip_ulp`
- `cargo test -p rs_cam_core -q --test predicted_feed_gates_f035`
- `cargo test -p rs_cam_core -q --test drill_evidence_wording_d3`

## Do not

- A Kc or factor change needs the slow `--test` sims, not `--lib` alone.
