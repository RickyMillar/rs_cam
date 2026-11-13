# Next session: feeds and speeds correctness (brief, 2026-11-08, second edition)

## HAND-OVER 2026-11-11 evening, updated 2026-11-12 night (read this section first)

You are the lead of the feeds contract. No agent of the old session exists for you. Start one
implementer of your own. The operator's standing rules are in the root `CLAUDE.md` and in the
memory folder (`MEMORY.md`); the ones that bite most:

- You decide and verify; agents implement and never commit. Review by path.
- Every cargo command: `timeout <s> scripts/cargo_lane.sh <args> -j 2`, in the foreground. No
  workspace-wide `cargo test`. No core full gate without the operator's go.
- Ask before a test run over about three minutes. The operator gave a standing go for ONE set at
  the end of each of P29-1, P29-2 and P29-3: the named sentries, `cargo test -p rs_cam_core -q
  --lib`, FM1 (`feeds_matrix_instrument_fm1 --features heavy-tests`) and full clippy.
- No heavy suite while the GUI holds a simulated big project (OOM). Check `gui_status`.
- Git: other sessions share the tree. Scope every `git add` and `git commit` to your own paths.
- A constant comes from math or from a source stored in the repo. Every finding warns; no job is
  refused. A decision that leaves a toolpath out of a limit goes to the operator.
- Never save over `planning/rivmap_block_job/nz_wanaka_21_operator.toml` without the operator's say.
- Prose in ASD-STE100 (short active sentences).

### What P29 is

One cut object. Every limit and metric is a function of a cut and returns one record. The op type
decides which cuts exist, never which limits apply. Read, in this order:
`phases/P29_FINDINGS.md` (design in section 4, change list in section 6, the operator's rulings
in sections 7 and 10, the probe result in section 11), then `phases/P29-0_probes.md`.

### State of the change list

| Step | State |
|---|---|
| P29-0 probes | DONE, committed `027cfa72c`. Section 9 of `P29-0_probes.md` corrects section 5 of the findings. |
| P29-1 `CutLoad` | See "P29-1 status" below. |
| P29-2 `Check`, `LimitId`, one basis | DONE. 2a `8c8671f07` (core folder `limits/`, sentry `every_limit_is_a_limit_id_p29_2`). 2b committed after it ("P29-2b" in `git log`): the MCP replies `get_tool_load_report`, `get_safe_envelope`, `get_suggest_rationale` hold `checks` beside the old fields; three lib tests `_p29_2b` in `rs_cam_viz/src/app/mcp/tests.rs`. Gates: viz named tests 18 pass, viz `--lib` 706 pass, clippy clean; no pin moved. Not done: a live MCP read of the new keys (needs a rebuilt release GUI). Open items: `limits/CLAUDE.md` and section 10 of `phases/P29-2_DESIGN.md`. |
| P29-3 `Cut` and `cuts()` | DONE, byte-identical, three commits. A `66e667ff8` (`limits/cut.rs`, `limits/exemptions.rs`, the straight plunge on `CutLoad::of_cut`, `Limit::depth_rule`). B `a26803721` (`compute/catalog/cuts.rs`; `PowerCases`, the slot ring, the offset-ring test and three depth-rule and five thinning readers read `cuts()`). C `8698facaf` (`limits/sample.rs`: `Cut::of_sample`, `is_judged_today`; sentry on four simulated fixtures). See "P29-3 status" below. |
| P29-4 to P29-6 | Behaviour steps; all come before the operator's cut. Write predictions before each. |
| P29-7 to P29-last | After the cut. |

### P29-8 and P29-9 status (2026-11-13)

Notes and predictions: `phases/P29-8_DESIGN.md`, `phases/P29-9_DESIGN.md` (commit `5ed089621`;
lead rulings in sections 13 and 11).

| Commit | State |
|---|---|
| 8a `e88fe9491` | DONE, byte-equal. `feeds/entry_rule.rs`: `ResolvedEntry` is the one door from an op to both entry emitters and the ramp notes. A helix with no room ramps at the op's own angle. F201 pin 146. |
| 8d `d7e6527b8` | DONE, no number moved. An entry is a declared cut (`DeclaredDepth::Own`, `Axial`; `planned::PlaneDepth`, `sets_a_ceiling`). It is a value check, never a ceiling. Row `U-EntryFallback`. Pins 40 / 29 / 39. |
| 8b `35f8e1a56` | DONE. Operator ruling O2: Easel, 5 degrees hardwood and 20 degrees softwood, plywood, MDF. `EntryAngle { Stated, Rule }`; 266 FM1 cells as predicted; wanaka byte-equal; op golden re-blessed in 22 arms; pin 144. |
| 8c `0fad0599d` | DONE. Operator ruling O1: the op plunge rate. Table `G` exact (wanaka idx 0, 8, 11, 12, 17 entry feeds double); FM1 byte-equal; pin 142. |
| Clip re-entries (was "tab re-plunge") | DONE. Notes `phases/P29-8e_TAB_REPLUNGE.md` (sections 10 to 15, lead rulings Q7 to Q11) and `phases/P29-8e_OBSTACLE_FRAME.md`. e1 `910ded4b4`: the clips return typed re-entries (`geometry::boundary::Reentering`), one step after the clips (`session/compute/clip_entry.rs`) sends them through the entry rule: a fold ramp under Ramp and Helix, the stated plunge under None; a surface-riding op keeps the plunge. Wanaka idx 17: 42 plunges to 0, 710.99 s to 740.54 s. e2 `187c4ee8d`: a tab descent under None runs at the op plunge rate. e3 `cc0a3cb74`: `LimitId::PlungeRate` judges every declared cut against `VerticalRateLimit::limit_at` (a warning; idx 17 plunge entry 1582 against 1347.47); `U-TabReplunge` closed, pins 37 / 26 / 37. e4 `d60ab8aa4`: the fold walk holds at least the minimum run (a defect since 8b: a 20 degree rule ramp fell back to a plunge on a fine polyline); 40 golden rows and 1 FM1 cell as predicted. **Frame verdict: the operator file is wrong, the code is consistent.** The clamp boxes of setup "3 Front" subtract the stock origin; the corrected boxes are in the frame note section 5. `nz_wanaka_21_operator.toml`, `nz_wanaka_21.toml` and `derive_wanaka21.py` hold the same error: NO edit without the operator. Open, logged: a fold ramp for a surface-riding clip re-entry; F-OF1 (one obstacle frame on every face) for the P27 brief; `geometry/boundary.rs` imports `feeds::entry_rule` and `apply_dressups_with_entry` is a second door (clean-up pass); the `PlungeRate` `Exceeds` shows only in MCP `cut_checks` until P29-10. |
| Entry chip rounding | DONE. `01a2ff1cc` (`entry_rule::StoredFeedChip`: a stored whole-mm/min feed states a chip interval; the margin follows the judged value) and `e6b587a2e` (the ramp and the plunge read the feed before the write step; FM1 +1 mm/min on 10 cells, `PlungeReDerived` leaves 7 cells). |
| P29-9 | DONE. 9a `8ccd709f6` (one depth door `DeclaredCuts::body_depth`, the true chamfer depth, 76 FM1 cells as predicted, `P28-32` closed). 9b `ef78cf0e6` (a Project Curve is judged at the lower bound of its depth; one sign rule `ops::project_curve::tip_depth_into_material_mm`; `P28-11` closed; the note's sign was wrong, sections 14 to 17 of `P29-9_DESIGN.md` hold the trace, the measured frame and the results). 9c + 9d `82ba2805f` (a finish is judged at the earlier op's leave; a depth limit with no edge writes a `NoNumber` row; `F-OffsetRing` re-filed). `a53e2c103` (`CutSpec::calc_inputs` gives no depth). Pins 38 / 27 / 37. |
| GUI number parity | DONE `0038733a5`. `SuggestContext` has no `Default`; the viz reads the session door (`ProjectSession::suggest_slots`); test `the_gui_reads_the_session_suggest_door_p29_9`. |

