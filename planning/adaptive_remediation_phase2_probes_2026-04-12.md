# Adaptive Remediation — Phase 2 Empirical Probes

**Date:** 2026-04-12 (fresh session after MCP server rebuild)
**Context:** Post-remediation empirical validation of Packages M, N, F, E, L from
the Phase 1 remediation series (`master~17..master` as of commit `0e17dc3`).

**Methodology:** Reload the MCP-connected rs_cam_gui, reproduce the original
Fixture 1 and Fixture 2 workflows from `planning/adaptive_review_2026-04.md`,
and measure the same metrics the review flagged.

---

## Probe 1 — F-2: 2D pocket simulation engagement

**Target finding:** F-2 (HIGH) — simulator reported `average_engagement = 0.0`
and `total_removed_volume_est = 0.0` for 2D SVG pocket operations despite
visible stock removal.

**Target fix:** Packages M + N (`StockConfig::update_from_bbox` handles
zero-height bboxes, `ProjectSession::add_model` triggers auto-resize).

**Setup:**
```
load_project sample_adaptive_debug_project.toml
import_model square.svg                          # triggers Package M+N path
add_toolpath setup_index=0 type=pocket tool=0 model_id=2
generate_toolpath 0
run_simulation
```

**Stock auto-size confirmation (before pocket):**

| field | pre-M/N | post-M/N | meaning |
|---|---|---|---|
| dimensions | 100×100×25 | **40×40×25** | XY right-sized to polygon + 2×padding |
| origin.x, origin.y | 0, 0 | 0, 0 | polygon at (5,5); padding=5 → origin at 0 |
| **origin.z** | **0** | **-25** | **Package N: 2D stock top now at Z=0** |

**Simulation result:**

| metric | pre-review baseline | post-remediation | verdict |
|---|---:|---:|---|
| `average_engagement` | **0.0** | **0.00588** | ✅ **was zero, now non-zero** |
| `total_removed_volume_est_mm3` | 0.0 | **20,612.5** | ✅ non-zero |
| `average_mrr_mm3_s` | 0.0 | 361.93 | ✅ non-zero |
| `peak_chipload_mm_per_tooth` | 0.0 | 0.0278 | ✅ non-zero |
| `air_cut_percentage` | 100% | 86.35% | ✅ −13.65% |
| `issue_count` | **1,638** | **14** | ✅ **117× reduction** (Package L segment coalescence) |
| `rapid_collision_count` | 24 | 11 | ✅ −54% |

**Cut trace detail (first 2 issues of 14):**

Issue 1 (plunge segment):
```
kind: air_cut
position:    [8.175, 31.825,  9.75]  ← start (z=9.75)
end_position [8.175, 31.825, -1.25]  ← end (z=-1.25)
sample_count: 23    duration_s: 1.32
```
One segment for the full 11mm plunge from safe_z through the stock top down
to the first cut depth. Under the old per-sample emission this was 23
separate `air_cut` issues.

Issue 2 (first cutting pass):
```
kind: low_engagement
position:    [8.42, 31.83, -1.5]
end_position [8.18, 25.67, -1.5]
sample_count: 180   duration_s: 5.29
radial_engagement:  0.063  (start)
min_radial_engagement: 0.024
```
**First time the simulator has ever reported non-zero engagement for the
pocket baseline.** The 2.4–6.3% reading is below the `LowEngagement`
threshold of 10% but exactly consistent with F-3's cylinder-vs-leading-edge
calibration caveat — the simulator measures cylinder-volume engagement
which runs ~10× lower than the algorithmic target.

**Verdict: F-2 CLOSED empirically. Packages M+N+L all confirmed working
on the real fixture.**

Also, the `0e17dc3` integration test (`two_d_pocket_simulation_reports_engagement`)
is the regression guard — it asserts non-zero engagement via the same
run_simulation path, so this win won't regress silently.

---

## Probe 2 — F-5: z_blend=true rapid collision count

**Target finding:** F-5 (HIGH) — on the terrain fixture, z_blend=true
triples `rapid_collision_count` from 232 to 569.

**Target fix:** Package F (commit `0958284`) — added `is_clear_path_3d`
gate to the ring-to-ring link decision in both
`clear_z_level_contour_parallel` (clearing.rs:500) and
`clear_z_level_adaptive` (clearing.rs:749).

