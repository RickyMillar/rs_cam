# P30: the modulation design step

Written 2026-11-14 at HEAD `b8dd6c739`. No product code changed. The operator decides this step.

The operator, 2026-11-12: "simulation should run whats going to happen in real life. having a
post-sim change seems not ideal.. we can work on that".

Words in this note:

- **Stated feed**: the feed of the op (`feed_rate` in the op table, the Passes card).
- **Dressed feed**: the feed of each move after generation and the dressups. Feed optimisation
  (a dressup, `feed_optimization`) can change it.
- **Modulated feed**: the feed of each move after the feed modulator ran on a simulation.
- **Emitted feed**: the F word in the G-code.
- "Measured" = a number from a test run in this step. "Reasoned" = read from the code, not run.
  "Stored" = a number from an earlier note, with its source.

## 0. The answer in short

1. **Safety question.** Yes. In the default state (simulated, modulation on), the G-code carries
   feeds that no surface prints as a number. On the Pocket fixture, 48 of 63 fed moves run above the
   stated 1000 mm/min, up to 3000 mm/min (measured, section 2). The only numbers that a surface
   prints are a median ("Feed vs commanded +45 % median") and the share of each binding limit.
   The machine stays inside its own limits: no modulated feed passes the op ceiling
   `min(machine cutting ceiling, feed_max_rate)` (section 3).
2. A second gap goes the other way. The limit rows say "before modulation" and soften to
   caution **whether or not a simulation modulated the toolpath**. They read a GUI constant, not the
   feed stage of the result. An export with no simulation then carries the stated feed that the row
   called "over", in the caution tone (section 3, case S0).
3. The simulated stock is already true: the dexel carve reads no feed. What is not true after the
   simulation is the feed of each move, the time, and the load cards. So the sentence of the
   operator asks for one thing: the feeds that the simulation judges must be the feeds that the
   export writes, and the op must state them before the cut.
4. Recommendation (section 5): **B now, then A**. First make every surface state the emitted feed and
   the feed stage (no feed moves). Then move the modulator into the generation plan as its own
   "feeds" step that reads the stock before the op, so the generated toolpath is what runs and the
   simulation only verifies.

## 1. What happens today, as a timeline

| # | Stage | Feed of each move | Who can change it | What each surface shows | Code |
|---|---|---|---|---|---|
| 1 | Op stated | stated feed, plunge, ramp, lead feeds | the operator; Suggest Apply (`Command::ApplyFeeds`) | Passes card: the stated feed and the stated rows | `feeds/CLAUDE.md` (apply is THE write) |
| 2 | Generate | the op feeds on each move; `Move::feed_cap` on capped moves | the generator | toolpath header: move count | `compute/execute.rs` |
| 3 | Dressups | feed optimisation (on by `DressupConfig::default`) lifts or lowers each move by the chip thinning factor, on a fresh block of stock at cell D/4; it never runs for a rest op or a 3D op | the dressup | **nothing prints the dressed feed** (A) | `compute/config.rs:1557`; `compute/execute/dressup_apply.rs:353-366, 1321-1375`; `session/compute.rs:1008-1029`; `compute/catalog.rs:2434-2449` |
| 4 | Machine caps | the op ceiling `min(cutting ceiling, feed_max_rate)` caps feed optimisation and the modulator; the cutting ceiling is the stated cutting feed, else the X/Y axis rate (P20c `$$`), else the travel rate | the machine profile, the op dial | Machine panel | `machine/mod.rs:1010-1038` |
| 5 | Store | slot `ResultProvenance::Dressed`, stage `FeedStage::Planned` | - | header hover "feeds: as planned, not modulated" | `session/result_provenance.rs:39-52, 151-175` |
| 6 | Simulate | the dexel carve reads geometry, not feed; samples carry the dressed feed and its segment time | - | the stock, the trace runtime | `compute/source_stock.rs:27-30` |
| 7 | Modulate (option on; GUI and MCP always on) | ConstrainedMax: each engaged move takes the lowest of the chip cap, power, deflection, force budget, op ceiling, kinematic reach; floor at the band minimum; `Move::feed_cap` last. It can **raise** a move far above the stated feed | the run | see row 8 | `session/compute/simulation.rs:300-349, 634-643, 709-863`; `dressup/feed_modulation.rs:857-1117, 1594-1640` |
| 8 | Store after modulation | the slot is rewritten in place: `ResultProvenance::Modulated { simulation_run, source }`. No dressed copy is kept | - | header hover and G-code header: "feeds: modulated from simulation run N"; Feeds card: "Limited by", "Feed vs commanded" (median against the stated feed), "Modulated n / m cuts"; viewport feed colour: feed / stated, **clamped at 1.5** (B); sample gates read the modulated feed (`predicted_feeds`); limit rows read the GUI constant (C) | `simulation.rs:855-861, 1021-1038`; `rs_cam_viz/src/ui/properties/feeds_speeds.rs:1114-1160`; `toolpath_panel/header.rs:203-205`; `rs_cam_viz/src/render/toolpath_render.rs:589-600, 697-699` |
| 9 | Time | with a `[machine.kinematics]` block: re-integrated on the modulated feeds. Without one (every preset): the trace keeps the dexel time at the dressed feeds (N7) | - | timeline, Readiness, export wizard: one cycle time (D) | `simulation.rs:902-1020`; `session/cycle_time.rs:368-401` |
| 10 | Stale | (a) an edit of the op drops its result: the export leaves the op out; a regenerate gives `Planned`. (b) an edit upstream changes the stock before the op: `ModulatedSuperseded`, old feeds stay in the slot | the session | (b) an export warning and the header clause "the current simulation is run M" | `result_provenance.rs:56-73, 121-125`; `session/compute/export.rs:562-582` |
| 11 | Export | the slot as it stands, any stage | - | G-code F words; one header line per op with the stage | `session/compute/export.rs:534-538`; `rs_cam_viz/src/io/export.rs:233-268` |
| 12 | Post clamp | an F word above the post's `limits.max_feed` becomes that value, with a G-code comment "WARNING: requested F.. clamped". The shipped posts leave `max_feed` unset | the post file | the export wizard line "Post limit: max feed .. (per-move clamping)"; the G-code comment | `gcode/emitter.rs:93-107`; `gcode/post.rs:169-184`; `rs_cam_viz/src/ui/export_wizard.rs:1226-1234` |

