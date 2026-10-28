# W4 pre-registration: the Waterline through-cut floor rule (D69 §4, rank 4)

Written before any code, on HEAD 2833d66bc (W2b). For the lead's approval.

## The ruling (2026-10-28)

On a through cut, the Waterline stops at the web the job holds, and the job ends with a tabbed Profile. The Waterline
gets no tabs.

## The defect

`ops/waterline.rs waterline_ladder` drops only the levels at or below the stock bottom + `STOCK_BOTTOM_FLOOR_EPS_MM`
(1e-6 mm). The web that the Waterline leaves on the part outline is the lowest level less the stock bottom: a phase of
the ladder, not a rule. With top 18 and z_step 0.7 the last level is 0.2 (D69 §4); with z_step 1 on a 18.0005 top it is
0.0005 mm.

## When the rule acts (all three hold)

1. The part goes through: the model bottom is at the stock bottom, to within the ladder's own "on a face" tolerance
   (`FLAT_FACE_SNAP_MM`, `ops/waterline.rs:114`). A model that stands on stock (a relief) keeps today's ladder.
2. The ring is an OUTER ring of its level: no other ring of the level holds it (the W1 containment test
   `ring_inside`, to within `sampling`). Only an outer ring separates the part from the stock. A hole ring, an inner
   wall ring or a pocket ring keeps its levels (D69: the 83 hex holes keep levels 3, 2, 1).
3. The ring's level is at or below the web Z (below).

Such a ring is not cut. The level's other rings are cut as today. The helix (W1) chains only the rings that are cut.

## The web Z (derived, no typed number), first that applies

1. A later tabbed Profile: an enabled Profile after the Waterline in the same setup that holds tabs (`tab_count > 0`
   or `tab_points` not empty) and whose floor reaches the model bottom (`Floor::ModelBottom { depth >= 0 }`, the
   `frees_the_part` floor of R-A, or any floor at or below the stock bottom). Web Z = stock bottom + its `tab_height`.
   The tab top of that Profile is its final floor + `tab_height`, and its final floor is at or below the stock bottom,
   so the Waterline web is at or above the tab top: the Waterline never cuts below a tab. Several such Profiles: the
   highest web.
2. Else the lowest floor an earlier op left: the lowest cutting Z of the current results of the earlier enabled ops
   of the setup, when it lies above the stock bottom + `STOCK_BOTTOM_FLOOR_EPS_MM`. On the Corne play job: op 1, 5 and
   3 cut down to Z 0.5 (op 1's axial leave, D69 `web_base_corne.txt`), so web Z = 0.5. An earlier op that already cut
   through gives no web (rule 2 does not apply).
3. Else no web: today's ladder, unchanged.

The session derives the web Z (it sees the later and earlier ops) and gives it to the generator through one new
`ExecutionContext` field, `through_cut_web_z: Option<f64>` (beside `rest_top_z` and `rest_axial_step_mm`), filled in
`session/compute.rs` and `session/compute/generation.rs`.

## The notice, and the Profile: a check that warns (recommended)

- When the rule acts (it skips at least one outer ring), the Waterline ladder finding (`WaterlineLadderFinding`,
  `compute/toolpath_stats.rs`) records the web Z and the count of skipped outer rings. The diagnostics and the
  narration read the finding (one line, for example "Waterline stops at the web Z 0.5 (later tabbed Profile)").
- With a through part and no later tabbed Profile, the same finding carries "no tabbed Profile follows". The
  diagnostic shows the existing R-A text `PART_FREE_TEXT` style as a Caution: "This job does not hold the part at the
  end: add a tabbed Profile on the outline." No new op is added by W4.
- Recommendation: W4 is the check that warns. Adding the Profile is a template job (step 7.8 owns "add it"); a
  generator that adds an op would be a second door for `AddToolpath`.

## Predicted change and re-bless

- `op_golden_g_opgolden.toml` `waterline/fresh`: unchanged (no later Profile, no earlier op: rule 3).
- `waterline/rest`: may move. The cap fixture's model bottom is the stock bottom (the ground at Z 0), so rule 1 holds;
  the earlier 3D Rough (default `stock_to_leave_axial` 0.5) gives web Z = its lowest cut. The golden's lowest level is
  1.0 (`bbox_min` Z 1.0), so no ring is skipped when that cut is at or below 1.0. Predicted: unchanged. Re-blessed only
  if the rough's lowest cut on that fixture is above 1.0, with the first differing move named.
- `perf_golden_sim_metrics_3d.json`: its Waterline has no earlier op and no Profile: unchanged.
- Every other golden: unchanged.

## Sentries (new file)

1. A through plate with a hole, a Waterline and a later tabbed Profile (`tab_height` h): no cut move of the outer ring
   below stock bottom + h; the hole ring keeps its low levels; the finding names the web and the skipped count.
2. The same job with no Profile and an earlier rough that leaves a floor f: no outer-ring cut below f.
3. A relief (model bottom above the stock bottom): the output equals today's, move for move.
4. A through part with no web (no Profile, no earlier op): today's ladder, and the finding says no tabbed Profile
   follows.

## Questions for the lead

1. Is the per-ring rule (outer rings only) right, or must the whole level stop at the web?
2. The check that warns (recommended) or an op that adds the Profile?
3. Rule 2 reads the earlier ops' current results (their lowest cutting Z). A stale or missing result gives no web.
   Is that acceptable, or must the web come from the earlier ops' configs (their resolved floors)?
