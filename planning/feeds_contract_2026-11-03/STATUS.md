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
| D2 | K2b envelope plot sketch (`phases/P5b.md`). **Ruled 2026-11-03: agreed as drawn, on the Passes tab.** | P5b | RULED |
| D3 | P0 D1 pin tool. **Ruled 2026-11-03: a longer-flute pin tool, the one the GUI pin template picks (`session/pins.rs`).** | P0 item 3 | RULED |
| D4 | P0 D2 drill dwell. **Ruled 2026-11-03: delete `dwell_time`, add a retired-key row, migrate the 52 files by script; G82 keeps its fixed dwell.** | P0 item 4 | RULED |
| D6 | P6 D3 tool library. **Ruled 2026-11-03: yes, the script migrates `~/.config/rs_cam/tools/` and keeps a `.bak` of each file.** | P6 | RULED |
| D7 | Missing input. **Ruled 2026-11-03: a missing input stops generation or emission with a reason; only findings warn.** | P2, P5 | RULED |
| D5 | P0 D3 Amana tapered 1/16" 2F rows. **Ruled 2026-11-03: the chip band from the printed IPM** (reverses the 2026-09-24 decision). | P0 item 5 | RULED |
| D8 | Registry defaults after P5. **Ruled 2026-11-03 (operator): (b) tool-derived. An op has no geometry default without a tool; each geometry default is a fraction of the tool (example: stepover = k x D). Each k is a `FormulaConstant` with a printed source or marked Unsourced (no number from memory).** Until it lands, (a) holds: the built-in default warns and the operator picks the proposal. New phase P10. Evidence after P5 (a5ec62f44): 16 FM1 sim cells (Pocket, Profile, Ø6) exceed the depth rigidity cap at the fixed default depth per pass; 15 Face cells have stepover > D. | P10 | RULED |
| D9 | The machine "aggressiveness" dial changes no number after P5 (pass 6b deleted; the slider, MCP and the explain payload only show it). Options: (a) remove the dial; (b) make it pick the proposal point inside the envelope (for example, how close to the binding edge); (c) keep it as a display. Lead recommends (b) in P5b or (a). | P5b | ASK THE OPERATOR |

## Phases

| Phase | Card | State | Commit | Next action |
|---|---|---|---|---|
| 0 Local fixes | `phases/P0.md` | DONE 2026-11-03 (FM1 re-blessed: 13 cells, TaperedBallNose 6.0 mdf; rough-score op 8 = 10 ramps, 9 led by a plunge; op 1 = 42 ramps, 2 real plunges) | see `git log` | |
| 1 Wildcard lint | `phases/P1.md` | DONE 2026-11-03 (563 hits: 256 exhaustive, 307 `#[expect]`; no value moved) | see `git log` | |
| 2 K6 consequence column, no refusals | `phases/P2.md` | DONE 2026-11-03 (A 4d69c2e3a: the column; B: no finding refuses; overrides deleted; G-code header warnings; GUI chips) | see `git log` | |
| 3 K4 entry contract | `phases/P3.md` | DONE 2026-11-03 (25 rows state `entry`; `EntryApproach`; `EntryRecord`; G-OPGOLDEN has `gcode_hash`, no byte moved) | see `git log` | |
| 4 K1 `CutSpec` | `phases/P4.md` | DONE 2026-11-03 (25 rows state `cut_rule`; FM1 unchanged) | see `git log` | |
| 5 K2 / K2b envelope | `phases/P5.md` | DONE 2026-11-04 (Suggest writes geometry only on a pick; `SafeEnvelope`; Q3; FM1 re-blessed). Heavy FM1 subset and `wanaka_suggest_integration`: approved, run after the commit | see `git log` | |
| 5b GUI envelope + K8 lineage | `phases/P5b.md` | DONE 2026-11-04 (envelope plot on the Passes tab, drag + live nomogram; Feeds tab merged into Passes (4 tabs); `Lineage` is a required field of every toolpath result; MCP `get_toolpath_lineage`; FM1 and G-OPGOLDEN unchanged) | see `git log` | Next: P5c, then P6, in a fresh session |
| 5c Plot review fixes | `phases/P5c.md` | NOT STARTED (operator review of P5b, 2026-11-04; do before P6) | | |
| 6 K3 `ToolFact` | `phases/P6.md` | NOT STARTED | | |
| 7 K5 feasibility rules | `phases/P7.md` | NOT STARTED (needs P6) | | |
| 8 Q1 / Q6 / Q8 | `phases/P8.md` | NOT STARTED (Q1 needs P9 changes 1-2 first) | | |
| 9 K7 one engagement | `phases/P9.md` | NOT STARTED (heavy-tests: ask first) | | |
| 10 Tool-derived geometry defaults (D8) | `phases/P10.md` | NOT STARTED (card drafted 2026-11-03; needs P5; lead answers its open points first) | | |

## Not in a phase

