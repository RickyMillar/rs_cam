# Fixes of the review of 2026-11-15

The lead rulings at the end of `REVIEW.md` set the scope. Each step has a
predictions section, written and measured before its code patch, and a
results section.

## R1 (F1): a fold ramp never goes below the cut it follows

### Cause

`dressup/entry_descent.rs` `collect_following_cut` took a tab lift and the
tab traverse into the run of the fold. `walk_fold_path` dropped the lift
vertex (it has no XY length), and `fold_ramp_points` held the ramp above the
cut only at a vertex. The return chord to the low vertex crossed the tab
below its top.

### Rules

1. The cut that a fold follows ends at a rise: a move of the cut whose end
   is more than `FOLD_RISE_EPS_MM` (1e-6 mm) higher than its start (a tab
   lift, a lead-out, a link). A fold never runs over a tab. A run that is
   too short for the fold takes the fallback that exists: the air lead and
   the stated plunge.
2. Every emitted fold point, and every point of each emitted segment, is at
   or above the followed cut at that position. `walk_fold_path` gives a
   vertex with no XY length the higher Z of the two points at that XY, so
   the vertex clamp holds the chord on both sides of it.

### Predictions (measured with the fix in place, before the patch)

- Operator file `nz_wanaka_21_operator.toml`: ops idx 0, 4 to 13 and 17
  dumped with the reviewer's probe (`zz_review_probe.rs`, no export) at HEAD
  and with the fix. HEAD dumps equal the reviewer's `now_<idx>.csv` byte for
  byte. Prediction: 0 of 12 ops move.
- Op golden (`every_op_keeps_its_toolpath_g_opgolden`): only the known 46
  rows of `opgolden_known_46_rows.txt`. Prediction: no R1 row. No re-bless.
- FM1 (`feeds_matrix_instrument_fm1`): no cell moves (the matrix holds no
  fold ramp).
- The other three project files: not generated (each op takes minutes).

### Tests

- `--lib dressup::tests::a_fold_ramp_never_runs_over_a_tab_review_f1`: the
  reviewer's reproduction. Red at HEAD (a ramp point at x 14.29 over the
  tab span x 3 to 20), green with the fix.
- `--lib dressup::tests::no_fold_ramp_point_is_below_the_followed_cut_review_f1`:
  400 generated cuts with lines, arcs and tabs (vertical and sloped lifts,
  traverses on a line or an arc), four angles, three tool radii, with and
  without the lap cap. Each emitted ramp segment is sampled at 41 points
  against the cut at that XY (arcs sampled from their own centre and
  angles). Red at HEAD (case 0: a sample at z -1.243 against the cut at
  -1.204 at that XY), green with the fix.

### Results

- Operator file: 0 of 12 ops move (all 12 dumps byte-equal to HEAD). As
  predicted.
- Op golden: only the known 46 rows. As predicted. No re-bless.
- FM1: 2 passed, no cell moves. As predicted.
- Gates: the 72 test binaries whose names hold entry, ramp, tab, clip,
  isoclip, boundary, dressup, helix or plunge pass, one at a time. Core
  `--lib`: 2820 passed, 1 failed (`the_envelope_meets_its_time_budget`, the
  known timing red of a parallel run).
- Not verified: the other three project files (no generation). The property
  test reads the lowest cut Z within 0.03 mm of a sample, so where a
  generated cut crosses itself, a floor segment can hide a tab top at the
  crossing.

## R2 to R5: predictions (rows, tones, faces, export warnings)

Measured with the code of each step in place, before the patches, with a
row dump probe (`zz_probe_rows_review.rs`: the P29-11 probe on the door of
P30, `ProjectSession::limit_rows(index, None)`) on the four project files,
READ-ONLY, no simulation: every envelope row (state, tone, caveats, face,
detail), the verdict rows, the Passes card face line (the worst stated row
by `rank_cmp` that exceeds) and, from R4 on, the export warning words of
`stated_cut_over_a_limit`.

### R2 (F2): `WorstCase` only where the value reads the ramp's own depth

- Rows: on each ramp row of a chip maximum, a chip minimum, a rubbing
  floor or a vertical rate, the caveat `worst_case` goes, and nothing else
  on the row changes. Count (planned and stated rows): operator 88,
  wanaka21 88, rivmap100 16, rivmap100_live 16 (22, 22, 4, 4 rows for each
  of the four limits). The depth ceilings and the power keep the caveat.
- Tones: 10 rows move from `exceeds_advisory` to `exceeds` (the safe side).
  No row moves to a softer tone.
  - operator idx 8 ANTENNA: ramp entry chip max (planned and stated);
  - wanaka21 idx 2 and idx 17: ramp entry rubbing floor (planned, stated);
  - rivmap100 and rivmap100_live idx 0 Face 5: ramp entry rubbing floor
    (planned and stated).
