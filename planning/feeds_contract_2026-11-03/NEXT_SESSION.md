# Next session: feeds and speeds correctness (brief, 2026-11-08, second edition)

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
