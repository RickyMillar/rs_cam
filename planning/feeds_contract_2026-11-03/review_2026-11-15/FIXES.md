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
