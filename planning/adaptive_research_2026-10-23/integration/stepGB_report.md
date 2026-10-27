# Step G-B: feed caps are visible

Implementer: I4. Date: 2026-10-28. Base: `de1696763` (worktree
`geodesic-integration`). Design: `gui_fix_design.md` §1, with the review
change (the dash in every colour mode).

Patch: `stepGB_tree_vs_de1696763.patch` (applies to a clean `de1696763`;
checked with `git apply --check` in a temporary worktree). The patch holds
only the G-B hunks. The tree also holds I3's core work and I3's
`FEATURE_CATALOG.md` hunks; the patch leaves them out.

## What changed

| Part | File | Change |
|---|---|---|
| a. Dash | `render/toolpath_render.rs` | `ToolpathLines::walk` (the one walk that `normal`, `engagement` and `advance_per_tooth` call) dashes the cut segments of a move with `feed_cap.is_some()`. It uses `push_dashed_segment`, in the colour of each segment's first vertex. The dash is in every `ToolpathColorMode`. |
| b. Legend | `ui/overlays/legend_rail.rs` | One entry `dashed: fed at a load cap`, with no swatch, on the three move legends: `Moves` (Normal mode, when cutting moves show), the Engagement ramp tail, and the Advance / tooth entries. The entry is always in the key; it does not scan the moves each frame. |
| c. Hover | `ui/properties/toolpath_panel.rs` | `HeaderRow::caps` (`CapFigures`: count and lowest cap of the stored toolpath). The move count's hover and `figures_hover` add the line `N moves at a load cap (planned), lowest X mm/min`. There is no line when the toolpath has no cap. |
| d. MCP | `app/mcp/simulation.rs`, `rs_cam_mcp/src/{response,server}.rs`, `mcp_server.rs` | `get_cut_trace` gives `capped_move_count` and `capped_min_feed_mm_min` (null when 0) always, and the bounded `capped_moves` rows (`insert_capped`) only when a row exists. The `toolpath_id` filter applies. New request field `max_capped_moves` (default `DEFAULT_MAX_CAPPED_MOVES` = `DEFAULT_MAX_ISSUES`, 50), held in `CutTraceCaps::capped_moves`. New `build_info` feature token `cut_trace_capped_moves`. The tool description names the keys. |
| e. Comment | `rs_cam_core/src/tool_load/verdict.rs` | `FeedCap` doc: "Geodesic Adaptive writes it on tight turns and plunge departures (integration step 7.4a)." Comment only, by the lead's leave. |
| Docs | `FEATURE_CATALOG.md` | One sentence on the Geodesic Adaptive row. |
| Wire | `tests/snapshots/mcp_wire_surface.json` | Re-blessed: only the `max_capped_moves` schema row (9 lines). |

## Panel line count

G-B adds no panel line. Every signal is a line style, a legend entry or a
hover.

| State (Cut tab, Advanced closed) | Before | After |
|---|---|---|
| Body op, clean | 23 | 23 |
| Rest edge op, worst state (§7.6) | 39 | 39 |

The figures hover gains one line when the result has a capped move. The
`the_inspector_text_is_bounded_ic1` sentry stays green.

## Tests

New:

- `--lib a_capped_move_draws_dashed_in_every_colour_mode_g_b`: in Normal,
  Engagement and Advance / tooth, the uncapped move draws 2 vertices and the
  capped move draws 4 (the two dashes at 0.35 and 0.65).
- `--lib the_cut_trace_reports_the_capped_moves_g_b`: no caps gives count 0,
  null lowest cap and no `capped_moves`; four caps give count 4, lowest 300,
  2 rows at a cap of 2 with `capped_moves_truncated`; the `toolpath_id`
  filter gives 3 and 600.
- `--lib the_figures_hover_names_the_feed_caps_g_b`: `CapFigures::of` and the
  hover text.

## Gates (all green)

All cargo runs went through `scripts/cargo_lane.sh`, with viz at `-j 2`.

- `cargo test -p rs_cam_viz --lib -- app::mcp:: legend toolpath_render properties::tests`: 84 passed.
- `cargo test -p rs_cam_viz --test` `mcp_wire_surface_pin`,
  `the_dock_states_read_live_state_g_vpstate`, `overlays_registry`,
  `render_pipelines_headless_g_pipesmoke`, `an_arc_move_draws_on_its_arc_r_arc`,
  `viewport_draws_selected_only_wp27`, `the_selection_colours_live_in_colors_p4_4`,
  `panels_read_the_token_module_up1`, `the_inspector_text_is_bounded_ic1`,
  `mcp_authoring_surface`: all passed.
- `cargo test -p rs_cam_mcp`: 36 passed (the `build_info` freeze included).
- `cargo clippy --no-deps -p rs_cam_viz -p rs_cam_mcp --all-targets -- -D warnings`: clean.
- `cargo fmt -p rs_cam_viz -p rs_cam_mcp -p rs_cam_core -- --check`: clean.

Not run: the full workspace clippy. Core does not pass clippy in the shared
tree now, because of I3's work in progress (`generation.rs:1037`,
`needless_pass_by_value`). For this reason the lint ran with `--no-deps`.
Run the full clippy after I3 lands.
