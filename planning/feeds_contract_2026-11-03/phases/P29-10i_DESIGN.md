# P29-10i: every declared cut is judged at the stated point

Design note and predictions for the lead (lead ruling L1, `P29-10_DESIGN.md`
section 12). Baseline: HEAD `702c8e97b` plus PB and 10h (the trees of
`pb.patch` and `10h.patch`). No product code of 10i is written yet.

## 1. The rule

One judge, two points. `SafeEnvelope::read_declared_cuts` becomes a function
of a judged point (`JudgedPoint`): the proposal (part two of the drawer,
"If you apply ⚡") or the stated cut (part one, "As the op stands"). The
three row writers (`ceiling_rows`, `value_rows`, `vertical_rows`) take the
point as their argument; no writer reads `proposal_point` or the proposal
speeds directly. Both points call `planned::judge_declared_cuts`. No edge
solve reads the stated point (`lowest_ceiling` and the drawn edges stay as
they are).

## 2. What a stated cut reads, for each kind of number

| Number | Proposal (today) | Stated cut (10i) |
|---|---|---|
| Width of the body column | proposal `ae` | stated `ae` (`SafeEnvelope::stated`) |
| Depth of the body plane | proposal `ap`; the leave of a finish (9c); the lower bound of a Project Curve (9b) | stated `ap`; the same leave and lower bound rules (`BodyDepth`) |
| Body chip | the commanded chip of the calculator at the proposal (`calc_at(proposal)`) | the stored feed over the stated rpm x Z, as a `StoredFeedChip` (`[f, f+1)`): a ceiling reads the stated chip, a floor the high end |
| Chip of a cut with `DeclaredFeed::Unlifted` (slot ring) | body chip over the body lift | the same rule on both ends of the interval |
| Entry chip (ramp, helix, plunge, peck) | the stored entry feed over the stated rpm (P29-8 8d, P29-9) | the same (the entry rule does not read the point) |
| Force budget, deflection, vendor depth table | the curve chip and the pick chip (the lower ceiling) | the stated chip (the cut as it stands has no pick) |
| Spindle power | the calculator power terms at the proposal rpm and feed, against the rated power at that rpm | the same terms at the stated rpm and the stored feed, against the rated power at the stated rpm |
| Chip band, floor, rubbing floor | the matched row of the calculator at the proposal | the matched row of the calculator at the stated point (`calc_at(stated)`) |
| Vertical rate | the stated entry feeds and the stated rpm | the same |

A cut that has no such number at the stated point: the row is `NoNumber`
with a named reason, never a guess:

- the op states no feed or no spindle speed: a new reason
  `NoNumberReason::NoStatedFeed` ("no stated feed"), open (a datum fills
  it), on every row of a cut that reads the body chip;
- an entry with no entry feed: today's `NotModelled` with the clause of
  `EntryNoNumber`;
- a cut with no width or depth on the plane: today's reasons
  (`NoEngagementModel` through `NotModelled`).

## 3. The wire

- New subject `Subject::StatedCut { cut, occurs, at }` (wire
  `{"kind": "stated_cut", "cut", "occurs", "at"}`): a declared cut at the
  stated point. `at` is the plane point of `marks_at_stated`.
- `Subject::Stated` stays for the limits that are not limits of each cut
  (flute guard, cutting length, vendor axial bound, corner coverage, tool
  diameter, move budget, engagement floor, feed caps): `of_envelope` at the
  stated point, as today.
- The `of_envelope` stated row of a limit that has a planned role (force
  budget, deflection, vendor depth table, power, chip band, rubbing floor)
  goes: the stated cut rows replace it (one judge, no second path).
- Core predicates for every reader, so no surface matches a subject:
  `CheckRow::is_planned()` (a row at the proposal, unchanged meaning),
  `CheckRow::is_stated()` (part one: a stated row or a stated cut row),
  `CheckRow::cut()` (the declared cut of a planned or a stated cut row).
- `Subject::rank`: a stated cut row ranks with the stated rows (0).

## 4. The structural proof (planned for the code)

- `read_declared_cuts(&self, at: &JudgedPoint) -> Vec<PlannedRow>`, with
  `JudgedPoint::{proposal, stated}` the only two constructors; the row
  writers read the point, its speeds and its body chip from the argument.
- First commit step: the refactor with the proposal as the only caller; the
  row probe must be byte-equal. Then the stated point is the second caller.
- Sentry claims (new `every_declared_cut_is_judged_at_the_stated_point_p29_10`):
  every op kind on the fixture tools has one stated cut row per declared
  cut and planned limit (a value, or a named `NoNumber` or
  `NotApplicable`); when the stated point equals the proposal point and the
  stated speeds equal the proposal speeds (a Suggest apply), each stated
  cut row equals its planned row in state and value.

## 5. The cost

The stated point adds one judge pass per planned limit and declared cut
(about 5 to 15 judges per limit), outside the edge solves. The measure is
the time-budget test alone, six runs, with `uptime`, before and after
(`feeds::envelope::tests::the_envelope_meets_its_time_budget`, 300 ms; it
read 181 to 199 ms).

## 6. Exemption rows

- `S-NotJudgedHere`: the stated `ChipMax` (per point) and the planned limits
  of a Project Curve at the stated point leave the arm (the stated cut rows
  judge them); the arm keeps `CuttingFeedCap`, `MoveBudget`,
  `ToolDiameter`, `EngagementFloor` and the fixed-axis limits that are not
  limits of each cut (flute guard, cutting length, vendor axial). The row
  narrows again; no pin moves.
