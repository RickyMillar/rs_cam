# Feeds contract: status

Start here in a new session:

1. Read `PLAN.md` section 0 (the operator rulings) and section 10 (the terms).
2. Find the first phase below that is not DONE.
3. Read its card in `phases/`. The card is the full hand-off.
4. Update this file in the same commit as the phase.

Citations in the cards are `path:line@40cf411a7` plus a symbol. Find the
code by the symbol when the line has moved.

## Overnight plan (operator, 2026-11-04 evening)

The operator: "if you don't need me lets just polish this off." The lead runs the remaining list
without the operator: P7 C -> D9 (remove the dial) -> P9 -> P8 -> P10 -> P11 steps 1-6 -> P12
(hand-off). Lead reading of the run rule: the operator's earlier yes covers core `--lib` and the
full viz run at each commit; the non-heavy core integration run is the broad check; a heavy
binary runs only when a card's gate names it, one at a time, never the full heavy gate. A phase
that needs an operator ruling stops at that point and records the question here.

Questions for the operator (overnight):
- P8 Q6 (collisions: one warning on every surface, the GUI "Export Anyway" step goes): the card
  needs an operator-agreed mock first (no text clutter). The agent writes
  `phases/P8_Q6_MOCK.md`; no Q6 code lands until the operator agrees it. Q1 and Q8 go ahead.
  **Answered 2026-11-05 (operator): the mock is agreed; the card counts toolpaths.** Q6 may
  now be built as `phases/P8_Q6_MOCK.md` states.
- P11 L1/L2 (chip thinning): **operator 2026-11-05: yes, as planned** (thin roughing and
  clearing ops; finish ops keep the band chip; no new cap).

## Next session (hand-off by the lead, 2026-11-05, after P10)

The operator moves this work to a fresh session (new account, 2026-11-05). This section is
the full entry point. Read it first, then the files it names.

### Where it stands

- DONE and committed: P0-P6, P7 (A, B, A2, C), D9, P9, P8 Q1 + Q8, P10. See the phase table
  and the "Result" section at the end of each card. P10 is 84bfeab68.
- Waiting: P8 Q6 (build it: the operator agreed the mock 2026-11-05, the card counts
  toolpaths), P11 steps 1-6 (decisions L1-L9 written in `phases/P11.md`; L1-L3 confirmed by the
  operator), P12 (hand-off for the review session), the derate audit (below).

### Order for the next session

0. Done: the P10 integration run finished after the commit. 31 targets fail, all on the
   known-red list in "Not in a phase"; no P10 regression.
1. DONE 2026-11-05: **P8 Q6** (`phases/P8_Q6_MOCK.md` as drawn; sentry
   `a_collision_is_the_same_warning_on_every_surface_q6`; no new GUI text).
