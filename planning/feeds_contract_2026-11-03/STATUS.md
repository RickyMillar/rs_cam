# Feeds contract: status

Start here in a new session:

1. Read `PLAN.md` section 0 (the operator rulings) and section 10 (the terms).
2. Find the first phase below that is not DONE.
3. Read its card in `phases/`. The card is the full hand-off.
4. Update this file in the same commit as the phase.

Citations in the cards are `path:line@40cf411a7` plus a symbol. Find the
code by the symbol when the line has moved.

## Next session (hand-off by the lead, 2026-11-04, after P5d)

Operator plan (2026-11-04): run two jobs in parallel, each by one Opus agent; the lead
writes each brief, reviews each diff by path and commits. The two file sets do not overlap.

1. **P6 `ToolFact` (K3), the "enforce more" stream.** Card `phases/P6.md`. No number moves.
   Before the brief:
   - Check that the perf session no longer owns `crates/rs_cam_core/src/compute/simulate.rs`
     (`git status`, `git log -3 -- <file>`; `planning/perf_sweep_2026-11-03/`).
   - Check `git status -- planning/rivmap_block_job/`: the two toml files still had
     uncommitted edits of another session on 2026-11-04. The migration script skips them
     until those edits are committed; ask the operator if they stay uncommitted.
   - The P6 heavy runs (core `--lib`, viz) can pass three minutes: ask the operator first.
   - Then P7 (K5 feasibility rules) follows P6.
2. **P11 step 0, the "change the model" stream (read-only research).** Card `phases/P11.md`
   step 0. Operator go given 2026-11-04. Output folder `research_p11_step0/`: a script, an
   HTML page and an assert script for each claim the page makes (operator working
   preference). It edits no crate code. Its result is the evidence for the operator ruling
   on chip thinning against N-8 (D11), which P11 steps 2-5 need.

Order after both (P6 and P11 step 0 DONE 2026-11-04): P7, then P9 (one `Engagement`, heavy-tests: ask first), then P8 and P10,
then P11 steps 1-6 (with P9 or after it).

Prompt for the new session (paste after `/clear`):

