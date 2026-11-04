# `diagnostics/` — the unified diagnostic model

One typed finding model for every upstream source. `diagnostics::diagnose` turns project state into a finding list.

## Files
- `mod.rs` — the `Diagnostic` model and the facade. `diagnose.rs` — the orchestrator.
- `ids.rs` — the stable rule identifiers. `evidence.rs`, `fix.rs` — the evidence and auto-fix payloads.
- `supersession.rs` — the reducer that drops a pre-simulation hint for a measured answer.
- `presentation.rs` — W1: `TABLE`, THE map from an id to its `Anchor` and its default `face`.
- `adapters/` — one adapter per source: feeds, generation findings, model references,
  preconditions, project diagnostics, stale defaults, static checks, tool load.
- `tests.rs` — the unit tests.

## Invariants

- `None` and `0.0` are different for a measured finding. `None` means not
  measured. `Some(0.0)` means measured clean. Preserve that distinction on a
  new field and in every adapter.
- A new rule needs an identifier in `ids.rs`. Do not raise a finding with an
  ad-hoc string. Write it as a row of `diag_ids!`: it emits the const and its `ALL` entry (F162).
- K6 (`consequence.rs`): each row ends `=> <column>`: `warn(Danger::X)`, `INFO` or `NOT_READY` (a
  missing input, ruling Q10); a row with none does not compile. The macro writes `CONSEQUENCES`.
  `Consequence` has no refusing arm (ruling Q6): no finding refuses generation or export. `Gate` decides
  whether an export raises a finding (`ProjectSession::export_warnings` walks `Gate::ALL`). The `emit.*`
  ids (`gcode::EmitError::id`) are not rows. K5 (P7): the three `tool.*` rule ids (`physical_rules!`) reach the list through the `from_generation` feasibility adapter and `Gate::Feasibility`. Sentry: `a_job_with_each_finding_exports_and_names_it_k6`.
- An adapter converts. It does not compute a verdict. The verdict belongs to
  the source layer. Example: the tool-kind finding reads
  `OperationType::tool_refusal` (W2.2); it is `Blocking` only where the generator cannot compute
  (`tool_cannot_compute`, K6 E1). S11: `precondition.not_set` (Blocking) states the stock thickness, floor, depth or feeds text that generation gives. R2: `precondition.no_tool` (Blocking) states `no_tool_state` and `CHOOSE_A_TOOL_FIX`; an op with no tool gets only the precondition and model-reference findings (W2b: THE list `diagnose_toolpath_with_no_tool`, the session and the GUI inspector call it). F033: the plane-order findings convert `compute::config::height_order`; `height_order_severity` is the one severity (the GUI badge reads it).
- W1: every `Diagnostic` has a `face` (at most 60 chars, plain words, no derivation; `message`
  keeps the detail) and an `anchor` (`Field { key }`, `Tool`, `Operation`, `Note` = Info/Hint at
  every site, `Project`, `View`). A new id needs a `TABLE` row. A site fills both from
  `presentation::face`/`anchor`, or builds its face with `face_or_default`. `fit_anchor` turns a
  field the op does not publish into `Operation`; the orchestrator calls it.

## Sentries

`cargo test -p rs_cam_core -q --test <name>`: `depth_beyond_stock_core_g_depthstockcore`,
`chipload_abstention_cannot_supersede_g_chipgate`, `drill_evidence_wording_d3`,
`region_cap_honesty_f3`, `gate_population_vacuity_xvac`, `a_job_with_each_finding_exports_and_names_it_k6`. `--lib`:
`the_static_check_and_the_generator_refuse_the_same_tools`, `_w1` (W1: a row, a short face and a
real field for each id).

## Do not

- Do not let a heuristic hint survive a measured answer. Use the supersession reducer.