- No new exemption row.

Measured before the code (the baseline of 10i, HEAD plus PB and 10h,
load average 1.36): 180, 187, 177, 176, 173, 174 ms.

## 7. Predicted rows and face lines

Method: a throwaway probe (deleted; a copy is in the agent scratchpad
`p29_10hi/`, `zz_probe_p29_10i_pred.rs`, output `pred_10i.jsonl`, script
`predict_10i.py`) judged every declared cut at the stated point from the
public pieces: the declared cuts of the Suggest context, the planned judges
of `feeds::planned`, the stored feed over the stated rpm as a
`StoredFeedChip`. Power and the vertical rate of a cut are not judged by the
probe: the body takes today's stated power row; an entry copies its planned
row (an entry reads the same stated numbers at both points). The face line
is the worst row of part one by the rule of `CheckRow::rank_cmp` (the
exceeding tone first, then advisory, then the lower margin). One correction
by hand: operator idx 5 has no body depth before the simulation, so its
body and first-slot depth rows have no number (the probe read the fixed
axis value). The face words are approximate; the tone, the cut and the
limit are the prediction.

Check of the emulation: the stated body chip values equal section 5 of the
point-band trace (`P29-10i_POINT_BAND.md`): idx 1 +21 %, idx 16 -15.5 %,
idx 3 and 4 -56 %.

Limit of the emulation (power): the slot ring, first slot and planner cap
power rows are copied from the proposal, at the proposal depth (idx 1: 2.06
mm against the stated 10 mm). Today's stated power row (the power edge at
the stated point, the worst case of the cuts that set a ceiling) is
`Within` on idx 1, 2, 14 and 15 (70 %, 24 %, 23 %, 22 % of the power cap),
so no power row of a cut that sets a ceiling can outrank the chip lines
below. A first slot (`MayOccur`) at the stated 10 mm can exceed the power
limit on idx 1 and 14; under its caveat it is a caution row, so it cannot
outrank a danger line either.

### The face line of the 18 operator ops

| idx | Op | Today (HEAD + PB + 10h) | After 10i (predicted) | Cause |
|---|---|---|---|---|
| 0 | Face | none | caution: ramp entry · chip min < 0.1 % under floor | the stored ramp feed (6447 mm/min) is 1.6 write steps under the printed point: two writes (the cut feed, then the ramp over the lift), outside `[f, f+1)` |
| 1 | 3D Rough | caution: stated point · depth 387 % over force cap | DANGER: planner cap · chip max 24 % over limit | stated feed 8475 at 18000 rpm: chip 0.235 mm; body +21 %, planner cap +24 % over the printed point (high side hard) |
| 2 | 3D Rough | caution: stated point · depth 74 % over force cap | DANGER: planner cap · chip max 2.9 % over limit | the stated feed holds the point at the body width (-0.008 %, `Within` with "stored feed"); the planner cap is wider, so its lift is smaller |
| 3 | Pin drill | none | caution: body · chip min 56 % under floor | the drill feed 1000 mm/min over the drill rule chip 0.0635 mm (advisory floor: one printed value) |
| 4 | Drill | none | caution: body · chip min 56 % under floor | as idx 3 |
| 5 | Project Curve | none | none | no body depth before the simulation; the chip is inside the band |
| 6 | Pocket | none | caution: ramp entry · chip min 76 % under floor | the entry chip 0.0133 mm against the point 0.0550 mm (not rounding) |
| 7 | Pocket | none | DANGER: body · chip max 27 % over limit | stated chip 0.0699 mm against the point 0.0550 mm |
| 8 | Pocket | none | DANGER: body · chip max 27 % over limit | as idx 7 |
| 9 | Project Curve | none | none | |
| 10 | Trace V60 | none | none | |
| 11 | Pocket | none | caution: ramp entry · chip min 66 % under floor | the entry chip (not rounding) |
| 12 | Face | none | caution: ramp entry · chip min < 0.1 % under floor | as idx 0 |
| 13 | Trace V60 | none | none | |
| 14 | 3D Rough | caution: stated point · depth 73 % over force cap | DANGER: planner cap · chip max 3.4 % over limit | as idx 2 |
| 15 | 3D Rough | caution: stated point · depth 57 % over force cap | DANGER: planner cap · chip max 3.1 % over limit | as idx 2 |
| 16 | Scallop | none | caution: helix entry · chip min 91 % under floor | the helix ranks before the body (body chip min -15.5 %, caution, also in part one) |
| 17 | Profile | none | caution: plunge entry · plunge rate 17 % over limit | the plunge entry row of part two, also a cut at the stated point |

The body rows that only the rounding puts under the point (idx 0, 2, 6, 11,
12, 14, 15: -0.008 % to -0.055 %) read `Within` with the caveat "stored
feed", as ruled. They make no face line. But on idx 0 and 12 the ramp and
helix entry rows make one, and on idx 2, 14 and 15 the planner cap makes a
danger line.

Readiness tool-load rows: no change (10i touches no gate).

### The other files (face lines after 10i, predicted)

