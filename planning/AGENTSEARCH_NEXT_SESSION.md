# AgentSearch / chipload-gate auto-fix plan

## Hard goal

**The auto-fix chain (suggest module → optimizer → toolpath) must produce a working wanaka project with no manual intervention.** Wanaka is the test. If the user has to apply a feed/RPM by hand, it's not done.

Current state: the chipload gate trips at `0.0875 mm/tooth` on Back Rough. Optimizer refuses with `BipolarEngagement`. Suggest module recommends a row but applying it doesn't drop chipload below the LUT cap. Three independent root causes are blocking the chain — fix each in isolation, validate per-cause, then check the end-to-end on wanaka.

## Test posture

- **No MCP, no GUI.** Pure `rs_cam_core` integration tests. Faster iteration, no live-binary reload.
- **No mesh/project changes.** Wanaka project file stays as-is. Fixes are in the engine, not the input.
- **Synthetic test fixtures.** Each worktree builds its own minimal repro that's deterministic and fast (<5s).
- **Live wanaka via `tests/wanaka_axial_doc.rs`** is the END-TO-END check, run only after all three worktrees land.

## Worktree split

Three independent investigations, each in its own worktree, none depend on the others' fixes. Run them in parallel.

```
~/work/rs_cam_o5b — coverage gaps in 2D adaptive
~/work/rs_cam_o5c — sim/plot disagreement
~/work/rs_cam_o6  — chipload formula
```

Pre-flight: from master, run `cargo test --workspace -q` to confirm green baseline. Each worktree branches off master.

---

## Worktree 1 — O5b: AgentSearch interior coverage gaps

### Problem statement

The 2D adaptive's agent-search walk leaves cells uncovered inside the polygon. At deeper z-levels these cells then bite through full-depth uncleared stock (axial DOC = `stock_top - z_level`, up to 18mm on wanaka).

### Repro (synthetic, deterministic)

Add `crates/rs_cam_core/tests/agent_search_coverage.rs`:

1. Construct a synthetic mesh that shapes a CONCAVE silhouette (e.g., L-shaped or with a notch) at known XY coordinates. **Not** the wanaka mesh — predictable geometry only.
2. Build dexel stock matching adaptive3d's expected dimensions (`mesh.bbox + cutter_radius`).
3. Run `adaptive_3d_segments` with `clearing_strategy = AgentSearch`, `region_ordering = Global`.
4. After each z-level's stamping, walk the dexel grid: for every cell where `bool_grid[cell] == true` (material to clear), assert `ray_top[cell] <= z_level + epsilon` after the stamp.
5. Currently fails: certain interior cells stay at `ray_top = stock_top` even after the z-level stamp.

### Pass criteria

After EVERY z-level pass, EVERY cell in the bool-grid polygon has `ray_top <= max(z_level, surface + stock_to_leave) + epsilon`. No exceptions.

### Files

