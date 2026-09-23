# `tool_load/` — the cutting-load guardrails

Independent gates over a simulated cut. The entry point is
`tool_load::evaluate_toolpath`.

## Files

- `mod.rs` — the monitor and the report entry point.
- `chipload.rs`, `power.rs`, `deflection.rs`, `depth.rs`, `plunge_stress.rs` —
  the milling gates; `drill_gates.rs` — chip welding, peck, plunge feed.
- `locality.rs`, `boundary.rs`, `verdict.rs`, `display.rs` — the steady-state
  predicate, the boundary contract, the verdict types, the display quantities.
- `distribution.rs`, `metric_guide.rs` — histograms of each gate's own
  population (call the gate filters; never copy them), and the hover copy.
- `optimize/` — the optimiser: search space, candidates and ranking,
  strategies, retargeters, the pre-flight gate, the outcome narrative.

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
Run one with `cargo test -p rs_cam_core -q --test <name>`: `gate_population_vacuity_xvac`,
`chipload_boundary_g_chip_ulp`, `predicted_feed_gates_f035`, `drill_evidence_wording_d3`,
`an_absent_limit_is_visibly_absent_g_gantry` (the gantry-push row is `Unmodeled`, never a reading, never
a prompt), `a_criterion_carries_its_own_bound_g_s4bound` and `a_weak_bound_cannot_refuse_an_export_g_s4weak`
(every row states its bound and its `BoundSource`; only a sourced bound may refuse an export),
`the_depth_that_cut_is_a_measured_load_g_s3depth` (depth is a post-sim row; it exceeds, it never refuses), `finish_depth_is_reported_not_capped_fm6`
(a finishing depth is `Reported`, no bound), `histogram_population_is_the_gate_population_g_cuthist` (histogram max and count match the gate).

## Do not
- A Kc or factor change needs the slow `--test` sims, not `--lib` alone.
