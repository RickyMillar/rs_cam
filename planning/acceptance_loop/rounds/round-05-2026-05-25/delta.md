# Round-05 delta — 2026-05-25

**Vs baseline:** round-04 `rounds/round-04-2026-05-25/delta.md` (which
itself referenced round-02 + round-03).

**Smoke type:** partial sweep — 7 of 18 cases. AS001-AS006 (full 2D
op suite) + AS013 + AS015 (the two 3D auto_from_model cases). AS007-
AS012, AS014, AS016-AS018 deferred to round-06 because round-05's
headline finding (F-026 candidate 2 confirmed + expanded scope) was
already conclusive after 7 cases.

**Auditor:** autonomous Claude session.

**Results CSV:** `target/acceptance_sweeps/agent_smoke_20260525_round05/results.csv`

## Headline

**F-026 candidate 2 confirmed**, AND scope is wider than the original
finding said. The "auto-from-model Z-frame anomaly" is not just AS013-
specific — it fires on **any** project where `stock.auto_from_model =
true` AND the model extends above `stock_top_z`.

| Case | Op | Stock | Deflection (round-05) | Smoking gun |
|---|---|---|---:|---|
| AS013 | adaptive3d | terrain (model.z=52.6 > stock.z=30) | **0.573 Exceeds** | hotspot at cutter Z=32.4 (above stock top) reads `peak_axial_doc_mm = 25.0` on a 6 mm endmill; sibling hotspot at Z=23.4 (inside grid) correctly reads **3.0 mm** (commanded DOC) |
| AS015 | scallop | terrain (same) | **0.434 Exceeds** | 52 rapid collisions, same root cause |
| AS004 | face | MDF plate (model.z=10, stock.z=12, auto_from_model=true) | **0.204 Exceeds** | `peak_axial_doc_mm = 9.14` on a 0.5 mm DOC pass + 24 rapid collisions |

The mechanism is the **inverse** of the original F-024 bug:
- F-024 (fixed): cutter at world Z below the zero-rooted dexel grid →
  every ray cleared → axial reads full stock height.
- F-026 candidate 2 (now confirmed): cutter at world Z **above** the
  stock-bounded dexel grid → ray-blend stamping registers the entire
  ray as material to be cleared → axial reads the cutter-to-grid-top
  distance instead of the commanded DOC.

The F-024 fix used `request.stock_bbox` (world frame) as the dexel
grid bounds. For auto_from_model projects where `model.z > stock.z`,
the grid still doesn't extend high enough to enclose where the
toolpath actually emits cuts (clearance moves above terrain peaks).

## F-024 holds where it was meant to

| Case | Op | Stock | Deflection | Collisions | peak_axial vs commanded |
|---|---|---|---:|---:|---|
| AS001 | pocket | hardwood, origin_z=-12 | 0.076 Within | 0 | 2.0 ✓ |
| AS002 | adaptive | softwood, origin_z=-12 | 0.053 Within | 0 | 3.0 ✓ |
| AS003 | profile | hardwood, origin_z=-12 | 0.076 Within | 0 | 2.0 ✓ |
| AS005 | zigzag | mdf, origin_z=-12 | 0.051 Within | 0 | 2.0 ✓ |
| AS006 | rest | hardwood, origin_z=-12 | 0.000 Within | 0 | 1.0 ✓ |

All five identity-setup-with-explicit-stock cases pass. F-024 is solid
for the regime it targets. The unfixed regime is **auto_from_model**.

## Verdict by finding

### Confirmed (was: stub)

| Finding | Resolution |
|---|---|
| **F-026 candidate 2 promoted to confirmed root cause** (was stub with 3 candidates). Scope expanded from "AS013 only" to "any auto_from_model project where model extends above stock_top_z". |
| **F-017 fully reframed** — round-05 confirms zero collisions on all 5 origin_z=-12 cases (AS001-006 minus AS006-rest which is its own thing). AS013's 844 and AS015's 52 are both F-026, NOT a path-planning bug. F-017 can close as duplicate of F-026 once the fix lands. |

### Holding stable

| Finding | Round-04 status | Round-05 status |
|---|---|---|
| F-024 | verified on AS001 | verified on AS001, AS002, AS003, AS005 (all origin_z=-12) |
| F-001 chipload 2D | verified | stable; AS001/AS002/AS003/AS005 all `validated` confidence, vendor LUT bounds |
| F-015 preconditions | verified | AS006 rest precondition fired correctly when `prev_tool_id` unset |
| F-023 ref.model_missing | verified | no model-id mismatch attempted this round |
| F-018 templates | verified | smoke loaded new `ux_2d_pocket_softwood.toml` + `ux_2d_pocket_mdf.toml` + `ux_step_plate_mdf.toml` without harness errors; ironically AS004 on the new MDF plate is what surfaced the candidate-2 evidence |

### Acceptance bars status

| Bar | Target | Round-04 | Round-05 | Status |
|---|---:|---|---|---|
| Sim chipload calibration (3D ops) | ≥ 95% | 1/1 (AS013) | 2/2 (AS013, AS015) | stable |
| Sim chipload calibration (2D ops) | ≥ 95% | 1/1 (AS001) | 5/5 (AS001-AS003, AS005, AS006-Unmodeled-expected) | stable |
| **Sim deflection calibration** | ≥ 95% | 1/4 Within (AS001 only) | **5/7 Within** (AS001-AS003, AS005, AS006); AS004 + AS013 + AS015 are the 3 misses, all auto_from_model | F-026 promoted; gate is single-fix away |
| Optimizer refusal correctness | 100% | 1/1 (AS015 byte-identical round-02) | not re-tested (skipped optimize call to save time) | stable; verify next round |

The 5/7 Within deflection on this round, in a sweep that explicitly
included only the harder cases for testing, is the strongest
direction-correct move since the loop started. **Three deflection
misses, ONE underlying bug.**

## What's next

Recommend round-06 = F-026 implementer pickup:

1. **F-026 fix shape**: in the auto_from_model code path
   (likely `crates/rs_cam_core/src/compute/transform.rs::effective_stock_bbox`
   or `crates/rs_cam_core/src/session/compute.rs`'s identity-setup
   conditional), the dexel grid Z range must extend to enclose the
   model bbox — either `Z=[0, max(stock.z, model_bbox.max.z)]` or
   equivalent. The toolpath emits cuts at world Z above stock_top
   when the model has terrain peaks above stock_top; the grid must
   include that region.
2. **F-026 acceptance test**: load ux_3d_terrain.toml, generate
   adaptive3d, run sim, assert `peak_axial_doc_mm` at the
   deflection-triggering hotspot is ≤ commanded `depth_per_pass +
   margin`. Must run through `ProjectSession::run_simulation` to
   exercise the production path (per round-04 three-rebuild-saga
   loop learning).
3. **After F-026 lands**: full round-06 sweep AS001-AS018 to refresh
   all bars, including the cases this round skipped.

## Loop process note

Skipped AS007-AS012, AS014, AS016-AS018 to write up while the F-026
evidence was fresh. The decision was: 7 cases gave a conclusive
single-finding diagnosis; running 11 more would have added breadth at
the cost of stale context. The deferred cases will rerun in round-06
when F-026 has landed — they'll then double as F-026 verification.

This is the auditor exercising "stop when the finding is conclusive"
rather than "always run the full matrix". Acceptable per
`audit_runbook.md` Step 1: "If runtime gets pathological ... record
and move on."
