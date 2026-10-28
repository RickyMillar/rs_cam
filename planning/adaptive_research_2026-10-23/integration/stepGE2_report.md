# Step G-E (2): the footer findings are a bounded list

Implementer: I4. Date: 2026-10-28. Base: `5ca83fb6d` (G-E1). Design:
`gui_fix_design.md` §7.2 rule 4 and §7.7, with the review change: the
remainder line is the `NoticeStack` form `Showing 3 of N · Show all`,
never `+N more`.

GT2 is reverted from the tree, as the lead ruled. I reverse-applied
`stepGT2_tree_vs_5b4a561ae.patch`; `tab_strip.rs` now equals HEAD. The patch
file stays as the record.

Patch: `stepGE2_tree_vs_5ca83fb6d.patch` (3 files).

## What changed

| File | Change |
|---|---|
| `ui/components/notice.rs` | The remainder wording is one function, `overflow_line(shown, total)`, and its clickable row is `overflow_row(ui, shown, total)`. `Resolved::overflow_text` and `NoticeStack::show` call them, so the toolpath footer and every `NoticeStack` print one wording. |
| `ui/properties/toolpath_panel.rs` | The footer finding rows (actionable, then the merged gate rows, then the other stateful rows, in today's order) go through `bounded_findings`. It keeps the first `FOOTER_FINDINGS_CAP` (3), plus every `Critical` or `Blocking` row however far down (the `NoticeStack` safety rule). When rows are left out, `overflow_row` draws `Showing 3 of N · Show all`, and a click shows them all. The flag is egui temp data keyed on the toolpath id, as the load-warnings modal keeps its flag. The `N hints` disclosure does not change. |
| `ui/properties/tests.rs` | `the_footer_findings_are_bounded_g_e`. |

The findings keep their renderer (`render_diagnostic_row`, with the Fix
button). Only the count of rows drawn changes.

## Panel line count

| State | Before | After |
|---|---|---|
| N actionable or stateful findings, none critical | N lines | min(N, 3) + 1 when N > 3 |
| A rough with five findings (the design's example) | 5 | 4 (3 + the remainder line) |
| Three or fewer findings | unchanged | unchanged |

The IC1 fixture holds three or fewer footer findings, so its counts do not
change.

## Tests

- New `--lib the_footer_findings_are_bounded_g_e`:
  - Five caution rows give [0, 1, 2].
  - Expanded, they give all five.
  - The remainder line reads `Showing 3 of 5 · Show all`.
  - A blocking fifth row gives [0, 1, 2, 4].
  - Two rows give both.

## Gates

All cargo runs went through `scripts/cargo_lane.sh`, with viz at `-j 2`.

- `cargo test -p rs_cam_viz --lib`: 600 passed, 2 ignored.
- `--test component_contracts_up2` (`NoticeStack`), `the_inspector_text_is_bounded_ic1`,
  `inspector_header_wraps_g_reachwrap`, `the_inspector_nests_once_dc5`,
  `tabular_data_reads_one_table_ic2`: green.
- The 19 integration tests that render the inspector (`properties::draw`):
  18 green. One arm fails, and it failed before this step:
  - The arm is `the_gui_can_set_a_feed_g_fscontrol` /
    `the_spindle_override_shows_the_project_default_g_fscontrol`.
  - It looks for `PrecedenceField::new("Spindle:"` in the properties
    source. That call left the source in commit `74af0f8e1` (S11, the
    underived-default removal), and no file has it now.
  - It does not come from G-E or any GUI-fix step. A fix belongs to the
    owner of the feeds panel.
- `cargo clippy --no-deps -p rs_cam_viz --all-targets -- -D warnings`: clean.
- `cargo fmt -p rs_cam_viz -- --check`: clean.
