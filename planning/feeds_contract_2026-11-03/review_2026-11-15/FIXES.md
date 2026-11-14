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
