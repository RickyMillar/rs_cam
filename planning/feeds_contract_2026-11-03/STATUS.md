# Feeds contract: status

Start here in a new session:

1. Read `PLAN.md` section 0 (the operator rulings) and section 10 (the terms).
2. Find the first phase below that is not DONE.
3. Read its card in `phases/`. The card is the full hand-off.
4. Update this file in the same commit as the phase.

Citations in the cards are `path:line@40cf411a7` plus a symbol. Find the
code by the symbol when the line has moved.

## Open decisions

| # | Decision | Blocks | State |
|---|---|---|---|
| D1 | Q6: do the refusals that exist today stay? **Ruled 2026-11-03: no. The program never refuses; every finding warns.** | P2 | RULED |
| D2 | K2b envelope plot sketch (`phases/P5b.md`). | P5b | not asked |
| D3 | P0 D1 pin tool. **Ruled 2026-11-03: a longer-flute pin tool, the one the GUI pin template picks (`session/pins.rs`).** | P0 item 3 | RULED |
| D4 | P0 D2 drill dwell. **Ruled 2026-11-03: delete `dwell_time`, add a retired-key row, migrate the 52 files by script; G82 keeps its fixed dwell.** | P0 item 4 | RULED |
| D6 | P6 D3 tool library. **Ruled 2026-11-03: yes, the script migrates `~/.config/rs_cam/tools/` and keeps a `.bak` of each file.** | P6 | RULED |
| D7 | Missing input. **Ruled 2026-11-03: a missing input stops generation or emission with a reason; only findings warn.** | P2, P5 | RULED |
| D5 | P0 D3 Amana tapered 1/16" 2F rows. **Ruled 2026-11-03: the chip band from the printed IPM** (reverses the 2026-09-24 decision). | P0 item 5 | RULED |

## Phases

| Phase | Card | State | Commit | Next action |
|---|---|---|---|---|
| 0 Local fixes | `phases/P0.md` | DONE 2026-11-03 (FM1 re-blessed: 13 cells, TaperedBallNose 6.0 mdf; rough-score op 8 = 10 ramps, 9 led by a plunge; op 1 = 42 ramps, 2 real plunges) | see `git log` | |
| 1 Wildcard lint | `phases/P1.md` | DONE 2026-11-03 (563 hits: 256 exhaustive, 307 `#[expect]`; no value moved) | see `git log` | |
| 2 K6 consequence column, no refusals | `phases/P2.md` | NOT STARTED | | |
| 3 K4 entry contract | `phases/P3.md` | NOT STARTED (needs P2: the `gcode_hash` baseline is blessed at the P2 commit; the perf session committed 98344a12f) | | |
| 4 K1 `CutSpec` | `phases/P4.md` | NOT STARTED | | |
| 5 K2 / K2b envelope | `phases/P5.md` | NOT STARTED (needs P4) | | |
| 5b GUI envelope + K8 lineage | `phases/P5b.md` | NOT STARTED (D2, needs P5) | | |
| 6 K3 `ToolFact` | `phases/P6.md` | NOT STARTED | | |
| 7 K5 feasibility rules | `phases/P7.md` | NOT STARTED (needs P6) | | |
| 8 Q1 / Q6 / Q8 | `phases/P8.md` | NOT STARTED (Q1 needs P9 changes 1-2 first) | | |
| 9 K7 one engagement | `phases/P9.md` | NOT STARTED (heavy-tests: ask first) | | |

## Not in a phase

- The `suggest` CLI command landed with Phase 0.
- G-OPGOLDEN prints 46 rows at clean HEAD (recorded by 98344a12f). A phase
  gate passes when it prints the same 46 rows and no other row.
- Known red at HEAD 218ba2858, not caused by a phase:
  `rs_cam_viz --test an_unbacked_cell_still_adds_its_operation_fm5` (3 tests:
  `commands.rs` has no `FeedsError::Unbacked` arm; the add stops at "import
  geometry first"). Also red at 218ba2858 and 890850a67 (checked in a
  worktree, 2026-11-03), 14 tests in 12 viz targets: `apply_contract_a3`,
  `effects_are_stamped_wp19` (2), `folder_maps_name_real_files_f140`,
  `generate_all_skips_disabled_g_genalldisabled`,
  `loose_executor_is_not_named_in_viz_wp12`, `mcp_authoring_surface`,
  `mcp_wire_surface_pin`, `new_3d_op_boundary_is_silhouette_plus_tool_r2`,
  `panel_drafts_leave_egui_memory_ui09` (2), `the_gui_can_set_a_feed_g_fscontrol`,
  `the_inspector_text_is_bounded_ic1`, `the_selection_section_writes_edits_only_p4_3`.
  `render_pipelines_headless_g_pipesmoke` can fail under load (flaky).
  Run viz with `--no-fail-fast`: without it, cargo stops at the first red target. `cargo fmt --check` fails in `rs_cam_mcp/src/server.rs`,
  viz `ui/components/mod.rs` and `ui/properties/tests.rs` (files no phase
  touched).
- The op 2 rest planner (14.5 mm entry, 13 mm links) needs a simulation; it
  is separate work (`PLAN.md` section 6).
