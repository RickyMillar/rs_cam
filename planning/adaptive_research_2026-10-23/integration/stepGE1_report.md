# Step G-E (1): the header refusal is one word and one line

Implementer: I4. Date: 2026-10-28. Base: `e5614d2b8`. Design:
`gui_fix_design.md` §7.5 and §7.7. Review order: sub-step (1) is the
`Error` arm and the IC1 rule "no text wraps in the toolpath panel".

Patch: `stepGE1_tree_vs_e5614d2b8.patch` (3 files). It does not hold GT2
(`tab_strip.rs`, which still waits for the lead).

## What changed

| File | Change |
|---|---|
| `ui/properties/toolpath_panel.rs` | The `FreshnessState::Error` arm of `draw_header_state` is the state word `Refused` (`REFUSED_STATE`, danger colour), and the full refusal is its hover. Before, `Error: {e}` printed in full and wrapped. A new face line goes under the Generate row: `OneLine::new(e)` in danger, which truncates, with the full text on hover. It shows only when no validation refusal stands (§7.5: a validation refusal answers the settings on screen, so it takes the line). `Waiting on upstream stock` is also a truncating state word now; it wrapped before. |
| `tests/the_inspector_text_is_bounded_ic1.rs` | Rule 1 is stricter: in a toolpath tab (`<op> / <tab>`) no galley wraps, danger text included. The new test `a_generate_refusal_takes_one_line_g_e` puts the D53 refusal (the longest) on the 3D Rough. It asserts the `Refused` word, the face galley with the full text on one row, no wrapped galley, one more visible line than the clean panel, a tab strip that moves down by less than 30 points, and no row wider than the panel. |
| `tests/inspector_header_wraps_g_reachwrap.rs` | The census arm for the two Generate-row statuses now requires a `state_word(` with `.on_hover_text(` and no `.wrap()`. That is the IC1 remedy the file's own doc names. |

The face has no action yet. The rest-boundary faces with their `⟳` action
are sub-step (3).

## Panel line count

| State (rest edge op, Cut tab) | Before | After |
|---|---|---|
| A generate refusal only | clean + 3 to 15 rows (measured in the design: D53 15 rows, "no regions" 7, "not generated" 5, memory 3) | clean + 1 |
| Worst state (§7.6) | 38 (after G-D) | 24 + 1 face line + the blocker line when G-C lands |

The IC1 fixture measures it: the refused 3D Rough Cut tab is one visible
line more than the clean tab, and the tab strip moves down by one line.
Before this step it moved 257 points for the D53 text.

Non-vacuity: with the old arm the test fails on its first assert, because
no `Refused` text is painted. The text was `Error: …`, wrapped to 15 rows.

## Gates (green)

All cargo runs went through `scripts/cargo_lane.sh`, with viz at `-j 2`.

- `cargo test -p rs_cam_viz --test` on 4 sentries. All passed.
  - `inspector_header_wraps_g_reachwrap` (8)
  - `the_inspector_text_is_bounded_ic1` (3, the new test included)
  - `the_inspector_nests_once_dc5`
  - `the_geometry_rows_fit_the_inspector_f22`
- `cargo test -p rs_cam_viz --lib -- properties::tests freshness`:
  47 passed.
- `cargo clippy --no-deps -p rs_cam_viz --all-targets -- -D warnings`:
  clean.
- `cargo fmt -p rs_cam_viz -- --check`: clean.
