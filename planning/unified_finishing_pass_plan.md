# Unified finishing pass — plan + tracking

Owner: Ricky + Fable. Started 2026-07-07 (same-day context: pencil ridge fix 29a6d61,
F.4 catch-22 fix + rest-UX 37ef782, sliver guard d0d6d75, MCP cancel 482be35).

## Goal (user's words, 2026-07-07)

"The goal is to cleanly remove shallow areas as fast as possible to a finish. I think
for steeps we will want to use contour still, but maybe look into similar methods for
linking, and build out some kind of steep/shallow/pencil method where we take the
shortest possible path, instead of doing them in phases."

Two threads, deliberately sequenced:

1. **The big lever: phase-merged, shortest-path finishing.** Today steep/shallow/crease
   finishing runs as separate ops — each traverses the part with its own retracts and
   its own boundary-clip fragmentation. All measured wins in this project were
   linking/ordering wins, not cutting wins.
2. **The quality increment: morphed spiral per shallow region.** One continuous path
   morphing boundary→center per island ("one clean path, no moves"). Commercial
   analogue: Fusion Morphed Spiral / PowerMill 3D-offset spiral. Stepover must be
   measured on the TOOL-CONTACT surface (or slope-corrected), not plan XY — planar
   spacing under-covers slopes by 1/cos(slope). This is a strategy PLUGIN for #1,
   not its own op, and it is built LAST.

## Evidence base (why linking is the suspected lever)

| Datum | Number | Source |
|---|---|---|
| Pencil R2 (machined stock) rapid vs cutting | 1027mm rapid vs 669mm cutting | 2026-07-07 live |
| Pencil hookup_distance surface-link fix | total motion −72% | 2026-06-25 (bf4e95d) |
| Contour-spiral trochoid cap vs adaptive agent | wall-clock −32%, rapid ÷3.6 | 2026-06-15 |
| 3D Finish boundary-clip fragmentation | 133,585mm rapid vs 3,795mm default-linked | 2026-07-04 probe |
| steep_shallow op A/B | 3.4× MORE total motion (WORSE) | 2026-07-04 probe |

The steep_shallow failure autopsy: it died of region fragmentation + per-fragment
retracts, NOT of the slope-classification idea. Lesson: never emit regions without
solving the linking. The planner must treat routing as first-class.

## Phases

### P0 — PROBE: linking share of finishing wall-clock (decision gate) `[ ]`

Measure the ADDRESSABLE overhead before building anything. On the current wanaka
finishing stack (coarse scallop + selective fine + pencil, and/or the rebuilt
Rivers→Finish chain), use the F-034 kinematics-aware integrator (machine profile is
real: $$-imported accels, junction deviation) to decompose wall-clock into:
cutting-in-material / air-cut feed (links) / retract+plunge / rapid travel.

- Available signals: `cut_trace.summary` (`cutting_runtime_s`, `rapid_runtime_s`,
  `air_cut_time_s`), per-toolpath `total_runtime_s` (integrator-backed when
  kinematics context is on), `MoveIntent` tags on moves (Retract / Linking /
  EntryPlunge / FinishingCut — tagged since the Step-1 2026-05-19 work).
- If per-intent TIME isn't already exposed, a small core addition (aggregate
  integrator time by MoveIntent class per toolpath) is in-scope for the probe.
- **Decision gate**: linking+retract+air share ≥ ~15–20% of finish wall-clock →
  proceed to P1/P2. Below that → deprioritize the planner; morphed spiral proceeds
  as a quality feature only (P3 direct).
- Log results here + probe log discipline (artifacts, params, numbers).

### P1 — Quantitative linker (benefits everything immediately) `[ ]`

Replace fixed-threshold link heuristics (`hookup_distance`, boundary-clip
retract-always, scallop connector hop cap) with a per-gap COST COMPARISON using
`compute_cycle_time`: time(surface-following feed across gap, gouge-checked) vs
time(retract + rapid + re-plunge). Pick the cheaper. Sites:

- pencil `emit_paths` (currently fixed hookup_distance)
- scallop continuous-mode ring/island connectors (currently hop ≤ 3R cap)
- `clip_toolpath_to_boundary*` re-entries (the 133k-rapid case)
- future planner region-to-region links

Acceptance: integrator wall-clock A/B per op on wanaka; no new sim collisions;
gouge-safety of surface links unchanged (drop-cutter sampled, fall back to retract).

### P2 — Finishing pass planner (phase merge) `[ ]`

One op (or orchestrated pass) that:

1. **Decomposes** the surface: steep regions (slope threshold — classifier exists in
   steep_shallow), shallow regions (complement), crease network (= the pencil ridge
   detector's centerlines, 29a6d61). All RegionSet-shaped; sliver guard d0d6d75
   applies.
2. **Assigns strategy** per region: steep → contour/waterline rings; shallow →
   raster or rings (morphed spiral later, P3); creases → pencil centerline trace.
3. **Routes globally**: order all region entry/exit points for shortest total time
   (greedy nearest-neighbour first; TSP polish only if the probe says it pays),
   with P1's quantitative link decision at every junction.

Constraints/reuse: RegionSet, per-island generation (P2 selective finishing),
existing waterline/scallop/pencil generators as region-scoped strategies; the
steep_shallow op's fragmentation failure is the anti-pattern this must not repeat.
Fresh design doc + Fable review before implementation (this is architecture).

### P3 — Morphed spiral strategy `[ ]`

Per shallow region: single continuous spiral morphing between the region boundary
and its innermost offset ring.

- Spacing measured on the tool-contact surface (slope-corrected), constant-scallop.
- Non-convex regions: split at medial-axis necks first, or per-region fallback to
  ring mode when a morph-degeneracy metric trips (boundary/center shape mismatch).
- Lives as a strategy in P2's planner (or a scallop mode if P2 is deferred), NOT a
  new top-level op.
- Spiral direction: outside-in ends at region center (needs retract) vs inside-out
  ends at boundary (can surface-link to the next region) — inside-out preferred for
  routing; verify chip/finish implications.

## Acceptance discipline (project-wide)

- Wall-clock = F-034 kinematics integrator, never move counts or path mm.
- A/B on wanaka: current phase-based stack vs unified pass, same tools + thresholds.
- Quality: sim deviation vs model, scallop-height spot checks, 0 new rapid
  collisions, sentries green.

## Tracking

- [ ] P0 probe run + numbers logged below
- [ ] P0 decision recorded (proceed / spiral-only)
- [ ] P1 quantitative linker (pencil, scallop, boundary-clip) + A/B
- [ ] P2 design doc (decomposition/strategy/routing interfaces)
- [ ] P2 implementation + A/B vs phase-based stack
- [ ] P3 morphed spiral strategy + degeneracy fallback
- [ ] Ledger + FEATURE_CATALOG + memory updates at each landing

## P0 results log

(empty — fill during the probe)
