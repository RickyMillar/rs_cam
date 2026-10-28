# Step GC4: the "Contour each level" row, and the 7.8 edge op turns it on

Implementer: I4. Date: 2026-10-28. Base: `dbdf8f111` (after step 7.8,
`5b28c9afb`, and C4, `e8072f172`). This patch has two items from the lead.
Step 7.8 was already committed when C4 landed, so the template line is a
small follow-up here and not part of the 7.8 patch.

Patch: `stepGC4_tree_vs_dbdf8f111.patch` (6 files).

## What changed

1. **The template** (`session/op_draft.rs`, `draft_geodesic_pair`): the
   edge op sets `contour_each_level = true`. The source, in the code
   comment and the doc, is G-RESTDOC, D61/D68: this pair is the G-RESTDOC
   case. Op 1 keeps the field default (off). The core sentry
   `the_template_adds_the_channel_pair_s7_8` asserts both. The pair still
   generates with the option on: the edge op holds a result after the
   simulation.
2. **The 3D Rough row "Contour each level:"** (`operations/surface_3d.rs`):
   - It sits in the main parameter grid, directly under `Strategy:`. The
     ruling makes it "one visible 3D Rough option", so it is not in
     Advanced. It belongs beside the strategy, because it changes how the
     rough cuts its walls.
   - The label and the checkbox take the registry help of
     `contour_each_level` as their hover (UI-04: keyed on the parameter
     name). That help is core's `HELP_CONTOUR_EACH_LEVEL`.
   - **The row shows only where it acts**: a rest rough
     (`stock_source = FromRemainingStock`) with its machining boundary on,
     or while the option is on. This is the precedent of the Pocket's "Cut
     past open sides" row. The core help says the option "acts on a rest
     rough (remaining stock) with a machining boundary". A body rough
     (fresh stock) draws no row.
   - `OpDrawCtx::boundary_enabled` carries the draft's boundary state, and
     `draw_adaptive3d_params` takes `contour_each_level_offered`.

## Line count (Cut tab, Advanced closed)

| State | Before | After |
|---|---|---|
| Body rough (fresh stock), clean | 22 | 22 (no row) |
| Rest edge op (remaining stock + boundary), clean | 24 | 25 (the row) |
| Rest edge op, worst state (§7.6) | 26 | 27 |
| An op with the option on | — | the row stays, so the operator can turn it off |

The counts for the rest edge op are derived from the G-C table, +1. The
IC1 fixture's 3D Rough is a fresh-stock body rough, so its counts do not
move (`the_inspector_text_is_bounded_ic1` green).

**What folds away: nothing in this patch.** The ruling asks for a visible
row, and the condition already keeps the row off every op where it would
do nothing. If the lead wants the rest op's count back at 24, there is one
candidate: `Entry Style:` could move into Advanced on a rest rough, where
the entry reads the remaining stock. That is a separate decision; I did not
make it.

## Tests

- New `--lib contour_each_level_shows_where_it_acts_c4`:
  - not offered and off: no row;
  - offered: one row;
  - on but not offered: one row;
  - the hover help names "Depth/Pass".
- `--lib the_strategy_row_carries_the_suggest_chip_g_d`: updated for the
  new parameter.
- Core `--test the_template_adds_the_channel_pair_s7_8`: the edge op has
  C4 on and op 1 has it off; the pair generates.

## Gates (green)

All cargo runs went through `scripts/cargo_lane.sh`, in the foreground.

- `cargo test -p rs_cam_viz --lib`: 608 passed.
- Viz `--test` sentries, all passed:
  - `the_inspector_text_is_bounded_ic1`
  - `the_help_key_is_the_param_name_ui04`
  - `operations_registry_ui05`
  - `the_template_has_one_door_s7_8`
  - `tabular_data_reads_one_table_ic2`
- Core `--test the_template_adds_the_channel_pair_s7_8`: passed.
- `cargo clippy --no-deps -p rs_cam_core -p rs_cam_viz --all-targets` with
  the heavy-tests, research and test-support features: clean.
- `cargo fmt --check`: clean.
