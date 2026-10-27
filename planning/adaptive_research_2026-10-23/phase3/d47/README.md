# D47: G-RESTSTALE, the export gate judged an operation the program does not emit

Tree: worktree `adaptive-head`, base `bd24814c7` (core `src/` is identical to `7bf7738a1`).
Patch: `tree_vs_bd24814c7.patch`, md5 `cb99509b1b4bf05233bba38329e8552b`. Not committed.

## Cause

The D44 consumer does not cause the refusal. The hypothesis "the consumer regenerates after the simulation" is refuted.

- Each D45 refusal (`fix31/*/stderr.txt`) names one stale row only: `toolpath 4`. That is "Waterline 3", which `run_fix31.sh` skips with `--skip 4`. Op 5, the consumer, is not in the rows.
- The fix27 runs have no consumer, and they also refuse with the same row (`fix27/s2p1/stderr.txt:55`). The override flag hid this.
- 52 earlier runs refuse with the same message. On Corne the row is always toolpath 4 (`--skip 4`). On wanaka200 it is toolpath 5, which is in setup 2, and `--setup 0` leaves it out.

The chain:

1. A skipped operation is enabled but has no result. The simulation does not carve it, and the program does not emit it.
2. All line numbers in this list are of the base tree. `ProjectSession::build_tool_load_report` (`crates/rs_cam_core/src/session/compute/export.rs:772`) makes one verdict for each enabled operation, emitted or not.
3. `sim_trace_is_fresh_for` (`export.rs:741-743`) reads "no result" as not fresh. The report then marks that row `StaleSimulation` (`export.rs:823-825`).
4. `export_program` handed the whole report to `emit_checked` (`export.rs:93` at the base). `enforce_unmodeled_policy` (`crates/rs_cam_core/src/gcode/mod.rs:1004-1010`) then refused the whole export as stale, for an operation that is not in the program.

## Fix

`ProjectSession::export_program` (`export.rs`) keeps only the report rows of the toolpaths in `request.emitted` before the load gate. The gate now judges what the program cuts. This is the one checked door for the CLI, the GUI and MCP.

- The gate is not weaker for an emitted operation. An emitted operation with an old trace still refuses as `StaleSimulation`. Part 3 of the sentry shows this.
- The freshness rule and the report are unchanged, so the cards and `get_tool_load_report` are unchanged.
- The patch adds no numeric constant. Programs that the gate does not refuse stay byte-identical: the change only removes rows from the gate input.

## Sentry

`crates/rs_cam_core/tests/a_skipped_op_does_not_stale_the_export_d47.rs`, test `a_skipped_op_does_not_stale_the_export`. It uses the D44 channel pair (a rough with rest analysis on, and a CP rough on `DerivedRestRegions` of the rough) and a third enabled rough. It needs no research arm.

1. Generate the pair, simulate with the third op in `skip_ids`, and export with no override. The export succeeds, and the program holds the pair and not the skipped op.
2. Generate all three, simulate, and export with no override. The export succeeds, so the consumer does not stale the simulation.
3. Edit the consumer after the simulation, regenerate it, and export with the old trace. The export refuses as `StaleSimulation`, and every row names the edited op.

Red on the `7bf7738a1` export code (`sentry_red_on_7bf7738a1.txt`): part 1 refuses with "cached simulation is stale" on `toolpath 2`, the skipped op. Green with the patch, both plain and with `--features research`.

## Gates (all through `scripts/cargo_lane.sh`)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | ok |
| clippy, full features with `research` | ok |
| clippy, the same features without `research` | ok |
| `adaptive3d_emission_byte_parity`, plain and `--features research` | 5/5 and 5/5 |
| `a_skipped_op_does_not_stale_the_export_d47`, plain and research | 1/1 and 1/1 |
| `a_rough_publishes_the_regions_it_leaves_d44` (arm env, `--ignored`) | 1/1 |
| `a_derived_rest_rough_cuts_only_the_leftovers_d44` (arm env, `--ignored`) | 1/1 |
| `one_export_door_runs_every_check_f067`, `an_unmodelled_refusal_names_every_row_g_t19`, `adaptive_feed_modulation_pipeline_f036b`, `gcode_validator_reads_the_emission_frame_r11` | 4, 10, 10, 4 ok |
| `rs_cam_core --lib` | 2698 ok, 13 ignored |
| `rs_cam_cli` | all ok |
| `rs_cam_viz -j 2 --no-fail-fast` | 1329 ok, 7 failed. All 7 also fail with the base export code (pre-existing, not export) |

The 7 viz failures that existed before this change:
- `an_unbacked_cell_still_adds_its_operation_fm5`
- `the_mcp_add_reply_carries_the_refusal_fm5`
- `both_creation_doors_call_the_shared_constructor`
- `every_justified_discard_still_names_a_real_site`
- `no_production_site_discards_an_effects_answer`
- `generate_all_without_a_chain_submits_only_enabled_ops`
- `the_spindle_override_shows_the_project_default_g_fscontrol`

## For the lead

- The DEFECTS row G-RESTSTALE has the wrong cause. The row is not edited here.
- The D45/fix27 runs do not need a re-run for this defect. Their programs did not hold toolpath 4, and the refusal was about toolpath 4 only.