Stale class checked (measured, probe 2): a machine kinematics edit after a modulated run drops the
result and the simulation (stale set `{0}`), so an edit of the machine limits cannot leave old
modulated feeds under a "current" header. Edits of the tool or the stock material were not
probed; the dependency edges (`session/dependencies.rs`) are the reasoned answer.

Places where two surfaces show two feeds for one move:

- **(A)** Feed optimisation changes the move feed at generation. The Passes card, Readiness and the
  pre-simulation cycle time (`cutting_distance / stated feed`, `cycle_time.rs:394-398`) read the
  stated feed. On the 2D Adaptive fixture 876 of 1091 fed moves leave generation above the stated
  1500 mm/min, up to 3000. With `feed_optimization = false` the same fixture has 0 moves above the
  stated feed (both measured; probe 2).
- **(B)** After modulation the Passes card shows the stated feed; the G-code holds the modulated
  feeds; the Feeds card shows one median; the viewport colour cannot tell 1.5x from 3x.
- **(C)** A limit row reads `RunModulation` from the constant `FEED_MODULATION`
  (`rs_cam_viz/src/state/simulation.rs:46-58`, readers `:344`, `:503`, MCP
  `app/mcp/generation.rs:1236, 1257`). It never reads `feed_stage`. So a row says "before
  modulation" on a `Planned` slot, and the export writes the stated feed.
- **(D)** With no kinematics block the cycle time is the dexel time at the dressed feeds
  (`CycleTimeBasis::SimulatedNoAccel`); the G-code runs the modulated feeds.
- **(E)** The samples hold the first run's feed, chip and segment time. The gates read the modulated
  feed through `predicted_feeds` (`simulation.rs:1036-1070`); the force card weight
  (`segment_time_s`) and the sample chip field do not (P29-6 note section 8, item 3; not a census).

The doors that set the option:

| Door | Modulation | Code |
|---|---|---|
| GUI run, MCP `run_simulation`, GUI Generate All (each `SimulatePrefix` and `SimulateAll` step drains through the same adopt) | always on, ConstrainedMax, scale 1; no switch | `rs_cam_viz/src/controller/events/compute.rs:971`; `controller.rs:411-416` (reasoned: one drain path) |
| CLI `project`, `rough_score` | on by default; `--no-adaptive-feed-modulation` | `rs_cam_cli/src/main.rs:378, 700, 760`; `project.rs:1191-1197` |
| Core `SimulationOptions::default()` | on | `session/mod.rs:1725, 1777` |
| CLI `suggest` | no simulation; its rows state the default (on) | `rs_cam_cli/src/suggest.rs:170-180` |

## 2. The facts in numbers

### 2.1 Measured on the sentry fixtures

Source: a throwaway test (copy `scratchpad/p30/zz_p30_probe.rs`, output `probe_output.txt`; deleted
from the tree) on the four fixtures of `tests/common/kind_fixtures.rs`. Each fixture: a 6 mm end
mill, 18000 rpm, `feed_max_rate` 3000 (the op ceiling), no kinematics block. "Cycle" is
`compute_cycle_time_breakdown` on the result toolpath with the generic kinematics fallback (the
clock of the re-time, `simulation.rs:925-930`); "trace" is the time the sim surfaces print. "Median
 change" is `ModulationSummary::median_feed_delta_pct`: the median over the touched moves, against
the stated feed (`feed_modulation.rs:753-811`), the number of the Feeds card. 14.7 s.

| Fixture | Stated | Fed moves | Median change | Binding: moves (lowered / raised) | Moves above stated: before / after | Max feed before / after | Cycle before / after | Trace time on and off |
|---|---|---|---|---|---|---|---|---|
| Pocket, ramp | 1000 | 63 | +45.2 % | FeedCap 8 (0/0); KinematicReach 26 (2/24); MachineMaxFeed 26 (0/18) | 6 / 48 | 1033 / 3000 | 63.56 / 48.77 s (-23.3 %) | 51.40 s, no change |
| Profile, tabs, leads | 1000 | 46 | +200 % | KinematicReach 2 (0/2); MachineMaxFeed 31 (0/29) | 3 / 31 | 1004 / 3000 | 36.21 / 20.08 s (-44.5 %) | 32.29 s, no change |
| 2D Adaptive, helix | 1500 | 1091 | -2.3 % | KinematicReach 864 (570/294); MachineMaxFeed 219 (0/135); PlungeRate 6 (0/0) | 876 / 525 | 3000 / 3000 | 159.16 / 148.00 s (-7.0 %) | 95.63 s, no change |
| Shelf 3D rough | 1500 | 993 | -4.5 % | FeedCap 132 (0/0); KinematicReach 658 (235/423); MachineMaxFeed 181 (0/56); PlungeRate 19 (14/0) | 0 / 433 | 1500 / 3000 | 163.99 / 147.97 s (-9.8 %) | 129.82 s, no change |

- No move bound on the chip cap, the chip floor, power, deflection or the force budget. These
  fixtures cut light; the 3x raise comes from a stated feed far under the chip band. **Do not read
  it as typical.** It shows what the code allows: up to the op ceiling.
- A second simulation with modulation on moved 0 moves again: the modulator is idempotent on these
  fixtures (an engaged move does not read its own feed, `feed_modulation.rs:857-1117`).
- A simulation with modulation **off** after one with it on keeps the modulated slot: the stage
  stays `ModulatedCurrent { run 2 }` and the G-code equals the modulated export (measured on all
  four). Only the CLI and library callers can reach this state; the header names the run.

### 2.2 Stored: where the load caps bind