- P5 follow-ups (lead, 2026-11-04): 16 `SuggestWarning` variants are never
  produced after P5 (the deleted geometry passes); passes 9 and 10 are inert in
  production; a pick sizes its speeds at the unsnapped depth while the snapped
  depth ships (safe side). The P8 and P9 card gates name the deleted tests t15
  and fm7: fix the cards before those phases.
- P5 follow-up (lead, P5b review 2026-11-04): the `SpeedsMap` grid (8 x 8)
  spans ap from 0.05 mm to the flute (step 3.6 mm on a 25 mm flute), while the
  P5b plot view is about 1.3 x the lowest ceiling. The grid is coarse where the
  operator drags. Span the grid over the plot view, or refine it there.
- P5b follow-ups (lead, 2026-11-04):
  - At the 240 pt Simulation rail, a number well in the Profile fixture's
    Passes rows ("2.0") is clipped to zero width. The up4 rail arm now skips
    a text whose clip has zero width
    (`inspector_width_is_tab_independent_up4.rs`): remove that skip when the
    row is fixed.
  - The tab merge changed test limits (each measured): `ToolpathTab::ALL`
    anchors 5 -> 4; screen non-vacuity 100 -> 80 (92 screens, 23 states x 4
    tabs) in w2b, w0, w6 and ic1; SCREEN height 1000 -> 2400 in w3k and
    g_hoverbound (the merged tab scrolls); ic1 caps realigned to 4 tabs,
    Passes = 36 / 34 / 28 (Pocket / 3D Rough / 3D Finish). ic1 now fails only
    on "Tool: 18 visible text rows, cap 17".
  - Idea (operator, 2026-11-04): show each envelope limit as a fill fraction
    at the pick (sorted bars beside the plot, live on drag). The operator
    has not ruled; a radar chart was considered and does not show the trade.
- Research (operator, 2026-11-04): stepover does not move the feeds and
  speeds, and a raised stepover does not lower the proposed depth. The operator
  expects a higher depth and a lower stepover to trade ("kind of cancel").
  Today the trade exists only as the Power and Deflection edges (`ap(ae)`
  curves), and radial chip thinning is observed, not applied
  (G-CHIPTHIN-HALFFIX: the vendor tables print no radial reference
  condition). Question: which printed sources in the repo, or new ones, give
  the depth-stepover-chipload relation with a reference condition? Link: P9.
- Backlog idea (operator, 2026-11-03): a geometry field may hold an expression
  of the tool, such as `0.2*D`, and fill itself from the tool, as in Fusion.
  Not in a phase; consider it after P10.

- The `suggest` CLI command landed with Phase 0.
- G-OPGOLDEN prints 46 rows at clean HEAD (recorded by 98344a12f). A phase
  gate passes when it prints the same 46 rows and no other row. The rows are in
  `opgolden_known_46_rows.txt`: compare with `diff`.
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
  `rs_cam_core --test every_op_conforms_w4` check6 is red since e8072f172
  (`contour_each_level` has `skip_serializing_if`, so a default config does
  not hold the field). Viz lib `duplicate_queued_toolpaths_are_coalesced`
  can fail under load (a 20 ms sleep).
  `rs_cam_core --test a_clearing_cut_holds_the_pass_load_g_adaptpassload`
  `a_straight_plunge_entry_is_exempt_only_leaving_its_hole` is red at
  c6826ad05 (checked in a worktree, 2026-11-03).
  `rs_cam_core` `_litmatrix_rubbing_floor_clamp` is red at c0db8dcb6 (Ipe 6 mm
  Pocket: the winner is freud-solid-carbide-quarter-hardwood, the test expects
  amana-compression-wood-pocket-6350-2f; checked in a worktree by p5-impl).
- G-OPGOLDEN has a `gcode_hash` per arm (P3). A field-only bless:
  `RS_CAM_BLESS_OPGOLDEN_FIELDS=<f,..>` (optional `RS_CAM_BLESS_OPGOLDEN_ARMS`).
  Never bless the fields of the 46 known rows; regenerate
  `opgolden_known_46_rows.txt` and show its diff in the commit.
  Run viz with `--no-fail-fast`: without it, cargo stops at the first red target.
  Confirmed at 218ba2858 in a worktree with its own lane script (2026-11-03).
- Trap: `scripts/cargo_lane.sh` changes into the top of ITS OWN checkout. In a
  worktree, run the worktree's copy (`<wt>/scripts/cargo_lane.sh`) with
  `CARGO_TARGET_DIR=/home/ricky/personal_repos/rs_cam/target`; the main
  tree's copy builds the main tree. `cargo fmt --check` fails in `rs_cam_mcp/src/server.rs`,
  viz `ui/components/mod.rs` and `ui/properties/tests.rs` (files no phase
  touched).
- The op 2 rest planner (14.5 mm entry, 13 mm links) needs a simulation; it
  is separate work (`PLAN.md` section 6).