Open from 8d for P29-10: the GUI envelope passes no dressups, so its `cut_checks` read the
registry default entry; the three or four entry `Exceeds` rows on the operator file are warnings
against the parked 35.3 N budget.

**The operator is away 2026-11-13 to 2026-11-15** and said: keep going until a real wall. Not
without the operator: an edit of the operator file, a push, the core full gate. Next in order
after the clip re-entry fix: P29-10 (`phases/P29-10_DESIGN.md`, `422de05ef`, lead rulings in its
section 12; start with 10a to 10d), 10i (every declared cut judged at the stated point; a note
with predicted rows first), P29-11, P29-last, the modulation design step, the clean-up pass with
a review agent that did not write the code, then P29 state into `phases/P27_BRIEF.md`.

### P29-3 status (2026-11-12, night)

Gates of each commit: fmt, the named sentries, core `--lib` (the one red is the timing trap,
green alone), FM1 2/0 with no CSV change, full clippy clean; viz `--lib` 706/0 and the three
named viz tests after commit B. No pin moved.

The exemption list: 67 rows; 8 physical, 3 with an operator ruling, 56 need a ruling. The
sentry `the_exemption_list_only_shrinks_p29_3` pins each count.

Lead decisions in P29-3:

- Row P28-13 (`trochoid_cap_mult` 1.6) is "needs a ruling", not "ruled": the operator said
  to retire the 1.6, and a lead decision (P23a) kept the read. The operator now leaves the
  call to the lead (see the rulings below).
- A dogbone relief is its own kind, `CutKind::DressupRelief`, read from the `DressupArtifact`
  span. The span wins over the cap table.
- New row `C-LeadInApproach`: the gates drop lead-in and entry-approach samples by the intent
  flag, and no row named them.
- The red sentry `the_simulation_is_placed_not_fed_back_k7` (red since P29-1) is green:
  `34e678d5f` gives `feeds/cut_load.rs` a named allowance for the sample doors.

Readers that WAIT (their context holds no operation config; `ToolpathLoadContext` and
`FeedsInput` must carry the body facts, about 140 test literals; do it at P29-6):
`tool_load/depth.rs`, `tool_load/cut_force.rs`, `force_budget::rigidity_depth_cap`, Step 5
of the calculator (`feeds/calculator/feed.rs`), `tool_load/chipload.rs`,
`tool_load/distribution.rs`, `deflection::exceeds_limit`. They still call
`depth_cap_rule(family, role)` and `RadialThinning::of`; both read the one filler.

Findings logged, not fixed:

- Scope reads with no marker and no row: `feeds/calculator/close.rs:248`,
  `feeds/plunge.rs:430`, `feeds/ramp.rs:393`, `feeds/predict.rs:374`,
  `feeds/suggest/adaptive_entry.rs:66`, `tool_load/distribution.rs:430` and `:604`. The
  vendor-row family keys (`chipload.rs`, `vendor_lookup.rs`, `extrapolation/family.rs`) are
  not audited. The list is not yet complete.
- M8 has a third reader with no span lookup: `tool_load/mod.rs:302`. It tests only
  `is_cutting`, so air-class fed samples enter the axial-depth population.
- The span filter calls air samples steady; each gate adds its own air test.
- An entry plunge after a tab that lies in no span is dropped as transit; it never reaches
  the entry-spike report.
- `is_judged_today` has no fixture for `Drill`, `Entry(Helix)`, `CappedStep(LevelContour)`,
  `FirstPass`, `SpringPass`.
- The default 3D Adaptive strategy is ContourParallel, so the default 3D rough holds the
  unsourced `CapMult 1.6` power case.
- A Profile of 40 x 24 mm that asks for 4 tabs gets 2 (`FewerThanAsked`).

### P29-4 status (2026-11-12, night)

Design and predictions: `phases/P29-4_DESIGN.md` (lead rulings in section 8),
`phases/P29-4_predictions_fm1.csv`, `phases/P29-4_predictions.csv` (commit `46564a8b6`).

- **Commit A is in** (`f5a5b4601`): the Adaptive roughing depth reads the force budget
  (ruling R1). FM1 moved 220 cells in 80 matrix rows and 10 simulation rows; each cell equals
  the prediction. The wanaka proposals equal the predictions. Exemption pins: ROWS 64,
  PHYSICAL 8, RULED 3, MAX_NEEDS_RULING 53, ROWS_WITH_A_FILE 56.