| Source | Fact |
|---|---|
| `P29-10i_DESIGN.md` section 11, "Q2: the measurement" (operator file, GUI `generate_all`, stored load report) | idx 1: chip cap binds on 27.6 % of moves, median -23.9 % (8475 to about 6450 mm/min); idx 2: 32.3 %, -13.4 %; idx 7: 83.3 %, -12.7 %; idx 8: 25 %, **median +508.8 %** (light moves raised); idx 14: 32.2 %, -20.0 %; idx 15: 36.7 %, -15.8 % |
| the same table | stated chip max rows: idx 1 body 0.2169 against 0.1791 mm (-21.1 %); idx 2 planner cap -2.9 %; idx 7, 8 body -27.0 %; idx 14 -3.4 %; idx 15 -3.1 %. Every one reads `within`, "peak at limit", after the simulation: **over as stated, within after** |
| `P29-11_DESIGN.md` 3.4 (fixtures, modulation on) | dome drop cutter (taper): ChiploadMax 25438 moves; dome 3D rough: FeedCap 467, KinematicReach 1462, MachineMaxFeed 384; no move on PowerMax |
| `NEXT_SESSION.md` P29-4 status | P29-4 R1: after a simulation, the exported feed of an Adaptive move over the force budget is lower; the depth stays. Wanaka op 5 is up to 3.6x over the budget (reasoned there, not simulated) |
| `P29-6_DESIGN.md` section 8 item 5 | with 6e, wanaka cleanup and link moves over the budget (286, 1 212, 235 samples) would be lowered after the simulation |
| `NEXT_SESSION.md:707` | "Three ops emit feeds modulated from an older simulation run: regenerate them before export." A real `ModulatedSuperseded` case on the operator file |

The operator file was not simulated in this step (one such run OOM-killed the GUI at 15 GB). Its
cutting ceiling is 15000 mm/min (`max_rate_xyz_mm_min` X/Y, `nz_wanaka_21_operator.toml:84-88`, no
`feed_max_rate`), so a raised light move can reach the chip cap or 15000, whichever is lower
(reasoned). All 18 ops have `feed_optimization = false` (read-only grep).

## 3. What export does in each state

| State | Slot feeds | Stage and header | Rows | Shown as a number? |
|---|---|---|---|---|
| S0 never simulated | dressed (stated, or feed optimisation) | `Planned`, "feeds: as planned, not modulated" | ChipMax rows on re-timed cuts say "before modulation" and soften `Exceeds` to caution (C) | stated feed yes; feed-optimised feeds **no** |
| S1 simulated, modulation on | modulated | `ModulatedCurrent`, "modulated from simulation run N" | the same constant caveat; gates `within` at the cap | median and binding share only; **max feed and moves above stated: no** |
| S2a then an edit of the op | none; after a regenerate, dressed | left out with `export.op_left_out`; then `Planned` | as S0 | as S0 |
| S2b then an edit upstream | the old modulated feeds | `ModulatedSuperseded`, a warning line, "the current simulation is run M" | as S1 | the warning says the feeds are old; not which feeds |
| S3 modulation off (CLI flag) on a fresh session | dressed | `Planned` | the CLI states `Off`; no caveat | as S0 |
| S3b off after on, same session | the modulated feeds stay | `ModulatedCurrent` of the old run | the CLI states `Off` | header only |

**The central safety question.** Is there a state where the G-code carries feeds that no surface
showed the operator? Yes, two:

1. **S1 (the default).** Measured on the Pocket fixture: the export before the simulation has 23 F
   words, maximum F1033; the export after has 51 F words, maximum F3000; 48 of 63 fed moves run
   above the stated 1000. The Passes card shows 1000. The Feeds card shows "+45 % median" and
   "Limited by" a share. The viewport colour clamps at 1.5x the stated feed. No GUI, MCP or CLI field
   prints the largest emitted feed or the count of moves above the stated feed. Read: every
   `mm/min` print of `rs_cam_viz/src/ui`; `render/toolpath_render.rs`; the header hover
   (`CapFigures::line`: moves at a load cap and the lowest cap, `state/runtime.rs:227-252`);
   MCP `get_cut_trace` (summaries, issues, `capped_min_feed_mm_min`,
   `app/mcp/simulation.rs:487-760`) and its `feeds` stage key (`:61-66`); `trace/narrate.rs:555-557`;
   `ModulationSummary` (`feed_modulation.rs:802-810`); the CLI `project` report (on or off only,
   `rs_cam_cli/src/project.rs:1235-1330`); `compute/toolpath_stats.rs` (no feed field). Not read in
   full: MCP `inspect_spans` (section 7).
2. **S0 with feed optimisation on.** The dressed feed is not printed anywhere (2D Adaptive fixture:
   876 moves above the stated feed, up to F3000).