- `nz_wanaka_21.toml`: 9 of its 18 ops state no feed and no spindle speed
  (idx 0, 1, 4, 6, 7, 8, 11, 12, 14). Their body-chip rows are `NoNumber(NoStatedFeed)`
  under the rule of section 2, and idx 1 loses its face line of today
  ("stated point · depth 266 % over force cap"). idx 2 keeps "moves 78 %
  over limit" (danger; not a limit of each cut). idx 16 gets a danger line
  (body · chip max 50 % over limit); idx 3, 5, 9, 10, 13, 15, 17 a caution
  line (chip min under the point or the band).
- `rivmap100.toml`: idx 0 caution (ramp entry chip min 89 % under), idx 1
  DANGER (helix entry · rubbing floor 44 % under floor: the helix is
  `Always` on this op, no caveat softens it), idx 2 DANGER (body · rubbing
  floor 17 % under floor), idx 3 caution (body chip min 73 % under, lower
  bound).
- `rivmap100_live_0925.toml`: idx 0 caution, idx 1 DANGER (stated point ·
  depth 33 % over vendor depth cap, unchanged), idx 2 DANGER (body rubbing
  floor), idx 3 caution.

## 8. Questions for the lead (each changes what the operator sees)

The rulings do not answer these, so this step stops after the note
(the brief: "STOP after the note and report").

1. **idx 0 and 12: a line from a two-write rounding.** The ramp feed is
   written from the stored cut feed over the lift, then rounded again, so
   it can sit up to `1 + 1/lift` steps under its source (`StoredFeedChip`
   doc). The rule `[f, f+1)` does not cover it, and both entry rows exceed
   by -0.0097 %. Options: (a) the stored-feed interval of a feed written
   from another stored feed covers `1 + 1/lift` steps (a change to the
   P29-9 rule, no number moves); (b) part one holds the cuts of the body
   plane only and the entries stay in part two (they read the same numbers
   at both points), so "every declared cut" becomes "every cut of the body
   plane"; (c) accept the caution line. My choice: (a), because it is the
   rounding the lead ruled out, and (b) breaks L1.
2. **idx 1, 2, 7, 8, 14, 15: danger lines from the stated chip maximum.**
   The high side stays hard (PB). idx 1, 7 and 8 are real overfeeds of the
   stated op (21 % to 27 % over the printed point). idx 2, 14 and 15 come
   from the planner cap: the stated feed was set at the stated width, and
   the planner cap (the pass ceiling width, P23a) has a smaller lift. Today
   the face of idx 1, 2, 14, 15 is the caution depth line. Accept the
   danger lines? My choice: accept; they are the cut the machine makes.
3. **idx 3 and 4: the drill body chip.** The drill op declares a body cut
   that the chip limits judge (the drill cycle cut is out of scope,
   P28-20). At the stated feed the body reads -56 % against the drill rule
   chip (caution: one printed value). Accept, or put the body of a drill op
   out of the chip scope?
4. **The stated rows of the planned limits leave.** One judge means the
   `of_envelope` stated rows of the force budget, deflection, vendor depth
   table, power, chip band and rubbing floor go; the stated cut rows replace
   them. The placement (`OutsideEnvelope`, the K2b warning) stays: it reads
   the pick chip (`max(stated, suggested)`), the stated cut rows read the
   stated chip. So on idx 1 the warning says "depth 387 % over" and the body
   row "depth 378 % over". Accept the two numbers, or move the placement to
   the stated chip (a warning text moves, no cutting number)?
5. **An op with no stated feed or speed.** 9 ops of `nz_wanaka_21.toml`
   state no feed and no rpm. Under section 2 their body-chip rows have no number, and the depth
   line of idx 1 leaves the face. Option: with no stated chip a depth
   ceiling reads the chip of the drawn edge (the suggested chip), as today's
   stated row does. My choice: that option for the depth ceilings only; the
   chip rows stay `NoStatedFeed`.

## 9. Not done in this step

No 10i code, no new sentry, no viz change, no screenshot of 10i. The viz
part one would need the same folds as part two (`compare.rs`: group the
stated cut rows by cut with `draw_cut`), and `envelope.rs` lines 1008, 1338
and 1359 would filter on `CheckRow::is_planned()`.

## 10. Lead rulings (2026-11-14)

On the questions of section 8 (commits `6e705fb22` to `41be7ca0f` are in).
For PB the lead accepts that the advisory-floor caveat has no face word:
the chip and the hover carry it.

1. **(a).** A feed written from another stored feed carries the interval of
   both writes: `1 + 1/lift` write steps, derived in the doc of
   `StoredFeedChip` from the two rounding steps, with a unit test of the
   bound (a case at the edge inside, a case just outside). No constant.
   idx 0 and 12 get no face line from rounding.
2. **Not ruled yet.** One measurement first: the simulation inspector of
   operator idx 1 reads "Chipload: peak at limit, within" while the stated
   row predicts "chip max 21 % over". Measure, for every op where a stated
   chip row exceeds, the stated row, the measured verdict of the last load
   report, the stage between the stated feed and the samples, and whether
   the two use the same chip model. Do not build Q2. Until the ruling, a
   stated chip max row is built with its true value and state, and one
   named, documented predicate keeps it out of the face line rank
   (`CheckRow::ranks_for_the_face`); the drawer shows the row as it is.
3. A drill op's body is not a milling cut. Its chip rows are out of the
   chip scope with a named reason that points at the existing row of the
   drill cycle (P28-20); the drill gates stay the judge. No "-56 %" line on
   idx 3 and 4. Use `limits::scope` and the existing `DrillCycle`
   engagement; no new exemption row if P28-20 covers it.
