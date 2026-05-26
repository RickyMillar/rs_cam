# F-038 — Adaptive3d AgentSearch entry-plunge fragmentation

- **Stage:** sim / adaptive3d (G-code emission)
- **Severity:** medium (cycle-time, not correctness — real-machine surface
  quality is unchanged; the saved time was being spent on overhead, not
  cutting)
- **Status:** landed 2026-05-27
- **First found in:** Wanaka Back Rough wall-clock measurement on
  Shapeoko XXL, 2026-05-26 (`planning/feed_modulation_calibration/`
  bench session). Profiled in
  `planning/feed_modulation_calibration/gcode/f036c_wanaka_back_rough_modulation_off.nc`.
- **Effort:** M (one filter, three call sites, schema bump, threshold
  tuning)
- **Linked PRs:** this commit
- **Workstream:** F-036c cycle-time follow-up (the .nc that was used as
  the real-machine F-036c calibration reference exhibited the
  fragmentation; F-038 fixes the planner so future .nc emissions avoid
  it)
- **Resolution:** post-emission filter on the AgentSearch 2.5D slice
  dispatch, gated on a new `Adaptive3dParams::min_region_cut_length_mm`
  (default 15.0 mm). Acceptance test:
  `crates/rs_cam_core/tests/adaptive3d_entry_coalescing_f038.rs`.

## Symptom

Real-machine inspection of the Wanaka Back Rough .nc against the
.gcode the user actually ran on hardware (2026-05-26) flagged:

- 149 `G1 ... F750` Z-entry descents, grouped into **131 entry events**
  at 42 unique XY points
- 30 of the 149 plunges cut zero material before retract
- 90 of the 149 plunges cut ≤ 10 mm before retract
- 23 of 42 unique XY plunge points clustered on the entry-bbox edge
- Plunge density: ~0.71 plunges/cm² over the 140×150 mm stock