**Setup:**
```
import_model fixtures/terrain_small.stl     # auto-size stock via Package M
                                             #   → 110×83.3×57.57, origin (−5,−5,0)
add_toolpath setup_index=0 type=adaptive3d tool=0 model=3
set_toolpath_param stock_top_z=57.57
set_toolpath_param z_blend=true
generate_toolpath + run_simulation
# then flip z_blend=false and re-run for comparison
```

**Results:**

| variant | moves | cut (mm) | rapid (mm) | rapid_frac | **collisions** |
|---|---:|---:|---:|---:|---:|
| z_blend=false (pre-review) | 30,441 | 65,129 | 9,818 | 0.131 | **232** |
| z_blend=false (post-remediation) | **30,319** | **62,244** | **10,093** | **0.139** | **262 (+13%)** |
| z_blend=true (pre-review) | 20,511 | 42,528 | 24,263 | 0.363 | **569** |
| z_blend=true (post-remediation) | **19,760** | **40,452** | **23,231** | **0.365** | **556 (−2.3%)** |

**Predicted delta:** z_blend=true collisions drop from 569 toward the
~232 z_blend=false baseline (the gap closes).

**Actual delta:** z_blend=true collisions dropped 13 (569→556). z_blend=false
**regressed** 30 (232→262). The gap only closed from 337 to 294 — a 13%
narrowing instead of the ~100% closure I predicted.

**Interpretation:** The `is_clear_path_3d` gate IS rejecting some previously-
unsafe Links, but the code path it falls into — `Adaptive3dSegment::Rapid`
emission — is **itself producing rapid collisions**. The regression in
z_blend=false (+30 collisions) is the smoking gun: before Package F, those
30 collisions didn't exist because the problematic moves were Links, not
Rapids. After Package F, the gate converted them to Rapids, which the
simulator's `check_rapid_collisions_against_stock` path then flagged.

**Root cause (revised hypothesis):** the Rapid emission in
`adaptive3d::path::segments_to_toolpath` may not be retracting to `safe_z`
before the XY move. Or the retract height is computed from a stale
per-region origin, not the actual stock top. Either way, rapids that
should be above stock are instead traveling through stock at some lower Z.

**Verdict: F-5 PARTIALLY fixed. The collision count gap closed 13% but the
bulk of the z_blend regression is still present. Package F's fix was the
correct direction but insufficient.** Needs follow-up — specifically, an
investigation into how `Adaptive3dSegment::Rapid` becomes Toolpath rapid
moves, and whether the safe_z lift is present and correct.

---

## Probe 3 — F-6: stepover=3 rapid collision spike

**Target finding:** F-6 (MEDIUM) — on the terrain fixture, stepover=3
produces 75% more rapid collisions than stepover=2 (232→405).

**Target fix:** Package E (commit `0caeba9`) scaled `max_link_dist` to
honor stepover, plus Package F's is_clear_path_3d gate.

**Setup:**
```
(continuing from Probe 2, z_blend=false)
set_toolpath_param stepover=3
generate_toolpath + run_simulation
```

**Results:**

| variant | moves | cut (mm) | rapid (mm) | **collisions** |
|---|---:|---:|---:|---:|
| stepover=2 (pre-review, z_blend=F) | 30,441 | 65,129 | 9,818 | 232 |
| stepover=3 (pre-review, z_blend=F) | 20,327 | 46,145 | 12,000 | **405** |
| stepover=3 (post-remediation, z_blend=F) | **20,246** | **45,737** | **15,055** | **499 (+23%)** |

**Predicted delta:** stepover=3 collisions drop from 405 toward 232
(toward the stepover=2 baseline) because of Package F's gate.

**Actual delta:** stepover=3 collisions **went up** from 405 to 499 (+23%),
and rapid distance went up from 12,000 to 15,055 (+25%).

**Interpretation:** Same mechanism as F-5. The `is_clear_path_3d` gate is
rejecting more Links at stepover=3 (larger inter-pass gaps → more
candidate links fail the clearance check), converting them to Rapids,
which inflates both `rapid_distance_mm` and `rapid_collision_count`. The
absolute collision count is WORSE than pre-remediation.

Note: Package E's `max_link_dist = max(tool_radius*6, stepover*6)` formula
doesn't trigger for the 6.35mm tool at stepover=3 (`max(19.05, 18) = 19.05`,
unchanged from pre-Package-E), so E contributes nothing to this
measurement — only F's gate is active.

