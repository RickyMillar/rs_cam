# F-028 — Face op cuts at world Z=-depth instead of world Z=stock_top-depth

- **Stage:** sim (planning/coordinate-frame bug, surfaces through simulator)
- **Severity:** medium
- **Status:** landed (commit `e48d7df`, 2026-05-25) + viz-path follow-up (commit `c9e203d`, 2026-05-25). **Follow-up 2 (viz-path)**: round-07 MCP smoke evidence (AS001 z_level=10/8/6 in viz path, peak_axial=0, air_cut=96 %) **did reproduce** through the GUI/MCP code path — `AppController::submit_toolpath_compute` (`controller/events/compute.rs`) built `HeightContext` from the local zero-rooted bbox even for identity setups. Site-1 fix in `session::compute::compute` only covered the direct `ProjectSession::run_simulation` API; viz/MCP path bypassed it. Single-line fix: gate `transform_setup = Some(...)` on `s.needs_transform()`. Regression test pins `heights.top_z = 0` for AS001 identity setup; pre-fix this read 12.0. Awaiting round-08 MCP smoke verification.
- **First found in:** round-05 (suspected); round-06 (confirmed independent of F-026/F-027); viz-path branch found round-07
- **Effort:** S–M (site 1: ~5 LOC in face/heights config; viz-path follow-up: 1 LOC + comment + regression test)
- **Linked PRs:** commit `e48d7df` (site 1, core), commit `c9e203d` (viz-path follow-up — `controller/events/compute.rs`)
- **Source audits:** round-06 verification of F-026 on AS004

## Evidence

`ux_step_plate_mdf.toml` (auto_from_model = true). Pre-F-026 stock
was z=12 (verbatim from TOML, stale). Post-F-026 stock grew to z=15
(model bbox z=10 + padding 5).

AS004 face op (`depth=1, depth_per_pass=0.5, stepover=3.0, feed=2400`,
End Mill 6mm, MDF):

| Metric | Round-05 (pre-F-026, stock.z=12) | Round-06 (post-F-026, stock.z=15) |
|---|---:|---:|
| `peak_axial_doc_mm` first pass | 9.14 | **11.42** (worse) |
| `deflection.peak_mm` | 0.204 | **0.243** (worse) |
| `rapid_collision_count` | 24 | 24 (same) |
| commanded `depth_per_pass` | 0.5 | 0.5 |

The `peak_axial_doc_mm` scales with stock height (~stock.z - 0.5),
not with commanded `depth_per_pass`. **The bug got worse when F-026
grew the stock.**

Per-pass report shows `z_level = -0.5` for the first face pass.
If z_level is "depth below stock top" (the convention used elsewhere),
then world Z = stock_top + z_level = 15 + (-0.5) = 14.5. But the
measured peak_axial_doc behavior (≈ stock height) suggests the cutter
is actually at world Z = -0.5 (interpreting z_level as world Z
directly), i.e. **below** the stock bottom (z=0), positioning the
cutter below the entire dexel grid → every ray cleared (F-024-class
symptom, but for the face op).

This is **NOT** F-027 (which is about adaptive3d planner's stock XY
bounds). AS004 is a face op; F-027 fix won't help here.

This is **NOT** F-024 (which fixed identity-setup grid frame for
pocket/adaptive/profile/zigzag/rest). The fix made all 5 those op
families' axial_doc match commanded. Face wasn't in the F-024 test
suite, and the bug evidently lives in face-specific z_level emission
rather than the dexel grid bounds.

## Root cause (hypothesis)

The face op's z_level emission likely uses a stock-top-relative
coordinate (z = -depth_below_top) that pre-F-024 was incorrectly
projected to world frame. F-024 fixed the dexel grid to use world
bounds; it did not touch the face op's z_level emission, so the
mismatch persists at the op-emission layer.

Mechanism: face emits `z_level = -depth_in_pass` (e.g. -0.5 for pass
1 at depth 0.5mm below stock top). The simulator interprets this as
**world Z = -0.5**, but the actual cutting Z should be **world Z =
stock_top - 0.5 = 14.5**. The cutter at world Z = -0.5 sits below
the stock-bounded dexel grid [0, stock.z], so the grid considers
every ray below the cutter cleared.

## Files

Likely starting points:

- `crates/rs_cam_core/src/face/` or similar — face op planner;
  search for `z_level`, `depth`, `stock_top` emission.
- `crates/rs_cam_core/src/compute/operation_configs.rs` — where
  face's `depth` param is consumed.
- `crates/rs_cam_core/src/session/compute.rs` — F-024 fix site;
  inspect whether the identity-setup conditional applies a transform
  to the face op's emitted toolpath frame.

Cross-reference F-024's commit `d82bd4d` and the viz-worker /
controller follow-ups (`0c907a6`, `67de558`) — F-028 may need
analogous patches at the same three sites or a higher-leverage fix
in the face op planner itself.

## Fix shape

Option A (highest leverage): face op emits world-frame Z. Compute
`world_z = stock_top - depth_in_pass` at emission time, not after.

Option B (uniform fix): all 2.5D ops emit stock-top-relative Z (a
local frame), and the session-level transform applies
`world_z = stock_top + local_z` after the fact. F-024 already
established the convention for identity setups — extend it to face
specifically.

Option C: at the simulator, detect when the cutter is outside the
dexel grid Z range and tag those samples as `MoveIntent::AboveStock`
or similar (instead of treating them as cutting). Wouldn't help if
the bug actually causes the cutter to mill at the wrong Z (the
g-code would be wrong), so only useful if the wrong-frame emission
is metric-only and not real-cut-wrong.

The right option depends on whether `inspect_spans` / cut_trace show
the face emitting actual G-code at z=-0.5 (real bug → Option A or B
required) or whether the simulator's z_level reporting is wrong
while the underlying G-code is correct (metric-only bug → Option C
sufficient).

## Acceptance test

1. Load `test_data/ux_step_plate_mdf.toml` via `ProjectSession`.
2. Add face op with AS004 params (depth=1, dpp=0.5, stepover=3.0,
   feed=2400).
3. Generate + simulate.
4. Assert: `peak_axial_doc_mm` for any pass `<= depth_per_pass + 0.1
   mm margin` (i.e. ≤ 0.6 for 0.5 mm DOC).
5. Assert: `deflection.peak_mm < 0.2`.
6. Assert: `rapid_collision_count == 0`.

Through `ProjectSession::run_simulation` per the production-entry-
point rule.

## Risk

S–M.

- S if Option A is feasible — single fix site at face op emission.
- M if face op emission can't be cleanly relocated and a session-
  level transform is needed (requires inventory of other 2.5D ops
  to confirm they all already do the right thing).

## Notes

- Likely **NOT a duplicate of F-027** — F-027 is adaptive3d planner
  stock-XY-bounds mismatch (a separate code path). F-028 is face op
  z-frame emission.
- Possibly **same root family as F-025** (non-identity setup z-frame)
  — if the face op's frame handling is shared with the local-to-
  global transform path, F-028's fix might also resolve F-025.
- The pre-F-026 evidence (round-05 AS004: 9.14 mm axial, 0.204 mm
  deflection) was originally attributed to F-026 candidate 2. Round-06
  evidence (axial scaled to 11.42 when stock grew to 15) demonstrates
  it's a separate bug.
</parameter>
</invoke>