Bounds that hold in every state (reasoned): a modulated or optimised feed never passes the op
ceiling (`simulation.rs:798`, `dressup_apply.rs:1338`), a move's `feed_cap` binds last, a
geometric plunge is capped at the op plunge rate (`feed_modulation.rs` module doc, "the Phase 3
plunge guard"), and an engaged move with a band never passes `band max x lift x rpm x Z`. So the
G-code never asks more than the machine profile and the op dial allow. It can ask far more than the
operator typed.

The opposite direction (C): rows soften by the run constant, not by the stage. In S0 the stated
ChipMax row of operator idx 7 and 8 reads "body · chip max 27 % over limit · before modulation" in
caution (`P29-10i_DESIGN.md` section 14). If that file is exported with no simulation, the G-code
runs the stated chip, 27 % over the band, under a caution line (reasoned from the row rule and the
export door; not exported here).

## 4. Options

Every option keeps the dexel carve as it is. "Rows" = the exemption rows of `limits/exemptions.rs`.

| | A. Modulation is a generation step | B. Stays in the simulation; every surface states it | C. Modulation proposes, the operator applies | D. Status quo with the caveat | E. Lower only | F. Off by default |
|---|---|---|---|---|---|---|
| What the operator sees | the op card shows the feeds that run (range, max, binding share) right after Generate | the stated feed plus, after a simulation, "runs at" min / median / max and the count above stated; rows read the stage; export warns on a stale or absent stage | a proposal "the simulation suggests: max X, median Y, cycle -Z %" and an Apply button; rows read the stated feed until Apply | as today | as today, but no feed above the stated feed | the stated feed is the emitted feed; modulation is a tool |
| What the machine runs | the generated toolpath, unchanged by the simulation | the modulated slot (as today) | the stated feeds, or the applied proposal | the slot as it stands | the dressed feeds, lowered where a cap binds | the dressed feeds |
| Does the simulation then run what happens? | **yes**: it simulates the stored toolpath and changes nothing | no: it still changes the feeds, but says so everywhere | yes until Apply; after Apply, yes | no | no (it still lowers after) | yes |
| Code that moves (reasoned size) | large: a plan step "Feeds" per op after its prior stock exists (the rest-op path: `generation_plan.rs`, `SimulatePrefix`), the per-op carve on a copy of the prior stock, the modulator call, a `Modulated` slot keyed by `SourceStock` (exists, `result_provenance.rs`); delete the post-pass (`simulation.rs:709-1094`, 386 lines) and its two calls; estimate 800 to 1500 lines changed, 3 to 5 commits | small: one summary field (max, count above stated), the row reads `feed_stage`, an export warning for `Planned` under a run that modulates and for `ModulatedSuperseded` (exists as a caution); estimate 150 to 300 lines | medium: keep the dressed slot beside the modulated one, a `Command::ApplyModulation` row, GUI and MCP doors, rows on the stated slot; estimate 400 to 700 lines | none | small: `constrained_max_move` takes `min(result, dressed feed)`; estimate 20 lines plus tests | small: the default and the GUI constant; estimate 30 lines |
| Rows | No row closes by the option alone; each needs its own change: `P28-16` (the modulator reads `deflection_limit_um`, 7d Q4), `P28-29` (6e: a lead takes `min(stated lead feed, load caps)`), `P28-30` (a plunge cap from the P29-8 planned engagement). A removes the reason for the hold: the change is seen before the cut. The caveat gaps (link, peck ladder, bandless, own material, disabled op) go only when the caveat goes | as A, but each change still moves a feed after the simulation | as A | the holds stay | none closes | the caveat and its gaps go; the three rows lose their reader |
| M4 | M4-A (one depth) moves a generated feed, shown before the cut: the objection to M4-A no longer holds | M4-A moves an exported feed after the simulation, now stated | M4-A moves only a proposal | M4-A1 or a ruling | M4-A1 | no feed to move |
| `S-NoSampleGate`, `P28-21` | independent of the option: D-1 (two gates from the band) then the plunge rate gate; `P28-21` closes when the envelope finding judges every group | the same | the same | the same | the same | the same |
| Cycle time | none lost (same feeds, earlier) | none | lost until Apply; fixture gain was 7 to 45 % | none | the raise share is lost: on the fixtures most changed moves were raised (Pocket 42 of 44) | all of it |
| Generation time | each op waits for the stock before it: one prefix simulation per op. The S5 memo resumes the carve, but its snapshot is taken before the end-of-group work (`sim_prefix.rs:21-25`), so each step still pays its mesh and deviations: one carve in total plus one tail per op (not measured; for scale, one wanaka200 simulation at 0.4 mm took 84 s, `sim_prefix.rs:13-15`); an upstream edit re-runs the feeds step of every later op (geometry stays: `carve_digest` reads no feed, `source_stock.rs:27-30`) | none | none | none | none | none |
| What can go wrong | a big project needs a simulation to finish Generate (memory: the 15 GB OOM); a fresh-stock op must read the true prior stock, not its own block (a finish after a rough would read full stock and be lowered to the chip floor); a feeds step that is stale must stop export or the export carries old feeds | the post-sim change stays; the operator objection stands | per-move feeds are lost when the op regenerates: an applied proposal must be keyed to the geometry, or every edit drops it | the gaps of section 3 stay | the stated feed becomes the only control; the chip floor is a raise, so E drops it too: a light cut stays under the band (rub and burn) | the machine runs a stated feed that a cap would lower |

The three symptoms of the task, against each option:

| Symptom | A | B | C | D | E | F |
|---|---|---|---|---|---|---|
| The op "as stated" is not what the machine runs | the op card states the generated feeds; the stated feed stays an input, not the run | stated beside "runs at" | true until Apply | stays | stays (lower only) | gone |
| A row reads "over" before and "within" after | the split moves earlier: a stated row is over before the feeds step and within after it, both before the cut. It goes away only if the rows judge the generated feeds, not the stated feed | stays, now stated by the stage | gone until Apply | stays | stays | gone |
| An export with and without a current simulation carries different feeds | gone: the feeds step, not the simulation, sets them; a stale feeds step is the one case (question 3) | stays, with a warning on both | gone | stays | stays | gone |

Notes on A:

- **The stock model.** The modulator needs the engagement of each move on the stock that the
  machine meets. That is the stock after every earlier op of the setup chain, with the stock
  changes: the same snapshot that a rest op reads today (`prior_stocks`, recorded by `SourceStock`,
  `source_stock.rs:1-17`). Feed optimisation today uses a fresh block at cell D/4
  (`session/compute.rs:1020-1025`); that model is wrong for every op after the first, and is the
  reason feed optimisation refuses rest and 3D ops (`catalog.rs:2434-2449`). A must not copy it.
- **A rest op** already waits for its prior stock in the plan. Under A every op waits, and the plan
  gains one "Feeds" step per op after its Generate step. The rest op keeps its geometry rule; only
  the feeds step reads the stock.
- **The simulation** then reads the stored feeds; its gates judge the emitted feeds (no
  `predicted_feeds` patch, no provenance refresh `simulation.rs:1072-1092`). The time is the time of
  the stored toolpath on both clocks.

## 5. Recommendation

**B now, then A.** Reason:

- The safety gap of section 3 is a display gap, and B closes it with no feed change. It is worth
  doing even if A follows: the operator must see the largest emitted feed before a cut.
- B alone does not answer the operator. The simulation still changes the feeds. A answers the
  sentence: generation gives the toolpath that runs, the simulation verifies it, and nothing
  changes after it.
- A keeps the cycle-time gain of the modulator. C and F give it up, E gives up the raises.
- A reuses three parts that exist: the plan's prior-stock steps, the S5 prefix memo and the
  `SourceStock` record. The modulator itself does not change.

The first three commits:

| # | What | Does a number move? | Predictions needed |
|---|---|---|---|
| 1 | `ModulationSummary` gains `max_feed_mm_min` and `moves_above_stated`; the Feeds card, MCP `get_tool_load_report` and the G-code header line of each op print them ("runs at up to F3000, 48 moves above the stated F1000"). | No feed moves. New text on three surfaces; FM1 sim CSV only if it holds the summary (check). | The header lines of the four fixtures (section 2.1 gives the values) |
| 2 | The limit row reads the feed stage of the result: `RunModulation` comes from `feed_stage` (`Planned` gives `Off`), not from the GUI constant. The export warns on `Planned` when the run would modulate, and keeps the `ModulatedSuperseded` warning. | Row tones move: an S0 row that is over loses "before modulation" and reads `Exceeds`. Operator idx 7, 8 face lines change before a simulation. | Row dump of the four project files at S0 and S1 (the 10z probe), face lines of the 18 operator ops |
| 3 | Design note and predictions for A (the plan step, the stock read, the slot), then a trial on the sentry fixtures: generate-with-feeds against today's simulate-then-modulate, move by move. | In the trial, no: the feeds must equal today's modulated feeds on the fixtures (both read the same stock and the same modulator). | The fixture table of 2.1 must come out equal; generation time against today's simulation time |

## 6. Questions for the operator

1. **Should the machine ever run a move faster than the feed you typed?** Today it can, up to the
   machine ceiling (15000 mm/min on your file), when a cut is light. Yes: the modulator keeps its
   raises and the time it saves (7 to 45 % on the test parts). No: the typed feed becomes a ceiling
   (option E); the modulator only slows moves.
2. **Is a longer Generate acceptable if the generated toolpath is then final?** Option A makes each op
   wait for the stock that the ops before it leave, about one simulation in total. Yes: A. No: B, and
   the simulation keeps its feed change, but every screen says so.
3. **After an edit upstream, must the export stop or warn when the feeds of a later op are old?**
   Today it warns and writes the old feeds. Stop: no G-code until the feeds step runs again. Warn: as
   today, with the old and new numbers named.
4. **Do you want to approve the modulated feeds by hand (option C)?** Yes: nothing changes a feed
   without a click; the click is lost when the op regenerates. No: the change is automatic, but shown.
5. **One depth for the power cap (M4-A) now?** Each simulation sample holds two depths of cut: the
   deepest point of the sample and the time mean. The power cap reads the mean, the force readings
   read the deepest point. M4-A makes every reader take the deepest point. With A or C the change
   shows before the cut, so the objection "an exported feed moves after the simulation" no longer
   holds. Yes: M4-A with this step. No: M4-A1
   (gates only).
6. **Feed optimisation (the dressup) has the same problem before any simulation: it changes the feed
   of each move on a fresh block of stock, and no screen shows it.** Keep it, fold it into the
   feeds step of A, or delete it? Your file has it off on all 18 ops.

## 7. What was not verified

- The operator file was not simulated or exported. Every operator number is stored (section 2.2).
- No load cap (chip, power, deflection, force budget) bound on the four fixtures; their lowering
  side is stored evidence only. A fixture where a load cap binds
  (`the_session_modulator_binds_on_the_force_budget_p16`) was not probed.
- The GUI adopt path was not run; the core door was. `the_adopt_modulates_as_run_simulation_does_f115`
  is the sentry that holds the two equal.
- That Generate All modulates through each `SimulatePrefix` step is reasoned from the one drain call
  (`compute.rs:971`), not run.
- The S0 export of operator idx 7 and 8 (the stated chip under a caution row) is reasoned.
- MCP `inspect_spans` was not read in full, so "no surface prints the largest emitted feed" holds for
  the surfaces named in section 3.
- The fixture flute count and the source of `feed_max_rate` 3000 in the fixtures were not traced.
- Edits of the tool or the stock material after a modulated run were not probed (the machine edit was).
- The sizes in section 4 are estimates from the files named; no code was written.
- The FM1 sim CSV was not checked for a `ModulationSummary` column (commit 1).

## Lead rulings (2026-11-14)

The operator decides the option (A to F) and the six questions. The lead builds now only what
shows the truth and moves no feed:

- **P30-1: print what runs.** After a simulation the exported feeds can be above the stated feed
  (measured: 48 of 63 fed moves on the Pocket fixture, up to F3000 against a stated 1000), and no
  surface prints the largest emitted feed. The Feeds card, the simulation inspector, MCP and the
  G-code header state, for each op: the feed stage of the result (stated; feed optimisation;
  modulated by simulation run n), the largest and the smallest emitted cutting feed, and the count
  of fed moves above the stated feed. One core record, read by every surface. No feed changes.
- **P30-2: a row does not soften for a stage that did not run.** The lead ruling of P29-10i
  (`Caveat::BeforeModulation` turns a row to caution while the session option is on) was wrong in
  one state: an export with no current simulation carries the stated feed, and the row showed
  caution for a chip 27 % over the band (operator idx 7 and 8). The caveat softens a row only when
  the op holds a current modulated result (the feed stage of the result says so). With no current
  modulated result the row keeps its gating tone, and the caveat word says that the simulation
  will cap it. The export of an op whose rows exceed and whose feed stage is not modulated carries
  a warning in the header and on the export surface. A warning, never a refusal.
- **Not built without the operator:** options A, C, E, F; any change of a feed; M4-A; feed
  optimisation kept, folded or deleted.
- **For the operator, first:** question 1 of section 6 (may a move run faster than the typed
  feed?). On the operator file the stored report gives idx 8 a median feed 508.8 % over the stated
  feed after modulation.

## P30-1 and P30-2: predicted and measured

Written 2026-11-14 at HEAD `03a042a04`, before the code. "Predicted" is the output of a probe on
HEAD plus a script that applies the rules below. The probes and the script are in the agent
scratchpad (`p30b/zz_probe_rows_p30.rs`, `p30b/zz_probe_p30_feeds.rs`, `p30b/predict.py`).

### P30-1: the rules as they will be built

- **One record per toolpath: `session::EmittedFeeds`.** The door is
  `ProjectSession::emitted_feeds(index)`. It reads the moves of the result slot, the moves that the
  export door gives the program builder (the export datum shift moves X and Y only). The program
  builder takes the record on `GcodePhase` in place of the feed-stage text, so the header and the
  F words come from one slot. The GUI, MCP and the CLI read the same door.
- **The fields.** The feed stage (`FeedStage`); `feed_optimised` (the feed optimisation dressup ran:
  the option is on and `feed_optimization_unavailable_reason` gives no reason, the predicate of the
  step itself); the stated cutting feed; the cutting moves (smallest and largest feed, the count, the
  count above the stated feed); the plunge entries and the ramp and helix entries (largest feed and
  the stated plunge or ramp feed).
- **The classes.** No new classifier: the move intents of the cut kinds
  (`dressup::feed_modulation::intents_of`, the table that the modulator and the limit rows read).
  A cutting move is a fed move with an intent of `CutKind::Body` (`ClearingCut`, `FinishingCut`,
  `Unknown`). A plunge entry is `CutKind::Entry(Plunge)` (`EntryPlunge`). A ramp entry is
  `Entry(Ramp)` or `Entry(Helix)`. A link, a lead, a retract, an approach and a drill move are in no
  class. The stated ramp feed is `entry_feed_rate()`, else the plunge rate (the rule of the entry
  emitters).
- **The numbers are the F words.** Each feed goes through the post of the session: the clamp at
  `limits.max_feed`, then the round at `decimals.feed`. The stated feed goes through the same
  function. A move is "above the stated feed" when its F word is larger than the F word of the
  stated feed. Not in the record: the rapid rewrite (`high_feedrate_mode` writes rapids as `G1`
  moves; they are not cutting moves) and the native drill cycles (drill moves are in no class).
- **The text, one writer in core.**
  - G-code header, after the label and the feed-stage line (the stage line does not change):
    `(feeds run 943 to 3000 mm/min, 32 of 40 cutting moves above the stated 1000 mm/min)`. With
    feed optimisation the line ends `, set by feed optimisation`. A second line only when the
    largest entry feed is not the stated one: `(ramp entries run up to 1523 mm/min, the stated
    ramp feed is 500 mm/min)`.
  - GUI and the CLI: `runs 943 to 3000 mm/min · 32 of 40 moves above the stated 1000 · modulated
    by simulation 2`. The stage words: `as stated`, `feed optimisation`, `modulated by simulation
    n`, `modulated by simulation n, not current`. The GUI shows the caution colour only when a move
    runs above the stated feed.
- **Surfaces.** The G-code header; MCP `export_gcode` (one line per emitted op in the reply text);
  MCP `get_tool_load_report` (a field `emitted_feeds` on each toolpath record); the CLI `project`
  report (one line per op); the GUI Passes tab (one line under the Limits drawer) and the simulation
  inspector (the same line on the op row).

### P30-1: predicted numbers on the four sentry fixtures

The post of the fixtures rounds the feed to 0 decimals. "Before" is the fixture state (a
simulation with modulation off, stage `Planned`). "After" is a second simulation with modulation
on (`SimulationOptions::default()`).

| Fixture | Stated | Cutting moves | Before: min / max, above | After: min / max, above | Entry, largest before / after |
|---|---|---|---|---|---|
| Pocket, ramp | 1000 | 40 | 943 / 1033, 6 | 943 / 3000, 32 | ramp 1000 / 1523 (stated 500) |
| Profile, tabs, leads | 1000 | 33 | 900 / 1003, 3 | 900 / 3000, 31 | plunge 500 / 500 (stated 500: no line) |
| 2D Adaptive, helix | 1500 | 534 | 1339 / 1847, 405 | 735 / 3000, 300 | helix 3000 / 3000 (stated 500) |
| Shelf 3D rough | 1500 | 754 | 522 / 1500, 0 | 500 / 3000, 375 | ramp 500 / 2622 (stated 500) |

The note of section 2.1 counted every fed move with a 0.5 mm/min tolerance (Pocket: 48 of 63). The
record counts cutting moves only, at the F word, so it gives other numbers. The flat fixtures have
feed optimisation on and it runs, so "before" already holds feeds above the stated feed. On the
shelf (a 3D op) the step does not run, so `feed_optimised` is false although the option is on.

Predicted header of the Pocket after the simulation:

```
(pocket)
(feeds: modulated from simulation run 2)
(feeds run 943 to 3000 mm/min, 32 of 40 cutting moves above the stated 1000 mm/min)
(ramp entries run up to 1523 mm/min, the stated ramp feed is 500 mm/min)
```

### P30-2: the rules as they will be built

- **The fact.** `RunModulation` has three values, and core builds them per toolpath with one
  constructor, `ProjectSession::run_modulation_of(index)`:
  - `Off`: no caveat. A disabled toolpath, and every reader with no session (FM1, a rule, a unit
    test, the CLI job file).
  - `Current { strategy, feed_scale }`: the result holds a current modulated result
    (`FeedStage::ModulatedCurrent`). The strategy and the scale come from the result:
    `ResultProvenance::Modulated` records them when the run rewrites the slot. The caveat softens.
  - `NotRun { export }`: an enabled toolpath with no current modulated result (no result, a
    `Planned` result, a superseded result). `export` says what an export carries now: the stated
    feed, or the feeds of an older simulation. The caveat does not soften.
- **The doors take no modulation argument.** `ProjectSession::limit_rows`, `safe_envelope` and
  `cutter_op_profile` read `run_modulation_of` themselves, so no surface can pass a constant. The
  GUI and MCP constant `run_modulation()` goes; `FEED_MODULATION` stays for the adopt step. The GUI
  feeds memo reads the value of the session (a field of `FeedsInputs`, in its key).
- **The caveat.** `Caveat::BeforeModulation` (softens) stays for `Current`. A new
  `Caveat::UntilModulated { export }` (does not soften) is for `NotRun`. The two have rank 0 and one
  of them at most is on a row. The face word is `until modulated` (15 characters; `before
  modulation` has 17, so every face that fits today fits). As for `before modulation`, the drawer
  row (`face_in_group`) does not carry it. The clause (the hover): "until modulated: a simulation
  with feed modulation re-times the moves of this cut and caps them at this bound; an export now
  carries the stated feed" (or "the feeds of an older simulation").
- **The list of capped limits for `NotRun`** is the list of a default run (`ConstrainedMax`, scale
  1: `ChipMax`), the run of the GUI, MCP and the CLI with no flag.
- **The export warning.** A new gate `Gate::UnmodulatedOverLimit` and the id
  `export.stated_feed_over_limit` (`Warn(Danger::Tool)`). It raises one warning per emitted op
  whose feed stage is `Planned` and which has a stated row with tone `Exceeds` and the caveat
  `UntilModulated`. The text names the face of the worst such row. A superseded op gets no second
  warning: the feeds caution of W2.5 names it. A warning, never a refusal. The session memoises the
  rows that it reads, because the pre-flight modal and the readiness panel read the warnings on each
  frame.

### P30-2: predicted rows with no simulation

The rule changes only the rows that carry `before_modulation` on HEAD (the run constant of the
GUI). Each takes `until_modulated` and the tone of the same row under `RunModulation::Off`.
`rank_cmp` and the face builder do not read the modulation caveat, so the face line is the face line
of the `Off` column with the word.

| File | Rows with the caveat (changed word) | Tone changes (caution to danger) |
|---|---|---|
| `nz_wanaka_21_operator.toml` | 112 | 9: idx 1 body and planner cap; idx 2, 14, 15 planner cap; idx 7 and 8 body and slot ring (all stated cut rows, chip max) |
| `nz_wanaka_21.toml` | 64 | 1: idx 16 body (stated cut, chip max) |
| `rivmap100.toml` | 24 | 2: idx 1 planner cap and first slot (proposal rows, chip max) |
| `rivmap100_live_0925.toml` | 16 | 2: idx 1 planner cap and first slot (proposal rows, chip max) |

14 tone changes in all: the 14 rows that P29-10z softened (section 14 of `P29-10i_DESIGN.md`) go
back. No state, value, bound or margin moves. The RPM rows and the gate rows (readiness, verdict) do
not read the modulation and do not move.

The face line of the 18 operator ops (the Passes card):

| idx | HEAD | After P30-2 (predicted) |
|---|---|---|
| 0, 3, 4, 5, 9, 10, 12, 13 | none | none |
| 1 | caution: planner cap · depth 387 % over force cap | **danger: planner cap · chip max 24 % over limit · until modulated** |
| 2 | caution: ramp entry · depth 100 % over force cap · worst case | **danger: planner cap · chip max 2.9 % over limit · until modulated** |
| 6 | caution: ramp entry · chip min 76 % under floor · worst case | the same |
| 7 | caution: body · chip max 27 % over limit · before modulation | **danger: body · chip max 27 % over limit · until modulated** |
| 8 | caution: body · chip max 27 % over limit · before modulation | **danger: body · chip max 27 % over limit · until modulated** |
| 11 | caution: ramp entry · chip min 66 % under floor · worst case | the same |
| 14 | caution: ramp entry · depth 78 % over force cap · worst case | **danger: planner cap · chip max 3.4 % over limit · until modulated** |
| 15 | caution: ramp entry · depth 84 % over force cap · worst case | **danger: planner cap · chip max 3.1 % over limit · until modulated** |
| 16 | caution: body · chip min 16 % under floor | the same |
| 17 | caution: plunge entry · plunge rate 17 % over limit · may not occur | the same |

The face lines of idx 1, 2, 14 and 15 move from the depth row to the chip row: a danger row ranks
before every caution row. On idx 1 the planner cap row (24 %) ranks before the body row (21 %)
because the two have the same tone and the planner cap has the lower margin.

The other files: `nz_wanaka_21.toml` idx 16 goes from "caution: body · chip max 50 % over limit ·
before modulation" to "danger: body · chip max 50 % over limit · until modulated"; every other face
line of the three files stays (the rivmap tone changes are proposal rows, not on the face).

### P30-2: predicted for the export, Readiness, FM1 and the op golden

- **Export warning.** On the operator file, each of idx 1, 2, 7, 8, 14 and 15 that holds a
  `Planned` result gets one `export.stated_feed_over_limit` warning (a header line and a row of the
  warning list). No other op of the four files has a stated danger row with the caveat.
- **Readiness.** No check state changes. The readiness checks read the gate rows (no modulation
  caveat) and, from the export warnings, the rapid-collision rows only. The pre-flight `Tool` chip
  counts the new warnings. The Export button style does not change (it reads `Machine` warnings).
- **FM1.** Byte-equal: FM1 states `RunModulation::Off` and prints no header.
- **The op golden.** It hashes the program that `export_gcode_checked(None)` writes, header
  included (`gcode_hash`). The P30-1 header line moves `gcode_hash` of every arm whose toolpath has a
  cutting move; no other field moves. That is the STOP of the brief: the agent does not re-bless, and
  it gives the list of arms and the header-only proof (`RS_CAM_OPGOLDEN_DUMP_GCODE`, a diff with the
  comment lines filtered).
- **The fixtures of the export tests.** The new header lines are comment lines. Every program stays
  byte-equal when the comment lines are filtered. A test that counts or orders header lines
  (`export_states_the_feed_stage_w2_5` reads the line after the label: no change) is read before the
  code changes.

### Measured (P30-1 and P30-2, on the code)

Instruments: the row probe on the code (`p30b/dumps/after`), `p30b/measure.py` (every row against
the prediction), the op golden with `RS_CAM_OPGOLDEN_DUMP_GCODE` and `p30b/headeronly.py` (the
header-only proof), FM1.

| Quantity | Predicted | Measured |
|---|---|---|
| Envelope rows of the four files equal to the prediction (value, bound, state, caveat, tone, face; the detail of the unchanged rows) | all | 3465 of 3465 |
| Rows with the caveat (operator, `nz_wanaka_21`, rivmap100, live) | 112, 64, 24, 16 | 112, 64, 24, 16 (`until_modulated`, `export: stated`) |
| Tone changes, caution to danger | 14 | 14, the same rows |
| Face lines of the four files | 44 as predicted | 44 of 44 |
| RPM rows, verdict rows | no change | equal to HEAD |
| FM1 (both CSVs) | byte-equal | byte-equal (2 of 2 pass) |
| Op golden: rows other than `gcode_hash` | the known 46 | the known 46 (diff empty) |
| Op golden: `gcode_hash` | moves on every arm with a cutting move, header only | 85 of 89 arms move; with the new comment lines removed, each program hashes to its stored value (85 of 85; 141 lines removed). No arm gets an `export.stated_feed_over_limit` line |

The 18 operator face lines are the predicted table above, measured equal.

P30-1: `the_record_states_what_the_gcode_runs_p30_1` parses the program of the export door and
holds the record equal to the F words in three states: a Pocket with feed optimisation off and no
simulation (no cutting move above the stated 1000; the generator writes some moves under it, so
the smallest feed is below the stated feed), the 2D Adaptive with feed optimisation on (moves
above the stated feed, the header line ends ", set by feed optimisation"), and the Pocket before
and after a simulation with modulation (more moves above the stated feed and a larger maximum
after it). The header of the op holds the lines of the record, in order, after the label. The
numbers of the fixture table are the probe numbers of the same rule (same moves, same classes,
same post); the test does not print them again.

Surfaces measured equal for one session state, before and after a simulation with modulation: the
GUI panel input (the Passes line and the simulation inspector line), MCP `get_tool_load_report`
`emitted_feeds` and the export reply (viz lib `one_feed_record_on_every_surface_p30_1`), and the
CLI `summary.json` and the `--summary` report (`the_summary_prints_the_emitted_feeds_p30_1`).

P30-2: `a_row_softens_only_under_a_current_modulation_p30_2` holds the three states on a Pocket
with a stated feed of 9000 mm/min: a planned result keeps the danger tone, carries `until
modulated` and the export warns (the list and the header); after a simulation with modulation the
row is a caution with "before modulation" and no warning; after a simulation with modulation off
the warning stays. The test went red when `UntilModulated` was made to soften (2 of 3 failed),
then green with the code restored.

The explainer data of P29-10 (`P29-10_explainer/data/`): the two `cli_suggest_*.jsonl` files are
the CLI `suggest --json` of this tree; the six `mcp_safe_envelope_operator_*.json` files take the
core serialisation of the envelope of this tree (a throwaway probe, `p30b/zz_probe_explainer_p30.rs`)
with the capped speeds grid of the old files spliced in byte for byte (P30-2 moves no number, and
the CLI speeds of these six ops are equal before and after). `build_page.py` rebuilt `index.html`;
`assert_claims.py`: 11 of 11 PASS. The old data was older than HEAD: besides the P30-2 rows
(`run_modulation`, the caveat, face, detail and tone of the capped rows), the regenerated CLI
files also move `check.basis` on 10 + 18 rows, `speeds.points[].rubbing` and one `boundary`
(changes of P29-11, not of P30).

Departure from the prediction (judged on the screenshot): the GUI line as predicted ("runs 1258
mm/min · 0 of 144 moves above the stated 1258 · as stated") elided its stage at the 340-point rail
on operator idx 7. The core line is now shorter: "runs 943 to 3000 mm/min · 32 of 40 above 1000 ·
modulated, run 2" (stage words "as stated", "feed optimisation", "modulated, run n", "modulated,
run n, not current"). The hover says what the counts mean ("32 of 40 cutting moves run above the
stated feed of 1000 mm/min") and holds the stage statement and the entry lines. The G-code header
lines do not change.

On the GUI (release build of this tree, shots `P29-10_shots/after/S12_*`): the face lines of
operator idx 7, 8 and 1 read as predicted, in the danger tone; idx 0 has none. An MCP export of the
operator file with no simulation (11 of 18 ops hold a result) carries
`export.stated_feed_over_limit` for idx 7 and idx 8 only: of the predicted ops (1, 2, 7, 8, 14,
15) only these two hold a planned result. Readiness shows no new row and no check state changes.
On rivmap100 Face 5 after a simulation with modulation, the header reads "(feeds run 2333 to 3000
mm/min, 77 of 77 cutting moves above the stated 2200 mm/min)" and "(ramp entries run up to 3000
mm/min, the stated ramp feed is 500 mm/min)", and the GUI line takes the caution colour.

Two changes after the review of the shots and the code:

- **The softening value is sealed.** `RunModulation::Current` holds a `HeldByResult` that has no
  public constructor (`ProjectSession::run_modulation_of` builds it from a stored modulated result;
  a test build states `RunModulation::current_for_tests`). A surface can state `Off` or `NotRun`
  (neither softens) but not `Current`, so the compiler holds "a surface cannot pass a constant that
  softens a row".
- **The stage of the GUI line does not elide.** After a feed stage the GUI prints the numbers on
  one line ("runs 2333 to 3000 mm/min · 77 of 77 above 2200") and the stage on the next ("feeds:
  modulated, run 1, not current", in the caution colour when the stage is not current). With the
  stated feeds it stays one line. CLI and MCP print the one line `EmittedFeeds::line`.
- Not changed: the simulation inspector prints the line only with its cut-metric cards, so a stale
  trace (the cards say "the cut trace is from an earlier version") shows no line there; the
  Passes tab still shows it.
