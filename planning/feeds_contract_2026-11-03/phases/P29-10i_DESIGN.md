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