- Faces: a row that loses `worst case` shows its next caveat word or none
  (operator 66, wanaka21 82, rivmap100 12, rivmap100_live 12 faces).
- `worst case` becomes a caveat of the row, not of the cut: the drawer
  title of a ramp cut does not name it, and a depth row names it on its own
  line (`face_in_group`).
- Explainer data (`P29-10_explainer/data/`): the CLI `suggest --json` of the
  two files moves only `limit_rows[].caveats` (operator 88, live 16),
  `detail` (88, 16), `face` (66, 12) and `tone` (2, 2), the same rows as
  above. Claim 6 of `assert_claims.py` states the new rule.

### R3 (F3): `LowerBound` softens nothing

- No row of the four files changes (no lower-bound row exceeds there).

### R4 (F4): the export warns for every stated row with the gating tone

- No row changes. The warning `export.stated_feed_over_limit` becomes
  `export.stated_cut_over_limit` (wire change). With no simulation, no op
  has a current measured verdict. Predicted warnings, one per op (from the
  stated rows of the R3 dump with tone `exceeds`: the worst by rank and the
  count of the others; 0 mismatches against the product door):
  - operator: idx 1 (planner cap chip max 24 %, 1 more), idx 2 (chip max
    2.9 %), idx 7 (body chip max 27 %, 1 more), idx 8 (body chip max 27 %,
    2 more), idx 14 (chip max 3.4 %), idx 15 (chip max 3.1 %). The export
    of the reviewer's probe holds results for idx 0, 3 to 13 and 17 only,
    so it warns for idx 7 and idx 8, as at HEAD; ops 1, 2, 14, 15 are left
    out (`export.op_left_out`).
  - wanaka21: idx 2 (stated point, moves 78 % over limit, 1 more), idx 16
    (body chip max 50 %), idx 17 (ramp entry rubbing 48 % under floor; it
    gates since R2).
  - rivmap100: idx 0 (ramp entry rubbing 44 %), idx 1 (helix entry rubbing
    44 %), idx 2 (body rubbing 17 %), idx 3 (body rubbing 17 %, 1 more).
  - rivmap100_live: idx 0 (ramp entry rubbing 44 %), idx 1 (stated point,
    depth 33 % over vendor depth cap), idx 2 (body rubbing 17 %), idx 3
    (body rubbing 17 %, 1 more).
- Op golden: 49 `gcode_hash` rows move (the header warning lines). No
  `moves_hash`, `spans_hash`, feed or count row moves.
- The claim 3 of `a_row_softens_only_under_a_current_modulation_p30_2`
  changes: a fresh simulation with modulation off is a current measured
  verdict, so its export names the measured chip row (`load.chipload.*`)
  and no stated-cut warning.

### R5 (F5, F6)

- No row of the four files changes: no planned deflection row there lacks
  a chip or a width, no cut is past its search span, no file holds a
  current modulation, and no simulation runs (so (d) cannot show).
- Op golden: as after R4.

### The 18 Passes card face lines of the operator file (before; after R2 to R5)

| idx | op | before | after |
|---|---|---|---|
| 0 | Face back to T1 | none | none |
| 1 | Back bulk rough 18 mm | Exceeds: planner cap · chip max 24 % over limit · until modulated | Exceeds: planner cap · chip max 24 % over limit · until modulated |
| 2 | Back rest 6 mm (cavity, hollow, lakes, pockets) | Exceeds: planner cap · chip max 2.9 % over limit · until modulated | Exceeds: planner cap · chip max 2.9 % over limit · until modulated |
| 3 | Pin drill, flip pins | none | none |
| 4 | Drill light holes 6 mm end mill | none | none |
| 5 | River channel 6 mm, pass 7 of 7 | none | none |
| 6 | Lake pocket 2 mm, 11 lakes to Z 5.88 | ExceedsAdvisory: ramp entry · chip min 76 % under floor · worst case | ExceedsAdvisory: ramp entry · chip min 76 % under floor · may not occur |
| 7 | Lake pocket 2 mm, 1 lake to Z 3.99 | Exceeds: body · chip max 27 % over limit · until modulated | Exceeds: body · chip max 27 % over limit · until modulated |
| 8 | ANTENNA_D5.00 pocket 2 mm | Exceeds: body · chip max 27 % over limit · until modulated | Exceeds: body · chip max 27 % over limit · until modulated |
| 9 | V rivers and lakes | none | none |
| 10 | Day strip slit V60 | none | none |
| 11 | Face clamp pads 6 mm | ExceedsAdvisory: ramp entry · chip min 66 % under floor · worst case | ExceedsAdvisory: ramp entry · chip min 66 % under floor · may not occur |
| 12 | Face front to T | none | none |
| 13 | Engrave day labels V60 | none | none |
| 14 | Front bulk rough 18 mm | Exceeds: planner cap · chip max 3.4 % over limit · until modulated | Exceeds: planner cap · chip max 3.4 % over limit · until modulated |
| 15 | Front 3D rough | Exceeds: planner cap · chip max 3.1 % over limit · until modulated | Exceeds: planner cap · chip max 3.1 % over limit · until modulated |
| 16 | Front scallop finish | ExceedsAdvisory: body · chip min 16 % under floor | ExceedsAdvisory: body · chip min 16 % under floor |
| 17 | Outline profile with tabs | ExceedsAdvisory: plunge entry · plunge rate 17 % over limit · may not occur | ExceedsAdvisory: plunge entry · plunge rate 17 % over limit · may not occur |

