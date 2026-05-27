# F-038b — Adaptive3d keep-tool-down link between cut groups

- **Stage:** simulator → adaptive3d planner emission
- **Severity:** medium (cycle-time-only; safe by construction)
- **Status:** landed
- **First found in:** user feedback after F-038 bench, 2026-05-26
- **Effort:** L (algorithm + per-link mesh probe + safety guards + TSP retract invariant fix)
- **Workstream:** adaptive3d quality (parallel to Feed Modulation)
- **Depends on:** F-038 landed

## Symptom

Pre-F-038b adaptive3d emits a full retract → safe-Z traverse → re-plunge
between every cut group, even when the adjacent groups sit on a flat
shelf and the heightfield between them is well below the cut depth. On
the wanaka Back Rough this costs ~3 minutes per run beyond what a
production CAM (Fusion HSM "stay down distance", Mastercam Dynamic Mill)
would emit.

## Hypothesised root cause

The planner emits `Adaptive3dSegment::Rapid(entry)` between every cut
group. `segments_to_toolpath` translates each one into the conservative
retract-rapid-plunge sequence (legacy path). There is no inspection of
the mesh heightfield along the candidate link line — every transition
gets the worst-case retract.

## Fix shape

### Part A — heightfield-sampled link planner (`crates/rs_cam_core/src/adaptive3d/path.rs`)

`try_emit_stay_down_link` runs before the legacy retract for every
`Adaptive3dSegment::Rapid` / `RapidWithFloor`, gated on
`EntryStyle3d::Plunge` (Helix / Ramp already have their own entry
geometry). Algorithm:

1. Reject if `xy_distance(from, to) > max_stay_down_distance_mm`.
2. Sample mesh heightfield at 10 evenly-spaced stations along the XY
   straight line (`max_mesh_z_along_line` — drops the operative cutter
   at each station via `dropcutter::point_drop_cutter`).
3. `link_z = max(samples_max, from.z, to.z) + stay_down_clearance_mm`.
4. Reject if `link_z > safe_z + 1e-9` (terrain peak above safe-Z guard).
5. Reject if `link_z > to.z + cutter.length() + 1e-9` (shank guard).
6. Emit three `MoveIntent::Linking` feed moves at `feed_rate`:
   (a) ascend to `link_z` at the current XY, (b) traverse at `link_z`,
   (c) descend to the entry point.

Rejection at any step falls through to the legacy retract.

### Part B — new operator knobs

`Adaptive3dParams` + `Adaptive3dConfig` gained two fields:

- `max_stay_down_distance_mm: Option<f64>` — None = planner default of
  8 × tool diameter (Fusion HSM roughing default for hardwood).
  `Some(0.0)` disables stay-down; `Some(x)` caps at `x` mm.
- `stay_down_clearance_mm: f64` — vertical clearance above the
  heightfield sample max. Default 0.5 mm (matches dexel cell-height
  noise floor at 0.5 mm sim resolution).

Plumbed through `compute/operation_configs.rs` → `compute/execute.rs` →
`compute/catalog.rs` (op-schema entries) → `rs_cam_cli/src/{job,main}.rs`.

### Part C — TSP retract invariant (`crates/rs_cam_core/src/tsp.rs`)

**Regression discovered during validation:** AS013 smoke case gained 2
rapid collisions when stay-down was on. Root cause was an existing
fragility in `tsp::rebuild_group` exposed by F-038b. When stay-down
eliminates all internal rapids within a depth-pass, that pass becomes a
single segment → `optimize_one_group` takes the `segments.len() == 1`
fast path → appends moves verbatim with no trailing retract → cutter
left at low Z. The next group's `rebuild_group` leading rapid (line 361)
then emits a single diagonal rapid `(low Z) → (next_seg.start.xy, safe_z)`
that slices through stock.