4. One number on screen. Every text about the stated point reads a stated
   row. The placement keeps its pick chip for the decision "outside or
   not"; no surface prints a margin from the placement. List every op of
   the four files where the placement says outside and no stated row
   exceeds, or the reverse, and report a list that is not empty.
5. As chosen: with no stated feed or speed, a depth ceiling row reads the
   chip of the drawn edge and carries a core caveat that says so ("at the
   suggested chip"); the chip rows are `NoNumber(NoStatedFeed)`. idx 1 of
   `nz_wanaka_21.toml` keeps its depth face line.

## 11. Predictions under the rulings (before the code)

Baseline: HEAD `41be7ca0f`. Rule of each ruling as it will be built:

- Ruling 1: the ramp and helix entry chips read the entry feed, which
  Suggest writes from the stored cut feed over the lift of the body
  (`UnliftedFeed`). Its interval is `[f, f + (1 + 1/lift) step)`, the lift
  being the body lift at the full-feed width of the stated width. A plunge
  feed keeps one write (it is not written from a stored feed).
- Ruling 3: a declared cut whose depth is the drill cycle
  (`DeclaredDepth::None(NoEngagementModel::DrillCycle)`, the body of a
  Drill and of an Alignment Pin Drill) is judged in the drill group, so
  `limits::scope` answers for it as for the drill cycle: the chip band,
  the rubbing floor and the vendor depth table `NotJudged` row `P28-20`,
  the power row `P28-10`, the deflection `P28-17`, the force budget
  `P28-04`. The row carries that scope row as its exemption (today a
  not-judged planned cell shows `S-NotJudgedHere`). No new exemption row.
  This moves the drill body rows of part two too: the body chip rows
  (today `Within`, 100 % of the point) and the body power row (today
  `Within`, 21 % of the limit) become `NotJudgedHere` with their rows.
- Ruling 4: the rationale and the diagnostic `project.outside_envelope`
  name the limit and the cut, with no margin; the MCP `outside_envelope`
  object and `envelope.operator` carry no margin; each surface that prints
  a number about the stated point prints a stated row.
- Ruling 5: as in the ruling; the caveat is `Caveat::AtSuggestedChip`.
- Q2 open: a stated cut row of `ChipMax` does not rank for the face line
  (`CheckRow::ranks_for_the_face`).

### The face line of the 18 operator ops

| idx | Today (HEAD) | After 10i (predicted) | Open on Q2 |
|---|---|---|---|
| 0 | none | none (the entry chip rows read the two-write interval: `Within`, "stored feed") | |
| 1 | caution: stated point · depth 387 % over force cap | caution: first slot · depth 420 % over force cap · may not occur | yes (body +21 %, planner cap +24 % chip max, held out) |
| 2 | caution: stated point · depth 74 % over force cap | caution: first slot · depth 120 % over force cap · may not occur | yes (planner cap +2.9 %) |
| 3 | none | none (the body is the drill cycle: chip rows `NotJudgedHere`, P28-20) | |
| 4 | none | none (as idx 3) | |
| 5 | none | none | |
| 6 | none | caution: ramp entry · chip min 76 % under floor | |
| 7 | none | caution: ramp entry · chip min 76 % under floor | yes (body and slot ring +27 %) |
| 8 | none | none | yes (body, slot ring, ramp, helix +27 %) |
| 9 | none | none | |
| 10 | none | none | |
| 11 | none | caution: ramp entry · chip min 66 % under floor | |
| 12 | none | none (as idx 0) | |
| 13 | none | none | |
| 14 | caution: stated point · depth 73 % over force cap | caution: first slot · depth 109 % over force cap · may not occur | yes (planner cap +3.4 %) |
| 15 | caution: stated point · depth 57 % over force cap | caution: first slot · depth 106 % over force cap · may not occur | yes (planner cap +3.1 %) |
| 16 | none | caution: helix entry · chip min 91 % under floor | |
| 17 | none | caution: plunge entry · plunge rate 17 % over limit | |

The face words are approximate (section 7); the cut, the limit and the tone
are the prediction. Part two changes on idx 0 and 12 (the entry chip rows
become `Within` with "stored feed") and on idx 3 and 4 (ruling 3).
`nz_wanaka_21.toml` idx 1 keeps a depth line ("depth ... over force cap",
caution, caveat "at the suggested chip").

### Q2: the measurement

| idx | Stated chip max row (predicted) | Measured chip verdict (load report of `generate_all`, `data/mcp_tool_load_report_operator.json`) | The stage between | Feed of the samples |
|---|---|---|---|---|
| 1 | body 0.2169 against 0.1791 (-21.1 %); planner cap 0.2219 (-23.9 %); first slot 0.2354 (-31.4 %, may occur) | `within`, peak 0.17913 at bound 0.17913, `vendor_lut_point_preset` | the adaptive feed modulation of the simulation (`constrained_max`; chip cap binds on 27.6 % of the moves) | median -23.9 % of the stated feed (8475 to about 6450 mm/min) |
| 2 | planner cap 0.1307 against 0.127 (-2.9 %); first slot -15.5 % | `within`, peak 0.127 at 0.127 | modulation, chip cap 32.3 % | median -13.4 % |
| 7 | body and slot ring 0.0699 against 0.0550 (-27.0 %) | `within`, peak 0.0550 at 0.0550 | modulation, chip cap 83.3 % | median -12.7 % |
| 8 | body, slot ring, ramp, helix 0.0699 (-27.0 %) | `within`, peak 0.0550 at 0.0550 | modulation, chip cap 25 % | median +508.8 % (the modulator raises the light moves) |
| 14 | planner cap 0.1851 against 0.1791 (-3.4 %); first slot -25.0 % | `within`, peak 0.17913 at 0.17913 | modulation, chip cap 32.2 % | median -20.0 % |
| 15 | planner cap 0.1309 against 0.127 (-3.1 %); first slot -18.7 % | `within`, peak 0.127 at 0.127 | modulation, chip cap 36.7 % | median -15.8 % |

- The stage: `SimulationOptions::adaptive_feed_modulation` (default `true`,
  `session/mod.rs`; the GUI run of `generate_all` used it). After the
  simulation the modulator re-times the moves (`apply_adaptive_feed_
  modulation`, strategy `constrained_max`) and caps each move's peak chip
  at the band times the lift of that move (P11b2); the export reads the
  modulated stage (`feed_stage`). So the samples carry the modulated feed,
  not the stated feed. Every simulated op of the operator file reads "peak
  at limit" (the cap holds the peak at the bound). No per-op switch exists.
- The chip model: the same thinning rule (`RadialThinning`, Garr, the op's
  `thins_chip`) on both sides. The stated row reads the stated feed over
  the stated rpm and the lift at the cut's declared width (the stated
  width for the body, the pass-ceiling width for the planner cap, D for a
  slot). The gate reads the achieved advance of each sample over the lift
  at that sample's measured arc, and reports the peak. So the stated row
  is the cut before the modulator; the gate is the cut after it.