- `crates/rs_cam_core/src/adaptive3d/clearing.rs` (`clear_z_level_agent_2d_slice`)
- `crates/rs_cam_core/src/adaptive/path.rs` (the 2D adaptive's `adaptive_segments_with_debug`)
- `crates/rs_cam_core/tests/agent_search_coverage.rs` (new)

### Likely fix direction

Two options to evaluate:

**A. Contour-parallel cleanup pass at each z-level** — after `clear_z_level_agent_2d_slice` runs, scan the bool grid for any remaining `true` cells. For each connected region of remaining cells, run a contour-parallel sweep (offset rings inward from the region boundary). This is what `waterline_cleanup` does at the bottom Z, but applied to ALL Z-levels and to ALL leftover cells, not just contour-traced ones.

**B. Replace agent-search with full-coverage spiral inside `clear_z_level_agent_2d_slice`** — switch the 2D adaptive call to a guaranteed-coverage strategy (e.g., the existing `clear_z_level_contour_parallel` with EDT-based region detection).

**Decision rule:** option A first (additive, lower regression risk). If the cleanup pass is too slow or produces ugly toolpaths, fall back to B.

### Out of scope for this worktree

Don't touch the lift function, don't touch the slope-aware Cut split, don't touch boundary handling. The bug is interior coverage — fix only that. The test must show pass on the synthetic case before this worktree merges.

---

## Worktree 2 — O5c: simulator chipload-vs-material disagreement

### Problem statement

User observed visually: "second sweep, the graph says the chipload is high, but in the sim NO MATERIAL IS REMOVED on this second pass." The chipload sample's reported `arc_engagement_radians` is high, but the dexel state at the cutter's actual position has no material to remove. Either:
- The sample is computed BEFORE the prior sample's stamp updates the dexel (stale read)
- The midpoint-based engagement calc disagrees with the per-cell stamp's depth
- Something else in the sim's stamp order

### Repro (synthetic, deterministic)

Add `crates/rs_cam_core/tests/sim_chipload_invariant.rs`:

1. Build a 50×50×10 mm rectangular dexel stock.
2. Sweep cutter (6mm flat endmill) from (10, 25, 8) to (40, 25, 8) at feed_rate. This first sweep clears stock to z=8 along that strip.
3. Sweep cutter back from (40, 25, 8) to (10, 25, 8) at feed_rate at the SAME Z. Second pass through cleared area.
4. For every sample in the second sweep, assert: `arc_engagement_radians > 0.05 ⟺ removed_volume_est_mm3 > 0`. They must agree.
5. Currently expected to fail: samples in the cleared trail report nonzero arc engagement when material is gone.

### Pass criteria

For every cut sample emitted by `simulate_toolpath_with_metrics_with_cancel`:
- If `arc_engagement_radians > 0.05`, then `removed_volume_est_mm3 > 0` (cutter actually removed material that sample).
- If `removed_volume_est_mm3 > 0`, then `arc_engagement_radians > 0.05` (a stamp that removed material must have non-trivial engagement).

The two must NEVER disagree by a factor of >2× over a contiguous run of samples.

### Files

- `crates/rs_cam_core/src/dexel_stock/simulation.rs` (sample generation, stamping order)
- `crates/rs_cam_core/src/dexel_stock/stamping.rs` (`stamp_segment_with_metrics`, especially the per-cell loop at line 277+)
- `crates/rs_cam_core/tests/sim_chipload_invariant.rs` (new)

### Likely fix direction

The `mid_d = midpoint.z` is the cutter's Z at the segment midpoint, but the per-cell stamp uses `depth = sd + t * seg_dd` per-cell (where `t` is the cell's projection parameter). If the segment has any Z change OR the cell's `t` puts the local cutter at a different Z than the midpoint, engagement is computed against a Z the cutter isn't actually at when stamping that cell.

**First check:** add a second sample-time read of the dexel state AFTER the stamp, compare with pre-stamp. If `pre - post` per cell ≠ what `axial_doc_mm` reports, the bug is in the metric → assertion mismatch.

**Possible fix:** compute `arc_engagement_radians` AFTER the per-cell stamps (engagement should reflect what was actually engaged), not at the midpoint snapshot.

### Out of scope

