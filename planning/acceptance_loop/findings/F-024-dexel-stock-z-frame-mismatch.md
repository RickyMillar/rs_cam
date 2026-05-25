# F-024 — Z-frame mismatch in dexel stock grid for identity setups

- **Stage:** substrate
- **Severity:** high
- **Status:** landed (3 fix sites: core `d82bd4d`, viz-worker `0c907a6`, viz-controller 2026-05-25)
- **Linked PRs:** commits `d82bd4d` + `0c907a6` + viz-controller third-site (2026-05-25)
- **First found in:** round-03 (2026-05-25), via F-002 implementer investigation
- **Effort:** M–L (~200–400 LOC across `compute/simulate.rs`, `compute/transform.rs`, `session/compute.rs` + fingerprint regen)
- **Linked PRs:** —
- **Source audits:** F-002 implementer's reproduction probe, 2026-05-25

## Evidence

This finding supersedes the round-02 reopen of F-002. The reopen
framing (linear-kinematics emission path stale relative to fixed
engagement-vector path) was wrong: F-002's original split landed
correctly in commit `072c11a`. The remaining deflection-gate
over-fire (round-02 AS001/AS002/AS013/AS015 all reading 374/408/573/431 µm)
has a different root cause, isolated below.

### What the F-002 implementer reproduced

Probe based on `tests/pocket_lift_bridge_b1.rs`, replicating AS001
(6mm flat endmill, hardwood stock, `origin_z=-12`, `depth=6`,
`depth_per_pass=2`):

```
peak linear sample: pos=(16.88, 15.20, -2.000) move=36 aem=12.0000  intent=ClearingCut
ray_before=[(0.0, 10.5)]  cell_tool_surface=-2.0000  → all 10.5mm removed
per_kinematics.Linear.peak_axial_doc_mm = 12.0000
per_kinematics.Helix.peak_axial_doc_mm  = 0.0   (transit-filtered; per-sample still 12.0)
per_kinematics.Arc.peak_axial_doc_mm    = 0.0   (transit-filtered; per-sample still 12.0)
```

Conclusion: **all four kinematics classes** (linear/helix/arc/plunge)
emit per-sample `axial_engagement_mm = 12.0` on a 2mm-DOC pocket
pass. The summary asymmetry (linear vs arc/helix in round-02 smoke)
is an artefact of `KinematicsAccumulator::observe`'s
`if !sample.in_transit_span` filter — arc/helix samples during pocket
entry are tagged transit; linear clearing cuts are not. Not a code
asymmetry between classes.

### Root cause (per implementer's pinpoint analysis)

A Z-frame mismatch in the dexel stock grid:

1. `effective_stock_bbox` (`compute/transform.rs:293`) builds
   `local_stock_bbox = (0, 0, 0)..(stock_x, stock_y, stock_z)`,
   always rooted at local origin `(0, 0, 0)`.

2. For an identity setup (`face_up=Top`, `z_rotation=Deg0`),
   `needs_transform()` returns `false` → `local_to_global = None` is
   set at `session/compute.rs:1140-1146`.

3. With `local_to_global = None`, **no transform is applied to the
   toolpath before stamping** in `compute/simulate.rs:386`. But the
   toolpath generator emits Z in a frame where Z=0 is stock top and
   negative goes deeper (e.g. pocket pass at `cut_depth: -2`, confirmed
   at `pocket.rs:171`).

4. So the cutter at Z=-2 is stamped into a dexel grid whose rays span
   Z=[0, 12]. Inside `stamp_segment_with_metrics`, the cell-surface
   for the cutter footprint resolves to -2 (below the bottom of the
   entire ray segment `(0, 10.5)`), so `ray_blend_above(ray, -2, 1.0)`
   clears the entire ray. `removed_here` = full ray length (10-12mm),
   which becomes the sample's `axial_engagement_mm`.

5. The deflection gate consumes the per-sample peak and reports
   ~374-573 µm tip deflection because it's reading "the full stock
   height was just engaged" rather than the commanded 2mm DOC.

### Why F-002's prescribed fix doesn't apply