- Options for the lead: (a) a caveat "before modulation" that softens a
  stated chip max row when the run modulates the chip (the session
  option; no per-op switch exists, so it would read the project's
  simulation option); (b) the stated row reads the feed the modulator
  emits (needs a simulation; a planned row then reads the simulation,
  against L2 and Q5); (c) accept the red line (the stored feed is over
  the printed value; the G-code before a simulation runs it); (d) keep
  the row out of the face rank for good (the predicate of today), the
  drawer shows it.

## 12. Measured beside predicted (10i)

The row probe of PB and 10h ran on the 10i tree (`dump_10i.jsonl`), with
the envelope placement on the record. "Measured" is the diff against the
HEAD dump.

### The face line of the 18 operator ops

All 18 face lines are as predicted in section 11 (the same cut, limit and
tone on every op; the face words: "· worst case" on the ramp rows and "·
may not occur" on the first slot, helix and plunge rows):

| idx | Predicted | Measured |
|---|---|---|
| 0, 3, 4, 5, 8, 9, 10, 12, 13 | none | none |
| 1 | first slot · depth 420 % over force cap (caution) | first slot · depth 420 % over force cap · may not occur (caution) |
| 2 | first slot · depth 120 % (caution) | the same |
| 6 | ramp entry · chip min 76 % under floor (caution) | ramp entry · chip min 76 % under floor · worst case (caution) |
| 7 | ramp entry · chip min 76 % under floor (caution) | the same as idx 6 |
| 11 | ramp entry · chip min 66 % under floor (caution) | the same, "· worst case" |
| 14 | first slot · depth 109 % (caution) | the same |
| 15 | first slot · depth 106 % (caution) | the same |
| 16 | helix entry · chip min 91 % under floor (caution) | the same, "· may not occur" |
| 17 | plunge entry · plunge rate 17 % over limit (caution) | the same, "· may not occur" |

Open on Q2 (held off the face by `CheckRow::ranks_for_the_face`, shown in
the drawer): idx 1 (body +21 %, planner cap +24 %), 2 (planner cap
+2.9 %), 7 and 8 (body and slot ring +27 %), 14 (+3.4 %), 15 (+3.1 %).
Without the predicate these would be danger lines.

`nz_wanaka_21.toml` idx 1 keeps a depth line: "first slot · depth 340 %
over force cap · may not occur", caveats `advisory`, `at_suggested_chip`
(the planner cap reads 266 %, the number of today's stated row).

### The rows

- Part one: 658 stated cut rows (operator), 643 (`nz_wanaka_21`), 139 on
  each rivmap file, one per declared cut and planned limit. The
  `of_envelope` rows of the planned limits leave part one: on the operator
  file 18 `ChipMax` (per point, `NotJudgedHere`), 15 force budget, 15
  deflection, 13 vendor depth table, 8 power.
- Part two changes, as predicted: operator idx 0 and 12, the ramp and
  helix `ChipMin` rows `Exceeds` to `Within` with "stored feed" (ruling 1);
  their `ChipMax` rows take the caveat "stored feed" (the bound is inside
  the wider interval); idx 3 and 4 (and `nz_wanaka_21` idx 3 and 4), the
  body chip and rubbing rows `Within` to `NotJudgedHere` row P28-20 and the
  body power row to `NotJudgedHere` row P28-10 (ruling 3); the drill cycle
  rows keep their state and now name P28-20 and P28-10 in place of
  `S-NotJudgedHere`.
- Not predicted in the note, a consequence of ruling 1 with no state or
  tone change: the margin of the entry `ChipMin` and `RubbingFloor` rows
  on every op with a whole-step ramp feed moves with the wider interval
  (operator idx 1, 2, 8, 14, 15, 17; `nz_wanaka_21` idx 15; live idx 1);
  on idx 0, 12 and `nz_wanaka_21` idx 15 the rubbing face percent moves.
- Readiness and the verdict rows: no change.

### Ruling 4: the placement against the stated rows

On the four files no op has the placement "outside" with no stated row
over, and no op has the placement "inside" with a stated depth or width
row over. Where the placement is outside (operator idx 1, 2, 14, 15;
`nz_wanaka_21` idx 1, 2, 14, 15; live idx 1), the stated row with
`check.binding` is the row of the binding limit: on operator idx 1, 2, 14,
15 the planner cap force-budget row reads 387 %, 74 %, 73 %, 57 %, the
numbers of the placement (one number); live idx 1: the first slot
force-budget row, 85 %.

The reverse in the wide sense is not empty: on 20 ops the placement is
inside and a stated value-check row exceeds (chip minimum, chip maximum,
rubbing floor, plunge rate). The placement does not judge these per-cut
limits (they have no edge on the plane), so the two do not contradict:
operator idx 6, 7, 8, 11, 16, 17; `nz_wanaka_21` idx 5, 9, 10, 13, 16, 17;
rivmap100 idx 0 to 3; live idx 0, 2, 3.

### The wire (P29-10 10i)

- New subject kind `stated_cut` {cut, occurs, at} in `limit_rows` (MCP
  `get_safe_envelope`, `get_suggest_rationale`, `get_tool_load_report`
  envelope part; CLI `suggest`, `project`).
- New caveat `at_suggested_chip`; new reason `no_stated_feed`.
- `envelope.operator`: `{kind: inside}` or `{kind: outside, binding}`; the
  `margin` key is gone. `outside_envelope` (the MCP `apply_feeds` reply):
  the `margin` key is gone. The `project.outside_envelope` message names
  the limit, not a share.
- `exemption` of a planned or stated cell that the scope does not judge
  names its scope row (P28-20, P28-10, P28-17, P28-04) in place of
  `S-NotJudgedHere`.

### Cost, the explainer and the text caps

- The time-budget test alone, six runs (load average 2.27): 182, 185, 186,
  193, 185, 182 ms (before 10i: 173 to 187 ms at 1.36). The stated judge
  runs once per envelope, outside the edge solves.
- `P29-10_explainer/data/` regenerated on the 10i tree (the envelope
  replies through the probe of the `get_safe_envelope` reply code, the CLI
  records through the release CLI). `assert_claims.py` reads the new
  subject: claim 2 accepts a stated cut chip row at the end of the
  interval of the op feed of its CLI record, and an entry chip row at the
  end of an interval of `1 + 1/lift` steps (it asserts a whole feed between
  the one-step and the two-step reading); claim 4 reads the stated body
  force-budget row as a `stated_cut` row. 11 of 11 claims pass.
- IC1 (`the_inspector_text_is_bounded_ic1`): each fixture now paints one
  face line (Pocket and 3D Rough "ramp entry · chip min 89 % under floor ·
  worst case", 3D Finish "body · chip min 78 % under floor"); the Passes
  caps rise by that line (41 to 42, 38 to 39, 30 to 31). This closes open
  item 4 of `P29-10_DESIGN.md` section 17.

## 13. P29-10z: rulings Q2, R and T (predicted before the code)

Baseline: HEAD `20fda1be1`. Method: the row probe of PB, 10h and 10i ran
again on HEAD (dump `p29_10z/dump_head.jsonl` in the agent scratchpad); a
script (`p29_10z/predict.py`) applies the rules below to the rows of the
dump. The prediction is the output of that script, not of the new code.

### The rules as they will be built

- **Q2, the declared list.** The modulator declares the limits that it
  caps (`dressup::feed_modulation::RunModulation::capped_limits`). The list
  holds `ChipMax` only:
  - `ConstrainedMax` caps each engaged move at `band max x lift x rpm x Z`
    (step 1); a feed scale above 1 pushes past that cap, so with a scale
    above 1 the list is empty.
  - `BandMid` caps at `band max x rpm x Z` with no lift, which is lower.
  - The power, deflection and force-budget arms can pin a move at the chip
    floor when no feed holds the budget, so the code does not prove that
    the modulated move is inside those limits. They are not in the list.
  - `ChipMin`: a point band has no floor, and a load cap wins over the
    floor (P20g). Not in the list.
- **Q2, the cuts.** The modulator re-times a move unless
  `should_skip_modulation` skips its intent (`EntryPlunge`,
  `EntryApproach`, `Drilling`, `LeadIn`, `LeadOut`, `Retract`,
  `ObstacleClearance`, every rapid). One match beside the modulator names
  the intents of each cut kind. A kind re-timed: body, first slot, first
  row, planner cap, spring pass, wall finish, ramp entry (`EntryRamp`),
  helix entry (`EntryHelix`), the three cleanups, capped step, slot ring,
  dogbone relief. A kind not re-timed: plunge entry, entry approach, lead
  in, lead out, drill cycle. Not decided from the code, so no caveat (a
  gap): the link (its moves are `Linking`, which the modulator re-times,
  or `Retract` and `ObstacleClearance`, which it skips) and the peck
  ladder (its plunges carry no intent of their own).
- **Q2, the row.** A planned or stated cut row of a limit in the list, on a
  cut kind that the modulator re-times, while the run modulates, carries
  `Caveat::BeforeModulation`. Only a row that holds a number takes it (a
  row with no number has no value that the modulator changes). The caveat
  softens an `Exceeds` row to caution. It ranks first among the caveats,
  so the face word of the row is "before modulation". The face carries the
  word: the longest face with the word on the four files is "planner cap ·
  chip max 2.9 % over limit · before modulation" (59 characters, operator
  idx 2). The row under its cut title in the drawer (`face_in_group`) does
  not carry the word (a 43-character row clipped at the rail width in PB);
  the chip word "advisory" and the hover carry it there.
- **Q2, the fact.** `RunModulation` (`Off`, or `Runs { strategy, feed_scale }`)
  is a field of `EnvelopeInput` with no default, and an argument of the
  doors `ProjectSession::safe_envelope`, `cutter_op_profile` and
  `limit_rows`. A disabled op is not simulated, so the session door gives
  it `Off`. The GUI and MCP read one value, the value that the GUI adopt
  step modulates with (`ConstrainedMax`, scale 1). The CLI `project`
  command reads its flag; the CLI `suggest` command runs no simulation and
  states the value of `SimulationOptions::default()` (on). FM1, the rules
  and the unit tests state `RunModulation::Off`.
- **Q2, the predicate.** `CheckRow::ranks_for_the_face` goes; every stated
  row ranks.
- **R.** `rank_cmp`: the tone group, then a row of a cut that always
  occurs before a row with the caveat "may not occur", then the subject
  (the stated point first), then the lower margin.
- **T.** The stated-point rows of the limits that are not limits of each
  cut get a title in the drawer, "the stated point" (a core constant).

### The face line of the 18 operator ops

| idx | Op | Today (HEAD) | After Q2 and R (predicted) |
|---|---|---|---|
| 0 | Face | none | none |
| 1 | 3D Rough | caution: first slot · depth 420 % over force cap · may not occur | caution: planner cap · depth 387 % over force cap |
| 2 | 3D Rough | caution: first slot · depth 120 % over force cap · may not occur | caution: ramp entry · depth 100 % over force cap · worst case |
| 3 | Pin drill | none | none |
| 4 | Drill | none | none |
| 5 | Project Curve | none | none |
| 6 | Pocket | caution: ramp entry · chip min 76 % under floor · worst case | the same |
| 7 | Pocket | caution: ramp entry · chip min 76 % under floor · worst case | caution: body · chip max 27 % over limit · before modulation |
| 8 | Pocket | none | caution: body · chip max 27 % over limit · before modulation |
| 9 | Project Curve | none | none |
| 10 | Trace V60 | none | none |
| 11 | Pocket | caution: ramp entry · chip min 66 % under floor · worst case | the same |
| 12 | Face | none | none |
| 13 | Trace V60 | none | none |
| 14 | 3D Rough | caution: first slot · depth 109 % over force cap · may not occur | caution: ramp entry · depth 78 % over force cap · worst case |
| 15 | 3D Rough | caution: first slot · depth 106 % over force cap · may not occur | caution: ramp entry · depth 84 % over force cap · worst case |
| 16 | Scallop | caution: helix entry · chip min 91 % under floor · may not occur | caution: body · chip min 16 % under floor |
| 17 | Profile | caution: plunge entry · plunge rate 17 % over limit · may not occur | the same (no row of a cut that always occurs exceeds) |

The causes:

- idx 1 (ruling R): the planner cap (387 %) always occurs; the first slot
  (420 %) may not. The body row (378 %) has a lower share over.
- idx 2, 14, 15 (ruling R): the ramp entry row always occurs and its depth
  share over (the depth per pass, the worst case) is larger than the share
  of the planner cap (74 %, 73 %, 57 %). The ruling does not rank a worst
  case after a certain cut, so the ramp line takes the face. The operator
  judges if this line reads as noise.
- idx 7, 8 (Q2): the body chip max row (+27 %) now ranks, with the caveat.
  On idx 7 the ramp chip min row (-76 %) has the caveat "may not occur",
  so the body row of a cut that always occurs ranks first.
- idx 16 (ruling R): the helix may not occur; the body chip min row
  (-16 %, advisory floor) always occurs.
- No face line is DANGER on the operator file: every stated chip max row
  that exceeds is on a re-timed cut and takes the caveat.

### The other files (predicted)

| File | idx | Today | After |
|---|---|---|---|
| `nz_wanaka_21.toml` | 1 | caution: first slot · depth 340 % over force cap · may not occur | caution: planner cap · depth 266 % over force cap |
| | 14 | caution: first slot · depth 104 % over force cap · may not occur | caution: planner cap · depth 69 % over force cap |
| | 16 | caution: helix entry · chip min 92 % under floor · may not occur | caution: body · chip max 50 % over limit · before modulation |
| | others | unchanged | unchanged |
| `rivmap100.toml`, `rivmap100_live_0925.toml` | all | unchanged | unchanged |

### The rows (predicted)

- Rows that take the caveat: operator 56 planned and 56 stated cut rows;
  `nz_wanaka_21` 40 and 24; each rivmap file 12 and 12 (224 in all).
- Tone `Exceeds` to `ExceedsAdvisory`: 14 rows. Stated cut rows: operator
  idx 1 (body, planner cap), 2, 14, 15 (planner cap), 7 and 8 (body, slot
  ring); `nz_wanaka_21` idx 16 (body). Planned rows: rivmap100 idx 1 and
  live idx 1 (planner cap, first slot).
- No state, value, bound or margin moves. The readiness and verdict rows
  do not move (no gate row carries the caveat, and no gate row has the
  caveat "may not occur", so ruling R does not reorder them).
- FM1 and the op golden do not move (no cutting number moves).

### Open for the operator (Q2)

The operator said on 2026-11-12: "simulation should run whats going to
happen in real life. having a post-sim change seems not ideal". This
caveat only names the split between the stated feed and the feed that the
modulator emits. The modulation design step decides the split.

## 14. P29-10z: measured beside predicted

The row probe of section 13 ran on the 10z tree (`p29_10z/dump_after.jsonl`,
the run modulation of `SimulationOptions::default()`, which is the value of
the GUI run). The script `p29_10z/measure.py` compares it with the
prediction and with the HEAD dump.

### The face line

44 of 44 face lines of the four files are as predicted (the cut, the
limit, the tone and the words). The 18 operator ops:

| idx | Predicted | Measured |
|---|---|---|
| 0, 3, 4, 5, 9, 10, 12, 13 | none | none |
| 1 | caution: planner cap · depth 387 % over force cap | the same |
| 2 | caution: ramp entry · depth 100 % over force cap · worst case | the same |
| 6 | caution: ramp entry · chip min 76 % under floor · worst case | the same |
| 7 | caution: body · chip max 27 % over limit · before modulation | the same |
| 8 | caution: body · chip max 27 % over limit · before modulation | the same |
| 11 | caution: ramp entry · chip min 66 % under floor · worst case | the same |
| 14 | caution: ramp entry · depth 78 % over force cap · worst case | the same |
| 15 | caution: ramp entry · depth 84 % over force cap · worst case | the same |
| 16 | caution: body · chip min 16 % under floor | the same |
| 17 | caution: plunge entry · plunge rate 17 % over limit · may not occur | the same |

10 of the 18 operator ops carry a face line, all in the caution tone; no
operator op has a danger line.

### The rows

| Quantity | Predicted | Measured |
|---|---|---|
| Rows with the caveat (operator, `nz_wanaka_21`, rivmap100, live) | 112, 64, 24, 24 | 112, 64, 24, 16 |
| Tone `Exceeds` to `ExceedsAdvisory` | 14 | 14, the same rows |
| Changes of state, value, bound or margin | 0 | 0 (the probe asserts the check of every row equal to HEAD) |
| Verdict rows (readiness) | no change | no change (equal to HEAD) |

The departure: `rivmap100_live_0925.toml` idx 2 and 3 are disabled. The
run does not simulate a disabled op, so the session door gives it
`RunModulation::Off` (the rule of section 13), and their 8 rows have no
caveat. The prediction script did not apply that rule.

### The face word

Decided by measurement:

- The card face carries the word. The longest exceeding face with the
  word on the four files is 59 characters ("planner cap · chip max 2.9 %
  over limit · before modulation", operator idx 2). A row whose face with
  the word passes 60 characters drops the word by the face rule (one row
  with no number on `nz_wanaka_21` would have 61; such a row takes no
  caveat).
- The drawer row does not carry the word. With the word in
  `face_in_group`, `the_card_shows_one_planned_row_and_a_drawer_p29_10`
  failed: "the rough: the drawer row "chip max 100 % of limit · before
  modulation ⓘ" is clipped at 340 points". The chip word "advisory" and
  the hover clause carry it there, as for the advisory floor of PB.

### Cost

The time-budget test alone, six runs (load average 4.67): 181, 183, 199,
182, 180, 204 ms (budget 300 ms; 10i read 182 to 193 ms at 2.27).

### Gaps and departures (for the lead)

- Ruling R moves the face of idx 2, 14 and 15 to the ramp entry line
  ("depth .. over force cap · worst case"), not to the planner cap: the
  ramp always occurs and its share over is larger. The ruling ranks only
  "may not occur" after a certain cut, not "worst case".
- Not decided from the code, so no caveat: the link (`Linking` is
  re-timed, `Retract` and `ObstacleClearance` are not) and the peck ladder
  (its plunges carry no intent of their own).
- The declared list holds `ChipMax` only. The power, deflection and
  force-budget arms can pin a move at the chip floor; `ChipMin` has no floor
  on a point band and a load cap wins over the floor. The chip minimum lines
  of idx 6, 11 and 16 are also numbers before modulation (`ConstrainedMax`
  raises a light move), but the code does not prove where the modulated
  chip ends, so they keep their tone.
- The test of the list proves the cap at the modulator's own band and lift.
  No test ties the band of `modulation_bands_for_session` to the bound of
  the envelope row, and no test reads simulated samples. A bandless toolpath
  and an own-material or not-judged move (S6) are not capped at the vendor
  band; the caveat does not see them.
- Only a row that holds a number takes the caveat. A disabled op reads
  `RunModulation::Off`, so its rows keep the gating tone; no test covers
  this rule.
- The face word: "before modulation" ranks first among the caveat words,
  so on the wire it replaces "stored feed", "worst case" and "may not
  occur" on the face of a capped row, `Within` rows included.