Don't touch toolpath generation. Don't touch the chipload formula (that's worktree 3). Don't change the public sample shape. Just make sample's reported engagement consistent with what its stamp removed.

---

## Worktree 3 — O6: chipload formula gate trip

### Problem statement

`flat_chip_geometry_for_radius` (in `tool/mod.rs:124`) caps `h_max` at full nominal `feed_per_tooth_mm` whenever `arc_engagement_radians ≥ π/2`. This is geometrically correct for "max chip thickness on the engagement arc" but the LUT cap the gate compares against is calibrated for a DIFFERENT quantity (the per-engagement-fraction effective chipload).

End effect: any sample where the cutter is at least half-engaged (arc ≥ π/2) reports chipload = nominal feed/(rpm × flutes) = 0.0875 on wanaka. The gate trips. Even after axial DOC = 3mm. **This is the actual gate blocker.**

### Repro (synthetic, deterministic)

Add `crates/rs_cam_core/tests/chipload_formula_calibration.rs`:

1. Compute `chip_geometry` for a 6mm flat endmill with:
   - `axial_doc = 3.0 mm` (one full pass)
   - `arc = 0.7 rad` (≈40°, partial engagement)
   - `feed_per_tooth = 0.0875` (wanaka nominal)
2. Compute the SAME parameters with `arc = π/2` (half engagement).
3. Compare against the LUT row's `chip_load_max_mm` for the same tool/material.
4. Currently expected: at `arc = π/2`, formula returns 0.0875 (= nominal). LUT cap is ~0.025. Formula > LUT → gate fails.
5. After fix: formula and LUT use the same convention so the gate's comparison is meaningful.

### Pass criteria

Two-part:

1. **Formula consistency.** `chip_geometry` returns a chip-thickness value the gate's LUT-cap comparison is calibrated for. The convention (max chip thickness on arc vs average vs equivalent rectangular) must match what the LUT was authored against. Pick one and document it.
2. **Wanaka end-to-end.** With this fix alone (no other worktrees), wanaka Back Rough's `peak_chipload_mm_per_tooth` drops below the LUT cap when commanded stepover is small (e.g., 14% radial). The gate's verdict for TP1 becomes `Within`.

### Files

- `crates/rs_cam_core/src/tool/mod.rs` (`flat_chip_geometry_for_radius`, lines 106-147)
- `crates/rs_cam_core/src/dexel_stock/simulation.rs` (`effective_chip_thickness_mm`, line 465)
- `crates/rs_cam_core/src/tool_load/chipload.rs` (gate logic comparing samples to LUT)
- `crates/rs_cam_core/tests/chipload_formula_calibration.rs` (new)

### Likely fix direction

Three options to evaluate:

**A. Switch the formula to AVERAGE chip thickness on arc.** Replace `h_max = feed_per_tooth × sin(arc).abs()` (and the `arc ≥ π` slot case) with the integral-average chip thickness across the engagement arc. This is `mean = (2 h_max / arc) × (1 - cos(arc/2))` which the formula ALREADY computes for `mean_chip_thickness_mm` — switch the gate to read `mean` instead of `max`.

**B. Calibrate the LUT cap to "max chip thickness on arc" semantics.** Update the LUT data (or scale it at lookup time) so the gate compares apples-to-apples.

**C. Use `effective_chip_thickness_mm` with a different `EngagementMode`.** Currently always `Slot`. Could be `LightFinish`/`HeavyRoughing` to honor partial engagement.

**Decision rule:** option A first if `mean_chip_thickness_mm` matches what the LUT was calibrated against. Document the convention in `tool_load/chipload.rs` so future code stays consistent.

### Out of scope

Don't touch toolpath generation. Don't touch the simulator's stamping. Just make the gate's comparison meaningful.

---

## End-to-end validation (after all three worktrees merge)

### Wanaka regression test

Add `crates/rs_cam_core/tests/wanaka_e2e_chipload_gate.rs`:

```text
1. Load wanaka_full_tuned.toml via ProjectSession::load.
2. Generate Pin Drill (TP0) + Back Rough (TP1).
3. Run simulation with capture_arc_engagement=true.
4. Get tool_load_report.
5. Assert TP1's chipload verdict is `Within` (not Exceeds).
6. Assert peak_axial_doc_mm <= depth_per_pass + 0.5 (3.5mm).
7. Assert peak_chipload_mm_per_tooth < LUT_cap_for_TP1_tool.
```

This is the BAR. If this test passes, the auto-fix chain works. If it doesn't, at least one worktree didn't fix its issue properly.

### Suggest module + optimizer chain

After the wanaka regression test passes:

1. Run `suggest_feeds_speeds` on wanaka. Confirm it picks the 1.587mm 2-flute ZrN row (it already does — this should be unchanged).
2. Apply the suggested feed+RPM via `set_toolpath_param`. Regenerate. Re-simulate.
3. Confirm verdict is still `Within`.
4. Run `optimize_toolpath` on wanaka. Confirm it doesn't refuse (no BipolarEngagement). 
5. The optimizer's output should match or improve on the suggest module's recommendation.

If any of these fail, the chain has a bug in the suggest/optimizer plumbing, not the gate input. Track separately.

---

## Pre-merge cleanup (this current session)

Before starting the new sessions, leave master in a clean state:

1. Identify the ONE clean fix in the current diff worth keeping: `simplify_path_3d` closed-loop bug (early-returns on first==last). Extract that fix + add a regression test, commit to master.
2. Stash the rest (slope-aware Cut split, perimeter sweep, boundary pre-clip plumbing, AgentSearch coverage workarounds) on a `wip/agentsearch-investigation` branch. Don't merge.
3. Confirm `cargo test --workspace -q` green on master.
4. Document this plan exists.

The investigation work isn't wasted — it found three real bugs and informed the worktree plan. But it shouldn't ship as a tangled diff.

---

## Worktree boot prompt (template, copy into each new session)

```
You are working on issue [O5b/O5c/O6] in worktree ~/work/rs_cam_[issue].

Read planning/AGENTSEARCH_NEXT_SESSION.md (the parent plan). Your scope is the
"Worktree N — [issue]" section ONLY. Do not touch files outside that section's
"Files" list. Do not run MCP. Do not rebuild the GUI. Use cargo test for
iteration.

Steps:
1. cd into your worktree. Branch from master.
2. Write the failing repro test described in the section.
3. Run it: cargo test --test [test_name] -- --nocapture. Confirm it fails as
   described (otherwise the bug isn't reproduced and you need to update the
   test, not skip it).
4. Implement the fix following the section's "Likely fix direction".
5. Confirm the test passes. Run cargo test --workspace -q. Confirm no regressions.
6. Update planning/AGENTSEARCH_INVESTIGATION_LOG.md with what worked and what didn't.
7. Stop. Commit. Don't merge yet — wait for the other two worktrees to land.

If your fix needs to touch files outside the listed Files set, STOP and report
back rather than expanding scope.
```
