# Step G-E (4): "Feeds unavailable" and the header validation errors are one line

Implementer: I4. Date: 2026-10-28. Base: `b8aee0bf2` (G-E3). Design:
`gui_fix_design.md` §7.2 rule 5 and §7.7 rows 4 and 6.

Patch: `stepGE4_tree_vs_b8aee0bf2.patch` (4 files, viz only). The tree
also holds another agent's uncommitted viz edits. These are the Waterline
`helical` rename in `operations/surface_3d.rs`, and `mcp_server.rs`. The
patch leaves them out.

## What changed

| File | Change |
|---|---|
| `ui/properties/toolpath_panel.rs` | The header validation errors are ONE line. When the Geometry tab owns a refusal, the line is `Refused · see Geometry` (as before). Else the line is the first validator sentence, truncated. Its hover holds every validation error, one per line. Before, each other sentence was its own wrapped row (`wrapped_small_label`), up to 3 rows each, and the tab strip moved down. The import of `wrapped_small_label` goes; the Geometry tab's selection section still uses it, which is out of G-E scope (§7.7 last paragraph). |
| `ui/properties/feeds_speeds.rs` | `Feeds unavailable: {e}` is a `OneLine` in danger. It truncates, and the hover holds the full text. Before, it was a `colored_label` that wrapped with `e`. |
| `tests/inspector_header_wraps_g_reachwrap.rs` | The arm `the_validation_errors_are_constructed_wrapped` becomes `the_validation_errors_are_one_line`. It requires `OneLine::new(face)` with `.detail(detail)`, and no `for err in other {` loop that draws a row per sentence. |
| `tests/the_inspector_text_is_bounded_ic1.rs` | New `a_validation_error_takes_one_line_g_e`. `Command::SetToolpathModel` binds the 3D Rough to the 2D model, so the header has `Selected model has no 3D mesh`. The test asserts that the line paints (non-vacuity), that no text wraps, and that the tab strip moves down by less than 30 points. |

## Panel line count

| State | Before | After |
|---|---|---|
| One header validation error | 1 to 3 rows (measured in the design: 3 rows at 264 points for the longest) | 1 |
| N validation errors | the sum of their rows | 1 |
| A Geometry refusal and another error | 1 + 1 to 3 | 1 |
| Feeds unavailable | wraps with the text of `e` | 1 |

With G-E (1) to (4), above the tab strip there are now only the title,
`Name:`, the Generate row, and at most one refusal line (§7.2 rule 5). The
G-E1 face line shows only when no validation refusal stands.

## Gates (green)

All cargo runs went through `scripts/cargo_lane.sh`, with viz at `-j 2`.

- `cargo test -p rs_cam_viz --lib`: 602 passed, 2 ignored.
- `cargo test -p rs_cam_viz --test` on 6 sentries. All passed.
  - `the_inspector_text_is_bounded_ic1` (5)
  - `inspector_header_wraps_g_reachwrap` (8)
  - `the_panel_feeds_run_once_per_input_f216`
  - `the_inspector_nests_once_dc5`
  - `tabular_data_reads_one_table_ic2`
  - `the_geometry_rows_fit_the_inspector_f22`
- `cargo clippy --no-deps -p rs_cam_viz --all-targets -- -D warnings`: clean.
- `cargo fmt -p rs_cam_viz -- --check`: clean.
