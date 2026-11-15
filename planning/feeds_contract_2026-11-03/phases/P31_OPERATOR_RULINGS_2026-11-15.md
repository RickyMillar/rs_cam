# P31: four operator rulings of 2026-11-15

Written 2026-11-15 at HEAD `7c28a48c5`. Each task has a predictions section, written before its
code patch, and a results section. "Measured" is a number from a run in this step. "Reasoned" is
read from the code, not run.

## A. The clamp boxes of setup "3 Front" (no code)

The operator ruled that the corrected boxes of `P29-8e_OBSTACLE_FRAME.md` section 5 go into the
two project files. Commit `7c28a48c5` wrote them (operator file sha256
`ee6869be015b633b424df0fadbf13a38874e0ef1e703c8c148ccdeeaa9d29666`). This task verifies the
toolpaths. It changes no code and no project file.

### A: predictions (before the run)

Method: the reviewer's probe (`review/zz_review_probe.rs.txt`, with a `collision_check` call for
each op and no export) generates operator ops idx 0, 4 to 13 and 17 at HEAD with no simulation,
`--release`. Each dump is compared with the reviewer's dump of commit `109ed6644`
(`review/now_<idx>.csv`).

- Idx 0 and 4 to 11 (setups "1 Back" and "2 Clamp pads"): byte-equal. A difference is a STOP.
- Idx 12, 13 and 17 (setup "3 Front") can differ. Idx 14, 15 and 16 are in that setup too; they
  take minutes each and are not generated.
- Idx 17 (Outline profile with tabs): the clip re-entries go from 42 to about 4. The frame note
  predicted "1560 moves, 4 EntryPlunge, no gap above the floor" from an in-memory probe at an
  older commit, before the re-entry ramps of `910ded4b4`. So the move count is not expected to
  equal 1560: each of the 4 re-entries can be a fold ramp, not a plunge. The cycle time falls.
- The remaining re-entries are at the floor level, 2 on each side in X, where the head band
  (radius 12.5 mm) reaches the grown clamp when the tip is below Z 0.5.
- No obstacle hit on a final toolpath (`ProjectSession::collision_check`).

### A: results (measured)

Run: release build of HEAD `7c28a48c5`, 12 ops generated in one session, no simulation. The dumps
and the probe are in the agent scratchpad (`p31/A/now_<idx>.csv`, `p31/A/zz_p31_probe_a.rs.txt`,
`p31/A/analyze.py`).

- Idx 0, 4, 5, 6, 7, 8, 9, 10, 11 and 13: byte-equal to the review dumps (`cmp`). No op of
  another setup moves. Idx 13 (setup "3 Front") does not move.
- Idx 12 and idx 17 move.

| | idx 12 Face front to T | idx 13 Engrave day labels V60 | idx 17 Outline profile with tabs |
|---|---|---|---|
| Moves, before / after | 371 / 361 | 314 / 314 | 1840 / 1566 |
| Clip re-entries, before / after | 2 / 0 | 0 / 0 | 42 / 4 |
| Straight plunges into stock (`EntryPlunge`), before / after | 2 / 0 | 31 / 31 (the op's own plunge entries, no change) | 0 / 2 |
| Obstacle hits on the final toolpath (`collision_check`) | 0 | 0 | 0 |
| Cycle time (s), before / after | 286.31 / 281.06 | 31.66 / 31.66 | 740.54 / 672.65 |

"Before" for idx 17 is the review dump of `109ed6644`, which holds the re-entry ramps of
`910ded4b4`: its 42 clip re-entries are fold ramps, not plunges. Count: the review dump has 78
entry runs and the new dump has 40. Each has 20 level starts and 16 tab descents (8 tabs on each of
the two lowest levels). The rest are clip re-entries: 78 - 36 = 42 and 40 - 36 = 4.

**Idx 12.** Before, 15 rows stopped at the old boxes (X 1.62 to 356.38, the frame of setup 1) and
two short pieces at X 401 to 404 had a plunge each. After, the 16 rows near the clamps run
X 20.12 to 374.88 (20.12 + 374.88 = 395: symmetric about the stock centre X 197.5), every other row
runs X -9 to 404, and no plunge is left.

**Idx 17.** Above the floor level no pass is clipped: each level from Z 23.75 to Z 1.25 is one
loop with its tab ramps. At the floor level (Z 0) four re-entries are left, two on each X side:

| # | X | Y | Z | Kind | Feed (mm/min) |
|---|---|---|---|---|---|
| 1 | 15.5 | 185.67 | 0 | fold ramp (air lead to Z 1.75) | 3165 |
| 2 | 15.5 | 327.25 | 0.5 | straight plunge from Z 2.375 | 1582 (the plunge rate) |
| 3 | 379.5 | 274.33 | 0 | fold ramp (air lead to Z 1.75) | 3165 |
| 4 | 379.5 | 132.75 | 0.5 | straight plunge from Z 2.375 | 1582 (the plunge rate) |

The reason is the head zone at the floor: the default head band (radius 12.5 mm) reaches the
grown clamp when the tip is below Z 0.5, on both X sides. The positions agree with the frame note
(X 15.5 at Y 185.67 and 327.25; X 379.5 at Y 274.33 and 132.75). The note predicted 4 `EntryPlunge`
and 1560 moves; with the re-entry ramps two of the four are fold ramps and the op has 1566 moves.

Each straight plunge is at the start of a tab. The kept piece there is the upper part of the
vertical tab lift (Z 0.5 to Z 2.0), so the tool plunges to Z 0.5 and then lifts over the tab. The
level before it cut that position to Z 1.25, so the plunge enters 0.75 mm of stock.

**For the operator: the floor level is not cut next to the clamps.** On each X side the floor pass
(Z 1.25 to Z 0) has two gaps, each 52.92 mm long and next to a tab: Y 132.75 to Y 185.67 and
Y 274.33 to Y 327.25, on X 15.5 and on X 379.5 (symmetric about Y 230). In each gap a skin of
1.25 mm stays; on the 9.5 mm next to one tab of each side the clipped tab ramp leaves 0.5 mm to
1.25 mm. So the part stays attached there after the op, as it does at a tab.

**Verdict.** The corrected boxes do what the frame note said. Only ops of setup "3 Front" move
(idx 12 and idx 17 of the ops that were generated). No obstacle hit. Not generated: idx 1, 2, 3,
14, 15 and 16 (idx 14, 15 and 16 are in setup "3 Front" and can move).
