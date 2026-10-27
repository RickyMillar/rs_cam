# Step G-B2: the capped-move legend entry shows only with a cap

Implementer: I4. Date: 2026-10-28. Base: `b8bb3a41d` (G-B). The lead's
follow-up to G-B: the entry `dashed: fed at a load cap` showed in the
legend when no move was capped.

Patch: `stepGB2_tree_vs_b8bb3a41d.patch`.

## What changed

| File | Change |
|---|---|
| `state/runtime.rs` | `CapFigures` moves here from `toolpath_panel.rs`. `ToolpathRuntime` gets `cap_memo: CapMemo` and `caps()`. The memo counts the capped moves once per result. Its key is a `Weak` of the result's `AnnotatedToolpath`, so a new result recounts and an old address is not reused. A frame reads through `&AppState`, so the memo writes through a `Mutex`. |
| `ui/overlays/legend_rail.rs` | `capped_moves_drawn(state)`: a drawn toolpath has `caps()`. The `Moves` entry, the Engagement tail and the Advance / tooth entry show only then (`with_capped_move_entry` in `line_key`). New `key_labels(state, line)`, the labels of a key, for the test. |
| `ui/properties/mod.rs` | `ToolpathPanelInputs::caps`, from `ToolpathRuntime::caps`. |
| `ui/properties/toolpath_panel.rs` | `HeaderRow::caps` reads `inputs.caps`. The panel no longer scans the moves each frame. |

## Panel and legend lines

No panel line changes (23 and 39, as G-B). The legend loses one entry on
every toolpath with no capped move.

## Tests

- New `--test the_dock_states_read_live_state_g_vpstate`
  `the_capped_move_entry_shows_only_with_a_cap_g_b2`: in palette and
  Engagement modes, no line of the rail carries the entry with no cap; after
  one cut move gets a cap, the entry shows. A non-vacuity check asserts a
  move legend is drawn.
- `--lib the_figures_hover_names_the_feed_caps_g_b` now also covers the
  memo: no result gives `None`, the second read gives the same value, and a
  new result with no cap gives `None`.

## Gates (green)

- `cargo test -p rs_cam_viz --test the_dock_states_read_live_state_g_vpstate --test the_inspector_text_is_bounded_ic1`: 21 and 2 passed.
- `cargo test -p rs_cam_viz --lib -- the_figures_hover legend properties::tests`: 24 passed.
- `cargo clippy --no-deps -p rs_cam_viz --all-targets -- -D warnings`: clean.
- `cargo fmt -p rs_cam_viz -- --check`: clean.
