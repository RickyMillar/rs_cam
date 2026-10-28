# Step GFM: the Max Rate row shows without feed optimisation

Implementer: I4. Date: 2026-10-28. Base: `a884902e6` with
`stepGE2_tree_vs_5ca83fb6d.patch` applied. Apply G-E2 first; both patches
touch `ui/properties/tests.rs`. The lead's item: C2 (G-FEEDMAX) makes
`feed_max_rate` cap the feed modulation too, so the GUI must show the field
whenever modulation can apply.

Patch: `stepGFM_tree_vs_a884902e6_plus_GE2.patch` (2 files).

## What changed

| File | Change |
|---|---|
| `ui/properties/linking_dressup.rs` | The Max Rate row (`feed_max_rate`, empty = "machine max") leaves the `cfg.feed_optimization && feed_opt_reason.is_none()` gate. It is the first row of the Linking tab's `Advanced` section and always shows there. The label loses its two-space indent (`Max Rate:`), because it is no longer a child of the checkbox. Ramp Rate stays under the optimiser, because only the optimiser reads it. The hover is unchanged: the core field description, which C2 already rewrote ("caps every feed stage (the optimiser and the feed modulation after a simulation); null = the machine maximum feed"). |
| `ui/properties/tests.rs` | `the_max_rate_row_shows_without_feed_optimization_g_feedmax`. |

"Whenever modulation can apply": the post-simulation modulation runs for
every op with a commanded feed (`session/compute/simulation.rs`, the
candidate loop). The row has no op gate, and every op draws the Linking tab.

## Panel line count

There is no always-visible line. `Advanced` is closed by default. Open, the
section has one more row when feed optimisation is off, and the same count
when it is on (the row moved). The IC1 caps hold (`the_inspector_text_is_bounded_ic1` green).

## Tests and gates (green)

- New `--lib the_max_rate_row_shows_without_feed_optimization_g_feedmax`:
  a Pocket with `feed_optimization = false`. A click opens `Advanced`. The
  test then reads `Feed rate optimization` (non-vacuity), `Max Rate:`, and
  no `Ramp Rate`.
- `--test the_help_key_is_the_param_name_ui04`, `the_inspector_text_is_bounded_ic1`,
  `tabular_data_reads_one_table_ic2`: green.
- `cargo clippy --no-deps -p rs_cam_viz --all-targets -- -D warnings`: clean.
- `cargo fmt -p rs_cam_viz -- --check`: clean.

## Note on the G-E2 patch name

`stepGE2_tree_vs_5ca83fb6d.patch` was cut against the tree at `a884902e6`.
That HEAD came after `5ca83fb6d`; I read the wrong line of the log when I
named the file. The two commits differ in no viz file (`git diff 5ca83fb6d
a884902e6 -- crates/rs_cam_viz` is empty). The patch applies to both.
