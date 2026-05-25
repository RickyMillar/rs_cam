# F-031 — Adaptive3d residual deep-Z planner↔simulator stamp parity gap

- **Stage:** sim / adaptive3d
- **Severity:** medium (closes the deflection bar to 7/7 once landed —
  same role F-029 was meant to play)
- **Status:** landed
- **First found in:** F-029 implementer pickup, 2026-05-26
- **Effort:** M-L (investigation-heavy; multiple plausible root causes;
  each sim run is ~60-90 s, slowing the diagnostic loop)
- **Linked PRs:** commit `497a3b2`
- **Resolution:** Root cause was the planner-↔-dressup helix entry-style
  parity gap (a hybrid of F-031 hypothesis #4 — frame interaction — but
  the "frame" turned out to be the planner-emission vs dressup-rewrite
  axis, not world/local). Fix: narrow the `prefer_helix` override in
  `DressupConfig::normalize_for_op` to 2D `Adaptive` only; force
  `entry_style = None` for `Adaptive3d`. Stamp-event diagnostic ruled
  out hypotheses #1/#2/#3 — planner and simulator share identical
  grids and stamp functions; only the toolpath shape (planner-emitted
  plunges vs dressup-rewritten helices) differed. Acceptance tests
  re-enabled in
  `crates/rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs`
  (`as013_terrain_whole_toolpath_axial_within_commanded_dpp_f031` +
  `as013_terrain_deflection_within_safe_band_f031`) pass post-fix.
- **Source audits:** F-029 implementer's per-cell diagnostic probe
  (`debug_adaptive_3d_segments_for_f029_probe`) on AS013 against
  `test_data/ux_3d_terrain.toml` via `ProjectSession::run_simulation`,
  after F-029's partial-landing cleanup-raster DPP clamp landed.

## Symptom

After F-029's partial landing (cleanup-raster per-cell DPP clamp +
planner-state diagnostic probe), AS013 adaptive3d on
`ux_3d_terrain.toml` still reports:

- worst `axial_engagement_mm` = 44.82 mm on a 3 mm-commanded DPP
  (sample at x ≈ -2.40, y ≈ 18.49, z ≈ 6.57)
- 282 of 1,006,552 cutting samples exceed `depth_per_pass + 0.5`
- `deflection.peak_mm` = 0.66 mm Exceeds

Diagnostic probe at the worst cell `(-2.5, 18.5)`:

- planner's final `material_stock` reads **`top = 0.5 mm`** (fully cleared)
- simulator's per-setup dexel **disagrees** — the worst-case sample at
  `z = 6.57` reads axial ≈ 44.8 mm, implying the simulator's stock at
  that XY was still at ≈ 51.4 mm (the cell's depth after the first four
  passes only) when the deepest pass swept through.

Sample history at the worst XY:

| Z pass | move idx | axial | Note |
|---|---|---|---|
| 54.565 | 236 | 2.062 | first pass, normal DOC |
| 51.565 | 1349 | 2.062 | second pass |
| 48.565 | 3044 | 2.062 | third pass |
| 45.6 | 6442 | 1.85-2.22 | fourth pass |
| **42.5 → 9.5** | **(none in 2 mm filter)** | **(none)** | **13 Z levels skipped** |
| 0.500 | 670155 | 0.053 | final, no axial inflation |

The deepest pass at z=6.57 is itself NOT in the sample-history filter
(out by > 2 mm from the (-2.5, 18.5) test point) — but the worst sample
at x = -2.40, y = 18.49 IS the failing one. So between passes at z=45.6
and z=6.57, the simulator's stock at this XY (or a nearby XY) was not
stamped down — even though the planner's `material_stock` ended at
`top=0.5`.

## Hypothesised root cause

The planner-side state is correct. The gap is on the simulator side or
in the planner→simulator handoff. Four candidates, in order of
likelihood:

1. **Sim sample density / per-Z-level filter parity.** The simulator
   tags samples by move boundary, not by stock cell coverage. A move at
   z=42 whose centreline passes within `tool_radius` of (-2.5, 18.5)
   stamps the cell, but the per-sample axial reading is taken at sub-
   sample centroids along the move. The histogram filter in the
   acceptance test (`within 2 mm of XY`) just may not catch every
   stamping move; the test diagnostic is incomplete proof of "the cell
   was missed". Need to instrument with **per-cell stamping events**
   rather than per-XY-near-cell sample filtering to confirm.

2. **Sim resampling at planner cell boundaries.** Planner cell_size for
   AS013 is `max(tool_radius/6, tolerance) = max(0.5, 0.25) = 0.5 mm` —
   matching the test's `resolution = 0.5`. So in this configuration
   resolutions agree. But the planner's stamp uses a swept-tube radial
   profile LUT; the simulator stamps a separate swept-tube with its own
   LUT and resampling cadence. Boundary cells (where the cutter
   centerline passes near the cell edge, not center) could be stamped
   differently between the two grids. Audit the planner-↔-sim parity
   for cells where the cutter passes at distance ≈ `tool_radius - eps`.

3. **Per-setup dexel origin/extent mismatch with planner grid.** F-027
   widened the planner grid to the union of mesh-bbox+r and world-stock-
   bbox. The simulator's per-setup dexel grid is independently bounded
   by the world stock bbox; there's no shared single source of truth.
   The widened planner grid's origin (`min(mesh.min - r, world.min)`)
   may not exactly equal the simulator's grid origin if the simulator
   does any padding/snapping. Worth instrumenting both origins and
   cell counts on a real run and asserting they match.

4. **F-028 / F-024 frame interaction.** AS013 uses the identity-setup
   path that F-024 and F-028's viz follow-up fixed. If a non-identity
   transformation is sneaking through, the simulator would interpret
   moves in a different frame than the planner stamps. F-030's
   architecture refactor is the structural fix here; an interim
   defensive cross-check (planner-stock-bbox vs simulator-stock-bbox
   assertion) would catch this if it's the culprit.

## Fix shape (sketch)

Diagnostic-first, then targeted fix. Recommended order:

1. **Stamp-event diagnostic:** add a planner-side per-stamp callback
   that logs `(move_idx, x, y, z, tool_radius)` to a side channel. Mirror
   the same in the simulator. Compare the two streams on AS013 — the
   first move where they diverge is the bug location.

2. **If (1) confirms simulator-side gap:** harden the simulator's
   swept-tube stamping to match the planner's LUT-based approach
   exactly. Likely requires touching `crates/rs_cam_core/src/dexel_stock/`
   — **out of scope of F-029's `crates/rs_cam_core/src/adaptive3d/`
   constraint**, hence the F-031 split.

3. **If (1) confirms planner-side gap:** the planner is stamping the
   cell but emitting a move that doesn't reach it. Investigate the
   `RapidWithFloor` peck-plunge stamping (planner stamps a vertical
   tube but the simulator's peck-plunge feed pattern is the
   interleaved peck/retract, which the planner's
   `stamp_linear_segment` may not exactly mirror).

## Acceptance test

Re-enable the existing `#[ignore]`d tests in
`crates/rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs`:

1. `as013_terrain_whole_toolpath_axial_within_commanded_dpp_f029` —
   worst-case axial ≤ `depth_per_pass + 0.5` margin.
2. `as013_terrain_deflection_within_safe_band_f029` —
   `deflection.peak_mm < 0.2`.

Both currently fail with axial=44.8 mm and deflection=0.66 mm.

## Risk / notes

- F-031 is a follow-up to F-029, not an independent finding. The
  partial-landing of F-029 (cleanup-raster clamp + diagnostic probe) is
  defensible defensive coding and doesn't regress anything; it just
  doesn't close the worst case.
- F-017 (rapid collisions) closure still depends on F-031 — the 3D-op
  collision surface waits on the same mechanism.
- F-030 architecture refactor unblocked once F-031 lands AND the 7/7
  deflection bar is met (precondition unchanged from F-029).
- Investigation will likely require touching
  `crates/rs_cam_core/src/dexel_stock/` (sim-side) which F-029's pickup
  intentionally stayed out of. That scoping decision is what split
  F-031 from F-029.