- **Commit B is in** (`b6688d289`; lead re-ran fmt, full clippy and FM1 on it): the dead Factor chain
  is deleted. FM1 is byte-equal. Left by B, not fixed: three texts that users see still say
  "rule of thumb" (the `ExceededCriterion::depth_of_cut` remedy in `tool_load/verdict.rs`,
  `DEPTH_OF_CUT` in `tool_load/metric_guide.rs`, viz `sim_diagnostics.rs` "This limit is a
  rule of thumb."); `feeds::geometry::depth_derate_diameter_for_cutter` has no caller; stale
  docs in `feeds/geometry.rs` (~513, ~561) and
  `tests/a_vbit_row_is_read_at_its_printed_key_b4.rs:679`.
- **Commit C is in** (`5549190ed`). The operator ruled on 2026-11-12: "go hard on any toml.
  I dont mind". The field `adaptive_doc_factor`, the default-depth floor and
  `ADAPTIVE_RIGIDITY_DOC_DIVISOR` are deleted; the key is a retired key (a file that holds it
  refuses to load); 333 tracked TOML files lost that one line, and the lead deleted it from
  the operator's machine library file `~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml`.
  FM1 byte-equal; both wanaka files load with equal proposals. Left: 
  `planning/rivmap_block_job/derive_wanaka21.py` (line 548 reads the key; a peer has
  uncommitted edits there) and `derive.py:395` still name the key, so their next output
  refuses to load or the script raises `KeyError`; four untracked play TOMLs under
  `planning/adaptive_research_2026-10-23/play/` also lost the line (not committed);
  `simulation_issue_channel_m1` was red before C (a Pocket with no stepover).
- **P29-5** (a slot or groove op reads the force budget): design and predictions committed
  `3d9e33387`; the operator said "go" on 2026-11-12 for the depth-limit-only scope (the lead
  rulings are in the implementer brief: Q0 narrow, Q1 close `F-DepthRule`, Q2 filing fixes to
  "P29-5b", Q3 card text "no force limit on this cut", Q5, Q6, Q7 as the note recommends).
  **Commit A is in** (`6adc14993`): 110 FM1 cells in 55 rows, equal to the predictions; the
  simulation file byte-equal; pins ROWS 63, PHYSICAL 8, RULED 3, MAX_NEEDS_RULING 52,
  ROWS_WITH_A_FILE 55; new sentry `a_slot_reads_the_force_budget_under_every_op_p29_5`.
  **Commit B is in** (`884fea167`): texts only; 27 `force_detail` cells of the FM1 simulation
  file moved for wording. The limit text says "an advisory", as the docs of
  `BoundSource::gates_export()` do. Open after P29-5: P29-5b (the filing fixes of the
  exemption rows), P29-5c (the groove width before the simulation; row `P28-03`). Not run:
  any simulation of the six ops (force card, modulator scope, `project.sim_outside_envelope`
  follow from the code only); the slow wanaka integration tests. Prediction to check at the
  operator's next simulation: wanaka op 5 (Project Curve, 6 mm, offset 4.0) is up to 3.6
  times over the budget, so its exported feed falls heavily with modulation on.
- **P29-6** (the gates judge every sample that removes material): design and predictions
  `98890cc09`. The operator said "go" on 2026-11-12 and ruled Q1 yes: "I think all moves
  should be judged", and "I want to leave the code in a place where its blatently obvious to
  anyone in the future that adding a mechanism like force neds to happen acreoss all
  movement." That sentence is the acceptance test: a limit is one function over a cut, the
  match over `CutKind` / `CutGroup` has no wildcard, "not judged" is a named variant with an
  exemption row. Lead rulings: Q2 to Q10 as the note recommends; Q7 an arc of 0 is no arc.
  Commits: 6a `5c6ae274b` (the context constructor, byte-equal), 6b `230692ac4` (the chip
  gate reads every removing sample; the 95 % filter is gone), 6c `adfda83de` (the depth gate
  judges each sample against its own cap), 6d `6409d36c6` (one driver, every gate judges
  every group). Each matched its predictions; one 6c cell differs in the last digit (a probe
  rounding error). Pins after 6d: ROWS 51, PHYSICAL 8, RULED 3, MAX_NEEDS_RULING 40,
  ROWS_WITH_A_FILE 43.
  **How to add a limit** (also in `limits/CLAUDE.md`): add a `LimitId`
  (`limits::scope(LimitId, CutGroup)` then does not compile until it answers every group);
  write a `MetricEvaluator` (`tool_load::groups::judge_by_group` builds the `ByGroup`
  record); by hand, one field of `GroupVerdicts` and one row of `GATE_LIMITS`.
  **6e is HELD by the lead** (the modulator lowers the feed of a lead move): it widens the
  post-simulation feed change that the operator wants redesigned, and the acceptance of
  P29-6 holds without it. It goes into the modulation design step below.
  Open after 6d: the depth card draws one bound over samples that each have their own cap;
  the summary `exceeds_breakdown` names no group; the triage `DedupKey` merges a headline and
  a group finding of one id; a `NotJudged` or `Unmodeled` group shows only in MCP;
  `GroupVerdicts::headline_only` is a second constructor (tests); `P28-10` (power on a
  drill) is NeedsRuling; a skin sample reads `ArcEngagementNotCaptured` and its text says
  "Run the simulation again", which does not help; the load report of the shelf fixture
  takes 373 ms, was 290 ms (debug build); the cap table of `load_context_parts` is built on
  each call (GUI thread; not measured); section 6 of `P29_FINDINGS.md` does not hold the
  P29-6 result. Not run: the slow 3D simulation tests whose docs name deleted predicates.
- **P29-5b DONE** (2026-11-12): design `8e28c1913`; `6e1e5a731` (a Profile writes
  `ClearingCut`; the G-code of operator idx 17 is byte-equal), `a5c697775` (the Bore depth is
  one helix turn; 41 FM1 cells), `eca178bd4` (first-pass filings, option A; 24 FM1 cells;
  rows `M11`, `P28-12` closed). Lead recount 65 of 65 cells. **Behaviour change the note did
  not predict:** a ContourParallel 3D rough ships the slot power case of its perimeter sweep,
  so a deep stated cut gets a lower Suggest feed (18 mm tool, 10 x 5.5 mm: 9123 to 2624
  mm/min, `PowerLimited`). The wanaka roughs are geodesic and do not move. Open: the `CapMult`
  power case can no longer be the largest case under ContourParallel (rows `P28-13`,
  `P28-14`); the planner could feed the sweep unlifted (5b note F1), which also closes
  `U-PerimeterSweep`.
- **P29-5c DONE**: `267dee41c` (VCarve, Inlay, InlayPlug solve the force budget at the slot
  width; Chamfer to `P28-32`; row `P28-03` closed; 162 FM1 cells, lead recount 0 mismatches).
  Pins now: ROWS 48, MAX_NEEDS_RULING 37, ROWS_WITH_A_FILE 41.
- **P29-7 in work**: design and lead rulings in `phases/P29-7_DESIGN.md` section 13
  (`MayOccur` names an exemption row and `U-PerimeterSweep` stays open; the modulator part of
  R5 is HELD, `P28-16` narrows; `VendorDepthTable` joins the driver in 7c, no 7c2). Order: 7a
  and 7b (no number moves), then a **prediction amendment before the 7c code** (the
  removal-rate pick with the new ceilings: `proposal_ae_mm`, `proposal_ap_mm`,
  `binding_limit`; the vendor depth table cells; a rivmap100 row for trap 1), then 7c, 7d.
  7c lowers the wanaka 3D rough proposals by 2 to 5 % (table in section 10.3); the operator
  has that table.
- **P29-7 DONE** (2026-11-12): 7a `cf0c421d3` (`FeedsInput::of_operation`, the one
  constructor), 7b `3e63a0902` (every op declares the rest of its cuts; `DeclaredFeed`,
  `Presence`), amendment `f27ff3444`, 7c `eb6a4a4a7` (`feeds/planned.rs`, the driver, one
  envelope loop over `LimitId::ALL`, `binding_cut`; 104 FM1 cells, lead recount 0 mismatches;
  47 wanaka and rivmap probe rows equal), 7d `0807a9ff3` (one deflection limit; the modulator
  part HELD). Test fix `abf09c546` (a k7 test red since 6d). Pins now: ROWS 39,
  MAX_NEEDS_RULING 28, ROWS_WITH_A_FILE 38. Result and findings: section 16 of
  `phases/P29-7_DESIGN.md`. What moved for the operator: wanaka 3D rough proposals fall 1.8 to
  5.2 %; the rivmap100 live copy 3D Rough (ContourParallel, stated 8 mm) proposal goes 6.00 to
  2.585 mm; groove ops read the full cone on every planned limit. Open: no repo machine and
  op has Power as the lowest depth ceiling of a slot-declaring rough (p20i derives a test
  machine); the chip rows of `cut_checks` reach no card (a zigzag Pocket first slot gives
  ChipMax 0.1704 against 0.127 mm; a geodesic rough body exceeds ChipMin) and the `MayOccur`
  note fills the card header (both P29-10); the unlifted slot-ring power gap (Q5); each op
  declares one capped-step source; `U-PerimeterSweep` keeps `Who::Undeclared`;
  `operation_kind: Option` is always `Some`. Next: design notes and predictions for P29-8.
- **Known reds, not from P29-5b or later** (clean-up pass): `layered_framework_owns_the_ladder_w3`
  (pinned move hashes stale since `5d700703a`, which added `Move::feed_cap`; traced from git
  history, not from a build); `_litmatrix_rubbing_floor_clamp::ipe_pocket_ships_its_computed_
  feed_above_the_floor_after_r4` (the vendor row changed; cause not traced);
  `every_op_conforms_w4` check6.
- **Operator, 2026-11-12: finish P29 before P27**, all steps of the change list (P29-5b, 5c,
  7, 8, 9, 10, 11, last), each with predictions first. The lead brings a prediction to the
  operator only when it changes what they cut with. **After P29-last: a tech-debt and
  clean-up pass** over the code that P29 moved (dead code, stale docs and folder `CLAUDE.md`
  files, duplicate helpers, test helpers, the exemption list, names), with a review agent
  that did not write the code. Then add the P29 state and the three findings (budget from
  slot rows, the budget depth step, two chips) to `phases/P27_BRIEF.md`.
- **New design step, from the operator (2026-11-12):** "simulation should run whats going to
  happen in real life. having a post-sim change seems not ideal". Today the modulator lowers
  feeds after the simulation and the export writes them. First task: a read-only check of
  what the load cards and the time estimate show after modulation (the first run or the
  lowered feeds). Then a design: the simulation shows the feeds that the export writes.

Tell the operator before the cut (ruling Q3): after a simulation with modulation on, the
exported feed of an Adaptive move over the force budget now falls. The depth stays.

Findings of commit A (not fixed):

- **The placement and the proposal read two chips.** `judged_ceiling` takes
  `min(CurveChip, Pick)`; the Pick chip is `max(operator chip, suggested chip)`. `propose`
  runs before the Pick entry exists and reads the operator chip. On the rivmap 3D Rough
  (4.217 x 2.303 mm, white oak, 6 mm) the proposal keeps 4.217 mm, and `placement_of` says
  Outside Rigidity (judged 3.27 mm). The force at the shipped chip is 31.25 N, under the
  35.32 N budget; at the suggested chip it is 43.15 N. So the card warns on a cut that is
  inside the budget as shipped. A speeds-only apply probably puts it over (inferred from the
  force at the suggested chip; what the apply writes was not probed). Every Adaptive op that
  states a chip under the suggested chip shows this. The fix moves proposals: its own step,
  predictions first. Lead proposal: `propose` reads the Pick chip, because an apply writes
  the suggested speeds.
- **Wanaka, probed on `b6688d289` (read-only loads).** Operator file idx 1, 2, 14, 15: the
  stated chip is above the suggested chip, so the judged ceiling equals the proposal and the
  conflict does not show. Work file: the suggested chip is about 1e-5 mm above the stated chip,
  so a point at the proposal depth is Outside Rigidity by about 2e-5 mm (`placement_of` has no
  tolerance); the card shows it on idx 15 only.
- **The proposal can leave the move-budget floor.** On operator idx 2 and work idx 1, 2 the
  point (proposal depth, stated width) is Outside Runtime (margins -0.184, -0.282, -0.167).
  The Runtime width floor reads the depth; `propose` checks the ceilings, not that floor.
- **The budget is probably low for a partial-width cut (operator, 2026-11-12: tune later, do
  not chase it now; the project is architecture first).** The 35.3 N is the largest of six
  printed SLOT rows. The Sienci hardwood chart prints a pocketing row for the same 1/4" tool:
  5.6 mm deep at 2.85 mm width, 2740 mm/min, 12000 RPM. The budget gives the 6 mm wanaka work
  ops 3.67 mm at 1.5 mm width (white oak, chip 0.143). Not probed: the model force at the
  printed pocketing rows. The value is a machine field (`[job.machine.rigidity]
  force_budget_n` in the project file), so a later tune needs no code. The operator says the
  Shapeoko Pro XXL is fairly rigid and has cut deeper than the proposals.
- **The budget depth has a step over the width (Q5).** The force over the depth has a flat
  stretch (at 1.2 mm width: 29.2 N from 5 to 15 mm deep). The force never falls, so there is
  one root. When the width lifts the flat level to the budget, the root drops (rivmap: 16.3 mm
  to 5.7 mm at 1.62 mm width). Carried to P27.
- `exact` holds each Stated-phase CurveChip entry twice.
- No Adaptive cell has power or deflection as the first limit on the hobby budget (probe over
  4 presets, 5 tools, 11 materials). The two k2b cells state `force_budget_n = f64::MAX`.
- The viz test p17b lost case C; no test reaches the two-flat-limit path of the sim chart.
- The p5d label test accepts a side place for the move-budget line (both end places are
  blocked by the Rigidity riser and the frame).

### P29-1 status

DONE and committed (see `git log`, "P29-1"). Gates: fmt; the new sentry (5 pass, with an
injected fault that turns it red); 12 named sentries (57 pass); core `--lib` (2795 pass; the one
red is the known timing trap `the_envelope_meets_its_time_budget`, green alone twice); FM1 (the
1000 cells and the simulation subset equal the committed CSVs); full clippy clean. No pin moved.

What P29-1 is: `crates/rs_cam_core/src/feeds/cut_load.rs` (`CutLoad`, `OperatingPoint`,
`NoNumber`), 11 product files that now call it, the sentry
`tests/one_cut_gives_one_load_on_every_door_p29_1.rs` (bit identity per door, a source scan that
keeps direct model calls out). No public signature changed.

Open items that P29-1 names and does not change (they belong to P29-3 or P29-6):

- M4: one sample gives two depths (`axial_doc_mm` for power and chip, `axial_engagement_mm` for
  force and deflection).
- The flute count and the chip differ by gate for one sample. With 3 flutes on the sample and 2
  on the tool: power x 1.31, force and deflection x 0.84, peak chip x 0.67. Each rule is a named
  `OperatingPoint` constructor in `cut_load.rs`.
- Not on `CutLoad` yet: the straight plunge (`close.rs:369`, `operating_point.rs:270`),
  `efficiency.rs:209`, `tool_load/depth.rs` (its own chip), every inverse.

### Rulings in force (the operator, 2026-11-11)

- R1: the force budget (35.3 N) sets the adaptive depth. R3: a slot is a slot under every op.
  R4: entries and cleanup are judged as their own group.
- R2 (lead default, no objection): Suggest keeps a stated stepover and caps the depth; with no
  stepover it proposes the best pair inside the limits, width ceiling 0.5 D.
- R5 (lead default): a rough pass may bend up to the stock it leaves, never above 200 um.
- R6: one entry rule set in one piece of code for every emitter. The proposal and its 17 stored
  sources: `research/entry_rules_2026-11-11.md`. OPEN with the operator: the hardwood ramp angle,
  5 deg (Easel; the lead recommends it) or 20 deg (Shaper). It is needed at P29-8 only.

### Operator rulings, 2026-11-11 night (before the operator went to bed)

- P29-4: "implement P29-4 after the predictions are committed". The lead writes and commits
  the predictions first. The result must match them; a mismatch stops the work.
- Gates: the core gate set (named sentries, core `--lib`, FM1, full clippy) covers P29-3
  commit C and P29-4. No core full gate.
- 3D Adaptive default: "id actually rather geodesic was the default. but thats ok. It just
  seems more consistent and peaks load less." A preference, not an order for tonight. It is
  its own step with predictions (it moves the output of every default 3D rough).
- `trochoid_cap_mult` 1.6 (row P28-13): "I am out of date. you make that call dependend on
  what you think gets best results / makes sense. im not attached to it". The lead decides.
- R6, the hardwood ramp angle: "probably a bit less than soft wood? does that help". Both
  sources agree on the direction (Easel 5 against 20 deg, Shaper ~20 against ~27 deg). The
  words "a bit less" fit the Shaper ratio better than the Easel ratio. Not closed; ask once
  more at P29-8 with the two pairs side by side.

### What the probes mean for the operator's cut

- idx 5 (Project Curve, 6 mm) is 2.7 to 3.6 x over the force budget on about 90 % of its samples
  and cuts up to 14 mm deep. No limit reads it today (P29-5).
- 6 mm rough idx 1: the stated cut is 1.12 x the budget (budget depth 3.95 mm at 1.2 mm). The
  cut reaches 12 mm in places; its cleanup holds the worst force of the op (90.3 N). No gate
  reads cleanup today (P29-6).
- The chip gate's 95 % feed filter drops the body of an op whose stated feed is above the
  modulated feed (operator idx 8: 100 %, and the gate prints "Within" on zero samples).

### Carried items (logged, not in work)

- The 6 mm working file `planning/rivmap_block_job/nz_wanaka_21_6mm_rough_work.toml`: delete idx
  15; simulate with the scallop finish (needs free memory); then move it into the operator file
  with the operator's say and re-grade the acceptance table (PASS 98 / FAIL 0 on `6f586706c`).
- P27 "feeds from fundamentals" (`phases/P27_BRIEF.md`): the operator wants a dedicated session.
- P25 ruling 6; P22f-2e (waits for a conservation check).
- Side defects: section 11 of `P29-0_probes.md` (the drill plunge gate prints Within at 166.7
  against 50; idx 2 engages 17 to 21 mm with a 6 mm tool; the trace feed depends on the number
  of simulation passes), and the list in the memory file
  `project_feeds_contract_state_2026_11_09.md`.
- `.mcp.json` is modified and not committed (the MCP proxy config). Leave it.

## P29 investigation DONE (2026-11-11): the operator reads `phases/P29_FINDINGS.md`

The findings, the design (Cut, cuts(), CutLoad, Check, check_all), the estimate of what moves and
the change list P29-0 to P29-last are in `phases/P29_FINDINGS.md`; the evidence is in
`phases/P29_parts/A_cuts_and_limits.md` and `B_metrics_and_display.md`. Six rulings wait for the
operator (R1 to R6, section 7). After the approval: compact, then P29-0 (probes), then P29-1.

## NEXT STEP (2026-11-11, written before a compaction): investigate, do not implement

Follow `phases/P29_INVESTIGATION_PLAN.md`. The operator wants an architectural fix: every cut is
one object, every limit and metric is a function of it and returns one record, the op type only
decides which cuts exist (cause: `phases/P28_limit_audit.md`, 34 exemptions; the Adaptive family
skips the force budget). Output: `phases/P29_FINDINGS.md` and a change list for the operator. The
session compacts again before any implementation.

## Operator rulings, 2026-11-11 morning (read this first)

1. "Let's use the 6mm": the two bulk roughs move to the 6 mm flat end mill. Agent `compare-runner`
   dials it in on `planning/rivmap_block_job/nz_wanaka_21_6mm_rough_work.toml` (the app's own cut for
   that tool, all 18 ops with Suggest applied, generated and simulated; `phases/P24_6mm_dialin.md`).
   When it simulates clean the lead moves it into the operator file and the table is re-graded for
   the changed ops. The 18 mm bit stays for facing.
2. The row lookup becomes categories (the operator's words are quoted in `phases/P25_DESIGN.md`): a
   tool is what it is (flute form straight or spiral; up-cut, down-cut and compression are kinds of
   spiral that read the generic spiral rows by default); a number is sourced (a row of the category at
   the size) or calculated by a stated rule inside the category; with neither, no number. Agent
   `p21c-impl` writes the design and the measurement of what moves; no code before the operator sees
   that list. This replaces the open series-first question.
3. Spiral force: one force line for every tool stays (the operator has never seen a helix quoted);
   `P24b_survey.md` is closed as "keep helix 0".
`6b4a74f74`: the micro-tool sentry follow-up and the scoped sweep of the core feeds integration tests
(235 binaries, 932 tests pass; no new red beyond the three stale SpeTool pins, all fixed).

## Operator finding, 2026-11-11: a limit that some toolpaths skip (top priority in this session)

`phases/P26_survey.md` found that the Adaptive family reads neither the machine force budget nor the
width rule: a P13c scope decision of 2026-11-05 (`P13.md`: "The Adaptive family keeps
`adaptive_doc_factor`"; code comment "the lead rules it next") that never reached the operator. By
the Pocket rule the operator's 18 mm rough (5.5 x 10 mm) is outside the 35.3 N budget by about 4 x,
and the 6 mm dial-in cut (1.2 x 6 mm) by 56 % in depth. The operator: every limit applies to every
toolpath unless there is a physical reason; an exemption is their decision. In progress: agent
`p21c-impl` audits every op-family exemption (`phases/P28_limit_audit.md`) and sketches a declared
op-family x limit table held by a sentry. Then: the Adaptive family reads the force budget (the
operator's message is the ruling; predictions before it lands). Suggest's Pocket rule gives the 6 mm
bulk roughs 5.12 x 3.0 mm; the lead proposed that cut for the working file; the operator has not
confirmed. Memory: the operator has not yet said yes to clearing `/tmp/claude-1001`.

## For the dedicated modelling session (operator direction 2026-11-11)

`phases/P27_BRIEF.md`: feeds from fundamentals. That session owns the project end to end: it works
out the problem, chooses the approach, builds the model into the product and presents the
verification. The brief gives the outcome, how the result is judged and the rules that do not bend;
the earlier lead's hypotheses are in `phases/P27_NOTES.md`, to use or discard.
P25 rulings 1 to 5 are given (`P25_DESIGN.md` section 8); ruling 6 (the no-row formula) is treated
as "no row, no number" until the model exists. Open at the time of writing: the stepover of the
6 mm bulk roughs (`P26_survey.md`, the Pocket width rule for adaptive ops), the deletion of idx 15
in the working file, and a full simulation with the scallop finish (it needs free memory).

## Morning note, 2026-11-11 (the overnight work)

Landed overnight: `910dcf6d5` P24a (a tool can state `cut_direction = "straight"`; it holds helix 0
and reads the straight-bit rows; 53 Onsrud 48-000 and Freud rows; a straight row serves only a
straight tool, so no existing value moved), `1a34bc612` (two pins the P23c gate list missed),
`1b0273059` (`phases/P24b_survey.md`: a helix-dependent force line; the lead agrees to keep helix 0),
`9e6130439` (`phases/P24_compare.md`: the 18 mm against 6 mm rough comparison, four variant project
files beside the operator file). A regression sweep was clean (core `--lib` 2795 pass + the known
timing trap; the viz suite with only `ic1` red). Running at the time of writing: agent `p21c-impl`
on a scoped sweep of the core feeds integration tests (`phases/SWEEP_2026-11-11.md`).

The comparison in one line: at ceiling 18000 the roughing chain takes 2113 s (18 mm as modelled),
1953 s (18 mm straight), 2008 s (bulk roughs on the 6 mm), 2039 s (straight at 6 mm depth): an 8 %
spread. The 6 mm variant is the only one with a clean simulation headline, no core finding and no
op at the power limit (0.39 of 1.125 kW). Lead reading: `P24_compare.md` section 7.

For the operator to decide:
1. Rough with the 6 mm (variant C) or keep the 18 mm. The numbers favour C: the same time, a third
   of the power, fewer findings. If the 18 mm stays: mark tool 6 straight in the operator file (its
   chip moves 0.179 -> 0.220, which changes the U1 ruling of 2026-11-10 and the table rows of idx 0,
   1, 11, 12, 14).
2. The series-first question (`P24a.md` open item): should a tool's own series always outrank the
   nearest diameter, as the ruling of 2026-10-31 reads?
3. The spiral force line (`P24b_survey.md`): keep helix 0 for every tool (recommended) or let a
   STATED helix read the paper's spiral fits.
4. Facts about the 18 mm bit, if it stays: brand and part number or a photo; a magnet test of the
   body (steel bends about 3 x carbide); measured stick-out and flute length.
Open defects found by the comparison, logged, not chased: one rapid collision on idx 14 at 6 mm
depth (variant D, cause unknown); idx 1 at depth per pass 6 measures a 9 mm engaged depth.
Still owed before any cut: the pre-cut list of `phases/P22f_wanaka.md` sections 4 and 6, and a full
apply + regenerate + simulate of the chosen variant on the final build.

## Lead review after the restart, 2026-11-10 ~12:30

Landed: `ef7e75977` P22b (accepted on wanaka idx 1 with helix: 47060 -> 14628 moves, fed time 558 ->
250 s, the 48 core plunges gone; `P22b.md` section 6), `b361a7b18` the operator's notice.rs fix.
`dce570108` P21c (the constant-chip rule with named limits; the three unsourced RPM caps deleted),
`33d297758` P23a (the chip-thinning lift reads the pass ceiling width), `0c67ad7fe` P23b (the rationale
prints the feed with the RPM), `2d0f7849f` P23c (the SpeTool tapered chart serves 3D finishing; an
exact-size member outranks a size claim). The operator ruled every open table question on 2026-11-10
(`acceptance/wanaka_21_header.md`). `6f586706c` P23d (a contour Pocket feeds its slot-first ring at
the unlifted feed). **Acceptance table on `6f586706c`: PASS 98, FAIL 0, NEEDS-RULING 0**
(`acceptance/grade_6f586706c.md`; 47 / 23 / 28 on the morning of 2026-11-10). Nothing was applied to
the wanaka project: its stored feeds are the old ones; an Apply under constant chip writes the new.
The table grades Suggest numbers, not the job: the pre-cut list is in `phases/P22f_wanaka.md` sections
4 and 6 (555 shank contacts on the 2 mm lake tool, the pin drill past its flute, the V-bit above its
flute, one core descent on idx 14, two entry-load criticals) and a full apply + regenerate + simulate
on the final build is still owed. Research on the 18 mm straight bit at the operator's chip size is
running (agent `bit-research-2`, `research/large_straight_bit_roughing_2026-11-10.md`).
`.mcp.json` (working tree, uncommitted) is back on `rs_cam_mcp_proxy` (proxy -> systemd-run 28 GB ->
`scripts/gui_logged.sh --mcp`): after a release rebuild call `gui_restart`, check `gui_status`
`binary.newer_than_process` is false; no `/mcp` from the operator is needed. Nothing was applied to
the wanaka project: its stored feeds are the old ones; an Apply under constant chip writes the new.

The review's finding on scope (operator concern: the sessions rabbit-hole): the acceptance table
(`acceptance/wanaka_21.md`) is the measure of done and was last graded at `b9e67b24e`: PASS 47, FAIL
23, NEEDS-RULING 28. About 20 phases landed since and moved almost no row. 16 of the 23 FAIL rows are
the RPM rows that P21c fixes. Order from here, by rows closed: (1) P21c; (2) re-grade the table
(P21e); (3) the operator rules the 28 NEEDS-RULING rows in one sitting; (4) the remaining FAIL rows
one by one. A new side phase starts only when it closes a table row or a safety finding of the
operator's cut; anything else is logged in one line and left. Parked under that rule: P22b-1..4,
P22-0c-7, P22f-2b, the straight-flute tool fact (it changes the chip band, so it gets a table row
first), the GUI-thread defect. P22f-2e (the operator's Go) moves no table row and loosens a limit:
it runs only after a conservation check shows the simulated removal summed over an op equals the
stock volume removed (the `mrr` it divides by was never validated; `P22f-2d_DESIGN.md` section 3).

## Restart hand-off, 2026-11-10 ~11:30 (the operator restarts the session for a model update)

**In the tree, uncommitted, not yet gated: P22b** (the helix bore opening for a helix 3D geodesic op;
implemented by agent `p22a-readout`, whose context ends with this session). The spec, the rulings, the
gate results of the run that passed, the edits written after it and the exact gate list to run are
in `phases/P22b.md` (section 5 "State of the tree at hand-off"). The Helix default for new ops is
OUT of the tree as `phases/P22b_helix_default.patch` (P22b-1, apply and measure separately). Also in
the tree, uncommitted: the operator's `crates/rs_cam_viz/src/ui/components/notice.rs` width fix with
its own lib test. Every cargo job waited at the lane's 12 GB gate because the GUI held 9.3 GB after
the wanaka simulation; the session restart restarts the GUI, which frees it.

Next session, in order: (1) rebuild nothing yet; run the P22b gate list from `P22b.md` section 5 with
a fresh implementer (one job at a time, -j 2), review by path, commit scoped to the P22b paths;
(2) viz `--lib notice`, then commit `notice.rs` as the operator's change; (3) P22b-1 (the default
patch, predictions first); (4) P21c (the engine reads the ceiling, the three `Legacy*` limits go,
A3 reported; the operator's answers are in `P21_DESIGN.md` section 9 and 10); (5) P22f-2e (the
operator's Go); (6) the GUI-thread defect on a post-config change (below); (7) the straight-flute
tool fact and the Freud straight-bit rows (`P21_DESIGN.md` section 10). Rebuild the release GUI
after every code commit before a wanaka read.

## Morning hand-off, 2026-11-10 (lead, written during the night run)

Committed overnight, in order: `5a56654dd` P22-0c-5 (every body reader on the mounted zones;
`ToolAssembly` gone; one holder answer on every surface); `ca13c390c` fourteen stale known-red
tests re-pinned (viz suite 1601 pass / 1 red: `the_inspector_text_is_bounded_ic1`, a lead
decision); `34c83e040` `perf_golden_sim_metrics` re-blessed (the bisect named 74af0f8e1, the S11
ruling; not a simulation defect); `125bb15dd` P22-0c-6a and `129deb5c2` P22-0c-6b (one upper
diameter rule, the cutting cap = width at the flute top never above the shaft on both engagement
doors, the ring plunge section `k = 1 - (d/D)^2`, a shank of 0 is not a fact); `6fd852527` +
`36b440b17` P22a (the wide steps on wanaka idx 1 were the 3D "Perimeter sweep", 72 %, not D65;
the exact `FeedCapSource` door); `7ef7a1284` P22f-1 (the nominal-margin arm of `grown_outlines`
deleted: the sweep is gone where the outline grows; idx 1 worst sample 1.98 -> 1.45 kW; Back bulk
rough motion lines 22920 -> 19650). Designs with lead rulings: `phases/P21_DESIGN.md`,
`P22-0c-6.md`, `P22f_DESIGN.md`, `P22f-2_DESIGN.md`, `KNOWN_REDS_TRIAGE.md`. Wanaka controller
`$30` corrected to 24000 (operator). Disk: two `cargo sweep` runs freed 205 GB from target/.

Later in the night: `87bc0bb17` P22f-2a (every fed geodesic step is read on the 3D planner stock;
over `c` up to a slot it is capped, `FeedCapSource::StockRead3d`; the band end 1.45 -> 1.26 kW, not
the 0.91 predicted; four pins blessed by lead ruling, the 46 known G-OPGOLDEN rows unchanged).
Finding 1 (`phases/P22f-2a.md` section 8): the widest remaining idx 1 cut (move 1621, 2.19 kW,
ap 10, ae 17.76) is a step where the planner's own 3D stock reads the load under `c` and the
simulation reads a near-full slot. The planner stock and the simulated stock disagree at the time
of the cut. `7b09f8d88` P22f-2c answered it (`phases/P22f-2c.md`): there is NO stock gap. Both
stocks see a 1 mm layer across the whole width plus a side band 3-10 mm deep. The planner reads
the section (0.06-0.36, about `c`). The simulation gate reads the deepest cell times the widest
arc (ap 10 x ae 17.7 = 2.19 kW), and the sample's own removal is 4-12 % of that rectangle. So the
"band" powers of P22a, P22f-1 and P22f-2a are in part a rectangle reading of a layered cut, and
the move-end samples (removal ratio 2-6, a segment-time artefact) inflate the "real removal"
counts. P22f-2d (`phases/P22f-2d_DESIGN.md`) re-read every wide sample by section (mid-move
samples, `mrr / (F / 60)` against the pass section `c D dpp`): **0 of the 3731 idx 1 samples are
wide by section**; every class's worst section sample is at or under 0.80 kW; the planner's own
read was right. So the app today slows moves that are not heavy: the P20g modulator and the P20i
envelope read a rectangle (deepest cell x widest arc) 8 to 25 times the removed section at the
band. Lead rulings in section 4: option (B), a measured `EngagedSection` variant for simulated
samples through the same power and force doors, the chip readers keep the arc; this is
**P22f-2e** and **waits for the operator's Go** (it moves every simulated power and deflection
verdict, the envelope count, the modulated feeds and F words, the FM1 sim cells; no
acceptance-table row). P22f-3 is dropped; P22f-2b waits for a section read of ops 20/21.
P21a committed (the one RPM-choice door `calculator/spindle.rs::rpm_choice`,
`FeedsResult::rpm_choice`, the limits `LegacySpeedup`, `LegacyHeadroom`, `LegacyTier`,
`VendorRpmMax`; byte-identical, FM1 unchanged; sentry `every_rpm_cap_is_a_named_limit_p21`).
`f06bde4e2` P21b (the strategy rename `MaxSpeed` -> `ConstantChip { rpm_ceiling }`; the file
holds two flat keys of `[job.post]`, `spindle_strategy = "constant_chip"` and
`spindle_rpm_ceiling`; `max_speed` refuses at load and names the new key, ruling Q16, the 39
planning files that hold it are not migrated; a ceiling of 0 refuses, one above the spindle
maximum warns with one sentence on every surface; the engine does not read the ceiling until
P21c; the MCP wire snapshot is the one bless, a stated schema break; no numeric pin moved; lead
rulings in `phases/P21b.md` section 5: the residual names `MaxSpeedPolicy` and
`step2b_max_speed` go to P21c). The release GUI binary was rebuilt after P21b for the wanaka read
(check its mtime against HEAD before you trust a GUI number; the running GUI needs `/mcp` or a
`gui_restart` to pick it up). Then: the wanaka read, P22b, P22-0c-7; P21c after the Q1/Q6/Q7
answers.

Operator answers, 2026-11-10 morning: (1) Go on P22f-2e ("if it is more correct, 100 % do it");
(2) yes, constant chip on wanaka with the ceiling 24000; (3) idx 1 goes to helix; (4) the acceleration
rule: "not sure", open (A3 ships in P21c, the share is shown first). The GUI restarted on the fresh
binary at 09:10; the wanaka read runs from it (`phases/P22f_wanaka.md`: the zone check reports 555
real shank contacts on the 2 mm lake pockets; constant chip under the P21b legacy caps LOWERS the 18 mm
roughs to 14000 rpm, so no Apply before P21c; idx 1 helix today plunges the core 7 mm on 48 entries,
the P22b case; idx 1 cuts at the chart feed 6450, the chip cap binds). The 18 mm tool is a
straight-flute router bit: `research/large_straight_router_bits_2026-11-10.md` and `P21_DESIGN.md`
section 10 (the 14000 tier has no source as a cap; a Freud straight-bit chip band exists; a tool
edge-geometry fact is a new item after P21c).

Open defect (2026-11-10, no fix landed): a post-config change (`SetPostConfig`, for example MCP
`set_spindle_strategy`) on a project with a simulation held blocks the GUI main thread for minutes
(wanaka: about 8 min, the depth gate log line every 11 s, 35 times for 18 ops). The operator's own
agent read it as far as: the tool-load report is rebuilt on the GUI thread, and the rebuild evaluates
both the stock and the own-material populations, so one op runs the depth gate more than once; it
ran out of budget before a fix. Fix shape: the F216 memo takes the policy as an input and reads the
gates from the stored simulation result, or the rebuild moves to a worker lane. In the tree,
uncommitted and not yet gated (the cargo lane waited on memory while the GUI held 8.9 GB): the
operator's `crates/rs_cam_viz/src/ui/components/notice.rs` empty-state width fix with its own lib
test `an_empty_state_respects_its_parent_width`; run viz `--lib notice`, then commit it as the
operator's change.

Operator decisions that were waiting (plain words in `P21_DESIGN.md` section 9 and the lead's morning
summary): (1) the wanaka project setting `spindle_strategy = "constant_chip"` with
`spindle_rpm_ceiling = 24000` (needed for the 16 rpm acceptance rows; P21c); (2) the P21
acceleration rule after the A3 share is visible on wanaka; (3) P4 (the ball feed lift, the taper);
(4) wanaka idx 1 (Back bulk rough 18 mm) states `entry_style = "ramp"`: under ruling Q1 the P22b
helix bore opens only a helix op, so idx 1 keeps its ramp and its 79 capped departure-window
samples until the operator sets it to helix (`P22_DESIGN.md` section 11, `P22b.md` section 3).


HEAD at the time of writing: `a52e88813` (P20d). The first edition of this brief (`02b832eb9`) is
done in part: section 1 of that edition became P20a and P20b.
The operator wants every feeds and speeds number to be correct and explainable, so that the app
can be shared. GUI work waits.

## Rule for this session: the app is under test

- A phase is done only when its rows in the acceptance table pass:
  `acceptance/wanaka_21.md` (and `.toml`, `README.md`, `derive_wanaka_21.py`; commit `a25ff317d`).
  A fresh agent grades the build. The table changes only by an operator ruling, never to match
  the app.
- Gates (clippy, G-OPGOLDEN, viz suite, FM1) prove only that nothing broke. FM1 is a regression
  pin, not a truth source. Do not call a number "correct" from a gate.
- Measure the emitted G-code, not the plan (see section 1).
- Protocol: the lead decides and verifies; agents implement and never commit. Every cargo command
  goes through `scripts/cargo_lane.sh` in `timeout`. Lead gate script: per phase, clippy,
  G-OPGOLDEN (46 known rows, `opgolden_known_46_rows.txt`), the viz suite (9 known red binaries
  including `the_selection_section_writes_edits_only_p4_3`; `g_pipesmoke` can SIGSEGV as a
  flake), fmt (3 known files). FM1 re-blesses only after the predicted cells are listed.

## Done on 2026-11-08

| Commit | Content |
|---|---|
| `998ac93a3` | P20a: the unsourced drill RPM tier is removed; drill feed ceilings from the 3 mm Spektra row |
| `a25ff317d` | The wanaka acceptance table: 108 rows; build 998ac93a3 PASS 31, FAIL 12, NEEDS-RULING 55 |
| `0a6e78116` | P20b: `feeds/vertical.rs` `VerticalRateLimit` (G6 axial chip and Z axis rate) for plunge, drill, ramp |

Records: `phases/P20.md` (audit table of 20 rows, P20a and P20b sections),
`phases/P20_chip_gate_abstain.md` (the chip gate finding, section 1 below).
The release GUI runs 998ac93a3 (not rebuilt for P20b; the operator said "rebuild later").

## Also done late on 2026-11-08

| Commit | Content |
|---|---|
| `42977948d` | P20c: `rate_bound_feed_mm_min` caps every fed move at the per-axis rate for its direction (one stage after the last feed writer); gates grade the reached feed (`use_predicted_feed_in_gates` = true) |
| `a52e88813` | P20d: `feeds/section.rs` `EngagedSection` (rectangle, V groove, axial circle); V-bit power from the cone; drill power advisory; `power_kw` is an Option (item 3b) |

Records: `phases/P20.md` sections 4 (P20c survey of feed writers and graders) and 5 (P20d).
The GUI is still 998ac93a3: rebuild, then re-grade the acceptance table (P20b, P20c, P20d rows).

Open from P20c/P20d (operator rulings): drill power stays advisory, or cite the MDF twist-drill
torque text (`planning/extrapolation_2026-09-24/fetch/G8/sources/materials_2019_mdf_drill_edge_radius.txt`;
the model reads 0.6-0.9x it); the drill power hover labels (ap is advance per rev, ae the
diameter); an operator-typed plunge and the axial-chip bound; the 2 mm 1F plunge (240, unsourced;
G10 refuses below 3.0 mm and 1 flute; Spektra prints 2F Ramp Down at 1/16 and 3/32 in).
Open in code: a `ReachableFeed` type (about 450 writer sites); the drill gate and plunge stress
read the op config, not the emitted move; accel reach is graded but not written; the drill
engagement floor still writes ap 0.05, ae 0.02; deflection still reads the rectangle on a V-bit.

## 1. Top finding, not yet fixed: the feed cap of 3000 and the chip gate

- Every toolpath in `planning/rivmap_block_job/nz_wanaka_21_operator.toml` has
  `feed_max_rate = 3000.0`. The modulator applies it as a hard cap (G-FEEDMAX). An export on
  2026-11-08 (three per-setup files) has no feed above F3000. The operator's 8475 mm/min on
  "Back bulk rough 18 mm" runs at 3000 (chip 0.083 mm, not 0.235 mm).
- The app does not tell the operator: the feeds card shows 8475, and the power gate reads the
  uncapped feed (it reports 1.25 kW, which is wrong for F3000).
- The chip gate abstains on every op with `feed_max_rate < 0.95 x feed_rate`
  (`tool_load/chipload.rs:223,234` compares the samples with the uncapped `feed_rate`), so it
  says "not enough steady cutting" on all roughs. Fix in `P20_chip_gate_abstain.md`: one
  reference feed `min(feed_rate, feed_max_rate, cutting ceiling)` for every gate. This is the
  same class as the P16 "one gate feed" rule. Do it before item 4c.
- Ask the operator where the 3000 came from (an old GUI default?) and if it is wanted. Under the
  "no underived defaults" ruling a written 3000 with no derivation is a defect.

## 2. Operator rulings (2026-11-08)

- P21 rebuilds the existing "Max speed" spindle strategy as the constant-chip strategy, at the
  Suggest level (one RPM per op; a VFD cannot change speed along a path).
  - It refuses without stated machine axis rates and accelerations.
  - It starts conservative: an operator RPM ceiling field; empty = no rise above the chart RPM.
  - No unexplained multipliers: `MAX_SPINDLE_SPEEDUP` 1.5, the milling RPM diameter tiers and
    `SPINDLE_CEILING_HEADROOM` 0.95 go. Every cap is a named limit (spindle max, power at that
    RPM, XY rate, Z rate, acceleration on the modulated moves, the operator ceiling).
  - Later: one "N % of the theoretical limit" setting (the old aggressiveness dial), applied to
    the binding limit and stated on the card.
- With a constant-torque VFD, power never binds a constant-chip RPM rise (required and available
  power both scale with RPM). On wanaka the spindle max binds every milling op; the Z rate binds
  the drills at 2500 / (0.0635 x 2) = 19685 rpm.
- The 10610 rpm on the 6 mm Onsrud-row ops (wanaka 5, 17) comes from the unsourced
  `material.base_cutting_speed_m_min.solid_wood`. The Onsrud chart prints a chip only. Fix it
  inside P21 (the RPM is a policy choice), not as a separate patch. The same 10610 sits in the
  project's `[job.post] spindle_speed`.

## 3. Open rulings in the acceptance table (ask the operator)

P1 RPM policy (mostly ruled above); P2 which chart row stands for the unbranded 6 mm tool;
P3 adaptive chip lift from the nominal stepover or the 1.6x engagement cap; P4 lift the ball feed
for axial thinning at a shallow depth (`calculator/feed.rs:97-98` says "observed, not applied").

## 4. Other open defects (from the acceptance table and the critique)

- Scallop (wanaka 16): SpeTool prints 0.0762 mm for a 2 mm tip; the app ships 0.0644 mm.
- The 6 mm tool gets two chips in the same oak (0.127 and 0.149 mm).
- A false alarm: "plunges faster than the plunge rate" at exactly 1.0x on the drills.
- The envelope message says 102 % where the power ratio is 111 %.
- Unsourced limits on the card: the 2 x D depth rule, the 88 % long-tool target, the vendor axial
  bound, the peck depth default (audit row 6), the drill floors (row 5).
- Machine file: `controller.rpm_max = 1000` while the spindle is 6000-24000; the operator checks
  grblHAL `$30`.
- The earlier list still holds: item 4c, BandMid, D2 (blocked on the Onsrud 60-100MW rows),
  D3/D6, D4, D13, D5, the HighAirCut headline.

## 5. The operator's cut (not yet cut)

Safety read on 998ac93a3 (generate all, simulate, export warnings): no collision, no rapid
collision. After that read the operator fixed some lakes in the project (2026-11-08,
uncommitted edits to the TOML), so regenerate, re-simulate and re-read the per-op warnings
before any verdict. The acceptance table rows are per toolpath and stay valid unless a toolpath,
tool or feed changed. Before the cut:

- The F3000 cap (section 1): the operator must decide whether the roughs run at 3000 or at the
  card feed. At the card feed, "Back bulk rough 18 mm" at 8475 needs about 1.11-1.25 kW of
  1.125: lower it to about 7000.
- Entry load (critical) on "Back rest 6 mm" (peak 15.9 mm at 94.1, 89.1, 8.84) and "Front 3D
  rough" (7.5 mm at 343.7, 36.8, 17.5): look at both on screen.
- Wood above the flute (export warnings): "Pin drill, flip pins" 30 mm over a 25 mm flute, "V
  rivers and lakes" 9.4 mm over 8.15 mm, "Day strip slit V60" 7.4 mm over 5.5 mm. Check each.
- Three ops emit feeds modulated from an older simulation run: regenerate them before export.
- `$30` (above).

## 6. GUI (later, low priority)

P17b open items in `phases/P17.md`; the explainer page `explainer_p13c/` has stale claims.
