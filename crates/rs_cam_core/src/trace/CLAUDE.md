# `trace/` — the records that describe a generated toolpath

The debug trace, the semantic trace, the spans, the narration and the
transform provenance. The agent-facing entry point is `trace::narrate`.

## Files

- `mod.rs` — the facade.
- `debug_trace.rs` — the per-operation debug record.
- `semantic_trace.rs` — the semantic items for operation-level diagnostics.
- `toolpath_spans.rs` — the semantic ranges of moves inside a `Toolpath`.
- `narrate.rs` — agent-friendly narration of a toolpath and a cut trace.
- `transform_provenance.rs` — the post-generation transform provenance.

## Invariants

- A trace with no provenance block is stale. Read the provenance before you
  cite a trace.
- The `narrate` Z ladder is NOMINAL, not achieved. "Did it cut too deep?" is
  answered by the emitted G-code against the mesh, not by this record.
- A span survives a relink. A relink that drops spans is a defect.

## Sentries

- `cargo test -p rs_cam_core -q --test remap_interval_index_c9`
- `cargo test -p rs_cam_core -q --test transform_provenance_fingerprints`
- `cargo test -p rs_cam_core -q --test exporter_span_classifier_x1`
- `cargo test -p rs_cam_core -q --test narrate_regions_closed_c8`

## Do not

- Do not confuse this folder with `ops/trace_path.rs`, the follow-path
  operation.