Two faces change, by word only, with the same tone: idx 6 and idx 11 read
"may not occur" in place of "worst case" (the chip minimum of a ramp is
exact; the cut may not occur).

### R5 (e): cuts marked "may occur" that exceed with no later gate

The "may occur" tone rule does not change (lead ruling). For the operator:
these stated rows exceed and read advisory because the cut may not occur,
and no sample gate judges their limit after a simulation (`gate_of` gives
none for the rubbing floor, the vertical rate and the vendor depth table;
row `S-NoSampleGate`). The planned rows of the same cuts read the same.

| file | op | limit | row | face |
|---|---|---|---|---|
| operator | tp6 Lake pocket 2 mm, 11 lakes to Z 5.88 | rubbing_floor | U-EntryFallback | ramp entry · rubbing 46 % under floor · may not occur |
| operator | tp6 Lake pocket 2 mm, 11 lakes to Z 5.88 | rubbing_floor | U-EntryFallback | helix entry · rubbing 46 % under floor · may not occur |
| operator | tp7 Lake pocket 2 mm, 1 lake to Z 3.99 | rubbing_floor | U-EntryFallback | ramp entry · rubbing 46 % under floor · may not occur |
| operator | tp7 Lake pocket 2 mm, 1 lake to Z 3.99 | rubbing_floor | U-EntryFallback | helix entry · rubbing 46 % under floor · may not occur |
| operator | tp16 Front scallop finish | rubbing_floor | U-EntryFallback | helix entry · rubbing 73 % under floor · may not occur |
| operator | tp17 Outline profile with tabs | plunge_rate | U-EntryFallback | plunge entry · plunge rate 17 % over limit · may not occur |
| wanaka21 | tp2 Back rest 6 mm (cavity, hollow, lakes, pockets) | rubbing_floor | U-EntryFallback | helix entry · rubbing 48 % under floor · may not occur |
| wanaka21 | tp16 Front scallop finish | rubbing_floor | U-EntryFallback | helix entry · rubbing 77 % under floor · may not occur |
| wanaka21 | tp17 Outline profile with tabs | rubbing_floor | U-EntryFallback | helix entry · rubbing 48 % under floor · may not occur |
| rivmap100 | tp0 Face 5 | rubbing_floor | U-EntryFallback | helix entry · rubbing 44 % under floor · may not occur |
| rivmap100 | tp1 3D Rough | rubbing_floor | U-EntryFallback | ramp entry · rubbing 44 % under floor · may not occur |
| rivmap100 | tp2 Scallop Finish | rubbing_floor | U-EntryFallback | helix entry · rubbing 80 % under floor · may not occur |
| rivmap100_live | tp0 Face 5 | rubbing_floor | U-EntryFallback | helix entry · rubbing 44 % under floor · may not occur |
| rivmap100_live | tp2 Scallop Finish | rubbing_floor | U-EntryFallback | helix entry · rubbing 80 % under floor · may not occur |

The other rows that exceed under "may occur" (chip, power, force budget,
deflection) have a sample gate that judges the cut after a simulation.

### R2 results

- Cause: `CheckRow::planned` pushed `WorstCase` on every ramp row with its
  own depth, and the caveat softens.
- Fix: `PlannedOwnDepth::worst_case` holds only where
  `feeds::planned::PlaneRole::reads_the_cut_depth` is true (a depth ceiling
  or the power; each value check reads no depth: the doc states the inputs
  of each judge). `WorstCase` is a caveat of the row.
- Proof: claim 5 of `one_record_on_every_surface_p29_10` asserts literal
  tones: a ramp chip maximum at 1.5 x the band maximum is `Exceeds` with no
  `WorstCase`; the far ramp chip minimum carries only
  `AdvisoryFloor { OnePrintedValue }` and reads `ExceedsAdvisory`; each ramp
  depth row with a number keeps `WorstCase`. Claim 1 names the four limits
  that read the depth. Before the fix the chip maximum row read advisory.
- Rows: as predicted (all four files: only `worst_case` drops, on 208
  rows; 10 tones move to `exceeds`; no tone softens). Explainer data
  regenerated; `assert_claims.py` 11 of 11 PASS.