2. DONE 2026-11-05: **P11a** (steps 2 and 3, L4 and L5): the envelope defects D-A and D-B, then the engaged-edge
   force as a method on `feeds/engagement.rs` `Engagement` (peak for lateral force and
   deflection, mean for power). Decide the two P9 open diameters (`phases/P11.md` "Open from
   P9"): every reader uses `Engagement::d_eng` at the cut depth.
3. DONE 2026-11-05: **P11b** (steps 1, 4, 5, 6; L1-L3, L6-L9; plus the P11a item: a stated depth above the
   flute guard or the slot cap still sizes the feed and power at the clamped depth in
   `calculate` (`DepthClamps::Apply`); P11b sizes them at the stated depth and warns): chip thinning in Suggest (Garr peak chip, finish
   ops do not thin, no new cap), chip first (`doc_derating_scale` stops scaling the chip; the
   vendor 1/2/3 x D table becomes a visible limit), the envelope sized at the operator's chip,
   the source hunt, and the after-page `research_p11_step0/after/` (L9).
3a. DONE 2026-11-05: **P11b2** (lead decision 2026-11-05, a consequence of L1 and L6): the post-sim chipload
   gate (`tool_load`) and the feed modulator still judge the advance per tooth against the
   band max, and the gate still de-rates the band by depth. Move both to the peak chip
   (`fz sin psi` at the measured engagement) and drop the gate's depth de-rate, so Suggest and
   the gate read one chip again. FM1: 4 sim cells raise `load.chipload.commanded_above_band`
   until then. Then P11c, then P13 (the lead plan below).
4. DONE 2026-11-05 (research): **the derate audit**, `research_derate/` (REPORT.md, index.html,
   assert_claims.py 61/61). Headline: the vendor feed path has no light-machine factor; the
   double margins are geometry (slot cap over `doc_roughing_factor`, depth stack 0.20 of the
   vendor depth, unread `woc_roughing_factor`) and the Shapeoko chip formula x surface speed on
   60 FM1 cells (feed 0.213 of the vendor row). Lineage today: 0 stored step records. Eight
   operator questions in REPORT section 6. Its proposal overlaps P11b L6 and L8.
4a. **P13 order (lead, 2026-11-05):** step 0 (read-only research, operator: "do research
   first"): is the force/power model right at the vendor full-slot points; frame compliance
   (printed stiffness or a measurement protocol, series with the tool); chatter; chip
   evacuation; the removal-rate aim (`research_p13_step0/`). In parallel P13a: split
   `calculate_with` by step (H2, pure refactor, FM1 with no re-bless), H3, the F201 rows. Then
   the operator rules on step 0, then P13b (the lineage store), then items 4 and 5.
   **Operator rulings on step 0 (2026-11-05, `research_p13_step0/FINDINGS.md`):**
   - O1 yes: derive the hobby depth from a force budget per machine class, set by the printed
     hobby full-slot rows (Sienci, Carbide 3D); it replaces the typed 0.25 D depth.
   - O4 yes: the envelope proposal is the cut with the most removal per minute inside the
     limits (after O1).
   - O3: the add-door Pocket stepover is the computed width (the O4 pick, bounded by the
     corner-coverage rule (1 + 1/sqrt 2) r = 0.854 D), not 0.35 D. The card states the
     assumptions "chips clear" and "tool life not modelled" as visible warnings. A printed heat
     or tool-life source later becomes a width limit (O5).
   - O2 (measured frame stiffness) not now; O5 out of scope until a source appears.
   - Lead reading of where 0.35 D comes from: heat and tool life, chip evacuation, metal-cutting
     habit, floor tram ridges; none is a force limit and none has a number in the repo.
4b. **P13** (D13 items 4, 5, 7) plus review H3, H3b, L3, L4: the tool-fact kind in `Basis`
   (the legacy row as a child) BEFORE the lineage store; F201 rows for `MIN_AP_MM`,
   `MIN_AE_MM`, the 30 degree helix, `DRILL_DWELL_S`.
4c. **P14 tidy** (operator 2026-11-05: "anything related to this work should be tidyed in this
   session and the rest logged for later"), from `../arch_review_2026-11-05/REPORT.md`:
   H2 (split `calculate_with` by step behind FM1), M2 (one thinning law: `BandMid` and
   `dressup::feedopt` read `RadialThinning`), M3 (one `d_eng` door; delete
   `feed_ladder_diameter_mm`), M5 (the Unsourced count line), M6 (PLAN K2 and the
   unconditional `accept_unsourced` agree), M7 (refusal from the row kind only; rename
   `Severity::Blocking`, `blocks_export`, `model_gouge_refusal`), M8 (delete the 14 unbuilt
   `SuggestWarning` variants), M9 (one core shipped-point method; MCP gets the power), M12
   (feeds, session, compute folder maps back to maps; `CONTRACTS.md` size), L1 (stale names:
   `enforce_invariants`, pass 6b), L2 (`DressupPolicy` beside `EntryContract`), L5
   (`Lineage::unrecorded` behind `test-support`), L6 (split `session/compute/export.rs`).
   In P11c: M1, M4, M11. Carried to P12: L7 (the accepted reds). Logged for the selection
   session (`geometry_selection_2026-10-16/PROMPT_NEXT.md`): H1, M10, L8.
5. **P12** (`phases/P12.md`): the hand-off folder `handoff/` for the independent review
   session. Include every reviewer item listed below.

### Rules this session followed (keep them)

- The lead decides and verifies; one Opus agent per phase implements and never commits. The
  lead reviews each diff by path, runs the gate, and commits with explicit paths.
- The lead gate per phase (operator, 2026-11-05): read the agent's own test logs, then run
  clippy, G-OPGOLDEN (the 46 rows), the full viz run and the fmt check. The non-heavy
  integration run, FM1, wanaka, cli, mcp and core `--lib` run once more before P12, not per
  phase. Three full gates found one new red, in the full viz run.
- No tree-wide git. Other sessions share the tree. Leave alone `.mcp.json`,
  `planning/adaptive_research_2026-10-23/**`, untracked `play/*.toml`, `.pi/`. Three files
  carry known fmt diffs at HEAD (`rs_cam_mcp/src/server.rs`, `rs_cam_viz/src/ui/components/mod.rs`,
  `rs_cam_viz/src/ui/properties/tests.rs`); an agent must not reformat them.
- Every cargo command goes through `scripts/cargo_lane.sh`. viz tests with `-j 2`.
- Runs the operator allows: core `--lib`, the full viz run, the non-heavy core integration run
  (`test -p rs_cam_core --tests`), and a heavy binary only when a card names it (FM1 sim CSV:
  `--features heavy-tests --test feeds_matrix_instrument_fm1`, about 20 s now;
  `wanaka_suggest_integration`, a few seconds). Never the full heavy gate. Ask before any
  other run over three minutes.
- Never type a number from memory. Explain a model as HTML plus an assert script.
- The lead gate scripts are in the old session scratchpad; copy the pattern: clippy full line;
  the card's focused tests; the G-OPGOLDEN rows diffed against `opgolden_known_46_rows.txt`
  (`SAME46`); core `--lib`; cli; mcp; viz `-j 2` (10 known red targets, listed below); fmt
  (3 known files); the heavy FM1 compare when a feeds number moves; the non-heavy integration
  run when many tests change (compare with the known-red list below).
- Benchmarks for a change that may move G-code: release CLI at HEAD before the edit, then
  after; rivmap100 (`planning/fixtures/rivmap100/rivmap100.toml`) and nz-wanaka-21
  (`planning/rivmap_block_job/nz_wanaka_21.toml`), plain and `--apply-suggest`; report per op:
  motion lines, F words, S words, header lines.

### Known reds (all pre-existing; do not chase in a phase)

- viz (10 targets): `an_unbacked_cell_still_adds_its_operation_fm5`, `effects_are_stamped_wp19`,
  `folder_maps_name_real_files_f140`, `generate_all_skips_disabled_g_genalldisabled`,
  `loose_executor_is_not_named_in_viz_wp12`, `new_3d_op_boundary_is_silhouette_plus_tool_r2`,
  `panel_drafts_leave_egui_memory_ui09`, `the_gui_can_set_a_feed_g_fscontrol`,
  `the_inspector_text_is_bounded_ic1`, `the_selection_section_writes_edits_only_p4_3`.
  Flake: `render_pipelines_headless_g_pipesmoke` (SIGSEGV in the full run only).
- core: G-OPGOLDEN (46 rows), W4 check6, `_litmatrix_rubbing_floor_clamp`,
  `a_clearing_cut_holds_the_pass_load_g_adaptpassload`, and the integration list under
  "Known red in the non-heavy core integration run" in "Not in a phase".

### Reviewer items collected so far (P12 must carry them)

- P7 C: `cavalier_contours` panics 12 times (caught) in nz-wanaka-21 "Front 3D rough" on the new
  remaining stock. Op 1 generation time 15 s -> 27-48 s.
- D9: the words "long-tool share of the load target" stay; nothing reads that target.
- P9: no noise floor on `project.sim_outside_envelope`; the placement reads the toolpath
  material, not the per-sample material; the inspector "Measured" row needs a memo; the plot
  draws no sim points; the 3D Rough planner cuts near-slot widths (4-5x the stepover) on the
  opening cut of each level; FM1 sim: the Suggest recipe is outside its own envelope on 18 of
  35 cells; rivmap100 "Face 5" states 0.812 mm per pass and cuts 2.00 mm in one level (a Face
  ladder fault).
- P8: the warning keeps the name `RpmLoweredForFeedCeiling` when arm S ships; the two plunge
  terms name two rows; a material with no force line counts an arm as inside.
- P8 Q6: the Readiness panel now builds the export warnings each frame (new frame cost).
  The status bar, the live overlay, the overlay registry and `issue_triage` still read the GUI
  move list `sim.checks.rapid_collisions` (the Simulation workspace keeps the move count, as
  the mock states). The card's "simulation ran" test reads GUI state, not the session.
- P11a: the reviewer list in `phases/P11.md` "Result P11a" (post-sim edge-sum cost 40.2 ->
  40.8 s on the FM1 sim subset; `Edge::Unmodelled` names a modelled absence; two diameter
  rules (`effective_diameter`, `engagement_radius`); one radius for a ball, taper or V-bit
  edge (an upper bound); 16-step deflection scan; the unsourced 30 degree helix default now
  reaches every force reader; modulator compliance at the toolpath peak depth; the deflection
  gate reads `axial_engagement_mm`, the power gate `axial_doc_mm`; `efficiency.rs` specific
  energy at the nominal diameter; a column over target at the shallowest depth draws an edge
  at 0 mm). The `feeds::predict` wanaka back-rough fixture now reads 18.8 um (was about 48).
- P10: the calculator's ap factors put the add-door default outside the repo's own envelope
  (option (a) measured: Rigidity 118 -> 134, VendorAxial 10 -> 33 FM1 cells), so the axial
  defaults scale today's 6 mm values instead. The radial and axial tables are Unsourced.
- P11b: the L9 page is FAIL under the step 0 rule (W 2/7, D 23/455; group D fails by design
  under L6; the default machine's power binds most Onsrud slots). The reviewer list in
  `phases/P11.md` "Result P11b" (no cap on the lift: a very light width lands on the feed cap
  with RPM down to 0.515 x; no deflection step caps the lifted feed in `calculate`; L2 reads
  "finish" as `PassRole::Finish`, so Waterline thins; a one-value row allows only 1 x D at
  the suggested chip; the depth-table ceiling divides by the nominal diameter; the ball's
  axial thinning is reported, not applied; `feed_ladder_diameter_mm` has no production
  reader; the operator-chip power edge cost is not measured; no tool-strength value).
  `render_pipelines_headless_g_pipesmoke` crashes (SIGSEGV) under the full viz run (P11b and
  P11b2 gates); it passes alone.
- P11b2: the reviewer list in `phases/P11.md` "Result P11b2". Waterline keeps
  `commanded_above_band` on 2 FM1 sim cells: Suggest assumes the 0.1 D default width (lift
  1.667), the sim measures a median arc of 90 degrees or more on the dome (lift 1). The gate is
  right; the Waterline width assumption is the open item (P11c, with the stepover). The heat
  map still paints the advance, not the peak chip; the modulator's deflection and power caps
  read the modelled width while its chip reads the measured arc; `BandMid` keeps
  `1/sqrt(woc)` and `dressup::feedopt` the clamped `radial_chip_thinning_factor`.
- The stepover numbers: 0.12 x D (the Shapeoko profile table), 0.20 x D (`adaptive_woc_factor`,
  from Fusion per the operator) and 0.15 x D (the stale check) are all typed with no source.

### Prompt for the new session (paste after `/clear`)

> Continue the feeds contract in /home/ricky/personal_repos/rs_cam. Read
> `planning/feeds_contract_2026-11-03/STATUS.md` ("Next session" first, then "Work item for
> the next session: the derate audit"), then `PLAN.md` sections 0 and 10, then the cards
> `phases/P8_Q6_MOCK.md`, `phases/P11.md` and `phases/P12.md`. You are the lead: decide and
> verify; Opus agents implement and never commit. Follow the order in "Next session". Brief
> one Opus agent per phase, review each result by path, run the gate, and commit each with
> its own paths. Start the derate audit as read-only research in parallel with P11a.

## Work item for the next session: the derate audit (operator, 2026-11-05)

The operator: "its a bit concerning shapeoko defaults are applied to the whole cam software.
Those will already be fairly de-rated so our later deratings will just cut it down more."

- `feeds/mod.rs` `operation_default_profile` (42 k x D rows) is a port of the Shapeoko
  feeds-and-speeds `calcs.rs` (not in the repo; F201 `Unsourced`). It is a light-machine
  table, and every machine profile reads it.
- The calculator then applies more factors: the machine `RigidityProfile` (for example
  `adaptive_woc_factor` 0.20, a rule of thumb, the operator says from Fusion; it overrides the
  table's 0.12 for wood Adaptive at `feeds/mod.rs` `ae_factor.max(..)`), `doc_derating_scale`
  (1/2/3 x D), the long-tool derate, the slot cap, the flute guard and the rubbing floor.
  `compute/validate.rs` `check_wood_adaptive_stepover` warns below a typed 0.15 x D.
- The task: trace the chain from the vendor chip and the vendor depth to the shipped feed, rpm,
  stepover and depth for each op family. List each multiplier with its source (or Unsourced) and
  its machine dependence. Find each place where a machine-specific margin is applied twice:
  once in the light-machine table and once in `RigidityProfile`. Propose one place for the
  machine margin (`RigidityProfile`) and a machine-neutral base table (from a printed vendor
  source, or from the P11 model), with an HTML page and an assert script (operator rule).
- P10 (the add-door geometry defaults) reads the same table. P11 L8 lists part of these
  constants. P12 hands the numbers to the review session; this audit is a named part of it.
- Operator (2026-11-05): "will it be easy to weave in lineage to programmatically see how all
  variables came about?" Build the audit as that trace. Each multiplier in the chain writes one
  step record (value in, factor, value out, source or Unsourced) into the existing K2 `Basis` /
  K8 `Lineage`, so each shipped number has a tree down to a vendor row, a cited constant or an
  Unsourced constant. Expose the tree through MCP and the CLI. First measure how deep
  `Basis::Derived { from }` goes today.

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
| D9 | The machine aggressiveness dial. **Ruled 2026-11-04 (operator): remove it** ("before it was a generic slider to represent machine rigidity"); machine rigidity stays in `machine::RigidityProfile`. Before removal: list every reader (`rg -il aggressiveness crates`: feed modulation, tool_load verdict, machine, diagnostics, save) and show which numbers it still moves; a reader that moves a number reads `RigidityProfile` instead or its change is named. Old files: the key joins the retired keys. | P5b follow-up | DONE 2026-11-04 (`phases/D9.md`: no number moved) |


### D13: derate audit rulings (operator, 2026-11-05; questions in `research_derate/REPORT.md` section 6)

1. Depth margin: a typed margin needs more than one source, not only Shapeoko. The operator
   states that 0.2 is the Fusion 360 default (an operator statement; find a printed source).
2. Pocket stepover: `woc_roughing_factor` is hard for people to understand. Prefer a stepover
   computed from the rigidity model (as the other limits are), not a typed factor.
3. Slot cap: move it into `RigidityProfile` (operator: "sounds like it should").
4. The 60 formula-chip cells: a law fitted to the vendor rows.
5. Load the `industrial_only/` rows. Intent: the vendor rows are baselines; the math must agree
   with them (same as D11). They are evidence for the model, not a lookup to ship from.
6. The two rigidity default sets: researched (`research_derate/RIGIDITY_DEFAULTS.md`). The
   question was based on a wrong audit claim (B03, now corrected): a machine table with no
   rigidity block fails to load. `generic_wood_router` (0.20 / 1.5 / 0.20) serves every door
   with no machine table and FM1; `RigidityProfile::default()` (0.25 / 2.0 / 0.25) serves only
   the two Shapeoko presets. Both are Unsourced ports (172d6de60). Recommendation: one set in
   `impl Default for RigidityProfile` (the generic one; FM1 does not move), the presets and
   `validate.rs:284` read it. Open for the operator: the 2026-10-23 "no underived defaults"
   rule would give `RigidityProfile` no default at all.
   **Ruled 2026-11-05 (operator): "let the default be about shapeoko. its probably a good
   start.. but make it explicit that this number is 'hobby grade' vs 'industrial' or similar."**
   Lead reading: one default set, the Shapeoko profile (`shapeoko_vfd_1500w_balanced`, 0.25 /
   2.0 / 0.25 today in `RigidityProfile::default()`), named by a machine class (for example
   `MachineClass::Hobby` against `Industrial`) that the GUI, MCP, the CLI and the lineage name
   beside each value. The values stay Unsourced (the Shapeoko port) until a printed source
   lands. `generic_wood_router` reads it. Cost: FM1 moves (the audit counts 280 cap cells and
   32 adaptive stepover cells at 0.20 D); P13 measures and names them.
7. Lineage: store it in the project file, if it is not a memory sink. Measure the size first.

Lead plan (revised 2026-11-05): P11b stays L1-L9. A new phase P11c (after P11b) takes items
1-3 and 6 (the slot cap into `RigidityProfile`, the stepover from the rigidity model, the depth
margin with more than one source, the Shapeoko default named hobby grade). A new phase P13
(after P11c, before P12) takes items 4, 5 and 7 (the vendor-row law, the industrial rows, the
lineage trace stored in the project file).

Lead rulings for P11c (2026-11-05):
- Item 2 mechanism: no machine-stiffness constant is added (the repo prints none). The add-door
  default stepover of a roughing Pocket-family op (`default_radial_width`, today the table
  0.35 D) is the K2b envelope proposal's stepover at the op's depth: the computed limits
  (power, the deflection edge, the chip) pick it. Where the deflection edge binds, the basis
  names the Unsourced 200 um target. `woc_roughing_factor` is deleted (unread today).
- Item 1: a hunt for printed sources IN THE REPO for `doc_roughing_factor`. Two or more printed
  sources: cite them (F201 Cited). Fewer: the factor stays Unsourced, no value changes. Fusion's
  0.2 counts only if a Fusion document in the repo prints it.
- Item 3: `SLOTTING_DOC_CAP` becomes `RigidityProfile::slot_doc_factor`; `SLOTTING_THRESHOLD`
  stays machine-neutral geometry. P11b's "a stated depth ships" holds.
- Item 6: `MachineClass::{Hobby, Industrial}` on the machine table (break). `Hobby` has the one
  default set (the Shapeoko values). `Industrial` carries no values; its table states its own
  rigidity block. The class shows beside each rigidity value on every surface.
- Out of P11c: the Waterline width (P12 reviewer item), and P13 items 4, 5, 7.

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
| 7 K5 feasibility rules | `phases/P7.md` | DONE 2026-11-04: A, B, A2 (D12 required facts), C (3D Rough cut, link and terrain ramp at the limit slope; wanaka op 1 core strikes 37 -> 0) | see `git log` | C |
| 8 Q1 / Q6 / Q8 | `phases/P8.md` | DONE 2026-11-05 (Q1, Q8; Q6 as the agreed mock: the rapid-collision card counts the core `project.rapid_collision` rows, one per toolpath, on the pre-flight and the Readiness panel; sentry `a_collision_is_the_same_warning_on_every_surface_q6`) | see `git log` | |
| 9 K7 one engagement | `phases/P9.md` | DONE 2026-11-04 (one `Engagement`; pre-sim power is the worst case with its name; the simulation is placed on the envelope, not fed back; open items in the card) | see `git log` | |
| 10 Tool-derived geometry defaults (D8) | `phases/P10.md` | DONE 2026-11-05 (no default without a tool; radial = the calculator's own width; axial k x D from today's 6 mm values; FM1 353 cells; G-code byte-identical) | 84bfeab68 | |
| 11 Force model, chip first, feed-driven limits | `phases/P11.md` | Step 0 DONE 2026-11-04; **P11a DONE 2026-11-05** (D-A grid at the requested depth, D-B solved deflection edge, L5 engaged-edge force with the helix (peak for deflection, mean = power), every reader at `d_eng` of the cut depth; FM1: 430 `force_n` cells and 20 sim `deflection_peak_mm` cells, no feed/rpm/depth/power cell). **P11b DONE 2026-11-05** (Garr peak-chip lift, finish and drill do not thin, chip first with `LimitKind::VendorDepthTable`, limits at the operator's chip, L8 source table, a stated depth ships; L9 page: FAIL under the step 0 rule, W 2/7, D 23/455, recorded as is for P12). **P11b2 DONE 2026-11-05** (one `RadialThinning` rule for Suggest, the post-sim gate and the modulator; the gate and the modulator judge the peak chip at the measured arc; no depth de-rate on any band; FM1 sim: 10 cells, `commanded_above_band` 4 -> 2). **P11c DONE 2026-11-05** (items 1, 3, 6 and review M1, M4, M11: `slot_doc_factor` in `RigidityProfile`, `MachineClass` Hobby/Industrial with the one hobby default, three `woc_*` fields deleted from code and 81 machine files, one 200 um constant; item 1: no printed 0.20/0.25 D depth in the repo, stays Unsourced; FM1 280 cap + 32 stepover cells as the audit predicted). Item 2 NOT LANDED: no computed limit binds the Pocket width below D (sentry `the_pocket_width_has_no_computed_limit_below_d_p11c`); the 0.35 D cell stays Unsourced; operator question open (frame stiffness, chatter, chip evacuation, the removal-rate aim). P13 NOT STARTED | see `git log` | P13 |
| 12 Hand-off for independent review | `phases/P12.md` | NOT STARTED (operator, 2026-11-04: last; another session verifies the output numbers against industry practice) | | after P11 |

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
