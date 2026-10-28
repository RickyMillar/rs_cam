# Step G-D: the Suggest chip, the load-refusal modal, the wording, D-1

Implementer: I4. Date: 2026-10-28. Base: `e46edcc53`. Design:
`gui_fix_design.md` §3, §4, §8 and §0.3 (ruling D-1).

Patch: `stepGD_tree_vs_e46edcc53.patch`, 17 files: 16 changed and the new
`ui/load_refusal_modal.rs`. Checked with `git apply --check` on a clean
`e46edcc53`. It leaves out I3's core work in the tree. It includes the core
doc comment in `feeds/suggest.rs` (§8: the `ContourParallel →
GeodesicAdaptive` promotion; comment only, by the lead's leave).

## What changed

| Item | Files | Change |
|---|---|---|
| §3 Suggest chip | `operations/surface_3d.rs`, `operations/registry.rs`, `toolpath_panel.rs` | The Strategy combo and a ⚡ `SuggestButton::field(ProvKind::RepoRule)` share one grid cell (`ui.horizontal`). Hover: `Suggest: Geodesic Adaptive for this terrain (mixed-terrain classifier). Click to switch.` A click writes `cfg.clearing_strategy` in the draft, so the draft commits it as one `ReplaceToolpathConfig`. The chip shows only when the Suggest records hold `StrategyRecommendedNotApplied { param: "clearing_strategy" }` and the draft does not hold the recommendation yet. `strategy_suggestion` reads the token with serde, so there is no second token table. `OpDrawCtx::strategy_suggestion` carries it. |
| §4 Modal | `ui/load_refusal_modal.rs` (new), `controller.rs`, `app/input.rs`, `app.rs`, `ui/mod.rs`, `components/notice.rs` | `AppController::report_open_error` sends a `VizError::ProjectLoad` to `load_refusal: Option<LoadRefusal { path, error }>`. Every other error stays a toast. `app.rs` draws the modal beside the load-warnings modal, in the same frame: the title `Project not opened`, the file name (the path on hover), a `NoticeStack` of `Role::Danger` rows (one per `RetiredKeyFound`, its `replacement` on hover), the fix line (the shared fix, or `Each line's hover names its fix.`), then `Close` and `Copy`. Another load error gives one row and no fix line. `Notice` gains an optional `detail` (the row hover), so the list stays one line per hit. The only interactive open route is `open_job_from_picked`. The `RS_CAM_JOB` start-up route keeps its log line, and MCP `load_project` does not change. |
| §8 Wording | `mcp_server.rs`, `rs_cam_mcp/src/server.rs`, `toolpath_panel.rs` | `set_machine_kinematics`: "the strategy ranking from `recommend_clearing_strategy`". `set_boundary_config`: "whose regions supply the boundary: its rest analysis, or the pieces a Geodesic Adaptive rough left". `set_rest_analysis_config`: adds "On a Geodesic Adaptive rough the regions are the pieces it left." The `SetMachineKinematicsParam` doc and the `source_toolpath_id` doc follow the design. The `Rest Regions` option hover and the `Rest source` hover take the design's shorter text (the `Rest source` hover goes from 4 lines to 1). |
| D-1 | `toolpath_panel.rs`, `properties/mod.rs` | The `Boundary: …` row is gone. Its text (`boundary_summary_line`) is the hover of the `Source:` combo. The function doc now says so. |
| Wire | `tests/snapshots/mcp_wire_surface.json` | Re-blessed: 2 description rows (the two `rs_cam_mcp` docs). |
| Docs | `FEATURE_CATALOG.md` | The 3D Adaptive row names the ⚡ chip and the modal. |

## Panel line count

| State (Cut tab, Advanced closed) | Before | After |
|---|---|---|
| Body op, boundary on, clean | 23 | 22 (D-1) |
| Rest edge op, worst state (§7.6) | 39 | 38 (D-1; the ⚡ chip takes no row) |

How I got the counts: the IC1 sentry measures the 3D Rough Cut tab at 19
lines before and after the change, because its fixture has the boundary
off. The design measured 23 lines with the boundary on. With the boundary
on, the removed `OneLine` row takes 1 line, so 23 becomes 22. The new lib
test `the_boundary_summary_is_the_source_hover_d1` renders the Cut tab with
the boundary on. It finds `Source:` and no `Boundary:` text.

The chip test renders the Strategy row with the chip at 264 points. The
chip is one `⚡` galley in the combo row, and it adds no row.

`ui04`: the chip hover is on the chip, not on a `p(...)` label, so the
help-key rule does not apply. The `the_help_key_is_the_param_name_ui04`
sentry stays green.

## Tests

New:

- `--lib the_strategy_row_carries_the_suggest_chip_g_d`:
  - `strategy_suggestion` reads the warning, and an empty list gives no
    suggestion.
  - With no advice: no chip.
  - When the draft already holds the advice: no chip.
  - With the advice: one chip, and no write without a click.
  - A click writes `GeodesicAdaptive`.
- `--lib load_refusal_modal::tests`, four tests:
  - Two retired values: two danger rows, the fix on hover, one fix line,
    and `Copy` with four lines.
  - Mixed fixes: the fix line is `Each line's hover names its fix.`
  - Another load error: one row and no fix line.
  - The modal paints the title, the file, the hit, the fix, `Copy` and
    `Close`.
- `--lib a_refused_open_opens_the_modal_not_a_toast_g_d` (controller):
  - A missing file opens the modal with one row and no toast.
  - Any other error stays a toast.
- `--lib the_boundary_summary_is_the_source_hover_d1`.

## Gates (green)

All cargo runs went through `scripts/cargo_lane.sh`, with viz at `-j 2`.

- `cargo test -p rs_cam_viz --lib -- _d1 _g_d properties::tests notice`:
  40 passed.
- `cargo test -p rs_cam_viz --test` on the 14 sentries below. All passed.
  - `mcp_wire_surface_pin`, `mcp_authoring_surface`
  - `the_inspector_text_is_bounded_ic1`, `tabular_data_reads_one_table_ic2`
  - `the_help_key_is_the_param_name_ui04`,
    `boundary_controls_always_visible_g_boundaryinherit`
  - `component_contracts_up2`, `chrome_reads_the_kit_up3`,
    `panels_read_the_token_module_up1`, `ui_string_hygiene`
  - `operations_registry_ui05`, `the_geometry_rows_fit_the_inspector_f22`,
    `inspector_width_is_tab_independent_up4`, `the_inspector_nests_once_dc5`
- `cargo test -p rs_cam_viz --test` on 3 more sentries. All passed.
  - `file_dialogs_run_off_the_frame_f218`
  - `the_dock_states_read_live_state_g_vpstate`
  - `the_toolpath_card_is_five_elements_dc1`
- `cargo test -p rs_cam_mcp`: 36 passed.
- `cargo clippy --no-deps -p rs_cam_viz -p rs_cam_mcp --all-targets -- -D warnings`: clean.
- `cargo fmt -p rs_cam_viz -p rs_cam_mcp -- --check`: clean.

Not run: the full workspace clippy. The lint used `--no-deps` because the
shared tree holds I3's uncommitted core work.
