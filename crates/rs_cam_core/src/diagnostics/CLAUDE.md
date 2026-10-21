# `diagnostics/` — the unified diagnostic model

One typed finding model for every upstream source. The entry point is
`diagnostics::diagnose`, which turns project state into a finding list.

## Files

- `mod.rs` — the `Diagnostic` model and the facade.
- `diagnose.rs` — the orchestrator.
- `ids.rs` — the stable rule identifiers.
- `evidence.rs`, `fix.rs` — the evidence payloads and the auto-fix payloads.
- `supersession.rs` — the reducer that drops a pre-simulation hint for a measured answer.
- `adapters/` — one adapter per source: feeds, generation findings, model references,
  preconditions, project diagnostics, stale defaults, static checks, tool load.
- `tests.rs` — the unit tests.

## Invariants

- `None` and `0.0` are different for a measured finding. `None` means not
  measured. `Some(0.0)` means measured clean. Preserve that distinction on a
  new field and in every adapter.
- A new rule needs an identifier in `ids.rs`. Do not raise a finding with an
  ad-hoc string. Write it as a row of `diag_ids!`: it emits the const and its `ALL` entry (F162).
- An adapter converts. It does not compute a verdict. The verdict belongs to
  the source layer. Example: the tool-kind finding reads
  `OperationType::tool_refusal`, the generator's own refusal (W2.2). F033: the plane-order findings convert `compute::config::height_order`; `height_order_severity` is the one severity (the GUI badge reads it).

## Sentries

- `cargo test -p rs_cam_core -q --test depth_beyond_stock_core_g_depthstockcore`
- `cargo test -p rs_cam_core -q --test chipload_abstention_cannot_supersede_g_chipgate`
- `cargo test -p rs_cam_core -q --test drill_evidence_wording_d3`
- `cargo test -p rs_cam_core -q --test region_cap_honesty_f3`
- `cargo test -p rs_cam_core -q --test gate_population_vacuity_xvac`
- `cargo test -p rs_cam_core -q --lib the_static_check_and_the_generator_refuse_the_same_tools`

## Do not

- Do not let a heuristic hint survive a measured answer. Use the supersession
  reducer.
