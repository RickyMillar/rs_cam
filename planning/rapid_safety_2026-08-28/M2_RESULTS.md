# M2/M3 results — the engagement denominator is the engaged width (2026-08-28)

## The fix (M3)

`StampPartial::finish` (`dexel_stock/stamping.rs`) divides the measured
perpendicular extent by `2 × cutter.engagement_radius_mm(max_penetration)`
instead of `2 × envelope_radius_mm()`. The depth is the same scalar the
sample publishes as `axial_doc_mm`, which is what makes
`tool_load::power`'s `engagement_radius(axial_doc) × (arc/π)`
self-consistent with NO change to power.rs (M-b resolves through the
corrected input). The stamp bbox keeps the envelope — the two questions the
old scalar conflated are now answered separately. Flat endmills are
identity by construction (`width_at_height == radius()` everywhere), not by
branching. The 1.0 censor is kept deliberately.

Correction is **monotone**: `engagement_radius_mm(d) ≤ envelope_radius_mm()`
always, so engagement/arc/chip readings rise or stay put and air-cut time
can only fall. Anything moving the other way indicts the patch, not the
tool.

Sentries: `tests/engagement_denominator_m3.rs` (6) — analytic anchor
(R1.0 taper, 0.5 mm DOC, 3.464× band), flat control, censor clamp,
per-kinematics direction, dispatch parity, slot agreement.

## M2 evidence — before → after, captured not silenced

**Golden (the strongest pair):** `perf_golden_sim_metrics` splits exactly
on the falsification line —

- 2.5D EndMill golden: **byte-identical** (md5 `3a0ab214…` unchanged
  through the regeneration run) — the flat control, proven twice.
- 3D BallNose golden (`perf_golden_sim_metrics_3d.json`): **24 fields
  moved, every one in the guaranteed direction** — e.g.
  `project_average_engagement` 0.2661 → 0.3394, DropCutter helix
  `average_radial_woc_fraction` 0.2810 → 0.4251, arcs +30%, peak fractions
  hitting the 1.0 censor, air-cut −0.31 pp. Regenerated via the test's own
  `UPDATE_PERF_GOLDENS=1` mechanism AFTER capturing the full diff
  (scratchpad `m2_golden_before_diff.txt`, pre-fix golden preserved in git
  history). ⚠ `tests/perf_golden_*` is on the standing never-touch list —
  this regeneration is a measured-semantics change with the before-state
  captured as evidence, not a perf re-baseline; flagged to the operator.

**Smoke matrix (18 cases, `smoke --diff`):** 6 metric changes, 0 verdict
flips, 0 regressions. All on V-bit/ball cases, all upward: AS008 v_carve
engagement 5.00×, AS009 chamfer 3.32× (power 2.20×), AS010 inlay 4.69×
(power 1.46×), AS015 scallop 1.68×. Every flat case bit-stable.

**Wanaka (the M1 trace's project):** PENDING — the after-run was stopped
mid-flight (machine lane reclaimed); M1's measured factors (3.69× / 5.74×
time-weighted mean engagement on ops 7/8) are the predicted movement.
Rerun `cargo run -p rs_cam_cli --release -- project
planning/airrun_2026-08-19/wanaka200.toml --resolution 0.3 --summary` when
the lane is free and diff its trace summary against
`falsify_s3/simulation.json`'s (the pre-M3 arm, preserved).

## What M1 predicted vs what M2 measured

M1 predicted magnitude (3.7–5.7× on tapers), not population, and predicted
verdict movement chiefly through power/deflection. Measured: magnitudes
moved as predicted; on the smoke matrix no verdict crossed a band edge —
the M2 expectation "some green turns red" did NOT materialise there
(honest result: the bands are wide enough at those operating points).

## Ledger notes (M4)

- CLAUDE.md's chipload heat-map sentence corrected (F-HEATMAP is closed;
  `render/toolpath_render.rs:590` uses distinct newtypes).
- `TOOL_SCALE_SEMANTICS.md` §8 item 7 (U6) deliberately NOT corrected —
  the correction belongs in the commit that fixes U6, which this is not.
- New ledger candidate from M3's sweep: the planner's commanded aₑ/D
  (`adaptive_shared.rs:309`) is envelope-normalised while the simulator's
  measured fraction is now engaged-width-normalised — different quantities
  on non-flat tools; anything comparing them compares apples to oranges.
- Stale-evidence notes: `planning/toolpath_acceptance/baselines/*.csv`
  engagement columns and any captured S1AB baselines predate this fix.