> Continue the feeds contract. Read `planning/feeds_contract_2026-11-03/STATUS.md`
> ("Next session" section first), then `PLAN.md` sections 0 and 10, then the cards
> `phases/P6.md` and `phases/P11.md`. You are the lead: decide and verify; Opus agents
> implement and never commit. Do the P6 pre-checks in STATUS, write the P6 design
> decisions into the card, and brief one Opus agent for P6. In parallel, brief one Opus
> agent for P11 step 0 (read-only research, output in `research_p11_step0/`). Review each
> result by path, run the gates, and commit each with its own paths.

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
| D10 | P11 step 0, the validation study (read-only). **Go given 2026-11-04 (operator: start P6 and step 0 in parallel).** | P11 step 0 | RULED |
| D11 | Chip thinning against N-8. **Ruled 2026-11-04 (operator): yes, use chip thinning, but as a model, matched to the rules: "I assume the rule is there because it makes sense in a shop, but there is likely some underlying math that causes that rule of thumb. The point of this whole session is to get our internal models dialed."** Effect: P11 models the mechanics (engagement geometry, chip thinning, the force line, heat or rubbing) so that each printed shop rule (Garr's thinning rule, the vendor 1/2/3 x D depth table, the Garr finish row) comes out of the model; where the model and a rule disagree, find the missing term (`research_p11_step0/REPORT.md` section 7); never fit a rule per row. | P11 steps 2-5 | RULED |
| D12 | Unknown centre cutting / V tip. **Ruled 2026-11-04 (operator): they are required inputs; only Stated (or a printed Catalogue row) counts; an op with an Unknown or Estimated fact stops with a reason.** PlungeKnown is deleted from P7. | P7 | RULED |
| D9 | The machine aggressiveness dial. **Ruled 2026-11-04 (operator): remove it** ("before it was a generic slider to represent machine rigidity"); machine rigidity stays in `machine::RigidityProfile`. Before removal: list every reader (`rg -il aggressiveness crates`: feed modulation, tool_load verdict, machine, diagnostics, save) and show which numbers it still moves; a reader that moves a number reads `RigidityProfile` instead or its change is named. Old files: the key joins the retired keys. | P5b follow-up | RULED (not scheduled: after P7) |

## Phases

| Phase | Card | State | Commit | Next action |
|---|---|---|---|---|
| 0 Local fixes | `phases/P0.md` | DONE 2026-11-03 (FM1 re-blessed: 13 cells, TaperedBallNose 6.0 mdf; rough-score op 8 = 10 ramps, 9 led by a plunge; op 1 = 42 ramps, 2 real plunges) | see `git log` | |
| 1 Wildcard lint | `phases/P1.md` | DONE 2026-11-03 (563 hits: 256 exhaustive, 307 `#[expect]`; no value moved) | see `git log` | |
| 2 K6 consequence column, no refusals | `phases/P2.md` | DONE 2026-11-03 (A 4d69c2e3a: the column; B: no finding refuses; overrides deleted; G-code header warnings; GUI chips) | see `git log` | |
| 3 K4 entry contract | `phases/P3.md` | DONE 2026-11-03 (25 rows state `entry`; `EntryApproach`; `EntryRecord`; G-OPGOLDEN has `gcode_hash`, no byte moved) | see `git log` | |
| 4 K1 `CutSpec` | `phases/P4.md` | DONE 2026-11-03 (25 rows state `cut_rule`; FM1 unchanged) | see `git log` | |
| 5 K2 / K2b envelope | `phases/P5.md` | DONE 2026-11-04 (Suggest writes geometry only on a pick; `SafeEnvelope`; Q3; FM1 re-blessed). Heavy FM1 subset and `wanaka_suggest_integration`: approved, run after the commit | see `git log` | |
| 5b GUI envelope + K8 lineage | `phases/P5b.md` | DONE 2026-11-04 (envelope plot on the Passes tab, drag + live nomogram; Feeds tab merged into Passes (4 tabs); `Lineage` is a required field of every toolpath result; MCP `get_toolpath_lineage`; FM1 and G-OPGOLDEN unchanged) | see `git log` | |
| 5c Plot review fixes | `phases/P5c.md` | DONE 2026-11-04 (lines 4+4 labels -> 1-2 lines, 0 labels on rivmap100 3D Rough; frame = hard stops; candidate ring only on drag, proposal diamond only off Now; no DANGER; feed axis ticks) | see `git log` | Next: P5d |
| 5d Plot review 2 | `phases/P5d.md` | DONE 2026-11-04 (every limit in view draws and is named; range holds every flat depth limit; one mark rule on both charts: white = setpoint, grey ghost = stored during a draft, quiet ACCENT = recommendation; names Vendor DOC k×D / Rigidity DOC / Slot DOC / Cut length; D tick paints, an off-step D names itself alone; ic1 3D Rough Passes cap 34 -> 35, one more depth tick row; operator approved the live screenshot) | see `git log` | |
| 6 K3 `ToolFact` | `phases/P6.md` | DONE 2026-11-04 (A: type, readers, builder; B: private fields, setters; C: `ToolFact` storage, TOML forms, Q9 refusal at all three loaders, doors, 136 files migrated; no number and no G-code byte moved; F201 149 -> 154) | see `git log` | |
| 7 K5 feasibility rules | `phases/P7.md` | IN PROGRESS: A, B, A2 DONE 2026-11-04 (A2: D12 required facts stop an op; V-tip door and domain; the operator's facts in 141 repo files). Next: C (3D Rough clamp) | see `git log` | C |
| 8 Q1 / Q6 / Q8 | `phases/P8.md` | NOT STARTED (Q1 needs P9 changes 1-2 first) | | |
| 9 K7 one engagement | `phases/P9.md` | NOT STARTED (heavy-tests: ask first) | | |
| 10 Tool-derived geometry defaults (D8) | `phases/P10.md` | NOT STARTED (card drafted 2026-11-03; needs P5; lead answers its open points first) | | |
| 11 Force model, chip first, feed-driven limits | `phases/P11.md` | Step 0 DONE 2026-11-04 (`research_p11_step0/`: verdict FAIL against the preregistered rule; best group W 67 %, best group D 7.3 %, wood force candidates 0 / 418; 21 claims hold). Steps 1-6 NOT STARTED (need D11) | see `git log` | D11 ruling; then steps 1-6 after P9 or with it |

## Not in a phase

- P5d follow-up (lead, 2026-11-04): MCP has no call that drags the envelope point or
  sets the rail to 240 pt, so the live check covered the plot at rest only; the sentry
  `every_limit_is_drawn_and_named_p5d` covers the drag and 240 pt. The hard depth top
  edge mark has no fixture that reaches it (same function as the D edge).

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
- P5c follow-ups (2026-11-04): the "6" tick at the frame edge D does not
  paint (egui_plot); at the 240 pt rail the nomogram RPM axis names only "0"
  and its "Now" / "Max RPM" labels touch (both predate P5c).
  The nomogram "Recommended" label never paints (no place near the ring
  clears the Max feed wall, the Max RPM wall and the suggested line); no
  text names the blue ring. Fix: let that label cross the suggested line,
  or widen `MARK_SCAN` with a leader.
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
- Known red in the non-heavy core integration run (`test -p rs_cam_core --tests`), measured by
  the lead 2026-11-04 at P7 A2, none caused by P6 or P7 (10 checked at 25e30c6cd in a worktree:
  same failures, same numbers): 19 targets stop on S11 "feeds not set" in their own fixtures
  (`adaptive3d_lift_bridge_b1`, `axial_engagement_vs_dpp_detector_f2`, `derived_stepover_pr6a`,
  `drill_fed_descents_motion`, `drill_material_plumbing_f016`, `drill_runtime_survives_retime_n2`,
  `identity_setup_emission_frame_audit`, `lateral_setup_end_to_end`,
  `modulated_feeds_read_their_own_prefix_b5`, `pencil_tip_float_channel_d1`,
  `pindrill_emission_frame_g_pindrill`, `retime_respects_no_kinematics_n7`,
  `safe_z_emission_frame_g_safez_local`, `simulation_issue_channel_m1`, `tier_arcfit_burial_f4`,
  `unified_finish_dropped_band_finding_d1`, `unified_finish_tapered_end_to_end_m21`); others:
  `a_lead_curls_away_from_the_part_p6_0c`, `a_model_bottom_floor_reads_the_model_d1` (needs
  `--features step`), `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift`,
  `a_rough_cuts_no_deeper_than_its_leave_g_roughgouge`, `a_signature_serialises_every_op_f065`,
  `an_op_change_keeps_its_selection_p2_3`, `layered_framework_owns_the_ladder_w3`,
  `perf_golden_sim_metrics` (Profile air cut 1.908 s vs golden 1.929 s), `setters_have_rows_wp15a`
  (`bind_tool_impl`), `vendor_lut_sub_1mm`; plus the listed W4 check6, G-OPGOLDEN 46 rows,
  `_litmatrix_rubbing_floor_clamp`, `a_clearing_cut_holds_the_pass_load_g_adaptpassload`.
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