- The linear-kinematics emission path is **NOT different** from arc/
  helix/plunge — they all flow through `record_cutting_subsegment`
  in `dexel_stock/simulation.rs:486`, which already does the
  axial-engagement vs plunge-descent split correctly per the F-002
  original premise.
- "Mirror the engagement-vector path for linear" has no surface to
  apply — there is nothing different to mirror.
- A minimal "clamp `removed_here`" workaround would mask the frame
  mismatch and silently break other sim outputs (rapid-collision Z
  math, deviation calculations, mesh extraction).

## Where the fix lives

Three candidate sites, one of which (or some combination) is the
load-bearing change:

1. **`compute/simulate.rs:386`** — build `group_stock` in the same
   frame as the toolpath. Likely re-root the bbox to match
   `request.stock_bbox` (i.e. apply `stock_origin_z` to the bbox so it
   spans the same Z range as the toolpath sees).

2. **`session/compute.rs:1140-1146`** — always populate
   `local_to_global` for non-zero `stock_origin_z`, not just for
   non-identity `face_up`/`z_rotation`. Treat origin offsets as
   transforms.

3. **`compute/transform.rs:293`** — `effective_stock_bbox` returns
   world coords when no rotation/flip is in play. Currently it
   unconditionally roots at `(0,0,0)`.

Pick the cleanest of the three. (2) is conceptually right —
`local_to_global` should mean "the transform from local to global",
and a non-zero origin IS a transform. (1) is the smallest surgical
fix. (3) widens the semantic of "effective bbox".

## Acceptance test

1. **Unit test (sim layer)**: build a pocket toolpath via
   `ProjectSession` (identity setup, `stock_origin_z = -12`,
   `depth = 6`, `depth_per_pass = 2`, hardwood, 6mm endmill —
   AS001 shape). Read the per-sample `axial_engagement_mm` for any
   linear/arc/helix cutting sample on the first pass. Assert it's
   <= 3.0 (allow some margin above the commanded 2.0 for grid
   discretisation, but well below 12).

2. **Integration test (deflection gate)**: same AS001 pocket setup,
   run the full sim through tool-load. Assert
   `deflection.peak_mm < 0.2` (i.e. moves from Exceeds into the
   < 200µm Within band). This is the smoke-acceptance equivalent
   inlined as a unit test.

3. **Smoke verification** (auditor's next round):
   - AS001 / AS002 / AS013 / AS015 deflection `peak_mm` drops out of
     Exceeds. AS001 should land near the validated band (<50µm).
   - AS013 adaptive3d: same.
   - Verify no regression in collision counts, sample counts, total
     removed volume vs round-02 baseline (those numbers were correct
     and shouldn't shift).

## Files

- `crates/rs_cam_core/src/compute/simulate.rs` (esp. `:386` —
  per-group stock dexel grid construction)
- `crates/rs_cam_core/src/compute/transform.rs` (esp. `:293`
  `effective_stock_bbox`)
- `crates/rs_cam_core/src/session/compute.rs` (esp. `:1140-1146`
  local-to-global gating)
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:486` — receives
  the (correct or incorrect) toolpath samples; understand here, don't
  edit here unless the diagnosis lands at this layer
- `crates/rs_cam_core/src/fingerprint.rs` — fingerprint suite
  regeneration in same PR; this fix will move axial-engagement
  numbers across the entire 54-sweep matrix

## Risk

M. Touches the sim coordinate frame which is foundational. The
fingerprint regen is unavoidable. Bench mark for a regression of the
core sim is the round-02 smoke evidence: AS013 (181k moves) sim
runtime + collision count + total volume should not move.

## Notes

- The implementer who diagnosed this (F-002 round-03 pickup) returned
  with a deliberate blocker rather than landing a band-aid. Their
  full reproduction probe is preserved in the agent's task output
  (not committed). Re-reading it before claiming F-024 is recommended.
- **Cross-references**: F-002's original split is real and landed.
  This finding is the next layer down — what value the split fields
  carry.
- F-017 (rapid collisions everywhere) may have related Z-frame
  symptoms worth investigating in the same fix's diff window.