Fix in `rebuild_group` `idx == 0` branch: emit a vertical retract first
when `result.moves.last().target.z < safe_z`, mirroring the (idx > 0)
two-rapid pattern. Pure additive — only fires when the cutter isn't
already at safe_z, so existing well-shaped toolpaths see no change.

## Acceptance tests

`crates/rs_cam_core/tests/adaptive3d_keep_down_link_f038b.rs`, 4 tests:

1. **Stay-down emitted when terrain permits** — flat-top two-peak mesh
   with `max_stay_down_distance_mm = 50`. Asserts no `EntryPlunge` moves
   emitted between the two regions; only `Linking` feeds at link Z.
2. **Peak between regions forces retract** — same fixture with a peak
   inserted in the gap that rises above the cut Z. Planner must reject
   stay-down and fall back to retract.
3. **Default knob caps stay-down at 8 × tool diameter** — `None` knob
   value; regions separated by > 8 × diameter must not use stay-down.
4. **Tool cutting-length safety guard** — short `tool_cutting_length`
   forces `link_z > entry.z + cutting_length` → stay-down rejected.

Plus 9 existing tests updated to populate the new struct fields. Plus
F-037 smoke baseline diff clean (the AS013 regression was caught here
during initial validation and traced to Part C).

## Real-machine performance (wanaka full project, default 8 × diameter knob)

Comparison generated via `rs_cam_cli project … --emit-gcode`, resolution
1.0 mm, against the production wanaka:

| Metric | Stay-down OFF | Stay-down ON | Delta |
|---|---|---|---|
| Back Rough rapid_mm | 6468 | 3338 | **−48%** |
| 3D Rough 6 rapid_mm | 2336 | 943 | **−60%** |
| Total project rapid_mm | 28608 | 25907 | **−2701 mm (−9.4%)** |
| Predicted runtime | 3218 s | 3044 s | **−174 s (≈3 min, −5.4%)** |
| Total cutting_mm | 63842 | 63181 | −661 (linking feeds reclassified) |

Bench verification deferred until F-039 lands — the user re-bench trip
will validate F-038b + F-039 together to amortize the setup cost.

## Files

- `crates/rs_cam_core/src/adaptive3d/path.rs` — algorithm (+283 lines net)
- `crates/rs_cam_core/src/adaptive3d/mod.rs` — `Adaptive3dParams` fields
- `crates/rs_cam_core/src/compute/operation_configs.rs` — config plumbing
- `crates/rs_cam_core/src/compute/execute.rs` — param routing
- `crates/rs_cam_core/src/compute/catalog.rs` — op-schema entries
- `crates/rs_cam_cli/src/{job,main}.rs` — CLI flag exposure
- `crates/rs_cam_core/src/tsp.rs` — `rebuild_group` retract invariant fix
- `crates/rs_cam_core/tests/adaptive3d_keep_down_link_f038b.rs` — 4 acceptance tests
- 9 existing tests updated for new struct fields

## Risk

L. Three safety guards (distance cap, safe-Z cap, cutter-length cap) all
fall back to legacy retract on any boundary violation. Default
8 × diameter matches the production-CAM convention. Smoke baseline diff
is clean across all 18 cases (caught and fixed the TSP regression
before commit). User can disable via `max_stay_down_distance_mm = 0.0`
per-op in the project TOML if any specific job needs the conservative
behaviour back.

## Notes

- The F-038b agent landed implementation and tests, hit its session
  limit before commit. Validation + TSP fix + finding doc + commit
  performed in the main session 2026-05-27.
- The TSP invariant fix (Part C) is a latent bug F-038b exposed — it
  was always wrong to assume a depth-pass's framing rapids were enough
  to leave the cutter at safe_z. Now `rebuild_group` is invariant under
  any segment composition.
- F-039 modulator should treat `MoveIntent::Linking` (stay-down's tag)
  the same as Linear feeds it can modulate — the linking feed-rate is
  the operation's `feed_rate`, and modulation can still raise/lower it
  within the chipload band like any other feed.
