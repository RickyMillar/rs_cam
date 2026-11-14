# `trace/` — the records that describe a generated toolpath

The debug trace, the semantic trace, the spans, the narration and the
transform provenance. The agent-facing entry point is `trace::narrate`.

## Files

- `mod.rs` — the facade.
- `debug_trace.rs` — the per-operation debug record. F017: an emitter names a `DebugSpanKind` (`as_key`, `help`, `structural`, exhaustive), never a string.
- `semantic_trace.rs` — the semantic items for operation-level diagnostics.
- `toolpath_spans.rs` — the semantic ranges of moves inside a `Toolpath`. GUI span text: `Span::display_label`, `SpanKind::display_label` (F074); `SpanKind::label` is the MCP name.
- `produced.rs` — `Produced`, the typed side channel of a generated toolpath (W6.2). K4: `Produced::entries` holds one `EntryRecord` per entry; `JobPhases::run_stages` builds it once, `translated` shifts it, and no stage that writes moves reads it.
- `lineage.rs` — K8 `Lineage` (P5b): the cut, the pick, the stored speeds with their stamps, the tool facts, the entry contract and entries, the generation findings, the K5 rule rows (`rules`, P7); built in `JobPhases::finish` from stored values (no Suggest run), a required field of `ToolpathComputeResult` (no `Default`; `unrecorded()` is a test door). MCP `get_toolpath_lineage`, CLI `summary.json` `lineage`; K7 `MeasuredLineage::row` (P29-10 item 11: the worst outside sample as one `CheckRow`, `binding` and `judged` deleted). Sentry `--test every_generated_toolpath_has_a_lineage_k8`. P13b: `StepRecord` (step, `StepQuantity`, value in, factor, value out, `BasisRecord` = the owned `feeds::Basis`, same JSON); THE store `FeedsProvenance::steps` = `StepRecords` (`Arc`, a producer replaces, never appends; file form C `StepTable`: interned `BasisNode`s by index); `Lineage::steps` = the view `current_steps` (every chain, plus a `stated_by_operator` / `later_write` record where a later write replaced the value); `Lineage::stored` (no result: MCP, CLI `suggest`, GUI). Producers: `feeds::calculator::calculate_with` (`StepRecording`: Apply path and the pick point), `feeds/suggest/apply.rs` (`funnel_steps`, `pick_step`, `with_explored_speeds`), `compute::geometry_default::write_rule_values`. `unrecorded()` is behind `test-support` (L5). Sentries `--test every_calculator_step_leaves_one_record_p13b`, `--test the_step_records_are_stored_and_bounded_p13b`, viz `the_step_records_reach_every_surface_p13b`; instrument `--test the_stored_step_records_stay_small_p13b -- --ignored`.
- `narrate.rs` — agent-friendly narration of a toolpath and a cut trace.
- `transform_provenance.rs` — the post-generation transform provenance.

## Invariants

- A trace with no provenance block is stale. Read the provenance before you
  cite a trace.
- The `narrate` Z ladder is NOMINAL, not achieved. "Did it cut too deep?" is
  answered by the emitted G-code against the mesh, not by this record.
- A span survives a relink. A relink that drops spans is a defect.
- W2.5: the narration prints the feed stage of the move list it describes
  (`ToolpathNarrationContext::feed_stage`). The GUI narrates the worker copy.
- The narration reads an op's DOC basis, air-cut hint, air-cut bar, relink dial,
  `plans_bands` and `ramp_descent` from `OperationType::capabilities()` (W2.4, C2).
  Do not add a `match` on the op type here. P29-11 11-6: the peak-depth line
  has no depth threshold; its severity is the worst depth row (`DepthRows`, a
  required argument, from `ProjectSession::narration_depth_rows`).

## Sentries

- `cargo test -p rs_cam_core -q --test remap_interval_index_c9`
- `cargo test -p rs_cam_core -q --test transform_provenance_fingerprints`
- `cargo test -p rs_cam_core -q --test exporter_span_classifier_x1`
- `cargo test -p rs_cam_core -q --test narrate_regions_closed_c8`

Do not confuse this folder with `ops/trace_path.rs`, the follow-path operation.