**Verdict: F-6 REGRESSED. The empirical data contradicts the commit
message prediction. Package F alone made the stepover=3 case worse, not
better. F-6 is NOT closed and Package E alone wouldn't close it either
at the tested condition.**

---

## Summary & next steps

| finding | prediction | reality | status |
|---|---|---|---|
| **F-2** | engagement 0 → non-zero | **0 → 0.00588** | ✅ **CLOSED** |
| **F-5** | collisions 569 → ~232 | 569 → 556 (−2.3%) | 🟡 partially improved |
| **F-6** | collisions 405 → ~232 | 405 → **499 (+23%)** | ❌ **REGRESSED** |
| F-15 (Package L) | 141K issues → dozens | 1,638 → 14 on the 2D fixture | ✅ **CLOSED** |

**Unexpected finding:** converting problematic Links to Rapids via the
`is_clear_path_3d` gate **moved the collision from the feed-move metric
to the rapid-move metric**. The underlying bug is further downstream —
specifically in how `Adaptive3dSegment::Rapid` becomes a toolpath Rapid
move. The safe_z lift is either missing, using a wrong reference, or
being bypassed for some subset of rapids.

**Concrete next investigation (Phase 3 candidate):**

1. **Trace `segments_to_toolpath` in `adaptive3d/path.rs`** for the
   `Adaptive3dSegment::Rapid(P3)` variant. How does it become
   `MoveType::Rapid` in the output toolpath? Is there a lift to
   `params.safe_z` before the XY move? Is `safe_z` being passed through
   correctly?

2. **Inspect `check_rapid_collisions_against_stock`** in
   `compute/simulate.rs`. What exactly does it count as a "rapid
   collision"? A rapid that stays above stock should register 0
   collisions. A rapid that descends through stock registers some. Which
   rapid lines are triggering the count on our fixture?

3. **Possible quick fix:** Before Package F lands in production the user
   should either (a) revert the `is_clear_path_3d` gate, or (b) pair the
   gate with a safe_z-lift fix for the Rapid emission path. Landing F
   alone trades Link-path collisions for Rapid-path collisions with no
   net improvement.

**Revert candidate?** Package F (commit `0958284`) is a net regression
on the empirical metrics. Consider reverting it pending a proper
safe_z-aware fix, OR land the safe_z fix before declaring F-5/F-6 done.

**Other packages are fine:**
- A, K, H, J, I, D: pure doc/fixture/log/warning changes — no behavior impact
- B, C, G: AgentSearch exposure + CLI flags + test default — no metric regressions
- E: max_link_dist scaling — no-op at the tested conditions (confirmed)
- L: issue coalescence — working as designed (117× reduction confirmed)
- M, N: stock auto-size + 2D Z frame — **confirmed working** (this is the F-2 win)

---

## Raw MCP probe transcript

See `/tmp/adaptive_review_notebook.md` for the original Phase 1 baselines
used for comparison. This document's metrics were captured from a live
MCP session on 2026-04-12 using the following probe sequence:

```
# Probe 1 (F-2)
mcp: load_project sample_adaptive_debug_project.toml
mcp: import_model square.svg
mcp: inspect_stock  → confirm origin.z = -25 (Package N)
mcp: add_toolpath type=pocket
mcp: generate_toolpath 0
mcp: run_simulation
mcp: get_cut_trace toolpath_id=0 max_issues=20

# Probe 2 (F-5)
mcp: remove_toolpath 0
mcp: import_model fixtures/terrain_small.stl
mcp: inspect_stock  → confirm 110×83.3×57.57, origin (-5,-5,0) (Package M+N 3D branch)
mcp: add_toolpath type=adaptive3d model_id=3
mcp: set_toolpath_param stock_top_z=57.57
mcp: set_toolpath_param z_blend=true
mcp: generate_toolpath 0 + run_simulation   → 556 collisions
mcp: set_toolpath_param z_blend=false
mcp: generate_toolpath 0 + run_simulation   → 262 collisions

# Probe 3 (F-6)
mcp: set_toolpath_param stepover=3
mcp: generate_toolpath 0 + run_simulation   → 499 collisions
mcp: get_cut_trace toolpath_id=1 max_hotspots=3 max_issues=5
```