Mean cycle time penalty: each entry costs ~0.4 s of retract + rapid +
peck-plunge. The 131 entries accounted for ~50 s of the 827 s
wall-clock — but more importantly, the *modulated* run amplified each
entry (F-036c's feed bin is wider on transit moves than cutting moves)
which produced the 1224 s figure. F-038 is the planner-side fix; F-036c
is the modulator-side reasoning.

## Root cause

`clear_z_level_agent_2d_slice` invokes the 2D adaptive once per
marching-squares region. The 2D adaptive's segment output is shaped
as `[Rapid(entry), Cut, Cut, ..., Rapid(entry), ...]` — each `Rapid`
becomes a downstream retract + rapid XY + peck-plunge entry sequence.

On terrain-shaped models the 2D adaptive splits its sweep into many
small passes, a significant fraction of which cut < 5 mm before the
next entry. The per-entry overhead dominates. Additionally:

1. The downstream engagement subdivider (the "demote air runs to
   rapids" pass) can produce two back-to-back `Rapid` segments when
   the intervening `Cut` had only one engaged point and was therefore
   silently dropped. The downstream emitter generates a full
   retract+plunge cycle for both.
2. The marching-squares region itself is only ONE region per Z level
   on the Wanaka geometry (verified by debug log) — so the
   fragmentation isn't a "many regions" symptom; it's a "many passes
   per region" symptom inside the 2D adaptive's output.

## Fix shape — chosen: post-planner coalescing (option 1)

Three filters on the AgentSearch 2.5D slice dispatch, all gated on
`Adaptive3dParams::min_region_cut_length_mm` (default 15.0 mm; set to
0.0 to disable):

1. **Region-level forecast filter** — for each marching-squares
   region, dry-run the perimeter sweep + 2D adaptive output, sum the
   XY cut length. If below the threshold, skip the region entirely.
   (This is a no-op on Wanaka because the region count is 1 per Z
   level, but it generalises the fix to multi-region terrain.)
2. **Group-level filter inside the 2D adaptive output** — group the
   2D adaptive segments by `Rapid` boundary, drop interior groups
   whose total `Cut` XY length is below the threshold.
3. **Post-emission coalescing** — walk the emitted `Adaptive3dSegment`
   list per Z-level and collapse `[Rapid, (Marker)*, Rapid, ...]`
   sequences to `[Rapid, ...]`, removing the redundant first entry.

Rejected option 2 (keep-tool-down link between boundary-adjacent
micro-passes via the dressup `link_max_distance`) — more principled
but requires substantial reshaping of the dressup pass; deferred as a
future F-038b. Option 1's post-planner filter wins on terrain in the
short term: micro-regions that emit < 15 mm of cut are mostly wasted
air, and the finishing pass cleans the residual.

## Acceptance test

`crates/rs_cam_core/tests/adaptive3d_entry_coalescing_f038.rs` —
synthetic 60×60 mm terrain with a central pad + 8 perimeter
micro-peaks (designed to reproduce the Wanaka fragmentation pattern).
Asserts:

- post-fix entry-plunge count is < 75 % of pre-fix on the fixture
- at least `PERIMETER_BUMPS` (8) entries get removed in absolute terms
- post-fix still emits at least one entry (catches over-correction)

Measured on the fixture: 775 → 280 entries (64 % reduction).

## Files

- `crates/rs_cam_core/src/compute/operation_configs.rs` —
  `Adaptive3dConfig::min_region_cut_length_mm` (serde default 15.0)
- `crates/rs_cam_core/src/adaptive3d/mod.rs` —
  `Adaptive3dParams::min_region_cut_length_mm` + new
  `ZLevelPlanMetrics::dropped_short_region_count` field
- `crates/rs_cam_core/src/adaptive3d/clearing.rs` — three filters
  inside `clear_z_level_agent_2d_slice` + new
  `coalesce_redundant_entries` + `polyline_xy_length` helpers
- `crates/rs_cam_core/src/adaptive3d/path.rs` —
  `ClearZLevelContext::min_region_cut_length_mm` plumbing
- `crates/rs_cam_core/src/compute/execute.rs` — forward field from
  `Adaptive3dConfig` into `Adaptive3dParams`
- `crates/rs_cam_core/src/compute/catalog.rs` — schema row in
  `ADAPTIVE3D` `ParamDef` list
- `crates/rs_cam_cli/src/job.rs` + `main.rs` — CLI plumbing
- `crates/rs_cam_core/tests/adaptive3d_entry_coalescing_f038.rs` —
  new acceptance test
- `crates/rs_cam_core/tests/feed_modulation_cycle_time_f036c.rs` —
  envelope widened from 1.7 to 2.0 with NEEDS-RE-MEASURE marker
- `crates/rs_cam_core/tests/machine_kinematics_cycle_time_f034.rs` —
  tolerance widened from ±15 % to [0.55, 1.25] with same marker

## Risk

- The 15 mm default threshold leaves micro-islands < 15 mm wide
  un-roughed. Finishing passes (scallop / waterline) handle these in
  the Wanaka flow. If a user roughs *without* a finishing pass, sub-
  15 mm features may show through. Mitigation: the threshold is a
  config knob; set to 0.0 to restore pre-fix behaviour.
- F-036c (real-machine modulated cycle ratio) and F-034 (cycle-time
  calibration) both reference a pre-F-038 wall-clock (827 s
  unmodulated, 1224 s modulated). Post-F-038 the model predicts 590 s
  unmodulated. These tests are now widened; tightening requires a
  re-bench on real hardware. Documented in test headers.

## Measured impact (Wanaka Back Rough)

Pre-fix (commit `a797909`):

| Metric | Value |
|---|---|
| F750 Z-entry descents | 149 |
| Entry events (peck-grouped) | 131 |
| Unique XY plunge points | 42 |
| Perimeter-clustered XY | 24 (57 %) |
| Zero-cut entry events | 12 (peck-grouped) / 30 (raw) |
| ≤ 10 mm cut entry events | 72 (peck-grouped) / 90 (raw) |
| Cycle time (model) | 789 s |

Post-fix (this commit, default threshold 15 mm):

| Metric | Value | Δ |
|---|---|---|
| F750 Z-entry descents | 77 | −48 % |
| Entry events | 59 | **−55 %** |
| Unique XY plunge points | 27 | −36 % |
| Perimeter-clustered XY | 9 (33 %) | −63 % |
| Zero-cut entry events | 0 | −100 % |
| ≤ 10 mm cut entry events | 5 | −93 % |
| Cycle time (model) | 576 s | **−27 %** |

The 60 % target stated in the original brief was a rough estimate; the
measured reduction on the primary entry-events metric is 55 %, with
much larger reductions on the secondary "useless cut" metrics
(zero-cut, ≤ 10 mm cut).

## Notes

- F-031 had previously fixed the **entry style** for adaptive3d
  (dressup helix override → planner-emitted plunge). F-038 is
  orthogonal: it doesn't change the entry style, only the count of
  entries emitted at planning time.
- The post-emission coalescer can fire when the engagement subdivider
  produces back-to-back rapids; this is a real artifact of the
  air-run / engaged-run flushing logic at lines ~1820–1880. A more
  principled refactor would unify the engagement subdivider with the
  group-level filter and remove the need for the coalescer entirely.
  Tracked as future work.
- Calibration .nc files in `planning/feed_modulation_calibration/gcode/`
  intentionally **not** regenerated as part of this PR — they are
  anchored to the 2026-05-26 measurement. The next real-machine bench
  (per `BENCH_CHECKLIST.md`) should regenerate them against the new
  default threshold.
