# Integration step 7.4c: the cut trace lists each capped move

Date: 2026-10-28. Branch `geodesic-integration`, base `f915d37dd` (7.5a).
The implementer did not commit.

Patch: `step74c_tree_vs_f915d37dd.patch`, md5
`806f4d10b44d2df7834681705c1924b8`. It holds 4 code and test files and
no planning file. `git apply --check -R` passes.

Why: D58 found that `Move::feed_cap` appeared in no output. Lead ruling
(option c): the cap belongs to the move, so the trace carries one table
row per capped move, and `SimulationCutSample` keeps its pinned 280
bytes. A per-sample field was measured first and rejected: 296 bytes
(`Option<f64>`) or 288 bytes (`Option<f32>`) against the S1 pin.

## 1. What changed

| File | Change |
|---|---|
| `stock/simulation_cut.rs` | `SimulationCutTrace::capped_moves: Vec<CappedMove>` (`serde(default, skip_serializing_if = "Vec::is_empty")`); `CappedMove { toolpath_id, move_index, feed_cap_mm_min: f64 }` and `CappedMove::of_toolpath`. The doc names the join key `(toolpath_id, move_index)` |
| `stock/simulation_cut/analysis.rs` | the two trace literals start the table empty |
| `compute/simulate.rs` | `assemble_cut_trace` fills the table once per move, from each simulated entry's `annotated.toolpath` (the toolpath the samples' `move_index` counts in) |
| `tests/a_trace_lists_each_capped_move_s7_4c.rs` (new) | §2 |

## 2. The sentry

`a_trace_lists_each_capped_move_s7_4c` runs `run_simulation` on a plunge,
an uncapped cut, a cut capped at 400 mm/min and a retract.
1. The table is exactly the capped move: `(3, 3, 400.0)`.
2. The audit join by `(toolpath_id, move_index)`: each sample that finds
   a row is a sample of move 3, with feed ≤ cap; no sample of the uncapped
   move finds one; both moves have samples.
3. The wire: the row serialises; a run with no cap writes no
   `capped_moves` key; a trace without the key reads back with an empty
   table.

With the fill removed, the test is red at check 1.

## 3. Surfaces

- `simulation.json` (CLI `--sim-artifact`) and the GUI's in-memory trace
  carry the table.
- MCP `get_cut_trace` returns no per-sample or per-move records (only the
  summaries, hotspots, issues, span summaries and drill samples,
  `rs_cam_viz/src/app/mcp/simulation.rs`), so the table does not reach
  MCP. `mcp_wire_surface_pin` is unchanged (2/2).

## 4. Gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check`, full clippy (heavy-tests, research, test-support) | ok |
| `material_per_segment_s1` (the 280-byte pin, untouched) | 4/4 |
| `memory_budget_core_b1_b2_b3`, `sim_trace_sample_rate_g_simmem`, `sim_peak_memory_g_simmem`, `sample_source_intent_r11` | ok |
| byte parity, opgolden | 6/6, 1/1 |
| viz `-j 2`: `mcp_wire_surface_pin`, `--lib cut_trace` | 2/2, 5/5 |
| `rs_cam_cli` (all) | 0 failed |
